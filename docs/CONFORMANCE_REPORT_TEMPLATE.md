# Conformance Report Template

Use this template when attaching official conformance results to a release candidate or stage-completion note.

## Report Identity

- Report path:
- Generated at:
- Command:
- Lock hash:
- Official lock file: `compat/official-revisions.lock`
- Git commit:
- Dirty workspace:
- Runner OS / arch:
- Node / package manager:
- Rust toolchain:

## Official Baselines

| Line | Official repo | Commit | npm packages |
| --- | --- | --- | --- |
| Vue 2.6 | `https://github.com/vuejs/vue` | `af43c9d14dd087b9852912bd15b1eacbda0e13b0` | `vue@2.6.14`, `vue-template-compiler@2.6.14` |
| Vue 2.7 | `https://github.com/vuejs/vue` | `13f4e7dc03e2caed900ac70ff8b8fe58dda45663` | `vue@2.7.16`, `vue-template-compiler@2.7.16` |
| Vue 3 | `https://github.com/vuejs/core` | `57545e958ae28ed17aa9e0ed321abcd8dc99f752` | `@vue/compiler-core@3.5.34`, `@vue/compiler-dom@3.5.34`, `@vue/compiler-sfc@3.5.34`, `@vue/compiler-ssr@3.5.34` |

## Execution Scope

| Suite | Package / entry | Backend | Command | Status | Pass | Fail | Pending | Report file |
| --- | --- | --- | --- | --- | ---: | ---: | ---: | --- |
|  |  | rust alias / NAPI alias / mixed prepared runner |  |  |  |  |  |  |

Backend must identify whether the suite used the Rust alias package, NAPI-backed official package-name alias, or a mixed prepared runner with official TypeScript/JavaScript source.

## Coverage Classification

| Suite | rust-backed pass / total | mixed pass / total | shim-backed pass / total | Top-level coverage | Reason |
| --- | ---: | ---: | ---: | --- | --- |
|  |  |  |  | rust-backed / mixed / shim-backed |  |

Only `rust-backed` rows, or focused checks that explicitly validate Rust compiler output, count as Rust compiler completion evidence. `mixed` and `shim-backed` rows are compatibility signals and must not be reported as standalone Rust compiler parity.

## File-Level Coverage

| Suite | File | Source | Pass | Fail | Pending | Reason |
| --- | --- | --- | ---: | ---: | ---: | --- |
|  |  | rust-backed / mixed / shim-backed |  |  |  |  |

## Failure Summary

| Suite | File | Test | Error class | Current owner | Next action |
| --- | --- | --- | --- | --- | --- |
|  |  |  | parser / transform / codegen / source-map / API adapter / runner | Rust compiler / adapter / upstream fixture |  |

## Compatibility Concerns

- New `xtask/src/compat.rs` changes:
- Classification: import/API adapter, runner support, AST hydration/dehydration, or temporary semantic shim.
- Rust migration plan for any temporary semantic shim:
- Related entries in `docs/COMPATIBILITY_CONCERNS.md`:

## Acceptance Decision

- Release/stage decision:
- Evidence counted as Rust compiler completion:
- Evidence kept as mixed/shim compatibility signal:
- Follow-up report path:
