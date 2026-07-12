pub(crate) fn vue3_core_transform_text_suite_value(payload: &Value) -> Value {
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
    root = vue3_text_suite_transform_node(root, &options);
    vue3_text_suite_finalize_root(&mut root);
    root
}

pub(crate) fn vue3_text_suite_transform_node(
    mut node: Value,
    options: &Vue3CompilerOptions,
) -> Value {
    if vue3_public_node_type(&node) == Some(1) && vue3_text_suite_directive(&node, "for").is_some()
    {
        return vue3_text_suite_transform_for_node(node, options);
    }

    if let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) {
        let transformed = std::mem::take(children)
            .into_iter()
            .map(|child| vue3_text_suite_transform_node(child, options))
            .collect::<Vec<_>>();
        *children = transformed;
    }

    if vue3_public_node_type(&node) == Some(5) {
        vue3_text_suite_process_interpolation(&mut node, options);
    }

    if matches!(vue3_public_node_type(&node), Some(0 | 1 | 10 | 11)) {
        vue3_text_suite_apply_transform_text(&mut node, options);
    }

    if vue3_public_node_type(&node) == Some(1) {
        node["codegenNode"] = vue3_text_suite_element_codegen(&node, false);
    }

    node
}

pub(crate) fn vue3_text_suite_transform_for_node(
    mut node: Value,
    options: &Vue3CompilerOptions,
) -> Value {
    let Some(dir) = vue3_text_suite_directive(&node, "for").cloned() else {
        return node;
    };
    let context = vue3_text_suite_transform_context(options);
    let projection = vuec_vue3_core::transform_for_projection(&json!({
        "node": node,
        "dir": dir,
        "context": context,
    }));
    let Some(parse_result) = projection
        .get("parseResult")
        .filter(|value| !value.is_null())
        .map(vue3_text_suite_materialize_for_parse_result)
    else {
        return node;
    };

    let fallback_loc = node.get("loc").cloned();
    let children = if projection.get("children").and_then(Value::as_str) == Some("template") {
        node.get_mut("children")
            .and_then(Value::as_array_mut)
            .map(std::mem::take)
            .unwrap_or_default()
            .into_iter()
            .map(|child| vue3_text_suite_transform_node(child, options))
            .collect::<Vec<_>>()
    } else {
        vue3_text_suite_remove_directive(&mut node, "for");
        vec![vue3_text_suite_transform_node(node, options)]
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
    vue3_text_suite_apply_transform_text(&mut for_node, options);
    for_node["codegenNode"] = vue3_text_suite_for_codegen(&for_node);
    for_node
}

pub(crate) fn vue3_text_suite_materialize_for_parse_result(parse_result: &Value) -> Value {
    let source = parse_result
        .get("source")
        .map(vue3_text_suite_materialize_process_child)
        .unwrap_or(Value::Null);
    let value = parse_result
        .get("value")
        .filter(|value| !value.is_null())
        .map(vue3_text_suite_materialize_process_child)
        .unwrap_or(Value::Null);
    let key = parse_result
        .get("key")
        .filter(|value| !value.is_null())
        .map(vue3_text_suite_materialize_process_child)
        .unwrap_or(Value::Null);
    let index = parse_result
        .get("index")
        .filter(|value| !value.is_null())
        .map(vue3_text_suite_materialize_process_child)
        .unwrap_or(Value::Null);
    json!({
        "source": source,
        "value": value,
        "key": key,
        "index": index,
        "finalized": parse_result
            .get("finalized")
            .and_then(Value::as_bool)
            .unwrap_or(true),
    })
}

pub(crate) fn vue3_text_suite_process_interpolation(
    node: &mut Value,
    options: &Vue3CompilerOptions,
) {
    if !options.prefix_identifiers {
        return;
    }
    let content = node.get("content").cloned().unwrap_or(Value::Null);
    let projection = vuec_vue3_core::process_expression_projection(&json!({
        "node": content,
        "context": vue3_text_suite_transform_context(options),
    }));
    node["content"] = vue3_text_suite_materialize_process_projection(&projection, &content);
}

pub(crate) fn vue3_text_suite_materialize_process_projection(
    projection: &Value,
    current: &Value,
) -> Value {
    match projection.get("kind").and_then(Value::as_str) {
        Some("simple") => json!({
            "type": 4,
            "content": projection.get("content").and_then(Value::as_str).unwrap_or(""),
            "isStatic": projection.get("isStatic").and_then(Value::as_bool).unwrap_or(false),
            "constType": projection.get("constType").and_then(Value::as_u64).unwrap_or(0),
            "loc": projection
                .get("loc")
                .cloned()
                .or_else(|| current.get("loc").cloned())
                .unwrap_or_else(vue3_loc_stub_value),
        }),
        Some("compound") => json!({
            "type": 8,
            "children": projection
                .get("children")
                .and_then(Value::as_array)
                .map(|children| children
                    .iter()
                    .map(vue3_text_suite_materialize_process_child)
                    .collect::<Vec<_>>())
                .unwrap_or_default(),
            "loc": projection
                .get("loc")
                .cloned()
                .or_else(|| current.get("loc").cloned())
                .unwrap_or_else(vue3_loc_stub_value),
        }),
        Some("setConstType") => {
            let mut next = current.clone();
            next["constType"] = projection.get("constType").cloned().unwrap_or(json!(0));
            next
        }
        _ => current.clone(),
    }
}

pub(crate) fn vue3_text_suite_materialize_process_child(child: &Value) -> Value {
    if child.is_string() || child.get("type").is_some() {
        return child.clone();
    }
    match child.get("kind").and_then(Value::as_str) {
        Some("simple") => json!({
            "type": 4,
            "content": child.get("content").and_then(Value::as_str).unwrap_or(""),
            "isStatic": child.get("isStatic").and_then(Value::as_bool).unwrap_or(false),
            "constType": child.get("constType").and_then(Value::as_u64).unwrap_or(0),
            "loc": child.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
        }),
        Some("compound") => json!({
            "type": 8,
            "children": child
                .get("children")
                .and_then(Value::as_array)
                .map(|children| children
                    .iter()
                    .map(vue3_text_suite_materialize_process_child)
                    .collect::<Vec<_>>())
                .unwrap_or_default(),
            "loc": child.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
        }),
        _ => child.clone(),
    }
}

pub(crate) fn vue3_text_suite_apply_transform_text(
    node: &mut Value,
    options: &Vue3CompilerOptions,
) {
    let context = vue3_text_suite_transform_context(options);
    vue3_text_suite_apply_transform_text_with_context(node, context);
}

pub(crate) fn vue3_text_suite_apply_transform_text_with_directives(
    node: &mut Value,
    options: &Vue3CompilerOptions,
    directive_transforms: &[&str],
) {
    let context = vue3_text_suite_transform_context_with_directives(options, directive_transforms);
    vue3_text_suite_apply_transform_text_with_context(node, context);
}

pub(crate) fn vue3_text_suite_apply_transform_text_with_context(node: &mut Value, context: Value) {
    let projection = vuec_vue3_core::transform_text_projection(&json!({
        "node": node,
        "context": context,
    }));
    let operations = projection
        .get("operations")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) else {
        return;
    };
    for operation in operations {
        match operation.get("kind").and_then(Value::as_str) {
            Some("mergeText") => {
                let start = operation.get("start").and_then(Value::as_u64).unwrap_or(0) as usize;
                let end = operation
                    .get("end")
                    .and_then(Value::as_u64)
                    .unwrap_or(start as u64) as usize;
                if start >= children.len() || end < start || end >= children.len() {
                    continue;
                }
                let compound = vue3_text_suite_compound(&children[start..=end]);
                children.splice(start..=end, std::iter::once(compound));
            }
            Some("wrapTextCall") => {
                let index = operation.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
                if index >= children.len() {
                    continue;
                }
                let child = children[index].clone();
                let include_content = operation
                    .get("includeContent")
                    .and_then(Value::as_bool)
                    .unwrap_or(true);
                let patch_flag = operation.get("patchFlag").cloned().filter(|value| {
                    !value.is_null() && value.as_str().is_none_or(|value| !value.trim().is_empty())
                });
                children[index] = vue3_text_suite_text_call(child, include_content, patch_flag);
            }
            _ => {}
        }
    }
}

