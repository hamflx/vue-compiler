pub(crate) fn vue3_slot_suite_element_codegen(
    node: &Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3SlotSuiteState,
    scope: &Vue3ModelSuiteScope,
    is_block: bool,
) -> Value {
    if state.transform_element_suite {
        return vue3_transform_element_suite_element_codegen(node, options, state, scope, is_block);
    }
    if let Some(slot) = vue3_text_suite_directive(node, "slot") {
        if node.get("tagType").and_then(Value::as_u64) == Some(0) {
            state.errors.push(json!({
                "code": 40,
                "loc": slot.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
            }));
        }
    }
    match (
        vue3_public_node_type(node),
        node.get("tagType").and_then(Value::as_u64),
    ) {
        (Some(1), Some(1)) => {
            vue3_slot_suite_component_codegen(node, options, state, scope, is_block)
        }
        (Some(1), Some(2)) => vue3_slot_suite_slot_outlet_codegen(node, options, state, scope),
        (Some(1), Some(0)) => {
            let mut if_state = Vue3IfSuiteState {
                cached: state.cached,
                ..Default::default()
            };
            let codegen =
                vue3_if_suite_element_codegen(node, options, &mut if_state, scope, is_block);
            state.errors.extend(if_state.errors);
            state.cached = if_state.cached;
            codegen
        }
        _ => Value::Null,
    }
}

pub(crate) fn vue3_slot_suite_component_codegen(
    node: &Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3SlotSuiteState,
    scope: &Vue3ModelSuiteScope,
    is_block: bool,
) -> Value {
    let mut if_state = Vue3IfSuiteState {
        cached: state.cached,
        ..Default::default()
    };
    let (props, mut patch_flag, dynamic_props, directives, should_use_block) =
        vue3_if_suite_props_codegen(node, options, &mut if_state, scope);
    state.errors.extend(if_state.errors);
    state.cached = if_state.cached;

    let children = node
        .get("children")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let slot_children = if children.is_empty() {
        Value::Null
    } else {
        vue3_slot_suite_build_slots(node, options, state, scope)
    };
    if !slot_children.is_null() {
        let projection = vuec_vue3_core::build_slots_projection(&json!({
            "node": node,
            "context": vue3_model_suite_transform_context(options, scope),
        }));
        if projection
            .get("hasDynamicSlots")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            let current = patch_flag.and_then(|flag| flag.as_u64()).unwrap_or(0);
            patch_flag = Some(json!(current | 1024));
        }
    }

    let tag = node.get("tag").and_then(Value::as_str).unwrap_or("");
    let mut vnode = vue3_once_suite_vnode_call(
        &vue3_once_suite_component_asset_id(tag),
        props,
        slot_children,
        patch_flag,
        dynamic_props,
        is_block || should_use_block,
        false,
        true,
    );
    vnode["directives"] = directives;
    vnode
}

pub(crate) fn vue3_transform_element_suite_element_codegen(
    node: &Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3SlotSuiteState,
    scope: &Vue3ModelSuiteScope,
    is_block: bool,
) -> Value {
    if let Some(slot) = vue3_text_suite_directive(node, "slot") {
        if node.get("tagType").and_then(Value::as_u64) == Some(0) {
            state.errors.push(json!({
                "code": 40,
                "loc": slot.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
            }));
        }
    }
    match (
        vue3_public_node_type(node),
        node.get("tagType").and_then(Value::as_u64),
    ) {
        (Some(1), Some(1)) => {
            vue3_transform_element_suite_component_codegen(node, options, state, scope, is_block)
        }
        (Some(1), Some(2)) => vue3_slot_suite_slot_outlet_codegen(node, options, state, scope),
        (Some(1), Some(0)) => vue3_transform_element_suite_plain_element_codegen(
            node, options, state, scope, is_block,
        ),
        _ => Value::Null,
    }
}

