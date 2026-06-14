pub(crate) fn lower_vue3_element_to_hir_kind(
    element: &Vue3Element,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    js: &mut JsAstStore,
    source_type: oxc_span::SourceType,
) -> HirNodeKind {
    if element.tag_type == Vue3ElementType::SlotOutlet {
        return HirNodeKind::SlotOutlet(HirSlotOutlet {
            name: element.props.iter().find_map(vue3_static_slot_outlet_name),
            props: lower_vue3_props_to_hir(&element.props, ast_node, js, source_type),
        });
    }

    let props = lower_vue3_props_to_hir(&element.props, ast_node, js, source_type);
    if element.tag_type == Vue3ElementType::Component {
        HirNodeKind::Component(vuec_ast::HirComponent {
            name: element.tag.clone(),
            props,
        })
    } else {
        HirNodeKind::Element(HirElement {
            tag: HirTag::Native(element.tag.clone()),
            namespace: element.ns,
            props,
            directives: lower_vue3_directives_to_hir(&element.props, ast_node, js, source_type),
            constness: HirConstness::Dynamic,
        })
    }
}

pub(crate) struct Vue3ForLoweringParts {
    pub(crate) source: JsExprId,
    pub(crate) value_alias: JsPatternId,
    pub(crate) key_alias: Option<JsPatternId>,
    pub(crate) index_alias: Option<JsPatternId>,
}

pub(crate) fn lower_vue3_for_directive(
    directive: &Vue3Directive,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    js: &mut JsAstStore,
    source_type: oxc_span::SourceType,
) -> Option<Vue3ForLoweringParts> {
    let expression = directive.exp.as_ref()?.source_string();
    let parsed = parse_vue3_for_expression(&expression)?;
    let span = directive.exp_span.or_else(|| ast_node.span.source());
    let source = js.register_expr(
        parsed.source.content.clone(),
        vue3_sub_span_or_fallback(span, parsed.source.start, parsed.source.end),
        source_type,
    );
    let value = parsed.value?;
    let value_alias = js.register_pattern(
        value.content,
        vue3_sub_span_or_fallback(span, value.start, value.end),
        source_type,
    );
    let key_alias = parsed.key.map(|part| {
        js.register_pattern(
            part.content,
            vue3_sub_span_or_fallback(span, part.start, part.end),
            source_type,
        )
    });
    let index_alias = parsed.index.map(|part| {
        js.register_pattern(
            part.content,
            vue3_sub_span_or_fallback(span, part.start, part.end),
            source_type,
        )
    });

    Some(Vue3ForLoweringParts {
        source,
        value_alias,
        key_alias,
        index_alias,
    })
}

pub(crate) fn lower_vue3_optional_condition(
    directive: &Vue3Directive,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    js: &mut JsAstStore,
    source_type: oxc_span::SourceType,
) -> Option<JsExprId> {
    directive.exp.as_ref().map(|exp| {
        register_vue3_expression_with_span(
            js,
            exp,
            directive.exp_span.or_else(|| ast_node.span.source()),
            source_type,
        )
    })
}

pub(crate) fn vue3_branch_condition(
    element: &Vue3Element,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    js: &mut JsAstStore,
    source_type: oxc_span::SourceType,
) -> Option<JsExprId> {
    directive_by_name(element, "if")
        .or_else(|| directive_by_name(element, "else-if"))
        .and_then(|dir| lower_vue3_optional_condition(dir, ast_node, js, source_type))
}

pub(crate) fn vue3_sub_span_or_fallback(base: Option<Span>, start: usize, end: usize) -> Span {
    if let Some(base) = base {
        Span::new(base.file_id, base.start.0 + start, base.start.0 + end)
    } else {
        Span::new(FileId(0), start, end)
    }
}

pub(crate) fn lower_vue3_ssr_v_show(
    hir_id: NodeId,
    state: &Vue3SsrLoweringState,
) -> Option<JsExprId> {
    match state.hir.node(hir_id).map(|node| &node.kind) {
        Some(HirNodeKind::Element(element)) => element
            .directives
            .iter()
            .find(|directive| directive.name == "show")
            .and_then(|directive| directive.expression),
        _ => None,
    }
}

