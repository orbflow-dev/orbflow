// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Port interfaces — the contracts that adapters must implement.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::alerts::AlertRule;
use crate::analytics::{ExecutionStats, FailureTrend, NodePerformance, TimeRange};
use crate::credential::{Credential, CredentialId, CredentialSummary};
use crate::error::OrbflowError;
use crate::event::DomainEvent;
use crate::execution::{Instance, InstanceId, TestNodeResult};
use crate::metering::AccountBudget;
use crate::metrics::{
    InstanceExecutionMetrics, NodeExecutionMetrics, NodeMetricsSummary, WorkflowMetricsSummary,
};
use crate::rbac::{PolicyBinding, PolicyScope, RbacPolicy, Role};
use crate::versioning::{ChangeRequest, ChangeRequestStatus, ReviewComment, WorkflowVersion};
use crate::workflow::{CapabilityPort, NodeKind, Workflow, WorkflowId};

/// Default page size used when no explicit limit is provided.
pub const DEFAULT_PAGE_SIZE: i64 = 20;

/// Configures pagination for list queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListOptions {
    #[serde(default)]
    pub offset: i64,
    #[serde(default)]
    pub limit: i64,
}

impl Default for ListOptions {
    fn default() -> Self {
        Self {
            offset: 0,
            limit: DEFAULT_PAGE_SIZE,
        }
    }
}

/// Input passed to a [`NodeExecutor`] when a task runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInput {
    pub instance_id: InstanceId,
    pub node_id: String,
    pub plugin_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<HashMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<HashMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<HashMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<HashMap<String, serde_json::Value>>,
    #[serde(default)]
    pub attempt: i32,
}

/// Output returned by a [`NodeExecutor`] after execution.
///
/// # Error contract
///
/// There are two distinct failure modes:
///
/// - **Configuration / validation errors**: the executor returns `Err(OrbflowError)`.
///   These indicate the node cannot run (e.g. missing required config, invalid input).
///   The engine marks the node as `failed` and records the `OrbflowError` message.
///
/// - **Runtime / business errors**: the executor returns `Ok(NodeOutput)` with
///   `error: Some(msg)` and optionally partial `data`. These indicate the node
///   executed but the external operation failed (e.g. HTTP 500 from a remote API).
///   The engine marks the node as `failed` but preserves any partial output data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeOutput {
    /// Successful output data from the node execution, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<HashMap<String, serde_json::Value>>,
    /// Runtime error message when the operation failed but the node executed.
    /// `None` indicates success; `Some(msg)` indicates a business-logic failure.
    /// See the type-level docs for the distinction from `Err(OrbflowError)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Supported field types for a [`FieldSchema`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    String,
    Number,
    Boolean,
    Object,
    Array,
    Select,
    Code,
    Json,
    Textarea,
    Password,
    Credential,
}

/// Describes a single input or output field of a node type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldSchema {
    pub key: String,
    pub label: String,
    #[serde(rename = "type")]
    pub field_type: FieldType,
    #[serde(default)]
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub r#enum: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_type: Option<String>,
}

/// Describes a node type's metadata and port definitions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSchema {
    pub plugin_ref: String,
    pub name: String,
    pub description: String,
    pub category: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_kind: Option<NodeKind>,
    pub icon: String,
    pub color: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub docs: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    pub inputs: Vec<FieldSchema>,
    pub outputs: Vec<FieldSchema>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameters: Vec<FieldSchema>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capability_ports: Vec<CapabilityPort>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub settings: Vec<FieldSchema>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provides_capability: Option<String>,
}

/// Optionally implemented by NodeExecutors to expose their schema.
pub trait NodeSchemaProvider: Send + Sync {
    fn node_schema(&self) -> NodeSchema;
}

/// Message handler callback type for bus subscriptions.
pub type MsgHandler = Arc<
    dyn Fn(String, Vec<u8>) -> Pin<Box<dyn Future<Output = Result<(), OrbflowError>> + Send>>
        + Send
        + Sync,
>;

// --- Port Traits ---

