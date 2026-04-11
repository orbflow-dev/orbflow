// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Wave 3 integration tests: RBAC enforcement, auto-versioning on workflow
//! update, and change request lifecycle via the engine.
//!
//! Uses MemStore (which implements versioning methods) + MockBus for
//! deterministic, in-process testing without any I/O.

use std::sync::Arc;

use chrono::Utc;

use orbflow_core::OrbflowError;
use orbflow_core::credential::{Credential, CredentialId};
use orbflow_core::options::EngineOptionsBuilder;
use orbflow_core::ports::{Bus, CredentialStore, Engine, ListOptions, Store};
use orbflow_core::rbac::{Permission, PolicyBinding, PolicyScope, RbacPolicy};
use orbflow_core::task_subject;
use orbflow_core::wire::TaskMessage;
use orbflow_core::workflow::{
    DefinitionStatus, Edge, Node, NodeKind, NodeType, Parameter, ParameterMode, Position, Workflow,
    WorkflowId,
};
use orbflow_engine::OrbflowEngine;
use orbflow_memstore::MemStore;
use orbflow_testutil::MockBus;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_node(id: &str, plugin_ref: &str, kind: NodeKind) -> Node {
    Node {
        id: id.to_owned(),
        name: id.to_owned(),
        kind,
        node_type: NodeType::Builtin,
        plugin_ref: plugin_ref.to_owned(),
        input_mapping: None,
        config: None,
        parameters: vec![],
        retry: None,
        compensate: None,
        position: Position::default(),
        capability_ports: vec![],
        metadata: None,
        trigger_config: None,
        requires_approval: false,
    }
}

fn make_edge(id: &str, source: &str, target: &str) -> Edge {
    Edge {
        id: id.to_owned(),
        source: source.to_owned(),
        target: target.to_owned(),
        condition: None,
    }
}

