# Security Decisions

This file records security risk-acceptance decisions and scope limitations for the Orbflow project. Each entry documents the rationale, the date it was last reviewed, and the condition under which the decision must be re-evaluated. It is the authoritative reference for security items that are intentionally deferred, narrowed, or accepted rather than fixed in full.

Entries are added by the team when a security finding is triaged as a known limitation rather than an immediate fix. If the re-evaluation condition is ever met, the entry must be revisited before the relevant code ships.

---

## RUSTSEC-2023-0071 — RSA timing side-channel (Marvin attack) via sqlx-mysql

**Status:** Accepted — vulnerable path never instantiated in this workspace.

**Rationale:** The advisory affects `rsa 0.9.x` and enters the dependency graph transitively through `sqlx-mysql`, which is a transitive dependency of the `sqlx` 0.8 macro crate. This workspace is PostgreSQL-only. The `sqlx-mysql` feature is never enabled in any crate's `Cargo.toml`, so the vulnerable RSA code path is never compiled into any release binary. Removing the advisory from `cargo audit` output would require patching sqlx macros — the effort is disproportionate to the risk given the path is never instantiated.

**Re-evaluation condition:** If a MySQL adapter, the `sqlx` `mysql` feature, or any crate that enables `sqlx-mysql` is added to this workspace, this acceptance must be revisited before the change ships.

**Last reviewed:** 2026-06-10.

**Reference:** `.github/workflows/security-audit.yml` ignore list; PLAN.md §Dropped findings (RUSTSEC-2023-0071).

---

## RUSTSEC-2026-0097 — rand 0.8.x unsoundness with a custom global logger

**Status:** Accepted — trigger condition not present in this workload.

**Rationale:** The advisory affects `rand 0.8.x` and describes an unsoundness that can only be triggered by a user-installed custom global logger that itself calls into `rand::rng()`. This advisory enters the dependency graph transitively through `tokio-websockets` (via `async-nats`), `tera`, and `sqlx-postgres`. The orbflow server workload does not install a custom global logger, so the unsound path cannot be triggered. This advisory was pre-existing on the main branch before this security batch and is not introduced by any batch change.

**Re-evaluation condition:** If `rand` is upgraded to 0.9 or later in the dependency tree (resolving the advisory), or if a custom global logger is introduced anywhere in the workspace, this entry must be revisited before the change ships.

**Last reviewed:** 2026-06-10.

**Reference:** `.github/workflows/security-audit.yml` ignore list; confirmed pre-existing on main HEAD by integration-checker Cycle 1.

---

## T-08 — Zeroizing wrap scope: decrypt buffer only, not Credential.data

**Status:** Partial mitigation in place; full fix deferred as T-14 (P2, requires human approval).

**Rationale:** T-08 wraps the AES-256-GCM decrypt output buffer in `Zeroizing<Vec<u8>>`, which zeroes the JSON-serialized credential bytes when the buffer is dropped. This zeroizes the decrypt buffer only. The deserialized `serde_json::Value` HashMap in `Credential.data` and the AES key material are NOT zeroized — they propagate to the worker heap. Full secret-path zeroization requires a `SecretString` newtype threaded through the `CredentialStore` port trait, tracked as T-14 (P2, requires human approval as a breaking port-trait change).

**Re-evaluation condition:** When T-14 (SecretString/SecretMap newtype, full secret-path zeroization) is scheduled, this entry must be updated to reflect the new scope. Do not describe the existing wrap as "credentials are zeroized" in documentation, PR descriptions, or changelogs.

**Last reviewed:** 2026-06-10.

**Reference:** PLAN.md T-08, T-14; Risk register #12.

---

## Deferred items table

The following tasks were scoped, triaged, and explicitly deferred from the current batch. They are tracked here so they are not silently dropped between planning cycles.

| Task | Title | Blocked on | Target |
|------|-------|-----------|--------|
| T-04 | gRPC per-RPC RBAC and owner-scoped dispatch (authorization parity with HTTP API) | OQ-1: human decision on loopback-bind vs per-user principal derivation | Follow-on PR after T-01 ships |
| T-12 | Strict enforcing Content-Security-Policy with nonce middleware | Report-Only CSP monitoring period (T-05 shipped); App-Router nonce middleware does not yet exist | P2; separate PR after T-05 |
| T-13 | Per-IP anonymous rate-limit with `ConnectInfo` | OQ-4: trusted-proxy decision (XFF spoofing risk without explicit `trusted_proxy_hops` config) | P2; implementation blocked until OQ-4 is answered |
| T-14 | Full secret-path zeroization via `SecretString`/`SecretMap` newtype | Port-trait breaking change; requires human approval before implementation | P2; follow-on design item |
