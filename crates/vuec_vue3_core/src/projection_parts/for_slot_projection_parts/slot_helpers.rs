pub(crate) fn vue3_for_parse_result_projection(
    node: &Value,
    dir: &Value,
    context: &Value,
) -> Value {
    transform_for_projection(&json!({
        "node": node,
        "dir": dir,
        "context": context,
    }))
}

pub(crate) fn vue3_slot_for_parse_result_projection(
    node: &Value,
    dir: &Value,
    context: &Value,
) -> Value {
    if let Some(parse_result) = dir.get("forParseResult").filter(|value| !value.is_null()) {
        return json!({
            "parseResult": {
                "source": parse_result.get("source").cloned().unwrap_or(Value::Null),
                "value": parse_result.get("value").cloned().unwrap_or(Value::Null),
                "key": parse_result.get("key").cloned().unwrap_or(Value::Null),
                "index": parse_result.get("index").cloned().unwrap_or(Value::Null),
                "finalized": parse_result.get("finalized").and_then(Value::as_bool).unwrap_or(true),
            }
        });
    }
    vue3_for_parse_result_projection(node, dir, context)
}

pub(crate) fn vue3_directive<'a>(
    node: &'a Value,
    name: &str,
    allow_empty: bool,
) -> Option<&'a Value> {
    node.get("props")
        .and_then(Value::as_array)?
        .iter()
        .find(|prop| {
            json_node_type(prop) == Some(7)
                && json_str(prop, "name") == Some(name)
                && (allow_empty || prop.get("exp").is_some_and(|exp| !exp.is_null()))
        })
}

pub(crate) fn vue3_slot_directive(node: &Value, allow_empty: bool) -> Option<&Value> {
    vue3_directive(node, "slot", allow_empty)
}

pub(crate) fn vue3_template_slot_directive(node: &Value) -> Option<&Value> {
    if json_node_type(node) == Some(1) && json_u64(node, "tagType") == Some(3) {
        vue3_slot_directive(node, true)
    } else {
        None
    }
}

pub(crate) fn vue3_else_slot_directive(node: &Value) -> Option<&Value> {
    node.get("props")
        .and_then(Value::as_array)?
        .iter()
        .find(|prop| {
            json_node_type(prop) == Some(7)
                && matches!(json_str(prop, "name"), Some("else") | Some("else-if"))
        })
}

pub(crate) fn vue3_template_has_if_like_slot_directive(node: &Value) -> bool {
    vue3_template_slot_directive(node).is_some()
        && node
            .get("props")
            .and_then(Value::as_array)
            .is_some_and(|props| {
                props.iter().any(|prop| {
                    json_node_type(prop) == Some(7)
                        && matches!(json_str(prop, "name"), Some("if") | Some("else-if"))
                })
            })
}

pub(crate) fn vue3_previous_non_comment_or_whitespace(
    children: &[Value],
    index: usize,
) -> Option<&Value> {
    children[..index]
        .iter()
        .rev()
        .find(|child| !vue3_is_comment_or_whitespace(child))
}

pub(crate) fn vue3_is_comment_or_whitespace(node: &Value) -> bool {
    json_node_type(node) == Some(3) || vue3_is_whitespace_text(node)
}

pub(crate) fn vue3_is_whitespace_text(node: &Value) -> bool {
    match json_node_type(node) {
        Some(2) => json_str(node, "content").is_some_and(|content| {
            content
                .chars()
                .all(|ch| matches!(ch, '\t' | '\r' | '\n' | '\u{000C}' | ' '))
        }),
        Some(12) => node.get("content").is_some_and(vue3_is_whitespace_text),
        _ => false,
    }
}

pub(crate) fn vue3_all_indices_are_whitespace_text(children: &[Value], indices: &[usize]) -> bool {
    indices
        .iter()
        .filter_map(|index| children.get(*index))
        .all(vue3_is_whitespace_text)
}

pub(crate) fn vue3_all_child_indices(children: &[Value]) -> Vec<usize> {
    (0..children.len()).collect()
}