fn make_workflow(id: &str, nodes: Vec<Node>, edges: Vec<Edge>) -> Workflow {
    Workflow {
        id: WorkflowId::new(id),
        name: format!("Test workflow {id}"),
        description: None,
        version: 0,
        status: DefinitionStatus::Draft,
        nodes,
        edges,
        capability_edges: vec![],
        triggers: vec![],
        annotations: vec![],
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

/// Creates an engine with MemStore (supports versioning) and optional RBAC.
fn build_engine_with_rbac(
    store: Arc<MemStore>,
    bus: Arc<MockBus>,
    rbac: Option<RbacPolicy>,
) -> Arc<OrbflowEngine> {
    let mut builder = EngineOptionsBuilder::new()
        .store(store.clone() as Arc<dyn Store>)
        .credential_store(store as Arc<dyn CredentialStore>)
        .bus(bus as Arc<dyn Bus>)
        .pool_name("test")
        .enable_resume(false);

    if let Some(policy) = rbac {
        builder = builder.rbac(policy);
    }

    let engine = Arc::new(OrbflowEngine::new(
        builder.build().expect("test engine options"),
    ));
    engine
}

/// Simple two-node workflow: trigger → action.
fn simple_workflow(id: &str) -> Workflow {
    let trigger = make_node("trigger-1", "builtin:trigger-manual", NodeKind::Trigger);
    let action = make_node("action-1", "builtin:log", NodeKind::Action);
    let edge = make_edge("e1", "trigger-1", "action-1");
    make_workflow(id, vec![trigger, action], vec![edge])
}

#[tokio::test]
async fn test_dispatch_includes_resolved_secret_credentials_for_worker_execution() {
    let store = Arc::new(MemStore::new());
    let bus = Arc::new(MockBus::new());
    let engine = build_engine_with_rbac(Arc::clone(&store), Arc::clone(&bus), None);

    let mut cred_data = std::collections::HashMap::new();
    cred_data.insert("api_key".into(), serde_json::json!("sk-test"));
    cred_data.insert(
        "base_url".into(),
        serde_json::json!("https://api.openai.com/v1"),
    );
    store
        .create_credential(&Credential {
            id: CredentialId::new("cred-1").unwrap(),
            name: "OpenAI".into(),
            credential_type: "openai".into(),
            data: cred_data,
            description: None,
            owner_id: None,
            access_tier: Default::default(),
            policy: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        })
        .await
        .unwrap();

    let trigger = make_node("trigger-1", "builtin:trigger-manual", NodeKind::Trigger);
    let mut action = make_node("action-1", "plugin:ai-codegen", NodeKind::Action);
    action.parameters = vec![Parameter {
        key: "credential_id".into(),
        mode: ParameterMode::Static,
        value: Some(serde_json::json!("cred-1")),
        expression: None,
    }];
    let wf = make_workflow(
        "wf-cred",
        vec![trigger, action],
        vec![make_edge("e1", "trigger-1", "action-1")],
    );

    engine.create_workflow(&wf).await.unwrap();
    engine
        .start_workflow(
            &WorkflowId::new("wf-cred"),
            std::collections::HashMap::new(),
        )
        .await
        .unwrap();

    let task_msgs = bus.messages_for(&task_subject("test"));
    assert_eq!(task_msgs.len(), 1, "expected one dispatched task");

    let task: TaskMessage = serde_json::from_slice(&task_msgs[0].data).unwrap();
    let params = task.parameters.expect("task should include parameters");
    // credential_id should be stripped from the dispatched task.
    assert_eq!(params.get("credential_id"), None);
    // Secret fields (field_type = "password") are redacted to Null before bus
    // publish to prevent plaintext secrets from traveling over NATS.
    assert_eq!(params.get("api_key"), Some(&serde_json::Value::Null));
    // Non-secret fields (field_type = "string") remain for the worker.
    assert_eq!(
        params.get("base_url"),
        Some(&serde_json::json!("https://api.openai.com/v1"))
    );
}

// ===========================================================================
// RBAC Tests
// ===========================================================================

#[tokio::test]
async fn test_rbac_check_permission_denies_without_execute() {
    // Setup RBAC policy where "alice" is a viewer (no Execute permission).
    let mut policy = RbacPolicy::with_defaults();
    policy
        .add_binding(PolicyBinding {
            subject: "alice".into(),
            role_id: "viewer".into(),
            scope: PolicyScope::Global,
        })
        .unwrap();

    let store = Arc::new(MemStore::new());
    let bus = Arc::new(MockBus::new());
    let engine = build_engine_with_rbac(store, bus, Some(policy));

    // The engine's check_permission should deny Execute for "alice".
    // We test this indirectly: alice only has View, not Execute.
    // Since check_permission is pub(crate), we verify via the RBAC policy directly.
    let rbac = RbacPolicy::with_defaults();
    let mut rbac_with_alice = rbac;
    rbac_with_alice
        .add_binding(PolicyBinding {
            subject: "alice".into(),
            role_id: "viewer".into(),
            scope: PolicyScope::Global,
        })
        .unwrap();

    assert!(
        rbac_with_alice.has_permission("alice", Permission::View, "wf-1", None),
        "alice should have View"
    );
    assert!(
        !rbac_with_alice.has_permission("alice", Permission::Execute, "wf-1", None),
        "alice should NOT have Execute"
    );

    // Engine should still be constructible with this policy (no panic).
    drop(engine);
}

#[tokio::test]
async fn test_rbac_check_permission_allows_with_execute() {
    // Setup RBAC policy where "bob" is an operator (has Execute permission).
    let mut policy = RbacPolicy::with_defaults();
    policy
        .add_binding(PolicyBinding {
            subject: "bob".into(),
            role_id: "operator".into(),
            scope: PolicyScope::Global,
        })
        .unwrap();

    assert!(
        policy.has_permission("bob", Permission::Execute, "wf-1", None),
        "bob should have Execute"
    );
    assert!(
        policy.has_permission("bob", Permission::View, "wf-1", None),
        "bob should have View"
    );
    assert!(
        policy.has_permission("bob", Permission::Approve, "wf-1", None),
        "bob should have Approve"
    );
    assert!(
        !policy.has_permission("bob", Permission::Edit, "wf-1", None),
        "bob (operator) should NOT have Edit"
    );
}

#[tokio::test]
async fn test_rbac_disabled_allows_all() {
    // Engine with no RBAC policy (None) should allow all operations.
    let store = Arc::new(MemStore::new());
    let bus = Arc::new(MockBus::new());
    let engine = build_engine_with_rbac(
        Arc::clone(&store),
        Arc::clone(&bus),
        None, // No RBAC
    );

    // Create and start a workflow — should succeed without any user_id checks.
    let wf = simple_workflow("wf-no-rbac");
    engine.create_workflow(&wf).await.unwrap();

    let fetched = engine
        .get_workflow(&WorkflowId::new("wf-no-rbac"))
        .await
        .unwrap();
    assert_eq!(fetched.name, "Test workflow wf-no-rbac");
}

#[tokio::test]
async fn test_rbac_node_level_permission_granularity() {
    // Carol has Execute only on a specific node in wf-1.
    let mut policy = RbacPolicy::with_defaults();
    policy
        .add_binding(PolicyBinding {
            subject: "carol".into(),
            role_id: "operator".into(),
            scope: PolicyScope::Node {
                workflow_id: "wf-1".into(),
                node_id: "sensitive-node".into(),
            },
        })
        .unwrap();

    // Carol can execute the specific node.
    assert!(policy.has_permission("carol", Permission::Execute, "wf-1", Some("sensitive-node")));

    // Carol cannot execute other nodes.
    assert!(!policy.has_permission("carol", Permission::Execute, "wf-1", Some("other-node")));

    // Carol cannot execute at workflow level.
    assert!(!policy.has_permission("carol", Permission::Execute, "wf-1", None));

    // Carol cannot execute in other workflows.
    assert!(!policy.has_permission("carol", Permission::Execute, "wf-2", Some("sensitive-node")));
}

#[tokio::test]
async fn test_rbac_admin_has_all_permissions() {
    let mut policy = RbacPolicy::with_defaults();
    policy
        .add_binding(PolicyBinding {
            subject: "admin-user".into(),
            role_id: "admin".into(),
            scope: PolicyScope::Global,
        })
        .unwrap();

    let all_perms = [
        Permission::View,
        Permission::Edit,
        Permission::Execute,
        Permission::Approve,
        Permission::Delete,
        Permission::ManageCredentials,
        Permission::Admin,
    ];

    for perm in &all_perms {
        assert!(
            policy.has_permission("admin-user", *perm, "any-wf", None),
            "admin should have {:?}",
            perm
        );
    }
}

#[tokio::test]
async fn test_rbac_effective_permissions_aggregation() {
    let mut policy = RbacPolicy::with_defaults();

    // Give bob both editor (View+Edit) at workflow level and operator (View+Execute+Approve) globally.
    policy
        .add_binding(PolicyBinding {
            subject: "bob".into(),
            role_id: "editor".into(),
            scope: PolicyScope::Workflow {
                workflow_id: "wf-1".into(),
            },
        })
        .unwrap();
    policy
        .add_binding(PolicyBinding {
            subject: "bob".into(),
            role_id: "operator".into(),
            scope: PolicyScope::Global,
        })
        .unwrap();

    let perms = policy.effective_permissions("bob", "wf-1", None);
    assert!(perms.contains(&Permission::View));
    assert!(perms.contains(&Permission::Edit));
    assert!(perms.contains(&Permission::Execute));
    assert!(perms.contains(&Permission::Approve));
    assert!(!perms.contains(&Permission::Delete));
}

// ===========================================================================
// Auto-Versioning Tests
// ===========================================================================

#[tokio::test]
async fn test_update_workflow_creates_version_snapshot() {
    let store = Arc::new(MemStore::new());
    let bus = Arc::new(MockBus::new());
    let engine = build_engine_with_rbac(Arc::clone(&store), Arc::clone(&bus), None);

    // Create workflow (version 1).
    let wf = simple_workflow("wf-ver");
    engine.create_workflow(&wf).await.unwrap();

    let created = engine
        .get_workflow(&WorkflowId::new("wf-ver"))
        .await
        .unwrap();
    assert_eq!(created.version, 1);

    // Update workflow — should snapshot version 1 and bump to version 2.
    let mut updated_wf = created.clone();
    updated_wf.name = "Updated workflow".into();
    engine.update_workflow(&updated_wf).await.unwrap();

    let after_update = engine
        .get_workflow(&WorkflowId::new("wf-ver"))
        .await
        .unwrap();
    assert_eq!(after_update.version, 2);
    assert_eq!(after_update.name, "Updated workflow");

    // Verify a version snapshot was saved for version 1.
    let (versions, total) = engine
        .list_workflow_versions(
            &WorkflowId::new("wf-ver"),
            ListOptions {
                offset: 0,
                limit: 100,
            },
        )
        .await
        .unwrap();

    assert_eq!(total, 1, "should have exactly 1 version snapshot");
    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0].version, 1);
    assert_eq!(versions[0].workflow_id, WorkflowId::new("wf-ver"));
}

