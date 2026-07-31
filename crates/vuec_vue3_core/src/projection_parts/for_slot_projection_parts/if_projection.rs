pub(crate) fn transform_if_process_projection(payload: &Value) -> Value {
    let dir = payload.get("dir").unwrap_or(&Value::Null);
    let node = payload.get("node").unwrap_or(&Value::Null);
    let context = payload.get("context").unwrap_or(&Value::Null);
    let siblings = payload
        .get("siblings")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let node_index = payload
        .get("nodeIndex")
        .and_then(Value::as_u64)
        .map(|index| index as usize);
    let dir_name = json_str(dir, "name").unwrap_or("");
    let mut errors = Vec::<Value>::new();
    let condition = transform_if_condition_projection(dir, node, context, &mut errors);
    let branch = json!({
        "condition": condition,
        "children": if json_u64(node, "tagType") == Some(3) && !json_node_has_directive(node, "for") {
            "template"
        } else {
            "self"
        },
        "isTemplateIf": json_u64(node, "tagType") == Some(3),
    });

    if dir_name == "if" {
        return json!({
            "errors": errors,
            "branch": branch,
            "action": {
                "kind": "create",
                "keyBase": node_index
                    .map(|index| transform_if_previous_key_base(siblings, index))
                    .unwrap_or_default(),
            },
        });
    }

    let Some(node_index) = node_index else {
        errors.push(json!({ "code": 30, "loc": "node" }));
        return json!({
            "errors": errors,
            "branch": branch,
            "action": { "kind": "noop" },
        });
    };

    let mut remove_indices = Vec::<usize>::new();
    let mut comment_indices = Vec::<usize>::new();
    let mut scan_index = node_index as isize - 1;
    while scan_index >= 0 {
        let index = scan_index as usize;
        let sibling = &siblings[index];
        if transform_if_is_comment_or_whitespace(sibling) {
            remove_indices.push(index);
            if json_node_type(sibling) == Some(3) {
                comment_indices.insert(0, index);
            }
            scan_index -= 1;
            continue;
        }

        if json_node_type(sibling) == Some(9) {
            if transform_if_last_branch_is_else(sibling) {
                errors.push(json!({ "code": 30, "loc": "node" }));
            }
            let current_key = payload.get("currentUserKey").unwrap_or(&Value::Null);
            if !current_key.is_null() {
                for branch in sibling
                    .get("branches")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    if transform_if_same_key(
                        branch.get("userKey").unwrap_or(&Value::Null),
                        current_key,
                    ) {
                        errors.push(json!({ "code": 29, "loc": "userKey" }));
                    }
                }
            }
            let parent = payload.get("parent").unwrap_or(&Value::Null);
            if transform_if_parent_is_transition(parent) {
                comment_indices.clear();
            }
            return json!({
                "errors": errors,
                "branch": branch,
                "action": {
                    "kind": "append",
                    "targetIndex": index,
                    "removeIndices": remove_indices,
                    "commentIndices": comment_indices,
                },
            });
        }

        errors.push(json!({ "code": 30, "loc": "node" }));
        return json!({
            "errors": errors,
            "branch": branch,
            "action": { "kind": "noop" },
        });
    }

    errors.push(json!({ "code": 30, "loc": "node" }));
    json!({
        "errors": errors,
        "branch": branch,
        "action": { "kind": "noop" },
    })
}

pub(crate) fn transform_if_condition_projection(
    dir: &Value,
    node: &Value,
    context: &Value,
    errors: &mut Vec<Value>,
) -> Value {
    if json_str(dir, "name") == Some("else") {
        return Value::Null;
    }
    let exp = dir.get("exp").filter(|value| !value.is_null());
    let raw_content = exp.and_then(|exp| json_str(exp, "content")).unwrap_or("");
    let missing = exp.is_none() || raw_content.trim().is_empty();
    if missing {
        errors.push(json!({ "code": 28, "loc": "dir" }));
        return json!({
            "kind": "simple",
            "content": "true",
            "isStatic": false,
            "constType": 0,
            "loc": exp
                .and_then(|exp| exp.get("loc"))
                .or_else(|| node.get("loc"))
                .cloned()
                .unwrap_or(Value::Null),
        });
    }

    if !json_bool(context, "prefixIdentifiers") {
        return Value::Null;
    }

    let options = vue3_options_from_transform_context(context);
    let locals = transform_context_locals(context);
    let rewritten = if locals.is_empty() {
        rewrite_js_like_expression(raw_content, &options)
    } else {
        rewrite_js_like_expression_with_locals(raw_content, &options, &locals)
    };
    json!({
        "kind": "simple",
        "content": rewritten,
        "isStatic": false,
        "constType": 0,
        "loc": exp
            .and_then(|exp| exp.get("loc"))
            .cloned()
            .unwrap_or(Value::Null),
    })
}

