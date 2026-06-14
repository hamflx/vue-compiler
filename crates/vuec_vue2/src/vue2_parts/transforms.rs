fn process_pre(element: &mut Vue2Element) {
    if remove_attr(element, "v-pre").is_some() {
        element.pre = true;
    }
}

fn process_raw_attrs(element: &mut Vue2Element) {
    element.attrs = element
        .attrs_list
        .iter()
        .map(|attr| Vue2Attribute {
            name: attr.name.clone(),
            value: js_string(&attr.value),
            span: attr.span,
            dynamic: false,
        })
        .collect();
}

fn process_structural_directives(element: &mut Vue2Element, diagnostics: &mut DiagnosticSink) {
    if let Some((value, span)) = remove_attr_with_span(element, "v-for") {
        element.for_span = span;
        if let Some(parsed) = parse_for(&value) {
            element.for_exp = Some(parsed.for_exp);
            element.alias = Some(parsed.alias);
            element.iterator1 = parsed.iterator1;
            element.iterator2 = parsed.iterator2;
        } else {
            diagnostics.push(vue2_warning(
                "W_VUE2_INVALID_FOR",
                format!("Invalid v-for expression: {value}"),
                span,
            ));
        }
    }
    if let Some((value, span)) = remove_attr_with_span(element, "v-if") {
        element.if_exp = Some(value);
        element.if_span = span;
    } else {
        if let Some((_, span)) = remove_attr_with_span(element, "v-else") {
            element.else_branch = true;
            element.else_span = span;
        }
        if let Some((value, span)) = remove_attr_with_span(element, "v-else-if") {
            element.elseif = Some(value);
            element.elseif_span = span;
        }
    }
    if remove_attr(element, "v-once").is_some() {
        element.once = true;
    }
}

fn process_element(
    element: &mut Vue2Element,
    diagnostics: &mut DiagnosticSink,
    options: &Vue2CompileOptions,
) {
    process_key(element, diagnostics);
    process_ref(element);
    process_slot_content(element, diagnostics);
    process_slot_outlet(element, diagnostics);
    process_component(element);
    process_platform_modules(element, diagnostics);
    process_attrs(element, diagnostics, options);
    process_sfc_asset_url_transform(element, options);
}

fn process_key(element: &mut Vue2Element, diagnostics: &mut DiagnosticSink) {
    if let Some((value, span)) = get_binding_attr_with_span(element, "key", true) {
        if element.tag == "template" {
            diagnostics.push(vue2_warning(
                "W_VUE2_TEMPLATE_KEY",
                "<template> cannot be keyed. Place the key on real elements instead.",
                span.or(element.span),
            ));
        }
        element.key = Some(value);
        element.key_span = span;
    }
}

fn process_ref(element: &mut Vue2Element) {
    if let Some(value) = get_binding_attr(element, "ref", true) {
        element.ref_name = Some(value);
        element.ref_in_for = element.for_exp.is_some();
    }
}

fn process_slot_content(element: &mut Vue2Element, diagnostics: &mut DiagnosticSink) {
    if element.tag == "template" {
        element.slot_scope =
            remove_attr(element, "scope").or_else(|| remove_attr(element, "slot-scope"));
    } else if let Some(slot_scope) = remove_attr(element, "slot-scope") {
        element.slot_scope = Some(slot_scope);
    }

    if let Some(slot_target) = get_binding_attr(element, "slot", true) {
        element.slot_target = Some(if slot_target == "\"\"" {
            "\"default\"".into()
        } else {
            slot_target.clone()
        });
        element.slot_target_dynamic = element.attrs_map.contains_key(":slot")
            || element.attrs_map.contains_key("v-bind:slot");
        if element.tag != "template" && element.slot_scope.is_none() {
            element.attrs.push(Vue2Attribute {
                name: "slot".into(),
                value: slot_target,
                span: element.span,
                dynamic: false,
            });
        }
    }

    if let Some((name, value, span)) = remove_slot_binding(element) {
        let (target, dynamic) = slot_name_from_binding(&name);
        let raw = name
            .strip_prefix("v-slot:")
            .or_else(|| name.strip_prefix('#'))
            .unwrap_or("default");
        if raw.starts_with('[') {
            warn_invalid_dynamic_arg(
                raw.trim_start_matches('[').trim_end_matches(']'),
                span,
                diagnostics,
            );
        }
        element.slot_target = Some(target);
        element.slot_target_dynamic = dynamic;
        element.slot_new_syntax = true;
        element.slot_scope = Some(if value.is_empty() {
            "_empty_".into()
        } else {
            value
        });
    }
}

