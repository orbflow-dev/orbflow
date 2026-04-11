// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! gRPC-based plugin executor.
//!
//! [`GrpcPluginExecutor`] implements [`NodeExecutor`] by forwarding calls to a
//! remote plugin server over gRPC. The connection uses `connect_lazy()` so
//! the worker can start even if the plugin is temporarily unavailable, and
//! tonic manages reconnection automatically via keepalive.
//!
//! # Security
//!
//! By default, connections use plaintext HTTP/2. This is acceptable for
//! loopback addresses in trusted environments. For production deployments
//! across network boundaries, configure TLS via [`GrpcPluginConfig`] in
//! `orbflow-config`.

use std::collections::HashMap;
use std::time::Duration;

use async_trait::async_trait;
use tonic::transport::Channel;

use orbflow_core::error::OrbflowError;
use orbflow_core::ports::{FieldSchema, NodeExecutor, NodeInput, NodeOutput, NodeSchema};
use orbflow_core::workflow::CapabilityPort;

use crate::grpc_proto::{
    ExecuteRequest as ProtoExecuteRequest, GetSchemasRequest,
    orbflow_plugin_client::OrbflowPluginClient,
};

/// Default timeout for gRPC RPC calls (30 seconds).
const DEFAULT_RPC_TIMEOUT: Duration = Duration::from_secs(30);

/// Default connect timeout (5 seconds).
const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// Default keepalive interval (30 seconds).
const DEFAULT_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(30);

/// A [`NodeExecutor`] that delegates to a remote gRPC plugin server.
///
/// Uses `connect_lazy()` with keepalive so the worker starts immediately
/// and tonic handles reconnection automatically when the plugin restarts.
/// All RPC calls have a configurable timeout to prevent indefinite blocking.
pub struct GrpcPluginExecutor {
    /// Display name of this plugin (for logging).
    name: String,
    /// Lazily-connected gRPC channel with auto-reconnect.
    channel: Channel,
    /// Timeout applied to each RPC call.
    rpc_timeout: Duration,
}

impl GrpcPluginExecutor {
    /// Creates a new gRPC plugin executor with a lazy channel.
    ///
    /// The TCP connection is **not** established until the first RPC call.
    /// If the plugin is down, RPCs will fail with a transport error but the
    /// worker remains operational.
    pub fn new(name: impl Into<String>, address: impl Into<String>) -> Result<Self, OrbflowError> {
        Self::with_timeout(name, address, DEFAULT_RPC_TIMEOUT)
    }

    /// Creates a new gRPC plugin executor with a custom RPC timeout.
    pub fn with_timeout(
        name: impl Into<String>,
        address: impl Into<String>,
        rpc_timeout: Duration,
    ) -> Result<Self, OrbflowError> {
        let name = name.into();
        let address = address.into();

        let channel = Channel::from_shared(address.clone())
            .map_err(|e| {
                OrbflowError::Internal(format!(
                    "grpc plugin {name}: invalid address {address}: {e}"
                ))
            })?
            .connect_timeout(DEFAULT_CONNECT_TIMEOUT)
            .keep_alive_timeout(DEFAULT_KEEPALIVE_INTERVAL)
            .connect_lazy();

        tracing::info!(
            plugin = %name,
            address = %address,
            timeout_secs = rpc_timeout.as_secs(),
            "created lazy gRPC plugin channel"
        );

        Ok(Self {
            name,
            channel,
            rpc_timeout,
        })
    }

    /// Returns the plugin name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Fetches node schemas from the remote plugin via the `GetSchemas` RPC.
    pub async fn fetch_schemas(&self) -> Result<Vec<NodeSchema>, OrbflowError> {
        let mut client = OrbflowPluginClient::new(self.channel.clone());

        let mut request = tonic::Request::new(GetSchemasRequest {});
        request.set_timeout(self.rpc_timeout);

        let response = client.get_schemas(request).await.map_err(|e| {
            OrbflowError::Internal(format!("grpc plugin {}: GetSchemas failed: {e}", self.name))
        })?;

        let proto_schemas = response.into_inner().schemas;
        let mut schemas = Vec::with_capacity(proto_schemas.len());

        for ps in proto_schemas {
            let schema = proto_schema_to_node_schema(ps).map_err(|e| {
                OrbflowError::Internal(format!("grpc plugin {}: parse schema: {e}", self.name))
            })?;
            schemas.push(schema);
        }

        tracing::info!(
            plugin = %self.name,
            count = schemas.len(),
            "fetched schemas from gRPC plugin"
        );

        Ok(schemas)
    }

