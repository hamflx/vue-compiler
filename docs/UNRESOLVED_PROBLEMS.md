# Unresolved Problems

## Vue 3 compiler-core conformance shims can diverge from the Rust implementation

The Vue 3 compiler-core conformance runner generates JavaScript shims for
internal Vue compiler APIs such as `transformExpression` and
`processExpression`. These shims are implemented in `xtask/src/compat.rs` as
part of `ALIAS_RUNTIME_JS` and are exported through the generated
`@vue/compiler-core` alias package.

This means official Vue tests that import internal modules like
`../../src/transforms/transformExpression` may exercise the JavaScript shim
implementation rather than the Rust compiler implementation. Normal public API
entry points such as `baseCompile` and `baseParse` do go through the
`vuec_node_bridge` into Rust, but internal transform tests can run against the
shim layer.

Impact:

- Passing internal transform conformance tests does not necessarily prove the
  equivalent Rust expression rewriting behavior is correct.
- A mismatch can develop between the JavaScript shim behavior and the Rust
  implementation in `crates/vuec_vue3_core`.
- Failures in these tests may reflect shim incompatibility rather than a Rust
  compiler bug.

Open question:

- Should these internal transform shims be replaced with bridge calls into
  Rust, or should reports clearly classify shim-heavy tests separately from
  Rust-backed conformance coverage?
