pub(crate) fn lower_vue3_ast_node_to_ssr_mir(
    ast_id: NodeId,
    ast: &Vue3Ast,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue3SsrLoweringState,
) -> Option<NodeId> {
    let ast_node = ast.node(ast_id)?;
    match &ast_node.kind {
        Vue3AstKind::Root(_) => None,
        Vue3AstKind::Element(element) => {
            if let Some(lowered) = lower_vue3_element_control_flow_to_ssr_mir(
                ast_id, element, ast, ast_node, hir_parent, mir_parent, state,
            ) {
                return lowered;
            }
            lower_vue3_plain_element_to_ssr_mir(
                ast_id, element, ast, ast_node, hir_parent, mir_parent, state,
            )
        }
        Vue3AstKind::Text(text) => {
            let hir_id = state.hir.push_child(
                hir_parent,
                HirNodeKind::Text(vuec_ast::HirText {
                    value: text.value.clone(),
                }),
                ast_node.span.clone(),
            );
            let mir_id = state.mir.push_child(
                mir_parent,
                Vue3SsrMirKind::PushString(escape_static_html_text(&text.value)),
                ast_node.span.clone(),
            );
            state.map.record_ast_to_hir(ast_id, hir_id);
            state.map.record_hir_to_mir(hir_id, mir_id);
            Some(hir_id)
        }
        Vue3AstKind::Interpolation(interpolation) => {
            let expr = register_vue3_expression_with_span(
                &mut state.js,
                &interpolation.expression,
                ast_node.span.source(),
                state.source_type,
            );
            let hir_id = state.hir.push_child(
                hir_parent,
                HirNodeKind::Interpolation(HirInterpolation {
                    expression: HirExpr::Js(expr),
                }),
                ast_node.span.clone(),
            );
            let mir_id = state.mir.push_child(
                mir_parent,
                Vue3SsrMirKind::PushInterpolated(MirExpr::JsExpr(expr)),
                ast_node.span.clone(),
            );
            state.map.record_ast_to_hir(ast_id, hir_id);
            state.map.record_hir_to_mir(hir_id, mir_id);
            Some(hir_id)
        }
        Vue3AstKind::Comment(comment) => {
            let span =
                NodeSpan::generated(ast_node.span.source(), vuec_ast::GeneratedReason::Lowering);
            let hir_id =
                state
                    .hir
                    .push_child(hir_parent, HirNodeKind::Fragment(HirFragment), span.clone());
            let mir_id = state.mir.push_child(
                mir_parent,
                Vue3SsrMirKind::PushString(format!("<!--{}-->", comment.value)),
                span,
            );
            state.map.record_ast_to_hir(ast_id, hir_id);
            state.map.record_hir_to_mir(hir_id, mir_id);
            Some(hir_id)
        }
        Vue3AstKind::CompoundExpression(_) | Vue3AstKind::TextCall(_) => {
            let hir_id = state.hir.push_child(
                hir_parent,
                HirNodeKind::Fragment(HirFragment),
                ast_node.span.clone(),
            );
            let mir_id = state.mir.push_child(
                mir_parent,
                Vue3SsrMirKind::PushString(String::new()),
                NodeSpan::generated(ast_node.span.source(), vuec_ast::GeneratedReason::Lowering),
            );
            state.map.record_ast_to_hir(ast_id, hir_id);
            state.map.record_hir_to_mir(hir_id, mir_id);
            Some(hir_id)
        }
        Vue3AstKind::If(_) | Vue3AstKind::IfBranch(_) | Vue3AstKind::For(_) => None,
    }
}

pub(crate) fn lower_vue3_ssr_child_sequence(
    children: &[NodeId],
    ast: &Vue3Ast,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue3SsrLoweringState,
) {
    let mut index = 0usize;
    while index < children.len() {
        let child_id = children[index];
        let Some(child) = ast.node(child_id) else {
            index += 1;
            continue;
        };
        if let Vue3AstKind::Element(element) = &child.kind {
            if directive_by_name(element, "if").is_some() {
                let (branch_ids, next_index) = collect_vue3_if_branch_chain(children, index, ast);
                lower_vue3_if_branch_chain_to_ssr_mir(
                    &branch_ids,
                    ast,
                    hir_parent,
                    mir_parent,
                    state,
                );
                index = next_index;
                continue;
            }
            if directive_by_name(element, "for").is_some() {
                lower_vue3_ast_node_to_ssr_mir(child_id, ast, hir_parent, mir_parent, state);
                index += 1;
                continue;
            }
            if is_else_branch(element) {
                index += 1;
                continue;
            }
        }
        lower_vue3_ast_node_to_ssr_mir(child_id, ast, hir_parent, mir_parent, state);
        index += 1;
    }
}

