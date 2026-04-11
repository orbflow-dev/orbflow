# Orbflow REST API Reference

## 1. Overview

Orbflow exposes a versioned REST API for managing workflows, executions, credentials, and platform configuration.

| Property | Value |
|----------|-------|
| **Base URL** | `http://localhost:8080/api/v1` |
| **Content-Type** | `application/json` |
| **Max request body** | 1 MB |
| **Authentication** | Bearer token via `Authorization` header |
| **User identity** | `X-User-Id` header (when `trust_x_user_id` is enabled) |

### Response Envelope

All responses follow a consistent JSON envelope:

**Single item:**

```json
{
  "data": { ... }
}
```

**Paginated list:**

```json
{
  "data": [ ... ],
  "meta": {
    "total": 42,
    "offset": 0,
    "limit": 20
  }
}
```

**Error:**

```json
{
  "data": null,
  "error": "human-readable error message"
}
```

---

## 2. Authentication

### Bearer Token Authentication

When the server is started with an `auth_token` configured (via `ORBFLOW_AUTH_TOKEN` environment variable or config file), all requests to non-public paths must include a valid bearer token.

```
Authorization: Bearer <token>
```

**Public paths** that bypass authentication:

| Path prefix | Purpose |
|-------------|---------|
| `/health` | Health check probes |
| `/node-types` | Node type schema discovery |
| `/credential-types` | Credential type schema discovery |
| `/webhooks/` | Inbound webhook triggers |

### Auth-Free Mode (Development)

When no `auth_token` is configured, authentication is disabled entirely. All requests pass through and are attributed to the user `"anonymous"`.

### User Identity

When `trust_x_user_id` is enabled (set `ORBFLOW_TRUST_X_USER_ID=true`), the server reads the caller's identity from the `X-User-Id` header. This should only be enabled when the API sits behind a trusted gateway that injects a verified header.

When disabled (default), all requests are attributed to `"anonymous"`.

### Bootstrap Admin

Set `ORBFLOW_BOOTSTRAP_ADMIN=<user_id>` to designate an initial admin user. This user has full access even when RBAC bindings are empty, allowing initial policy setup.

---

## 3. Health

### GET /health

Returns the server health status. Also available at the root path `/health` (outside `/api/v1`) for load balancer probes.

**Permission:** None (public)

**Response:**

```json
{
  "status": "ok"
}
```

---

## 4. Workflows

### POST /workflows

Create a new workflow definition.

**Permission:** `Edit`

**Request body:**

```json
{
  "name": "Order Processing",
  "description": "Handles incoming orders end-to-end",
  "nodes": [
    {
      "id": "fetch-order",
      "type": "http_request",
      "label": "Fetch Order",
      "config": {
        "url": "https://api.example.com/orders/{{order_id}}",
        "method": "GET"
      },
      "position": { "x": 100, "y": 200 }
    }
  ],
  "edges": [
    {
      "source": "trigger",
      "target": "fetch-order"
    }
  ]
}
```

**Response:** `201 Created`

```json
{
  "data": {
    "id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
    "name": "Order Processing",
    "description": "Handles incoming orders end-to-end",
    "version": 0,
    "status": "draft",
    "nodes": [ ... ],
    "edges": [ ... ],
    "created_at": "2026-03-22T10:00:00Z",
    "updated_at": "2026-03-22T10:00:00Z"
  }
}
```

**Errors:**

| Status | Condition |
|--------|-----------|
| 400 | Validation error (e.g., cycle detected, invalid node config) |

---

### GET /workflows

List all workflow definitions with pagination.

**Permission:** `View`

**Query parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `offset` | integer | `0` | Number of items to skip |
| `limit` | integer | `20` | Number of items to return (max 100) |

**Response:** `200 OK`

```json
{
  "data": [
    {
      "id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
      "name": "Order Processing",
      "version": 3,
      "status": "active",
      "nodes": [ ... ],
      "edges": [ ... ],
      "created_at": "2026-03-20T08:00:00Z",
      "updated_at": "2026-03-22T10:00:00Z"
    }
  ],
  "meta": {
    "total": 42,
    "offset": 0,
    "limit": 20
  }
}
```

---

### GET /workflows/{id}

Retrieve a single workflow by ID.

**Permission:** `View` (scoped to workflow)

**Path parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | string | Workflow ID (UUID) |

**Response:** `200 OK`

```json
{
  "data": {
    "id": "a1b2c3d4-...",
    "name": "Order Processing",
    "version": 3,
    "status": "active",
    "nodes": [ ... ],
    "edges": [ ... ],
    "created_at": "2026-03-20T08:00:00Z",
    "updated_at": "2026-03-22T10:00:00Z"
  }
}
```

**Errors:**

| Status | Condition |
|--------|-----------|
| 404 | Workflow not found |

---

### PUT /workflows/{id}

Update an existing workflow definition. Increments the version automatically.

**Permission:** `Edit` (scoped to workflow)

**Path parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | string | Workflow ID (UUID) |

**Request body:** Same shape as `POST /workflows`.

**Response:** `200 OK` with the updated workflow object.

**Errors:**

| Status | Condition |
|--------|-----------|
| 400 | Validation error |
| 404 | Workflow not found |
| 409 | Version conflict (reload and retry) |

