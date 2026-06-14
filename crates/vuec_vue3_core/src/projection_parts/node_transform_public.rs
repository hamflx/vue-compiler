/// Projects Rust-backed `transformModel` behavior for bridge callers.
pub fn transform_model_projection(payload: &Value) -> Value {
    let dir = payload.get("dir").unwrap_or(&Value::Null);
    let node = payload.get("node").unwrap_or(&Value::Null);
    let context = payload.get("context").unwrap_or(&Value::Null);
    let Some(exp) = dir.get("exp").filter(|value| !value.is_null()) else {
        return json!({ "errors": [41], "props": [] });
    };

    let raw_exp = exp
        .get("loc")
        .and_then(|loc| loc.get("source"))
        .and_then(Value::as_str)
        .unwrap_or_else(|| json_str(exp, "content").unwrap_or(""))
        .trim();
    let exp_string = json_str(exp, "content").unwrap_or(raw_exp);
    let binding_type = context
        .get("bindingMetadata")
        .and_then(|metadata| metadata.get(raw_exp))
        .and_then(Value::as_str);

    if matches!(binding_type, Some("props" | "props-aliased")) {
        return json!({ "errors": [44], "props": [] });
    }
    if matches!(binding_type, Some("literal-const" | "setup-const")) {
        return json!({ "errors": [45], "props": [] });
    }

    let maybe_ref = json_bool(context, "inline")
        && matches!(
            binding_type,
            Some("setup-let" | "setup-ref" | "setup-maybe-ref")
        );
    if exp_string.trim().is_empty() || (!model_is_member_expression(raw_exp) && !maybe_ref) {
        return json!({ "errors": [42], "props": [] });
    }
    if json_bool(context, "prefixIdentifiers")
        && is_simple_identifier_ascii(exp_string)
        && context_identifier_count(context, exp_string) > 0
    {
        return json!({ "errors": [43], "props": [] });
    }

    let arg = dir.get("arg").filter(|value| !value.is_null());
    let event_arg = if json_bool(context, "isTS") {
        "($event: any)"
    } else {
        "$event"
    };
    let assignment = model_assignment_projection(exp, raw_exp, event_arg, binding_type, maybe_ref);
    let mut props = vec![
        json!({
            "kind": "modelValue",
            "key": model_prop_name_projection(arg),
            "value": { "kind": "node", "path": "dir.exp" },
            "dynamic": true,
        }),
        json!({
            "kind": "modelUpdate",
            "key": model_event_name_projection(arg),
            "value": assignment,
            "cache": should_cache_model_update(exp, context),
            "dynamic": !should_cache_model_update(exp, context),
            "hydrate": model_update_needs_hydration_event(arg, node),
        }),
    ];

    if dir
        .get("modifiers")
        .and_then(Value::as_array)
        .is_some_and(|modifiers| !modifiers.is_empty())
        && json_u64(node, "tagType") == Some(1)
    {
        props.push(json!({
            "kind": "modelModifiers",
            "key": model_modifiers_key_projection(arg),
            "value": model_modifiers_expression(dir),
            "dynamic": false,
        }));
    }

    json!({
        "errors": [],
        "props": props,
    })
}

/// Projects Rust-backed `transformBind` behavior for bridge callers.
pub fn transform_bind_projection(payload: &Value) -> Value {
    let dir = payload.get("dir").unwrap_or(&Value::Null);
    let context = payload.get("context").unwrap_or(&Value::Null);
    let arg = dir.get("arg").filter(|value| !value.is_null());
    let mut exp = dir.get("exp").filter(|value| !value.is_null());

    if let Some(current_exp) = exp {
        if json_node_type(current_exp) == Some(4)
            && json_str(current_exp, "content")
                .unwrap_or("")
                .trim()
                .is_empty()
        {
            if !json_bool(context, "browser") {
                return json!({
                    "errors": [{ "code": 34, "loc": "dir" }],
                    "props": [{
                        "key": transform_bind_raw_arg_projection(arg, dir),
                        "value": transform_bind_empty_expression_value(dir),
                    }],
                });
            }
            exp = None;
        }
    }

    json!({
        "errors": [],
        "props": [{
            "key": transform_bind_key_projection(arg, dir, context),
            "value": exp
                .map(|_| json!({ "kind": "node", "path": "dir.exp" }))
                .unwrap_or_else(|| json!({ "kind": "undefined" })),
        }],
    })
}

