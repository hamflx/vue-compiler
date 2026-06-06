//! Project automation for compatibility, release, benchmark, and verification gates.
//!
//! The binary hosts deterministic `cargo xtask ...` commands used by the
//! development plan. It orchestrates official fixture sync, API/option/output
//! contract checks, conformance reports, release documentation gates, and
//! targeted verification helpers without owning compiler semantics.

#![forbid(unsafe_code)]

mod compat;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use compat::{
    audit_option_matrix, diff_api, export_api, generate_option_matrix, generate_output_contract,
    prepare_runtime_smoke, run_conformance, run_napi_conformance, run_napi_option_matrix,
    run_napi_output_contract, run_option_matrix, run_output_contract, summarize_compat,
    sync_official_tests, verify_npm_alias, verify_official_lock, verify_vue27_project_corpus,
    verify_vue2_project_corpus, ConformanceArgs, SelectionArgs, Vue27ProjectCorpusArgs,
    Vue2ProjectCorpusArgs,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, ExitStatus, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{thread, time::Duration};
use sysinfo::{Pid, ProcessesToUpdate, System};

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
        Command::ExportApi { scope, out_dir } => {
            let report = export_api(&scope);
            ensure_dir(&out_dir)?;
            report
        }
        Command::DiffApi { scope } => diff_api(&scope),
        Command::GenerateOptionMatrix { scope, out_dir } => {
            let report = generate_option_matrix(&scope);
            ensure_dir(&out_dir)?;
            report
        }
        Command::AuditOptionMatrix { scope } => audit_option_matrix(&scope),
        Command::RunOptionMatrix { scope } => run_option_matrix(&scope),
        Command::RunNapiOptionMatrix { scope } => run_napi_option_matrix(&scope),
        Command::RunConformance { args } => run_conformance(&args),
        Command::RunNapiConformance { args } => run_napi_conformance(&args),
        Command::GenerateOutputContract { scope, out_dir } => {
            let report = generate_output_contract(&scope);
            ensure_dir(&out_dir)?;
            report
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
        Command::SummarizeCompat { locked, lock } => summarize_compat(locked, &lock),
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    if report.status != "pass" && report.status != "pending" {
        std::process::exit(1);
    }
    Ok(())
}

fn verify_napi_api() -> Result<compat::JsonReport> {
    let targets = [
        NapiApiTarget {
            version_line: "vue2_6",
            package: "vue-template-compiler",
            entry: "index",
            alias: NapiApiAlias::Vue2TemplateCompiler {
                template_variant: "vue2_6",
            },
        },
        NapiApiTarget {
            version_line: "vue2_7",
            package: "vue-template-compiler",
            entry: "index",
            alias: NapiApiAlias::Vue2TemplateCompiler {
                template_variant: "vue2_7",
            },
        },
        NapiApiTarget {
            version_line: "vue2_7",
            package: "vue/compiler-sfc",
            entry: "vue/compiler-sfc",
            alias: NapiApiAlias::PackageTemplate {
                source: "packages/native-aliases/vue",
                package_subpath: &["vue"],
                manifest_package: "vue",
                manifest_file: "vue_compiler-sfc.json",
                package_json_subpath: &["vue"],
                types_base_subpath: &["vue"],
            },
        },
        NapiApiTarget {
            version_line: "vue3",
            package: "@vue/compiler-core",
            entry: "index",
            alias: NapiApiAlias::PackageTemplate {
                source: "packages/native-aliases/@vue/compiler-core",
                package_subpath: &["@vue", "compiler-core"],
                manifest_package: "_vue_compiler-core",
                manifest_file: "index.json",
                package_json_subpath: &["@vue", "compiler-core"],
                types_base_subpath: &["@vue", "compiler-core"],
            },
        },
        NapiApiTarget {
            version_line: "vue3",
            package: "@vue/compiler-ssr",
            entry: "index",
            alias: NapiApiAlias::PackageTemplate {
                source: "packages/native-aliases/@vue/compiler-ssr",
                package_subpath: &["@vue", "compiler-ssr"],
                manifest_package: "_vue_compiler-ssr",
                manifest_file: "index.json",
                package_json_subpath: &["@vue", "compiler-ssr"],
                types_base_subpath: &["@vue", "compiler-ssr"],
            },
        },
        NapiApiTarget {
            version_line: "vue3",
            package: "@vue/compiler-sfc",
            entry: "index",
            alias: NapiApiAlias::PackageTemplate {
                source: "packages/native-aliases/@vue/compiler-sfc",
                package_subpath: &["@vue", "compiler-sfc"],
                manifest_package: "_vue_compiler-sfc",
                manifest_file: "index.json",
                package_json_subpath: &["@vue", "compiler-sfc"],
                types_base_subpath: &["@vue", "compiler-sfc"],
            },
        },
        NapiApiTarget {
            version_line: "vue3",
            package: "@vue/compiler-dom",
            entry: "index",
            alias: NapiApiAlias::PackageTemplate {
                source: "packages/native-aliases/@vue/compiler-dom",
                package_subpath: &["@vue", "compiler-dom"],
                manifest_package: "_vue_compiler-dom",
                manifest_file: "index.json",
                package_json_subpath: &["@vue", "compiler-dom"],
                types_base_subpath: &["@vue", "compiler-dom"],
            },
        },
    ];
    let mut violations = Vec::new();
    let mut created = Vec::new();
    let mut items = Vec::new();

    let build_failure = build_napi_crate()
        .err()
        .map(|err| format!("failed to build vuec_napi: {err:#}"));
    if let Some(err) = &build_failure {
        violations.push(err.clone());
    }

    for target in targets {
        let mut target_violations = Vec::new();
        let root = PathBuf::from("target")
            .join("napi-api")
            .join(target.version_line)
            .join(target.target_dir_name());
        let binding_path = root
            .join("node_modules")
            .join("@vuec-rs")
            .join("native")
            .join("vuec_napi.node");
        if build_failure.is_none() {
            match prepare_napi_api_tree(&root, target) {
                Ok(paths) => {
                    created.extend(paths.into_iter().map(|path| path.display().to_string()))
                }
                Err(err) => target_violations.push(format!(
                    "{} failed to prepare NAPI API tree: {err:#}",
                    target.display()
                )),
            }
        }
        if build_failure.is_none() && target_violations.is_empty() {
            match copy_napi_binding(&binding_path) {
                Ok(path) => created.push(path.display().to_string()),
                Err(err) => target_violations.push(format!(
                    "{} failed to install NAPI binding: {err:#}",
                    target.display()
                )),
            }
        }

        let detail = if build_failure.is_none() && target_violations.is_empty() {
            match run_napi_api_probe(&root, target) {
                Ok(detail) => detail,
                Err(err) => {
                    target_violations.push(format!(
                        "{} NAPI API diff failed: {err:#}",
                        target.display()
                    ));
                    "NAPI API diff did not pass".into()
                }
            }
        } else {
            "NAPI API diff did not run".into()
        };
        let status = if build_failure.is_none() && target_violations.is_empty() {
            compat::ReportStatus::Pass
        } else {
            compat::ReportStatus::Fail
        };
        violations.extend(target_violations);
        items.push(compat::ReportItem::new(
            target.display(),
            status,
            detail,
            Some(root),
        ));
    }

    Ok(compat::JsonReport::new(
        "verify_napi_api",
        if violations.is_empty() {
            compat::ReportStatus::Pass
        } else {
            compat::ReportStatus::Fail
        },
    )
    .with_items(items)
    .with_created(created)
    .with_violations(violations)
    .with_note("compares official API manifests against NAPI-backed official package-name aliases"))
}

#[derive(Clone, Copy)]
struct NapiApiTarget {
    version_line: &'static str,
    package: &'static str,
    entry: &'static str,
    alias: NapiApiAlias,
}

#[derive(Clone, Copy)]
enum NapiApiAlias {
    Vue2TemplateCompiler {
        template_variant: &'static str,
    },
    PackageTemplate {
        source: &'static str,
        package_subpath: &'static [&'static str],
        manifest_package: &'static str,
        manifest_file: &'static str,
        package_json_subpath: &'static [&'static str],
        types_base_subpath: &'static [&'static str],
    },
}

