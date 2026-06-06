# Conformance And CompileScript Architecture Plan

This plan validates the two review reports against the current repository and proposes architecture work for review. It does not implement the changes.

Checked state:

- Current HEAD: `9f470a6 Document Vue 2 corpus completion`.
- Worktree was clean before this document was added.
- The referenced profiling files were not found in this repo or the adjacent `..\webpack5-vue2.7\artifacts\compiler-perf` path, so the exact microsecond numbers from the report are not independently verified here.

## Executive Summary

The main conformance problem is not that the official tests are useless. They are valuable, but the current evidence model is too coarse. It mixes prepared official tests, JavaScript package adapters, suite-only bridge projections, callback boundaries, and Rust public API execution under labels that can read as stronger than the actual call path.

The largest concrete reporting issue is provenance. `xtask/src/compat.rs` has suite-level defaults that mark Vue 3 generated conformance as `mixed`, but file-level path whitelists can reclassify prepared Vue 3 SFC/SSR/core files as `rust-backed` when a Vitest report is present. Cached reports and docs already show the resulting drift: the same Vue 3 SSR suite appears as `mixed 129/129` in one generated report and `rust-backed 129/129` in another, while some reasons still describe mixed SSR/DOM source execution.

The compileScript performance report is structurally credible even without the profiling artifacts. Current `vuec_sfc` code eagerly builds public JSON AST projections, recomputes Vue 2.7 template usage for each import, and re-runs script setup analysis across error/content/binding paths.

## Findings Validation

### Conformance And Reporting

| Review claim | Current validation | Proposed handling |
| --- | --- | --- |
| `rust-backed` classification is overconfident. | Valid as an architecture issue. `conformance_coverage_kind()` defaults generated Vue 3 suites to `mixed`, but `conformance_coverage_file_kind()` reclassifies many files by path, including Vue 3 SSR files. With execution results, all files can aggregate back to `rust-backed` even when the suite reason describes prepared SSR/DOM TypeScript source plus alias runtime participation. | Replace path-only classification with execution provenance. Keep path labels as metadata only, not as proof of Rust execution. |
| `ALIAS_RUNTIME_JS` includes non-thin compiler behavior. | Valid. `ALIAS_RUNTIME_JS` defines runtime enums/helpers, transform context behavior, helper tracking, AST hydration/materialization, and a JavaScript `runtime.transformElement()` that computes props, directives, patch flags, block usage, dynamic props, and callback handling while calling Rust projections for sub-decisions. | Split adapters into thin package/API adapters, Rust projection hydrators, callback boundaries, and semantic JS shims. Only the first two can count toward Rust completion without qualification. |
| Official tests are prepared, not unmodified. | Valid. `write_vue3_core_conformance_shims()` and related `rewrite_vue3_*_public_api_spec()` functions rewrite imports and add `.rust-api.ts` helper routes. This is reasonable, but it must be reported as `prepared official tests`. | Add a machine-readable prepared-test manifest and report the rewrite class per suite/file. |
| Suite bridge commands risk overfitting. | Valid risk, not automatically wrong. `vuec_node_bridge` exposes commands such as `vue3.core.transformBindSuite`, `transformElementSuite`, `cacheStaticSuite`, and others. These are useful migration probes, but they are not public Vue APIs and some functions reconstruct public JSON AST shapes around suite expectations. | Mark suite-only commands as `suite-projection`, never `pure-rust public api`. Add ownership and migration notes for each command. |
| SFC/style JS callback boundary is reasonable but unclear. | Valid. Vue 2.7 style aliases execute caller PostCSS plugins in JavaScript after Rust style compilation, and the bridge returns a placeholder `rawResult`. This is a real JavaScript callback boundary and should stay `mixed`. | Keep callback execution in JS, but make the report category explicit: Rust style compiler plus JS callback boundary. |
| External project corpus is not central enough. | Partially addressed. The Vue 2 project corpus is now strong and recently passed 15/15, but it is still separate from the main official `summarize-compat` style evidence. | Promote corpus reports into release/CI summaries as production corpus evidence, separate from official conformance. |

