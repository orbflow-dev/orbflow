// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Integration tests for builtin nodes using the `orbflow-test` framework.
//!
//! These tests exercise actual node executors (not mocks) through the
//! `TestRunner` harness, validating config handling, output shape, and
//! error paths.

use std::sync::Arc;

use orbflow_builtins::delay::DelayNode;
use orbflow_builtins::encode::EncodeNode;
use orbflow_builtins::filter::FilterNode;
use orbflow_builtins::log::LogNode;
use orbflow_builtins::sort::SortNode;
use orbflow_builtins::template::TemplateNode;
use orbflow_builtins::transform::TransformNode;
use orbflow_test::{NodeTestCase, TestRunner};
use serde_json::json;

/// Creates a `TestRunner` pre-loaded with all side-effect-free builtins.
fn runner() -> TestRunner {
    let mut r = TestRunner::new();
    r.register("builtin:log", Arc::new(LogNode));
    r.register("builtin:encode", Arc::new(EncodeNode));
    r.register("builtin:template", Arc::new(TemplateNode));
    r.register("builtin:delay", Arc::new(DelayNode));
    r.register("builtin:transform", Arc::new(TransformNode::new()));
    r.register("builtin:filter", Arc::new(FilterNode::new()));
    r.register("builtin:sort", Arc::new(SortNode));
    r
}

// ─── LogNode ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn log_node_echoes_message() {
    let r = runner();
    let test = NodeTestCase::new("builtin:log")
        .with_name("log echoes message")
        .with_config("message", json!("hello world"))
        .expect_output("message", json!("hello world"));
    let outcome = r.run_test(&test).await;
    assert!(outcome.passed, "outcome: {outcome:?}");
}

#[tokio::test]
async fn log_node_no_config_succeeds() {
    let r = runner();
    let test = NodeTestCase::new("builtin:log").with_name("log no config succeeds");
    let outcome = r.run_test(&test).await;
    assert!(outcome.passed, "outcome: {outcome:?}");
}

// ─── EncodeNode ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn encode_base64_roundtrip() {
    let r = runner();

    let encode = NodeTestCase::new("builtin:encode")
        .with_name("base64 encode")
        .with_config("input", json!("Hello, Orbflow!"))
        .with_config("operation", json!("base64-encode"))
        .expect_output("result", json!("SGVsbG8sIE9yYmZsb3ch"))
        .expect_output("operation", json!("base64-encode"));
    let outcome = r.run_test(&encode).await;
    assert!(outcome.passed, "encode: {outcome:?}");

    let decode = NodeTestCase::new("builtin:encode")
        .with_name("base64 decode")
        .with_config("input", json!("SGVsbG8sIE9yYmZsb3ch"))
        .with_config("operation", json!("base64-decode"))
        .expect_output("result", json!("Hello, Orbflow!"));
    let outcome = r.run_test(&decode).await;
    assert!(outcome.passed, "decode: {outcome:?}");
}

#[tokio::test]
async fn encode_sha256() {
    let r = runner();
    let test = NodeTestCase::new("builtin:encode")
        .with_name("sha256 hash")
        .with_config("input", json!("test"))
        .with_config("operation", json!("sha256"))
        .expect_exists("result")
        .expect_output("operation", json!("sha256"));
    let outcome = r.run_test(&test).await;
    assert!(outcome.passed, "outcome: {outcome:?}");
}

#[tokio::test]
async fn encode_md5() {
    let r = runner();
    let test = NodeTestCase::new("builtin:encode")
        .with_name("md5 hash")
        .with_config("input", json!("test"))
        .with_config("operation", json!("md5"))
        .expect_exists("result")
        .expect_output("operation", json!("md5"));
    let outcome = r.run_test(&test).await;
    assert!(outcome.passed, "outcome: {outcome:?}");
}

#[tokio::test]
async fn encode_url_roundtrip() {
    let r = runner();

    let encode = NodeTestCase::new("builtin:encode")
        .with_name("url encode")
        .with_config("input", json!("hello world&foo=bar"))
        .with_config("operation", json!("url-encode"))
        .expect_output("result", json!("hello+world%26foo%3Dbar"));
    let outcome = r.run_test(&encode).await;
    assert!(outcome.passed, "url-encode: {outcome:?}");

    let decode = NodeTestCase::new("builtin:encode")
        .with_name("url decode")
        .with_config("input", json!("hello+world%26foo%3Dbar"))
        .with_config("operation", json!("url-decode"))
        .expect_output("result", json!("hello world&foo=bar"));
    let outcome = r.run_test(&decode).await;
    assert!(outcome.passed, "url-decode: {outcome:?}");
}

#[tokio::test]
async fn encode_missing_config_fails() {
    let r = runner();
    let test = NodeTestCase::new("builtin:encode")
        .with_name("missing config")
        .should_fail_with("input and operation are required");
    let outcome = r.run_test(&test).await;
    assert!(outcome.passed, "outcome: {outcome:?}");
}

