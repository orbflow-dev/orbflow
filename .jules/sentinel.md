## 2025-02-28 - [High] Fix XSS in plugin repository URL
**Vulnerability:** XSS vulnerability where `plugin.repository` in `apps/web/src/components/marketplace/plugin-detail.tsx` was directly injected into an `href` without URL sanitization.
**Learning:** React component anchor tags can lead to XSS if `href` takes `javascript:` or `data:text/html` schemes. External data must be verified.
**Prevention:** Always use `isSafeUrl` before assigning external data to the `href` attribute.