---

### DELETE /workflows/{id}

Delete a workflow definition.

**Permission:** `Delete` (scoped to workflow)

**Path parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | string | Workflow ID (UUID) |

**Response:** `200 OK`

```json
{
  "data": {
    "deleted": true
  }
}
```

**Errors:**

| Status | Condition |
|--------|-----------|
| 404 | Workflow not found |

---

### POST /workflows/{id}/start

Start a new execution of a workflow. Subject to per-workflow rate limiting (minimum 2 seconds between starts of the same workflow).

**Permission:** `Execute` (scoped to workflow)

**Path parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | string | Workflow ID (UUID) |

**Request body:** Input data as a JSON object. Keys are available to nodes via CEL expressions.

```json
{
  "order_id": "ORD-12345",
  "priority": "high"
}
```

**Response:** `200 OK`

```json
{
  "data": {
    "id": "inst-abcdef12-3456-7890-abcd-ef1234567890",
    "workflow_id": "a1b2c3d4-...",
    "status": "running",
    "node_states": { ... },
    "context": {
      "input": { "order_id": "ORD-12345", "priority": "high" },
      "outputs": {}
    },
    "version": 1,
    "created_at": "2026-03-22T10:05:00Z"
  }
}
```

**Errors:**

| Status | Condition |
|--------|-----------|
| 400 | Validation error (e.g., invalid input) |
| 404 | Workflow not found |
| 429 | Rate limited (workflow started too recently) |

---

### POST /workflows/{id}/test-node

Execute a single node in isolation for testing purposes. Uses the same rate limiter as `/start`.

**Permission:** `Execute` (scoped to workflow)

**Request body:**

```json
{
  "node_id": "fetch-order",
  "cached_outputs": {
    "previous-node": {
      "result": "some value"
    }
  }
}
```

**Response:** `200 OK` with the node execution result.

**Errors:**

| Status | Condition |
|--------|-----------|
| 400 | `node_id` is empty |
| 404 | Workflow not found |
| 422 | Node executor not found |
| 429 | Rate limited |

---

### POST /workflows/{id}/test-suite

Run a test suite against a workflow. Maximum 100 test cases per suite. Times out after 5 minutes.

**Permission:** `Execute` (scoped to workflow)

**Request body:**

```json
{
  "workflow_id": "a1b2c3d4-...",
  "cases": [
    {
      "name": "Happy path",
      "input": { "order_id": "ORD-001" },
      "expected_status": "completed"
    }
  ]
}
```

**Response:** `200 OK` with test suite results.

**Errors:**

| Status | Condition |
|--------|-----------|
| 400 | Suite `workflow_id` does not match path, or exceeds 100 cases |
| 404 | Workflow not found |
| 504 | Test suite execution timed out |

---

### POST /workflows/{id}/test-coverage

Compute test coverage for a workflow given a test suite definition.

**Permission:** `View` (scoped to workflow)

**Request body:** Same shape as test suite (see `POST /workflows/{id}/test-suite`).

**Response:** `200 OK` with coverage report.

**Errors:**

| Status | Condition |
|--------|-----------|
| 404 | Workflow not found |

---

## 5. Instances

### GET /instances

List all workflow execution instances with pagination.

**Permission:** `View`

**Query parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `offset` | integer | `0` | Number of items to skip |
| `limit` | integer | `20` | Number of items to return (max 100) |

**Response:** `200 OK`

```json
{
  "data": [
    {
      "id": "inst-abcdef12-...",
      "workflow_id": "a1b2c3d4-...",
      "status": "completed",
      "node_states": { ... },
      "context": { ... },
      "version": 5,
      "created_at": "2026-03-22T10:05:00Z"
    }
  ],
  "meta": {
    "total": 128,
    "offset": 0,
    "limit": 20
  }
}
```

---

### GET /instances/{id}

Retrieve a single instance by ID.

**Permission:** `View`

**Path parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | string | Instance ID (UUID) |

**Response:** `200 OK` with the full instance object.

**Errors:**

| Status | Condition |
|--------|-----------|
| 404 | Instance not found |

---

### POST /instances/{id}/cancel

Cancel a running instance.

**Permission:** `Execute`

**Path parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | string | Instance ID (UUID) |

**Response:** `200 OK`

```json
{
  "data": {
    "status": "cancelled"
  }
}
```

**Errors:**

| Status | Condition |
|--------|-----------|
| 404 | Instance not found |

---

## 6. Approval Gates

### POST /instances/{instance_id}/nodes/{node_id}/approve

Approve a node that is in `WaitingApproval` state.

**Permission:** `Approve` (scoped to node)

**Path parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `instance_id` | string | Instance ID |
| `node_id` | string | Node ID within the workflow |

**Request body:**

```json
{
  "approved_by": "alice@example.com"
}
```

**Response:** `200 OK`

```json
{
  "data": {
    "status": "approved"
  }
}
```

**Errors:**

| Status | Condition |
|--------|-----------|
| 400 | Node is not in a valid state for approval |
| 404 | Instance or node not found |

---

### POST /instances/{instance_id}/nodes/{node_id}/reject

Reject a node that is in `WaitingApproval` state.

