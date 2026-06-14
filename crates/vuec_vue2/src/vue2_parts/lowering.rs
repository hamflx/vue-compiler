struct Vue2LoweringState {
    hir: Hir,
    mir: Vue2Mir,
    map: LoweringMap,
    js: JsAstStore,
    static_render_index: u32,
    once_id: u32,
    suppress_static_once_for: Option<NodeId>,
}

fn lower_vue2_child_sequence(
    children: &[NodeId],
    ast: &Vue2Ast,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue2LoweringState,
) -> Vec<(NodeId, NodeId, NodeId)> {
    let mut lowered = Vec::new();
    for child in children {
        if let Some((hir, mir)) =
            lower_vue2_ast_node_to_mir(*child, ast, hir_parent, mir_parent, state)
        {
            lowered.push((*child, hir, mir));
        }
    }
    lowered
}

fn lower_vue2_ast_node_to_mir(
    ast_id: NodeId,
    ast: &Vue2Ast,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue2LoweringState,
) -> Option<(NodeId, NodeId)> {
    lower_vue2_ast_node_to_mir_inner(ast_id, ast, hir_parent, mir_parent, state, true)
}

fn lower_vue2_ast_node_to_mir_inner(
    ast_id: NodeId,
    ast: &Vue2Ast,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue2LoweringState,
    allow_for: bool,
) -> Option<(NodeId, NodeId)> {
    let ast_node = ast.node(ast_id)?;
    match &ast_node.kind {
        Vue2AstKind::Root(_) => None,
        Vue2AstKind::Element(element) => {
            if allow_for && element.for_exp.is_some() {
                if element.once && !element.static_in_for {
                    return lower_vue2_once_for_to_mir(
                        ast_id, element, ast, ast_node, hir_parent, mir_parent, state,
                    );
                }
                return lower_vue2_for_to_mir(
                    ast_id,
                    element,
                    ast,
                    ast_node,
                    hir_parent,
                    mir_parent,
                    state,
                    Vue2ForBodyMode::Normal,
                );
            }
            if element.if_exp.is_some() && !element.if_conditions.is_empty() {
                return lower_vue2_if_to_mir(
                    ast_id, element, ast, ast_node, hir_parent, mir_parent, state,
                );
            }
            if element.elseif.is_some() || element.else_branch {
                return None;
            }
            lower_vue2_plain_element_to_mir(
                ast_id, element, ast, ast_node, hir_parent, mir_parent, state,
            )
        }
        Vue2AstKind::Text(text) => {
            let hir_id = state.hir.push_child(
                hir_parent,
                HirNodeKind::Text(vuec_ast::HirText {
                    value: text.value.clone(),
                }),
                ast_node.span.clone(),
            );
            let mir_id = state.mir.push_child(
                mir_parent,
                Vue2MirKind::Text(Vue2TextCall {
                    value: MirExpr::String(text.value.clone()),
                }),
                ast_node.span.clone(),
            );
            state.map.record_ast_to_hir(ast_id, hir_id);
            state.map.record_hir_to_mir(hir_id, mir_id);
            Some((hir_id, mir_id))
        }
        Vue2AstKind::ExpressionText(text) => {
            let expression = text
                .filter_expr
                .as_ref()
                .map(|filter| HirExpr::Vue2Filter(filter.clone()))
                .or_else(|| text.expr.map(HirExpr::Js));
            let hir_id = state.hir.push_child(
                hir_parent,
                HirNodeKind::Interpolation(HirInterpolation {
                    expression: expression.unwrap_or(HirExpr::Js(JsExprId(0))),
                }),
                ast_node.span.clone(),
            );
            let mir_id = state.mir.push_child(
                mir_parent,
                Vue2MirKind::Text(Vue2TextCall {
                    value: text
                        .filter_expr
                        .as_ref()
                        .map(|filter| MirExpr::JsExpr(filter.base))
                        .or_else(|| text.expr.map(MirExpr::JsExpr))
                        .unwrap_or_else(|| MirExpr::String(text.raw.clone())),
                }),
                ast_node.span.clone(),
            );
            state.map.record_ast_to_hir(ast_id, hir_id);
            state.map.record_hir_to_mir(hir_id, mir_id);
            if let Some(filter) = &text.filter_expr {
                for call in &filter.filters {
                    let filter_id = state.mir.push_child(
                        mir_id,
                        Vue2MirKind::FilterCall {
                            name: call.name.clone(),
                            args: call.args.clone(),
                        },
                        NodeSpan::generated(ast_node.span.source(), GeneratedReason::Lowering),
                    );
                    state.map.record_hir_to_mir(hir_id, filter_id);
                }
            }
            Some((hir_id, mir_id))
        }
        Vue2AstKind::Comment(comment) => {
            let span = NodeSpan::generated(ast_node.span.source(), GeneratedReason::Lowering);
            let hir_id =
                state
                    .hir
                    .push_child(hir_parent, HirNodeKind::Fragment(HirFragment), span.clone());
            let mir_id = state.mir.push_child(
                mir_parent,
                Vue2MirKind::Comment {
                    value: comment.value.clone(),
                },
                span,
            );
            state.map.record_ast_to_hir(ast_id, hir_id);
            state.map.record_hir_to_mir(hir_id, mir_id);
            Some((hir_id, mir_id))
        }
    }
}

