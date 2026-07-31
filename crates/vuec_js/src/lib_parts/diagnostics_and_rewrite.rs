/// Converts an Oxc parse error into Vue 3's invalid-expression diagnostic shape.
pub fn js_error_to_vue3_invalid_expression_diagnostic(
    error: &JsParseError,
    source_text: &str,
    span: Option<Span>,
) -> Diagnostic {
    Diagnostic::vue3_error(
        Vue3ErrorCode::XInvalidExpression,
        vue3_expression_parse_error_message(error.message()),
        js_parse_error_span(error, source_text, span),
    )
}

/// Converts checked program parser output into a Vue 3 invalid-expression diagnostic.
pub fn js_program_errors_to_vue3_invalid_expression_diagnostic(
    errors: &[OxcDiagnostic],
    source_text: &str,
    span: Option<Span>,
) -> Diagnostic {
    let error = JsParseError::from_diagnostics(errors.to_vec());
    js_error_to_vue3_invalid_expression_diagnostic(&error, source_text, span)
}

/// Returns the byte offset of the primary label on the first Oxc diagnostic.
pub fn js_diagnostics_primary_offset(errors: &[OxcDiagnostic]) -> Option<usize> {
    errors.first().and_then(primary_label_offset)
}

/// Creates the official Vue 3 invalid-expression message from Oxc text.
pub fn vue3_expression_parse_error_message(raw: &str) -> String {
    let detail = raw
        .lines()
        .find_map(|line| line.trim().strip_prefix("× ").map(str::trim))
        .or_else(|| raw.lines().next().map(str::trim))
        .unwrap_or("Unexpected token");
    let detail = if detail == "Unexpected token" {
        "Unexpected token (1:3)"
    } else {
        detail
    };
    format!("Error parsing JavaScript expression: {detail}")
}

/// Maps an Oxc parse error onto the original source span for a JavaScript snippet.
pub fn js_parse_error_span(
    error: &JsParseError,
    source_text: &str,
    span: Option<Span>,
) -> Option<Span> {
    let span = span?;
    let relative = js_diagnostics_primary_offset(error.diagnostics())
        .or_else(|| parse_oxc_line_column(error.message()).map(|(_line, column)| column))
        .map(|offset| offset.min(source_text.len()))
        .unwrap_or(source_text.len());
    let start = span.start.0.saturating_add(relative).min(span.end.0);
    Some(Span::new(span.file_id, start, start))
}

/// Prefixes identifiers in a JavaScript-like Vue expression using a caller-supplied mapper.
pub fn prefix_expression_identifiers(
    expression: &str,
    rewriter: impl IdentifierRewriter,
    locals: &[String],
) -> String {
    let mut output = String::new();
    let mut quote = None::<char>;
    let mut escaped = false;
    let chars = expression.char_indices().collect::<Vec<_>>();
    let mut index = 0usize;
    let mut last_end = 0usize;

    while index < chars.len() {
        let start = chars[index].0;
        let ch = chars[index].1;
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
            index += 1;
            continue;
        }
        if matches!(ch, '\'' | '"' | '`') {
            quote = Some(ch);
            index += 1;
            continue;
        }
        if !is_identifier_start(ch) {
            index += 1;
            continue;
        }

        index += 1;
        while index < chars.len() && is_identifier_continue(chars[index].1) {
            index += 1;
        }
        let end = chars
            .get(index)
            .map_or(expression.len(), |(offset, _)| *offset);
        let ident = &expression[start..end];
        let prev = previous_non_ws(expression, start);
        let next = next_non_ws(expression, end);
        let property_key = next == Some(':') && prev != Some('?');
        let object_shorthand = prev.is_some_and(|prev| matches!(prev, '{' | ','))
            && next.is_some_and(|next| matches!(next, '}' | ','));
        let should_keep = is_keyword(ident)
            || is_global_or_literal(ident)
            || locals.iter().any(|local| local == ident)
            || prev == Some('.')
            || property_key;
        if should_keep {
            continue;
        }
        if let Some(replacement) = rewriter.rewrite_identifier(ident) {
            output.push_str(expression.get(last_end..start).unwrap_or_default());
            if object_shorthand {
                output.push_str(ident);
                output.push_str(": ");
            }
            output.push_str(&replacement);
            last_end = end;
        }
    }

    if last_end == 0 {
        return expression.to_string();
    }
    output.push_str(expression.get(last_end..).unwrap_or_default());
    output
}
