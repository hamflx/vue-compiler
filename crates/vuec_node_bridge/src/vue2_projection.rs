use crate::*;

pub(crate) fn vue2_compile_value(
    compiled: &Vue2CompiledResult,
    options: &Vue2CompileOptions,
) -> Value {
    json!({
        "ast": vue2_public_ast_value(compiled),
        "ast_document": compiled.ast,
        "element_ast": compiled.element_ast,
        "ast_public": vue2_public_ast_value(compiled),
        "element_public_ast": vue2_public_ast_value(compiled),
        "render": compiled.render,
        "staticRenderFns": compiled.static_render_fns,
        "static_render_fns": compiled.static_render_fns,
        "errors": vue2_errors_value(&compiled.errors, options.output_source_range),
        "tips": vue2_tips_value(&compiled.tips, options.output_source_range),
    })
}

pub(crate) fn vue2_public_ast_value(compiled: &Vue2CompiledResult) -> Value {
    match compiled.element_ast.as_ref() {
        Some(element) => vue2_public_element_ast_value(element),
        None => Value::Null,
    }
}

pub(crate) fn vue2_public_element_ast_value(element: &Vue2Element) -> Value {
    let mut object = Map::new();
    object.insert("type".into(), json!(1));
    object.insert("tag".into(), json!(element.tag));
    if let Some(ns) = element.ns.as_ref() {
        object.insert("ns".into(), json!(ns));
    }
    object.insert(
        "attrsList".into(),
        Value::Array(
            element
                .raw_attrs_list
                .iter()
                .map(vue2_public_raw_attr_value)
                .collect(),
        ),
    );
    object.insert("attrsMap".into(), json!(element.attrs_map));
    object.insert(
        "rawAttrsMap".into(),
        Value::Object(
            element
                .raw_attrs_map
                .iter()
                .map(|(name, attr)| (name.clone(), vue2_public_raw_attr_value(attr)))
                .collect(),
        ),
    );
    if !element.attrs.is_empty() {
        object.insert(
            "attrs".into(),
            Value::Array(element.attrs.iter().map(vue2_public_attr_value).collect()),
        );
    }
    if !element.props.is_empty() {
        object.insert(
            "props".into(),
            Value::Array(element.props.iter().map(vue2_public_attr_value).collect()),
        );
    }
    if !element.dynamic_attrs.is_empty() {
        object.insert(
            "dynamicAttrs".into(),
            Value::Array(
                element
                    .dynamic_attrs
                    .iter()
                    .map(vue2_public_attr_value)
                    .collect(),
            ),
        );
    }
    if !element.directives.is_empty() {
        object.insert(
            "directives".into(),
            Value::Array(
                element
                    .directives
                    .iter()
                    .map(vue2_public_directive_value)
                    .collect(),
            ),
        );
    }
    if !element.events.is_empty() {
        object.insert("events".into(), vue2_public_events_value(&element.events));
    }
    if !element.native_events.is_empty() {
        object.insert(
            "nativeEvents".into(),
            vue2_public_events_value(&element.native_events),
        );
    }
    object.insert(
        "children".into(),
        Value::Array(
            element
                .children
                .iter()
                .map(vue2_public_node_ast_value)
                .collect(),
        ),
    );
    object.insert("plain".into(), json!(element.plain));
    insert_true(&mut object, "forbidden", element.forbidden);
    insert_true(&mut object, "pre", element.pre);
    insert_true(&mut object, "once", element.once);
    insert_true(&mut object, "hasBindings", element.has_bindings);
    insert_optional_string(&mut object, "if", element.if_exp.as_ref());
    insert_optional_string(&mut object, "elseif", element.elseif.as_ref());
    insert_true(&mut object, "else", element.else_branch);
    if !element.if_conditions.is_empty() {
        object.insert(
            "ifConditions".into(),
            Value::Array(
                element
                    .if_conditions
                    .iter()
                    .map(vue2_public_if_condition_value)
                    .collect(),
            ),
        );
    }
    insert_optional_string(&mut object, "for", element.for_exp.as_ref());
    insert_optional_string(&mut object, "alias", element.alias.as_ref());
    insert_optional_string(&mut object, "iterator1", element.iterator1.as_ref());
    insert_optional_string(&mut object, "iterator2", element.iterator2.as_ref());
    insert_optional_string(&mut object, "key", element.key.as_ref());
    insert_optional_string(&mut object, "ref", element.ref_name.as_ref());
    insert_true(&mut object, "refInFor", element.ref_in_for);
    insert_optional_string(&mut object, "slotName", element.slot_name.as_ref());
    insert_optional_string(&mut object, "slotTarget", element.slot_target.as_ref());
    insert_true(
        &mut object,
        "slotTargetDynamic",
        element.slot_target_dynamic,
    );
    insert_optional_string(&mut object, "slotScope", element.slot_scope.as_ref());
    insert_true(&mut object, "slotNewSyntax", element.slot_new_syntax);
    if !element.scoped_slots.is_empty() {
        object.insert(
            "scopedSlots".into(),
            Value::Object(
                element
                    .scoped_slots
                    .iter()
                    .map(|(name, slot)| {
                        (
                            vue2_public_slot_key(name),
                            vue2_public_element_ast_value(slot),
                        )
                    })
                    .collect(),
            ),
        );
    }
    insert_optional_string(&mut object, "component", element.component.as_ref());
    insert_true(&mut object, "inlineTemplate", element.inline_template);
    insert_optional_string(&mut object, "staticClass", element.static_class.as_ref());
    insert_optional_string(&mut object, "classBinding", element.class_binding.as_ref());
    insert_optional_string(&mut object, "staticStyle", element.static_style.as_ref());
    insert_optional_string(&mut object, "styleBinding", element.style_binding.as_ref());
    if let Some(model) = element.model.as_ref() {
        object.insert("model".into(), json!(model));
    }
    if let Some(wrap_data) = element.wrap_data.as_ref() {
        object.insert("wrapData".into(), json!(wrap_data));
    }
    insert_optional_string(
        &mut object,
        "wrapListeners",
        element.wrap_listeners.as_ref(),
    );
    if let Some(validate) = element.validate.as_ref() {
        object.insert("validate".into(), json!(validate));
    }
    if !element.validators.is_empty() {
        object.insert("validators".into(), json!(element.validators));
    }
    object.insert("static".into(), json!(element.static_node));
    object.insert("staticRoot".into(), json!(element.static_root));
    object.insert("staticInFor".into(), json!(element.static_in_for));
    Value::Object(object)
}

