// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! PostgreSQL implementation of the MetricsStore port.

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use orbflow_core::OrbflowError;
use orbflow_core::execution::InstanceId;
use orbflow_core::metrics::{
    InstanceExecutionMetrics, NodeExecutionMetrics, NodeMetricsSummary, WorkflowMetricsSummary,
};
use orbflow_core::ports::MetricsStore;
use orbflow_core::workflow::WorkflowId;

use crate::store::PgStore;

#[derive(Debug, sqlx::FromRow)]
struct WorkflowMetricsRow {
    total: i64,
    successful: i64,
    failed: i64,
    avg_ms: f64,
    p50: f64,
    p95: f64,
    p99: f64,
}

#[derive(Debug, sqlx::FromRow)]
struct NodeMetricsRow {
    node_id: String,
    plugin_ref: String,
    total: i64,
    successful: i64,
    failed: i64,
    avg_ms: f64,
    p50: f64,
    p95: f64,
}

#[derive(Debug, sqlx::FromRow)]
struct InstanceMetricsRow {
    instance_id: String,
    workflow_id: String,
    status: String,
    duration_ms: i64,
    node_count: i32,
    failed_node_count: i32,
    started_at: DateTime<Utc>,
    completed_at: DateTime<Utc>,
    node_durations: serde_json::Value,
}

#[async_trait]
impl MetricsStore for PgStore {
    async fn record_node_metrics(&self, m: &NodeExecutionMetrics) -> Result<(), OrbflowError> {
        sqlx::query(
            "INSERT INTO node_metrics (instance_id, workflow_id, node_id, plugin_ref, status, duration_ms, started_at, completed_at, attempt, tokens, cost_usd_scaled)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
        )
        .bind(&m.instance_id.0)
        .bind(&m.workflow_id.0)
        .bind(&m.node_id)
        .bind(&m.plugin_ref)
        .bind(&m.status)
        .bind(m.duration_ms)
        .bind(m.started_at)
        .bind(m.completed_at)
        .bind(m.attempt)
        .bind(m.tokens)
        .bind(m.cost_usd_scaled)
        .execute(&self.pool)
        .await
        .map_err(|e| OrbflowError::Internal(format!("record node metrics: {e}")))?;
        Ok(())
    }

    async fn record_instance_metrics(
        &self,
        m: &InstanceExecutionMetrics,
    ) -> Result<(), OrbflowError> {
        let node_durations = serde_json::to_value(&m.node_durations)
            .map_err(|e| OrbflowError::Internal(format!("serialize node_durations: {e}")))?;

        sqlx::query(
            "INSERT INTO instance_metrics (instance_id, workflow_id, status, duration_ms, node_count, failed_node_count, started_at, completed_at, node_durations)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
             ON CONFLICT (instance_id) DO UPDATE SET
                status = EXCLUDED.status,
                duration_ms = EXCLUDED.duration_ms,
                node_count = EXCLUDED.node_count,
                failed_node_count = EXCLUDED.failed_node_count,
                completed_at = EXCLUDED.completed_at,
                node_durations = EXCLUDED.node_durations",
        )
        .bind(&m.instance_id.0)
        .bind(&m.workflow_id.0)
        .bind(&m.status)
        .bind(m.duration_ms)
        .bind(m.node_count)
        .bind(m.failed_node_count)
        .bind(m.started_at)
        .bind(m.completed_at)
        .bind(node_durations)
        .execute(&self.pool)
        .await
        .map_err(|e| OrbflowError::Internal(format!("record instance metrics: {e}")))?;
        Ok(())
    }

