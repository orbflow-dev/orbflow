// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Workflow domain types — the DAG blueprint defining automated tasks.

use std::collections::HashMap;
use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::trigger::{Trigger, TriggerType};

/// Uniquely identifies a workflow definition.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorkflowId(pub String);

impl WorkflowId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl fmt::Display for WorkflowId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for WorkflowId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for WorkflowId {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

/// Lifecycle state of a workflow definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DefinitionStatus {
    Draft,
    Active,
    Archived,
}

impl Default for DefinitionStatus {
    fn default() -> Self {
        Self::Draft
    }
}

/// Categorizes nodes for routing to the correct executor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    Builtin,
    Plugin,
}

impl<'de> serde::Deserialize<'de> for NodeType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "builtin" | "task" => Ok(Self::Builtin),
            "plugin" => Ok(Self::Plugin),
            other => Err(serde::de::Error::unknown_variant(
                other,
                &["builtin", "task", "plugin"],
            )),
        }
    }
}

impl Default for NodeType {
    fn default() -> Self {
        Self::Builtin
    }
}

/// Distinguishes the three top-level node categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Trigger,
    Action,
    Capability,
}

impl Default for NodeKind {
    fn default() -> Self {
        Self::Action
    }
}

/// Indicates how a parameter gets its value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterMode {
    Static,
    Expression,
}

/// Visual position of a node in the editor.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

impl Default for Position {
    fn default() -> Self {
        Self { x: 0.0, y: 0.0 }
    }
}

/// Retry behavior configuration for a node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_attempts: i32,
    /// Delay in milliseconds.
    pub delay: u64,
    pub multiplier: f64,
}

/// Compensation action for saga rollback.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompensateConfig {
    pub plugin_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_mapping: Option<HashMap<String, serde_json::Value>>,
}

/// A single named config value that can be static or expression-mapped.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameter {
    pub key: String,
    pub mode: ParameterMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expression: Option<String>,
}

/// Declares that a node requires a capability of a given type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityPort {
    pub key: String,
    pub capability_type: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Connects an action node's capability port to a capability node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityEdge {
    pub id: String,
    pub source_node_id: String,
    pub target_node_id: String,
    pub target_port_key: String,
}

/// Richer descriptive information for a node instance.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NodeMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub docs: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
}

/// A non-executable canvas element (sticky note, text, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Annotation {
    pub id: String,
    #[serde(rename = "type")]
    pub annotation_type: String,
    pub content: String,
    pub position: Position,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<HashMap<String, serde_json::Value>>,
}

/// Trigger-specific settings for trigger-kind nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerNodeConfig {
    pub trigger_type: TriggerType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cron: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// An atomic unit of work within a workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub kind: NodeKind,
    #[serde(rename = "type")]
    pub node_type: NodeType,
    pub plugin_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_mapping: Option<HashMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<HashMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameters: Vec<Parameter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry: Option<RetryPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compensate: Option<CompensateConfig>,
    #[serde(default)]
    pub position: Position,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capability_ports: Vec<CapabilityPort>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<NodeMetadata>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger_config: Option<TriggerNodeConfig>,
    /// When true, the node pauses for human approval before executing.
    #[serde(default)]
    pub requires_approval: bool,
}

/// Connects two nodes. Condition is an optional CEL expression.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub id: String,
    pub source: String,
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
}

/// Returns the current UTC timestamp; used as serde default for timestamp fields.
fn default_now() -> DateTime<Utc> {
    Utc::now()
}

/// A DAG blueprint defining a sequence of automated tasks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    #[serde(default)]
    pub id: WorkflowId,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub version: i32,
    #[serde(default)]
    pub status: DefinitionStatus,
    #[serde(default)]
    pub nodes: Vec<Node>,
    #[serde(default)]
    pub edges: Vec<Edge>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capability_edges: Vec<CapabilityEdge>,
    /// Deprecated: Use trigger-kind Nodes with TriggerConfig instead.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub triggers: Vec<Trigger>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub annotations: Vec<Annotation>,
    #[serde(default = "default_now")]
    pub created_at: DateTime<Utc>,
    #[serde(default = "default_now")]
    pub updated_at: DateTime<Utc>,
}

