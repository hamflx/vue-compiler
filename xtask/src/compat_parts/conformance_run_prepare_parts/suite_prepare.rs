fn prepare_vue2_compiler_conformance_suite(
    spec: ConformanceSuiteSpec,
    official_root: &Path,
    lock_hash: Option<&str>,
) -> Result<PathBuf> {
    let prepared_root = prepared_conformance_root(spec, lock_hash);
    reset_prepared_root(&prepared_root)?;
    let official_tests = official_root
        .join("test")
        .join("unit")
        .join("modules")
        .join("compiler");
    let prepared_tests = prepared_root
        .join("test")
        .join("unit")
        .join("modules")
        .join("compiler");
    copy_dir_recursive(&official_tests, &prepared_tests)?;
    write_vue2_compiler_source_shims(&prepared_root, false)?;
    write_vue2_jasmine_runner(&prepared_root)?;
    write_prepared_test_manifest_for_suite(spec, &prepared_root)?;
    Ok(prepared_root)
}

fn prepare_vue27_compiler_conformance_suite(
    spec: ConformanceSuiteSpec,
    official_root: &Path,
    lock_hash: Option<&str>,
) -> Result<PathBuf> {
    let prepared_root = prepared_conformance_root(spec, lock_hash);
    reset_prepared_root(&prepared_root)?;
    let official_tests = official_root
        .join("test")
        .join("unit")
        .join("modules")
        .join("compiler");
    let prepared_tests = prepared_root
        .join("test")
        .join("unit")
        .join("modules")
        .join("compiler");
    copy_dir_recursive(&official_tests, &prepared_tests)?;
    write_vue2_compiler_source_shims(&prepared_root, true)?;
    write_vue27_compiler_conformance_shims(&prepared_root)?;
    write_prepared_test_manifest_for_suite(spec, &prepared_root)?;
    Ok(prepared_root)
}

fn prepare_vue27_sfc_conformance_suite(
    spec: ConformanceSuiteSpec,
    official_root: &Path,
    lock_hash: Option<&str>,
) -> Result<PathBuf> {
    let prepared_root = prepared_conformance_root(spec, lock_hash);
    reset_prepared_root(&prepared_root)?;
    let official_tests = official_root
        .join("packages")
        .join("compiler-sfc")
        .join("test");
    let prepared_tests = prepared_root
        .join("packages")
        .join("compiler-sfc")
        .join("test");
    copy_dir_recursive(&official_tests, &prepared_tests)?;
    fs::copy(
        official_root.join("tsconfig.json"),
        prepared_root.join("tsconfig.json"),
    )
    .with_context(|| "failed to copy Vue 2.7 root tsconfig for SFC conformance")?;
    write_vue2_compiler_source_shims(&prepared_root, true)?;
    write_vue27_sfc_source_shims(&prepared_root)?;
    write_vue27_sfc_conformance_shims(&prepared_root)?;
    write_prepared_test_manifest_for_suite(spec, &prepared_root)?;
    Ok(prepared_root)
}

fn prepare_vue3_core_conformance_suite(
    spec: ConformanceSuiteSpec,
    official_root: &Path,
    lock_hash: Option<&str>,
) -> Result<PathBuf> {
    let prepared_root = prepared_conformance_root(spec, lock_hash);
    reset_prepared_root(&prepared_root)?;
    let official_tests = official_root
        .join("packages")
        .join("compiler-core")
        .join("__tests__");
    let prepared_tests = prepared_root
        .join("packages")
        .join("compiler-core")
        .join("__tests__");
    copy_dir_recursive(&official_tests, &prepared_tests)?;
    write_vue3_core_conformance_shims(&prepared_root)?;
    write_prepared_test_manifest_for_suite(spec, &prepared_root)?;
    Ok(prepared_root)
}

fn prepared_conformance_root(spec: ConformanceSuiteSpec, lock_hash: Option<&str>) -> PathBuf {
    PathBuf::from("target")
        .join("conformance")
        .join(lock_hash.unwrap_or("unknown-lock"))
        .join("prepared")
        .join(spec.name)
}

fn reset_prepared_root(prepared_root: &Path) -> Result<()> {
    if prepared_root.exists() {
        fs::remove_dir_all(prepared_root)
            .with_context(|| format!("failed to remove {}", prepared_root.display()))?;
    }
    Ok(())
}

fn prepared_test_manifest_path(prepared_root: &Path) -> PathBuf {
    prepared_root.join("prepared-test-manifest.json")
}

fn prepared_manifest_file(prepared_root: &Path) -> Option<String> {
    let path = prepared_test_manifest_path(prepared_root);
    path.exists().then(|| path.display().to_string())
}

fn prepared_test_manifest_report(path: &str) -> Option<PreparedTestManifestReport> {
    let path = Path::new(path);
    let manifest = read_json::<PreparedTestManifest>(path).ok()?;
    Some(PreparedTestManifestReport {
        official_test_origin: manifest.derived_origin().to_string(),
        manifest_file: path.display().to_string(),
        entry_count: manifest.entries.len(),
        alias_runtime_fragments: manifest.alias_runtime_fragments,
    })
}

fn write_prepared_test_manifest_for_suite(
    spec: ConformanceSuiteSpec,
    prepared_root: &Path,
) -> Result<()> {
    let manifest = prepared_test_manifest_for_suite(spec);
    write_json(&prepared_test_manifest_path(prepared_root), &manifest)
}
