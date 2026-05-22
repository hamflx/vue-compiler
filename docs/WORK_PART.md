# Work Part

## Current Stage

- [x] Normalize goals and compatibility acceptance criteria.
- [x] Workspace bootstrap.
- [x] Shared source and diagnostic crates.
- [x] Official test harness and lock validation.
- [x] Vue 3 core and SFC skeletons.
- [x] Vue 3 DOM / SSR / style early backend crates.

## Completed This Round

- Added `vuec_vue3_dom`, `vuec_vue3_ssr`, and `vuec_style` to the workspace.
- Extended `vuec_ast::Vue3NodeKind::Element` to keep template attributes and self-closing state.
- Reworked `vuec_vue3_core::base_parse` to consume the shared HTML tokenizer and emit text, interpolation, comment, and element nodes.
- Added DOM directive extraction and summaries for `v-html`, `v-text`, `v-show`, `v-model`, `v-on`, `v-bind`, asset URLs, and custom elements.
- Added SSR compile output shape with `ssrRender`, interpolation emit, scope/slotted attributes, and SSR node summaries.
- Added style compilation primitives for scoped selector rewrite, `:deep`, `:slotted`, `:global`, CSS vars, CSS modules, and source map shape.
- Connected `vuec_sfc::compileTemplate` to DOM/SSR backends, `compileStyle` to `vuec_style`, and `compileScript` to Oxc-backed script summaries.

## Previous Round

- Workspace, package manifests, and `.cargo` alias were added.
- Shared source, diagnostics, HTML, JS, AST, pass, and codegen crates were scaffolded.
- Vue 3 core and SFC crates were scaffolded with initial parse/compile wrappers.
- `xtask` now includes compatibility-oriented JSON report commands and the official lock file.

## Notes

- This repository started from docs only; no compiler code existed before the current implementation work.
- The new Vue 3 DOM / SSR / style crates are not official-compatible yet. They establish module boundaries, API shapes, and smoke-tested behavior that must be expanded against official compiler tests.
