pub(crate) fn vue3_core_transform_for_suite_value(payload: &Value) -> Value {
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
    let mut state = Vue3ForSuiteState::default();
    if let Some(children) = root.get_mut("children").and_then(Value::as_array_mut) {
        let transformed = vue3_for_suite_transform_children(
            std::mem::take(children),
            &options,
            &mut state,
            &Vue3ModelSuiteScope::default(),
        );
        *children = transformed;
    }
    vue3_for_suite_finalize_root(&mut root);
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

pub(crate) fn vue3_for_suite_transform_children(
    children: Vec<Value>,
    options: &Vue3CompilerOptions,
    state: &mut Vue3ForSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> Vec<Value> {
    let mut transformed = Vec::new();
    let mut index = 0usize;
    while index < children.len() {
        let child = children[index].clone();
        if vue3_public_node_type(&child) == Some(1)
            && vue3_text_suite_directive(&child, "if").is_some()
        {
            let if_node =
                vue3_for_suite_transform_if_node(child, &children, index, options, state, scope);
            transformed.push(if_node);
        } else {
            transformed.push(vue3_for_suite_transform_node(child, options, state, scope));
        }
        index += 1;
    }
    transformed
}

pub(crate) fn vue3_for_suite_transform_node(
    mut node: Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3ForSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> Value {
    if vue3_public_node_type(&node) == Some(1) {
        vue3_for_suite_apply_bind_shorthand(&mut node, state);
        if vue3_text_suite_directive(&node, "for").is_some() {
            return vue3_for_suite_transform_for_node(node, options, state, scope);
        }
        vue3_model_suite_process_directive_expressions(&mut node, options, scope);
    }

    if let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) {
        let transformed =
            vue3_for_suite_transform_children(std::mem::take(children), options, state, scope);
        *children = transformed;
    }

    if vue3_public_node_type(&node) == Some(5) {
        vue3_for_suite_process_expression_node(&mut node, "content", options, scope);
    }

    if vue3_public_node_type(&node) == Some(1) {
        node["codegenNode"] = vue3_for_suite_element_codegen(&node, options, state, scope, false);
    }
    node
}

pub(crate) fn vue3_for_suite_transform_if_node(
    mut node: Value,
    siblings: &[Value],
    index: usize,
    options: &Vue3CompilerOptions,
    state: &mut Vue3ForSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> Value {
    vue3_for_suite_apply_bind_shorthand(&mut node, state);
    let Some(dir) = vue3_text_suite_directive(&node, "if").cloned() else {
        return vue3_for_suite_transform_node(node, options, state, scope);
    };
    let projection = vuec_vue3_core::transform_if_projection(&json!({
        "node": node,
        "dir": dir,
        "siblings": siblings,
        "nodeIndex": index,
        "context": vue3_model_suite_transform_context(options, scope),
    }));
    for error in projection
        .get("errors")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        state.errors.push(vue3_model_suite_error_value(error, &dir));
    }

    let mut branch_child = node.clone();
    vue3_text_suite_remove_directive(&mut branch_child, "if");
    let transformed_child = vue3_for_suite_transform_node(branch_child, options, state, scope);
    let condition = projection
        .get("branch")
        .and_then(|branch| branch.get("condition"))
        .filter(|condition| !condition.is_null())
        .map(|condition| vue3_text_suite_materialize_process_projection(condition, condition))
        .or_else(|| dir.get("exp").cloned())
        .unwrap_or(Value::Null);

    let loc = node.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value);
    let mut if_node = json!({
        "type": 9,
        "branches": [{
            "type": 10,
            "condition": condition,
            "children": [transformed_child],
            "userKey": Value::Null,
            "isTemplateIf": node.get("tagType").and_then(Value::as_u64) == Some(3),
            "loc": loc,
        }],
        "codegenNode": Value::Null,
        "loc": loc,
    });
    let key_base = projection
        .get("action")
        .and_then(|action| action.get("keyBase"))
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    if_node["codegenNode"] = vue3_for_suite_if_codegen(&mut if_node, key_base);
    if_node
}

pub(crate) fn vue3_for_suite_transform_for_node(
    mut node: Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3ForSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> Value {
    let Some(dir) = vue3_text_suite_directive(&node, "for").cloned() else {
        return vue3_for_suite_transform_node(node, options, state, scope);
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
    for error in projection
        .get("templateKeyErrors")
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

    let mut child_scope = scope.clone();
    vue3_model_suite_add_locals(&mut child_scope, projection.get("locals"));
    child_scope.v_for_depth += 1;
    let original_node = node.clone();
    let fallback_loc = node.get("loc").cloned();
    let children = if projection.get("children").and_then(Value::as_str) == Some("template") {
        node.get_mut("children")
            .and_then(Value::as_array_mut)
            .map(std::mem::take)
            .unwrap_or_default()
            .into_iter()
            .map(|child| vue3_for_suite_transform_node(child, options, state, &child_scope))
            .collect::<Vec<_>>()
    } else {
        vue3_text_suite_remove_directive(&mut node, "for");
        vec![vue3_for_suite_transform_node(
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
    for_node["codegenNode"] =
        vue3_for_suite_for_codegen(&original_node, &mut for_node, options, &child_scope, state);
    for_node
}

pub(crate) fn vue3_for_suite_apply_bind_shorthand(node: &mut Value, state: &mut Vue3ForSuiteState) {
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

pub(crate) fn vue3_for_suite_process_expression_node(
    node: &mut Value,
    key: &str,
    options: &Vue3CompilerOptions,
    scope: &Vue3ModelSuiteScope,
) {
    if !options.prefix_identifiers {
        return;
    }
    let current = node.get(key).cloned().unwrap_or(Value::Null);
    if current.is_null() {
        return;
    }
    let projection = vuec_vue3_core::process_expression_projection(&json!({
        "node": current,
        "context": vue3_model_suite_transform_context(options, scope),
    }));
    node[key] = vue3_text_suite_materialize_process_projection(&projection, &current);
}

pub(crate) fn vue3_for_suite_for_codegen(
    original_node: &Value,
    for_node: &mut Value,
    options: &Vue3CompilerOptions,
    scope: &Vue3ModelSuiteScope,
    state: &mut Vue3ForSuiteState,
) -> Value {
    let context = vue3_model_suite_transform_context(options, scope);
    let codegen_projection = vuec_vue3_core::transform_for_projection(&json!({
        "phase": "codegen",
        "node": original_node,
        "forNode": vue3_for_suite_for_node_payload(for_node),
        "context": context,
    }));
    let key_property = vue3_for_suite_key_property(
        codegen_projection.get("keyProperty"),
        original_node,
        options,
        scope,
    );
    let is_stable_fragment = codegen_projection
        .get("isStableFragment")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let exit_projection = vuec_vue3_core::transform_for_projection(&json!({
        "phase": "exitCodegen",
        "node": original_node,
        "forNode": vue3_for_suite_for_node_payload(for_node),
        "isStableFragment": is_stable_fragment,
    }));
    let child_block = vue3_for_suite_child_block(
        &exit_projection,
        original_node,
        for_node,
        key_property.clone(),
        options,
        scope,
        state,
    );
    let render_list = vue3_text_suite_call(
        "RENDER_LIST",
        vec![
            for_node.get("source").cloned().unwrap_or(Value::Null),
            json!({
                "type": 18,
                "params": vue3_for_suite_loop_params(for_node.get("parseResult").unwrap_or(&Value::Null)),
                "returns": child_block,
                "newline": true,
                "isSlot": false,
                "loc": for_node.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
            }),
        ],
    );
    vue3_once_suite_vnode_call(
        "FRAGMENT",
        Value::Null,
        render_list,
        Some(
            codegen_projection
                .get("fragmentFlag")
                .cloned()
                .unwrap_or_else(|| json!(256)),
        ),
        Value::Null,
        true,
        codegen_projection
            .get("disableTracking")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        false,
    )
}

pub(crate) fn vue3_for_suite_child_block(
    projection: &Value,
    original_node: &Value,
    for_node: &mut Value,
    key_property: Option<Value>,
    options: &Vue3CompilerOptions,
    scope: &Vue3ModelSuiteScope,
    state: &mut Vue3ForSuiteState,
) -> Value {
    match projection.get("kind").and_then(Value::as_str) {
        Some("slotOutlet") => {
            let slot = if projection.get("path").and_then(Value::as_str) == Some("templateChild") {
                let index = projection.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
                for_node
                    .get("children")
                    .and_then(Value::as_array)
                    .and_then(|children| children.get(index))
                    .cloned()
                    .unwrap_or(Value::Null)
            } else {
                for_node
                    .get("children")
                    .and_then(Value::as_array)
                    .and_then(|children| children.first())
                    .cloned()
                    .unwrap_or(Value::Null)
            };
            let mut codegen = slot.get("codegenNode").cloned().unwrap_or(Value::Null);
            if projection.get("path").and_then(Value::as_str) == Some("templateChild") {
                if let Some(key_property) = key_property {
                    vue3_for_suite_inject_prop(&mut codegen, key_property);
                }
            }
            codegen
        }
        Some("fragmentWrapper") => {
            let props = key_property
                .map(|key| {
                    json!({
                        "type": 15,
                        "properties": [key],
                        "loc": for_node.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
                    })
                })
                .unwrap_or(Value::Null);
            vue3_once_suite_vnode_call(
                "FRAGMENT",
                props,
                for_node
                    .get("children")
                    .cloned()
                    .unwrap_or_else(|| json!([])),
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
            let mut child_block = for_node
                .get("children")
                .and_then(Value::as_array)
                .and_then(|children| children.first())
                .and_then(|child| child.get("codegenNode"))
                .cloned()
                .unwrap_or(Value::Null);
            if original_node.get("tagType").and_then(Value::as_u64) == Some(3) {
                if let Some(key_property) = key_property {
                    vue3_for_suite_inject_prop(&mut child_block, key_property);
                }
            }
            if vue3_public_node_type(&child_block) == Some(13) {
                child_block["isBlock"] = json!(projection
                    .get("childBlockIsBlock")
                    .and_then(Value::as_bool)
                    .unwrap_or(true));
            }
            let _ = (options, scope, state);
            child_block
        }
    }
}

pub(crate) fn vue3_for_suite_element_codegen(
    node: &Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3ForSuiteState,
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
        (Some(1), Some(0)) => {
            let (props, mut patch_flag, dynamic_props, directives) =
                vue3_for_suite_props_codegen(node, options, state, scope);
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
            if patch_flag.is_none() && !directives.is_null() {
                patch_flag = Some(json!(512));
            }
            let mut vnode = vue3_text_suite_vnode_call(
                &vue3_once_suite_quote_string(
                    node.get("tag").and_then(Value::as_str).unwrap_or(""),
                ),
                props,
                children,
                patch_flag,
                is_block,
                false,
                directives,
            );
            vnode["dynamicProps"] = dynamic_props;
            vnode
        }
        _ => Value::Null,
    }
}

pub(crate) fn vue3_for_suite_props_codegen(
    node: &Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3ForSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> (Value, Option<Value>, Value, Value) {
    let mut properties = Vec::new();
    let mut merge_args = Vec::<Value>::new();
    let mut dynamic_props = Vec::<Value>::new();
    let mut runtime_directives = Vec::<Value>::new();
    let mut prop_summaries = Vec::<Value>::new();
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
                prop_summaries.push(json!({
                    "kind": "attribute",
                    "name": name,
                }));
                let value = prop
                    .get("value")
                    .and_then(|value| value.get("content"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                properties.push(vue3_once_suite_object_property(
                    vue3_once_suite_simple_expression(name, true),
                    vue3_once_suite_simple_expression(value, true),
                ));
            }
            Some(7) if prop.get("name").and_then(Value::as_str) == Some("bind") => {
                if prop.get("arg").is_none_or(Value::is_null) {
                    if let Some(exp) = prop.get("exp").filter(|value| !value.is_null()) {
                        prop_summaries.push(json!({ "kind": "objectBind" }));
                        vue3_for_suite_push_props_object_arg(
                            &mut merge_args,
                            &mut properties,
                            node,
                        );
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
                    if let Some(name) = vue3_model_suite_static_prop_name(&key) {
                        if name != "key" {
                            dynamic_props.push(Value::String(vue3_once_suite_quote_string(&name)));
                        }
                        prop_summaries.push(json!({
                            "kind": "directiveProp",
                            "name": name,
                            "dynamicKey": false,
                            "valueConstant": false,
                        }));
                    } else {
                        prop_summaries.push(json!({
                            "kind": "directiveProp",
                            "dynamicKey": true,
                            "valueConstant": false,
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
        "isComponent": false,
        "isDynamicComponent": false,
        "context": vue3_model_suite_transform_context(options, scope),
    }));
    let mut props = if merge_args.is_empty() {
        if properties.is_empty() {
            Value::Null
        } else {
            vue3_for_suite_props_object(properties, node)
        }
    } else {
        vue3_for_suite_push_props_object_arg(&mut merge_args, &mut properties, node);
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
    let patch_flag = (!dynamic_props.is_empty()).then(|| json!(8));
    let dynamic_props = if dynamic_props.is_empty() {
        Value::Null
    } else {
        Value::Array(dynamic_props)
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
    (props, patch_flag, dynamic_props, directives)
}

pub(crate) fn vue3_for_suite_push_props_object_arg(
    merge_args: &mut Vec<Value>,
    properties: &mut Vec<Value>,
    node: &Value,
) {
    if properties.is_empty() {
        return;
    }
    merge_args.push(vue3_for_suite_props_object(
        std::mem::take(properties),
        node,
    ));
}

pub(crate) fn vue3_for_suite_props_object(properties: Vec<Value>, node: &Value) -> Value {
    json!({
        "type": 15,
        "properties": properties,
        "loc": node.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
    })
}

pub(crate) fn vue3_for_suite_ref_for_property() -> Value {
    vue3_once_suite_object_property(
        vue3_once_suite_simple_expression("ref_for", true),
        vue3_once_suite_simple_expression("true", false),
    )
}

pub(crate) fn vue3_for_suite_prepend_props_expression_prop(
    mut props: Value,
    property: Value,
    loc: Value,
) -> Value {
    if props.is_null() || props.is_string() {
        return json!({
            "type": 15,
            "properties": [property],
            "loc": loc,
        });
    }
    if vue3_public_node_type(&props) == Some(15) {
        vue3_for_suite_prepend_object_property(&mut props, property);
        return props;
    }
    if vue3_public_node_type(&props) == Some(14)
        && props.get("callee").and_then(Value::as_str) == Some("MERGE_PROPS")
    {
        if let Some(first) = props
            .get_mut("arguments")
            .and_then(Value::as_array_mut)
            .and_then(|arguments| arguments.first_mut())
        {
            if vue3_public_node_type(first) == Some(15) {
                vue3_for_suite_prepend_object_property(first, property);
                return props;
            }
        }
        if let Some(arguments) = props.get_mut("arguments").and_then(Value::as_array_mut) {
            arguments.insert(
                0,
                json!({
                    "type": 15,
                    "properties": [property],
                    "loc": loc,
                }),
            );
            return props;
        }
    }
    vue3_text_suite_call(
        "MERGE_PROPS",
        vec![
            json!({
                "type": 15,
                "properties": [property],
                "loc": loc,
            }),
            props,
        ],
    )
}

pub(crate) fn vue3_for_suite_prepend_object_property(object: &mut Value, property: Value) {
    let key_name = vue3_model_suite_static_prop_name(property.get("key").unwrap_or(&Value::Null));
    if let Some(properties) = object.get_mut("properties").and_then(Value::as_array_mut) {
        if key_name.is_some_and(|key_name| {
            properties.iter().any(|existing| {
                vue3_model_suite_static_prop_name(existing.get("key").unwrap_or(&Value::Null))
                    .as_deref()
                    == Some(key_name.as_str())
            })
        }) {
            return;
        }
        properties.insert(0, property);
    }
}

pub(crate) fn vue3_for_suite_if_codegen(if_node: &mut Value, key_base: usize) -> Value {
    let branch = if_node
        .get("branches")
        .and_then(Value::as_array)
        .and_then(|branches| branches.first())
        .cloned()
        .unwrap_or(Value::Null);
    let projection = vuec_vue3_core::transform_if_projection(&json!({
        "phase": "branchCodegen",
        "branch": branch,
        "keyIndex": key_base,
    }));
    let mut consequent = branch
        .get("children")
        .and_then(Value::as_array)
        .and_then(|children| children.first())
        .and_then(|child| child.get("codegenNode"))
        .cloned()
        .unwrap_or(Value::Null);
    if projection.get("kind").and_then(Value::as_str) == Some("for") {
        vue3_for_suite_inject_prop(
            &mut consequent,
            vue3_for_suite_branch_key_property(key_base),
        );
    }
    json!({
        "type": 19,
        "test": branch.get("condition").cloned().unwrap_or(Value::Null),
        "consequent": consequent,
        "alternate": vue3_text_suite_call("CREATE_COMMENT", vec![json!("\"v-if\""), json!("true")]),
        "newline": true,
        "loc": branch.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
    })
}

pub(crate) fn vue3_for_suite_key_property(
    projection: Option<&Value>,
    original_node: &Value,
    options: &Vue3CompilerOptions,
    scope: &Vue3ModelSuiteScope,
) -> Option<Value> {
    let value = projection
        .and_then(|projection| projection.get("value"))
        .filter(|value| !value.is_null())?;
    let materialized = vue3_text_suite_materialize_process_projection(value, value);
    let _ = (original_node, options, scope);
    Some(vue3_once_suite_object_property(
        vue3_once_suite_simple_expression("key", true),
        materialized,
    ))
}

pub(crate) fn vue3_for_suite_branch_key_property(index: usize) -> Value {
    vue3_once_suite_object_property(
        vue3_once_suite_simple_expression("key", true),
        json!({
            "type": 4,
            "content": index.to_string(),
            "isStatic": false,
            "constType": 2,
            "loc": vue3_loc_stub_value(),
        }),
    )
}

pub(crate) fn vue3_for_suite_inject_prop(vnode_call: &mut Value, property: Value) {
    if vue3_public_node_type(vnode_call) != Some(13) {
        return;
    }
    if vnode_call.get("props").is_none_or(Value::is_null) {
        vnode_call["props"] = json!({
            "type": 15,
            "properties": [property],
            "loc": vnode_call.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
        });
        return;
    }
    if let Some(properties) = vnode_call
        .get_mut("props")
        .and_then(|props| props.get_mut("properties"))
        .and_then(Value::as_array_mut)
    {
        properties.insert(0, property);
    }
}

pub(crate) fn vue3_for_suite_loop_params(parse_result: &Value) -> Vec<Value> {
    let args = ["value", "key", "index"]
        .into_iter()
        .map(|key| parse_result.get(key).cloned().unwrap_or(Value::Null))
        .collect::<Vec<_>>();
    let Some(last) = args.iter().rposition(|arg| !arg.is_null()) else {
        return Vec::new();
    };
    args.into_iter()
        .take(last + 1)
        .enumerate()
        .map(|(index, arg)| {
            if arg.is_null() {
                vue3_once_suite_simple_expression(&"_".repeat(index + 1), false)
            } else {
                arg
            }
        })
        .collect()
}

pub(crate) fn vue3_for_suite_for_node_payload(for_node: &Value) -> Value {
    json!({
        "source": for_node.get("source").cloned().unwrap_or(Value::Null),
        "children": for_node
            .get("children")
            .and_then(Value::as_array)
            .map(|children| children
                .iter()
                .map(|child| json!({
                    "type": child.get("type").cloned().unwrap_or(Value::Null),
                    "tagType": child.get("tagType").cloned().unwrap_or(Value::Null),
                    "codegenNode": child.get("codegenNode").map(|codegen| json!({
                        "type": codegen.get("type").cloned().unwrap_or(Value::Null),
                        "isBlock": codegen.get("isBlock").cloned().unwrap_or(Value::Bool(false)),
                        "isComponent": codegen.get("isComponent").cloned().unwrap_or(Value::Bool(false)),
                    })).unwrap_or(Value::Null),
                }))
                .collect::<Vec<_>>())
            .unwrap_or_default(),
    })
}

pub(crate) fn vue3_for_suite_slot_outlet_context(
    options: &Vue3CompilerOptions,
    scope: &Vue3ModelSuiteScope,
) -> Value {
    let mut context = vue3_model_suite_transform_context(options, scope);
    context["scopeId"] = options
        .scope_id
        .clone()
        .map(Value::String)
        .unwrap_or(Value::Null);
    context["slotted"] = json!(options.slotted);
    context
}

pub(crate) fn vue3_for_suite_finalize_root(root: &mut Value) {
    vue3_once_suite_set_root_codegen(root);
    root["components"] = json!([]);
    root["directives"] = json!(vue3_text_suite_collect_directives(root));
    root["helpers"] = json!(vue3_for_suite_helpers(root));
    root["hoists"] = json!([]);
    root["cached"] = json!([]);
    root["temps"] = json!(0);
}

pub(crate) fn vue3_for_suite_helpers(root: &Value) -> Vec<String> {
    let mut used = Vec::new();
    vue3_for_suite_collect_helpers(root.get("codegenNode").unwrap_or(&Value::Null), &mut used);
    if !vue3_text_suite_collect_directives(root).is_empty() {
        vue3_text_suite_add_helper(&mut used, "RESOLVE_DIRECTIVE");
    }
    [
        "RENDER_LIST",
        "FRAGMENT",
        "OPEN_BLOCK",
        "CREATE_ELEMENT_BLOCK",
        "TO_DISPLAY_STRING",
        "CREATE_ELEMENT_VNODE",
        "RENDER_SLOT",
        "CREATE_COMMENT",
        "RESOLVE_DIRECTIVE",
        "WITH_DIRECTIVES",
    ]
    .into_iter()
    .filter(|helper| used.iter().any(|used| *used == *helper))
    .map(str::to_string)
    .collect()
}

pub(crate) fn vue3_for_suite_collect_helpers(node: &Value, used: &mut Vec<&'static str>) {
    match vue3_public_node_type(node) {
        Some(5) => vue3_text_suite_add_helper(used, "TO_DISPLAY_STRING"),
        Some(13) => {
            if node.get("tag").and_then(Value::as_str) == Some("FRAGMENT") {
                vue3_text_suite_add_helper(used, "FRAGMENT");
            }
            if node.get("directives").is_some_and(|value| !value.is_null()) {
                vue3_text_suite_add_helper(used, "WITH_DIRECTIVES");
            }
            if node
                .get("isBlock")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                vue3_text_suite_add_helper(used, "OPEN_BLOCK");
                vue3_text_suite_add_helper(used, "CREATE_ELEMENT_BLOCK");
            } else {
                vue3_text_suite_add_helper(used, "CREATE_ELEMENT_VNODE");
            }
        }
        Some(14) => match node.get("callee").and_then(Value::as_str) {
            Some("RENDER_LIST") => vue3_text_suite_add_helper(used, "RENDER_LIST"),
            Some("RENDER_SLOT") => vue3_text_suite_add_helper(used, "RENDER_SLOT"),
            Some("CREATE_COMMENT") => vue3_text_suite_add_helper(used, "CREATE_COMMENT"),
            _ => {}
        },
        _ => {}
    }
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
        let Some(value) = node.get(key) else {
            continue;
        };
        if let Some(items) = value.as_array() {
            for item in items {
                vue3_for_suite_collect_helpers(item, used);
            }
        } else if value.is_object() {
            vue3_for_suite_collect_helpers(value, used);
        }
    }
}
