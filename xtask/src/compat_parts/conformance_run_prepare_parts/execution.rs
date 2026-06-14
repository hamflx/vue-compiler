fn run_conformance_smokes(
    spec: ConformanceSuiteSpec,
    backend: AliasBackend,
) -> Vec<ConformanceSmokeResult> {
    conformance_smoke_targets(spec)
        .into_iter()
        .map(|target| {
            let request = api_require_request(target);
            match run_alias_smoke(target, &backend.root(target.version_line)) {
                Ok(detail) => ConformanceSmokeResult {
                    request,
                    status: "pass".into(),
                    detail,
                },
                Err(err) => ConformanceSmokeResult {
                    request,
                    status: "fail".into(),
                    detail: format!("{err:#}"),
                },
            }
        })
        .collect()
}

fn conformance_smoke_targets(spec: ConformanceSuiteSpec) -> Vec<TargetSpec> {
    all_targets()
        .iter()
        .copied()
        .filter(|target| {
            target.version_line == spec.version_line
                && spec
                    .package_requests
                    .iter()
                    .any(|request| api_require_request(*target) == *request)
        })
        .collect()
}

fn run_conformance_execution(
    spec: ConformanceSuiteSpec,
    official_root: &Path,
    discovered: &[String],
    lock_hash: Option<&str>,
    backend: AliasBackend,
) -> Result<ConformanceExecutionResult> {
    match spec.name {
        "vue2-compiler" => {
            run_vue2_compiler_conformance(spec, official_root, discovered, lock_hash, backend)
        }
        "vue27-compiler" => {
            run_vue27_compiler_conformance(spec, official_root, discovered, lock_hash, backend)
        }
        "vue27-sfc" => {
            run_vue27_sfc_conformance(spec, official_root, discovered, lock_hash, backend)
        }
        "vue3-core" => {
            run_vue3_core_conformance(spec, official_root, discovered, lock_hash, backend)
        }
        "vue3-dom" => run_vue3_dom_conformance(spec, official_root, discovered, lock_hash, backend),
        "vue3-sfc" => run_vue3_sfc_conformance(spec, official_root, discovered, lock_hash, backend),
        "vue3-ssr" => run_vue3_ssr_conformance(spec, official_root, discovered, lock_hash, backend),
        _ => Ok(ConformanceExecutionResult {
            status: "pending".into(),
            runner: "not-wired".into(),
            prepared_root: String::new(),
            prepared_manifest_file: None,
            output_file: String::new(),
            exit_code: None,
            stdout: String::new(),
            stderr: format!("{} official execution is not wired yet", spec.name),
            counts: ConformanceExecutionCounts {
                total: discovered.len(),
                pending: discovered.len(),
                ..ConformanceExecutionCounts::default()
            },
        }),
    }
}

fn run_vue2_compiler_conformance(
    spec: ConformanceSuiteSpec,
    official_root: &Path,
    discovered: &[String],
    lock_hash: Option<&str>,
    backend: AliasBackend,
) -> Result<ConformanceExecutionResult> {
    let prepared_root = prepare_vue2_compiler_conformance_suite(spec, official_root, lock_hash)?;
    run_jasmine_conformance(spec, prepared_root, discovered, backend)
}

fn run_vue27_compiler_conformance(
    spec: ConformanceSuiteSpec,
    official_root: &Path,
    discovered: &[String],
    lock_hash: Option<&str>,
    backend: AliasBackend,
) -> Result<ConformanceExecutionResult> {
    let prepared_root = prepare_vue27_compiler_conformance_suite(spec, official_root, lock_hash)?;
    run_vitest_conformance(spec, prepared_root, discovered, backend)
}

fn run_vue27_sfc_conformance(
    spec: ConformanceSuiteSpec,
    official_root: &Path,
    discovered: &[String],
    lock_hash: Option<&str>,
    backend: AliasBackend,
) -> Result<ConformanceExecutionResult> {
    let prepared_root = prepare_vue27_sfc_conformance_suite(spec, official_root, lock_hash)?;
    run_vitest_conformance(spec, prepared_root, discovered, backend)
}

fn run_vue3_core_conformance(
    spec: ConformanceSuiteSpec,
    official_root: &Path,
    discovered: &[String],
    lock_hash: Option<&str>,
    backend: AliasBackend,
) -> Result<ConformanceExecutionResult> {
    let prepared_root = prepare_vue3_core_conformance_suite(spec, official_root, lock_hash)?;
    run_vitest_conformance(spec, prepared_root, discovered, backend)
}

fn run_vue3_dom_conformance(
    spec: ConformanceSuiteSpec,
    official_root: &Path,
    discovered: &[String],
    lock_hash: Option<&str>,
    backend: AliasBackend,
) -> Result<ConformanceExecutionResult> {
    let prepared_root = prepare_vue3_dom_conformance_suite(spec, official_root, lock_hash)?;
    run_vitest_conformance(spec, prepared_root, discovered, backend)
}

fn run_vue3_sfc_conformance(
    spec: ConformanceSuiteSpec,
    official_root: &Path,
    discovered: &[String],
    lock_hash: Option<&str>,
    backend: AliasBackend,
) -> Result<ConformanceExecutionResult> {
    let prepared_root = prepare_vue3_sfc_conformance_suite(spec, official_root, lock_hash)?;
    run_vitest_conformance(spec, prepared_root, discovered, backend)
}

fn run_vue3_ssr_conformance(
    spec: ConformanceSuiteSpec,
    official_root: &Path,
    discovered: &[String],
    lock_hash: Option<&str>,
    backend: AliasBackend,
) -> Result<ConformanceExecutionResult> {
    let prepared_root = prepare_vue3_ssr_conformance_suite(spec, official_root, lock_hash)?;
    run_vitest_conformance(spec, prepared_root, discovered, backend)
}

