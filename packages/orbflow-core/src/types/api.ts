// Wire types: fields use snake_case matching the Rust serde JSON output.
// UI types in schema.ts use camelCase for React/TS ergonomics.
// The two layers are intentionally different — api.ts mirrors the REST payload shape.

import type { NodeKind, ParameterMode, CapabilityEdge, Annotation } from "./schema";
import type { ExecutionStatus } from "../execution/execution-status";

/**
 * Wraps a paginated list response, preserving the total count and cursor
 * information returned by the server alongside the item array.
 */
export interface PaginatedResult<T> {
  items: T[];
  total: number;
  offset: number;
  limit: number;
}

export type DefinitionStatus = "draft" | "active" | "archived";

export interface Workflow {
  id: string;
  name: string;
  description?: string;
  version: number;
  status: DefinitionStatus;
  nodes: WorkflowNode[];
  edges: WorkflowEdge[];
  capability_edges?: CapabilityEdgeData[];
  annotations?: AnnotationData[];
  created_at: string;
  updated_at: string;
}

export interface WorkflowNode {
  id: string;
  name: string;
  kind?: NodeKind;
  type: string;
  plugin_ref: string;
  input_mapping?: Record<string, unknown>;
  config?: Record<string, unknown>;
  parameters?: { key: string; mode: ParameterMode; value?: unknown; expression?: string }[];
  retry?: {
    max_attempts: number;
    delay: string;
    multiplier: number;
  };
  compensate?: {
    plugin_ref: string;
    input_mapping?: Record<string, unknown>;
  };
  capability_ports?: { key: string; capability_type: string; required?: boolean }[];
  metadata?: { description?: string; docs?: string; image_url?: string };
  trigger_config?: { trigger_type: string; cron?: string; event_name?: string; path?: string };
  requires_approval?: boolean;
  position: { x: number; y: number };
  parent_id?: string;
}

export interface CapabilityEdgeData {
  id: string;
  source_node_id: string;
  target_node_id: string;
  target_port_key: string;
}

export interface AnnotationData {
  id: string;
  type: string;
  content: string;
  position: { x: number; y: number };
  style?: Record<string, unknown>;
}

/** Convert wire-format CapabilityEdgeData to UI-format CapabilityEdge. */
export function toCapabilityEdge(wire: CapabilityEdgeData): CapabilityEdge {
  return {
    id: wire.id,
    sourceNodeId: wire.source_node_id,
    targetNodeId: wire.target_node_id,
    targetPortKey: wire.target_port_key,
  };
}

/** Convert wire-format AnnotationData to UI-format Annotation. */
export function toAnnotation(wire: AnnotationData): Annotation {
  return {
    id: wire.id,
    type: wire.type as Annotation["type"],
    content: wire.content,
    position: wire.position,
    style: wire.style,
  };
}

export interface WorkflowEdge {
  id: string;
  source: string;
  target: string;
  condition?: string;
}

export interface Instance {
  id: string;
  workflow_id: string;
  status: ExecutionStatus;
  node_states: Record<string, NodeState>;
  /** Version of the workflow definition at execution time. Use get_workflow_version to fetch it. */
  workflow_version?: number;
  created_at: string;
  updated_at: string;
}

export interface NodeState {
  node_id: string;
  status: ExecutionStatus;
  input?: Record<string, unknown>;
  output?: Record<string, unknown>;
  parameters?: Record<string, unknown>;
  error?: string;
  attempt: number;
  started_at?: string;
  ended_at?: string;
}

export interface TestNodeResult {
  node_outputs: Record<string, NodeState>;
  target_node: string;
  warnings?: string[];
}

export interface NodeTypeSchema {
  plugin_ref: string;
  name: string;
  description: string;
  category: string;
  icon: string;
  color: string;
  inputs: {
    key: string;
    label: string;
    type: string;
    required?: boolean;
    default?: unknown;
    description?: string;
    enum?: string[];
  }[];
  outputs: {
    key: string;
    label: string;
    type: string;
    description?: string;
  }[];
}

export type CredentialAccessTier = "proxy" | "scoped_token" | "raw";

export interface CredentialPolicy {
  allowed_tiers: CredentialAccessTier[];
  allowed_domains: string[];
  rate_limit_per_minute: number;
}

