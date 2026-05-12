# Complete Sing Language Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build Sing from parser-plus-semantic-checker into a complete, performant, versatile, minimum-token programming language for LLM-first development.

**Architecture:** Sing grows through executable compiler slices: language contract, parser V2, semantic graph, full type/effect/memory checking, HIR/MIR/IR, interpreter oracle, then native backend and production tooling. Each slice must preserve the token-economy law: compact source, compact JSON machine output, rich IR, stable IDs, and measurable token-cost regressions.

**Tech Stack:** Rust 2021 workspace, serde/serde_json for stable machine surfaces, clap for CLI, insta for snapshots, optional future Cranelift backend, and repository-local conformance fixtures.

---

### Task 1: Spec Lockdown And Token-Cost Model

**Files:**
- Create: `LANGUAGE.md`
- Create: `crates/sing_token/Cargo.toml`
- Create: `crates/sing_token/src/lib.rs`
- Create: `crates/sing_token/tests/token_cost.rs`
- Modify: `Cargo.toml`
- Modify: `crates/sing_cli/Cargo.toml`
- Modify: `crates/sing_cli/src/main.rs`
- Modify: `README.md`
- Modify: `ROADMAP.md`
- Modify: `SPEC.md`

- [x] **Step 1: Write failing token-cost tests**

```rust
use sing_token::{compare_snippets, token_cost, TokenCost};

#[test]
fn sing_dot_product_is_cheaper_than_common_equivalents() {
    let sing = r#"KV f dot(a,b:&f4[])>f4:?a.len=b.len;sum(a*b)"#;
    let rust = r#"fn dot(a:&[f32],b:&[f32])->f32{assert_eq!(a.len(),b.len());a.iter().zip(b).map(|(x,y)|x*y).sum()}"#;
    let c = r#"float dot(float*a,float*b,size_t n){float s=0;for(size_t i=0;i<n;i++){s+=a[i]*b[i];}return s;}"#;
    let zig = r#"fn dot(a:[]const f32,b:[]const f32)f32{std.debug.assert(a.len==b.len);var s:f32=0;for(a,b)|x,y|s+=x*y;return s;}"#;
    let go = r#"func dot(a,b[]float32)float32{if len(a)!=len(b){panic("len")}var s float32;for i:=range a{s+=a[i]*b[i]}return s}"#;
    let python = r#"def dot(a,b): assert len(a)==len(b); return sum(x*y for x,y in zip(a,b))"#;

    let report = compare_snippets([
        ("Sing", sing),
        ("Rust", rust),
        ("C", c),
        ("Zig", zig),
        ("Go", go),
        ("Python", python),
    ]);

    let sing_cost = report.iter().find(|row| row.name == "Sing").unwrap().cost.total;
    for lang in ["Rust", "C", "Zig", "Go", "Python"] {
        let other = report.iter().find(|row| row.name == lang).unwrap();
        assert!(sing_cost < other.cost.total, "Sing should beat {lang}");
    }
}

#[test]
fn token_cost_counts_bytes_chars_lexemes_and_llmish_tokens() {
    let cost = token_cost(r#"m H;f main()>v:0"#);
    assert_eq!(cost.bytes, 16);
    assert_eq!(cost.chars, 16);
    assert!(cost.lexemes > 0);
    assert!(cost.llm_tokens > 0);
    assert!(cost.total >= cost.llm_tokens);
}
```

- [x] **Step 2: Run test to verify it fails**

Run: `cargo test -p sing_token`

Expected: FAIL because package `sing_token` does not exist.

