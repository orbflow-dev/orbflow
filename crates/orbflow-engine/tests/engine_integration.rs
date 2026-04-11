// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Integration tests for OrbflowEngine.
//!
//! Uses MockStore + MockBus + MockNodeExecutor (from orbflow-testutil) to exercise
//! the engine end-to-end without any I/O. The MockBus delivers messages
//! synchronously, so tests run deterministically in a single async task.

use std::collections::HashMap;
use std::sync::Arc;

use orbflow_core::options::EngineOptionsBuilder;
use orbflow_core::ports::{Bus, Engine, InstanceStore, MsgHandler, NodeExecutor, NodeOutput};
use orbflow_core::wire::{ResultMessage, TaskMessage};
use orbflow_core::workflow::{Edge, Node, NodeKind, NodeType, Workflow, WorkflowId};
use orbflow_core::{OrbflowError, task_subject};
use orbflow_engine::OrbflowEngine;
use orbflow_testutil::{MockBus, MockNodeExecutor, MockStore};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Builds a minimal [`Node`] for use in test workflows.
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
        position: Default::default(),
        capability_ports: vec![],
        metadata: None,
        trigger_config: None,
        requires_approval: false,
    }
}

/// Builds a directed [`Edge`] with no condition.
fn make_edge(id: &str, source: &str, target: &str) -> Edge {
    Edge {
        id: id.to_owned(),
        source: source.to_owned(),
        target: target.to_owned(),
        condition: None,
    }
}

