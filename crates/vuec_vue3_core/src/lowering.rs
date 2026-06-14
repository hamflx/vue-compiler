use crate::*;

pub(crate) fn inline_preamble_helpers(helpers: &mut Vec<RuntimeHelper>, expr: &str) {
    if expr.contains("_vModel") {
        let preferred = if expr.contains("_Fragment") {
            &[
                RuntimeHelper::Vue3VModelRadio,
                RuntimeHelper::Vue3VModelCheckbox,
                RuntimeHelper::Vue3VModelText,
                RuntimeHelper::Vue3VModelSelect,
                RuntimeHelper::Vue3VModelDynamic,
                RuntimeHelper::Vue3CreateElementVNode,
                RuntimeHelper::Vue3WithDirectives,
                RuntimeHelper::Vue3Unref,
                RuntimeHelper::Vue3IsRef,
                RuntimeHelper::Vue3Fragment,
                RuntimeHelper::Vue3OpenBlock,
                RuntimeHelper::Vue3CreateElementBlock,
            ][..]
        } else {
            &[
                RuntimeHelper::Vue3Unref,
                RuntimeHelper::Vue3IsRef,
                RuntimeHelper::Vue3VModelRadio,
                RuntimeHelper::Vue3VModelCheckbox,
                RuntimeHelper::Vue3VModelText,
                RuntimeHelper::Vue3VModelSelect,
                RuntimeHelper::Vue3VModelDynamic,
                RuntimeHelper::Vue3WithDirectives,
                RuntimeHelper::Vue3OpenBlock,
                RuntimeHelper::Vue3CreateElementBlock,
            ][..]
        };
        reorder_helpers_by_preference(helpers, preferred);
        return;
    }

    if expr.contains("_createElementVNode") {
        let mut preferred = Vec::new();
        if helpers.contains(&RuntimeHelper::Vue3Unref) {
            preferred.push(RuntimeHelper::Vue3Unref);
        }
        if expr.contains("_toDisplayString") {
            preferred.push(RuntimeHelper::Vue3ToDisplayString);
            if expr.contains("_createTextVNode") {
                preferred.push(RuntimeHelper::Vue3CreateTextVNode);
            }
        }
        if expr.contains("_withCtx") {
            preferred.push(RuntimeHelper::Vue3WithCtx);
            preferred.push(RuntimeHelper::Vue3CreateVNode);
            preferred.push(RuntimeHelper::Vue3CreateElementVNode);
        } else {
            if !expr.contains("_toDisplayString") {
                preferred.clear();
            }
            if helpers.contains(&RuntimeHelper::Vue3Unref)
                && !helpers.contains(&RuntimeHelper::Vue3IsRef)
                && expr.contains("_withDirectives")
            {
                preferred.push(RuntimeHelper::Vue3Unref);
            }
            preferred.push(RuntimeHelper::Vue3CreateElementVNode);
            preferred.push(RuntimeHelper::Vue3IsRef);
            if helpers.contains(&RuntimeHelper::Vue3Unref)
                && !preferred.contains(&RuntimeHelper::Vue3Unref)
            {
                preferred.push(RuntimeHelper::Vue3Unref);
            }
            preferred.push(RuntimeHelper::Vue3WithDirectives);
            preferred.push(RuntimeHelper::Vue3CreateVNode);
        }
        preferred.push(RuntimeHelper::Vue3Fragment);
        preferred.push(RuntimeHelper::Vue3OpenBlock);
        preferred.push(RuntimeHelper::Vue3CreateElementBlock);
        reorder_helpers_by_preference(helpers, &preferred);
    } else {
        if expr.contains("\"onUpdate:")
            && helpers.contains(&RuntimeHelper::Vue3Unref)
            && helpers.contains(&RuntimeHelper::Vue3ResolveComponent)
            && helpers.contains(&RuntimeHelper::Vue3IsRef)
        {
            reorder_helpers_by_preference(
                helpers,
                &[
                    RuntimeHelper::Vue3Unref,
                    RuntimeHelper::Vue3ResolveComponent,
                    RuntimeHelper::Vue3IsRef,
                    RuntimeHelper::Vue3OpenBlock,
                    RuntimeHelper::Vue3CreateBlock,
                ],
            );
            return;
        }
        move_helper_before(
            helpers,
            RuntimeHelper::Vue3Unref,
            RuntimeHelper::Vue3OpenBlock,
        );
    }
}

/// Lower a Vue 3 AST document into the shared HIR and the Vue 3 DOM MIR target.
///
/// The lowering records explicit AST -> HIR and HIR -> MIR edges in
/// `LoweringMap`, and registers template expressions into `JsAstStore`.
/// Lowers a Vue 3 AST into shared HIR plus DOM target MIR.
pub fn lower_vue3_ast_to_dom_mir(
    ast: &Vue3Ast,
    options: &Vue3CompilerOptions,
) -> Vue3DomLoweringResult {
    let root_span = ast
        .root_node()
        .map(|node| node.span.clone())
        .unwrap_or_else(|| NodeSpan::missing(MissingSpanReason::LoweringGap));
    let mut state = Vue3DomLoweringState {
        hir: Hir::new(HirNodeKind::Root(HirRoot), root_span.clone()),
        mir: Vue3DomMir::new(
            Vue3DomMirKind::Root(Vue3DomRoot {
                imports: vue3_codegen_root(ast)
                    .map(|root| root.imports.clone())
                    .unwrap_or_default(),
            }),
            root_span,
        ),
        map: LoweringMap::default(),
        js: JsAstStore::new(),
        options: options.clone(),
        source_type: expression_source_type(options),
        do_not_hoist_root: ast
            .root_node()
            .and_then(|root| vue3_single_static_root_child(&root.children, ast)),
        next_hoist_index: 1,
        next_cache_index: 0,
        in_v_once: 0,
        in_static_hoist: 0,
    };
    state.map.record_ast_to_hir(ast.root, state.hir.root);
    state.map.record_hir_to_mir(state.hir.root, state.mir.root);

    if let Some(root) = ast.root_node() {
        lower_vue3_dom_child_sequence(
            &root.children,
            ast,
            state.hir.root,
            state.mir.root,
            &mut state,
        );
    }

    Vue3DomLoweringResult {
        hir: state.hir,
        mir: state.mir,
        map: state.map,
        js: state.js,
    }
}

/// Lower a Vue 3 AST document into the shared HIR and the Vue 3 SSR MIR target.
///
/// This is a structural contract entry for SSR. It records explicit AST -> HIR
/// and HIR -> MIR edges and keeps SSR output in `Vue3SsrMir` instead of
/// deriving it from DOM MIR.
/// Lowers a Vue 3 AST into shared HIR plus SSR target MIR.
pub fn lower_vue3_ast_to_ssr_mir(
    ast: &Vue3Ast,
    options: &Vue3CompilerOptions,
) -> Vue3SsrLoweringResult {
    let root_span = ast
        .root_node()
        .map(|node| node.span.clone())
        .unwrap_or_else(|| NodeSpan::missing(MissingSpanReason::LoweringGap));
    let mut state = Vue3SsrLoweringState {
        hir: Hir::new(HirNodeKind::Root(HirRoot), root_span.clone()),
        mir: Vue3SsrMir::new(
            Vue3SsrMirKind::Root(Vue3SsrRoot {
                imports: vue3_codegen_root(ast)
                    .map(|root| root.imports.clone())
                    .unwrap_or_default(),
            }),
            root_span,
        ),
        map: LoweringMap::default(),
        js: JsAstStore::new(),
        options: options.clone(),
        source_type: expression_source_type(options),
        select_model_stack: Vec::new(),
        flatten_ssr_fragments: 0,
    };
    state.map.record_ast_to_hir(ast.root, state.hir.root);
    state.map.record_hir_to_mir(state.hir.root, state.mir.root);

    if let Some(root) = ast.root_node() {
        lower_vue3_ssr_child_sequence(
            &root.children,
            ast,
            state.hir.root,
            state.mir.root,
            &mut state,
        );
    }

    Vue3SsrLoweringResult {
        hir: state.hir,
        mir: state.mir,
        map: state.map,
        js: state.js,
    }
}

