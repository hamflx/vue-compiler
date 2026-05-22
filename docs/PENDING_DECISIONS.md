# Pending Decisions

- Package alias shape for the final NAPI publishing flow still needs to be finalized after the Rust surface stabilizes. It must cover Vue 2.6 and Vue 2.7 `vue-template-compiler`, Vue 2.7 `vue/compiler-sfc`, and Vue 3 `@vue/compiler-*` entries without changing official test imports.
- Decide whether the first alias runner should call Rust through NAPI immediately or use a temporary JSON CLI bridge while NAPI package scaffolding is being built. The final acceptance path still requires NAPI alias execution.
