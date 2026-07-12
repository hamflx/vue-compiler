fn selected_api_manifest_sides(scope: &SelectionArgs) -> Vec<ApiManifestSide> {
    match (scope.official, scope.rust) {
        (true, false) => vec![ApiManifestSide::Official],
        (false, true) => vec![ApiManifestSide::Rust],
        _ => vec![ApiManifestSide::Official, ApiManifestSide::Rust],
    }
}

pub fn diff_api(scope: &SelectionArgs) -> JsonReport {
    let targets = select_targets(scope);
    let mut items = Vec::new();
    let mut violations = Vec::new();
    let lock_path = PathBuf::from("compat/official-revisions.lock");
    let lock_hash = file_sha256(&lock_path).ok();
    let lock = load_official_lock(&lock_path).ok();
    let allowed = load_allowed_api_diffs(&PathBuf::from("compat/api/allowed-diff.json"));
    for target in targets {
        let official_path = target.relative_api_manifest_path(ApiManifestSide::Official.as_str());
        let rust_path = target.relative_api_manifest_path(ApiManifestSide::Rust.as_str());
        match (
            read_json::<ManifestFile>(&official_path),
            read_json::<ManifestFile>(&rust_path),
        ) {
            (Ok(official), Ok(rust)) => {
                let mut diffs = compare_api_manifests(&official, &rust);
                diffs.retain(|diff| !is_allowed_api_diff(&allowed, target, diff));
                if diffs.is_empty() {
                    items.push(ReportItem::new(
                        target.display(),
                        ReportStatus::Pass,
                        "official and Rust API manifests match",
                        Some(rust_path),
                    ));
                } else {
                    violations.extend(
                        diffs
                            .iter()
                            .map(|diff| format!("{}: {diff}", target.display())),
                    );
                    items.push(ReportItem::new(
                        target.display(),
                        ReportStatus::Fail,
                        format!("{} API manifest differences", diffs.len()),
                        Some(rust_path),
                    ));
                }
            }
            (Err(err), _) => {
                violations.push(format!(
                    "{} official manifest missing/invalid: {err}",
                    target.display()
                ));
                items.push(ReportItem::new(
                    target.display(),
                    ReportStatus::Fail,
                    "official API manifest missing or invalid",
                    Some(official_path),
                ));
            }
            (_, Err(err)) => {
                violations.push(format!(
                    "{} Rust manifest missing/invalid: {err}",
                    target.display()
                ));
                items.push(ReportItem::new(
                    target.display(),
                    ReportStatus::Fail,
                    "Rust API manifest missing or invalid",
                    Some(rust_path),
                ));
            }
        }
    }
    let mut report = JsonReport::new("diff_api", ReportStatus::Pending);
    report.metadata = report.metadata.with_lock_context(lock_hash, lock.as_ref());
    report
        .with_scope(scope)
        .with_items(items)
        .with_violations(violations)
        .with_note("diff compares generated official and Rust alias manifests field-by-field")
}

fn export_official_api_manifest(
    target: TargetSpec,
    lock: Option<&OfficialRevisionsLock>,
    lock_hash: Option<String>,
) -> Result<ManifestFile> {
    let lock = lock.context("compat/official-revisions.lock is missing or invalid")?;
    let baseline = baseline_for(lock, target.version_line)
        .context("target version line is missing from official lock")?;
    let install_root = ensure_official_npm_install(target.version_line, baseline)?;
    let request = api_require_request(target);
    let probe = probe_api_exports(&install_root, target.package, &request)?;
    Ok(manifest_from_probe(
        target,
        ApiManifestSide::Official,
        lock_hash,
        Some(baseline.rev.clone()),
        probe,
    ))
}

fn export_rust_api_manifest(target: TargetSpec, lock_hash: Option<String>) -> Result<ManifestFile> {
    let alias_root = PathBuf::from("target")
        .join("compat")
        .join("rust-alias")
        .join(target.version_line.as_str());
    let request = api_require_request(target);
    let probe = probe_api_exports(&alias_root, target.package, &request)?;
    Ok(manifest_from_probe(
        target,
        ApiManifestSide::Rust,
        lock_hash,
        None,
        probe,
    ))
}

fn generate_rust_alias_packages(targets: &[TargetSpec]) -> Result<Vec<PathBuf>> {
    ensure_node_bridge_binary()?;
    let mut created = Vec::new();
    for target in targets {
        let official_manifest_path =
            target.relative_api_manifest_path(ApiManifestSide::Official.as_str());
        let manifest = read_json::<ManifestFile>(&official_manifest_path).with_context(|| {
            format!(
                "official API manifest {} is required before Rust alias generation; run `cargo xtask export-api --official --all`",
                official_manifest_path.display()
            )
        })?;
        let root = rust_alias_root(target.version_line);
        let package_dir = rust_alias_package_dir(*target);
        fs::create_dir_all(&package_dir)
            .with_context(|| format!("failed to create {}", package_dir.display()))?;
        write_alias_package_json(&package_dir, *target, &manifest)?;
        write_alias_index(&root, &package_dir, *target, &manifest)?;
        write_alias_types(&package_dir, *target, &manifest)?;
        created.push(package_dir);
    }
    Ok(created)
}

