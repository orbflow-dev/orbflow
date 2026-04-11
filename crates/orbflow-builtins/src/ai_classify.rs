// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! AI Classify node: classify text into categories using an LLM.

use async_trait::async_trait;
use serde_json::Value;

use orbflow_core::OrbflowError;
use orbflow_core::ports::{
    FieldSchema, FieldType, NodeExecutor, NodeInput, NodeOutput, NodeSchema, NodeSchemaProvider,
};

use crate::ai_common::{
    AiConfig, ai_common_outputs, ai_common_parameters, chat_completion, estimate_cost,
    usage_to_json,
};
use crate::util::{bool_val, make_output, resolve_config, string_val};

/// Classifies text into one or more categories using an LLM.
pub struct AiClassifyNode;

impl NodeSchemaProvider for AiClassifyNode {
    fn node_schema(&self) -> NodeSchema {
        NodeSchema {
            plugin_ref: "builtin:ai-classify".into(),
            name: "AI Classify".into(),
            description: "Classify text into categories using an LLM".into(),
            category: "builtin".into(),
            node_kind: None,
            icon: "brain".into(),
            color: "#9333EA".into(),
            image_url: None,
            docs: None,
            inputs: vec![
                FieldSchema {
                    key: "text".into(),
                    label: "Text".into(),
                    field_type: FieldType::String,
                    required: true,
                    default: None,
                    description: Some("The text to classify".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "categories".into(),
                    label: "Categories".into(),
                    field_type: FieldType::String,
                    required: true,
                    default: None,
                    description: Some("Comma-separated list of categories".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "instructions".into(),
                    label: "Instructions".into(),
                    field_type: FieldType::String,
                    required: false,
                    default: None,
                    description: Some("Additional classification instructions".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
            ],
            outputs: vec![
                FieldSchema {
                    key: "category".into(),
                    label: "Category".into(),
                    field_type: FieldType::String,
                    required: false,
                    default: None,
                    description: Some("Top predicted category (single-label mode)".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "categories".into(),
                    label: "Categories".into(),
                    field_type: FieldType::Array,
                    required: false,
                    default: None,
                    description: Some(
                        "All predicted categories with confidence scores (multi-label mode)".into(),
                    ),
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "confidence".into(),
                    label: "Confidence".into(),
                    field_type: FieldType::Number,
                    required: false,
                    default: None,
                    description: Some("Confidence score (0.0–1.0)".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "reasoning".into(),
                    label: "Reasoning".into(),
                    field_type: FieldType::String,
                    required: false,
                    default: None,
                    description: Some("Brief explanation of the classification".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
            ]
            .into_iter()
            .chain(ai_common_outputs())
            .collect(),
            parameters: vec![FieldSchema {
                key: "multi_label".into(),
                label: "Multi-Label".into(),
                field_type: FieldType::Boolean,
                required: false,
                default: Some(Value::Bool(false)),
                description: Some("Allow multiple categories to be selected".into()),
                r#enum: vec![],
                credential_type: None,
            }]
            .into_iter()
            .chain(ai_common_parameters())
            .collect(),
            capability_ports: vec![],
            settings: vec![],
            provides_capability: None,
        }
    }
}

#[async_trait]
impl NodeExecutor for AiClassifyNode {
    async fn execute(&self, input: &NodeInput) -> Result<NodeOutput, OrbflowError> {
        let cfg = resolve_config(input);

        let text = string_val(&cfg, "text", "");
        if text.is_empty() {
            return Err(OrbflowError::InvalidNodeConfig(
                "ai-classify node: text is required".into(),
            ));
        }

        let categories = string_val(&cfg, "categories", "");
        if categories.is_empty() {
            return Err(OrbflowError::InvalidNodeConfig(
                "ai-classify node: categories is required".into(),
            ));
        }

        let instructions = string_val(&cfg, "instructions", "");
        let multi_label = bool_val(&cfg, "multi_label", false);
        let config = AiConfig::from_config(&cfg)?;

        let system_prompt = if multi_label {
            "You are a text classifier. Classify the given text into one or more of the provided \
            categories. Respond ONLY with valid JSON: \
            {\"categories\": [{\"category\": \"...\", \"confidence\": 0.0}], \
            \"reasoning\": \"brief explanation\"}"
        } else {
            "You are a text classifier. Classify the given text into exactly one of the provided \
            categories. Respond ONLY with valid JSON: \
            {\"category\": \"chosen_category\", \"confidence\": 0.0, \
            \"reasoning\": \"brief explanation\"}"
        };

        let user_content = if instructions.is_empty() {
            format!("Categories: {categories}\n\nText:\n{text}")
        } else {
            format!("Categories: {categories}\n\n{instructions}\n\nText:\n{text}")
        };

        let response = chat_completion(
            &config,
            Some(system_prompt),
            vec![("user".into(), user_content)],
            true,
        )
        .await?;

        let cost = estimate_cost(&config.provider, &config.model, &response.usage);
        let usage_val = usage_to_json(&response.usage);

        // Parse the JSON response to extract classification fields.
        let parsed = serde_json::from_str::<Value>(&response.content)
            .unwrap_or(Value::Object(serde_json::Map::new()));

        let category = parsed
            .get("category")
            .and_then(|v| v.as_str())
            .map(|s| Value::String(s.into()))
            .unwrap_or(Value::Null);

        let categories_val = parsed
            .get("categories")
            .cloned()
            .unwrap_or(Value::Array(vec![]));

        let confidence = parsed
            .get("confidence")
            .and_then(|v| v.as_f64())
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
            .unwrap_or(Value::Null);

        let reasoning = parsed
            .get("reasoning")
            .and_then(|v| v.as_str())
            .map(|s| Value::String(s.into()))
            .unwrap_or(Value::Null);

        Ok(NodeOutput {
            data: Some(make_output(vec![
                ("category", category),
                ("categories", categories_val),
                ("confidence", confidence),
                ("reasoning", reasoning),
                ("usage", usage_val),
                (
                    "cost_usd",
                    Value::Number(serde_json::Number::from_f64(cost).unwrap_or(0.into())),
                ),
            ])),
            error: None,
        })
    }
}
