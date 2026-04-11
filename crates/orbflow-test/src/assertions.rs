// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Declarative assertions for node output validation.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A declarative assertion against a node's output data.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Assertion {
    /// Assert the node completed successfully (no error).
    Succeeds,
    /// Assert the node failed with an error.
    Fails,
    /// output[field] == expected (deep equality).
    Equals { field: String, expected: Value },
    /// output[field] contains substring (string fields only).
    Contains { field: String, substring: String },
    /// output[field] exists and is not null.
    Exists { field: String },
    /// output[field] matches the given JSON type ("string", "number", "boolean", "object", "array").
    TypeIs {
        field: String,
        expected_type: String,
    },
    /// output[field] > threshold (numeric fields only).
    GreaterThan { field: String, threshold: f64 },
    /// output[field] < threshold (numeric fields only).
    LessThan { field: String, threshold: f64 },
    /// output[field] value (as string) contains the given pattern substring.
    Matches { field: String, pattern: String },
}

/// The result of evaluating a single assertion.
#[derive(Debug, Clone, Serialize)]
pub struct AssertionResult {
    /// The assertion that was evaluated.
    pub assertion: String,
    /// Whether the assertion passed.
    pub passed: bool,
    /// Human-readable explanation of the result.
    pub message: String,
}

/// Navigate a dot-separated path into a JSON value map.
///
/// For example, `get_nested(map, "response.headers.content_type")` will:
/// 1. Look up `"response"` in the map
/// 2. Navigate into `"headers"` within that value
/// 3. Navigate into `"content_type"` within that value
pub fn get_nested(map: &HashMap<String, Value>, path: &str) -> Option<Value> {
    let parts: Vec<&str> = path.split('.').collect();
    if parts.is_empty() {
        return None;
    }
    let mut current: &Value = map.get(parts[0])?;
    for part in &parts[1..] {
        current = current.get(*part)?;
    }
    Some(current.clone())
}

/// Evaluate an assertion against a node's output, taking both data and error into account.
///
/// This is useful for `Succeeds` and `Fails` assertions that need to inspect
/// whether the node produced an error.
pub fn evaluate_with_error(
    assertion: &Assertion,
    output: &Option<HashMap<String, Value>>,
    error: &Option<String>,
) -> AssertionResult {
    match assertion {
        Assertion::Succeeds => {
            let passed = error.is_none();
            AssertionResult {
                assertion: "succeeds".into(),
                passed,
                message: if passed {
                    "node succeeded".into()
                } else {
                    format!(
                        "expected success, got error: {}",
                        error.as_deref().unwrap_or("unknown")
                    )
                },
            }
        }
        Assertion::Fails => {
            let passed = error.is_some();
            AssertionResult {
                assertion: "fails".into(),
                passed,
                message: if passed {
                    "node failed as expected".into()
                } else {
                    "expected failure, but node succeeded".into()
                },
            }
        }
        // For all other assertions, delegate to the data-only evaluate method.
        other => {
            let data = output.clone().unwrap_or_default();
            other.evaluate(&data)
        }
    }
}

