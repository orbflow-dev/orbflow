## 2024-05-05 - [CRITICAL] Prevent Unicode-based homograph attacks in identifier validation
**Vulnerability:** The application was using `char::is_alphanumeric()` to validate plugin names and repositories. This Rust method allows all Unicode alphanumeric characters, meaning names with Cyrillic or Greek characters could pass validation.
**Learning:** `char::is_alphanumeric()` in Rust is not ASCII-restricted and can lead to homograph attacks, where an attacker registers a plugin with a visually identical name using non-ASCII characters.
**Prevention:** Always use `char::is_ascii_alphanumeric()` when validating system identifiers, URLs, or file paths where you expect standard ASCII characters.
## 2026-05-16 - [HIGH] Prevent XSS bypass via URL scheme obfuscation with control characters
**Vulnerability:** The `isSafeUrl` function checked for unsafe URL schemes (e.g., `javascript:`, `data:`) by trimming and lowercasing the input, but did not handle non-printable control characters. Attackers could bypass the check by injecting characters like `\x01` or tabs (`\x09`) into the URL scheme (e.g., `java\x09script:alert(1)`), which the browser would ignore and execute as XSS.
**Learning:** Browsers are highly lenient when parsing URL schemes and will strip out invalid control characters before evaluation. Simple string prefix checks (`startsWith`) are insufficient for validating URLs because they don't account for these obfuscation techniques.
**Prevention:** Before validating a URL scheme against a blocklist, always sanitize the input by explicitly stripping non-printable control characters (`[\x00-\x1F\x7F-\x9F]`) using a regex.
## 2024-05-18 - SSRF Defense in `CredentialProxy`

**Vulnerability:** The `CredentialProxy` in `orbflow-worker` validated URLs before making an HTTP request, but it did not protect against DNS rebinding (TOCTOU attacks) during the actual request execution by `reqwest`.
**Learning:** `reqwest` resolves DNS internally, meaning an attacker could provide a domain that points to a safe IP during validation but quickly rebinds to an internal/private IP when `reqwest` resolves it.
**Prevention:** Implement a custom `reqwest::dns::Resolve` trait (`ProxySsrfSafeResolver`) that performs async DNS resolution and blocks private IPs using `is_private_ip()`. Then inject this resolver into the `reqwest::Client` builder using `.dns_resolver()`. Also, aggressively disable redirects (`.redirect(reqwest::redirect::Policy::none())`) to prevent credentials from leaking if an attacker redirects the request.
