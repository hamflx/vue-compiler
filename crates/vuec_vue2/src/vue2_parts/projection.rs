fn project_public_ast(
    template: &str,
    element_ast: Option<&Vue2Element>,
) -> Vue2AstProjectionResult {
    let mut projection = Vue2AstProjection {
        ast: Vue2Ast::with_capacity(
            Vue2NodeKind::root(),
            Some(Span::new(FileId(0), 0, template.len())),
            vuec_ast::template_node_capacity_hint(template),
        ),
        js: JsAstStore::new(),
        source_type: SourceType::script(),
    };
    let root = projection.ast.root;
    if let Some(element) = element_ast {
        let mut element = element.clone();
        sync_scoped_slot_if_conditions(Some(&mut element));
        projection.project_element(root, &element);
    }
    Vue2AstProjectionResult {
        ast: projection.ast,
        js: projection.js,
    }
}

struct Vue2AstProjection {
    ast: Vue2Ast,
    js: JsAstStore,
    source_type: SourceType,
}

impl Vue2AstProjection {
    fn project_element(&mut self, parent: NodeId, element: &Vue2Element) -> NodeId {
        let payload = self.project_element_payload(element);
        let id = self.ast.push(Vue2AstKind::Element(payload), element.span);
        self.ast.attach_child(parent, id);

        for child in &element.children {
            match child {
                Vue2Node::Element(element) => {
                    self.project_element(id, element);
                }
                Vue2Node::Text(text) => {
                    self.project_text(id, text);
                }
            }
        }

        let scoped_slots = element
            .scoped_slots
            .iter()
            .map(|(name, slot)| (name.clone(), self.project_element(id, slot)))
            .collect::<BTreeMap<_, _>>();
        let if_conditions = self.project_if_conditions(id, element);
        if let Some(node) = self.ast.node_mut(id) {
            if let Vue2AstKind::Element(projected) = &mut node.kind {
                projected.scoped_slots = scoped_slots;
                projected.if_conditions = if_conditions;
            }
        }
        id
    }

    fn project_text(&mut self, parent: NodeId, text: &Vue2Text) -> NodeId {
        let kind = if text.is_comment {
            Vue2AstKind::Comment(vuec_ast::Vue2Comment {
                value: text.text.clone(),
            })
        } else if let Some(expression) = text.expression.as_ref() {
            Vue2AstKind::ExpressionText(vuec_ast::Vue2ExpressionText {
                raw: text.text.clone(),
                expr: Some(self.register_expr(expression, text.span)),
                filter_expr: self.project_filter_expression(&text.text, text.span),
            })
        } else {
            Vue2AstKind::Text(vuec_ast::Vue2Text {
                value: text.text.clone(),
                static_node: text.static_node,
            })
        };
        self.ast.push_child(parent, kind, text.span)
    }

    fn project_if_conditions(
        &mut self,
        primary_id: NodeId,
        element: &Vue2Element,
    ) -> Vec<vuec_ast::Vue2IfCondition> {
        if element.if_conditions.is_empty() {
            return element
                .if_exp
                .as_ref()
                .map(|condition| {
                    vec![vuec_ast::Vue2IfCondition {
                        exp: Some(self.register_expr(condition, element.if_span.or(element.span))),
                        block: primary_id,
                        span: element.if_span,
                    }]
                })
                .unwrap_or_default();
        }

        element
            .if_conditions
            .iter()
            .enumerate()
            .map(|(index, condition)| {
                let block = if index == 0 {
                    primary_id
                } else {
                    self.project_element(primary_id, &condition.block)
                };
                vuec_ast::Vue2IfCondition {
                    exp: condition.exp.as_ref().map(|exp| {
                        self.register_expr(
                            exp,
                            condition
                                .block
                                .if_span
                                .or(condition.block.elseif_span)
                                .or(element.span),
                        )
                    }),
                    block,
                    span: condition
                        .block
                        .if_span
                        .or(condition.block.elseif_span)
                        .or(condition.block.else_span),
                }
            })
            .collect()
    }

