pub(crate) fn transform_on_projection_const_type(projection: &Value) -> u64 {
    projection
        .get("constType")
        .and_then(Value::as_u64)
        .unwrap_or_default()
}

pub(crate) fn transform_on_has_scope_ref(exp: &Value, context: &Value) -> bool {
    let source = model_expression_source(exp);
    context
        .get("identifiers")
        .and_then(Value::as_object)
        .is_some_and(|identifiers| {
            identifiers.iter().any(|(name, count)| {
                count.as_i64().unwrap_or_default() > 0 && source_contains_identifier(&source, name)
            })
        })
}

pub(crate) fn transform_on_is_member_expression(expression: &str, context: &Value) -> bool {
    let store = JsAstStore::new();
    let wrapped = format!("({})", expression.trim());
    match store.parse_expression(&wrapped, transform_on_source_type(context)) {
        Ok(expression) => transform_on_expression_is_member(&expression),
        Err(_) if json_bool(context, "allowLexerFallback") => {
            transform_on_is_member_expression_lexer(expression)
        }
        Err(_) => false,
    }
}

pub(crate) fn transform_on_expression_is_member(expression: &Expression<'_>) -> bool {
    match expression {
        Expression::Identifier(identifier) => identifier.name != "undefined",
        Expression::ComputedMemberExpression(_)
        | Expression::StaticMemberExpression(_)
        | Expression::PrivateFieldExpression(_) => true,
        Expression::ChainExpression(chain) => {
            transform_on_chain_element_is_member(&chain.expression)
        }
        Expression::ParenthesizedExpression(expression) => {
            transform_on_expression_is_member(&expression.expression)
        }
        Expression::TSAsExpression(expression) => {
            transform_on_expression_is_member(&expression.expression)
        }
        Expression::TSSatisfiesExpression(expression) => {
            transform_on_expression_is_member(&expression.expression)
        }
        Expression::TSTypeAssertion(expression) => {
            transform_on_expression_is_member(&expression.expression)
        }
        Expression::TSNonNullExpression(expression) => {
            transform_on_expression_is_member(&expression.expression)
        }
        Expression::TSInstantiationExpression(expression) => {
            transform_on_expression_is_member(&expression.expression)
        }
        _ => false,
    }
}

pub(crate) fn transform_on_chain_element_is_member(element: &ChainElement<'_>) -> bool {
    matches!(
        element,
        ChainElement::ComputedMemberExpression(_)
            | ChainElement::StaticMemberExpression(_)
            | ChainElement::PrivateFieldExpression(_)
            | ChainElement::TSNonNullExpression(_)
    )
}

pub(crate) fn transform_on_is_fn_expression(expression: &str, context: &Value) -> bool {
    let trimmed = expression.trim_start();
    if transform_on_is_fn_expression_lexer(trimmed) {
        return true;
    }
    let store = JsAstStore::new();
    store
        .parse_expression(expression.trim(), transform_on_source_type(context))
        .map(|expression| transform_on_expression_is_fn(&expression))
        .unwrap_or(false)
}

pub(crate) fn transform_on_expression_is_fn(expression: &Expression<'_>) -> bool {
    match expression {
        Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_) => true,
        Expression::TSAsExpression(expression) => {
            transform_on_expression_is_fn(&expression.expression)
        }
        Expression::TSSatisfiesExpression(expression) => {
            transform_on_expression_is_fn(&expression.expression)
        }
        Expression::TSTypeAssertion(expression) => {
            transform_on_expression_is_fn(&expression.expression)
        }
        Expression::TSNonNullExpression(expression) => {
            transform_on_expression_is_fn(&expression.expression)
        }
        Expression::TSInstantiationExpression(expression) => {
            transform_on_expression_is_fn(&expression.expression)
        }
        _ => false,
    }
}

pub(crate) fn transform_on_is_fn_expression_lexer(expression: &str) -> bool {
    expression.starts_with("function")
        || expression.starts_with("async function")
        || expression
            .find("=>")
            .is_some_and(|index| transform_on_arrow_prefix_is_fn_like(&expression[..index]))
}

pub(crate) fn transform_on_arrow_prefix_is_fn_like(prefix: &str) -> bool {
    let prefix = prefix.trim();
    let prefix = prefix.strip_prefix("async").unwrap_or(prefix).trim();
    if prefix.starts_with('(') {
        return prefix.ends_with(')');
    }
    is_simple_identifier_ascii(prefix)
}

pub(crate) fn transform_on_root_function_locals(expression: &str) -> Vec<String> {
    let store = JsAstStore::new();
    store
        .parse_expression(expression.trim(), oxc_span::SourceType::ts())
        .map(|expression| {
            let mut locals = Vec::new();
            transform_on_collect_root_function_locals(&expression, &mut locals);
            locals.sort();
            locals.dedup();
            locals
        })
        .unwrap_or_else(|_| transform_on_root_function_locals_lexer(expression))
}

