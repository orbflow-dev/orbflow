// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Filter node: per-item CEL evaluation on arrays.

use std::collections::HashMap;

use async_trait::async_trait;
use serde_json::Value;

use orbflow_cel::CelEvaluator;
use orbflow_core::OrbflowError;
use orbflow_core::ports::{
    FieldSchema, FieldType, NodeExecutor, NodeInput, NodeOutput, NodeSchema, NodeSchemaProvider,
};

use crate::util::{make_output, resolve_config, string_val, to_slice};

/// Evaluates a CEL expression per item in an array and returns matching items.
pub struct FilterNode {
    evaluator: CelEvaluator,
}

impl FilterNode {
    pub fn new() -> Self {
        Self {
            evaluator: CelEvaluator::new(),
        }
    }
}

impl Default for FilterNode {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeSchemaProvider for FilterNode {
    fn node_schema(&self) -> NodeSchema {
        NodeSchema {
            plugin_ref: "builtin:filter".into(),
            name: "Filter".into(),
            description: "Filter an array using an expression".into(),
            category: "builtin".into(),
            node_kind: None,
            icon: "filter".into(),
            color: "#ec4899".into(),
            image_url: Some("/icons/filter.svg".into()),
            docs: None,
            inputs: vec![
                FieldSchema {
                    key: "items".into(),
                    label: "Items".into(),
                    field_type: FieldType::Array,
                    required: true,
                    default: None,
                    description: Some("The array to filter".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "expression".into(),
                    label: "Condition".into(),
                    field_type: FieldType::String,
                    required: true,
                    default: None,
                    description: Some(
                        "CEL expression per item (e.g. item.status == \"active\")".into(),
                    ),
                    r#enum: vec![],
                    credential_type: None,
                },
            ],
            outputs: vec![
                FieldSchema {
                    key: "result".into(),
                    label: "Filtered Items".into(),
                    field_type: FieldType::Array,
                    required: false,
                    default: None,
                    description: None,
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "count".into(),
                    label: "Match Count".into(),
                    field_type: FieldType::Number,
                    required: false,
                    default: None,
                    description: None,
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "total".into(),
                    label: "Original Count".into(),
                    field_type: FieldType::Number,
                    required: false,
                    default: None,
                    description: None,
                    r#enum: vec![],
                    credential_type: None,
                },
            ],
            parameters: vec![],
            capability_ports: vec![],
            settings: vec![],
            provides_capability: None,
        }
    }
}

#[async_trait]
impl NodeExecutor for FilterNode {
    async fn execute(&self, input: &NodeInput) -> Result<NodeOutput, OrbflowError> {
        let cfg = resolve_config(input);
        let expr = string_val(&cfg, "expression", "");
        if expr.is_empty() {
            return Err(OrbflowError::InvalidNodeConfig(
                "filter node: expression is required".into(),
            ));
        }

        let raw_items = cfg.get("items").ok_or_else(|| {
            OrbflowError::InvalidNodeConfig("filter node: items is required".into())
        })?;

        let items = to_slice(raw_items).ok_or_else(|| {
            OrbflowError::InvalidNodeConfig("filter node: items must be an array".into())
        })?;

        const MAX_ARRAY_ITEMS: usize = 10_000;
        if items.len() > MAX_ARRAY_ITEMS {
            return Err(OrbflowError::InvalidNodeConfig(format!(
                "filter node: items array exceeds maximum of {MAX_ARRAY_ITEMS} elements"
            )));
        }

        let total = items.len();
        let mut result = Vec::new();

        for (i, item) in items.iter().enumerate() {
            let mut ctx = HashMap::new();
            ctx.insert("item".into(), item.clone());
            ctx.insert("i".into(), Value::Number((i as i64).into()));

            match self.evaluator.eval_bool(&expr, &ctx) {
                Ok(true) => result.push(item.clone()),
                Ok(false) => {}
                Err(e) => {
                    return Err(OrbflowError::InvalidNodeConfig(format!(
                        "filter node: expression evaluation failed at item {i}: {e}"
                    )));
                }
            }
        }

        let count = result.len();

        Ok(NodeOutput {
            data: Some(make_output(vec![
                ("result", Value::Array(result)),
                ("count", Value::Number(count.into())),
                ("total", Value::Number(total.into())),
            ])),
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orbflow_core::execution::InstanceId;

    fn make_input(config: HashMap<String, Value>) -> NodeInput {
        NodeInput {
            instance_id: InstanceId::new("inst-1"),
            node_id: "filter-1".into(),
            plugin_ref: "builtin:filter".into(),
            config: Some(config),
            input: None,
            parameters: None,
            capabilities: None,
            attempt: 1,
        }
    }

    #[tokio::test]
    async fn test_filter_basic() {
        let node = FilterNode::new();
        let mut config = HashMap::new();
        config.insert("expression".into(), serde_json::json!("item > 2"));
        config.insert("items".into(), serde_json::json!([1, 2, 3, 4, 5]));

        let output = node.execute(&make_input(config)).await.unwrap();
        let data = output.data.unwrap();
        let result = data.get("result").unwrap().as_array().unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(data.get("count").unwrap(), 3);
        assert_eq!(data.get("total").unwrap(), 5);
    }

    #[tokio::test]
    async fn test_filter_empty_result() {
        let node = FilterNode::new();
        let mut config = HashMap::new();
        config.insert("expression".into(), serde_json::json!("item > 100"));
        config.insert("items".into(), serde_json::json!([1, 2, 3]));

        let output = node.execute(&make_input(config)).await.unwrap();
        let data = output.data.unwrap();
        let result = data.get("result").unwrap().as_array().unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_filter_missing_items() {
        let node = FilterNode::new();
        let mut config = HashMap::new();
        config.insert("expression".into(), serde_json::json!("true"));

        let result = node.execute(&make_input(config)).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_filter_schema() {
        let node = FilterNode::new();
        let schema = node.node_schema();
        assert_eq!(schema.plugin_ref, "builtin:filter");
    }
}