/// Projects a public AST root codegen node into bridge JSON.
pub fn root_codegen_projection(root: &Value) -> Value {
    let children = root
        .get("children")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    match children {
        [] => json!({ "kind": "none" }),
        [_] => root_single_child_codegen_projection(children),
        _ => json!({
            "kind": "fragment",
            "patchFlag": root_fragment_patch_flag(children),
        }),
    }
}

pub(crate) struct Vue3DomLoweringState {
    pub(crate) hir: Hir,
    pub(crate) mir: Vue3DomMir,
    pub(crate) map: LoweringMap,
    pub(crate) js: JsAstStore,
    pub(crate) options: Vue3CompilerOptions,
    pub(crate) source_type: oxc_span::SourceType,
    pub(crate) do_not_hoist_root: Option<NodeId>,
    pub(crate) next_hoist_index: u32,
    pub(crate) next_cache_index: u32,
    pub(crate) in_v_once: u32,
    pub(crate) in_static_hoist: u32,
}

pub(crate) struct Vue3SsrLoweringState {
    pub(crate) hir: Hir,
    pub(crate) mir: Vue3SsrMir,
    pub(crate) map: LoweringMap,
    pub(crate) js: JsAstStore,
    pub(crate) options: Vue3CompilerOptions,
    pub(crate) source_type: oxc_span::SourceType,
    pub(crate) select_model_stack: Vec<JsExprId>,
    pub(crate) flatten_ssr_fragments: u32,
}

