pub(crate) fn transform_bind_guarded_arg_projection(arg: Option<&Value>, dir: &Value) -> Value {
    let loc = arg
        .and_then(|arg| arg.get("loc").cloned())
        .unwrap_or_else(|| dir.get("loc").cloned().unwrap_or(Value::Null));
    let Some(arg) = arg else {
        return json!({
            "kind": "simple",
            "content": "",
            "isStatic": true,
            "loc": loc,
        });
    };

    if json_node_type(arg) == Some(4) {
        let content = json_str(arg, "content").unwrap_or("");
        if json_bool(arg, "isStatic") {
            return json!({
                "kind": "simple",
                "content": content,
                "isStatic": true,
                "loc": loc,
            });
        }
        return json!({
            "kind": "simple",
            "content": if content.is_empty() { "\"\"".to_string() } else { format!("{content} || \"\"") },
            "isStatic": false,
            "loc": loc,
            "constType": arg.get("constType").cloned().unwrap_or(json!(0)),
        });
    }

    json!({
        "kind": "compound",
        "children": [
            "(",
            { "kind": "node", "path": "dir.arg.children" },
            ") || \"\"",
        ],
        "loc": loc,
        "constType": arg.get("constType").cloned().unwrap_or(json!(0)),
    })
}

pub(crate) fn transform_bind_empty_expression_value(dir: &Value) -> Value {
    json!({
        "kind": "simple",
        "content": "",
        "isStatic": true,
        "loc": dir.get("loc").cloned().unwrap_or(Value::Null),
    })
}

pub(crate) fn transform_bind_camel_projection(key: Value) -> Value {
    match json_str(&key, "kind") {
        Some("simple") if json_bool(&key, "isStatic") => {
            let mut next = key;
            let content = json_str(&next, "content").unwrap_or("").to_string();
            next["content"] = json!(camelize(&content));
            next
        }
        Some("simple") => {
            let mut next = key;
            let content = json_str(&next, "content").unwrap_or("").to_string();
            next["content"] = json!(format!("_camelize({content})"));
            next["helpers"] = json!(["CAMELIZE"]);
            next
        }
        Some("compound") => {
            let children = key.get("children").cloned().unwrap_or_else(|| json!([]));
            let mut next = key;
            next["children"] = json!([
                { "kind": "helperString", "helper": "CAMELIZE" },
                { "kind": "children", "children": children },
                ")",
            ]);
            next
        }
        _ => key,
    }
}

pub(crate) fn transform_bind_prefix_projection(key: Value, prefix: &str) -> Value {
    match json_str(&key, "kind") {
        Some("simple") if json_bool(&key, "isStatic") => {
            let mut next = key;
            let content = json_str(&next, "content").unwrap_or("").to_string();
            next["content"] = json!(format!("{prefix}{content}"));
            next
        }
        Some("simple") => {
            let mut next = key;
            let content = json_str(&next, "content").unwrap_or("").to_string();
            next["content"] = json!(format!("`{prefix}${{{content}}}`"));
            next
        }
        Some("compound") => {
            let children = key.get("children").cloned().unwrap_or_else(|| json!([]));
            let mut next = key;
            next["children"] = json!([
                format!("'{prefix}' + ("),
                { "kind": "children", "children": children },
                ")",
            ]);
            next
        }
        _ => key,
    }
}

pub(crate) fn transform_v_bind_shorthand_operation(
    index: usize,
    prop: &Value,
    browser: bool,
) -> Option<Value> {
    if json_node_type(prop) != Some(7)
        || json_str(prop, "name") != Some("bind")
        || prop.get("arg").is_none_or(Value::is_null)
        || !transform_v_bind_shorthand_needs_expansion(prop, browser)
    {
        return None;
    }

    let arg = prop.get("arg").unwrap_or(&Value::Null);
    let loc = arg.get("loc").cloned().unwrap_or(Value::Null);
    if json_node_type(arg) != Some(4) || !json_bool(arg, "isStatic") {
        return Some(json!({
            "kind": "setExp",
            "index": index,
            "exp": {
                "kind": "simple",
                "content": "",
                "isStatic": true,
                "loc": loc,
            },
            "errors": [{ "code": 53, "loc": "arg" }],
        }));
    }

    let prop_name = camelize(json_str(arg, "content").unwrap_or(""));
    if !transform_v_bind_shorthand_valid_first_char(&prop_name) {
        return None;
    }
    Some(json!({
        "kind": "setExp",
        "index": index,
        "exp": {
            "kind": "simple",
            "content": prop_name,
            "isStatic": false,
            "loc": loc,
        },
        "errors": [],
    }))
}