pub(crate) fn vue3_slot_name_projection(slot: &Value, context: &Value) -> Value {
    let Some(arg) = slot.get("arg").filter(|arg| !arg.is_null()) else {
        return vue3_static_slot_key("default");
    };
    if json_bool(arg, "isStatic") {
        return vue3_static_slot_key(json_str(arg, "content").unwrap_or("default"));
    }
    let _ = context;
    arg.clone()
}

pub(crate) fn vue3_static_slot_name(slot: &Value) -> Option<String> {
    let Some(arg) = slot.get("arg").filter(|arg| !arg.is_null()) else {
        return Some("default".to_string());
    };
    json_bool(arg, "isStatic").then(|| json_str(arg, "content").unwrap_or("default").to_string())
}

pub(crate) fn vue3_static_slot_key(name: &str) -> Value {
    json!({
        "kind": "simple",
        "content": name,
        "isStatic": true,
        "constType": 3,
    })
}

pub(crate) fn vue3_slot_param_locals(exp: &Value) -> Vec<String> {
    let source = model_expression_source(exp);
    vue3_for_alias_locals(source.trim())
}

pub(crate) fn vue3_slot_condition_projection(dir: &Value, context: &Value) -> Value {
    let Some(exp) = dir.get("exp").filter(|exp| !exp.is_null()) else {
        return json!({ "kind": "undefined" });
    };
    let _ = context;
    exp.clone()
}

pub(crate) fn vue3_slot_function_projection(
    slot_dir: &Value,
    indices: &[usize],
    child: &Value,
) -> Value {
    json!({
        "kind": "slotFunction",
        "params": slot_dir.get("exp").cloned().unwrap_or(Value::Null),
        "indices": indices,
        "unwrapTemplate": true,
        "loc": child.get("loc").cloned().unwrap_or(Value::Null),
    })
}

pub(crate) fn vue3_dynamic_slot_projection(name: Value, slot: Value, key: Option<usize>) -> Value {
    let mut value = json!({
        "kind": "dynamicSlot",
        "name": name,
        "slot": slot,
    });
    if let Some(key) = key {
        value["key"] = json!(key.to_string());
    }
    value
}

pub(crate) fn vue3_default_fallback_projection() -> Value {
    json!({
        "kind": "simple",
        "content": "undefined",
        "isStatic": false,
        "constType": 0,
    })
}

pub(crate) fn vue3_append_slot_conditional_alternate(
    dynamic_slots: &mut [Value],
    alternate: Value,
) {
    let Some(last) = dynamic_slots.last_mut() else {
        return;
    };
    let mut target = last;
    loop {
        let nested = target
            .get("alternate")
            .and_then(|value| value.get("kind"))
            .and_then(Value::as_str)
            == Some("conditional");
        if !nested {
            target["alternate"] = alternate;
            break;
        }
        target = target.get_mut("alternate").expect("checked alternate");
    }
}

pub(crate) fn vue3_slot_flag_text(flag: u8) -> &'static str {
    match flag {
        1 => "STABLE",
        2 => "DYNAMIC",
        3 => "FORWARDED",
        _ => "",
    }
}

pub(crate) fn vue3_has_forwarded_slots(children: &[Value]) -> bool {
    children.iter().any(|child| match json_node_type(child) {
        Some(1) => {
            json_u64(child, "tagType") == Some(2)
                || child
                    .get("children")
                    .and_then(Value::as_array)
                    .is_some_and(|children| vue3_has_forwarded_slots(children))
        }
        Some(9) => child
            .get("branches")
            .and_then(Value::as_array)
            .is_some_and(|branches| vue3_has_forwarded_slots(branches)),
        Some(10) | Some(11) => child
            .get("children")
            .and_then(Value::as_array)
            .is_some_and(|children| vue3_has_forwarded_slots(children)),
        _ => false,
    })
}

