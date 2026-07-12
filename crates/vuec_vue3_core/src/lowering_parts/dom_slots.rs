pub(crate) fn lower_vue3_dom_child_sequence(
    children: &[NodeId],
    ast: &Vue3Ast,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue3DomLoweringState,
) {
    let mut index = 0usize;
    let mut branch_key_base = 0usize;
    while index < children.len() {
        let child_id = children[index];
        let Some(child) = ast.node(child_id) else {
            index += 1;
            continue;
        };
        if let Vue3AstKind::Element(element) = &child.kind {
            if directive_by_name(element, "if").is_some() {
                let (branch_ids, next_index) = collect_vue3_if_branch_chain(children, index, ast);
                lower_vue3_if_branch_chain_to_dom_mir(
                    &branch_ids,
                    branch_key_base,
                    ast,
                    hir_parent,
                    mir_parent,
                    state,
                );
                branch_key_base = branch_key_base.saturating_add(branch_ids.len());
                index = next_index;
                continue;
            }
            if directive_by_name(element, "for").is_some() {
                lower_vue3_ast_node_to_dom_mir(child_id, ast, hir_parent, mir_parent, state);
                index += 1;
                continue;
            }
            if is_else_branch(element) {
                index += 1;
                continue;
            }
        }
        lower_vue3_ast_node_to_dom_mir(child_id, ast, hir_parent, mir_parent, state);
        index += 1;
    }
}

pub(crate) fn collect_vue3_if_branch_chain(
    children: &[NodeId],
    start: usize,
    ast: &Vue3Ast,
) -> (Vec<NodeId>, usize) {
    let mut branches = vec![children[start]];
    let mut index = start + 1;
    while index < children.len() {
        let Some(node) = ast.node(children[index]) else {
            index += 1;
            continue;
        };
        match &node.kind {
            Vue3AstKind::Comment(_) => {
                index += 1;
            }
            Vue3AstKind::Text(text) if text.value.trim().is_empty() => {
                index += 1;
            }
            Vue3AstKind::Element(element) if is_else_branch(element) => {
                branches.push(children[index]);
                index += 1;
            }
            _ => break,
        }
    }
    (branches, index)
}

pub(crate) fn lower_vue3_ast_node_to_dom_mir(
    ast_id: NodeId,
    ast: &Vue3Ast,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue3DomLoweringState,
) -> Option<(NodeId, NodeId)> {
    let ast_node = ast.node(ast_id)?;
    match &ast_node.kind {
        Vue3AstKind::Root(_) => None,
        Vue3AstKind::Element(element) => {
            if let Some(lowered) = lower_vue3_element_control_flow_to_dom_mir(
                ast_id, element, ast, ast_node, hir_parent, mir_parent, state,
            ) {
                return lowered;
            }
            lower_vue3_non_control_element_to_dom_mir(
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
                Vue3DomMirKind::TextCall {
                    value: MirExpr::String(text.value.clone()),
                },
                ast_node.span.clone(),
            );
            state.map.record_ast_to_hir(ast_id, hir_id);
            state.map.record_hir_to_mir(hir_id, mir_id);
            Some((hir_id, mir_id))
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
                Vue3DomMirKind::Interpolation { expression: expr },
                ast_node.span.clone(),
            );
            state.map.record_ast_to_hir(ast_id, hir_id);
            state.map.record_hir_to_mir(hir_id, mir_id);
            Some((hir_id, mir_id))
        }
        Vue3AstKind::Comment(comment) => {
            let hir_id = state.hir.push_child(
                hir_parent,
                HirNodeKind::Fragment(HirFragment),
                NodeSpan::generated(ast_node.span.source(), vuec_ast::GeneratedReason::Lowering),
            );
            let mir_id = state.mir.push_child(
                mir_parent,
                Vue3DomMirKind::TextCall {
                    value: MirExpr::String(format!("<!--{}-->", comment.value)),
                },
                NodeSpan::generated(ast_node.span.source(), vuec_ast::GeneratedReason::Lowering),
            );
            state.map.record_ast_to_hir(ast_id, hir_id);
            state.map.record_hir_to_mir(hir_id, mir_id);
            Some((hir_id, mir_id))
        }
        Vue3AstKind::CompoundExpression(_) | Vue3AstKind::TextCall(_) => {
            let hir_id = state.hir.push_child(
                hir_parent,
                HirNodeKind::Fragment(HirFragment),
                ast_node.span.clone(),
            );
            let mir_id =
                state
                    .mir
                    .push_child(mir_parent, Vue3DomMirKind::Fragment, ast_node.span.clone());
            state.map.record_ast_to_hir(ast_id, hir_id);
            state.map.record_hir_to_mir(hir_id, mir_id);
            Some((hir_id, mir_id))
        }
        Vue3AstKind::If(_) | Vue3AstKind::IfBranch(_) | Vue3AstKind::For(_) => None,
    }
}

