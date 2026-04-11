// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Worker that subscribes to the task bus, executes nodes, and publishes results.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use orbflow_core::metering;
use orbflow_core::streaming::{StreamChunk, StreamMessage, StreamSender, StreamingNodeExecutor};
use orbflow_core::{
    Bus, NodeExecutor, NodeInput, NodeOutput, OrbflowError, ResultMessage, TaskMessage,
    WIRE_VERSION, result_subject, stream_subject, task_subject,
};
use tokio::sync::Notify;
use tracing::{Instrument, debug, error, info};
use tracing_opentelemetry::OpenTelemetrySpanExt;

use orbflow_core::telemetry::*;

/// Configuration options for [`Worker`].
pub struct WorkerOptions {
    /// Pool name for bus subject routing (default: `"default"`).
    pub pool_name: String,
    /// Maximum execution time per task (default: 5 minutes).
    pub task_timeout: Duration,
}

impl Default for WorkerOptions {
    fn default() -> Self {
        Self {
            pool_name: "default".into(),
            task_timeout: Duration::from_secs(300),
        }
    }
}

impl WorkerOptions {
    /// Creates a new set of default options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the pool name.
    pub fn pool_name(mut self, name: impl Into<String>) -> Self {
        self.pool_name = name.into();
        self
    }

    /// Sets the task timeout duration.
    pub fn task_timeout(mut self, timeout: Duration) -> Self {
        self.task_timeout = timeout;
        self
    }
}

/// Worker subscribes to the task bus and executes nodes.
///
/// It routes incoming [`TaskMessage`]s to the appropriate [`NodeExecutor`]
/// and publishes [`ResultMessage`]s back to the bus. For executors that
/// implement [`StreamingNodeExecutor`], it relays incremental chunks to
/// the stream bus subject for real-time UI updates.
pub struct Worker {
    executors: Arc<RwLock<HashMap<String, Arc<dyn NodeExecutor>>>>,
    streaming_executors: Arc<RwLock<HashMap<String, Arc<dyn StreamingNodeExecutor>>>>,
    bus: Arc<dyn Bus>,
    pool: String,
    task_timeout: Duration,
    stop_notify: Arc<Notify>,
}

impl Worker {
    /// Creates a new worker with the given bus and options.
    pub fn new(bus: Arc<dyn Bus>, options: WorkerOptions) -> Self {
        Self {
            executors: Arc::new(RwLock::new(HashMap::new())),
            streaming_executors: Arc::new(RwLock::new(HashMap::new())),
            bus,
            pool: options.pool_name,
            task_timeout: options.task_timeout,
            stop_notify: Arc::new(Notify::new()),
        }
    }

    /// Registers a node executor under the given name (plugin_ref).
    ///
    /// Used during initial setup before [`start`](Worker::start).
    pub fn register_node(&mut self, name: impl Into<String>, executor: Arc<dyn NodeExecutor>) {
        self.executors
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(name.into(), executor);
    }

    /// Registers a node executor at runtime (after the worker has started).
    ///
    /// Unlike [`register_node`](Worker::register_node) this takes `&self` so
    /// it can be called on a shared `Arc<Worker>`.
    pub fn register_node_dynamic(&self, name: impl Into<String>, executor: Arc<dyn NodeExecutor>) {
        self.executors
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(name.into(), executor);
    }

    /// Returns `true` if an executor is registered for the given plugin_ref.
    pub fn has_executor(&self, name: &str) -> bool {
        self.executors
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .contains_key(name)
    }

    /// Registers a streaming executor for real-time incremental output.
    pub fn register_streaming(
        &mut self,
        name: impl Into<String>,
        executor: Arc<dyn StreamingNodeExecutor>,
    ) {
        self.streaming_executors
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(name.into(), executor);
    }

