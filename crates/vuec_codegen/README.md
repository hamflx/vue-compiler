# vuec_codegen

Code generation result and source map helpers for the vuec Rust Vue compiler.

This crate is part of the `vue-compiler` workspace and provides shared output structures used by Vue 2, Vue 3 DOM, Vue 3 SSR, SFC, CLI, NAPI, and WASM paths.

It owns the shared codegen foundation:

- `CodeWriter` with pretty, condensed, and exact whitespace modes
- target-aware `EmitOptions` and MIR-first `MirEmitter` contracts
- runtime helper name, alias, import, and local reference mapping
- source-map builders, generated-position lookup, and SFC block map merging
- snapshot-friendly `EmitResult` serialization

Concrete Vue 2, Vue 3 DOM, and Vue 3 SSR emission semantics live in the dialect crates and can use these contracts without depending on AST internals.
