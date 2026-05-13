# Sing Language Contract

Sing is a minimum-token, native-speed, AI-first systems programming language.

The non-negotiable design goal is to minimize total LLM token spend across source, diagnostics, IR, repair loops, docs, and generated tooling output while preserving performance and systems-language control.

## Token-Economy Law

Every feature must reduce or justify total AI token cost.

Sing measures token economy with these stable fields:

- `bytes`: UTF-8 source bytes.
- `chars`: Unicode scalar count.
- `lexemes`: deterministic language-ish lexical units.
- `llm_tokens`: deterministic model-independent proxy, currently punctuation plus roughly four characters per word token.
- `total`: weighted score: `bytes + chars + lexemes*4 + llm_tokens*8`.

The proxy is intentionally simple and deterministic. A later model-specific tokenizer may be added, but it must not replace these stable baseline fields.

Compiler output rules:

- Default machine output is compact JSON.
- Pretty prose is optional and never the only output.
- Diagnostics use short stable codes.
- IR carries rich meaning so AI tools do not repeatedly infer semantics from tiny syntax.
- Minification must preserve semantic equivalence.
- Features that save human typing but increase AI repair cost are rejected.

## File Model

- Source files use `.sg`.
- A file contains zero or more top-level items.
- `;` separates top-level items and statements.
- Whitespace is insignificant outside strings and chars.
- Line comments start with `//`.
- Block comments use `/* ... */`.

## Top-Level Item Tokens

| Token | Meaning |
| --- | --- |
| `m` | module declaration |
| `u` | import/use declaration |
| `a` | type alias |
| `k` | constant |
| `t` | struct-like type declaration |
| `e` | enum declaration |
| `q` | trait declaration |
| `i` | impl declaration |
| `f` | function declaration |
| `x` | extern function declaration |
| `p` | macro call |
| `#` | test declaration |

All top-level item markers are reserved in item position.

## Attributes

Attributes are one-character uppercase markers before items. Attribute clusters lex as separate attrs, so `KV f` is `K`, `V`, then `f`.

| Attr | Contract |
| --- | --- |
| `K` | kernel profile: restricted low-level function suitable for kernel/no-host environments |
| `V` | vectorize: optimizer may prefer vector lowering |
| `P` | parallelize: optimizer may split independent work |
| `C` | C ABI or C layout, depending on item kind |
| `U` | unsafe: may use raw pointers or unsafe operations |
| `H` | no heap: no dynamic allocation or heap-backed literals |
| `D` | no dynamic dispatch |
| `G` | no garbage collection |
| `Z` | no panic |
| `R` | no runtime |
| `I` | inline preference |
| `N` | no-inline preference |
| `O` | optimize aggressively |
| `E` | export symbol |
| `L` | packed layout |
| `S` | structure-of-arrays layout |

Unknown attrs are errors. Attrs are contracts, not comments; compiler phases must either enforce them or reject the program.

Current v1-alpha placement:

- Function attrs: `K V P C U H D G Z R I N O E`
- Extern attrs: `C E`
- Type attrs: `C E L S`
- Enum attrs: `C E`
- Trait/impl attrs: `E`
- Const/macro attrs: `E`
- `I` and `N` conflict.

Current v1-alpha contracts:

- `H`, `D`, `G`, `Z`, and `R` reject heap, dynamic dispatch, GC, panic, and runtime/system effects respectively.
- `K`, `V`, and `P` reject effect families that break kernel, vector, or parallel lowering assumptions.
- `C` rejects generic functions and non-ABI-safe field, parameter, or return types.
- `D` rejects dynamic trait objects in function signatures.
- `Z` rejects runtime contract statements `?` and `~`.
- `P` rejects mutation and mutable borrows.

## Effects

Effects appear after `!` in function signatures.

