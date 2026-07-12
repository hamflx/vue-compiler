pub(crate) fn vue3_slot_suite_finalize_root(root: &mut Value, state: &Vue3SlotSuiteState) {
    vue3_once_suite_set_root_codegen(root);
    root["components"] = json!(vue3_slot_suite_components(root));
    root["directives"] = json!(vue3_if_suite_collect_directives(root));
    root["helpers"] = json!(vue3_slot_suite_helpers(root));
    root["hoists"] = json!([]);
    root["cached"] = Value::Array((0..state.cached).map(|_| Value::Null).collect());
    root["temps"] = json!(0);
}

pub(crate) fn vue3_transform_suite_finalize_root(root: &mut Value, state: &Vue3SlotSuiteState) {
    vue3_transform_suite_set_root_codegen(root);
    root["components"] = json!(vue3_slot_suite_components(root));
    root["directives"] = json!(vue3_if_suite_collect_directives(root));
    root["helpers"] = json!(vue3_slot_suite_helpers(root));
    root["hoists"] = json!([]);
    root["cached"] = Value::Array((0..state.cached).map(|_| Value::Null).collect());
    root["temps"] = json!(0);
}

pub(crate) fn vue3_transform_suite_set_root_codegen(root: &mut Value) {
    let children = root
        .get("children")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    root["codegenNode"] = match children.as_slice() {
        [] => Value::Null,
        [_] => vue3_transform_suite_single_root_codegen(root),
        _ => vue3_once_suite_vnode_call(
            "FRAGMENT",
            Value::Null,
            Value::Array(children.clone()),
            Some(json!(vue3_transform_suite_root_fragment_patch_flag(
                &children
            ))),
            Value::Null,
            true,
            false,
            false,
        ),
    };
}

pub(crate) fn vue3_transform_suite_single_root_codegen(root: &mut Value) -> Value {
    let Some(child) = root
        .get_mut("children")
        .and_then(Value::as_array_mut)
        .and_then(|children| children.first_mut())
    else {
        return Value::Null;
    };
    if vue3_transform_suite_is_single_element_root_child(child)
        && child
            .get("codegenNode")
            .is_some_and(|codegen| !codegen.is_null())
    {
        if child.get("codegenNode").and_then(vue3_public_node_type) == Some(13) {
            child["codegenNode"]["isBlock"] = json!(true);
        }
        return child.get("codegenNode").cloned().unwrap_or(Value::Null);
    }
    child.clone()
}

pub(crate) fn vue3_transform_suite_is_single_element_root_child(child: &Value) -> bool {
    vue3_public_node_type(child) == Some(1)
        && child.get("tagType").and_then(Value::as_u64) != Some(2)
}

pub(crate) fn vue3_transform_suite_root_fragment_patch_flag(children: &[Value]) -> u16 {
    if children
        .iter()
        .filter(|child| vue3_public_node_type(child) != Some(3))
        .count()
        == 1
        && children
            .iter()
            .any(|child| vue3_public_node_type(child) == Some(3))
    {
        64 | 2048
    } else {
        64
    }
}

pub(crate) fn vue3_slot_suite_helpers(root: &Value) -> Vec<String> {
    let mut used = Vec::new();
    vue3_slot_suite_collect_helpers(root, &mut used);
    vue3_slot_suite_collect_helpers(root.get("codegenNode").unwrap_or(&Value::Null), &mut used);
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
    [
        "TO_DISPLAY_STRING",
        "CREATE_ELEMENT_VNODE",
        "CREATE_TEXT",
        "CREATE_COMMENT",
        "RESOLVE_COMPONENT",
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
    ]
    .into_iter()
    .filter(|helper| used.iter().any(|used| used == helper))
    .map(str::to_string)
    .collect()
}

pub(crate) fn vue3_slot_suite_collect_helpers(node: &Value, used: &mut Vec<&'static str>) {
    match vue3_public_node_type(node) {
        Some(3) => vue3_text_suite_add_helper(used, "CREATE_COMMENT"),
        Some(5) => vue3_text_suite_add_helper(used, "TO_DISPLAY_STRING"),
        Some(12) => {
            if let Some(codegen) = node.get("codegenNode") {
                vue3_slot_suite_collect_helpers(codegen, used);
            }
            if let Some(content) = node.get("content") {
                vue3_slot_suite_collect_helpers(content, used);
            }
        }
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
            Some("CREATE_TEXT") => vue3_text_suite_add_helper(used, "CREATE_TEXT"),
            Some("CREATE_COMMENT") => vue3_text_suite_add_helper(used, "CREATE_COMMENT"),
            Some("RENDER_LIST") => vue3_text_suite_add_helper(used, "RENDER_LIST"),
            Some("CREATE_SLOTS") => vue3_text_suite_add_helper(used, "CREATE_SLOTS"),
            Some("RENDER_SLOT") => vue3_text_suite_add_helper(used, "RENDER_SLOT"),
            Some("MERGE_PROPS") => vue3_text_suite_add_helper(used, "MERGE_PROPS"),
            Some("NORMALIZE_PROPS") => vue3_text_suite_add_helper(used, "NORMALIZE_PROPS"),
            Some("NORMALIZE_CLASS") => vue3_text_suite_add_helper(used, "NORMALIZE_CLASS"),
            Some("NORMALIZE_STYLE") => vue3_text_suite_add_helper(used, "NORMALIZE_STYLE"),
            Some("GUARD_REACTIVE_PROPS") => {
                vue3_text_suite_add_helper(used, "GUARD_REACTIVE_PROPS")
            }
            Some("TO_HANDLERS") => vue3_text_suite_add_helper(used, "TO_HANDLERS"),
            Some("TO_HANDLER_KEY") => vue3_text_suite_add_helper(used, "TO_HANDLER_KEY"),
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
        Some(18) if node.get("isSlot").and_then(Value::as_bool).unwrap_or(false) => {
            vue3_text_suite_add_helper(used, "WITH_CTX");
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
        "hoists",
        "cached",
    ] {
        let Some(value) = node.get(key) else {
            continue;
        };
        if let Some(items) = value.as_array() {
            for item in items {
                vue3_slot_suite_collect_helpers(item, used);
            }
        } else if value.is_object() {
            vue3_slot_suite_collect_helpers(value, used);
        }
    }
}
