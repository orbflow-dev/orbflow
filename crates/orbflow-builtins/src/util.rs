// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Shared utilities for built-in node executors.

use std::collections::HashMap;

use serde_json::Value;

use orbflow_core::ports::NodeInput;

/// Builds the effective configuration map for a node execution.
///
/// Merges `input.config` (static node config), `input.input` (runtime-resolved
/// mappings), and optionally `input.parameters` (credential-resolved values)
/// using last-writer-wins semantics at each layer.
pub fn resolve_config(input: &NodeInput) -> HashMap<String, Value> {
    let mut cfg = merge_config(input.config.as_ref(), input.input.as_ref());
    if let Some(params) = &input.parameters {
        cfg = merge_config(Some(&cfg), Some(params));
    }
    cfg
}

/// Merges two optional config maps. Values from `overlay` override `base`.
pub fn merge_config(
    base: Option<&HashMap<String, Value>>,
    overlay: Option<&HashMap<String, Value>>,
) -> HashMap<String, Value> {
    let mut result = HashMap::new();
    if let Some(b) = base {
        for (k, v) in b {
            result.insert(k.clone(), v.clone());
        }
    }
    if let Some(o) = overlay {
        for (k, v) in o {
            result.insert(k.clone(), v.clone());
        }
    }
    result
}

/// Extracts a string value from a config map, returning `fallback` if missing or wrong type.
pub fn string_val(m: &HashMap<String, Value>, key: &str, fallback: &str) -> String {
    match m.get(key) {
        Some(Value::String(s)) => s.clone(),
        _ => fallback.to_owned(),
    }
}

/// Extracts an integer value from a config map, returning `fallback` if missing or wrong type.
pub fn int_val(m: &HashMap<String, Value>, key: &str, fallback: i64) -> i64 {
    match m.get(key) {
        Some(Value::Number(n)) => {
            if let Some(i) = n.as_i64() {
                i
            } else if let Some(f) = n.as_f64() {
                f as i64
            } else {
                fallback
            }
        }
        _ => fallback,
    }
}

/// Extracts a float value from a config map, returning `fallback` if missing or wrong type.
pub fn float_val(m: &HashMap<String, Value>, key: &str, fallback: f64) -> f64 {
    match m.get(key) {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(fallback),
        _ => fallback,
    }
}

/// Extracts a boolean value from a config map, returning `fallback` if missing or wrong type.
pub fn bool_val(m: &HashMap<String, Value>, key: &str, fallback: bool) -> bool {
    match m.get(key) {
        Some(Value::Bool(b)) => *b,
        _ => fallback,
    }
}

/// Converts a JSON value to a `Vec<Value>` if it is an array.
/// Returns `None` for non-array values.
pub fn to_slice(v: &Value) -> Option<Vec<Value>> {
    match v {
        Value::Array(arr) => Some(arr.clone()),
        _ => None,
    }
}

/// Builds a `HashMap<String, Value>` output from key-value pairs.
pub fn make_output(pairs: Vec<(&str, Value)>) -> HashMap<String, Value> {
    pairs.into_iter().map(|(k, v)| (k.to_owned(), v)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_config_both_none() {
        let result = merge_config(None, None);
        assert!(result.is_empty());
    }

    #[test]
    fn test_merge_config_overlay_wins() {
        let base: HashMap<String, Value> = [("k".into(), Value::String("a".into()))].into();
        let overlay: HashMap<String, Value> = [("k".into(), Value::String("b".into()))].into();
        let result = merge_config(Some(&base), Some(&overlay));
        assert_eq!(result.get("k").unwrap(), &Value::String("b".into()));
    }

    #[test]
    fn test_string_val_present() {
        let m: HashMap<String, Value> = [("key".into(), Value::String("val".into()))].into();
        assert_eq!(string_val(&m, "key", ""), "val");
    }

    #[test]
    fn test_string_val_missing() {
        let m: HashMap<String, Value> = HashMap::new();
        assert_eq!(string_val(&m, "key", "default"), "default");
    }

    #[test]
    fn test_int_val_from_float() {
        let m: HashMap<String, Value> = [("port".into(), serde_json::json!(587.0))].into();
        assert_eq!(int_val(&m, "port", 0), 587);
    }

    #[test]
    fn test_bool_val() {
        let m: HashMap<String, Value> = [("flag".into(), Value::Bool(true))].into();
        assert!(bool_val(&m, "flag", false));
        assert!(!bool_val(&m, "missing", false));
    }

    #[test]
    fn test_to_slice_array() {
        let v = serde_json::json!([1, 2, 3]);
        let slice = to_slice(&v).unwrap();
        assert_eq!(slice.len(), 3);
    }

    #[test]
    fn test_to_slice_non_array() {
        let v = serde_json::json!("not an array");
        assert!(to_slice(&v).is_none());
    }
}
