/**
 * Configurable API client factory.
 *
 * Creates a typed HTTP client for the Orbflow workflow engine API.
 * The base URL is injected at creation time — no process.env dependency.
 */

import type {
  Workflow,
  Instance,
  PaginatedResult,
  TestNodeResult,
  NodeTypeSchema,
  CredentialSummary,
  Credential,
  CredentialCreate,
  CredentialTypeSchema,
  WorkflowMetricsSummary,
  NodeMetricsSummary,
  InstanceExecutionMetrics,
  AuditVerifyResult,
  RbacPolicy,
  WorkflowVersion,
  WorkflowDiff,
  PluginSummary,
  PluginDetail,
  ChangeRequest,
  ChangeRequestStatus,
  CreateChangeRequestInput,
  ReviewComment,
  AddCommentInput,
  TestSuite,
  TestSuiteResult,
  CoverageReport,
  AccountBudget,
  CreateBudgetInput,
  CostAnalytics,
  AlertRule,
  CreateAlertInput,
  AuditRecord,
  AuditProofResult,
  ComplianceFormat,
} from "../types/api";

/* ═══════════════════════════════════════════════════════
   Internal response envelope
   ═══════════════════════════════════════════════════════ */

interface ApiResponse<T> {
  data: T;
  error?: string;
  meta?: { total: number; offset: number; limit: number };
}

/* ═══════════════════════════════════════════════════════
   Pagination helpers
   ═══════════════════════════════════════════════════════ */

/** Optional pagination cursor accepted by list endpoints. */
export interface ListParams {
  offset?: number;
  limit?: number;
}

/* ═══════════════════════════════════════════════════════
   Public API Client type
   ═══════════════════════════════════════════════════════ */

export interface TestNodeRequest {
  nodeId: string;
  cachedOutputs?: Record<string, Record<string, unknown>>;
}

export interface ApproveNodeRequest {
  approved_by?: string;
}

export interface RejectNodeRequest {
  reason?: string;
  rejected_by?: string;
}

