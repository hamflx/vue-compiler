pub(crate) fn vue3_core_transform_model_suite_value(payload: &Value) -> Value {
    let source = template_source(payload);
    let options = vue3_options(payload.get("options"));
    let ast = Vue3Dialect::base_parse(source.clone(), &options);
    let mut root = vue3_parse_value(
        &ast,
        &source.source,
        source.base_offset,
        false,
        &options,
        false,
    );
    let mut state = Vue3ModelSuiteState::default();
    if let Some(children) = root.get_mut("children").and_then(Value::as_array_mut) {
        let transformed = vue3_model_suite_transform_children(
            std::mem::take(children),
            &options,
            &mut state,
            &Vue3ModelSuiteScope::default(),
        );
        *children = transformed;
    }
    vue3_model_suite_finalize_root(&mut root, &state);
    root["__vuecErrors"] = json!(state.errors);
    root
}

pub(crate) fn vue3_model_suite_transform_children(
    children: Vec<Value>,
    options: &Vue3CompilerOptions,
    state: &mut Vue3ModelSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> Vec<Value> {
    children
        .into_iter()
        .map(|child| vue3_model_suite_transform_node(child, options, state, scope))
        .collect()
}

pub(crate) fn vue3_model_suite_transform_node(
    mut node: Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3ModelSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> Value {
    if vue3_public_node_type(&node) == Some(1) && vue3_text_suite_directive(&node, "for").is_some()
    {
        return vue3_model_suite_transform_for_node(node, options, state, scope);
    }

    if vue3_public_node_type(&node) == Some(1) {
        vue3_model_suite_process_directive_expressions(&mut node, options, scope);
    }

    let once_projection = vue3_once_suite_once_projection(&node, scope.in_v_once);
    let enters_once = once_projection.get("kind").and_then(Value::as_str) == Some("enter");
    let mut child_scope = scope.clone();
    child_scope.in_v_once = child_scope.in_v_once || enters_once;
    vue3_model_suite_track_slot_scope(&node, &mut child_scope);

    if let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) {
        let transformed = vue3_model_suite_transform_children(
            std::mem::take(children),
            options,
            state,
            &child_scope,
        );
        *children = transformed;
    }

    if vue3_public_node_type(&node) == Some(1) {
        node["codegenNode"] = vue3_model_suite_element_codegen(&node, options, state, scope, false);
    }
    if enters_once {
        let codegen = node.get("codegenNode").cloned().unwrap_or(Value::Null);
        node["codegenNode"] = vue3_model_suite_cache_expression(state, codegen, true, true, false);
    }
    node
}

pub(crate) fn vue3_model_suite_transform_for_node(
    mut node: Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3ModelSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> Value {
    let Some(dir) = vue3_text_suite_directive(&node, "for").cloned() else {
        return vue3_model_suite_transform_node(node, options, state, scope);
    };
    let context = vue3_model_suite_transform_context(options, scope);
    let projection = vuec_vue3_core::transform_for_projection(&json!({
        "node": node,
        "dir": dir,
        "context": context,
    }));
    for error in projection
        .get("errors")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        state.errors.push(vue3_model_suite_error_value(error, &dir));
    }
    let parse_result = projection
        .get("parseResult")
        .filter(|value| !value.is_null())
        .map(vue3_text_suite_materialize_for_parse_result)
        .unwrap_or_else(|| {
            dir.get("forParseResult")
                .map(vue3_text_suite_materialize_for_parse_result)
                .unwrap_or(Value::Null)
        });

    let once_projection = vue3_once_suite_once_projection(&node, scope.in_v_once);
    let enters_once = once_projection.get("kind").and_then(Value::as_str) == Some("enter");
    let mut child_scope = scope.clone();
    child_scope.in_v_once = child_scope.in_v_once || enters_once;
    vue3_model_suite_add_locals(&mut child_scope, projection.get("locals"));
    child_scope.v_for_depth += 1;

    let fallback_loc = node.get("loc").cloned();
    let children = if projection.get("children").and_then(Value::as_str) == Some("template") {
        node.get_mut("children")
            .and_then(Value::as_array_mut)
            .map(std::mem::take)
            .unwrap_or_default()
            .into_iter()
            .map(|child| vue3_model_suite_transform_node(child, options, state, &child_scope))
            .collect::<Vec<_>>()
    } else {
        vue3_text_suite_remove_directive(&mut node, "for");
        vec![vue3_model_suite_transform_node(
            node,
            options,
            state,
            &child_scope,
        )]
    };

    let loc = dir
        .get("loc")
        .cloned()
        .or(fallback_loc)
        .unwrap_or_else(vue3_loc_stub_value);
    let mut for_node = json!({
        "type": 11,
        "source": parse_result.get("source").cloned().unwrap_or(Value::Null),
        "valueAlias": parse_result.get("value").cloned().unwrap_or(Value::Null),
        "keyAlias": parse_result.get("key").cloned().unwrap_or(Value::Null),
        "objectIndexAlias": parse_result.get("index").cloned().unwrap_or(Value::Null),
        "parseResult": parse_result,
        "children": children,
        "codegenNode": Value::Null,
        "loc": loc,
    });
    for_node["codegenNode"] = vue3_text_suite_for_codegen(&for_node);
    if enters_once {
        let codegen = for_node.get("codegenNode").cloned().unwrap_or(Value::Null);
        for_node["codegenNode"] =
            vue3_model_suite_cache_expression(state, codegen, true, true, false);
    }
    for_node
}

pub(crate) fn vue3_model_suite_process_directive_expressions(
    node: &mut Value,
    options: &Vue3CompilerOptions,
    scope: &Vue3ModelSuiteScope,
) {
    if !options.prefix_identifiers {
        return;
    }
    let context = vue3_model_suite_transform_context(options, scope);
    let Some(props) = node.get_mut("props").and_then(Value::as_array_mut) else {
        return;
    };
    for prop in props {
        if vue3_public_node_type(prop) != Some(7) {
            continue;
        }
        if let Some(current) = prop.get("exp").filter(|value| !value.is_null()).cloned() {
            if vue3_public_node_type(&current) == Some(4) {
                let projection = vuec_vue3_core::process_expression_projection(&json!({
                    "node": current,
                    "context": context,
                }));
                prop["exp"] = vue3_text_suite_materialize_process_projection(&projection, &current);
            }
        }
        if let Some(current) = prop.get("arg").filter(|value| !value.is_null()).cloned() {
            if vue3_public_node_type(&current) == Some(4)
                && !current
                    .get("isStatic")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            {
                let projection = vuec_vue3_core::process_expression_projection(&json!({
                    "node": current,
                    "context": context,
                }));
                prop["arg"] = vue3_text_suite_materialize_process_projection(&projection, &current);
            }
        }
    }
}

pub(crate) fn vue3_model_suite_track_slot_scope(node: &Value, scope: &mut Vue3ModelSuiteScope) {
    if vue3_public_node_type(node) != Some(1) {
        return;
    }
    let projection = vuec_vue3_core::track_slot_scopes_projection(&json!({ "node": node }));
    if projection
        .get("track")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        vue3_model_suite_add_locals(scope, projection.get("locals"));
    }
}