    fn project_element_payload(&mut self, element: &Vue2Element) -> vuec_ast::Vue2Element {
        let mut payload = vuec_ast::Vue2Element::new(element.tag.clone());
        payload.attrs_list = element.attrs_list.iter().map(project_attribute).collect();
        payload.attrs_map = element.attrs_map.clone();
        payload.raw_attrs_map = element
            .raw_attrs_map
            .iter()
            .map(|(name, attr)| (name.clone(), project_attribute(attr)))
            .collect();
        payload.attrs = element.attrs.iter().map(project_attribute).collect();
        payload.props = element.props.iter().map(project_attribute).collect();
        payload.dynamic_attrs = element
            .dynamic_attrs
            .iter()
            .map(project_attribute)
            .collect();
        payload.directives = element
            .directives
            .iter()
            .map(|directive| self.project_directive(directive))
            .collect();
        payload.events = self.project_events(&element.events);
        payload.native_events = self.project_events(&element.native_events);
        payload.ns = element.ns.clone();
        payload.plain = element.plain;
        payload.forbidden = element.forbidden;
        payload.pre = element.pre;
        payload.once = element.once;
        payload.has_bindings = element.has_bindings;
        payload.if_exp = element
            .if_exp
            .as_ref()
            .map(|exp| self.register_expr(exp, element.if_span.or(element.span)));
        payload.if_span = element.if_span;
        payload.elseif = element
            .elseif
            .as_ref()
            .map(|exp| self.register_expr(exp, element.elseif_span.or(element.span)));
        payload.elseif_span = element.elseif_span;
        payload.else_branch = element.else_branch;
        payload.else_span = element.else_span;
        payload.for_exp = element
            .for_exp
            .as_ref()
            .map(|exp| self.register_expr(exp, element.for_span.or(element.span)));
        payload.for_span = element.for_span;
        payload.alias = element
            .alias
            .as_ref()
            .map(|alias| self.register_pattern(alias, element.for_span.or(element.span)));
        payload.iterator1 = element
            .iterator1
            .as_ref()
            .map(|alias| self.register_pattern(alias, element.for_span.or(element.span)));
        payload.iterator2 = element
            .iterator2
            .as_ref()
            .map(|alias| self.register_pattern(alias, element.for_span.or(element.span)));
        payload.key = element
            .key
            .as_ref()
            .map(|key| self.register_expr(key, element.key_span.or(element.span)));
        payload.key_span = element.key_span;
        payload.ref_name = element.ref_name.clone();
        payload.ref_in_for = element.ref_in_for;
        payload.slot_name = element.slot_name.clone();
        payload.slot_target = element.slot_target.clone();
        payload.slot_target_dynamic = element.slot_target_dynamic;
        payload.slot_scope = element
            .slot_scope
            .as_ref()
            .map(|scope| self.register_pattern(scope, element.span));
        payload.slot_new_syntax = element.slot_new_syntax;
        payload.component = element.component.clone();
        payload.inline_template = element.inline_template;
        payload.static_class = element.static_class.clone();
        payload.class_binding = element
            .class_binding
            .as_ref()
            .map(|binding| self.register_expr(binding, element.span));
        payload.static_style = element.static_style.clone();
        payload.style_binding = element
            .style_binding
            .as_ref()
            .map(|binding| self.register_expr(binding, element.span));
        payload.model = element
            .model
            .as_ref()
            .map(|model| vuec_ast::Vue2ComponentModel {
                value: self.register_expr(&model.value, element.span),
                callback: self.register_stmt(&model.callback, element.span),
                expression: model.expression.clone(),
            });
        payload.wrap_data = element.wrap_data.as_ref().map(|wrap| match wrap {
            Vue2DataWrap::Bind { value, prop, sync } => vuec_ast::Vue2DataWrap::Bind {
                value: self.register_expr(value, element.span),
                prop: *prop,
                sync: *sync,
            },
        });
        payload.wrap_listeners = element.wrap_listeners.clone();
        payload.validate = element
            .validate
            .as_ref()
            .map(|validate| vuec_ast::Vue2Validation {
                field: validate.field.clone(),
                groups: validate.groups.clone(),
            });
        payload.validators = element
            .validators
            .iter()
            .map(|validator| vuec_ast::Vue2Validator {
                name: validator.name.clone(),
                rule: validator.rule.clone(),
            })
            .collect();
        payload.static_node = element.static_node;
        payload.static_root = element.static_root;
        payload.static_in_for = element.static_in_for;
        payload
    }

    fn project_directive(&mut self, directive: &Vue2Directive) -> vuec_ast::Vue2Directive {
        vuec_ast::Vue2Directive {
            name: directive.name.clone(),
            raw_name: directive.raw_name.clone(),
            value: directive
                .value
                .as_ref()
                .map(|value| self.register_expr(value, directive.span)),
            arg: directive.arg.clone(),
            is_dynamic_arg: directive.is_dynamic_arg,
            modifiers: directive.modifiers.clone(),
        }
    }

    fn project_events(
        &mut self,
        events: &BTreeMap<String, Vec<Vue2EventHandler>>,
    ) -> BTreeMap<String, Vec<vuec_ast::Vue2EventHandler>> {
        events
            .iter()
            .map(|(name, handlers)| {
                (
                    name.clone(),
                    handlers
                        .iter()
                        .map(|handler| vuec_ast::Vue2EventHandler {
                            value: self.register_stmt(&handler.value, handler.span),
                            modifiers: handler.modifiers.clone(),
                            modifier_order: handler.modifier_order.clone(),
                            has_modifier_object: handler.has_modifier_object,
                            dynamic: handler.dynamic,
                            span: handler.span,
                        })
                        .collect(),
                )
            })
            .collect()
    }

    fn project_filter_expression(
        &mut self,
        raw_text: &str,
        span: Option<Span>,
    ) -> Option<vuec_ast::Vue2FilterExpr> {
        let source = single_default_interpolation(raw_text)?;
        let parsed = parse_vue2_filter_expression(source);
        if parsed.filters.is_empty() {
            return None;
        }
        Some(vuec_ast::Vue2FilterExpr {
            raw: source.to_string(),
            base: self.register_expr(parsed.base, span),
            filters: parsed
                .filters
                .into_iter()
                .map(|filter| vuec_ast::Vue2FilterCall {
                    name: filter.name.to_string(),
                    args: filter
                        .args
                        .into_iter()
                        .map(|arg| self.register_expr(arg, span))
                        .collect(),
                })
                .collect(),
        })
    }

    fn register_expr(&mut self, source: &str, span: Option<Span>) -> JsExprId {
        self.js
            .register_expr(source, js_span(span), self.source_type)
    }

    fn register_stmt(&mut self, source: &str, span: Option<Span>) -> JsStmtId {
        self.js
            .register_stmt(source, js_span(span), self.source_type)
    }

    fn register_pattern(&mut self, source: &str, span: Option<Span>) -> JsPatternId {
        self.js
            .register_pattern(source, js_span(span), self.source_type)
    }
}

fn project_attribute(attr: &Vue2Attribute) -> vuec_ast::Vue2Attribute {
    vuec_ast::Vue2Attribute {
        name: attr.name.clone(),
        value: attr.value.clone(),
        span: attr.span,
        dynamic: attr.dynamic,
    }
}
