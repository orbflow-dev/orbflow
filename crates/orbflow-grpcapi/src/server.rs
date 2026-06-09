// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! JSON-RPC TCP server for the Orbflow workflow engine.
//!
//! This crate retains the historical `grpcapi` name, but this server is a
//! newline-delimited JSON-RPC transport rather than standard protobuf gRPC.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::watch;

use orbflow_core::error::OrbflowError;
use orbflow_core::execution::InstanceId;
use orbflow_core::ports::Engine;

use crate::types;

// Request / Response types (JSON wire format, matches Go grpcapi/types.go)

// These wire-format structs document the JSON contract even though request
// dispatch currently deserializes through `serde_json::Value` to support
// envelope-level auth inspection before typed decoding.
#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateWorkflowRequest {
    pub definition: Vec<u8>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct GetWorkflowRequest {
    pub id: String,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct WorkflowResponse {
    pub data: Vec<u8>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct ListRequest {
    #[serde(default)]
    pub offset: i32,
    #[serde(default)]
    pub limit: i32,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct ListWorkflowsResponse {
    pub items: Vec<Vec<u8>>,
    pub total: i64,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct StartWorkflowRequest {
    pub workflow_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<String>,
    #[serde(default)]
    pub input: Vec<u8>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct GetInstanceRequest {
    pub id: String,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct CancelInstanceRequest {
    pub id: String,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct InstanceResponse {
    pub data: Vec<u8>,
}

/// Envelope for JSON-RPC TCP communication.
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

const MAX_RPC_FRAME_BYTES: usize = 1 << 20;
const RPC_READ_DEADLINE: Duration = Duration::from_secs(30);

// GrpcServer

/// The Orbflow JSON-RPC TCP server wrapping an Engine.
///
/// Scope: exposes a subset of the HTTP API — workflow lifecycle only
/// (create, get, list, start, get instance, cancel instance). This keeps the
/// JSON contract minimal and stable for machine-to-machine integrations.
pub struct GrpcServer {
    engine: Arc<dyn Engine>,
    auth_token: Option<String>,
    shutdown_tx: watch::Sender<bool>,
    shutdown_rx: watch::Receiver<bool>,
}

impl GrpcServer {
    /// Creates a new JSON-RPC TCP server wrapping the engine.
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
    /// This is a TCP server using newline-delimited JSON, not protobuf gRPC.
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
            .map_err(|e| OrbflowError::Internal(format!("json-rpc: bind {addr}: {e}")))?;

        tracing::info!("JSON-RPC TCP server listening on {addr}");

        let mut shutdown_rx = self.shutdown_rx.clone();

        loop {
            tokio::select! {
                result = listener.accept() => {
                    let (stream, peer) = result.map_err(|e| {
                        OrbflowError::Internal(format!("json-rpc: accept: {e}"))
                    })?;

                    tracing::debug!("JSON-RPC TCP connection from {peer}");
                    let engine = self.engine.clone();
                    let auth_token = self.auth_token.clone();

                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(stream, engine, auth_token).await {
                            tracing::warn!("JSON-RPC TCP connection error from {peer}: {e}");
                        }
                    });
                }
                _ = shutdown_rx.changed() => {
                    tracing::info!("JSON-RPC TCP server shutting down");
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
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);

    loop {
        let frame = match read_rpc_frame(&mut reader).await? {
            FrameRead::Frame(frame) => frame,
            FrameRead::Eof => break,
            FrameRead::TooLarge => {
                let resp = error_response(
                    "RESOURCE_EXHAUSTED",
                    format!("request frame exceeds {MAX_RPC_FRAME_BYTES} bytes"),
                );
                write_rpc_response(&mut writer, &resp).await?;
                break;
            }
            FrameRead::Timeout => {
                let resp = error_response(
                    "DEADLINE_EXCEEDED",
                    format!("request read deadline exceeded after {RPC_READ_DEADLINE:?}"),
                );
                write_rpc_response(&mut writer, &resp).await?;
                break;
            }
        };

        // Parse into a raw JSON object first so we can extract auth_token
        // before deserializing the typed RpcRequest.
        let raw: serde_json::Value = match serde_json::from_slice(&frame) {
            Ok(v) => v,
            Err(e) => {
                let resp = error_response("INVALID_ARGUMENT", format!("invalid request: {e}"));
                write_rpc_response(&mut writer, &resp).await?;
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
                write_rpc_response(&mut writer, &resp).await?;
                continue;
            }
        }

        let request: RpcRequest = match serde_json::from_value(raw) {
            Ok(r) => r,
            Err(e) => {
                let resp = error_response("INVALID_ARGUMENT", format!("invalid request: {e}"));
                write_rpc_response(&mut writer, &resp).await?;
                continue;
            }
        };

        let response = dispatch(&engine, &request).await;
        write_rpc_response(&mut writer, &response).await?;
    }

    Ok(())
}

enum FrameRead {
    Frame(Vec<u8>),
    Eof,
    TooLarge,
    Timeout,
}

async fn read_rpc_frame<R>(reader: &mut R) -> Result<FrameRead, OrbflowError>
where
    R: AsyncBufRead + Unpin,
{
    let mut frame = Vec::new();

    loop {
        let available = match tokio::time::timeout(RPC_READ_DEADLINE, reader.fill_buf()).await {
            Ok(Ok(buf)) => buf,
            Ok(Err(e)) => return Err(OrbflowError::Internal(format!("json-rpc: read: {e}"))),
            Err(_) => return Ok(FrameRead::Timeout),
        };

        if available.is_empty() {
            return if frame.is_empty() {
                Ok(FrameRead::Eof)
            } else {
                Err(OrbflowError::Internal(
                    "json-rpc: connection closed mid-frame".into(),
                ))
            };
        }

        if let Some(pos) = available.iter().position(|&b| b == b'\n') {
            if frame.len().saturating_add(pos) > MAX_RPC_FRAME_BYTES {
                return Ok(FrameRead::TooLarge);
            }
            frame.extend_from_slice(&available[..pos]);
            reader.consume(pos + 1);
            if frame.last() == Some(&b'\r') {
                frame.pop();
            }
            return Ok(FrameRead::Frame(frame));
        }

        if frame.len().saturating_add(available.len()) > MAX_RPC_FRAME_BYTES {
            return Ok(FrameRead::TooLarge);
        }
        frame.extend_from_slice(available);
        let consumed = available.len();
        reader.consume(consumed);
    }
}

async fn write_rpc_response<W>(writer: &mut W, response: &RpcResponse) -> Result<(), OrbflowError>
where
    W: AsyncWrite + Unpin,
{
    let mut out = serde_json::to_vec(response)
        .map_err(|e| OrbflowError::Internal(format!("json-rpc: serialize response: {e}")))?;
    out.push(b'\n');
    writer
        .write_all(&out)
        .await
        .map_err(|e| OrbflowError::Internal(format!("json-rpc: write: {e}")))
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

fn decode_rpc_bytes(value: &serde_json::Value, field: &str) -> Result<Vec<u8>, RpcResponse> {
    if let Some(s) = value.as_str() {
        use base64::Engine as _;
        return base64::engine::general_purpose::STANDARD
            .decode(s)
            .map_err(|e| {
                error_response(
                    "INVALID_ARGUMENT",
                    format!("{field} is not valid base64: {e}"),
                )
            });
    }

    let Some(arr) = value.as_array() else {
        return Err(error_response(
            "INVALID_ARGUMENT",
            format!("{field} must be a base64 string or byte array"),
        ));
    };

    if arr.len() > MAX_RPC_FRAME_BYTES {
        return Err(error_response(
            "RESOURCE_EXHAUSTED",
            format!("{field} byte array exceeds {MAX_RPC_FRAME_BYTES} bytes"),
        ));
    }

    let mut out = Vec::with_capacity(arr.len());
    for (idx, item) in arr.iter().enumerate() {
        let Some(n) = item.as_u64() else {
            return Err(error_response(
                "INVALID_ARGUMENT",
                format!("{field}[{idx}] must be an integer in 0..=255"),
            ));
        };
        let byte = u8::try_from(n).map_err(|_| {
            error_response(
                "INVALID_ARGUMENT",
                format!("{field}[{idx}] is outside byte range 0..=255"),
            )
        })?;
        out.push(byte);
    }

    Ok(out)
}

fn parse_start_input(
    body: &serde_json::Value,
) -> Result<HashMap<String, serde_json::Value>, RpcResponse> {
    let Some(input_value) = body.get("input") else {
        return Ok(HashMap::new());
    };

    if input_value.is_null() {
        return Ok(HashMap::new());
    }

    if input_value.is_object() {
        return serde_json::from_value(input_value.clone()).map_err(|e| {
            error_response(
                "INVALID_ARGUMENT",
                format!("input object is invalid workflow input: {e}"),
            )
        });
    }

    if input_value.is_string() || input_value.is_array() {
        let bytes = decode_rpc_bytes(input_value, "input")?;
        return types::parse_input(&bytes).map_err(orbflow_error_to_response);
    }

    Err(error_response(
        "INVALID_ARGUMENT",
        "input must be an object, base64 string, byte array, or null",
    ))
}

async fn handle_create_workflow(engine: &Arc<dyn Engine>, body: &serde_json::Value) -> RpcResponse {
    let definition = match body.get("definition") {
        Some(value) => match decode_rpc_bytes(value, "definition") {
            Ok(bytes) => bytes,
            Err(resp) => return resp,
        },
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

    let input = match parse_start_input(body) {
        Ok(input) => input,
        Err(resp) => return resp,
    };

    if let Some(owner_id) = body.get("owner_id").filter(|value| !value.is_null()) {
        if owner_id
            .as_str()
            .map(str::trim)
            .filter(|owner_id| !owner_id.is_empty())
            .is_none()
        {
            return error_response("INVALID_ARGUMENT", "owner_id must be a non-empty string");
        }
        return error_response(
            "UNAUTHENTICATED",
            "owner-scoped StartWorkflow requires a trusted transport principal; request body owner_id is not accepted",
        );
    }

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

// Helpers

fn ok_response(data: serde_json::Value) -> RpcResponse {
    RpcResponse {
        data: Some(data),
        error: None,
    }
}

fn error_response(code: &str, message: impl Into<String>) -> RpcResponse {
    RpcResponse {
        data: None,
        error: Some(RpcError {
            code: code.to_owned(),
            message: message.into(),
        }),
    }
}

/// Maps a [`OrbflowError`] to an RPC error response.
fn orbflow_error_to_response(e: OrbflowError) -> RpcResponse {
    let code = if e.is_validation_error() {
        "INVALID_ARGUMENT"
    } else {
        match &e {
            OrbflowError::NotFound => "NOT_FOUND",
            OrbflowError::AlreadyExists => "ALREADY_EXISTS",
            OrbflowError::Conflict => "ABORTED",
            OrbflowError::Forbidden(_) => "PERMISSION_DENIED",
            OrbflowError::Cancelled => "CANCELLED",
            OrbflowError::Timeout => "DEADLINE_EXCEEDED",
            _ => "INTERNAL",
        }
    };

    error_response(code, e.to_string())
}
