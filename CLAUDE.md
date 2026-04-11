# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Development Commands

Run `just` or `just --list` to see all targets with descriptions. Key commands below.

### Quick Start

```bash
just setup                                # First-time: check tools, install deps, start infra, verify workspace
just dev                                  # Start everything (server + worker + web) with live reload
just dev-backend                          # Start server + worker only (no frontend)
```

### Build

```bash
just build                                # Build release binaries → target/release/orbflow-server, orbflow-worker-bin
just build-debug                          # Build debug binaries (fast compile)
just build-web                            # Build Next.js 16 frontend for production
just check                                # Type-check Rust workspace (no codegen)
```

### Test

```bash
just test                                 # Run all Rust tests
just test-crate orbflow-core                 # Run tests for a single crate (with --nocapture)
just test-verbose                         # Run all Rust tests with stdout visible
just test-web                             # Run frontend tests
just test-all                             # Run all tests (Rust + frontend)
```

### Lint & Format

```bash
just lint                                 # cargo clippy --workspace -- -D warnings
just fmt                                  # Format all Rust code
just fmt-check                            # Check formatting (CI-friendly, no changes)
just lint-all                             # All linters: clippy + fmt-check + frontend lint
```

### Quality Gate

```bash
just ci                                   # Full CI pipeline: format + lint + test + build
just pre-commit                           # Quick pre-commit: format + lint + test
```

### Infrastructure

```bash
just infra                                # Start Postgres + NATS (waits until healthy)
just infra-down                           # Stop infrastructure containers
just infra-reset                          # Destroy + recreate infra (wipes all data, 3s safety delay)
just infra-logs                           # Tail Postgres + NATS logs
just infra-status                         # Show container status
```

### Database

```bash
just db-shell                             # Open psql shell to local Postgres
just db-status                            # Check connection and table count
```

### Docker (Production)

```bash
just docker-build                         # Build all Docker images
just docker-up                            # Start all services in Docker
just docker-down                          # Stop all Docker services
just docker-restart                       # Restart all Docker services
just docker-logs                          # Tail all service logs
```

### Debugging

```bash
just debug-server                         # Run server with RUST_LOG=debug
just debug-worker                         # Run worker with RUST_LOG=debug
just env-check                            # Verify all required tools and env vars
just tree                                 # Show crate dependency graph
```

### Cleanup

```bash
just clean                                # Remove Rust build artifacts
just clean-all                            # Remove all artifacts (Rust + node_modules + .next)
just clean-docker                         # Remove Docker volumes (wipes Postgres + NATS data)
```

### Configuration

Three config files in `configs/`:
- `orbflow.yaml` — production defaults
- `orbflow.dev.yaml` — local development (used by `just dev*` targets)
- `orbflow.docker.yaml` — Docker Compose services

Backend API URL defaults to `http://localhost:8080` (configurable via `NEXT_PUBLIC_API_URL`).

### Windows Note

The `.cargo/config.toml` disables PDB generation (`/DEBUG:NONE`) to avoid the MSVC linker LNK1318 error on large test binaries. `Cargo.toml` also sets `debug = false` for dev/test profiles.

## Architecture

Orbflow is a distributed workflow automation engine (Rust edition 2024, AGPL-3.0-or-later). Run `orbflow-server` (coordinator + HTTP/gRPC API) and `orbflow-worker` (task executor) as separate processes, backed by PostgreSQL and NATS JetStream. gRPC service definitions live in `proto/`.

### Rust Backend — Ports & Adapters

`orbflow-core` defines all domain types and port traits. Every other crate implements one adapter. Dependencies point inward — only `orbflow-core` is imported across crate boundaries.

**`orbflow-core` domain modules** (beyond ports): `workflow`, `execution`, `event` (event sourcing), `edge` (DAG edges), `wire` (bus message types), `credential`/`credential_proxy`, `schema` (node field schemas), `validate` (workflow validation), `subjects` (NATS subject helpers), `pagination`, `options` (engine builder), `rbac`, `audit`, `alerts`, `metering`, `analytics`, `compliance`, `streaming`, `prediction`, `versioning`, `telemetry`/`otel`/`metrics`, `crypto`, `testing`.

**Port traits** (in `orbflow-core::ports`):
- `Engine` — orchestrate workflows: create/start/cancel, register node executors
- `Store` = `WorkflowStore` + `InstanceStore` + `EventStore` — persistence
- `Bus` — publish/subscribe message transport between coordinator and workers
- `NodeExecutor` — execute a single node: `async fn execute(&self, input: &NodeInput) -> Result<NodeOutput, OrbflowError>`
- `CredentialStore` — encrypted credential management (AES-256-GCM)
- `MetricsStore`, `AnalyticsStore` — usage metering and workflow analytics
- `RbacStore` — role-based access control
- `BudgetStore` — execution budget tracking
- `AlertStore` — alerting rules and notifications
- `ChangeRequestStore` — approval workflows for changes

**Adapter crates**:

