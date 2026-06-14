#[derive(Parser)]
#[command(author, version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    VerifyOfficialLock {
        #[arg(long, default_value = "compat/official-revisions.lock")]
        path: PathBuf,
        #[arg(long, default_value = "vendor")]
        vendor_dir: PathBuf,
        #[arg(long)]
        require_vendor: bool,
    },
    SyncOfficialTests {
        #[arg(long, default_value = "compat/official-revisions.lock")]
        lock: PathBuf,
        #[arg(long)]
        locked: bool,
        #[arg(long, default_value = "vendor")]
        out_dir: PathBuf,
    },
    PrepareRuntimeSmoke {
        #[arg(long, default_value = "compat/official-revisions.lock")]
        lock: PathBuf,
        #[arg(long, default_value = "vendor")]
        vendor_dir: PathBuf,
    },
    ExportApi {
        #[command(flatten)]
        scope: SelectionArgs,
        #[arg(long, default_value = "compat")]
        out_dir: PathBuf,
    },
    DiffApi {
        #[command(flatten)]
        scope: SelectionArgs,
    },
    GenerateOptionMatrix {
        #[command(flatten)]
        scope: SelectionArgs,
        #[arg(long, default_value = "compat")]
        out_dir: PathBuf,
    },
    AuditOptionMatrix {
        #[command(flatten)]
        scope: SelectionArgs,
    },
    RunOptionMatrix {
        #[command(flatten)]
        scope: SelectionArgs,
    },
    RunNapiOptionMatrix {
        #[command(flatten)]
        scope: SelectionArgs,
    },
    RunConformance {
        #[command(flatten)]
        args: ConformanceArgs,
    },
    RunNapiConformance {
        #[command(flatten)]
        args: ConformanceArgs,
    },
    GenerateOutputContract {
        #[command(flatten)]
        scope: SelectionArgs,
        #[arg(long, default_value = "compat")]
        out_dir: PathBuf,
    },
    RunOutputContract {
        #[command(flatten)]
        scope: SelectionArgs,
    },
    RunNapiOutputContract {
        #[command(flatten)]
        scope: SelectionArgs,
    },
    VerifyNpmAlias {
        #[command(flatten)]
        scope: SelectionArgs,
    },
    VerifyNapi,
    VerifyNapiAlias,
    VerifyNapiApi,
    VerifyNapiPlatform,
    VerifyWasm,
    VerifyWasmBrowser,
    VerifyWasmWasi,
    VerifyCli,
    VerifyIncremental,
    VerifyParallel,
    VerifyAstCache,
    VerifyArena,
    VerifyStringInterning,
    VerifyReleaseDocs,
    VerifyPublicApiDocs,
    VerifyCrateMetadata,
    VerifySupplyChain,
    VerifyCiStatus {
        #[arg(long)]
        repo: Option<String>,
        #[arg(long)]
        commit: Option<String>,
        #[arg(long, default_value = "ci.yml")]
        workflow: String,
        #[arg(long)]
        runs_json: Option<PathBuf>,
        #[arg(long)]
        jobs_json: Option<PathBuf>,
    },
    VerifyReleaseDryRun {
        #[arg(long)]
        native_artifacts_dir: Option<PathBuf>,
    },
    VerifyReleaseInstallSmoke {
        #[arg(long)]
        native_artifacts_dir: Option<PathBuf>,
        #[arg(long)]
        current_platform_only: bool,
    },
    VerifyVue27ProjectCorpus {
        #[command(flatten)]
        args: Vue27ProjectCorpusArgs,
    },
    VerifyVue2ProjectCorpus {
        #[command(flatten)]
        args: Vue2ProjectCorpusArgs,
    },
    Bench {
        #[arg(long, default_value_t = 10)]
        iterations: usize,
        #[arg(long, default_value = "target/bench")]
        out_dir: PathBuf,
        #[arg(long, default_value = "compat/official-revisions.lock")]
        lock: PathBuf,
        #[arg(long)]
        skip_official_js: bool,
    },
    ProfileCompileScript {
        #[arg(long, default_value = "vue2_7")]
        version_line: String,
        #[arg(long, default_value = "compat/perf/vue27-sfc")]
        fixture_corpus: PathBuf,
        #[arg(long, default_value_t = 60)]
        iterations: usize,
        #[arg(long, default_value = "none")]
        script_ast_mode: CompileScriptProfileAstMode,
        #[arg(long, default_value = "target/perf/compile-script")]
        out_dir: PathBuf,
    },
    CompareCompileScriptProfile {
        #[arg(long)]
        full: PathBuf,
        #[arg(long)]
        top_level: PathBuf,
        #[arg(long)]
        none: PathBuf,
        #[arg(long, default_value = "target/perf/compile-script-comparison")]
        out_dir: PathBuf,
        #[arg(long, default_value_t = 1.2)]
        min_full_to_none_compile_ratio: f64,
    },
    SummarizeCompat {
        #[arg(long)]
        locked: bool,
        #[arg(long, default_value = "compat/official-revisions.lock")]
        lock: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let report = match cli.command {
        Command::VerifyOfficialLock {
            path,
            vendor_dir,
            require_vendor,
        } => verify_official_lock(&path, &vendor_dir, require_vendor),
        Command::SyncOfficialTests {
            lock,
            locked,
            out_dir,
        } => sync_official_tests(&lock, locked, &out_dir),
        Command::PrepareRuntimeSmoke { lock, vendor_dir } => {
            prepare_runtime_smoke(&lock, &vendor_dir)
        }
        Command::ExportApi { scope, out_dir } => export_api(&scope, &out_dir),
        Command::DiffApi { scope } => diff_api(&scope),
        Command::GenerateOptionMatrix { scope, out_dir } => {
            generate_option_matrix(&scope, &out_dir)
        }
        Command::AuditOptionMatrix { scope } => audit_option_matrix(&scope),
        Command::RunOptionMatrix { scope } => run_option_matrix(&scope),
        Command::RunNapiOptionMatrix { scope } => run_napi_option_matrix(&scope),
        Command::RunConformance { args } => run_conformance(&args),
        Command::RunNapiConformance { args } => run_napi_conformance(&args),
        Command::GenerateOutputContract { scope, out_dir } => {
            generate_output_contract(&scope, &out_dir)
        }
        Command::RunOutputContract { scope } => run_output_contract(&scope),
        Command::RunNapiOutputContract { scope } => run_napi_output_contract(&scope),
        Command::VerifyNpmAlias { scope } => verify_npm_alias(&scope),
        Command::VerifyNapi => verify_napi()?,
        Command::VerifyNapiAlias => verify_napi_alias()?,
        Command::VerifyNapiApi => verify_napi_api()?,
        Command::VerifyNapiPlatform => verify_napi_platform()?,
        Command::VerifyWasm => verify_wasm()?,
        Command::VerifyWasmBrowser => verify_wasm_browser()?,
        Command::VerifyWasmWasi => verify_wasm_wasi()?,
        Command::VerifyCli => verify_cli()?,
        Command::VerifyIncremental => verify_incremental()?,
        Command::VerifyParallel => verify_parallel()?,
        Command::VerifyAstCache => verify_ast_cache()?,
        Command::VerifyArena => verify_arena()?,
        Command::VerifyStringInterning => verify_string_interning()?,
        Command::VerifyReleaseDocs => verify_release_docs()?,
        Command::VerifyPublicApiDocs => verify_public_api_docs()?,
        Command::VerifyCrateMetadata => verify_crate_metadata()?,
        Command::VerifySupplyChain => verify_supply_chain()?,
        Command::VerifyCiStatus {
            repo,
            commit,
            workflow,
            runs_json,
            jobs_json,
        } => verify_ci_status(
            repo.as_deref(),
            commit.as_deref(),
            &workflow,
            runs_json.as_deref(),
            jobs_json.as_deref(),
        )?,
        Command::VerifyReleaseDryRun {
            native_artifacts_dir,
        } => verify_release_dry_run(native_artifacts_dir.as_deref())?,
        Command::VerifyReleaseInstallSmoke {
            native_artifacts_dir,
            current_platform_only,
        } => verify_release_install_smoke(native_artifacts_dir.as_deref(), current_platform_only)?,
        Command::VerifyVue27ProjectCorpus { args } => verify_vue27_project_corpus(&args),
        Command::VerifyVue2ProjectCorpus { args } => verify_vue2_project_corpus(&args),
        Command::Bench {
            iterations,
            out_dir,
            lock,
            skip_official_js,
        } => bench(iterations, &out_dir, &lock, skip_official_js)?,
        Command::ProfileCompileScript {
            version_line,
            fixture_corpus,
            iterations,
            script_ast_mode,
            out_dir,
        } => profile_compile_script(
            &version_line,
            &fixture_corpus,
            iterations,
            script_ast_mode.into(),
            &out_dir,
        )?,
        Command::CompareCompileScriptProfile {
            full,
            top_level,
            none,
            out_dir,
            min_full_to_none_compile_ratio,
        } => compare_compile_script_profile(
            &full,
            &top_level,
            &none,
            &out_dir,
            min_full_to_none_compile_ratio,
        )?,
        Command::SummarizeCompat { locked, lock } => summarize_compat(locked, &lock),
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    if report.status != "pass" && report.status != "pending" {
        std::process::exit(1);
    }
    Ok(())
}