impl NapiApiTarget {
    fn display(self) -> String {
        format!("{}::{}/{}", self.version_line, self.package, self.entry)
    }

    fn target_dir_name(self) -> &'static str {
        match self.alias {
            NapiApiAlias::Vue2TemplateCompiler { .. } => "vue-template-compiler",
            NapiApiAlias::PackageTemplate {
                manifest_package, ..
            } => manifest_package,
        }
    }

    fn source_path(self) -> PathBuf {
        match self.alias {
            NapiApiAlias::Vue2TemplateCompiler { .. } => {
                PathBuf::from("packages/native-aliases/vue-template-compiler")
            }
            NapiApiAlias::PackageTemplate { source, .. } => PathBuf::from(source),
        }
    }

    fn package_subpath(self) -> &'static [&'static str] {
        match self.alias {
            NapiApiAlias::Vue2TemplateCompiler { .. } => &["vue-template-compiler"],
            NapiApiAlias::PackageTemplate {
                package_subpath, ..
            } => package_subpath,
        }
    }

    fn package_json_subpath(self) -> &'static [&'static str] {
        match self.alias {
            NapiApiAlias::Vue2TemplateCompiler { .. } => &["vue-template-compiler"],
            NapiApiAlias::PackageTemplate {
                package_json_subpath,
                ..
            } => package_json_subpath,
        }
    }

    fn types_base_subpath(self) -> &'static [&'static str] {
        match self.alias {
            NapiApiAlias::Vue2TemplateCompiler { .. } => &["vue-template-compiler"],
            NapiApiAlias::PackageTemplate {
                types_base_subpath, ..
            } => types_base_subpath,
        }
    }

    fn official_manifest_path(self) -> PathBuf {
        let (manifest_package, manifest_file) = match self.alias {
            NapiApiAlias::Vue2TemplateCompiler { .. } => ("vue-template-compiler", "index.json"),
            NapiApiAlias::PackageTemplate {
                manifest_package, ..
            } => (manifest_package, self.manifest_file_name()),
        };
        PathBuf::from("compat")
            .join("api")
            .join("official")
            .join(self.version_line)
            .join(manifest_package)
            .join(manifest_file)
    }

    fn manifest_file_name(self) -> &'static str {
        match self.alias {
            NapiApiAlias::Vue2TemplateCompiler { .. } => "index.json",
            NapiApiAlias::PackageTemplate { manifest_file, .. } => manifest_file,
        }
    }
}

