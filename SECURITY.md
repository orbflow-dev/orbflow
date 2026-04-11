# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability in Orbflow, please report it responsibly.

**Do not open a public GitHub issue for security vulnerabilities.**

Instead, please email **security@orbflow.dev** with:

- A description of the vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if any)

We will acknowledge receipt within 48 hours and provide a timeline for a fix. We ask that you give us reasonable time to address the issue before public disclosure.

## Supported Versions

| Version | Supported |
|---------|-----------|
| 0.1.x   | Yes       |

## Security Measures

Orbflow implements the following security measures:

- **Encrypted credentials**: AES-256-GCM encryption for stored credentials with trust tiers
- **RBAC**: Role-based access control with scoped permissions
- **Rate limiting**: Configurable rate limits on all API endpoints
- **SSRF protection**: URL validation and private IP blocking on the HTTP node
- **Input validation**: Schema-based validation at all API boundaries
- **Audit logging**: Domain event sourcing for full audit trail
- **CORS**: Configurable origin allowlists
- **Non-root Docker**: Production container runs as unprivileged user
