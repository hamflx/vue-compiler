pub(crate) fn vue3_transform_element_suite_runtime_directive(dir: &Value) -> Value {
    let projection = vuec_vue3_core::build_directive_args_projection(&json!({
        "dir": dir,
        "needRuntime": Value::Null,
    }));
    let mut elements = Vec::new();
    elements.push(vue3_transform_element_suite_directive_runtime(
        projection.get("runtime").unwrap_or(&Value::Null),
        dir,
    ));
    let include_exp = projection
        .get("includeExp")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let include_arg = projection
        .get("includeArg")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if include_exp {
        elements.push(dir.get("exp").cloned().unwrap_or(Value::Null));
    } else if include_arg {
        elements.push(Value::String("undefined".to_string()));
    }
    if include_arg {
        elements.push(dir.get("arg").cloned().unwrap_or(Value::Null));
    }
    let modifiers = projection
        .get("modifiers")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|modifier| modifier.get("name").and_then(Value::as_str))
                .map(|name| {
                    vue3_once_suite_object_property(
                        vue3_once_suite_simple_expression(name, true),
                        vue3_once_suite_simple_expression("true", false),
                    )
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !modifiers.is_empty() {
        elements.push(vue3_if_suite_props_object(modifiers, dir));
    }
    json!({
        "type": 17,
        "elements": elements,
        "loc": dir.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
    })
}

pub(crate) fn vue3_transform_element_suite_directive_runtime(
    runtime: &Value,
    dir: &Value,
) -> Value {
    match runtime.get("kind").and_then(Value::as_str) {
        Some("helper") => runtime
            .get("helper")
            .and_then(Value::as_str)
            .or_else(|| runtime.get("helperName").and_then(Value::as_str))
            .map(|helper| Value::String(format!("_{}", vue3_bind_suite_helper_name(helper))))
            .unwrap_or(Value::Null),
        Some("asset") | _ => runtime
            .get("name")
            .and_then(Value::as_str)
            .or_else(|| dir.get("name").and_then(Value::as_str))
            .map(vue3_text_suite_directive_asset_id)
            .map(Value::String)
            .unwrap_or(Value::Null),
    }
}

pub(crate) fn vue3_transform_element_suite_apply_inline_template_refs(
    props: &mut Value,
    projection: &Value,
    node: &Value,
) {
    let refs = projection
        .get("inlineTemplateRefs")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if refs.is_empty() || props.is_null() {
        return;
    }
    for reference in refs.into_iter().rev() {
        let Some(content) = reference.get("content").and_then(Value::as_str) else {
            continue;
        };
        vue3_transform_element_suite_apply_inline_template_ref_value(props, content);
        let property = vue3_once_suite_object_property(
            vue3_once_suite_simple_expression("ref_key", true),
            vue3_once_suite_simple_expression(content, true),
        );
        *props = vue3_for_suite_prepend_props_expression_prop(
            std::mem::take(props),
            property,
            node.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
        );
    }
}

pub(crate) fn vue3_transform_element_suite_apply_inline_template_ref_value(
    props: &mut Value,
    content: &str,
) {
    let Some(properties) = props.get_mut("properties").and_then(Value::as_array_mut) else {
        return;
    };
    for property in properties {
        let Some(key) = property
            .get("key")
            .and_then(vue3_model_suite_static_prop_name)
        else {
            continue;
        };
        if key == "ref" {
            property["value"] = vue3_once_suite_simple_expression(content, false);
        }
    }
}

pub(crate) fn vue3_transform_element_suite_is_dynamic_component_is_prop(
    node: &Value,
    prop: &Value,
) -> bool {
    matches!(
        node.get("tag").and_then(Value::as_str),
        Some("component" | "Component")
    ) && vue3_transform_element_suite_static_arg(prop).as_deref() == Some("is")
}

pub(crate) fn vue3_transform_element_suite_static_arg(prop: &Value) -> Option<String> {
    prop.get("arg")
        .filter(|value| !value.is_null())
        .filter(|arg| {
            arg.get("isStatic")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .and_then(|arg| arg.get("content").and_then(Value::as_str))
        .map(ToOwned::to_owned)
}

pub(crate) fn vue3_transform_element_suite_has_modifier(prop: &Value, name: &str) -> bool {
    prop.get("modifiers")
        .and_then(Value::as_array)
        .is_some_and(|modifiers| {
            modifiers.iter().any(|modifier| {
                modifier
                    .as_str()
                    .or_else(|| modifier.get("content").and_then(Value::as_str))
                    == Some(name)
            })
        })
}

pub(crate) fn vue3_transform_element_suite_dedupe_properties(properties: Vec<Value>) -> Vec<Value> {
    let mut deduped = Vec::<Value>::new();
    for property in properties {
        let key_name = property
            .get("key")
            .and_then(vue3_model_suite_static_prop_name);
        let Some(key_name) = key_name else {
            deduped.push(property);
            continue;
        };
        let Some(existing) = deduped.iter_mut().find(|existing| {
            existing
                .get("key")
                .and_then(vue3_model_suite_static_prop_name)
                .as_deref()
                == Some(key_name.as_str())
        }) else {
            deduped.push(property);
            continue;
        };
        if key_name == "class" || key_name == "style" || key_name.starts_with("on") {
            let next_value = property.get("value").cloned().unwrap_or(Value::Null);
            vue3_transform_element_suite_merge_property_value(existing, next_value);
        } else {
            existing["value"] = property.get("value").cloned().unwrap_or(Value::Null);
        }
    }
    deduped
}

pub(crate) fn vue3_transform_element_suite_merge_property_value(
    property: &mut Value,
    next_value: Value,
) {
    let current = property.get("value").cloned().unwrap_or(Value::Null);
    if vue3_public_node_type(&current) == Some(17) {
        property["value"] = current;
        if let Some(elements) = property
            .get_mut("value")
            .and_then(|value| value.get_mut("elements"))
            .and_then(Value::as_array_mut)
        {
            elements.push(next_value);
        }
        return;
    }
    property["value"] = json!({
        "type": 17,
        "elements": [current, next_value],
        "loc": property.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
    });
}

pub(crate) fn vue3_transform_element_suite_cache_expression(
    state: &mut Vue3SlotSuiteState,
    value: Value,
    need_array_spread: bool,
) -> Value {
    let index = state.cached;
    state.cached += 1;
    json!({
        "type": 20,
        "index": index,
        "value": value,
        "needPauseTracking": false,
        "inVOnce": false,
        "needArraySpread": need_array_spread,
        "loc": vue3_loc_stub_value(),
    })
}

pub(crate) fn vue3_transform_element_suite_component_context(
    options: &Vue3CompilerOptions,
    state: &Vue3SlotSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> Value {
    let mut context = vue3_model_suite_transform_context(options, scope);
    context["selfName"] = state
        .transform_element_self_name
        .clone()
        .map(Value::String)
        .unwrap_or(Value::Null);
    if let Some(is_script_setup) = state.transform_element_is_script_setup {
        context["isScriptSetup"] = json!(is_script_setup);
    }
    context["compatIsOnElement"] = json!(false);
    context["builtInComponents"] = json!([]);
    context
}

pub(crate) fn vue3_transform_element_suite_component_tag(
    projection: &Value,
    node: &Value,
    state: &mut Vue3SlotSuiteState,
) -> Value {
    match projection.get("kind").and_then(Value::as_str) {
        Some("dynamic") => {
            vue3_transform_element_suite_register_projection_helpers(projection, state);
            let argument = vue3_slot_suite_projection_node(
                projection.get("argument").unwrap_or(&Value::Null),
                node,
            );
            vue3_text_suite_call("RESOLVE_DYNAMIC_COMPONENT", vec![argument])
        }
        Some("helper") => {
            vue3_transform_element_suite_register_projection_helpers(projection, state);
            projection
                .get("helper")
                .and_then(Value::as_str)
                .or_else(|| projection.get("helperName").and_then(Value::as_str))
                .map(|helper| Value::String(helper.to_string()))
                .unwrap_or(Value::Null)
        }
        Some("expression") => {
            vue3_transform_element_suite_register_projection_helpers(projection, state);
            projection
                .get("content")
                .and_then(Value::as_str)
                .map(|content| Value::String(content.to_string()))
                .unwrap_or(Value::Null)
        }
        Some("asset") => {
            if let Some(component) = projection.get("component").and_then(Value::as_str) {
                vue3_transform_element_suite_push_unique(
                    &mut state.transform_element_components,
                    component.to_string(),
                );
            }
            projection
                .get("assetId")
                .and_then(Value::as_str)
                .map(|asset| Value::String(asset.to_string()))
                .unwrap_or_else(|| {
                    Value::String(vue3_once_suite_component_asset_id(
                        node.get("tag").and_then(Value::as_str).unwrap_or(""),
                    ))
                })
        }
        _ => Value::String(vue3_once_suite_component_asset_id(
            node.get("tag").and_then(Value::as_str).unwrap_or(""),
        )),
    }
}

pub(crate) fn vue3_transform_element_suite_register_projection_helpers(
    projection: &Value,
    state: &mut Vue3SlotSuiteState,
) {
    if projection
        .get("registerHelper")
        .and_then(Value::as_bool)
        .unwrap_or(true)
    {
        if let Some(helper) = projection
            .get("helper")
            .and_then(Value::as_str)
            .or_else(|| projection.get("helperName").and_then(Value::as_str))
        {
            vue3_transform_element_suite_push_unique(
                &mut state.transform_element_helpers,
                helper.to_string(),
            );
        }
    }
    for helper in projection
        .get("helpers")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        vue3_transform_element_suite_push_unique(
            &mut state.transform_element_helpers,
            helper.to_string(),
        );
    }
}

pub(crate) fn vue3_transform_element_suite_should_build_component_slots(
    tag: &Value,
    children: &Value,
) -> bool {
    if children.is_null() {
        return false;
    }
    !vue3_transform_element_suite_helper_tag_name(tag)
        .is_some_and(|helper| matches!(helper, "TELEPORT" | "KEEP_ALIVE"))
}

pub(crate) fn vue3_transform_element_suite_helper_tag_name(tag: &Value) -> Option<&str> {
    tag.as_str()
}

pub(crate) fn vue3_transform_element_suite_element_children(node: &Value) -> Value {
    let children = node
        .get("children")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if children.is_empty() {
        Value::Null
    } else if children.len() == 1 && vue3_text_suite_direct_child_value(&children[0]) {
        children[0].clone()
    } else {
        Value::Array(children)
    }
}

pub(crate) fn vue3_transform_element_suite_push_unique(items: &mut Vec<String>, value: String) {
    if !items.iter().any(|item| item == &value) {
        items.push(value);
    }
}

pub(crate) fn vue3_transform_element_suite_finalize_root(
    root: &mut Value,
    state: &Vue3SlotSuiteState,
) {
    vue3_once_suite_set_root_codegen(root);
    root["components"] = json!(state.transform_element_components.clone());
    root["directives"] = json!(vue3_transform_element_suite_collect_directives(root, state));
    root["helpers"] = json!(vue3_transform_element_suite_helpers(root, state));
    root["hoists"] = json!([]);
    root["cached"] = Value::Array((0..state.cached).map(|_| Value::Null).collect());
    root["temps"] = json!(0);
}

pub(crate) fn vue3_transform_element_suite_collect_directives(
    root: &Value,
    state: &Vue3SlotSuiteState,
) -> Vec<String> {
    vue3_if_suite_collect_directives(root)
        .into_iter()
        .filter(|directive| {
            !state
                .transform_element_noop_directives
                .iter()
                .any(|noop| noop == directive)
        })
        .collect()
}

pub(crate) fn vue3_transform_element_suite_helpers(
    root: &Value,
    state: &Vue3SlotSuiteState,
) -> Vec<String> {
    let mut used = Vec::<&'static str>::new();
    vue3_slot_suite_collect_helpers(root, &mut used);
    vue3_slot_suite_collect_helpers(root.get("codegenNode").unwrap_or(&Value::Null), &mut used);
    for helper in &state.transform_element_helpers {
        if let Some(helper) = vue3_transform_element_suite_known_helper(helper) {
            vue3_text_suite_add_helper(&mut used, helper);
        }
    }
    if !state.transform_element_components.is_empty() {
        vue3_text_suite_add_helper(&mut used, "RESOLVE_COMPONENT");
    }
    if !vue3_transform_element_suite_collect_directives(root, state).is_empty() {
        vue3_text_suite_add_helper(&mut used, "RESOLVE_DIRECTIVE");
    }
    [
        "TO_DISPLAY_STRING",
        "CREATE_ELEMENT_VNODE",
        "CREATE_TEXT",
        "CREATE_COMMENT",
        "RESOLVE_COMPONENT",
        "RESOLVE_DYNAMIC_COMPONENT",
        "WITH_CTX",
        "RENDER_LIST",
        "CREATE_SLOTS",
        "CREATE_VNODE",
        "OPEN_BLOCK",
        "CREATE_BLOCK",
        "CREATE_ELEMENT_BLOCK",
        "FRAGMENT",
        "RENDER_SLOT",
        "MERGE_PROPS",
        "NORMALIZE_PROPS",
        "NORMALIZE_CLASS",
        "NORMALIZE_STYLE",
        "GUARD_REACTIVE_PROPS",
        "TO_HANDLERS",
        "TO_HANDLER_KEY",
        "SET_BLOCK_TRACKING",
        "RESOLVE_DIRECTIVE",
        "WITH_DIRECTIVES",
        "TELEPORT",
        "SUSPENSE",
        "KEEP_ALIVE",
        "BASE_TRANSITION",
        "UNREF",
    ]
    .into_iter()
    .filter(|helper| used.iter().any(|used| used == helper))
    .map(str::to_string)
    .collect()
}

pub(crate) fn vue3_transform_element_suite_known_helper(helper: &str) -> Option<&'static str> {
    match helper {
        "TO_DISPLAY_STRING" => Some("TO_DISPLAY_STRING"),
        "CREATE_ELEMENT_VNODE" => Some("CREATE_ELEMENT_VNODE"),
        "CREATE_TEXT" => Some("CREATE_TEXT"),
        "CREATE_COMMENT" => Some("CREATE_COMMENT"),
        "RESOLVE_COMPONENT" => Some("RESOLVE_COMPONENT"),
        "RESOLVE_DYNAMIC_COMPONENT" => Some("RESOLVE_DYNAMIC_COMPONENT"),
        "WITH_CTX" => Some("WITH_CTX"),
        "RENDER_LIST" => Some("RENDER_LIST"),
        "CREATE_SLOTS" => Some("CREATE_SLOTS"),
        "CREATE_VNODE" => Some("CREATE_VNODE"),
        "OPEN_BLOCK" => Some("OPEN_BLOCK"),
        "CREATE_BLOCK" => Some("CREATE_BLOCK"),
        "CREATE_ELEMENT_BLOCK" => Some("CREATE_ELEMENT_BLOCK"),
        "FRAGMENT" => Some("FRAGMENT"),
        "RENDER_SLOT" => Some("RENDER_SLOT"),
        "MERGE_PROPS" => Some("MERGE_PROPS"),
        "NORMALIZE_PROPS" => Some("NORMALIZE_PROPS"),
        "NORMALIZE_CLASS" => Some("NORMALIZE_CLASS"),
        "NORMALIZE_STYLE" => Some("NORMALIZE_STYLE"),
        "GUARD_REACTIVE_PROPS" => Some("GUARD_REACTIVE_PROPS"),
        "TO_HANDLERS" => Some("TO_HANDLERS"),
        "TO_HANDLER_KEY" => Some("TO_HANDLER_KEY"),
        "SET_BLOCK_TRACKING" => Some("SET_BLOCK_TRACKING"),
        "RESOLVE_DIRECTIVE" => Some("RESOLVE_DIRECTIVE"),
        "WITH_DIRECTIVES" => Some("WITH_DIRECTIVES"),
        "TELEPORT" => Some("TELEPORT"),
        "SUSPENSE" => Some("SUSPENSE"),
        "KEEP_ALIVE" => Some("KEEP_ALIVE"),
        "BASE_TRANSITION" => Some("BASE_TRANSITION"),
        "UNREF" => Some("UNREF"),
        _ => None,
    }
}
