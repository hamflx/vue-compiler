fn process_if_conditions(
    element: Vue2Element,
    parent: &mut Vue2Element,
    diagnostics: &mut DiagnosticSink,
) {
    let mut index = parent.children.len();
    while index > 0 {
        index -= 1;
        match &mut parent.children[index] {
            Vue2Node::Element(prev) => {
                if prev.if_exp.is_some() {
                    prev.if_conditions.push(Vue2IfCondition {
                        exp: element.elseif.clone(),
                        block: Box::new(element),
                    });
                    if let Some((name, if_conditions)) = scoped_slot_if_conditions_update(prev) {
                        if let Some(slot) = parent.scoped_slots.get_mut(&name) {
                            slot.if_conditions = if_conditions;
                        }
                    }
                } else {
                    diagnostics.push(vue2_warning(
                        "W_VUE2_ELSE_WITHOUT_IF",
                        format!(
                            "v-{} used on element <{}> without corresponding v-if.",
                            if let Some(exp) = &element.elseif {
                                format!("else-if=\"{exp}\"")
                            } else {
                                "else".into()
                            },
                            element.tag
                        ),
                        element.span,
                    ));
                }
                return;
            }
            Vue2Node::Text(text) => {
                if text.text != " " {
                    diagnostics.push(vue2_warning(
                        "W_VUE2_TEXT_BETWEEN_IF",
                        format!(
                            "text \"{}\" between v-if and v-else(-if) will be ignored.",
                            text.text.trim()
                        ),
                        text.span,
                    ));
                }
                parent.children.pop();
            }
        }
    }
    diagnostics.push(vue2_warning(
        "W_VUE2_ELSE_WITHOUT_IF",
        format!(
            "v-{} used on element <{}> without corresponding v-if.",
            if let Some(exp) = &element.elseif {
                format!("else-if=\"{exp}\"")
            } else {
                "else".into()
            },
            element.tag
        ),
        element.span,
    ));
}

fn scoped_slot_if_conditions_update(
    element: &Vue2Element,
) -> Option<(String, Vec<Vue2IfCondition>)> {
    if element.slot_scope.is_none() || element.if_conditions.is_empty() {
        return None;
    }
    let name = element
        .slot_target
        .clone()
        .unwrap_or_else(|| "\"default\"".into());
    Some((name, element.if_conditions.clone()))
}

fn sync_scoped_slot_if_conditions(root: Option<&mut Vue2Element>) {
    let Some(root) = root else {
        return;
    };
    sync_scoped_slot_if_conditions_for_element(root);
}

fn sync_scoped_slot_if_conditions_for_element(element: &mut Vue2Element) {
    let updates = element
        .children
        .iter()
        .filter_map(|child| {
            let Vue2Node::Element(child) = child else {
                return None;
            };
            if child.slot_scope.is_none() || child.if_conditions.is_empty() {
                return None;
            }
            let name = child
                .slot_target
                .clone()
                .unwrap_or_else(|| "\"default\"".into());
            Some((name, child.if_conditions.clone()))
        })
        .collect::<Vec<_>>();
    for (name, if_conditions) in updates {
        if let Some(slot) = element.scoped_slots.get_mut(&name) {
            slot.if_conditions = if_conditions;
        }
    }

    for child in &mut element.children {
        if let Vue2Node::Element(child) = child {
            sync_scoped_slot_if_conditions_for_element(child);
        }
    }
    for slot in element.scoped_slots.values_mut() {
        sync_scoped_slot_if_conditions_for_element(slot);
    }
    for condition in element.if_conditions.iter_mut().skip(1) {
        sync_scoped_slot_if_conditions_for_element(&mut condition.block);
    }
}

fn element_generates_empty_data(element: &Vue2Element) -> bool {
    element.key.is_none()
        && element.ref_name.is_none()
        && !element.ref_in_for
        && !element.pre
        && element.component.is_none()
        && element.static_class.is_none()
        && element.class_binding.is_none()
        && element.static_style.is_none()
        && element.style_binding.is_none()
        && element.attrs.is_empty()
        && element.props.is_empty()
        && element.dynamic_attrs.is_empty()
        && element.directives.is_empty()
        && element.events.is_empty()
        && element.native_events.is_empty()
        && element.slot_target.is_none()
        && element.slot_scope.is_none()
        && element.scoped_slots.is_empty()
        && element.model.is_none()
        && element.wrap_data.is_none()
        && element.wrap_listeners.is_none()
        && element.validate.is_none()
        && element.validators.is_empty()
        && !element.inline_template
        && !element.has_bindings
}

