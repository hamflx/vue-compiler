impl<'a> Vue3SsrMirCodegen<'a> {
    fn is_compiler_root_static_attr(&self, name: &str) -> bool {
        self.options
            .scope_id
            .as_deref()
            .is_some_and(|scope_id| scope_id == name)
    }

    fn render_static_attr_tail(&self, entries: &[(String, Option<String>)]) -> String {
        let mut rendered = String::new();
        for (name, value) in entries {
            rendered.push(' ');
            rendered.push_str(name);
            if let Some(value) = value {
                rendered.push_str("=\"");
                rendered.push_str(value);
                rendered.push('"');
            }
        }
        rendered
    }

    fn component_declarations(&self) -> Vec<String> {
        let mut components = Vec::<String>::new();
        for node in &self.mir.nodes {
            let Vue3SsrMirKind::RenderComponent(component) = &node.kind else {
                continue;
            };
            if component.dynamic {
                continue;
            }
            let MirExpr::String(tag) = &component.tag else {
                continue;
            };
            if components.iter().any(|item| item == tag) {
                continue;
            }
            components.push(tag.clone());
        }
        components
            .iter()
            .map(|tag| {
                format!(
                    "const {} = _resolveComponent({})",
                    component_asset_id(tag),
                    quote_string(tag)
                )
            })
            .collect()
    }

    fn directive_declarations(&self) -> Vec<String> {
        let mut directives = Vec::<String>::new();
        for node in &self.mir.nodes {
            let node_directives = match &node.kind {
                Vue3SsrMirKind::RenderAttrs(attrs) => attrs.directives.as_slice(),
                Vue3SsrMirKind::RenderComponent(component) => component.directives.as_slice(),
                _ => &[],
            };
            for directive in node_directives {
                if directives.iter().any(|item| item == &directive.name) {
                    continue;
                }
                directives.push(directive.name.clone());
            }
        }
        directives
            .iter()
            .map(|directive| {
                format!(
                    "const {} = _resolveDirective({})",
                    directive_asset_id(directive),
                    quote_string(directive)
                )
            })
            .collect()
    }

    fn render_function_start(&self, writer: &mut CodeWriter) {
        let args = self.render_function_args();
        if self.options.inline {
            writer.push_line(&format!("({args}) => {{"));
        } else {
            writer.push_line(&format!("function ssrRender({args}) {{"));
        }
    }

    fn render_function_args(&self) -> String {
        if self.options.inline || self.options.binding_metadata.is_empty() {
            "_ctx, _push, _parent, _attrs".into()
        } else {
            "_ctx, _push, _parent, _attrs, $props, $setup, $data, $options".into()
        }
    }

    fn use_with_block(&self) -> bool {
        !self.options.prefix_identifiers && self.options.mode != "module"
    }

    fn needs_dynamic_model_temp(&self) -> bool {
        self.mir.nodes.iter().any(|node| {
            matches!(
                &node.kind,
                Vue3SsrMirKind::RenderAttrs(attrs)
                    if attrs.directive_content || attrs.textarea_value_fallback.is_some() || matches!(
                        attrs.v_model.as_ref().map(|model| &model.kind),
                        Some(Vue3SsrModelKind::InputDynamicProps)
                    )
            )
        })
    }

    fn has_ssr_css_vars(&self) -> bool {
        self.options
            .ssr_css_vars
            .as_ref()
            .is_some_and(|value| !value.trim().is_empty())
    }

    fn render_ssr_css_vars(&self, scope: &RenderScope) -> Option<String> {
        let vars = self.options.ssr_css_vars.as_ref()?.trim();
        if vars.is_empty() {
            return None;
        }
        Some(rewrite_ssr_css_vars_expression(vars, self.options, scope))
    }

    fn root_attrs_for_children(&self, children: &[NodeId]) -> Option<SsrRootAttrs> {
        let children = self.effective_root_children(children);
        let css_vars = self.has_ssr_css_vars().then(|| "_cssVars".to_string());
        let root_spans = self.root_spans(children);
        if root_spans.len() == 1 {
            let root_span = *root_spans.first()?;
            let root = *children.get(root_span.start)?;
            if self.node_accepts_root_attrs(root) {
                return Some(SsrRootAttrs {
                    attrs: Some("_attrs".to_string()),
                    css_vars,
                    target_start: Some(root_span.start),
                });
            }
            return None;
        }
        let visible_spans = root_spans
            .iter()
            .copied()
            .filter(|span| !self.is_ssr_root_comment_span(children, *span))
            .collect::<Vec<_>>();
        if let [root_span] = visible_spans.as_slice() {
            let root = *children.get(root_span.start)?;
            if self.node_accepts_root_attrs(root) {
                return Some(SsrRootAttrs {
                    attrs: Some("_attrs".to_string()),
                    css_vars,
                    target_start: Some(root_span.start),
                });
            }
        }
        css_vars.map(|css_vars| SsrRootAttrs {
            attrs: None,
            css_vars: Some(css_vars),
            target_start: None,
        })
    }

    fn is_ssr_root_comment_span(&self, children: &[NodeId], span: SsrRootSpan) -> bool {
        children
            .get(span.start)
            .and_then(|id| self.ssr_push_string(*id))
            .is_some_and(|value| {
                value.starts_with("<!--") && value != "<!--[-->" && value != "<!--]-->"
            })
    }

    fn is_ssr_comment_or_fragment_marker_span(
        &self,
        children: &[NodeId],
        span: SsrRootSpan,
    ) -> bool {
        children
            .get(span.start)
            .and_then(|id| self.ssr_push_string(*id))
            .is_some_and(|value| value.starts_with("<!--"))
    }

    fn root_attrs_for_branch_children(
        &self,
        children: &[NodeId],
        root_attrs: Option<&SsrRootAttrs>,
    ) -> Option<SsrRootAttrs> {
        let root_attrs = root_attrs?;
        if root_attrs.attrs.is_none() {
            return Some(root_attrs.clone());
        }
        let root_spans = self.root_spans(children);
        let visible_spans = root_spans
            .iter()
            .copied()
            .filter(|span| !self.is_ssr_comment_or_fragment_marker_span(children, *span))
            .collect::<Vec<_>>();
        if let [root_span] = visible_spans.as_slice() {
            if children
                .get(root_span.start)
                .is_some_and(|root| self.node_accepts_root_attrs(*root))
            {
                let mut attrs = root_attrs.clone();
                attrs.target_start = Some(root_span.start);
                return Some(attrs);
            }
        }
        self.root_attrs_css_vars_only(root_attrs)
    }

    fn root_attrs_css_vars_only(&self, root_attrs: &SsrRootAttrs) -> Option<SsrRootAttrs> {
        root_attrs.css_vars.as_ref().map(|css_vars| SsrRootAttrs {
            attrs: None,
            css_vars: Some(css_vars.clone()),
            target_start: None,
        })
    }

    fn ssr_css_vars_only_root_attrs(&self) -> Option<SsrRootAttrs> {
        self.has_ssr_css_vars().then(|| SsrRootAttrs {
            attrs: None,
            css_vars: Some("_cssVars".to_string()),
            target_start: None,
        })
    }

    fn root_attr_node_for_children(
        &self,
        children: &[NodeId],
        root_attrs: &SsrRootAttrs,
    ) -> Option<NodeId> {
        let target_start = root_attrs.target_start?;
        let children = self.effective_root_children(children);
        let root_spans = self.root_spans(children);
        let root_span = root_spans.iter().find(|span| span.start == target_start)?;
        let attrs_index = root_span.attrs_index?;
        children.get(attrs_index).copied()
    }

    fn root_attrs_for_child_slice(
        &self,
        root_attrs: Option<&SsrRootAttrs>,
        offset: usize,
    ) -> Option<SsrRootAttrs> {
        root_attrs.and_then(|attrs| {
            let mut attrs = attrs.clone();
            if let Some(target_start) = attrs.target_start {
                attrs.target_start = Some(target_start.checked_sub(offset)?);
            }
            Some(attrs)
        })
    }

    fn root_attrs_for_render_index(
        &self,
        children: &[NodeId],
        index: usize,
        root_attrs: &SsrRootAttrs,
    ) -> Option<SsrRootAttrs> {
        if let Some(target_start) = root_attrs.target_start {
            if index == target_start
                || self.root_attrs_apply_to_render_attrs_index(children, index, target_start)
            {
                let mut attrs = root_attrs.clone();
                attrs.target_start = Some(0);
                return Some(attrs);
            }
            return None;
        }
        if root_attrs.attrs.is_none() && root_attrs.css_vars.is_some() {
            return Some(root_attrs.clone());
        }
        None
    }

    fn root_attrs_apply_to_render_attrs_index(
        &self,
        children: &[NodeId],
        index: usize,
        target_start: usize,
    ) -> bool {
        target_start.checked_add(1) == Some(index)
            && children
                .get(target_start)
                .and_then(|id| self.ssr_push_string(*id))
                .is_some_and(|value| parse_ssr_open_tag_start(value).is_some())
            && children.get(index).is_some_and(|id| {
                self.mir
                    .node(*id)
                    .is_some_and(|node| matches!(node.kind, Vue3SsrMirKind::RenderAttrs(_)))
            })
    }

    fn node_accepts_root_attrs(&self, node_id: NodeId) -> bool {
        match self.mir.node(node_id).map(|node| &node.kind) {
            Some(Vue3SsrMirKind::RenderComponent(_)) => true,
            Some(Vue3SsrMirKind::PushString(value)) => parse_ssr_open_tag_start(value).is_some(),
            Some(Vue3SsrMirKind::If { .. }) => self
                .mir
                .node(node_id)
                .is_some_and(|node| self.if_branches_accept_root_attrs(&node.children)),
            _ => false,
        }
    }

    fn if_branches_accept_root_attrs(&self, branch_ids: &[NodeId]) -> bool {
        let mut current = branch_ids;
        let mut accepts = false;
        loop {
            let (primary, alternate) = self.split_if_children(current);
            if !primary.is_empty() && self.children_accept_root_attrs(&primary) {
                accepts = true;
            }
            let Some(alternate) = alternate else {
                return accepts;
            };
            let Some(alternate_node) = self.mir.node(alternate) else {
                return accepts;
            };
            if matches!(alternate_node.kind, Vue3SsrMirKind::If { .. }) {
                current = &alternate_node.children;
                continue;
            }
            return accepts || self.node_accepts_root_attrs(alternate);
        }
    }

    fn children_accept_root_attrs(&self, children: &[NodeId]) -> bool {
        let spans = self.root_spans(children);
        let [span] = spans.as_slice() else {
            return false;
        };
        children
            .get(span.start)
            .is_some_and(|root| self.node_accepts_root_attrs(*root))
    }

}