fn ensure_node_bridge_binary() -> Result<PathBuf> {
    run_command("cargo", &["build", "-p", "vuec_node_bridge"], None)
        .context("failed to build vuec_node_bridge")?;
    let exe_name = if cfg!(windows) {
        "vuec_node_bridge.exe"
    } else {
        "vuec_node_bridge"
    };
    Ok(PathBuf::from("target").join("debug").join(exe_name))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AliasBackend {
    Generated,
    Napi,
}

impl AliasBackend {
    fn name(self) -> &'static str {
        match self {
            AliasBackend::Generated => "generated",
            AliasBackend::Napi => "napi",
        }
    }

    fn label(self) -> &'static str {
        match self {
            AliasBackend::Generated => "generated Rust",
            AliasBackend::Napi => "NAPI-backed",
        }
    }

    fn root(self, version_line: VersionLine) -> PathBuf {
        match self {
            AliasBackend::Generated => rust_alias_root(version_line),
            AliasBackend::Napi => napi_alias_root(version_line),
        }
    }

    fn option_command(self) -> &'static str {
        match self {
            AliasBackend::Generated => "run_option_matrix",
            AliasBackend::Napi => "run_napi_option_matrix",
        }
    }

    fn output_command(self) -> &'static str {
        match self {
            AliasBackend::Generated => "run_output_contract",
            AliasBackend::Napi => "run_napi_output_contract",
        }
    }

    fn conformance_command(self) -> &'static str {
        match self {
            AliasBackend::Generated => "run_conformance",
            AliasBackend::Napi => "run_napi_conformance",
        }
    }

    fn option_report_name(self) -> &'static str {
        match self {
            AliasBackend::Generated => "option-matrix.json",
            AliasBackend::Napi => "napi-option-matrix.json",
        }
    }

    fn output_report_name(self) -> &'static str {
        match self {
            AliasBackend::Generated => "output-contract.json",
            AliasBackend::Napi => "napi-output-contract.json",
        }
    }

    fn conformance_report_name(self, spec: ConformanceSuiteSpec) -> String {
        match self {
            AliasBackend::Generated => format!("{}.json", spec.name),
            AliasBackend::Napi => format!("napi-{}.json", spec.name),
        }
    }

    fn option_side(self) -> &'static str {
        match self {
            AliasBackend::Generated => "rust",
            AliasBackend::Napi => "napi",
        }
    }

    fn option_note(self) -> &'static str {
        match self {
            AliasBackend::Generated => {
                "option matrix now executes official vs Rust probe cases and records per-row results"
            }
            AliasBackend::Napi => {
                "option matrix executes official packages against NAPI-backed official package-name aliases"
            }
        }
    }

    fn output_note(self) -> &'static str {
        match self {
            AliasBackend::Generated => {
                "output contract executes official npm packages and generated Rust alias packages against representative fixtures"
            }
            AliasBackend::Napi => {
                "output contract executes official npm packages and NAPI-backed official package-name aliases against representative fixtures"
            }
        }
    }

    fn conformance_note(self) -> &'static str {
        match self {
            AliasBackend::Generated => {
                "official conformance executes against generated Rust alias packages"
            }
            AliasBackend::Napi => {
                "official conformance executes against NAPI-backed official package-name aliases; coverage still distinguishes rust-backed, shim-backed, and mixed paths"
            }
        }
    }
}

fn rust_alias_root(version_line: VersionLine) -> PathBuf {
    PathBuf::from("target")
        .join("compat")
        .join("rust-alias")
        .join(version_line.as_str())
}

fn napi_alias_root(version_line: VersionLine) -> PathBuf {
    PathBuf::from("target")
        .join("compat")
        .join("napi-alias")
        .join(version_line.as_str())
}

fn prepare_alias_backend(backend: AliasBackend, targets: &[TargetSpec]) -> Result<Vec<PathBuf>> {
    match backend {
        AliasBackend::Generated => generate_rust_alias_packages(targets),
        AliasBackend::Napi => prepare_napi_alias_packages(targets),
    }
}

fn prepare_napi_alias_packages(targets: &[TargetSpec]) -> Result<Vec<PathBuf>> {
    run_command("cargo", &["build", "-p", "vuec_napi"], None)
        .context("failed to build vuec_napi")?;
    let mut version_lines = Vec::new();
    for target in targets {
        if !version_lines.contains(&target.version_line) {
            version_lines.push(target.version_line);
        }
    }
    let mut created = Vec::new();
    for version_line in version_lines {
        let root = napi_alias_root(version_line);
        reset_napi_alias_root(&root)?;
        prepare_napi_alias_root(version_line, &root)?;
        created.push(root);
    }
    Ok(created)
}

fn reset_napi_alias_root(root: &Path) -> Result<()> {
    ensure_target_compat_child(root, "napi-alias")?;
    if root.exists() {
        fs::remove_dir_all(root).with_context(|| format!("failed to remove {}", root.display()))?;
    }
    fs::create_dir_all(root).with_context(|| format!("failed to create {}", root.display()))
}

fn ensure_target_compat_child(path: &Path, child: &str) -> Result<()> {
    let cwd = std::env::current_dir().context("failed to resolve current directory")?;
    let expected = cwd.join("target").join("compat").join(child);
    let absolute = absolute_path(path);
    ensure!(
        absolute.starts_with(&expected),
        "refusing to recursively replace {}; expected a path under {}",
        absolute.display(),
        expected.display()
    );
    Ok(())
}

fn prepare_napi_alias_root(version_line: VersionLine, root: &Path) -> Result<()> {
    let node_modules = root.join("node_modules");
    fs::create_dir_all(&node_modules)
        .with_context(|| format!("failed to create {}", node_modules.display()))?;

    let native_target = node_modules.join("@vuec-rs").join("native");
    copy_dir_recursive(Path::new("packages/native"), &native_target)?;
    copy_napi_binding(&native_target.join("vuec_napi.node"))?;

    copy_napi_alias_package(
        Path::new("packages/native-aliases/vue-template-compiler"),
        &node_modules.join("vue-template-compiler"),
    )?;
    select_napi_vue_template_compiler(version_line, &node_modules.join("vue-template-compiler"))?;
    copy_napi_alias_package(
        Path::new("packages/native-aliases/vue"),
        &node_modules.join("vue"),
    )?;
    copy_napi_alias_package(
        Path::new("packages/native-aliases/@vue/compiler-core"),
        &node_modules.join("@vue").join("compiler-core"),
    )?;
    copy_napi_alias_package(
        Path::new("packages/native-aliases/@vue/compiler-dom"),
        &node_modules.join("@vue").join("compiler-dom"),
    )?;
    copy_napi_alias_package(
        Path::new("packages/native-aliases/@vue/compiler-ssr"),
        &node_modules.join("@vue").join("compiler-ssr"),
    )?;
    copy_napi_alias_package(
        Path::new("packages/native-aliases/@vue/compiler-sfc"),
        &node_modules.join("@vue").join("compiler-sfc"),
    )?;
    write_napi_alias_versions(version_line, &node_modules)?;
    Ok(())
}

fn copy_napi_alias_package(source: &Path, target: &Path) -> Result<()> {
    copy_dir_recursive(source, target)
}

fn select_napi_vue_template_compiler(version_line: VersionLine, package_dir: &Path) -> Result<()> {
    let variant = match version_line {
        VersionLine::Vue26 => "index-vue2_6.js",
        VersionLine::Vue27 | VersionLine::Vue3 => "index-vue2_7.js",
    };
    fs::copy(package_dir.join(variant), package_dir.join("index.js"))
        .with_context(|| format!("failed to select {} for {}", variant, package_dir.display()))?;
    Ok(())
}

