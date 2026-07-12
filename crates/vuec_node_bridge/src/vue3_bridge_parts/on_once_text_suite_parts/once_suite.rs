#[derive(Default)]
pub(crate) struct Vue3OnceSuiteState {
    pub(crate) cached: usize,
}

pub(crate) fn vue3_core_transform_once_suite_value(payload: &Value) -> Value {
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
    let mut state = Vue3OnceSuiteState::default();
    if let Some(children) = root.get_mut("children").and_then(Value::as_array_mut) {
        let transformed = vue3_once_suite_transform_children(
            std::mem::take(children),
            &options,
            &mut state,
            false,
        );
        *children = transformed;
    }
    vue3_once_suite_finalize_root(&mut root, &state);
    root
}

pub(crate) fn vue3_once_suite_transform_children(
    children: Vec<Value>,
    options: &Vue3CompilerOptions,
    state: &mut Vue3OnceSuiteState,
    in_v_once: bool,
) -> Vec<Value> {
    let mut transformed = Vec::new();
    let mut index = 0usize;
    while index < children.len() {
        let child = children[index].clone();
        if vue3_public_node_type(&child) == Some(1)
            && vue3_text_suite_directive(&child, "if").is_some()
        {
            let (if_node, consumed) =
                vue3_once_suite_transform_if_node(&children[index..], options, state, in_v_once);
            transformed.push(if_node);
            index += consumed.max(1);
            continue;
        }
        transformed.push(vue3_once_suite_transform_node(
            child, options, state, in_v_once,
        ));
        index += 1;
    }
    transformed
}

pub(crate) fn vue3_once_suite_transform_node(
    mut node: Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3OnceSuiteState,
    in_v_once: bool,
) -> Value {
    if vue3_public_node_type(&node) == Some(1) && vue3_text_suite_directive(&node, "for").is_some()
    {
        return vue3_once_suite_transform_for_node(node, options, state, in_v_once);
    }

    let once_projection = vue3_once_suite_once_projection(&node, in_v_once);
    let enters_once = once_projection.get("kind").and_then(Value::as_str) == Some("enter");
    let child_in_v_once = in_v_once || enters_once;

    if let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) {
        let transformed = vue3_once_suite_transform_children(
            std::mem::take(children),
            options,
            state,
            child_in_v_once,
        );
        *children = transformed;
    }

    if vue3_public_node_type(&node) == Some(1) {
        node["codegenNode"] = vue3_once_suite_element_codegen(&node, false);
    }

    if enters_once {
        let codegen = node.get("codegenNode").cloned().unwrap_or(Value::Null);
        node["codegenNode"] = vue3_once_suite_cache_expression(state, codegen);
    }
    node
}