pub(crate) fn transform_on_collect_root_function_locals(
    expression: &Expression<'_>,
    locals: &mut Vec<String>,
) {
    match expression {
        Expression::ArrowFunctionExpression(function) => {
            for param in &function.params.items {
                collect_vue3_for_binding_pattern(&param.pattern, locals);
            }
            if let Some(rest) = &function.params.rest {
                collect_vue3_for_binding_pattern(&rest.rest.argument, locals);
            }
        }
        Expression::FunctionExpression(function) => {
            for param in &function.params.items {
                collect_vue3_for_binding_pattern(&param.pattern, locals);
            }
            if let Some(rest) = &function.params.rest {
                collect_vue3_for_binding_pattern(&rest.rest.argument, locals);
            }
        }
        Expression::TSAsExpression(expression) => {
            transform_on_collect_root_function_locals(&expression.expression, locals)
        }
        Expression::TSSatisfiesExpression(expression) => {
            transform_on_collect_root_function_locals(&expression.expression, locals)
        }
        Expression::TSTypeAssertion(expression) => {
            transform_on_collect_root_function_locals(&expression.expression, locals)
        }
        Expression::TSNonNullExpression(expression) => {
            transform_on_collect_root_function_locals(&expression.expression, locals)
        }
        Expression::TSInstantiationExpression(expression) => {
            transform_on_collect_root_function_locals(&expression.expression, locals)
        }
        _ => {}
    }
}

pub(crate) fn transform_on_root_function_locals_lexer(expression: &str) -> Vec<String> {
    let trimmed = expression.trim_start();
    let Some(arrow_index) = trimmed.find("=>") else {
        return Vec::new();
    };
    let mut params = trimmed[..arrow_index].trim();
    params = params.strip_prefix("async").unwrap_or(params).trim();
    if params.starts_with('(') && params.ends_with(')') {
        params = &params[1..params.len() - 1];
    }
    split_top_level_like(params, ',')
        .into_iter()
        .flat_map(extract_slot_params)
        .collect()
}

pub(crate) fn transform_on_source_type(context: &Value) -> oxc_span::SourceType {
    let _ = context;
    oxc_span::SourceType::ts()
}

pub(crate) fn transform_on_is_member_expression_lexer(expression: &str) -> bool {
    let path = normalize_member_expression_whitespace(expression.trim());
    if path.is_empty() {
        return false;
    }
    let mut depth_square = 0usize;
    let mut depth_paren = 0usize;
    let mut quote = None::<char>;
    let mut escaped = false;
    let mut chars = path.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' | '`' => quote = Some(ch),
            '[' => depth_square += 1,
            ']' => {
                if depth_square == 0 {
                    return false;
                }
                depth_square -= 1;
            }
            '(' => depth_paren += 1,
            ')' => {
                if chars.peek().is_none() {
                    return false;
                }
                if depth_paren == 0 {
                    return false;
                }
                depth_paren -= 1;
            }
            _ if depth_square == 0 && depth_paren == 0 => {
                let valid = if index == 0 {
                    is_identifier_start(ch)
                } else {
                    is_identifier_continue(ch) || matches!(ch, '.' | '?')
                };
                if !valid {
                    return false;
                }
            }
            _ => {}
        }
    }
    depth_square == 0 && depth_paren == 0 && quote.is_none()
}

pub(crate) fn normalize_member_expression_whitespace(expression: &str) -> String {
    let mut output = String::new();
    let chars = expression.chars().collect::<Vec<_>>();
    for (index, ch) in chars.iter().copied().enumerate() {
        if ch.is_whitespace() {
            let prev = chars[..index]
                .iter()
                .rev()
                .find(|candidate| !candidate.is_whitespace())
                .copied();
            let next = chars[index + 1..]
                .iter()
                .find(|candidate| !candidate.is_whitespace())
                .copied();
            if matches!(prev, Some('.' | '[')) || matches!(next, Some('.' | '[')) {
                continue;
            }
        }
        output.push(ch);
    }
    output
}

/// Returns whether a `v-model` expression is assignable as a member expression.
pub fn model_is_member_expression(expression: &str) -> bool {
    let store = JsAstStore::new();
    store
        .parse_expression(expression, oxc_span::SourceType::mjs())
        .map(|expression| match expression {
            Expression::Identifier(_) => true,
            Expression::ComputedMemberExpression(_)
            | Expression::StaticMemberExpression(_)
            | Expression::PrivateFieldExpression(_) => true,
            Expression::ChainExpression(chain) => model_chain_element_is_member(&chain.expression),
            _ => false,
        })
        .unwrap_or(false)
}

pub(crate) fn model_chain_element_is_member(element: &ChainElement<'_>) -> bool {
    matches!(
        element,
        ChainElement::ComputedMemberExpression(_)
            | ChainElement::StaticMemberExpression(_)
            | ChainElement::PrivateFieldExpression(_)
    )
}

pub(crate) fn context_identifier_count(context: &Value, name: &str) -> i64 {
    context
        .get("identifiers")
        .and_then(|identifiers| identifiers.get(name))
        .and_then(Value::as_i64)
        .unwrap_or_default()
}
