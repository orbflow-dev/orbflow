// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Saga compensation: walks completed nodes in reverse topological order and
//! dispatches compensation tasks when a workflow fails.

use chrono::Utc;
use tracing::{error, info};

use orbflow_core::event::*;
use orbflow_core::execution::*;
use orbflow_core::wire::{TaskMessage, WIRE_VERSION};
use orbflow_core::workflow::Workflow;
use orbflow_core::{OrbflowError, task_subject};

use crate::engine::OrbflowEngine;
use crate::topo::topological_order;

/// Initiates saga rollback by walking completed nodes in reverse topological
/// order and dispatching their compensation actions.
pub(crate) async fn start_compensation(
    engine: &OrbflowEngine,
    inst: &mut Instance,
    wf: &Workflow,
    failed_node_id: &str,
) -> Result<(), OrbflowError> {
    inst.saga = Some(SagaState {
        compensating: true,
        failed_node: Some(failed_node_id.to_owned()),
        completed_nodes: Vec::new(),
        compensated_nodes: Vec::new(),
    });

    // Collect completed nodes that have compensation configs, in topo order.
    let execution_order = topological_order(wf);
    let mut to_compensate = Vec::new();
    for node_id in &execution_order {
        let ns = match inst.node_states.get(node_id.as_str()) {
            Some(ns) => ns,
            None => continue,
        };
        if ns.status != NodeStatus::Completed {
            continue;
        }
        let node = match wf.node_by_id(node_id) {
            Some(n) => n,
            None => continue,
        };
        if node.compensate.is_none() {
            continue;
        }
        to_compensate.push(node_id.clone());
    }

    // Reverse for compensation (last completed first).
    to_compensate.reverse();

    // Track only the nodes we actually dispatch, so the completion check
    // in handle_compensation_result matches dispatched vs completed counts.
    let mut dispatched_nodes = Vec::with_capacity(to_compensate.len());

    if let Err(e) = engine
        .store()
        .append_event(DomainEvent::CompensationStarted(CompensationStartedEvent {
            base: BaseEvent::new(inst.id.clone(), inst.version),
            failed_node: failed_node_id.to_owned(),
        }))
        .await
    {
        error!(error = %e, instance = %inst.id, "failed to persist CompensationStarted event");
    }

    info!(
        instance = %inst.id,
        failed_node = failed_node_id,
        nodes_to_compensate = to_compensate.len(),
        "saga: starting compensation"
    );

    // Dispatch compensation tasks — continue on individual failures so the
    // saga can still terminate when transient errors affect only one node.
    for node_id in &to_compensate {
        if let Err(e) = dispatch_compensation(engine, inst, wf, node_id).await {
            error!(
                node = node_id.as_str(),
                error = %e,
                "saga: compensation dispatch failed, skipping node"
            );
            continue;
        }
        dispatched_nodes.push(node_id.clone());
    }

    // Only track nodes that were actually dispatched so completion check
    // in handle_compensation_result will fire correctly.
    if let Some(ref mut saga) = inst.saga {
        saga.completed_nodes = dispatched_nodes;
    }

    engine.save_instance(inst).await
}

/// Sends a compensation task for a completed node.
async fn dispatch_compensation(
    engine: &OrbflowEngine,
    inst: &Instance,
    wf: &Workflow,
    node_id: &str,
) -> Result<(), OrbflowError> {
    let node = match wf.node_by_id(node_id) {
        Some(n) => n,
        None => return Ok(()),
    };
    let compensate = match &node.compensate {
        Some(c) => c,
        None => return Ok(()),
    };

    let input = engine
        .resolve_input_mapping(compensate.input_mapping.as_ref(), &inst.context)
        .await?;

    let task = TaskMessage {
        instance_id: inst.id.clone(),
        node_id: format!("_compensate_{node_id}"),
        plugin_ref: compensate.plugin_ref.clone(),
        config: None,
        input: Some(input),
        parameters: None,
        capabilities: None,
        attempt: 1,
        trace_context: None,
        v: WIRE_VERSION,
    };

    let data = serde_json::to_vec(&task)
        .map_err(|e| OrbflowError::Internal(format!("marshal compensation task: {e}")))?;

    engine
        .bus()
        .publish(&task_subject(engine.pool_name()), &data)
        .await
}

/// Processes a compensation task result. Tracks which nodes have been
/// compensated and emits a completion event when all are done.
pub(crate) async fn handle_compensation_result(
    engine: &OrbflowEngine,
    inst: &mut Instance,
    node_id: &str,
) -> Result<(), OrbflowError> {
    let saga = match &mut inst.saga {
        Some(s) => s,
        None => return Ok(()),
    };

    // Strip the _compensate_ prefix to get original node ID.
    let orig_node_id = node_id
        .strip_prefix("_compensate_")
        .unwrap_or(node_id)
        .to_owned();

    if !saga.compensated_nodes.contains(&orig_node_id) {
        saga.compensated_nodes.push(orig_node_id);
    }
    inst.updated_at = Utc::now();

    // Check if all compensations are done.
    // Guard against empty saga (no nodes had compensation configs) triggering
    // a false-positive completion on stale results.
    let all_done = !saga.completed_nodes.is_empty()
        && saga.compensated_nodes.len() == saga.completed_nodes.len()
        && saga
            .compensated_nodes
            .iter()
            .all(|n| saga.completed_nodes.contains(n));

    if all_done {
        let compensated = saga.compensated_nodes.clone();
        if let Err(e) = engine
            .store()
            .append_event(DomainEvent::CompensationCompleted(
                CompensationCompletedEvent {
                    base: BaseEvent::new(inst.id.clone(), inst.version),
                },
            ))
            .await
        {
            error!(error = %e, instance = %inst.id, "failed to persist CompensationCompleted event");
        }
        info!(
            instance = %inst.id,
            compensated = compensated.len(),
            "saga: compensation completed"
        );
    }

    engine.save_instance(inst).await
}
