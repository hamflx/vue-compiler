impl<'a> Vue3DomMirCodegen<'a> {
    fn render_if(
        &self,
        node_id: NodeId,
        condition: Option<JsExprId>,
        scope: &RenderScope,
    ) -> String {
        let Some(node) = self.mir.node(node_id) else {
            return "null".into();
        };
        let mut branch_ids = Vec::new();
        let mut alternate = None;
        for child_id in &node.children {
            if alternate.is_none()
                && matches!(
                    self.mir.node(*child_id).map(|child| &child.kind),
                    Some(Vue3DomMirKind::If { .. })
                )
            {
                alternate = Some(*child_id);
            } else {
                branch_ids.push(*child_id);
            }
        }
        let rendered = branch_ids
            .iter()
            .filter_map(|child_id| {
                self.render_node(*child_id, Vue3DomMirRenderMode::Root, scope)
            })
            .collect::<Vec<_>>();
        let branch = match rendered.as_slice() {
            [] => "null".into(),
            [single] => single.clone(),
            _ => render_array(&rendered),
        };
        let Some(condition) = condition else {
            return branch;
        };
        let condition = self.render_js_expr(condition, scope);
        let alternate = alternate
            .and_then(|alternate| {
                self.render_node(alternate, Vue3DomMirRenderMode::Root, scope)
            })
            .unwrap_or_else(|| "_createCommentVNode(\"v-if\", true)".into());
        format!(
            "{}\n  ? {}\n  : {}",
            render_condition(&condition, self.options),
            indent_after_first_line(&branch, 4),
            indent_after_first_line(&alternate, 4)
        )
    }

    fn render_for(&self, node_id: NodeId, for_mir: &Vue3ForMir, scope: &RenderScope) -> String {
        let source = self.render_js_expr(for_mir.source, scope);
        let child_scope = self.scope_with_for_mir(scope, for_mir);
        let children = self.render_children(node_id, Vue3DomMirRenderMode::Root, &child_scope);
        let body = match children.as_slice() {
            [] => "null".into(),
            [single] => single.clone(),
            _ => render_array(&children),
        };
        let params = self.render_for_params(for_mir);
        if let Some(memo) = &for_mir.memo {
            return self.render_memo_for(
                &source,
                &params,
                for_mir.key.as_ref(),
                for_mir.branch_key,
                memo,
                &body,
                &child_scope,
            );
        }
        let fragment_flag = if for_mir.key.is_some() {
            "128 /* KEYED_FRAGMENT */"
        } else {
            "256 /* UNKEYED_FRAGMENT */"
        };
        let fragment_props = for_mir
            .branch_key
            .map(|key| format!("{{ key: {key} }}"))
            .unwrap_or_else(|| "null".into());
        format!(
            "(_openBlock(true), _createElementBlock(_Fragment, {fragment_props}, _renderList({source}, ({params}) => {{\n  return {}\n}}), {fragment_flag}))",
            indent_after_first_line(&body, 2)
        )
    }

    fn render_for_params(&self, for_mir: &Vue3ForMir) -> String {
        let mut params = vec![self.render_js_pattern(for_mir.value_alias)];
        if let Some(key) = for_mir.key_alias {
            params.push(self.render_js_pattern(key));
        }
        if let Some(index) = for_mir.index_alias {
            params.push(self.render_js_pattern(index));
        }
        params.join(", ")
    }

    fn render_memo_for(
        &self,
        source: &str,
        params: &str,
        key: Option<&MirExpr>,
        branch_key: Option<u32>,
        memo: &Vue3ForMemo,
        body: &str,
        scope: &RenderScope,
    ) -> String {
        let params = format!("{params}, __, ___, _cached");
        let memo_expression = self.render_js_expr(memo.expression, scope);
        let guard = key.map_or_else(
            || "_cached && _cached.el && _isMemoSame(_cached, _memo)".into(),
            |key| {
                format!(
                    "_cached && _cached.el && _cached.key === {} && _isMemoSame(_cached, _memo)",
                    self.render_mir_expr(key, scope)
                )
            },
        );
        let fragment_props = branch_key
            .map(|key| format!("{{ key: {key} }}"))
            .unwrap_or_else(|| "null".into());
        format!(
            "(_openBlock(true), _createElementBlock(_Fragment, {fragment_props}, _renderList({source}, ({params}) => {{\n  const _memo = ({memo_expression})\n  if ({guard}) return _cached\n  const _item = {}\n  _item.memo = _memo\n  return _item\n}}, _cache, {}), 128 /* KEYED_FRAGMENT */))",
            indent_after_first_line(body, 2),
            memo.index
        )
    }

}
