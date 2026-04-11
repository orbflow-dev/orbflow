// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Router construction: builds the Axum [`Router`] with all routes, CORS, and tracing.

use std::sync::{Arc, RwLock};

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::http::HeaderValue;
use axum::middleware as axum_middleware;
use axum::routing::{delete, get, post, put};
use orbflow_config::RateLimitConfig;
use orbflow_core::rbac::RbacPolicy;
use orbflow_core::{
    AlertStore, AnalyticsStore, BudgetStore, ChangeRequestStore, CredentialStore, Engine,
    MetricsStore, PluginIndex, PluginManager, RbacStore,
};
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::handlers::{self, AppState};
use crate::middleware::{
    StartRateLimiter, auth_middleware, read_rate_limiter, sensitive_rate_limiter,
    write_rate_limiter,
};

/// Maximum allowed size of a JSON request body (1 MB).
const MAX_REQUEST_BODY_SIZE: usize = 1 << 20;

/// Options for creating the HTTP API router.
pub struct HttpApiOptions {
    /// The engine that handles workflow operations.
    pub engine: Arc<dyn Engine>,
    /// Optional credential store. If `None`, the `/credentials` routes are omitted.
    pub credential_store: Option<Arc<dyn CredentialStore>>,
    /// Optional bus for SSE streaming endpoints. If `None`, streaming routes are omitted.
    pub bus: Option<Arc<dyn orbflow_core::Bus>>,
    /// Optional metrics store for workflow/instance metrics endpoints.
    pub metrics_store: Option<Arc<dyn MetricsStore>>,
    /// Optional bearer token for API authentication.
    ///
    /// When `Some`, all routes except public paths require
    /// `Authorization: Bearer <token>`. When `None`, auth is disabled.
    pub auth_token: Option<String>,
    /// Optional RBAC policy for permission enforcement.
    /// When `Some`, the `/rbac/policy` endpoints are available.
    /// Pass a pre-built `Arc<RwLock<RbacPolicy>>` so that the same Arc can be
    /// shared with a background reload task in multi-instance deployments.
    pub rbac: Option<Arc<RwLock<RbacPolicy>>>,
    /// Optional local plugin index for the marketplace.
    /// When `Some`, the `/marketplace` endpoints are available.
    pub plugin_index: Option<Arc<dyn PluginIndex>>,
    /// Optional plugin installer for marketplace install/uninstall.
    pub plugin_installer: Option<Arc<dyn orbflow_core::PluginInstaller>>,
    /// Optional change request store for PR-style collaboration.
    /// When `Some`, the `/workflows/{id}/change-requests` routes are available.
    pub change_request_store: Option<Arc<dyn ChangeRequestStore>>,
    /// Optional RBAC store for persisting policy changes to the database.
    /// When `Some`, policy updates via PUT /rbac/policy are persisted.
    pub rbac_store: Option<Arc<dyn RbacStore>>,
    /// Optional budget store for cost tracking and budget enforcement.
    /// When `Some`, the `/budgets` and `/analytics/costs` routes are available.
    pub budget_store: Option<Arc<dyn BudgetStore>>,
    /// Optional analytics store for aggregated execution statistics.
    /// When `Some`, the `/analytics/*` routes are available.
    pub analytics_store: Option<Arc<dyn AnalyticsStore>>,
    /// Optional alert store for alert rule management.
    /// When `Some`, the `/alerts` routes are available.
    pub alert_store: Option<Arc<dyn AlertStore>>,
    /// Whether to trust the `X-User-Id` header for user identity.
    ///
    /// When `false` (default), the header is ignored and all requests are
    /// attributed to `"anonymous"`. Set to `true` only when the API is behind
    /// a trusted gateway that injects a verified `X-User-Id` header.
    pub trust_x_user_id: bool,
    /// Bootstrap admin user ID (from `ORBFLOW_BOOTSTRAP_ADMIN` env var).
    /// Read once at startup to avoid per-request syscalls.
    pub bootstrap_admin: Option<String>,
    /// Optional shared plugin process manager for start/stop/restart via API.
    pub plugin_manager: Option<Arc<dyn PluginManager>>,
    /// Optional path to the plugins directory for plugin installation.
    pub plugins_dir: Option<String>,
    /// Allowed CORS origins. When empty, all origins are allowed (dev only).
    pub cors_origins: Vec<String>,
    /// Per-user rate limit configuration for tiered API endpoints.
    /// Uses `RateLimitConfig::default()` when not provided.
    pub rate_limit: RateLimitConfig,
}

