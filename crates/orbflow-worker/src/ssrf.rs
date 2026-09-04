use orbflow_core::ssrf::is_private_ip;
use reqwest::dns::{Addrs, Name, Resolve};
use tokio::net::lookup_host;

#[derive(Clone)]
pub(crate) struct ProxySsrfSafeResolver;

impl Resolve for ProxySsrfSafeResolver {
    fn resolve(&self, name: Name) -> reqwest::dns::Resolving {
        let name_str = name.as_str().to_string();
        Box::pin(async move {
            let addrs = lookup_host((name_str.as_str(), 0))
                .await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

            let mut validated = Vec::new();
            for addr in addrs {
                if is_private_ip(&addr.ip(), false).is_some() {
                    return Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::PermissionDenied,
                        "credential proxy SSRF policy blocked DNS resolution to private IP",
                    ))
                        as Box<dyn std::error::Error + Send + Sync>);
                }
                validated.push(addr);
            }

            if validated.is_empty() {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "credential proxy SSRF policy blocked all resolved IPs",
                ))
                    as Box<dyn std::error::Error + Send + Sync>);
            }

            Ok(Box::new(validated.into_iter()) as Addrs)
        })
    }
}
