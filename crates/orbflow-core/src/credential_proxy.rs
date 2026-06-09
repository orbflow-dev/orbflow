// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Credential proxy types for secure credential handling.
//!
//! Three tiers of credential access control how plugins and MCP servers
//! interact with secrets:
//!
//! - **Proxy** (default): The plugin never sees the credential. It sends a
//!   [`CapabilityRequest`] and the worker injects credentials before proxying
//!   the HTTP call.
//! - **ScopedToken**: The plugin receives a short-lived, scope-limited token
//!   (future phase — variant defined for forward compatibility).
//! - **Raw**: The plugin receives the raw credential value. Must be explicitly
//!   opted in per credential.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::credential::CredentialId;
use crate::error::OrbflowError;

// Access tier

/// Determines how a credential is shared with plugins/MCP servers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialAccessTier {
    /// Plugin never sees the credential. Worker proxies HTTP calls on behalf
    /// of the plugin.
    #[default]
    Proxy,
    /// Plugin receives a short-lived, scope-limited token (future).
    ScopedToken,
    /// Plugin receives the raw credential value. Must be explicitly opted in.
    Raw,
}

impl CredentialAccessTier {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Proxy => "proxy",
            Self::ScopedToken => "scoped_token",
            Self::Raw => "raw",
        }
    }

    pub fn requires_explicit_policy(self) -> bool {
        !matches!(self, Self::Proxy)
    }

    pub fn execution_mode(self) -> CredentialExecutionMode {
        match self {
            Self::Proxy => CredentialExecutionMode::Proxy,
            Self::ScopedToken => CredentialExecutionMode::ScopedToken,
            Self::Raw => CredentialExecutionMode::RawSecret,
        }
    }
}

/// Runtime execution path selected for a credential reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialExecutionMode {
    /// Worker/runtime must execute through the credential proxy/capability path.
    Proxy,
    /// Worker/runtime must obtain a short-lived scoped token.
    ScopedToken,
    /// Worker/runtime may receive raw secret fields.
    RawSecret,
}

/// Execution paths supported by the current runtime adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CredentialExecutionSupport {
    pub proxy: bool,
    pub scoped_token: bool,
    pub raw_secret: bool,
}

impl CredentialExecutionSupport {
    pub const FAIL_CLOSED: Self = Self {
        proxy: false,
        scoped_token: false,
        raw_secret: false,
    };

    pub const RAW_SECRET_ONLY: Self = Self {
        proxy: false,
        scoped_token: false,
        raw_secret: true,
    };

    pub fn supports(self, mode: CredentialExecutionMode) -> bool {
        match mode {
            CredentialExecutionMode::Proxy => self.proxy,
            CredentialExecutionMode::ScopedToken => self.scoped_token,
            CredentialExecutionMode::RawSecret => self.raw_secret,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CredentialExecutionError {
    TierNotAllowed {
        tier: CredentialAccessTier,
    },
    UnsupportedExecutionMode {
        tier: CredentialAccessTier,
        mode: CredentialExecutionMode,
    },
}

impl std::fmt::Display for CredentialExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TierNotAllowed { tier } => write!(
                f,
                "credential tier '{}' is not allowed by explicit credential policy",
                tier.as_str()
            ),
            Self::UnsupportedExecutionMode { tier, mode } => write!(
                f,
                "credential tier '{}' requires unsupported execution mode {:?}",
                tier.as_str(),
                mode
            ),
        }
    }
}

impl std::error::Error for CredentialExecutionError {}

impl From<CredentialExecutionError> for OrbflowError {
    fn from(value: CredentialExecutionError) -> Self {
        OrbflowError::InvalidNodeConfig(value.to_string())
    }
}

// Capability request / response

/// A request from a plugin/MCP server to make an authenticated HTTP call
/// without seeing the credential.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityRequest {
    /// Type of capability requested.
    pub request_type: CapabilityRequestType,
    /// Target URL for the HTTP request.
    pub url: String,
    /// HTTP method (GET, POST, PUT, DELETE, etc.).
    #[serde(default = "default_method")]
    pub method: String,
    /// HTTP headers (excluding Authorization — injected by proxy).
    #[serde(default)]
    pub headers: HashMap<String, String>,
    /// Request body (for POST/PUT).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<serde_json::Value>,
    /// Which credential to use for authentication.
    pub credential_id: CredentialId,
}

