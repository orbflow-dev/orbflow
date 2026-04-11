// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Per-execution resource metering and cost tracking.
//!
//! Captures CPU time, wall time, API call counts, LLM token usage, and
//! estimated USD cost for each node execution. Aggregated into per-instance
//! metrics for dashboards and budget enforcement.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Resource metrics collected for a single node execution.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NodeMetrics {
    /// Wall-clock execution time in milliseconds.
    pub wall_time_ms: u64,
    /// CPU time in milliseconds (if available from the runtime).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_time_ms: Option<u64>,
    /// Number of external API calls made by this node.
    #[serde(default)]
    pub api_calls: u32,
    /// LLM token usage (if this was an AI node).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens: Option<TokenMetrics>,
    /// Estimated cost in USD for this node execution.
    #[serde(default)]
    pub cost_usd: f64,
    /// Bytes transferred (request + response payloads).
    #[serde(default)]
    pub bytes_transferred: u64,
}

/// LLM token usage metrics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenMetrics {
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
}

/// Aggregated metrics for an entire workflow instance execution.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InstanceMetrics {
    /// Total estimated cost in USD across all nodes.
    pub total_cost_usd: f64,
    /// Total wall-clock time in milliseconds (instance start to end).
    pub total_wall_time_ms: u64,
    /// Total LLM tokens consumed across all AI nodes.
    pub total_tokens: i64,
    /// Total external API calls across all nodes.
    pub total_api_calls: u32,
    /// Total bytes transferred across all nodes.
    pub total_bytes_transferred: u64,
    /// Per-node metrics keyed by node_id.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub node_metrics: HashMap<String, NodeMetrics>,
}

impl InstanceMetrics {
    /// Creates a new empty instance metrics.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records metrics for a node execution and updates aggregates.
    pub fn record_node(&mut self, node_id: impl Into<String>, metrics: NodeMetrics) {
        self.total_cost_usd += metrics.cost_usd;
        self.total_wall_time_ms += metrics.wall_time_ms;
        self.total_api_calls += metrics.api_calls;
        self.total_bytes_transferred += metrics.bytes_transferred;

        if let Some(ref tokens) = metrics.tokens {
            self.total_tokens += tokens.total_tokens;
        }

        self.node_metrics.insert(node_id.into(), metrics);
    }

    /// Returns the total number of nodes that have been metered.
    pub fn node_count(&self) -> usize {
        self.node_metrics.len()
    }
}

/// Budget configuration for cost enforcement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Budget {
    /// Maximum USD spend per workflow execution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit_usd: Option<f64>,
    /// Maximum total tokens per workflow execution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit_tokens: Option<i64>,
    /// Maximum wall-clock time in milliseconds per workflow execution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit_wall_time_ms: Option<u64>,
}

impl Budget {
    /// Checks if the given metrics exceed any budget limit.
    ///
    /// Returns `Some(reason)` if a limit is exceeded, `None` otherwise.
    pub fn check(&self, metrics: &InstanceMetrics) -> Option<String> {
        if let Some(limit) = self.limit_usd
            && metrics.total_cost_usd > limit
        {
            return Some(format!(
                "cost budget exceeded: ${:.4} > ${:.4}",
                metrics.total_cost_usd, limit
            ));
        }

        if let Some(limit) = self.limit_tokens
            && metrics.total_tokens > limit
        {
            return Some(format!(
                "token budget exceeded: {} > {}",
                metrics.total_tokens, limit
            ));
        }

        if let Some(limit) = self.limit_wall_time_ms
            && metrics.total_wall_time_ms > limit
        {
            return Some(format!(
                "time budget exceeded: {}ms > {}ms",
                metrics.total_wall_time_ms, limit
            ));
        }

        None
    }
}

/// Persistent budget configuration for cost enforcement across workflow executions.
///
/// Unlike [`Budget`] which tracks per-execution limits, `AccountBudget` represents
/// an organizational budget that accumulates costs over a time period and resets
/// automatically (daily, weekly, or monthly).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountBudget {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team: Option<String>,
    pub period: BudgetPeriod,
    pub limit_usd: f64,
    pub current_usd: f64,
    pub reset_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

/// Time period for budget resets.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BudgetPeriod {
    Daily,
    Weekly,
    Monthly,
}

