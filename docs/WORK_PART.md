# Work Part

## Current Stage

- [x] Normalize goals and compatibility acceptance criteria.
- [x] Workspace bootstrap.
- [x] Shared source and diagnostic crates.
- [x] Official test harness and lock validation.
- [x] Vue 3 core and SFC skeletons.
- [x] Vue 3 DOM / SSR / style early backend crates.
- [x] Official repository sync and conformance discovery harness.
- [x] Vue 2 compiler skeleton.
- [x] Vue 2 compiler main-path parser / optimizer / codegen expansion.
- [x] Official API manifest generation and diff harness.

## Completed This Round

- Replaced the spec-only `export-api` behavior with a real Node probe that installs exact official npm package versions from `compat/official-revisions.lock` under `target/compat/npm/<version-line>` and records `Object.keys(require(...)).sort()`, export type details, function arity, class/async flags, package version, type declaration path, require status, lock hash, and official revision.
- Changed `cargo xtask export-api --all` to generate both official and Rust-side manifests. Official manifests pass for all seven compiler targets; Rust manifests are generated as `pending` until alias packages exist under `target/compat/rust-alias/<version-line>`.
- Implemented `cargo xtask diff-api --all` as a real field-level manifest comparison with an explicit `compat/api/allowed-diff.json` approval file. It currently fails deterministically because the Rust alias packages are not implemented yet.
- Generated official and Rust API manifest files under `compat/api/` for Vue 2.6 `vue-template-compiler`, Vue 2.7 `vue-template-compiler`, Vue 2.7 `vue/compiler-sfc`, and Vue 3 `@vue/compiler-core`, `@vue/compiler-dom`, `@vue/compiler-ssr`, `@vue/compiler-sfc`.
- Verification this round: `cargo fmt --all --check`, `cargo test -p xtask`, `cargo xtask export-api --all`, and `cargo xtask diff-api --all` as the expected failing alias gate.

## Previous Round

- Replaced the old `vuec_vue2` skeleton with a recursive Vue 2 element AST that preserves raw attrs, processed attrs/props/directives/events, structural directive state, static flags, slot/component/model metadata, diagnostics spans, and public AST projection.
- Implemented Vue 2 main-path parsing for `v-if` / `v-else-if` / `v-else`, `v-for`, `v-pre`, `v-once`, `v-bind`, `v-on`, `v-model`, custom directives, slots, component `is`, inline-template metadata, class/style modules, and validator-like compile option hooks.
- Implemented Vue 2 filter parsing and rewriting to `_f(...)`, interpolation parsing with custom delimiters, event handler generation with key/generic modifiers, DOM/component model assignment helpers, static optimization, static render hoisting, and Vue 2 style codeframe generation.
- Fixed the shared HTML tokenizer so Vue directive attribute names such as `@click.stop`, `#default`, `.prop`, and dynamic argument names are consumed correctly instead of stalling.
- Updated `xtask` version-line parsing so the deterministic plan commands accept `vue2_6`, `vue2_7`, and `vue3` directly, while preserving `vue26` / `vue27` compatibility.
- Generated the initial Vue 2.6 `vue-template-compiler` option matrix and output contract files under `compat/`.
- Verification this round: `cargo test --workspace`, `cargo xtask verify-official-lock`, `cargo xtask run-conformance --suite vue2-compiler`, `cargo xtask generate-option-matrix --version-line vue2_6 --package vue-template-compiler`, and `cargo xtask generate-output-contract --version-line vue2_6 --package vue-template-compiler`.
- `run-conformance --suite vue2-compiler` still reports `pending` because official execution remains discovery-only until the NAPI/alias runner is implemented.

## Earlier Round

- Added `vuec_vue3_dom`, `vuec_vue3_ssr`, and `vuec_style` to the workspace.
- Extended `vuec_ast::Vue3NodeKind::Element` to keep template attributes and self-closing state.
- Reworked `vuec_vue3_core::base_parse` to consume the shared HTML tokenizer and emit text, interpolation, comment, and element nodes.
- Added DOM directive extraction and summaries for `v-html`, `v-text`, `v-show`, `v-model`, `v-on`, `v-bind`, asset URLs, and custom elements.
- Added SSR compile output shape with `ssrRender`, interpolation emit, scope/slotted attributes, and SSR node summaries.
- Added style compilation primitives for scoped selector rewrite, `:deep`, `:slotted`, `:global`, CSS vars, CSS modules, and source map shape.
- Connected `vuec_sfc::compileTemplate` to DOM/SSR backends, `compileStyle` to `vuec_style`, and `compileScript` to Oxc-backed script summaries.
- `xtask sync-official-tests --locked` now checks out the official Vue 2.6 / 2.7 / 3 repositories at the pinned commit SHAs and writes revision metadata with lock hashes.
- `xtask run-conformance --all` now discovers official test files for every suite, emits lock-scoped JSON discovery reports, and fails explicitly when a suite is misconfigured or not synced.
- Added `vuec_vue2` to the workspace with `compile`, `compileToFunctions`, and `compile_ssr` entry points, plus a minimal AST and diagnostic pipeline.
- Workspace, package manifests, and `.cargo` alias were added.
- Shared source, diagnostics, HTML, JS, AST, pass, and codegen crates were scaffolded.
- Vue 3 core and SFC crates were scaffolded with initial parse/compile wrappers.
- `xtask` now includes compatibility-oriented JSON report commands and the official lock file.

## Notes

- This repository started from docs only; no compiler code existed before the current implementation work.
- The new Vue 3 DOM / SSR / style crates are not official-compatible yet. They establish module boundaries, API shapes, and smoke-tested behavior that must be expanded against official compiler tests.
- The expanded Vue 2 compiler is substantially closer to M09, but official Vue 2.6 parity is not proven until `run-conformance` executes the official compiler tests through the Rust alias/NAPI package.
