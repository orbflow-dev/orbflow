// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Webhook trigger: Axum handler that matches incoming HTTP requests to workflows.

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::DefaultBodyLimit;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use dashmap::DashMap;
use hmac::{Hmac, Mac};
use orbflow_core::workflow::WorkflowId;
use sha2::Sha256;
use subtle::ConstantTimeEq;
use tracing::{info, warn};

use crate::TriggerCallback;

type HmacSha256 = Hmac<Sha256>;

/// Maximum allowed webhook request body size (256 KB).
const MAX_WEBHOOK_BODY_SIZE: usize = 256 * 1024;

/// Maximum webhook requests per workflow per rate-limit window.
const WEBHOOK_RATE_LIMIT: u32 = 30;
/// Rate-limit window duration.
const WEBHOOK_RATE_WINDOW: std::time::Duration = std::time::Duration::from_secs(60);

/// Webhook handler state shared across requests.
#[derive(Clone)]
pub struct WebhookState {
    /// Maps route key -> (workflow ID, HMAC secret).
    routes: Arc<DashMap<String, (WorkflowId, Option<String>)>>,
    /// Per-workflow rate limiter: workflow_id -> (count, window_start).
    rate_limits: Arc<DashMap<String, (u32, std::time::Instant)>>,
    fire: TriggerCallback,
}

/// Provides HTTP endpoints for webhook triggers.
///
/// Webhooks are accessible at `/webhooks/{workflow_id}` or
/// `/webhooks/{workflow_id}/{path}`.
pub struct WebhookHandler {
    routes: Arc<DashMap<String, (WorkflowId, Option<String>)>>,
    fire: TriggerCallback,
}

impl WebhookHandler {
    /// Creates a new webhook handler.
    pub fn new(fire: TriggerCallback) -> Self {
        Self {
            routes: Arc::new(DashMap::new()),
            fire,
        }
    }

    /// Registers a webhook path for a workflow with an optional HMAC secret.
    ///
    /// When `secret` is `Some`, incoming requests must include a valid
    /// `X-Orbflow-Signature` header containing the hex-encoded HMAC-SHA256
    /// of the request body. Requests without a valid signature are rejected
    /// with HTTP 401.
    pub fn register(&self, workflow_id: &WorkflowId, path: &str) {
        self.register_with_secret(workflow_id, path, None);
    }

    /// Registers a webhook path with HMAC-SHA256 signature verification.
    pub fn register_with_secret(
        &self,
        workflow_id: &WorkflowId,
        path: &str,
        secret: Option<String>,
    ) {
        let key = webhook_key(workflow_id, path);
        self.routes
            .insert(key.clone(), (workflow_id.clone(), secret));
        info!(
            workflow = %workflow_id,
            path = %key,
            "webhook route registered"
        );
    }

    /// Removes all webhook routes for a workflow.
    pub fn remove(&self, workflow_id: &WorkflowId) {
        let wf_str = workflow_id.to_string();
        self.routes.retain(|_, (v, _)| v.to_string() != wf_str);
    }

    /// Returns an Axum [`Router`] that handles incoming webhook requests.
    pub fn router(&self) -> Router {
        let state = WebhookState {
            routes: Arc::clone(&self.routes),
            rate_limits: Arc::new(DashMap::new()),
            fire: Arc::clone(&self.fire),
        };

        Router::new()
            .route("/webhooks/{workflow_id}", get(handle_webhook))
            .route("/webhooks/{workflow_id}", post(handle_webhook))
            .route(
                "/webhooks/{workflow_id}/{*path}",
                get(handle_webhook_with_path),
            )
            .route(
                "/webhooks/{workflow_id}/{*path}",
                post(handle_webhook_with_path),
            )
            .layer(DefaultBodyLimit::max(MAX_WEBHOOK_BODY_SIZE))
            .with_state(state)
    }
}

