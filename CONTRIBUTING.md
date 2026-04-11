# Contributing to Orbflow

Thank you for your interest in contributing to Orbflow! This guide will help you get started.

## Getting Started

1. Fork the repository
2. Clone your fork: `git clone https://github.com/<your-username>/orbflow.git`
3. Run `just setup` to install dependencies and start infrastructure
4. Create a feature branch: `git checkout -b feat/my-feature`

## Development Setup

**Prerequisites**: Rust (edition 2024), Node.js 22+, pnpm, Docker, just

```bash
just setup          # First-time setup
just dev            # Start server + worker + frontend with live reload
just dev-backend    # Backend only (no frontend)
```

## Making Changes

### Code Style

- **Rust**: Run `just fmt` and `just lint` before committing
- **Frontend**: Run `pnpm lint` in `apps/web/`
- Follow existing patterns in the codebase

### Testing

All changes should include tests. Target 80%+ coverage.

```bash
just test           # Run all Rust tests
just test-web       # Run frontend tests
just test-all       # Run everything
just ci             # Full CI pipeline (format + lint + test + build)
```

### Commit Messages

Use [Conventional Commits](https://www.conventionalcommits.org/):

```
feat: add new node type for S3 operations
fix: handle timeout in HTTP node executor
refactor: simplify DAG evaluation logic
docs: update API endpoint documentation
test: add integration tests for trigger system
chore: upgrade tokio to 1.40
```

## Pull Request Process

1. Run `just ci` to ensure all checks pass
2. Update documentation if your change affects the public API
3. Write a clear PR description explaining the **why**, not just the **what**
4. Link any related issues

## Reporting Issues

- Use [GitHub Issues](https://github.com/orbflow-dev/orbflow/issues)
- Include steps to reproduce, expected vs actual behavior, and environment details
- For security vulnerabilities, see [SECURITY.md](SECURITY.md) (do not open a public issue)

## Architecture

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for an overview of the codebase structure. The project follows a ports-and-adapters pattern where `orbflow-core` defines all domain types and traits, and other crates implement adapters.

## License

By contributing, you agree that your contributions will be licensed under the [AGPL-3.0-or-later](LICENSE) license.
