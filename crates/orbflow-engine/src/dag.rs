// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Multi-pass DAG evaluator with OR-join skip semantics and CEL edge condition
//! evaluation. Determines which nodes are ready to execute based on predecessor
//! states and edge conditions.

use std::collections::HashMap;

use orbflow_cel::CelEvaluator;
use orbflow_core::execution::{Instance, NodeStatus};
use orbflow_core::workflow::Workflow;

/// Represents an incoming edge to a node.
struct IncomingEdge<'a> {
    source_id: &'a str,
    condition: Option<&'a str>,
}

/// Returns node IDs that are pending and whose predecessors are all terminal,
/// with at least one incoming edge providing a live (completed + condition-true)
/// path.
///
/// # Skip semantics (OR-join)
///
/// A node is skipped only when ALL incoming paths are dead (predecessor
/// skipped/failed/cancelled, or edge condition is false). If at least one
/// incoming edge leads from a completed predecessor with a true (or absent)
/// condition, the node is ready to execute.
///
/// Multiple passes cascade skips through the DAG so that downstream nodes
/// of skipped branches are also skipped.
pub(crate) async fn find_ready_nodes(
    wf: &Workflow,
    inst: &mut Instance,
    cel: &CelEvaluator,
) -> Vec<String> {
    // Build incoming edges per node.
    let mut incoming: HashMap<&str, Vec<IncomingEdge<'_>>> = HashMap::with_capacity(wf.nodes.len());
    for n in &wf.nodes {
        incoming.entry(n.id.as_str()).or_default();
    }
    for e in &wf.edges {
        incoming
            .entry(e.target.as_str())
            .or_default()
            .push(IncomingEdge {
                source_id: e.source.as_str(),
                condition: e.condition.as_deref(),
            });
    }

    // Multi-pass skip cascade: mark nodes as skipped when all predecessors are
    // terminal and no incoming edge provides a live path.
    let mut changed = true;
    while changed {
        changed = false;
        for n in &wf.nodes {
            let status = inst.node_states.get(&n.id).map(|ns| ns.status);
            if status != Some(NodeStatus::Pending) {
                continue;
            }

            let edges = match incoming.get(n.id.as_str()) {
                Some(e) => e,
                None => continue,
            };
            if edges.is_empty() {
                // Entry nodes with pending status should not be skipped here.
                continue;
            }

            let mut all_pred_terminal = true;
            let mut has_live_path = false;

            for ie in edges {
                let pred_status = inst.node_states.get(ie.source_id).map(|ns| ns.status);

                match pred_status {
                    Some(NodeStatus::Completed) => {
                        if eval_edge_condition(ie, &n.id, inst, cel).await {
                            has_live_path = true;
                        }
                    }
                    Some(NodeStatus::Skipped)
                    | Some(NodeStatus::Failed)
                    | Some(NodeStatus::Cancelled) => {
                        // Dead path — predecessor is terminal but not completed.
                    }
                    _ => {
                        // Still running, queued, or pending.
                        all_pred_terminal = false;
                    }
                }
            }

            if all_pred_terminal
                && !has_live_path
                && let Some(ns) = inst.node_states.get_mut(&n.id)
            {
                ns.status = NodeStatus::Skipped;
                changed = true;
            }
        }
    }

    // Second pass: find nodes that are ready to execute.
    let mut ready = Vec::new();
    for n in &wf.nodes {
        let status = inst.node_states.get(&n.id).map(|ns| ns.status);
        if status != Some(NodeStatus::Pending) {
            continue;
        }

        let edges = match incoming.get(n.id.as_str()) {
            Some(e) => e,
            None => continue,
        };

        let mut all_done = true;
        let mut has_valid_path = false;

        for ie in edges {
            let pred_status = inst.node_states.get(ie.source_id).map(|ns| ns.status);

            match pred_status {
                Some(NodeStatus::Completed) => {
                    if eval_edge_condition(ie, &n.id, inst, cel).await {
                        has_valid_path = true;
                    }
                }
                Some(NodeStatus::Skipped)
                | Some(NodeStatus::Failed)
                | Some(NodeStatus::Cancelled) => {
                    // Dead path — continue checking other edges.
                }
                _ => {
                    all_done = false;
                }
            }
        }

        if all_done && has_valid_path {
            ready.push(n.id.clone());
        }
    }

    ready
}