fn process_slot_outlet(element: &mut Vue2Element, diagnostics: &mut DiagnosticSink) {
    if element.tag == "slot" {
        element.slot_name = get_binding_attr(element, "name", true);
        if element.key.is_some() {
            diagnostics.push(vue2_warning(
                "W_VUE2_SLOT_KEY",
                "`key` does not work on <slot> because slots are abstract outlets and can possibly expand into multiple elements.",
                element.key_span.or(element.span),
            ));
        }
    }
}

fn process_component(element: &mut Vue2Element) {
    if let Some(value) = get_binding_attr(element, "is", true) {
        element.component = Some(value);
    }
    if remove_attr(element, "inline-template").is_some() {
        element.inline_template = true;
    }
}

fn pre_transform_dynamic_input_model(
    element: &mut Vue2Element,
    diagnostics: &mut DiagnosticSink,
    options: &Vue2CompileOptions,
) -> bool {
    if element.tag != "input" || !element.attrs_map.contains_key("v-model") {
        return false;
    }

    let type_binding = if element.attrs_map.contains_key(":type")
        || element.attrs_map.contains_key("v-bind:type")
    {
        get_binding_attr(element, "type", false)
    } else if !element.attrs_map.contains_key("type") {
        element
            .attrs_map
            .get("v-bind")
            .map(|binding| format!("({binding}).type"))
    } else {
        None
    };
    let Some(type_binding) = type_binding else {
        return false;
    };

    let original_if = element.if_exp.clone();
    let original_if_span = element.if_span;
    let original_elseif = element.elseif.clone();
    let original_elseif_span = element.elseif_span;
    let original_else = element.else_branch;
    let original_else_span = element.else_span;
    let if_condition_extra = original_if
        .as_ref()
        .map(|condition| format!("&&({condition})"))
        .unwrap_or_default();

    let mut branch0 = clone_input_model_branch(element);
    add_raw_attr(&mut branch0, "type", "checkbox", element.span);
    process_element(&mut branch0, diagnostics, options);
    branch0.if_exp = Some(format!("({type_binding})==='checkbox'{if_condition_extra}"));
    branch0.if_span = original_if_span.or(element.span);
    branch0.elseif = original_elseif;
    branch0.elseif_span = original_elseif_span;
    branch0.else_branch = original_else;
    branch0.else_span = original_else_span;
    branch0.if_conditions = vec![Vue2IfCondition {
        exp: branch0.if_exp.clone(),
        block: Box::new(branch0.clone_without_conditions()),
    }];

    let mut branch1 = clone_input_model_branch(element);
    clear_vue2_for_fields(&mut branch1);
    clear_vue2_condition_fields(&mut branch1);
    add_raw_attr(&mut branch1, "type", "radio", element.span);
    process_element(&mut branch1, diagnostics, options);
    branch0.if_conditions.push(Vue2IfCondition {
        exp: Some(format!("({type_binding})==='radio'{if_condition_extra}")),
        block: Box::new(branch1.clone_without_conditions()),
    });

    let mut branch2 = clone_input_model_branch(element);
    clear_vue2_for_fields(&mut branch2);
    clear_vue2_condition_fields(&mut branch2);
    add_raw_attr(&mut branch2, ":type", &type_binding, element.span);
    process_element(&mut branch2, diagnostics, options);
    branch0.if_conditions.push(Vue2IfCondition {
        exp: original_if,
        block: Box::new(branch2.clone_without_conditions()),
    });

    *element = branch0;
    true
}

fn clone_input_model_branch(element: &Vue2Element) -> Vue2Element {
    let mut clone = element.clone();
    rebuild_attr_maps_from_list(&mut clone);
    clone
}

fn rebuild_attr_maps_from_list(element: &mut Vue2Element) {
    element.attrs_map.clear();
    element.raw_attrs_map.clear();
    for attr in &element.attrs_list {
        element
            .attrs_map
            .insert(attr.name.clone(), attr.value.clone());
        element
            .raw_attrs_map
            .insert(attr.name.clone(), attr.clone());
    }
}

