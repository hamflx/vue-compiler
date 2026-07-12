impl<'a> Vue3DomMirCodegen<'a> {
    pub(crate) fn new(
        mir: &'a Vue3DomMir,
        js: &'a JsAstStore,
        options: &'a Vue3CompilerOptions,
    ) -> Self {
        Self { mir, js, options }
    }

    pub(crate) fn generate(self) -> CodegenResult {
        let mut writer = CodeWriter::new();
        let scope = RenderScope::default();
        let hoists = self.hoist_declarations(&scope);
        let declarations = self
            .component_declarations()
            .into_iter()
            .chain(self.directive_declarations())
            .collect::<Vec<_>>();
        let body = self.render_root_children(self.mir.root, &scope);
        let helpers = self.helpers(&format!(
            "{}\n{}\n{}",
            hoists.join("\n"),
            declarations.join("\n"),
            body
        ));
        self.render_preamble(&mut writer, &helpers, &hoists);
        self.render_render_start(&mut writer);
        writer.indent();
        let use_with = self.use_with_block();
        if use_with {
            writer.push_line("with (_ctx) {");
            writer.indent();
            if !helpers.is_empty() {
                writer.push_line(&format!("const {{ {} }} = _Vue", helper_aliases(&helpers)));
                writer.newline();
            }
        }
        for declaration in &declarations {
            writer.push_line(declaration);
        }
        if !declarations.is_empty() {
            writer.newline();
        }
        writer.push_line(&format!("return {body}"));
        if use_with {
            writer.dedent();
            writer.push_line("}");
        }
        writer.dedent();
        writer.push_line("}");
        CodegenResult {
            code: writer.finish().trim_end().to_string(),
            map: None,
            ast_summary: format!("vue3-dom-mir-nodes={}", self.mir.len()),
            diagnostics: Vec::new(),
            preamble: String::new(),
        }
    }

    fn render_preamble(
        &self,
        writer: &mut CodeWriter,
        helpers: &[RuntimeHelper],
        hoists: &[String],
    ) {
        if self.options.mode == "module" {
            if !helpers.is_empty() {
                writer.push_line(&format!(
                    "import {{ {} }} from \"vue\"",
                    import_helper_aliases(helpers)
                ));
                writer.newline();
            }
            for import in self.asset_imports() {
                writer.push_line(&format!("import {} from '{}'", import.name, import.path));
            }
            if !self.asset_imports().is_empty() {
                writer.newline();
                writer.newline();
            }
        } else if self.options.prefix_identifiers {
            if !helpers.is_empty() {
                writer.push_line(&format!("const {{ {} }} = Vue", helper_aliases(helpers)));
                writer.newline();
            }
        } else if !helpers.is_empty() {
            writer.push_line("const _Vue = Vue");
            if !hoists.is_empty() {
                let static_helpers = hoist_static_helpers(helpers);
                if !static_helpers.is_empty() {
                    writer.push_line(&format!(
                        "const {{ {} }} = _Vue",
                        helper_aliases(&static_helpers)
                    ));
                }
            }
            writer.newline();
        }
        for hoist in hoists {
            writer.push_line(hoist);
        }
        if !hoists.is_empty() {
            writer.newline();
        }
    }

    fn asset_imports(&self) -> &[vuec_ast::Vue3ImportItem] {
        self.mir
            .node(self.mir.root)
            .and_then(|node| match &node.kind {
                Vue3DomMirKind::Root(root) => Some(root.imports.as_slice()),
                _ => None,
            })
            .unwrap_or(&[])
    }

    fn render_render_start(&self, writer: &mut CodeWriter) {
        if self.options.mode == "module" {
            writer.push_line(&format!(
                "export function render({}) {{",
                render_args(self.options)
            ));
        } else {
            writer.push_line(&format!(
                "return function render({}) {{",
                render_args(self.options)
            ));
        }
    }

    fn use_with_block(&self) -> bool {
        !self.options.prefix_identifiers && self.options.mode != "module"
    }

    fn hoist_declarations(&self, scope: &RenderScope) -> Vec<String> {
        self.mir
            .nodes
            .iter()
            .filter_map(|node| match node.kind {
                Vue3DomMirKind::Hoisted { index } => {
                    Some((index, self.render_hoisted_value(node.id, scope)))
                }
                _ => None,
            })
            .map(|(index, value)| format!("const _hoisted_{index} = {value}"))
            .collect()
    }

    fn render_hoisted_value(&self, node_id: NodeId, scope: &RenderScope) -> String {
        let children = self.render_children(node_id, Vue3DomMirRenderMode::Child, scope);
        match children.as_slice() {
            [] => "null".into(),
            [single] => single.clone(),
            _ => render_array(&children),
        }
    }

    fn helpers(&self, helper_probe: &str) -> Vec<RuntimeHelper> {
        let mut helpers = Vec::new();
        for node in &self.mir.nodes {
            match &node.kind {
                Vue3DomMirKind::Root(_)
                | Vue3DomMirKind::WithDirectives
                | Vue3DomMirKind::Fragment => {}
                Vue3DomMirKind::VNodeCall(call) => {
                    match &call.tag {
                        Vue3DomTag::Native(_) => {}
                        Vue3DomTag::ComponentAsset(_) => {
                            push_unique_helper(&mut helpers, RuntimeHelper::Vue3ResolveComponent);
                        }
                        Vue3DomTag::DynamicComponent(_) => {
                            push_unique_helper(
                                &mut helpers,
                                RuntimeHelper::Vue3ResolveDynamicComponent,
                            );
                        }
                        Vue3DomTag::RuntimeHelper(helper) => {
                            push_unique_helper(&mut helpers, *helper);
                        }
                    }
                    if let MirChildren::Slots(slots) = &call.children {
                        push_unique_helper(&mut helpers, RuntimeHelper::Vue3WithCtx);
                        if !slots.dynamic_slots.is_empty() {
                            push_unique_helper(&mut helpers, RuntimeHelper::Vue3CreateSlots);
                        }
                        if slots
                            .dynamic_slots
                            .iter()
                            .any(|slot| matches!(slot, vuec_ast::Vue3DomDynamicSlot::For(_)))
                        {
                            push_unique_helper(&mut helpers, RuntimeHelper::Vue3RenderList);
                        }
                    }
                    self.push_prop_helpers(&call.props, &mut helpers);
                    if !call.directives.is_empty() || call.v_show.is_some() {
                        push_unique_helper(&mut helpers, RuntimeHelper::Vue3WithDirectives);
                    }
                    if !call.models.is_empty() {
                        push_unique_helper(&mut helpers, RuntimeHelper::Vue3WithDirectives);
                    }
                    if call.v_show.is_some() {
                        push_unique_helper(&mut helpers, RuntimeHelper::Vue3VShow);
                    }
                    if !call.directives.is_empty() {
                        push_unique_helper(&mut helpers, RuntimeHelper::Vue3ResolveDirective);
                    }
                    for model in &call.models {
                        push_unique_helper(&mut helpers, vue3_dom_model_runtime_helper(model.kind));
                    }
                    let is_root_call = node.parent == Some(self.mir.root) || call.is_block;
                    if is_root_call {
                        push_unique_helper(&mut helpers, RuntimeHelper::Vue3OpenBlock);
                        push_unique_helper(
                            &mut helpers,
                            if call.is_component {
                                RuntimeHelper::Vue3CreateBlock
                            } else {
                                RuntimeHelper::Vue3CreateElementBlock
                            },
                        );
                    } else {
                        push_unique_helper(
                            &mut helpers,
                            if call.is_component {
                                RuntimeHelper::Vue3CreateVNode
                            } else {
                                RuntimeHelper::Vue3CreateElementVNode
                            },
                        );
                    }
                }
                Vue3DomMirKind::TextCall { .. } => {
                    push_unique_helper(&mut helpers, RuntimeHelper::Vue3CreateTextVNode);
                }
                Vue3DomMirKind::Interpolation { .. } => {
                    push_unique_helper(&mut helpers, RuntimeHelper::Vue3CreateTextVNode);
                    push_unique_helper(&mut helpers, RuntimeHelper::Vue3ToDisplayString);
                }
                Vue3DomMirKind::If { condition } => {
                    if condition.is_some() {
                        push_unique_helper(&mut helpers, RuntimeHelper::Vue3CreateCommentVNode);
                    }
                }
                Vue3DomMirKind::For(for_mir) => {
                    push_unique_helper(&mut helpers, RuntimeHelper::Vue3OpenBlock);
                    push_unique_helper(&mut helpers, RuntimeHelper::Vue3CreateElementBlock);
                    push_unique_helper(&mut helpers, RuntimeHelper::Vue3Fragment);
                    push_unique_helper(&mut helpers, RuntimeHelper::Vue3RenderList);
                    if for_mir.memo.is_some() {
                        push_unique_helper(&mut helpers, RuntimeHelper::Vue3IsMemoSame);
                    }
                }
                Vue3DomMirKind::RenderSlot(slot) => {
                    push_unique_helper(&mut helpers, RuntimeHelper::Vue3RenderSlot);
                    self.push_prop_helpers(&slot.props, &mut helpers);
                }
                Vue3DomMirKind::Memo { .. } => {
                    push_unique_helper(&mut helpers, RuntimeHelper::Vue3WithMemo);
                }
                Vue3DomMirKind::Cache { .. } | Vue3DomMirKind::Hoisted { .. } => {}
            }
        }
        for helper in render_helpers_from_code(vue3_helper_order(false), helper_probe) {
            push_unique_helper(&mut helpers, helper);
        }
        sort_helpers_by_order(&mut helpers, vue3_helper_order(false));
        helpers
    }

}