pub(crate) fn lower_vue3_ssr_v_model(
    element: &Vue3Element,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    state: &mut Vue3SsrLoweringState,
) -> Option<Vue3SsrModel> {
    if element.tag == "option" {
        let expression = *state.select_model_stack.last()?;
        if vue3_ssr_has_static_attr(element, "selected") {
            return None;
        }
        return Some(Vue3SsrModel {
            expression,
            kind: Vue3SsrModelKind::SelectOption {
                value: vue3_ssr_value_binding(element, ast_node, state),
            },
        });
    }
    if !matches!(element.tag.as_str(), "input" | "textarea") {
        return None;
    }

    let expression = vue3_ssr_v_model_expression(element, ast_node, state)?;
    let kind = match element.tag.as_str() {
        "input" => vue3_ssr_input_v_model_kind(element, ast_node, state),
        "textarea" => Some(Vue3SsrModelKind::Textarea),
        _ => None,
    }?;
    Some(Vue3SsrModel { expression, kind })
}

pub(crate) fn lower_vue3_ssr_content(
    element: &Vue3Element,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    state: &mut Vue3SsrLoweringState,
) -> Option<Vue3SsrContent> {
    if let Some(expression) = vue3_ssr_directive_expression(element, "html", ast_node, state) {
        return Some(Vue3SsrContent::Html { expression });
    }
    if let Some(expression) = vue3_ssr_directive_expression(element, "text", ast_node, state) {
        return Some(Vue3SsrContent::Text { expression });
    }
    if element.tag == "textarea" {
        return vue3_ssr_dynamic_textarea_value(element, ast_node, state)
            .map(|expression| Vue3SsrContent::Text { expression });
    }
    None
}

pub(crate) fn vue3_ssr_dynamic_textarea_value(
    element: &Vue3Element,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    state: &mut Vue3SsrLoweringState,
) -> Option<JsExprId> {
    element.props.iter().find_map(|prop| match prop {
        Vue3Prop::Directive(dir)
            if dir.name == "bind"
                && !dir.is_dynamic_arg
                && dir
                    .arg
                    .as_ref()
                    .is_some_and(|arg| arg.source_string() == "value") =>
        {
            let expression = dir.exp.as_ref()?;
            Some(register_or_reuse_vue3_expression_with_span(
                &mut state.js,
                expression,
                dir.exp_span.or_else(|| ast_node.span.source()),
                state.source_type,
            ))
        }
        _ => None,
    })
}

pub(crate) fn vue3_ssr_static_textarea_value(element: &Vue3Element) -> Option<String> {
    element.props.iter().find_map(|prop| match prop {
        Vue3Prop::Attribute(attr) if attr.name == "value" => attr.value.clone(),
        _ => None,
    })
}

pub(crate) fn vue3_ssr_has_object_v_bind(element: &Vue3Element) -> bool {
    element.props.iter().any(|prop| {
        matches!(
            prop,
            Vue3Prop::Directive(dir) if dir.name == "bind" && dir.arg.is_none()
        )
    })
}

pub(crate) fn vue3_ssr_static_textarea_fallback(
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    ast: &Vue3Ast,
) -> Option<String> {
    let mut fallback = String::new();
    for child in &ast_node.children {
        let node = ast.node(*child)?;
        match &node.kind {
            Vue3AstKind::Text(text) => fallback.push_str(&text.value),
            Vue3AstKind::Comment(_) => {}
            _ => return None,
        }
    }
    Some(fallback)
}

pub(crate) fn vue3_ssr_directive_expression(
    element: &Vue3Element,
    name: &str,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    state: &mut Vue3SsrLoweringState,
) -> Option<JsExprId> {
    let directive = directive_by_name(element, name)?;
    let expression = directive.exp.as_ref()?;
    if expression.source_string().trim().is_empty() {
        return None;
    }
    Some(register_or_reuse_vue3_expression_with_span(
        &mut state.js,
        expression,
        directive.exp_span.or_else(|| ast_node.span.source()),
        state.source_type,
    ))
}

pub(crate) fn vue3_ssr_v_model_expression(
    element: &Vue3Element,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    state: &mut Vue3SsrLoweringState,
) -> Option<JsExprId> {
    let directive = directive_by_name(element, "model")?;
    let expression = directive.exp.as_ref()?;
    if expression.source_string().trim().is_empty() {
        return None;
    }
    Some(register_or_reuse_vue3_expression_with_span(
        &mut state.js,
        expression,
        directive.exp_span.or_else(|| ast_node.span.source()),
        state.source_type,
    ))
}