#[derive(Clone, Copy)]
enum Vue2ForBodyMode {
    Normal,
    IfBranch,
}

fn lower_vue2_once_for_to_mir(
    ast_id: NodeId,
    element: &vuec_ast::Vue2Element,
    ast: &Vue2Ast,
    ast_node: &vuec_ast::Node<Vue2NodeKind>,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue2LoweringState,
) -> Option<(NodeId, NodeId)> {
    let index = state.static_render_index;
    state.static_render_index += 1;
    let wrapper = state.mir.push_child(
        mir_parent,
        Vue2MirKind::RenderStatic(Vue2RenderStatic {
            index,
            body: None,
            in_for: false,
        }),
        ast_node.span.clone(),
    );
    let previous = state.suppress_static_once_for.replace(ast_id);
    let lowered = lower_vue2_for_to_mir(
        ast_id,
        element,
        ast,
        ast_node,
        hir_parent,
        wrapper,
        state,
        Vue2ForBodyMode::Normal,
    );
    state.suppress_static_once_for = previous;
    let (hir_id, for_mir_id) = lowered?;
    if let Some(node) = state.mir.node_mut(wrapper) {
        if let Vue2MirKind::RenderStatic(render_static) = &mut node.kind {
            render_static.body = Some(for_mir_id);
        }
    }
    state.map.record_hir_to_mir(hir_id, wrapper);
    Some((hir_id, wrapper))
}

fn lower_vue2_for_to_mir(
    ast_id: NodeId,
    element: &vuec_ast::Vue2Element,
    ast: &Vue2Ast,
    ast_node: &vuec_ast::Node<Vue2NodeKind>,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue2LoweringState,
    body_mode: Vue2ForBodyMode,
) -> Option<(NodeId, NodeId)> {
    let source = element.for_exp?;
    let alias = element.alias.unwrap_or_else(|| {
        state
            .js
            .register_pattern("item", ast_node_span(ast_node), SourceType::script())
    });
    let hir_id = state.hir.push_child(
        hir_parent,
        HirNodeKind::For(HirFor {
            source,
            value_alias: alias,
            key_alias: element.iterator1,
            index_alias: element.iterator2,
            body: NodeId(0),
        }),
        ast_node.span.clone(),
    );
    let mir_id = state.mir.push_child(
        mir_parent,
        Vue2MirKind::For(Vue2ForMir {
            source,
            alias,
            iterator1: element.iterator1,
            iterator2: element.iterator2,
            body: NodeId(0),
        }),
        ast_node.span.clone(),
    );
    state.map.record_ast_to_hir(ast_id, hir_id);
    state.map.record_hir_to_mir(hir_id, mir_id);
    let previous_suppressed = if element.once {
        state.suppress_static_once_for.replace(ast_id)
    } else {
        state.suppress_static_once_for
    };
    let lowered_body = match body_mode {
        Vue2ForBodyMode::Normal => {
            lower_vue2_ast_node_to_mir_inner(ast_id, ast, hir_id, mir_id, state, false)?
        }
        Vue2ForBodyMode::IfBranch => {
            lower_vue2_if_branch_body_to_mir(ast_id, element, ast, ast_node, hir_id, mir_id, state)?
        }
    };
    if element.once {
        state.suppress_static_once_for = previous_suppressed;
    }
    let (body_hir, body_mir) = lowered_body;
    if let Some(node) = state.hir.node_mut(hir_id) {
        if let HirNodeKind::For(for_node) = &mut node.kind {
            for_node.body = body_hir;
        }
    }
    if let Some(node) = state.mir.node_mut(mir_id) {
        if let Vue2MirKind::For(for_node) = &mut node.kind {
            for_node.body = body_mir;
        }
    }
    Some((hir_id, mir_id))
}

fn lower_vue2_if_to_mir(
    ast_id: NodeId,
    element: &vuec_ast::Vue2Element,
    ast: &Vue2Ast,
    ast_node: &vuec_ast::Node<Vue2NodeKind>,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue2LoweringState,
) -> Option<(NodeId, NodeId)> {
    let hir_id = state.hir.push_child(
        hir_parent,
        HirNodeKind::If(HirIf {
            branches: Vec::new(),
        }),
        ast_node.span.clone(),
    );
    let mir_id = state.mir.push_child(
        mir_parent,
        Vue2MirKind::If(Vue2IfMir {
            branches: Vec::new(),
        }),
        ast_node.span.clone(),
    );
    state.map.record_ast_to_hir(ast_id, hir_id);
    state.map.record_hir_to_mir(hir_id, mir_id);

    let mut branches = Vec::new();
    let mut mir_branches = Vec::new();
    for condition in &element.if_conditions {
        let block = condition.block;
        let (body_hir, body_mir) = if block == ast_id {
            lower_vue2_plain_element_to_mir(ast_id, element, ast, ast_node, hir_id, mir_id, state)?
        } else {
            lower_vue2_branch_block_to_mir(block, ast, hir_id, mir_id, state)?
        };
        branches.push(HirIfBranch {
            condition: condition.exp,
            body: body_hir,
        });
        mir_branches.push(Vue2IfMirBranch {
            condition: condition.exp,
            body: body_mir,
        });
    }
    if let Some(node) = state.hir.node_mut(hir_id) {
        if let HirNodeKind::If(if_node) = &mut node.kind {
            if_node.branches = branches;
        }
    }
    if let Some(node) = state.mir.node_mut(mir_id) {
        if let Vue2MirKind::If(if_node) = &mut node.kind {
            if_node.branches = mir_branches;
        }
    }
    Some((hir_id, mir_id))
}

