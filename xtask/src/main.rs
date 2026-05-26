#![forbid(unsafe_code)]

mod compat;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use compat::{
    audit_option_matrix, diff_api, export_api, generate_option_matrix, generate_output_contract,
    run_conformance, run_option_matrix, run_output_contract, summarize_compat, sync_official_tests,
    verify_npm_alias, verify_official_lock, ConformanceArgs, SelectionArgs,
};
use serde_json::Value as JsonValue;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

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
    },
    SyncOfficialTests {
        #[arg(long, default_value = "compat/official-revisions.lock")]
        lock: PathBuf,
        #[arg(long)]
        locked: bool,
        #[arg(long, default_value = "vendor")]
        out_dir: PathBuf,
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
    RunConformance {
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
    VerifyNpmAlias {
        #[command(flatten)]
        scope: SelectionArgs,
    },
    VerifyNapi,
    VerifyNapiAlias,
    VerifyNapiApi,
    VerifyNapiPlatform,
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
        Command::VerifyOfficialLock { path } => verify_official_lock(&path),
        Command::SyncOfficialTests {
            lock,
            locked,
            out_dir,
        } => sync_official_tests(&lock, locked, &out_dir),
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
        Command::RunConformance { args } => run_conformance(&args),
        Command::GenerateOutputContract { scope, out_dir } => {
            let report = generate_output_contract(&scope);
            ensure_dir(&out_dir)?;
            report
        }
        Command::RunOutputContract { scope } => run_output_contract(&scope),
        Command::VerifyNpmAlias { scope } => verify_npm_alias(&scope),
        Command::VerifyNapi => verify_napi()?,
        Command::VerifyNapiAlias => verify_napi_alias()?,
        Command::VerifyNapiApi => verify_napi_api()?,
        Command::VerifyNapiPlatform => verify_napi_platform()?,
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

fn copy_napi_binding(target_path: &Path) -> Result<PathBuf> {
    let source_path = napi_library_path();
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
    let (prefix, suffix) = match std::env::consts::OS {
        "windows" => ("", ".dll"),
        "macos" => ("lib", ".dylib"),
        _ => ("lib", ".so"),
    };
    PathBuf::from("target")
        .join("debug")
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
