pub(crate) fn lower_vue3_component_slots_to_ssr_mir(
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    ast: &Vue3Ast,
    hir_id: NodeId,
    mir_id: NodeId,
    state: &mut Vue3SsrLoweringState,
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
                    slots.push(lower_vue3_static_slot_to_ssr_mir(
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
                    lower_vue3_dynamic_slot_to_ssr_mir(&visible, index, ast, hir_id, mir_id, state)
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
        slots.push(lower_vue3_default_slot_to_ssr_mir(
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

pub(crate) fn lower_vue3_dynamic_slot_to_ssr_mir(
    visible: &[NodeId],
    index: usize,
    ast: &Vue3Ast,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue3SsrLoweringState,
) -> Option<vuec_ast::Vue3DomDynamicSlot> {
    let ast_id = *visible.get(index)?;
    let ast_node = ast.node(ast_id)?;
    let Vue3AstKind::Element(element) = &ast_node.kind else {
        return None;
    };
    let slot = directive_by_name(element, "slot")?;
    if directive_by_name(element, "if").is_some() {
        let (branches, _) = collect_vue3_dynamic_slot_branch_chain(visible, index, ast);
        return lower_vue3_dynamic_slot_branch_to_ssr_mir(
            &branches, 0, ast, hir_parent, mir_parent, state,
        );
    }
    if is_else_branch(element) {
        return None;
    }
    if let Some(for_dir) = directive_by_name(element, "for") {
        return Some(vuec_ast::Vue3DomDynamicSlot::For(
            lower_vue3_for_slot_to_ssr_mir(
                ast_id, ast_node, slot, for_dir, ast, hir_parent, mir_parent, state,
            )?,
        ));
    }
    Some(vuec_ast::Vue3DomDynamicSlot::Slot(
        lower_vue3_dynamic_slot_object_to_ssr_mir(
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
    ))
}

pub(crate) fn lower_vue3_dynamic_slot_branch_to_ssr_mir(
    branches: &[(NodeId, usize)],
    index: usize,
    ast: &Vue3Ast,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue3SsrLoweringState,
) -> Option<vuec_ast::Vue3DomDynamicSlot> {
    let (ast_id, key_index) = branches.get(index).copied()?;
    let ast_node = ast.node(ast_id)?;
    let Vue3AstKind::Element(element) = &ast_node.kind else {
        return None;
    };
    let slot = directive_by_name(element, "slot")?;
    let slot_object = lower_vue3_dynamic_slot_object_to_ssr_mir(
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
        let alternate = lower_vue3_dynamic_slot_branch_to_ssr_mir(
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

pub(crate) fn lower_vue3_for_slot_to_ssr_mir(
    ast_id: NodeId,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    slot: &Vue3Directive,
    for_dir: &Vue3Directive,
    ast: &Vue3Ast,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue3SsrLoweringState,
) -> Option<vuec_ast::Vue3DomForSlot> {
    let parsed = lower_vue3_for_directive(for_dir, ast_node, &mut state.js, state.source_type)?;
    Some(vuec_ast::Vue3DomForSlot {
        source: parsed.source,
        value_alias: parsed.value_alias,
        key_alias: parsed.key_alias,
        index_alias: parsed.index_alias,
        slot: lower_vue3_dynamic_slot_object_to_ssr_mir(
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

pub(crate) fn lower_vue3_dynamic_slot_object_to_ssr_mir(
    ast_id: NodeId,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    slot: &Vue3Directive,
    children: &[NodeId],
    ast: &Vue3Ast,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue3SsrLoweringState,
    key: Option<String>,
) -> vuec_ast::Vue3DomDynamicSlotObject {
    let name = lower_vue3_slot_name_to_ssr_mir(slot, ast_node, state);
    let params = register_vue3_slot_params_ssr(slot, ast_node.span.source(), state);
    let slot_hir_id = lower_vue3_slot_decl_to_hir_ssr(
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
        children: lower_vue3_slot_children_to_ssr_mir(
            children,
            ast,
            slot_hir_id,
            mir_parent,
            state,
        ),
        key,
    }
}

pub(crate) fn lower_vue3_slot_name_to_ssr_mir(
    slot: &Vue3Directive,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    state: &mut Vue3SsrLoweringState,
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

pub(crate) fn lower_vue3_suspense_slots_to_ssr_mir(
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    ast: &Vue3Ast,
    hir_id: NodeId,
    mir_id: NodeId,
    state: &mut Vue3SsrLoweringState,
) -> Option<vuec_ast::Vue3DomSlots> {
    let visible = visible_child_ids(ast, &ast_node.children);
    if visible.is_empty() {
        return None;
    }
    let mut slots = Vec::new();
    let mut default_children = Vec::new();
    for child_id in visible {
        let Some(child) = ast.node(child_id) else {
            continue;
        };
        if let Vue3AstKind::Element(child_element) = &child.kind {
            if let Some(slot) = directive_by_name(child_element, "slot") {
                if slot.is_dynamic_arg
                    || directive_by_name(child_element, "if").is_some()
                    || directive_by_name(child_element, "else").is_some()
                    || directive_by_name(child_element, "else-if").is_some()
                    || directive_by_name(child_element, "for").is_some()
                {
                    continue;
                }
                slots.push(lower_vue3_static_slot_to_ssr_mir(
                    child_id,
                    child,
                    slot,
                    &child.children,
                    ast,
                    hir_id,
                    mir_id,
                    state,
                ));
                continue;
            }
        }
        default_children.push(child_id);
    }
    if !default_children.is_empty() {
        slots.push(lower_vue3_default_slot_to_ssr_mir(
            ast_node,
            None,
            &default_children,
            ast,
            hir_id,
            mir_id,
            state,
        ));
    }
    if slots.is_empty() {
        None
    } else {
        Some(vuec_ast::Vue3DomSlots {
            slots,
            dynamic_slots: Vec::new(),
            flag: Vue3SlotFlag::Stable,
        })
    }
}

pub(crate) fn lower_vue3_static_slot_to_ssr_mir(
    ast_id: NodeId,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    slot: &Vue3Directive,
    children: &[NodeId],
    ast: &Vue3Ast,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue3SsrLoweringState,
) -> vuec_ast::Vue3DomSlot {
    let name = vue3_static_slot_directive_name(slot);
    let params = register_vue3_slot_params_ssr(slot, ast_node.span.source(), state);
    let slot_hir_id = lower_vue3_slot_decl_to_hir_ssr(
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
        children: lower_vue3_slot_children_to_ssr_mir(
            children,
            ast,
            slot_hir_id,
            mir_parent,
            state,
        ),
    }
}

pub(crate) fn lower_vue3_default_slot_to_ssr_mir(
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    on_component_slot: Option<&Vue3Directive>,
    children: &[NodeId],
    ast: &Vue3Ast,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue3SsrLoweringState,
) -> vuec_ast::Vue3DomSlot {
    let slot_name = on_component_slot
        .map(vue3_static_slot_directive_name)
        .unwrap_or_else(|| "default".into());
    let slot_params = on_component_slot
        .and_then(|slot| register_vue3_slot_params_ssr(slot, ast_node.span.source(), state));
    let slot_hir_id = lower_vue3_slot_decl_to_hir_ssr(
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
        children: lower_vue3_slot_children_to_ssr_mir(
            children,
            ast,
            slot_hir_id,
            mir_parent,
            state,
        ),
    }
}

pub(crate) fn lower_vue3_slot_decl_to_hir_ssr(
    ast_id: NodeId,
    hir_parent: NodeId,
    mir_id: NodeId,
    name: String,
    params: Option<JsPatternId>,
    span: NodeSpan,
    state: &mut Vue3SsrLoweringState,
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

pub(crate) fn register_vue3_slot_params_ssr(
    slot: &Vue3Directive,
    fallback_span: Option<Span>,
    state: &mut Vue3SsrLoweringState,
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

pub(crate) fn lower_vue3_slot_children_to_ssr_mir(
    children: &[NodeId],
    ast: &Vue3Ast,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue3SsrLoweringState,
) -> Vec<NodeId> {
    let before = state.mir.nodes.len();
    lower_vue3_ssr_child_sequence(children, ast, hir_parent, mir_parent, state);
    state
        .mir
        .nodes
        .iter()
        .skip(before)
        .filter(|node| node.parent == Some(mir_parent))
        .map(|node| node.id)
        .collect::<Vec<_>>()
}

pub(crate) fn vue3_ssr_empty_slots(flag: Vue3SlotFlag) -> vuec_ast::Vue3DomSlots {
    vuec_ast::Vue3DomSlots {
        slots: Vec::new(),
        dynamic_slots: Vec::new(),
        flag,
    }
}

pub(crate) fn visible_child_ids(ast: &Vue3Ast, children: &[NodeId]) -> Vec<NodeId> {
    children
        .iter()
        .copied()
        .filter(|child_id| {
            ast.node(*child_id).is_some_and(|child| match &child.kind {
                Vue3AstKind::Comment(_) => false,
                Vue3AstKind::Text(text) => !text.value.trim().is_empty(),
                _ => true,
            })
        })
        .collect()
}

pub(crate) fn root_needs_fragment_block(ast: &Vue3Ast) -> bool {
    let Some(root) = ast.root_node() else {
        return false;
    };
    let visible = visible_child_ids(ast, &root.children);
    match visible.as_slice() {
        [] => false,
        [single] => {
            root.children.as_slice() != [*single]
                || !root_single_visible_child_uses_direct_codegen(ast, *single)
        }
        _ => true,
    }
}

pub(crate) fn component_children_are_stable_slots(ast: &Vue3Ast, children: &[NodeId]) -> bool {
    children.iter().all(|child_id| {
        let Some(child) = ast.node(*child_id) else {
            return false;
        };
        let Vue3AstKind::Element(element) = &child.kind else {
            return true;
        };
        let Some(slot) = directive_by_name(element, "slot") else {
            return true;
        };
        !slot.is_dynamic_arg
            && directive_by_name(element, "if").is_none()
            && directive_by_name(element, "else").is_none()
            && directive_by_name(element, "else-if").is_none()
            && directive_by_name(element, "for").is_none()
    })
}

pub(crate) fn vue3_dom_stable_slot_flag(ast: &Vue3Ast, children: &[NodeId]) -> Vue3SlotFlag {
    if vue3_dom_slot_children_forward_slots(ast, children) {
        Vue3SlotFlag::Forwarded
    } else {
        Vue3SlotFlag::Stable
    }
}

pub(crate) fn vue3_dom_slot_children_forward_slots(ast: &Vue3Ast, children: &[NodeId]) -> bool {
    children.iter().any(|child_id| {
        let Some(child) = ast.node(*child_id) else {
            return false;
        };
        match &child.kind {
            Vue3AstKind::Element(element) => {
                element.tag_type == Vue3ElementType::SlotOutlet
                    || vue3_dom_slot_children_forward_slots(ast, &child.children)
            }
            _ => false,
        }
    })
}