pub(crate) fn vue3_component_slot_scope_ref(
    node: &Value,
    children: &[Value],
    context: &Value,
) -> bool {
    let mut names = transform_context_locals(context);
    if let Some(slot) = vue3_slot_directive(node, false) {
        if let Some(exp) = slot.get("exp").filter(|exp| !exp.is_null()) {
            let slot_locals = vue3_slot_param_locals(exp);
            names.retain(|name| !slot_locals.iter().any(|local| local == name));
        }
    }
    if names.is_empty() {
        return false;
    }
    node.get("props")
        .and_then(Value::as_array)
        .is_some_and(|props| {
            props.iter().any(|prop| {
                json_str(prop, "name") == Some("slot")
                    && (prop
                        .get("arg")
                        .is_some_and(|arg| vue3_node_source_contains_any(arg, &names))
                        || prop
                            .get("exp")
                            .is_some_and(|exp| vue3_node_source_contains_any(exp, &names)))
            })
        })
        || children
            .iter()
            .any(|child| vue3_node_source_contains_any(child, &names))
}

pub(crate) fn vue3_node_source_contains_any(node: &Value, names: &[String]) -> bool {
    if node.is_null() {
        return false;
    }
    match json_node_type(node) {
        Some(1) => {
            if node
                .get("props")
                .and_then(Value::as_array)
                .is_some_and(|props| {
                    props.iter().any(|prop| {
                        json_node_type(prop) == Some(7)
                            && (prop
                                .get("arg")
                                .is_some_and(|arg| vue3_node_source_contains_any(arg, names))
                                || prop
                                    .get("exp")
                                    .is_some_and(|exp| vue3_node_source_contains_any(exp, names)))
                    })
                })
            {
                return true;
            }
            node.get("children")
                .and_then(Value::as_array)
                .is_some_and(|children| {
                    children
                        .iter()
                        .any(|child| vue3_node_source_contains_any(child, names))
                })
        }
        Some(11) => {
            if node
                .get("source")
                .is_some_and(|source| vue3_node_source_contains_any(source, names))
            {
                return true;
            }
            node.get("children")
                .and_then(Value::as_array)
                .is_some_and(|children| {
                    children
                        .iter()
                        .any(|child| vue3_node_source_contains_any(child, names))
                })
        }
        Some(9) => node
            .get("branches")
            .and_then(Value::as_array)
            .is_some_and(|branches| {
                branches
                    .iter()
                    .any(|branch| vue3_node_source_contains_any(branch, names))
            }),
        Some(10) => {
            if node
                .get("condition")
                .is_some_and(|condition| vue3_node_source_contains_any(condition, names))
            {
                return true;
            }
            node.get("children")
                .and_then(Value::as_array)
                .is_some_and(|children| {
                    children
                        .iter()
                        .any(|child| vue3_node_source_contains_any(child, names))
                })
        }
        Some(4) => {
            let content = json_str(node, "content").unwrap_or("");
            !json_bool(node, "isStatic")
                && (names
                    .iter()
                    .any(|name| source_contains_identifier(content, name))
                    || node
                        .get("loc")
                        .and_then(|loc| json_str(loc, "source"))
                        .is_some_and(|source| {
                            names
                                .iter()
                                .any(|name| source_contains_identifier(source, name))
                        }))
        }
        Some(8) => node
            .get("children")
            .and_then(Value::as_array)
            .is_some_and(|children| {
                children
                    .iter()
                    .filter(|child| child.is_object())
                    .any(|child| vue3_node_source_contains_any(child, names))
            }),
        Some(5) | Some(12) => node
            .get("content")
            .is_some_and(|content| vue3_node_source_contains_any(content, names)),
        Some(2) | Some(3) | Some(20) => false,
        _ => false,
    }
}

pub(crate) fn source_contains_identifier(source: &str, name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let mut search_start = 0usize;
    while let Some(offset) = source[search_start..].find(name) {
        let start = search_start + offset;
        let end = start + name.len();
        let before = source[..start].chars().next_back();
        let after = source[end..].chars().next();
        if before.is_none_or(|ch| !is_identifier_continue(ch))
            && after.is_none_or(|ch| !is_identifier_continue(ch))
        {
            return true;
        }
        search_start = end;
    }
    false
}
