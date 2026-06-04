// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Wire format types for the message bus — the contract between engine and worker.

use std::{collections::HashMap, error::Error, fmt};

use serde::{Deserialize, Serialize};

use crate::execution::InstanceId;

/// Wire format for tasks published to the bus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskMessage {
    pub instance_id: InstanceId,
    pub node_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dispatch_id: Option<String>,
    pub plugin_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<HashMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<HashMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<HashMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<HashMap<String, serde_json::Value>>,
    #[serde(default)]
    pub attempt: i32,
    /// W3C TraceContext headers for distributed trace propagation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_context: Option<HashMap<String, String>>,
    /// Wire format version for backward-compatible evolution.
    #[serde(default = "default_wire_version")]
    pub v: u8,
}

/// Wire format for results received from workers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultMessage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_id: Option<String>,
    pub instance_id: InstanceId,
    pub node_id: String,
    #[serde(default)]
    pub attempt: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dispatch_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<HashMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// W3C TraceContext headers propagated back from the worker.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_context: Option<HashMap<String, String>>,
    /// Wire format version for backward-compatible evolution.
    #[serde(default = "default_wire_version")]
    pub v: u8,
}

/// Current wire format version for bus messages.
pub const WIRE_VERSION: u8 = 1;

/// Stable identity for a single dispatch attempt.
///
/// The engine derives this from durable state (`instance_id`, `node_id`, and
/// `attempt`) so crash recovery can reject stale worker results without adding
/// another persisted field to every node state.
pub fn dispatch_identity(instance_id: &InstanceId, node_id: &str, attempt: i32) -> String {
    format!("{}:{node_id}:{attempt}", instance_id.0)
}

/// Result identity validation path accepted for a worker result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultIdentityMode {
    /// Result carried a matching dispatch_id and exact attempt.
    Modern,
    /// Legacy v1 result omitted dispatch_id and was accepted by explicit policy.
    LegacyV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResultIdentityError {
    InstanceMismatch {
        expected: InstanceId,
        actual: InstanceId,
    },
    NodeMismatch {
        expected: String,
        actual: String,
    },
    AttemptMismatch {
        node_id: String,
        expected: i32,
        actual: i32,
    },
    MissingDispatchId {
        node_id: String,
    },
    DispatchIdMismatch {
        node_id: String,
        expected: String,
        actual: String,
    },
}

impl fmt::Display for ResultIdentityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InstanceMismatch { expected, actual } => write!(
                f,
                "result instance_id {actual} does not match expected instance_id {expected}"
            ),
            Self::NodeMismatch { expected, actual } => {
                write!(
                    f,
                    "result node_id {actual} does not match expected node_id {expected}"
                )
            }
            Self::AttemptMismatch {
                node_id,
                expected,
                actual,
            } => write!(
                f,
                "stale result for node {node_id}: attempt {actual} does not match expected attempt {expected}"
            ),
            Self::MissingDispatchId { node_id } => {
                write!(f, "result for node {node_id} is missing dispatch_id")
            }
            Self::DispatchIdMismatch {
                node_id,
                expected,
                actual,
            } => write!(
                f,
                "stale result for node {node_id}: dispatch_id {actual} does not match expected {expected}"
            ),
        }
    }
}

impl Error for ResultIdentityError {}

/// Verifies that a worker result belongs to the expected dispatch attempt.
///
/// The modern path requires exact instance, node, attempt, and dispatch_id
/// matches. Set `allow_legacy_v1_without_dispatch_id` only for documented
/// rolling-upgrade windows where v1 workers may omit `dispatch_id`.
pub fn verify_result_identity(
    result: &ResultMessage,
    expected_instance_id: &InstanceId,
    expected_node_id: &str,
    expected_attempt: i32,
    allow_legacy_v1_without_dispatch_id: bool,
) -> Result<ResultIdentityMode, ResultIdentityError> {
    if &result.instance_id != expected_instance_id {
        return Err(ResultIdentityError::InstanceMismatch {
            expected: expected_instance_id.clone(),
            actual: result.instance_id.clone(),
        });
    }

    if result.node_id != expected_node_id {
        return Err(ResultIdentityError::NodeMismatch {
            expected: expected_node_id.to_owned(),
            actual: result.node_id.clone(),
        });
    }

    if result.dispatch_id.is_none()
        && allow_legacy_v1_without_dispatch_id
        && result.v <= WIRE_VERSION
        && (result.attempt == 0 || result.attempt == expected_attempt)
    {
        return Ok(ResultIdentityMode::LegacyV1);
    }

    if result.attempt != expected_attempt {
        return Err(ResultIdentityError::AttemptMismatch {
            node_id: result.node_id.clone(),
            expected: expected_attempt,
            actual: result.attempt,
        });
    }

    let expected_dispatch_id =
        dispatch_identity(expected_instance_id, expected_node_id, expected_attempt);
    match result.dispatch_id.as_deref() {
        Some(dispatch_id) if dispatch_id == expected_dispatch_id => Ok(ResultIdentityMode::Modern),
        Some(dispatch_id) => Err(ResultIdentityError::DispatchIdMismatch {
            node_id: result.node_id.clone(),
            expected: expected_dispatch_id,
            actual: dispatch_id.to_owned(),
        }),
        None => Err(ResultIdentityError::MissingDispatchId {
            node_id: result.node_id.clone(),
        }),
    }
}

