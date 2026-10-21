use orbflow_core::ssrf::{BLOCKED_HOSTNAMES, is_private_ip};

#[derive(Clone)]
pub struct ProxySsrfSafeResolver;

impl reqwest::dns::Resolve for ProxySsrfSafeResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let host = name.as_str().to_string();

        let fut = async move {
            let host_lower = host.to_ascii_lowercase();
            if host_lower == "localhost" || BLOCKED_HOSTNAMES.contains(&host_lower.as_str()) {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    format!("credential proxy blocked resolution of internal host: {host}"),
                ))
                    as Box<dyn std::error::Error + Send + Sync>);
            }

            let resolved = tokio::net::lookup_host((host.as_str(), 0))
                .await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

            let mut addrs = Vec::new();
            for addr in resolved {
                if let Some(reason) = is_private_ip(&addr.ip(), false) {
                    return Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::PermissionDenied,
                        format!(
                            "credential proxy hostname '{host}' resolves to {reason} ({})",
                            addr.ip()
                        ),
                    ))
                        as Box<dyn std::error::Error + Send + Sync>);
                }
                addrs.push(addr);
            }

            if addrs.is_empty() {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("credential proxy hostname '{host}' resolved no addresses"),
                ))
                    as Box<dyn std::error::Error + Send + Sync>);
            }

            let iter: reqwest::dns::Addrs = Box::new(addrs.into_iter());
            Ok(iter)
        };

        Box::pin(fut)
    }
}