pub(crate) fn vue3_once_suite_transform_if_node(
    siblings: &[Value],
    options: &Vue3CompilerOptions,
    state: &mut Vue3OnceSuiteState,
    in_v_once: bool,
) -> (Value, usize) {
    let first = siblings.first().cloned().unwrap_or(Value::Null);
    let once_projection = vue3_once_suite_once_projection(&first, in_v_once);
    let enters_once = once_projection.get("kind").and_then(Value::as_str) == Some("enter");
    let branch_in_v_once = in_v_once || enters_once;
    let mut branches = Vec::new();
    let mut consumed = 0usize;

    for sibling in siblings {
        let dir_name = if consumed == 0 {
            "if"
        } else if vue3_text_suite_directive(sibling, "else-if").is_some() {
            "else-if"
        } else if vue3_text_suite_directive(sibling, "else").is_some() {
            "else"
        } else {
            break;
        };
        let dir = vue3_text_suite_directive(sibling, dir_name);
        if consumed > 0 && dir.is_none() {
            break;
        }
        let mut branch_child = sibling.clone();
        vue3_text_suite_remove_directive(&mut branch_child, "if");
        vue3_text_suite_remove_directive(&mut branch_child, "else-if");
        vue3_text_suite_remove_directive(&mut branch_child, "else");
        let transformed_child =
            vue3_once_suite_transform_node(branch_child, options, state, branch_in_v_once);
        let condition = dir
            .and_then(|dir| dir.get("exp"))
            .filter(|value| !value.is_null())
            .cloned()
            .unwrap_or(Value::Null);
        branches.push(json!({
            "type": 10,
            "condition": condition,
            "children": [transformed_child],
            "userKey": Value::Null,
            "isTemplateIf": false,
            "loc": sibling.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
        }));
        consumed += 1;
        if dir_name == "else" {
            break;
        }
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
    if_node["codegenNode"] = vue3_once_suite_if_codegen(&if_node);
    if enters_once {
        let codegen = if_node.get("codegenNode").cloned().unwrap_or(Value::Null);
        if_node["codegenNode"] = vue3_once_suite_cache_expression(state, codegen);
    }
    (if_node, consumed)
}

pub(crate) fn vue3_once_suite_if_codegen(if_node: &Value) -> Value {
    let branches = if_node
        .get("branches")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut alternate =
        vue3_text_suite_call("CREATE_COMMENT", vec![json!("\"v-if\""), json!("true")]);
    for branch in branches.iter().rev() {
        let child_codegen = branch
            .get("children")
            .and_then(Value::as_array)
            .and_then(|children| children.first())
            .and_then(|child| child.get("codegenNode"))
            .cloned()
            .unwrap_or(Value::Null);
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

pub(crate) fn vue3_once_suite_transform_for_node(
    mut node: Value,
    options: &Vue3CompilerOptions,
    state: &mut Vue3OnceSuiteState,
    in_v_once: bool,
) -> Value {
    let Some(dir) = vue3_text_suite_directive(&node, "for").cloned() else {
        return vue3_once_suite_transform_node(node, options, state, in_v_once);
    };
    let once_projection = vue3_once_suite_once_projection(&node, in_v_once);
    let enters_once = once_projection.get("kind").and_then(Value::as_str) == Some("enter");
    let child_in_v_once = in_v_once || enters_once;
    let context = vue3_text_suite_transform_context(options);
    let projection = vuec_vue3_core::transform_for_projection(&json!({
        "node": node,
        "dir": dir,
        "context": context,
    }));
    let parse_result = projection
        .get("parseResult")
        .filter(|value| !value.is_null())
        .map(vue3_text_suite_materialize_for_parse_result)
        .unwrap_or_else(|| {
            dir.get("forParseResult")
                .map(vue3_text_suite_materialize_for_parse_result)
                .unwrap_or(Value::Null)
        });

    vue3_text_suite_remove_directive(&mut node, "for");
    let child = vue3_once_suite_transform_node(node, options, state, child_in_v_once);
    let loc = dir
        .get("loc")
        .cloned()
        .or_else(|| child.get("loc").cloned())
        .unwrap_or_else(vue3_loc_stub_value);
    let mut for_node = json!({
        "type": 11,
        "source": parse_result.get("source").cloned().unwrap_or(Value::Null),
        "valueAlias": parse_result.get("value").cloned().unwrap_or(Value::Null),
        "keyAlias": parse_result.get("key").cloned().unwrap_or(Value::Null),
        "objectIndexAlias": parse_result.get("index").cloned().unwrap_or(Value::Null),
        "parseResult": parse_result,
        "children": [child],
        "codegenNode": Value::Null,
        "loc": loc,
    });
    for_node["codegenNode"] = vue3_text_suite_for_codegen(&for_node);
    if enters_once {
        let codegen = for_node.get("codegenNode").cloned().unwrap_or(Value::Null);
        for_node["codegenNode"] = vue3_once_suite_cache_expression(state, codegen);
    }
    for_node
}

pub(crate) fn vue3_once_suite_once_projection(node: &Value, in_v_once: bool) -> Value {
    vuec_vue3_core::transform_once_projection(&json!({
        "node": node,
        "context": {
            "inVOnce": in_v_once,
            "inSSR": false,
        },
        "seen": false,
    }))
}

pub(crate) fn vue3_once_suite_cache_expression(
    state: &mut Vue3OnceSuiteState,
    value: Value,
) -> Value {
    let index = state.cached;
    state.cached += 1;
    json!({
        "type": 20,
        "index": index,
        "value": value,
        "needPauseTracking": true,
        "inVOnce": true,
        "needArraySpread": false,
        "loc": vue3_loc_stub_value(),
    })
}

pub(crate) fn vue3_once_suite_element_codegen(node: &Value, is_block: bool) -> Value {
    match (
        vue3_public_node_type(node),
        node.get("tagType").and_then(Value::as_u64),
    ) {
        (Some(1), Some(2)) => {
            vue3_text_suite_call("RENDER_SLOT", vec![json!("$slots"), json!("\"default\"")])
        }
        (Some(1), Some(1)) => {
            let tag = node.get("tag").and_then(Value::as_str).unwrap_or("");
            let (props, patch_flag, dynamic_props) = vue3_once_suite_props_codegen(node);
            vue3_once_suite_vnode_call(
                &vue3_once_suite_component_asset_id(tag),
                props,
                Value::Null,
                patch_flag,
                dynamic_props,
                is_block,
                false,
                true,
            )
        }
        (Some(1), Some(0)) => {
            let (props, patch_flag, dynamic_props) = vue3_once_suite_props_codegen(node);
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
            vue3_once_suite_vnode_call(
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
            )
        }
        _ => Value::Null,
    }
}

pub(crate) fn vue3_once_suite_props_codegen(node: &Value) -> (Value, Option<Value>, Value) {
    let mut properties = Vec::new();
    let mut dynamic_props = Vec::new();
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
            }
            Some(7) if prop.get("name").and_then(Value::as_str) == Some("bind") => {
                let Some(arg) = prop.get("arg").filter(|value| !value.is_null()) else {
                    continue;
                };
                let Some(name) = arg.get("content").and_then(Value::as_str) else {
                    continue;
                };
                let value = prop
                    .get("exp")
                    .filter(|value| !value.is_null())
                    .cloned()
                    .unwrap_or_else(|| vue3_once_suite_simple_expression(name, false));
                properties.push(vue3_once_suite_object_property(arg.clone(), value));
                dynamic_props.push(Value::String(vue3_once_suite_quote_string(name)));
            }
            _ => {}
        }
    }
    let props = if properties.is_empty() {
        Value::Null
    } else {
        json!({
            "type": 15,
            "properties": properties,
            "loc": node.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
        })
    };
    let patch_flag = (!dynamic_props.is_empty()).then(|| json!(8));
    let dynamic_props = if dynamic_props.is_empty() {
        Value::Null
    } else {
        Value::Array(dynamic_props)
    };
    (props, patch_flag, dynamic_props)
}