/// Projects Rust-backed v-bind shorthand behavior for bridge callers.
pub fn transform_v_bind_shorthand_projection(payload: &Value) -> Value {
    let node = payload.get("node").unwrap_or(&Value::Null);
    if json_node_type(node) != Some(1) {
        return json!({ "operations": [] });
    }
    let context = payload.get("context").unwrap_or(&Value::Null);
    let operations = node
        .get("props")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
        .filter_map(|(index, prop)| {
            transform_v_bind_shorthand_operation(index, prop, json_bool(context, "browser"))
        })
        .collect::<Vec<_>>();

    json!({ "operations": operations })
}

/// Projects Rust-backed `transformOn` behavior for bridge callers.
pub fn transform_on_projection(payload: &Value) -> Value {
    let dir = payload.get("dir").unwrap_or(&Value::Null);
    let node = payload.get("node").unwrap_or(&Value::Null);
    let context = payload.get("context").unwrap_or(&Value::Null);
    let arg = dir.get("arg").filter(|value| !value.is_null());
    let mut errors = Vec::<Value>::new();

    if dir.get("exp").is_none_or(Value::is_null)
        && dir
            .get("modifiers")
            .and_then(Value::as_array)
            .is_none_or(Vec::is_empty)
    {
        errors.push(json!({ "code": 35, "loc": "dir" }));
    }

    let event_name = transform_on_event_name_projection(arg, node, &mut errors);
    let handler = transform_on_handler_projection(dir, node, context);
    let cache = json_bool(&handler, "cache");
    let value = handler
        .get("value")
        .cloned()
        .unwrap_or_else(|| transform_on_empty_handler_projection(dir));

    json!({
        "errors": errors,
        "props": [{
            "key": event_name,
            "value": value,
            "cache": cache,
            "valueConstant": transform_on_projection_const_type(&value) > 0,
            "handlerKey": true,
            "dynamicKey": arg.is_some_and(|arg| !json_bool(arg, "isStatic")),
            "ignoreDynamicKeyForNormalize": true,
        }],
    })
}

/// Projects Rust-backed `transformIf` behavior for bridge callers.
pub fn transform_if_projection(payload: &Value) -> Value {
    if json_str(payload, "phase") == Some("branchCodegen") {
        return transform_if_branch_codegen_projection(payload);
    }
    transform_if_process_projection(payload)
}

