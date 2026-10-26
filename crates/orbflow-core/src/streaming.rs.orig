// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Streaming types for node executors that produce incremental output.
//!
//! Some node types (notably LLM/AI nodes) can produce output incrementally
//! rather than waiting for the full result. This module defines the types
//! and traits needed to support streaming execution.
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────┐   StreamChunk    ┌──────────────┐   StreamMessage   ┌─────┐
//! │ NodeExecutor │ ──────────────▶  │    Worker     │ ────────────────▶ │ Bus │
//! │  (streaming) │   via channel    │ (relay loop)  │   via publish()  │     │
//! └──────────────┘                  └──────────────┘                   └─────┘
//! ```
//!
//! The worker creates a channel, passes the sender to the executor, and
//! relays chunks to the bus as `StreamMessage`. The engine or SSE endpoint
//! subscribes to the stream subject and forwards chunks to the UI.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::mpsc;

use crate::error::OrbflowError;
use crate::execution::InstanceId;
use crate::ports::{NodeInput, NodeOutput};

// ─── Types ──────────────────────────────────────────────────────────────────

/// A chunk of streaming data from a node executor.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamChunk {
    /// Incremental data (e.g., a single LLM token or partial result).
    Data { payload: Value },
    /// Stream completed successfully with the final aggregated output.
    Done { output: NodeOutput },
    /// Stream encountered an error and is terminating.
    Error { message: String },
}

/// Wire format for streaming chunks published to the bus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamMessage {
    pub instance_id: InstanceId,
    pub node_id: String,
    pub chunk: StreamChunk,
    /// Monotonically increasing sequence number within this stream.
    pub seq: u64,
    /// Wire format version for backward-compatible evolution.
    #[serde(default = "default_stream_wire_version")]
    pub v: u8,
}

fn default_stream_wire_version() -> u8 {
    1
}

// ─── StreamSender ───────────────────────────────────────────────────────────

/// Sender handle for pushing stream chunks from a node executor.
///
/// Wraps a `tokio::sync::mpsc::Sender<StreamChunk>` with convenience methods
/// for the common chunk types.
#[derive(Clone)]
pub struct StreamSender {
    tx: mpsc::Sender<StreamChunk>,
}

impl StreamSender {
    pub fn new(tx: mpsc::Sender<StreamChunk>) -> Self {
        Self { tx }
    }

    /// Create a channel pair (sender, receiver) with the given buffer size.
    pub fn channel(buffer: usize) -> (Self, mpsc::Receiver<StreamChunk>) {
        let (tx, rx) = mpsc::channel(buffer);
        (Self::new(tx), rx)
    }

    /// Send a data chunk (e.g., an LLM token).
    pub async fn send_data(&self, payload: Value) -> Result<(), OrbflowError> {
        self.tx
            .send(StreamChunk::Data { payload })
            .await
            .map_err(|_| OrbflowError::Internal("stream receiver dropped".into()))
    }

    /// Send the final output, completing the stream.
    pub async fn send_done(&self, output: NodeOutput) -> Result<(), OrbflowError> {
        self.tx
            .send(StreamChunk::Done { output })
            .await
            .map_err(|_| OrbflowError::Internal("stream receiver dropped".into()))
    }

    /// Send an error, terminating the stream.
    pub async fn send_error(&self, message: String) -> Result<(), OrbflowError> {
        self.tx
            .send(StreamChunk::Error { message })
            .await
            .map_err(|_| OrbflowError::Internal("stream receiver dropped".into()))
    }
}

// ─── StreamingNodeExecutor ──────────────────────────────────────────────────

/// Optional trait for node executors that support incremental streaming output.
///
/// Executors implementing this trait produce output incrementally (e.g., LLM
/// tokens) via the provided [`StreamSender`]. The final aggregated output is
/// sent as [`StreamChunk::Done`].
///
/// # Contract
///
/// - The implementation MUST send exactly one terminal chunk: either
///   `StreamChunk::Done` (success) or `StreamChunk::Error` (failure).
/// - Zero or more `StreamChunk::Data` chunks may precede the terminal chunk.
/// - The non-streaming `NodeExecutor::execute()` MUST still work as a fallback
///   for workers that don't support streaming.
///
/// # Example
///
/// ```ignore
/// #[async_trait]
/// impl StreamingNodeExecutor for AiChatNode {
///     async fn execute_streaming(
///         &self,
///         input: &NodeInput,
///         sender: StreamSender,
///     ) -> Result<(), OrbflowError> {
///         // Stream tokens one by one
///         for token in stream_from_llm(input).await? {
///             sender.send_data(json!({"token": token})).await?;
///         }
///         // Send final aggregated output
///         sender.send_done(NodeOutput { data: Some(full_response), error: None }).await?;
///         Ok(())
///     }
/// }
/// ```
#[async_trait]
pub trait StreamingNodeExecutor: Send + Sync {
    /// Execute the node, sending incremental chunks via the sender.
    async fn execute_streaming(
        &self,
        input: &NodeInput,
        sender: StreamSender,
    ) -> Result<(), OrbflowError>;
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_stream_sender_data_and_done() {
        let (sender, mut rx) = StreamSender::channel(16);

        sender
            .send_data(serde_json::json!({"token": "Hello"}))
            .await
            .unwrap();
        sender
            .send_data(serde_json::json!({"token": " world"}))
            .await
            .unwrap();
        sender
            .send_done(NodeOutput {
                data: Some(HashMap::from([(
                    "content".into(),
                    serde_json::json!("Hello world"),
                )])),
                error: None,
            })
            .await
            .unwrap();

        // Verify chunks arrive in order
        let c1 = rx.recv().await.unwrap();
        assert!(matches!(c1, StreamChunk::Data { .. }));

        let c2 = rx.recv().await.unwrap();
        assert!(matches!(c2, StreamChunk::Data { .. }));

        let c3 = rx.recv().await.unwrap();
        assert!(matches!(c3, StreamChunk::Done { .. }));
    }

    #[tokio::test]
    async fn test_stream_sender_error() {
        let (sender, mut rx) = StreamSender::channel(4);

        sender.send_error("API rate limited".into()).await.unwrap();

        let chunk = rx.recv().await.unwrap();
        match chunk {
            StreamChunk::Error { message } => assert_eq!(message, "API rate limited"),
            _ => panic!("expected error chunk"),
        }
    }

    #[tokio::test]
    async fn test_stream_sender_dropped_receiver() {
        let (sender, rx) = StreamSender::channel(1);
        drop(rx);

        let result = sender.send_data(serde_json::json!("token")).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_stream_chunk_serde() {
        let chunk = StreamChunk::Data {
            payload: serde_json::json!({"token": "hi"}),
        };
        let json = serde_json::to_string(&chunk).unwrap();
        assert!(json.contains("\"type\":\"data\""));

        let deserialized: StreamChunk = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, StreamChunk::Data { .. }));
    }

    #[test]
    fn test_stream_message_serde() {
        let msg = StreamMessage {
            instance_id: InstanceId::new("inst-1"),
            node_id: "node-1".into(),
            chunk: StreamChunk::Data {
                payload: serde_json::json!({"token": "hello"}),
            },
            seq: 0,
            v: 1,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let msg2: StreamMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg2.seq, 0);
        assert_eq!(msg2.node_id, "node-1");
        assert_eq!(msg2.v, 1);
    }

    #[test]
    fn test_stream_message_backward_compat_no_version() {
        let json = r#"{"instance_id":"inst-1","node_id":"n1","chunk":{"type":"data","payload":"hi"},"seq":0}"#;
        let msg: StreamMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.v, 1);
    }
}