fn add_raw_attr(element: &mut Vue2Element, name: &str, value: &str, span: Option<Span>) {
    let attr = Vue2Attribute {
        name: name.into(),
        value: value.into(),
        span,
        dynamic: false,
    };
    element
        .attrs_map
        .insert(attr.name.clone(), attr.value.clone());
    element
        .raw_attrs_map
        .insert(attr.name.clone(), attr.clone());
    element.attrs_list.push(attr);
}

fn clear_vue2_for_fields(element: &mut Vue2Element) {
    element.for_exp = None;
    element.for_span = None;
    element.alias = None;
    element.iterator1 = None;
    element.iterator2 = None;
}

fn clear_vue2_condition_fields(element: &mut Vue2Element) {
    element.if_exp = None;
    element.if_span = None;
    element.elseif = None;
    element.elseif_span = None;
    element.else_branch = false;
    element.else_span = None;
    element.if_conditions = Vec::new();
}

fn process_platform_modules(element: &mut Vue2Element, diagnostics: &mut DiagnosticSink) {
    if let Some(value) = remove_attr(element, "class") {
        if value.contains("{{") {
            diagnostics.push(vue2_warning(
                "W_VUE2_ATTR_INTERPOLATION",
                "Interpolation inside attributes has been removed. Use v-bind or the colon shorthand instead.",
                element.span,
            ));
        }
        if !value.is_empty() {
            element.static_class = Some(js_string(&normalize_vue2_static_class(&value)));
        }
    }
    if let Some(value) = get_binding_attr(element, "class", false) {
        element.class_binding = Some(value);
    }
    if let Some(value) = remove_attr(element, "style") {
        if !value.is_empty() {
            element.static_style = Some(vue2_static_style_expression(&value));
        }
    }
    if let Some(value) = get_binding_attr(element, "style", false) {
        element.style_binding = Some(value);
    }
}

struct PendingDomModel {
    value: String,
    modifiers: BTreeMap<String, bool>,
}

