
fn validate_element_expressions(
    element: &Vue2Element,
    js: &JsAstStore,
    diagnostics: &mut DiagnosticSink,
) {
    for (raw, expr, span) in [
        ("v-if", element.if_exp.as_deref(), element.if_span),
        ("v-else-if", element.elseif.as_deref(), element.elseif_span),
        ("v-for", element.for_exp.as_deref(), element.for_span),
    ] {
        validate_vue2_expression_field(js, raw, expr, span.or(element.span), diagnostics);
    }
    if element.for_exp.is_some() {
        validate_vue2_params_field(
            js,
            "v-for alias",
            vue2_for_alias_params(element).as_deref(),
            element.for_span.or(element.span),
            diagnostics,
        );
    }
    for (raw, expr, span) in [
        (
            "key",
            element.key.as_deref(),
            element.key_span.or(element.span),
        ),
        ("ref", element.ref_name.as_deref(), element.span),
        ("slot", element.slot_target.as_deref(), element.span),
        ("is", element.component.as_deref(), element.span),
        ("class", element.class_binding.as_deref(), element.span),
        ("style", element.style_binding.as_deref(), element.span),
    ] {
        validate_vue2_expression_field(js, raw, expr, span, diagnostics);
    }
    for attr in element
        .attrs
        .iter()
        .chain(element.props.iter())
        .chain(element.dynamic_attrs.iter())
    {
        validate_vue2_expression_field(
            js,
            &attr.name,
            Some(&attr.value),
            attr.span.or(element.span),
            diagnostics,
        );
        if attr.dynamic {
            validate_vue2_expression_field(
                js,
                "dynamic argument",
                Some(&attr.name),
                attr.span.or(element.span),
                diagnostics,
            );
        }
    }
    validate_vue2_directives(element, js, diagnostics);
    validate_vue2_events(&element.events, js, diagnostics);
    validate_vue2_events(&element.native_events, js, diagnostics);
    if let Some(wrap) = &element.wrap_data {
        match wrap {
            Vue2DataWrap::Bind { value, .. } => {
                validate_vue2_expression_field(js, "v-bind", Some(value), element.span, diagnostics)
            }
        }
    }
    validate_vue2_expression_field(
        js,
        "v-on",
        element.wrap_listeners.as_deref(),
        element.span,
        diagnostics,
    );
    if let Some(model) = &element.model {
        let raw = serde_json::from_str::<String>(&model.expression).unwrap_or_else(|_| {
            model
                .value
                .trim()
                .strip_prefix('(')
                .and_then(|value| value.strip_suffix(')'))
                .unwrap_or(model.value.as_str())
                .to_string()
        });
        validate_vue2_expression_field(js, "v-model", Some(&raw), element.span, diagnostics);
        validate_vue2_assignment_field(
            js,
            "v-model assignment",
            Some(&raw),
            element.span,
            diagnostics,
        );
    }
    validate_vue2_params_field(
        js,
        "slot-scope",
        element.slot_scope.as_deref(),
        element.span,
        diagnostics,
    );
    for child in &element.children {
        match child {
            Vue2Node::Element(child) => validate_element_expressions(child, js, diagnostics),
            Vue2Node::Text(text) => {
                if let Some(expression) = &text.expression {
                    if !is_valid_vue2_interpolation_expression(js, expression) {
                        diagnostics.push(vue2_error(
                            "E_VUE2_INVALID_EXPRESSION",
                            format!("Raw expression: {}", text.text),
                            text.span,
                        ));
                    }
                }
            }
        }
    }
    for condition in element.if_conditions.iter().skip(1) {
        validate_element_expressions(&condition.block, js, diagnostics);
    }
    for slot in element.scoped_slots.values() {
        validate_element_expressions(slot, js, diagnostics);
    }
}

fn is_valid_vue2_expression(js: &JsAstStore, expr: &str) -> bool {
    js.parse_vue2_filter_expression(expr.trim(), SourceType::script())
        .is_ok()
}

fn validate_vue2_directives(
    element: &Vue2Element,
    js: &JsAstStore,
    diagnostics: &mut DiagnosticSink,
) {
    for directive in &element.directives {
        validate_vue2_expression_field(
            js,
            &directive.raw_name,
            directive.value.as_deref(),
            directive.span.or(element.span),
            diagnostics,
        );
        if directive.is_dynamic_arg {
            validate_vue2_expression_field(
                js,
                "dynamic directive argument",
                directive.arg.as_deref(),
                directive.span.or(element.span),
                diagnostics,
            );
        }
        if directive.name == "model" {
            validate_vue2_assignment_field(
                js,
                "v-model assignment",
                directive.value.as_deref(),
                directive.span.or(element.span),
                diagnostics,
            );
        }
    }
}

fn validate_vue2_events(
    events: &BTreeMap<String, Vec<Vue2EventHandler>>,
    js: &JsAstStore,
    diagnostics: &mut DiagnosticSink,
) {
    for (name, handlers) in events {
        for handler in handlers {
            if handler.span.is_none() {
                continue;
            }
            if handler.dynamic {
                validate_vue2_expression_field(
                    js,
                    "dynamic event",
                    Some(name),
                    handler.span,
                    diagnostics,
                );
            }
            validate_vue2_handler_field(js, name, Some(&handler.value), handler.span, diagnostics);
        }
    }
}

fn validate_vue2_expression_field(
    js: &JsAstStore,
    raw: &str,
    expr: Option<&str>,
    span: Option<Span>,
    diagnostics: &mut DiagnosticSink,
) {
    let Some(expr) = expr.map(str::trim).filter(|expr| !expr.is_empty()) else {
        return;
    };
    if !is_valid_vue2_expression(js, expr) {
        diagnostics.push(vue2_error(
            "E_VUE2_INVALID_EXPRESSION",
            format!("Raw expression: {raw}=\"{expr}\""),
            span,
        ));
    }
}

