fn conformance_coverage_report(
    spec: ConformanceSuiteSpec,
    backend: AliasBackend,
    execution: Option<&ConformanceExecutionResult>,
) -> ConformanceCoverageReport {
    let default_provenance = conformance_default_coverage_provenance(spec, backend);
    let source = default_provenance.legacy_source();
    let default_reason = conformance_coverage_reason(spec, backend).to_string();
    let counts = execution.map(|result| result.counts).unwrap_or_default();
    let prepared_manifest = execution
        .and_then(|result| result.prepared_manifest_file.as_deref())
        .and_then(|path| read_json::<PreparedTestManifest>(Path::new(path)).ok());
    let files = execution
        .and_then(|result| {
            conformance_coverage_files(
                spec,
                backend,
                result,
                prepared_manifest.as_ref(),
                &default_reason,
            )
            .ok()
        })
        .unwrap_or_default();
    let report_source =
        conformance_coverage_report_kind(source, &files, prepared_manifest.as_ref());
    let counts_by_source =
        conformance_counts_by_source(report_source, counts, &files, prepared_manifest.as_ref());
    let summary = conformance_counts_by_execution_path(&default_provenance, counts, &files);
    let rust_backed = counts_by_source
        .get(ConformanceCoverageKind::RustBacked.as_str())
        .copied()
        .unwrap_or_default();
    let reason = conformance_coverage_report_reason(spec, backend, report_source, &default_reason);
    ConformanceCoverageReport {
        source: report_source,
        reason,
        summary,
        counts_by_source,
        rust_backed_pass: rust_backed.pass,
        rust_backed_total: rust_backed.total,
        files,
    }
}

fn conformance_counts_by_source(
    report_source: ConformanceCoverageKind,
    default_counts: ConformanceExecutionCounts,
    files: &[ConformanceCoverageFile],
    manifest: Option<&PreparedTestManifest>,
) -> BTreeMap<String, ConformanceExecutionCounts> {
    let mut counts_by_source = BTreeMap::new();
    for kind in [
        ConformanceCoverageKind::RustBacked,
        ConformanceCoverageKind::ShimBacked,
        ConformanceCoverageKind::Mixed,
    ] {
        counts_by_source.insert(
            kind.as_str().to_string(),
            ConformanceExecutionCounts::default(),
        );
    }
    if manifest.is_some_and(|manifest| manifest_contains_mixed_official_source_boundary(manifest))
        || files.is_empty()
    {
        if let Some(bucket) = counts_by_source.get_mut(report_source.as_str()) {
            *bucket = default_counts;
        }
    } else {
        for file in files {
            if let Some(bucket) = counts_by_source.get_mut(file.source.as_str()) {
                accumulate_counts(bucket, file.counts);
            }
        }
    }
    counts_by_source
}

fn conformance_counts_by_execution_path(
    default_provenance: &ConformanceCoverageProvenance,
    default_counts: ConformanceExecutionCounts,
    files: &[ConformanceCoverageFile],
) -> BTreeMap<String, ConformanceExecutionCounts> {
    let mut summary = BTreeMap::new();
    for execution_path in [
        "pure-rust-public-api",
        "rust-bridge-shape-adapter",
        "hybrid-js-adapter-rust-projection",
        "mixed-js-callback-boundary",
        "shim-backed-semantic-js",
    ] {
        summary.insert(
            execution_path.to_string(),
            ConformanceExecutionCounts::default(),
        );
    }
    if files.is_empty() {
        if let Some(bucket) = summary.get_mut(&default_provenance.execution_path) {
            *bucket = default_counts;
        }
    } else {
        for file in files {
            if let Some(bucket) = summary.get_mut(&file.provenance.execution_path) {
                accumulate_counts(bucket, file.counts);
            }
        }
    }
    summary
}

fn accumulate_counts(target: &mut ConformanceExecutionCounts, counts: ConformanceExecutionCounts) {
    target.total += counts.total;
    target.pass += counts.pass;
    target.fail += counts.fail;
    target.skip += counts.skip;
    target.pending += counts.pending;
}

