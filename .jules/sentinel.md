## 2025-05-18 - Fix SSRF TOCTOU Vulnerability in CredentialProxy

**Vulnerability:** A Time-of-Check-to-Time-of-Use (TOCTOU) DNS rebinding vulnerability and a redirect bypass vulnerability existed in `crates/orbflow-worker/src/credential_proxy.rs`. The proxy validated the URL hostname upfront against SSRF blocklists using `tokio::net::lookup_host`, but then passed the URL to a default `reqwest::Client` which re-resolved the DNS (potentially to a different, malicious internal IP) and allowed HTTP redirects to private IPs.

**Learning:** URL string validation and initial DNS resolution are insufficient when the HTTP client performs its own resolution later and follows redirects. Attackers can exploit this delay (DNS rebinding) or return HTTP 3xx responses pointing to internal infrastructure. This bypasses the upfront `is_private_ip` validation.

**Prevention:** SSRF protection must be implemented inside the HTTP client's connection pipeline. Always use a custom DNS resolver implementing `reqwest::dns::Resolve` to enforce policies during the actual connection attempt, and configure the client to disable redirects using `.redirect(reqwest::redirect::Policy::none())` when acting as a proxy. Use `.expect()` to fail securely (closed) during client initialization if the configuration fails.