pub(crate) fn lower_vue3_plain_element_to_ssr_mir(
    ast_id: NodeId,
    element: &Vue3Element,
    ast: &Vue3Ast,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue3SsrLoweringState,
) -> Option<NodeId> {
    let hir_kind =
        lower_vue3_element_to_hir_kind(element, ast_node, &mut state.js, state.source_type);
    let hir_id = state
        .hir
        .push_child(hir_parent, hir_kind, ast_node.span.clone());
    state.map.record_ast_to_hir(ast_id, hir_id);

    match element.tag_type {
        Vue3ElementType::Component => {
            if let Some(mir_id) = lower_vue3_builtin_component_to_ssr_mir(
                element, ast, ast_node, hir_id, mir_parent, state,
            ) {
                state.map.record_hir_to_mir(hir_id, mir_id);
                return Some(hir_id);
            }
            let props = match state.hir.node(hir_id).map(|node| &node.kind) {
                Some(HirNodeKind::Component(component)) => {
                    let props = if vue3_ssr_component_is_dynamic(element) {
                        filter_vue3_dynamic_component_is_props(&component.props)
                    } else {
                        component.props.clone()
                    };
                    lower_hir_component_props_to_ssr_mir(&props)
                }
                _ => Vue3DomProps::default(),
            };
            let directives = lower_vue3_component_directives_to_ssr_mir(element, ast_node, state);
            let (tag, dynamic) = vue3_ssr_component_tag(element, ast_node, state);
            let mir_id = state.mir.push_child(
                mir_parent,
                Vue3SsrMirKind::RenderComponent(Vue3SsrComponent {
                    tag,
                    props,
                    directives,
                    slots: None,
                    dynamic,
                }),
                ast_node.span.clone(),
            );
            state.map.record_hir_to_mir(hir_id, mir_id);
            if let Some(slots) =
                lower_vue3_component_slots_to_ssr_mir(ast_node, ast, hir_id, mir_id, state)
            {
                if let Some(node) = state.mir.node_mut(mir_id) {
                    if let Vue3SsrMirKind::RenderComponent(component) = &mut node.kind {
                        component.slots = Some(slots);
                    }
                }
            } else {
                lower_vue3_ssr_child_sequence(&ast_node.children, ast, hir_id, mir_id, state);
            }
        }
        Vue3ElementType::SlotOutlet => {
            let Some(HirNodeKind::SlotOutlet(slot)) = state.hir.node(hir_id).map(|node| &node.kind)
            else {
                return Some(hir_id);
            };
            let mir_id = state.mir.push_child(
                mir_parent,
                Vue3SsrMirKind::RenderSlot(vuec_ast::Vue3SsrSlot {
                    name: lower_hir_slot_outlet_name_to_dom_mir(slot),
                    props: lower_vue3_slot_outlet_props_to_ssr_mir(&slot.props),
                    fallback: Vec::new(),
                    inner: false,
                }),
                ast_node.span.clone(),
            );
            state.map.record_hir_to_mir(hir_id, mir_id);
            lower_vue3_ssr_child_sequence(&ast_node.children, ast, hir_id, mir_id, state);
            let fallback = state
                .mir
                .node(mir_id)
                .map(|node| node.children.clone())
                .unwrap_or_default();
            if let Some(node) = state.mir.node_mut(mir_id) {
                if let Vue3SsrMirKind::RenderSlot(slot) = &mut node.kind {
                    slot.fallback = fallback;
                }
            }
        }
        Vue3ElementType::Element => {
            lower_vue3_native_element_to_ssr_mir(element, ast_node, ast, hir_id, mir_parent, state);
        }
        Vue3ElementType::Template => {
            lower_vue3_template_element_to_ssr_mir(ast_node, ast, hir_id, mir_parent, state);
        }
    }

    Some(hir_id)
}

pub(crate) fn lower_vue3_template_element_to_ssr_mir(
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    ast: &Vue3Ast,
    hir_id: NodeId,
    mir_parent: NodeId,
    state: &mut Vue3SsrLoweringState,
) {
    let generated_span =
        NodeSpan::generated(ast_node.span.source(), vuec_ast::GeneratedReason::Lowering);
    let wrap_fragment = state.flatten_ssr_fragments == 0
        && vue3_ssr_template_children_need_fragment(ast, &ast_node.children);
    if wrap_fragment {
        let open_id = state.mir.push_child(
            mir_parent,
            Vue3SsrMirKind::PushString("<!--[-->".into()),
            generated_span.clone(),
        );
        state.map.record_hir_to_mir(hir_id, open_id);
    }
    lower_vue3_ssr_child_sequence(&ast_node.children, ast, hir_id, mir_parent, state);
    if wrap_fragment {
        let close_id = state.mir.push_child(
            mir_parent,
            Vue3SsrMirKind::PushString("<!--]-->".into()),
            generated_span,
        );
        state.map.record_hir_to_mir(hir_id, close_id);
    }
}

pub(crate) fn vue3_ssr_template_children_need_fragment(ast: &Vue3Ast, children: &[NodeId]) -> bool {
    let visible = visible_child_ids(ast, children);
    let [single] = visible.as_slice() else {
        return !visible.is_empty();
    };
    let Some(child) = ast.node(*single) else {
        return true;
    };
    let Vue3AstKind::Element(element) = &child.kind else {
        return true;
    };
    element.tag_type == Vue3ElementType::Template
}

pub(crate) fn lower_vue3_builtin_component_to_ssr_mir(
    element: &Vue3Element,
    ast: &Vue3Ast,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    hir_id: NodeId,
    mir_parent: NodeId,
    state: &mut Vue3SsrLoweringState,
) -> Option<NodeId> {
    match element.tag.as_str() {
        "Transition" | "transition" => {
            lower_vue3_transition_to_ssr_mir(element, ast, ast_node, hir_id, mir_parent, state)
        }
        "TransitionGroup" | "transition-group" => lower_vue3_transition_group_to_ssr_mir(
            element, ast, ast_node, hir_id, mir_parent, state,
        ),
        "KeepAlive" | "Keepalive" | "keep-alive" => {
            lower_vue3_keep_alive_to_ssr_mir(ast, ast_node, hir_id, mir_parent, state)
        }
        "Teleport" | "teleport" => {
            let target = vue3_ssr_teleport_target(element, ast_node, state)?;
            let disabled = vue3_ssr_teleport_disabled(element, ast_node, state);
            let mir_id = state.mir.push_child(
                mir_parent,
                Vue3SsrMirKind::Teleport(Vue3SsrTeleport { target, disabled }),
                ast_node.span.clone(),
            );
            lower_vue3_ssr_child_sequence(&ast_node.children, ast, hir_id, mir_id, state);
            Some(mir_id)
        }
        "Suspense" | "suspense" => {
            let mir_id = state.mir.push_child(
                mir_parent,
                Vue3SsrMirKind::Suspense(Vue3SsrSuspense {
                    slots: vue3_ssr_empty_slots(Vue3SlotFlag::Stable),
                }),
                ast_node.span.clone(),
            );
            let slots = lower_vue3_suspense_slots_to_ssr_mir(ast_node, ast, hir_id, mir_id, state)
                .unwrap_or_else(|| vue3_ssr_empty_slots(Vue3SlotFlag::Stable));
            if let Some(node) = state.mir.node_mut(mir_id) {
                if let Vue3SsrMirKind::Suspense(suspense) = &mut node.kind {
                    suspense.slots = slots;
                }
            }
            Some(mir_id)
        }
        _ => None,
    }
}

