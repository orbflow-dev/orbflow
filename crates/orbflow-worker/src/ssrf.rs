// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Shared SSRF (Server-Side Request Forgery) validation for worker.

use orbflow_core::ssrf::is_private_ip;
use reqwest::dns::{Addrs, Name, Resolve, Resolving};

/// Custom DNS resolver that validates each resolved IP against `is_private_ip`
/// before allowing the connection, closing the TOCTOU gap between the pre-flight
/// SSRF check and reqwest's own DNS resolution.
pub struct ProxySsrfSafeResolver;

impl Resolve for ProxySsrfSafeResolver {
    fn resolve(&self, name: Name) -> Resolving {
        Box::pin(async move {
            let resolved = tokio::net::lookup_host((name.as_str(), 0))
                .await
                .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;

            let mut addrs = Vec::new();
            for addr in resolved {
                if is_private_ip(&addr.ip(), false).is_none() {
                    addrs.push(addr);
                }
            }

            if addrs.is_empty() {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "all resolved addresses are private/internal (SSRF protection)",
                ))
                    as Box<dyn std::error::Error + Send + Sync>);
            }

            let iter: Addrs = Box::new(addrs.into_iter());
            Ok(iter)
        })
    }
}
