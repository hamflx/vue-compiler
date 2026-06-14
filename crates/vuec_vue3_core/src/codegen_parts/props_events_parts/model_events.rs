pub(crate) fn render_model_update_prop_exact(
    dir: &Vue3Directive,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> String {
    format!(
        "{}: {}",
        json_key("onUpdate:modelValue"),
        render_model_assignment_for_directive_cached(dir, options, scope, memo_index)
    )
}

pub(crate) fn render_component_model_props(
    dir: &Vue3Directive,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> Vec<String> {
    let prop_name = component_model_prop_name(dir);
    let value = dir
        .exp
        .as_ref()
        .map(Vue3Expression::source_string)
        .unwrap_or_default();
    let value = rewrite_expression_with_scope(&value, options, scope);
    let mut props = vec![format!("{}: {}", json_key(&prop_name), value)];
    props.push(render_component_model_update_prop(
        dir, options, scope, memo_index,
    ));
    let modifiers = component_model_modifiers_prop(dir);
    if !modifiers.is_empty() {
        props.push(modifiers);
    }
    props
}

pub(crate) fn component_model_prop_name(dir: &Vue3Directive) -> String {
    dir.arg
        .as_ref()
        .map(Vue3Expression::source_string)
        .filter(|arg| !arg.trim().is_empty())
        .map(|arg| arg.trim().to_string())
        .unwrap_or_else(|| "modelValue".into())
}

pub(crate) fn render_component_model_update_prop(
    dir: &Vue3Directive,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> String {
    format!(
        "{}: {}",
        json_key(&format!(
            "onUpdate:{}",
            camelize(&component_model_prop_name(dir))
        )),
        render_model_assignment_for_directive_cached(dir, options, scope, memo_index)
    )
}

pub(crate) fn component_model_modifiers_prop(dir: &Vue3Directive) -> String {
    if dir.modifiers.is_empty() {
        return String::new();
    }
    let prop_name = if dir.arg.is_some() {
        format!("{}Modifiers", component_model_prop_name(dir))
    } else {
        "modelModifiers".into()
    };
    let entries = dir
        .modifiers
        .iter()
        .map(|modifier| format!("{modifier}: true"))
        .collect::<Vec<_>>();
    format!("{}: {}", json_key(&prop_name), render_object(&entries))
}

pub(crate) fn render_model_assignment_for_directive(
    dir: &Vue3Directive,
    options: &Vue3CompilerOptions,
) -> String {
    let raw = dir
        .exp
        .as_ref()
        .map(Vue3Expression::source_string)
        .unwrap_or_default();
    let raw = raw.trim();
    render_inline_model_assignment(
        raw,
        "$event",
        options.binding_metadata.get(raw).map(String::as_str),
        options,
        || rewrite_expression_with_scope(raw, options, &RenderScope::default()),
    )
}

pub(crate) fn render_model_assignment_for_directive_cached(
    dir: &Vue3Directive,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> String {
    let raw = dir
        .exp
        .as_ref()
        .map(Vue3Expression::source_string)
        .unwrap_or_default();
    let raw = raw.trim();
    let mut assignment = render_inline_model_assignment(
        raw,
        "$event",
        options.binding_metadata.get(raw).map(String::as_str),
        options,
        || rewrite_expression_with_scope(raw, options, scope),
    );
    if should_cache_model_update_exact(raw, options, scope) {
        let index = memo_index.alloc();
        assignment = format!("_cache[{index}] || (_cache[{index}] = {assignment})");
    }
    assignment
}

pub(crate) fn should_cache_model_update_exact(
    raw: &str,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> bool {
    options.cache_handlers
        && uses_prefixed_identifiers(options)
        && !scope.in_v_once
        && !event_handler_has_scope_ref(raw, scope)
}

pub(crate) fn render_static_binding_prop_key(dir: &Vue3Directive) -> String {
    let mut key = dir
        .arg
        .as_ref()
        .map(Vue3Expression::source_string)
        .unwrap_or_default();
    if dir.modifiers.iter().any(|modifier| modifier == "camel") {
        key = camelize(&key);
    }
    if dir.modifiers.iter().any(|modifier| modifier == "prop") {
        key = format!(".{key}");
    } else if dir.modifiers.iter().any(|modifier| modifier == "attr") {
        key = format!("^{key}");
    }
    key
}

pub(crate) fn render_dynamic_binding_prop_key(
    dir: &Vue3Directive,
    arg: &str,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    let mut key = render_dynamic_prop_key(&rewrite_expression_with_scope(arg, options, scope));
    if dir.modifiers.iter().any(|modifier| modifier == "camel") {
        key = format!("_camelize({key})");
    }
    if dir.modifiers.iter().any(|modifier| modifier == "prop") {
        key = format!("'.' + ({key})");
    } else if dir.modifiers.iter().any(|modifier| modifier == "attr") {
        key = format!("'^' + ({key})");
    }
    key
}

pub(crate) fn render_event_prop(
    element: &Vue3Element,
    dir: &Vue3Directive,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> String {
    let key = render_event_prop_key(element, dir, options, scope);
    let value = render_event_handler_value(element, dir, options, scope, memo_index);
    format!("{key}: {value}")
}

pub(crate) fn render_event_prop_key(
    element: &Vue3Element,
    dir: &Vue3Directive,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    if dir.is_dynamic_arg {
        let event = dir
            .arg
            .as_ref()
            .map(Vue3Expression::source_string)
            .unwrap_or_default();
        let event = rewrite_expression_with_scope(&event, options, scope);
        let event = format!("_toHandlerKey({})", event.trim());
        return format!("[{event}]");
    }

    let event = dir
        .arg
        .as_ref()
        .map(Vue3Expression::source_string)
        .unwrap_or_default();
    json_key(&event_handler_prop_name(element, &event))
}

pub(crate) fn render_event_handler_value(
    element: &Vue3Element,
    dir: &Vue3Directive,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> String {
    let raw = dir
        .exp
        .as_ref()
        .map(Vue3Expression::source_string)
        .unwrap_or_default();
    let raw = raw.trim();
    let is_member = transform_on_is_member_expression(
        raw,
        &json!({
            "allowLexerFallback": true,
            "isTS": options.is_ts,
            "expressionPlugins": options.expression_plugins,
        }),
    );
    let is_fn = transform_on_is_fn_expression(
        raw,
        &json!({
            "isTS": options.is_ts,
            "expressionPlugins": options.expression_plugins,
        }),
    );
    let is_inline = !is_member && !is_fn;
    let mut handler = if raw.is_empty() {
        "() => {}".into()
    } else if is_inline {
        let value = rewrite_handler_expression_with_scope(raw, options, scope);
        let has_multiple_statements = raw.contains(';');
        if has_multiple_statements {
            format!("$event => {{{value}}}")
        } else {
            format!("$event => ({value})")
        }
    } else {
        rewrite_handler_expression_with_scope(raw, options, scope)
    };

    let should_cache =
        should_cache_event_handler(element, dir, options, scope, raw, is_member, is_inline);
    if should_cache && is_member {
        let value = rewrite_handler_expression_with_scope(raw, options, scope);
        handler = format!("(...args) => ({value} && {value}(...args))");
    }
    if should_cache {
        let index = memo_index.alloc();
        if handler.contains('\n') {
            handler = dedent_after_first_line(&handler, 2);
        }
        handler = format!("_cache[{index}] || (_cache[{index}] = {handler})");
    }
    handler
}

pub(crate) fn should_cache_event_handler(
    element: &Vue3Element,
    dir: &Vue3Directive,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    raw: &str,
    is_member: bool,
    is_inline: bool,
) -> bool {
    if !options.cache_handlers || scope.in_v_once {
        return false;
    }
    if !uses_prefixed_identifiers(options) {
        return false;
    }
    if raw.is_empty() {
        return true;
    }
    if element.tag_type == Vue3ElementType::Component && is_member {
        return false;
    }
    if event_handler_has_scope_ref(raw, scope) {
        return false;
    }
    if !is_inline && event_handler_is_const_binding(raw, options) {
        return false;
    }
    if is_inline && vue3_for_const_type(raw) > 0 {
        return false;
    }
    if dir.is_dynamic_arg {
        return true;
    }
    true
}

pub(crate) fn event_handler_has_scope_ref(raw: &str, scope: &RenderScope) -> bool {
    scope
        .locals
        .iter()
        .any(|local| source_contains_identifier(raw, local))
}

pub(crate) fn event_handler_is_const_binding(raw: &str, options: &Vue3CompilerOptions) -> bool {
    let trimmed = raw.trim();
    is_simple_identifier_ascii(trimmed)
        && matches!(
            options.binding_metadata.get(trimmed).map(String::as_str),
            Some("setup-const" | "literal-const")
        )
}
