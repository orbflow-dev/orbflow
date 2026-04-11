// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! HTTP route handlers for the Orbflow REST API.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use orbflow_core::analytics::TimeRange;
use orbflow_core::rbac::Permission;
use orbflow_core::streaming::StreamMessage;
use orbflow_core::{
    AccountBudget, AlertRule, AlertStore, AnalyticsStore, BudgetPeriod, BudgetStore, Bus,
    CREDENTIAL_SCHEMAS, ChangeRequestStore, CredentialStore, DEFAULT_PAGE_SIZE, Engine,
    ListOptions, MetricsStore, MsgHandler, OrbflowError, RbacStore, WorkflowId, stream_subject,
    validate_plugin_name,
};
use serde::Deserialize;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{debug, error};
use uuid::Uuid;

use crate::errors::{write_data, write_error, write_list, write_safe_error};
use crate::middleware::{AuthUser, StartRateLimiter, check_permission};

/// Shared application state for all handlers.
#[derive(Clone)]
pub struct AppState {
    pub engine: Arc<dyn Engine>,
    pub credential_store: Option<Arc<dyn CredentialStore>>,
    /// Optional bus for SSE streaming endpoints.
    pub bus: Option<Arc<dyn Bus>>,
    pub rate_limiter: StartRateLimiter,
    /// Optional metrics store for workflow/instance metrics endpoints.
    pub metrics_store: Option<Arc<dyn MetricsStore>>,
    /// Optional RBAC policy for permission enforcement.
    pub rbac: Option<Arc<RwLock<orbflow_core::rbac::RbacPolicy>>>,
    /// Optional local plugin index for the marketplace.
    pub plugin_index: Option<Arc<dyn orbflow_core::PluginIndex>>,
    /// Optional plugin installer for marketplace install/uninstall.
    pub plugin_installer: Option<Arc<dyn orbflow_core::PluginInstaller>>,
    /// Optional change request store for PR-style collaboration.
    pub change_request_store: Option<Arc<dyn ChangeRequestStore>>,
    /// Optional RBAC store for persisting policy changes to the database.
    pub rbac_store: Option<Arc<dyn RbacStore>>,
    /// Optional budget store for cost tracking and budget enforcement.
    pub budget_store: Option<Arc<dyn BudgetStore>>,
    /// Optional analytics store for aggregated execution statistics.
    pub analytics_store: Option<Arc<dyn AnalyticsStore>>,
    /// Optional alert store for alert rule CRUD.
    pub alert_store: Option<Arc<dyn AlertStore>>,
    /// Whether to trust the `X-User-Id` header for caller identity.
    /// When `false`, all requests are attributed to `"anonymous"`.
    pub trust_x_user_id: bool,
    /// Bootstrap admin user ID (from `ORBFLOW_BOOTSTRAP_ADMIN` env var, read once at startup).
    pub bootstrap_admin: Option<String>,
    /// Optional plugin process manager for starting/stopping plugins via API.
    pub plugin_manager: Option<Arc<dyn orbflow_core::PluginManager>>,
    /// Optional path to the plugins directory for plugin installation.
    pub plugins_dir: Option<String>,
    /// Shared HTTP client for plugin downloads (connection pool reuse).
    pub http_client: reqwest::Client,
}

// --- Pagination ---

/// Query parameters for paginated list endpoints.
#[derive(Debug, Deserialize)]
pub struct PaginationParams {
    #[serde(default)]
    pub offset: Option<i64>,
    #[serde(default)]
    pub limit: Option<i64>,
}

impl PaginationParams {
    fn to_list_options(&self) -> ListOptions {
        let offset = self.offset.unwrap_or(0).max(0);
        let mut limit = self.limit.unwrap_or(DEFAULT_PAGE_SIZE);
        if limit <= 0 {
            limit = DEFAULT_PAGE_SIZE;
        }
        if limit > 100 {
            limit = 100;
        }
        ListOptions { offset, limit }
    }
}

// --- Health ---

pub async fn health_check() -> Response {
    (StatusCode::OK, Json(serde_json::json!({"status": "ok"}))).into_response()
}

// --- Node Types ---

pub async fn list_node_types(State(state): State<AppState>) -> Response {
    let schemas = state.engine.node_schemas();
    write_data(StatusCode::OK, schemas)
}

// --- Credential Types ---

pub async fn list_credential_types() -> Response {
    let schemas: &Vec<orbflow_core::CredentialTypeSchema> = &CREDENTIAL_SCHEMAS;
    write_data(StatusCode::OK, schemas)
}

// --- Workflows ---

pub async fn create_workflow(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Json(mut wf): Json<orbflow_core::Workflow>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::Edit,
        "*",
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    // Generate server-side fields when the client omits them.
    if wf.id.0.is_empty() {
        wf.id = WorkflowId::new(Uuid::new_v4().to_string());
    }
    let now = Utc::now();
    wf.created_at = now;
    wf.updated_at = now;

    match state.engine.create_workflow(&wf).await {
        Ok(()) => write_data(StatusCode::CREATED, wf),
        Err(e) => {
            if e.is_validation_error() {
                return write_error(StatusCode::BAD_REQUEST, e.to_string());
            }
            error!(error = %e, "failed to create workflow");
            write_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to create workflow",
            )
        }
    }
}

pub async fn list_workflows(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Query(params): Query<PaginationParams>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::View,
        "*",
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let opts = params.to_list_options();
    match state.engine.list_workflows(opts.clone()).await {
        Ok((workflows, total)) => write_list(workflows, total, opts.offset, opts.limit),
        Err(e) => {
            error!(error = %e, "failed to list workflows");
            write_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to list workflows",
            )
        }
    }
}

pub async fn get_workflow(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Path(id): Path<String>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::View,
        &id,
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let wf_id = orbflow_core::WorkflowId::new(id);
    match state.engine.get_workflow(&wf_id).await {
        Ok(wf) => write_data(StatusCode::OK, wf),
        Err(OrbflowError::NotFound) => write_error(StatusCode::NOT_FOUND, "workflow not found"),
        Err(e) => {
            error!(error = %e, "failed to get workflow");
            write_error(StatusCode::INTERNAL_SERVER_ERROR, "failed to get workflow")
        }
    }
}

pub async fn update_workflow(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Path(id): Path<String>,
    Json(mut wf): Json<orbflow_core::Workflow>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::Edit,
        &id,
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    wf.id = orbflow_core::WorkflowId::new(id);
    match state.engine.update_workflow(&wf).await {
        Ok(()) => write_data(StatusCode::OK, wf),
        Err(OrbflowError::NotFound) => write_error(StatusCode::NOT_FOUND, "workflow not found"),
        Err(OrbflowError::Conflict) => {
            write_error(StatusCode::CONFLICT, "version conflict — reload and retry")
        }
        Err(e) => {
            if e.is_validation_error() {
                return write_error(StatusCode::BAD_REQUEST, e.to_string());
            }
            error!(error = %e, "failed to update workflow");
            write_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to update workflow",
            )
        }
    }
}

pub async fn delete_workflow(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Path(id): Path<String>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::Delete,
        &id,
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let wf_id = orbflow_core::WorkflowId::new(id);
    match state.engine.delete_workflow(&wf_id).await {
        Ok(()) => write_data(StatusCode::OK, serde_json::json!({ "deleted": true })),
        Err(OrbflowError::NotFound) => write_error(StatusCode::NOT_FOUND, "workflow not found"),
        Err(e) => {
            error!(error = %e, "failed to delete workflow");
            write_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to delete workflow",
            )
        }
    }
}

pub async fn start_workflow(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Path(id): Path<String>,
    Json(input): Json<HashMap<String, serde_json::Value>>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::Execute,
        &id,
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    // Per-workflow rate limiting.
    if let Err(resp) = state.rate_limiter.check(&id) {
        return resp;
    }

    let wf_id = orbflow_core::WorkflowId::new(id);
    match state.engine.start_workflow(&wf_id, input).await {
        Ok(inst) => write_data(StatusCode::OK, inst),
        Err(OrbflowError::NotFound) => write_error(StatusCode::NOT_FOUND, "workflow not found"),
        Err(e) => {
            if e.is_validation_error() {
                return write_error(StatusCode::BAD_REQUEST, e.to_string());
            }
            error!(error = %e, "failed to start workflow");
            write_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to start workflow",
            )
        }
    }
}

/// Request body for the test-node endpoint.
#[derive(Debug, Deserialize)]
pub struct TestNodeRequest {
    pub node_id: String,
    #[serde(default)]
    pub cached_outputs: HashMap<String, HashMap<String, serde_json::Value>>,
}

pub async fn test_node(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Path(id): Path<String>,
    Json(req): Json<TestNodeRequest>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::Execute,
        &id,
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    if req.node_id.is_empty() {
        return write_error(StatusCode::BAD_REQUEST, "node_id is required");
    }

    // Per-workflow rate limiting (same policy as start_workflow).
    if let Err(resp) = state.rate_limiter.check(&id) {
        return resp;
    }

    let wf_id = orbflow_core::WorkflowId::new(id);
    match state
        .engine
        .test_node(
            &wf_id,
            &req.node_id,
            req.cached_outputs,
            Some(&auth_user.user_id),
        )
        .await
    {
        Ok(result) => write_data(StatusCode::OK, result),
        Err(OrbflowError::NotFound) => write_error(StatusCode::NOT_FOUND, "workflow not found"),
        Err(OrbflowError::NodeNotFound) => {
            write_error(StatusCode::UNPROCESSABLE_ENTITY, "node executor not found")
        }
        Err(e) => {
            error!(error = %e, "failed to test node");
            write_error(StatusCode::INTERNAL_SERVER_ERROR, "failed to test node")
        }
    }
}

// --- Instances ---

pub async fn list_instances(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Query(params): Query<PaginationParams>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::View,
        "*",
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let opts = params.to_list_options();
    match state.engine.list_instances(opts.clone()).await {
        Ok((instances, total)) => write_list(instances, total, opts.offset, opts.limit),
        Err(e) => {
            error!(error = %e, "failed to list instances");
            write_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to list instances",
            )
        }
    }
}

pub async fn get_instance(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Path(id): Path<String>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::View,
        "*",
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let inst_id = orbflow_core::InstanceId::new(id);
    match state.engine.get_instance(&inst_id).await {
        Ok(inst) => write_data(StatusCode::OK, inst),
        Err(OrbflowError::NotFound) => write_error(StatusCode::NOT_FOUND, "instance not found"),
        Err(e) => {
            error!(error = %e, "failed to get instance");
            write_error(StatusCode::INTERNAL_SERVER_ERROR, "failed to get instance")
        }
    }
}

pub async fn cancel_instance(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Path(id): Path<String>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::Execute,
        "*",
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let inst_id = orbflow_core::InstanceId::new(id);
    match state.engine.cancel_instance(&inst_id).await {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({"data": {"status": "cancelled"}})),
        )
            .into_response(),
        Err(OrbflowError::NotFound) => write_error(StatusCode::NOT_FOUND, "instance not found"),
        Err(e) => {
            error!(error = %e, "failed to cancel instance");
            write_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to cancel instance",
            )
        }
    }
}

// --- Credentials ---

pub async fn create_credential(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Json(req): Json<orbflow_core::CreateCredentialRequest>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::ManageCredentials,
        "*",
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let store = match &state.credential_store {
        Some(s) => s,
        None => return write_error(StatusCode::NOT_FOUND, "credentials API not enabled"),
    };

    let mut cred = match req.into_credential() {
        Ok(c) => c,
        Err(e) => return write_safe_error(&e),
    };
    cred.owner_id = Some(auth_user.user_id.clone());
    match store.create_credential(&cred).await {
        Ok(()) => write_data(
            StatusCode::CREATED,
            orbflow_core::CredentialSummary::from(&cred),
        ),
        Err(OrbflowError::AlreadyExists) => {
            write_error(StatusCode::CONFLICT, "credential already exists")
        }
        Err(e) => {
            error!(error = %e, "failed to create credential");
            write_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to create credential",
            )
        }
    }
}

pub async fn list_credentials(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::ManageCredentials,
        "*",
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let store = match &state.credential_store {
        Some(s) => s,
        None => return write_error(StatusCode::NOT_FOUND, "credentials API not enabled"),
    };

    match store.list_credentials_for_owner(Some(&auth_user.user_id)).await {
        Ok(creds) => write_data(StatusCode::OK, creds),
        Err(e) => {
            error!(error = %e, "failed to list credentials");
            write_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to list credentials",
            )
        }
    }
}

pub async fn get_credential(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Path(id): Path<String>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::ManageCredentials,
        "*",
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let store = match &state.credential_store {
        Some(s) => s,
        None => return write_error(StatusCode::NOT_FOUND, "credentials API not enabled"),
    };

    let cred_id = match orbflow_core::CredentialId::new(id) {
        Ok(id) => id,
        Err(e) => return write_safe_error(&e),
    };
    match store
        .get_credential_for_owner(&cred_id, Some(&auth_user.user_id))
        .await
    {
        Ok(cred) => {
            let summary = orbflow_core::CredentialSummary::from(&cred);
            write_data(StatusCode::OK, summary)
        }
        Err(OrbflowError::NotFound) => write_error(StatusCode::NOT_FOUND, "credential not found"),
        Err(e) => {
            error!(error = %e, "failed to get credential");
            write_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to get credential",
            )
        }
    }
}

