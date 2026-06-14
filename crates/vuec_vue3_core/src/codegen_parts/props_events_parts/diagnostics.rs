pub(crate) fn push_vue3_parser_diagnostic(
    ast: &mut Vue3Ast,
    code: Vue3ErrorCode,
    file_id: FileId,
    offset: usize,
) {
    let diagnostic = Vue3ParserDiagnostic {
        code: code.as_u16(),
        message: vue3_parse_error_message(code).into(),
        span: Some(Span::new(file_id, offset, offset)),
    };
    if let Some(root_node) = ast.root_node_mut() {
        if let Vue3AstKind::Root(root) = &mut root_node.kind {
            if !root.parser_diagnostics.iter().any(|existing| {
                existing.code == diagnostic.code && existing.span == diagnostic.span
            }) {
                root.parser_diagnostics.push(diagnostic);
            }
        }
    }
}

/// Returns Vue 3 parser diagnostics recorded on the AST root.
pub fn vue3_parser_diagnostics(ast: &Vue3Ast) -> Vec<Diagnostic> {
    ast.root_node()
        .and_then(|node| match &node.kind {
            Vue3AstKind::Root(root) => Some(root.parser_diagnostics.as_slice()),
            _ => None,
        })
        .into_iter()
        .flatten()
        .map(|diagnostic| {
            Diagnostic::error(diagnostic.code.to_string(), diagnostic.message.clone())
                .with_span(diagnostic.span)
        })
        .collect()
}

pub(crate) fn vue3_parse_error_message(code: Vue3ErrorCode) -> &'static str {
    match code {
        Vue3ErrorCode::DuplicateAttribute => "Duplicate attribute.",
        Vue3ErrorCode::EofBeforeTagName => "Unexpected EOF in tag.",
        Vue3ErrorCode::EofInTag => "Unexpected EOF in tag.",
        Vue3ErrorCode::MissingEndTagName => "End tag name was expected.",
        Vue3ErrorCode::XInvalidEndTag => "Invalid end tag.",
        Vue3ErrorCode::XMissingEndTag => "Element is missing end tag.",
        Vue3ErrorCode::XMissingInterpolationEnd => "Interpolation end sign was not found.",
        Vue3ErrorCode::XMissingDirectiveName => "Legal directive name was expected.",
        _ => "Vue compiler parse error",
    }
}

