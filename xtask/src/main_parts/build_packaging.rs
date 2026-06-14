fn write_wasm_webdriver_config(chrome_binary: Option<&Path>) -> Result<PathBuf> {
    let dir = PathBuf::from("target").join("wasm-browser");
    std::fs::create_dir_all(&dir).with_context(|| format!("failed to create {}", dir.display()))?;
    let path = dir.join("webdriver.json");
    let chrome_options = if let Some(binary) = chrome_binary {
        json!({
            "binary": binary.display().to_string(),
            "args": [
                "headless=new",
                "disable-dev-shm-usage",
                "no-sandbox"
            ]
        })
    } else {
        json!({
            "args": [
                "headless=new",
                "disable-dev-shm-usage",
                "no-sandbox"
            ]
        })
    };
    let config = json!({
        "browserName": "chrome",
        "goog:chromeOptions": chrome_options,
    });
    std::fs::write(&path, serde_json::to_vec_pretty(&config)?)
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(path)
}

fn resolve_browser_chrome_binary() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("VUEC_WASM_CHROME") {
        let path = PathBuf::from(path);
        if path.exists() {
            return Some(path);
        }
    }
    if cfg!(windows) {
        return chrome_candidate_paths()
            .into_iter()
            .find(|path| path.exists());
    }
    chrome_candidate_paths()
        .into_iter()
        .find(|path| is_real_chrome_binary(path))
}

fn resolve_browser_chromedriver() -> Option<PathBuf> {
    let path = std::env::var_os("VUEC_WASM_CHROMEDRIVER").map(PathBuf::from)?;
    if path.exists() {
        Some(path)
    } else {
        None
    }
}

fn chrome_candidate_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if cfg!(windows) {
        paths.push(PathBuf::from(
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
        ));
        paths.push(PathBuf::from(
            r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
        ));
    } else if cfg!(target_os = "macos") {
        paths.push(PathBuf::from(
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        ));
    } else {
        for program in [
            "google-chrome",
            "google-chrome-stable",
            "chromium",
            "chromium-browser",
        ] {
            if let Ok(path) = resolve_program(program) {
                paths.push(PathBuf::from(path));
            }
        }
    }
    paths
}

fn is_real_chrome_binary(path: &Path) -> bool {
    if !path.exists() {
        return false;
    }
    let output = ProcessCommand::new(path).arg("--version").output();
    output
        .ok()
        .filter(|output| output.status.success())
        .map(|output| {
            let text = normalize_command_output(&output.stdout, &output.stderr);
            text.to_ascii_lowercase().contains("chrome")
                || text.to_ascii_lowercase().contains("chromium")
        })
        .unwrap_or(false)
}

