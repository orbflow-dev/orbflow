// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Test runner: executes test cases against registered node executors.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use crate::assertions::evaluate_with_error;
use crate::types::{NodeTestCase, TestOutcome, TestReport, TestSuiteConfig};
use orbflow_core::execution::InstanceId;
use orbflow_core::ports::{NodeExecutor, NodeInput};

/// Runs node test cases against registered executors.
pub struct TestRunner {
    executors: HashMap<String, Arc<dyn NodeExecutor>>,
}

impl TestRunner {
    /// Creates a new test runner with no executors registered.
    pub fn new() -> Self {
        Self {
            executors: HashMap::new(),
        }
    }

    /// Registers a node executor under the given plugin_ref.
    pub fn register(
        &mut self,
        plugin_ref: impl Into<String>,
        executor: Arc<dyn NodeExecutor>,
    ) -> &mut Self {
        self.executors.insert(plugin_ref.into(), executor);
        self
    }

    /// Runs a single test case and returns the outcome.
    pub async fn run_test(&self, test: &NodeTestCase) -> TestOutcome {
        let name = if test.name.is_empty() {
            format!("test:{}", test.plugin_ref)
        } else {
            test.name.clone()
        };

        let start = Instant::now();

        let input = NodeInput {
            instance_id: InstanceId::new(format!("test-{}", uuid_stub())),
            node_id: "test-node".into(),
            plugin_ref: test.plugin_ref.clone(),
            config: Some(test.config.clone()),
            input: if test.input.is_empty() {
                None
            } else {
                Some(test.input.clone())
            },
            parameters: if test.parameters.is_empty() {
                None
            } else {
                Some(test.parameters.clone())
            },
            capabilities: None,
            attempt: 0,
        };

        let executor = match self.executors.get(&test.plugin_ref) {
            Some(e) => e,
            None => {
                return TestOutcome {
                    name,
                    passed: false,
                    assertion_results: vec![],
                    output: None,
                    error: Some(format!("no executor registered for '{}'", test.plugin_ref)),
                    duration_ms: start.elapsed().as_millis() as u64,
                };
            }
        };

        let timeout = tokio::time::Duration::from_millis(test.timeout_ms);
        let exec_result = tokio::time::timeout(timeout, executor.execute(&input)).await;

        let duration_ms = start.elapsed().as_millis() as u64;

        // Unpack timeout → executor result → node output.
        let (output_data, output_error) = match exec_result {
            Err(_) => {
                // Timeout expired.
                return TestOutcome {
                    name,
                    passed: test.expect_error,
                    assertion_results: vec![],
                    output: None,
                    error: Some(format!("test timed out after {}ms", test.timeout_ms)),
                    duration_ms,
                };
            }
            Ok(Err(e)) => {
                // Executor returned Err(OrbflowError).
                let error_msg = e.to_string();
                if test.expect_error {
                    let error_match = test
                        .expected_error_contains
                        .as_ref()
                        .is_none_or(|expected| error_msg.contains(expected.as_str()));
                    return TestOutcome {
                        name,
                        passed: error_match,
                        assertion_results: vec![],
                        output: None,
                        error: Some(error_msg),
                        duration_ms,
                    };
                }
                return TestOutcome {
                    name,
                    passed: false,
                    assertion_results: vec![],
                    output: None,
                    error: Some(error_msg),
                    duration_ms,
                };
            }
            Ok(Ok(output)) => {
                // Executor returned Ok(NodeOutput).
                (output.data.clone(), output.error.clone())
            }
        };

        // Check if node returned a business error.
        if let Some(ref err_msg) = output_error {
            if test.expect_error {
                let error_match = test
                    .expected_error_contains
                    .as_ref()
                    .is_none_or(|expected| err_msg.contains(expected.as_str()));
                return TestOutcome {
                    name,
                    passed: error_match,
                    assertion_results: vec![],
                    output: output_data,
                    error: Some(err_msg.clone()),
                    duration_ms,
                };
            }
            return TestOutcome {
                name,
                passed: false,
                assertion_results: vec![],
                output: output_data,
                error: Some(err_msg.clone()),
                duration_ms,
            };
        }

        if test.expect_error {
            return TestOutcome {
                name,
                passed: false,
                assertion_results: vec![],
                output: output_data,
                error: Some("expected error but node succeeded".into()),
                duration_ms,
            };
        }

        // Evaluate assertions against output data, using error-aware evaluation.
        let assertion_results: Vec<_> = test
            .assertions
            .iter()
            .map(|a| evaluate_with_error(a, &output_data, &output_error))
            .collect();
        let all_passed = assertion_results.iter().all(|r| r.passed);

        TestOutcome {
            name,
            passed: all_passed,
            assertion_results,
            output: output_data.or_else(|| Some(HashMap::new())),
            error: None,
            duration_ms,
        }
    }

