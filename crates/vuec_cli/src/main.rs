//! Command line interface for the Rust Vue compiler.
//!
//! The binary exposes release-facing commands for Vue 2 template compilation,
//! Vue 3 DOM/SSR template compilation, SFC parsing and compilation, batch
//! compilation, conformance summaries, and benchmark execution.

#![forbid(unsafe_code)]

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
    SfcCompiler, SfcScriptBlock, SfcScriptCompileOptions, SfcStyleCompileOptions,
    SfcStyleCompileResult, SfcTemplateCompileOptions, SfcTemplateCompileResult,
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

fn main() {
    match run_with_args(std::env::args_os()) {
        Ok(output) => {
            if !output.stdout.is_empty() {
                print!("{}", output.stdout);
            }
            if !output.stderr.is_empty() {
                eprint!("{}", output.stderr);
            }
            std::process::exit(output.code);
        }
        Err(err) => {
            eprintln!("{err:#}");
            std::process::exit(1);
        }
    }
}

fn run_with_args<I, T>(args: I) -> Result<RunOutput>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(error) => {
            let code = if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) {
                0
            } else {
                2
            };
            return Ok(RunOutput {
                stdout: if code == 0 {
                    error.to_string()
                } else {
                    String::new()
                },
                stderr: if code == 0 {
                    String::new()
                } else {
                    error.to_string()
                },
                code,
            });
        }
    };
    match cli.command {
        CliCommand::CompileTemplate(args) => compile_template_command(args),
        CliCommand::CompileSfc(args) => compile_sfc_command(args),
        CliCommand::CompileSsr(args) => compile_ssr_command(args),
        CliCommand::CompileBatch(args) => compile_batch_command(args),
        CliCommand::ParseSfc(args) => parse_sfc_command(args),
        CliCommand::Conformance(args) => conformance_command(args),
        CliCommand::Bench(args) => bench_command(args),
    }
}

fn compile_template_command(args: CompileTemplateArgs) -> Result<RunOutput> {
    let input = read_input(&args.input)?;
    match args.target {
        TemplateTarget::Vue2 => {
            let result = vuec_vue2::compile(&input.source, Vue2CompileOptions::default());
            let diagnostics = vue2_diagnostics(&result);
            let payload = json!({
                "kind": "vue2-template",
                "input": input.path,
                "render": result.render,
                "staticRenderFns": result.static_render_fns,
                "errors": result.errors,
                "tips": result.tips,
                "diagnostics": diagnostics,
            });
            emit_result(
                args.json,
                args.diagnostics,
                payload,
                format!(
                    "{}\n{}",
                    result.render,
                    render_static_fns_text(&result.static_render_fns)
                ),
                diagnostics,
            )
        }
        TemplateTarget::Vue3 => {
            let result =
                compile_vue3_template(&input, args.source_map, args.mode, args.prefix_identifiers);
            write_optional_map(args.map_out.as_deref(), result.map.as_ref())?;
            let diagnostics = diagnostics_from_core(&result.diagnostics);
            let payload = json!({
                "kind": "vue3-template",
                "input": input.path,
                "code": result.code,
                "map": result.map,
                "astSummary": result.ast_summary,
                "preamble": result.preamble,
                "diagnostics": diagnostics,
            });
            emit_result(
                args.json,
                args.diagnostics,
                payload,
                result.code,
                diagnostics,
            )
        }
    }
}

fn compile_sfc_command(args: CompileSfcArgs) -> Result<RunOutput> {
    let input = read_input(&args.input)?;
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(input.path.clone(), &input.source);
    let template = descriptor.template.as_ref().map(|_| {
        compiler.compile_template(
            &descriptor,
            SfcTemplateCompileOptions {
                ssr: args.ssr,
                id: args.id.clone(),
                scope_id: args.id.as_ref().map(|id| format!("data-v-{id}")),
                ..SfcTemplateCompileOptions::default()
            },
        )
    });
    let script = if descriptor.script.is_some() || descriptor.script_setup.is_some() {
        Some(compiler.compile_script(
            &descriptor,
            SfcScriptCompileOptions {
                id: args.id.clone(),
                inline_template: args.inline_template,
                ..SfcScriptCompileOptions::default()
            },
        ))
    } else {
        None
    };
    let styles = if descriptor.styles.is_empty() {
        Vec::new()
    } else {
        vec![compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                id: args.id.clone(),
                source_map: args.source_map,
                ..SfcStyleCompileOptions::default()
            },
        )]
    };
    if let Some(map_out) = args.map_out.as_deref() {
        let map = template.as_ref().and_then(|template| template.map.as_ref());
        write_optional_map(Some(map_out), map)?;
    }
    let diagnostics = sfc_diagnostics(template.as_ref(), script.as_ref(), &styles);
    let text = render_sfc_text(template.as_ref(), script.as_ref(), &styles);
    let payload = json!({
        "kind": if args.ssr { "vue3-sfc-ssr" } else { "vue3-sfc" },
        "input": input.path,
        "descriptor": descriptor,
        "template": template,
        "script": script,
        "styles": styles,
        "diagnostics": diagnostics,
    });
    emit_result(args.json, args.diagnostics, payload, text, diagnostics)
}