fn build_wasm_package() -> Result<Vec<PathBuf>> {
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
    let pkg_dir = PathBuf::from("packages").join("wasm").join("pkg-node");
    if pkg_dir.exists() {
        std::fs::remove_dir_all(&pkg_dir)
            .with_context(|| format!("failed to remove {}", pkg_dir.display()))?;
    }
    std::fs::create_dir_all(&pkg_dir)
        .with_context(|| format!("failed to create {}", pkg_dir.display()))?;
    let output = ProcessCommand::new("cargo")
        .args([
            "build",
            "-p",
            "vuec_wasm",
            "--target",
            "wasm32-unknown-unknown",
        ])
        .output()
        .context("failed to spawn cargo build -p vuec_wasm --target wasm32-unknown-unknown")?;
    if !output.status.success() {
        anyhow::bail!(
            "cargo build -p vuec_wasm --target wasm32-unknown-unknown exited with {:?}\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let wasm_path = PathBuf::from("target")
        .join("wasm32-unknown-unknown")
        .join("debug")
        .join("vuec_wasm.wasm");
    let output = ProcessCommand::new(wasm_bindgen)
        .args([
            "--target",
            "nodejs",
            "--out-dir",
            &pkg_dir.display().to_string(),
            &wasm_path.display().to_string(),
        ])
        .output()
        .context("failed to spawn wasm-bindgen")?;
    if !output.status.success() {
        anyhow::bail!(
            "wasm-bindgen exited with {:?}\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    std::fs::write(pkg_dir.join("package.json"), r#"{"type":"commonjs"}"#)
        .with_context(|| format!("failed to write {}", pkg_dir.join("package.json").display()))?;
    Ok(vec![pkg_dir])
}

fn installed_rust_targets() -> Result<Vec<String>> {
    let output = ProcessCommand::new("rustup")
        .args(["target", "list", "--installed"])
        .output()
        .context("failed to spawn rustup target list --installed")?;
    if !output.status.success() {
        anyhow::bail!(
            "rustup target list --installed exited with {:?}\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect())
}

fn resolve_program(program: &str) -> Result<String> {
    let check = if cfg!(windows) { "where" } else { "which" };
    let output = ProcessCommand::new(check)
        .arg(program)
        .output()
        .with_context(|| format!("failed to spawn {check} {program}"))?;
    if !output.status.success() {
        anyhow::bail!("required program `{program}` was not found on PATH");
    }
    let candidates = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if cfg!(windows) {
        if let Some(path) = candidates.iter().find(|path| is_windows_executable(path)) {
            return Ok(path.clone());
        }
    }
    Ok(candidates
        .into_iter()
        .next()
        .unwrap_or_else(|| program.to_string()))
}

fn is_windows_executable(path: &str) -> bool {
    matches!(
        Path::new(path)
            .extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("exe" | "cmd" | "bat" | "com")
    )
}

fn run_wasm_smoke() -> Result<String> {
    let pkg_path = absolute_path(&PathBuf::from("packages/wasm/pkg-node/vuec_wasm.js"));
    let output = ProcessCommand::new("node")
        .arg("smoke.js")
        .env("VUEC_WASM_PKG", pkg_path)
        .current_dir("packages/wasm")
        .output()
        .context("failed to spawn @vuec-rs/wasm smoke")?;
    if !output.status.success() {
        anyhow::bail!(
            "node WASM smoke exited with {:?}\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn normalize_command_output(stdout: &[u8], stderr: &[u8]) -> String {
    let mut text = String::new();
    text.push_str(String::from_utf8_lossy(stdout).trim());
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

fn build_napi_crate() -> Result<()> {
    let output = ProcessCommand::new("cargo")
        .args(["build", "-p", "vuec_napi"])
        .output()
        .context("failed to spawn cargo build -p vuec_napi")?;
    if !output.status.success() {
        anyhow::bail!(
            "cargo build -p vuec_napi exited with {:?}\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

fn build_napi_crate_release() -> Result<()> {
    let output = ProcessCommand::new("cargo")
        .args(["build", "-p", "vuec_napi", "--release"])
        .output()
        .context("failed to spawn cargo build -p vuec_napi --release")?;
    if !output.status.success() {
        anyhow::bail!(
            "cargo build -p vuec_napi --release exited with {:?}\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

fn copy_napi_binding(target_path: &Path) -> Result<PathBuf> {
    let source_path = napi_library_path();
    copy_napi_binding_from(&source_path, target_path)
}

fn copy_napi_binding_from(source_path: &Path, target_path: &Path) -> Result<PathBuf> {
    let parent = target_path
        .parent()
        .context("NAPI target path has no parent directory")?;
    std::fs::create_dir_all(parent)
        .with_context(|| format!("failed to create {}", parent.display()))?;
    std::fs::copy(&source_path, target_path).with_context(|| {
        format!(
            "failed to copy {} to {}",
            source_path.display(),
            target_path.display()
        )
    })?;
    Ok(target_path.to_path_buf())
}

fn prepare_napi_alias_tree(alias_root: &Path) -> Result<Vec<PathBuf>> {
    ensure_target_child(alias_root, "napi-alias")?;
    let node_modules = alias_root.join("node_modules");
    if alias_root.exists() {
        std::fs::remove_dir_all(alias_root)
            .with_context(|| format!("failed to remove {}", alias_root.display()))?;
    }
    std::fs::create_dir_all(&node_modules)
        .with_context(|| format!("failed to create {}", node_modules.display()))?;

    let mut created = Vec::new();
    let native_target = node_modules.join("@vuec-rs").join("native");
    copy_dir_recursive(Path::new("packages/native"), &native_target)?;
    created.push(native_target);

    for (source, target) in [
        (
            PathBuf::from("packages/native-aliases/vue-template-compiler"),
            node_modules.join("vue-template-compiler"),
        ),
        (
            PathBuf::from("packages/native-aliases/vue"),
            node_modules.join("vue"),
        ),
        (
            PathBuf::from("packages/native-aliases/@vue/compiler-core"),
            node_modules.join("@vue").join("compiler-core"),
        ),
        (
            PathBuf::from("packages/native-aliases/@vue/compiler-dom"),
            node_modules.join("@vue").join("compiler-dom"),
        ),
        (
            PathBuf::from("packages/native-aliases/@vue/compiler-ssr"),
            node_modules.join("@vue").join("compiler-ssr"),
        ),
        (
            PathBuf::from("packages/native-aliases/@vue/compiler-sfc"),
            node_modules.join("@vue").join("compiler-sfc"),
        ),
    ] {
        copy_dir_recursive(&source, &target)?;
        created.push(target);
    }

    std::fs::copy(
        Path::new("packages/native-aliases/smoke.js"),
        alias_root.join("smoke.js"),
    )
    .context("failed to copy NAPI alias smoke script")?;
    created.push(alias_root.join("smoke.js"));
    Ok(created)
}

fn prepare_napi_api_tree(root: &Path, target: NapiApiTarget) -> Result<Vec<PathBuf>> {
    ensure_nested_target_child(
        root,
        &["napi-api", target.version_line, target.target_dir_name()],
    )?;
    let node_modules = root.join("node_modules");
    if root.exists() {
        std::fs::remove_dir_all(root)
            .with_context(|| format!("failed to remove {}", root.display()))?;
    }
    std::fs::create_dir_all(&node_modules)
        .with_context(|| format!("failed to create {}", node_modules.display()))?;

    let mut created = Vec::new();
    let native_target = node_modules.join("@vuec-rs").join("native");
    copy_dir_recursive(Path::new("packages/native"), &native_target)?;
    created.push(native_target);

    let package_target = join_path_segments(&node_modules, target.package_subpath());
    copy_dir_recursive(&target.source_path(), &package_target)?;
    if matches!(
        target.package,
        "@vue/compiler-dom" | "@vue/compiler-ssr" | "@vue/compiler-sfc"
    ) {
        let core_target = node_modules.join("@vue").join("compiler-core");
        copy_dir_recursive(
            Path::new("packages/native-aliases/@vue/compiler-core"),
            &core_target,
        )?;
        created.push(core_target);
    }
    if let NapiApiAlias::Vue2TemplateCompiler { template_variant } = target.alias {
        std::fs::copy(
            package_target.join(format!("index-{template_variant}.js")),
            package_target.join("index.js"),
        )
        .with_context(|| {
            format!("failed to select {template_variant} vue-template-compiler alias")
        })?;
    }
    let official = read_json_file(&target.official_manifest_path())?;
    let package_json_path =
        join_path_segments(&node_modules, target.package_json_subpath()).join("package.json");
    write_package_version(
        &package_json_path,
        official
            .get("package_version")
            .and_then(JsonValue::as_str)
            .unwrap_or("0.0.0"),
    )?;
    created.push(package_target);
    Ok(created)
}

fn prepare_napi_platform_tree(platform_root: &Path, package_name: &str) -> Result<Vec<PathBuf>> {
    ensure_target_child(platform_root, "napi-platform")?;
    let node_modules = platform_root.join("node_modules");
    if platform_root.exists() {
        std::fs::remove_dir_all(platform_root)
            .with_context(|| format!("failed to remove {}", platform_root.display()))?;
    }
    std::fs::create_dir_all(&node_modules)
        .with_context(|| format!("failed to create {}", node_modules.display()))?;

    let mut created = Vec::new();
    let native_target = node_modules.join("@vuec-rs").join("native");
    copy_dir_recursive(Path::new("packages/native"), &native_target)?;
    let local_binding = native_target.join("vuec_napi.node");
    if local_binding.exists() {
        std::fs::remove_file(&local_binding)
            .with_context(|| format!("failed to remove {}", local_binding.display()))?;
    }
    created.push(native_target);

    let package_source = platform_template_dir(package_name)?;
    let package_target = platform_package_path(&node_modules, package_name);
    copy_dir_recursive(&package_source, &package_target)?;
    created.push(package_target);
    Ok(created)
}

fn ensure_target_child(path: &Path, child: &str) -> Result<()> {
    ensure_nested_target_child(path, &[child])
}

fn ensure_nested_target_child(path: &Path, children: &[&str]) -> Result<()> {
    let cwd = std::env::current_dir().context("failed to resolve current directory")?;
    let mut expected = cwd.join("target");
    for child in children {
        expected = expected.join(child);
    }
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    };
    if absolute != expected {
        anyhow::bail!(
            "refusing to recursively replace {}; expected {}",
            absolute.display(),
            expected.display()
        );
    }
    Ok(())
}

fn current_platform_package_name() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => Some("@vuec-rs/native-win32-x64"),
        ("windows", "aarch64") => Some("@vuec-rs/native-win32-arm64"),
        ("macos", "x86_64") => Some("@vuec-rs/native-darwin-x64"),
        ("macos", "aarch64") => Some("@vuec-rs/native-darwin-arm64"),
        ("linux", "x86_64") if cfg!(target_env = "musl") => Some("@vuec-rs/native-linux-x64-musl"),
        ("linux", "x86_64") => Some("@vuec-rs/native-linux-x64-gnu"),
        ("linux", "aarch64") if cfg!(target_env = "musl") => {
            Some("@vuec-rs/native-linux-arm64-musl")
        }
        ("linux", "aarch64") => Some("@vuec-rs/native-linux-arm64-gnu"),
        _ => None,
    }
}

fn platform_template_dir(package_name: &str) -> Result<PathBuf> {
    let suffix = package_name
        .strip_prefix("@vuec-rs/native-")
        .with_context(|| format!("unsupported platform package name {package_name}"))?;
    Ok(PathBuf::from("packages")
        .join("native-platforms")
        .join(suffix))
}

fn platform_package_path(node_modules: &Path, package_name: &str) -> PathBuf {
    let Some((scope, name)) = package_name.split_once('/') else {
        return node_modules.join(package_name);
    };
    node_modules.join(scope).join(name)
}

fn copy_dir_recursive(source: &Path, target: &Path) -> Result<()> {
    std::fs::create_dir_all(target)
        .with_context(|| format!("failed to create {}", target.display()))?;
    for entry in
        std::fs::read_dir(source).with_context(|| format!("failed to read {}", source.display()))?
    {
        let entry = entry?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir_recursive(&source_path, &target_path)?;
        } else {
            std::fs::copy(&source_path, &target_path).with_context(|| {
                format!(
                    "failed to copy {} to {}",
                    source_path.display(),
                    target_path.display()
                )
            })?;
        }
    }
    Ok(())
}

fn join_path_segments(base: &Path, segments: &[&str]) -> PathBuf {
    segments
        .iter()
        .fold(base.to_path_buf(), |path, segment| path.join(segment))
}

fn read_json_file(path: &Path) -> Result<JsonValue> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("failed to parse {}", path.display()))
}

fn write_package_version(path: &Path, version: &str) -> Result<()> {
    let mut value = read_json_file(path)?;
    value["version"] = JsonValue::String(version.to_string());
    std::fs::write(path, serde_json::to_string_pretty(&value)?)
        .with_context(|| format!("failed to write {}", path.display()))
}

fn napi_library_path() -> PathBuf {
    napi_library_path_for_profile("debug")
}

fn napi_release_library_path() -> PathBuf {
    napi_library_path_for_profile("release")
}

fn napi_library_path_for_profile(profile: &str) -> PathBuf {
    let (prefix, suffix) = match std::env::consts::OS {
        "windows" => ("", ".dll"),
        "macos" => ("lib", ".dylib"),
        _ => ("lib", ".so"),
    };
    PathBuf::from("target")
        .join(profile)
        .join(format!("{prefix}vuec_napi{suffix}"))
}

fn run_native_smoke(package_dir: &Path) -> Result<String> {
    let output = ProcessCommand::new("node")
        .arg("smoke.js")
        .current_dir(package_dir)
        .output()
        .with_context(|| format!("failed to spawn node smoke in {}", package_dir.display()))?;
    if !output.status.success() {
        anyhow::bail!(
            "node smoke exited with {:?}\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn run_napi_alias_smoke(alias_root: &Path) -> Result<String> {
    let output = ProcessCommand::new("node")
        .arg("smoke.js")
        .current_dir(alias_root)
        .output()
        .with_context(|| format!("failed to spawn node smoke in {}", alias_root.display()))?;
    if !output.status.success() {
        anyhow::bail!(
            "node NAPI alias smoke exited with {:?}\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn run_napi_platform_smoke(platform_root: &Path) -> Result<String> {
    let platform_root = absolute_path(platform_root);
    let script = r##"
const path = require('path');
const { createRequire } = require('module');
const rootRequire = createRequire(path.join(process.env.VUEC_NAPI_PLATFORM_ROOT, 'package.json'));
const native = rootRequire('@vuec-rs/native');
const info = native.bindingInfo();
if (info.source !== 'platform') {
  throw new Error(`expected platform binding source, got ${JSON.stringify(info)}`);
}
const result = native.compileDom('<div>{{ msg }}</div>', { mode: 'module', prefixIdentifiers: true });
if (!result || !/_ctx\.msg/.test(result.code)) {
  throw new Error('platform package compile smoke failed');
}
process.stdout.write(JSON.stringify({ status: 'pass', binding: info }));
"##;
    let output = ProcessCommand::new("node")
        .arg("-e")
        .arg(script)
        .env("VUEC_NAPI_PLATFORM_ROOT", &platform_root)
        .output()
        .with_context(|| {
            format!(
                "failed to spawn node platform smoke in {}",
                platform_root.display()
            )
        })?;
    if !output.status.success() {
        anyhow::bail!(
            "node NAPI platform smoke exited with {:?}\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn run_napi_api_probe(root: &Path, target: NapiApiTarget) -> Result<String> {
    let root = absolute_path(root);
    let official_path = absolute_path(&target.official_manifest_path());
    let package_json_path =
        join_path_segments(&root.join("node_modules"), target.package_json_subpath())
            .join("package.json");
    let types_base_path =
        join_path_segments(&root.join("node_modules"), target.types_base_subpath());
    let script = r##"
const fs = require('fs');
const path = require('path');
const { createRequire } = require('module');

const root = process.env.VUEC_NAPI_API_ROOT;
const official = JSON.parse(fs.readFileSync(process.env.VUEC_NAPI_API_OFFICIAL, 'utf8'));
const request = process.env.VUEC_NAPI_API_REQUEST;
const rootRequire = createRequire(path.join(root, 'package.json'));
const api = rootRequire(request);
const resolved = rootRequire.resolve(request);
const packageJson = JSON.parse(fs.readFileSync(process.env.VUEC_NAPI_API_PACKAGE_JSON, 'utf8'));
const typesBase = process.env.VUEC_NAPI_API_TYPES_BASE;

function describeExport(value) {
  const detail = {
    kind: typeof value,
    tag: Object.prototype.toString.call(value),
    name: typeof value === 'function' ? value.name : null,
    function_arity: typeof value === 'function' ? value.length : null,
    is_async_function: typeof value === 'function' ? value.constructor && value.constructor.name === 'AsyncFunction' : null,
    is_class_like: typeof value === 'function' ? /^class\s/.test(Function.prototype.toString.call(value)) : null,
    own_property_names: Object.getOwnPropertyNames(value).sort(),
  };
  if (typeof value === 'symbol') {
    detail.own_property_names = [];
  }
  return detail;
}

const manifest = {
  package_version: packageJson.version,
  exports: Object.keys(api).sort(),
  export_details: {},
  require: {
    request,
    success: true,
    resolved,
    error_name: null,
    error_code: null,
    error_message: null,
  },
  types: {
    package_types: packageJson.types || null,
    exists: fs.existsSync(path.join(typesBase, packageJson.types || '')),
  },
};
for (const key of manifest.exports) {
  manifest.export_details[key] = describeExport(api[key]);
}

const diffs = [];
for (const field of ['package_version', 'exports']) {
  if (JSON.stringify(official[field]) !== JSON.stringify(manifest[field])) {
    diffs.push(`${field} differs: official=${JSON.stringify(official[field])} napi=${JSON.stringify(manifest[field])}`);
  }
}
for (const name of Array.from(new Set([...official.exports, ...manifest.exports])).sort()) {
  if (JSON.stringify(official.export_details[name]) !== JSON.stringify(manifest.export_details[name])) {
    diffs.push(`export ${name} detail differs: official=${JSON.stringify(official.export_details[name])} napi=${JSON.stringify(manifest.export_details[name])}`);
  }
}
if (official.types.package_types !== manifest.types.package_types) {
  diffs.push(`types package path differs: official=${JSON.stringify(official.types.package_types)} napi=${JSON.stringify(manifest.types.package_types)}`);
}
if (official.types.exists !== manifest.types.exists) {
  diffs.push(`types existence differs: official=${official.types.exists} napi=${manifest.types.exists}`);
}
if (diffs.length) {
  throw new Error(diffs.join('\n'));
}
process.stdout.write(JSON.stringify({ status: 'pass', exports: manifest.exports, version: manifest.package_version }));
"##;
    let output = ProcessCommand::new("node")
        .arg("-e")
        .arg(script)
        .env("VUEC_NAPI_API_ROOT", &root)
        .env("VUEC_NAPI_API_OFFICIAL", &official_path)
        .env("VUEC_NAPI_API_REQUEST", target.package)
        .env("VUEC_NAPI_API_PACKAGE_JSON", &package_json_path)
        .env("VUEC_NAPI_API_TYPES_BASE", &types_base_path)
        .output()
        .with_context(|| {
            format!(
                "failed to spawn node NAPI API probe for {}",
                target.display()
            )
        })?;
    if !output.status.success() {
        anyhow::bail!(
            "node NAPI API probe failed for {} with status {:?}\nstdout:\n{}\nstderr:\n{}",
            target.display(),
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn absolute_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(path))
        .unwrap_or_else(|_| path.to_path_buf())
}