fn process_attrs(
    element: &mut Vue2Element,
    diagnostics: &mut DiagnosticSink,
    options: &Vue2CompileOptions,
) {
    let mut pending_dom_model = None;
    let list = element.attrs_list.clone();
    for attr in list {
        if !element
            .attrs_list
            .iter()
            .any(|current| current.name == attr.name)
        {
            continue;
        }
        let raw_name = attr.name.clone();
        let value = attr.value.clone();
        if is_directive_name(&raw_name) {
            element.has_bindings = true;
            let (name_no_modifiers, modifiers, modifier_order) = split_modifiers(&raw_name);
            if is_bind_name(&name_no_modifiers) {
                let mut name = bind_arg_name(&name_no_modifiers);
                let is_dynamic = is_dynamic_arg(&name);
                if name.starts_with('[') {
                    warn_invalid_dynamic_arg(
                        name.trim_start_matches('[').trim_end_matches(']'),
                        attr.span,
                        diagnostics,
                    );
                }
                if is_dynamic {
                    name = name
                        .trim_start_matches('[')
                        .trim_end_matches(']')
                        .to_string();
                }
                if value.trim().is_empty() {
                    diagnostics.push(vue2_warning(
                        "W_VUE2_EMPTY_BIND",
                        format!(
                            "The value for a v-bind expression cannot be empty. Found in \"v-bind:{name}\""
                        ),
                        attr.span,
                    ));
                }
                let parsed_value = parse_filters(&value);
                if name.is_empty() {
                    let prop = modifiers.get("prop").copied().unwrap_or(false);
                    let sync = modifiers.get("sync").copied().unwrap_or(false);
                    element.wrap_data = Some(Vue2DataWrap::Bind {
                        value: parsed_value,
                        prop,
                        sync,
                    });
                } else {
                    let target_attr = Vue2Attribute {
                        name: normalize_bound_name(&name, &modifiers, is_dynamic),
                        value: parsed_value.clone(),
                        span: attr.span,
                        dynamic: is_dynamic,
                    };
                    if should_use_prop(element, &target_attr.name, &modifiers, options) {
                        element.props.push(target_attr);
                    } else if is_dynamic {
                        element.dynamic_attrs.push(target_attr);
                    } else {
                        element.attrs.push(target_attr);
                    }
                    if modifiers.get("sync").copied().unwrap_or(false) {
                        let sync_code = gen_assignment_code(&parsed_value, "$event");
                        let camel_name = camelize(&name);
                        add_handler(
                            &mut element.events,
                            format!("update:{camel_name}"),
                            sync_code.clone(),
                            BTreeMap::new(),
                            Vec::new(),
                            false,
                            false,
                            false,
                            attr.span,
                        );
                        let hyphen_name = hyphenate(&name);
                        if hyphen_name != camel_name {
                            add_handler(
                                &mut element.events,
                                format!("update:{hyphen_name}"),
                                sync_code,
                                BTreeMap::new(),
                                Vec::new(),
                                false,
                                false,
                                false,
                                attr.span,
                            );
                        }
                    }
                }
            } else if is_on_name(&name_no_modifiers) {
                let mut name = on_arg_name(&name_no_modifiers);
                let is_dynamic = is_dynamic_arg(&name);
                if name.starts_with('[') {
                    warn_invalid_dynamic_arg(
                        name.trim_start_matches('[').trim_end_matches(']'),
                        attr.span,
                        diagnostics,
                    );
                }
                if is_dynamic {
                    name = name[1..name.len() - 1].to_string();
                }
                let mut modifiers = modifiers;
                let mut modifier_order = modifier_order;
                let has_modifier_object = !modifiers.is_empty();
                let events = if modifiers.remove("native").is_some() {
                    modifier_order.retain(|modifier| modifier != "native");
                    &mut element.native_events
                } else {
                    &mut element.events
                };
                add_handler(
                    events,
                    name,
                    value,
                    modifiers,
                    modifier_order,
                    has_modifier_object,
                    is_dynamic,
                    false,
                    attr.span,
                );
            } else {
                let (name, arg, is_dynamic_arg) = directive_name_and_arg(&name_no_modifiers);
                if is_dynamic_arg || arg.as_ref().is_some_and(|arg| arg.starts_with('[')) {
                    if let Some(arg) = arg.as_ref() {
                        warn_invalid_dynamic_arg(
                            arg.trim_start_matches('[').trim_end_matches(']'),
                            attr.span,
                            diagnostics,
                        );
                    }
                }
                if name == "model" {
                    if is_component(element, options) {
                        gen_component_model(element, &value, &modifiers);
                    } else {
                        add_dom_model_directive(element, &raw_name, &value, &modifiers);
                        pending_dom_model = Some(PendingDomModel {
                            value: value.clone(),
                            modifiers: modifiers.clone(),
                        });
                    }
                }
                if name == "html" {
                    element.props.push(Vue2Attribute {
                        name: "innerHTML".into(),
                        value: format!("_s({value})"),
                        span: attr.span,
                        dynamic: false,
                    });
                } else if name == "text" {
                    element.props.push(Vue2Attribute {
                        name: "textContent".into(),
                        value: format!("_s({value})"),
                        span: attr.span,
                        dynamic: false,
                    });
                } else if name == "bind" && arg.is_none() {
                    element.wrap_data = Some(Vue2DataWrap::Bind {
                        value: value.clone(),
                        prop: modifiers.get("prop").copied().unwrap_or(false),
                        sync: modifiers.get("sync").copied().unwrap_or(false),
                    });
                } else if name == "on" && arg.is_none() {
                    element.wrap_listeners = Some(value.clone());
                } else if !matches!(name.as_str(), "model") {
                    element.directives.push(Vue2Directive {
                        name,
                        raw_name,
                        value: if value.is_empty() { None } else { Some(value) },
                        arg,
                        is_dynamic_arg,
                        modifiers,
                        span: attr.span,
                    });
                }
            }
            remove_attr(element, &attr.name);
        } else {
            if value.contains("{{") {
                diagnostics.push(vue2_warning(
                    "W_VUE2_ATTR_INTERPOLATION",
                    format!("{raw_name}=\"{value}\": Interpolation inside attributes has been removed. Use v-bind or the colon shorthand instead."),
                    attr.span,
                ));
            }
            element.attrs.push(Vue2Attribute {
                name: raw_name.clone(),
                value: js_string(&value),
                span: attr.span,
                dynamic: false,
            });
            if raw_name == "muted" && element.tag == "video" {
                element.props.push(Vue2Attribute {
                    name: raw_name,
                    value: "true".into(),
                    span: attr.span,
                    dynamic: false,
                });
            }
            remove_attr(element, &attr.name);
        }
    }

    if let Some(model) = pending_dom_model {
        gen_dom_model(element, &model.value, &model.modifiers);
    }

    if options.warn && has_duplicate_attr(&element.raw_attrs_list) {
        diagnostics.push(vue2_warning(
            "W_VUE2_DUPLICATE_ATTR",
            "duplicate attribute",
            element.span,
        ));
    }
}
