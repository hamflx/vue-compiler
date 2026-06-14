fn conformance_targets(suites: &[ConformanceSuite]) -> Vec<TargetSpec> {
    let mut targets = Vec::new();
    for suite in suites {
        for target in conformance_smoke_targets(suite_spec(*suite)) {
            if !targets.contains(&target) {
                targets.push(target);
            }
        }
    }
    targets
}

fn conformance_readiness(
    spec: ConformanceSuiteSpec,
    backend: AliasBackend,
) -> ConformanceReadiness {
    let alias_root = backend.root(spec.version_line);
    let npm_root = PathBuf::from("target")
        .join("compat")
        .join("npm")
        .join(spec.version_line.as_str())
        .join("node_modules");
    let missing_alias_packages = spec
        .package_requests
        .iter()
        .filter(|request| !alias_package_available(&alias_root, request))
        .map(|request| (*request).to_string())
        .collect::<Vec<_>>();
    let mut missing_runner_dependencies = spec
        .runner_dependencies
        .iter()
        .filter(|dependency| !node_dependency_available(&npm_root, dependency))
        .map(|dependency| (*dependency).to_string())
        .collect::<Vec<_>>();
    if missing_runner_dependencies.is_empty() {
        missing_runner_dependencies.extend(conformance_runner_startup_dependency_errors(
            spec, &npm_root,
        ));
    }
    ConformanceReadiness {
        alias_ready: missing_alias_packages.is_empty(),
        runner_ready: missing_runner_dependencies.is_empty(),
        package_requests: spec
            .package_requests
            .iter()
            .map(|request| (*request).to_string())
            .collect(),
        runner_dependencies: spec
            .runner_dependencies
            .iter()
            .map(|dependency| (*dependency).to_string())
            .collect(),
        missing_alias_packages,
        missing_runner_dependencies,
    }
}

fn alias_package_available(alias_root: &Path, request: &str) -> bool {
    if request == "vue/compiler-sfc" {
        return alias_root
            .join("node_modules")
            .join("vue")
            .join("compiler-sfc")
            .join("index.js")
            .is_file();
    }
    node_dependency_available(&alias_root.join("node_modules"), request)
}

fn node_dependency_available(node_modules: &Path, request: &str) -> bool {
    let segments = request.split('/').collect::<Vec<_>>();
    let package_dir = if request.starts_with('@') && segments.len() >= 2 {
        node_modules.join(segments[0]).join(segments[1])
    } else {
        node_modules.join(segments[0])
    };
    package_dir.join("package.json").is_file() || package_dir.join("index.js").is_file()
}

fn verify_conformance_runner_startup_dependencies(
    spec: ConformanceSuiteSpec,
    node_modules: &Path,
) -> Result<()> {
    let errors = conformance_runner_startup_dependency_errors(spec, node_modules);
    if errors.is_empty() {
        return Ok(());
    }
    anyhow::bail!("{}", errors.join("; "))
}

fn conformance_runner_startup_dependency_errors(
    spec: ConformanceSuiteSpec,
    node_modules: &Path,
) -> Vec<String> {
    conformance_runner_startup_probe_requests(spec)
        .into_iter()
        .filter(|request| node_dependency_available(node_modules, request))
        .filter_map(|request| {
            probe_conformance_runner_startup_dependency(node_modules, request)
                .err()
                .map(|err| format!("runner-native:{request}: {err:#}"))
        })
        .collect()
}

fn conformance_runner_startup_probe_requests(spec: ConformanceSuiteSpec) -> Vec<&'static str> {
    if !spec.runner_dependencies.contains(&"vitest") {
        return Vec::new();
    }
    vec!["rollup", "rolldown"]
}

fn probe_conformance_runner_startup_dependency(node_modules: &Path, request: &str) -> Result<()> {
    let node = resolve_program("node");
    let output = Command::new(node)
        .arg("--input-type=module")
        .arg("-e")
        .arg(RUNNER_STARTUP_PROBE_SCRIPT)
        .env("VUEC_RUNNER_NODE_MODULES", absolute_path(node_modules))
        .env("VUEC_RUNNER_PROBE_REQUEST", request)
        .output()
        .with_context(|| format!("failed to spawn node runner startup probe for {request}"))?;
    if !output.status.success() {
        anyhow::bail!(
            "node runner startup probe failed with status {:?}: {}",
            output.status.code(),
            normalize_command_output(&output.stdout, &output.stderr)
        );
    }
    Ok(())
}

