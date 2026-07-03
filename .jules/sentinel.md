## 2024-05-05 - [CRITICAL] Prevent Unicode-based homograph attacks in identifier validation
**Vulnerability:** The application was using `char::is_alphanumeric()` to validate plugin names and repositories. This Rust method allows all Unicode alphanumeric characters, meaning names with Cyrillic or Greek characters could pass validation.
**Learning:** `char::is_alphanumeric()` in Rust is not ASCII-restricted and can lead to homograph attacks, where an attacker registers a plugin with a visually identical name using non-ASCII characters.
**Prevention:** Always use `char::is_ascii_alphanumeric()` when validating system identifiers, URLs, or file paths where you expect standard ASCII characters.
## 2026-05-16 - [HIGH] Prevent XSS bypass via URL scheme obfuscation with control characters
**Vulnerability:** The `isSafeUrl` function checked for unsafe URL schemes (e.g., `javascript:`, `data:`) by trimming and lowercasing the input, but did not handle non-printable control characters. Attackers could bypass the check by injecting characters like `\x01` or tabs (`\x09`) into the URL scheme (e.g., `java\x09script:alert(1)`), which the browser would ignore and execute as XSS.
**Learning:** Browsers are highly lenient when parsing URL schemes and will strip out invalid control characters before evaluation. Simple string prefix checks (`startsWith`) are insufficient for validating URLs because they don't account for these obfuscation techniques.
**Prevention:** Before validating a URL scheme against a blocklist, always sanitize the input by explicitly stripping non-printable control characters (`[\x00-\x1F\x7F-\x9F]`) using a regex.

## 2024-05-24 - Path Traversal bypass via canonicalize fallback
**Vulnerability:** The HTTP API handler for uninstalling plugins allowed path traversal because it validated directory boundaries using `Path::starts_with()`. When `std::fs::canonicalize()` failed, it gracefully fell back to the uncanonicalized path via `.unwrap_or_else()`. Since `starts_with()` compares components, an uncanonicalized path with `..` segments bypasses the check.
**Learning:** Rust's `starts_with` does not resolve `..` components. Falling back to an uncanonicalized path upon a `canonicalize` error introduces a subtle bypass for boundary enforcement.
**Prevention:** Always fail securely when security-critical path transformations like `canonicalize` fail. Never fallback to the raw input.
