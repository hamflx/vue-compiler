const NPM_INSTALL_ARGS: &[&str] = &[
    "install",
    "--ignore-scripts",
    "--include=optional",
    "--no-audit",
    "--no-fund",
    "--package-lock=false",
    "--omit=dev",
];

fn ensure_official_npm_install(
    version_line: VersionLine,
    baseline: &BaselineLock,
) -> Result<PathBuf> {
    let install_root = PathBuf::from("target")
        .join("compat")
        .join("npm")
        .join(version_line.as_str());
    let node_modules = install_root.join("node_modules");
    let specs = baseline
        .npm
        .iter()
        .map(|(package, version)| format!("{package}@{version}"))
        .collect::<Vec<_>>();
    let marker = install_root.join("official-install.json");
    if node_modules.exists() && official_install_marker_matches(&marker, &specs) {
        return Ok(install_root);
    }
    reset_official_npm_node_modules(&install_root)?;
    fs::create_dir_all(&install_root)
        .with_context(|| format!("failed to create {}", install_root.display()))?;
    let package_json = serde_json::json!({
        "private": true,
        "name": format!("vuec-compat-{}", version_line.as_str()),
        "version": "0.0.0",
    });
    write_json(&install_root.join("package.json"), &package_json)?;
    run_npm_install_specs(
        &install_root,
        &specs,
        "official npm package install",
        false,
    )?;

    let marker_body = serde_json::json!({
        "version_line": version_line,
        "packages": specs,
        "rev": baseline.rev,
        "platform": npm_install_platform_marker(),
    });
    write_json(&marker, &marker_body)?;
    Ok(install_root)
}

fn official_install_marker_matches(marker: &Path, specs: &[String]) -> bool {
    let Ok(value) = read_json::<serde_json::Value>(marker) else {
        return false;
    };
    let Some(packages) = value.get("packages").and_then(|value| value.as_array()) else {
        return false;
    };
    let actual = packages
        .iter()
        .filter_map(|value| value.as_str().map(ToOwned::to_owned))
        .collect::<Vec<_>>();
    actual == specs
        && value
            .get("platform")
            .and_then(|value| value.as_str())
            .is_some_and(|platform| platform == npm_install_platform_marker())
}

fn npm_install_platform_marker() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

fn reset_official_npm_node_modules(install_root: &Path) -> Result<()> {
    ensure_target_compat_child(install_root, "npm")?;
    let node_modules = install_root.join("node_modules");
    if node_modules.exists() {
        fs::remove_dir_all(&node_modules)
            .with_context(|| format!("failed to remove {}", node_modules.display()))?;
    }
    Ok(())
}