pub(crate) fn vue3_transform_element_suite_plain_element_codegen(
    node: &Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3SlotSuiteState,
    scope: &Vue3ModelSuiteScope,
    is_block: bool,
) -> Value {
    let tag = vue3_once_suite_quote_string(node.get("tag").and_then(Value::as_str).unwrap_or(""));
    let (props, mut patch_flag, dynamic_props, directives, should_use_block) =
        vue3_transform_element_suite_props_codegen(node, options, state, scope, false, false);
    let children = vue3_transform_element_suite_element_children(node);
    if patch_flag.is_none() && vue3_suite_child_needs_text_patch_flag(&children, options, scope) {
        patch_flag = Some(json!(1));
    }
    let mut is_block = is_block || should_use_block;
    if !is_block {
        if let Some(tag) = node.get("tag").and_then(Value::as_str) {
            is_block = matches!(tag, "svg" | "foreignObject" | "math");
        }
    }
    if patch_flag.is_none() && !directives.is_null() && !is_block {
        patch_flag = Some(json!(512));
    }
    let mut vnode = vue3_once_suite_vnode_call(
        &tag,
        props,
        children,
        patch_flag,
        dynamic_props,
        is_block,
        false,
        false,
    );
    vnode["directives"] = directives;
    vnode
}

pub(crate) fn vue3_transform_element_suite_component_codegen(
    node: &Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3SlotSuiteState,
    scope: &Vue3ModelSuiteScope,
    is_block: bool,
) -> Value {
    let context = vue3_transform_element_suite_component_context(options, state, scope);
    let component = vuec_vue3_core::resolve_component_type_projection(&json!({
        "node": node,
        "context": context,
        "ssr": false,
    }));
    let tag = vue3_transform_element_suite_component_tag(&component, node, state);
    let is_dynamic_component =
        tag.get("callee").and_then(Value::as_str) == Some("RESOLVE_DYNAMIC_COMPONENT");
    let (props, mut patch_flag, dynamic_props, directives, should_use_block) =
        vue3_transform_element_suite_props_codegen(
            node,
            options,
            state,
            scope,
            true,
            is_dynamic_component,
        );

    let mut children = vue3_transform_element_suite_element_children(node);
    if vue3_transform_element_suite_should_build_component_slots(&tag, &children) {
        children = vue3_slot_suite_build_slots(node, options, state, scope);
        if !children.is_null() {
            let projection = vuec_vue3_core::build_slots_projection(&json!({
                "node": node,
                "context": vue3_model_suite_transform_context(options, scope),
            }));
            if projection
                .get("hasDynamicSlots")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                let current = patch_flag.as_ref().and_then(Value::as_u64).unwrap_or(0);
                patch_flag = Some(json!(current | 1024));
            }
        }
    } else if let Some(tag_name) = vue3_transform_element_suite_helper_tag_name(&tag) {
        let projection = vuec_vue3_core::transform_element_children_projection(&json!({
            "tag": tag_name,
            "children": node.get("children").cloned().unwrap_or_else(|| json!([])),
        }));
        if projection.get("kind").and_then(Value::as_str) == Some("children") {
            if projection
                .get("shouldUseBlock")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                children = vue3_transform_element_suite_element_children(node);
            }
            if let Some(projected) = projection.get("patchFlag").and_then(Value::as_u64) {
                let current = patch_flag.as_ref().and_then(Value::as_u64).unwrap_or(0);
                patch_flag = Some(json!(current | projected));
            }
        }
    }

    let is_block = is_block
        || should_use_block
        || is_dynamic_component
        || vue3_transform_element_suite_helper_tag_name(&tag)
            .is_some_and(|helper| matches!(helper, "TELEPORT" | "SUSPENSE" | "KEEP_ALIVE"));
    if patch_flag.is_none() && !directives.is_null() && !is_block {
        patch_flag = Some(json!(512));
    }
    let mut vnode = vue3_once_suite_vnode_call(
        "",
        props,
        children,
        patch_flag,
        dynamic_props,
        is_block,
        false,
        true,
    );
    vnode["tag"] = tag;
    vnode["directives"] = directives;
    vnode
}

