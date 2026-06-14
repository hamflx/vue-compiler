fn verify_release_dry_run(native_artifacts_dir: Option<&Path>) -> Result<compat::JsonReport> {
    let mut items = Vec::new();
    let mut violations = Vec::new();
    let mut created = Vec::new();
    let staging_root = PathBuf::from("target").join("release-dry-run");
    ensure_target_child(&staging_root, "release-dry-run")?;
    if staging_root.exists() {
        fs::remove_dir_all(&staging_root)
            .with_context(|| format!("failed to remove {}", staging_root.display()))?;
    }
    fs::create_dir_all(&staging_root)
        .with_context(|| format!("failed to create {}", staging_root.display()))?;
    created.push(staging_root.display().to_string());

    match verify_release_npm_pack_dry_runs(&staging_root, native_artifacts_dir) {
        Ok((mut npm_items, mut npm_created)) => {
            items.append(&mut npm_items);
            created.append(&mut npm_created);
        }
        Err(err) => {
            violations.push(format!("npm release dry-run setup failed: {err:#}"));
            items.push(compat::ReportItem::new(
                "npm-release-dry-run",
                compat::ReportStatus::Fail,
                format!("{err:#}"),
                Some(staging_root.join("npm")),
            ));
        }
    }

    match verify_release_cargo_dry_runs() {
        Ok(mut cargo_items) => items.append(&mut cargo_items),
        Err(err) => {
            violations.push(format!("cargo release dry-run setup failed: {err:#}"));
            items.push(compat::ReportItem::new(
                "cargo-release-dry-run",
                compat::ReportStatus::Fail,
                format!("{err:#}"),
                Some(PathBuf::from("Cargo.toml")),
            ));
        }
    }

    for item in &items {
        if item.status == compat::ReportStatus::Fail {
            violations.push(format!("{}: {}", item.target, item.detail));
        }
    }

    Ok(compat::JsonReport::new(
        "verify_release_dry_run",
        if violations.is_empty() {
            compat::ReportStatus::Pass
        } else {
            compat::ReportStatus::Fail
        },
    )
    .with_items(items)
    .with_created(created)
    .with_violations(violations)
    .with_note("runs real npm pack dry-runs from staged package directories, verifies staged package file lists, accepts release-built native artifacts through --native-artifacts-dir, runs cargo publish dry-run where crates.io can resolve dependencies, and marks first-release or missing cross-platform artifact constraints as pending instead of counting them as passed"))
}

fn verify_release_install_smoke(
    native_artifacts_dir: Option<&Path>,
    current_platform_only: bool,
) -> Result<compat::JsonReport> {
    let mut items = Vec::new();
    let mut violations = Vec::new();
    let mut created = Vec::new();
    let root = PathBuf::from("target").join("release-install-smoke");
    ensure_target_child(&root, "release-install-smoke")?;
    if root.exists() {
        fs::remove_dir_all(&root)
            .with_context(|| format!("failed to remove {}", root.display()))?;
    }
    fs::create_dir_all(&root).with_context(|| format!("failed to create {}", root.display()))?;
    created.push(root.display().to_string());

    let package_root = root.join("packages");
    fs::create_dir_all(&package_root)
        .with_context(|| format!("failed to create {}", package_root.display()))?;

    let mut package_items = Vec::new();
    let mut package_violations = Vec::new();
    let mut npm_created = Vec::new();
    let current_platform = current_platform_package_name();
    let package_result = prepare_release_install_packages(&package_root, native_artifacts_dir);
    match package_result {
        Ok(paths) => npm_created.extend(paths.into_iter().map(|path| path.display().to_string())),
        Err(err) => package_violations.push(format!("{err:#}")),
    }
    created.extend(npm_created);

    if package_violations.is_empty() {
        match run_native_release_install_smoke(&root, current_platform) {
            Ok(detail) => package_items.push(compat::ReportItem::new(
                "install-smoke:@vuec-rs/native",
                compat::ReportStatus::Pass,
                detail,
                Some(root.join("native-project")),
            )),
            Err(err) => package_items.push(compat::ReportItem::new(
                "install-smoke:@vuec-rs/native",
                compat::ReportStatus::Fail,
                format!("{err:#}"),
                Some(root.join("native-project")),
            )),
        }
        match run_wasm_release_install_smoke(&root) {
            Ok(detail) => package_items.push(compat::ReportItem::new(
                "install-smoke:@vuec-rs/wasm",
                compat::ReportStatus::Pass,
                detail,
                Some(root.join("wasm-project")),
            )),
            Err(err) => package_items.push(compat::ReportItem::new(
                "install-smoke:@vuec-rs/wasm",
                compat::ReportStatus::Fail,
                format!("{err:#}"),
                Some(root.join("wasm-project")),
            )),
        }
    } else {
        for violation in &package_violations {
            package_items.push(compat::ReportItem::new(
                "install-smoke:package-preparation",
                compat::ReportStatus::Fail,
                violation,
                Some(package_root.clone()),
            ));
        }
    }

    let installed_platform = current_platform.unwrap_or("unsupported-platform");
    if !current_platform_only {
        for package_dir in collect_native_platform_package_dirs()? {
            let package_name = read_package_display_name(&package_dir.join("package.json"))?;
            if package_name == installed_platform {
                continue;
            }
            let suffix = native_platform_suffix(&package_name)?;
            let detail = find_native_artifact(native_artifacts_dir, suffix)?
                .map(|artifact| {
                    format!(
                        "release artifact is available at {}, but executable install smoke still requires a matching target-platform host",
                        artifact.display()
                    )
                })
                .unwrap_or_else(|| {
                    "non-current platform package install smoke requires a matching target-platform release artifact and host run".into()
                });
            package_items.push(compat::ReportItem::new(
                format!("install-smoke:{package_name}"),
                compat::ReportStatus::Pending,
                detail,
                Some(package_dir),
            ));
        }
    }

    for item in &package_items {
        if item.status == compat::ReportStatus::Fail {
            violations.push(format!("{}: {}", item.target, item.detail));
        }
    }
    items.extend(package_items);

    Ok(compat::JsonReport::new(
        "verify_release_install_smoke",
        if violations.is_empty() {
            compat::ReportStatus::Pass
        } else {
            compat::ReportStatus::Fail
        },
    )
    .with_items(items)
    .with_created(created)
    .with_violations(violations)
    .with_note("packs release-built npm artifacts, installs them into clean projects, smoke-calls @vuec-rs/native through the current optional platform package and @vuec-rs/wasm through its published package entry, accepts the current platform native artifact through --native-artifacts-dir, and marks non-current platform install smoke as pending unless --current-platform-only is used for a matrix runner"))
}

