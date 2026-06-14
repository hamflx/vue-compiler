fn bench(
    iterations: usize,
    out_dir: &Path,
    lock: &Path,
    skip_official_js: bool,
) -> Result<compat::JsonReport> {
    if iterations == 0 {
        anyhow::bail!("--iterations must be greater than zero");
    }
    ensure_target_child(out_dir, "bench")?;
    fs::create_dir_all(out_dir)
        .with_context(|| format!("failed to create {}", out_dir.display()))?;
    let fixture_dir = out_dir.join("fixtures");
    fs::create_dir_all(&fixture_dir)
        .with_context(|| format!("failed to create {}", fixture_dir.display()))?;

    let fixtures = write_bench_fixtures(&fixture_dir)?;
    let env = bench_environment(lock);
    let mut items = Vec::new();
    let mut violations = Vec::new();
    let mut results = Vec::new();

    build_cli_binary()?;
    for fixture in &fixtures {
        match run_rust_bench_case(fixture, iterations) {
            Ok(result) => {
                items.push(compat::ReportItem::new(
                    format!("rust-{}", fixture.name),
                    compat::ReportStatus::Pass,
                    serde_json::to_string(&result)?,
                    Some(fixture.path.clone()),
                ));
                results.push(result);
            }
            Err(err) => {
                violations.push(format!("Rust benchmark {} failed: {err:#}", fixture.name));
                items.push(compat::ReportItem::new(
                    format!("rust-{}", fixture.name),
                    compat::ReportStatus::Fail,
                    format!("{err:#}"),
                    Some(fixture.path.clone()),
                ));
            }
        }
    }

    if skip_official_js {
        let detail = json!({
            "backend": "official-js",
            "status": "pending",
            "reason": "--skip-official-js was supplied"
        })
        .to_string();
        items.push(compat::ReportItem::new(
            "official-js",
            compat::ReportStatus::Pending,
            detail,
            Some(out_dir.to_path_buf()),
        ));
    } else {
        match prepare_official_js_bench_root(out_dir, lock) {
            Ok(official_root) => {
                for fixture in &fixtures {
                    match run_official_js_bench_case(&official_root, fixture, iterations) {
                        Ok(result) => {
                            items.push(compat::ReportItem::new(
                                format!("official-js-{}", fixture.name),
                                compat::ReportStatus::Pass,
                                serde_json::to_string(&result)?,
                                Some(fixture.path.clone()),
                            ));
                            results.push(result);
                        }
                        Err(err) => {
                            violations.push(format!(
                                "official JS benchmark {} failed: {err:#}",
                                fixture.name
                            ));
                            items.push(compat::ReportItem::new(
                                format!("official-js-{}", fixture.name),
                                compat::ReportStatus::Fail,
                                format!("{err:#}"),
                                Some(fixture.path.clone()),
                            ));
                        }
                    }
                }
            }
            Err(err) => {
                let detail = json!({
                    "backend": "official-js",
                    "status": "pending",
                    "reason": format!("{err:#}")
                })
                .to_string();
                items.push(compat::ReportItem::new(
                    "official-js",
                    compat::ReportStatus::Pending,
                    detail,
                    Some(out_dir.to_path_buf()),
                ));
            }
        }
    }

    let bench_report = BenchReport {
        status: if violations.is_empty() {
            "pass".into()
        } else {
            "fail".into()
        },
        iterations,
        environment: env,
        fixtures: fixtures.iter().map(BenchFixtureReport::from).collect(),
        results,
    };
    let report_path = out_dir.join("bench-report.json");
    fs::write(&report_path, serde_json::to_string_pretty(&bench_report)?)
        .with_context(|| format!("failed to write {}", report_path.display()))?;

    Ok(
        compat::JsonReport::new("bench", compat::ReportStatus::Pass)
            .with_items(items)
            .with_violations(violations)
            .with_created(vec![report_path.display().to_string()])
            .with_note("generates a reproducible benchmark report with input hashes, environment, git commit, Rust CLI timings, best-effort peak RSS, and official JS compiler timings when the locked npm compilers are available"),
    )
}

