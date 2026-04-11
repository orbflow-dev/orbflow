// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Configuration loading with YAML parsing and environment variable expansion.

use std::path::Path;

use serde::Deserialize;

/// Top-level configuration, matching the Go `config.Config`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub server: ServerConfig,
    pub grpc: GrpcConfig,
    pub worker: WorkerConfig,
    pub database: DatabaseConfig,
    pub nats: NatsConfig,
    pub plugins: PluginConfig,
    pub credentials: CredentialConfig,
    pub mcp: McpConfig,
    pub log: LogConfig,
    pub otel: OtelConfig,
}

// Default is derived: all fields implement Default.

/// HTTP API server configuration.
///
/// `Debug` is manually implemented to redact the `auth_token` field.
#[derive(Clone, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    /// Optional bearer token required for authenticated API endpoints.
    ///
    /// When set, all routes except `/health`, `/node-types`, and
    /// `/credential-types` paths must include `Authorization: Bearer <token>`.
    /// When absent or empty, authentication is disabled.
    pub auth_token: Option<String>,
    /// Allowed CORS origins.
    ///
    /// When empty, all cross-origin requests are denied (safe default).
    /// Set to `["*"]` for development or specific origins for production.
    #[serde(default)]
    pub cors_origins: Vec<String>,
    /// Per-user rate limit configuration for tiered API endpoints.
    #[serde(default)]
    pub rate_limit: RateLimitConfig,
}

impl std::fmt::Debug for ServerConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerConfig")
            .field("host", &self.host)
            .field("port", &self.port)
            .field(
                "auth_token",
                if self.auth_token.is_some() {
                    &"<redacted>"
                } else {
                    &"None"
                },
            )
            .field("cors_origins", &self.cors_origins)
            .field("rate_limit", &self.rate_limit)
            .finish()
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".into(),
            port: 8080,
            auth_token: None,
            cors_origins: vec![],
            rate_limit: RateLimitConfig::default(),
        }
    }
}

/// Per-user rate limit configuration for tiered API endpoints.
///
/// Defaults match the original hardcoded values:
/// - read: 1 request per 300ms, burst 200
/// - write: 1 request per 600ms, burst 100
/// - sensitive: 2 per second, burst 30
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct RateLimitConfig {
    pub read_per_ms: u64,
    pub read_burst: u32,
    pub write_per_ms: u64,
    pub write_burst: u32,
    pub sensitive_per_sec: u64,
    pub sensitive_burst: u32,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            read_per_ms: 300,
            read_burst: 200,
            write_per_ms: 600,
            write_burst: 100,
            sensitive_per_sec: 2,
            sensitive_burst: 30,
        }
    }
}

/// Optional gRPC server configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct GrpcConfig {
    pub enabled: bool,
    pub port: u16,
}

impl Default for GrpcConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            port: 9090,
        }
    }
}

/// Worker process configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct WorkerConfig {
    pub pool: String,
    pub concurrency: usize,
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            pool: "default".into(),
            concurrency: 4,
        }
    }
}

/// PostgreSQL connection configuration.
///
/// `Debug` is manually implemented to redact the DSN (which contains credentials).
#[derive(Clone, Default, Deserialize)]
#[serde(default)]
pub struct DatabaseConfig {
    pub dsn: String,
}

impl std::fmt::Debug for DatabaseConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DatabaseConfig")
            .field(
                "dsn",
                if self.dsn.is_empty() {
                    &"<empty>"
                } else {
                    &"<redacted>"
                },
            )
            .finish()
    }
}

// Default is derived: String::default() == "".

/// NATS connection configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct NatsConfig {
    pub url: String,
    pub embedded: bool,
    #[serde(default = "default_nats_data_dir")]
    pub data_dir: String,
}

/// Returns a cross-platform default NATS data directory using the OS temp dir.
fn default_nats_data_dir() -> String {
    let mut path = std::env::temp_dir();
    path.push("orbflow-nats");
    path.to_string_lossy().into_owned()
}

impl Default for NatsConfig {
    fn default() -> Self {
        Self {
            url: "nats://127.0.0.1:4222".into(),
            embedded: true,
            data_dir: default_nats_data_dir(),
        }
    }
}