// ─── TemplateNode ────────────────────────────────────────────────────────────

#[tokio::test]
async fn template_renders_variables() {
    let r = runner();
    let test = NodeTestCase::new("builtin:template")
        .with_name("simple template")
        .with_config("template", json!("Hello, {{ name }}!"))
        .with_config("variables", json!({"name": "Orbflow"}))
        .expect_output("result", json!("Hello, Orbflow!"));
    let outcome = r.run_test(&test).await;
    assert!(outcome.passed, "outcome: {outcome:?}");
}

#[tokio::test]
async fn template_go_style_syntax() {
    let r = runner();
    let test = NodeTestCase::new("builtin:template")
        .with_name("go-style template")
        .with_config("template", json!("Hi, {{.user}}!"))
        .with_config("variables", json!({"user": "Alice"}))
        .expect_output("result", json!("Hi, Alice!"));
    let outcome = r.run_test(&test).await;
    assert!(outcome.passed, "outcome: {outcome:?}");
}

#[tokio::test]
async fn template_missing_template_fails() {
    let r = runner();
    let test = NodeTestCase::new("builtin:template")
        .with_name("no template")
        .should_fail_with("template is required");
    let outcome = r.run_test(&test).await;
    assert!(outcome.passed, "outcome: {outcome:?}");
}

// ─── TransformNode ───────────────────────────────────────────────────────────

#[tokio::test]
async fn transform_evaluates_cel_expression() {
    let r = runner();
    let test = NodeTestCase::new("builtin:transform")
        .with_name("cel addition")
        .with_config("expression", json!("1 + 2"))
        .expect_output("result", json!(3));
    let outcome = r.run_test(&test).await;
    assert!(outcome.passed, "outcome: {outcome:?}");
}

#[tokio::test]
async fn transform_with_data_context() {
    let r = runner();
    let test = NodeTestCase::new("builtin:transform")
        .with_name("cel with data")
        .with_config("expression", json!("input.x + input.y"))
        .with_config("data", json!({"x": 10, "y": 20}))
        .expect_output("result", json!(30));
    let outcome = r.run_test(&test).await;
    assert!(outcome.passed, "outcome: {outcome:?}");
}

#[tokio::test]
async fn transform_string_expression() {
    let r = runner();
    let test = NodeTestCase::new("builtin:transform")
        .with_name("cel string concat")
        .with_config("expression", json!("'hello' + ' ' + 'world'"))
        .expect_output("result", json!("hello world"))
        .expect_output("type", json!("string"));
    let outcome = r.run_test(&test).await;
    assert!(outcome.passed, "outcome: {outcome:?}");
}

#[tokio::test]
async fn transform_missing_expression_fails() {
    let r = runner();
    let test = NodeTestCase::new("builtin:transform")
        .with_name("no expression")
        .should_fail_with("expression is required");
    let outcome = r.run_test(&test).await;
    assert!(outcome.passed, "outcome: {outcome:?}");
}

// ─── FilterNode ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn filter_selects_matching_items() {
    let r = runner();
    let test = NodeTestCase::new("builtin:filter")
        .with_name("filter even numbers")
        .with_config("items", json!([1, 2, 3, 4, 5, 6]))
        .with_config("expression", json!("item % 2 == 0"))
        .expect_output("result", json!([2, 4, 6]))
        .expect_output("count", json!(3))
        .expect_output("total", json!(6));
    let outcome = r.run_test(&test).await;
    assert!(outcome.passed, "outcome: {outcome:?}");
}

#[tokio::test]
async fn filter_empty_result() {
    let r = runner();
    let test = NodeTestCase::new("builtin:filter")
        .with_name("filter no match")
        .with_config("items", json!([1, 3, 5]))
        .with_config("expression", json!("item > 10"))
        .expect_output("result", json!([]))
        .expect_output("count", json!(0));
    let outcome = r.run_test(&test).await;
    assert!(outcome.passed, "outcome: {outcome:?}");
}

#[tokio::test]
async fn filter_missing_expression_fails() {
    let r = runner();
    let test = NodeTestCase::new("builtin:filter")
        .with_name("no expression")
        .with_config("items", json!([1, 2, 3]))
        .should_fail_with("expression is required");
    let outcome = r.run_test(&test).await;
    assert!(outcome.passed, "outcome: {outcome:?}");
}

#[tokio::test]
async fn filter_missing_items_fails() {
    let r = runner();
    let test = NodeTestCase::new("builtin:filter")
        .with_name("no items")
        .with_config("expression", json!("item > 0"))
        .should_fail_with("items is required");
    let outcome = r.run_test(&test).await;
    assert!(outcome.passed, "outcome: {outcome:?}");
}

