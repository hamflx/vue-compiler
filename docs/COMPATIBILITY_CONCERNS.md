# Compatibility Concerns

- Vue 2.7 SFC compatibility must use the `vue/compiler-sfc` export from the `vue` package.
- Compiler parity must be verified against official option matrices and output contracts, not only against pass/fail fixture counts.
- Vue 2.7 SFC `compileStyle` is mostly Rust-backed, but caller-provided PostCSS plugins/options are JavaScript callbacks and cannot be serialized through the JSON bridge. The generated alias may execute those callbacks in a narrow API adapter after Rust style compilation; report that file as `mixed` and do not count the callback execution as Rust compiler semantics.
- Vue 3 compiler-core internal conformance tests can pass through JavaScript alias runtime shims in `xtask/src/compat.rs`. These passes validate test-runner/package compatibility and shim behavior, but they do not by themselves prove Rust compiler parity. Reports and progress notes must label these cases as `shim-backed` or `mixed` until the equivalent behavior is executed by Rust-backed APIs.
- Growing `compat.rs` into a full JavaScript `compiler-core` implementation conflicts with the Rust compiler goal. New shim behavior should be treated as temporary adapter code unless it forwards to Rust or has a documented Rust migration path.