pub(crate) fn vue3_text_suite_compound(children: &[Value]) -> Value {
    let mut compound_children = Vec::new();
    for (index, child) in children.iter().enumerate() {
        if index > 0 {
            compound_children.push(json!(" + "));
        }
        compound_children.push(child.clone());
    }
    json!({
        "type": 8,
        "children": compound_children,
        "loc": children
            .first()
            .and_then(|child| child.get("loc"))
            .cloned()
            .unwrap_or_else(vue3_loc_stub_value),
    })
}

pub(crate) fn vue3_text_suite_text_call(
    child: Value,
    include_content: bool,
    patch_flag: Option<Value>,
) -> Value {
    let mut args = Vec::new();
    if include_content {
        args.push(child.clone());
    }
    if let Some(flag) = patch_flag {
        args.push(flag);
    }
    let loc = child
        .get("loc")
        .cloned()
        .unwrap_or_else(vue3_loc_stub_value);
    json!({
        "type": 12,
        "content": child,
        "loc": loc,
        "codegenNode": {
            "type": 14,
            "callee": "CREATE_TEXT",
            "arguments": args,
            "loc": loc,
        },
    })
}

pub(crate) fn vue3_text_suite_finalize_root(root: &mut Value) {
    vue3_text_suite_set_root_codegen(root);
    let directives = vue3_text_suite_collect_directives(root);
    root["directives"] = json!(directives);
    root["helpers"] = json!(vue3_text_suite_helpers(root));
    root["components"] = json!([]);
    root["hoists"] = json!([]);
    root["cached"] = json!([]);
    root["temps"] = json!(0);
}