impl Assertion {
    /// Evaluates this assertion against the given output data.
    ///
    /// For `Succeeds` / `Fails` assertions that require error context, use
    /// [`evaluate_with_error()`] instead.
    pub fn evaluate(&self, output: &HashMap<String, Value>) -> AssertionResult {
        match self {
            Assertion::Succeeds | Assertion::Fails => AssertionResult {
                assertion: if matches!(self, Assertion::Succeeds) {
                    "succeeds"
                } else {
                    "fails"
                }
                .to_string(),
                passed: false,
                message:
                    "cannot evaluate without error context — use evaluate_with_error() instead"
                        .into(),
            },

            Assertion::Equals { field, expected } => {
                let actual = get_nested(output, field);
                let passed = actual.as_ref() == Some(expected);
                AssertionResult {
                    assertion: format!("{field} == {expected}"),
                    passed,
                    message: if passed {
                        "matched".into()
                    } else {
                        format!(
                            "expected {expected}, got {}",
                            actual.map_or("(missing)".into(), |v| v.to_string())
                        )
                    },
                }
            }

            Assertion::Contains { field, substring } => {
                let actual = get_nested(output, field);
                let passed = actual
                    .as_ref()
                    .and_then(|v| v.as_str())
                    .is_some_and(|s| s.contains(substring.as_str()));
                AssertionResult {
                    assertion: format!("{field} contains \"{substring}\""),
                    passed,
                    message: if passed {
                        "matched".into()
                    } else {
                        format!(
                            "field {} does not contain \"{substring}\"",
                            actual.map_or("(missing)".into(), |v| v.to_string())
                        )
                    },
                }
            }

            Assertion::Exists { field } => {
                let actual = get_nested(output, field);
                let passed = actual.as_ref().is_some_and(|v| !v.is_null());
                AssertionResult {
                    assertion: format!("{field} exists"),
                    passed,
                    message: if passed {
                        "exists".into()
                    } else {
                        "field is missing or null".into()
                    },
                }
            }

            Assertion::TypeIs {
                field,
                expected_type,
            } => {
                let actual = get_nested(output, field);
                let actual_type = actual.as_ref().map(json_type_name);
                let passed = actual_type.as_deref() == Some(expected_type.as_str());
                AssertionResult {
                    assertion: format!("{field} is {expected_type}"),
                    passed,
                    message: if passed {
                        "type matched".into()
                    } else {
                        format!(
                            "expected type {expected_type}, got {}",
                            actual_type.unwrap_or_else(|| "(missing)".into())
                        )
                    },
                }
            }

            Assertion::GreaterThan { field, threshold } => {
                let actual = get_nested(output, field).and_then(|v| v.as_f64());
                let passed = actual.is_some_and(|n| n > *threshold);
                AssertionResult {
                    assertion: format!("{field} > {threshold}"),
                    passed,
                    message: if passed {
                        "passed".into()
                    } else {
                        format!(
                            "expected > {threshold}, got {}",
                            actual.map_or("(missing/non-numeric)".into(), |n| n.to_string())
                        )
                    },
                }
            }

            Assertion::LessThan { field, threshold } => {
                let actual = get_nested(output, field).and_then(|v| v.as_f64());
                let passed = actual.is_some_and(|n| n < *threshold);
                AssertionResult {
                    assertion: format!("{field} < {threshold}"),
                    passed,
                    message: if passed {
                        "passed".into()
                    } else {
                        format!(
                            "expected < {threshold}, got {}",
                            actual.map_or("(missing/non-numeric)".into(), |n| n.to_string())
                        )
                    },
                }
            }

            Assertion::Matches { field, pattern } => {
                let actual = get_nested(output, field);
                let passed = actual
                    .as_ref()
                    .and_then(|v| v.as_str())
                    .is_some_and(|s| s.contains(pattern.as_str()));
                AssertionResult {
                    assertion: format!("{field} matches \"{pattern}\""),
                    passed,
                    message: if passed {
                        "matched".into()
                    } else {
                        format!(
                            "field {} does not match pattern \"{pattern}\"",
                            actual.map_or("(missing)".into(), |v| v.to_string())
                        )
                    },
                }
            }
        }
    }
}

