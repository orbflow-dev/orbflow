// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! MCP transport layer — HTTP+SSE for remote MCP servers.

use std::net::IpAddr;
use std::sync::{Arc, OnceLock};

use crate::schema::{JsonRpcRequest, JsonRpcResponse};
use orbflow_core::OrbflowError;
use orbflow_core::ssrf::{ALLOWED_SCHEMES, BLOCKED_HOSTNAMES, is_private_ip};
use reqwest::dns::{Addrs, Name, Resolve, Resolving};

/// Policy for loopback MCP endpoints.
///
/// Production paths should use [`McpLocalhostPolicy::Deny`]. Local development
/// can opt in with [`McpLocalhostPolicy::AllowForDev`], which permits localhost
/// and literal loopback URLs while continuing to block private, link-local, and
/// metadata endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpLocalhostPolicy {
    Deny,
    AllowForDev,
}

impl McpLocalhostPolicy {
    pub fn allow_localhost(self) -> bool {
        matches!(self, Self::AllowForDev)
    }
}

/// Custom DNS resolver that validates each resolved IP against [`is_private_ip`]
/// before allowing the connection.
struct McpSsrfSafeResolver {
    localhost_policy: McpLocalhostPolicy,
}

impl Resolve for McpSsrfSafeResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let allow_localhost = self.localhost_policy.allow_localhost()
            && name
                .as_str()
                .trim_end_matches('.')
                .eq_ignore_ascii_case("localhost");
        Box::pin(async move {
            let addrs: Vec<std::net::SocketAddr> =
                tokio::net::lookup_host(format!("{}:0", name.as_str()))
                    .await
                    .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?
                    .collect();
            let validated: Vec<std::net::SocketAddr> = addrs
                .into_iter()
                .filter(|a| is_private_ip(&a.ip(), allow_localhost).is_none())
                .collect();
            if validated.is_empty() {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "all resolved addresses are private/internal (SSRF protection)",
                ))
                    as Box<dyn std::error::Error + Send + Sync>);
            }
            Ok(Box::new(validated.into_iter()) as Addrs)
        })
    }
}

/// Returns a shared reqwest client with SSRF-safe DNS resolver and sensible timeouts.
fn shared_mcp_client(
    localhost_policy: McpLocalhostPolicy,
) -> Result<&'static reqwest::Client, OrbflowError> {
    static REMOTE_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    static LOCAL_DEV_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

    let cell = if localhost_policy.allow_localhost() {
        &LOCAL_DEV_CLIENT
    } else {
        &REMOTE_CLIENT
    };

    if let Some(client) = cell.get() {
        return Ok(client);
    }

    let client = reqwest::Client::builder()
        .dns_resolver(Arc::new(McpSsrfSafeResolver { localhost_policy }))
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(60))
        .connect_timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| {
            OrbflowError::InvalidNodeConfig(format!("failed to build MCP HTTP client: {e}"))
        })?;
    Ok(cell.get_or_init(|| client))
}

/// Validates that `url_str` does not point to a private, link-local, or
/// cloud-metadata address.
///
/// This is a sync, defense-in-depth guard. The full async DNS rebinding
/// check is performed by `McpToolNode` via `orbflow-builtins::ssrf` before
/// constructing the transport.
fn validate_mcp_url(
    url_str: &str,
    localhost_policy: McpLocalhostPolicy,
) -> Result<(), OrbflowError> {
    let parsed = url::Url::parse(url_str)
        .map_err(|_| OrbflowError::InvalidNodeConfig(format!("invalid MCP URL: {url_str}")))?;

    if !ALLOWED_SCHEMES.contains(&parsed.scheme()) {
        return Err(OrbflowError::InvalidNodeConfig(format!(
            "MCP URL scheme '{}' is not allowed (only http and https)",
            parsed.scheme()
        )));
    }

    let host = parsed.host_str().filter(|h| !h.is_empty()).ok_or_else(|| {
        OrbflowError::InvalidNodeConfig(format!("MCP URL has no host: {url_str}"))
    })?;

    let lower = host.to_lowercase();
    if BLOCKED_HOSTNAMES.contains(&lower.as_str()) {
        return Err(OrbflowError::InvalidNodeConfig(
            "MCP server URL points to cloud metadata endpoint".into(),
        ));
    }

    let ip = match parsed.host() {
        Some(url::Host::Ipv4(v4)) => Some(IpAddr::V4(v4)),
        Some(url::Host::Ipv6(v6)) => Some(IpAddr::V6(v6)),
        _ => None,
    };

    let is_loopback_target = lower == "localhost" || ip.is_some_and(|addr| addr.is_loopback());
    if is_loopback_target && !localhost_policy.allow_localhost() {
        return Err(OrbflowError::InvalidNodeConfig(
            "MCP server URL points to localhost; enable MCP localhost dev mode to allow it".into(),
        ));
    }

    if parsed.scheme() == "http" && !(localhost_policy.allow_localhost() && is_loopback_target) {
        return Err(OrbflowError::InvalidNodeConfig(
            "MCP server URL must use HTTPS unless MCP localhost dev mode is enabled".into(),
        ));
    }

    if let Some(ip) = ip
        && let Some(reason) = is_private_ip(&ip, localhost_policy.allow_localhost())
    {
        return Err(OrbflowError::InvalidNodeConfig(format!(
            "MCP server URL points to {reason}: {host}"
        )));
    }

    Ok(())
}

/// HTTP transport for communicating with remote MCP servers.
pub struct HttpTransport {
    client: reqwest::Client,
    base_url: String,
}

impl HttpTransport {
    /// Creates a new HTTP transport for the given MCP server URL.
    ///
    /// Returns an error if the URL points to a known-dangerous internal address.
    /// Localhost is denied by default; use
    /// [`HttpTransport::new_with_localhost_policy`] for explicit local dev mode.
    pub fn new(base_url: impl Into<String>) -> Result<Self, OrbflowError> {
        Self::new_with_localhost_policy(base_url, McpLocalhostPolicy::Deny)
    }

    /// Creates a new HTTP transport with an explicit localhost policy.
    pub fn new_with_localhost_policy(
        base_url: impl Into<String>,
        localhost_policy: McpLocalhostPolicy,
    ) -> Result<Self, OrbflowError> {
        let base_url = base_url.into();

        validate_mcp_url(&base_url, localhost_policy)?;

        if !base_url.starts_with("https://") {
            tracing::warn!(
                url = %base_url,
                "MCP server URL is not HTTPS; this is only allowed for explicit localhost dev mode"
            );
        }

        let client = shared_mcp_client(localhost_policy)?.clone();
        Ok(Self { client, base_url })
    }

    /// Send a JSON-RPC request and receive the response.
    pub async fn send(&self, request: &JsonRpcRequest) -> Result<JsonRpcResponse, OrbflowError> {
        let resp = self
            .client
            .post(&self.base_url)
            .json(request)
            .timeout(std::time::Duration::from_secs(60))
            .send()
            .await
            .map_err(|e| OrbflowError::Internal(format!("MCP transport error: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            const MAX_ERROR_BODY: usize = 512;
            let body_truncated = if body.len() > MAX_ERROR_BODY {
                format!("{}... (truncated)", &body[..MAX_ERROR_BODY])
            } else {
                body
            };
            tracing::error!(
                status = %status,
                body = %body_truncated,
                "MCP server returned error response"
            );
            return Err(OrbflowError::Internal("MCP server request failed".into()));
        }

        resp.json::<JsonRpcResponse>()
            .await
            .map_err(|e| OrbflowError::Internal(format!("MCP response parse error: {e}")))
    }
}
