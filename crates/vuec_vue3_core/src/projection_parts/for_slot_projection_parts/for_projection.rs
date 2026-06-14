pub(crate) fn vue3_for_expression_projection(
    content: &str,
    exp: &Value,
    start: usize,
    end: usize,
    ast_mode: Vue3ForAstMode,
) -> Value {
    json!({
        "kind": "simple",
        "content": content,
        "isStatic": false,
        "constType": 0,
        "loc": vue3_for_exp_loc(exp, start, end),
        "astMode": vue3_for_ast_mode_name(ast_mode),
    })
}

pub(crate) fn vue3_for_rewrite_projection_node(
    raw: &str,
    options: &Vue3CompilerOptions,
    locals: &[String],
    loc: Value,
    ast_mode: Vue3ForAstMode,
    force_compound_for_complex: bool,
) -> Value {
    let rewritten = if locals.is_empty() {
        rewrite_js_like_expression(raw, options)
    } else {
        rewrite_js_like_expression_with_locals(raw, options, locals)
    };
    let children = vue3_for_compound_children(raw, options, locals, ast_mode, &loc);
    let is_simple = is_simple_identifier_ascii(raw.trim())
        || children.is_empty()
        || (!force_compound_for_complex && rewritten == raw.trim());
    if is_simple {
        return vue3_for_simple_projection(
            rewritten.trim(),
            loc,
            vue3_for_const_type(rewritten.trim()),
            ast_mode,
        );
    }
    let helpers = vue3_for_helpers_for_content(&rewritten);
    let mut value = json!({
        "kind": "compound",
        "children": children,
        "loc": loc,
        "astMode": vue3_for_ast_mode_name(ast_mode),
    });
    if !helpers.is_empty() {
        value["helpers"] = json!(helpers);
    }
    value
}

pub(crate) fn vue3_for_simple_projection(
    content: &str,
    loc: Value,
    const_type: u8,
    ast_mode: Vue3ForAstMode,
) -> Value {
    let mut value = json!({
        "kind": "simple",
        "content": content,
        "isStatic": false,
        "constType": const_type,
        "loc": loc,
        "astMode": vue3_for_ast_mode_name(ast_mode),
    });
    let helpers = vue3_for_helpers_for_content(content);
    if !helpers.is_empty() {
        value["helpers"] = json!(helpers);
    }
    value
}

pub(crate) fn vue3_for_compound_children(
    raw: &str,
    options: &Vue3CompilerOptions,
    locals: &[String],
    ast_mode: Vue3ForAstMode,
    loc: &Value,
) -> Vec<Value> {
    let mut children = Vec::new();
    let mut quote = None::<char>;
    let mut escaped = false;
    let mut index = 0usize;
    let mut last = 0usize;
    let chars = raw.char_indices().collect::<Vec<_>>();
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
        let end = chars.get(index).map_or(raw.len(), |(offset, _)| *offset);
        let ident = &raw[start..end];
        let Some(replacement) = vue3_for_identifier_projection_content(
            raw, start, end, ident, options, locals, ast_mode,
        ) else {
            continue;
        };
        if last < start {
            children.push(json!(raw[last..start]));
        }
        children.push(vue3_for_simple_projection(
            &replacement,
            vue3_for_child_loc(loc, raw, start, end),
            if replacement == ident {
                3
            } else {
                vue3_for_const_type(&replacement)
            },
            ast_mode,
        ));
        last = end;
    }
    if last < raw.len() {
        children.push(json!(raw[last..].to_string()));
    }
    children
}

pub(crate) fn vue3_for_identifier_projection_content(
    raw: &str,
    start: usize,
    end: usize,
    ident: &str,
    options: &Vue3CompilerOptions,
    locals: &[String],
    ast_mode: Vue3ForAstMode,
) -> Option<String> {
    if is_keyword(ident) || is_global_or_literal(ident) {
        return None;
    }
    let prev = previous_non_ws(raw, start);
    let next = next_non_ws(raw, end);
    if next == Some(':') {
        return None;
    }
    if prev == Some('.') {
        return Some(ident.to_string());
    }
    if locals.iter().any(|local| local == ident) {
        return Some(ident.to_string());
    }
    if ast_mode == Vue3ForAstMode::Params && next == Some('=') {
        return Some(ident.to_string());
    }
    Some(rewrite_identifier(ident, options))
}

pub(crate) fn previous_non_ws(source: &str, offset: usize) -> Option<char> {
    source[..offset]
        .chars()
        .rev()
        .find(|ch| !ch.is_whitespace())
}

pub(crate) fn vue3_for_exp_loc(exp: &Value, start: usize, end: usize) -> Value {
    let loc = exp.get("loc").unwrap_or(&Value::Null);
    let source = json_str(loc, "source")
        .or_else(|| json_str(exp, "content"))
        .unwrap_or("");
    vue3_for_loc_from_start(loc.get("start").unwrap_or(&Value::Null), source, start, end)
}

pub(crate) fn vue3_for_child_loc(
    parent_loc: &Value,
    source: &str,
    start: usize,
    end: usize,
) -> Value {
    vue3_for_loc_from_start(
        parent_loc.get("start").unwrap_or(&Value::Null),
        source,
        start,
        end,
    )
}

