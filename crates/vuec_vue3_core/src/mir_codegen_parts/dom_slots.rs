impl<'a> Vue3DomMirCodegen<'a> {
    fn render_slot_outlet(&self, slot: &vuec_ast::Vue3RenderSlot, scope: &RenderScope) -> String {
        let slots = if uses_prefixed_identifiers(self.options) {
            "_ctx.$slots"
        } else {
            "$slots"
        };
        let name = self.render_slot_name(&slot.name, scope);
        let props = self.render_normalized_props(
            &slot.props,
            self.render_ordered_props(&slot.props, &Vue3DomTag::Native("slot".into()), scope)
                .unwrap_or_else(|| "{}".into()),
        );
        let fallback = if slot.fallback.is_empty() {
            None
        } else {
            let rendered = slot
                .fallback
                .iter()
                .filter_map(|child_id| {
                    self.render_node(*child_id, Vue3DomMirRenderMode::Child, scope)
                })
                .collect::<Vec<_>>();
            Some(format!("() => {}", render_array(&rendered)))
        };
        let mut args = vec![slots.to_string(), name];
        if props != "{}" || fallback.is_some() {
            args.push(props);
        }
        if let Some(fallback) = fallback {
            args.push(fallback);
        }
        format!("_renderSlot({})", args.join(", "))
    }

    fn render_directive_arg(&self, directive: &Vue3DomDirective, scope: &RenderScope) -> String {
        let runtime = directive_asset_id(&directive.name);
        let mut args = vec![runtime];
        if let Some(expression) = directive.expression {
            args.push(self.render_js_expr(expression, scope));
        } else if directive.argument.is_some()
            || directive.dynamic_argument.is_some()
            || !directive.modifiers.is_empty()
        {
            args.push("void 0".into());
        }
        if let Some(argument) = &directive.argument {
            args.push(quote_string(argument));
        } else if let Some(argument) = directive.dynamic_argument {
            args.push(self.render_js_expr(argument, scope));
        } else if !directive.modifiers.is_empty() {
            args.push("void 0".into());
        }
        if !directive.modifiers.is_empty() {
            let modifiers = directive
                .modifiers
                .iter()
                .map(|modifier| format!("{}: true", json_key(modifier)))
                .collect::<Vec<_>>();
            args.push(render_object(&modifiers));
        }
        format!("[{}]", args.join(", "))
    }

    fn render_vnode_children(&self, call: &Vue3VNodeCall, scope: &RenderScope) -> Option<String> {
        match &call.children {
            MirChildren::None => None,
            MirChildren::Text(value) => Some(quote_string(value)),
            MirChildren::Slots(slots) => Some(self.render_slots(slots, scope)),
            MirChildren::Nodes(children) => {
                let rendered = children
                    .iter()
                    .filter_map(|child_id| {
                        self.render_node(*child_id, Vue3DomMirRenderMode::Child, scope)
                    })
                    .collect::<Vec<_>>();
                if matches!(
                    call.tag,
                    Vue3DomTag::RuntimeHelper(RuntimeHelper::Vue3Fragment)
                ) {
                    Some(render_array(&rendered))
                } else if rendered.is_empty() {
                    None
                } else if rendered.len() == 1 {
                    rendered.into_iter().next()
                } else {
                    Some(render_array(&rendered))
                }
            }
        }
    }

    fn render_slots(&self, slots: &vuec_ast::Vue3DomSlots, scope: &RenderScope) -> String {
        let mut properties = slots
            .slots
            .iter()
            .map(|slot| self.render_slot(slot, scope))
            .collect::<Vec<_>>();
        properties.push(format!("_: {}", vue3_slot_flag_value(slots.flag)));
        let base = render_object(&properties);
        if slots.dynamic_slots.is_empty() {
            base
        } else {
            format!(
                "_createSlots({base}, {})",
                self.render_dynamic_slots(slots, scope)
            )
        }
    }

    fn render_slot(&self, slot: &vuec_ast::Vue3DomSlot, scope: &RenderScope) -> String {
        let params = slot
            .params
            .map(|params| format!("({})", self.render_js_pattern(params)))
            .unwrap_or_else(|| "()".into());
        let child_scope = slot
            .params
            .map(|params| self.scope_with_pattern(scope, params))
            .unwrap_or_else(|| scope.clone());
        let rendered = slot
            .children
            .iter()
            .filter_map(|child_id| {
                self.render_node(*child_id, Vue3DomMirRenderMode::Child, &child_scope)
            })
            .collect::<Vec<_>>();
        let body = render_array(&rendered);
        format!("{}: _withCtx({params} => {body})", json_key(&slot.name))
    }

    fn render_dynamic_slots(&self, slots: &vuec_ast::Vue3DomSlots, scope: &RenderScope) -> String {
        let rendered = slots
            .dynamic_slots
            .iter()
            .map(|slot| self.render_dynamic_slot(slot, scope))
            .collect::<Vec<_>>();
        render_array(&rendered)
    }

    fn render_dynamic_slot(
        &self,
        slot: &vuec_ast::Vue3DomDynamicSlot,
        scope: &RenderScope,
    ) -> String {
        match slot {
            vuec_ast::Vue3DomDynamicSlot::Slot(slot) => {
                self.render_dynamic_slot_object(slot, scope)
            }
            vuec_ast::Vue3DomDynamicSlot::Conditional(slot) => {
                let condition = slot
                    .condition
                    .map(|condition| {
                        render_condition(&self.render_js_expr(condition, scope), self.options)
                    })
                    .unwrap_or_else(|| "true".into());
                let slot_object = self.render_dynamic_slot_object(&slot.slot, scope);
                let alternate = slot
                    .alternate
                    .as_deref()
                    .map(|alternate| self.render_dynamic_slot(alternate, scope))
                    .unwrap_or_else(|| "undefined".into());
                format!(
                    "{condition}\n  ? {}\n  : {}",
                    indent_after_first_line(&slot_object, 4),
                    indent_after_first_line(&alternate, 4)
                )
            }
            vuec_ast::Vue3DomDynamicSlot::For(slot) => self.render_for_slot(slot, scope),
        }
    }

    fn render_for_slot(&self, slot: &vuec_ast::Vue3DomForSlot, scope: &RenderScope) -> String {
        let source = self.render_js_expr(slot.source, scope);
        let params = self.render_for_slot_params(slot);
        let child_scope = self.scope_with_for_slot(scope, slot);
        let body = self.render_dynamic_slot_object(&slot.slot, &child_scope);
        format!(
            "_renderList({source}, ({params}) => {{\n  return {}\n}})",
            indent_after_first_line(&body, 2)
        )
    }

    fn render_for_slot_params(&self, slot: &vuec_ast::Vue3DomForSlot) -> String {
        let mut params = vec![self.render_js_pattern(slot.value_alias)];
        if let Some(key) = slot.key_alias {
            params.push(self.render_js_pattern(key));
        }
        if let Some(index) = slot.index_alias {
            params.push(self.render_js_pattern(index));
        }
        params.join(", ")
    }

    fn render_dynamic_slot_object(
        &self,
        slot: &vuec_ast::Vue3DomDynamicSlotObject,
        scope: &RenderScope,
    ) -> String {
        let params = slot
            .params
            .map(|params| format!("({})", self.render_js_pattern(params)))
            .unwrap_or_else(|| "()".into());
        let child_scope = slot
            .params
            .map(|params| self.scope_with_pattern(scope, params))
            .unwrap_or_else(|| scope.clone());
        let children = slot
            .children
            .iter()
            .filter_map(|child_id| {
                self.render_node(*child_id, Vue3DomMirRenderMode::Child, &child_scope)
            })
            .collect::<Vec<_>>();
        let body = render_array(&children);
        let mut properties = vec![
            format!("name: {}", self.render_slot_name(&slot.name, scope)),
            format!("fn: _withCtx({params} => {body})"),
        ];
        if let Some(key) = &slot.key {
            properties.push(format!("key: {}", quote_string(key)));
        }
        render_object(&properties)
    }

    fn render_slot_name(&self, name: &Vue3DomSlotName, scope: &RenderScope) -> String {
        match name {
            Vue3DomSlotName::Static(name) => quote_string(name),
            Vue3DomSlotName::Dynamic(name) => self.render_js_expr(*name, scope),
        }
    }

}