fn default_method() -> String {
    "GET".into()
}

/// Types of capability requests the proxy can handle.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityRequestType {
    /// Proxied HTTP request with credential injection.
    HttpRequest,
}

/// Response from the credential proxy after executing a capability request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityResponse {
    /// HTTP status code from the upstream service.
    pub status_code: u16,
    /// Response headers (sanitized — no credential data).
    #[serde(default)]
    pub headers: HashMap<String, String>,
    /// Response body.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<serde_json::Value>,
    /// Error message if the proxy call failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// Credential policy

/// Per-credential policy controlling how it can be used via the proxy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialPolicy {
    /// Which tiers are allowed for this credential.
    #[serde(default = "default_allowed_tiers")]
    pub allowed_tiers: Vec<CredentialAccessTier>,
    /// Domain allowlist — only proxy requests to these hosts.
    /// Empty means all domains are allowed.
    #[serde(default)]
    pub allowed_domains: Vec<String>,
    /// Rate limit: max requests per minute. 0 = unlimited.
    #[serde(default)]
    pub rate_limit_per_minute: u32,
}

fn default_allowed_tiers() -> Vec<CredentialAccessTier> {
    vec![CredentialAccessTier::Proxy]
}

impl Default for CredentialPolicy {
    fn default() -> Self {
        Self {
            allowed_tiers: default_allowed_tiers(),
            allowed_domains: Vec::new(),
            rate_limit_per_minute: 0,
        }
    }
}

impl CredentialPolicy {
    /// Returns true when this policy explicitly permits `tier`.
    pub fn allows_tier(&self, tier: CredentialAccessTier) -> bool {
        self.allowed_tiers.contains(&tier)
    }

    /// Checks if a URL is allowed by the domain allowlist.
    ///
    /// When the allowlist is empty every domain is permitted. Otherwise the
    /// host extracted from `url` must exactly match one of the entries or be
    /// a subdomain of one (e.g. `api.example.com` matches `example.com`).
    pub fn is_domain_allowed(&self, url: &str) -> bool {
        if self.allowed_domains.is_empty() {
            return true;
        }

        // Use url::Url for host extraction — handles IPv6 literals,
        // percent-encoding, userinfo, and unusual-but-valid URL forms.
        let host = url::Url::parse(url)
            .ok()
            .and_then(|u| u.host_str().map(str::to_owned))
            .unwrap_or_default();

        if host.is_empty() {
            return false;
        }

        self.allowed_domains
            .iter()
            .any(|d| host == *d || host.ends_with(&format!(".{d}")))
    }
}

/// Resolves the execution mode for a credential tier under explicit runtime support.
///
/// Non-Proxy tiers require an explicit policy entry. Unsupported runtime paths
/// fail closed with a validation error instead of silently redacting or leaking
/// secrets.
pub fn resolve_credential_execution_mode(
    tier: CredentialAccessTier,
    policy: Option<&CredentialPolicy>,
    support: CredentialExecutionSupport,
) -> Result<CredentialExecutionMode, CredentialExecutionError> {
    let policy_allows = policy
        .map(|policy| policy.allows_tier(tier))
        .unwrap_or(!tier.requires_explicit_policy());
    if !policy_allows {
        return Err(CredentialExecutionError::TierNotAllowed { tier });
    }

    let mode = tier.execution_mode();
    if !support.supports(mode) {
        return Err(CredentialExecutionError::UnsupportedExecutionMode { tier, mode });
    }

    Ok(mode)
}

// Tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_access_tier_is_proxy() {
        assert_eq!(CredentialAccessTier::default(), CredentialAccessTier::Proxy);
    }

    #[test]
    fn test_access_tier_serde_roundtrip() {
        let tier = CredentialAccessTier::Raw;
        let json = serde_json::to_string(&tier).unwrap();
        assert_eq!(json, r#""raw""#);

        let back: CredentialAccessTier = serde_json::from_str(&json).unwrap();
        assert_eq!(back, CredentialAccessTier::Raw);
    }

    #[test]
    fn test_policy_allows_tier() {
        let policy = CredentialPolicy {
            allowed_tiers: vec![CredentialAccessTier::Proxy, CredentialAccessTier::Raw],
            ..Default::default()
        };
        assert!(policy.allows_tier(CredentialAccessTier::Raw));
        assert!(!policy.allows_tier(CredentialAccessTier::ScopedToken));
    }

    #[test]
    fn test_raw_execution_requires_policy_and_support() {
        assert!(matches!(
            resolve_credential_execution_mode(
                CredentialAccessTier::Raw,
                None,
                CredentialExecutionSupport::RAW_SECRET_ONLY
            ),
            Err(CredentialExecutionError::TierNotAllowed { .. })
        ));

        let policy = CredentialPolicy {
            allowed_tiers: vec![CredentialAccessTier::Raw],
            ..Default::default()
        };
        assert_eq!(
            resolve_credential_execution_mode(
                CredentialAccessTier::Raw,
                Some(&policy),
                CredentialExecutionSupport::RAW_SECRET_ONLY
            )
            .unwrap(),
            CredentialExecutionMode::RawSecret
        );
    }

    #[test]
    fn test_unsupported_proxy_mode_fails_closed() {
        let err = resolve_credential_execution_mode(
            CredentialAccessTier::Proxy,
            None,
            CredentialExecutionSupport::FAIL_CLOSED,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            CredentialExecutionError::UnsupportedExecutionMode { .. }
        ));
    }

    #[test]
    fn test_policy_allows_all_when_empty() {
        let policy = CredentialPolicy::default();
        assert!(policy.is_domain_allowed("https://any.example.com/path"));
    }

    #[test]
    fn test_policy_exact_domain_match() {
        let policy = CredentialPolicy {
            allowed_domains: vec!["api.example.com".into()],
            ..Default::default()
        };
        assert!(policy.is_domain_allowed("https://api.example.com/v1/data"));
        assert!(!policy.is_domain_allowed("https://evil.com/v1/data"));
    }

    #[test]
    fn test_policy_subdomain_match() {
        let policy = CredentialPolicy {
            allowed_domains: vec!["example.com".into()],
            ..Default::default()
        };
        assert!(policy.is_domain_allowed("https://api.example.com/v1"));
        assert!(policy.is_domain_allowed("https://example.com/v1"));
        assert!(!policy.is_domain_allowed("https://notexample.com/v1"));
    }

    #[test]
    fn test_policy_domain_with_port() {
        let policy = CredentialPolicy {
            allowed_domains: vec!["localhost".into()],
            ..Default::default()
        };
        assert!(policy.is_domain_allowed("http://localhost:8080/api"));
    }

    #[test]
    fn test_capability_request_serde() {
        let req = CapabilityRequest {
            request_type: CapabilityRequestType::HttpRequest,
            url: "https://api.example.com/data".into(),
            method: "POST".into(),
            headers: HashMap::from([("Content-Type".into(), "application/json".into())]),
            body: Some(serde_json::json!({"key": "value"})),
            credential_id: CredentialId::new("cred-123").unwrap(),
        };

        let json = serde_json::to_string(&req).unwrap();
        let back: CapabilityRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(back.url, "https://api.example.com/data");
        assert_eq!(back.method, "POST");
        assert_eq!(back.credential_id.0, "cred-123");
    }

    #[test]
    fn test_capability_request_default_method() {
        let json = r#"{
            "request_type": "http_request",
            "url": "https://example.com",
            "credential_id": "cred-1"
        }"#;
        let req: CapabilityRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.method, "GET");
    }

    #[test]
    fn test_capability_response_serde() {
        let resp = CapabilityResponse {
            status_code: 200,
            headers: HashMap::new(),
            body: Some(serde_json::json!({"ok": true})),
            error: None,
        };

        let json = serde_json::to_string(&resp).unwrap();
        let back: CapabilityResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(back.status_code, 200);
        assert!(back.error.is_none());
    }
}
