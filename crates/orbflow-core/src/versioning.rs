// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Workflow versioning and PR-style change request system.
//!
//! Every workflow save creates a new version. Change requests allow proposing,
//! reviewing, and approving workflow modifications before publishing.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::workflow::WorkflowId;

/// A snapshot of a workflow definition at a specific version.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowVersion {
    /// Auto-incrementing version number (1-based).
    pub version: i32,
    /// The workflow this version belongs to.
    pub workflow_id: WorkflowId,
    /// The full workflow definition as JSON (nodes, edges, config).
    pub definition: Value,
    /// Who created this version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Commit-style message describing the changes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// When this version was created.
    pub created_at: DateTime<Utc>,
}

/// A diff between two workflow versions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDiff {
    /// Version being compared from.
    pub from_version: i32,
    /// Version being compared to.
    pub to_version: i32,
    /// Nodes added in the new version.
    pub added_nodes: Vec<String>,
    /// Nodes removed from the old version.
    pub removed_nodes: Vec<String>,
    /// Nodes that exist in both but have changed config/position.
    pub modified_nodes: Vec<String>,
    /// Edges added.
    pub added_edges: Vec<(String, String)>,
    /// Edges removed.
    pub removed_edges: Vec<(String, String)>,
}

/// Computes the diff between two workflow definitions.
///
/// Both definitions are expected to be JSON objects with `nodes` and `edges` arrays.
pub fn compute_diff(from: &Value, to: &Value, from_ver: i32, to_ver: i32) -> WorkflowDiff {
    let from_nodes = extract_node_ids(from);
    let to_nodes = extract_node_ids(to);
    let from_edges = extract_edges(from);
    let to_edges = extract_edges(to);

    let added_nodes: Vec<String> = to_nodes
        .keys()
        .filter(|id| !from_nodes.contains_key(*id))
        .cloned()
        .collect();

    let removed_nodes: Vec<String> = from_nodes
        .keys()
        .filter(|id| !to_nodes.contains_key(*id))
        .cloned()
        .collect();

    let modified_nodes: Vec<String> = from_nodes
        .keys()
        .filter(|id| {
            to_nodes
                .get(*id)
                .is_some_and(|to_val| from_nodes.get(*id) != Some(to_val))
        })
        .cloned()
        .collect();

    let added_edges: Vec<(String, String)> = to_edges
        .iter()
        .filter(|e| !from_edges.contains(e))
        .cloned()
        .collect();

    let removed_edges: Vec<(String, String)> = from_edges
        .iter()
        .filter(|e| !to_edges.contains(e))
        .cloned()
        .collect();

    WorkflowDiff {
        from_version: from_ver,
        to_version: to_ver,
        added_nodes,
        removed_nodes,
        modified_nodes,
        added_edges,
        removed_edges,
    }
}

/// Extracts node IDs and their full definitions from a workflow JSON.
fn extract_node_ids(definition: &Value) -> HashMap<String, Value> {
    let mut map = HashMap::new();
    if let Some(nodes) = definition.get("nodes").and_then(|n| n.as_array()) {
        for node in nodes {
            if let Some(id) = node.get("id").and_then(|v| v.as_str()) {
                map.insert(id.to_string(), node.clone());
            }
        }
    }
    map
}

/// Extracts edges as (source, target) pairs from a workflow JSON.
fn extract_edges(definition: &Value) -> Vec<(String, String)> {
    let mut edges = Vec::new();
    if let Some(edge_arr) = definition.get("edges").and_then(|e| e.as_array()) {
        for edge in edge_arr {
            let source = edge.get("source").and_then(|v| v.as_str());
            let target = edge.get("target").and_then(|v| v.as_str());
            if let (Some(s), Some(t)) = (source, target) {
                edges.push((s.to_string(), t.to_string()));
            }
        }
    }
    edges
}

// ─── Change Requests ────────────────────────────────────────────────────────

/// Status of a change request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeRequestStatus {
    /// Initial state — author is still editing.
    Draft,
    /// Submitted for review.
    Open,
    /// Approved by reviewer(s).
    Approved,
    /// Rejected by reviewer(s).
    Rejected,
    /// Merged into the workflow (new version created).
    Merged,
}

