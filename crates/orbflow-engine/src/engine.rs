// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Core engine implementation: DAG coordinator, node dispatching, result
//! handling, capability resolution, and input mapping.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock, Weak};

use async_trait::async_trait;
use chrono::Utc;
use dashmap::DashMap;
use tokio::sync::{Mutex, Notify};
use tracing::{Instrument, error, info, warn};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use uuid::Uuid;

use orbflow_cel::CelEvaluator;
use orbflow_cel::evaluator::build_mapping_context;
use orbflow_core::event::*;
use orbflow_core::execution::*;
use orbflow_core::metering;
use orbflow_core::metrics::{InstanceExecutionMetrics, NodeExecutionMetrics};
use orbflow_core::otel::MetricsRecorder;
use orbflow_core::ports::MetricsStore;
use orbflow_core::ports::*;
use orbflow_core::telemetry::*;
use orbflow_core::wire::*;
use orbflow_core::workflow::*;
use orbflow_core::credential_proxy::CredentialAccessTier;
use orbflow_core::{OrbflowError, result_subject, task_subject, validate_workflow};

/// Key used to reference a credential in node config/parameters maps.
const CREDENTIAL_ID_KEY: &str = "credential_id";

use crate::dag::find_ready_nodes;
use crate::sla::{SlaCheckResult, SlaMonitor};

/// Maximum number of retry attempts when a version conflict occurs during save.
const SAVE_INSTANCE_MAX_RETRIES: usize = 3;

/// Maps a Z-score magnitude to a severity string for anomaly events.
fn severity_from_z(z: f64) -> &'static str {
    let abs_z = z.abs();
    if abs_z > 4.0 {
        "critical"
    } else if abs_z > 3.0 {
        "high"
    } else {
        "warning"
    }
}

/// The single-process implementation of [`orbflow_core::ports::Engine`] that
/// coordinates workflow execution, node dispatching, and saga compensation.
pub struct OrbflowEngine {
    store: Arc<dyn Store>,
    bus: Arc<dyn Bus>,
    cred_store: Option<Arc<dyn CredentialStore>>,
    pool_name: String,
    snapshot_interval: i64,
    cel: CelEvaluator,
    nodes: RwLock<HashMap<String, Arc<dyn NodeExecutor>>>,
    schemas: RwLock<HashMap<String, NodeSchema>>,
    /// Per-instance mutex to serialize result processing.
    instance_mu: DashMap<InstanceId, Arc<Mutex<()>>>,
    /// Per-instance bounded dedup tracker for result idempotency.
    processed_results: DashMap<InstanceId, Arc<crate::dedup::ResultSet>>,
    /// Used to signal the engine to stop.
    stop_notify: Notify,
    /// OpenTelemetry metrics recorder.
    metrics: MetricsRecorder,
    /// Persistent metrics store for analytics queries.
    metrics_store: Option<Arc<dyn MetricsStore>>,
    /// SLA/SLO monitor for anomaly detection.
    sla_monitor: SlaMonitor,
    /// Optional budget for per-execution cost/resource enforcement.
    /// Checked after every node completion in `handle_node_result`.
    budget: Option<orbflow_core::metering::Budget>,
    // NOTE: RBAC enforcement lives in the HTTP layer (AppState holds
    // Arc<RwLock<RbacPolicy>>). The engine does not need its own copy.
    // If per-execution RBAC is needed in the future, thread it through
    // NodeInput or a separate authorization port trait.
    /// Optional persistent budget store for org-level cost enforcement.
    budget_store: Option<Arc<dyn orbflow_core::ports::BudgetStore>>,
    /// Semaphore to bound the number of in-flight metrics/budget persistence tasks.
    /// Prevents unbounded task accumulation under Postgres degradation.
    metrics_semaphore: Arc<tokio::sync::Semaphore>,
    /// Semaphore to bound the number of concurrent result message handlers.
    /// Used by the engine's run loop to cap concurrent `handle_result_message` calls.
    #[allow(dead_code)]
    result_semaphore: Arc<tokio::sync::Semaphore>,
    /// Weak self-reference set after wrapping in `Arc`, used by `start()`.
    self_ref: OnceLock<Weak<Self>>,
}

impl OrbflowEngine {
    /// Creates a new engine from [`orbflow_core::EngineOptions`].
    pub fn new(opts: orbflow_core::EngineOptions) -> Self {
        Self {
            store: opts.store,
            bus: opts.bus,
            cred_store: opts.credential_store,
            metrics_store: opts.metrics_store,
            pool_name: opts.pool_name,
            snapshot_interval: opts.snapshot_interval,
            cel: CelEvaluator::new(),
            nodes: RwLock::new(HashMap::new()),
            schemas: RwLock::new(HashMap::new()),
            instance_mu: DashMap::new(),
            processed_results: DashMap::new(),
            stop_notify: Notify::new(),
            metrics: MetricsRecorder::new(),
            sla_monitor: SlaMonitor::new(),
            budget: opts.budget,
            budget_store: opts.budget_store,
            metrics_semaphore: Arc::new(tokio::sync::Semaphore::new(100)),
            result_semaphore: Arc::new(tokio::sync::Semaphore::new(256)),
            self_ref: OnceLock::new(),
        }
    }

    /// Stores a weak self-reference so `start()` can obtain an `Arc<Self>`
    /// without unsafe code. Must be called after wrapping in `Arc`.
    pub fn set_self_ref(self: &Arc<Self>) {
        let _ = self.self_ref.set(Arc::downgrade(self));
    }

    /// Returns a per-instance mutex, creating one if needed.
    pub(crate) fn lock_instance(&self, id: &InstanceId) -> Arc<Mutex<()>> {
        self.instance_mu
            .entry(id.clone())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .value()
            .clone()
    }

    /// Removes per-instance state once the instance is terminal.
    fn cleanup_instance(&self, inst: &Instance) {
        if inst.is_terminal() {
            self.instance_mu.remove(&inst.id);
            self.processed_results.remove(&inst.id);
        }
    }

    /// Removes DashMap entries for instances that are confirmed terminal in the store.
    /// Called periodically to prevent unbounded memory growth from stalled instances.
    #[allow(dead_code)]
    pub(crate) async fn sweep_stale_instance_state(&self) {
        let instance_ids: Vec<InstanceId> = self
            .instance_mu
            .iter()
            .map(|entry| entry.key().clone())
            .collect();

        for id in instance_ids {
            match self.store.get_instance(&id).await {
                Ok(inst) if inst.is_terminal() => {
                    self.instance_mu.remove(&id);
                    self.processed_results.remove(&id);
                }
                Err(_) => {
                    // Instance not found in store — clean up stale entry.
                    self.instance_mu.remove(&id);
                    self.processed_results.remove(&id);
                }
                _ => {}
            }
        }
    }

    /// Checks SLA thresholds and records any anomaly events on the instance.
    ///
    /// Consolidates the duplicated duration-violation / latency / failure-rate
    /// anomaly handling that was previously copy-pasted across three methods.
    async fn check_and_record_sla_anomalies(&self, inst: &Instance, duration_ms: i64) {
        match self
            .sla_monitor
            .check_and_record(&inst.workflow_id, duration_ms)
        {
            SlaCheckResult::Ok => {}
            SlaCheckResult::DurationViolation { actual_ms, max_ms } => {
                warn!(
                    instance = %inst.id,
                    workflow = %inst.workflow_id,
                    actual_ms,
                    max_ms,
                    "SLA violation: workflow duration exceeded threshold"
                );
                if let Err(e) = self
                    .store
                    .append_event(DomainEvent::AnomalyDetected(AnomalyDetectedEvent {
                        base: BaseEvent::new(inst.id.clone(), inst.version + 1),
                        anomaly_type: "duration_violation".into(),
                        message: format!("Duration {actual_ms}ms exceeded threshold {max_ms}ms"),
                        severity: "high".into(),
                    }))
                    .await
                {
                    error!(error = %e, instance = %inst.id, "failed to persist SLA duration violation event");
                }
            }
            SlaCheckResult::LatencyAnomaly {
                actual_ms,
                expected_mean_ms,
                z_score,
            } => {
                warn!(
                    instance = %inst.id,
                    workflow = %inst.workflow_id,
                    actual_ms,
                    expected_mean_ms,
                    z_score,
                    "SLA anomaly: unusual workflow latency detected"
                );
                if let Err(e) = self.store.append_event(DomainEvent::AnomalyDetected(AnomalyDetectedEvent {
                    base: BaseEvent::new(inst.id.clone(), inst.version + 1),
                    anomaly_type: "latency".into(),
                    message: format!("Latency {actual_ms}ms deviates from mean {expected_mean_ms:.1}ms (z={z_score:.2})"),
                    severity: severity_from_z(z_score).into(),
                })).await {
                    error!(error = %e, instance = %inst.id, "failed to persist SLA latency anomaly event");
                }
            }
            SlaCheckResult::FailureRateAnomaly {
                workflow_id: _,
                failure_rate,
                expected_rate,
                z_score,
            } => {
                warn!(
                    instance = %inst.id,
                    workflow = %inst.workflow_id,
                    failure_rate,
                    expected_rate,
                    z_score,
                    "SLA anomaly: unusual workflow failure rate detected"
                );
                if let Err(e) = self.store.append_event(DomainEvent::AnomalyDetected(AnomalyDetectedEvent {
                    base: BaseEvent::new(inst.id.clone(), inst.version + 1),
                    anomaly_type: "failure_rate".into(),
                    message: format!("Failure rate {failure_rate:.2} deviates from expected {expected_rate:.2} (z={z_score:.2})"),
                    severity: severity_from_z(z_score).into(),
                })).await {
                    error!(error = %e, instance = %inst.id, "failed to persist SLA failure rate anomaly event");
                }
            }
        }
    }