### CompileScript Performance

| Review claim | Current validation | Proposed handling |
| --- | --- | --- |
| `script_ast` / `script_setup_ast` JSON projection is expensive. | Structurally valid. `compile_script()` and `compile_vue27_script()` always call `sfc_script_ast_body()` for JS-like script blocks. Projection recursively allocates `serde_json::Value`, `loc`, and `source` slices. `position_at()` scans from the start of the source for each start/end offset. | Add lazy/optional public AST projection and a line index for offset-to-location conversion. |
| Vue 2.7 return bindings repeatedly scan the template. | Valid. `vue27_script_setup_return_bindings()` checks every relevant import through `vue27_script_setup_import_is_returned()`, which calls `vue27_template_uses_identifier()`, which rebuilds the template usage string each time. | Precompute a `TemplateUsageIndex` once per template and reuse it for all imports. |
| `analyze_vue27_script_setup()` runs repeatedly. | Valid. The current compile path calls it for script errors, generated content, and binding metadata. This reparses and re-analyzes the same setup block. | Introduce a compile context that owns the setup analysis and shares it across errors, content, bindings, and return binding generation. |
| NAPI/SFC parse overhead is the main issue. | Not validated. The bridge does parse source into a descriptor for `sfc.vue27.compileScript`, but the current code shape supports the report's conclusion that AST projection and repeated analysis are larger structural costs. | Treat descriptor reuse as a later optimization after the Rust internal hot paths are fixed. |
| Exact profiling timings should drive prioritization. | Not independently verified. The named artifact files were not present in the checked paths. | Recreate a reproducible checked-in profiling command before accepting specific microsecond targets as gates. |

## Conformance Architecture Proposal

### 1. Replace Source Labels With Provenance Dimensions

Current labels should become a roll-up over explicit dimensions, not an input inferred from file paths.

Required dimensions:

- `test_origin`: `unmodified-official`, `prepared-official`, `project-corpus`, `api-manifest`, `option-matrix`, `output-contract`, `custom-regression`.
- `execution_path`: `pure-rust-public-api`, `rust-bridge-shape-adapter`, `hybrid-js-adapter-rust-projection`, `mixed-js-callback-boundary`, `shim-backed-semantic-js`.
- `api_surface`: `public-package-api`, `public-rust-api`, `suite-only-bridge-command`, `internal-helper-import`.
- `adapter_role`: `import-rewrite`, `runner-support`, `hydration-dehydration`, `callback-materialization`, `semantic-shim`.
- `bridge_commands`: the actual `callBridge()` command names observed for the file or assertion.

Suggested JSON shape:

```json
{
  "coverage": {
    "summary": {
      "pure-rust-public-api": { "pass": 0, "total": 0 },
      "rust-bridge-shape-adapter": { "pass": 0, "total": 0 },
      "hybrid-js-adapter-rust-projection": { "pass": 0, "total": 0 },
      "mixed-js-callback-boundary": { "pass": 0, "total": 0 },
      "shim-backed-semantic-js": { "pass": 0, "total": 0 }
    },
    "files": [
      {
        "path": "packages/compiler-core/__tests__/transforms/transformElement.spec.ts",
        "test_origin": "prepared-official",
        "execution_path": "hybrid-js-adapter-rust-projection",
        "api_surface": "suite-only-bridge-command",
        "bridge_commands": ["vue3.core.transformElementSuite"],
        "adapter_role": ["import-rewrite", "hydration-dehydration"],
        "reason": "..."
      }
    ]
  }
}
```

The existing `rust-backed`, `mixed`, and `shim-backed` labels can remain as compatibility aliases, but they should be derived from these dimensions:

- `rust-backed` only when execution is `pure-rust-public-api` or narrowly `rust-bridge-shape-adapter` with no JavaScript semantic decisions.
- `mixed` when JavaScript callbacks, official TypeScript source, or suite-only projections participate in behavior under assertion.
- `shim-backed` when JavaScript implements compiler semantics that Rust does not execute.

### 2. Add A Prepared-Test Manifest

Every rewrite helper should emit a manifest entry when preparing official tests.

