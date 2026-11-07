use orbflow_core::ssrf::is_private_ip;
use reqwest::dns::{Name, Resolve, Resolving};
use std::net::SocketAddr;

pub struct ProxySsrfSafeResolver;

impl Resolve for ProxySsrfSafeResolver {
    fn resolve(&self, name: Name) -> Resolving {
        Box::pin(async move {
            let addrs: Vec<SocketAddr> = tokio::net::lookup_host(format!("{}:0", name.as_str()))
                .await
                .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?
                .collect();
            let validated: Vec<SocketAddr> = addrs
                .into_iter()
                .filter(|a| is_private_ip(&a.ip(), false).is_none())
                .collect();
            if validated.is_empty() {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "all resolved addresses are private/internal (SSRF protection)",
                ))
                    as Box<dyn std::error::Error + Send + Sync>);
            }
            Ok(Box::new(validated.into_iter()) as Box<dyn Iterator<Item = SocketAddr> + Send>)
        })
    }
}
