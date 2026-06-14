impl<'a> Vue3SsrMirCodegen<'a> {
    fn render_teleport(
        &self,
        node_id: NodeId,
        teleport: &Vue3SsrTeleport,
        scope: &RenderScope,
        writer: &mut CodeWriter,
    ) {
        writer.push_line("_ssrRenderTeleport(_push, (_push) => {");
        writer.indent();
        self.render_children(node_id, scope, writer);
        writer.dedent();
        writer.push_line(&format!(
            "}}, {}, {}, _parent)",
            self.render_mir_expr(&teleport.target, scope),
            self.render_mir_expr(&teleport.disabled, scope)
        ));
    }

    fn render_suspense(
        &self,
        suspense: &Vue3SsrSuspense,
        scope: &RenderScope,
        writer: &mut CodeWriter,
    ) {
        writer.push_line("_ssrRenderSuspense(_push, {");
        writer.indent();
        for slot in &suspense.slots.slots {
            self.render_suspense_slot(slot, scope, writer);
        }
        writer.push_line(&format!(
            "_: {}",
            vue3_slot_flag_with_comment(suspense.slots.flag)
        ));
        writer.dedent();
        writer.push_line("})");
    }

    fn render_suspense_slot(
        &self,
        slot: &vuec_ast::Vue3DomSlot,
        scope: &RenderScope,
        writer: &mut CodeWriter,
    ) {
        let child_scope = slot
            .params
            .map(|params| self.scope_with_pattern(scope, params))
            .unwrap_or_else(|| scope.clone());
        writer.push_line(&format!("{}: () => {{", json_key(&slot.name)));
        writer.indent();
        let root_attrs = self.ssr_css_vars_only_root_attrs();
        self.render_child_slice(
            &slot.children,
            &child_scope,
            None,
            root_attrs.as_ref(),
            writer,
        );
        writer.dedent();
        writer.push_line("},");
    }

    fn render_slot(
        &self,
        slot: &vuec_ast::Vue3SsrSlot,
        scope: &RenderScope,
        writer: &mut CodeWriter,
    ) {
        let props = self.render_slot_props(&slot.props, scope);
        let fallback = if slot.fallback.is_empty() {
            "null".to_string()
        } else {
            let mut fallback = CodeWriter::new();
            fallback.push_line("() => {");
            fallback.indent();
            let mut index = 0usize;
            while index < slot.fallback.len() {
                if let Some((html, next_index)) =
                    self.render_ssr_template_literal_slice(&slot.fallback, index, scope, None, None)
                {
                    fallback.push_line(&format!("_push({html})"));
                    index = next_index;
                    continue;
                }
                self.render_node(slot.fallback[index], scope, None, &mut fallback);
                index += 1;
            }
            fallback.dedent();
            fallback.push_line("}");
            fallback.finish().trim_end().to_string()
        };
        let scope_id = self.render_slot_scope_id_arg(slot, scope);
        let helper = if slot.inner {
            "_ssrRenderSlotInner"
        } else {
            "_ssrRenderSlot"
        };
        let mut args = vec![
            "_ctx.$slots".to_string(),
            self.render_slot_name(&slot.name, scope),
            props,
            fallback,
            "_push".into(),
            "_parent".into(),
        ];
        if let Some(scope_id) = scope_id {
            args.push(scope_id);
        }
        if slot.inner {
            if args.len() == 6 {
                args.push("null".into());
            }
            args.push("true".into());
        }
        writer.push_line(&format!("{}({})", helper, args.join(", ")));
    }

    fn render_slot_scope_id_arg(
        &self,
        slot: &vuec_ast::Vue3SsrSlot,
        scope: &RenderScope,
    ) -> Option<String> {
        if !self.options.slotted {
            return None;
        }
        let local_scope = scope
            .locals
            .iter()
            .any(|local| local == "_scopeId")
            .then_some("_scopeId");
        let scope_id = self
            .options
            .scope_id
            .as_ref()
            .map(|scope_id| format!("{}-s", scope_id));
        match (scope_id, local_scope) {
            (Some(scope_id), Some(local_scope)) => {
                Some(format!("{} + {}", quote_string(&scope_id), local_scope))
            }
            (Some(scope_id), None) => Some(quote_string(&scope_id)),
            (None, Some(local_scope)) if self.render_slot_as_vnode_fallback(slot) => {
                Some(local_scope.to_string())
            }
            (None, Some(local_scope)) if slot.inner => Some(local_scope.to_string()),
            _ => None,
        }
    }

}