**Permission:** `Approve` (scoped to node)

**Request body:**

```json
{
  "reason": "Missing required data",
  "rejected_by": "bob@example.com"
}
```

**Response:** `200 OK`

```json
{
  "data": {
    "status": "rejected"
  }
}
```

**Errors:**

| Status | Condition |
|--------|-----------|
| 400 | Node is not in a valid state for rejection |
| 404 | Instance or node not found |

---

## 7. Workflow Versions

### GET /workflows/{id}/versions

List all versions of a workflow definition.

**Permission:** `View` (scoped to workflow)

**Query parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `offset` | integer | `0` | Number of items to skip |
| `limit` | integer | `20` | Number of items to return (max 100) |

**Response:** `200 OK` with paginated list of version objects.

**Errors:**

| Status | Condition |
|--------|-----------|
| 404 | Workflow not found |

---

### GET /workflows/{id}/versions/{version}

Retrieve a specific version of a workflow definition.

**Permission:** `View` (scoped to workflow)

**Path parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | string | Workflow ID |
| `version` | integer | Version number |

**Response:** `200 OK` with the versioned workflow definition.

**Errors:**

| Status | Condition |
|--------|-----------|
| 404 | Version not found |

---

### GET /workflows/{id}/diff

Compare two versions of a workflow definition.

**Permission:** `View` (scoped to workflow)

**Query parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `from` | integer | Yes | Source version number |
| `to` | integer | Yes | Target version number |

**Response:** `200 OK`

```json
{
  "data": {
    "from_version": 1,
    "to_version": 3,
    "added_nodes": ["node-c"],
    "removed_nodes": ["node-b"],
    "modified_nodes": ["node-a"],
    "added_edges": [ ... ],
    "removed_edges": [ ... ]
  }
}
```

**Errors:**

| Status | Condition |
|--------|-----------|
| 404 | One or both versions not found |

---

## 8. Change Requests (PR-Style Collaboration)

Change requests enable a review-and-approve workflow for modifying workflow definitions. They follow a lifecycle: `Draft` -> `Open` -> `Approved` -> `Merged` (or `Rejected`).

> Requires a `ChangeRequestStore` to be configured on the server.

### POST /workflows/{id}/change-requests

Create a new change request for a workflow.

**Permission:** `Edit` (scoped to workflow)

**Request body:**

```json
{
  "title": "Add email notification step",
  "description": "Adds a notification node after order completion",
  "proposed_definition": { ... },
  "base_version": 3,
  "author": "alice@example.com",
  "reviewers": ["bob@example.com", "carol@example.com"]
}
```

**Validation:**

| Field | Constraint |
|-------|-----------|
| `title` | Required, non-empty, max 200 characters |
| `author` | Required, non-empty, max 100 characters |
| `description` | Optional, max 5000 characters |

**Response:** `201 Created` with the full change request object.

---

### GET /workflows/{id}/change-requests

List change requests for a workflow.

**Permission:** `View` (scoped to workflow)

**Query parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `offset` | integer | `0` | Number of items to skip |
| `limit` | integer | `20` | Number of items to return (max 100) |
| `status` | string | none | Filter by status: `draft`, `open`, `approved`, `rejected`, `merged` |

**Response:** `200 OK` with paginated list.

---

### GET /workflows/{id}/change-requests/{cr_id}

Retrieve a single change request.

**Permission:** `View` (scoped to workflow)

**Response:** `200 OK` with the full change request object including comments.

**Errors:**

| Status | Condition |
|--------|-----------|
| 404 | Change request not found or does not belong to this workflow |

---

### PUT /workflows/{id}/change-requests/{cr_id}

Update a change request. Only allowed when status is `Draft` or `Open`.

**Permission:** `Edit` (scoped to workflow)

**Request body:**

```json
{
  "title": "Updated title",
  "description": "Updated description",
  "proposed_definition": { ... },
  "reviewers": ["bob@example.com"]
}
```

All fields are optional; only provided fields are updated.

**Errors:**

| Status | Condition |
|--------|-----------|
| 400 | Validation error (empty title, etc.) |
| 404 | Change request not found |
| 409 | Cannot update in current status |

---

### POST /workflows/{id}/change-requests/{cr_id}/submit

Transition a change request from `Draft` to `Open`, making it visible to reviewers.

**Permission:** `Edit` (scoped to workflow)

**Request body:** None

**Response:** `200 OK`

```json
{
  "data": {
    "status": "open"
  }
}
```

**Errors:**

| Status | Condition |
|--------|-----------|
| 404 | Change request not found |
| 409 | Change request is not in `draft` status |

---

### POST /workflows/{id}/change-requests/{cr_id}/approve

Approve an open change request. Authors cannot approve their own change requests.

**Permission:** `Approve` (scoped to workflow)

**Request body:** None

**Response:** `200 OK`

```json
{
  "data": {
    "status": "approved"
  }
}
```

**Errors:**

| Status | Condition |
|--------|-----------|
| 403 | Cannot approve your own change request |
| 404 | Change request not found |
| 409 | Change request is not in `open` status |

---

### POST /workflows/{id}/change-requests/{cr_id}/reject

Reject an open change request. Authors cannot reject their own change requests.

