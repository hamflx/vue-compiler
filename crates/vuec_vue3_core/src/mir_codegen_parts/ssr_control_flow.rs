impl<'a> Vue3SsrMirCodegen<'a> {
    fn render_if(
        &self,
        node_id: NodeId,
        condition: Option<JsExprId>,
        comment: bool,
        scope: &RenderScope,
        root_attrs: Option<&SsrRootAttrs>,
        writer: &mut CodeWriter,
    ) {
        let Some(condition) = condition else {
            self.render_if_branch_children(node_id, scope, root_attrs, writer);
            return;
        };
        writer.push_line(&format!(
            "if ({}) {{",
            self.render_js_expr(condition, scope)
        ));
        writer.indent();
        let alternate = self.render_if_branch_children(node_id, scope, root_attrs, writer);
        writer.dedent();
        if let Some(alternate) = alternate {
            self.render_if_alternate(alternate, scope, root_attrs, writer);
        } else if comment {
            writer.push_line("} else {");
            writer.indent();
            writer.push_line("_push(`<!---->`)");
            writer.dedent();
            writer.push_line("}");
        } else {
            writer.push_line("}");
        }
    }

    fn render_if_branch_children(
        &self,
        node_id: NodeId,
        scope: &RenderScope,
        root_attrs: Option<&SsrRootAttrs>,
        writer: &mut CodeWriter,
    ) -> Option<NodeId> {
        let node = self.mir.node(node_id)?;
        let (primary_children, alternate) = self.split_if_children(&node.children);
        let branch_root_attrs = self.root_attrs_for_branch_children(&primary_children, root_attrs);
        let scope_id = scope
            .locals
            .iter()
            .any(|local| local == "_scopeId")
            .then_some("_scopeId");
        self.render_child_slice(
            &primary_children,
            scope,
            scope_id,
            branch_root_attrs.as_ref(),
            writer,
        );
        alternate
    }

    fn render_if_alternate(
        &self,
        alternate: NodeId,
        scope: &RenderScope,
        root_attrs: Option<&SsrRootAttrs>,
        writer: &mut CodeWriter,
    ) {
        let Some(node) = self.mir.node(alternate) else {
            writer.push_line("} else {");
            writer.indent();
            writer.push_line("_push(`<!---->`)");
            writer.dedent();
            writer.push_line("}");
            return;
        };
        match node.kind {
            Vue3SsrMirKind::If {
                condition: Some(condition),
                comment,
            } => {
                writer.push_line(&format!(
                    "}} else if ({}) {{",
                    self.render_js_expr(condition, scope)
                ));
                writer.indent();
                let nested_alternate =
                    self.render_if_branch_children(alternate, scope, root_attrs, writer);
                writer.dedent();
                if let Some(nested_alternate) = nested_alternate {
                    self.render_if_alternate(nested_alternate, scope, root_attrs, writer);
                } else if comment {
                    writer.push_line("} else {");
                    writer.indent();
                    writer.push_line("_push(`<!---->`)");
                    writer.dedent();
                    writer.push_line("}");
                } else {
                    writer.push_line("}");
                }
            }
            Vue3SsrMirKind::If {
                condition: None, ..
            } => {
                writer.push_line("} else {");
                writer.indent();
                self.render_if_branch_children(alternate, scope, root_attrs, writer);
                writer.dedent();
                writer.push_line("}");
            }
            _ => {
                writer.push_line("} else {");
                writer.indent();
                self.render_node(alternate, scope, root_attrs, writer);
                writer.dedent();
                writer.push_line("}");
            }
        }
    }

    fn split_if_children(&self, children: &[NodeId]) -> (Vec<NodeId>, Option<NodeId>) {
        let mut alternate = None;
        let mut primary_children = Vec::new();
        for child_id in children {
            if matches!(
                self.mir.node(*child_id).map(|child| &child.kind),
                Some(Vue3SsrMirKind::If { .. })
            ) && alternate.is_none()
            {
                alternate = Some(*child_id);
                continue;
            }
            primary_children.push(*child_id);
        }
        (primary_children, alternate)
    }

    fn render_for(
        &self,
        node_id: NodeId,
        for_mir: &Vue3SsrFor,
        scope: &RenderScope,
        writer: &mut CodeWriter,
    ) {
        if for_mir.fragment {
            writer.push_line("_push(`<!--[-->`)");
        }
        self.render_for_list_with_optional_fragment(node_id, for_mir, scope, writer);
        if for_mir.fragment {
            writer.push_line("_push(`<!--]-->`)");
        }
    }

    fn render_for_list_with_optional_fragment(
        &self,
        node_id: NodeId,
        for_mir: &Vue3SsrFor,
        scope: &RenderScope,
        writer: &mut CodeWriter,
    ) {
        let Some(node) = self.mir.node(node_id) else {
            return;
        };
        if matches!(
            node.children.as_slice(),
            [child]
                if matches!(
                    self.mir.node(*child).map(|node| &node.kind),
                    Some(Vue3SsrMirKind::If { .. })
                )
        ) {
            self.render_for_list_with_fragment(node_id, for_mir, scope, writer);
        } else {
            self.render_for_list(node_id, for_mir, scope, writer);
        }
    }

    fn render_for_list_with_fragment(
        &self,
        node_id: NodeId,
        for_mir: &Vue3SsrFor,
        scope: &RenderScope,
        writer: &mut CodeWriter,
    ) {
        let source = self.render_js_expr(for_mir.source, scope);
        let params = self.render_for_params(for_mir);
        let child_scope = self.scope_with_for_mir(scope, for_mir);
        writer.push_line(&format!("_ssrRenderList({source}, ({params}) => {{"));
        writer.indent();
        writer.push_line("_push(`<!--[-->`)");
        let children = self
            .mir
            .node(node_id)
            .map(|node| node.children.clone())
            .unwrap_or_default();
        let scope_id = scope
            .locals
            .iter()
            .any(|local| local == "_scopeId")
            .then_some("_scopeId");
        self.render_child_slice(&children, &child_scope, scope_id, None, writer);
        writer.push_line("_push(`<!--]-->`)");
        writer.dedent();
        writer.push_line("})");
    }

    fn render_for_list(
        &self,
        node_id: NodeId,
        for_mir: &Vue3SsrFor,
        scope: &RenderScope,
        writer: &mut CodeWriter,
    ) {
        let source = self.render_js_expr(for_mir.source, scope);
        let params = self.render_for_params(for_mir);
        let child_scope = self.scope_with_for_mir(scope, for_mir);
        writer.push_line(&format!("_ssrRenderList({source}, ({params}) => {{"));
        writer.indent();
        let children = self
            .mir
            .node(node_id)
            .map(|node| node.children.clone())
            .unwrap_or_default();
        let scope_id = scope
            .locals
            .iter()
            .any(|local| local == "_scopeId")
            .then_some("_scopeId");
        self.render_child_slice(&children, &child_scope, scope_id, None, writer);
        writer.dedent();
        writer.push_line("})");
    }

    fn render_for_params(&self, for_mir: &Vue3SsrFor) -> String {
        let mut params = vec![self.render_js_pattern(for_mir.value_alias)];
        if let Some(key) = for_mir.key_alias {
            params.push(self.render_js_pattern(key));
        }
        if let Some(index) = for_mir.index_alias {
            params.push(self.render_js_pattern(index));
        }
        params.join(", ")
    }

}
