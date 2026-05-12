# Sing

Sing is a minimum-token, native-speed, AI-first systems programming language. The slogan is: tiny source, rich IR, brutal machine code.

This repository contains the v1-alpha parser milestone only: `.sg` source is lexed, parsed into the stable AST, and printed as pretty JSON. Code generation, borrow checking, macro expansion, LLVM, Cranelift, async, GPU support, and package management are intentionally out of scope.

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
```

The binary name is `sing`, so an installed build can run:

```bash
sing ast examples/hello.sg
```
