#![forbid(unsafe_code)]

mod compat;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use compat::{
    audit_option_matrix, diff_api, export_api, generate_option_matrix, generate_output_contract,
    run_conformance, run_option_matrix, run_output_contract, summarize_compat, sync_official_tests,
    verify_npm_alias, verify_official_lock, ConformanceArgs, SelectionArgs,
};
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
        Command::SummarizeCompat { locked, lock } => summarize_compat(locked, &lock),
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    if report.status != "pass" && report.status != "pending" {
        std::process::exit(1);
    }
    Ok(())
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
