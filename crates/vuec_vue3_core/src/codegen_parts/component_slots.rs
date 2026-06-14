#[derive(Clone, Debug, Default)]
pub(crate) struct ComponentSlotAnalysis {
    pub(crate) has_slots: bool,
    pub(crate) has_dynamic_slots: bool,
}

pub(crate) fn analyze_component_slots(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
) -> ComponentSlotAnalysis {
    let Some(node) = ast.node(node_id) else {
        return ComponentSlotAnalysis::default();
    };
    let visible = visible_children(ast, &node.children);
    if visible.is_empty() {
        return ComponentSlotAnalysis::default();
    }
    let mut analysis = ComponentSlotAnalysis {
        has_slots: true,
        has_dynamic_slots: false,
    };
    for child in visible {
        if let Vue3AstKind::Element(element) = &child.kind {
            if directive_by_name(element, "slot").is_some()
                && (directive_by_name(element, "if").is_some()
                    || directive_by_name(element, "for").is_some()
                    || directive_by_name(element, "else").is_some()
                    || directive_by_name(element, "else-if").is_some()
                    || directive_by_name(element, "slot").is_some_and(|slot| slot.is_dynamic_arg))
            {
                analysis.has_dynamic_slots = true;
            }
        }
    }
    analysis
}

pub(crate) fn render_component_slots(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> Option<String> {
    let node = ast.node(node_id)?;
    let visible = visible_children(ast, &node.children);
    if visible.is_empty() {
        return None;
    }
    let dynamic_slots = visible.iter().any(|child| {
        matches!(
            &child.kind,
            Vue3AstKind::Element(element)
                if directive_by_name(element, "slot").is_some()
                    && (directive_by_name(element, "if").is_some()
                        || directive_by_name(element, "for").is_some()
                        || directive_by_name(element, "else").is_some()
                        || directive_by_name(element, "else-if").is_some())
        )
    });
    if dynamic_slots {
        Some(render_dynamic_component_slots(
            ast, &visible, options, scope, memo_index,
        ))
    } else {
        Some(render_stable_component_slots(
            ast, &visible, options, scope, memo_index,
        ))
    }
}

pub(crate) fn render_stable_component_slots(
    ast: &Vue3Ast,
    children: &[&vuec_ast::Node<Vue3NodeKind>],
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> String {
    let mut slots = Vec::new();
    let mut default_children = Vec::new();
    for child in children {
        if let Vue3AstKind::Element(element) = &child.kind {
            if let Some(slot) = directive_by_name(element, "slot") {
                slots.push(render_static_slot_property(
                    ast, child.id, element, slot, options, scope, memo_index,
                ));
                continue;
            }
        }
        default_children.push(child.id);
    }
    if !default_children.is_empty() {
        slots.push(render_slot_property(
            "default",
            "()",
            render_slot_children(ast, &default_children, options, scope, memo_index),
        ));
    }
    slots.push("_: 1 /* STABLE */".into());
    render_object(&slots)
}

pub(crate) fn render_dynamic_component_slots(
    ast: &Vue3Ast,
    children: &[&vuec_ast::Node<Vue3NodeKind>],
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> String {
    let mut dynamic_entries = Vec::new();
    for (index, child) in children.iter().enumerate() {
        let Vue3AstKind::Element(element) = &child.kind else {
            continue;
        };
        let Some(slot) = directive_by_name(element, "slot") else {
            continue;
        };
        if let Some(if_dir) = directive_by_name(element, "if") {
            dynamic_entries.push(render_conditional_dynamic_slot(
                ast, child.id, element, slot, if_dir, options, scope, index, memo_index,
            ));
        } else if let Some(for_dir) = directive_by_name(element, "for") {
            dynamic_entries.push(render_for_dynamic_slot(
                ast, child.id, element, slot, for_dir, options, scope, memo_index,
            ));
        } else {
            dynamic_entries.push(render_dynamic_slot_object(
                ast, child.id, element, slot, options, scope, None, memo_index,
            ));
        }
    }
    format!(
        "_createSlots({{ _: 2 /* DYNAMIC */ }}, {})",
        render_array(&dynamic_entries)
    )
}

pub(crate) fn render_static_slot_property(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    _element: &Vue3Element,
    slot: &Vue3Directive,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> String {
    let name = slot
        .arg
        .as_ref()
        .map(Vue3Expression::source_string)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "default".into());
    let params = slot
        .exp
        .as_ref()
        .map(Vue3Expression::source_string)
        .filter(|params| !params.trim().is_empty())
        .map(|params| format!("({})", params.trim()))
        .unwrap_or_else(|| "()".into());
    let slot_scope = slot_function_scope(scope, &params);
    let children = ast
        .node(node_id)
        .map(|node| render_slot_children(ast, &node.children, options, &slot_scope, memo_index))
        .unwrap_or_else(|| "[]".into());
    render_slot_property(&name, &params, children)
}

pub(crate) fn render_slot_property(name: &str, params: &str, children: String) -> String {
    format!("{}: _withCtx({params} => {children})", json_key(name))
}

pub(crate) fn render_conditional_dynamic_slot(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    element: &Vue3Element,
    slot: &Vue3Directive,
    if_dir: &Vue3Directive,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    index: usize,
    memo_index: &mut MemoIndex,
) -> String {
    let condition = if_dir
        .exp
        .as_ref()
        .map(Vue3Expression::source_string)
        .unwrap_or_default();
    let condition = render_condition(
        &rewrite_expression_with_scope(&condition, options, scope),
        options,
    );
    let slot = render_dynamic_slot_object(
        ast,
        node_id,
        element,
        slot,
        options,
        scope,
        Some(index),
        memo_index,
    );
    format!(
        "{condition}\n  ? {}\n  : undefined",
        indent_after_first_line(&slot, 4)
    )
}

pub(crate) fn render_for_dynamic_slot(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    element: &Vue3Element,
    slot: &Vue3Directive,
    for_dir: &Vue3Directive,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> String {
    let expression = for_dir
        .exp
        .as_ref()
        .map(Vue3Expression::source_string)
        .unwrap_or_default();
    let Some((source, aliases)) = parse_v_for_expression(&expression) else {
        return render_dynamic_slot_object(
            ast, node_id, element, slot, options, scope, None, memo_index,
        );
    };
    let source = rewrite_expression_with_scope(&source, options, scope);
    let scoped = scope.with_locals(normalize_v_for_aliases(&aliases));
    let params = aliases.join(", ");
    let body = render_dynamic_slot_object(
        ast, node_id, element, slot, options, &scoped, None, memo_index,
    );
    format!(
        "_renderList({source}, ({params}) => {{\n  return {}\n}})",
        indent_after_first_line(&body, 2)
    )
}

pub(crate) fn render_dynamic_slot_object(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    _element: &Vue3Element,
    slot: &Vue3Directive,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    key: Option<usize>,
    memo_index: &mut MemoIndex,
) -> String {
    let name = slot_name_expression(slot, options, scope);
    let params = slot
        .exp
        .as_ref()
        .map(Vue3Expression::source_string)
        .filter(|params| !params.trim().is_empty())
        .map(|params| format!("({})", params.trim()))
        .unwrap_or_else(|| "()".into());
    let slot_scope = slot_function_scope(scope, &params);
    let children = ast
        .node(node_id)
        .map(|node| render_slot_children(ast, &node.children, options, &slot_scope, memo_index))
        .unwrap_or_else(|| "[]".into());
    let mut properties = vec![
        format!("name: {name}"),
        format!("fn: _withCtx({params} => {children})"),
    ];
    if let Some(key) = key {
        properties.push(format!("key: {}", quote_string(&key.to_string())));
    }
    render_object(&properties)
}

pub(crate) fn slot_name_expression(
    slot: &Vue3Directive,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    let Some(arg) = slot.arg.as_ref() else {
        return quote_string("default");
    };
    let name = arg.source_string();
    if slot.is_dynamic_arg {
        rewrite_expression_with_scope(&name, options, scope)
    } else {
        quote_string(&name)
    }
}

pub(crate) fn render_slot_children(
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> String {
    let rendered = render_child_sequence(
        ast,
        children,
        options,
        NodeRenderMode::Child,
        scope,
        memo_index,
    );
    render_array(&rendered)
}

pub(crate) fn slot_function_scope(scope: &RenderScope, params: &str) -> RenderScope {
    scope.with_locals(extract_slot_params(params))
}

pub(crate) fn extract_slot_params(params: &str) -> Vec<String> {
    let params = params
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .trim();
    let mut output = Vec::new();
    let mut ident = String::new();
    for ch in params.chars() {
        if is_identifier_continue(ch) {
            ident.push(ch);
        } else if !ident.is_empty() {
            output.push(std::mem::take(&mut ident));
        }
    }
    if !ident.is_empty() {
        output.push(ident);
    }
    output
}

pub(crate) fn component_patch_flag_kind(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> Option<i32> {
    let Some(node) = ast.node(node_id) else {
        return None;
    };
    let visible = visible_children(ast, &node.children);
    let mut flag = if has_dynamic_arg_binding(element) {
        16
    } else if has_dynamic_non_key_props(element, options, scope) {
        8
    } else {
        0
    };
    if visible.iter().any(|child| {
        matches!(
            &child.kind,
            Vue3AstKind::Element(element)
                if directive_by_name(element, "slot").is_some()
                    && (directive_by_name(element, "if").is_some()
                        || directive_by_name(element, "for").is_some()
                        || directive_by_name(element, "else").is_some()
                        || directive_by_name(element, "else-if").is_some())
        )
    }) {
        flag |= 1024;
    }
    (flag != 0).then_some(flag)
}

pub(crate) fn visible_children<'a>(
    ast: &'a Vue3Ast,
    children: &[vuec_ast::NodeId],
) -> Vec<&'a vuec_ast::Node<Vue3NodeKind>> {
    children
        .iter()
        .filter_map(|child_id| ast.node(*child_id))
        .filter(|child| match &child.kind {
            Vue3AstKind::Comment(_) => false,
            Vue3AstKind::Text(text) => !text.value.trim().is_empty(),
            _ => true,
        })
        .collect()
}

pub(crate) fn collect_component_tags(ast: &Vue3Ast) -> Vec<String> {
    let mut tags = Vec::new();
    for node in &ast.nodes {
        if let Vue3AstKind::Element(element) = &node.kind {
            if element.tag_type == Vue3ElementType::Component
                && !tags.iter().any(|tag| tag == &element.tag)
            {
                tags.push(element.tag.clone());
            }
        }
    }
    tags
}

pub(crate) fn collect_runtime_directive_names(ast: &Vue3Ast) -> Vec<String> {
    let mut directives = Vec::new();
    for node in &ast.nodes {
        if let Vue3AstKind::Element(element) = &node.kind {
            for prop in &element.props {
                let Vue3Prop::Directive(dir) = prop else {
                    continue;
                };
                if dir.name != "show"
                    && vue3_directive_needs_runtime_asset(&dir.name)
                    && !directives.iter().any(|existing| existing == &dir.name)
                {
                    directives.push(dir.name.clone());
                }
            }
        }
    }
    directives
}

pub(crate) fn collect_vue3_component_asset(
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
    components: &mut BTreeSet<String>,
    helpers: &mut BTreeSet<RuntimeHelper>,
) {
    if let Some(helper) = vue3_core_component_runtime_helper(&element.tag) {
        helpers.insert(helper);
    } else if !vue3_tag_is_direct_setup_or_props_reference(&element.tag, options)
        && !matches!(element.tag.as_str(), "component" | "Component")
        && !vue3_tag_is_namespaced_setup_or_props_reference(&element.tag, options)
    {
        components.insert(element.tag.clone());
    }
    if matches!(element.tag.as_str(), "component" | "Component")
        || vue3_dynamic_component_is_expression(element).is_some()
    {
        helpers.insert(RuntimeHelper::Vue3ResolveDynamicComponent);
    }
}

pub(crate) fn vue3_tag_is_direct_setup_or_props_reference(
    tag: &str,
    options: &Vue3CompilerOptions,
) -> bool {
    setup_reference_name_for_tag(tag, options).is_some()
}

pub(crate) fn vue3_tag_is_namespaced_setup_or_props_reference(
    tag: &str,
    options: &Vue3CompilerOptions,
) -> bool {
    let Some((namespace, member)) = tag.split_once('.') else {
        return false;
    };
    !namespace.is_empty()
        && !member.is_empty()
        && options.binding_metadata.get(namespace).is_some_and(|kind| {
            matches!(
                kind.as_str(),
                "setup-ref"
                    | "setup-maybe-ref"
                    | "setup-let"
                    | "setup-const"
                    | "setup-reactive-const"
                    | "literal-const"
                    | "props"
                    | "props-aliased"
            )
        })
}

pub(crate) fn collect_vue3_runtime_directive_asset(
    directive: &Vue3Directive,
    options: &Vue3CompilerOptions,
    directives: &mut BTreeSet<String>,
    helpers: &mut BTreeSet<RuntimeHelper>,
) {
    if !vue3_directive_needs_runtime_asset(&directive.name) {
        return;
    }
    if directive.name == "show" {
        helpers.insert(RuntimeHelper::Vue3VShow);
    } else if directive.name == "model" {
        helpers.insert(RuntimeHelper::Vue3VModelDynamic);
    } else if setup_reference_name(&format!("v-{}", directive.name), options).is_some() {
        if render_setup_runtime_directive(&directive.name, options)
            .is_some_and(|runtime| runtime.contains("_unref("))
        {
            helpers.insert(RuntimeHelper::Vue3Unref);
        }
    } else {
        directives.insert(directive.name.clone());
        helpers.insert(RuntimeHelper::Vue3ResolveDirective);
    }
    helpers.insert(RuntimeHelper::Vue3WithDirectives);
}

pub(crate) fn collect_vue3_binding_rewrite_helpers(
    directive: &Vue3Directive,
    options: &Vue3CompilerOptions,
    helpers: &mut BTreeSet<RuntimeHelper>,
) {
    if !uses_prefixed_identifiers(options) {
        return;
    }
    let source = directive
        .exp
        .as_ref()
        .map(Vue3Expression::source_string)
        .unwrap_or_default();
    if source.trim().is_empty() {
        return;
    }
    let rewritten = if directive.name == "on" && directive.arg.is_some() {
        let scope = RenderScope::default();
        rewrite_handler_expression_with_scope(&source, options, &scope)
    } else {
        rewrite_js_like_expression(&source, options)
    };
    for helper in vue3_for_helpers_for_content(&rewritten) {
        match helper {
            "UNREF" => {
                helpers.insert(RuntimeHelper::Vue3Unref);
            }
            "IS_REF" => {
                helpers.insert(RuntimeHelper::Vue3IsRef);
            }
            _ => {}
        }
    }
}

pub(crate) fn vue3_core_component_runtime_helper(tag: &str) -> Option<RuntimeHelper> {
    match tag {
        "Teleport" | "teleport" => Some(RuntimeHelper::Vue3Teleport),
        "Suspense" | "suspense" => Some(RuntimeHelper::Vue3Suspense),
        "KeepAlive" | "keep-alive" => Some(RuntimeHelper::Vue3KeepAlive),
        "BaseTransition" | "base-transition" => Some(RuntimeHelper::Vue3BaseTransition),
        "Transition" | "transition" => Some(RuntimeHelper::Vue3Transition),
        "TransitionGroup" | "transition-group" => Some(RuntimeHelper::Vue3TransitionGroup),
        _ => None,
    }
}

pub(crate) fn vue3_dynamic_component_is_expression(
    element: &Vue3Element,
) -> Option<&Vue3Expression> {
    element.props.iter().find_map(|prop| match prop {
        Vue3Prop::Directive(dir)
            if dir.name == "bind"
                && dir
                    .arg
                    .as_ref()
                    .is_some_and(|arg| arg.source_string() == "is") =>
        {
            dir.exp.as_ref()
        }
        _ => None,
    })
}

pub(crate) fn vue3_directive_needs_runtime_asset(name: &str) -> bool {
    !matches!(
        name,
        "bind"
            | "cloak"
            | "else"
            | "else-if"
            | "for"
            | "html"
            | "if"
            | "memo"
            | "model"
            | "on"
            | "once"
            | "pre"
            | "slot"
            | "text"
    )
}

pub(crate) fn component_asset_id(tag: &str) -> String {
    to_valid_asset_id(tag, "component")
}

pub(crate) fn to_valid_asset_id(name: &str, asset_type: &str) -> String {
    format!("_{asset_type}_{}", to_valid_asset_part(name))
}

pub(crate) fn to_valid_asset_part(value: &str) -> String {
    let mut output = String::new();
    for unit in value.encode_utf16() {
        match unit {
            65..=90 | 97..=122 | 48..=57 | 95 => {
                let ch = char::from_u32(unit as u32).unwrap_or('_');
                output.push(ch);
            }
            45 => output.push('_'),
            _ => output.push_str(&unit.to_string()),
        }
    }
    output
}