fn lower_vue2_branch_block_to_mir(
    block: NodeId,
    ast: &Vue2Ast,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue2LoweringState,
) -> Option<(NodeId, NodeId)> {
    let ast_node = ast.node(block)?;
    match &ast_node.kind {
        Vue2AstKind::Element(element) => {
            if element.for_exp.is_some() {
                lower_vue2_for_to_mir(
                    block,
                    element,
                    ast,
                    ast_node,
                    hir_parent,
                    mir_parent,
                    state,
                    Vue2ForBodyMode::IfBranch,
                )
            } else {
                lower_vue2_if_branch_body_to_mir(
                    block, element, ast, ast_node, hir_parent, mir_parent, state,
                )
            }
        }
        _ => lower_vue2_ast_node_to_mir_inner(block, ast, hir_parent, mir_parent, state, false),
    }
}

fn lower_vue2_if_branch_body_to_mir(
    ast_id: NodeId,
    element: &vuec_ast::Vue2Element,
    ast: &Vue2Ast,
    ast_node: &vuec_ast::Node<Vue2NodeKind>,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue2LoweringState,
) -> Option<(NodeId, NodeId)> {
    if element.if_exp.is_some() && !element.if_conditions.is_empty() {
        lower_vue2_if_to_mir(
            ast_id, element, ast, ast_node, hir_parent, mir_parent, state,
        )
    } else {
        lower_vue2_plain_element_to_mir(
            ast_id, element, ast, ast_node, hir_parent, mir_parent, state,
        )
    }
}

fn lower_vue2_plain_element_to_mir(
    ast_id: NodeId,
    element: &vuec_ast::Vue2Element,
    ast: &Vue2Ast,
    ast_node: &vuec_ast::Node<Vue2NodeKind>,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue2LoweringState,
) -> Option<(NodeId, NodeId)> {
    let hir_kind = lower_vue2_element_to_hir_kind(element, ast_node, state);
    let suppress_static_once = state.suppress_static_once_for == Some(ast_id);
    let render_static = if !suppress_static_once
        && (element.static_root || (element.once && !element.static_in_for))
    {
        let index = state.static_render_index;
        state.static_render_index += 1;
        Some(Vue2RenderStatic {
            index,
            body: None,
            in_for: element.static_in_for,
        })
    } else {
        None
    };
    let hir_id = state
        .hir
        .push_child(hir_parent, hir_kind, ast_node.span.clone());
    state.map.record_ast_to_hir(ast_id, hir_id);

    let wrapper_mir = if let Some(render_static) = render_static {
        Some(state.mir.push_child(
            mir_parent,
            Vue2MirKind::RenderStatic(render_static),
            ast_node.span.clone(),
        ))
    } else if !suppress_static_once && element.once && element.static_in_for {
        let once_id = state.once_id;
        state.once_id += 1;
        Some(state.mir.push_child(
            mir_parent,
            Vue2MirKind::Once(Vue2Once {
                body: NodeId(0),
                once_id,
                key: element.key.map(MirExpr::JsExpr),
            }),
            ast_node.span.clone(),
        ))
    } else {
        None
    };
    let content_parent = wrapper_mir.unwrap_or(mir_parent);
    if let Some(wrapper) = wrapper_mir {
        state.map.record_hir_to_mir(hir_id, wrapper);
    }

    let mir_kind = lower_vue2_element_to_mir_kind(element, ast_node, state);
    let mir_id = state
        .mir
        .push_child(content_parent, mir_kind, ast_node.span.clone());
    state.map.record_hir_to_mir(hir_id, mir_id);

    let branch_blocks = element
        .if_conditions
        .iter()
        .skip(1)
        .map(|condition| condition.block)
        .collect::<Vec<_>>();
    let children = if element.inline_template {
        Vec::new()
    } else {
        ast_node
            .children
            .iter()
            .copied()
            .filter(|child| {
                !branch_blocks.contains(child)
                    && !element.scoped_slots.values().any(|slot| slot == child)
            })
            .collect::<Vec<_>>()
    };
    lower_vue2_child_sequence(&children, ast, hir_id, mir_id, state);
    lower_vue2_inline_template_to_mir(element, ast, ast_node, hir_id, mir_id, state);
    lower_vue2_scoped_slots_to_mir(element, ast_id, ast, hir_id, mir_id, state);

    if let Some(wrapper) = wrapper_mir {
        if let Some(node) = state.mir.node_mut(wrapper) {
            match &mut node.kind {
                Vue2MirKind::RenderStatic(render_static) => {
                    render_static.body = Some(mir_id);
                }
                Vue2MirKind::Once(once) => {
                    once.body = mir_id;
                }
                _ => {}
            }
        }
    }
    Some((hir_id, wrapper_mir.unwrap_or(mir_id)))
}