pub(crate) fn lower_vue3_keep_alive_to_ssr_mir(
    ast: &Vue3Ast,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    hir_id: NodeId,
    mir_parent: NodeId,
    state: &mut Vue3SsrLoweringState,
) -> Option<NodeId> {
    let first_output = state.mir.nodes.len();
    lower_vue3_ssr_child_sequence(&ast_node.children, ast, hir_id, mir_parent, state);
    state.mir.nodes.get(first_output).map(|node| node.id)
}

pub(crate) fn lower_vue3_transition_to_ssr_mir(
    element: &Vue3Element,
    ast: &Vue3Ast,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    hir_id: NodeId,
    mir_parent: NodeId,
    state: &mut Vue3SsrLoweringState,
) -> Option<NodeId> {
    let generated_span =
        NodeSpan::generated(ast_node.span.source(), vuec_ast::GeneratedReason::Lowering);
    if state
        .mir
        .node(mir_parent)
        .is_some_and(|node| matches!(node.kind, Vue3SsrMirKind::RenderComponent(_)))
    {
        let mir_id = state.mir.push_child(
            mir_parent,
            Vue3SsrMirKind::Transition,
            ast_node.span.clone(),
        );
        lower_vue3_ssr_child_sequence(&ast_node.children, ast, hir_id, mir_id, state);
        mark_new_direct_ssr_slots_inner(&mut state.mir, mir_id, 0);
        return Some(mir_id);
    }
    let first_output = state.mir.nodes.len();
    let appear = vue3_ssr_has_static_attr(element, "appear");
    if appear {
        state.mir.push_child(
            mir_parent,
            Vue3SsrMirKind::PushString("<template".into()),
            generated_span.clone(),
        );
        state.mir.push_child(
            mir_parent,
            Vue3SsrMirKind::PushString(">".into()),
            generated_span.clone(),
        );
    }
    lower_vue3_ssr_child_sequence(&ast_node.children, ast, hir_id, mir_parent, state);
    mark_new_direct_ssr_slots_inner(&mut state.mir, mir_parent, first_output);
    if appear {
        state.mir.push_child(
            mir_parent,
            Vue3SsrMirKind::PushString("</template>".into()),
            generated_span,
        );
    }
    state.mir.nodes.get(first_output).map(|node| node.id)
}

pub(crate) fn lower_vue3_transition_group_to_ssr_mir(
    element: &Vue3Element,
    ast: &Vue3Ast,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    hir_id: NodeId,
    mir_parent: NodeId,
    state: &mut Vue3SsrLoweringState,
) -> Option<NodeId> {
    let generated_span =
        NodeSpan::generated(ast_node.span.source(), vuec_ast::GeneratedReason::Lowering);
    let first_output = state.mir.nodes.len();
    let tag = vue3_ssr_transition_group_tag(element, ast_node, state);
    let attrs = vue3_ssr_transition_group_attrs(element, hir_id, state);
    match &tag {
        Some(tag) => {
            state.mir.push_child(
                mir_parent,
                Vue3SsrMirKind::PushString(tag.open_start(&state.options)),
                generated_span.clone(),
            );
            if let Some(attrs) = attrs {
                state.mir.push_child(
                    mir_parent,
                    Vue3SsrMirKind::RenderAttrs(attrs),
                    generated_span.clone(),
                );
            }
            state.mir.push_child(
                mir_parent,
                Vue3SsrMirKind::PushString(">".into()),
                generated_span.clone(),
            );
        }
        None => {
            state.mir.push_child(
                mir_parent,
                Vue3SsrMirKind::PushString("<!--[-->".into()),
                generated_span.clone(),
            );
        }
    }
    let content_start = state.mir.nodes.len();
    state.flatten_ssr_fragments += 1;
    lower_vue3_ssr_child_sequence(&ast_node.children, ast, hir_id, mir_parent, state);
    state.flatten_ssr_fragments -= 1;
    strip_new_direct_transition_group_artifacts(&mut state.mir, mir_parent, content_start);
    mark_new_direct_ssr_slots_inner(&mut state.mir, mir_parent, content_start);
    match tag {
        Some(tag) => {
            state.mir.push_child(
                mir_parent,
                Vue3SsrMirKind::PushString(tag.close()),
                generated_span,
            );
        }
        None => {
            state.mir.push_child(
                mir_parent,
                Vue3SsrMirKind::PushString("<!--]-->".into()),
                generated_span,
            );
        }
    }
    state.mir.nodes.get(first_output).map(|node| node.id)
}

pub(crate) enum Vue3SsrTransitionGroupTag {
    Static(String),
    Dynamic(JsExprId),
}

