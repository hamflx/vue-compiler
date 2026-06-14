fn parse_element_tree(
    diagnostics: &mut DiagnosticSink,
    template: &str,
    options: &Vue2CompileOptions,
) -> Option<Vue2Element> {
    let mut tokenizer = HtmlTokenizer::new(template);
    let mut stack: Vec<Vue2Element> = Vec::new();
    let mut root: Option<Vue2Element> = None;
    let mut in_v_pre = false;

    loop {
        let in_pre_tag = stack.iter().any(|element| element.tag == "pre");
        if let Some(parent) = stack.last_mut() {
            if is_text_tag(&parent.tag) {
                let tag = parent.tag.clone();
                let raw = consume_raw_text(template, &mut tokenizer, &tag);
                if !raw.text.is_empty() {
                    push_text_node(
                        parent,
                        &raw.text,
                        raw.start,
                        raw.text_end,
                        options,
                        in_v_pre,
                        in_pre_tag,
                    );
                }
                if raw.has_end_tag {
                    close_until_matching_end_tag(
                        &tag,
                        &mut stack,
                        &mut root,
                        diagnostics,
                        options,
                        &mut in_v_pre,
                    );
                }
                if raw.reached_eof {
                    break;
                }
                continue;
            }
        }

        let token = tokenizer.next_token();
        match token.kind {
            HtmlTokenKind::StartTag {
                name,
                attributes,
                self_closing,
            } => {
                let mut element = create_element(name, attributes, token.start, token.end, options);
                if let Some(namespace) = namespace_for_tag(&element.tag, options) {
                    element.ns = Some(namespace);
                } else if let Some(parent) = stack.last() {
                    element.ns = parent.ns.clone();
                }
                if is_forbidden_tag(&element) {
                    element.forbidden = true;
                    diagnostics.push(vue2_warning(
                        "W_VUE2_FORBIDDEN_TAG",
                        format!(
                            "Templates should only be responsible for mapping the state to the UI. Avoid placing tags with side-effects in your templates, such as <{}>, as they will not be parsed.",
                            element.tag
                        ),
                        element.span,
                    ));
                }

                if !in_v_pre {
                    process_pre(&mut element);
                    in_v_pre = element.pre;
                }
                if in_v_pre {
                    process_raw_attrs(&mut element);
                } else {
                    process_structural_directives(&mut element, diagnostics);
                    if !pre_transform_dynamic_input_model(&mut element, diagnostics, options) {
                        process_element(&mut element, diagnostics, options);
                    }
                }
                if self_closing || is_unary_tag(&element.tag) {
                    close_element(
                        element,
                        &mut stack,
                        &mut root,
                        diagnostics,
                        options,
                        &mut in_v_pre,
                    );
                } else {
                    stack.push(element);
                }
            }
            HtmlTokenKind::EndTag { name } => {
                close_until_matching_end_tag(
                    &name,
                    &mut stack,
                    &mut root,
                    diagnostics,
                    options,
                    &mut in_v_pre,
                );
            }
            HtmlTokenKind::Text(text) | HtmlTokenKind::Cdata(text) => {
                if let Some(parent) = stack.last_mut() {
                    push_text_node(
                        parent,
                        &text,
                        token.start,
                        token.end,
                        options,
                        in_v_pre,
                        in_pre_tag,
                    );
                } else if !text.trim().is_empty() {
                    let message = if text == template {
                        "Component template requires a root element, rather than just text."
                            .to_string()
                    } else {
                        format!(
                            "text \"{}\" outside root element will be ignored.",
                            text.trim()
                        )
                    };
                    diagnostics.push(vue2_warning(
                        "W_VUE2_TEXT_OUTSIDE_ROOT",
                        message,
                        Some(Span::new(FileId(0), token.start, token.end)),
                    ));
                }
            }
            HtmlTokenKind::Comment(text) if options.comments => {
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(Vue2Node::Text(Vue2Text {
                        text,
                        expression: None,
                        is_comment: true,
                        span: Some(Span::new(FileId(0), token.start, token.end)),
                        static_node: true,
                    }));
                }
            }
            HtmlTokenKind::Comment(_)
            | HtmlTokenKind::BogusQuestionTag
            | HtmlTokenKind::Doctype(_) => {}
            HtmlTokenKind::Eof => break,
        }
    }

    while let Some(element) = stack.pop() {
        diagnostics.push(vue2_error(
            "E_VUE2_UNCLOSED_TAG",
            format!("tag <{}> has no matching end tag.", element.tag),
            element.span,
        ));
        close_element(
            element,
            &mut stack,
            &mut root,
            diagnostics,
            options,
            &mut in_v_pre,
        );
    }

    if let Some(root) = root.as_mut() {
        mark_ref_in_for(root, false);
    }

    root
}