fn default_wire_version() -> u8 {
    WIRE_VERSION
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_message_serde_roundtrip() {
        let msg = TaskMessage {
            instance_id: InstanceId::new("inst-1"),
            node_id: "node-1".into(),
            dispatch_id: Some(dispatch_identity(&InstanceId::new("inst-1"), "node-1", 1)),
            plugin_ref: "builtin:http".into(),
            config: Some(HashMap::from([(
                "url".into(),
                serde_json::json!("https://example.com"),
            )])),
            input: None,
            parameters: None,
            capabilities: None,
            attempt: 1,
            trace_context: None,
            v: 1,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let msg2: TaskMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg.instance_id, msg2.instance_id);
        assert_eq!(msg.node_id, msg2.node_id);
        assert_eq!(msg2.v, 1);
    }

    #[test]
    fn test_result_message_serde_roundtrip() {
        let msg = ResultMessage {
            result_id: Some("r-1".into()),
            instance_id: InstanceId::new("inst-1"),
            node_id: "node-1".into(),
            attempt: 1,
            dispatch_id: Some(dispatch_identity(&InstanceId::new("inst-1"), "node-1", 1)),
            output: Some(HashMap::from([("status".into(), serde_json::json!(200))])),
            error: None,
            trace_context: None,
            v: 1,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let msg2: ResultMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg.result_id, msg2.result_id);
        assert_eq!(msg2.v, 1);
    }

    #[test]
    fn test_task_message_trace_context_roundtrip() {
        let mut ctx = HashMap::new();
        ctx.insert("traceparent".into(), "00-abc123-def456-01".into());
        ctx.insert("tracestate".into(), "orbflow=t:1".into());

        let msg = TaskMessage {
            instance_id: InstanceId::new("inst-1"),
            node_id: "node-1".into(),
            dispatch_id: Some(dispatch_identity(&InstanceId::new("inst-1"), "node-1", 1)),
            plugin_ref: "builtin:http".into(),
            config: None,
            input: None,
            parameters: None,
            capabilities: None,
            attempt: 1,
            trace_context: Some(ctx),
            v: 1,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let msg2: TaskMessage = serde_json::from_str(&json).unwrap();
        let tc = msg2.trace_context.unwrap();
        assert_eq!(tc.get("traceparent").unwrap(), "00-abc123-def456-01");
        assert_eq!(tc.get("tracestate").unwrap(), "orbflow=t:1");
    }

    #[test]
    fn test_wire_backward_compat_no_trace_context() {
        // Old messages without trace_context should deserialize fine.
        let json =
            r#"{"instance_id":"inst-1","node_id":"n1","plugin_ref":"builtin:http","attempt":1}"#;
        let msg: TaskMessage = serde_json::from_str(json).unwrap();
        assert!(msg.trace_context.is_none());
        assert_eq!(msg.v, 1);
    }

    #[test]
    fn test_wire_backward_compat_no_version() {
        // Old messages without `v` should deserialize with v=1.
        let json = r#"{"result_id":"r-1","instance_id":"inst-1","node_id":"n1"}"#;
        let msg: ResultMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.v, 1);
    }

    #[test]
    fn result_identity_accepts_modern_dispatch() {
        let result = ResultMessage {
            result_id: Some("r-1".into()),
            instance_id: InstanceId::new("inst-1"),
            node_id: "node-1".into(),
            attempt: 2,
            dispatch_id: Some(dispatch_identity(&InstanceId::new("inst-1"), "node-1", 2)),
            output: None,
            error: None,
            trace_context: None,
            v: WIRE_VERSION,
        };

        assert_eq!(
            verify_result_identity(&result, &InstanceId::new("inst-1"), "node-1", 2, false)
                .unwrap(),
            ResultIdentityMode::Modern
        );
    }

    #[test]
    fn result_identity_rejects_stale_attempt() {
        let result = ResultMessage {
            result_id: Some("r-1".into()),
            instance_id: InstanceId::new("inst-1"),
            node_id: "node-1".into(),
            attempt: 1,
            dispatch_id: Some(dispatch_identity(&InstanceId::new("inst-1"), "node-1", 1)),
            output: None,
            error: None,
            trace_context: None,
            v: WIRE_VERSION,
        };

        let err = verify_result_identity(&result, &InstanceId::new("inst-1"), "node-1", 2, false)
            .unwrap_err();
        assert!(matches!(err, ResultIdentityError::AttemptMismatch { .. }));
    }

    #[test]
    fn result_identity_legacy_acceptance_is_explicit() {
        let result = ResultMessage {
            result_id: Some("r-1".into()),
            instance_id: InstanceId::new("inst-1"),
            node_id: "node-1".into(),
            attempt: 0,
            dispatch_id: None,
            output: None,
            error: None,
            trace_context: None,
            v: 1,
        };

        assert_eq!(
            verify_result_identity(&result, &InstanceId::new("inst-1"), "node-1", 2, true).unwrap(),
            ResultIdentityMode::LegacyV1
        );
        assert!(
            verify_result_identity(&result, &InstanceId::new("inst-1"), "node-1", 2, false)
                .is_err()
        );
    }
}
