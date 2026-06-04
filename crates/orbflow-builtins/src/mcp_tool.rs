// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! MCP tool node — calls external MCP servers from within workflows.

use std::collections::HashMap;

use async_trait::async_trait;

use orbflow_core::OrbflowError;
use orbflow_core::ports::{
    FieldSchema, FieldType, NodeExecutor, NodeInput, NodeOutput, NodeSchema, NodeSchemaProvider,
};
use orbflow_mcp::schema::{
    ClientInfo, InitializeParams, JsonRpcRequest, McpContent, McpToolResult, ToolCallParams,
};
use orbflow_mcp::transport::{HttpTransport, McpLocalhostPolicy};

/// Validates that an MCP server URL does not point to private/internal addresses (SSRF protection).
async fn validate_mcp_url(url: &str, policy: McpLocalhostPolicy) -> Result<(), OrbflowError> {
    let parsed = url::Url::parse(url)
        .map_err(|e| OrbflowError::InvalidNodeConfig(format!("mcp_tool: invalid URL: {e}")))?;

    let host_is_loopback = match parsed.host() {
        Some(url::Host::Ipv4(v4)) => v4.is_loopback(),
        Some(url::Host::Ipv6(v6)) => v6.is_loopback(),
        _ => parsed
            .host_str()
            .is_some_and(|h| h.eq_ignore_ascii_case("localhost")),
    };

    if parsed.scheme() == "http" && !(policy.allow_localhost() && host_is_loopback) {
        return Err(OrbflowError::InvalidNodeConfig(
            "mcp_tool: server_url must use HTTPS unless allow_localhost is enabled for local dev"
                .into(),
        ));
    }

    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err(OrbflowError::InvalidNodeConfig(
            "mcp_tool: server_url scheme must be http or https".into(),
        ));
    }

    crate::ssrf::validate_url_not_private_async(url, policy.allow_localhost()).await
}

fn config_bool(input: &NodeInput, key: &str) -> Result<bool, OrbflowError> {
    let value = input
        .config
        .as_ref()
        .and_then(|c| c.get(key))
        .or_else(|| input.parameters.as_ref().and_then(|p| p.get(key)));

    match value {
        None => Ok(false),
        Some(serde_json::Value::Bool(value)) => Ok(*value),
        Some(_) => Err(OrbflowError::InvalidNodeConfig(format!(
            "mcp_tool: {key} must be a boolean"
        ))),
    }
}

async fn initialize_mcp(transport: &HttpTransport) -> Result<(), OrbflowError> {
    let params = InitializeParams {
        protocol_version: "2024-11-05".into(),
        capabilities: serde_json::json!({}),
        client_info: ClientInfo {
            name: "orbflow".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        },
    };

    let request = JsonRpcRequest::new(
        "initialize",
        Some(
            serde_json::to_value(&params)
                .map_err(|e| OrbflowError::Internal(format!("mcp_tool: serialize params: {e}")))?,
        ),
    )
    .with_id(1);
    let response = transport.send(&request).await?;
    if let Some(err) = response.error {
        return Err(OrbflowError::Internal(format!(
            "MCP initialize failed: {}",
            err.message
        )));
    }

    let initialized = JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: serde_json::Value::Null,
        method: "notifications/initialized".into(),
        params: None,
    };
    let _ = transport.send(&initialized).await;

    Ok(())
}

async fn call_mcp_tool(
    transport: &HttpTransport,
    tool_name: &str,
    arguments: HashMap<String, serde_json::Value>,
) -> Result<McpToolResult, OrbflowError> {
    let params = ToolCallParams {
        name: tool_name.into(),
        arguments,
    };
    let request = JsonRpcRequest::new(
        "tools/call",
        Some(
            serde_json::to_value(&params)
                .map_err(|e| OrbflowError::Internal(format!("mcp_tool: serialize params: {e}")))?,
        ),
    )
    .with_id(2);

    let response = transport.send(&request).await?;
    if let Some(err) = response.error {
        return Err(OrbflowError::Internal(format!(
            "MCP tools/call '{}' failed: {}",
            tool_name, err.message
        )));
    }

    let result = response.result.unwrap_or(serde_json::Value::Null);
    serde_json::from_value::<McpToolResult>(result)
        .map_err(|e| OrbflowError::Internal(format!("MCP result parse error: {e}")))
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
        let allow_localhost = config_bool(input, "allow_localhost")?;
        let localhost_policy = if allow_localhost {
            McpLocalhostPolicy::AllowForDev
        } else {
            McpLocalhostPolicy::Deny
        };

        // Validate URL to prevent SSRF against internal services.
        validate_mcp_url(server_url, localhost_policy).await?;

        // Connect to MCP server.
        let transport = HttpTransport::new_with_localhost_policy(server_url, localhost_policy)?;
        initialize_mcp(&transport).await?;

        // Call the tool.
        let result = call_mcp_tool(&transport, tool_name, arguments).await?;

        // Collect text content from the result.
        let text_content: Vec<String> = result
            .content
            .iter()
            .filter_map(|c| match c {
                McpContent::Text { text } => Some(text.clone()),
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
            settings: vec![FieldSchema {
                key: "allow_localhost".into(),
                label: "Allow Localhost".into(),
                field_type: FieldType::Boolean,
                required: false,
                description: Some("Allow HTTP loopback MCP servers for local development".into()),
                default: Some(serde_json::json!(false)),
                r#enum: vec![],
                credential_type: None,
            }],
            provides_capability: None,
        }
    }
}