/// Orchestrates workflow execution.
///
/// Read methods (get/list) are part of this interface so that every consumer
/// interacts with a single, mockable surface rather than holding separate
/// references to both an Engine and a Store.
#[async_trait]
pub trait Engine: Send + Sync {
    async fn create_workflow(&self, wf: &Workflow) -> Result<(), OrbflowError>;
    async fn update_workflow(&self, wf: &Workflow) -> Result<(), OrbflowError>;
    async fn delete_workflow(&self, id: &WorkflowId) -> Result<(), OrbflowError>;
    async fn get_workflow(&self, id: &WorkflowId) -> Result<Workflow, OrbflowError>;
    async fn list_workflows(&self, opts: ListOptions)
    -> Result<(Vec<Workflow>, i64), OrbflowError>;

    /// Lists version history for a workflow.
    async fn list_workflow_versions(
        &self,
        id: &WorkflowId,
        opts: ListOptions,
    ) -> Result<(Vec<WorkflowVersion>, i64), OrbflowError> {
        let _ = (id, opts);
        Ok((Vec::new(), 0))
    }

    /// Gets a specific version snapshot of a workflow.
    async fn get_workflow_version(
        &self,
        id: &WorkflowId,
        version: i32,
    ) -> Result<WorkflowVersion, OrbflowError> {
        let _ = (id, version);
        Err(OrbflowError::NotFound)
    }

    async fn start_workflow(
        &self,
        id: &WorkflowId,
        input: HashMap<String, serde_json::Value>,
    ) -> Result<Instance, OrbflowError>;
    async fn get_instance(&self, id: &InstanceId) -> Result<Instance, OrbflowError>;
    async fn list_instances(&self, opts: ListOptions)
    -> Result<(Vec<Instance>, i64), OrbflowError>;
    async fn cancel_instance(&self, id: &InstanceId) -> Result<(), OrbflowError>;

    /// Approves a node that is in `WaitingApproval` state, allowing execution to proceed.
    async fn approve_node(
        &self,
        instance_id: &InstanceId,
        node_id: &str,
        approved_by: Option<String>,
    ) -> Result<(), OrbflowError>;

    /// Rejects a node that is in `WaitingApproval` state, marking it as failed.
    async fn reject_node(
        &self,
        instance_id: &InstanceId,
        node_id: &str,
        reason: Option<String>,
        rejected_by: Option<String>,
    ) -> Result<(), OrbflowError>;

    async fn test_node(
        &self,
        workflow_id: &WorkflowId,
        node_id: &str,
        cached_outputs: HashMap<String, HashMap<String, serde_json::Value>>,
        owner_id: Option<&str>,
    ) -> Result<TestNodeResult, OrbflowError>;

    fn register_node(
        &self,
        name: &str,
        executor: Arc<dyn NodeExecutor>,
    ) -> Result<(), OrbflowError>;

    /// Registers a node executor together with its schema.
    /// Default implementation delegates to `register_node` and discards the schema.
    fn register_node_with_schema(
        &self,
        name: &str,
        executor: Arc<dyn NodeExecutor>,
        schema: NodeSchema,
    ) -> Result<(), OrbflowError> {
        let _ = schema;
        self.register_node(name, executor)
    }

    fn node_schemas(&self) -> Vec<NodeSchema>;

    /// Returns only the plugin_ref strings of registered schemas.
    /// Cheaper than `node_schemas()` when only refs are needed for dedup.
    fn node_schema_refs(&self) -> Vec<String> {
        self.node_schemas()
            .into_iter()
            .map(|s| s.plugin_ref)
            .collect()
    }

    /// Register a schema without an executor (for plugin nodes that are
    /// hosted externally via gRPC and may not be running at startup).
    ///
    /// **Implementors that support schema-only registration MUST override
    /// this method.** The default is a no-op intended only for mock/test
    /// engines that do not serve the node picker.
    fn register_schema(&self, _name: &str, _schema: NodeSchema) {
        // No-op default — see doc comment above.
    }

    /// Remove a previously registered schema (e.g. after plugin uninstall).
    /// The default is a no-op for mock/test engines.
    fn unregister_schema(&self, _name: &str) {
        // No-op default.
    }

