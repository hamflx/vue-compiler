/// Projects Rust-backed component type resolution for bridge callers.
pub fn resolve_component_type_projection(payload: &Value) -> Value {
    let node = payload.get("node").unwrap_or(&Value::Null);
    let context = payload.get("context").unwrap_or(&Value::Null);
    let ssr = json_bool(payload, "ssr");
    let mut tag = json_str(node, "tag").unwrap_or("").to_string();
    let is_explicit_dynamic = matches!(tag.as_str(), "component" | "Component");
    let is_prop = resolve_component_is_prop(node);

    if let Some(is_prop) = is_prop {
        if is_explicit_dynamic || json_bool(context, "compatIsOnElement") {
            if let Some(exp) = resolve_component_is_prop_expression(is_prop, context) {
                return json!({
                    "kind": "dynamic",
                    "helper": "RESOLVE_DYNAMIC_COMPONENT",
                    "argument": exp,
                });
            }
        } else if json_node_type(is_prop) == Some(6)
            && is_prop
                .get("value")
                .and_then(|value| json_str(value, "content"))
                .is_some_and(|value| value.starts_with("vue:"))
        {
            tag = is_prop
                .get("value")
                .and_then(|value| json_str(value, "content"))
                .map(|value| value[4..].to_string())
                .unwrap_or(tag);
        }
    }

    if let Some(helper) = vue3_core_component_helper(&tag) {
        return json!({
            "kind": "helper",
            "helper": helper,
            "registerHelper": !ssr,
        });
    }
    if let Some(projection) = context
        .get("builtInComponents")
        .and_then(Value::as_array)
        .and_then(|components| {
            components.iter().find_map(|component| {
                if component.as_str() == Some(&tag) {
                    return Some(json!({
                        "kind": "helper",
                        "helper": tag,
                        "registerHelper": !ssr,
                    }));
                }
                let component_tag = component.get("tag").and_then(Value::as_str)?;
                (component_tag == tag).then(|| {
                    json!({
                        "kind": "helper",
                        "helperName": component.get("helperName").and_then(Value::as_str).unwrap_or(component_tag),
                        "registerHelper": !ssr,
                    })
                })
            })
        })
    {
        return projection;
    }

    if let Some(from_setup) = resolve_setup_reference(&tag, context) {
        return from_setup;
    }
    if let Some(dot_index) = tag.find('.') {
        if dot_index > 0 {
            if let Some(mut namespace) = resolve_setup_reference(&tag[..dot_index], context) {
                if let Some(content) = json_str(&namespace, "content") {
                    let resolved = format!("{}{}", content, &tag[dot_index..]);
                    namespace["content"] = json!(resolved);
                    return namespace;
                }
            }
        }
    }

    let self_name = json_str(context, "selfName");
    let component_name =
        if self_name.is_some_and(|self_name| capitalize(&camelize(&tag)) == self_name) {
            format!("{tag}__self")
        } else {
            tag.clone()
        };
    json!({
        "kind": "asset",
        "helper": "RESOLVE_COMPONENT",
        "component": component_name,
        "assetId": component_asset_id(&tag),
    })
}

