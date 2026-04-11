// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Log node: logs input data and passes through.

use async_trait::async_trait;
use tracing::info;

use orbflow_core::OrbflowError;
use orbflow_core::ports::{
    FieldSchema, FieldType, NodeExecutor, NodeInput, NodeOutput, NodeSchema, NodeSchemaProvider,
};

use crate::util::{resolve_config, string_val};

/// A pass-through node that logs its input and returns it as output.
pub struct LogNode;

impl NodeSchemaProvider for LogNode {
    fn node_schema(&self) -> NodeSchema {
        NodeSchema {
            plugin_ref: "builtin:log".into(),
            name: "Log".into(),
            description: "Log a message and pass input through as output".into(),
            category: "builtin".into(),
            node_kind: None,
            icon: "terminal".into(),
            color: "#22d3ee".into(),
            image_url: Some("/icons/terminal.svg".into()),
            docs: None,
            inputs: vec![FieldSchema {
                key: "message".into(),
                label: "Message".into(),
                field_type: FieldType::String,
                required: false,
                default: None,
                description: Some("Message to log".into()),
                r#enum: vec![],
                credential_type: None,
            }],
            outputs: vec![FieldSchema {
                key: "message".into(),
                label: "Message".into(),
                field_type: FieldType::String,
                required: false,
                default: None,
                description: None,
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
impl NodeExecutor for LogNode {
    async fn execute(&self, input: &NodeInput) -> Result<NodeOutput, OrbflowError> {
        let cfg = resolve_config(input);

        const MAX_LOG_MSG_LEN: usize = 1024;
        let raw = string_val(&cfg, "message", "log node");
        let msg: String = raw
            .chars()
            .filter(|c| !c.is_control() || *c == ' ')
            .take(MAX_LOG_MSG_LEN)
            .collect();
        info!(
            node_id = %input.node_id,
            instance_id = %input.instance_id,
            message = %msg,
            "orbflow:log"
        );

        Ok(NodeOutput {
            data: Some(cfg),
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    use orbflow_core::execution::InstanceId;

    #[tokio::test]
    async fn test_log_passthrough() {
        let node = LogNode;
        let mut config = HashMap::new();
        config.insert("message".into(), serde_json::json!("hello world"));

        let input = NodeInput {
            instance_id: InstanceId::new("inst-1"),
            node_id: "log-1".into(),
            plugin_ref: "builtin:log".into(),
            config: Some(config),
            input: None,
            parameters: None,
            capabilities: None,
            attempt: 1,
        };

        let output = node.execute(&input).await.unwrap();
        let data = output.data.unwrap();
        assert_eq!(data.get("message").unwrap(), "hello world");
    }

    #[test]
    fn test_log_schema() {
        let node = LogNode;
        let schema = node.node_schema();
        assert_eq!(schema.plugin_ref, "builtin:log");
    }
}
