pub(crate) fn vue3_is_text_node(node: &Value) -> bool {
    matches!(json_node_type(node), Some(2 | 5))
}

pub(crate) fn vue3_text_compound(children: &[Value]) -> Value {
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
            .unwrap_or(Value::Null),
    })
}

pub(crate) fn vue3_text_has_untransformed_custom_directive(node: &Value, context: &Value) -> bool {
    let transformed = context
        .get("directiveTransforms")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    node.get("props")
        .and_then(Value::as_array)
        .is_some_and(|props| {
            props.iter().any(|prop| {
                json_node_type(prop) == Some(7)
                    && json_str(prop, "name")
                        .is_some_and(|name| !transformed.iter().any(|known| *known == name))
            })
        })
}

pub(crate) fn component_slot_projections(children: &[Value]) -> Vec<Value> {
    let mut slots = Vec::new();
    let mut plain_indices = Vec::new();
    for (index, child) in children.iter().enumerate() {
        if json_str(child, "tag") == Some("template") {
            if let Some(slot_name) = template_slot_name(child) {
                slots.push(json!({
                    "name": slot_name,
                    "indices": [index],
                    "unwrapTemplate": true,
                }));
                continue;
            }
        }
        plain_indices.push(index);
    }
    if !plain_indices.is_empty() {
        slots.insert(
            0,
            json!({
                "name": "default",
                "indices": plain_indices,
                "unwrapTemplate": false,
            }),
        );
    }
    slots
}

pub(crate) fn template_slot_name(node: &Value) -> Option<&str> {
    node.get("props")
        .and_then(Value::as_array)?
        .iter()
        .find_map(|prop| {
            if json_str(prop, "name") == Some("slot") {
                prop.get("arg")
                    .and_then(|arg| arg.get("content"))
                    .and_then(Value::as_str)
            } else {
                None
            }
        })
}

pub(crate) fn inline_template_ref_projections(props: &[Value], context: &Value) -> Vec<Value> {
    if !json_bool(context, "inline") {
        return Vec::new();
    }
    let Some(binding_metadata) = context.get("bindingMetadata").and_then(Value::as_object) else {
        return Vec::new();
    };
    props
        .iter()
        .filter_map(|prop| {
            if json_str(prop, "kind") != Some("attribute") || json_str(prop, "name") != Some("ref")
            {
                return None;
            }
            let content = json_str(prop, "value")?;
            let binding = binding_metadata.get(content).and_then(Value::as_str)?;
            if matches!(binding, "setup-let" | "setup-ref" | "setup-maybe-ref") {
                Some(json!({ "content": content }))
            } else {
                None
            }
        })
        .collect()
}

pub(crate) fn prop_requires_normalize_style(prop: &Value) -> bool {
    json_str(prop, "kind") == Some("directiveProp")
        && json_str(prop, "name") == Some("style")
        && (json_bool(prop, "valueStartsWithArray")
            || prop.get("valueType").and_then(Value::as_u64) == Some(17))
}

pub(crate) fn prop_requires_normalize_class(prop: &Value) -> bool {
    json_str(prop, "kind") == Some("directiveProp")
        && json_str(prop, "name") == Some("class")
        && !json_bool(prop, "valueStatic")
}

pub(crate) fn prop_output_name(prop: &Value) -> Option<&str> {
    match json_str(prop, "kind") {
        Some("attribute") | Some("directiveProp") => json_str(prop, "name"),
        _ => None,
    }
}

pub(crate) fn prop_name_is_event_handler(name: &str) -> bool {
    name.starts_with("on")
        && name
            .chars()
            .nth(2)
            .is_some_and(|ch| !matches!(ch, 'a'..='z' | '-' | ':'))
}

pub(crate) fn prop_name_is_reserved(name: &str) -> bool {
    matches!(name, "key" | "ref" | "ref_for" | "ref_key")
        || name.starts_with("onVnode")
        || name.starts_with("onUpdate:")
}