pub(crate) fn vue3_ssr_input_v_model_kind(
    element: &Vue3Element,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    state: &mut Vue3SsrLoweringState,
) -> Option<Vue3SsrModelKind> {
    if vue3_ssr_has_dynamic_key_v_bind(element) {
        return Some(Vue3SsrModelKind::InputDynamicProps);
    }
    let input_type = vue3_ssr_input_type_binding(element, ast_node, state);
    match input_type {
        Some(Vue3SsrInputType::Dynamic(type_expr)) => Some(Vue3SsrModelKind::InputDynamicType {
            type_expr,
            value: vue3_ssr_value_binding(element, ast_node, state),
        }),
        Some(Vue3SsrInputType::Static(value)) => match value.as_str() {
            "radio" => Some(Vue3SsrModelKind::InputRadio {
                value: vue3_ssr_value_binding(element, ast_node, state),
            }),
            "checkbox" => vue3_ssr_true_value_binding(element, ast_node, state)
                .map(|true_value| Vue3SsrModelKind::InputCheckboxTrueValue { true_value })
                .or_else(|| {
                    Some(Vue3SsrModelKind::InputCheckbox {
                        value: vue3_ssr_value_binding(element, ast_node, state),
                    })
                }),
            "file" => None,
            _ => Some(Vue3SsrModelKind::InputValue),
        },
        None => Some(Vue3SsrModelKind::InputValue),
    }
}

pub(crate) enum Vue3SsrInputType {
    Static(String),
    Dynamic(JsExprId),
}

pub(crate) fn vue3_ssr_input_type_binding(
    element: &Vue3Element,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    state: &mut Vue3SsrLoweringState,
) -> Option<Vue3SsrInputType> {
    element.props.iter().find_map(|prop| match prop {
        Vue3Prop::Attribute(attr) if attr.name == "type" => Some(Vue3SsrInputType::Static(
            attr.value.clone().unwrap_or_default(),
        )),
        Vue3Prop::Directive(dir)
            if dir.name == "bind"
                && !dir.is_dynamic_arg
                && dir
                    .arg
                    .as_ref()
                    .is_some_and(|arg| arg.source_string() == "type") =>
        {
            dir.exp.as_ref().map(|exp| {
                Vue3SsrInputType::Dynamic(register_or_reuse_vue3_expression_with_span(
                    &mut state.js,
                    exp,
                    dir.exp_span.or_else(|| ast_node.span.source()),
                    state.source_type,
                ))
            })
        }
        _ => None,
    })
}

pub(crate) fn vue3_ssr_value_binding(
    element: &Vue3Element,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    state: &mut Vue3SsrLoweringState,
) -> MirExpr {
    vue3_ssr_static_or_dynamic_prop_expr(element, "value", ast_node, state).unwrap_or(MirExpr::Null)
}

pub(crate) fn vue3_ssr_true_value_binding(
    element: &Vue3Element,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    state: &mut Vue3SsrLoweringState,
) -> Option<MirExpr> {
    vue3_ssr_static_or_dynamic_prop_expr(element, "true-value", ast_node, state)
}

pub(crate) fn vue3_ssr_static_or_dynamic_prop_expr(
    element: &Vue3Element,
    name: &str,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    state: &mut Vue3SsrLoweringState,
) -> Option<MirExpr> {
    element.props.iter().find_map(|prop| match prop {
        Vue3Prop::Attribute(attr) if attr.name == name => {
            Some(MirExpr::String(attr.value.clone().unwrap_or_default()))
        }
        Vue3Prop::Directive(dir)
            if dir.name == "bind"
                && !dir.is_dynamic_arg
                && dir
                    .arg
                    .as_ref()
                    .is_some_and(|arg| arg.source_string() == name) =>
        {
            dir.exp.as_ref().map(|exp| {
                MirExpr::JsExpr(register_or_reuse_vue3_expression_with_span(
                    &mut state.js,
                    exp,
                    dir.exp_span.or_else(|| ast_node.span.source()),
                    state.source_type,
                ))
            })
        }
        _ => None,
    })
}

pub(crate) fn vue3_ssr_has_static_attr(element: &Vue3Element, name: &str) -> bool {
    element.props.iter().any(|prop| {
        matches!(
            prop,
            Vue3Prop::Attribute(attr) if attr.name == name
        )
    })
}

pub(crate) fn vue3_ssr_has_dynamic_key_v_bind(element: &Vue3Element) -> bool {
    element.props.iter().any(|prop| {
        matches!(
            prop,
            Vue3Prop::Directive(dir)
                if dir.name == "bind" && dir.exp.is_some() && (dir.arg.is_none() || dir.is_dynamic_arg)
        )
    })
}

