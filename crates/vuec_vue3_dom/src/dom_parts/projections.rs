/// Projects static `style` attributes for the public DOM `transformStyle` helper.
pub fn transform_style_projection(payload: &Value) -> Value {
    let props = payload
        .get("node")
        .and_then(|node| node.get("props"))
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let replacements = props
        .iter()
        .enumerate()
        .filter_map(|(index, prop)| {
            let is_static_style = prop.get("type").and_then(Value::as_u64) == Some(6)
                && prop.get("name").and_then(Value::as_str) == Some("style")
                && prop.get("value").is_some_and(|value| !value.is_null());
            if !is_static_style {
                return None;
            }
            let value = prop
                .get("value")
                .and_then(|value| value.get("content"))
                .and_then(Value::as_str)
                .unwrap_or("");
            Some(json!({
                "index": index,
                "expression": style_json_string(value),
            }))
        })
        .collect::<Vec<_>>();
    json!({ "replacements": replacements })
}

/// Projects the DOM `ignoreSideEffectTags` node transform for compatibility bridge callers.
pub fn ignore_side_effect_tags_projection(payload: &Value) -> Value {
    let node = payload.get("node").unwrap_or(&Value::Null);
    if !json_node_is_side_effect_tag(node) {
        return json!({
            "remove": false,
            "errors": [],
        });
    }

    json!({
        "remove": true,
        "errors": [{
            "code": 64,
            "loc": node.get("loc").cloned().unwrap_or(Value::Null),
        }],
    })
}

/// Projects browser HTML entity decoding for DOM parser option compatibility.
pub fn decode_html_browser_projection(payload: &Value) -> Value {
    let raw = json_str(payload, "raw").unwrap_or("");
    let decoded = if json_bool(payload, "asAttr") {
        decode_html_attr_entities(raw)
    } else {
        decode_html_text_entities(raw)
    };
    json!({ "decoded": decoded })
}

/// Projects the DOM `v-html` directive transform for compatibility bridge callers.
pub fn transform_v_html_projection(payload: &Value) -> Value {
    transform_dom_content_directive_projection(
        payload,
        DomContentDirectiveProjection {
            key: "innerHTML",
            key_loc: Some("dir"),
            missing_expression_code: 54,
            with_children_code: 55,
            wrap_dynamic_text: false,
        },
    )
}

/// Projects the DOM `v-text` directive transform for compatibility bridge callers.
pub fn transform_v_text_projection(payload: &Value) -> Value {
    transform_dom_content_directive_projection(
        payload,
        DomContentDirectiveProjection {
            key: "textContent",
            key_loc: None,
            missing_expression_code: 56,
            with_children_code: 57,
            wrap_dynamic_text: true,
        },
    )
}

/// Projects the DOM `v-show` directive transform for compatibility bridge callers.
pub fn transform_show_projection(payload: &Value) -> Value {
    let dir = payload.get("dir").unwrap_or(&Value::Null);
    let exp = dir.get("exp").filter(|exp| !exp.is_null());
    let errors = if exp.is_none() {
        vec![json!({
            "code": 62,
            "loc": "dir",
        })]
    } else {
        Vec::new()
    };
    json!({
        "props": [],
        "errors": errors,
        "needRuntime": "V_SHOW",
    })
}

/// Projects the DOM `v-on` directive transform for compatibility bridge callers.
pub fn transform_on_projection(payload: &Value) -> Value {
    let dir = payload.get("dir").unwrap_or(&Value::Null);
    let mut projection = vuec_vue3_core::transform_on_projection(payload);
    let modifiers = dom_directive_modifiers(dir);
    if modifiers.is_empty() {
        return projection;
    }

    let Some(first_prop) = projection
        .get("props")
        .and_then(Value::as_array)
        .and_then(|props| props.first())
        .cloned()
    else {
        return projection;
    };

    let mut key = first_prop
        .get("key")
        .cloned()
        .unwrap_or_else(|| json!({ "kind": "undefined" }));
    let mut value = first_prop
        .get("value")
        .cloned()
        .unwrap_or_else(|| json!({ "kind": "undefined" }));
    let resolved = dom_resolve_event_modifiers(&key, &modifiers);

    if resolved
        .non_key_modifiers
        .iter()
        .any(|modifier| modifier == "right")
    {
        key = dom_transform_click_projection(key, "onContextmenu");
    }
    if resolved
        .non_key_modifiers
        .iter()
        .any(|modifier| modifier == "middle")
    {
        key = dom_transform_click_projection(key, "onMouseup");
    }

    if !resolved.non_key_modifiers.is_empty() {
        value = dom_helper_call_projection(
            "V_ON_WITH_MODIFIERS",
            vec![
                value,
                json!(dom_json_string_array(&resolved.non_key_modifiers)),
            ],
        );
    }

    if !resolved.key_modifiers.is_empty()
        && (!dom_projection_is_static_expression(&key) || dom_projection_is_keyboard_event(&key))
    {
        value = dom_helper_call_projection(
            "V_ON_WITH_KEYS",
            vec![value, json!(dom_json_string_array(&resolved.key_modifiers))],
        );
    }

    if !resolved.event_option_modifiers.is_empty() {
        let postfix = resolved
            .event_option_modifiers
            .iter()
            .map(|modifier| dom_capitalize(modifier))
            .collect::<String>();
        key = dom_event_option_key_projection(key, &postfix);
    }

    let mut prop = first_prop;
    prop["key"] = key;
    prop["value"] = value;
    projection["props"] = json!([prop]);
    projection
}