fn run_npm_install_specs(
    install_root: &Path,
    specs: &[String],
    label: &str,
    legacy_peer_deps: bool,
) -> Result<()> {
    let npm = resolve_program("npm");
    let mut command = Command::new(npm);
    command.args(NPM_INSTALL_ARGS);
    if legacy_peer_deps {
        // npm 11.4 can fail while auto-resolving Vitest's optional peer graph.
        // The requested runner package versions stay locked, and ordinary dependencies
        // are still installed.
        command.arg("--legacy-peer-deps");
    }
    command.args(specs).current_dir(install_root);
    let output = command.output().with_context(|| {
        format!(
            "failed to spawn npm install for {label} in {}",
            install_root.display()
        )
    })?;
    if !output.status.success() {
        anyhow::bail!(
            "`npm install {}` for {label} failed with status {:?}\nstdout:\n{}\nstderr:\n{}",
            specs.join(" "),
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).trim(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

fn ensure_official_runner_dependencies(
    spec: ConformanceSuiteSpec,
    baseline: &BaselineLock,
    vendor_dir: &Path,
) -> Result<PathBuf> {
    let install_root = ensure_official_npm_install(spec.version_line, baseline)?;
    let node_modules = install_root.join("node_modules");
    let runner_specs = runner_dependency_specs(spec, vendor_dir)?.ok_or_else(|| {
        anyhow::anyhow!(
            "{} has runner dependencies but no deterministic versions could be resolved",
            spec.name
        )
    })?;
    if runner_specs.is_empty() {
        return Ok(install_root);
    }
    let marker = install_root.join(format!("runner-install-{}.json", spec.name));
    if node_modules.exists()
        && official_install_marker_matches(&marker, &runner_specs)
        && verify_conformance_runner_startup_dependencies(spec, &node_modules).is_ok()
    {
        return Ok(install_root);
    }

    run_npm_install_specs(
        &install_root,
        &runner_specs,
        "official runner dependency install",
        runner_install_uses_legacy_peer_deps(spec.version_line),
    )?;
    if let Err(first_err) = verify_conformance_runner_startup_dependencies(spec, &node_modules) {
        reset_official_npm_node_modules(&install_root)?;
        ensure_official_npm_install(spec.version_line, baseline)?;
        run_npm_install_specs(
            &install_root,
            &runner_specs,
            "official runner dependency reinstall",
            runner_install_uses_legacy_peer_deps(spec.version_line),
        )?;
        verify_conformance_runner_startup_dependencies(spec, &node_modules).with_context(|| {
            format!(
                "{} runner dependencies still cannot start after reinstall; first failure: {first_err:#}",
                spec.name
            )
        })?;
    }

    let marker_body = serde_json::json!({
        "version_line": spec.version_line,
        "suite": spec.name,
        "packages": runner_specs,
        "rev": baseline.rev,
        "platform": npm_install_platform_marker(),
    });
    write_json(&marker, &marker_body)?;
    Ok(install_root)
}

fn prepare_runtime_smoke_root(
    version_line: VersionLine,
    baseline: &BaselineLock,
    vendor_dir: &Path,
) -> Result<PathBuf> {
    let install_root = ensure_official_npm_install(version_line, baseline)?;
    ensure_runtime_smoke_dependencies(version_line, vendor_dir, &install_root)?;
    let node_modules = install_root.join("node_modules");
    verify_runtime_smoke_root(version_line, &node_modules)?;
    Ok(node_modules)
}

fn ensure_runtime_smoke_dependencies(
    version_line: VersionLine,
    vendor_dir: &Path,
    install_root: &Path,
) -> Result<()> {
    let specs = runtime_smoke_dependency_specs(version_line, vendor_dir)?;
    let node_modules = install_root.join("node_modules");
    let marker = install_root.join("runtime-smoke-install.json");
    if node_modules.exists()
        && official_install_marker_matches(&marker, &specs)
        && node_dependency_available(&node_modules, "jsdom")
    {
        return Ok(());
    }

    run_npm_install_specs(
        install_root,
        &specs,
        "runtime smoke dependency install",
        false,
    )?;
    let marker_body = serde_json::json!({
        "version_line": version_line,
        "packages": specs,
        "platform": npm_install_platform_marker(),
    });
    write_json(&marker, &marker_body)?;
    Ok(())
}

fn runner_install_uses_legacy_peer_deps(version_line: VersionLine) -> bool {
    matches!(version_line, VersionLine::Vue3)
}

fn runtime_smoke_dependency_specs(
    version_line: VersionLine,
    vendor_dir: &Path,
) -> Result<Vec<String>> {
    let root = vendor_dir.join(version_line.as_str());
    let package_json = root.join("package.json");
    if !package_json.is_file() {
        anyhow::bail!(
            "{} is missing; run `cargo xtask sync-official-tests --locked` before preparing runtime smoke dependencies",
            package_json.display()
        );
    }
    let manifest = read_json::<serde_json::Value>(&package_json)?;
    let mut specs = Vec::new();
    for dependency in runtime_smoke_runner_dependencies() {
        let version = locked_runner_dependency_version(&root, dependency)
            .or_else(|| manifest_dependency_version(&manifest, dependency))
            .or_else(|| fallback_runner_dependency_version(vendor_dir, version_line, dependency));
        let Some(version) = version else {
            anyhow::bail!(
                "failed to resolve deterministic npm version for runtime smoke dependency {dependency} from {}",
                root.display()
            );
        };
        if is_unpublished_dependency_spec(&version) {
            anyhow::bail!(
                "runtime smoke dependency {dependency} resolved to unpublished spec {version:?}"
            );
        }
        specs.push(format!("{dependency}@{version}"));
    }
    specs.sort();
    specs.dedup();
    Ok(specs)
}

fn runtime_smoke_runner_dependencies() -> &'static [&'static str] {
    &["jsdom"]
}

fn verify_runtime_smoke_root(version_line: VersionLine, node_modules: &Path) -> Result<()> {
    for dependency in runtime_smoke_required_node_dependencies(version_line) {
        if !node_dependency_available(node_modules, dependency) {
            anyhow::bail!(
                "missing runtime smoke dependency {dependency} in {}",
                node_modules.display()
            );
        }
    }
    Ok(())
}

fn runtime_smoke_required_node_dependencies(version_line: VersionLine) -> &'static [&'static str] {
    match version_line {
        VersionLine::Vue26 | VersionLine::Vue27 => &["vue", "jsdom"],
        VersionLine::Vue3 => &["vue", "@vue/compiler-ssr", "@vue/server-renderer", "jsdom"],
    }
}

fn runner_dependency_specs(
    spec: ConformanceSuiteSpec,
    vendor_dir: &Path,
) -> Result<Option<Vec<String>>> {
    if spec.runner_dependencies.is_empty() {
        return Ok(Some(Vec::new()));
    }
    let root = vendor_dir.join(spec.version_line.as_str());
    let package_json = root.join("package.json");
    if !package_json.is_file() {
        return Ok(None);
    }
    let manifest = read_json::<serde_json::Value>(&package_json)?;
    let mut specs = Vec::new();
    for dependency in spec.runner_dependencies {
        let version = locked_runner_dependency_version(&root, dependency)
            .or_else(|| manifest_dependency_version(&manifest, dependency))
            .or_else(|| {
                fallback_runner_dependency_version(vendor_dir, spec.version_line, dependency)
            });
        let Some(version) = version else {
            return Ok(None);
        };
        if is_unpublished_dependency_spec(&version) {
            return Ok(None);
        }
        specs.push(format!("{dependency}@{version}"));
    }
    specs.sort();
    specs.dedup();
    Ok(Some(specs))
}

fn fallback_runner_dependency_version(
    vendor_dir: &Path,
    current: VersionLine,
    dependency: &str,
) -> Option<String> {
    [VersionLine::Vue26, VersionLine::Vue27, VersionLine::Vue3]
        .into_iter()
        .filter(|version_line| *version_line != current)
        .find_map(|version_line| {
            let root = vendor_dir.join(version_line.as_str());
            let manifest = read_json::<serde_json::Value>(&root.join("package.json")).ok();
            locked_runner_dependency_version(&root, dependency).or_else(|| {
                manifest
                    .as_ref()
                    .and_then(|manifest| manifest_dependency_version(manifest, dependency))
            })
        })
}

fn locked_runner_dependency_version(root: &Path, dependency: &str) -> Option<String> {
    let pnpm_lock = root.join("pnpm-lock.yaml");
    if pnpm_lock.is_file() {
        let lock = fs::read_to_string(pnpm_lock).ok()?;
        if let Some(version) = locked_pnpm_dependency_version(&lock, dependency) {
            return Some(version);
        }
    }
    let yarn_lock = root.join("yarn.lock");
    if yarn_lock.is_file() {
        let lock = fs::read_to_string(yarn_lock).ok()?;
        if let Some(version) = locked_yarn_dependency_version(&lock, dependency) {
            return Some(version);
        }
    }
    None
}

fn locked_pnpm_dependency_version(lock: &str, dependency: &str) -> Option<String> {
    for line in lock.lines() {
        let trimmed = line.trim_start().trim_start_matches(['\'', '"']);
        let candidate = trimmed
            .strip_prefix(&format!("{dependency}@"))
            .or_else(|| trimmed.strip_prefix(&format!("/{dependency}@")));
        let Some(candidate) = candidate else {
            continue;
        };
        let version_end = candidate
            .find(['(', ':', '\'', '"'])
            .unwrap_or(candidate.len());
        let version = candidate[..version_end].trim();
        if is_publishable_version(version) {
            return Some(version.to_string());
        }
    }
    None
}

fn locked_yarn_dependency_version(lock: &str, dependency: &str) -> Option<String> {
    let mut lines = lock.lines().peekable();
    while let Some(line) = lines.next() {
        if line.starts_with(char::is_whitespace)
            || !yarn_lock_key_matches_dependency(line, dependency)
        {
            continue;
        }
        while let Some(next) = lines.peek().copied() {
            if !next.starts_with("  ") {
                break;
            }
            let value = next.trim();
            if let Some(version) = value.strip_prefix("version ") {
                let version = version.trim_matches('"');
                if is_publishable_version(version) {
                    return Some(version.to_string());
                }
            }
            lines.next();
        }
    }
    None
}

fn yarn_lock_key_matches_dependency(line: &str, dependency: &str) -> bool {
    let key = line.trim().trim_end_matches(':');
    key.split(',').any(|part| {
        let part = part.trim().trim_matches('"');
        yarn_lock_package_name(part).is_some_and(|name| name == dependency)
    })
}

fn yarn_lock_package_name(spec: &str) -> Option<&str> {
    if spec.starts_with('@') {
        let slash = spec.find('/')?;
        let after_scope = &spec[slash + 1..];
        let at = after_scope.find('@')?;
        return Some(&spec[..slash + 1 + at]);
    }
    let at = spec.find('@')?;
    Some(&spec[..at])
}

fn manifest_dependency_version(manifest: &serde_json::Value, dependency: &str) -> Option<String> {
    ["dependencies", "devDependencies", "peerDependencies"]
        .into_iter()
        .find_map(|section| {
            manifest
                .get(section)
                .and_then(|value| value.get(dependency))
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned)
        })
}

