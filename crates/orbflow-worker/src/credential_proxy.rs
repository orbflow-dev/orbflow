// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Credential proxy — executes HTTP requests on behalf of plugins
//! by injecting credentials the plugin never sees.
//!
//! When a plugin needs to call an authenticated API it sends a
//! [`CapabilityRequest`] instead of receiving raw credentials. The
//! [`CredentialProxy`] fetches the credential from the store, injects
//! the appropriate authentication header, executes the HTTP request,
//! and returns a sanitized [`CapabilityResponse`].

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;

use reqwest::dns::{Addrs, Name, Resolve, Resolving};

use reqwest::Url;

use orbflow_core::OrbflowError;
use orbflow_core::credential_proxy::{CapabilityRequest, CapabilityResponse};
use orbflow_core::ports::CredentialStore;
use orbflow_core::ssrf::{BLOCKED_HOSTNAMES, is_private_ip};

/// Custom DNS resolver that validates each resolved IP against [`is_private_ip`]
/// before allowing the connection. Localhost is allowed for development.
struct ProxySsrfSafeResolver;

impl Resolve for ProxySsrfSafeResolver {
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

/// Executes capability requests by injecting credentials into HTTP calls.
///
/// The proxy ensures that plugins and MCP servers never see raw API keys.
/// Credential data is fetched from the [`CredentialStore`], injected into the
/// outgoing HTTP request as an `Authorization` header, and the response is
/// sanitized before being returned to the caller.
pub struct CredentialProxy {
    cred_store: Arc<dyn CredentialStore>,
    http_client: reqwest::Client,
}

impl CredentialProxy {
    /// Creates a new proxy backed by the given credential store.
    pub fn new(cred_store: Arc<dyn CredentialStore>) -> Self {
        let http_client = reqwest::Client::builder()
            .dns_resolver(Arc::new(ProxySsrfSafeResolver))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("failed to build credential proxy HTTP client");

        Self {
            cred_store,
            http_client,
        }
    }

    /// Handle a capability request from a plugin/MCP server.
    ///
    /// 1. Validates the URL against SSRF blocklists.
    /// 2. Fetches the credential from the store.
    /// 3. Validates the request against the credential's domain policy.
    /// 4. Builds an HTTP request with injected authentication.
    /// 5. Executes the request and returns a sanitized response.
    pub async fn handle(
        &self,
        req: &CapabilityRequest,
    ) -> Result<CapabilityResponse, OrbflowError> {
        // 0. Validate URL against SSRF blocklists
        validate_proxy_url(&req.url)?;

        // 1. Fetch the credential
        let cred = self.cred_store.get_credential(&req.credential_id).await?;

        // 2. Check domain allowlist
        if let Some(ref policy) = cred.policy
            && !policy.is_domain_allowed(&req.url)
        {
            return Ok(CapabilityResponse {
                status_code: 403,
                headers: HashMap::new(),
                body: None,
                error: Some(format!(
                    "domain not allowed for credential '{}': {}",
                    cred.name, req.url
                )),
            });
        }

        // 3. Build HTTP request with injected credentials
        let method =
            reqwest::Method::from_bytes(req.method.as_bytes()).unwrap_or(reqwest::Method::GET);
        let mut http_req = self.http_client.request(method, &req.url);

        // Add plugin-provided headers
        for (k, v) in &req.headers {
            http_req = http_req.header(k, v);
        }

        // Inject credential as Authorization header.
        // Supports common patterns: api_key, token, bearer, username+password.
        if let Some(key) = cred.data.get("api_key").and_then(|v| v.as_str()) {
            http_req = http_req.header("Authorization", format!("Bearer {key}"));
        } else if let Some(token) = cred.data.get("token").and_then(|v| v.as_str()) {
            http_req = http_req.header("Authorization", format!("Bearer {token}"));
        } else if let Some(user) = cred.data.get("username").and_then(|v| v.as_str()) {
            let pass = cred
                .data
                .get("password")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            http_req = http_req.basic_auth(user, Some(pass));
        }

        // Add body if present
        if let Some(ref body) = req.body {
            http_req = http_req.json(body);
        }

        // 4. Execute the request
        let response = http_req
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| OrbflowError::Internal(format!("credential proxy HTTP error: {e}")))?;

        let status_code = response.status().as_u16();

        // Collect response headers (sanitized — strip auth-related headers)
        let resp_headers: HashMap<String, String> = response
            .headers()
            .iter()
            .filter(|(k, _)| {
                let name = k.as_str().to_lowercase();
                name != "authorization" && name != "set-cookie" && !name.starts_with("x-api-key")
            })
            .filter_map(|(k, v)| v.to_str().ok().map(|v| (k.to_string(), v.to_string())))
            .collect();

        // Parse response body as JSON, fall back to string
        let body_bytes = response.bytes().await.unwrap_or_default();
        let body = serde_json::from_slice(&body_bytes).ok().or_else(|| {
            String::from_utf8(body_bytes.to_vec())
                .ok()
                .map(serde_json::Value::String)
        });

        // 5. Log usage (never log the credential itself)
        tracing::info!(
            credential_id = %req.credential_id,
            url = %req.url,
            method = %req.method,
            status = status_code,
            "credential proxy: request completed"
        );

        Ok(CapabilityResponse {
            status_code,
            headers: resp_headers,
            body,
            error: None,
        })
    }
}

/// Validates a proxy URL to prevent SSRF attacks.
///
/// Enforces HTTPS (except localhost for development) and blocks
/// known cloud metadata endpoints.
fn validate_proxy_url(url_str: &str) -> Result<(), OrbflowError> {
    let parsed = Url::parse(url_str)
        .map_err(|_| OrbflowError::InvalidNodeConfig(format!("invalid proxy URL: {url_str}")))?;

    let is_localhost =
        url_str.starts_with("http://localhost") || url_str.starts_with("http://127.0.0.1");

    if !is_localhost && parsed.scheme() != "https" {
        return Err(OrbflowError::InvalidNodeConfig(
            "credential proxy only allows HTTPS URLs (or localhost for development)".into(),
        ));
    }

    let host = parsed.host_str().filter(|h| !h.is_empty()).ok_or_else(|| {
        OrbflowError::InvalidNodeConfig(format!("proxy URL has no host: {url_str}"))
    })?;

    let lower = host.to_lowercase();
    if BLOCKED_HOSTNAMES.contains(&lower.as_str()) {
        return Err(OrbflowError::InvalidNodeConfig(
            "credential proxy blocked request to internal address".into(),
        ));
    }

    let ip = match parsed.host() {
        Some(url::Host::Ipv4(v4)) => Some(IpAddr::V4(v4)),
        Some(url::Host::Ipv6(v6)) => Some(IpAddr::V6(v6)),
        _ => None,
    };

    if let Some(ip) = ip {
        if let Some(reason) = is_private_ip(&ip, true) {
            return Err(OrbflowError::InvalidNodeConfig(format!(
                "credential proxy URL points to {reason}: {host}"
            )));
        }
    }

    Ok(())
}