fn write_napi_alias_versions(version_line: VersionLine, node_modules: &Path) -> Result<()> {
    for target in all_targets()
        .iter()
        .copied()
        .filter(|target| target.version_line == version_line)
    {
        let manifest = read_json::<ManifestFile>(&target.relative_api_manifest_path("official"))?;
        let package_json = napi_alias_package_json_path(target, node_modules);
        write_package_json_version(
            &package_json,
            manifest.package_version.as_deref().unwrap_or("0.0.0"),
        )?;
    }
    Ok(())
}

fn napi_alias_package_json_path(target: TargetSpec, node_modules: &Path) -> PathBuf {
    match target.kind {
        TargetKind::Vue26Template | TargetKind::Vue27Template => node_modules
            .join("vue-template-compiler")
            .join("package.json"),
        TargetKind::Vue27Sfc => node_modules.join("vue").join("package.json"),
        TargetKind::Vue3Core => node_modules
            .join("@vue")
            .join("compiler-core")
            .join("package.json"),
        TargetKind::Vue3Dom => node_modules
            .join("@vue")
            .join("compiler-dom")
            .join("package.json"),
        TargetKind::Vue3Ssr => node_modules
            .join("@vue")
            .join("compiler-ssr")
            .join("package.json"),
        TargetKind::Vue3Sfc => node_modules
            .join("@vue")
            .join("compiler-sfc")
            .join("package.json"),
    }
}

fn write_package_json_version(path: &Path, version: &str) -> Result<()> {
    let mut value = read_json::<serde_json::Value>(path)?;
    value["version"] = serde_json::Value::String(version.to_string());
    write_json(path, &value)
}

fn copy_napi_binding(target_path: &Path) -> Result<()> {
    let source_path = napi_library_path();
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::copy(&source_path, target_path).with_context(|| {
        format!(
            "failed to copy {} to {}",
            source_path.display(),
            target_path.display()
        )
    })?;
    Ok(())
}

fn napi_library_path() -> PathBuf {
    let (prefix, suffix) = match std::env::consts::OS {
        "windows" => ("", ".dll"),
        "macos" => ("lib", ".dylib"),
        _ => ("lib", ".so"),
    };
    PathBuf::from("target")
        .join("debug")
        .join(format!("{prefix}vuec_napi{suffix}"))
}

fn rust_alias_package_dir(target: TargetSpec) -> PathBuf {
    let root = rust_alias_root(target.version_line).join("node_modules");
    match target.package {
        package if package.starts_with("@vue/") => {
            let package_name = package.trim_start_matches("@vue/");
            root.join("@vue").join(package_name)
        }
        "vue" => root.join("vue"),
        package => root.join(package),
    }
}

fn write_alias_package_json(
    package_dir: &Path,
    target: TargetSpec,
    manifest: &ManifestFile,
) -> Result<()> {
    let main = match target.kind {
        TargetKind::Vue3Sfc => "dist/compiler-sfc.cjs.js",
        TargetKind::Vue3Ssr => "dist/compiler-ssr.cjs.js",
        TargetKind::Vue27Sfc => "index.js",
        _ => "index.js",
    };
    let types = manifest
        .types
        .package_types
        .as_deref()
        .unwrap_or("index.d.ts");
    let package_json = serde_json::json!({
        "name": target.package,
        "version": manifest.package_version.as_deref().unwrap_or("0.0.0"),
        "private": true,
        "main": main,
        "types": types,
        "description": "Generated Rust Vue compiler compatibility alias package",
    });
    write_json(&package_dir.join("package.json"), &package_json)
}

fn write_alias_index(
    alias_root: &Path,
    package_dir: &Path,
    target: TargetSpec,
    manifest: &ManifestFile,
) -> Result<()> {
    let main_path = match target.kind {
        TargetKind::Vue3Sfc => package_dir.join("dist").join("compiler-sfc.cjs.js"),
        TargetKind::Vue3Ssr => package_dir.join("dist").join("compiler-ssr.cjs.js"),
        TargetKind::Vue27Sfc => package_dir.join("compiler-sfc").join("index.js"),
        _ => package_dir.join("index.js"),
    };
    if let Some(parent) = main_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let mut source = String::new();
    source.push_str("'use strict';\n\n");
    source.push_str("const cp = require('child_process');\n");
    source.push_str("const path = require('path');\n\n");
    source.push_str("const BRIDGE_BIN = process.env.VUEC_NODE_BRIDGE || path.resolve(__dirname, ");
    source.push_str(&js_string_literal(&bridge_relative_path(
        alias_root, &main_path,
    )));
    source.push_str(");\n");
    source.push('\n');
    source.push_str(ALIAS_RUNTIME_JS);
    source.push('\n');
    if target.kind == TargetKind::Vue3Core {
        source.push_str("Object.defineProperty(vue3CoreRuntime, 'callBridge', { value: callBridge, enumerable: false });\n");
        source.push_str("Object.defineProperty(exports, '__vuecRuntime', { value: vue3CoreRuntime, enumerable: false });\n");
    } else if target.kind == TargetKind::Vue3Dom {
        source.push_str("Object.defineProperty(exports, '__vuecRuntime', { value: Object.assign({}, vue3CoreRuntime, { decodeHtmlBrowser: vue3CoreRuntime.decodeHtmlBrowser, ignoreSideEffectTags: vue3CoreRuntime.ignoreSideEffectTags, transformOn: vue3CoreRuntime.transformDomOn, transformModel: vue3CoreRuntime.transformDomModel, transformTransition: vue3CoreRuntime.transformDomTransition, validateHtmlNesting: vue3CoreRuntime.validateHtmlNesting, isValidHTMLNesting: vue3CoreRuntime.isValidHTMLNesting }), enumerable: false });\n");
    } else if matches!(
        target.kind,
        TargetKind::Vue26Template | TargetKind::Vue27Template | TargetKind::Vue3Sfc
    ) {
        source.push_str("Object.defineProperty(exports, '__vuecRuntime', { value: vuecBridgeRuntime, enumerable: false });\n");
    }
    for export_name in &manifest.exports {
        let detail = manifest.export_details.get(export_name);
        source.push_str("exports[");
        source.push_str(&js_string_literal(export_name));
        source.push_str("] = ");
        source.push_str(&alias_export_expression(target, export_name, detail));
        source.push_str(";\n");
    }
    write_text(&main_path, &source)?;
    if target.kind == TargetKind::Vue27Sfc {
        write_text(
            &package_dir.join("index.js"),
            "module.exports = require('./compiler-sfc/index.js');\n",
        )?;
    }
    Ok(())
}