fn is_unpublished_dependency_spec(version: &str) -> bool {
    let version = version.trim();
    version.is_empty()
        || version == "catalog:"
        || version.starts_with("workspace:")
        || version == "link:"
        || version.starts_with("file:")
}

fn is_publishable_version(version: &str) -> bool {
    let first = version.chars().next();
    first.is_some_and(|ch| ch.is_ascii_digit())
}

fn probe_api_exports(root: &Path, package_name: &str, request: &str) -> Result<ApiProbeOutput> {
    let root = absolute_path(root);
    let node = resolve_program("node");
    let output = Command::new(node)
        .arg("-e")
        .arg(API_PROBE_SCRIPT)
        .env("VUEC_API_PROBE_ROOT", &root)
        .env("VUEC_API_PROBE_PACKAGE", package_name)
        .env("VUEC_API_PROBE_REQUEST", request)
        .output()
        .with_context(|| format!("failed to spawn node API probe for {request}"))?;
    if !output.status.success() {
        anyhow::bail!(
            "node API probe failed for {} with status {:?}\nstdout:\n{}\nstderr:\n{}",
            request,
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).trim(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(stdout.trim())
        .with_context(|| format!("failed to parse node API probe output for {request}"))
}

fn run_alias_smoke(target: TargetSpec, root: &Path) -> Result<String> {
    let root = absolute_path(root);
    let request = api_require_request(target);
    let script = alias_smoke_script(target);
    let node = resolve_program("node");
    let output = Command::new(node)
        .arg("-e")
        .arg(script)
        .env("VUEC_ALIAS_ROOT", &root)
        .env("VUEC_ALIAS_REQUEST", &request)
        .output()
        .with_context(|| format!("failed to spawn npm alias smoke for {request}"))?;
    if !output.status.success() {
        anyhow::bail!(
            "node alias smoke failed for {} with status {:?}\nstdout:\n{}\nstderr:\n{}",
            request,
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).trim(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.trim().to_string())
}

fn run_output_contract_probe(
    target: TargetSpec,
    official_root: &Path,
    rust_root: &Path,
) -> Result<serde_json::Value> {
    let official_root = absolute_path(official_root);
    let rust_root = absolute_path(rust_root);
    let request = api_require_request(target);
    let fixture = output_contract_fixture(target);
    let node = resolve_program("node");
    let output = Command::new(node)
        .arg("-e")
        .arg(OUTPUT_CONTRACT_PROBE_SCRIPT)
        .env("VUEC_OUTPUT_OFFICIAL_ROOT", &official_root)
        .env("VUEC_OUTPUT_RUST_ROOT", &rust_root)
        .env("VUEC_OUTPUT_REQUEST", &request)
        .env("VUEC_OUTPUT_KIND", output_contract_kind(target))
        .env("VUEC_OUTPUT_VERSION_LINE", target.version_line.as_str())
        .env("VUEC_OUTPUT_ENTRY", target.entry)
        .env("VUEC_OUTPUT_FIXTURE", fixture)
        .output()
        .with_context(|| format!("failed to spawn output contract probe for {request}"))?;
    if !output.status.success() {
        anyhow::bail!(
            "node output contract probe failed for {} with status {:?}\nstdout:\n{}\nstderr:\n{}",
            request,
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).trim(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(stdout.trim())
        .with_context(|| format!("failed to parse output contract probe for {request}"))
}

struct OptionProbeRequest<'a> {
    side: &'a str,
    target: TargetSpec,
    root: &'a Path,
    request: &'a str,
    method: &'a str,
    fixture_source: &'a str,
    fixture_id: &'a str,
    option_name: &'a str,
    option_path: &'a str,
    input_kind: &'a str,
    option_value: Option<&'a serde_json::Value>,
}

fn run_option_probe(probe: OptionProbeRequest<'_>) -> Result<OptionProbeOutput> {
    let OptionProbeRequest {
        side,
        target,
        root,
        request,
        method,
        fixture_source,
        fixture_id,
        option_name,
        option_path,
        input_kind,
        option_value,
    } = probe;
    let root = absolute_path(root);
    let node = resolve_program("node");
    let payload = serde_json::json!({
        "request": request,
        "method": method,
        "source": fixture_source,
        "fixture_id": fixture_id,
        "option_name": option_name,
        "option_path": option_path,
        "input_kind": input_kind,
        "option_value": option_value,
        "target_version_line": target.version_line.as_str(),
        "target_package": target.package,
        "target_entry": target.entry,
    });
    let output = Command::new(node)
        .arg("-e")
        .arg(OPTION_MATRIX_PROBE_SCRIPT)
        .env("VUEC_OPTION_ROOT", &root)
        .env("VUEC_OPTION_SIDE", side)
        .env("VUEC_OPTION_PAYLOAD", serde_json::to_string(&payload)?)
        .output()
        .with_context(|| format!("failed to spawn option matrix probe for {request}"))?;
    if !output.status.success() {
        anyhow::bail!(
            "node option matrix probe failed for {} with status {:?}\nstdout:\n{}\nstderr:\n{}",
            request,
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).trim(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(stdout.trim())
        .with_context(|| format!("failed to parse option matrix probe for {request}"))
}

fn output_contract_kind(target: TargetSpec) -> &'static str {
    match target.kind {
        TargetKind::Vue26Template | TargetKind::Vue27Template => "vue2-template",
        TargetKind::Vue27Sfc | TargetKind::Vue3Sfc => "sfc",
        TargetKind::Vue3Core => "vue3-core",
        TargetKind::Vue3Dom => "vue3-dom",
        TargetKind::Vue3Ssr => "vue3-ssr",
    }
}

fn output_contract_fixture(target: TargetSpec) -> &'static str {
    match target.kind {
        TargetKind::Vue26Template | TargetKind::Vue27Template => {
            "<div id=\"app\"><span>{{ msg }}</span></div>"
        }
        TargetKind::Vue27Sfc | TargetKind::Vue3Sfc => {
            "<template><div class=\"a\">{{ msg }}</div></template><script>export default { props: ['msg'] }</script><style scoped>.a{ color: v-bind(color); }</style>"
        }
        TargetKind::Vue3Core | TargetKind::Vue3Dom | TargetKind::Vue3Ssr => {
            "<div class=\"a\"><span>{{ msg }}</span></div>"
        }
    }
}

fn json_usize(value: &serde_json::Value, path: &[&str]) -> usize {
    let mut cursor = value;
    for key in path {
        let Some(next) = cursor.get(*key) else {
            return 0;
        };
        cursor = next;
    }
    cursor.as_u64().unwrap_or_default() as usize
}

fn output_contract_counts_from_items(items: &[ReportItem]) -> serde_json::Value {
    serde_json::json!({
        "total": items.len(),
        "pass": items.iter().filter(|item| item.status == ReportStatus::Pass).count(),
        "pending": items.iter().filter(|item| item.status == ReportStatus::Pending).count(),
        "fail": items.iter().filter(|item| item.status == ReportStatus::Fail).count(),
    })
}

fn vue2_project_corpus_targets(compiler_version_line: VersionLine) -> Vec<TargetSpec> {
    all_targets()
        .iter()
        .copied()
        .filter(|target| target.version_line == compiler_version_line)
        .collect()
}

fn read_json_or_toml<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let text =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("toml"))
    {
        toml::from_str(&text).with_context(|| format!("failed to parse {}", path.display()))
    } else {
        serde_json::from_str(&text).with_context(|| format!("failed to parse {}", path.display()))
    }
}

fn sanitize_path_segment(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn package_dependency_spec(package: &serde_json::Value, name: &str) -> Option<String> {
    package_dependency_specs(package, name).into_iter().next()
}

fn package_dependency_specs(package: &serde_json::Value, name: &str) -> Vec<String> {
    [
        "dependencies",
        "devDependencies",
        "peerDependencies",
        "optionalDependencies",
    ]
    .into_iter()
    .filter_map(|section| {
        package
            .get(section)
            .and_then(|value| value.get(name))
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned)
    })
    .collect()
}

fn format_dependency_specs(specs: &[String]) -> Option<String> {
    if specs.is_empty() {
        None
    } else {
        Some(specs.join("; "))
    }
}

fn any_vue_dependency_for_version(specs: &[String], version_line: VersionLine) -> bool {
    specs
        .iter()
        .any(|spec| vue_dependency_spec_supports_version(spec, version_line))
}

fn vue_dependency_spec_supports_version(spec: &str, version_line: VersionLine) -> bool {
    let minor = match version_line {
        VersionLine::Vue26 => "2.6",
        VersionLine::Vue27 => "2.7",
        VersionLine::Vue3 => return false,
    };
    let cleaned = clean_vue_dependency_spec(spec);
    let mut simple = cleaned.as_str();
    for prefix in ["^", "~", ">=", "="] {
        if let Some(stripped) = simple.strip_prefix(prefix) {
            simple = stripped.trim();
            break;
        }
    }
    simple == minor
        || simple.starts_with(&format!("{minor}."))
        || simple.starts_with(&format!("{minor} "))
        || (version_line == VersionLine::Vue26 && vue26_semver_range_supports_vue26(&cleaned))
}

fn vue26_semver_range_supports_vue26(spec: &str) -> bool {
    let target =
        nodejs_semver::Version::parse("2.6.14").expect("hard-coded Vue 2.6 target must parse");
    nodejs_semver::Range::parse(spec).is_ok_and(|range| range.satisfies(&target))
}

fn clean_vue_dependency_spec(spec: &str) -> String {
    let mut cleaned = spec.trim();
    if let Some(stripped) = cleaned.strip_prefix("workspace:") {
        cleaned = stripped.trim();
    }
    if let Some(stripped) = cleaned
        .strip_prefix("npm:vue@")
        .or_else(|| cleaned.strip_prefix("vue@"))
    {
        cleaned = stripped.trim();
    }
    cleaned.to_string()
}

#[derive(Clone, Debug)]
struct Vue2ProjectScan {
    total: usize,
    files: Vec<PathBuf>,
}

fn scan_project_vue_files(root: &Path, project: &Vue2ProjectSpec) -> Vue2ProjectScan {
    let mut files = Vec::new();
    let mut total = 0;
    scan_project_vue_files_recursive(root, root, project, &mut files, &mut total);
    files.sort();
    Vue2ProjectScan { total, files }
}

fn scan_project_vue_files_recursive(
    root: &Path,
    dir: &Path,
    project: &Vue2ProjectSpec,
    files: &mut Vec<PathBuf>,
    total: &mut usize,
) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if file_name == ".git" || file_name == "node_modules" || file_name == "dist" {
            continue;
        }
        if path.is_dir() {
            scan_project_vue_files_recursive(root, &path, project, files, total);
        } else if path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("vue"))
        {
            *total += 1;
            let relative = relative_slash_path(root, &path);
            if project_path_selected(project, &relative) {
                files.push(path);
            }
        }
    }
}

