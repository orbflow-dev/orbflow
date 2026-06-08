## 2025-02-27 - [Fix] Fix TOCTOU SSRF Vulnerability in CredentialProxy

**Vulnerability:** The `CredentialProxy` in `orbflow-worker` performed SSRF protection by merely substring checking the URL against metadata IPs, but then passed the URL to a default `reqwest::Client`. This opens a TOCTOU (Time-of-Check to Time-of-Use) window where an attacker could exploit DNS rebinding to bypass the check and hit local/private network assets on behalf of the proxy.

**Learning:** `reqwest` clients require a custom DNS resolver implementing `reqwest::dns::Resolve` to enforce safe connections synchronously at the precise time of DNS resolution, which prevents rebinding attacks. Additionally, relying on `unwrap_or_default()` when building a security-critical client fails-open, silently defaulting to an insecure client.

**Prevention:** I implemented a `ProxySsrfSafeResolver` that asynchronously resolves addresses and strictly filters them through `orbflow_core::ssrf::is_private_ip`. I updated the HTTP client initialization to use this resolver, to reject redirects (`.redirect(reqwest::redirect::Policy::none())`), and to fail-closed with `expect()` on builder error.