pub(crate) fn vue3_text_suite_set_root_codegen(root: &mut Value) {
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
            let child = children.get(*index).cloned().unwrap_or(Value::Null);
            match vue3_public_node_type(&child) {
                Some(1) => {
                    if let Some(root_child) = root
                        .get_mut("children")
                        .and_then(Value::as_array_mut)
                        .and_then(|items| items.get_mut(*index))
                    {
                        if let Some(codegen) = root_child.get_mut("codegenNode") {
                            codegen["isBlock"] = json!(true);
                        }
                    }
                    root.get("children")
                        .and_then(Value::as_array)
                        .and_then(|items| items.get(*index))
                        .and_then(|child| child.get("codegenNode"))
                        .cloned()
                        .unwrap_or(Value::Null)
                }
                Some(11) => child.get("codegenNode").cloned().unwrap_or(Value::Null),
                _ => child,
            }
        }
        _ => vue3_text_suite_vnode_call(
            "FRAGMENT",
            Value::Null,
            Value::Array(children),
            Some(json!(64)),
            true,
            false,
            Value::Null,
        ),
    };
}

pub(crate) fn vue3_text_suite_element_codegen(node: &Value, is_block: bool) -> Value {
    if vue3_public_node_type(node) != Some(1)
        || node.get("tagType").and_then(Value::as_u64) != Some(0)
    {
        return Value::Null;
    }
    let children = node
        .get("children")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let children_value = if children.is_empty() {
        Value::Null
    } else if children.len() == 1 && vue3_text_suite_direct_child_value(&children[0]) {
        children[0].clone()
    } else {
        Value::Array(children)
    };
    let directives = vue3_text_suite_runtime_directives(node);
    vue3_text_suite_vnode_call(
        &format!(
            "\"{}\"",
            node.get("tag").and_then(Value::as_str).unwrap_or("")
        ),
        Value::Null,
        children_value,
        None,
        is_block,
        false,
        directives,
    )
}

pub(crate) fn vue3_text_suite_for_codegen(node: &Value) -> Value {
    let parse_result = node.get("parseResult").unwrap_or(&Value::Null);
    let source = parse_result.get("source").cloned().unwrap_or(Value::Null);
    let params = ["value", "key", "index"]
        .into_iter()
        .filter_map(|key| {
            parse_result
                .get(key)
                .filter(|value| !value.is_null())
                .cloned()
        })
        .collect::<Vec<_>>();
    let children = node
        .get("children")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let inner_fragment = vue3_text_suite_vnode_call(
        "FRAGMENT",
        Value::Null,
        Value::Array(children),
        Some(json!(64)),
        true,
        false,
        Value::Null,
    );
    let render_list = vue3_text_suite_call(
        "RENDER_LIST",
        vec![
            source,
            json!({
                "type": 18,
                "params": params,
                "returns": inner_fragment,
                "newline": true,
                "isSlot": false,
                "loc": node.get("loc").cloned().unwrap_or_else(vue3_loc_stub_value),
            }),
        ],
    );
    vue3_text_suite_vnode_call(
        "FRAGMENT",
        Value::Null,
        render_list,
        Some(json!(256)),
        true,
        true,
        Value::Null,
    )
}

pub(crate) fn vue3_text_suite_vnode_call(
    tag: &str,
    props: Value,
    children: Value,
    patch_flag: Option<Value>,
    is_block: bool,
    disable_tracking: bool,
    directives: Value,
) -> Value {
    json!({
        "type": 13,
        "tag": tag,
        "props": props,
        "children": children,
        "patchFlag": patch_flag.unwrap_or(Value::Null),
        "dynamicProps": Value::Null,
        "directives": directives,
        "isBlock": is_block,
        "disableTracking": disable_tracking,
        "isComponent": false,
        "loc": vue3_loc_stub_value(),
    })
}