pub(crate) fn transform_v_bind_shorthand_needs_expansion(prop: &Value, browser: bool) -> bool {
    match prop.get("exp").filter(|value| !value.is_null()) {
        None => true,
        Some(exp) => {
            browser
                && json_node_type(exp) == Some(4)
                && json_str(exp, "content").unwrap_or("").trim().is_empty()
        }
    }
}

pub(crate) fn transform_v_bind_shorthand_valid_first_char(value: &str) -> bool {
    value.chars().next().is_some_and(|ch| {
        ch == '-' || ch == '_' || ch == '$' || ch.is_ascii_alphabetic() || ch >= '\u{00a0}'
    })
}

pub(crate) fn directive_has_modifier(dir: &Value, name: &str) -> bool {
    dir.get("modifiers")
        .and_then(Value::as_array)
        .is_some_and(|modifiers| {
            modifiers.iter().any(|modifier| {
                modifier.as_str().or_else(|| json_str(modifier, "content")) == Some(name)
            })
        })
}

pub(crate) fn transform_on_event_name_projection(
    arg: Option<&Value>,
    node: &Value,
    errors: &mut Vec<Value>,
) -> Value {
    let Some(arg) = arg else {
        return json!({ "kind": "static", "content": "on" });
    };
    if json_node_type(arg) == Some(4) {
        if json_bool(arg, "isStatic") {
            let mut raw_name = json_str(arg, "content").unwrap_or("").to_string();
            if raw_name.starts_with("vnode") {
                errors.push(json!({ "code": 52, "loc": "arg" }));
            }
            if let Some(rest) = raw_name.strip_prefix("vue:") {
                raw_name = format!("vnode-{rest}");
            }
            let event_string = if json_u64(node, "tagType") != Some(0)
                || raw_name.starts_with("vnode")
                || !raw_name.chars().any(|ch| ch.is_ascii_uppercase())
            {
                to_handler_key(&camelize(&raw_name))
            } else {
                format!("on:{raw_name}")
            };
            return json!({
                "kind": "simple",
                "content": event_string,
                "isStatic": true,
                "loc": arg.get("loc").cloned().unwrap_or(Value::Null),
            });
        }
        return json!({
            "kind": "compound",
            "children": [
                { "kind": "helperString", "helper": "TO_HANDLER_KEY" },
                { "kind": "node", "path": "dir.arg" },
                ")",
            ],
        });
    }
    json!({
        "kind": "compound",
        "children": [
            { "kind": "helperString", "helper": "TO_HANDLER_KEY" },
            { "kind": "node", "path": "dir.arg.children" },
            ")",
        ],
        "loc": arg.get("loc").cloned().unwrap_or(Value::Null),
    })
}

pub(crate) fn transform_on_handler_projection(dir: &Value, node: &Value, context: &Value) -> Value {
    let Some(exp) = dir.get("exp").filter(|value| !value.is_null()) else {
        return json!({ "cache": json_bool(context, "cacheHandlers") && !json_bool(context, "inVOnce") });
    };
    let raw = transform_on_expression_source(exp);
    if raw.trim().is_empty() {
        return json!({ "cache": json_bool(context, "cacheHandlers") && !json_bool(context, "inVOnce") });
    }

    let is_member = transform_on_is_member_expression(&raw, context);
    let is_fn = transform_on_is_fn_expression(&raw, context);
    let is_inline = !is_member && !is_fn;
    let has_multiple_statements = raw.contains(';');
    let mut processed = json!({ "kind": "node", "path": "dir.exp" });
    let mut should_cache = false;

    if json_bool(context, "prefixIdentifiers") {
        let options = vue3_options_from_transform_context(context);
        let mut locals = transform_context_locals(context);
        if is_inline {
            locals.push("$event".to_string());
        }
        processed = transform_on_rewrite_expression_node(
            &raw,
            exp,
            &options,
            &locals,
            has_multiple_statements,
        );
        should_cache = json_bool(context, "cacheHandlers")
            && !json_bool(context, "inVOnce")
            && transform_on_projection_const_type(&processed) == 0
            && !(is_member && json_u64(node, "tagType") == Some(1))
            && !transform_on_has_scope_ref(&processed, context);
        if should_cache && is_member {
            processed = transform_on_member_invocation_projection(processed);
        }
    }

    if is_inline || (should_cache && is_member) {
        processed = transform_on_wrap_handler_projection(
            processed,
            is_inline,
            has_multiple_statements,
            json_bool(context, "isTS"),
        );
    }

    json!({
        "value": processed,
        "cache": should_cache,
        "isInlineStatement": is_inline,
        "isMemberExpression": is_member,
        "isFunctionExpression": is_fn,
    })
}