fn lower_vue2_element_to_hir_kind(
    element: &vuec_ast::Vue2Element,
    ast_node: &vuec_ast::Node<Vue2NodeKind>,
    state: &mut Vue2LoweringState,
) -> HirNodeKind {
    if element.tag == "slot" {
        return HirNodeKind::SlotOutlet(HirSlotOutlet {
            name: element.slot_name.clone(),
            props: lower_vue2_props_to_hir(element, ast_node, state),
        });
    }

    let props = lower_vue2_props_to_hir(element, ast_node, state);
    if let Some(component) = &element.component {
        return HirNodeKind::Component(vuec_ast::HirComponent {
            name: component.clone(),
            props,
        });
    }

    HirNodeKind::Element(HirElement {
        tag: HirTag::Native(element.tag.clone()),
        namespace: vue2_namespace(element.ns.as_deref()),
        props,
        directives: lower_vue2_directives_to_hir(element, ast_node, state),
        constness: if element.static_root {
            HirConstness::Constant
        } else if element.static_node {
            HirConstness::Static
        } else {
            HirConstness::Dynamic
        },
    })
}

fn lower_vue2_element_to_mir_kind(
    element: &vuec_ast::Vue2Element,
    ast_node: &vuec_ast::Node<Vue2NodeKind>,
    state: &mut Vue2LoweringState,
) -> Vue2MirKind {
    if element.tag == "slot" {
        return Vue2MirKind::SlotOutlet(Vue2SlotOutlet {
            name: element
                .slot_name
                .as_ref()
                .map(|name| {
                    MirExpr::JsExpr(state.js.register_expr(
                        name,
                        ast_node_span(ast_node),
                        SourceType::script(),
                    ))
                })
                .unwrap_or_else(|| MirExpr::String("default".into())),
            props: lower_vue2_slot_outlet_props(element, ast_node, state),
            bind: lower_vue2_slot_outlet_bind(element, ast_node, state),
        });
    }

    let explicit_component = element.component.is_some();
    let tag = element
        .component
        .as_ref()
        .map(|component| {
            MirExpr::JsExpr(state.js.register_expr(
                component,
                ast_node_span(ast_node),
                SourceType::script(),
            ))
        })
        .unwrap_or_else(|| MirExpr::String(element.tag.clone()));

    Vue2MirKind::CreateElement(Vue2CreateElement {
        tag,
        data: lower_vue2_data_object(element, ast_node, state),
        is_component: explicit_component,
        is_template: element.tag == "template" && element.slot_target.is_none(),
        validation: (!element.validators.is_empty() || element.validate.is_some()).then(|| {
            Vue2ValidationData {
                validate: element.validate.clone(),
                validators: element.validators.clone(),
            }
        }),
        normalization_type: Vue2NormalizationType::None,
    })
}

fn lower_vue2_data_object(
    element: &vuec_ast::Vue2Element,
    ast_node: &vuec_ast::Node<Vue2NodeKind>,
    state: &mut Vue2LoweringState,
) -> Option<Vue2DataObject> {
    let mut data = Vue2DataObject::default();
    data.directives = element
        .directives
        .iter()
        .map(|directive| Vue2DirectiveRuntime {
            name: directive.name.clone(),
            raw_name: directive.raw_name.clone(),
            value: directive.value.map(MirExpr::JsExpr),
            arg: directive.arg.as_ref().map(|arg| {
                if directive.is_dynamic_arg {
                    MirExpr::JsExpr(state.js.register_expr(
                        arg,
                        ast_node_span(ast_node),
                        SourceType::script(),
                    ))
                } else {
                    MirExpr::String(arg.clone())
                }
            }),
            is_dynamic_arg: directive.is_dynamic_arg,
            modifiers: directive.modifiers.clone(),
        })
        .collect();
    data.key = element.key.map(MirExpr::JsExpr);
    data.ref_name = element.ref_name.as_ref().map(|name| {
        MirExpr::JsExpr(
            state
                .js
                .register_expr(name, ast_node_span(ast_node), SourceType::script()),
        )
    });
    data.ref_in_for = element.ref_in_for;
    data.pre = element.pre;
    data.tag = element.component.as_ref().map(|_| element.tag.clone());
    data.static_class = element.static_class.as_ref().map(|value| {
        MirExpr::JsExpr(state.js.register_expr(
            value,
            ast_node_span(ast_node),
            SourceType::script(),
        ))
    });
    data.class_binding = element.class_binding.map(MirExpr::JsExpr);
    data.static_style = element.static_style.as_ref().map(|value| {
        MirExpr::JsExpr(state.js.register_expr(
            value,
            ast_node_span(ast_node),
            SourceType::script(),
        ))
    });
    data.style_binding = element.style_binding.map(MirExpr::JsExpr);
    data.attrs = lower_vue2_data_props(&element.attrs, ast_node, state);
    data.dom_props = lower_vue2_data_props(&element.props, ast_node, state);
    data.dynamic_attrs = lower_vue2_data_props(&element.dynamic_attrs, ast_node, state);
    data.events = element.events.clone();
    data.native_events = element.native_events.clone();
    if element.slot_scope.is_none() {
        data.slot = element.slot_target.as_ref().map(|slot| {
            MirExpr::JsExpr(state.js.register_expr(
                slot,
                ast_node_span(ast_node),
                SourceType::script(),
            ))
        });
    }
    data.model = element.model.as_ref().map(|model| Vue2ComponentModelMir {
        value: MirExpr::JsExpr(model.value),
        callback: model.callback,
        expression: model.expression.clone(),
    });
    data.validate = element.validate.clone();
    data.validators = element.validators.clone();
    data.wrap_data = element.wrap_data.as_ref().map(|wrap| match wrap {
        vuec_ast::Vue2DataWrap::Bind { value, prop, sync } => Vue2BindWrap {
            value: MirExpr::JsExpr(*value),
            prop: *prop,
            sync: *sync,
        },
    });
    data.wrap_listeners = element.wrap_listeners.as_ref().map(|listeners| {
        MirExpr::JsExpr(state.js.register_expr(
            listeners,
            ast_node_span(ast_node),
            SourceType::script(),
        ))
    });

    (!data.directives.is_empty()
        || data.key.is_some()
        || data.ref_name.is_some()
        || data.ref_in_for
        || data.pre
        || data.tag.is_some()
        || data.static_class.is_some()
        || data.class_binding.is_some()
        || data.static_style.is_some()
        || data.style_binding.is_some()
        || !data.attrs.is_empty()
        || !data.dom_props.is_empty()
        || !data.dynamic_attrs.is_empty()
        || !data.events.is_empty()
        || !data.native_events.is_empty()
        || data.slot.is_some()
        || !data.scoped_slots.is_empty()
        || data.model.is_some()
        || data.inline_template.is_some()
        || data.validate.is_some()
        || !data.validators.is_empty()
        || data.wrap_data.is_some()
        || data.wrap_listeners.is_some()
        || element.slot_scope.is_some()
        || element.inline_template
        || vue2_raw_empty_class_or_style_requires_data(element))
    .then_some(data)
}

