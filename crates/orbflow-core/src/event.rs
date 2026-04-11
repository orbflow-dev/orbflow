// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Domain events for the event sourcing model.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::execution::InstanceId;

/// Categorizes domain events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventType {
    #[serde(rename = "instance.started")]
    InstanceStarted,
    #[serde(rename = "node.queued")]
    NodeQueued,
    #[serde(rename = "node.started")]
    NodeStarted,
    #[serde(rename = "node.completed")]
    NodeCompleted,
    #[serde(rename = "node.failed")]
    NodeFailed,
    #[serde(rename = "instance.completed")]
    InstanceCompleted,
    #[serde(rename = "instance.failed")]
    InstanceFailed,
    #[serde(rename = "instance.cancelled")]
    InstanceCancelled,
    #[serde(rename = "node.approval_requested")]
    NodeApprovalRequested,
    #[serde(rename = "node.approved")]
    NodeApproved,
    #[serde(rename = "node.rejected")]
    NodeRejected,
    #[serde(rename = "compensation.started")]
    CompensationStarted,
    #[serde(rename = "compensation.completed")]
    CompensationCompleted,
    #[serde(rename = "anomaly.detected")]
    AnomalyDetected,
    #[serde(rename = "change_request.state_changed")]
    ChangeRequestStateChanged,
    #[serde(rename = "policy.changed")]
    PolicyChanged,
    #[serde(rename = "version.created")]
    VersionCreated,
}

/// A domain event in the event sourcing model.
///
/// Uses an enum-dispatch pattern for easy serialization and pattern matching.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DomainEvent {
    #[serde(rename = "instance.started")]
    InstanceStarted(InstanceStartedEvent),

    #[serde(rename = "node.queued")]
    NodeQueued(NodeQueuedEvent),

    #[serde(rename = "node.started")]
    NodeStarted(NodeStartedEvent),

    #[serde(rename = "node.completed")]
    NodeCompleted(NodeCompletedEvent),

    #[serde(rename = "node.failed")]
    NodeFailed(NodeFailedEvent),

    #[serde(rename = "instance.completed")]
    InstanceCompleted(InstanceCompletedEvent),

    #[serde(rename = "instance.failed")]
    InstanceFailed(InstanceFailedEvent),

    #[serde(rename = "instance.cancelled")]
    InstanceCancelled(InstanceCancelledEvent),

    #[serde(rename = "node.approval_requested")]
    NodeApprovalRequested(NodeApprovalRequestedEvent),

    #[serde(rename = "node.approved")]
    NodeApproved(NodeApprovedEvent),

    #[serde(rename = "node.rejected")]
    NodeRejected(NodeRejectedEvent),

    #[serde(rename = "compensation.started")]
    CompensationStarted(CompensationStartedEvent),

    #[serde(rename = "compensation.completed")]
    CompensationCompleted(CompensationCompletedEvent),

    #[serde(rename = "anomaly.detected")]
    AnomalyDetected(AnomalyDetectedEvent),

    /// A change request status transition occurred.
    #[serde(rename = "change_request.state_changed")]
    ChangeRequestStateChanged(ChangeRequestStateChangedEvent),

    /// A RBAC policy was changed (role or binding created/updated/deleted).
    #[serde(rename = "policy.changed")]
    PolicyChanged(PolicyChangedEvent),

    /// A new workflow version snapshot was created.
    #[serde(rename = "version.created")]
    VersionCreated(VersionCreatedEvent),
}

impl DomainEvent {
    pub fn event_type(&self) -> EventType {
        match self {
            Self::InstanceStarted(_) => EventType::InstanceStarted,
            Self::NodeQueued(_) => EventType::NodeQueued,
            Self::NodeStarted(_) => EventType::NodeStarted,
            Self::NodeCompleted(_) => EventType::NodeCompleted,
            Self::NodeFailed(_) => EventType::NodeFailed,
            Self::NodeApprovalRequested(_) => EventType::NodeApprovalRequested,
            Self::NodeApproved(_) => EventType::NodeApproved,
            Self::NodeRejected(_) => EventType::NodeRejected,
            Self::InstanceCompleted(_) => EventType::InstanceCompleted,
            Self::InstanceFailed(_) => EventType::InstanceFailed,
            Self::InstanceCancelled(_) => EventType::InstanceCancelled,
            Self::CompensationStarted(_) => EventType::CompensationStarted,
            Self::CompensationCompleted(_) => EventType::CompensationCompleted,
            Self::AnomalyDetected(_) => EventType::AnomalyDetected,
            Self::ChangeRequestStateChanged(_) => EventType::ChangeRequestStateChanged,
            Self::PolicyChanged(_) => EventType::PolicyChanged,
            Self::VersionCreated(_) => EventType::VersionCreated,
        }
    }

