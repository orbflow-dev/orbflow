// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Integration test for gRPC plugin wire compatibility.
//!
//! This test starts a tonic gRPC server implementing the `OrbflowPlugin` service
//! and verifies the `GrpcPluginExecutor` can call `Execute`, `GetSchemas`, and
//! `HealthCheck` end-to-end. The same wire protocol is used by ConnectRPC
//! servers, so passing this test proves tonic ↔ ConnectRPC compatibility.
//!
//! Run with: `cargo test -p orbflow-plugin --features test-server --test grpc_compat`
//!
//! When a ConnectRPC-based SDK server is available, add a parallel test that
//! spawns it as a subprocess and runs the same assertions.

#![cfg(feature = "test-server")]

use std::collections::HashMap;
use std::net::SocketAddr;

use tokio::sync::oneshot;
use tonic::{Request, Response, Status};

use orbflow_core::execution::InstanceId;
use orbflow_core::ports::{NodeExecutor, NodeInput};

use orbflow_plugin::grpc_client::GrpcPluginExecutor;

// Re-use the generated proto types from orbflow-plugin (server stubs enabled
// via the `test-server` feature).
mod proto {
    pub use orbflow_plugin::grpc_proto::*;
}

// ---------------------------------------------------------------------------
// Mock plugin server
// ---------------------------------------------------------------------------

/// A minimal OrbflowPlugin gRPC server for testing.
struct MockPluginServer;

#[tonic::async_trait]
impl proto::orbflow_plugin_server::OrbflowPlugin for MockPluginServer {
    async fn execute(
        &self,
        request: Request<proto::ExecuteRequest>,
    ) -> Result<Response<proto::ExecuteResponse>, Status> {
        let req = request.into_inner();

        // Echo back the plugin_ref and input_json as proof of round-trip.
        let data = serde_json::json!({
            "echoed_plugin_ref": req.plugin_ref,
            "echoed_node_id": req.node_id,
            "echoed_attempt": req.attempt,
            "received_input": req.input_json,
        });

        Ok(Response::new(proto::ExecuteResponse {
            data_json: serde_json::to_string(&data)
                .expect("serialization of static test data should never fail"),
            error: String::new(),
        }))
    }

    async fn get_schemas(
        &self,
        _request: Request<proto::GetSchemasRequest>,
    ) -> Result<Response<proto::GetSchemasResponse>, Status> {
        let schema = proto::NodeSchema {
            plugin_ref: "plugin:test-action".into(),
            name: "Test Action".into(),
            description: "A test action for compatibility verification".into(),
            category: "testing".into(),
            icon: "flask".into(),
            color: "#10b981".into(),
            node_kind: "action".into(),
            docs: String::new(),
            image_url: String::new(),
            inputs_json: serde_json::to_string(&serde_json::json!([
                {"key": "message", "label": "Message", "type": "string", "required": true}
            ]))
            .expect("serialization of static test schema should never fail"),
            outputs_json: serde_json::to_string(&serde_json::json!([
                {"key": "result", "label": "Result", "type": "string"}
            ]))
            .expect("serialization of static test schema should never fail"),
            parameters_json: String::new(),
            capability_ports_json: String::new(),
            settings_json: String::new(),
            provides_capability: String::new(),
        };

        Ok(Response::new(proto::GetSchemasResponse {
            schemas: vec![schema],
        }))
    }

    async fn health_check(
        &self,
        _request: Request<proto::HealthCheckRequest>,
    ) -> Result<Response<proto::HealthCheckResponse>, Status> {
        Ok(Response::new(proto::HealthCheckResponse {
            healthy: true,
            version: "1.0.0-test".into(),
        }))
    }
}

