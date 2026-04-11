// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Shared SSRF (Server-Side Request Forgery) validation.
//!
//! Provides a single function to check whether a URL points to a private or
//! internal network address. Used by the HTTP builtin node, MCP tool node,
//! and AI provider base-URL validation.

use std::net::IpAddr;

use orbflow_core::OrbflowError;
use orbflow_core::ssrf::{ALLOWED_SCHEMES, BLOCKED_HOSTNAMES, is_private_ip};
use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use url::Url;

/// Custom DNS resolver that validates each resolved IP against [`is_private_ip`]
/// before allowing the connection, closing the TOCTOU gap between the pre-flight
/// SSRF check and reqwest's own DNS resolution.
pub struct SsrfSafeResolver {
    pub allow_localhost: bool,
}

impl Resolve for SsrfSafeResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let allow_localhost = self.allow_localhost;
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

/// Validates that `url_str` does not point to a private, loopback, or
/// cloud-metadata address.
///
/// When `allow_localhost` is `true`, loopback addresses (`127.0.0.1`, `::1`)
/// and the hostname `localhost` are permitted (useful for MCP dev servers and
/// Ollama).
///
/// Checks literal IPs and known hostnames synchronously. For async DNS
/// rebinding defense, use [`validate_url_not_private_async`].
pub fn validate_url_not_private(url_str: &str, allow_localhost: bool) -> Result<(), OrbflowError> {
    let parsed = Url::parse(url_str)
        .map_err(|_| OrbflowError::InvalidNodeConfig(format!("invalid URL: {url_str}")))?;

    // Enforce scheme allowlist to block file://, ftp://, gopher://, etc.
    if !ALLOWED_SCHEMES.contains(&parsed.scheme()) {
        return Err(OrbflowError::InvalidNodeConfig(format!(
            "URL scheme '{}' is not allowed (only http and https)",
            parsed.scheme()
        )));
    }

    let host = parsed
        .host_str()
        .filter(|h| !h.is_empty())
        .ok_or_else(|| OrbflowError::InvalidNodeConfig(format!("URL has no host: {url_str}")))?;

    // --- Check known blocked hostnames ---
    let lower = host.to_lowercase();
    if lower == "localhost" && !allow_localhost {
        return Err(OrbflowError::InvalidNodeConfig(
            "URL points to localhost".into(),
        ));
    }
    if BLOCKED_HOSTNAMES.contains(&lower.as_str()) {
        return Err(OrbflowError::InvalidNodeConfig(
            "URL points to cloud metadata endpoint".into(),
        ));
    }

    // --- Check literal IP addresses ---
    let ip_from_host = match parsed.host() {
        Some(url::Host::Ipv4(v4)) => Some(IpAddr::V4(v4)),
        Some(url::Host::Ipv6(v6)) => Some(IpAddr::V6(v6)),
        _ => None,
    };

    if let Some(ip) = ip_from_host
        && let Some(reason) = is_private_ip(&ip, allow_localhost)
    {
        return Err(OrbflowError::InvalidNodeConfig(format!(
            "URL points to {reason}: {host}"
        )));
    }

    Ok(())
}

