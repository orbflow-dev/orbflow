// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Sub-workflow executor: starts a child workflow and polls until completion.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use async_trait::async_trait;
use tracing::info;

use orbflow_core::OrbflowError;
use orbflow_core::execution::InstanceStatus;
use orbflow_core::ports::{
    Engine, FieldSchema, FieldType, NodeExecutor, NodeInput, NodeOutput, NodeSchema,
    NodeSchemaProvider,
};
use orbflow_core::workflow::WorkflowId;

/// Global semaphore: bounds concurrent sub-workflow pollers to prevent
/// unbounded task accumulation and database pressure.
static SUB_WORKFLOW_SEMAPHORE: LazyLock<tokio::sync::Semaphore> =
    LazyLock::new(|| tokio::sync::Semaphore::new(32));

/// Plugin reference for the built-in sub-workflow executor.
pub const SUB_WORKFLOW_PLUGIN_REF: &str = "builtin:sub-workflow";

/// Default maximum time to wait for a child workflow to complete.
const DEFAULT_SUB_WORKFLOW_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// Default polling interval.
const DEFAULT_POLL_INTERVAL: Duration = Duration::from_secs(2);

/// Starts a child workflow and polls until it completes. Registered as a
/// built-in node executor.
pub struct SubWorkflowExecutor {
    engine: Arc<dyn Engine>,
    /// Kept for API backward-compatibility with `with_timing`; the executor
    /// now uses exponential backoff instead of a fixed interval.
    #[allow(dead_code)]
    poll_interval: Duration,
    max_wait: Duration,
}

impl SubWorkflowExecutor {
    /// Creates a sub-workflow executor tied to an engine.
    pub fn new(engine: Arc<dyn Engine>) -> Self {
        Self {
            engine,
            poll_interval: DEFAULT_POLL_INTERVAL,
            max_wait: DEFAULT_SUB_WORKFLOW_TIMEOUT,
        }
    }

    /// Creates a sub-workflow executor with custom timing parameters.
    pub fn with_timing(
        engine: Arc<dyn Engine>,
        poll_interval: Duration,
        max_wait: Duration,
    ) -> Self {
        Self {
            engine,
            poll_interval,
            max_wait,
        }
    }
}

impl NodeSchemaProvider for SubWorkflowExecutor {
    fn node_schema(&self) -> NodeSchema {
        NodeSchema {
            plugin_ref: SUB_WORKFLOW_PLUGIN_REF.into(),
            name: "Sub-Workflow".into(),
            description: "Execute another workflow as a child process".into(),
            category: "builtin".into(),
            node_kind: None,
            icon: "workflow".into(),
            color: "#8b5cf6".into(),
            docs: None,
            image_url: Some("/icons/workflow.svg".into()),
            inputs: vec![FieldSchema {
                key: "workflow_id".into(),
                label: "Workflow ID".into(),
                field_type: FieldType::String,
                required: true,
                default: None,
                description: Some("ID of the child workflow to execute".into()),
                r#enum: vec![],
                credential_type: None,
            }],
            outputs: vec![
                FieldSchema {
                    key: "child_instance_id".into(),
                    label: "Child Instance ID".into(),
                    field_type: FieldType::String,
                    required: false,
                    default: None,
                    description: None,
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "child_workflow_id".into(),
                    label: "Child Workflow ID".into(),
                    field_type: FieldType::String,
                    required: false,
                    default: None,
                    description: None,
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "status".into(),
                    label: "Status".into(),
                    field_type: FieldType::String,
                    required: false,
                    default: None,
                    description: None,
                    r#enum: vec![],
                    credential_type: None,
                },
                FieldSchema {
                    key: "outputs".into(),
                    label: "Outputs".into(),
                    field_type: FieldType::Object,
                    required: false,
                    default: None,
                    description: None,
                    r#enum: vec![],
                    credential_type: None,
                },
            ],
            parameters: vec![],
            capability_ports: vec![],
            settings: vec![],
            provides_capability: None,
        }
    }
}

