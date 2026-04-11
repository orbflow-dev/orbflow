// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Alert evaluation engine for checking metrics against alert rules.

use orbflow_core::OrbflowError;
use orbflow_core::alerts::{AlertMetric, AlertRule};
use orbflow_core::analytics::{ExecutionStats, TimeRange};
use orbflow_core::ports::AnalyticsStore;
use std::sync::Arc;
use tracing::info;

/// Evaluates all enabled alert rules against current analytics data.
pub struct AlertEvaluator {
    analytics_store: Arc<dyn AnalyticsStore>,
}

impl AlertEvaluator {
    /// Creates a new alert evaluator backed by the given analytics store.
    pub fn new(analytics_store: Arc<dyn AnalyticsStore>) -> Self {
        Self { analytics_store }
    }

    /// Evaluates a set of alert rules against current metrics for the given time range.
    /// Returns the rules that triggered (i.e., their condition was met).
    pub async fn evaluate_rules(
        &self,
        rules: &[AlertRule],
        range: &TimeRange,
    ) -> Result<Vec<AlertRule>, OrbflowError> {
        let stats = self.analytics_store.execution_stats(range).await?;
        let mut triggered = Vec::new();

        for rule in rules {
            if !rule.enabled {
                continue;
            }

            let current_value = extract_metric_value(&stats, &rule.metric);
            if rule.evaluate(current_value) {
                info!(
                    rule_id = %rule.id,
                    metric = ?rule.metric,
                    current_value,
                    threshold = rule.threshold,
                    "alert rule triggered"
                );
                triggered.push(rule.clone());
            }
        }

        Ok(triggered)
    }
}

/// Extracts the current value for a given metric from execution stats.
fn extract_metric_value(stats: &ExecutionStats, metric: &AlertMetric) -> f64 {
    match metric {
        AlertMetric::FailureRate => {
            if stats.total == 0 {
                0.0
            } else {
                stats.failed as f64 / stats.total as f64
            }
        }
        AlertMetric::P95Duration => stats.p95_duration_ms,
        AlertMetric::ExecutionCount => stats.total as f64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use orbflow_core::alerts::{AlertChannel, AlertOperator};
    use orbflow_core::analytics::DailyCount;

    fn make_stats(total: i64, failed: i64, p95: f64) -> ExecutionStats {
        ExecutionStats {
            total,
            succeeded: total - failed,
            failed,
            running: 0,
            avg_duration_ms: 100.0,
            p50_duration_ms: 80.0,
            p95_duration_ms: p95,
            p99_duration_ms: p95 * 1.2,
            executions_by_day: vec![DailyCount {
                date: "2026-03-22".to_string(),
                count: total,
                failed,
            }],
        }
    }

    #[test]
    fn extracts_failure_rate_correctly() {
        let stats = make_stats(100, 25, 500.0);
        let value = extract_metric_value(&stats, &AlertMetric::FailureRate);
        assert!((value - 0.25).abs() < f64::EPSILON);
    }

    #[test]
    fn extracts_failure_rate_zero_when_no_executions() {
        let stats = make_stats(0, 0, 0.0);
        let value = extract_metric_value(&stats, &AlertMetric::FailureRate);
        assert!((value - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn extracts_p95_duration() {
        let stats = make_stats(100, 5, 1234.5);
        let value = extract_metric_value(&stats, &AlertMetric::P95Duration);
        assert!((value - 1234.5).abs() < f64::EPSILON);
    }

    #[test]
    fn extracts_execution_count() {
        let stats = make_stats(42, 3, 100.0);
        let value = extract_metric_value(&stats, &AlertMetric::ExecutionCount);
        assert!((value - 42.0).abs() < f64::EPSILON);
    }

    fn make_rule(metric: AlertMetric, operator: AlertOperator, threshold: f64) -> AlertRule {
        AlertRule {
            id: "test".to_string(),
            workflow_id: None,
            metric,
            operator,
            threshold,
            channel: AlertChannel::Log,
            enabled: true,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn failure_rate_alert_triggers() {
        let rule = make_rule(AlertMetric::FailureRate, AlertOperator::GreaterThan, 0.1);
        let stats = make_stats(100, 20, 500.0);
        let value = extract_metric_value(&stats, &rule.metric);
        assert!(rule.evaluate(value));
    }

    #[test]
    fn p95_alert_does_not_trigger_below_threshold() {
        let rule = make_rule(AlertMetric::P95Duration, AlertOperator::GreaterThan, 1000.0);
        let stats = make_stats(100, 5, 500.0);
        let value = extract_metric_value(&stats, &rule.metric);
        assert!(!rule.evaluate(value));
    }
}