    /// Runs a full test suite and returns a summary report.
    pub async fn run_suite(&self, suite: &TestSuiteConfig) -> TestReport {
        let start = Instant::now();
        let mut outcomes = Vec::with_capacity(suite.tests.len());

        for test in &suite.tests {
            let outcome = self.run_test(test).await;
            outcomes.push(outcome);
        }

        let total = outcomes.len();
        let passed = outcomes.iter().filter(|o| o.passed).count();
        let failed = total - passed;

        TestReport {
            suite_name: suite.name.clone(),
            outcomes,
            total,
            passed,
            failed,
            duration_ms: start.elapsed().as_millis() as u64,
        }
    }

    /// Loads a test suite from a YAML string and runs it.
    pub async fn run_yaml(&self, yaml: &str) -> Result<TestReport, String> {
        let suite: TestSuiteConfig =
            serde_yaml::from_str(yaml).map_err(|e| format!("invalid test YAML: {e}"))?;
        Ok(self.run_suite(&suite).await)
    }

    /// Loads a test suite from a JSON string and runs it.
    pub async fn run_json(&self, json: &str) -> Result<TestReport, String> {
        let suite: TestSuiteConfig =
            serde_json::from_str(json).map_err(|e| format!("invalid test JSON: {e}"))?;
        Ok(self.run_suite(&suite).await)
    }
}

impl Default for TestRunner {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple counter-based ID for test instances (avoids uuid dependency).
fn uuid_stub() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    format!("{}", COUNTER.fetch_add(1, Ordering::Relaxed))
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use orbflow_core::{NodeOutput, OrbflowError};
    use serde_json::Value;

    /// A mock executor that echoes input as output.
    struct EchoExecutor;

    #[async_trait]
    impl NodeExecutor for EchoExecutor {
        async fn execute(&self, input: &NodeInput) -> Result<NodeOutput, OrbflowError> {
            // Merge config + input into output.
            let mut data: HashMap<String, Value> = HashMap::new();
            if let Some(ref cfg) = input.config {
                data.extend(cfg.clone());
            }
            if let Some(ref inp) = input.input {
                data.extend(inp.clone());
            }
            Ok(NodeOutput {
                data: Some(data),
                error: None,
            })
        }
    }

    /// A mock executor that always fails.
    struct FailExecutor;

    #[async_trait]
    impl NodeExecutor for FailExecutor {
        async fn execute(&self, _input: &NodeInput) -> Result<NodeOutput, OrbflowError> {
            Err(OrbflowError::InvalidNodeConfig(
                "intentional failure".into(),
            ))
        }
    }

    /// A mock executor that returns a business error.
    struct BizErrorExecutor;

    #[async_trait]
    impl NodeExecutor for BizErrorExecutor {
        async fn execute(&self, _input: &NodeInput) -> Result<NodeOutput, OrbflowError> {
            Ok(NodeOutput {
                data: None,
                error: Some("rate limited".into()),
            })
        }
    }

    #[tokio::test]
    async fn test_echo_pass() {
        let mut runner = TestRunner::new();
        runner.register("echo", Arc::new(EchoExecutor));

        let test = NodeTestCase::new("echo")
            .with_name("echo test")
            .with_config("greeting", serde_json::json!("hello"))
            .expect_output("greeting", serde_json::json!("hello"));

        let outcome = runner.run_test(&test).await;
        assert!(outcome.passed, "outcome: {:?}", outcome);
    }

    #[tokio::test]
    async fn test_echo_fail_wrong_value() {
        let mut runner = TestRunner::new();
        runner.register("echo", Arc::new(EchoExecutor));

        let test = NodeTestCase::new("echo")
            .with_config("greeting", serde_json::json!("hello"))
            .expect_output("greeting", serde_json::json!("world"));

        let outcome = runner.run_test(&test).await;
        assert!(!outcome.passed);
    }

    #[tokio::test]
    async fn test_expect_error_pass() {
        let mut runner = TestRunner::new();
        runner.register("fail", Arc::new(FailExecutor));

        let test = NodeTestCase::new("fail")
            .with_name("should fail")
            .should_fail_with("intentional");

        let outcome = runner.run_test(&test).await;
        assert!(outcome.passed, "outcome: {:?}", outcome);
    }

    #[tokio::test]
    async fn test_expect_error_wrong_message() {
        let mut runner = TestRunner::new();
        runner.register("fail", Arc::new(FailExecutor));

        let test = NodeTestCase::new("fail").should_fail_with("wrong message");

        let outcome = runner.run_test(&test).await;
        assert!(!outcome.passed);
    }

    #[tokio::test]
    async fn test_business_error() {
        let mut runner = TestRunner::new();
        runner.register("biz-err", Arc::new(BizErrorExecutor));

        let test = NodeTestCase::new("biz-err").should_fail_with("rate limited");

        let outcome = runner.run_test(&test).await;
        assert!(outcome.passed, "outcome: {:?}", outcome);
    }

    #[tokio::test]
    async fn test_missing_executor() {
        let runner = TestRunner::new();
        let test = NodeTestCase::new("nonexistent");

        let outcome = runner.run_test(&test).await;
        assert!(!outcome.passed);
        assert!(outcome.error.unwrap().contains("no executor registered"));
    }

