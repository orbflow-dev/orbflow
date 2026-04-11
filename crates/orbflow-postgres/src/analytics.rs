// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! PostgreSQL implementation of the AnalyticsStore port.

use async_trait::async_trait;

use orbflow_core::OrbflowError;
use orbflow_core::analytics::{
    DailyCount, ExecutionStats, FailureTrend, NodePerformance, TimeRange,
};
use orbflow_core::ports::AnalyticsStore;

use crate::store::PgStore;

#[async_trait]
impl AnalyticsStore for PgStore {
    async fn execution_stats(&self, range: &TimeRange) -> Result<ExecutionStats, OrbflowError> {
        // Aggregate totals and percentiles from instance_metrics.
        let summary: (i64, i64, i64, i64, f64, f64, f64, f64) = sqlx::query_as(
            "SELECT
                COUNT(*) AS total,
                COUNT(*) FILTER (WHERE status = 'completed') AS succeeded,
                COUNT(*) FILTER (WHERE status = 'failed') AS failed,
                COUNT(*) FILTER (WHERE status = 'running') AS running,
                COALESCE(AVG(duration_ms)::float8, 0) AS avg_ms,
                COALESCE(PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY duration_ms)::float8, 0) AS p50,
                COALESCE(PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY duration_ms)::float8, 0) AS p95,
                COALESCE(PERCENTILE_CONT(0.99) WITHIN GROUP (ORDER BY duration_ms)::float8, 0) AS p99
             FROM instance_metrics
             WHERE started_at >= $1 AND started_at < $2",
        )
        .bind(range.start)
        .bind(range.end)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| OrbflowError::Internal(format!("execution_stats summary: {e}")))?;

        // Daily breakdown.
        let daily_rows: Vec<(String, i64, i64)> = sqlx::query_as(
            "SELECT
                TO_CHAR(started_at::date, 'YYYY-MM-DD') AS day,
                COUNT(*) AS count,
                COUNT(*) FILTER (WHERE status = 'failed') AS failed
             FROM instance_metrics
             WHERE started_at >= $1 AND started_at < $2
             GROUP BY started_at::date
             ORDER BY started_at::date",
        )
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| OrbflowError::Internal(format!("execution_stats daily: {e}")))?;

        let executions_by_day = daily_rows
            .into_iter()
            .map(|(date, count, failed)| DailyCount {
                date,
                count,
                failed,
            })
            .collect();

        Ok(ExecutionStats {
            total: summary.0,
            succeeded: summary.1,
            failed: summary.2,
            running: summary.3,
            avg_duration_ms: summary.4,
            p50_duration_ms: summary.5,
            p95_duration_ms: summary.6,
            p99_duration_ms: summary.7,
            executions_by_day,
        })
    }

    async fn node_performance(
        &self,
        range: &TimeRange,
    ) -> Result<Vec<NodePerformance>, OrbflowError> {
        let rows: Vec<(String, String, i64, i64, f64, f64, f64)> = sqlx::query_as(
            "SELECT
                node_id,
                plugin_ref,
                COUNT(*) AS execution_count,
                COUNT(*) FILTER (WHERE status = 'failed') AS failure_count,
                COALESCE(AVG(duration_ms)::float8, 0) AS avg_ms,
                COALESCE(PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY duration_ms)::float8, 0) AS p95,
                COALESCE(PERCENTILE_CONT(0.99) WITHIN GROUP (ORDER BY duration_ms)::float8, 0) AS p99
             FROM node_metrics
             WHERE started_at >= $1 AND started_at < $2
             GROUP BY node_id, plugin_ref
             ORDER BY execution_count DESC",
        )
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| OrbflowError::Internal(format!("node_performance: {e}")))?;

        Ok(rows
            .into_iter()
            .map(
                |(node_id, plugin_ref, execution_count, failure_count, avg_ms, p95, p99)| {
                    NodePerformance {
                        node_id,
                        plugin_ref,
                        execution_count,
                        failure_count,
                        avg_duration_ms: avg_ms,
                        p95_duration_ms: p95,
                        p99_duration_ms: p99,
                    }
                },
            )
            .collect())
    }

    async fn failure_trends(&self, range: &TimeRange) -> Result<Vec<FailureTrend>, OrbflowError> {
        let rows: Vec<(String, String, i64, i64)> = sqlx::query_as(
            "SELECT
                TO_CHAR(started_at::date, 'YYYY-MM-DD') AS day,
                workflow_id,
                COUNT(*) FILTER (WHERE status = 'failed') AS failure_count,
                COUNT(*) AS total_count
             FROM instance_metrics
             WHERE started_at >= $1 AND started_at < $2
             GROUP BY started_at::date, workflow_id
             HAVING COUNT(*) FILTER (WHERE status = 'failed') > 0
             ORDER BY started_at::date, workflow_id",
        )
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| OrbflowError::Internal(format!("failure_trends: {e}")))?;

        Ok(rows
            .into_iter()
            .map(|(date, workflow_id, failure_count, total_count)| {
                let failure_rate = if total_count > 0 {
                    failure_count as f64 / total_count as f64
                } else {
                    0.0
                };
                FailureTrend {
                    date,
                    workflow_id,
                    failure_count,
                    total_count,
                    failure_rate,
                }
            })
            .collect())
    }
}
