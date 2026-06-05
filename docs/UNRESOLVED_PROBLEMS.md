# Unresolved Problems

## M20 release dry-run and cross-platform install smoke need complete external release evidence

`cargo xtask verify-release-dry-run` and
`cargo xtask verify-release-install-smoke` are implemented. They now accept
release-built native platform bindings through `--native-artifacts-dir`, using
either `<platform>/vuec_napi.node` or `<platform>.node` artifact layout, and
`verify-release-install-smoke --current-platform-only` gives a CI matrix runner
an honest current-host smoke gate. `.github/workflows/ci.yml` wires this into a
Windows/Linux/macOS `release-smoke` matrix plus a `release-dry-run` artifact
aggregation job.

Current evidence:

- `cargo xtask verify-release-install-smoke --current-platform-only`: `2`
  checks total, `2` pass, `0` pending, `0` fail on the current Windows x64
  host.
- `cargo xtask verify-release-dry-run --native-artifacts-dir
  target/release-install-smoke/packages/native-platforms`: `25` checks total,
  `5` pass, `20` pending, `0` fail; the supplied `win32-x64` artifact is staged
  into the platform package and npm pack dry-run passes for that package.
- The repository-owned CI workflow can now execute current-platform install
  smoke on Windows/Linux/macOS runners, upload those release bindings, and feed
  them into the release dry-run gate. Actual remote pass/fail remains a GitHub
  Actions runtime fact, not something proven by this Windows workspace alone.

Blocking conditions:

- Native platform npm packages that are not produced by the current CI matrix
  still require target-platform release-built `vuec_napi.node` artifacts before
  `npm pack --dry-run` can prove their tarball contents. This currently includes
  platform variants such as musl and ARM hosts not covered by the default
  Windows/Linux/macOS x64 runner set.
- Non-current platform install smoke still requires matching host/platform runs;
  downloaded artifacts can prove package file lists, but they cannot prove
  executable loading on a different OS/arch/libc host.
- First-time crates.io `cargo publish --dry-run` for crates with internal path
  dependencies requires the dependency crates to exist in the registry first;
  until then, the gate can only prove package file lists and registry-resolvable
  leaf crates.

Impact:

- M20 release verification infrastructure can now consume external native
  artifacts instead of permanently classifying all non-current platform npm
  packages as untestable from one host.
- Final release execution still needs all required platform artifacts, matching
  host install-smoke runs, remote CI pass evidence, and ordered registry
  publication outside this local repository state.

Decision:

- Treat the local implementation work for these M20 artifact-consumption gates
  as complete because the remaining rows require external platform/runtime or
  first-publication state, not compiler semantics.
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