export interface ApiClient {
  workflows: {
    list: (params?: ListParams) => Promise<PaginatedResult<Workflow>>;
    get: (id: string) => Promise<Workflow>;
    create: (wf: Partial<Workflow>) => Promise<Workflow>;
    update: (id: string, wf: Partial<Workflow>) => Promise<Workflow>;
    start: (id: string, input?: Record<string, unknown>) => Promise<Instance>;
    testNode: (id: string, req: TestNodeRequest) => Promise<TestNodeResult>;
  };
  instances: {
    list: (params?: ListParams) => Promise<PaginatedResult<Instance>>;
    get: (id: string) => Promise<Instance>;
    cancel: (id: string) => Promise<{ status: string }>;
    approveNode: (instanceId: string, nodeId: string, req?: ApproveNodeRequest) => Promise<{ status: string }>;
    rejectNode: (instanceId: string, nodeId: string, req?: RejectNodeRequest) => Promise<{ status: string }>;
    /** Returns the SSE stream URL for a node's real-time output. */
    streamUrl: (instanceId: string, nodeId: string) => string;
    /** Verify the audit trail integrity for an instance. */
    verifyAudit: (instanceId: string) => Promise<AuditVerifyResult>;
    /** Fetch the full audit trail for an instance. */
    getAuditTrail: (instanceId: string) => Promise<AuditRecord[]>;
    /** Get a Merkle proof for a specific event in the audit trail. */
    getAuditProof: (instanceId: string, eventIndex: number) => Promise<AuditProofResult>;
    /** Returns the download URL for a compliance export of the audit trail. */
    exportAuditTrail: (instanceId: string, format: ComplianceFormat) => string;
  };
  nodeTypes: {
    list: () => Promise<NodeTypeSchema[]>;
  };
  credentialTypes: {
    list: () => Promise<CredentialTypeSchema[]>;
  };
  credentials: {
    list: () => Promise<CredentialSummary[]>;
    get: (id: string) => Promise<Credential>;
    create: (cred: CredentialCreate) => Promise<Credential>;
    update: (id: string, cred: Partial<CredentialCreate>) => Promise<Credential>;
    delete: (id: string) => Promise<{ status: string }>;
  };
  metrics: {
    getWorkflowMetrics: (workflowId: string) => Promise<WorkflowMetricsSummary>;
    getWorkflowNodeMetrics: (workflowId: string) => Promise<NodeMetricsSummary[]>;
    getInstanceMetrics: (instanceId: string) => Promise<InstanceExecutionMetrics | null>;
  };
  versions: {
    list: (workflowId: string, params?: ListParams) => Promise<PaginatedResult<WorkflowVersion>>;
    get: (workflowId: string, version: number) => Promise<WorkflowVersion>;
    diff: (workflowId: string, from: number, to: number) => Promise<WorkflowDiff>;
  };
  rbac: {
    getPolicy: () => Promise<RbacPolicy>;
    updatePolicy: (policy: RbacPolicy) => Promise<RbacPolicy>;
    listSubjects: () => Promise<string[]>;
  };
  marketplace: {
    list: (params?: { query?: string; category?: string; sort?: string; order?: string; installed_only?: boolean } & ListParams) => Promise<PaginatedResult<PluginSummary>>;
    get: (name: string) => Promise<PluginDetail>;
    install: (name: string) => Promise<{ name: string; status: string; path: string }>;
    uninstall: (name: string) => Promise<{ name: string; status: string }>;
    validateManifest: (manifest: Record<string, unknown>) => Promise<{ valid: boolean; errors?: string[] }>;
  };
  changeRequests: {
    list: (workflowId: string, params?: ListParams & { status?: ChangeRequestStatus }) => Promise<PaginatedResult<ChangeRequest>>;
    get: (workflowId: string, crId: string) => Promise<ChangeRequest>;
    create: (workflowId: string, input: CreateChangeRequestInput) => Promise<ChangeRequest>;
    update: (workflowId: string, crId: string, input: Partial<CreateChangeRequestInput>) => Promise<ChangeRequest>;
    submit: (workflowId: string, crId: string) => Promise<{ status: string }>;
    approve: (workflowId: string, crId: string) => Promise<{ status: string }>;
    reject: (workflowId: string, crId: string, reason?: string) => Promise<{ status: string }>;
    merge: (workflowId: string, crId: string) => Promise<{ status: string }>;
    rebase: (workflowId: string, crId: string) => Promise<{ status: string; base_version: number; cr_status?: string }>;
    addComment: (workflowId: string, crId: string, comment: AddCommentInput) => Promise<ReviewComment>;
    resolveComment: (workflowId: string, crId: string, commentId: string) => Promise<{ status: string }>;
  };
  testing: {
    runSuite: (workflowId: string, suite: TestSuite) => Promise<TestSuiteResult>;
    getCoverage: (workflowId: string, suite: TestSuite) => Promise<CoverageReport>;
  };
  budgets: {
    list: () => Promise<AccountBudget[]>;
    create: (budget: CreateBudgetInput) => Promise<AccountBudget>;
    update: (id: string, budget: Partial<CreateBudgetInput>) => Promise<AccountBudget>;
    delete: (id: string) => Promise<{ status: string }>;
    costs: (range?: string) => Promise<CostAnalytics>;
  };
  alerts: {
    list: () => Promise<AlertRule[]>;
    create: (rule: CreateAlertInput) => Promise<AlertRule>;
    update: (id: string, rule: Partial<CreateAlertInput>) => Promise<AlertRule>;
    delete: (id: string) => Promise<{ status: string }>;
  };
}

/* ═══════════════════════════════════════════════════════
   Factory
   ═══════════════════════════════════════════════════════ */

/** Options for creating an API client. */
export interface ApiClientOptions {
  /** Root URL of the Orbflow API (e.g. "http://localhost:8080") */
  baseUrl: string;
  /** Optional bearer token for authenticated requests. */
  authToken?: string;
}

/**
 * Create a fully-typed API client bound to the given base URL.
 *
 * @param baseUrlOrOpts - Root URL string, or an options object with baseUrl and optional authToken.
 * @returns An `ApiClient` object with namespaced methods for every resource.
 */
