# Sing

Sing is a minimum-token, native-speed, AI-first systems programming language. The slogan is: tiny source, rich IR, brutal machine code.

The north-star goal is to be the least AI-token-consuming programming language in the world. That means compact syntax is not cosmetic: every compiler surface should preserve tiny source, deterministic lowering, compact machine-readable diagnostics, and rich IR that lets AI tools reason with fewer prompt tokens.

This repository currently contains the v1-alpha parser milestone plus semantic, IR, interpreter, native-alpha build, tooling/package, and deterministic macro-expansion slices: `.sg` source is lexed, parsed into the stable AST, checked for names/types/effects/memory rules, lowered to typed HIR with metadata, lowered further into MIR control-flow/dataflow summaries, executable through the interpreter oracle, buildable into Cranelift object artifacts for the direct subset or a host native launcher fallback, format/minified for token cost, documented as compact JSON, expanded from compact `p` macros, and packaged with a reproducible lock report. LLVM, async, and GPU support remain intentionally out of scope.

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
cargo run -p sing_cli -- contract
cargo run -p sing_cli -- check examples/dot.sg
cargo run -p sing_cli -- tokens examples/dot.sg
cargo run -p sing_cli -- run examples/hello.sg
cargo run -p sing_cli -- build examples/hello.sg
cargo run -p sing_cli -- fmt examples/dot.sg
cargo run -p sing_cli -- min examples/dot.sg
cargo run -p sing_cli -- test tests/conformance/tooling.sg
cargo run -p sing_cli -- doc examples/hello.sg
cargo run -p sing_cli -- explain E0401
cargo run -p sing_cli -- expand examples/web.sg
cargo run -p sing_cli -- pkg .
```

The binary name is `sing`, so an installed build can run:

```bash
sing ast examples/hello.sg
sing contract
sing run examples/hello.sg
sing build examples/hello.sg
sing min examples/dot.sg
sing expand examples/web.sg
sing pkg .
```