**Permission:** `Approve` (scoped to workflow)

**Request body:**

```json
{
  "reason": "Does not meet requirements"
}
```

**Response:** `200 OK`

```json
{
  "data": {
    "status": "rejected"
  }
}
```

**Errors:**

| Status | Condition |
|--------|-----------|
| 403 | Cannot reject your own change request |
| 404 | Change request not found |
| 409 | Change request is not in `open` status |

---

### POST /workflows/{id}/change-requests/{cr_id}/rebase

Rebase a change request to the workflow's current version. If the base version changed, an `Approved` CR is reset to `Open` (re-review required).

**Permission:** `Edit` (scoped to workflow)

**Request body:** None

**Response:** `200 OK`

```json
{
  "data": {
    "status": "open",
    "base_version": 5
  }
}
```

**Errors:**

| Status | Condition |
|--------|-----------|
| 404 | Change request not found |
| 409 | Cannot rebase in current status (must be `Open` or `Approved`) |

---

### POST /workflows/{id}/change-requests/{cr_id}/merge

Merge an approved change request into the workflow definition. This operation is atomic: it locks both the change request and workflow rows, verifies the base version has not changed, and updates both in a single transaction.

**Permission:** `Edit` (scoped to workflow)

**Request body:** None

**Response:** `200 OK`

```json
{
  "data": {
    "status": "merged"
  }
}
```

**Errors:**

| Status | Condition |
|--------|-----------|
| 404 | Workflow or change request not found |
| 409 | Base version is stale or status changed (reload and retry) |

---

### POST /workflows/{id}/change-requests/{cr_id}/comments

Add a review comment to a change request. Comments can optionally reference a specific node or edge.

**Permission:** `View` (scoped to workflow)

**Request body:**

```json
{
  "author": "bob@example.com",
  "body": "This node should use retry logic.",
  "node_id": "fetch-order",
  "edge_ref": null
}
```

**Validation:**

| Field | Constraint |
|-------|-----------|
| `author` | Required, non-empty, max 100 characters |
| `body` | Required, non-empty, max 5000 characters |
| `node_id` | Optional, references a node in the workflow |
| `edge_ref` | Optional, tuple of `[source_id, target_id]` |

**Response:** `201 Created`

```json
{
  "data": {
    "id": "comment-uuid",
    "author": "bob@example.com",
    "body": "This node should use retry logic.",
    "node_id": "fetch-order",
    "edge_ref": null,
    "resolved": false,
    "created_at": "2026-03-22T11:00:00Z"
  }
}
```

---

### POST /workflows/{id}/change-requests/{cr_id}/comments/{comment_id}/resolve

Mark a review comment as resolved.

**Permission:** `Edit` (scoped to workflow)

**Request body:** None

**Response:** `200 OK`

```json
{
  "data": {
    "status": "resolved"
  }
}
```

**Errors:**

| Status | Condition |
|--------|-----------|
| 404 | Change request or comment not found |

---

## 9. Credentials

Manage encrypted credentials (API keys, OAuth tokens, etc.) used by workflow nodes. Credential values are encrypted with AES-256-GCM; the API never returns raw secrets -- only summaries.

> Requires a `CredentialStore` to be configured on the server.

### POST /credentials

Create a new credential.

**Permission:** `ManageCredentials`

**Request body:**

```json
{
  "name": "Stripe API Key",
  "type": "api_key",
  "data": {
    "api_key": "sk_live_abc123..."
  },
  "description": "Production Stripe key",
  "access_tier": "proxy"
}
```

**Response:** `201 Created` (returns a summary without secret values)

```json
{
  "data": {
    "id": "cred-uuid",
    "name": "Stripe API Key",
    "type": "api_key",
    "description": "Production Stripe key",
    "created_at": "2026-03-22T10:00:00Z"
  }
}
```

**Errors:**

| Status | Condition |
|--------|-----------|
| 409 | Credential with this name already exists |

---

### GET /credentials

List all credentials (summaries only, no secret data). Returns all credentials for the authenticated user without pagination.

**Permission:** `ManageCredentials`

**Response:** `200 OK`

```json
{
  "data": [
    {
      "id": "cred-uuid",
      "name": "Stripe API Key",
      "type": "api_key",
      "description": "Production Stripe key",
      "created_at": "2026-03-22T10:00:00Z"
    }
  ]
}
```

---

### GET /credentials/{id}

Retrieve a credential summary by ID (no secret data).

**Permission:** `ManageCredentials`

**Path parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | string | Credential ID (UUID) |

**Response:** `200 OK` with credential summary.

**Errors:**

| Status | Condition |
|--------|-----------|
| 404 | Credential not found |

---

### PUT /credentials/{id}

Update a credential (replaces all fields including encrypted data).

**Permission:** `ManageCredentials`

**Request body:** Same shape as `POST /credentials`.

**Response:** `200 OK` with updated credential summary.

**Errors:**

| Status | Condition |
|--------|-----------|
| 404 | Credential not found |

---

### DELETE /credentials/{id}

Delete a credential.

**Permission:** `ManageCredentials`

**Response:** `200 OK`

```json
{
  "data": {
    "status": "deleted"
  }
}
```

**Errors:**

