// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Trigger manager: coordinates cron, event, and webhook triggers.

use std::collections::HashMap;
use std::sync::Arc;

use axum::Router;
use orbflow_core::workflow::{DefinitionStatus, Node, ParameterMode, Workflow, WorkflowId};
use orbflow_core::{Engine, ListOptions, OrbflowError, Trigger, TriggerType, WorkflowStore};
use tracing::{error, info, warn};

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
        Self::new_with_auth(engine, store, None).await
    }

    /// Creates a new trigger manager with optional server bearer-token auth
    /// for webhook ingress.
    pub async fn new_with_auth(
        engine: Arc<dyn Engine>,
        store: Arc<dyn WorkflowStore>,
        auth_token: Option<String>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let fire_engine = Arc::clone(&engine);
        let fire_store = Arc::clone(&store);
        let fire: TriggerCallback = Arc::new(move |wf_id, trigger_type, payload| {
            let engine = Arc::clone(&fire_engine);
            let store = Arc::clone(&fire_store);
            Box::pin(async move {
                fire_trigger(&*engine, &*store, wf_id, trigger_type, payload).await;
            })
        });

        let cron = CronScheduler::new(Arc::clone(&fire)).await?;
        let event = EventBus::new(Arc::clone(&fire));
        let webhook = WebhookHandler::new_with_auth(fire, auth_token);

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

    /// Replaces all registered triggers for a workflow with the workflow's
    /// current active definition.
    pub async fn refresh_workflow_from_def(&self, wf: &Workflow) {
        self.unregister_workflow(&wf.id).await;
        if wf.status == DefinitionStatus::Active {
            self.register_workflow_triggers(wf).await;
        }
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
    /// Trigger-kind nodes are authoritative. Legacy trigger definitions are
    /// used only when a workflow has no trigger nodes.
    async fn register_workflow_triggers(&self, wf: &Workflow) {
        let trigger_nodes = wf.trigger_nodes();
        if !trigger_nodes.is_empty() {
            // Engine-side legacy migration leaves the deprecated `triggers`
            // array in place after creating trigger nodes. Once trigger nodes
            // exist, they are the source of truth to avoid duplicate schedules,
            // webhook routes, and event subscriptions.
            for node in trigger_nodes {
                self.register_trigger_node(&wf.id, node).await;
            }
            return;
        }

        // Legacy-only workflows have no trigger nodes, so register the
        // deprecated trigger definitions directly.
        for trigger in &wf.triggers {
            self.register_trigger(&wf.id, trigger).await;
        }
    }

    async fn register_trigger_node(&self, wf_id: &WorkflowId, node: &Node) {
        if let Some(ref tc) = node.trigger_config {
            match tc.trigger_type {
                TriggerType::Schedule => {
                    if let Some(ref cron_expr) = tc.cron
                        && !cron_expr.is_empty()
                    {
                        self.cron
                            .add_trigger_node(wf_id, Some(&node.id), cron_expr)
                            .await;
                    }
                }
                TriggerType::Event => {
                    if let Some(ref event_name) = tc.event_name
                        && !event_name.is_empty()
                    {
                        self.event
                            .subscribe_trigger_node(wf_id, event_name, Some(&node.id));
                    }
                }
                TriggerType::Webhook => {
                    let path = tc.path.as_deref().unwrap_or("");
                    let secret = webhook_secret_from_node(node);
                    self.webhook
                        .register_trigger_node_with_secret(wf_id, path, &node.id, secret);
                }
                TriggerType::Manual => {
                    // Manual triggers are started via the API — nothing to register.
                }
            }
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

fn webhook_secret_from_node(node: &Node) -> Option<String> {
    const SECRET_KEYS: &[&str] = &["secret", "webhook_secret", "signature_secret"];

    if let Some(config) = &node.config {
        for key in SECRET_KEYS {
            if let Some(secret) = config.get(*key).and_then(non_empty_string) {
                return Some(secret.to_owned());
            }
        }
    }

    for param in &node.parameters {
        if param.mode != ParameterMode::Static || !SECRET_KEYS.iter().any(|key| *key == param.key) {
            continue;
        }
        if let Some(secret) = param.value.as_ref().and_then(non_empty_string) {
            return Some(secret.to_owned());
        }
    }

    None
}

fn non_empty_string(value: &serde_json::Value) -> Option<&str> {
    value.as_str().map(str::trim).filter(|s| !s.is_empty())
}

fn workflow_uses_credentials(wf: &Workflow) -> bool {
    wf.nodes.iter().any(node_uses_credentials)
}

fn node_uses_credentials(node: &Node) -> bool {
    const CREDENTIAL_ID_KEY: &str = "credential_id";

    if node
        .config
        .as_ref()
        .and_then(|config| config.get(CREDENTIAL_ID_KEY))
        .and_then(non_empty_string)
        .is_some()
    {
        return true;
    }

    node.parameters.iter().any(|param| {
        param.key == CREDENTIAL_ID_KEY
            && (param.value.as_ref().and_then(non_empty_string).is_some()
                || param
                    .expression
                    .as_deref()
                    .map(str::trim)
                    .is_some_and(|expr| !expr.is_empty()))
    })
}

/// Fires a trigger by starting the workflow via the engine.
async fn fire_trigger(
    engine: &dyn Engine,
    store: &dyn WorkflowStore,
    wf_id: WorkflowId,
    trigger_type: TriggerType,
    payload: HashMap<String, serde_json::Value>,
) {
    match store.get_workflow(&wf_id).await {
        Ok(wf) if workflow_uses_credentials(&wf) => {
            warn!(
                workflow = %wf_id,
                trigger = %trigger_type,
                "trigger: workflow uses credentials but trigger execution has no owner context; starting anyway so credential resolution fails visibly"
            );
        }
        Ok(_) => {}
        Err(e) => {
            error!(
                workflow = %wf_id,
                trigger = %trigger_type,
                error = %e,
                "trigger: failed to load workflow before start"
            );
            return;
        }
    }

    let mut input: HashMap<String, serde_json::Value> = HashMap::new();
    for (k, v) in payload {
        input.insert(k, v);
    }
    input.insert(
        "_trigger_type".to_owned(),
        serde_json::Value::String(trigger_type.to_string()),
    );

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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use orbflow_core::workflow::{Edge, NodeKind, NodeType, Position};

    fn test_node(id: &str) -> Node {
        Node {
            id: id.to_owned(),
            name: id.to_owned(),
            kind: NodeKind::Action,
            node_type: NodeType::Builtin,
            plugin_ref: "builtin:http".to_owned(),
            input_mapping: None,
            config: None,
            parameters: vec![],
            retry: None,
            compensate: None,
            position: Position::default(),
            capability_ports: vec![],
            metadata: None,
            trigger_config: None,
            requires_approval: false,
        }
    }

    fn test_workflow(nodes: Vec<Node>) -> Workflow {
        Workflow {
            id: WorkflowId::new("wf-1"),
            name: "wf-1".to_owned(),
            description: None,
            version: 1,
            status: DefinitionStatus::Active,
            nodes,
            edges: Vec::<Edge>::new(),
            capability_edges: vec![],
            triggers: vec![],
            annotations: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn workflow_uses_credentials_detects_config_credential_id() {
        let mut node = test_node("n1");
        node.config = Some(HashMap::from([(
            "credential_id".to_owned(),
            serde_json::json!("cred-1"),
        )]));

        assert!(workflow_uses_credentials(&test_workflow(vec![node])));
    }

    #[test]
    fn workflow_uses_credentials_detects_parameter_credential_id() {
        let mut node = test_node("n1");
        node.parameters = vec![orbflow_core::workflow::Parameter {
            key: "credential_id".to_owned(),
            mode: ParameterMode::Expression,
            value: None,
            expression: Some("secrets.current".to_owned()),
        }];

        assert!(workflow_uses_credentials(&test_workflow(vec![node])));
    }

    #[test]
    fn workflow_uses_credentials_ignores_secret_fields_without_credential_ref() {
        let mut node = test_node("n1");
        node.config = Some(HashMap::from([(
            "webhook_secret".to_owned(),
            serde_json::json!("route-secret"),
        )]));

        assert!(!workflow_uses_credentials(&test_workflow(vec![node])));
    }
}
