# Memory

- Active objective: build a Rust Vue compiler that can replace official Vue 2.6, Vue 2.7, and Vue 3 compiler flows.
- Current focus: conformance harness real checkout/discovery, then wiring actual alias execution for official compiler suites.
- Compatibility rule: Vue 2.7 SFC export is `vue/compiler-sfc`, not a standalone `@vue/compiler-sfc@2.7.x` package.
- Workspace now has initial crates for `vuec_source`, `vuec_diagnostics`, `vuec_html`, `vuec_js`, `vuec_ast`, `vuec_pass`, `vuec_codegen`, plus an `xtask` compatibility harness scaffold.
- Workspace now also has initial `vuec_sfc` and `vuec_vue3_core` crates with compileable public interfaces and smoke tests.
- Added `vuec_vue3_dom`, `vuec_vue3_ssr`, and `vuec_style` crates. These are functional early backends with unit tests, not official parity-complete implementations.
- `vuec_sfc` now routes `compileTemplate` through DOM or SSR backends, routes `compileStyle` through `vuec_style`, and uses Oxc-backed `vuec_js` program summaries for script bindings/errors.
- `xtask sync-official-tests --locked` now performs real git checkout of the official Vue 2.6 / 2.7 / 3 repositories pinned by `compat/official-revisions.lock`.
- `xtask run-conformance --all` now discovers official test files from the synced checkouts and writes lock-hash-scoped JSON discovery reports, but execution is still pending alias-backed runners.
- Added `vuec_vue2` crate as a compileable Vue 2 compiler skeleton with `compile`, `compileToFunctions`, and SSR entry points, but it is not yet parity-complete.