    /// Verifies the audit hash chain for an instance's event log.
    /// Returns (valid, event_count, optional error message).
    async fn verify_audit_chain(
        &self,
        _instance_id: &InstanceId,
    ) -> Result<(bool, usize, Option<String>), OrbflowError> {
        Err(OrbflowError::InvalidNodeConfig(
            "audit verification not supported by this engine".into(),
        ))
    }

    /// Loads audit records for an instance's event log.
    /// Returns an empty vec by default (engines without audit support).
    async fn load_audit_records(
        &self,
        _instance_id: &InstanceId,
    ) -> Result<Vec<crate::audit::AuditRecord>, OrbflowError> {
        Ok(Vec::new())
    }

    /// Runs a test suite against the engine and returns aggregate results.
    ///
    /// Default implementation iterates over test cases and uses
    /// [`Engine::test_node`] for each. Override for custom test execution.
    async fn run_test_suite(
        &self,
        suite: &crate::testing::TestSuite,
    ) -> Result<crate::testing::TestSuiteResult, OrbflowError> {
        use crate::testing::*;
        let start = std::time::Instant::now();
        let mut results = Vec::with_capacity(suite.cases.len());

        for case in &suite.cases {
            let cached_outputs = build_test_cached_outputs(case.input_overrides.as_ref());
            let case_start = std::time::Instant::now();
            match self
                .test_node(&suite.workflow_id, &case.node_id, cached_outputs, None)
                .await
            {
                Ok(result) => {
                    let output: std::collections::HashMap<String, serde_json::Value> = result
                        .node_outputs
                        .get(&case.node_id)
                        .and_then(|ns| ns.output.clone())
                        .unwrap_or_default();
                    let assertion_results: Vec<AssertionResult> = case
                        .assertions
                        .iter()
                        .map(|a| evaluate_assertion(a, &output))
                        .collect();
                    let all_passed = assertion_results.iter().all(|r| r.passed);
                    results.push(TestCaseResult {
                        name: case.name.clone(),
                        node_id: case.node_id.clone(),
                        passed: all_passed,
                        assertions: assertion_results,
                        error: None,
                        duration_ms: case_start.elapsed().as_millis() as u64,
                    });
                }
                Err(e) => {
                    results.push(TestCaseResult {
                        name: case.name.clone(),
                        node_id: case.node_id.clone(),
                        passed: false,
                        assertions: Vec::new(),
                        error: Some(e.to_string()),
                        duration_ms: case_start.elapsed().as_millis() as u64,
                    });
                }
            }
        }

        let passed = results.iter().filter(|r| r.passed).count();
        let failed = results.len() - passed;
        Ok(TestSuiteResult {
            suite_name: suite.name.clone(),
            workflow_id: suite.workflow_id.clone(),
            total: results.len(),
            passed,
            failed,
            results,
            duration_ms: start.elapsed().as_millis() as u64,
            run_at: chrono::Utc::now(),
        })
    }

