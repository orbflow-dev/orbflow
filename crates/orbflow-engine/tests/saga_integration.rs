// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Saga compensation integration tests.
//!
//! Exercises `orbflow-engine/src/saga.rs` via the public Engine API:
//! builds a workflow where a downstream node fails, verifies the engine
//! walks completed upstream nodes in reverse topological order,
//! dispatches compensation tasks for those with `compensate` configs,
//! and emits `CompensationStarted` / `CompensationCompleted` events.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;

use orbflow_core::event::{DomainEvent, EventType};
use orbflow_core::options::EngineOptionsBuilder;
use orbflow_core::ports::{
    Bus, Engine, EventStore, InstanceStore, ListOptions, MsgHandler, NodeExecutor,
};
use orbflow_core::wire::{ResultMessage, TaskMessage};
use orbflow_core::workflow::{
    CompensateConfig, DefinitionStatus, Edge, Node, NodeKind, NodeType, Position, Workflow,
    WorkflowId,
};
use orbflow_core::{OrbflowError, task_subject};
use orbflow_engine::OrbflowEngine;
use orbflow_testutil::{MockBus, MockNodeExecutor, MockStore};

// Helpers

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

fn make_node_with_compensate(id: &str, plugin_ref: &str, compensate_plugin: &str) -> Node {
    let mut n = make_node(id, plugin_ref, NodeKind::Action);
    n.compensate = Some(CompensateConfig {
        plugin_ref: compensate_plugin.to_owned(),
        input_mapping: None,
    });
    n
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
        name: format!("Saga test {id}"),
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

/// Constructs an engine wired to MockStore + MockBus and subscribes a task
/// handler that routes bus tasks to registered executors, then pipes their
/// outputs back through `engine.handle_node_result`. Mirrors the pattern used
/// by `engine_integration.rs`.
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

    let executor_map: HashMap<String, Arc<dyn NodeExecutor>> = executors
        .iter()
        .map(|(name, exec)| ((*name).to_string(), Arc::clone(exec)))
        .collect();

    for (name, exec) in executors {
        engine.register_node(name, exec).expect("register_node");
    }

    let eng_for_handler = Arc::clone(&engine);
    let execs_for_handler = executor_map;
    let handler: MsgHandler = Arc::new(move |_subject, data| {
        let eng = Arc::clone(&eng_for_handler);
        let execs = execs_for_handler.clone();
        Box::pin(async move {
            let task: TaskMessage =
                serde_json::from_slice(&data).map_err(|e| OrbflowError::Internal(e.to_string()))?;

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
                    attempt: task.attempt,
                    dispatch_id: task.dispatch_id,
                    output: out.data,
                    error: out.error,
                    trace_context: None,
                    v: 1,
                },
                Err(e) => ResultMessage {
                    result_id: None,
                    instance_id: task.instance_id,
                    node_id: task.node_id,
                    attempt: task.attempt,
                    dispatch_id: task.dispatch_id,
                    output: None,
                    error: Some(e.to_string()),
                    trace_context: None,
                    v: 1,
                },
            };

            tokio::spawn(async move {
                let _ = eng.handle_node_result(&result).await;
            });
            Ok(())
        })
    });

    bus.subscribe(&task_subject("test"), handler)
        .await
        .expect("subscribe");

    engine
}

