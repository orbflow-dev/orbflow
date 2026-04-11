// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! OpenTelemetry metric names and span conventions for Orbflow.
//!
//! Defines standard metric names and attribute keys used across all crates.
//! The actual OpenTelemetry SDK initialization lives in `orbflow-config` (tracing
//! setup) — this module provides the constants and helpers for instrumenting
//! the engine, worker, and API.

/// Metric namespace prefix.
pub const METRIC_PREFIX: &str = "orbflow";

// ─── Metric Names ───────────────────────────────────────────────────────────

/// Counter: total workflow instances started.
pub const METRIC_WORKFLOWS_STARTED: &str = "orbflow.workflow.started_total";

/// Counter: total workflow instances completed (success).
pub const METRIC_WORKFLOWS_COMPLETED: &str = "orbflow.workflow.completed_total";

/// Counter: total workflow instances failed.
pub const METRIC_WORKFLOWS_FAILED: &str = "orbflow.workflow.failed_total";

/// Counter: total node executions completed (success).
pub const METRIC_NODES_COMPLETED: &str = "orbflow.node.completed_total";

/// Counter: total node executions failed.
pub const METRIC_NODES_FAILED: &str = "orbflow.node.failed_total";

/// Histogram: node execution duration in seconds.
pub const METRIC_NODE_DURATION: &str = "orbflow.node.duration_seconds";

/// Histogram: workflow instance duration in seconds.
pub const METRIC_WORKFLOW_DURATION: &str = "orbflow.workflow.duration_seconds";

/// Counter: total LLM tokens consumed.
pub const METRIC_LLM_TOKENS: &str = "orbflow.llm.tokens_total";

/// Counter: total estimated cost in USD (scaled by 10000 for integer precision).
pub const METRIC_COST_USD: &str = "orbflow.cost.usd_total";

/// Gauge: currently running instances.
pub const METRIC_RUNNING_INSTANCES: &str = "orbflow.instances.running";

// ─── Attribute Keys ─────────────────────────────────────────────────────────

/// Workflow ID attribute.
pub const ATTR_WORKFLOW_ID: &str = "orbflow.workflow.id";

/// Instance ID attribute.
pub const ATTR_INSTANCE_ID: &str = "orbflow.instance.id";

/// Node ID attribute.
pub const ATTR_NODE_ID: &str = "orbflow.node.id";

/// Plugin reference attribute (e.g., "builtin:http").
pub const ATTR_PLUGIN_REF: &str = "orbflow.plugin_ref";

/// Node status attribute.
pub const ATTR_STATUS: &str = "orbflow.status";

/// Worker pool name attribute.
pub const ATTR_POOL: &str = "orbflow.pool";

/// AI provider attribute.
pub const ATTR_AI_PROVIDER: &str = "orbflow.ai.provider";

/// AI model attribute.
pub const ATTR_AI_MODEL: &str = "orbflow.ai.model";

// ─── Span Names ─────────────────────────────────────────────────────────────

/// Span for the full lifecycle of a workflow instance.
pub const SPAN_WORKFLOW_EXECUTE: &str = "orbflow.workflow.execute";

/// Span for a single node execution.
pub const SPAN_NODE_EXECUTE: &str = "orbflow.node.execute";

/// Span for the engine's DAG evaluation cycle.
pub const SPAN_DAG_EVALUATE: &str = "orbflow.engine.evaluate_dag";

/// Span for the worker's task handling.
pub const SPAN_WORKER_HANDLE_TASK: &str = "orbflow.worker.handle_task";

/// Span for CEL expression evaluation.
pub const SPAN_CEL_EVALUATE: &str = "orbflow.cel.evaluate";

/// Span for a bus publish operation.
pub const SPAN_BUS_PUBLISH: &str = "orbflow.bus.publish";

/// Span for an HTTP API request.
pub const SPAN_HTTP_REQUEST: &str = "orbflow.http.request";

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Creates a set of standard tracing span fields for a node execution.
///
/// Usage with the `tracing` crate:
/// ```ignore
/// let span = tracing::info_span!(
///     SPAN_NODE_EXECUTE,
///     %instance_id,
///     %node_id,
///     plugin_ref = %plugin_ref,
/// );
/// ```
///
/// This function is a convenience for building structured log fields
/// consistent with OpenTelemetry semantic conventions.
pub fn node_span_fields(
    instance_id: &str,
    node_id: &str,
    plugin_ref: &str,
) -> Vec<(&'static str, String)> {
    vec![
        (ATTR_INSTANCE_ID, instance_id.to_string()),
        (ATTR_NODE_ID, node_id.to_string()),
        (ATTR_PLUGIN_REF, plugin_ref.to_string()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metric_names_have_prefix() {
        assert!(METRIC_WORKFLOWS_STARTED.starts_with(METRIC_PREFIX));
        assert!(METRIC_NODES_COMPLETED.starts_with(METRIC_PREFIX));
        assert!(METRIC_NODE_DURATION.starts_with(METRIC_PREFIX));
        assert!(METRIC_LLM_TOKENS.starts_with(METRIC_PREFIX));
    }

    #[test]
    fn test_node_span_fields() {
        let fields = node_span_fields("inst-1", "node-2", "builtin:http");
        assert_eq!(fields.len(), 3);
        assert_eq!(fields[0], (ATTR_INSTANCE_ID, "inst-1".to_string()));
        assert_eq!(fields[1], (ATTR_NODE_ID, "node-2".to_string()));
        assert_eq!(fields[2], (ATTR_PLUGIN_REF, "builtin:http".to_string()));
    }
}
