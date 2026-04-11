// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Trigger schema nodes: TriggerManual, TriggerCron, TriggerWebhook, TriggerEvent.
//!
//! These are schema-only nodes (NodeSchemaProvider + minimal NodeExecutor).
//! The engine resolves triggers inline; the schemas are served via the API
//! so the frontend can offer them in the node picker.

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::Utc;
use serde_json::Value;

use orbflow_core::OrbflowError;
use orbflow_core::ports::{
    FieldSchema, FieldType, NodeExecutor, NodeInput, NodeOutput, NodeSchema, NodeSchemaProvider,
};
use orbflow_core::workflow::NodeKind;

use crate::util::{make_output, resolve_config, string_val};

// ---------------------------------------------------------------------------
// TriggerManual
// ---------------------------------------------------------------------------

/// Manual trigger — the workflow is started explicitly by a user or API call.
pub struct TriggerManual;

impl NodeSchemaProvider for TriggerManual {
    fn node_schema(&self) -> NodeSchema {
        NodeSchema {
            plugin_ref: "builtin:trigger-manual".into(),
            name: "Manual Trigger".into(),
            description: "Manually start a workflow".into(),
            category: "builtin".into(),
            node_kind: Some(NodeKind::Trigger),
            icon: "play".into(),
            color: "#10B981".into(),
            image_url: Some("/icons/play.svg".into()),
            docs: None,
            inputs: vec![],
            outputs: vec![FieldSchema {
                key: "triggered_at".into(),
                label: "Triggered At".into(),
                field_type: FieldType::String,
                required: false,
                default: None,
                description: Some("ISO 8601 timestamp".into()),
                r#enum: vec![],
                credential_type: None,
            }],
            parameters: vec![],
            capability_ports: vec![],
            settings: vec![],
            provides_capability: None,
        }
    }
}

#[async_trait]
impl NodeExecutor for TriggerManual {
    async fn execute(&self, _input: &NodeInput) -> Result<NodeOutput, OrbflowError> {
        Ok(NodeOutput {
            data: Some(make_output(vec![(
                "triggered_at",
                Value::String(Utc::now().to_rfc3339()),
            )])),
            error: None,
        })
    }
}

// ---------------------------------------------------------------------------
// TriggerCron
// ---------------------------------------------------------------------------

/// Schedule-based trigger — runs on a cron schedule.
pub struct TriggerCron;