fn compile_ssr_command(args: CompileSsrArgs) -> Result<RunOutput> {
    let input = read_input(&args.input)?;
    if args.sfc {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(input.path.clone(), &input.source);
        let result = compiler.compile_template(
            &descriptor,
            SfcTemplateCompileOptions {
                ssr: true,
                ..SfcTemplateCompileOptions::default()
            },
        );
        write_optional_map(args.map_out.as_deref(), result.map.as_ref())?;
        let diagnostics = sfc_template_diagnostics(&result);
        let payload = json!({
            "kind": "vue3-sfc-ssr-template",
            "input": input.path,
            "code": result.code,
            "map": result.map,
            "astSummary": result.ast_summary,
            "diagnostics": diagnostics,
        });
        return emit_result(
            args.json,
            args.diagnostics,
            payload,
            result.code,
            diagnostics,
        );
    }
    let result = compile_vue3_ssr_template(&input, args.source_map);
    write_optional_map(args.map_out.as_deref(), result.map.as_ref())?;
    let diagnostics = diagnostics_from_core(&result.diagnostics);
    let payload = json!({
        "kind": "vue3-ssr-template",
        "input": input.path,
        "code": result.code,
        "map": result.map,
        "astSummary": result.ast_summary,
        "preamble": result.preamble,
        "diagnostics": diagnostics,
    });
    emit_result(
        args.json,
        args.diagnostics,
        payload,
        result.code,
        diagnostics,
    )
}

fn compile_batch_command(args: CompileBatchArgs) -> Result<RunOutput> {
    let worker_count = batch_worker_count(args.jobs, args.inputs.len());
    let started = Instant::now();
    let results = compile_batch_parallel(args.inputs, args.target, worker_count)?;
    let has_errors = results.iter().any(|result| result.item.status == "error");
    let payload = json!({
        "kind": "compile-batch",
        "target": args.target.as_str(),
        "jobs": worker_count,
        "inputs": results.len(),
        "elapsedMicros": started.elapsed().as_micros(),
        "results": results.iter().map(|result| result.item.clone()).collect::<Vec<_>>(),
    });
    let stdout = if args.json {
        format!("{}\n", serde_json::to_string_pretty(&payload)?)
    } else {
        render_batch_text(&results)
    };
    Ok(RunOutput {
        stdout,
        stderr: String::new(),
        code: if has_errors { 1 } else { 0 },
    })
}

fn parse_sfc_command(args: ParseSfcArgs) -> Result<RunOutput> {
    let input = read_input(&args.input)?;
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(input.path.clone(), &input.source);
    let payload = json!({
        "kind": "sfc-descriptor",
        "input": input.path,
        "descriptor": descriptor,
        "diagnostics": [],
    });
    let stdout = if args.json {
        format!("{}\n", serde_json::to_string_pretty(&payload)?)
    } else {
        format!(
            "{}\n",
            serde_json::to_string_pretty(&payload["descriptor"])?
        )
    };
    Ok(RunOutput {
        stdout,
        stderr: String::new(),
        code: 0,
    })
}

fn conformance_command(args: ConformanceArgs) -> Result<RunOutput> {
    let mut command = Command::new("cargo");
    command.arg("xtask").arg("run-conformance");
    if args.all || args.suite.is_none() {
        command.arg("--all");
    } else if let Some(suite) = args.suite {
        command.arg("--suite").arg(suite);
    }
    let output = command
        .output()
        .context("failed to run cargo xtask run-conformance")?;
    Ok(RunOutput {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        code: output.status.code().unwrap_or(1),
    })
}

