## YYYY-MM-DD - SSRF via DNS Rebinding in Credential Proxy
**Vulnerability:** The credential proxy implemented an incomplete SSRF defense. It validated URLs synchronously but used the default `reqwest` client, which resolved hostnames independently. This opened the proxy to DNS rebinding attacks (TOCTOU). Additionally, the client could follow redirects, bypassing the initial validation. Finally, an initial fix attempt fell back to a default unprotected client (`unwrap_or_else`) if the secure configuration failed to build.
**Learning:** Security controls like hardened HTTP clients must ensure the validated IP is the exact IP used for the connection. Relying on separate validation and resolution steps creates TOCTOU vulnerabilities. Furthermore, security controls must fail closed; falling back to an insecure default undermines the defense mechanism entirely.
**Prevention:**
1. Use a custom DNS resolver (`reqwest::dns::Resolve`) that validates the resolved IP addresses and passes only the safe ones to the HTTP client.
2. Disable HTTP redirects (`reqwest::redirect::Policy::none()`) when performing proxy requests.
3. Use `.expect()` or return an error during initialization of secure clients rather than falling back to an insecure default.
