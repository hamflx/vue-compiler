# Architecture

This document is the release-facing architecture map for the Rust Vue compiler. The detailed design remains split across `docs/1.RUST_VUE_COMPILER_DESIGN.md` and the deterministic AST/HIR/MIR contract in `docs/3.AST_HIR_MIR_DESIGN.md`.

## Goals

- Support Vue 2.6, Vue 2.7, and Vue 3 compiler APIs from one Rust workspace.
- Preserve official observable behavior for public AST projection, diagnostics, source locations, source maps, render/code strings, options, and package API shape.
- Keep AST/HIR/MIR structure governed only by `docs/3.AST_HIR_MIR_DESIGN.md`.
- Keep conformance reporting honest by distinguishing `rust-backed`, `mixed`, and `shim-backed` coverage.

## Layering

```text
source file / template / SFC
  -> vuec_source spans and source maps
  -> vuec_html tokenizer / CST data
  -> Vue 2 or Vue 3 AST arena in vuec_ast
  -> vuec_js side store for template and script JavaScript
  -> HIR lowering with LoweringMap edges
  -> target-split MIR: Vue2Mir, Vue3DomMir, Vue3SsrMir, VaporMir
  -> vuec_codegen output, diagnostics, and source maps
  -> Rust API, CLI, NAPI, WASM, and official package-name aliases
```

Vue 2 and Vue 3 share infrastructure, not final semantics. Parser options, filters, slots, static optimization, patch flags, DOM transforms, SSR codegen, and SFC script/style behavior stay in their version-specific crates.

## Workspace Map

| Area | Crates / packages | Responsibility |
| --- | --- | --- |
| Source and diagnostics | `vuec_source`, `vuec_diagnostics` | Source identity, spans, code frames, diagnostic payloads, rendering. |
| Parsing infrastructure | `vuec_html`, `vuec_js`, `vuec_ast` | Template tokenization, JS/Oxc side store, CST/AST/HIR/MIR arena structures. |
| Passes and output | `vuec_pass`, `vuec_codegen` | Transform context, helper tracking, code writer, source map artifacts. |
| Vue 2 compiler | `vuec_vue2` | Vue 2.6 / 2.7 template parse, optimize, codegen, warnings, filters, model, slots. |
| Vue 3 compiler | `vuec_vue3_core`, `vuec_vue3_dom`, `vuec_vue3_ssr`, `vuec_vue3_asset` | Vue 3 parser, transforms, DOM/SSR targets, asset URL/srcset transforms. |
| SFC and style | `vuec_sfc`, `vuec_style` | Vue 2.7 / Vue 3 SFC parse, compileTemplate, compileScript, compileStyle, scoped CSS and CSS variables. |
| Distribution | `vuec_cli`, `vuec_napi`, `vuec_wasm`, `packages/native`, `packages/wasm` | CLI, Node native binding, npm loader/platform packages, wasm-bindgen package. |
| Verification | `xtask`, `vuec_node_bridge`, `vuec_runtime_tests` | Official test preparation, API/output/option/conformance gates, Node bridge, runtime smoke helpers. |

## AST / HIR / MIR Contract

`AstDocument<K>` is the internal arena container for all compiler tree documents. Internal nodes are referenced through `NodeId`; boxed nested nodes are only allowed in public projection data. The published structure names are concrete and target-specific:

- `Vue2Ast`
- `Vue3Ast`
- `HIR`
- `Vue2Mir`
- `Vue3DomMir`
- `Vue3SsrMir`
- `VaporMir`

Lowering records explicit AST-to-HIR and HIR-to-MIR edges in `LoweringMap`. DOM and SSR MIR documents are separate targets; SSR output must not be derived from DOM MIR. Template JavaScript is registered in `vuec_js::JsAstStore` and referenced by typed IDs rather than treated as anonymous strings in semantic structures.

## Public Projection

Official compiler APIs observe AST shape, expression content, source locations, helper names, warnings, and output strings. Rust internals may use arena documents, but public projection must preserve official differences such as Vue 3 directive `arg`, `exp`, `modifiers`, `content`, `loc`, and `isStatic`.

The compatibility aliases are allowed to hydrate/dehydrate official-shaped AST values across the Node bridge. That adapter work does not replace Rust parser, transform, lowering, or codegen ownership.

## Entry Points

| Surface | Primary implementation | Verification |
| --- | --- | --- |
| Rust crates | `vuec_vue2`, `vuec_vue3_core`, `vuec_vue3_dom`, `vuec_vue3_ssr`, `vuec_sfc`, `vuec_style` | Rust unit tests, output contracts, focused official conformance. |
| CLI | `vuec_cli` | `cargo xtask verify-cli`. |
| NAPI | `vuec_napi`, `packages/native`, `packages/native-platforms/*`, `packages/native-aliases/*` | `cargo xtask verify-napi`, `verify-napi-alias`, `verify-napi-api`, `verify-napi-platform`, NAPI option/output/conformance gates. |
| WASM | `vuec_wasm`, `packages/wasm` | `cargo xtask verify-wasm`, `verify-wasm-browser`, `verify-wasm-wasi`. |
| Official conformance | `xtask`, generated alias packages, `vuec_node_bridge` | `cargo xtask run-conformance --all`, `cargo xtask summarize-compat --locked`. |

## Compatibility Harness Boundary

`xtask/src/compat.rs` prepares official tests, generates import aliases, runs Vitest/Jasmine, records reports, and provides package/API adapter glue. Changes there must be classified as one of:

- import/API adapter
- runner support
- AST hydration/dehydration
- temporary semantic shim with a Rust migration plan

Compiler semantics such as `processExpression`, `transformExpression`, `transformElement`, `processIf`, `processFor`, `transformText`, `buildProps`, and `generate` only count as Rust compiler completion when they are implemented through Rust parser/transform/lowering/codegen or Rust-backed bridge/API paths.

## Conformance Evidence

Official conformance reports must keep coverage labels:

- `rust-backed`: the pass/fail signal exercises Rust compiler implementation.
- `mixed`: official source, JavaScript adapter code, callbacks, or runner shims participate with Rust code.
- `shim-backed`: the pass/fail signal is satisfied by shim behavior rather than Rust compiler semantics.

Only `rust-backed` coverage, or focused checks that explicitly validate Rust compiler output, can be used as compiler completion evidence. Mixed and shim-backed coverage remain useful compatibility signals, but release notes and progress docs must label them as such.

## Release Gates

The release path is intentionally explicit:

- `cargo xtask verify-official-lock`
- `cargo xtask verify-release-docs`
- `cargo xtask verify-crate-metadata`
- `cargo xtask run-output-contract --all`
- `cargo xtask run-conformance --all`
- `cargo xtask summarize-compat --locked`
- CLI, NAPI, WASM, benchmark, dry-run, and install smoke gates from `docs/RELEASE_CHECKLIST.md`