export interface CredentialSummary {
  id: string;
  name: string;
  type: string;
  description: string;
  access_tier?: CredentialAccessTier;
  policy?: CredentialPolicy;
  created_at: string;
  updated_at: string;
}

export interface Credential extends CredentialSummary {
  data: Record<string, unknown>;
}

export interface CredentialCreate {
  name: string;
  type: string;
  description?: string;
  data: Record<string, unknown>;
  access_tier?: CredentialAccessTier;
  policy?: CredentialPolicy;
}

export interface CredentialTypeSchema {
  type: string;
  name: string;
  description: string;
  icon: string;
  color: string;
  fields: {
    key: string;
    label: string;
    type: string;
    required?: boolean;
    default?: unknown;
    description?: string;
    enum?: string[];
  }[];
}

/** A point-in-time snapshot of a workflow definition. */
export interface WorkflowVersion {
  id: number;
  workflow_id: string;
  version: number;
  definition: Record<string, unknown>;
  author: string | null;
  message: string | null;
  created_at: string;
}

/** Structured diff between two workflow versions. */
export interface WorkflowDiff {
  from_version: number;
  to_version: number;
  added_nodes: string[];
  removed_nodes: string[];
  modified_nodes: string[];
  added_edges: string[];
  removed_edges: string[];
}

/** Aggregated metrics summary for a workflow definition. */
export interface WorkflowMetricsSummary {
  workflow_id: string;
  total_executions: number;
  successful_executions: number;
  failed_executions: number;
  success_rate: number;
  avg_duration_ms: number;
  p50_duration_ms: number;
  p95_duration_ms: number;
  p99_duration_ms: number;
  since: string;
}

/** Per-node aggregated metrics within a workflow. */
export interface NodeMetricsSummary {
  node_id: string;
  plugin_ref: string;
  total_executions: number;
  successful_executions: number;
  failed_executions: number;
  success_rate: number;
  avg_duration_ms: number;
  p50_duration_ms: number;
  p95_duration_ms: number;
}

/** Metrics for a specific workflow instance execution. */
export interface InstanceExecutionMetrics {
  instance_id: string;
  workflow_id: string;
  status: string;
  duration_ms: number;
  node_count: number;
  failed_node_count: number;
  started_at: string;
  completed_at: string;
  node_durations: Record<string, number>;
}

/* ═══════════════════════════════════════════════════════
   Change Requests
   ═══════════════════════════════════════════════════════ */

export type ChangeRequestStatus = "draft" | "open" | "approved" | "rejected" | "merged";

export interface ReviewComment {
  id: string;
  author: string;
  body: string;
  node_id?: string;
  edge_ref?: [string, string];
  resolved: boolean;
  created_at: string;
}

export interface ChangeRequest {
  id: string;
  workflow_id: string;
  title: string;
  description?: string;
  proposed_definition: Record<string, unknown>;
  base_version: number;
  status: ChangeRequestStatus;
  author: string;
  reviewers: string[];
  comments: ReviewComment[];
  created_at: string;
  updated_at: string;
}

export interface CreateChangeRequestInput {
  title: string;
  description?: string;
  proposed_definition: Record<string, unknown>;
  base_version: number;
  author: string;
  reviewers?: string[];
}

export interface AddCommentInput {
  author: string;
  body: string;
  node_id?: string;
  edge_ref?: [string, string];
}

/* ═══════════════════════════════════════════════════════
   Audit verification
   ═══════════════════════════════════════════════════════ */

export interface AuditVerifyResult {
  valid: boolean;
  error?: string;
  event_count: number;
}

/* ═══════════════════════════════════════════════════════
   Plugin Marketplace
   ═══════════════════════════════════════════════════════ */

export interface PluginSummary {
  name: string;
  description: string | null;
  latest_version: string;
  author: string | null;
  downloads: number;
  tags: string[];
  icon: string | null;
  category: string | null;
  color: string | null;
  installed?: boolean;
  update_available?: boolean;
  latest_version_available?: string;
}

export interface PluginDetail {
  name: string;
  version: string;
  description: string | null;
  author: string | null;
  license: string | null;
  repository: string | null;
  node_types: string[];
  orbflow_version: string | null;
  tags: string[];
  icon: string | null;
  category: string | null;
  color: string | null;
  language: string | null;
  readme: string | null;
  downloads: number;
  installed?: boolean;
}