export function createApiClient(baseUrlOrOpts: string | ApiClientOptions): ApiClient {
  const opts = typeof baseUrlOrOpts === "string" ? { baseUrl: baseUrlOrOpts } : baseUrlOrOpts;
  const { baseUrl, authToken } = opts;

  function buildHeaders(): Record<string, string> {
    const h: Record<string, string> = { "Content-Type": "application/json" };
    if (authToken) h["Authorization"] = `Bearer ${authToken}`;
    return h;
  }

  async function rawRequest<T>(path: string, options?: RequestInit): Promise<ApiResponse<T>> {
    const method = options?.method?.toUpperCase() ?? "GET";
    const headers = buildHeaders();
    // Don't send Content-Type on GET/HEAD — it's unnecessary and triggers CORS preflight
    if (method === "GET" || method === "HEAD") {
      delete headers["Content-Type"];
    }
    const res = await fetch(`${baseUrl}${path}`, {
      headers,
      ...options,
    });

    // Handle non-JSON responses (e.g., HTML error pages from reverse proxy)
    const contentType = res.headers.get("content-type") || "";
    if (!contentType.includes("application/json")) {
      if (!res.ok) {
        const text = await res.text().catch(() => "");
        throw new Error(
          `Server error ${res.status}: ${text.slice(0, 200) || res.statusText}`
        );
      }
      throw new Error(`Unexpected response type: ${contentType || "empty"}`);
    }

    const json: ApiResponse<T> = await res.json();
    if (json.error) throw new Error(json.error);

    if (!res.ok) {
      const raw = json as unknown as Record<string, unknown>;
      const msg = raw.message || raw.detail || `Request failed with status ${res.status}`;
      throw new Error(String(msg));
    }

    return json;
  }

  async function request<T>(path: string, options?: RequestInit): Promise<T> {
    const json = await rawRequest<T>(path, options);
    return json.data;
  }

  /** Fetch a list endpoint and return data + pagination metadata. */
  async function requestPaged<T>(path: string, params?: ListParams): Promise<PaginatedResult<T>> {
    const search = new URLSearchParams();
    if (params?.offset !== undefined) search.set("offset", String(params.offset));
    if (params?.limit !== undefined) search.set("limit", String(params.limit));
    const qs = search.toString();
    const json = await rawRequest<T[]>(`${path}${qs ? `?${qs}` : ""}`);
    const meta = json.meta ?? { total: Array.isArray(json.data) ? json.data.length : 0, offset: params?.offset ?? 0, limit: params?.limit ?? 20 };
    return {
      items: json.data,
      total: meta.total,
      offset: meta.offset,
      limit: meta.limit,
    };
  }

  return {
    workflows: {
      list: (params) => requestPaged<Workflow>("/workflows", params),
      get: (id) => request<Workflow>(`/workflows/${encodeURIComponent(id)}`),
      create: (wf) =>
        request<Workflow>("/workflows", {
          method: "POST",
          body: JSON.stringify(wf),
        }),
      update: (id, wf) =>
        request<Workflow>(`/workflows/${encodeURIComponent(id)}`, {
          method: "PUT",
          body: JSON.stringify(wf),
        }),
      start: (id, input) =>
        request<Instance>(`/workflows/${encodeURIComponent(id)}/start`, {
          method: "POST",
          body: JSON.stringify(input || {}),
        }),
      testNode: (id, req) =>
        request<TestNodeResult>(`/workflows/${encodeURIComponent(id)}/test-node`, {
          method: "POST",
          body: JSON.stringify({ node_id: req.nodeId, cached_outputs: req.cachedOutputs }),
        }),
    },
    instances: {
      list: (params) => requestPaged<Instance>("/instances", params),
      get: (id) => request<Instance>(`/instances/${encodeURIComponent(id)}`),
      cancel: (id) =>
        request<{ status: string }>(`/instances/${encodeURIComponent(id)}/cancel`, {
          method: "POST",
        }),
      approveNode: (instanceId, nodeId, req) =>
        request<{ status: string }>(`/instances/${encodeURIComponent(instanceId)}/nodes/${encodeURIComponent(nodeId)}/approve`, {
          method: "POST",
          body: JSON.stringify(req || {}),
        }),
      rejectNode: (instanceId, nodeId, req) =>
        request<{ status: string }>(`/instances/${encodeURIComponent(instanceId)}/nodes/${encodeURIComponent(nodeId)}/reject`, {
          method: "POST",
          body: JSON.stringify(req || {}),
        }),
      streamUrl: (instanceId, nodeId) =>
        `${baseUrl}/instances/${encodeURIComponent(instanceId)}/nodes/${encodeURIComponent(nodeId)}/stream`,
      verifyAudit: (instanceId) =>
        request<AuditVerifyResult>(`/instances/${encodeURIComponent(instanceId)}/audit/verify`),
      getAuditTrail: (instanceId) =>
        request<AuditRecord[]>(`/instances/${encodeURIComponent(instanceId)}/audit/trail`),
      getAuditProof: (instanceId, eventIndex) =>
        request<AuditProofResult>(`/instances/${encodeURIComponent(instanceId)}/audit/proof/${encodeURIComponent(String(eventIndex))}`),
      exportAuditTrail: (instanceId, format) =>
        `${baseUrl}/instances/${encodeURIComponent(instanceId)}/audit/export?format=${encodeURIComponent(format)}`,
    },
    nodeTypes: {
      list: () => request<NodeTypeSchema[]>("/node-types"),
    },
    credentialTypes: {
      list: () => request<CredentialTypeSchema[]>("/credential-types"),
    },
    credentials: {
      list: () => request<CredentialSummary[]>("/credentials"),
      get: (id) => request<Credential>(`/credentials/${encodeURIComponent(id)}`),
      create: (cred) =>
        request<Credential>("/credentials", {
          method: "POST",
          body: JSON.stringify(cred),
        }),
      update: (id, cred) =>
        request<Credential>(`/credentials/${encodeURIComponent(id)}`, {
          method: "PUT",
          body: JSON.stringify(cred),
        }),
      delete: (id) =>
        request<{ status: string }>(`/credentials/${encodeURIComponent(id)}`, {
          method: "DELETE",
        }),
    },
    metrics: {
      getWorkflowMetrics: (workflowId) =>
        request<WorkflowMetricsSummary>(`/workflows/${encodeURIComponent(workflowId)}/metrics`),
      getWorkflowNodeMetrics: (workflowId) =>
        request<NodeMetricsSummary[]>(`/workflows/${encodeURIComponent(workflowId)}/metrics/nodes`),
      getInstanceMetrics: (instanceId) =>
        request<InstanceExecutionMetrics | null>(`/instances/${encodeURIComponent(instanceId)}/metrics`),
    },
    versions: {
      list: (workflowId, params) =>
        requestPaged<WorkflowVersion>(`/workflows/${encodeURIComponent(workflowId)}/versions`, params),
      get: (workflowId, version) =>
        request<WorkflowVersion>(`/workflows/${encodeURIComponent(workflowId)}/versions/${encodeURIComponent(String(version))}`),
      diff: (workflowId, from, to) =>
        request<WorkflowDiff>(`/workflows/${encodeURIComponent(workflowId)}/diff?from=${encodeURIComponent(String(from))}&to=${encodeURIComponent(String(to))}`),
    },
    rbac: {
      getPolicy: () => request<RbacPolicy>("/rbac/policy"),
      updatePolicy: (policy) =>
        request<RbacPolicy>("/rbac/policy", {
          method: "PUT",
          body: JSON.stringify(policy),
        }),
      listSubjects: () => request<string[]>("/rbac/subjects"),
    },
    marketplace: {
      list: async (params) => {
        const search = new URLSearchParams();
        if (params?.query) search.set("q", params.query);
        if (params?.category) search.set("category", params.category);
        if (params?.sort) search.set("sort", params.sort);
        if (params?.order) search.set("order", params.order);
        if (params?.installed_only) search.set("installed_only", "true");
        if (params?.offset !== undefined) search.set("offset", String(params.offset));
        if (params?.limit !== undefined) search.set("limit", String(params.limit));
        const qs = search.toString();
        const json = await rawRequest<PluginSummary[]>(`/marketplace/plugins${qs ? `?${qs}` : ""}`);
        const meta = json.meta ?? { total: Array.isArray(json.data) ? json.data.length : 0, offset: params?.offset ?? 0, limit: params?.limit ?? 20 };
        return { items: json.data, total: meta.total, offset: meta.offset, limit: meta.limit };
      },
      get: (name) => request<PluginDetail>(`/marketplace/plugins/${encodeURIComponent(name)}`),
      install: (name) =>
        request<{ name: string; status: string; path: string }>(
          `/marketplace/plugins/${encodeURIComponent(name)}/install`,
          { method: "POST" },
        ),
      uninstall: (name) =>
        request<{ name: string; status: string }>(
          `/marketplace/plugins/${encodeURIComponent(name)}`,
          { method: "DELETE" },
        ),
      validateManifest: (manifest) =>
        request<{ valid: boolean; errors?: string[] }>(
          "/marketplace/validate-manifest",
          { method: "POST", body: JSON.stringify(manifest) },
        ),
    },
    changeRequests: {
      list: async (workflowId, params) => {
        const wid = encodeURIComponent(workflowId);
        const search = new URLSearchParams();
        if (params?.status) search.set("status", params.status);
        if (params?.offset !== undefined) search.set("offset", String(params.offset));
        if (params?.limit !== undefined) search.set("limit", String(params.limit));
        const qs = search.toString();
        const json = await rawRequest<ChangeRequest[]>(
          `/workflows/${wid}/change-requests${qs ? `?${qs}` : ""}`
        );
        const meta = json.meta ?? {
          total: Array.isArray(json.data) ? json.data.length : 0,
          offset: params?.offset ?? 0,
          limit: params?.limit ?? 20,
        };
        return { items: json.data, total: meta.total, offset: meta.offset, limit: meta.limit };
      },
      get: (workflowId, crId) =>
        request<ChangeRequest>(`/workflows/${encodeURIComponent(workflowId)}/change-requests/${encodeURIComponent(crId)}`),
      create: (workflowId, input) =>
        request<ChangeRequest>(`/workflows/${encodeURIComponent(workflowId)}/change-requests`, {
          method: "POST",
          body: JSON.stringify(input),
        }),
      update: (workflowId, crId, input) =>
        request<ChangeRequest>(`/workflows/${encodeURIComponent(workflowId)}/change-requests/${encodeURIComponent(crId)}`, {
          method: "PUT",
          body: JSON.stringify(input),
        }),
      submit: (workflowId, crId) =>
        request<{ status: string }>(`/workflows/${encodeURIComponent(workflowId)}/change-requests/${encodeURIComponent(crId)}/submit`, {
          method: "POST",
        }),
      approve: (workflowId, crId) =>
        request<{ status: string }>(`/workflows/${encodeURIComponent(workflowId)}/change-requests/${encodeURIComponent(crId)}/approve`, {
          method: "POST",
        }),
      reject: (workflowId, crId, reason) =>
        request<{ status: string }>(`/workflows/${encodeURIComponent(workflowId)}/change-requests/${encodeURIComponent(crId)}/reject`, {
          method: "POST",
          body: reason ? JSON.stringify({ reason }) : JSON.stringify({}),
        }),
      merge: (workflowId, crId) =>
        request<{ status: string }>(`/workflows/${encodeURIComponent(workflowId)}/change-requests/${encodeURIComponent(crId)}/merge`, {
          method: "POST",
        }),
      rebase: (workflowId, crId) =>
        request<{ status: string; base_version: number; cr_status?: string }>(`/workflows/${encodeURIComponent(workflowId)}/change-requests/${encodeURIComponent(crId)}/rebase`, {
          method: "POST",
        }),
      addComment: (workflowId, crId, comment) =>
        request<ReviewComment>(`/workflows/${encodeURIComponent(workflowId)}/change-requests/${encodeURIComponent(crId)}/comments`, {
          method: "POST",
          body: JSON.stringify(comment),
        }),
      resolveComment: (workflowId, crId, commentId) =>
        request<{ status: string }>(
          `/workflows/${encodeURIComponent(workflowId)}/change-requests/${encodeURIComponent(crId)}/comments/${encodeURIComponent(commentId)}/resolve`,
          { method: "POST" }
        ),
    },
    testing: {
      runSuite: (workflowId, suite) =>
        request<TestSuiteResult>(`/workflows/${encodeURIComponent(workflowId)}/test-suite`, {
          method: "POST",
          body: JSON.stringify(suite),
        }),
      getCoverage: (workflowId, suite) =>
        request<CoverageReport>(`/workflows/${encodeURIComponent(workflowId)}/test-coverage`, {
          method: "POST",
          body: JSON.stringify(suite),
        }),
    },
    budgets: {
      list: () => request<AccountBudget[]>("/budgets"),
      create: (budget) =>
        request<AccountBudget>("/budgets", {
          method: "POST",
          body: JSON.stringify(budget),
        }),
      update: (id, budget) =>
        request<AccountBudget>(`/budgets/${encodeURIComponent(id)}`, {
          method: "PUT",
          body: JSON.stringify(budget),
        }),
      delete: (id) =>
        request<{ status: string }>(`/budgets/${encodeURIComponent(id)}`, {
          method: "DELETE",
        }),
      costs: (range) =>
        request<CostAnalytics>(`/analytics/costs${range ? `?range=${encodeURIComponent(range)}` : ""}`),
    },
    alerts: {
      list: () => request<AlertRule[]>("/alerts"),
      create: (rule) =>
        request<AlertRule>("/alerts", {
          method: "POST",
          body: JSON.stringify(rule),
        }),
      update: (id, rule) =>
        request<AlertRule>(`/alerts/${encodeURIComponent(id)}`, {
          method: "PUT",
          body: JSON.stringify(rule),
        }),
      delete: (id) =>
        request<{ status: string }>(`/alerts/${encodeURIComponent(id)}`, {
          method: "DELETE",
        }),
    },
  };
}
