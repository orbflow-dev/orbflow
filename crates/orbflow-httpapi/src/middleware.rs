// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! HTTP middleware: API rate limiter (via tower-governor), bearer token auth,
//! and RBAC permission enforcement.

use std::sync::{Arc, RwLock};

use axum::Json;
use axum::body::{Body, to_bytes};
use axum::http::header::{CONTENT_LENGTH, CONTENT_TYPE};
use axum::http::{Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use governor::middleware::StateInformationMiddleware;
use orbflow_core::OrbflowError;
use orbflow_core::rbac::{Permission, RbacPolicy};
use serde::Serialize;
use tower_governor::GovernorLayer;
use tower_governor::errors::GovernorError;
use tower_governor::governor::GovernorConfigBuilder;
use tower_governor::key_extractor::KeyExtractor;

/// Concrete layer type returned by our rate limiter constructors.
pub type RateLimiterLayer =
    GovernorLayer<UserKeyExtractor, StateInformationMiddleware, axum::body::Body>;

// ─── Rate Limiting ──────────────────────────────────────────────────────────

/// Extracts the user identity from the `AuthUser` extension (set by auth
/// middleware) for per-user rate limiting. Falls back to `"anonymous"`.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct UserKeyExtractor;

impl KeyExtractor for UserKeyExtractor {
    type Key = String;

    fn name(&self) -> &'static str {
        "UserKeyExtractor"
    }

    fn extract<B>(&self, req: &Request<B>) -> Result<Self::Key, GovernorError> {
        Ok(req
            .extensions()
            .get::<AuthUser>()
            .map(|u| u.user_id.clone())
            .unwrap_or_else(|| "anonymous".to_owned()))
    }

    fn key_name(&self, key: &Self::Key) -> Option<String> {
        Some(key.clone())
    }
}

/// Creates a `GovernorLayer` for **read** endpoints (GET requests).
///
/// `per_ms`: replenish interval in milliseconds (default 300).
/// `burst`: maximum burst size (default 200).
///
/// Returns an error if the governor configuration is invalid (e.g. zero values).
pub fn read_rate_limiter(per_ms: u64, burst: u32) -> Result<RateLimiterLayer, OrbflowError> {
    let config = GovernorConfigBuilder::default()
        .per_millisecond(per_ms)
        .burst_size(burst)
        .key_extractor(UserKeyExtractor)
        .use_headers()
        .finish()
        .ok_or_else(|| OrbflowError::Internal("invalid read rate limiter config".into()))?;

    Ok(GovernorLayer::new(config))
}

/// Creates a `GovernorLayer` for **write** endpoints (POST/PUT/DELETE).
///
/// `per_ms`: replenish interval in milliseconds (default 600).
/// `burst`: maximum burst size (default 100).
///
/// Returns an error if the governor configuration is invalid (e.g. zero values).
pub fn write_rate_limiter(per_ms: u64, burst: u32) -> Result<RateLimiterLayer, OrbflowError> {
    let config = GovernorConfigBuilder::default()
        .per_millisecond(per_ms)
        .burst_size(burst)
        .key_extractor(UserKeyExtractor)
        .use_headers()
        .finish()
        .ok_or_else(|| OrbflowError::Internal("invalid write rate limiter config".into()))?;

    Ok(GovernorLayer::new(config))
}

/// Creates a `GovernorLayer` for **sensitive/admin** endpoints
/// (plugin lifecycle, RBAC, alerts, budgets mutations).
///
/// `per_sec`: replenish rate in requests per second (default 2).
/// `burst`: maximum burst size (default 30).
///
/// Returns an error if the governor configuration is invalid (e.g. zero values).
pub fn sensitive_rate_limiter(per_sec: u64, burst: u32) -> Result<RateLimiterLayer, OrbflowError> {
    let config = GovernorConfigBuilder::default()
        .per_second(per_sec)
        .burst_size(burst)
        .key_extractor(UserKeyExtractor)
        .use_headers()
        .finish()
        .ok_or_else(|| OrbflowError::Internal("invalid sensitive rate limiter config".into()))?;

    Ok(GovernorLayer::new(config))
}

// ─── Error Envelope Normalization ──────────────────────────────────────────

const ERROR_ENVELOPE_BODY_LIMIT: usize = 64 * 1024;

#[derive(Serialize)]
struct ErrorEnvelopeBody {
    data: Option<()>,
    error: String,
}