| Effect | Meaning |
| --- | --- |
| `h` | heap allocation |
| `d` | dynamic dispatch |
| `g` | garbage collection |
| `z` | may panic |
| `fr` | file read |
| `fw` | file write |
| `ir` | stdin/input read |
| `iw` | stdout/output write |
| `nr` | network read |
| `nw` | network write |
| `dr` | device read |
| `dw` | device write |
| `tm` | time access |
| `rn` | randomness |
| `th` | thread creation or synchronization |
| `at` | atomic operation |
| `ff` | foreign function call |
| `bk` | blocking operation |
| `sy` | syscall |

Effects are part of a function type. A caller must declare every effect it may trigger. The current lattice keeps exact effects by default and lets `sy` cover the system-boundary family `fr fw ir iw nr nw dr dw tm rn th ff bk sy`, reducing tokens when a function crosses broad OS/runtime boundaries.

## Primitive Types

| Type | Meaning |
| --- | --- |
| `b` | bool |
| `i1` | signed 8-bit integer |
| `i2` | signed 16-bit integer |
| `i4` | signed 32-bit integer |
| `i8` | signed 64-bit integer |
| `iz` | pointer-sized signed integer |
| `u1` | unsigned 8-bit integer |
| `u2` | unsigned 16-bit integer |
| `u4` | unsigned 32-bit integer |
| `u8` | unsigned 64-bit integer |
| `uz` | pointer-sized unsigned integer |
| `f4` | 32-bit float |
| `f8` | 64-bit float |
| `c` | Unicode scalar char |
| `s` | string view |
| `y` | byte |
| `v` | void/unit |
| `*` | any/dynamic placeholder, only where the current milestone permits it |

Integer suffixes such as `42u4` and `42i8` select literal type. Float suffixes such as `3.14f4` select float type.

## Compound Types

| Syntax | Meaning |
| --- | --- |
| `&T` | shared borrow |
| `&!T` | mutable borrow |
| `^T` | raw pointer |
| `T[]` | slice |
| `T[N]` | fixed array |
| `T?` | optional |
| `T/E` | result with ok `T` and error `E` |
| `(A,B)` | tuple |
| `(A,B)>C` | function type |
| `*Q` | dynamic trait object |

Compound type operators bind tighter than result `/` except where parentheses force grouping.

## Operators

Structural operators:

```text
: arithmetic add
-: arithmetic sub
*: arithmetic mul or dynamic placeholder in type position
/: arithmetic division when between expressions; result type separator in type position; postfix propagation when after an expression
%: remainder
: field/type separator depending on grammar position
;: item/statement separator
,: list separator
.: field/path separator
[]: index, array, slice, generic list
(): call, tuple, params
{}: block, fields, object literal
>: return type marker, return statement, or comparison depending on grammar position
?: optional type, require marker, ternary marker
~: ensure marker or unary not
!: effects marker or mutable ref marker after `&`
&: borrow or boolean and
|: boolean or
^: raw pointer or raw address
=: bind at statement start; equality inside expressions
:=: mutation
\: reserved
_: reserved placeholder identifier
```

Overflow arithmetic:

| Op | Meaning |
| --- | --- |
| `+%` `-%` `*%` | wrapping arithmetic |
| `+|` `-|` `*|` | saturating arithmetic |
| `+?` `-?` `*?` | checked arithmetic returning result/option by context |

Precedence from strongest to weakest:

1. call, index, field
2. unary `-`, `~`, `&`, `&!`, `^`
3. multiply `/`, `*`, `%`, `*%`, `*|`, `*?`
4. add `+`, `-`, `+%`, `-%`, `+|`, `-|`, `+?`, `-?`
5. compare `<`, `<=`, `>`, `>=`, `==`, `!=`, expression `=`
6. `&`
7. `|`
8. ternary `a?b:c`
9. postfix propagation `expr/`

Evaluation order is left-to-right for operands and call arguments.

## Literals

- Decimal integer: `42`
- Hex integer: `0xff`
- Binary integer: `0b1010`
- Float: `3.14`
- String: `"text"`
- Byte string: `b"text"`
- Char: `'a'`
- Bool: `true`, `false`
- None: `none` or milestone shorthand `n`

Escapes are `\n`, `\r`, `\t`, `\0`, `\\`, `\"`, and `\'`. Invalid escapes are diagnostics.