fn verify_napi_platform() -> Result<compat::JsonReport> {
    let mut violations = Vec::new();
    let mut created = Vec::new();
    let platform_root = PathBuf::from("target").join("napi-platform");
    let node_modules = platform_root.join("node_modules");
    let native_package_dir = node_modules.join("@vuec-rs").join("native");
    let platform_package = current_platform_package_name();
    let platform_package_dir =
        platform_package_path(&node_modules, platform_package.unwrap_or("unsupported"));

    match build_napi_crate() {
        Ok(()) => {}
        Err(err) => violations.push(format!("failed to build vuec_napi: {err:#}")),
    }

    if platform_package.is_none() {
        violations.push(format!(
            "unsupported NAPI platform package for os={} arch={}",
            std::env::consts::OS,
            std::env::consts::ARCH
        ));
    }

    if violations.is_empty() {
        match prepare_napi_platform_tree(&platform_root, platform_package.unwrap()) {
            Ok(paths) => created.extend(paths.into_iter().map(|path| path.display().to_string())),
            Err(err) => {
                violations.push(format!("failed to prepare NAPI platform package: {err:#}"))
            }
        }
    }

    if violations.is_empty() {
        match copy_napi_binding(&platform_package_dir.join("vuec_napi.node")) {
            Ok(path) => created.push(path.display().to_string()),
            Err(err) => {
                violations.push(format!("failed to install platform NAPI binding: {err:#}"))
            }
        }
    }

    let smoke_output = if violations.is_empty() {
        match run_napi_platform_smoke(&platform_root) {
            Ok(output) => Some(output),
            Err(err) => {
                violations.push(format!("NAPI platform package smoke failed: {err:#}"));
                None
            }
        }
    } else {
        None
    };

    let item_status = if violations.is_empty() {
        compat::ReportStatus::Pass
    } else {
        compat::ReportStatus::Fail
    };
    Ok(
        compat::JsonReport::new("verify_napi_platform", item_status)
            .with_items(vec![compat::ReportItem::new(
                platform_package.unwrap_or("unsupported-platform"),
                item_status,
                smoke_output.unwrap_or_else(|| "NAPI platform package smoke did not run".into()),
                Some(native_package_dir),
            )])
            .with_created(created)
            .with_violations(violations)
            .with_note("builds vuec_napi, installs the current optional platform package under target/napi-platform, and verifies @vuec-rs/native loads from that package instead of a local .node"),
    )
}

fn verify_napi_alias() -> Result<compat::JsonReport> {
    let mut violations = Vec::new();
    let mut created = Vec::new();
    let alias_root = PathBuf::from("target").join("napi-alias");
    let node_modules = alias_root.join("node_modules");
    let native_package_dir = node_modules.join("@vuec-rs").join("native");
    let binding_path = native_package_dir.join("vuec_napi.node");

    match build_napi_crate() {
        Ok(()) => {}
        Err(err) => violations.push(format!("failed to build vuec_napi: {err:#}")),
    }

    if violations.is_empty() {
        match prepare_napi_alias_tree(&alias_root) {
            Ok(paths) => created.extend(paths.into_iter().map(|path| path.display().to_string())),
            Err(err) => violations.push(format!("failed to prepare NAPI alias packages: {err:#}")),
        }
    }

    if violations.is_empty() {
        match copy_napi_binding(&binding_path) {
            Ok(path) => created.push(path.display().to_string()),
            Err(err) => violations.push(format!("failed to install NAPI binding: {err:#}")),
        }
    }

    let smoke_output = if violations.is_empty() {
        match run_napi_alias_smoke(&alias_root) {
            Ok(output) => Some(output),
            Err(err) => {
                violations.push(format!("NAPI alias smoke failed: {err:#}"));
                None
            }
        }
    } else {
        None
    };

    let item_status = if violations.is_empty() {
        compat::ReportStatus::Pass
    } else {
        compat::ReportStatus::Fail
    };
    Ok(
        compat::JsonReport::new("verify_napi_alias", item_status)
            .with_items(vec![compat::ReportItem::new(
                "official-package-name-napi-alias",
                item_status,
                smoke_output.unwrap_or_else(|| "NAPI alias smoke did not run".into()),
                Some(alias_root),
            )])
            .with_created(created)
            .with_violations(violations)
            .with_note("builds vuec_napi, installs @vuec-rs/native plus official package-name alias templates under target/napi-alias, and requires them from Node"),
    )
}

