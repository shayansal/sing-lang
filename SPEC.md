# Sing v1-alpha Frozen Spec

Sing is a minimum-token, native-speed, AI-first systems programming language. The primary product goal is to be the least AI-token-consuming programming language in the world.

This repository implements the parser milestone and the first semantic checker slice: source -> lexer -> parser -> AST -> resolver/type checker/effect checker -> typed HIR -> JSON diagnostics.

The full language contract is maintained in `LANGUAGE.md`.

## Hard Constraints

- Do not redesign syntax.
- Do not add features outside v1-alpha.
- Do not implement Rust codegen, borrow checking, full macro expansion, LLVM, Cranelift, async, GPU, package management, or a runtime until their own approved milestones.
- The parser milestone succeeds when `sing ast examples/*.sg` prints valid JSON AST.
- The semantic milestone succeeds when `sing check examples/*.sg` exits cleanly with compact JSON check output.

## Token-Economy Design Law

Every technical decision must reduce total AI token cost across reading, writing, checking, and repairing Sing code.

- Source syntax stays tiny.
- Diagnostics use stable short codes plus compact JSON fields.
- HIR is richer than source so tools do not need to infer meaning repeatedly.
- Defaults should be deterministic and machine-readable.
- Human prose is documentation; compiler output favors structured data.
- Token cost is measured with stable `bytes`, `chars`, `lexemes`, `llm_tokens`, and weighted `total` fields.

## Tokens

Top-level item markers are `m`, `u`, `a`, `k`, `t`, `e`, `q`, `i`, `f`, `x`, `p`, and `#`. Uppercase attrs before items are lexed as one-character attrs; `KV f` becomes `Attr("K")`, `Attr("V")`, `Id("f")`.

Primitive types are `b`, `i1`, `i2`, `i4`, `i8`, `iz`, `u1`, `u2`, `u4`, `u8`, `uz`, `f4`, `f8`, `c`, `s`, `y`, `v`, and `*`.

Effects are parsed as effect strings after `!`: `h`, `d`, `g`, `z`, `fr`, `fw`, `ir`, `iw`, `nr`, `nw`, `dr`, `dw`, `tm`, `rn`, `th`, `at`, `ff`, `bk`, and `sy`.

## Implemented Grammar

The parser implements the frozen v1-alpha item grammar for modules, imports, aliases, constants, type declarations, enum declarations, trait declarations, impl declarations, function declarations, extern declarations, macro calls, and tests.

It supports grouped fields and params, generics, returns, effects, colon and block bodies, require/ensure/return statements, binds, mutations, range and while loops, break/continue, primitive and compound types, function calls, indexing, field access, object literals, shorthand object fields, ternary expressions, overflow operators, and postfix result propagation.

## Operator Precedence

From strongest to weakest:

1. call, index, field
2. unary `-`, `~`, `&`, `&!`, `^`
3. multiply `*`, `/`, `%`, `*%`, `*|`, `*?`
4. add `+`, `-`, `+%`, `-%`, `+|`, `-|`, `+?`, `-?`
5. compare `<`, `<=`, `>`, `>=`, `==`, `!=`
6. and `&`
7. or `|`
8. ternary `a?b:c`
9. postfix propagation `expr/`

## Alpha Decisions

- Single `=` is parsed as equality inside expressions so the frozen examples `?a.len=b.len` and `b=0?...` remain valid.
- A statement starting with `ID = expr` is still parsed as a bind, matching the v1-alpha statement grammar.
- Prefix `/Name` in expressions is accepted as a path-like shorthand to keep the frozen `div.sg` example parseable. Full result construction semantics are reserved for later milestones.
- Bare `*` in expression position is represented as `Expr::Missing`, which keeps macro arguments like `a:*` parseable without adding semantic wildcard support.
- Macro arguments are parsed as permissive expression lists; `p name{...}` is represented as an object expression using `name` as the object type.
- `sing check` treats conditions as bool-compatible when they are `b` or numeric. This is an alpha semantic choice in favor of fewer source tokens.
- `sing check` emits compact JSON by default to reduce AI/tooling token load.
- `b"..."` byte strings lex as one literal and currently lower through `Expr::String` to keep the stable AST unchanged until the byte value model lands.
- Parser V2 exposes sidecar spans through `parse_file_spanned` and recovery through `parse_file_recovering`; the stable AST JSON remains unchanged.

## Semantic Model Slice

- Top-level declarations define module, import, type, enum, trait, function, extern, const, macro, and test symbols.
- Imports currently create external module placeholders. Built-in imported names are modeled for `out`, `sqrt`, and `sum`.
- All top-level declarations are visible inside the file. There is no visibility token in v1-alpha.
- Struct object literals must use known fields and include all declared fields.
- Enum unit variants resolve as values of their enum type.
- Function bodies are checked against declared returns and declared effects.
- Generic calls instantiate type parameters from argument types and substitute returns.
- Literal suffixes infer exact primitive types; unsuffixed numeric literals use contextual types when available.
- Tuple, option, result, ref, raw pointer, slice, array, and function types are checked structurally.
- Raw pointer creation requires an unsafe `U` item boundary.
- Statements after terminating control flow are reported as unreachable.
- Bare `*` lowers to HIR `Any`, preserving the compact wildcard contract.
- `sy` is the compact umbrella effect for system-boundary effects.
- Attrs are checked for known markers, placement, conflicts, ABI/layout safety, and effect contracts.
- The alpha memory model rejects use-after-move, borrow conflicts, mutation while borrowed, escaping local refs, and parallel data-race risks.
- HIR lowers grouped params and fields into explicit per-name entries.
- HIR carries sidecar metadata with source spans and stable symbol references.
- MIR lowers functions into explicit basic blocks with operations, terminators, def/use dataflow, dead-block summaries, and optimization hooks.
- MIR constant-folds simple integer arithmetic before later interpreter/codegen milestones consume it.
- `sing run` executes functions, structs, enum unit values, tuples, loops, break/continue, option/result wrapping and propagation, and builtins `out`, `sqrt`, and `sum`.
- `sing build` selects the `cranelift-alpha` backend contract, emits backend IR, primitive layout metadata, an object artifact, debug metadata, and a linked host executable launcher.
- Diagnostics use stable codes:
  - `E0001` parse error
  - `E0101` duplicate symbol
  - `E0102` duplicate field
  - `E0103` duplicate enum variant
  - `E0104` duplicate parameter
  - `E0201` unknown name or path
  - `E0202` unknown type or trait
  - `E0301` invalid field access
  - `E0302` unknown field
  - `E0303` missing field
  - `E0304` invalid index
  - `E0401` type mismatch
  - `E0402` incompatible ternary branches
  - `E0403` invalid condition
  - `E0404` integer requirement
  - `E0405` invalid unary op
  - `E0406` invalid propagation
  - `E0407` wrong arg count
  - `E0408` invalid binary op
  - `E0410` generic instantiation mismatch
  - `E0501` unsafe operation outside `U`
  - `E0502` unreachable statement
  - `E0503` use after move
  - `E0504` borrow conflict
  - `E0505` mutation while borrowed
  - `E0506` escaping local reference
  - `E0507` parallel data-race risk
  - `E0601` unknown effect
  - `E0602` undeclared effect use
  - `E0701` attribute contract violation
  - `E0702` unknown attr
  - `E0703` conflicting attrs
  - `E0704` invalid attr placement