Default literal inference:

- Integer suffixes `i1`, `i2`, `i4`, `i8`, `iz`, `u1`, `u2`, `u4`, `u8`, and `uz` select the exact literal type.
- Float suffixes `f4` and `f8` select the exact literal type.
- Unsuffixed integer defaults to `i4` until context chooses another integer type.
- Unsuffixed float defaults to `f4` until context chooses another float type.
- Strings default to `s`.
- Chars default to `c`.

## Modules And Imports

`m path` declares the current module.

`u path (, path)*` imports modules or symbols. Multi-file resolution maps module paths to files through the package manifest once package support exists.

Visibility:

- v1-alpha has no visibility token.
- File-local checking treats top-level declarations as visible inside the file.
- Cross-file visibility is package-private by default until an export rule is finalized.
- `E` exports an item for ABI/package boundaries.

## Declarations

```sing
a Name=T
k Name:T=expr
t Name{x,y:f4}
e E{Div0,Other:i4}
q Q{f run(x:i4)>v}
i Q T{f run(x:T)>v:0}
f name(args)>ret !effects:body
x c_puts(s:^y)>i4 !ff
p macro args
# name: expr
```

Grouped fields and params desugar into one field or param per name in HIR.

## Expressions

Expressions include identifiers, paths, literals, tuples, object literals, field access, indexing, calls, unary ops, binary ops, ternary, propagation, and lambdas.

Object literals:

```sing
V{x:1,y:2,z:3}
V{x,y,z}
```

Shorthand fields use the field name as an identifier expression.

Enum construction:

- Unit variants may appear as values by variant name when unambiguous.
- Payload variants use the variant construction form defined by the enum payload rule.

Option/result:

- `T?` carries none or a `T`.
- `T/E` carries ok `T` or error `E`.
- Postfix `/` propagates error/none from the current function.

## Statements

| Syntax | Meaning |
| --- | --- |
| `x=e` | bind local or compact expression-bodied equality in expression position |
| `x:T=e` | typed bind |
| `x:=e` | mutate existing place |
| `?e` | require precondition |
| `~e` | ensure postcondition |
| `>e` | return |
| `l i:a..b stmt` | range loop |
| `l cond { ... }` | while loop |
| `b` | break |
| `c` | continue |

`?` and `~` are semantic contracts. In no-panic contexts they must lower to result/error paths or compile-time proofs, not runtime panic.

## Traits And Impl

Traits declare required function signatures. Impl blocks bind a trait to a type and provide items.

Trait lookup rules:

- Exact impl for concrete type wins.
- Generic impls are considered after concrete impls.
- Ambiguous impls are diagnostics.
- Dynamic trait objects use `*Q` and require dynamic-dispatch permission.

## Generics

Generic lists use `[T]` or `[T:Bound]`.

Rules:

- Generic parameters are type-level names.
- Bounds constrain traits or structural requirements.
- Bounds are resolved when declarations are checked; unresolved bounds are diagnostics.
- Calls instantiate generic parameters from argument types and substitute them into return types.
- Monomorphization is the default native-code strategy unless a later backend chooses sharing.
- Generic inference must prefer the shortest unambiguous source form.

## Memory Model

Default values are owned. `&T` creates shared borrows. `&!T` creates exclusive mutable borrows. `^T` creates raw pointers and requires unsafe permission.

Current v1-alpha enforcement:

- Primitive values, refs, raw pointers, slices, function values, and recursively copyable tuples/options/results/arrays are copy.
- Structs, enums, dynamic trait objects, generics, `*`, and unknown values are non-copy until proven otherwise.
- A moved non-copy local cannot be reused.
- Shared borrows may alias other shared borrows.
- Mutable borrows exclude other active borrows.
- Mutation while borrowed is rejected.
- References to local values cannot escape a function.
- `P` rejects mutation and mutable borrows as a conservative data-race rule.
- Raw pointer creation requires `U`.
- Stack placement is preferred.
- Heap allocation requires effect `h` and is forbidden under attr `H`.