fn bench_command(args: BenchArgs) -> Result<RunOutput> {
    if args.iterations == 0 {
        bail!("--iterations must be greater than zero");
    }
    let input = read_input(&args.input)?;
    let started = Instant::now();
    for _ in 0..args.iterations {
        match args.target {
            BenchTarget::Vue2Template => {
                let _ = vuec_vue2::compile(&input.source, Vue2CompileOptions::default());
            }
            BenchTarget::Vue3Template => {
                let _ = compile_vue3_template(&input, false, Vue3Mode::Function, false);
            }
            BenchTarget::Vue3Sfc => {
                let mut compiler = SfcCompiler::new();
                let descriptor = compiler.parse(input.path.clone(), &input.source);
                let _ =
                    compiler.compile_template(&descriptor, SfcTemplateCompileOptions::default());
            }
            BenchTarget::Vue3Ssr => {
                let _ = compile_vue3_ssr_template(&input, false);
            }
        }
    }
    let elapsed = started.elapsed();
    let micros_total = elapsed.as_micros();
    let micros_per_iter = micros_total / args.iterations as u128;
    let payload = json!({
        "kind": "bench",
        "input": input.path,
        "target": format!("{:?}", args.target),
        "iterations": args.iterations,
        "elapsedMicros": micros_total,
        "microsPerIteration": micros_per_iter,
    });
    let stdout = if args.json {
        format!("{}\n", serde_json::to_string_pretty(&payload)?)
    } else {
        format!(
            "target={:?} iterations={} elapsed_us={} per_iter_us={}\n",
            args.target, args.iterations, micros_total, micros_per_iter
        )
    };
    Ok(RunOutput {
        stdout,
        stderr: String::new(),
        code: 0,
    })
}

fn compile_batch_parallel(
    inputs: Vec<PathBuf>,
    target: CompileBatchTarget,
    worker_count: usize,
) -> Result<Vec<BatchCompiled>> {
    if inputs.is_empty() {
        bail!("compile-batch requires at least one input");
    }

    let worker_count = worker_count.max(1).min(inputs.len());
    let next = AtomicUsize::new(0);
    let (sender, receiver) = mpsc::channel();

    thread::scope(|scope| {
        for _ in 0..worker_count {
            let sender = sender.clone();
            let inputs = &inputs;
            let next = &next;
            scope.spawn(move || loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                if index >= inputs.len() {
                    break;
                }
                let path = &inputs[index];
                let compiled = compile_batch_input(index, path, target)
                    .unwrap_or_else(|err| batch_compile_error(index, path, err));
                if sender.send((index, compiled)).is_err() {
                    break;
                }
            });
        }
    });
    drop(sender);

    let mut results = (0..inputs.len()).map(|_| None).collect::<Vec<_>>();
    for _ in 0..inputs.len() {
        let (index, compiled) = receiver
            .recv()
            .context("compile-batch worker stopped before reporting all inputs")?;
        results[index] = Some(compiled);
    }
    results
        .into_iter()
        .enumerate()
        .map(|(index, result)| {
            result.with_context(|| format!("compile-batch missing result for input {index}"))
        })
        .collect()
}