fn project_path_selected(project: &Vue2ProjectSpec, path: &str) -> bool {
    let included = project
        .include
        .as_ref()
        .is_none_or(|patterns| patterns.iter().any(|pattern| wildcard_match(pattern, path)));
    let excluded = project
        .exclude
        .as_ref()
        .is_some_and(|patterns| patterns.iter().any(|pattern| wildcard_match(pattern, path)));
    included && !excluded
}

fn wildcard_match(pattern: &str, value: &str) -> bool {
    let pattern = pattern.replace('\\', "/");
    wildcard_match_bytes(pattern.as_bytes(), value.as_bytes())
}

fn wildcard_match_bytes(pattern: &[u8], value: &[u8]) -> bool {
    if pattern.is_empty() {
        return value.is_empty();
    }
    if pattern[0] == b'*' {
        return wildcard_match_bytes(&pattern[1..], value)
            || (!value.is_empty() && wildcard_match_bytes(pattern, &value[1..]));
    }
    !value.is_empty() && pattern[0] == value[0] && wildcard_match_bytes(&pattern[1..], &value[1..])
}

fn relative_slash_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn limit_vue_files(files: Vec<PathBuf>, max_files: usize) -> Vec<PathBuf> {
    if max_files == 0 || files.len() <= max_files {
        return files;
    }
    files.into_iter().take(max_files).collect()
}

