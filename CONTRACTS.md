# Sing Native V1 Contract Lock

Sing's production contract is versioned so AI tools can rely on stable, compact shapes instead of re-inferring compiler behavior.

Current contract:

- Language version: `sing-v1-native-draft.1`
- Contract version: `contract.v1`
- Edition: `2026`
- North star: lowest total AI token spend across source, diagnostics, IR, docs, repair loops, and generated output.

`sing contract` is the single machine-readable authority for these versions.

## Schema Versions

| Surface | Version | Producer |
| --- | --- | --- |
| diagnostics/explain | `diagnostic.v1` | `sing explain`, diagnostic entries |
| check output | `check.v1` | `sing check`, `sing_sem::check_source` |
| HIR | `hir.v1` | checked HIR payloads |
| MIR | `mir.v1` | MIR lowering payloads |
| build output | `build.v1` | `sing build`, `sing_codegen` |
| package output | `package.v1` | `sing pkg`, `sing_pkg` |
| macro expansion | `macro.v1` | `sing expand`, `sing_macro` |
| token cost | `token.v1` | `sing tokens`, token-cost fields |

Machine-facing reports that combine multiple surfaces include a compact `contract` object:

```json
{"language_version":"sing-v1-native-draft.1","contract_version":"contract.v1","schema_version":"check.v1"}
```

## Stable Shapes

Diagnostic:

```json
{"code":"E0401","severity":"error","message":"...","labels":[{"span":{"start":0,"end":1},"message":"..."}],"notes":[]}
```

Check:

```json
{"contract":{"language_version":"sing-v1-native-draft.1","contract_version":"contract.v1","schema_version":"check.v1"},"ok":true,"diagnostics":[],"hir":{}}
```

Build:

```json
{"contract":{"language_version":"sing-v1-native-draft.1","contract_version":"contract.v1","schema_version":"build.v1"},"backend":"cranelift-alpha","abi":"sing-v1-alpha","target":"...","codegen_strategy":"...","direct_native":false,"backend_ir":"...","object_path":"...","direct_object_path":null,"binary_path":"...","debug_path":"...","fallback_reason":"...","layouts":[]}
```

Package:

```json
{"contract":{"language_version":"sing-v1-native-draft.1","contract_version":"contract.v1","schema_version":"package.v1"},"manifest":{},"lock":{},"runtime":{}}
```

Macro expansion:

```json
{"contract":{"language_version":"sing-v1-native-draft.1","contract_version":"contract.v1","schema_version":"macro.v1"},"ok":true,"generated_items":0,"source":"","original_cost":{},"expanded_cost":{},"expansion_ratio_x100":100,"expansions":[]}
```

Token cost stays intentionally tiny:

```json
{"bytes":0,"chars":0,"lexemes":0,"llm_tokens":0,"total":0}
```

## Token-Economy Gate

Every new language feature or public tooling shape must document:

- source-token impact
- diagnostic-token impact
- IR/tooling clarity impact
- likely AI repair-loop impact

Features that shorten human typing but increase total AI reasoning or repair tokens are rejected.