    pub fn instance_id(&self) -> &InstanceId {
        match self {
            Self::InstanceStarted(e) => &e.base.instance_id,
            Self::NodeQueued(e) => &e.base.instance_id,
            Self::NodeStarted(e) => &e.base.instance_id,
            Self::NodeCompleted(e) => &e.base.instance_id,
            Self::NodeFailed(e) => &e.base.instance_id,
            Self::NodeApprovalRequested(e) => &e.base.instance_id,
            Self::NodeApproved(e) => &e.base.instance_id,
            Self::NodeRejected(e) => &e.base.instance_id,
            Self::InstanceCompleted(e) => &e.base.instance_id,
            Self::InstanceFailed(e) => &e.base.instance_id,
            Self::InstanceCancelled(e) => &e.base.instance_id,
            Self::CompensationStarted(e) => &e.base.instance_id,
            Self::CompensationCompleted(e) => &e.base.instance_id,
            Self::AnomalyDetected(e) => &e.base.instance_id,
            Self::ChangeRequestStateChanged(e) => &e.base.instance_id,
            Self::PolicyChanged(e) => &e.base.instance_id,
            Self::VersionCreated(e) => &e.base.instance_id,
        }
    }

    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Self::InstanceStarted(e) => e.base.timestamp,
            Self::NodeQueued(e) => e.base.timestamp,
            Self::NodeStarted(e) => e.base.timestamp,
            Self::NodeCompleted(e) => e.base.timestamp,
            Self::NodeFailed(e) => e.base.timestamp,
            Self::NodeApprovalRequested(e) => e.base.timestamp,
            Self::NodeApproved(e) => e.base.timestamp,
            Self::NodeRejected(e) => e.base.timestamp,
            Self::InstanceCompleted(e) => e.base.timestamp,
            Self::InstanceFailed(e) => e.base.timestamp,
            Self::InstanceCancelled(e) => e.base.timestamp,
            Self::CompensationStarted(e) => e.base.timestamp,
            Self::CompensationCompleted(e) => e.base.timestamp,
            Self::AnomalyDetected(e) => e.base.timestamp,
            Self::ChangeRequestStateChanged(e) => e.base.timestamp,
            Self::PolicyChanged(e) => e.base.timestamp,
            Self::VersionCreated(e) => e.base.timestamp,
        }
    }

    pub fn version(&self) -> i64 {
        match self {
            Self::InstanceStarted(e) => e.base.version,
            Self::NodeQueued(e) => e.base.version,
            Self::NodeStarted(e) => e.base.version,
            Self::NodeCompleted(e) => e.base.version,
            Self::NodeFailed(e) => e.base.version,
            Self::NodeApprovalRequested(e) => e.base.version,
            Self::NodeApproved(e) => e.base.version,
            Self::NodeRejected(e) => e.base.version,
            Self::InstanceCompleted(e) => e.base.version,
            Self::InstanceFailed(e) => e.base.version,
            Self::InstanceCancelled(e) => e.base.version,
            Self::CompensationStarted(e) => e.base.version,
            Self::CompensationCompleted(e) => e.base.version,
            Self::AnomalyDetected(e) => e.base.version,
            Self::ChangeRequestStateChanged(e) => e.base.version,
            Self::PolicyChanged(e) => e.base.version,
            Self::VersionCreated(e) => e.base.version,
        }
    }
}

/// Common fields for all domain events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseEvent {
    pub instance_id: InstanceId,
    pub version: i64,
    pub timestamp: DateTime<Utc>,
}