/// Projects Rust-backed element prop transform behavior for bridge callers.
pub fn transform_element_props_projection(payload: &Value) -> Value {
    let props = payload
        .get("props")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let context = payload.get("context").unwrap_or(&Value::Null);
    let has_children = json_bool(payload, "hasChildren");
    let is_component = json_bool(payload, "isComponent");
    let is_dynamic_component = json_bool(payload, "isDynamicComponent");
    let in_ssr = json_bool(context, "inSSR");
    let in_v_for = context
        .get("vForDepth")
        .and_then(Value::as_u64)
        .is_some_and(|depth| depth > 0);
    let inline_template_refs = inline_template_ref_projections(props, context);
    let mut patch_flag = 0u16;
    let mut dynamic_prop_names = Vec::<String>::new();
    let mut has_ref = false;
    let mut has_class_binding = false;
    let mut has_style_binding = false;
    let mut has_hydration_event_binding = false;
    let mut has_dynamic_keys = false;
    let mut has_vnode_hook = false;
    let mut should_use_block = false;
    let mut normalize_props = false;
    let mut guard_reactive_props = false;
    let mut normalize_class = false;
    let mut normalize_style = false;
    let mut has_runtime_directives = false;
    let mut has_dynamic_object = false;
    let mut has_normalize_dynamic_keys = false;
    let ref_for_marker = in_v_for
        && props.iter().any(|prop| {
            (matches!(
                json_str(prop, "kind"),
                Some("attribute") | Some("directiveProp")
            ) && json_str(prop, "name") == Some("ref"))
                || json_str(prop, "kind") == Some("objectBind")
        });

    for prop in props {
        match json_str(prop, "kind") {
            Some("attribute") if json_str(prop, "name") == Some("ref") => {
                has_ref = true;
            }
            Some("objectBind") => {
                has_dynamic_keys = true;
                has_normalize_dynamic_keys = true;
                has_dynamic_object = true;
            }
            Some("objectOn") => {
                has_dynamic_keys = true;
                has_normalize_dynamic_keys = true;
                has_dynamic_object = true;
            }
            Some("runtimeDirective") => {
                has_runtime_directives = true;
                if has_children {
                    should_use_block = true;
                }
            }
            Some("directiveProp") => {
                if json_bool(prop, "dynamicKey") {
                    has_dynamic_keys = true;
                    if !json_bool(prop, "ignoreDynamicKeyForNormalize") {
                        has_normalize_dynamic_keys = true;
                    }
                } else if let Some(name) = json_str(prop, "name") {
                    let value_constant = json_bool(prop, "valueConstant");
                    let value_cached = json_bool(prop, "valueCached");
                    let is_event = prop_name_is_event_handler(name);
                    if is_event
                        && (!is_component || is_dynamic_component)
                        && !name.eq_ignore_ascii_case("onclick")
                        && name != "onUpdate:modelValue"
                        && !prop_name_is_reserved(name)
                    {
                        has_hydration_event_binding = true;
                    }
                    if is_event && prop_name_is_reserved(name) {
                        has_vnode_hook = true;
                    }
                    if !value_cached && !value_constant {
                        if name == "ref" {
                            has_ref = true;
                        } else if name == "class" {
                            has_class_binding = true;
                        } else if name == "style" {
                            has_style_binding = true;
                        } else if name != "key"
                            && !dynamic_prop_names.iter().any(|existing| existing == name)
                        {
                            dynamic_prop_names.push(name.to_string());
                        }
                        if is_component
                            && matches!(name, "class" | "style")
                            && !dynamic_prop_names.iter().any(|existing| existing == name)
                        {
                            dynamic_prop_names.push(name.to_string());
                        }
                    }
                }
                if json_bool(prop, "propModifier") {
                    patch_flag |= 32;
                }
                if json_bool(prop, "forceBlock") {
                    should_use_block = true;
                }
            }
            _ => {}
        }
    }

    if has_dynamic_keys {
        patch_flag |= 16;
    } else {
        if has_class_binding && !is_component {
            patch_flag |= 2;
        }
        if has_style_binding && !is_component {
            patch_flag |= 4;
        }
        if !dynamic_prop_names.is_empty() {
            patch_flag |= 8;
        }
        if has_hydration_event_binding {
            patch_flag |= 32;
        }
    }

    if !should_use_block
        && (patch_flag == 0 || patch_flag == 32)
        && (has_ref || has_vnode_hook || has_runtime_directives)
    {
        patch_flag |= 512;
    }

    if !in_ssr {
        normalize_class = has_class_binding || props.iter().any(prop_requires_normalize_class);
        normalize_style = has_style_binding
            || props.iter().any(prop_requires_normalize_style)
            || props
                .iter()
                .filter(|prop| prop_output_name(prop) == Some("style"))
                .count()
                > 1;
        if has_dynamic_object {
            normalize_props = true;
            guard_reactive_props = true;
        } else if has_normalize_dynamic_keys {
            normalize_props = true;
        }
    }

    json!({
        "patchFlag": patch_flag,
        "dynamicPropNames": dynamic_prop_names,
        "shouldUseBlock": should_use_block,
        "normalizeProps": normalize_props,
        "guardReactiveProps": guard_reactive_props,
        "normalizeClass": normalize_class,
        "normalizeStyle": normalize_style,
        "refForMarker": ref_for_marker,
        "inlineTemplateRefs": inline_template_refs,
    })
}

