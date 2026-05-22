# Pending Decisions

- Package alias shape for the final NAPI publishing flow still needs to be finalized after the Rust surface stabilizes. It must cover Vue 2.6 and Vue 2.7 `vue-template-compiler`, Vue 2.7 `vue/compiler-sfc`, and Vue 3 `@vue/compiler-*` entries without changing official test imports.
- The first alias runner now uses a temporary JSON CLI bridge through `vuec_node_bridge`. The final acceptance path still requires replacing this with NAPI alias execution without changing official test imports.
- `target/compat/rust-alias/<version-line>` is now the generated probe root for Rust API manifests and alias smoke tests. The final NAPI package layout must preserve the same package names/subpaths or teach `xtask` where the built packages live without weakening manifest comparison.
- Option matrix rows can now be executable or explicit pending. Pending rows currently cover callback/plugin-style options and incomplete diagnostic/source-range/SFC semantics; final acceptance still requires converting these rows to executable pass rows, not deleting them or treating pending as compatible.
