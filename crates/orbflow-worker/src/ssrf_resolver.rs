use orbflow_core::ssrf::is_private_ip;
use reqwest::dns::{Addrs, Name, Resolve, Resolving};

/// A custom DNS resolver that validates resolved IPs to block internal addresses
/// (preventing DNS rebinding and TOCTOU attacks).
#[derive(Clone, Default)]
pub struct ProxySsrfSafeResolver;

impl Resolve for ProxySsrfSafeResolver {
    fn resolve(&self, name: Name) -> Resolving {
        Box::pin(async move {
            let host = name.as_str();
            let resolved = tokio::net::lookup_host((host, 0)).await?;
            let mut safe_addrs = Vec::new();

            for addr in resolved {
                if let Some(reason) = is_private_ip(&addr.ip(), false) {
                    return Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::PermissionDenied,
                        format!(
                            "credential proxy DNS rebinding blocked: {} resolved to {} ({})",
                            host,
                            addr.ip(),
                            reason
                        ),
                    ))
                        as Box<dyn std::error::Error + Send + Sync>);
                }
                safe_addrs.push(addr);
            }

            if safe_addrs.is_empty() {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("credential proxy DNS: no addresses found for {}", host),
                ))
                    as Box<dyn std::error::Error + Send + Sync>);
            }

            let addrs: Addrs = Box::new(safe_addrs.into_iter());
            Ok(addrs)
        })
    }
}