pub(crate) fn vue3_text_suite_call(callee: &str, arguments: Vec<Value>) -> Value {
    json!({
        "type": 14,
        "callee": callee,
        "arguments": arguments,
        "loc": vue3_loc_stub_value(),
    })
}

pub(crate) fn vue3_text_suite_runtime_directives(node: &Value) -> Value {
    let directives = vue3_text_suite_runtime_directive_names(node)
        .into_iter()
        .map(|name| {
            Value::Array(vec![Value::String(vue3_text_suite_directive_asset_id(
                &name,
            ))])
        })
        .collect::<Vec<_>>();
    if directives.is_empty() {
        Value::Null
    } else {
        Value::Array(directives)
    }
}

pub(crate) fn vue3_text_suite_collect_directives(root: &Value) -> Vec<String> {
    let mut directives = Vec::new();
    vue3_text_suite_collect_directives_for_node(root, &mut directives);
    directives
}

pub(crate) fn vue3_text_suite_collect_directives_for_node(
    node: &Value,
    directives: &mut Vec<String>,
) {
    for name in vue3_text_suite_runtime_directive_names(node) {
        if !directives.iter().any(|existing| existing == &name) {
            directives.push(name);
        }
    }
    if let Some(children) = node.get("children").and_then(Value::as_array) {
        for child in children {
            vue3_text_suite_collect_directives_for_node(child, directives);
        }
    }
    if let Some(content) = node.get("content").filter(|value| value.is_object()) {
        vue3_text_suite_collect_directives_for_node(content, directives);
    }
}

pub(crate) fn vue3_text_suite_runtime_directive_names(node: &Value) -> Vec<String> {
    node.get("props")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|prop| {
            if vue3_public_node_type(prop) != Some(7) {
                return None;
            }
            let name = prop.get("name").and_then(Value::as_str)?;
            (!vue3_text_suite_builtin_directive(name)).then(|| name.to_string())
        })
        .collect()
}

pub(crate) fn vue3_text_suite_builtin_directive(name: &str) -> bool {
    matches!(
        name,
        "bind"
            | "cloak"
            | "else"
            | "else-if"
            | "for"
            | "html"
            | "if"
            | "memo"
            | "model"
            | "on"
            | "once"
            | "pre"
            | "slot"
            | "text"
    )
}

pub(crate) fn vue3_text_suite_directive_asset_id(name: &str) -> String {
    let mut id = String::from("_directive_");
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            id.push(ch);
        } else {
            id.push('_');
        }
    }
    id
}

pub(crate) fn vue3_text_suite_helpers(root: &Value) -> Vec<String> {
    let mut used = Vec::new();
    vue3_text_suite_collect_helpers(root, &mut used);
    if !vue3_text_suite_collect_directives(root).is_empty() {
        vue3_text_suite_add_helper(&mut used, "RESOLVE_DIRECTIVE");
    }
    let order = if vue3_text_suite_single_for_root(root) {
        [
            "RENDER_LIST",
            "FRAGMENT",
            "OPEN_BLOCK",
            "CREATE_ELEMENT_BLOCK",
            "CREATE_TEXT",
            "TO_DISPLAY_STRING",
        ]
        .as_slice()
    } else if vue3_text_suite_single_directive_element_root(root) {
        [
            "TO_DISPLAY_STRING",
            "CREATE_TEXT",
            "RESOLVE_DIRECTIVE",
            "OPEN_BLOCK",
            "CREATE_ELEMENT_BLOCK",
            "WITH_DIRECTIVES",
        ]
        .as_slice()
    } else if root
        .get("codegenNode")
        .and_then(|node| node.get("tag"))
        .and_then(Value::as_str)
        == Some("FRAGMENT")
    {
        [
            "CREATE_ELEMENT_VNODE",
            "TO_DISPLAY_STRING",
            "CREATE_TEXT",
            "FRAGMENT",
            "OPEN_BLOCK",
            "CREATE_ELEMENT_BLOCK",
            "RENDER_LIST",
            "WITH_DIRECTIVES",
            "RESOLVE_DIRECTIVE",
        ]
        .as_slice()
    } else {
        [
            "TO_DISPLAY_STRING",
            "CREATE_TEXT",
            "CREATE_ELEMENT_VNODE",
            "OPEN_BLOCK",
            "CREATE_ELEMENT_BLOCK",
            "FRAGMENT",
            "RENDER_LIST",
            "WITH_DIRECTIVES",
            "RESOLVE_DIRECTIVE",
        ]
        .as_slice()
    };
    order
        .iter()
        .copied()
        .filter(|helper| used.contains(helper))
        .map(str::to_string)
        .collect()
}