/* ═══════════════════════════════════════════════════════
   Testing Framework
   ═══════════════════════════════════════════════════════ */

export type MatcherType = "equals" | "contains" | "greater_than" | "less_than" | "exists" | "not_exists" | "regex" | "type_of";

export interface TestAssertion {
  field_path: string;
  matcher: MatcherType;
  expected?: unknown;
  message?: string;
}

export interface TestCase {
  name: string;
  node_id: string;
  input_overrides?: Record<string, unknown>;
  assertions: TestAssertion[];
}

export interface TestSuite {
  name: string;
  workflow_id: string;
  description?: string;
  cases: TestCase[];
}

export interface AssertionResult {
  passed: boolean;
  field_path: string;
  matcher: MatcherType;
  expected?: unknown;
  actual?: unknown;
  message?: string;
}

export interface TestCaseResult {
  name: string;
  node_id: string;
  passed: boolean;
  assertions: AssertionResult[];
  error?: string;
  duration_ms: number;
}

export interface TestSuiteResult {
  suite_name: string;
  workflow_id: string;
  total: number;
  passed: number;
  failed: number;
  results: TestCaseResult[];
  duration_ms: number;
  run_at: string;
}

export interface CoverageReport {
  workflow_id: string;
  total_nodes: number;
  tested_nodes: number;
  coverage_pct: number;
  untested_nodes: string[];
}

/* ═══════════════════════════════════════════════════════
   RBAC Policy
   ═══════════════════════════════════════════════════════ */

export type Permission = "view" | "edit" | "execute" | "approve" | "delete" | "manage_credentials" | "admin";

export interface Role {
  id: string;
  name: string;
  description: string;
  permissions: Permission[];
  builtin?: boolean;
}

export interface PolicyBinding {
  subject: string;
  role_id: string;
  scope: PolicyScope;
}

export type PolicyScope =
  | { type: "global" }
  | { type: "workflow"; workflow_id: string }
  | { type: "node"; workflow_id: string; node_id: string };

export interface RbacPolicy {
  roles: Role[];
  bindings: PolicyBinding[];
}

// ─── Budget & Cost Tracking ─────────────────────────────────────────────────

export type BudgetPeriod = "daily" | "weekly" | "monthly";

export interface AccountBudget {
  id: string;
  workflow_id: string | null;
  team: string | null;
  period: BudgetPeriod;
  limit_usd: number;
  current_usd: number;
  reset_at: string;
  created_at: string;
}

export interface CreateBudgetInput {
  id?: string;
  workflow_id?: string;
  team?: string;
  period: BudgetPeriod;
  limit_usd: number;
}

export interface CostAnalytics {
  total_cost_usd: number;
  workflow_costs: WorkflowCost[];
  period_start: string;
  period_end: string;
}

export interface WorkflowCost {
  workflow_id: string;
  workflow_name: string;
  total_cost_usd: number;
  execution_count: number;
  avg_cost_per_execution: number;
}

// ─── Alert Management ───────────────────────────────────────────────────────

export type AlertMetric = "failure_rate" | "p95_duration" | "execution_count";

export type AlertOperator = "greater_than" | "less_than" | "equals";

export type AlertChannel =
  | { type: "webhook"; url: string }
  | { type: "log" };

export interface AlertRule {
  id: string;
  workflow_id: string | null;
  metric: AlertMetric;
  operator: AlertOperator;
  threshold: number;
  channel: AlertChannel;
  enabled: boolean;
  created_at: string;
}

export interface CreateAlertInput {
  workflow_id?: string;
  metric: AlertMetric;
  operator: AlertOperator;
  threshold: number;
  channel: AlertChannel;
  enabled?: boolean;
}

// ─── Audit Trail & Compliance ───────────────────────────────────────────────

export interface AuditRecord {
  seq: number;
  event_hash: string;
  prev_hash: string;
  event_data: string;
  signature?: string;
}

export interface MerkleProofNode {
  hash: string;
  position: "left" | "right";
}

export interface AuditProofResult {
  leaf_hash: string;
  merkle_root: string;
  proof: MerkleProofNode[];
  valid: boolean;
}

export type ComplianceFormat = "soc2" | "hipaa" | "pci";
