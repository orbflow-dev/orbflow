// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! In-memory pub/sub event bus for named events.

use std::collections::HashMap;
use std::sync::Arc;

use dashmap::DashMap;
use orbflow_core::workflow::WorkflowId;
use tracing::{info, warn};

use crate::TriggerCallback;

/// In-memory pub/sub for named events.
///
/// Workflows subscribe to event names. When an event is emitted,
/// all subscribed workflows are triggered.
pub struct EventBus {
    /// Maps event_name -> list of workflow IDs subscribed to that event.
    subscriptions: DashMap<String, Vec<WorkflowId>>,
    /// Maps workflow_id -> list of event names it is subscribed to.
    workflow_events: DashMap<String, Vec<String>>,
    fire: TriggerCallback,
}

impl EventBus {
    /// Creates a new event bus.
    pub fn new(fire: TriggerCallback) -> Self {
        Self {
            subscriptions: DashMap::new(),
            workflow_events: DashMap::new(),
            fire,
        }
    }

    /// Subscribes a workflow to a named event.
    pub fn subscribe(&self, workflow_id: &WorkflowId, event_name: &str) {
        self.subscriptions
            .entry(event_name.to_owned())
            .or_default()
            .push(workflow_id.clone());

        self.workflow_events
            .entry(workflow_id.to_string())
            .or_default()
            .push(event_name.to_owned());

        info!(
            workflow = %workflow_id,
            event = %event_name,
            "event subscription registered"
        );
    }

    /// Emits a named event, triggering all subscribed workflows.
    pub async fn emit(&self, event_name: &str, payload: HashMap<String, serde_json::Value>) {
        let subscribers = match self.subscriptions.get(event_name) {
            Some(subs) => subs.clone(),
            None => {
                warn!(event = %event_name, "no subscribers for event");
                return;
            }
        };

        info!(
            event = %event_name,
            subscribers = subscribers.len(),
            "emitting event"
        );

        for wf_id in subscribers {
            let fire = Arc::clone(&self.fire);
            let payload = payload.clone();
            let wf_id = wf_id.clone();

            tokio::spawn(async move {
                fire(wf_id, orbflow_core::TriggerType::Event, payload).await;
            });
        }
    }

    /// Removes all event subscriptions for a workflow.
    pub fn remove(&self, workflow_id: &WorkflowId) {
        let wf_key = workflow_id.to_string();

        // Get the event names this workflow is subscribed to.
        if let Some((_, event_names)) = self.workflow_events.remove(&wf_key) {
            for event_name in &event_names {
                if let Some(mut subs) = self.subscriptions.get_mut(event_name) {
                    subs.retain(|id| id.to_string() != wf_key);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn noop_callback() -> TriggerCallback {
        Arc::new(|_wf, _tt, _payload| Box::pin(async {}))
    }

    fn counting_callback() -> (TriggerCallback, Arc<AtomicUsize>) {
        let count = Arc::new(AtomicUsize::new(0));
        let count_clone = Arc::clone(&count);
        let cb: TriggerCallback = Arc::new(move |_wf, _tt, _payload| {
            count_clone.fetch_add(1, Ordering::SeqCst);
            Box::pin(async {})
        });
        (cb, count)
    }

    fn wf_id(s: &str) -> WorkflowId {
        WorkflowId(s.to_string())
    }

    #[test]
    fn subscribe_registers_in_both_maps() {
        let bus = EventBus::new(noop_callback());
        bus.subscribe(&wf_id("w1"), "order.created");

        assert!(bus.subscriptions.contains_key("order.created"));
        assert!(bus.workflow_events.contains_key("w1"));
    }

    #[test]
    fn subscribe_multiple_workflows_to_same_event() {
        let bus = EventBus::new(noop_callback());
        bus.subscribe(&wf_id("w1"), "order.created");
        bus.subscribe(&wf_id("w2"), "order.created");

        let subs = bus.subscriptions.get("order.created").unwrap();
        assert_eq!(subs.len(), 2);
    }

    #[test]
    fn subscribe_workflow_to_multiple_events() {
        let bus = EventBus::new(noop_callback());
        bus.subscribe(&wf_id("w1"), "order.created");
        bus.subscribe(&wf_id("w1"), "order.updated");

        let events = bus.workflow_events.get("w1").unwrap();
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn remove_cleans_up_subscriptions() {
        let bus = EventBus::new(noop_callback());
        bus.subscribe(&wf_id("w1"), "order.created");
        bus.subscribe(&wf_id("w1"), "order.updated");
        bus.subscribe(&wf_id("w2"), "order.created");

        bus.remove(&wf_id("w1"));

        // w1 removed from workflow_events
        assert!(!bus.workflow_events.contains_key("w1"));

        // w1 removed from subscription lists, w2 remains
        let subs = bus.subscriptions.get("order.created").unwrap();
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].to_string(), "w2");
    }

    #[test]
    fn remove_nonexistent_workflow_is_noop() {
        let bus = EventBus::new(noop_callback());
        bus.subscribe(&wf_id("w1"), "evt");
        bus.remove(&wf_id("w999"));

        // w1 still present
        assert!(bus.subscriptions.contains_key("evt"));
    }

    #[tokio::test]
    async fn emit_fires_callback_for_each_subscriber() {
        let (cb, count) = counting_callback();
        let bus = EventBus::new(cb);
        bus.subscribe(&wf_id("w1"), "evt");
        bus.subscribe(&wf_id("w2"), "evt");

        bus.emit("evt", HashMap::new()).await;

        // Allow spawned tasks to complete
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert_eq!(count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn emit_unknown_event_does_not_fire() {
        let (cb, count) = counting_callback();
        let bus = EventBus::new(cb);
        bus.subscribe(&wf_id("w1"), "evt");

        bus.emit("unknown", HashMap::new()).await;

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }
}
