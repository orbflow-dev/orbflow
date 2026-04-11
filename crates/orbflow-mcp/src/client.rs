// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! MCP client — connects to an MCP server, lists tools, calls tools.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use orbflow_core::OrbflowError;

use crate::schema::{
    ClientInfo, InitializeParams, JsonRpcRequest, McpTool, McpToolResult, ToolCallParams,
};
use crate::transport::HttpTransport;

/// Client for interacting with an MCP-compatible server.
pub struct McpClient {
    transport: HttpTransport,
    request_id: AtomicU64,
    initialized: bool,
}

impl McpClient {
    /// Create a new MCP client connected to the given server URL.
    ///
    /// Returns an error if the server URL points to a blocked address.
    pub fn new(server_url: impl Into<String>) -> Result<Self, OrbflowError> {
        Ok(Self {
            transport: HttpTransport::new(server_url)?,
            request_id: AtomicU64::new(1),
            initialized: false,
        })
    }

    fn next_id(&self) -> u64 {
        self.request_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Initialize the MCP connection (handshake).
    pub async fn initialize(&mut self) -> Result<serde_json::Value, OrbflowError> {
        let params = InitializeParams {
            protocol_version: "2024-11-05".into(),
            capabilities: serde_json::json!({}),
            client_info: ClientInfo {
                name: "orbflow".into(),
                version: env!("CARGO_PKG_VERSION").into(),
            },
        };

        let req = JsonRpcRequest::new(
            "initialize",
            Some(
                serde_json::to_value(&params)
                    .map_err(|e| OrbflowError::Internal(format!("mcp: serialize params: {e}")))?,
            ),
        )
        .with_id(self.next_id());
        let resp = self.transport.send(&req).await?;

        if let Some(err) = resp.error {
            return Err(OrbflowError::Internal(format!(
                "MCP initialize failed: {}",
                err.message
            )));
        }

        // Send initialized notification (fire and forget).
        let notif = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: serde_json::Value::Null,
            method: "notifications/initialized".into(),
            params: None,
        };
        let _ = self.transport.send(&notif).await;

        self.initialized = true;
        Ok(resp.result.unwrap_or(serde_json::Value::Null))
    }

    /// List all available tools from the MCP server.
    pub async fn list_tools(&self) -> Result<Vec<McpTool>, OrbflowError> {
        let req = JsonRpcRequest::new("tools/list", None).with_id(self.next_id());
        let resp = self.transport.send(&req).await?;

        if let Some(err) = resp.error {
            return Err(OrbflowError::Internal(format!(
                "MCP tools/list failed: {}",
                err.message
            )));
        }

        let result = resp.result.unwrap_or(serde_json::Value::Null);
        let tools: Vec<McpTool> = result
            .get("tools")
            .and_then(|t| serde_json::from_value(t.clone()).ok())
            .unwrap_or_default();

        Ok(tools)
    }

    /// Call a tool on the MCP server.
    pub async fn call_tool(
        &self,
        name: &str,
        arguments: HashMap<String, serde_json::Value>,
    ) -> Result<McpToolResult, OrbflowError> {
        let params = ToolCallParams {
            name: name.into(),
            arguments,
        };

        let req = JsonRpcRequest::new(
            "tools/call",
            Some(
                serde_json::to_value(&params)
                    .map_err(|e| OrbflowError::Internal(format!("mcp: serialize params: {e}")))?,
            ),
        )
        .with_id(self.next_id());
        let resp = self.transport.send(&req).await?;

        if let Some(err) = resp.error {
            return Err(OrbflowError::Internal(format!(
                "MCP tools/call '{}' failed: {}",
                name, err.message
            )));
        }

        let result = resp.result.unwrap_or(serde_json::Value::Null);
        serde_json::from_value::<McpToolResult>(result)
            .map_err(|e| OrbflowError::Internal(format!("MCP result parse error: {e}")))
    }
}
