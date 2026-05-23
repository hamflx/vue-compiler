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
- [x] Rust npm alias bridge and API manifest parity.
- [x] Vue 3 SFC official output-contract parity and option-matrix closure.
- [x] AST / HIR / MIR base arena migration and lowering pipeline alignment.

## Completed This Round

- Added deterministic official test runner dependency provisioning to `cargo xtask run-conformance`: runner packages are resolved from the synced official lock/manifests and installed into the lock-scoped official npm probe roots before readiness is evaluated.
- Corrected Vue 2.7 conformance runner classification from the old Vue 2.6 Jasmine/Babel path to the actual Vue 2.7 Vitest/esbuild/TypeScript test setup.
- Fixed yarn lock dependency lookup to match exact package names, preventing `eslint-plugin-jasmine` from being mistaken for `jasmine`; added regression coverage for pnpm and yarn runner dependency resolution.
- Current conformance status: all seven suites discover official test files, pass alias smoke, and report `runner is ready to execute`; they still remain `pending` because full official spec execution has not yet been wired to the runner.
- Verification this round: `cargo fmt --all --check`, `cargo test --workspace`, `cargo xtask run-conformance --all`, `cargo xtask summarize-compat --locked`, `cargo xtask diff-api --all`, `cargo xtask run-option-matrix --all`, and `cargo xtask run-output-contract --all`.
- Expanded `vuec_ast` from the earlier enum placeholders into first-class deterministic AST/HIR schema structs matching `docs/3.AST_HIR_MIR_DESIGN.md`: Vue 2 root/element/text/expression/comment/filter/model/directive/event structures, Vue 3 root/element/text/comment/interpolation/compound/if/for/text-call structures, and shared HIR semantic structures.
- Migrated `vuec_vue3_core`, `vuec_vue3_dom`, `vuec_vue3_ssr`, `vuec_vue2`, and `vuec_node_bridge` to the new tuple-variant AST schema while preserving the current compatibility harness behavior through compatibility constructors and `TemplateAttribute` projection.
- Fixed a real `TemplateSource.base_offset` span bug in Vue 3 parsing: closing-tag-updated element spans now keep absolute offsets into the original `.vue` file instead of mixing absolute starts with template-local ends.
- Tightened `docs/2.DEVELOPMENT_PLAN.md` so AST/HIR/MIR development and acceptance refer directly to the concrete schemas from `docs/3.AST_HIR_MIR_DESIGN.md`, including Vue2Ast/Vue3Ast field semantics and target-split MIR.
- Verification this round: `cargo fmt --all --check`, `cargo test --workspace`, `git diff --check`, `cargo xtask diff-api --all`, `cargo xtask run-option-matrix --all`, `cargo xtask run-output-contract --all`, `cargo xtask run-conformance --all`, and `cargo xtask summarize-compat --locked`.
- Current compatibility status remains honest: API, options, output contract, and lock validation pass for all seven targets; full official conformance is still `pending` because runner dependencies are not yet provisioned for the official test suites.
- Upgraded `cargo xtask run-conformance --all` from pure discovery reports to discovery plus execution-readiness reports: each suite now records required Rust alias package requests, required official test runner dependencies, missing runner dependencies, and a `blocked` / `ready` execution state.
- Added per-suite conformance alias smoke execution using the generated Rust alias packages and existing Node bridge, so every suite now has a real compiler entry smoke result in `target/conformance/<lock-hash>/<suite>.json`.
- Current conformance status: all seven suites discover official test files and pass alias smoke execution, but remain `pending` because the current pinned npm probe installs do not include the full official test runner dependencies (`vitest`, `esbuild`, `typescript`, `@babel/register`, `jasmine`).
- Verification this round: `cargo fmt --all --check`, `cargo test -p xtask`, `cargo xtask run-conformance --all`, and `cargo xtask summarize-compat --locked`.
- Replaced the scaffolded `cargo xtask summarize-compat --locked` output with a real artifact aggregator that reads official/Rust API manifests, option-matrix results, output-contract results, conformance reports, and lock validation status.
- Added deterministic status parsing for nested conformance JSON (`counts`, target rows, output checks), including pass/pending/fail propagation and regression tests for nested rows, checks, and discovery-only reports.
- Current summary status: `cargo xtask summarize-compat --locked` reports API, option matrix, output contract, lock validation, and conformance alias smoke as `pass` for all seven compiler targets, while keeping each target `pending` because full official spec execution is still blocked on runner dependencies.
- Verification this round: `cargo fmt --all --check`, `cargo test -p xtask`, and `cargo xtask summarize-compat --locked`.
- Fixed the output-contract runner for Vue 2.7 `vue/compiler-sfc` so it uses the official single-object SFC parse API (`parse({ source, filename })`) while preserving Vue 3's `parse(source, options)` path.
- Added Vue 2.7 SFC-specific template/style source extraction in the output-contract probe, matching the public 2.7 API shape already used by the option matrix runner.
- Added an `xtask` regression test that locks the Vue 2.7 SFC output-contract version context and object-style parse call.
- Current output-contract status: `cargo xtask run-output-contract --all` has no failing checks; all seven targets report `5/5` checks passed.
- Verification this round: `cargo fmt --all --check`, `cargo test -p xtask`, `cargo xtask run-output-contract --version-line vue2_7 --package vue --entry vue/compiler-sfc`, `cargo xtask run-output-contract --all`, and `cargo xtask run-option-matrix --all`.
- Updated `vuec_ast` toward the deterministic AST/HIR/MIR design: `AstDocument.root` is now a required `NodeId`, tree edges can be validated with `validate_tree`, child attachment now maintains parent/index metadata, and root construction always creates an explicit root node.
- Added AST/IR contract scaffolding for CST nodes, `PublicProjection`, nested public snapshots, explicit `LoweringMap` edge recording/lookups, `NodeSpan::generated` / `NodeSpan::missing`, HIR without runtime-helper/codegen-call variants, and target-split `Vue2Mir` / `Vue3DomMir` / `Vue3SsrMir` / `VaporMir`.
- Updated Vue 2 public AST projection, Vue 3 core AST construction/generation, DOM side-effect traversal, pass walking, and Node bridge parse projection to use the deterministic root and proper `attach_child` parent/index maintenance.
- Added `vuec_ast` tests for deterministic public projection, tree invariants, lowering map lookup, target-split MIR, and the absence of helper/call variants in HIR.
- Verification this round: `cargo fmt --all --check`, `cargo check -p vuec_ast -p vuec_pass -p vuec_js -p vuec_vue3_core -p vuec_vue3_dom -p vuec_vue3_ssr -p vuec_vue2 -p vuec_node_bridge`, `cargo test -p vuec_ast -p vuec_pass -p vuec_js`, `cargo test -p vuec_vue3_core -p vuec_vue3_dom -p vuec_vue3_ssr`, `cargo test -p vuec_vue2 -p vuec_node_bridge`, `cargo xtask run-option-matrix --all`, `cargo xtask diff-api --all`, `cargo xtask verify-npm-alias --all`, and `cargo xtask run-output-contract --version-line vue3 --package @vue/compiler-core` (`5/5` checks passed).
- Closed the Vue 2.7 `vue-template-compiler` option matrix by converting `warn`, `modules`, and `directives` to executable diff rows; the target now reports `3/3` rows passing.
- Closed the Vue 2.6 `vue-template-compiler` option matrix by enabling executable `warn`, `outputSourceRange`, `modules`, and `directives` rows; the target now reports `10/10` rows passing.
- Aligned Vue 2 malformed-tag recovery and default static optimization for the `outputSourceRange` fixture: `</div>` now closes through unmatched `<span>` and reports the intermediate tag once, and default compile options now run static optimization like the official compiler.
- Added Vue 2 npm alias public projection for `compile`, including official-style warning classification into `errors`, string-array `errors` / `tips` when `outputSourceRange` is false, ranged objects when enabled, and no public `diagnostics` field.
- Updated the option probe so JSON directive fixtures can be converted into official callable Vue 2 directive transforms, allowing the same deterministic directive row to exercise official and Rust outputs.
- Verification this round: `cargo fmt --all --check`, `cargo check -p vuec_vue2 -p vuec_node_bridge -p xtask`, `cargo test -p vuec_vue2 -p vuec_node_bridge -p xtask`, `cargo xtask generate-option-matrix --version-line vue2_7 --package vue-template-compiler`, `cargo xtask run-option-matrix --version-line vue2_7 --package vue-template-compiler`, `cargo xtask audit-option-matrix --version-line vue2_7 --package vue-template-compiler`, `cargo xtask generate-option-matrix --version-line vue2_6 --package vue-template-compiler`, `cargo xtask run-option-matrix --version-line vue2_6 --package vue-template-compiler`, `cargo xtask audit-option-matrix --version-line vue2_6 --package vue-template-compiler`, `cargo xtask run-option-matrix --all`, `cargo xtask audit-option-matrix --all`, `cargo xtask diff-api --version-line vue2_6 --package vue-template-compiler`, `cargo xtask verify-npm-alias --version-line vue2_6 --package vue-template-compiler`, `cargo xtask diff-api --version-line vue2_7 --package vue-template-compiler`, and `cargo xtask verify-npm-alias --version-line vue2_7 --package vue-template-compiler`.
- Wired Vue 3 `@vue/compiler-core` interpolation expression validation through `vuec_js::JsAstStore`, selecting Oxc TypeScript source mode when `isTS` is enabled or `expressionPlugins` contains `typescript`.
- Added Vue 3 core alias preflight parity for official `cacheHandlers` and `scopeId` invalid option-combination `SyntaxError`s, including official error codes 50 and 51.
- Added the current Vue 3 core slot outlet and static child cache codegen paths used by the option fixtures: `<slot>` lowers to `_renderSlot(...)`, and `hoistStatic` emits `_cache[0]` cached static children.
- Converted all remaining Vue 3 `@vue/compiler-core` option rows from pending to executable diff mode; the target now reports `8/8` option rows passing with no failures and no pending rows.
- Verification this round: `cargo fmt --all --check`, `cargo check -p vuec_ast -p vuec_vue3_core -p vuec_node_bridge -p xtask`, `cargo test -p vuec_ast -p vuec_vue3_core -p xtask`, `cargo xtask generate-option-matrix --version-line vue3 --package @vue/compiler-core`, `cargo xtask run-option-matrix --version-line vue3 --package @vue/compiler-core`, `cargo xtask audit-option-matrix --version-line vue3 --package @vue/compiler-core`, `cargo xtask diff-api --version-line vue3 --package @vue/compiler-core`, and `cargo xtask verify-npm-alias --version-line vue3 --package @vue/compiler-core`.
- Converted the Vue 3 `@vue/compiler-dom` `isCustomElement` option row from pending to executable diff mode and taught the option probe to convert the JSON fixture list into the official predicate callback shape; the target now reports `4/4` option rows passing with no pending rows.
- Verification this round: `cargo xtask run-option-matrix --version-line vue3 --package @vue/compiler-dom`, `cargo xtask audit-option-matrix --version-line vue3 --package @vue/compiler-dom`, `cargo xtask diff-api --version-line vue3 --package @vue/compiler-dom`, and `cargo xtask verify-npm-alias --version-line vue3 --package @vue/compiler-dom`.
- Converted the Vue 3 `@vue/compiler-ssr` `scopeId` option row from pending to executable diff mode; the target now reports `2/2` option rows passing with no pending rows.
- Verification this round: `cargo xtask run-option-matrix --version-line vue3 --package @vue/compiler-ssr`, `cargo xtask audit-option-matrix --version-line vue3 --package @vue/compiler-ssr`, `cargo xtask diff-api --version-line vue3 --package @vue/compiler-ssr`, and `cargo xtask verify-npm-alias --version-line vue3 --package @vue/compiler-ssr`.
- Added version-specific Vue 2.7 `vue/compiler-sfc` alias bridge commands so parse/template/script/style results are projected to the Vue 2.7 public API shape instead of reusing the Vue 3 SFC wrapper.
- Fixed the option probe and npm alias smoke paths for Vue 2.7's single-object SFC API, including template/style block source extraction for executable diff rows.
- Converted Vue 2.7 `compileScript` and `compileStyle` rows from pending to executable diff mode; `cargo xtask run-option-matrix --version-line vue2_7 --package vue --entry vue/compiler-sfc` now reports `4/4` rows passing with no pending rows.
- Verification this round: `cargo check -p vuec_node_bridge -p xtask`, `cargo xtask verify-npm-alias --version-line vue2_7 --package vue --entry vue/compiler-sfc`, `cargo xtask diff-api --version-line vue2_7 --package vue --entry vue/compiler-sfc`, `cargo xtask run-option-matrix --version-line vue2_7 --package vue --entry vue/compiler-sfc`, `cargo xtask audit-option-matrix --version-line vue2_7 --package vue --entry vue/compiler-sfc`, plus Vue 3 SFC matrix/audit regression checks.
- Converted the Vue 3 `@vue/compiler-sfc` `compileScript` option row from pending to executable diff mode; the target now reports `4/4` option rows passing with no pending rows.
- Added a minimal official-style script setup codegen path for simple const bindings, including `defineComponent` wrapping and object-form binding metadata for the option probe.
- Fixed the option probe to pass `parse(...).descriptor` into official `compileScript`, matching the public API contract.
- Verification this round: `cargo test -p vuec_sfc -p xtask`, `cargo check -p vuec_sfc -p vuec_node_bridge`, `cargo xtask run-option-matrix --version-line vue3 --package @vue/compiler-sfc`, and `cargo xtask audit-option-matrix --version-line vue3 --package @vue/compiler-sfc`.
- Converted the Vue 3 `@vue/compiler-sfc` `compileStyle` option row from pending to executable diff mode and got the target to `3/4` rows passing with only `compileScript` still pending.
- Aligned SFC style result serialization with official field names, including `rawResult`, deterministic dependencies, and source-map/null behavior for the executable option row.
- Updated style CSS var rewriting and scoped output formatting to match the official Vue 3 `compileStyle` option probe for the current fixture.
- Updated the option probe normalizer so Sets containing Symbols and PostCSS `rawResult` objects serialize deterministically instead of throwing or expanding unstable internals.
- Verification this round: `cargo test -p vuec_style -p vuec_sfc -p xtask`, `cargo check -p vuec_sfc -p vuec_node_bridge`, `cargo xtask run-option-matrix --version-line vue3 --package @vue/compiler-sfc`, and `cargo xtask audit-option-matrix --version-line vue3 --package @vue/compiler-sfc`.
- Implemented the first `vuec_js::JsAstStore` registry layer with `JsEntry`, `JsSourceType`, `JsExprId` / `JsStmtId` / `JsPatternId` / `JsProgramId` allocation, lookup, and parse-by-id APIs, while keeping the direct Oxc parse wrappers available for existing call sites.
- Added registry tests for expression, statement, pattern, and program ids, including stable source/span/mode/source-type metadata.
- Updated `vuec_sfc::compile_script` to register `<script>` and `<script setup>` as JS programs and emit official-style `type`, `setup`, `lang`, `imports`, `scriptAst`, `scriptSetupAst`, and `deps` fields.
- Verification this round: `cargo test -p vuec_js`, `cargo test -p vuec_sfc`, and `cargo check -p vuec_js -p vuec_sfc -p vuec_vue2 -p vuec_node_bridge`.
- Updated the development plan to align with `docs/3.AST_HIR_MIR_DESIGN.md` as the authoritative AST/IR contract and tightened the deterministic acceptance language around AST projections, lowering maps, and MIR targets.
- Upgraded `compat/options/*` from category-only placeholders to schema v2 option case matrices. Each row now records option path, accepted types, missing/undefined/null behavior notes, affected output fields, fixture source, input kind, method, execution mode, and status.
- Replaced `run-option-matrix` scaffold output with a real Node probe that loads the pinned official npm package and the generated Rust alias package for the same target, runs each executable option row, compares affected fields, and writes `target/conformance/<lock-hash>/option-matrix.json`.
- Added deterministic handling for `missing`, `undefined`, `null`, and concrete option values in the option probe input model, so future rows can test all required value states without changing runner architecture.
- Wired Vue 2 `shouldDecodeNewlines` and `shouldDecodeNewlinesForHref` through `vuec_node_bridge` and adjusted Vue 2 attr codegen newline escaping so the executable Vue 2.6 decode rows pass.
- Current `run-option-matrix` result: Vue 2.6 has 6 executable rows passing and 4 explicit pending rows; Vue 2.7 and Vue 3 option rows still expose real output/shape/codegen differences and remain failing rather than being hidden.
- Verification this round: `cargo test -p vuec_vue2`, `cargo test -p vuec_node_bridge`, `cargo xtask generate-option-matrix --all`, `cargo xtask audit-option-matrix --all`, and `cargo xtask run-option-matrix --version-line vue2_6 --package vue-template-compiler`.
- Vue 3 SFC template compilation now preserves the raw source through codegen, emits module-mode imports directly from the render backend, and attaches source maps from the render output instead of rebuilding strings after the fact.
- Vue 3 side-effect `<script>/<style>` handling is now AST/diagnostic-driven rather than string-pruned, and the remaining work is closing the official compileScript/compileStyle shape gap.
- `vuec_ast` has been expanded to the new arena contract with `NodeSpan`, parent/index tracking, `LoweringMap`, helper enums, JS id aliases, and compatibility `Hir` / `Mir` aliases.
- `vuec_pass` now has enum-based helpers and a depth-first document walker, which is the first step in moving transforms onto the new AST/HIR/MIR path.