impl Workflow {
    /// Returns the node with the given ID, or `None`.
    pub fn node_by_id(&self, id: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// Returns nodes with no incoming edges (excludes capability nodes).
    pub fn entry_nodes(&self) -> Vec<&Node> {
        let has_incoming: std::collections::HashSet<&str> =
            self.edges.iter().map(|e| e.target.as_str()).collect();
        self.nodes
            .iter()
            .filter(|n| n.kind != NodeKind::Capability && !has_incoming.contains(n.id.as_str()))
            .collect()
    }

    /// Returns all nodes with `Kind == Trigger`.
    pub fn trigger_nodes(&self) -> Vec<&Node> {
        self.nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Trigger)
            .collect()
    }

    /// Returns all nodes with `Kind == Capability`.
    pub fn capability_nodes(&self) -> Vec<&Node> {
        self.nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Capability)
            .collect()
    }

    /// Returns all non-trigger, non-capability nodes.
    pub fn action_nodes(&self) -> Vec<&Node> {
        self.nodes
            .iter()
            .filter(|n| n.kind != NodeKind::Trigger && n.kind != NodeKind::Capability)
            .collect()
    }

    /// Returns all ancestor node IDs in topological order (leaves first).
    pub fn ancestors_of(&self, node_id: &str) -> Vec<String> {
        let mut predecessors: HashMap<&str, Vec<&str>> = HashMap::new();
        for e in &self.edges {
            predecessors
                .entry(e.target.as_str())
                .or_default()
                .push(e.source.as_str());
        }

        let mut visited = std::collections::HashSet::new();
        let mut result = Vec::new();

        fn dfs<'a>(
            id: &'a str,
            predecessors: &HashMap<&str, Vec<&'a str>>,
            visited: &mut std::collections::HashSet<&'a str>,
            result: &mut Vec<String>,
        ) {
            if visited.contains(id) {
                return;
            }
            visited.insert(id);
            if let Some(preds) = predecessors.get(id) {
                for &p in preds {
                    dfs(p, predecessors, visited, result);
                }
            }
            result.push(id.to_owned());
        }

        if let Some(preds) = predecessors.get(node_id) {
            for &p in preds {
                dfs(p, &predecessors, &mut visited, &mut result);
            }
        }

        result
    }

    /// Returns edges originating from the given node.
    pub fn outgoing_edges(&self, node_id: &str) -> Vec<&Edge> {
        self.edges.iter().filter(|e| e.source == node_id).collect()
    }
}