fn conformance_item_detail(
    test_count: usize,
    readiness: &ConformanceReadiness,
    execution: Option<&ConformanceExecutionResult>,
) -> String {
    if let Some(execution) = execution {
        return format!(
            "{}/{} official tests passed, {} failed, {} skipped, {} pending",
            execution.counts.pass,
            execution.counts.total,
            execution.counts.fail,
            execution.counts.skip,
            execution.counts.pending
        );
    }
    if readiness.alias_ready && readiness.runner_ready {
        return format!("{test_count} official test files discovered; runner is ready to execute");
    }
    let mut missing = Vec::new();
    if !readiness.alias_ready {
        missing.push(format!(
            "missing alias packages: {}",
            readiness.missing_alias_packages.join(", ")
        ));
    }
    if !readiness.runner_ready {
        missing.push(format!(
            "missing runner dependencies: {}",
            readiness.missing_runner_dependencies.join(", ")
        ));
    }
    format!(
        "{test_count} official test files discovered; execution blocked by {}",
        missing.join("; ")
    )
}

fn suite_spec(suite: ConformanceSuite) -> ConformanceSuiteSpec {
    match suite {
        ConformanceSuite::Vue2Compiler => ConformanceSuiteSpec {
            name: "vue2-compiler",
            version_line: VersionLine::Vue26,
            relative_test_dirs: &["test/unit/modules/compiler"],
            package_requests: &["vue-template-compiler"],
            runner_dependencies: &["@babel/register", "jasmine", "jsdom"],
        },
        ConformanceSuite::Vue27Compiler => ConformanceSuiteSpec {
            name: "vue27-compiler",
            version_line: VersionLine::Vue27,
            relative_test_dirs: &["test/unit/modules/compiler"],
            package_requests: &["vue-template-compiler"],
            runner_dependencies: &["vitest", "esbuild", "typescript", "jsdom"],
        },
        ConformanceSuite::Vue27Sfc => ConformanceSuiteSpec {
            name: "vue27-sfc",
            version_line: VersionLine::Vue27,
            relative_test_dirs: &["packages/compiler-sfc/test"],
            package_requests: &["vue/compiler-sfc"],
            runner_dependencies: &["vitest", "esbuild", "typescript", "jsdom"],
        },
        ConformanceSuite::Vue3Core => ConformanceSuiteSpec {
            name: "vue3-core",
            version_line: VersionLine::Vue3,
            relative_test_dirs: &["packages/compiler-core/__tests__"],
            package_requests: &["@vue/compiler-core"],
            runner_dependencies: &["vitest", "esbuild", "source-map-js"],
        },
        ConformanceSuite::Vue3Dom => ConformanceSuiteSpec {
            name: "vue3-dom",
            version_line: VersionLine::Vue3,
            relative_test_dirs: &["packages/compiler-dom/__tests__"],
            package_requests: &["@vue/compiler-dom", "@vue/compiler-core"],
            runner_dependencies: &["vitest", "esbuild", "source-map-js", "jsdom"],
        },
        ConformanceSuite::Vue3Sfc => ConformanceSuiteSpec {
            name: "vue3-sfc",
            version_line: VersionLine::Vue3,
            relative_test_dirs: &["packages/compiler-sfc/__tests__"],
            package_requests: &[
                "@vue/compiler-core",
                "@vue/compiler-dom",
                "@vue/compiler-sfc",
                "@vue/compiler-ssr",
            ],
            runner_dependencies: &[
                "@babel/parser",
                "@babel/types",
                "@vue/consolidate",
                "esbuild",
                "estree-walker",
                "hash-sum",
                "lru-cache",
                "magic-string",
                "merge-source-map",
                "minimatch",
                "postcss-modules",
                "postcss-selector-parser",
                "pug",
                "sass",
                "source-map-js",
                "typescript",
                "vitest",
            ],
        },
        ConformanceSuite::Vue3Ssr => ConformanceSuiteSpec {
            name: "vue3-ssr",
            version_line: VersionLine::Vue3,
            relative_test_dirs: &["packages/compiler-ssr/__tests__"],
            package_requests: &["@vue/compiler-ssr", "@vue/compiler-core"],
            runner_dependencies: &["vitest", "esbuild"],
        },
    }
}