/// Projects Rust-backed directive argument building for bridge callers.
pub fn build_directive_args_projection(payload: &Value) -> Value {
    let dir = payload.get("dir").unwrap_or(&Value::Null);
    let need_runtime = payload.get("needRuntime").unwrap_or(&Value::Null);
    let runtime = if let Some(helper) = need_runtime.get("helper").and_then(Value::as_str) {
        json!({ "kind": "helper", "helper": helper })
    } else if let Some(helper_name) = need_runtime.get("helperName").and_then(Value::as_str) {
        json!({ "kind": "helper", "helperName": helper_name })
    } else {
        json!({
            "kind": "asset",
            "name": json_str(dir, "name").unwrap_or(""),
        })
    };
    let modifiers = dir
        .get("modifiers")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
        .iter()
        .filter_map(|modifier| {
            modifier
                .as_str()
                .or_else(|| modifier.get("content").and_then(Value::as_str))
                .map(|name| json!({ "name": name }))
        })
        .collect::<Vec<_>>();
    json!({
        "runtime": runtime,
        "includeExp": dir.get("exp").is_some_and(|exp| !exp.is_null()),
        "includeArg": dir.get("arg").is_some_and(|arg| !arg.is_null()),
        "modifiers": modifiers,
    })
}

/// Projects Rust-backed built-in element child transform behavior.
pub fn transform_element_children_projection(payload: &Value) -> Value {
    let tag = json_str(payload, "tag").unwrap_or("");
    let children = payload
        .get("children")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    match public_helper_by_name(tag) {
        Some(RuntimeHelper::Vue3Suspense | RuntimeHelper::Vue3BaseTransition) => {
            let slots = component_slot_projections(children);
            json!({
                "kind": "slots",
                "slots": slots,
                "slotFlag": "1 /* STABLE */",
                "patchFlag": null,
                "shouldUseBlock": public_helper_by_name(tag) == Some(RuntimeHelper::Vue3Suspense),
            })
        }
        Some(RuntimeHelper::Vue3KeepAlive) => json!({
            "kind": "children",
            "patchFlag": 1024,
            "shouldUseBlock": true,
        }),
        _ => json!({ "kind": "default" }),
    }
}

/// Projects Rust-backed text transform behavior for bridge callers.
pub fn transform_text_projection(payload: &Value) -> Value {
    let node = payload.get("node").unwrap_or(&Value::Null);
    let context = payload.get("context").unwrap_or(&Value::Null);
    if !matches!(json_node_type(node), Some(0 | 1 | 10 | 11)) {
        return json!({ "operations": [] });
    }
    let Some(source_children) = node.get("children").and_then(Value::as_array) else {
        return json!({ "operations": [] });
    };
    let mut children = source_children.clone();
    let mut operations = Vec::new();
    let mut has_text = false;
    let mut index = 0usize;
    while index < children.len() {
        if !vue3_is_text_node(&children[index]) {
            index += 1;
            continue;
        }
        has_text = true;
        let start = index;
        let mut end = index;
        while end + 1 < children.len() && vue3_is_text_node(&children[end + 1]) {
            end += 1;
        }
        if end > start {
            let compound = vue3_text_compound(&children[start..=end]);
            operations.push(json!({
                "kind": "mergeText",
                "start": start,
                "end": end,
            }));
            children.splice(start..=end, std::iter::once(compound));
            index = start + 1;
        } else {
            index += 1;
        }
    }

    if !has_text {
        return json!({ "operations": operations });
    }

    let single_plain_element_text = children.len() == 1
        && json_node_type(node) == Some(1)
        && json_u64(node, "tagType") == Some(0)
        && !vue3_text_has_untransformed_custom_directive(node, context)
        && !(json_bool(context, "compat") && json_str(node, "tag") == Some("template"));
    if children.len() == 1 && (json_node_type(node) == Some(0) || single_plain_element_text) {
        return json!({ "operations": operations });
    }

    let ssr = json_bool(context, "ssr");
    for (index, child) in children.iter().enumerate() {
        if !(vue3_is_text_node(child) || json_node_type(child) == Some(8)) {
            continue;
        }
        let patch_flag = (!ssr && vue3_constant_type(child, context) == VUE3_CONSTANT_NOT)
            .then_some("1 /* TEXT */");
        operations.push(json!({
            "kind": "wrapTextCall",
            "index": index,
            "includeContent": !(json_node_type(child) == Some(2)
                && json_str(child, "content") == Some(" ")),
            "patchFlag": patch_flag,
        }));
    }

    json!({ "operations": operations })
}
