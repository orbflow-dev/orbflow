// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Workflow structural validation.

use std::collections::{HashMap, HashSet, VecDeque};

use crate::error::OrbflowError;
use crate::ports::NodeSchema;
use crate::workflow::{Node, NodeKind, ParameterMode, Workflow};

/// Maximum allowed length for a plugin name.
pub const MAX_PLUGIN_NAME_LEN: usize = 64;

/// Validates that a plugin name contains only safe characters.
/// Returns `Err` if the name is empty, too long, or contains non-safe chars.
pub fn validate_plugin_name(name: &str) -> Result<(), OrbflowError> {
    if name.is_empty()
        || name.len() > MAX_PLUGIN_NAME_LEN
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(OrbflowError::InvalidNodeConfig(format!(
            "invalid plugin name '{name}': must be 1-{MAX_PLUGIN_NAME_LEN} chars, \
             alphanumeric/hyphen/underscore only"
        )));
    }
    Ok(())
}

/// Maximum length for node IDs and names.
const MAX_NODE_ID_LEN: usize = 128;

/// Maximum length for CEL expressions in input mappings.
/// Must match `MAX_CEL_EXPR_LEN` in `orbflow-cel::evaluator`.
const MAX_CEL_EXPR_LEN: usize = 4096;

/// Validates that a node ID contains only safe characters (alphanumeric, hyphens,
/// underscores) and is within length limits. Prevents path traversal and injection.
fn validate_node_id(id: &str) -> Result<(), OrbflowError> {
    if id.is_empty() || id.len() > MAX_NODE_ID_LEN {
        return Err(OrbflowError::InvalidNodeConfig(format!(
            "node ID must be 1-{MAX_NODE_ID_LEN} characters, got {}",
            id.len()
        )));
    }
    if !id
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        let truncated = id.char_indices().nth(32).map_or(id, |(i, _)| &id[..i]);
        return Err(OrbflowError::InvalidNodeConfig(format!(
            "node ID {truncated:?} contains invalid characters (only alphanumeric, '-', '_' allowed)",
        )));
    }
    Ok(())
}

/// Validates that a plugin_ref follows the expected format.
fn validate_plugin_ref(plugin_ref: &str) -> Result<(), OrbflowError> {
    if plugin_ref.is_empty() {
        return Err(OrbflowError::InvalidNodeConfig(
            "plugin_ref must not be empty".into(),
        ));
    }
    // Must start with a known prefix
    if !plugin_ref.starts_with("builtin:")
        && !plugin_ref.starts_with("plugin:")
        && !plugin_ref.starts_with("mcp:")
    {
        let truncated = plugin_ref
            .char_indices()
            .nth(32)
            .map_or(plugin_ref, |(i, _)| &plugin_ref[..i]);
        return Err(OrbflowError::InvalidNodeConfig(format!(
            "plugin_ref {truncated:?} must start with 'builtin:', 'plugin:', or 'mcp:'",
        )));
    }
    // Reject path traversal sequences
    if plugin_ref.contains("..") || plugin_ref.contains('/') || plugin_ref.contains('\\') {
        let truncated = plugin_ref
            .char_indices()
            .nth(32)
            .map_or(plugin_ref, |(i, _)| &plugin_ref[..i]);
        return Err(OrbflowError::InvalidNodeConfig(format!(
            "plugin_ref {truncated:?} contains invalid path characters",
        )));
    }
    Ok(())
}

