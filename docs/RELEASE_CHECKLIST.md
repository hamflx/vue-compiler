# Release Checklist

Use this checklist before publishing a release candidate.

## Preflight

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo test --workspace`
- [ ] `cargo xtask verify-official-lock`
- [ ] `cargo xtask verify-release-docs`
- [ ] `cargo xtask verify-crate-metadata`
- [ ] `cargo xtask summarize-compat --locked`
- [ ] `cargo xtask bench --iterations 1`

## Package Verification

- [ ] `cargo xtask verify-cli`
- [ ] `cargo xtask verify-napi`
- [ ] `cargo xtask verify-napi-alias`
- [ ] `cargo xtask verify-napi-api`
- [ ] `cargo xtask verify-napi-platform`
- [ ] `cargo xtask verify-wasm`
- [ ] `cargo xtask verify-wasm-browser`
- [ ] `cargo xtask verify-wasm-wasi`

## Publication

- [ ] Confirm every npm package has `README.md`, `package.json`, license metadata, and a stable file list.
- [ ] Confirm every published crate has package metadata (`cargo xtask verify-crate-metadata`) and public API docs.
- [ ] Run npm pack dry-runs for `packages/native`, all `packages/native-platforms/*`, and `packages/wasm`.
- [ ] Run cargo publish dry-runs for published crates.
- [ ] Install packed npm artifacts into a clean temp project and run Node smoke tests.
- [ ] Record the conformance report path and lock hash in the release notes.

## Rollback

- [ ] Keep previous npm package versions available.
- [ ] Record any yanked crate/package and the reason in `CHANGELOG.md`.
