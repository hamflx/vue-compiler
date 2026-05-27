# Compatibility Matrix

This matrix records the pinned official compiler baselines and the current Rust validation gates.

## Baselines

| Line | Official repo | Commit | npm packages |
| --- | --- | --- | --- |
| Vue 2.6 | `https://github.com/vuejs/vue` | `af43c9d14dd087b9852912bd15b1eacbda0e13b0` | `vue@2.6.14`, `vue-template-compiler@2.6.14` |
| Vue 2.7 | `https://github.com/vuejs/vue` | `13f4e7dc03e2caed900ac70ff8b8fe58dda45663` | `vue@2.7.16`, `vue-template-compiler@2.7.16` |
| Vue 3 | `https://github.com/vuejs/core` | `57545e958ae28ed17aa9e0ed321abcd8dc99f752` | `@vue/compiler-core@3.5.34`, `@vue/compiler-dom@3.5.34`, `@vue/compiler-sfc@3.5.34`, `@vue/compiler-ssr@3.5.34` |

## Current Gates

| Area | Command |
| --- | --- |
| Official lock | `cargo xtask verify-official-lock` |
| API manifest | `cargo xtask diff-api --all` |
| Option matrix | `cargo xtask run-option-matrix --all` |
| Output contract | `cargo xtask run-output-contract --all` |
| Official conformance | `cargo xtask run-conformance --all` |
| CLI smoke | `cargo xtask verify-cli` |
| NAPI smoke | `cargo xtask verify-napi`, `cargo xtask verify-napi-alias`, `cargo xtask verify-napi-api`, `cargo xtask verify-napi-platform` |
| WASM smoke | `cargo xtask verify-wasm`, `cargo xtask verify-wasm-browser`, `cargo xtask verify-wasm-wasi` |
| Performance | `cargo xtask bench`, `cargo xtask verify-arena`, `cargo xtask verify-string-interning`, `cargo xtask verify-ast-cache`, `cargo xtask verify-parallel`, `cargo xtask verify-incremental` |
| Release docs | `cargo xtask verify-release-docs` |

## Coverage Rule

Conformance reports must preserve `rust-backed`, `mixed`, and `shim-backed` coverage labels. Only `rust-backed` cases, or focused checks that explicitly validate Rust compiler output, count as Rust compiler completion evidence.

## Release Documentation Coverage

`cargo xtask verify-release-docs` verifies that the repository README, CHANGELOG, compatibility matrix, release checklist, and every `packages/**/package.json` directory README are present and non-empty. If a package manifest has a `files` array, the gate also requires `README.md` to be listed explicitly.