| Crate | Implements | Purpose |
|-------|-----------|---------|
| `orbflow-engine` | `Engine` | DAG coordinator, CEL evaluation, saga compensation, crash recovery |
| `orbflow-postgres` | `Store` | PostgreSQL persistence with event sourcing + snapshots |
| `orbflow-memstore` | `Store` | In-memory store for testing |
| `orbflow-natsbus` | `Bus` | NATS JetStream message transport |
| `orbflow-httpapi` | — | Axum REST API with CORS, rate limiting, 1MB body limit |
| `orbflow-grpcapi` | — | gRPC API surface (JSON codec over TCP) |
| `orbflow-worker` | — | Task executor: subscribes to bus, routes to `NodeExecutor` impls |
| `orbflow-builtins` | `NodeExecutor` | Built-in nodes: HTTP, email, transform, filter, delay, sort, encode, template, log, MCP tool, AI nodes (chat, classify, extract, sentiment, summarize, translate) |
| `orbflow-trigger` | — | Cron scheduler + webhook + event trigger system |
| `orbflow-plugin` | — | External plugin loader via JSON-RPC subprocess protocol |
| `orbflow-cel` | — | CEL expression evaluator with program cache |
| `orbflow-config` | — | YAML config loading with env var expansion + tracing setup |
| `orbflow-mcp` | — | MCP (Model Context Protocol) client: schema, transport, tool invocation |
| `orbflow-registry` | — | Node/plugin registry: manifest parsing, index, remote client |
| `orbflow-testutil` | — | MockBus, MockStore, MockNodeExecutor for testing |
| `orbflow-test` | — | Integration test harness: runner, assertions, test types |
| `orbflow-server` | — | Server binary: wires Postgres + NATS + engine + HTTP + gRPC |
| `orbflow-worker-bin` | — | Worker binary: wires NATS + builtins + plugins |

### Execution Data Flow

1. `Engine::start_workflow()` creates an `Instance`, evaluates the DAG, dispatches ready nodes
2. Node tasks published to `Bus` as `TaskMessage` (defined in `orbflow-core::wire`)
3. `Worker` receives tasks, calls the matching `NodeExecutor::execute()`
4. Results published back as `ResultMessage`
5. Engine processes results, advances DAG, dispatches next ready nodes
6. Bus subjects: `task_subject(pool)` / `result_subject(pool)` (in `orbflow-core::subjects`)

### Builtin Node Convention

All builtin executors in `orbflow-builtins` follow the same pattern:
```rust
#[async_trait]
impl NodeExecutor for MyNode {
    async fn execute(&self, input: &NodeInput) -> Result<NodeOutput, OrbflowError> {
        let cfg = resolve_config(input);
        // ... validate, execute, return
        Ok(NodeOutput { data: Some(result), error: None })
    }
}

impl NodeSchemaProvider for MyNode {
    fn node_schema(&self) -> NodeSchema { /* field definitions */ }
}
```

AI builtin nodes (`ai_chat`, `ai_classify`, `ai_extract`, `ai_sentiment`, `ai_summarize`, `ai_translate`) share common infrastructure via `ai_common.rs`. The `ssrf.rs` module provides SSRF protection for the HTTP node. Builtins are wired via `register.rs`.

### Error Handling

`OrbflowError` enum in `orbflow-core::error`. Key variants: `NotFound`, `AlreadyExists`, `Conflict`, `CycleDetected`, `InvalidNodeConfig(String)`. The `is_validation_error()` method identifies client-side mistakes (mapped to HTTP 400 / gRPC InvalidArgument).

### HTTP API Response Envelope

All responses use: `{ "data": T, "error"?: string, "meta"?: { total, offset, limit } }`. Error responses include `"data": null`.

### Frontend — Monorepo Structure

**Stack**: Next.js 16 (Turbopack), React 19, TypeScript, TailwindCSS 4, Zustand, @xyflow/react

```
apps/web/           → Next.js 16 app (Turbopack, visual builder + execution monitor)
packages/orbflow-core/ → Headless SDK (stores, types, hooks, schemas — zero CSS)
```

**`packages/orbflow-core`** exports Zustand stores, types, and utilities consumed by `apps/web`. Key exports: `canvasStore`, `workflowStore`, `executionOverlayStore`, `credentialStore`, `alertStore`, `budgetStore`, `changeRequestStore`, `nodeOutputCacheStore`, `historyStore`, `panelStore`, `pickerStore`, `toastStore`, `NodeSchemaRegistry`, `createApiClient()`.

**`apps/web`** has two layers:
- `src/core/` — Embeddable workflow builder components (canvas, config modal, toolbar)
- `src/components/` — App-specific features (workflow-builder, execution-viewer, credential-manager)

### Key Patterns

- **CEL expressions**: All dynamic values use CEL (Common Expression Language). Values prefixed with `=` in the frontend are CEL expressions evaluated by the engine via `orbflow-cel`.
- **Event sourcing**: Instance state changes are persisted as `DomainEvent` variants with periodic snapshots for crash recovery.
- **Builder pattern**: `EngineOptionsBuilder` replaces Go functional options for engine/worker construction.
- **Immutable domain objects**: Engine creates new Instance copies rather than mutating in place.
- **Per-instance locking**: `DashMap<InstanceId, Arc<Mutex<()>>>` for concurrent result handling with optimistic locking retry (max 3 attempts).
- **Wire compatibility**: JSON field names use snake_case matching the frontend's `api.ts` types.
