pub(crate) fn vue3_slot_suite_slot_outlet_codegen(
    node: &Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3SlotSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> Value {
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
    let slot_props =
        vue3_slot_outlet_suite_props_codegen(node, &non_name_props, options, &mut slot_state);
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

pub(crate) fn vue3_slot_suite_build_slots(
    node: &Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3SlotSuiteState,
    scope: &Vue3ModelSuiteScope,
) -> Value {
    let projection = vuec_vue3_core::build_slots_projection(&json!({
        "node": node,
        "context": vue3_model_suite_transform_context(options, scope),
    }));
    for error in projection
        .get("errors")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        state.errors.push(json!({
            "code": error.get("code").cloned().unwrap_or(json!(0)),
            "loc": error.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
        }));
    }
    vue3_slot_suite_materialize_slots_projection(&projection, node)
}

pub(crate) fn vue3_slot_suite_materialize_slots_projection(
    projection: &Value,
    node: &Value,
) -> Value {
    let mut properties = Vec::<Value>::new();
    for property in projection
        .get("properties")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        properties.push(vue3_once_suite_object_property(
            vue3_slot_suite_projection_node(property.get("key").unwrap_or(&Value::Null), node),
            vue3_slot_suite_slot_function(property, node),
        ));
    }
    let slot_flag = projection
        .get("slotFlag")
        .and_then(Value::as_u64)
        .unwrap_or(1);
    let slot_flag_text = projection
        .get("slotFlagText")
        .and_then(Value::as_str)
        .unwrap_or(match slot_flag {
            2 => "DYNAMIC",
            3 => "FORWARDED",
            _ => "STABLE",
        });
    properties.push(vue3_once_suite_object_property(
        vue3_once_suite_simple_expression("_", true),
        vue3_once_suite_simple_expression(&format!("{slot_flag} /* {slot_flag_text} */"), false),
    ));
    let base = json!({
        "type": 15,
        "properties": properties,
        "loc": node.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
    });
    let dynamic_slots = projection
        .get("dynamicSlots")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    if dynamic_slots.is_empty() {
        return base;
    }
    vue3_text_suite_call(
        "CREATE_SLOTS",
        vec![
            base,
            json!({
                "type": 17,
                "elements": dynamic_slots
                    .iter()
                    .map(|slot| vue3_slot_suite_dynamic_slot(slot, node))
                    .collect::<Vec<_>>(),
                "loc": node.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
            }),
        ],
    )
}

pub(crate) fn vue3_slot_suite_slot_function(property: &Value, node: &Value) -> Value {
    let returns = vue3_slot_suite_slot_children(property, node);
    json!({
        "type": 18,
        "params": vue3_slot_suite_projection_node(property.get("params").unwrap_or(&Value::Null), node),
        "returns": returns,
        "newline": false,
        "isSlot": true,
        "loc": property
            .get("loc")
            .cloned()
            .or_else(|| node.get("loc").cloned())
            .unwrap_or_else(vue3_loc_stub_value),
    })
}

pub(crate) fn vue3_slot_suite_slot_children(property: &Value, node: &Value) -> Value {
    let mut out = Vec::<Value>::new();
    let unwrap_template = property
        .get("unwrapTemplate")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    for index in property
        .get("indices")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_u64)
        .map(|index| index as usize)
    {
        let Some(child) = node
            .get("children")
            .and_then(Value::as_array)
            .and_then(|children| children.get(index))
        else {
            continue;
        };
        if unwrap_template
            && vue3_public_node_type(child) == Some(1)
            && child.get("tag").and_then(Value::as_str) == Some("template")
        {
            out.extend(
                child
                    .get("children")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default(),
            );
        } else {
            out.push(child.clone());
        }
    }
    Value::Array(out)
}

