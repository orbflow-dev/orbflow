// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! gRPC API surface for Orbflow.
//!
//! Provides a JSON-RPC server over TCP that mirrors the Go gRPC API.
//! The server exposes workflow lifecycle methods: CreateWorkflow, GetWorkflow,
//! ListWorkflows, StartWorkflow, GetInstance, CancelInstance.

mod server;
mod types;

pub use server::GrpcServer;
