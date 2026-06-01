// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use orbflow_core::error::OrbflowError;
use orbflow_core::execution::{Instance, InstanceId, TestNodeResult};
use orbflow_core::ports::{Engine, ListOptions, NodeExecutor, NodeSchema};
use orbflow_core::workflow::{Workflow, WorkflowId};
use orbflow_grpcapi::GrpcServer;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

#[derive(Default)]
struct RecordingEngine {
    create_calls: AtomicUsize,
    start_calls: AtomicUsize,
    owner_start_calls: AtomicUsize,
    last_owner_id: Mutex<Option<String>>,
}

#[async_trait]
impl Engine for RecordingEngine {
    async fn create_workflow(&self, _wf: &Workflow) -> Result<(), OrbflowError> {
        self.create_calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn update_workflow(&self, _wf: &Workflow) -> Result<(), OrbflowError> {
        Ok(())
    }

    async fn delete_workflow(&self, _id: &WorkflowId) -> Result<(), OrbflowError> {
        Ok(())
    }

    async fn get_workflow(&self, id: &WorkflowId) -> Result<Workflow, OrbflowError> {
        Ok(test_workflow(&id.to_string()))
    }

    async fn list_workflows(
        &self,
        _opts: ListOptions,
    ) -> Result<(Vec<Workflow>, i64), OrbflowError> {
        Ok((Vec::new(), 0))
    }

    async fn start_workflow(
        &self,
        id: &WorkflowId,
        _input: HashMap<String, Value>,
    ) -> Result<Instance, OrbflowError> {
        self.start_calls.fetch_add(1, Ordering::SeqCst);
        Ok(test_instance(id))
    }

    async fn start_workflow_for_owner(
        &self,
        id: &WorkflowId,
        _input: HashMap<String, Value>,
        owner_id: &str,
    ) -> Result<Instance, OrbflowError> {
        self.owner_start_calls.fetch_add(1, Ordering::SeqCst);
        *self.last_owner_id.lock().unwrap() = Some(owner_id.to_owned());
        Ok(test_instance(id))
    }

    async fn get_instance(&self, id: &InstanceId) -> Result<Instance, OrbflowError> {
        let mut instance = test_instance(&WorkflowId::new("wf-1"));
        instance.id = id.clone();
        Ok(instance)
    }

    async fn list_instances(
        &self,
        _opts: ListOptions,
    ) -> Result<(Vec<Instance>, i64), OrbflowError> {
        Ok((Vec::new(), 0))
    }

    async fn cancel_instance(&self, _id: &InstanceId) -> Result<(), OrbflowError> {
        Ok(())
    }

    async fn start(&self) -> Result<(), OrbflowError> {
        Ok(())
    }

    async fn stop(&self) -> Result<(), OrbflowError> {
        Ok(())
    }

    async fn approve_node(
        &self,
        _instance_id: &InstanceId,
        _node_id: &str,
        _approved_by: Option<String>,
    ) -> Result<(), OrbflowError> {
        Ok(())
    }

    async fn reject_node(
        &self,
        _instance_id: &InstanceId,
        _node_id: &str,
        _reason: Option<String>,
        _rejected_by: Option<String>,
    ) -> Result<(), OrbflowError> {
        Ok(())
    }

    async fn test_node(
        &self,
        _workflow_id: &WorkflowId,
        _node_id: &str,
        _cached_outputs: HashMap<String, HashMap<String, Value>>,
        _owner_id: Option<&str>,
    ) -> Result<TestNodeResult, OrbflowError> {
        Ok(TestNodeResult {
            node_outputs: HashMap::new(),
            target_node: "node-1".into(),
            warnings: Vec::new(),
        })
    }

    fn register_node(
        &self,
        _name: &str,
        _executor: Arc<dyn NodeExecutor>,
    ) -> Result<(), OrbflowError> {
        Ok(())
    }

    fn node_schemas(&self) -> Vec<NodeSchema> {
        Vec::new()
    }
}

fn test_workflow(id: &str) -> Workflow {
    serde_json::from_value(json!({
        "id": id,
        "name": format!("Workflow {id}"),
        "description": null,
        "version": 1,
        "status": "active",
        "nodes": [{
            "id": "node-1",
            "name": "Log",
            "kind": "action",
            "type": "builtin",
            "plugin_ref": "builtin:log",
            "position": { "x": 0.0, "y": 0.0 },
            "input_mapping": null,
            "config": null,
            "parameters": [],
            "retry": null,
            "compensate": null,
            "capability_ports": [],
            "metadata": null,
            "trigger_config": null,
            "requires_approval": false
        }],
        "edges": [],
        "capability_edges": [],
        "triggers": [],
        "annotations": [],
        "created_at": "2026-01-01T00:00:00Z",
        "updated_at": "2026-01-01T00:00:00Z"
    }))
    .unwrap()
}

fn test_instance(workflow_id: &WorkflowId) -> Instance {
    serde_json::from_value(json!({
        "id": "inst-1",
        "workflow_id": workflow_id.to_string(),
        "status": "running",
        "node_states": {},
        "context": {
            "variables": {},
            "node_outputs": {},
            "trigger_data": null,
            "user_id": null
        },
        "saga": null,
        "parent_id": null,
        "instance_metrics": null,
        "workflow_version": 1,
        "owner_id": "owner-1",
        "version": 1,
        "created_at": "2026-01-01T00:00:00Z",
        "updated_at": "2026-01-01T00:00:00Z"
    }))
    .unwrap()
}

async fn start_server(engine: Arc<RecordingEngine>) -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let reserved = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = reserved.local_addr().unwrap();
    drop(reserved);

