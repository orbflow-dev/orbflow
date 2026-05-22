## 2025-02-24 - Validate external URL attributes
**Vulnerability:** XSS via malicious URI scheme.
**Learning:** `plugin.repository` in `plugin-detail.tsx` was rendered in an `href` attribute without checking if the URL was safe. Because the URL is provided by a community index, it's externally controlled.
**Prevention:** Always validate URLs constructed from external properties using `isSafeUrl()` before injecting them into the DOM as `href` or `src` attributes.
