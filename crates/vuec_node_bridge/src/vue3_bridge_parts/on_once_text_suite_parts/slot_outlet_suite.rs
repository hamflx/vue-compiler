#[derive(Default)]
pub(crate) struct Vue3SlotOutletSuiteState {
    pub(crate) errors: Vec<Value>,
}

pub(crate) fn vue3_core_transform_slot_outlet_suite_value(payload: &Value) -> Value {
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
    let mut state = Vue3SlotOutletSuiteState::default();
    root = vue3_slot_outlet_suite_transform_node(root, &options, &mut state);
    root["helpers"] = json!(vue3_slot_outlet_suite_helpers(&root));
    root["components"] = json!([]);
    root["directives"] = json!([]);
    root["hoists"] = json!([]);
    root["cached"] = json!([]);
    root["temps"] = json!(0);
    root["__vuecErrors"] = json!(state.errors);
    root
}

pub(crate) fn vue3_slot_outlet_suite_transform_node(
    mut node: Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3SlotOutletSuiteState,
) -> Value {
    if vue3_public_node_type(&node) == Some(1) {
        vue3_slot_outlet_suite_process_directive_expressions(&mut node, options);
    }

    if let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) {
        let transformed = std::mem::take(children)
            .into_iter()
            .map(|child| vue3_slot_outlet_suite_transform_node(child, options, state))
            .collect::<Vec<_>>();
        *children = transformed;
    }

    if vue3_public_node_type(&node) == Some(1)
        && node.get("tagType").and_then(Value::as_u64) == Some(2)
    {
        let context = vue3_slot_outlet_suite_transform_context(options);
        let projection = vuec_vue3_core::transform_slot_outlet_projection(&json!({
            "node": node,
            "context": context,
        }));
        vue3_slot_outlet_suite_apply_mutations(&mut node, projection.get("process"));
        let non_name_props = projection
            .get("process")
            .and_then(|process| process.get("nonNameProps"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let slot_props =
            vue3_slot_outlet_suite_props_codegen(&node, &non_name_props, options, state);
        let slot_name = vue3_slot_outlet_suite_slot_name(&node, projection.get("process"));
        let codegen = projection.get("codegen").unwrap_or(&Value::Null);
        node["codegenNode"] = vue3_slot_outlet_suite_codegen(&node, slot_name, slot_props, codegen);
    }
    node
}

pub(crate) fn vue3_slot_outlet_suite_process_directive_expressions(
    node: &mut Value,
    options: &Vue3CompilerOptions,
) {
    if !options.prefix_identifiers {
        return;
    }
    let Some(props) = node.get_mut("props").and_then(Value::as_array_mut) else {
        return;
    };
    let context = vue3_slot_outlet_suite_transform_context(options);
    for prop in props {
        if vue3_public_node_type(prop) != Some(7) {
            continue;
        }
        for key in ["exp", "arg"] {
            let Some(current) = prop.get(key).filter(|value| !value.is_null()).cloned() else {
                continue;
            };
            if vue3_public_node_type(&current) != Some(4) {
                continue;
            }
            let projection = vuec_vue3_core::process_expression_projection(&json!({
                "node": current,
                "context": context,
            }));
            prop[key] = vue3_text_suite_materialize_process_projection(&projection, &current);
        }
    }
}

pub(crate) fn vue3_slot_outlet_suite_apply_mutations(node: &mut Value, process: Option<&Value>) {
    let mutations = process
        .and_then(|process| process.get("mutations"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let Some(props) = node.get_mut("props").and_then(Value::as_array_mut) else {
        return;
    };
    for mutation in mutations {
        let Some(index) = mutation.get("index").and_then(Value::as_u64) else {
            continue;
        };
        let Some(prop) = props.get_mut(index as usize) else {
            continue;
        };
        match mutation.get("kind").and_then(Value::as_str) {
            Some("setPropName") => {
                if let Some(name) = mutation.get("name").and_then(Value::as_str) {
                    prop["name"] = json!(name);
                }
            }
            Some("setDirectiveArgContent") => {
                if let Some(content) = mutation.get("content").and_then(Value::as_str) {
                    prop["arg"]["content"] = json!(content);
                }
            }
            Some("setDirectiveExp") => {
                let value = mutation.get("value").cloned().unwrap_or(Value::Null);
                prop["exp"] = vue3_text_suite_materialize_process_projection(&value, &value);
            }
            _ => {}
        }
    }
}

pub(crate) fn vue3_slot_outlet_suite_slot_name(node: &Value, process: Option<&Value>) -> Value {
    let slot_name = process
        .and_then(|process| process.get("slotName"))
        .unwrap_or(&Value::Null);
    match slot_name.get("kind").and_then(Value::as_str) {
        Some("literal") => slot_name.get("value").cloned().unwrap_or(Value::Null),
        Some("node") => {
            let index = slot_name.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
            let field = slot_name
                .get("field")
                .and_then(Value::as_str)
                .unwrap_or("exp");
            node.get("props")
                .and_then(Value::as_array)
                .and_then(|props| props.get(index))
                .and_then(|prop| prop.get(field))
                .cloned()
                .unwrap_or(Value::Null)
        }
        _ => json!("\"default\""),
    }
}

pub(crate) fn vue3_slot_outlet_suite_props_codegen(
    node: &Value,
    indices: &[Value],
    options: &Vue3CompilerOptions,
    state: &mut Vue3SlotOutletSuiteState,
) -> Value {
    let mut properties = Vec::new();
    let props = node
        .get("props")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    for index in indices
        .iter()
        .filter_map(Value::as_u64)
        .map(|index| index as usize)
    {
        let Some(prop) = props.get(index) else {
            continue;
        };
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
            }
            Some(7) if prop.get("name").and_then(Value::as_str) == Some("bind") => {
                let key = prop.get("arg").cloned().unwrap_or(Value::Null);
                let value = prop
                    .get("exp")
                    .filter(|value| !value.is_null())
                    .cloned()
                    .unwrap_or_else(|| {
                        let content = key
                            .get("content")
                            .and_then(Value::as_str)
                            .unwrap_or_default();
                        vue3_once_suite_simple_expression(content, false)
                    });
                properties.push(vue3_once_suite_object_property(key, value));
            }
            Some(7) if prop.get("name").and_then(Value::as_str) == Some("on") => {
                let projection = vuec_vue3_core::transform_on_projection(&json!({
                    "dir": prop,
                    "node": node,
                    "context": vue3_slot_outlet_suite_transform_context(options),
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
                    let value =
                        vue3_on_suite_materialize_projection(projected_prop.get("value"), prop);
                    properties.push(vue3_once_suite_object_property(key, value));
                }
            }
            Some(7) => {
                state.errors.push(json!({
                    "code": 36,
                    "loc": prop.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
                }));
            }
            _ => {}
        }
    }
    if properties.is_empty() {
        Value::Null
    } else {
        json!({
            "type": 15,
            "properties": properties,
            "loc": node.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
        })
    }
}

pub(crate) fn vue3_slot_outlet_suite_codegen(
    node: &Value,
    slot_name: Value,
    slot_props: Value,
    codegen: &Value,
) -> Value {
    let slots = codegen
        .get("slots")
        .and_then(Value::as_str)
        .unwrap_or("$slots");
    let mut args = vec![
        Value::String(slots.to_string()),
        slot_name,
        Value::String("{}".to_string()),
        Value::String("undefined".to_string()),
        Value::String("true".to_string()),
    ];
    let mut expected_len = codegen
        .get("expectedLen")
        .and_then(Value::as_u64)
        .unwrap_or(2) as usize;
    if !slot_props.is_null() {
        args[2] = slot_props;
        expected_len = expected_len.max(3);
    }
    if node
        .get("children")
        .and_then(Value::as_array)
        .is_some_and(|children| !children.is_empty())
    {
        args[3] = json!({
            "type": 18,
            "params": [],
            "returns": node.get("children").cloned().unwrap_or_else(|| json!([])),
            "newline": false,
            "isSlot": false,
            "loc": node.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
        });
        expected_len = expected_len.max(4);
    }
    args.truncate(expected_len);
    vue3_text_suite_call("RENDER_SLOT", args)
}

pub(crate) fn vue3_slot_outlet_suite_helpers(root: &Value) -> Vec<String> {
    let mut used = Vec::new();
    vue3_slot_outlet_suite_collect_helpers(root, &mut used);
    ["RENDER_SLOT"]
        .into_iter()
        .filter(|helper| used.iter().any(|used| used == helper))
        .map(str::to_string)
        .collect()
}

pub(crate) fn vue3_slot_outlet_suite_collect_helpers(node: &Value, used: &mut Vec<&'static str>) {
    if vue3_public_node_type(node) == Some(14)
        && node.get("callee").and_then(Value::as_str) == Some("RENDER_SLOT")
    {
        vue3_text_suite_add_helper(used, "RENDER_SLOT");
    }
    for key in [
        "children",
        "content",
        "codegenNode",
        "arguments",
        "returns",
        "params",
        "props",
        "value",
        "key",
        "properties",
    ] {
        let Some(value) = node.get(key) else {
            continue;
        };
        if let Some(items) = value.as_array() {
            for item in items {
                vue3_slot_outlet_suite_collect_helpers(item, used);
            }
        } else if value.is_object() {
            vue3_slot_outlet_suite_collect_helpers(value, used);
        }
    }
}

pub(crate) fn vue3_slot_outlet_suite_transform_context(options: &Vue3CompilerOptions) -> Value {
    let mut context = vue3_text_suite_transform_context(options);
    context["scopeId"] = options
        .scope_id
        .clone()
        .map(Value::String)
        .unwrap_or(Value::Null);
    context["slotted"] = json!(options.slotted);
    context
}