impl Vue3SsrTransitionGroupTag {
    pub(crate) fn open_start(&self, options: &Vue3CompilerOptions) -> String {
        let scope_id = options
            .scope_id
            .as_ref()
            .map(|scope_id| format!(" {scope_id}"))
            .unwrap_or_default();
        match self {
            Self::Static(tag) => format!("<{tag}{scope_id}"),
            Self::Dynamic(id) => format!("<#expr{}{scope_id}", id.0),
        }
    }

    pub(crate) fn close(&self) -> String {
        match self {
            Self::Static(tag) => format!("</{tag}>"),
            Self::Dynamic(id) => format!("</#expr{}>", id.0),
        }
    }
}

pub(crate) fn vue3_ssr_transition_group_tag(
    element: &Vue3Element,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    state: &mut Vue3SsrLoweringState,
) -> Option<Vue3SsrTransitionGroupTag> {
    element.props.iter().find_map(|prop| match prop {
        Vue3Prop::Attribute(attr) if attr.name == "tag" => Some(Vue3SsrTransitionGroupTag::Static(
            attr.value.clone().unwrap_or_default(),
        )),
        Vue3Prop::Directive(dir)
            if dir.name == "bind"
                && !dir.is_dynamic_arg
                && dir
                    .arg
                    .as_ref()
                    .is_some_and(|arg| arg.source_string() == "tag") =>
        {
            let exp = dir.exp.as_ref().or(dir.arg.as_ref())?;
            Some(Vue3SsrTransitionGroupTag::Dynamic(
                register_or_reuse_vue3_expression_with_span(
                    &mut state.js,
                    exp,
                    dir.exp_span
                        .or(dir.arg_span)
                        .or_else(|| ast_node.span.source()),
                    state.source_type,
                ),
            ))
        }
        _ => None,
    })
}

pub(crate) fn vue3_ssr_transition_group_attrs(
    _element: &Vue3Element,
    hir_id: NodeId,
    state: &Vue3SsrLoweringState,
) -> Option<Vue3SsrAttrs> {
    let mut attrs = match state.hir.node(hir_id).map(|node| &node.kind) {
        Some(HirNodeKind::Component(component)) => component.props.clone(),
        _ => HirProps::default(),
    };
    attrs.static_attrs.retain(|attr| attr.name != "tag");
    attrs
        .dynamic_bindings
        .retain(|binding| binding.dynamic_arg || binding.name != "tag");
    attrs.segments.retain(|segment| match segment {
        HirPropSegment::StaticAttr(attr) => attr.name != "tag",
        HirPropSegment::DynamicBinding(binding) => binding.dynamic_arg || binding.name != "tag",
        HirPropSegment::Event(_)
        | HirPropSegment::ObjectBinding(_)
        | HirPropSegment::ObjectListeners(_) => true,
    });
    let props = lower_hir_props_to_dom_mir_without_event_cache(&attrs);
    if props.static_attrs.is_empty()
        && props.dynamic_bindings.is_empty()
        && props.events.is_empty()
        && props.object_bindings.is_empty()
        && props.object_listeners.is_empty()
        && props.segments.is_empty()
    {
        return None;
    }
    Some(Vue3SsrAttrs {
        props,
        directives: Vec::new(),
        directive_content: false,
        textarea_value_fallback: None,
        force_render_attrs: true,
        v_show: None,
        v_model: None,
    })
}

pub(crate) fn mark_new_direct_ssr_slots_inner(
    mir: &mut Vue3SsrMir,
    parent: NodeId,
    first_output: usize,
) {
    let children = mir
        .node(parent)
        .map(|node| node.children.clone())
        .unwrap_or_default();
    for child_id in children {
        if (child_id.0 as usize) < first_output {
            continue;
        }
        if let Some(node) = mir.node_mut(child_id) {
            match &mut node.kind {
                Vue3SsrMirKind::RenderSlot(slot) => {
                    slot.inner = true;
                }
                Vue3SsrMirKind::If { .. } => {
                    mark_new_direct_ssr_slots_inner(mir, child_id, first_output);
                }
                _ => {}
            }
        }
    }
}

pub(crate) fn strip_new_direct_transition_group_artifacts(
    mir: &mut Vue3SsrMir,
    parent: NodeId,
    first_output: usize,
) {
    let children = mir
        .node(parent)
        .map(|node| node.children.clone())
        .unwrap_or_default();
    for child_id in &children {
        if (child_id.0 as usize) < first_output {
            continue;
        }
        match mir.node(*child_id).map(|node| &node.kind) {
            Some(Vue3SsrMirKind::If { .. }) => {
                strip_new_direct_transition_group_artifacts(mir, *child_id, first_output);
                strip_empty_else_branch_comment(mir, *child_id);
            }
            Some(Vue3SsrMirKind::For(_)) => {
                strip_direct_for_fragment_markers(mir, *child_id);
            }
            _ => {}
        }
    }
    let remove = children
        .iter()
        .copied()
        .filter(|child_id| (child_id.0 as usize) >= first_output)
        .filter(|child_id| {
            mir.node(*child_id).is_some_and(|child| {
                matches!(&child.kind, Vue3SsrMirKind::PushString(value) if value.starts_with("<!--"))
            })
        })
        .collect::<Vec<_>>();
    if let Some(node) = mir.node_mut(parent) {
        node.children
            .retain(|child_id| !remove.iter().any(|remove_id| remove_id == child_id));
    }
}

