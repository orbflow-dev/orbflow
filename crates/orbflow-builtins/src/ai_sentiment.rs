// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! AI Sentiment node: sentiment analysis of text using an LLM.

use async_trait::async_trait;
use serde_json::Value;

use orbflow_core::OrbflowError;
use orbflow_core::ports::{
    FieldSchema, FieldType, NodeExecutor, NodeInput, NodeOutput, NodeSchema, NodeSchemaProvider,
};

use crate::ai_common::{ai_common_outputs, ai_common_parameters, execute_ai_node};

/// Analyzes sentiment of text using an LLM.
pub struct AiSentimentNode;

impl NodeSchemaProvider for AiSentimentNode {
    fn node_schema(&self) -> NodeSchema {
        NodeSchema {
            plugin_ref: "builtin:ai-sentiment".into(),
            name: "AI Sentiment".into(),
            description: "Analyze the sentiment of text using an LLM".into(),
            category: "builtin".into(),
            node_kind: None,
            icon: "brain".into(),
            color: "#C084FC".into(),
            image_url: None,
            docs: None,
            inputs: vec![FieldSchema {
                key: "text".into(),
                label: "Text".into(),
                field_type: FieldType::String,
                required: true,
                default: None,
                description: Some("The text to analyze for sentiment".into()),
                r#enum: vec![],
                credential_type: None,
            }],
            outputs: vec![
                FieldSchema {
                    key: "sentiment".into(),
                    label: "Sentiment".into(),
                    field_type: FieldType::String,
                    required: false,
                    default: None,
                    description: Some(
                        "Overall sentiment: positive, negative, neutral, or mixed".into(),
                    ),
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "score".into(),
                    label: "Score".into(),
                    field_type: FieldType::Number,
                    required: false,
                    default: None,
                    description: Some(
                        "Sentiment score from -1.0 (negative) to 1.0 (positive)".into(),
                    ),
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "emotions".into(),
                    label: "Emotions".into(),
                    field_type: FieldType::Object,
                    required: false,
                    default: None,
                    description: Some(
                        "Detected emotions with intensity scores (0.0 to 1.0)".into(),
                    ),
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "reasoning".into(),
                    label: "Reasoning".into(),
                    field_type: FieldType::String,
                    required: false,
                    default: None,
                    description: Some("Explanation of the sentiment analysis".into()),
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
impl NodeExecutor for AiSentimentNode {
    async fn execute(&self, input: &NodeInput) -> Result<NodeOutput, OrbflowError> {
        let system_prompt = "Analyze the sentiment of the given text. \
            Return JSON: {\"sentiment\": \"positive|negative|neutral|mixed\", \
            \"score\": -1.0 to 1.0, \
            \"emotions\": {\"joy\": 0.0-1.0, ...}, \
            \"reasoning\": \"...\"}";

        execute_ai_node(input, "ai-sentiment", "text", system_prompt, |parsed| {
            vec![
                (
                    "sentiment",
                    parsed
                        .get("sentiment")
                        .cloned()
                        .unwrap_or(Value::String("neutral".into())),
                ),
                (
                    "score",
                    parsed.get("score").cloned().unwrap_or(Value::from(0.0f64)),
                ),
                (
                    "emotions",
                    parsed
                        .get("emotions")
                        .cloned()
                        .unwrap_or(Value::Object(serde_json::Map::new())),
                ),
                (
                    "reasoning",
                    parsed
                        .get("reasoning")
                        .cloned()
                        .unwrap_or(Value::String(String::new())),
                ),
            ]
        })
        .await
    }
}