fn run_vue2_project_probe(
    official_root: &Path,
    rust_root: &Path,
    project_root: &Path,
    files: &[PathBuf],
) -> Result<Vue2ProjectProbeReport> {
    let official_root = absolute_path(official_root);
    let rust_root = absolute_path(rust_root);
    let project_root = absolute_path(project_root);
    let relative_files = files
        .iter()
        .map(|path| relative_slash_path(&project_root, path))
        .collect::<Vec<_>>();
    let output = Command::new(resolve_program("node"))
        .arg("-e")
        .arg(VUE2_PROJECT_CORPUS_PROBE_SCRIPT)
        .env("VUEC_PROJECT_OFFICIAL_ROOT", &official_root)
        .env("VUEC_PROJECT_RUST_ROOT", &rust_root)
        .env("VUEC_PROJECT_ROOT", &project_root)
        .env(
            "NODE_PATH",
            conformance_node_path(&rust_root, &official_root),
        )
        .env(
            "VUEC_PROJECT_FILES",
            serde_json::to_string(&relative_files)?,
        )
        .output()
        .with_context(|| "failed to spawn Vue 2 project corpus probe")?;
    if !output.status.success() {
        anyhow::bail!(
            "Vue 2 project corpus probe failed with status {:?}\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).trim(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    serde_json::from_slice(&output.stdout).context("failed to parse Vue 2 project probe output")
}

pub fn verify_vue27_project_corpus(args: &Vue27ProjectCorpusArgs) -> JsonReport {
    let generic = Vue2ProjectCorpusArgs {
        manifest: args.manifest.clone(),
        out_dir: args.out_dir.clone(),
        project_vue_version: VersionLine::Vue27,
        compiler_version_line: VersionLine::Vue27,
        project: args.project.clone(),
        max_files_per_project: args.max_files_per_project,
    };
    verify_vue2_project_corpus_with_command(&generic, "verify_vue27_project_corpus")
}

pub fn verify_vue2_project_corpus(args: &Vue2ProjectCorpusArgs) -> JsonReport {
    verify_vue2_project_corpus_with_command(args, "verify_vue2_project_corpus")
}

fn verify_vue2_project_corpus_with_command(
    args: &Vue2ProjectCorpusArgs,
    command_name: &str,
) -> JsonReport {
    let manifest_path = absolute_path(&args.manifest);
    let out_dir = absolute_path(&args.out_dir);
    let mut items = Vec::new();
    let mut violations = Vec::new();
    let mut created = Vec::new();
    let mut project_reports = Vec::new();

    if !matches!(
        args.project_vue_version,
        VersionLine::Vue26 | VersionLine::Vue27
    ) {
        return JsonReport::new(command_name, ReportStatus::Fail).with_violations(vec![format!(
            "project Vue version must be vue2_6 or vue2_7; got {}",
            args.project_vue_version.as_str()
        )]);
    }
    if args.compiler_version_line != VersionLine::Vue27 {
        return JsonReport::new(command_name, ReportStatus::Fail).with_violations(vec![format!(
            "Vue 2 project corpus comparison uses the official Vue 2.7 compiler package boundary; got compiler_version_line={}",
            args.compiler_version_line.as_str()
        )]);
    }

    let manifest = match read_json_or_toml::<Vue2ProjectCorpusManifest>(&manifest_path) {
        Ok(manifest) => manifest,
        Err(err) => {
            return JsonReport::new(command_name, ReportStatus::Fail).with_violations(vec![
                format!(
                    "failed to read Vue 2 project corpus manifest {}: {err:#}",
                    manifest_path.display()
                ),
            ]);
        }
    };

    if manifest.schema_version != 1 {
        violations.push(format!(
            "unsupported Vue 2 project corpus manifest schema {}; expected 1",
            manifest.schema_version
        ));
    }

    let selected_projects = manifest
        .projects
        .iter()
        .filter(|project| {
            args.project
                .as_ref()
                .is_none_or(|name| project.name == *name)
        })
        .collect::<Vec<_>>();
    let required_projects = if args.project.is_some() {
        1
    } else {
        manifest.min_projects.unwrap_or(15)
    };
    if selected_projects.len() < required_projects {
        violations.push(format!(
            "Vue 2 project corpus selected {} projects; expected at least {required_projects}",
            selected_projects.len()
        ));
    }

    let targets = vue2_project_corpus_targets(args.compiler_version_line);
    let lock_path = PathBuf::from("compat/official-revisions.lock");
    let lock_hash = file_sha256(&lock_path).ok();
    let lock = match load_official_lock(&lock_path) {
        Ok(lock) => lock,
        Err(err) => {
            return JsonReport::new(command_name, ReportStatus::Fail)
                .with_violations(vec![format!("failed to load official lock: {err:#}")]);
        }
    };
    let metadata = ReportMetadata::capture().with_lock_context(lock_hash.clone(), Some(&lock));
    let Some(baseline) = baseline_for(&lock, args.compiler_version_line) else {
        return JsonReport::new(command_name, ReportStatus::Fail).with_violations(vec![format!(
            "official {} baseline is missing",
            args.compiler_version_line.as_str()
        )]);
    };
    let official_root = match ensure_official_npm_install(args.compiler_version_line, baseline) {
        Ok(root) => root,
        Err(err) => {
            return JsonReport::new(command_name, ReportStatus::Fail).with_violations(vec![
                format!(
                    "failed to prepare official {} packages: {err:#}",
                    args.compiler_version_line.as_str()
                ),
            ]);
        }
    };
    match prepare_alias_backend(AliasBackend::Generated, &targets) {
        Ok(paths) => created.extend(paths.into_iter().map(|path| path.display().to_string())),
        Err(err) => {
            return JsonReport::new(command_name, ReportStatus::Fail).with_violations(vec![
                format!(
                    "failed to prepare generated {} alias packages: {err:#}",
                    args.compiler_version_line.as_str()
                ),
            ]);
        }
    }
    let rust_root = rust_alias_root(args.compiler_version_line);

    let checkout_root = out_dir.join("checkouts");
    let report_root = out_dir.join("reports");
    if let Err(err) = fs::create_dir_all(&checkout_root) {
        violations.push(format!(
            "failed to create checkout root {}: {err}",
            checkout_root.display()
        ));
    }
    if let Err(err) = fs::create_dir_all(&report_root) {
        violations.push(format!(
            "failed to create report root {}: {err}",
            report_root.display()
        ));
    }

    for project in selected_projects {
        let package_json_rel = project
            .package_json
            .as_deref()
            .unwrap_or("package.json")
            .replace('\\', "/");
        let checkout = checkout_root.join(sanitize_path_segment(&project.name));
        let mut status = ReportStatus::Pass;
        let detail: Option<String>;
        let mut report_path = None;
        let mut vue_files = 0usize;
        let mut selected_vue_files = 0usize;
        let mut vue_dependency = None;
        let mut vue_template_compiler_dependency = None;

        if let Err(err) = sync_git_checkout(
            &project.repo,
            &project.rev,
            &checkout,
            project.submodules.unwrap_or(true),
        ) {
            status = ReportStatus::Fail;
            detail = Some(format!("checkout failed: {err:#}"));
            violations.push(format!("{} checkout failed: {err:#}", project.name));
        } else {
            let package_json = checkout.join(&package_json_rel);
            match read_json::<serde_json::Value>(&package_json) {
                Ok(package) => {
                    let vue_dependencies = package_dependency_specs(&package, "vue");
                    vue_dependency = format_dependency_specs(&vue_dependencies);
                    vue_template_compiler_dependency =
                        package_dependency_spec(&package, "vue-template-compiler");
                    if !any_vue_dependency_for_version(&vue_dependencies, args.project_vue_version)
                    {
                        status = ReportStatus::Fail;
                        detail = Some(format!(
                            "package {} does not declare support for {}; found {:?}",
                            package_json_rel,
                            args.project_vue_version.as_str(),
                            vue_dependency
                        ));
                        violations.push(format!(
                            "{} does not declare support for {}",
                            project.name,
                            args.project_vue_version.as_str()
                        ));
                    } else {
                        let discovered = scan_project_vue_files(&checkout, project);
                        vue_files = discovered.total;
                        let max_files = project.max_vue_files.unwrap_or(args.max_files_per_project);
                        let selected = limit_vue_files(discovered.files, max_files);
                        selected_vue_files = selected.len();
                        let min_vue_files = project
                            .min_vue_files
                            .or(manifest.min_vue_files_per_project)
                            .unwrap_or(20);
                        if selected_vue_files < min_vue_files {
                            status = ReportStatus::Fail;
                            detail = Some(format!(
                                "selected {selected_vue_files} Vue SFC files; expected at least {min_vue_files}"
                            ));
                            violations.push(format!(
                                "{} selected too few Vue SFC files: {selected_vue_files}/{min_vue_files}",
                                project.name
                            ));
                        } else {
                            match run_vue2_project_probe(
                                &official_root,
                                &rust_root,
                                &checkout,
                                &selected,
                            ) {
                                Ok(probe) => {
                                    let path = report_root.join(format!(
                                        "{}.json",
                                        sanitize_path_segment(&project.name)
                                    ));
                                    if let Err(err) = write_json(&path, &probe) {
                                        status = ReportStatus::Fail;
                                        detail = Some(format!(
                                            "failed to write project report {}: {err}",
                                            path.display()
                                        ));
                                        violations.push(format!(
                                            "{} report write failed: {err}",
                                            project.name
                                        ));
                                    } else {
                                        created.push(path.display().to_string());
                                        report_path = Some(path.display().to_string());
                                        if probe.counts.fail > 0 {
                                            status = ReportStatus::Fail;
                                            detail = Some(format!(
                                                "{} template modes passed, {} failed across {} template files",
                                                probe.counts.pass,
                                                probe.counts.fail,
                                                probe.counts.template_files
                                            ));
                                            violations.push(format!(
                                                "{} has {} compiler output mismatches",
                                                project.name, probe.counts.fail
                                            ));
                                        } else {
                                            detail = Some(format!(
                                                "{} template modes passed across {} template files",
                                                probe.counts.pass, probe.counts.template_files
                                            ));
                                        }
                                    }
                                }
                                Err(err) => {
                                    status = ReportStatus::Fail;
                                    detail = Some(format!("project probe failed: {err:#}"));
                                    violations.push(format!(
                                        "{} project probe failed: {err:#}",
                                        project.name
                                    ));
                                }
                            }
                        }
                    }
                }
                Err(err) => {
                    status = ReportStatus::Fail;
                    detail = Some(format!(
                        "failed to read package manifest {package_json_rel}: {err:#}"
                    ));
                    violations.push(format!(
                        "{} package manifest is missing/invalid: {err:#}",
                        project.name
                    ));
                }
            }
        }

        let detail = detail.unwrap_or_else(|| "project validation did not run".into());
        items.push(ReportItem::new(
            project.name.clone(),
            status,
            detail.clone(),
            Some(checkout.clone()),
        ));
        project_reports.push(Vue2ProjectCorpusProjectReport {
            name: project.name.clone(),
            repo: project.repo.clone(),
            rev: project.rev.clone(),
            package_json: package_json_rel,
            checkout: checkout.display().to_string(),
            project_vue_version: args.project_vue_version.as_str().into(),
            compiler_version_line: args.compiler_version_line.as_str().into(),
            vue_files,
            selected_vue_files,
            vue_dependency,
            vue_template_compiler_dependency,
            status: status.as_str().into(),
            detail,
            report: report_path,
        });
    }

    let aggregate_path = out_dir.join(format!("{command_name}.json"));
    let aggregate = serde_json::json!({
        "command": command_name,
        "metadata": metadata,
        "manifest": manifest_path,
        "project_vue_version": args.project_vue_version.as_str(),
        "compiler_version_line": args.compiler_version_line.as_str(),
        "official_root": official_root,
        "rust_root": rust_root,
        "projects": project_reports,
        "counts": output_contract_counts_from_items(&items),
    });
    if let Some(parent) = aggregate_path.parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            violations.push(format!("failed to create {}: {err}", parent.display()));
        }
    }
    if let Err(err) = write_json(&aggregate_path, &aggregate) {
        violations.push(format!(
            "failed to write aggregate project corpus report {}: {err}",
            aggregate_path.display()
        ));
    } else {
        created.push(aggregate_path.display().to_string());
    }

    let mut report = JsonReport::new(command_name, ReportStatus::Pending);
    report.metadata = metadata;
    report
        .with_items(items)
        .with_violations(violations)
        .with_created(created)
        .with_note(format!(
            "verifies fixed external {} project SFC templates by comparing official {} vue-template-compiler / vue/compiler-sfc output with generated Rust aliases",
            args.project_vue_version.as_str(),
            args.compiler_version_line.as_str()
        ))
}

