# ╔══════════════════════════════════════════════════════════════════════════════╗
# ║  Orbflow — Distributed Workflow Automation Engine                             ║
# ║  Run `make help` to see all available targets                              ║
# ╚══════════════════════════════════════════════════════════════════════════════╝

# ── Configuration ────────────────────────────────────────────────────────────

CARGO        := cargo
CONFIG       := configs/orbflow.yaml
CONFIG_DEV   := configs/orbflow.dev.yaml
CONFIG_DOCKER := configs/orbflow.docker.yaml
SHELL        := bash
.DEFAULT_GOAL := help

# Binaries
SERVER_BIN   := orbflow-server
WORKER_BIN   := orbflow-worker-bin

# Colors (disable with NO_COLOR=1)
ifndef NO_COLOR
  CYAN    := \033[36m
  GREEN   := \033[32m
  YELLOW  := \033[33m
  RED     := \033[31m
  BOLD    := \033[1m
  DIM     := \033[2m
  RESET   := \033[0m
else
  CYAN    :=
  GREEN   :=
  YELLOW  :=
  RED     :=
  BOLD    :=
  DIM     :=
  RESET   :=
endif

# ── Help ─────────────────────────────────────────────────────────────────────

.PHONY: help
help: ## Show this help message
	@printf "\n$(BOLD)$(CYAN)  Orbflow Makefile$(RESET)\n"
	@printf "  $(DIM)─────────────────────────────────────────────────$(RESET)\n\n"
	@awk 'BEGIN {FS = ":.*##"} \
		/^##@/ { printf "  $(BOLD)$(YELLOW)%s$(RESET)\n", substr($$0, 5) } \
		/^[a-zA-Z_-]+:.*##/ { printf "  $(GREEN)%-20s$(RESET) %s\n", $$1, $$2 }' \
		$(MAKEFILE_LIST)
	@printf "\n"

# ── Quick Start ──────────────────────────────────────────────────────────────

##@ Quick Start

.PHONY: setup
setup: ## First-time setup: install tools, deps, start infra, run migrations
	@printf "$(BOLD)$(CYAN)[1/4]$(RESET) Checking prerequisites...\n"
	@command -v cargo  >/dev/null 2>&1 || { printf "$(RED)error:$(RESET) cargo not found — install Rust via rustup.rs\n"; exit 1; }
	@command -v pnpm   >/dev/null 2>&1 || { printf "$(RED)error:$(RESET) pnpm not found — install via corepack or npm\n"; exit 1; }
	@command -v docker >/dev/null 2>&1 || { printf "$(RED)error:$(RESET) docker not found — install Docker Desktop\n"; exit 1; }
	@printf "  $(GREEN)OK$(RESET) All prerequisites found\n"
	@printf "$(BOLD)$(CYAN)[2/4]$(RESET) Installing frontend dependencies...\n"
	@pnpm install
	@printf "$(BOLD)$(CYAN)[3/4]$(RESET) Starting infrastructure...\n"
	@$(MAKE) --no-print-directory infra
	@printf "$(BOLD)$(CYAN)[4/4]$(RESET) Verifying Rust workspace...\n"
	@$(CARGO) check --workspace 2>&1 | tail -1
	@printf "\n$(GREEN)$(BOLD)  Setup complete!$(RESET)\n"
	@printf "  Run $(CYAN)make dev$(RESET) to start everything.\n\n"

# ── Development ──────────────────────────────────────────────────────────────

##@ Development

.PHONY: dev dev-backend dev-server dev-worker dev-web

dev: infra ## Start everything (server + worker + web) with live reload
	@printf "$(BOLD)$(CYAN)Starting full dev stack...$(RESET)\n"
	@printf "  $(DIM)Server  → http://localhost:8080$(RESET)\n"
	@printf "  $(DIM)Web     → http://localhost:3000$(RESET)\n\n"
	@npx concurrently -k \
		-n server,worker,web \
		-c blue,green,magenta \
		"$(CARGO) run -p $(SERVER_BIN) -- $(CONFIG_DEV)" \
		"$(CARGO) run -p $(WORKER_BIN) -- $(CONFIG_DEV)" \
		"pnpm dev"

dev-backend: infra ## Start server + worker only (no frontend)
	@printf "$(BOLD)$(CYAN)Starting backend...$(RESET)\n"
	@printf "  $(DIM)Server  → http://localhost:8080$(RESET)\n\n"
	@npx concurrently -k \
		-n server,worker \
		-c blue,green \
		"$(CARGO) run -p $(SERVER_BIN) -- $(CONFIG_DEV)" \
		"$(CARGO) run -p $(WORKER_BIN) -- $(CONFIG_DEV)"

dev-server: ## Run the server only (requires infra)
	@$(CARGO) run -p $(SERVER_BIN) -- $(CONFIG_DEV)

dev-worker: ## Run the worker only (requires infra)
	@$(CARGO) run -p $(WORKER_BIN) -- $(CONFIG_DEV)

dev-web: ## Run the Next.js frontend dev server (Turbopack)
	@pnpm dev

# ── Build ────────────────────────────────────────────────────────────────────

##@ Build

.PHONY: build build-debug build-web check

build: ## Build release binaries (optimized)
	@printf "$(BOLD)$(CYAN)Building release binaries...$(RESET)\n"
	@$(CARGO) build --release -p $(SERVER_BIN) -p $(WORKER_BIN)
	@printf "$(GREEN)$(BOLD)  Build complete$(RESET)\n"
	@printf "  $(DIM)target/release/orbflow-server$(RESET)\n"
	@printf "  $(DIM)target/release/orbflow-worker-bin$(RESET)\n"

build-debug: ## Build debug binaries (fast compile)
	@$(CARGO) build -p $(SERVER_BIN) -p $(WORKER_BIN)

build-web: ## Build the Next.js frontend for production
	@pnpm build

check: ## Type-check the entire Rust workspace (no codegen)
	@$(CARGO) check --workspace

# ── Test ─────────────────────────────────────────────────────────────────────

##@ Test

.PHONY: test test-crate test-verbose test-web test-all

test: ## Run all Rust tests
	@printf "$(BOLD)$(CYAN)Running workspace tests...$(RESET)\n"
	@$(CARGO) test --workspace
	@printf "$(GREEN)$(BOLD)  All tests passed$(RESET)\n"

test-crate: ## Run tests for one crate (usage: make test-crate CRATE=orbflow-core)
ifndef CRATE
	@printf "$(RED)error:$(RESET) specify CRATE=<name>, e.g. $(CYAN)make test-crate CRATE=orbflow-core$(RESET)\n" && exit 1
else
	@printf "$(BOLD)$(CYAN)Testing $(CRATE)...$(RESET)\n"
	@$(CARGO) test -p $(CRATE) -- --nocapture
endif

test-verbose: ## Run all Rust tests with output visible
	@$(CARGO) test --workspace -- --nocapture

test-web: ## Run frontend tests
	@pnpm test

test-all: test test-web ## Run all tests (Rust + frontend)

# ── Lint & Format ────────────────────────────────────────────────────────────

##@ Lint & Format

.PHONY: lint fmt fmt-check lint-web lint-all

lint: ## Run clippy with strict warnings
	@printf "$(BOLD)$(CYAN)Running clippy...$(RESET)\n"
	@$(CARGO) clippy --workspace -- -D warnings
	@printf "$(GREEN)$(BOLD)  No warnings$(RESET)\n"

fmt: ## Format all Rust code
	@$(CARGO) fmt --all

fmt-check: ## Check Rust formatting (CI-friendly, no changes)
	@$(CARGO) fmt --all -- --check

lint-web: ## Lint frontend code
	@pnpm lint

lint-all: lint fmt-check lint-web ## Run all linters (Rust + frontend)

# ── Quality Gate ─────────────────────────────────────────────────────────────

##@ Quality Gate

.PHONY: ci pre-commit

ci: fmt-check lint test build ## Full CI pipeline: format + lint + test + build
	@printf "\n$(GREEN)$(BOLD)  CI pipeline passed$(RESET)\n"

pre-commit: fmt-check lint test ## Quick pre-commit check: format + lint + test
	@printf "\n$(GREEN)$(BOLD)  Pre-commit checks passed$(RESET)\n"

# ── Infrastructure ───────────────────────────────────────────────────────────

##@ Infrastructure

.PHONY: infra infra-down infra-reset infra-logs infra-status

infra: ## Start Postgres + NATS containers (waits until healthy)
	@printf "$(BOLD)$(CYAN)Starting infrastructure...$(RESET)\n"
	@docker compose up -d postgres nats --wait
	@printf "  $(GREEN)Postgres$(RESET)  → localhost:5432\n"
	@printf "  $(GREEN)NATS$(RESET)      → localhost:4222 $(DIM)(monitor: :8222)$(RESET)\n"

infra-down: ## Stop infrastructure containers
	@docker compose down --remove-orphans

infra-reset: ## Destroy and recreate infra (wipes all data)
	@printf "$(YELLOW)$(BOLD)  WARNING: This will delete all Postgres and NATS data!$(RESET)\n"
	@printf "  Press Ctrl+C to cancel, or wait 3 seconds...\n"
	@sleep 3
	@docker compose down --remove-orphans -v
	@$(MAKE) --no-print-directory infra
	@printf "$(GREEN)$(BOLD)  Infrastructure reset complete$(RESET)\n"

infra-logs: ## Tail infrastructure container logs
	@docker compose logs -f postgres nats

infra-status: ## Show infrastructure container status
	@docker compose ps

# ── Docker (Production) ─────────────────────────────────────────────────────

##@ Docker (Production)

.PHONY: docker-build docker-up docker-down docker-logs docker-restart

docker-build: ## Build all Docker images
	@docker compose build

docker-up: ## Start all services in Docker
	@docker compose up -d
	@printf "$(GREEN)$(BOLD)  All services started$(RESET)\n"
	@docker compose ps

docker-down: ## Stop all Docker services
	@docker compose down --remove-orphans

docker-logs: ## Tail all Docker service logs
	@docker compose logs -f

docker-restart: docker-down docker-up ## Restart all Docker services

# ── Database ─────────────────────────────────────────────────────────────────

##@ Database

.PHONY: db-shell db-status

db-shell: ## Open psql shell to local Postgres
	@docker compose exec postgres psql -U orbflow -d orbflow

db-status: ## Check database connection and table count
	@docker compose exec postgres psql -U orbflow -d orbflow -c \
		"SELECT count(*) as tables FROM information_schema.tables WHERE table_schema = 'public';" \
		2>/dev/null || printf "$(RED)error:$(RESET) Postgres is not running. Run $(CYAN)make infra$(RESET) first.\n"

# ── Cleanup ──────────────────────────────────────────────────────────────────

##@ Cleanup

.PHONY: clean clean-all clean-docker

clean: ## Remove Rust build artifacts
	@$(CARGO) clean
	@printf "$(DIM)Rust build artifacts removed$(RESET)\n"

clean-all: clean ## Remove all artifacts (Rust + node_modules + docker volumes)
	@rm -rf node_modules apps/web/node_modules apps/web/.next packages/orbflow-core/node_modules
	@printf "$(DIM)Node modules removed$(RESET)\n"

clean-docker: ## Remove Docker volumes (wipes Postgres + NATS data)
	@docker compose down --remove-orphans -v
	@printf "$(DIM)Docker volumes removed$(RESET)\n"

# ── Debugging ────────────────────────────────────────────────────────────────

##@ Debugging

.PHONY: debug-server debug-worker env-check tree

debug-server: infra ## Run server with RUST_LOG=debug
	@RUST_LOG=debug $(CARGO) run -p $(SERVER_BIN) -- $(CONFIG_DEV)

debug-worker: ## Run worker with RUST_LOG=debug
	@RUST_LOG=debug $(CARGO) run -p $(WORKER_BIN) -- $(CONFIG_DEV)

env-check: ## Verify all required env vars and tools are set
	@printf "$(BOLD)Environment Check$(RESET)\n"
	@printf "  $(DIM)──────────────────────────────────$(RESET)\n"
	@printf "  cargo:   " && (cargo --version 2>/dev/null || printf "$(RED)not found$(RESET)\n")
	@printf "  rustc:   " && (rustc --version 2>/dev/null || printf "$(RED)not found$(RESET)\n")
	@printf "  pnpm:    " && (pnpm --version 2>/dev/null || printf "$(RED)not found$(RESET)\n")
	@printf "  node:    " && (node --version 2>/dev/null || printf "$(RED)not found$(RESET)\n")
	@printf "  docker:  " && (docker --version 2>/dev/null || printf "$(RED)not found$(RESET)\n")
	@printf "  $(DIM)──────────────────────────────────$(RESET)\n"
	@printf "  config:  $(CONFIG_DEV)\n"
	@[ -f .env ] && printf "  .env:    $(GREEN)found$(RESET)\n" || printf "  .env:    $(YELLOW)missing (copy .env.example)$(RESET)\n"

tree: ## Show crate dependency graph (requires cargo-tree)
	@$(CARGO) tree --workspace --depth 1 -e no-dev 2>/dev/null || \
		printf "$(YELLOW)Install cargo-tree:$(RESET) cargo install cargo-tree\n"