    /// Computes test coverage showing which workflow nodes are exercised.
    async fn compute_test_coverage(
        &self,
        workflow_id: &WorkflowId,
        suite: &crate::testing::TestSuite,
    ) -> Result<crate::testing::CoverageReport, OrbflowError> {
        use std::collections::HashSet;
        let workflow = self.get_workflow(workflow_id).await?;
        let all_node_ids: Vec<String> = workflow.nodes.iter().map(|n| n.id.clone()).collect();
        let tested_ids: HashSet<&str> = suite.cases.iter().map(|c| c.node_id.as_str()).collect();
        let untested: Vec<String> = all_node_ids
            .iter()
            .filter(|id| !tested_ids.contains(id.as_str()))
            .cloned()
            .collect();
        let total = all_node_ids.len();
        let tested = total - untested.len();
        let pct = if total > 0 {
            (tested as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        Ok(crate::testing::CoverageReport {
            workflow_id: workflow_id.clone(),
            total_nodes: total,
            tested_nodes: tested,
            coverage_pct: pct,
            untested_nodes: untested,
        })
    }

    async fn start(&self) -> Result<(), OrbflowError>;
    async fn stop(&self) -> Result<(), OrbflowError>;
}

/// Executes a single node's logic.
#[async_trait]
pub trait NodeExecutor: Send + Sync {
    async fn execute(&self, input: &NodeInput) -> Result<NodeOutput, OrbflowError>;
}

/// Composite store: workflows + instances + events.
pub trait Store: WorkflowStore + InstanceStore + EventStore + Send + Sync {}

/// Manages workflow definitions.
#[async_trait]
pub trait WorkflowStore: Send + Sync {
    async fn create_workflow(&self, wf: &Workflow) -> Result<(), OrbflowError>;
    async fn get_workflow(&self, id: &WorkflowId) -> Result<Workflow, OrbflowError>;
    async fn update_workflow(&self, wf: &Workflow) -> Result<(), OrbflowError>;
    async fn delete_workflow(&self, id: &WorkflowId) -> Result<(), OrbflowError>;
    async fn list_workflows(&self, opts: ListOptions)
    -> Result<(Vec<Workflow>, i64), OrbflowError>;

    /// Saves a version snapshot of a workflow definition.
    ///
    /// Called automatically by the engine before each workflow update to preserve
    /// the current definition as a historical version. Every store implementation
    /// must handle this explicitly.
    async fn save_workflow_version(&self, version: &WorkflowVersion) -> Result<(), OrbflowError>;

    /// Lists version history for a workflow, ordered by version descending.
    async fn list_workflow_versions(
        &self,
        id: &WorkflowId,
        opts: ListOptions,
    ) -> Result<(Vec<WorkflowVersion>, i64), OrbflowError> {
        let _ = (id, opts);
        Ok((Vec::new(), 0))
    }

    /// Gets a specific version snapshot of a workflow.
    async fn get_workflow_version(
        &self,
        id: &WorkflowId,
        version: i32,
    ) -> Result<WorkflowVersion, OrbflowError> {
        let _ = (id, version);
        Err(OrbflowError::NotFound)
    }
}

/// Manages workflow execution instances.
#[async_trait]
pub trait InstanceStore: Send + Sync {
    async fn create_instance(&self, inst: &Instance) -> Result<(), OrbflowError>;
    async fn get_instance(&self, id: &InstanceId) -> Result<Instance, OrbflowError>;
    async fn update_instance(&self, inst: &Instance) -> Result<(), OrbflowError>;
    async fn list_instances(&self, opts: ListOptions)
    -> Result<(Vec<Instance>, i64), OrbflowError>;
    async fn list_running_instances(&self) -> Result<Vec<Instance>, OrbflowError>;
}

/// Optionally implemented by stores that support creating an instance and
/// appending its first event in a single transaction.
#[async_trait]
pub trait AtomicInstanceCreator: Send + Sync {
    async fn create_instance_tx(
        &self,
        inst: &Instance,
        event: DomainEvent,
    ) -> Result<(), OrbflowError>;
}

/// Manages the append-only event log.
#[async_trait]
pub trait EventStore: Send + Sync {
    async fn append_event(&self, event: DomainEvent) -> Result<(), OrbflowError>;
    async fn load_events(
        &self,
        instance_id: &InstanceId,
        after_version: i64,
    ) -> Result<Vec<DomainEvent>, OrbflowError>;
    async fn save_snapshot(&self, inst: &Instance) -> Result<(), OrbflowError>;
    async fn load_snapshot(
        &self,
        instance_id: &InstanceId,
    ) -> Result<Option<Instance>, OrbflowError>;

    /// Loads audit records for an instance's event log.
    /// Returns an empty vec by default (stores without hash columns).
    async fn load_audit_records(
        &self,
        _instance_id: &InstanceId,
    ) -> Result<Vec<crate::audit::AuditRecord>, OrbflowError> {
        Ok(Vec::new())
    }
}

/// Dispatches tasks to workers and receives results.
#[async_trait]
pub trait Bus: Send + Sync {
    async fn publish(&self, subject: &str, data: &[u8]) -> Result<(), OrbflowError>;
    async fn subscribe(&self, subject: &str, handler: MsgHandler) -> Result<(), OrbflowError>;
    async fn close(&self) -> Result<(), OrbflowError>;
}

/// Manages encrypted credentials.
#[async_trait]
pub trait CredentialStore: Send + Sync {
    async fn create_credential(&self, cred: &Credential) -> Result<(), OrbflowError>;
    async fn get_credential(&self, id: &CredentialId) -> Result<Credential, OrbflowError>;

    /// Gets a credential by ID, scoped to the given owner.
    ///
    /// When `owner_id` is `Some`, returns `NotFound` if the credential doesn't
    /// exist or doesn't belong to this owner. Pass `None` only from privileged
    /// admin paths that bypass ownership checks. Implementations MUST NOT fall
    /// back to an unscoped query on error — all errors must be propagated.
    ///
    /// Default implementation delegates to [`get_credential`](Self::get_credential)
    /// (no owner filtering) for stores that don't yet support owner scoping.
    async fn get_credential_for_owner(
        &self,
        id: &CredentialId,
        _owner_id: Option<&str>,
    ) -> Result<Credential, OrbflowError> {
        self.get_credential(id).await
    }

    async fn update_credential(&self, cred: &Credential) -> Result<(), OrbflowError>;

    /// Deletes a credential by ID, optionally scoped to an owner.
    ///
    /// When `owner_id` is `Some`, the DELETE is scoped to rows where
    /// `owner_id = $2`, so a credential belonging to a different owner
    /// returns `NotFound` rather than being deleted. Pass `None` only
    /// from privileged admin paths that bypass ownership checks.
    async fn delete_credential(
        &self,
        id: &CredentialId,
        owner_id: Option<&str>,
    ) -> Result<(), OrbflowError>;
    async fn list_credentials(&self) -> Result<Vec<CredentialSummary>, OrbflowError>;

    /// Lists credentials scoped to the given owner.
    ///
    /// When `owner_id` is `Some`, returns only credentials belonging to that
    /// owner. When `None`, returns unowned (global) credentials. Default
    /// delegates to unscoped [`list_credentials`](Self::list_credentials)
    /// for backward compatibility with stores that don't yet support owner scoping.
    async fn list_credentials_for_owner(
        &self,
        _owner_id: Option<&str>,
    ) -> Result<Vec<CredentialSummary>, OrbflowError> {
        self.list_credentials().await
    }
}

/// Persistence port for execution metrics.
#[async_trait]
pub trait MetricsStore: Send + Sync {
    /// Records metrics for a completed node execution.
    async fn record_node_metrics(&self, metrics: &NodeExecutionMetrics)
    -> Result<(), OrbflowError>;

    /// Records metrics for a completed workflow instance.
    async fn record_instance_metrics(
        &self,
        metrics: &InstanceExecutionMetrics,
    ) -> Result<(), OrbflowError>;

    /// Gets aggregated metrics for a workflow (success rate, avg duration, etc.).
    async fn get_workflow_metrics(
        &self,
        workflow_id: &WorkflowId,
        since: chrono::DateTime<chrono::Utc>,
    ) -> Result<WorkflowMetricsSummary, OrbflowError>;

    /// Gets per-node metrics breakdown for a workflow.
    async fn get_node_metrics(
        &self,
        workflow_id: &WorkflowId,
        since: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<NodeMetricsSummary>, OrbflowError>;

    /// Gets metrics for a specific instance.
    async fn get_instance_metrics(
        &self,
        instance_id: &InstanceId,
    ) -> Result<Option<InstanceExecutionMetrics>, OrbflowError>;
}

/// Manages change requests for collaborative workflow editing.
#[async_trait]
pub trait ChangeRequestStore: Send + Sync {
    async fn create_change_request(&self, cr: &ChangeRequest) -> Result<(), OrbflowError>;
    async fn get_change_request(&self, id: &str) -> Result<ChangeRequest, OrbflowError>;
    async fn list_change_requests(
        &self,
        workflow_id: &WorkflowId,
        status: Option<ChangeRequestStatus>,
        opts: ListOptions,
    ) -> Result<(Vec<ChangeRequest>, i64), OrbflowError>;
    async fn update_change_request(&self, cr: &ChangeRequest) -> Result<(), OrbflowError>;
    async fn add_comment(&self, cr_id: &str, comment: &ReviewComment) -> Result<(), OrbflowError>;
    async fn resolve_comment(
        &self,
        cr_id: &str,
        comment_id: &str,
        resolved: bool,
    ) -> Result<(), OrbflowError>;

    /// Atomically merges an approved change request into its workflow.
    ///
    /// Within a single transaction: locks the CR row, verifies it is still
    /// `Approved`, checks the workflow version matches `expected_version`,
    /// updates the workflow definition, bumps the version, and marks the CR
    /// as `Merged`. Returns `Conflict` if the CR status changed or the
    /// workflow version is stale.
    async fn merge_change_request(
        &self,
        cr_id: &str,
        expected_version: i32,
        new_definition: &serde_json::Value,
    ) -> Result<(), OrbflowError>;
}

/// Manages RBAC role definitions and policy bindings.
#[async_trait]
pub trait RbacStore: Send + Sync {
    /// Loads the full RBAC policy (all roles + all bindings).
    async fn load_policy(&self) -> Result<RbacPolicy, OrbflowError>;

    /// Replaces the entire RBAC policy (full overwrite within a transaction).
    async fn save_policy(&self, policy: &RbacPolicy) -> Result<(), OrbflowError>;

    /// Creates a new role definition.
    async fn create_role(&self, role: &Role) -> Result<(), OrbflowError>;

    /// Deletes a role by ID (cascades to bindings).
    async fn delete_role(&self, role_id: &str) -> Result<(), OrbflowError>;

    /// Updates an existing role's name, permissions, and description.
    /// Returns `NotFound` if the role does not exist or is a builtin role.
    async fn update_role(&self, role: &Role) -> Result<(), OrbflowError>;

    /// Lists all role definitions.
    async fn list_roles(&self) -> Result<Vec<Role>, OrbflowError>;

    /// Adds a policy binding (subject + role + scope).
    async fn add_binding(&self, binding: &PolicyBinding) -> Result<(), OrbflowError>;

    /// Removes a specific policy binding.
    async fn remove_binding(
        &self,
        subject: &str,
        role_id: &str,
        scope: &PolicyScope,
    ) -> Result<(), OrbflowError>;

    /// Lists policy bindings, optionally filtered by subject.
    async fn list_bindings(
        &self,
        subject: Option<&str>,
    ) -> Result<Vec<PolicyBinding>, OrbflowError>;

    /// Lists all distinct subjects from policy bindings.
    async fn list_subjects(&self) -> Result<Vec<String>, OrbflowError>;
}

// ─── Plugin Management Ports ────────────────────────────────────────────────

/// Summary information about a managed plugin process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub name: String,
    pub version: Option<String>,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
}

/// An entry in the plugin registry index.
///
/// Carries enough detail to render both summary lists and detail pages in the
/// frontend marketplace without reaching back into adapter-specific types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginIndexEntry {
    pub name: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub node_types: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orbflow_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub readme: Option<String>,
    /// Path within the plugin repository (monorepo layout).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Plugin protocol (e.g., "grpc", "subprocess").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol: Option<String>,
    /// SHA-256 checksum of the plugin tarball (optional, verified on install).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checksum: Option<String>,
}

/// Port trait for managing external plugin processes.
///
/// Adapter: `PluginProcessManager` in `orbflow-plugin`.
// TODO: implement PluginManager for PluginProcessManager in orbflow-plugin
#[async_trait]
pub trait PluginManager: Send + Sync {
    /// Lists status of all managed plugins.
    async fn list_plugins(&self) -> Result<Vec<PluginInfo>, OrbflowError>;
    /// Gets status of a single plugin by name.
    async fn get_plugin(&self, name: &str) -> Result<PluginInfo, OrbflowError>;
    /// Starts a plugin process.
    async fn start_plugin(&self, name: &str) -> Result<PluginInfo, OrbflowError>;
    /// Stops a plugin process.
    async fn stop_plugin(&self, name: &str) -> Result<(), OrbflowError>;
    /// Stops all plugins, re-scans manifests, and spawns everything again.
    async fn reload_all(&self) -> Result<Vec<PluginInfo>, OrbflowError>;
}

/// Port trait for the node/plugin registry index.
///
/// Adapters: `LocalIndex` and `CommunityIndex` in `orbflow-registry`.
#[async_trait]
pub trait PluginIndex: Send + Sync {
    /// Lists all installed/available plugins in the index.
    async fn list_available(&self) -> Result<Vec<PluginIndexEntry>, OrbflowError>;
    /// Gets details for a specific plugin by name.
    async fn get_entry(&self, name: &str) -> Result<Option<PluginIndexEntry>, OrbflowError>;
}

/// Installs a plugin by name into a local directory.
///
/// Separated from [`PluginIndex`] because not all index implementations
/// support installation (e.g., a read-only local index). Implementors that
/// combine multiple sources (local + community) typically implement this.
#[async_trait]
pub trait PluginInstaller: Send + Sync {
    /// Downloads and installs a plugin by name to the given destination.
    /// Returns the number of files extracted.
    async fn install_plugin(
        &self,
        name: &str,
        dest: &std::path::Path,
    ) -> Result<usize, OrbflowError>;
}

/// Manages persistent budget configurations for cost enforcement.
#[async_trait]
pub trait BudgetStore: Send + Sync {
    /// Creates a new budget entry.
    async fn create_budget(&self, budget: &AccountBudget) -> Result<(), OrbflowError>;

