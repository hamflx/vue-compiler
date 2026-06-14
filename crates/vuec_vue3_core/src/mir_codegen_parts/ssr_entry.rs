impl<'a> Vue3SsrMirCodegen<'a> {
    pub(crate) fn new(
        mir: &'a Vue3SsrMir,
        js: &'a JsAstStore,
        options: &'a Vue3CompilerOptions,
    ) -> Self {
        Self { mir, js, options }
    }

    pub(crate) fn generate(self) -> CodegenResult {
        let mut writer = CodeWriter::new();
        let scope = RenderScope::default();
        let vue_helpers = self.vue_helpers();
        let ssr_helpers = self.ssr_helpers();
        let mut declarations = self.component_declarations();
        declarations.extend(self.directive_declarations());
        let mut preamble = String::new();
        self.render_preamble(&mut writer, &mut preamble, &vue_helpers, &ssr_helpers);
        self.render_function_start(&mut writer);
        writer.indent();
        if self.needs_dynamic_model_temp() {
            writer.push_line("let _temp0");
            writer.newline();
        }
        let use_with = self.use_with_block();
        if use_with {
            writer.push_line("with (_ctx) {");
            writer.indent();
        }
        for declaration in &declarations {
            writer.push_line(declaration);
        }
        if !declarations.is_empty() {
            writer.newline();
        }
        if let Some(css_vars) = self.render_ssr_css_vars(&scope) {
            writer.push_str("const _cssVars = { style: ");
            writer.push_raw(&css_vars);
            writer.push_str("}");
            writer.newline();
        }
        self.render_root_children(&scope, &mut writer);
        if use_with {
            writer.dedent();
            writer.push_line("}");
        }
        writer.dedent();
        writer.push_line("}");
        CodegenResult {
            code: self.finish_code(writer),
            map: None,
            ast_summary: format!("vue3-ssr-mir-nodes={}", self.mir.len()),
            diagnostics: Vec::new(),
            preamble,
        }
    }

    fn finish_code(&self, writer: CodeWriter) -> String {
        let code = writer.finish().trim_end().to_string();
        if self.options.inline || self.options.mode == "module" || code.starts_with("const ") {
            code
        } else {
            format!("\n{code}")
        }
    }

    fn render_preamble(
        &self,
        writer: &mut CodeWriter,
        preamble: &mut String,
        vue_helpers: &[RuntimeHelper],
        ssr_helpers: &[RuntimeHelper],
    ) {
        if self.options.inline {
            let mut inline = CodeWriter::new();
            if self.options.mode == "module" {
                if !vue_helpers.is_empty() {
                    inline.push_line(&format!(
                        "import {{ {} }} from \"vue\"",
                        import_helper_aliases(vue_helpers)
                    ));
                }
                if !ssr_helpers.is_empty() {
                    inline.push_line(&format!(
                        "import {{ {} }} from \"vue/server-renderer\"",
                        import_helper_aliases(ssr_helpers)
                    ));
                }
            } else {
                if !vue_helpers.is_empty() {
                    inline.push_line(&format!(
                        "const {{ {} }} = require(\"vue\")",
                        helper_aliases(vue_helpers)
                    ));
                }
                if !ssr_helpers.is_empty() {
                    inline.push_line(&format!(
                        "const {{ {} }} = require(\"vue/server-renderer\")",
                        helper_aliases(ssr_helpers)
                    ));
                }
                inline.push_str("return ");
            }
            let rendered = inline.finish();
            if !rendered.is_empty() {
                if self.options.mode == "module" {
                    *preamble = format!("{}\n\n", rendered.trim_end());
                } else {
                    *preamble = rendered;
                }
            }
            return;
        }
        if self.options.mode == "module" {
            if !vue_helpers.is_empty() {
                writer.push_line(&format!(
                    "import {{ {} }} from \"vue\"",
                    import_helper_aliases(vue_helpers)
                ));
            }
            if !ssr_helpers.is_empty() {
                writer.push_line(&format!(
                    "import {{ {} }} from \"vue/server-renderer\"",
                    import_helper_aliases(ssr_helpers)
                ));
            }
            if (!vue_helpers.is_empty() || !ssr_helpers.is_empty())
                && self.asset_imports().is_empty()
            {
                writer.newline();
            }
            for import in self.asset_imports() {
                writer.push_line(&format!("import {} from '{}'", import.name, import.path));
            }
            if !self.asset_imports().is_empty() {
                writer.newline();
                writer.newline();
            }
            writer.push_str("export ");
            return;
        }
        if !vue_helpers.is_empty() {
            writer.push_line(&format!(
                "const {{ {} }} = require(\"vue\")",
                helper_aliases(vue_helpers)
            ));
        }
        if !ssr_helpers.is_empty() {
            writer.push_line(&format!(
                "const {{ {} }} = require(\"vue/server-renderer\")",
                helper_aliases(ssr_helpers)
            ));
        }
        if !vue_helpers.is_empty() || !ssr_helpers.is_empty() {
            writer.newline();
        }
        writer.push_str("return ");
    }

    fn asset_imports(&self) -> &[vuec_ast::Vue3ImportItem] {
        self.mir
            .node(self.mir.root)
            .and_then(|node| match &node.kind {
                Vue3SsrMirKind::Root(root) => Some(root.imports.as_slice()),
                _ => None,
            })
            .unwrap_or(&[])
    }

    fn root_children(&self) -> &[NodeId] {
        self.mir
            .node(self.mir.root)
            .map(|node| node.children.as_slice())
            .unwrap_or(&[])
    }

    fn effective_root_children<'b>(&self, children: &'b [NodeId]) -> &'b [NodeId] {
        if children.len() < 3 {
            return children;
        }
        let has_template_open = self.ssr_push_string(children[0]).is_some_and(|value| {
            parse_ssr_open_tag_start(value)
                .is_some_and(|(tag, attrs)| tag == "template" && attrs.is_empty())
        }) && self
            .ssr_push_string(children[1])
            .is_some_and(|value| value == ">");
        let has_template_close = children
            .last()
            .and_then(|id| self.ssr_push_string(*id))
            .is_some_and(|value| value == "</template>");
        if has_template_open && has_template_close {
            &children[2..children.len() - 1]
        } else {
            children
        }
    }

    fn root_spans(&self, children: &[NodeId]) -> Vec<SsrRootSpan> {
        let mut spans = Vec::new();
        let mut index = 0usize;
        while index < children.len() {
            if let Some(next_index) = self.collect_ssr_text_span(children, index) {
                spans.push(SsrRootSpan {
                    start: index,
                    attrs_index: None,
                });
                index = next_index;
            } else if let Some((_, next_index)) =
                self.collect_ssr_static_element_span(children, index)
            {
                let attrs_index = children.get(index + 1).and_then(|id| {
                    self.mir
                        .node(*id)
                        .is_some_and(|node| matches!(node.kind, Vue3SsrMirKind::RenderAttrs(_)))
                        .then_some(index + 1)
                });
                spans.push(SsrRootSpan {
                    start: index,
                    attrs_index,
                });
                index = next_index;
            } else {
                spans.push(SsrRootSpan {
                    start: index,
                    attrs_index: None,
                });
                index += 1;
            }
        }
        spans
    }

    fn collect_ssr_text_span(&self, children: &[NodeId], index: usize) -> Option<usize> {
        let mut cursor = index;
        while cursor < children.len() && self.is_ssr_text_span_part(children[cursor]) {
            cursor += 1;
        }
        (cursor > index).then_some(cursor)
    }

    fn is_ssr_text_span_part(&self, child: NodeId) -> bool {
        match self.mir.node(child).map(|node| &node.kind) {
            Some(Vue3SsrMirKind::PushInterpolated(_)) => true,
            Some(Vue3SsrMirKind::PushString(value)) => {
                !value.starts_with("<!--")
                    && value != ">"
                    && value != "/>"
                    && !value.starts_with("</")
                    && parse_ssr_open_tag_start(value).is_none()
            }
            _ => false,
        }
    }

    fn collect_ssr_static_element_span(
        &self,
        children: &[NodeId],
        index: usize,
    ) -> Option<(String, usize)> {
        let (tag, _) = self
            .mir
            .node(*children.get(index)?)
            .and_then(|node| match &node.kind {
                Vue3SsrMirKind::PushString(value) => parse_ssr_open_tag_start(value),
                _ => None,
            })?;
        let mut cursor = index + 1;
        if children
            .get(cursor)
            .and_then(|id| self.mir.node(*id))
            .is_some_and(|node| matches!(node.kind, Vue3SsrMirKind::RenderAttrs(_)))
        {
            cursor += 1;
        }
        let close_open = self.ssr_push_string(*children.get(cursor)?)?;
        cursor += 1;
        if close_open == "/>" {
            return Some((tag, cursor));
        }
        if close_open != ">" {
            return None;
        }
        while cursor < children.len() {
            if self
                .ssr_push_string(children[cursor])
                .is_some_and(|value| value == format!("</{tag}>"))
            {
                return Some((tag, cursor + 1));
            }
            if let Some(value) = self.ssr_push_string(children[cursor]) {
                if parse_ssr_open_tag_start(value).is_some() {
                    let (_, next_index) = self.collect_ssr_static_element_span(children, cursor)?;
                    cursor = next_index;
                    continue;
                }
            }
            cursor += 1;
        }
        None
    }

    fn root_static_merge_entries(
        &self,
        entries: &[(String, Option<String>)],
    ) -> Vec<(String, Option<String>)> {
        entries
            .iter()
            .filter(|(name, _)| !self.is_compiler_root_static_attr(name))
            .cloned()
            .collect()
    }

    fn root_static_tail_entries(
        &self,
        entries: &[(String, Option<String>)],
    ) -> Vec<(String, Option<String>)> {
        if self.options.scope_id.is_none() {
            return Vec::new();
        }
        let mut tail = Vec::new();
        if !entries
            .iter()
            .any(|(name, _)| self.is_compiler_root_static_attr(name))
        {
            if let Some(scope_id) = &self.options.scope_id {
                tail.push((scope_id.clone(), None));
            }
        }
        tail.extend(
            entries
                .iter()
                .filter(|(name, _)| self.is_compiler_root_static_attr(name))
                .cloned(),
        );
        tail
    }

}
