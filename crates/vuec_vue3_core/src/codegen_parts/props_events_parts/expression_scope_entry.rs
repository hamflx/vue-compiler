pub(crate) fn rewrite_handler_expression_with_scope(
    expression: &str,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    let scope = scope.with_locals(vec!["$event".into()]);
    normalize_handler_indent(&rewrite_expression_with_scope(expression, options, &scope))
}

pub(crate) fn rewrite_expression_with_scope(
    expression: &str,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    let expression = expression.trim();
    if !uses_prefixed_identifiers(options) {
        return expression.to_string();
    }
    if scope.locals.is_empty() {
        rewrite_js_like_expression(expression, options)
    } else {
        rewrite_js_like_expression_with_locals(expression, options, &scope.locals)
    }
}

pub(crate) fn rewrite_ssr_css_vars_expression(
    expression: &str,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    let trimmed = expression.trim();
    let Some(body) = trimmed
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
    else {
        return rewrite_expression_with_scope(trimmed, options, scope);
    };
    let multiline = body.contains('\n');
    let properties = split_top_level_like(body, ',')
        .into_iter()
        .map(|property| rewrite_ssr_css_var_property(property, options, scope))
        .collect::<Vec<_>>();
    if multiline {
        return format!("{{\n  {}\n}}", properties.join(",\n  "));
    }
    format!("{{ {} }}", properties.join(", "))
}

pub(crate) fn rewrite_ssr_css_var_property(
    property: &str,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    let property = property.trim();
    if property.starts_with("...") {
        return rewrite_expression_with_scope(property, options, scope);
    }
    if let Some(colon) = find_top_level_char(property, ':') {
        let key = property[..colon].trim();
        let value = property[colon + 1..].trim();
        return format!(
            "{key}: {}",
            rewrite_expression_with_scope(value, options, scope)
        );
    }
    if is_simple_identifier(property) {
        format!(
            "{property}: {}",
            rewrite_identifier_with_scope(property, options, scope)
        )
    } else {
        rewrite_expression_with_scope(property, options, scope)
    }
}

pub(crate) fn rewrite_expression_with_scope_preserve_outer(
    expression: &str,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    if !uses_prefixed_identifiers(options) {
        return expression.to_string();
    }
    if scope.locals.is_empty() {
        rewrite_js_like_expression(expression, options)
    } else {
        rewrite_js_like_expression_with_locals(expression, options, &scope.locals)
    }
}

pub(crate) fn rewrite_identifier_with_scope(
    ident: &str,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    if !uses_prefixed_identifiers(options) || scope.locals.iter().any(|local| local == ident) {
        ident.to_string()
    } else {
        rewrite_identifier(ident, options)
    }
}

pub(crate) fn uses_prefixed_identifiers(options: &Vue3CompilerOptions) -> bool {
    options.prefix_identifiers || options.mode == "module"
}

pub(crate) fn normalize_handler_indent(expression: &str) -> String {
    let mut lines = expression.lines();
    let Some(first) = lines.next() else {
        return String::new();
    };
    let mut normalized = String::from(first);
    for line in lines {
        normalized.push('\n');
        normalized.push_str(
            line.strip_prefix("    ")
                .or_else(|| line.strip_prefix("  "))
                .unwrap_or(line),
        );
    }
    normalized
}
