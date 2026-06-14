pub(crate) fn transform_for_codegen_projection(payload: &Value) -> Value {
    let node = payload.get("node").unwrap_or(&Value::Null);
    let for_node = payload.get("forNode").unwrap_or(&Value::Null);
    let context = payload.get("context").unwrap_or(&Value::Null);
    let source = for_node.get("source").unwrap_or(&Value::Null);
    let is_stable_fragment = json_node_type(source) == Some(4)
        && json_u64(source, "constType").is_some_and(|value| value > 0);
    let key_projection = vue3_for_key_property_projection(node, context);
    json!({
        "keyProperty": key_projection,
        "fragmentFlag": if is_stable_fragment {
            64
        } else if !key_projection.is_null() {
            128
        } else {
            256
        },
        "disableTracking": !is_stable_fragment,
        "isStableFragment": is_stable_fragment,
    })
}

pub(crate) fn transform_for_exit_codegen_projection(payload: &Value) -> Value {
    let node = payload.get("node").unwrap_or(&Value::Null);
    let for_node = payload.get("forNode").unwrap_or(&Value::Null);
    let children = for_node
        .get("children")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    if vue3_for_is_slot_outlet_summary(node) {
        return json!({ "kind": "slotOutlet", "path": "node" });
    }
    if json_u64(node, "tagType") == Some(3)
        && node
            .get("children")
            .and_then(Value::as_array)
            .is_some_and(|children| {
                children.len() == 1 && vue3_for_is_slot_outlet_summary(&children[0])
            })
    {
        return json!({ "kind": "slotOutlet", "path": "templateChild", "index": 0 });
    }
    let need_fragment_wrapper =
        children.len() != 1 || children.first().and_then(json_node_type) != Some(1);
    if need_fragment_wrapper {
        return json!({ "kind": "fragmentWrapper", "patchFlag": 64 });
    }
    json!({
        "kind": "singleElement",
        "childBlockIsBlock": !json_bool(payload, "isStableFragment"),
    })
}

pub(crate) fn vue3_for_is_slot_outlet_summary(node: &Value) -> bool {
    json_node_type(node) == Some(1) && json_u64(node, "tagType") == Some(2)
}

pub(crate) fn vue3_for_key_property_projection(node: &Value, context: &Value) -> Value {
    let Some((prop, is_directive)) = vue3_for_key_prop(node) else {
        return Value::Null;
    };
    let value = if is_directive {
        let Some(exp) = prop.get("exp").filter(|value| !value.is_null()) else {
            return Value::Null;
        };
        let raw = json_str(exp, "content")
            .or_else(|| exp.get("loc").and_then(|loc| json_str(loc, "source")))
            .unwrap_or("");
        if json_bool(context, "prefixIdentifiers") {
            let options = vue3_options_from_transform_context(context);
            let locals = transform_context_locals(context);
            vue3_for_rewrite_projection_node(
                raw,
                &options,
                &locals,
                exp.get("loc").cloned().unwrap_or(Value::Null),
                Vue3ForAstMode::Expression,
                false,
            )
        } else {
            vue3_for_expression_projection(raw, exp, 0, raw.len(), Vue3ForAstMode::Expression)
        }
    } else {
        let Some(value) = prop.get("value").filter(|value| !value.is_null()) else {
            return Value::Null;
        };
        let content = json_str(value, "content").unwrap_or("");
        json!({
            "kind": "simple",
            "content": content,
            "isStatic": true,
            "constType": 3,
            "loc": value.get("loc").cloned().unwrap_or_else(|| prop.get("loc").cloned().unwrap_or(Value::Null)),
            "astMode": "expression",
        })
    };
    json!({ "value": value })
}

pub(crate) fn vue3_for_key_prop(node: &Value) -> Option<(&Value, bool)> {
    node.get("props")
        .and_then(Value::as_array)?
        .iter()
        .find_map(|prop| match json_node_type(prop) {
            Some(6) if json_str(prop, "name") == Some("key") => Some((prop, false)),
            Some(7)
                if json_str(prop, "name") == Some("bind")
                    && prop.get("arg").is_some_and(|arg| {
                        json_str(arg, "content") == Some("key") && json_bool(arg, "isStatic")
                    }) =>
            {
                Some((prop, true))
            }
            _ => None,
        })
}