pub async fn update_credential(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Path(id): Path<String>,
    Json(req): Json<orbflow_core::CreateCredentialRequest>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::ManageCredentials,
        "*",
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let store = match &state.credential_store {
        Some(s) => s,
        None => return write_error(StatusCode::NOT_FOUND, "credentials API not enabled"),
    };

    let mut cred = match req.into_credential() {
        Ok(c) => c,
        Err(e) => return write_safe_error(&e),
    };
    cred.id = match orbflow_core::CredentialId::new(id) {
        Ok(id) => id,
        Err(e) => return write_safe_error(&e),
    };
    // Verify the caller owns this credential before allowing update,
    // and capture the existing record to preserve created_at.
    let existing = match store
        .get_credential_for_owner(&cred.id, Some(&auth_user.user_id))
        .await
    {
        Ok(c) => c,
        Err(e) => {
            return match e {
                OrbflowError::NotFound => {
                    write_error(StatusCode::NOT_FOUND, "credential not found")
                }
                other => {
                    error!(error = %other, "ownership check failed for credential update");
                    write_error(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "failed to update credential",
                    )
                }
            };
        }
    };
    cred.owner_id = Some(auth_user.user_id.clone());
    cred.created_at = existing.created_at;
    // Preserve access tier and policy from existing credential — tier changes
    // require a separate admin-level operation, not a standard credential update.
    cred.access_tier = existing.access_tier;
    cred.policy = existing.policy;
    match store.update_credential(&cred).await {
        Ok(()) => write_data(StatusCode::OK, orbflow_core::CredentialSummary::from(&cred)),
        Err(OrbflowError::NotFound) => write_error(StatusCode::NOT_FOUND, "credential not found"),
        Err(e) => {
            error!(error = %e, "failed to update credential");
            write_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to update credential",
            )
        }
    }
}

pub async fn delete_credential(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Path(id): Path<String>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::ManageCredentials,
        "*",
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let store = match &state.credential_store {
        Some(s) => s,
        None => return write_error(StatusCode::NOT_FOUND, "credentials API not enabled"),
    };

    let cred_id = match orbflow_core::CredentialId::new(id) {
        Ok(id) => id,
        Err(e) => return write_safe_error(&e).into_response(),
    };
    match store.delete_credential(&cred_id, Some(&auth_user.user_id)).await {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({"data": {"status": "deleted"}})),
        )
            .into_response(),
        Err(OrbflowError::NotFound) => write_error(StatusCode::NOT_FOUND, "credential not found"),
        Err(e) => {
            error!(error = %e, "failed to delete credential");
            write_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to delete credential",
            )
        }
    }
}

// --- Approval Gates ---

/// Path parameters for approval endpoints.
#[derive(Deserialize)]
pub struct ApprovalPathParams {
    instance_id: String,
    node_id: String,
}

/// Request body for approving a node.
#[derive(Deserialize)]
pub struct ApproveRequest {
    #[serde(default)]
    pub approved_by: Option<String>,
}

/// Request body for rejecting a node.
#[derive(Deserialize)]
pub struct RejectRequest {
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub rejected_by: Option<String>,
}

/// `POST /instances/{instance_id}/nodes/{node_id}/approve`
pub async fn approve_node(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Path(params): Path<ApprovalPathParams>,
    Json(_body): Json<ApproveRequest>,
) -> Response {
    // Fetch the instance to get the workflow_id for proper RBAC scoping.
    let instance_id = orbflow_core::InstanceId::new(&params.instance_id);
    let instance = match state.engine.get_instance(&instance_id).await {
        Ok(inst) => inst,
        Err(OrbflowError::NotFound) => {
            return write_error(StatusCode::NOT_FOUND, "instance not found");
        }
        Err(e) => {
            error!(error = %e, "failed to get instance for approval");
            return write_error(StatusCode::INTERNAL_SERVER_ERROR, "failed to get instance");
        }
    };

    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::Approve,
        &instance.workflow_id.0,
        Some(&params.node_id),
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    // Use the authenticated user's identity — ignore caller-supplied approved_by (L4).
    let approved_by = Some(auth_user.user_id.clone());
    match state
        .engine
        .approve_node(&instance_id, &params.node_id, approved_by)
        .await
    {
        Ok(()) => write_data(StatusCode::OK, serde_json::json!({"status": "approved"})),
        Err(OrbflowError::NotFound) => {
            write_error(StatusCode::NOT_FOUND, "instance or node not found")
        }
        Err(e) if e.is_validation_error() => write_error(StatusCode::BAD_REQUEST, e.to_string()),
        Err(e) => {
            error!(error = %e, "approve_node failed");
            write_error(StatusCode::INTERNAL_SERVER_ERROR, "failed to approve node")
        }
    }
}

/// `POST /instances/{instance_id}/nodes/{node_id}/reject`
pub async fn reject_node(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Path(params): Path<ApprovalPathParams>,
    Json(body): Json<RejectRequest>,
) -> Response {
    // Fetch the instance to get the workflow_id for proper RBAC scoping.
    let instance_id = orbflow_core::InstanceId::new(&params.instance_id);
    let instance = match state.engine.get_instance(&instance_id).await {
        Ok(inst) => inst,
        Err(OrbflowError::NotFound) => {
            return write_error(StatusCode::NOT_FOUND, "instance not found");
        }
        Err(e) => {
            error!(error = %e, "failed to get instance for rejection");
            return write_error(StatusCode::INTERNAL_SERVER_ERROR, "failed to get instance");
        }
    };

    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::Approve,
        &instance.workflow_id.0,
        Some(&params.node_id),
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    // Use the authenticated user's identity — ignore caller-supplied rejected_by (L4).
    let rejected_by = Some(auth_user.user_id.clone());
    match state
        .engine
        .reject_node(&instance_id, &params.node_id, body.reason, rejected_by)
        .await
    {
        Ok(()) => write_data(StatusCode::OK, serde_json::json!({"status": "rejected"})),
        Err(OrbflowError::NotFound) => {
            write_error(StatusCode::NOT_FOUND, "instance or node not found")
        }
        Err(e) if e.is_validation_error() => write_error(StatusCode::BAD_REQUEST, e.to_string()),
        Err(e) => {
            error!(error = %e, "reject_node failed");
            write_error(StatusCode::INTERNAL_SERVER_ERROR, "failed to reject node")
        }
    }
}

// --- Streaming SSE ---

/// Path parameters for the streaming endpoint.
#[derive(Deserialize)]
pub struct StreamPathParams {
    instance_id: String,
    node_id: String,
}

/// SSE endpoint that streams node execution chunks in real-time.
///
/// `GET /instances/{instance_id}/nodes/{node_id}/stream`
///
/// The client receives Server-Sent Events with JSON-encoded [`StreamMessage`]s.
/// The stream ends when a `done` or `error` chunk is received.
pub async fn stream_node(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Path(params): Path<StreamPathParams>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::View,
        "*",
        Some(&params.node_id),
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let bus = match &state.bus {
        Some(b) => Arc::clone(b),
        None => {
            return write_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "streaming not available (no bus configured)",
            );
        }
    };

    // Verify the instance exists before subscribing to prevent IDOR.
    let inst_id = orbflow_core::InstanceId::new(&params.instance_id);
    match state.engine.get_instance(&inst_id).await {
        Ok(_) => {}
        Err(OrbflowError::NotFound) => {
            return write_error(StatusCode::NOT_FOUND, "instance not found");
        }
        Err(e) => {
            error!(error = %e, "failed to verify instance for stream");
            return write_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to verify instance",
            );
        }
    }

    let subject = stream_subject(&params.instance_id, &params.node_id);
    let (tx, rx) = tokio::sync::mpsc::channel::<Vec<u8>>(64);

    // Subscribe to the stream subject and relay messages to the channel.
    // When the SSE client disconnects, tx.send will fail. We return an error
    // containing "stream closed" so the NATS subscription loop can detect the
    // disconnect and self-terminate (preventing leaked background tasks).
    let handler: MsgHandler = Arc::new(move |_subject, data| {
        let tx = tx.clone();
        Box::pin(async move {
            tx.send(data)
                .await
                .map_err(|_| OrbflowError::Bus("stream closed".into()))
        })
    });

    if let Err(e) = bus.subscribe(&subject, handler).await {
        debug!(error = %e, "failed to subscribe to stream subject");
        return write_error(StatusCode::INTERNAL_SERVER_ERROR, "failed to start stream");
    }

    // Convert the mpsc receiver into an SSE stream.
    let stream = ReceiverStream::new(rx).filter_map(|data| {
        // Parse as StreamMessage, validate wire version, then send raw JSON.
        let msg = match serde_json::from_slice::<StreamMessage>(&data) {
            Ok(msg) => msg,
            Err(_) => {
                let json = String::from_utf8_lossy(&data).into_owned();
                return Some(Ok::<_, std::convert::Infallible>(
                    Event::default().event("data").data(json),
                ));
            }
        };
        // Reject messages from newer wire versions (same guard as TaskMessage/ResultMessage).
        if msg.v > orbflow_core::WIRE_VERSION {
            tracing::warn!(
                v = msg.v,
                current = orbflow_core::WIRE_VERSION,
                "ignoring stream message with unknown wire version"
            );
            return None;
        }
        let event_type = match msg.chunk {
            orbflow_core::streaming::StreamChunk::Data { .. } => "data",
            orbflow_core::streaming::StreamChunk::Done { .. } => "done",
            orbflow_core::streaming::StreamChunk::Error { .. } => "error",
        };
        let json = String::from_utf8_lossy(&data).into_owned();
        Some(Ok(Event::default().event(event_type).data(json)))
    });

    Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}

// --- Workflow Versions ---

/// Query parameters for the diff endpoint.
#[derive(Debug, Deserialize)]
pub struct DiffParams {
    pub from: i32,
    pub to: i32,
}

/// `GET /workflows/{id}/versions`
pub async fn list_workflow_versions(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Path(id): Path<String>,
    Query(params): Query<PaginationParams>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::View,
        &id,
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let wf_id = orbflow_core::WorkflowId::new(id);
    let opts = params.to_list_options();
    match state
        .engine
        .list_workflow_versions(&wf_id, opts.clone())
        .await
    {
        Ok((versions, total)) => write_list(versions, total, opts.offset, opts.limit),
        Err(OrbflowError::NotFound) => write_error(StatusCode::NOT_FOUND, "workflow not found"),
        Err(e) => {
            error!(error = %e, "failed to list workflow versions");
            write_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to list workflow versions",
            )
        }
    }
}

/// `GET /workflows/{id}/versions/{version}`
pub async fn get_workflow_version(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Path((id, version)): Path<(String, i32)>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::View,
        &id,
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let wf_id = orbflow_core::WorkflowId::new(id);
    match state.engine.get_workflow_version(&wf_id, version).await {
        Ok(ver) => write_data(StatusCode::OK, ver),
        Err(OrbflowError::NotFound) => write_error(StatusCode::NOT_FOUND, "version not found"),
        Err(e) => {
            error!(error = %e, "failed to get workflow version");
            write_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to get workflow version",
            )
        }
    }
}

/// `GET /workflows/{id}/diff?from=X&to=Y`
pub async fn diff_workflow_versions(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Path(id): Path<String>,
    Query(params): Query<DiffParams>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::View,
        &id,
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let wf_id = orbflow_core::WorkflowId::new(id);

    // Fetch both versions.
    let from_ver = match state.engine.get_workflow_version(&wf_id, params.from).await {
        Ok(v) => v,
        Err(OrbflowError::NotFound) => {
            return write_error(
                StatusCode::NOT_FOUND,
                format!("version {} not found", params.from),
            );
        }
        Err(e) => {
            error!(error = %e, "failed to get from-version for diff");
            return write_error(StatusCode::INTERNAL_SERVER_ERROR, "failed to compute diff");
        }
    };

    let to_ver = match state.engine.get_workflow_version(&wf_id, params.to).await {
        Ok(v) => v,
        Err(OrbflowError::NotFound) => {
            return write_error(
                StatusCode::NOT_FOUND,
                format!("version {} not found", params.to),
            );
        }
        Err(e) => {
            error!(error = %e, "failed to get to-version for diff");
            return write_error(StatusCode::INTERNAL_SERVER_ERROR, "failed to compute diff");
        }
    };

    let diff = orbflow_core::versioning::compute_diff(
        &from_ver.definition,
        &to_ver.definition,
        params.from,
        params.to,
    );

    write_data(StatusCode::OK, diff)
}

// --- Metrics ---

/// Query params for metrics endpoints.
#[derive(Debug, Deserialize)]
pub struct MetricsQueryParams {
    /// ISO 8601 datetime string for the start of the metrics window.
    /// Defaults to 24 hours ago if not specified.
    #[allow(dead_code)]
    pub since: Option<String>,
}

/// `GET /workflows/{id}/metrics`
pub async fn get_workflow_metrics(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Path(id): Path<String>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::View,
        &id,
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let metrics_store = match &state.metrics_store {
        Some(s) => s,
        None => return write_error(StatusCode::NOT_IMPLEMENTED, "metrics store not configured"),
    };

    let since = Utc::now() - chrono::Duration::hours(24);
    let wf_id = WorkflowId::new(id);

    match metrics_store.get_workflow_metrics(&wf_id, since).await {
        Ok(summary) => write_data(StatusCode::OK, &summary),
        Err(e) => write_safe_error(&e),
    }
}

/// `GET /workflows/{id}/metrics/nodes`
pub async fn get_workflow_node_metrics(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Path(id): Path<String>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::View,
        &id,
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let metrics_store = match &state.metrics_store {
        Some(s) => s,
        None => return write_error(StatusCode::NOT_IMPLEMENTED, "metrics store not configured"),
    };

    let since = Utc::now() - chrono::Duration::hours(24);
    let wf_id = WorkflowId::new(id);

    match metrics_store.get_node_metrics(&wf_id, since).await {
        Ok(nodes) => write_data(StatusCode::OK, &nodes),
        Err(e) => write_safe_error(&e),
    }
}

/// `GET /instances/{id}/metrics`
pub async fn get_instance_metrics(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Path(id): Path<String>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::View,
        "*",
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let metrics_store = match &state.metrics_store {
        Some(s) => s,
        None => return write_error(StatusCode::NOT_IMPLEMENTED, "metrics store not configured"),
    };

    let inst_id = orbflow_core::InstanceId::new(id);

    match metrics_store.get_instance_metrics(&inst_id).await {
        Ok(Some(metrics)) => write_data(StatusCode::OK, &metrics),
        Ok(None) => write_error(StatusCode::NOT_FOUND, "no metrics found for this instance"),
        Err(e) => write_safe_error(&e),
    }
}