    /// Gets a budget by its ID.
    async fn get_budget(&self, id: &str) -> Result<AccountBudget, OrbflowError>;

    /// Lists all budget entries.
    async fn list_budgets(&self) -> Result<Vec<AccountBudget>, OrbflowError>;

    /// Updates an existing budget entry.
    async fn update_budget(&self, budget: &AccountBudget) -> Result<(), OrbflowError>;

    /// Deletes a budget by its ID.
    async fn delete_budget(&self, id: &str) -> Result<(), OrbflowError>;

    /// Checks if a budget exists and applies to the given workflow.
    /// Returns the matching budget if found.
    async fn check_budget(&self, workflow_id: &str) -> Result<Option<AccountBudget>, OrbflowError>;

    /// Increments the accumulated cost for budgets matching the given workflow.
    async fn increment_cost(&self, workflow_id: &str, cost_usd: f64) -> Result<(), OrbflowError>;
}

/// Aggregated analytics queries across all workflows and instances.
#[async_trait]
pub trait AnalyticsStore: Send + Sync {
    /// Returns overall execution statistics for a time range.
    async fn execution_stats(&self, range: &TimeRange) -> Result<ExecutionStats, OrbflowError>;

    /// Returns per-node performance metrics for a time range.
    async fn node_performance(
        &self,
        range: &TimeRange,
    ) -> Result<Vec<NodePerformance>, OrbflowError>;

    /// Returns daily failure trends grouped by workflow for a time range.
    async fn failure_trends(&self, range: &TimeRange) -> Result<Vec<FailureTrend>, OrbflowError>;
}

/// CRUD operations for alert rules.
#[async_trait]
pub trait AlertStore: Send + Sync {
    /// Creates a new alert rule.
    async fn create_alert(&self, rule: &AlertRule) -> Result<(), OrbflowError>;

    /// Gets an alert rule by its ID.
    async fn get_alert(&self, id: &str) -> Result<AlertRule, OrbflowError>;

    /// Lists all alert rules.
    async fn list_alerts(&self) -> Result<Vec<AlertRule>, OrbflowError>;

    /// Updates an existing alert rule.
    async fn update_alert(&self, rule: &AlertRule) -> Result<(), OrbflowError>;

    /// Deletes an alert rule by its ID.
    async fn delete_alert(&self, id: &str) -> Result<(), OrbflowError>;
}