fn conformance_coverage_report_reason(
    spec: ConformanceSuiteSpec,
    backend: AliasBackend,
    report_source: ConformanceCoverageKind,
    default_reason: &str,
) -> String {
    match (backend, spec.name, report_source) {
        (AliasBackend::Generated, "vue3-sfc", ConformanceCoverageKind::RustBacked) => {
            "Vue 3 compiler-sfc official tests run through a prepared Vitest suite whose files are all routed to public @vue/compiler-sfc helpers or Rust-backed projection helpers; generated import/API adapters only preserve official test import paths, materialize non-serializable test inputs, and hydrate public result shapes while compiler behavior routes through vuec_node_bridge into Rust."
                .to_string()
        }
        (AliasBackend::Generated, "vue3-core", ConformanceCoverageKind::Mixed) => {
            "Vue 3 compiler-core official tests now route serializable parser, transform, and codegen assertions through Rust-backed public APIs or vuec_node_bridge projection helpers. The remaining mixed coverage is limited to official tests that exercise caller-provided JavaScript NodeTransform/directiveTransform callbacks and mutable transform context APIs, which cannot be serialized into the Rust bridge and are not counted as Rust compiler completion evidence."
                .to_string()
        }
        _ => default_reason.to_string(),
    }
}

fn conformance_coverage_report_kind(
    default: ConformanceCoverageKind,
    files: &[ConformanceCoverageFile],
    manifest: Option<&PreparedTestManifest>,
) -> ConformanceCoverageKind {
    if manifest.is_some_and(|manifest| manifest_contains_mixed_official_source_boundary(manifest)) {
        return ConformanceCoverageKind::Mixed;
    }
    let Some(first) = files.first() else {
        return default;
    };
    if files.iter().all(|file| file.source == first.source) {
        first.source
    } else {
        ConformanceCoverageKind::Mixed
    }
}

fn manifest_contains_mixed_official_source_boundary(manifest: &PreparedTestManifest) -> bool {
    manifest.entries.iter().any(|entry| {
        entry
            .expected_provenance
            .api_surface
            .contains("mixed-official-source-boundary")
    })
}

fn conformance_default_coverage_provenance(
    spec: ConformanceSuiteSpec,
    backend: AliasBackend,
) -> ConformanceCoverageProvenance {
    match backend {
        AliasBackend::Napi => ConformanceCoverageProvenance::new(
            "prepared-official",
            "hybrid-js-adapter-rust-projection",
            "public-package-api",
            &["import-rewrite", "runner-support"],
            &[],
        ),
        AliasBackend::Generated => match spec.name {
            "vue2-compiler" | "vue27-compiler" => ConformanceCoverageProvenance::new(
                "prepared-official",
                "rust-bridge-shape-adapter",
                "public-package-api",
                &["import-rewrite", "hydration-dehydration"],
                &["vue2.compile"],
            ),
            "vue27-sfc" => ConformanceCoverageProvenance::new(
                "prepared-official",
                "hybrid-js-adapter-rust-projection",
                "public-package-api",
                &["import-rewrite", "hydration-dehydration"],
                &[
                    "sfc.vue27.parse",
                    "sfc.vue27.compileTemplate",
                    "sfc.vue27.compileScript",
                    "sfc.vue27.compileStyle",
                ],
            ),
            "vue3-core" | "vue3-dom" | "vue3-sfc" | "vue3-ssr" => {
                ConformanceCoverageProvenance::new(
                    "prepared-official",
                    "hybrid-js-adapter-rust-projection",
                    "internal-helper-import",
                    &["import-rewrite", "hydration-dehydration"],
                    &[],
                )
            }
            _ => ConformanceCoverageProvenance::new(
                "custom-regression",
                "rust-bridge-shape-adapter",
                "public-package-api",
                &["hydration-dehydration"],
                &[],
            ),
        },
    }
}

