// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Execution metrics types for persistence and API responses.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::execution::InstanceId;
use crate::workflow::WorkflowId;

/// Metrics recorded for a single node execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeExecutionMetrics {
    pub instance_id: InstanceId,
    pub workflow_id: WorkflowId,
    pub node_id: String,
    pub plugin_ref: String,
    pub status: String,
    pub duration_ms: i64,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
    pub attempt: i32,
    /// Optional token usage for AI nodes.
    pub tokens: Option<i64>,
    /// Optional cost in USD (scaled by 10000).
    pub cost_usd_scaled: Option<i64>,
}

/// Metrics recorded for a completed workflow instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceExecutionMetrics {
    pub instance_id: InstanceId,
    pub workflow_id: WorkflowId,
    pub status: String,
    pub duration_ms: i64,
    pub node_count: i32,
    pub failed_node_count: i32,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
    /// Per-node durations for breakdown.
    pub node_durations: HashMap<String, i64>,
}

/// Aggregated metrics summary for a workflow definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowMetricsSummary {
    pub workflow_id: WorkflowId,
    pub total_executions: i64,
    pub successful_executions: i64,
    pub failed_executions: i64,
    pub success_rate: f64,
    pub avg_duration_ms: f64,
    pub p50_duration_ms: f64,
    pub p95_duration_ms: f64,
    pub p99_duration_ms: f64,
    pub since: DateTime<Utc>,
}

/// Per-node aggregated metrics within a workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeMetricsSummary {
    pub node_id: String,
    pub plugin_ref: String,
    pub total_executions: i64,
    pub successful_executions: i64,
    pub failed_executions: i64,
    pub success_rate: f64,
    pub avg_duration_ms: f64,
    pub p50_duration_ms: f64,
    pub p95_duration_ms: f64,
}
