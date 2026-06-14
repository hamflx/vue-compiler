pub(crate) fn model_assignment_projection(
    exp: &Value,
    raw_exp: &str,
    event_arg: &str,
    binding_type: Option<&str>,
    maybe_ref: bool,
) -> Value {
    if maybe_ref {
        if binding_type == Some("setup-ref") {
            return json!({
                "kind": "compound",
                "children": [
                    format!("{event_arg} => (("),
                    { "kind": "simple", "content": raw_exp, "isStatic": false, "loc": exp.get("loc").cloned().unwrap_or(Value::Null) },
                    ").value = $event)"
                ]
            });
        }
        let alt_assignment = if binding_type == Some("setup-let") {
            format!("{raw_exp} = $event")
        } else {
            "null".to_string()
        };
        return json!({
            "kind": "compound",
            "children": [
                format!("{event_arg} => (_isRef({raw_exp}) ? ("),
                { "kind": "simple", "content": raw_exp, "isStatic": false, "loc": exp.get("loc").cloned().unwrap_or(Value::Null) },
                format!(").value = $event : {alt_assignment})")
            ],
            "helpers": ["IS_REF"]
        });
    }

    json!({
        "kind": "compound",
        "children": [
            format!("{event_arg} => (("),
            { "kind": "node", "path": "dir.exp" },
            ") = $event)"
        ]
    })
}

pub(crate) fn render_inline_model_assignment(
    raw: &str,
    event_arg: &str,
    binding_type: Option<&str>,
    options: &Vue3CompilerOptions,
    fallback_target: impl FnOnce() -> String,
) -> String {
    if !options.inline || !is_simple_identifier_ascii(raw) {
        let target = fallback_target();
        return format!("{event_arg} => (({target}) = $event)");
    }
    match binding_type {
        Some("setup-ref") => format!("{event_arg} => (({raw}).value = $event)"),
        Some("setup-maybe-ref") => {
            format!("{event_arg} => (_isRef({raw}) ? ({raw}).value = $event : null)")
        }
        Some("setup-let") => {
            format!("{event_arg} => (_isRef({raw}) ? ({raw}).value = $event : {raw} = $event)")
        }
        _ => {
            let target = fallback_target();
            format!("{event_arg} => (({target}) = $event)")
        }
    }
}

pub(crate) fn model_prop_name_projection(arg: Option<&Value>) -> Value {
    match arg {
        Some(_) => json!({ "kind": "node", "path": "dir.arg" }),
        None => json!({ "kind": "static", "content": "modelValue" }),
    }
}

pub(crate) fn model_event_name_projection(arg: Option<&Value>) -> Value {
    match arg {
        Some(arg) if json_bool(arg, "isStatic") => json!({
            "kind": "static",
            "content": format!("onUpdate:{}", camelize(json_str(arg, "content").unwrap_or(""))),
        }),
        Some(_) => json!({
            "kind": "compound",
            "children": [
                "\"onUpdate:\" + ",
                { "kind": "node", "path": "dir.arg" }
            ],
        }),
        None => json!({ "kind": "static", "content": "onUpdate:modelValue" }),
    }
}

pub(crate) fn model_update_needs_hydration_event(arg: Option<&Value>, node: &Value) -> bool {
    arg.is_some_and(|arg| json_bool(arg, "isStatic")) && json_u64(node, "tagType") != Some(1)
}

pub(crate) fn model_modifiers_key_projection(arg: Option<&Value>) -> Value {
    match arg {
        Some(arg) if json_bool(arg, "isStatic") => json!({
            "kind": "static",
            "content": format!("{}Modifiers", json_str(arg, "content").unwrap_or("")),
        }),
        Some(_) => json!({
            "kind": "compound",
            "children": [
                { "kind": "node", "path": "dir.arg" },
                " + \"Modifiers\""
            ],
        }),
        None => json!({ "kind": "static", "content": "modelModifiers" }),
    }
}

pub(crate) fn model_modifiers_expression(dir: &Value) -> Value {
    let modifiers = dir
        .get("modifiers")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|modifier| json_str(modifier, "content"))
        .map(|modifier| {
            if is_simple_identifier_ascii(modifier) {
                format!("{modifier}: true")
            } else {
                format!("{}: true", quote_string(modifier))
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    json!({
        "kind": "simple",
        "content": format!("{{ {modifiers} }}"),
        "isStatic": false,
        "constType": 2,
    })
}

pub(crate) fn should_cache_model_update(exp: &Value, context: &Value) -> bool {
    json_bool(context, "prefixIdentifiers")
        && json_bool(context, "cacheHandlers")
        && !json_bool(context, "inVOnce")
        && !model_has_scope_ref(exp, context)
}

pub(crate) fn model_has_scope_ref(exp: &Value, context: &Value) -> bool {
    let source = model_expression_source(exp);
    context
        .get("identifiers")
        .and_then(Value::as_object)
        .is_some_and(|identifiers| {
            identifiers.iter().any(|(name, count)| {
                count.as_i64().unwrap_or_default() > 0 && source.contains(name)
            })
        })
}

pub(crate) fn model_expression_source(exp: &Value) -> String {
    if let Some(content) = json_str(exp, "content") {
        return content.to_string();
    }
    if let Some(children) = exp.get("children").and_then(Value::as_array) {
        return children
            .iter()
            .map(model_expression_child_source)
            .collect::<String>();
    }
    exp.get("loc")
        .and_then(|loc| loc.get("source"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

pub(crate) fn model_expression_child_source(child: &Value) -> String {
    child
        .as_str()
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| model_expression_source(child))
}

pub(crate) fn transform_bind_key_projection(
    arg: Option<&Value>,
    dir: &Value,
    context: &Value,
) -> Value {
    let mut key = transform_bind_guarded_arg_projection(arg, dir);
    if directive_has_modifier(dir, "camel") {
        key = transform_bind_camel_projection(key);
    }
    if !json_bool(context, "inSSR") {
        if directive_has_modifier(dir, "prop") {
            key = transform_bind_prefix_projection(key, ".");
        }
        if directive_has_modifier(dir, "attr") {
            key = transform_bind_prefix_projection(key, "^");
        }
    }
    key
}

pub(crate) fn transform_bind_raw_arg_projection(arg: Option<&Value>, dir: &Value) -> Value {
    let loc = arg
        .and_then(|arg| arg.get("loc").cloned())
        .unwrap_or_else(|| dir.get("loc").cloned().unwrap_or(Value::Null));
    match arg {
        Some(_) => json!({ "kind": "node", "path": "dir.arg", "loc": loc }),
        None => json!({
            "kind": "simple",
            "content": "",
            "isStatic": true,
            "loc": loc,
        }),
    }
}