fn profile_compile_script(
    version_line: &str,
    fixture_corpus: &Path,
    iterations: usize,
    script_ast_mode: vuec_sfc::SfcScriptAstMode,
    out_dir: &Path,
) -> Result<compat::JsonReport> {
    if iterations == 0 {
        anyhow::bail!("--iterations must be greater than zero");
    }
    let version = CompileScriptProfileVersion::parse(version_line)?;
    ensure_nested_target_child(out_dir, &["perf", "compile-script"])?;
    fs::create_dir_all(out_dir)
        .with_context(|| format!("failed to create {}", out_dir.display()))?;

    let fixtures = load_compile_script_profile_fixtures(fixture_corpus)?;
    let mut items = Vec::new();
    let mut violations = Vec::new();
    let mut results = Vec::new();

    for fixture in &fixtures {
        match compile_script_profile_fixture(version, fixture, iterations, script_ast_mode) {
            Ok(result) => {
                items.push(compat::ReportItem::new(
                    format!("{}-{}", version.canonical(), fixture.name),
                    compat::ReportStatus::Pass,
                    serde_json::to_string(&json!({
                        "versionLine": result.version_line,
                        "iterations": result.iterations,
                        "parseMedianMicros": result.parse.median_micros,
                        "compileScriptMedianMicros": result.compile_script.median_micros,
                        "serializeMedianMicros": result.serialize.median_micros,
                        "structuralCounts": result.structural_counts,
                    }))?,
                    Some(fixture.path.clone()),
                ));
                results.push(result);
            }
            Err(err) => {
                violations.push(format!(
                    "compileScript profile {} failed: {err:#}",
                    fixture.name
                ));
                items.push(compat::ReportItem::new(
                    format!("{}-{}", version.canonical(), fixture.name),
                    compat::ReportStatus::Fail,
                    format!("{err:#}"),
                    Some(fixture.path.clone()),
                ));
            }
        }
    }

    let report = CompileScriptProfileReport {
        status: if violations.is_empty() {
            "pass".into()
        } else {
            "fail".into()
        },
        version_line: version.canonical().into(),
        iterations,
        build_profile: compile_script_build_profile().into(),
        script_ast_mode: compile_script_ast_mode_name(script_ast_mode).into(),
        environment: bench_environment(Path::new("compat/official-revisions.lock")),
        fixtures: fixtures
            .iter()
            .map(CompileScriptProfileFixtureReport::from)
            .collect(),
        results,
    };
    let report_path = out_dir.join(format!(
        "{}.{}.json",
        version.canonical(),
        compile_script_ast_mode_name(script_ast_mode)
    ));
    fs::write(&report_path, serde_json::to_string_pretty(&report)?)
        .with_context(|| format!("failed to write {}", report_path.display()))?;

    Ok(compat::JsonReport::new("profile_compile_script", compat::ReportStatus::Pass)
        .with_items(items)
        .with_violations(violations)
        .with_created(vec![report_path.display().to_string()])
        .with_note("profiles Rust compileScript parse, compile, and serialization phases for fixed SFC fixture corpora; --script-ast-mode selects full, top-level, or no public AST projection for root-cause comparisons"))
}