/// Converts deprecated `Workflow.triggers` into trigger-kind Node entries.
/// No-op if the workflow already has trigger nodes or no legacy triggers.
pub fn migrate_legacy_triggers(wf: &mut Workflow) {
    if !wf.trigger_nodes().is_empty() || wf.triggers.is_empty() {
        return;
    }

    let entry_nodes: Vec<String> = wf.entry_nodes().iter().map(|n| n.id.clone()).collect();
    let base_x = wf
        .entry_nodes()
        .first()
        .map(|n| n.position.x - 250.0)
        .unwrap_or(0.0);

    let trigger_plugin_ref = |t: &TriggerType| -> Option<&'static str> {
        match t {
            TriggerType::Manual => Some("builtin:trigger-manual"),
            TriggerType::Schedule => Some("builtin:trigger-cron"),
            TriggerType::Webhook => Some("builtin:trigger-webhook"),
            TriggerType::Event => Some("builtin:trigger-event"),
        }
    };

    let trigger_name = |t: &TriggerType| -> &'static str {
        match t {
            TriggerType::Manual => "Manual Trigger",
            TriggerType::Schedule => "Schedule",
            TriggerType::Webhook => "Webhook",
            TriggerType::Event => "Event",
        }
    };

    let triggers_clone = wf.triggers.clone();
    for (i, t) in triggers_clone.iter().enumerate() {
        let Some(plugin_ref) = trigger_plugin_ref(&t.trigger_type) else {
            continue;
        };
        let node_id = format!("_trigger_{i}");
        let node = Node {
            id: node_id.clone(),
            name: trigger_name(&t.trigger_type).to_owned(),
            kind: NodeKind::Trigger,
            node_type: NodeType::Builtin,
            plugin_ref: plugin_ref.to_owned(),
            position: Position {
                x: base_x,
                y: (i as f64) * 150.0,
            },
            trigger_config: Some(TriggerNodeConfig {
                trigger_type: t.trigger_type.clone(),
                cron: t.config.cron.clone(),
                event_name: t.config.event_name.clone(),
                path: t.config.path.clone(),
            }),
            input_mapping: None,
            config: None,
            parameters: Vec::new(),
            retry: None,
            compensate: None,
            capability_ports: Vec::new(),
            metadata: None,
            requires_approval: false,
        };
        wf.nodes.push(node);

        for en_id in &entry_nodes {
            wf.edges.push(Edge {
                id: format!("_trigger_edge_{i}_{en_id}"),
                source: node_id.clone(),
                target: en_id.clone(),
                condition: None,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_workflow() -> Workflow {
        Workflow {
            id: WorkflowId::new("wf-1"),
            name: "Test".into(),
            description: None,
            version: 1,
            status: DefinitionStatus::Active,
            nodes: vec![
                Node {
                    id: "a".into(),
                    name: "A".into(),
                    kind: NodeKind::Action,
                    node_type: NodeType::Builtin,
                    plugin_ref: "builtin:log".into(),
                    position: Position { x: 0.0, y: 0.0 },
                    input_mapping: None,
                    config: None,
                    parameters: vec![],
                    retry: None,
                    compensate: None,
                    capability_ports: vec![],
                    metadata: None,
                    trigger_config: None,
                    requires_approval: false,
                },
                Node {
                    id: "b".into(),
                    name: "B".into(),
                    kind: NodeKind::Action,
                    node_type: NodeType::Builtin,
                    plugin_ref: "builtin:log".into(),
                    position: Position { x: 200.0, y: 0.0 },
                    input_mapping: None,
                    config: None,
                    parameters: vec![],
                    retry: None,
                    compensate: None,
                    capability_ports: vec![],
                    metadata: None,
                    trigger_config: None,
                    requires_approval: false,
                },
                Node {
                    id: "c".into(),
                    name: "C".into(),
                    kind: NodeKind::Action,
                    node_type: NodeType::Builtin,
                    plugin_ref: "builtin:log".into(),
                    position: Position { x: 400.0, y: 0.0 },
                    input_mapping: None,
                    config: None,
                    parameters: vec![],
                    retry: None,
                    compensate: None,
                    capability_ports: vec![],
                    metadata: None,
                    trigger_config: None,
                    requires_approval: false,
                },
            ],
            edges: vec![
                Edge {
                    id: "e1".into(),
                    source: "a".into(),
                    target: "b".into(),
                    condition: None,
                },
                Edge {
                    id: "e2".into(),
                    source: "b".into(),
                    target: "c".into(),
                    condition: None,
                },
            ],
            capability_edges: vec![],
            triggers: vec![],
            annotations: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn test_node_by_id() {
        let wf = test_workflow();
        assert!(wf.node_by_id("a").is_some());
        assert!(wf.node_by_id("missing").is_none());
    }

    #[test]
    fn test_entry_nodes() {
        let wf = test_workflow();
        let entries = wf.entry_nodes();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "a");
    }

    #[test]
    fn test_ancestors_of() {
        let wf = test_workflow();
        let ancestors = wf.ancestors_of("c");
        assert_eq!(ancestors, vec!["a", "b"]);
    }

    #[test]
    fn test_outgoing_edges() {
        let wf = test_workflow();
        let out = wf.outgoing_edges("a");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].target, "b");
    }

    #[test]
    fn test_serde_roundtrip() {
        let wf = test_workflow();
        let json = serde_json::to_string(&wf).unwrap();
        let wf2: Workflow = serde_json::from_str(&json).unwrap();
        assert_eq!(wf.id, wf2.id);
        assert_eq!(wf.nodes.len(), wf2.nodes.len());
    }

    // ── WorkflowId ──────────────────────────────────────────────

    #[test]
    fn workflow_id_display() {
        let id = WorkflowId::new("wf-42");
        assert_eq!(id.to_string(), "wf-42");
    }

    #[test]
    fn workflow_id_from_string() {
        let id: WorkflowId = String::from("wf-owned").into();
        assert_eq!(id.0, "wf-owned");
    }

    #[test]
    fn workflow_id_from_str() {
        let id: WorkflowId = "wf-ref".into();
        assert_eq!(id.0, "wf-ref");
    }

    #[test]
    fn workflow_id_default_is_empty() {
        let id = WorkflowId::default();
        assert!(id.0.is_empty());
    }

    // ── Defaults ────────────────────────────────────────────────

    #[test]
    fn definition_status_default_is_draft() {
        assert_eq!(DefinitionStatus::default(), DefinitionStatus::Draft);
    }

    #[test]
    fn node_type_default_is_builtin() {
        assert_eq!(NodeType::default(), NodeType::Builtin);
    }

    #[test]
    fn node_kind_default_is_action() {
        assert_eq!(NodeKind::default(), NodeKind::Action);
    }

    #[test]
    fn position_default_is_origin() {
        let p = Position::default();
        assert_eq!(p.x, 0.0);
        assert_eq!(p.y, 0.0);
    }

    // ── NodeType custom deserialization ─────────────────────────

    #[test]
    fn node_type_deserializes_builtin() {
        let nt: NodeType = serde_json::from_str(r#""builtin""#).unwrap();
        assert_eq!(nt, NodeType::Builtin);
    }

    #[test]
    fn node_type_deserializes_plugin() {
        let nt: NodeType = serde_json::from_str(r#""plugin""#).unwrap();
        assert_eq!(nt, NodeType::Plugin);
    }

    #[test]
    fn node_type_task_maps_to_builtin() {
        let nt: NodeType = serde_json::from_str(r#""task""#).unwrap();
        assert_eq!(nt, NodeType::Builtin);
    }

    #[test]
    fn node_type_unknown_string_returns_error() {
        let result: Result<NodeType, _> = serde_json::from_str(r#""custom""#);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("unknown variant"));
    }

    // ── DefinitionStatus serde ──────────────────────────────────

    #[test]
    fn definition_status_serde_roundtrip() {
        for status in [
            DefinitionStatus::Draft,
            DefinitionStatus::Active,
            DefinitionStatus::Archived,
        ] {
            let json = serde_json::to_string(&status).unwrap();
            let back: DefinitionStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(status, back);
        }
    }

    // ── Workflow query methods ───────────────────────────────────

    fn workflow_with_kinds() -> Workflow {
        let make_node = |id: &str, kind: NodeKind| Node {
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
        };

        Workflow {
            id: WorkflowId::new("wf-kinds"),
            name: "Kinds".into(),
            description: Some("test description".into()),
            version: 1,
            status: DefinitionStatus::Active,
            nodes: vec![
                make_node("t1", NodeKind::Trigger),
                make_node("a1", NodeKind::Action),
                make_node("a2", NodeKind::Action),
                make_node("c1", NodeKind::Capability),
            ],
            edges: vec![
                Edge {
                    id: "e1".into(),
                    source: "t1".into(),
                    target: "a1".into(),
                    condition: None,
                },
                Edge {
                    id: "e2".into(),
                    source: "a1".into(),
                    target: "a2".into(),
                    condition: Some("result.ok == true".into()),
                },
            ],
            capability_edges: vec![],
            triggers: vec![],
            annotations: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn trigger_nodes_returns_only_triggers() {
        let wf = workflow_with_kinds();
        let triggers = wf.trigger_nodes();
        assert_eq!(triggers.len(), 1);
        assert_eq!(triggers[0].id, "t1");
    }

    #[test]
    fn capability_nodes_returns_only_capabilities() {
        let wf = workflow_with_kinds();
        let caps = wf.capability_nodes();
        assert_eq!(caps.len(), 1);
        assert_eq!(caps[0].id, "c1");
    }

    #[test]
    fn action_nodes_excludes_trigger_and_capability() {
        let wf = workflow_with_kinds();
        let actions = wf.action_nodes();
        assert_eq!(actions.len(), 2);
        let ids: Vec<&str> = actions.iter().map(|n| n.id.as_str()).collect();
        assert!(ids.contains(&"a1"));
        assert!(ids.contains(&"a2"));
    }

    #[test]
    fn entry_nodes_excludes_capability_nodes() {
        let wf = workflow_with_kinds();
        // t1 has no incoming edge and is a Trigger (not Capability) → entry
        // c1 has no incoming edge but is Capability → excluded
        let entries = wf.entry_nodes();
        let ids: Vec<&str> = entries.iter().map(|n| n.id.as_str()).collect();
        assert!(ids.contains(&"t1"));
        assert!(!ids.contains(&"c1"));
    }

    #[test]
    fn outgoing_edges_returns_empty_for_leaf_node() {
        let wf = test_workflow();
        let out = wf.outgoing_edges("c");
        assert!(out.is_empty());
    }

    #[test]
    fn outgoing_edges_returns_empty_for_unknown_node() {
        let wf = test_workflow();
        let out = wf.outgoing_edges("nonexistent");
        assert!(out.is_empty());
    }

    #[test]
    fn ancestors_of_root_is_empty() {
        let wf = test_workflow();
        let ancestors = wf.ancestors_of("a");
        assert!(ancestors.is_empty());
    }

    #[test]
    fn ancestors_of_unknown_node_is_empty() {
        let wf = test_workflow();
        let ancestors = wf.ancestors_of("nonexistent");
        assert!(ancestors.is_empty());
    }

    // ── Diamond DAG ancestors ───────────────────────────────────

    #[test]
    fn ancestors_of_diamond_dag() {
        // a -> b, a -> c, b -> d, c -> d  (diamond)
        let mut wf = test_workflow();
        wf.nodes.push(Node {
            id: "d".into(),
            name: "D".into(),
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
        });
        // Overwrite edges: a->b, a->c, b->d, c->d
        wf.edges = vec![
            Edge {
                id: "e1".into(),
                source: "a".into(),
                target: "b".into(),
                condition: None,
            },
            Edge {
                id: "e2".into(),
                source: "a".into(),
                target: "c".into(),
                condition: None,
            },
            Edge {
                id: "e3".into(),
                source: "b".into(),
                target: "d".into(),
                condition: None,
            },
            Edge {
                id: "e4".into(),
                source: "c".into(),
                target: "d".into(),
                condition: None,
            },
        ];

        let ancestors = wf.ancestors_of("d");
        // Should include a, b, c — each exactly once
        assert_eq!(ancestors.len(), 3);
        assert!(ancestors.contains(&"a".to_string()));
        assert!(ancestors.contains(&"b".to_string()));
        assert!(ancestors.contains(&"c".to_string()));
    }

    // ── migrate_legacy_triggers ─────────────────────────────────

    #[test]
    fn migrate_legacy_triggers_adds_trigger_nodes_and_edges() {
        let mut wf = test_workflow();
        wf.triggers = vec![
            Trigger {
                trigger_type: TriggerType::Manual,
                config: crate::trigger::TriggerConfig::default(),
            },
            Trigger {
                trigger_type: TriggerType::Schedule,
                config: crate::trigger::TriggerConfig {
                    cron: Some("0 * * * *".into()),
                    event_name: None,
                    path: None,
                },
            },
        ];

        let original_node_count = wf.nodes.len();
        let original_edge_count = wf.edges.len();

        migrate_legacy_triggers(&mut wf);

        // Two trigger nodes added
        assert_eq!(wf.nodes.len(), original_node_count + 2);
        // Each trigger connects to the single entry node ("a")
        assert_eq!(wf.edges.len(), original_edge_count + 2);

        // Verify trigger node properties
        let t0 = wf.node_by_id("_trigger_0").unwrap();
        assert_eq!(t0.kind, NodeKind::Trigger);
        assert_eq!(t0.plugin_ref, "builtin:trigger-manual");

        let t1 = wf.node_by_id("_trigger_1").unwrap();
        assert_eq!(t1.plugin_ref, "builtin:trigger-cron");
        assert!(t1.trigger_config.as_ref().unwrap().cron.is_some());
    }

    #[test]
    fn migrate_legacy_triggers_noop_when_trigger_nodes_exist() {
        let mut wf = workflow_with_kinds(); // already has a trigger node
        wf.triggers = vec![Trigger {
            trigger_type: TriggerType::Webhook,
            config: crate::trigger::TriggerConfig::default(),
        }];

        let node_count_before = wf.nodes.len();
        migrate_legacy_triggers(&mut wf);
        assert_eq!(wf.nodes.len(), node_count_before); // no change
    }

    #[test]
    fn migrate_legacy_triggers_noop_when_no_legacy_triggers() {
        let mut wf = test_workflow();
        assert!(wf.triggers.is_empty());

        let node_count_before = wf.nodes.len();
        migrate_legacy_triggers(&mut wf);
        assert_eq!(wf.nodes.len(), node_count_before);
    }

    #[test]
    fn migrate_legacy_triggers_all_trigger_types() {
        let mut wf = test_workflow();
        wf.triggers = vec![
            Trigger {
                trigger_type: TriggerType::Manual,
                config: crate::trigger::TriggerConfig::default(),
            },
            Trigger {
                trigger_type: TriggerType::Webhook,
                config: crate::trigger::TriggerConfig {
                    cron: None,
                    event_name: None,
                    path: Some("/hook".into()),
                },
            },
            Trigger {
                trigger_type: TriggerType::Event,
                config: crate::trigger::TriggerConfig {
                    cron: None,
                    event_name: Some("order.created".into()),
                    path: None,
                },
            },
        ];

        migrate_legacy_triggers(&mut wf);

        let t0 = wf.node_by_id("_trigger_0").unwrap();
        assert_eq!(t0.plugin_ref, "builtin:trigger-manual");
        assert_eq!(t0.name, "Manual Trigger");

        let t1 = wf.node_by_id("_trigger_1").unwrap();
        assert_eq!(t1.plugin_ref, "builtin:trigger-webhook");
        assert_eq!(t1.name, "Webhook");
        assert_eq!(
            t1.trigger_config.as_ref().unwrap().path.as_deref(),
            Some("/hook")
        );

        let t2 = wf.node_by_id("_trigger_2").unwrap();
        assert_eq!(t2.plugin_ref, "builtin:trigger-event");
        assert_eq!(t2.name, "Event");
        assert_eq!(
            t2.trigger_config.as_ref().unwrap().event_name.as_deref(),
            Some("order.created")
        );
    }

    // ── Serde edge cases ────────────────────────────────────────

    #[test]
    fn workflow_deserializes_with_minimal_json() {
        let json = r#"{"name": "Minimal"}"#;
        let wf: Workflow = serde_json::from_str(json).unwrap();
        assert_eq!(wf.name, "Minimal");
        assert_eq!(wf.id, WorkflowId::default());
        assert_eq!(wf.version, 0);
        assert_eq!(wf.status, DefinitionStatus::Draft);
        assert!(wf.nodes.is_empty());
        assert!(wf.edges.is_empty());
    }

    #[test]
    fn edge_with_condition_serde_roundtrip() {
        let edge = Edge {
            id: "e1".into(),
            source: "a".into(),
            target: "b".into(),
            condition: Some("result.status == 200".into()),
        };
        let json = serde_json::to_string(&edge).unwrap();
        let back: Edge = serde_json::from_str(&json).unwrap();
        assert_eq!(back.condition.as_deref(), Some("result.status == 200"));
    }

    #[test]
    fn node_metadata_serde_roundtrip() {
        let meta = NodeMetadata {
            description: Some("desc".into()),
            docs: Some("http://docs".into()),
            image_url: None,
        };
        let json = serde_json::to_string(&meta).unwrap();
        let back: NodeMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(back.description.as_deref(), Some("desc"));
        assert!(back.image_url.is_none());
    }

    #[test]
    fn parameter_mode_serde_roundtrip() {
        for mode in [ParameterMode::Static, ParameterMode::Expression] {
            let json = serde_json::to_string(&mode).unwrap();
            let back: ParameterMode = serde_json::from_str(&json).unwrap();
            assert_eq!(mode, back);
        }
    }

    #[test]
    fn node_kind_serde_roundtrip() {
        for kind in [NodeKind::Trigger, NodeKind::Action, NodeKind::Capability] {
            let json = serde_json::to_string(&kind).unwrap();
            let back: NodeKind = serde_json::from_str(&json).unwrap();
            assert_eq!(kind, back);
        }
    }

    #[test]
    fn migrate_positions_offset_from_entry_node() {
        let mut wf = test_workflow();
        // entry node "a" is at x=0 → trigger nodes placed at x = 0 - 250 = -250
        wf.triggers = vec![Trigger {
            trigger_type: TriggerType::Manual,
            config: crate::trigger::TriggerConfig::default(),
        }];
        migrate_legacy_triggers(&mut wf);
        let t0 = wf.node_by_id("_trigger_0").unwrap();
        assert_eq!(t0.position.x, -250.0);
        assert_eq!(t0.position.y, 0.0);
    }

    #[test]
    fn migrate_multiple_triggers_stacks_vertically() {
        let mut wf = test_workflow();
        wf.triggers = vec![
            Trigger {
                trigger_type: TriggerType::Manual,
                config: crate::trigger::TriggerConfig::default(),
            },
            Trigger {
                trigger_type: TriggerType::Webhook,
                config: crate::trigger::TriggerConfig::default(),
            },
        ];
        migrate_legacy_triggers(&mut wf);
        let t0 = wf.node_by_id("_trigger_0").unwrap();
        let t1 = wf.node_by_id("_trigger_1").unwrap();
        assert_eq!(t0.position.y, 0.0);
        assert_eq!(t1.position.y, 150.0);
    }
}
