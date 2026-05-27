# API Reference

This document describes the release-facing API surface for the Rust Vue compiler
stack. It is a user and package boundary reference; the internal AST/HIR/MIR
schema remains governed by `docs/3.AST_HIR_MIR_DESIGN.md`, and this document
does not redefine those structures.

## Rust Crate APIs

Rust crates expose the compiler entry points used by the CLI, NAPI package,
WASM package, and compatibility harness. Lower-level public projection helpers
are compatibility support APIs and must continue to preserve the public
projection rules from the AST/HIR/MIR design.

| Crate | Release-facing APIs | Purpose |
| --- | --- | --- |
| `vuec_vue2` | `compile`, `compile_to_functions`, `compile_ssr`, `generate_code_frame`, `Vue2Compiler`, `Vue2CompileOptions`, `Vue2CompiledResult`, `Vue2FunctionResult` | Vue 2 template compilation, function wrapper output, SSR output, and official-style code frames. |
| `vuec_vue3_core` | `base_compile`, `compile_dom`, `compile_ssr`, `generate_public_ast`, `TemplateSource`, `Vue3CompilerOptions`, `CodegenResult` | Vue 3 core parse/transform/codegen entry points, DOM/SSR target dispatch, public AST projection, and source metadata. |
| `vuec_vue3_dom` | `parse`, `compile`, `DomCompiler`, `DomCompilerOptions`, `AssetUrlOptions` | Vue 3 DOM template parsing/normalization, DOM codegen, asset URL handling, and incremental DOM AST cache access. |
| `vuec_vue3_ssr` | `compile`, `SsrCompilerOptions`, `SsrCompileResult`, `AssetUrlOptions` | Vue 3 SSR template compilation and SSR transform summaries. |
| `vuec_sfc` | `SfcCompiler`, `parse`, `parse_vue27_component`, `compile_template`, `compile_template_source`, `compile_script`, `compile_vue27_script`, `compile_style`, `cache_stats`, `SfcTemplateCompileOptions`, `SfcScriptCompileOptions`, `SfcStyleCompileOptions`, `SfcTemplateCompileResult`, `SfcStyleCompileResult` | Vue 3 and Vue 2.7 SFC descriptor parsing, template/script/style compilation, and descriptor-cache reporting. |
| `vuec_style` | `compile_style`, `rewrite_scoped_selectors`, `collect_css_vars`, `gen_css_var_name`, `rewrite_css_vars`, `StyleCompileOptions`, `StyleCompileResult` | Scoped CSS rewriting, CSS variable collection/rewriting, and style compile result projection. |

All high-level compiler functions return structured result objects instead of
panicking for normal compiler diagnostics. Fatal host errors should remain at the
outer integration layer.

## CLI

The `vuec` binary is the supported command-line interface. It delegates compiler
semantics to the Rust crates and keeps command handling, file I/O, JSON output,
and benchmark orchestration at the CLI boundary.

| Command | Primary use |
| --- | --- |
| `vuec compile-template <INPUT>` | Compile a Vue 2 or Vue 3 template. Use `--target vue2` or `--target vue3`, plus `--json`, `--diagnostics`, `--source-map`, `--map-out`, `--mode`, and `--prefix-identifiers` as needed. |
| `vuec compile-sfc <INPUT>` | Compile an SFC template/script/style bundle through the SFC compiler. Supports `--ssr`, `--json`, `--diagnostics`, `--source-map`, `--map-out`, `--id`, and `--inline-template`. |
| `vuec compile-ssr <INPUT>` | Compile a Vue 3 SSR template, or an SFC with `--sfc`. Supports JSON, diagnostics, and source-map output. |
| `vuec compile-batch <INPUT>...` | Compile independent files concurrently while preserving deterministic input-order JSON results. Targets are `vue2-template`, `vue3-template`, `vue3-sfc`, and `vue3-ssr`. |
| `vuec parse-sfc <INPUT>` | Parse an SFC descriptor and optionally emit JSON. |
| `vuec conformance` | Run the configured compatibility/conformance command path from the CLI wrapper. |
| `vuec bench <INPUT>` | Benchmark a representative compile path and emit timing JSON when requested. |