    /// Starts processing tasks from the bus.
    ///
    /// This subscribes to the task subject for the worker's pool and
    /// processes incoming tasks until [`stop`](Worker::stop) is called.
    pub async fn start(&self) -> Result<(), OrbflowError> {
        let subject = task_subject(&self.pool);
        let result_subj = result_subject(&self.pool);
        let bus = Arc::clone(&self.bus);
        let task_timeout = self.task_timeout;
        let stop_notify = Arc::clone(&self.stop_notify);

        // Share the live executor registries so runtime-registered plugins
        // are visible to the handler without restart.
        let executors = Arc::clone(&self.executors);
        let streaming_executors = Arc::clone(&self.streaming_executors);
        let bus_for_handler = Arc::clone(&bus);

        let handler: orbflow_core::MsgHandler = Arc::new(move |_subject, data| {
            let executors = Arc::clone(&executors);
            let streaming_executors = Arc::clone(&streaming_executors);
            let bus = Arc::clone(&bus_for_handler);
            let result_subj = result_subj.clone();

            Box::pin(async move {
                handle_task(
                    &executors,
                    &streaming_executors,
                    &*bus,
                    &result_subj,
                    task_timeout,
                    &data,
                )
                .await
            })
        });

        bus.subscribe(&subject, handler).await?;

        info!(pool = %self.pool, "worker started");

        // Wait for stop signal.
        stop_notify.notified().await;

        info!(pool = %self.pool, "worker stopped");
        Ok(())
    }

    /// Gracefully stops the worker.
    pub async fn stop(&self) -> Result<(), OrbflowError> {
        self.stop_notify.notify_one();
        self.bus.close().await?;
        Ok(())
    }
}

/// Handles a single task message: deserialize, execute, publish result.
///
/// If a streaming executor is registered for the task's plugin_ref, chunks are
/// relayed to the bus stream subject for real-time UI updates.
async fn handle_task(
    executors: &RwLock<HashMap<String, Arc<dyn NodeExecutor>>>,
    streaming_executors: &RwLock<HashMap<String, Arc<dyn StreamingNodeExecutor>>>,
    bus: &dyn Bus,
    result_subject: &str,
    task_timeout: Duration,
    data: &[u8],
) -> Result<(), OrbflowError> {
    let task: TaskMessage = match serde_json::from_slice(data) {
        Ok(t) => t,
        Err(e) => {
            error!(error = %e, "failed to unmarshal task");
            return Err(OrbflowError::Internal(format!("unmarshal task: {e}")));
        }
    };

    // Reject tasks from newer, unknown wire versions to prevent silent
    // behavior divergence during rolling deployments.
    if task.v > WIRE_VERSION {
        tracing::warn!(
            v = task.v,
            current = WIRE_VERSION,
            instance = %task.instance_id,
            node = %task.node_id,
            "ignoring task with unknown wire version"
        );
        return Ok(());
    }

    info!(
        instance = %task.instance_id,
        node = %task.node_id,
        plugin = %task.plugin_ref,
        "executing node"
    );

    let span = tracing::info_span!(
        SPAN_WORKER_HANDLE_TASK,
        instance_id = %task.instance_id,
        node_id = %task.node_id,
        plugin_ref = %task.plugin_ref,
        attempt = task.attempt,
    );

    // Extract distributed trace context propagated from the engine through
    // NATS and link this worker span to the parent trace.
    if let Some(ref trace_ctx) = task.trace_context {
        let parent_cx = opentelemetry::global::get_text_map_propagator(|propagator| {
            propagator.extract(trace_ctx)
        });
        let _ = span.set_parent(parent_cx);
    }

    let exec_start = std::time::Instant::now();

    // Check if a streaming executor is registered before destructuring task.
    // Hold the read lock only long enough to clone the Arc.
    let streaming_exec = streaming_executors
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .get(&task.plugin_ref)
        .cloned();
    let non_streaming_exec = if streaming_exec.is_none() {
        executors
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(&task.plugin_ref)
            .cloned()
    } else {
        None
    };

    // Destructure task to move fields into NodeInput instead of cloning.
    // trace_context was already extracted above; v is Copy.
    let TaskMessage {
        instance_id,
        node_id,
        plugin_ref,
        config,
        input: task_input,
        parameters,
        capabilities,
        attempt,
        trace_context: _,
        v: _,
    } = task;

    let input = NodeInput {
        instance_id,
        node_id,
        plugin_ref,
        config,
        input: task_input,
        parameters,
        capabilities,
        attempt,
    };

    // Check if this executor supports streaming.
    // The streaming handler creates its own span, so we don't enter ours here.
    if let Some(streaming_exec) = streaming_exec {
        return handle_streaming_task(&streaming_exec, &input, bus, result_subject, task_timeout)
            .await;
    }

    // Non-streaming path: execute and publish result.
    // Provide a user-friendly error when the executor is missing instead of
    // the generic "node executor not registered" from execute_sandboxed.
    let exec_result = if non_streaming_exec.is_none() {
        Err(OrbflowError::InvalidNodeConfig(format!(
            "Plugin '{}' is not available. The plugin process may have failed to start — \
             check server logs for details, or try restarting the server.",
            input.plugin_ref
        )))
    } else {
        execute_sandboxed(non_streaming_exec.as_ref(), input.clone(), task_timeout)
            .instrument(span.clone())
            .await
    };
    let wall_time_ms = exec_start.elapsed().as_millis() as u64;
    let result = build_result_message(&input, exec_result, wall_time_ms);

    let duration = exec_start.elapsed();
    span.in_scope(|| {
        tracing::info!(
            instance_id = %input.instance_id,
            node_id = %input.node_id,
            plugin_ref = %input.plugin_ref,
            duration_ms = duration.as_millis() as u64,
            success = result.error.is_none(),
            "node execution completed"
        );
    });

    let result_data = serde_json::to_vec(&result)
        .map_err(|e| OrbflowError::Internal(format!("marshal result: {e}")))?;

    bus.publish(result_subject, &result_data).await
}