pub async fn error_envelope_middleware(req: Request<Body>, next: Next) -> Response {
    let resp = next.run(req).await;
    let status = resp.status();

    if !status.is_client_error() && !status.is_server_error() {
        return resp;
    }

    let (parts, body) = resp.into_parts();
    let bytes = match to_bytes(body, ERROR_ENVELOPE_BODY_LIMIT).await {
        Ok(bytes) => bytes,
        Err(e) => {
            tracing::warn!(error = %e, "failed to read error response body for envelope normalization");
            return (
                status,
                Json(ErrorEnvelopeBody {
                    data: None,
                    error: status
                        .canonical_reason()
                        .unwrap_or("request failed")
                        .to_ascii_lowercase(),
                }),
            )
                .into_response();
        }
    };

    if is_error_envelope(&bytes) {
        return Response::from_parts(parts, Body::from(bytes));
    }

    let message = error_message_from_body(status, &bytes);
    let mut normalized = (
        status,
        Json(ErrorEnvelopeBody {
            data: None,
            error: message,
        }),
    )
        .into_response();

    for (name, value) in parts.headers.iter() {
        if name != CONTENT_TYPE && name != CONTENT_LENGTH {
            normalized.headers_mut().insert(name.clone(), value.clone());
        }
    }

    normalized
}

fn is_error_envelope(bytes: &[u8]) -> bool {
    serde_json::from_slice::<serde_json::Value>(bytes)
        .ok()
        .and_then(|value| {
            let object = value.as_object()?;
            Some(object.contains_key("data") && object.contains_key("error"))
        })
        .unwrap_or(false)
}

fn error_message_from_body(status: StatusCode, bytes: &[u8]) -> String {
    if status == StatusCode::PAYLOAD_TOO_LARGE {
        return "request body too large".to_string();
    }

    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(bytes)
        && let Some(error) = value.get("error").and_then(|v| v.as_str())
        && !error.trim().is_empty()
    {
        return error.trim().to_string();
    }

    let text = String::from_utf8_lossy(bytes).trim().to_string();
    if !text.is_empty() {
        return text;
    }

    status
        .canonical_reason()
        .unwrap_or("request failed")
        .to_ascii_lowercase()
}

// ─── Legacy StartRateLimiter (per-workflow inline check) ────────────────────

use dashmap::DashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Minimum interval between consecutive starts of the same workflow.
const START_RATE_LIMIT_WINDOW: Duration = Duration::from_secs(2);

/// Per-workflow rate limiter for the `/start` endpoint.
///
/// Uses a [`DashMap`] to store the last start timestamp per workflow ID.
/// This is checked inline in the handler (not as tower middleware) because
/// it needs the workflow ID from the path parameter.
#[derive(Clone)]
pub struct StartRateLimiter {
    last_start: DashMap<String, Instant>,
    /// Guard to prevent multiple concurrent eviction passes.
    evicting: Arc<AtomicBool>,
}

impl StartRateLimiter {
    pub fn new() -> Self {
        Self {
            last_start: DashMap::new(),
            evicting: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Maximum number of entries before triggering eviction of stale entries.
    const EVICTION_THRESHOLD: usize = 10_000;

    #[allow(clippy::result_large_err)]
    pub fn check(&self, workflow_id: &str) -> Result<(), Response> {
        let now = Instant::now();

        // Evict stale entries when the map grows too large to prevent unbounded
        // memory growth from unique workflow IDs. The AtomicBool guard ensures
        // only one thread runs eviction at a time — concurrent callers skip
        // rather than blocking on DashMap shard write-locks.
        if self.last_start.len() > Self::EVICTION_THRESHOLD
            && !self.evicting.swap(true, Ordering::Relaxed)
        {
            self.last_start
                .retain(|_, last| now.duration_since(*last) < START_RATE_LIMIT_WINDOW * 2);
            self.evicting.store(false, Ordering::Relaxed);
        }

        if self
            .last_start
            .get(workflow_id)
            .is_some_and(|last| now.duration_since(*last) < START_RATE_LIMIT_WINDOW)
        {
            return Err((
                StatusCode::TOO_MANY_REQUESTS,
                Json(RateLimitBody {
                    error: "rate limit: workflow was started recently, please wait".into(),
                }),
            )
                .into_response());
        }

        self.last_start.insert(workflow_id.to_owned(), now);
        Ok(())
    }
}

impl Default for StartRateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Serialize)]
struct RateLimitBody {
    error: String,
}

#[allow(clippy::result_large_err)]
/// Extracts and validates a workflow ID from the path, then checks rate limit.
pub async fn check_start_rate_limit(
    limiter: &StartRateLimiter,
    workflow_id: &str,
) -> Result<(), Response> {
    limiter.check(workflow_id)
}