/// Projects Rust-backed `transformFor` behavior for bridge callers.
pub fn transform_for_projection(payload: &Value) -> Value {
    if json_str(payload, "phase") == Some("codegen") {
        return transform_for_codegen_projection(payload);
    }
    if json_str(payload, "phase") == Some("exitCodegen") {
        return transform_for_exit_codegen_projection(payload);
    }

    let dir = payload.get("dir").unwrap_or(&Value::Null);
    let node = payload.get("node").unwrap_or(&Value::Null);
    let context = payload.get("context").unwrap_or(&Value::Null);
    let mut errors = Vec::<Value>::new();
    let Some(exp) = dir.get("exp").filter(|value| !value.is_null()) else {
        errors.push(json!({ "code": 31, "loc": "dir" }));
        return json!({ "errors": errors });
    };
    let raw = json_str(exp, "content")
        .or_else(|| exp.get("loc").and_then(|loc| json_str(loc, "source")))
        .unwrap_or("");
    let Some(parsed) = parse_vue3_for_expression(raw) else {
        errors.push(json!({ "code": 32, "loc": "dir" }));
        return json!({ "errors": errors });
    };

    let mut source = vue3_for_expression_projection(
        &parsed.source.content,
        exp,
        parsed.source.start,
        parsed.source.end,
        Vue3ForAstMode::Expression,
    );
    let mut value = parsed.value.as_ref().map(|part| {
        vue3_for_expression_projection(
            &part.content,
            exp,
            part.start,
            part.end,
            Vue3ForAstMode::Params,
        )
    });
    let mut key = parsed.key.as_ref().map(|part| {
        vue3_for_expression_projection(
            &part.content,
            exp,
            part.start,
            part.end,
            Vue3ForAstMode::Params,
        )
    });
    let mut index = parsed.index.as_ref().map(|part| {
        vue3_for_expression_projection(
            &part.content,
            exp,
            part.start,
            part.end,
            Vue3ForAstMode::Params,
        )
    });

    if json_bool(context, "prefixIdentifiers") {
        let options = vue3_options_from_transform_context(context);
        let locals = transform_context_locals(context);
        source = vue3_for_rewrite_projection_node(
            &parsed.source.content,
            &options,
            &locals,
            source["loc"].clone(),
            Vue3ForAstMode::Expression,
            false,
        );
        let scoped = parsed
            .all_alias_locals()
            .into_iter()
            .chain(locals)
            .collect::<Vec<_>>();
        if let Some(part) = parsed.value.as_ref() {
            value = Some(vue3_for_rewrite_projection_node(
                &part.content,
                &options,
                &scoped,
                value
                    .as_ref()
                    .and_then(|node| node.get("loc"))
                    .cloned()
                    .unwrap_or(Value::Null),
                Vue3ForAstMode::Params,
                true,
            ));
        }
        if let Some(part) = parsed.key.as_ref() {
            key = Some(vue3_for_rewrite_projection_node(
                &part.content,
                &options,
                &scoped,
                key.as_ref()
                    .and_then(|node| node.get("loc"))
                    .cloned()
                    .unwrap_or(Value::Null),
                Vue3ForAstMode::Params,
                true,
            ));
        }
        if let Some(part) = parsed.index.as_ref() {
            index = Some(vue3_for_rewrite_projection_node(
                &part.content,
                &options,
                &scoped,
                index
                    .as_ref()
                    .and_then(|node| node.get("loc"))
                    .cloned()
                    .unwrap_or(Value::Null),
                Vue3ForAstMode::Params,
                true,
            ));
        }
    }

    let parse_result = json!({
        "source": source,
        "value": value,
        "key": key,
        "index": index,
        "finalized": true,
    });
    let template_key_errors = vue3_for_template_key_errors(node);

    json!({
        "errors": errors,
        "parseResult": parse_result,
        "locals": parsed.all_alias_locals(),
        "children": if json_u64(node, "tagType") == Some(3) { "template" } else { "self" },
        "templateKeyErrors": template_key_errors,
    })
}

/// Projects Rust-backed slot-scope tracking for bridge callers.
pub fn track_slot_scopes_projection(payload: &Value) -> Value {
    let node = payload.get("node").unwrap_or(&Value::Null);
    let Some(slot) = vue3_slot_directive(node, false) else {
        return json!({ "track": false });
    };
    let locals = slot
        .get("exp")
        .filter(|exp| !exp.is_null())
        .map(vue3_slot_param_locals)
        .unwrap_or_default();
    json!({
        "track": true,
        "slotProps": slot.get("exp").cloned().unwrap_or(Value::Null),
        "locals": locals,
    })
}

/// Projects Rust-backed `v-for` slot-scope tracking for bridge callers.
pub fn track_v_for_slot_scopes_projection(payload: &Value) -> Value {
    let node = payload.get("node").unwrap_or(&Value::Null);
    if json_node_type(node) != Some(1)
        || json_u64(node, "tagType") != Some(3)
        || vue3_slot_directive(node, true).is_none()
    {
        return json!({ "track": false });
    }
    let Some(dir) = vue3_directive(node, "for", true) else {
        return json!({ "track": false });
    };
    let context = payload.get("context").unwrap_or(&Value::Null);
    let projection = vue3_for_parse_result_projection(node, dir, context);
    if projection.get("parseResult").is_none() {
        return json!({ "track": false, "errors": projection.get("errors").cloned().unwrap_or_else(|| json!([])) });
    }
    json!({
        "track": true,
        "dir": dir,
        "parseResult": projection["parseResult"].clone(),
        "locals": projection["locals"].clone(),
    })
}