    // --- Validation helpers ---

    /// Checks that every capability edge connects a provider whose capability
    /// type matches the consumer port's required type.
    fn validate_capability_types(&self, wf: &Workflow) -> Result<(), OrbflowError> {
        let schemas = self
            .schemas
            .read()
            .map_err(|_| OrbflowError::Internal("schemas lock poisoned".into()))?;
        for ce in &wf.capability_edges {
            let source_node = match wf.node_by_id(&ce.source_node_id) {
                Some(n) => n,
                None => continue,
            };
            let schema = match schemas.get(&source_node.plugin_ref) {
                Some(s) => s,
                None => continue,
            };
            let target_node = match wf.node_by_id(&ce.target_node_id) {
                Some(n) => n,
                None => continue,
            };
            for port in &target_node.capability_ports {
                if port.key == ce.target_port_key {
                    if schema.provides_capability.as_deref() != Some(&port.capability_type) {
                        return Err(OrbflowError::InvalidCapabilityEdge);
                    }
                    break;
                }
            }
        }
        Ok(())
    }

    /// Checks that every required capability port on every node is connected.
    fn validate_required_capabilities(&self, wf: &Workflow) -> Result<(), OrbflowError> {
        let mut connected: HashMap<&str, HashMap<&str, bool>> = HashMap::new();
        for ce in &wf.capability_edges {
            connected
                .entry(ce.target_node_id.as_str())
                .or_default()
                .insert(ce.target_port_key.as_str(), true);
        }
        for node in &wf.nodes {
            for port in &node.capability_ports {
                if port.required {
                    let is_connected = connected
                        .get(node.id.as_str())
                        .and_then(|m| m.get(port.key.as_str()))
                        .copied()
                        .unwrap_or(false);
                    if !is_connected {
                        return Err(OrbflowError::MissingCapability);
                    }
                }
            }
        }
        Ok(())
    }

    /// Validates node configs against registered schemas.
    fn validate_node_configs(&self, wf: &Workflow) -> Result<(), OrbflowError> {
        let schemas = self
            .schemas
            .read()
            .map_err(|_| OrbflowError::Internal("schemas lock poisoned".into()))?;
        orbflow_core::validate_node_configs(wf, &schemas)
    }

    // --- Core dispatch/resolution methods ---

    /// Dispatches a node to the bus for execution.
    pub(crate) async fn dispatch_node(
        &self,
        inst: &mut Instance,
        wf: &Workflow,
        node: &Node,
    ) -> Result<(), OrbflowError> {
        let dispatch_span = tracing::info_span!(
            "orbflow.engine.dispatch_node",
            workflow_id = %inst.workflow_id,
            instance_id = %inst.id,
            node_id = %node.id,
            plugin_ref = %node.plugin_ref,
        );

        dispatch_span.in_scope(|| {
            tracing::debug!(
                instance_id = %inst.id,
                node_id = %node.id,
                plugin_ref = %node.plugin_ref,
                "dispatching node"
            );
        });

        let ns = inst
            .node_states
            .get_mut(&node.id)
            .ok_or_else(|| OrbflowError::Internal(format!("node state not found: {}", node.id)))?;

        // --- Approval gate ---
        // If the node requires human approval and is not already resuming from
        // an approved state, pause execution instead of dispatching to the bus.
        if node.requires_approval && ns.status != NodeStatus::Queued {
            ns.status = NodeStatus::WaitingApproval;
            let now = Utc::now();
            ns.started_at = Some(now);

            // Resolve and store input so it's visible in the approval UI.
            let input = self
                .resolve_input_mapping(node.input_mapping.as_ref(), &inst.context)
                .await?;
            ns.input = Some(input);

            if let Err(e) = self
                .store
                .append_event(DomainEvent::NodeApprovalRequested(
                    NodeApprovalRequestedEvent {
                        base: BaseEvent::new(inst.id.clone(), inst.version),
                        node_id: node.id.clone(),
                        message: None,
                    },
                ))
                .await
            {
                error!(error = %e, instance = %inst.id, node = %node.id, "failed to persist approval request event");
            }

            info!(
                instance = %inst.id,
                node = %node.id,
                "node requires approval — paused"
            );
            return Ok(());
        }

        ns.status = NodeStatus::Queued;
        let now = Utc::now();
        ns.started_at = Some(now);
        if ns.attempt == 0 {
            ns.attempt = 1;
        }

        let input = self
            .resolve_input_mapping(node.input_mapping.as_ref(), &inst.context)
            .await?;
        ns.input = Some(input.clone());

        // Append queued event.
        //
        // DESIGN: Event-sourcing appends are observability/audit records; the primary source of
        // truth is the instance snapshot persisted via `save_instance()`. Failures are logged
        // with structured fields but do not abort execution — a transient event-store write
        // failure does not corrupt workflow state.
        if let Err(e) = self
            .store
            .append_event(DomainEvent::NodeQueued(NodeQueuedEvent {
                base: BaseEvent::new(inst.id.clone(), inst.version),
                node_id: node.id.clone(),
            }))
            .await
        {
            error!(error = %e, instance = %inst.id, node = %node.id, "failed to persist NodeQueued event");
        }

        let mut task = TaskMessage {
            instance_id: inst.id.clone(),
            node_id: node.id.clone(),
            plugin_ref: node.plugin_ref.clone(),
            config: node.config.clone(),
            input: Some(input),
            parameters: None,
            capabilities: None,
            attempt: ns.attempt,
            trace_context: None,
            v: WIRE_VERSION,
        };

        // Resolve unified parameters.
        if !node.parameters.is_empty() {
            task.parameters = Some(
                self.resolve_parameters(&node.parameters, &inst.context)
                    .await,
            );
        }

        // Resolve credential references from config and parameters.
        // Deduplicate: collect credential_ids from both, fetch each once.
        let resolved_creds = self
            .resolve_credentials_deduped(
                task.config.as_ref(),
                task.parameters.as_ref(),
                inst.owner_id.as_deref(),
            )
            .await?;

        // Apply resolved credential data into config and parameters.
        // Apply resolved credentials into config.
        if let Some(config) = task.config.as_mut()
            && let Some(serde_json::Value::String(cred_id)) = config.remove(CREDENTIAL_ID_KEY)
            && let Some((_cred_type, _tier, cred_data)) = resolved_creds.get(&cred_id)
        {
            for (k, v) in cred_data {
                config.entry(k.clone()).or_insert_with(|| v.clone());
            }
        }
        // Apply resolved credentials into parameters (backward compat).
        if let Some(params) = task.parameters.as_mut()
            && let Some(serde_json::Value::String(cred_id)) = params.remove(CREDENTIAL_ID_KEY)
            && let Some((_cred_type, _tier, cred_data)) = resolved_creds.get(&cred_id)
        {
            for (k, v) in cred_data {
                params.entry(k.clone()).or_insert_with(|| v.clone());
            }
        }

        // Resolve capability edges.
        if !node.capability_ports.is_empty() {
            task.capabilities = resolve_capability_edges_from_context(&inst.context, wf, node);
        }

        // Store parameters in node state for activity viewer display,
        // redacting only secret (password-type) credential fields.
        // Non-secret fields like base_url and max_tokens remain visible.
        ns.parameters = task.parameters.as_ref().map(|params| {
            let mut redacted = params.clone();
            for (cred_type, _tier, cred_data) in resolved_creds.values() {
                let secrets = orbflow_core::CredentialSchemas::secret_keys(cred_type);
                for key in cred_data.keys() {
                    // For known credential types, only redact password-type fields.
                    // For unknown/custom types, redact all credential-derived keys.
                    if (secrets.is_empty() || secrets.contains(key.as_str()))
                        && let Some(entry) = redacted.get_mut(key)
                    {
                        *entry = serde_json::Value::String(
                            "\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}"
                                .to_string(),
                        );
                    }
                }
            }
            redacted
        });

        // Redact secret credential fields from the task message before bus
        // publish so that passwords/tokens never travel in plaintext over NATS.
        // Non-secret fields (base_url, model, etc.) remain for the worker.
        // When the credential's access tier is Raw, secrets are passed through
        // so that plugins making direct API calls can use them.
        for map in [task.config.as_mut(), task.parameters.as_mut()]
            .into_iter()
            .flatten()
        {
            for (cred_type, tier, cred_data) in resolved_creds.values() {
                if *tier == CredentialAccessTier::Raw {
                    continue;
                }
                let secrets = orbflow_core::CredentialSchemas::secret_keys(cred_type);
                for key in cred_data.keys() {
                    if (secrets.is_empty() || secrets.contains(key.as_str()))
                        && let Some(entry) = map.get_mut(key)
                    {
                        *entry = serde_json::Value::Null;
                    }
                }
            }
        }

        // Inject OTel trace context into the task message for distributed
        // trace propagation through NATS to the worker.
        {
            let cx = dispatch_span.context();
            let mut injector = std::collections::HashMap::new();
            opentelemetry::global::get_text_map_propagator(|propagator| {
                propagator.inject_context(&cx, &mut injector);
            });
            if !injector.is_empty() {
                task.trace_context = Some(injector);
            }
        }

        let data = serde_json::to_vec(&task)
            .map_err(|e| OrbflowError::Internal(format!("marshal task: {e}")))?;

        self.bus
            .publish(&task_subject(&self.pool_name), &data)
            .await
    }

