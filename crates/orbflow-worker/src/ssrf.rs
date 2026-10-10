use reqwest::dns::{Addrs, Resolve, Resolving};

use orbflow_core::ssrf::is_private_ip;

/// A custom DNS resolver for `reqwest` that blocks hostnames resolving to
/// private, link-local, or cloud-metadata IP addresses.
pub struct ProxySsrfSafeResolver;

impl Resolve for ProxySsrfSafeResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> Resolving {
        let name_str = name.as_str().to_string();

        let fut = async move {
            let addrs = tokio::net::lookup_host((name_str.as_str(), 0))
                .await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

            let mut valid_addrs = Vec::new();
            for addr in addrs {
                if let Some(reason) = is_private_ip(&addr.ip(), false) {
                    return Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::PermissionDenied,
                        format!("hostname resolved to blocked IP {reason}"),
                    ))
                        as Box<dyn std::error::Error + Send + Sync>);
                }
                valid_addrs.push(addr);
            }

            if valid_addrs.is_empty() {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "hostname resolved to no valid addresses",
                ))
                    as Box<dyn std::error::Error + Send + Sync>);
            }

            let addrs: Addrs = Box::new(valid_addrs.into_iter());
            Ok(addrs)
        };

        Box::pin(fut)
    }
}
