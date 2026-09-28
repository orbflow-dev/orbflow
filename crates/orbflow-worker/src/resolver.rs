use orbflow_core::ssrf::is_private_ip;
use reqwest::dns::{Addrs, Name, Resolve};

#[derive(Clone)]
pub struct ProxySsrfSafeResolver;

impl Resolve for ProxySsrfSafeResolver {
    fn resolve(&self, name: Name) -> reqwest::dns::Resolving {
        let name_str = name.as_str().to_string();
        Box::pin(async move {
            let mut resolved = tokio::net::lookup_host((name_str.as_str(), 0)).await?;
            let mut safe_addrs = Vec::new();

            for addr in resolved.by_ref() {
                if let Some(reason) = is_private_ip(&addr.ip(), false) {
                    return Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::PermissionDenied,
                        format!("credential proxy SSRF blocked: {reason}"),
                    ))
                        as Box<dyn std::error::Error + Send + Sync>);
                }
                safe_addrs.push(addr);
            }

            if safe_addrs.is_empty() {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "no addresses found",
                ))
                    as Box<dyn std::error::Error + Send + Sync>);
            }

            let addrs: Addrs = Box::new(safe_addrs.into_iter());
            Ok(addrs)
        })
    }
}