fn compare_compile_script_profile(
    full_path: &Path,
    top_level_path: &Path,
    none_path: &Path,
    out_dir: &Path,
    min_full_to_none_compile_ratio: f64,
) -> Result<compat::JsonReport> {
    ensure_nested_target_child(out_dir, &["perf", "compile-script-comparison"])?;
    fs::create_dir_all(out_dir)
        .with_context(|| format!("failed to create {}", out_dir.display()))?;
    let full = read_compile_script_profile_report(full_path)?;
    let top_level = read_compile_script_profile_report(top_level_path)?;
    let none = read_compile_script_profile_report(none_path)?;
    validate_compile_script_profile_mode(&full, "full", full_path)?;
    validate_compile_script_profile_mode(&top_level, "top-level", top_level_path)?;
    validate_compile_script_profile_mode(&none, "none", none_path)?;
    validate_compile_script_profile_compatible(&full, &top_level, "top-level")?;
    validate_compile_script_profile_compatible(&full, &none, "none")?;

    let mut comparisons = Vec::new();
    let mut items = Vec::new();
    let mut violations = Vec::new();
    for full_result in &full.results {
        let top_level_result = find_compile_script_profile_result(&top_level, full_result)?;
        let none_result = find_compile_script_profile_result(&none, full_result)?;
        let comparison = compare_compile_script_profile_result(
            &full.version_line,
            full_result,
            top_level_result,
            none_result,
            min_full_to_none_compile_ratio,
        );
        if !comparison.ast_projection_problem_confirmed {
            violations.push(format!(
                "{} full/none compileScript ratio {:.3} is below threshold {:.3}",
                comparison.name,
                comparison.full_to_none_compile_ratio,
                min_full_to_none_compile_ratio
            ));
        }
        items.push(compat::ReportItem::new(
            format!("{}-{}", full.version_line, comparison.name),
            if comparison.ast_projection_problem_confirmed {
                compat::ReportStatus::Pass
            } else {
                compat::ReportStatus::Fail
            },
            serde_json::to_string(&comparison)?,
            Some(none_path.to_path_buf()),
        ));
        comparisons.push(comparison);
    }

    let report = CompileScriptProfileComparisonReport {
        status: if violations.is_empty() {
            "pass".into()
        } else {
            "fail".into()
        },
        version_line: full.version_line.clone(),
        build_profile: full.build_profile.clone(),
        iterations: full.iterations,
        min_full_to_none_compile_ratio,
        full_report: full_path.display().to_string(),
        top_level_report: top_level_path.display().to_string(),
        none_report: none_path.display().to_string(),
        comparisons,
    };
    let json_path = out_dir.join(format!("{}.comparison.json", full.version_line));
    let markdown_path = out_dir.join(format!("{}.comparison.md", full.version_line));
    fs::write(&json_path, serde_json::to_string_pretty(&report)?)
        .with_context(|| format!("failed to write {}", json_path.display()))?;
    fs::write(
        &markdown_path,
        render_compile_script_profile_comparison_markdown(&report),
    )
    .with_context(|| format!("failed to write {}", markdown_path.display()))?;

    Ok(
        compat::JsonReport::new("compare_compile_script_profile", compat::ReportStatus::Pass)
            .with_items(items)
            .with_violations(violations)
            .with_created(vec![
                json_path.display().to_string(),
                markdown_path.display().to_string(),
            ])
            .with_note("compares full, top-level, and no-AST compileScript profiles; pass means full public AST projection is measurably slower than no-AST for each fixture at the configured threshold"),
    )
}

fn read_compile_script_profile_report(path: &Path) -> Result<CompileScriptProfileReport> {
    let source =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_str(&source)
        .with_context(|| format!("failed to parse compileScript profile {}", path.display()))
}

fn validate_compile_script_profile_mode(
    report: &CompileScriptProfileReport,
    expected: &str,
    path: &Path,
) -> Result<()> {
    if report.script_ast_mode != expected {
        anyhow::bail!(
            "{} has scriptAstMode {}, expected {}",
            path.display(),
            report.script_ast_mode,
            expected
        );
    }
    Ok(())
}