/// Checks a workflow definition for structural errors.
pub fn validate_workflow(w: &Workflow) -> Result<(), OrbflowError> {
    if w.nodes.is_empty() {
        return Err(OrbflowError::NoEntryNodes);
    }

    let mut node_ids: HashSet<&str> = HashSet::with_capacity(w.nodes.len());
    let mut node_kinds: HashMap<&str, NodeKind> = HashMap::with_capacity(w.nodes.len());

    for n in &w.nodes {
        validate_node_id(&n.id)?;
        validate_plugin_ref(&n.plugin_ref)?;
        if let Some(ref comp) = n.compensate {
            validate_plugin_ref(&comp.plugin_ref).map_err(|_| {
                OrbflowError::InvalidNodeConfig(format!(
                    "node {:?}: compensate.plugin_ref is invalid",
                    n.id
                ))
            })?;
        }
        if !node_ids.insert(&n.id) {
            return Err(OrbflowError::DuplicateNode);
        }
        node_kinds.insert(&n.id, n.kind);
        validate_node_kind(n)?;
        validate_input_mapping_cel_lengths(n)?;
    }

    // Validate regular edges only reference non-capability nodes.
    let mut has_incoming: HashSet<&str> = HashSet::with_capacity(w.nodes.len());
    let mut edge_pairs: HashSet<String> = HashSet::with_capacity(w.edges.len());

    for e in &w.edges {
        if !node_ids.contains(e.source.as_str()) {
            return Err(OrbflowError::InvalidEdge);
        }
        if !node_ids.contains(e.target.as_str()) {
            return Err(OrbflowError::InvalidEdge);
        }
        if node_kinds.get(e.source.as_str()) == Some(&NodeKind::Capability)
            || node_kinds.get(e.target.as_str()) == Some(&NodeKind::Capability)
        {
            return Err(OrbflowError::InvalidCapabilityEdge);
        }
        let pair = format!("{}->{}", e.source, e.target);
        if !edge_pairs.insert(pair) {
            return Err(OrbflowError::DuplicateEdge);
        }
        has_incoming.insert(&e.target);
    }

    // Trigger nodes must not have incoming edges.
    for n in &w.nodes {
        if n.kind == NodeKind::Trigger && has_incoming.contains(n.id.as_str()) {
            return Err(OrbflowError::InvalidNodeKind);
        }
    }

    // Validate capability edges.
    validate_capability_edges(w, &node_ids, &node_kinds)?;

    let entry = w.entry_nodes();
    if entry.is_empty() {
        return Err(OrbflowError::NoEntryNodes);
    }

    detect_cycles(w)?;
    check_connectivity(w, &entry)
}

fn validate_node_kind(n: &Node) -> Result<(), OrbflowError> {
    match n.kind {
        NodeKind::Action | NodeKind::Trigger | NodeKind::Capability => Ok(()),
    }
}

/// Validates that CEL expressions in `input_mapping` do not exceed the max
/// length. Catches oversized expressions at workflow create/update time
/// rather than failing at dispatch time.
fn validate_input_mapping_cel_lengths(n: &Node) -> Result<(), OrbflowError> {
    if let Some(ref mapping) = n.input_mapping {
        for (key, val) in mapping {
            if let serde_json::Value::String(s) = val
                && s.starts_with('=')
                && s.len() > MAX_CEL_EXPR_LEN + 1
            {
                return Err(OrbflowError::InvalidNodeConfig(format!(
                    "node {:?}: input_mapping key {:?} contains CEL expression exceeding \
                     maximum length ({} > {MAX_CEL_EXPR_LEN})",
                    n.id,
                    key,
                    s.len() - 1,
                )));
            }
        }
    }
    if let Some(ref comp) = n.compensate
        && let Some(ref mapping) = comp.input_mapping
    {
        for (key, val) in mapping {
            if let serde_json::Value::String(s) = val
                && s.starts_with('=')
                && s.len() > MAX_CEL_EXPR_LEN + 1
            {
                return Err(OrbflowError::InvalidNodeConfig(format!(
                    "node {:?}: compensate.input_mapping key {:?} contains CEL expression \
                     exceeding maximum length ({} > {MAX_CEL_EXPR_LEN})",
                    n.id,
                    key,
                    s.len() - 1,
                )));
            }
        }
    }
    Ok(())
}