    let server = GrpcServer::new(engine, None);
    let addr_string = addr.to_string();
    let handle = tokio::spawn(async move {
        let _ = server.serve(&addr_string).await;
    });

    for _ in 0..50 {
        if TcpStream::connect(addr).await.is_ok() {
            return (addr, handle);
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    handle.abort();
    panic!("gRPC test server did not start on {addr}");
}

async fn send_rpc(addr: SocketAddr, request: Value) -> Value {
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let mut body = serde_json::to_vec(&request).unwrap();
    body.push(b'\n');
    stream.write_all(&body).await.unwrap();

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).await.unwrap();
    serde_json::from_str(line.trim()).unwrap()
}

#[tokio::test]
async fn create_workflow_rejects_non_byte_definition_array_entries() {
    let engine = Arc::new(RecordingEngine::default());
    let (addr, handle) = start_server(Arc::clone(&engine)).await;

    let bytes = serde_json::to_vec(&test_workflow("wf-byte-validation")).unwrap();
    let mut definition: Vec<Value> = bytes.into_iter().map(|b| json!(b)).collect();
    definition.insert(1, json!("not-a-byte"));

    let response = send_rpc(
        addr,
        json!({
            "method": "CreateWorkflow",
            "body": { "definition": definition }
        }),
    )
    .await;

    handle.abort();

    assert_eq!(response["error"]["code"], "INVALID_ARGUMENT");
    assert_eq!(engine.create_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn start_workflow_rejects_non_object_input() {
    let engine = Arc::new(RecordingEngine::default());
    let (addr, handle) = start_server(Arc::clone(&engine)).await;

    let response = send_rpc(
        addr,
        json!({
            "method": "StartWorkflow",
            "body": {
                "workflow_id": "wf-input-validation",
                "input": "not an object"
            }
        }),
    )
    .await;

    handle.abort();

    assert_eq!(response["error"]["code"], "INVALID_ARGUMENT");
    assert_eq!(engine.start_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn start_workflow_uses_owner_context_when_provided() {
    let engine = Arc::new(RecordingEngine::default());
    let (addr, handle) = start_server(Arc::clone(&engine)).await;

    let response = send_rpc(
        addr,
        json!({
            "method": "StartWorkflow",
            "body": {
                "workflow_id": "wf-owner-start",
                "owner_id": "owner-123",
                "input": { "order_id": "o-1" }
            }
        }),
    )
    .await;

    handle.abort();

    assert!(response["error"].is_null());
    assert_eq!(engine.start_calls.load(Ordering::SeqCst), 0);
    assert_eq!(engine.owner_start_calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        engine.last_owner_id.lock().unwrap().as_deref(),
        Some("owner-123")
    );
}

#[tokio::test]
async fn start_workflow_rejects_invalid_owner_id() {
    let engine = Arc::new(RecordingEngine::default());
    let (addr, handle) = start_server(Arc::clone(&engine)).await;

    let response = send_rpc(
        addr,
        json!({
            "method": "StartWorkflow",
            "body": {
                "workflow_id": "wf-owner-start",
                "owner_id": "",
                "input": {}
            }
        }),
    )
    .await;

    handle.abort();

    assert_eq!(response["error"]["code"], "INVALID_ARGUMENT");
    assert_eq!(engine.start_calls.load(Ordering::SeqCst), 0);
    assert_eq!(engine.owner_start_calls.load(Ordering::SeqCst), 0);
}