/// Polls `predicate` every 20ms until it returns Some, or times out.
async fn wait_for<T, F, Fut>(timeout: std::time::Duration, mut predicate: F) -> Option<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Option<T>>,
{
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if let Some(v) = predicate().await {
            return Some(v);
        }
        if std::time::Instant::now() > deadline {
            return None;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
}

// Tests

/// Saga: upstream node A (with compensate) completes, downstream B fails.
/// Engine must:
///   1. start saga compensation (emit CompensationStarted with failed_node=action-b)
///   2. dispatch _compensate_action-a via the bus using A's compensate plugin_ref
///   3. on compensation result, emit CompensationCompleted
#[tokio::test]
async fn saga_compensation_runs_for_completed_upstream_when_downstream_fails() {
    let store = Arc::new(MockStore::new());
    let bus = Arc::new(MockBus::new());

    let trigger = make_node("trigger-1", "builtin:trigger-manual", NodeKind::Trigger);
    let action_a = make_node_with_compensate("action-a", "builtin:action-a", "builtin:compensate");
    let action_b = make_node("action-b", "builtin:action-b", NodeKind::Action);

    let wf = make_workflow(
        "wf-saga-basic",
        vec![trigger, action_a, action_b],
        vec![
            make_edge("e1", "trigger-1", "action-a"),
            make_edge("e2", "action-a", "action-b"),
        ],
    );

    let trig_exec = Arc::new(MockNodeExecutor::ok());
    let a_exec = Arc::new(MockNodeExecutor::ok());
    let b_exec = Arc::new(MockNodeExecutor::with_error(OrbflowError::Internal(
        "simulated downstream failure".into(),
    )));
    let comp_exec = Arc::new(MockNodeExecutor::ok());

    let engine = build_engine(
        Arc::clone(&store),
        Arc::clone(&bus),
        vec![
            ("builtin:trigger-manual", trig_exec as Arc<dyn NodeExecutor>),
            ("builtin:action-a", a_exec as Arc<dyn NodeExecutor>),
            ("builtin:action-b", b_exec as Arc<dyn NodeExecutor>),
            (
                "builtin:compensate",
                Arc::clone(&comp_exec) as Arc<dyn NodeExecutor>,
            ),
        ],
    )
    .await;

    engine.create_workflow(&wf).await.expect("create_workflow");
    engine
        .start_workflow(&WorkflowId::new("wf-saga-basic"), HashMap::new())
        .await
        .expect("start_workflow");

    let store_poll = Arc::clone(&store);
    let inst_id = wait_for(std::time::Duration::from_secs(5), || {
        let store_poll = Arc::clone(&store_poll);
        async move {
            let (instances, _) = store_poll
                .list_instances(ListOptions {
                    offset: 0,
                    limit: 0,
                })
                .await
                .ok()?;
            let inst = instances.first()?;
            let events = store_poll.load_events(&inst.id, 0).await.ok()?;
            let has_started = events
                .iter()
                .any(|e| matches!(e, DomainEvent::CompensationStarted(_)));
            let has_completed = events
                .iter()
                .any(|e| matches!(e, DomainEvent::CompensationCompleted(_)));
            if has_started && has_completed {
                Some(inst.id.clone())
            } else {
                None
            }
        }
    })
    .await
    .expect("saga compensation events within timeout");

    let events = store.load_events(&inst_id, 0).await.expect("load_events");
    let started_idx = events
        .iter()
        .position(|e| matches!(e, DomainEvent::CompensationStarted(_)))
        .expect("CompensationStarted present");
    let completed_idx = events
        .iter()
        .position(|e| matches!(e, DomainEvent::CompensationCompleted(_)))
        .expect("CompensationCompleted present");
    assert!(
        started_idx < completed_idx,
        "CompensationStarted must precede CompensationCompleted"
    );

    assert_eq!(
        comp_exec.call_count(),
        1,
        "compensate plugin should fire once for node A"
    );

    if let DomainEvent::CompensationStarted(e) = &events[started_idx] {
        assert_eq!(e.failed_node, "action-b");
    } else {
        panic!("expected CompensationStarted at index {started_idx}");
    }

    assert!(
        events
            .iter()
            .any(|e| e.event_type() == EventType::CompensationStarted)
    );
    assert!(
        events
            .iter()
            .any(|e| e.event_type() == EventType::CompensationCompleted)
    );
}

/// Saga: no compensate configs anywhere → workflow fails but
/// `CompensationCompleted` must never fire (guarded by the empty-list check
/// in saga.rs to prevent a false-positive completion).
#[tokio::test]
async fn saga_does_not_complete_when_no_nodes_have_compensate_config() {
    let store = Arc::new(MockStore::new());
    let bus = Arc::new(MockBus::new());

    let trigger = make_node("trigger-1", "builtin:trigger-manual", NodeKind::Trigger);
    let action_a = make_node("action-a", "builtin:action-a", NodeKind::Action);
    let action_b = make_node("action-b", "builtin:action-b", NodeKind::Action);

    let wf = make_workflow(
        "wf-saga-none",
        vec![trigger, action_a, action_b],
        vec![
            make_edge("e1", "trigger-1", "action-a"),
            make_edge("e2", "action-a", "action-b"),
        ],
    );

    let trig_exec = Arc::new(MockNodeExecutor::ok());
    let a_exec = Arc::new(MockNodeExecutor::ok());
    let b_exec = Arc::new(MockNodeExecutor::with_error(OrbflowError::Internal(
        "simulated downstream failure".into(),
    )));

    let engine = build_engine(
        Arc::clone(&store),
        Arc::clone(&bus),
        vec![
            ("builtin:trigger-manual", trig_exec as Arc<dyn NodeExecutor>),
            ("builtin:action-a", a_exec as Arc<dyn NodeExecutor>),
            ("builtin:action-b", b_exec as Arc<dyn NodeExecutor>),
        ],
    )
    .await;

    engine.create_workflow(&wf).await.expect("create_workflow");
    engine
        .start_workflow(&WorkflowId::new("wf-saga-none"), HashMap::new())
        .await
        .expect("start_workflow");

    use orbflow_core::execution::InstanceStatus;
    let store_poll = Arc::clone(&store);
    let failed_id = wait_for(std::time::Duration::from_secs(5), || {
        let store_poll = Arc::clone(&store_poll);
        async move {
            let (instances, _) = store_poll
                .list_instances(ListOptions {
                    offset: 0,
                    limit: 0,
                })
                .await
                .ok()?;
            let inst = instances.first()?;
            if inst.status == InstanceStatus::Failed {
                Some(inst.id.clone())
            } else {
                None
            }
        }
    })
    .await
    .expect("instance should reach Failed status");

    let events = store.load_events(&failed_id, 0).await.expect("load_events");
    let completed_count = events
        .iter()
        .filter(|e| matches!(e, DomainEvent::CompensationCompleted(_)))
        .count();
    assert_eq!(
        completed_count, 0,
        "CompensationCompleted must not fire when no compensate configs exist"
    );
}

#[tokio::test]
async fn saga_failed_compensation_marks_instance_terminal_failed() {
    let store = Arc::new(MockStore::new());
    let bus = Arc::new(MockBus::new());

    let trigger = make_node("trigger-1", "builtin:trigger-manual", NodeKind::Trigger);
    let action_a = make_node_with_compensate("action-a", "builtin:action-a", "builtin:compensate");
    let action_b = make_node("action-b", "builtin:action-b", NodeKind::Action);

    let wf = make_workflow(
        "wf-saga-comp-fails",
        vec![trigger, action_a, action_b],
        vec![
            make_edge("e1", "trigger-1", "action-a"),
            make_edge("e2", "action-a", "action-b"),
        ],
    );

    let trig_exec = Arc::new(MockNodeExecutor::ok());
    let a_exec = Arc::new(MockNodeExecutor::ok());
    let b_exec = Arc::new(MockNodeExecutor::with_error(OrbflowError::Internal(
        "simulated downstream failure".into(),
    )));
    let comp_exec = Arc::new(MockNodeExecutor::with_error(OrbflowError::Internal(
        "compensation failure".into(),
    )));

    let engine = build_engine(
        Arc::clone(&store),
        Arc::clone(&bus),
        vec![
            ("builtin:trigger-manual", trig_exec as Arc<dyn NodeExecutor>),
            ("builtin:action-a", a_exec as Arc<dyn NodeExecutor>),
            ("builtin:action-b", b_exec as Arc<dyn NodeExecutor>),
            (
                "builtin:compensate",
                Arc::clone(&comp_exec) as Arc<dyn NodeExecutor>,
            ),
        ],
    )
    .await;

    engine.create_workflow(&wf).await.expect("create_workflow");
    engine
        .start_workflow(&WorkflowId::new("wf-saga-comp-fails"), HashMap::new())
        .await
        .expect("start_workflow");

    use orbflow_core::execution::InstanceStatus;
    let store_poll = Arc::clone(&store);
    let failed_id = wait_for(std::time::Duration::from_secs(5), || {
        let store_poll = Arc::clone(&store_poll);
        async move {
            let (instances, _) = store_poll
                .list_instances(ListOptions {
                    offset: 0,
                    limit: 0,
                })
                .await
                .ok()?;
            let inst = instances.first()?;
            let saga_terminal = inst.saga.as_ref().is_some_and(|s| !s.compensating);
            if inst.status == InstanceStatus::Failed && saga_terminal {
                Some(inst.id.clone())
            } else {
                None
            }
        }
    })
    .await
    .expect("failed compensation should terminalize instance");

    let events = store.load_events(&failed_id, 0).await.expect("load_events");
    assert!(
        events
            .iter()
            .any(|e| matches!(e, DomainEvent::InstanceFailed(f) if f.error.contains("compensation for node action-a failed"))),
        "compensation failure should emit a terminal InstanceFailed event"
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, DomainEvent::CompensationCompleted(_))),
        "failed compensation must not emit CompensationCompleted"
    );
}