## Previous Round

- Added `vuec_node_bridge`, a JSON stdin/stdout bridge binary that lets generated Node alias packages call the current Rust Vue 2, Vue 3 core/dom/ssr, and SFC compiler entry points.
- Implemented `xtask` generation for `target/compat/rust-alias/<version-line>/node_modules/...` packages matching the official package names and subpath layout, including Vue 2.7 `vue/compiler-sfc`.
- Generated Rust alias exports from official API manifests so `Object.keys`, function arity/name/prototype shape, class-like exports, enum-like objects, symbol exports, package versions, and type declaration paths match the official manifests.
- Replaced `verify-npm-alias` pending output with a real require-and-smoke-call check for all seven compiler targets.
- Regenerated Rust API manifests under `compat/api/rust/`; `cargo xtask export-api --all`, `cargo xtask diff-api --all`, and `cargo xtask verify-npm-alias --all` now pass.
- This bridge is deliberately recorded as a development bridge, not the final NAPI package required by M16. Remaining official conformance and option/output contract work must still exercise real behavior and cannot rely on export shape alone.

## Earlier API Round

- Replaced the spec-only `export-api` behavior with a real Node probe that installs exact official npm package versions from `compat/official-revisions.lock` under `target/compat/npm/<version-line>` and records `Object.keys(require(...)).sort()`, export type details, function arity, class/async flags, package version, type declaration path, require status, lock hash, and official revision.
- Changed `cargo xtask export-api --all` to generate both official and Rust-side manifests. Official manifests pass for all seven compiler targets; Rust manifests are generated as `pending` until alias packages exist under `target/compat/rust-alias/<version-line>`.
- Implemented `cargo xtask diff-api --all` as a real field-level manifest comparison with an explicit `compat/api/allowed-diff.json` approval file. It currently fails deterministically because the Rust alias packages are not implemented yet.
- Generated official and Rust API manifest files under `compat/api/` for Vue 2.6 `vue-template-compiler`, Vue 2.7 `vue-template-compiler`, Vue 2.7 `vue/compiler-sfc`, and Vue 3 `@vue/compiler-core`, `@vue/compiler-dom`, `@vue/compiler-ssr`, `@vue/compiler-sfc`.
- Verification this round: `cargo fmt --all --check`, `cargo test -p xtask`, `cargo xtask export-api --all`, and `cargo xtask diff-api --all` as the expected failing alias gate.

