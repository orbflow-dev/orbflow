// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Mock implementation of [`Bus`] for testing.
//!
//! When [`MockBus::publish`] is called it records the message *and* immediately
//! invokes any registered handler for that subject, giving tests synchronous
//! delivery semantics.

use std::collections::HashMap;

use async_trait::async_trait;
use parking_lot::Mutex;

use orbflow_core::error::OrbflowError;
use orbflow_core::ports::{Bus, MsgHandler};

/// A message captured by [`MockBus::publish`].
#[derive(Debug, Clone)]
pub struct PublishedMessage {
    pub subject: String,
    pub data: Vec<u8>,
}

/// Mock bus that delivers messages synchronously for deterministic testing.
pub struct MockBus {
    inner: Mutex<MockBusInner>,
}

struct MockBusInner {
    handlers: HashMap<String, MsgHandler>,
    messages: Vec<PublishedMessage>,
}

impl MockBus {
    /// Creates a new, empty mock bus.
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(MockBusInner {
                handlers: HashMap::new(),
                messages: Vec::new(),
            }),
        }
    }

    /// Returns a snapshot of all published messages.
    pub fn messages(&self) -> Vec<PublishedMessage> {
        self.inner.lock().messages.clone()
    }

    /// Returns the number of published messages.
    pub fn message_count(&self) -> usize {
        self.inner.lock().messages.len()
    }

    /// Returns published messages filtered by subject.
    pub fn messages_for(&self, subject: &str) -> Vec<PublishedMessage> {
        self.inner
            .lock()
            .messages
            .iter()
            .filter(|m| m.subject == subject)
            .cloned()
            .collect()
    }
}

impl Default for MockBus {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Bus for MockBus {
    /// Records the message and delivers it synchronously to any subscriber.
    async fn publish(&self, subject: &str, data: &[u8]) -> Result<(), OrbflowError> {
        // Record the message and grab the handler (if any) while holding the lock.
        let handler = {
            let mut inner = self.inner.lock();
            inner.messages.push(PublishedMessage {
                subject: subject.to_owned(),
                data: data.to_vec(),
            });
            inner.handlers.get(subject).cloned()
        };

        // Deliver immediately outside the lock for synchronous testing.
        if let Some(handler) = handler {
            let _ = handler(subject.to_owned(), data.to_vec()).await;
        }

        Ok(())
    }

    /// Registers a handler for the given subject.
    async fn subscribe(&self, subject: &str, handler: MsgHandler) -> Result<(), OrbflowError> {
        self.inner
            .lock()
            .handlers
            .insert(subject.to_owned(), handler);
        Ok(())
    }

    /// No-op for the mock bus.
    async fn close(&self) -> Result<(), OrbflowError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    #[tokio::test]
    async fn test_publish_records_message() {
        let bus = MockBus::new();
        bus.publish("tasks.default", b"hello").await.unwrap();

        let msgs = bus.messages();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].subject, "tasks.default");
        assert_eq!(msgs[0].data, b"hello");
    }

    #[tokio::test]
    async fn test_subscribe_and_deliver() {
        let bus = MockBus::new();
        let received = Arc::new(Mutex::new(Vec::<Vec<u8>>::new()));
        let received_clone = received.clone();

        let handler: MsgHandler = Arc::new(move |_subject, data| {
            let rx = received_clone.clone();
            Box::pin(async move {
                rx.lock().push(data);
                Ok(())
            })
        });

        bus.subscribe("tasks.pool1", handler).await.unwrap();
        bus.publish("tasks.pool1", b"payload").await.unwrap();

        let got = received.lock().clone();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0], b"payload");
    }

    #[tokio::test]
    async fn test_publish_without_subscriber_does_not_panic() {
        let bus = MockBus::new();
        // No subscriber registered — should succeed silently.
        bus.publish("no.subscriber", b"data").await.unwrap();
        assert_eq!(bus.message_count(), 1);
    }

    #[tokio::test]
    async fn test_messages_for_filters_by_subject() {
        let bus = MockBus::new();
        bus.publish("a", b"1").await.unwrap();
        bus.publish("b", b"2").await.unwrap();
        bus.publish("a", b"3").await.unwrap();

        let a_msgs = bus.messages_for("a");
        assert_eq!(a_msgs.len(), 2);
        assert_eq!(a_msgs[0].data, b"1");
        assert_eq!(a_msgs[1].data, b"3");

        let b_msgs = bus.messages_for("b");
        assert_eq!(b_msgs.len(), 1);
    }

    #[tokio::test]
    async fn test_close_is_noop() {
        let bus = MockBus::new();
        bus.close().await.unwrap();
    }
}
