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
    let parsed = compiler.parse_vue3(input.path.clone(), &input.source);
    let descriptor = &parsed.descriptor;
    let template = descriptor.template.as_ref().map(|_| {
        compiler.compile_template(
            descriptor,
            SfcTemplateCompileOptions {
                ssr: args.ssr,
                id: args.id.clone(),
                scope_id: args.id.as_ref().map(|id| format!("data-v-{id}")),
                source_map: args.source_map,
                ..SfcTemplateCompileOptions::default()
            },
        )
    });
    let script = if descriptor.script.is_some() || descriptor.script_setup.is_some() {
        Some(
            compiler.compile_script(
                descriptor,
                SfcScriptCompileOptions {
                    id: args.id.clone(),
                    inline_template: args.inline_template,
                    inline_template_ssr: args.inline_template && args.ssr,
                    source_map: args.source_map,
                    props_destructure: args.props_destructure.into(),
                    global_type_files: args
                        .global_type_files
                        .iter()
                        .map(|path| path.to_string_lossy().to_string())
                        .collect(),
                    ..SfcScriptCompileOptions::default()
                },
            ),
        )
    } else {
        None
    };
    let styles = if descriptor.styles.is_empty() {
        Vec::new()
    } else {
        vec![compiler.compile_style(
            descriptor,
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
    let mut diagnostics = diagnostics_from_core(&vue3_sfc_parse_diagnostics(&parsed));
    diagnostics.extend(sfc_diagnostics(
        template.as_ref(),
        script.as_ref(),
        &styles,
    ));
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
        let parsed = compiler.parse_vue3(input.path.clone(), &input.source);
        let result = compiler.compile_template(
            &parsed.descriptor,
            SfcTemplateCompileOptions {
                ssr: true,
                source_map: args.source_map,
                ..SfcTemplateCompileOptions::default()
            },
        );
        write_optional_map(args.map_out.as_deref(), result.map.as_ref())?;
        let mut diagnostics = diagnostics_from_core(&vue3_sfc_parse_diagnostics(&parsed));
        diagnostics.extend(sfc_template_diagnostics(&result));
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
    let has_errors = results.iter().any(batch_item_failed);
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
    let parsed = compiler.parse_vue3(input.path.clone(), &input.source);
    let diagnostics = diagnostics_from_core(&vue3_sfc_parse_diagnostics(&parsed));
    let payload = json!({
        "kind": "sfc-descriptor",
        "input": input.path,
        "descriptor": parsed.descriptor,
        "diagnostics": diagnostics,
    });
    let text = serde_json::to_string_pretty(&payload["descriptor"])?;
    emit_result(
        args.json,
        true,
        payload,
        text,
        diagnostics,
    )
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