pub(crate) fn vue3_model_suite_add_locals(scope: &mut Vue3ModelSuiteScope, locals: Option<&Value>) {
    for local in locals
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        *scope.identifiers.entry(local.to_string()).or_insert(0) += 1;
    }
}

pub(crate) fn vue3_model_suite_element_codegen(
    node: &Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3ModelSuiteState,
    scope: &Vue3ModelSuiteScope,
    is_block: bool,
) -> Value {
    let (props, patch_flag, dynamic_props) =
        vue3_model_suite_props_codegen(node, options, state, scope);
    match (
        vue3_public_node_type(node),
        node.get("tagType").and_then(Value::as_u64),
    ) {
        (Some(1), Some(1)) => {
            let tag = node.get("tag").and_then(Value::as_str).unwrap_or("");
            vue3_once_suite_vnode_call(
                &vue3_once_suite_component_asset_id(tag),
                props,
                Value::Null,
                patch_flag,
                dynamic_props,
                is_block,
                false,
                true,
            )
        }
        (Some(1), Some(0)) => {
            let children = node
                .get("children")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let children = if children.is_empty() {
                Value::Null
            } else if children.len() == 1 && vue3_text_suite_direct_child_value(&children[0]) {
                children[0].clone()
            } else {
                Value::Array(children)
            };
            vue3_once_suite_vnode_call(
                &vue3_once_suite_quote_string(
                    node.get("tag").and_then(Value::as_str).unwrap_or(""),
                ),
                props,
                children,
                patch_flag,
                dynamic_props,
                is_block,
                false,
                false,
            )
        }
        _ => Value::Null,
    }
}

