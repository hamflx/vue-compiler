use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::Instant;

use anyhow::{bail, Context, Result};
use clap::{error::ErrorKind, Parser, ValueEnum};
use serde::Serialize;
use serde_json::{json, Value};
use vuec_codegen::SourceMapArtifact;
use vuec_diagnostics::Diagnostic;
use vuec_sfc::{
    vue3_sfc_parse_diagnostics, SfcCompiler, SfcPropsDestructureMode, SfcScriptBlock,
    SfcScriptCompileOptions, SfcStyleCompileOptions, SfcStyleCompileResult,
    SfcTemplateCompileOptions, SfcTemplateCompileResult,
};
use vuec_source::FileId;
use vuec_vue2::{Vue2CompileOptions, Vue2CompiledResult};
use vuec_vue3_core::{CodegenResult, TemplateSource, Vue3CompilerOptions};
use vuec_vue3_dom::{apply_dom_parser_defaults, compile as compile_dom, DomCompilerOptions};
use vuec_vue3_ssr::{compile as compile_ssr, SsrCompilerOptions};

#[derive(Parser, Debug)]
#[command(name = "vuec", about = "Rust Vue compiler command line interface")]
struct Cli {
    #[command(subcommand)]
    command: CliCommand,
}

#[derive(clap::Subcommand, Debug)]
enum CliCommand {
    CompileTemplate(CompileTemplateArgs),
    CompileSfc(CompileSfcArgs),
    CompileSsr(CompileSsrArgs),
    CompileBatch(CompileBatchArgs),
    ParseSfc(ParseSfcArgs),
    Conformance(ConformanceArgs),
    Bench(BenchArgs),
}

#[derive(clap::Args, Debug)]
struct CompileTemplateArgs {
    #[arg(value_name = "INPUT")]
    input: PathBuf,
    #[arg(long, value_enum, default_value_t = TemplateTarget::Vue3)]
    target: TemplateTarget,
    #[arg(long)]
    json: bool,
    #[arg(long)]
    diagnostics: bool,
    #[arg(long)]
    source_map: bool,
    #[arg(long, value_name = "PATH")]
    map_out: Option<PathBuf>,
    #[arg(long, value_enum, default_value_t = Vue3Mode::Function)]
    mode: Vue3Mode,
    #[arg(long)]
    prefix_identifiers: bool,
}

#[derive(clap::Args, Debug)]
struct CompileSfcArgs {
    #[arg(value_name = "INPUT")]
    input: PathBuf,
    #[arg(long)]
    ssr: bool,
    #[arg(long)]
    json: bool,
    #[arg(long)]
    diagnostics: bool,
    #[arg(long)]
    source_map: bool,
    #[arg(long, value_name = "PATH")]
    map_out: Option<PathBuf>,
    #[arg(long)]
    id: Option<String>,
    #[arg(long)]
    inline_template: bool,
    #[arg(long, value_enum, default_value_t = CliPropsDestructureMode::Enabled)]
    props_destructure: CliPropsDestructureMode,
    #[arg(long = "global-type-file", value_name = "PATH")]
    global_type_files: Vec<PathBuf>,
}

#[derive(clap::Args, Debug)]
struct CompileSsrArgs {
    #[arg(value_name = "INPUT")]
    input: PathBuf,
    #[arg(long)]
    sfc: bool,
    #[arg(long)]
    json: bool,
    #[arg(long)]
    diagnostics: bool,
    #[arg(long)]
    source_map: bool,
    #[arg(long, value_name = "PATH")]
    map_out: Option<PathBuf>,
}

#[derive(clap::Args, Debug)]
struct CompileBatchArgs {
    #[arg(value_name = "INPUT", required = true)]
    inputs: Vec<PathBuf>,
    #[arg(long, value_enum, default_value_t = CompileBatchTarget::Vue3Template)]
    target: CompileBatchTarget,
    #[arg(long, default_value_t = 0)]
    jobs: usize,
    #[arg(long)]
    json: bool,
}

#[derive(clap::Args, Debug)]
struct ParseSfcArgs {
    #[arg(value_name = "INPUT")]
    input: PathBuf,
    #[arg(long)]
    json: bool,
}

#[derive(clap::Args, Debug)]
struct ConformanceArgs {
    #[arg(long)]
    all: bool,
    #[arg(long)]
    suite: Option<String>,
}

#[derive(clap::Args, Debug)]
struct BenchArgs {
    #[arg(value_name = "INPUT")]
    input: PathBuf,
    #[arg(long, value_enum, default_value_t = BenchTarget::Vue3Template)]
    target: BenchTarget,
    #[arg(long, default_value_t = 100)]
    iterations: usize,
    #[arg(long)]
    json: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum TemplateTarget {
    Vue2,
    Vue3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum Vue3Mode {
    Function,
    Module,
}

impl Vue3Mode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Module => "module",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum BenchTarget {
    Vue2Template,
    Vue3Template,
    Vue3Sfc,
    Vue3Ssr,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum CompileBatchTarget {
    Vue2Template,
    Vue3Template,
    Vue3Sfc,
    Vue3Ssr,
}

impl CompileBatchTarget {
    fn as_str(self) -> &'static str {
        match self {
            Self::Vue2Template => "vue2-template",
            Self::Vue3Template => "vue3-template",
            Self::Vue3Sfc => "vue3-sfc",
            Self::Vue3Ssr => "vue3-ssr",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum CliPropsDestructureMode {
    Enabled,
    Disabled,
    Error,
}

impl From<CliPropsDestructureMode> for SfcPropsDestructureMode {
    fn from(value: CliPropsDestructureMode) -> Self {
        match value {
            CliPropsDestructureMode::Enabled => Self::Enabled,
            CliPropsDestructureMode::Disabled => Self::Disabled,
            CliPropsDestructureMode::Error => Self::Error,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct RunOutput {
    stdout: String,
    stderr: String,
    code: i32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CliDiagnostic {
    code: String,
    severity: String,
    message: String,
    start: Option<usize>,
    end: Option<usize>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BatchCompileItem {
    index: usize,
    input: String,
    status: &'static str,
    result: Option<Value>,
    error: Option<String>,
}

#[derive(Debug)]
struct BatchCompiled {
    item: BatchCompileItem,
    text: String,
}
