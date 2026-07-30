use crate::*;

/// Compiles SFC style source according to `options`.
pub fn compile_style(source: &str, options: StyleCompileOptions) -> StyleCompileResult {
    let mut errors = Vec::new();
    let mut diagnostics = Vec::new();
    let mut dependencies = Vec::new();
    let mut code = match preprocess_style(source, &options) {
        Ok(result) => {
            dependencies.extend(result.dependencies);
            result.code
        }
        Err(error) => {
            diagnostics.push(preprocess_error_diagnostic(&error, source, &options));
            errors.push(error.message);
            source.to_string()
        }
    };
    let option_id = options.id.clone();
    let id = option_id.clone().unwrap_or_else(|| "data-v-vuec".into());
    let vars = if options.vars.is_empty() {
        collect_css_vars_with_options(
            &code,
            CssVarCollectOptions {
                ignore_line_comments: options.css_var_ignore_line_comments,
            },
        )
    } else {
        options.vars.clone()
    };

    if options.scoped {
        if options.warn_deprecated_scoped_selectors {
            diagnostics.extend(scoped_selector_deprecation_warnings(&code));
        }
        code = rewrite_scoped_selectors(&code, &id);
    }
    if !vars.is_empty() {
        let var_id = option_id.as_deref().map(style_var_id).unwrap_or_default();
        code = rewrite_css_vars_with_options(
            &code,
            &var_id,
            CssVarRewriteOptions {
                is_prod: options.is_prod,
                name_style: options.css_var_name_style,
                ignore_line_comments: options.css_var_ignore_line_comments,
            },
        );
    }
    let css_modules_hash_source = code.clone();
    code = normalize_style_output(&code);
    let modules = if options.modules {
        let result = compile_css_modules(&code, &css_modules_hash_source, &options);
        errors.extend(
            result
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.message.clone()),
        );
        let has_fatal_module_error = !result.diagnostics.is_empty();
        diagnostics.extend(result.diagnostics);
        if has_fatal_module_error {
            code.clear();
            None
        } else {
            code = result.code;
            Some(result.modules)
        }
    } else {
        code = normalize_public_closing_brace_whitespace(&code);
        None
    };
    let map = if options.source_map {
        Some(style_source_map(&code, source, &options))
    } else {
        None
    };

    StyleCompileResult {
        code,
        map,
        errors,
        diagnostics,
        modules,
        vars,
        dependencies,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct StylePreprocessError {
    pub(crate) code: &'static str,
    pub(crate) message: String,
    pub(crate) span: Option<(usize, usize)>,
}

impl StylePreprocessError {
    pub(crate) fn unsupported(message: impl Into<String>) -> Self {
        Self {
            code: "VUEC_STYLE_UNSUPPORTED_PREPROCESSOR",
            message: message.into(),
            span: None,
        }
    }

    pub(crate) fn import_resolve(message: impl Into<String>, span: Option<(usize, usize)>) -> Self {
        Self {
            code: "VUEC_STYLE_IMPORT_RESOLVE",
            message: message.into(),
            span,
        }
    }

    pub(crate) fn import_limit(message: impl Into<String>, span: Option<(usize, usize)>) -> Self {
        Self {
            code: "VUEC_STYLE_IMPORT_LIMIT",
            message: message.into(),
            span,
        }
    }
}

pub(crate) fn preprocess_error_diagnostic(
    error: &StylePreprocessError,
    source: &str,
    options: &StyleCompileOptions,
) -> Diagnostic {
    let span = error.span.unwrap_or_else(|| (0, first_span_end(source)));
    Diagnostic::error(error.code, &error.message)
        .with_span(Some(style_source_span(options, span.0, span.1)))
}

pub(crate) fn first_span_end(source: &str) -> usize {
    source.chars().next().map_or(0, char::len_utf8)
}

pub(crate) fn style_source_span(
    options: &StyleCompileOptions,
    local_start: usize,
    local_end: usize,
) -> Span {
    let file_id = options.source_map_file_id.unwrap_or(FileId(0));
    Span::new(
        file_id,
        options.source_map_base_offset + local_start,
        options.source_map_base_offset + local_end,
    )
}

pub(crate) fn style_source_map(
    generated: &str,
    original_style_source: &str,
    options: &StyleCompileOptions,
) -> SourceMapArtifact {
    let filename = options
        .filename
        .clone()
        .unwrap_or_else(|| "style.css".into());
    let source_content = options
        .source_map_source
        .clone()
        .unwrap_or_else(|| original_style_source.to_string());
    let source_name = filename.clone();
    let file_id = options.source_map_file_id.unwrap_or(FileId(0));
    let mut builder = SourceMapBuilder::new().file(filename);
    builder.add_source_content(source_name.clone(), source_content);

    let mut original_line_starts = line_starts(original_style_source);
    if original_line_starts.is_empty() {
        original_line_starts.push(0);
    }
    let generated_line_count = generated.lines().count().max(1);
    for generated_line in 0..generated_line_count {
        let local_start = original_line_starts
            .get(generated_line)
            .copied()
            .unwrap_or_else(|| *original_line_starts.last().unwrap_or(&0));
        let absolute = options.source_map_base_offset + local_start;
        builder.add_mapping(
            generated_line + 1,
            0,
            Some(Span::new(file_id, absolute, absolute)),
            Some(source_name.clone()),
        );
    }
    builder.build()
}

pub(crate) fn line_starts(source: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (index, ch) in source.char_indices() {
        if ch == '\n' {
            starts.push(index + ch.len_utf8());
        }
    }
    starts
}

pub(crate) fn normalize_style_output(source: &str) -> String {
    source
        .replace("; }", ";\n}")
        .replace("} }", "}\n}")
        .replace("} .", "}\n.")
        .replace("; .", ";\n.")
        .lines()
        .map(|line| if line.trim() == "}" { "}" } else { line })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn normalize_public_closing_brace_whitespace(source: &str) -> String {
    let mut output = String::new();
    let mut state = CssScannerState::Normal;
    let mut index = 0usize;
    while index < source.len() {
        let Some(ch) = source[index..].chars().next() else {
            break;
        };
        match state {
            CssScannerState::Normal => {
                if source[index..].starts_with("/*") {
                    let Some(end_offset) = source[index + 2..].find("*/") else {
                        output.push_str(&source[index..]);
                        break;
                    };
                    let end = index + 2 + end_offset + 2;
                    output.push_str(&source[index..end]);
                    index = end;
                    continue;
                }
                match ch {
                    '\'' => state = CssScannerState::SingleQuote,
                    '"' => state = CssScannerState::DoubleQuote,
                    '}' => {
                        normalize_pending_closing_brace_whitespace(&mut output);
                        output.push('}');
                        index += ch.len_utf8();
                        continue;
                    }
                    _ => {}
                }
            }
            CssScannerState::SingleQuote => {
                if ch == '\\' {
                    output.push(ch);
                    index += ch.len_utf8();
                    if index < source.len() {
                        let escaped = source[index..].chars().next().expect("valid char boundary");
                        output.push(escaped);
                        index += escaped.len_utf8();
                    }
                    continue;
                }
                if ch == '\'' {
                    state = CssScannerState::Normal;
                }
            }
            CssScannerState::DoubleQuote => {
                if ch == '\\' {
                    output.push(ch);
                    index += ch.len_utf8();
                    if index < source.len() {
                        let escaped = source[index..].chars().next().expect("valid char boundary");
                        output.push(escaped);
                        index += escaped.len_utf8();
                    }
                    continue;
                }
                if ch == '"' {
                    state = CssScannerState::Normal;
                }
            }
            CssScannerState::BlockComment => {}
        }
        output.push(ch);
        index += ch.len_utf8();
    }
    output
}

pub(crate) fn normalize_pending_closing_brace_whitespace(output: &mut String) {
    let line_start = output.rfind('\n').map_or(0, |index| index + 1);
    let line_suffix = &output[line_start..];
    if !line_suffix.is_empty()
        && line_suffix
            .chars()
            .all(|ch| matches!(ch, ' ' | '\t' | '\r'))
    {
        output.truncate(line_start);
        return;
    }

    let original_len = output.len();
    while output.ends_with([' ', '\t', '\r']) {
        output.pop();
    }
    if output.len() != original_len && !output.ends_with('\n') {
        output.push('\n');
    }
}
