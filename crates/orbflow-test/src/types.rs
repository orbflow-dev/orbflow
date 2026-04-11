// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Test case definitions and result types.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::assertions::Assertion;

/// A single node test case: execute one node with given inputs, assert outputs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeTestCase {
    /// Human-readable name for this test case.
    #[serde(default)]
    pub name: String,
    /// The plugin_ref of the node to test (e.g. "builtin:http").
    pub plugin_ref: String,
    /// Config values to pass to the node.
    #[serde(default)]
    pub config: HashMap<String, Value>,
    /// Input values to pass to the node.
    #[serde(default)]
    pub input: HashMap<String, Value>,
    /// Parameter values to pass to the node.
    #[serde(default)]
    pub parameters: HashMap<String, Value>,
    /// Assertions to check against the node output.
    #[serde(default)]
    pub assertions: Vec<Assertion>,
    /// If true, the node is expected to fail (error output or Err result).
    #[serde(default)]
    pub expect_error: bool,
    /// Optional expected error message substring.
    #[serde(default)]
    pub expected_error_contains: Option<String>,
    /// Timeout in milliseconds (default: 30000).
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

fn default_timeout() -> u64 {
    30000
}

impl NodeTestCase {
    /// Creates a new test case for the given plugin_ref.
    pub fn new(plugin_ref: impl Into<String>) -> Self {
        Self {
            name: String::new(),
            plugin_ref: plugin_ref.into(),
            config: HashMap::new(),
            input: HashMap::new(),
            parameters: HashMap::new(),
            assertions: Vec::new(),
            expect_error: false,
            expected_error_contains: None,
            timeout_ms: default_timeout(),
        }
    }

    /// Sets the test case name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Adds a config key-value pair.
    pub fn with_config(mut self, key: impl Into<String>, value: Value) -> Self {
        self.config.insert(key.into(), value);
        self
    }

    /// Adds an input key-value pair.
    pub fn with_input(mut self, key: impl Into<String>, value: Value) -> Self {
        self.input.insert(key.into(), value);
        self
    }

    /// Adds a parameter key-value pair.
    pub fn with_param(mut self, key: impl Into<String>, value: Value) -> Self {
        self.parameters.insert(key.into(), value);
        self
    }

    /// Adds an assertion that output[key] equals expected value.
    pub fn expect_output(mut self, key: impl Into<String>, expected: Value) -> Self {
        self.assertions.push(Assertion::Equals {
            field: key.into(),
            expected,
        });
        self
    }

    /// Adds an assertion that output[key] contains the given substring.
    pub fn expect_contains(mut self, key: impl Into<String>, substring: impl Into<String>) -> Self {
        self.assertions.push(Assertion::Contains {
            field: key.into(),
            substring: substring.into(),
        });
        self
    }

    /// Adds an assertion that output[key] exists and is not null.
    pub fn expect_exists(mut self, key: impl Into<String>) -> Self {
        self.assertions
            .push(Assertion::Exists { field: key.into() });
        self
    }

    /// Marks this test as expecting an error.
    pub fn should_fail(mut self) -> Self {
        self.expect_error = true;
        self
    }

    /// Marks this test as expecting an error containing the given substring.
    pub fn should_fail_with(mut self, msg: impl Into<String>) -> Self {
        self.expect_error = true;
        self.expected_error_contains = Some(msg.into());
        self
    }

    /// Sets the timeout in milliseconds for this test case.
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }
}

/// The outcome of a single test case execution.
#[derive(Debug, Clone, Serialize)]
pub struct TestOutcome {
    /// Test case name.
    pub name: String,
    /// Whether the test passed all assertions.
    pub passed: bool,
    /// Individual assertion results.
    pub assertion_results: Vec<crate::assertions::AssertionResult>,
    /// The actual output data (if node succeeded).
    pub output: Option<HashMap<String, Value>>,
    /// Error message (if node failed).
    pub error: Option<String>,
    /// Execution duration in milliseconds.
    pub duration_ms: u64,
}

/// A test suite loaded from a YAML/JSON file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestSuiteConfig {
    /// Suite name.
    pub name: String,
    /// Optional description of the suite.
    #[serde(default)]
    pub description: String,
    /// List of node test cases.
    #[serde(default)]
    pub tests: Vec<NodeTestCase>,
}

/// Summary report for a test suite run.
#[derive(Debug, Clone, Serialize)]
pub struct TestReport {
    /// Suite name.
    pub suite_name: String,
    /// Individual test outcomes.
    pub outcomes: Vec<TestOutcome>,
    /// Total tests run.
    pub total: usize,
    /// Tests passed.
    pub passed: usize,
    /// Tests failed.
    pub failed: usize,
    /// Total duration in milliseconds.
    pub duration_ms: u64,
}

impl TestReport {
    /// Returns true if all tests passed.
    pub fn all_passed(&self) -> bool {
        self.failed == 0
    }
}