pub(crate) fn vue3_once_suite_vnode_call(
    tag: &str,
    props: Value,
    children: Value,
    patch_flag: Option<Value>,
    dynamic_props: Value,
    is_block: bool,
    disable_tracking: bool,
    is_component: bool,
) -> Value {
    json!({
        "type": 13,
        "tag": tag,
        "props": props,
        "children": children,
        "patchFlag": patch_flag.unwrap_or(Value::Null),
        "dynamicProps": dynamic_props,
        "directives": Value::Null,
        "isBlock": is_block,
        "disableTracking": disable_tracking,
        "isComponent": is_component,
        "loc": vue3_loc_stub_value(),
    })
}

pub(crate) fn vue3_once_suite_object_property(key: Value, value: Value) -> Value {
    json!({
        "type": 16,
        "key": key,
        "value": value,
        "loc": vue3_loc_stub_value(),
    })
}

pub(crate) fn vue3_once_suite_simple_expression(content: &str, is_static: bool) -> Value {
    json!({
        "type": 4,
        "content": content,
        "isStatic": is_static,
        "constType": if is_static { 3 } else { 0 },
        "loc": vue3_loc_stub_value(),
    })
}

pub(crate) fn vue3_once_suite_quote_string(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

pub(crate) fn vue3_once_suite_component_asset_id(tag: &str) -> String {
    let mut id = String::from("_component_");
    for ch in tag.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            id.push(ch);
        } else {
            id.push('_');
        }
    }
    id
}

pub(crate) fn vue3_once_suite_finalize_root(root: &mut Value, state: &Vue3OnceSuiteState) {
    vue3_once_suite_set_root_codegen(root);
    root["components"] = json!(vue3_once_suite_components(root));
    root["helpers"] = json!(vue3_once_suite_helpers(root));
    root["directives"] = json!([]);
    root["hoists"] = json!([]);
    root["cached"] = Value::Array((0..state.cached).map(|_| Value::Null).collect());
    root["temps"] = json!(0);
}

pub(crate) fn vue3_once_suite_set_root_codegen(root: &mut Value) {
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
    root["codegenNode"] = match visible.as_slice() {
        [] => Value::Null,
        [index] => {
            if let Some(root_child) = root
                .get_mut("children")
                .and_then(Value::as_array_mut)
                .and_then(|items| items.get_mut(*index))
            {
                if vue3_public_node_type(root_child) == Some(1)
                    && root_child
                        .get("codegenNode")
                        .and_then(vue3_public_node_type)
                        == Some(13)
                {
                    root_child["codegenNode"]["isBlock"] = json!(true);
                }
            }
            root.get("children")
                .and_then(Value::as_array)
                .and_then(|items| items.get(*index))
                .and_then(|child| {
                    if matches!(vue3_public_node_type(child), Some(1 | 9 | 11)) {
                        child.get("codegenNode")
                    } else {
                        Some(child)
                    }
                })
                .cloned()
                .unwrap_or(Value::Null)
        }
        _ => vue3_once_suite_vnode_call(
            "FRAGMENT",
            Value::Null,
            Value::Array(children),
            Some(json!(64)),
            Value::Null,
            true,
            false,
            false,
        ),
    };
}

