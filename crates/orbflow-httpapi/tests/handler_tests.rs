// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Integration tests for HTTP handler routes using tower's `ServiceExt::oneshot`.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use chrono::Utc;
use http_body_util::BodyExt;
use orbflow_core::credential::{Credential, CredentialId};
use orbflow_core::{
    ChangeRequestStore, CredentialStore, Engine, Instance, InstanceId, ListOptions, NodeExecutor,
    NodeSchema, OrbflowError, TestNodeResult, Workflow, WorkflowId,
};
use orbflow_httpapi::{HttpApiOptions, create_router};
use orbflow_memstore::MemStore;
use parking_lot::RwLock;
use tower::ServiceExt;

// Minimal MockEngine — only the Engine methods exercised by the tested routes.

struct MockEngine {
    workflows: RwLock<Vec<Workflow>>,
}

impl MockEngine {
    fn new() -> Self {
        Self {
            workflows: RwLock::new(Vec::new()),
        }
    }
}

#[async_trait]
impl Engine for MockEngine {
    async fn create_workflow(&self, wf: &Workflow) -> Result<(), OrbflowError> {
        self.workflows.write().push(wf.clone());
        Ok(())
    }

    async fn update_workflow(&self, _wf: &Workflow) -> Result<(), OrbflowError> {
        Ok(())
    }

    async fn delete_workflow(&self, id: &WorkflowId) -> Result<(), OrbflowError> {
        let mut wfs = self.workflows.write();
        let idx = wfs
            .iter()
            .position(|w| &w.id == id)
            .ok_or(OrbflowError::NotFound)?;
        wfs.remove(idx);
        Ok(())
    }

    async fn get_workflow(&self, id: &WorkflowId) -> Result<Workflow, OrbflowError> {
        self.workflows
            .read()
            .iter()
            .find(|w| &w.id == id)
            .cloned()
            .ok_or(OrbflowError::NotFound)
    }

    async fn list_workflows(
        &self,
        _opts: ListOptions,
    ) -> Result<(Vec<Workflow>, i64), OrbflowError> {
        let list = self.workflows.read().clone();
        let total = list.len() as i64;
        Ok((list, total))
    }

    async fn start_workflow(
        &self,
        _id: &WorkflowId,
        _input: HashMap<String, serde_json::Value>,
    ) -> Result<Instance, OrbflowError> {
        Err(OrbflowError::NotFound)
    }

    async fn get_instance(&self, _id: &InstanceId) -> Result<Instance, OrbflowError> {
        Err(OrbflowError::NotFound)
    }

    async fn list_instances(
        &self,
        _opts: ListOptions,
    ) -> Result<(Vec<Instance>, i64), OrbflowError> {
        Ok((vec![], 0))
    }

    async fn cancel_instance(&self, _id: &InstanceId) -> Result<(), OrbflowError> {
        Err(OrbflowError::NotFound)
    }

    async fn test_node(
        &self,
        _workflow_id: &WorkflowId,
        _node_id: &str,
        _cached_outputs: HashMap<String, HashMap<String, serde_json::Value>>,
        _owner_id: Option<&str>,
    ) -> Result<TestNodeResult, OrbflowError> {
        Err(OrbflowError::NotFound)
    }

    fn register_node(
        &self,
        _name: &str,
        _executor: Arc<dyn NodeExecutor>,
    ) -> Result<(), OrbflowError> {
        Ok(())
    }

    fn node_schemas(&self) -> Vec<NodeSchema> {
        vec![]
    }

    async fn start(&self) -> Result<(), OrbflowError> {
        Ok(())
    }

    async fn approve_node(
        &self,
        _instance_id: &InstanceId,
        _node_id: &str,
        _approved_by: Option<String>,
    ) -> Result<(), OrbflowError> {
        Ok(())
    }

    async fn reject_node(
        &self,
        _instance_id: &InstanceId,
        _node_id: &str,
        _reason: Option<String>,
        _rejected_by: Option<String>,
    ) -> Result<(), OrbflowError> {
        Ok(())
    }

    async fn stop(&self) -> Result<(), OrbflowError> {
        Ok(())
    }
}

// Helpers

