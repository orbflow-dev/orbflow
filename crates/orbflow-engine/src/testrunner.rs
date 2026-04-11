// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Test runner for executing workflow test suites.
//!
//! [`TestRunner`] wraps an [`Engine`] and drives [`TestSuite`] execution:
//! for each [`TestCase`] it calls `Engine::test_node`, evaluates the
//! assertions against the node output, and collects per-case results into
//! a [`TestSuiteResult`]. It can also compute [`CoverageReport`]s showing
//! which workflow nodes lack test coverage.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use serde_json::Value;

use orbflow_core::error::OrbflowError;
use orbflow_core::ports::Engine;
use orbflow_core::testing::*;
use orbflow_core::workflow::WorkflowId;

/// Executes [`TestSuite`]s against a workflow engine.
pub struct TestRunner {
    engine: Arc<dyn Engine>,
}

impl TestRunner {
    /// Creates a new runner backed by the given engine.
    pub fn new(engine: Arc<dyn Engine>) -> Self {
        Self { engine }
    }

    /// Runs every [`TestCase`] in the suite and returns aggregate results.
    pub async fn run_suite(&self, suite: &TestSuite) -> Result<TestSuiteResult, OrbflowError> {
        let start = Instant::now();
        let mut results = Vec::with_capacity(suite.cases.len());

        for case in &suite.cases {
            let case_result = self.run_case(&suite.workflow_id, case).await;
            results.push(case_result);
        }

        let passed = results.iter().filter(|r| r.passed).count();
        let failed = results.len() - passed;

        Ok(TestSuiteResult {
            suite_name: suite.name.clone(),
            workflow_id: suite.workflow_id.clone(),
            total: results.len(),
            passed,
            failed,
            results,
            duration_ms: start.elapsed().as_millis() as u64,
            run_at: chrono::Utc::now(),
        })
    }

    /// Runs a single [`TestCase`] and evaluates its assertions.
    async fn run_case(&self, workflow_id: &WorkflowId, case: &TestCase) -> TestCaseResult {
        let start = Instant::now();

        // Convert flat input_overrides (node_id → Value) into the nested
        // HashMap<String, HashMap<String, Value>> that Engine::test_node expects.
        // Each value is expected to be a JSON object representing that node's
        // output fields; non-object values are wrapped as `{"value": v}`.
        let cached_outputs = build_cached_outputs(case.input_overrides.as_ref());

        match self
            .engine
            .test_node(workflow_id, &case.node_id, cached_outputs)
            .await
        {
            Ok(result) => {
                // Extract the target node's output map (field_name → value).
                let output: HashMap<String, Value> = result
                    .node_outputs
                    .get(&case.node_id)
                    .and_then(|ns| ns.output.clone())
                    .unwrap_or_default();

                // Evaluate every assertion against the output.
                let assertion_results: Vec<AssertionResult> = case
                    .assertions
                    .iter()
                    .map(|a| evaluate_assertion(a, &output))
                    .collect();

                let all_passed = assertion_results.iter().all(|r| r.passed);

                TestCaseResult {
                    name: case.name.clone(),
                    node_id: case.node_id.clone(),
                    passed: all_passed,
                    assertions: assertion_results,
                    error: None,
                    duration_ms: start.elapsed().as_millis() as u64,
                }
            }
            Err(e) => TestCaseResult {
                name: case.name.clone(),
                node_id: case.node_id.clone(),
                passed: false,
                assertions: Vec::new(),
                error: Some(e.to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
            },
        }
    }

    /// Computes a [`CoverageReport`] showing which nodes are exercised by at
    /// least one test case in the suite.
    pub async fn compute_coverage(
        &self,
        workflow_id: &WorkflowId,
        suite: &TestSuite,
    ) -> Result<CoverageReport, OrbflowError> {
        let workflow = self.engine.get_workflow(workflow_id).await?;

        let all_node_ids: Vec<String> = workflow.nodes.iter().map(|n| n.id.clone()).collect();
        let tested_ids: HashSet<&str> = suite.cases.iter().map(|c| c.node_id.as_str()).collect();

        let untested: Vec<String> = all_node_ids
            .iter()
            .filter(|id| !tested_ids.contains(id.as_str()))
            .cloned()
            .collect();

        let total = all_node_ids.len();
        let tested = total - untested.len();
        let pct = if total > 0 {
            (tested as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        Ok(CoverageReport {
            workflow_id: workflow_id.clone(),
            total_nodes: total,
            tested_nodes: tested,
            coverage_pct: pct,
            untested_nodes: untested,
        })
    }
}

/// Converts the flat `input_overrides` map into the nested structure expected
/// by `Engine::test_node`.
///
/// Keys are treated as node IDs. Object values are flattened into field maps;
/// non-object values are wrapped as `{"value": v}` so every entry is a valid
/// `HashMap<String, Value>`.
fn build_cached_outputs(
    overrides: Option<&HashMap<String, Value>>,
) -> HashMap<String, HashMap<String, Value>> {
    let Some(map) = overrides else {
        return HashMap::new();
    };

    map.iter()
        .map(|(node_id, val)| {
            let fields = match val {
                Value::Object(obj) => obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
                other => {
                    let mut m = HashMap::with_capacity(1);
                    m.insert("value".to_owned(), other.clone());
                    m
                }
            };
            (node_id.clone(), fields)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn build_cached_outputs_none() {
        let result = build_cached_outputs(None);
        assert!(result.is_empty());
    }

    #[test]
    fn build_cached_outputs_object_values() {
        let mut overrides = HashMap::new();
        overrides.insert("node_1".to_owned(), json!({"status": 200, "body": "ok"}));
        let result = build_cached_outputs(Some(&overrides));

        assert_eq!(result.len(), 1);
        let node1 = result.get("node_1").unwrap();
        assert_eq!(node1.get("status"), Some(&json!(200)));
        assert_eq!(node1.get("body"), Some(&json!("ok")));
    }

    #[test]
    fn build_cached_outputs_scalar_wrapped() {
        let mut overrides = HashMap::new();
        overrides.insert("node_2".to_owned(), json!(42));
        let result = build_cached_outputs(Some(&overrides));

        let node2 = result.get("node_2").unwrap();
        assert_eq!(node2.get("value"), Some(&json!(42)));
    }
}
