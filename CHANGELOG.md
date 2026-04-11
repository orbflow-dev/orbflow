# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-03-30

### Added

- **Engine**: DAG-based workflow execution with CEL expression evaluation, saga compensation, and crash recovery
- **21 built-in nodes**: HTTP, email, transform, filter, delay, sort, encode, template, log, MCP tool, and AI nodes (chat, classify, extract, sentiment, summarize, translate)
- **Event sourcing**: Instance state changes persisted as domain events with periodic snapshots
- **REST API**: Axum-based HTTP API with CORS, rate limiting, and consistent response envelope
- **gRPC API**: JSON codec over TCP for programmatic access
- **Worker system**: Distributed task execution via NATS JetStream message transport
- **Credential management**: Encrypted credential storage (AES-256-GCM) with trust tiers and access policies
- **RBAC**: Role-based access control with policy bindings and scoped permissions
- **Trigger system**: Cron scheduler, webhook, and event-based triggers
- **Plugin system**: External plugin loader via JSON-RPC subprocess protocol with gRPC support
- **Visual workflow builder**: Next.js 16 frontend with drag-and-drop canvas, config modal, and execution viewer
- **Change requests**: PR-style collaboration workflow with visual diff and review comments
- **Alerts and budgets**: Configurable alerting rules and execution budget tracking
- **Analytics and metering**: Workflow execution analytics and usage metering
- **OpenTelemetry**: Distributed tracing and metrics export
- **PostgreSQL storage**: Full persistence with migrations, event sourcing, and snapshots
- **NATS JetStream**: Message transport for coordinator-worker communication
- **Docker deployment**: Production-ready Dockerfile and docker-compose configuration

[0.1.0]: https://github.com/orbflow-dev/orbflow/releases/tag/v0.1.0
