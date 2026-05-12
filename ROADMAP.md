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

- Parser V2 spans and recovery.
- Real multi-file semantic model.
- Full type inference, generics, effects, attrs, and memory checking.
- HIR to MIR and optimizer foundations.
- Interpreter/oracle execution beyond the current `main`/arithmetic/`out` foundation.
- Formatting and minification.
- Macro expansion.
- Borrow checking and effect validation.
- Lowering to IR.
- Native code generation.

Later milestones are being implemented incrementally on verifiable compiler slices.