pub(crate) fn vue3_once_suite_helpers(root: &Value) -> Vec<String> {
    let mut used = Vec::new();
    vue3_once_suite_collect_helpers(root.get("codegenNode").unwrap_or(&Value::Null), &mut used);
    if root
        .get("components")
        .and_then(Value::as_array)
        .is_some_and(|components| !components.is_empty())
    {
        vue3_text_suite_add_helper(&mut used, "RESOLVE_COMPONENT");
    }
    [
        "SET_BLOCK_TRACKING",
        "RESOLVE_COMPONENT",
        "CREATE_VNODE",
        "RENDER_SLOT",
        "CREATE_ELEMENT_VNODE",
        "RENDER_LIST",
        "FRAGMENT",
        "OPEN_BLOCK",
        "CREATE_ELEMENT_BLOCK",
        "CREATE_COMMENT",
    ]
    .into_iter()
    .filter(|helper| used.iter().any(|used| used == helper))
    .map(str::to_string)
    .collect()
}

pub(crate) fn vue3_once_suite_collect_helpers(node: &Value, used: &mut Vec<&'static str>) {
    match vue3_public_node_type(node) {
        Some(13) => {
            if node.get("tag").and_then(Value::as_str) == Some("FRAGMENT") {
                vue3_text_suite_add_helper(used, "FRAGMENT");
            }
            if node
                .get("isBlock")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                vue3_text_suite_add_helper(used, "OPEN_BLOCK");
                vue3_text_suite_add_helper(used, "CREATE_ELEMENT_BLOCK");
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
            Some("RENDER_SLOT") => vue3_text_suite_add_helper(used, "RENDER_SLOT"),
            Some("RENDER_LIST") => vue3_text_suite_add_helper(used, "RENDER_LIST"),
            Some("CREATE_COMMENT") => vue3_text_suite_add_helper(used, "CREATE_COMMENT"),
            _ => {}
        },
        Some(20) => {
            vue3_text_suite_add_helper(used, "SET_BLOCK_TRACKING");
        }
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
                vue3_once_suite_collect_helpers(item, used);
            }
        } else if value.is_object() {
            vue3_once_suite_collect_helpers(value, used);
        }
    }
}

pub(crate) fn vue3_once_suite_components(root: &Value) -> Vec<String> {
    let mut components = Vec::new();
    vue3_once_suite_collect_components(root, &mut components);
    components
}

pub(crate) fn vue3_slot_suite_components(root: &Value) -> Vec<String> {
    let mut components = Vec::new();
    vue3_slot_suite_collect_components(root, &mut components);
    components
}

pub(crate) fn vue3_slot_suite_collect_components(node: &Value, components: &mut Vec<String>) {
    for key in ["children", "branches"] {
        let Some(value) = node.get(key) else {
            continue;
        };
        if let Some(items) = value.as_array() {
            for item in items {
                vue3_slot_suite_collect_components(item, components);
            }
        }
    }
    if vue3_public_node_type(node) == Some(1)
        && node.get("tagType").and_then(Value::as_u64) == Some(1)
    {
        if let Some(tag) = node.get("tag").and_then(Value::as_str) {
            if !components.iter().any(|component| component == tag) {
                components.push(tag.to_string());
            }
        }
    }
}

pub(crate) fn vue3_once_suite_collect_components(node: &Value, components: &mut Vec<String>) {
    if vue3_public_node_type(node) == Some(1)
        && node.get("tagType").and_then(Value::as_u64) == Some(1)
    {
        if let Some(tag) = node.get("tag").and_then(Value::as_str) {
            if !components.iter().any(|component| component == tag) {
                components.push(tag.to_string());
            }
        }
    }
    for key in ["children", "branches"] {
        let Some(value) = node.get(key) else {
            continue;
        };
        if let Some(items) = value.as_array() {
            for item in items {
                vue3_once_suite_collect_components(item, components);
            }
        }
    }
}
