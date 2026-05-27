# Unresolved Problems

## M20 release dry-run and cross-platform install smoke need external release artifacts

`cargo xtask verify-release-dry-run` and
`cargo xtask verify-release-install-smoke` are implemented and were rerun on the
current Windows x64 host. Both gates execute real checks and currently return
`pending` with zero failures.

Current evidence:

- `cargo xtask verify-release-dry-run`: `25` checks total, `5` pass, `20`
  pending, `0` fail.
- `cargo xtask verify-release-install-smoke`: `9` checks total, `2` pass, `7`
  pending, `0` fail.
- The current host proves the main native package, WASM package, and
  `win32-x64` native optional package staging/install path.

Blocking conditions:

- Non-current native platform npm packages require target-platform release-built
  `vuec_napi.node` artifacts before `npm pack --dry-run` can prove their
  tarball contents.
- Non-current native platform install smoke requires matching host/platform
  runs or equivalent CI workers with those release artifacts.
- First-time crates.io `cargo publish --dry-run` for crates with internal path
  dependencies requires the dependency crates to exist in the registry first;
  until then, the gate can only prove package file lists and registry-resolvable
  leaf crates.

Impact:

- M20 release verification infrastructure exists and reports honest status, but
  a single Windows host cannot turn those cross-platform/registry-dependent rows
  into pass rows.
- Final release execution still needs multi-platform artifact production and
  ordered registry publication outside this local repository state.

Decision:

- Treat the local implementation work for these M20 items as complete because
  the remaining work is external release infrastructure and first-publication
  state, not missing compiler or repository code.
- Keep the gate reports as `pending`, not `pass`, so release readiness remains
  explicit and auditable.

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