fn verify_release_npm_pack_dry_runs(
    staging_root: &Path,
    native_artifacts_dir: Option<&Path>,
) -> Result<(Vec<compat::ReportItem>, Vec<String>)> {
    let mut items = Vec::new();
    let mut created = Vec::new();
    let npm_root = staging_root.join("npm");
    fs::create_dir_all(&npm_root)
        .with_context(|| format!("failed to create {}", npm_root.display()))?;

    let wasm_release_ready = match build_wasm_release_packages() {
        Ok(paths) => {
            created.extend(paths.into_iter().map(|path| path.display().to_string()));
            true
        }
        Err(err) => {
            items.push(compat::ReportItem::new(
                "npm:@vuec-rs/wasm-build",
                compat::ReportStatus::Fail,
                format!("failed to build wasm-bindgen release packages: {err:#}"),
                Some(PathBuf::from("crates/vuec_wasm")),
            ));
            false
        }
    };

    let native_stage = npm_root.join("native");
    stage_package_dir(Path::new("packages/native"), &native_stage)?;
    created.push(native_stage.display().to_string());
    items.push(run_npm_pack_check(
        "@vuec-rs/native",
        &native_stage,
        &["README.md", "index.d.ts", "index.js", "package.json"],
    ));

    if wasm_release_ready {
        let wasm_stage = npm_root.join("wasm");
        stage_package_dir(Path::new("packages/wasm"), &wasm_stage)?;
        created.push(wasm_stage.display().to_string());
        items.push(run_npm_pack_check(
            "@vuec-rs/wasm",
            &wasm_stage,
            &[
                "README.md",
                "index.d.ts",
                "index.js",
                "package.json",
                "pkg/vuec_wasm.js",
                "pkg/vuec_wasm_bg.wasm",
                "pkg-node/vuec_wasm.js",
                "pkg-node/vuec_wasm_bg.wasm",
            ],
        ));
    } else {
        items.push(compat::ReportItem::new(
            "npm:@vuec-rs/wasm",
            compat::ReportStatus::Fail,
            "release wasm-bindgen packages were not rebuilt, so npm pack dry-run was not run",
            Some(PathBuf::from("packages/wasm")),
        ));
    }

    let current_platform = current_platform_package_name();
    let mut current_release_binding: Option<std::result::Result<PathBuf, String>> = None;
    for package_dir in collect_native_platform_package_dirs()? {
        let manifest_path = package_dir.join("package.json");
        let package_name = read_package_display_name(&manifest_path)?;
        let suffix = native_platform_suffix(&package_name)?;
        let binding_source = match find_native_artifact(native_artifacts_dir, suffix)? {
            Some(path) => Some(Ok((path, "external artifact"))),
            None if Some(package_name.as_str()) == current_platform => {
                let source = current_release_binding
                    .get_or_insert_with(|| {
                        build_napi_crate_release()
                            .map(|()| napi_release_library_path())
                            .map_err(|err| format!("{err:#}"))
                    })
                    .clone();
                Some(source.map(|path| (path, "current cargo release build")))
            }
            None => None,
        };

        let Some(binding_source) = binding_source else {
            items.push(compat::ReportItem::new(
                format!("npm:{package_name}"),
                compat::ReportStatus::Pending,
                "non-current platform package requires its own release-build vuec_napi.node artifact before npm pack dry-run can prove publishability; pass --native-artifacts-dir with <platform>/vuec_napi.node or <platform>.node to verify it",
                Some(package_dir),
            ));
            continue;
        };

        let (binding_path, binding_source_label) = match binding_source {
            Ok(source) => source,
            Err(err) => {
                items.push(compat::ReportItem::new(
                    format!("npm:{package_name}"),
                    compat::ReportStatus::Fail,
                    format!(
                        "failed to build current platform release NAPI binding and no external artifact was provided: {err}"
                    ),
                    Some(package_dir),
                ));
                continue;
            }
        };
        let stage_dir = npm_root.join("native-platforms").join(suffix);
        match stage_release_native_platform_package(&package_dir, &stage_dir, &binding_path) {
            Ok(path) => {
                created.push(stage_dir.display().to_string());
                created.push(path.display().to_string());
                let mut item = run_npm_pack_check(
                    &package_name,
                    &stage_dir,
                    &["README.md", "package.json", "vuec_napi.node"],
                );
                if item.status == compat::ReportStatus::Pass {
                    item.detail = format!(
                        "{}; staged vuec_napi.node from {} ({})",
                        item.detail,
                        binding_source_label,
                        native_binding_fingerprint(&binding_path)
                    );
                }
                items.push(item);
            }
            Err(err) => items.push(compat::ReportItem::new(
                format!("npm:{package_name}"),
                compat::ReportStatus::Fail,
                format!("failed to stage release NAPI binding: {err:#}"),
                Some(package_dir),
            )),
        }
    }

    Ok((items, created))
}

