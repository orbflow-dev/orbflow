// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! AI Chat node: generate text using an LLM provider.

use async_trait::async_trait;
use serde_json::Value;

use orbflow_core::OrbflowError;
use orbflow_core::ports::{
    FieldSchema, FieldType, NodeExecutor, NodeInput, NodeOutput, NodeSchema, NodeSchemaProvider,
};
use orbflow_core::streaming::{StreamSender, StreamingNodeExecutor};

use crate::ai_common::{
    AiConfig, ai_common_outputs, ai_common_parameters, chat_completion, estimate_cost,
    resolve_model_id, usage_to_json,
};
use crate::util::{make_output, resolve_config, string_val};

/// Generates text using an LLM (OpenAI, Anthropic, Google AI).
pub struct AiChatNode;

impl NodeSchemaProvider for AiChatNode {
    fn node_schema(&self) -> NodeSchema {
        NodeSchema {
            plugin_ref: "builtin:ai-chat".into(),
            name: "AI Chat".into(),
            description: "Generate text using an LLM (OpenAI, Anthropic, Google AI)".into(),
            category: "builtin".into(),
            node_kind: None,
            icon: "message-circle".into(),
            color: "#A855F7".into(),
            image_url: None,
            docs: None,
            inputs: vec![
                FieldSchema {
                    key: "prompt".into(),
                    label: "Prompt".into(),
                    field_type: FieldType::String,
                    required: true,
                    default: None,
                    description: Some("The user message or prompt".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "system_prompt".into(),
                    label: "System Prompt".into(),
                    field_type: FieldType::String,
                    required: false,
                    default: None,
                    description: Some("Instructions that define the AI's behavior".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "context".into(),
                    label: "Context".into(),
                    field_type: FieldType::Object,
                    required: false,
                    default: None,
                    description: Some("Additional context data (appended as JSON)".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
            ],
            outputs: vec![
                FieldSchema {
                    key: "content".into(),
                    label: "Content".into(),
                    field_type: FieldType::String,
                    required: false,
                    default: None,
                    description: Some("AI response text".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "parsed_json".into(),
                    label: "Parsed JSON".into(),
                    field_type: FieldType::Object,
                    required: false,
                    default: None,
                    description: Some("Parsed JSON (when response_format is json)".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "model".into(),
                    label: "Model".into(),
                    field_type: FieldType::String,
                    required: false,
                    default: None,
                    description: Some("Model used".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "finish_reason".into(),
                    label: "Finish Reason".into(),
                    field_type: FieldType::String,
                    required: false,
                    default: None,
                    description: Some("Why the model stopped".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
            ]
            .into_iter()
            .chain(ai_common_outputs())
            .collect(),
            parameters: vec![
                FieldSchema {
                    key: "temperature".into(),
                    label: "Temperature".into(),
                    field_type: FieldType::Number,
                    required: false,
                    default: Some(Value::Number(
                        serde_json::Number::from_f64(0.7)
                            .unwrap_or_else(|| serde_json::Number::from(1)),
                    )),
                    description: Some("Sampling temperature (0.0–2.0)".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "max_tokens".into(),
                    label: "Max Tokens".into(),
                    field_type: FieldType::Number,
                    required: false,
                    default: Some(Value::Number(1024.into())),
                    description: Some("Maximum tokens to generate".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "response_format".into(),
                    label: "Response Format".into(),
                    field_type: FieldType::String,
                    required: false,
                    default: Some(Value::String("text".into())),
                    description: Some("Output format: plain text or JSON".into()),
                    r#enum: vec!["text".into(), "json".into()],
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
impl NodeExecutor for AiChatNode {
    async fn execute(&self, input: &NodeInput) -> Result<NodeOutput, OrbflowError> {
        let cfg = resolve_config(input);

        let prompt = string_val(&cfg, "prompt", "");
        if prompt.is_empty() {
            return Err(OrbflowError::InvalidNodeConfig(
                "ai-chat node: prompt is required".into(),
            ));
        }

        let config = AiConfig::from_config(&cfg)?;
        let response_format = string_val(&cfg, "response_format", "text");
        let json_mode = response_format == "json";

        // Extract system prompt if provided.
        let system_prompt = string_val(&cfg, "system_prompt", "");
        let system_prompt_opt: Option<&str> = if system_prompt.is_empty() {
            None
        } else {
            Some(&system_prompt)
        };

        // Build user message, appending context JSON if present.
        let user_content = if let Some(ctx) = cfg.get("context") {
            if !ctx.is_null() {
                let ctx_json = serde_json::to_string_pretty(ctx).unwrap_or_default();
                format!("{prompt}\n\nContext:\n{ctx_json}")
            } else {
                prompt
            }
        } else {
            prompt
        };

        let response = chat_completion(
            &config,
            system_prompt_opt,
            vec![("user".into(), user_content)],
            json_mode,
        )
        .await?;

        let cost = estimate_cost(&config.provider, &config.model, &response.usage);
        let usage_val = usage_to_json(&response.usage);

        // Attempt JSON parse when json_mode is active.
        let parsed_json = if json_mode {
            serde_json::from_str::<Value>(&response.content).ok()
        } else {
            None
        };

        let mut pairs: Vec<(&str, Value)> = vec![
            ("content", Value::String(response.content.clone())),
            ("model", Value::String(response.model.clone())),
            ("usage", usage_val),
            (
                "cost_usd",
                Value::Number(serde_json::Number::from_f64(cost).unwrap_or(0.into())),
            ),
            (
                "finish_reason",
                Value::String(response.finish_reason.clone()),
            ),
        ];

        if let Some(parsed) = parsed_json {
            pairs.push(("parsed_json", parsed));
        }

        Ok(NodeOutput {
            data: Some(make_output(pairs)),
            error: None,
        })
    }
}

#[async_trait]
impl StreamingNodeExecutor for AiChatNode {
    async fn execute_streaming(
        &self,
        input: &NodeInput,
        sender: StreamSender,
    ) -> Result<(), OrbflowError> {
        let cfg = resolve_config(input);

        let prompt = string_val(&cfg, "prompt", "");
        if prompt.is_empty() {
            sender
                .send_error("ai-chat node: prompt is required".into())
                .await?;
            return Ok(());
        }

        let config = match AiConfig::from_config(&cfg) {
            Ok(c) => c,
            Err(e) => {
                sender.send_error(e.to_string()).await?;
                return Ok(());
            }
        };

        let response_format = string_val(&cfg, "response_format", "text");
        let json_mode = response_format == "json";

        let system_prompt = string_val(&cfg, "system_prompt", "");
        let system_prompt_opt: Option<&str> = if system_prompt.is_empty() {
            None
        } else {
            Some(&system_prompt)
        };

        let user_content = if let Some(ctx) = cfg.get("context") {
            if !ctx.is_null() {
                let ctx_json = serde_json::to_string_pretty(ctx).unwrap_or_default();
                format!("{prompt}\n\nContext:\n{ctx_json}")
            } else {
                prompt
            }
        } else {
            prompt
        };

        // For now, use the non-streaming chat_completion and emit the full
        // response as a single data chunk followed by done. True token-by-token
        // streaming (via genai's exec_chat_stream) will be added in Phase 2
        // when we integrate the streaming genai API.
        match chat_completion(
            &config,
            system_prompt_opt,
            vec![("user".into(), user_content)],
            json_mode,
        )
        .await
        {
            Ok(response) => {
                // Emit the content as a data chunk for the UI.
                sender
                    .send_data(serde_json::json!({"token": &response.content}))
                    .await?;

                let cost = estimate_cost(&config.provider, &config.model, &response.usage);
                let usage_val = usage_to_json(&response.usage);
                let _model_id = resolve_model_id(&config);

                let parsed_json = if json_mode {
                    serde_json::from_str::<Value>(&response.content).ok()
                } else {
                    None
                };

                let mut pairs: Vec<(&str, Value)> = vec![
                    ("content", Value::String(response.content.clone())),
                    ("model", Value::String(response.model.clone())),
                    ("usage", usage_val),
                    (
                        "cost_usd",
                        Value::Number(serde_json::Number::from_f64(cost).unwrap_or(0.into())),
                    ),
                    (
                        "finish_reason",
                        Value::String(response.finish_reason.clone()),
                    ),
                ];

                if let Some(parsed) = parsed_json {
                    pairs.push(("parsed_json", parsed));
                }

                sender
                    .send_done(NodeOutput {
                        data: Some(make_output(pairs)),
                        error: None,
                    })
                    .await?;
            }
            Err(e) => {
                sender.send_error(e.to_string()).await?;
            }
        }

        Ok(())
    }
}
