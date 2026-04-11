// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Configuration loading and logger setup.
//!
//! Provides [`Config`] for loading YAML configuration with environment
//! variable expansion, and [`init_tracing`] / [`init_tracing_with_config`]
//! for setting up the tracing subscriber.

pub mod config;
pub mod logger;

pub use config::{
    Config, ConfigError, CredentialConfig, DatabaseConfig, GrpcConfig, LogConfig, McpConfig,
    NatsConfig, OtelConfig, PluginConfig, RateLimitConfig, ServerConfig, WorkerConfig,
};
pub use logger::{OtelGuard, init_tracing, init_tracing_with_config, init_tracing_with_otel};
