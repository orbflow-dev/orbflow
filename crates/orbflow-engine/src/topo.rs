// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! BFS-based topological sort (Kahn's algorithm) for saga compensation ordering.

use std::collections::HashMap;
use std::collections::VecDeque;

use orbflow_core::workflow::Workflow;

/// Returns node IDs in topological order using Kahn's algorithm (BFS from
/// entry nodes). Used by saga compensation to walk completed nodes in reverse.
pub(crate) fn topological_order(wf: &Workflow) -> Vec<String> {
    let mut in_degree: HashMap<&str, usize> = HashMap::with_capacity(wf.nodes.len());
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::with_capacity(wf.nodes.len());

    for n in &wf.nodes {
        in_degree.entry(n.id.as_str()).or_insert(0);
    }
    for e in &wf.edges {
        adj.entry(e.source.as_str())
            .or_default()
            .push(e.target.as_str());
        *in_degree.entry(e.target.as_str()).or_insert(0) += 1;
    }

    let mut queue: VecDeque<&str> = VecDeque::new();
    for n in &wf.nodes {
        if in_degree.get(n.id.as_str()).copied().unwrap_or(0) == 0 {
            queue.push_back(n.id.as_str());
        }
    }

    let mut order = Vec::with_capacity(wf.nodes.len());
    while let Some(cur) = queue.pop_front() {
        order.push(cur.to_owned());
        if let Some(neighbors) = adj.get(cur) {
            for &next in neighbors {
                if let Some(deg) = in_degree.get_mut(next) {
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push_back(next);
                    }
                }
            }
        }
    }

    order
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use orbflow_core::workflow::*;

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

    #[test]
    fn test_linear_topo_order() {
        let wf = Workflow {
            id: WorkflowId::new("wf-1"),
            name: "test".into(),
            description: None,
            version: 1,
            status: DefinitionStatus::Active,
            nodes: vec![make_node("a"), make_node("b"), make_node("c")],
            edges: vec![make_edge("e1", "a", "b"), make_edge("e2", "b", "c")],
            capability_edges: vec![],
            triggers: vec![],
            annotations: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let order = topological_order(&wf);
        assert_eq!(order, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_diamond_topo_order() {
        let wf = Workflow {
            id: WorkflowId::new("wf-1"),
            name: "test".into(),
            description: None,
            version: 1,
            status: DefinitionStatus::Active,
            nodes: vec![
                make_node("a"),
                make_node("b"),
                make_node("c"),
                make_node("d"),
            ],
            edges: vec![
                make_edge("e1", "a", "b"),
                make_edge("e2", "a", "c"),
                make_edge("e3", "b", "d"),
                make_edge("e4", "c", "d"),
            ],
            capability_edges: vec![],
            triggers: vec![],
            annotations: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let order = topological_order(&wf);
        // a must come first, d must come last. b and c can be in either order.
        assert_eq!(order[0], "a");
        assert_eq!(order[3], "d");
        assert!(order.contains(&"b".to_owned()));
        assert!(order.contains(&"c".to_owned()));
    }
}