pub(crate) fn lower_vue3_plain_element_to_dom_mir(
    ast_id: NodeId,
    element: &Vue3Element,
    ast: &Vue3Ast,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue3DomLoweringState,
) -> Option<(NodeId, NodeId)> {
    let hir_kind =
        lower_vue3_element_to_hir_kind(element, ast_node, &mut state.js, state.source_type);
    let mir_kind =
        lower_vue3_element_to_dom_mir_kind(element, ast, ast_id, ast_node, &hir_kind, state);
    let hir_id = state
        .hir
        .push_child(hir_parent, hir_kind, ast_node.span.clone());
    let mir_id = state
        .mir
        .push_child(mir_parent, mir_kind, ast_node.span.clone());
    state.map.record_ast_to_hir(ast_id, hir_id);
    state.map.record_hir_to_mir(hir_id, mir_id);
    lower_vue3_dom_children(ast_node, ast, hir_id, mir_id, state);
    Some((hir_id, mir_id))
}

pub(crate) fn lower_vue3_non_control_element_to_dom_mir(
    ast_id: NodeId,
    element: &Vue3Element,
    ast: &Vue3Ast,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue3DomLoweringState,
) -> Option<(NodeId, NodeId)> {
    if let Some(memo) =
        directive_by_name(element, "memo").filter(|_| directive_by_name(element, "for").is_none())
    {
        let expression = register_vue3_expression_with_span(
            &mut state.js,
            memo.exp
                .as_ref()
                .unwrap_or(&Vue3Expression::Raw(String::new())),
            memo.exp_span.or_else(|| ast_node.span.source()),
            state.source_type,
        );
        let cache_id = state.next_cache_index;
        state.next_cache_index += 1;
        let wrapper_id = state.mir.push_child(
            mir_parent,
            Vue3DomMirKind::Memo {
                expression,
                index: cache_id,
            },
            NodeSpan::generated(ast_node.span.source(), vuec_ast::GeneratedReason::Lowering),
        );
        let lowered = lower_vue3_plain_element_to_dom_mir(
            ast_id, element, ast, ast_node, hir_parent, wrapper_id, state,
        );
        if let Some((hir_id, _)) = lowered {
            state.map.record_hir_to_mir(hir_id, wrapper_id);
            return Some((hir_id, wrapper_id));
        }
        return None;
    }

    if directive_by_name(element, "once").is_some() {
        return lower_vue3_with_once_cache(
            element,
            ast_node,
            mir_parent,
            state,
            |wrapper_id, state| {
                lower_vue3_plain_element_to_dom_mir(
                    ast_id, element, ast, ast_node, hir_parent, wrapper_id, state,
                )
            },
        );
    }

    if state.options.hoist_static
        && state.in_v_once == 0
        && state.in_static_hoist == 0
        && state.do_not_hoist_root != Some(ast_id)
        && vue3_dom_mir_can_hoist_static_node(ast, ast_id)
    {
        let hoist_id = state.next_hoist_index;
        state.next_hoist_index += 1;
        let wrapper_id = state.mir.push_child(
            mir_parent,
            Vue3DomMirKind::Hoisted { index: hoist_id },
            NodeSpan::generated(ast_node.span.source(), vuec_ast::GeneratedReason::Lowering),
        );
        state.in_static_hoist += 1;
        let lowered = lower_vue3_plain_element_to_dom_mir(
            ast_id, element, ast, ast_node, hir_parent, wrapper_id, state,
        );
        state.in_static_hoist -= 1;
        if let Some((hir_id, _)) = lowered {
            state.map.record_hir_to_mir(hir_id, wrapper_id);
            return Some((hir_id, wrapper_id));
        }
        return None;
    }

    lower_vue3_plain_element_to_dom_mir(
        ast_id, element, ast, ast_node, hir_parent, mir_parent, state,
    )
}