    #[tokio::test]
    async fn test_suite_yaml() {
        let mut runner = TestRunner::new();
        runner.register("echo", Arc::new(EchoExecutor));

        let yaml = r#"
name: Echo Suite
tests:
  - name: basic echo
    plugin_ref: echo
    config:
      status: 200
    assertions:
      - type: equals
        field: status
        expected: 200
  - name: contains check
    plugin_ref: echo
    config:
      body: "Hello, world!"
    assertions:
      - type: contains
        field: body
        substring: "world"
"#;

        let report = runner.run_yaml(yaml).await.unwrap();
        assert_eq!(report.total, 2);
        assert_eq!(report.passed, 2);
        assert_eq!(report.failed, 0);
        assert!(report.all_passed());
    }

    #[tokio::test]
    async fn test_suite_mixed_results() {
        let mut runner = TestRunner::new();
        runner.register("echo", Arc::new(EchoExecutor));

        let suite = TestSuiteConfig {
            name: "Mixed Suite".into(),
            description: String::new(),
            tests: vec![
                NodeTestCase::new("echo")
                    .with_name("pass")
                    .with_config("x", serde_json::json!(1))
                    .expect_output("x", serde_json::json!(1)),
                NodeTestCase::new("echo")
                    .with_name("fail")
                    .with_config("x", serde_json::json!(1))
                    .expect_output("x", serde_json::json!(2)),
            ],
        };

        let report = runner.run_suite(&suite).await;
        assert_eq!(report.total, 2);
        assert_eq!(report.passed, 1);
        assert_eq!(report.failed, 1);
        assert!(!report.all_passed());
    }

    /// A mock executor that sleeps forever (for timeout testing).
    struct SlowExecutor;

    #[async_trait]
    impl NodeExecutor for SlowExecutor {
        async fn execute(&self, _input: &NodeInput) -> Result<NodeOutput, OrbflowError> {
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
            Ok(NodeOutput {
                data: None,
                error: None,
            })
        }
    }

    #[tokio::test]
    async fn test_timeout() {
        let mut runner = TestRunner::new();
        runner.register("slow", Arc::new(SlowExecutor));

        let test = NodeTestCase::new("slow")
            .with_name("should timeout")
            .with_timeout(50); // 50ms timeout

        let outcome = runner.run_test(&test).await;
        assert!(!outcome.passed);
        assert!(outcome.error.unwrap().contains("timed out"));
    }

    #[tokio::test]
    async fn test_timeout_expected_error() {
        let mut runner = TestRunner::new();
        runner.register("slow", Arc::new(SlowExecutor));

        let test = NodeTestCase::new("slow")
            .with_name("timeout as expected error")
            .with_timeout(50)
            .should_fail();

        let outcome = runner.run_test(&test).await;
        assert!(outcome.passed, "outcome: {:?}", outcome);
    }

    #[tokio::test]
    async fn test_succeeds_assertion() {
        let mut runner = TestRunner::new();
        runner.register("echo", Arc::new(EchoExecutor));

        let test = NodeTestCase {
            name: "succeeds check".into(),
            plugin_ref: "echo".into(),
            config: {
                let mut m = HashMap::new();
                m.insert("key".into(), serde_json::json!("value"));
                m
            },
            input: HashMap::new(),
            parameters: HashMap::new(),
            assertions: vec![crate::assertions::Assertion::Succeeds],
            expect_error: false,
            expected_error_contains: None,
            timeout_ms: 30000,
        };

        let outcome = runner.run_test(&test).await;
        assert!(outcome.passed, "outcome: {:?}", outcome);
        assert_eq!(outcome.assertion_results.len(), 1);
        assert!(outcome.assertion_results[0].passed);
    }

    #[tokio::test]
    async fn test_matches_assertion_via_runner() {
        let mut runner = TestRunner::new();
        runner.register("echo", Arc::new(EchoExecutor));

        let test = NodeTestCase {
            name: "matches check".into(),
            plugin_ref: "echo".into(),
            config: {
                let mut m = HashMap::new();
                m.insert("message".into(), serde_json::json!("Hello, world!"));
                m
            },
            input: HashMap::new(),
            parameters: HashMap::new(),
            assertions: vec![crate::assertions::Assertion::Matches {
                field: "message".into(),
                pattern: "world".into(),
            }],
            expect_error: false,
            expected_error_contains: None,
            timeout_ms: 30000,
        };

        let outcome = runner.run_test(&test).await;
        assert!(outcome.passed, "outcome: {:?}", outcome);
    }

    #[tokio::test]
    async fn test_fails_assertion_on_success() {
        let mut runner = TestRunner::new();
        runner.register("echo", Arc::new(EchoExecutor));

        // Assertion::Fails should not pass when node succeeds.
        let test = NodeTestCase {
            name: "fails assertion on success".into(),
            plugin_ref: "echo".into(),
            config: HashMap::new(),
            input: HashMap::new(),
            parameters: HashMap::new(),
            assertions: vec![crate::assertions::Assertion::Fails],
            expect_error: false,
            expected_error_contains: None,
            timeout_ms: 30000,
        };

        let outcome = runner.run_test(&test).await;
        assert!(!outcome.passed);
    }
}
