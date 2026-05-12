# Sing v1-alpha Frozen Spec

Sing is a minimum-token, native-speed, AI-first systems programming language. This repository implements the parser milestone only: source -> lexer -> parser -> AST -> JSON output.

## Hard Constraints

- Do not redesign syntax.
- Do not add features outside v1-alpha.
- Do not implement Rust codegen, borrow checking, full macro expansion, LLVM, Cranelift, async, GPU, package management, or a runtime.
- The milestone succeeds when `sing ast examples/*.sg` prints valid JSON AST.

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
