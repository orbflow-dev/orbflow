// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Domain types for the native workflow testing framework.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::workflow::WorkflowId;

/// Matcher type for assertion evaluation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MatcherType {
    /// Exact JSON value match.
    Equals,
    /// String contains substring.
    Contains,
    /// Numeric comparison: actual > expected.
    GreaterThan,
    /// Numeric comparison: actual < expected.
    LessThan,
    /// Field exists and is not null.
    Exists,
    /// Field does not exist or is null.
    NotExists,
    /// Regex pattern match on string value.
    Regex,
    /// Checks JSON type (string, number, boolean, object, array).
    TypeOf,
}

/// A single assertion on a node's output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestAssertion {
    /// Dot-separated path like "body.status".
    pub field_path: String,
    /// The matcher to apply.
    pub matcher: MatcherType,
    /// Expected value (`None` for `Exists`/`NotExists`).
    pub expected: Option<Value>,
    /// Custom failure message.
    pub message: Option<String>,
}

/// A single test for one node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCase {
    /// Human-readable test name.
    pub name: String,
    /// ID of the node under test.
    pub node_id: String,
    /// Override node inputs for this test.
    pub input_overrides: Option<HashMap<String, Value>>,
    /// Assertions to evaluate against the node output.
    pub assertions: Vec<TestAssertion>,
}

/// Collection of test cases for a workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestSuite {
    /// Suite name.
    pub name: String,
    /// Workflow under test.
    pub workflow_id: WorkflowId,
    /// Optional description.
    pub description: Option<String>,
    /// Test cases in this suite.
    pub cases: Vec<TestCase>,
    /// When this suite was created.
    pub created_at: DateTime<Utc>,
}

/// Result of evaluating one assertion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssertionResult {
    /// Whether the assertion passed.
    pub passed: bool,
    /// The field path that was checked.
    pub field_path: String,
    /// The matcher that was applied.
    pub matcher: MatcherType,
    /// The expected value.
    pub expected: Option<Value>,
    /// The actual value found at `field_path`.
    pub actual: Option<Value>,
    /// Human-readable message (custom or generated).
    pub message: Option<String>,
}

/// Result of running one test case.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCaseResult {
    /// Test case name.
    pub name: String,
    /// Node that was tested.
    pub node_id: String,
    /// Whether all assertions passed and no execution error occurred.
    pub passed: bool,
    /// Individual assertion results.
    pub assertions: Vec<AssertionResult>,
    /// Execution error if the node failed to run.
    pub error: Option<String>,
    /// Wall-clock duration in milliseconds.
    pub duration_ms: u64,
}

/// Result of running an entire test suite.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestSuiteResult {
    /// Name of the suite that was run.
    pub suite_name: String,
    /// Workflow that was tested.
    pub workflow_id: WorkflowId,
    /// Total number of test cases.
    pub total: usize,
    /// Number of passing test cases.
    pub passed: usize,
    /// Number of failing test cases.
    pub failed: usize,
    /// Per-case results.
    pub results: Vec<TestCaseResult>,
    /// Total wall-clock duration in milliseconds.
    pub duration_ms: u64,
    /// When the suite was executed.
    pub run_at: DateTime<Utc>,
}

/// Coverage report showing which nodes are exercised by tests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageReport {
    /// Workflow analysed.
    pub workflow_id: WorkflowId,
    /// Total nodes in the workflow.
    pub total_nodes: usize,
    /// Nodes covered by at least one test case.
    pub tested_nodes: usize,
    /// Coverage percentage (0.0–100.0).
    pub coverage_pct: f64,
    /// Node IDs that have no test coverage.
    pub untested_nodes: Vec<String>,
}

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