// ─── Authentication ─────────────────────────────────────────────────────────

/// Paths that are always publicly accessible, regardless of auth configuration.
///
/// NOTE: `/webhooks/` is intentionally excluded — webhook endpoints enforce
/// their own bearer-token or signature checks. Public access to `/webhooks/`
/// would allow unauthenticated workflow execution.
const PUBLIC_PATHS: &[&str] = &[
    "/health",
    "/api/v1/health",
    "/node-types",
    "/api/v1/node-types",
    "/credential-types",
    "/api/v1/credential-types",
];

/// Authenticated user identity stored in request extensions by the auth middleware.
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: String,
}

/// Bearer token authentication middleware.
///
/// Identity (`AuthUser`) is only inserted into request extensions AFTER the
/// bearer token check passes (or when no token is configured). This prevents
/// unauthenticated requests from carrying a spoofed `X-User-Id` identity.
pub async fn auth_middleware(
    mut req: Request<Body>,
    next: Next,
    token: Option<String>,
    trust_x_user_id: bool,
) -> Response {
    let Some(ref expected) = token else {
        // No auth configured — attach identity and proceed.
        insert_user_identity(&mut req, trust_x_user_id);
        return next.run(req).await;
    };

    let path = req.uri().path();
    if PUBLIC_PATHS.contains(&path) {
        insert_user_identity(&mut req, trust_x_user_id);
        return next.run(req).await;
    }

    let provided = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "));

    match provided {
        Some(tok) if constant_time_eq(tok.as_bytes(), expected.as_bytes()) => {
            insert_user_identity(&mut req, trust_x_user_id);
            next.run(req).await
        }
        _ => unauthorized_response(),
    }
}

/// Inserts `AuthUser` into request extensions from `X-User-Id` header (if
/// trusted) or defaults to `"anonymous"`.
fn insert_user_identity(req: &mut Request<Body>, trust_x_user_id: bool) {
    let user_id = if trust_x_user_id {
        req.headers()
            .get("X-User-Id")
            .and_then(|v| v.to_str().ok())
            .filter(|v| {
                !v.is_empty()
                    && v.len() <= 128
                    && v.chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
            })
            .unwrap_or("anonymous")
            .to_owned()
    } else {
        "anonymous".to_owned()
    };
    req.extensions_mut().insert(AuthUser { user_id });
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    use subtle::ConstantTimeEq;
    a.ct_eq(b).into()
}

#[derive(Serialize)]
struct UnauthorizedBody {
    data: Option<()>,
    error: String,
}

fn unauthorized_response() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(UnauthorizedBody {
            data: None,
            error: "unauthorized".into(),
        }),
    )
        .into_response()
}

// ─── RBAC Permission Enforcement ─────────────────────────────────────────────

#[derive(Serialize)]
struct ForbiddenBody {
    data: Option<()>,
    error: String,
}

fn forbidden_response() -> Response {
    (
        StatusCode::FORBIDDEN,
        Json(ForbiddenBody {
            data: None,
            error: "Forbidden: insufficient permissions".into(),
        }),
    )
        .into_response()
}

#[allow(clippy::result_large_err)]
pub fn check_permission(
    rbac_policy: &Option<Arc<RwLock<RbacPolicy>>>,
    user_id: &str,
    permission: Permission,
    workflow_id: &str,
    node_id: Option<&str>,
    bootstrap_admin: Option<&str>,
) -> Result<(), Response> {
    if has_permission(
        rbac_policy,
        user_id,
        permission,
        workflow_id,
        node_id,
        bootstrap_admin,
    )? {
        Ok(())
    } else {
        Err(forbidden_response())
    }
}

#[allow(clippy::result_large_err)]
pub fn has_permission(
    rbac_policy: &Option<Arc<RwLock<RbacPolicy>>>,
    user_id: &str,
    permission: Permission,
    workflow_id: &str,
    node_id: Option<&str>,
    bootstrap_admin: Option<&str>,
) -> Result<bool, Response> {
    let Some(policy_lock) = rbac_policy else {
        return Ok(true);
    };

    let policy = policy_lock.read().map_err(|_| {
        tracing::error!("RBAC policy lock is poisoned — returning 500");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"data": null, "error": "Internal server error"})),
        )
            .into_response()
    })?;

    if policy.bindings.is_empty() {
        if bootstrap_admin.is_some_and(|admin| user_id == admin) {
            return Ok(true);
        }
        return Ok(false);
    }

    if policy.has_permission(user_id, permission, workflow_id, node_id) {
        Ok(true)
    } else {
        Ok(false)
    }
}
