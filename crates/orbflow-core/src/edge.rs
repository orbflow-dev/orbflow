// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Edge-to-Cloud workflow continuity types.
//!
//! Defines the protocol for lightweight edge workers that can execute workflows
//! offline and sync results back to the cloud when connectivity is restored.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────┐                    ┌──────────────────┐
//! │  Edge Worker     │  ── sync when ──▶  │  Cloud Server    │
//! │  (SQLite queue)  │     online         │  (Postgres/NATS) │
//! └─────────────────┘                    └──────────────────┘
//! ```

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::execution::InstanceId;
use crate::workflow::WorkflowId;

/// Connection status of an edge worker to the cloud.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectivityStatus {
    /// Connected to the cloud — real-time sync active.
    Online,
    /// Disconnected — queuing results locally.
    Offline,
    /// Attempting to reconnect.
    Reconnecting,
}

/// A queued execution result waiting to be synced to the cloud.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedResult {
    /// Unique ID for deduplication during sync.
    pub id: String,
    /// The instance this result belongs to.
    pub instance_id: InstanceId,
    /// The node that produced this result.
    pub node_id: String,
    /// Output data from the node execution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<HashMap<String, Value>>,
    /// Error message if the node failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// When the execution completed on the edge.
    pub executed_at: DateTime<Utc>,
    /// How many sync attempts have been made.
    #[serde(default)]
    pub sync_attempts: u32,
    /// Current sync status.
    pub sync_status: SyncStatus,
}

/// Sync status for a queued result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncStatus {
    /// Waiting to be synced.
    Pending,
    /// Currently being synced.
    InProgress,
    /// Successfully synced to cloud.
    Synced,
    /// Sync failed (will retry).
    Failed,
    /// Conflict detected — needs manual resolution.
    Conflict,
}

/// Configuration for an edge worker deployment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeWorkerConfig {
    /// Unique identifier for this edge worker.
    pub worker_id: String,
    /// Human-readable name/location (e.g., "warehouse-floor-1").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Cloud server URL to sync with.
    pub cloud_url: String,
    /// Workflows this edge worker is authorized to execute.
    #[serde(default)]
    pub allowed_workflows: Vec<WorkflowId>,
    /// Maximum number of queued results before blocking new executions.
    #[serde(default = "default_max_queue_size")]
    pub max_queue_size: usize,
    /// Sync interval in seconds when online.
    #[serde(default = "default_sync_interval_secs")]
    pub sync_interval_secs: u64,
    /// Maximum sync retry attempts before marking as conflict.
    #[serde(default = "default_max_retries")]
    pub max_sync_retries: u32,
}

fn default_max_queue_size() -> usize {
    10_000
}
fn default_sync_interval_secs() -> u64 {
    30
}
fn default_max_retries() -> u32 {
    5
}

/// A sync batch sent from edge to cloud.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncBatch {
    /// Edge worker identifier.
    pub worker_id: String,
    /// Results to sync.
    pub results: Vec<QueuedResult>,
    /// Timestamp when this batch was created.
    pub created_at: DateTime<Utc>,
}

/// Response from the cloud after processing a sync batch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResponse {
    /// IDs that were successfully synced.
    pub synced: Vec<String>,
    /// IDs that had conflicts (need resolution).
    pub conflicts: Vec<SyncConflict>,
    /// IDs that failed (transient, will retry).
    pub failed: Vec<String>,
}

/// A conflict detected during sync.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConflict {
    /// The queued result ID.
    pub result_id: String,
    /// Explanation of the conflict.
    pub reason: String,
    /// The cloud's version of the data (if any).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloud_data: Option<Value>,
}

/// Status report from an edge worker to the cloud.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeStatusReport {
    pub worker_id: String,
    pub connectivity: ConnectivityStatus,
    pub queue_depth: usize,
    pub pending_sync: usize,
    pub last_sync_at: Option<DateTime<Utc>>,
    pub uptime_secs: u64,
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edge_config_defaults() {
        let config: EdgeWorkerConfig = serde_json::from_str(
            r#"{"worker_id":"edge-1","cloud_url":"https://cloud.example.com"}"#,
        )
        .unwrap();
        assert_eq!(config.max_queue_size, 10_000);
        assert_eq!(config.sync_interval_secs, 30);
        assert_eq!(config.max_sync_retries, 5);
        assert!(config.allowed_workflows.is_empty());
    }

    #[test]
    fn test_queued_result_serde() {
        let result = QueuedResult {
            id: "qr-1".into(),
            instance_id: InstanceId::new("inst-1"),
            node_id: "http-1".into(),
            output: Some(HashMap::from([("status".into(), serde_json::json!(200))])),
            error: None,
            executed_at: Utc::now(),
            sync_attempts: 0,
            sync_status: SyncStatus::Pending,
        };
        let json = serde_json::to_string(&result).unwrap();
        let result2: QueuedResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result2.id, "qr-1");
        assert_eq!(result2.sync_status, SyncStatus::Pending);
    }

    #[test]
    fn test_sync_batch_serde() {
        let batch = SyncBatch {
            worker_id: "edge-1".into(),
            results: vec![QueuedResult {
                id: "qr-1".into(),
                instance_id: InstanceId::new("inst-1"),
                node_id: "n-1".into(),
                output: None,
                error: Some("timeout".into()),
                executed_at: Utc::now(),
                sync_attempts: 1,
                sync_status: SyncStatus::Failed,
            }],
            created_at: Utc::now(),
        };
        let json = serde_json::to_string(&batch).unwrap();
        let batch2: SyncBatch = serde_json::from_str(&json).unwrap();
        assert_eq!(batch2.worker_id, "edge-1");
        assert_eq!(batch2.results.len(), 1);
    }

    #[test]
    fn test_sync_response_serde() {
        let resp = SyncResponse {
            synced: vec!["qr-1".into()],
            conflicts: vec![SyncConflict {
                result_id: "qr-2".into(),
                reason: "instance already completed".into(),
                cloud_data: None,
            }],
            failed: vec!["qr-3".into()],
        };
        let json = serde_json::to_string(&resp).unwrap();
        let resp2: SyncResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(resp2.synced.len(), 1);
        assert_eq!(resp2.conflicts.len(), 1);
        assert_eq!(resp2.failed.len(), 1);
    }

    #[test]
    fn test_edge_status_report() {
        let report = EdgeStatusReport {
            worker_id: "edge-1".into(),
            connectivity: ConnectivityStatus::Online,
            queue_depth: 42,
            pending_sync: 3,
            last_sync_at: Some(Utc::now()),
            uptime_secs: 86400,
        };
        let json = serde_json::to_string(&report).unwrap();
        let report2: EdgeStatusReport = serde_json::from_str(&json).unwrap();
        assert_eq!(report2.connectivity, ConnectivityStatus::Online);
        assert_eq!(report2.queue_depth, 42);
    }

    #[test]
    fn test_connectivity_status_values() {
        let statuses = vec![
            ConnectivityStatus::Online,
            ConnectivityStatus::Offline,
            ConnectivityStatus::Reconnecting,
        ];
        for s in statuses {
            let json = serde_json::to_value(s).unwrap();
            let s2: ConnectivityStatus = serde_json::from_value(json).unwrap();
            assert_eq!(s, s2);
        }
    }
}