/// Builds the route key from workflow ID and optional path.
fn webhook_key(workflow_id: &WorkflowId, path: &str) -> String {
    let trimmed = path.trim_matches('/');
    if trimmed.is_empty() {
        workflow_id.to_string()
    } else {
        format!("{}/{}", workflow_id, trimmed)
    }
}

/// Query parameters forwarded as part of the webhook payload.
type WebhookQuery = HashMap<String, String>;

/// Handles a webhook request without an extra path segment.
async fn handle_webhook(
    State(state): State<WebhookState>,
    headers: HeaderMap,
    Path(workflow_id): Path<String>,
    Query(query): Query<WebhookQuery>,
    body: axum::body::Bytes,
) -> Response {
    let key = workflow_id.clone();
    process_webhook(&state, &key, &headers, query, body).await
}

/// Handles a webhook request with an extra path segment.
async fn handle_webhook_with_path(
    State(state): State<WebhookState>,
    headers: HeaderMap,
    Path((workflow_id, path)): Path<(String, String)>,
    Query(query): Query<WebhookQuery>,
    body: axum::body::Bytes,
) -> Response {
    let trimmed = path.trim_matches('/');
    let key = if trimmed.is_empty() {
        workflow_id
    } else {
        format!("{}/{}", workflow_id, trimmed)
    };
    process_webhook(&state, &key, &headers, query, body).await
}

/// Verifies the HMAC-SHA256 signature of the request body.
fn verify_signature(secret: &str, body_bytes: &[u8], signature_header: &str) -> bool {
    // Signature header format: "sha256=<hex-encoded HMAC>"
    let sig_hex = signature_header
        .strip_prefix("sha256=")
        .unwrap_or(signature_header);

    let Ok(mut mac) = HmacSha256::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    mac.update(body_bytes);
    let expected = mac.finalize().into_bytes();

    // Decode the provided hex signature.
    let Ok(provided) = hex_decode(sig_hex) else {
        return false;
    };

    expected.as_slice().ct_eq(&provided).into()
}

/// Decodes a hex string to bytes.
fn hex_decode(s: &str) -> Result<Vec<u8>, ()> {
    if s.len() % 2 != 0 {
        return Err(());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| ()))
        .collect()
}