fn run_vitest_conformance(
    spec: ConformanceSuiteSpec,
    prepared_root: PathBuf,
    discovered: &[String],
    backend: AliasBackend,
) -> Result<ConformanceExecutionResult> {
    let output_file = prepared_root.join("vitest-report.json");
    let npm_root = PathBuf::from("target")
        .join("compat")
        .join("npm")
        .join(spec.version_line.as_str());
    let alias_root = backend.root(spec.version_line);
    let absolute_npm_root = absolute_path(&npm_root);
    let absolute_alias_root = absolute_path(&alias_root);
    let absolute_prepared_root = absolute_path(&prepared_root);
    let absolute_output_file = absolute_path(&output_file);
    let absolute_bridge_bin = absolute_path(&ensure_node_bridge_binary()?);
    let provenance_sidecar_base = absolute_prepared_root.join("vuec-provenance");
    remove_vitest_provenance_sidecars(&absolute_prepared_root)?;
    let node_modules = absolute_npm_root.join("node_modules");
    let vitest_bin = node_modules
        .join("vitest")
        .join("vitest.mjs")
        .display()
        .to_string();
    let node = resolve_program("node");
    let output = Command::new(node)
        .arg(vitest_bin)
        .arg("run")
        .arg("--globals")
        .arg("--testTimeout=30000")
        .arg("--reporter=json")
        .arg(format!("--outputFile={}", absolute_output_file.display()))
        .env("VUEC_NODE_BRIDGE", &absolute_bridge_bin)
        .env("VUEC_ALIAS_ROOT", &absolute_alias_root)
        .env("VUEC_RUST_ALIAS_ROOT", &absolute_alias_root)
        .env("VUEC_OFFICIAL_NPM_ROOT", &absolute_npm_root)
        .env("VUEC_PROVENANCE_SIDECAR", &provenance_sidecar_base)
        .env(
            "NODE_PATH",
            conformance_node_path(&absolute_alias_root, &absolute_npm_root),
        )
        .current_dir(&absolute_prepared_root)
        .output()
        .with_context(|| format!("failed to spawn Vitest for {}", spec.name))?;
    let stdout = normalize_conformance_output(&String::from_utf8_lossy(&output.stdout));
    let stderr = normalize_conformance_output(&String::from_utf8_lossy(&output.stderr));
    merge_vitest_provenance_sidecars(&output_file, &absolute_output_file, &absolute_prepared_root)?;
    let counts = read_vitest_counts(&output_file)
        .or_else(|_| read_vitest_counts(&absolute_output_file))
        .unwrap_or_else(|_| ConformanceExecutionCounts {
            total: discovered.len(),
            pending: discovered.len(),
            ..ConformanceExecutionCounts::default()
        });
    let status = if counts.fail > 0 || !output.status.success() {
        "failed"
    } else {
        "executed"
    };
    Ok(ConformanceExecutionResult {
        status: status.into(),
        runner: "vitest".into(),
        prepared_root: prepared_root.display().to_string(),
        prepared_manifest_file: prepared_manifest_file(&prepared_root),
        output_file: output_file.display().to_string(),
        exit_code: output.status.code(),
        stdout,
        stderr,
        counts,
    })
}

fn run_jasmine_conformance(
    spec: ConformanceSuiteSpec,
    prepared_root: PathBuf,
    discovered: &[String],
    backend: AliasBackend,
) -> Result<ConformanceExecutionResult> {
    let output_file = prepared_root.join("jasmine-report.json");
    let npm_root = PathBuf::from("target")
        .join("compat")
        .join("npm")
        .join(spec.version_line.as_str());
    let alias_root = backend.root(spec.version_line);
    let absolute_npm_root = absolute_path(&npm_root);
    let absolute_alias_root = absolute_path(&alias_root);
    let absolute_prepared_root = absolute_path(&prepared_root);
    let absolute_output_file = absolute_path(&output_file);
    let absolute_bridge_bin = absolute_path(&ensure_node_bridge_binary()?);
    let node = resolve_program("node");
    let output = Command::new(node)
        .arg("vuec-jasmine-runner.js")
        .env("VUEC_NODE_BRIDGE", &absolute_bridge_bin)
        .env("VUEC_ALIAS_ROOT", &absolute_alias_root)
        .env("VUEC_RUST_ALIAS_ROOT", &absolute_alias_root)
        .env("VUEC_OFFICIAL_NPM_ROOT", &absolute_npm_root)
        .env("VUEC_JASMINE_REPORT", &absolute_output_file)
        .env(
            "NODE_PATH",
            conformance_node_path(&absolute_alias_root, &absolute_npm_root),
        )
        .current_dir(&absolute_prepared_root)
        .output()
        .with_context(|| format!("failed to spawn Jasmine for {}", spec.name))?;
    let stdout = normalize_conformance_output(&String::from_utf8_lossy(&output.stdout));
    let stderr = normalize_conformance_output(&String::from_utf8_lossy(&output.stderr));
    let counts = read_jasmine_counts(&output_file)
        .or_else(|_| read_jasmine_counts(&absolute_output_file))
        .unwrap_or_else(|_| ConformanceExecutionCounts {
            total: discovered.len(),
            pending: discovered.len(),
            ..ConformanceExecutionCounts::default()
        });
    let status = if counts.fail > 0 || !output.status.success() {
        "failed"
    } else {
        "executed"
    };
    Ok(ConformanceExecutionResult {
        status: status.into(),
        runner: "jasmine".into(),
        prepared_root: prepared_root.display().to_string(),
        prepared_manifest_file: prepared_manifest_file(&prepared_root),
        output_file: output_file.display().to_string(),
        exit_code: output.status.code(),
        stdout,
        stderr,
        counts,
    })
}