impl BaseEvent {
    /// Creates a new base event. Version must be positive (>= 1).
    pub fn new(instance_id: InstanceId, version: i64) -> Self {
        debug_assert!(version >= 1, "event version must be >= 1, got {version}");
        Self {
            instance_id,
            version,
            timestamp: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceStartedEvent {
    #[serde(flatten)]
    pub base: BaseEvent,
    pub input: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeQueuedEvent {
    #[serde(flatten)]
    pub base: BaseEvent,
    pub node_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeStartedEvent {
    #[serde(flatten)]
    pub base: BaseEvent,
    pub node_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeCompletedEvent {
    #[serde(flatten)]
    pub base: BaseEvent,
    pub node_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeFailedEvent {
    #[serde(flatten)]
    pub base: BaseEvent,
    pub node_id: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceCompletedEvent {
    #[serde(flatten)]
    pub base: BaseEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceFailedEvent {
    #[serde(flatten)]
    pub base: BaseEvent,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceCancelledEvent {
    #[serde(flatten)]
    pub base: BaseEvent,
}

/// A node requires human approval before continuing execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeApprovalRequestedEvent {
    #[serde(flatten)]
    pub base: BaseEvent,
    pub node_id: String,
    /// Optional message explaining what needs approval.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// A node was approved and can proceed with execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeApprovedEvent {
    #[serde(flatten)]
    pub base: BaseEvent,
    pub node_id: String,
    /// Who approved (user ID, email, etc.).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approved_by: Option<String>,
}

/// A node was rejected and will be marked as failed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeRejectedEvent {
    #[serde(flatten)]
    pub base: BaseEvent,
    pub node_id: String,
    /// Why the node was rejected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Who rejected (user ID, email, etc.).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rejected_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompensationStartedEvent {
    #[serde(flatten)]
    pub base: BaseEvent,
    pub failed_node: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompensationCompletedEvent {
    #[serde(flatten)]
    pub base: BaseEvent,
}

/// An anomaly was detected in workflow execution metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyDetectedEvent {
    #[serde(flatten)]
    pub base: BaseEvent,
    /// Type of anomaly (e.g., "latency", "failure_rate", "duration_violation").
    pub anomaly_type: String,
    /// Human-readable description of the anomaly.
    pub message: String,
    /// Severity level (e.g., "warning", "high", "critical").
    pub severity: String,
}

/// A change request status transition occurred.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeRequestStateChangedEvent {
    #[serde(flatten)]
    pub base: BaseEvent,
    pub change_request_id: String,
    pub workflow_id: String,
    pub old_status: String,
    pub new_status: String,
    pub actor: String,
}

/// A RBAC policy was changed (role or binding created/updated/deleted).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyChangedEvent {
    #[serde(flatten)]
    pub base: BaseEvent,
    pub user_id: String,
    /// Action performed: "role_created", "role_deleted", "binding_added", "binding_removed".
    pub action: String,
    /// Resource type: "role" or "binding".
    pub resource_type: String,
    /// Human-readable description of the change.
    pub detail: String,
}

/// A new workflow version snapshot was created.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionCreatedEvent {
    #[serde(flatten)]
    pub base: BaseEvent,
    pub workflow_id: String,
    pub workflow_version: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_domain_event_serde_roundtrip() {
        let event = DomainEvent::InstanceStarted(InstanceStartedEvent {
            base: BaseEvent::new(InstanceId::new("inst-1"), 1),
            input: HashMap::from([("key".into(), serde_json::json!("value"))]),
        });
        let json = serde_json::to_string(&event).unwrap();
        let event2: DomainEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event.event_type(), event2.event_type());
        assert_eq!(event.instance_id(), event2.instance_id());
    }

    #[test]
    fn test_event_type_accessor() {
        let base = BaseEvent::new(InstanceId::new("i-1"), 1);
        let events = vec![
            DomainEvent::InstanceStarted(InstanceStartedEvent {
                base: base.clone(),
                input: HashMap::new(),
            }),
            DomainEvent::NodeQueued(NodeQueuedEvent {
                base: base.clone(),
                node_id: "n".into(),
            }),
            DomainEvent::NodeCompleted(NodeCompletedEvent {
                base: base.clone(),
                node_id: "n".into(),
                output: None,
            }),
        ];
        assert_eq!(events[0].event_type(), EventType::InstanceStarted);
        assert_eq!(events[1].event_type(), EventType::NodeQueued);
        assert_eq!(events[2].event_type(), EventType::NodeCompleted);
    }

    #[test]
    fn test_policy_changed_serde_roundtrip() {
        let event = DomainEvent::PolicyChanged(PolicyChangedEvent {
            base: BaseEvent::new(InstanceId::new("system"), 1),
            user_id: "user-42".into(),
            action: "role_created".into(),
            resource_type: "role".into(),
            detail: "Created admin role".into(),
        });
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: DomainEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.event_type(), EventType::PolicyChanged);
        assert!(json.contains("policy.changed"));
    }

    #[test]
    fn test_version_created_serde_roundtrip() {
        let event = DomainEvent::VersionCreated(VersionCreatedEvent {
            base: BaseEvent::new(InstanceId::new("system"), 1),
            workflow_id: "wf-1".into(),
            workflow_version: 3,
            author: Some("alice".into()),
            message: Some("Updated retry logic".into()),
        });
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: DomainEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.event_type(), EventType::VersionCreated);
        assert!(json.contains("version.created"));
    }

    #[test]
    fn test_version_created_optional_fields_absent() {
        let event = DomainEvent::VersionCreated(VersionCreatedEvent {
            base: BaseEvent::new(InstanceId::new("system"), 1),
            workflow_id: "wf-1".into(),
            workflow_version: 1,
            author: None,
            message: None,
        });
        let json = serde_json::to_string(&event).unwrap();
        assert!(!json.contains("author"));
        assert!(!json.contains("message"));
        let deserialized: DomainEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.event_type(), EventType::VersionCreated);
    }

    #[test]
    fn test_change_request_state_changed_serde_roundtrip() {
        let event = DomainEvent::ChangeRequestStateChanged(ChangeRequestStateChangedEvent {
            base: BaseEvent::new(InstanceId::new("system"), 1),
            change_request_id: "cr-99".into(),
            workflow_id: "wf-1".into(),
            old_status: "draft".into(),
            new_status: "submitted".into(),
            actor: "bob".into(),
        });
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: DomainEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(
            deserialized.event_type(),
            EventType::ChangeRequestStateChanged
        );
        assert!(json.contains("change_request.state_changed"));
    }

    /// Compile-time exhaustiveness check: if a new `DomainEvent` variant is
    /// added without a corresponding `EventType`, this function will fail to
    /// compile due to a non-exhaustive match. This keeps the two enums in sync.
    #[test]
    fn event_type_covers_all_domain_events() {
        fn check(e: &DomainEvent) -> EventType {
            match e {
                DomainEvent::InstanceStarted(_) => EventType::InstanceStarted,
                DomainEvent::NodeQueued(_) => EventType::NodeQueued,
                DomainEvent::NodeStarted(_) => EventType::NodeStarted,
                DomainEvent::NodeCompleted(_) => EventType::NodeCompleted,
                DomainEvent::NodeFailed(_) => EventType::NodeFailed,
                DomainEvent::InstanceCompleted(_) => EventType::InstanceCompleted,
                DomainEvent::InstanceFailed(_) => EventType::InstanceFailed,
                DomainEvent::InstanceCancelled(_) => EventType::InstanceCancelled,
                DomainEvent::NodeApprovalRequested(_) => EventType::NodeApprovalRequested,
                DomainEvent::NodeApproved(_) => EventType::NodeApproved,
                DomainEvent::NodeRejected(_) => EventType::NodeRejected,
                DomainEvent::CompensationStarted(_) => EventType::CompensationStarted,
                DomainEvent::CompensationCompleted(_) => EventType::CompensationCompleted,
                DomainEvent::AnomalyDetected(_) => EventType::AnomalyDetected,
                DomainEvent::ChangeRequestStateChanged(_) => EventType::ChangeRequestStateChanged,
                DomainEvent::PolicyChanged(_) => EventType::PolicyChanged,
                DomainEvent::VersionCreated(_) => EventType::VersionCreated,
            }
        }
        // Exercise the function to prove it compiles and covers all arms.
        for evt in all_events() {
            let _ = check(&evt);
        }
    }

    // ── Helper ──────────────────────────────────────────────────────

    fn base(id: &str, version: i64) -> BaseEvent {
        BaseEvent::new(InstanceId::new(id), version)
    }

    /// Build every `DomainEvent` variant so we can exercise all match arms.
    fn all_events() -> Vec<DomainEvent> {
        let b = || base("inst-all", 7);
        vec![
            DomainEvent::InstanceStarted(InstanceStartedEvent {
                base: b(),
                input: HashMap::new(),
            }),
            DomainEvent::NodeQueued(NodeQueuedEvent {
                base: b(),
                node_id: "n1".into(),
            }),
            DomainEvent::NodeStarted(NodeStartedEvent {
                base: b(),
                node_id: "n2".into(),
            }),
            DomainEvent::NodeCompleted(NodeCompletedEvent {
                base: b(),
                node_id: "n3".into(),
                output: Some(HashMap::from([("r".into(), serde_json::json!(42))])),
            }),
            DomainEvent::NodeFailed(NodeFailedEvent {
                base: b(),
                node_id: "n4".into(),
                error: "boom".into(),
            }),
            DomainEvent::InstanceCompleted(InstanceCompletedEvent { base: b() }),
            DomainEvent::InstanceFailed(InstanceFailedEvent {
                base: b(),
                error: "fatal".into(),
            }),
            DomainEvent::InstanceCancelled(InstanceCancelledEvent { base: b() }),
            DomainEvent::NodeApprovalRequested(NodeApprovalRequestedEvent {
                base: b(),
                node_id: "n5".into(),
                message: Some("please approve".into()),
            }),
            DomainEvent::NodeApproved(NodeApprovedEvent {
                base: b(),
                node_id: "n5".into(),
                approved_by: Some("alice".into()),
            }),
            DomainEvent::NodeRejected(NodeRejectedEvent {
                base: b(),
                node_id: "n5".into(),
                reason: Some("bad idea".into()),
                rejected_by: Some("bob".into()),
            }),
            DomainEvent::CompensationStarted(CompensationStartedEvent {
                base: b(),
                failed_node: "n4".into(),
            }),
            DomainEvent::CompensationCompleted(CompensationCompletedEvent { base: b() }),
            DomainEvent::AnomalyDetected(AnomalyDetectedEvent {
                base: b(),
                anomaly_type: "latency".into(),
                message: "p99 spike".into(),
                severity: "high".into(),
            }),
            DomainEvent::ChangeRequestStateChanged(ChangeRequestStateChangedEvent {
                base: b(),
                change_request_id: "cr-1".into(),
                workflow_id: "wf-1".into(),
                old_status: "draft".into(),
                new_status: "submitted".into(),
                actor: "eve".into(),
            }),
            DomainEvent::PolicyChanged(PolicyChangedEvent {
                base: b(),
                user_id: "u-1".into(),
                action: "role_created".into(),
                resource_type: "role".into(),
                detail: "created admin".into(),
            }),
            DomainEvent::VersionCreated(VersionCreatedEvent {
                base: b(),
                workflow_id: "wf-2".into(),
                workflow_version: 5,
                author: Some("carol".into()),
                message: None,
            }),
        ]
    }

    // ── event_type() coverage for every variant ─────────────────────

    #[test]
    fn event_type_returns_correct_variant_for_all_events() {
        let expected = vec![
            EventType::InstanceStarted,
            EventType::NodeQueued,
            EventType::NodeStarted,
            EventType::NodeCompleted,
            EventType::NodeFailed,
            EventType::InstanceCompleted,
            EventType::InstanceFailed,
            EventType::InstanceCancelled,
            EventType::NodeApprovalRequested,
            EventType::NodeApproved,
            EventType::NodeRejected,
            EventType::CompensationStarted,
            EventType::CompensationCompleted,
            EventType::AnomalyDetected,
            EventType::ChangeRequestStateChanged,
            EventType::PolicyChanged,
            EventType::VersionCreated,
        ];
        let events = all_events();
        assert_eq!(events.len(), expected.len());
        for (evt, exp) in events.iter().zip(expected.iter()) {
            assert_eq!(&evt.event_type(), exp);
        }
    }

    // ── instance_id() coverage for every variant ────────────────────

    #[test]
    fn instance_id_returns_base_id_for_all_events() {
        for evt in all_events() {
            assert_eq!(evt.instance_id(), &InstanceId::new("inst-all"));
        }
    }

    // ── timestamp() coverage for every variant ──────────────────────

    #[test]
    fn timestamp_returns_base_timestamp_for_all_events() {
        let events = all_events();
        for evt in &events {
            // All events created with Utc::now(); just ensure accessor doesn't panic
            let _ts = evt.timestamp();
        }
        // Timestamps should be very close (same test run)
        let first = events[0].timestamp();
        let last = events[events.len() - 1].timestamp();
        assert!((last - first).num_seconds() < 2);
    }

    // ── version() coverage for every variant ────────────────────────

    #[test]
    fn version_returns_base_version_for_all_events() {
        for evt in all_events() {
            assert_eq!(evt.version(), 7);
        }
    }

    // ── Serde roundtrips for previously-uncovered variants ──────────

    #[test]
    fn serde_roundtrip_all_variants() {
        for evt in all_events() {
            let json = serde_json::to_string(&evt).unwrap();
            let restored: DomainEvent = serde_json::from_str(&json).unwrap();
            assert_eq!(evt.event_type(), restored.event_type());
            assert_eq!(evt.instance_id(), restored.instance_id());
            assert_eq!(evt.version(), restored.version());
        }
    }

    // ── EventType serde ─────────────────────────────────────────────

    #[test]
    fn event_type_serde_uses_dotted_names() {
        let cases: Vec<(EventType, &str)> = vec![
            (EventType::InstanceStarted, "\"instance.started\""),
            (EventType::NodeQueued, "\"node.queued\""),
            (EventType::NodeStarted, "\"node.started\""),
            (EventType::NodeCompleted, "\"node.completed\""),
            (EventType::NodeFailed, "\"node.failed\""),
            (EventType::InstanceCompleted, "\"instance.completed\""),
            (EventType::InstanceFailed, "\"instance.failed\""),
            (EventType::InstanceCancelled, "\"instance.cancelled\""),
            (
                EventType::NodeApprovalRequested,
                "\"node.approval_requested\"",
            ),
            (EventType::NodeApproved, "\"node.approved\""),
            (EventType::NodeRejected, "\"node.rejected\""),
            (EventType::CompensationStarted, "\"compensation.started\""),
            (
                EventType::CompensationCompleted,
                "\"compensation.completed\"",
            ),
            (EventType::AnomalyDetected, "\"anomaly.detected\""),
            (
                EventType::ChangeRequestStateChanged,
                "\"change_request.state_changed\"",
            ),
            (EventType::PolicyChanged, "\"policy.changed\""),
            (EventType::VersionCreated, "\"version.created\""),
        ];
        for (et, expected_json) in &cases {
            let json = serde_json::to_string(et).unwrap();
            assert_eq!(&json, expected_json, "serialization mismatch for {et:?}");
            let restored: EventType = serde_json::from_str(&json).unwrap();
            assert_eq!(&restored, et, "deserialization mismatch for {et:?}");
        }
    }

    // ── Optional field handling ─────────────────────────────────────

    #[test]
    fn node_completed_output_none_omitted_in_json() {
        let evt = DomainEvent::NodeCompleted(NodeCompletedEvent {
            base: base("i-1", 1),
            node_id: "n".into(),
            output: None,
        });
        let json = serde_json::to_string(&evt).unwrap();
        assert!(!json.contains("output"));
    }

    #[test]
    fn node_completed_output_some_present_in_json() {
        let evt = DomainEvent::NodeCompleted(NodeCompletedEvent {
            base: base("i-1", 1),
            node_id: "n".into(),
            output: Some(HashMap::from([("x".into(), serde_json::json!(1))])),
        });
        let json = serde_json::to_string(&evt).unwrap();
        assert!(json.contains("output"));
        let restored: DomainEvent = serde_json::from_str(&json).unwrap();
        if let DomainEvent::NodeCompleted(e) = restored {
            assert!(e.output.is_some());
            assert_eq!(e.output.unwrap()["x"], serde_json::json!(1));
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn node_approval_requested_optional_message_absent() {
        let evt = DomainEvent::NodeApprovalRequested(NodeApprovalRequestedEvent {
            base: base("i-1", 1),
            node_id: "n".into(),
            message: None,
        });
        let json = serde_json::to_string(&evt).unwrap();
        assert!(!json.contains("message"));
        let restored: DomainEvent = serde_json::from_str(&json).unwrap();
        if let DomainEvent::NodeApprovalRequested(e) = restored {
            assert!(e.message.is_none());
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn node_approved_optional_approved_by_absent() {
        let evt = DomainEvent::NodeApproved(NodeApprovedEvent {
            base: base("i-1", 1),
            node_id: "n".into(),
            approved_by: None,
        });
        let json = serde_json::to_string(&evt).unwrap();
        assert!(!json.contains("approved_by"));
    }

    #[test]
    fn node_rejected_optional_fields_absent() {
        let evt = DomainEvent::NodeRejected(NodeRejectedEvent {
            base: base("i-1", 1),
            node_id: "n".into(),
            reason: None,
            rejected_by: None,
        });
        let json = serde_json::to_string(&evt).unwrap();
        assert!(!json.contains("reason"));
        assert!(!json.contains("rejected_by"));
    }

    #[test]
    fn node_rejected_optional_fields_present() {
        let evt = DomainEvent::NodeRejected(NodeRejectedEvent {
            base: base("i-1", 1),
            node_id: "n".into(),
            reason: Some("too risky".into()),
            rejected_by: Some("carol".into()),
        });
        let json = serde_json::to_string(&evt).unwrap();
        assert!(json.contains("too risky"));
        assert!(json.contains("carol"));
        let restored: DomainEvent = serde_json::from_str(&json).unwrap();
        if let DomainEvent::NodeRejected(e) = restored {
            assert_eq!(e.reason.as_deref(), Some("too risky"));
            assert_eq!(e.rejected_by.as_deref(), Some("carol"));
        } else {
            panic!("wrong variant");
        }
    }

    // ── BaseEvent constructor ───────────────────────────────────────

    #[test]
    fn base_event_new_sets_fields_correctly() {
        let b = BaseEvent::new(InstanceId::new("test-id"), 42);
        assert_eq!(b.instance_id, InstanceId::new("test-id"));
        assert_eq!(b.version, 42);
        // timestamp should be very recent
        let diff = Utc::now() - b.timestamp;
        assert!(diff.num_seconds() < 2);
    }

    // ── Debug impl smoke test ───────────────────────────────────────

    #[test]
    fn debug_impl_does_not_panic() {
        for evt in all_events() {
            let _s = format!("{evt:?}");
        }
    }

    // ── Clone produces independent copies ───────────────────────────

    #[test]
    fn clone_produces_equal_event() {
        let evt = DomainEvent::NodeFailed(NodeFailedEvent {
            base: base("i-1", 3),
            node_id: "n".into(),
            error: "oops".into(),
        });
        let cloned = evt.clone();
        assert_eq!(evt.event_type(), cloned.event_type());
        assert_eq!(evt.instance_id(), cloned.instance_id());
        assert_eq!(evt.version(), cloned.version());
    }

    // ── Deserialization from raw JSON ───────────────────────────────

    #[test]
    fn deserialize_node_failed_from_raw_json() {
        let json = r#"{
            "type": "node.failed",
            "instance_id": "i-99",
            "version": 5,
            "timestamp": "2026-01-15T10:30:00Z",
            "node_id": "step_3",
            "error": "timeout"
        }"#;
        let evt: DomainEvent = serde_json::from_str(json).unwrap();
        assert_eq!(evt.event_type(), EventType::NodeFailed);
        assert_eq!(evt.instance_id(), &InstanceId::new("i-99"));
        assert_eq!(evt.version(), 5);
        if let DomainEvent::NodeFailed(e) = &evt {
            assert_eq!(e.node_id, "step_3");
            assert_eq!(e.error, "timeout");
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn deserialize_anomaly_detected_from_raw_json() {
        let json = r#"{
            "type": "anomaly.detected",
            "instance_id": "i-50",
            "version": 2,
            "timestamp": "2026-03-01T12:00:00Z",
            "anomaly_type": "failure_rate",
            "message": "spike in errors",
            "severity": "critical"
        }"#;
        let evt: DomainEvent = serde_json::from_str(json).unwrap();
        assert_eq!(evt.event_type(), EventType::AnomalyDetected);
        if let DomainEvent::AnomalyDetected(e) = &evt {
            assert_eq!(e.anomaly_type, "failure_rate");
            assert_eq!(e.severity, "critical");
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn deserialize_compensation_started_from_raw_json() {
        let json = r#"{
            "type": "compensation.started",
            "instance_id": "i-10",
            "version": 3,
            "timestamp": "2026-02-20T08:00:00Z",
            "failed_node": "payment_step"
        }"#;
        let evt: DomainEvent = serde_json::from_str(json).unwrap();
        assert_eq!(evt.event_type(), EventType::CompensationStarted);
        if let DomainEvent::CompensationStarted(e) = &evt {
            assert_eq!(e.failed_node, "payment_step");
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn deserialize_instance_cancelled_from_raw_json() {
        let json = r#"{
            "type": "instance.cancelled",
            "instance_id": "i-77",
            "version": 4,
            "timestamp": "2026-06-01T00:00:00Z"
        }"#;
        let evt: DomainEvent = serde_json::from_str(json).unwrap();
        assert_eq!(evt.event_type(), EventType::InstanceCancelled);
        assert_eq!(evt.instance_id(), &InstanceId::new("i-77"));
    }
}