fn conformance_coverage_reason(spec: ConformanceSuiteSpec, backend: AliasBackend) -> &'static str {
    match backend {
        AliasBackend::Napi => match spec.name {
            "vue2-compiler" => {
                "Vue 2.6 compiler official tests execute through a prepared Jasmine suite whose public vue-template-compiler package request resolves to the NAPI-backed official package-name alias. Prepared source shims still adapt official internal imports, so this report is mixed harness coverage; failures are real NAPI/Rust compiler parity gaps, not not-wired pending status."
            }
            "vue27-compiler" => {
                "Vue 2.7 compiler official tests execute through a prepared Vitest suite whose public vue-template-compiler package request resolves to the NAPI-backed official package-name alias. Prepared source shims still adapt official internal imports, so this report is mixed harness coverage; failures are real NAPI/Rust compiler parity gaps, not not-wired pending status."
            }
            "vue27-sfc" => {
                "Vue 2.7 compiler-sfc official tests execute through a prepared Vitest suite whose public vue/compiler-sfc package request resolves to the NAPI-backed official package-name alias. Prepared source shims and JavaScript-only callback adapters still participate, so this report is mixed harness coverage; failures are real NAPI/Rust SFC parity gaps, not not-wired pending status."
            }
            "vue3-core" => {
                "Vue 3 compiler-core official tests execute through a prepared Vitest suite whose public @vue/compiler-core package request resolves to the NAPI-backed official package-name alias. Generated source-path shims and internal helper adapters still participate, so this report is mixed harness coverage rather than pure NAPI semantic coverage."
            }
            "vue3-dom" => {
                "Vue 3 compiler-dom official tests execute through a prepared Vitest suite whose public @vue/compiler-dom and @vue/compiler-core package requests resolve to NAPI-backed official package-name aliases. Official TypeScript source, generated source-path shims, and helper adapters still participate, so this report is mixed harness coverage."
            }
            "vue3-sfc" => {
                "Vue 3 compiler-sfc official tests execute through a prepared Vitest suite whose public compiler package requests resolve to NAPI-backed official package-name aliases. Official SFC TypeScript source, generated source-path shims, and JavaScript-only helper adapters still participate, so this report is mixed harness coverage rather than standalone Rust SFC parity."
            }
            "vue3-ssr" => {
                "Vue 3 compiler-ssr official tests execute through a prepared Vitest suite whose public @vue/compiler-ssr and @vue/compiler-core package requests resolve to NAPI-backed official package-name aliases. Official SSR/DOM TypeScript source, generated source-path shims, and helper adapters still participate, so this report is mixed harness coverage."
            }
            _ => "Suite is routed through NAPI-backed official package-name aliases with prepared source shims.",
        },
        AliasBackend::Generated => match spec.name {
            "vue2-compiler" => {
                "Vue 2.6 compiler official tests execute through a prepared Jasmine suite. Generated source-path import shims preserve official internal module requests and route compiler/codeframe calls into the Rust vue-template-compiler alias through vuec_node_bridge; these failures are real Rust compiler parity gaps, not not-wired pending status."
            }
            "vue27-compiler" => {
                "Vue 2.7 compiler official tests execute through a prepared Vitest suite. Generated source-path import shims preserve official internal module requests and route compiler/codeframe calls into the Rust vue-template-compiler alias through vuec_node_bridge; these failures are real Rust compiler parity gaps, not not-wired pending status."
            }
            "vue27-sfc" => {
                "Vue 2.7 compiler-sfc official tests execute through a prepared Vitest suite. Generated source-path import shims preserve official imports and route public vue/compiler-sfc calls into the Rust alias through vuec_node_bridge; compileStyle PostCSS plugin callbacks execute in the JavaScript API adapter because caller-provided plugins are JavaScript functions and cannot be serialized into Rust. Remaining failures are real Vue 2.7 SFC parity gaps, not not-wired pending status."
            }
            "vue3-core" => {
                "Vue 3 compiler-core official tests run through generated import shims and the @vue/compiler-core alias runtime; serializable compiler behavior is routed through Rust-backed public APIs or vuec_node_bridge projection helpers, while caller-provided JavaScript callback/context extension points remain mixed and are excluded from Rust compiler completion evidence."
            }
            "vue3-dom" => {
                "Vue 3 compiler-dom official tests run through a prepared Vitest suite with official DOM source imports, generated compiler-core import shims, and the @vue/compiler-dom alias runtime. Public compile/parse exports call the Rust bridge, but internal DOM transform imports mostly execute official TypeScript source or compatibility adapter code; only explicitly bridged projections count as Rust-backed."
            }
            "vue3-sfc" => {
                "Vue 3 compiler-sfc official tests run through a prepared Vitest suite with official SFC TypeScript source and generated aliases for @vue/compiler-core, @vue/compiler-dom, @vue/compiler-ssr, and @vue/compiler-sfc. The SFC source under test executes mixed official TypeScript logic plus Rust alias bridge calls for compiler dependencies; this runner is conformance harness coverage, not standalone Rust SFC parity."
            }
            "vue3-ssr" => {
                "Vue 3 compiler-ssr official tests run through a prepared Vitest suite with official SSR and DOM source imports, generated compiler-core import shims, and the alias runtime. Public @vue/compiler-ssr exports call the Rust bridge, but prepared SSR source tests execute mixed official TypeScript source, alias adapter code, and Rust bridge projections."
            }
            _ => "Suite is routed through Rust alias package smoke/output paths.",
        },
    }
}
