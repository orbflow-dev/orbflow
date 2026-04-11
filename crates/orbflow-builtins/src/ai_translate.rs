// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! AI Translate node: translate text to another language using an LLM.

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

/// Translates text to another language using an LLM.
pub struct AiTranslateNode;

impl NodeSchemaProvider for AiTranslateNode {
    fn node_schema(&self) -> NodeSchema {
        NodeSchema {
            plugin_ref: "builtin:ai-translate".into(),
            name: "AI Translate".into(),
            description: "Translate text to another language using an LLM".into(),
            category: "builtin".into(),
            node_kind: None,
            icon: "globe".into(),
            color: "#9333EA".into(),
            image_url: None,
            docs: None,
            inputs: vec![FieldSchema {
                key: "text".into(),
                label: "Text".into(),
                field_type: FieldType::String,
                required: true,
                default: None,
                description: Some("The text to translate".into()),
                r#enum: vec![],
                credential_type: None,
            }],
            outputs: vec![
                FieldSchema {
                    key: "translated".into(),
                    label: "Translated".into(),
                    field_type: FieldType::String,
                    required: false,
                    default: None,
                    description: Some("The translated text".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "detected_language".into(),
                    label: "Detected Language".into(),
                    field_type: FieldType::String,
                    required: false,
                    default: None,
                    description: Some("The detected or specified source language".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
            ]
            .into_iter()
            .chain(ai_common_outputs())
            .collect(),
            parameters: vec![
                FieldSchema {
                    key: "target_language".into(),
                    label: "Target Language".into(),
                    field_type: FieldType::String,
                    required: true,
                    default: Some(Value::String("English".into())),
                    description: Some("The language to translate into".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "source_language".into(),
                    label: "Source Language".into(),
                    field_type: FieldType::String,
                    required: false,
                    default: Some(Value::String("auto".into())),
                    description: Some(
                        "The source language (use 'auto' to detect automatically)".into(),
                    ),
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "tone".into(),
                    label: "Tone".into(),
                    field_type: FieldType::String,
                    required: false,
                    default: Some(Value::String("formal".into())),
                    description: Some("Translation tone".into()),
                    r#enum: vec![
                        "formal".into(),
                        "informal".into(),
                        "technical".into(),
                        "casual".into(),
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
impl NodeExecutor for AiTranslateNode {
    async fn execute(&self, input: &NodeInput) -> Result<NodeOutput, OrbflowError> {
        let cfg = resolve_config(input);

        let text = string_val(&cfg, "text", "");
        if text.is_empty() {
            return Err(OrbflowError::InvalidNodeConfig(
                "ai-translate node: text is required".into(),
            ));
        }

        let target_language = string_val(&cfg, "target_language", "English");
        if target_language.is_empty() {
            return Err(OrbflowError::InvalidNodeConfig(
                "ai-translate node: target_language is required".into(),
            ));
        }

        let source_language = string_val(&cfg, "source_language", "auto");
        let tone = string_val(&cfg, "tone", "formal");
        let config = AiConfig::from_config(&cfg)?;

        let system_prompt = format!(
            "Translate the given text to {target_language} in a {tone} tone. \
            If source_language is auto, detect it. \
            Return JSON: {{\"translated\": \"...\", \"detected_language\": \"...\"}}",
            target_language = target_language,
            tone = tone,
        );

        let user_content = if source_language == "auto" {
            text
        } else {
            format!(
                "Source language: {source_language}\n\n{text}",
                source_language = source_language,
                text = text
            )
        };

        let response = chat_completion(
            &config,
            Some(&system_prompt),
            vec![("user".into(), user_content)],
            true,
        )
        .await?;

        let cost = estimate_cost(&config.provider, &config.model, &response.usage);
        let usage_val = usage_to_json(&response.usage);

        let parsed = serde_json::from_str::<Value>(&response.content).unwrap_or(Value::Null);
        let translated = parsed
            .get("translated")
            .cloned()
            .unwrap_or(Value::String(response.content.clone()));
        let detected_language = parsed
            .get("detected_language")
            .cloned()
            .unwrap_or(Value::String(source_language));

        Ok(NodeOutput {
            data: Some(make_output(vec![
                ("translated", translated),
                ("detected_language", detected_language),
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