fn validate_compile_script_profile_compatible(
    expected: &CompileScriptProfileReport,
    actual: &CompileScriptProfileReport,
    label: &str,
) -> Result<()> {
    if actual.version_line != expected.version_line {
        anyhow::bail!(
            "{label} profile versionLine {} does not match full profile {}",
            actual.version_line,
            expected.version_line
        );
    }
    if actual.build_profile != expected.build_profile {
        anyhow::bail!(
            "{label} profile buildProfile {} does not match full profile {}",
            actual.build_profile,
            expected.build_profile
        );
    }
    let expected_fixtures = expected
        .fixtures
        .iter()
        .map(|fixture| (&fixture.name, &fixture.sha256))
        .collect::<BTreeMap<_, _>>();
    let actual_fixtures = actual
        .fixtures
        .iter()
        .map(|fixture| (&fixture.name, &fixture.sha256))
        .collect::<BTreeMap<_, _>>();
    if actual_fixtures != expected_fixtures {
        anyhow::bail!("{label} profile fixtures do not match full profile");
    }
    Ok(())
}

fn find_compile_script_profile_result<'a>(
    report: &'a CompileScriptProfileReport,
    needle: &CompileScriptProfileResult,
) -> Result<&'a CompileScriptProfileResult> {
    report
        .results
        .iter()
        .find(|result| result.name == needle.name && result.input_sha256 == needle.input_sha256)
        .with_context(|| {
            format!(
                "profile {} is missing fixture {} ({})",
                report.script_ast_mode, needle.name, needle.input_sha256
            )
        })
}

fn compare_compile_script_profile_result(
    version_line: &str,
    full: &CompileScriptProfileResult,
    top_level: &CompileScriptProfileResult,
    none: &CompileScriptProfileResult,
    min_full_to_none_compile_ratio: f64,
) -> CompileScriptProfileComparison {
    let full_compile = full.compile_script.median_micros;
    let top_level_compile = top_level.compile_script.median_micros;
    let none_compile = none.compile_script.median_micros;
    let full_to_none_compile_ratio = ratio(full_compile, none_compile);
    let full_to_top_level_compile_ratio = ratio(full_compile, top_level_compile);
    CompileScriptProfileComparison {
        name: full.name.clone(),
        version_line: version_line.into(),
        input_sha256: full.input_sha256.clone(),
        full_compile_median_micros: full_compile,
        top_level_compile_median_micros: top_level_compile,
        none_compile_median_micros: none_compile,
        full_to_none_compile_ratio,
        full_to_top_level_compile_ratio,
        none_compile_improvement_percent: percent_reduction(full_compile, none_compile),
        top_level_compile_improvement_percent: percent_reduction(full_compile, top_level_compile),
        full_serialize_median_micros: full.serialize.median_micros,
        none_serialize_median_micros: none.serialize.median_micros,
        full_to_none_serialize_ratio: ratio(
            full.serialize.median_micros,
            none.serialize.median_micros,
        ),
        full_total_median_micros: full.total.median_micros,
        none_total_median_micros: none.total.median_micros,
        full_to_none_total_ratio: ratio(full.total.median_micros, none.total.median_micros),
        ast_projection_statement_count: full.structural_counts.ast_projection_statement_count,
        template_usage_scan_count: none.structural_counts.template_usage_scan_count,
        setup_analysis_count: none.structural_counts.setup_analysis_count,
        ast_projection_problem_confirmed: full_to_none_compile_ratio
            >= min_full_to_none_compile_ratio
            && full.structural_counts.ast_projection_enabled
            && !none.structural_counts.ast_projection_enabled,
    }
}

fn ratio(numerator: u128, denominator: u128) -> f64 {
    if denominator == 0 {
        return f64::INFINITY;
    }
    numerator as f64 / denominator as f64
}

fn percent_reduction(before: u128, after: u128) -> f64 {
    if before == 0 {
        return 0.0;
    }
    ((before as f64 - after as f64) / before as f64) * 100.0
}

