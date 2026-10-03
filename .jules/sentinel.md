## 2024-05-05 - [CRITICAL] Prevent Unicode-based homograph attacks in identifier validation
**Vulnerability:** The application was using `char::is_alphanumeric()` to validate plugin names and repositories. This Rust method allows all Unicode alphanumeric characters, meaning names with Cyrillic or Greek characters could pass validation.
**Learning:** `char::is_alphanumeric()` in Rust is not ASCII-restricted and can lead to homograph attacks, where an attacker registers a plugin with a visually identical name using non-ASCII characters.
**Prevention:** Always use `char::is_ascii_alphanumeric()` when validating system identifiers, URLs, or file paths where you expect standard ASCII characters.
## 2026-05-16 - [HIGH] Prevent XSS bypass via URL scheme obfuscation with control characters
**Vulnerability:** The `isSafeUrl` function checked for unsafe URL schemes (e.g., `javascript:`, `data:`) by trimming and lowercasing the input, but did not handle non-printable control characters. Attackers could bypass the check by injecting characters like `\x01` or tabs (`\x09`) into the URL scheme (e.g., `java\x09script:alert(1)`), which the browser would ignore and execute as XSS.
**Learning:** Browsers are highly lenient when parsing URL schemes and will strip out invalid control characters before evaluation. Simple string prefix checks (`startsWith`) are insufficient for validating URLs because they don't account for these obfuscation techniques.
**Prevention:** Before validating a URL scheme against a blocklist, always sanitize the input by explicitly stripping non-printable control characters (`[\x00-\x1F\x7F-\x9F]`) using a regex.

## 2024-10-04 - [Fail-Open Path Canonicalization Vulnerability]
**Vulnerability:** The path traversal mitigation for plugin directory validation used `unwrap_or_else` on `std::fs::canonicalize`, falling back to the uncanonicalized path if it failed (e.g., if the path didn't exist). Because `Path::starts_with` only checks components and doesn't resolve `..`, this allowed bypassing the directory containment check completely if canonicalization was forced to fail.
**Learning:** Security validations that rely on path canonicalization must strictly fail closed. A fallback to an uncanonicalized string re-introduces the exact path traversal risk the check was meant to prevent.
**Prevention:** Remove `unwrap_or_else` in canonicalization checks for security boundaries and use `map_err` or the `?` operator to ensure the operation aborts safely on error.
