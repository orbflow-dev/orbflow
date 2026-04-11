# ADR-001: Cargo Workspace with Adapter Crates

## Status
Accepted

## Context
A distributed workflow engine with 20 crates needs a clear crate structure that enforces dependency direction, prevents circular imports, and keeps each crate focused on a single responsibility.

## Decision
Use a Cargo workspace with a Ports & Adapters layout:

- **`orbflow-core`** defines all domain types and port traits (the only crate imported across crate boundaries)
- Adapter crates are named by the dependency they wrap: `orbflow-postgres`, `orbflow-natsbus`, `orbflow-httpapi`, etc.
- Dependencies point inward -- adapter crates depend on `orbflow-core`; a few crates (e.g., `orbflow-builtins`, `orbflow-engine`) also depend on utility adapters (`orbflow-cel`, `orbflow-mcp`) when they need their functionality
- Binary crates (`orbflow-server`, `orbflow-worker-bin`) wire adapters together at the composition root

## Consequences
- Cargo enforces no circular dependencies at compile time
- Each adapter crate has one primary external dependency (Axum, sqlx, async-nats, etc.)
- Adding a new adapter means adding a new crate -- existing crates are unaffected
- Crate-level visibility (`pub(crate)`) replaces Go's `internal/` convention
- Clear import graph: `orbflow-server` -> `orbflow-engine` -> `orbflow-core` (never the reverse)