fn render_compile_script_profile_comparison_markdown(
    report: &CompileScriptProfileComparisonReport,
) -> String {
    let mut out = String::new();
    out.push_str("# compileScript profile comparison\n\n");
    out.push_str(&format!(
        "- status: `{}`\n- version line: `{}`\n- build profile: `{}`\n- iterations: `{}`\n- minimum full/no-AST compile ratio: `{:.3}`\n\n",
        report.status,
        report.version_line,
        report.build_profile,
        report.iterations,
        report.min_full_to_none_compile_ratio
    ));
    out.push_str("| Fixture | full compile us | top-level compile us | no-AST compile us | full/no-AST | no-AST improvement | AST statements | template scans | setup analyses |\n");
    out.push_str("| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |\n");
    for comparison in &report.comparisons {
        out.push_str(&format!(
            "| `{}` | {} | {} | {} | {:.3}x | {:.1}% | {} | {} | {} |\n",
            comparison.name,
            comparison.full_compile_median_micros,
            comparison.top_level_compile_median_micros,
            comparison.none_compile_median_micros,
            comparison.full_to_none_compile_ratio,
            comparison.none_compile_improvement_percent,
            comparison.ast_projection_statement_count,
            comparison.template_usage_scan_count,
            comparison.setup_analysis_count
        ));
    }
    out
}

fn load_compile_script_profile_fixtures(
    fixture_corpus: &Path,
) -> Result<Vec<CompileScriptProfileFixture>> {
    if !fixture_corpus.is_dir() {
        anyhow::bail!(
            "compileScript fixture corpus {} is not a directory",
            fixture_corpus.display()
        );
    }
    let mut paths = Vec::new();
    collect_compile_script_profile_fixture_paths(fixture_corpus, &mut paths)?;
    paths.sort();
    if paths.is_empty() {
        anyhow::bail!(
            "compileScript fixture corpus {} does not contain .vue files",
            fixture_corpus.display()
        );
    }
    paths
        .into_iter()
        .map(read_compile_script_profile_fixture)
        .collect()
}