pub(crate) fn expression_diagnostics(
    ast: &Vue3Ast,
    options: &Vue3CompilerOptions,
) -> Vec<Diagnostic> {
    let store = JsAstStore::new();
    let source_type = expression_source_type(options);
    let mut diagnostics = Vec::new();
    for node in &ast.nodes {
        match &node.kind {
            Vue3AstKind::Interpolation(interpolation) => {
                push_expression_parse_diagnostic(
                    &store,
                    &interpolation.expression.source_string(),
                    node.span.source(),
                    source_type,
                    &mut diagnostics,
                );
            }
            Vue3AstKind::Element(element) => {
                for prop in &element.props {
                    if let Vue3Prop::Directive(dir) = prop {
                        push_directive_expression_diagnostic(
                            &store,
                            dir,
                            source_type,
                            &mut diagnostics,
                        );
                        if dir.name == "model" {
                            push_model_binding_diagnostic(element, dir, options, &mut diagnostics);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    diagnostics
}

pub(crate) fn push_directive_expression_diagnostic(
    store: &JsAstStore,
    dir: &Vue3Directive,
    source_type: oxc_span::SourceType,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match dir.name.as_str() {
        "for" => push_for_expression_diagnostic(store, dir, source_type, diagnostics),
        "on" if dir.arg.is_some() => {
            if let Some(expression) = dir.exp.as_ref() {
                push_event_handler_parse_diagnostic(
                    store,
                    &expression.source_string(),
                    dir.exp_span,
                    source_type,
                    diagnostics,
                );
            }
        }
        _ => {
            if let Some(expression) = dir.exp.as_ref() {
                push_expression_parse_diagnostic(
                    store,
                    &expression.source_string(),
                    dir.exp_span,
                    source_type,
                    diagnostics,
                );
            }
        }
    }
}

pub(crate) fn push_for_expression_diagnostic(
    store: &JsAstStore,
    dir: &Vue3Directive,
    source_type: oxc_span::SourceType,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(expression) = dir.exp.as_ref() else {
        diagnostics.push(vue3_for_diagnostic(
            Vue3ErrorCode::XVForNoExpression,
            "v-for is missing expression.",
            dir.span,
        ));
        return;
    };
    let expression = expression.source_string();
    let expression = expression.trim();
    if expression.is_empty() {
        diagnostics.push(vue3_for_diagnostic(
            Vue3ErrorCode::XVForNoExpression,
            "v-for is missing expression.",
            dir.exp_span.or(dir.span),
        ));
        return;
    }
    if store.parse_for_expression(expression, source_type).is_err() {
        diagnostics.push(vue3_for_diagnostic(
            Vue3ErrorCode::XVForMalformedExpression,
            "v-for has invalid expression.",
            dir.exp_span.or(dir.span),
        ));
    }
}

pub(crate) fn vue3_for_diagnostic(
    code: Vue3ErrorCode,
    message: &str,
    span: Option<Span>,
) -> Diagnostic {
    Diagnostic::vue3_error(code, message, span)
}

pub(crate) fn push_model_binding_diagnostic(
    element: &Vue3Element,
    dir: &Vue3Directive,
    options: &Vue3CompilerOptions,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(expression) = dir.exp.as_ref() else {
        diagnostics.push(vue3_model_diagnostic(
            "41",
            "v-model is missing expression.",
            dir.arg_span.or(dir.exp_span),
        ));
        return;
    };
    let raw = expression.source_string();
    let raw = raw.trim();
    if raw.is_empty() {
        diagnostics.push(vue3_model_diagnostic(
            "42",
            "v-model value must be a valid JavaScript member expression.",
            dir.exp_span,
        ));
        return;
    }
    match options.binding_metadata.get(raw).map(String::as_str) {
        Some("props" | "props-aliased") => diagnostics.push(vue3_model_diagnostic(
            "44",
            "v-model cannot be used on a prop, because local prop bindings are not writable.\nUse a v-bind binding combined with a v-on listener that emits update:x event instead.",
            dir.exp_span,
        )),
        Some("literal-const" | "setup-const") => diagnostics.push(vue3_model_diagnostic(
            "45",
            "v-model cannot be used on a const binding because it is not writable.",
            dir.exp_span,
        )),
        _ if model_binding_host_supports_expression_diagnostic(element)
            && !model_is_member_expression(raw) =>
        {
            diagnostics.push(vue3_model_diagnostic(
                "42",
                "v-model value must be a valid JavaScript member expression.",
                dir.exp_span,
            ));
        }
        _ => {}
    }
}

pub(crate) fn model_binding_host_supports_expression_diagnostic(element: &Vue3Element) -> bool {
    element.tag_type == Vue3ElementType::Component || vue3_dom_model_kind(element).is_some()
}

pub(crate) fn vue3_model_diagnostic(code: &str, message: &str, span: Option<Span>) -> Diagnostic {
    Diagnostic::error(code, message).with_span(span)
}

pub(crate) fn push_event_handler_parse_diagnostic(
    store: &JsAstStore,
    expression: &str,
    span: Option<Span>,
    source_type: oxc_span::SourceType,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let expression = expression.trim();
    if expression.is_empty() {
        return;
    }
    let wrapped = format!("({expression})");
    if store.parse_expression(&wrapped, source_type).is_ok() {
        return;
    }
    if !expression.contains(';') {
        push_expression_parse_diagnostic(store, expression, span, source_type, diagnostics);
        return;
    }
    let parsed = store.parse_program(expression, source_type);
    if !parsed.panicked && parsed.errors.is_empty() {
        return;
    }
    diagnostics.push(js_program_errors_to_vue3_invalid_expression_diagnostic(
        &parsed.errors,
        expression,
        span,
    ));
}

pub(crate) fn push_expression_parse_diagnostic(
    store: &JsAstStore,
    expression: &str,
    span: Option<Span>,
    source_type: oxc_span::SourceType,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let expression = expression.trim();
    if expression.is_empty() {
        return;
    }
    let wrapped = format!("({expression})");
    let Err(err) = store.parse_expression(&wrapped, source_type) else {
        return;
    };
    diagnostics.push(js_error_to_vue3_invalid_expression_diagnostic(
        &err, expression, span,
    ));
}

pub(crate) fn expression_source_type(options: &Vue3CompilerOptions) -> oxc_span::SourceType {
    if options.is_ts
        || options
            .expression_plugins
            .iter()
            .any(|plugin| plugin == "typescript")
    {
        oxc_span::SourceType::ts()
    } else {
        oxc_span::SourceType::mjs()
    }
}