// --- Audit ---

/// `GET /instances/{id}/audit/verify`
pub async fn verify_instance_audit(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Path(id): Path<String>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::View,
        "*",
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let inst_id = orbflow_core::InstanceId::new(id);
    match state.engine.verify_audit_chain(&inst_id).await {
        Ok((valid, event_count, error)) => write_data(
            StatusCode::OK,
            serde_json::json!({
                "valid": valid,
                "event_count": event_count,
                "error": error,
            }),
        ),
        Err(OrbflowError::NotFound) => write_error(StatusCode::NOT_FOUND, "instance not found"),
        Err(e) => {
            error!(error = %e, "failed to verify audit chain");
            write_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to verify audit chain",
            )
        }
    }
}

/// `GET /instances/{id}/audit/trail`
///
/// Returns the full audit trail (event records with hashes) for an instance.
pub async fn get_audit_trail(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Path(id): Path<String>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::View,
        "*",
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let inst_id = orbflow_core::InstanceId::new(id);
    match state.engine.load_audit_records(&inst_id).await {
        Ok(records) => write_data(StatusCode::OK, &records),
        Err(OrbflowError::NotFound) => write_error(StatusCode::NOT_FOUND, "instance not found"),
        Err(e) => {
            error!(error = %e, "failed to load audit trail");
            write_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to load audit trail",
            )
        }
    }
}

/// `GET /instances/{id}/audit/proof/{event_index}`
///
/// Returns a Merkle inclusion proof for the event at the given index.
pub async fn get_audit_proof(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Path((id, event_index)): Path<(String, usize)>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::View,
        "*",
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let inst_id = orbflow_core::InstanceId::new(id);
    let records = match state.engine.load_audit_records(&inst_id).await {
        Ok(r) => r,
        Err(OrbflowError::NotFound) => {
            return write_error(StatusCode::NOT_FOUND, "instance not found");
        }
        Err(e) => {
            error!(error = %e, "failed to load audit records for proof");
            return write_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to load audit records",
            );
        }
    };

    if event_index >= records.len() {
        return write_error(
            StatusCode::BAD_REQUEST,
            format!(
                "event_index {} out of range (0..{})",
                event_index,
                records.len()
            ),
        );
    }

    let hashes: Vec<String> = records.iter().map(|r| r.event_hash.clone()).collect();
    let tree = orbflow_core::audit::MerkleTree::build(&hashes);
    let proof = tree.proof(event_index);
    let root = tree.root().to_string();
    let leaf = hashes[event_index].clone();
    let valid = orbflow_core::audit::MerkleTree::verify_proof(&leaf, &proof, &root);

    write_data(
        StatusCode::OK,
        serde_json::json!({
            "event_index": event_index,
            "leaf_hash": leaf,
            "merkle_root": root,
            "proof": proof,
            "valid": valid,
        }),
    )
}

/// Query parameters for the audit export endpoint.
#[derive(Debug, Deserialize)]
pub struct AuditExportParams {
    /// Compliance format: `soc2`, `hipaa`, or `pci`.
    pub format: String,
}

/// `GET /instances/{id}/audit/export?format=soc2`
///
/// Downloads the audit trail as a compliance-formatted CSV file.
pub async fn export_audit_trail(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Path(id): Path<String>,
    Query(params): Query<AuditExportParams>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::View,
        "*",
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let format = match orbflow_core::compliance::ComplianceFormat::from_str_opt(&params.format) {
        Some(f) => f,
        None => {
            return write_error(
                StatusCode::BAD_REQUEST,
                format!(
                    "unsupported compliance format '{}'; supported: soc2, hipaa, pci",
                    params.format
                ),
            );
        }
    };

    let inst_id = orbflow_core::InstanceId::new(id.clone());
    let records = match state.engine.load_audit_records(&inst_id).await {
        Ok(r) => r,
        Err(OrbflowError::NotFound) => {
            return write_error(StatusCode::NOT_FOUND, "instance not found");
        }
        Err(e) => {
            error!(error = %e, "failed to load audit records for export");
            return write_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to load audit records",
            );
        }
    };

    let exporter = orbflow_core::compliance::exporter_for(format);
    let csv_bytes = match exporter.export(&records) {
        Ok(b) => b,
        Err(e) => {
            error!(error = %e, "failed to export audit trail");
            return write_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to export audit trail",
            );
        }
    };

    let filename = format!(
        "audit-{}-{}.{}",
        id,
        params.format,
        exporter.file_extension()
    );

    (
        StatusCode::OK,
        [
            (
                axum::http::header::CONTENT_TYPE,
                exporter.content_type().to_string(),
            ),
            (
                axum::http::header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
        ],
        csv_bytes,
    )
        .into_response()
}

// --- RBAC ---

/// Returns the current RBAC policy, or a default empty policy if none is configured.
pub async fn get_rbac_policy(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::Admin,
        "*",
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    match &state.rbac {
        Some(rbac) => match rbac.read() {
            Ok(policy) => {
                let mut result = policy.clone();
                result.ensure_defaults();
                write_data(StatusCode::OK, result)
            }
            Err(_) => {
                error!("RBAC policy lock is poisoned");
                write_error(StatusCode::INTERNAL_SERVER_ERROR, "Internal server error")
            }
        },
        None => write_data(
            StatusCode::OK,
            orbflow_core::rbac::RbacPolicy::with_defaults(),
        ),
    }
}

/// Replaces the current RBAC policy.
pub async fn update_rbac_policy(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Json(new_policy): Json<orbflow_core::rbac::RbacPolicy>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::Admin,
        "*",
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    if !new_policy.has_admin_binding() {
        return write_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Policy must contain at least one admin binding",
        );
    }

    // Validate builtin role integrity, duplicate IDs, orphaned bindings, etc.
    if let Err(e) = orbflow_core::rbac::RbacPolicy::validate_update(&new_policy) {
        return write_error(StatusCode::UNPROCESSABLE_ENTITY, e.to_string());
    }

    // Verify the calling user retains global admin access in the new policy.
    // Only Global-scoped admin bindings count because the entry guard
    // (check_permission) also requires global scope for policy management.
    let is_bootstrap = state
        .bootstrap_admin
        .as_deref()
        .is_some_and(|ba| ba == auth_user.user_id);
    let caller_retains_admin = is_bootstrap
        || new_policy.bindings.iter().any(|b| {
            b.subject == auth_user.user_id
                && b.scope == orbflow_core::rbac::PolicyScope::Global
                && new_policy
                    .get_role(&b.role_id)
                    .is_some_and(|r| r.permissions.contains(&Permission::Admin))
        });
    if !caller_retains_admin {
        return write_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Policy update would remove your own admin access",
        );
    }

    match &state.rbac {
        Some(rbac) => {
            // Persist to database first (if an RBAC store is configured).
            if let Some(ref rbac_store) = state.rbac_store
                && let Err(e) = rbac_store.save_policy(&new_policy).await
            {
                error!("failed to persist RBAC policy: {e}");
                return write_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to persist RBAC policy",
                );
            }
            // Update the in-memory cache so the change takes effect immediately.
            match rbac.write() {
                Ok(mut policy) => {
                    *policy = new_policy.clone();
                    write_data(StatusCode::OK, &new_policy)
                }
                Err(_) => {
                    error!("RBAC policy lock is poisoned");
                    write_error(StatusCode::INTERNAL_SERVER_ERROR, "Internal server error")
                }
            }
        }
        None => write_error(
            StatusCode::NOT_IMPLEMENTED,
            "RBAC is not enabled on this server",
        ),
    }
}

/// Returns the list of distinct subjects in the current RBAC policy.
pub async fn list_rbac_subjects(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::Admin,
        "*",
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    // Prefer the persistent store (database) as the source of truth when
    // available; fall back to the in-memory policy snapshot otherwise.
    if let Some(ref rbac_store) = state.rbac_store {
        match rbac_store.list_subjects().await {
            Ok(mut subjects) => {
                subjects.sort();
                subjects.dedup();
                return write_data(StatusCode::OK, subjects);
            }
            Err(e) => {
                error!("failed to list RBAC subjects from store: {e}");
                return write_error(StatusCode::INTERNAL_SERVER_ERROR, "Failed to list subjects");
            }
        }
    }

    match &state.rbac {
        Some(rbac) => match rbac.read() {
            Ok(policy) => {
                let mut subjects: Vec<String> = policy
                    .bindings
                    .iter()
                    .map(|b| b.subject.clone())
                    .collect::<std::collections::HashSet<_>>()
                    .into_iter()
                    .collect();
                subjects.sort();
                write_data(StatusCode::OK, subjects)
            }
            Err(_) => {
                error!("RBAC policy lock is poisoned");
                write_error(StatusCode::INTERNAL_SERVER_ERROR, "Internal server error")
            }
        },
        None => write_data(StatusCode::OK, Vec::<String>::new()),
    }
}

// --- Marketplace ---

/// Marketplace plugin summary for the list endpoint.
#[derive(Debug, serde::Serialize)]
struct MarketplacePluginSummary {
    name: String,
    latest_version: String,
    description: Option<String>,
    author: Option<String>,
    tags: Vec<String>,
    icon: Option<String>,
    category: Option<String>,
    color: Option<String>,
    downloads: u64,
    installed: bool,
}

/// Marketplace plugin detail for the get endpoint.
#[derive(Debug, serde::Serialize)]
struct MarketplacePluginDetail {
    name: String,
    version: String,
    description: Option<String>,
    author: Option<String>,
    license: Option<String>,
    repository: Option<String>,
    node_types: Vec<String>,
    orbflow_version: Option<String>,
    tags: Vec<String>,
    icon: Option<String>,
    category: Option<String>,
    color: Option<String>,
    language: Option<String>,
    readme: Option<String>,
    downloads: u64,
    installed: bool,
}