Manifest fields:

- original file path
- prepared file path
- rewrite operation: import rewrite, helper injection, test utility replacement, callback adapter
- helper file path, if any
- bridge command(s)
- expected coverage dimensions

This prevents statements like "official tests passed" from implying "unmodified official tests passed." The report should say "prepared official tests passed" unless the manifest is empty for that suite.

### 3. Instrument Runtime Provenance

The alias runtime should record actual execution markers during each test file or assertion:

- `callBridge(command)` command names.
- JavaScript semantic adapter markers, for example `js.transformElement.props`, `js.transformContext.replaceNode`, `js.postcss.plugin`.
- Callback boundary markers, for example `callback.nodeTransform`, `callback.directiveTransform`, `callback.postcssPlugin`.

The runner can flush these markers into the Vitest/Jasmine report after each test. File path classification can still provide defaults, but any observed marker must be able to downgrade a file from `rust-backed` to `mixed` or `shim-backed`.

### 4. Split The Alias Runtime

`ALIAS_RUNTIME_JS` is currently doing too many jobs. Split it conceptually and, ideally, physically:

- `package-api-adapter`: arity, names, public exports, argument normalization.
- `bridge-shape-adapter`: JSON serialization, symbol hydration, `undefined` restoration, public result shape.
- `callback-boundary`: non-serializable caller callbacks such as PostCSS plugins and Node/directive transforms.
- `semantic-js-shim`: any JavaScript code that makes compiler decisions.
- `suite-helper`: prepared official spec helpers and suite-only bridge glue.

Rules:

- New semantic JavaScript shims require a linked Rust migration plan.
- Suite helpers must not be counted as public API parity.
- Callback boundaries are acceptable but always mixed.
- Thin adapters may live in JS permanently, but they must not own compiler semantics.

### 5. Reclassify Suite Bridge Commands

Commands such as `vue3.core.transformBindSuite` and `vue3.core.transformElementSuite` should be renamed or tagged as compatibility suite commands. They can still be useful for migrating official assertions, but reports should not equate them with public compiler APIs.

Recommended categories:

- `public-command`: e.g. `sfc.compileScript`, `vue3.dom.compile`, `vue3.ssr.compile`.
- `projection-command`: Rust projection for an internal transform, usable as focused evidence.
- `suite-command`: command shaped around official spec helper expectations.

Acceptance rule: a suite command can support "Rust projection coverage" but not "pure Rust public API coverage" by itself.

### 6. Promote Production Corpus Evidence

The Vue 2 project corpus should be reported as a separate release gate:

- Include `verify-vue2-project-corpus` in CI/release summaries.
- Publish project count, file count, mode count, official compiler baseline, and Rust commit.
- Keep corpus failures separate from official conformance failures.
- Add Vue 3 project corpus later, but do not block the current conformance reporting fix on it.

## CompileScript Architecture Proposal

### 1. Introduce A Script Compile Context

Add a `Vue27ScriptCompileContext` or broader `SfcScriptCompileContext` that is built once per `compile_vue27_script()` call.

It should own or reference:

- normal script parse result or registered program id
- setup script parse result or registered program id
- Vue 2.7 setup analysis
- normal script option/default-export analysis
- normal script return bindings/imports
- binding metadata
- template usage index
- CSS vars
- public AST projection state

Then derive errors, generated content, bindings, imports, and AST projection from the context instead of reparsing or reanalyzing.

### 2. Make Public AST Projection Lazy Or Optional

The current public result shape includes `scriptAst` and `scriptSetupAst`, so this cannot be removed blindly from public APIs. The safer path is staged:

1. Add an internal `SfcScriptAstMode` option: `None`, `TopLevel`, `Full`.
2. Keep official public API aliases on the current full mode until output contracts prove a narrower default is safe.
3. Route internal benchmark/corpus paths that do not inspect AST through `None`.
4. Store enough parse identity in the compile context to materialize AST projection late when a caller or serializer requires it.

Projection should use a precomputed line index:

- build line start offsets once per source
- convert offsets with binary search
- compute UTF-16 columns from the line slice only
- avoid repeated full-prefix scans in `position_at()`