## Earlier Vue 2 Round

- Replaced the old `vuec_vue2` skeleton with a recursive Vue 2 element AST that preserves raw attrs, processed attrs/props/directives/events, structural directive state, static flags, slot/component/model metadata, diagnostics spans, and public AST projection.
- Implemented Vue 2 main-path parsing for `v-if` / `v-else-if` / `v-else`, `v-for`, `v-pre`, `v-once`, `v-bind`, `v-on`, `v-model`, custom directives, slots, component `is`, inline-template metadata, class/style modules, and validator-like compile option hooks.
- Implemented Vue 2 filter parsing and rewriting to `_f(...)`, interpolation parsing with custom delimiters, event handler generation with key/generic modifiers, DOM/component model assignment helpers, static optimization, static render hoisting, and Vue 2 style codeframe generation.
- Fixed the shared HTML tokenizer so Vue directive attribute names such as `@click.stop`, `#default`, `.prop`, and dynamic argument names are consumed correctly instead of stalling.
- Updated `xtask` version-line parsing so the deterministic plan commands accept `vue2_6`, `vue2_7`, and `vue3` directly, while preserving `vue26` / `vue27` compatibility.
- Generated the initial Vue 2.6 `vue-template-compiler` option matrix and output contract files under `compat/`.
- Verification this round: `cargo test --workspace`, `cargo xtask verify-official-lock`, `cargo xtask run-conformance --suite vue2-compiler`, `cargo xtask generate-option-matrix --version-line vue2_6 --package vue-template-compiler`, and `cargo xtask generate-output-contract --version-line vue2_6 --package vue-template-compiler`.
- `run-conformance --suite vue2-compiler` now reports discovered official tests plus alias smoke/readiness status; it remains `pending` until the full official test runner dependencies are installed and executed.

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
