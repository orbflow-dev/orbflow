// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! OpenTelemetry metrics recorder for Orbflow.
//!
//! Wraps the OpenTelemetry meter API to provide type-safe metric recording
//! using the constants defined in [`crate::telemetry`]. Uses the global
//! meter provider — call `opentelemetry::global::set_meter_provider()`
//! before constructing a [`MetricsRecorder`].

use opentelemetry::KeyValue;
use opentelemetry::metrics::{Counter, Histogram, UpDownCounter};

use crate::telemetry::*;

/// Pre-allocated OpenTelemetry instruments for recording Orbflow metrics.
///
/// Construct once at startup and share via `Arc<MetricsRecorder>`. All methods
/// are `&self` and thread-safe.
#[derive(Clone)]
pub struct MetricsRecorder {
    workflows_started: Counter<u64>,
    workflows_completed: Counter<u64>,
    workflows_failed: Counter<u64>,
    nodes_completed: Counter<u64>,
    nodes_failed: Counter<u64>,
    node_duration: Histogram<f64>,
    workflow_duration: Histogram<f64>,
    llm_tokens: Counter<u64>,
    cost_usd: Counter<u64>,
    running_instances: UpDownCounter<i64>,
}

impl MetricsRecorder {
    /// Creates a new recorder using the global meter provider.
    pub fn new() -> Self {
        let meter = opentelemetry::global::meter("orbflow");

        Self {
            workflows_started: meter
                .u64_counter(METRIC_WORKFLOWS_STARTED)
                .with_description("Total workflow instances started")
                .build(),
            workflows_completed: meter
                .u64_counter(METRIC_WORKFLOWS_COMPLETED)
                .with_description("Total workflow instances completed (success)")
                .build(),
            workflows_failed: meter
                .u64_counter(METRIC_WORKFLOWS_FAILED)
                .with_description("Total workflow instances failed")
                .build(),
            nodes_completed: meter
                .u64_counter(METRIC_NODES_COMPLETED)
                .with_description("Total node executions completed (success)")
                .build(),
            nodes_failed: meter
                .u64_counter(METRIC_NODES_FAILED)
                .with_description("Total node executions failed")
                .build(),
            node_duration: meter
                .f64_histogram(METRIC_NODE_DURATION)
                .with_description("Node execution duration in seconds")
                .with_unit("s")
                .build(),
            workflow_duration: meter
                .f64_histogram(METRIC_WORKFLOW_DURATION)
                .with_description("Workflow instance duration in seconds")
                .with_unit("s")
                .build(),
            llm_tokens: meter
                .u64_counter(METRIC_LLM_TOKENS)
                .with_description("Total LLM tokens consumed")
                .build(),
            cost_usd: meter
                .u64_counter(METRIC_COST_USD)
                .with_description("Total estimated cost in USD (scaled by 10000)")
                .build(),
            running_instances: meter
                .i64_up_down_counter(METRIC_RUNNING_INSTANCES)
                .with_description("Currently running workflow instances")
                .build(),
        }
    }

    /// Records a workflow start.
    pub fn record_workflow_started(&self, workflow_id: &str) {
        self.workflows_started.add(
            1,
            &[KeyValue::new(ATTR_WORKFLOW_ID, workflow_id.to_string())],
        );
        self.running_instances.add(
            1,
            &[KeyValue::new(ATTR_WORKFLOW_ID, workflow_id.to_string())],
        );
    }

    /// Records a workflow completion (success).
    pub fn record_workflow_completed(&self, workflow_id: &str, duration_secs: f64) {
        let attrs = [KeyValue::new(ATTR_WORKFLOW_ID, workflow_id.to_string())];
        self.workflows_completed.add(1, &attrs);
        self.workflow_duration.record(duration_secs, &attrs);
        self.running_instances.add(-1, &attrs);
    }

    /// Records a workflow failure.
    pub fn record_workflow_failed(&self, workflow_id: &str, duration_secs: f64) {
        let attrs = [KeyValue::new(ATTR_WORKFLOW_ID, workflow_id.to_string())];
        self.workflows_failed.add(1, &attrs);
        self.workflow_duration.record(duration_secs, &attrs);
        self.running_instances.add(-1, &attrs);
    }

    /// Records a node execution success.
    pub fn record_node_completed(
        &self,
        workflow_id: &str,
        node_id: &str,
        plugin_ref: &str,
        duration_secs: f64,
    ) {
        let attrs = [
            KeyValue::new(ATTR_WORKFLOW_ID, workflow_id.to_string()),
            KeyValue::new(ATTR_NODE_ID, node_id.to_string()),
            KeyValue::new(ATTR_PLUGIN_REF, plugin_ref.to_string()),
        ];
        self.nodes_completed.add(1, &attrs);
        self.node_duration.record(duration_secs, &attrs);
    }

    /// Records a node execution failure.
    pub fn record_node_failed(
        &self,
        workflow_id: &str,
        node_id: &str,
        plugin_ref: &str,
        duration_secs: f64,
    ) {
        let attrs = [
            KeyValue::new(ATTR_WORKFLOW_ID, workflow_id.to_string()),
            KeyValue::new(ATTR_NODE_ID, node_id.to_string()),
            KeyValue::new(ATTR_PLUGIN_REF, plugin_ref.to_string()),
        ];
        self.nodes_failed.add(1, &attrs);
        self.node_duration.record(duration_secs, &attrs);
    }

    /// Records LLM token usage.
    pub fn record_llm_tokens(&self, tokens: u64, provider: &str, model: &str) {
        let attrs = [
            KeyValue::new(ATTR_AI_PROVIDER, provider.to_string()),
            KeyValue::new(ATTR_AI_MODEL, model.to_string()),
        ];
        self.llm_tokens.add(tokens, &attrs);
    }

    /// Records estimated cost (scaled by 10000 for integer precision).
    pub fn record_cost(&self, cost_scaled: u64, workflow_id: &str) {
        self.cost_usd.add(
            cost_scaled,
            &[KeyValue::new(ATTR_WORKFLOW_ID, workflow_id.to_string())],
        );
    }
}

impl Default for MetricsRecorder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_recorder_creation() {
        // Uses the noop global meter (no provider set) — should not panic.
        let recorder = MetricsRecorder::new();
        recorder.record_workflow_started("wf-1");
        recorder.record_workflow_completed("wf-1", 1.5);
        recorder.record_node_completed("wf-1", "node-1", "builtin:http", 0.3);
        recorder.record_node_failed("wf-1", "node-2", "builtin:email", 0.1);
        recorder.record_llm_tokens(150, "openai", "gpt-4");
        recorder.record_cost(500, "wf-1");
        recorder.record_workflow_failed("wf-2", 2.0);
    }
}