fn compile_batch_input(
    index: usize,
    path: &Path,
    target: CompileBatchTarget,
) -> Result<BatchCompiled> {
    let input = read_input(path)?;
    let (payload, text) = match target {
        CompileBatchTarget::Vue2Template => {
            let result = vuec_vue2::compile(&input.source, Vue2CompileOptions::default());
            let diagnostics = vue2_diagnostics(&result);
            let payload = json!({
                "kind": "vue2-template",
                "input": input.path,
                "render": result.render,
                "staticRenderFns": result.static_render_fns,
                "errors": result.errors,
                "tips": result.tips,
                "diagnostics": diagnostics,
            });
            let text = format!(
                "{}\n{}",
                result.render,
                render_static_fns_text(&result.static_render_fns)
            );
            (payload, text)
        }
        CompileBatchTarget::Vue3Template => {
            let result = compile_vue3_template(&input, false, Vue3Mode::Function, false);
            let diagnostics = diagnostics_from_core(&result.diagnostics);
            let payload = json!({
                "kind": "vue3-template",
                "input": input.path,
                "code": result.code,
                "map": result.map,
                "astSummary": result.ast_summary,
                "preamble": result.preamble,
                "diagnostics": diagnostics,
            });
            (payload, result.code)
        }
        CompileBatchTarget::Vue3Sfc => {
            let mut compiler = SfcCompiler::new();
            let descriptor = compiler.parse(input.path.clone(), &input.source);
            let template = descriptor.template.as_ref().map(|_| {
                compiler.compile_template(&descriptor, SfcTemplateCompileOptions::default())
            });
            let script = if descriptor.script.is_some() || descriptor.script_setup.is_some() {
                Some(compiler.compile_script(&descriptor, SfcScriptCompileOptions::default()))
            } else {
                None
            };
            let styles = if descriptor.styles.is_empty() {
                Vec::new()
            } else {
                vec![compiler.compile_style(&descriptor, SfcStyleCompileOptions::default())]
            };
            let diagnostics = sfc_diagnostics(template.as_ref(), script.as_ref(), &styles);
            let text = render_sfc_text(template.as_ref(), script.as_ref(), &styles);
            let payload = json!({
                "kind": "vue3-sfc",
                "input": input.path,
                "descriptor": descriptor,
                "template": template,
                "script": script,
                "styles": styles,
                "diagnostics": diagnostics,
            });
            (payload, text)
        }
        CompileBatchTarget::Vue3Ssr => {
            let result = compile_vue3_ssr_template(&input, false);
            let diagnostics = diagnostics_from_core(&result.diagnostics);
            let payload = json!({
                "kind": "vue3-ssr-template",
                "input": input.path,
                "code": result.code,
                "map": result.map,
                "astSummary": result.ast_summary,
                "preamble": result.preamble,
                "diagnostics": diagnostics,
            });
            (payload, result.code)
        }
    };
    Ok(BatchCompiled {
        item: BatchCompileItem {
            index,
            input: input.path,
            status: "ok",
            result: Some(payload),
            error: None,
        },
        text,
    })
}

fn batch_compile_error(index: usize, path: &Path, err: anyhow::Error) -> BatchCompiled {
    BatchCompiled {
        item: BatchCompileItem {
            index,
            input: path.display().to_string(),
            status: "error",
            result: None,
            error: Some(format!("{err:#}")),
        },
        text: String::new(),
    }
}

fn batch_worker_count(requested_jobs: usize, input_count: usize) -> usize {
    if input_count == 0 {
        return 0;
    }
    let requested_jobs = if requested_jobs == 0 {
        thread::available_parallelism()
            .map(|count| count.get())
            .unwrap_or(1)
    } else {
        requested_jobs
    };
    requested_jobs.max(1).min(input_count)
}

fn render_batch_text(results: &[BatchCompiled]) -> String {
    let mut output = String::new();
    for result in results {
        output.push_str("== ");
        output.push_str(&result.item.input);
        output.push_str(" ==\n");
        if let Some(error) = &result.item.error {
            output.push_str("error: ");
            output.push_str(error);
            output.push('\n');
        } else {
            output.push_str(&ensure_trailing_newline(result.text.clone()));
        }
    }
    output
}

fn compile_vue3_template(
    input: &InputSource,
    source_map: bool,
    mode: Vue3Mode,
    prefix_identifiers: bool,
) -> CodegenResult {
    let mut core = Vue3CompilerOptions {
        mode: mode.as_str().into(),
        prefix_identifiers,
        source_map,
        source_map_source: source_map.then(|| input.path.clone()),
        ..Vue3CompilerOptions::default()
    };
    apply_dom_parser_defaults(&mut core);
    if matches!(mode, Vue3Mode::Module) {
        core.prefix_identifiers = true;
    }
    compile_dom(
        template_source(input),
        DomCompilerOptions {
            core,
            ..DomCompilerOptions::default()
        },
    )
}

fn compile_vue3_ssr_template(
    input: &InputSource,
    source_map: bool,
) -> vuec_vue3_ssr::SsrCompileResult {
    let core = Vue3CompilerOptions {
        source_map,
        source_map_source: source_map.then(|| input.path.clone()),
        ..Vue3CompilerOptions::default()
    };
    let mut core = core;
    apply_dom_parser_defaults(&mut core);
    compile_ssr(
        template_source(input),
        SsrCompilerOptions {
            core,
            ..SsrCompilerOptions::default()
        },
    )
}

