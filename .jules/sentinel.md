## 2025-05-15 - [SSRF Bypass in Credential Proxy]
**Vulnerability:** The `validate_proxy_url` function in `crates/orbflow-worker/src/credential_proxy.rs` was vulnerable to Server-Side Request Forgery (SSRF) bypasses due to string-based URL checks.
**Learning:** Checking for prefixes like `http://localhost` allows bypasses like `http://localhost.evildomain.com`, while `contains` string matching for blocked metadata IPs allows bypasses using integer representation of IP addresses (like `2852039166` for `169.254.169.254`).
**Prevention:** Always use a robust URL parsing library (`url::Url`) to extract the schema and the normalized host string, then enforce exact host matching, instead of relying on string containment.
