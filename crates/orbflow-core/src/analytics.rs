// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Analytics types for aggregated execution statistics and trend analysis.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Time range filter for analytics queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRange {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

/// Aggregated execution statistics over a time range.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStats {
    pub total: i64,
    pub succeeded: i64,
    pub failed: i64,
    pub running: i64,
    pub avg_duration_ms: f64,
    pub p50_duration_ms: f64,
    pub p95_duration_ms: f64,
    pub p99_duration_ms: f64,
    pub executions_by_day: Vec<DailyCount>,
}

/// Per-day execution counts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyCount {
    pub date: String,
    pub count: i64,
    pub failed: i64,
}

/// Per-node performance metrics over a time range.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodePerformance {
    pub node_id: String,
    pub plugin_ref: String,
    pub execution_count: i64,
    pub failure_count: i64,
    pub avg_duration_ms: f64,
    pub p95_duration_ms: f64,
    pub p99_duration_ms: f64,
}

/// Daily failure trend for a workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureTrend {
    pub date: String,
    pub workflow_id: String,
    pub failure_count: i64,
    pub total_count: i64,
    pub failure_rate: f64,
}
