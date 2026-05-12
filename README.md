# Sing

Sing is a minimum-token, native-speed, AI-first systems programming language. The slogan is: tiny source, rich IR, brutal machine code.

The north-star goal is to be the least AI-token-consuming programming language in the world. That means compact syntax is not cosmetic: every compiler surface should preserve tiny source, deterministic lowering, compact machine-readable diagnostics, and rich IR that lets AI tools reason with fewer prompt tokens.

This repository currently contains the v1-alpha parser milestone plus the first semantic checker slice: `.sg` source is lexed, parsed into the stable AST, checked for names/types/effects, lowered to typed HIR, and reported as JSON. Code generation, borrow checking, macro expansion, LLVM, Cranelift, async, GPU support, and package management remain intentionally out of scope.

```sing
m H;u io;f main()>v !iw:out("hi")
```

## Build

```bash
cargo check
cargo test
```

## Print An AST

```bash
cargo run -p sing_cli -- ast examples/dot.sg
cargo run -p sing_cli -- check examples/dot.sg
```

The binary name is `sing`, so an installed build can run:

```bash
sing ast examples/hello.sg
```