fn ensure_dir(path: &PathBuf) -> Result<()> {
    std::fs::create_dir_all(path)?;
    Ok(())
}

fn verify_napi() -> Result<compat::JsonReport> {
    let mut violations = Vec::new();
    let mut created = Vec::new();
    let package_dir = PathBuf::from("packages/native");
    let binding_path = package_dir.join("vuec_napi.node");

    match build_napi_crate() {
        Ok(()) => {}
        Err(err) => violations.push(format!("failed to build vuec_napi: {err:#}")),
    }

    if violations.is_empty() {
        match copy_napi_binding(&binding_path) {
            Ok(path) => created.push(path.display().to_string()),
            Err(err) => violations.push(format!("failed to install NAPI binding: {err:#}")),
        }
    }

    let smoke_output = if violations.is_empty() {
        match run_native_smoke(&package_dir) {
            Ok(output) => Some(output),
            Err(err) => {
                violations.push(format!("native smoke failed: {err:#}"));
                None
            }
        }
    } else {
        None
    };

    let item_status = if violations.is_empty() {
        compat::ReportStatus::Pass
    } else {
        compat::ReportStatus::Fail
    };
    let item_detail = smoke_output.unwrap_or_else(|| "NAPI smoke did not run".into());
    Ok(
        compat::JsonReport::new("verify_napi", item_status)
            .with_items(vec![compat::ReportItem::new(
                "@vuec-rs/native",
                item_status,
                item_detail,
                Some(binding_path),
            )])
            .with_created(created)
            .with_violations(violations)
            .with_note("builds vuec_napi, installs packages/native/vuec_napi.node, and runs the Node loader smoke"),
    )
}

fn verify_wasm() -> Result<compat::JsonReport> {
    let mut items = Vec::new();
    let mut violations = Vec::new();
    let mut created = Vec::new();

    let rust_status = match run_cargo(&["test", "-p", "vuec_wasm"]) {
        Ok(output) => {
            items.push(compat::ReportItem::new(
                "vuec_wasm-rust-api",
                compat::ReportStatus::Pass,
                output,
                Some(PathBuf::from("crates/vuec_wasm")),
            ));
            compat::ReportStatus::Pass
        }
        Err(err) => {
            violations.push(format!("vuec_wasm Rust tests failed: {err:#}"));
            items.push(compat::ReportItem::new(
                "vuec_wasm-rust-api",
                compat::ReportStatus::Fail,
                format!("{err:#}"),
                Some(PathBuf::from("crates/vuec_wasm")),
            ));
            compat::ReportStatus::Fail
        }
    };

    let wasm_status = match build_wasm_package() {
        Ok(paths) => {
            created.extend(paths.into_iter().map(|path| path.display().to_string()));
            match run_wasm_smoke() {
                Ok(output) => {
                    items.push(compat::ReportItem::new(
                        "@vuec-rs/wasm-node-smoke",
                        compat::ReportStatus::Pass,
                        output,
                        Some(PathBuf::from("packages/wasm")),
                    ));
                    compat::ReportStatus::Pass
                }
                Err(err) => {
                    violations.push(format!("WASM Node smoke failed: {err:#}"));
                    items.push(compat::ReportItem::new(
                        "@vuec-rs/wasm-node-smoke",
                        compat::ReportStatus::Fail,
                        format!("{err:#}"),
                        Some(PathBuf::from("packages/wasm")),
                    ));
                    compat::ReportStatus::Fail
                }
            }
        }
        Err(err) => {
            violations.push(format!("failed to build wasm package: {err:#}"));
            items.push(compat::ReportItem::new(
                "@vuec-rs/wasm-build",
                compat::ReportStatus::Fail,
                format!("{err:#}"),
                Some(PathBuf::from("packages/wasm/pkg-node")),
            ));
            compat::ReportStatus::Fail
        }
    };

    let status =
        if rust_status == compat::ReportStatus::Pass && wasm_status == compat::ReportStatus::Pass {
            compat::ReportStatus::Pass
        } else {
            compat::ReportStatus::Fail
        };
    Ok(
        compat::JsonReport::new("verify_wasm", status)
            .with_items(items)
            .with_created(created)
            .with_violations(violations)
            .with_note("runs vuec_wasm Rust API tests, builds the wasm-bindgen package when the wasm target/tooling is installed, and executes the @vuec-rs/wasm Node smoke; browser coverage is handled by verify-wasm-browser and WASI coverage remains a separate gate"),
    )
}