fn test_router() -> axum::Router {
    let engine: Arc<dyn Engine> = Arc::new(MockEngine::new());
    create_router(HttpApiOptions {
        engine,
        credential_store: None,
        bus: None,
        metrics_store: None,
        auth_token: None,
        rbac: None,
        rbac_store: None,
        plugin_index: None,
        plugin_installer: None,
        change_request_store: None,
        budget_store: None,
        analytics_store: None,
        alert_store: None,
        trust_x_user_id: false,
        bootstrap_admin: None,
        plugin_manager: None,
        plugins_dir: None,
        cors_origins: vec![],
        rate_limit: orbflow_config::RateLimitConfig::default(),
    })
    .expect("failed to create test router")
}

fn authed_router_with_memstore() -> (axum::Router, Arc<MemStore>) {
    let engine: Arc<dyn Engine> = Arc::new(MockEngine::new());
    let store = Arc::new(MemStore::new());
    let router = create_router(HttpApiOptions {
        engine,
        credential_store: Some(Arc::clone(&store) as Arc<dyn CredentialStore>),
        bus: None,
        metrics_store: None,
        auth_token: Some("test-token".to_string()),
        rbac: None,
        rbac_store: None,
        plugin_index: None,
        plugin_installer: None,
        change_request_store: Some(Arc::clone(&store) as Arc<dyn ChangeRequestStore>),
        budget_store: None,
        analytics_store: None,
        alert_store: None,
        trust_x_user_id: true,
        bootstrap_admin: None,
        plugin_manager: None,
        plugins_dir: None,
        cors_origins: vec![],
        rate_limit: orbflow_config::RateLimitConfig::default(),
    })
    .expect("failed to create authenticated test router");

    (router, store)
}

fn authed_json_request(
    method: Method,
    uri: &str,
    user_id: &str,
    payload: serde_json::Value,
) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("Authorization", "Bearer test-token")
        .header("X-User-Id", user_id)
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&payload).unwrap()))
        .unwrap()
}