/// Returns the JSON type name for a value.
fn json_type_name(v: &Value) -> String {
    match v {
        Value::Null => "null".into(),
        Value::Bool(_) => "boolean".into(),
        Value::Number(_) => "number".into(),
        Value::String(_) => "string".into(),
        Value::Array(_) => "array".into(),
        Value::Object(_) => "object".into(),
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_output() -> HashMap<String, Value> {
        let mut m = HashMap::new();
        m.insert("status".into(), serde_json::json!(200));
        m.insert("body".into(), serde_json::json!("Hello, world!"));
        m.insert("count".into(), serde_json::json!(42));
        m.insert(
            "headers".into(),
            serde_json::json!({"content-type": "application/json"}),
        );
        m
    }

    #[test]
    fn test_equals_pass() {
        let a = Assertion::Equals {
            field: "status".into(),
            expected: serde_json::json!(200),
        };
        let r = a.evaluate(&sample_output());
        assert!(r.passed);
    }

    #[test]
    fn test_equals_fail() {
        let a = Assertion::Equals {
            field: "status".into(),
            expected: serde_json::json!(404),
        };
        let r = a.evaluate(&sample_output());
        assert!(!r.passed);
    }

    #[test]
    fn test_contains_pass() {
        let a = Assertion::Contains {
            field: "body".into(),
            substring: "world".into(),
        };
        let r = a.evaluate(&sample_output());
        assert!(r.passed);
    }

    #[test]
    fn test_contains_fail() {
        let a = Assertion::Contains {
            field: "body".into(),
            substring: "missing".into(),
        };
        let r = a.evaluate(&sample_output());
        assert!(!r.passed);
    }

    #[test]
    fn test_exists_pass() {
        let a = Assertion::Exists {
            field: "status".into(),
        };
        let r = a.evaluate(&sample_output());
        assert!(r.passed);
    }

    #[test]
    fn test_exists_missing() {
        let a = Assertion::Exists {
            field: "nonexistent".into(),
        };
        let r = a.evaluate(&sample_output());
        assert!(!r.passed);
    }

    #[test]
    fn test_type_is() {
        let a = Assertion::TypeIs {
            field: "status".into(),
            expected_type: "number".into(),
        };
        assert!(a.evaluate(&sample_output()).passed);

        let a = Assertion::TypeIs {
            field: "body".into(),
            expected_type: "string".into(),
        };
        assert!(a.evaluate(&sample_output()).passed);

        let a = Assertion::TypeIs {
            field: "headers".into(),
            expected_type: "object".into(),
        };
        assert!(a.evaluate(&sample_output()).passed);
    }

    #[test]
    fn test_greater_than() {
        let a = Assertion::GreaterThan {
            field: "count".into(),
            threshold: 40.0,
        };
        assert!(a.evaluate(&sample_output()).passed);

        let a = Assertion::GreaterThan {
            field: "count".into(),
            threshold: 50.0,
        };
        assert!(!a.evaluate(&sample_output()).passed);
    }

    #[test]
    fn test_less_than() {
        let a = Assertion::LessThan {
            field: "count".into(),
            threshold: 50.0,
        };
        assert!(a.evaluate(&sample_output()).passed);

        let a = Assertion::LessThan {
            field: "count".into(),
            threshold: 10.0,
        };
        assert!(!a.evaluate(&sample_output()).passed);
    }

    #[test]
    fn test_assertion_serde_roundtrip() {
        let a = Assertion::Equals {
            field: "status".into(),
            expected: serde_json::json!(200),
        };
        let json = serde_json::to_string(&a).unwrap();
        let a2: Assertion = serde_json::from_str(&json).unwrap();
        assert!(matches!(a2, Assertion::Equals { .. }));
    }

    #[test]
    fn test_matches_pass() {
        let a = Assertion::Matches {
            field: "body".into(),
            pattern: "Hello".into(),
        };
        let r = a.evaluate(&sample_output());
        assert!(r.passed);
    }

    #[test]
    fn test_matches_fail() {
        let a = Assertion::Matches {
            field: "body".into(),
            pattern: "Goodbye".into(),
        };
        let r = a.evaluate(&sample_output());
        assert!(!r.passed);
    }

    #[test]
    fn test_matches_missing_field() {
        let a = Assertion::Matches {
            field: "nonexistent".into(),
            pattern: "anything".into(),
        };
        let r = a.evaluate(&sample_output());
        assert!(!r.passed);
    }

    #[test]
    fn test_succeeds_with_error_context() {
        let a = Assertion::Succeeds;
        let data = Some(sample_output());

        // No error → succeeds passes.
        let r = evaluate_with_error(&a, &data, &None);
        assert!(r.passed);
        assert_eq!(r.message, "node succeeded");

        // With error → succeeds fails.
        let r = evaluate_with_error(&a, &data, &Some("boom".into()));
        assert!(!r.passed);
        assert!(r.message.contains("boom"));
    }

    #[test]
    fn test_fails_with_error_context() {
        let a = Assertion::Fails;
        let data: Option<HashMap<String, Value>> = None;

        // With error → fails passes.
        let r = evaluate_with_error(&a, &data, &Some("something broke".into()));
        assert!(r.passed);
        assert_eq!(r.message, "node failed as expected");

        // No error → fails does not pass.
        let r = evaluate_with_error(&a, &Some(sample_output()), &None);
        assert!(!r.passed);
        assert_eq!(r.message, "expected failure, but node succeeded");
    }

    #[test]
    fn test_evaluate_with_error_delegates_data_assertions() {
        let a = Assertion::Equals {
            field: "status".into(),
            expected: serde_json::json!(200),
        };
        let data = Some(sample_output());
        let r = evaluate_with_error(&a, &data, &None);
        assert!(r.passed);
    }

    #[test]
    fn test_get_nested_flat() {
        let map = sample_output();
        let val = get_nested(&map, "status");
        assert_eq!(val, Some(serde_json::json!(200)));
    }

    #[test]
    fn test_get_nested_deep() {
        let map = sample_output();
        let val = get_nested(&map, "headers.content-type");
        assert_eq!(val, Some(serde_json::json!("application/json")));
    }

    #[test]
    fn test_get_nested_missing() {
        let map = sample_output();
        assert_eq!(get_nested(&map, "headers.x-missing"), None);
        assert_eq!(get_nested(&map, "nonexistent.deep.path"), None);
    }

    #[test]
    fn test_get_nested_empty_path() {
        let map = sample_output();
        assert_eq!(get_nested(&map, ""), None);
    }

    #[test]
    fn test_succeeds_serde_roundtrip() {
        let a = Assertion::Succeeds;
        let json = serde_json::to_string(&a).unwrap();
        let a2: Assertion = serde_json::from_str(&json).unwrap();
        assert!(matches!(a2, Assertion::Succeeds));
    }

    #[test]
    fn test_fails_serde_roundtrip() {
        let a = Assertion::Fails;
        let json = serde_json::to_string(&a).unwrap();
        let a2: Assertion = serde_json::from_str(&json).unwrap();
        assert!(matches!(a2, Assertion::Fails));
    }

    #[test]
    fn test_matches_serde_roundtrip() {
        let a = Assertion::Matches {
            field: "body".into(),
            pattern: "hello".into(),
        };
        let json = serde_json::to_string(&a).unwrap();
        let a2: Assertion = serde_json::from_str(&json).unwrap();
        assert!(matches!(a2, Assertion::Matches { .. }));
    }
}
