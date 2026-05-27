# Security And Supply Chain

This checklist records the release-time security and supply-chain controls for the Rust Vue compiler.

## Locked Inputs

- Rust dependencies are locked by `Cargo.lock`.
- JavaScript package manager is pinned by root `packageManager` as `pnpm@9.0.0`.
- Official Vue baselines are pinned in `compat/official-revisions.lock` by 40-character Git commits and exact npm package versions.
- Generated conformance reports must record the lock hash and official baselines through `docs/CONFORMANCE_REPORT_TEMPLATE.md`.

## Package Metadata

- Rust crates use workspace license metadata: `MIT OR Apache-2.0`.
- npm release packages use `license: "MIT OR Apache-2.0"`.
- npm platform packages must publish only `vuec_napi.node` and `README.md`.
- npm loader packages must use stable `files` lists and exact optional dependency versions.
- Generated wasm-bindgen output directories remain ignored and are rebuilt during release verification.

## Audit Commands

Run these before a release candidate:

```bash
cargo xtask verify-official-lock
cargo xtask verify-release-docs
cargo xtask verify-crate-metadata
cargo xtask verify-supply-chain
cargo audit
pnpm audit --prod
```

`cargo audit` and `pnpm audit --prod` require the corresponding tools and advisory databases to be available in the release environment. If either external audit cannot run, record the reason and the follow-up in the filled conformance/release report.

## Artifact Provenance

- Record the git commit and dirty state for every release candidate.
- Record Rust, Node, pnpm, and npm versions in the release report.
- Record the conformance report path and lock hash in release notes.
- Keep npm package dry-run output and cargo publish dry-run output with the release candidate artifacts.

## Compatibility Boundary

Any release that includes changes under `xtask/src/compat.rs` must classify them as import/API adapter, runner support, AST hydration/dehydration, or temporary semantic shim. Temporary semantic shims require a Rust migration plan in `docs/COMPATIBILITY_CONCERNS.md` or the filled release report.