| Status | Condition |
|--------|-----------|
| 404 | Credential not found |

---

## 10. Metrics

> Requires a `MetricsStore` to be configured on the server. Returns `501 Not Implemented` if not configured.

### GET /workflows/{id}/metrics

Retrieve aggregated execution metrics for a workflow over the last 24 hours.

**Permission:** `View` (scoped to workflow)

**Response:** `200 OK`

```json
{
  "data": {
    "total_executions": 156,
    "successful": 142,
    "failed": 14,
    "avg_duration_ms": 3200,
    "p95_duration_ms": 8500
  }
}
```

---

### GET /workflows/{id}/metrics/nodes

Retrieve per-node execution metrics for a workflow over the last 24 hours.

**Permission:** `View` (scoped to workflow)

**Response:** `200 OK`

```json
{
  "data": [
    {
      "node_id": "fetch-order",
      "executions": 156,
      "avg_duration_ms": 450,
      "error_rate": 0.02
    }
  ]
}
```

---

### GET /instances/{id}/metrics

Retrieve metrics for a specific instance execution.

**Permission:** `View`

**Response:** `200 OK` with instance-level metrics.

**Errors:**

| Status | Condition |
|--------|-----------|
| 404 | No metrics found for this instance |

---

## 11. Analytics

> Requires an `AnalyticsStore` to be configured on the server. Returns `503 Service Unavailable` if not configured.

All analytics endpoints accept a `range` query parameter using shorthand notation.

**Range format:** `<number>d` for days or `<number>h` for hours. Examples: `24h`, `7d`, `30d`, `90d`. Default: `7d`. Maximum: `365d` or `8760h`.

### GET /analytics/executions

Retrieve aggregated execution statistics.

**Permission:** `View`

**Query parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `range` | string | `7d` | Time range (e.g., `24h`, `7d`, `30d`) |

**Response:** `200 OK` with execution statistics.

---

### GET /analytics/nodes

Retrieve node performance analytics.

**Permission:** `View`

**Query parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `range` | string | `7d` | Time range |

**Response:** `200 OK` with per-node performance data.

---

### GET /analytics/failures

Retrieve failure trend analytics.

**Permission:** `View`

**Query parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `range` | string | `7d` | Time range |

**Response:** `200 OK` with failure trend data.

---

## 12. Budgets

Manage cost budgets for workflows and teams. Budgets automatically reset based on their configured period.

> Requires a `BudgetStore` to be configured on the server. Returns `501 Not Implemented` if not configured.

### GET /budgets

List all configured budgets.

**Permission:** `Admin`

**Response:** `200 OK`

```json
{
  "data": [
    {
      "id": "budget-uuid",
      "workflow_id": "a1b2c3d4-...",
      "team": null,
      "period": "monthly",
      "limit_usd": 500.00,
      "current_usd": 123.45,
      "reset_at": "2026-04-22T10:00:00Z",
      "created_at": "2026-03-22T10:00:00Z"
    }
  ]
}
```

---

### POST /budgets

Create a new budget.

**Permission:** `Admin`

**Request body:**