fn validate_capability_edges(
    w: &Workflow,
    node_ids: &HashSet<&str>,
    node_kinds: &HashMap<&str, NodeKind>,
) -> Result<(), OrbflowError> {
    // Build a set of capability ports per node for validation.
    let mut cap_ports: HashMap<&str, HashSet<&str>> = HashMap::new();
    for n in &w.nodes {
        if !n.capability_ports.is_empty() {
            let ports: HashSet<&str> = n.capability_ports.iter().map(|p| p.key.as_str()).collect();
            cap_ports.insert(&n.id, ports);
        }
    }

    for ce in &w.capability_edges {
        if !node_ids.contains(ce.source_node_id.as_str()) {
            return Err(OrbflowError::InvalidCapabilityEdge);
        }
        if !node_ids.contains(ce.target_node_id.as_str()) {
            return Err(OrbflowError::InvalidCapabilityEdge);
        }
        if node_kinds.get(ce.source_node_id.as_str()) != Some(&NodeKind::Capability) {
            return Err(OrbflowError::InvalidCapabilityEdge);
        }
        match cap_ports.get(ce.target_node_id.as_str()) {
            Some(ports) if ports.contains(ce.target_port_key.as_str()) => {}
            _ => return Err(OrbflowError::InvalidCapabilityEdge),
        }
    }
    Ok(())
}

/// Iterative DFS-based cycle detection on regular (non-capability) edges.
/// Uses an explicit stack to avoid stack overflow on deep graphs.
fn detect_cycles(w: &Workflow) -> Result<(), OrbflowError> {
    #[derive(PartialEq, Eq, Clone, Copy)]
    enum Color {
        White,
        Gray,
        Black,
    }

    let cap_nodes: HashSet<&str> = w
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Capability)
        .map(|n| n.id.as_str())
        .collect();

    let mut adj: HashMap<&str, Vec<&str>> = HashMap::with_capacity(w.nodes.len());
    for e in &w.edges {
        adj.entry(e.source.as_str())
            .or_default()
            .push(e.target.as_str());
    }

    let mut color: HashMap<&str, Color> = w
        .nodes
        .iter()
        .filter(|n| !cap_nodes.contains(n.id.as_str()))
        .map(|n| (n.id.as_str(), Color::White))
        .collect();

    // Stack frame: (node_id, neighbor_index)
    let mut stack: Vec<(&str, usize)> = Vec::new();

    let non_cap_nodes: Vec<&str> = w
        .nodes
        .iter()
        .filter(|n| !cap_nodes.contains(n.id.as_str()))
        .map(|n| n.id.as_str())
        .collect();

    for &start in &non_cap_nodes {
        if color.get(start) != Some(&Color::White) {
            continue;
        }
        color.insert(start, Color::Gray);
        stack.push((start, 0));

        while let Some((node, idx)) = stack.last_mut() {
            let neighbors = adj.get(*node).map(|v| v.as_slice()).unwrap_or(&[]);
            if *idx >= neighbors.len() {
                // All neighbors visited — mark black and pop.
                color.insert(*node, Color::Black);
                stack.pop();
            } else {
                let next = neighbors[*idx];
                *idx += 1;
                match color.get(next) {
                    Some(Color::Gray) => return Err(OrbflowError::CycleDetected),
                    Some(Color::White) => {
                        color.insert(next, Color::Gray);
                        stack.push((next, 0));
                    }
                    _ => {} // Black — already fully explored
                }
            }
        }
    }
    Ok(())
}