/// Converts the flat `input_overrides` map into the nested structure expected
/// by [`Engine::test_node`].
///
/// Keys are treated as node IDs. Object values are flattened into field maps;
/// non-object values are wrapped as `{"value": v}`.
pub fn build_test_cached_outputs(
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

// ---------------------------------------------------------------------------
// Assertion evaluation
// ---------------------------------------------------------------------------

/// Navigate a dot-separated field path into a JSON value tree.
fn resolve_field_path(output: &HashMap<String, Value>, path: &str) -> Option<Value> {
    let segments: Vec<&str> = path.split('.').collect();
    if segments.is_empty() {
        return None;
    }

    let mut current: &Value = output.get(segments[0])?;
    for segment in &segments[1..] {
        current = current.get(*segment)?;
    }
    Some(current.clone())
}

/// Evaluate a single [`TestAssertion`] against the actual node output.
///
/// The function navigates `field_path` through `output`, applies the
/// [`MatcherType`], and returns an [`AssertionResult`].
pub fn evaluate_assertion(
    assertion: &TestAssertion,
    output: &HashMap<String, Value>,
) -> AssertionResult {
    let actual = resolve_field_path(output, &assertion.field_path);

    let passed = match assertion.matcher {
        MatcherType::Exists => actual.is_some(),
        MatcherType::NotExists => actual.is_none(),
        MatcherType::Equals => match (&actual, &assertion.expected) {
            (Some(a), Some(e)) => a == e,
            _ => false,
        },
        MatcherType::Contains => match (&actual, &assertion.expected) {
            (Some(Value::String(haystack)), Some(Value::String(needle))) => {
                haystack.contains(needle.as_str())
            }
            _ => false,
        },
        MatcherType::GreaterThan => match (&actual, &assertion.expected) {
            (Some(a), Some(e)) => a.as_f64().zip(e.as_f64()).is_some_and(|(a, e)| a > e),
            _ => false,
        },
        MatcherType::LessThan => match (&actual, &assertion.expected) {
            (Some(a), Some(e)) => a.as_f64().zip(e.as_f64()).is_some_and(|(a, e)| a < e),
            _ => false,
        },
        MatcherType::Regex => match (&actual, &assertion.expected) {
            (Some(Value::String(text)), Some(Value::String(pattern))) => {
                if pattern.len() > 512 {
                    return AssertionResult {
                        passed: false,
                        field_path: assertion.field_path.clone(),
                        matcher: assertion.matcher.clone(),
                        expected: assertion.expected.clone(),
                        actual: actual.clone(),
                        message: Some("regex pattern exceeds 512 byte limit".to_string()),
                    };
                }
                match regex::RegexBuilder::new(pattern)
                    .size_limit(1_000_000)
                    .dfa_size_limit(1_000_000)
                    .build()
                {
                    Ok(re) => re.is_match(text),
                    Err(e) => {
                        return AssertionResult {
                            passed: false,
                            field_path: assertion.field_path.clone(),
                            matcher: assertion.matcher.clone(),
                            expected: assertion.expected.clone(),
                            actual: actual.clone(),
                            message: Some(format!("invalid regex pattern: {e}")),
                        };
                    }
                }
            }
            _ => false,
        },
        MatcherType::TypeOf => match (&actual, &assertion.expected) {
            (Some(val), Some(Value::String(expected_type))) => {
                let actual_type = match val {
                    Value::String(_) => "string",
                    Value::Number(_) => "number",
                    Value::Bool(_) => "boolean",
                    Value::Object(_) => "object",
                    Value::Array(_) => "array",
                    Value::Null => "null",
                };
                actual_type == expected_type.as_str()
            }
            _ => false,
        },
    };

    AssertionResult {
        passed,
        field_path: assertion.field_path.clone(),
        matcher: assertion.matcher.clone(),
        expected: assertion.expected.clone(),
        actual,
        message: assertion.message.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_output(pairs: Vec<(&str, Value)>) -> HashMap<String, Value> {
        pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
    }

    #[test]
    fn test_equals_pass() {
        let assertion = TestAssertion {
            field_path: "status".into(),
            matcher: MatcherType::Equals,
            expected: Some(json!(200)),
            message: None,
        };
        let output = make_output(vec![("status", json!(200))]);
        let result = evaluate_assertion(&assertion, &output);
        assert!(result.passed);
    }

    #[test]
    fn test_equals_fail() {
        let assertion = TestAssertion {
            field_path: "status".into(),
            matcher: MatcherType::Equals,
            expected: Some(json!(200)),
            message: None,
        };
        let output = make_output(vec![("status", json!(404))]);
        let result = evaluate_assertion(&assertion, &output);
        assert!(!result.passed);
    }

    #[test]
    fn test_contains_pass() {
        let assertion = TestAssertion {
            field_path: "body".into(),
            matcher: MatcherType::Contains,
            expected: Some(json!("hello")),
            message: None,
        };
        let output = make_output(vec![("body", json!("say hello world"))]);
        let result = evaluate_assertion(&assertion, &output);
        assert!(result.passed);
    }

    #[test]
    fn test_greater_than() {
        let assertion = TestAssertion {
            field_path: "count".into(),
            matcher: MatcherType::GreaterThan,
            expected: Some(json!(5)),
            message: None,
        };
        let output = make_output(vec![("count", json!(10))]);
        let result = evaluate_assertion(&assertion, &output);
        assert!(result.passed);
    }

    #[test]
    fn test_less_than() {
        let assertion = TestAssertion {
            field_path: "count".into(),
            matcher: MatcherType::LessThan,
            expected: Some(json!(10)),
            message: None,
        };
        let output = make_output(vec![("count", json!(3))]);
        let result = evaluate_assertion(&assertion, &output);
        assert!(result.passed);
    }

    #[test]
    fn test_exists() {
        let assertion = TestAssertion {
            field_path: "data".into(),
            matcher: MatcherType::Exists,
            expected: None,
            message: None,
        };
        let output = make_output(vec![("data", json!({"key": "val"}))]);
        let result = evaluate_assertion(&assertion, &output);
        assert!(result.passed);
    }

    #[test]
    fn test_not_exists() {
        let assertion = TestAssertion {
            field_path: "missing".into(),
            matcher: MatcherType::NotExists,
            expected: None,
            message: None,
        };
        let output = make_output(vec![("other", json!(1))]);
        let result = evaluate_assertion(&assertion, &output);
        assert!(result.passed);
    }

    #[test]
    fn test_regex_pass() {
        let assertion = TestAssertion {
            field_path: "email".into(),
            matcher: MatcherType::Regex,
            expected: Some(json!(r"^[^@]+@[^@]+\.[^@]+$")),
            message: None,
        };
        let output = make_output(vec![("email", json!("user@example.com"))]);
        let result = evaluate_assertion(&assertion, &output);
        assert!(result.passed);
    }

    #[test]
    fn test_typeof_string() {
        let assertion = TestAssertion {
            field_path: "name".into(),
            matcher: MatcherType::TypeOf,
            expected: Some(json!("string")),
            message: None,
        };
        let output = make_output(vec![("name", json!("Alice"))]);
        let result = evaluate_assertion(&assertion, &output);
        assert!(result.passed);
    }

    #[test]
    fn test_regex_invalid_pattern() {
        let mut output = HashMap::new();
        output.insert("name".into(), json!("hello"));
        let assertion = TestAssertion {
            field_path: "name".into(),
            matcher: MatcherType::Regex,
            expected: Some(json!("(invalid[[")),
            message: None,
        };
        let result = evaluate_assertion(&assertion, &output);
        assert!(!result.passed);
        assert!(result.message.unwrap().contains("invalid regex"));
    }

    #[test]
    fn test_nested_field_path() {
        let assertion = TestAssertion {
            field_path: "body.status".into(),
            matcher: MatcherType::Equals,
            expected: Some(json!("ok")),
            message: None,
        };
        let output = make_output(vec![("body", json!({"status": "ok"}))]);
        let result = evaluate_assertion(&assertion, &output);
        assert!(result.passed);
    }
}