pub(crate) fn transform_if_branch_codegen_projection(payload: &Value) -> Value {
    let branch = payload.get("branch").unwrap_or(&Value::Null);
    let children = branch
        .get("children")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let first = children.first();
    let need_fragment_wrapper = children.len() != 1
        || first
            .and_then(json_node_type)
            .is_some_and(|node_type| node_type != 1);
    if need_fragment_wrapper {
        if children.len() == 1 && first.and_then(json_node_type) == Some(11) {
            return json!({ "kind": "for" });
        }
        let mut patch_flag = 64u16;
        if !json_bool(branch, "isTemplateIf")
            && children
                .iter()
                .filter(|child| json_node_type(child) != Some(3))
                .count()
                == 1
        {
            patch_flag |= 2048;
        }
        return json!({
            "kind": "fragment",
            "patchFlag": patch_flag,
        });
    }

    json!({
        "kind": "single",
        "convertToBlock": first
            .and_then(|child| json_u64(child, "memoedCodegenType"))
            == Some(13),
    })
}

pub(crate) fn transform_if_previous_key_base(siblings: &[Value], node_index: usize) -> usize {
    siblings
        .iter()
        .take(node_index)
        .filter(|sibling| json_node_type(sibling) == Some(9))
        .map(|sibling| {
            sibling
                .get("branches")
                .and_then(Value::as_array)
                .map(Vec::len)
                .unwrap_or_default()
        })
        .sum()
}

pub(crate) fn transform_if_is_comment_or_whitespace(node: &Value) -> bool {
    match json_node_type(node) {
        Some(3) => true,
        Some(2) => {
            let content_is_ascii_whitespace =
                json_str(node, "content").is_some_and(transform_if_is_ascii_html_whitespace);
            let loc_is_ascii_whitespace = json_str(node, "locSource")
                .map(transform_if_is_ascii_html_whitespace)
                .unwrap_or(true);
            content_is_ascii_whitespace && loc_is_ascii_whitespace
        }
        Some(12) => node
            .get("content")
            .is_some_and(transform_if_is_comment_or_whitespace),
        _ => false,
    }
}

pub(crate) fn transform_if_is_ascii_html_whitespace(content: &str) -> bool {
    content
        .bytes()
        .all(|byte| matches!(byte, b'\t' | b'\n' | b'\x0C' | b'\r' | b' '))
}

pub(crate) fn transform_if_last_branch_is_else(if_node: &Value) -> bool {
    if_node
        .get("branches")
        .and_then(Value::as_array)
        .and_then(|branches| branches.last())
        .is_some_and(|branch| !json_bool(branch, "hasCondition"))
}

pub(crate) fn transform_if_same_key(a: &Value, b: &Value) -> bool {
    if a.is_null() || b.is_null() || json_node_type(a) != json_node_type(b) {
        return false;
    }
    match json_node_type(a) {
        Some(6) => {
            a.get("value").and_then(|value| json_str(value, "content"))
                == b.get("value").and_then(|value| json_str(value, "content"))
        }
        Some(7) => {
            let a_exp = a.get("exp").unwrap_or(&Value::Null);
            let b_exp = b.get("exp").unwrap_or(&Value::Null);
            json_node_type(a_exp) == json_node_type(b_exp)
                && json_bool(a_exp, "isStatic") == json_bool(b_exp, "isStatic")
                && json_str(a_exp, "content") == json_str(b_exp, "content")
        }
        _ => false,
    }
}

pub(crate) fn transform_if_parent_is_transition(parent: &Value) -> bool {
    json_node_type(parent) == Some(1)
        && matches!(json_str(parent, "tag"), Some("transition" | "Transition"))
}

pub(crate) fn json_node_has_directive(node: &Value, name: &str) -> bool {
    node.get("props")
        .and_then(Value::as_array)
        .is_some_and(|props| {
            props
                .iter()
                .any(|prop| json_node_type(prop) == Some(7) && json_str(prop, "name") == Some(name))
        })
}

pub(crate) fn vue3_options_from_transform_context(context: &Value) -> Vue3CompilerOptions {
    let mut options = Vue3CompilerOptions {
        prefix_identifiers: json_bool(context, "prefixIdentifiers"),
        inline: json_bool(context, "inline"),
        is_ts: json_bool(context, "isTS"),
        ..Vue3CompilerOptions::default()
    };
    options.expression_plugins = context
        .get("expressionPlugins")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|plugin| {
            plugin.as_str().or_else(|| {
                plugin
                    .as_array()
                    .and_then(|items| items.first())
                    .and_then(Value::as_str)
            })
        })
        .map(str::to_string)
        .collect();
    if let Some(metadata) = context.get("bindingMetadata").and_then(Value::as_object) {
        for (key, value) in metadata {
            if key == "__propsAliases" {
                if let Some(aliases) = value.as_object() {
                    options.props_aliases = aliases
                        .iter()
                        .filter_map(|(alias, source)| {
                            source
                                .as_str()
                                .map(|source| (alias.clone(), source.to_string()))
                        })
                        .collect();
                }
            } else if let Some(kind) = value.as_str() {
                options
                    .binding_metadata
                    .insert(key.clone(), kind.to_string());
            }
        }
    }
    options
}

pub(crate) fn transform_context_locals(context: &Value) -> Vec<String> {
    context
        .get("identifiers")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|identifiers| identifiers.iter())
        .filter(|(_, count)| count.as_i64().unwrap_or_default() > 0)
        .map(|(name, _)| name.clone())
        .collect()
}
