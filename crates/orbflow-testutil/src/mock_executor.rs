// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Mock implementation of [`NodeExecutor`] for testing.
//!
//! Returns a configurable output (or error) and counts invocations.

use std::sync::atomic::{AtomicI64, Ordering};

use async_trait::async_trait;

use orbflow_core::error::OrbflowError;
use orbflow_core::ports::{NodeExecutor, NodeInput, NodeOutput};

/// Mock node executor that returns a pre-configured result.
///
/// `calls` is incremented atomically and is safe for concurrent use.
pub struct MockNodeExecutor {
    output: Option<NodeOutput>,
    error: Option<OrbflowError>,
    calls: AtomicI64,
}

impl MockNodeExecutor {
    /// Creates a mock executor that always returns the given output.
    pub fn with_output(output: NodeOutput) -> Self {
        Self {
            output: Some(output),
            error: None,
            calls: AtomicI64::new(0),
        }
    }

    /// Creates a mock executor that always returns the given error.
    pub fn with_error(err: OrbflowError) -> Self {
        Self {
            output: None,
            error: Some(err),
            calls: AtomicI64::new(0),
        }
    }

    /// Creates a mock executor that returns an empty successful output.
    pub fn ok() -> Self {
        Self::with_output(NodeOutput {
            data: None,
            error: None,
        })
    }

    /// Returns how many times [`execute`](NodeExecutor::execute) has been called.
    pub fn call_count(&self) -> i64 {
        self.calls.load(Ordering::SeqCst)
    }

    /// Resets the call counter to zero.
    pub fn reset_calls(&self) {
        self.calls.store(0, Ordering::SeqCst);
    }
}

#[async_trait]
impl NodeExecutor for MockNodeExecutor {
    async fn execute(&self, _input: &NodeInput) -> Result<NodeOutput, OrbflowError> {
        self.calls.fetch_add(1, Ordering::SeqCst);

        if let Some(ref err) = self.error {
            // Re-create the error since OrbflowError is not Clone.
            // We match on the stored variant to produce an equivalent error.
            return Err(recreate_error(err));
        }

        Ok(self.output.clone().unwrap_or(NodeOutput {
            data: None,
            error: None,
        }))
    }
}

/// Best-effort recreation of a [`OrbflowError`] variant.
///
/// `OrbflowError` does not implement `Clone`, so we match on the stored reference
/// and produce an equivalent new value.
fn recreate_error(err: &OrbflowError) -> OrbflowError {
    match err {
        OrbflowError::NotFound => OrbflowError::NotFound,
        OrbflowError::AlreadyExists => OrbflowError::AlreadyExists,
        OrbflowError::CycleDetected => OrbflowError::CycleDetected,
        OrbflowError::DuplicateNode => OrbflowError::DuplicateNode,
        OrbflowError::DuplicateEdge => OrbflowError::DuplicateEdge,
        OrbflowError::InvalidEdge => OrbflowError::InvalidEdge,
        OrbflowError::NoEntryNodes => OrbflowError::NoEntryNodes,
        OrbflowError::Disconnected => OrbflowError::Disconnected,
        OrbflowError::InvalidStatus => OrbflowError::InvalidStatus,
        OrbflowError::Cancelled => OrbflowError::Cancelled,
        OrbflowError::Timeout => OrbflowError::Timeout,
        OrbflowError::EngineStopped => OrbflowError::EngineStopped,
        OrbflowError::NodeNotFound => OrbflowError::NodeNotFound,
        OrbflowError::Conflict => OrbflowError::Conflict,
        OrbflowError::InvalidNodeKind => OrbflowError::InvalidNodeKind,
        OrbflowError::MissingCapability => OrbflowError::MissingCapability,
        OrbflowError::InvalidCapabilityEdge => OrbflowError::InvalidCapabilityEdge,
        OrbflowError::InvalidNodeConfig(s) => OrbflowError::InvalidNodeConfig(s.clone()),
        OrbflowError::EmptyNodeKind => OrbflowError::EmptyNodeKind,
        OrbflowError::NilExecutor => OrbflowError::NilExecutor,
        OrbflowError::DuplicateNodeKind(s) => OrbflowError::DuplicateNodeKind(s.clone()),
        OrbflowError::Internal(s) => OrbflowError::Internal(s.clone()),
        OrbflowError::Crypto(s) => OrbflowError::Crypto(s.clone()),
        OrbflowError::Database(s) => OrbflowError::Database(s.clone()),
        OrbflowError::Bus(s) => OrbflowError::Bus(s.clone()),
        OrbflowError::Forbidden(s) => OrbflowError::Forbidden(s.clone()),
        OrbflowError::BudgetExceeded(s) => OrbflowError::BudgetExceeded(s.clone()),
        OrbflowError::InvalidPolicy(s) => OrbflowError::InvalidPolicy(s.clone()),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use orbflow_core::execution::InstanceId;

    use super::*;

    fn dummy_input() -> NodeInput {
        NodeInput {
            instance_id: InstanceId::new("inst-1"),
            node_id: "node-1".into(),
            plugin_ref: "builtin:test".into(),
            config: None,
            input: None,
            parameters: None,
            capabilities: None,
            attempt: 1,
        }
    }

    #[tokio::test]
    async fn test_returns_configured_output() {
        let mut data = HashMap::new();
        data.insert("key".into(), serde_json::json!("value"));
        let exec = MockNodeExecutor::with_output(NodeOutput {
            data: Some(data),
            error: None,
        });

        let result = exec.execute(&dummy_input()).await.unwrap();
        assert_eq!(
            result.data.unwrap().get("key").unwrap(),
            &serde_json::json!("value")
        );
        assert_eq!(exec.call_count(), 1);
    }

    #[tokio::test]
    async fn test_returns_configured_error() {
        let exec = MockNodeExecutor::with_error(OrbflowError::Internal("boom".into()));
        let result = exec.execute(&dummy_input()).await;
        assert!(result.is_err());
        assert_eq!(exec.call_count(), 1);
    }

    #[tokio::test]
    async fn test_ok_returns_empty_output() {
        let exec = MockNodeExecutor::ok();
        let result = exec.execute(&dummy_input()).await.unwrap();
        assert!(result.data.is_none());
        assert!(result.error.is_none());
    }

    #[tokio::test]
    async fn test_call_count_increments() {
        let exec = MockNodeExecutor::ok();
        assert_eq!(exec.call_count(), 0);

        exec.execute(&dummy_input()).await.unwrap();
        exec.execute(&dummy_input()).await.unwrap();
        exec.execute(&dummy_input()).await.unwrap();
        assert_eq!(exec.call_count(), 3);
    }

    #[tokio::test]
    async fn test_reset_calls() {
        let exec = MockNodeExecutor::ok();
        exec.execute(&dummy_input()).await.unwrap();
        assert_eq!(exec.call_count(), 1);

        exec.reset_calls();
        assert_eq!(exec.call_count(), 0);
    }

    #[tokio::test]
    async fn test_error_can_be_returned_multiple_times() {
        let exec = MockNodeExecutor::with_error(OrbflowError::Timeout);
        for _ in 0..3 {
            assert!(exec.execute(&dummy_input()).await.is_err());
        }
        assert_eq!(exec.call_count(), 3);
    }
}