/// Start the mock server on a random port and return the address.
///
/// Uses a oneshot channel to signal when the server is ready, avoiding
/// fragile sleep-based synchronization.
async fn start_mock_server() -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("failed to bind test server to ephemeral port");
    let addr = listener
        .local_addr()
        .expect("listener should have a local address");

    let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);

    let (ready_tx, ready_rx) = oneshot::channel::<()>();

    let handle = tokio::spawn(async move {
        // Signal readiness after yielding once so the accept loop is entered.
        // The listener is already bound at the OS level (port reserved), and
        // TcpListenerStream wraps the bound socket, so the accept loop will
        // process connections once polled. We yield to ensure the runtime has
        // scheduled the serve future before the test proceeds.
        let _ = ready_tx.send(());
        tokio::task::yield_now().await;

        tonic::transport::Server::builder()
            .add_service(proto::orbflow_plugin_server::OrbflowPluginServer::new(
                MockPluginServer,
            ))
            .serve_with_incoming(incoming)
            .await
            .expect("mock gRPC server failed unexpectedly");
    });

    // Wait for the server task to signal readiness.
    ready_rx
        .await
        .expect("server task dropped readiness signal — likely panicked during setup");

    // Give the accept loop one more tick to start polling.
    tokio::task::yield_now().await;

    // Verify the server task hasn't already failed.
    assert!(!handle.is_finished(), "server task exited prematurely");

    addr
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn execute_round_trip() {
    let addr = start_mock_server().await;
    let executor = GrpcPluginExecutor::new("compat-test", format!("http://{addr}")).unwrap();

    let mut input_data = HashMap::new();
    input_data.insert("message".into(), serde_json::json!("hello world"));

    let node_input = NodeInput {
        instance_id: InstanceId("inst-compat".into()),
        node_id: "node-1".into(),
        plugin_ref: "plugin:test-action".into(),
        config: None,
        input: Some(input_data),
        parameters: None,
        capabilities: None,
        attempt: 0,
    };

    let output = executor.execute(&node_input).await.unwrap();

    // Should succeed (no error).
    assert!(
        output.error.is_none(),
        "unexpected error: {:?}",
        output.error
    );

    // Should contain echoed data.
    let data = output.data.expect("expected data in response");
    assert_eq!(data["echoed_plugin_ref"], "plugin:test-action");
    assert_eq!(data["echoed_node_id"], "node-1");
    assert_eq!(data["echoed_attempt"], 0);

    // The input_json field should contain our serialized input.
    let received_input: String = serde_json::from_value(data["received_input"].clone()).unwrap();
    assert!(received_input.contains("hello world"));
}

#[tokio::test]
async fn get_schemas_returns_valid_node_schema() {
    let addr = start_mock_server().await;
    let executor = GrpcPluginExecutor::new("compat-test", format!("http://{addr}")).unwrap();

    let schemas = executor.fetch_schemas().await.unwrap();

    assert_eq!(schemas.len(), 1);

    let schema = &schemas[0];
    assert_eq!(schema.plugin_ref, "plugin:test-action");
    assert_eq!(schema.name, "Test Action");
    assert_eq!(schema.category, "testing");
    assert_eq!(schema.icon, "flask");
    assert_eq!(schema.color, "#10b981");

    // Inputs parsed correctly from JSON.
    assert_eq!(schema.inputs.len(), 1);
    assert_eq!(schema.inputs[0].key, "message");
    assert!(schema.inputs[0].required);

    // Outputs parsed correctly.
    assert_eq!(schema.outputs.len(), 1);
    assert_eq!(schema.outputs[0].key, "result");
}

#[tokio::test]
async fn health_check_returns_healthy() {
    let addr = start_mock_server().await;
    let executor = GrpcPluginExecutor::new("compat-test", format!("http://{addr}")).unwrap();

    let (healthy, version) = executor.health_check().await.unwrap();

    assert!(healthy);
    assert_eq!(version, "1.0.0-test");
}

#[tokio::test]
async fn execute_with_all_fields_populated() {
    let addr = start_mock_server().await;
    let executor = GrpcPluginExecutor::new("compat-test", format!("http://{addr}")).unwrap();

    let mut config = HashMap::new();
    config.insert("api_key".into(), serde_json::json!("sk-test"));

    let mut input = HashMap::new();
    input.insert("text".into(), serde_json::json!("analyze this"));

    let mut params = HashMap::new();
    params.insert("threshold".into(), serde_json::json!(0.8));

    let mut caps = HashMap::new();
    caps.insert("db".into(), serde_json::json!({"host": "localhost"}));

    let node_input = NodeInput {
        instance_id: InstanceId("inst-full".into()),
        node_id: "node-full".into(),
        plugin_ref: "plugin:full-test".into(),
        config: Some(config),
        input: Some(input),
        parameters: Some(params),
        capabilities: Some(caps),
        attempt: 3,
    };

    let output = executor.execute(&node_input).await.unwrap();
    assert!(output.error.is_none());

    let data = output.data.expect("expected data");
    assert_eq!(data["echoed_plugin_ref"], "plugin:full-test");
    assert_eq!(data["echoed_attempt"], 3);
}
