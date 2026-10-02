## 2024-05-05 - [CRITICAL] Prevent Unicode-based homograph attacks in identifier validation
**Vulnerability:** The application was using `char::is_alphanumeric()` to validate plugin names and repositories. This Rust method allows all Unicode alphanumeric characters, meaning names with Cyrillic or Greek characters could pass validation.
**Learning:** `char::is_alphanumeric()` in Rust is not ASCII-restricted and can lead to homograph attacks, where an attacker registers a plugin with a visually identical name using non-ASCII characters.
**Prevention:** Always use `char::is_ascii_alphanumeric()` when validating system identifiers, URLs, or file paths where you expect standard ASCII characters.
## 2026-05-16 - [HIGH] Prevent XSS bypass via URL scheme obfuscation with control characters
**Vulnerability:** The `isSafeUrl` function checked for unsafe URL schemes (e.g., `javascript:`, `data:`) by trimming and lowercasing the input, but did not handle non-printable control characters. Attackers could bypass the check by injecting characters like `\x01` or tabs (`\x09`) into the URL scheme (e.g., `java\x09script:alert(1)`), which the browser would ignore and execute as XSS.
**Learning:** Browsers are highly lenient when parsing URL schemes and will strip out invalid control characters before evaluation. Simple string prefix checks (`startsWith`) are insufficient for validating URLs because they don't account for these obfuscation techniques.
**Prevention:** Before validating a URL scheme against a blocklist, always sanitize the input by explicitly stripping non-printable control characters (`[\x00-\x1F\x7F-\x9F]`) using a regex.

## 2024-10-02 - Path Traversal Bypass via Fail-Open Canonicalization
**Vulnerability:** The `uninstall_plugin` handler used `.unwrap_or_else()` to fall back to an uncanonicalized path when `std::fs::canonicalize` failed. This creates a fail-open condition where path traversal containment checks (using `starts_with()`) could be bypassed.
**Learning:** Using `.unwrap_or_else()` on security-critical initialization steps leads to fail-open vulnerabilities. If canonicalization fails, falling back to a raw path bypasses component-based path comparison logic.
**Prevention:** Always fail closed on security-critical operations. If canonicalization fails during containment validation, immediately return an error rather than falling back to potentially malicious input.