/// Extracts node metrics from a node output's data map.
///
/// Looks for standard fields like `cost_usd`, `usage.total_tokens`, etc.
/// that AI nodes and HTTP nodes produce.
pub fn extract_metrics_from_output(
    output: &HashMap<String, serde_json::Value>,
    wall_time_ms: u64,
) -> NodeMetrics {
    let cost_usd = output
        .get("cost_usd")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    let tokens = output.get("usage").and_then(|usage| {
        let prompt = usage.get("prompt_tokens")?.as_i64()?;
        let completion = usage.get("completion_tokens")?.as_i64()?;
        let total = usage.get("total_tokens")?.as_i64()?;
        Some(TokenMetrics {
            prompt_tokens: prompt,
            completion_tokens: completion,
            total_tokens: total,
        })
    });

    let bytes_transferred = output
        .get("body")
        .map(|v| v.to_string().len() as u64)
        .unwrap_or(0);

    let api_calls = if output.contains_key("status") || output.contains_key("content") {
        1
    } else {
        0
    };

    NodeMetrics {
        wall_time_ms,
        cpu_time_ms: None,
        api_calls,
        tokens,
        cost_usd,
        bytes_transferred,
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_instance_metrics_aggregate() {
        let mut im = InstanceMetrics::new();

        im.record_node(
            "http-1",
            NodeMetrics {
                wall_time_ms: 100,
                api_calls: 1,
                cost_usd: 0.0,
                bytes_transferred: 1024,
                ..Default::default()
            },
        );

        im.record_node(
            "ai-chat-1",
            NodeMetrics {
                wall_time_ms: 500,
                api_calls: 1,
                cost_usd: 0.003,
                tokens: Some(TokenMetrics {
                    prompt_tokens: 100,
                    completion_tokens: 200,
                    total_tokens: 300,
                }),
                bytes_transferred: 2048,
                ..Default::default()
            },
        );

        assert_eq!(im.node_count(), 2);
        assert_eq!(im.total_wall_time_ms, 600);
        assert_eq!(im.total_api_calls, 2);
        assert_eq!(im.total_tokens, 300);
        assert!((im.total_cost_usd - 0.003).abs() < 1e-9);
        assert_eq!(im.total_bytes_transferred, 3072);
    }

    #[test]
    fn test_budget_within_limits() {
        let budget = Budget {
            limit_usd: Some(1.0),
            limit_tokens: Some(10_000),
            limit_wall_time_ms: None,
        };
        let metrics = InstanceMetrics {
            total_cost_usd: 0.5,
            total_tokens: 5000,
            ..Default::default()
        };
        assert!(budget.check(&metrics).is_none());
    }

    #[test]
    fn test_budget_cost_exceeded() {
        let budget = Budget {
            limit_usd: Some(1.0),
            limit_tokens: None,
            limit_wall_time_ms: None,
        };
        let metrics = InstanceMetrics {
            total_cost_usd: 1.5,
            ..Default::default()
        };
        let reason = budget.check(&metrics).unwrap();
        assert!(reason.contains("cost budget exceeded"));
    }

    #[test]
    fn test_budget_token_exceeded() {
        let budget = Budget {
            limit_usd: None,
            limit_tokens: Some(1000),
            limit_wall_time_ms: None,
        };
        let metrics = InstanceMetrics {
            total_tokens: 2000,
            ..Default::default()
        };
        let reason = budget.check(&metrics).unwrap();
        assert!(reason.contains("token budget exceeded"));
    }

    #[test]
    fn test_extract_metrics_from_ai_output() {
        let mut output = HashMap::new();
        output.insert("content".into(), serde_json::json!("Hello world"));
        output.insert("cost_usd".into(), serde_json::json!(0.005));
        output.insert(
            "usage".into(),
            serde_json::json!({
                "prompt_tokens": 50,
                "completion_tokens": 100,
                "total_tokens": 150,
            }),
        );

        let metrics = extract_metrics_from_output(&output, 250);
        assert_eq!(metrics.wall_time_ms, 250);
        assert!((metrics.cost_usd - 0.005).abs() < 1e-9);
        assert_eq!(metrics.api_calls, 1);
        let tokens = metrics.tokens.unwrap();
        assert_eq!(tokens.prompt_tokens, 50);
        assert_eq!(tokens.completion_tokens, 100);
        assert_eq!(tokens.total_tokens, 150);
    }

    #[test]
    fn test_extract_metrics_from_empty_output() {
        let output = HashMap::new();
        let metrics = extract_metrics_from_output(&output, 100);
        assert_eq!(metrics.wall_time_ms, 100);
        assert!((metrics.cost_usd - 0.0).abs() < f64::EPSILON);
        assert!(metrics.tokens.is_none());
        assert_eq!(metrics.api_calls, 0);
    }

    #[test]
    fn test_metrics_serde_roundtrip() {
        let mut im = InstanceMetrics::new();
        im.record_node(
            "test",
            NodeMetrics {
                wall_time_ms: 100,
                cost_usd: 0.01,
                ..Default::default()
            },
        );
        let json = serde_json::to_string(&im).unwrap();
        let im2: InstanceMetrics = serde_json::from_str(&json).unwrap();
        assert_eq!(im2.total_cost_usd, im.total_cost_usd);
        assert_eq!(im2.node_count(), 1);
    }
}