fn prepare_release_install_packages(
    package_root: &Path,
    native_artifacts_dir: Option<&Path>,
) -> Result<Vec<PathBuf>> {
    let mut created = Vec::new();
    fs::create_dir_all(package_root)
        .with_context(|| format!("failed to create {}", package_root.display()))?;

    build_wasm_release_packages()?;

    let native_stage = package_root.join("native");
    stage_package_dir(Path::new("packages/native"), &native_stage)?;
    created.push(native_stage.clone());

    let wasm_stage = package_root.join("wasm");
    stage_package_dir(Path::new("packages/wasm"), &wasm_stage)?;
    created.push(wasm_stage.clone());

    let platform_name = current_platform_package_name().with_context(|| {
        format!(
            "unsupported NAPI platform package for os={} arch={}",
            std::env::consts::OS,
            std::env::consts::ARCH
        )
    })?;
    let platform_suffix = native_platform_suffix(platform_name)?;
    let platform_artifact = find_native_artifact(native_artifacts_dir, platform_suffix)?;
    let binding_source = if let Some(artifact) = platform_artifact {
        artifact
    } else {
        build_napi_crate_release()?;
        napi_release_library_path()
    };
    let platform_stage = package_root.join("native-platforms").join(platform_suffix);
    stage_release_native_platform_package(
        &platform_template_dir(platform_name)?,
        &platform_stage,
        &binding_source,
    )?;
    created.push(platform_stage.clone());

    for stage in [&native_stage, &wasm_stage, &platform_stage] {
        let tarball = npm_pack(stage)?;
        created.push(tarball);
    }
    Ok(created)
}