pub(crate) fn transform_on_empty_handler_projection(dir: &Value) -> Value {
    json!({
        "kind": "simple",
        "content": "() => {}",
        "isStatic": false,
        "loc": dir.get("loc").cloned().unwrap_or(Value::Null),
    })
}

pub(crate) fn transform_on_rewrite_expression_node(
    raw: &str,
    exp: &Value,
    options: &Vue3CompilerOptions,
    locals: &[String],
    as_raw_statements: bool,
) -> Value {
    let trimmed = raw.trim();
    let loc = exp.get("loc").cloned().unwrap_or(Value::Null);
    let mut effective_locals = locals.to_vec();
    effective_locals.extend(transform_on_root_function_locals(raw));
    effective_locals.sort();
    effective_locals.dedup();
    let rewritten = if effective_locals.is_empty() {
        rewrite_js_like_expression(raw, options)
    } else {
        rewrite_js_like_expression_with_locals(raw, options, &effective_locals)
    };
    let children = process_expression_compound_children(raw, options, &effective_locals, &loc);
    let const_type = transform_on_const_type(trimmed, rewritten.trim(), options);
    if is_simple_identifier_ascii(trimmed) || (children.is_empty() && !as_raw_statements) {
        return transform_on_simple_projection(rewritten.trim(), exp, const_type);
    }
    let helpers = vue3_for_helpers_for_content(&rewritten);
    let mut value = json!({
        "kind": "compound",
        "children": children,
        "loc": loc,
        "constType": const_type,
    });
    if !helpers.is_empty() {
        value["helpers"] = json!(helpers);
    }
    value
}

pub(crate) fn transform_on_simple_projection(content: &str, exp: &Value, const_type: u8) -> Value {
    let mut value = json!({
        "kind": "simple",
        "content": content,
        "isStatic": false,
        "constType": const_type,
        "loc": exp.get("loc").cloned().unwrap_or(Value::Null),
    });
    let helpers = vue3_for_helpers_for_content(content);
    if !helpers.is_empty() {
        value["helpers"] = json!(helpers);
    }
    value
}

pub(crate) fn transform_on_const_type(
    raw: &str,
    rewritten: &str,
    options: &Vue3CompilerOptions,
) -> u8 {
    if is_simple_identifier_ascii(raw)
        && matches!(
            options.binding_metadata.get(raw).map(String::as_str),
            Some("setup-const" | "literal-const")
        )
    {
        return 1;
    }
    vue3_for_const_type(rewritten)
}

pub(crate) fn transform_on_member_invocation_projection(processed: Value) -> Value {
    match json_str(&processed, "kind") {
        Some("simple") => {
            let content = json_str(&processed, "content").unwrap_or("").to_string();
            let mut next = processed;
            next["content"] = json!(format!("{content} && {content}(...args)"));
            next
        }
        Some("compound") => {
            let children = processed
                .get("children")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let mut next_children = children.clone();
            next_children.push(json!(" && "));
            next_children.extend(children);
            next_children.push(json!("(...args)"));
            let mut next = processed;
            next["children"] = json!(next_children);
            next
        }
        _ => processed,
    }
}

pub(crate) fn transform_on_wrap_handler_projection(
    processed: Value,
    is_inline: bool,
    has_multiple_statements: bool,
    is_ts: bool,
) -> Value {
    let param = if is_inline {
        if is_ts {
            "($event: any)"
        } else {
            "$event"
        }
    } else if is_ts {
        "\n//@ts-ignore\n(...args)"
    } else {
        "(...args)"
    };
    json!({
        "kind": "compound",
        "children": [
            format!("{param} => {}", if has_multiple_statements { "{" } else { "(" }),
            processed,
            if has_multiple_statements { "}" } else { ")" },
        ],
    })
}

pub(crate) fn transform_on_expression_source(exp: &Value) -> String {
    if let Some(content) = json_str(exp, "content") {
        return content.to_string();
    }
    exp.get("loc")
        .and_then(|loc| json_str(loc, "source"))
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| model_expression_source(exp))
}

