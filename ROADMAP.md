# Roadmap

## v1-alpha

- Lexer for `.sg` source.
- Parser for the frozen item, type, statement, and expression grammar.
- Stable serde AST.
- `sing ast file.sg` JSON output.
- Parser snapshots for the checked-in examples.
- Token-economy rule: shortest useful source, richest deterministic IR, compact diagnostics.

## v1-alpha.2

- Semantic checker for local name/type/effect validation.
- Stable JSON diagnostics with short error codes.
- Typed HIR summaries for compiler/tooling consumers.
- `sing check file.sg` compact JSON output.

## Later Milestones

- Formatting and minification.
- Macro expansion.
- Borrow checking and effect validation.
- Interpreter/oracle execution for semantic conformance.
- Lowering to IR.
- Native code generation.

No later milestone is implemented in this repository yet.
