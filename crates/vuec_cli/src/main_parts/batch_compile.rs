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
            let parsed = compiler.parse_vue3(input.path.clone(), &input.source);
            let descriptor = &parsed.descriptor;
            let template = descriptor.template.as_ref().map(|_| {
                compiler.compile_template(descriptor, SfcTemplateCompileOptions::default())
            });
            let script = if descriptor.script.is_some() || descriptor.script_setup.is_some() {
                Some(compiler.compile_script(descriptor, SfcScriptCompileOptions::default()))
            } else {
                None
            };
            let styles = if descriptor.styles.is_empty() {
                Vec::new()
            } else {
                vec![compiler.compile_style(descriptor, SfcStyleCompileOptions::default())]
            };
            let mut diagnostics = diagnostics_from_core(&vue3_sfc_parse_diagnostics(&parsed));
            diagnostics.extend(sfc_diagnostics(
                template.as_ref(),
                script.as_ref(),
                &styles,
            ));
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