struct RawText {
    text: String,
    start: usize,
    text_end: usize,
    has_end_tag: bool,
    reached_eof: bool,
}

fn consume_raw_text(template: &str, tokenizer: &mut HtmlTokenizer<'_>, tag: &str) -> RawText {
    let start = tokenizer.cursor();
    let close_tag = format!("</{tag}");
    let lower = template[start..].to_ascii_lowercase();
    let Some(relative_end) = lower.find(&close_tag) else {
        tokenizer.set_cursor(template.len());
        return RawText {
            text: template[start..].to_string(),
            start,
            text_end: template.len(),
            has_end_tag: false,
            reached_eof: true,
        };
    };
    let text_end = start + relative_end;
    tokenizer.set_cursor(text_end);
    let end_tag = tokenizer.next_token();
    RawText {
        text: template[start..text_end].to_string(),
        start,
        text_end,
        has_end_tag: matches!(end_tag.kind, HtmlTokenKind::EndTag { ref name } if name.eq_ignore_ascii_case(tag)),
        reached_eof: false,
    }
}

fn close_until_matching_end_tag(
    name: &str,
    stack: &mut Vec<Vue2Element>,
    root: &mut Option<Vue2Element>,
    diagnostics: &mut DiagnosticSink,
    options: &Vue2CompileOptions,
    in_v_pre: &mut bool,
) {
    let Some(mut index) = stack.iter().rposition(|element| element.tag == name) else {
        return;
    };
    while stack.len() > index + 1 {
        let Some(element) = stack.pop() else {
            return;
        };
        diagnostics.push(vue2_error(
            "E_VUE2_UNCLOSED_TAG",
            format!("tag <{}> has no matching end tag.", element.tag),
            element.span,
        ));
        if element.pre {
            *in_v_pre = false;
        }
        close_element(element, stack, root, diagnostics, options, in_v_pre);
        index = index.min(stack.len());
    }
    let Some(element) = stack.pop() else {
        return;
    };
    if element.pre {
        *in_v_pre = false;
    }
    close_element(element, stack, root, diagnostics, options, in_v_pre);
}

fn create_element(
    tag: String,
    attributes: Vec<HtmlAttribute>,
    start: usize,
    end: usize,
    options: &Vue2CompileOptions,
) -> Vue2Element {
    let attrs_list = attributes
        .into_iter()
        .map(|attr| Vue2Attribute {
            value: decode_vue2_attr_entities(
                &tag,
                &attr.name,
                &attr.value.unwrap_or_default(),
                options,
            ),
            name: attr.name,
            span: Some(Span::new(FileId(0), attr.start, attr.end)),
            dynamic: false,
        })
        .collect::<Vec<_>>();
    let mut attrs_map = BTreeMap::new();
    let mut raw_attrs_map = BTreeMap::new();
    for attr in &attrs_list {
        attrs_map.insert(attr.name.clone(), attr.value.clone());
        raw_attrs_map.insert(attr.name.clone(), attr.clone());
    }
    Vue2Element {
        tag,
        raw_attrs_list: attrs_list.clone(),
        attrs_list,
        attrs_map,
        raw_attrs_map,
        attrs: Vec::new(),
        props: Vec::new(),
        dynamic_attrs: Vec::new(),
        directives: Vec::new(),
        events: BTreeMap::new(),
        native_events: BTreeMap::new(),
        children: Vec::new(),
        span: Some(Span::new(FileId(0), start, end)),
        ns: None,
        plain: false,
        forbidden: false,
        pre: false,
        once: false,
        has_bindings: false,
        if_exp: None,
        if_span: None,
        elseif: None,
        elseif_span: None,
        else_branch: false,
        else_span: None,
        if_conditions: Vec::new(),
        for_exp: None,
        for_span: None,
        alias: None,
        iterator1: None,
        iterator2: None,
        key: None,
        key_span: None,
        ref_name: None,
        ref_in_for: false,
        slot_name: None,
        slot_target: None,
        slot_target_dynamic: false,
        slot_scope: None,
        slot_new_syntax: false,
        scoped_slots: BTreeMap::new(),
        component: None,
        inline_template: false,
        static_class: None,
        class_binding: None,
        static_style: None,
        style_binding: None,
        model: None,
        wrap_data: None,
        wrap_listeners: None,
        validate: None,
        validators: Vec::new(),
        static_node: false,
        static_root: false,
        static_in_for: false,
        static_processed: false,
        once_processed: false,
        for_processed: false,
        if_processed: false,
    }
}

