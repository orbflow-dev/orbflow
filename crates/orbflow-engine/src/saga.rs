// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Saga compensation: walks completed nodes in reverse topological order and
//! dispatches compensation tasks when a workflow fails.

use chrono::Utc;
use tracing::{error, info, warn};

use orbflow_core::event::*;
use orbflow_core::execution::*;
use orbflow_core::wire::{ResultMessage, TaskMessage, WIRE_VERSION, dispatch_identity};
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

    inst.status = InstanceStatus::Running;
    inst.saga = Some(SagaState {
        compensating: true,
        failed_node: Some(failed_node_id.to_owned()),
        completed_nodes: to_compensate.clone(),
        compensated_nodes: Vec::new(),
    });

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

    if to_compensate.is_empty() {
        finalize_compensation(engine, inst).await?;
        return engine.save_instance(inst).await;
    }

    // Persist the compensation plan before dispatching so crash recovery can
    // re-drive any compensation task that was not observed as successful.
    engine.save_instance(inst).await?;
    dispatch_pending_compensations(engine, inst, wf).await
}

/// Re-drives compensation tasks that are still incomplete after restart.
pub(crate) async fn resume_compensation(
    engine: &OrbflowEngine,
    inst: &mut Instance,
    wf: &Workflow,
) -> Result<(), OrbflowError> {
    dispatch_pending_compensations(engine, inst, wf).await?;
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
        node_id: compensation_task_node_id(node_id),
        dispatch_id: Some(dispatch_identity(
            &inst.id,
            &compensation_task_node_id(node_id),
            1,
        )),
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

async fn dispatch_pending_compensations(
    engine: &OrbflowEngine,
    inst: &mut Instance,
    wf: &Workflow,
) -> Result<(), OrbflowError> {
    let pending_nodes = match inst.saga.as_ref() {
        Some(saga) if saga.compensating => saga
            .completed_nodes
            .iter()
            .filter(|node_id| !saga.compensated_nodes.contains(node_id))
            .cloned()
            .collect::<Vec<_>>(),
        _ => return Ok(()),
    };

    if pending_nodes.is_empty() {
        finalize_compensation(engine, inst).await?;
        return Ok(());
    }

    for node_id in &pending_nodes {
        if let Err(e) = dispatch_compensation(engine, inst, wf, node_id).await {
            error!(
                node = node_id.as_str(),
                error = %e,
                "saga: compensation dispatch failed; leaving node pending for recovery"
            );
        }
    }

    Ok(())
}

fn compensation_task_node_id(node_id: &str) -> String {
    format!("_compensate_{node_id}")
}

/// Processes a compensation task result. Tracks which nodes have been
/// compensated and emits a completion event when all are done.
pub(crate) async fn handle_compensation_result(
    engine: &OrbflowEngine,
    inst: &mut Instance,
    result: &ResultMessage,
) -> Result<(), OrbflowError> {
    // Strip the _compensate_ prefix to get original node ID.
    let orig_node_id = result
        .node_id
        .strip_prefix("_compensate_")
        .unwrap_or(&result.node_id)
        .to_owned();

    if let Some(error_msg) = result.error.as_ref() {
        warn!(
            instance = %inst.id,
            compensation_node = result.node_id.as_str(),
            original_node = orig_node_id.as_str(),
            error = error_msg.as_str(),
            "saga: compensation task failed; marking compensation terminal"
        );
        fail_compensation(engine, inst, &orig_node_id, error_msg).await?;
        return engine.save_instance(inst).await;
    }

    let all_done: bool;
    {
        let saga = match &mut inst.saga {
            Some(s) if s.compensating => s,
            _ => {
                return Err(OrbflowError::Bus(format!(
                    "compensation result for node {} received without active saga",
                    result.node_id
                )));
            }
        };

        if !saga.completed_nodes.contains(&orig_node_id) {
            return Err(OrbflowError::Bus(format!(
                "compensation result for node {} was not dispatched",
                result.node_id
            )));
        }
        if saga.compensated_nodes.contains(&orig_node_id) {
            return Err(OrbflowError::Bus(format!(
                "duplicate compensation result for node {}",
                result.node_id
            )));
        }

        saga.compensated_nodes.push(orig_node_id);
        inst.updated_at = Utc::now();

        all_done = !saga.completed_nodes.is_empty()
            && saga.compensated_nodes.len() == saga.completed_nodes.len()
            && saga
                .compensated_nodes
                .iter()
                .all(|n| saga.completed_nodes.contains(n));
    }

    if all_done {
        finalize_compensation(engine, inst).await?;
    }

    engine.save_instance(inst).await
}

async fn finalize_compensation(
    engine: &OrbflowEngine,
    inst: &mut Instance,
) -> Result<(), OrbflowError> {
    let compensated = inst
        .saga
        .as_ref()
        .map(|s| s.compensated_nodes.len())
        .unwrap_or_default();
    let failed_node = inst
        .saga
        .as_ref()
        .and_then(|s| s.failed_node.clone())
        .unwrap_or_else(|| "unknown".to_owned());

    if let Some(saga) = inst.saga.as_mut() {
        saga.compensating = false;
    }
    inst.status = InstanceStatus::Failed;
    inst.updated_at = Utc::now();

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

    if let Err(e) = engine
        .store()
        .append_event(DomainEvent::InstanceFailed(InstanceFailedEvent {
            base: BaseEvent::new(inst.id.clone(), inst.version),
            error: format!("node {failed_node} failed; compensation completed"),
        }))
        .await
    {
        error!(error = %e, instance = %inst.id, "failed to persist InstanceFailed after compensation");
    }

    info!(
        instance = %inst.id,
        compensated = compensated,
        "saga: compensation completed"
    );

    Ok(())
}

async fn fail_compensation(
    engine: &OrbflowEngine,
    inst: &mut Instance,
    original_node_id: &str,
    error_msg: &str,
) -> Result<(), OrbflowError> {
    let failed_node = inst
        .saga
        .as_ref()
        .and_then(|s| s.failed_node.clone())
        .unwrap_or_else(|| "unknown".to_owned());

    if let Some(saga) = inst.saga.as_mut() {
        saga.compensating = false;
    }
    inst.status = InstanceStatus::Failed;
    inst.updated_at = Utc::now();

    if let Err(e) = engine
        .store()
        .append_event(DomainEvent::InstanceFailed(InstanceFailedEvent {
            base: BaseEvent::new(inst.id.clone(), inst.version),
            error: format!(
                "node {failed_node} failed; compensation for node {original_node_id} failed: {error_msg}"
            ),
        }))
        .await
    {
        error!(
            error = %e,
            instance = %inst.id,
            "failed to persist InstanceFailed after compensation failure"
        );
    }

    Ok(())
}
