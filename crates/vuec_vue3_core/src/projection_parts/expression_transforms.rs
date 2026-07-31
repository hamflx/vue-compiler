/// Projects Rust-backed `processExpression` behavior for bridge callers.
pub fn process_expression_projection(payload: &Value) -> Value {
    let node = payload.get("node").unwrap_or(&Value::Null);
    let context = payload.get("context").unwrap_or(&Value::Null);
    let raw = json_str(node, "content").unwrap_or("");
    let as_params = json_bool(payload, "asParams");
    let as_raw_statements = json_bool(payload, "asRawStatements");
    if json_node_type(node) != Some(4)
        || json_bool(node, "isStatic")
        || !json_bool(context, "prefixIdentifiers")
        || raw.trim().is_empty()
    {
        return json!({ "kind": "unchanged" });
    }

    if process_expression_is_static_literal(raw) {
        return json!({
            "kind": "setConstType",
            "constType": 3,
        });
    }

    let options = vue3_options_from_transform_context(context);
    let source_type = transform_on_source_type(context);
    if process_expression_ast_required_unavailable(raw, source_type) {
        return json!({
            "kind": "error",
            "code": 46,
            "loc": node.get("loc").cloned().unwrap_or(Value::Null),
            "message": PROCESS_EXPRESSION_AST_LIMIT_MESSAGE,
        });
    }
    let locals = process_expression_locals(payload, context);
    if as_params {
        if is_simple_identifier_ascii(raw) {
            return json!({
                "kind": "setConstType",
                "constType": 2,
            });
        }
        return process_expression_params_projection(raw, node, context, &options);
    }
    let literal = matches!(raw, "true" | "false" | "null" | "this");
    if is_simple_identifier_ascii(raw) {
        let is_local = locals.iter().any(|local| local == raw);
        let is_global = is_global_or_literal(raw);
        if !as_params
            && !is_local
            && !literal
            && (!is_global || options.binding_metadata.contains_key(raw))
        {
            let content = process_expression_rewrite_identifier(
                raw, &options, None, None, false, &[], None,
            );
            return json!({
                "kind": "simple",
                "content": content,
                "isStatic": false,
                "constType": if process_expression_is_const_binding(raw, &options) { 1 } else { 0 },
                "loc": node.get("loc").cloned().unwrap_or(Value::Null),
                "helpers": vue3_for_helpers_for_content(&content),
            });
        }
        if !is_local {
            return json!({
                "kind": "setConstType",
                "constType": if literal { 3 } else { 2 },
            });
        }
        return json!({ "kind": "unchanged" });
    }

    let source = if as_raw_statements {
        format!(" {raw} ")
    } else {
        format!("({raw}){}", if as_params { "=>{}" } else { "" })
    };
    let store = JsAstStore::new();
    let parse_ok = if process_expression_uses_supported_external_plugin(raw, context) {
        true
    } else if as_raw_statements {
        let parsed = store.parse_program(&source, source_type);
        !parsed.panicked && parsed.errors.is_empty()
    } else {
        store.parse_expression(&source, source_type).is_ok()
    };
    if !parse_ok {
        return json!({
            "kind": "error",
            "code": 46,
            "loc": node.get("loc").cloned().unwrap_or(Value::Null),
            "message": "Error parsing JavaScript expression: Unexpected token",
        });
    }

    let mut effective_locals = locals;
    if !as_raw_statements {
        effective_locals.extend(transform_on_root_function_locals(raw));
        effective_locals.sort();
        effective_locals.dedup();
    }
    let children = process_expression_compound_children(
        raw,
        &options,
        &effective_locals,
        node.get("loc").unwrap_or(&Value::Null),
    );
    if children.is_empty() {
        return json!({
            "kind": "setConstType",
            "constType": 3,
        });
    }
    let rewritten = process_expression_rewrite_source(raw, &options, &effective_locals);
    let mut helper_source = rewritten.clone();
    for child in &children {
        if let Some(content) = child.get("content").and_then(Value::as_str) {
            helper_source.push_str(content);
        }
    }
    json!({
        "kind": "compound",
        "children": children,
        "loc": node.get("loc").cloned().unwrap_or(Value::Null),
        "identifiers": effective_locals,
        "helpers": vue3_for_helpers_for_content(&helper_source),
    })
}

/// Projects Rust-backed `transformExpression` behavior for bridge callers.
pub fn transform_expression_projection(payload: &Value) -> Value {
    let node = payload.get("node").unwrap_or(&Value::Null);
    let context = payload.get("context").unwrap_or(&Value::Null);
    let mut operations = Vec::<Value>::new();
    match json_node_type(node) {
        Some(5) => {
            let Some(content) = node.get("content") else {
                return json!({ "operations": operations });
            };
            operations.push(json!({
                "kind": "process",
                "path": ["content"],
                "projection": process_expression_projection(&json!({
                    "node": content,
                    "context": context,
                })),
            }));
        }
        Some(1) => {
            let memo_index = node
                .get("props")
                .and_then(Value::as_array)
                .and_then(|props| {
                    props.iter().position(|prop| {
                        json_node_type(prop) == Some(7) && json_str(prop, "name") == Some("memo")
                    })
                });
            for (index, dir) in node
                .get("props")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or(&[])
                .iter()
                .enumerate()
            {
                if json_node_type(dir) != Some(7) || json_str(dir, "name") == Some("for") {
                    continue;
                }
                let arg = dir.get("arg").unwrap_or(&Value::Null);
                if let Some(exp) = dir.get("exp").filter(|exp| json_node_type(exp) == Some(4)) {
                    let skip_on_arg = json_str(dir, "name") == Some("on") && !arg.is_null();
                    let skip_memo_key = memo_index.is_some()
                        && json_node_type(arg) == Some(4)
                        && json_str(arg, "content") == Some("key");
                    if !skip_on_arg && !skip_memo_key {
                        operations.push(json!({
                            "kind": "process",
                            "path": ["props", index.to_string(), "exp"],
                            "projection": process_expression_projection(&json!({
                                "node": exp,
                                "context": context,
                                "asParams": json_str(dir, "name") == Some("slot"),
                            })),
                        }));
                    }
                }
                if json_node_type(arg) == Some(4) && !json_bool(arg, "isStatic") {
                    operations.push(json!({
                        "kind": "process",
                        "path": ["props", index.to_string(), "arg"],
                        "projection": process_expression_projection(&json!({
                            "node": arg,
                            "context": context,
                        })),
                    }));
                }
            }
        }
        _ => {}
    }
    json!({ "operations": operations })
}