    /// Resolves input mapping: maps node input declarations to values from the
    /// execution context. If a value is a string starting with "=", it's
    /// treated as a CEL expression.
    pub(crate) async fn resolve_input_mapping(
        &self,
        mapping: Option<&HashMap<String, serde_json::Value>>,
        ec: &ExecutionContext,
    ) -> Result<HashMap<String, serde_json::Value>, OrbflowError> {
        let mapping = match mapping {
            Some(m) if !m.is_empty() => m,
            // No input mapping — return a shallow copy of variables.
            // TODO: Consider using Arc<HashMap> for the variables map to avoid
            // per-node cloning. For workflows with many variables × many nodes
            // this can cause significant memory growth.
            _ => return Ok(ec.variables.clone()),
        };

        let cel_ctx = build_mapping_context(&ec.node_outputs, &ec.variables);

        let mut result = HashMap::with_capacity(mapping.len());
        for (key, val) in mapping {
            match val {
                serde_json::Value::String(v) => {
                    // CEL expression: starts with "="
                    if v.len() > 1 && v.starts_with('=') {
                        let expr = &v[1..];
                        match self.cel.eval_any_async(expr, &cel_ctx).await {
                            Ok(evaluated) => {
                                result.insert(key.clone(), evaluated);
                                continue;
                            }
                            Err(e) => {
                                warn!(
                                    expression = expr,
                                    key = key.as_str(),
                                    error = %e,
                                    "CEL evaluation failed — returning error instead of raw expression"
                                );
                                return Err(OrbflowError::InvalidNodeConfig(format!(
                                    "CEL expression failed for key \"{key}\": {e}"
                                )));
                            }
                        }
                    }
                    // Simple variable reference fallback (non-CEL string values).
                    if let Some(out) = ec.node_outputs.get(v.as_str()) {
                        result.insert(
                            key.clone(),
                            serde_json::Value::Object(
                                out.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
                            ),
                        );
                    } else if let Some(var_val) = ec.variables.get(v.as_str()) {
                        result.insert(key.clone(), var_val.clone());
                    } else {
                        result.insert(key.clone(), val.clone());
                    }
                }
                _ => {
                    result.insert(key.clone(), val.clone());
                }
            }
        }
        Ok(result)
    }

    /// Resolves unified Parameter values using CEL for expressions.
    pub(crate) async fn resolve_parameters(
        &self,
        params: &[Parameter],
        ec: &ExecutionContext,
    ) -> HashMap<String, serde_json::Value> {
        if params.is_empty() {
            return HashMap::new();
        }
        let cel_ctx = build_mapping_context(&ec.node_outputs, &ec.variables);
        let mut result = HashMap::with_capacity(params.len());
        for p in params {
            match p.mode {
                ParameterMode::Expression => {
                    if let Some(ref expr) = p.expression
                        && !expr.is_empty()
                    {
                        match self.cel.eval_any_async(expr, &cel_ctx).await {
                            Ok(evaluated) => {
                                result.insert(p.key.clone(), evaluated);
                                continue;
                            }
                            Err(e) => {
                                warn!(
                                    expression = expr.as_str(),
                                    key = p.key.as_str(),
                                    error = %e,
                                    "CEL parameter evaluation failed"
                                );
                            }
                        }
                    }
                    // Fallback to static value.
                    if let Some(ref v) = p.value {
                        result.insert(p.key.clone(), v.clone());
                    }
                }
                ParameterMode::Static => {
                    if let Some(ref v) = p.value {
                        result.insert(p.key.clone(), v.clone());
                    }
                }
            }
        }
        result
    }

    /// Resolves credential references by collecting unique credential_ids from
    /// both config and parameters, fetching each once, and returning the data.
    ///
    /// Returns a map of credential_id → (credential_type, credential data).
    /// The caller applies the data into the appropriate maps and strips
    /// credential_id before publishing to the bus.  The credential_type is
    /// used to look up which fields are secrets (password-type) for redaction.
    ///
    /// When `owner_id` is provided, credentials are fetched with owner scoping
    /// via `get_credential_for_owner` to enforce tenant isolation.
    pub(crate) async fn resolve_credentials_deduped(
        &self,
        config: Option<&HashMap<String, serde_json::Value>>,
        parameters: Option<&HashMap<String, serde_json::Value>>,
        owner_id: Option<&str>,
    ) -> Result<HashMap<String, (String, CredentialAccessTier, HashMap<String, serde_json::Value>)>, OrbflowError> {
        let mut result = HashMap::new();

        let cred_store = match &self.cred_store {
            Some(cs) => cs,
            None => return Ok(result),
        };

        // Collect unique credential IDs from both maps.
        let mut cred_ids = std::collections::HashSet::new();
        for map in [config, parameters].into_iter().flatten() {
            if let Some(serde_json::Value::String(id)) = map.get(CREDENTIAL_ID_KEY)
                && !id.is_empty()
            {
                cred_ids.insert(id.clone());
            }
        }

        // Fetch each unique credential once.
        for cred_id in cred_ids {
            let cred_id_obj = match orbflow_core::CredentialId::new(&cred_id) {
                Ok(id) => id,
                Err(e) => {
                    return Err(OrbflowError::Internal(format!(
                        "invalid credential ID: {e}"
                    )));
                }
            };
            let fetch_result = match owner_id {
                Some(oid) => cred_store.get_credential_for_owner(&cred_id_obj, Some(oid)).await,
                None => cred_store.get_credential(&cred_id_obj).await,
            };
            match fetch_result {
                Ok(cred) => {
                    // Enforce allowed_tiers policy: only allow non-Proxy tiers
                    // when the credential has an explicit policy that permits it.
                    // Raw credentials without a policy fall back to Proxy to
                    // prevent plaintext secrets on NATS.
                    let effective_tier = match &cred.policy {
                        None if cred.access_tier != CredentialAccessTier::Proxy => {
                            tracing::warn!(
                                credential = %cred_id,
                                tier = ?cred.access_tier,
                                "non-Proxy tier with no policy, falling back to Proxy"
                            );
                            CredentialAccessTier::default()
                        }
                        Some(policy)
                            if !policy.allowed_tiers.contains(&cred.access_tier) =>
                        {
                            tracing::warn!(
                                credential = %cred_id,
                                tier = ?cred.access_tier,
                                allowed = ?policy.allowed_tiers,
                                "access tier not in allowed_tiers, falling back to Proxy"
                            );
                            CredentialAccessTier::default()
                        }
                        _ => cred.access_tier,
                    };
                    result.insert(
                        cred_id,
                        (cred.credential_type, effective_tier, cred.data),
                    );
                }
                Err(e) => {
                    let truncated = &cred_id[..8.min(cred_id.len())];
                    if e.is_not_found() {
                        return Err(OrbflowError::InvalidNodeConfig(format!(
                            "credential '{truncated}' not found"
                        )));
                    }
                    return Err(OrbflowError::Internal(format!(
                        "failed to resolve credential {truncated}: {e}"
                    )));
                }
            }
        }

        Ok(result)
    }

    /// Executes all capability nodes at instance start. Their outputs are cached
    /// in the execution context for use by action nodes.
    pub(crate) async fn resolve_capabilities(
        &self,
        inst: &mut Instance,
        wf: &Workflow,
    ) -> Result<(), OrbflowError> {
        let cap_nodes = wf.capability_nodes();
        if cap_nodes.is_empty() {
            return Ok(());
        }

        for cn in cap_nodes {
            let executor = {
                let nodes = self
                    .nodes
                    .read()
                    .map_err(|_| OrbflowError::Internal("node registry lock poisoned".into()))?;
                nodes
                    .get(&cn.plugin_ref)
                    .cloned()
                    .ok_or(OrbflowError::NodeNotFound)?
            };

            // Resolve parameters for the capability node.
            let mut resolved_params = if !cn.parameters.is_empty() {
                Some(self.resolve_parameters(&cn.parameters, &inst.context).await)
            } else {
                None
            };

            // Resolve credential references in capability node config + params.
            let cap_creds = self
                .resolve_credentials_deduped(
                    cn.config.as_ref(),
                    resolved_params.as_ref(),
                    inst.owner_id.as_deref(),
                )
                .await?;
            if let Some(params) = resolved_params.as_mut()
                && let Some(serde_json::Value::String(cred_id)) = params.remove(CREDENTIAL_ID_KEY)
                && let Some((_cred_type, _tier, cred_data)) = cap_creds.get(&cred_id)
            {
                for (k, v) in cred_data {
                    params.entry(k.clone()).or_insert_with(|| v.clone());
                }
            }

            let input = NodeInput {
                instance_id: inst.id.clone(),
                node_id: cn.id.clone(),
                plugin_ref: cn.plugin_ref.clone(),
                config: cn.config.clone(),
                input: Some(
                    self.resolve_input_mapping(cn.input_mapping.as_ref(), &inst.context)
                        .await?,
                ),
                parameters: resolved_params,
                capabilities: None,
                attempt: 1,
            };

            let output = executor.execute(&input).await?;
            if let Some(ref err) = output.error {
                return Err(OrbflowError::Internal(format!(
                    "capability node {} error: {err}",
                    cn.id
                )));
            }

            let now = Utc::now();
            if let Some(ns) = inst.node_states.get_mut(&cn.id) {
                ns.status = NodeStatus::Completed;
                ns.output = output.data.clone();
                ns.started_at = Some(now);
                ns.ended_at = Some(now);
                // Redact only secret (password-type) credential fields.
                ns.parameters = input.parameters.as_ref().map(|params| {
                    let mut redacted = params.clone();
                    for (cred_type, _tier, cred_data) in cap_creds.values() {
                        let secrets = orbflow_core::CredentialSchemas::secret_keys(cred_type);
                        for key in cred_data.keys() {
                            if (secrets.is_empty() || secrets.contains(key.as_str()))
                                && let Some(entry) = redacted.get_mut(key)
                            {
                                *entry = serde_json::Value::String(
                                    "\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}".to_string(),
                                );
                            }
                        }
                    }
                    redacted
                });
            }
            if let Some(data) = output.data {
                inst.context.node_outputs.insert(cn.id.clone(), data);
            }
        }
        Ok(())
    }

