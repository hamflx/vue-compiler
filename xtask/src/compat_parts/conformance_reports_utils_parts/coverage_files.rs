fn conformance_coverage_files(
    spec: ConformanceSuiteSpec,
    backend: AliasBackend,
    execution: &ConformanceExecutionResult,
    manifest: Option<&PreparedTestManifest>,
    reason: &str,
) -> Result<Vec<ConformanceCoverageFile>> {
    let output_file = PathBuf::from(&execution.output_file);
    let value = read_json::<serde_json::Value>(&output_file)?;
    let default_provenance = conformance_default_coverage_provenance(spec, backend);
    let mut files = Vec::new();
    if let Some(results) = value.get("testResults").and_then(|value| value.as_array()) {
        for result in results {
            let path = result
                .get("name")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .replace('\\', "/");
            files.extend(conformance_coverage_file_entries(
                spec,
                backend,
                &path,
                result,
                manifest,
                &default_provenance,
                reason,
            ));
        }
    }
    Ok(files)
}

fn conformance_coverage_file_entries(
    spec: ConformanceSuiteSpec,
    backend: AliasBackend,
    path: &str,
    result: &serde_json::Value,
    manifest: Option<&PreparedTestManifest>,
    default_provenance: &ConformanceCoverageProvenance,
    reason: &str,
) -> Vec<ConformanceCoverageFile> {
    if path.ends_with("packages/compiler-core/__tests__/transforms/transformElement.spec.ts") {
        let entries = conformance_coverage_transform_element_entries(path, result, reason);
        if !entries.is_empty() {
            return entries;
        }
    }
    if path.ends_with("packages/compiler-core/__tests__/transform.spec.ts") {
        let entries = conformance_coverage_transform_entries(path, result, reason);
        if !entries.is_empty() {
            return entries;
        }
    }

    let counts = json_conformance_file_counts(result);
    let provenance =
        conformance_file_provenance(spec, backend, path, result, manifest, default_provenance);
    let file_reason = conformance_coverage_file_reason(path, &provenance, reason);
    vec![conformance_coverage_file(
        path,
        None,
        provenance,
        file_reason,
        counts,
    )]
}

fn conformance_coverage_file(
    path: &str,
    scope: Option<&str>,
    provenance: ConformanceCoverageProvenance,
    reason: String,
    counts: ConformanceExecutionCounts,
) -> ConformanceCoverageFile {
    let source = provenance.legacy_source();
    ConformanceCoverageFile {
        path: path.to_string(),
        scope: scope.map(str::to_string),
        source,
        provenance,
        reason,
        counts,
    }
}

fn conformance_file_provenance(
    spec: ConformanceSuiteSpec,
    backend: AliasBackend,
    path: &str,
    result: &serde_json::Value,
    manifest: Option<&PreparedTestManifest>,
    default_provenance: &ConformanceCoverageProvenance,
) -> ConformanceCoverageProvenance {
    let base = manifest
        .and_then(|manifest| conformance_manifest_entry_for_path(manifest, path))
        .map(ConformanceCoverageProvenance::from_prepared_expectation)
        .unwrap_or_else(|| {
            conformance_path_default_provenance(spec, backend, path, default_provenance)
        });
    base.with_runtime_markers(conformance_runtime_markers(result))
}

fn conformance_manifest_entry_for_path<'a>(
    manifest: &'a PreparedTestManifest,
    path: &str,
) -> Option<&'a PreparedTestManifestEntry> {
    let path = path.replace('\\', "/");
    manifest
        .entries
        .iter()
        .filter(|entry| !entry.prepared_path.contains("**/*.spec.ts"))
        .filter(|entry| {
            path_matches_manifest_entry(&path, &entry.prepared_path)
                || entry
                    .helper_path
                    .as_deref()
                    .is_some_and(|helper| path_matches_manifest_entry(&path, helper))
        })
        .max_by_key(|entry| entry.prepared_path.len())
}

fn path_matches_manifest_entry(path: &str, manifest_path: &str) -> bool {
    let manifest_path = manifest_path.replace('\\', "/");
    if let Some(prefix) = manifest_path.strip_suffix("/**") {
        return path.contains(prefix);
    }
    path.ends_with(&manifest_path)
}

fn conformance_path_default_provenance(
    spec: ConformanceSuiteSpec,
    backend: AliasBackend,
    path: &str,
    default: &ConformanceCoverageProvenance,
) -> ConformanceCoverageProvenance {
    if backend == AliasBackend::Generated
        && spec.name == "vue27-sfc"
        && path.ends_with("packages/compiler-sfc/test/compileStyle.spec.ts")
    {
        return ConformanceCoverageProvenance::new(
            "prepared-official",
            "mixed-js-callback-boundary",
            "public-package-api",
            &[
                "import-rewrite",
                "hydration-dehydration",
                "callback-materialization",
            ],
            &["sfc.vue27.compileStyle", "sfc.vue27.compileStyleAsync"],
        );
    }
    default.clone()
}

fn conformance_runtime_markers(result: &serde_json::Value) -> Vec<String> {
    let mut markers = Vec::new();
    collect_conformance_runtime_markers(result.get("vuecProvenance"), &mut markers);
    collect_conformance_runtime_markers(result.get("__vuecProvenance"), &mut markers);
    collect_conformance_runtime_markers(result.get("coverageProvenance"), &mut markers);
    if let Some(assertions) = result
        .get("assertionResults")
        .and_then(|value| value.as_array())
    {
        for assertion in assertions {
            collect_conformance_runtime_markers(assertion.get("vuecProvenance"), &mut markers);
            collect_conformance_runtime_markers(assertion.get("__vuecProvenance"), &mut markers);
            collect_conformance_runtime_markers(assertion.get("coverageProvenance"), &mut markers);
        }
    }
    markers
}

fn conformance_runtime_markers_from_assertions(assertions: &[&serde_json::Value]) -> Vec<String> {
    let mut markers = Vec::new();
    for assertion in assertions {
        collect_conformance_runtime_markers(assertion.get("vuecProvenance"), &mut markers);
        collect_conformance_runtime_markers(assertion.get("__vuecProvenance"), &mut markers);
        collect_conformance_runtime_markers(assertion.get("coverageProvenance"), &mut markers);
    }
    markers
}

fn collect_conformance_runtime_markers(
    value: Option<&serde_json::Value>,
    markers: &mut Vec<String>,
) {
    match value {
        Some(serde_json::Value::String(marker)) => push_unique_string(markers, marker),
        Some(serde_json::Value::Array(values)) => {
            for value in values {
                collect_conformance_runtime_markers(Some(value), markers);
            }
        }
        Some(serde_json::Value::Object(object)) => {
            for key in ["runtime_markers", "runtimeMarkers", "markers"] {
                collect_conformance_runtime_markers(object.get(key), markers);
            }
        }
        _ => {}
    }
}