fn vue2_raw_empty_class_or_style_requires_data(element: &vuec_ast::Vue2Element) -> bool {
    ["class", "style"].iter().any(|name| {
        element
            .raw_attrs_map
            .get(*name)
            .is_some_and(|attr| attr.value.is_empty())
    })
}

fn lower_vue2_data_props(
    attrs: &[vuec_ast::Vue2Attribute],
    ast_node: &vuec_ast::Node<Vue2NodeKind>,
    state: &mut Vue2LoweringState,
) -> Vec<Vue2DataProp> {
    attrs
        .iter()
        .map(|attr| Vue2DataProp {
            name: attr.name.clone(),
            value: MirExpr::JsExpr(state.js.register_expr(
                attr.value.clone(),
                attr.span.unwrap_or_else(|| ast_node_span(ast_node)),
                SourceType::script(),
            )),
            dynamic: attr.dynamic,
            static_attribute: !attr.dynamic && attr.value.trim_start().starts_with('"'),
        })
        .collect()
}

fn lower_vue2_slot_outlet_props(
    element: &vuec_ast::Vue2Element,
    ast_node: &vuec_ast::Node<Vue2NodeKind>,
    state: &mut Vue2LoweringState,
) -> Vec<Vue2DataProp> {
    element
        .attrs
        .iter()
        .chain(element.dynamic_attrs.iter())
        .map(|attr| Vue2DataProp {
            name: camelize(&attr.name),
            value: MirExpr::JsExpr(state.js.register_expr(
                attr.value.clone(),
                attr.span.unwrap_or_else(|| ast_node_span(ast_node)),
                SourceType::script(),
            )),
            dynamic: attr.dynamic,
            static_attribute: !attr.dynamic && attr.value.trim_start().starts_with('"'),
        })
        .collect()
}

fn lower_vue2_slot_outlet_bind(
    element: &vuec_ast::Vue2Element,
    ast_node: &vuec_ast::Node<Vue2NodeKind>,
    state: &mut Vue2LoweringState,
) -> Option<MirExpr> {
    let value = element.attrs_map.get("v-bind")?;
    let span = element
        .raw_attrs_map
        .get("v-bind")
        .and_then(|attr| attr.span)
        .unwrap_or_else(|| ast_node_span(ast_node));
    Some(MirExpr::JsExpr(state.js.register_expr(
        value.clone(),
        span,
        SourceType::script(),
    )))
}

fn lower_vue2_inline_template_to_mir(
    element: &vuec_ast::Vue2Element,
    ast: &Vue2Ast,
    ast_node: &vuec_ast::Node<Vue2NodeKind>,
    hir_parent: NodeId,
    mir_id: NodeId,
    state: &mut Vue2LoweringState,
) {
    if !element.inline_template {
        return;
    }
    let body = ast_node.children.iter().copied().find(|child| {
        ast.node(*child)
            .is_some_and(|node| matches!(node.kind, Vue2AstKind::Element(_)))
    });
    let lowered_body = body.and_then(|body| {
        lower_vue2_ast_node_to_mir_inner(body, ast, hir_parent, mir_id, state, true)
            .map(|(_, mir)| mir)
    });
    if let Some(Vue2MirKind::CreateElement(create)) =
        state.mir.node_mut(mir_id).map(|node| &mut node.kind)
    {
        let data = create.data.get_or_insert_with(Vue2DataObject::default);
        data.inline_template = Some(Vue2InlineTemplate { body: lowered_body });
    }
}