/// Projects Rust-backed slot outlet transforms for bridge callers.
pub fn transform_slot_outlet_projection(payload: &Value) -> Value {
    let node = payload.get("node").unwrap_or(&Value::Null);
    if json_node_type(node) != Some(1) || json_u64(node, "tagType") != Some(2) {
        return json!({ "transform": false });
    }
    let context = payload.get("context").unwrap_or(&Value::Null);
    let process = process_slot_outlet_projection(node, context);
    let has_children = node
        .get("children")
        .and_then(Value::as_array)
        .is_some_and(|children| !children.is_empty());
    let mut expected_len = 2;
    if has_children {
        expected_len = 4;
    }
    if json_str(context, "scopeId").is_some() && !json_bool(context, "slotted") {
        expected_len = 5;
    }

    json!({
        "transform": true,
        "process": process,
        "codegen": {
            "slots": if json_bool(context, "prefixIdentifiers") { "_ctx.$slots" } else { "$slots" },
            "expectedLen": expected_len,
            "hasChildren": has_children,
            "helper": "RENDER_SLOT",
        },
    })
}

pub(crate) fn process_slot_outlet_projection(node: &Value, context: &Value) -> Value {
    let mut slot_name = json!({ "kind": "literal", "value": "\"default\"" });
    let mut non_name_props = Vec::<Value>::new();
    let mut mutations = Vec::<Value>::new();

    for (index, prop) in node
        .get("props")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
    {
        if json_node_type(prop) == Some(6) {
            if prop.get("value").is_none_or(Value::is_null) {
                continue;
            }
            if json_str(prop, "name") == Some("name") {
                let content = prop
                    .get("value")
                    .and_then(|value| json_str(value, "content"))
                    .unwrap_or("");
                slot_name = json!({ "kind": "literal", "value": quote_string(content) });
            } else {
                if let Some(name) = json_str(prop, "name") {
                    let camel = camelize(name);
                    if camel != name {
                        mutations.push(json!({
                            "kind": "setPropName",
                            "index": index,
                            "name": camel,
                        }));
                    }
                }
                non_name_props.push(json!(index));
            }
            continue;
        }

        if json_str(prop, "name") == Some("bind")
            && prop.get("arg").is_some_and(|arg| {
                json_node_type(arg) == Some(4)
                    && json_bool(arg, "isStatic")
                    && json_str(arg, "content") == Some("name")
            })
        {
            if prop.get("exp").is_some_and(|exp| !exp.is_null()) {
                slot_name =
                    json!({ "kind": "node", "path": "props", "index": index, "field": "exp" });
            } else if prop
                .get("arg")
                .is_some_and(|arg| json_node_type(arg) == Some(4))
            {
                let name = prop
                    .get("arg")
                    .and_then(|arg| json_str(arg, "content"))
                    .map(camelize)
                    .unwrap_or_default();
                let loc = prop
                    .get("arg")
                    .and_then(|arg| arg.get("loc"))
                    .cloned()
                    .unwrap_or(Value::Null);
                let exp = json!({
                    "kind": "simple",
                    "content": name,
                    "isStatic": false,
                    "loc": loc,
                });
                mutations.push(json!({
                    "kind": "setDirectiveExp",
                    "index": index,
                    "value": process_slot_outlet_maybe_process_expression(exp, context),
                }));
                slot_name =
                    json!({ "kind": "node", "path": "props", "index": index, "field": "exp" });
            }
            continue;
        }

        if json_str(prop, "name") == Some("bind")
            && prop
                .get("arg")
                .is_some_and(|arg| json_node_type(arg) == Some(4) && json_bool(arg, "isStatic"))
        {
            let content = prop
                .get("arg")
                .and_then(|arg| json_str(arg, "content"))
                .unwrap_or("");
            let camel = camelize(content);
            if camel != content {
                mutations.push(json!({
                    "kind": "setDirectiveArgContent",
                    "index": index,
                    "content": camel,
                }));
            }
        }
        non_name_props.push(json!(index));
    }

    json!({
        "slotName": slot_name,
        "nonNameProps": non_name_props,
        "mutations": mutations,
    })
}

