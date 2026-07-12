fn run_cargo(args: &[&str]) -> Result<String> {
    let output = ProcessCommand::new("cargo")
        .args(args)
        .output()
        .with_context(|| format!("failed to spawn cargo {}", args.join(" ")))?;
    if !output.status.success() {
        anyhow::bail!(
            "cargo {} exited with {:?}\nstdout:\n{}\nstderr:\n{}",
            args.join(" "),
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(normalize_command_output(&output.stdout, &output.stderr))
}

fn run_wasm_pack_browser_test() -> Result<String> {
    let wasm_pack = resolve_program("wasm-pack")?;
    let chrome = resolve_browser_chrome_binary();
    let webdriver = write_wasm_webdriver_config(chrome.as_deref())?;
    let mut command = ProcessCommand::new(wasm_pack);
    command.args(["test", "--headless", "--chrome"]);
    if let Some(driver) = resolve_browser_chromedriver() {
        command.arg("--chromedriver").arg(driver);
    }
    command.arg("crates/vuec_wasm").env(
        "WASM_BINDGEN_TEST_WEBDRIVER_JSON",
        absolute_path(&webdriver),
    );
    let output = command
        .output()
        .context("failed to spawn wasm-pack test --headless --chrome crates/vuec_wasm")?;
    if !output.status.success() {
        anyhow::bail!(
            "wasm-pack browser test exited with {:?}\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(normalize_command_output(&output.stdout, &output.stderr))
}

fn run_wasi_smoke() -> Result<String> {
    let installed = installed_rust_targets()?;
    if !installed.iter().any(|target| target == "wasm32-wasip1") {
        anyhow::bail!(
            "rust target wasm32-wasip1 is not installed; run `rustup target add wasm32-wasip1`"
        );
    }
    let wasmtime = resolve_program("wasmtime")?;
    let output = ProcessCommand::new("cargo")
        .args([
            "build",
            "-p",
            "vuec_wasm",
            "--bin",
            "wasi_smoke",
            "--target",
            "wasm32-wasip1",
        ])
        .output()
        .context(
            "failed to spawn cargo build -p vuec_wasm --bin wasi_smoke --target wasm32-wasip1",
        )?;
    if !output.status.success() {
        anyhow::bail!(
            "cargo build -p vuec_wasm --bin wasi_smoke --target wasm32-wasip1 exited with {:?}\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let smoke_path = PathBuf::from("target")
        .join("wasm32-wasip1")
        .join("debug")
        .join("wasi_smoke.wasm");
    let request = json!({
        "cases": [
            {
                "name": "vue2-template",
                "command": "compileVue2",
                "source": "<div>{{ msg }}</div>"
            },
            {
                "name": "vue3-dom",
                "command": "compileVue3Dom",
                "source": "<div>{{ msg }}</div>",
                "options": {
                    "mode": "module",
                    "prefixIdentifiers": true,
                    "sourceMap": true
                }
            },
            {
                "name": "sfc-template",
                "command": "compileSfcTemplate",
                "source": "<template><div>{{ msg }}</div></template>",
                "options": {
                    "filename": "Wasi.vue"
                }
            }
        ]
    });
    let mut child = ProcessCommand::new(wasmtime)
        .arg(smoke_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to spawn wasmtime for vuec_wasm WASI smoke")?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(serde_json::to_string(&request)?.as_bytes())
            .context("failed to write WASI smoke request")?;
    }
    let output = child.wait_with_output()?;
    if !output.status.success() {
        anyhow::bail!(
            "wasmtime WASI smoke exited with {:?}\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    verify_wasi_smoke_output(&stdout)?;
    Ok(stdout)
}

fn verify_wasi_smoke_output(stdout: &str) -> Result<()> {
    let value: JsonValue = serde_json::from_str(stdout.trim())
        .with_context(|| format!("WASI smoke stdout was not JSON: {stdout}"))?;
    if value.get("status").and_then(JsonValue::as_str) != Some("pass") {
        anyhow::bail!("WASI smoke reported non-pass status: {value}");
    }
    let vue2 = find_wasi_case(&value, "vue2-template")?;
    let vue2_render = vue2
        .get("render")
        .and_then(JsonValue::as_str)
        .context("WASI vue2-template result missing render")?;
    if !vue2_render.contains("_s(msg)") {
        anyhow::bail!("WASI vue2-template render did not contain _s(msg): {vue2_render}");
    }
    let dom = find_wasi_case(&value, "vue3-dom")?;
    let dom_code = dom
        .get("code")
        .and_then(JsonValue::as_str)
        .context("WASI vue3-dom result missing code")?;
    if !dom_code.contains("_toDisplayString(_ctx.msg)") {
        anyhow::bail!("WASI vue3-dom code did not contain _ctx msg display: {dom_code}");
    }
    if dom.pointer("/map/version").and_then(JsonValue::as_u64) != Some(3) {
        anyhow::bail!("WASI vue3-dom source map version was not 3: {dom}");
    }
    let sfc = find_wasi_case(&value, "sfc-template")?;
    let sfc_code = sfc
        .get("code")
        .and_then(JsonValue::as_str)
        .context("WASI sfc-template result missing code")?;
    if !sfc_code.contains("export function render") {
        anyhow::bail!("WASI sfc-template code did not contain render export: {sfc_code}");
    }
    Ok(())
}

fn find_wasi_case<'a>(value: &'a JsonValue, name: &str) -> Result<&'a JsonValue> {
    value
        .get("cases")
        .and_then(JsonValue::as_array)
        .and_then(|cases| {
            cases.iter().find_map(|case| {
                if case.get("name").and_then(JsonValue::as_str) == Some(name) {
                    case.get("result")
                } else {
                    None
                }
            })
        })
        .with_context(|| format!("WASI smoke case `{name}` was not found"))
}

fn run_cli_smoke_suite() -> Result<String> {
    build_cli_binary()?;

    let root = PathBuf::from("target").join("cli-smoke");
    if root.exists() {
        fs::remove_dir_all(&root)
            .with_context(|| format!("failed to remove {}", root.display()))?;
    }
    fs::create_dir_all(&root).with_context(|| format!("failed to create {}", root.display()))?;
    let vue2 = write_cli_fixture(&root, "vue2.html", "<div>{{ msg }}</div>")?;
    let vue3 = write_cli_fixture(&root, "vue3.html", "<div>{{ msg }}</div>")?;
    let sfc = write_cli_fixture(
        &root,
        "App.vue",
        "<template><div>{{ msg }}</div></template><script setup>const msg = 'hi'</script>",
    )?;
    let invalid = write_cli_fixture(&root, "invalid.html", r#"<div v-model="baz"/>"#)?;
    let map_path = root.join("vue3.map.json");
    let exe = cli_executable_path();
    let mut checks = Vec::new();

    let help = run_cli_command(&exe, &["--help"])?;
    if help.status != 0 || !help.stdout.contains("compile-template") {
        anyhow::bail!("vuec --help did not exit successfully with command list");
    }
    checks.push("help");

    let vue2_out = run_cli_command(
        &exe,
        &[
            "compile-template",
            "--target",
            "vue2",
            "--json",
            &vue2.display().to_string(),
        ],
    )?;
    let vue2_json = parse_cli_json("vue2 template", &vue2_out)?;
    if vue2_json.get("kind").and_then(JsonValue::as_str) != Some("vue2-template") {
        anyhow::bail!("vue2 template CLI kind mismatch: {vue2_json}");
    }
    if !vue2_json
        .get("render")
        .and_then(JsonValue::as_str)
        .unwrap_or_default()
        .contains("_s(msg)")
    {
        anyhow::bail!("vue2 template CLI render missing _s(msg): {vue2_json}");
    }
    checks.push("vue2-template");

    let vue3_out = run_cli_command(
        &exe,
        &[
            "compile-template",
            "--target",
            "vue3",
            "--mode",
            "module",
            "--source-map",
            "--map-out",
            &map_path.display().to_string(),
            "--json",
            &vue3.display().to_string(),
        ],
    )?;
    let vue3_json = parse_cli_json("vue3 template", &vue3_out)?;
    if vue3_json.get("kind").and_then(JsonValue::as_str) != Some("vue3-template") {
        anyhow::bail!("vue3 template CLI kind mismatch: {vue3_json}");
    }
    if !vue3_json
        .get("code")
        .and_then(JsonValue::as_str)
        .unwrap_or_default()
        .contains("export function render")
    {
        anyhow::bail!("vue3 template CLI code missing module render export: {vue3_json}");
    }
    let map: JsonValue = serde_json::from_str(
        &fs::read_to_string(&map_path)
            .with_context(|| format!("failed to read {}", map_path.display()))?,
    )?;
    if map.get("version").and_then(JsonValue::as_u64) != Some(3) {
        anyhow::bail!("vue3 template CLI source map version was not 3: {map}");
    }
    checks.push("vue3-template-map");

    let diagnostic_out = run_cli_command(
        &exe,
        &[
            "compile-template",
            "--target",
            "vue3",
            "--diagnostics",
            &invalid.display().to_string(),
        ],
    )?;
    if !diagnostic_out.stderr.contains("[error]") || !diagnostic_out.stderr.contains("v-model") {
        anyhow::bail!("vue3 diagnostics CLI output missing expected v-model error");
    }
    checks.push("diagnostics");

    let sfc_out = run_cli_command(&exe, &["compile-sfc", "--json", &sfc.display().to_string()])?;
    let sfc_json = parse_cli_json("sfc", &sfc_out)?;
    if sfc_json.get("kind").and_then(JsonValue::as_str) != Some("vue3-sfc") {
        anyhow::bail!("SFC CLI kind mismatch: {sfc_json}");
    }
    if !sfc_json
        .pointer("/template/code")
        .and_then(JsonValue::as_str)
        .unwrap_or_default()
        .contains("function render")
    {
        anyhow::bail!("SFC CLI template output missing render: {sfc_json}");
    }
    checks.push("sfc");

    let ssr_out = run_cli_command(
        &exe,
        &["compile-ssr", "--json", &vue3.display().to_string()],
    )?;
    let ssr_json = parse_cli_json("ssr", &ssr_out)?;
    if ssr_json.get("kind").and_then(JsonValue::as_str) != Some("vue3-ssr-template") {
        anyhow::bail!("SSR CLI kind mismatch: {ssr_json}");
    }
    if !ssr_json
        .get("code")
        .and_then(JsonValue::as_str)
        .unwrap_or_default()
        .contains("ssrRender")
    {
        anyhow::bail!("SSR CLI output missing ssrRender: {ssr_json}");
    }
    checks.push("ssr");

    let parse_out = run_cli_command(&exe, &["parse-sfc", "--json", &sfc.display().to_string()])?;
    let parse_json = parse_cli_json("parse-sfc", &parse_out)?;
    if parse_json.pointer("/descriptor/template").is_none() {
        anyhow::bail!("parse-sfc CLI output missing descriptor template: {parse_json}");
    }
    checks.push("parse-sfc");

    let bench_out = run_cli_command(
        &exe,
        &[
            "bench",
            "--target",
            "vue3-template",
            "--iterations",
            "1",
            "--json",
            &vue3.display().to_string(),
        ],
    )?;
    let bench_json = parse_cli_json("bench", &bench_out)?;
    if bench_json.get("kind").and_then(JsonValue::as_str) != Some("bench")
        || bench_json.get("iterations").and_then(JsonValue::as_u64) != Some(1)
    {
        anyhow::bail!("bench CLI output did not report one iteration: {bench_json}");
    }
    checks.push("bench");

    Ok(json!({
        "status": "pass",
        "checks": checks,
        "fixtureRoot": root,
    })
    .to_string())
}

fn run_parallel_smoke_suite() -> Result<String> {
    build_cli_binary()?;

    let root = PathBuf::from("target").join("parallel-smoke");
    if root.exists() {
        fs::remove_dir_all(&root)
            .with_context(|| format!("failed to remove {}", root.display()))?;
    }
    fs::create_dir_all(&root).with_context(|| format!("failed to create {}", root.display()))?;
    let first = write_cli_fixture(&root, "first.html", "<div>{{ first }}</div>")?;
    let second = write_cli_fixture(&root, "second.html", "<section>{{ second }}</section>")?;
    let vue2_first = write_cli_fixture(&root, "vue2-first.html", "<p>{{ first }}</p>")?;
    let vue2_second = write_cli_fixture(&root, "vue2-second.html", "<p>{{ second }}</p>")?;
    let sfc_first = write_cli_fixture(
        &root,
        "First.vue",
        "<template><div>{{ first }}</div></template><script setup>const first = 'one'</script>",
    )?;
    let sfc_second = write_cli_fixture(
        &root,
        "Second.vue",
        "<template><section>{{ second }}</section></template><script setup>const second = 'two'</script>",
    )?;
    let exe = cli_executable_path();
    let mut checks = Vec::new();

    let template_out = run_cli_command(
        &exe,
        &[
            "compile-batch",
            "--target",
            "vue3-template",
            "--jobs",
            "2",
            "--json",
            &first.display().to_string(),
            &second.display().to_string(),
        ],
    )?;
    let template_json = parse_cli_json("parallel vue3 template", &template_out)?;
    assert_parallel_batch_result(
        &template_json,
        "vue3-template",
        2,
        &[
            ParallelExpectation {
                path_fragment: "first.html",
                result_kind: "vue3-template",
                output_pointer: "/result/code",
                output_fragment: "first",
            },
            ParallelExpectation {
                path_fragment: "second.html",
                result_kind: "vue3-template",
                output_pointer: "/result/code",
                output_fragment: "second",
            },
        ],
    )?;
    checks.push("vue3-template-order");

    let vue2_out = run_cli_command(
        &exe,
        &[
            "compile-batch",
            "--target",
            "vue2-template",
            "--jobs",
            "2",
            "--json",
            &vue2_first.display().to_string(),
            &vue2_second.display().to_string(),
        ],
    )?;
    let vue2_json = parse_cli_json("parallel vue2 template", &vue2_out)?;
    assert_parallel_batch_result(
        &vue2_json,
        "vue2-template",
        2,
        &[
            ParallelExpectation {
                path_fragment: "vue2-first.html",
                result_kind: "vue2-template",
                output_pointer: "/result/render",
                output_fragment: "first",
            },
            ParallelExpectation {
                path_fragment: "vue2-second.html",
                result_kind: "vue2-template",
                output_pointer: "/result/render",
                output_fragment: "second",
            },
        ],
    )?;
    checks.push("vue2-template-order");

    let sfc_out = run_cli_command(
        &exe,
        &[
            "compile-batch",
            "--target",
            "vue3-sfc",
            "--jobs",
            "2",
            "--json",
            &sfc_first.display().to_string(),
            &sfc_second.display().to_string(),
        ],
    )?;
    let sfc_json = parse_cli_json("parallel sfc", &sfc_out)?;
    assert_parallel_batch_result(
        &sfc_json,
        "vue3-sfc",
        2,
        &[
            ParallelExpectation {
                path_fragment: "First.vue",
                result_kind: "vue3-sfc",
                output_pointer: "/result/template/code",
                output_fragment: "first",
            },
            ParallelExpectation {
                path_fragment: "Second.vue",
                result_kind: "vue3-sfc",
                output_pointer: "/result/template/code",
                output_fragment: "second",
            },
        ],
    )?;
    checks.push("sfc-order");

    let ssr_out = run_cli_command(
        &exe,
        &[
            "compile-batch",
            "--target",
            "vue3-ssr",
            "--jobs",
            "2",
            "--json",
            &first.display().to_string(),
            &second.display().to_string(),
        ],
    )?;
    let ssr_json = parse_cli_json("parallel ssr", &ssr_out)?;
    assert_parallel_batch_result(
        &ssr_json,
        "vue3-ssr",
        2,
        &[
            ParallelExpectation {
                path_fragment: "first.html",
                result_kind: "vue3-ssr-template",
                output_pointer: "/result/code",
                output_fragment: "ssrRender",
            },
            ParallelExpectation {
                path_fragment: "second.html",
                result_kind: "vue3-ssr-template",
                output_pointer: "/result/code",
                output_fragment: "ssrRender",
            },
        ],
    )?;
    checks.push("ssr-order");

    Ok(json!({
        "status": "pass",
        "checks": checks,
        "fixtureRoot": root,
    })
    .to_string())
}

struct ParallelExpectation {
    path_fragment: &'static str,
    result_kind: &'static str,
    output_pointer: &'static str,
    output_fragment: &'static str,
}

fn assert_parallel_batch_result(
    value: &JsonValue,
    target: &str,
    jobs: u64,
    expected: &[ParallelExpectation],
) -> Result<()> {
    if value.get("kind").and_then(JsonValue::as_str) != Some("compile-batch") {
        anyhow::bail!("parallel batch kind mismatch: {value}");
    }
    if value.get("target").and_then(JsonValue::as_str) != Some(target) {
        anyhow::bail!("parallel batch target mismatch for {target}: {value}");
    }
    if value.get("jobs").and_then(JsonValue::as_u64) != Some(jobs) {
        anyhow::bail!("parallel batch jobs mismatch for {target}: {value}");
    }
    let results = value
        .get("results")
        .and_then(JsonValue::as_array)
        .with_context(|| format!("parallel batch {target} missing results array"))?;
    if results.len() != expected.len() {
        anyhow::bail!(
            "parallel batch {target} result length mismatch: expected {}, got {} in {value}",
            expected.len(),
            results.len()
        );
    }
    for (index, (result, expected)) in results.iter().zip(expected).enumerate() {
        if result.get("index").and_then(JsonValue::as_u64) != Some(index as u64) {
            anyhow::bail!("parallel batch {target} result index mismatch at {index}: {result}");
        }
        if result.get("status").and_then(JsonValue::as_str) != Some("ok") {
            anyhow::bail!("parallel batch {target} result did not pass at {index}: {result}");
        }
        if !result
            .get("input")
            .and_then(JsonValue::as_str)
            .unwrap_or_default()
            .contains(expected.path_fragment)
        {
            anyhow::bail!("parallel batch {target} input order mismatch at {index}: {result}");
        }
        if result.pointer("/result/kind").and_then(JsonValue::as_str) != Some(expected.result_kind)
        {
            anyhow::bail!("parallel batch {target} result kind mismatch at {index}: {result}");
        }
        if !result
            .pointer(expected.output_pointer)
            .and_then(JsonValue::as_str)
            .unwrap_or_default()
            .contains(expected.output_fragment)
        {
            anyhow::bail!("parallel batch {target} output mismatch at {index}: {result}");
        }
    }
    Ok(())
}

fn build_cli_binary() -> Result<()> {
    let build = ProcessCommand::new("cargo")
        .args(["build", "-p", "vuec_cli", "--bin", "vuec"])
        .output()
        .context("failed to spawn cargo build -p vuec_cli --bin vuec")?;
    if !build.status.success() {
        anyhow::bail!(
            "cargo build -p vuec_cli --bin vuec exited with {:?}\nstdout:\n{}\nstderr:\n{}",
            build.status.code(),
            String::from_utf8_lossy(&build.stdout),
            String::from_utf8_lossy(&build.stderr)
        );
    }
    Ok(())
}

fn write_cli_fixture(root: &Path, name: &str, source: &str) -> Result<PathBuf> {
    let path = root.join(name);
    fs::write(&path, source).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(path)
}

fn cli_executable_path() -> PathBuf {
    let exe = if cfg!(windows) { "vuec.exe" } else { "vuec" };
    PathBuf::from("target").join("debug").join(exe)
}

struct CliProcessOutput {
    status: i32,
    stdout: String,
    stderr: String,
}

fn run_cli_command(exe: &Path, args: &[&str]) -> Result<CliProcessOutput> {
    let output = ProcessCommand::new(exe)
        .args(args)
        .output()
        .with_context(|| format!("failed to run vuec {}", args.join(" ")))?;
    Ok(CliProcessOutput {
        status: output.status.code().unwrap_or(1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

fn parse_cli_json(label: &str, output: &CliProcessOutput) -> Result<JsonValue> {
    if output.status != 0 {
        anyhow::bail!(
            "{label} CLI command failed with {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            output.stdout,
            output.stderr
        );
    }
    serde_json::from_str(&output.stdout)
        .with_context(|| format!("{label} CLI stdout was not JSON:\n{}", output.stdout))
}

struct MonitoredOutput {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    peak_rss_bytes: Option<u64>,
}

fn run_monitored_command(mut command: ProcessCommand) -> Result<MonitoredOutput> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .context("failed to spawn monitored command")?;
    let pid = child.id();
    let mut sampler = ProcessMemorySampler::new(pid);
    let mut peak = sampler.sample_peak_rss_bytes();
    loop {
        if child
            .try_wait()
            .context("failed to poll monitored command")?
            .is_some()
        {
            break;
        }
        peak = max_optional_u64(peak, sampler.sample_peak_rss_bytes());
        thread::sleep(Duration::from_millis(5));
    }
    peak = max_optional_u64(peak, sampler.sample_peak_rss_bytes());
    let output = child
        .wait_with_output()
        .context("failed to collect monitored command output")?;
    Ok(MonitoredOutput {
        status: output.status,
        stdout: output.stdout,
        stderr: output.stderr,
        peak_rss_bytes: peak,
    })
}

struct ProcessMemorySampler {
    system: System,
    pid: Pid,
}

impl ProcessMemorySampler {
    fn new(pid: u32) -> Self {
        Self {
            system: System::new(),
            pid: Pid::from_u32(pid),
        }
    }

    fn sample_peak_rss_bytes(&mut self) -> Option<u64> {
        self.system
            .refresh_processes(ProcessesToUpdate::Some(&[self.pid]), false);
        self.system
            .process(self.pid)
            .map(|process| process.memory())
    }
}

fn max_optional_u64(left: Option<u64>, right: Option<u64>) -> Option<u64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}

#[cfg(test)]
fn parse_proc_status_rss_bytes(status: &str) -> Option<u64> {
    let mut rss = None;
    for key in ["VmHWM:", "VmRSS:"] {
        if let Some(bytes) = status
            .lines()
            .find_map(|line| line.strip_prefix(key).and_then(parse_proc_status_kb_line))
        {
            rss = Some(bytes);
            if key == "VmHWM:" {
                break;
            }
        }
    }
    rss
}

#[cfg(test)]
fn parse_proc_status_kb_line(value: &str) -> Option<u64> {
    let kb = value.split_whitespace().next()?.parse::<u64>().ok()?;
    kb.checked_mul(1024)
}

fn parse_monitored_json(label: &str, output: &MonitoredOutput) -> Result<JsonValue> {
    if !output.status.success() {
        anyhow::bail!(
            "{label} command failed with {:?}\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    serde_json::from_slice(&output.stdout).with_context(|| {
        format!(
            "{label} stdout was not JSON:\n{}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

fn run_incremental_smoke_suite() -> Result<String> {
    let mut compiler = vuec_sfc::SfcCompiler::new();
    let source_one = incremental_source("one");
    let first = compiler.parse("Incremental.vue", &source_one);
    let second = compiler.parse("Incremental.vue", &source_one);
    if first != second {
        anyhow::bail!("SFC descriptor cache returned a different descriptor for unchanged input");
    }
    let hit_stats = compiler.cache_stats();
    if hit_stats.descriptor_hits != 1
        || hit_stats.descriptor_misses != 1
        || hit_stats.descriptor_invalidations != 0
        || compiler.descriptor_cache_len() != 1
    {
        anyhow::bail!(
            "SFC descriptor cache did not record one hit and one miss for unchanged input: {:?}, cache_len={}",
            hit_stats,
            compiler.descriptor_cache_len()
        );
    }

    let source_two = incremental_source("two");
    let changed = compiler.parse("Incremental.vue", &source_two);
    let changed_content = changed
        .template
        .as_ref()
        .map(|template| template.content.as_str())
        .unwrap_or_default();
    if !changed_content.contains("two") {
        anyhow::bail!("SFC descriptor cache did not return changed template content");
    }
    let invalidated_stats = compiler.cache_stats();
    if invalidated_stats.descriptor_hits != 1
        || invalidated_stats.descriptor_misses != 2
        || invalidated_stats.descriptor_invalidations != 1
        || compiler.descriptor_cache_len() != 1
    {
        anyhow::bail!(
            "SFC descriptor cache did not invalidate changed same-file input: {:?}, cache_len={}",
            invalidated_stats,
            compiler.descriptor_cache_len()
        );
    }

    let source_legacy = incremental_source("legacy");
    let vue27_first = compiler.parse_vue27_component_with_filename(
        "IncrementalVue27.vue",
        &source_legacy,
        vuec_sfc::Vue27ParseComponentOptions::default(),
    );
    let vue27_second = compiler.parse_vue27_component_with_filename(
        "IncrementalVue27.vue",
        &source_legacy,
        vuec_sfc::Vue27ParseComponentOptions::default(),
    );
    if vue27_first.descriptor != vue27_second.descriptor {
        anyhow::bail!("Vue 2.7 SFC descriptor cache returned different descriptors");
    }
    let vue27_stats = compiler.cache_stats();
    if vue27_stats.descriptor_hits != 2 || vue27_stats.descriptor_misses != 3 {
        anyhow::bail!("Vue 2.7 SFC descriptor cache did not record reuse: {vue27_stats:?}");
    }

    Ok(json!({
        "status": "pass",
        "checks": [
            "same-file-same-source-hit",
            "same-file-changed-source-invalidates",
            "vue27-mode-cache-hit"
        ],
        "stats": vue27_stats,
        "cacheEntries": compiler.descriptor_cache_len(),
    })
    .to_string())
}

fn incremental_source(value: &str) -> String {
    format!(
        r#"<template><div>{{{{ {value} }}}}</div></template><script setup>const {value} = "{value}"</script>"#
    )
}

fn run_ast_cache_smoke_suite() -> Result<String> {
    let mut compiler = vuec_vue3_dom::DomCompiler::new();
    let mut options = vuec_vue3_dom::DomCompilerOptions::default();
    options.core.prefix_identifiers = true;
    options.core.mode = "module".into();

    let source_one = dom_cache_template_source("Cached.vue", "<div>{{ one }}</div>");
    let first = compiler.compile(source_one.clone(), options.clone());
    let second = compiler.compile(source_one, options.clone());
    if first.code != second.code || first.ast_summary != second.ast_summary {
        anyhow::bail!("Vue3 DOM AST cache changed output for unchanged input");
    }
    let hit_stats = compiler.cache_stats();
    if hit_stats.ast_hits != 1
        || hit_stats.ast_misses != 1
        || hit_stats.ast_invalidations != 0
        || compiler.ast_cache_len() != 1
    {
        anyhow::bail!(
            "Vue3 DOM AST cache did not record one hit and one miss for unchanged input: {:?}, cache_len={}",
            hit_stats,
            compiler.ast_cache_len()
        );
    }

    let source_two = dom_cache_template_source("Cached.vue", "<section>{{ two }}</section>");
    let changed = compiler.compile(source_two, options.clone());
    if !changed.code.contains("section") || changed.code == first.code {
        anyhow::bail!("Vue3 DOM AST cache did not return changed same-file output");
    }
    let invalidated_stats = compiler.cache_stats();
    if invalidated_stats.ast_hits != 1
        || invalidated_stats.ast_misses != 2
        || invalidated_stats.ast_invalidations != 1
        || compiler.ast_cache_len() != 1
    {
        anyhow::bail!(
            "Vue3 DOM AST cache did not invalidate changed same-file input: {:?}, cache_len={}",
            invalidated_stats,
            compiler.ast_cache_len()
        );
    }

    let mut without_comments = options.clone();
    without_comments.core.comments = false;
    let source_with_comment =
        dom_cache_template_source("OptionCached.vue", "<div><!--x-->{{ one }}</div>");
    let with_comments = compiler.compile(source_with_comment.clone(), options);
    let without_comments = compiler.compile(source_with_comment, without_comments);
    if with_comments.ast_summary == without_comments.ast_summary {
        anyhow::bail!("Vue3 DOM AST cache did not separate parse options");
    }
    let option_stats = compiler.cache_stats();
    if option_stats.ast_hits != 1
        || option_stats.ast_misses != 4
        || option_stats.ast_invalidations != 1
        || compiler.ast_cache_len() != 3
    {
        anyhow::bail!(
            "Vue3 DOM AST cache option-key stats mismatch: {:?}, cache_len={}",
            option_stats,
            compiler.ast_cache_len()
        );
    }

    Ok(json!({
        "status": "pass",
        "checks": [
            "same-file-same-source-hit",
            "same-file-changed-source-invalidates",
            "parse-options-separate-cache-entry"
        ],
        "stats": option_stats,
        "cacheEntries": compiler.ast_cache_len(),
    })
    .to_string())
}

fn dom_cache_template_source(filename: &str, source: &str) -> vuec_vue3_core::TemplateSource {
    vuec_vue3_core::TemplateSource {
        filename: filename.into(),
        source: source.into(),
        file_id: vuec_source::FileId(0),
        base_offset: 0,
    }
}

fn run_arena_smoke_suite() -> Result<String> {
    let manual_hint = 64usize;
    let mut manual = vuec_ast::Vue3Ast::with_capacity(
        vuec_ast::Vue3NodeKind::root(),
        vuec_ast::NodeSpan::missing(vuec_ast::MissingSpanReason::Synthetic),
        manual_hint,
    );
    let child = manual.push_child(manual.root, vuec_ast::Vue3NodeKind::text("arena"), None);
    manual.validate_tree().map_err(|err| {
        anyhow::anyhow!(
            "AstDocument::with_capacity produced invalid root/child metadata after push_child: {err:?}"
        )
    })?;
    if manual.root != vuec_ast::NodeId(0) || child != vuec_ast::NodeId(1) {
        anyhow::bail!(
            "AstDocument::with_capacity changed deterministic NodeId allocation: root={:?}, child={:?}",
            manual.root,
            child
        );
    }
    if manual.node_capacity() < manual_hint {
        anyhow::bail!(
            "AstDocument::with_capacity did not reserve requested node capacity: capacity={}, requested={manual_hint}",
            manual.node_capacity()
        );
    }

    let vue3_source = repeated_arena_template("section", "item", 80);
    let vue3_hint = vuec_ast::template_node_capacity_hint(&vue3_source);
    let vue3_ast = vuec_vue3_core::Vue3Dialect::base_parse(
        vuec_vue3_core::TemplateSource {
            filename: "Arena.vue".into(),
            source: vue3_source.clone(),
            file_id: vuec_source::FileId(0),
            base_offset: 0,
        },
        &vuec_vue3_core::Vue3CompilerOptions::default(),
    );
    vue3_ast.validate_tree().map_err(|err| {
        anyhow::anyhow!("Vue3 base_parse returned an invalid arena tree: {err:?}")
    })?;
    if vue3_ast.node_capacity() < vue3_hint {
        anyhow::bail!(
            "Vue3 base_parse did not apply template node capacity hint: capacity={}, hint={vue3_hint}, len={}",
            vue3_ast.node_capacity(),
            vue3_ast.len()
        );
    }

    let vue2_source = repeated_arena_template("p", "entry", 80);
    let vue2_hint = vuec_ast::template_node_capacity_hint(&vue2_source);
    let vue2 = vuec_vue2::compile(&vue2_source, vuec_vue2::Vue2CompileOptions::default());
    vue2.ast.validate_tree().map_err(|err| {
        anyhow::anyhow!("Vue2 public AST projection returned an invalid arena tree: {err:?}")
    })?;
    if vue2.ast.node_capacity() < vue2_hint {
        anyhow::bail!(
            "Vue2 public AST projection did not apply template node capacity hint: capacity={}, hint={vue2_hint}, len={}",
            vue2.ast.node_capacity(),
            vue2.ast.len()
        );
    }

    Ok(json!({
        "status": "pass",
        "checks": [
            "ast-document-with-capacity-preserves-node-ids",
            "vue3-base-parse-preallocates-arena",
            "vue2-public-projection-preallocates-arena"
        ],
        "manual": {
            "requestedCapacity": manual_hint,
            "capacity": manual.node_capacity(),
            "nodes": manual.len()
        },
        "vue3": {
            "hint": vue3_hint,
            "capacity": vue3_ast.node_capacity(),
            "nodes": vue3_ast.len()
        },
        "vue2": {
            "hint": vue2_hint,
            "capacity": vue2.ast.node_capacity(),
            "nodes": vue2.ast.len()
        }
    })
    .to_string())
}

fn repeated_arena_template(tag: &str, prefix: &str, count: usize) -> String {
    let mut source = String::from("<div>");
    for index in 0..count {
        source.push_str(&format!(
            "<{tag} data-index=\"{index}\">{{{{ {prefix}{index} }}}}</{tag}>text{index}"
        ));
    }
    source.push_str("</div>");
    source
}

fn run_string_interning_smoke_suite() -> Result<String> {
    let mut store = vuec_js::JsAstStore::new();
    let expr = store.register_expr(
        "item.count",
        vuec_source::Span::new(vuec_source::FileId(0), 0, 10),
        oxc_span::SourceType::script(),
    );
    let stmt = store.register_stmt(
        "item.count",
        vuec_source::Span::new(vuec_source::FileId(0), 20, 30),
        oxc_span::SourceType::script(),
    );
    let pattern = store.register_pattern(
        "item",
        vuec_source::Span::new(vuec_source::FileId(0), 40, 44),
        oxc_span::SourceType::script(),
    );
    let program = store.register_program(
        "item.count",
        vuec_source::Span::new(vuec_source::FileId(0), 50, 60),
        vuec_js::JsParseMode::ScriptModule,
        oxc_span::SourceType::mjs(),
    );

    let expr_entry = store.expr_entry(expr).context("missing interned expr")?;
    let stmt_entry = store.stmt_entry(stmt).context("missing interned stmt")?;
    let pattern_entry = store
        .pattern_entry(pattern)
        .context("missing interned pattern")?;
    let program_entry = store
        .program_entry(program)
        .context("missing interned program")?;
    if !store.interned_source_ptr_eq(expr_entry, stmt_entry)
        || !store.interned_source_ptr_eq(expr_entry, program_entry)
    {
        anyhow::bail!("repeated JS source text did not share interned storage");
    }
    if store.interned_source_ptr_eq(expr_entry, pattern_entry) {
        anyhow::bail!("distinct JS source text unexpectedly shared interned storage");
    }
    let stats = store.string_interner_stats();
    if stats.hits != 2 || stats.misses != 2 || stats.entries != 2 {
        anyhow::bail!("JS source interner stats mismatch: {stats:?}");
    }
    let serialized = serde_json::to_value(expr_entry)?;
    if serialized.get("source").and_then(JsonValue::as_str) != Some("item.count") {
        anyhow::bail!("interned JS source did not serialize as a plain string: {serialized}");
    }

    let lowering_options = vuec_vue3_core::Vue3CompilerOptions {
        prefix_identifiers: true,
        mode: "module".into(),
        ..vuec_vue3_core::Vue3CompilerOptions::default()
    };
    let source = vuec_vue3_core::TemplateSource {
        filename: "Interned.vue".into(),
        source: "<div>{{ item.count }}</div>".into(),
        file_id: vuec_source::FileId(0),
        base_offset: 0,
    };
    let mut ast = vuec_vue3_core::Vue3Dialect::base_parse(source, &lowering_options);
    let mut ctx = vuec_pass::TransformContext::default();
    vuec_vue3_core::Vue3Dialect::transform(&mut ast, &mut ctx, &lowering_options);
    let lowered = vuec_vue3_core::lower_vue3_ast_to_dom_mir(&ast, &lowering_options);
    let generated =
        vuec_vue3_core::generate_vue3_dom_mir(&lowered.mir, &lowered.js, &lowering_options);
    if !generated.code.contains("_ctx.item.count") {
        anyhow::bail!(
            "Vue3 DOM MIR codegen did not consume interned JS source: {}",
            generated.code
        );
    }

    Ok(json!({
        "status": "pass",
        "checks": [
            "repeated-js-source-shares-interned-storage",
            "distinct-js-source-keeps-distinct-storage",
            "interned-source-serializes-as-string",
            "vue3-dom-mir-codegen-consumes-interned-js-store"
        ],
        "stats": stats,
        "serializedSource": serialized["source"],
    })
    .to_string())
}
