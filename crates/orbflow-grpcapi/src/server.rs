// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! gRPC server for the Orbflow workflow engine.
//!
//! Uses tonic for transport with a manually-defined service (no .proto file).
//! This mirrors the Go implementation which uses a hand-written ServiceDesc
//! and JSON codec instead of protobuf.

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tokio::sync::watch;

use orbflow_core::error::OrbflowError;
use orbflow_core::execution::InstanceId;
use orbflow_core::ports::Engine;

use crate::types;

// ---------------------------------------------------------------------------
// Request / Response types (JSON wire format, matches Go grpcapi/types.go)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateWorkflowRequest {
    pub definition: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetWorkflowRequest {
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WorkflowResponse {
    pub data: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListRequest {
    #[serde(default)]
    pub offset: i32,
    #[serde(default)]
    pub limit: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListWorkflowsResponse {
    pub items: Vec<Vec<u8>>,
    pub total: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StartWorkflowRequest {
    pub workflow_id: String,
    #[serde(default)]
    pub input: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetInstanceRequest {
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CancelInstanceRequest {
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InstanceResponse {
    pub data: Vec<u8>,
}

/// Envelope for JSON-RPC-like gRPC communication.
#[derive(Debug, Serialize, Deserialize)]
struct RpcRequest {
    method: String,
    #[serde(default)]
    body: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct RpcResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<RpcError>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RpcError {
    code: String,
    message: String,
}

// ---------------------------------------------------------------------------
// GrpcServer
// ---------------------------------------------------------------------------

/// The Orbflow gRPC server wrapping an Engine.
///
/// Scope: exposes a subset of the HTTP API — workflow lifecycle only
/// (create, get, list, start, get instance, cancel instance). This keeps the
/// gRPC contract minimal and stable for machine-to-machine integrations.
pub struct GrpcServer {
    engine: Arc<dyn Engine>,
    auth_token: Option<String>,
    shutdown_tx: watch::Sender<bool>,
    shutdown_rx: watch::Receiver<bool>,
}

impl GrpcServer {
    /// Creates a new gRPC server wrapping the engine.
    ///
    /// When `auth_token` is `Some(t)`, every JSON-RPC request must include an
    /// `"auth_token"` field in the envelope that matches `t` exactly. Requests
    /// that fail this check receive an `UNAUTHENTICATED` error response.
    /// When `auth_token` is `None`, authentication is disabled.
    pub fn new(engine: Arc<dyn Engine>, auth_token: Option<String>) -> Self {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        Self {
            engine,
            auth_token,
            shutdown_tx,
            shutdown_rx,
        }
    }

    /// Starts the JSON-RPC server on the given address (e.g., "0.0.0.0:9090").
    ///
    /// This is a TCP server using newline-delimited JSON, providing a gRPC-like
    /// experience without requiring protobuf.
    ///
    /// # Security note
    ///
    /// When `auth_token` is configured, every request envelope must carry a
    /// matching `"auth_token"` field. For deployments without a token, ensure
    /// network-level isolation (firewall rules, Kubernetes NetworkPolicy, a
    /// reverse proxy) provides the trust boundary.
    pub async fn serve(&self, addr: &str) -> Result<(), OrbflowError> {
        let listener = TcpListener::bind(addr)
            .await
            .map_err(|e| OrbflowError::Internal(format!("grpc: bind {addr}: {e}")))?;

        tracing::info!("gRPC server listening on {addr}");

        let mut shutdown_rx = self.shutdown_rx.clone();

        loop {
            tokio::select! {
                result = listener.accept() => {
                    let (stream, peer) = result.map_err(|e| {
                        OrbflowError::Internal(format!("grpc: accept: {e}"))
                    })?;

                    tracing::debug!("gRPC connection from {peer}");
                    let engine = self.engine.clone();
                    let auth_token = self.auth_token.clone();

                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(stream, engine, auth_token).await {
                            tracing::warn!("gRPC connection error from {peer}: {e}");
                        }
                    });
                }
                _ = shutdown_rx.changed() => {
                    tracing::info!("gRPC server shutting down");
                    break;
                }
            }
        }

        Ok(())
    }

    /// Signals the server to stop.
    pub fn stop(&self) {
        let _ = self.shutdown_tx.send(true);
    }
}

/// Handles a single client connection using newline-delimited JSON.
async fn handle_connection(
    stream: tokio::net::TcpStream,
    engine: Arc<dyn Engine>,
    auth_token: Option<String>,
) -> Result<(), OrbflowError> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    loop {
        line.clear();
        let n = reader
            .read_line(&mut line)
            .await
            .map_err(|e| OrbflowError::Internal(format!("grpc: read: {e}")))?;

        if n == 0 {
            break; // Connection closed.
        }

        // Parse into a raw JSON object first so we can extract auth_token
        // before deserializing the typed RpcRequest.
        let raw: serde_json::Value = match serde_json::from_str(line.trim()) {
            Ok(v) => v,
            Err(e) => {
                let resp = RpcResponse {
                    data: None,
                    error: Some(RpcError {
                        code: "INVALID_ARGUMENT".into(),
                        message: format!("invalid request: {e}"),
                    }),
                };
                let out = match serde_json::to_vec(&resp) {
                    Ok(mut b) => {
                        b.push(b'\n');
                        b
                    }
                    Err(_) => {
                        b"{\"data\":null,\"error\":\"internal: response serialization failed\"}\n"
                            .to_vec()
                    }
                };
                if let Err(e) = writer.write_all(&out).await {
                    tracing::warn!("gRPC: failed to write error response: {e}");
                    break;
                }
                continue;
            }
        };

        // Check auth_token in the request envelope when the server has one configured.
        if let Some(ref expected) = auth_token {
            let provided = raw.get("auth_token").and_then(|v| v.as_str()).unwrap_or("");
            let is_valid =
                orbflow_core::crypto::constant_time_eq(provided.as_bytes(), expected.as_bytes());
            if !is_valid {
                let resp = error_response("UNAUTHENTICATED", "unauthorized");
                let out = match serde_json::to_vec(&resp) {
                    Ok(mut b) => {
                        b.push(b'\n');
                        b
                    }
                    Err(_) => {
                        b"{\"data\":null,\"error\":\"internal: response serialization failed\"}\n"
                            .to_vec()
                    }
                };
                if let Err(e) = writer.write_all(&out).await {
                    tracing::warn!("gRPC: failed to write error response: {e}");
                    break;
                }
                continue;
            }
        }

        let request: RpcRequest = match serde_json::from_value(raw) {
            Ok(r) => r,
            Err(e) => {
                let resp = RpcResponse {
                    data: None,
                    error: Some(RpcError {
                        code: "INVALID_ARGUMENT".into(),
                        message: format!("invalid request: {e}"),
                    }),
                };
                let out = match serde_json::to_vec(&resp) {
                    Ok(mut b) => {
                        b.push(b'\n');
                        b
                    }
                    Err(_) => {
                        b"{\"data\":null,\"error\":\"internal: response serialization failed\"}\n"
                            .to_vec()
                    }
                };
                if let Err(e) = writer.write_all(&out).await {
                    tracing::warn!("gRPC: failed to write error response: {e}");
                    break;
                }
                continue;
            }
        };

        let response = dispatch(&engine, &request).await;

        let out = match serde_json::to_vec(&response) {
            Ok(mut b) => {
                b.push(b'\n');
                b
            }
            Err(_) => {
                b"{\"data\":null,\"error\":\"internal: response serialization failed\"}\n".to_vec()
            }
        };
        writer
            .write_all(&out)
            .await
            .map_err(|e| OrbflowError::Internal(format!("grpc: write: {e}")))?;
    }

    Ok(())
}

/// Dispatches an RPC request to the appropriate engine method.
async fn dispatch(engine: &Arc<dyn Engine>, req: &RpcRequest) -> RpcResponse {
    match req.method.as_str() {
        "CreateWorkflow" => handle_create_workflow(engine, &req.body).await,
        "GetWorkflow" => handle_get_workflow(engine, &req.body).await,
        "ListWorkflows" => handle_list_workflows(engine, &req.body).await,
        "StartWorkflow" => handle_start_workflow(engine, &req.body).await,
        "GetInstance" => handle_get_instance(engine, &req.body).await,
        "CancelInstance" => handle_cancel_instance(engine, &req.body).await,
        _ => RpcResponse {
            data: None,
            error: Some(RpcError {
                code: "UNIMPLEMENTED".into(),
                message: format!("unknown method: {}", req.method),
            }),
        },
    }
}

async fn handle_create_workflow(engine: &Arc<dyn Engine>, body: &serde_json::Value) -> RpcResponse {
    let definition = match body.get("definition").and_then(|v| {
        // Accept both raw bytes (array) and base64 string.
        if let Some(s) = v.as_str() {
            use base64::Engine as _;
            base64::engine::general_purpose::STANDARD.decode(s).ok()
        } else {
            v.as_array().map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_u64().map(|n| n as u8))
                    .collect()
            })
        }
    }) {
        Some(d) => d,
        None => {
            // Try treating the whole body as the workflow definition.
            match serde_json::to_vec(body) {
                Ok(d) => d,
                Err(_) => {
                    return error_response("INVALID_ARGUMENT", "missing or invalid definition");
                }
            }
        }
    };

    let wf = match types::workflow_from_bytes(&definition) {
        Ok(wf) => wf,
        Err(e) => return orbflow_error_to_response(e),
    };

    match engine.create_workflow(&wf).await {
        Ok(()) => match types::workflow_to_bytes(&wf) {
            Ok(data) => ok_response(serde_json::json!({ "data": data })),
            Err(e) => orbflow_error_to_response(e),
        },
        Err(e) => orbflow_error_to_response(e),
    }
}

async fn handle_get_workflow(engine: &Arc<dyn Engine>, body: &serde_json::Value) -> RpcResponse {
    let id = body.get("id").and_then(|v| v.as_str()).unwrap_or_default();

    let wf_id = match types::parse_workflow_id(id) {
        Ok(id) => id,
        Err(e) => return orbflow_error_to_response(e),
    };

    match engine.get_workflow(&wf_id).await {
        Ok(wf) => match types::workflow_to_bytes(&wf) {
            Ok(data) => ok_response(serde_json::json!({ "data": data })),
            Err(e) => orbflow_error_to_response(e),
        },
        Err(e) => orbflow_error_to_response(e),
    }
}

async fn handle_list_workflows(engine: &Arc<dyn Engine>, body: &serde_json::Value) -> RpcResponse {
    let offset = body
        .get("offset")
        .and_then(|v| v.as_i64())
        .and_then(|n| i32::try_from(n).ok())
        .unwrap_or(0)
        .max(0);
    let limit = body
        .get("limit")
        .and_then(|v| v.as_i64())
        .and_then(|n| i32::try_from(n).ok())
        .unwrap_or(orbflow_core::ports::DEFAULT_PAGE_SIZE as i32)
        .clamp(1, 100);

    let opts = types::parse_list_options(offset, limit);

    match engine.list_workflows(opts).await {
        Ok((workflows, total)) => {
            let items: Result<Vec<Vec<u8>>, _> =
                workflows.iter().map(types::workflow_to_bytes).collect();
            match items {
                Ok(items) => ok_response(serde_json::json!({
                    "items": items,
                    "total": total,
                })),
                Err(e) => orbflow_error_to_response(e),
            }
        }
        Err(e) => orbflow_error_to_response(e),
    }
}

async fn handle_start_workflow(engine: &Arc<dyn Engine>, body: &serde_json::Value) -> RpcResponse {
    let wf_id_str = body
        .get("workflow_id")
        .and_then(|v| v.as_str())
        .unwrap_or_default();

    let wf_id = match types::parse_workflow_id(wf_id_str) {
        Ok(id) => id,
        Err(e) => return orbflow_error_to_response(e),
    };

    let input: HashMap<String, serde_json::Value> = body
        .get("input")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    match engine.start_workflow(&wf_id, input).await {
        Ok(inst) => match types::instance_to_bytes(&inst) {
            Ok(data) => ok_response(serde_json::json!({ "data": data })),
            Err(e) => orbflow_error_to_response(e),
        },
        Err(e) => orbflow_error_to_response(e),
    }
}

async fn handle_get_instance(engine: &Arc<dyn Engine>, body: &serde_json::Value) -> RpcResponse {
    let id = body.get("id").and_then(|v| v.as_str()).unwrap_or_default();

    if id.is_empty() {
        return error_response("INVALID_ARGUMENT", "instance id is required");
    }

    match engine.get_instance(&InstanceId::new(id)).await {
        Ok(inst) => match types::instance_to_bytes(&inst) {
            Ok(data) => ok_response(serde_json::json!({ "data": data })),
            Err(e) => orbflow_error_to_response(e),
        },
        Err(e) => orbflow_error_to_response(e),
    }
}

async fn handle_cancel_instance(engine: &Arc<dyn Engine>, body: &serde_json::Value) -> RpcResponse {
    let id = body.get("id").and_then(|v| v.as_str()).unwrap_or_default();

    if id.is_empty() {
        return error_response("INVALID_ARGUMENT", "instance id is required");
    }

    let inst_id = InstanceId::new(id);

    if let Err(e) = engine.cancel_instance(&inst_id).await {
        return orbflow_error_to_response(e);
    }

    // Return the updated instance.
    match engine.get_instance(&inst_id).await {
        Ok(inst) => match types::instance_to_bytes(&inst) {
            Ok(data) => ok_response(serde_json::json!({ "data": data })),
            Err(e) => orbflow_error_to_response(e),
        },
        Err(e) => orbflow_error_to_response(e),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn ok_response(data: serde_json::Value) -> RpcResponse {
    RpcResponse {
        data: Some(data),
        error: None,
    }
}

fn error_response(code: &str, message: &str) -> RpcResponse {
    RpcResponse {
        data: None,
        error: Some(RpcError {
            code: code.to_owned(),
            message: message.to_owned(),
        }),
    }
}

/// Maps a [`OrbflowError`] to an RPC error response.
fn orbflow_error_to_response(e: OrbflowError) -> RpcResponse {
    let code = match &e {
        OrbflowError::NotFound => "NOT_FOUND",
        OrbflowError::AlreadyExists => "ALREADY_EXISTS",
        OrbflowError::Conflict => "ABORTED",
        OrbflowError::InvalidNodeConfig(_) => "INVALID_ARGUMENT",
        OrbflowError::CycleDetected => "INVALID_ARGUMENT",
        OrbflowError::DuplicateNode => "INVALID_ARGUMENT",
        OrbflowError::DuplicateEdge => "INVALID_ARGUMENT",
        OrbflowError::Cancelled => "CANCELLED",
        OrbflowError::Timeout => "DEADLINE_EXCEEDED",
        _ => "INTERNAL",
    };

    error_response(code, &e.to_string())
}
