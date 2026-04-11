// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Shared SSRF (Server-Side Request Forgery) primitives.
//!
//! Provides common functions for checking whether an IP address or URL hostname
//! points to a private, internal, or cloud-metadata address. Used by adapter
//! crates (`orbflow-builtins`, `orbflow-mcp`) for defense-in-depth URL validation.

use std::net::IpAddr;

/// Known cloud metadata hostnames that must always be blocked.
pub const BLOCKED_HOSTNAMES: &[&str] = &[
    "metadata.google.internal",
    "metadata.internal",
    "169.254.169.254",
];

/// Allowed URL schemes for SSRF validation.
pub const ALLOWED_SCHEMES: &[&str] = &["http", "https"];

/// Checks whether an IP address is private, loopback, link-local, or
/// otherwise internal (cloud metadata, etc.).
///
/// Returns `Some(reason)` if the IP is considered private/internal,
/// `None` if it is a public address. When `allow_localhost` is `true`,
/// loopback addresses are permitted.
pub fn is_private_ip(ip: &IpAddr, allow_localhost: bool) -> Option<&'static str> {
    if ip.is_loopback() && !allow_localhost {
        return Some("loopback address");
    }

    match ip {
        IpAddr::V4(v4) => {
            if v4.is_private() {
                return Some("private IPv4 address");
            }
            if v4.is_link_local() || (v4.octets()[0] == 169 && v4.octets()[1] == 254) {
                return Some("link-local address");
            }
            // CGNAT / Shared Address Space (RFC 6598): 100.64.0.0/10
            let octets = v4.octets();
            if octets[0] == 100 && (octets[1] & 0xC0) == 0x40 {
                return Some("shared address space (CGNAT)");
            }
            if v4.is_broadcast() || v4.octets()[0] == 0 {
                return Some("non-routable address");
            }
            None
        }
        IpAddr::V6(v6) => {
            if v6.is_loopback() && !allow_localhost {
                return Some("loopback address");
            }
            let segments = v6.segments();
            // ULA (fc00::/7)
            if (segments[0] & 0xfe00) == 0xfc00 {
                return Some("private IPv6 (ULA) address");
            }
            // Link-local (fe80::/10)
            if (segments[0] & 0xffc0) == 0xfe80 {
                return Some("link-local IPv6 address");
            }
            // AWS IMDSv2 IPv6 (fd00:ec2::254)
            if segments[0] == 0xfd00
                && segments[1] == 0x0ec2
                && segments.iter().skip(2).take(5).all(|&s| s == 0)
                && segments[7] == 0x0254
            {
                return Some("cloud metadata IPv6 address");
            }
            None
        }
    }
}
