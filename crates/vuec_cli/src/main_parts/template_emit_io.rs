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
        code: if has_error_diagnostic(&diagnostics) {
            1
        } else {
            0
        },
    })
}

fn has_error_diagnostic(diagnostics: &[CliDiagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == "error")
}

fn batch_item_failed(result: &BatchCompiled) -> bool {
    if result.item.status == "error" {
        return true;
    }
    result
        .item
        .result
        .as_ref()
        .is_some_and(value_has_error_diagnostic)
}

fn value_has_error_diagnostic(value: &Value) -> bool {
    value
        .get("diagnostics")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|diagnostic| {
            diagnostic
                .get("severity")
                .and_then(Value::as_str)
                .is_some_and(|severity| severity == "error")
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
