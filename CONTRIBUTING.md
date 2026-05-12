# Contributing

Sing v1-alpha is intentionally narrow. Contributions should keep the parser milestone stable and avoid adding later compiler phases.

Before opening changes:

```bash
cargo fmt
cargo check
cargo test
```

Parser changes should include focused tests or snapshot updates for the affected `.sg` syntax.