fn compare_option_probe(
    row: &OptionMatrixRow,
    official: &OptionProbeOutput,
    rust: &OptionProbeOutput,
) -> bool {
    if official.ok != rust.ok {
        return false;
    }
    if !official.ok {
        return official.error == rust.error;
    }
    let official_value = official.value.as_ref().unwrap_or(&serde_json::Value::Null);
    let rust_value = rust.value.as_ref().unwrap_or(&serde_json::Value::Null);
    for field in &row.output_fields_affected {
        if let Some(expected) = field.strip_prefix("code:contains:") {
            let official_code = json_path(official_value, "code").and_then(|value| value.as_str());
            let rust_code = json_path(rust_value, "code").and_then(|value| value.as_str());
            if official_code.map(|code| code.contains(expected)) != Some(true)
                || rust_code.map(|code| code.contains(expected)) != Some(true)
            {
                return false;
            }
            continue;
        }
        let official_field = json_path(official_value, field);
        let rust_field =
            json_path(rust_value, field).or_else(|| rust_alias_field(rust_value, field));
        if official_field != rust_field {
            return false;
        }
    }
    true
}

fn json_path<'a>(value: &'a serde_json::Value, path: &str) -> Option<&'a serde_json::Value> {
    let mut cursor = value;
    for segment in path.split('.') {
        if let Ok(index) = segment.parse::<usize>() {
            cursor = cursor.get(index)?;
        } else {
            cursor = cursor.get(segment)?;
        }
    }
    Some(cursor)
}