pub(crate) fn vue3_slot_suite_dynamic_slot(projection: &Value, node: &Value) -> Value {
    match projection.get("kind").and_then(Value::as_str) {
        Some("conditional") => json!({
            "type": 19,
            "test": vue3_slot_suite_projection_node(projection.get("test").unwrap_or(&Value::Null), node),
            "consequent": vue3_slot_suite_dynamic_slot(projection.get("consequent").unwrap_or(&Value::Null), node),
            "alternate": vue3_slot_suite_dynamic_slot(projection.get("alternate").unwrap_or(&Value::Null), node),
            "newline": true,
            "loc": node.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
        }),
        Some("for") => {
            let params = projection.get("params").unwrap_or(&Value::Null);
            vue3_text_suite_call(
                "RENDER_LIST",
                vec![
                    vue3_slot_suite_projection_node(
                        projection.get("source").unwrap_or(&Value::Null),
                        node,
                    ),
                    json!({
                        "type": 18,
                        "params": vue3_slot_suite_loop_params(params, node),
                        "returns": vue3_slot_suite_dynamic_slot(projection.get("slot").unwrap_or(&Value::Null), node),
                        "newline": true,
                        "isSlot": false,
                        "loc": node.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
                    }),
                ],
            )
        }
        Some("dynamicSlot") => {
            let mut properties = vec![
                vue3_once_suite_object_property(
                    vue3_once_suite_simple_expression("name", true),
                    vue3_slot_suite_projection_node(
                        projection.get("name").unwrap_or(&Value::Null),
                        node,
                    ),
                ),
                vue3_once_suite_object_property(
                    vue3_once_suite_simple_expression("fn", true),
                    vue3_slot_suite_slot_function(
                        projection.get("slot").unwrap_or(&Value::Null),
                        node,
                    ),
                ),
            ];
            if let Some(key) = projection.get("key").and_then(Value::as_str) {
                properties.push(vue3_once_suite_object_property(
                    vue3_once_suite_simple_expression("key", true),
                    vue3_once_suite_simple_expression(key, true),
                ));
            }
            json!({
                "type": 15,
                "properties": properties,
                "loc": node.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
            })
        }
        Some("simple") | Some("compound") | None => {
            vue3_slot_suite_projection_node(projection, node)
        }
        _ => vue3_slot_suite_projection_node(projection, node),
    }
}

pub(crate) fn vue3_slot_suite_loop_params(params: &Value, node: &Value) -> Vec<Value> {
    let args = ["value", "key", "index"]
        .into_iter()
        .map(|key| {
            params
                .get(key)
                .map(|value| vue3_slot_suite_projection_node(value, node))
                .unwrap_or(Value::Null)
        })
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

pub(crate) fn vue3_slot_suite_projection_node(projection: &Value, node: &Value) -> Value {
    if projection.is_null()
        || projection
            .get("kind")
            .and_then(Value::as_str)
            .is_some_and(|kind| kind == "undefined" || kind == "unchanged")
    {
        return Value::Null;
    }
    if projection.is_string() || projection.get("type").is_some() {
        return projection.clone();
    }
    match projection.get("kind").and_then(Value::as_str) {
        Some("simple") => json!({
            "type": 4,
            "content": projection.get("content").and_then(Value::as_str).unwrap_or(""),
            "isStatic": projection.get("isStatic").and_then(Value::as_bool).unwrap_or(false),
            "constType": projection.get("constType").and_then(Value::as_u64).unwrap_or(0),
            "loc": projection
                .get("loc")
                .cloned()
                .or_else(|| node.get("loc").cloned())
                .unwrap_or_else(vue3_loc_stub_value),
        }),
        Some("compound") => json!({
            "type": 8,
            "children": projection
                .get("children")
                .and_then(Value::as_array)
                .map(|children| children
                    .iter()
                    .map(|child| vue3_slot_suite_projection_node(child, node))
                    .collect::<Vec<_>>())
                .unwrap_or_default(),
            "loc": projection
                .get("loc")
                .cloned()
                .or_else(|| node.get("loc").cloned())
                .unwrap_or_else(vue3_loc_stub_value),
        }),
        _ => Value::Null,
    }
}

pub(crate) fn vue3_slot_suite_is_template_slot(node: &Value) -> bool {
    vue3_public_node_type(node) == Some(1)
        && node.get("tagType").and_then(Value::as_u64) == Some(3)
        && vue3_text_suite_directive(node, "slot").is_some()
}
