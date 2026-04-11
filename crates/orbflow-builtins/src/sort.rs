// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Sort node: stable sort by key with direction (asc/desc).

use async_trait::async_trait;
use serde_json::Value;

use orbflow_core::OrbflowError;
use orbflow_core::ports::{
    FieldSchema, FieldType, NodeExecutor, NodeInput, NodeOutput, NodeSchema, NodeSchemaProvider,
};

use crate::util::{make_output, resolve_config, string_val, to_slice};

/// Sorts an array of objects by a specified key.
pub struct SortNode;

impl NodeSchemaProvider for SortNode {
    fn node_schema(&self) -> NodeSchema {
        NodeSchema {
            plugin_ref: "builtin:sort".into(),
            name: "Sort".into(),
            description: "Sort an array by a field".into(),
            category: "builtin".into(),
            node_kind: None,
            icon: "arrow-up-down".into(),
            color: "#EC4899".into(),
            image_url: Some("/icons/arrow-up-down.svg".into()),
            docs: None,
            inputs: vec![FieldSchema {
                key: "items".into(),
                label: "Items".into(),
                field_type: FieldType::Array,
                required: true,
                default: None,
                description: Some("Array of objects to sort".into()),
                r#enum: vec![],
                credential_type: None,
            }],
            outputs: vec![FieldSchema {
                key: "items".into(),
                label: "Sorted Items".into(),
                field_type: FieldType::Array,
                required: false,
                default: None,
                description: None,
                r#enum: vec![],
                credential_type: None,
            }],
            parameters: vec![
                FieldSchema {
                    key: "key".into(),
                    label: "Sort Key".into(),
                    field_type: FieldType::String,
                    required: true,
                    default: None,
                    description: Some("Field name to sort by".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "direction".into(),
                    label: "Direction".into(),
                    field_type: FieldType::String,
                    required: false,
                    default: Some(Value::String("asc".into())),
                    description: Some("Sort direction".into()),
                    r#enum: vec!["asc".into(), "desc".into()],
                    credential_type: None,
                },
            ],
            capability_ports: vec![],
            settings: vec![],
            provides_capability: None,
        }
    }
}

/// Extracts a field value from a JSON object for sorting.
fn extract_sort_value(item: &Value, key: &str) -> Option<Value> {
    match item {
        Value::Object(m) => m.get(key).cloned(),
        _ => None,
    }
}

/// Performs a less-than comparison for common JSON value types.
fn compare_values(a: &Option<Value>, b: &Option<Value>) -> std::cmp::Ordering {
    use std::cmp::Ordering;

    match (a, b) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (Some(va), Some(vb)) => compare_json_values(va, vb),
    }
}

fn compare_json_values(a: &Value, b: &Value) -> std::cmp::Ordering {
    use std::cmp::Ordering;

    match (a, b) {
        (Value::Number(na), Value::Number(nb)) => {
            let fa = na.as_f64().unwrap_or(0.0);
            let fb = nb.as_f64().unwrap_or(0.0);
            fa.partial_cmp(&fb).unwrap_or(Ordering::Equal)
        }
        (Value::String(sa), Value::String(sb)) => sa.cmp(sb),
        (Value::Bool(ba), Value::Bool(bb)) => ba.cmp(bb),
        (Value::Null, Value::Null) => Ordering::Equal,
        (Value::Null, _) => Ordering::Less,
        (_, Value::Null) => Ordering::Greater,
        // Fallback: compare string representations.
        _ => a.to_string().cmp(&b.to_string()),
    }
}

#[async_trait]
impl NodeExecutor for SortNode {
    async fn execute(&self, input: &NodeInput) -> Result<NodeOutput, OrbflowError> {
        let cfg = resolve_config(input);

        let raw_items = cfg.get("items").ok_or_else(|| {
            OrbflowError::InvalidNodeConfig("sort node: items field is required".into())
        })?;

        let items = to_slice(raw_items).ok_or_else(|| {
            OrbflowError::InvalidNodeConfig("sort node: items must be an array".into())
        })?;

        const MAX_ARRAY_ITEMS: usize = 10_000;
        if items.len() > MAX_ARRAY_ITEMS {
            return Err(OrbflowError::InvalidNodeConfig(format!(
                "sort node: items array exceeds maximum of {MAX_ARRAY_ITEMS} elements"
            )));
        }

        let key = string_val(&cfg, "key", "");
        if key.is_empty() {
            return Err(OrbflowError::InvalidNodeConfig(
                "sort node: sort key is required".into(),
            ));
        }
        let direction = string_val(&cfg, "direction", "asc");

        // Make a copy to avoid mutating the input.
        let mut sorted = items;

        sorted.sort_by(|a, b| {
            let va = extract_sort_value(a, &key);
            let vb = extract_sort_value(b, &key);
            let ordering = compare_values(&va, &vb);
            if direction == "desc" {
                ordering.reverse()
            } else {
                ordering
            }
        });

        Ok(NodeOutput {
            data: Some(make_output(vec![("items", Value::Array(sorted))])),
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orbflow_core::execution::InstanceId;
    use std::collections::HashMap;

    fn make_input(config: HashMap<String, Value>) -> NodeInput {
        NodeInput {
            instance_id: InstanceId::new("inst-1"),
            node_id: "sort-1".into(),
            plugin_ref: "builtin:sort".into(),
            config: Some(config),
            input: None,
            parameters: None,
            capabilities: None,
            attempt: 1,
        }
    }

    #[tokio::test]
    async fn test_sort_asc() {
        let node = SortNode;
        let mut config = HashMap::new();
        config.insert(
            "items".into(),
            serde_json::json!([
                {"name": "Charlie", "age": 30},
                {"name": "Alice", "age": 25},
                {"name": "Bob", "age": 28},
            ]),
        );
        config.insert("key".into(), serde_json::json!("name"));
        config.insert("direction".into(), serde_json::json!("asc"));

        let output = node.execute(&make_input(config)).await.unwrap();
        let data = output.data.unwrap();
        let items = data.get("items").unwrap().as_array().unwrap();
        assert_eq!(items[0]["name"], "Alice");
        assert_eq!(items[1]["name"], "Bob");
        assert_eq!(items[2]["name"], "Charlie");
    }

    #[tokio::test]
    async fn test_sort_desc_numeric() {
        let node = SortNode;
        let mut config = HashMap::new();
        config.insert(
            "items".into(),
            serde_json::json!([
                {"name": "Alice", "age": 25},
                {"name": "Charlie", "age": 30},
                {"name": "Bob", "age": 28},
            ]),
        );
        config.insert("key".into(), serde_json::json!("age"));
        config.insert("direction".into(), serde_json::json!("desc"));

        let output = node.execute(&make_input(config)).await.unwrap();
        let data = output.data.unwrap();
        let items = data.get("items").unwrap().as_array().unwrap();
        assert_eq!(items[0]["name"], "Charlie");
        assert_eq!(items[1]["name"], "Bob");
        assert_eq!(items[2]["name"], "Alice");
    }

    #[tokio::test]
    async fn test_sort_missing_key() {
        let node = SortNode;
        let mut config = HashMap::new();
        config.insert("items".into(), serde_json::json!([1, 2, 3]));

        let result = node.execute(&make_input(config)).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_sort_schema() {
        let node = SortNode;
        let schema = node.node_schema();
        assert_eq!(schema.plugin_ref, "builtin:sort");
    }
}
