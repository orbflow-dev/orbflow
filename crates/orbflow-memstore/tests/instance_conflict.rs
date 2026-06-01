// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;

use chrono::Utc;
use orbflow_core::error::OrbflowError;
use orbflow_core::execution::{ExecutionContext, Instance, InstanceId, InstanceStatus};
use orbflow_core::ports::InstanceStore;
use orbflow_core::workflow::WorkflowId;
use orbflow_memstore::MemStore;

fn test_instance(id: &str, version: i64, status: InstanceStatus) -> Instance {
    Instance {
        id: InstanceId::new(id),
        workflow_id: WorkflowId::new("wf-1"),
        status,
        node_states: HashMap::new(),
        context: ExecutionContext::new(HashMap::new()),
        saga: None,
        parent_id: None,
        instance_metrics: None,
        workflow_version: Some(1),
        owner_id: Some("owner-1".into()),
        version,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

#[tokio::test]
async fn update_instance_rejects_stale_version() {
    let store = MemStore::new();
    let original = test_instance("inst-stale", 1, InstanceStatus::Running);
    store.create_instance(&original).await.unwrap();

    let mut first_writer = store.get_instance(&original.id).await.unwrap();
    first_writer.status = InstanceStatus::Failed;
    first_writer.version += 1;
    first_writer.updated_at = Utc::now();
    store.update_instance(&first_writer).await.unwrap();

    let mut stale_writer = original.clone();
    stale_writer.status = InstanceStatus::Completed;
    stale_writer.version += 1;
    stale_writer.updated_at = Utc::now();

    let err = store.update_instance(&stale_writer).await.unwrap_err();
    assert!(
        matches!(err, OrbflowError::Conflict),
        "expected stale instance update to return Conflict, got {err:?}"
    );

    let stored = store.get_instance(&original.id).await.unwrap();
    assert_eq!(stored.status, InstanceStatus::Failed);
    assert_eq!(stored.version, 2);
}