/// Handles a streaming task: relays chunks to bus, then publishes final result.
async fn handle_streaming_task(
    streaming_exec: &Arc<dyn StreamingNodeExecutor>,
    input: &NodeInput,
    bus: &dyn Bus,
    result_subject: &str,
    task_timeout: Duration,
) -> Result<(), OrbflowError> {
    let _span = tracing::info_span!(
        SPAN_WORKER_HANDLE_TASK,
        instance_id = %input.instance_id,
        node_id = %input.node_id,
        plugin_ref = %input.plugin_ref,
        streaming = true,
    );

    let stream_start = std::time::Instant::now();

    let (sender, mut rx) = StreamSender::channel(64);
    let stream_subj = stream_subject(&input.instance_id.0, &input.node_id);

    let exec = Arc::clone(streaming_exec);
    let input_owned = input.clone();
    let exec_handle = tokio::task::spawn(async move {
        tokio::time::timeout(task_timeout, exec.execute_streaming(&input_owned, sender)).await
    });

    // Relay chunks from the executor to the bus.
    let mut seq: u64 = 0;
    let mut final_output: Option<NodeOutput> = None;
    let mut final_error: Option<String> = None;

    while let Some(chunk) = rx.recv().await {
        let is_terminal = matches!(chunk, StreamChunk::Done { .. } | StreamChunk::Error { .. });

        // Extract final output/error before moving chunk into StreamMessage.
        match &chunk {
            StreamChunk::Done { output } => {
                final_output = Some(output.clone());
            }
            StreamChunk::Error { message } => {
                final_error = Some(message.clone());
            }
            StreamChunk::Data { .. } => {}
        }

        let msg = StreamMessage {
            instance_id: input.instance_id.clone(),
            node_id: input.node_id.clone(),
            chunk,
            seq,
            v: WIRE_VERSION,
        };

        if let Ok(data) = serde_json::to_vec(&msg)
            && let Err(e) = bus.publish(&stream_subj, &data).await
        {
            debug!(error = %e, "failed to publish stream chunk");
        }

        seq += 1;

        if is_terminal {
            break;
        }
    }

    // Wait for the executor to finish (it may have already).
    match exec_handle.await {
        Ok(Ok(Ok(()))) => {}
        Ok(Ok(Err(e))) => {
            if final_error.is_none() {
                final_error = Some(e.to_string());
            }
        }
        Ok(Err(_elapsed)) => {
            if final_error.is_none() {
                final_error = Some("streaming task timed out".into());
            }
        }
        Err(join_err) => {
            if final_error.is_none() {
                final_error = Some(format!("streaming task panicked: {join_err}"));
            }
        }
    }

    let wall_time_ms = stream_start.elapsed().as_millis() as u64;

    // Build and publish the final ResultMessage with embedded metrics.
    let result = if let Some(err) = final_error {
        ResultMessage {
            result_id: Some(uuid::Uuid::new_v4().to_string()),
            instance_id: input.instance_id.clone(),
            node_id: input.node_id.clone(),
            output: inject_metrics(None, wall_time_ms),
            error: Some(err),
            trace_context: None,
            v: WIRE_VERSION,
        }
    } else if let Some(output) = final_output {
        if let Some(ref err_msg) = output.error {
            ResultMessage {
                result_id: Some(uuid::Uuid::new_v4().to_string()),
                instance_id: input.instance_id.clone(),
                node_id: input.node_id.clone(),
                output: inject_metrics(output.data, wall_time_ms),
                error: Some(err_msg.clone()),
                trace_context: None,
                v: WIRE_VERSION,
            }
        } else {
            ResultMessage {
                result_id: Some(uuid::Uuid::new_v4().to_string()),
                instance_id: input.instance_id.clone(),
                node_id: input.node_id.clone(),
                output: inject_metrics(output.data, wall_time_ms),
                error: None,
                trace_context: None,
                v: WIRE_VERSION,
            }
        }
    } else {
        ResultMessage {
            result_id: Some(uuid::Uuid::new_v4().to_string()),
            instance_id: input.instance_id.clone(),
            node_id: input.node_id.clone(),
            output: inject_metrics(None, wall_time_ms),
            error: Some("streaming executor did not send terminal chunk".into()),
            trace_context: None,
            v: WIRE_VERSION,
        }
    };

    let result_data = serde_json::to_vec(&result)
        .map_err(|e| OrbflowError::Internal(format!("marshal result: {e}")))?;

    bus.publish(result_subject, &result_data).await
}