fn lower_vue2_scoped_slots_to_mir(
    element: &vuec_ast::Vue2Element,
    element_id: NodeId,
    ast: &Vue2Ast,
    hir_parent: NodeId,
    mir_id: NodeId,
    state: &mut Vue2LoweringState,
) {
    if element.scoped_slots.is_empty() {
        return;
    }

    let mut scoped_slots = element.scoped_slots.iter().collect::<Vec<_>>();
    scoped_slots.sort_by_key(|(_, slot_id)| vue2_ast_node_source_order(ast, **slot_id));

    let mut slots = Vec::new();
    for (key, slot_id) in scoped_slots {
        let Some(slot_node) = ast.node(*slot_id) else {
            continue;
        };
        let Vue2AstKind::Element(slot) = &slot_node.kind else {
            continue;
        };
        if let Some(slot_payload) = lower_vue2_scoped_slot_to_mir(
            key,
            *slot_id,
            slot,
            ast,
            hir_parent,
            mir_id,
            state,
            slot.if_exp.filter(|_| slot.slot_new_syntax),
            slot.if_exp
                .filter(|_| !slot.slot_new_syntax && slot.tag == "template"),
            slot.slot_new_syntax,
            element.for_exp.is_some(),
        ) {
            slots.push(slot_payload);
        }
    }

    let (force_update, needs_key) =
        vue2_scoped_slot_stability(element_id, element, ast, &slots, state);
    for slot in &mut slots {
        slot.force_update = force_update;
        slot.needs_key = !force_update && needs_key;
    }
    if let Some(Vue2MirKind::CreateElement(create)) =
        state.mir.node_mut(mir_id).map(|node| &mut node.kind)
    {
        let data = create.data.get_or_insert_with(Vue2DataObject::default);
        data.scoped_slots = slots;
    }
}

#[allow(clippy::too_many_arguments)]
fn lower_vue2_scoped_slot_to_mir(
    key: &str,
    slot_id: NodeId,
    slot: &vuec_ast::Vue2Element,
    ast: &Vue2Ast,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue2LoweringState,
    condition: Option<JsExprId>,
    legacy_condition: Option<JsExprId>,
    include_new_syntax_branches: bool,
    parent_forces_update: bool,
) -> Option<Vue2ScopedSlot> {
    let slot_node = ast.node(slot_id)?;
    let slot_mir_id = state.mir.push_child(
        mir_parent,
        Vue2MirKind::ScopedSlot(Vue2ScopedSlot {
            name: slot
                .slot_target
                .as_ref()
                .map(|target| {
                    MirExpr::JsExpr(state.js.register_expr(
                        target,
                        ast_node_span(slot_node),
                        SourceType::script(),
                    ))
                })
                .unwrap_or_else(|| {
                    MirExpr::JsExpr(state.js.register_expr(
                        key,
                        ast_node_span(slot_node),
                        SourceType::script(),
                    ))
                }),
            params: slot.slot_scope,
            body: Vec::new(),
            proxy: slot
                .slot_scope
                .and_then(|scope| state.js.pattern_entry(scope))
                .is_some_and(|entry| entry.source.as_str() == "_empty_"),
            new_syntax: slot.slot_new_syntax,
            body_is_fragment: slot.tag == "template",
            condition,
            branches: Vec::new(),
            legacy_condition,
            for_source: slot.for_exp,
            for_alias: slot.alias,
            for_iterator1: slot.iterator1,
            for_iterator2: slot.iterator2,
            force_update: false,
            needs_key: false,
        }),
        slot_node.span.clone(),
    );

    let body =
        lower_vue2_scoped_slot_body_to_mir(slot_id, slot, ast, hir_parent, slot_mir_id, state);
    let branches = if include_new_syntax_branches {
        lower_vue2_scoped_slot_branches_to_mir(slot, ast, hir_parent, mir_parent, state)
    } else {
        Vec::new()
    };
    let force_update =
        vue2_scoped_slot_forces_update(parent_forces_update, slot_id, slot, ast, &branches);

    if let Some(node) = state.mir.node_mut(slot_mir_id) {
        if let Vue2MirKind::ScopedSlot(slot_payload) = &mut node.kind {
            slot_payload.body = body;
            slot_payload.branches = branches;
            slot_payload.force_update = force_update;
            return Some(slot_payload.clone());
        }
    }
    None
}

fn lower_vue2_scoped_slot_branches_to_mir(
    slot: &vuec_ast::Vue2Element,
    ast: &Vue2Ast,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue2LoweringState,
) -> Vec<Vue2ScopedSlotBranch> {
    slot.if_conditions
        .iter()
        .skip(1)
        .filter_map(|condition| {
            let branch_node = ast.node(condition.block)?;
            let Vue2AstKind::Element(branch_slot) = &branch_node.kind else {
                return None;
            };
            let key = branch_slot.slot_target.as_deref().unwrap_or("\"default\"");
            let slot = lower_vue2_scoped_slot_to_mir(
                key,
                condition.block,
                branch_slot,
                ast,
                hir_parent,
                mir_parent,
                state,
                None,
                None,
                false,
                false,
            )?;
            Some(Vue2ScopedSlotBranch {
                condition: condition.exp,
                slot: Box::new(slot),
            })
        })
        .collect()
}

fn vue2_scoped_slot_forces_update(
    parent_forces_update: bool,
    slot_id: NodeId,
    slot: &vuec_ast::Vue2Element,
    ast: &Vue2Ast,
    branches: &[Vue2ScopedSlotBranch],
) -> bool {
    parent_forces_update
        || slot.slot_target_dynamic
        || slot.if_exp.is_some()
        || slot.for_exp.is_some()
        || vue2_ast_contains_slot_child(slot_id, ast)
        || branches.iter().any(|branch| branch.slot.force_update)
}