fn rust_alias_field<'a>(
    value: &'a serde_json::Value,
    field: &str,
) -> Option<&'a serde_json::Value> {
    match field {
        "staticRenderFns" => value.get("static_render_fns"),
        "template" | "script" | "styles" | "customBlocks" => value
            .get("descriptor")
            .and_then(|descriptor| descriptor.get(field)),
        "descriptor.scriptSetup.content" => value
            .get("descriptor")
            .and_then(|descriptor| descriptor.get("script_setup"))
            .and_then(|script_setup| script_setup.get("content")),
        "descriptor.styles.0.scoped" => value
            .get("descriptor")
            .and_then(|descriptor| descriptor.get("styles"))
            .and_then(|styles| styles.get(0))
            .and_then(|style| style.get("attrs"))
            .and_then(|attrs| attrs.get("scoped")),
        "ast" => value
            .get("element_ast")
            .or_else(|| value.get("ast_summary")),
        _ => None,
    }
}

fn alias_smoke_script(target: TargetSpec) -> String {
    let call = match target.kind {
        TargetKind::Vue26Template | TargetKind::Vue27Template => {
            "const result = api.compile('<div>{{ msg }}</div>', { optimize: true }); assert(result && typeof result.render === 'string', 'compile render missing');"
        }
        TargetKind::Vue27Sfc => {
            "const result = api.parse({ source: '<template><div/></template><script>export default {}</script>', filename: 'smoke.vue' }); assert(result && result.template, 'parse descriptor missing template');"
        }
        TargetKind::Vue3Sfc => {
            "const result = api.parse('<template><div/></template><script>export default {}</script>'); assert(result && result.descriptor && result.descriptor.template, 'parse descriptor missing template');"
        }
        TargetKind::Vue3Core => {
            "const result = api.baseCompile('<div>{{ msg }}</div>', {}); assert(result && typeof result.code === 'string', 'baseCompile code missing');"
        }
        TargetKind::Vue3Dom => {
            "const result = api.compile('<input v-model=\"msg\">', {}); assert(result && typeof result.code === 'string', 'dom compile code missing');"
        }
        TargetKind::Vue3Ssr => {
            "const result = api.compile('<div>{{ msg }}</div>'); assert(result && typeof result.code === 'string', 'ssr compile code missing');"
        }
    };
    format!(
        r#"
const path = require('path');
const {{ createRequire }} = require('module');
const root = process.env.VUEC_ALIAS_ROOT;
const request = process.env.VUEC_ALIAS_REQUEST;
const rootRequire = createRequire(path.join(root, 'package.json'));
function assert(value, message) {{
  if (!value) {{
    throw new Error(message);
  }}
}}
const api = rootRequire(request);
assert(api && typeof api === 'object', 'API object missing');
{call}
process.stdout.write('pass ' + request);
"#
    )
}

fn absolute_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(path))
        .unwrap_or_else(|_| path.to_path_buf())
}

fn resolve_program(name: &str) -> String {
    if cfg!(windows) && !name.contains('.') {
        if let Some(path) = find_on_path(&format!("{name}.cmd")) {
            return path;
        }
        if let Some(path) = find_on_path(&format!("{name}.exe")) {
            return path;
        }
    }
    name.to_string()
}

fn find_on_path(name: &str) -> Option<String> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate.display().to_string());
        }
    }
    None
}

fn normalize_command_output(stdout: &[u8], stderr: &[u8]) -> String {
    let mut text = String::new();
    let stdout = String::from_utf8_lossy(stdout);
    let stdout = stdout.trim();
    if !stdout.is_empty() {
        text.push_str(stdout);
    }
    let stderr = String::from_utf8_lossy(stderr);
    let stderr = stderr.trim();
    if !stderr.is_empty() {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(stderr);
    }
    text.lines().take(40).collect::<Vec<_>>().join("\n")
}