#[async_trait]
impl NodeExecutor for SubWorkflowExecutor {
    async fn execute(&self, input: &NodeInput) -> Result<NodeOutput, OrbflowError> {
        let wf_id = input
            .input
            .as_ref()
            .and_then(|m| m.get("workflow_id"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                OrbflowError::Internal("sub-workflow: workflow_id is required in input".into())
            })?
            .to_owned();

        if wf_id.is_empty()
            || wf_id.len() > 128
            || !wf_id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(OrbflowError::InvalidNodeConfig(
                "sub-workflow: workflow_id must be 1-128 alphanumeric/hyphen/underscore characters"
                    .into(),
            ));
        }

        // Collect child workflow input (everything except workflow_id).
        let child_input: HashMap<String, serde_json::Value> = input
            .input
            .as_ref()
            .map(|m| {
                m.iter()
                    .filter(|(k, _)| k.as_str() != "workflow_id")
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect()
            })
            .unwrap_or_default();

        // Start child workflow.
        let child_inst = self
            .engine
            .start_workflow(&WorkflowId::new(&wf_id), child_input)
            .await
            .map_err(|e| OrbflowError::Internal(format!("sub-workflow: start failed: {e}")))?;

        info!(
            parent = %input.instance_id,
            child = %child_inst.id,
            child_workflow = wf_id.as_str(),
            "sub-workflow started"
        );

        // Exponential backoff polling with global concurrency limit.
        let deadline = tokio::time::Instant::now() + self.max_wait;
        let mut backoff = Duration::from_millis(200);
        const MAX_BACKOFF: Duration = Duration::from_secs(10);

        loop {
            // Acquire semaphore permit (limits concurrent pollers).
            // The permit is acquired per iteration so a long-running child at
            // max backoff (10 s) does not hold a slot during its sleep.
            let _permit = tokio::select! {
                permit = SUB_WORKFLOW_SEMAPHORE.acquire() => {
                    permit.map_err(|_| OrbflowError::Internal(
                        "sub-workflow semaphore closed".into()
                    ))?
                }
                _ = tokio::time::sleep_until(deadline) => {
                    return Err(OrbflowError::Internal(format!(
                        "sub-workflow: timed out waiting for child {} after {:?}",
                        child_inst.id, self.max_wait
                    )));
                }
            };

            // Sleep with backoff, respecting deadline.
            tokio::select! {
                _ = tokio::time::sleep(backoff) => {}
                _ = tokio::time::sleep_until(deadline) => {
                    return Err(OrbflowError::Internal(format!(
                        "sub-workflow: timed out waiting for child {} after {:?}",
                        child_inst.id, self.max_wait
                    )));
                }
            }

            let child = self
                .engine
                .get_instance(&child_inst.id)
                .await
                .map_err(|e| {
                    OrbflowError::Internal(format!(
                        "sub-workflow: failed to check child {}: {e}",
                        child_inst.id
                    ))
                })?;

            // Release concurrency slot before processing result.
            drop(_permit);

            if !child.is_terminal() {
                // Exponential backoff: double interval, cap at MAX_BACKOFF.
                backoff = (backoff * 2).min(MAX_BACKOFF);
                continue;
            }

            if child.status == InstanceStatus::Failed {
                return Err(OrbflowError::Internal(format!(
                    "sub-workflow: child {} failed",
                    child_inst.id
                )));
            }
            if child.status == InstanceStatus::Cancelled {
                return Err(OrbflowError::Internal(format!(
                    "sub-workflow: child {} cancelled",
                    child_inst.id
                )));
            }

            // Child completed successfully — collect outputs.
            let mut outputs = serde_json::Map::new();
            for (node_id, ns) in &child.node_states {
                if let Some(ref output) = ns.output {
                    outputs.insert(
                        node_id.clone(),
                        serde_json::Value::Object(
                            output.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
                        ),
                    );
                }
            }

            let mut data = HashMap::new();
            data.insert(
                "child_instance_id".into(),
                serde_json::Value::String(child_inst.id.0.clone()),
            );
            data.insert("child_workflow_id".into(), serde_json::Value::String(wf_id));
            data.insert("status".into(), serde_json::json!(child.status));
            data.insert("outputs".into(), serde_json::Value::Object(outputs));

            return Ok(NodeOutput {
                data: Some(data),
                error: None,
            });
        }
    }
}