/// Core webhook processing: look up route, verify signature, merge payload, fire trigger.
async fn process_webhook(
    state: &WebhookState,
    key: &str,
    headers: &HeaderMap,
    query: WebhookQuery,
    raw_body: axum::body::Bytes,
) -> Response {
    let (wf_id, secret) = match state.routes.get(key) {
        Some(entry) => entry.value().clone(),
        None => {
            warn!(key = %key, "no webhook route found");
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "webhook not found"})),
            )
                .into_response();
        }
    };

    // Verify HMAC-SHA256 signature against the raw request bytes (not re-serialized).
    if let Some(ref secret) = secret {
        let sig_header = headers
            .get("x-orbflow-signature")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        if sig_header.is_empty() {
            warn!(key = %key, "webhook request missing X-Orbflow-Signature header");
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "missing signature"})),
            )
                .into_response();
        }

        if !verify_signature(secret, &raw_body, sig_header) {
            warn!(key = %key, "webhook signature verification failed");
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "invalid signature"})),
            )
                .into_response();
        }
    }

    // Deserialize body after signature verification.
    let body: Option<HashMap<String, serde_json::Value>> = if raw_body.is_empty() {
        None
    } else {
        match serde_json::from_slice(&raw_body) {
            Ok(parsed) => Some(parsed),
            Err(e) => {
                warn!(key = %key, error = %e, "webhook body is not valid JSON");
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": "invalid JSON body"})),
                )
                    .into_response();
            }
        }
    };

    // Per-workflow rate limiting: simple sliding window counter.
    {
        let now = std::time::Instant::now();
        let wf_key = wf_id.to_string();

        // Periodically evict expired entries to prevent unbounded memory growth.
        // Run eviction roughly every 100 entries to amortize cost.
        if state.rate_limits.len() > 100 {
            state.rate_limits.retain(|_, (_, window_start)| {
                now.duration_since(*window_start) < WEBHOOK_RATE_WINDOW * 2
            });
        }

        let mut entry = state.rate_limits.entry(wf_key).or_insert((0, now));
        let (count, window_start) = entry.value_mut();
        if now.duration_since(*window_start) >= WEBHOOK_RATE_WINDOW {
            *count = 0;
            *window_start = now;
        }
        *count += 1;
        if *count > WEBHOOK_RATE_LIMIT {
            warn!(
                workflow = %wf_id,
                key = %key,
                "webhook rate limit exceeded"
            );
            return (
                StatusCode::TOO_MANY_REQUESTS,
                Json(serde_json::json!({"error": "rate limit exceeded"})),
            )
                .into_response();
        }
    }

    // Merge body and query parameters into a single payload.
    let mut payload: HashMap<String, serde_json::Value> = body.unwrap_or_default();

    for (k, v) in query {
        payload.insert(k, serde_json::Value::String(v));
    }

    info!(
        workflow = %wf_id,
        key = %key,
        "webhook trigger fired"
    );

    let fire = Arc::clone(&state.fire);
    let wf_id_clone = wf_id.clone();
    tokio::spawn(async move {
        fire(wf_id_clone, orbflow_core::TriggerType::Webhook, payload).await;
    });

    (
        StatusCode::OK,
        Json(serde_json::json!({"status": "triggered", "workflow_id": wf_id.to_string()})),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noop_callback() -> TriggerCallback {
        Arc::new(|_wf, _tt, _payload| Box::pin(async {}))
    }

    fn wf_id(s: &str) -> WorkflowId {
        WorkflowId(s.to_string())
    }

    #[test]
    fn webhook_key_without_path() {
        let key = webhook_key(&wf_id("abc-123"), "");
        assert_eq!(key, "abc-123");
    }

    #[test]
    fn webhook_key_with_path() {
        let key = webhook_key(&wf_id("abc-123"), "notify");
        assert_eq!(key, "abc-123/notify");
    }

    #[test]
    fn webhook_key_trims_slashes() {
        let key = webhook_key(&wf_id("abc"), "/some/path/");
        assert_eq!(key, "abc/some/path");
    }

    #[test]
    fn webhook_key_slash_only_treated_as_empty() {
        let key = webhook_key(&wf_id("abc"), "/");
        assert_eq!(key, "abc");
    }

    #[test]
    fn register_stores_route() {
        let handler = WebhookHandler::new(noop_callback());
        handler.register(&wf_id("w1"), "hook");
        assert!(handler.routes.contains_key("w1/hook"));
    }

    #[test]
    fn register_multiple_paths_for_same_workflow() {
        let handler = WebhookHandler::new(noop_callback());
        handler.register(&wf_id("w1"), "a");
        handler.register(&wf_id("w1"), "b");
        assert!(handler.routes.contains_key("w1/a"));
        assert!(handler.routes.contains_key("w1/b"));
    }

    #[test]
    fn remove_clears_all_routes_for_workflow() {
        let handler = WebhookHandler::new(noop_callback());
        handler.register(&wf_id("w1"), "a");
        handler.register(&wf_id("w1"), "b");
        handler.register(&wf_id("w2"), "c");

        handler.remove(&wf_id("w1"));

        assert!(!handler.routes.contains_key("w1/a"));
        assert!(!handler.routes.contains_key("w1/b"));
        assert!(handler.routes.contains_key("w2/c"));
    }

    #[test]
    fn remove_nonexistent_is_noop() {
        let handler = WebhookHandler::new(noop_callback());
        handler.register(&wf_id("w1"), "a");
        handler.remove(&wf_id("w999"));
        assert!(handler.routes.contains_key("w1/a"));
    }

    #[test]
    fn router_returns_axum_router() {
        let handler = WebhookHandler::new(noop_callback());
        let _router = handler.router();
        // Constructing the router should not panic
    }
}
