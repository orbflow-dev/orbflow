# ADR-002: Event Sourcing with Snapshotting

## Status
Accepted

## Date
2025-12

## Context
Workflow execution state changes frequently -- a single instance may transition through dozens of node states during its lifetime. We need:

1. **Full audit trails** for compliance (SOC2, HIPAA, PCI)
2. **Crash recovery** without data loss
3. **Time-travel debugging** to reconstruct any past state

### Alternatives Considered

- **Mutable state + audit log**: Simpler, but the audit log is a secondary projection that can drift from actual state. Recovery requires trusting the current snapshot, which may be corrupted.
- **CQRS without event sourcing**: Separates reads and writes but still relies on mutable state as the source of truth. Loses the ability to replay and reconstruct.
- **Append-only events (chosen)**: The event log *is* the source of truth. State is derived, never stored directly. Recovery is deterministic: replay events from the last snapshot.

## Decision
Use event sourcing for workflow instances (not workflow definitions):

- Every state change emits a `DomainEvent` (defined in `orbflow-core::event`) -- 16 variants covering instance lifecycle, node execution, approval flows, anomaly detection, and policy changes
- Events are append-only and immutable once written
- The `EventStore` port trait provides `append_event`, `load_events`, and `save_snapshot`
- Snapshots are taken at configurable intervals (default: every 10 events) via `EngineOptionsBuilder`
- Crash recovery: load the last snapshot, replay events since that snapshot
- Per-instance locking via `DashMap<InstanceId, Arc<Mutex<()>>>` prevents concurrent event write conflicts

## Consequences

**Benefits:**
- Complete, tamper-evident audit trail (SHA-256 hash chain, Merkle proofs)
- Deterministic crash recovery -- no data loss even on unclean shutdown
- Time-travel debugging: reconstruct instance state at any point
- Natural fit for the distributed architecture (events flow through NATS)

**Trade-offs:**
- Storage grows with event count (mitigated by snapshots and configurable retention)
- Event replay cost scales with instance duration between snapshots
- Schema evolution requires careful event versioning (serde rename tags maintain wire stability)
- Append-only migrations are more complex than simple ALTER TABLE

See: `orbflow-core::event::DomainEvent`, `orbflow-core::ports::EventStore`, `orbflow-postgres::event`