fn verify_wasm_browser() -> Result<compat::JsonReport> {
    let mut violations = Vec::new();
    let mut items = Vec::new();
    let status = match run_wasm_pack_browser_test() {
        Ok(output) => {
            items.push(compat::ReportItem::new(
                "vuec_wasm-browser-smoke",
                compat::ReportStatus::Pass,
                output,
                Some(PathBuf::from("crates/vuec_wasm")),
            ));
            compat::ReportStatus::Pass
        }
        Err(err) => {
            violations.push(format!("WASM browser smoke failed: {err:#}"));
            items.push(compat::ReportItem::new(
                "vuec_wasm-browser-smoke",
                compat::ReportStatus::Fail,
                format!("{err:#}"),
                Some(PathBuf::from("crates/vuec_wasm")),
            ));
            compat::ReportStatus::Fail
        }
    };

    Ok(
        compat::JsonReport::new("verify_wasm_browser", status)
            .with_items(items)
            .with_violations(violations)
            .with_note("runs `wasm-pack test --headless --chrome crates/vuec_wasm`; requires wasm-pack plus Chrome, and supports VUEC_WASM_CHROME/VUEC_WASM_CHROMEDRIVER overrides"),
    )
}

fn verify_wasm_wasi() -> Result<compat::JsonReport> {
    let mut violations = Vec::new();
    let mut items = Vec::new();
    let status = match run_wasi_smoke() {
        Ok(output) => {
            items.push(compat::ReportItem::new(
                "vuec_wasm-wasi-smoke",
                compat::ReportStatus::Pass,
                output,
                Some(PathBuf::from("crates/vuec_wasm/src/bin/wasi_smoke.rs")),
            ));
            compat::ReportStatus::Pass
        }
        Err(err) => {
            violations.push(format!("WASI smoke failed: {err:#}"));
            items.push(compat::ReportItem::new(
                "vuec_wasm-wasi-smoke",
                compat::ReportStatus::Fail,
                format!("{err:#}"),
                Some(PathBuf::from("crates/vuec_wasm/src/bin/wasi_smoke.rs")),
            ));
            compat::ReportStatus::Fail
        }
    };

    Ok(
        compat::JsonReport::new("verify_wasm_wasi", status)
            .with_items(items)
            .with_violations(violations)
            .with_note("builds vuec_wasm's WASI smoke binary for wasm32-wasip1 and runs it with wasmtime; requires rust target wasm32-wasip1 and wasmtime on PATH"),
    )
}

fn verify_cli() -> Result<compat::JsonReport> {
    let mut violations = Vec::new();
    let mut items = Vec::new();

    let status = match run_cli_smoke_suite() {
        Ok(output) => {
            items.push(compat::ReportItem::new(
                "vuec-cli-smoke",
                compat::ReportStatus::Pass,
                output,
                Some(PathBuf::from("crates/vuec_cli")),
            ));
            compat::ReportStatus::Pass
        }
        Err(err) => {
            violations.push(format!("CLI smoke failed: {err:#}"));
            items.push(compat::ReportItem::new(
                "vuec-cli-smoke",
                compat::ReportStatus::Fail,
                format!("{err:#}"),
                Some(PathBuf::from("crates/vuec_cli")),
            ));
            compat::ReportStatus::Fail
        }
    };

    Ok(
        compat::JsonReport::new("verify_cli", status)
            .with_items(items)
            .with_violations(violations)
            .with_note("builds the vuec CLI and verifies real subcommand smoke coverage for Vue 2 template, Vue 3 template, Vue 3 SFC, SSR, parse-sfc, JSON output, diagnostics, source maps, and bench output"),
    )
}

fn verify_incremental() -> Result<compat::JsonReport> {
    let mut violations = Vec::new();
    let mut items = Vec::new();

    match run_incremental_smoke_suite() {
        Ok(output) => {
            items.push(compat::ReportItem::new(
                "sfc-descriptor-cache",
                compat::ReportStatus::Pass,
                output,
                Some(PathBuf::from("crates/vuec_sfc")),
            ));
        }
        Err(err) => {
            violations.push(format!("SFC incremental cache smoke failed: {err:#}"));
            items.push(compat::ReportItem::new(
                "sfc-descriptor-cache",
                compat::ReportStatus::Fail,
                format!("{err:#}"),
                Some(PathBuf::from("crates/vuec_sfc")),
            ));
        }
    }

    Ok(
        compat::JsonReport::new("verify_incremental", compat::ReportStatus::Pass)
            .with_items(items)
            .with_violations(violations)
            .with_note("verifies SFC descriptor cache reuse for unchanged input and invalidation for changed same-file input; compiler semantics remain in vuec_sfc"),
    )
}