/// A change request proposing modifications to a workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeRequest {
    /// Unique change request ID.
    pub id: String,
    /// The workflow being modified.
    pub workflow_id: WorkflowId,
    /// Short title describing the change.
    pub title: String,
    /// Detailed description of the change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The proposed workflow definition (full snapshot).
    pub proposed_definition: Value,
    /// The base version this CR was created from.
    pub base_version: i32,
    /// Current status.
    pub status: ChangeRequestStatus,
    /// Who created the CR.
    pub author: String,
    /// Assigned reviewers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reviewers: Vec<String>,
    /// Inline comments on specific nodes/edges.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub comments: Vec<ReviewComment>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// An inline comment on a specific element of a workflow change request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewComment {
    /// Unique comment ID.
    pub id: String,
    /// Who wrote the comment.
    pub author: String,
    /// The comment text.
    pub body: String,
    /// Optional reference to a specific node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
    /// Optional reference to a specific edge (source -> target).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edge_ref: Option<(String, String)>,
    /// Whether this comment has been resolved.
    #[serde(default)]
    pub resolved: bool,
    pub created_at: DateTime<Utc>,
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn v1_definition() -> Value {
        serde_json::json!({
            "nodes": [
                {"id": "start", "type": "trigger"},
                {"id": "http-1", "type": "http", "config": {"url": "https://example.com"}},
                {"id": "end", "type": "log"}
            ],
            "edges": [
                {"source": "start", "target": "http-1"},
                {"source": "http-1", "target": "end"}
            ]
        })
    }

    fn v2_definition() -> Value {
        serde_json::json!({
            "nodes": [
                {"id": "start", "type": "trigger"},
                {"id": "http-1", "type": "http", "config": {"url": "https://new-url.com"}},
                {"id": "transform-1", "type": "transform"},
                {"id": "end", "type": "log"}
            ],
            "edges": [
                {"source": "start", "target": "http-1"},
                {"source": "http-1", "target": "transform-1"},
                {"source": "transform-1", "target": "end"}
            ]
        })
    }

    #[test]
    fn test_diff_added_nodes() {
        let diff = compute_diff(&v1_definition(), &v2_definition(), 1, 2);
        assert_eq!(diff.added_nodes, vec!["transform-1"]);
        assert!(diff.removed_nodes.is_empty());
    }

    #[test]
    fn test_diff_modified_nodes() {
        let diff = compute_diff(&v1_definition(), &v2_definition(), 1, 2);
        assert!(diff.modified_nodes.contains(&"http-1".to_string()));
    }

    #[test]
    fn test_diff_edges() {
        let diff = compute_diff(&v1_definition(), &v2_definition(), 1, 2);
        assert!(
            diff.added_edges
                .contains(&("http-1".into(), "transform-1".into()))
        );
        assert!(
            diff.added_edges
                .contains(&("transform-1".into(), "end".into()))
        );
        assert!(
            diff.removed_edges
                .contains(&("http-1".into(), "end".into()))
        );
    }

    #[test]
    fn test_diff_identical() {
        let def = v1_definition();
        let diff = compute_diff(&def, &def, 1, 1);
        assert!(diff.added_nodes.is_empty());
        assert!(diff.removed_nodes.is_empty());
        assert!(diff.modified_nodes.is_empty());
        assert!(diff.added_edges.is_empty());
        assert!(diff.removed_edges.is_empty());
    }

    #[test]
    fn test_diff_removed_nodes() {
        let diff = compute_diff(&v2_definition(), &v1_definition(), 2, 1);
        assert_eq!(diff.removed_nodes, vec!["transform-1"]);
    }

    #[test]
    fn test_change_request_serde() {
        let cr = ChangeRequest {
            id: "cr-1".into(),
            workflow_id: WorkflowId::new("wf-1"),
            title: "Add transform step".into(),
            description: Some("Adds a transform node between HTTP and log".into()),
            proposed_definition: v2_definition(),
            base_version: 1,
            status: ChangeRequestStatus::Open,
            author: "alice".into(),
            reviewers: vec!["bob".into()],
            comments: vec![ReviewComment {
                id: "comment-1".into(),
                author: "bob".into(),
                body: "Looks good, but should we validate the schema?".into(),
                node_id: Some("transform-1".into()),
                edge_ref: None,
                resolved: false,
                created_at: Utc::now(),
            }],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let json = serde_json::to_string(&cr).unwrap();
        let cr2: ChangeRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(cr2.id, "cr-1");
        assert_eq!(cr2.status, ChangeRequestStatus::Open);
        assert_eq!(cr2.comments.len(), 1);
    }

    #[test]
    fn test_change_request_status_values() {
        let statuses = vec![
            ChangeRequestStatus::Draft,
            ChangeRequestStatus::Open,
            ChangeRequestStatus::Approved,
            ChangeRequestStatus::Rejected,
            ChangeRequestStatus::Merged,
        ];
        for status in statuses {
            let json = serde_json::to_value(status).unwrap();
            let roundtripped: ChangeRequestStatus = serde_json::from_value(json).unwrap();
            assert_eq!(status, roundtripped);
        }
    }

    #[test]
    fn test_workflow_version_serde() {
        let wv = WorkflowVersion {
            version: 3,
            workflow_id: WorkflowId::new("wf-1"),
            definition: v1_definition(),
            author: Some("alice".into()),
            message: Some("Initial version".into()),
            created_at: Utc::now(),
        };
        let json = serde_json::to_string(&wv).unwrap();
        let wv2: WorkflowVersion = serde_json::from_str(&json).unwrap();
        assert_eq!(wv2.version, 3);
        assert_eq!(wv2.author, Some("alice".into()));
    }
}