/// External plugin loading configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct PluginConfig {
    /// Directory for legacy subprocess plugins.
    pub dir: String,
    /// gRPC plugin endpoints (persistent connections).
    #[serde(default)]
    pub grpc: Vec<GrpcPluginConfig>,
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            dir: "./plugins".into(),
            grpc: Vec::new(),
        }
    }
}

/// A single gRPC plugin endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct GrpcPluginConfig {
    /// Human-readable name for logging and identification.
    pub name: String,
    /// gRPC address (e.g. `http://localhost:50051`).
    ///
    /// **Security**: When TLS is not enabled, this should be a loopback address
    /// (`http://localhost:*` or `http://127.0.0.1:*`). Plugin traffic may include
    /// credentials passed via capability nodes.
    pub address: String,
    /// RPC timeout in seconds (default: 30).
    #[serde(default = "default_rpc_timeout")]
    pub timeout_secs: u64,
}

fn default_rpc_timeout() -> u64 {
    30
}

/// Credential store configuration.
///
/// `Debug` is manually implemented to redact the encryption key.
#[derive(Clone, Default, Deserialize)]
#[serde(default)]
pub struct CredentialConfig {
    pub encryption_key: String,
}

impl std::fmt::Debug for CredentialConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CredentialConfig")
            .field(
                "encryption_key",
                if self.encryption_key.is_empty() {
                    &"<empty>"
                } else {
                    &"<redacted>"
                },
            )
            .finish()
    }
}

// Default is derived: String::default() == "".

/// MCP server configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct McpConfig {
    /// Enable MCP server (default: false).
    pub enabled: bool,
    /// Transport: "http" (default).
    pub transport: String,
    /// Port for HTTP transport (default: 3001).
    pub port: u16,
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            transport: "http".into(),
            port: 3001,
        }
    }
}

/// Logging configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct LogConfig {
    pub level: String,
    pub format: String,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: "info".into(),
            format: "json".into(),
        }
    }
}

/// OpenTelemetry observability configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct OtelConfig {
    /// Enable OpenTelemetry export (default: false).
    pub enabled: bool,
    /// OTLP exporter endpoint (default: "http://localhost:4317").
    pub endpoint: String,
    /// Service name reported in traces and metrics (default: "orbflow").
    pub service_name: String,
    /// Trace sampling rate between 0.0 and 1.0 (default: 1.0 = sample everything).
    pub sample_rate: f64,
}

impl Default for OtelConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: "http://localhost:4317".into(),
            service_name: "orbflow".into(),
            sample_rate: 1.0,
        }
    }
}

impl Config {
    /// Loads a config from the given YAML file path.
    ///
    /// Environment variables in the YAML are expanded before parsing:
    /// - `${VAR}` and `$VAR` patterns are replaced with their environment values.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let data = std::fs::read_to_string(path)
            .map_err(|e| ConfigError::Io(format!("read {}: {e}", path.display())))?;

        Self::from_str(&data)
    }

    /// Parses a config from a YAML string with environment variable expansion.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(yaml: &str) -> Result<Self, ConfigError> {
        let expanded = expand_env(yaml).map_err(ConfigError::Validation)?;

        let cfg: Config = serde_yaml::from_str(&expanded)
            .map_err(|e| ConfigError::Parse(format!("parse config: {e}")))?;

        cfg.validate()?;
        Ok(cfg)
    }

    /// Validates the configuration.
    fn validate(&self) -> Result<(), ConfigError> {
        if self.server.port == 0 {
            return Err(ConfigError::Validation(
                "server.port must be non-zero".into(),
            ));
        }

        if self.grpc.enabled && self.grpc.port == 0 {
            return Err(ConfigError::Validation(
                "grpc.port must be non-zero when grpc is enabled".into(),
            ));
        }

        match self.log.format.as_str() {
            "" | "json" | "console" => {}
            other => {
                return Err(ConfigError::Validation(format!(
                    "log.format must be one of: json, console (got: {other})"
                )));
            }
        }

        if self.otel.enabled && !(0.0..=1.0).contains(&self.otel.sample_rate) {
            return Err(ConfigError::Validation(format!(
                "otel.sample_rate must be between 0.0 and 1.0 (got: {})",
                self.otel.sample_rate
            )));
        }

        Ok(())
    }
}

