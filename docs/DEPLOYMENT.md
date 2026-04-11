# Orbflow Deployment and Configuration Guide

This guide covers building, configuring, and deploying the Orbflow distributed workflow automation engine in development and production environments.

---

## Table of Contents

1. [Prerequisites](#1-prerequisites)
2. [Building from Source](#2-building-from-source)
3. [Configuration Reference](#3-configuration-reference)
4. [Environment Variables](#4-environment-variables)
5. [Docker Compose (Development)](#5-docker-compose-development)
6. [Docker Compose (Production)](#6-docker-compose-production)
7. [Kubernetes](#7-kubernetes)
8. [RBAC Setup](#8-rbac-setup)
9. [Observability Setup](#9-observability-setup)
10. [Security Checklist](#10-security-checklist)
11. [Backup and Recovery](#11-backup-and-recovery)
12. [Troubleshooting](#12-troubleshooting)

---

## 1. Prerequisites

| Dependency       | Minimum Version | Purpose                          |
|------------------|-----------------|----------------------------------|
| Rust (rustc)     | 1.85+           | Compiles the backend binaries    |
| Cargo            | (bundled)       | Rust build system and package manager |
| Node.js          | 20+             | Builds and runs the frontend     |
| pnpm             | 10+             | Frontend package manager (monorepo) |
| PostgreSQL       | 16+             | Persistent storage (event sourcing) |
| NATS             | 2.10+           | Message transport (JetStream required) |
| Docker (optional)| 24+             | Containerized deployment         |

Verify your toolchain:

```bash
rustc --version    # 1.85.0 or later
cargo --version
node --version     # v20.x or later
pnpm --version     # 10.x or later
docker --version   # optional
```

Alternatively, run `just env-check` from the project root to verify all tools at once.

---

## 2. Building from Source

### Backend (Rust)

```bash
# Release build (optimized)
just build
# Outputs:
#   target/release/orbflow-server
#   target/release/orbflow-worker-bin
```

For a faster debug build (unoptimized):

```bash
just build-debug
```

### Frontend (Next.js)

```bash
# Install dependencies
pnpm install

# Production build (standalone output)
just build-web
# Outputs: apps/web/.next/standalone/
```

### Full CI Pipeline

Run the complete quality gate (format check, lint, test, build):

```bash
just ci
```

---

## 3. Configuration Reference

Orbflow loads configuration from a YAML file passed as the first argument to the binary:

```bash
./target/release/orbflow-server configs/orbflow.yaml
./target/release/orbflow-worker-bin configs/orbflow.yaml
```

Environment variables are expanded in YAML values using `${VAR}` or `$VAR` syntax. Unknown variables resolve to empty strings.

### Full Annotated Configuration

```yaml
# ── HTTP API Server ─────────────────────────────────────────
server:
  host: "0.0.0.0"              # Bind address (default: "0.0.0.0")
  port: 8080                    # HTTP port (default: 8080, must be non-zero)
  auth_token: "${AUTH_TOKEN}"   # Bearer token for API auth (default: none/disabled)
                                # When set, all routes except /health, /node-types,
                                # /credential-types, and /webhooks/* require
                                # Authorization: Bearer <token>

# ── gRPC API Server ─────────────────────────────────────────
grpc:
  enabled: false                # Enable gRPC API surface (default: false)
  port: 9090                    # gRPC port (default: 9090, must be non-zero if enabled)

# ── Worker Process ──────────────────────────────────────────
worker:
  pool: "default"               # Worker pool name for task routing (default: "default")
  concurrency: 4                # Max concurrent task executions (default: 4)

# ── PostgreSQL ──────────────────────────────────────────────
database:
  dsn: "postgres://orbflow:orbflow@localhost:5432/orbflow?sslmode=disable"
                                # PostgreSQL connection string (default: "")

# ── NATS ────────────────────────────────────────────────────
nats:
  url: "nats://127.0.0.1:4222" # NATS server URL (default: "nats://127.0.0.1:4222")
  embedded: true                # Use embedded NATS server (default: true)
                                # Set to false in production with external NATS
  data_dir: "/tmp/orbflow-nats"    # JetStream storage directory (default: "/tmp/orbflow-nats")
                                # Only used when embedded is true

# ── Plugins ─────────────────────────────────────────────────
plugins:
  dir: "./plugins"              # Directory for legacy subprocess plugins (default: "./plugins")
  grpc:                         # gRPC plugin endpoints (persistent connections)
    - name: "sentiment"         # Human-readable plugin name
      address: "http://localhost:50051"  # gRPC endpoint address
      timeout_secs: 30          # RPC timeout in seconds (default: 30)

# ── Credentials ────────────────────────────────────────────
credentials:
  encryption_key: "${CREDENTIAL_ENCRYPTION_KEY}"
                                # AES-256-GCM key for encrypting stored credentials
                                # (default: "", required if using credential features)

# ── MCP Server ──────────────────────────────────────────────
mcp:
  enabled: false                # Enable MCP server (default: false)
  transport: "http"             # Transport protocol: "http" (default: "http")
  port: 3001                    # MCP HTTP port (default: 3001)

# ── Logging ─────────────────────────────────────────────────
log:
  level: "info"                 # Log level: trace, debug, info, warn, error
                                # (default: "info")
  format: "json"                # Output format: "json" or "console"
                                # (default: "json")

# ── OpenTelemetry ───────────────────────────────────────────
otel:
  enabled: false                # Enable OTLP export (default: false)
  endpoint: "http://localhost:4317"  # OTLP gRPC endpoint (default: "http://localhost:4317")
  service_name: "orbflow"          # Service name in traces (default: "orbflow")
  sample_rate: 1.0              # Trace sampling rate, 0.0-1.0 (default: 1.0)
```

---

## 4. Environment Variables

| Variable | Description | Default | Required |
|----------|-------------|---------|----------|
| `CREDENTIAL_ENCRYPTION_KEY` | AES-256-GCM encryption key for credential storage | (empty) | Yes, if using credentials |
| `ORBFLOW_BOOTSTRAP_ADMIN` | User ID that always has admin access (RBAC safety net). Cannot be `"anonymous"`. | (none) | No |
| `NEXT_PUBLIC_API_URL` | Backend API URL for the frontend | `http://localhost:8080` | No |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | OpenTelemetry collector endpoint (standard OTel env var) | (none) | No, only for tracing |
| `AUTH_TOKEN` | Bearer token for HTTP API authentication (referenced in YAML as `${AUTH_TOKEN}`) | (none) | Yes, in production |
| `DATABASE_URL` | PostgreSQL DSN (referenced in Docker YAML as `${DATABASE_URL}`) | (none) | Yes |
| `NATS_URL` | NATS server URL (referenced in Docker YAML as `${NATS_URL}`) | `nats://127.0.0.1:4222` | Yes, when `embedded: false` |
| `LOG_LEVEL` | Log level override (referenced in YAML as `${LOG_LEVEL}`) | `info` | No |
| `RUST_LOG` | Standard Rust logging filter (overrides config when set) | (none) | No |
| `NODE_ENV` | Node.js environment for the frontend | `production` | No |
| `NEXT_TELEMETRY_DISABLED` | Disable Next.js telemetry | `1` (in Docker) | No |

---

## 5. Docker Compose (Development)

The project includes a `docker-compose.yml` for local infrastructure. It starts PostgreSQL and NATS only -- you run the Rust binaries and frontend natively for fast iteration.

### Services

| Service    | Image              | Port(s)                       | Purpose |
|------------|--------------------|-------------------------------|---------|
| `postgres` | `postgres:16-alpine` | `127.0.0.1:5432` (host-only) | Persistent storage |
| `nats`     | `nats:2.11-alpine`   | `127.0.0.1:4222` (client), `127.0.0.1:8222` (monitoring) | JetStream message bus |

### Volumes

| Volume     | Mount Point                    | Purpose |
|------------|--------------------------------|---------|
| `pgdata`   | `/var/lib/postgresql/data`     | PostgreSQL data persistence |
| `natsdata` | `/data`                        | NATS JetStream storage |

### Quick Start

```bash
# Start infrastructure (waits for health checks)
just infra

# Start the full dev stack (server + worker + frontend with live reload)
just dev

# Or start backend only
just dev-backend

# Check infrastructure status
just infra-status

# View infrastructure logs
just infra-logs

# Stop infrastructure
just infra-down

# Reset infrastructure (destroys all data)
just infra-reset
```

### Default Credentials

| Service    | User   | Password | Database |
|------------|--------|----------|----------|
| PostgreSQL | `orbflow` | `orbflow`   | `orbflow`   |

Access the database shell:

```bash
just db-shell
```

---

## 6. Docker Compose (Production)

Below is a production-ready Docker Compose template with replicated services. This file does not exist in the repository -- save it as `docker-compose.prod.yml` and customize for your environment:

```yaml
services:
  # ── PostgreSQL ──────────────────────────────────────────
  postgres:
    image: postgres:16-alpine
    restart: always
    environment:
      POSTGRES_USER: orbflow
      POSTGRES_PASSWORD: "${POSTGRES_PASSWORD}"   # Set in .env
      POSTGRES_DB: orbflow
    ports:
      - "127.0.0.1:5432:5432"
    volumes:
      - pgdata:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U orbflow"]
      interval: 5s
      timeout: 3s
      retries: 10
    deploy:
      resources:
        limits:
          memory: 1G

  # ── NATS with JetStream ────────────────────────────────
  nats:
    image: nats:2.11-alpine
    restart: always
    command: ["--jetstream", "--store_dir", "/data", "-m", "8222"]
    ports:
      - "127.0.0.1:4222:4222"
      - "127.0.0.1:8222:8222"
    volumes:
      - natsdata:/data
    healthcheck:
      test: ["CMD", "sh", "-c", "wget -qO- http://localhost:8222/healthz || exit 1"]
      interval: 5s
      timeout: 3s
      retries: 10
      start_period: 5s

  # ── Orbflow Server (2 replicas) ───────────────────────────
  server:
    image: orbflow-server:latest
    restart: always
    depends_on:
      postgres:
        condition: service_healthy
      nats:
        condition: service_healthy
    environment:
      DATABASE_URL: "postgres://orbflow:${POSTGRES_PASSWORD}@postgres:5432/orbflow?sslmode=disable"
      NATS_URL: "nats://nats:4222"
      AUTH_TOKEN: "${AUTH_TOKEN}"
      CREDENTIAL_ENCRYPTION_KEY: "${CREDENTIAL_ENCRYPTION_KEY}"
      LOG_LEVEL: "info"
      ORBFLOW_BOOTSTRAP_ADMIN: "${ORBFLOW_BOOTSTRAP_ADMIN}"
    volumes:
      - ./configs/orbflow.docker.yaml:/etc/orbflow/orbflow.yaml:ro
    command: ["/etc/orbflow/orbflow.yaml"]
    # Note: With replicas > 1, remove explicit port mappings and place a
    # reverse proxy (nginx, Traefik, or cloud LB) in front of the servers.
    # The port mapping below works only with replicas: 1.
    ports:
      - "127.0.0.1:8080:8080"
      - "127.0.0.1:9090:9090"
    deploy:
      replicas: 2
      resources:
        limits:
          memory: 512M

  # ── Orbflow Worker (2 replicas) ───────────────────────────
  # Workers are stateless: they communicate via NATS only and do not need
  # direct database access. Scale horizontally based on queue depth.
  worker:
    image: orbflow-worker:latest
    restart: always
    depends_on:
      nats:
        condition: service_healthy
    environment:
      NATS_URL: "nats://nats:4222"
      CREDENTIAL_ENCRYPTION_KEY: "${CREDENTIAL_ENCRYPTION_KEY}"
      LOG_LEVEL: "info"
    volumes:
      - ./configs/orbflow.docker.yaml:/etc/orbflow/orbflow.yaml:ro
      - ./plugins:/plugins:ro
    command: ["/etc/orbflow/orbflow.yaml"]
    deploy:
      replicas: 2
      resources:
        limits:
          memory: 512M

  # ── Frontend ───────────────────────────────────────────
  web:
    build:
      context: .
      dockerfile: Dockerfile
    restart: always
    depends_on:
      - server
    environment:
      NEXT_PUBLIC_API_URL: "http://server:8080"
      NODE_ENV: production
      NEXT_TELEMETRY_DISABLED: "1"
    ports:
      - "3000:3000"
    deploy:
      resources:
        limits:
          memory: 256M

volumes:
  pgdata:
    driver: local
  natsdata:
    driver: local
```

### Production .env File

Create a `.env` file alongside the compose file (never commit this to version control):

```bash
POSTGRES_PASSWORD=<strong-random-password>
AUTH_TOKEN=<strong-random-token>
CREDENTIAL_ENCRYPTION_KEY=<32-byte-hex-or-base64-key>
ORBFLOW_BOOTSTRAP_ADMIN=admin@example.com
```

### Running in Production

```bash
# Build images
docker compose -f docker-compose.prod.yml build

# Start all services
docker compose -f docker-compose.prod.yml up -d

# Check status
docker compose -f docker-compose.prod.yml ps

# View logs
docker compose -f docker-compose.prod.yml logs -f server worker

# Scale workers
docker compose -f docker-compose.prod.yml up -d --scale worker=4

# Stop
docker compose -f docker-compose.prod.yml down
```

---

## 7. Kubernetes

Orbflow components map naturally to Kubernetes resource types. Below is guidance for each service.

### Server (Deployment -- stateless)

- Run as a **Deployment** with 2+ replicas behind a **Service** (ClusterIP or LoadBalancer).
- Liveness probe: `GET /health` on port 8080.
- Mount the YAML config via a **ConfigMap** at `/etc/orbflow/orbflow.yaml`.
- Inject secrets (`AUTH_TOKEN`, `CREDENTIAL_ENCRYPTION_KEY`, `DATABASE_URL`) via a **Secret** resource referenced as environment variables.

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: orbflow-server
spec:
  replicas: 2
  selector:
    matchLabels:
      app: orbflow-server
  template:
    metadata:
      labels:
        app: orbflow-server
    spec:
      containers:
        - name: server
          image: orbflow-server:latest
          args: ["/etc/orbflow/orbflow.yaml"]
          ports:
            - containerPort: 8080
            - containerPort: 9090
          livenessProbe:
            httpGet:
              path: /health
              port: 8080
            initialDelaySeconds: 10
            periodSeconds: 15
          readinessProbe:
            httpGet:
              path: /health
              port: 8080
            initialDelaySeconds: 5
            periodSeconds: 5
          envFrom:
            - secretRef:
                name: orbflow-secrets
          volumeMounts:
            - name: config
              mountPath: /etc/orbflow
      volumes:
        - name: config
          configMap:
            name: orbflow-config
```

### Worker (Deployment -- stateless, horizontally scalable)

- Run as a **Deployment**. Scale replicas based on queue depth.
- No ingress required -- workers communicate via NATS only.
- Mount the same config and secrets as the server.
- Consider a **HorizontalPodAutoscaler** based on CPU or custom NATS queue-depth metrics.

### PostgreSQL

- Use a managed service (AWS RDS, GCP Cloud SQL, Azure Database) for production.
- If self-hosted, deploy as a **StatefulSet** with persistent volume claims.
- Enable SSL for connections (`sslmode=require` in the DSN).

### NATS

- Use the official [NATS Helm chart](https://github.com/nats-io/k8s) or a managed NATS service.
- Deploy as a **StatefulSet** with JetStream storage on persistent volumes.
- For high availability, run a 3-node NATS cluster.

### Frontend

- Run as a **Deployment** with a **Service** and **Ingress**.
- Set `NEXT_PUBLIC_API_URL` to the server's internal or external URL.
- Serve behind a reverse proxy (nginx, Traefik, or cloud load balancer) with TLS termination.

---

## 8. RBAC Setup

Orbflow includes a role-based access control system with permissions: `view`, `edit`, `execute`, `approve`, `delete`, `manage_credentials`, and `admin`.

### Step 1: Start Without RBAC (Development)

By default, when no `auth_token` is set and no RBAC policies exist, all API endpoints are unauthenticated and unrestricted. This is suitable for local development only.

### Step 2: Enable Authentication

Set `server.auth_token` in your config YAML or via the `AUTH_TOKEN` environment variable:

```yaml
server:
  auth_token: "${AUTH_TOKEN}"
```

All API requests (except `/health`, `/node-types`, `/credential-types`, and `/webhooks/*`) must now include:

```
Authorization: Bearer <your-token>
```

### Step 3: Set a Bootstrap Admin

Set the `ORBFLOW_BOOTSTRAP_ADMIN` environment variable to a user ID that should always have full admin access, regardless of RBAC policies:

```bash
export ORBFLOW_BOOTSTRAP_ADMIN="admin@example.com"
```

This acts as a safety net -- if you accidentally lock yourself out via RBAC policies, the bootstrap admin retains full access. The value cannot be `"anonymous"`.

### Step 4: Create RBAC Policies via API

Create your first policy using the REST API:

```bash
curl -X POST http://localhost:8080/rbac/policies \
  -H "Authorization: Bearer ${AUTH_TOKEN}" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "editors",
    "roles": [
      {
        "name": "editor",
        "permissions": ["view", "edit", "execute"]
      }
    ],
    "bindings": [
      {
        "role": "editor",
        "subjects": ["user:alice@example.com", "user:bob@example.com"]
      }
    ],
    "scope": {
      "type": "global"
    }
  }'
```

### Step 5: Use the Access Tab in the UI

The web frontend includes an **Access** tab with the RBAC Editor. It provides a visual interface for managing roles, permissions, bindings, and scopes (global, per-workflow, or per-node).

---

## 9. Observability Setup

### OpenTelemetry Configuration

Enable OTLP export in your config:

```yaml
otel:
  enabled: true
  endpoint: "http://otel-collector:4317"  # gRPC OTLP endpoint
  service_name: "orbflow"
  sample_rate: 1.0                         # 1.0 = 100% of traces, reduce in production
```

### Connecting to Jaeger

Run Jaeger with OTLP ingestion:

```bash
docker run -d --name jaeger \
  -p 16686:16686 \
  -p 4317:4317 \
  jaegertracing/all-in-one:latest
```

Then set:

```yaml
otel:
  enabled: true
  endpoint: "http://localhost:4317"
```

Access the Jaeger UI at `http://localhost:16686`.

### Connecting to Grafana and Prometheus

Use an OpenTelemetry Collector as an intermediary to fan out traces to Jaeger and metrics to Prometheus:

```yaml
# otel-collector-config.yaml
receivers:
  otlp:
    protocols:
      grpc:
        endpoint: "0.0.0.0:4317"

exporters:
  prometheus:
    endpoint: "0.0.0.0:8889"
  otlp/jaeger:
    endpoint: "jaeger:4317"
    tls:
      insecure: true

service:
  pipelines:
    traces:
      receivers: [otlp]
      exporters: [otlp/jaeger]
    metrics:
      receivers: [otlp]
      exporters: [prometheus]
```

Configure Grafana to use Prometheus (`http://prometheus:9090`) as a data source and Jaeger (`http://jaeger:16686`) for trace exploration.

### Structured Logging

Orbflow outputs structured JSON logs by default. Configure the log level and format:

```yaml
log:
  level: "info"      # trace | debug | info | warn | error
  format: "json"     # json | console
```

Use `"console"` format for human-readable output during development. Use `"json"` in production for log aggregation (ELK, Loki, CloudWatch).

Override the log level at runtime with the `RUST_LOG` environment variable:

```bash
RUST_LOG=debug ./target/release/orbflow-server configs/orbflow.yaml
```

---

## 10. Security Checklist

Before deploying to production, verify the following:

- [ ] **Set `auth_token`**: Configure `server.auth_token` in the YAML config to require bearer token authentication on all API endpoints.
- [ ] **Set `encryption_key`**: Provide a strong `CREDENTIAL_ENCRYPTION_KEY` for AES-256-GCM encryption of stored credentials.
- [ ] **Configure RBAC**: Create RBAC policies and set `ORBFLOW_BOOTSTRAP_ADMIN` as a safety net.
- [ ] **Enable TLS**: Terminate TLS at a reverse proxy (nginx, Traefik, cloud load balancer) in front of the server. Orbflow does not natively serve HTTPS.
- [ ] **Set log level to `"info"` or `"warn"`**: Avoid `"debug"` or `"trace"` in production to prevent leaking sensitive data in logs.
- [ ] **Review CORS settings**: Ensure CORS is configured to allow only trusted origins.
- [ ] **Use strong PostgreSQL credentials**: Replace the default `orbflow/orbflow` credentials with strong, randomly generated passwords.
- [ ] **Enable PostgreSQL SSL**: Use `sslmode=require` or `sslmode=verify-full` in the DSN.
- [ ] **Restrict NATS access**: Bind NATS to `127.0.0.1` or use NATS authentication in production. Plugin gRPC traffic should also use loopback addresses when TLS is not enabled.
- [ ] **Never commit `.env` files**: Keep secrets out of version control. Use a secret manager (Vault, AWS Secrets Manager, etc.) for production secrets.
- [ ] **Audit dependencies**: Run `cargo audit` and `cargo deny check` regularly to scan for known vulnerabilities.

---

## 11. Backup and Recovery

### PostgreSQL Backup

Orbflow uses event sourcing with periodic snapshots. This means the PostgreSQL database contains the complete history of all workflow instances, enabling point-in-time recovery.

**Automated backups with `pg_dump`:**

```bash
# Full backup
pg_dump -U orbflow -h localhost -d orbflow -Fc -f orbflow_backup_$(date +%Y%m%d_%H%M%S).dump

# Restore from backup
pg_restore -U orbflow -h localhost -d orbflow -c orbflow_backup_20260322_120000.dump
```

**Continuous archiving (WAL):**

For production, enable PostgreSQL WAL archiving for point-in-time recovery (PITR). If using a managed database (RDS, Cloud SQL), enable automated backups in the provider's console.

**Recommended backup schedule:**

| Backup Type | Frequency | Retention |
|-------------|-----------|-----------|
| Full dump   | Daily     | 30 days   |
| WAL archiving | Continuous | 7 days |
| Snapshot (if cloud) | Daily | 14 days |

### NATS JetStream Persistence

NATS JetStream stores message streams on disk (in `data_dir` for embedded, or `/data` in Docker). Back up the JetStream data directory periodically, though this is less critical than PostgreSQL -- NATS primarily serves as a transient message bus, and Orbflow's event-sourced state in PostgreSQL is the source of truth.

For external NATS clusters, use NATS's built-in replication (cluster mode with 3+ nodes) for high availability rather than file-level backups.

---

## 12. Troubleshooting

### "Forbidden: insufficient permissions"

This indicates an RBAC policy is blocking the request.

1. Verify the user ID in the request matches a subject in an RBAC policy binding.
2. Check that the policy grants the required permission (`view`, `edit`, `execute`, etc.).
3. Check the policy scope -- a workflow-scoped policy does not grant access to other workflows.
4. As a temporary workaround, set `ORBFLOW_BOOTSTRAP_ADMIN` to your user ID to bypass RBAC checks.

### Connection Refused to PostgreSQL

```
error: config: I/O error: read configs/orbflow.yaml: Connection refused
```

1. Verify PostgreSQL is running: `docker compose ps postgres` or `pg_isready -h localhost -U orbflow`.
2. Check the `database.dsn` in your config matches the actual host, port, user, and database.
3. Ensure the database exists: `psql -U orbflow -h localhost -c "SELECT 1"`.
4. If using Docker, ensure the container is healthy: `just infra-status`.

### Connection Refused to NATS

1. If using embedded NATS (`embedded: true`), ensure `data_dir` is writable.
2. If using external NATS, verify the URL: `nats sub -s nats://localhost:4222 test`.
3. Ensure JetStream is enabled on the NATS server (`--jetstream` flag).
4. Check NATS monitoring: `curl http://localhost:8222/healthz`.

### LNK1318 Error on Windows

The MSVC linker may fail with `LNK1318` on large test binaries. This is already handled by the project's `.cargo/config.toml`, which disables PDB generation (`/DEBUG:NONE`). The workspace `Cargo.toml` also sets `debug = false` for dev and test profiles.

If you still encounter this error:

1. Verify `.cargo/config.toml` contains the linker flags.
2. Run `cargo clean` and rebuild.
3. Ensure you have sufficient disk space for the build artifacts.

### Frontend Cannot Reach Backend

1. Check `NEXT_PUBLIC_API_URL` is set correctly (default: `http://localhost:8080`).
2. Verify the backend is running and responding: `curl http://localhost:8080/health`.
3. If using Docker, the frontend container must use the Docker service name (`http://server:8080`), not `localhost`.
4. Check browser console for CORS errors -- ensure the server's CORS configuration allows the frontend origin.

### High Memory Usage on Workers

1. Reduce `worker.concurrency` in the config to limit parallel task execution.
2. Check for runaway HTTP request nodes -- consider setting timeouts on external API calls.
3. Monitor with `RUST_LOG=debug` temporarily to identify which tasks consume memory.

### Embedded NATS Data Corruption

If embedded NATS fails to start after an unclean shutdown:

1. Stop the server.
2. Delete the NATS data directory: `rm -rf /tmp/orbflow-nats` (or the configured `data_dir`).
3. Restart the server. JetStream streams will be recreated automatically.
4. In-flight tasks will be re-dispatched via the engine's crash recovery mechanism (event sourcing replay).