fn template_source(input: &InputSource) -> TemplateSource {
    TemplateSource {
        filename: input.path.clone(),
        source: input.source.clone(),
        file_id: FileId(0),
        base_offset: 0,
    }
}

fn emit_result(
    json_output: bool,
    diagnostics_output: bool,
    payload: Value,
    text: String,
    diagnostics: Vec<CliDiagnostic>,
) -> Result<RunOutput> {
    let stdout = if json_output {
        format!("{}\n", serde_json::to_string_pretty(&payload)?)
    } else {
        ensure_trailing_newline(text)
    };
    let stderr = if diagnostics_output && !diagnostics.is_empty() {
        render_diagnostics_text(&diagnostics)
    } else {
        String::new()
    };
    Ok(RunOutput {
        stdout,
        stderr,
        code: 0,
    })
}

fn write_optional_map(path: Option<&Path>, map: Option<&SourceMapArtifact>) -> Result<()> {
    let Some(path) = path else {
        return Ok(());
    };
    let Some(map) = map else {
        bail!(
            "source map was requested for {}, but this compile result has no map",
            path.display()
        );
    };
    fs::write(path, serde_json::to_string_pretty(map)?)
        .with_context(|| format!("failed to write source map to {}", path.display()))
}

fn read_input(path: &Path) -> Result<InputSource> {
    let source =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    Ok(InputSource {
        path: path.display().to_string(),
        source,
    })
}

#[derive(Clone, Debug)]
struct InputSource {
    path: String,
    source: String,
}

fn diagnostics_from_core(diagnostics: &[Diagnostic]) -> Vec<CliDiagnostic> {
    diagnostics
        .iter()
        .map(|diagnostic| CliDiagnostic {
            code: diagnostic.code.clone(),
            severity: diagnostic.severity.as_str().into(),
            message: diagnostic.message.clone(),
            start: diagnostic.span.map(|span| span.start.0),
            end: diagnostic.span.map(|span| span.end.0),
        })
        .collect()
}

fn vue2_diagnostics(result: &Vue2CompiledResult) -> Vec<CliDiagnostic> {
    let mut diagnostics = result
        .errors
        .iter()
        .map(|error| CliDiagnostic {
            code: "vue2-error".into(),
            severity: "error".into(),
            message: error.msg.clone(),
            start: error.start,
            end: error.end,
        })
        .collect::<Vec<_>>();
    diagnostics.extend(result.tips.iter().map(|tip| CliDiagnostic {
        code: "vue2-tip".into(),
        severity: if tip.tip { "tip" } else { "warning" }.into(),
        message: tip.msg.clone(),
        start: tip.start,
        end: tip.end,
    }));
    diagnostics
}

fn sfc_diagnostics(
    template: Option<&SfcTemplateCompileResult>,
    script: Option<&SfcScriptBlock>,
    styles: &[SfcStyleCompileResult],
) -> Vec<CliDiagnostic> {
    let mut diagnostics = template.map(sfc_template_diagnostics).unwrap_or_default();
    if let Some(script) = script {
        diagnostics.extend(script.errors.iter().map(|error| CliDiagnostic {
            code: "sfc-script".into(),
            severity: "error".into(),
            message: error.clone(),
            start: None,
            end: None,
        }));
    }
    for style in styles {
        diagnostics.extend(style.errors.iter().map(|error| CliDiagnostic {
            code: "sfc-style".into(),
            severity: "error".into(),
            message: error.clone(),
            start: None,
            end: None,
        }));
    }
    diagnostics
}

fn sfc_template_diagnostics(template: &SfcTemplateCompileResult) -> Vec<CliDiagnostic> {
    template
        .errors
        .iter()
        .map(|error| CliDiagnostic {
            code: error.code.to_string(),
            severity: "error".into(),
            message: if error.loc.source.is_empty() {
                format!("template error {}", error.code)
            } else {
                format!("template error {} near {}", error.code, error.loc.source)
            },
            start: Some(error.loc.start.offset),
            end: Some(error.loc.end.offset),
        })
        .chain(template.tips.iter().map(|tip| CliDiagnostic {
            code: "sfc-template-tip".into(),
            severity: "tip".into(),
            message: tip.clone(),
            start: None,
            end: None,
        }))
        .collect()
}