fn collect_compile_script_profile_fixture_paths(
    root: &Path,
    paths: &mut Vec<PathBuf>,
) -> Result<()> {
    for entry in fs::read_dir(root).with_context(|| format!("failed to read {}", root.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_compile_script_profile_fixture_paths(&path, paths)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("vue") {
            paths.push(path);
        }
    }
    Ok(())
}

fn read_compile_script_profile_fixture(path: PathBuf) -> Result<CompileScriptProfileFixture> {
    let source =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    compile_script_profile_fixture_from_source(path, source)
}

fn compile_script_profile_fixture_from_source(
    path: PathBuf,
    source: String,
) -> Result<CompileScriptProfileFixture> {
    let name = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(ToOwned::to_owned)
        .with_context(|| format!("fixture path {} has no UTF-8 file stem", path.display()))?;
    let (template_bytes, script_bytes, script_setup_bytes) =
        compile_script_fixture_block_sizes(&source);
    Ok(CompileScriptProfileFixture {
        name,
        path,
        sha256: sha256_bytes(source.as_bytes()),
        source_bytes: source.len(),
        template_bytes,
        script_bytes,
        script_setup_bytes,
        source,
    })
}

fn compile_script_profile_fixture(
    version: CompileScriptProfileVersion,
    fixture: &CompileScriptProfileFixture,
    iterations: usize,
    script_ast_mode: vuec_sfc::SfcScriptAstMode,
) -> Result<CompileScriptProfileResult> {
    let mut parse_samples = Vec::with_capacity(iterations);
    let mut compile_samples = Vec::with_capacity(iterations);
    let mut serialize_samples = Vec::with_capacity(iterations);
    let mut total_samples = Vec::with_capacity(iterations);
    let mut output_bytes = 0usize;
    let mut errors = 0usize;
    let mut warnings = 0usize;
    let mut structural_counts = CompileScriptStructuralCounts::default();

    for _ in 0..iterations {
        let total_started = Instant::now();
        let mut compiler = vuec_sfc::SfcCompiler::new();
        let filename = fixture.path.display().to_string();

        let parse_started = Instant::now();
        let descriptor = match version {
            CompileScriptProfileVersion::Vue27 => {
                compiler
                    .parse_vue27_component_with_filename(
                        filename,
                        &fixture.source,
                        vuec_sfc::Vue27ParseComponentOptions::default(),
                    )
                    .descriptor
            }
            CompileScriptProfileVersion::Vue3 => {
                compiler.parse_vue3(filename, &fixture.source).descriptor
            }
        };
        parse_samples.push(parse_started.elapsed().as_micros());

        let compile_started = Instant::now();
        let script = match version {
            CompileScriptProfileVersion::Vue27 => compiler
                .compile_vue27_script(&descriptor, compile_script_profile_options(script_ast_mode)),
            CompileScriptProfileVersion::Vue3 => compiler
                .compile_script(&descriptor, compile_script_profile_options(script_ast_mode)),
        };
        compile_samples.push(compile_started.elapsed().as_micros());

        let serialize_started = Instant::now();
        let serialized = serde_json::to_vec(&script)?;
        serialize_samples.push(serialize_started.elapsed().as_micros());
        total_samples.push(total_started.elapsed().as_micros());

        output_bytes = serialized.len();
        errors = script.errors.len();
        warnings = script.warnings.len();
        structural_counts =
            compile_script_structural_counts(version, &descriptor, &script, script_ast_mode);
    }

    Ok(CompileScriptProfileResult {
        name: fixture.name.clone(),
        version_line: version.canonical().into(),
        iterations,
        parse: profile_phase(parse_samples),
        compile_script: profile_phase(compile_samples),
        serialize: profile_phase(serialize_samples),
        total: profile_phase(total_samples),
        output_bytes,
        errors,
        warnings,
        structural_counts,
        input_sha256: fixture.sha256.clone(),
    })
}

fn compile_script_profile_options(
    script_ast_mode: vuec_sfc::SfcScriptAstMode,
) -> vuec_sfc::SfcScriptCompileOptions {
    vuec_sfc::SfcScriptCompileOptions {
        script_ast_mode,
        ..vuec_sfc::SfcScriptCompileOptions::default()
    }
}

fn profile_phase(samples: Vec<u128>) -> CompileScriptPhaseProfile {
    CompileScriptPhaseProfile {
        median_micros: median_micros(&samples),
        p95_micros: p95_micros(&samples),
    }
}

fn median_micros(samples: &[u128]) -> u128 {
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    match sorted.len() {
        0 => 0,
        len if len % 2 == 1 => sorted[len / 2],
        len => (sorted[len / 2 - 1] + sorted[len / 2]) / 2,
    }
}

fn p95_micros(samples: &[u128]) -> u128 {
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    if sorted.is_empty() {
        return 0;
    }
    let index = ((sorted.len() * 95 + 99) / 100).saturating_sub(1);
    sorted[index]
}

fn compile_script_structural_counts(
    version: CompileScriptProfileVersion,
    descriptor: &vuec_sfc::SfcDescriptor,
    script: &vuec_sfc::SfcScriptBlock,
    ast_mode: vuec_sfc::SfcScriptAstMode,
) -> CompileScriptStructuralCounts {
    let has_js_like_script = descriptor
        .script
        .as_ref()
        .is_some_and(compile_script_block_is_js_like)
        || descriptor
            .script_setup
            .as_ref()
            .is_some_and(compile_script_block_is_js_like);
    let ast_projection_enabled =
        has_js_like_script && !matches!(ast_mode, vuec_sfc::SfcScriptAstMode::None);
    CompileScriptStructuralCounts {
        ast_projection_enabled,
        ast_projection_mode: compile_script_ast_mode_name(ast_mode).into(),
        ast_projection_loc_strategy: if ast_projection_enabled {
            "line-index".into()
        } else {
            "not-run".into()
        },
        ast_projection_statement_count: script.script_ast.len() + script.script_setup_ast.len(),
        template_usage_scan_count: compile_script_template_usage_scan_count(version, descriptor),
        setup_analysis_count: compile_script_setup_analysis_count(version, descriptor),
        script_compile_error_analysis_count: compile_script_error_analysis_count(
            version, descriptor,
        ),
    }
}

fn compile_script_ast_mode_name(mode: vuec_sfc::SfcScriptAstMode) -> &'static str {
    match mode {
        vuec_sfc::SfcScriptAstMode::None => "none",
        vuec_sfc::SfcScriptAstMode::TopLevel => "top-level",
        vuec_sfc::SfcScriptAstMode::Full => "full",
    }
}

