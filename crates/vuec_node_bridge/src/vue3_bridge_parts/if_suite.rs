#[derive(Default)]
pub(crate) struct Vue3IfSuiteState {
    pub(crate) errors: Vec<Value>,
    pub(crate) cached: usize,
}

pub(crate) fn vue3_core_transform_if_suite_value(payload: &Value) -> Value {
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
    let mut state = Vue3IfSuiteState::default();
    if let Some(children) = root.get_mut("children").and_then(Value::as_array_mut) {
        *children = vue3_if_suite_transform_children(
            std::mem::take(children),
            &options,
            &mut state,
            &Vue3ModelSuiteScope::default(),
        );
    }
    vue3_if_suite_finalize_root(&mut root, &state);
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

pub(crate) fn vue3_if_suite_transform_children(
    children: Vec<Value>,
    options: &Vue3CompilerOptions,
    state: &mut Vue3IfSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> Vec<Value> {
    let mut transformed = Vec::new();
    let mut index = 0usize;
    let mut key_base = 0usize;
    while index < children.len() {
        let child = children[index].clone();
        if vue3_public_node_type(&child) == Some(1)
            && vue3_text_suite_directive(&child, "if").is_some()
        {
            let (if_node, consumed) =
                vue3_if_suite_transform_if_chain(&children, index, key_base, options, state, scope);
            key_base += if_node
                .get("branches")
                .and_then(Value::as_array)
                .map(Vec::len)
                .unwrap_or_default();
            transformed.push(if_node);
            index += consumed.max(1);
            continue;
        }
        if vue3_public_node_type(&child) == Some(1)
            && (vue3_text_suite_directive(&child, "else").is_some()
                || vue3_text_suite_directive(&child, "else-if").is_some())
        {
            state.errors.push(json!({
                "code": 30,
                "loc": child.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
            }));
        }
        transformed.push(vue3_if_suite_transform_node(child, options, state, scope));
        index += 1;
    }
    transformed
}

pub(crate) fn vue3_if_suite_transform_if_chain(
    siblings: &[Value],
    start: usize,
    key_base: usize,
    options: &Vue3CompilerOptions,
    state: &mut Vue3IfSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> (Value, usize) {
    let first = siblings.get(start).cloned().unwrap_or(Value::Null);
    let mut branches = Vec::new();
    let first_branch =
        vue3_if_suite_branch_from_node(first.clone(), "if", Vec::new(), options, state, scope);
    branches.push(first_branch);

    let mut scan = start + 1;
    let mut consumed = 1usize;
    while scan < siblings.len() {
        let mut gap = Vec::<Value>::new();
        let mut candidate_index = scan;
        while let Some(candidate) = siblings.get(candidate_index) {
            if !vue3_if_suite_is_comment_or_ascii_whitespace(candidate) {
                break;
            }
            if vue3_public_node_type(candidate) == Some(3) {
                gap.push(candidate.clone());
            }
            candidate_index += 1;
        }
        let Some(candidate) = siblings.get(candidate_index) else {
            break;
        };
        let dir_name = if vue3_public_node_type(candidate) == Some(1)
            && vue3_text_suite_directive(candidate, "else-if").is_some()
        {
            "else-if"
        } else if vue3_public_node_type(candidate) == Some(1)
            && vue3_text_suite_directive(candidate, "else").is_some()
        {
            "else"
        } else {
            break;
        };
        let branch =
            vue3_if_suite_branch_from_node(candidate.clone(), dir_name, gap, options, state, scope);
        if branches
            .last()
            .and_then(|branch| branch.get("condition"))
            .is_some_and(Value::is_null)
        {
            state.errors.push(json!({
                "code": 30,
                "loc": branch.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
            }));
        }
        let current_key = branch.get("userKey").unwrap_or(&Value::Null);
        if !current_key.is_null()
            && branches.iter().any(|existing| {
                vue3_if_suite_same_user_key(
                    existing.get("userKey").unwrap_or(&Value::Null),
                    current_key,
                )
            })
        {
            state.errors.push(json!({
                "code": 29,
                "loc": current_key
                    .get("loc")
                    .cloned()
                    .unwrap_or_else(vue3_loc_stub_value),
            }));
        }
        branches.push(branch);
        consumed = candidate_index - start + 1;
        scan = candidate_index + 1;
    }

    let loc = first
        .get("loc")
        .cloned()
        .unwrap_or_else(vue3_loc_stub_value);
    let mut if_node = json!({
        "type": 9,
        "branches": branches,
        "codegenNode": Value::Null,
        "loc": loc,
    });
    let codegen = vue3_if_suite_if_codegen(&mut if_node, key_base);
    if_node["codegenNode"] = codegen;
    (if_node, consumed)
}

pub(crate) fn vue3_if_suite_branch_from_node(
    mut node: Value,
    dir_name: &str,
    comments: Vec<Value>,
    options: &Vue3CompilerOptions,
    state: &mut Vue3IfSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> Value {
    vue3_if_suite_apply_bind_shorthand(&mut node, state);
    let dir = vue3_text_suite_directive(&node, dir_name).cloned();
    let condition = vue3_if_suite_condition(dir.as_ref(), &node, options, state, scope);
    let user_key = vue3_if_suite_user_key(&node).unwrap_or(Value::Null);
    let is_template_if = node.get("tagType").and_then(Value::as_u64) == Some(3);

    vue3_text_suite_remove_directive(&mut node, "if");
    vue3_text_suite_remove_directive(&mut node, "else-if");
    vue3_text_suite_remove_directive(&mut node, "else");

    let mut children = comments;
    if is_template_if && vue3_text_suite_directive(&node, "for").is_none() {
        let template_children = node
            .get_mut("children")
            .and_then(Value::as_array_mut)
            .map(std::mem::take)
            .unwrap_or_default();
        children.extend(vue3_if_suite_transform_children(
            template_children,
            options,
            state,
            scope,
        ));
    } else {
        children.push(vue3_if_suite_transform_node(
            node.clone(),
            options,
            state,
            scope,
        ));
    }

    json!({
        "type": 10,
        "condition": condition,
        "children": children,
        "userKey": user_key,
        "isTemplateIf": is_template_if,
        "loc": node.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
    })
}

pub(crate) fn vue3_if_suite_transform_node(
    mut node: Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3IfSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> Value {
    if vue3_public_node_type(&node) == Some(1) {
        vue3_if_suite_apply_bind_shorthand(&mut node, state);
        vue3_model_suite_process_directive_expressions(&mut node, options, scope);
    }

    if let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) {
        *children =
            vue3_if_suite_transform_children(std::mem::take(children), options, state, scope);
    }

    if vue3_public_node_type(&node) == Some(5) {
        vue3_for_suite_process_expression_node(&mut node, "content", options, scope);
    }

    if vue3_public_node_type(&node) == Some(1) {
        node["codegenNode"] = vue3_if_suite_element_codegen(&node, options, state, scope, false);
    }
    node
}

pub(crate) fn vue3_if_suite_condition(
    dir: Option<&Value>,
    node: &Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3IfSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> Value {
    let Some(dir) = dir else {
        return Value::Null;
    };
    if dir.get("name").and_then(Value::as_str) == Some("else") {
        return Value::Null;
    }
    let exp = dir.get("exp").filter(|value| !value.is_null());
    let raw = exp
        .and_then(|exp| exp.get("content"))
        .and_then(Value::as_str)
        .unwrap_or("");
    if exp.is_none() || raw.trim().is_empty() {
        state.errors.push(json!({
            "code": 28,
            "loc": dir.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
        }));
        return json!({
            "type": 4,
            "content": "true",
            "isStatic": false,
            "constType": 0,
            "loc": exp
                .and_then(|exp| exp.get("loc"))
                .cloned()
                .or_else(|| node.get("loc").cloned())
                .unwrap_or_else(vue3_loc_stub_value),
        });
    }
    let current = exp.cloned().unwrap_or(Value::Null);
    if !options.prefix_identifiers {
        return current;
    }
    let projection = vuec_vue3_core::process_expression_projection(&json!({
        "node": current,
        "context": vue3_model_suite_transform_context(options, scope),
    }));
    vue3_text_suite_materialize_process_projection(&projection, &current)
}

pub(crate) fn vue3_if_suite_element_codegen(
    node: &Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3IfSuiteState,
    scope: &Vue3ModelSuiteScope,
    is_block: bool,
) -> Value {
    match (
        vue3_public_node_type(node),
        node.get("tagType").and_then(Value::as_u64),
    ) {
        (Some(1), Some(2)) => {
            let projection = vuec_vue3_core::transform_slot_outlet_projection(&json!({
                "node": node,
                "context": vue3_for_suite_slot_outlet_context(options, scope),
            }));
            let non_name_props = projection
                .get("process")
                .and_then(|process| process.get("nonNameProps"))
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let mut slot_state = Vue3SlotOutletSuiteState::default();
            let slot_props = vue3_slot_outlet_suite_props_codegen(
                node,
                &non_name_props,
                options,
                &mut slot_state,
            );
            for error in slot_state.errors {
                state.errors.push(error);
            }
            let slot_name = vue3_slot_outlet_suite_slot_name(node, projection.get("process"));
            vue3_slot_outlet_suite_codegen(
                node,
                slot_name,
                slot_props,
                projection.get("codegen").unwrap_or(&Value::Null),
            )
        }
        (Some(1), Some(1)) => {
            let (props, patch_flag, dynamic_props, directives, should_use_block) =
                vue3_if_suite_props_codegen(node, options, state, scope);
            let tag = node.get("tag").and_then(Value::as_str).unwrap_or("");
            let mut vnode = vue3_once_suite_vnode_call(
                &vue3_once_suite_component_asset_id(tag),
                props,
                Value::Null,
                patch_flag,
                dynamic_props,
                is_block || should_use_block,
                false,
                true,
            );
            vnode["directives"] = directives;
            vnode
        }
        (Some(1), Some(0)) => {
            let (props, mut patch_flag, dynamic_props, directives, should_use_block) =
                vue3_if_suite_props_codegen(node, options, state, scope);
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
            if patch_flag.is_none()
                && vue3_suite_child_needs_text_patch_flag(&children, options, scope)
            {
                patch_flag = Some(json!(1));
            }
            let is_block = is_block || should_use_block;
            if patch_flag.is_none() && !directives.is_null() && !is_block {
                patch_flag = Some(json!(512));
            }
            let mut vnode = vue3_once_suite_vnode_call(
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
            );
            vnode["directives"] = directives;
            vnode
        }
        _ => Value::Null,
    }
}

pub(crate) fn vue3_if_suite_props_codegen(
    node: &Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3IfSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> (Value, Option<Value>, Value, Value, bool) {
    let mut properties = Vec::<Value>::new();
    let mut merge_args = Vec::<Value>::new();
    let mut dynamic_props = Vec::<String>::new();
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
                properties.push(vue3_once_suite_object_property(
                    vue3_once_suite_simple_expression(name, true),
                    vue3_once_suite_simple_expression(value, true),
                ));
                prop_summaries.push(json!({ "kind": "attribute", "name": name, "value": value }));
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
                    if let Some(name) = vue3_model_suite_static_prop_name(&key) {
                        if name != "key" && !value_constant {
                            dynamic_props.push(name.clone());
                        }
                        prop_summaries.push(json!({
                            "kind": "directiveProp",
                            "name": name,
                            "dynamicKey": false,
                            "valueConstant": value_constant,
                            "forceBlock": name == "key",
                        }));
                    } else {
                        prop_summaries.push(json!({
                            "kind": "directiveProp",
                            "dynamicKey": true,
                            "valueConstant": value_constant,
                        }));
                    }
                    properties.push(vue3_once_suite_object_property(key, value));
                }
            }
            Some(7) if prop.get("name").and_then(Value::as_str) == Some("on") => {
                if prop.get("arg").is_none_or(Value::is_null) {
                    prop_summaries.push(json!({ "kind": "objectOn" }));
                    vue3_if_suite_push_props_object_arg(&mut merge_args, &mut properties, node);
                    if let Some(exp) = prop.get("exp").filter(|value| !value.is_null()) {
                        let mut args = vec![exp.clone()];
                        if node.get("tagType").and_then(Value::as_u64) == Some(0) {
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
                        value = vue3_if_suite_cache_expression(state, value, false, false, false);
                    }
                    if !projected_prop
                        .get("dynamicKey")
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                    {
                        if let Some(name) = vue3_model_suite_static_prop_name(&key) {
                            if !cached
                                && !projected_prop
                                    .get("valueConstant")
                                    .and_then(Value::as_bool)
                                    .unwrap_or(false)
                            {
                                dynamic_props.push(name.clone());
                            }
                            prop_summaries.push(json!({
                                "kind": "directiveProp",
                                "name": name,
                                "dynamicKey": false,
                                "valueConstant": projected_prop
                                    .get("valueConstant")
                                    .and_then(Value::as_bool)
                                    .unwrap_or(false),
                                "valueCached": cached,
                            }));
                        }
                    } else {
                        prop_summaries.push(json!({
                            "kind": "directiveProp",
                            "dynamicKey": true,
                            "ignoreDynamicKeyForNormalize": true,
                            "valueConstant": false,
                            "valueCached": cached,
                        }));
                    }
                    properties.push(vue3_once_suite_object_property(key, value));
                }
            }
            Some(7) => {
                let Some(name) = prop.get("name").and_then(Value::as_str) else {
                    continue;
                };
                if !vue3_text_suite_builtin_directive(name) {
                    prop_summaries.push(json!({ "kind": "runtimeDirective" }));
                    runtime_directives.push(Value::Array(vec![Value::String(
                        vue3_text_suite_directive_asset_id(name),
                    )]));
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
        "isComponent": node.get("tagType").and_then(Value::as_u64) == Some(1),
        "isDynamicComponent": false,
        "context": context,
    }));

    let mut props = if merge_args.is_empty() {
        if properties.is_empty() {
            Value::Null
        } else {
            vue3_if_suite_props_object(properties, node)
        }
    } else {
        vue3_if_suite_push_props_object_arg(&mut merge_args, &mut properties, node);
        if merge_args.len() == 1 {
            merge_args.pop().unwrap_or(Value::Null)
        } else {
            vue3_text_suite_call("MERGE_PROPS", merge_args)
        }
    };
    vue3_if_suite_apply_props_normalizers(&mut props, &props_projection);
    let patch_flag = props_projection
        .get("patchFlag")
        .and_then(Value::as_u64)
        .filter(|flag| *flag > 0)
        .map(|flag| json!(flag));
    let projected_dynamic_props = props_projection
        .get("dynamicPropNames")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or(dynamic_props);
    let dynamic_props = if projected_dynamic_props.is_empty() {
        Value::Null
    } else {
        Value::String(vue3_model_suite_dynamic_props_string(
            &projected_dynamic_props,
        ))
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

pub(crate) fn vue3_if_suite_value_constant(value: &Value, context: &Value) -> u64 {
    if let Some(const_type) = value.get("constType").and_then(Value::as_u64) {
        return const_type;
    }
    vuec_vue3_core::get_constant_type_projection(&json!({
        "node": value,
        "context": context,
    }))
    .get("constantType")
    .and_then(Value::as_u64)
    .unwrap_or(0)
}

pub(crate) fn vue3_suite_child_needs_text_patch_flag(
    child: &Value,
    options: &Vue3CompilerOptions,
    scope: &Vue3ModelSuiteScope,
) -> bool {
    if !matches!(vue3_public_node_type(child), Some(5 | 8)) {
        return false;
    }
    let context = vue3_model_suite_transform_context(options, scope);
    vue3_if_suite_value_constant(child, &context) == 0
}

pub(crate) fn vue3_if_suite_apply_props_normalizers(props: &mut Value, projection: &Value) {
    if props.is_null() {
        return;
    }
    let is_call = vue3_public_node_type(props) == Some(14);
    if !is_call {
        if projection
            .get("normalizeClass")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            vue3_if_suite_normalize_object_prop(props, "class", "NORMALIZE_CLASS");
        }
        if projection
            .get("normalizeStyle")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            vue3_if_suite_normalize_object_prop(props, "style", "NORMALIZE_STYLE");
        }
    }
    let guard_reactive_props = projection
        .get("guardReactiveProps")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let normalize_props = projection
        .get("normalizeProps")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if is_call {
        return;
    }
    if guard_reactive_props {
        let current = std::mem::take(props);
        *props = vue3_text_suite_call("GUARD_REACTIVE_PROPS", vec![current]);
    }
    if normalize_props && props.get("callee").and_then(Value::as_str) != Some("NORMALIZE_PROPS") {
        let current = std::mem::take(props);
        *props = vue3_text_suite_call("NORMALIZE_PROPS", vec![current]);
    }
}

pub(crate) fn vue3_if_suite_normalize_object_prop(props: &mut Value, name: &str, helper: &str) {
    match vue3_public_node_type(props) {
        Some(15) => {
            let Some(properties) = props.get_mut("properties").and_then(Value::as_array_mut) else {
                return;
            };
            for property in properties {
                let key_name = property
                    .get("key")
                    .and_then(vue3_model_suite_static_prop_name);
                if key_name.as_deref() != Some(name) {
                    continue;
                }
                let value = property.get("value").cloned().unwrap_or(Value::Null);
                if value.get("callee").and_then(Value::as_str) != Some(helper) {
                    property["value"] = vue3_text_suite_call(helper, vec![value]);
                }
            }
        }
        Some(14) => {
            let Some(arguments) = props.get_mut("arguments").and_then(Value::as_array_mut) else {
                return;
            };
            for argument in arguments {
                vue3_if_suite_normalize_object_prop(argument, name, helper);
            }
        }
        _ => {}
    }
}

pub(crate) fn vue3_if_suite_if_codegen(if_node: &mut Value, key_base: usize) -> Value {
    let branches = if_node
        .get_mut("branches")
        .and_then(Value::as_array_mut)
        .map(Vec::as_mut_slice)
        .unwrap_or(&mut []);
    let mut alternate =
        vue3_text_suite_call("CREATE_COMMENT", vec![json!("\"v-if\""), json!("true")]);
    for (index, branch) in branches.iter_mut().enumerate().rev() {
        let child_codegen = vue3_if_suite_branch_codegen(branch, key_base + index);
        let condition = branch.get("condition").cloned().unwrap_or(Value::Null);
        if condition.is_null() {
            alternate = child_codegen;
        } else {
            alternate = json!({
                "type": 19,
                "test": condition,
                "consequent": child_codegen,
                "alternate": alternate,
                "newline": true,
                "loc": branch.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
            });
        }
    }
    alternate
}

pub(crate) fn vue3_if_suite_branch_codegen(branch: &mut Value, key_index: usize) -> Value {
    let projection = vuec_vue3_core::transform_if_projection(&json!({
        "phase": "branchCodegen",
        "branch": branch,
        "keyIndex": key_index,
    }));
    let key_property = vue3_for_suite_branch_key_property(key_index);
    match projection.get("kind").and_then(Value::as_str) {
        Some("for") => {
            if let Some(codegen) = branch
                .get_mut("children")
                .and_then(Value::as_array_mut)
                .and_then(|children| children.first_mut())
                .and_then(|child| child.get_mut("codegenNode"))
            {
                vue3_if_suite_inject_prop(codegen, key_property);
                return codegen.clone();
            }
            Value::Null
        }
        Some("fragment") => {
            let props = json!({
                "type": 15,
                "properties": [key_property],
                "loc": branch.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
            });
            vue3_once_suite_vnode_call(
                "FRAGMENT",
                props,
                branch.get("children").cloned().unwrap_or_else(|| json!([])),
                Some(
                    projection
                        .get("patchFlag")
                        .cloned()
                        .unwrap_or_else(|| json!(64)),
                ),
                Value::Null,
                true,
                false,
                false,
            )
        }
        _ => {
            if let Some(codegen) = branch
                .get_mut("children")
                .and_then(Value::as_array_mut)
                .and_then(|children| children.first_mut())
                .and_then(|child| child.get_mut("codegenNode"))
            {
                if vue3_public_node_type(codegen) == Some(13) {
                    codegen["isBlock"] = json!(true);
                }
                vue3_if_suite_inject_prop(codegen, key_property);
                return codegen.clone();
            }
            Value::Null
        }
    }
}

pub(crate) fn vue3_if_suite_inject_prop(target: &mut Value, property: Value) {
    match vue3_public_node_type(target) {
        Some(13) => {
            let props = target.get("props").cloned().unwrap_or(Value::Null);
            target["props"] = vue3_if_suite_inject_prop_into_props(props, property);
        }
        Some(14) if target.get("callee").and_then(Value::as_str) == Some("RENDER_SLOT") => {
            let Some(arguments) = target.get_mut("arguments").and_then(Value::as_array_mut) else {
                return;
            };
            while arguments.len() <= 2 {
                arguments.push(if arguments.len() == 2 {
                    Value::Null
                } else {
                    Value::String("undefined".to_string())
                });
            }
            let props = arguments.get(2).cloned().unwrap_or(Value::Null);
            arguments[2] = vue3_if_suite_inject_prop_into_props(props, property);
        }
        _ => {}
    }
}

pub(crate) fn vue3_if_suite_inject_prop_into_props(props: Value, property: Value) -> Value {
    if props.is_null() || props.is_string() {
        return vue3_if_suite_props_object(vec![property], &Value::Null);
    }
    if vue3_public_node_type(&props) == Some(15) {
        let mut object = props;
        vue3_for_suite_prepend_object_property(&mut object, property);
        return object;
    }
    if vue3_public_node_type(&props) == Some(14) {
        let callee = props.get("callee").and_then(Value::as_str);
        if callee == Some("NORMALIZE_PROPS") {
            let mut call = props;
            let Some(arguments) = call.get_mut("arguments").and_then(Value::as_array_mut) else {
                return call;
            };
            let first = arguments.first().cloned().unwrap_or(Value::Null);
            let injected =
                if first.get("callee").and_then(Value::as_str) == Some("GUARD_REACTIVE_PROPS") {
                    first
                        .get("arguments")
                        .and_then(Value::as_array)
                        .and_then(|arguments| arguments.first())
                        .cloned()
                        .map(|raw| vue3_if_suite_inject_prop_into_props(raw, property))
                        .unwrap_or(first)
                } else {
                    vue3_if_suite_inject_prop_into_props(first, property)
                };
            if arguments.is_empty() {
                arguments.push(injected);
            } else {
                arguments[0] = injected;
            }
            return call;
        }
        if callee == Some("GUARD_REACTIVE_PROPS") {
            return props
                .get("arguments")
                .and_then(Value::as_array)
                .and_then(|arguments| arguments.first())
                .cloned()
                .map(|raw| vue3_if_suite_inject_prop_into_props(raw, property))
                .unwrap_or(props);
        }
        if callee == Some("MERGE_PROPS") {
            let mut call = props;
            if let Some(arguments) = call.get_mut("arguments").and_then(Value::as_array_mut) {
                if let Some(first) = arguments.first_mut() {
                    if vue3_public_node_type(first) == Some(15) {
                        vue3_for_suite_prepend_object_property(first, property);
                    } else {
                        arguments
                            .insert(0, vue3_if_suite_props_object(vec![property], &Value::Null));
                    }
                } else {
                    arguments.push(vue3_if_suite_props_object(vec![property], &Value::Null));
                }
            }
            return call;
        }
        if callee == Some("TO_HANDLERS") {
            return vue3_text_suite_call(
                "MERGE_PROPS",
                vec![
                    vue3_if_suite_props_object(vec![property], &Value::Null),
                    props,
                ],
            );
        }
        let mut call = props;
        if let Some(arguments) = call.get_mut("arguments").and_then(Value::as_array_mut) {
            arguments.insert(0, vue3_if_suite_props_object(vec![property], &Value::Null));
        }
        return call;
    }
    vue3_text_suite_call(
        "MERGE_PROPS",
        vec![
            vue3_if_suite_props_object(vec![property], &Value::Null),
            props,
        ],
    )
}

pub(crate) fn vue3_if_suite_push_props_object_arg(
    merge_args: &mut Vec<Value>,
    properties: &mut Vec<Value>,
    node: &Value,
) {
    if properties.is_empty() {
        return;
    }
    merge_args.push(vue3_if_suite_props_object(std::mem::take(properties), node));
}

pub(crate) fn vue3_if_suite_props_object(properties: Vec<Value>, node: &Value) -> Value {
    json!({
        "type": 15,
        "properties": properties,
        "loc": node.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
    })
}

pub(crate) fn vue3_if_suite_apply_bind_shorthand(node: &mut Value, state: &mut Vue3IfSuiteState) {
    let projection = vuec_vue3_core::transform_v_bind_shorthand_projection(&json!({
        "node": node,
        "context": { "browser": false },
    }));
    let operations = projection
        .get("operations")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let Some(props) = node.get_mut("props").and_then(Value::as_array_mut) else {
        return;
    };
    for operation in operations {
        let Some(index) = operation.get("index").and_then(Value::as_u64) else {
            continue;
        };
        let Some(prop) = props.get_mut(index as usize) else {
            continue;
        };
        for error in operation
            .get("errors")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            state.errors.push(vue3_bind_suite_error_value(error, prop));
        }
        if operation.get("kind").and_then(Value::as_str) == Some("setExp") {
            let exp = operation.get("exp").cloned().unwrap_or(Value::Null);
            prop["exp"] = vue3_text_suite_materialize_process_projection(&exp, &exp);
        }
    }
}

pub(crate) fn vue3_if_suite_user_key(node: &Value) -> Option<Value> {
    node.get("props")
        .and_then(Value::as_array)?
        .iter()
        .find(|prop| match vue3_public_node_type(prop) {
            Some(6) => prop.get("name").and_then(Value::as_str) == Some("key"),
            Some(7) => {
                prop.get("name").and_then(Value::as_str) == Some("bind")
                    && prop.get("arg").is_some_and(|arg| {
                        vue3_public_node_type(arg) == Some(4)
                            && arg
                                .get("isStatic")
                                .and_then(Value::as_bool)
                                .unwrap_or(false)
                            && arg.get("content").and_then(Value::as_str) == Some("key")
                    })
            }
            _ => false,
        })
        .cloned()
}

pub(crate) fn vue3_if_suite_same_user_key(a: &Value, b: &Value) -> bool {
    if a.is_null() || b.is_null() || vue3_public_node_type(a) != vue3_public_node_type(b) {
        return false;
    }
    match vue3_public_node_type(a) {
        Some(6) => {
            a.get("value")
                .and_then(|value| value.get("content"))
                .and_then(Value::as_str)
                == b.get("value")
                    .and_then(|value| value.get("content"))
                    .and_then(Value::as_str)
        }
        Some(7) => {
            let a_exp = a.get("exp").unwrap_or(&Value::Null);
            let b_exp = b.get("exp").unwrap_or(&Value::Null);
            vue3_public_node_type(a_exp) == vue3_public_node_type(b_exp)
                && a_exp.get("isStatic").and_then(Value::as_bool)
                    == b_exp.get("isStatic").and_then(Value::as_bool)
                && a_exp.get("content").and_then(Value::as_str)
                    == b_exp.get("content").and_then(Value::as_str)
        }
        _ => false,
    }
}

pub(crate) fn vue3_if_suite_is_comment_or_ascii_whitespace(node: &Value) -> bool {
    match vue3_public_node_type(node) {
        Some(3) => true,
        Some(2) => node
            .get("content")
            .and_then(Value::as_str)
            .is_some_and(|content| {
                content
                    .bytes()
                    .all(|byte| matches!(byte, b'\t' | b'\n' | b'\x0C' | b'\r' | b' '))
            }),
        Some(12) => node
            .get("content")
            .is_some_and(vue3_if_suite_is_comment_or_ascii_whitespace),
        _ => false,
    }
}

pub(crate) fn vue3_if_suite_cache_expression(
    state: &mut Vue3IfSuiteState,
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

pub(crate) fn vue3_if_suite_finalize_root(root: &mut Value, state: &Vue3IfSuiteState) {
    vue3_once_suite_set_root_codegen(root);
    root["components"] = json!(vue3_once_suite_components(root));
    root["directives"] = json!(vue3_if_suite_collect_directives(root));
    root["helpers"] = json!(vue3_if_suite_helpers(root));
    root["hoists"] = json!([]);
    root["cached"] = Value::Array((0..state.cached).map(|_| Value::Null).collect());
    root["temps"] = json!(0);
}

pub(crate) fn vue3_if_suite_collect_directives(root: &Value) -> Vec<String> {
    let mut directives = Vec::new();
    vue3_if_suite_collect_directives_for_node(root, &mut directives);
    directives
}

pub(crate) fn vue3_if_suite_collect_directives_for_node(
    node: &Value,
    directives: &mut Vec<String>,
) {
    for name in vue3_text_suite_runtime_directive_names(node) {
        if !directives.iter().any(|existing| existing == &name) {
            directives.push(name);
        }
    }
    for key in ["children", "branches", "content"] {
        let Some(value) = node.get(key) else {
            continue;
        };
        if let Some(items) = value.as_array() {
            for item in items {
                vue3_if_suite_collect_directives_for_node(item, directives);
            }
        } else if value.is_object() {
            vue3_if_suite_collect_directives_for_node(value, directives);
        }
    }
}

pub(crate) fn vue3_if_suite_helpers(root: &Value) -> Vec<String> {
    let mut used = Vec::new();
    vue3_if_suite_collect_helpers(root.get("codegenNode").unwrap_or(&Value::Null), &mut used);
    if root
        .get("components")
        .and_then(Value::as_array)
        .is_some_and(|components| !components.is_empty())
    {
        vue3_text_suite_add_helper(&mut used, "RESOLVE_COMPONENT");
    }
    if !vue3_if_suite_collect_directives(root).is_empty() {
        vue3_text_suite_add_helper(&mut used, "RESOLVE_DIRECTIVE");
    }
    used.into_iter().map(str::to_string).collect()
}

pub(crate) fn vue3_if_suite_collect_helpers(node: &Value, used: &mut Vec<&'static str>) {
    match vue3_public_node_type(node) {
        Some(9) => {
            if let Some(branches) = node.get("branches").and_then(Value::as_array) {
                if let Some(first) = branches.first() {
                    vue3_if_suite_collect_branch_child_helpers(first, used);
                    vue3_text_suite_add_helper(used, "CREATE_COMMENT");
                }
                for branch in branches.iter().skip(1) {
                    vue3_if_suite_collect_branch_child_helpers(branch, used);
                }
            }
            if let Some(codegen) = node.get("codegenNode") {
                vue3_if_suite_collect_call_helpers(codegen, used);
            }
        }
        Some(19) => {
            vue3_if_suite_collect_helpers(node.get("consequent").unwrap_or(&Value::Null), used);
            vue3_text_suite_add_helper(used, "CREATE_COMMENT");
            vue3_if_suite_collect_helpers(node.get("alternate").unwrap_or(&Value::Null), used);
        }
        Some(13) => {
            if let Some(children) = node.get("children") {
                vue3_if_suite_collect_helpers(children, used);
            }
            if let Some(props) = node.get("props") {
                vue3_if_suite_collect_helpers(props, used);
            }
            if let Some(directives) = node.get("directives") {
                vue3_if_suite_collect_helpers(directives, used);
            }
            if node.get("directives").is_some_and(|value| !value.is_null()) {
                vue3_text_suite_add_helper(used, "WITH_DIRECTIVES");
            }
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
        Some(14) => {
            if let Some(callee) = node.get("callee").and_then(Value::as_str) {
                match callee {
                    "CREATE_COMMENT" => vue3_text_suite_add_helper(used, "CREATE_COMMENT"),
                    "MERGE_PROPS" => vue3_text_suite_add_helper(used, "MERGE_PROPS"),
                    "NORMALIZE_PROPS" => vue3_text_suite_add_helper(used, "NORMALIZE_PROPS"),
                    "RENDER_SLOT" => vue3_text_suite_add_helper(used, "RENDER_SLOT"),
                    "TO_HANDLERS" => vue3_text_suite_add_helper(used, "TO_HANDLERS"),
                    _ => {}
                }
            }
            for key in ["arguments", "props", "children"] {
                if let Some(value) = node.get(key) {
                    vue3_if_suite_collect_helpers(value, used);
                }
            }
        }
        Some(5) => vue3_text_suite_add_helper(used, "TO_DISPLAY_STRING"),
        Some(12) => {
            if let Some(codegen) = node.get("codegenNode") {
                vue3_if_suite_collect_helpers(codegen, used);
            }
        }
        _ => {
            if let Some(items) = node.as_array() {
                for item in items {
                    vue3_if_suite_collect_helpers(item, used);
                }
            } else {
                for key in [
                    "children",
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
                    if let Some(value) = node.get(key) {
                        vue3_if_suite_collect_helpers(value, used);
                    }
                }
            }
        }
    }
}

pub(crate) fn vue3_if_suite_collect_branch_child_helpers(
    branch: &Value,
    used: &mut Vec<&'static str>,
) {
    if let Some(children) = branch.get("children").and_then(Value::as_array) {
        if children.len() != 1 || children.first().and_then(vue3_public_node_type) != Some(1) {
            for child in children {
                vue3_if_suite_collect_helpers(child, used);
            }
            vue3_text_suite_add_helper(used, "FRAGMENT");
            vue3_text_suite_add_helper(used, "OPEN_BLOCK");
            vue3_text_suite_add_helper(used, "CREATE_ELEMENT_BLOCK");
            return;
        }
        if let Some(child) = children.first() {
            vue3_if_suite_collect_helpers(child.get("codegenNode").unwrap_or(&Value::Null), used);
        }
    }
}

pub(crate) fn vue3_if_suite_collect_call_helpers(node: &Value, used: &mut Vec<&'static str>) {
    if vue3_public_node_type(node) == Some(14) {
        if let Some(callee) = node.get("callee").and_then(Value::as_str) {
            match callee {
                "MERGE_PROPS" => vue3_text_suite_add_helper(used, "MERGE_PROPS"),
                "NORMALIZE_PROPS" => vue3_text_suite_add_helper(used, "NORMALIZE_PROPS"),
                "TO_HANDLERS" => vue3_text_suite_add_helper(used, "TO_HANDLERS"),
                _ => {}
            }
        }
    }
    for key in ["arguments", "props", "children", "consequent", "alternate"] {
        if let Some(value) = node.get(key) {
            vue3_if_suite_collect_call_helpers(value, used);
        }
    }
    if let Some(items) = node.as_array() {
        for item in items {
            vue3_if_suite_collect_call_helpers(item, used);
        }
    }
}