#[tokio::test]
async fn test_multiple_updates_create_sequential_versions() {
    let store = Arc::new(MemStore::new());
    let bus = Arc::new(MockBus::new());
    let engine = build_engine_with_rbac(Arc::clone(&store), Arc::clone(&bus), None);

    let wf = simple_workflow("wf-multi-ver");
    engine.create_workflow(&wf).await.unwrap();

    // Update 3 times: v1→v2, v2→v3, v3→v4.
    for i in 1..=3 {
        let mut current = engine
            .get_workflow(&WorkflowId::new("wf-multi-ver"))
            .await
            .unwrap();
        current.name = format!("Version {}", i + 1);
        engine.update_workflow(&current).await.unwrap();
    }

    let final_wf = engine
        .get_workflow(&WorkflowId::new("wf-multi-ver"))
        .await
        .unwrap();
    assert_eq!(final_wf.version, 4);

    // Should have 3 version snapshots (v1, v2, v3).
    let (versions, total) = engine
        .list_workflow_versions(
            &WorkflowId::new("wf-multi-ver"),
            ListOptions {
                offset: 0,
                limit: 100,
            },
        )
        .await
        .unwrap();

    assert_eq!(total, 3, "should have 3 version snapshots");
    assert_eq!(versions.len(), 3);

    // Versions should be returned in descending order (newest first).
    assert_eq!(versions[0].version, 3);
    assert_eq!(versions[1].version, 2);
    assert_eq!(versions[2].version, 1);
}