pub(crate) fn vue3_model_suite_props_codegen(
    node: &Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3ModelSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> (Value, Option<Value>, Value) {
    let mut properties = Vec::new();
    let mut dynamic_props = Vec::<String>::new();
    let mut needs_hydration = false;
    let context = vue3_model_suite_transform_context(options, scope);

    for prop in node
        .get("props")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if vue3_public_node_type(prop) != Some(7)
            || prop.get("name").and_then(Value::as_str) != Some("model")
        {
            continue;
        }
        let projection = vuec_vue3_core::transform_model_projection(&json!({
            "dir": prop,
            "node": node,
            "context": context,
        }));
        for error in projection
            .get("errors")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            state.errors.push(vue3_model_suite_error_value(error, prop));
        }
        for projected_prop in projection
            .get("props")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let key = vue3_model_suite_materialize_projection(projected_prop.get("key"), prop);
            let mut value =
                vue3_model_suite_materialize_projection(projected_prop.get("value"), prop);
            if projected_prop
                .get("cache")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                value = vue3_model_suite_cache_expression(state, value, false, false, false);
            }
            if projected_prop
                .get("dynamic")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                if let Some(name) = vue3_model_suite_static_prop_name(&key) {
                    dynamic_props.push(name);
                }
            }
            needs_hydration = needs_hydration
                || projected_prop
                    .get("hydrate")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
            properties.push(vue3_once_suite_object_property(key, value));
        }
    }

    let object = if properties.is_empty() {
        Value::Null
    } else {
        json!({
            "type": 15,
            "properties": properties,
            "loc": node.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
        })
    };
    if object.is_null() {
        return (Value::Null, None, Value::Null);
    }
    if vue3_bind_suite_has_dynamic_key(&object) {
        return (
            vue3_text_suite_call("NORMALIZE_PROPS", vec![object]),
            Some(json!(16)),
            Value::Null,
        );
    }

    let patch_flag = (!dynamic_props.is_empty() || needs_hydration).then(|| {
        let mut flag = 0;
        if !dynamic_props.is_empty() {
            flag |= 8;
        }
        if needs_hydration {
            flag |= 32;
        }
        json!(flag)
    });
    let dynamic_props = if dynamic_props.is_empty() {
        Value::Null
    } else {
        Value::String(vue3_model_suite_dynamic_props_string(&dynamic_props))
    };
    (object, patch_flag, dynamic_props)
}