fn render_sfc_text(
    template: Option<&SfcTemplateCompileResult>,
    script: Option<&SfcScriptBlock>,
    styles: &[SfcStyleCompileResult],
) -> String {
    let mut output = String::new();
    if let Some(template) = template {
        output.push_str(&template.code);
        output.push('\n');
    }
    if let Some(script) = script {
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str(&script.content);
        output.push('\n');
    }
    for style in styles {
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str(&style.code);
        output.push('\n');
    }
    output
}

fn render_static_fns_text(static_render_fns: &[String]) -> String {
    if static_render_fns.is_empty() {
        return "[]".into();
    }
    static_render_fns.join("\n")
}

fn render_diagnostics_text(diagnostics: &[CliDiagnostic]) -> String {
    diagnostics
        .iter()
        .map(|diagnostic| {
            let range = match (diagnostic.start, diagnostic.end) {
                (Some(start), Some(end)) => format!(" @{start}-{end}"),
                (Some(start), None) => format!(" @{start}"),
                _ => String::new(),
            };
            format!(
                "[{}] {}: {}{}\n",
                diagnostic.severity, diagnostic.code, diagnostic.message, range
            )
        })
        .collect()
}

fn ensure_trailing_newline(mut value: String) -> String {
    if !value.ends_with('\n') {
        value.push('\n');
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_exits_successfully() {
        let output = run_with_args(["vuec", "--help"]).expect("run");
        assert_eq!(output.code, 0);
        assert!(output.stdout.contains("compile-template"));
        assert!(output.stderr.is_empty());
    }

    #[test]
    fn compiles_vue2_template_json() {
        let path = write_temp("vuec-cli-vue2.html", "<div>{{ msg }}</div>");
        let output = run_with_args([
            "vuec",
            "compile-template",
            "--target",
            "vue2",
            "--json",
            path.to_str().unwrap(),
        ])
        .expect("run");
        let value: Value = serde_json::from_str(&output.stdout).expect("json");
        assert_eq!(value["kind"], json!("vue2-template"));
        assert!(value["render"].as_str().unwrap().contains("_c('div'"));
    }

    #[test]
    fn compiles_vue3_template_json() {
        let path = write_temp("vuec-cli-vue3.html", "<div>{{ msg }}</div>");
        let output = run_with_args([
            "vuec",
            "compile-template",
            "--target",
            "vue3",
            "--json",
            path.to_str().unwrap(),
        ])
        .expect("run");
        let value: Value = serde_json::from_str(&output.stdout).expect("json");
        assert_eq!(value["kind"], json!("vue3-template"));
        assert!(value["code"].as_str().unwrap().contains("function render"));
    }

    #[test]
    fn compiles_vue3_sfc_json() {
        let path = write_temp(
            "vuec-cli-sfc.vue",
            "<template><div>{{ msg }}</div></template><script setup>const msg = 'hi'</script>",
        );
        let output =
            run_with_args(["vuec", "compile-sfc", "--json", path.to_str().unwrap()]).expect("run");
        let value: Value = serde_json::from_str(&output.stdout).expect("json");
        assert_eq!(value["kind"], json!("vue3-sfc"));
        assert!(value["template"]["code"]
            .as_str()
            .unwrap()
            .contains("function render"));
        assert!(value["script"]["content"]
            .as_str()
            .unwrap()
            .contains("setup"));
    }

    #[test]
    fn compiles_vue3_ssr_json() {
        let path = write_temp("vuec-cli-ssr.html", "<div>{{ msg }}</div>");
        let output =
            run_with_args(["vuec", "compile-ssr", "--json", path.to_str().unwrap()]).expect("run");
        let value: Value = serde_json::from_str(&output.stdout).expect("json");
        assert_eq!(value["kind"], json!("vue3-ssr-template"));
        assert!(value["code"].as_str().unwrap().contains("ssrRender"));
    }

    #[test]
    fn parses_sfc_json() {
        let path = write_temp("vuec-cli-parse.vue", "<template><p/></template>");
        let output =
            run_with_args(["vuec", "parse-sfc", "--json", path.to_str().unwrap()]).expect("run");
        let value: Value = serde_json::from_str(&output.stdout).expect("json");
        assert_eq!(value["kind"], json!("sfc-descriptor"));
        assert!(value["descriptor"]["template"].is_object());
    }

    #[test]
    fn benchmarks_vue3_template_json() {
        let path = write_temp("vuec-cli-bench.html", "<div/>");
        let output = run_with_args([
            "vuec",
            "bench",
            "--target",
            "vue3-template",
            "--iterations",
            "1",
            "--json",
            path.to_str().unwrap(),
        ])
        .expect("run");
        let value: Value = serde_json::from_str(&output.stdout).expect("json");
        assert_eq!(value["kind"], json!("bench"));
        assert_eq!(value["iterations"], json!(1));
    }

    #[test]
    fn compiles_batch_in_input_order() {
        let first = write_temp("vuec-cli-batch-first.html", "<div>{{ first }}</div>");
        let second = write_temp(
            "vuec-cli-batch-second.html",
            "<section>{{ second }}</section>",
        );
        let output = run_with_args([
            "vuec",
            "compile-batch",
            "--target",
            "vue3-template",
            "--jobs",
            "2",
            "--json",
            first.to_str().unwrap(),
            second.to_str().unwrap(),
        ])
        .expect("run");
        assert_eq!(output.code, 0);
        let value: Value = serde_json::from_str(&output.stdout).expect("json");
        assert_eq!(value["kind"], json!("compile-batch"));
        assert_eq!(value["target"], json!("vue3-template"));
        assert_eq!(value["jobs"], json!(2));
        let results = value["results"].as_array().expect("results");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0]["index"], json!(0));
        assert_eq!(results[1]["index"], json!(1));
        assert!(results[0]["input"]
            .as_str()
            .unwrap()
            .contains("vuec-cli-batch-first.html"));
        assert!(results[1]["input"]
            .as_str()
            .unwrap()
            .contains("vuec-cli-batch-second.html"));
        assert!(results[0]["result"]["code"]
            .as_str()
            .unwrap()
            .contains("first"));
        assert!(results[1]["result"]["code"]
            .as_str()
            .unwrap()
            .contains("second"));
    }

    #[test]
    fn compile_batch_reports_read_errors() {
        let missing = std::env::temp_dir().join(format!(
            "vuec-cli-batch-missing-{}-{}",
            std::process::id(),
            Instant::now().elapsed().as_nanos()
        ));
        let output = run_with_args([
            "vuec",
            "compile-batch",
            "--target",
            "vue3-template",
            "--jobs",
            "8",
            "--json",
            missing.to_str().unwrap(),
        ])
        .expect("run");
        assert_eq!(output.code, 1);
        let value: Value = serde_json::from_str(&output.stdout).expect("json");
        assert_eq!(value["jobs"], json!(1));
        assert_eq!(value["results"][0]["status"], json!("error"));
        assert!(value["results"][0]["error"]
            .as_str()
            .unwrap()
            .contains("failed to read"));
    }

    #[test]
    fn writes_source_map_for_vue3_template() {
        let path = write_temp("vuec-cli-map.html", "<div>{{ msg }}</div>");
        let map_path = write_temp("vuec-cli-map.json", "");
        let output = run_with_args([
            "vuec",
            "compile-template",
            "--target",
            "vue3",
            "--source-map",
            "--map-out",
            map_path.to_str().unwrap(),
            path.to_str().unwrap(),
        ])
        .expect("run");
        assert!(output.stdout.contains("function render"));
        let map = fs::read_to_string(map_path).expect("map");
        let value: Value = serde_json::from_str(&map).expect("map json");
        assert_eq!(value["version"], json!(3));
        assert!(value["sources"].as_array().unwrap()[0]
            .as_str()
            .unwrap()
            .contains("vuec-cli-map.html"));
    }

    #[test]
    fn prints_diagnostics_to_stderr_when_requested() {
        let path = write_temp("vuec-cli-diagnostic.html", r#"<div v-model="baz"/>"#);
        let output = run_with_args([
            "vuec",
            "compile-template",
            "--target",
            "vue3",
            "--diagnostics",
            path.to_str().unwrap(),
        ])
        .expect("run");
        assert!(output.stderr.contains("[error]"));
        assert!(output.stderr.contains("v-model can only be used"));
    }

    fn write_temp(name: &str, content: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let unique = format!(
            "{}-{}-{}",
            name,
            std::process::id(),
            Instant::now().elapsed().as_nanos()
        );
        path.push(unique);
        fs::write(&path, content).expect("write temp");
        path
    }
}