/// Registers node schemas from a plugin manifest JSON with the engine.
///
/// Called after a successful install so that new node types immediately
/// appear in the node picker without a server restart.
///
/// Reads the plugin manifest from the install directory to extract field
/// definitions (inputs, outputs, parameters). Falls back to empty fields
/// if the manifest is missing or cannot be parsed.
fn register_plugin_schemas(
    engine: &Arc<dyn Engine>,
    entry: &orbflow_core::ports::PluginIndexEntry,
    manifest_json: Option<&serde_json::Value>,
) {
    let desc = entry.description.as_deref().unwrap_or("").to_string();
    let category = entry.category.as_deref().unwrap_or("plugin").to_string();
    let icon = entry.icon.as_deref().unwrap_or("puzzle").to_string();
    let color = entry.color.as_deref().unwrap_or("#6366f1").to_string();

    // Parse field schemas from manifest JSON. Each field is a serde_json::Value
    // that deserializes into FieldSchema.
    let parse_fields = |key: &str| -> Vec<orbflow_core::ports::FieldSchema> {
        manifest_json
            .and_then(|m| m.get(key))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .enumerate()
                    .filter_map(|(i, v)| {
                        serde_json::from_value(v.clone())
                            .map_err(|e| {
                                tracing::warn!(
                                    plugin = %entry.name,
                                    field_index = i,
                                    kind = key,
                                    error = %e,
                                    "malformed field schema in manifest — skipping"
                                );
                                e
                            })
                            .ok()
                    })
                    .collect()
            })
            .unwrap_or_default()
    };

    let inputs = parse_fields("inputs");
    let outputs = parse_fields("outputs");
    let parameters = parse_fields("parameters");

    for node_type in &entry.node_types {
        let display_name = node_type
            .strip_prefix("plugin:")
            .unwrap_or(node_type)
            .split('-')
            .map(|w| {
                let mut c = w.chars();
                match c.next() {
                    Some(first) => first.to_uppercase().to_string() + c.as_str(),
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ");

        let schema = orbflow_core::ports::NodeSchema {
            plugin_ref: node_type.to_string(),
            name: display_name,
            description: desc.clone(),
            category: category.clone(),
            node_kind: None,
            icon: icon.clone(),
            color: color.clone(),
            docs: None,
            image_url: None,
            inputs: inputs.clone(),
            outputs: outputs.clone(),
            parameters: parameters.clone(),
            capability_ports: vec![],
            settings: vec![],
            provides_capability: None,
        };
        engine.register_schema(node_type, schema);
        tracing::info!(node_type = %node_type, "registered node schema after install");
    }
}

/// Compute a relevance score for a plugin against a search query.
///
/// Higher score = better match. Returns 0 if no match.
/// Scoring: exact name (100) > name contains (50) > tag match (30) > description (10) > author (5).
///
/// D2 fix: operates on `serde_json::Value` since the filter/sort pipeline uses
/// serialized JSON values. A typed pipeline refactor is tracked separately.
fn search_score(plugin: &serde_json::Value, q: &str) -> u32 {
    let mut score = 0u32;
    if let Some(name) = plugin["name"].as_str() {
        let name_lower = name.to_lowercase();
        if name_lower == q {
            score += 100;
        } else if name_lower.contains(q) {
            score += 50;
        }
    }
    if let Some(tags) = plugin["tags"].as_array()
        && tags
            .iter()
            .any(|t| t.as_str().is_some_and(|s| s.to_lowercase().contains(q)))
    {
        score += 30;
    }
    if let Some(desc) = plugin["description"].as_str()
        && desc.to_lowercase().contains(q)
    {
        score += 10;
    }
    if let Some(author) = plugin["author"].as_str()
        && author.to_lowercase().contains(q)
    {
        score += 5;
    }
    score
}

/// Query parameters for `GET /marketplace/plugins`.
#[derive(Debug, Deserialize)]
pub struct MarketplaceQueryParams {
    /// Free-text search (matches name, description, tags, author).
    pub q: Option<String>,
    /// Filter by category slug (e.g. "ai", "database").
    pub category: Option<String>,
    /// Sort field: "name", "downloads", "updated" (default: "name").
    pub sort: Option<String>,
    /// Sort order: "asc" (default) or "desc".
    pub order: Option<String>,
    /// Pagination offset (default 0).
    pub offset: Option<i64>,
    /// Page size (default 20, max 100).
    pub limit: Option<i64>,
    /// When true, only return locally installed plugins.
    pub installed_only: Option<bool>,
}

/// `GET /marketplace/plugins` -- list plugins with filtering, sorting, and pagination.
pub async fn list_installed_plugins(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Query(params): Query<MarketplaceQueryParams>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::View,
        "*",
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let index = match &state.plugin_index {
        Some(idx) => idx,
        None => {
            return write_error(
                StatusCode::NOT_IMPLEMENTED,
                "plugin marketplace not configured",
            );
        }
    };

    match index.list_available().await {
        Ok(entries) => {
            // Determine which plugins are locally installed by checking the filesystem.
            // A plugin is "installed" if its directory contains an orbflow-plugin.json manifest.
            let installed_names: std::collections::HashSet<String> = match &state.plugins_dir {
                Some(dir) => {
                    let dir = dir.clone();
                    tokio::task::spawn_blocking(move || {
                        let mut names = std::collections::HashSet::new();
                        if let Ok(entries) = std::fs::read_dir(&dir) {
                            for entry in entries.flatten() {
                                if entry.path().join("orbflow-plugin.json").exists()
                                    && let Some(name) = entry.file_name().to_str()
                                {
                                    names.insert(name.to_string());
                                }
                            }
                        }
                        names
                    })
                    .await
                    .unwrap_or_default()
                }
                None => std::collections::HashSet::new(),
            };

            // Build typed summaries with installed flag.
            // P5 fix: log and skip entries that fail serialization instead of
            // silently inserting null via unwrap_or_default().
            let mut plugins: Vec<serde_json::Value> = entries
                .iter()
                .filter_map(|e| {
                    let summary = MarketplacePluginSummary {
                        name: e.name.clone(),
                        latest_version: e.version.clone(),
                        description: e.description.clone(),
                        author: e.author.clone(),
                        tags: e.tags.clone(),
                        icon: e.icon.clone(),
                        category: e.category.clone(),
                        color: e.color.clone(),
                        downloads: 0,
                        installed: installed_names.contains(&e.name),
                    };
                    match serde_json::to_value(summary) {
                        Ok(v) => Some(v),
                        Err(err) => {
                            tracing::error!(plugin = %e.name, error = %err, "failed to serialize plugin summary");
                            None
                        }
                    }
                })
                .collect();

            // --- Filter ---
            if params.installed_only.unwrap_or(false) {
                plugins.retain(|p| p["installed"].as_bool().unwrap_or(false));
            }
            if let Some(ref cat) = params.category {
                let cat_lower = cat.to_lowercase();
                plugins.retain(|p| {
                    p["category"]
                        .as_str()
                        .is_some_and(|c| c.to_lowercase() == cat_lower)
                });
            }
            // Search with relevance scoring: exact name > name contains > tag > desc > author.
            let has_query = params.q.as_ref().is_some_and(|q| !q.trim().is_empty());
            if let Some(ref q) = params.q {
                let q_lower = q.to_lowercase();
                if !q_lower.is_empty() {
                    plugins.retain(|p| search_score(p, &q_lower) > 0);
                }
            }

            // --- Sort ---
            // Pre-compute search scores and lowercase names to avoid repeated
            // allocations inside sort comparators (O(n) instead of O(n log n)).
            let sort_field =
                params
                    .sort
                    .as_deref()
                    .unwrap_or(if has_query { "relevance" } else { "name" });
            let descending = params.order.as_deref() == Some("desc");
            let q_for_sort = params
                .q
                .as_ref()
                .map(|q| q.to_lowercase())
                .unwrap_or_default();

            match sort_field {
                "relevance" => {
                    // D1 fix: sort in-place with cached keys instead of
                    // building an intermediate scored Vec.
                    plugins.sort_by_cached_key(|p| std::cmp::Reverse(search_score(p, &q_for_sort)));
                }
                "downloads" => {
                    plugins.sort_by(|a, b| {
                        let da = a["downloads"].as_u64().unwrap_or(0);
                        let db = b["downloads"].as_u64().unwrap_or(0);
                        let cmp = da.cmp(&db);
                        if descending { cmp.reverse() } else { cmp }
                    });
                }
                _ => {
                    // Pre-compute lowercase keys once via sort_by_cached_key.
                    plugins.sort_by_cached_key(|p| p["name"].as_str().unwrap_or("").to_lowercase());
                    if descending {
                        plugins.reverse();
                    }
                }
            }

            // --- Paginate ---
            let total = plugins.len() as i64;
            let offset = params.offset.unwrap_or(0).max(0);
            let limit = params.limit.unwrap_or(20).clamp(1, 100);
            let page: Vec<_> = plugins
                .into_iter()
                .skip(offset as usize)
                .take(limit as usize)
                .collect();

            axum::Json(serde_json::json!({
                "data": page,
                "error": null,
                "meta": {
                    "total": total,
                    "offset": offset,
                    "limit": limit,
                },
            }))
            .into_response()
        }
        Err(e) => write_safe_error(&e),
    }
}

/// `GET /marketplace/plugins/{name}` -- get details of a specific installed plugin.
pub async fn get_installed_plugin(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Path(name): Path<String>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::View,
        "*",
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    // Validate plugin name: only alphanumeric, hyphens, underscores allowed.
    if !name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return write_error(StatusCode::BAD_REQUEST, "invalid plugin name");
    }

    let index = match &state.plugin_index {
        Some(idx) => idx,
        None => {
            return write_error(
                StatusCode::NOT_IMPLEMENTED,
                "plugin marketplace not configured",
            );
        }
    };

    match index.get_entry(&name).await {
        Ok(Some(entry)) => {
            // Check filesystem for installed status (manifest exists on disk).
            let installed = match &state.plugins_dir {
                Some(dir) => {
                    let manifest = std::path::PathBuf::from(dir)
                        .join(&name)
                        .join("orbflow-plugin.json");
                    tokio::task::spawn_blocking(move || manifest.exists())
                        .await
                        .unwrap_or(false)
                }
                None => false,
            };
            let detail = MarketplacePluginDetail {
                name: entry.name,
                version: entry.version,
                description: entry.description,
                author: entry.author,
                license: entry.license,
                repository: entry.repository,
                node_types: entry.node_types,
                orbflow_version: entry.orbflow_version,
                tags: entry.tags,
                icon: entry.icon,
                category: entry.category,
                color: entry.color,
                language: entry.language,
                readme: entry.readme,
                downloads: 0,
                installed,
            };
            write_data(StatusCode::OK, detail)
        }
        Ok(None) => write_error(StatusCode::NOT_FOUND, "plugin not found"),
        Err(e) => write_safe_error(&e),
    }
}

/// `POST /marketplace/plugins/{name}/install` -- install a community plugin.
pub async fn install_plugin(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Path(name): Path<String>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::Admin,
        "*",
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    // Validate plugin name.
    if !name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return write_error(StatusCode::BAD_REQUEST, "invalid plugin name");
    }

    let plugins_dir = match &state.plugins_dir {
        Some(dir) => dir.clone(),
        None => {
            return write_error(
                StatusCode::NOT_IMPLEMENTED,
                "plugin installation not configured",
            );
        }
    };

    // Get plugin installer.
    let installer = match &state.plugin_installer {
        Some(inst) => inst,
        None => {
            return write_error(
                StatusCode::NOT_IMPLEMENTED,
                "plugin installation not configured",
            );
        }
    };

    // Get plugin metadata from the index.
    let index = match &state.plugin_index {
        Some(idx) => idx,
        None => {
            return write_error(
                StatusCode::NOT_IMPLEMENTED,
                "plugin marketplace not configured",
            );
        }
    };

    let entry = match index.get_entry(&name).await {
        Ok(Some(e)) => e,
        Ok(None) => return write_error(StatusCode::NOT_FOUND, "plugin not found in index"),
        Err(e) => return write_safe_error(&e),
    };

    let plugin_dir = std::path::PathBuf::from(&plugins_dir).join(&name);

    // Download and install plugin files via the PluginInstaller port trait.
    match installer.install_plugin(&name, &plugin_dir).await {
        Ok(file_count) => {
            tracing::info!(
                plugin = %name,
                files = file_count,
                path = %plugin_dir.display(),
                "plugin installed via index"
            );
        }
        Err(e) => {
            tracing::error!(plugin = %name, error = %e, "failed to install plugin");
            return write_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to download and install plugin from registry",
            );
        }
    }

    // Read the downloaded plugin's manifest (if any). This may be a proper
    // PluginManifest with protocol, inputs, outputs, parameters — we must
    // preserve its structure so LocalIndex::scan() can parse it on restart.
    let manifest_path = plugin_dir.join("orbflow-plugin.json");
    let downloaded_manifest: Option<serde_json::Value> =
        match tokio::fs::read_to_string(&manifest_path).await {
            Ok(data) => serde_json::from_str(&data)
                .map_err(|e| {
                    tracing::warn!(
                        plugin = %name,
                        error = %e,
                        "failed to parse downloaded plugin manifest"
                    );
                    e
                })
                .ok(),
            Err(_) => None,
        };

    // Build the manifest to write to disk.
    //
    // If the download included a valid PluginManifest, use it as the base and
    // enrich with index metadata (description, tags, icon, etc.). This preserves
    // the `protocol` enum and required fields that LocalIndex::scan() needs.
    //
    // If no valid manifest was downloaded, construct a PluginManifest-compatible
    // JSON from the index entry so it can be parsed on restart.
    let has_valid_manifest = downloaded_manifest
        .as_ref()
        .is_some_and(|m| m.get("protocol").is_some_and(|p| p.is_object()));

    let manifest_str = {
        let manifest_json =
            if let Some(m) = downloaded_manifest.as_ref().filter(|_| has_valid_manifest) {
                let mut m = m.clone();
                // Enrich with index metadata that may be richer than the plugin's own manifest.
                if let Some(desc) = entry.description.as_deref() {
                    m["description"] = serde_json::Value::String(desc.to_string());
                }
                if let Some(icon) = entry.icon.as_deref() {
                    m["icon"] = serde_json::Value::String(icon.to_string());
                }
                if let Some(category) = entry.category.as_deref() {
                    m["category"] = serde_json::Value::String(category.to_string());
                }
                if let Some(color) = entry.color.as_deref() {
                    m["color"] = serde_json::Value::String(color.to_string());
                }
                if !entry.tags.is_empty() {
                    m["tags"] = serde_json::to_value(&entry.tags).unwrap_or_default();
                }
                if let Some(readme) = entry.readme.as_deref() {
                    m["readme"] = serde_json::Value::String(readme.to_string());
                }
                m
            } else {
                // No valid manifest from download — build a PluginManifest-compatible JSON.
                serde_json::json!({
                    "name": name,
                    "version": entry.version,
                    "description": entry.description.as_deref().unwrap_or(""),
                    "author": entry.author.as_deref().unwrap_or("Unknown"),
                    "license": entry.license.as_deref().unwrap_or("Unknown"),
                    "node_types": entry.node_types,
                    "orbflow_version": entry.orbflow_version.as_deref().unwrap_or("0.1.0"),
                    "protocol": { "Subprocess": { "binary_name": name } },
                    "tags": entry.tags,
                    "icon": entry.icon.as_deref().unwrap_or("puzzle"),
                    "category": entry.category.as_deref().unwrap_or("plugin"),
                    "color": entry.color.as_deref().unwrap_or("#6366f1"),
                })
            };
        match serde_json::to_string_pretty(&manifest_json) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(plugin = %name, error = %e, "failed to serialize plugin manifest");
                return write_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "failed to create plugin manifest",
                );
            }
        }
    };

    // P1 fix: use tokio::fs::write instead of blocking std::fs::write.
    match tokio::fs::write(&manifest_path, manifest_str).await {
        Ok(()) => {
            tracing::info!(plugin = %name, path = %plugin_dir.display(), "plugin installed");

            // Register node schemas with the engine so they appear in the node picker
            // with proper field definitions (inputs, outputs, parameters).
            register_plugin_schemas(&state.engine, &entry, downloaded_manifest.as_ref());

            // Notify workers to reload plugins from disk so the new executor
            // is available for task execution without a worker restart.
            // Workers handle both subprocess and gRPC plugin discovery/spawning.
            // The server does NOT spawn gRPC processes here to avoid port
            // conflicts with the worker (which owns the plugin processes).
            let reload_status = if let Some(bus) = &state.bus {
                let reload_subj = orbflow_core::plugin_reload_subject();
                let bus = Arc::clone(bus);
                tokio::spawn(async move {
                    if let Err(e) = bus.publish(&reload_subj, b"reload").await {
                        tracing::warn!(error = %e, "failed to publish plugin reload signal");
                    } else {
                        tracing::info!("published plugin reload signal to workers");
                    }
                });
                "pending"
            } else {
                "skipped"
            };

            write_data(
                StatusCode::OK,
                serde_json::json!({
                    "name": name,
                    "status": "installed",
                    "reload": reload_status,
                }),
            )
        }
        Err(e) => {
            tracing::error!(plugin = %name, error = %e, "failed to write plugin manifest");
            write_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to write plugin manifest",
            )
        }
    }
}

