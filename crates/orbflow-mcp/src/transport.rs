// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! MCP transport layer — HTTP+SSE for remote MCP servers.

use std::net::IpAddr;
use std::sync::{Arc, OnceLock};

use crate::schema::{JsonRpcRequest, JsonRpcResponse};
use orbflow_core::OrbflowError;
use orbflow_core::ssrf::{ALLOWED_SCHEMES, BLOCKED_HOSTNAMES, is_private_ip};
use reqwest::dns::{Addrs, Name, Resolve, Resolving};

/// Custom DNS resolver that validates each resolved IP against [`is_private_ip`]
/// before allowing the connection. Localhost is allowed for MCP dev servers.
struct McpSsrfSafeResolver;

impl Resolve for McpSsrfSafeResolver {
    fn resolve(&self, name: Name) -> Resolving {
        Box::pin(async move {
            let addrs: Vec<std::net::SocketAddr> =
                tokio::net::lookup_host(format!("{}:0", name.as_str()))
                    .await
                    .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?
                    .collect();
            let validated: Vec<std::net::SocketAddr> = addrs
                .into_iter()
                .filter(|a| is_private_ip(&a.ip(), true).is_none())
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
fn shared_mcp_client() -> Result<&'static reqwest::Client, OrbflowError> {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    if let Some(client) = CLIENT.get() {
        return Ok(client);
    }
    let client = reqwest::Client::builder()
        .dns_resolver(Arc::new(McpSsrfSafeResolver))
        .timeout(std::time::Duration::from_secs(60))
        .connect_timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| {
            OrbflowError::InvalidNodeConfig(format!("failed to build MCP HTTP client: {e}"))
        })?;
    Ok(CLIENT.get_or_init(|| client))
}

/// Validates that `url_str` does not point to a private, link-local, or
/// cloud-metadata address. Localhost is allowed for MCP dev servers.
///
/// This is a sync, defense-in-depth guard. The full async DNS rebinding
/// check is performed by `McpToolNode` via `orbflow-builtins::ssrf` before
/// constructing the transport.
fn validate_mcp_url(url_str: &str) -> Result<(), OrbflowError> {
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

    // Check literal IP addresses against private/link-local ranges.
    // Allow loopback (localhost) for MCP dev servers.
    let ip = match parsed.host() {
        Some(url::Host::Ipv4(v4)) => Some(IpAddr::V4(v4)),
        Some(url::Host::Ipv6(v6)) => Some(IpAddr::V6(v6)),
        _ => None,
    };

    if let Some(ip) = ip
        && let Some(reason) = is_private_ip(&ip, true)
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
    /// Localhost is allowed since MCP dev servers commonly run locally.
    pub fn new(base_url: impl Into<String>) -> Result<Self, OrbflowError> {
        let base_url = base_url.into();

        // SSRF guard: validate the URL does not point to private/internal
        // addresses. Allow localhost since MCP dev servers commonly run locally.
        // The async variant (with DNS rebinding defense) is called by McpToolNode
        // before constructing the transport; this sync check provides defense-in-depth.
        validate_mcp_url(&base_url)?;

        if !base_url.starts_with("https://")
            && !base_url.starts_with("http://localhost")
            && !base_url.starts_with("http://127.0.0.1")
        {
            tracing::warn!(
                url = %base_url,
                "MCP server URL is not HTTPS — consider using HTTPS in production"
            );
        }
        let client = shared_mcp_client()?.clone();
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