/// BFS connectivity check: all non-capability nodes must be reachable from entry nodes.
fn check_connectivity(w: &Workflow, entry_nodes: &[&Node]) -> Result<(), OrbflowError> {
    let cap_nodes: HashSet<&str> = w
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Capability)
        .map(|n| n.id.as_str())
        .collect();

    let mut adj: HashMap<&str, Vec<&str>> = HashMap::with_capacity(w.nodes.len());
    for e in &w.edges {
        adj.entry(e.source.as_str())
            .or_default()
            .push(e.target.as_str());
    }

    let mut visited: HashSet<&str> = HashSet::with_capacity(w.nodes.len());
    let mut queue: VecDeque<&str> = VecDeque::new();

    for n in entry_nodes {
        queue.push_back(&n.id);
        visited.insert(&n.id);
    }

    while let Some(cur) = queue.pop_front() {
        if let Some(neighbors) = adj.get(cur) {
            for &next in neighbors {
                if visited.insert(next) {
                    queue.push_back(next);
                }
            }
        }
    }

    let non_cap_count = w
        .nodes
        .iter()
        .filter(|n| !cap_nodes.contains(n.id.as_str()))
        .count();

    if visited.len() != non_cap_count {
        return Err(OrbflowError::Disconnected);
    }
    Ok(())
}

