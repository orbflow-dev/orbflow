// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Security regression tests for the shipped Docker configuration.
//!
//! T-01 regression net: configs/orbflow.docker.yaml must keep gRPC disabled by
//! default so that the unauthenticated gRPC surface is never accidentally exposed
//! in production Docker deployments.

use std::path::PathBuf;

use orbflow_config::Config;

/// Returns the absolute path to the repo-root configs/orbflow.docker.yaml,
/// resolved relative to this crate's Cargo.toml location.
fn docker_yaml_path() -> PathBuf {
    // CARGO_MANIFEST_DIR is set by cargo to the directory of this crate's
    // Cargo.toml (crates/orbflow-config/).  Two levels up reaches the repo root.
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir)
        .join("..") // crates/
        .join("..") // repo root
        .join("configs")
        .join("orbflow.docker.yaml")
}

/// T-01 regression net — gRPC must be disabled in the shipped Docker config.
///
/// This test loads configs/orbflow.docker.yaml via the production Config::load()
/// path (including env-var expansion) and asserts that grpc.enabled is false.
/// If a future commit accidentally flips `enabled: false` to `enabled: true` in
/// orbflow.docker.yaml, this test will fail — catching the regression before it
/// ships to production where the gRPC port would be exposed unauthenticated.
#[test]
fn shipped_docker_config_keeps_grpc_disabled() {
    // The YAML expands ${DATABASE_URL}, ${NATS_URL}, ${CREDENTIAL_ENCRYPTION_KEY},
    // and ${LOG_LEVEL} to empty strings when unset (unwrap_or_default behavior in
    // expand_env).  Config::validate() does not check DSN/key formats, so the load
    // succeeds without needing dummy env vars.
    let path = docker_yaml_path();
    let cfg = Config::load(&path).unwrap_or_else(|e| {
        panic!(
            "Config::load({}) failed: {e}\n\
             Hint: if expand_env returns Err for an env var, \
             set DATABASE_URL / NATS_URL / CREDENTIAL_ENCRYPTION_KEY / LOG_LEVEL \
             to dummy values in this test.",
            path.display()
        )
    });

    // T-01 regression net: grpc.enabled MUST be false in the shipped docker config.
    assert!(
        !cfg.grpc.enabled,
        "T-01 REGRESSION: grpc.enabled must be false in configs/orbflow.docker.yaml \
         (got true). Exposing the unauthenticated gRPC surface is a security risk. \
         See b7dbec9 for the fix that introduced this guard."
    );
}

/// Document the shipped gRPC port default so any change surfaces in test output.
///
/// This is not a security assertion — it records the expected default so that a
/// port change is visible in CI even when gRPC is still disabled.
#[test]
fn shipped_docker_config_grpc_port_is_default() {
    let path = docker_yaml_path();
    let cfg = Config::load(&path)
        .unwrap_or_else(|e| panic!("Config::load({}) failed: {e}", path.display()));

    assert_eq!(
        cfg.grpc.port, 9090,
        "grpc.port in configs/orbflow.docker.yaml changed from 9090; \
         update this test if intentional."
    );
}