/// Bus wrapper that fails any publish of a compensation task (payload contains
/// `_compensate_`) while delegating everything else to the inner MockBus.
/// Simulates a bus outage at exactly the moment the saga dispatches compensation.
struct CompensationFailingBus {
    inner: Arc<MockBus>,
}

#[async_trait::async_trait]
impl Bus for CompensationFailingBus {
    async fn publish(&self, subject: &str, data: &[u8]) -> Result<(), OrbflowError> {
        if std::str::from_utf8(data).is_ok_and(|s| s.contains("_compensate_")) {
            return Err(OrbflowError::Internal(
                "simulated bus outage during compensation dispatch".into(),
            ));
        }
        self.inner.publish(subject, data).await
    }

    async fn subscribe(
        &self,
        subject: &str,
        handler: MsgHandler,
    ) -> Result<(), OrbflowError> {
        self.inner.subscribe(subject, handler).await
    }

    async fn close(&self) -> Result<(), OrbflowError> {
        self.inner.close().await
    }
}

/// Saga: compensation DISPATCH fails (bus error), exercising the
/// `dispatch_pending_compensations` error arm: the failure must be absorbed
/// (logged, node left pending for recovery) — the saga stays in `compensating`,
/// the node is never marked compensated, no CompensationCompleted fires, and
/// the compensation executor never runs.
#[tokio::test]
async fn saga_dispatch_failure_leaves_node_pending_for_recovery() {
    let store = Arc::new(MockStore::new());
    let mock_bus = Arc::new(MockBus::new());
    let failing_bus = Arc::new(CompensationFailingBus {
        inner: Arc::clone(&mock_bus),
    });

    let trigger = make_node("trigger-1", "builtin:trigger-manual", NodeKind::Trigger);
    let action_a = make_node_with_compensate("action-a", "builtin:action-a", "builtin:compensate");
    let action_b = make_node("action-b", "builtin:action-b", NodeKind::Action);

    let wf = make_workflow(
        "wf-saga-dispatch-fails",
        vec![trigger, action_a, action_b],
        vec![
            make_edge("e1", "trigger-1", "action-a"),
            make_edge("e2", "action-a", "action-b"),
        ],
    );

    let trig_exec = Arc::new(MockNodeExecutor::ok());
    let a_exec = Arc::new(MockNodeExecutor::ok());
    let b_exec = Arc::new(MockNodeExecutor::with_error(OrbflowError::Internal(
        "simulated downstream failure".into(),
    )));
    let comp_exec = Arc::new(MockNodeExecutor::ok());

    let opts = EngineOptionsBuilder::new()
        .store(Arc::clone(&store) as Arc<dyn orbflow_core::ports::Store>)
        .bus(Arc::clone(&failing_bus) as Arc<dyn Bus>)
        .pool_name("test")
        .enable_resume(false)
        .build()
        .expect("test engine options");
    let engine = Arc::new(OrbflowEngine::new(opts));

    let executors: Vec<(&str, Arc<dyn NodeExecutor>)> = vec![
        ("builtin:trigger-manual", trig_exec as Arc<dyn NodeExecutor>),
        ("builtin:action-a", a_exec as Arc<dyn NodeExecutor>),
        ("builtin:action-b", b_exec as Arc<dyn NodeExecutor>),
        (
            "builtin:compensate",
            Arc::clone(&comp_exec) as Arc<dyn NodeExecutor>,
        ),
    ];
    let executor_map: HashMap<String, Arc<dyn NodeExecutor>> = executors
        .iter()
        .map(|(name, exec)| ((*name).to_string(), Arc::clone(exec)))
        .collect();
    for (name, exec) in executors {
        engine.register_node(name, exec).expect("register_node");
    }

    let eng_for_handler = Arc::clone(&engine);
    let handler: MsgHandler = Arc::new(move |_subject, data| {
        let eng = Arc::clone(&eng_for_handler);
        let execs = executor_map.clone();
        Box::pin(async move {
            let task: TaskMessage =
                serde_json::from_slice(&data).map_err(|e| OrbflowError::Internal(e.to_string()))?;
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
                    attempt: task.attempt,
                    dispatch_id: task.dispatch_id,
                    output: out.data,
                    error: out.error,
                    trace_context: None,
                    v: 1,
                },
                Err(e) => ResultMessage {
                    result_id: None,
                    instance_id: task.instance_id,
                    node_id: task.node_id,
                    attempt: task.attempt,
                    dispatch_id: task.dispatch_id,
                    output: None,
                    error: Some(e.to_string()),
                    trace_context: None,
                    v: 1,
                },
            };
            tokio::spawn(async move {
                let _ = eng.handle_node_result(&result).await;
            });
            Ok(())
        })
    });
    failing_bus
        .subscribe(&task_subject("test"), handler)
        .await
        .expect("subscribe");

    engine.create_workflow(&wf).await.expect("create_workflow");
    engine
        .start_workflow(&WorkflowId::new("wf-saga-dispatch-fails"), HashMap::new())
        .await
        .expect("start_workflow");

    // Wait until the saga enters compensating state (CompensationStarted recorded,
    // dispatch attempted and failed via the failing bus).
    let store_poll = Arc::clone(&store);
    let inst_id = wait_for(std::time::Duration::from_secs(5), || {
        let store_poll = Arc::clone(&store_poll);
        async move {
            let (instances, _) = store_poll
                .list_instances(ListOptions {
                    offset: 0,
                    limit: 0,
                })
                .await
                .ok()?;
            let inst = instances.first()?;
            if inst.saga.as_ref().is_some_and(|s| s.compensating) {
                Some(inst.id.clone())
            } else {
                None
            }
        }
    })
    .await
    .expect("saga should enter compensating state despite dispatch failure");

    // Allow any stray async work to settle, then assert the absorbed-failure
    // contract: still compensating, node still pending, no completion, no
    // compensation execution.
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let inst = store
        .get_instance(&inst_id)
        .await
        .expect("get_instance");
    let saga = inst.saga.as_ref().expect("saga state present");
    assert!(
        saga.compensating,
        "saga must remain in compensating state when dispatch fails (pending recovery)"
    );
    assert!(
        !saga.compensated_nodes.contains(&"action-a".to_string()),
        "action-a must not be marked compensated when its dispatch never reached the bus"
    );

    let events = store.load_events(&inst_id, 0).await.expect("load_events");
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, DomainEvent::CompensationCompleted(_))),
        "CompensationCompleted must not fire when the compensation task was never dispatched"
    );
    assert_eq!(
        comp_exec.call_count(),
        0,
        "compensation executor must never run when the bus publish failed"
    );
}