/// `POST /marketplace/validate-manifest` -- validate a plugin manifest JSON.
///
/// Public endpoint (no auth required beyond standard middleware) so the submit
/// wizard can validate before the user opens a GitHub PR.
pub async fn validate_manifest(Json(body): Json<serde_json::Value>) -> Response {
    let mut errors: Vec<String> = Vec::new();

    // Required string fields.
    for field in ["name", "version", "description", "author"] {
        match body.get(field).and_then(|v| v.as_str()) {
            Some(s) if s.trim().is_empty() => errors.push(format!("{field} must not be empty")),
            Some(_) => {}
            None => errors.push(format!("{field} is required")),
        }
    }

    // Validate name format: alphanumeric, hyphens, underscores only.
    if let Some(name) = body.get("name").and_then(|v| v.as_str()) {
        if !name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            errors.push(
                "name must contain only alphanumeric characters, hyphens, and underscores".into(),
            );
        }
        if name.len() > 64 {
            errors.push("name must be 64 characters or fewer".into());
        }
    }

    // Validate version looks like semver.
    if let Some(ver) = body.get("version").and_then(|v| v.as_str()) {
        let parts: Vec<&str> = ver.split('.').collect();
        if parts.len() < 2 || parts.len() > 3 || parts.iter().any(|p| p.parse::<u64>().is_err()) {
            errors.push("version must be a valid semver (e.g., 1.0.0)".into());
        }
    }

    // node_types must be a non-empty array of strings (max 100).
    match body.get("node_types") {
        Some(v) if v.is_array() => {
            if let Some(arr) = v.as_array() {
                if arr.is_empty() {
                    errors.push("node_types must contain at least one entry".into());
                } else if arr.len() > 100 {
                    errors.push("node_types must contain at most 100 entries".into());
                }
                for (i, item) in arr.iter().take(100).enumerate() {
                    if !item.is_string() {
                        errors.push(format!("node_types[{i}] must be a string"));
                    }
                }
            }
        }
        _ => errors.push("node_types is required and must be an array".into()),
    }

    // protocol must be present and a valid variant.
    match body.get("protocol").and_then(|v| v.as_str()) {
        Some("subprocess" | "grpc") => {}
        Some(other) => errors.push(format!(
            "protocol must be \"subprocess\" or \"grpc\", got \"{other}\""
        )),
        None => {
            // Also accept object form: { "Subprocess": { "binary_name": "..." } }
            if body.get("protocol").and_then(|v| v.as_object()).is_none() {
                errors.push("protocol is required (\"subprocess\" or \"grpc\")".into());
            }
        }
    }

    // Optional field validations.
    if let Some(cat) = body.get("category").and_then(|v| v.as_str()) {
        let valid = [
            "ai",
            "database",
            "communication",
            "utility",
            "monitoring",
            "security",
            "cloud",
            "integration",
        ];
        if !valid.contains(&cat) {
            errors.push(format!("category must be one of: {}", valid.join(", ")));
        }
    }

    if let Some(color) = body.get("color").and_then(|v| v.as_str())
        && (!color.starts_with('#') || (color.len() != 4 && color.len() != 7))
    {
        errors.push("color must be a valid hex color (e.g., #6366F1)".into());
    }

    if let Some(repo) = body.get("repo").and_then(|v| v.as_str())
        && (!repo.contains('/') || repo.contains(".."))
    {
        errors.push("repo must be in \"owner/repo\" format".into());
    }

    if errors.is_empty() {
        write_data(StatusCode::OK, serde_json::json!({ "valid": true }))
    } else {
        axum::Json(serde_json::json!({
            "data": { "valid": false, "errors": errors },
            "error": null,
        }))
        .into_response()
    }
}

/// `DELETE /marketplace/plugins/{name}` -- uninstall a plugin (admin-only).
pub async fn uninstall_plugin(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Path(name): Path<String>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::Admin,
        "*",
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    // Validate plugin name.
    if !name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return write_error(StatusCode::BAD_REQUEST, "invalid plugin name");
    }

    let plugins_dir = match &state.plugins_dir {
        Some(dir) => dir.clone(),
        None => {
            return write_error(
                StatusCode::NOT_IMPLEMENTED,
                "plugin installation not configured",
            );
        }
    };

    let plugin_dir = std::path::PathBuf::from(&plugins_dir).join(&name);

    // Security: verify the resolved path stays within plugins_dir and is not a symlink.
    let plugins_base = plugins_dir.clone();
    let dir_check = plugin_dir.clone();
    let check_result = tokio::task::spawn_blocking(move || {
        if !dir_check.exists() {
            return Err("not_found");
        }
        // Reject symlinks to prevent following links outside plugins_dir.
        if let Ok(meta) = std::fs::symlink_metadata(&dir_check)
            && meta.file_type().is_symlink()
        {
            return Err("symlink");
        }
        // Canonicalize and verify it stays within plugins_dir.
        let canonical_base = std::fs::canonicalize(&plugins_base)
            .unwrap_or_else(|_| std::path::PathBuf::from(&plugins_base));
        let canonical_dir = std::fs::canonicalize(&dir_check).unwrap_or_else(|_| dir_check.clone());
        if !canonical_dir.starts_with(&canonical_base) {
            return Err("outside_base");
        }
        Ok(())
    })
    .await;

    match check_result {
        Ok(Ok(())) => {}
        Ok(Err("not_found")) => return write_error(StatusCode::NOT_FOUND, "plugin not installed"),
        Ok(Err(_)) => return write_error(StatusCode::BAD_REQUEST, "invalid plugin directory"),
        Err(_) => return write_error(StatusCode::INTERNAL_SERVER_ERROR, "check task failed"),
    }

    // Stop the plugin if it is running.
    if let Some(pm) = &state.plugin_manager
        && let Ok(info) = pm.get_plugin(&name).await
        && info.status == "running"
        && let Err(e) = pm.stop_plugin(&name).await
    {
        tracing::warn!(plugin = %name, error = %e, "failed to stop plugin before uninstall");
    }

    // Read manifest before deletion so we can unregister node schemas.
    let manifest_path = plugin_dir.join("orbflow-plugin.json");
    let node_types_to_remove: Vec<String> = match std::fs::read_to_string(&manifest_path) {
        Ok(content) => serde_json::from_str::<orbflow_core::ports::PluginIndexEntry>(&content)
            .map(|entry| entry.node_types)
            .unwrap_or_default(),
        Err(_) => vec![],
    };

    // Remove the plugin directory in a blocking task.
    let dir_to_remove = plugin_dir.clone();
    let remove_result =
        tokio::task::spawn_blocking(move || std::fs::remove_dir_all(&dir_to_remove)).await;

    match remove_result {
        Ok(Ok(())) => {
            tracing::info!(plugin = %name, path = %plugin_dir.display(), "plugin uninstalled");

            // Unregister node schemas so the node picker updates immediately.
            for nt in &node_types_to_remove {
                state.engine.unregister_schema(nt);
                tracing::info!(node_type = %nt, "unregistered node schema after uninstall");
            }

            // Reload plugins so the engine removes the uninstalled node types.
            if let Some(pm) = &state.plugin_manager {
                let pm = Arc::clone(pm);
                tokio::spawn(async move {
                    match pm.reload_all().await {
                        Ok(plugins) => {
                            tracing::info!(
                                count = plugins.len(),
                                "plugins reloaded after uninstall"
                            );
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "plugin reload failed after uninstall");
                        }
                    }
                });
            }

            write_data(
                StatusCode::OK,
                serde_json::json!({
                    "name": name,
                    "status": "uninstalled",
                }),
            )
        }
        Ok(Err(e)) => {
            tracing::error!(plugin = %name, error = %e, "failed to remove plugin directory");
            write_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to remove plugin directory",
            )
        }
        Err(e) => {
            tracing::error!(plugin = %name, error = %e, "uninstall task panicked");
            write_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to remove plugin directory",
            )
        }
    }
}

// --- Plugin Lifecycle ---

fn get_plugin_manager(
    state: &AppState,
) -> Result<&Arc<dyn orbflow_core::PluginManager>, Box<Response>> {
    state.plugin_manager.as_ref().ok_or_else(|| {
        Box::new(write_error(
            StatusCode::NOT_IMPLEMENTED,
            "plugin manager not configured",
        ))
    })
}

/// Validate plugin name at the HTTP boundary using the shared validator.
fn check_plugin_name(name: &str) -> Result<(), Box<Response>> {
    validate_plugin_name(name)
        .map_err(|e| Box::new(write_error(StatusCode::BAD_REQUEST, e.to_string())))
}

/// GET /plugins/status — list all plugin statuses (running, stopped, available).
pub async fn list_plugin_status(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::View,
        "*",
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let pm = match get_plugin_manager(&state) {
        Ok(pm) => pm,
        Err(resp) => return *resp,
    };

    match pm.list_plugins().await {
        Ok(plugins) => write_data(StatusCode::OK, serde_json::json!(plugins)),
        Err(e) => write_safe_error(&e),
    }
}

/// GET /plugins/{name}/status — get a single plugin's status.
pub async fn get_plugin_status(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Path(name): Path<String>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::View,
        "*",
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    if let Err(resp) = check_plugin_name(&name) {
        return *resp;
    }

    let pm = match get_plugin_manager(&state) {
        Ok(pm) => pm,
        Err(resp) => return *resp,
    };

    match pm.get_plugin(&name).await {
        Ok(info) => write_data(StatusCode::OK, serde_json::json!(info)),
        Err(orbflow_core::OrbflowError::NotFound) => {
            write_error(StatusCode::NOT_FOUND, "plugin not found")
        }
        Err(e) => write_safe_error(&e),
    }
}

/// POST /plugins/{name}/start — start a plugin process.
///
/// [B1] The mutex is held only for the spawn phase. The health-check
/// (up to 30s) runs in a background task to avoid blocking other API calls.
/// Returns 202 Accepted immediately; poll GET /plugins/{name}/status to check.
pub async fn start_plugin(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Path(name): Path<String>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::Admin,
        "*",
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    if let Err(resp) = check_plugin_name(&name) {
        return *resp;
    }

    let pm = match get_plugin_manager(&state) {
        Ok(pm) => pm,
        Err(resp) => return *resp,
    };

    // Spawn in a background task to avoid blocking the API during health-check.
    let pm = Arc::clone(pm);
    let name_clone = name.clone();
    tokio::spawn(async move {
        match pm.start_plugin(&name_clone).await {
            Ok(info) => {
                tracing::info!(plugin = %info.name, address = ?info.address, "plugin started via API");
            }
            Err(e) => {
                tracing::warn!(plugin = %name_clone, error = %e, "plugin start failed via API");
            }
        }
    });

    write_data(
        StatusCode::ACCEPTED,
        serde_json::json!({
            "name": name,
            "status": "starting",
            "message": "plugin is starting — poll GET /plugins/{name}/status",
        }),
    )
}

/// POST /plugins/{name}/stop — stop a running plugin process.
pub async fn stop_plugin(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Path(name): Path<String>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::Admin,
        "*",
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    if let Err(resp) = check_plugin_name(&name) {
        return *resp;
    }

    let pm = match get_plugin_manager(&state) {
        Ok(pm) => pm,
        Err(resp) => return *resp,
    };

    match pm.stop_plugin(&name).await {
        Ok(()) => write_data(
            StatusCode::OK,
            serde_json::json!({
                "name": name,
                "status": "stopped",
            }),
        ),
        Err(e) => write_safe_error(&e),
    }
}

/// POST /plugins/{name}/restart — stop then start a plugin.
///
/// Stop is synchronous, start runs in background (same as start_plugin).
pub async fn restart_plugin(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Path(name): Path<String>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::Admin,
        "*",
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    if let Err(resp) = check_plugin_name(&name) {
        return *resp;
    }

    let pm = match get_plugin_manager(&state) {
        Ok(pm) => pm,
        Err(resp) => return *resp,
    };

    // Stop synchronously (fast), then spawn start in background.
    let _ = pm.stop_plugin(&name).await;

    let pm = Arc::clone(pm);
    let name_clone = name.clone();
    tokio::spawn(async move {
        match pm.start_plugin(&name_clone).await {
            Ok(info) => {
                tracing::info!(plugin = %info.name, address = ?info.address, "plugin restarted via API");
            }
            Err(e) => {
                tracing::warn!(plugin = %name_clone, error = %e, "plugin restart failed via API");
            }
        }
    });

    write_data(
        StatusCode::ACCEPTED,
        serde_json::json!({
            "name": name,
            "status": "restarting",
            "message": "plugin is restarting — poll GET /plugins/{name}/status",
        }),
    )
}

/// POST /plugins/reload — stop all plugins, re-scan, and spawn everything again.
pub async fn reload_all_plugins(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::Admin,
        "*",
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let pm = match get_plugin_manager(&state) {
        Ok(pm) => pm,
        Err(resp) => return *resp,
    };

    let pm = Arc::clone(pm);
    let handle = tokio::spawn(async move {
        match pm.reload_all().await {
            Ok(spawned) => {
                tracing::info!(count = spawned.len(), "all plugins reloaded via API");
            }
            Err(e) => {
                tracing::error!(error = %e, "plugin reload failed via API");
            }
        }
    });
    // Log if the background task panics.
    tokio::spawn(async move {
        if let Err(e) = handle.await {
            tracing::error!(error = %e, "plugin reload task failed");
        }
    });

    write_data(
        StatusCode::ACCEPTED,
        serde_json::json!({
            "status": "reloading",
            "message": "all plugins are reloading — poll GET /plugins/status",
        }),
    )
}

// --- Change Requests ---

/// Query params for listing change requests.
#[derive(Debug, Deserialize)]
pub struct ListChangeRequestsParams {
    #[serde(default)]
    pub offset: Option<i64>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub status: Option<String>,
}

/// Request body for creating a change request.
#[derive(Debug, Deserialize)]
pub struct CreateChangeRequestBody {
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    pub proposed_definition: serde_json::Value,
    pub base_version: i32,
    pub author: String,
    #[serde(default)]
    pub reviewers: Vec<String>,
}

/// Request body for updating a change request.
#[derive(Debug, Deserialize)]
pub struct UpdateChangeRequestBody {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub proposed_definition: Option<serde_json::Value>,
    #[serde(default)]
    pub reviewers: Option<Vec<String>>,
}

/// Request body for rejecting a change request.
#[derive(Debug, Deserialize)]
pub struct RejectChangeRequestBody {
    #[serde(default)]
    pub reason: Option<String>,
}

