## 2023-10-27 - [Fix SSRF Vulnerability in CredentialProxy]
**Vulnerability:** The `CredentialProxy` in `orbflow-worker` used a default `reqwest::Client` without a custom DNS resolver, leaving it vulnerable to SSRF (Server-Side Request Forgery) and DNS rebinding attacks via TOCTOU (Time-of-Check to Time-of-Use).
**Learning:** Pre-flight validation of URLs using `is_private_ip` is insufficient if the IP resolves differently at the time of the actual HTTP connection.
**Prevention:** Implement a custom `reqwest::dns::Resolve` trait (`ProxySsrfSafeResolver`) that asynchronously resolves domains and validates all IPs immediately prior to connection, rejecting any private or internal addresses.