This keeps official result compatibility while removing unnecessary JSON allocation from hot internal paths.

### 3. Precompute Template Usage Once

Replace the current per-import template scan with a reusable `TemplateUsageIndex`.

Minimum viable change:

- compute `vue27_template_usage_check_string(template, is_ts)` once
- pass it through `vue27_script_setup_return_bindings()`
- use it for every import check

Better follow-up:

- parse the usage string into an identifier set once
- make identifier lookup O(1)
- preserve current expression processing boundaries for TS and template syntax

This is low risk because it should not change semantics; it only removes repeated work.

### 4. Deduplicate Vue 2.7 Setup Analysis

Refactor these consumers to share one analysis result:

- `vue27_script_compile_errors()`
- `vue27_script_setup_content()`
- `vue27_setup_binding_metadata()`

The context should expose:

- `setup_analysis(is_prod)`
- `setup_errors`
- `setup_bindings`
- `return_bindings`

If production mode changes analysis output, the context can cache by the small set of mode flags instead of recomputing unconditionally.

### 5. Treat Descriptor Reuse As Secondary

The bridge currently parses source for `sfc.vue27.compileScript`. Descriptor reuse across `parse()` and `compileScript()` may save some time, but it should come after:

1. AST projection mode
2. template usage index
3. shared setup analysis

Those are Rust-internal structural costs and do not require changing the public package call contract.

### 6. Add Reproducible Perf Gates

Before implementing performance changes, add or restore a reproducible command that writes stable artifacts, for example:

```text
cargo xtask profile-compile-script --fixture-corpus compat/perf/vue27-sfc --iterations 60
```

Required report fields:

- Rust commit
- official compiler package version
- release/profile/debug mode
- fixture list and file sizes
- median/p95 per phase
- whether public AST projection was enabled
- whether template usage index was enabled

Acceptance gates after implementation:

- Vue 2.7 SFC conformance still passes.
- Vue 2 project corpus still passes for affected modes.
- Output contracts for `vue/compiler-sfc` still pass.
- Public API output shape still includes `scriptAst` / `scriptSetupAst` when the public mode requires it.
- Median `compileScript` time improves on the same corpus, with no worse p95 regression on large templates.

## Prioritized Milestones

1. Reporting truth pass:
   - Add prepared-test manifest.
   - Add provenance dimensions to reports.
   - Stop using path whitelist alone to upgrade coverage to `rust-backed`.
   - Fix docs that currently contradict current reports.

2. Adapter boundary split:
   - Split `ALIAS_RUNTIME_JS` roles.
   - Tag semantic JS shims and callback boundaries.
   - Mark suite-only bridge commands separately from public APIs.

3. CompileScript low-risk performance pass:
   - Add shared Vue 2.7 compile context.
   - Precompute template usage once.
   - Deduplicate setup analysis.

4. AST projection performance pass:
   - Add AST projection mode.
   - Add line index for location projection.
   - Route internal/corpus/benchmark paths through no-AST mode where safe.

5. Evidence hardening:
   - Add reproducible compileScript profiling command.
   - Promote Vue 2 project corpus into summary/CI release evidence.
   - Add future Vue 3 project corpus planning separately.

## What Not To Change Yet

- Do not remove JavaScript callback support for PostCSS plugins or caller-provided transforms. Those are real JS API boundaries.
- Do not call prepared official tests "unmodified official tests."
- Do not delete suite bridge commands before equivalent public Rust API or focused Rust tests exist.
- Do not change public `compileScript` result shape until output contracts and API consumers prove the mode is safe.
- Do not use a new performance number as an acceptance gate until the profiling artifact can be reproduced from a checked-in command.

## Review Questions

- Should `rust-backed` continue to exist as a public report label, or should reports move entirely to provenance categories?
- Should suite-only bridge commands ever count toward Rust completion, or only toward migration progress?
- Should public `compileScript` default to full AST projection forever, with only internal callers opting out?
- Should the Vue 2 project corpus become part of `summarize-compat`, or stay as a separate production corpus gate with its own summary line?