    async fn get_workflow_metrics(
        &self,
        workflow_id: &WorkflowId,
        since: DateTime<Utc>,
    ) -> Result<WorkflowMetricsSummary, OrbflowError> {
        let row: WorkflowMetricsRow = sqlx::query_as(
            "SELECT
                COUNT(*) as total,
                COUNT(*) FILTER (WHERE status = 'completed') as successful,
                COUNT(*) FILTER (WHERE status = 'failed') as failed,
                COALESCE(AVG(duration_ms)::float8, 0) as avg_ms,
                COALESCE(PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY duration_ms)::float8, 0) as p50,
                COALESCE(PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY duration_ms)::float8, 0) as p95,
                COALESCE(PERCENTILE_CONT(0.99) WITHIN GROUP (ORDER BY duration_ms)::float8, 0) as p99
             FROM instance_metrics
             WHERE workflow_id = $1 AND created_at >= $2",
        )
        .bind(&workflow_id.0)
        .bind(since)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| OrbflowError::Internal(format!("get workflow metrics: {e}")))?;

        let success_rate = if row.total > 0 {
            row.successful as f64 / row.total as f64
        } else {
            0.0
        };

        Ok(WorkflowMetricsSummary {
            workflow_id: workflow_id.clone(),
            total_executions: row.total,
            successful_executions: row.successful,
            failed_executions: row.failed,
            success_rate,
            avg_duration_ms: row.avg_ms,
            p50_duration_ms: row.p50,
            p95_duration_ms: row.p95,
            p99_duration_ms: row.p99,
            since,
        })
    }

    async fn get_node_metrics(
        &self,
        workflow_id: &WorkflowId,
        since: DateTime<Utc>,
    ) -> Result<Vec<NodeMetricsSummary>, OrbflowError> {
        let rows: Vec<NodeMetricsRow> = sqlx::query_as(
            "SELECT
                node_id,
                plugin_ref,
                COUNT(*) as total,
                COUNT(*) FILTER (WHERE status = 'completed') as successful,
                COUNT(*) FILTER (WHERE status = 'failed') as failed,
                COALESCE(AVG(duration_ms)::float8, 0) as avg_ms,
                COALESCE(PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY duration_ms)::float8, 0) as p50,
                COALESCE(PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY duration_ms)::float8, 0) as p95
             FROM node_metrics
             WHERE workflow_id = $1 AND created_at >= $2
             GROUP BY node_id, plugin_ref
             ORDER BY node_id",
        )
        .bind(&workflow_id.0)
        .bind(since)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| OrbflowError::Internal(format!("get node metrics: {e}")))?;

        Ok(rows
            .into_iter()
            .map(|r| {
                let success_rate = if r.total > 0 {
                    r.successful as f64 / r.total as f64
                } else {
                    0.0
                };
                NodeMetricsSummary {
                    node_id: r.node_id,
                    plugin_ref: r.plugin_ref,
                    total_executions: r.total,
                    successful_executions: r.successful,
                    failed_executions: r.failed,
                    success_rate,
                    avg_duration_ms: r.avg_ms,
                    p50_duration_ms: r.p50,
                    p95_duration_ms: r.p95,
                }
            })
            .collect())
    }

    async fn get_instance_metrics(
        &self,
        instance_id: &InstanceId,
    ) -> Result<Option<InstanceExecutionMetrics>, OrbflowError> {
        let row: Option<InstanceMetricsRow> = sqlx::query_as(
            "SELECT instance_id, workflow_id, status, duration_ms, node_count, failed_node_count, started_at, completed_at, node_durations
             FROM instance_metrics WHERE instance_id = $1",
        )
        .bind(&instance_id.0)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| OrbflowError::Internal(format!("get instance metrics: {e}")))?;

        match row {
            None => Ok(None),
            Some(r) => {
                let node_durations: std::collections::HashMap<String, i64> =
                    serde_json::from_value(r.node_durations).map_err(|e| {
                        OrbflowError::Internal(format!("deserialize node_durations: {e}"))
                    })?;
                Ok(Some(InstanceExecutionMetrics {
                    instance_id: InstanceId::new(&r.instance_id),
                    workflow_id: WorkflowId(r.workflow_id),
                    status: r.status,
                    duration_ms: r.duration_ms,
                    node_count: r.node_count,
                    failed_node_count: r.failed_node_count,
                    started_at: r.started_at,
                    completed_at: r.completed_at,
                    node_durations,
                }))
            }
        }
    }
}