    /// Marks all trigger-kind nodes as completed, recording the workflow input
    /// as their output so downstream nodes can reference it. Returns `true` when
    /// at least one trigger node was found.
    fn mark_triggers_completed(
        &self,
        inst: &mut Instance,
        wf: &Workflow,
        input: &HashMap<String, serde_json::Value>,
    ) -> bool {
        let trigger_nodes = wf.trigger_nodes();
        if trigger_nodes.is_empty() {
            return false;
        }
        let now = Utc::now();
        for tn in &trigger_nodes {
            if let Some(ns) = inst.node_states.get_mut(&tn.id) {
                ns.status = NodeStatus::Completed;
                ns.output = Some(input.clone());
                ns.started_at = Some(now);
                ns.ended_at = Some(now);
            }
            inst.context
                .node_outputs
                .insert(tn.id.clone(), input.clone());
        }
        true
    }

    /// Dispatches the initial set of ready nodes after workflow start.
    ///
    /// When a node fails to dispatch (e.g. missing credential), the node is
    /// marked as `Failed` and the instance is transitioned to `Failed` so the
    /// UI correctly reflects the error.
    async fn dispatch_initial_nodes(
        &self,
        inst: &mut Instance,
        wf: &Workflow,
        has_triggers: bool,
    ) -> Result<(), OrbflowError> {
        // Dispatch non-trigger entry nodes.
        let entry_nodes: Vec<Node> = wf
            .entry_nodes()
            .into_iter()
            .filter(|n| n.kind != NodeKind::Trigger)
            .cloned()
            .collect();
        for n in &entry_nodes {
            if let Err(e) = self.dispatch_node(inst, wf, n).await {
                error!(node = n.id.as_str(), error = %e, "failed to dispatch entry node");
                self.mark_node_dispatch_failed(inst, &n.id, &e).await?;
            }
        }

        // If there were trigger nodes, also dispatch nodes that are now ready
        // downstream of those triggers.
        if !has_triggers {
            return Ok(());
        }
        let ready = find_ready_nodes(wf, inst, &self.cel).await;
        for node_id in &ready {
            if let Some(node) = wf.node_by_id(node_id) {
                let node = node.clone();
                if let Err(e) = self.dispatch_node(inst, wf, &node).await {
                    error!(
                        node = node_id.as_str(),
                        error = %e,
                        "failed to dispatch post-trigger node"
                    );
                    self.mark_node_dispatch_failed(inst, node_id, &e).await?;
                }
            }
        }
        Ok(())
    }

    /// Marks a node as failed due to a dispatch-time error (e.g. missing
    /// credential) and transitions the instance to `Failed`.
    async fn mark_node_dispatch_failed(
        &self,
        inst: &mut Instance,
        node_id: &str,
        error: &OrbflowError,
    ) -> Result<(), OrbflowError> {
        let now = Utc::now();
        let error_msg = error.to_string();

        if let Some(ns) = inst.node_states.get_mut(node_id) {
            ns.status = NodeStatus::Failed;
            ns.error = Some(error_msg.clone());
            ns.started_at = Some(now);
            ns.ended_at = Some(now);
        }

        inst.status = InstanceStatus::Failed;
        inst.updated_at = now;

        self.sla_monitor.record_failure(&inst.workflow_id);
        let duration_secs = (now - inst.created_at).num_milliseconds() as f64 / 1000.0;
        self.metrics
            .record_workflow_failed(&inst.workflow_id.0, duration_secs);

        self.store
            .append_event(DomainEvent::NodeFailed(NodeFailedEvent {
                base: BaseEvent::new(inst.id.clone(), inst.version),
                node_id: node_id.to_string(),
                error: error_msg.clone(),
            }))
            .await?;

        inst.version += 1;

        self.store
            .append_event(DomainEvent::InstanceFailed(InstanceFailedEvent {
                base: BaseEvent::new(inst.id.clone(), inst.version),
                error: format!("node {} failed to dispatch: {}", node_id, error_msg),
            }))
            .await?;

        Ok(())
    }

    /// Returns true if any completed node in the instance has a compensation config.
    fn has_compensation(&self, wf: &Workflow, inst: &Instance) -> bool {
        for ns in inst.node_states.values() {
            if ns.status != NodeStatus::Completed {
                continue;
            }
            if let Some(node) = wf.node_by_id(&ns.node_id)
                && node.compensate.is_some()
            {
                return true;
            }
        }
        false
    }