fn select_conformance_suites(args: &ConformanceArgs) -> Vec<ConformanceSuite> {
    if args.all {
        return vec![
            ConformanceSuite::Vue2Compiler,
            ConformanceSuite::Vue27Compiler,
            ConformanceSuite::Vue27Sfc,
            ConformanceSuite::Vue3Core,
            ConformanceSuite::Vue3Dom,
            ConformanceSuite::Vue3Sfc,
            ConformanceSuite::Vue3Ssr,
        ];
    }
    args.suite
        .map(|suite| vec![suite])
        .unwrap_or_else(|| vec![ConformanceSuite::Vue3Core])
}

fn discover_test_files(dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            discover_test_files(&path, out);
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let is_test = file_name.ends_with(".spec.ts")
            || file_name.ends_with(".spec.js")
            || file_name.ends_with(".test.ts")
            || file_name.ends_with(".test.js");
        if is_test {
            out.push(path.display().to_string());
        }
    }
}

fn aggregate_status(items: &[ReportItem]) -> ReportStatus {
    if items.iter().any(|item| item.status == ReportStatus::Fail) {
        ReportStatus::Fail
    } else if items
        .iter()
        .any(|item| item.status == ReportStatus::Pending)
    {
        ReportStatus::Pending
    } else {
        ReportStatus::Pass
    }
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let json = serde_json::to_string_pretty(value)?;
    fs::write(path, json).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let data =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let value = serde_json::from_str(&data)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(value)
}

fn sync_git_checkout(repo: &str, rev: &str, dir: &Path, submodules: bool) -> Result<()> {
    if dir.join(".git").exists() {
        ensure_existing_git_checkout_matches(repo, dir)?;
        run_git(dir, &["fetch", "--tags", "--force", "origin"])?;
    } else {
        if let Some(parent) = dir.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        let dir_arg = dir.display().to_string();
        run_command("git", &["clone", repo, &dir_arg], None)
            .with_context(|| format!("failed to clone {repo} into {}", dir.display()))?;
    }
    run_git(dir, &["checkout", "--detach", rev])?;
    if submodules {
        run_git(dir, &["submodule", "update", "--init", "--recursive"])?;
    }
    Ok(())
}

fn ensure_existing_git_checkout_matches(expected_repo: &str, dir: &Path) -> Result<()> {
    let actual_repo = git_output(dir, &["remote", "get-url", "origin"])
        .with_context(|| format!("failed to inspect origin for {}", dir.display()))?;
    ensure!(
        normalize_git_remote_url(&actual_repo) == normalize_git_remote_url(expected_repo),
        "refusing to reuse {}; origin is {}, expected {}",
        dir.display(),
        actual_repo,
        expected_repo
    );

    let status = git_output(dir, &["status", "--porcelain"])
        .with_context(|| format!("failed to inspect git status for {}", dir.display()))?;
    let dirty_lines = official_sync_dirty_status_lines(&status);
    ensure!(
        dirty_lines.is_empty(),
        "refusing to checkout {} because it has local changes",
        dir.display()
    );
    Ok(())
}

fn official_sync_dirty_status_lines(status: &str) -> Vec<&str> {
    status
        .lines()
        .filter(|line| !is_official_sync_metadata_status_line(line))
        .collect()
}

fn is_official_sync_metadata_status_line(line: &str) -> bool {
    line.get(3..).map(str::trim).is_some_and(|path| {
        path == "official-revision.json" || path == "\"official-revision.json\""
    })
}

fn normalize_git_remote_url(url: &str) -> String {
    let mut normalized = url.trim().trim_end_matches('/').to_string();
    if let Some(stripped) = normalized.strip_suffix(".git") {
        normalized = stripped.to_string();
    }
    normalized
}

fn run_git(dir: &Path, args: &[&str]) -> Result<()> {
    run_command("git", args, Some(dir))
}

fn run_command(program: &str, args: &[&str], current_dir: Option<&Path>) -> Result<()> {
    let mut command = Command::new(program);
    command.args(args);
    if let Some(current_dir) = current_dir {
        command.current_dir(current_dir);
    }
    let output = command
        .output()
        .with_context(|| format!("failed to spawn {program} {}", args.join(" ")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        anyhow::bail!(
            "`{} {}` failed with status {:?}\nstdout:\n{}\nstderr:\n{}",
            program,
            args.join(" "),
            output.status.code(),
            stdout.trim(),
            stderr.trim()
        );
    }
    Ok(())
}

fn command_output(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn file_sha256(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}