pub(crate) fn process_expression_locals(payload: &Value, context: &Value) -> Vec<String> {
    if let Some(locals) = payload.get("localVars").and_then(Value::as_object) {
        return locals
            .iter()
            .filter(|(_, count)| count.as_i64().unwrap_or(1) > 0)
            .map(|(name, _)| name.clone())
            .collect();
    }
    transform_context_locals(context)
}

pub(crate) fn process_expression_is_const_binding(
    raw: &str,
    options: &Vue3CompilerOptions,
) -> bool {
    matches!(
        options.binding_metadata.get(raw).map(String::as_str),
        Some("setup-const" | "literal-const")
    )
}

pub(crate) fn process_expression_is_static_literal(raw: &str) -> bool {
    let trimmed = raw.trim();
    matches!(trimmed, "true" | "false" | "null" | "this")
        || trimmed.ends_with('n')
            && trimmed[..trimmed.len().saturating_sub(1)]
                .parse::<i128>()
                .is_ok()
        || trimmed.parse::<f64>().is_ok()
}

pub(crate) fn process_expression_uses_supported_external_plugin(
    raw: &str,
    context: &Value,
) -> bool {
    raw.contains("|>")
        && context
            .get("expressionPlugins")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .any(|plugin| {
                plugin
                    .as_str()
                    .is_some_and(|name| name == "pipelineOperator")
                    || plugin
                        .as_array()
                        .and_then(|items| items.first())
                        .and_then(Value::as_str)
                        == Some("pipelineOperator")
            })
}

pub(crate) fn process_expression_rewrite_source(
    raw: &str,
    options: &Vue3CompilerOptions,
    locals: &[String],
) -> String {
    let mut identifiers = process_expression_identifier_spans(raw, options, locals);
    identifiers.sort_by_key(|identifier| (identifier.start, identifier.end));
    let mut filtered = Vec::<ProcessExpressionIdentifier>::new();
    for identifier in identifiers {
        if identifier.start >= identifier.end || identifier.end > raw.len() {
            continue;
        }
        if filtered
            .last()
            .is_some_and(|previous| identifier.start < previous.end)
        {
            continue;
        }
        filtered.push(identifier);
    }

    if filtered.is_empty() {
        return raw.to_string();
    }

    let mut output = String::new();
    let mut last_end = 0usize;
    for identifier in &filtered {
        output.push_str(raw.get(last_end..identifier.start).unwrap_or_default());
        if let Some(prefix) = &identifier.prefix {
            output.push_str(prefix);
        }
        let content = parenthesize_rewritten_identifier_for_new_expression(
            raw,
            identifier.start,
            identifier.end,
            &identifier.content,
        );
        output.push_str(&content);
        last_end = identifier.end;
    }
    output.push_str(raw.get(last_end..).unwrap_or_default());
    output
}

pub(crate) fn parenthesize_rewritten_identifier_for_new_expression(
    raw: &str,
    start: usize,
    end: usize,
    content: &str,
) -> String {
    if !content.starts_with("_unref(") || !process_expression_is_in_new_expression(raw, start) {
        return content.to_string();
    }
    match next_non_ws(raw, end) {
        Some('.') | Some('(') => format!("({content})"),
        _ => content.to_string(),
    }
}

pub(crate) fn process_expression_compound_children(
    raw: &str,
    options: &Vue3CompilerOptions,
    locals: &[String],
    loc: &Value,
) -> Vec<Value> {
    let mut identifiers = process_expression_identifier_spans(raw, options, locals);
    identifiers.sort_by_key(|identifier| (identifier.start, identifier.end));
    let mut filtered = Vec::<ProcessExpressionIdentifier>::new();
    for identifier in identifiers {
        if identifier.start >= identifier.end || identifier.end > raw.len() {
            continue;
        }
        if filtered
            .last()
            .is_some_and(|previous| identifier.start < previous.end)
        {
            continue;
        }
        filtered.push(identifier);
    }

    let mut children = Vec::new();
    for (index, identifier) in filtered.iter().enumerate() {
        let leading_start = filtered
            .get(index.wrapping_sub(1))
            .map(|last| last.end)
            .unwrap_or(0);
        if leading_start < identifier.start || identifier.prefix.is_some() {
            children.push(json!(format!(
                "{}{}",
                raw.get(leading_start..identifier.start).unwrap_or_default(),
                identifier.prefix.as_deref().unwrap_or("")
            )));
        }
        let source = raw
            .get(identifier.start..identifier.end)
            .unwrap_or_default()
            .to_string();
        children.push(json!({
            "kind": "simple",
            "content": identifier.content,
            "isStatic": false,
            "constType": if identifier.is_constant { 3 } else { 0 },
            "loc": vue3_for_child_loc(loc, raw, identifier.start, identifier.end),
        }));
        if index + 1 == filtered.len() && identifier.end < raw.len() {
            children.push(json!(raw[identifier.end..].to_string()));
        }
        let _ = source;
    }
    children
}

