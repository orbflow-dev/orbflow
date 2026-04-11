# ╔══════════════════════════════════════════════════════════════════════════════╗
# ║  Orbflow — Distributed Workflow Automation Engine                             ║
# ║  Install just: cargo install just | brew install just | winget install just║
# ║  Run `just` to see all available recipes                                   ║
# ╚══════════════════════════════════════════════════════════════════════════════╝

# Cross-platform shell configuration
set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]
set dotenv-load := true

# ── Variables ───────────────────────────────────────────────────────────────

config         := "configs/orbflow.yaml"
config_dev     := "configs/orbflow.dev.yaml"
config_docker  := "configs/orbflow.docker.yaml"
server_bin     := "orbflow-server"
worker_bin     := "orbflow-worker-bin"
web_port       := env("WEB_PORT", "3000")

# Cap Node.js heap to 4 GB — prevents Turbopack dev server from ballooning.
# Exported so it reaches child processes spawned by concurrently on all platforms.
export NODE_OPTIONS := "--max-old-space-size=4096"

# ── Default: show help ─────────────────────────────────────────────────────

[doc("Show all available recipes")]
default:
    @just --list --unsorted

# ═══════════════════════════════════════════════════════════════════════════════
# Quick Start
# ═══════════════════════════════════════════════════════════════════════════════

[doc("First-time setup: check tools, install deps, start infra, create dev DB, verify workspace")]
setup:
    @echo "[1/5] Checking prerequisites..."
    cargo --version
    pnpm --version
    docker --version
    @echo "[2/5] Installing frontend dependencies..."
    pnpm install
    @echo "[3/5] Starting infrastructure..."
    just infra
    @echo "[4/5] Creating dev database..."
    just db-create
    @echo "[5/5] Verifying Rust workspace..."
    cargo check --workspace
    @echo ""
    @echo "Setup complete! Run 'just dev' to start everything."

# ═══════════════════════════════════════════════════════════════════════════════
# Development
# ═══════════════════════════════════════════════════════════════════════════════

[doc("Start everything: server + worker + web (live reload). Set WEB_PORT to override (default 3000)")]
dev: infra _kill-stale-processes
    @echo "Starting full dev stack..."
    @echo "  Server → http://localhost:8080"
    @echo "  Web    → http://localhost:{{web_port}}"
    npx concurrently -k \
        -n server,worker,web \
        -c blue,green,magenta \
        "cargo run -p {{server_bin}} -- {{config_dev}}" \
        "cargo run -p {{worker_bin}} -- {{config_dev}}" \
        "pnpm dev"


[doc("Start server + worker only (no frontend)")]
dev-backend: infra
    npx concurrently -k \
        -n server,worker \
        -c blue,green \
        "cargo run -p {{server_bin}} -- {{config_dev}}" \
        "cargo run -p {{worker_bin}} -- {{config_dev}}"

[doc("Run the server only (requires infra)")]
dev-server:
    cargo run -p {{server_bin}} -- {{config_dev}}

[doc("Run the worker only (requires infra)")]
dev-worker:
    cargo run -p {{worker_bin}} -- {{config_dev}}

[doc("Run the Next.js frontend dev server")]
dev-web:
    pnpm dev

