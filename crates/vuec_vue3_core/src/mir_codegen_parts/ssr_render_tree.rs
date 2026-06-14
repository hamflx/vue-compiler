impl<'a> Vue3SsrMirCodegen<'a> {
    fn render_children(&self, parent: NodeId, scope: &RenderScope, writer: &mut CodeWriter) {
        let Some(node) = self.mir.node(parent) else {
            return;
        };
        self.render_child_slice(&node.children, scope, None, None, writer);
    }

    fn render_root_children(&self, scope: &RenderScope, writer: &mut CodeWriter) {
        let children = self.root_children();
        let effective_children = self.effective_root_children(children);
        let root_spans = self.root_spans(effective_children);
        let root_attrs = self.root_attrs_for_children(children);
        if effective_children.len() != children.len() {
            if let Some((html, next_index)) = self.render_ssr_template_literal_slice_with_prefix(
                effective_children,
                0,
                scope,
                None,
                "<template>",
                root_attrs.as_ref(),
            ) {
                if next_index == effective_children.len() {
                    writer.push_line(&format!(
                        "_push({})",
                        append_static_to_ssr_template_literal(html, "</template>")
                    ));
                    return;
                }
                writer.push_line(&format!("_push({html})"));
                let sliced_root_attrs =
                    self.root_attrs_for_child_slice(root_attrs.as_ref(), next_index);
                let close_appended = self.render_child_slice_with_final_suffix(
                    &effective_children[next_index..],
                    scope,
                    None,
                    sliced_root_attrs.as_ref(),
                    "</template>",
                    writer,
                );
                if !close_appended {
                    writer.push_line("_push(`</template>`)");
                }
                return;
            }
            writer.push_line("_push(`<template>`)");
            self.render_child_slice(effective_children, scope, None, root_attrs.as_ref(), writer);
            writer.push_line("_push(`</template>`)");
            return;
        }
        if self.root_children_have_explicit_fragment(children) {
            self.render_child_slice(children, scope, None, root_attrs.as_ref(), writer);
        } else if self.effective_root_children(children).len() == children.len()
            && root_spans.len() > 1
        {
            let mut index = 0usize;
            if let Some((html, next_index)) = self.render_ssr_template_literal_slice_with_prefix(
                children,
                0,
                scope,
                None,
                "<!--[-->",
                root_attrs.as_ref(),
            ) {
                if next_index == children.len() {
                    writer.push_line(&format!(
                        "_push({})",
                        append_static_to_ssr_template_literal(html, "<!--]-->")
                    ));
                    return;
                }
                writer.push_line(&format!("_push({html})"));
                index = next_index;
            } else {
                writer.push_line("_push(`<!--[-->`)");
            }
            let sliced_root_attrs = self.root_attrs_for_child_slice(root_attrs.as_ref(), index);
            let close_appended = self.render_child_slice_with_final_suffix(
                &children[index..],
                scope,
                None,
                sliced_root_attrs.as_ref(),
                "<!--]-->",
                writer,
            );
            if !close_appended {
                writer.push_line("_push(`<!--]-->`)");
            }
        } else {
            self.render_child_slice(children, scope, None, root_attrs.as_ref(), writer);
        }
    }

    fn root_children_have_explicit_fragment(&self, children: &[NodeId]) -> bool {
        children
            .first()
            .and_then(|id| self.ssr_push_string(*id))
            .is_some_and(|value| value == "<!--[-->")
            && children
                .last()
                .and_then(|id| self.ssr_push_string(*id))
                .is_some_and(|value| value == "<!--]-->")
    }

    fn render_child_slice(
        &self,
        children: &[NodeId],
        scope: &RenderScope,
        scope_id_expr: Option<&str>,
        root_attrs: Option<&SsrRootAttrs>,
        writer: &mut CodeWriter,
    ) {
        let _ =
            self.render_child_slice_inner(children, scope, scope_id_expr, root_attrs, None, writer);
    }

    fn render_child_slice_with_final_suffix(
        &self,
        children: &[NodeId],
        scope: &RenderScope,
        scope_id_expr: Option<&str>,
        root_attrs: Option<&SsrRootAttrs>,
        final_suffix: &str,
        writer: &mut CodeWriter,
    ) -> bool {
        self.render_child_slice_inner(
            children,
            scope,
            scope_id_expr,
            root_attrs,
            Some(final_suffix),
            writer,
        )
    }

    fn render_child_slice_inner(
        &self,
        children: &[NodeId],
        scope: &RenderScope,
        scope_id_expr: Option<&str>,
        root_attrs: Option<&SsrRootAttrs>,
        final_suffix: Option<&str>,
        writer: &mut CodeWriter,
    ) -> bool {
        let mut index = 0usize;
        let mut appended_final_suffix = false;
        while index < children.len() {
            let current_root_attrs = root_attrs
                .and_then(|attrs| self.root_attrs_for_render_index(children, index, attrs));
            if let Some((html, next_index)) = self.render_ssr_template_literal_slice(
                children,
                index,
                scope,
                scope_id_expr,
                current_root_attrs.as_ref(),
            ) {
                let html = if next_index == children.len() {
                    if let Some(final_suffix) = final_suffix {
                        appended_final_suffix = true;
                        append_static_to_ssr_template_literal(html, final_suffix)
                    } else {
                        html
                    }
                } else {
                    html
                };
                writer.push_line(&format!("_push({html})"));
                index = next_index;
                continue;
            }
            if let Some(next_index) = self.render_ssr_static_shell_with_dynamic_descendant(
                children,
                index,
                scope,
                scope_id_expr,
                current_root_attrs.as_ref(),
                writer,
            ) {
                index = next_index;
                continue;
            }
            self.render_node(children[index], scope, current_root_attrs.as_ref(), writer);
            index += 1;
        }
        appended_final_suffix
    }

    fn render_ssr_static_shell_with_dynamic_descendant(
        &self,
        children: &[NodeId],
        index: usize,
        scope: &RenderScope,
        scope_id_expr: Option<&str>,
        root_attrs: Option<&SsrRootAttrs>,
        writer: &mut CodeWriter,
    ) -> Option<usize> {
        if let Some(next_index) = self.render_ssr_static_shell_with_single_dynamic_child(
            children,
            index,
            scope,
            scope_id_expr,
            root_attrs,
            writer,
        ) {
            return Some(next_index);
        }
        self.render_ssr_static_shell_around_nested_dynamic(
            children, index, scope, root_attrs, writer,
        )
    }

    fn render_ssr_static_shell_with_single_dynamic_child(
        &self,
        children: &[NodeId],
        index: usize,
        scope: &RenderScope,
        scope_id_expr: Option<&str>,
        root_attrs: Option<&SsrRootAttrs>,
        writer: &mut CodeWriter,
    ) -> Option<usize> {
        let open_start = self.ssr_push_string(*children.get(index)?)?;
        let (open_tag, static_entries) = parse_ssr_open_tag_start(open_start)?;
        let mut open_end = index + 1;
        let attrs = children
            .get(open_end)
            .and_then(|id| self.mir.node(*id))
            .and_then(|node| match &node.kind {
                Vue3SsrMirKind::RenderAttrs(attrs) => Some(attrs),
                _ => None,
            });
        if attrs.is_some() {
            open_end += 1;
        }
        if self.ssr_push_string(*children.get(open_end)?)? != ">" {
            return None;
        }
        let (_, end_index) = self.collect_ssr_static_element_span(children, index)?;
        let close_index = end_index.checked_sub(1)?;
        let attrs = children
            .get(index + 1)
            .and_then(|id| self.mir.node(*id))
            .and_then(|node| match &node.kind {
                Vue3SsrMirKind::RenderAttrs(attrs) => Some(attrs),
                _ => None,
            });
        let forced_attrs = attrs.is_some_and(|attrs| attrs.force_render_attrs);
        let rebuilt_attrs = attrs.is_some_and(|attrs| {
            attrs.v_show.is_none()
                && attrs.v_model.is_none()
                && self.ssr_attrs_use_rebuilt_element_attrs(attrs)
        });
        let [dynamic_id] = children.get(open_end + 1..close_index)? else {
            return None;
        };
        if !matches!(
            self.mir.node(*dynamic_id).map(|node| &node.kind),
            Some(Vue3SsrMirKind::For(_))
                | Some(Vue3SsrMirKind::If { .. })
                | Some(Vue3SsrMirKind::RenderSlot(_))
        ) {
            return None;
        }
        let open_static = if root_attrs.is_some() {
            format!("<{open_tag}")
        } else {
            open_start.to_string()
        };
        let mut open_parts = Vec::new();
        if let Some(expression) = self.dynamic_tag_name_expr(&open_tag, scope) {
            open_parts.push(SsrTemplatePart::Static("<".into()));
            open_parts.push(SsrTemplatePart::Expr(expression));
        } else if forced_attrs || rebuilt_attrs {
            open_parts.push(SsrTemplatePart::Static(format!("<{open_tag}")));
        } else {
            open_parts.push(SsrTemplatePart::Static(open_static));
        }
        let dynamic_open_tag = self.dynamic_tag_name_expr(&open_tag, scope).is_some();
        if let Some(attrs) = attrs {
            let root_attrs = root_attrs
                .filter(|root_attrs| root_attrs.attrs.is_some() || root_attrs.css_vars.is_some());
            if let Some(root_attrs) = root_attrs {
                if matches!(
                    attrs.v_model.as_ref().map(|model| &model.kind),
                    Some(Vue3SsrModelKind::InputDynamicProps)
                ) {
                    return None;
                }
                if attrs.v_show.is_some() {
                    open_parts.push(SsrTemplatePart::Expr(
                        self.render_root_element_v_show_attrs_expr_with_static(
                            attrs,
                            root_attrs,
                            scope,
                            &static_entries,
                        ),
                    ));
                } else {
                    open_parts.push(SsrTemplatePart::Expr(
                        self.render_root_element_attrs_expr_with_static(
                            attrs,
                            root_attrs,
                            scope,
                            &open_tag,
                            &static_entries,
                        ),
                    ));
                }
            } else {
                self.collect_ssr_template_attrs_for_open_tag(
                    attrs,
                    scope,
                    &open_tag,
                    dynamic_open_tag,
                    &static_entries,
                    &mut open_parts,
                )?;
            }
        } else if let Some(root_attrs) = root_attrs {
            let rendered = self.render_root_attrs_expr_with_static(root_attrs, &static_entries);
            if !rendered.is_empty() {
                open_parts.push(SsrTemplatePart::Expr(rendered));
            }
        }
        if root_attrs.is_some() || dynamic_open_tag || forced_attrs || rebuilt_attrs {
            let tail =
                self.render_static_attr_tail(&self.root_static_tail_entries(&static_entries));
            if !tail.is_empty() {
                open_parts.push(SsrTemplatePart::Static(tail));
            }
        }
        if let Some(scope_id_expr) = scope_id_expr {
            open_parts.push(SsrTemplatePart::Expr(scope_id_expr.to_string()));
        }
        let close = self.ssr_push_string(*children.get(close_index)?)?;
        open_parts.push(SsrTemplatePart::Static(">".into()));
        if matches!(
            self.mir.node(*dynamic_id).map(|node| &node.kind),
            Some(Vue3SsrMirKind::For(for_mir)) if for_mir.fragment
        ) {
            open_parts.push(SsrTemplatePart::Static("<!--[-->".into()));
        }
        let open = render_ssr_template_literal(&open_parts);
        writer.push_line(&format!("_push({open})"));
        match self.mir.node(*dynamic_id).map(|node| &node.kind)? {
            Vue3SsrMirKind::For(for_mir) => {
                self.render_for_list(*dynamic_id, for_mir, scope, writer)
            }
            Vue3SsrMirKind::If { condition, comment } => {
                self.render_if(*dynamic_id, *condition, *comment, scope, None, writer)
            }
            Vue3SsrMirKind::RenderSlot(slot) => self.render_slot(slot, scope, writer),
            _ => return None,
        }
        let close_prefix = if matches!(
            self.mir.node(*dynamic_id).map(|node| &node.kind),
            Some(Vue3SsrMirKind::For(for_mir)) if for_mir.fragment
        ) {
            "<!--]-->"
        } else {
            ""
        };
        let close = if let Some(expression) = self.dynamic_tag_name_expr(&open_tag, scope) {
            let close_parts = vec![
                SsrTemplatePart::Static(close_prefix.to_string()),
                SsrTemplatePart::Static("</".into()),
                SsrTemplatePart::Expr(expression),
                SsrTemplatePart::Static(">".into()),
            ];
            render_ssr_template_literal(&close_parts)
        } else {
            render_ssr_template_literal(&[SsrTemplatePart::Static(format!(
                "{close_prefix}{close}"
            ))])
        };
        writer.push_line(&format!("_push({close})"));
        Some(end_index)
    }

    fn render_ssr_static_shell_around_nested_dynamic(
        &self,
        children: &[NodeId],
        index: usize,
        scope: &RenderScope,
        root_attrs: Option<&SsrRootAttrs>,
        writer: &mut CodeWriter,
    ) -> Option<usize> {
        let (_, end_index) = self.collect_ssr_static_element_span(children, index)?;
        let close_index = end_index.checked_sub(1)?;
        let dynamic_count = children
            .get(index + 1..close_index)
            .unwrap_or(&[])
            .iter()
            .filter(|node_id| {
                matches!(
                    self.mir.node(**node_id).map(|node| &node.kind),
                    Some(Vue3SsrMirKind::For(_))
                        | Some(Vue3SsrMirKind::If { .. })
                        | Some(Vue3SsrMirKind::RenderSlot(_))
                )
            })
            .count();
        if dynamic_count > 1 {
            return self.render_ssr_shell_with_dynamic_children(
                children, index, end_index, scope, root_attrs, writer,
            );
        }
        let (dynamic_index, dynamic_id) =
            self.find_dynamic_descendant_in_static_shell(children.get(index..close_index)?, index)?;
        let mut prefix_parts = Vec::new();
        let prefix_end = self.collect_ssr_linear_template_parts(
            children,
            index,
            dynamic_index,
            scope,
            root_attrs,
            &mut prefix_parts,
        )?;
        if prefix_end != dynamic_index || prefix_parts.is_empty() {
            return None;
        }
        if matches!(
            self.mir.node(dynamic_id).map(|node| &node.kind),
            Some(Vue3SsrMirKind::For(for_mir)) if for_mir.fragment
        ) {
            prefix_parts.push(SsrTemplatePart::Static("<!--[-->".into()));
        }
        writer.push_line(&format!(
            "_push({})",
            render_ssr_template_literal(&prefix_parts)
        ));
        match self.mir.node(dynamic_id).map(|node| &node.kind)? {
            Vue3SsrMirKind::For(for_mir) => {
                self.render_for_list_with_optional_fragment(dynamic_id, for_mir, scope, writer)
            }
            Vue3SsrMirKind::If { condition, comment } => {
                self.render_if(dynamic_id, *condition, *comment, scope, None, writer)
            }
            Vue3SsrMirKind::RenderSlot(slot) => self.render_slot(slot, scope, writer),
            _ => return None,
        }
        let mut suffix_parts = Vec::new();
        if matches!(
            self.mir.node(dynamic_id).map(|node| &node.kind),
            Some(Vue3SsrMirKind::For(for_mir)) if for_mir.fragment
        ) {
            suffix_parts.push(SsrTemplatePart::Static("<!--]-->".into()));
        }
        let suffix_start = dynamic_index + 1;
        if suffix_start < end_index {
            let suffix_end = self.collect_ssr_linear_template_parts(
                children,
                suffix_start,
                end_index,
                scope,
                None,
                &mut suffix_parts,
            )?;
            if suffix_end != end_index {
                return None;
            }
        }
        writer.push_line(&format!(
            "_push({})",
            render_ssr_template_literal(&suffix_parts)
        ));
        Some(end_index)
    }

    fn render_ssr_shell_with_dynamic_children(
        &self,
        children: &[NodeId],
        index: usize,
        end_index: usize,
        scope: &RenderScope,
        root_attrs: Option<&SsrRootAttrs>,
        writer: &mut CodeWriter,
    ) -> Option<usize> {
        let open_end = self.render_ssr_shell_open(children, index, scope, root_attrs, writer)?;
        let close_index = end_index.checked_sub(1)?;
        let mut cursor = open_end;
        while cursor < close_index {
            self.render_node(children[cursor], scope, None, writer);
            cursor += 1;
        }
        let close = self.ssr_push_string(*children.get(close_index)?)?;
        writer.push_line(&format!(
            "_push({})",
            render_ssr_template_literal(&[SsrTemplatePart::Static(close.to_string())])
        ));
        Some(end_index)
    }

    fn render_ssr_shell_open(
        &self,
        children: &[NodeId],
        index: usize,
        scope: &RenderScope,
        root_attrs: Option<&SsrRootAttrs>,
        writer: &mut CodeWriter,
    ) -> Option<usize> {
        let open_start = self.ssr_push_string(*children.get(index)?)?;
        let (open_tag, static_entries) = parse_ssr_open_tag_start(open_start)?;
        let attrs = children
            .get(index + 1)
            .and_then(|id| self.mir.node(*id))
            .and_then(|node| match &node.kind {
                Vue3SsrMirKind::RenderAttrs(attrs) => Some(attrs),
                _ => None,
            });
        let forced_attrs = attrs.is_some_and(|attrs| attrs.force_render_attrs);
        let rebuilt_attrs = attrs.is_some_and(|attrs| {
            attrs.v_show.is_none()
                && attrs.v_model.is_none()
                && self.ssr_attrs_use_rebuilt_element_attrs(attrs)
        });
        let mut parts = Vec::new();
        if let Some(expression) = self.dynamic_tag_name_expr(&open_tag, scope) {
            parts.push(SsrTemplatePart::Static("<".into()));
            parts.push(SsrTemplatePart::Expr(expression));
        } else if root_attrs.is_some() || forced_attrs || rebuilt_attrs {
            parts.push(SsrTemplatePart::Static(format!("<{open_tag}")));
        } else {
            parts.push(SsrTemplatePart::Static(open_start.to_string()));
        }
        let mut cursor = index + 1;
        let dynamic_open_tag = self.dynamic_tag_name_expr(&open_tag, scope).is_some();
        if let Some(attrs) = attrs {
            if let Some(root_attrs) = root_attrs {
                if attrs.v_show.is_some() {
                    parts.push(SsrTemplatePart::Expr(
                        self.render_root_element_v_show_attrs_expr_with_static(
                            attrs,
                            root_attrs,
                            scope,
                            &static_entries,
                        ),
                    ));
                } else {
                    parts.push(SsrTemplatePart::Expr(
                        self.render_root_element_attrs_expr_with_static(
                            attrs,
                            root_attrs,
                            scope,
                            &open_tag,
                            &static_entries,
                        ),
                    ));
                }
            } else {
                self.collect_ssr_template_attrs_for_open_tag(
                    attrs,
                    scope,
                    &open_tag,
                    dynamic_open_tag,
                    &static_entries,
                    &mut parts,
                )?;
            }
            cursor += 1;
        } else if let Some(root_attrs) = root_attrs {
            let rendered = self.render_root_attrs_expr_with_static(root_attrs, &static_entries);
            if !rendered.is_empty() {
                parts.push(SsrTemplatePart::Expr(rendered));
            }
        }
        if root_attrs.is_some() || dynamic_open_tag || forced_attrs || rebuilt_attrs {
            let tail =
                self.render_static_attr_tail(&self.root_static_tail_entries(&static_entries));
            if !tail.is_empty() {
                parts.push(SsrTemplatePart::Static(tail));
            }
        }
        if self.ssr_push_string(*children.get(cursor)?)? != ">" {
            return None;
        }
        parts.push(SsrTemplatePart::Static(">".into()));
        writer.push_line(&format!("_push({})", render_ssr_template_literal(&parts)));
        Some(cursor + 1)
    }

    fn find_dynamic_descendant_in_static_shell(
        &self,
        slice: &[NodeId],
        base_index: usize,
    ) -> Option<(usize, NodeId)> {
        let mut found = None;
        for (offset, node_id) in slice.iter().copied().enumerate() {
            if matches!(
                self.mir.node(node_id).map(|node| &node.kind),
                Some(Vue3SsrMirKind::For(_))
                    | Some(Vue3SsrMirKind::If { .. })
                    | Some(Vue3SsrMirKind::RenderSlot(_))
            ) {
                if found.is_some() {
                    return None;
                }
                found = Some((base_index + offset, node_id));
            }
        }
        found
    }

    fn collect_ssr_linear_template_parts(
        &self,
        children: &[NodeId],
        index: usize,
        end_index: usize,
        scope: &RenderScope,
        root_attrs: Option<&SsrRootAttrs>,
        parts: &mut Vec<SsrTemplatePart>,
    ) -> Option<usize> {
        let mut cursor = index;
        while cursor < end_index {
            match self.mir.node(*children.get(cursor)?)?.kind.clone() {
                Vue3SsrMirKind::PushString(value) => {
                    if let Some((tag, static_entries)) = parse_ssr_open_tag_start(&value) {
                        if let Some(root_attrs) = root_attrs.filter(|_| cursor == index) {
                            parts.push(SsrTemplatePart::Static(format!("<{tag}")));
                            if let Some(attrs) = children
                                .get(cursor + 1)
                                .and_then(|id| self.mir.node(*id))
                                .and_then(|node| match &node.kind {
                                    Vue3SsrMirKind::RenderAttrs(attrs) => Some(attrs),
                                    _ => None,
                                })
                            {
                                if attrs.v_show.is_some() {
                                    parts.push(SsrTemplatePart::Expr(
                                        self.render_root_element_v_show_attrs_expr_with_static(
                                            attrs,
                                            root_attrs,
                                            scope,
                                            &static_entries,
                                        ),
                                    ));
                                } else {
                                    parts.push(SsrTemplatePart::Expr(
                                        self.render_root_element_attrs_expr_with_static(
                                            attrs,
                                            root_attrs,
                                            scope,
                                            &tag,
                                            &static_entries,
                                        ),
                                    ));
                                }
                                cursor += 1;
                            } else {
                                let rendered = self.render_root_attrs_expr_with_static(
                                    root_attrs,
                                    &static_entries,
                                );
                                if !rendered.is_empty() {
                                    parts.push(SsrTemplatePart::Expr(rendered));
                                }
                            }
                            let root_tail_entries = self.root_static_tail_entries(&static_entries);
                            let tail = self.render_static_attr_tail(&root_tail_entries);
                            if !tail.is_empty() {
                                parts.push(SsrTemplatePart::Static(tail));
                            }
                        } else {
                            parts.push(SsrTemplatePart::Static(value));
                        }
                    } else if value == "/>" {
                        parts.push(SsrTemplatePart::Static(">".into()));
                    } else {
                        parts.push(SsrTemplatePart::Static(value));
                    }
                }
                Vue3SsrMirKind::RenderAttrs(attrs) => {
                    self.collect_ssr_template_attrs(&attrs, scope, parts)?;
                }
                Vue3SsrMirKind::PushInterpolated(expr) => {
                    parts.push(SsrTemplatePart::Expr(format!(
                        "_ssrInterpolate({})",
                        self.render_mir_expr(&expr, scope)
                    )));
                }
                Vue3SsrMirKind::RenderContent(Vue3SsrContent::Html { expression }) => {
                    parts.push(SsrTemplatePart::Expr(format!(
                        "({}) ?? ''",
                        self.render_js_expr(expression, scope)
                    )));
                }
                Vue3SsrMirKind::RenderContent(Vue3SsrContent::Text { expression }) => {
                    parts.push(SsrTemplatePart::Expr(format!(
                        "_ssrInterpolate({})",
                        self.render_js_expr(expression, scope)
                    )));
                }
                _ => return None,
            }
            cursor += 1;
        }
        Some(cursor)
    }

    fn render_ssr_template_literal_slice(
        &self,
        children: &[NodeId],
        index: usize,
        scope: &RenderScope,
        scope_id_expr: Option<&str>,
        root_attrs: Option<&SsrRootAttrs>,
    ) -> Option<(String, usize)> {
        self.render_ssr_template_literal_slice_with_prefix(
            children,
            index,
            scope,
            scope_id_expr,
            "",
            root_attrs,
        )
    }

    fn render_ssr_template_literal_slice_with_prefix(
        &self,
        children: &[NodeId],
        index: usize,
        scope: &RenderScope,
        scope_id_expr: Option<&str>,
        prefix: &str,
        root_attrs: Option<&SsrRootAttrs>,
    ) -> Option<(String, usize)> {
        let mut parts = Vec::new();
        if !prefix.is_empty() {
            parts.push(SsrTemplatePart::Static(prefix.to_string()));
        }
        let mut dynamic = !prefix.is_empty() || scope_id_expr.is_some();
        let current_root_attrs =
            root_attrs.and_then(|attrs| self.root_attrs_for_render_index(children, index, attrs));
        let next_index = self.collect_ssr_template_node(
            children,
            index,
            scope,
            scope_id_expr,
            &mut parts,
            &mut dynamic,
            current_root_attrs.as_ref(),
        )?;
        let mut next_index = next_index;
        while next_index < children.len() {
            let continuation_root_attrs = root_attrs
                .and_then(|attrs| self.root_attrs_for_render_index(children, next_index, attrs));
            let Some(next) = self.collect_ssr_template_node(
                children,
                next_index,
                scope,
                scope_id_expr,
                &mut parts,
                &mut dynamic,
                continuation_root_attrs.as_ref(),
            ) else {
                break;
            };
            next_index = next;
        }
        let has_content = dynamic
            || parts.iter().any(|part| match part {
                SsrTemplatePart::Static(value) => !value.is_empty(),
                SsrTemplatePart::Expr(_) => true,
            });
        if !has_content {
            return None;
        }
        Some((render_ssr_template_literal(&parts), next_index))
    }

}