    /// Performs a health check against the remote plugin.
    pub async fn health_check(&self) -> Result<(bool, String), OrbflowError> {
        let mut client = OrbflowPluginClient::new(self.channel.clone());

        let mut request = tonic::Request::new(crate::grpc_proto::HealthCheckRequest {});
        request.set_timeout(self.rpc_timeout);

        let response = client.health_check(request).await.map_err(|e| {
            OrbflowError::Internal(format!(
                "grpc plugin {}: HealthCheck failed: {e}",
                self.name
            ))
        })?;

        let inner = response.into_inner();
        Ok((inner.healthy, inner.version))
    }
}

#[async_trait]
impl NodeExecutor for GrpcPluginExecutor {
    async fn execute(&self, input: &NodeInput) -> Result<NodeOutput, OrbflowError> {
        let mut client = OrbflowPluginClient::new(self.channel.clone());

        let proto_req = node_input_to_proto(input)?;
        let mut request = tonic::Request::new(proto_req);
        request.set_timeout(self.rpc_timeout);

        let response = client.execute(request).await.map_err(|e| {
            OrbflowError::Internal(format!(
                "grpc plugin {}: Execute RPC failed: {e}",
                self.name
            ))
        })?;

        let proto_resp = response.into_inner();
        proto_response_to_node_output(proto_resp)
    }
}

// ---------------------------------------------------------------------------
// Conversion helpers
// ---------------------------------------------------------------------------

/// Converts a [`NodeInput`] into a protobuf [`ExecuteRequest`].
fn node_input_to_proto(input: &NodeInput) -> Result<ProtoExecuteRequest, OrbflowError> {
    let serialize_opt =
        |opt: &Option<HashMap<String, serde_json::Value>>| -> Result<String, OrbflowError> {
            match opt {
                Some(map) => serde_json::to_string(map)
                    .map_err(|e| OrbflowError::Internal(format!("grpc: serialize field: {e}"))),
                None => Ok(String::new()),
            }
        };

    Ok(ProtoExecuteRequest {
        instance_id: input.instance_id.0.clone(),
        node_id: input.node_id.clone(),
        plugin_ref: input.plugin_ref.clone(),
        config_json: serialize_opt(&input.config)?,
        input_json: serialize_opt(&input.input)?,
        parameters_json: serialize_opt(&input.parameters)?,
        capabilities_json: serialize_opt(&input.capabilities)?,
        attempt: input.attempt,
    })
}

/// Converts a protobuf [`ExecuteResponse`] into a [`NodeOutput`].
fn proto_response_to_node_output(
    resp: crate::grpc_proto::ExecuteResponse,
) -> Result<NodeOutput, OrbflowError> {
    let data =
        if resp.data_json.is_empty() {
            None
        } else {
            Some(serde_json::from_str(&resp.data_json).map_err(|e| {
                OrbflowError::Internal(format!("grpc: parse response data_json: {e}"))
            })?)
        };

    let error = if resp.error.is_empty() {
        None
    } else {
        Some(resp.error)
    };

    Ok(NodeOutput { data, error })
}

/// Converts a protobuf [`NodeSchema`] into the domain [`NodeSchema`].
fn proto_schema_to_node_schema(ps: crate::grpc_proto::NodeSchema) -> Result<NodeSchema, String> {
    let parse_json_vec = |json: &str| -> Result<Vec<FieldSchema>, String> {
        if json.is_empty() {
            return Ok(Vec::new());
        }
        serde_json::from_str(json).map_err(|e| format!("parse field schemas: {e}"))
    };

    let parse_capability_ports = |json: &str| -> Result<Vec<CapabilityPort>, String> {
        if json.is_empty() {
            return Ok(Vec::new());
        }
        serde_json::from_str(json).map_err(|e| format!("parse capability ports: {e}"))
    };

    let node_kind = if ps.node_kind.is_empty() {
        None
    } else {
        Some(
            serde_json::from_value(serde_json::Value::String(ps.node_kind.clone()))
                .map_err(|e| format!("parse node_kind '{}': {e}", ps.node_kind))?,
        )
    };

    Ok(NodeSchema {
        plugin_ref: ps.plugin_ref,
        name: ps.name,
        description: ps.description,
        category: ps.category,
        icon: ps.icon,
        color: ps.color,
        node_kind,
        docs: non_empty(ps.docs),
        image_url: non_empty(ps.image_url),
        inputs: parse_json_vec(&ps.inputs_json)?,
        outputs: parse_json_vec(&ps.outputs_json)?,
        parameters: parse_json_vec(&ps.parameters_json)?,
        capability_ports: parse_capability_ports(&ps.capability_ports_json)?,
        settings: parse_json_vec(&ps.settings_json)?,
        provides_capability: non_empty(ps.provides_capability),
    })
}

