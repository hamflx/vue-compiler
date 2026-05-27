# vue-compiler

Rust implementation of the Vue compiler stack for Vue 2.6, Vue 2.7, and Vue 3 compatibility.

This repository contains compiler crates, NAPI and WASM packages, a CLI, and official conformance tooling. The source of truth for compiler architecture is `docs/1.RUST_VUE_COMPILER_DESIGN.md`; the AST/HIR/MIR structure contract is `docs/3.AST_HIR_MIR_DESIGN.md`.

## Status

- Vue baselines are pinned in `compat/official-revisions.lock`.
- Official conformance and output-contract reports are generated through `cargo xtask`.
- Coverage reports distinguish `rust-backed`, `mixed`, and `shim-backed` execution. Mixed or shim-backed coverage is not counted as standalone Rust compiler parity.
- M19 performance and incremental gates are in place: benchmark reporting, SFC cache, AST cache, parallel compilation, arena preallocation, and JS source string interning.

## Common Commands

```bash
cargo fmt --all -- --check
cargo test --workspace
cargo xtask verify-official-lock
cargo xtask run-output-contract --all
cargo xtask run-conformance --all
cargo xtask bench --iterations 1
```

Focused gates are useful while developing:

```bash
cargo xtask verify-cli
cargo xtask verify-napi
cargo xtask verify-wasm
cargo xtask verify-arena
cargo xtask verify-string-interning
cargo xtask verify-release-docs
cargo xtask verify-crate-metadata
cargo xtask verify-supply-chain
```

## Packages

- `@vuec-rs/native`: Node/NAPI package that loads the platform native binding.
- `@vuec-rs/wasm`: browser/Node WASM package.
- `vuec_cli`: command-line compiler binary.
- `crates/vuec_*`: Rust compiler, AST, source, diagnostics, SFC, style, NAPI, and WASM crates.

## Documentation

- `docs/2.DEVELOPMENT_PLAN.md`: development plan and acceptance gates.
- `docs/WORK_PART.md`: stage progress report.
- `docs/MEMORY.md`: rolling implementation memory.
- `docs/COMPATIBILITY_CONCERNS.md`: compatibility caveats and shim/mixed coverage concerns.
- `docs/RELEASE_CHECKLIST.md`: release dry-run and publication checklist.
- `docs/COMPATIBILITY_MATRIX.md`: current official compatibility matrix.
- `docs/CONFORMANCE_REPORT_TEMPLATE.md`: release/stage conformance report template.
- `docs/ARCHITECTURE.md`: release-facing compiler architecture map.
- `docs/SECURITY_SUPPLY_CHAIN.md`: release security and supply-chain checks.
