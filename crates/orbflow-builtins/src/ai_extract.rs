// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! AI Extract node: structured data extraction from text using an LLM.

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
use crate::util::{make_output, resolve_config, string_val};

/// Extracts structured data from text using an LLM with a user-supplied JSON schema.
pub struct AiExtractNode;

impl NodeSchemaProvider for AiExtractNode {
    fn node_schema(&self) -> NodeSchema {
        NodeSchema {
            plugin_ref: "builtin:ai-extract".into(),
            name: "AI Extract".into(),
            description: "Extract structured data from text using an LLM and a JSON schema".into(),
            category: "builtin".into(),
            node_kind: None,
            icon: "sparkles".into(),
            color: "#C084FC".into(),
            image_url: None,
            docs: None,
            inputs: vec![
                FieldSchema {
                    key: "text".into(),
                    label: "Text".into(),
                    field_type: FieldType::String,
                    required: true,
                    default: None,
                    description: Some("The text to extract data from".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "schema".into(),
                    label: "Schema".into(),
                    field_type: FieldType::String,
                    required: true,
                    default: None,
                    description: Some("JSON schema describing the structure to extract".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "instructions".into(),
                    label: "Instructions".into(),
                    field_type: FieldType::String,
                    required: false,
                    default: None,
                    description: Some("Additional extraction instructions for the model".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
            ],
            outputs: vec![
                FieldSchema {
                    key: "extracted".into(),
                    label: "Extracted".into(),
                    field_type: FieldType::Object,
                    required: false,
                    default: None,
                    description: Some("Extracted data matching the provided schema".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "raw_response".into(),
                    label: "Raw Response".into(),
                    field_type: FieldType::String,
                    required: false,
                    default: None,
                    description: Some("Raw model response text".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
            ]
            .into_iter()
            .chain(ai_common_outputs())
            .collect(),
            parameters: ai_common_parameters(),
            capability_ports: vec![],
            settings: vec![],
            provides_capability: None,
        }
    }
}

#[async_trait]
impl NodeExecutor for AiExtractNode {
    async fn execute(&self, input: &NodeInput) -> Result<NodeOutput, OrbflowError> {
        let cfg = resolve_config(input);

        let text = string_val(&cfg, "text", "");
        if text.is_empty() {
            return Err(OrbflowError::InvalidNodeConfig(
                "ai-extract node: text is required".into(),
            ));
        }

        let schema = string_val(&cfg, "schema", "");
        if schema.is_empty() {
            return Err(OrbflowError::InvalidNodeConfig(
                "ai-extract node: schema is required".into(),
            ));
        }

        let instructions = string_val(&cfg, "instructions", "");
        let config = AiConfig::from_config(&cfg)?;

        let system_prompt = "You are a data extraction assistant. Extract data from the provided \
            text according to the given JSON schema. Return ONLY valid JSON matching the schema, \
            with no additional text.";

        let user_content = if instructions.is_empty() {
            format!("Schema:\n{schema}\n\nText:\n{text}")
        } else {
            format!("Schema:\n{schema}\n\n{instructions}\n\nText:\n{text}")
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

        let extracted = serde_json::from_str::<Value>(&response.content).unwrap_or(Value::Null);

        Ok(NodeOutput {
            data: Some(make_output(vec![
                ("extracted", extracted),
                ("raw_response", Value::String(response.content)),
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