#[tokio::test]
async fn filter_non_array_items_fails() {
    let r = runner();
    let test = NodeTestCase::new("builtin:filter")
        .with_name("items not array")
        .with_config("items", json!("not an array"))
        .with_config("expression", json!("item > 0"))
        .should_fail_with("items must be an array");
    let outcome = r.run_test(&test).await;
    assert!(outcome.passed, "outcome: {outcome:?}");
}

// ─── SortNode ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn sort_ascending_by_key() {
    let r = runner();
    let test = NodeTestCase::new("builtin:sort")
        .with_name("sort asc")
        .with_config(
            "items",
            json!([
                {"name": "Charlie", "age": 30},
                {"name": "Alice", "age": 25},
                {"name": "Bob", "age": 28}
            ]),
        )
        .with_param("key", json!("age"))
        .with_param("direction", json!("asc"))
        .expect_output(
            "items",
            json!([
                {"name": "Alice", "age": 25},
                {"name": "Bob", "age": 28},
                {"name": "Charlie", "age": 30}
            ]),
        );
    let outcome = r.run_test(&test).await;
    assert!(outcome.passed, "outcome: {outcome:?}");
}

#[tokio::test]
async fn sort_descending_by_key() {
    let r = runner();
    let test = NodeTestCase::new("builtin:sort")
        .with_name("sort desc")
        .with_config(
            "items",
            json!([
                {"name": "Alice", "score": 85},
                {"name": "Bob", "score": 92},
                {"name": "Charlie", "score": 78}
            ]),
        )
        .with_param("key", json!("score"))
        .with_param("direction", json!("desc"))
        .expect_output(
            "items",
            json!([
                {"name": "Bob", "score": 92},
                {"name": "Alice", "score": 85},
                {"name": "Charlie", "score": 78}
            ]),
        );
    let outcome = r.run_test(&test).await;
    assert!(outcome.passed, "outcome: {outcome:?}");
}

#[tokio::test]
async fn sort_missing_items_fails() {
    let r = runner();
    let test = NodeTestCase::new("builtin:sort")
        .with_name("no items")
        .with_param("key", json!("age"))
        .should_fail_with("items field is required");
    let outcome = r.run_test(&test).await;
    assert!(outcome.passed, "outcome: {outcome:?}");
}

#[tokio::test]
async fn sort_missing_key_fails() {
    let r = runner();
    let test = NodeTestCase::new("builtin:sort")
        .with_name("no key")
        .with_config("items", json!([1, 2, 3]))
        .should_fail_with("key is required");
    let outcome = r.run_test(&test).await;
    assert!(outcome.passed, "outcome: {outcome:?}");
}

// ─── DelayNode ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn delay_short_duration() {
    let r = runner();
    let test = NodeTestCase::new("builtin:delay")
        .with_name("10ms delay")
        .with_config("duration", json!("10ms"))
        .with_timeout(5000)
        .expect_output("delayed", json!("10ms"));
    let outcome = r.run_test(&test).await;
    assert!(outcome.passed, "outcome: {outcome:?}");
}

#[tokio::test]
async fn delay_missing_duration_uses_default() {
    let r = runner();
    let test = NodeTestCase::new("builtin:delay")
        .with_name("missing duration uses default")
        .with_timeout(5000)
        .expect_exists("delayed");
    let outcome = r.run_test(&test).await;
    assert!(outcome.passed, "outcome: {outcome:?}");
}

#[tokio::test]
async fn delay_invalid_duration_fails() {
    let r = runner();
    let test = NodeTestCase::new("builtin:delay")
        .with_name("bad duration")
        .with_config("duration", json!("not-a-duration"))
        .should_fail_with("invalid duration");
    let outcome = r.run_test(&test).await;
    assert!(outcome.passed, "outcome: {outcome:?}");
}

// ─── YAML Suite Test ─────────────────────────────────────────────────────────

#[tokio::test]
async fn yaml_suite_mixed_builtins() {
    let r = runner();
    let yaml = r#"
name: Builtin Smoke Tests
description: Quick validation of core builtins via YAML suite
tests:
  - name: log passthrough
    plugin_ref: "builtin:log"
    config:
      message: "smoke test"
    assertions:
      - type: succeeds

  - name: base64 encode
    plugin_ref: "builtin:encode"
    config:
      input: "orbflow"
      operation: "base64-encode"
    assertions:
      - type: equals
        field: result
        expected: "b3JiZmxvdw=="

  - name: simple template
    plugin_ref: "builtin:template"
    config:
      template: "{{ greeting }}, {{ target }}!"
      variables:
        greeting: Hello
        target: World
    assertions:
      - type: equals
        field: result
        expected: "Hello, World!"

  - name: transform arithmetic
    plugin_ref: "builtin:transform"
    config:
      expression: "2 * 21"
    assertions:
      - type: equals
        field: result
        expected: 42
"#;
    let report = r.run_yaml(yaml).await.unwrap();
    assert!(
        report.all_passed(),
        "suite failed: {} / {} passed\n{report:?}",
        report.passed,
        report.total
    );
}
