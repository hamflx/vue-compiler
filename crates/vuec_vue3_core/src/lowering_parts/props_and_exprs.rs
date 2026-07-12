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
                    if !dir.is_dynamic_arg && name == "key" {
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
