fn verify_release_docs() -> Result<compat::JsonReport> {
    let mut violations = Vec::new();
    let mut items = Vec::new();

    for path in [
        PathBuf::from("README.md"),
        PathBuf::from("CHANGELOG.md"),
        PathBuf::from("docs").join("COMPATIBILITY_MATRIX.md"),
        PathBuf::from("docs").join("RELEASE_CHECKLIST.md"),
        PathBuf::from("docs").join("CONFORMANCE_REPORT_TEMPLATE.md"),
        PathBuf::from("docs").join("ARCHITECTURE.md"),
        PathBuf::from("docs").join("API.md"),
        PathBuf::from("docs").join("SECURITY_SUPPLY_CHAIN.md"),
    ] {
        match require_non_empty_file(&path) {
            Ok(()) => items.push(compat::ReportItem::new(
                format!("doc:{}", path.display()),
                compat::ReportStatus::Pass,
                "release documentation file exists and is non-empty",
                Some(path),
            )),
            Err(err) => {
                violations.push(format!("{err:#}"));
                items.push(compat::ReportItem::new(
                    format!("doc:{}", path.display()),
                    compat::ReportStatus::Fail,
                    format!("{err:#}"),
                    Some(path),
                ));
            }
        }
    }

    let package_dirs = collect_package_manifest_dirs(Path::new("packages"))?;
    if package_dirs.is_empty() {
        violations.push("no package.json files found under packages".into());
    }

    let template_path = PathBuf::from("docs").join("CONFORMANCE_REPORT_TEMPLATE.md");
    let template_requirements = [
        "## Report Identity",
        "## Official Baselines",
        "## Execution Scope",
        "## Coverage Classification",
        "## File-Level Coverage",
        "## Failure Summary",
        "## Compatibility Concerns",
        "## Acceptance Decision",
        "rust-backed",
        "mixed",
        "shim-backed",
        "Lock hash",
        "Official lock file",
        "xtask/src/compat.rs",
    ];
    match require_file_contains_all(&template_path, &template_requirements) {
        Ok(()) => items.push(compat::ReportItem::new(
            "conformance-report-template",
            compat::ReportStatus::Pass,
            "template contains required report identity, baseline, scope, coverage, failure, compatibility, and acceptance sections",
            Some(template_path),
        )),
        Err(err) => {
            violations.push(format!("{err:#}"));
            items.push(compat::ReportItem::new(
                "conformance-report-template",
                compat::ReportStatus::Fail,
                format!("{err:#}"),
                Some(template_path),
            ));
        }
    }

    let architecture_path = PathBuf::from("docs").join("ARCHITECTURE.md");
    let architecture_requirements = [
        "## Layering",
        "## Workspace Map",
        "## AST / HIR / MIR Contract",
        "## Public Projection",
        "## Entry Points",
        "## Compatibility Harness Boundary",
        "## Conformance Evidence",
        "## Release Gates",
        "AstDocument<K>",
        "LoweringMap",
        "Vue2Ast",
        "Vue3Ast",
        "Vue3DomMir",
        "Vue3SsrMir",
        "xtask/src/compat.rs",
        "rust-backed",
        "mixed",
        "shim-backed",
    ];
    match require_file_contains_all(&architecture_path, &architecture_requirements) {
        Ok(()) => items.push(compat::ReportItem::new(
            "architecture-doc",
            compat::ReportStatus::Pass,
            "architecture document covers layering, workspace map, AST/HIR/MIR contract, public projection, entry points, harness boundaries, conformance evidence, and release gates",
            Some(architecture_path),
        )),
        Err(err) => {
            violations.push(format!("{err:#}"));
            items.push(compat::ReportItem::new(
                "architecture-doc",
                compat::ReportStatus::Fail,
                format!("{err:#}"),
                Some(architecture_path),
            ));
        }
    }

    let api_path = PathBuf::from("docs").join("API.md");
    let api_requirements = [
        "## Rust Crate APIs",
        "## CLI",
        "## NAPI",
        "## WASM",
        "## Official Package-Name Aliases",
        "## Verification",
        "compileVue2",
        "compileVue3Dom",
        "compileSfcTemplate",
        "baseCompileVue3",
        "compile-template",
        "verify-napi-api",
        "diff-api",
    ];
    match require_file_contains_all(&api_path, &api_requirements) {
        Ok(()) => items.push(compat::ReportItem::new(
            "api-doc",
            compat::ReportStatus::Pass,
            "API document covers Rust crate, CLI, NAPI, WASM, official alias, and verification surfaces",
            Some(api_path),
        )),
        Err(err) => {
            violations.push(format!("{err:#}"));
            items.push(compat::ReportItem::new(
                "api-doc",
                compat::ReportStatus::Fail,
                format!("{err:#}"),
                Some(api_path),
            ));
        }
    }

    let security_path = PathBuf::from("docs").join("SECURITY_SUPPLY_CHAIN.md");
    let security_requirements = [
        "## Locked Inputs",
        "## Package Metadata",
        "## Audit Commands",
        "## Artifact Provenance",
        "## Compatibility Boundary",
        "Cargo.lock",
        "pnpm@9.0.0",
        "compat/official-revisions.lock",
        "cargo audit",
        "pnpm audit --prod",
        "xtask/src/compat.rs",
    ];
    match require_file_contains_all(&security_path, &security_requirements) {
        Ok(()) => items.push(compat::ReportItem::new(
            "security-supply-chain-doc",
            compat::ReportStatus::Pass,
            "security and supply-chain document covers locked inputs, metadata, audits, provenance, and compatibility boundaries",
            Some(security_path),
        )),
        Err(err) => {
            violations.push(format!("{err:#}"));
            items.push(compat::ReportItem::new(
                "security-supply-chain-doc",
                compat::ReportStatus::Fail,
                format!("{err:#}"),
                Some(security_path),
            ));
        }
    }
    for package_dir in package_dirs {
        let manifest_path = package_dir.join("package.json");
        let name = read_package_display_name(&manifest_path)
            .unwrap_or_else(|_| package_dir.display().to_string());
        let readme_path = package_dir.join("README.md");
        let mut package_violations = Vec::new();
        if let Err(err) = require_non_empty_file(&readme_path) {
            package_violations.push(format!("{err:#}"));
        }
        match package_files_array_includes_readme(&manifest_path) {
            Ok(true) => {}
            Ok(false) => package_violations.push(format!(
                "{} has a files array that does not include README.md",
                manifest_path.display()
            )),
            Err(err) => package_violations.push(format!("{err:#}")),
        }

        let status = if package_violations.is_empty() {
            compat::ReportStatus::Pass
        } else {
            violations.extend(package_violations.iter().cloned());
            compat::ReportStatus::Fail
        };
        let detail = if package_violations.is_empty() {
            "package README exists; files array includes README.md when present".into()
        } else {
            package_violations.join("; ")
        };
        items.push(compat::ReportItem::new(
            format!("package:{name}"),
            status,
            detail,
            Some(package_dir),
        ));
    }

    let status = if violations.is_empty() {
        compat::ReportStatus::Pass
    } else {
        compat::ReportStatus::Fail
    };
    Ok(
        compat::JsonReport::new("verify_release_docs", status)
            .with_items(items)
            .with_violations(violations)
            .with_note("verifies M20 release documentation skeletons, API documentation, package README coverage, and explicit README.md package file-list entries"),
    )
}

