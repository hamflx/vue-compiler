#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum BenchCaseKind {
    Vue2Template,
    Vue3Template,
    Vue3Sfc,
    Vue3Ssr,
}

impl BenchCaseKind {
    const fn cli_target(self) -> &'static str {
        match self {
            BenchCaseKind::Vue2Template => "vue2-template",
            BenchCaseKind::Vue3Template => "vue3-template",
            BenchCaseKind::Vue3Sfc => "vue3-sfc",
            BenchCaseKind::Vue3Ssr => "vue3-ssr",
        }
    }

    const fn official_package(self) -> &'static str {
        match self {
            BenchCaseKind::Vue2Template => "vue-template-compiler",
            BenchCaseKind::Vue3Template => "@vue/compiler-dom",
            BenchCaseKind::Vue3Sfc => "@vue/compiler-sfc",
            BenchCaseKind::Vue3Ssr => "@vue/compiler-ssr",
        }
    }
}

#[derive(Clone, Debug)]
struct BenchFixture {
    name: &'static str,
    kind: BenchCaseKind,
    path: PathBuf,
    bytes: u64,
    sha256: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BenchFixtureReport {
    name: String,
    kind: BenchCaseKind,
    path: String,
    bytes: u64,
    sha256: String,
}

impl From<&BenchFixture> for BenchFixtureReport {
    fn from(value: &BenchFixture) -> Self {
        Self {
            name: value.name.into(),
            kind: value.kind,
            path: value.path.display().to_string(),
            bytes: value.bytes,
            sha256: value.sha256.clone(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BenchResult {
    name: String,
    backend: String,
    package: String,
    iterations: usize,
    elapsed_micros: u128,
    micros_per_iteration: u128,
    peak_rss_bytes: Option<u64>,
    input_sha256: String,
    output_bytes: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BenchEnvironment {
    git_commit: Option<String>,
    git_dirty: bool,
    rustc: Option<String>,
    node: Option<String>,
    npm: Option<String>,
    pnpm: Option<String>,
    os: String,
    arch: String,
    lock_path: String,
    lock_hash: Option<String>,
    created_unix: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BenchReport {
    status: String,
    iterations: usize,
    environment: BenchEnvironment,
    fixtures: Vec<BenchFixtureReport>,
    results: Vec<BenchResult>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum CompileScriptProfileVersion {
    Vue27,
    Vue3,
}

impl CompileScriptProfileVersion {
    fn parse(value: &str) -> Result<Self> {
        match value {
            "vue2_7" | "vue27" => Ok(Self::Vue27),
            "vue3" => Ok(Self::Vue3),
            _ => anyhow::bail!("unsupported compileScript profile version-line {value}"),
        }
    }

    const fn canonical(self) -> &'static str {
        match self {
            Self::Vue27 => "vue2_7",
            Self::Vue3 => "vue3",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum CompileScriptProfileAstMode {
    None,
    TopLevel,
    Full,
}

impl From<CompileScriptProfileAstMode> for vuec_sfc::SfcScriptAstMode {
    fn from(value: CompileScriptProfileAstMode) -> Self {
        match value {
            CompileScriptProfileAstMode::None => Self::None,
            CompileScriptProfileAstMode::TopLevel => Self::TopLevel,
            CompileScriptProfileAstMode::Full => Self::Full,
        }
    }
}

#[derive(Clone, Debug)]
struct CompileScriptProfileFixture {
    name: String,
    path: PathBuf,
    source: String,
    sha256: String,
    source_bytes: usize,
    template_bytes: usize,
    script_bytes: usize,
    script_setup_bytes: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompileScriptProfileFixtureReport {
    name: String,
    path: String,
    source_bytes: usize,
    template_bytes: usize,
    script_bytes: usize,
    script_setup_bytes: usize,
    sha256: String,
}

impl From<&CompileScriptProfileFixture> for CompileScriptProfileFixtureReport {
    fn from(value: &CompileScriptProfileFixture) -> Self {
        Self {
            name: value.name.clone(),
            path: value.path.display().to_string(),
            source_bytes: value.source_bytes,
            template_bytes: value.template_bytes,
            script_bytes: value.script_bytes,
            script_setup_bytes: value.script_setup_bytes,
            sha256: value.sha256.clone(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompileScriptStructuralCounts {
    ast_projection_enabled: bool,
    ast_projection_mode: String,
    ast_projection_loc_strategy: String,
    ast_projection_statement_count: usize,
    template_usage_scan_count: usize,
    setup_analysis_count: usize,
    script_compile_error_analysis_count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompileScriptPhaseProfile {
    median_micros: u128,
    p95_micros: u128,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompileScriptProfileResult {
    name: String,
    version_line: String,
    iterations: usize,
    parse: CompileScriptPhaseProfile,
    compile_script: CompileScriptPhaseProfile,
    serialize: CompileScriptPhaseProfile,
    total: CompileScriptPhaseProfile,
    output_bytes: usize,
    errors: usize,
    warnings: usize,
    structural_counts: CompileScriptStructuralCounts,
    input_sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompileScriptProfileReport {
    status: String,
    version_line: String,
    iterations: usize,
    build_profile: String,
    script_ast_mode: String,
    environment: BenchEnvironment,
    fixtures: Vec<CompileScriptProfileFixtureReport>,
    results: Vec<CompileScriptProfileResult>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompileScriptProfileComparison {
    name: String,
    version_line: String,
    input_sha256: String,
    full_compile_median_micros: u128,
    top_level_compile_median_micros: u128,
    none_compile_median_micros: u128,
    full_to_none_compile_ratio: f64,
    full_to_top_level_compile_ratio: f64,
    none_compile_improvement_percent: f64,
    top_level_compile_improvement_percent: f64,
    full_serialize_median_micros: u128,
    none_serialize_median_micros: u128,
    full_to_none_serialize_ratio: f64,
    full_total_median_micros: u128,
    none_total_median_micros: u128,
    full_to_none_total_ratio: f64,
    ast_projection_statement_count: usize,
    template_usage_scan_count: usize,
    setup_analysis_count: usize,
    ast_projection_problem_confirmed: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompileScriptProfileComparisonReport {
    status: String,
    version_line: String,
    build_profile: String,
    iterations: usize,
    min_full_to_none_compile_ratio: f64,
    full_report: String,
    top_level_report: String,
    none_report: String,
    comparisons: Vec<CompileScriptProfileComparison>,
}

fn write_bench_fixtures(root: &Path) -> Result<Vec<BenchFixture>> {
    let specs = [
        (
            "vue2-template",
            BenchCaseKind::Vue2Template,
            "vue2-template.html",
            bench_template_source(),
        ),
        (
            "vue3-template",
            BenchCaseKind::Vue3Template,
            "vue3-template.html",
            bench_template_source(),
        ),
        (
            "vue3-sfc",
            BenchCaseKind::Vue3Sfc,
            "App.vue",
            bench_sfc_source(),
        ),
        (
            "vue3-ssr",
            BenchCaseKind::Vue3Ssr,
            "vue3-ssr.html",
            bench_template_source(),
        ),
    ];
    specs
        .into_iter()
        .map(|(name, kind, file_name, source)| {
            let path = root.join(file_name);
            fs::write(&path, source)
                .with_context(|| format!("failed to write {}", path.display()))?;
            let bytes = source.len() as u64;
            let sha256 = sha256_bytes(source.as_bytes());
            Ok(BenchFixture {
                name,
                kind,
                path,
                bytes,
                sha256,
            })
        })
        .collect()
}

fn bench_template_source() -> &'static str {
    r#"<section class="bench-root">
  <header>
    <h1>{{ title }}</h1>
    <button @click="refresh">Refresh</button>
  </header>
  <ul>
    <li v-for="item in items" :key="item.id" :class="{ active: item.active }">
      <span>{{ item.name }}</span>
      <strong v-if="item.count > 0">{{ item.count }}</strong>
      <em v-else>empty</em>
    </li>
  </ul>
  <footer v-show="ready" :data-total="items.length">{{ footer }}</footer>
</section>"#
}

fn bench_sfc_source() -> &'static str {
    r#"<template>
  <section class="bench-root">
    <header>
      <h1>{{ title }}</h1>
      <button @click="refresh">Refresh</button>
    </header>
    <ul>
      <li v-for="item in items" :key="item.id" :class="{ active: item.active }">
        <span>{{ item.name }}</span>
        <strong v-if="item.count > 0">{{ item.count }}</strong>
        <em v-else>empty</em>
      </li>
    </ul>
    <footer v-show="ready" :data-total="items.length">{{ footer }}</footer>
  </section>
</template>
<script setup>
const title = 'Benchmark'
const footer = 'done'
const ready = true
const items = []
function refresh() {}
</script>
<style scoped>
.bench-root { display: grid; gap: 8px; }
.active { font-weight: 600; }
</style>"#
}

fn run_rust_bench_case(fixture: &BenchFixture, iterations: usize) -> Result<BenchResult> {
    let exe = cli_executable_path();
    let output = run_monitored_command({
        let mut command = ProcessCommand::new(&exe);
        command.args([
            "bench",
            "--target",
            fixture.kind.cli_target(),
            "--iterations",
            &iterations.to_string(),
            "--json",
            &fixture.path.display().to_string(),
        ]);
        command
    })
    .with_context(|| format!("failed to run vuec bench {}", fixture.name))?;
    let value = parse_monitored_json(fixture.name, &output)?;
    let elapsed = value
        .get("elapsedMicros")
        .and_then(JsonValue::as_u64)
        .context("Rust bench result missing elapsedMicros")? as u128;
    let per_iter = value
        .get("microsPerIteration")
        .and_then(JsonValue::as_u64)
        .context("Rust bench result missing microsPerIteration")? as u128;
    Ok(BenchResult {
        name: fixture.name.into(),
        backend: "rust-cli".into(),
        package: "vuec_cli".into(),
        iterations,
        elapsed_micros: elapsed,
        micros_per_iteration: per_iter,
        peak_rss_bytes: output.peak_rss_bytes,
        input_sha256: fixture.sha256.clone(),
        output_bytes: None,
    })
}

fn prepare_official_js_bench_root(out_dir: &Path, lock: &Path) -> Result<PathBuf> {
    let root = out_dir.join("official-js");
    fs::create_dir_all(&root).with_context(|| format!("failed to create {}", root.display()))?;
    let package_json = root.join("package.json");
    if !package_json.exists() {
        let versions = official_npm_versions(lock)?;
        let value = json!({
            "private": true,
            "type": "commonjs",
            "dependencies": {
                "vue": versions.vue2,
                "vue-template-compiler": versions.vue_template_compiler,
                "@vue/compiler-dom": versions.vue_compiler_dom,
                "@vue/compiler-sfc": versions.vue_compiler_sfc,
                "@vue/compiler-ssr": versions.vue_compiler_ssr
            }
        });
        fs::write(&package_json, serde_json::to_string_pretty(&value)?)
            .with_context(|| format!("failed to write {}", package_json.display()))?;
    }
    let node_modules = root.join("node_modules");
    if !node_modules.exists() {
        let npm = resolve_program("npm")?;
        let output = ProcessCommand::new(npm)
            .args(["install", "--ignore-scripts", "--no-audit", "--no-fund"])
            .current_dir(&root)
            .output()
            .with_context(|| format!("failed to spawn npm install in {}", root.display()))?;
        if !output.status.success() {
            anyhow::bail!(
                "npm install for official JS benchmark exited with {:?}\nstdout:\n{}\nstderr:\n{}",
                output.status.code(),
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
    Ok(root)
}

#[derive(Clone, Debug)]
struct OfficialNpmVersions {
    vue2: String,
    vue_template_compiler: String,
    vue_compiler_dom: String,
    vue_compiler_sfc: String,
    vue_compiler_ssr: String,
}

fn official_npm_versions(lock: &Path) -> Result<OfficialNpmVersions> {
    let value: toml::Value = toml::from_str(
        &fs::read_to_string(lock).with_context(|| format!("failed to read {}", lock.display()))?,
    )
    .with_context(|| format!("failed to parse {}", lock.display()))?;
    Ok(OfficialNpmVersions {
        vue2: toml_string(&value, &["vue2_7", "npm", "vue"])?,
        vue_template_compiler: toml_string(&value, &["vue2_7", "npm", "vue-template-compiler"])?,
        vue_compiler_dom: toml_string(&value, &["vue3", "npm", "@vue/compiler-dom"])?,
        vue_compiler_sfc: toml_string(&value, &["vue3", "npm", "@vue/compiler-sfc"])?,
        vue_compiler_ssr: toml_string(&value, &["vue3", "npm", "@vue/compiler-ssr"])?,
    })
}

fn toml_string(value: &toml::Value, path: &[&str]) -> Result<String> {
    let mut cursor = value;
    for segment in path {
        cursor = cursor
            .get(*segment)
            .with_context(|| format!("{} missing in official lock", path.join(".")))?;
    }
    cursor
        .as_str()
        .map(ToOwned::to_owned)
        .with_context(|| format!("{} was not a string", path.join(".")))
}

fn run_official_js_bench_case(
    root: &Path,
    fixture: &BenchFixture,
    iterations: usize,
) -> Result<BenchResult> {
    let script = r##"
const fs = require('fs');
const source = fs.readFileSync(process.env.VUEC_BENCH_INPUT, 'utf8');
const iterations = Number(process.env.VUEC_BENCH_ITERATIONS);
const kind = process.env.VUEC_BENCH_KIND;
let output = null;
const started = process.hrtime.bigint();
for (let i = 0; i < iterations; i++) {
  if (kind === 'vue2-template') {
    output = require('vue-template-compiler').compile(source);
  } else if (kind === 'vue3-template') {
    output = require('@vue/compiler-dom').compile(source);
  } else if (kind === 'vue3-sfc') {
    const sfc = require('@vue/compiler-sfc');
    const parsed = sfc.parse(source, { filename: process.env.VUEC_BENCH_INPUT }).descriptor;
    output = sfc.compileTemplate({ source: parsed.template ? parsed.template.content : '', filename: process.env.VUEC_BENCH_INPUT, id: 'bench' });
  } else if (kind === 'vue3-ssr') {
    output = require('@vue/compiler-ssr').compile(source);
  } else {
    throw new Error(`unsupported benchmark kind ${kind}`);
  }
}
const elapsedMicros = Number((process.hrtime.bigint() - started) / 1000n);
function outputSize(value) {
  if (!value) return 0;
  if (typeof value.code === 'string') return Buffer.byteLength(value.code);
  if (typeof value.render === 'string') return Buffer.byteLength(value.render);
  try {
    return Buffer.byteLength(JSON.stringify(value));
  } catch {
    return 0;
  }
}
process.stdout.write(JSON.stringify({
  elapsedMicros,
  microsPerIteration: Math.floor(elapsedMicros / iterations),
  outputBytes: outputSize(output)
}));
"##;
    let output = run_monitored_command({
        let mut command = ProcessCommand::new("node");
        command
            .arg("-e")
            .arg(script)
            .current_dir(root)
            .env("VUEC_BENCH_INPUT", absolute_path(&fixture.path))
            .env("VUEC_BENCH_ITERATIONS", iterations.to_string())
            .env("VUEC_BENCH_KIND", fixture.kind.cli_target());
        command
    })
    .with_context(|| {
        format!(
            "failed to spawn official JS benchmark {} in {}",
            fixture.name,
            root.display()
        )
    })?;
    if !output.status.success() {
        anyhow::bail!(
            "official JS benchmark {} exited with {:?}\nstdout:\n{}\nstderr:\n{}",
            fixture.name,
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let value: JsonValue = serde_json::from_slice(&output.stdout).with_context(|| {
        format!(
            "official JS benchmark {} stdout was not JSON:\n{}",
            fixture.name,
            String::from_utf8_lossy(&output.stdout)
        )
    })?;
    Ok(BenchResult {
        name: fixture.name.into(),
        backend: "official-js".into(),
        package: fixture.kind.official_package().into(),
        iterations,
        elapsed_micros: value
            .get("elapsedMicros")
            .and_then(JsonValue::as_u64)
            .context("official bench result missing elapsedMicros")?
            as u128,
        micros_per_iteration: value
            .get("microsPerIteration")
            .and_then(JsonValue::as_u64)
            .context("official bench result missing microsPerIteration")?
            as u128,
        peak_rss_bytes: output.peak_rss_bytes,
        input_sha256: fixture.sha256.clone(),
        output_bytes: value.get("outputBytes").and_then(JsonValue::as_u64),
    })
}

fn bench_environment(lock: &Path) -> BenchEnvironment {
    BenchEnvironment {
        git_commit: command_output("git", &["rev-parse", "HEAD"]),
        git_dirty: git_dirty(),
        rustc: command_output("rustc", &["--version"]),
        node: command_output("node", &["--version"]),
        npm: command_output_resolved("npm", &["--version"]),
        pnpm: command_output_resolved("pnpm", &["--version"]),
        os: std::env::consts::OS.into(),
        arch: std::env::consts::ARCH.into(),
        lock_path: lock.display().to_string(),
        lock_hash: sha256_file(lock).ok(),
        created_unix: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or_default(),
    }
}

fn command_output(program: &str, args: &[&str]) -> Option<String> {
    let output = ProcessCommand::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn command_output_resolved(program: &str, args: &[&str]) -> Option<String> {
    let program = resolve_program(program).ok()?;
    command_output(&program, args)
}

fn git_dirty() -> bool {
    ProcessCommand::new("git")
        .args(["diff", "--quiet"])
        .status()
        .map(|status| !status.success())
        .unwrap_or(true)
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    Ok(sha256_bytes(&bytes))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("{digest:x}")
}