fn vue2_ast_node_source_order(ast: &Vue2Ast, node_id: NodeId) -> (usize, usize, u32) {
    ast.node(node_id)
        .and_then(|node| {
            node.span
                .source()
                .map(|span| (span.start.0, span.end.0, node_id.0))
        })
        .unwrap_or((usize::MAX, usize::MAX, node_id.0))
}

fn vue2_scoped_slot_stability(
    element_id: NodeId,
    element: &vuec_ast::Vue2Element,
    ast: &Vue2Ast,
    slots: &[Vue2ScopedSlot],
    state: &Vue2LoweringState,
) -> (bool, bool) {
    let mut force_update = element.for_exp.is_some() || slots.iter().any(|slot| slot.force_update);
    let mut needs_key = element.if_exp.is_some();

    if !force_update {
        let mut child_id = element_id;
        let mut parent = ast.node(element_id).and_then(|node| node.parent);
        while let Some(parent_id) = parent {
            let Some(parent_node) = ast.node(parent_id) else {
                break;
            };
            if let Vue2AstKind::Element(parent_element) = &parent_node.kind {
                if vue2_is_synthetic_if_branch_parent(parent_element, parent_id, child_id) {
                    child_id = parent_id;
                    parent = parent_node.parent;
                    continue;
                }
                if vue2_scoped_slot_parent_scope_forces_update(parent_element, state)
                    || parent_element.for_exp.is_some()
                {
                    force_update = true;
                    break;
                }
                if parent_element.if_exp.is_some() {
                    needs_key = true;
                }
            }
            child_id = parent_id;
            parent = parent_node.parent;
        }
    }

    (force_update, needs_key)
}

fn vue2_is_synthetic_if_branch_parent(
    parent: &vuec_ast::Vue2Element,
    parent_id: NodeId,
    child_id: NodeId,
) -> bool {
    parent
        .if_conditions
        .iter()
        .skip(1)
        .any(|condition| condition.block == child_id && condition.block != parent_id)
}

fn vue2_scoped_slot_parent_scope_forces_update(
    element: &vuec_ast::Vue2Element,
    state: &Vue2LoweringState,
) -> bool {
    let Some(scope) = element.slot_scope else {
        return false;
    };
    state
        .js
        .pattern_entry(scope)
        .is_some_and(|entry| entry.source.as_str() != "_empty_")
}

