// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Transform node: CEL expression evaluation against input data.

use std::collections::HashMap;

use async_trait::async_trait;
use serde_json::Value;

use orbflow_cel::CelEvaluator;
use orbflow_core::OrbflowError;
use orbflow_core::ports::{
    FieldSchema, FieldType, NodeExecutor, NodeInput, NodeOutput, NodeSchema, NodeSchemaProvider,
};

use crate::util::{make_output, resolve_config, string_val};

/// Evaluates a CEL expression against input data.
pub struct TransformNode {
    evaluator: CelEvaluator,
}

impl TransformNode {
    pub fn new() -> Self {
        Self {
            evaluator: CelEvaluator::new(),
        }
    }
}

impl Default for TransformNode {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeSchemaProvider for TransformNode {
    fn node_schema(&self) -> NodeSchema {
        NodeSchema {
            plugin_ref: "builtin:transform".into(),
            name: "Transform".into(),
            description: "Reshape data using a CEL expression".into(),
            category: "builtin".into(),
            node_kind: None,
            icon: "zap".into(),
            color: "#8b5cf6".into(),
            image_url: Some("/icons/zap.svg".into()),
            docs: None,
            inputs: vec![
                FieldSchema {
                    key: "expression".into(),
                    label: "Expression".into(),
                    field_type: FieldType::String,
                    required: true,
                    default: None,
                    description: Some(
                        "CEL expression to evaluate (use 'input' to reference data)".into(),
                    ),
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "data".into(),
                    label: "Input Data".into(),
                    field_type: FieldType::Object,
                    required: false,
                    default: None,
                    description: Some("Data available as 'input' in the expression".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
            ],
            outputs: vec![
                FieldSchema {
                    key: "result".into(),
                    label: "Result".into(),
                    field_type: FieldType::Object,
                    required: false,
                    default: None,
                    description: None,
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "type".into(),
                    label: "Result Type".into(),
                    field_type: FieldType::String,
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

/// Returns a human-readable type name for a JSON value.
fn value_type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(n) => {
            if n.is_i64() || n.is_u64() {
                "int"
            } else {
                "float"
            }
        }
        Value::String(_) => "string",
        Value::Array(_) => "list",
        Value::Object(_) => "map",
    }
}

#[async_trait]
impl NodeExecutor for TransformNode {
    async fn execute(&self, input: &NodeInput) -> Result<NodeOutput, OrbflowError> {
        let cfg = resolve_config(input);
        let expr = string_val(&cfg, "expression", "");
        if expr.is_empty() {
            return Err(OrbflowError::InvalidNodeConfig(
                "transform node: expression is required".into(),
            ));
        }

        let data = cfg
            .get("data")
            .cloned()
            .unwrap_or(Value::Object(serde_json::Map::new()));

        let mut cel_ctx = HashMap::new();
        cel_ctx.insert("input".into(), data);

        let result = self
            .evaluator
            .eval_any(&expr, &cel_ctx)
            .map_err(|e| OrbflowError::Internal(format!("transform node: eval expression: {e}")))?;

        let type_name = value_type_name(&result);

        Ok(NodeOutput {
            data: Some(make_output(vec![
                ("result", result),
                ("type", Value::String(type_name.into())),
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
            node_id: "transform-1".into(),
            plugin_ref: "builtin:transform".into(),
            config: Some(config),
            input: None,
            parameters: None,
            capabilities: None,
            attempt: 1,
        }
    }

    #[tokio::test]
    async fn test_transform_arithmetic() {
        let node = TransformNode::new();
        let mut config = HashMap::new();
        config.insert("expression".into(), serde_json::json!("input.a + input.b"));
        config.insert("data".into(), serde_json::json!({"a": 10, "b": 20}));

        let output = node.execute(&make_input(config)).await.unwrap();
        let data = output.data.unwrap();
        assert_eq!(data.get("result").unwrap(), &serde_json::json!(30));
    }

    #[tokio::test]
    async fn test_transform_missing_expression() {
        let node = TransformNode::new();
        let config = HashMap::new();
        let result = node.execute(&make_input(config)).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_transform_schema() {
        let node = TransformNode::new();
        let schema = node.node_schema();
        assert_eq!(schema.plugin_ref, "builtin:transform");
    }
}
