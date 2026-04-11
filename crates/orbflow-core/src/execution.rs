// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Execution domain types — workflow instances and node states.

use std::collections::HashMap;
use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::metering::InstanceMetrics;
use crate::trigger::TriggerType;
use crate::workflow::WorkflowId;

/// Uniquely identifies a workflow execution instance.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InstanceId(pub String);

impl InstanceId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl fmt::Display for InstanceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for InstanceId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for InstanceId {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

/// Lifecycle state of a running workflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstanceStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// Lifecycle state of a node execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeStatus {
    Pending,
    Queued,
    Running,
    Completed,
    Failed,
    Skipped,
    Cancelled,
    /// Node is paused, waiting for human approval before execution continues.
    WaitingApproval,
}

impl NodeStatus {
    /// Returns true if the node is in a terminal state.
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed | Self::Skipped | Self::Cancelled
        )
    }

    /// Returns true if the node is paused waiting for external input.
    pub fn is_waiting(self) -> bool {
        matches!(self, Self::WaitingApproval)
    }
}

/// Captures how a workflow instance was triggered.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerInfo {
    #[serde(rename = "type")]
    pub trigger_type: TriggerType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<HashMap<String, serde_json::Value>>,
}

/// Shared state accessible during workflow execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    pub variables: HashMap<String, serde_json::Value>,
    pub node_outputs: HashMap<String, HashMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger_data: Option<TriggerInfo>,
    /// The user who initiated this execution. `None` for anonymous/system executions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
}

impl ExecutionContext {
    /// Creates a context initialized with the given input variables.
    pub fn new(input: HashMap<String, serde_json::Value>) -> Self {
        Self {
            variables: input,
            node_outputs: HashMap::new(),
            trigger_data: None,
            user_id: None,
        }
    }

    /// Creates a context with the given input variables and user ID.
    pub fn with_user(input: HashMap<String, serde_json::Value>, user_id: Option<String>) -> Self {
        Self {
            variables: input,
            node_outputs: HashMap::new(),
            trigger_data: None,
            user_id,
        }
    }
}

/// Tracks the execution state of a single node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeState {
    pub node_id: String,
    pub status: NodeStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<HashMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<HashMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<HashMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default)]
    pub attempt: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<DateTime<Utc>>,
}

/// Tracks compensation progress during rollback.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SagaState {
    pub compensating: bool,
    #[serde(default)]
    pub completed_nodes: Vec<String>,
    #[serde(default)]
    pub compensated_nodes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failed_node: Option<String>,
}

/// A running (or completed) workflow execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instance {
    pub id: InstanceId,
    pub workflow_id: WorkflowId,
    pub status: InstanceStatus,
    pub node_states: HashMap<String, NodeState>,
    pub context: ExecutionContext,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub saga: Option<SagaState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<InstanceId>,
    /// Accumulated resource metrics for this execution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance_metrics: Option<InstanceMetrics>,
    /// Version of the workflow definition at the time this instance was created.
    /// Used to fetch the correct workflow version for rendering the execution map.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow_version: Option<i32>,
    /// Owner/tenant identifier used for scoped credential resolution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<String>,
    #[serde(default)]
    pub version: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Instance {
    /// Returns true if the instance is in a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status,
            InstanceStatus::Completed | InstanceStatus::Failed | InstanceStatus::Cancelled
        )
    }

    /// Returns true if every node is in a terminal state.
    pub fn all_nodes_terminal(&self) -> bool {
        self.node_states.values().all(|ns| ns.status.is_terminal())
    }
}

/// Result of executing a single node (and its ancestors) for testing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestNodeResult {
    pub node_outputs: HashMap<String, NodeState>,
    pub target_node: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_status_is_terminal() {
        assert!(NodeStatus::Completed.is_terminal());
        assert!(NodeStatus::Failed.is_terminal());
        assert!(NodeStatus::Skipped.is_terminal());
        assert!(NodeStatus::Cancelled.is_terminal());
        assert!(!NodeStatus::Pending.is_terminal());
        assert!(!NodeStatus::Queued.is_terminal());
        assert!(!NodeStatus::Running.is_terminal());
    }

    #[test]
    fn test_instance_is_terminal() {
        let inst = Instance {
            id: InstanceId::new("inst-1"),
            workflow_id: WorkflowId::new("wf-1"),
            status: InstanceStatus::Completed,
            node_states: HashMap::new(),
            context: ExecutionContext::new(HashMap::new()),
            saga: None,
            parent_id: None,
            instance_metrics: None,
            workflow_version: None,
            owner_id: None,
            version: 1,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        assert!(inst.is_terminal());
    }

    #[test]
    fn test_all_nodes_terminal() {
        let mut inst = Instance {
            id: InstanceId::new("inst-1"),
            workflow_id: WorkflowId::new("wf-1"),
            status: InstanceStatus::Running,
            node_states: HashMap::new(),
            context: ExecutionContext::new(HashMap::new()),
            saga: None,
            parent_id: None,
            instance_metrics: None,
            workflow_version: None,
            owner_id: None,
            version: 1,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        inst.node_states.insert(
            "a".into(),
            NodeState {
                node_id: "a".into(),
                status: NodeStatus::Completed,
                input: None,
                output: None,
                parameters: None,
                error: None,
                attempt: 1,
                started_at: None,
                ended_at: None,
            },
        );
        assert!(inst.all_nodes_terminal());

        inst.node_states.insert(
            "b".into(),
            NodeState {
                node_id: "b".into(),
                status: NodeStatus::Running,
                input: None,
                output: None,
                parameters: None,
                error: None,
                attempt: 1,
                started_at: None,
                ended_at: None,
            },
        );
        assert!(!inst.all_nodes_terminal());
    }

    #[test]
    fn test_execution_context_new() {
        let mut input = HashMap::new();
        input.insert("key".to_owned(), serde_json::json!("value"));
        let ctx = ExecutionContext::new(input);
        assert_eq!(ctx.variables.get("key").unwrap(), "value");
        assert!(ctx.node_outputs.is_empty());
        assert!(ctx.trigger_data.is_none());
    }
}
