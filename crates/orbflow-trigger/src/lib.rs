// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Cron, webhook, and event trigger system.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use orbflow_core::TriggerType;
use orbflow_core::workflow::WorkflowId;

pub mod cron;
pub mod event;
pub mod manager;
pub mod webhook;

pub use cron::CronScheduler;
pub use event::EventBus;
pub use manager::TriggerManager;
pub use webhook::WebhookHandler;

/// Callback invoked when a trigger fires.
///
/// Receives the workflow ID, trigger type, and payload.
/// Returns a future that completes when the trigger has been processed.
pub type TriggerCallback = Arc<
    dyn Fn(
            WorkflowId,
            TriggerType,
            HashMap<String, serde_json::Value>,
        ) -> Pin<Box<dyn Future<Output = ()> + Send>>
        + Send
        + Sync,
>;