    /// Saves an instance with optimistic locking retry (loop-based to avoid
    /// async recursion). Increments version before persisting. On version
    /// conflicts, re-fetches, merges, and retries up to
    /// [`SAVE_INSTANCE_MAX_RETRIES`] times.
    pub(crate) async fn save_instance(&self, inst: &mut Instance) -> Result<(), OrbflowError> {
        let mut attempt: usize = 0;
        loop {
            inst.version += 1;
            inst.updated_at = Utc::now();
            match self.store.update_instance(inst).await {
                Ok(()) => {
                    // Take a snapshot at configured intervals for crash recovery.
                    if self.snapshot_interval > 0
                        && inst.version % self.snapshot_interval == 0
                        && let Err(e) = self.store.save_snapshot(inst).await
                    {
                        warn!(
                            instance = %inst.id,
                            version = inst.version,
                            error = %e,
                            "failed to save snapshot"
                        );
                    }
                    return Ok(());
                }
                Err(e) if e.is_conflict() => {
                    if attempt >= SAVE_INSTANCE_MAX_RETRIES {
                        error!(
                            instance = %inst.id,
                            attempts = attempt,
                            "instance version conflict: max retries exceeded, marking as failed"
                        );
                        // Best-effort: mark instance as Failed so it doesn't stall forever.
                        // Re-fetch to get latest version, then force-set Failed status.
                        if let Ok(mut latest) = self.store.get_instance(&inst.id).await
                            && !latest.is_terminal()
                        {
                            latest.status = InstanceStatus::Failed;
                            latest.version += 1;
                            latest.updated_at = Utc::now();
                            let _ = self.store.update_instance(&latest).await;
                        }
                        return Err(OrbflowError::Internal(format!(
                            "save instance {}: max retries exceeded after version conflict",
                            inst.id
                        )));
                    }

                    warn!(
                        instance = %inst.id,
                        local_version = inst.version,
                        attempt = attempt + 1,
                        "instance version conflict, re-fetching"
                    );

                    let fresh = self.store.get_instance(&inst.id).await?;

                    // Check if all local node state changes are already applied.
                    let mut needs_merge = false;
                    for (node_id, local_ns) in &inst.node_states {
                        match fresh.node_states.get(node_id) {
                            None => {
                                needs_merge = true;
                                break;
                            }
                            Some(fresh_ns) => {
                                if local_ns.status.is_terminal() && !fresh_ns.status.is_terminal() {
                                    needs_merge = true;
                                    break;
                                }
                            }
                        }
                    }

                    if !needs_merge {
                        info!(
                            instance = %inst.id,
                            "version conflict resolved: changes already applied"
                        );
                        return Ok(());
                    }

                    // Merge local changes into fresh instance.
                    let local_status = inst.status;
                    let local_updated = inst.updated_at;
                    let local_outputs = inst.context.node_outputs.clone();
                    let local_variables = inst.context.variables.clone();
                    let updates: Vec<(String, NodeState)> = inst
                        .node_states
                        .iter()
                        .filter_map(|(node_id, local_ns)| match fresh.node_states.get(node_id) {
                            None => Some((node_id.clone(), local_ns.clone())),
                            Some(fresh_ns) => {
                                if (local_ns.status.is_terminal() && !fresh_ns.status.is_terminal())
                                    || (local_ns.status == NodeStatus::Queued
                                        && fresh_ns.status == NodeStatus::Pending)
                                {
                                    Some((node_id.clone(), local_ns.clone()))
                                } else {
                                    None
                                }
                            }
                        })
                        .collect();

                    // Replace inst with fresh, then apply local updates.
                    *inst = fresh;
                    for (node_id, ns) in updates {
                        inst.node_states.insert(node_id, ns);
                    }
                    inst.status = local_status;
                    inst.updated_at = local_updated;
                    for (k, v) in local_outputs {
                        inst.context.node_outputs.entry(k).or_insert(v);
                    }
                    for (k, v) in local_variables {
                        inst.context.variables.entry(k).or_insert(v);
                    }

                    attempt += 1;
                    // Loop will retry with the merged instance.
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Processes a result message received from the bus.
    async fn handle_result_message(
        &self,
        _subject: String,
        data: Vec<u8>,
    ) -> Result<(), OrbflowError> {
        let result: ResultMessage = serde_json::from_slice(&data)
            .map_err(|e| OrbflowError::Internal(format!("unmarshal result: {e}")))?;

        // Validate wire version: reject messages from newer, unknown versions
        // to prevent silent behavior divergence during rolling deployments.
        if result.v > WIRE_VERSION {
            return Err(OrbflowError::Bus(format!(
                "received result with wire version {} but engine only supports up to {}",
                result.v, WIRE_VERSION
            )));
        }

        self.handle_node_result(&result).await
    }

    /// Public entry point for processing a completed node result and advancing
    /// the DAG.
    pub async fn handle_node_result(&self, result: &ResultMessage) -> Result<(), OrbflowError> {
        // Serialize result processing per instance — lock BEFORE idempotency
        // check to prevent TOCTOU race where two concurrent results with the
        // same result_id both pass the `contains` check before either records.
        let mu = self.lock_instance(&result.instance_id);
        let _guard = mu.lock().await;

        // Idempotency check (inside per-instance lock).
        if let Some(ref result_id) = result.result_id
            && !result_id.is_empty()
        {
            let rs = self
                .processed_results
                .entry(result.instance_id.clone())
                .or_insert_with(|| Arc::new(crate::dedup::ResultSet::new()))
                .value()
                .clone();
            if rs.contains(result_id) {
                info!(
                    instance = %result.instance_id,
                    node = result.node_id.as_str(),
                    result_id = result_id.as_str(),
                    "duplicate result skipped"
                );
                return Ok(());
            }
            rs.add(result_id.clone());
        }

        let span = tracing::info_span!(
            SPAN_NODE_EXECUTE,
            instance_id = %result.instance_id,
            node_id = %result.node_id,
            otel.name = "handle_node_result",
        );
        let _enter = span.enter();
        // Drop the span guard before any `.await` to satisfy `Send` bounds.
        drop(_enter);

        let mut inst = self.store.get_instance(&result.instance_id).await?;

        // Route compensation results to the saga handler.
        if result.node_id.starts_with("_compensate_") {
            crate::saga::handle_compensation_result(self, &mut inst, &result.node_id).await?;
            self.cleanup_instance(&inst);
            return Ok(());
        }

        if inst.is_terminal() {
            self.cleanup_instance(&inst);
            return Ok(());
        }

        let wf = self.store.get_workflow(&inst.workflow_id).await?;

        let ns = inst.node_states.get(&result.node_id).ok_or_else(|| {
            OrbflowError::Internal(format!(
                "unknown node {} in instance {}",
                result.node_id, inst.id
            ))
        })?;
        // We need the attempt count before borrowing mutably.
        let attempt = ns.attempt;

        let now = Utc::now();

        if let Some(ref error_msg) = result.error {
            // Extract worker-provided `_metrics` from the error result output.
            let failed_worker_metrics = result
                .output
                .as_ref()
                .and_then(|o| o.get("_metrics"))
                .and_then(|v| serde_json::from_value::<metering::NodeMetrics>(v.clone()).ok());

            info!(
                instance = %result.instance_id,
                node = result.node_id.as_str(),
                error = error_msg.as_str(),
                attempt = attempt,
                "node execution failed"
            );

            // Check for retry.
            let node = wf.node_by_id(&result.node_id);
            if let Some(node) = node
                && let Some(ref retry) = node.retry
                && attempt < retry.max_attempts
            {
                let ns = inst.node_states.get_mut(&result.node_id).ok_or_else(|| {
                    OrbflowError::Internal(format!(
                        "node state missing for result: {}",
                        result.node_id
                    ))
                })?;
                ns.attempt += 1;
                ns.status = NodeStatus::Queued;
                inst.updated_at = now;
                self.save_instance(&mut inst).await?;
                let node = node.clone();
                return self.dispatch_node(&mut inst, &wf, &node).await;
            }

            // Max retries exhausted or no retry policy.
            let ns = inst.node_states.get_mut(&result.node_id).ok_or_else(|| {
                OrbflowError::Internal(format!("node state missing for result: {}", result.node_id))
            })?;
            ns.status = NodeStatus::Failed;
            ns.error = Some(error_msg.clone());
            ns.ended_at = Some(now);
            inst.updated_at = now;

            if let (Some(started), Some(ended)) = (ns.started_at, ns.ended_at) {
                let duration_secs = (ended - started).num_milliseconds() as f64 / 1000.0;
                let plugin_ref = wf
                    .node_by_id(&result.node_id)
                    .map(|n| n.plugin_ref.as_str())
                    .unwrap_or("unknown");
                self.metrics.record_node_failed(
                    &inst.workflow_id.0,
                    &result.node_id,
                    plugin_ref,
                    duration_secs,
                );
                // Persist node metrics for analytics queries.
                // Use worker-provided metrics for token/cost when available.
                if let Some(ref ms) = self.metrics_store {
                    let (tokens, cost_scaled) = match &failed_worker_metrics {
                        Some(wm) => (
                            wm.tokens.as_ref().map(|t| t.total_tokens),
                            if wm.cost_usd > 0.0 {
                                Some((wm.cost_usd * 10000.0) as i64)
                            } else {
                                None
                            },
                        ),
                        None => (None, None),
                    };
                    let nm = NodeExecutionMetrics {
                        instance_id: inst.id.clone(),
                        workflow_id: inst.workflow_id.clone(),
                        node_id: result.node_id.clone(),
                        plugin_ref: plugin_ref.to_string(),
                        status: "failed".into(),
                        duration_ms: (ended - started).num_milliseconds(),
                        started_at: started,
                        completed_at: ended,
                        attempt: ns.attempt,
                        tokens,
                        cost_usd_scaled: cost_scaled,
                    };
                    if let Ok(permit) = self.metrics_semaphore.clone().try_acquire_owned() {
                        let ms = ms.clone();
                        tokio::spawn(async move {
                            let _permit = permit;
                            if let Err(e) = ms.record_node_metrics(&nm).await {
                                tracing::warn!(error = %e, "failed to persist node metrics");
                            }
                        });
                    } else {
                        tracing::warn!(
                            "metrics write backlog full — skipping node metrics persistence"
                        );
                    }
                }
            }

            // Aggregate worker metrics into instance-level metering for failed nodes.
            if let Some(wm) = failed_worker_metrics {
                let im = inst.instance_metrics.get_or_insert_with(Default::default);
                im.record_node(&result.node_id, wm);
            }

            if let Err(e) = self
                .store
                .append_event(DomainEvent::NodeFailed(NodeFailedEvent {
                    base: BaseEvent::new(inst.id.clone(), inst.version),
                    node_id: result.node_id.clone(),
                    error: error_msg.clone(),
                }))
                .await
            {
                error!(error = %e, instance = %inst.id, node = %result.node_id, "failed to persist NodeFailed event");
            }

            // Check if saga compensation is needed.
            if self.has_compensation(&wf, &inst) {
                inst.status = InstanceStatus::Failed;
                self.save_instance(&mut inst).await?;
                return crate::saga::start_compensation(self, &mut inst, &wf, &result.node_id)
                    .await;
            }

            inst.status = InstanceStatus::Failed;
            self.sla_monitor.record_failure(&inst.workflow_id);
            let duration_secs = (Utc::now() - inst.created_at).num_milliseconds() as f64 / 1000.0;
            self.metrics
                .record_workflow_failed(&inst.workflow_id.0, duration_secs);
            let duration_ms = (Utc::now() - inst.created_at).num_milliseconds();

            // Check SLA.
            self.check_and_record_sla_anomalies(&inst, duration_ms)
                .await;

            // Persist instance metrics for analytics queries.
            if let Some(ref ms) = self.metrics_store {
                let node_durations: HashMap<String, i64> = inst
                    .node_states
                    .iter()
                    .filter_map(|(nid, ns)| {
                        if let (Some(s), Some(e)) = (ns.started_at, ns.ended_at) {
                            Some((nid.clone(), (e - s).num_milliseconds()))
                        } else {
                            None
                        }
                    })
                    .collect();
                let failed_count = inst
                    .node_states
                    .values()
                    .filter(|ns| ns.status == NodeStatus::Failed)
                    .count() as i32;
                let im = InstanceExecutionMetrics {
                    instance_id: inst.id.clone(),
                    workflow_id: inst.workflow_id.clone(),
                    status: "failed".into(),
                    duration_ms,
                    node_count: inst.node_states.len() as i32,
                    failed_node_count: failed_count,
                    started_at: inst.created_at,
                    completed_at: Utc::now(),
                    node_durations,
                };
                if let Ok(permit) = self.metrics_semaphore.clone().try_acquire_owned() {
                    let ms = ms.clone();
                    tokio::spawn(async move {
                        let _permit = permit;
                        if let Err(e) = ms.record_instance_metrics(&im).await {
                            tracing::warn!(error = %e, "failed to persist instance metrics");
                        }
                    });
                } else {
                    tracing::warn!(
                        "metrics write backlog full — skipping instance metrics persistence"
                    );
                }
            }

            if let Err(e) = self
                .store
                .append_event(DomainEvent::InstanceFailed(InstanceFailedEvent {
                    base: BaseEvent::new(inst.id.clone(), inst.version),
                    error: format!("node {} failed: {}", result.node_id, error_msg),
                }))
                .await
            {
                error!(error = %e, instance = %inst.id, node = %result.node_id, "failed to persist InstanceFailed event");
            }

            self.save_instance(&mut inst).await?;
            self.cleanup_instance(&inst);
            return Ok(());
        }

        // Success path.
        //
        // Extract worker-provided `_metrics` from the output, then strip it
        // so downstream consumers never see internal metering data.
        let (output, worker_metrics) = {
            let mut raw = result.output.clone();
            let wm = raw
                .as_mut()
                .and_then(|o| o.remove("_metrics"))
                .and_then(|v| serde_json::from_value::<metering::NodeMetrics>(v).ok());
            (raw, wm)
        };

        {
            let ns = inst.node_states.get_mut(&result.node_id).ok_or_else(|| {
                OrbflowError::Internal(format!("node state missing for result: {}", result.node_id))
            })?;
            ns.status = NodeStatus::Completed;
            ns.output = output.clone();
            ns.ended_at = Some(now);

            if let (Some(started), Some(ended)) = (ns.started_at, ns.ended_at) {
                let duration_secs = (ended - started).num_milliseconds() as f64 / 1000.0;
                let plugin_ref = wf
                    .node_by_id(&result.node_id)
                    .map(|n| n.plugin_ref.as_str())
                    .unwrap_or("unknown");
                self.metrics.record_node_completed(
                    &inst.workflow_id.0,
                    &result.node_id,
                    plugin_ref,
                    duration_secs,
                );
                // Persist node metrics for analytics queries.
                // Use worker-provided metrics for token/cost when available.
                if let Some(ref ms) = self.metrics_store {
                    let (tokens, cost_scaled) = match &worker_metrics {
                        Some(wm) => (
                            wm.tokens.as_ref().map(|t| t.total_tokens),
                            if wm.cost_usd > 0.0 {
                                Some((wm.cost_usd * 10000.0) as i64)
                            } else {
                                None
                            },
                        ),
                        None => (None, None),
                    };
                    let nm = NodeExecutionMetrics {
                        instance_id: inst.id.clone(),
                        workflow_id: inst.workflow_id.clone(),
                        node_id: result.node_id.clone(),
                        plugin_ref: plugin_ref.to_string(),
                        status: "completed".into(),
                        duration_ms: (ended - started).num_milliseconds(),
                        started_at: started,
                        completed_at: ended,
                        attempt: ns.attempt,
                        tokens,
                        cost_usd_scaled: cost_scaled,
                    };
                    if let Ok(permit) = self.metrics_semaphore.clone().try_acquire_owned() {
                        let ms = ms.clone();
                        tokio::spawn(async move {
                            let _permit = permit;
                            if let Err(e) = ms.record_node_metrics(&nm).await {
                                tracing::warn!(error = %e, "failed to persist node metrics");
                            }
                        });
                    } else {
                        tracing::warn!(
                            "metrics write backlog full — skipping node metrics persistence"
                        );
                    }
                }
            }
        }

        // --- Metering & budget enforcement ---
        {
            // Prefer worker-measured metrics; fall back to engine-side extraction.
            let node_metrics = if let Some(wm) = worker_metrics {
                wm
            } else {
                let ns = inst.node_states.get(&result.node_id);
                let wall_time_ms = ns
                    .and_then(|ns| ns.started_at.zip(ns.ended_at))
                    .map(|(s, e)| (e - s).num_milliseconds() as u64)
                    .unwrap_or(0);
                metering::extract_metrics_from_output(
                    output.as_ref().unwrap_or(&HashMap::new()),
                    wall_time_ms,
                )
            };

            let im = inst.instance_metrics.get_or_insert_with(Default::default);
            im.record_node(&result.node_id, node_metrics);

            // Budget enforcement: fail the instance if any limit is exceeded.
            if let Some(ref budget) = self.budget
                && let Some(reason) = budget.check(im)
            {
                warn!(instance = %inst.id, reason = %reason, "budget exceeded — failing instance");
                inst.status = InstanceStatus::Failed;
                if let Err(e) = self
                    .store
                    .append_event(DomainEvent::InstanceFailed(InstanceFailedEvent {
                        base: BaseEvent::new(inst.id.clone(), inst.version),
                        error: format!("budget exceeded: {reason}"),
                    }))
                    .await
                {
                    error!(error = %e, instance = %inst.id, "failed to persist InstanceFailed (budget) event");
                }
                self.save_instance(&mut inst).await?;
                self.cleanup_instance(&inst);
                return Ok(());
            }
        }

        if let Some(ref out) = output {
            inst.context
                .node_outputs
                .insert(result.node_id.clone(), out.clone());
        }
        inst.updated_at = now;

        // Redact known secret field names from node output before persisting
        // to the event store, preventing credential leakage in the audit log.
        const SECRET_KEYS: &[&str] = &[
            "password",
            "token",
            "secret",
            "api_key",
            "apikey",
            "access_token",
            "refresh_token",
            "private_key",
        ];
        let redacted_output = output.as_ref().map(|out| {
            let needs_redaction = SECRET_KEYS.iter().any(|k| out.contains_key(*k));
            if needs_redaction {
                let mut redacted = out.clone();
                for key in SECRET_KEYS {
                    if let Some(entry) = redacted.get_mut(*key) {
                        *entry = serde_json::Value::String("••••••••".to_string());
                    }
                }
                redacted
            } else {
                out.clone()
            }
        });
        if let Err(e) = self
            .store
            .append_event(DomainEvent::NodeCompleted(NodeCompletedEvent {
                base: BaseEvent::new(inst.id.clone(), inst.version),
                node_id: result.node_id.clone(),
                output: redacted_output,
            }))
            .await
        {
            error!(error = %e, instance = %inst.id, node = %result.node_id, "failed to persist NodeCompleted event");
        }

        // Find and dispatch ready nodes.
        let ready_nodes = find_ready_nodes(&wf, &mut inst, &self.cel).await;
        for node_id in &ready_nodes {
            if let Some(node) = wf.node_by_id(node_id) {
                let node = node.clone();
                if let Err(e) = self.dispatch_node(&mut inst, &wf, &node).await {
                    error!(node = node_id.as_str(), error = %e, "failed to dispatch node");
                    // Mark the node as failed so the instance doesn't stall silently.
                    if let Some(ns) = inst.node_states.get_mut(node_id) {
                        ns.status = NodeStatus::Failed;
                        ns.error = Some(format!("dispatch failed: {e}"));
                        ns.ended_at = Some(Utc::now());
                    }
                }
            }
        }

        if inst.all_nodes_terminal() {
            let has_failed_nodes = inst
                .node_states
                .values()
                .any(|ns| ns.status == NodeStatus::Failed);

            if has_failed_nodes {
                inst.status = InstanceStatus::Failed;
                self.sla_monitor.record_failure(&inst.workflow_id);
            } else {
                inst.status = InstanceStatus::Completed;
                self.sla_monitor.record_success(&inst.workflow_id);
            }

            let duration_secs = (Utc::now() - inst.created_at).num_milliseconds() as f64 / 1000.0;
            self.metrics
                .record_workflow_completed(&inst.workflow_id.0, duration_secs);
            let duration_ms = (Utc::now() - inst.created_at).num_milliseconds();

            // Check SLA.
            self.check_and_record_sla_anomalies(&inst, duration_ms)
                .await;

            // Persist instance metrics for analytics queries.
            if let Some(ref ms) = self.metrics_store {
                let node_durations: HashMap<String, i64> = inst
                    .node_states
                    .iter()
                    .filter_map(|(nid, ns)| {
                        if let (Some(s), Some(e)) = (ns.started_at, ns.ended_at) {
                            Some((nid.clone(), (e - s).num_milliseconds()))
                        } else {
                            None
                        }
                    })
                    .collect();
                let failed_count = inst
                    .node_states
                    .values()
                    .filter(|ns| ns.status == NodeStatus::Failed)
                    .count() as i32;
                let status_str = if has_failed_nodes {
                    "failed"
                } else {
                    "completed"
                };
                let im = InstanceExecutionMetrics {
                    instance_id: inst.id.clone(),
                    workflow_id: inst.workflow_id.clone(),
                    status: status_str.into(),
                    duration_ms,
                    node_count: inst.node_states.len() as i32,
                    failed_node_count: failed_count,
                    started_at: inst.created_at,
                    completed_at: Utc::now(),
                    node_durations,
                };
                if let Ok(permit) = self.metrics_semaphore.clone().try_acquire_owned() {
                    let ms = ms.clone();
                    tokio::spawn(async move {
                        let _permit = permit;
                        if let Err(e) = ms.record_instance_metrics(&im).await {
                            tracing::warn!(error = %e, "failed to persist instance metrics");
                        }
                    });
                } else {
                    tracing::warn!(
                        "metrics write backlog full — skipping instance metrics persistence"
                    );
                }
            }

            // Increment persistent budget cost after successful completion.
            // Sum actual metered cost from node outputs (e.g. AI nodes report cost_usd).
            if let Some(ref bs) = self.budget_store {
                let cost: f64 = inst
                    .node_states
                    .values()
                    .filter_map(|ns| ns.output.as_ref()?.get("cost_usd")?.as_f64())
                    .sum();
                if cost > 0.0 {
                    if let Ok(permit) = self.metrics_semaphore.clone().try_acquire_owned() {
                        let wf_id = inst.workflow_id.0.clone();
                        let bs = bs.clone();
                        tokio::spawn(async move {
                            let _permit = permit;
                            if let Err(e) = bs.increment_cost(&wf_id, cost).await {
                                tracing::warn!(error = %e, "failed to increment budget cost");
                            }
                        });
                    } else {
                        tracing::warn!(
                            "metrics write backlog full — skipping budget cost persistence"
                        );
                    }
                }
            }

            if let Err(e) = self
                .store
                .append_event(DomainEvent::InstanceCompleted(InstanceCompletedEvent {
                    base: BaseEvent::new(inst.id.clone(), inst.version),
                }))
                .await
            {
                error!(error = %e, instance = %inst.id, "failed to persist InstanceCompleted event");
            }
        }

        self.save_instance(&mut inst).await?;
        self.cleanup_instance(&inst);
        Ok(())
    }

    /// Accessor for the store (used by saga, resume, subworkflow modules).
    pub(crate) fn store(&self) -> &Arc<dyn Store> {
        &self.store
    }

    /// Accessor for the bus.
    pub(crate) fn bus(&self) -> &Arc<dyn Bus> {
        &self.bus
    }

    /// Accessor for the pool name.
    pub(crate) fn pool_name(&self) -> &str {
        &self.pool_name
    }

    /// Accessor for the CEL evaluator.
    pub(crate) fn cel(&self) -> &CelEvaluator {
        &self.cel
    }

    /// Accessor for the nodes registry.
    pub(crate) fn get_executor(
        &self,
        plugin_ref: &str,
    ) -> Result<Option<Arc<dyn NodeExecutor>>, OrbflowError> {
        let nodes = self
            .nodes
            .read()
            .map_err(|_| OrbflowError::Internal("node registry lock poisoned".into()))?;
        Ok(nodes.get(plugin_ref).cloned())
    }
}

/// Resolves capability data for an action node from the execution context.
pub(crate) fn resolve_capability_edges_from_context(
    ec: &ExecutionContext,
    wf: &Workflow,
    node: &Node,
) -> Option<HashMap<String, serde_json::Value>> {
    let mut caps = HashMap::new();
    for ce in &wf.capability_edges {
        if ce.target_node_id != node.id {
            continue;
        }
        if let Some(output) = ec.node_outputs.get(&ce.source_node_id) {
            caps.insert(
                ce.target_port_key.clone(),
                serde_json::Value::Object(
                    output.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
                ),
            );
        }
    }
    if caps.is_empty() { None } else { Some(caps) }
}

#[async_trait]
impl Engine for OrbflowEngine {
    async fn create_workflow(&self, wf: &Workflow) -> Result<(), OrbflowError> {
        let mut wf = wf.clone();
        if wf.id.0.is_empty() {
            wf.id = WorkflowId::new(Uuid::new_v4().to_string());
        }
        migrate_legacy_triggers(&mut wf);
        validate_workflow(&wf)?;
        self.validate_capability_types(&wf)?;
        self.validate_required_capabilities(&wf)?;
        let now = Utc::now();
        wf.version = 1;
        if wf.status == DefinitionStatus::Draft {
            wf.status = DefinitionStatus::Active;
        }
        wf.created_at = now;
        wf.updated_at = now;
        self.store.create_workflow(&wf).await
    }

    async fn update_workflow(&self, wf: &Workflow) -> Result<(), OrbflowError> {
        let existing = self.store.get_workflow(&wf.id).await?;
        let mut wf = wf.clone();
        migrate_legacy_triggers(&mut wf);
        validate_workflow(&wf)?;
        self.validate_capability_types(&wf)?;
        self.validate_required_capabilities(&wf)?;

        // Snapshot the current definition before overwriting it.
        let definition = serde_json::to_value(&existing).map_err(|e| {
            OrbflowError::Internal(format!("serialize workflow for version snapshot: {e}"))
        })?;
        let snapshot = orbflow_core::versioning::WorkflowVersion {
            version: existing.version,
            workflow_id: existing.id.clone(),
            definition,
            author: None,
            message: None,
            created_at: existing.updated_at,
        };
        self.store.save_workflow_version(&snapshot).await?;

        wf.version = existing.version + 1;
        wf.created_at = existing.created_at;
        wf.updated_at = Utc::now();
        self.store.update_workflow(&wf).await
    }

    async fn delete_workflow(&self, id: &WorkflowId) -> Result<(), OrbflowError> {
        self.store.delete_workflow(id).await
    }

    async fn get_workflow(&self, id: &WorkflowId) -> Result<Workflow, OrbflowError> {
        self.store.get_workflow(id).await
    }

    async fn list_workflows(
        &self,
        opts: ListOptions,
    ) -> Result<(Vec<Workflow>, i64), OrbflowError> {
        self.store.list_workflows(opts).await
    }

    async fn list_workflow_versions(
        &self,
        id: &WorkflowId,
        opts: ListOptions,
    ) -> Result<(Vec<orbflow_core::versioning::WorkflowVersion>, i64), OrbflowError> {
        self.store.list_workflow_versions(id, opts).await
    }

    async fn get_workflow_version(
        &self,
        id: &WorkflowId,
        version: i32,
    ) -> Result<orbflow_core::versioning::WorkflowVersion, OrbflowError> {
        self.store.get_workflow_version(id, version).await
    }

    async fn start_workflow(
        &self,
        id: &WorkflowId,
        input: HashMap<String, serde_json::Value>,
    ) -> Result<Instance, OrbflowError> {
        let span = tracing::info_span!(
            SPAN_WORKFLOW_EXECUTE,
            workflow_id = %id,
            otel.name = "start_workflow",
        );

        async move {
            let wf = self.store.get_workflow(id).await?;
            self.validate_node_configs(&wf)?;

            // Enforce persistent budget limits before starting.
            if let Some(ref bs) = self.budget_store {
                crate::budget::check_budget_before_start(bs.as_ref(), &id.0).await?;
            }

            let now = Utc::now();
            let mut inst = Instance {
                id: InstanceId::new(Uuid::new_v4().to_string()),
                workflow_id: wf.id.clone(),
                status: InstanceStatus::Running,
                node_states: wf
                    .nodes
                    .iter()
                    .map(|n| {
                        (
                            n.id.clone(),
                            NodeState {
                                node_id: n.id.clone(),
                                status: NodeStatus::Pending,
                                input: None,
                                output: None,
                                parameters: None,
                                error: None,
                                attempt: 0,
                                started_at: None,
                                ended_at: None,
                            },
                        )
                    })
                    .collect(),
                context: ExecutionContext::new(input.clone()),
                saga: None,
                parent_id: None,
                instance_metrics: None,
                workflow_version: Some(wf.version),
                owner_id: None,
                version: 1,
                created_at: now,
                updated_at: now,
            };

            let start_event = DomainEvent::InstanceStarted(InstanceStartedEvent {
                base: BaseEvent::new(inst.id.clone(), inst.version),
                input: input.clone(),
            });

            // Create instance and append start event (separate calls).
            self.store.create_instance(&inst).await?;
            self.metrics.record_workflow_started(&wf.id.0);
            if let Err(e) = self.store.append_event(start_event).await {
                warn!(
                    instance = %inst.id,
                    error = %e,
                    "failed to append instance-started event"
                );
            }

            // Resolve capability nodes first.
            self.resolve_capabilities(&mut inst, &wf).await?;

            // Mark trigger nodes as completed so downstream nodes can be dispatched.
            let has_triggers = self.mark_triggers_completed(&mut inst, &wf, &input);

            // Dispatch initial nodes.
            self.dispatch_initial_nodes(&mut inst, &wf, has_triggers)
                .await?;

            // If all nodes reached a terminal state, mark as completed.
            // Do not overwrite a Failed status set during dispatch.
            if inst.all_nodes_terminal() && inst.status != InstanceStatus::Failed {
                inst.status = InstanceStatus::Completed;
                self.sla_monitor.record_success(&inst.workflow_id);
                let duration_secs =
                    (Utc::now() - inst.created_at).num_milliseconds() as f64 / 1000.0;
                self.metrics
                    .record_workflow_completed(&wf.id.0, duration_secs);
                let duration_ms = (Utc::now() - inst.created_at).num_milliseconds();

                // Check SLA.
                self.check_and_record_sla_anomalies(&inst, duration_ms)
                    .await;

                // Increment persistent budget cost after successful completion.
                // Sum actual metered cost from node outputs (e.g. AI nodes report cost_usd).
                if let Some(ref bs) = self.budget_store {
                    let cost: f64 = inst
                        .node_states
                        .values()
                        .filter_map(|ns| ns.output.as_ref()?.get("cost_usd")?.as_f64())
                        .sum();
                    if cost > 0.0 {
                        if let Ok(permit) = self.metrics_semaphore.clone().try_acquire_owned() {
                            let wf_id = inst.workflow_id.0.clone();
                            let bs = bs.clone();
                            tokio::spawn(async move {
                                let _permit = permit;
                                if let Err(e) = bs.increment_cost(&wf_id, cost).await {
                                    tracing::warn!(error = %e, "failed to increment budget cost");
                                }
                            });
                        } else {
                            tracing::warn!(
                                "metrics write backlog full — skipping budget cost persistence"
                            );
                        }
                    }
                }

                if let Err(e) = self
                    .store
                    .append_event(DomainEvent::InstanceCompleted(InstanceCompletedEvent {
                        base: BaseEvent::new(inst.id.clone(), inst.version),
                    }))
                    .await
                {
                    error!(error = %e, instance = %inst.id, "failed to persist InstanceCompleted event");
                }
            }

            // Persist state after dispatching.
            self.save_instance(&mut inst).await?;

            Ok(inst)
        }
        .instrument(span)
        .await
    }

    async fn get_instance(&self, id: &InstanceId) -> Result<Instance, OrbflowError> {
        self.store.get_instance(id).await
    }

    async fn list_instances(
        &self,
        opts: ListOptions,
    ) -> Result<(Vec<Instance>, i64), OrbflowError> {
        self.store.list_instances(opts).await
    }

    async fn cancel_instance(&self, id: &InstanceId) -> Result<(), OrbflowError> {
        let mu = self.lock_instance(id);
        let _guard = mu.lock().await;

        let mut inst = self.store.get_instance(id).await?;
        if inst.is_terminal() {
            return Err(OrbflowError::InvalidStatus);
        }
        inst.status = InstanceStatus::Cancelled;
        inst.updated_at = Utc::now();

        for ns in inst.node_states.values_mut() {
            if matches!(
                ns.status,
                NodeStatus::Pending
                    | NodeStatus::Queued
                    | NodeStatus::Running
                    | NodeStatus::WaitingApproval
            ) {
                ns.status = NodeStatus::Cancelled;
            }
        }

        self.save_instance(&mut inst).await?;

        if let Err(e) = self
            .store
            .append_event(DomainEvent::InstanceCancelled(InstanceCancelledEvent {
                base: BaseEvent::new(inst.id.clone(), inst.version),
            }))
            .await
        {
            error!(error = %e, instance = %inst.id, "failed to persist InstanceCancelled event");
        }
        Ok(())
    }

    async fn test_node(
        &self,
        workflow_id: &WorkflowId,
        node_id: &str,
        cached_outputs: HashMap<String, HashMap<String, serde_json::Value>>,
        owner_id: Option<&str>,
    ) -> Result<TestNodeResult, OrbflowError> {
        crate::testnode::test_node(self, workflow_id, node_id, cached_outputs, owner_id).await
    }

    fn register_node(
        &self,
        name: &str,
        executor: Arc<dyn NodeExecutor>,
    ) -> Result<(), OrbflowError> {
        if name.trim().is_empty() {
            return Err(OrbflowError::EmptyNodeKind);
        }

        let mut nodes = self
            .nodes
            .write()
            .map_err(|_| OrbflowError::Internal("node registry lock poisoned".into()))?;
        if nodes.contains_key(name) {
            return Err(OrbflowError::DuplicateNodeKind(name.into()));
        }

        // Check if the executor provides a schema.
        // We use a dynamic downcast approach. Since NodeSchemaProvider is a
        // separate trait, we check via a helper trait object.
        // For now, we store the executor and check schemas separately.
        nodes.insert(name.to_owned(), executor.clone());
        drop(nodes);

        // Try to extract schema if the executor implements NodeSchemaProvider.
        // This requires the concrete type to be known. In Rust we handle this
        // through a separate registration or by making NodeExecutor extend
        // NodeSchemaProvider optionally. For now, skip auto-schema detection
        // and require explicit schema registration.
        Ok(())
    }

    fn register_node_with_schema(
        &self,
        name: &str,
        executor: Arc<dyn NodeExecutor>,
        schema: NodeSchema,
    ) -> Result<(), OrbflowError> {
        self.register_node(name, executor)?;
        let mut guard = self.schemas.write().map_err(|_| {
            tracing::error!("schema registry write lock poisoned during register_node_with_schema");
            OrbflowError::Internal("schema registry lock poisoned".into())
        })?;
        guard.insert(name.to_owned(), schema);
        Ok(())
    }

    fn register_schema(&self, name: &str, schema: NodeSchema) {
        match self.schemas.write() {
            Ok(mut guard) => {
                guard.insert(name.to_owned(), schema);
            }
            Err(_) => {
                tracing::error!(
                    node = name,
                    "schema registry write lock poisoned — schema not registered"
                );
            }
        }
    }

    fn unregister_schema(&self, name: &str) {
        match self.schemas.write() {
            Ok(mut guard) => {
                guard.remove(name);
            }
            Err(_) => {
                tracing::error!(
                    node = name,
                    "schema registry write lock poisoned — schema not removed"
                );
            }
        }
    }

    fn node_schemas(&self) -> Vec<NodeSchema> {
        match self.schemas.read() {
            Ok(guard) => guard.values().cloned().collect(),
            Err(_) => {
                tracing::error!("schema registry read lock poisoned — returning empty schemas");
                Vec::new()
            }
        }
    }

    fn node_schema_refs(&self) -> Vec<String> {
        match self.schemas.read() {
            Ok(guard) => guard.keys().cloned().collect(),
            Err(_) => {
                tracing::error!("schema registry read lock poisoned — returning empty refs");
                Vec::new()
            }
        }
    }

    async fn verify_audit_chain(
        &self,
        instance_id: &orbflow_core::InstanceId,
    ) -> Result<(bool, usize, Option<String>), OrbflowError> {
        let records = self.store.load_audit_records(instance_id).await?;
        let count = records.len();
        match orbflow_core::audit::verify_chain(&records) {
            Ok(()) => Ok((true, count, None)),
            Err(e) => Ok((false, count, Some(e.to_string()))),
        }
    }

    async fn load_audit_records(
        &self,
        instance_id: &orbflow_core::InstanceId,
    ) -> Result<Vec<orbflow_core::audit::AuditRecord>, OrbflowError> {
        // Verify instance exists first.
        let _ = self.store.get_instance(instance_id).await?;
        self.store.load_audit_records(instance_id).await
    }

    async fn start(&self) -> Result<(), OrbflowError> {
        let result_subject = result_subject(&self.pool_name);
        let engine: Arc<OrbflowEngine> =
            self.self_ref
                .get()
                .and_then(|w| w.upgrade())
                .ok_or_else(|| {
                    OrbflowError::Internal(
                        "engine self-ref not set; call set_self_ref() after wrapping in Arc".into(),
                    )
                })?;
        let handler: MsgHandler = Arc::new(move |subject, data| {
            let eng = engine.clone();
            Box::pin(async move { eng.handle_result_message(subject, data).await })
        });
        self.bus.subscribe(&result_subject, handler).await?;

        info!(pool = self.pool_name.as_str(), "engine started");

        // Resume running instances.
        crate::resume::resume_running(self).await?;

        // Wait for stop signal.
        self.stop_notify.notified().await;
        Ok(())
    }

    async fn approve_node(
        &self,
        instance_id: &InstanceId,
        node_id: &str,
        approved_by: Option<String>,
    ) -> Result<(), OrbflowError> {
        let mu = self.lock_instance(instance_id);
        let _guard = mu.lock().await;

        let mut inst = self.store.get_instance(instance_id).await?;

        // Reject if instance is already terminal (e.g. cancelled concurrently).
        if inst.is_terminal() {
            return Err(OrbflowError::InvalidStatus);
        }

        let node_state = inst
            .node_states
            .get(node_id)
            .ok_or(OrbflowError::NotFound)?;

        if node_state.status != NodeStatus::WaitingApproval {
            return Err(OrbflowError::InvalidStatus);
        }

        // Record the approval event.
        let event = DomainEvent::NodeApproved(orbflow_core::event::NodeApprovedEvent {
            base: orbflow_core::event::BaseEvent::new(instance_id.clone(), inst.version + 1),
            node_id: node_id.to_string(),
            approved_by,
        });
        if let Err(e) = self.store.append_event(event).await {
            error!(error = %e, instance = %instance_id, node = %node_id, "failed to persist approval event");
        }

        // Fetch the workflow to get the node definition for dispatch.
        let wf = self.store.get_workflow(&inst.workflow_id).await?;
        let node = wf
            .nodes
            .iter()
            .find(|n| n.id == node_id)
            .ok_or(OrbflowError::NotFound)?;

        info!(instance = %instance_id, node = %node_id, "node approved — dispatching");

        // Transition to Queued so dispatch_node's approval gate is bypassed.
        let ns = inst
            .node_states
            .get_mut(node_id)
            .ok_or(OrbflowError::NotFound)?;
        ns.status = NodeStatus::Queued;

        // Dispatch the node for execution.
        self.dispatch_node(&mut inst, &wf, node).await?;
        self.save_instance(&mut inst).await?;

        Ok(())
    }

    async fn reject_node(
        &self,
        instance_id: &InstanceId,
        node_id: &str,
        reason: Option<String>,
        rejected_by: Option<String>,
    ) -> Result<(), OrbflowError> {
        let mu = self.lock_instance(instance_id);
        let _guard = mu.lock().await;

        let mut inst = self.store.get_instance(instance_id).await?;

        // Reject if instance is already terminal.
        if inst.is_terminal() {
            return Err(OrbflowError::InvalidStatus);
        }

        let node_state = inst
            .node_states
            .get(node_id)
            .ok_or(OrbflowError::NotFound)?;

        if node_state.status != NodeStatus::WaitingApproval {
            return Err(OrbflowError::InvalidStatus);
        }

        // Record the rejection event.
        let event = DomainEvent::NodeRejected(orbflow_core::event::NodeRejectedEvent {
            base: orbflow_core::event::BaseEvent::new(instance_id.clone(), inst.version + 1),
            node_id: node_id.to_string(),
            reason: reason.clone(),
            rejected_by,
        });
        if let Err(e) = self.store.append_event(event).await {
            error!(error = %e, instance = %instance_id, node = %node_id, "failed to persist rejection event");
        }

        // Mark node as failed with the rejection reason.
        let rejection_msg = reason.unwrap_or_else(|| "rejected by reviewer".into());
        let ns = inst
            .node_states
            .get_mut(node_id)
            .ok_or(OrbflowError::NotFound)?;
        ns.status = NodeStatus::Failed;
        ns.error = Some(rejection_msg.clone());
        ns.ended_at = Some(Utc::now());

        // Fail the instance — consistent with normal node failure handling.
        inst.status = InstanceStatus::Failed;
        if let Err(e) = self
            .store
            .append_event(DomainEvent::InstanceFailed(InstanceFailedEvent {
                base: BaseEvent::new(instance_id.clone(), inst.version + 1),
                error: format!("node {node_id} rejected: {rejection_msg}"),
            }))
            .await
        {
            error!(error = %e, instance = %instance_id, "failed to persist instance-failed event after rejection");
        }

        self.save_instance(&mut inst).await?;

        info!(instance = %instance_id, node = %node_id, "node rejected — instance failed");

        self.cleanup_instance(&inst);
        Ok(())
    }

    async fn stop(&self) -> Result<(), OrbflowError> {
        self.stop_notify.notify_one();
        info!("engine stopped");
        Ok(())
    }
}