fn verify_parallel() -> Result<compat::JsonReport> {
    let mut violations = Vec::new();
    let mut items = Vec::new();

    let status = match run_parallel_smoke_suite() {
        Ok(output) => {
            items.push(compat::ReportItem::new(
                "vuec-cli-compile-batch",
                compat::ReportStatus::Pass,
                output,
                Some(PathBuf::from("crates/vuec_cli")),
            ));
            compat::ReportStatus::Pass
        }
        Err(err) => {
            violations.push(format!("parallel compile smoke failed: {err:#}"));
            items.push(compat::ReportItem::new(
                "vuec-cli-compile-batch",
                compat::ReportStatus::Fail,
                format!("{err:#}"),
                Some(PathBuf::from("crates/vuec_cli")),
            ));
            compat::ReportStatus::Fail
        }
    };

    Ok(
        compat::JsonReport::new("verify_parallel", status)
            .with_items(items)
            .with_violations(violations)
            .with_note("builds the vuec CLI and verifies compile-batch runs independent inputs concurrently while preserving deterministic input-order JSON results"),
    )
}

fn verify_ast_cache() -> Result<compat::JsonReport> {
    let mut violations = Vec::new();
    let mut items = Vec::new();

    let status = match run_ast_cache_smoke_suite() {
        Ok(output) => {
            items.push(compat::ReportItem::new(
                "vue3-dom-ast-cache",
                compat::ReportStatus::Pass,
                output,
                Some(PathBuf::from("crates/vuec_vue3_dom")),
            ));
            compat::ReportStatus::Pass
        }
        Err(err) => {
            violations.push(format!("Vue3 DOM AST cache smoke failed: {err:#}"));
            items.push(compat::ReportItem::new(
                "vue3-dom-ast-cache",
                compat::ReportStatus::Fail,
                format!("{err:#}"),
                Some(PathBuf::from("crates/vuec_vue3_dom")),
            ));
            compat::ReportStatus::Fail
        }
    };

    Ok(
        compat::JsonReport::new("verify_ast_cache", status)
            .with_items(items)
            .with_violations(violations)
            .with_note("verifies the Vue3 DOM compiler AST cache reuses parse/DOM-normalize results for unchanged inputs, invalidates changed same-file inputs, and keeps compile output stable"),
    )
}

fn verify_arena() -> Result<compat::JsonReport> {
    let mut violations = Vec::new();
    let mut items = Vec::new();

    let status = match run_arena_smoke_suite() {
        Ok(output) => {
            items.push(compat::ReportItem::new(
                "ast-document-arena-preallocation",
                compat::ReportStatus::Pass,
                output,
                Some(PathBuf::from("crates/vuec_ast")),
            ));
            compat::ReportStatus::Pass
        }
        Err(err) => {
            violations.push(format!("arena allocation smoke failed: {err:#}"));
            items.push(compat::ReportItem::new(
                "ast-document-arena-preallocation",
                compat::ReportStatus::Fail,
                format!("{err:#}"),
                Some(PathBuf::from("crates/vuec_ast")),
            ));
            compat::ReportStatus::Fail
        }
    };

    Ok(
        compat::JsonReport::new("verify_arena", status)
            .with_items(items)
            .with_violations(violations)
            .with_note("verifies AstDocument node preallocation APIs plus Vue 2 and Vue 3 parser entrypoints use arena capacity hints without changing tree invariants"),
    )
}