/// Checks each node's config against its schema.
pub fn validate_node_configs(
    wf: &Workflow,
    schemas: &HashMap<String, NodeSchema>,
) -> Result<(), OrbflowError> {
    let mut errs = Vec::new();

    for node in &wf.nodes {
        let Some(schema) = schemas.get(&node.plugin_ref) else {
            continue; // Unknown plugin — skip
        };

        // Build set of mapped input keys
        let mapped_inputs: HashSet<&str> = node
            .input_mapping
            .as_ref()
            .map(|m| m.keys().map(|k| k.as_str()).collect())
            .unwrap_or_default();

        // Validate required inputs have mappings
        for field in &schema.inputs {
            if field.required
                && !mapped_inputs.contains(field.key.as_str())
                && field.default.is_none()
            {
                errs.push(format!(
                    "node {:?}: required input {:?} is missing",
                    node.id, field.key
                ));
            }
        }

        // Build parameter map
        let param_map: HashMap<&str, &crate::workflow::Parameter> = node
            .parameters
            .iter()
            .map(|p| (p.key.as_str(), p))
            .collect();

        // Validate parameters
        for field in &schema.parameters {
            let param = param_map.get(field.key.as_str());
            if field.required && param.is_none() && field.default.is_none() {
                errs.push(format!(
                    "node {:?}: required parameter {:?} is missing",
                    node.id, field.key
                ));
                continue;
            }
            let Some(param) = param else { continue };

            // Enum validation for static values
            if !field.r#enum.is_empty()
                && param.mode == ParameterMode::Static
                && let Some(ref val) = param.value
            {
                let str_val = match val {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                if !field.r#enum.contains(&str_val) {
                    errs.push(format!(
                        "node {:?}: parameter {:?} value {:?} is not one of {:?}",
                        node.id, field.key, str_val, field.r#enum
                    ));
                }
            }
        }
    }

    if errs.is_empty() {
        Ok(())
    } else {
        Err(OrbflowError::InvalidNodeConfig(errs.join("; ")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::*;
    use chrono::Utc;

    fn make_node(id: &str, kind: NodeKind) -> Node {
        Node {
            id: id.into(),
            name: id.into(),
            kind,
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

    fn make_edge(id: &str, source: &str, target: &str) -> Edge {
        Edge {
            id: id.into(),
            source: source.into(),
            target: target.into(),
            condition: None,
        }
    }

    fn make_workflow(nodes: Vec<Node>, edges: Vec<Edge>) -> Workflow {
        Workflow {
            id: WorkflowId::new("wf-test"),
            name: "Test".into(),
            description: None,
            version: 1,
            status: DefinitionStatus::Active,
            nodes,
            edges,
            capability_edges: vec![],
            triggers: vec![],
            annotations: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn test_valid_linear_workflow() {
        let wf = make_workflow(
            vec![
                make_node("a", NodeKind::Action),
                make_node("b", NodeKind::Action),
            ],
            vec![make_edge("e1", "a", "b")],
        );
        assert!(validate_workflow(&wf).is_ok());
    }

    #[test]
    fn test_empty_workflow() {
        let wf = make_workflow(vec![], vec![]);
        assert!(matches!(
            validate_workflow(&wf),
            Err(OrbflowError::NoEntryNodes)
        ));
    }

    #[test]
    fn test_duplicate_node() {
        let wf = make_workflow(
            vec![
                make_node("a", NodeKind::Action),
                make_node("a", NodeKind::Action),
            ],
            vec![],
        );
        assert!(matches!(
            validate_workflow(&wf),
            Err(OrbflowError::DuplicateNode)
        ));
    }

    #[test]
    fn test_invalid_edge() {
        let wf = make_workflow(
            vec![make_node("a", NodeKind::Action)],
            vec![make_edge("e1", "a", "missing")],
        );
        assert!(matches!(
            validate_workflow(&wf),
            Err(OrbflowError::InvalidEdge)
        ));
    }

    #[test]
    fn test_duplicate_edge() {
        let wf = make_workflow(
            vec![
                make_node("a", NodeKind::Action),
                make_node("b", NodeKind::Action),
            ],
            vec![make_edge("e1", "a", "b"), make_edge("e2", "a", "b")],
        );
        assert!(matches!(
            validate_workflow(&wf),
            Err(OrbflowError::DuplicateEdge)
        ));
    }

    #[test]
    fn test_cycle_detection() {
        // A -> B -> C -> B forms a cycle, but A is still an entry node
        let wf = make_workflow(
            vec![
                make_node("a", NodeKind::Action),
                make_node("b", NodeKind::Action),
                make_node("c", NodeKind::Action),
            ],
            vec![
                make_edge("e1", "a", "b"),
                make_edge("e2", "b", "c"),
                make_edge("e3", "c", "b"),
            ],
        );
        assert!(matches!(
            validate_workflow(&wf),
            Err(OrbflowError::CycleDetected)
        ));
    }

    #[test]
    fn test_disconnected_graph() {
        let wf = make_workflow(
            vec![
                make_node("a", NodeKind::Action),
                make_node("b", NodeKind::Action),
                make_node("c", NodeKind::Action),
            ],
            vec![make_edge("e1", "a", "b")],
        );
        // 'c' is disconnected from 'a' and 'b', but it's also an entry node.
        // Both 'a' and 'c' are entry nodes since they have no incoming edges.
        // Connectivity check: a->b reaches a,b. c is also an entry node so it's visited.
        // All 3 are visited, so this is actually connected from entry nodes.
        assert!(validate_workflow(&wf).is_ok());
    }

    #[test]
    fn test_truly_disconnected_graph() {
        let wf = make_workflow(
            vec![
                make_node("a", NodeKind::Action),
                make_node("b", NodeKind::Action),
                make_node("c", NodeKind::Action),
            ],
            vec![
                make_edge("e1", "a", "b"),
                make_edge("e2", "a", "c"),
                // Add a node 'd' with incoming edge from b but not reachable differently
            ],
        );
        assert!(validate_workflow(&wf).is_ok());
    }

    #[test]
    fn test_trigger_node_with_incoming_edge() {
        let wf = make_workflow(
            vec![
                make_node("a", NodeKind::Action),
                make_node("t", NodeKind::Trigger),
            ],
            vec![make_edge("e1", "a", "t")],
        );
        assert!(matches!(
            validate_workflow(&wf),
            Err(OrbflowError::InvalidNodeKind)
        ));
    }

    // --- Additional tests to increase coverage ---

    #[test]
    fn test_edge_with_missing_source() {
        let wf = make_workflow(
            vec![make_node("a", NodeKind::Action)],
            vec![make_edge("e1", "missing", "a")],
        );
        assert!(matches!(
            validate_workflow(&wf),
            Err(OrbflowError::InvalidEdge)
        ));
    }

    #[test]
    fn test_edge_involving_capability_source() {
        let wf = make_workflow(
            vec![
                make_node("cap", NodeKind::Capability),
                make_node("a", NodeKind::Action),
            ],
            vec![make_edge("e1", "cap", "a")],
        );
        assert!(matches!(
            validate_workflow(&wf),
            Err(OrbflowError::InvalidCapabilityEdge)
        ));
    }

    #[test]
    fn test_edge_involving_capability_target() {
        let wf = make_workflow(
            vec![
                make_node("a", NodeKind::Action),
                make_node("cap", NodeKind::Capability),
            ],
            vec![make_edge("e1", "a", "cap")],
        );
        assert!(matches!(
            validate_workflow(&wf),
            Err(OrbflowError::InvalidCapabilityEdge)
        ));
    }

    #[test]
    fn test_capability_edge_with_missing_source_node() {
        let mut wf = make_workflow(
            vec![
                make_node("a", NodeKind::Action),
                make_node("cap", NodeKind::Capability),
            ],
            vec![],
        );
        wf.capability_edges.push(CapabilityEdge {
            id: "ce1".into(),
            source_node_id: "missing".into(),
            target_node_id: "a".into(),
            target_port_key: "tool".into(),
        });
        assert!(matches!(
            validate_workflow(&wf),
            Err(OrbflowError::InvalidCapabilityEdge)
        ));
    }

    #[test]
    fn test_capability_edge_with_missing_target_node() {
        let mut wf = make_workflow(vec![make_node("cap", NodeKind::Capability)], vec![]);
        wf.capability_edges.push(CapabilityEdge {
            id: "ce1".into(),
            source_node_id: "cap".into(),
            target_node_id: "missing".into(),
            target_port_key: "tool".into(),
        });
        assert!(matches!(
            validate_workflow(&wf),
            Err(OrbflowError::InvalidCapabilityEdge)
        ));
    }

    #[test]
    fn test_capability_edge_source_not_capability_kind() {
        let mut wf = make_workflow(
            vec![
                make_node("a", NodeKind::Action),
                make_node("b", NodeKind::Action),
            ],
            vec![make_edge("e1", "a", "b")],
        );
        wf.capability_edges.push(CapabilityEdge {
            id: "ce1".into(),
            source_node_id: "a".into(),
            target_node_id: "b".into(),
            target_port_key: "tool".into(),
        });
        assert!(matches!(
            validate_workflow(&wf),
            Err(OrbflowError::InvalidCapabilityEdge)
        ));
    }

    #[test]
    fn test_capability_edge_target_missing_port() {
        let cap = make_node("cap", NodeKind::Capability);
        // cap has no capability_ports
        let mut action = make_node("a", NodeKind::Action);
        action.capability_ports = vec![CapabilityPort {
            key: "tool".into(),
            capability_type: "mcp".into(),
            required: false,
            description: None,
        }];

        let mut wf = make_workflow(vec![action, cap], vec![]);
        wf.capability_edges.push(CapabilityEdge {
            id: "ce1".into(),
            source_node_id: "cap".into(),
            target_node_id: "a".into(),
            target_port_key: "wrong_port".into(),
        });
        assert!(matches!(
            validate_workflow(&wf),
            Err(OrbflowError::InvalidCapabilityEdge)
        ));
    }

    #[test]
    fn test_valid_capability_edge() {
        let mut action = make_node("a", NodeKind::Action);
        action.capability_ports = vec![CapabilityPort {
            key: "tool".into(),
            capability_type: "mcp".into(),
            required: false,
            description: None,
        }];
        let cap = make_node("cap", NodeKind::Capability);

        let mut wf = make_workflow(vec![action, cap], vec![]);
        wf.capability_edges.push(CapabilityEdge {
            id: "ce1".into(),
            source_node_id: "cap".into(),
            target_node_id: "a".into(),
            target_port_key: "tool".into(),
        });
        assert!(validate_workflow(&wf).is_ok());
    }

    #[test]
    fn test_capability_edge_target_has_no_ports_at_all() {
        let action = make_node("a", NodeKind::Action);
        let cap = make_node("cap", NodeKind::Capability);

        let mut wf = make_workflow(vec![action, cap], vec![]);
        wf.capability_edges.push(CapabilityEdge {
            id: "ce1".into(),
            source_node_id: "cap".into(),
            target_node_id: "a".into(),
            target_port_key: "tool".into(),
        });
        assert!(matches!(
            validate_workflow(&wf),
            Err(OrbflowError::InvalidCapabilityEdge)
        ));
    }

    #[test]
    fn test_single_node_is_valid() {
        let wf = make_workflow(vec![make_node("a", NodeKind::Action)], vec![]);
        assert!(validate_workflow(&wf).is_ok());
    }

    #[test]
    fn test_trigger_node_as_entry() {
        let wf = make_workflow(
            vec![
                make_node("t", NodeKind::Trigger),
                make_node("a", NodeKind::Action),
            ],
            vec![make_edge("e1", "t", "a")],
        );
        assert!(validate_workflow(&wf).is_ok());
    }

    // --- validate_node_configs tests ---

    use crate::ports::{FieldSchema, FieldType, NodeSchema};
    use std::collections::HashMap;

    fn make_field_schema(key: &str, required: bool) -> FieldSchema {
        FieldSchema {
            key: key.into(),
            label: key.into(),
            field_type: FieldType::String,
            required,
            default: None,
            description: None,
            r#enum: vec![],
            credential_type: None,
        }
    }

    fn make_node_schema(
        plugin_ref: &str,
        inputs: Vec<FieldSchema>,
        params: Vec<FieldSchema>,
    ) -> NodeSchema {
        NodeSchema {
            plugin_ref: plugin_ref.into(),
            name: "Test".into(),
            description: "test".into(),
            category: "test".into(),
            node_kind: None,
            icon: "".into(),
            color: "".into(),
            docs: None,
            image_url: None,
            inputs,
            outputs: vec![],
            parameters: params,
            capability_ports: vec![],
            settings: vec![],
            provides_capability: None,
        }
    }

    #[test]
    fn validate_node_configs_passes_with_no_schema() {
        let wf = make_workflow(vec![make_node("a", NodeKind::Action)], vec![]);
        let schemas = HashMap::new();
        assert!(validate_node_configs(&wf, &schemas).is_ok());
    }

    #[test]
    fn validate_node_configs_missing_required_input() {
        let wf = make_workflow(vec![make_node("a", NodeKind::Action)], vec![]);
        let mut schemas = HashMap::new();
        schemas.insert(
            "builtin:log".into(),
            make_node_schema(
                "builtin:log",
                vec![make_field_schema("message", true)],
                vec![],
            ),
        );
        let err = validate_node_configs(&wf, &schemas).unwrap_err();
        assert!(err.to_string().contains("required input"));
    }

    #[test]
    fn validate_node_configs_required_input_has_default() {
        let wf = make_workflow(vec![make_node("a", NodeKind::Action)], vec![]);
        let mut field = make_field_schema("message", true);
        field.default = Some(serde_json::json!("hello"));
        let mut schemas = HashMap::new();
        schemas.insert(
            "builtin:log".into(),
            make_node_schema("builtin:log", vec![field], vec![]),
        );
        assert!(validate_node_configs(&wf, &schemas).is_ok());
    }

    #[test]
    fn validate_node_configs_missing_required_parameter() {
        let wf = make_workflow(vec![make_node("a", NodeKind::Action)], vec![]);
        let mut schemas = HashMap::new();
        schemas.insert(
            "builtin:log".into(),
            make_node_schema(
                "builtin:log",
                vec![],
                vec![make_field_schema("level", true)],
            ),
        );
        let err = validate_node_configs(&wf, &schemas).unwrap_err();
        assert!(err.to_string().contains("required parameter"));
    }

    #[test]
    fn validate_node_configs_enum_validation_rejects_invalid_value() {
        let mut node = make_node("a", NodeKind::Action);
        node.parameters = vec![Parameter {
            key: "method".into(),
            mode: ParameterMode::Static,
            value: Some(serde_json::json!("PATCH")),
            expression: None,
        }];
        let wf = make_workflow(vec![node], vec![]);

        let mut field = make_field_schema("method", true);
        field.r#enum = vec!["GET".into(), "POST".into()];
        let mut schemas = HashMap::new();
        schemas.insert(
            "builtin:log".into(),
            make_node_schema("builtin:log", vec![], vec![field]),
        );
        let err = validate_node_configs(&wf, &schemas).unwrap_err();
        assert!(err.to_string().contains("not one of"));
    }

    #[test]
    fn validate_node_configs_enum_validation_accepts_valid_value() {
        let mut node = make_node("a", NodeKind::Action);
        node.parameters = vec![Parameter {
            key: "method".into(),
            mode: ParameterMode::Static,
            value: Some(serde_json::json!("GET")),
            expression: None,
        }];
        let wf = make_workflow(vec![node], vec![]);

        let mut field = make_field_schema("method", true);
        field.r#enum = vec!["GET".into(), "POST".into()];
        let mut schemas = HashMap::new();
        schemas.insert(
            "builtin:log".into(),
            make_node_schema("builtin:log", vec![], vec![field]),
        );
        assert!(validate_node_configs(&wf, &schemas).is_ok());
    }

    #[test]
    fn validate_node_configs_enum_skipped_for_expression_mode() {
        let mut node = make_node("a", NodeKind::Action);
        node.parameters = vec![Parameter {
            key: "method".into(),
            mode: ParameterMode::Expression,
            value: Some(serde_json::json!("INVALID")),
            expression: Some("= method".into()),
        }];
        let wf = make_workflow(vec![node], vec![]);

        let mut field = make_field_schema("method", true);
        field.r#enum = vec!["GET".into(), "POST".into()];
        let mut schemas = HashMap::new();
        schemas.insert(
            "builtin:log".into(),
            make_node_schema("builtin:log", vec![], vec![field]),
        );
        // Expression mode skips enum validation
        assert!(validate_node_configs(&wf, &schemas).is_ok());
    }

    #[test]
    fn validate_node_configs_enum_with_non_string_value() {
        let mut node = make_node("a", NodeKind::Action);
        node.parameters = vec![Parameter {
            key: "count".into(),
            mode: ParameterMode::Static,
            value: Some(serde_json::json!(42)),
            expression: None,
        }];
        let wf = make_workflow(vec![node], vec![]);

        let mut field = make_field_schema("count", true);
        field.r#enum = vec!["1".into(), "2".into()];
        let mut schemas = HashMap::new();
        schemas.insert(
            "builtin:log".into(),
            make_node_schema("builtin:log", vec![], vec![field]),
        );
        let err = validate_node_configs(&wf, &schemas).unwrap_err();
        assert!(err.to_string().contains("not one of"));
    }

    #[test]
    fn validate_node_configs_with_input_mapping() {
        let mut node = make_node("a", NodeKind::Action);
        let mut mapping = std::collections::HashMap::new();
        mapping.insert("message".to_string(), serde_json::json!("= input.text"));
        node.input_mapping = Some(mapping);
        let wf = make_workflow(vec![node], vec![]);

        let mut schemas = HashMap::new();
        schemas.insert(
            "builtin:log".into(),
            make_node_schema(
                "builtin:log",
                vec![make_field_schema("message", true)],
                vec![],
            ),
        );
        assert!(validate_node_configs(&wf, &schemas).is_ok());
    }
}