pub(crate) fn vue3_model_suite_materialize_projection(
    projection: Option<&Value>,
    dir: &Value,
) -> Value {
    let Some(projection) = projection else {
        return Value::Null;
    };
    if projection.is_string() || projection.get("type").is_some() {
        return projection.clone();
    }
    match projection.get("kind").and_then(Value::as_str) {
        Some("undefined") => Value::Null,
        Some("node") => match projection.get("path").and_then(Value::as_str) {
            Some("dir.arg") => dir.get("arg").cloned().unwrap_or(Value::Null),
            Some("dir.exp") => dir.get("exp").cloned().unwrap_or(Value::Null),
            _ => Value::Null,
        },
        Some("static") | Some("simple") => json!({
            "type": 4,
            "content": projection.get("content").and_then(Value::as_str).unwrap_or(""),
            "isStatic": projection
                .get("isStatic")
                .and_then(Value::as_bool)
                .unwrap_or_else(|| projection.get("kind").and_then(Value::as_str) == Some("static")),
            "constType": projection.get("constType").and_then(Value::as_u64).unwrap_or_else(|| {
                if projection.get("kind").and_then(Value::as_str) == Some("static") { 3 } else { 0 }
            }),
            "loc": projection
                .get("loc")
                .cloned()
                .or_else(|| dir.get("exp").and_then(|exp| exp.get("loc")).cloned())
                .or_else(|| dir.get("arg").and_then(|arg| arg.get("loc")).cloned())
                .or_else(|| dir.get("loc").cloned())
                .unwrap_or_else(vue3_loc_stub_value),
        }),
        Some("compound") => {
            let children = projection
                .get("children")
                .and_then(Value::as_array)
                .map(|children| {
                    children
                        .iter()
                        .map(|child| vue3_model_suite_materialize_projection(Some(child), dir))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            json!({
                "type": 8,
                "children": children,
                "loc": projection
                    .get("loc")
                    .cloned()
                    .or_else(|| dir.get("exp").and_then(|exp| exp.get("loc")).cloned())
                    .or_else(|| dir.get("loc").cloned())
                    .unwrap_or_else(vue3_loc_stub_value),
            })
        }
        _ => Value::Null,
    }
}

pub(crate) fn vue3_model_suite_cache_expression(
    state: &mut Vue3ModelSuiteState,
    value: Value,
    need_pause_tracking: bool,
    in_v_once: bool,
    need_array_spread: bool,
) -> Value {
    let index = state.cached;
    state.cached += 1;
    json!({
        "type": 20,
        "index": index,
        "value": value,
        "needPauseTracking": need_pause_tracking,
        "inVOnce": in_v_once,
        "needArraySpread": need_array_spread,
        "loc": vue3_loc_stub_value(),
    })
}

pub(crate) fn vue3_model_suite_static_prop_name(key: &Value) -> Option<String> {
    (vue3_public_node_type(key) == Some(4)
        && key
            .get("isStatic")
            .and_then(Value::as_bool)
            .unwrap_or(false))
    .then(|| key.get("content").and_then(Value::as_str).unwrap_or(""))
    .filter(|name| !name.is_empty())
    .map(ToOwned::to_owned)
}

pub(crate) fn vue3_model_suite_dynamic_props_string(props: &[String]) -> String {
    let values = props
        .iter()
        .map(|prop| serde_json::to_string(prop).unwrap_or_else(|_| "\"\"".to_string()))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{values}]")
}

pub(crate) fn vue3_model_suite_finalize_root(root: &mut Value, state: &Vue3ModelSuiteState) {
    vue3_once_suite_set_root_codegen(root);
    root["components"] = json!(vue3_once_suite_components(root));
    root["helpers"] = json!(vue3_model_suite_helpers(root));
    root["directives"] = json!([]);
    root["hoists"] = json!([]);
    root["cached"] = Value::Array((0..state.cached).map(|_| Value::Null).collect());
    root["temps"] = json!(0);
}

pub(crate) fn vue3_model_suite_helpers(root: &Value) -> Vec<String> {
    let mut used = Vec::new();
    vue3_model_suite_collect_helpers(root.get("codegenNode").unwrap_or(&Value::Null), &mut used);
    if root
        .get("components")
        .and_then(Value::as_array)
        .is_some_and(|components| !components.is_empty())
    {
        vue3_text_suite_add_helper(&mut used, "RESOLVE_COMPONENT");
    }
    [
        "NORMALIZE_PROPS",
        "SET_BLOCK_TRACKING",
        "RESOLVE_COMPONENT",
        "CREATE_BLOCK",
        "CREATE_VNODE",
        "CREATE_ELEMENT_VNODE",
        "RENDER_LIST",
        "FRAGMENT",
        "OPEN_BLOCK",
        "CREATE_ELEMENT_BLOCK",
        "IS_REF",
        "TO_HANDLER_KEY",
    ]
    .into_iter()
    .filter(|helper| used.iter().any(|used| used == helper))
    .map(str::to_string)
    .collect()
}

pub(crate) fn vue3_model_suite_collect_helpers(node: &Value, used: &mut Vec<&'static str>) {
    if node.as_str() == Some("_toHandlerKey(") {
        vue3_text_suite_add_helper(used, "TO_HANDLER_KEY");
        return;
    }
    match vue3_public_node_type(node) {
        Some(13) => {
            if node.get("tag").and_then(Value::as_str) == Some("FRAGMENT") {
                vue3_text_suite_add_helper(used, "FRAGMENT");
            }
            if node
                .get("isBlock")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                vue3_text_suite_add_helper(used, "OPEN_BLOCK");
                if node
                    .get("isComponent")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                {
                    vue3_text_suite_add_helper(used, "CREATE_BLOCK");
                } else {
                    vue3_text_suite_add_helper(used, "CREATE_ELEMENT_BLOCK");
                }
            } else if node
                .get("isComponent")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                vue3_text_suite_add_helper(used, "CREATE_VNODE");
            } else {
                vue3_text_suite_add_helper(used, "CREATE_ELEMENT_VNODE");
            }
        }
        Some(14) => match node.get("callee").and_then(Value::as_str) {
            Some("NORMALIZE_PROPS") => vue3_text_suite_add_helper(used, "NORMALIZE_PROPS"),
            Some("RENDER_LIST") => vue3_text_suite_add_helper(used, "RENDER_LIST"),
            _ => {}
        },
        Some(20)
            if node
                .get("needPauseTracking")
                .and_then(Value::as_bool)
                .unwrap_or(false) =>
        {
            vue3_text_suite_add_helper(used, "SET_BLOCK_TRACKING");
        }
        _ => {}
    }
    for key in [
        "children",
        "props",
        "content",
        "codegenNode",
        "arguments",
        "returns",
        "params",
        "directives",
        "source",
        "valueAlias",
        "keyAlias",
        "objectIndexAlias",
        "parseResult",
        "branches",
        "condition",
        "test",
        "consequent",
        "alternate",
        "value",
        "elements",
        "properties",
        "key",
    ] {
        let Some(value) = node.get(key) else {
            continue;
        };
        if let Some(items) = value.as_array() {
            for item in items {
                vue3_model_suite_collect_helpers(item, used);
            }
        } else if value.is_object() {
            vue3_model_suite_collect_helpers(value, used);
        }
    }
}

pub(crate) fn vue3_model_suite_error_value(error: &Value, dir: &Value) -> Value {
    let code = error
        .as_u64()
        .or_else(|| error.get("code").and_then(Value::as_u64))
        .unwrap_or(0);
    let loc = match error.get("loc").and_then(Value::as_str) {
        Some("arg") => dir
            .get("arg")
            .and_then(|arg| arg.get("loc"))
            .cloned()
            .or_else(|| dir.get("loc").cloned())
            .unwrap_or_else(vue3_loc_stub_value),
        _ => dir.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
    };
    json!({ "code": code, "loc": loc })
}

pub(crate) fn vue3_model_suite_transform_context(
    options: &Vue3CompilerOptions,
    scope: &Vue3ModelSuiteScope,
) -> Value {
    let mut context = vue3_text_suite_transform_context(options);
    context["cacheHandlers"] = json!(options.cache_handlers);
    context["inVOnce"] = json!(scope.in_v_once);
    context["vForDepth"] = json!(scope.v_for_depth);
    context["vSlotDepth"] = json!(scope.v_slot_depth);
    context["identifiers"] = Value::Object(
        scope
            .identifiers
            .iter()
            .map(|(name, count)| (name.clone(), json!(count)))
            .collect(),
    );
    context
}
