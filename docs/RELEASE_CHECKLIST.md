# Release Checklist

Use this checklist before publishing a release candidate.

## Preflight

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo xtask sync-official-tests --locked`
- [ ] `cargo xtask prepare-runtime-smoke`
- [ ] `cargo test --workspace`
- [ ] `cargo xtask verify-official-lock`
- [ ] `cargo xtask verify-release-docs`
- [ ] `cargo xtask verify-public-api-docs`
- [ ] `cargo xtask verify-crate-metadata`
- [ ] `cargo xtask verify-supply-chain`
- [ ] `cargo xtask verify-release-dry-run --native-artifacts-dir <native-artifacts>`
- [ ] `cargo xtask verify-release-install-smoke` on every required OS/arch/libc host; CI matrix runners may use `--current-platform-only`.
- [ ] `cargo xtask verify-ci-status --commit <sha>` returns `pass` for the release candidate commit and required workflow jobs.
- [ ] `cargo xtask summarize-compat --locked`
- [ ] `cargo xtask bench --iterations 1`
- [ ] Fill `docs/CONFORMANCE_REPORT_TEMPLATE.md` for the release candidate report.

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
- [ ] Run external advisory audits from `docs/SECURITY_SUPPLY_CHAIN.md` where the release environment has the tools and advisory databases.
- [ ] Confirm every published crate has package metadata (`cargo xtask verify-crate-metadata`) and public API docs (`cargo xtask verify-public-api-docs`).
- [ ] Run `cargo xtask verify-release-dry-run --native-artifacts-dir <native-artifacts>` and require no `fail` or `pending` rows before publication.
- [ ] Run npm pack dry-runs for `packages/native`, all `packages/native-platforms/*`, and `packages/wasm`. Cross-platform native packages need release-built `vuec_napi.node` artifacts from their target platforms, provided as `<platform>/vuec_napi.node`, `<platform>.node`, or the downloaded GitHub artifact wrapper layout such as `native-Linux-X64/linux-x64-gnu/vuec_napi.node`.
- [ ] Run cargo publish dry-runs for published crates. On the first crates.io release, publish leaf crates first, then rerun dry-runs for crates that depend on already-published internal crates.
- [ ] Install packed npm artifacts into clean temp projects and run Node smoke tests on each required target host: `cargo xtask verify-release-install-smoke`. Use `--current-platform-only` only for per-runner CI evidence; require the aggregated release evidence to have no `fail` or `pending` rows before publication.
- [ ] Confirm `cargo xtask verify-ci-status --commit <sha>` reports `pass` for the final candidate commit, including Windows/Linux/macOS compatibility jobs, product smoke, Windows/Linux/macOS release install smoke jobs, and release dry-run.
- [ ] Record the conformance report path, lock hash, and coverage classification in the release notes.

## Rollback

- [ ] Keep previous npm package versions available.
- [ ] Record any yanked crate/package and the reason in `CHANGELOG.md`.
