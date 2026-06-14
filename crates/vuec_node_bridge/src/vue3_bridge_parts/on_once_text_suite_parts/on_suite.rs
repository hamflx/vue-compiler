pub(crate) fn vue3_core_transform_on_suite_value(payload: &Value) -> Value {
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
        let transformed = vue3_on_suite_transform_children(
            std::mem::take(children),
            &options,
            &mut state,
            &Vue3ModelSuiteScope::default(),
        );
        *children = transformed;
    }
    vue3_model_suite_finalize_root(&mut root, &state);
    root["__vuecErrors"] = json!(state.errors);

    let node = root
        .get("children")
        .and_then(Value::as_array)
        .and_then(|children| children.first())
        .cloned()
        .unwrap_or(Value::Null);
    json!({
        "root": root,
        "node": node,
    })
}

pub(crate) fn vue3_on_suite_transform_children(
    children: Vec<Value>,
    options: &Vue3CompilerOptions,
    state: &mut Vue3ModelSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> Vec<Value> {
    children
        .into_iter()
        .map(|child| vue3_on_suite_transform_node(child, options, state, scope))
        .collect()
}

pub(crate) fn vue3_on_suite_transform_node(
    mut node: Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3ModelSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> Value {
    if vue3_public_node_type(&node) == Some(1) && vue3_text_suite_directive(&node, "for").is_some()
    {
        return vue3_on_suite_transform_for_node(node, options, state, scope);
    }

    if vue3_public_node_type(&node) == Some(1) {
        vue3_on_suite_process_dynamic_args(&mut node, options, scope);
    }

    let once_projection = vue3_once_suite_once_projection(&node, scope.in_v_once);
    let enters_once = once_projection.get("kind").and_then(Value::as_str) == Some("enter");
    let mut child_scope = scope.clone();
    child_scope.in_v_once = child_scope.in_v_once || enters_once;
    vue3_model_suite_track_slot_scope(&node, &mut child_scope);

    if let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) {
        let transformed = vue3_on_suite_transform_children(
            std::mem::take(children),
            options,
            state,
            &child_scope,
        );
        *children = transformed;
    }

    if vue3_public_node_type(&node) == Some(1) {
        node["codegenNode"] = vue3_on_suite_element_codegen(&node, options, state, scope, false);
    }
    if enters_once {
        let codegen = node.get("codegenNode").cloned().unwrap_or(Value::Null);
        node["codegenNode"] = vue3_model_suite_cache_expression(state, codegen, true, true, false);
    }
    node
}

pub(crate) fn vue3_on_suite_transform_for_node(
    mut node: Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3ModelSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> Value {
    let Some(dir) = vue3_text_suite_directive(&node, "for").cloned() else {
        return vue3_on_suite_transform_node(node, options, state, scope);
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
            .map(|child| vue3_on_suite_transform_node(child, options, state, &child_scope))
            .collect::<Vec<_>>()
    } else {
        vue3_text_suite_remove_directive(&mut node, "for");
        vec![vue3_on_suite_transform_node(
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

pub(crate) fn vue3_on_suite_process_dynamic_args(
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
        if vue3_public_node_type(prop) != Some(7)
            || prop.get("name").and_then(Value::as_str) != Some("on")
        {
            continue;
        }
        let Some(current) = prop.get("arg").filter(|value| !value.is_null()).cloned() else {
            continue;
        };
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

pub(crate) fn vue3_on_suite_element_codegen(
    node: &Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3ModelSuiteState,
    scope: &Vue3ModelSuiteScope,
    is_block: bool,
) -> Value {
    let (props, patch_flag, dynamic_props) =
        vue3_on_suite_props_codegen(node, options, state, scope);
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

pub(crate) fn vue3_on_suite_props_codegen(
    node: &Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3ModelSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> (Value, Option<Value>, Value) {
    let mut properties = Vec::new();
    let mut dynamic_props = Vec::<String>::new();
    let mut has_dynamic_key = false;
    let context = vue3_model_suite_transform_context(options, scope);

    for prop in node
        .get("props")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if vue3_public_node_type(prop) != Some(7)
            || prop.get("name").and_then(Value::as_str) != Some("on")
        {
            continue;
        }
        let projection = vuec_vue3_core::transform_on_projection(&json!({
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
            let key = vue3_on_suite_materialize_projection(projected_prop.get("key"), prop);
            let mut value = vue3_on_suite_materialize_projection(projected_prop.get("value"), prop);
            let cached = projected_prop
                .get("cache")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if cached {
                value = vue3_model_suite_cache_expression(state, value, false, false, false);
            }
            let dynamic_key = projected_prop
                .get("dynamicKey")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            has_dynamic_key = has_dynamic_key || dynamic_key;
            if !cached && !dynamic_key {
                let value_constant = projected_prop
                    .get("valueConstant")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                if !value_constant {
                    if let Some(name) = vue3_model_suite_static_prop_name(&key) {
                        dynamic_props.push(name);
                    }
                }
            }
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

    let patch_flag = if has_dynamic_key {
        Some(json!(16))
    } else if !dynamic_props.is_empty() {
        Some(json!(8))
    } else {
        None
    };
    let dynamic_props = if dynamic_props.is_empty() {
        Value::Null
    } else {
        Value::String(vue3_model_suite_dynamic_props_string(&dynamic_props))
    };
    (object, patch_flag, dynamic_props)
}

pub(crate) fn vue3_on_suite_materialize_projection(
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
            Some("dir.arg.children") => dir
                .get("arg")
                .and_then(|arg| arg.get("children"))
                .cloned()
                .unwrap_or_else(|| json!([])),
            _ => Value::Null,
        },
        Some("children") => projection
            .get("children")
            .and_then(Value::as_array)
            .map(|children| {
                Value::Array(
                    children
                        .iter()
                        .flat_map(|child| {
                            let materialized =
                                vue3_on_suite_materialize_projection(Some(child), dir);
                            match materialized {
                                Value::Array(items) => items,
                                value => vec![value],
                            }
                        })
                        .collect(),
                )
            })
            .unwrap_or_else(|| json!([])),
        Some("helperString") => {
            let helper = projection
                .get("helper")
                .and_then(Value::as_str)
                .unwrap_or_default();
            Value::String(format!("_{}(", vue3_bind_suite_helper_name(helper)))
        }
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
                        .flat_map(|child| {
                            let materialized =
                                vue3_on_suite_materialize_projection(Some(child), dir);
                            match materialized {
                                Value::Array(items) => items,
                                value => vec![value],
                            }
                        })
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
                    .or_else(|| dir.get("arg").and_then(|arg| arg.get("loc")).cloned())
                    .or_else(|| dir.get("loc").cloned())
                    .unwrap_or_else(vue3_loc_stub_value),
            })
        }
        _ => Value::Null,
    }
}