fn require_non_empty_file(path: &Path) -> Result<()> {
    let text =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    if text.trim().is_empty() {
        anyhow::bail!("{} is empty", path.display());
    }
    Ok(())
}

fn require_file_contains_all(path: &Path, required: &[&str]) -> Result<()> {
    let text =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let missing = required
        .iter()
        .copied()
        .filter(|needle| !text.contains(needle))
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        anyhow::bail!(
            "{} missing required text: {}",
            path.display(),
            missing.join(", ")
        );
    }
    Ok(())
}

fn verify_public_api_docs() -> Result<compat::JsonReport> {
    let documented_crates = [
        "vuec_source",
        "vuec_diagnostics",
        "vuec_codegen",
        "vuec_ast",
        "vuec_html",
        "vuec_js",
        "vuec_pass",
        "vuec_style",
        "vuec_vue3_asset",
        "vuec_vue3_core",
        "vuec_vue3_dom",
        "vuec_vue3_ssr",
        "vuec_vue2",
        "vuec_sfc",
        "vuec_napi",
        "vuec_wasm",
        "vuec_node_bridge",
        "vuec_runtime_tests",
        "vuec_cli",
        "xtask",
    ];
    let mut items = Vec::new();
    let mut violations = Vec::new();

    for crate_name in documented_crates {
        match verify_crate_missing_docs(crate_name) {
            Ok(output) => items.push(compat::ReportItem::new(
                format!("rustdoc:{crate_name}"),
                compat::ReportStatus::Pass,
                output,
                Some(crate_manifest_path(crate_name)?),
            )),
            Err(err) => {
                violations.push(format!("{crate_name}: {err:#}"));
                items.push(compat::ReportItem::new(
                    format!("rustdoc:{crate_name}"),
                    compat::ReportStatus::Fail,
                    format!("{err:#}"),
                    Some(crate_manifest_path(crate_name)?),
                ));
            }
        }
    }

    let status = if violations.is_empty() {
        compat::ReportStatus::Pass
    } else {
        compat::ReportStatus::Fail
    };
    Ok(
        compat::JsonReport::new("verify_public_api_docs", status)
            .with_items(items)
            .with_violations(violations)
            .with_note("verifies missing-docs rustdoc coverage for the documented public API crate set; expand this gate as additional public crates are documented"),
    )
}