pub(crate) fn lower_vue3_dom_child_sequence(
    children: &[NodeId],
    ast: &Vue3Ast,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue3DomLoweringState,
) {
    let mut index = 0usize;
    while index < children.len() {
        let child_id = children[index];
        let Some(child) = ast.node(child_id) else {
            index += 1;
            continue;
        };
        if let Vue3AstKind::Element(element) = &child.kind {
            if directive_by_name(element, "for").is_some() {
                lower_vue3_ast_node_to_dom_mir(child_id, ast, hir_parent, mir_parent, state);
                index += 1;
                continue;
            }
            if directive_by_name(element, "if").is_some() {
                let (branch_ids, next_index) = collect_vue3_if_branch_chain(children, index, ast);
                lower_vue3_if_branch_chain_to_dom_mir(
                    &branch_ids,
                    ast,
                    hir_parent,
                    mir_parent,
                    state,
                );
                index = next_index;
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

    if directive_by_name(element, "once").is_some() && state.in_v_once == 0 {
        let cache_id = state.next_cache_index;
        state.next_cache_index += 1;
        let wrapper_id = state.mir.push_child(
            mir_parent,
            Vue3DomMirKind::Cache { index: cache_id },
            NodeSpan::generated(ast_node.span.source(), vuec_ast::GeneratedReason::Lowering),
        );
        state.in_v_once += 1;
        let lowered = lower_vue3_plain_element_to_dom_mir(
            ast_id, element, ast, ast_node, hir_parent, wrapper_id, state,
        );
        state.in_v_once -= 1;
        if let Some((hir_id, _)) = lowered {
            state.map.record_hir_to_mir(hir_id, wrapper_id);
            return Some((hir_id, wrapper_id));
        }
        return None;
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

pub(crate) fn lower_vue3_element_to_dom_mir_kind(
    element: &Vue3Element,
    ast: &Vue3Ast,
    ast_id: NodeId,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    hir_kind: &HirNodeKind,
    state: &mut Vue3DomLoweringState,
) -> Vue3DomMirKind {
    if element.tag_type == Vue3ElementType::SlotOutlet {
        let HirNodeKind::SlotOutlet(slot) = hir_kind else {
            return Vue3DomMirKind::Fragment;
        };
        return Vue3DomMirKind::RenderSlot(vuec_ast::Vue3RenderSlot {
            name: lower_hir_slot_outlet_name_to_dom_mir(slot),
            props: lower_vue3_slot_outlet_props_to_dom_mir(&slot.props, state),
            fallback: Vec::new(),
        });
    }

    let is_component = element.tag_type == Vue3ElementType::Component;
    let (mut props, directives, v_show) = lower_vue3_hir_payload_to_dom_mir(hir_kind, state);
    let content = inject_vue3_dom_content_props(&mut props, element, ast_node, state);
    let models = inject_vue3_dom_model_props(&mut props, element, ast_node, state);
    inject_vue3_transition_persisted_prop(&mut props, element, ast, ast_node);
    let tag = lower_vue3_element_tag_to_dom_mir(element, &props);
    Vue3DomMirKind::VNodeCall(Vue3VNodeCall {
        tag,
        props,
        v_show,
        directives,
        models,
        content,
        children: if ast_node.children.is_empty() {
            MirChildren::None
        } else {
            MirChildren::Nodes(Vec::new())
        },
        patch_flag: Vue3PatchFlags {
            bits: vue3_dom_mir_patch_flag(ast, ast_id, element, &state.options),
        },
        dynamic_props: vue3_dom_mir_dynamic_props(element),
        is_block: false,
        disable_tracking: false,
        is_component,
    })
}

pub(crate) fn inject_vue3_dom_content_props(
    props: &mut Vue3DomProps,
    element: &Vue3Element,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    state: &mut Vue3DomLoweringState,
) -> Option<Vue3DomContent> {
    let mut segment_index = 0usize;
    let mut content = None;
    let mut segments = Vec::new();
    for prop in &element.props {
        match prop {
            Vue3Prop::Attribute(_) => {
                push_existing_vue3_dom_prop_segment(props, &mut segments, &mut segment_index);
            }
            Vue3Prop::Directive(dir) if dir.name == "html" || dir.name == "text" => {
                let lowered = if dir.name == "html" {
                    Vue3DomContent::Html {
                        expression: lower_vue3_dom_content_expression(dir, ast_node, state),
                    }
                } else {
                    Vue3DomContent::Text {
                        expression: lower_vue3_dom_content_expression(dir, ast_node, state),
                    }
                };
                if content.is_none() {
                    content = Some(lowered.clone());
                }
                segments.push(Vue3DomPropSegment::Content(lowered));
            }
            Vue3Prop::Directive(dir) if dir.name == "bind" || dir.name == "on" => {
                push_existing_vue3_dom_prop_segment(props, &mut segments, &mut segment_index);
            }
            Vue3Prop::Directive(_) => {}
        }
    }
    while segment_index < props.segments.len() {
        push_existing_vue3_dom_prop_segment(props, &mut segments, &mut segment_index);
    }
    if content.is_some() {
        props.segments = segments;
    }
    content
}

pub(crate) fn inject_vue3_dom_model_props(
    props: &mut Vue3DomProps,
    element: &Vue3Element,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    state: &mut Vue3DomLoweringState,
) -> Vec<Vue3DomModel> {
    let mut segment_index = 0usize;
    let mut models = Vec::new();
    let mut segments = Vec::new();
    for prop in &element.props {
        match prop {
            Vue3Prop::Attribute(_) => {
                push_existing_vue3_dom_prop_segment(props, &mut segments, &mut segment_index);
            }
            Vue3Prop::Directive(dir) if dir.name == "model" => {
                if let Some(model) = lower_vue3_dom_model(element, dir, ast_node, state) {
                    models.push(model.clone());
                    segments.push(Vue3DomPropSegment::Model(model));
                }
            }
            Vue3Prop::Directive(dir) if dir.name == "bind" || dir.name == "on" => {
                push_existing_vue3_dom_prop_segment(props, &mut segments, &mut segment_index);
            }
            Vue3Prop::Directive(_) => {}
        }
    }
    while segment_index < props.segments.len() {
        push_existing_vue3_dom_prop_segment(props, &mut segments, &mut segment_index);
    }
    if !models.is_empty() {
        props.segments = segments;
    }
    models
}

pub(crate) fn lower_vue3_dom_model(
    element: &Vue3Element,
    directive: &Vue3Directive,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    state: &mut Vue3DomLoweringState,
) -> Option<Vue3DomModel> {
    if element.tag_type == Vue3ElementType::Component || directive.arg.is_some() {
        return None;
    }
    if !matches!(element.tag.as_str(), "input" | "textarea" | "select")
        && !state
            .options
            .custom_elements
            .iter()
            .any(|candidate| candidate == &element.tag)
    {
        return None;
    }
    let expression = directive.exp.as_ref()?;
    if expression.source_string().trim().is_empty() {
        return None;
    }
    let kind = vue3_dom_model_kind(element)?;
    Some(Vue3DomModel {
        expression: register_or_reuse_vue3_expression_with_span(
            &mut state.js,
            expression,
            directive.exp_span.or_else(|| ast_node.span.source()),
            state.source_type,
        ),
        kind,
        modifiers: directive.modifiers.clone(),
    })
}

pub(crate) fn vue3_dom_model_kind(element: &Vue3Element) -> Option<Vue3DomModelKind> {
    match element.tag.as_str() {
        "select" => Some(Vue3DomModelKind::Select),
        "textarea" => Some(Vue3DomModelKind::Text),
        "input" => vue3_dom_input_model_kind(element),
        _ if element.tag_type == Vue3ElementType::Element => Some(Vue3DomModelKind::Text),
        _ => None,
    }
}

pub(crate) fn vue3_dom_input_model_kind(element: &Vue3Element) -> Option<Vue3DomModelKind> {
    if vue3_dom_has_dynamic_key_v_bind(element) {
        return Some(Vue3DomModelKind::Dynamic);
    }
    match vue3_dom_input_type(element) {
        Some(Vue3DomInputType::Dynamic) => Some(Vue3DomModelKind::Dynamic),
        Some(Vue3DomInputType::Static(value)) => match value.as_str() {
            "radio" => Some(Vue3DomModelKind::Radio),
            "checkbox" => Some(Vue3DomModelKind::Checkbox),
            "file" => None,
            _ => Some(Vue3DomModelKind::Text),
        },
        None => Some(Vue3DomModelKind::Text),
    }
}

pub(crate) enum Vue3DomInputType {
    Static(String),
    Dynamic,
}

pub(crate) fn vue3_dom_input_type(element: &Vue3Element) -> Option<Vue3DomInputType> {
    element.props.iter().find_map(|prop| match prop {
        Vue3Prop::Attribute(attr) if attr.name == "type" => Some(Vue3DomInputType::Static(
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
            Some(Vue3DomInputType::Dynamic)
        }
        _ => None,
    })
}

pub(crate) fn vue3_dom_has_dynamic_key_v_bind(element: &Vue3Element) -> bool {
    element.props.iter().any(|prop| {
        matches!(
            prop,
            Vue3Prop::Directive(dir)
                if dir.name == "bind" && dir.exp.is_some() && (dir.arg.is_none() || dir.is_dynamic_arg)
        )
    })
}

pub(crate) fn push_existing_vue3_dom_prop_segment(
    props: &Vue3DomProps,
    segments: &mut Vec<Vue3DomPropSegment>,
    index: &mut usize,
) {
    if let Some(segment) = props.segments.get(*index).cloned() {
        segments.push(segment);
        *index += 1;
    }
}

pub(crate) fn lower_vue3_dom_content_expression(
    directive: &Vue3Directive,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    state: &mut Vue3DomLoweringState,
) -> Option<JsExprId> {
    directive.exp.as_ref().and_then(|expression| {
        (!expression.source_string().trim().is_empty()).then(|| {
            register_or_reuse_vue3_expression_with_span(
                &mut state.js,
                expression,
                directive.exp_span.or_else(|| ast_node.span.source()),
                state.source_type,
            )
        })
    })
}

pub(crate) fn lower_hir_slot_outlet_name_to_dom_mir(slot: &HirSlotOutlet) -> Vue3DomSlotName {
    if let Some(binding) = slot
        .props
        .dynamic_bindings
        .iter()
        .find(|binding| !binding.dynamic_arg && binding.name == "name")
    {
        Vue3DomSlotName::Dynamic(binding.value)
    } else {
        Vue3DomSlotName::Static(slot.name.clone().unwrap_or_else(|| "default".into()))
    }
}

pub(crate) fn lower_vue3_slot_outlet_props_to_dom_mir(
    props: &HirProps,
    state: &mut Vue3DomLoweringState,
) -> Vue3DomProps {
    let filtered = filter_vue3_slot_outlet_name_props(props);
    lower_hir_props_to_dom_mir(&filtered, false, state)
}

pub(crate) fn filter_vue3_slot_outlet_name_props(props: &HirProps) -> HirProps {
    let mut filtered = props.clone();
    filtered.static_attrs.retain(|attr| attr.name != "name");
    filtered
        .dynamic_bindings
        .retain(|binding| binding.dynamic_arg || binding.name != "name");
    filtered.segments.retain(|segment| match segment {
        HirPropSegment::StaticAttr(attr) => attr.name != "name",
        HirPropSegment::DynamicBinding(binding) => binding.dynamic_arg || binding.name != "name",
        HirPropSegment::Event(_)
        | HirPropSegment::ObjectBinding(_)
        | HirPropSegment::ObjectListeners(_) => true,
    });
    filtered
}

pub(crate) fn lower_vue3_element_tag_to_dom_mir(
    element: &Vue3Element,
    props: &Vue3DomProps,
) -> Vue3DomTag {
    if element.tag_type != Vue3ElementType::Component {
        return Vue3DomTag::Native(element.tag.clone());
    }
    if let Some(expression) = props
        .dynamic_bindings
        .iter()
        .find(|binding| !binding.dynamic_arg && binding.name == "is")
        .map(|binding| binding.value)
    {
        return Vue3DomTag::DynamicComponent(expression);
    }
    if let Some(helper) = vue3_core_component_runtime_helper(&element.tag) {
        return Vue3DomTag::RuntimeHelper(helper);
    }
    Vue3DomTag::ComponentAsset(element.tag.clone())
}

pub(crate) fn inject_vue3_transition_persisted_prop(
    props: &mut Vue3DomProps,
    element: &Vue3Element,
    ast: &Vue3Ast,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
) {
    if element.tag_type != Vue3ElementType::Component
        || vue3_core_component_runtime_helper(&element.tag) != Some(RuntimeHelper::Vue3Transition)
        || !vue3_transition_single_child_has_v_show(ast, &ast_node.children)
        || vue3_dom_final_prop_group_has_static_key(props, "persisted")
    {
        return;
    }
    let attr = Vue3DomStaticAttr {
        name: "persisted".into(),
        value: String::new(),
    };
    props.static_attrs.push(attr.clone());
    if !props.segments.is_empty() {
        props.segments.push(Vue3DomPropSegment::StaticAttr(attr));
    }
}

pub(crate) fn vue3_dom_final_prop_group_has_static_key(props: &Vue3DomProps, name: &str) -> bool {
    if props.segments.is_empty() {
        return props.static_attrs.iter().any(|attr| attr.name == name)
            || props
                .dynamic_bindings
                .iter()
                .any(|binding| !binding.dynamic_arg && binding.name == name);
    }
    for segment in props.segments.iter().rev() {
        match segment {
            Vue3DomPropSegment::StaticAttr(attr) if attr.name == name => return true,
            Vue3DomPropSegment::DynamicBinding(binding)
                if !binding.dynamic_arg && binding.name == name =>
            {
                return true;
            }
            Vue3DomPropSegment::ObjectBinding(_) | Vue3DomPropSegment::ObjectListeners(_) => {
                return false;
            }
            Vue3DomPropSegment::StaticAttr(_)
            | Vue3DomPropSegment::DynamicBinding(_)
            | Vue3DomPropSegment::Content(_)
            | Vue3DomPropSegment::Model(_)
            | Vue3DomPropSegment::Event(_) => {}
        }
    }
    false
}

pub(crate) fn vue3_transition_single_child_has_v_show(ast: &Vue3Ast, children: &[NodeId]) -> bool {
    let visible = vue3_transition_visible_child_ids(ast, children);
    let [child_id] = visible.as_slice() else {
        return false;
    };
    let Some(child) = ast.node(*child_id) else {
        return false;
    };
    let Vue3AstKind::Element(element) = &child.kind else {
        return false;
    };
    directive_by_name(element, "show").is_some()
        && directive_by_name(element, "if").is_none()
        && directive_by_name(element, "else").is_none()
        && directive_by_name(element, "else-if").is_none()
        && directive_by_name(element, "for").is_none()
}

pub(crate) fn vue3_transition_visible_child_ids(ast: &Vue3Ast, children: &[NodeId]) -> Vec<NodeId> {
    children
        .iter()
        .copied()
        .filter(|child_id| {
            ast.node(*child_id).is_some_and(|child| match &child.kind {
                Vue3AstKind::Comment(_) => false,
                Vue3AstKind::Text(text) => !text.value.chars().all(is_vue3_html_whitespace),
                _ => true,
            })
        })
        .collect()
}

pub(crate) fn vue3_dom_mir_patch_flag(
    ast: &Vue3Ast,
    ast_id: NodeId,
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
) -> i32 {
    let children = ast
        .node(ast_id)
        .map(|node| node.children.as_slice())
        .unwrap_or(&[]);
    let mut bits = 0;
    if has_dynamic_arg_binding(element) {
        bits |= 16;
    } else {
        if has_class_binding(element) && element.tag_type != Vue3ElementType::Component {
            bits |= 2;
        }
        if has_style_binding(element) && element.tag_type != Vue3ElementType::Component {
            bits |= 4;
        }
        if !vue3_dom_mir_props_patch_names(element).is_empty() {
            bits |= 8;
        }
        if has_hydration_event_binding(element) || has_prop_bind_modifier(element) {
            bits |= 32;
        }
    }
    if element.tag != "template" && child_sequence_is_direct_dynamic_text(ast, children, options) {
        bits |= 1;
    }
    if (bits == 0 || bits == 32)
        && (has_vnode_hook(element)
            || has_runtime_directive(element)
            || has_native_v_model(element))
    {
        bits |= 512;
    }
    if element.tag_type == Vue3ElementType::Component && component_has_dynamic_slots(ast, children)
    {
        bits |= 1024;
    }
    bits
}

pub(crate) fn component_has_dynamic_slots(ast: &Vue3Ast, children: &[NodeId]) -> bool {
    visible_child_ids(ast, children).iter().any(|child_id| {
        let Some(child) = ast.node(*child_id) else {
            return false;
        };
        let Vue3AstKind::Element(element) = &child.kind else {
            return false;
        };
        directive_by_name(element, "slot").is_some_and(|slot| slot.is_dynamic_arg)
            || directive_by_name(element, "slot").is_some()
                && (directive_by_name(element, "if").is_some()
                    || directive_by_name(element, "else").is_some()
                    || directive_by_name(element, "else-if").is_some()
                    || directive_by_name(element, "for").is_some())
    })
}

pub(crate) fn lower_vue3_hir_payload_to_dom_mir(
    hir_kind: &HirNodeKind,
    state: &mut Vue3DomLoweringState,
) -> (Vue3DomProps, Vec<Vue3DomDirective>, Option<JsExprId>) {
    match hir_kind {
        HirNodeKind::Element(element) => {
            let mut v_show = None;
            let directives = element
                .directives
                .iter()
                .filter_map(|directive| {
                    if directive.name == "show" {
                        v_show = directive.expression;
                        None
                    } else if vue3_directive_needs_runtime_asset(&directive.name) {
                        Some(lower_hir_directive_to_dom_mir(directive))
                    } else {
                        None
                    }
                })
                .collect();
            (
                lower_hir_props_to_dom_mir(&element.props, false, state),
                directives,
                v_show,
            )
        }
        HirNodeKind::Component(component) => (
            lower_hir_props_to_dom_mir(&component.props, true, state),
            Vec::new(),
            None,
        ),
        HirNodeKind::SlotOutlet(slot) => (
            lower_hir_props_to_dom_mir(&slot.props, false, state),
            Vec::new(),
            None,
        ),
        _ => (Vue3DomProps::default(), Vec::new(), None),
    }
}

pub(crate) fn lower_hir_props_to_dom_mir(
    props: &HirProps,
    is_component: bool,
    state: &mut Vue3DomLoweringState,
) -> Vue3DomProps {
    if !props.segments.is_empty() {
        return lower_ordered_hir_props_to_dom_mir(props, is_component, state);
    }

    Vue3DomProps {
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
            .map(|event| lower_hir_event_to_dom_mir(event, is_component, state))
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
                preserve_case: !is_component,
            })
            .collect(),
        normalize: Vue3DomPropsNormalize {
            normalize_props: props
                .segments
                .iter()
                .any(|segment| matches!(segment, HirPropSegment::ObjectBinding(_))),
            guard_reactive_props: props
                .segments
                .iter()
                .any(|segment| matches!(segment, HirPropSegment::ObjectBinding(_))),
        },
        segments: Vec::new(),
    }
}

pub(crate) fn lower_ordered_hir_props_to_dom_mir(
    props: &HirProps,
    is_component: bool,
    state: &mut Vue3DomLoweringState,
) -> Vue3DomProps {
    let mut segments = Vec::new();
    let mut static_attrs = Vec::new();
    let mut dynamic_bindings = Vec::new();
    let mut events = Vec::new();
    let mut object_bindings = Vec::new();
    let mut object_listeners = Vec::new();

    for segment in &props.segments {
        match segment {
            HirPropSegment::StaticAttr(attr) => {
                let lowered = lower_hir_static_attr_to_dom_mir(attr);
                static_attrs.push(lowered.clone());
                segments.push(Vue3DomPropSegment::StaticAttr(lowered));
            }
            HirPropSegment::DynamicBinding(binding) => {
                let lowered = lower_hir_binding_to_dom_mir(binding);
                dynamic_bindings.push(lowered.clone());
                segments.push(Vue3DomPropSegment::DynamicBinding(lowered));
            }
            HirPropSegment::Event(event) => {
                let lowered = lower_hir_event_to_dom_mir(event, is_component, state);
                events.push(lowered.clone());
                segments.push(Vue3DomPropSegment::Event(lowered));
            }
            HirPropSegment::ObjectBinding(binding) => {
                let lowered = Vue3DomObjectBinding {
                    value: binding.value,
                };
                object_bindings.push(lowered.clone());
                segments.push(Vue3DomPropSegment::ObjectBinding(lowered));
            }
            HirPropSegment::ObjectListeners(listeners) => {
                let lowered = Vue3DomObjectListeners {
                    value: listeners.value,
                    preserve_case: !is_component,
                };
                object_listeners.push(lowered.clone());
                segments.push(Vue3DomPropSegment::ObjectListeners(lowered));
            }
        }
    }

    Vue3DomProps {
        segments,
        static_attrs,
        dynamic_bindings,
        events,
        object_bindings,
        object_listeners,
        normalize: Vue3DomPropsNormalize {
            normalize_props: props
                .segments
                .iter()
                .any(|segment| matches!(segment, HirPropSegment::ObjectBinding(_))),
            guard_reactive_props: props
                .segments
                .iter()
                .any(|segment| matches!(segment, HirPropSegment::ObjectBinding(_))),
        },
    }
}

pub(crate) fn lower_hir_static_attr_to_dom_mir(attr: &HirStaticAttr) -> Vue3DomStaticAttr {
    Vue3DomStaticAttr {
        name: attr.name.clone(),
        value: attr.value.clone(),
    }
}

pub(crate) fn lower_hir_binding_to_dom_mir(binding: &HirBinding) -> Vue3DomBinding {
    Vue3DomBinding {
        name: binding.name.clone(),
        dynamic_name: binding.dynamic_name,
        value: binding.value,
        dynamic_arg: binding.dynamic_arg,
        camel: binding.modifiers.iter().any(|modifier| modifier == "camel"),
        force_prop: binding.modifiers.iter().any(|modifier| modifier == "prop"),
        force_attr: binding.modifiers.iter().any(|modifier| modifier == "attr"),
    }
}

pub(crate) fn lower_hir_event_to_dom_mir(
    event: &HirEvent,
    is_component: bool,
    state: &mut Vue3DomLoweringState,
) -> Vue3DomEvent {
    let cache = vue3_dom_event_cache(event, is_component, state);
    let base_name = if event.dynamic_arg {
        event.name.clone()
    } else if is_component {
        event_handler_prop_name_for_component(&event.name)
    } else {
        event_handler_prop_name_for_element(&event.name)
    };
    let modifiers = vue3_dom_event_modifiers_for(&base_name, event.dynamic_arg, &event.modifiers);
    Vue3DomEvent {
        name: if event.dynamic_arg {
            event.name.clone()
        } else if is_component {
            let event_name = modifiers
                .click_event
                .map(vue3_dom_click_event_name)
                .unwrap_or_else(|| event.name.clone());
            event_handler_prop_name_for_component(&event_name)
        } else {
            let event_name = modifiers
                .click_event
                .map(vue3_dom_click_event_name)
                .unwrap_or_else(|| event.name.clone());
            event_handler_prop_name_for_element(&event_name)
        },
        dynamic_name: event.dynamic_name,
        handler: event.handler,
        dynamic_arg: event.dynamic_arg,
        runtime_modifiers: modifiers.runtime_modifiers,
        key_modifiers: modifiers.key_modifiers,
        option_modifiers: modifiers.option_modifiers,
        click_event: modifiers.click_event,
        cache,
    }
}

#[derive(Default)]
pub(crate) struct Vue3DomEventModifiers {
    pub(crate) runtime_modifiers: Vec<String>,
    pub(crate) key_modifiers: Vec<String>,
    pub(crate) option_modifiers: Vec<String>,
    pub(crate) click_event: Option<Vue3DomClickEvent>,
}

pub(crate) fn vue3_dom_event_modifiers_for(
    event_key: &str,
    dynamic_arg: bool,
    raw_modifiers: &[String],
) -> Vue3DomEventModifiers {
    let mut modifiers = Vue3DomEventModifiers::default();
    for modifier in raw_modifiers {
        if vue3_dom_event_option_modifier(modifier) {
            modifiers.option_modifiers.push(modifier.clone());
            continue;
        }
        if vue3_dom_event_maybe_key_modifier(modifier) {
            if dynamic_arg {
                modifiers.runtime_modifiers.push(modifier.clone());
                modifiers.key_modifiers.push(modifier.clone());
            } else if vue3_dom_event_is_keyboard_event_key(event_key) {
                modifiers.key_modifiers.push(modifier.clone());
            } else {
                modifiers.runtime_modifiers.push(modifier.clone());
            }
            continue;
        }
        if vue3_dom_event_non_key_modifier(modifier) {
            modifiers.runtime_modifiers.push(modifier.clone());
        } else if dynamic_arg || vue3_dom_event_is_keyboard_event_key(event_key) {
            modifiers.key_modifiers.push(modifier.clone());
        }
    }
    if dynamic_arg || event_key.eq_ignore_ascii_case("onclick") {
        if modifiers
            .runtime_modifiers
            .iter()
            .any(|modifier| modifier == "right")
        {
            modifiers.click_event = Some(Vue3DomClickEvent::ContextMenu);
        }
        if modifiers
            .runtime_modifiers
            .iter()
            .any(|modifier| modifier == "middle")
        {
            modifiers.click_event = Some(Vue3DomClickEvent::MouseUp);
        }
    }
    modifiers
}

pub(crate) fn vue3_dom_event_option_modifier(modifier: &str) -> bool {
    matches!(modifier, "passive" | "once" | "capture")
}

pub(crate) fn vue3_dom_event_non_key_modifier(modifier: &str) -> bool {
    matches!(
        modifier,
        "stop" | "prevent" | "self" | "ctrl" | "shift" | "alt" | "meta" | "exact" | "middle"
    )
}

pub(crate) fn vue3_dom_event_maybe_key_modifier(modifier: &str) -> bool {
    matches!(modifier, "left" | "right")
}

pub(crate) fn vue3_dom_event_is_keyboard_event_key(event_key: &str) -> bool {
    matches!(
        event_key.to_ascii_lowercase().as_str(),
        "onkeyup" | "onkeydown" | "onkeypress"
    )
}

pub(crate) fn vue3_dom_click_event_name(click_event: Vue3DomClickEvent) -> String {
    match click_event {
        Vue3DomClickEvent::ContextMenu => "contextmenu".into(),
        Vue3DomClickEvent::MouseUp => "mouseup".into(),
    }
}

pub(crate) fn vue3_dom_event_cache(
    event: &HirEvent,
    is_component: bool,
    state: &mut Vue3DomLoweringState,
) -> Option<Vue3DomEventCache> {
    if !state.options.cache_handlers || state.in_v_once > 0 || is_component {
        return None;
    }
    if event.dynamic_arg {
        return None;
    }
    let index = state.next_cache_index;
    state.next_cache_index += 1;
    Some(Vue3DomEventCache { index })
}

pub(crate) fn lower_hir_directive_to_dom_mir(directive: &HirDirectiveUse) -> Vue3DomDirective {
    Vue3DomDirective {
        name: directive.name.clone(),
        argument: directive.argument.clone(),
        dynamic_argument: directive.dynamic_argument,
        expression: directive.expression,
        modifiers: directive.modifiers.clone(),
    }
}

pub(crate) fn vue3_dom_mir_dynamic_props(element: &Vue3Element) -> Vec<String> {
    if has_dynamic_arg_binding(element) {
        return Vec::new();
    }
    let mut props = element
        .props
        .iter()
        .filter_map(|prop| match prop {
            Vue3Prop::Directive(dir) if dir.name == "on" && !event_directive_is_vnode_hook(dir) => {
                if dir.is_dynamic_arg || dir.arg.is_none() {
                    return None;
                }
                let event = dir
                    .arg
                    .as_ref()
                    .map(Vue3Expression::source_string)
                    .unwrap_or_default();
                Some(event_handler_prop_name(element, &event))
            }
            Vue3Prop::Directive(dir) if dir.name == "bind" && !has_key_bind_dir(dir) => {
                if is_asset_import_binding(dir) {
                    return None;
                }
                if dir.is_dynamic_arg {
                    return None;
                }
                vue3_bind_directive_static_dom_key(dir, true)
            }
            Vue3Prop::Directive(dir)
                if dir.name == "model" && vue3_dom_model_kind(element).is_some() =>
            {
                Some("onUpdate:modelValue".into())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    props.extend(vue3_dom_content_dynamic_props(element));
    props
}

pub(crate) fn vue3_dom_mir_props_patch_names(element: &Vue3Element) -> Vec<String> {
    let mut props = element
        .props
        .iter()
        .filter_map(|prop| match prop {
            Vue3Prop::Directive(dir) if dir.name == "on" && !event_directive_is_vnode_hook(dir) => {
                if dir.is_dynamic_arg || dir.arg.is_none() {
                    return None;
                }
                let event = dir
                    .arg
                    .as_ref()
                    .map(Vue3Expression::source_string)
                    .unwrap_or_default();
                Some(event_handler_prop_name(element, &event))
            }
            Vue3Prop::Directive(dir)
                if dir.name == "bind"
                    && !has_class_bind_dir(dir)
                    && !has_style_bind_dir(dir)
                    && !has_key_bind_dir(dir) =>
            {
                if is_asset_import_binding(dir) {
                    return None;
                }
                if dir.is_dynamic_arg {
                    return None;
                }
                vue3_bind_directive_static_dom_key(dir, true)
            }
            Vue3Prop::Directive(dir)
                if dir.name == "model" && vue3_dom_model_kind(element).is_some() =>
            {
                Some("onUpdate:modelValue".into())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    props.extend(vue3_dom_content_dynamic_props(element));
    props
}

pub(crate) fn vue3_dom_content_dynamic_props(element: &Vue3Element) -> Vec<String> {
    element
        .props
        .iter()
        .filter_map(|prop| match prop {
            Vue3Prop::Directive(dir) if dir.name == "html" => Some("innerHTML".into()),
            Vue3Prop::Directive(dir) if dir.name == "text" => Some("textContent".into()),
            _ => None,
        })
        .collect()
}

pub(crate) fn has_native_v_model(element: &Vue3Element) -> bool {
    element.props.iter().any(|prop| {
        matches!(
            prop,
            Vue3Prop::Directive(dir)
                if dir.name == "model"
                    && dir.arg.is_none()
                    && vue3_dom_model_kind(element).is_some()
        )
    })
}

pub(crate) fn vue3_bind_directive_static_dom_key(
    directive: &Vue3Directive,
    apply_dom_prefix: bool,
) -> Option<String> {
    if directive.is_dynamic_arg {
        return None;
    }
    let name = directive.arg.as_ref().map(Vue3Expression::source_string)?;
    let binding = Vue3DomBinding {
        name,
        dynamic_name: None,
        value: JsExprId(0),
        dynamic_arg: false,
        camel: directive
            .modifiers
            .iter()
            .any(|modifier| modifier == "camel"),
        force_prop: directive
            .modifiers
            .iter()
            .any(|modifier| modifier == "prop"),
        force_attr: directive
            .modifiers
            .iter()
            .any(|modifier| modifier == "attr"),
    };
    Some(render_vue3_dom_binding_static_key(
        &binding,
        apply_dom_prefix,
    ))
}

pub(crate) fn has_style_binding(element: &Vue3Element) -> bool {
    element.props.iter().any(|prop| {
        matches!(
            prop,
            Vue3Prop::Directive(dir)
                if dir.name == "bind"
                    && dir
                        .arg
                        .as_ref()
                        .is_some_and(|arg| arg.source_string() == "style")
        )
    })
}

pub(crate) fn has_style_bind_dir(dir: &Vue3Directive) -> bool {
    dir.arg
        .as_ref()
        .is_some_and(|arg| arg.source_string() == "style")
}

pub(crate) fn has_dynamic_arg_binding(element: &Vue3Element) -> bool {
    element.props.iter().any(|prop| {
        matches!(
            prop,
            Vue3Prop::Directive(dir)
                if matches!(dir.name.as_str(), "bind" | "on")
                    && (dir.is_dynamic_arg || dir.arg.is_none())
        )
    })
}

pub(crate) fn has_prop_bind_modifier(element: &Vue3Element) -> bool {
    element.props.iter().any(|prop| {
        matches!(
            prop,
            Vue3Prop::Directive(dir)
                if dir.name == "bind" && dir.modifiers.iter().any(|modifier| modifier == "prop")
        )
    })
}

pub(crate) fn has_hydration_event_binding(element: &Vue3Element) -> bool {
    let is_component = element.tag_type == Vue3ElementType::Component;
    element.props.iter().any(|prop| {
        let Vue3Prop::Directive(dir) = prop else {
            return false;
        };
        if dir.name != "on" || event_directive_is_vnode_hook(dir) || is_component {
            return false;
        }
        let Some(event) = dir.arg.as_ref().map(Vue3Expression::source_string) else {
            return false;
        };
        let prop = event_handler_prop_name(element, &event);
        prop.to_ascii_lowercase() != "onclick" && prop != "onUpdate:modelValue"
    })
}

pub(crate) fn lower_vue3_element_control_flow_to_dom_mir(
    ast_id: NodeId,
    element: &Vue3Element,
    ast: &Vue3Ast,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue3DomLoweringState,
) -> Option<Option<(NodeId, NodeId)>> {
    if let Some(for_dir) = directive_by_name(element, "for") {
        let lowered = lower_vue3_for_directive_to_dom_mir(
            ast_id, element, ast, ast_node, for_dir, hir_parent, mir_parent, state,
        );
        return Some(lowered);
    }
    let if_dir = directive_by_name(element, "if")
        .or_else(|| directive_by_name(element, "else-if"))
        .filter(|dir| dir.exp.is_some());
    if let Some(if_dir) = if_dir {
        let lowered = lower_vue3_if_directive_to_dom_mir(
            ast_id, element, ast, ast_node, if_dir, hir_parent, mir_parent, state,
        );
        return Some(lowered);
    }
    None
}

pub(crate) fn lower_vue3_if_branch_chain_to_dom_mir(
    branch_ids: &[NodeId],
    ast: &Vue3Ast,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue3DomLoweringState,
) -> Option<(NodeId, NodeId)> {
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
        Vue3DomMirKind::If { condition },
        first_node.span.clone(),
    );
    state.map.record_ast_to_hir(first_id, hir_id);
    state.map.record_hir_to_mir(hir_id, mir_id);

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
        if *branch_id != first_id {
            let branch_mir = state.mir.push_child(
                mir_id,
                Vue3DomMirKind::If { condition },
                branch_node.span.clone(),
            );
            state.map.record_ast_to_hir(*branch_id, hir_id);
            state.map.record_hir_to_mir(hir_id, branch_mir);
        }
        if let Some((body_hir, _)) = lower_vue3_non_control_element_to_dom_mir(
            *branch_id,
            branch_element,
            ast,
            branch_node,
            hir_id,
            mir_id,
            state,
        ) {
            if let Some(node) = state.hir.node_mut(hir_id) {
                if let HirNodeKind::If(hir_if) = &mut node.kind {
                    hir_if.branches.push(HirIfBranch {
                        condition,
                        body: body_hir,
                    });
                }
            }
        }
    }

    Some((hir_id, mir_id))
}

pub(crate) fn lower_vue3_for_directive_to_dom_mir(
    ast_id: NodeId,
    element: &Vue3Element,
    ast: &Vue3Ast,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    directive: &Vue3Directive,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue3DomLoweringState,
) -> Option<(NodeId, NodeId)> {
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
        Vue3DomMirKind::For(Vue3ForMir {
            source: parsed.source,
            value_alias: parsed.value_alias,
            key_alias: parsed.key_alias,
            index_alias: parsed.index_alias,
            key: None,
            memo: None,
        }),
        ast_node.span.clone(),
    );
    state.map.record_ast_to_hir(ast_id, hir_id);
    state.map.record_hir_to_mir(hir_id, mir_id);

    let body =
        if let Some(if_dir) = directive_by_name(element, "if").filter(|dir| dir.exp.is_some()) {
            lower_vue3_if_directive_to_dom_mir(
                ast_id, element, ast, ast_node, if_dir, hir_id, mir_id, state,
            )
        } else {
            lower_vue3_non_control_element_to_dom_mir(
                ast_id, element, ast, ast_node, hir_id, mir_id, state,
            )
        };
    if let Some((body_hir, _)) = body {
        if let Some(node) = state.hir.node_mut(hir_id) {
            if let HirNodeKind::For(hir_for) = &mut node.kind {
                hir_for.body = body_hir;
            }
        }
    }
    let key = vue3_for_key_mir_expr(element, ast_node, &mut state.js, state.source_type);
    let memo = vue3_for_memo_mir(element, ast_node, state);
    if let Some(node) = state.mir.node_mut(mir_id) {
        if let Vue3DomMirKind::For(for_mir) = &mut node.kind {
            for_mir.key = key;
            for_mir.memo = memo;
        }
    }
    Some((hir_id, mir_id))
}

pub(crate) fn vue3_for_key_mir_expr(
    element: &Vue3Element,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    js: &mut JsAstStore,
    source_type: oxc_span::SourceType,
) -> Option<MirExpr> {
    element.props.iter().find_map(|prop| match prop {
        Vue3Prop::Directive(dir)
            if dir.name == "bind"
                && dir
                    .arg
                    .as_ref()
                    .is_some_and(|arg| arg.source_string() == "key") =>
        {
            dir.exp
                .as_ref()
                .filter(|exp| !exp.source_string().trim().is_empty())
                .map(|exp| {
                    MirExpr::JsExpr(register_or_reuse_vue3_expression_with_span(
                        js,
                        exp,
                        dir.exp_span.or_else(|| ast_node.span.source()),
                        source_type,
                    ))
                })
        }
        Vue3Prop::Attribute(attr) if attr.name == "key" => attr.value.clone().map(MirExpr::String),
        _ => None,
    })
}

pub(crate) fn vue3_for_memo_mir(
    element: &Vue3Element,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    state: &mut Vue3DomLoweringState,
) -> Option<Vue3ForMemo> {
    let memo = directive_by_name(element, "memo")?;
    let expression = register_vue3_expression_with_span(
        &mut state.js,
        memo.exp
            .as_ref()
            .unwrap_or(&Vue3Expression::Raw(String::new())),
        memo.exp_span.or_else(|| ast_node.span.source()),
        state.source_type,
    );
    let index = state.next_cache_index;
    state.next_cache_index += 1;
    Some(Vue3ForMemo { expression, index })
}

pub(crate) fn lower_vue3_if_directive_to_dom_mir(
    ast_id: NodeId,
    element: &Vue3Element,
    ast: &Vue3Ast,
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    directive: &Vue3Directive,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue3DomLoweringState,
) -> Option<(NodeId, NodeId)> {
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
        Vue3DomMirKind::If { condition },
        ast_node.span.clone(),
    );
    state.map.record_ast_to_hir(ast_id, hir_id);
    state.map.record_hir_to_mir(hir_id, mir_id);

    if let Some((body_hir, _)) = lower_vue3_non_control_element_to_dom_mir(
        ast_id, element, ast, ast_node, hir_id, mir_id, state,
    ) {
        if let Some(node) = state.hir.node_mut(hir_id) {
            if let HirNodeKind::If(hir_if) = &mut node.kind {
                hir_if.branches.push(HirIfBranch {
                    condition,
                    body: body_hir,
                });
            }
        }
    }
    Some((hir_id, mir_id))
}

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
            if directive_by_name(element, "for").is_some() {
                lower_vue3_ast_node_to_ssr_mir(child_id, ast, hir_parent, mir_parent, state);
                index += 1;
                continue;
            }
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
    if let Some(for_dir) = directive_by_name(element, "for") {
        let lowered = lower_vue3_for_directive_to_ssr_mir(
            ast_id, element, ast, ast_node, for_dir, hir_parent, mir_parent, state,
        );
        return Some(lowered);
    }
    let if_dir = directive_by_name(element, "if")
        .or_else(|| directive_by_name(element, "else-if"))
        .filter(|dir| dir.exp.is_some());
    if let Some(if_dir) = if_dir {
        let lowered = lower_vue3_if_directive_to_ssr_mir(
            ast_id, element, ast, ast_node, if_dir, hir_parent, mir_parent, state,
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
        if let Some(body_hir) = lower_vue3_plain_element_to_ssr_mir(
            *branch_id,
            branch_element,
            ast,
            branch_node,
            hir_id,
            branch_mir,
            state,
        ) {
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

    let body = if let Some(if_dir) =
        directive_by_name(element, "if").filter(|dir| dir.exp.is_some())
    {
        lower_vue3_if_directive_to_ssr_mir(
            ast_id, element, ast, ast_node, if_dir, hir_id, mir_id, state,
        )
    } else {
        lower_vue3_plain_element_to_ssr_mir(ast_id, element, ast, ast_node, hir_id, mir_id, state)
    };
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

    if let Some(body_hir) =
        lower_vue3_plain_element_to_ssr_mir(ast_id, element, ast, ast_node, hir_id, mir_id, state)
    {
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

pub(crate) fn lower_vue3_props_to_hir(
    props: &[Vue3Prop],
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    js: &mut JsAstStore,
    source_type: oxc_span::SourceType,
) -> HirProps {
    let mut hir = HirProps::default();
    for prop in props {
        match prop {
            Vue3Prop::Attribute(attr) => {
                let lowered = HirStaticAttr {
                    name: attr.name.clone(),
                    value: attr.value.clone().unwrap_or_default(),
                };
                hir.segments
                    .push(HirPropSegment::StaticAttr(lowered.clone()));
                hir.static_attrs.push(lowered);
            }
            Vue3Prop::Directive(dir) if dir.name == "bind" => {
                if let Some(exp) = &dir.exp {
                    let Some(arg) = &dir.arg else {
                        let value = register_vue3_expression_with_span(
                            js,
                            exp,
                            dir.exp_span.or_else(|| ast_node.span.source()),
                            source_type,
                        );
                        let lowered = HirObjectBinding { value };
                        hir.segments
                            .push(HirPropSegment::ObjectBinding(lowered.clone()));
                        hir.object_bindings.push(lowered);
                        continue;
                    };
                    let name = arg.source_string();
                    let dynamic_name = dir.is_dynamic_arg.then(|| {
                        register_vue3_expression_with_span(
                            js,
                            arg,
                            dir.arg_span.or_else(|| ast_node.span.source()),
                            source_type,
                        )
                    });
                    let value = register_vue3_expression_with_span(
                        js,
                        exp,
                        dir.exp_span.or_else(|| ast_node.span.source()),
                        source_type,
                    );
                    if name == "key" {
                        hir.key = Some(value);
                    } else if name == "ref" {
                        hir.ref_name = Some(vuec_ast::HirRef {
                            name: exp.source_string(),
                            in_for: false,
                        });
                    }
                    let lowered = HirBinding {
                        name,
                        dynamic_name,
                        value,
                        dynamic_arg: dir.is_dynamic_arg,
                        modifiers: dir.modifiers.clone(),
                    };
                    hir.segments
                        .push(HirPropSegment::DynamicBinding(lowered.clone()));
                    hir.dynamic_bindings.push(lowered);
                }
            }
            Vue3Prop::Directive(dir) if dir.name == "on" => {
                if let Some(exp) = &dir.exp {
                    if let Some(arg) = &dir.arg {
                        let dynamic_name = dir.is_dynamic_arg.then(|| {
                            register_vue3_expression_with_span(
                                js,
                                arg,
                                dir.arg_span.or_else(|| ast_node.span.source()),
                                source_type,
                            )
                        });
                        let lowered = HirEvent {
                            name: arg.source_string(),
                            dynamic_name,
                            handler: register_vue3_statement_with_span(
                                js,
                                exp,
                                dir.exp_span.or_else(|| ast_node.span.source()),
                                source_type,
                            ),
                            dynamic_arg: dir.is_dynamic_arg,
                            modifiers: dir.modifiers.clone(),
                        };
                        hir.segments.push(HirPropSegment::Event(lowered.clone()));
                        hir.events.push(lowered);
                    } else {
                        let lowered = HirObjectListeners {
                            value: register_vue3_expression_with_span(
                                js,
                                exp,
                                dir.exp_span.or_else(|| ast_node.span.source()),
                                source_type,
                            ),
                        };
                        hir.segments
                            .push(HirPropSegment::ObjectListeners(lowered.clone()));
                        hir.object_listeners.push(lowered);
                    }
                }
            }
            Vue3Prop::Directive(_) => {}
        }
    }
    hir
}

pub(crate) fn lower_vue3_directives_to_hir(
    props: &[Vue3Prop],
    ast_node: &vuec_ast::Node<Vue3NodeKind>,
    js: &mut JsAstStore,
    source_type: oxc_span::SourceType,
) -> Vec<HirDirectiveUse> {
    props
        .iter()
        .filter_map(|prop| match prop {
            Vue3Prop::Directive(dir)
                if !matches!(
                    dir.name.as_str(),
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
                ) =>
            {
                let dynamic_argument = dir.arg.as_ref().and_then(|arg| {
                    dir.is_dynamic_arg.then(|| {
                        register_vue3_expression_with_span(
                            js,
                            arg,
                            dir.arg_span.or_else(|| ast_node.span.source()),
                            source_type,
                        )
                    })
                });
                Some(HirDirectiveUse {
                    name: dir.name.clone(),
                    argument: dir
                        .arg
                        .as_ref()
                        .filter(|_| !dir.is_dynamic_arg)
                        .map(Vue3Expression::source_string),
                    dynamic_argument,
                    expression: dir.exp.as_ref().map(|exp| {
                        register_vue3_expression_with_span(
                            js,
                            exp,
                            dir.exp_span.or_else(|| ast_node.span.source()),
                            source_type,
                        )
                    }),
                    modifiers: dir.modifiers.clone(),
                })
            }
            _ => None,
        })
        .collect()
}

pub(crate) fn vue3_static_slot_outlet_name(prop: &Vue3Prop) -> Option<String> {
    match prop {
        Vue3Prop::Attribute(attr) if attr.name == "name" => attr.value.clone(),
        Vue3Prop::Directive(dir)
            if dir.name == "bind"
                && dir
                    .arg
                    .as_ref()
                    .is_some_and(|arg| arg.source_string() == "name") =>
        {
            dir.exp.as_ref().map(Vue3Expression::source_string)
        }
        _ => None,
    }
}

pub(crate) fn register_vue3_expression_with_span(
    store: &mut JsAstStore,
    expression: &Vue3Expression,
    span: Option<Span>,
    source_type: oxc_span::SourceType,
) -> vuec_ast::JsExprId {
    match expression {
        Vue3Expression::Raw(source) => store.register_expr(
            source.clone(),
            span.unwrap_or_else(|| Span::new(FileId(0), 0, source.len())),
            source_type,
        ),
        Vue3Expression::JsExpr(id) => *id,
    }
}

pub(crate) fn register_or_reuse_vue3_expression_with_span(
    store: &mut JsAstStore,
    expression: &Vue3Expression,
    span: Option<Span>,
    source_type: oxc_span::SourceType,
) -> vuec_ast::JsExprId {
    match expression {
        Vue3Expression::Raw(source) => {
            let span = span.unwrap_or_else(|| Span::new(FileId(0), 0, source.len()));
            if let Some((index, _)) = store
                .expressions()
                .iter()
                .enumerate()
                .find(|(_, entry)| entry.source == *source && entry.span == span)
            {
                return JsExprId(index as u32);
            }
            store.register_expr(source.clone(), span, source_type)
        }
        Vue3Expression::JsExpr(id) => *id,
    }
}

pub(crate) fn register_vue3_statement_with_span(
    store: &mut JsAstStore,
    expression: &Vue3Expression,
    span: Option<Span>,
    source_type: oxc_span::SourceType,
) -> vuec_ast::JsStmtId {
    match expression {
        Vue3Expression::Raw(source) => store.register_stmt(
            source.clone(),
            span.unwrap_or_else(|| Span::new(FileId(0), 0, source.len())),
            source_type,
        ),
        Vue3Expression::JsExpr(id) => store.register_stmt(
            format!("#expr{}", id.0),
            span.unwrap_or_else(|| Span::new(FileId(0), 0, 0)),
            source_type,
        ),
    }
}

pub(crate) fn register_vue3_pattern_with_span(
    store: &mut JsAstStore,
    expression: &Vue3Expression,
    span: Option<Span>,
    source_type: oxc_span::SourceType,
) -> JsPatternId {
    match expression {
        Vue3Expression::Raw(source) => store.register_pattern(
            source.clone(),
            span.unwrap_or_else(|| Span::new(FileId(0), 0, source.len())),
            source_type,
        ),
        Vue3Expression::JsExpr(id) => store.register_pattern(
            format!("#expr{}", id.0),
            span.unwrap_or_else(|| Span::new(FileId(0), 0, 0)),
            source_type,
        ),
    }
}
