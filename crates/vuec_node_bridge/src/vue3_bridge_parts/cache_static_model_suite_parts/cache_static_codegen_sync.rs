pub(crate) fn vue3_cache_static_suite_set_root_codegen(root: &mut Value) {
    let children = root
        .get("children")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let visible = children
        .iter()
        .enumerate()
        .filter(|(_, child)| vue3_public_node_type(child) != Some(3))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let has_comments = children
        .iter()
        .any(|child| vue3_public_node_type(child) == Some(3));
    if has_comments && visible.len() == 1 && children.len() > 1 {
        root["codegenNode"] = vue3_once_suite_vnode_call(
            "FRAGMENT",
            Value::Null,
            Value::Array(children),
            Some(json!(2112)),
            Value::Null,
            true,
            false,
            false,
        );
        return;
    }
    vue3_once_suite_set_root_codegen(root);
}

pub(crate) fn vue3_cache_static_suite_sync_public_codegen_refs(node: &mut Value) {
    match vue3_public_node_type(node) {
        Some(0) => {
            if let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) {
                for child in children {
                    vue3_cache_static_suite_sync_public_codegen_refs(child);
                }
            }
        }
        Some(1) => {
            if let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) {
                for child in children {
                    vue3_cache_static_suite_sync_public_codegen_refs(child);
                }
            }
            vue3_cache_static_suite_sync_element_codegen(node);
        }
        Some(9) => {
            if let Some(branches) = node.get_mut("branches").and_then(Value::as_array_mut) {
                for branch in branches {
                    vue3_cache_static_suite_sync_public_codegen_refs(branch);
                }
            }
            vue3_cache_static_suite_sync_if_codegen(node);
        }
        Some(10) => {
            if let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) {
                for child in children {
                    vue3_cache_static_suite_sync_public_codegen_refs(child);
                }
            }
        }
        Some(11) => {
            if let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) {
                for child in children {
                    vue3_cache_static_suite_sync_public_codegen_refs(child);
                }
            }
            vue3_cache_static_suite_sync_for_codegen(node);
        }
        _ => {}
    }
}

pub(crate) fn vue3_cache_static_suite_sync_element_codegen(node: &mut Value) {
    let children = node.get("children").cloned().unwrap_or_else(|| json!([]));
    let tag_type = node.get("tagType").and_then(Value::as_u64);
    if tag_type == Some(0) {
        if let Some(codegen) = node.get_mut("codegenNode") {
            if vue3_public_node_type(codegen) == Some(13)
                && codegen.get("children").and_then(Value::as_array).is_some()
            {
                codegen["children"] = children;
            }
        }
        return;
    }
    if tag_type == Some(1) {
        vue3_cache_static_suite_sync_component_slot_returns(node);
    }
}

pub(crate) fn vue3_cache_static_suite_sync_component_slot_returns(node: &mut Value) {
    let children = node
        .get("children")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let Some(codegen_children) = node
        .get_mut("codegenNode")
        .and_then(|codegen| codegen.get_mut("children"))
    else {
        return;
    };
    vue3_cache_static_suite_sync_slot_object_returns(codegen_children, &children);
    if vue3_public_node_type(codegen_children) == Some(14)
        && codegen_children.get("callee").and_then(Value::as_str) == Some("CREATE_SLOTS")
    {
        if let Some(arguments) = codegen_children
            .get_mut("arguments")
            .and_then(Value::as_array_mut)
        {
            if let Some(base_slots) = arguments.get_mut(0) {
                vue3_cache_static_suite_sync_slot_object_returns(base_slots, &children);
            }
        }
    }
}

pub(crate) fn vue3_cache_static_suite_sync_slot_object_returns(
    slots: &mut Value,
    children: &[Value],
) {
    if vue3_public_node_type(slots) != Some(15) {
        return;
    }
    let Some(properties) = slots.get_mut("properties").and_then(Value::as_array_mut) else {
        return;
    };
    for property in properties {
        let key = property.get("key").cloned().unwrap_or(Value::Null);
        if key.get("content").and_then(Value::as_str) == Some("_") {
            continue;
        }
        let Some(returns) = property
            .get_mut("value")
            .and_then(|value| value.get_mut("returns"))
        else {
            continue;
        };
        if !returns.is_array() {
            continue;
        }
        if key.get("content").and_then(Value::as_str) == Some("default")
            && key
                .get("isStatic")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        {
            *returns = Value::Array(
                children
                    .iter()
                    .filter(|child| !vue3_slot_suite_is_template_slot(child))
                    .cloned()
                    .collect(),
            );
            continue;
        }
        if let Some(template) = children
            .iter()
            .find(|child| vue3_cache_static_suite_template_slot_matches(child, &key))
        {
            *returns = template
                .get("children")
                .cloned()
                .unwrap_or_else(|| json!([]));
        }
    }
}

