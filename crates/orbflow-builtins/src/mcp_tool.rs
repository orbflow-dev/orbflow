// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! MCP tool node — calls external MCP servers from within workflows.

use std::collections::HashMap;

use async_trait::async_trait;

use orbflow_core::OrbflowError;
use orbflow_core::ports::{
    FieldSchema, FieldType, NodeExecutor, NodeInput, NodeOutput, NodeSchema, NodeSchemaProvider,
};
use orbflow_mcp::client::McpClient;

/// Validates that an MCP server URL does not point to private/internal addresses (SSRF protection).
/// Allows localhost for MCP development servers.
async fn validate_mcp_url(url: &str) -> Result<(), OrbflowError> {
    crate::ssrf::validate_url_not_private_async(url, true).await
}

/// Builtin node that calls an MCP tool on an external server.
pub struct McpToolNode;

#[async_trait]
impl NodeExecutor for McpToolNode {
    async fn execute(&self, input: &NodeInput) -> Result<NodeOutput, OrbflowError> {
        // Extract config values (config or parameters).
        let server_url = input
            .config
            .as_ref()
            .and_then(|c| c.get("server_url"))
            .or_else(|| input.parameters.as_ref().and_then(|p| p.get("server_url")))
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                OrbflowError::InvalidNodeConfig("mcp_tool: server_url is required".into())
            })?;

        let tool_name = input
            .config
            .as_ref()
            .and_then(|c| c.get("tool_name"))
            .or_else(|| input.parameters.as_ref().and_then(|p| p.get("tool_name")))
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                OrbflowError::InvalidNodeConfig("mcp_tool: tool_name is required".into())
            })?;

        // Build arguments from input mapping.
        let arguments: HashMap<String, serde_json::Value> = input.input.clone().unwrap_or_default();

        // Validate URL to prevent SSRF against internal services.
        validate_mcp_url(server_url).await?;

        // Connect to MCP server.
        let mut client = McpClient::new(server_url)?;
        client.initialize().await?;

        // Call the tool.
        let result = client.call_tool(tool_name, arguments).await?;

        // Collect text content from the result.
        let text_content: Vec<String> = result
            .content
            .iter()
            .filter_map(|c| match c {
                orbflow_mcp::schema::McpContent::Text { text } => Some(text.clone()),
                _ => None,
            })
            .collect();

        let mut output_data = HashMap::new();
        output_data.insert("content".into(), serde_json::json!(text_content.join("\n")));
        output_data.insert("is_error".into(), serde_json::json!(result.is_error));
        output_data.insert(
            "raw_content".into(),
            serde_json::to_value(&result.content).unwrap_or_default(),
        );

        if result.is_error {
            Ok(NodeOutput {
                data: Some(output_data),
                error: Some(text_content.join("\n")),
            })
        } else {
            Ok(NodeOutput {
                data: Some(output_data),
                error: None,
            })
        }
    }
}

impl NodeSchemaProvider for McpToolNode {
    fn node_schema(&self) -> NodeSchema {
        NodeSchema {
            plugin_ref: "builtin:mcp_tool".into(),
            name: "MCP Tool".into(),
            description: "Call a tool on an external MCP server".into(),
            icon: "plug".into(),
            color: "#8B5CF6".into(),
            category: "AI & MCP".into(),
            node_kind: None,
            docs: None,
            image_url: None,
            inputs: vec![
                FieldSchema {
                    key: "server_url".into(),
                    label: "MCP Server URL".into(),
                    field_type: FieldType::String,
                    required: true,
                    description: Some("URL of the MCP server (HTTP transport)".into()),
                    default: None,
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "tool_name".into(),
                    label: "Tool Name".into(),
                    field_type: FieldType::String,
                    required: true,
                    description: Some("Name of the MCP tool to call".into()),
                    default: None,
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
                    description: Some("Text content returned by the tool".into()),
                    default: None,
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "is_error".into(),
                    label: "Is Error".into(),
                    field_type: FieldType::Boolean,
                    required: false,
                    description: Some("Whether the tool call returned an error".into()),
                    default: None,
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "raw_content".into(),
                    label: "Raw Content".into(),
                    field_type: FieldType::Object,
                    required: false,
                    description: Some("Full MCP content blocks (text and images)".into()),
                    default: None,
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
