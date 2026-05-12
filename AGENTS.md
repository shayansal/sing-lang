# Agent Rules

- Do not redesign syntax.
- Do not add unrequested features.
- Parser milestone only.
- All changes must pass tests.
- Keep AST stable.
- Do not implement codegen, borrow checking, LLVM, Cranelift, async, GPU, package management, or runtime work unless a later approved spec asks for it.
- Optimize for Sing's north star: least AI-token-consuming programming language in the world.
- Prefer compact stable JSON for machine/tool output.