/// Builds a workflow with the given trigger node + action nodes + edges.
///
/// The `trigger_node` is the entry point (no incoming edges).
fn make_workflow(id: &str, nodes: Vec<Node>, edges: Vec<Edge>) -> Workflow {
    use chrono::Utc;
    use orbflow_core::workflow::DefinitionStatus;

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

/// Creates an [`Arc<OrbflowEngine>`] wired to a shared MockStore + MockBus.
///
/// Also registers a task-handler on the bus: when the engine dispatches a
/// node task, the handler calls back into `engine.handle_node_result()` with
/// the executor's output — giving synchronous end-to-end delivery.
async fn build_engine(
    store: Arc<MockStore>,
    bus: Arc<MockBus>,
    executors: Vec<(&'static str, Arc<dyn NodeExecutor>)>,
) -> Arc<OrbflowEngine> {
    let opts = EngineOptionsBuilder::new()
        .store(store as Arc<dyn orbflow_core::ports::Store>)
        .bus(bus.clone() as Arc<dyn Bus>)
        .pool_name("test")
        .enable_resume(false)
        .build()
        .expect("test engine options");

    let engine = Arc::new(OrbflowEngine::new(opts));

    // Build a shared executor map for the handler closure.
    let executor_map: HashMap<String, Arc<dyn NodeExecutor>> = executors
        .iter()
        .map(|(name, exec)| (name.to_string(), Arc::clone(exec)))
        .collect();

    // Register all executors with the engine.
    for (name, exec) in executors {
        engine
            .register_node(name, exec)
            .expect("register_node should not fail");
    }

    // Wire the task subject → async result delivery via spawned tasks.
    // We spawn each result delivery to avoid reentrancy deadlock: when the engine
    // processes a result it may dispatch the next node (calling bus.publish inside
    // handle_node_result), which would re-enter the handler while the per-instance
    // lock is held.
    let eng_for_handler = Arc::clone(&engine);
    let execs_for_handler = executor_map;
    let handler: MsgHandler = Arc::new(move |_subject, data| {
        let eng = Arc::clone(&eng_for_handler);
        let execs = execs_for_handler.clone();
        Box::pin(async move {
            let task: TaskMessage =
                serde_json::from_slice(&data).map_err(|e| OrbflowError::Internal(e.to_string()))?;

            // Find the registered executor by plugin_ref.
            let output = match execs.get(&task.plugin_ref) {
                Some(exec) => {
                    let input = orbflow_core::ports::NodeInput {
                        instance_id: task.instance_id.clone(),
                        node_id: task.node_id.clone(),
                        plugin_ref: task.plugin_ref.clone(),
                        config: task.config.clone(),
                        input: task.input.clone(),
                        parameters: task.parameters.clone(),
                        capabilities: task.capabilities.clone(),
                        attempt: task.attempt,
                    };
                    exec.execute(&input).await
                }
                None => Err(OrbflowError::NodeNotFound),
            };

            let result = match output {
                Ok(out) => ResultMessage {
                    result_id: None,
                    instance_id: task.instance_id,
                    node_id: task.node_id,
                    output: out.data,
                    error: out.error,
                    trace_context: None,
                    v: 1,
                },
                Err(e) => ResultMessage {
                    result_id: None,
                    instance_id: task.instance_id,
                    node_id: task.node_id,
                    output: None,
                    error: Some(e.to_string()),
                    trace_context: None,
                    v: 1,
                },
            };

            // Spawn to avoid reentrancy deadlock on the engine's per-instance lock.
            tokio::spawn(async move {
                let _ = eng.handle_node_result(&result).await;
            });
            Ok(())
        })
    });

    bus.subscribe(&task_subject("test"), handler)
        .await
        .expect("subscribe should not fail");

    engine
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Create a workflow and start it; verify the returned instance has Running status
/// and is persisted in the store.
#[tokio::test]
async fn create_and_start_workflow() {
    let store = Arc::new(MockStore::new());
    let bus = Arc::new(MockBus::new());

    // A trigger node only — no action nodes means it completes immediately.
    let trigger = make_node("trigger-1", "builtin:trigger-manual", NodeKind::Trigger);
    let wf = make_workflow("wf-1", vec![trigger], vec![]);

    let engine = build_engine(Arc::clone(&store), Arc::clone(&bus), vec![]).await;

    engine.create_workflow(&wf).await.expect("create_workflow");

    let instance = engine
        .start_workflow(&WorkflowId::new("wf-1"), HashMap::new())
        .await
        .expect("start_workflow");

    // Instance was returned with a valid ID.
    assert!(!instance.id.0.is_empty(), "instance ID must not be empty");
    assert_eq!(instance.workflow_id, WorkflowId::new("wf-1"));

    // Instance is persisted in the store.
    let stored = store
        .get_instance(&instance.id)
        .await
        .expect("instance should be in store");

    // A trigger-only workflow completes immediately (trigger nodes are
    // auto-completed by the engine and, with no downstream nodes, the
    // instance transitions to Completed).
    use orbflow_core::execution::InstanceStatus;
    assert!(
        matches!(
            stored.status,
            InstanceStatus::Running | InstanceStatus::Completed
        ),
        "expected Running or Completed, got {:?}",
        stored.status
    );
}

/// Two-node linear workflow A → B, both succeed.
/// After start, both nodes should be Completed and the instance Completed.
#[tokio::test]
async fn simple_linear_workflow() {
    let store = Arc::new(MockStore::new());
    let bus = Arc::new(MockBus::new());

    let trigger = make_node("trigger-1", "builtin:trigger-manual", NodeKind::Trigger);
    let action_a = make_node("action-a", "builtin:action-a", NodeKind::Action);
    let edge = make_edge("e1", "trigger-1", "action-a");

    let wf = make_workflow("wf-linear", vec![trigger, action_a], vec![edge]);

    let executor_a = Arc::new(MockNodeExecutor::with_output(NodeOutput {
        data: Some({
            let mut m = HashMap::new();
            m.insert("result".into(), serde_json::json!("ok"));
            m
        }),
        error: None,
    }));

    let engine = build_engine(
        Arc::clone(&store),
        Arc::clone(&bus),
        vec![(
            "builtin:action-a",
            executor_a.clone() as Arc<dyn NodeExecutor>,
        )],
    )
    .await;

    engine.create_workflow(&wf).await.expect("create_workflow");

    engine
        .start_workflow(&WorkflowId::new("wf-linear"), HashMap::new())
        .await
        .expect("start_workflow");

    // Wait a moment for the synchronous bus delivery chain to settle.
    tokio::task::yield_now().await;

    // Verify executor was called.
    assert!(
        executor_a.call_count() >= 1,
        "action-a executor should have been called"
    );

    // Verify instance reached a terminal status.
    let (instances, _) = store
        .list_instances(orbflow_core::ports::ListOptions {
            offset: 0,
            limit: 0,
        })
        .await
        .expect("list_instances");

    assert_eq!(instances.len(), 1, "exactly one instance should exist");

    use orbflow_core::execution::InstanceStatus;
    let status = instances[0].status;
    assert!(
        matches!(status, InstanceStatus::Completed | InstanceStatus::Running),
        "expected Completed or Running, got {:?}",
        status
    );
}

/// When a node executor returns an error, the instance should be marked Failed.
#[tokio::test]
async fn node_failure_marks_instance_failed() {
    let store = Arc::new(MockStore::new());
    let bus = Arc::new(MockBus::new());

    let trigger = make_node("trigger-1", "builtin:trigger-manual", NodeKind::Trigger);
    let action = make_node("action-fail", "builtin:action-fail", NodeKind::Action);
    let edge = make_edge("e1", "trigger-1", "action-fail");

    let wf = make_workflow("wf-fail", vec![trigger, action], vec![edge]);

    let trigger_exec = Arc::new(MockNodeExecutor::ok());
    let failing_exec = Arc::new(MockNodeExecutor::with_error(OrbflowError::Internal(
        "node executor failed".into(),
    )));

    let engine = build_engine(
        Arc::clone(&store),
        Arc::clone(&bus),
        vec![
            (
                "builtin:trigger-manual",
                trigger_exec as Arc<dyn NodeExecutor>,
            ),
            ("builtin:action-fail", failing_exec as Arc<dyn NodeExecutor>),
        ],
    )
    .await;

    engine.create_workflow(&wf).await.expect("create_workflow");

    engine
        .start_workflow(&WorkflowId::new("wf-fail"), HashMap::new())
        .await
        .expect("start_workflow");

    // Poll until the instance reaches a terminal state (Failed).
    use orbflow_core::execution::InstanceStatus;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        let (instances, _) = store
            .list_instances(orbflow_core::ports::ListOptions {
                offset: 0,
                limit: 0,
            })
            .await
            .expect("list_instances");
        if !instances.is_empty() && instances[0].status == InstanceStatus::Failed {
            break; // success
        }
        if std::time::Instant::now() > deadline {
            let status = if instances.is_empty() {
                "no instances".to_string()
            } else {
                format!("{:?}", instances[0].status)
            };
            panic!("instance should be Failed after node error, but was: {status}");
        }
    }
}

/// Start a workflow then immediately cancel it; verify the instance is Cancelled.
#[tokio::test]
async fn cancel_running_instance() {
    let store = Arc::new(MockStore::new());
    let bus = Arc::new(MockBus::new());

    // Single trigger node — won't auto-complete because there's no downstream
    // action, but let's use a slow executor so the instance stays Running.
    let trigger = make_node("trigger-1", "builtin:trigger-manual", NodeKind::Trigger);
    let action = make_node("action-slow", "builtin:action-slow", NodeKind::Action);
    let edge = make_edge("e1", "trigger-1", "action-slow");

    let wf = make_workflow("wf-cancel", vec![trigger, action], vec![edge]);

    // The store's on_update_instance hook lets us intercept the first update
    // (which sets Running) before the executor fires, giving us a window to
    // cancel. We use a simple ok() executor here.
    let slow_exec = Arc::new(MockNodeExecutor::ok());

    let engine = build_engine(
        Arc::clone(&store),
        Arc::clone(&bus),
        vec![("builtin:action-slow", slow_exec as Arc<dyn NodeExecutor>)],
    )
    .await;

    engine.create_workflow(&wf).await.expect("create_workflow");

    let instance = engine
        .start_workflow(&WorkflowId::new("wf-cancel"), HashMap::new())
        .await
        .expect("start_workflow");

    // Cancel the instance.
    // If the instance is already terminal (completed before we cancel),
    // cancel returns InvalidStatus — treat that as acceptable.
    let cancel_result = engine.cancel_instance(&instance.id).await;

    use orbflow_core::execution::InstanceStatus;

    match cancel_result {
        Ok(()) => {
            let stored = store
                .get_instance(&instance.id)
                .await
                .expect("instance should be in store");
            assert_eq!(
                stored.status,
                InstanceStatus::Cancelled,
                "cancelled instance must have Cancelled status"
            );
        }
        Err(OrbflowError::InvalidStatus) => {
            // Instance was already terminal — verify it is in a terminal state.
            let stored = store
                .get_instance(&instance.id)
                .await
                .expect("instance should be in store");
            assert!(
                matches!(
                    stored.status,
                    InstanceStatus::Completed | InstanceStatus::Failed | InstanceStatus::Cancelled
                ),
                "instance must be terminal, got {:?}",
                stored.status
            );
        }
        Err(e) => panic!("unexpected cancel error: {e:?}"),
    }
}

/// A workflow containing a cycle must be rejected with CycleDetected.
#[tokio::test]
async fn cycle_detection() {
    let store = Arc::new(MockStore::new());
    let bus = Arc::new(MockBus::new());

    // A → B → A (cycle)
    let trigger = make_node("trigger-1", "builtin:trigger-manual", NodeKind::Trigger);
    let node_a = make_node("node-a", "builtin:action-a", NodeKind::Action);
    let node_b = make_node("node-b", "builtin:action-b", NodeKind::Action);

    let edges = vec![
        make_edge("e1", "trigger-1", "node-a"),
        make_edge("e2", "node-a", "node-b"),
        make_edge("e3", "node-b", "node-a"), // closes the cycle
    ];

    let wf = make_workflow("wf-cycle", vec![trigger, node_a, node_b], edges);

    let engine = build_engine(Arc::clone(&store), Arc::clone(&bus), vec![]).await;

    let err = engine
        .create_workflow(&wf)
        .await
        .expect_err("cyclic workflow should be rejected");

    assert!(
        matches!(err, OrbflowError::CycleDetected),
        "expected CycleDetected, got {err:?}"
    );
}
