// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! JSON protocol for communicating with external plugin subprocesses.
//!
//! Each plugin binary is a standalone process that reads JSON requests from
//! stdin and writes JSON responses to stdout. This avoids the complexity of
//! a full gRPC handshake while keeping plugins language-agnostic.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Request sent to a plugin subprocess via stdin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteRequest {
    /// Unique instance identifier.
    pub instance_id: String,
    /// Node identifier within the workflow.
    pub node_id: String,
    /// Plugin reference (e.g., "myplugin:action-name").
    pub plugin_ref: String,
    /// Node configuration (static values).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<HashMap<String, serde_json::Value>>,
    /// Evaluated input from upstream nodes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<HashMap<String, serde_json::Value>>,
    /// Parameter values.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<HashMap<String, serde_json::Value>>,
    /// Capability values (e.g., database connections).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<HashMap<String, serde_json::Value>>,
    /// Retry attempt number (0-based).
    #[serde(default)]
    pub attempt: i32,
}

/// Response received from a plugin subprocess via stdout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteResponse {
    /// Output data on success.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<HashMap<String, serde_json::Value>>,
    /// Error message on failure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl From<&orbflow_core::ports::NodeInput> for ExecuteRequest {
    fn from(input: &orbflow_core::ports::NodeInput) -> Self {
        Self {
            instance_id: input.instance_id.0.clone(),
            node_id: input.node_id.clone(),
            plugin_ref: input.plugin_ref.clone(),
            config: input.config.clone(),
            input: input.input.clone(),
            parameters: input.parameters.clone(),
            capabilities: input.capabilities.clone(),
            attempt: input.attempt,
        }
    }
}

impl From<ExecuteResponse> for orbflow_core::ports::NodeOutput {
    fn from(resp: ExecuteResponse) -> Self {
        Self {
            data: resp.data,
            error: resp.error,
        }
    }
}
