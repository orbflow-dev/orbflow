// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! End-to-end test: TypeScript SDK plugin server ↔ Orbflow tonic gRPC client.
//!
//! Spawns the TypeScript example weather plugin and verifies that
//! `GrpcPluginExecutor` (tonic) can call Execute, GetSchemas, HealthCheck.
//!
//! Run with: `cargo test -p orbflow-plugin --features test-server --test sdk_e2e_ts`

#![cfg(feature = "test-server")]

use std::net::TcpListener;
use std::process::{Child, Command};
use std::time::Duration;

use orbflow_core::execution::InstanceId;
use orbflow_core::ports::{NodeExecutor, NodeInput};
use orbflow_plugin::grpc_client::GrpcPluginExecutor;

fn _ephemeral_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("failed to bind ephemeral port")
        .local_addr()
        .unwrap()
        .port()
}

struct NodeProcess(Child);

impl Drop for NodeProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

async fn start_ts_plugin() -> (NodeProcess, u16) {
    let project_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let sdk_dir = project_root.join("sdks/typescript");
    let actual_port = 50052_u16; // Weather example default

    // Use node --import tsx to run TypeScript directly.
    // On Windows, Command needs the .cmd extension for npm scripts.
    let npx = if cfg!(target_os = "windows") {
        "npx.cmd"
    } else {
        "npx"
    };

    let child = Command::new(npx)
        .args(["tsx", "examples/weather/index.ts"])
        .current_dir(&sdk_dir)
        .spawn()
        .unwrap_or_else(|e| {
            panic!(
                "failed to start TS plugin with `{npx} tsx`: {e}. \
             Ensure Node.js 18+ and tsx are installed."
            )
        });

    let addr = format!("http://127.0.0.1:{actual_port}");
    let executor = GrpcPluginExecutor::new("ts-probe", &addr).unwrap();

    for attempt in 0..30 {
        tokio::time::sleep(Duration::from_millis(200)).await;
        match executor.health_check().await {
            Ok((true, _)) => {
                eprintln!(
                    "TS plugin ready on port {actual_port} after {}ms",
                    (attempt + 1) * 200
                );
                return (NodeProcess(child), actual_port);
            }
            _ => continue,
        }
    }

    panic!("TS plugin did not become ready within 6 seconds");
}

#[tokio::test]
async fn ts_sdk_execute_weather() {
    let (_proc, port) = start_ts_plugin().await;
    let executor =
        GrpcPluginExecutor::new("weather-e2e", format!("http://127.0.0.1:{port}")).unwrap();

    let mut input = std::collections::HashMap::new();
    input.insert("city".into(), serde_json::json!("London"));

    let node_input = NodeInput {
        instance_id: InstanceId("e2e-ts".into()),
        node_id: "weather-1".into(),
        plugin_ref: "plugin:weather-forecast".into(),
        config: None,
        input: Some(input),
        parameters: None,
        capabilities: None,
        attempt: 0,
    };

    let output = executor.execute(&node_input).await.unwrap();
    assert!(output.error.is_none(), "error: {:?}", output.error);

    let data = output.data.expect("expected data");
    eprintln!("Weather result: {:?}", data);

    assert_eq!(data["city"], "London");
    assert!(data["temperature"].as_f64().is_some());
    assert!(data["condition"].as_str().is_some());
}

#[tokio::test]
async fn ts_sdk_get_schemas() {
    let (_proc, port) = start_ts_plugin().await;
    let executor =
        GrpcPluginExecutor::new("schema-ts", format!("http://127.0.0.1:{port}")).unwrap();

    let schemas = executor.fetch_schemas().await.unwrap();
    assert_eq!(schemas.len(), 1);

    let schema = &schemas[0];
    assert_eq!(schema.plugin_ref, "plugin:weather-forecast");
    assert_eq!(schema.name, "Weather Forecast");
    assert_eq!(schema.category, "utility");
    assert!(schema.inputs.len() >= 1);
    assert!(schema.outputs.len() >= 2);
}

#[tokio::test]
async fn ts_sdk_health_check() {
    let (_proc, port) = start_ts_plugin().await;
    let executor =
        GrpcPluginExecutor::new("health-ts", format!("http://127.0.0.1:{port}")).unwrap();

    let (healthy, version) = executor.health_check().await.unwrap();
    assert!(healthy);
    assert_eq!(version, "1.0.0");
}