fn verify_crate_missing_docs(crate_name: &str) -> Result<String> {
    let output = ProcessCommand::new("cargo")
        .args(["doc", "--no-deps", "-p", crate_name])
        .env("RUSTDOCFLAGS", "-D missing_docs")
        .output()
        .with_context(|| format!("failed to spawn cargo doc -p {crate_name}"))?;
    if !output.status.success() {
        anyhow::bail!(
            "cargo doc -p {crate_name} exited with {:?}\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(normalize_command_output(&output.stdout, &output.stderr))
}

fn crate_manifest_path(crate_name: &str) -> Result<PathBuf> {
    let metadata = cargo_metadata_json()?;
    let packages = metadata
        .get("packages")
        .and_then(JsonValue::as_array)
        .context("cargo metadata output did not include packages array")?;
    packages
        .iter()
        .find(|package| package.get("name").and_then(JsonValue::as_str) == Some(crate_name))
        .and_then(|package| package.get("manifest_path").and_then(JsonValue::as_str))
        .map(PathBuf::from)
        .with_context(|| format!("failed to find manifest for crate {crate_name}"))
}

fn collect_package_manifest_dirs(root: &Path) -> Result<Vec<PathBuf>> {
    let mut dirs = Vec::new();
    collect_package_manifest_dirs_inner(root, &mut dirs)?;
    dirs.sort();
    Ok(dirs)
}

fn collect_package_manifest_dirs_inner(path: &Path, dirs: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(path).with_context(|| format!("failed to read {}", path.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if is_generated_package_output_dir(&path) {
                continue;
            }
            let manifest_path = path.join("package.json");
            if manifest_path.exists() {
                dirs.push(path.clone());
            }
            collect_package_manifest_dirs_inner(&path, dirs)?;
        }
    }
    Ok(())
}

fn is_generated_package_output_dir(path: &Path) -> bool {
    path == Path::new("packages").join("wasm").join("pkg")
        || path == Path::new("packages").join("wasm").join("pkg-node")
}

fn read_package_display_name(manifest_path: &Path) -> Result<String> {
    let manifest = read_json_file(manifest_path)?;
    manifest
        .get("name")
        .and_then(JsonValue::as_str)
        .map(ToOwned::to_owned)
        .with_context(|| format!("{} has no package name", manifest_path.display()))
}

fn package_files_array_includes_readme(manifest_path: &Path) -> Result<bool> {
    let manifest = read_json_file(manifest_path)?;
    let Some(files) = manifest.get("files") else {
        return Ok(true);
    };
    let Some(files) = files.as_array() else {
        anyhow::bail!("{} files field is not an array", manifest_path.display());
    };
    Ok(files
        .iter()
        .filter_map(JsonValue::as_str)
        .any(|entry| entry == "README.md" || entry == "./README.md"))
}

fn verify_crate_metadata() -> Result<compat::JsonReport> {
    let metadata = cargo_metadata_json()?;
    let packages = metadata
        .get("packages")
        .and_then(JsonValue::as_array)
        .context("cargo metadata output did not include packages array")?;
    let mut items = Vec::new();
    let mut violations = Vec::new();

    for package in packages {
        let name = package
            .get("name")
            .and_then(JsonValue::as_str)
            .unwrap_or("<unknown>");
        let manifest_path = package
            .get("manifest_path")
            .and_then(JsonValue::as_str)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("<unknown>"));
        let publishable = package_is_publishable(package);
        let mut package_violations = Vec::new();

        for field in [
            "description",
            "repository",
            "homepage",
            "documentation",
            "readme",
        ] {
            if package
                .get(field)
                .and_then(JsonValue::as_str)
                .is_none_or(str::is_empty)
            {
                package_violations.push(format!("{name} missing package.{field}"));
            }
        }
        if package
            .get("license")
            .and_then(JsonValue::as_str)
            .is_none_or(str::is_empty)
        {
            package_violations.push(format!("{name} missing package.license"));
        }
        if package
            .get("keywords")
            .and_then(JsonValue::as_array)
            .is_none_or(|array| array.is_empty())
        {
            package_violations.push(format!("{name} missing package.keywords"));
        }
        if package
            .get("categories")
            .and_then(JsonValue::as_array)
            .is_none_or(|array| array.is_empty())
        {
            package_violations.push(format!("{name} missing package.categories"));
        }

        if let Some(readme) = package.get("readme").and_then(JsonValue::as_str) {
            let readme_path = manifest_path
                .parent()
                .map(|path| path.join(readme))
                .unwrap_or_else(|| PathBuf::from(readme));
            if let Err(err) = require_non_empty_file(&readme_path) {
                package_violations.push(format!("{err:#}"));
            }
        }

        if publishable {
            for dependency in package
                .get("dependencies")
                .and_then(JsonValue::as_array)
                .into_iter()
                .flatten()
                .filter(|dependency| dependency.get("path").is_some())
            {
                let dep_name = dependency
                    .get("name")
                    .and_then(JsonValue::as_str)
                    .unwrap_or("<unknown>");
                let version_req = dependency
                    .get("req")
                    .and_then(JsonValue::as_str)
                    .unwrap_or("*");
                if version_req == "*" {
                    package_violations.push(format!(
                        "{name} path dependency {dep_name} must include a crates.io version requirement"
                    ));
                }
            }
        }

        let status = if package_violations.is_empty() {
            compat::ReportStatus::Pass
        } else {
            violations.extend(package_violations.iter().cloned());
            compat::ReportStatus::Fail
        };
        let detail = if package_violations.is_empty() {
            if publishable {
                "crate metadata is publish-ready; path dependencies include version requirements"
                    .into()
            } else {
                "internal crate metadata is present and publish=false".into()
            }
        } else {
            package_violations.join("; ")
        };
        items.push(compat::ReportItem::new(
            format!("crate:{name}"),
            status,
            detail,
            Some(manifest_path),
        ));
    }

    let status = if violations.is_empty() {
        compat::ReportStatus::Pass
    } else {
        compat::ReportStatus::Fail
    };
    Ok(
        compat::JsonReport::new("verify_crate_metadata", status)
            .with_items(items)
            .with_violations(violations)
            .with_note("verifies M20 crates.io metadata, crate READMEs, publish=false internal crate boundaries, and versioned path dependencies for publishable crates"),
    )
}

fn verify_supply_chain() -> Result<compat::JsonReport> {
    let mut violations = Vec::new();
    let mut items = Vec::new();

    for path in [
        PathBuf::from("Cargo.lock"),
        PathBuf::from("compat").join("official-revisions.lock"),
        PathBuf::from("docs").join("SECURITY_SUPPLY_CHAIN.md"),
    ] {
        push_file_check_item(&mut items, &mut violations, path);
    }

    match root_package_manager_is_pinned(Path::new("package.json")) {
        Ok(detail) => items.push(compat::ReportItem::new(
            "root-package-manager",
            compat::ReportStatus::Pass,
            detail,
            Some(PathBuf::from("package.json")),
        )),
        Err(err) => {
            violations.push(format!("{err:#}"));
            items.push(compat::ReportItem::new(
                "root-package-manager",
                compat::ReportStatus::Fail,
                format!("{err:#}"),
                Some(PathBuf::from("package.json")),
            ));
        }
    }

    let package_dirs = collect_package_manifest_dirs(Path::new("packages"))?;
    for package_dir in package_dirs {
        let manifest_path = package_dir.join("package.json");
        let name = read_package_display_name(&manifest_path)
            .unwrap_or_else(|_| package_dir.display().to_string());
        let mut package_violations = Vec::new();
        match verify_npm_manifest_supply_chain(&manifest_path) {
            Ok(()) => {}
            Err(err) => package_violations.push(format!("{err:#}")),
        }
        let status = if package_violations.is_empty() {
            compat::ReportStatus::Pass
        } else {
            violations.extend(package_violations.iter().cloned());
            compat::ReportStatus::Fail
        };
        let detail = if package_violations.is_empty() {
            "npm manifest has license metadata, exact dependency versions, and stable package file metadata where required".into()
        } else {
            package_violations.join("; ")
        };
        items.push(compat::ReportItem::new(
            format!("npm-package:{name}"),
            status,
            detail,
            Some(manifest_path),
        ));
    }

    match cargo_metadata_json() {
        Ok(metadata) => {
            let package_count = metadata
                .get("packages")
                .and_then(JsonValue::as_array)
                .map_or(0, Vec::len);
            items.push(compat::ReportItem::new(
                "cargo-metadata",
                compat::ReportStatus::Pass,
                format!("cargo metadata resolved {package_count} workspace packages"),
                Some(PathBuf::from("Cargo.toml")),
            ));
        }
        Err(err) => {
            violations.push(format!("{err:#}"));
            items.push(compat::ReportItem::new(
                "cargo-metadata",
                compat::ReportStatus::Fail,
                format!("{err:#}"),
                Some(PathBuf::from("Cargo.toml")),
            ));
        }
    }

    let status = if violations.is_empty() {
        compat::ReportStatus::Pass
    } else {
        compat::ReportStatus::Fail
    };
    Ok(
        compat::JsonReport::new("verify_supply_chain", status)
            .with_items(items)
            .with_violations(violations)
            .with_note("verifies M20 security/supply-chain release controls: lock files, pinned package manager, npm license metadata, exact dependency versions, stable package files, and Cargo metadata resolution"),
    )
}

const REQUIRED_CI_JOBS: &[&str] = &[
    "Compatibility (ubuntu-latest)",
    "Compatibility (macos-latest)",
    "Compatibility (windows-latest)",
    "Product Smoke",
    "Release Install Smoke (ubuntu-latest)",
    "Release Install Smoke (macos-latest)",
    "Release Install Smoke (windows-latest)",
    "Release Dry Run",
];

#[derive(Clone, Debug, Deserialize)]
struct GithubWorkflowRun {
    id: u64,
    head_sha: Option<String>,
    status: Option<String>,
    conclusion: Option<String>,
    html_url: Option<String>,
    run_number: Option<u64>,
}

#[derive(Clone, Debug, Deserialize)]
struct GithubJob {
    name: String,
    status: Option<String>,
    conclusion: Option<String>,
    html_url: Option<String>,
}

fn verify_ci_status(
    repo: Option<&str>,
    commit: Option<&str>,
    workflow: &str,
    runs_json: Option<&Path>,
    jobs_json: Option<&Path>,
) -> Result<compat::JsonReport> {
    if runs_json.is_some() && jobs_json.is_none() {
        anyhow::bail!("--jobs-json is required when --runs-json is supplied");
    }

    let repo = match repo.map(str::to_owned).or_else(default_github_repo) {
        Some(repo) => repo,
        None => {
            return Ok(ci_status_pending_report(
                "github-repository",
                "GitHub repository could not be inferred; pass --repo owner/name",
                None,
            ));
        }
    };
    let commit = match commit.map(str::to_owned).or_else(default_git_commit) {
        Some(commit) => commit,
        None => {
            return Ok(ci_status_pending_report(
                "git-commit",
                "Git commit could not be inferred; pass --commit <sha>",
                None,
            ));
        }
    };

    let runs_value = match runs_json {
        Some(path) => read_json_file(path).with_context(|| {
            format!("failed to read CI workflow runs fixture {}", path.display())
        })?,
        None => match github_api_json(
            &repo,
            &format!(
                "actions/workflows/{}/runs?head_sha={}&per_page=10",
                github_path_component(workflow),
                github_path_component(&commit)
            ),
        ) {
            Ok(value) => value,
            Err(err) => {
                return Ok(ci_status_pending_report(
                    "github-actions-runs",
                    format!("GitHub Actions workflow run evidence is unavailable: {err:#}"),
                    None,
                ));
            }
        },
    };
    let runs = parse_github_workflow_runs(runs_value)?;
    let Some(run) = select_workflow_run_for_commit(&runs, &commit) else {
        return Ok(ci_status_pending_report(
            format!("workflow:{workflow}@{commit}"),
            format!("no GitHub Actions workflow run for {repo}/{workflow} at commit {commit}"),
            runs_json.map(Path::to_path_buf),
        ));
    };

    let mut items = Vec::new();
    let mut violations = Vec::new();
    let run_status = ci_status_from_github(run.status.as_deref(), run.conclusion.as_deref());
    let run_detail = ci_run_detail(&repo, workflow, &commit, run);
    if run_status == compat::ReportStatus::Fail {
        violations.push(format!("workflow run failed: {run_detail}"));
    }
    items.push(compat::ReportItem::new(
        format!("workflow:{workflow}@{commit}"),
        run_status,
        run_detail,
        runs_json.map(Path::to_path_buf),
    ));

    let jobs_value = match jobs_json {
        Some(path) => read_json_file(path)
            .with_context(|| format!("failed to read CI jobs fixture {}", path.display()))?,
        None => match github_api_json(&repo, &format!("actions/runs/{}/jobs?per_page=100", run.id))
        {
            Ok(value) => value,
            Err(err) => {
                let detail = format!("GitHub Actions job evidence is unavailable: {err:#}");
                for job in REQUIRED_CI_JOBS {
                    items.push(compat::ReportItem::new(
                        format!("job:{job}"),
                        compat::ReportStatus::Pending,
                        detail.clone(),
                        None,
                    ));
                }
                return Ok(compat::JsonReport::new(
                    "verify_ci_status",
                    compat::ReportStatus::Pending,
                )
                .with_items(items)
                .with_note("verifies that the repository CI workflow has successful Windows, Linux, macOS compatibility jobs, product smoke, release install-smoke jobs, and release dry-run evidence for the requested commit"));
            }
        },
    };
    let jobs = parse_github_jobs(jobs_value)?;
    for required in REQUIRED_CI_JOBS {
        let matching = jobs.iter().find(|job| job.name == *required);
        let (status, detail) = match matching {
            Some(job) => (
                ci_status_from_github(job.status.as_deref(), job.conclusion.as_deref()),
                ci_job_detail(job),
            ),
            None => {
                let status = if run.status.as_deref() == Some("completed") {
                    compat::ReportStatus::Fail
                } else {
                    compat::ReportStatus::Pending
                };
                (
                    status,
                    format!(
                        "required job `{required}` is missing from CI workflow run {}",
                        run.id
                    ),
                )
            }
        };
        if status == compat::ReportStatus::Fail {
            violations.push(format!("{required}: {detail}"));
        }
        items.push(compat::ReportItem::new(
            format!("job:{required}"),
            status,
            detail,
            jobs_json.map(Path::to_path_buf),
        ));
    }

    Ok(
        compat::JsonReport::new("verify_ci_status", compat::ReportStatus::Pass)
            .with_items(items)
            .with_violations(violations)
            .with_note(format!(
                "verifies CI evidence for {repo}/{workflow} at {commit}; required jobs: {}",
                REQUIRED_CI_JOBS.join(", ")
            )),
    )
}

fn ci_status_pending_report(
    target: impl Into<String>,
    detail: impl Into<String>,
    path: Option<PathBuf>,
) -> compat::JsonReport {
    compat::JsonReport::new("verify_ci_status", compat::ReportStatus::Pending)
        .with_items(vec![compat::ReportItem::new(
            target,
            compat::ReportStatus::Pending,
            detail,
            path,
        )])
        .with_note("verifies that the repository CI workflow has successful Windows, Linux, macOS compatibility jobs, product smoke, release install-smoke jobs, and release dry-run evidence for the requested commit")
}

fn parse_github_workflow_runs(value: JsonValue) -> Result<Vec<GithubWorkflowRun>> {
    if value.is_array() {
        return serde_json::from_value(value).context("failed to parse GitHub workflow run array");
    }
    let runs = value
        .get("workflow_runs")
        .cloned()
        .context("GitHub workflow runs JSON missing workflow_runs")?;
    serde_json::from_value(runs).context("failed to parse GitHub workflow_runs")
}

fn parse_github_jobs(value: JsonValue) -> Result<Vec<GithubJob>> {
    if value.is_array() {
        return serde_json::from_value(value).context("failed to parse GitHub job array");
    }
    let jobs = value
        .get("jobs")
        .cloned()
        .context("GitHub jobs JSON missing jobs")?;
    serde_json::from_value(jobs).context("failed to parse GitHub jobs")
}

fn select_workflow_run_for_commit<'a>(
    runs: &'a [GithubWorkflowRun],
    commit: &str,
) -> Option<&'a GithubWorkflowRun> {
    runs.iter()
        .find(|run| run.head_sha.as_deref() == Some(commit))
}

