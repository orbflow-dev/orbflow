// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Integration tests for the change request lifecycle using MemStore.
//!
//! Covers: create → submit → comment → approve → merge, rejection handling,
//! and stale version merge (version bumps from current).

use chrono::Utc;
use serde_json::json;

use orbflow_core::error::OrbflowError;
use orbflow_core::ports::{ChangeRequestStore, ListOptions, WorkflowStore};
use orbflow_core::versioning::{ChangeRequest, ChangeRequestStatus, ReviewComment};
use orbflow_core::workflow::{
    DefinitionStatus, Node, NodeKind, NodeType, Position, Workflow, WorkflowId,
};
use orbflow_memstore::MemStore;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn test_workflow(id: &str) -> Workflow {
    Workflow {
        id: WorkflowId::new(id),
        name: format!("Workflow {id}"),
        description: None,
        version: 1,
        status: DefinitionStatus::Active,
        nodes: vec![Node {
            id: "start".into(),
            name: "Start".into(),
            kind: NodeKind::Trigger,
            node_type: NodeType::Builtin,
            plugin_ref: "builtin:trigger-manual".into(),
            position: Position::default(),
            input_mapping: None,
            config: None,
            parameters: vec![],
            retry: None,
            compensate: None,
            capability_ports: vec![],
            metadata: None,
            trigger_config: None,
            requires_approval: false,
        }],
        edges: vec![],
        capability_edges: vec![],
        triggers: vec![],
        annotations: vec![],
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

fn make_change_request(
    id: &str,
    workflow_id: &str,
    base_version: i32,
    status: ChangeRequestStatus,
) -> ChangeRequest {
    ChangeRequest {
        id: id.into(),
        workflow_id: WorkflowId::new(workflow_id),
        title: format!("CR {id}"),
        description: Some("Test change request".into()),
        proposed_definition: json!({
            "nodes": [
                {"id": "start", "name": "Start", "kind": "trigger", "node_type": "builtin",
                 "plugin_ref": "builtin:trigger-manual", "position": {"x": 0, "y": 0},
                 "parameters": [], "capability_ports": [], "requires_approval": false},
                {"id": "new-node", "name": "New Node", "kind": "action", "node_type": "builtin",
                 "plugin_ref": "builtin:log", "position": {"x": 100, "y": 0},
                 "parameters": [], "capability_ports": [], "requires_approval": false}
            ],
            "edges": [
                {"id": "e1", "source": "start", "target": "new-node"}
            ]
        }),
        base_version,
        status,
        author: "alice".into(),
        reviewers: vec!["bob".into()],
        comments: vec![],
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

fn make_comment(id: &str, author: &str, body: &str) -> ReviewComment {
    ReviewComment {
        id: id.into(),
        author: author.into(),
        body: body.into(),
        node_id: None,
        edge_ref: None,
        resolved: false,
        created_at: Utc::now(),
    }
}

// ===========================================================================
// Change Request Lifecycle Tests
// ===========================================================================

#[tokio::test]
async fn test_change_request_full_lifecycle() {
    let store = MemStore::new();

    // Create the workflow.
    let wf = test_workflow("wf-1");
    store.create_workflow(&wf).await.unwrap();

    // Step 1: Create CR in Draft status.
    let cr = make_change_request("cr-1", "wf-1", 1, ChangeRequestStatus::Draft);
    store.create_change_request(&cr).await.unwrap();

    let fetched = store.get_change_request("cr-1").await.unwrap();
    assert_eq!(fetched.status, ChangeRequestStatus::Draft);

    // Step 2: Submit (transition to Open).
    let mut cr_open = fetched;
    cr_open.status = ChangeRequestStatus::Open;
    cr_open.updated_at = Utc::now();
    store.update_change_request(&cr_open).await.unwrap();

    let fetched = store.get_change_request("cr-1").await.unwrap();
    assert_eq!(fetched.status, ChangeRequestStatus::Open);

    // Step 3: Add a review comment.
    let comment = make_comment("comment-1", "bob", "Looks good to me!");
    store.add_comment("cr-1", &comment).await.unwrap();

    let fetched = store.get_change_request("cr-1").await.unwrap();
    assert_eq!(fetched.comments.len(), 1);
    assert_eq!(fetched.comments[0].body, "Looks good to me!");
    assert!(!fetched.comments[0].resolved);

    // Step 4: Resolve the comment.
    store
        .resolve_comment("cr-1", "comment-1", true)
        .await
        .unwrap();
    let fetched = store.get_change_request("cr-1").await.unwrap();
    assert!(fetched.comments[0].resolved);

    // Step 5: Approve.
    let mut cr_approved = fetched;
    cr_approved.status = ChangeRequestStatus::Approved;
    cr_approved.updated_at = Utc::now();
    store.update_change_request(&cr_approved).await.unwrap();

    let fetched = store.get_change_request("cr-1").await.unwrap();
    assert_eq!(fetched.status, ChangeRequestStatus::Approved);

    // Step 6: Merge — should update workflow definition and mark CR as Merged.
    let proposed_def = fetched.proposed_definition.clone();
    store
        .merge_change_request("cr-1", 1, &proposed_def)
        .await
        .unwrap();

    // Verify CR is now Merged.
    let merged_cr = store.get_change_request("cr-1").await.unwrap();
    assert_eq!(merged_cr.status, ChangeRequestStatus::Merged);

    // Verify workflow was updated (version bumped).
    let updated_wf = store.get_workflow(&WorkflowId::new("wf-1")).await.unwrap();
    assert_eq!(
        updated_wf.version, 2,
        "version should be bumped after merge"
    );
}

#[tokio::test]
async fn test_merge_rejected_cr_fails_with_conflict() {
    let store = MemStore::new();

    let wf = test_workflow("wf-reject");
    store.create_workflow(&wf).await.unwrap();

    // Create CR and reject it.
    let cr = make_change_request("cr-rejected", "wf-reject", 1, ChangeRequestStatus::Rejected);
    store.create_change_request(&cr).await.unwrap();

    // Try to merge a rejected CR — should fail with Conflict.
    let err = store
        .merge_change_request("cr-rejected", 1, &json!({}))
        .await
        .unwrap_err();

    assert!(
        matches!(err, OrbflowError::Conflict),
        "expected Conflict when merging rejected CR, got {err:?}"
    );

    // Workflow version should remain unchanged.
    let wf = store
        .get_workflow(&WorkflowId::new("wf-reject"))
        .await
        .unwrap();
    assert_eq!(wf.version, 1);
}

#[tokio::test]
async fn test_merge_draft_cr_fails_with_conflict() {
    let store = MemStore::new();

    let wf = test_workflow("wf-draft");
    store.create_workflow(&wf).await.unwrap();

    // Create CR in Draft status (not yet approved).
    let cr = make_change_request("cr-draft", "wf-draft", 1, ChangeRequestStatus::Draft);
    store.create_change_request(&cr).await.unwrap();

    // Try to merge a draft CR — should fail.
    let err = store
        .merge_change_request("cr-draft", 1, &json!({}))
        .await
        .unwrap_err();

    assert!(
        matches!(err, OrbflowError::Conflict),
        "expected Conflict when merging draft CR, got {err:?}"
    );
}

#[tokio::test]
async fn test_merge_open_cr_fails_with_conflict() {
    let store = MemStore::new();

    let wf = test_workflow("wf-open");
    store.create_workflow(&wf).await.unwrap();

    // Create CR in Open status (submitted but not approved).
    let cr = make_change_request("cr-open", "wf-open", 1, ChangeRequestStatus::Open);
    store.create_change_request(&cr).await.unwrap();

    let err = store
        .merge_change_request("cr-open", 1, &json!({}))
        .await
        .unwrap_err();

    assert!(
        matches!(err, OrbflowError::Conflict),
        "expected Conflict when merging non-approved CR, got {err:?}"
    );
}

#[tokio::test]
async fn test_merge_stale_version_succeeds() {
    let store = MemStore::new();

    // Create workflow at version 1.
    let wf = test_workflow("wf-stale");
    store.create_workflow(&wf).await.unwrap();

    // Simulate workflow being updated to version 2 (by someone else).
    let mut wf_v2 = store
        .get_workflow(&WorkflowId::new("wf-stale"))
        .await
        .unwrap();
    wf_v2.version = 2;
    wf_v2.name = "Updated externally".into();
    store.update_workflow(&wf_v2).await.unwrap();

    // Create CR based on version 1 (now stale) and approve it.
    let cr = make_change_request("cr-stale", "wf-stale", 1, ChangeRequestStatus::Approved);
    store.create_change_request(&cr).await.unwrap();

    // Merge should succeed — proposed definition is the desired end state,
    // version bumps from current (2 → 3).
    let new_def = json!({"nodes": [{"id": "merged"}], "edges": []});
    store
        .merge_change_request("cr-stale", 1, &new_def)
        .await
        .unwrap();

    let merged_cr = store.get_change_request("cr-stale").await.unwrap();
    assert_eq!(merged_cr.status, ChangeRequestStatus::Merged);

    // Workflow should now be at version 3 (bumped from 2).
    let wf = store
        .get_workflow(&WorkflowId::new("wf-stale"))
        .await
        .unwrap();
    assert_eq!(wf.version, 3);
}

#[tokio::test]
async fn test_list_change_requests_by_workflow() {
    let store = MemStore::new();

    let wf1 = test_workflow("wf-list-1");
    let wf2 = test_workflow("wf-list-2");
    store.create_workflow(&wf1).await.unwrap();
    store.create_workflow(&wf2).await.unwrap();

    // Create CRs for different workflows.
    let cr1 = make_change_request("cr-a", "wf-list-1", 1, ChangeRequestStatus::Open);
    let cr2 = make_change_request("cr-b", "wf-list-1", 1, ChangeRequestStatus::Draft);
    let cr3 = make_change_request("cr-c", "wf-list-2", 1, ChangeRequestStatus::Open);
    store.create_change_request(&cr1).await.unwrap();
    store.create_change_request(&cr2).await.unwrap();
    store.create_change_request(&cr3).await.unwrap();

    // List CRs for wf-list-1 (all statuses).
    let (crs, total) = store
        .list_change_requests(
            &WorkflowId::new("wf-list-1"),
            None,
            ListOptions {
                offset: 0,
                limit: 100,
            },
        )
        .await
        .unwrap();
    assert_eq!(total, 2);
    assert_eq!(crs.len(), 2);

    // List CRs for wf-list-1 filtered by Open status.
    let (open_crs, open_total) = store
        .list_change_requests(
            &WorkflowId::new("wf-list-1"),
            Some(ChangeRequestStatus::Open),
            ListOptions {
                offset: 0,
                limit: 100,
            },
        )
        .await
        .unwrap();
    assert_eq!(open_total, 1);
    assert_eq!(open_crs[0].id, "cr-a");

    // List CRs for wf-list-2.
    let (crs2, total2) = store
        .list_change_requests(
            &WorkflowId::new("wf-list-2"),
            None,
            ListOptions {
                offset: 0,
                limit: 100,
            },
        )
        .await
        .unwrap();
    assert_eq!(total2, 1);
    assert_eq!(crs2[0].id, "cr-c");
}

#[tokio::test]
async fn test_create_duplicate_change_request_fails() {
    let store = MemStore::new();

    let wf = test_workflow("wf-dup");
    store.create_workflow(&wf).await.unwrap();

    let cr = make_change_request("cr-dup", "wf-dup", 1, ChangeRequestStatus::Draft);
    store.create_change_request(&cr).await.unwrap();

    let err = store.create_change_request(&cr).await.unwrap_err();
    assert!(
        matches!(err, OrbflowError::AlreadyExists),
        "expected AlreadyExists, got {err:?}"
    );
}

#[tokio::test]
async fn test_get_nonexistent_change_request_returns_not_found() {
    let store = MemStore::new();

    let err = store.get_change_request("nonexistent").await.unwrap_err();
    assert!(
        matches!(err, OrbflowError::NotFound),
        "expected NotFound, got {err:?}"
    );
}

#[tokio::test]
async fn test_add_multiple_comments_to_change_request() {
    let store = MemStore::new();

    let wf = test_workflow("wf-comments");
    store.create_workflow(&wf).await.unwrap();

    let cr = make_change_request("cr-comments", "wf-comments", 1, ChangeRequestStatus::Open);
    store.create_change_request(&cr).await.unwrap();

    // Add multiple comments.
    store
        .add_comment("cr-comments", &make_comment("c1", "bob", "First comment"))
        .await
        .unwrap();
    store
        .add_comment(
            "cr-comments",
            &make_comment("c2", "carol", "Second comment"),
        )
        .await
        .unwrap();
    store
        .add_comment(
            "cr-comments",
            &ReviewComment {
                id: "c3".into(),
                author: "bob".into(),
                body: "Comment on specific node".into(),
                node_id: Some("new-node".into()),
                edge_ref: None,
                resolved: false,
                created_at: Utc::now(),
            },
        )
        .await
        .unwrap();

    let fetched = store.get_change_request("cr-comments").await.unwrap();
    assert_eq!(fetched.comments.len(), 3);
    assert_eq!(fetched.comments[0].author, "bob");
    assert_eq!(fetched.comments[1].author, "carol");
    assert_eq!(fetched.comments[2].node_id, Some("new-node".into()));
}

#[tokio::test]
async fn test_merge_approved_cr_applies_definition() {
    let store = MemStore::new();

    // Create workflow with 1 node.
    let wf = test_workflow("wf-apply");
    store.create_workflow(&wf).await.unwrap();

    let original = store
        .get_workflow(&WorkflowId::new("wf-apply"))
        .await
        .unwrap();
    assert_eq!(original.nodes.len(), 1);

    // Create an approved CR that adds a second node.
    let cr = make_change_request("cr-apply", "wf-apply", 1, ChangeRequestStatus::Approved);
    store.create_change_request(&cr).await.unwrap();

    let proposed = cr.proposed_definition.clone();
    store
        .merge_change_request("cr-apply", 1, &proposed)
        .await
        .unwrap();

    // Verify the workflow version was bumped and CR is merged.
    let updated = store
        .get_workflow(&WorkflowId::new("wf-apply"))
        .await
        .unwrap();
    assert_eq!(updated.version, 2, "version should be bumped after merge");

    let merged_cr = store.get_change_request("cr-apply").await.unwrap();
    assert_eq!(merged_cr.status, ChangeRequestStatus::Merged);
}

#[tokio::test]
async fn test_merge_nonexistent_cr_returns_not_found() {
    let store = MemStore::new();

    let err = store
        .merge_change_request("nonexistent", 1, &json!({}))
        .await
        .unwrap_err();

    assert!(
        matches!(err, OrbflowError::NotFound),
        "expected NotFound, got {err:?}"
    );
}

#[tokio::test]
async fn test_resolve_nonexistent_comment_returns_not_found() {
    let store = MemStore::new();

    let wf = test_workflow("wf-resolve");
    store.create_workflow(&wf).await.unwrap();

    let cr = make_change_request("cr-resolve", "wf-resolve", 1, ChangeRequestStatus::Open);
    store.create_change_request(&cr).await.unwrap();

    let err = store
        .resolve_comment("cr-resolve", "nonexistent-comment", true)
        .await
        .unwrap_err();

    assert!(
        matches!(err, OrbflowError::NotFound),
        "expected NotFound, got {err:?}"
    );
}