## Unsafe, Panic, Runtime

`U` marks an unsafe boundary. Unsafe operations outside `U` are diagnostics.

`Z` marks no-panic. A `Z` item cannot use panic paths, unchecked contract failures, or operations whose failure cannot be represented in the type/effect system.

`R` marks no-runtime. An `R` item cannot depend on startup/runtime services, allocation, unwinding, dynamic dispatch, GC, or hidden syscalls.

Undefined behavior is only possible through explicitly unsafe operations. Safe Sing code must not trigger UB.

## Macro Model

`p` invokes deterministic macros. Macro expansion must be:

- bounded
- hygienic
- reproducible
- diagnostic-rich
- visible in JSON tooling output

Full macro expansion is reserved for a later milestone.

## HIR And MIR

HIR is the typed summary layer. It keeps source compact by expanding grouped params and fields into explicit per-name entries, attaching stable symbol references, and preserving source spans in sidecar metadata.

MIR is the first control-flow IR. It desugars compact statements and expressions into:

- explicit basic blocks
- explicit assign, mutate, require, ensure, and eval operations
- return and branch terminators
- local def/use dataflow summaries
- dead-block summaries
- optimization hooks such as constant folding

MIR exists to save AI repair tokens: tools can inspect explicit control flow and dataflow instead of re-deriving them from minimum-token source.

## Interpreter Oracle

`sing run` is the semantic oracle before native codegen. It executes the compact source directly enough to validate language meaning:

- function calls and returns
- local binds and mutations
- struct literals and field access
- enum unit values
- tuples
- option and result wrapping/propagation
- range and while loops
- `b`/`c` break and continue
- builtins `out`, `sqrt`, and `sum`

The interpreter is intentionally not the final performance story. Its job is to make semantics testable before Cranelift or another native backend exists.

## Native Backend Alpha

`sing build` is the native artifact path. The selected backend contract is `cranelift-alpha`: MIR lowers into a compact backend IR with target triple, ABI, layout table, object artifact, linked executable, and debug metadata.

Current alpha behavior:

- target defaults to the host triple
- ABI is `sing-v1-alpha`
- primitive layouts are emitted as JSON metadata
- backend IR is written as compact text for AI/tool inspection
- an object file and host executable are emitted
- debug metadata records backend, target, source hash, MIR functions, object, and binary paths

The first executable is a deterministic native launcher backed by the interpreter oracle while direct Cranelift machine-code emission matures. This preserves the north star: tiny Sing source produces rich machine-readable IR now, with a clear path to brutal native code without changing source syntax.

## Standard Library Shape

Stdlib APIs must be token-minimal and AI-readable.

Initial modules:

- `io`: input/output
- `math`: numeric operations
- `str`: strings
- `bytes`: bytes/chars
- `slice`: slices/arrays
- `mem`: memory operations
- `fs`: files
- `proc`: process/env
- `time`: time
- `test`: assertions
- `c`: C interop

Short aliases may exist when they reduce total source plus repair token cost.

## Tooling Contract

Commands:

- `sing ast file.sg`: pretty JSON AST
- `sing check file.sg`: compact JSON diagnostics and HIR
- `sing tokens file.sg`: compact JSON token-cost metrics
- `sing fmt file.sg`: canonical formatter
- `sing min file.sg`: canonical minimizer
- `sing run file.sg`: interpreter/oracle execution
- `sing build file.sg`: native build
- `sing test`: tests
- `sing doc`: docs
- `sing repl`: REPL
- `sing explain E0401`: diagnostic explanation

Every command must support stable JSON output. Compact JSON is the default for machine-facing commands.

## Performance Contract

Sing must optimize for:

- minimal source tokens
- native-speed compiled output
- low compile-time surprises
- predictable memory layout
- deterministic tooling
- fast AI repair loops

The interpreter is a semantic oracle, not the final performance target. The current oracle executes `main`, local binds, returns, integer arithmetic, simple calls, and `out`; native codegen must eventually target object files and linked binaries.