/// Internal key used to embed node metrics in the result output.
/// The engine strips this before storing the user-visible output.
const METRICS_KEY: &str = "_metrics";

/// Injects metering data into a result output map.
///
/// Extracts cost and token information from the node output (if present),
/// combines it with the measured wall time, and inserts a `_metrics` JSON
/// value into the output map. Returns a new map (or creates one) so the
/// original output is not mutated in place.
fn inject_metrics(
    output: Option<HashMap<String, serde_json::Value>>,
    wall_time_ms: u64,
) -> Option<HashMap<String, serde_json::Value>> {
    let base = output.unwrap_or_default();
    let node_metrics = metering::extract_metrics_from_output(&base, wall_time_ms);
    let mut result = base;
    if let Ok(metrics_value) = serde_json::to_value(&node_metrics) {
        result.insert(METRICS_KEY.to_owned(), metrics_value);
    }
    Some(result)
}

/// Builds a [`ResultMessage`] from the input and execution result.
fn build_result_message(
    input: &NodeInput,
    exec_result: Result<NodeOutput, OrbflowError>,
    wall_time_ms: u64,
) -> ResultMessage {
    match exec_result {
        Ok(output) => {
            if let Some(ref err_msg) = output.error {
                // Even on business-logic errors, inject metrics so the engine
                // can track cost/duration for failed nodes.
                ResultMessage {
                    result_id: Some(uuid::Uuid::new_v4().to_string()),
                    instance_id: input.instance_id.clone(),
                    node_id: input.node_id.clone(),
                    output: inject_metrics(output.data, wall_time_ms),
                    error: Some(err_msg.clone()),
                    trace_context: None,
                    v: WIRE_VERSION,
                }
            } else {
                ResultMessage {
                    result_id: Some(uuid::Uuid::new_v4().to_string()),
                    instance_id: input.instance_id.clone(),
                    node_id: input.node_id.clone(),
                    output: inject_metrics(output.data, wall_time_ms),
                    error: None,
                    trace_context: None,
                    v: WIRE_VERSION,
                }
            }
        }
        Err(e) => {
            // Infrastructure errors (timeout, panic, node not found) still
            // get a metrics entry with zero cost but measured wall time.
            ResultMessage {
                result_id: Some(uuid::Uuid::new_v4().to_string()),
                instance_id: input.instance_id.clone(),
                node_id: input.node_id.clone(),
                output: inject_metrics(None, wall_time_ms),
                error: Some(e.to_string()),
                trace_context: None,
                v: WIRE_VERSION,
            }
        }
    }
}

/// Builds a [`TaskMessage`] for testing purposes.
#[cfg(test)]
fn make_test_task(plugin_ref: &str) -> TaskMessage {
    TaskMessage {
        instance_id: orbflow_core::execution::InstanceId::new("inst-test"),
        node_id: "node-1".into(),
        plugin_ref: plugin_ref.into(),
        config: None,
        input: None,
        parameters: None,
        capabilities: None,
        attempt: 1,
        trace_context: None,
        v: WIRE_VERSION,
    }
}

