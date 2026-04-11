// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Compiles `proto/plugin.proto` into Rust client stubs via tonic-build.
//!
//! Uses `protox` (pure-Rust protobuf parser) so no external `protoc` binary
//! is required. Only the client side is generated — plugin servers are
//! implemented in Python / Go / etc.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Re-run if the proto file changes.
    println!("cargo:rerun-if-changed=../../proto/plugin.proto");

    // Parse the proto file using protox (pure Rust, no protoc needed).
    let file_descriptors = protox::compile(["plugin.proto"], ["../../proto"])?;

    // Generate client stubs (always) and server stubs (only for tests).
    let build_server = std::env::var("CARGO_FEATURE_TEST_SERVER").is_ok();

    tonic_prost_build::configure()
        .build_server(build_server)
        .build_client(true)
        .compile_fds(file_descriptors)?;

    Ok(())
}