fn write_alias_types(
    package_dir: &Path,
    target: TargetSpec,
    manifest: &ManifestFile,
) -> Result<()> {
    let relative = manifest
        .types
        .package_types
        .as_deref()
        .unwrap_or("index.d.ts");
    let path = package_dir.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let mut body = String::new();
    body.push_str("// Generated compatibility alias declarations.\n");
    body.push_str("export const __vuecRustAlias: true;\n");
    if target.kind == TargetKind::Vue27Sfc {
        let root_types = package_dir.join("index.d.ts");
        write_text(&root_types, "export * from './compiler-sfc/index';\n")?;
    }
    write_text(&path, &body)
}

fn bridge_relative_path(alias_root: &Path, from_file: &Path) -> String {
    let depth = from_file
        .parent()
        .and_then(|parent| parent.strip_prefix(alias_root).ok())
        .map(|relative| relative.components().count())
        .unwrap_or(0);
    let mut path = String::new();
    for _ in 0..depth {
        path.push_str("../");
    }
    path.push_str("../../../debug/");
    path.push_str(if cfg!(windows) {
        "vuec_node_bridge.exe"
    } else {
        "vuec_node_bridge"
    });
    path
}

fn alias_export_expression(
    target: TargetSpec,
    export_name: &str,
    detail: Option<&ApiExportDetail>,
) -> String {
    let Some(detail) = detail else {
        return "undefined".into();
    };
    if target.kind == TargetKind::Vue3Core
        && vue3_core_runtime_export(export_name, detail).is_some()
    {
        if detail.kind == "function" {
            return alias_runtime_function_expression("vue3CoreRuntime", export_name, detail);
        }
        return format!("vue3CoreRuntime[{}]", js_string_literal(export_name));
    }
    if target.kind == TargetKind::Vue3Dom && export_name == "parserOptions" {
        return "vue3DomParserOptions".into();
    }
    if target.kind == TargetKind::Vue3Dom && export_name == "transformOn" {
        return alias_runtime_function_expression_as(
            "vue3CoreRuntime",
            "transformDomOn",
            export_name,
            detail,
        );
    }
    if target.kind == TargetKind::Vue3Dom && export_name == "transformModel" {
        return alias_runtime_function_expression_as(
            "vue3CoreRuntime",
            "transformDomModel",
            export_name,
            detail,
        );
    }
    if target.kind == TargetKind::Vue3Dom
        && !matches!(
            export_name,
            "baseCompile" | "baseParse" | "compile" | "generate" | "parse"
        )
        && vue3_core_runtime_export(export_name, detail).is_some()
    {
        if detail.kind == "function" {
            return alias_runtime_function_expression("vue3CoreRuntime", export_name, detail);
        }
        return format!("vue3CoreRuntime[{}]", js_string_literal(export_name));
    }
    if target.kind == TargetKind::Vue3Sfc && matches!(export_name, "babelParse" | "walkIdentifiers")
    {
        return alias_runtime_function_expression("vue3CoreRuntime", export_name, detail);
    }
    if target.kind == TargetKind::Vue27Sfc
        && matches!(export_name, "compileStyle" | "compileStyleAsync")
        && detail.kind == "function"
    {
        return vue27_sfc_style_function_expression(export_name, detail);
    }
    match detail.kind.as_str() {
        "function" => alias_function_expression(target, export_name, detail),
        "symbol" => "Symbol.for('vuec.alias')".into(),
        "string" => manifest_string_value(target, export_name),
        "object" if detail.tag == "[object Array]" => {
            let entries = detail
                .own_property_names
                .iter()
                .filter(|name| name.chars().all(|ch| ch.is_ascii_digit()))
                .map(|name| js_string_literal(name))
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{entries}]")
        }
        "object" if detail.tag == "[object RegExp]" => "/(?:)/".into(),
        "object" => object_from_property_names(&detail.own_property_names),
        _ => "undefined".into(),
    }
}

fn vue27_sfc_style_function_expression(export_name: &str, detail: &ApiExportDetail) -> String {
    let name = detail.name.as_deref().unwrap_or(export_name);
    let arity = detail.function_arity.unwrap_or(0);
    let command = bridge_command(
        TargetSpec {
            version_line: VersionLine::Vue27,
            package: "vue",
            entry: "vue/compiler-sfc",
            kind: TargetKind::Vue27Sfc,
        },
        export_name,
    )
    .unwrap_or("sfc.vue27.compileStyle");
    let postcss_call = if export_name == "compileStyleAsync" {
        "applyVue27StylePostcssAsync(__vuecBridgeResult, __vuecPayload.options)"
    } else {
        "applyVue27StylePostcssSync(__vuecBridgeResult, __vuecPayload.options)"
    };
    let body = format!(
        "const __vuecPayload = resolveStylePreprocessPayload(normalizeArgs({})); preflightAliasCall({}, __vuecPayload); const __vuecBridgePayload = vue27StyleBridgePayload(__vuecPayload); const __vuecBridgeResult = callBridge({}, bridgePayloadForCall(__vuecBridgePayload)); return {postcss_call};",
        alias_argument_object(
            TargetSpec {
                version_line: VersionLine::Vue27,
                package: "vue",
                entry: "vue/compiler-sfc",
                kind: TargetKind::Vue27Sfc,
            },
            export_name,
            arity,
        ),
        js_string_literal(alias_preflight_name(
            TargetSpec {
                version_line: VersionLine::Vue27,
                package: "vue",
                entry: "vue/compiler-sfc",
                kind: TargetKind::Vue27Sfc,
            },
            export_name,
        )),
        js_string_literal(command),
    );
    let expression = format!("function {name}(a0) {{ {body} }}");
    if detail
        .own_property_names
        .iter()
        .any(|prop| prop == "prototype")
    {
        expression
    } else {
        format!(
            "namedArity({}, {}, {})",
            js_string_literal(name),
            arity,
            expression
        )
    }
}

fn vue3_core_runtime_export(export_name: &str, detail: &ApiExportDetail) -> Option<()> {
    match export_name {
        "baseCompile" | "baseParse" | "generate" => None,
        _ if detail.kind == "function" => Some(()),
        _ if detail.kind == "symbol" => Some(()),
        "BindingTypes"
        | "CompilerDeprecationTypes"
        | "ConstantTypes"
        | "ElementTypes"
        | "ErrorCodes"
        | "Namespaces"
        | "NodeTypes"
        | "TS_NODE_TYPES"
        | "errorMessages"
        | "helperNameMap"
        | "locStub"
        | "forAliasRE"
        | "validFirstIdentCharRE" => Some(()),
        _ => None,
    }
}