#[tokio::test]
async fn test_get_specific_version_snapshot() {
    let store = Arc::new(MemStore::new());
    let bus = Arc::new(MockBus::new());
    let engine = build_engine_with_rbac(Arc::clone(&store), Arc::clone(&bus), None);

    let wf = simple_workflow("wf-get-ver");
    engine.create_workflow(&wf).await.unwrap();

    // Update once to create version 1 snapshot.
    let mut current = engine
        .get_workflow(&WorkflowId::new("wf-get-ver"))
        .await
        .unwrap();
    current.name = "After first update".into();
    engine.update_workflow(&current).await.unwrap();

    // Retrieve the specific version snapshot.
    let v1 = engine
        .get_workflow_version(&WorkflowId::new("wf-get-ver"), 1)
        .await
        .unwrap();

    assert_eq!(v1.version, 1);
    assert_eq!(v1.workflow_id, WorkflowId::new("wf-get-ver"));

    // The definition should contain the original workflow name.
    let def_name = v1.definition.get("name").and_then(|v| v.as_str());
    assert_eq!(def_name, Some("Test workflow wf-get-ver"));
}

#[tokio::test]
async fn test_get_nonexistent_version_returns_not_found() {
    let store = Arc::new(MemStore::new());
    let bus = Arc::new(MockBus::new());
    let engine = build_engine_with_rbac(Arc::clone(&store), Arc::clone(&bus), None);

    let wf = simple_workflow("wf-no-ver");
    engine.create_workflow(&wf).await.unwrap();

    // No updates yet, so no version snapshots exist.
    let err = engine
        .get_workflow_version(&WorkflowId::new("wf-no-ver"), 1)
        .await
        .unwrap_err();

    assert!(
        matches!(err, OrbflowError::NotFound),
        "expected NotFound, got {err:?}"
    );
}