pub(crate) fn process_expression_params_projection(
    raw: &str,
    node: &Value,
    context: &Value,
    options: &Vue3CompilerOptions,
) -> Value {
    let source = format!("({raw})=>{{}}");
    let store = JsAstStore::new();
    if store
        .parse_expression(&source, transform_on_source_type(context))
        .is_err()
    {
        return json!({
            "kind": "error",
            "code": 46,
            "loc": node.get("loc").cloned().unwrap_or(Value::Null),
            "message": "Error parsing JavaScript expression: Unexpected token",
        });
    }
    let children =
        process_expression_params_children(raw, options, node.get("loc").unwrap_or(&Value::Null));
    if children.is_empty() {
        return json!({
            "kind": "setConstType",
            "constType": 3,
        });
    }
    let identifiers = vue3_for_alias_locals(raw);
    let mut helper_source = String::new();
    for child in &children {
        if let Some(content) = child.get("content").and_then(Value::as_str) {
            helper_source.push_str(content);
        }
    }
    json!({
        "kind": "compound",
        "children": children,
        "loc": node.get("loc").cloned().unwrap_or(Value::Null),
        "identifiers": identifiers,
        "helpers": vue3_for_helpers_for_content(&helper_source),
    })
}

pub(crate) fn process_expression_params_children(
    raw: &str,
    options: &Vue3CompilerOptions,
    loc: &Value,
) -> Vec<Value> {
    let mut identifiers = process_expression_param_identifier_spans(raw, (0, raw.len()), options);
    identifiers.sort_by_key(|identifier| (identifier.start, identifier.end));
    let mut filtered = Vec::<ProcessExpressionIdentifier>::new();
    for identifier in identifiers {
        if filtered
            .last()
            .is_some_and(|previous| identifier.start < previous.end)
        {
            continue;
        }
        filtered.push(identifier);
    }
    process_expression_children_from_identifiers(raw, loc, &filtered)
}

pub(crate) fn process_expression_children_from_identifiers(
    raw: &str,
    loc: &Value,
    identifiers: &[ProcessExpressionIdentifier],
) -> Vec<Value> {
    let mut children = Vec::new();
    for (index, identifier) in identifiers.iter().enumerate() {
        let leading_start = identifiers
            .get(index.wrapping_sub(1))
            .map(|last| last.end)
            .unwrap_or(0);
        if leading_start < identifier.start || identifier.prefix.is_some() {
            children.push(json!(format!(
                "{}{}",
                raw.get(leading_start..identifier.start).unwrap_or_default(),
                identifier.prefix.as_deref().unwrap_or("")
            )));
        }
        children.push(json!({
            "kind": "simple",
            "content": identifier.content,
            "isStatic": false,
            "constType": if identifier.is_constant { 3 } else { 0 },
            "loc": vue3_for_child_loc(loc, raw, identifier.start, identifier.end),
        }));
        if index + 1 == identifiers.len() && identifier.end < raw.len() {
            children.push(json!(raw[identifier.end..].to_string()));
        }
    }
    children
}

#[derive(Clone, Debug)]
pub(crate) struct ProcessExpressionIdentifier {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) content: String,
    pub(crate) prefix: Option<String>,
    pub(crate) is_constant: bool,
}

#[derive(Clone, Copy, Debug)]
struct ProcessExpressionArrowScope {
    body_start: usize,
    max_body_end: usize,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ProcessExpressionArrowBindingIndex {
    params: BTreeSet<(usize, usize)>,
    scopes: BTreeMap<String, Vec<ProcessExpressionArrowScope>>,
}

#[derive(Clone, Debug)]
pub(crate) struct ProcessExpressionAssignmentRhs<'a> {
    pub(crate) operator: &'a str,
    pub(crate) source: &'a str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ProcessExpressionUpdate {
    pub(crate) operator: &'static str,
    pub(crate) prefix: bool,
}