/// Executes a node with timeout and panic isolation via `tokio::task::spawn`.
async fn execute_sandboxed(
    executor: Option<&Arc<dyn NodeExecutor>>,
    input: NodeInput,
    timeout: Duration,
) -> Result<NodeOutput, OrbflowError> {
    tracing::debug!(
        plugin_ref = %input.plugin_ref,
        instance_id = %input.instance_id,
        node_id = %input.node_id,
        "executing node in sandbox"
    );

    let executor = match executor {
        Some(e) => Arc::clone(e),
        None => return Err(OrbflowError::NodeNotFound),
    };

    // Spawn into a separate task for panic isolation.
    let handle =
        tokio::task::spawn(
            async move { tokio::time::timeout(timeout, executor.execute(&input)).await },
        );

    match handle.await {
        Ok(Ok(inner)) => inner,
        Ok(Err(_elapsed)) => {
            error!("task execution timed out after {:?}", timeout);
            Err(OrbflowError::Timeout)
        }
        Err(join_err) => {
            // JoinError means the task panicked or was cancelled.
            error!(error = %join_err, "task panicked");
            Err(OrbflowError::Internal(format!("task panicked: {join_err}")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::time::Duration;

    use async_trait::async_trait;
    use orbflow_core::ports::{NodeExecutor, NodeInput, NodeOutput};
    use orbflow_core::streaming::{
        StreamChunk, StreamMessage, StreamSender, StreamingNodeExecutor,
    };
    use orbflow_testutil::MockBus;

    /// A streaming executor that sends N data chunks then Done.
    struct FakeStreamingExecutor {
        chunks: Vec<serde_json::Value>,
        final_output: NodeOutput,
    }

    #[async_trait]
    impl StreamingNodeExecutor for FakeStreamingExecutor {
        async fn execute_streaming(
            &self,
            _input: &NodeInput,
            sender: StreamSender,
        ) -> Result<(), OrbflowError> {
            for chunk in &self.chunks {
                sender.send_data(chunk.clone()).await?;
            }
            sender.send_done(self.final_output.clone()).await?;
            Ok(())
        }
    }

    /// A streaming executor that sends one data chunk then an error.
    struct ErrorStreamingExecutor;

    #[async_trait]
    impl StreamingNodeExecutor for ErrorStreamingExecutor {
        async fn execute_streaming(
            &self,
            _input: &NodeInput,
            sender: StreamSender,
        ) -> Result<(), OrbflowError> {
            sender
                .send_data(serde_json::json!({"token": "partial"}))
                .await?;
            sender.send_error("upstream API failed".into()).await?;
            Ok(())
        }
    }

    fn make_task_bytes(plugin_ref: &str) -> Vec<u8> {
        let task = make_test_task(plugin_ref);
        serde_json::to_vec(&task).unwrap()
    }

    // ─── Test: streaming chunks relayed ─────────────────────────────────

    #[tokio::test]
    async fn test_streaming_chunks_relayed() {
        let bus = Arc::new(MockBus::new());
        let mut data = HashMap::new();
        data.insert("content".into(), serde_json::json!("abc"));

        let streaming_exec: Arc<dyn StreamingNodeExecutor> = Arc::new(FakeStreamingExecutor {
            chunks: vec![
                serde_json::json!({"token": "a"}),
                serde_json::json!({"token": "b"}),
                serde_json::json!({"token": "c"}),
            ],
            final_output: NodeOutput {
                data: Some(data),
                error: None,
            },
        });

        let executors = RwLock::new(HashMap::<String, Arc<dyn NodeExecutor>>::new());
        let mut streaming_map_inner = HashMap::<String, Arc<dyn StreamingNodeExecutor>>::new();
        streaming_map_inner.insert("builtin:stream".into(), streaming_exec);
        let streaming_map = RwLock::new(streaming_map_inner);

        let task_data = make_task_bytes("builtin:stream");
        let result_subj = result_subject("default");

        handle_task(
            &executors,
            &streaming_map,
            &*bus,
            &result_subj,
            Duration::from_secs(10),
            &task_data,
        )
        .await
        .unwrap();

        // Expect 4 stream messages (3 Data + 1 Done) on the stream subject.
        let stream_subj = stream_subject("inst-test", "node-1");
        let stream_msgs = bus.messages_for(&stream_subj);
        assert_eq!(
            stream_msgs.len(),
            4,
            "expected 3 Data + 1 Done stream messages, got {}",
            stream_msgs.len()
        );

        // Verify sequence numbers are 0..3.
        for (i, msg) in stream_msgs.iter().enumerate() {
            let sm: StreamMessage = serde_json::from_slice(&msg.data).unwrap();
            assert_eq!(sm.seq, i as u64, "expected seq={i}, got {}", sm.seq);
        }

        // Last stream message should be Done.
        let last: StreamMessage = serde_json::from_slice(&stream_msgs[3].data).unwrap();
        assert!(
            matches!(last.chunk, StreamChunk::Done { .. }),
            "expected Done chunk"
        );

        // Expect 1 ResultMessage on the result subject.
        let result_msgs = bus.messages_for(&result_subj);
        assert_eq!(result_msgs.len(), 1);
        let result: ResultMessage = serde_json::from_slice(&result_msgs[0].data).unwrap();
        assert!(result.error.is_none(), "expected no error in result");
        assert!(result.output.is_some(), "expected output in result");
    }

    // ─── Test: streaming error terminates ───────────────────────────────

    #[tokio::test]
    async fn test_streaming_error_terminates() {
        let bus = Arc::new(MockBus::new());

        let streaming_exec: Arc<dyn StreamingNodeExecutor> = Arc::new(ErrorStreamingExecutor);

        let executors = RwLock::new(HashMap::<String, Arc<dyn NodeExecutor>>::new());
        let mut streaming_map_inner = HashMap::<String, Arc<dyn StreamingNodeExecutor>>::new();
        streaming_map_inner.insert("builtin:err_stream".into(), streaming_exec);
        let streaming_map = RwLock::new(streaming_map_inner);

        let task_data = make_task_bytes("builtin:err_stream");
        let result_subj = result_subject("default");

        handle_task(
            &executors,
            &streaming_map,
            &*bus,
            &result_subj,
            Duration::from_secs(10),
            &task_data,
        )
        .await
        .unwrap();

        // Expect 2 stream messages: 1 Data + 1 Error.
        let stream_subj = stream_subject("inst-test", "node-1");
        let stream_msgs = bus.messages_for(&stream_subj);
        assert_eq!(
            stream_msgs.len(),
            2,
            "expected 1 Data + 1 Error stream messages, got {}",
            stream_msgs.len()
        );

        // Second stream message should be Error.
        let error_msg: StreamMessage = serde_json::from_slice(&stream_msgs[1].data).unwrap();
        match error_msg.chunk {
            StreamChunk::Error { ref message } => {
                assert_eq!(message, "upstream API failed");
            }
            _ => panic!("expected Error chunk, got {:?}", error_msg.chunk),
        }

        // ResultMessage should carry the error.
        let result_msgs = bus.messages_for(&result_subj);
        assert_eq!(result_msgs.len(), 1);
        let result: ResultMessage = serde_json::from_slice(&result_msgs[0].data).unwrap();
        assert!(result.error.is_some(), "expected error in result");
        assert_eq!(result.error.unwrap(), "upstream API failed");
    }

    // ─── Test: non-streaming still works ────────────────────────────────

    #[tokio::test]
    async fn test_non_streaming_still_works() {
        let bus = Arc::new(MockBus::new());

        let mut output_data = HashMap::new();
        output_data.insert("status".into(), serde_json::json!("ok"));

        let exec: Arc<dyn NodeExecutor> = Arc::new(
            orbflow_testutil::MockNodeExecutor::with_output(NodeOutput {
                data: Some(output_data.clone()),
                error: None,
            }),
        );

        let mut executors_inner = HashMap::<String, Arc<dyn NodeExecutor>>::new();
        executors_inner.insert("builtin:http".into(), exec);
        let executors = RwLock::new(executors_inner);
        let streaming_map = RwLock::new(HashMap::<String, Arc<dyn StreamingNodeExecutor>>::new());

        let task_data = make_task_bytes("builtin:http");
        let result_subj = result_subject("default");

        handle_task(
            &executors,
            &streaming_map,
            &*bus,
            &result_subj,
            Duration::from_secs(10),
            &task_data,
        )
        .await
        .unwrap();

        // No stream messages should exist.
        let stream_subj = stream_subject("inst-test", "node-1");
        let stream_msgs = bus.messages_for(&stream_subj);
        assert!(
            stream_msgs.is_empty(),
            "expected no stream messages for non-streaming executor"
        );

        // Exactly 1 ResultMessage on the result subject.
        let result_msgs = bus.messages_for(&result_subj);
        assert_eq!(result_msgs.len(), 1);
        let result: ResultMessage = serde_json::from_slice(&result_msgs[0].data).unwrap();
        assert!(result.error.is_none(), "expected no error");
        assert!(result.output.is_some(), "expected output");
        let out = result.output.unwrap();
        assert_eq!(out.get("status").unwrap(), &serde_json::json!("ok"));
    }
}