fn close_element(
    mut element: Vue2Element,
    stack: &mut [Vue2Element],
    root: &mut Option<Vue2Element>,
    diagnostics: &mut DiagnosticSink,
    options: &Vue2CompileOptions,
    in_v_pre: &mut bool,
) {
    if element.pre {
        *in_v_pre = false;
    }
    let in_pre_tag = element.tag == "pre" || stack.iter().any(|ancestor| ancestor.tag == "pre");
    if !in_pre_tag {
        trim_ending_whitespace(&mut element);
    }
    normalize_component_v_slot(&mut element);
    cleanup_scoped_slot_children(&mut element, in_pre_tag);
    element.plain = element_generates_empty_data(&element);

    let parent_in_pre_tag = stack.iter().any(|ancestor| ancestor.tag == "pre");
    if let Some(parent) = stack.last_mut() {
        if element.elseif.is_some() || element.else_branch {
            process_if_conditions(element, parent, diagnostics);
        } else {
            let mut element = element;
            if element.if_exp.is_some() && element.if_conditions.is_empty() {
                element.if_conditions = vec![Vue2IfCondition {
                    exp: element.if_exp.clone(),
                    block: Box::new(element.clone_without_conditions()),
                }];
            }
            if let Some(slot_scope) = element.slot_scope.clone() {
                let name = element
                    .slot_target
                    .clone()
                    .unwrap_or_else(|| "\"default\"".into());
                let mut scoped = element.clone();
                scoped.slot_scope = Some(slot_scope);
                parent.scoped_slots.insert(name, scoped);
            }
            parent.children.push(Vue2Node::Element(Box::new(element)));
        }
        if !parent_in_pre_tag {
            trim_ending_whitespace(parent);
        }
        return;
    }

    if let Some(existing) = root.as_mut() {
        if existing.if_exp.is_some() && (element.elseif.is_some() || element.else_branch) {
            if element.for_exp.is_some() {
                diagnostics.push(vue2_warning(
                    "W_VUE2_FOR_ROOT",
                    "Cannot use v-for on stateful component root element because it renders multiple elements.",
                    element.span,
                ));
            }
            existing.if_conditions.push(Vue2IfCondition {
                exp: element.elseif.clone(),
                block: Box::new(element),
            });
        } else if !is_ignorable_root_whitespace(&element) {
            push_vue2_warning_once(
                diagnostics,
                "W_VUE2_MULTIPLE_ROOTS",
                "Component template should contain exactly one root element. If you are using v-if on multiple elements, use v-else-if to chain them instead.",
                element.span,
            );
        }
    } else {
        if matches!(element.tag.as_str(), "slot" | "template") {
            diagnostics.push(vue2_warning(
                "W_VUE2_INVALID_ROOT",
                format!(
                    "Cannot use <{}> as component root element because it may contain multiple nodes.",
                    element.tag
                ),
                element.span,
            ));
        }
        if element.for_exp.is_some() {
            diagnostics.push(vue2_warning(
                "W_VUE2_FOR_ROOT",
                "Cannot use v-for on stateful component root element because it renders multiple elements.",
                element.span,
            ));
        }
        let mut element = element;
        if element.if_exp.is_some() && element.if_conditions.is_empty() {
            element.if_conditions = vec![Vue2IfCondition {
                exp: element.if_exp.clone(),
                block: Box::new(element.clone_without_conditions()),
            }];
        }
        *root = Some(element);
    }

    if root.is_none() && options.warn {
        diagnostics.push(vue2_error(
            "E_VUE2_NO_ROOT",
            "Component template requires a root element, rather than just text.",
            None,
        ));
    }
}

fn is_ignorable_root_whitespace(_element: &Vue2Element) -> bool {
    false
}