pub(crate) fn process_slot_outlet_maybe_process_expression(node: Value, context: &Value) -> Value {
    if !json_bool(context, "prefixIdentifiers") {
        return node;
    }
    let processed = process_expression_projection(&json!({
        "node": {
            "type": 4,
            "content": json_str(&node, "content").unwrap_or(""),
            "isStatic": json_bool(&node, "isStatic"),
            "loc": node.get("loc").cloned().unwrap_or(Value::Null),
        },
        "context": context,
    }));
    match json_str(&processed, "kind") {
        Some("simple") | Some("compound") => processed,
        _ => node,
    }
}

/// Projects Rust-backed `buildSlots` behavior for bridge callers.
pub fn build_slots_projection(payload: &Value) -> Value {
    let node = payload.get("node").unwrap_or(&Value::Null);
    let context = payload.get("context").unwrap_or(&Value::Null);
    let children = node
        .get("children")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let mut properties = Vec::<Value>::new();
    let mut dynamic_slots = Vec::<Value>::new();
    let mut errors = Vec::<Value>::new();
    let mut has_dynamic_slots = json_usize(context, "vSlotDepth").unwrap_or_default() > 0
        || json_usize(context, "vForDepth").unwrap_or_default() > 0;

    if !json_bool(context, "ssr") && json_bool(context, "prefixIdentifiers") {
        has_dynamic_slots = vue3_component_slot_scope_ref(node, children, context);
    }

    let on_component_slot = vue3_slot_directive(node, true);
    if let Some(slot) = on_component_slot {
        if slot
            .get("arg")
            .filter(|arg| !arg.is_null())
            .is_some_and(|arg| !json_bool(arg, "isStatic"))
        {
            has_dynamic_slots = true;
        }
        properties.push(json!({
            "kind": "property",
            "key": vue3_slot_name_projection(slot, context),
            "params": slot.get("exp").cloned().unwrap_or(Value::Null),
            "indices": vue3_all_child_indices(children),
            "loc": node.get("loc").cloned().unwrap_or(Value::Null),
        }));
    }

    let mut has_template_slots = false;
    let mut has_named_default_slot = false;
    let mut implicit_default_indices = Vec::<usize>::new();
    let mut seen_slot_names = Vec::<String>::new();
    let mut conditional_branch_index = 0usize;

    for (index, child) in children.iter().enumerate() {
        let Some(slot_dir) = vue3_template_slot_directive(child) else {
            if json_node_type(child) != Some(3) {
                implicit_default_indices.push(index);
            }
            continue;
        };

        if on_component_slot.is_some() {
            errors.push(
                json!({ "code": 37, "loc": slot_dir.get("loc").cloned().unwrap_or(Value::Null) }),
            );
            break;
        }

        has_template_slots = true;
        let slot_name = vue3_slot_name_projection(slot_dir, context);
        let static_slot_name = vue3_static_slot_name(slot_dir);
        if static_slot_name.is_none() {
            has_dynamic_slots = true;
        }
        let slot = vue3_slot_function_projection(slot_dir, &[index], child);

        if let Some(if_dir) = vue3_directive(child, "if", false) {
            has_dynamic_slots = true;
            dynamic_slots.push(json!({
                "kind": "conditional",
                "test": vue3_slot_condition_projection(if_dir, context),
                "consequent": vue3_dynamic_slot_projection(slot_name, slot, Some(conditional_branch_index)),
                "alternate": vue3_default_fallback_projection(),
            }));
            conditional_branch_index += 1;
            continue;
        }

        if let Some(else_dir) = vue3_else_slot_directive(child) {
            if let Some(previous) = vue3_previous_non_comment_or_whitespace(children, index) {
                if vue3_template_has_if_like_slot_directive(previous) {
                    let alternate = if json_str(else_dir, "name") == Some("else-if") {
                        json!({
                            "kind": "conditional",
                            "test": vue3_slot_condition_projection(else_dir, context),
                            "consequent": vue3_dynamic_slot_projection(slot_name, slot, Some(conditional_branch_index)),
                            "alternate": vue3_default_fallback_projection(),
                        })
                    } else {
                        vue3_dynamic_slot_projection(
                            slot_name,
                            slot,
                            Some(conditional_branch_index),
                        )
                    };
                    vue3_append_slot_conditional_alternate(&mut dynamic_slots, alternate);
                    conditional_branch_index += 1;
                } else {
                    errors.push(json!({ "code": 30, "loc": else_dir.get("loc").cloned().unwrap_or(Value::Null) }));
                }
            } else {
                errors.push(json!({ "code": 30, "loc": else_dir.get("loc").cloned().unwrap_or(Value::Null) }));
            }
            continue;
        }

        if let Some(for_dir) = vue3_directive(child, "for", true) {
            has_dynamic_slots = true;
            let parsed_projection = vue3_slot_for_parse_result_projection(child, for_dir, context);
            if let Some(parse_result) = parsed_projection.get("parseResult") {
                dynamic_slots.push(json!({
                    "kind": "for",
                    "source": parse_result["source"].clone(),
                    "params": {
                        "value": parse_result["value"].clone(),
                        "key": parse_result["key"].clone(),
                        "index": parse_result["index"].clone(),
                    },
                    "slot": vue3_dynamic_slot_projection(slot_name, slot, None),
                }));
            } else {
                errors.push(json!({ "code": 32, "loc": for_dir.get("loc").cloned().unwrap_or(Value::Null) }));
            }
            continue;
        }

        if let Some(name) = static_slot_name {
            if seen_slot_names.iter().any(|seen| seen == &name) {
                errors.push(json!({ "code": 38, "loc": slot_dir.get("loc").cloned().unwrap_or(Value::Null) }));
                continue;
            }
            if name == "default" {
                has_named_default_slot = true;
            }
            seen_slot_names.push(name);
        }
        properties.push(json!({
            "kind": "property",
            "key": slot_name,
            "params": slot_dir.get("exp").cloned().unwrap_or(Value::Null),
            "indices": [index],
            "unwrapTemplate": true,
            "loc": child.get("loc").cloned().unwrap_or_else(|| node.get("loc").cloned().unwrap_or(Value::Null)),
        }));
    }

    if on_component_slot.is_none() {
        if !has_template_slots {
            properties.push(json!({
                "kind": "property",
                "key": vue3_static_slot_key("default"),
                "params": Value::Null,
                "indices": vue3_all_child_indices(children),
                "loc": node.get("loc").cloned().unwrap_or(Value::Null),
                "nonScoped": true,
            }));
        } else if !implicit_default_indices.is_empty()
            && !vue3_all_indices_are_whitespace_text(children, &implicit_default_indices)
        {
            if has_named_default_slot {
                if let Some(child) = implicit_default_indices
                    .first()
                    .and_then(|index| children.get(*index))
                {
                    errors.push(json!({ "code": 39, "loc": child.get("loc").cloned().unwrap_or(Value::Null) }));
                }
            } else {
                properties.push(json!({
                    "kind": "property",
                    "key": vue3_static_slot_key("default"),
                    "params": Value::Null,
                    "indices": implicit_default_indices,
                    "loc": node.get("loc").cloned().unwrap_or(Value::Null),
                    "nonScoped": true,
                }));
            }
        }
    }

    let slot_flag = if has_dynamic_slots {
        2
    } else if vue3_has_forwarded_slots(children) {
        3
    } else {
        1
    };

    json!({
        "properties": properties,
        "dynamicSlots": dynamic_slots,
        "slotFlag": slot_flag,
        "slotFlagText": vue3_slot_flag_text(slot_flag),
        "hasDynamicSlots": has_dynamic_slots,
        "errors": errors,
    })
}
