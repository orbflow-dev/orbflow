use orbflow_core::ssrf::is_private_ip;
use reqwest::dns::{Addrs, Name, Resolve, Resolving};

/// A custom DNS resolver for reqwest that blocks private/internal IPs to prevent SSRF attacks.
#[derive(Clone)]
pub struct ProxySsrfSafeResolver;

impl Resolve for ProxySsrfSafeResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let name_str = name.as_str().to_string();
        Box::pin(async move {
            let mut resolved = Vec::new();
            // Append dummy port (0) since lookup_host requires it
            let addrs = tokio::net::lookup_host((name_str.as_str(), 0))
                .await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

            for addr in addrs {
                let ip = addr.ip();
                if let Some(reason) = is_private_ip(&ip, false) {
                    return Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::PermissionDenied,
                        format!(
                            "credential proxy DNS resolution blocked request to {reason}: {ip}"
                        ),
                    ))
                        as Box<dyn std::error::Error + Send + Sync>);
                }
                resolved.push(addr);
            }

            if resolved.is_empty() {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "resolved no addresses",
                ))
                    as Box<dyn std::error::Error + Send + Sync>);
            }

            let it = resolved.into_iter();
            Ok(Box::new(it) as Addrs)
        })
    }
}