#[tokio::test]
async fn test_version_snapshot_preserves_old_definition() {
    let store = Arc::new(MemStore::new());
    let bus = Arc::new(MockBus::new());
    let engine = build_engine_with_rbac(Arc::clone(&store), Arc::clone(&bus), None);

    // Create a workflow with specific node structure.
    let trigger = make_node("trigger-1", "builtin:trigger-manual", NodeKind::Trigger);
    let action = make_node("action-orig", "builtin:log", NodeKind::Action);
    let edge = make_edge("e1", "trigger-1", "action-orig");
    let wf = make_workflow("wf-preserve", vec![trigger, action], vec![edge]);
    engine.create_workflow(&wf).await.unwrap();

    // Update with different node structure.
    let mut updated = engine
        .get_workflow(&WorkflowId::new("wf-preserve"))
        .await
        .unwrap();
    let new_action = make_node("action-new", "builtin:log", NodeKind::Action);
    updated.nodes.push(new_action);
    updated
        .edges
        .push(make_edge("e2", "trigger-1", "action-new"));
    engine.update_workflow(&updated).await.unwrap();

    // Version 1 snapshot should have the OLD 2-node structure.
    let v1 = engine
        .get_workflow_version(&WorkflowId::new("wf-preserve"), 1)
        .await
        .unwrap();

    let nodes = v1.definition.get("nodes").and_then(|v| v.as_array());
    assert!(nodes.is_some(), "version snapshot should have nodes array");
    assert_eq!(
        nodes.unwrap().len(),
        2,
        "version 1 should have 2 nodes (trigger + action-orig)"
    );

    // Current workflow should have 3 nodes.
    let current = engine
        .get_workflow(&WorkflowId::new("wf-preserve"))
        .await
        .unwrap();
    assert_eq!(current.nodes.len(), 3);
    assert_eq!(current.version, 2);
}

// ===========================================================================
// Engine with RBAC + Versioning Combined
// ===========================================================================

#[tokio::test]
async fn test_engine_with_rbac_and_versioning_combined() {
    let mut policy = RbacPolicy::with_defaults();
    policy
        .add_binding(PolicyBinding {
            subject: "editor-user".into(),
            role_id: "editor".into(),
            scope: PolicyScope::Global,
        })
        .unwrap();

    let store = Arc::new(MemStore::new());
    let bus = Arc::new(MockBus::new());
    let engine = build_engine_with_rbac(Arc::clone(&store), Arc::clone(&bus), Some(policy));

    // Create + update workflow — versioning should work even with RBAC enabled.
    let wf = simple_workflow("wf-combined");
    engine.create_workflow(&wf).await.unwrap();

    let mut current = engine
        .get_workflow(&WorkflowId::new("wf-combined"))
        .await
        .unwrap();
    current.name = "Updated with RBAC".into();
    engine.update_workflow(&current).await.unwrap();

    let after = engine
        .get_workflow(&WorkflowId::new("wf-combined"))
        .await
        .unwrap();
    assert_eq!(after.version, 2);

    let (versions, _) = engine
        .list_workflow_versions(
            &WorkflowId::new("wf-combined"),
            ListOptions {
                offset: 0,
                limit: 100,
            },
        )
        .await
        .unwrap();
    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0].version, 1);
}

// ===========================================================================
// Missing Credential Tests
// ===========================================================================

#[tokio::test]
async fn test_missing_credential_marks_instance_failed() {
    let store = Arc::new(MemStore::new());
    let bus = Arc::new(MockBus::new());
    let engine = build_engine_with_rbac(Arc::clone(&store), Arc::clone(&bus), None);

    // Build a single-action workflow (no trigger) whose node references a
    // credential that does not exist in the store.
    let mut action = make_node("action-1", "plugin:some-service", NodeKind::Action);
    action.parameters = vec![Parameter {
        key: "credential_id".into(),
        mode: ParameterMode::Static,
        value: Some(serde_json::json!("nonexistent-cred")),
        expression: None,
    }];
    let wf = make_workflow("wf-missing-cred", vec![action], vec![]);

    engine.create_workflow(&wf).await.unwrap();

    let inst = engine
        .start_workflow(
            &WorkflowId::new("wf-missing-cred"),
            std::collections::HashMap::new(),
        )
        .await
        .unwrap();

    assert_eq!(
        inst.status,
        orbflow_core::InstanceStatus::Failed,
        "instance should be Failed when a referenced credential does not exist"
    );

    let node_state = inst
        .node_states
        .get("action-1")
        .expect("node state should exist");
    let error_msg = node_state.error.as_deref().unwrap_or("");
    assert!(
        error_msg.to_lowercase().contains("credential"),
        "node error should mention 'credential', got: {error_msg:?}"
    );
}
