// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Test-node execution: runs a single node (and its uncached ancestors) inline
//! without persistence or bus publishing. Used by the frontend to discover a
//! node's runtime output structure.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use tracing::warn;

use orbflow_core::OrbflowError;
use orbflow_core::execution::*;
use orbflow_core::ports::*;
use orbflow_core::workflow::*;

use crate::engine::{OrbflowEngine, resolve_capability_edges_from_context};

/// Maximum duration for executing a single node during a test-node request.
const TEST_NODE_TIMEOUT: Duration = Duration::from_secs(30);
const CREDENTIAL_ID_KEY: &str = "credential_id";

/// Executes a single node (and any uncached ancestors) inline without creating
/// a persisted Instance or publishing to the bus.
pub(crate) async fn test_node(
    engine: &OrbflowEngine,
    workflow_id: &WorkflowId,
    node_id: &str,
    cached_outputs: HashMap<String, HashMap<String, serde_json::Value>>,
    owner_id: Option<&str>,
) -> Result<TestNodeResult, OrbflowError> {
    let wf = engine.store().get_workflow(workflow_id).await?;

    let target_node = wf.node_by_id(node_id).ok_or_else(|| {
        OrbflowError::Internal(format!("test node: node {node_id:?} not found in workflow"))
    })?;
    let target_node = target_node.clone();

    // Build a synthetic execution context populated with cached outputs.
    let mut ec = ExecutionContext::new(HashMap::new());
    for (nid, out) in &cached_outputs {
        ec.node_outputs.insert(nid.clone(), out.clone());
    }

    // Get ancestor IDs in topological order (leaves first).
    let ancestor_ids = wf.ancestors_of(node_id);
    let mut result = TestNodeResult {
        node_outputs: HashMap::new(),
        target_node: node_id.to_owned(),
        warnings: Vec::new(),
    };

    // Execute ancestors that don't have cached outputs.
    for aid in &ancestor_ids {
        if ec.node_outputs.contains_key(aid.as_str()) {
            continue;
        }

        let ancestor_node = match wf.node_by_id(aid) {
            Some(n) => n.clone(),
            None => continue,
        };

        match execute_node_inline(engine, &ancestor_node, &ec, &wf, owner_id).await {
            Ok(ns) => {
                if let Some(ref out) = ns.output {
                    ec.node_outputs.insert(aid.clone(), out.clone());
                }
                result.node_outputs.insert(aid.clone(), ns);
            }
            Err(e) => {
                let ns = NodeState {
                    node_id: aid.clone(),
                    status: NodeStatus::Failed,
                    input: None,
                    output: None,
                    parameters: None,
                    error: Some(e.to_string()),
                    attempt: 0,
                    started_at: None,
                    ended_at: None,
                };
                result.node_outputs.insert(aid.clone(), ns);
                result
                    .warnings
                    .push(format!("ancestor node {aid:?} failed: {e}"));
                return Ok(result);
            }
        }
    }

    // Execute the target node.
    match execute_node_inline(engine, &target_node, &ec, &wf, owner_id).await {
        Ok(ns) => {
            result.node_outputs.insert(node_id.to_owned(), ns);
        }
        Err(e) => {
            let ns = NodeState {
                node_id: node_id.to_owned(),
                status: NodeStatus::Failed,
                input: None,
                output: None,
                parameters: None,
                error: Some(e.to_string()),
                attempt: 0,
                started_at: None,
                ended_at: None,
            };
            result.node_outputs.insert(node_id.to_owned(), ns);
        }
    }

    Ok(result)
}

/// Runs a single node directly (no bus, no Instance). Follows the same
/// pattern as resolve_capabilities.
async fn execute_node_inline(
    engine: &OrbflowEngine,
    node: &Node,
    ec: &ExecutionContext,
    wf: &Workflow,
    owner_id: Option<&str>,
) -> Result<NodeState, OrbflowError> {
    // For trigger nodes, return empty output.
    if node.kind == NodeKind::Trigger {
        let now = Utc::now();
        return Ok(NodeState {
            node_id: node.id.clone(),
            status: NodeStatus::Completed,
            input: None,
            output: Some(HashMap::new()),
            parameters: None,
            error: None,
            attempt: 1,
            started_at: Some(now),
            ended_at: Some(now),
        });
    }

    let executor: Arc<dyn NodeExecutor> = engine
        .get_executor(&node.plugin_ref)?
        .ok_or(OrbflowError::NodeNotFound)?;

    // Resolve input mapping.
    let resolved_input = engine
        .resolve_input_mapping(node.input_mapping.as_ref(), ec)
        .await?;

    // Resolve parameters.
    let mut resolved_params = if !node.parameters.is_empty() {
        Some(engine.resolve_parameters(&node.parameters, ec).await)
    } else {
        None
    };

    // Resolve credential references the same way normal task dispatch does so
    // test-node execution behaves like a real run.
    let resolved_creds = engine
        .resolve_credentials_deduped(node.config.as_ref(), resolved_params.as_ref(), owner_id)
        .await?;

    let mut resolved_config = node.config.clone();
    if let Some(config) = resolved_config.as_mut()
        && let Some(serde_json::Value::String(cred_id)) = config.remove(CREDENTIAL_ID_KEY)
        && let Some((_cred_type, _tier, cred_data)) = resolved_creds.get(&cred_id)
    {
        for (k, v) in cred_data {
            config.entry(k.clone()).or_insert_with(|| v.clone());
        }
    }
    if let Some(params) = resolved_params.as_mut()
        && let Some(serde_json::Value::String(cred_id)) = params.remove(CREDENTIAL_ID_KEY)
        && let Some((_cred_type, _tier, cred_data)) = resolved_creds.get(&cred_id)
    {
        for (k, v) in cred_data {
            params.entry(k.clone()).or_insert_with(|| v.clone());
        }
    }

    // Resolve capability edges.
    let caps = if !node.capability_ports.is_empty() {
        resolve_capability_edges_from_context(ec, wf, node)
    } else {
        None
    };

    let input = NodeInput {
        instance_id: InstanceId::new(""),
        node_id: node.id.clone(),
        plugin_ref: node.plugin_ref.clone(),
        config: resolved_config,
        input: Some(resolved_input),
        parameters: resolved_params,
        capabilities: caps,
        attempt: 1,
    };

    let started_at = Utc::now();
    let output = tokio::time::timeout(TEST_NODE_TIMEOUT, executor.execute(&input))
        .await
        .map_err(|_| OrbflowError::Timeout)?
        .map_err(|e| {
            warn!(
                node = node.id.as_str(),
                plugin = node.plugin_ref.as_str(),
                error = %e,
                "test-node execution failed"
            );
            e
        })?;
    let ended_at = Utc::now();

    if let Some(ref err) = output.error {
        return Err(OrbflowError::Internal(format!("node error: {err}")));
    }

    Ok(NodeState {
        node_id: node.id.clone(),
        status: NodeStatus::Completed,
        input: None,
        output: output.data,
        parameters: None,
        error: None,
        attempt: 1,
        started_at: Some(started_at),
        ended_at: Some(ended_at),
    })
}
