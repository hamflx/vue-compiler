#![forbid(unsafe_code)]

mod compat;

use anyhow::Result;
use clap::{Parser, Subcommand};
use compat::{
    audit_option_matrix, diff_api, export_api, generate_option_matrix, generate_output_contract,
    run_conformance, run_option_matrix, run_output_contract, summarize_compat, sync_official_tests,
    verify_npm_alias, verify_official_lock, ConformanceArgs, SelectionArgs,
};
use std::path::PathBuf;

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
