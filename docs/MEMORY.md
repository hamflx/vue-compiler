# Memory

- Active objective: build a Rust Vue compiler that can replace official Vue 2.6, Vue 2.7, and Vue 3 compiler flows.
- Current focus: build the deterministic compatibility harness around official API manifests and then wire Rust NAPI alias packages so `diff-api`, options/output contracts, and official test execution can become pass/fail gates.
- Compatibility rule: Vue 2.7 SFC export is `vue/compiler-sfc`, not a standalone `@vue/compiler-sfc@2.7.x` package.
- Workspace now has initial crates for `vuec_source`, `vuec_diagnostics`, `vuec_html`, `vuec_js`, `vuec_ast`, `vuec_pass`, `vuec_codegen`, plus an `xtask` compatibility harness scaffold.
- Workspace now also has initial `vuec_sfc` and `vuec_vue3_core` crates with compileable public interfaces and smoke tests.
- Added `vuec_vue3_dom`, `vuec_vue3_ssr`, and `vuec_style` crates. These are functional early backends with unit tests, not official parity-complete implementations.
- `vuec_sfc` now routes `compileTemplate` through DOM or SSR backends, routes `compileStyle` through `vuec_style`, and uses Oxc-backed `vuec_js` program summaries for script bindings/errors.
- `xtask sync-official-tests --locked` now performs real git checkout of the official Vue 2.6 / 2.7 / 3 repositories pinned by `compat/official-revisions.lock`.
- `xtask run-conformance --all` now discovers official test files from the synced checkouts and writes lock-hash-scoped JSON discovery reports, but execution is still pending alias-backed runners.
- `vuec_vue2` has moved beyond the skeleton: it now has a recursive Vue 2 element AST, directive parsing for core Vue 2 directives, filter rewriting, static optimization, official-style render/staticRenderFns codegen, event handler generation, model helpers, diagnostics, and Vue 2 codeframe generation. It is still not official parity-complete.
- `vuec_html` now accepts Vue directive attribute names such as `@click.stop`, `#default`, `.prop`, and dynamic-argument syntax without tokenizer stalls.
- `xtask --version-line` now accepts the canonical plan values `vue2_6`, `vue2_7`, and `vue3` while retaining `vue26` / `vue27` aliases.
- `cargo xtask export-api --all` now installs exact official npm versions from `compat/official-revisions.lock`, probes real package exports with Node, writes normalized official/Rust API manifests for all seven compiler targets, and reports Rust alias manifests as pending until alias packages exist.
- `cargo xtask diff-api --all` now performs field-level manifest diffing with an explicit `compat/api/allowed-diff.json` approval file; it currently fails because no Rust alias packages are present under `target/compat/rust-alias`.