fn alias_function_expression(
    target: TargetSpec,
    export_name: &str,
    detail: &ApiExportDetail,
) -> String {
    let name = detail.name.as_deref().unwrap_or(export_name);
    let arity = detail.function_arity.unwrap_or(0);
    let body_arity = alias_body_arity(target, export_name, arity);
    let command = bridge_command(target, export_name);
    if detail.is_class_like.unwrap_or(false) {
        let args = (0..arity)
            .map(|index| format!("a{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let mut expression = format!(
            "class {} {{ constructor({args}) {{ this.args = Array.prototype.slice.call(arguments); }} }}",
            sanitize_js_identifier(name)
        );
        expression = format!("(() => {{ const cls = {expression};");
        expression.push_str(&format!(
            " Object.defineProperty(cls, 'name', {{ value: {}, configurable: true }});",
            js_string_literal(name)
        ));
        add_static_function_props(&mut expression, detail);
        expression.push_str(" return cls; })()");
        return expression;
    }
    let args = (0..arity)
        .map(|index| format!("a{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let argument_bindings = if body_arity > arity {
        (arity..body_arity)
            .map(|index| format!("const a{index} = arguments[{index}];"))
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        String::new()
    };
    let body = match command {
        Some("vue3.core.baseCompile") => format!(
            "{argument_bindings} const __vuecPayload = normalizeArgs({}); preflightAliasCall({}, __vuecPayload); if (usesAliasRuntimeCompile(__vuecPayload.options)) return vue3CoreRuntime.baseCompile(__vuecPayload.source, __vuecPayload.options || {{}}); const __vuecResult = callBridge({}, bridgePayloadForCall(__vuecPayload)); emitVue3CompileDiagnostics(__vuecResult, __vuecPayload.options, __vuecPayload.source); return __vuecResult;",
            alias_argument_object(target, export_name, body_arity),
            js_string_literal(alias_preflight_name(target, export_name)),
            js_string_literal("vue3.core.baseCompile"),
        ),
        Some("vue3.dom.compile") => format!(
            "{argument_bindings} const __vuecPayload = normalizeArgs({}); preflightAliasCall({}, __vuecPayload); const __vuecResult = callBridge({}, bridgePayloadForCall(__vuecPayload)); emitVue3CompileDiagnostics(__vuecResult, __vuecPayload.options, __vuecPayload.source); return __vuecResult;",
            alias_argument_object(target, export_name, body_arity),
            js_string_literal(alias_preflight_name(target, export_name)),
            js_string_literal("vue3.dom.compile"),
        ),
        Some(command) => {
            let call = if matches!(
                (target.kind, export_name),
                (TargetKind::Vue3Core, "baseParse") | (TargetKind::Vue3Dom, "parse")
            ) {
                format!(
                    "hydrateVue3Ast(callBridge({}, bridgePayloadForCall(__vuecBridgePayload)), __vuecPayload.options)",
                    js_string_literal(command)
                )
            } else {
                format!(
                    "callBridge({}, bridgePayloadForCall(__vuecBridgePayload))",
                    js_string_literal(command)
                )
            };
            let is_vue3_generate = target.kind == TargetKind::Vue3Core && export_name == "generate";
            let is_vue2_template_compile = matches!(
                (target.kind, export_name),
                (
                    TargetKind::Vue26Template | TargetKind::Vue27Template,
                    "compile" | "compileToFunctions" | "ssrCompile" | "ssrCompileToFunctions"
                )
            );
            let is_vue27_sfc_compile_script =
                target.kind == TargetKind::Vue27Sfc && export_name == "compileScript";
            let is_vue27_sfc_compile_template =
                target.kind == TargetKind::Vue27Sfc && export_name == "compileTemplate";
            let is_vue3_sfc_compile_script =
                target.kind == TargetKind::Vue3Sfc && export_name == "compileScript";
            let is_vue3_sfc_compile_template =
                target.kind == TargetKind::Vue3Sfc && export_name == "compileTemplate";
            let is_vue3_sfc_parse = target.kind == TargetKind::Vue3Sfc && export_name == "parse";
            let is_vue3_ssr_compile = target.kind == TargetKind::Vue3Ssr && export_name == "compile";
            let is_sfc_compile_style = matches!(
                (target.kind, export_name),
                (
                    TargetKind::Vue3Sfc | TargetKind::Vue27Sfc,
                    "compileStyle" | "compileStyleAsync"
                )
            );
            let is_vue3_sfc_compile_style = target.kind == TargetKind::Vue3Sfc
                && matches!(export_name, "compileStyle" | "compileStyleAsync");
            let payload = if is_vue3_generate {
                "Object.assign({}, __vuecPayload, { ast: vue3CoreRuntime.dehydrateForBridge(a0), source: '' })"
            } else if is_vue3_sfc_parse {
                "vue3SfcParseBridgePayload(__vuecPayload)"
            } else if is_vue27_sfc_compile_script {
                "vue27CompileScriptBridgePayload(__vuecPayload)"
            } else if is_vue27_sfc_compile_template {
                "vue27SfcCompileTemplateBridgePayload(__vuecPayload)"
            } else if is_vue3_sfc_compile_script {
                "vue3CompileScriptBridgePayload(__vuecPayload)"
            } else if is_vue3_sfc_compile_template {
                "vue3SfcCompileTemplateBridgePayload(__vuecPayload)"
            } else if is_vue3_sfc_compile_style {
                "vue3StyleBridgePayload(__vuecPayload)"
            } else {
                "__vuecPayload"
            };
            let payload_init = if is_sfc_compile_style {
                format!(
                    "resolveStylePreprocessPayload(normalizeArgs({}))",
                    alias_argument_object(target, export_name, body_arity)
                )
            } else {
                format!(
                    "normalizeArgs({})",
                    alias_argument_object(target, export_name, body_arity)
                )
            };
            let return_expr = if is_vue3_sfc_compile_style {
                format!(
                    "(() => {{ const __vuecStyleResult = {call}; return normalizeStyleAliasResult(emitVue3StyleWarnings(__vuecStyleResult)); }})()"
                )
            } else if is_vue3_generate {
                format!(
                    "(() => {{ const __vuecGenerateResult = {call}; emitVue3CompileDiagnostics(__vuecGenerateResult, __vuecPayload.options, __vuecPayload.source); __vuecGenerateResult.ast = a0; return __vuecGenerateResult; }})()"
                )
            } else if is_vue3_sfc_parse {
                format!("hydrateVue3SfcParseResult(applyVue3SfcCustomCompilerParse({call}, __vuecPayload.source, __vuecPayload.options, __vuecPayload.filename))")
            } else if is_vue3_sfc_compile_script {
                format!("hydrateVue3CompileScriptResult({call})")
            } else if is_vue3_sfc_compile_template {
                format!("(() => {{ const __vuecCustomTemplateResult = vue3SfcCustomCompileTemplateResult(__vuecPayload); if (__vuecCustomTemplateResult !== undefined) return __vuecCustomTemplateResult; return hydrateVue3SfcCompileTemplateResult({call}); }})()")
            } else if is_vue27_sfc_compile_script {
                format!("hydrateVue27CompileScriptResult({call})")
            } else if is_vue27_sfc_compile_template {
                format!(
                    "prettifyVue27SfcTemplateResult({call}, __vuecBridgePayload.options, __vuecPayload.filename)"
                )
            } else if is_vue3_ssr_compile {
                format!("hydrateVue3SsrCompileResult({call}, __vuecPayload.options, __vuecPayload.source)")
            } else if is_vue2_template_compile {
                format!(
                    "(() => {{ const __vuecVue2Result = {call}; emitVue2CompileWarnings(__vuecVue2Result, __vuecPayload.options); return hydrateVue2CompileResult(__vuecVue2Result); }})()"
                )
            } else {
                call
            };
            format!(
                "{argument_bindings} const __vuecPayload = {payload_init}; preflightAliasCall({}, __vuecPayload); const __vuecBridgePayload = {payload}; return {return_expr};",
                js_string_literal(alias_preflight_name(target, export_name)),
            )
        }
        None => format!(
            "{argument_bindings} return notImplemented({});",
            js_string_literal(export_name)
        ),
    };
    if detail
        .own_property_names
        .iter()
        .any(|name| name == "prototype")
    {
        format!("function {name}({args}) {{ {body} }}")
    } else {
        let expression = format!("function {name}({args}) {{ {body} }}");
        format!(
            "namedArity({}, {}, {})",
            js_string_literal(name),
            arity,
            expression
        )
    }
}

fn alias_runtime_function_expression(
    runtime_object: &str,
    export_name: &str,
    detail: &ApiExportDetail,
) -> String {
    alias_runtime_function_expression_as(runtime_object, export_name, export_name, detail)
}

fn alias_runtime_function_expression_as(
    runtime_object: &str,
    runtime_key: &str,
    export_name: &str,
    detail: &ApiExportDetail,
) -> String {
    let name = detail.name.as_deref().unwrap_or(export_name);
    let arity = detail.function_arity.unwrap_or(0);
    let args = (0..arity)
        .map(|index| format!("a{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let apply_body = format!(
        "return {}[{}].apply(this, arguments);",
        runtime_object,
        js_string_literal(runtime_key)
    );
    if detail
        .own_property_names
        .iter()
        .any(|name| name == "prototype")
    {
        format!("function {name}({args}) {{ {apply_body} }}")
    } else {
        let expression = format!("function {name}({args}) {{ {apply_body} }}");
        format!(
            "namedArity({}, {}, {})",
            js_string_literal(name),
            arity,
            expression
        )
    }
}

fn alias_body_arity(target: TargetSpec, export_name: &str, arity: u32) -> u32 {
    match (target.kind, export_name) {
        (TargetKind::Vue3Core | TargetKind::Vue3Dom | TargetKind::Vue3Ssr, "baseCompile")
        | (TargetKind::Vue3Core | TargetKind::Vue3Dom | TargetKind::Vue3Ssr, "baseParse")
        | (TargetKind::Vue3Core, "generate")
        | (TargetKind::Vue3Dom, "parse")
        | (TargetKind::Vue3Core | TargetKind::Vue3Dom | TargetKind::Vue3Ssr, "compile")
        | (TargetKind::Vue3Sfc, "parse")
        | (TargetKind::Vue27Sfc, "parseComponent")
        | (TargetKind::Vue27Sfc, "rewriteDefault")
        | (TargetKind::Vue27Sfc | TargetKind::Vue3Sfc, "compileScript") => arity.max(2),
        (TargetKind::Vue27Sfc, "prefixIdentifiers") => arity.max(5),
        (TargetKind::Vue26Template | TargetKind::Vue27Template, "generateCodeFrame") => {
            arity.max(3)
        }
        _ => arity,
    }
}

fn alias_preflight_name(target: TargetSpec, export_name: &str) -> &'static str {
    match (target.kind, export_name) {
        (TargetKind::Vue3Core, "baseCompile") => "vue3.core.baseCompile",
        _ => "",
    }
}

fn add_static_function_props(source: &mut String, detail: &ApiExportDetail) {
    for prop in &detail.own_property_names {
        if matches!(prop.as_str(), "length" | "name" | "prototype") {
            continue;
        }
        source.push_str(" cls[");
        source.push_str(&js_string_literal(prop));
        source.push_str("] = ");
        source.push_str(&object_value_for_property(prop));
        source.push(';');
    }
}

fn bridge_command(target: TargetSpec, export_name: &str) -> Option<&'static str> {
    match (target.kind, export_name) {
        (TargetKind::Vue26Template | TargetKind::Vue27Template, "compile") => Some("vue2.compile"),
        (TargetKind::Vue26Template | TargetKind::Vue27Template, "compileToFunctions") => {
            Some("vue2.compileToFunctions")
        }
        (TargetKind::Vue26Template | TargetKind::Vue27Template, "ssrCompile") => {
            Some("vue2.ssrCompile")
        }
        (TargetKind::Vue26Template | TargetKind::Vue27Template, "ssrCompileToFunctions") => {
            Some("vue2.ssrCompileToFunctions")
        }
        (TargetKind::Vue26Template | TargetKind::Vue27Template, "generateCodeFrame") => {
            Some("vue2.generateCodeFrame")
        }
        (TargetKind::Vue27Sfc, "parse") => Some("sfc.vue27.parse"),
        (TargetKind::Vue27Sfc, "parseComponent") => Some("sfc.vue27.parseComponent"),
        (TargetKind::Vue27Sfc, "rewriteDefault") => Some("sfc.vue27.rewriteDefault"),
        (TargetKind::Vue27Sfc, "prefixIdentifiers") => Some("sfc.vue27.prefixIdentifiers"),
        (TargetKind::Vue3Sfc, "parse") => Some("sfc.parse"),
        (TargetKind::Vue3Sfc, "rewriteDefault") => Some("sfc.rewriteDefault"),
        (TargetKind::Vue27Sfc, "compileTemplate") => Some("sfc.vue27.compileTemplate"),
        (TargetKind::Vue3Sfc, "compileTemplate") => Some("sfc.compileTemplate"),
        (TargetKind::Vue27Sfc, "compileScript") => Some("sfc.vue27.compileScript"),
        (TargetKind::Vue3Sfc, "compileScript") => Some("sfc.compileScript"),
        (TargetKind::Vue27Sfc, "compileStyle") => Some("sfc.vue27.compileStyle"),
        (TargetKind::Vue27Sfc, "compileStyleAsync") => Some("sfc.vue27.compileStyleAsync"),
        (TargetKind::Vue3Sfc, "compileStyle") => Some("sfc.compileStyle"),
        (TargetKind::Vue3Sfc, "compileStyleAsync") => Some("sfc.compileStyleAsync"),
        (TargetKind::Vue3Core, "baseCompile") => Some("vue3.core.baseCompile"),
        (TargetKind::Vue3Core, "baseParse") => Some("vue3.core.baseParse"),
        (TargetKind::Vue3Core, "generate") => Some("vue3.core.generate"),
        (TargetKind::Vue3Dom, "compile") => Some("vue3.dom.compile"),
        (TargetKind::Vue3Dom, "parse") => Some("vue3.dom.parse"),
        (TargetKind::Vue3Ssr, "compile") => Some("vue3.ssr.compile"),
        _ => None,
    }
}

fn alias_argument_object(target: TargetSpec, export_name: &str, _arity: u32) -> String {
    match (target.kind, export_name) {
        (TargetKind::Vue26Template | TargetKind::Vue27Template, "generateCodeFrame") => {
            "{ source: a0, start: a1, end: a2 }".into()
        }
        (TargetKind::Vue26Template | TargetKind::Vue27Template, _) => {
            "{ template: a0, options: a1 }".into()
        }
        (TargetKind::Vue27Sfc, "parse") => {
            "{ source: a0 && a0.source ? a0.source : '', filename: a0 && a0.filename, options: a0 }".into()
        }
        (TargetKind::Vue27Sfc, "parseComponent") => {
            "{ source: a0 == null ? '' : String(a0), options: a1 || {} }".into()
        }
        (TargetKind::Vue27Sfc, "rewriteDefault") => {
            "{ source: a0 == null ? '' : String(a0), variable: a1 || 'script', plugins: a2 }"
                .into()
        }
        (TargetKind::Vue27Sfc, "prefixIdentifiers") => {
            "{ source: a0 == null ? '' : String(a0), isFunctional: !!a1, isTS: !!a2, babelOptions: a3 || {}, bindings: a4 || {} }"
                .into()
        }
        (TargetKind::Vue27Sfc, "compileTemplate") => {
            "{ source: a0 && a0.source ? a0.source : '', filename: a0 && (a0.filename || a0.id || 'template.vue.html'), options: a0 }"
                .into()
        }
        (TargetKind::Vue27Sfc, "compileScript") => {
            "{ source: a0 && a0.descriptor && a0.descriptor.source ? a0.descriptor.source : (a0 && a0.source ? a0.source : ''), filename: a0 && a0.descriptor && a0.descriptor.filename || (a0 && a0.filename), options: a1 || a0 }"
                .into()
        }
        (TargetKind::Vue27Sfc, "compileStyle") | (TargetKind::Vue27Sfc, "compileStyleAsync") => {
            "{ source: extractStyleSource(a0 && a0.source ? a0.source : ''), filename: a0 && a0.filename, options: a0 }"
                .into()
        }
        (TargetKind::Vue27Sfc, _) => {
            "{ source: a0 && a0.source ? a0.source : '', filename: a0 && a0.filename, options: a0 }"
                .into()
        }
        (TargetKind::Vue3Sfc, "parse") => {
            "{ source: a0, filename: a1 && a1.filename, options: a1 }".into()
        }
        (TargetKind::Vue3Sfc, "rewriteDefault") => {
            "{ source: a0 == null ? '' : String(a0), variable: a1 || 'script', plugins: a2 }"
                .into()
        }
        (TargetKind::Vue3Sfc, "compileTemplate") => {
            "{ source: a0 && a0.source ? a0.source : '', filename: a0 && (a0.filename || 'template.vue.html'), options: a0 }"
                .into()
        }
        (TargetKind::Vue3Sfc, "compileScript") => {
            "{ source: a0 && a0.descriptor && a0.descriptor.source ? a0.descriptor.source : (a0 && a0.source ? a0.source : ''), filename: a0 && a0.descriptor && a0.descriptor.filename || (a0 && a0.filename), options: a1 || a0 }"
                .into()
        }
        (TargetKind::Vue3Sfc, "compileStyle") | (TargetKind::Vue3Sfc, "compileStyleAsync") => {
            "{ source: a0 && a0.source ? a0.source : '', filename: a0 && a0.filename, options: a0 }"
                .into()
        }
        (TargetKind::Vue3Sfc, _) => {
            "{ source: a0 && a0.source ? a0.source : '', filename: a0 && a0.filename, options: a0 }"
                .into()
        }
        (TargetKind::Vue3Dom, "parse") => {
            "vue3BridgePayload(a0 && a0.source ? a0.source : a0, undefined, a1 || (a0 && a0.options) || {})"
                .into()
        }
        (TargetKind::Vue3Core, "baseCompile")
        | (TargetKind::Vue3Dom, "compile")
        | (TargetKind::Vue3Ssr, "compile") => {
            "vue3CompileBridgePayload(a0, a0 && a0.filename, a1 || (a0 && a0.options) || {})"
                .into()
        }
        (TargetKind::Vue3Core | TargetKind::Vue3Dom | TargetKind::Vue3Ssr, _) => {
            "vue3BridgePayload(a0 && a0.source ? a0.source : a0, a0 && a0.filename, a1 || (a0 && a0.options) || {})"
                .into()
        }
    }
}

fn manifest_string_value(target: TargetSpec, export_name: &str) -> String {
    if target.kind == TargetKind::Vue3Sfc && export_name == "version" {
        js_string_literal("3.5.34")
    } else {
        "''".into()
    }
}

fn object_from_property_names(properties: &[String]) -> String {
    let entries = properties
        .iter()
        .map(|prop| {
            format!(
                "{}: {}",
                js_string_literal(prop),
                object_value_for_property(prop)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("{{{entries}}}")
}

fn object_value_for_property(prop: &str) -> String {
    if prop.chars().all(|ch| ch.is_ascii_digit()) {
        prop.to_string()
    } else {
        "undefined".into()
    }
}

fn sanitize_js_identifier(name: &str) -> String {
    let mut out = String::new();
    for (index, ch) in name.chars().enumerate() {
        let valid = ch == '_' || ch == '$' || ch.is_ascii_alphanumeric();
        if !valid || (index == 0 && ch.is_ascii_digit()) {
            out.push('_');
        } else {
            out.push(ch);
        }
    }
    if out.is_empty() {
        "_VuecAlias".into()
    } else {
        out
    }
}

fn js_string_literal(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".into())
}

fn write_text(path: &Path, value: &str) -> Result<()> {
    fs::write(path, value).with_context(|| format!("failed to write {}", path.display()))
}

fn manifest_from_probe(
    target: TargetSpec,
    side: ApiManifestSide,
    lock_hash: Option<String>,
    official_revision: Option<String>,
    probe: ApiProbeOutput,
) -> ManifestFile {
    let status = match (side, probe.require.success) {
        (_, true) => "pass",
        (ApiManifestSide::Official, false) => "fail",
        (ApiManifestSide::Rust, false) => "pending",
    };
    ManifestFile {
        schema_version: 1,
        version_line: target.version_line,
        package: target.package.to_string(),
        entry: target.entry.to_string(),
        package_version: probe.package_version,
        exports: probe.exports,
        export_details: probe.export_details,
        require: probe.require,
        types: probe.types,
        status: status.to_string(),
        source: side.as_str().to_string(),
        lock_hash,
        official_revision,
    }
}

fn failed_api_manifest(
    target: TargetSpec,
    side: ApiManifestSide,
    lock_hash: Option<String>,
    official_revision: Option<String>,
    message: String,
) -> ManifestFile {
    ManifestFile {
        schema_version: 1,
        version_line: target.version_line,
        package: target.package.to_string(),
        entry: target.entry.to_string(),
        package_version: None,
        exports: Vec::new(),
        export_details: BTreeMap::new(),
        require: ApiRequireRecord {
            request: api_require_request(target),
            success: false,
            resolved: None,
            error_name: Some("XtaskError".into()),
            error_code: None,
            error_message: Some(message),
        },
        types: ApiTypesRecord::default(),
        status: if side == ApiManifestSide::Rust {
            "pending"
        } else {
            "fail"
        }
        .into(),
        source: side.as_str().to_string(),
        lock_hash,
        official_revision,
    }
}

fn manifest_status(manifest: &ManifestFile) -> ReportStatus {
    match manifest.status.as_str() {
        "pass" => ReportStatus::Pass,
        "pending" => ReportStatus::Pending,
        _ => ReportStatus::Fail,
    }
}

fn compare_api_manifests(official: &ManifestFile, rust: &ManifestFile) -> Vec<String> {
    let mut diffs = Vec::new();
    if official.version_line != rust.version_line {
        diffs.push(format!(
            "version_line differs: official={} rust={}",
            official.version_line, rust.version_line
        ));
    }
    if official.package != rust.package {
        diffs.push(format!(
            "package differs: official={} rust={}",
            official.package, rust.package
        ));
    }
    if official.entry != rust.entry {
        diffs.push(format!(
            "entry differs: official={} rust={}",
            official.entry, rust.entry
        ));
    }
    if official.require.success != rust.require.success {
        diffs.push(format!(
            "require success differs: official={} rust={}",
            official.require.success, rust.require.success
        ));
    }
    if !official.require.success {
        diffs.push(format!(
            "official manifest did not load: {}",
            official
                .require
                .error_message
                .as_deref()
                .unwrap_or("unknown error")
        ));
    }
    if !rust.require.success {
        diffs.push(format!(
            "Rust alias manifest did not load: {}",
            rust.require
                .error_message
                .as_deref()
                .unwrap_or("unknown error")
        ));
    }
    if official.package_version != rust.package_version {
        diffs.push(format!(
            "package_version differs: official={:?} rust={:?}",
            official.package_version, rust.package_version
        ));
    }
    if official.exports != rust.exports {
        diffs.push(format!(
            "exports differ: official={:?} rust={:?}",
            official.exports, rust.exports
        ));
    }
    for export_name in official
        .exports
        .iter()
        .chain(rust.exports.iter())
        .collect::<std::collections::BTreeSet<_>>()
    {
        let official_detail = official.export_details.get(export_name.as_str());
        let rust_detail = rust.export_details.get(export_name.as_str());
        if official_detail != rust_detail {
            diffs.push(format!(
                "export {export_name} detail differs: official={official_detail:?} rust={rust_detail:?}"
            ));
        }
    }
    if official.types.package_types != rust.types.package_types {
        diffs.push(format!(
            "types package path differs: official={:?} rust={:?}",
            official.types.package_types, rust.types.package_types
        ));
    }
    if official.types.exists != rust.types.exists {
        diffs.push(format!(
            "types existence differs: official={} rust={}",
            official.types.exists, rust.types.exists
        ));
    }
    diffs
}

fn load_allowed_api_diffs(path: &Path) -> AllowedApiDiffFile {
    read_json::<AllowedApiDiffFile>(path).unwrap_or_default()
}

fn is_allowed_api_diff(allowed: &AllowedApiDiffFile, target: TargetSpec, diff: &str) -> bool {
    allowed.entries.iter().any(|entry| {
        entry.version_line == target.version_line
            && entry.package == target.package
            && entry.entry == target.entry
            && entry.diff == diff
            && !entry.reason.trim().is_empty()
    })
}

fn api_require_request(target: TargetSpec) -> String {
    if target.entry == "index" {
        target.package.to_string()
    } else {
        target.entry.to_string()
    }
}

fn baseline_for(lock: &OfficialRevisionsLock, version_line: VersionLine) -> Option<&BaselineLock> {
    match version_line {
        VersionLine::Vue26 => Some(&lock.vue2_6),
        VersionLine::Vue27 => Some(&lock.vue2_7),
        VersionLine::Vue3 => Some(&lock.vue3),
    }
}