pub(crate) fn vue3_text_suite_collect_helpers(node: &Value, used: &mut Vec<&'static str>) {
    match vue3_public_node_type(node) {
        Some(5) => vue3_text_suite_add_helper(used, "TO_DISPLAY_STRING"),
        Some(12) => {
            if let Some(codegen) = node.get("codegenNode") {
                vue3_text_suite_collect_helpers(codegen, used);
            }
            if let Some(content) = node.get("content") {
                vue3_text_suite_collect_helpers(content, used);
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
                vue3_text_suite_add_helper(used, "CREATE_ELEMENT_BLOCK");
            } else {
                vue3_text_suite_add_helper(used, "CREATE_ELEMENT_VNODE");
            }
        }
        Some(14) => {
            if let Some(callee) = node.get("callee").and_then(Value::as_str) {
                match callee {
                    "CREATE_TEXT" => vue3_text_suite_add_helper(used, "CREATE_TEXT"),
                    "RENDER_LIST" => vue3_text_suite_add_helper(used, "RENDER_LIST"),
                    _ => {}
                }
            }
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
    ] {
        let Some(value) = node.get(key) else {
            continue;
        };
        if let Some(items) = value.as_array() {
            for item in items {
                vue3_text_suite_collect_helpers(item, used);
            }
        } else if value.is_object() {
            vue3_text_suite_collect_helpers(value, used);
        }
    }
}

pub(crate) fn vue3_text_suite_add_helper(used: &mut Vec<&'static str>, helper: &'static str) {
    if !used.contains(&helper) {
        used.push(helper);
    }
}

pub(crate) fn vue3_text_suite_single_for_root(root: &Value) -> bool {
    root.get("children")
        .and_then(Value::as_array)
        .is_some_and(|children| {
            children.len() == 1 && vue3_public_node_type(&children[0]) == Some(11)
        })
}

pub(crate) fn vue3_text_suite_single_directive_element_root(root: &Value) -> bool {
    root.get("children")
        .and_then(Value::as_array)
        .is_some_and(|children| {
            children.len() == 1
                && vue3_public_node_type(&children[0]) == Some(1)
                && !vue3_text_suite_runtime_directive_names(&children[0]).is_empty()
        })
}

pub(crate) fn vue3_text_suite_direct_child_value(child: &Value) -> bool {
    matches!(vue3_public_node_type(child), Some(2 | 5 | 8))
}

pub(crate) fn vue3_text_suite_directive<'a>(node: &'a Value, name: &str) -> Option<&'a Value> {
    node.get("props")
        .and_then(Value::as_array)?
        .iter()
        .find(|prop| {
            vue3_public_node_type(prop) == Some(7)
                && prop.get("name").and_then(Value::as_str) == Some(name)
        })
}

pub(crate) fn vue3_text_suite_remove_directive(node: &mut Value, name: &str) {
    if let Some(props) = node.get_mut("props").and_then(Value::as_array_mut) {
        props.retain(|prop| {
            !(vue3_public_node_type(prop) == Some(7)
                && prop.get("name").and_then(Value::as_str) == Some(name))
        });
    }
}

pub(crate) fn vue3_text_suite_transform_context(options: &Vue3CompilerOptions) -> Value {
    json!({
        "compat": true,
        "ssr": false,
        "inSSR": false,
        "prefixIdentifiers": options.prefix_identifiers,
        "inline": options.inline,
        "isTS": options.is_ts,
        "expressionPlugins": options.expression_plugins,
        "directiveTransforms": [],
        "identifiers": {},
        "bindingMetadata": options.binding_metadata,
    })
}

pub(crate) fn vue3_text_suite_transform_context_with_directives(
    options: &Vue3CompilerOptions,
    directive_transforms: &[&str],
) -> Value {
    let mut context = vue3_text_suite_transform_context(options);
    context["directiveTransforms"] = Value::Array(
        directive_transforms
            .iter()
            .map(|name| Value::String((*name).to_string()))
            .collect(),
    );
    context
}

pub(crate) fn vue3_public_node_type(node: &Value) -> Option<u64> {
    node.get("type").and_then(Value::as_u64)
}