/// Evaluates a CEL condition on an edge. Returns `true` if the condition is
/// absent or evaluates to true; `false` otherwise.
async fn eval_edge_condition(
    ie: &IncomingEdge<'_>,
    _node_id: &str,
    inst: &Instance,
    cel: &CelEvaluator,
) -> bool {
    let condition = match ie.condition {
        Some(c) if !c.is_empty() => c,
        _ => return true,
    };

    // Build the CEL context: expose `node` as the source node's output,
    // `nodes` as all node outputs, and `vars` as workflow variables.
    let mut ctx = orbflow_cel::evaluator::build_edge_context(&inst.context.node_outputs);

    // Add the source node's output as `node` for backward-compatible expressions
    // like `node.status == "ok"`.
    if let Some(source_output) = inst.context.node_outputs.get(ie.source_id) {
        ctx.insert(
            "node".into(),
            serde_json::Value::Object(
                source_output
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
            ),
        );
    }

    // Add variables.
    ctx.insert(
        "vars".into(),
        serde_json::Value::Object(
            inst.context
                .variables
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        ),
    );

    // Use async variant to avoid blocking the Tokio runtime on pathological
    // CEL expressions.
    match cel.eval_bool_async(condition, &ctx).await {
        Ok(result) => result,
        Err(e) => {
            tracing::warn!(
                instance = %inst.id,
                node = _node_id,
                condition = condition,
                error = %e,
                "CEL edge condition evaluation failed"
            );
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use orbflow_core::execution::*;
    use orbflow_core::workflow::*;
    use std::collections::HashMap;

    fn make_node(id: &str) -> Node {
        Node {
            id: id.into(),
            name: id.into(),
            kind: NodeKind::Action,
            node_type: NodeType::Builtin,
            plugin_ref: "builtin:log".into(),
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
        }
    }

    fn make_edge(id: &str, src: &str, tgt: &str) -> Edge {
        Edge {
            id: id.into(),
            source: src.into(),
            target: tgt.into(),
            condition: None,
        }
    }

    fn make_edge_cond(id: &str, src: &str, tgt: &str, cond: &str) -> Edge {
        Edge {
            id: id.into(),
            source: src.into(),
            target: tgt.into(),
            condition: Some(cond.into()),
        }
    }

    fn make_ns(id: &str, status: NodeStatus) -> (String, NodeState) {
        (
            id.into(),
            NodeState {
                node_id: id.into(),
                status,
                input: None,
                output: None,
                parameters: None,
                error: None,
                attempt: 0,
                started_at: None,
                ended_at: None,
            },
        )
    }

    fn make_inst(states: Vec<(String, NodeState)>) -> Instance {
        Instance {
            id: InstanceId::new("inst-1"),
            workflow_id: WorkflowId::new("wf-1"),
            status: InstanceStatus::Running,
            node_states: states.into_iter().collect(),
            context: ExecutionContext::new(HashMap::new()),
            saga: None,
            parent_id: None,
            instance_metrics: None,
            workflow_version: None,
            owner_id: None,
            version: 1,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_linear_ready() {
        let wf = Workflow {
            id: WorkflowId::new("wf-1"),
            name: "test".into(),
            description: None,
            version: 1,
            status: DefinitionStatus::Active,
            nodes: vec![make_node("a"), make_node("b")],
            edges: vec![make_edge("e1", "a", "b")],
            capability_edges: vec![],
            triggers: vec![],
            annotations: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let mut inst = make_inst(vec![
            make_ns("a", NodeStatus::Completed),
            make_ns("b", NodeStatus::Pending),
        ]);
        inst.context.node_outputs.insert("a".into(), HashMap::new());

        let cel = CelEvaluator::new();
        let ready = find_ready_nodes(&wf, &mut inst, &cel).await;
        assert_eq!(ready, vec!["b"]);
    }

    #[tokio::test]
    async fn test_join_not_ready_until_all_preds_done() {
        let wf = Workflow {
            id: WorkflowId::new("wf-1"),
            name: "test".into(),
            description: None,
            version: 1,
            status: DefinitionStatus::Active,
            nodes: vec![make_node("a"), make_node("b"), make_node("c")],
            edges: vec![make_edge("e1", "a", "c"), make_edge("e2", "b", "c")],
            capability_edges: vec![],
            triggers: vec![],
            annotations: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let mut inst = make_inst(vec![
            make_ns("a", NodeStatus::Completed),
            make_ns("b", NodeStatus::Pending),
            make_ns("c", NodeStatus::Pending),
        ]);
        inst.context.node_outputs.insert("a".into(), HashMap::new());

        let cel = CelEvaluator::new();
        let ready = find_ready_nodes(&wf, &mut inst, &cel).await;
        assert!(ready.is_empty());
    }

    #[tokio::test]
    async fn test_skip_cascade_all_false() {
        let wf = Workflow {
            id: WorkflowId::new("wf-1"),
            name: "test".into(),
            description: None,
            version: 1,
            status: DefinitionStatus::Active,
            nodes: vec![
                make_node("t"),
                make_node("a"),
                make_node("b"),
                make_node("d"),
            ],
            edges: vec![
                make_edge_cond("e1", "t", "a", "false"),
                make_edge_cond("e2", "t", "b", "false"),
                make_edge("e3", "a", "d"),
                make_edge("e4", "b", "d"),
            ],
            capability_edges: vec![],
            triggers: vec![],
            annotations: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let mut inst = make_inst(vec![
            make_ns("t", NodeStatus::Completed),
            make_ns("a", NodeStatus::Pending),
            make_ns("b", NodeStatus::Pending),
            make_ns("d", NodeStatus::Pending),
        ]);
        inst.context.node_outputs.insert("t".into(), HashMap::new());

        let cel = CelEvaluator::new();
        let ready = find_ready_nodes(&wf, &mut inst, &cel).await;
        assert!(ready.is_empty());
        assert_eq!(inst.node_states["a"].status, NodeStatus::Skipped);
        assert_eq!(inst.node_states["b"].status, NodeStatus::Skipped);
        assert_eq!(inst.node_states["d"].status, NodeStatus::Skipped);
    }

    #[tokio::test]
    async fn test_mixed_conditions_or_join() {
        let wf = Workflow {
            id: WorkflowId::new("wf-1"),
            name: "test".into(),
            description: None,
            version: 1,
            status: DefinitionStatus::Active,
            nodes: vec![
                make_node("t"),
                make_node("a"),
                make_node("b"),
                make_node("d"),
            ],
            edges: vec![
                make_edge_cond("e1", "t", "a", "true"),
                make_edge_cond("e2", "t", "b", "false"),
                make_edge("e3", "a", "d"),
                make_edge("e4", "b", "d"),
            ],
            capability_edges: vec![],
            triggers: vec![],
            annotations: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let mut inst = make_inst(vec![
            make_ns("t", NodeStatus::Completed),
            make_ns("a", NodeStatus::Completed),
            make_ns("b", NodeStatus::Pending),
            make_ns("d", NodeStatus::Pending),
        ]);
        inst.context.node_outputs.insert("t".into(), HashMap::new());
        inst.context.node_outputs.insert("a".into(), HashMap::new());

        let cel = CelEvaluator::new();
        let ready = find_ready_nodes(&wf, &mut inst, &cel).await;
        // b should be skipped (false condition), d should be ready via a
        assert_eq!(inst.node_states["b"].status, NodeStatus::Skipped);
        assert_eq!(ready, vec!["d"]);
    }
}