fn ci_status_from_github(status: Option<&str>, conclusion: Option<&str>) -> compat::ReportStatus {
    if status != Some("completed") {
        return compat::ReportStatus::Pending;
    }
    match conclusion {
        Some("success") => compat::ReportStatus::Pass,
        None => compat::ReportStatus::Pending,
        _ => compat::ReportStatus::Fail,
    }
}

fn ci_run_detail(repo: &str, workflow: &str, commit: &str, run: &GithubWorkflowRun) -> String {
    format!(
        "repo={repo}, workflow={workflow}, commit={commit}, run_id={}, run_number={}, status={}, conclusion={}, url={}",
        run.id,
        run.run_number
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unknown".into()),
        run.status.as_deref().unwrap_or("unknown"),
        run.conclusion.as_deref().unwrap_or("unknown"),
        run.html_url.as_deref().unwrap_or("unknown")
    )
}

fn ci_job_detail(job: &GithubJob) -> String {
    format!(
        "job={}, status={}, conclusion={}, url={}",
        job.name,
        job.status.as_deref().unwrap_or("unknown"),
        job.conclusion.as_deref().unwrap_or("unknown"),
        job.html_url.as_deref().unwrap_or("unknown")
    )
}

fn github_api_json(repo: &str, endpoint: &str) -> Result<JsonValue> {
    let curl = resolve_program("curl")?;
    let url = format!("https://api.github.com/repos/{repo}/{endpoint}");
    let mut command = ProcessCommand::new(curl);
    command
        .args([
            "-fsSL",
            "-H",
            "Accept: application/vnd.github+json",
            "-H",
            "X-GitHub-Api-Version: 2022-11-28",
        ])
        .arg("-H")
        .arg("User-Agent: vuec-xtask")
        .arg(&url);
    if let Some(token) = std::env::var("GITHUB_TOKEN")
        .ok()
        .or_else(|| std::env::var("GH_TOKEN").ok())
        .filter(|token| !token.trim().is_empty())
    {
        command
            .arg("-H")
            .arg(format!("Authorization: Bearer {token}"));
    }
    let output = command
        .output()
        .with_context(|| format!("failed to spawn curl for {url}"))?;
    if !output.status.success() {
        anyhow::bail!(
            "GitHub API request failed for {url} with {:?}\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    serde_json::from_slice(&output.stdout).with_context(|| {
        format!(
            "GitHub API response was not JSON for {url}:\n{}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

fn github_path_component(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'.' | b'-' | b'_' => {
                encoded.push(byte as char)
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

fn default_github_repo() -> Option<String> {
    std::env::var("GITHUB_REPOSITORY")
        .ok()
        .filter(|repo| !repo.trim().is_empty())
        .or_else(|| {
            command_output("git", &["remote", "get-url", "origin"])
                .and_then(|remote| parse_github_remote_repo(&remote))
        })
}

fn default_git_commit() -> Option<String> {
    std::env::var("GITHUB_SHA")
        .ok()
        .filter(|sha| !sha.trim().is_empty())
        .or_else(|| command_output("git", &["rev-parse", "HEAD"]))
}

fn parse_github_remote_repo(remote: &str) -> Option<String> {
    let trimmed = remote.trim();
    let repo = trimmed
        .strip_prefix("git@github.com:")
        .or_else(|| trimmed.strip_prefix("ssh://git@github.com/"))
        .or_else(|| trimmed.strip_prefix("https://github.com/"))
        .or_else(|| trimmed.strip_prefix("http://github.com/"))?;
    let repo = repo.strip_suffix(".git").unwrap_or(repo);
    let mut parts = repo.split('/');
    let owner = parts.next()?;
    let name = parts.next()?;
    if owner.is_empty() || name.is_empty() {
        return None;
    }
    Some(format!("{owner}/{name}"))
}
