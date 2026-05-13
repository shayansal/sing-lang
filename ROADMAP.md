# Roadmap

## v1-alpha

- Lexer for `.sg` source.
- Parser for the frozen item, type, statement, and expression grammar.
- Stable serde AST.
- `sing ast file.sg` JSON output.
- Parser snapshots for the checked-in examples.
- Token-economy rule: shortest useful source, richest deterministic IR, compact diagnostics.
- `sing tokens file.sg` token-cost metrics.
- `LANGUAGE.md` full language contract.

## v1-alpha.2

- Semantic checker for local name/type/effect validation.
- Stable JSON diagnostics with short error codes.
- Typed HIR summaries for compiler/tooling consumers.
- `sing check file.sg` compact JSON output.

## Later Milestones

- Expand direct Cranelift emission beyond constant-return `main`.
- Package dependency fetching beyond local manifests.
- Hygienic user-defined macros beyond deterministic v1 built-ins.
- Full LSP server transport beyond compact document facts.
- Formatter write/check modes.
- Release artifact signing.
- Larger stdlib implementations behind the current token-minimal contracts.

Later milestones are being implemented incrementally on verifiable compiler slices.