## NAPI

The `@vuec-rs/native` package is the Node/NAPI API. The canonical type surface
is `packages/native/index.d.ts`.

| Export group | Exports |
| --- | --- |
| Introspection | `version`, `apiManifest`, `bindingInfo` |
| Vue 2 | `compileVue2`, `compileToFunctionsVue2`, `compileSsrVue2`, `generateCodeFrameVue2`, `callVue2Bridge`, `rewriteDefaultVue27` |
| Vue 3 core | `baseCompileVue3`, `baseParseVue3`, `generateVue3Core` |
| Vue 3 DOM / SSR | `compileVue3Dom`, `parseVue3Dom`, `compileVue3Ssr` |
| SFC / style | `parseSfc`, `compileSfcTemplate`, `compileSfcTemplateSource`, `compileSfcScript`, `compileVue27SfcTemplate`, `compileVue27SfcScript`, `compileSfcStyle` |
| Official-like aliases | `compile`, `compileToFunctions`, `baseCompile`, `compileDom`, `compileSsr`, `parse`, `compileTemplate`, `compileScript`, `compileStyle` |

NAPI options are passed as JavaScript records and converted at the Rust boundary
into the corresponding compiler option structs. Alias exports intentionally
mirror the official package call shapes where the compatibility layer supports
them.

## WASM

The `@vuec-rs/wasm` package is the browser/Node WASM API. The canonical type
surface is `packages/wasm/index.d.ts`.

| Export group | Exports |
| --- | --- |
| Initialization | `init`, `version` |
| Direct compiler calls | `compileVue2`, `compileVue3Dom`, `compileVue3Ssr`, `parseSfc`, `compileSfcTemplate`, `compileSfcTemplateSource`, `compileSfcScript`, `compileSfcStyle` |
| Official-like aliases | `compile`, `compileDom`, `compileSsr`, `parse`, `compileTemplate`, `compileScript`, `compileStyle` |

`init()` loads the generated wasm-bindgen module before compiler functions are
used in browser or Node environments. JSON-string ABI helpers in the Rust WASM
crate remain an implementation boundary; package consumers should use the typed
JavaScript exports above.

## Official Package-Name Aliases

The compatibility harness also prepares official package-name aliases so API,
option, output, and official conformance checks can execute against the same
package names used by the Vue upstream tests.

| Alias package | Version line |
| --- | --- |
| `vue-template-compiler` | Vue 2.6 and Vue 2.7 template compiler compatibility |
| `vue/compiler-sfc` | Vue 2.7 SFC compiler compatibility |
| `@vue/compiler-core` | Vue 3 core compiler compatibility |
| `@vue/compiler-dom` | Vue 3 DOM compiler compatibility |
| `@vue/compiler-sfc` | Vue 3 SFC compiler compatibility |
| `@vue/compiler-ssr` | Vue 3 SSR compiler compatibility |

These aliases are API compatibility adapters and conformance harness surfaces.
They do not change the coverage classification rule: reports must still label
execution as `rust-backed`, `mixed`, or `shim-backed`, and only qualifying Rust
compiler evidence counts toward Rust parity.

## Verification

Use these gates when changing the release-facing API surface:

```bash
cargo xtask diff-api --all
cargo xtask verify-napi-api
cargo xtask verify-cli
cargo xtask verify-wasm
cargo xtask run-output-contract --all
cargo xtask verify-release-docs
```

`diff-api` validates official API shape compatibility, `verify-napi-api`
validates the maintained NAPI package surface, `verify-cli` and `verify-wasm`
exercise published entry points, and `run-output-contract` checks output/runtime
contracts across official package targets.
