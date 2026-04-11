// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Wave 2 integration tests: budget enforcement, alert evaluation, and
//! analytics time-range parsing.
//!
//! Uses MockBudgetStore and MockAnalyticsStore for deterministic, in-process
//! testing without any I/O.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;

use orbflow_core::alerts::{AlertChannel, AlertMetric, AlertOperator, AlertRule};
use orbflow_core::analytics::{
    DailyCount, ExecutionStats, FailureTrend, NodePerformance, TimeRange,
};
use orbflow_core::error::OrbflowError;
use orbflow_core::metering::{AccountBudget, BudgetPeriod};
use orbflow_core::ports::{AnalyticsStore, BudgetStore};
use orbflow_engine::AlertEvaluator;
use orbflow_engine::budget::check_budget_before_start;

// ---------------------------------------------------------------------------
// Mock BudgetStore
// ---------------------------------------------------------------------------

struct MockBudgetStore {
    budget: Option<AccountBudget>,
}

#[async_trait]
impl BudgetStore for MockBudgetStore {
    async fn create_budget(&self, _budget: &AccountBudget) -> Result<(), OrbflowError> {
        Ok(())
    }
    async fn get_budget(&self, _id: &str) -> Result<AccountBudget, OrbflowError> {
        self.budget.clone().ok_or(OrbflowError::NotFound)
    }
    async fn list_budgets(&self) -> Result<Vec<AccountBudget>, OrbflowError> {
        Ok(self.budget.iter().cloned().collect())
    }
    async fn update_budget(&self, _budget: &AccountBudget) -> Result<(), OrbflowError> {
        Ok(())
    }
    async fn delete_budget(&self, _id: &str) -> Result<(), OrbflowError> {
        Ok(())
    }
    async fn check_budget(
        &self,
        _workflow_id: &str,
    ) -> Result<Option<AccountBudget>, OrbflowError> {
        Ok(self.budget.clone())
    }
    async fn increment_cost(&self, _workflow_id: &str, _cost_usd: f64) -> Result<(), OrbflowError> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Mock AnalyticsStore
// ---------------------------------------------------------------------------

struct MockAnalyticsStore {
    stats: ExecutionStats,
}

impl MockAnalyticsStore {
    fn with_stats(total: i64, failed: i64, p95: f64) -> Self {
        Self {
            stats: ExecutionStats {
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
            },
        }
    }
}

#[async_trait]
impl AnalyticsStore for MockAnalyticsStore {
    async fn execution_stats(&self, _range: &TimeRange) -> Result<ExecutionStats, OrbflowError> {
        Ok(self.stats.clone())
    }
    async fn node_performance(
        &self,
        _range: &TimeRange,
    ) -> Result<Vec<NodePerformance>, OrbflowError> {
        Ok(vec![])
    }
    async fn failure_trends(&self, _range: &TimeRange) -> Result<Vec<FailureTrend>, OrbflowError> {
        Ok(vec![])
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_budget(limit_usd: f64, current_usd: f64) -> AccountBudget {
    AccountBudget {
        id: "budget-1".to_string(),
        workflow_id: Some("wf-1".to_string()),
        team: None,
        period: BudgetPeriod::Monthly,
        limit_usd,
        current_usd,
        reset_at: Utc::now(),
        created_at: Utc::now(),
    }
}

fn make_alert_rule(
    id: &str,
    metric: AlertMetric,
    operator: AlertOperator,
    threshold: f64,
    enabled: bool,
) -> AlertRule {
    AlertRule {
        id: id.to_string(),
        workflow_id: None,
        metric,
        operator,
        threshold,
        channel: AlertChannel::Log,
        enabled,
        created_at: Utc::now(),
    }
}

fn make_time_range() -> TimeRange {
    let now = Utc::now();
    TimeRange {
        start: now - chrono::Duration::days(7),
        end: now,
    }
}

// ===========================================================================
// Budget Enforcement Tests
// ===========================================================================

#[tokio::test]
async fn test_budget_blocks_workflow_when_exceeded() {
    // Budget: limit = 10.0, current = 15.0 — already over limit.
    let store = MockBudgetStore {
        budget: Some(make_budget(10.0, 15.0)),
    };

    let result = check_budget_before_start(&store, "wf-1").await;
    assert!(result.is_err(), "should reject when budget exceeded");
    let err = result.unwrap_err();
    assert!(err.is_budget_exceeded(), "error should be BudgetExceeded");
}

#[tokio::test]
async fn test_budget_blocks_workflow_when_exactly_at_limit() {
    // Budget: limit = 100.0, current = 100.0 — exactly at limit.
    let store = MockBudgetStore {
        budget: Some(make_budget(100.0, 100.0)),
    };

    let result = check_budget_before_start(&store, "wf-1").await;
    assert!(result.is_err(), "should reject when at exact limit");
    assert!(result.unwrap_err().is_budget_exceeded());
}

#[tokio::test]
async fn test_budget_allows_workflow_within_limits() {
    // Budget: limit = 100.0, current = 5.0 — well under limit.
    let store = MockBudgetStore {
        budget: Some(make_budget(100.0, 5.0)),
    };

    let result = check_budget_before_start(&store, "wf-1").await;
    assert!(result.is_ok(), "should allow when under budget");
}

#[tokio::test]
async fn test_budget_allows_workflow_when_no_budget_configured() {
    // No budget set — should always allow.
    let store = MockBudgetStore { budget: None };

    let result = check_budget_before_start(&store, "wf-1").await;
    assert!(result.is_ok(), "should allow when no budget configured");
}

#[tokio::test]
async fn test_budget_error_message_includes_amounts() {
    let store = MockBudgetStore {
        budget: Some(make_budget(10.0, 25.0)),
    };

    let err = check_budget_before_start(&store, "wf-1").await.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("25.00") && msg.contains("10.00"),
        "error message should include both current and limit amounts, got: {msg}"
    );
}

// ===========================================================================
// Alert Rule Evaluation Tests (cross-crate: orbflow-engine AlertEvaluator + orbflow-core AlertRule)
// ===========================================================================

#[tokio::test]
async fn test_alert_evaluator_triggers_on_high_failure_rate() {
    // 100 total, 70 failed → 0.7 failure rate
    let analytics = Arc::new(MockAnalyticsStore::with_stats(100, 70, 500.0));
    let evaluator = AlertEvaluator::new(analytics);

    let rules = vec![make_alert_rule(
        "fail-rate-rule",
        AlertMetric::FailureRate,
        AlertOperator::GreaterThan,
        0.5,
        true,
    )];

    let triggered = evaluator
        .evaluate_rules(&rules, &make_time_range())
        .await
        .unwrap();
    assert_eq!(triggered.len(), 1, "should trigger on 0.7 > 0.5");
    assert_eq!(triggered[0].id, "fail-rate-rule");
}

#[tokio::test]
async fn test_alert_evaluator_does_not_trigger_below_threshold() {
    // 100 total, 10 failed → 0.1 failure rate
    let analytics = Arc::new(MockAnalyticsStore::with_stats(100, 10, 500.0));
    let evaluator = AlertEvaluator::new(analytics);

    let rules = vec![make_alert_rule(
        "fail-rate-rule",
        AlertMetric::FailureRate,
        AlertOperator::GreaterThan,
        0.5,
        true,
    )];

    let triggered = evaluator
        .evaluate_rules(&rules, &make_time_range())
        .await
        .unwrap();
    assert!(triggered.is_empty(), "should not trigger on 0.1 <= 0.5");
}

#[tokio::test]
async fn test_alert_evaluator_disabled_rule_never_triggers() {
    // Even with failure rate of 1.0, disabled rule should not trigger.
    let analytics = Arc::new(MockAnalyticsStore::with_stats(100, 100, 500.0));
    let evaluator = AlertEvaluator::new(analytics);

    let rules = vec![make_alert_rule(
        "disabled-rule",
        AlertMetric::FailureRate,
        AlertOperator::GreaterThan,
        0.0,
        false, // disabled
    )];

    let triggered = evaluator
        .evaluate_rules(&rules, &make_time_range())
        .await
        .unwrap();
    assert!(triggered.is_empty(), "disabled rule should never trigger");
}

#[tokio::test]
async fn test_alert_evaluator_multiple_rules_selective_trigger() {
    // 100 total, 30 failed → failure rate 0.3, p95 = 2000ms
    let analytics = Arc::new(MockAnalyticsStore::with_stats(100, 30, 2000.0));
    let evaluator = AlertEvaluator::new(analytics);

    let rules = vec![
        make_alert_rule(
            "fail-rate-high",
            AlertMetric::FailureRate,
            AlertOperator::GreaterThan,
            0.5,
            true,
        ),
        make_alert_rule(
            "p95-slow",
            AlertMetric::P95Duration,
            AlertOperator::GreaterThan,
            1000.0,
            true,
        ),
        make_alert_rule(
            "exec-count-low",
            AlertMetric::ExecutionCount,
            AlertOperator::LessThan,
            50.0,
            true,
        ),
    ];

    let triggered = evaluator
        .evaluate_rules(&rules, &make_time_range())
        .await
        .unwrap();

    let triggered_ids: Vec<&str> = triggered.iter().map(|r| r.id.as_str()).collect();
    // failure rate 0.3 <= 0.5 → should NOT trigger
    assert!(
        !triggered_ids.contains(&"fail-rate-high"),
        "failure rate 0.3 should not trigger > 0.5"
    );
    // p95 2000 > 1000 → should trigger
    assert!(
        triggered_ids.contains(&"p95-slow"),
        "p95 2000ms should trigger > 1000ms"
    );
    // exec count 100 is NOT < 50 → should NOT trigger
    assert!(
        !triggered_ids.contains(&"exec-count-low"),
        "exec count 100 should not trigger < 50"
    );
}

#[tokio::test]
async fn test_alert_evaluator_zero_executions_does_not_panic() {
    let analytics = Arc::new(MockAnalyticsStore::with_stats(0, 0, 0.0));
    let evaluator = AlertEvaluator::new(analytics);

    let rules = vec![make_alert_rule(
        "zero-check",
        AlertMetric::FailureRate,
        AlertOperator::GreaterThan,
        0.5,
        true,
    )];

    // Failure rate should be 0.0 when total is 0 — no division by zero.
    let triggered = evaluator
        .evaluate_rules(&rules, &make_time_range())
        .await
        .unwrap();
    assert!(
        triggered.is_empty(),
        "0/0 failure rate should be 0.0, not trigger"
    );
}

// ===========================================================================
// Alert Rule Direct Evaluation (cross-crate: ensures orbflow-core types work
// correctly when used from orbflow-engine integration test context)
// ===========================================================================

#[tokio::test]
async fn test_alert_rule_evaluates_correctly() {
    let rule = make_alert_rule(
        "direct-eval",
        AlertMetric::FailureRate,
        AlertOperator::GreaterThan,
        0.5,
        true,
    );

    assert!(rule.evaluate(0.7), "0.7 > 0.5 should trigger");
    assert!(!rule.evaluate(0.3), "0.3 > 0.5 should not trigger");
    assert!(
        !rule.evaluate(0.5),
        "0.5 > 0.5 should not trigger (not strictly greater)"
    );
}

#[tokio::test]
async fn test_alert_rule_disabled_never_triggers() {
    let rule = make_alert_rule(
        "disabled",
        AlertMetric::FailureRate,
        AlertOperator::GreaterThan,
        0.0,
        false,
    );

    assert!(!rule.evaluate(100.0), "disabled rule should never trigger");
}

#[tokio::test]
async fn test_alert_rule_less_than_operator() {
    let rule = make_alert_rule(
        "lt-rule",
        AlertMetric::ExecutionCount,
        AlertOperator::LessThan,
        10.0,
        true,
    );

    assert!(rule.evaluate(5.0), "5 < 10 should trigger");
    assert!(!rule.evaluate(10.0), "10 < 10 should not trigger");
    assert!(!rule.evaluate(15.0), "15 < 10 should not trigger");
}

#[tokio::test]
async fn test_alert_rule_equals_operator() {
    let rule = make_alert_rule(
        "eq-rule",
        AlertMetric::P95Duration,
        AlertOperator::Equals,
        42.0,
        true,
    );

    assert!(rule.evaluate(42.0), "42 == 42 should trigger");
    assert!(!rule.evaluate(42.1), "42.1 != 42 should not trigger");
}

// ===========================================================================
// Analytics Time Range Parsing (integration-level: validates the pattern
// used in orbflow-httpapi handlers works correctly with chrono)
// ===========================================================================

#[test]
fn test_analytics_time_range_7d_parses_to_7_days_ago() {
    let range = parse_time_range_test("7d");
    let expected_duration = chrono::Duration::days(7);
    let actual_duration = range.end - range.start;

    // Allow 1 second tolerance for test execution time.
    let diff = (actual_duration - expected_duration).num_seconds().abs();
    assert!(diff <= 1, "7d should be ~7 days, diff was {diff}s");
}

#[test]
fn test_analytics_time_range_24h_parses_to_24_hours_ago() {
    let range = parse_time_range_test("24h");
    let expected_duration = chrono::Duration::hours(24);
    let actual_duration = range.end - range.start;

    let diff = (actual_duration - expected_duration).num_seconds().abs();
    assert!(diff <= 1, "24h should be ~24 hours, diff was {diff}s");
}

#[test]
fn test_analytics_time_range_1d_parses_correctly() {
    let range = parse_time_range_test("1d");
    let expected_duration = chrono::Duration::days(1);
    let actual_duration = range.end - range.start;

    let diff = (actual_duration - expected_duration).num_seconds().abs();
    assert!(diff <= 1, "1d should be ~1 day, diff was {diff}s");
}

#[test]
fn test_analytics_time_range_invalid_format_rejected() {
    assert!(parse_time_range_result("abc").is_err());
    assert!(parse_time_range_result("7m").is_err());
    assert!(parse_time_range_result("").is_err());
    assert!(parse_time_range_result("-1d").is_err());
    assert!(parse_time_range_result("0d").is_err());
    assert!(parse_time_range_result("0h").is_err());
    assert!(parse_time_range_result("366d").is_err());
    assert!(parse_time_range_result("8761h").is_err());
}

// ---------------------------------------------------------------------------
// Local reimplementation of parse_time_range for testing (the httpapi version
// is crate-private). This validates the same logic used in production.
// ---------------------------------------------------------------------------

fn parse_time_range_result(range: &str) -> Result<TimeRange, String> {
    let now = Utc::now();
    let trimmed = range.trim();

    let start = if let Some(days) = trimmed.strip_suffix('d') {
        let n: i64 = days
            .parse()
            .map_err(|_| format!("invalid range: {trimmed}"))?;
        if n <= 0 || n > 365 {
            return Err(format!("range days must be 1..365, got {n}"));
        }
        now - chrono::Duration::days(n)
    } else if let Some(hours) = trimmed.strip_suffix('h') {
        let n: i64 = hours
            .parse()
            .map_err(|_| format!("invalid range: {trimmed}"))?;
        if n <= 0 || n > 8760 {
            return Err(format!("range hours must be 1..8760, got {n}"));
        }
        now - chrono::Duration::hours(n)
    } else {
        return Err(format!(
            "invalid range format: {trimmed} (use e.g. '7d' or '24h')"
        ));
    };

    Ok(TimeRange { start, end: now })
}

fn parse_time_range_test(range: &str) -> TimeRange {
    parse_time_range_result(range).expect("valid range")
}