fn normalize_component_v_slot(element: &mut Vue2Element) {
    if element.tag == "template" || !element.slot_new_syntax || element.slot_scope.is_none() {
        return;
    }

    let slot_target = element
        .slot_target
        .clone()
        .unwrap_or_else(|| "\"default\"".into());
    let slot_target_dynamic = element.slot_target_dynamic;
    let slot_scope = element.slot_scope.clone();
    let span = element.span.unwrap_or_else(|| Span::new(FileId(0), 0, 0));
    let mut slot_container = create_element(
        "template".into(),
        Vec::new(),
        span.start.0,
        span.end.0,
        &Vue2CompileOptions::default(),
    );
    slot_container.slot_target = Some(slot_target.clone());
    slot_container.slot_target_dynamic = slot_target_dynamic;
    slot_container.slot_scope = slot_scope;
    slot_container.slot_new_syntax = true;
    slot_container.children = std::mem::take(&mut element.children)
        .into_iter()
        .filter(|child| {
            !matches!(
                child,
                Vue2Node::Element(child_element) if child_element.slot_scope.is_some()
            )
        })
        .collect();

    element.scoped_slots.insert(slot_target, slot_container);
    element.slot_target = None;
    element.slot_target_dynamic = false;
    element.slot_scope = None;
    element.slot_new_syntax = false;
}

fn push_vue2_warning_once(
    diagnostics: &mut DiagnosticSink,
    code: &str,
    message: impl Into<String>,
    span: Option<Span>,
) {
    if diagnostics
        .as_slice()
        .iter()
        .any(|diagnostic| diagnostic.code == code)
    {
        return;
    }
    diagnostics.push(vue2_warning(code, message, span));
}

fn cleanup_scoped_slot_children(element: &mut Vue2Element, in_pre_tag: bool) {
    element.children.retain(|child| {
        !matches!(
            child,
            Vue2Node::Element(child_element) if child_element.slot_scope.is_some()
        )
    });
    if !in_pre_tag {
        trim_ending_whitespace(element);
    }
}

fn mark_ref_in_for(element: &mut Vue2Element, in_for: bool) {
    let current_in_for = in_for || element.for_exp.is_some();
    if element.ref_name.is_some() && current_in_for {
        element.ref_in_for = true;
    }
    for child in &mut element.children {
        if let Vue2Node::Element(child) = child {
            mark_ref_in_for(child, current_in_for);
        }
    }
    for slot in element.scoped_slots.values_mut() {
        mark_ref_in_for(slot, current_in_for);
    }
    for condition in &mut element.if_conditions {
        mark_ref_in_for(&mut condition.block, in_for);
    }
}

fn collect_element_warnings(
    element: Option<&Vue2Element>,
    options: &Vue2CompileOptions,
    diagnostics: &mut DiagnosticSink,
) {
    let Some(element) = element else {
        return;
    };
    collect_element_warning_node(element, options, diagnostics);
}

fn collect_element_warning_node(
    element: &Vue2Element,
    options: &Vue2CompileOptions,
    diagnostics: &mut DiagnosticSink,
) {
    if element.inline_template && element.children.len() != 1 {
        diagnostics.push(vue2_warning(
            "W_VUE2_INLINE_TEMPLATE_CHILDREN",
            "Inline-template components must have exactly one child element.",
            element.span,
        ));
    }
    if element.for_exp.is_some()
        && is_component(element, options)
        && element.tag != "slot"
        && element.tag != "template"
        && element.key.is_none()
    {
        diagnostics.push(vue2_tip(
            "T_VUE2_COMPONENT_V_FOR_KEY",
            vue2_component_v_for_key_tip(element),
            element.for_span,
        ));
    }
    if element.tag == "transition-group" {
        for child in &element.children {
            let Vue2Node::Element(child) = child else {
                continue;
            };
            let Some(key) = child.key.as_deref() else {
                continue;
            };
            if child.for_exp.is_some()
                && (child.iterator1.as_deref() == Some(key)
                    || child.iterator2.as_deref() == Some(key))
            {
                diagnostics.push(vue2_warning(
                    "W_VUE2_TRANSITION_GROUP_INDEX_KEY",
                    "Do not use v-for index as key on <transition-group> children, this is the same as not using keys.",
                    child.span,
                ));
            }
        }
    }
    for child in &element.children {
        if let Vue2Node::Element(child) = child {
            collect_element_warning_node(child, options, diagnostics);
        }
    }
    for slot in element.scoped_slots.values() {
        collect_element_warning_node(slot, options, diagnostics);
    }
    for condition in element.if_conditions.iter().skip(1) {
        collect_element_warning_node(&condition.block, options, diagnostics);
    }
}

fn vue2_component_v_for_key_tip(element: &Vue2Element) -> String {
    let alias = element.alias.as_deref().unwrap_or("");
    let exp = element.for_exp.as_deref().unwrap_or("");
    format!(
        "<{} v-for=\"{} in {}\">: component lists rendered with v-for should have explicit keys. See https://v2.vuejs.org/v2/guide/list.html#key for more info.",
        element.tag, alias, exp
    )
}
