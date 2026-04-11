# ADR-003: CEL for Expression Evaluation

## Status
Accepted

## Date
2025-12

## Context
Workflows need conditional edges (branching logic) and dynamic input mapping (transforming data between nodes). The expression language must be:

1. **Safe** -- no filesystem access, network calls, or infinite loops
2. **Fast** -- evaluated on every edge/node in the critical path
3. **Deterministic** -- same inputs always produce same outputs

### Alternatives Considered

- **JavaScript (V8/QuickJS)**: Turing-complete, sandbox escape risks, heavy runtime. Overkill for expressions.
- **Lua**: Lighter than JS but still Turing-complete. Sandbox hardening is error-prone.
- **JSONPath + template literals**: Too limited -- no arithmetic, no boolean logic, no function calls. Can't express `vars.amount > 100 && nodes.check.result == true`.
- **Jsonata**: Powerful but niche, poor Rust ecosystem support, unfamiliar syntax for most developers.
- **CEL (chosen)**: Purpose-built for policy/config evaluation. Non-Turing-complete by design. C-like syntax familiar to most developers.

## Decision
Use [Google's Common Expression Language](https://cel.dev/) (CEL) via the `cel-interpreter` and `cel-parser` Rust crates (v0.10):

- The `orbflow-cel` crate provides an evaluator with compiled program caching for performance
- Values prefixed with `=` in the frontend UI are CEL expressions (e.g., `=vars.amount * 1.1`)
- Used for: conditional edge evaluation, dynamic node input mapping, filter/transform node logic
- Available context variables: `vars` (workflow input), `nodes` (completed outputs), `node` (source node), `trigger` (metadata)

## Consequences

**Benefits:**
- Safe by design -- guaranteed termination in bounded time, no side effects, no sandbox escapes
- Fast evaluation with expression caching (compile once, evaluate many times)
- Type-safe with compile-time checking before execution
- C-like syntax (`&&`, `||`, `.`, `[]`) is immediately readable

**Trade-offs:**
- Limited expressiveness by design -- complex data transformations may need dedicated transform nodes instead of inline expressions
- Dependency on `cel-interpreter`/`cel-parser` crates (evaluate maintenance health periodically)
- Learning curve for users unfamiliar with CEL (mitigated by C-like syntax and in-app documentation)
- The `=` prefix UX convention needs clear documentation to avoid user confusion

See: `orbflow-cel::evaluator`, `orbflow-engine` (CEL integration), [CEL spec](https://cel.dev/)