fn verify_string_interning() -> Result<compat::JsonReport> {
    let mut violations = Vec::new();
    let mut items = Vec::new();

    let status = match run_string_interning_smoke_suite() {
        Ok(output) => {
            items.push(compat::ReportItem::new(
                "js-source-string-interner",
                compat::ReportStatus::Pass,
                output,
                Some(PathBuf::from("crates/vuec_js")),
            ));
            compat::ReportStatus::Pass
        }
        Err(err) => {
            violations.push(format!("string interning smoke failed: {err:#}"));
            items.push(compat::ReportItem::new(
                "js-source-string-interner",
                compat::ReportStatus::Fail,
                format!("{err:#}"),
                Some(PathBuf::from("crates/vuec_js")),
            ));
            compat::ReportStatus::Fail
        }
    };

    Ok(
        compat::JsonReport::new("verify_string_interning", status)
            .with_items(items)
            .with_violations(violations)
            .with_note("verifies JS expression side-store string interning reuses repeated source text while preserving serialized string output and AST/HIR/MIR structure boundaries"),
    )
}

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
    if diagnostic_out.status != 0
        || !diagnostic_out.stderr.contains("[error]")
        || !diagnostic_out.stderr.contains("v-model")
    {
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
    if !parse_json.pointer("/descriptor/template").is_some() {
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

    let mut lowering_options = vuec_vue3_core::Vue3CompilerOptions::default();
    lowering_options.prefix_identifiers = true;
    lowering_options.mode = "module".into();
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

#[derive(Clone, Debug, Serialize)]
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
    if target.package == "@vue/compiler-dom" {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bench_case_kind_uses_cli_targets() {
        assert_eq!(BenchCaseKind::Vue2Template.cli_target(), "vue2-template");
        assert_eq!(BenchCaseKind::Vue3Template.cli_target(), "vue3-template");
        assert_eq!(BenchCaseKind::Vue3Sfc.cli_target(), "vue3-sfc");
        assert_eq!(BenchCaseKind::Vue3Ssr.cli_target(), "vue3-ssr");
    }

    #[test]
    fn windows_executable_detection_prefers_spawnable_shims() {
        assert!(is_windows_executable(r"C:\node\npm.cmd"));
        assert!(is_windows_executable(r"C:\node\pnpm.exe"));
        assert!(!is_windows_executable(r"C:\node\npm"));
    }

    #[test]
    fn proc_status_rss_parser_prefers_high_water_mark() {
        let status = "Name:\tnode\nVmRSS:\t 100 kB\nVmHWM:\t 256 kB\n";
        assert_eq!(parse_proc_status_rss_bytes(status), Some(256 * 1024));
    }

    #[test]
    fn proc_status_rss_parser_falls_back_to_current_rss() {
        let status = "Name:\tnode\nVmRSS:\t 64 kB\n";
        assert_eq!(parse_proc_status_rss_bytes(status), Some(64 * 1024));
    }

    #[test]
    fn official_npm_versions_read_locked_compilers() {
        let lock = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace root")
            .join("compat")
            .join("official-revisions.lock");
        let versions = official_npm_versions(&lock).expect("official versions");
        assert_eq!(versions.vue2, "2.7.16");
        assert_eq!(versions.vue_template_compiler, "2.7.16");
        assert_eq!(versions.vue_compiler_dom, "3.5.34");
        assert_eq!(versions.vue_compiler_sfc, "3.5.34");
        assert_eq!(versions.vue_compiler_ssr, "3.5.34");
    }

    #[test]
    fn sha256_bytes_is_stable() {
        assert_eq!(
            sha256_bytes(b"vuec"),
            "1fc8cc70af7ec7c20b935e8970e8641a6acc9fd856788a44a68507e33c8d561d"
        );
    }

    #[test]
    fn native_artifact_lookup_accepts_platform_subdir() {
        let root = unique_target_test_dir("native-artifact-subdir");
        let artifact = root.join("linux-x64-gnu").join("vuec_napi.node");
        fs::create_dir_all(artifact.parent().expect("artifact parent")).unwrap();
        fs::write(&artifact, b"native").unwrap();

        let found = find_native_artifact(Some(&root), "linux-x64-gnu")
            .expect("artifact lookup")
            .expect("artifact path");
        assert_eq!(found, artifact);
        assert!(find_native_artifact(Some(&root), "darwin-arm64")
            .expect("missing artifact lookup")
            .is_none());
    }

    #[test]
    fn native_artifact_lookup_accepts_flat_node_file() {
        let root = unique_target_test_dir("native-artifact-flat");
        let artifact = root.join("darwin-arm64.node");
        fs::create_dir_all(&root).unwrap();
        fs::write(&artifact, b"native").unwrap();

        let found = find_native_artifact(Some(&root), "darwin-arm64")
            .expect("artifact lookup")
            .expect("artifact path");
        assert_eq!(found, artifact);
    }

    #[test]
    fn native_artifact_lookup_accepts_downloaded_github_artifact_layout() {
        let root = unique_target_test_dir("native-artifact-github-download");
        let artifact = root
            .join("native-Linux-X64")
            .join("linux-x64-gnu")
            .join("vuec_napi.node");
        fs::create_dir_all(artifact.parent().expect("artifact parent")).unwrap();
        fs::write(&artifact, b"native").unwrap();

        let found = find_native_artifact(Some(&root), "linux-x64-gnu")
            .expect("artifact lookup")
            .expect("artifact path");
        assert_eq!(found, artifact);
    }

    #[test]
    fn ci_status_fixture_passes_when_required_jobs_succeed() {
        let root = unique_target_test_dir("ci-status-pass");
        let (runs, jobs) = write_ci_status_fixture(&root, "success", None);

        let report = verify_ci_status(
            Some("hamflx/vue-compiler"),
            Some("abc123"),
            "ci.yml",
            Some(&runs),
            Some(&jobs),
        )
        .expect("ci status report");

        assert_eq!(report.status, "pass");
        assert_eq!(report.summary.total, REQUIRED_CI_JOBS.len() + 1);
        assert_eq!(report.summary.pass, REQUIRED_CI_JOBS.len() + 1);
        assert!(report.violations.is_empty());
    }

    #[test]
    fn ci_status_fixture_fails_when_required_job_fails() {
        let root = unique_target_test_dir("ci-status-fail");
        let (runs, jobs) = write_ci_status_fixture(
            &root,
            "success",
            Some(("Compatibility (macos-latest)", "failure")),
        );

        let report = verify_ci_status(
            Some("hamflx/vue-compiler"),
            Some("abc123"),
            "ci.yml",
            Some(&runs),
            Some(&jobs),
        )
        .expect("ci status report");

        assert_eq!(report.status, "fail");
        assert!(report
            .violations
            .iter()
            .any(|violation| violation.contains("Compatibility (macos-latest)")));
    }

    #[test]
    fn ci_status_fixture_fails_when_completed_run_misses_required_job() {
        let root = unique_target_test_dir("ci-status-missing");
        let runs = root.join("runs.json");
        let jobs = root.join("jobs.json");
        fs::create_dir_all(&root).unwrap();
        fs::write(&runs, ci_runs_fixture("success")).unwrap();
        let jobs_json = json!({
            "jobs": REQUIRED_CI_JOBS
                .iter()
                .filter(|job| **job != "Release Dry Run")
                .map(|job| json!({
                    "name": job,
                    "status": "completed",
                    "conclusion": "success",
                    "html_url": format!("https://example.test/{job}")
                }))
                .collect::<Vec<_>>()
        });
        fs::write(&jobs, serde_json::to_vec_pretty(&jobs_json).unwrap()).unwrap();

        let report = verify_ci_status(
            Some("hamflx/vue-compiler"),
            Some("abc123"),
            "ci.yml",
            Some(&runs),
            Some(&jobs),
        )
        .expect("ci status report");

        assert_eq!(report.status, "fail");
        assert!(report
            .items
            .iter()
            .any(|item| item.target == "job:Release Dry Run"
                && item.status == compat::ReportStatus::Fail));
    }

    #[test]
    fn ci_status_fixture_is_pending_when_workflow_is_not_completed() {
        let root = unique_target_test_dir("ci-status-pending");
        let runs = root.join("runs.json");
        let jobs = root.join("jobs.json");
        fs::create_dir_all(&root).unwrap();
        fs::write(&runs, ci_runs_fixture_with("in_progress", None)).unwrap();
        let jobs_json = json!({
            "jobs": REQUIRED_CI_JOBS
                .iter()
                .map(|job| json!({
                    "name": job,
                    "status": "queued",
                    "conclusion": null,
                    "html_url": format!("https://example.test/{job}")
                }))
                .collect::<Vec<_>>()
        });
        fs::write(&jobs, serde_json::to_vec_pretty(&jobs_json).unwrap()).unwrap();

        let report = verify_ci_status(
            Some("hamflx/vue-compiler"),
            Some("abc123"),
            "ci.yml",
            Some(&runs),
            Some(&jobs),
        )
        .expect("ci status report");

        assert_eq!(report.status, "pending");
        assert_eq!(report.summary.pending, REQUIRED_CI_JOBS.len() + 1);
        assert!(report.violations.is_empty());
    }

    #[test]
    fn github_remote_parser_accepts_common_origin_shapes() {
        assert_eq!(
            parse_github_remote_repo("git@github.com:hamflx/vue-compiler.git").as_deref(),
            Some("hamflx/vue-compiler")
        );
        assert_eq!(
            parse_github_remote_repo("https://github.com/hamflx/vue-compiler.git").as_deref(),
            Some("hamflx/vue-compiler")
        );
        assert_eq!(
            parse_github_remote_repo("ssh://git@github.com/hamflx/vue-compiler.git").as_deref(),
            Some("hamflx/vue-compiler")
        );
        assert!(parse_github_remote_repo("https://example.com/hamflx/vue-compiler").is_none());
    }

    fn unique_target_test_dir(name: &str) -> PathBuf {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace root");
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock before unix epoch")
            .as_nanos();
        workspace
            .join("target")
            .join("xtask-tests")
            .join(format!("{name}-{}-{stamp}", std::process::id()))
    }

    fn write_ci_status_fixture(
        root: &Path,
        default_conclusion: &str,
        override_job: Option<(&str, &str)>,
    ) -> (PathBuf, PathBuf) {
        fs::create_dir_all(root).unwrap();
        let runs = root.join("runs.json");
        let jobs = root.join("jobs.json");
        fs::write(&runs, ci_runs_fixture(default_conclusion)).unwrap();
        let jobs_json = json!({
            "jobs": REQUIRED_CI_JOBS
                .iter()
                .map(|job| {
                    let conclusion = override_job
                        .filter(|(name, _)| name == job)
                        .map(|(_, conclusion)| conclusion)
                        .unwrap_or(default_conclusion);
                    json!({
                        "name": job,
                        "status": "completed",
                        "conclusion": conclusion,
                        "html_url": format!("https://example.test/{job}")
                    })
                })
                .collect::<Vec<_>>()
        });
        fs::write(&jobs, serde_json::to_vec_pretty(&jobs_json).unwrap()).unwrap();
        (runs, jobs)
    }

    fn ci_runs_fixture(conclusion: &str) -> Vec<u8> {
        ci_runs_fixture_with("completed", Some(conclusion))
    }

    fn ci_runs_fixture_with(status: &str, conclusion: Option<&str>) -> Vec<u8> {
        serde_json::to_vec_pretty(&json!({
            "workflow_runs": [{
                "id": 42,
                "head_sha": "abc123",
                "status": status,
                "conclusion": conclusion,
                "html_url": "https://example.test/run/42",
                "run_number": 7
            }]
        }))
        .unwrap()
    }
}