pub(crate) fn process_expression_identifier_spans(
    raw: &str,
    options: &Vue3CompilerOptions,
    locals: &[String],
) -> Vec<ProcessExpressionIdentifier> {
    let mut spans = Vec::new();
    let mut quote = None::<char>;
    let mut escaped = false;
    let mut chars = raw.char_indices().peekable();
    let arrow_bindings = process_expression_arrow_bindings(raw);
    let function_bindings =
        process_expression_function_bindings(raw, expression_source_type(options));
    while let Some((start, ch)) = chars.next() {
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
        if matches!(ch, '\'' | '"' | '`') {
            quote = Some(ch);
            continue;
        }
        if !is_identifier_start(ch) {
            continue;
        }
        let mut end = start + ch.len_utf8();
        while let Some(&(offset, next)) = chars.peek() {
            if !is_identifier_continue(next) {
                break;
            }
            chars.next();
            end = offset + next.len_utf8();
        }
        let ident = &raw[start..end];
        let prev = previous_non_ws(raw, start);
        let next = next_non_ws(raw, end);
        if is_keyword(ident) {
            continue;
        }
        if matches!(ident, "true" | "false" | "null" | "this") {
            continue;
        }
        let local = locals.iter().any(|local| local == ident);
        let property_key = next == Some(':') && prev != Some('?');
        let static_member = prev == Some('.');
        if process_expression_is_function_non_reference_key(&function_bindings, start, end) {
            continue;
        }
        let arrow_param = process_expression_is_arrow_param(&arrow_bindings, start, end);
        let arrow_local = process_expression_is_arrow_local(&arrow_bindings, ident, start, end);
        let function_param = arrow_param
            || process_expression_is_function_binding(
                &function_bindings,
                ident,
                start,
                end,
            );
        if property_key && !function_param {
            continue;
        }
        let is_global = is_global_or_literal(ident);
        let assignment_rhs = process_expression_assignment_rhs(raw, start, end);
        let update_argument = process_expression_update_argument(raw, start, end);
        let destructure_assignment = process_expression_is_destructure_assignment(raw, start);
        let content = if static_member || local || function_param || arrow_local || is_global {
            if !static_member
                && !local
                && !function_param
                && arrow_local
                && (assignment_rhs.is_some() || update_argument.is_some() || destructure_assignment)
                && options.binding_metadata.contains_key(ident)
            {
                process_expression_rewrite_identifier(
                    ident,
                    options,
                    assignment_rhs.as_ref(),
                    update_argument,
                    destructure_assignment,
                    locals,
                )
            } else {
                ident.to_string()
            }
        } else {
            process_expression_rewrite_identifier(
                ident,
                options,
                assignment_rhs.as_ref(),
                update_argument,
                destructure_assignment,
                locals,
            )
        };
        let (replacement_start, replacement_end) = if let Some(update) =
            update_argument.filter(|update| content != ident && content.contains(update.operator))
        {
            process_expression_update_range(raw, start, end, update).unwrap_or((start, end))
        } else {
            (start, end)
        };
        let object_shorthand = process_expression_object_shorthand(raw, start, end);
        let prefix = if property_key && content != ident
            || object_shorthand
                && (content != ident
                    || destructure_assignment
                        && options
                            .binding_metadata
                            .get(ident)
                            .is_some_and(|kind| kind == "setup-let"))
        {
            Some(format!("{ident}: "))
        } else {
            None
        };
        let dynamic_static_reference = (static_member || is_global)
            && process_expression_dynamic_static_reference(raw, start, end);
        spans.push(ProcessExpressionIdentifier {
            start: replacement_start,
            end: replacement_end,
            content,
            prefix,
            is_constant: ((static_member || is_global) && !dynamic_static_reference)
                || function_param
                || arrow_local,
        });
    }
    spans
}

pub(crate) fn process_expression_param_identifier_spans(
    raw: &str,
    range: (usize, usize),
    options: &Vue3CompilerOptions,
) -> Vec<ProcessExpressionIdentifier> {
    let mut spans = Vec::new();
    let mut quote = None::<char>;
    let mut escaped = false;
    let mut chars = raw[range.0..range.1]
        .char_indices()
        .map(|(offset, ch)| (range.0 + offset, ch))
        .peekable();
    while let Some((start, ch)) = chars.next() {
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
        if matches!(ch, '\'' | '"' | '`') {
            quote = Some(ch);
            continue;
        }
        if !is_identifier_start(ch) {
            continue;
        }
        let mut end = start + ch.len_utf8();
        while let Some(&(offset, next)) = chars.peek() {
            if !is_identifier_continue(next) {
                break;
            }
            chars.next();
            end = offset + next.len_utf8();
        }
        let ident = &raw[start..end];
        if is_keyword(ident) || next_non_ws(raw, end) == Some(':') {
            continue;
        }
        if process_expression_param_default_rhs(raw, range.0, start) {
            let content = if is_global_or_literal(ident) {
                ident.to_string()
            } else {
                process_expression_rewrite_identifier(ident, options, None, None, false, &[])
            };
            spans.push(ProcessExpressionIdentifier {
                start,
                end,
                content,
                prefix: None,
                is_constant: is_global_or_literal(ident),
            });
        } else {
            spans.push(ProcessExpressionIdentifier {
                start,
                end,
                content: ident.to_string(),
                prefix: None,
                is_constant: true,
            });
        }
    }
    spans
}

