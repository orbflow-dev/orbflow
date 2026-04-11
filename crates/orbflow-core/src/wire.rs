// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Wire format types for the message bus — the contract between engine and worker.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::execution::InstanceId;

/// Wire format for tasks published to the bus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskMessage {
    pub instance_id: InstanceId,
    pub node_id: String,
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
}