pub(crate) fn vue3_ssr_open_tag_start(
    element: &Vue3Element,
    omit_static_style: bool,
    v_model: Option<&Vue3SsrModel>,
    options: &Vue3CompilerOptions,
) -> String {
    let mut rendered = String::new();
    rendered.push('<');
    rendered.push_str(&element.tag);
    for prop in &element.props {
        if let Vue3Prop::Attribute(attr) = prop {
            if vue3_ssr_should_omit_static_attr(
                element,
                attr.name.as_str(),
                omit_static_style,
                v_model,
            ) {
                continue;
            }
            rendered.push(' ');
            rendered.push_str(&attr.name);
            if let Some(value) = &attr.value {
                rendered.push_str("=\"");
                rendered.push_str(&vue3_ssr_escape_attr(value));
                rendered.push('"');
            }
        }
    }
    if let Some(scope_id) = &options.scope_id {
        rendered.push(' ');
        rendered.push_str(scope_id);
    }
    rendered
}

pub(crate) fn vue3_ssr_should_omit_static_attr(
    element: &Vue3Element,
    name: &str,
    omit_static_style: bool,
    v_model: Option<&Vue3SsrModel>,
) -> bool {
    if matches!(name, "key" | "ref") || (element.tag == "textarea" && name == "value") {
        return true;
    }
    if omit_static_style && name == "style" {
        return true;
    }
    if element.tag == "input" && matches!(name, "true-value" | "false-value") {
        return true;
    }
    if matches!(
        v_model.map(|model| &model.kind),
        Some(Vue3SsrModelKind::InputDynamicProps)
    ) {
        return true;
    }
    if matches!(
        v_model.map(|model| &model.kind),
        Some(Vue3SsrModelKind::InputValue)
            | Some(Vue3SsrModelKind::InputDynamicType {
                type_expr: _,
                value: _
            })
    ) && name == "value"
    {
        return true;
    }
    false
}

pub(crate) fn lower_vue3_ssr_attrs(
    hir_id: NodeId,
    v_show: Option<JsExprId>,
    v_model: Option<Vue3SsrModel>,
    directive_content: bool,
    textarea_value_fallback: Option<String>,
    state: &Vue3SsrLoweringState,
) -> Option<Vue3SsrAttrs> {
    let (props, directives) = match state.hir.node(hir_id).map(|node| &node.kind) {
        Some(HirNodeKind::Element(element)) => {
            let tag = match &element.tag {
                HirTag::Native(tag) => tag.as_str(),
                HirTag::Dynamic(_) => "",
            };
            (
                filter_vue3_ssr_attr_props(&element.props, tag, v_show, v_model.as_ref()),
                element
                    .directives
                    .iter()
                    .filter(|directive| directive.name != "show")
                    .map(lower_hir_directive_to_dom_mir)
                    .collect::<Vec<_>>(),
            )
        }
        _ => (HirProps::default(), Vec::new()),
    };
    let has_dynamic_props = !props.dynamic_bindings.is_empty() || !props.object_bindings.is_empty();
    if props.segments.is_empty()
        && props.dynamic_bindings.is_empty()
        && props.object_bindings.is_empty()
        && directives.is_empty()
        && !directive_content
        && textarea_value_fallback.is_none()
        && v_show.is_none()
        && v_model.is_none()
    {
        return None;
    }
    if !has_dynamic_props
        && directives.is_empty()
        && !directive_content
        && textarea_value_fallback.is_none()
        && v_show.is_none()
        && v_model.is_none()
    {
        return None;
    }
    Some(Vue3SsrAttrs {
        props: lower_hir_props_to_dom_mir_without_event_cache(&props),
        directives,
        directive_content,
        textarea_value_fallback,
        force_render_attrs: false,
        v_show,
        v_model,
    })
}