fn run_native_release_install_smoke(
    root: &Path,
    current_platform: Option<&'static str>,
) -> Result<String> {
    let platform_name = current_platform.context("current platform package is unsupported")?;
    let project = root.join("native-project");
    fs::create_dir_all(&project)
        .with_context(|| format!("failed to create {}", project.display()))?;
    write_clean_npm_project(&project, "vuec-release-native-install-smoke", None)?;
    let native_tarball = find_single_tgz(&root.join("packages").join("native"))?;
    let platform_tarball = find_single_tgz(
        &root
            .join("packages")
            .join("native-platforms")
            .join(native_platform_suffix(platform_name)?),
    )?;
    npm_install_tarballs(&project, &[&platform_tarball, &native_tarball])?;
    let script = r#"
const assert = require('node:assert/strict');
const native = require('@vuec-rs/native');
assert.equal(typeof native.version(), 'string');
const info = native.bindingInfo();
assert.equal(info.source, 'platform');
assert.ok(info.package);
const vue2 = native.compile('<div>{{ msg }}</div>');
assert.match(vue2.render, /_s\(msg\)/);
const dom = native.compileDom('<div>{{ msg }}</div>', { mode: 'module', prefixIdentifiers: true, sourceMap: true });
assert.match(dom.code, /export function render/);
assert.match(dom.code, /_toDisplayString\(_ctx\.msg\)/);
assert.equal(dom.map.version, 3);
const ssr = native.compileSsr('<div>{{ msg }}</div>', { mode: 'module', prefixIdentifiers: true });
assert.match(ssr.code, /export function ssrRender/);
const descriptor = native.parse('<template><p/></template>', { filename: 'smoke.vue' });
assert.equal(descriptor.filename, 'smoke.vue');
const style = native.compileStyle({ source: '.a{ color: v-bind(color); }', id: 'data-v-smoke', scoped: true });
assert.match(style.code, /data-v-smoke/);
process.stdout.write(JSON.stringify({ status: 'pass', binding: info, exports: Object.keys(native).sort() }));
"#;
    run_node_script(&project, script, NodeScriptMode::CommonJs)
}

fn run_wasm_release_install_smoke(root: &Path) -> Result<String> {
    let project = root.join("wasm-project");
    fs::create_dir_all(&project)
        .with_context(|| format!("failed to create {}", project.display()))?;
    write_clean_npm_project(&project, "vuec-release-wasm-install-smoke", Some("module"))?;
    let wasm_tarball = find_single_tgz(&root.join("packages").join("wasm"))?;
    npm_install_tarballs(&project, &[&wasm_tarball])?;
    let script = r#"
import assert from 'node:assert/strict';
import { init } from '@vuec-rs/wasm';
const wasm = await init();
assert.equal(typeof wasm.version(), 'string');
const vue2 = wasm.compile('<div>{{ msg }}</div>');
assert.match(vue2.render, /_s\(msg\)/);
const dom = wasm.compileDom('<div>{{ msg }}</div>', { mode: 'module', prefixIdentifiers: true, sourceMap: true });
assert.match(dom.code, /export function render/);
assert.match(dom.code, /_toDisplayString\(_ctx\.msg\)/);
assert.equal(dom.map.version, 3);
const ssr = wasm.compileSsr('<div>{{ msg }}</div>', { mode: 'module', prefixIdentifiers: true });
assert.match(ssr.code, /export function ssrRender/);
const descriptor = wasm.parse('<template><p/></template>', { filename: 'smoke.vue' });
assert.equal(descriptor.filename, 'smoke.vue');
const style = wasm.compileStyle({ source: '.a{ color: v-bind(color); }', id: 'data-v-smoke', scoped: true });
assert.match(style.code, /data-v-smoke/);
process.stdout.write(JSON.stringify({ status: 'pass', exports: Object.keys(wasm).sort() }));
"#;
    run_node_script(&project, script, NodeScriptMode::Module)
}

fn verify_release_cargo_dry_runs() -> Result<Vec<compat::ReportItem>> {
    let metadata = cargo_metadata_json()?;
    let packages = metadata
        .get("packages")
        .and_then(JsonValue::as_array)
        .context("cargo metadata output did not include packages array")?;
    let mut items = Vec::new();
    for package in packages
        .iter()
        .filter(|package| package_is_publishable(package))
    {
        let name = package
            .get("name")
            .and_then(JsonValue::as_str)
            .unwrap_or("<unknown>");
        let manifest_path = package
            .get("manifest_path")
            .and_then(JsonValue::as_str)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("Cargo.toml"));

        let package_list = run_cargo(&["package", "--list", "--allow-dirty", "-p", name]);
        if let Err(err) = package_list {
            items.push(compat::ReportItem::new(
                format!("cargo-package-list:{name}"),
                compat::ReportStatus::Fail,
                format!("{err:#}"),
                Some(manifest_path),
            ));
            continue;
        }

        let path_dependencies = package_path_dependencies(package);
        if !path_dependencies.is_empty() {
            items.push(compat::ReportItem::new(
                format!("cargo-publish-dry-run:{name}"),
                compat::ReportStatus::Pending,
                format!(
                    "cargo package file list resolved; cargo publish --dry-run requires internal dependencies to exist in the registry first: {}",
                    path_dependencies.join(", ")
                ),
                Some(manifest_path),
            ));
            continue;
        }

        match run_cargo(&["publish", "--dry-run", "--allow-dirty", "-p", name]) {
            Ok(output) => items.push(compat::ReportItem::new(
                format!("cargo-publish-dry-run:{name}"),
                compat::ReportStatus::Pass,
                output,
                Some(manifest_path),
            )),
            Err(err) => items.push(compat::ReportItem::new(
                format!("cargo-publish-dry-run:{name}"),
                compat::ReportStatus::Fail,
                format!("{err:#}"),
                Some(manifest_path),
            )),
        }
    }
    Ok(items)
}

