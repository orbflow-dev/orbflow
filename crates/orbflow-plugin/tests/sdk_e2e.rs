// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! End-to-end test: Python SDK plugin server ↔ Orbflow tonic gRPC client.
//!
//! This test spawns a real plugin (orbflow-uuid-gen) from the plugins/
//! directory and verifies that `GrpcPluginExecutor` (the Orbflow gRPC client)
//! can call Execute, GetSchemas, and HealthCheck against it.
//!
//! Run with: `cargo test -p orbflow-plugin --features test-server --test sdk_e2e`

#![cfg(feature = "test-server")]

use std::net::TcpListener;
use std::process::{Child, Command};
use std::time::Duration;

use orbflow_core::execution::InstanceId;
use orbflow_core::ports::{NodeExecutor, NodeInput};
use orbflow_plugin::grpc_client::GrpcPluginExecutor;

/// Returns the correct Python command for the current platform.
fn python_cmd() -> &'static str {
    if cfg!(target_os = "windows") {
        "py"
    } else {
        "python3"
    }
}

/// Find an available ephemeral port.
fn ephemeral_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("failed to bind ephemeral port")
        .local_addr()
        .expect("listener should have a local address")
        .port()
}

struct PythonPluginProcess(Child);

impl Drop for PythonPluginProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Start a Python SDK plugin from the plugins/ directory on an ephemeral port.
async fn start_plugin(plugin_dir: &str, plugin_module: &str) -> (PythonPluginProcess, u16) {
    let project_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let sdk_src = project_root.join("sdks/python/src");
    let plugin_path = project_root.join("plugins").join(plugin_dir);
    let port = ephemeral_port();

    let child = Command::new(python_cmd())
        .arg("-c")
        .arg(format!(
            r#"
import sys
sys.path.insert(0, r'{sdk_src}')
sys.path.insert(0, r'{plugin_parent}')

from {module} import *
from orbflow_sdk.decorators import get_plugin_meta

import importlib
mod = importlib.import_module('{module}')
plugin_cls = None
for name in dir(mod):
    obj = getattr(mod, name)
    if isinstance(obj, type) and get_plugin_meta(obj):
        plugin_cls = obj
        break

from orbflow_sdk import run
run(plugin_cls, host='127.0.0.1', port={port})
"#,
            sdk_src = sdk_src.display(),
            plugin_parent = plugin_path.parent().unwrap().display(),
            module = plugin_module,
            port = port,
        ))
        .current_dir(&plugin_path)
        .spawn()
        .unwrap_or_else(|e| {
            panic!(
                "failed to start Python plugin with `{}`: {e}. \
             Ensure Python 3.10+ is installed.",
                python_cmd()
            )
        });

    // Wait for the server to be ready by polling health check.
    let addr = format!("http://127.0.0.1:{port}");
    let executor = GrpcPluginExecutor::new("e2e-probe", &addr).unwrap();

    for attempt in 0..30 {
        tokio::time::sleep(Duration::from_millis(200)).await;
        match executor.health_check().await {
            Ok((true, _)) => {
                eprintln!(
                    "Plugin ready on port {port} after {}ms",
                    (attempt + 1) * 200
                );
                return (PythonPluginProcess(child), port);
            }
            _ => continue,
        }
    }

    panic!("Plugin did not become ready within 6 seconds (port {port})");
}

// ---------------------------------------------------------------------------
// UUID Generator plugin tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn uuid_gen_execute() {
    let (_proc, port) = start_plugin("orbflow-uuid-gen", "main").await;
    let executor = GrpcPluginExecutor::new("uuid-e2e", format!("http://127.0.0.1:{port}")).unwrap();

    let node_input = NodeInput {
        instance_id: InstanceId("e2e-inst".into()),
        node_id: "uuid-1".into(),
        plugin_ref: "plugin:uuid-gen".into(),
        config: None,
        input: None,
        parameters: None,
        capabilities: None,
        attempt: 0,
    };

    let output = executor.execute(&node_input).await.unwrap();
    assert!(
        output.error.is_none(),
        "unexpected error: {:?}",
        output.error
    );

    let data = output.data.expect("expected data in response");
    eprintln!("UUID result: {:?}", data);

    let uuid = data["uuid"].as_str().expect("uuid should be a string");
    assert_eq!(uuid.len(), 36, "UUID should be 36 chars (with hyphens)");
    assert!(uuid.contains('-'), "UUID should contain hyphens");
    assert_eq!(data["count"], 1);
}

#[tokio::test]
async fn uuid_gen_get_schemas() {
    let (_proc, port) = start_plugin("orbflow-uuid-gen", "main").await;
    let executor =
        GrpcPluginExecutor::new("schema-e2e", format!("http://127.0.0.1:{port}")).unwrap();

    let schemas = executor.fetch_schemas().await.unwrap();

    assert_eq!(schemas.len(), 1);
    let schema = &schemas[0];
    assert_eq!(schema.plugin_ref, "plugin:uuid-gen");
    assert_eq!(schema.name, "UUID Generator");
    assert_eq!(schema.category, "utility");
    assert_eq!(schema.icon, "hash");

    assert_eq!(schema.inputs.len(), 0);
    assert_eq!(schema.outputs.len(), 3);
    assert_eq!(schema.parameters.len(), 2);

    let output_keys: Vec<&str> = schema.outputs.iter().map(|o| o.key.as_str()).collect();
    assert!(output_keys.contains(&"uuid"));
    assert!(output_keys.contains(&"uuids"));
    assert!(output_keys.contains(&"count"));
}

#[tokio::test]
async fn uuid_gen_health_check() {
    let (_proc, port) = start_plugin("orbflow-uuid-gen", "main").await;
    let executor =
        GrpcPluginExecutor::new("health-e2e", format!("http://127.0.0.1:{port}")).unwrap();

    let (healthy, version) = executor.health_check().await.unwrap();
    assert!(healthy);
    assert_eq!(version, "0.2.0");
}