fn lower_vue2_scoped_slot_body_to_mir(
    slot_id: NodeId,
    slot: &vuec_ast::Vue2Element,
    ast: &Vue2Ast,
    hir_parent: NodeId,
    mir_parent: NodeId,
    state: &mut Vue2LoweringState,
) -> Vec<NodeId> {
    if slot.tag == "template" {
        let branch_blocks = slot
            .if_conditions
            .iter()
            .skip(1)
            .map(|condition| condition.block)
            .collect::<Vec<_>>();
        let children = ast
            .node(slot_id)
            .map(|node| {
                node.children
                    .iter()
                    .copied()
                    .filter(|child| !branch_blocks.contains(child))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        return lower_vue2_child_sequence(&children, ast, hir_parent, mir_parent, state)
            .into_iter()
            .map(|(_, _, mir)| mir)
            .collect();
    }

    lower_vue2_ast_node_to_mir_inner(slot_id, ast, hir_parent, mir_parent, state, false)
        .map(|(_, mir)| vec![mir])
        .unwrap_or_default()
}

fn vue2_ast_contains_slot_child(ast_id: NodeId, ast: &Vue2Ast) -> bool {
    let Some(node) = ast.node(ast_id) else {
        return false;
    };
    match &node.kind {
        Vue2AstKind::Element(element) if element.tag == "slot" => true,
        Vue2AstKind::Element(element) => {
            let branch_blocks = element
                .if_conditions
                .iter()
                .skip(1)
                .map(|condition| condition.block)
                .collect::<Vec<_>>();
            node.children
                .iter()
                .filter(|child| {
                    !branch_blocks.contains(child)
                        && !element.scoped_slots.values().any(|slot| slot == *child)
                })
                .any(|child| vue2_ast_contains_slot_child(*child, ast))
        }
        Vue2AstKind::Root(_) => node
            .children
            .iter()
            .any(|child| vue2_ast_contains_slot_child(*child, ast)),
        Vue2AstKind::Text(_) | Vue2AstKind::ExpressionText(_) | Vue2AstKind::Comment(_) => false,
    }
}

fn lower_vue2_props_to_hir(
    element: &vuec_ast::Vue2Element,
    ast_node: &vuec_ast::Node<Vue2NodeKind>,
    state: &mut Vue2LoweringState,
) -> HirProps {
    let mut props = HirProps {
        key: element.key,
        ref_name: element.ref_name.as_ref().map(|name| HirRef {
            name: name.clone(),
            in_for: element.ref_in_for,
        }),
        ..HirProps::default()
    };

    for attr in &element.attrs {
        lower_vue2_attr_to_hir(attr, ast_node, &mut props, state);
    }
    for attr in &element.props {
        lower_vue2_binding_attr_to_hir(attr, ast_node, &mut props, state);
    }
    for attr in &element.dynamic_attrs {
        lower_vue2_binding_attr_to_hir(attr, ast_node, &mut props, state);
    }
    if let Some(class_binding) = element.class_binding {
        push_hir_binding(
            &mut props,
            HirBinding {
                name: "class".into(),
                dynamic_name: None,
                value: class_binding,
                dynamic_arg: false,
                modifiers: Vec::new(),
            },
        );
    }
    if let Some(style_binding) = element.style_binding {
        push_hir_binding(
            &mut props,
            HirBinding {
                name: "style".into(),
                dynamic_name: None,
                value: style_binding,
                dynamic_arg: false,
                modifiers: Vec::new(),
            },
        );
    }
    for (event, handlers) in &element.events {
        for handler in handlers {
            let lowered = HirEvent {
                name: event.clone(),
                dynamic_name: None,
                handler: handler.value,
                dynamic_arg: handler.dynamic,
                modifiers: handler.modifiers.keys().cloned().collect(),
            };
            props.segments.push(HirPropSegment::Event(lowered.clone()));
            props.events.push(lowered);
        }
    }
    for (event, handlers) in &element.native_events {
        for handler in handlers {
            let lowered = HirEvent {
                name: format!("native:{event}"),
                dynamic_name: None,
                handler: handler.value,
                dynamic_arg: handler.dynamic,
                modifiers: handler.modifiers.keys().cloned().collect(),
            };
            props.segments.push(HirPropSegment::Event(lowered.clone()));
            props.events.push(lowered);
        }
    }
    if let Some(vuec_ast::Vue2DataWrap::Bind { value, .. }) = element.wrap_data {
        let lowered = HirObjectBinding { value };
        props
            .segments
            .push(HirPropSegment::ObjectBinding(lowered.clone()));
        props.object_bindings.push(lowered);
    }
    if let Some(listeners) = &element.wrap_listeners {
        let lowered = HirObjectListeners {
            value: state
                .js
                .register_expr(listeners, ast_node_span(ast_node), SourceType::script()),
        };
        props
            .segments
            .push(HirPropSegment::ObjectListeners(lowered.clone()));
        props.object_listeners.push(lowered);
    }
    props
}

fn lower_vue2_attr_to_hir(
    attr: &vuec_ast::Vue2Attribute,
    ast_node: &vuec_ast::Node<Vue2NodeKind>,
    props: &mut HirProps,
    state: &mut Vue2LoweringState,
) {
    if !attr.dynamic && attr.value.trim_start().starts_with('"') {
        let lowered = HirStaticAttr {
            name: attr.name.clone(),
            value: attr.value.clone(),
        };
        props
            .segments
            .push(HirPropSegment::StaticAttr(lowered.clone()));
        props.static_attrs.push(lowered);
    } else {
        lower_vue2_binding_attr_to_hir(attr, ast_node, props, state);
    }
}

fn lower_vue2_binding_attr_to_hir(
    attr: &vuec_ast::Vue2Attribute,
    ast_node: &vuec_ast::Node<Vue2NodeKind>,
    props: &mut HirProps,
    state: &mut Vue2LoweringState,
) {
    let value = state.js.register_expr(
        attr.value.clone(),
        attr.span.unwrap_or_else(|| ast_node_span(ast_node)),
        SourceType::script(),
    );
    let lowered = HirBinding {
        name: attr.name.clone(),
        dynamic_name: attr.dynamic.then(|| {
            state.js.register_expr(
                attr.name.clone(),
                attr.span.unwrap_or_else(|| ast_node_span(ast_node)),
                SourceType::script(),
            )
        }),
        value,
        dynamic_arg: attr.dynamic,
        modifiers: Vec::new(),
    };
    push_hir_binding(props, lowered);
}

fn push_hir_binding(props: &mut HirProps, binding: HirBinding) {
    props
        .segments
        .push(HirPropSegment::DynamicBinding(binding.clone()));
    props.dynamic_bindings.push(binding);
}

fn lower_vue2_directives_to_hir(
    element: &vuec_ast::Vue2Element,
    ast_node: &vuec_ast::Node<Vue2NodeKind>,
    state: &mut Vue2LoweringState,
) -> Vec<HirDirectiveUse> {
    element
        .directives
        .iter()
        .map(|directive| HirDirectiveUse {
            name: directive.name.clone(),
            argument: (!directive.is_dynamic_arg)
                .then(|| directive.arg.clone())
                .flatten(),
            dynamic_argument: directive.arg.as_ref().and_then(|arg| {
                directive.is_dynamic_arg.then(|| {
                    state
                        .js
                        .register_expr(arg, ast_node_span(ast_node), SourceType::script())
                })
            }),
            expression: directive.value,
            modifiers: directive.modifiers.keys().cloned().collect(),
        })
        .collect()
}

fn vue2_namespace(ns: Option<&str>) -> HtmlNamespace {
    match ns {
        Some("svg") => HtmlNamespace::Svg,
        Some("math") | Some("mathml") => HtmlNamespace::MathMl,
        _ => HtmlNamespace::Html,
    }
}

fn ast_node_span(node: &vuec_ast::Node<Vue2NodeKind>) -> Span {
    node.span
        .source()
        .unwrap_or_else(|| Span::new(FileId(0), 0, 0))
}

fn js_span(span: Option<Span>) -> Span {
    span.unwrap_or_else(|| Span::new(FileId(0), 0, 0))
}
