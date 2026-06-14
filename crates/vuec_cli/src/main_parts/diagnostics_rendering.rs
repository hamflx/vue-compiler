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
        diagnostics.extend(style.diagnostics.iter().map(|diagnostic| CliDiagnostic {
            code: diagnostic.code.clone(),
            severity: diagnostic.severity.as_str().into(),
            message: diagnostic.message.clone(),
            start: diagnostic.span.map(|span| span.start.0),
            end: diagnostic.span.map(|span| span.end.0),
        }));
        diagnostics.extend(style.errors.iter().filter_map(|error| {
            if style
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message == *error)
            {
                return None;
            }
            Some(CliDiagnostic {
                code: "sfc-style".into(),
                severity: "error".into(),
                message: error.clone(),
                start: None,
                end: None,
            })
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