/// Creates the Axum [`Router`] with all Orbflow API routes.
///
/// The router includes:
/// - CORS middleware (allow all origins)
/// - tower-http tracing
/// - 1 MB request body limit
/// - Per-user rate limiting via tower-governor (tiered: read/write/sensitive)
/// - All workflow, instance, and credential CRUD routes
pub fn create_router(opts: HttpApiOptions) -> Result<Router, orbflow_core::OrbflowError> {
    // trust_x_user_id is only safe when auth is also configured — without auth,
    // any caller can forge their identity via the X-User-Id header.

    let state = AppState {
        engine: opts.engine,
        credential_store: opts.credential_store.clone(),
        bus: opts.bus,
        rate_limiter: StartRateLimiter::new(),
        metrics_store: opts.metrics_store,
        rbac: opts.rbac,
        plugin_index: opts.plugin_index,
        plugin_installer: opts.plugin_installer,
        change_request_store: opts.change_request_store.clone(),
        rbac_store: opts.rbac_store,
        budget_store: opts.budget_store.clone(),
        analytics_store: opts.analytics_store.clone(),
        alert_store: opts.alert_store.clone(),
        trust_x_user_id: opts.trust_x_user_id,
        bootstrap_admin: opts.bootstrap_admin,
        plugin_manager: opts.plugin_manager,
        plugins_dir: opts.plugins_dir,
        http_client: reqwest::Client::new(),
    };

    // ── Read-only routes (STANDARD rate limit) ──────────────────────────────
    let read_routes = Router::new()
        .route("/health", get(handlers::health_check))
        .route("/node-types", get(handlers::list_node_types))
        .route("/credential-types", get(handlers::list_credential_types))
        .route("/workflows", get(handlers::list_workflows))
        .route("/workflows/{id}", get(handlers::get_workflow))
        .route(
            "/workflows/{id}/versions",
            get(handlers::list_workflow_versions),
        )
        .route(
            "/workflows/{id}/versions/{version}",
            get(handlers::get_workflow_version),
        )
        .route(
            "/workflows/{id}/diff",
            get(handlers::diff_workflow_versions),
        )
        .route("/instances", get(handlers::list_instances))
        .route("/instances/{id}", get(handlers::get_instance))
        .route(
            "/workflows/{id}/metrics",
            get(handlers::get_workflow_metrics),
        )
        .route(
            "/workflows/{id}/metrics/nodes",
            get(handlers::get_workflow_node_metrics),
        )
        .route(
            "/instances/{id}/metrics",
            get(handlers::get_instance_metrics),
        )
        .route(
            "/instances/{id}/audit/verify",
            get(handlers::verify_instance_audit),
        )
        .route(
            "/instances/{id}/audit/trail",
            get(handlers::get_audit_trail),
        )
        .route(
            "/instances/{id}/audit/proof/{event_index}",
            get(handlers::get_audit_proof),
        )
        .route(
            "/instances/{id}/audit/export",
            get(handlers::export_audit_trail),
        )
        .route(
            "/instances/{instance_id}/nodes/{node_id}/stream",
            get(handlers::stream_node),
        )
        .route("/rbac/policy", get(handlers::get_rbac_policy))
        .route("/rbac/subjects", get(handlers::list_rbac_subjects))
        .route(
            "/marketplace/plugins",
            get(handlers::list_installed_plugins),
        )
        .route(
            "/marketplace/plugins/{name}",
            get(handlers::get_installed_plugin),
        )
        .route("/plugins/status", get(handlers::list_plugin_status))
        .route("/plugins/{name}/status", get(handlers::get_plugin_status))
        .layer(read_rate_limiter(
            opts.rate_limit.read_per_ms,
            opts.rate_limit.read_burst,
        )?);

    // ── Write routes (WRITE rate limit) ─────────────────────────────────────
    let mut write_routes = Router::new()
        .route("/workflows", post(handlers::create_workflow))
        .route("/workflows/{id}", put(handlers::update_workflow))
        .route("/workflows/{id}", delete(handlers::delete_workflow))
        .route("/workflows/{id}/start", post(handlers::start_workflow))
        .route("/workflows/{id}/test-node", post(handlers::test_node))
        .route("/workflows/{id}/test-suite", post(handlers::run_test_suite))
        .route(
            "/workflows/{id}/test-coverage",
            post(handlers::get_test_coverage),
        )
        .route(
            "/marketplace/validate-manifest",
            post(handlers::validate_manifest),
        )
        .route("/instances/{id}/cancel", post(handlers::cancel_instance))
        .route(
            "/instances/{instance_id}/nodes/{node_id}/approve",
            post(handlers::approve_node),
        )
        .route(
            "/instances/{instance_id}/nodes/{node_id}/reject",
            post(handlers::reject_node),
        );

    // Credentials (only if a credential store is configured).
    if opts.credential_store.is_some() {
        write_routes = write_routes
            .route("/credentials", post(handlers::create_credential))
            .route("/credentials", get(handlers::list_credentials))
            .route("/credentials/{id}", get(handlers::get_credential))
            .route("/credentials/{id}", put(handlers::update_credential))
            .route("/credentials/{id}", delete(handlers::delete_credential));
    }

    // Change requests (only if a change request store is configured).
    if opts.change_request_store.is_some() {
        write_routes = write_routes
            .route(
                "/workflows/{id}/change-requests",
                post(handlers::create_change_request).get(handlers::list_change_requests),
            )
            .route(
                "/workflows/{id}/change-requests/{cr_id}",
                get(handlers::get_change_request).put(handlers::update_change_request),
            )
            .route(
                "/workflows/{id}/change-requests/{cr_id}/submit",
                post(handlers::submit_change_request),
            )
            .route(
                "/workflows/{id}/change-requests/{cr_id}/approve",
                post(handlers::approve_change_request),
            )
            .route(
                "/workflows/{id}/change-requests/{cr_id}/reject",
                post(handlers::reject_change_request),
            )
            .route(
                "/workflows/{id}/change-requests/{cr_id}/rebase",
                post(handlers::rebase_change_request),
            )
            .route(
                "/workflows/{id}/change-requests/{cr_id}/merge",
                post(handlers::merge_change_request),
            )
            .route(
                "/workflows/{id}/change-requests/{cr_id}/comments",
                post(handlers::add_cr_comment),
            )
            .route(
                "/workflows/{id}/change-requests/{cr_id}/comments/{comment_id}/resolve",
                post(handlers::resolve_cr_comment),
            );
    }

    // Analytics (only if an analytics store is configured).
    if opts.analytics_store.is_some() {
        write_routes = write_routes
            .route(
                "/analytics/executions",
                get(handlers::get_execution_analytics),
            )
            .route("/analytics/nodes", get(handlers::get_node_analytics))
            .route("/analytics/failures", get(handlers::get_failure_analytics));
    }

    let write_routes = write_routes.layer(write_rate_limiter(
        opts.rate_limit.write_per_ms,
        opts.rate_limit.write_burst,
    )?);

    // ── Sensitive routes (SENSITIVE rate limit) ─────────────────────────────
    let mut sensitive_routes = Router::new()
        .route("/rbac/policy", put(handlers::update_rbac_policy))
        .route("/plugins/{name}/start", post(handlers::start_plugin))
        .route("/plugins/{name}/stop", post(handlers::stop_plugin))
        .route("/plugins/{name}/restart", post(handlers::restart_plugin))
        .route("/plugins/reload", post(handlers::reload_all_plugins))
        .route(
            "/marketplace/plugins/{name}/install",
            post(handlers::install_plugin),
        )
        .route(
            "/marketplace/plugins/{name}",
            delete(handlers::uninstall_plugin),
        );

    // Alert rules (only if an alert store is configured).
    if opts.alert_store.is_some() {
        sensitive_routes = sensitive_routes
            .route("/alerts", get(handlers::list_alerts))
            .route("/alerts", post(handlers::create_alert))
            .route("/alerts/{id}", put(handlers::update_alert))
            .route("/alerts/{id}", delete(handlers::delete_alert));
    }

    // Budget management (only if a budget store is configured).
    if opts.budget_store.is_some() {
        sensitive_routes = sensitive_routes
            .route("/analytics/costs", get(handlers::get_cost_analytics))
            .route("/budgets", get(handlers::list_budgets))
            .route("/budgets", post(handlers::create_budget))
            .route("/budgets/{id}", put(handlers::update_budget))
            .route("/budgets/{id}", delete(handlers::delete_budget));
    }

    let sensitive_routes = sensitive_routes.layer(sensitive_rate_limiter(
        opts.rate_limit.sensitive_per_sec,
        opts.rate_limit.sensitive_burst,
    )?);

    // ── Merge all route groups ──────────────────────────────────────────────
    let app = Router::new()
        .merge(read_routes)
        .merge(write_routes)
        .merge(sensitive_routes);

    let auth_token = opts.auth_token.clone();
    let trust_x_user_id = if opts.trust_x_user_id && opts.auth_token.is_none() {
        return Err(orbflow_core::OrbflowError::InvalidNodeConfig(
            "trust_x_user_id=true requires auth_token to be configured. \
             Without auth, any caller can forge X-User-Id identity."
                .into(),
        ));
    } else if opts.trust_x_user_id {
        tracing::warn!(
            "trust_x_user_id is enabled — X-User-Id header will be trusted for identity. \
             Only enable this behind a trusted API gateway."
        );
        true
    } else {
        false
    };

    // Nest all API routes under /api/v1 for forward-compatible versioning.
    // Health check is also available at root /health for load balancer probes.
    let versioned = Router::new()
        .route("/health", get(handlers::health_check))
        .nest("/api/v1", app);

    Ok(versioned
        .layer(axum_middleware::from_fn(move |req, next| {
            auth_middleware(req, next, auth_token.clone(), trust_x_user_id)
        }))
        .layer(DefaultBodyLimit::max(MAX_REQUEST_BODY_SIZE))
        .layer(build_cors_layer(&opts.cors_origins))
        .layer(axum_middleware::from_fn(security_headers))
        .layer(TraceLayer::new_for_http())
        .with_state(state))
}

