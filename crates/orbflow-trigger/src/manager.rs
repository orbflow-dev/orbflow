// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Trigger manager: coordinates cron, event, and webhook triggers.

use std::collections::HashMap;
use std::sync::Arc;

use axum::Router;
use orbflow_core::workflow::{DefinitionStatus, Workflow, WorkflowId};
use orbflow_core::{
    Engine, ListOptions, OrbflowError, Trigger, TriggerConfig, TriggerType, WorkflowStore,
};
use tracing::{error, info};

use crate::TriggerCallback;
use crate::cron::CronScheduler;
use crate::event::EventBus;
use crate::webhook::WebhookHandler;

/// Coordinates all trigger types and starts workflows when they fire.
pub struct TriggerManager {
    #[allow(dead_code)]
    engine: Arc<dyn Engine>,
    store: Arc<dyn WorkflowStore>,
    cron: CronScheduler,
    event: EventBus,
    webhook: WebhookHandler,
}

impl TriggerManager {
    /// Creates a new trigger manager.
    ///
    /// The `engine` is used to start workflows when triggers fire.
    /// The `store` is used to load workflow definitions on startup.
    pub async fn new(
        engine: Arc<dyn Engine>,
        store: Arc<dyn WorkflowStore>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let fire_engine = Arc::clone(&engine);
        let fire: TriggerCallback = Arc::new(move |wf_id, trigger_type, payload| {
            let engine = Arc::clone(&fire_engine);
            Box::pin(async move {
                fire_trigger(&*engine, wf_id, trigger_type, payload).await;
            })
        });

        let cron = CronScheduler::new(Arc::clone(&fire)).await?;
        let event = EventBus::new(Arc::clone(&fire));
        let webhook = WebhookHandler::new(fire);

        Ok(Self {
            engine,
            store,
            cron,
            event,
            webhook,
        })
    }

    /// Loads all active workflows and registers their triggers.
    ///
    /// Workflows are loaded in pages to avoid a single oversized query.
    pub async fn start(&self) -> Result<(), OrbflowError> {
        const PAGE_SIZE: i64 = 100;
        let mut total_loaded: usize = 0;

        let mut offset: i64 = 0;
        loop {
            let (workflows, _total) = self
                .store
                .list_workflows(ListOptions {
                    offset,
                    limit: PAGE_SIZE,
                })
                .await?;

            for wf in &workflows {
                if wf.status != DefinitionStatus::Active {
                    continue;
                }
                self.register_workflow_triggers(wf).await;
            }

            let count = workflows.len() as i64;
            total_loaded += workflows.len();

            if count < PAGE_SIZE {
                break;
            }
            offset += PAGE_SIZE;
        }

        self.cron
            .start()
            .await
            .map_err(|e| OrbflowError::Internal(format!("cron scheduler start: {e}")))?;

        info!(workflows = total_loaded, "trigger manager started");
        Ok(())
    }

    /// Stops all trigger handlers.
    pub async fn stop(&mut self) {
        if let Err(e) = self.cron.stop().await {
            error!(error = %e, "failed to stop cron scheduler");
        }
        info!("trigger manager stopped");
    }

    /// Registers triggers for a workflow from its trigger-kind nodes.
    pub async fn register_workflow_from_def(&self, wf: &Workflow) {
        self.register_workflow_triggers(wf).await;
    }

    /// Registers explicit trigger definitions for a workflow.
    pub async fn register_workflow(&self, wf_id: &WorkflowId, triggers: &[Trigger]) {
        for t in triggers {
            self.register_trigger(wf_id, t).await;
        }
    }

    /// Removes all triggers for a workflow.
    pub async fn unregister_workflow(&self, wf_id: &WorkflowId) {
        self.cron.remove(wf_id).await;
        self.event.remove(wf_id);
        self.webhook.remove(wf_id);
    }

    /// Returns the Axum router for webhook trigger endpoints.
    pub fn webhook_router(&self) -> Router {
        self.webhook.router()
    }

    /// Emits a named event, triggering any workflows listening for it.
    pub async fn emit_event(&self, event_name: &str, payload: HashMap<String, serde_json::Value>) {
        self.event.emit(event_name, payload).await;
    }

    /// Registers triggers for a workflow by examining its trigger-kind nodes.
    ///
    /// This calls `migrate_legacy_triggers` first to convert any legacy
    /// `Triggers` field entries into trigger-kind nodes.
    async fn register_workflow_triggers(&self, wf: &Workflow) {
        // Note: migrate_legacy_triggers takes &mut, but we work with an
        // immutable reference. For registration purposes, we build trigger
        // definitions from the existing trigger nodes + legacy triggers.

        // Register from trigger-kind nodes.
        for node in wf.trigger_nodes() {
            if let Some(ref tc) = node.trigger_config {
                let trigger = Trigger {
                    trigger_type: tc.trigger_type.clone(),
                    config: TriggerConfig {
                        cron: tc.cron.clone(),
                        event_name: tc.event_name.clone(),
                        path: tc.path.clone(),
                    },
                };
                self.register_trigger(&wf.id, &trigger).await;
            }
        }

        // Also register from the legacy triggers field.
        for trigger in &wf.triggers {
            self.register_trigger(&wf.id, trigger).await;
        }
    }

    /// Registers a single trigger for a workflow.
    async fn register_trigger(&self, wf_id: &WorkflowId, trigger: &Trigger) {
        match trigger.trigger_type {
            TriggerType::Schedule => {
                if let Some(ref cron_expr) = trigger.config.cron
                    && !cron_expr.is_empty()
                {
                    self.cron.add(wf_id, cron_expr).await;
                }
            }
            TriggerType::Event => {
                if let Some(ref event_name) = trigger.config.event_name
                    && !event_name.is_empty()
                {
                    self.event.subscribe(wf_id, event_name);
                }
            }
            TriggerType::Webhook => {
                let path = trigger.config.path.as_deref().unwrap_or("");
                self.webhook.register(wf_id, path);
            }
            TriggerType::Manual => {
                // Manual triggers are started via the API — nothing to register.
            }
        }
    }
}

/// Fires a trigger by starting the workflow via the engine.
async fn fire_trigger(
    engine: &dyn Engine,
    wf_id: WorkflowId,
    trigger_type: TriggerType,
    payload: HashMap<String, serde_json::Value>,
) {
    let mut input: HashMap<String, serde_json::Value> = HashMap::new();
    input.insert(
        "_trigger_type".to_owned(),
        serde_json::Value::String(trigger_type.to_string()),
    );
    for (k, v) in payload {
        input.insert(k, v);
    }

    match engine.start_workflow(&wf_id, input).await {
        Ok(inst) => {
            info!(
                workflow = %wf_id,
                instance = %inst.id,
                trigger = %trigger_type,
                "trigger: workflow started"
            );
        }
        Err(e) => {
            error!(
                workflow = %wf_id,
                trigger = %trigger_type,
                error = %e,
                "trigger: failed to start workflow"
            );
        }
    }
}