fn validate_vue2_handler_field(
    js: &JsAstStore,
    raw: &str,
    expr: Option<&str>,
    span: Option<Span>,
    diagnostics: &mut DiagnosticSink,
) {
    let Some(expr) = expr.map(str::trim) else {
        return;
    };
    if is_valid_vue2_handler(js, expr) {
        return;
    }
    diagnostics.push(vue2_error(
        "E_VUE2_INVALID_EXPRESSION",
        format!("Raw expression: @{raw}=\"{expr}\""),
        span,
    ));
}

fn validate_vue2_params_field(
    js: &JsAstStore,
    raw: &str,
    expr: Option<&str>,
    span: Option<Span>,
    diagnostics: &mut DiagnosticSink,
) {
    let Some(expr) = expr.map(str::trim).filter(|expr| !expr.is_empty()) else {
        return;
    };
    if js.parse_params(expr).is_ok() {
        return;
    }
    diagnostics.push(vue2_error(
        "E_VUE2_INVALID_EXPRESSION",
        format!("Raw expression: {raw}=\"{expr}\""),
        span,
    ));
}

fn validate_vue2_assignment_field(
    js: &JsAstStore,
    raw: &str,
    expr: Option<&str>,
    span: Option<Span>,
    diagnostics: &mut DiagnosticSink,
) {
    let Some(expr) = expr.map(str::trim).filter(|expr| !expr.is_empty()) else {
        return;
    };
    if !is_valid_vue2_expression(js, expr) || is_valid_vue2_assignment_target(js, expr) {
        return;
    }
    diagnostics.push(vue2_error(
        "E_VUE2_INVALID_EXPRESSION",
        format!("Raw expression: {raw}=\"{expr}\""),
        span,
    ));
}

fn is_valid_vue2_handler(js: &JsAstStore, expr: &str) -> bool {
    expr.is_empty()
        || js.validate_expression(expr, SourceType::script()).is_ok()
        || js
            .validate_function_body(expr, SourceType::script())
            .is_ok()
}

fn is_valid_vue2_assignment_target(js: &JsAstStore, expr: &str) -> bool {
    js.validate_function_body(&format!("{expr}=__vuec_value__;"), SourceType::script())
        .is_ok()
}

fn vue2_for_alias_params(element: &Vue2Element) -> Option<String> {
    let mut params = Vec::new();
    params.push(element.alias.as_deref()?.trim());
    if let Some(iterator) = element.iterator1.as_deref().map(str::trim) {
        params.push(iterator);
    }
    if let Some(iterator) = element.iterator2.as_deref().map(str::trim) {
        params.push(iterator);
    }
    Some(params.join(","))
}

fn is_valid_vue2_interpolation_expression(js: &JsAstStore, expression: &str) -> bool {
    let Some(raw) = expression
        .trim()
        .strip_prefix("_s(")
        .and_then(|value| value.strip_suffix(')'))
    else {
        return is_valid_vue2_expression(js, expression);
    };
    is_valid_vue2_expression(js, raw)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ParsedFor {
    for_exp: String,
    alias: String,
    iterator1: Option<String>,
    iterator2: Option<String>,
}

fn parse_for(exp: &str) -> Option<ParsedFor> {
    let (alias, for_exp) = split_for_expression(exp)?;
    let alias = strip_parens(alias.trim());
    let parts = split_top_level(alias, ',');
    Some(ParsedFor {
        for_exp: for_exp.trim().to_string(),
        alias: parts.first().copied().unwrap_or(alias).trim().to_string(),
        iterator1: parts.get(1).map(|part| part.trim().to_string()),
        iterator2: parts.get(2).map(|part| part.trim().to_string()),
    })
}

fn split_for_expression(source: &str) -> Option<(&str, &str)> {
    let mut depth = 0usize;
    for (index, ch) in source.char_indices() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ' ' if depth == 0 => {
                let rest = &source[index..];
                if rest.starts_with(" in ") {
                    return Some((&source[..index], &source[index + 4..]));
                }
                if rest.starts_with(" of ") {
                    return Some((&source[..index], &source[index + 4..]));
                }
            }
            _ => {}
        }
    }
    None
}

fn strip_parens(value: &str) -> &str {
    let trimmed = value.trim();
    if trimmed.starts_with('(') && trimmed.ends_with(')') {
        trimmed[1..trimmed.len() - 1].trim()
    } else {
        trimmed
    }
}

fn parse_text(text: &str, delimiters: Option<&[String; 2]>) -> Option<String> {
    let (open, close) =
        delimiters.map_or(("{{", "}}"), |items| (items[0].as_str(), items[1].as_str()));
    let mut tokens = Vec::new();
    let mut cursor = 0usize;
    while let Some(open_offset) = text[cursor..].find(open) {
        let open_index = cursor + open_offset;
        if open_index > cursor {
            tokens.push(js_string(&text[cursor..open_index]));
        }
        let expression_start = open_index + open.len();
        let Some(close_offset) = text[expression_start..].find(close) else {
            return None;
        };
        let close_index = expression_start + close_offset;
        let expression = parse_filters(text[expression_start..close_index].trim());
        tokens.push(format!("_s({expression})"));
        cursor = close_index + close.len();
    }
    if cursor == 0 {
        return None;
    }
    if cursor < text.len() {
        tokens.push(js_string(&text[cursor..]));
    }
    Some(tokens.join("+"))
}

fn parse_filters(exp: &str) -> String {
    rewrite_vue2_filter_expression(exp)
}