pub(crate) fn vue3_for_loc_from_start(
    start_pos: &Value,
    source: &str,
    start: usize,
    end: usize,
) -> Value {
    let start = start.min(source.len());
    let end = end.min(source.len()).max(start);
    json!({
        "start": vue3_for_advance_position(start_pos, source, start),
        "end": vue3_for_advance_position(start_pos, source, end),
        "source": source.get(start..end).unwrap_or_default(),
    })
}

pub(crate) fn vue3_for_advance_position(start_pos: &Value, source: &str, amount: usize) -> Value {
    let mut offset = start_pos
        .get("offset")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    let mut line = start_pos.get("line").and_then(Value::as_i64).unwrap_or(1);
    let mut column = start_pos.get("column").and_then(Value::as_i64).unwrap_or(1);
    let mut index = 0usize;
    for ch in source.chars() {
        if index >= amount {
            break;
        }
        let len = ch.len_utf8();
        if index + len > amount {
            offset += (amount - index) as i64;
            column += (amount - index) as i64;
            return json!({ "offset": offset, "line": line, "column": column });
        }
        index += len;
        offset += len as i64;
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    if amount > index {
        offset += (amount - index) as i64;
        column += (amount - index) as i64;
    }
    json!({ "offset": offset, "line": line, "column": column })
}

pub(crate) fn vue3_for_ast_mode_name(mode: Vue3ForAstMode) -> &'static str {
    match mode {
        Vue3ForAstMode::Expression => "expression",
        Vue3ForAstMode::Params => "params",
    }
}

pub(crate) fn vue3_for_const_type(content: &str) -> u8 {
    let content = content.trim();
    if matches!(content, "true" | "false" | "null") {
        return 3;
    }
    if (content.starts_with('"') && content.ends_with('"'))
        || (content.starts_with('\'') && content.ends_with('\''))
        || content.parse::<f64>().is_ok()
    {
        return 3;
    }
    0
}

pub(crate) fn vue3_for_helpers_for_content(content: &str) -> Vec<&'static str> {
    let mut helpers = Vec::new();
    if content.contains("_unref(") {
        helpers.push("UNREF");
    }
    if content.contains("_isRef(") {
        helpers.push("IS_REF");
    }
    helpers
}

pub(crate) fn vue3_for_alias_locals(alias: &str) -> Vec<String> {
    let store = JsAstStore::new();
    let wrapped = format!("({alias})=>{{}}");
    if let Ok(Expression::ArrowFunctionExpression(function)) =
        store.parse_expression(&wrapped, oxc_span::SourceType::ts())
    {
        let mut locals = Vec::new();
        for param in &function.params.items {
            collect_vue3_for_binding_pattern(&param.pattern, &mut locals);
        }
        if let Some(rest) = &function.params.rest {
            collect_vue3_for_binding_pattern(&rest.rest.argument, &mut locals);
        }
        locals.sort();
        locals.dedup();
        return locals;
    }
    extract_v_for_alias_locals(alias)
}

pub(crate) fn collect_vue3_for_binding_pattern(
    pattern: &BindingPattern<'_>,
    locals: &mut Vec<String>,
) {
    match pattern {
        BindingPattern::BindingIdentifier(identifier) => {
            locals.push(identifier.name.to_string());
        }
        BindingPattern::ObjectPattern(object) => {
            for property in &object.properties {
                collect_vue3_for_binding_pattern(&property.value, locals);
            }
            if let Some(rest) = &object.rest {
                collect_vue3_for_binding_pattern(&rest.argument, locals);
            }
        }
        BindingPattern::ArrayPattern(array) => {
            for element in array.elements.iter().flatten() {
                collect_vue3_for_binding_pattern(element, locals);
            }
            if let Some(rest) = &array.rest {
                collect_vue3_for_binding_pattern(&rest.argument, locals);
            }
        }
        BindingPattern::AssignmentPattern(assignment) => {
            collect_vue3_for_binding_pattern(&assignment.left, locals);
        }
    }
}

pub(crate) fn vue3_for_template_key_errors(node: &Value) -> Vec<Value> {
    if json_u64(node, "tagType") != Some(3) {
        return Vec::new();
    }
    node.get("children")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|child| json_node_type(child) == Some(1))
        .filter(|child| !vue3_for_child_has_structural_directive(child))
        .filter_map(vue3_for_child_key_loc)
        .take(1)
        .map(|loc| json!({ "code": 33, "loc": loc }))
        .collect()
}

pub(crate) fn vue3_for_child_has_structural_directive(node: &Value) -> bool {
    node.get("props")
        .and_then(Value::as_array)
        .is_some_and(|props| {
            props.iter().any(|prop| {
                json_node_type(prop) == Some(7)
                    && matches!(
                        json_str(prop, "name"),
                        Some("for" | "if" | "else" | "else-if")
                    )
            })
        })
}

pub(crate) fn vue3_for_child_key_loc(node: &Value) -> Option<Value> {
    node.get("props")
        .and_then(Value::as_array)?
        .iter()
        .find(|prop| match json_node_type(prop) {
            Some(6) => json_str(prop, "name") == Some("key"),
            Some(7) => {
                json_str(prop, "name") == Some("bind")
                    && prop.get("arg").is_some_and(|arg| {
                        json_str(arg, "content") == Some("key") && json_bool(arg, "isStatic")
                    })
            }
            _ => false,
        })
        .and_then(|prop| prop.get("loc").cloned())
}