```json
{
  "workflow_id": "a1b2c3d4-...",
  "team": null,
  "period": "monthly",
  "limit_usd": 500.00
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `workflow_id` | string | No | Scope budget to a specific workflow |
| `team` | string | No | Scope budget to a team |
| `period` | string | No | Reset period: `daily`, `weekly`, `monthly` (default: `monthly`) |
| `limit_usd` | number | Yes | Budget limit in USD (must be positive) |

**Response:** `201 Created` with the budget object.

**Errors:**

| Status | Condition |
|--------|-----------|
| 400 | `limit_usd` is not positive |

---

### PUT /budgets/{id}

Update an existing budget. Preserves the current spend amount.

**Permission:** `Admin`

**Request body:** Same shape as `POST /budgets`.

**Response:** `200 OK` with updated budget object.

**Errors:**

| Status | Condition |
|--------|-----------|
| 400 | `limit_usd` is not positive |
| 404 | Budget not found |

---

### DELETE /budgets/{id}

Delete a budget.

**Permission:** `Admin`

**Response:** `200 OK`

```json
{
  "data": {
    "deleted": true
  }
}
```

**Errors:**

| Status | Condition |
|--------|-----------|
| 404 | Budget not found |

---

### GET /analytics/costs

Retrieve cost analytics grouped by workflow or team.

**Permission:** `Admin`

**Query parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `range` | string | `30d` | Time range in days (e.g., `30d`) |
| `group_by` | string | `workflow` | Grouping: `workflow` or `team` |

**Response:** `200 OK`

```json
{
  "data": {
    "range_days": 30,
    "since": "2026-02-20T10:00:00Z",
    "group_by": "workflow",
    "budgets": [ ... ]
  }
}
```

---

## 13. Alerts

Manage alert rules that trigger notifications based on workflow execution metrics.

> Requires an `AlertStore` to be configured on the server. Returns `503 Service Unavailable` if not configured.

### GET /alerts

List all alert rules.

**Permission:** `Admin`

**Response:** `200 OK`

```json
{
  "data": [
    {
      "id": "alert-uuid",
      "workflow_id": "a1b2c3d4-...",
      "metric": "failure_rate",
      "operator": "greater_than",
      "threshold": 0.1,
      "channel": {
        "type": "webhook",
        "url": "https://hooks.slack.com/..."
      },
      "enabled": true,
      "created_at": "2026-03-22T10:00:00Z"
    }
  ]
}
```

---

### POST /alerts

Create a new alert rule.

**Permission:** `Admin`

**Request body:**

```json
{
  "workflow_id": "a1b2c3d4-...",
  "metric": "failure_rate",
  "operator": "greater_than",
  "threshold": 0.1,
  "channel": {
    "type": "webhook",
    "url": "https://hooks.slack.com/..."
  },
  "enabled": true
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `workflow_id` | string | No | Scope alert to a specific workflow |
| `metric` | string | Yes | Metric to monitor: `failure_rate`, `p95_duration`, `execution_count` |
| `operator` | string | Yes | Comparison: `greater_than`, `less_than`, `equals` |
| `threshold` | number | Yes | Threshold value for triggering the alert |
| `channel` | object | Yes | Notification channel (see below) |
| `enabled` | boolean | No | Whether the alert is active (default: `true`) |

**Channel types:**

| Type | Fields | Description |
|------|--------|-------------|
| `webhook` | `url` (string) | Send HTTP POST to URL |
| `log` | (none) | Log the alert to server logs |

**Response:** `201 Created` with the alert rule object.

---

### PUT /alerts/{id}

Update an existing alert rule.

**Permission:** `Admin`

**Request body:** Same shape as `POST /alerts`.

**Response:** `200 OK` with updated alert rule.

**Errors:**

| Status | Condition |
|--------|-----------|
| 404 | Alert not found |

---

### DELETE /alerts/{id}

Delete an alert rule.

**Permission:** `Admin`

**Response:** `200 OK`

```json
{
  "data": {
    "deleted": true
  }
}
```

**Errors:**

| Status | Condition |
|--------|-----------|
| 404 | Alert not found |

---

## 14. Audit and Compliance

Orbflow maintains a tamper-evident audit trail for every workflow execution using hash chains and Merkle trees.

### GET /instances/{id}/audit/trail

Retrieve the full audit trail for an instance.

**Permission:** `View`

**Response:** `200 OK`

```json
{
  "data": [
    {
      "event_index": 0,
      "event_type": "InstanceCreated",
      "event_hash": "sha256:abc123...",
      "prev_hash": "sha256:000000...",
      "timestamp": "2026-03-22T10:05:00Z",
      "data": { ... }
    }
  ]
}
```

**Errors:**

| Status | Condition |
|--------|-----------|
| 404 | Instance not found |

---

### GET /instances/{id}/audit/verify

Verify the integrity of an instance's audit chain.

**Permission:** `View`

**Response:** `200 OK`

```json
{
  "data": {
    "valid": true,
    "event_count": 12,
    "error": null
  }
}
```

When the chain is broken:

```json
{
  "data": {
    "valid": false,
    "event_count": 12,
    "error": "hash mismatch at event index 7"
  }
}
```

**Errors:**

| Status | Condition |
|--------|-----------|
| 404 | Instance not found |

---

### GET /instances/{id}/audit/proof/{event_index}

Get a Merkle inclusion proof for a specific event in the audit trail.

**Permission:** `View`

**Path parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | string | Instance ID |
| `event_index` | integer | Zero-based event index |

**Response:** `200 OK`

```json
{
  "data": {
    "event_index": 3,
    "leaf_hash": "sha256:abc123...",
    "merkle_root": "sha256:def456...",
    "proof": [
      { "hash": "sha256:...", "side": "left" },
      { "hash": "sha256:...", "side": "right" }
    ],
    "valid": true
  }
}
```

**Errors:**

| Status | Condition |
|--------|-----------|
| 400 | `event_index` out of range |
| 404 | Instance not found |

---

### GET /instances/{id}/audit/export

Export the audit trail as a compliance-formatted file download.

**Permission:** `View`

**Query parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `format` | string | Yes | Compliance format: `soc2`, `hipaa`, or `pci` |

**Response:** `200 OK` with file download (CSV or similar format).

Response headers include `Content-Disposition: attachment; filename="audit-{id}-{format}.csv"`.

**Errors:**

| Status | Condition |
|--------|-----------|
| 400 | Unsupported compliance format |
| 404 | Instance not found |

---

## 15. RBAC (Role-Based Access Control)

### GET /rbac/subjects

List distinct subjects (user IDs, role names) currently referenced in the RBAC policy. Used by the RBAC editor's auto-suggest feature.

**Permission:** `Admin`

**Response:** `200 OK`

```json
{
  "data": ["alice@example.com", "bob@example.com", "role:engineer", "role:viewer"]
}
```

---

Orbflow supports fine-grained RBAC at three levels of granularity: global, workflow-scoped, and node-scoped. Permissions are resolved from most specific to least specific.

### Permissions

| Permission | Description |
|-----------|-------------|
| `view` | View workflow definitions and execution results |
| `edit` | Modify workflow definitions |
| `execute` | Start workflow executions |
| `approve` | Approve/reject nodes and change requests |
| `delete` | Delete workflows |
| `manage_credentials` | Create, update, and delete credentials |
| `admin` | Manage RBAC policies, budgets, and alerts |

### Policy Scopes

| Scope | Description |
|-------|-------------|
| `global` | Applies to all workflows and nodes |
| `workflow` | Applies to a specific workflow (requires `workflow_id`) |
| `node` | Applies to a specific node within a workflow (requires `workflow_id` and `node_id`) |

### GET /rbac/policy

Retrieve the current RBAC policy. Returns a default empty policy if RBAC is not configured.

**Permission:** `Admin`

**Response:** `200 OK`

```json
{
  "data": {
    "roles": [
      {
        "id": "viewer",
        "name": "Viewer",
        "permissions": ["view"],
        "description": "Read-only access"
      },
      {
        "id": "editor",
        "name": "Editor",
        "permissions": ["view", "edit", "execute"],
        "description": "Can modify and run workflows"
      },
      {
        "id": "admin",
        "name": "Admin",
        "permissions": ["view", "edit", "execute", "approve", "delete", "manage_credentials", "admin"]
      }
    ],
    "bindings": [
      {
        "subject": "alice@example.com",
        "role_id": "admin",
        "scope": { "type": "global" }
      },
      {
        "subject": "bob@example.com",
        "role_id": "editor",
        "scope": {
          "type": "workflow",
          "workflow_id": "a1b2c3d4-..."
        }
      }
    ]
  }
}
```

---

### PUT /rbac/policy

Replace the entire RBAC policy. The new policy must contain at least one admin binding to prevent lockout.

**Permission:** `Admin`

**Request body:** Full `RbacPolicy` object (same shape as the GET response `data`).

**Response:** `200 OK` with the updated policy.

**Errors:**

| Status | Condition |
|--------|-----------|
| 422 | Policy does not contain at least one admin binding |
| 501 | RBAC is not enabled on this server |

---

## 16. Streaming (SSE)

### GET /instances/{instance_id}/nodes/{node_id}/stream

Stream real-time execution output from a node via Server-Sent Events (SSE). The server verifies the instance exists before establishing the stream.

**Permission:** `View` (scoped to node)

**Path parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `instance_id` | string | Instance ID |
| `node_id` | string | Node ID within the workflow |

**SSE event types:**

| Event | Description |
|-------|-------------|
| `data` | Incremental output chunk from the node |
| `done` | Node execution completed successfully |
| `error` | Node execution failed |

**Example SSE stream:**

```
event: data
data: {"instance_id":"inst-...","node_id":"ai-node","chunk":{"type":"data","content":"Hello"}}

event: data
data: {"instance_id":"inst-...","node_id":"ai-node","chunk":{"type":"data","content":" world"}}

event: done
data: {"instance_id":"inst-...","node_id":"ai-node","chunk":{"type":"done","final_output":{...}}}
```

**Errors:**

| Status | Condition |
|--------|-----------|
| 404 | Instance not found |
| 503 | Streaming not available (no message bus configured) |

---

## 17. Marketplace

### GET /marketplace/plugins

List installed plugins from the local plugin index with optional filtering and sorting.

**Permission:** `View`

**Query parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `q` | string | (none) | Search query (matches name, description, tags) |
| `category` | string | (none) | Filter by category |
| `sort` | string | `name` | Sort field |
| `order` | string | `asc` | Sort order: `asc` or `desc` |
| `installed_only` | boolean | `false` | Only show installed plugins |
| `offset` | integer | `0` | Pagination offset |
| `limit` | integer | `20` | Items per page (max 100) |

**Response:** `200 OK`

```json
{
  "data": [
    {
      "name": "orbflow-slack",
      "version": "1.2.0",
      "description": "Slack integration for Orbflow",
      "author": "Orbflow Team",
      "node_types": ["slack_message", "slack_channel"],
      "tags": ["messaging", "notification"],
      "icon": "slack-icon.png",
      "category": "communication",
      "installed": true,
      "downloads": 1234,
      "color": "#4A154B"
    }
  ],
  "meta": {
    "total": 15,
    "offset": 0,
    "limit": 20
  }
}
```

---

### GET /marketplace/plugins/{name}

Get details of a specific installed plugin.

**Permission:** `View`

**Path parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `name` | string | Plugin name (alphanumeric, hyphens, underscores only) |

**Response:** `200 OK` with full plugin manifest including `license`, `repository`, `orbflow_version`, `protocol`, `readme`, and other metadata.

**Errors:**

| Status | Condition |
|--------|-----------|
| 400 | Invalid plugin name |
| 404 | Plugin not found |

---

## 18. Plugin Lifecycle

Manage plugin processes (start, stop, restart, reload). All endpoints require `Admin` permission.

### GET /plugins/status

List all plugin statuses (running, stopped, available).

**Permission:** `View`

**Response:** `200 OK` with array of plugin status objects.

---

### GET /plugins/{name}/status

Get a single plugin's status.

**Permission:** `View`

**Response:** `200 OK` with plugin status object.

**Errors:**

| Status | Condition |
|--------|-----------|
| 404 | Plugin not found |

---

### POST /plugins/{name}/start

Start a plugin process. Returns immediately; the plugin starts in the background with a health check.

**Permission:** `Admin`

**Response:** `202 Accepted`

---

### POST /plugins/{name}/stop

Stop a running plugin process.

**Permission:** `Admin`

**Response:** `200 OK`

---

### POST /plugins/{name}/restart

Restart a plugin (stop + start in background).

**Permission:** `Admin`

**Response:** `202 Accepted`

---

### POST /plugins/reload

Stop all plugins, re-scan the plugins directory, and respawn.

**Permission:** `Admin`

**Response:** `202 Accepted`

---

### POST /marketplace/plugins/{name}/install

Install a community plugin from the marketplace.

**Permission:** `Admin`

**Response:** `200 OK` with install status. Triggers a plugin reload.

**Errors:**

| Status | Condition |
|--------|-----------|
| 404 | Plugin not found in marketplace |
| 409 | Plugin already installed |

---

### DELETE /marketplace/plugins/{name}

Uninstall a plugin (removes directory, stops process, unregisters schemas).

**Permission:** `Admin`

**Response:** `200 OK`

**Errors:**

| Status | Condition |
|--------|-----------|
| 404 | Plugin not found |

---

### POST /marketplace/validate-manifest

Validate a plugin manifest JSON without installing. Used by the plugin submission wizard.

**Permission:** `View`

**Request body:** Plugin manifest JSON object.

**Response:** `200 OK`

```json
{
  "data": {
    "valid": true,
    "errors": []
  }
}
```

---

### Metadata

### GET /node-types

List all available node type schemas (built-in and plugin-provided).

**Permission:** None (public)

**Response:** `200 OK` with array of node schema definitions.

---

### GET /credential-types

List all available credential type schemas.

**Permission:** None (public)

**Response:** `200 OK` with array of credential type schema definitions.

---

## 20. Pagination

All list endpoints support offset-based pagination via query parameters.

| Parameter | Type | Default | Max | Description |
|-----------|------|---------|-----|-------------|
| `offset` | integer | `0` | -- | Number of items to skip |
| `limit` | integer | `20` | `100` | Number of items per page |

If `limit` is less than or equal to 0, it defaults to 20. If `limit` exceeds 100, it is clamped to 100.

**Example:** Fetch the second page of 10 items:

```
GET /api/v1/workflows?offset=10&limit=10
```

**Response metadata:**

```json
{
  "data": [ ... ],
  "meta": {
    "total": 42,
    "offset": 10,
    "limit": 10
  }
}
```

To iterate through all pages:

```
Page 1: GET /api/v1/workflows?offset=0&limit=20
Page 2: GET /api/v1/workflows?offset=20&limit=20
Page 3: GET /api/v1/workflows?offset=40&limit=20
...continue until offset >= meta.total
```

---

## 21. Error Codes

| HTTP Status | Meaning | Common Causes |
|-------------|---------|---------------|
| 400 | Bad Request | Validation error, cycle detected, invalid node config, malformed input |
| 401 | Unauthorized | Missing or invalid bearer token |
| 403 | Forbidden | Insufficient RBAC permissions |
| 404 | Not Found | Resource does not exist |
| 409 | Conflict | Version conflict (optimistic locking), resource already exists |
| 410 | Gone | Instance was cancelled |
| 422 | Unprocessable Entity | Invalid status transition, missing admin binding in RBAC policy |
| 429 | Too Many Requests | Workflow start rate limited (2-second cooldown per workflow), budget exceeded |
| 500 | Internal Server Error | Unexpected server-side failure |
| 501 | Not Implemented | Feature not enabled on this server (e.g., RBAC, metrics store) |
| 503 | Service Unavailable | Required subsystem not configured (e.g., streaming bus, analytics store) |
| 504 | Gateway Timeout | Operation timed out (e.g., test suite exceeded 5-minute limit) |

All error responses follow the standard envelope:

```json
{
  "data": null,
  "error": "human-readable error message"
}
```

---

## 22. Rate Limiting

The API enforces three tiers of per-user rate limiting:

| Tier | Applies to | Description |
|------|-----------|-------------|
| **Read** | GET endpoints | Standard read rate limit |
| **Write** | POST, PUT, DELETE endpoints | Write operation rate limit |
| **Sensitive** | RBAC policy updates, plugin management, alerts, budgets | Stricter limit for admin operations |

Rate limits are configured via `RateLimitConfig` in the server configuration.

Additionally, `/workflows/{id}/start` and `/workflows/{id}/test-node` enforce a per-workflow cooldown of 2 seconds. If a workflow is started again within this window, the server responds with HTTP 429:

```json
{
  "error": "rate limit: workflow was started recently, please wait"
}
```

---

## 23. CORS

CORS behavior is controlled by the `server.cors_origins` configuration:

| Configuration | Behavior |
|--------------|----------|
| `cors_origins: ["*"]` | Allows all origins (development only -- logged as warning) |
| `cors_origins: ["https://app.example.com"]` | Only the listed origins are allowed (recommended for production) |
| `cors_origins: []` (or omitted) | Denies all cross-origin requests (safe default) |

In all modes, only the following methods and headers are allowed:

- **Methods:** GET, POST, PUT, DELETE, OPTIONS
- **Headers:** Authorization, Content-Type