impl NodeSchemaProvider for TriggerCron {
    fn node_schema(&self) -> NodeSchema {
        NodeSchema {
            plugin_ref: "builtin:trigger-cron".into(),
            name: "Schedule".into(),
            description: "Run on a time-based schedule".into(),
            category: "builtin".into(),
            node_kind: Some(NodeKind::Trigger),
            icon: "clock".into(),
            color: "#F59E0B".into(),
            image_url: Some("/icons/clock.svg".into()),
            docs: None,
            inputs: vec![],
            outputs: vec![
                FieldSchema {
                    key: "scheduled_time".into(),
                    label: "Scheduled Time".into(),
                    field_type: FieldType::String,
                    required: false,
                    default: None,
                    description: Some("ISO 8601 timestamp of the scheduled run".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "cron_expression".into(),
                    label: "Cron Expression".into(),
                    field_type: FieldType::String,
                    required: false,
                    default: None,
                    description: Some("The cron expression that triggered this run".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
            ],
            parameters: vec![FieldSchema {
                key: "cron".into(),
                label: "Cron Expression".into(),
                field_type: FieldType::String,
                required: true,
                default: None,
                description: Some("Cron schedule (e.g. '0 */5 * * *')".into()),
                r#enum: vec![],
                credential_type: None,
            }],
            capability_ports: vec![],
            settings: vec![],
            provides_capability: None,
        }
    }
}

#[async_trait]
impl NodeExecutor for TriggerCron {
    async fn execute(&self, input: &NodeInput) -> Result<NodeOutput, OrbflowError> {
        let cfg = resolve_config(input);
        let cron = string_val(&cfg, "cron", "");

        Ok(NodeOutput {
            data: Some(make_output(vec![
                ("scheduled_time", Value::String(Utc::now().to_rfc3339())),
                ("cron_expression", Value::String(cron)),
            ])),
            error: None,
        })
    }
}

// ---------------------------------------------------------------------------
// TriggerWebhook
// ---------------------------------------------------------------------------

/// Webhook trigger — runs when an HTTP request arrives at the webhook endpoint.
pub struct TriggerWebhook;

impl NodeSchemaProvider for TriggerWebhook {
    fn node_schema(&self) -> NodeSchema {
        NodeSchema {
            plugin_ref: "builtin:trigger-webhook".into(),
            name: "Webhook".into(),
            description: "Run when an HTTP request is received".into(),
            category: "builtin".into(),
            node_kind: Some(NodeKind::Trigger),
            icon: "webhook".into(),
            color: "#6366F1".into(),
            image_url: Some("/icons/webhook.svg".into()),
            docs: None,
            inputs: vec![],
            outputs: vec![
                FieldSchema {
                    key: "body".into(),
                    label: "Request Body".into(),
                    field_type: FieldType::Object,
                    required: false,
                    default: None,
                    description: Some("HTTP request body".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "headers".into(),
                    label: "Request Headers".into(),
                    field_type: FieldType::Object,
                    required: false,
                    default: None,
                    description: Some("HTTP request headers".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "method".into(),
                    label: "HTTP Method".into(),
                    field_type: FieldType::String,
                    required: false,
                    default: None,
                    description: Some("HTTP method used".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "path".into(),
                    label: "Path".into(),
                    field_type: FieldType::String,
                    required: false,
                    default: None,
                    description: Some("Webhook path".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
            ],
            parameters: vec![FieldSchema {
                key: "path".into(),
                label: "Webhook Path".into(),
                field_type: FieldType::String,
                required: false,
                default: None,
                description: Some("Custom webhook path".into()),
                r#enum: vec![],
                credential_type: None,
            }],
            capability_ports: vec![],
            settings: vec![],
            provides_capability: None,
        }
    }
}

#[async_trait]
impl NodeExecutor for TriggerWebhook {
    async fn execute(&self, input: &NodeInput) -> Result<NodeOutput, OrbflowError> {
        // The actual webhook data is injected by the trigger system.
        // This executor just passes through whatever it receives.
        let cfg = resolve_config(input);
        let mut data = HashMap::new();

        // Pass through any trigger payload data.
        if let Some(body) = cfg.get("body") {
            data.insert("body".into(), body.clone());
        } else {
            data.insert("body".into(), Value::Object(serde_json::Map::new()));
        }
        if let Some(headers) = cfg.get("headers") {
            data.insert("headers".into(), headers.clone());
        } else {
            data.insert("headers".into(), Value::Object(serde_json::Map::new()));
        }
        if let Some(method) = cfg.get("method") {
            data.insert("method".into(), method.clone());
        } else {
            data.insert("method".into(), Value::String("POST".into()));
        }
        if let Some(path) = cfg.get("path") {
            data.insert("path".into(), path.clone());
        } else {
            data.insert("path".into(), Value::String("".into()));
        }

        Ok(NodeOutput {
            data: Some(data),
            error: None,
        })
    }
}

// ---------------------------------------------------------------------------
// TriggerEvent
// ---------------------------------------------------------------------------

/// Event trigger — runs when a named event is published.
pub struct TriggerEvent;

impl NodeSchemaProvider for TriggerEvent {
    fn node_schema(&self) -> NodeSchema {
        NodeSchema {
            plugin_ref: "builtin:trigger-event".into(),
            name: "Event".into(),
            description: "Run when a named event is published".into(),
            category: "builtin".into(),
            node_kind: Some(NodeKind::Trigger),
            icon: "zap".into(),
            color: "#EF4444".into(),
            image_url: Some("/icons/zap.svg".into()),
            docs: None,
            inputs: vec![],
            outputs: vec![
                FieldSchema {
                    key: "event_name".into(),
                    label: "Event Name".into(),
                    field_type: FieldType::String,
                    required: false,
                    default: None,
                    description: Some("The event that was fired".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "payload".into(),
                    label: "Event Payload".into(),
                    field_type: FieldType::Object,
                    required: false,
                    default: None,
                    description: Some("Event payload data".into()),
                    r#enum: vec![],
                    credential_type: None,
                },
            ],
            parameters: vec![FieldSchema {
                key: "event_name".into(),
                label: "Event Name".into(),
                field_type: FieldType::String,
                required: true,
                default: None,
                description: Some("Name of the event to listen for".into()),
                r#enum: vec![],
                credential_type: None,
            }],
            capability_ports: vec![],
            settings: vec![],
            provides_capability: None,
        }
    }
}

#[async_trait]
impl NodeExecutor for TriggerEvent {
    async fn execute(&self, input: &NodeInput) -> Result<NodeOutput, OrbflowError> {
        let cfg = resolve_config(input);
        let event_name = string_val(&cfg, "event_name", "");
        let payload = cfg
            .get("payload")
            .cloned()
            .unwrap_or(Value::Object(serde_json::Map::new()));

        Ok(NodeOutput {
            data: Some(make_output(vec![
                ("event_name", Value::String(event_name)),
                ("payload", payload),
            ])),
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orbflow_core::execution::InstanceId;

    fn empty_input(plugin_ref: &str) -> NodeInput {
        NodeInput {
            instance_id: InstanceId::new("inst-1"),
            node_id: "trigger-1".into(),
            plugin_ref: plugin_ref.into(),
            config: None,
            input: None,
            parameters: None,
            capabilities: None,
            attempt: 1,
        }
    }

    #[test]
    fn test_trigger_manual_schema() {
        let t = TriggerManual;
        let s = t.node_schema();
        assert_eq!(s.plugin_ref, "builtin:trigger-manual");
        assert_eq!(s.node_kind, Some(NodeKind::Trigger));
    }

    #[test]
    fn test_trigger_cron_schema() {
        let t = TriggerCron;
        let s = t.node_schema();
        assert_eq!(s.plugin_ref, "builtin:trigger-cron");
        assert_eq!(s.node_kind, Some(NodeKind::Trigger));
        assert!(!s.parameters.is_empty());
    }

    #[test]
    fn test_trigger_webhook_schema() {
        let t = TriggerWebhook;
        let s = t.node_schema();
        assert_eq!(s.plugin_ref, "builtin:trigger-webhook");
        assert_eq!(s.node_kind, Some(NodeKind::Trigger));
    }

    #[test]
    fn test_trigger_event_schema() {
        let t = TriggerEvent;
        let s = t.node_schema();
        assert_eq!(s.plugin_ref, "builtin:trigger-event");
        assert_eq!(s.node_kind, Some(NodeKind::Trigger));
    }

    #[tokio::test]
    async fn test_trigger_manual_execute() {
        let t = TriggerManual;
        let output = t
            .execute(&empty_input("builtin:trigger-manual"))
            .await
            .unwrap();
        let data = output.data.unwrap();
        assert!(data.contains_key("triggered_at"));
    }

    #[tokio::test]
    async fn test_trigger_cron_execute() {
        let t = TriggerCron;
        let mut cfg = HashMap::new();
        cfg.insert("cron".into(), serde_json::json!("0 */5 * * *"));
        let input = NodeInput {
            instance_id: InstanceId::new("inst-1"),
            node_id: "trigger-1".into(),
            plugin_ref: "builtin:trigger-cron".into(),
            config: Some(cfg),
            input: None,
            parameters: None,
            capabilities: None,
            attempt: 1,
        };
        let output = t.execute(&input).await.unwrap();
        let data = output.data.unwrap();
        assert_eq!(data.get("cron_expression").unwrap(), "0 */5 * * *");
    }
}