pub(crate) fn process_expression_is_arrow_param(
    bindings: &ProcessExpressionArrowBindingIndex,
    start: usize,
    end: usize,
) -> bool {
    bindings.params.contains(&(start, end))
}

pub(crate) fn process_expression_is_arrow_local(
    bindings: &ProcessExpressionArrowBindingIndex,
    ident: &str,
    start: usize,
    end: usize,
) -> bool {
    let Some(scopes) = bindings.scopes.get(ident) else {
        return false;
    };
    let containing = scopes.partition_point(|scope| scope.body_start <= start);
    containing > 0 && end <= scopes[containing - 1].max_body_end
}

pub(crate) fn process_expression_is_in_new_expression(raw: &str, start: usize) -> bool {
    let head = raw.get(..start).unwrap_or("").trim_end();
    if head.ends_with("new") {
        return head
            .strip_suffix("new")
            .and_then(|before| before.chars().next_back())
            .is_none_or(|ch| !is_identifier_continue(ch));
    }
    let mut depth = 0usize;
    for (index, ch) in head.char_indices().rev() {
        match ch {
            ')' | ']' => depth += 1,
            '(' | '[' => {
                depth = depth.saturating_sub(1);
            }
            '.' if depth == 0 => {
                return process_expression_is_in_new_expression(raw, index);
            }
            ch if ch.is_whitespace() && depth == 0 => {
                let prefix = head.get(..index).unwrap_or("").trim_end();
                return prefix.ends_with("new")
                    && prefix
                        .strip_suffix("new")
                        .and_then(|before| before.chars().next_back())
                        .is_none_or(|before| !is_identifier_continue(before));
            }
            _ => {}
        }
    }
    false
}

pub(crate) fn process_expression_object_shorthand(raw: &str, start: usize, end: usize) -> bool {
    previous_non_ws(raw, start).is_some_and(|prev| matches!(prev, '{' | ','))
        && next_non_ws(raw, end).is_some_and(|next| matches!(next, '}' | ','))
}

pub(crate) fn process_expression_rewrite_identifier(
    ident: &str,
    options: &Vue3CompilerOptions,
    assignment_rhs: Option<&ProcessExpressionAssignmentRhs<'_>>,
    update_argument: Option<ProcessExpressionUpdate>,
    destructure_assignment: bool,
    locals: &[String],
) -> String {
    match options.binding_metadata.get(ident).map(String::as_str) {
        Some("setup-ref") if options.inline => {
            if let Some(update) = update_argument {
                let prefix = if update.prefix { update.operator } else { "" };
                let postfix = if update.prefix { "" } else { update.operator };
                format!("{prefix}{ident}.value{postfix}")
            } else {
                format!("{ident}.value")
            }
        }
        Some("setup-maybe-ref") if options.inline => {
            if let Some(update) = update_argument {
                let prefix = if update.prefix { update.operator } else { "" };
                let postfix = if update.prefix { "" } else { update.operator };
                format!("{prefix}{ident}.value{postfix}")
            } else if assignment_rhs.is_some() || destructure_assignment {
                format!("{ident}.value")
            } else {
                format!("_unref({ident})")
            }
        }
        Some("setup-let") if options.inline => {
            if let Some(rhs) = assignment_rhs {
                let rewritten_rhs = process_expression_rewrite_source(rhs.source, options, locals);
                format!(
                    "_isRef({ident}) ? {ident}.value {} {} : {ident}",
                    rhs.operator,
                    rewritten_rhs.trim()
                )
            } else if let Some(update) = update_argument {
                let prefix = if update.prefix { update.operator } else { "" };
                let postfix = if update.prefix { "" } else { update.operator };
                format!(
                    "_isRef({ident}) ? {prefix}{ident}.value{postfix} : {prefix}{ident}{postfix}"
                )
            } else if destructure_assignment {
                ident.to_string()
            } else {
                format!("_unref({ident})")
            }
        }
        _ => rewrite_identifier(ident, options),
    }
}