fn push_text_node(
    parent: &mut Vue2Element,
    text: &str,
    start: usize,
    end: usize,
    options: &Vue2CompileOptions,
    in_v_pre: bool,
    in_pre_tag: bool,
) {
    let mut text = if is_raw_text_tag(&parent.tag) {
        text.to_string()
    } else {
        decode_html_text_entities(text)
    };
    if matches!(parent.tag.as_str(), "pre" | "textarea")
        && text.starts_with('\n')
        && parent.children.is_empty()
    {
        text.remove(0);
    }
    if text_is_collapsible_whitespace(&text) {
        if !in_pre_tag {
            if options.whitespace.as_deref() == Some("condense") {
                if text.contains('\n') {
                    return;
                }
            } else if parent.children.is_empty() || !options.preserve_whitespace {
                return;
            }
            text = if options.whitespace.as_deref() == Some("condense") {
                condense_whitespace(&text)
            } else {
                " ".into()
            };
            if parent
                .children
                .last()
                .is_some_and(|child| matches!(child, Vue2Node::Text(t) if t.text == " "))
            {
                return;
            }
        }
    } else if options.whitespace.as_deref() == Some("condense") && !in_pre_tag {
        text = condense_whitespace(&text);
    }
    let expression = if parent.pre || in_v_pre {
        None
    } else {
        parse_text(&text, options.delimiters.as_ref())
    };
    parent.children.push(Vue2Node::Text(Vue2Text {
        text,
        expression,
        is_comment: false,
        span: Some(Span::new(FileId(0), start, end)),
        static_node: false,
    }));
}

fn text_is_collapsible_whitespace(value: &str) -> bool {
    value.chars().all(|ch| ch.is_ascii_whitespace())
}

fn mark_static_node(node: &mut Vue2Node, options: &Vue2CompileOptions) -> bool {
    match node {
        Vue2Node::Text(text) => {
            text.static_node = text.expression.is_none();
            text.static_node
        }
        Vue2Node::Element(element) => mark_static_element(element, options),
    }
}

fn mark_static_element(element: &mut Vue2Element, options: &Vue2CompileOptions) -> bool {
    let mut static_node = element.pre
        || (!element.has_bindings
            && element.if_exp.is_none()
            && element.elseif.is_none()
            && !element.else_branch
            && element.for_exp.is_none()
            && !is_built_in_tag(&element.tag)
            && is_reserved_tag_with_options(&element.tag, options)
            && element.ns.is_none()
            && element.key.is_none()
            && element.ref_name.is_none()
            && element.slot_target.is_none()
            && element.component.is_none()
            && element.directives.is_empty()
            && element.events.is_empty()
            && element.dynamic_attrs.is_empty()
            && element.class_binding.is_none()
            && element.style_binding.is_none()
            && element.model.is_none());
    if !is_reserved_tag_with_options(&element.tag, options)
        && element.tag != "slot"
        && !element.inline_template
    {
        element.static_node = false;
        return false;
    }
    for child in &mut element.children {
        if !mark_static_node(child, options) {
            static_node = false;
        }
    }
    for (index, condition) in element.if_conditions.iter_mut().enumerate() {
        let condition_static = mark_static_element(&mut condition.block, options);
        if index == 0 && element.if_exp.is_some() {
            condition.block.static_node = false;
        } else if !condition_static {
            static_node = false;
        }
    }
    element.static_node = static_node;
    static_node
}

fn mark_static_roots(element: &mut Vue2Element, in_for: bool, options: &Vue2CompileOptions) {
    if element.static_node || element.once {
        element.static_in_for = in_for;
    }
    if element.static_node
        && !element.children.is_empty()
        && !(element.children.len() == 1
            && matches!(element.children.first(), Some(Vue2Node::Text(text)) if text.expression.is_none()))
    {
        element.static_root = true;
        return;
    }
    element.static_root = false;
    for child in &mut element.children {
        if let Vue2Node::Element(child) = child {
            mark_static_roots(child, in_for || element.for_exp.is_some(), options);
        }
    }
    for condition in &mut element.if_conditions {
        mark_static_roots(&mut condition.block, in_for, options);
    }
}
