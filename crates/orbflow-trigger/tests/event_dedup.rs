// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use orbflow_core::workflow::WorkflowId;
use orbflow_trigger::{EventBus, TriggerCallback};

fn counting_callback() -> (TriggerCallback, Arc<AtomicUsize>) {
    let count = Arc::new(AtomicUsize::new(0));
    let count_clone = Arc::clone(&count);
    let cb: TriggerCallback = Arc::new(move |_wf, _tt, _payload| {
        let count = Arc::clone(&count_clone);
        Box::pin(async move {
            count.fetch_add(1, Ordering::SeqCst);
        })
    });
    (cb, count)
}

#[tokio::test]
async fn duplicate_event_registration_fires_workflow_once() {
    let (callback, count) = counting_callback();
    let bus = EventBus::new(callback);
    let workflow_id = WorkflowId::new("wf-events");

    bus.subscribe(&workflow_id, "order.created");
    bus.subscribe(&workflow_id, "order.created");

    bus.emit("order.created", HashMap::new()).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    assert_eq!(
        count.load(Ordering::SeqCst),
        1,
        "duplicate registration should not start the same workflow twice"
    );
}