/// Projects Rust-backed `transformOnce` behavior for bridge callers.
pub fn transform_once_projection(payload: &Value) -> Value {
    let node = payload.get("node").unwrap_or(&Value::Null);
    let context = payload.get("context").unwrap_or(&Value::Null);
    if json_node_type(node) != Some(1)
        || vue3_directive(node, "once", true).is_none()
        || json_bool(payload, "seen")
        || json_bool(context, "inVOnce")
        || json_bool(context, "inSSR")
    {
        return json!({ "kind": "noop" });
    }
    json!({
        "kind": "enter",
        "helper": "SET_BLOCK_TRACKING",
        "markSeen": true,
        "enterInVOnce": true,
        "exit": {
            "restoreInVOnce": false,
            "cacheCodegen": true,
            "isVNode": true,
            "inVOnce": true,
        }
    })
}

/// Projects Rust-backed `transformMemo` behavior for bridge callers.
pub fn transform_memo_projection(payload: &Value) -> Value {
    let node = payload.get("node").unwrap_or(&Value::Null);
    let context = payload.get("context").unwrap_or(&Value::Null);
    let Some(dir) = vue3_directive(node, "memo", false) else {
        return json!({ "kind": "noop" });
    };
    if json_node_type(node) != Some(1) || json_bool(payload, "seen") || json_bool(context, "inSSR")
    {
        return json!({ "kind": "noop" });
    }
    json!({
        "kind": "enter",
        "markSeen": true,
        "exit": {
            "wrapMemo": true,
            "convertToBlock": json_u64(node, "tagType") != Some(1),
            "helper": "WITH_MEMO",
            "exp": dir.get("exp").cloned().unwrap_or(Value::Null),
            "cacheIndex": json_u64(context, "cachedLength").unwrap_or(0),
        }
    })
}

/// Projects Rust-backed static-cache analysis for bridge callers.
pub fn cache_static_projection(payload: &Value) -> Value {
    let root = payload.get("root").unwrap_or(&Value::Null);
    let context = payload.get("context").unwrap_or(&Value::Null);
    let children = root
        .get("children")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let do_not_hoist_root = vue3_single_element_root(children).is_some();
    let mut state = Vue3CacheStaticState::default();
    vue3_cache_static_walk(
        children,
        vec!["children".to_string()],
        None,
        root,
        context,
        do_not_hoist_root,
        &mut state,
    );
    json!({
        "operations": state.operations,
    })
}

/// Projects Rust-backed `stringifyStatic` transform-hoist behavior for public AST callers.
pub fn stringify_static_projection(payload: &Value) -> Value {
    let children = payload
        .get("children")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let context = payload.get("context").unwrap_or(&Value::Null);
    let parent = payload.get("parent").unwrap_or(&Value::Null);
    if json_usize(
        context
            .get("scopes")
            .unwrap_or_else(|| context.get("scope").unwrap_or(&Value::Null)),
        "vSlot",
    )
    .unwrap_or(0)
        > 0
    {
        return json!({ "operations": [] });
    }

    let is_parent_cached = vue3_stringify_parent_is_cached(parent);
    let mut virtual_children = (0..children.len())
        .map(Vue3StringifyVirtualChild::Original)
        .collect::<Vec<_>>();
    let mut operations = Vec::new();
    let mut current_chunk = Vec::<StaticHtmlAnalysis>::new();
    let mut index = 0usize;
    while index < virtual_children.len() {
        let child = match virtual_children[index] {
            Vue3StringifyVirtualChild::Original(original) => children.get(original),
            Vue3StringifyVirtualChild::StaticCall => None,
        };
        if let Some(child) = child {
            let is_cached = is_parent_cached || vue3_stringify_cached_node(child).is_some();
            if is_cached {
                if let Some(analysis) = vue3_stringify_analyze_public_node(child, context) {
                    current_chunk.push(analysis);
                    index += 1;
                    continue;
                }
            }
        }

        let delete_count = vue3_stringify_flush_public_chunk(
            index,
            is_parent_cached,
            &mut current_chunk,
            &mut virtual_children,
            &mut operations,
        );
        current_chunk.clear();
        index = index.saturating_sub(delete_count) + 1;
    }
    vue3_stringify_flush_public_chunk(
        virtual_children.len(),
        is_parent_cached,
        &mut current_chunk,
        &mut virtual_children,
        &mut operations,
    );

    json!({ "operations": operations })
}