pub(crate) fn vue3_cache_static_suite_template_slot_matches(template: &Value, key: &Value) -> bool {
    if !vue3_slot_suite_is_template_slot(template) {
        return false;
    }
    let Some(slot) = vue3_text_suite_directive(template, "slot") else {
        return false;
    };
    let arg = slot.get("arg").unwrap_or(&Value::Null);
    if key
        .get("isStatic")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return arg
            .get("isStatic")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            && key.get("content").and_then(Value::as_str)
                == arg.get("content").and_then(Value::as_str);
    }
    key == arg
}

pub(crate) fn vue3_cache_static_suite_sync_if_codegen(node: &mut Value) {
    let branches = node
        .get("branches")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if branches.is_empty() {
        return;
    }
    let current = node.get("codegenNode").cloned().unwrap_or(Value::Null);
    node["codegenNode"] = vue3_cache_static_suite_sync_if_codegen_node(&current, &branches, 0);
}

pub(crate) fn vue3_cache_static_suite_sync_if_codegen_node(
    current: &Value,
    branches: &[Value],
    index: usize,
) -> Value {
    let Some(branch) = branches.get(index) else {
        return current.clone();
    };
    if vue3_public_node_type(current) == Some(19) {
        let mut next = current.clone();
        next["consequent"] = vue3_cache_static_suite_branch_codegen(
            next.get("consequent").unwrap_or(&Value::Null),
            branch,
        );
        let alternate = next.get("alternate").cloned().unwrap_or(Value::Null);
        next["alternate"] =
            vue3_cache_static_suite_sync_if_codegen_node(&alternate, branches, index + 1);
        return next;
    }
    vue3_cache_static_suite_branch_codegen(current, branch)
}

pub(crate) fn vue3_cache_static_suite_branch_codegen(existing: &Value, branch: &Value) -> Value {
    let children = branch
        .get("children")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if children.len() == 1 {
        return children[0]
            .get("codegenNode")
            .cloned()
            .unwrap_or_else(|| children[0].clone());
    }
    if vue3_public_node_type(existing) == Some(13)
        && existing.get("tag").and_then(Value::as_str) == Some("FRAGMENT")
    {
        let mut next = existing.clone();
        next["children"] = Value::Array(children);
        return next;
    }
    existing.clone()
}

pub(crate) fn vue3_cache_static_suite_sync_for_codegen(node: &mut Value) {
    let returns = vue3_cache_static_suite_for_returns(node);
    let Some(returns) = returns else {
        return;
    };
    let Some(function) = node
        .get_mut("codegenNode")
        .and_then(|codegen| codegen.get_mut("children"))
        .and_then(|children| children.get_mut("arguments"))
        .and_then(Value::as_array_mut)
        .and_then(|arguments| arguments.get_mut(1))
    else {
        return;
    };
    let existing = function.get("returns").cloned().unwrap_or(Value::Null);
    function["returns"] = vue3_cache_static_suite_merge_for_returns(&existing, returns);
}

pub(crate) fn vue3_cache_static_suite_for_returns(node: &Value) -> Option<Value> {
    let children = node.get("children").and_then(Value::as_array)?;
    if children.len() == 1 {
        return Some(
            children[0]
                .get("codegenNode")
                .cloned()
                .unwrap_or_else(|| children[0].clone()),
        );
    }
    Some(Value::Array(children.clone()))
}

pub(crate) fn vue3_cache_static_suite_merge_for_returns(existing: &Value, updated: Value) -> Value {
    if vue3_public_node_type(existing) != Some(13) || vue3_public_node_type(&updated) != Some(13) {
        return updated;
    }
    let mut merged = updated;
    if let Some(is_block) = existing.get("isBlock").and_then(Value::as_bool) {
        merged["isBlock"] = json!(is_block);
    }
    if let Some(disable_tracking) = existing.get("disableTracking").and_then(Value::as_bool) {
        merged["disableTracking"] = json!(disable_tracking);
    }
    if let Some(key) = existing
        .get("props")
        .and_then(vue3_cache_static_suite_vnode_key_property)
    {
        vue3_cache_static_suite_inject_key_if_missing(&mut merged, key);
    }
    merged
}

pub(crate) fn vue3_cache_static_suite_vnode_key_property(props: &Value) -> Option<Value> {
    props
        .get("properties")
        .and_then(Value::as_array)?
        .iter()
        .find(|property| {
            property
                .get("key")
                .and_then(vue3_model_suite_static_prop_name)
                .as_deref()
                == Some("key")
        })
        .cloned()
}

pub(crate) fn vue3_cache_static_suite_inject_key_if_missing(vnode: &mut Value, key: Value) {
    if vue3_public_node_type(vnode) != Some(13) {
        return;
    }
    if vnode
        .get("props")
        .is_some_and(|props| vue3_cache_static_suite_vnode_key_property(props).is_some())
    {
        return;
    }
    vue3_for_suite_inject_prop(vnode, key);
}
