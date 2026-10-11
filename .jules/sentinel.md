## 2024-05-05 - [CRITICAL] Prevent Unicode-based homograph attacks in identifier validation
**Vulnerability:** The application was using `char::is_alphanumeric()` to validate plugin names and repositories. This Rust method allows all Unicode alphanumeric characters, meaning names with Cyrillic or Greek characters could pass validation.
**Learning:** `char::is_alphanumeric()` in Rust is not ASCII-restricted and can lead to homograph attacks, where an attacker registers a plugin with a visually identical name using non-ASCII characters.
**Prevention:** Always use `char::is_ascii_alphanumeric()` when validating system identifiers, URLs, or file paths where you expect standard ASCII characters.
## 2026-05-16 - [HIGH] Prevent XSS bypass via URL scheme obfuscation with control characters
**Vulnerability:** The `isSafeUrl` function checked for unsafe URL schemes (e.g., `javascript:`, `data:`) by trimming and lowercasing the input, but did not handle non-printable control characters. Attackers could bypass the check by injecting characters like `\x01` or tabs (`\x09`) into the URL scheme (e.g., `java\x09script:alert(1)`), which the browser would ignore and execute as XSS.
**Learning:** Browsers are highly lenient when parsing URL schemes and will strip out invalid control characters before evaluation. Simple string prefix checks (`startsWith`) are insufficient for validating URLs because they don't account for these obfuscation techniques.
**Prevention:** Before validating a URL scheme against a blocklist, always sanitize the input by explicitly stripping non-printable control characters (`[\x00-\x1F\x7F-\x9F]`) using a regex.

## 2024-03-20 - [CRITICAL] Prevent SSRF TOCTOU in CredentialProxy via Custom DNS Resolver
**Vulnerability:** The `CredentialProxy` validated URLs before making an HTTP request using reqwest, but left a Time-Of-Check to Time-Of-Use (TOCTOU) gap. The proxy validated that a hostname didn't resolve to a private IP, but when reqwest resolved the same hostname later, it could have changed (DNS rebinding attack), leading to SSRF and leakage of credentials.
**Learning:** Checking a hostname's resolution before creating a request is not sufficient for SSRF protection because the resolution can change between check and use.
**Prevention:** Always implement `reqwest::dns::Resolve` to perform synchronous asynchronous SSRF checking at resolution time inside reqwest, using `is_private_ip`, and inject it via `dns_resolver()`.