fn compile_script_template_usage_scan_count(
    version: CompileScriptProfileVersion,
    descriptor: &vuec_sfc::SfcDescriptor,
) -> usize {
    if descriptor.script_setup.is_none() {
        return 0;
    }
    let Some(template) = descriptor.template.as_ref() else {
        return 0;
    };
    if template.attrs.src.is_some() || template.attrs.lang.is_some() {
        return 0;
    }
    match version {
        CompileScriptProfileVersion::Vue27 => 1,
        CompileScriptProfileVersion::Vue3 => usize::from(
            descriptor
                .script_setup
                .as_ref()
                .is_some_and(compile_script_block_is_js_like),
        ),
    }
}

fn compile_script_setup_analysis_count(
    version: CompileScriptProfileVersion,
    descriptor: &vuec_sfc::SfcDescriptor,
) -> usize {
    if descriptor.script_setup.is_none() {
        return 0;
    }
    match version {
        CompileScriptProfileVersion::Vue27 => 1,
        CompileScriptProfileVersion::Vue3 => 1,
    }
}

fn compile_script_error_analysis_count(
    version: CompileScriptProfileVersion,
    _descriptor: &vuec_sfc::SfcDescriptor,
) -> usize {
    match version {
        CompileScriptProfileVersion::Vue27 => 0,
        CompileScriptProfileVersion::Vue3 => 1,
    }
}

fn compile_script_block_is_js_like(block: &vuec_sfc::SfcBlock) -> bool {
    block
        .attrs
        .lang
        .as_deref()
        .map(|lang| matches!(lang, "js" | "jsx" | "ts" | "tsx"))
        .unwrap_or(true)
}

fn compile_script_fixture_block_sizes(source: &str) -> (usize, usize, usize) {
    (
        sfc_block_content_bytes(source, "template", None),
        sfc_block_content_bytes(source, "script", Some(false)),
        sfc_block_content_bytes(source, "script", Some(true)),
    )
}

fn sfc_block_content_bytes(source: &str, tag: &str, setup: Option<bool>) -> usize {
    let lower = source.to_ascii_lowercase();
    let mut cursor = 0usize;
    let mut total = 0usize;
    let open_tag = format!("<{tag}");
    let close_tag = format!("</{tag}>");
    while let Some(relative_start) = lower[cursor..].find(&open_tag) {
        let start = cursor + relative_start;
        let Some(relative_open_end) = lower[start..].find('>') else {
            break;
        };
        let open_end = start + relative_open_end;
        let open = &lower[start..=open_end];
        if setup.is_none_or(|expected| sfc_script_open_has_setup(open) == expected) {
            let content_start = open_end + 1;
            let Some(relative_close_start) = lower[content_start..].find(&close_tag) else {
                break;
            };
            let content_end = content_start + relative_close_start;
            total += content_end.saturating_sub(content_start);
            cursor = content_end + close_tag.len();
        } else {
            cursor = open_end + 1;
        }
    }
    total
}

fn sfc_script_open_has_setup(open: &str) -> bool {
    open.split(|ch: char| ch.is_ascii_whitespace() || matches!(ch, '>' | '/'))
        .any(|part| part == "setup" || part.starts_with("setup="))
}

fn compile_script_build_profile() -> &'static str {
    if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    }
}
