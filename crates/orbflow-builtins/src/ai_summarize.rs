// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! AI Summarize node: summarize text using an LLM.

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

/// Summarizes text using an LLM.
pub struct AiSummarizeNode;

impl NodeSchemaProvider for AiSummarizeNode {
    fn node_schema(&self) -> NodeSchema {
        NodeSchema {
            plugin_ref: "builtin:ai-summarize".into(),
            name: "AI Summarize".into(),
            description: "Summarize text using an LLM".into(),
            category: "builtin".into(),
            node_kind: None,
            icon: "file-text".into(),
            color: "#A855F7".into(),
            image_url: None,
            docs: None,
            inputs: vec![FieldSchema {
                key: "text".into(),
                label: "Text".into(),
                field_type: FieldType::String,
                required: true,
                default: None,
                description: Some("The text to summarize".into()),
                r#enum: vec![],
                credential_type: None,
            }],
            outputs: vec![
                FieldSchema {
                    key: "summary".into(),
                    label: "Summary".into(),
                    field_type: FieldType::String,
                    required: false,
                    default: None,
                    description: Some("The generated summary".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "key_points".into(),
                    label: "Key Points".into(),
                    field_type: FieldType::Array,
                    required: false,
                    default: None,
                    description: Some("List of key points from the text".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
            ]
            .into_iter()
            .chain(ai_common_outputs())
            .collect(),
            parameters: vec![
                FieldSchema {
                    key: "style".into(),
                    label: "Style".into(),
                    field_type: FieldType::String,
                    required: false,
                    default: Some(Value::String("brief".into())),
                    description: Some("Summarization style".into()),
                    r#enum: vec![
                        "brief".into(),
                        "detailed".into(),
                        "bullet_points".into(),
                        "key_takeaways".into(),
                    ],
                    credential_type: None,
                },
                FieldSchema {
                    key: "max_length".into(),
                    label: "Max Length".into(),
                    field_type: FieldType::String,
                    required: false,
                    default: Some(Value::String("1 paragraph".into())),
                    description: Some("Maximum length of the summary".into()),
                    r#enum: vec![
                        "1-2 sentences".into(),
                        "1 paragraph".into(),
                        "3 paragraphs".into(),
                        "unlimited".into(),
                    ],
                    credential_type: None,
                },
            ]
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
impl NodeExecutor for AiSummarizeNode {
    async fn execute(&self, input: &NodeInput) -> Result<NodeOutput, OrbflowError> {
        let cfg = resolve_config(input);

        let text = string_val(&cfg, "text", "");
        if text.is_empty() {
            return Err(OrbflowError::InvalidNodeConfig(
                "ai-summarize node: text is required".into(),
            ));
        }

        let style = string_val(&cfg, "style", "brief");
        let max_length = string_val(&cfg, "max_length", "1 paragraph");
        let config = AiConfig::from_config(&cfg)?;

        let system_prompt = format!(
            "You are a text summarizer. Summarize the given text in {style} style. \
            Max length: {max_length}. \
            Return JSON: {{\"summary\": \"...\", \"key_points\": [\"...\"]}}",
            style = style,
            max_length = max_length,
        );

        let response = chat_completion(
            &config,
            Some(&system_prompt),
            vec![("user".into(), text)],
            true,
        )
        .await?;

        let cost = estimate_cost(&config.provider, &config.model, &response.usage);
        let usage_val = usage_to_json(&response.usage);

        let parsed = serde_json::from_str::<Value>(&response.content).unwrap_or(Value::Null);
        let summary = parsed
            .get("summary")
            .cloned()
            .unwrap_or(Value::String(response.content.clone()));
        let key_points = parsed
            .get("key_points")
            .cloned()
            .unwrap_or(Value::Array(vec![]));

        Ok(NodeOutput {
            data: Some(make_output(vec![
                ("summary", summary),
                ("key_points", key_points),
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