/// Projects the DOM `v-model` directive transform for compatibility bridge callers.
pub fn transform_model_projection(payload: &Value) -> Value {
    let dir = payload.get("dir").unwrap_or(&Value::Null);
    let node = payload.get("node").unwrap_or(&Value::Null);
    let context = payload.get("context").unwrap_or(&Value::Null);
    let mut projection = vuec_vue3_core::transform_model_projection(payload);
    if projection
        .get("props")
        .and_then(Value::as_array)
        .is_none_or(Vec::is_empty)
        || json_u64(node, "tagType") == Some(1)
    {
        return dom_normalize_core_model_projection(projection, dir);
    }

    let mut errors = projection
        .get("errors")
        .and_then(Value::as_array)
        .map(|errors| {
            errors
                .iter()
                .filter_map(|error| error.as_u64().map(|code| dom_core_model_error(code, dir)))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if let Some(arg) = dir.get("arg").filter(|arg| !arg.is_null()) {
        errors.push(json!({
            "code": 59,
            "loc": arg.get("loc").cloned().unwrap_or_else(|| dir.get("loc").cloned().unwrap_or(Value::Null)),
        }));
    }

    let mut need_runtime = None::<&'static str>;
    let tag = json_str(node, "tag").unwrap_or("");
    let is_custom_element = json_bool(context, "isCustomElement");
    if matches!(tag, "input" | "textarea" | "select") || is_custom_element {
        let mut helper = "V_MODEL_TEXT";
        let mut invalid_type = false;
        if tag == "input" || is_custom_element {
            match dom_model_input_type(node) {
                DomModelInputType::Dynamic => helper = "V_MODEL_DYNAMIC",
                DomModelInputType::Static("radio") => helper = "V_MODEL_RADIO",
                DomModelInputType::Static("checkbox") => helper = "V_MODEL_CHECKBOX",
                DomModelInputType::Static("file") => {
                    invalid_type = true;
                    errors.push(json!({
                        "code": 60,
                        "loc": dir.get("loc").cloned().unwrap_or(Value::Null),
                    }));
                }
                DomModelInputType::PresentWithoutValue => {}
                DomModelInputType::Static(_) | DomModelInputType::None => {
                    if let Some(value_loc) = dom_model_dynamic_value_binding_loc(node) {
                        errors.push(json!({
                            "code": 61,
                            "loc": value_loc,
                        }));
                    }
                }
            }
        } else if tag == "select" {
            helper = "V_MODEL_SELECT";
        } else if let Some(value_loc) = dom_model_dynamic_value_binding_loc(node) {
            errors.push(json!({
                "code": 61,
                "loc": value_loc,
            }));
        }
        if !invalid_type {
            need_runtime = Some(helper);
        }
    } else {
        errors.push(json!({
            "code": 58,
            "loc": dir.get("loc").cloned().unwrap_or(Value::Null),
        }));
    }

    projection["errors"] = json!(errors);
    projection["props"] = json!(dom_filter_native_model_props(
        projection
            .get("props")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    ));
    if let Some(helper) = need_runtime {
        projection["needRuntime"] = json!(helper);
    }
    projection
}

/// Projects the DOM `Transition` node transform for compatibility bridge callers.
pub fn transform_transition_projection(payload: &Value) -> Value {
    let node = payload.get("node").unwrap_or(&Value::Null);
    let context = payload.get("context").unwrap_or(&Value::Null);
    if !json_bool(context, "isTransition") {
        return json!({ "transform": false });
    }
    let Some(children) = node.get("children").and_then(Value::as_array) else {
        return json!({ "transform": true, "errors": [] });
    };
    if children.is_empty() {
        return json!({ "transform": true, "errors": [] });
    }

    let visible_indices = transition_json_visible_child_indices(children);
    let visible_children = visible_indices
        .iter()
        .filter_map(|index| children.get(*index))
        .collect::<Vec<_>>();
    let mut errors = Vec::new();
    if transition_json_child_sequence_is_invalid(&visible_children, false) {
        if let Some(loc) = transition_json_error_loc(&visible_children) {
            errors.push(json!({
                "code": 63,
                "loc": loc,
            }));
        }
    }

    json!({
        "transform": true,
        "keepChildren": visible_indices,
        "errors": errors,
        "injectPersisted": transition_json_single_child_has_v_show(&visible_children),
    })
}

/// Projects the DOM HTML nesting validator for compatibility bridge callers.
pub fn validate_html_nesting_projection(payload: &Value) -> Value {
    let node = payload.get("node").unwrap_or(&Value::Null);
    let parent = payload.get("parent").unwrap_or(&Value::Null);
    if json_u64(node, "type") != Some(1)
        || json_u64(node, "tagType") != Some(0)
        || json_u64(parent, "type") != Some(1)
        || json_u64(parent, "tagType") != Some(0)
    {
        return json!({ "warnings": [] });
    }
    let parent_tag = json_str(parent, "tag").unwrap_or("");
    let child_tag = json_str(node, "tag").unwrap_or("");
    if is_valid_html_nesting(parent_tag, child_tag) {
        return json!({ "warnings": [] });
    }
    json!({
        "warnings": [{
            "message": format!(
                "<{child_tag}> cannot be child of <{parent_tag}>, according to HTML specifications. This can cause hydration errors or potentially disrupt future functionality."
            ),
            "loc": node.get("loc").cloned().unwrap_or(Value::Null),
        }]
    })
}

/// Returns whether the given parent-child pair is valid according to Vue's DOM nesting table.
pub fn is_valid_html_nesting_projection(payload: &Value) -> Value {
    let parent = json_str(payload, "parent").unwrap_or("");
    let child = json_str(payload, "child").unwrap_or("");
    json!({
        "valid": is_valid_html_nesting(parent, child),
    })
}
