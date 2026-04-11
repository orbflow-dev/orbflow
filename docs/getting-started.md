# Getting Started with Orbflow

## Prerequisites

| Tool       | Version | Purpose                       |
|------------|---------|-----------------------------|
| Rust       | 1.85+   | Compiles the backend binaries |
| Node.js    | 20+     | Builds and runs the frontend  |
| pnpm       | 10+     | Frontend package manager      |
| Docker     | Latest  | PostgreSQL + NATS containers  |
| just       | any     | Task runner (`cargo install just`) |

Verify your toolchain:

```bash
rustc --version     # 1.85.0 or later
node --version      # v20.x or later
pnpm --version      # 10.x or later
docker --version    # 24.x or later
just --version
```

Or run `just env-check` to verify everything at once.

---

## Quick Start

### 1. First-Time Setup

```bash
git clone https://github.com/orbflow-dev/orbflow.git
cd orbflow

# Check tools, install frontend deps, start Postgres + NATS, create dev DB, verify workspace
just setup
```

### 2. Start Development

```bash
# Start server + worker + frontend with live reload
just dev
```

This starts:
- **Server** at `http://localhost:8080` (coordinator + HTTP API)
- **Worker** (task executor, connects via NATS)
- **Frontend** at `http://localhost:3000` (visual workflow builder)

### 3. Verify the Server is Running

```bash
curl http://localhost:8080/health
```

You should see `{"status":"ok"}`. If you get a connection error, wait a few seconds for the server to finish starting.

### 4. Create a Workflow via API

```bash
curl -X POST http://localhost:8080/api/v1/workflows \
  -H "Content-Type: application/json" \
  -d '{
    "name": "hello-world",
    "nodes": [
      {"id": "start", "type": "builtin:log", "config": {"message": "Hello Orbflow!"}},
      {"id": "end", "type": "builtin:log", "config": {"message": "Done."}}
    ],
    "edges": [
      {"source": "start", "target": "end"}
    ]
  }'
```

### 5. Start the Workflow

```bash
curl -X POST http://localhost:8080/api/v1/workflows/<id>/start \
  -H "Content-Type: application/json" \
  -d '{"message": "hello orbflow"}'
```

The JSON body becomes the `vars` object available in CEL expressions (e.g., `vars.message`).

### 6. Check the Result

```bash
curl http://localhost:8080/api/v1/instances | jq '.data[0].status'
```

Or open the frontend at `http://localhost:3000` to see the execution in the visual builder.

---

## Standalone Mode (Production)

Build optimized release binaries and run them directly:

```bash
# Build
just build

# Start infrastructure
just infra

# Set required env vars
export CREDENTIAL_ENCRYPTION_KEY=$(openssl rand -hex 32)
export AUTH_TOKEN=$(openssl rand -hex 32)

# Run server (in one terminal)
./target/release/orbflow-server configs/orbflow.yaml

# Run worker (in another terminal)
./target/release/orbflow-worker-bin configs/orbflow.yaml
```

> **Note:** Use `configs/orbflow.dev.yaml` for local development instead. See [DEPLOYMENT.md](DEPLOYMENT.md) for full production configuration, Docker Compose, and Kubernetes deployment.

---

## CEL Expressions

All dynamic values use [CEL](https://cel.dev/) (Common Expression Language) -- a safe, non-Turing-complete language with guaranteed termination.

### Edge Conditions

```json
{
  "condition": "vars.amount > 100 && nodes.check.result == true"
}
```

### Available Variables

- `vars` -- workflow input variables
- `nodes` -- map of completed node outputs (e.g., `nodes.step1.result`)
- `node` -- output of the source node (for edge conditions)
- `trigger` -- trigger metadata

### Input Mapping

Prefix values with `=` to evaluate them as CEL expressions:

```json
{
  "input_mapping": {
    "url": "=nodes.config.api_url",
    "amount": "=vars.order_total * 1.1"
  }
}
```

---

## Triggers

Workflows can be triggered by:

- **Manual**: via API call (`POST /api/v1/workflows/{id}/start`)
- **Cron**: `{"type": "schedule", "config": {"cron": "0 */5 * * * *"}}`
- **Event**: `{"type": "event", "config": {"event_name": "order.created"}}`
- **Webhook**: `{"type": "webhook", "config": {"path": "my-hook"}}`

---

## Frontend

The visual workflow builder is a Next.js 16 app with a drag-and-drop canvas for building DAGs, configuring nodes, and monitoring executions in real-time.

```bash
# Start frontend only (requires backend running)
pnpm --filter orbflow-web dev
```

Or use `just dev` to start everything together.

---

## Next Steps

- [Architecture](ARCHITECTURE.md) -- Crate layout, execution data flow, key patterns
- [API Reference](API.md) -- Full REST API documentation
- [Deployment](DEPLOYMENT.md) -- Configuration, Docker, Kubernetes, RBAC, observability
