// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! External plugin loader via subprocess and gRPC protocols.

pub mod grpc_client;
pub mod grpc_proto;
pub mod loader;
pub mod process_manager;
pub mod protocol;

pub use grpc_client::GrpcPluginExecutor;
pub use loader::PluginLoader;
pub use process_manager::{ManagedPlugins, PluginProcessManager};