/// Errors from configuration loading.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("config: I/O error: {0}")]
    Io(String),
    #[error("config: parse error: {0}")]
    Parse(String),
    #[error("config: validation error: {0}")]
    Validation(String),
}

/// Escapes a value for safe embedding in YAML.
///
/// Values that are "YAML-safe" (only alphanumeric, hyphens, underscores,
/// dots, forward slashes, and `+`) are returned unquoted to avoid double-
/// quoting when the template already uses YAML quotes like `"${VAR}"`.
///
/// Values containing YAML structural characters (`:`, `#`, `{`, `[`,
/// newlines, single quotes, spaces, etc.) are wrapped in single quotes
/// with internal `'` escaped as `''`.
fn yaml_escape_value(value: &str) -> String {
    let needs_quoting = value.is_empty()
        || value.bytes().any(|b| {
            !b.is_ascii_alphanumeric()
                && b != b'-'
                && b != b'_'
                && b != b'.'
                && b != b'/'
                && b != b'+'
        });
    if needs_quoting {
        format!("'{}'", value.replace('\'', "''"))
    } else {
        value.to_owned()
    }
}

/// Expands environment variables in a string.
///
/// Supports `${VAR}` and `$VAR` patterns. Unknown variables are replaced
/// with empty strings. Expanded values are wrapped in YAML single quotes
/// to prevent injection of YAML structural characters.
fn expand_env(input: &str) -> Result<String, String> {
    let mut result = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '$' && i + 1 < chars.len() {
            if chars[i + 1] == '{' {
                // ${VAR} syntax
                if let Some(end) = chars[i + 2..].iter().position(|&c| c == '}') {
                    let var_name: String = chars[i + 2..i + 2 + end].iter().collect();
                    let value = std::env::var(&var_name).unwrap_or_default();
                    // Defense-in-depth: null bytes are never valid in any context.
                    if value.contains('\0') {
                        return Err(format!(
                            "environment variable '{var_name}' contains null bytes"
                        ));
                    }
                    result.push_str(&yaml_escape_value(&value));
                    i = i + 3 + end;
                    continue;
                }
            } else if chars[i + 1].is_ascii_alphabetic() || chars[i + 1] == '_' {
                // $VAR syntax
                let start = i + 1;
                let mut end = start;
                while end < chars.len() && (chars[end].is_ascii_alphanumeric() || chars[end] == '_')
                {
                    end += 1;
                }
                let var_name: String = chars[start..end].iter().collect();
                let value = std::env::var(&var_name).unwrap_or_default();
                // Defense-in-depth: null bytes are never valid in any context.
                if value.contains('\0') {
                    return Err(format!(
                        "environment variable '{var_name}' contains null bytes"
                    ));
                }
                result.push_str(&yaml_escape_value(&value));
                i = end;
                continue;
            }
        }
        result.push(chars[i]);
        i += 1;
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = Config::default();
        assert_eq!(cfg.server.host, "0.0.0.0");
        assert_eq!(cfg.server.port, 8080);
        assert_eq!(cfg.grpc.port, 9090);
        assert!(!cfg.grpc.enabled);
        assert_eq!(cfg.worker.pool, "default");
        assert_eq!(cfg.nats.url, "nats://127.0.0.1:4222");
        assert_eq!(cfg.nats.data_dir, default_nats_data_dir());
        assert_eq!(cfg.log.level, "info");
        assert_eq!(cfg.log.format, "json");
        assert!(cfg.plugins.grpc.is_empty());
        // Rate limit defaults
        assert_eq!(cfg.server.rate_limit.read_per_ms, 300);
        assert_eq!(cfg.server.rate_limit.read_burst, 200);
        assert_eq!(cfg.server.rate_limit.write_per_ms, 600);
        assert_eq!(cfg.server.rate_limit.write_burst, 100);
        assert_eq!(cfg.server.rate_limit.sensitive_per_sec, 2);
        assert_eq!(cfg.server.rate_limit.sensitive_burst, 30);
    }

    #[test]
    fn test_parse_grpc_plugins() {
        let yaml = r#"
plugins:
  dir: ./my-plugins
  grpc:
    - name: sentiment
      address: "http://localhost:50051"
    - name: image
      address: "http://localhost:50052"
"#;
        let cfg = Config::from_str(yaml).unwrap();
        assert_eq!(cfg.plugins.dir, "./my-plugins");
        assert_eq!(cfg.plugins.grpc.len(), 2);
        assert_eq!(cfg.plugins.grpc[0].name, "sentiment");
        assert_eq!(cfg.plugins.grpc[0].address, "http://localhost:50051");
        assert_eq!(cfg.plugins.grpc[1].name, "image");
    }

    #[test]
    fn test_parse_minimal_yaml() {
        let yaml = r#"
server:
  port: 3000
database:
  dsn: "postgres://localhost/orbflow"
"#;
        let cfg = Config::from_str(yaml).unwrap();
        assert_eq!(cfg.server.port, 3000);
        assert_eq!(cfg.database.dsn, "postgres://localhost/orbflow");
        // Defaults should be applied for missing fields.
        assert_eq!(cfg.nats.url, "nats://127.0.0.1:4222");
    }

    #[test]
    fn test_env_expansion() {
        // SAFETY: unique env var name per test to avoid parallel collisions.
        unsafe { std::env::set_var("ORBFLOW_TEST_ENV_EXPANSION_PORT", "9999") };
        let expanded = expand_env("port: ${ORBFLOW_TEST_ENV_EXPANSION_PORT}").unwrap();
        assert_eq!(expanded, "port: 9999");
        unsafe { std::env::remove_var("ORBFLOW_TEST_ENV_EXPANSION_PORT") };
    }

    #[test]
    fn test_env_expansion_dollar() {
        // SAFETY: unique env var name per test to avoid parallel collisions.
        unsafe { std::env::set_var("ORBFLOW_TEST_ENV_EXPANSION_DOLLAR_VAL", "hello") };
        let expanded = expand_env("val: $ORBFLOW_TEST_ENV_EXPANSION_DOLLAR_VAL end").unwrap();
        assert_eq!(expanded, "val: hello end");
        unsafe { std::env::remove_var("ORBFLOW_TEST_ENV_EXPANSION_DOLLAR_VAL") };
    }

    #[test]
    fn test_env_expansion_yaml_escapes_quotes() {
        // Verify that single quotes inside env values are properly escaped.
        unsafe { std::env::set_var("ORBFLOW_TEST_YAML_ESCAPE_QUOTES", "it's a test") };
        let expanded = expand_env("val: ${ORBFLOW_TEST_YAML_ESCAPE_QUOTES}").unwrap();
        assert_eq!(expanded, "val: 'it''s a test'");
        unsafe { std::env::remove_var("ORBFLOW_TEST_YAML_ESCAPE_QUOTES") };
    }

    #[test]
    fn test_env_expansion_yaml_escapes_structural() {
        // Verify that YAML structural characters are safely quoted.
        unsafe {
            std::env::set_var(
                "ORBFLOW_TEST_YAML_ESCAPE_STRUCT",
                "key: value\n  nested: true",
            )
        };
        let expanded = expand_env("val: ${ORBFLOW_TEST_YAML_ESCAPE_STRUCT}").unwrap();
        // The value is wrapped in single quotes, preventing YAML injection.
        assert!(expanded.starts_with("val: '"));
        assert!(expanded.ends_with('\''));
        unsafe { std::env::remove_var("ORBFLOW_TEST_YAML_ESCAPE_STRUCT") };
    }

    #[test]
    fn test_parse_otel_config() {
        let yaml = r#"
otel:
  enabled: true
  endpoint: "http://jaeger:4317"
  service_name: "orbflow-test"
  sample_rate: 0.5
"#;
        let cfg = Config::from_str(yaml).unwrap();
        assert!(cfg.otel.enabled);
        assert_eq!(cfg.otel.endpoint, "http://jaeger:4317");
        assert_eq!(cfg.otel.service_name, "orbflow-test");
        assert!((cfg.otel.sample_rate - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_otel_defaults() {
        let cfg = Config::default();
        assert!(!cfg.otel.enabled);
        assert_eq!(cfg.otel.endpoint, "http://localhost:4317");
        assert_eq!(cfg.otel.service_name, "orbflow");
    }

    #[test]
    fn test_validation_invalid_log_format() {
        let yaml = r#"
log:
  format: "xml"
"#;
        let result = Config::from_str(yaml);
        assert!(result.is_err());
    }
}