pub(crate) fn vue2_public_node_ast_value(node: &vuec_vue2::Vue2Node) -> Value {
    match node {
        vuec_vue2::Vue2Node::Element(element) => vue2_public_element_ast_value(element),
        vuec_vue2::Vue2Node::Text(text) => {
            let mut object = Map::new();
            if let Some(expression) = text.expression.as_ref() {
                object.insert("type".into(), json!(2));
                object.insert("expression".into(), json!(expression));
                object.insert(
                    "tokens".into(),
                    json!([{ "@binding": vue27_binding_from_expression(expression) }]),
                );
            } else {
                object.insert("type".into(), json!(3));
            }
            object.insert("text".into(), json!(text.text));
            if text.is_comment {
                object.insert("isComment".into(), json!(true));
            }
            object.insert("static".into(), json!(text.static_node));
            Value::Object(object)
        }
    }
}

pub(crate) fn vue2_public_raw_attr_value(attr: &vuec_vue2::Vue2Attribute) -> Value {
    json!({
        "name": attr.name,
        "value": attr.value,
    })
}

pub(crate) fn vue2_public_attr_value(attr: &vuec_vue2::Vue2Attribute) -> Value {
    json!({
        "name": attr.name,
        "value": attr.value,
        "dynamic": attr.dynamic,
    })
}

pub(crate) fn vue2_public_directive_value(directive: &vuec_vue2::Vue2Directive) -> Value {
    let mut object = Map::new();
    object.insert("name".into(), json!(directive.name));
    object.insert("rawName".into(), json!(directive.raw_name));
    if let Some(value) = directive.value.as_ref() {
        object.insert("value".into(), json!(value));
    }
    if let Some(arg) = directive.arg.as_ref() {
        object.insert("arg".into(), json!(arg));
    }
    insert_true(&mut object, "isDynamicArg", directive.is_dynamic_arg);
    if !directive.modifiers.is_empty() {
        object.insert("modifiers".into(), json!(directive.modifiers));
    }
    Value::Object(object)
}

pub(crate) fn vue2_public_events_value(
    events: &BTreeMap<String, Vec<vuec_vue2::Vue2EventHandler>>,
) -> Value {
    Value::Object(
        events
            .iter()
            .map(|(name, handlers)| {
                let value = if handlers.len() == 1 {
                    vue2_public_event_handler_value(&handlers[0])
                } else {
                    Value::Array(
                        handlers
                            .iter()
                            .map(vue2_public_event_handler_value)
                            .collect(),
                    )
                };
                (name.clone(), value)
            })
            .collect(),
    )
}

pub(crate) fn vue2_public_event_handler_value(handler: &vuec_vue2::Vue2EventHandler) -> Value {
    let mut object = Map::new();
    object.insert("value".into(), json!(handler.value));
    insert_true(&mut object, "dynamic", handler.dynamic);
    if !handler.modifier_order.is_empty() {
        object.insert("modifierOrder".into(), json!(handler.modifier_order));
    }
    insert_true(
        &mut object,
        "hasModifierObject",
        handler.has_modifier_object,
    );
    if !handler.modifiers.is_empty() {
        object.insert("modifiers".into(), json!(handler.modifiers));
    }
    Value::Object(object)
}

pub(crate) fn vue2_public_if_condition_value(condition: &vuec_vue2::Vue2IfCondition) -> Value {
    json!({
        "exp": condition.exp,
        "block": vue2_public_element_ast_value(&condition.block),
    })
}

pub(crate) fn vue2_public_slot_key(name: &str) -> String {
    name.strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(name)
        .to_string()
}

pub(crate) fn insert_optional_string(
    object: &mut Map<String, Value>,
    key: &str,
    value: Option<&String>,
) {
    if let Some(value) = value {
        object.insert(key.into(), json!(value));
    }
}

pub(crate) fn insert_true(object: &mut Map<String, Value>, key: &str, value: bool) {
    if value {
        object.insert(key.into(), json!(true));
    }
}

pub(crate) fn vue2_errors_value(errors: &[Vue2Error], output_source_range: bool) -> Value {
    if output_source_range {
        json!(errors)
    } else {
        json!(errors
            .iter()
            .map(|error| error.msg.clone())
            .collect::<Vec<_>>())
    }
}

pub(crate) fn vue2_tips_value(tips: &[Vue2Warning], output_source_range: bool) -> Value {
    if output_source_range {
        Value::Array(tips.iter().map(vue2_tip_range_value).collect())
    } else {
        json!(tips.iter().map(|tip| tip.msg.clone()).collect::<Vec<_>>())
    }
}

pub(crate) fn vue2_tip_range_value(tip: &Vue2Warning) -> Value {
    let mut value = Map::new();
    value.insert("msg".into(), json!(tip.msg));
    if let Some(start) = tip.start {
        value.insert("start".into(), json!(start));
    }
    if let Some(end) = tip.end {
        value.insert("end".into(), json!(end));
    }
    Value::Object(value)
}