/// Request body for adding a comment.
#[derive(Debug, Deserialize)]
pub struct AddCommentBody {
    pub author: String,
    pub body: String,
    #[serde(default)]
    pub node_id: Option<String>,
    #[serde(default)]
    pub edge_ref: Option<(String, String)>,
}

/// Path params for change request endpoints.
#[derive(Debug, Deserialize)]
pub struct ChangeRequestPathParams {
    pub id: String,
    pub cr_id: String,
}

/// Path params for comment resolve endpoint.
#[derive(Debug, Deserialize)]
pub struct CommentResolvePathParams {
    pub id: String,
    pub cr_id: String,
    pub comment_id: String,
}

/// `POST /workflows/{id}/change-requests`
pub async fn create_change_request(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Path(id): Path<String>,
    Json(body): Json<CreateChangeRequestBody>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::Edit,
        &id,
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let store = match &state.change_request_store {
        Some(s) => s,
        None => return write_error(StatusCode::NOT_FOUND, "change requests API not enabled"),
    };

    if body.title.trim().is_empty() {
        return write_error(StatusCode::BAD_REQUEST, "title must not be empty");
    }
    if body.title.len() > 200 {
        return write_error(
            StatusCode::BAD_REQUEST,
            "title must not exceed 200 characters",
        );
    }
    if body.author.trim().is_empty() {
        return write_error(StatusCode::BAD_REQUEST, "author must not be empty");
    }
    if body.author.len() > 100 {
        return write_error(
            StatusCode::BAD_REQUEST,
            "author must not exceed 100 characters",
        );
    }
    if let Some(ref desc) = body.description
        && desc.len() > 5000
    {
        return write_error(
            StatusCode::BAD_REQUEST,
            "description must not exceed 5000 characters",
        );
    }

    let cr = orbflow_core::ChangeRequest {
        id: Uuid::new_v4().to_string(),
        workflow_id: WorkflowId::new(id),
        title: body.title,
        description: body.description,
        proposed_definition: body.proposed_definition,
        base_version: body.base_version,
        status: orbflow_core::ChangeRequestStatus::Draft,
        author: body.author,
        reviewers: body.reviewers,
        comments: Vec::new(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    match store.create_change_request(&cr).await {
        Ok(()) => write_data(StatusCode::CREATED, cr),
        Err(e) => {
            error!(error = %e, "failed to create change request");
            write_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to create change request",
            )
        }
    }
}

/// `GET /workflows/{id}/change-requests`
pub async fn list_change_requests(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Path(id): Path<String>,
    Query(params): Query<ListChangeRequestsParams>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::View,
        &id,
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let store = match &state.change_request_store {
        Some(s) => s,
        None => return write_error(StatusCode::NOT_FOUND, "change requests API not enabled"),
    };

    let offset = params.offset.unwrap_or(0).max(0);
    let mut limit = params.limit.unwrap_or(DEFAULT_PAGE_SIZE);
    if limit <= 0 {
        limit = DEFAULT_PAGE_SIZE;
    }
    if limit > 100 {
        limit = 100;
    }
    let opts = ListOptions { offset, limit };

    let status_filter = params
        .status
        .as_deref()
        .and_then(|s| serde_json::from_value(serde_json::Value::String(s.to_string())).ok());

    let wf_id = WorkflowId::new(id);
    match store
        .list_change_requests(&wf_id, status_filter, opts)
        .await
    {
        Ok((crs, total)) => write_list(crs, total, offset, limit),
        Err(e) => {
            error!(error = %e, "failed to list change requests");
            write_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to list change requests",
            )
        }
    }
}

/// `GET /workflows/{id}/change-requests/{cr_id}`
pub async fn get_change_request(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Path(params): Path<ChangeRequestPathParams>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::View,
        &params.id,
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let store = match &state.change_request_store {
        Some(s) => s,
        None => return write_error(StatusCode::NOT_FOUND, "change requests API not enabled"),
    };

    match store.get_change_request(&params.cr_id).await {
        Ok(cr) => {
            if cr.workflow_id.0 != params.id {
                return write_error(StatusCode::NOT_FOUND, "change request not found");
            }
            write_data(StatusCode::OK, cr)
        }
        Err(OrbflowError::NotFound) => {
            write_error(StatusCode::NOT_FOUND, "change request not found")
        }
        Err(e) => {
            error!(error = %e, "failed to get change request");
            write_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to get change request",
            )
        }
    }
}

/// `PUT /workflows/{id}/change-requests/{cr_id}`
pub async fn update_change_request(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Path(params): Path<ChangeRequestPathParams>,
    Json(body): Json<UpdateChangeRequestBody>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::Edit,
        &params.id,
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let store = match &state.change_request_store {
        Some(s) => s,
        None => return write_error(StatusCode::NOT_FOUND, "change requests API not enabled"),
    };

    let cr = match store.get_change_request(&params.cr_id).await {
        Ok(cr) => cr,
        Err(OrbflowError::NotFound) => {
            return write_error(StatusCode::NOT_FOUND, "change request not found");
        }
        Err(e) => {
            error!(error = %e, "failed to get change request");
            return write_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to get change request",
            );
        }
    };

    if cr.workflow_id.0 != params.id {
        return write_error(StatusCode::NOT_FOUND, "change request not found");
    }

    if let Some(ref title) = body.title {
        if title.trim().is_empty() {
            return write_error(StatusCode::BAD_REQUEST, "title must not be empty");
        }
        if title.len() > 200 {
            return write_error(
                StatusCode::BAD_REQUEST,
                "title must not exceed 200 characters",
            );
        }
    }
    if let Some(ref desc) = body.description
        && desc.len() > 5000
    {
        return write_error(
            StatusCode::BAD_REQUEST,
            "description must not exceed 5000 characters",
        );
    }

    if !matches!(
        cr.status,
        orbflow_core::ChangeRequestStatus::Draft | orbflow_core::ChangeRequestStatus::Open
    ) {
        return write_error(
            StatusCode::CONFLICT,
            format!("cannot update a change request in status {:?}", cr.status),
        );
    }

    let updated = orbflow_core::ChangeRequest {
        title: body.title.unwrap_or(cr.title),
        description: body.description.or(cr.description),
        proposed_definition: body.proposed_definition.unwrap_or(cr.proposed_definition),
        reviewers: body.reviewers.unwrap_or(cr.reviewers),
        updated_at: Utc::now(),
        ..cr
    };

    match store.update_change_request(&updated).await {
        Ok(()) => write_data(StatusCode::OK, updated),
        Err(e) => {
            error!(error = %e, "failed to update change request");
            write_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to update change request",
            )
        }
    }
}

/// `POST /workflows/{id}/change-requests/{cr_id}/submit`
pub async fn submit_change_request(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Path(params): Path<ChangeRequestPathParams>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::Edit,
        &params.id,
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let store = match &state.change_request_store {
        Some(s) => s,
        None => return write_error(StatusCode::NOT_FOUND, "change requests API not enabled"),
    };

    let cr = match store.get_change_request(&params.cr_id).await {
        Ok(cr) => cr,
        Err(OrbflowError::NotFound) => {
            return write_error(StatusCode::NOT_FOUND, "change request not found");
        }
        Err(e) => {
            error!(error = %e, "failed to get change request");
            return write_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to get change request",
            );
        }
    };

    if cr.workflow_id.0 != params.id {
        return write_error(StatusCode::NOT_FOUND, "change request not found");
    }

    if cr.status != orbflow_core::ChangeRequestStatus::Draft {
        return write_error(
            StatusCode::CONFLICT,
            "change request is not in draft status",
        );
    }

    let updated = orbflow_core::ChangeRequest {
        status: orbflow_core::ChangeRequestStatus::Open,
        updated_at: Utc::now(),
        ..cr
    };

    match store.update_change_request(&updated).await {
        Ok(()) => write_data(StatusCode::OK, serde_json::json!({"status": "open"})),
        Err(e) => {
            error!(error = %e, "failed to submit change request");
            write_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to submit change request",
            )
        }
    }
}

/// `POST /workflows/{id}/change-requests/{cr_id}/approve`
pub async fn approve_change_request(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Path(params): Path<ChangeRequestPathParams>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::Approve,
        &params.id,
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let store = match &state.change_request_store {
        Some(s) => s,
        None => return write_error(StatusCode::NOT_FOUND, "change requests API not enabled"),
    };

    let cr = match store.get_change_request(&params.cr_id).await {
        Ok(cr) => cr,
        Err(OrbflowError::NotFound) => {
            return write_error(StatusCode::NOT_FOUND, "change request not found");
        }
        Err(e) => {
            error!(error = %e, "failed to get change request");
            return write_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to get change request",
            );
        }
    };

    if cr.workflow_id.0 != params.id {
        return write_error(StatusCode::NOT_FOUND, "change request not found");
    }

    if cr.status != orbflow_core::ChangeRequestStatus::Open {
        return write_error(StatusCode::CONFLICT, "change request is not in open status");
    }

    // Prevent self-approval: the author of a change request cannot approve it.
    if cr.author == auth_user.user_id {
        return write_error(
            StatusCode::FORBIDDEN,
            "cannot approve your own change request",
        );
    }

    let updated = orbflow_core::ChangeRequest {
        status: orbflow_core::ChangeRequestStatus::Approved,
        updated_at: Utc::now(),
        ..cr
    };

    match store.update_change_request(&updated).await {
        Ok(()) => write_data(StatusCode::OK, serde_json::json!({"status": "approved"})),
        Err(e) => {
            error!(error = %e, "failed to approve change request");
            write_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to approve change request",
            )
        }
    }
}

/// `POST /workflows/{id}/change-requests/{cr_id}/reject`
pub async fn reject_change_request(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Path(params): Path<ChangeRequestPathParams>,
    // TODO: store body.reason when ChangeRequest gains a rejection_reason field
    Json(body): Json<RejectChangeRequestBody>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::Approve,
        &params.id,
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let _ = &body; // acknowledge the body to suppress unused warnings

    let store = match &state.change_request_store {
        Some(s) => s,
        None => return write_error(StatusCode::NOT_FOUND, "change requests API not enabled"),
    };

    let cr = match store.get_change_request(&params.cr_id).await {
        Ok(cr) => cr,
        Err(OrbflowError::NotFound) => {
            return write_error(StatusCode::NOT_FOUND, "change request not found");
        }
        Err(e) => {
            error!(error = %e, "failed to get change request");
            return write_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to get change request",
            );
        }
    };

    if cr.workflow_id.0 != params.id {
        return write_error(StatusCode::NOT_FOUND, "change request not found");
    }

    if cr.status != orbflow_core::ChangeRequestStatus::Open {
        return write_error(StatusCode::CONFLICT, "change request is not in open status");
    }

    // Prevent self-rejection: the author of a change request cannot reject it.
    if cr.author == auth_user.user_id {
        return write_error(
            StatusCode::FORBIDDEN,
            "cannot reject your own change request",
        );
    }

    let updated = orbflow_core::ChangeRequest {
        status: orbflow_core::ChangeRequestStatus::Rejected,
        updated_at: Utc::now(),
        ..cr
    };

    match store.update_change_request(&updated).await {
        Ok(()) => write_data(StatusCode::OK, serde_json::json!({"status": "rejected"})),
        Err(e) => {
            error!(error = %e, "failed to reject change request");
            write_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to reject change request",
            )
        }
    }
}

/// `POST /workflows/{id}/change-requests/{cr_id}/rebase`
///
/// Updates the CR's base_version to the workflow's current version.
/// Allowed in Draft, Open, or Approved status. If Approved, resets to Open
/// because the base changed and re-review is needed.
pub async fn rebase_change_request(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Path(params): Path<ChangeRequestPathParams>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::Edit,
        &params.id,
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let store = match &state.change_request_store {
        Some(s) => s,
        None => return write_error(StatusCode::NOT_FOUND, "change requests API not enabled"),
    };

    let cr = match store.get_change_request(&params.cr_id).await {
        Ok(cr) => cr,
        Err(OrbflowError::NotFound) => {
            return write_error(StatusCode::NOT_FOUND, "change request not found");
        }
        Err(e) => {
            error!(error = %e, "failed to get change request");
            return write_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to get change request",
            );
        }
    };

    if cr.workflow_id.0 != params.id {
        return write_error(StatusCode::NOT_FOUND, "change request not found");
    }

    if matches!(
        cr.status,
        orbflow_core::ChangeRequestStatus::Merged | orbflow_core::ChangeRequestStatus::Rejected
    ) {
        return write_error(
            StatusCode::CONFLICT,
            format!("cannot rebase a change request in status {:?}", cr.status),
        );
    }

    // Get the current workflow version.
    let wf_id = WorkflowId::new(&params.id);
    let workflow = match state.engine.get_workflow(&wf_id).await {
        Ok(wf) => wf,
        Err(OrbflowError::NotFound) => {
            return write_error(StatusCode::NOT_FOUND, "workflow not found");
        }
        Err(e) => {
            error!(error = %e, "failed to get workflow for rebase");
            return write_error(StatusCode::INTERNAL_SERVER_ERROR, "failed to get workflow");
        }
    };

    let current_version = workflow.version;

    if cr.base_version == current_version {
        return write_data(
            StatusCode::OK,
            serde_json::json!({"status": "already_up_to_date", "base_version": current_version}),
        );
    }

    // If the CR was approved, reset to open since the base changed.
    let new_status = if cr.status == orbflow_core::ChangeRequestStatus::Approved {
        orbflow_core::ChangeRequestStatus::Open
    } else {
        cr.status
    };

    let updated = orbflow_core::ChangeRequest {
        base_version: current_version,
        status: new_status,
        updated_at: Utc::now(),
        ..cr
    };

    match store.update_change_request(&updated).await {
        Ok(()) => write_data(
            StatusCode::OK,
            serde_json::json!({
                "status": "rebased",
                "base_version": current_version,
                "cr_status": new_status,
            }),
        ),
        Err(e) => {
            error!(error = %e, "failed to rebase change request");
            write_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to rebase change request",
            )
        }
    }
}