pub(crate) fn filter_vue3_ssr_attr_props(
    props: &HirProps,
    tag: &str,
    include_static_style: Option<JsExprId>,
    v_model: Option<&Vue3SsrModel>,
) -> HirProps {
    let mut filtered = HirProps::default();
    for segment in &props.segments {
        match segment {
            HirPropSegment::StaticAttr(attr)
                if !vue3_ssr_should_skip_static_attr_payload(attr, tag)
                    && (include_static_style.is_none()
                        || attr.name == "style"
                        || vue3_ssr_should_keep_static_attr_in_payload(attr, v_model)) =>
            {
                filtered.static_attrs.push(attr.clone());
                filtered
                    .segments
                    .push(HirPropSegment::StaticAttr(attr.clone()));
            }
            HirPropSegment::DynamicBinding(binding) => {
                if vue3_ssr_should_skip_dynamic_binding(binding, v_model, tag) {
                    continue;
                }
                filtered.dynamic_bindings.push(binding.clone());
                filtered
                    .segments
                    .push(HirPropSegment::DynamicBinding(binding.clone()));
            }
            HirPropSegment::ObjectBinding(binding) => {
                filtered.object_bindings.push(binding.clone());
                filtered
                    .segments
                    .push(HirPropSegment::ObjectBinding(binding.clone()));
            }
            HirPropSegment::StaticAttr(_)
            | HirPropSegment::Event(_)
            | HirPropSegment::ObjectListeners(_) => {}
        }
    }
    if filtered.segments.is_empty() {
        filtered.dynamic_bindings = props
            .dynamic_bindings
            .iter()
            .filter(|binding| !vue3_ssr_should_skip_dynamic_binding(binding, v_model, tag))
            .cloned()
            .collect();
        filtered.object_bindings = props.object_bindings.clone();
    }
    filtered
}

pub(crate) fn vue3_ssr_should_skip_static_attr_payload(attr: &HirStaticAttr, tag: &str) -> bool {
    matches!(attr.name.as_str(), "key" | "ref") || (tag == "textarea" && attr.name == "value")
}

pub(crate) fn vue3_ssr_should_keep_static_attr_in_payload(
    attr: &HirStaticAttr,
    v_model: Option<&Vue3SsrModel>,
) -> bool {
    matches!(
        v_model.map(|model| &model.kind),
        Some(Vue3SsrModelKind::InputDynamicProps)
    ) && !matches!(attr.name.as_str(), "true-value" | "false-value")
        || matches!(
            v_model.map(|model| &model.kind),
            Some(Vue3SsrModelKind::InputDynamicType {
                type_expr: _,
                value: _
            })
        ) && attr.name == "value"
}

pub(crate) fn vue3_ssr_should_skip_dynamic_binding(
    binding: &HirBinding,
    v_model: Option<&Vue3SsrModel>,
    tag: &str,
) -> bool {
    if binding.dynamic_arg {
        return false;
    }
    matches!(
        binding.name.as_str(),
        "key" | "ref" | "true-value" | "false-value"
    ) || (tag == "textarea" && binding.name == "value")
        || matches!(
            v_model.map(|model| &model.kind),
            Some(Vue3SsrModelKind::InputValue)
        ) && binding.name == "value"
}

pub(crate) fn vue3_ssr_escape_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
}

pub(crate) fn decode_vue3_ssr_escaped_attr(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&amp;", "&")
}

pub(crate) fn vue3_static_style_object_expr(value: &str) -> String {
    let properties = vue3_parse_static_style(value)
        .iter()
        .map(|(name, value)| {
            format!(
                "{}:{}",
                serde_json::to_string(name).unwrap_or_else(|_| "\"\"".into()),
                serde_json::to_string(value).unwrap_or_else(|_| "\"\"".into())
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{properties}}}")
}

pub(crate) fn vue3_parse_static_style(value: &str) -> Vec<(String, String)> {
    let mut style = Vec::new();
    let mut current = String::new();
    let mut depth = 0usize;
    for ch in vue3_strip_css_comments(value).chars() {
        match ch {
            '(' => {
                depth += 1;
                current.push(ch);
            }
            ')' => {
                depth = depth.saturating_sub(1);
                current.push(ch);
            }
            ';' if depth == 0 => {
                vue3_push_static_style_decl(&mut style, &current);
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    vue3_push_static_style_decl(&mut style, &current);
    style
}

pub(crate) fn vue3_strip_css_comments(value: &str) -> String {
    let mut output = String::new();
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '/' && chars.peek() == Some(&'*') {
            chars.next();
            let mut previous = '\0';
            for next in chars.by_ref() {
                if previous == '*' && next == '/' {
                    break;
                }
                previous = next;
            }
        } else {
            output.push(ch);
        }
    }
    output
}

pub(crate) fn vue3_push_static_style_decl(style: &mut Vec<(String, String)>, item: &str) {
    let Some((name, value)) = item.split_once(':') else {
        return;
    };
    let name = name.trim();
    let value = value.trim();
    if name.is_empty() || value.is_empty() {
        return;
    }
    if let Some((_, existing)) = style.iter_mut().find(|(existing, _)| existing == name) {
        *existing = value.to_string();
    } else {
        style.push((name.to_string(), value.to_string()));
    }
}