pub(crate) fn strip_direct_for_fragment_markers(mir: &mut Vue3SsrMir, for_id: NodeId) {
    let children = mir
        .node(for_id)
        .map(|node| node.children.clone())
        .unwrap_or_default();
    let remove_first = children.first().is_some_and(|child_id| {
        mir.node(*child_id).is_some_and(
            |child| matches!(&child.kind, Vue3SsrMirKind::PushString(value) if value == "<!--[-->"),
        )
    });
    let remove_last = children.last().is_some_and(|child_id| {
        mir.node(*child_id).is_some_and(
            |child| matches!(&child.kind, Vue3SsrMirKind::PushString(value) if value == "<!--]-->"),
        )
    });
    if let Some(node) = mir.node_mut(for_id) {
        if remove_first {
            node.children.remove(0);
        }
        if remove_last {
            node.children.pop();
        }
    }
}

pub(crate) fn strip_empty_else_branch_comment(mir: &mut Vue3SsrMir, if_id: NodeId) {
    let alternate = mir.node(if_id).and_then(|node| {
        node.children.iter().copied().find(|child_id| {
            mir.node(*child_id).is_some_and(|child| {
                matches!(
                    child.kind,
                    Vue3SsrMirKind::If {
                        condition: None,
                        ..
                    }
                )
            })
        })
    });
    let Some(alternate) = alternate else {
        return;
    };
    let should_remove = mir.node(alternate).is_some_and(|node| {
        node.children.len() == 1
            && node.children.first().is_some_and(|child_id| {
                mir.node(*child_id).is_some_and(|child| {
                    matches!(&child.kind, Vue3SsrMirKind::PushString(value) if value == "<!---->")
                })
            })
    });
    if should_remove {
        if let Some(node) = mir.node_mut(if_id) {
            node.children.retain(|child_id| *child_id != alternate);
        }
    }
}

pub(crate) fn vue3_ssr_component_tag(
    element: &Vue3Element,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    state: &mut Vue3SsrLoweringState,
) -> (MirExpr, bool) {
    if let Some(value) = vue3_dynamic_component_is_static_value(element) {
        return (MirExpr::String(value), true);
    }
    if let Some(expression) = vue3_dynamic_component_is_expression(element) {
        return (
            MirExpr::JsExpr(register_vue3_expression_with_span(
                &mut state.js,
                expression,
                ast_node.span.source(),
                state.source_type,
            )),
            true,
        );
    }
    if let Some(helper) = vue3_core_component_runtime_helper(&element.tag) {
        return (MirExpr::Helper(helper), false);
    }
    (MirExpr::String(element.tag.clone()), false)
}

pub(crate) fn vue3_ssr_component_is_dynamic(element: &Vue3Element) -> bool {
    vue3_dynamic_component_is_static_value(element).is_some()
        || vue3_dynamic_component_is_expression(element).is_some()
}

pub(crate) fn vue3_dynamic_component_is_static_value(element: &Vue3Element) -> Option<String> {
    if element.tag != "component" && element.tag != "Component" {
        return None;
    }
    element.props.iter().find_map(|prop| match prop {
        Vue3Prop::Attribute(attr) if attr.name == "is" => {
            attr.value.clone().or_else(|| Some(String::new()))
        }
        _ => None,
    })
}

pub(crate) fn filter_vue3_dynamic_component_is_props(props: &HirProps) -> HirProps {
    let mut filtered = props.clone();
    filtered.static_attrs.retain(|attr| attr.name != "is");
    filtered
        .dynamic_bindings
        .retain(|binding| binding.dynamic_arg || binding.name != "is");
    filtered.segments.retain(|segment| match segment {
        HirPropSegment::StaticAttr(attr) => attr.name != "is",
        HirPropSegment::DynamicBinding(binding) => binding.dynamic_arg || binding.name != "is",
        HirPropSegment::Event(_)
        | HirPropSegment::ObjectBinding(_)
        | HirPropSegment::ObjectListeners(_) => true,
    });
    filtered
}

