// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Alert rule types for threshold-based monitoring of workflow metrics.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A rule that triggers an alert when a metric crosses a threshold.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRule {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow_id: Option<String>,
    pub metric: AlertMetric,
    pub operator: AlertOperator,
    pub threshold: f64,
    pub channel: AlertChannel,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
}

/// The metric an alert rule monitors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertMetric {
    FailureRate,
    P95Duration,
    ExecutionCount,
}

/// Comparison operator for threshold evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertOperator {
    GreaterThan,
    LessThan,
    Equals,
}

/// Notification channel for triggered alerts.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum AlertChannel {
    Webhook { url: String },
    Log,
}

impl AlertRule {
    /// Evaluates whether the current value triggers this alert rule.
    pub fn evaluate(&self, current_value: f64) -> bool {
        if !self.enabled {
            return false;
        }
        match self.operator {
            AlertOperator::GreaterThan => current_value > self.threshold,
            AlertOperator::LessThan => current_value < self.threshold,
            AlertOperator::Equals => (current_value - self.threshold).abs() < f64::EPSILON,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_rule(operator: AlertOperator, threshold: f64, enabled: bool) -> AlertRule {
        AlertRule {
            id: "test-rule".to_string(),
            workflow_id: None,
            metric: AlertMetric::FailureRate,
            operator,
            threshold,
            channel: AlertChannel::Log,
            enabled,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn greater_than_triggers_when_above_threshold() {
        let rule = make_rule(AlertOperator::GreaterThan, 0.5, true);
        assert!(rule.evaluate(0.6));
        assert!(!rule.evaluate(0.5));
        assert!(!rule.evaluate(0.4));
    }

    #[test]
    fn less_than_triggers_when_below_threshold() {
        let rule = make_rule(AlertOperator::LessThan, 100.0, true);
        assert!(rule.evaluate(50.0));
        assert!(!rule.evaluate(100.0));
        assert!(!rule.evaluate(150.0));
    }

    #[test]
    fn equals_triggers_on_exact_match() {
        let rule = make_rule(AlertOperator::Equals, 42.0, true);
        assert!(rule.evaluate(42.0));
        assert!(!rule.evaluate(42.1));
        assert!(!rule.evaluate(41.9));
    }

    #[test]
    fn disabled_rule_never_triggers() {
        let rule = make_rule(AlertOperator::GreaterThan, 0.0, false);
        assert!(!rule.evaluate(100.0));
    }

    #[test]
    fn serialization_roundtrip() {
        let rule = make_rule(AlertOperator::GreaterThan, 0.5, true);
        let json = serde_json::to_string(&rule).unwrap();
        let deserialized: AlertRule = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, rule.id);
        assert_eq!(deserialized.operator, rule.operator);
    }

    #[test]
    fn webhook_channel_serialization() {
        let rule = AlertRule {
            id: "wh-rule".to_string(),
            workflow_id: Some("wf-1".to_string()),
            metric: AlertMetric::P95Duration,
            operator: AlertOperator::GreaterThan,
            threshold: 5000.0,
            channel: AlertChannel::Webhook {
                url: "https://example.com/hook".to_string(),
            },
            enabled: true,
            created_at: Utc::now(),
        };
        let json = serde_json::to_string(&rule).unwrap();
        assert!(json.contains("webhook"));
        assert!(json.contains("https://example.com/hook"));
    }
}