# Kill stale orbflow/node processes from a previous dev session to avoid "Access is denied" build errors.
[private]
[windows]
_kill-stale-processes:
    function Stop-ProcessTree([int]$Pid) { $children = Get-CimInstance Win32_Process | Where-Object { $_.ParentProcessId -eq $Pid }; foreach ($child in $children) { Stop-ProcessTree -Pid $child.ProcessId }; Stop-Process -Id $Pid -Force -ErrorAction SilentlyContinue }; $killed = @(); foreach ($name in @('orbflow-server', 'orbflow-worker-bin')) { $procs = Get-Process -Name $name -ErrorAction SilentlyContinue; if ($procs) { $procs | Stop-Process -ErrorAction SilentlyContinue; Start-Sleep -Seconds 2; $still = Get-Process -Name $name -ErrorAction SilentlyContinue; if ($still) { $still | Stop-Process -Force -ErrorAction SilentlyContinue }; $killed += $name } }; $webDir = (Resolve-Path 'apps/web').Path; $nextDevRoots = Get-CimInstance Win32_Process -Filter "Name = 'node.exe'" | Where-Object { $_.CommandLine -like '*next\dist\bin\next*' -and $_.CommandLine -like '*\"dev\"*' -and $_.CommandLine -like "*$webDir*" }; foreach ($proc in $nextDevRoots) { Stop-ProcessTree -Pid $proc.ProcessId; $killed += "next-dev:$($proc.ProcessId)" }; if ($killed.Count -gt 0) { Write-Host "Killed stale processes: $($killed -join ', ')" -ForegroundColor Yellow } else { Write-Host "No stale processes found" -ForegroundColor DarkGray }; $port = {{web_port}}; try { $c = Get-NetTCPConnection -LocalPort $port -State Listen -ErrorAction Stop } catch { $c = $null }; if ($c) { $procId = $c.OwningProcess; $n = (Get-Process -Id $procId -ErrorAction SilentlyContinue).ProcessName; if ($n -eq 'node') { Stop-Process -Id $procId -Force -ErrorAction SilentlyContinue; Write-Host "Killed stale node process on port $port (PID $procId)" -ForegroundColor Yellow } else { Write-Host ""; Write-Host "ERROR: Port $port is already in use by '$n' (PID $procId)." -ForegroundColor Red; Write-Host "  Use a different port:  WEB_PORT=3001 just dev" -ForegroundColor Yellow; Write-Host "  Or stop it yourself:  Stop-Process -Id $procId -Force" -ForegroundColor Yellow; Write-Host ""; exit 1 } }