pub(crate) fn process_expression_arrow_bindings(
    raw: &str,
) -> ProcessExpressionArrowBindingIndex {
    let mut bindings = ProcessExpressionArrowBindingIndex::default();
    let mut arrows = Vec::new();
    for arrow in process_expression_arrow_offsets(raw) {
        let Some(param_range) = process_expression_arrow_param_range(raw, arrow) else {
            continue;
        };
        let body_start = skip_ws_forward(raw, arrow + 2);
        arrows.push((param_range, body_start));
    }
    let body_starts = arrows
        .iter()
        .map(|(_, body_start)| *body_start)
        .collect::<Vec<_>>();
    let body_ends = process_expression_arrow_body_ends(raw, &body_starts);
    for ((param_range, body_start), body_end) in arrows.into_iter().zip(body_ends) {
        for (param_start, param_end) in process_expression_param_binding_spans(raw, param_range) {
            bindings.params.insert((param_start, param_end));
            bindings
                .scopes
                .entry(raw[param_start..param_end].to_string())
                .or_default()
                .push(ProcessExpressionArrowScope {
                    body_start,
                    max_body_end: body_end,
                });
        }
    }
    for scopes in bindings.scopes.values_mut() {
        scopes.sort_unstable_by_key(|scope| (scope.body_start, scope.max_body_end));
        let mut max_body_end = 0usize;
        for scope in scopes {
            max_body_end = max_body_end.max(scope.max_body_end);
            scope.max_body_end = max_body_end;
        }
    }
    bindings
}

pub(crate) fn process_expression_arrow_offsets(raw: &str) -> Vec<usize> {
    let mut offsets = Vec::new();
    let mut quote = None::<char>;
    let mut escaped = false;
    let mut chars = raw.char_indices();
    while let Some((offset, ch)) = chars.next() {
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
        if matches!(ch, '\'' | '"' | '`') {
            quote = Some(ch);
            continue;
        }
        if ch == '=' && raw[offset..].starts_with("=>") {
            offsets.push(offset);
            chars.next();
            continue;
        }
    }
    offsets
}

pub(crate) fn process_expression_arrow_param_range(
    raw: &str,
    arrow: usize,
) -> Option<(usize, usize)> {
    let (param_end, end_char) = previous_non_ws_index(raw, arrow)?;
    if end_char == ')' {
        let open = find_matching_backward(raw, param_end, '(', ')')?;
        return Some((open + 1, param_end));
    }
    if !is_identifier_continue(end_char) {
        return None;
    }
    let mut start = param_end;
    while start > 0 {
        let Some((prev, ch)) = previous_char(raw, start) else {
            break;
        };
        if !is_identifier_continue(ch) {
            break;
        }
        start = prev;
    }
    Some((start, param_end + end_char.len_utf8()))
}

pub(crate) fn process_expression_param_binding_spans(
    raw: &str,
    range: (usize, usize),
) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut quote = None::<char>;
    let mut escaped = false;
    let mut chars = raw[range.0..range.1]
        .char_indices()
        .map(|(offset, ch)| (range.0 + offset, ch))
        .peekable();
    while let Some((start, ch)) = chars.next() {
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
        if matches!(ch, '\'' | '"' | '`') {
            quote = Some(ch);
            continue;
        }
        if !is_identifier_start(ch) {
            continue;
        }
        let mut end = start + ch.len_utf8();
        while let Some(&(offset, next)) = chars.peek() {
            if !is_identifier_continue(next) {
                break;
            }
            chars.next();
            end = offset + next.len_utf8();
        }
        let ident = &raw[start..end];
        if is_keyword(ident)
            || process_expression_param_default_rhs(raw, range.0, start)
            || next_non_ws(raw, end) == Some(':')
        {
            continue;
        }
        spans.push((start, end));
    }
    spans
}

pub(crate) fn process_expression_param_default_rhs(
    raw: &str,
    range_start: usize,
    start: usize,
) -> bool {
    let mut offset = start;
    while offset > range_start {
        let Some((prev, ch)) = previous_char(raw, offset) else {
            break;
        };
        if ch.is_whitespace() {
            offset = prev;
            continue;
        }
        return ch == '=';
    }
    false
}