- [x] **Step 3: Implement `sing_token`**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenCost {
    pub bytes: usize,
    pub chars: usize,
    pub lexemes: usize,
    pub llm_tokens: usize,
    pub total: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnippetCost {
    pub name: String,
    pub cost: TokenCost,
}

pub fn token_cost(src: &str) -> TokenCost {
    // Deterministic proxy until a model-specific tokenizer is selected.
}

pub fn compare_snippets<const N: usize>(snippets: [(&str, &str); N]) -> Vec<SnippetCost> {
    // Sort ascending by total cost.
}
```

- [x] **Step 4: Wire CLI**

Run: `cargo run -p sing_cli -- tokens examples/hello.sg`

Expected: compact JSON with `bytes`, `chars`, `lexemes`, `llm_tokens`, and `total`.

- [x] **Step 5: Write `LANGUAGE.md`**

Include exact rules for tokens, operators, attrs, effects, types, modules, imports, visibility, literals, construction, propagation, evaluation order, unsafe/no-panic/no-runtime behavior, and token-economy requirements.

- [x] **Step 6: Verify**

Run:

```bash
cargo fmt --all -- --check
cargo check
cargo test
cargo run -q -p sing_cli -- tokens examples/hello.sg
```

Expected: all commands pass, and the token command prints valid compact JSON.

### Task 2: Parser V2

**Files:**
- Modify: `crates/sing_ast/src/lib.rs`
- Modify: `crates/sing_parse/src/lexer.rs`
- Modify: `crates/sing_parse/src/parser.rs`
- Modify: `crates/sing_parse/tests/examples.rs`
- Create: `crates/sing_parse/tests/negative.rs`
- Create: `crates/sing_parse/tests/escapes.rs`

- [x] Add `Spanned<T>` wrappers or parallel span tables without breaking stable AST JSON.
- [x] Add parser error expected-token sets.
- [x] Add recovery so one bad item does not hide later item diagnostics.
- [x] Add negative parser tests for missing delimiters and bad statements.
- [x] Add string, char, and byte-string escape tests.
- [ ] Add conformance fixtures that assert parser output remains stable after formatting.

### Task 3: Real Semantic Model

**Files:**
- Split: `crates/sing_sem/src/lib.rs`
- Create: `crates/sing_sem/src/module.rs`
- Create: `crates/sing_sem/src/symbol.rs`
- Create: `crates/sing_sem/src/resolve.rs`
- Create: `crates/sing_sem/src/types.rs`
- Create: `crates/sing_sem/tests/modules.rs`

- [x] Add stable `SymbolId`.
- [x] Add namespaces for modules, types, traits, values, variants, macros.
- [x] Add multi-file `check_package`.
- [x] Add real import graph resolution for imported module function signatures.
- [x] Add cycle diagnostics.
- [ ] Add alias and const resolution.
- [ ] Add trait and impl lookup.

### Task 4: Full Type Checker

**Files:**
- Modify: `crates/sing_sem/src/types.rs`
- Create: `crates/sing_sem/src/infer.rs`
- Create: `crates/sing_sem/tests/type_infer.rs`
- Create: `crates/sing_sem/tests/generics.rs`

- [x] Add inference variables and unification.
- [x] Add generic params and bounds.
- [x] Add literal inference.
- [x] Add numeric coercions.
- [x] Add result/option compatibility.
- [x] Add ref/raw pointer checking.
- [x] Add control-flow return and reachability checking.
- [x] Add precise `*`/Any semantics.

### Task 5: Effects, Attrs, And Memory Model

**Files:**
- Create: `crates/sing_sem/src/effects.rs`
- Create: `crates/sing_sem/src/attrs.rs`
- Create: `crates/sing_sem/src/memory.rs`
- Create: `crates/sing_sem/tests/effects.rs`
- Create: `crates/sing_sem/tests/memory.rs`

- [ ] Add effect lattice and propagation.
- [ ] Enforce every v1 effect.
- [ ] Enforce every v1 attr.
- [ ] Add ownership, move, copy, and drop rules.
- [ ] Add shared and mutable borrow rules.
- [ ] Add raw pointer and unsafe-boundary rules.
- [ ] Add escape, aliasing, and data-race diagnostics.

### Task 6: HIR, MIR, IR

**Files:**
- Modify: `crates/sing_hir/src/lib.rs`
- Create: `crates/sing_mir/Cargo.toml`
- Create: `crates/sing_mir/src/lib.rs`
- Create: `crates/sing_mir/tests/lower.rs`

- [ ] Add rich HIR nodes with spans and symbols.
- [ ] Desugar compact syntax.
- [ ] Add MIR control-flow graph.
- [ ] Add dataflow framework.
- [ ] Add constant folding.
- [ ] Add dead code and reachability checks.
- [ ] Add optimization hooks.

### Task 7: Interpreter Oracle

**Files:**
- Create: `crates/sing_interp/Cargo.toml`
- Create: `crates/sing_interp/src/lib.rs`
- Create: `crates/sing_interp/tests/run_examples.rs`
- Modify: `crates/sing_cli/src/main.rs`

- [x] Add `sing run`.
- [x] Execute core expressions, local binds, returns, and simple function calls.
- [x] Add stdlib builtins for `out` and `sqrt`.
- [ ] Execute structs, enums, loops, result/option flow.
- [ ] Add stdlib builtin `sum`.
- [x] Use interpreter outputs as semantic conformance fixtures for `hello`.

### Task 8: Native Backend

**Files:**
- Create: `crates/sing_codegen/Cargo.toml`
- Create: `crates/sing_codegen/src/lib.rs`
- Create: `crates/sing_codegen/tests/build.rs`
- Modify: `crates/sing_cli/src/main.rs`

- [ ] Select Cranelift as the first native backend unless the spec changes.
- [ ] Define ABI and data layouts.
- [ ] Lower MIR to backend IR.
- [ ] Emit object files.
- [ ] Link binaries.
- [ ] Add debug info and target triples.

### Task 9: Runtime, Stdlib, Tooling, Package System, Release

**Files:**
- Create: `stdlib/`
- Create: `crates/sing_pkg/`
- Create: `crates/sing_fmt/`
- Create: `crates/sing_lsp/`
- Create: `.github/workflows/ci.yml`

- [ ] Add runtime/no-runtime model.
- [ ] Add token-minimal stdlib.
- [ ] Add `fmt`, `min`, `test`, `doc`, `repl`, `explain`, `build`.
- [ ] Add manifest, lockfile, workspaces, dependency resolution, and reproducible builds.
- [ ] Add CI, clippy, release binaries, install script, conformance suite, performance suite, and token benchmark suite.