async fn response_json(body: Body) -> serde_json::Value {
    let bytes = body.collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

fn backend_workflow_definition() -> serde_json::Value {
    serde_json::json!({
        "id": "wf-cr",
        "name": "Change request workflow",
        "description": "definition used by handler security regressions",
        "version": 1,
        "nodes": [{
            "id": "start",
            "name": "Start",
            "kind": "trigger",
            "type": "builtin",
            "plugin_ref": "builtin:trigger-manual",
            "position": { "x": 0.0, "y": 0.0 },
            "parameters": [],
            "capability_ports": [],
            "requires_approval": false
        }],
        "edges": []
    })
}

async fn create_change_request_as(
    router: &axum::Router,
    user_id: &str,
    body_author: &str,
) -> String {
    let resp = router
        .clone()
        .oneshot(authed_json_request(
            Method::POST,
            "/api/v1/workflows/wf-cr/change-requests",
            user_id,
            serde_json::json!({
                "title": "Security CR",
                "description": "actor stamping regression",
                "proposed_definition": backend_workflow_definition(),
                "base_version": 1,
                "author": body_author,
                "reviewers": ["bob"]
            }),
        ))
        .await
        .unwrap();

    let status = resp.status();
    let json = response_json(resp.into_body()).await;
    assert_eq!(status, StatusCode::CREATED, "response body: {json}");
    json["data"]["id"]
        .as_str()
        .expect("created change request id")
        .to_string()
}

// Tests

#[tokio::test]
async fn health_check_returns_200() {
    let resp = test_router()
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    let json = response_json(resp.into_body()).await;
    assert_eq!(json["status"], "ok");
}

#[tokio::test]
async fn list_workflows_returns_envelope_with_meta() {
    let resp = test_router()
        .oneshot(
            Request::builder()
                .uri("/api/v1/workflows")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    let json = response_json(resp.into_body()).await;
    assert!(json["data"].is_array(), "envelope must have a data array");
    assert!(json["meta"].is_object(), "envelope must have a meta object");
    assert!(json["meta"]["total"].is_number());
    assert!(json["meta"]["offset"].is_number());
    assert!(json["meta"]["limit"].is_number());
}

#[tokio::test]
async fn create_workflow_with_invalid_json_returns_client_error() {
    let resp = test_router()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workflows")
                .header("Content-Type", "application/json")
                .body(Body::from("not valid json {{"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert!(
        resp.status().is_client_error(),
        "expected 4xx, got {}",
        resp.status()
    );
}

#[tokio::test]
async fn create_workflow_with_valid_body_returns_201() {
    let payload = serde_json::json!({ "name": "hello-world" });

    let resp = test_router()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workflows")
                .header("Content-Type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::CREATED);

    let json = response_json(resp.into_body()).await;
    assert_eq!(json["data"]["name"], "hello-world");
}

#[tokio::test]
async fn update_credential_metadata_only_preserves_existing_secret_data() {
    let (router, store) = authed_router_with_memstore();
    let cred_id = CredentialId::new("cred-preserve").unwrap();
    let now = Utc::now();
    store
        .create_credential(&Credential {
            id: cred_id.clone(),
            name: "OpenAI".into(),
            credential_type: "openai".into(),
            data: HashMap::from([
                ("api_key".into(), serde_json::json!("sk-original")),
                (
                    "base_url".into(),
                    serde_json::json!("https://api.openai.com/v1"),
                ),
            ]),
            description: Some("original description".into()),
            owner_id: Some("alice".into()),
            access_tier: Default::default(),
            policy: None,
            created_at: now,
            updated_at: now,
        })
        .await
        .unwrap();

    let resp = router
        .oneshot(authed_json_request(
            Method::PUT,
            "/api/v1/credentials/cred-preserve",
            "alice",
            serde_json::json!({
                "name": "OpenAI renamed",
                "type": "openai",
                "description": "metadata-only update"
            }),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let stored = store.get_credential(&cred_id).await.unwrap();
    assert_eq!(stored.name, "OpenAI renamed");
    assert_eq!(stored.description.as_deref(), Some("metadata-only update"));
    assert_eq!(
        stored.data.get("api_key"),
        Some(&serde_json::json!("sk-original")),
        "metadata-only update must not wipe or redact the stored secret"
    );
    assert_eq!(
        stored.data.get("base_url"),
        Some(&serde_json::json!("https://api.openai.com/v1"))
    );
}

#[tokio::test]
async fn change_request_create_and_comment_stamp_authenticated_actor() {
    let (router, _store) = authed_router_with_memstore();

    let resp = router
        .clone()
        .oneshot(authed_json_request(
            Method::POST,
            "/api/v1/workflows/wf-cr/change-requests",
            "alice",
            serde_json::json!({
                "title": "Spoofed author CR",
                "description": "caller body attempts to spoof author",
                "proposed_definition": backend_workflow_definition(),
                "base_version": 1,
                "author": "mallory",
                "reviewers": ["bob"]
            }),
        ))
        .await
        .unwrap();

    let status = resp.status();
    let json = response_json(resp.into_body()).await;
    assert_eq!(status, StatusCode::CREATED, "response body: {json}");
    assert_eq!(
        json["data"]["author"], "alice",
        "CR author must be stamped from AuthUser, not request body"
    );
    let cr_id = json["data"]["id"].as_str().unwrap();

    let resp = router
        .oneshot(authed_json_request(
            Method::POST,
            &format!("/api/v1/workflows/wf-cr/change-requests/{cr_id}/comments"),
            "bob",
            serde_json::json!({
                "author": "mallory",
                "body": "review comment"
            }),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::CREATED);
    let json = response_json(resp.into_body()).await;
    assert_eq!(
        json["data"]["author"], "bob",
        "CR comment author must be stamped from AuthUser, not request body"
    );
}

#[tokio::test]
async fn change_request_self_approve_and_reject_ignore_spoofed_author_field() {
    let (router, _store) = authed_router_with_memstore();

    let approve_cr_id = create_change_request_as(&router, "alice", "mallory").await;
    let resp = router
        .clone()
        .oneshot(authed_json_request(
            Method::POST,
            &format!("/api/v1/workflows/wf-cr/change-requests/{approve_cr_id}/submit"),
            "alice",
            serde_json::json!({}),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = router
        .clone()
        .oneshot(authed_json_request(
            Method::POST,
            &format!("/api/v1/workflows/wf-cr/change-requests/{approve_cr_id}/approve"),
            "alice",
            serde_json::json!({}),
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "spoofed request-body author must not let the creator self-approve"
    );

    let reject_cr_id = create_change_request_as(&router, "alice", "mallory").await;
    let resp = router
        .clone()
        .oneshot(authed_json_request(
            Method::POST,
            &format!("/api/v1/workflows/wf-cr/change-requests/{reject_cr_id}/submit"),
            "alice",
            serde_json::json!({}),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = router
        .oneshot(authed_json_request(
            Method::POST,
            &format!("/api/v1/workflows/wf-cr/change-requests/{reject_cr_id}/reject"),
            "alice",
            serde_json::json!({ "reason": "needs changes" }),
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "spoofed request-body author must not let the creator self-reject"
    );
}