pub(crate) fn lower_vue3_with_once_cache(
    element: &Vue3Element,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    mir_parent: NodeId,
    state: &mut Vue3DomLoweringState,
    lower: impl FnOnce(NodeId, &mut Vue3DomLoweringState) -> Option<(NodeId, NodeId)>,
) -> Option<(NodeId, NodeId)> {
    if directive_by_name(element, "once").is_none() || state.in_v_once > 0 {
        return lower(mir_parent, state);
    }

    let cache_id = state.next_cache_index;
    state.next_cache_index += 1;
    let wrapper_id = state.mir.push_child(
        mir_parent,
        Vue3DomMirKind::Cache { index: cache_id },
        NodeSpan::generated(ast_node.span.source(), vuec_ast::GeneratedReason::Lowering),
    );
    state.in_v_once += 1;
    let lowered = lower(wrapper_id, state);
    state.in_v_once -= 1;
    let (hir_id, _) = lowered?;
    state.map.record_hir_to_mir(hir_id, wrapper_id);
    Some((hir_id, wrapper_id))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Vue3StructuralTemplateBodyKind {
    If,
    For,
}

pub(crate) fn lower_vue3_structural_template_body_to_dom_mir(
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    ast: &Vue3Ast,
    hir_parent: NodeId,
    mir_parent: NodeId,
    injected_key: Option<Vue3DomKey>,
    body_kind: Vue3StructuralTemplateBodyKind,
    state: &mut Vue3DomLoweringState,
) -> Option<(NodeId, NodeId)> {
    if let Some(child_id) = vue3_structural_template_direct_child(ast_node, ast, body_kind) {
        let previous_do_not_hoist_root = state.do_not_hoist_root.replace(child_id);
        let lowered = lower_vue3_ast_node_to_dom_mir(
            child_id, ast, hir_parent, mir_parent, state,
        );
        state.do_not_hoist_root = previous_do_not_hoist_root;
        let lowered = lowered?;
        if let Some(key) = injected_key {
            inject_vue3_dom_key(lowered.1, key, state);
        }
        return Some(lowered);
    }

    let span = NodeSpan::generated(
        ast_node.span.source(),
        vuec_ast::GeneratedReason::Lowering,
    );
    let hir_id = state
        .hir
        .push_child(hir_parent, HirNodeKind::Fragment(HirFragment), span.clone());
    let mir_id = state.mir.push_child(
        mir_parent,
        Vue3DomMirKind::VNodeCall(Vue3VNodeCall {
            tag: Vue3DomTag::RuntimeHelper(RuntimeHelper::Vue3Fragment),
            props: Vue3DomProps {
                injected_key,
                ..Vue3DomProps::default()
            },
            v_show: None,
            directives: Vec::new(),
            models: Vec::new(),
            content: None,
            children: MirChildren::Nodes(Vec::new()),
            patch_flag: Vue3PatchFlags { bits: 64 },
            dynamic_props: Vec::new(),
            is_block: true,
            disable_tracking: false,
            is_component: false,
        }),
        span,
    );
    state.map.record_hir_to_mir(hir_id, mir_id);
    lower_vue3_dom_child_sequence(&ast_node.children, ast, hir_id, mir_id, state);
    let children = state
        .mir
        .node(mir_id)
        .map(|node| node.children.clone())
        .unwrap_or_default();
    if let Some(node) = state.mir.node_mut(mir_id) {
        if let Vue3DomMirKind::VNodeCall(call) = &mut node.kind {
            call.children = MirChildren::Nodes(children);
        }
    }
    Some((hir_id, mir_id))
}

fn vue3_structural_template_direct_child(
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    ast: &Vue3Ast,
    body_kind: Vue3StructuralTemplateBodyKind,
) -> Option<NodeId> {
    let [child_id] = ast_node.children.as_slice() else {
        return None;
    };
    let child = ast.node(*child_id)?;
    let Vue3AstKind::Element(element) = &child.kind else {
        return None;
    };
    if directive_by_name(element, "if").is_some() || is_else_branch(element) {
        return None;
    }
    if body_kind == Vue3StructuralTemplateBodyKind::For
        && directive_by_name(element, "for").is_some()
    {
        return None;
    }
    Some(*child_id)
}

pub(crate) fn inject_vue3_dom_key(
    mir_id: NodeId,
    key: Vue3DomKey,
    state: &mut Vue3DomLoweringState,
) {
    let wrapper_child = {
        let Some(node) = state.mir.node_mut(mir_id) else {
            return;
        };
        match &mut node.kind {
            Vue3DomMirKind::VNodeCall(call) => {
                call.is_block = true;
                inject_vue3_dom_key_into_props(&mut call.props, key);
                return;
            }
            Vue3DomMirKind::RenderSlot(slot) => {
                inject_vue3_dom_key_into_props(&mut slot.props, key);
                return;
            }
            Vue3DomMirKind::For(for_mir) => {
                if let Vue3DomKey::Branch(branch_key) = key {
                    for_mir.branch_key = Some(branch_key);
                }
                return;
            }
            Vue3DomMirKind::WithDirectives
            | Vue3DomMirKind::Cache { .. }
            | Vue3DomMirKind::Memo { .. }
            | Vue3DomMirKind::Hoisted { .. } => node.children.first().copied(),
            _ => None,
        }
    };
    if let Some(child) = wrapper_child {
        inject_vue3_dom_key(child, key, state);
    }
}

pub(crate) fn set_vue3_dom_for_body_block(
    mir_id: NodeId,
    is_stable: bool,
    state: &mut Vue3DomLoweringState,
) {
    let wrapper_child = {
        let Some(node) = state.mir.node_mut(mir_id) else {
            return;
        };
        match &mut node.kind {
            Vue3DomMirKind::VNodeCall(call) => {
                if call.tag != Vue3DomTag::RuntimeHelper(RuntimeHelper::Vue3Fragment) {
                    call.is_block = !is_stable;
                }
                return;
            }
            Vue3DomMirKind::WithDirectives
            | Vue3DomMirKind::Cache { .. }
            | Vue3DomMirKind::Memo { .. }
            | Vue3DomMirKind::Hoisted { .. } => node.children.first().copied(),
            _ => None,
        }
    };
    if let Some(child) = wrapper_child {
        set_vue3_dom_for_body_block(child, is_stable, state);
    }
}

fn inject_vue3_dom_key_into_props(props: &mut Vue3DomProps, key: Vue3DomKey) {
    if props.injected_key.is_none() && !vue3_dom_props_has_explicit_key(props) {
        props.injected_key = Some(key);
    }
}

pub(crate) fn vue3_dom_props_has_explicit_key(props: &Vue3DomProps) -> bool {
    props.static_attrs.iter().any(|attr| attr.name == "key")
        || props
            .dynamic_bindings
            .iter()
            .any(|binding| !binding.dynamic_arg && binding.name == "key")
}

pub(crate) fn lower_vue3_dom_children(
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    ast: &Vue3Ast,
    hir_id: NodeId,
    mir_id: NodeId,
    state: &mut Vue3DomLoweringState,
) {
    if let Vue3AstKind::Element(element) = &ast_node.kind {
        if element.tag_type == Vue3ElementType::SlotOutlet {
            let fallback =
                lower_vue3_slot_outlet_fallback_to_dom_mir(ast_node, ast, hir_id, mir_id, state);
            if let Some(node) = state.mir.node_mut(mir_id) {
                if let Vue3DomMirKind::RenderSlot(slot) = &mut node.kind {
                    slot.fallback = fallback;
                }
            }
            return;
        }
    }

    if let Some(node) = state.mir.node_mut(mir_id) {
        if let Vue3DomMirKind::VNodeCall(call) = &mut node.kind {
            if call.content.is_some() {
                call.children = MirChildren::None;
                return;
            }
        }
    }

    if let Some(slots) = lower_vue3_component_slots_to_dom_mir(ast_node, ast, hir_id, mir_id, state)
    {
        if let Some(node) = state.mir.node_mut(mir_id) {
            if let Vue3DomMirKind::VNodeCall(call) = &mut node.kind {
                call.children = MirChildren::Slots(slots);
            }
        }
        return;
    }

    let mut child_mir_ids = Vec::new();
    let before = state.mir.nodes.len();
    lower_vue3_dom_child_sequence(&ast_node.children, ast, hir_id, mir_id, state);
    for node in state.mir.nodes.iter().skip(before) {
        if node.parent == Some(mir_id) {
            child_mir_ids.push(node.id);
        }
    }
    if let Some(node) = state.mir.node_mut(mir_id) {
        if let Vue3DomMirKind::VNodeCall(call) = &mut node.kind {
            call.children = if child_mir_ids.is_empty() {
                MirChildren::None
            } else {
                MirChildren::Nodes(child_mir_ids)
            };
        }
    }
}

pub(crate) fn lower_vue3_slot_outlet_fallback_to_dom_mir(
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    ast: &Vue3Ast,
    hir_id: NodeId,
    mir_id: NodeId,
    state: &mut Vue3DomLoweringState,
) -> Vec<NodeId> {
    let before = state.mir.nodes.len();
    lower_vue3_dom_child_sequence(&ast_node.children, ast, hir_id, mir_id, state);
    state
        .mir
        .nodes
        .iter()
        .skip(before)
        .filter(|node| node.parent == Some(mir_id))
        .map(|node| node.id)
        .collect()
}

pub(crate) fn lower_vue3_component_slots_to_dom_mir(
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    ast: &Vue3Ast,
    hir_id: NodeId,
    mir_id: NodeId,
    state: &mut Vue3DomLoweringState,
) -> Option<vuec_ast::Vue3DomSlots> {
    let Vue3AstKind::Element(element) = &ast_node.kind else {
        return None;
    };
    if element.tag_type != Vue3ElementType::Component {
        return None;
    }
    let on_component_slot = directive_by_name(element, "slot");
    if on_component_slot.is_some_and(|slot| slot.is_dynamic_arg) {
        return None;
    }
    let visible = visible_child_ids(ast, &ast_node.children);
    if visible.is_empty() {
        return None;
    }
    let slots_are_stable = component_children_are_stable_slots(ast, &visible);

    let mut slots = Vec::new();
    let mut dynamic_slots = Vec::new();
    let mut default_children = Vec::new();
    let mut index = 0usize;
    while index < visible.len() {
        let child_id = visible[index];
        let Some(child) = ast.node(child_id) else {
            index += 1;
            continue;
        };
        if let Vue3AstKind::Element(child_element) = &child.kind {
            if let Some(slot) = directive_by_name(child_element, "slot") {
                if slots_are_stable {
                    slots.push(lower_vue3_static_slot_to_dom_mir(
                        child_id,
                        child,
                        slot,
                        &child.children,
                        ast,
                        hir_id,
                        mir_id,
                        state,
                    ));
                    index += 1;
                } else if let Some(dynamic_slot) =
                    lower_vue3_dynamic_slot_to_dom_mir(&visible, index, ast, hir_id, mir_id, state)
                {
                    let next_index = if directive_by_name(child_element, "if").is_some() {
                        collect_vue3_dynamic_slot_branch_chain(&visible, index, ast).1
                    } else {
                        index + 1
                    };
                    dynamic_slots.push(dynamic_slot);
                    index = next_index;
                } else {
                    index += 1;
                }
                continue;
            }
        }
        if slots_are_stable {
            default_children.push(child_id);
        }
        index += 1;
    }
    if slots_are_stable && !default_children.is_empty() {
        slots.push(lower_vue3_default_slot_to_dom_mir(
            ast_node,
            on_component_slot,
            &default_children,
            ast,
            hir_id,
            mir_id,
            state,
        ));
    }
    if slots.is_empty() && dynamic_slots.is_empty() {
        None
    } else {
        let flag = if slots_are_stable {
            vue3_dom_stable_slot_flag(ast, &visible)
        } else {
            Vue3SlotFlag::Dynamic
        };
        Some(vuec_ast::Vue3DomSlots {
            slots,
            dynamic_slots,
            flag,
        })
    }
}

pub(crate) fn lower_vue3_static_slot_to_dom_mir(
    ast_id: NodeId,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    slot: &Vue3Directive,
    children: &[NodeId],
    ast: &Vue3Ast,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue3DomLoweringState,
) -> vuec_ast::Vue3DomSlot {
    let name = vue3_static_slot_directive_name(slot);
    let params = register_vue3_slot_params(slot, ast_node.span.source(), state);
    let slot_hir_id = lower_vue3_slot_decl_to_hir(
        ast_id,
        hir_parent,
        mir_parent,
        name.clone(),
        params,
        slot.span
            .map(NodeSpan::from)
            .unwrap_or_else(|| ast_node.span.clone()),
        state,
    );
    vuec_ast::Vue3DomSlot {
        name,
        params,
        children: lower_vue3_slot_children_to_dom_mir(
            children,
            ast,
            slot_hir_id,
            mir_parent,
            state,
        ),
    }
}

pub(crate) fn lower_vue3_default_slot_to_dom_mir(
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    on_component_slot: Option<&Vue3Directive>,
    children: &[NodeId],
    ast: &Vue3Ast,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue3DomLoweringState,
) -> vuec_ast::Vue3DomSlot {
    let slot_name = on_component_slot
        .map(vue3_static_slot_directive_name)
        .unwrap_or_else(|| "default".into());
    let slot_params = on_component_slot
        .and_then(|slot| register_vue3_slot_params(slot, ast_node.span.source(), state));
    let slot_hir_id = lower_vue3_slot_decl_to_hir(
        ast_node.id,
        hir_parent,
        mir_parent,
        slot_name.clone(),
        slot_params,
        on_component_slot
            .and_then(|slot| slot.span)
            .map(NodeSpan::from)
            .unwrap_or_else(|| {
                NodeSpan::generated(ast_node.span.source(), vuec_ast::GeneratedReason::Lowering)
            }),
        state,
    );
    vuec_ast::Vue3DomSlot {
        name: slot_name,
        params: slot_params,
        children: lower_vue3_slot_children_to_dom_mir(
            children,
            ast,
            slot_hir_id,
            mir_parent,
            state,
        ),
    }
}

pub(crate) fn lower_vue3_dynamic_slot_to_dom_mir(
    visible: &[NodeId],
    index: usize,
    ast: &Vue3Ast,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue3DomLoweringState,
) -> Option<vuec_ast::Vue3DomDynamicSlot> {
    let ast_id = *visible.get(index)?;
    let ast_node = ast.node(ast_id)?;
    let Vue3AstKind::Element(element) = &ast_node.kind else {
        return None;
    };
    let slot = directive_by_name(element, "slot")?;
    if directive_by_name(element, "if").is_some() {
        let (branches, _) = collect_vue3_dynamic_slot_branch_chain(visible, index, ast);
        return lower_vue3_dynamic_slot_branch_to_dom_mir(
            &branches, 0, ast, hir_parent, mir_parent, state,
        );
    }
    if is_else_branch(element) {
        return None;
    }
    if let Some(for_dir) = directive_by_name(element, "for") {
        return Some(vuec_ast::Vue3DomDynamicSlot::For(
            lower_vue3_for_slot_to_dom_mir(
                ast_id, ast_node, slot, for_dir, ast, hir_parent, mir_parent, state,
            )?,
        ));
    }
    let slot_object = lower_vue3_dynamic_slot_object_to_dom_mir(
        ast_id,
        ast_node,
        slot,
        &ast_node.children,
        ast,
        hir_parent,
        mir_parent,
        state,
        if directive_by_name(element, "if").is_some()
            || directive_by_name(element, "else-if").is_some()
        {
            Some(index.to_string())
        } else {
            None
        },
    );
    Some(vuec_ast::Vue3DomDynamicSlot::Slot(slot_object))
}

pub(crate) fn collect_vue3_dynamic_slot_branch_chain(
    visible: &[NodeId],
    start: usize,
    ast: &Vue3Ast,
) -> (Vec<(NodeId, usize)>, usize) {
    let mut branches = vec![(visible[start], start)];
    let mut index = start + 1;
    while index < visible.len() {
        let Some(node) = ast.node(visible[index]) else {
            index += 1;
            continue;
        };
        let Vue3AstKind::Element(element) = &node.kind else {
            break;
        };
        if is_else_branch(element) && directive_by_name(element, "slot").is_some() {
            branches.push((visible[index], index));
            index += 1;
        } else {
            break;
        }
    }
    (branches, index)
}

pub(crate) fn lower_vue3_dynamic_slot_branch_to_dom_mir(
    branches: &[(NodeId, usize)],
    index: usize,
    ast: &Vue3Ast,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue3DomLoweringState,
) -> Option<vuec_ast::Vue3DomDynamicSlot> {
    let (ast_id, key_index) = branches.get(index).copied()?;
    let ast_node = ast.node(ast_id)?;
    let Vue3AstKind::Element(element) = &ast_node.kind else {
        return None;
    };
    let slot = directive_by_name(element, "slot")?;
    let slot_object = lower_vue3_dynamic_slot_object_to_dom_mir(
        ast_id,
        ast_node,
        slot,
        &ast_node.children,
        ast,
        hir_parent,
        mir_parent,
        state,
        Some(key_index.to_string()),
    );
    let condition = if index == 0 {
        directive_by_name(element, "if")
    } else {
        directive_by_name(element, "else-if")
    };
    if let Some(condition) = condition {
        let condition =
            lower_vue3_optional_condition(condition, ast_node, &mut state.js, state.source_type);
        let alternate = lower_vue3_dynamic_slot_branch_to_dom_mir(
            branches,
            index + 1,
            ast,
            hir_parent,
            mir_parent,
            state,
        )
        .map(Box::new);
        Some(vuec_ast::Vue3DomDynamicSlot::Conditional(
            vuec_ast::Vue3DomConditionalSlot {
                condition,
                slot: slot_object,
                alternate,
            },
        ))
    } else {
        Some(vuec_ast::Vue3DomDynamicSlot::Slot(slot_object))
    }
}

pub(crate) fn lower_vue3_for_slot_to_dom_mir(
    ast_id: NodeId,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    slot: &Vue3Directive,
    for_dir: &Vue3Directive,
    ast: &Vue3Ast,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue3DomLoweringState,
) -> Option<vuec_ast::Vue3DomForSlot> {
    let parsed = lower_vue3_for_directive(for_dir, ast_node, &mut state.js, state.source_type)?;
    Some(vuec_ast::Vue3DomForSlot {
        source: parsed.source,
        value_alias: parsed.value_alias,
        key_alias: parsed.key_alias,
        index_alias: parsed.index_alias,
        slot: lower_vue3_dynamic_slot_object_to_dom_mir(
            ast_id,
            ast_node,
            slot,
            &ast_node.children,
            ast,
            hir_parent,
            mir_parent,
            state,
            None,
        ),
    })
}

pub(crate) fn lower_vue3_dynamic_slot_object_to_dom_mir(
    ast_id: NodeId,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    slot: &Vue3Directive,
    children: &[NodeId],
    ast: &Vue3Ast,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue3DomLoweringState,
    key: Option<String>,
) -> vuec_ast::Vue3DomDynamicSlotObject {
    let name = lower_vue3_slot_name_to_dom_mir(slot, ast_node, state);
    let params = register_vue3_slot_params(slot, ast_node.span.source(), state);
    let slot_hir_id = lower_vue3_slot_decl_to_hir(
        ast_id,
        hir_parent,
        mir_parent,
        vue3_slot_name_text(&name),
        params,
        slot.span
            .map(NodeSpan::from)
            .unwrap_or_else(|| ast_node.span.clone()),
        state,
    );
    vuec_ast::Vue3DomDynamicSlotObject {
        name,
        params,
        children: lower_vue3_slot_children_to_dom_mir(
            children,
            ast,
            slot_hir_id,
            mir_parent,
            state,
        ),
        key,
    }
}

pub(crate) fn lower_vue3_slot_name_to_dom_mir(
    slot: &Vue3Directive,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    state: &mut Vue3DomLoweringState,
) -> Vue3DomSlotName {
    let Some(arg) = slot.arg.as_ref() else {
        return Vue3DomSlotName::Static("default".into());
    };
    if slot.is_dynamic_arg {
        Vue3DomSlotName::Dynamic(register_vue3_expression_with_span(
            &mut state.js,
            arg,
            slot.arg_span.or_else(|| ast_node.span.source()),
            state.source_type,
        ))
    } else {
        Vue3DomSlotName::Static(arg.source_string())
    }
}

pub(crate) fn vue3_slot_name_text(name: &Vue3DomSlotName) -> String {
    match name {
        Vue3DomSlotName::Static(name) => name.clone(),
        Vue3DomSlotName::Dynamic(id) => format!("#expr{}", id.0),
    }
}

pub(crate) fn lower_vue3_slot_decl_to_hir(
    ast_id: NodeId,
    hir_parent: NodeId,
    mir_id: NodeId,
    name: String,
    params: Option<JsPatternId>,
    span: NodeSpan,
    state: &mut Vue3DomLoweringState,
) -> NodeId {
    let hir_id = state.hir.push_child(
        hir_parent,
        HirNodeKind::SlotDecl(HirSlotDecl { name, params }),
        span,
    );
    state.map.record_ast_to_hir(ast_id, hir_id);
    state.map.record_hir_to_mir(hir_id, mir_id);
    hir_id
}

pub(crate) fn vue3_static_slot_directive_name(slot: &Vue3Directive) -> String {
    slot.arg
        .as_ref()
        .map(Vue3Expression::source_string)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "default".into())
}

pub(crate) fn register_vue3_slot_params(
    slot: &Vue3Directive,
    fallback_span: Option<Span>,
    state: &mut Vue3DomLoweringState,
) -> Option<JsPatternId> {
    slot.exp.as_ref().map(|exp| {
        register_vue3_pattern_with_span(
            &mut state.js,
            exp,
            slot.exp_span.or(fallback_span),
            state.source_type,
        )
    })
}

pub(crate) fn lower_vue3_slot_children_to_dom_mir(
    children: &[NodeId],
    ast: &Vue3Ast,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue3DomLoweringState,
) -> Vec<NodeId> {
    let before = state.mir.nodes.len();
    lower_vue3_dom_child_sequence(children, ast, hir_parent, mir_parent, state);
    state
        .mir
        .nodes
        .iter()
        .skip(before)
        .filter(|node| node.parent == Some(mir_parent))
        .map(|node| node.id)
        .collect()
}