/// Returns `None` if the string is empty, `Some(s)` otherwise.
fn non_empty(s: String) -> Option<String> {
    if s.is_empty() { None } else { Some(s) }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use orbflow_core::execution::InstanceId;
    use orbflow_core::ports::FieldType;

    #[test]
    fn test_node_input_to_proto_empty_fields() {
        let input = NodeInput {
            instance_id: InstanceId("inst-1".into()),
            node_id: "node-1".into(),
            plugin_ref: "plugin:action".into(),
            config: None,
            input: None,
            parameters: None,
            capabilities: None,
            attempt: 0,
        };

        let proto = node_input_to_proto(&input).unwrap();
        assert_eq!(proto.instance_id, "inst-1");
        assert_eq!(proto.node_id, "node-1");
        assert_eq!(proto.plugin_ref, "plugin:action");
        assert!(proto.config_json.is_empty());
        assert!(proto.input_json.is_empty());
        assert_eq!(proto.attempt, 0);
    }

    #[test]
    fn test_node_input_to_proto_with_data() {
        let mut config = HashMap::new();
        config.insert("url".into(), serde_json::json!("https://example.com"));

        let input = NodeInput {
            instance_id: InstanceId("inst-2".into()),
            node_id: "http-1".into(),
            plugin_ref: "plugin:http".into(),
            config: Some(config),
            input: None,
            parameters: None,
            capabilities: None,
            attempt: 2,
        };

        let proto = node_input_to_proto(&input).unwrap();
        assert_eq!(proto.attempt, 2);
        assert!(proto.config_json.contains("example.com"));
    }

    #[test]
    fn test_proto_response_to_node_output_success() {
        let resp = crate::grpc_proto::ExecuteResponse {
            data_json: r#"{"result":"ok"}"#.into(),
            error: String::new(),
        };

        let output = proto_response_to_node_output(resp).unwrap();
        assert!(output.error.is_none());
        assert_eq!(
            output.data.unwrap().get("result").unwrap(),
            &serde_json::json!("ok")
        );
    }

    #[test]
    fn test_proto_response_to_node_output_error() {
        let resp = crate::grpc_proto::ExecuteResponse {
            data_json: String::new(),
            error: "something failed".into(),
        };

        let output = proto_response_to_node_output(resp).unwrap();
        assert_eq!(output.error.as_deref(), Some("something failed"));
        assert!(output.data.is_none());
    }

    #[test]
    fn test_proto_response_to_node_output_empty() {
        let resp = crate::grpc_proto::ExecuteResponse {
            data_json: String::new(),
            error: String::new(),
        };

        let output = proto_response_to_node_output(resp).unwrap();
        assert!(output.error.is_none());
        assert!(output.data.is_none());
    }

    #[test]
    fn test_proto_schema_to_node_schema_minimal() {
        let ps = crate::grpc_proto::NodeSchema {
            plugin_ref: "plugin:sentiment".into(),
            name: "Sentiment Analysis".into(),
            description: "Analyzes text sentiment".into(),
            category: "AI".into(),
            icon: "brain".into(),
            color: "#6366f1".into(),
            node_kind: String::new(),
            docs: String::new(),
            image_url: String::new(),
            inputs_json: r#"[{"key":"text","label":"Text","type":"string","required":true}]"#
                .into(),
            outputs_json: r#"[{"key":"sentiment","label":"Sentiment","type":"string"}]"#.into(),
            parameters_json: String::new(),
            capability_ports_json: String::new(),
            settings_json: String::new(),
            provides_capability: String::new(),
        };

        let schema = proto_schema_to_node_schema(ps).unwrap();
        assert_eq!(schema.plugin_ref, "plugin:sentiment");
        assert_eq!(schema.name, "Sentiment Analysis");
        assert_eq!(schema.inputs.len(), 1);
        assert_eq!(schema.inputs[0].key, "text");
        assert_eq!(schema.inputs[0].field_type, FieldType::String);
        assert!(schema.inputs[0].required);
        assert_eq!(schema.outputs.len(), 1);
        assert!(schema.node_kind.is_none());
        assert!(schema.docs.is_none());
        assert!(schema.provides_capability.is_none());
    }

    #[test]
    fn test_non_empty() {
        assert_eq!(non_empty(String::new()), None);
        assert_eq!(non_empty("hello".into()), Some("hello".into()));
    }

    #[tokio::test]
    async fn test_new_creates_executor() {
        let exec = GrpcPluginExecutor::new("test", "http://localhost:50051").unwrap();
        assert_eq!(exec.name(), "test");
    }

    #[test]
    fn test_invalid_address_fails() {
        // connect_lazy needs a runtime, but invalid URI should fail at parse time.
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(async { GrpcPluginExecutor::new("bad", "not a valid uri \x00") });
        assert!(result.is_err());
    }
}
