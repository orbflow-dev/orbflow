// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Crash recovery: resumes running instances by resetting Queued/Running nodes
//! back to Pending and re-dispatching ready nodes.

use tracing::{error, info};

use orbflow_core::OrbflowError;
use orbflow_core::execution::NodeStatus;

use crate::dag::find_ready_nodes;
use crate::engine::OrbflowEngine;

/// Lists all running instances, resets Queued/Running nodes back to Pending,
/// and re-dispatches ready nodes. Called during engine startup for crash
/// recovery.
pub(crate) async fn resume_running(engine: &OrbflowEngine) -> Result<(), OrbflowError> {
    let instances = engine.store().list_running_instances().await?;

    if instances.is_empty() {
        return Ok(());
    }

    info!(count = instances.len(), "resuming running instances");

    for inst_summary in instances {
        // Acquire per-instance mutex before re-fetching.
        let mu = engine.lock_instance(&inst_summary.id);
        let _guard = mu.lock().await;

        // Re-fetch the instance while holding the lock.
        let mut inst = match engine.store().get_instance(&inst_summary.id).await {
            Ok(i) => i,
            Err(e) => {
                error!(
                    instance = %inst_summary.id,
                    error = %e,
                    "failed to re-fetch instance for resume"
                );
                continue;
            }
        };

        // Instance may have become terminal between listing and re-fetch.
        if inst.is_terminal() {
            continue;
        }

        let wf = match engine.store().get_workflow(&inst.workflow_id).await {
            Ok(w) => w,
            Err(e) => {
                error!(
                    instance = %inst.id,
                    error = %e,
                    "failed to load workflow for resume"
                );
                continue;
            }
        };

        // Reset Queued/Running nodes back to Pending.
        for (node_id, ns) in inst.node_states.iter_mut() {
            if ns.status == NodeStatus::Queued || ns.status == NodeStatus::Running {
                let node = wf.node_by_id(node_id);
                if node.is_none() {
                    continue;
                }
                ns.attempt += 1; // count the crash as a failed attempt
                ns.status = NodeStatus::Pending;
                info!(
                    instance = %inst.id,
                    node = node_id.as_str(),
                    "re-dispatching node"
                );
            }
        }

        if let Err(e) = engine.save_instance(&mut inst).await {
            error!(
                instance = %inst.id,
                error = %e,
                "failed to persist reset node states"
            );
            continue;
        }

        // Find and dispatch ready nodes.
        let ready_nodes = find_ready_nodes(&wf, &mut inst, engine.cel()).await;
        for node_id in &ready_nodes {
            if let Some(node) = wf.node_by_id(node_id) {
                let node = node.clone();
                if let Err(e) = engine.dispatch_node(&mut inst, &wf, &node).await {
                    error!(
                        instance = %inst.id,
                        node = node_id.as_str(),
                        error = %e,
                        "failed to re-dispatch node"
                    );
                }
            }
        }

        // Persist the dispatched (Queued) state.
        if !ready_nodes.is_empty()
            && let Err(e) = engine.save_instance(&mut inst).await
        {
            error!(
                instance = %inst.id,
                error = %e,
                "failed to persist dispatched node states"
            );
        }
    }

    Ok(())
}