/// Async version of [`validate_url_not_private`] that additionally performs
/// non-blocking DNS resolution to defend against DNS rebinding attacks.
///
/// Use this from async node executors (HTTP node, MCP tool node) where the
/// URL comes from user-controlled workflow config.
pub async fn validate_url_not_private_async(
    url_str: &str,
    allow_localhost: bool,
) -> Result<(), OrbflowError> {
    // Run all the synchronous checks first.
    validate_url_not_private(url_str, allow_localhost)?;

    // --- Async DNS resolution check (defends against DNS rebinding) ---
    let parsed = Url::parse(url_str)
        .map_err(|_| OrbflowError::InvalidNodeConfig(format!("invalid URL: {url_str}")))?;
    let host = parsed
        .host_str()
        .filter(|h| !h.is_empty())
        .ok_or_else(|| OrbflowError::InvalidNodeConfig(format!("URL has no host: {url_str}")))?;
    let port = parsed
        .port()
        .unwrap_or(if parsed.scheme() == "https" { 443 } else { 80 });
    let host_port = format!("{host}:{port}");

    // Determine if the host is a literal IP (already validated synchronously above).
    let is_literal_ip = matches!(
        parsed.host(),
        Some(url::Host::Ipv4(_)) | Some(url::Host::Ipv6(_))
    );

    match tokio::net::lookup_host(&host_port).await {
        Ok(addrs) => {
            for addr in addrs {
                if let Some(reason) = is_private_ip(&addr.ip(), allow_localhost) {
                    return Err(OrbflowError::InvalidNodeConfig(format!(
                        "URL hostname '{host}' resolves to {reason} ({})",
                        addr.ip()
                    )));
                }
            }
        }
        Err(_) if is_literal_ip => {
            // Literal IPs were already validated synchronously — safe to proceed.
        }
        Err(_) => {
            // SECURITY: For non-literal-IP hostnames, DNS resolution failure
            // means we cannot verify the target address. Deny the request to
            // prevent DNS rebinding attacks.
            return Err(OrbflowError::InvalidNodeConfig(format!(
                "URL hostname '{host}' could not be resolved"
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_public_url() {
        assert!(validate_url_not_private("https://api.example.com/v1", false).is_ok());
    }

    #[test]
    fn blocks_localhost_when_not_allowed() {
        assert!(validate_url_not_private("http://localhost:8080/", false).is_err());
        assert!(validate_url_not_private("http://127.0.0.1/", false).is_err());
    }

    #[test]
    fn allows_localhost_when_allowed() {
        assert!(validate_url_not_private("http://localhost:8080/", true).is_ok());
        assert!(validate_url_not_private("http://127.0.0.1/", true).is_ok());
    }

    #[test]
    fn blocks_private_ipv4() {
        assert!(validate_url_not_private("http://10.0.0.1/", false).is_err());
        assert!(validate_url_not_private("http://192.168.1.1/", false).is_err());
        assert!(validate_url_not_private("http://172.16.0.1/", false).is_err());
        assert!(validate_url_not_private("http://172.31.255.255/", false).is_err());
    }

    #[test]
    fn allows_172_public_range() {
        // 172.1.x.x is NOT in the private 172.16-31 range
        assert!(validate_url_not_private("http://172.1.2.3/", false).is_ok());
        // 172.32.x.x is outside the private range
        assert!(validate_url_not_private("http://172.32.0.1/", false).is_ok());
    }

    #[test]
    fn blocks_link_local() {
        assert!(validate_url_not_private("http://169.254.169.254/", false).is_err());
    }

    #[test]
    fn blocks_cloud_metadata_hostname() {
        assert!(validate_url_not_private("http://metadata.google.internal/v1/", false).is_err());
    }

    #[test]
    fn blocks_ipv6_loopback_when_not_allowed() {
        assert!(validate_url_not_private("http://[::1]/", false).is_err());
    }

    #[test]
    fn allows_ipv6_loopback_when_allowed() {
        assert!(validate_url_not_private("http://[::1]/", true).is_ok());
    }

    #[test]
    fn blocks_ipv6_ula() {
        assert!(validate_url_not_private("http://[fd12::1]/", false).is_err());
        assert!(validate_url_not_private("http://[fc00::1]/", false).is_err());
    }

    #[test]
    fn blocks_ipv6_link_local() {
        assert!(validate_url_not_private("http://[fe80::1]/", false).is_err());
    }

    #[test]
    fn blocks_zero_address() {
        assert!(validate_url_not_private("http://0.0.0.0/", false).is_err());
    }

    #[test]
    fn rejects_invalid_url() {
        assert!(validate_url_not_private("not a url", false).is_err());
    }

    // ---- Valid public URLs (should pass) ----

    #[test]
    fn allows_public_https_domain() {
        assert!(validate_url_not_private("https://example.com", false).is_ok());
    }

    #[test]
    fn allows_public_ipv4_literal() {
        assert!(validate_url_not_private("https://1.2.3.4", false).is_ok());
    }

    #[test]
    fn allows_public_ipv4_with_port() {
        assert!(validate_url_not_private("http://8.8.8.8:8080/path", false).is_ok());
    }

    #[test]
    fn allows_public_ipv6_literal() {
        assert!(validate_url_not_private("http://[2001:db8::1]/resource", false).is_ok());
    }

    #[test]
    fn allows_public_url_with_path_and_query() {
        assert!(
            validate_url_not_private("https://api.example.com/v2/data?key=val&page=1", false)
                .is_ok()
        );
    }

    // ---- Private IPv4 ranges (should fail) ----

    #[test]
    fn blocks_10_0_0_0_slash_8() {
        assert!(validate_url_not_private("http://10.0.0.1/", false).is_err());
        assert!(validate_url_not_private("http://10.255.255.255/", false).is_err());
        assert!(validate_url_not_private("http://10.100.50.25:9090/", false).is_err());
    }

    #[test]
    fn blocks_172_16_slash_12() {
        assert!(validate_url_not_private("http://172.16.0.1/", false).is_err());
        assert!(validate_url_not_private("http://172.20.10.5/", false).is_err());
        assert!(validate_url_not_private("http://172.31.255.255/", false).is_err());
    }

    #[test]
    fn blocks_192_168_slash_16() {
        assert!(validate_url_not_private("http://192.168.0.1/", false).is_err());
        assert!(validate_url_not_private("http://192.168.1.1/", false).is_err());
        assert!(validate_url_not_private("http://192.168.255.255/", false).is_err());
    }

    #[test]
    fn blocks_127_slash_8_loopback() {
        assert!(validate_url_not_private("http://127.0.0.1/", false).is_err());
        assert!(validate_url_not_private("http://127.0.0.2/", false).is_err());
        assert!(validate_url_not_private("http://127.255.255.255/", false).is_err());
    }

    // ---- Private IPv6 (should fail) ----

    #[test]
    fn blocks_ipv6_loopback() {
        assert!(validate_url_not_private("http://[::1]/", false).is_err());
    }

    #[test]
    fn blocks_ipv6_link_local_fe80() {
        assert!(validate_url_not_private("http://[fe80::1]/", false).is_err());
        assert!(validate_url_not_private("http://[fe80::abcd:1234]/", false).is_err());
    }

    #[test]
    fn blocks_ipv6_ula_fc00() {
        assert!(validate_url_not_private("http://[fc00::1]/", false).is_err());
    }

    #[test]
    fn blocks_ipv6_ula_fd00() {
        assert!(validate_url_not_private("http://[fd00::1]/", false).is_err());
        assert!(validate_url_not_private("http://[fd12:3456:789a::1]/", false).is_err());
    }

    // ---- Cloud metadata endpoints (should fail) ----

    #[test]
    fn blocks_aws_metadata_ip() {
        assert!(validate_url_not_private("http://169.254.169.254/", false).is_err());
        assert!(
            validate_url_not_private("http://169.254.169.254/latest/meta-data/", false).is_err()
        );
    }

    #[test]
    fn blocks_gcp_metadata_hostname() {
        assert!(
            validate_url_not_private("http://metadata.google.internal/computeMetadata/v1/", false)
                .is_err()
        );
    }

    #[test]
    fn blocks_metadata_internal_hostname() {
        assert!(validate_url_not_private("http://metadata.internal/", false).is_err());
    }

    #[test]
    fn blocks_metadata_ip_as_hostname_string() {
        // 169.254.169.254 is in BLOCKED_HOSTNAMES as a string
        assert!(
            validate_url_not_private("http://169.254.169.254/latest/api/token", false).is_err()
        );
    }

    #[test]
    fn blocks_aws_metadata_ipv6() {
        // fd00:ec2::254 — AWS IMDSv2 IPv6 endpoint
        assert!(validate_url_not_private("http://[fd00:ec2::254]/", false).is_err());
    }

    // ---- Link-local range (should fail) ----

    #[test]
    fn blocks_link_local_169_254() {
        assert!(validate_url_not_private("http://169.254.0.1/", false).is_err());
        assert!(validate_url_not_private("http://169.254.1.1/", false).is_err());
    }

    // ---- CGNAT / Shared Address Space (RFC 6598) ----

    #[test]
    fn blocks_cgnat_range() {
        assert!(validate_url_not_private("http://100.64.0.1/", false).is_err());
        assert!(validate_url_not_private("http://100.127.255.255/", false).is_err());
    }

    #[test]
    fn allows_public_100_outside_cgnat() {
        // 100.63.x.x is below CGNAT range
        assert!(validate_url_not_private("http://100.63.255.255/", false).is_ok());
        // 100.128.x.x is above CGNAT range
        assert!(validate_url_not_private("http://100.128.0.1/", false).is_ok());
    }

    // ---- Non-routable addresses (should fail) ----

    #[test]
    fn blocks_zero_prefix_addresses() {
        assert!(validate_url_not_private("http://0.0.0.0/", false).is_err());
        assert!(validate_url_not_private("http://0.1.2.3/", false).is_err());
    }

    #[test]
    fn blocks_broadcast_address() {
        assert!(validate_url_not_private("http://255.255.255.255/", false).is_err());
    }

    // ---- Edge cases: missing scheme, empty host, unusual ports ----

    #[test]
    fn rejects_missing_scheme() {
        assert!(validate_url_not_private("example.com", false).is_err());
    }

    #[test]
    fn rejects_empty_string() {
        assert!(validate_url_not_private("", false).is_err());
    }

    #[test]
    fn rejects_only_scheme() {
        assert!(validate_url_not_private("http://", false).is_err());
    }

    #[test]
    fn allows_unusual_port_on_public_ip() {
        assert!(validate_url_not_private("http://8.8.8.8:12345/", false).is_ok());
    }

    #[test]
    fn blocks_private_ip_with_unusual_port() {
        assert!(validate_url_not_private("http://10.0.0.1:9999/", false).is_err());
        assert!(validate_url_not_private("http://192.168.1.1:443/", false).is_err());
    }

    #[test]
    fn rejects_whitespace_url() {
        assert!(validate_url_not_private("   ", false).is_err());
    }

    // ---- Localhost variations ----

    #[test]
    fn blocks_localhost_string() {
        assert!(validate_url_not_private("http://localhost/", false).is_err());
        assert!(validate_url_not_private("http://localhost:3000/", false).is_err());
    }

    #[test]
    fn blocks_localhost_ip() {
        assert!(validate_url_not_private("http://127.0.0.1/", false).is_err());
        assert!(validate_url_not_private("http://127.0.0.1:8080/api", false).is_err());
    }

    #[test]
    fn blocks_localhost_ipv6_bracket() {
        assert!(validate_url_not_private("http://[::1]/", false).is_err());
        assert!(validate_url_not_private("http://[::1]:8080/", false).is_err());
    }

    // ---- allow_localhost flag interactions ----

    #[test]
    fn allow_localhost_permits_all_loopback_forms() {
        assert!(validate_url_not_private("http://localhost:8080/", true).is_ok());
        assert!(validate_url_not_private("http://127.0.0.1/", true).is_ok());
        assert!(validate_url_not_private("http://[::1]/", true).is_ok());
        assert!(validate_url_not_private("http://[::1]:3000/path", true).is_ok());
    }

    #[test]
    fn allow_localhost_still_blocks_private_ranges() {
        assert!(validate_url_not_private("http://10.0.0.1/", true).is_err());
        assert!(validate_url_not_private("http://192.168.1.1/", true).is_err());
        assert!(validate_url_not_private("http://172.16.0.1/", true).is_err());
    }

    #[test]
    fn allow_localhost_still_blocks_cloud_metadata() {
        assert!(validate_url_not_private("http://169.254.169.254/", true).is_err());
        assert!(validate_url_not_private("http://metadata.google.internal/", true).is_err());
    }

    #[test]
    fn allow_localhost_still_blocks_link_local() {
        assert!(validate_url_not_private("http://169.254.0.1/", true).is_err());
    }

    #[test]
    fn allow_localhost_still_blocks_ipv6_ula() {
        assert!(validate_url_not_private("http://[fd00::1]/", true).is_err());
        assert!(validate_url_not_private("http://[fc00::1]/", true).is_err());
    }

    #[test]
    fn allow_localhost_still_blocks_ipv6_link_local() {
        assert!(validate_url_not_private("http://[fe80::1]/", true).is_err());
    }

    // ---- Error message content validation ----

    #[test]
    fn error_mentions_loopback_for_localhost_ip() {
        let err = validate_url_not_private("http://127.0.0.1/", false).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("loopback"), "expected 'loopback' in: {msg}");
    }

    #[test]
    fn error_mentions_private_for_rfc1918() {
        let err = validate_url_not_private("http://10.0.0.1/", false).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("private"), "expected 'private' in: {msg}");
    }

    #[test]
    fn error_mentions_cloud_metadata_for_blocked_hostname() {
        let err = validate_url_not_private("http://metadata.google.internal/", false).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("cloud metadata"),
            "expected 'cloud metadata' in: {msg}"
        );
    }

    #[test]
    fn error_mentions_localhost_for_hostname() {
        let err = validate_url_not_private("http://localhost/", false).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("localhost"), "expected 'localhost' in: {msg}");
    }

    #[test]
    fn error_mentions_link_local_for_169_254() {
        let err = validate_url_not_private("http://169.254.1.1/", false).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("link-local"),
            "expected 'link-local' in: {msg}"
        );
    }

    #[test]
    fn error_mentions_ula_for_ipv6_fd() {
        let err = validate_url_not_private("http://[fd12::1]/", false).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("ULA"), "expected 'ULA' in: {msg}");
    }

    // ---- Boundary / public range checks ----

    #[test]
    fn allows_172_outside_private_range() {
        // 172.15.x.x is below private range (172.16-31)
        assert!(validate_url_not_private("http://172.15.255.255/", false).is_ok());
        // 172.32.x.x is above private range
        assert!(validate_url_not_private("http://172.32.0.1/", false).is_ok());
    }

    #[test]
    fn allows_public_ipv4_just_above_ten() {
        assert!(validate_url_not_private("http://11.0.0.1/", false).is_ok());
    }

    #[test]
    fn allows_public_192_non_168() {
        assert!(validate_url_not_private("http://192.167.1.1/", false).is_ok());
        assert!(validate_url_not_private("http://192.169.1.1/", false).is_ok());
    }

    #[test]
    fn allows_public_ipv6_global_unicast() {
        assert!(validate_url_not_private("http://[2607:f8b0:4004:800::200e]/", false).is_ok());
    }

    // ---- Scheme allowlist ----

    #[test]
    fn blocks_file_scheme() {
        let err = validate_url_not_private("file:///etc/passwd", false).unwrap_err();
        assert!(err.to_string().contains("scheme"));
    }

    #[test]
    fn blocks_ftp_scheme() {
        let err = validate_url_not_private("ftp://evil.com/file", false).unwrap_err();
        assert!(err.to_string().contains("scheme"));
    }

    #[test]
    fn blocks_gopher_scheme() {
        let err = validate_url_not_private("gopher://evil.com/", false).unwrap_err();
        assert!(err.to_string().contains("scheme"));
    }

    #[test]
    fn blocks_data_scheme() {
        let err = validate_url_not_private("data:text/html,<h1>hi</h1>", false).unwrap_err();
        assert!(err.to_string().contains("scheme"));
    }

    #[test]
    fn allows_http_scheme() {
        assert!(validate_url_not_private("http://example.com", false).is_ok());
    }

    #[test]
    fn allows_https_scheme() {
        assert!(validate_url_not_private("https://example.com", false).is_ok());
    }
}
