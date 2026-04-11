// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Integration tests for HTTP handler routes using tower's `ServiceExt::oneshot`.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use orbflow_core::{
    Engine, Instance, InstanceId, ListOptions, NodeExecutor, NodeSchema, OrbflowError,
    TestNodeResult, Workflow, WorkflowId,
};
use orbflow_httpapi::{HttpApiOptions, create_router};
use parking_lot::RwLock;
use tower::ServiceExt;

// ---------------------------------------------------------------------------
// Minimal MockEngine — only the Engine methods exercised by the tested routes.
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

async fn response_json(body: Body) -> serde_json::Value {
    let bytes = body.collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

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