/// Builds the CORS layer from the configured allowed origins.
///
/// - `["*"]`: allows all origins (development only — logged as warning).
/// - Specific origins: only those origins are allowed (recommended for production).
/// - **Empty list**: denies all cross-origin requests (safe default).
fn build_cors_layer(cors_origins: &[String]) -> CorsLayer {
    if cors_origins.is_empty() {
        tracing::info!(
            "CORS: no origins configured — all cross-origin requests will be denied. \
             Set server.cors_origins in config to allow specific origins."
        );
        // Return a CORS layer with no allowed origins — browsers will block
        // cross-origin requests because the response lacks the
        // Access-Control-Allow-Origin header.
        return CorsLayer::new()
            .allow_methods([
                axum::http::Method::GET,
                axum::http::Method::POST,
                axum::http::Method::PUT,
                axum::http::Method::DELETE,
                axum::http::Method::OPTIONS,
            ])
            .allow_headers([
                axum::http::header::AUTHORIZATION,
                axum::http::header::CONTENT_TYPE,
            ]);
    }

    let is_wildcard = cors_origins.len() == 1 && cors_origins[0] == "*";

    if is_wildcard {
        tracing::warn!(
            "CORS: allowing all origins — set server.cors_origins to specific origins for production"
        );
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods([
                axum::http::Method::GET,
                axum::http::Method::POST,
                axum::http::Method::PUT,
                axum::http::Method::DELETE,
                axum::http::Method::OPTIONS,
            ])
            .allow_headers([
                axum::http::header::AUTHORIZATION,
                axum::http::header::CONTENT_TYPE,
            ])
    } else {
        let origins: Vec<axum::http::HeaderValue> =
            cors_origins
                .iter()
                .filter_map(|o| {
                    o.parse().map_err(|e| {
                    tracing::warn!(origin = %o, error = %e, "invalid CORS origin — skipping");
                    e
                }).ok()
                })
                .collect();

        CorsLayer::new()
            .allow_origin(AllowOrigin::list(origins))
            .allow_methods([
                axum::http::Method::GET,
                axum::http::Method::POST,
                axum::http::Method::PUT,
                axum::http::Method::DELETE,
                axum::http::Method::OPTIONS,
            ])
            .allow_headers([
                axum::http::header::AUTHORIZATION,
                axum::http::header::CONTENT_TYPE,
            ])
    }
}

/// Middleware that appends standard security response headers in a single pass.
async fn security_headers(
    req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let mut resp = next.run(req).await;
    let headers = resp.headers_mut();
    headers.insert(
        axum::http::header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        axum::http::header::X_FRAME_OPTIONS,
        HeaderValue::from_static("DENY"),
    );
    headers.insert(
        axum::http::header::HeaderName::from_static("x-xss-protection"),
        HeaderValue::from_static("1; mode=block"),
    );
    headers.insert(
        axum::http::header::HeaderName::from_static("referrer-policy"),
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    headers.insert(
        axum::http::header::HeaderName::from_static("strict-transport-security"),
        HeaderValue::from_static("max-age=63072000; includeSubDomains"),
    );
    headers.insert(
        axum::http::header::HeaderName::from_static("content-security-policy"),
        HeaderValue::from_static("default-src 'none'; frame-ancestors 'none'"),
    );
    resp
}
