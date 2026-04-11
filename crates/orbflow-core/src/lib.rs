// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Core domain types, traits, and utilities for the Orbflow workflow engine.
//!
//! This crate defines all domain types and port interfaces. Every other crate
//! implements one adapter or orchestration concern. Dependencies point inward —
//! only `orbflow-core` is imported across crate boundaries.

pub mod alerts;
pub mod analytics;
pub mod audit;
pub mod compliance;
pub mod credential;
pub mod credential_proxy;
pub mod crypto;
pub mod edge;
pub mod error;
pub mod event;
pub mod execution;
pub mod metering;
pub mod metrics;
pub mod options;
pub mod otel;
pub mod pagination;
pub mod ports;
pub mod prediction;
pub mod rbac;
pub mod schema;
pub mod ssrf;
pub mod streaming;
pub mod subjects;
pub mod telemetry;
pub mod testing;
pub mod trigger;
pub mod validate;
pub mod versioning;
pub mod wire;
pub mod workflow;

// Re-export commonly used types at the crate root.
pub use alerts::{AlertChannel, AlertMetric, AlertOperator, AlertRule};
pub use analytics::{DailyCount, ExecutionStats, FailureTrend, NodePerformance, TimeRange};
pub use credential::{
    CreateCredentialRequest, Credential, CredentialId, CredentialSummary, CredentialTypeSchema,
};
pub use credential_proxy::{
    CapabilityRequest, CapabilityRequestType, CapabilityResponse, CredentialAccessTier,
    CredentialPolicy,
};
pub use error::OrbflowError;
pub use event::DomainEvent;
pub use execution::{
    ExecutionContext, Instance, InstanceId, InstanceStatus, NodeState, NodeStatus, SagaState,
    TestNodeResult, TriggerInfo,
};
pub use metering::{AccountBudget, BudgetPeriod};
pub use metrics::{
    InstanceExecutionMetrics, NodeExecutionMetrics, NodeMetricsSummary, WorkflowMetricsSummary,
};
pub use options::EngineOptions;
pub use otel::MetricsRecorder;
pub use pagination::paginate;
pub use ports::{
    AlertStore, AnalyticsStore, AtomicInstanceCreator, BudgetStore, Bus, ChangeRequestStore,
    CredentialStore, DEFAULT_PAGE_SIZE, Engine, EventStore, FieldSchema, FieldType, InstanceStore,
    ListOptions, MetricsStore, MsgHandler, NodeExecutor, NodeInput, NodeOutput, NodeSchema,
    NodeSchemaProvider, PluginIndex, PluginIndexEntry, PluginInfo, PluginInstaller, PluginManager,
    RbacStore, Store, WorkflowStore,
};
pub use rbac::{Permission, PolicyBinding, PolicyScope, RbacPolicy, Role};
pub use schema::{CREDENTIAL_SCHEMAS, CredentialSchemas};
pub use streaming::{StreamChunk, StreamMessage, StreamSender, StreamingNodeExecutor};
pub use subjects::{
    SUBJECT_PREFIX, plugin_reload_subject, result_subject, stream_subject, task_subject,
};
pub use testing::{
    CoverageReport, MatcherType, TestAssertion, TestCase, TestCaseResult, TestSuite,
    TestSuiteResult, build_test_cached_outputs, evaluate_assertion,
};
pub use trigger::{Trigger, TriggerConfig, TriggerType};
pub use validate::{validate_node_configs, validate_plugin_name, validate_workflow};
pub use versioning::{ChangeRequest, ChangeRequestStatus, ReviewComment, WorkflowVersion};
pub use wire::{ResultMessage, TaskMessage, WIRE_VERSION};
pub use workflow::{
    Annotation, CapabilityEdge, CapabilityPort, CompensateConfig, DefinitionStatus, Edge, Node,
    NodeKind, NodeMetadata, NodeType, ParameterMode, Position, RetryPolicy, Workflow, WorkflowId,
};