[private]
[unix]
_kill-stale-processes:
    #!/usr/bin/env bash
    killed=()
    for name in orbflow-server orbflow-worker-bin; do
        pids=$(pgrep -x "$name" 2>/dev/null)
        if [ -n "$pids" ]; then
            echo "$pids" | xargs kill -TERM 2>/dev/null
            sleep 2
            # Force-kill any survivors
            remaining=$(pgrep -x "$name" 2>/dev/null)
            if [ -n "$remaining" ]; then
                echo "$remaining" | xargs kill -9 2>/dev/null
            fi
            killed+=("$name")
        fi
    done
    if [ ${#killed[@]} -gt 0 ]; then
        echo -e "\033[33mKilled stale processes: ${killed[*]}\033[0m"
    else
        echo -e "\033[90mNo stale processes found\033[0m"
    fi
    web_dir="$(pwd)/apps/web"
    next_pids=$(pgrep -af "next.*dev" 2>/dev/null | grep "$web_dir" | awk '{print $1}' || true)
    if [ -n "$next_pids" ]; then
        echo "$next_pids" | xargs kill -TERM 2>/dev/null
        sleep 1
        remaining_next=$(echo "$next_pids" | xargs -r -I{} sh -c 'kill -0 "{}" 2>/dev/null && echo "{}"' || true)
        if [ -n "$remaining_next" ]; then
            echo "$remaining_next" | xargs kill -9 2>/dev/null
        fi
        echo -e "\033[33mKilled stale Next dev process(es): $(echo "$next_pids" | tr '\n' ' ')\033[0m"
    fi
    pid=$(lsof -ti :{{web_port}} 2>/dev/null | head -1)
    if [ -n "$pid" ]; then
        name=$(ps -p "$pid" -o comm= 2>/dev/null || echo "unknown")
        if [ "$name" = "node" ]; then
            kill -TERM "$pid" 2>/dev/null
            sleep 1
            kill -0 "$pid" 2>/dev/null && kill -9 "$pid" 2>/dev/null
            echo -e "\033[33mKilled stale node process on port {{web_port}} (PID $pid)\033[0m"
        else
            echo ""
            echo -e "\033[31mERROR: Port {{web_port}} is already in use by '$name' (PID $pid).\033[0m"
            echo -e "\033[33m  Use a different port:  WEB_PORT=3001 just dev\033[0m"
            echo -e "\033[33m  Or stop it yourself:  kill $pid\033[0m"
            echo ""
            exit 1
        fi
    fi

[doc("Build and run everything in production mode (release binaries + Next.js production build)")]
prod: infra _kill-stale-processes
    @echo "Building release binaries..."
    cargo build --release -p {{server_bin}} -p {{worker_bin}}
    @echo "Building Next.js frontend..."
    pnpm build
    @echo ""
    @echo "Starting production stack..."
    @echo "  Server → http://localhost:8080"
    @echo "  Web    → http://localhost:{{web_port}}"
    npx concurrently -k \
        -n server,worker,web \
        -c blue,green,magenta \
        "cargo run --release -p {{server_bin}} -- {{config_dev}}" \
        "cargo run --release -p {{worker_bin}} -- {{config_dev}}" \
        "pnpm --filter orbflow-web start -p {{web_port}}"

# ═══════════════════════════════════════════════════════════════════════════════
# Build
# ═══════════════════════════════════════════════════════════════════════════════

[doc("Build release binaries (optimized)")]
build:
    cargo build --release -p {{server_bin}} -p {{worker_bin}}
    @echo "Build complete:"
    @echo "  target/release/orbflow-server"
    @echo "  target/release/orbflow-worker-bin"

[doc("Build debug binaries (fast compile)")]
build-debug:
    cargo build -p {{server_bin}} -p {{worker_bin}}

[doc("Build the Next.js frontend for production")]
build-web:
    pnpm build

[doc("Type-check the entire Rust workspace")]
check:
    cargo check --workspace

# ═══════════════════════════════════════════════════════════════════════════════
# Test
# ═══════════════════════════════════════════════════════════════════════════════

[doc("Run all Rust tests")]
test:
    cargo test --workspace

[doc("Run tests for a single crate (e.g., just test-crate orbflow-core)")]
test-crate crate:
    cargo test -p {{crate}} -- --nocapture

[doc("Run all Rust tests with stdout visible")]
test-verbose:
    cargo test --workspace -- --nocapture

[doc("Run frontend tests")]
test-web:
    pnpm test

[doc("Run ALL tests (Rust + frontend)")]
test-all: test test-web

# ═══════════════════════════════════════════════════════════════════════════════
# Lint & Format
# ═══════════════════════════════════════════════════════════════════════════════

[doc("Run clippy with strict warnings")]
lint:
    cargo clippy --workspace -- -D warnings

[doc("Format all Rust code")]
fmt:
    cargo fmt --all

[doc("Check Rust formatting (CI-friendly, no changes)")]
fmt-check:
    cargo fmt --all -- --check

[doc("Lint frontend code")]
lint-web:
    pnpm lint

[doc("Run all linters (Rust + frontend)")]
lint-all: lint fmt-check lint-web

# ═══════════════════════════════════════════════════════════════════════════════
# Quality Gate
# ═══════════════════════════════════════════════════════════════════════════════

[doc("Full CI pipeline: format + lint + test + build")]
ci: fmt-check lint test build
    @echo "CI pipeline passed"

[doc("Quick pre-commit check: format + lint + test")]
pre-commit: fmt-check lint test
    @echo "Pre-commit checks passed"

# ═══════════════════════════════════════════════════════════════════════════════
# Infrastructure
# ═══════════════════════════════════════════════════════════════════════════════

[doc("Start Postgres + NATS containers (waits until healthy)")]
infra:
    docker compose up -d postgres nats --wait
    @echo "Postgres → localhost:5432"
    @echo "NATS     → localhost:4222 (monitor: :8222)"

[doc("Stop infrastructure containers")]
infra-down:
    docker compose down --remove-orphans

[doc("Destroy and recreate infra (wipes all data)")]
infra-reset:
    @echo "WARNING: This will delete all Postgres and NATS data!"
    @echo "Press Ctrl+C to cancel..."
    sleep 3
    docker compose down --remove-orphans -v
    just infra
    @echo "Infrastructure reset complete"

[doc("Tail infrastructure container logs")]
infra-logs:
    docker compose logs -f postgres nats

[doc("Show infrastructure container status")]
infra-status:
    docker compose ps

# ═══════════════════════════════════════════════════════════════════════════════
# Docker (Production)
# ═══════════════════════════════════════════════════════════════════════════════

[doc("Build all Docker images")]
docker-build:
    docker compose build

[doc("Start all services in Docker")]
docker-up:
    docker compose up -d
    docker compose ps

[doc("Stop all Docker services")]
docker-down:
    docker compose down --remove-orphans

[doc("Tail all Docker service logs")]
docker-logs:
    docker compose logs -f

[doc("Restart all Docker services")]
docker-restart: docker-down docker-up

# ═══════════════════════════════════════════════════════════════════════════════
# Database
# ═══════════════════════════════════════════════════════════════════════════════

[doc("Open psql shell to local Postgres")]
db-shell:
    docker compose exec postgres psql -U orbflow -d orbflow

[doc("Create the orbflow_dev database (idempotent)")]
[unix]
db-create:
    -docker compose exec postgres createdb -U orbflow orbflow_dev 2>/dev/null || true
    @echo "orbflow_dev database ready"

[doc("Create the orbflow_dev database (idempotent)")]
[windows]
db-create:
    -docker compose exec postgres createdb -U orbflow orbflow_dev
    @echo "orbflow_dev database ready"

[doc("Check database connection and table count")]
db-status:
    docker compose exec postgres psql -U orbflow -d orbflow -c "SELECT count(*) as tables FROM information_schema.tables WHERE table_schema = 'public';"

# ═══════════════════════════════════════════════════════════════════════════════
# Cleanup
# ═══════════════════════════════════════════════════════════════════════════════

[doc("Remove Rust build artifacts")]
clean:
    cargo clean

[doc("Remove all artifacts (Rust + node_modules + .next)")]
[unix]
clean-all: clean
    rm -rf node_modules apps/web/node_modules apps/web/.next packages/orbflow-core/node_modules

[doc("Remove all artifacts (Rust + node_modules + .next)")]
[windows]
clean-all: clean
    if (Test-Path node_modules) { Remove-Item -Recurse -Force node_modules }
    if (Test-Path apps/web/node_modules) { Remove-Item -Recurse -Force apps/web/node_modules }
    if (Test-Path apps/web/.next) { Remove-Item -Recurse -Force apps/web/.next }
    if (Test-Path packages/orbflow-core/node_modules) { Remove-Item -Recurse -Force packages/orbflow-core/node_modules }

[doc("Remove Docker volumes (wipes Postgres + NATS data)")]
clean-docker:
    docker compose down --remove-orphans -v

# ═══════════════════════════════════════════════════════════════════════════════
# Debugging
# ═══════════════════════════════════════════════════════════════════════════════

[doc("Run server with RUST_LOG=debug")]
debug-server: infra
    RUST_LOG=debug cargo run -p {{server_bin}} -- {{config_dev}}

[doc("Run worker with RUST_LOG=debug")]
debug-worker:
    RUST_LOG=debug cargo run -p {{worker_bin}} -- {{config_dev}}

[doc("Verify all required tools are installed")]
env-check:
    @echo "Environment Check"
    @echo "──────────────────────────────────"
    cargo --version
    rustc --version
    pnpm --version
    node --version
    docker --version
    @echo "──────────────────────────────────"
    @echo "Config: {{config_dev}}"

[doc("Show crate dependency graph")]
tree:
    cargo tree --workspace --depth 1 -e no-dev