pub(crate) fn resolve_component_is_prop(node: &Value) -> Option<&Value> {
    node.get("props")
        .and_then(Value::as_array)?
        .iter()
        .find(|prop| {
            if json_node_type(prop) == Some(6) {
                json_str(prop, "name") == Some("is")
            } else {
                json_str(prop, "name") == Some("bind")
                    && prop.get("arg").is_some_and(|arg| {
                        json_bool(arg, "isStatic") && json_str(arg, "content") == Some("is")
                    })
            }
        })
}

pub(crate) fn resolve_component_is_prop_expression(prop: &Value, context: &Value) -> Option<Value> {
    if json_node_type(prop) == Some(6) {
        return prop
            .get("value")
            .and_then(|value| json_str(value, "content").map(|content| (value, content)))
            .map(|(value, content)| {
                json!({
                    "kind": "simple",
                    "content": content,
                    "isStatic": true,
                    "constType": 3,
                    "loc": value.get("loc").cloned().unwrap_or(Value::Null),
                })
            });
    }

    if let Some(exp) = prop.get("exp").filter(|value| !value.is_null()) {
        return Some(exp.clone());
    }

    let content = if json_bool(context, "prefixIdentifiers") {
        rewrite_js_like_expression("is", &vue3_options_from_transform_context(context))
    } else {
        "is".to_string()
    };
    Some(json!({
        "kind": "simple",
        "content": content,
        "isStatic": false,
        "constType": 0,
        "loc": prop
            .get("arg")
            .and_then(|arg| arg.get("loc"))
            .cloned()
            .unwrap_or(Value::Null),
    }))
}

pub(crate) fn vue3_core_component_helper(tag: &str) -> Option<&'static str> {
    match tag {
        "Teleport" | "teleport" => Some("TELEPORT"),
        "Suspense" | "suspense" => Some("SUSPENSE"),
        "KeepAlive" | "keep-alive" => Some("KEEP_ALIVE"),
        "BaseTransition" | "base-transition" => Some("BASE_TRANSITION"),
        _ => None,
    }
}

pub(crate) fn resolve_setup_reference(name: &str, context: &Value) -> Option<Value> {
    let bindings = context.get("bindingMetadata")?;
    if context.get("isScriptSetup").and_then(Value::as_bool) == Some(false) {
        return None;
    }

    let camel_name = camelize(name);
    let pascal_name = capitalize(&camel_name);
    let from_const = binding_with_type(
        bindings,
        &[name, &camel_name, &pascal_name],
        &["setup-const", "setup-reactive-const", "literal-const"],
    );
    if let Some(name) = from_const {
        return Some(json!({
            "kind": "expression",
            "content": if json_bool(context, "inline") {
                name.to_string()
            } else {
                format!("$setup[{}]", quote_string(name))
            },
        }));
    }

    let from_maybe_ref = binding_with_type(
        bindings,
        &[name, &camel_name, &pascal_name],
        &["setup-let", "setup-ref", "setup-maybe-ref"],
    );
    if let Some(name) = from_maybe_ref {
        return Some(json!({
            "kind": "expression",
            "content": if json_bool(context, "inline") {
                format!("_unref({name})")
            } else {
                format!("$setup[{}]", quote_string(name))
            },
            "helpers": if json_bool(context, "inline") {
                json!(["UNREF"])
            } else {
                json!([])
            },
        }));
    }

    let from_props = binding_with_type(bindings, &[name, &camel_name, &pascal_name], &["props"]);
    if let Some(name) = from_props {
        return Some(json!({
            "kind": "expression",
            "content": format!(
                "_unref({}[{}])",
                if json_bool(context, "inline") { "__props" } else { "$props" },
                quote_string(name),
            ),
            "helpers": ["UNREF"],
        }));
    }

    None
}

pub(crate) fn binding_with_type<'a>(
    bindings: &'a Value,
    names: &[&'a str],
    types: &[&str],
) -> Option<&'a str> {
    names.iter().copied().find(|name| {
        bindings
            .get(*name)
            .and_then(Value::as_str)
            .is_some_and(|binding_type| types.contains(&binding_type))
    })
}