/// `POST /workflows/{id}/change-requests/{cr_id}/merge`
pub async fn merge_change_request(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Path(params): Path<ChangeRequestPathParams>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::Edit,
        &params.id,
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let store = match &state.change_request_store {
        Some(s) => s,
        None => return write_error(StatusCode::NOT_FOUND, "change requests API not enabled"),
    };

    let cr = match store.get_change_request(&params.cr_id).await {
        Ok(cr) => cr,
        Err(OrbflowError::NotFound) => {
            return write_error(StatusCode::NOT_FOUND, "change request not found");
        }
        Err(e) => {
            error!(error = %e, "failed to get change request");
            return write_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to get change request",
            );
        }
    };

    if cr.workflow_id.0 != params.id {
        return write_error(StatusCode::NOT_FOUND, "change request not found");
    }

    if cr.status != orbflow_core::ChangeRequestStatus::Approved {
        return write_error(
            StatusCode::CONFLICT,
            "change request is not in approved status",
        );
    }

    // Atomic merge: locks CR + workflow rows, checks versions, updates both in one transaction.
    match store
        .merge_change_request(&params.cr_id, cr.base_version, &cr.proposed_definition)
        .await
    {
        Ok(()) => write_data(StatusCode::OK, serde_json::json!({"status": "merged"})),
        Err(OrbflowError::NotFound) => write_error(
            StatusCode::NOT_FOUND,
            "workflow or change request not found",
        ),
        Err(OrbflowError::Conflict) => write_error(
            StatusCode::CONFLICT,
            "base version is stale or change request status changed — reload and retry",
        ),
        Err(e) => {
            error!(error = %e, "failed to merge change request");
            write_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to merge change request",
            )
        }
    }
}

/// `POST /workflows/{id}/change-requests/{cr_id}/comments`
pub async fn add_cr_comment(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Path(params): Path<ChangeRequestPathParams>,
    Json(body): Json<AddCommentBody>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::View,
        &params.id,
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let store = match &state.change_request_store {
        Some(s) => s,
        None => return write_error(StatusCode::NOT_FOUND, "change requests API not enabled"),
    };

    if body.body.trim().is_empty() {
        return write_error(StatusCode::BAD_REQUEST, "comment body must not be empty");
    }
    if body.body.len() > 5000 {
        return write_error(
            StatusCode::BAD_REQUEST,
            "comment body must not exceed 5000 characters",
        );
    }
    if body.author.trim().is_empty() {
        return write_error(StatusCode::BAD_REQUEST, "author must not be empty");
    }
    if body.author.len() > 100 {
        return write_error(
            StatusCode::BAD_REQUEST,
            "author must not exceed 100 characters",
        );
    }

    // Verify the change request exists and belongs to this workflow.
    let cr = match store.get_change_request(&params.cr_id).await {
        Ok(cr) => cr,
        Err(OrbflowError::NotFound) => {
            return write_error(StatusCode::NOT_FOUND, "change request not found");
        }
        Err(e) => {
            error!(error = %e, "failed to get change request");
            return write_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to get change request",
            );
        }
    };

    if cr.workflow_id.0 != params.id {
        return write_error(StatusCode::NOT_FOUND, "change request not found");
    }

    let comment = orbflow_core::ReviewComment {
        id: Uuid::new_v4().to_string(),
        author: body.author,
        body: body.body,
        node_id: body.node_id,
        edge_ref: body.edge_ref,
        resolved: false,
        created_at: Utc::now(),
    };

    match store.add_comment(&params.cr_id, &comment).await {
        Ok(()) => write_data(StatusCode::CREATED, comment),
        Err(OrbflowError::NotFound) => {
            write_error(StatusCode::NOT_FOUND, "change request not found")
        }
        Err(e) => {
            error!(error = %e, "failed to add comment");
            write_error(StatusCode::INTERNAL_SERVER_ERROR, "failed to add comment")
        }
    }
}

/// `POST /workflows/{id}/change-requests/{cr_id}/comments/{comment_id}/resolve`
pub async fn resolve_cr_comment(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Path(params): Path<CommentResolvePathParams>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::Edit,
        &params.id,
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let store = match &state.change_request_store {
        Some(s) => s,
        None => return write_error(StatusCode::NOT_FOUND, "change requests API not enabled"),
    };

    // Verify the change request exists and belongs to this workflow.
    match store.get_change_request(&params.cr_id).await {
        Ok(cr) => {
            if cr.workflow_id.0 != params.id {
                return write_error(StatusCode::NOT_FOUND, "change request not found");
            }
        }
        Err(OrbflowError::NotFound) => {
            return write_error(StatusCode::NOT_FOUND, "change request not found");
        }
        Err(e) => {
            error!(error = %e, "failed to get change request");
            return write_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to get change request",
            );
        }
    }

    match store
        .resolve_comment(&params.cr_id, &params.comment_id, true)
        .await
    {
        Ok(()) => write_data(StatusCode::OK, serde_json::json!({"status": "resolved"})),
        Err(OrbflowError::NotFound) => write_error(StatusCode::NOT_FOUND, "comment not found"),
        Err(e) => {
            error!(error = %e, "failed to resolve comment");
            write_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to resolve comment",
            )
        }
    }
}

// --- Test Suites ---

/// `POST /workflows/{id}/test-suite`
pub async fn run_test_suite(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Path(id): Path<String>,
    Json(suite): Json<orbflow_core::TestSuite>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::Execute,
        &id,
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let wf_id = WorkflowId::new(&id);

    // Validate that the suite's workflow_id matches the path parameter.
    if suite.workflow_id != wf_id {
        return write_error(
            StatusCode::BAD_REQUEST,
            "suite workflow_id does not match path",
        );
    }

    if suite.cases.len() > 100 {
        return write_error(
            StatusCode::BAD_REQUEST,
            format!(
                "test suite exceeds maximum of 100 cases (got {})",
                suite.cases.len()
            ),
        );
    }

    match tokio::time::timeout(
        std::time::Duration::from_secs(300), // 5 minute max for entire suite
        state.engine.run_test_suite(&suite),
    )
    .await
    {
        Ok(Ok(result)) => write_data(StatusCode::OK, result),
        Ok(Err(OrbflowError::NotFound)) => write_error(StatusCode::NOT_FOUND, "workflow not found"),
        Ok(Err(e)) => {
            error!(error = %e, "failed to run test suite");
            write_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to run test suite",
            )
        }
        Err(_) => write_error(
            StatusCode::GATEWAY_TIMEOUT,
            "test suite execution timed out",
        ),
    }
}

/// `POST /workflows/{id}/test-coverage`
pub async fn get_test_coverage(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Path(id): Path<String>,
    Json(suite): Json<orbflow_core::TestSuite>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::View,
        &id,
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let wf_id = WorkflowId::new(&id);

    match state.engine.compute_test_coverage(&wf_id, &suite).await {
        Ok(report) => write_data(StatusCode::OK, report),
        Err(OrbflowError::NotFound) => write_error(StatusCode::NOT_FOUND, "workflow not found"),
        Err(e) => {
            error!(error = %e, "failed to compute test coverage");
            write_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to compute coverage",
            )
        }
    }
}

// --- Budgets ---

/// Request body for creating or updating a budget.
#[derive(Debug, Deserialize)]
pub struct BudgetRequest {
    #[serde(default)]
    pub workflow_id: Option<String>,
    #[serde(default)]
    pub team: Option<String>,
    #[serde(default = "default_budget_period")]
    pub period: BudgetPeriod,
    pub limit_usd: f64,
}

fn default_budget_period() -> BudgetPeriod {
    BudgetPeriod::Monthly
}

/// Query parameters for cost analytics.
#[derive(Debug, Deserialize)]
pub struct CostQuery {
    /// Time range in days (e.g. "30d"). Defaults to 30.
    #[serde(default = "default_cost_range")]
    pub range: String,
    /// Group costs by: "workflow" or "team". Defaults to "workflow".
    #[serde(default = "default_cost_group_by")]
    pub group_by: String,
}

fn default_cost_range() -> String {
    "30d".into()
}

fn default_cost_group_by() -> String {
    "workflow".into()
}

/// `GET /analytics/costs?range=30d&group_by=workflow`
pub async fn get_cost_analytics(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Query(query): Query<CostQuery>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::Admin,
        "*",
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    // Parse range (e.g. "30d" -> 30 days).
    let days: i64 = match query.range.trim_end_matches('d').parse::<i64>() {
        Ok(n) if (1..=365).contains(&n) => n,
        _ => {
            return write_error(
                StatusCode::BAD_REQUEST,
                "invalid range: expected 1-365 days (e.g. '30d')",
            );
        }
    };
    let since = Utc::now() - chrono::Duration::days(days);

    let budget_store = match &state.budget_store {
        Some(s) => s,
        None => return write_error(StatusCode::NOT_IMPLEMENTED, "budget store not configured"),
    };

    match budget_store.list_budgets().await {
        Ok(budgets) => {
            // Compute totals from budget data to match the CostAnalytics frontend type.
            let total_cost_usd: f64 = budgets.iter().map(|b| b.current_usd).sum();
            let workflow_costs: Vec<serde_json::Value> = budgets
                .iter()
                .filter(|b| b.workflow_id.is_some())
                .map(|b| {
                    serde_json::json!({
                        "workflow_id": b.workflow_id,
                        "workflow_name": b.workflow_id.as_deref().unwrap_or("unknown"),
                        "total_cost_usd": b.current_usd,
                        "execution_count": 0,
                        "avg_cost_per_execution": 0.0,
                    })
                })
                .collect();

            let cost_summary = serde_json::json!({
                "total_cost_usd": total_cost_usd,
                "workflow_costs": workflow_costs,
                "period_start": since.to_rfc3339(),
                "period_end": Utc::now().to_rfc3339(),
            });
            write_data(StatusCode::OK, cost_summary)
        }
        Err(e) => write_safe_error(&e),
    }
}

/// `GET /budgets`
pub async fn list_budgets(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::Admin,
        "*",
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let budget_store = match &state.budget_store {
        Some(s) => s,
        None => return write_error(StatusCode::NOT_IMPLEMENTED, "budget store not configured"),
    };

    match budget_store.list_budgets().await {
        Ok(budgets) => write_data(StatusCode::OK, budgets),
        Err(e) => write_safe_error(&e),
    }
}

/// `POST /budgets`
pub async fn create_budget(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Json(body): Json<BudgetRequest>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::Admin,
        "*",
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let budget_store = match &state.budget_store {
        Some(s) => s,
        None => return write_error(StatusCode::NOT_IMPLEMENTED, "budget store not configured"),
    };

    if body.limit_usd <= 0.0 {
        return write_error(StatusCode::BAD_REQUEST, "limit_usd must be positive");
    }

    let now = Utc::now();
    let reset_at = compute_next_reset(now, body.period);

    let budget = AccountBudget {
        id: Uuid::new_v4().to_string(),
        workflow_id: body.workflow_id,
        team: body.team,
        period: body.period,
        limit_usd: body.limit_usd,
        current_usd: 0.0,
        reset_at,
        created_at: now,
    };

    match budget_store.create_budget(&budget).await {
        Ok(()) => write_data(StatusCode::CREATED, &budget),
        Err(e) => write_safe_error(&e),
    }
}

/// `PUT /budgets/{id}`
pub async fn update_budget(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Path(id): Path<String>,
    Json(body): Json<BudgetRequest>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::Admin,
        "*",
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let budget_store = match &state.budget_store {
        Some(s) => s,
        None => return write_error(StatusCode::NOT_IMPLEMENTED, "budget store not configured"),
    };

    if body.limit_usd <= 0.0 {
        return write_error(StatusCode::BAD_REQUEST, "limit_usd must be positive");
    }

    // Fetch existing budget to preserve immutable fields.
    let existing = match budget_store.get_budget(&id).await {
        Ok(b) => b,
        Err(OrbflowError::NotFound) => {
            return write_error(StatusCode::NOT_FOUND, "budget not found");
        }
        Err(e) => {
            return write_safe_error(&e);
        }
    };

    let reset_at = if body.period != existing.period {
        compute_next_reset(Utc::now(), body.period)
    } else {
        existing.reset_at
    };

    let updated = AccountBudget {
        id: existing.id,
        workflow_id: body.workflow_id,
        team: body.team,
        period: body.period,
        limit_usd: body.limit_usd,
        current_usd: existing.current_usd,
        reset_at,
        created_at: existing.created_at,
    };

    match budget_store.update_budget(&updated).await {
        Ok(()) => write_data(StatusCode::OK, &updated),
        Err(e) => write_safe_error(&e),
    }
}

/// `DELETE /budgets/{id}`
pub async fn delete_budget(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Path(id): Path<String>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::Admin,
        "*",
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let budget_store = match &state.budget_store {
        Some(s) => s,
        None => return write_error(StatusCode::NOT_IMPLEMENTED, "budget store not configured"),
    };

    match budget_store.delete_budget(&id).await {
        Ok(()) => write_data(StatusCode::OK, serde_json::json!({"deleted": true})),
        Err(OrbflowError::NotFound) => write_error(StatusCode::NOT_FOUND, "budget not found"),
        Err(e) => write_safe_error(&e),
    }
}

/// Computes the next budget reset timestamp based on the period.
fn compute_next_reset(now: chrono::DateTime<Utc>, period: BudgetPeriod) -> chrono::DateTime<Utc> {
    match period {
        BudgetPeriod::Daily => now + chrono::Duration::days(1),
        BudgetPeriod::Weekly => now + chrono::Duration::weeks(1),
        BudgetPeriod::Monthly => now + chrono::Duration::days(30),
    }
}

// --- Analytics ---

/// Query parameters for analytics endpoints.
#[derive(Debug, Deserialize)]
pub struct AnalyticsParams {
    /// Time range shorthand: "24h", "7d", "30d", "90d". Defaults to "7d".
    #[serde(default = "default_range")]
    pub range: String,
}

