impl<'a> Vue3SsrMirCodegen<'a> {
    fn render_mir_expr(&self, expr: &MirExpr, scope: &RenderScope) -> String {
        match expr {
            MirExpr::String(value) => quote_string(value),
            MirExpr::Bool(value) => value.to_string(),
            MirExpr::Null => "null".into(),
            MirExpr::JsExpr(expr) => self.render_js_expr(*expr, scope),
            MirExpr::Helper(helper) => helper_reference(*helper),
        }
    }

    fn render_js_expr(&self, id: JsExprId, scope: &RenderScope) -> String {
        self.js
            .expressions()
            .get(id.0 as usize)
            .map(|entry| rewrite_expression_with_scope(&entry.source, self.options, scope))
            .unwrap_or_else(|| "undefined".into())
    }

    fn render_js_stmt(&self, id: vuec_ast::JsStmtId, scope: &RenderScope) -> String {
        self.js
            .statements()
            .get(id.0 as usize)
            .map(|entry| rewrite_handler_expression_with_scope(&entry.source, self.options, scope))
            .unwrap_or_else(|| "undefined".into())
    }

    fn render_js_pattern(&self, id: JsPatternId) -> String {
        self.js
            .patterns()
            .get(id.0 as usize)
            .map(|entry| entry.source.to_string())
            .unwrap_or_else(|| "_item".into())
    }

    fn scope_with_pattern(&self, scope: &RenderScope, pattern: JsPatternId) -> RenderScope {
        scope.with_locals(extract_v_for_alias_locals(&self.render_js_pattern(pattern)))
    }

    fn scope_with_for_mir(&self, scope: &RenderScope, for_mir: &Vue3SsrFor) -> RenderScope {
        let mut locals = extract_v_for_alias_locals(&self.render_js_pattern(for_mir.value_alias));
        if let Some(key) = for_mir.key_alias {
            locals.extend(extract_v_for_alias_locals(&self.render_js_pattern(key)));
        }
        if let Some(index) = for_mir.index_alias {
            locals.extend(extract_v_for_alias_locals(&self.render_js_pattern(index)));
        }
        scope.with_locals(locals)
    }

    fn render_dom_for_slot_params(&self, slot: &vuec_ast::Vue3DomForSlot) -> String {
        let mut params = vec![self.render_js_pattern(slot.value_alias)];
        if let Some(key) = slot.key_alias {
            params.push(self.render_js_pattern(key));
        }
        if let Some(index) = slot.index_alias {
            if slot.key_alias.is_none() {
                params.push("_".into());
            }
            params.push(self.render_js_pattern(index));
        }
        params.join(", ")
    }

    fn scope_with_dom_for_slot(
        &self,
        scope: &RenderScope,
        slot: &vuec_ast::Vue3DomForSlot,
    ) -> RenderScope {
        let mut locals = extract_v_for_alias_locals(&self.render_js_pattern(slot.value_alias));
        if let Some(key) = slot.key_alias {
            locals.extend(extract_v_for_alias_locals(&self.render_js_pattern(key)));
        }
        if let Some(index) = slot.index_alias {
            locals.extend(extract_v_for_alias_locals(&self.render_js_pattern(index)));
        }
        scope.with_locals(locals)
    }
}
