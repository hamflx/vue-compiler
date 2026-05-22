# Memory

- Active objective: build a Rust Vue compiler that can replace official Vue 2.6, Vue 2.7, and Vue 3 compiler flows.
- Current focus: workspace bootstrap, shared source/diagnostic foundations, and xtask harness skeleton.
- Compatibility rule: Vue 2.7 SFC export is `vue/compiler-sfc`, not a standalone `@vue/compiler-sfc@2.7.x` package.
- Workspace now has initial crates for `vuec_source`, `vuec_diagnostics`, `vuec_html`, `vuec_js`, `vuec_ast`, `vuec_pass`, `vuec_codegen`, plus an `xtask` compatibility harness scaffold.
- Workspace now also has initial `vuec_sfc` and `vuec_vue3_core` crates with compileable public interfaces and smoke tests.
