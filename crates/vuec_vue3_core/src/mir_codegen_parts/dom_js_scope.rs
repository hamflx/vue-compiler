impl<'a> Vue3DomMirCodegen<'a> {
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

    fn raw_js_expr(&self, id: JsExprId) -> Option<&str> {
        self.js
            .expressions()
            .get(id.0 as usize)
            .map(|entry| entry.source.trim())
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

    fn scope_with_for_mir(&self, scope: &RenderScope, for_mir: &Vue3ForMir) -> RenderScope {
        let mut locals = self.pattern_locals(for_mir.value_alias);
        if let Some(key) = for_mir.key_alias {
            locals.extend(self.pattern_locals(key));
        }
        if let Some(index) = for_mir.index_alias {
            locals.extend(self.pattern_locals(index));
        }
        scope.with_locals(locals)
    }

    fn scope_with_for_slot(
        &self,
        scope: &RenderScope,
        slot: &vuec_ast::Vue3DomForSlot,
    ) -> RenderScope {
        let mut locals = self.pattern_locals(slot.value_alias);
        if let Some(key) = slot.key_alias {
            locals.extend(self.pattern_locals(key));
        }
        if let Some(index) = slot.index_alias {
            locals.extend(self.pattern_locals(index));
        }
        scope.with_locals(locals)
    }

    fn scope_with_pattern(&self, scope: &RenderScope, pattern: JsPatternId) -> RenderScope {
        scope.with_locals(self.pattern_locals(pattern))
    }

    fn pattern_locals(&self, pattern: JsPatternId) -> Vec<String> {
        extract_v_for_alias_locals(&self.render_js_pattern(pattern))
    }
}