fn default_range() -> String {
    "7d".to_string()
}

/// Parses a range string (e.g., "7d", "24h") into a [`TimeRange`].
fn parse_time_range(range: &str) -> Result<TimeRange, String> {
    let now = Utc::now();
    let trimmed = range.trim();

    let start = if let Some(days) = trimmed.strip_suffix('d') {
        let n: i64 = days
            .parse()
            .map_err(|_| format!("invalid range: {trimmed}"))?;
        if n <= 0 || n > 365 {
            return Err(format!("range days must be 1..365, got {n}"));
        }
        now - chrono::Duration::days(n)
    } else if let Some(hours) = trimmed.strip_suffix('h') {
        let n: i64 = hours
            .parse()
            .map_err(|_| format!("invalid range: {trimmed}"))?;
        if n <= 0 || n > 8760 {
            return Err(format!("range hours must be 1..8760, got {n}"));
        }
        now - chrono::Duration::hours(n)
    } else {
        return Err(format!(
            "invalid range format: {trimmed} (use e.g. '7d' or '24h')"
        ));
    };

    Ok(TimeRange { start, end: now })
}

/// `GET /analytics/executions?range=7d`
pub async fn get_execution_analytics(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Query(params): Query<AnalyticsParams>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::View,
        "*",
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let store = match &state.analytics_store {
        Some(s) => Arc::clone(s),
        None => return write_error(StatusCode::SERVICE_UNAVAILABLE, "analytics not available"),
    };

    let range = match parse_time_range(&params.range) {
        Ok(r) => r,
        Err(msg) => return write_error(StatusCode::BAD_REQUEST, msg),
    };

    match store.execution_stats(&range).await {
        Ok(stats) => write_data(StatusCode::OK, stats),
        Err(e) => {
            error!(error = %e, "failed to get execution analytics");
            write_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to get execution analytics",
            )
        }
    }
}

/// `GET /analytics/nodes?range=7d`
pub async fn get_node_analytics(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Query(params): Query<AnalyticsParams>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::View,
        "*",
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let store = match &state.analytics_store {
        Some(s) => Arc::clone(s),
        None => return write_error(StatusCode::SERVICE_UNAVAILABLE, "analytics not available"),
    };

    let range = match parse_time_range(&params.range) {
        Ok(r) => r,
        Err(msg) => return write_error(StatusCode::BAD_REQUEST, msg),
    };

    match store.node_performance(&range).await {
        Ok(nodes) => write_data(StatusCode::OK, nodes),
        Err(e) => {
            error!(error = %e, "failed to get node analytics");
            write_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to get node analytics",
            )
        }
    }
}

/// `GET /analytics/failures?range=7d`
pub async fn get_failure_analytics(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Query(params): Query<AnalyticsParams>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::View,
        "*",
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let store = match &state.analytics_store {
        Some(s) => Arc::clone(s),
        None => return write_error(StatusCode::SERVICE_UNAVAILABLE, "analytics not available"),
    };

    let range = match parse_time_range(&params.range) {
        Ok(r) => r,
        Err(msg) => return write_error(StatusCode::BAD_REQUEST, msg),
    };

    match store.failure_trends(&range).await {
        Ok(trends) => write_data(StatusCode::OK, trends),
        Err(e) => {
            error!(error = %e, "failed to get failure analytics");
            write_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to get failure analytics",
            )
        }
    }
}

// --- Alerts ---

/// Request body for creating or updating an alert rule.
#[derive(Debug, Deserialize)]
pub struct AlertRequest {
    #[serde(default)]
    pub workflow_id: Option<String>,
    pub metric: orbflow_core::AlertMetric,
    pub operator: orbflow_core::AlertOperator,
    pub threshold: f64,
    pub channel: orbflow_core::AlertChannel,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

/// `GET /alerts`
pub async fn list_alerts(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::Admin,
        "*",
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let store = match &state.alert_store {
        Some(s) => Arc::clone(s),
        None => return write_error(StatusCode::SERVICE_UNAVAILABLE, "alerts not available"),
    };

    match store.list_alerts().await {
        Ok(alerts) => write_data(StatusCode::OK, alerts),
        Err(e) => {
            error!(error = %e, "failed to list alerts");
            write_error(StatusCode::INTERNAL_SERVER_ERROR, "failed to list alerts")
        }
    }
}

/// `POST /alerts`
pub async fn create_alert(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Json(body): Json<AlertRequest>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::Admin,
        "*",
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let store = match &state.alert_store {
        Some(s) => Arc::clone(s),
        None => return write_error(StatusCode::SERVICE_UNAVAILABLE, "alerts not available"),
    };

    let rule = AlertRule {
        id: Uuid::new_v4().to_string(),
        workflow_id: body.workflow_id,
        metric: body.metric,
        operator: body.operator,
        threshold: body.threshold,
        channel: body.channel,
        enabled: body.enabled,
        created_at: Utc::now(),
    };

    match store.create_alert(&rule).await {
        Ok(()) => write_data(StatusCode::CREATED, rule),
        Err(e) => {
            error!(error = %e, "failed to create alert");
            write_error(StatusCode::INTERNAL_SERVER_ERROR, "failed to create alert")
        }
    }
}

/// `PUT /alerts/{id}`
pub async fn update_alert(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Path(id): Path<String>,
    Json(body): Json<AlertRequest>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::Admin,
        "*",
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let store = match &state.alert_store {
        Some(s) => Arc::clone(s),
        None => return write_error(StatusCode::SERVICE_UNAVAILABLE, "alerts not available"),
    };

    // Verify the alert exists first.
    let existing = match store.get_alert(&id).await {
        Ok(r) => r,
        Err(OrbflowError::NotFound) => {
            return write_error(StatusCode::NOT_FOUND, "alert not found");
        }
        Err(e) => {
            error!(error = %e, "failed to get alert for update");
            return write_error(StatusCode::INTERNAL_SERVER_ERROR, "failed to update alert");
        }
    };

    let updated = AlertRule {
        id: existing.id,
        workflow_id: body.workflow_id,
        metric: body.metric,
        operator: body.operator,
        threshold: body.threshold,
        channel: body.channel,
        enabled: body.enabled,
        created_at: existing.created_at,
    };

    match store.update_alert(&updated).await {
        Ok(()) => write_data(StatusCode::OK, updated),
        Err(OrbflowError::NotFound) => write_error(StatusCode::NOT_FOUND, "alert not found"),
        Err(e) => {
            error!(error = %e, "failed to update alert");
            write_error(StatusCode::INTERNAL_SERVER_ERROR, "failed to update alert")
        }
    }
}

/// `DELETE /alerts/{id}`
pub async fn delete_alert(
    State(state): State<AppState>,
    axum::Extension(auth_user): axum::Extension<AuthUser>,
    Path(id): Path<String>,
) -> Response {
    if let Err(resp) = check_permission(
        &state.rbac,
        &auth_user.user_id,
        Permission::Admin,
        "*",
        None,
        state.bootstrap_admin.as_deref(),
    ) {
        return resp;
    }

    let store = match &state.alert_store {
        Some(s) => Arc::clone(s),
        None => return write_error(StatusCode::SERVICE_UNAVAILABLE, "alerts not available"),
    };

    match store.delete_alert(&id).await {
        Ok(()) => write_data(StatusCode::OK, serde_json::json!({"deleted": true})),
        Err(OrbflowError::NotFound) => write_error(StatusCode::NOT_FOUND, "alert not found"),
        Err(e) => {
            error!(error = %e, "failed to delete alert");
            write_error(StatusCode::INTERNAL_SERVER_ERROR, "failed to delete alert")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_plugin(name: &str, desc: &str, tags: &[&str], author: &str) -> serde_json::Value {
        serde_json::json!({
            "name": name,
            "description": desc,
            "author": author,
            "tags": tags,
            "downloads": 0,
        })
    }

    #[test]
    fn search_score_exact_name_highest() {
        let p = test_plugin("slack", "Send messages", &["messaging"], "Orbflow");
        assert_eq!(search_score(&p, "slack"), 100); // name exact only
    }

    #[test]
    fn search_score_name_contains() {
        let p = test_plugin("orbflow-slack", "Send messages", &["messaging"], "Orbflow");
        assert_eq!(search_score(&p, "slack"), 50); // name contains only
    }

    #[test]
    fn search_score_tag_match() {
        let p = test_plugin("notifier", "Sends alerts", &["slack", "email"], "Orbflow");
        assert_eq!(search_score(&p, "slack"), 30); // tag only
    }

    #[test]
    fn search_score_description_match() {
        let p = test_plugin("alerter", "Send slack messages", &["notify"], "Orbflow");
        assert_eq!(search_score(&p, "slack"), 10); // desc only
    }

    #[test]
    fn search_score_author_match() {
        let p = test_plugin("plugin", "Does stuff", &[], "SlackTeam");
        assert_eq!(search_score(&p, "slack"), 5); // author only (case-insensitive)
    }

    #[test]
    fn search_score_no_match_returns_zero() {
        let p = test_plugin("database", "Postgres connector", &["sql"], "Orbflow");
        assert_eq!(search_score(&p, "slack"), 0);
    }

    #[test]
    fn search_score_multiple_matches_accumulate() {
        let p = test_plugin("slack-bot", "Send slack messages", &["slack"], "SlackCorp");
        // name contains (50) + tag (30) + desc (10) + author (5) = 95
        assert_eq!(search_score(&p, "slack"), 95);
    }

    #[test]
    fn search_score_case_insensitive() {
        let p = test_plugin("SLACK-BOT", "description", &[], "author");
        assert_eq!(search_score(&p, "slack"), 50); // name contains, case-insensitive
    }

    /// Helper: calls validate_manifest and returns the parsed JSON body.
    fn call_validate(body: serde_json::Value) -> serde_json::Value {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let resp = rt.block_on(validate_manifest(Json(body)));
        let response = resp.into_response();
        assert_eq!(response.status(), StatusCode::OK);
        let body_bytes = rt.block_on(async {
            use http_body_util::BodyExt;
            axum::body::Body::new(response.into_body())
                .collect()
                .await
                .unwrap()
                .to_bytes()
        });
        serde_json::from_slice(&body_bytes).unwrap()
    }

    #[test]
    fn validate_manifest_accepts_valid() {
        let body = serde_json::json!({
            "name": "orbflow-test",
            "version": "1.0.0",
            "description": "A test plugin",
            "author": "Tester",
            "node_types": ["plugin:test"],
            "protocol": "grpc",
        });

        let json = call_validate(body);
        assert_eq!(json["data"]["valid"], true);
    }

    #[test]
    fn validate_manifest_rejects_missing_name() {
        let body = serde_json::json!({
            "version": "1.0.0",
            "description": "A test plugin",
            "author": "Tester",
            "node_types": ["plugin:test"],
            "protocol": "grpc",
        });

        let json = call_validate(body);
        assert_eq!(json["data"]["valid"], false);
        let errors = json["data"]["errors"]
            .as_array()
            .expect("errors should be an array");
        assert!(
            !errors.is_empty(),
            "expected validation errors for missing name"
        );
    }

    #[test]
    fn validate_manifest_rejects_invalid_name_chars() {
        let body = serde_json::json!({
            "name": "bad name!@#",
            "version": "1.0.0",
            "description": "desc",
            "author": "auth",
            "node_types": ["plugin:test"],
            "protocol": "grpc",
        });

        let json = call_validate(body);
        assert_eq!(json["data"]["valid"], false);
        let errors = json["data"]["errors"].as_array().unwrap();
        assert!(
            errors
                .iter()
                .any(|e| e.as_str().unwrap().contains("alphanumeric"))
        );
    }

    #[test]
    fn validate_manifest_rejects_invalid_version() {
        let body = serde_json::json!({
            "name": "test",
            "version": "not-a-version",
            "description": "desc",
            "author": "auth",
            "node_types": ["plugin:test"],
            "protocol": "grpc",
        });

        let json = call_validate(body);
        assert_eq!(json["data"]["valid"], false);
        let errors = json["data"]["errors"].as_array().unwrap();
        assert!(
            errors
                .iter()
                .any(|e| e.as_str().unwrap().contains("semver"))
        );
    }

    #[test]
    fn validate_manifest_rejects_empty_node_types() {
        let body = serde_json::json!({
            "name": "test",
            "version": "1.0.0",
            "description": "desc",
            "author": "auth",
            "node_types": [],
            "protocol": "grpc",
        });

        let json = call_validate(body);
        assert_eq!(json["data"]["valid"], false);
        let errors = json["data"]["errors"].as_array().unwrap();
        assert!(
            errors
                .iter()
                .any(|e| e.as_str().unwrap().contains("at least one"))
        );
    }

    #[test]
    fn validate_manifest_rejects_invalid_protocol() {
        let body = serde_json::json!({
            "name": "test",
            "version": "1.0.0",
            "description": "desc",
            "author": "auth",
            "node_types": ["plugin:test"],
            "protocol": "websocket",
        });

        let json = call_validate(body);
        assert_eq!(json["data"]["valid"], false);
        let errors = json["data"]["errors"].as_array().unwrap();
        assert!(
            errors
                .iter()
                .any(|e| e.as_str().unwrap().contains("protocol"))
        );
    }

    #[test]
    fn validate_manifest_caps_node_types_at_100() {
        let node_types: Vec<String> = (0..150).map(|i| format!("plugin:node-{i}")).collect();
        let body = serde_json::json!({
            "name": "test",
            "version": "1.0.0",
            "description": "desc",
            "author": "auth",
            "node_types": node_types,
            "protocol": "grpc",
        });

        let json = call_validate(body);
        assert_eq!(json["data"]["valid"], false);
        let errors = json["data"]["errors"].as_array().unwrap();
        assert!(
            errors
                .iter()
                .any(|e| e.as_str().unwrap().contains("at most 100"))
        );
    }
}