pub(crate) fn vue3_transform_element_suite_props_codegen(
    node: &Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3SlotSuiteState,
    scope: &Vue3ModelSuiteScope,
    is_component: bool,
    is_dynamic_component: bool,
) -> (Value, Option<Value>, Value, Value, bool) {
    let mut properties = Vec::<Value>::new();
    let mut merge_args = Vec::<Value>::new();
    let mut runtime_directives = Vec::<Value>::new();
    let mut prop_summaries = Vec::<Value>::new();
    let context = vue3_model_suite_transform_context(options, scope);

    for prop in node
        .get("props")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        match vue3_public_node_type(prop) {
            Some(6) => {
                let Some(name) = prop.get("name").and_then(Value::as_str) else {
                    continue;
                };
                let value = prop
                    .get("value")
                    .and_then(|value| value.get("content"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                if name == "is"
                    && (matches!(
                        node.get("tag").and_then(Value::as_str),
                        Some("component" | "Component")
                    ) || value.starts_with("vue:"))
                {
                    continue;
                }
                properties.push(vue3_once_suite_object_property(
                    vue3_once_suite_simple_expression(name, true),
                    vue3_once_suite_simple_expression(value, true),
                ));
                prop_summaries.push(json!({
                    "kind": "attribute",
                    "name": name,
                    "value": value,
                }));
            }
            Some(7) if prop.get("name").and_then(Value::as_str) == Some("bind") => {
                if prop.get("arg").is_none_or(Value::is_null) {
                    prop_summaries.push(json!({ "kind": "objectBind" }));
                    vue3_if_suite_push_props_object_arg(&mut merge_args, &mut properties, node);
                    if let Some(exp) = prop.get("exp").filter(|value| !value.is_null()) {
                        merge_args.push(exp.clone());
                    } else {
                        state
                            .errors
                            .push(vue3_bind_suite_error_value(&json!(34), prop));
                    }
                    continue;
                }

                if vue3_transform_element_suite_is_dynamic_component_is_prop(node, prop) {
                    continue;
                }

                if !state.transform_element_bind {
                    if vue3_transform_element_suite_static_arg(prop).as_deref() == Some("key") {
                        prop_summaries.push(json!({
                            "kind": "directiveProp",
                            "name": "key",
                            "dynamicKey": false,
                            "valueConstant": true,
                            "forceBlock": true,
                        }));
                    }
                    continue;
                }

                let projection = vuec_vue3_core::transform_bind_projection(&json!({
                    "dir": prop,
                    "context": vue3_bind_suite_transform_context(false),
                }));
                for error in projection
                    .get("errors")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    state.errors.push(vue3_bind_suite_error_value(error, prop));
                }
                for projected_prop in projection
                    .get("props")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    let key =
                        vue3_bind_suite_materialize_projection(projected_prop.get("key"), prop);
                    let value =
                        vue3_bind_suite_materialize_projection(projected_prop.get("value"), prop);
                    let value_constant = vue3_if_suite_value_constant(&value, &context) > 0;
                    let mut summary = json!({
                        "kind": "directiveProp",
                        "dynamicKey": vue3_model_suite_static_prop_name(&key).is_none(),
                        "valueConstant": value_constant,
                        "valueStatic": value
                            .get("isStatic")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                        "valueType": value.get("type").cloned().unwrap_or(Value::Null),
                        "valueStartsWithArray": value
                            .get("content")
                            .and_then(Value::as_str)
                            .is_some_and(|content| content.trim_start().starts_with('[')),
                    });
                    if let Some(name) = vue3_model_suite_static_prop_name(&key) {
                        summary["name"] = json!(name);
                        if name == "key" {
                            summary["forceBlock"] = json!(true);
                        }
                    }
                    if vue3_transform_element_suite_has_modifier(prop, "prop") {
                        summary["propModifier"] = json!(true);
                    }
                    prop_summaries.push(summary);
                    properties.push(vue3_once_suite_object_property(key, value));
                }
            }
            Some(7) if prop.get("name").and_then(Value::as_str) == Some("on") => {
                if prop.get("arg").is_none_or(Value::is_null) {
                    prop_summaries.push(json!({ "kind": "objectOn" }));
                    vue3_if_suite_push_props_object_arg(&mut merge_args, &mut properties, node);
                    if let Some(exp) = prop.get("exp").filter(|value| !value.is_null()) {
                        let mut args = vec![exp.clone()];
                        if !is_component {
                            args.push(json!("true"));
                        }
                        merge_args.push(vue3_text_suite_call("TO_HANDLERS", args));
                    } else {
                        state
                            .errors
                            .push(vue3_model_suite_error_value(&json!(35), prop));
                    }
                    continue;
                }

                let force_before_update_block = node
                    .get("children")
                    .and_then(Value::as_array)
                    .is_some_and(|children| !children.is_empty())
                    && vue3_transform_element_suite_static_arg(prop).as_deref()
                        == Some("vue:before-update");
                if !state.transform_element_on {
                    if force_before_update_block {
                        prop_summaries.push(json!({
                            "kind": "directiveProp",
                            "forceBlock": true,
                        }));
                    }
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
                    let mut value =
                        vue3_on_suite_materialize_projection(projected_prop.get("value"), prop);
                    let cached = projected_prop
                        .get("cache")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    if cached {
                        value = vue3_transform_element_suite_cache_expression(state, value, false);
                    }
                    let dynamic_key = projected_prop
                        .get("dynamicKey")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    let mut summary = json!({
                        "kind": "directiveProp",
                        "dynamicKey": dynamic_key,
                        "ignoreDynamicKeyForNormalize": projected_prop
                            .get("ignoreDynamicKeyForNormalize")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                        "valueConstant": projected_prop
                            .get("valueConstant")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                        "valueCached": cached,
                        "forceBlock": force_before_update_block,
                    });
                    if !dynamic_key {
                        if let Some(name) = vue3_model_suite_static_prop_name(&key) {
                            summary["name"] = json!(name);
                        }
                    }
                    prop_summaries.push(summary);
                    properties.push(vue3_once_suite_object_property(key, value));
                }
            }
            Some(7) => {
                let Some(name) = prop.get("name").and_then(Value::as_str) else {
                    continue;
                };
                if matches!(name, "once" | "memo") {
                    continue;
                }
                if name == "slot" {
                    continue;
                }
                if state
                    .transform_element_noop_directives
                    .iter()
                    .any(|directive| directive == name)
                {
                    continue;
                }
                if !vue3_text_suite_builtin_directive(name) {
                    prop_summaries.push(json!({ "kind": "runtimeDirective" }));
                    runtime_directives.push(vue3_transform_element_suite_runtime_directive(prop));
                }
            }
            _ => {}
        }
    }

    let props_projection = vuec_vue3_core::transform_element_props_projection(&json!({
        "props": prop_summaries,
        "hasChildren": node
            .get("children")
            .and_then(Value::as_array)
            .is_some_and(|children| !children.is_empty()),
        "isComponent": is_component,
        "isDynamicComponent": is_dynamic_component,
        "context": context,
    }));

    let mut props = if merge_args.is_empty() {
        if properties.is_empty() {
            Value::Null
        } else {
            vue3_if_suite_props_object(
                vue3_transform_element_suite_dedupe_properties(properties),
                node,
            )
        }
    } else {
        vue3_if_suite_push_props_object_arg(&mut merge_args, &mut properties, node);
        if merge_args.len() == 1 {
            merge_args.pop().unwrap_or(Value::Null)
        } else {
            vue3_text_suite_call("MERGE_PROPS", merge_args)
        }
    };

    if props_projection
        .get("refForMarker")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        props = vue3_for_suite_prepend_props_expression_prop(
            props,
            vue3_for_suite_ref_for_property(),
            node.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
        );
    }
    vue3_transform_element_suite_apply_inline_template_refs(&mut props, &props_projection, node);
    vue3_if_suite_apply_props_normalizers(&mut props, &props_projection);

    let patch_flag = props_projection
        .get("patchFlag")
        .and_then(Value::as_u64)
        .filter(|flag| *flag > 0)
        .map(|flag| json!(flag));
    let dynamic_prop_names = props_projection
        .get("dynamicPropNames")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let dynamic_props = if dynamic_prop_names.is_empty() {
        Value::Null
    } else {
        Value::String(vue3_model_suite_dynamic_props_string(&dynamic_prop_names))
    };
    let directives = if runtime_directives.is_empty() {
        Value::Null
    } else {
        json!({
            "type": 17,
            "elements": runtime_directives,
            "loc": node.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
        })
    };
    let should_use_block = props_projection
        .get("shouldUseBlock")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    (
        props,
        patch_flag,
        dynamic_props,
        directives,
        should_use_block,
    )
}
