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
use std::sync::Arc;

use orbflow_core::OrbflowError;
use orbflow_core::credential_proxy::{CapabilityRequest, CapabilityResponse};
use orbflow_core::ports::CredentialStore;

/// Executes capability requests by injecting credentials into HTTP calls.
///
/// The proxy ensures that plugins and MCP servers never see raw API keys.
/// Credential data is fetched from the [`CredentialStore`], injected into the
/// outgoing HTTP request as an `Authorization` header, and the response is
/// sanitized before being returned to the caller.
pub struct CredentialProxy {
    cred_store: Arc<dyn CredentialStore>,
}

impl CredentialProxy {
    /// Creates a new proxy backed by the given credential store.
    pub fn new(cred_store: Arc<dyn CredentialStore>) -> Self {
        Self { cred_store }
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
        let (resolved_ip, parsed_url) = validate_proxy_url(&req.url).await?;

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
        let mut client_builder = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(30));

        if let Some(ip) = resolved_ip
            && let Some(host) = parsed_url.host_str()
        {
            client_builder = client_builder.resolve(
                host,
                std::net::SocketAddr::new(ip, parsed_url.port_or_known_default().unwrap_or(443)),
            );
        }

        let client = client_builder
            .build()
            .map_err(|e| OrbflowError::Internal(format!("failed to build HTTP client: {e}")))?;

        let method =
            reqwest::Method::from_bytes(req.method.as_bytes()).unwrap_or(reqwest::Method::GET);
        let mut http_req = client.request(method, parsed_url);

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
async fn validate_proxy_url(
    url_str: &str,
) -> Result<(Option<std::net::IpAddr>, url::Url), OrbflowError> {
    let parsed = url::Url::parse(url_str)
        .map_err(|_| OrbflowError::InvalidNodeConfig(format!("invalid URL: {url_str}")))?;

    // Must be HTTPS (except localhost for dev)
    let is_localhost_dev =
        parsed.host_str() == Some("localhost") || parsed.host_str() == Some("127.0.0.1");
    if parsed.scheme() != "https" && !is_localhost_dev {
        return Err(OrbflowError::InvalidNodeConfig(
            "credential proxy only allows HTTPS URLs (or localhost for development)".into(),
        ));
    }

    let host_str = parsed
        .host_str()
        .ok_or_else(|| OrbflowError::InvalidNodeConfig("URL must have a host".into()))?;

    // Check known blocked hostnames immediately
    let lower_host = host_str.to_lowercase();
    if orbflow_core::ssrf::BLOCKED_HOSTNAMES.contains(&lower_host.as_str()) {
        return Err(OrbflowError::InvalidNodeConfig(
            "credential proxy blocked request to internal address".into(),
        ));
    }

    // Resolve host
    let port = parsed.port_or_known_default().unwrap_or(443);
    let addrs: Vec<std::net::SocketAddr> =
        tokio::net::lookup_host(format!("{}:{}", host_str, port))
            .await
            .map_err(|e| OrbflowError::InvalidNodeConfig(format!("DNS resolution failed: {e}")))?
            .collect();

    if addrs.is_empty() {
        return Err(OrbflowError::InvalidNodeConfig(
            "DNS resolution returned no addresses".into(),
        ));
    }

    // Validate each IP and select the first safe one
    let mut safe_ip = None;
    for addr in addrs {
        let ip = addr.ip();
        // Allow localhost loopback in development, but it shouldn't hit production
        if let Some(reason) = orbflow_core::ssrf::is_private_ip(&ip, is_localhost_dev) {
            return Err(OrbflowError::InvalidNodeConfig(format!(
                "credential proxy blocked request to {reason}: {ip}"
            )));
        }
        if safe_ip.is_none() {
            safe_ip = Some(ip);
        }
    }

    Ok((safe_ip, parsed))
}