fn stage_package_dir(source: &Path, target: &Path) -> Result<()> {
    if target.exists() {
        fs::remove_dir_all(target)
            .with_context(|| format!("failed to remove {}", target.display()))?;
    }
    copy_dir_recursive(source, target)
}

fn stage_release_native_platform_package(
    source: &Path,
    target: &Path,
    binding_source: &Path,
) -> Result<PathBuf> {
    stage_package_dir(source, target)?;
    copy_napi_binding_from(binding_source, &target.join("vuec_napi.node"))
}

fn find_native_artifact(root: Option<&Path>, platform_suffix: &str) -> Result<Option<PathBuf>> {
    let Some(root) = root else {
        return Ok(None);
    };
    if !root.exists() {
        anyhow::bail!(
            "native artifact directory {} does not exist",
            root.display()
        );
    }
    if !root.is_dir() {
        anyhow::bail!(
            "native artifact path {} must be a directory",
            root.display()
        );
    }

    let mut matches = Vec::new();
    for candidate in [
        root.join(platform_suffix).join("vuec_napi.node"),
        root.join(format!("{platform_suffix}.node")),
    ] {
        push_native_artifact_match(&mut matches, candidate, platform_suffix)?;
    }
    collect_native_artifact_matches(root, platform_suffix, &mut matches)?;
    matches.sort();
    matches.dedup();
    match matches.as_slice() {
        [] => Ok(None),
        [path] => Ok(Some(path.clone())),
        _ => anyhow::bail!(
            "multiple native artifacts found for {platform_suffix}: {}",
            matches
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn collect_native_artifact_matches(
    root: &Path,
    platform_suffix: &str,
    matches: &mut Vec<PathBuf>,
) -> Result<()> {
    for entry in fs::read_dir(root).with_context(|| format!("failed to read {}", root.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_native_artifact_matches(&path, platform_suffix, matches)?;
        } else {
            push_native_artifact_match(matches, path, platform_suffix)?;
        }
    }
    Ok(())
}

fn push_native_artifact_match(
    matches: &mut Vec<PathBuf>,
    candidate: PathBuf,
    platform_suffix: &str,
) -> Result<()> {
    if !candidate.exists() {
        return Ok(());
    }
    let file_name = candidate.file_name().and_then(|name| name.to_str());
    let parent_name = candidate
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str());
    let matches_platform = file_name == Some("vuec_napi.node")
        && parent_name == Some(platform_suffix)
        || file_name == Some(&format!("{platform_suffix}.node"));
    if !matches_platform {
        return Ok(());
    }
    if !candidate.is_file() {
        anyhow::bail!("native artifact {} is not a file", candidate.display());
    }
    if candidate
        .metadata()
        .with_context(|| format!("failed to inspect {}", candidate.display()))?
        .len()
        == 0
    {
        anyhow::bail!("native artifact {} is empty", candidate.display());
    }
    matches.push(candidate);
    Ok(())
}

fn native_binding_fingerprint(path: &Path) -> String {
    let size = path.metadata().map(|metadata| metadata.len()).unwrap_or(0);
    let hash = sha256_file(path)
        .map(|hash| hash.chars().take(16).collect::<String>())
        .unwrap_or_else(|_| "unavailable".into());
    format!("{size} bytes, sha256={hash}")
}

fn collect_native_platform_package_dirs() -> Result<Vec<PathBuf>> {
    let root = Path::new("packages").join("native-platforms");
    let mut dirs = Vec::new();
    for entry in
        fs::read_dir(&root).with_context(|| format!("failed to read {}", root.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() && path.join("package.json").exists() {
            dirs.push(path);
        }
    }
    dirs.sort();
    Ok(dirs)
}

fn native_platform_suffix(package_name: &str) -> Result<&str> {
    package_name
        .strip_prefix("@vuec-rs/native-")
        .with_context(|| format!("unsupported platform package name {package_name}"))
}

fn run_npm_pack_check(
    package_name: &str,
    package_dir: &Path,
    required_files: &[&str],
) -> compat::ReportItem {
    match npm_pack_dry_run(package_dir) {
        Ok(summary) => {
            let mut missing = required_files
                .iter()
                .copied()
                .filter(|file| !summary.files.iter().any(|packed| packed == file))
                .collect::<Vec<_>>();
            missing.sort();
            if missing.is_empty() {
                compat::ReportItem::new(
                    format!("npm:{package_name}"),
                    compat::ReportStatus::Pass,
                    format!(
                        "npm pack --dry-run produced {} with {} files ({} bytes packed)",
                        summary.filename, summary.entry_count, summary.size
                    ),
                    Some(package_dir.to_path_buf()),
                )
            } else {
                compat::ReportItem::new(
                    format!("npm:{package_name}"),
                    compat::ReportStatus::Fail,
                    format!(
                        "npm pack --dry-run omitted required files: {}; packed files: {}",
                        missing.join(", "),
                        summary.files.join(", ")
                    ),
                    Some(package_dir.to_path_buf()),
                )
            }
        }
        Err(err) => compat::ReportItem::new(
            format!("npm:{package_name}"),
            compat::ReportStatus::Fail,
            format!("{err:#}"),
            Some(package_dir.to_path_buf()),
        ),
    }
}

struct NpmPackSummary {
    filename: String,
    entry_count: usize,
    size: u64,
    files: Vec<String>,
}

fn npm_pack_dry_run(package_dir: &Path) -> Result<NpmPackSummary> {
    let npm = resolve_program("npm")?;
    let output = ProcessCommand::new(npm)
        .args(["pack", "--dry-run", "--json"])
        .current_dir(package_dir)
        .output()
        .with_context(|| {
            format!(
                "failed to spawn npm pack --dry-run in {}",
                package_dir.display()
            )
        })?;
    if !output.status.success() {
        anyhow::bail!(
            "npm pack --dry-run in {} exited with {:?}\nstdout:\n{}\nstderr:\n{}",
            package_dir.display(),
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let value: JsonValue = serde_json::from_slice(&output.stdout).with_context(|| {
        format!(
            "npm pack --dry-run stdout was not JSON in {}:\n{}",
            package_dir.display(),
            String::from_utf8_lossy(&output.stdout)
        )
    })?;
    let entry = value
        .as_array()
        .and_then(|entries| entries.first())
        .context("npm pack --dry-run JSON did not include a package entry")?;
    let files = entry
        .get("files")
        .and_then(JsonValue::as_array)
        .context("npm pack --dry-run JSON did not include files")?
        .iter()
        .filter_map(|file| file.get("path").and_then(JsonValue::as_str))
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    Ok(NpmPackSummary {
        filename: entry
            .get("filename")
            .and_then(JsonValue::as_str)
            .unwrap_or("<unknown>")
            .to_string(),
        entry_count: entry
            .get("entryCount")
            .and_then(JsonValue::as_u64)
            .unwrap_or(files.len() as u64) as usize,
        size: entry.get("size").and_then(JsonValue::as_u64).unwrap_or(0),
        files,
    })
}

fn npm_pack(package_dir: &Path) -> Result<PathBuf> {
    let npm = resolve_program("npm")?;
    let output = ProcessCommand::new(npm)
        .args(["pack", "--json"])
        .current_dir(package_dir)
        .output()
        .with_context(|| format!("failed to spawn npm pack in {}", package_dir.display()))?;
    if !output.status.success() {
        anyhow::bail!(
            "npm pack in {} exited with {:?}\nstdout:\n{}\nstderr:\n{}",
            package_dir.display(),
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let value: JsonValue = serde_json::from_slice(&output.stdout).with_context(|| {
        format!(
            "npm pack stdout was not JSON in {}:\n{}",
            package_dir.display(),
            String::from_utf8_lossy(&output.stdout)
        )
    })?;
    let filename = value
        .as_array()
        .and_then(|entries| entries.first())
        .and_then(|entry| entry.get("filename"))
        .and_then(JsonValue::as_str)
        .context("npm pack JSON did not include filename")?;
    Ok(package_dir.join(filename))
}

fn find_single_tgz(dir: &Path) -> Result<PathBuf> {
    let mut tarballs = fs::read_dir(dir)
        .with_context(|| format!("failed to read {}", dir.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("tgz"))
        .collect::<Vec<_>>();
    tarballs.sort();
    match tarballs.as_slice() {
        [path] => Ok(path.clone()),
        [] => anyhow::bail!("no .tgz package found in {}", dir.display()),
        _ => anyhow::bail!("multiple .tgz packages found in {}", dir.display()),
    }
}

fn write_clean_npm_project(root: &Path, name: &str, package_type: Option<&str>) -> Result<()> {
    let mut manifest = json!({
        "name": name,
        "version": "0.0.0",
        "private": true,
    });
    if let Some(package_type) = package_type {
        manifest["type"] = JsonValue::String(package_type.to_string());
    }
    fs::write(
        root.join("package.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )
    .with_context(|| format!("failed to write {}", root.join("package.json").display()))
}

fn npm_install_tarballs(root: &Path, tarballs: &[&Path]) -> Result<()> {
    let npm = resolve_program("npm")?;
    let mut command = ProcessCommand::new(npm);
    command
        .arg("install")
        .arg("--ignore-scripts")
        .arg("--no-audit")
        .arg("--no-fund")
        .arg("--package-lock=false");
    for tarball in tarballs {
        command.arg(absolute_path(tarball));
    }
    let output = command
        .current_dir(root)
        .output()
        .with_context(|| format!("failed to spawn npm install in {}", root.display()))?;
    if !output.status.success() {
        anyhow::bail!(
            "npm install in {} exited with {:?}\nstdout:\n{}\nstderr:\n{}",
            root.display(),
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum NodeScriptMode {
    CommonJs,
    Module,
}

fn run_node_script(root: &Path, script: &str, mode: NodeScriptMode) -> Result<String> {
    let mut command = ProcessCommand::new("node");
    if matches!(mode, NodeScriptMode::Module) {
        command.arg("--input-type=module");
    }
    let output = command
        .arg("-e")
        .arg(script)
        .current_dir(root)
        .output()
        .with_context(|| format!("failed to spawn node install smoke in {}", root.display()))?;
    if !output.status.success() {
        anyhow::bail!(
            "node install smoke in {} exited with {:?}\nstdout:\n{}\nstderr:\n{}",
            root.display(),
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn package_path_dependencies(package: &JsonValue) -> Vec<String> {
    let mut dependencies = package
        .get("dependencies")
        .and_then(JsonValue::as_array)
        .into_iter()
        .flatten()
        .filter(|dependency| dependency.get("path").is_some())
        .filter_map(|dependency| dependency.get("name").and_then(JsonValue::as_str))
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    dependencies.sort();
    dependencies
}

fn build_wasm_release_packages() -> Result<Vec<PathBuf>> {
    let installed = installed_rust_targets()?;
    if !installed
        .iter()
        .any(|target| target == "wasm32-unknown-unknown")
    {
        anyhow::bail!(
            "rust target wasm32-unknown-unknown is not installed; run `rustup target add wasm32-unknown-unknown`"
        );
    }
    let wasm_bindgen = resolve_program("wasm-bindgen")?;
    let output = ProcessCommand::new("cargo")
        .args([
            "build",
            "-p",
            "vuec_wasm",
            "--release",
            "--target",
            "wasm32-unknown-unknown",
        ])
        .output()
        .context(
            "failed to spawn cargo build -p vuec_wasm --release --target wasm32-unknown-unknown",
        )?;
    if !output.status.success() {
        anyhow::bail!(
            "cargo build -p vuec_wasm --release --target wasm32-unknown-unknown exited with {:?}\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let wasm_path = PathBuf::from("target")
        .join("wasm32-unknown-unknown")
        .join("release")
        .join("vuec_wasm.wasm");
    let specs = [
        ("web", PathBuf::from("packages").join("wasm").join("pkg")),
        (
            "nodejs",
            PathBuf::from("packages").join("wasm").join("pkg-node"),
        ),
    ];
    let mut created = Vec::new();
    for (target, pkg_dir) in specs {
        if pkg_dir.exists() {
            fs::remove_dir_all(&pkg_dir)
                .with_context(|| format!("failed to remove {}", pkg_dir.display()))?;
        }
        fs::create_dir_all(&pkg_dir)
            .with_context(|| format!("failed to create {}", pkg_dir.display()))?;
        let output = ProcessCommand::new(&wasm_bindgen)
            .args([
                "--target",
                target,
                "--out-dir",
                &pkg_dir.display().to_string(),
                &wasm_path.display().to_string(),
            ])
            .output()
            .with_context(|| format!("failed to spawn wasm-bindgen --target {target}"))?;
        if !output.status.success() {
            anyhow::bail!(
                "wasm-bindgen --target {target} exited with {:?}\nstdout:\n{}\nstderr:\n{}",
                output.status.code(),
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        if target == "nodejs" {
            fs::write(pkg_dir.join("package.json"), r#"{"type":"commonjs"}"#).with_context(
                || format!("failed to write {}", pkg_dir.join("package.json").display()),
            )?;
        }
        created.push(pkg_dir);
    }
    Ok(created)
}

fn push_file_check_item(
    items: &mut Vec<compat::ReportItem>,
    violations: &mut Vec<String>,
    path: PathBuf,
) {
    match require_non_empty_file(&path) {
        Ok(()) => items.push(compat::ReportItem::new(
            format!("file:{}", path.display()),
            compat::ReportStatus::Pass,
            "file exists and is non-empty",
            Some(path),
        )),
        Err(err) => {
            violations.push(format!("{err:#}"));
            items.push(compat::ReportItem::new(
                format!("file:{}", path.display()),
                compat::ReportStatus::Fail,
                format!("{err:#}"),
                Some(path),
            ));
        }
    }
}

fn root_package_manager_is_pinned(path: &Path) -> Result<String> {
    let manifest = read_json_file(path)?;
    let package_manager = manifest
        .get("packageManager")
        .and_then(JsonValue::as_str)
        .with_context(|| format!("{} missing packageManager", path.display()))?;
    if package_manager != "pnpm@9.0.0" {
        anyhow::bail!(
            "{} packageManager must be pnpm@9.0.0, got {package_manager}",
            path.display()
        );
    }
    if manifest
        .get("license")
        .and_then(JsonValue::as_str)
        .is_none_or(str::is_empty)
    {
        anyhow::bail!("{} missing license metadata", path.display());
    }
    Ok(format!("packageManager pinned to {package_manager}"))
}

fn verify_npm_manifest_supply_chain(path: &Path) -> Result<()> {
    let manifest = read_json_file(path)?;
    let name = manifest
        .get("name")
        .and_then(JsonValue::as_str)
        .with_context(|| format!("{} missing package name", path.display()))?;
    if manifest
        .get("license")
        .and_then(JsonValue::as_str)
        .is_none_or(str::is_empty)
    {
        anyhow::bail!("{name} missing license metadata");
    }
    for field in [
        "dependencies",
        "optionalDependencies",
        "peerDependencies",
        "devDependencies",
    ] {
        verify_exact_npm_dependency_versions(path, name, field, &manifest)?;
    }
    if name.starts_with("@vuec-rs/native-") {
        verify_platform_package_files(path, name, &manifest)?;
    }
    Ok(())
}

fn verify_exact_npm_dependency_versions(
    path: &Path,
    package_name: &str,
    field: &str,
    manifest: &JsonValue,
) -> Result<()> {
    let Some(dependencies) = manifest.get(field) else {
        return Ok(());
    };
    let Some(dependencies) = dependencies.as_object() else {
        anyhow::bail!("{} {field} field is not an object", path.display());
    };
    for (name, version) in dependencies {
        let Some(version) = version.as_str() else {
            anyhow::bail!("{package_name} dependency {name} in {field} is not a string");
        };
        if !is_exact_npm_version(version) {
            anyhow::bail!(
                "{package_name} dependency {name} in {field} must use an exact version, got {version}"
            );
        }
    }
    Ok(())
}

fn is_exact_npm_version(version: &str) -> bool {
    !version.is_empty()
        && !version.starts_with(['^', '~', '>', '<', '=', '*'])
        && !version.contains("||")
        && !version.contains(" - ")
        && version != "latest"
}

fn verify_platform_package_files(
    path: &Path,
    package_name: &str,
    manifest: &JsonValue,
) -> Result<()> {
    let files = manifest
        .get("files")
        .and_then(JsonValue::as_array)
        .with_context(|| format!("{package_name} missing files array"))?;
    let entries = files
        .iter()
        .filter_map(JsonValue::as_str)
        .collect::<Vec<_>>();
    let expected = ["vuec_napi.node", "README.md"];
    if entries != expected {
        anyhow::bail!(
            "{} files must be {:?}, got {:?}",
            path.display(),
            expected,
            entries
        );
    }
    Ok(())
}

fn package_is_publishable(package: &JsonValue) -> bool {
    match package.get("publish") {
        None | Some(JsonValue::Null) => true,
        Some(JsonValue::Array(registries)) => !registries.is_empty(),
        Some(JsonValue::Bool(value)) => *value,
        _ => true,
    }
}

fn cargo_metadata_json() -> Result<JsonValue> {
    let output = ProcessCommand::new("cargo")
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .output()
        .context("failed to spawn cargo metadata")?;
    if !output.status.success() {
        anyhow::bail!(
            "cargo metadata exited with {:?}\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    serde_json::from_slice(&output.stdout).context("failed to parse cargo metadata JSON")
}