pub(crate) fn vue3_ssr_teleport_target(
    element: &Vue3Element,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    state: &mut Vue3SsrLoweringState,
) -> Option<MirExpr> {
    element.props.iter().find_map(|prop| match prop {
        Vue3Prop::Attribute(attr) if attr.name == "to" => attr
            .value
            .as_ref()
            .map(|value| MirExpr::String(value.clone())),
        Vue3Prop::Directive(dir)
            if dir.name == "bind"
                && !dir.is_dynamic_arg
                && dir
                    .arg
                    .as_ref()
                    .is_some_and(|arg| arg.source_string() == "to") =>
        {
            dir.exp
                .as_ref()
                .filter(|exp| !exp.source_string().trim().is_empty())
                .map(|exp| {
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

pub(crate) fn vue3_ssr_teleport_disabled(
    element: &Vue3Element,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    state: &mut Vue3SsrLoweringState,
) -> MirExpr {
    element
        .props
        .iter()
        .find_map(|prop| match prop {
            Vue3Prop::Attribute(attr) if attr.name == "disabled" => Some(MirExpr::Bool(true)),
            Vue3Prop::Directive(dir)
                if dir.name == "bind"
                    && !dir.is_dynamic_arg
                    && dir
                        .arg
                        .as_ref()
                        .is_some_and(|arg| arg.source_string() == "disabled") =>
            {
                Some(
                    dir.exp
                        .as_ref()
                        .filter(|exp| !exp.source_string().trim().is_empty())
                        .map(|exp| {
                            MirExpr::JsExpr(register_or_reuse_vue3_expression_with_span(
                                &mut state.js,
                                exp,
                                dir.exp_span.or_else(|| ast_node.span.source()),
                                state.source_type,
                            ))
                        })
                        .unwrap_or(MirExpr::Bool(false)),
                )
            }
            _ => None,
        })
        .unwrap_or(MirExpr::Bool(false))
}

pub(crate) fn lower_vue3_slot_outlet_props_to_ssr_mir(props: &HirProps) -> Vue3DomProps {
    let filtered = filter_vue3_slot_outlet_name_props(props);
    lower_hir_props_to_dom_mir_without_event_cache(&filtered)
}

pub(crate) fn lower_hir_props_to_dom_mir_without_event_cache(props: &HirProps) -> Vue3DomProps {
    if !props.segments.is_empty() {
        let mut lowered = Vue3DomProps {
            normalize: Vue3DomPropsNormalize {
                normalize_props: props.object_bindings.len() > 1
                    || !props.object_listeners.is_empty()
                    || props
                        .dynamic_bindings
                        .iter()
                        .any(|binding| binding.dynamic_arg),
                guard_reactive_props: !props.object_bindings.is_empty(),
            },
            ..Vue3DomProps::default()
        };
        for segment in &props.segments {
            match segment {
                HirPropSegment::StaticAttr(attr) => {
                    let attr = lower_hir_static_attr_to_dom_mir(attr);
                    lowered.static_attrs.push(attr.clone());
                    lowered.segments.push(Vue3DomPropSegment::StaticAttr(attr));
                }
                HirPropSegment::DynamicBinding(binding) => {
                    let binding = lower_hir_binding_to_dom_mir(binding);
                    lowered.dynamic_bindings.push(binding.clone());
                    lowered
                        .segments
                        .push(Vue3DomPropSegment::DynamicBinding(binding));
                }
                HirPropSegment::Event(event) => {
                    let event = lower_hir_event_to_dom_mir_without_cache(event);
                    lowered.events.push(event.clone());
                    lowered.segments.push(Vue3DomPropSegment::Event(event));
                }
                HirPropSegment::ObjectBinding(binding) => {
                    let binding = Vue3DomObjectBinding {
                        value: binding.value,
                    };
                    lowered.object_bindings.push(binding.clone());
                    lowered
                        .segments
                        .push(Vue3DomPropSegment::ObjectBinding(binding));
                }
                HirPropSegment::ObjectListeners(listeners) => {
                    let listeners = Vue3DomObjectListeners {
                        value: listeners.value,
                        preserve_case: true,
                    };
                    lowered.object_listeners.push(listeners.clone());
                    lowered
                        .segments
                        .push(Vue3DomPropSegment::ObjectListeners(listeners));
                }
            }
        }
        return lowered;
    }

    Vue3DomProps {
        injected_key: None,
        static_attrs: props
            .static_attrs
            .iter()
            .map(lower_hir_static_attr_to_dom_mir)
            .collect(),
        dynamic_bindings: props
            .dynamic_bindings
            .iter()
            .map(lower_hir_binding_to_dom_mir)
            .collect(),
        events: props
            .events
            .iter()
            .map(lower_hir_event_to_dom_mir_without_cache)
            .collect(),
        object_bindings: props
            .object_bindings
            .iter()
            .map(|binding| Vue3DomObjectBinding {
                value: binding.value,
            })
            .collect(),
        object_listeners: props
            .object_listeners
            .iter()
            .map(|listeners| Vue3DomObjectListeners {
                value: listeners.value,
                preserve_case: true,
            })
            .collect(),
        segments: Vec::new(),
        normalize: Vue3DomPropsNormalize {
            normalize_props: props.object_bindings.len() > 1
                || !props.object_listeners.is_empty()
                || props
                    .dynamic_bindings
                    .iter()
                    .any(|binding| binding.dynamic_arg),
            guard_reactive_props: !props.object_bindings.is_empty(),
        },
    }
}

pub(crate) fn lower_hir_component_props_to_ssr_mir(props: &HirProps) -> Vue3DomProps {
    let mut lowered = lower_hir_props_to_dom_mir_without_event_cache(props);
    for event in &mut lowered.events {
        if !event.dynamic_arg {
            event.name = event_handler_prop_name_for_component(&event.name);
        }
    }
    for segment in &mut lowered.segments {
        if let Vue3DomPropSegment::Event(event) = segment {
            if !event.dynamic_arg {
                event.name = event_handler_prop_name_for_component(&event.name);
            }
        }
    }
    lowered
}

pub(crate) fn lower_hir_event_to_dom_mir_without_cache(event: &HirEvent) -> Vue3DomEvent {
    let modifiers = vue3_dom_event_modifiers_for(
        &event_handler_prop_name_for_element(&event.name),
        event.dynamic_arg,
        &event.modifiers,
    );
    Vue3DomEvent {
        name: event.name.clone(),
        dynamic_name: event.dynamic_name,
        handler: event.handler,
        dynamic_arg: event.dynamic_arg,
        runtime_modifiers: modifiers.runtime_modifiers,
        key_modifiers: modifiers.key_modifiers,
        option_modifiers: modifiers.option_modifiers,
        click_event: modifiers.click_event,
        cache: None,
    }
}

pub(crate) fn lower_vue3_component_directives_to_ssr_mir(
    element: &Vue3Element,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    state: &mut Vue3SsrLoweringState,
) -> Vec<Vue3DomDirective> {
    lower_vue3_directives_to_hir(&element.props, ast_node, &mut state.js, state.source_type)
        .into_iter()
        .filter(|directive| directive.name != "show")
        .map(|directive| lower_hir_directive_to_dom_mir(&directive))
        .collect()
}

pub(crate) fn lower_vue3_element_control_flow_to_ssr_mir(
    ast_id: NodeId,
    element: &Vue3Element,
    ast: &Vue3Ast,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue3SsrLoweringState,
) -> Option<Option<NodeId>> {
    let if_dir = directive_by_name(element, "if")
        .or_else(|| directive_by_name(element, "else-if"))
        .filter(|dir| dir.exp.is_some());
    if let Some(if_dir) = if_dir {
        let lowered = lower_vue3_if_directive_to_ssr_mir(
            ast_id, element, ast, ast_node, if_dir, hir_parent, mir_parent, state,
        );
        return Some(lowered);
    }
    if let Some(for_dir) = directive_by_name(element, "for") {
        let lowered = lower_vue3_for_directive_to_ssr_mir(
            ast_id, element, ast, ast_node, for_dir, hir_parent, mir_parent, state,
        );
        return Some(lowered);
    }
    None
}

pub(crate) fn lower_vue3_if_branch_chain_to_ssr_mir(
    branch_ids: &[NodeId],
    ast: &Vue3Ast,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue3SsrLoweringState,
) -> Option<NodeId> {
    let first_id = *branch_ids.first()?;
    let first_node = ast.node(first_id)?;
    let first_element = match &first_node.kind {
        Vue3AstKind::Element(element) => element,
        _ => return None,
    };
    let first_dir = directive_by_name(first_element, "if")
        .or_else(|| directive_by_name(first_element, "else-if"))?;
    let condition =
        lower_vue3_optional_condition(first_dir, first_node, &mut state.js, state.source_type);
    let hir_id = state.hir.push_child(
        hir_parent,
        HirNodeKind::If(HirIf {
            branches: Vec::new(),
        }),
        first_node.span.clone(),
    );
    let mir_id = state.mir.push_child(
        mir_parent,
        Vue3SsrMirKind::If {
            condition,
            comment: state.flatten_ssr_fragments == 0,
        },
        first_node.span.clone(),
    );
    state.map.record_ast_to_hir(first_id, hir_id);
    state.map.record_hir_to_mir(hir_id, mir_id);

    let mut previous_branch_mir = mir_id;
    for branch_id in branch_ids {
        let Some(branch_node) = ast.node(*branch_id) else {
            continue;
        };
        let Vue3AstKind::Element(branch_element) = &branch_node.kind else {
            continue;
        };
        let condition = if *branch_id == first_id {
            condition
        } else {
            vue3_branch_condition(
                branch_element,
                branch_node,
                &mut state.js,
                state.source_type,
            )
        };
        let branch_mir = if *branch_id != first_id {
            let branch_mir = state.mir.push_child(
                previous_branch_mir,
                Vue3SsrMirKind::If {
                    condition,
                    comment: state.flatten_ssr_fragments == 0,
                },
                branch_node.span.clone(),
            );
            state.map.record_ast_to_hir(*branch_id, hir_id);
            state.map.record_hir_to_mir(hir_id, branch_mir);
            branch_mir
        } else {
            mir_id
        };
        let body = if let Some(for_dir) = directive_by_name(branch_element, "for") {
            lower_vue3_for_directive_to_ssr_mir(
                *branch_id,
                branch_element,
                ast,
                branch_node,
                for_dir,
                hir_id,
                branch_mir,
                state,
            )
        } else {
            lower_vue3_plain_element_to_ssr_mir(
                *branch_id,
                branch_element,
                ast,
                branch_node,
                hir_id,
                branch_mir,
                state,
            )
        };
        if let Some(body_hir) = body {
            if let Some(node) = state.hir.node_mut(hir_id) {
                if let HirNodeKind::If(hir_if) = &mut node.kind {
                    hir_if.branches.push(HirIfBranch {
                        condition,
                        body: body_hir,
                    });
                }
            }
        }
        previous_branch_mir = branch_mir;
    }

    Some(hir_id)
}

pub(crate) fn lower_vue3_for_directive_to_ssr_mir(
    ast_id: NodeId,
    element: &Vue3Element,
    ast: &Vue3Ast,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    directive: &Vue3Directive,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue3SsrLoweringState,
) -> Option<NodeId> {
    let parsed = lower_vue3_for_directive(directive, ast_node, &mut state.js, state.source_type)?;
    let hir_id = state.hir.push_child(
        hir_parent,
        HirNodeKind::For(HirFor {
            source: parsed.source,
            value_alias: parsed.value_alias,
            key_alias: parsed.key_alias,
            index_alias: parsed.index_alias,
            body: NodeId(0),
        }),
        ast_node.span.clone(),
    );
    let mir_id = state.mir.push_child(
        mir_parent,
        Vue3SsrMirKind::For(Vue3SsrFor {
            source: parsed.source,
            value_alias: parsed.value_alias,
            key_alias: parsed.key_alias,
            index_alias: parsed.index_alias,
            fragment: state.flatten_ssr_fragments == 0,
        }),
        ast_node.span.clone(),
    );
    state.map.record_ast_to_hir(ast_id, hir_id);
    state.map.record_hir_to_mir(hir_id, mir_id);

    let body =
        lower_vue3_plain_element_to_ssr_mir(ast_id, element, ast, ast_node, hir_id, mir_id, state);
    if let Some(body_hir) = body {
        if let Some(node) = state.hir.node_mut(hir_id) {
            if let HirNodeKind::For(hir_for) = &mut node.kind {
                hir_for.body = body_hir;
            }
        }
    }
    Some(hir_id)
}

pub(crate) fn lower_vue3_if_directive_to_ssr_mir(
    ast_id: NodeId,
    element: &Vue3Element,
    ast: &Vue3Ast,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    directive: &Vue3Directive,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue3SsrLoweringState,
) -> Option<NodeId> {
    let condition =
        lower_vue3_optional_condition(directive, ast_node, &mut state.js, state.source_type);
    let hir_id = state.hir.push_child(
        hir_parent,
        HirNodeKind::If(HirIf {
            branches: Vec::new(),
        }),
        ast_node.span.clone(),
    );
    let mir_id = state.mir.push_child(
        mir_parent,
        Vue3SsrMirKind::If {
            condition,
            comment: state.flatten_ssr_fragments == 0,
        },
        ast_node.span.clone(),
    );
    state.map.record_ast_to_hir(ast_id, hir_id);
    state.map.record_hir_to_mir(hir_id, mir_id);

    let body = if let Some(for_dir) = directive_by_name(element, "for") {
        lower_vue3_for_directive_to_ssr_mir(
            ast_id, element, ast, ast_node, for_dir, hir_id, mir_id, state,
        )
    } else {
        lower_vue3_plain_element_to_ssr_mir(
            ast_id, element, ast, ast_node, hir_id, mir_id, state,
        )
    };
    if let Some(body_hir) = body {
        if let Some(node) = state.hir.node_mut(hir_id) {
            if let HirNodeKind::If(hir_if) = &mut node.kind {
                hir_if.branches.push(HirIfBranch {
                    condition,
                    body: body_hir,
                });
            }
        }
    }
    Some(hir_id)
}

pub(crate) fn lower_vue3_native_element_to_ssr_mir(
    element: &Vue3Element,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    ast: &Vue3Ast,
    hir_id: NodeId,
    mir_parent: NodeId,
    state: &mut Vue3SsrLoweringState,
) {
    let generated_span =
        NodeSpan::generated(ast_node.span.source(), vuec_ast::GeneratedReason::Lowering);
    let v_show = lower_vue3_ssr_v_show(hir_id, state);
    let select_model = if element.tag == "select" {
        vue3_ssr_v_model_expression(element, ast_node, state)
    } else {
        None
    };
    let v_model = lower_vue3_ssr_v_model(element, ast_node, state);
    let content = lower_vue3_ssr_content(element, ast_node, state);
    let textarea_static_value = (element.tag == "textarea" && content.is_none())
        .then(|| vue3_ssr_static_textarea_value(element))
        .flatten();
    let textarea_value_fallback = (element.tag == "textarea"
        && content.is_none()
        && textarea_static_value.is_none()
        && vue3_ssr_has_object_v_bind(element))
    .then(|| vue3_ssr_static_textarea_fallback(ast_node, ast))
    .flatten();
    let is_void = state
        .options
        .void_tags
        .iter()
        .any(|candidate| candidate == &element.tag);
    let has_content_override = content.is_some()
        || textarea_static_value.is_some()
        || textarea_value_fallback.is_some()
        || matches!(
            v_model.as_ref().map(|model| &model.kind),
            Some(Vue3SsrModelKind::Textarea)
        );
    let directive_content = !is_void
        && !has_content_override
        && ast_node.children.is_empty()
        && state.hir.node(hir_id).is_some_and(|node| {
            matches!(
                &node.kind,
                HirNodeKind::Element(element)
                    if element.directives.iter().any(|directive| directive.name != "show")
            )
        });
    let open_id = state.mir.push_child(
        mir_parent,
        Vue3SsrMirKind::PushString(vue3_ssr_open_tag_start(
            element,
            v_show.is_some(),
            v_model.as_ref(),
            &state.options,
        )),
        generated_span.clone(),
    );
    state.map.record_hir_to_mir(hir_id, open_id);

    if let Some(attrs) = lower_vue3_ssr_attrs(
        hir_id,
        v_show,
        v_model.clone(),
        directive_content,
        textarea_value_fallback.clone(),
        state,
    ) {
        let attrs_id = state.mir.push_child(
            mir_parent,
            Vue3SsrMirKind::RenderAttrs(attrs),
            generated_span.clone(),
        );
        state.map.record_hir_to_mir(hir_id, attrs_id);
    }

    let close_open_id = state.mir.push_child(
        mir_parent,
        Vue3SsrMirKind::PushString(
            if is_void && !has_content_override {
                "/>"
            } else {
                ">"
            }
            .into(),
        ),
        generated_span.clone(),
    );
    state.map.record_hir_to_mir(hir_id, close_open_id);

    if is_void && !has_content_override {
        return;
    }

    if let Some(model) = select_model {
        state.select_model_stack.push(model);
    }
    if let Some(content) = content {
        let content_id = state.mir.push_child(
            mir_parent,
            Vue3SsrMirKind::RenderContent(content),
            generated_span.clone(),
        );
        state.map.record_hir_to_mir(hir_id, content_id);
    } else if let Some(value) = textarea_static_value {
        let text_id = state.mir.push_child(
            mir_parent,
            Vue3SsrMirKind::PushString(escape_static_html_text(&value)),
            generated_span.clone(),
        );
        state.map.record_hir_to_mir(hir_id, text_id);
    } else if textarea_value_fallback.is_some() {
        // The content expression is emitted together with the attrs temp in codegen.
    } else if let Some(expression) = v_model
        .as_ref()
        .filter(|model| matches!(model.kind, Vue3SsrModelKind::Textarea))
        .map(|model| model.expression)
    {
        let text_id = state.mir.push_child(
            mir_parent,
            Vue3SsrMirKind::PushInterpolated(MirExpr::JsExpr(expression)),
            generated_span.clone(),
        );
        state.map.record_hir_to_mir(hir_id, text_id);
    } else {
        lower_vue3_ssr_child_sequence(&ast_node.children, ast, hir_id, mir_parent, state);
    }
    if select_model.is_some() {
        state.select_model_stack.pop();
    }

    let close_id = state.mir.push_child(
        mir_parent,
        Vue3SsrMirKind::PushString(format!("</{}>", element.tag)),
        generated_span,
    );
    state.map.record_hir_to_mir(hir_id, close_id);
}
