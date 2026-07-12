/// Stateful Vue 2 compiler facade.
pub struct Vue2Compiler {
    js: JsAstStore,
}

impl Vue2Compiler {
    /// Creates a new Vue 2 compiler facade.
    pub fn new() -> Self {
        Self {
            js: JsAstStore::new(),
        }
    }

    /// Parses, optimizes, and generates a Vue 2 template.
    pub fn compile(&self, template: &str, options: Vue2CompileOptions) -> Vue2CompiledResult {
        let source = template;
        let leading_space_len = source.len() - source.trim_start().len();
        let template = source.trim();
        let mut diagnostics = DiagnosticSink::default();
        let mut element_ast = parse_element_tree(&mut diagnostics, template, &options);
        sync_scoped_slot_if_conditions(element_ast.as_mut());
        collect_element_warnings(element_ast.as_ref(), &options, &mut diagnostics);
        if options.optimize {
            if let Some(root) = element_ast.as_mut() {
                optimize(root, &options);
            }
        }
        let mut static_render_fns = Vec::new();
        validate_expressions(element_ast.as_ref(), &self.js, &mut diagnostics);
        let projection = project_public_ast(template, element_ast.as_ref());
        let lowered = lower_vue2_ast_to_mir(&projection.ast, projection.js);
        let render =
            generate_render_mir(&lowered.mir, &lowered.js, &options, &mut static_render_fns);
        let ast = projection.ast;
        let diagnostics_messages = diagnostics
            .as_slice()
            .iter()
            .map(render_diagnostic_message)
            .collect();
        let (errors, tips) = split_compilation_issues(&diagnostics, source, leading_space_len);
        Vue2CompiledResult {
            ast,
            element_ast,
            render,
            static_render_fns,
            errors,
            tips,
            diagnostics: diagnostics_messages,
        }
    }

    /// Compiles a template into official-style function result fields.
    pub fn compile_to_functions(
        &self,
        template: &str,
        options: Vue2CompileOptions,
    ) -> Vue2FunctionResult {
        let compiled = self.compile(template, options);
        Vue2FunctionResult {
            render: compiled.render,
            static_render_fns: compiled.static_render_fns,
            warnings: compiled.tips,
            errors: compiled.diagnostics,
        }
    }

    /// Compiles a template for the Vue 2 SSR render entry shape.
    pub fn compile_ssr(&self, template: &str, options: Vue2CompileOptions) -> Vue2CompiledResult {
        let mut compiled = self.compile(template, options);
        compiled.render = format!(
            "function ssrRender(_ctx, _push, _parent, _attrs){{return {}}}",
            compiled.render
        );
        compiled
    }

    /// Generates render code from an existing Vue 2 element tree.
    pub fn generate(
        &self,
        element: Option<&Vue2Element>,
        options: &Vue2CompileOptions,
    ) -> Vue2CodegenResult {
        generate(element, options)
    }

    /// Projects the compatibility element tree into canonical Vue 2 AST.
    pub fn project_ast(
        &self,
        template: &str,
        element: Option<&Vue2Element>,
    ) -> Vue2AstProjectionResult {
        project_vue2_public_ast(template, element)
    }

    /// Lowers canonical Vue 2 AST to shared HIR and Vue 2 target MIR.
    pub fn lower_to_mir(ast: &Vue2Ast, js: JsAstStore) -> Vue2LoweringResult {
        lower_vue2_ast_to_mir(ast, js)
    }

    /// Returns the JavaScript side store used by this compiler.
    pub fn js(&self) -> &JsAstStore {
        &self.js
    }
}

impl Default for Vue2Compiler {
    fn default() -> Self {
        Self::new()
    }
}

/// Parses, optimizes, and generates a Vue 2 template.
pub fn compile(template: &str, options: Vue2CompileOptions) -> Vue2CompiledResult {
    Vue2Compiler::new().compile(template, options)
}

/// Compiles a template into official-style function result fields.
pub fn compile_to_functions(template: &str, options: Vue2CompileOptions) -> Vue2FunctionResult {
    Vue2Compiler::new().compile_to_functions(template, options)
}

/// Compiles a template for the Vue 2 SSR render entry shape.
pub fn compile_ssr(template: &str, options: Vue2CompileOptions) -> Vue2CompiledResult {
    Vue2Compiler::new().compile_ssr(template, options)
}

/// Generates render code from an existing Vue 2 element tree.
pub fn generate(element: Option<&Vue2Element>, options: &Vue2CompileOptions) -> Vue2CodegenResult {
    let mut static_render_fns = Vec::new();
    let projected = project_public_ast("", element);
    let lowered = lower_vue2_ast_to_mir(&projected.ast, projected.js);
    let render = generate_render_mir(&lowered.mir, &lowered.js, options, &mut static_render_fns);
    Vue2CodegenResult {
        render,
        static_render_fns,
    }
}

/// Projects a Vue 2 compatibility parser tree into the canonical arena AST.
pub fn project_vue2_public_ast(
    template: &str,
    element_ast: Option<&Vue2Element>,
) -> Vue2AstProjectionResult {
    project_public_ast(template, element_ast)
}

/// Lowers canonical Vue 2 AST to shared HIR and Vue 2 target MIR.
pub fn lower_vue2_ast_to_mir(ast: &Vue2Ast, js: JsAstStore) -> Vue2LoweringResult {
    let root_span = ast
        .root_node()
        .map(|node| node.span.clone())
        .unwrap_or_else(|| NodeSpan::missing(MissingSpanReason::LoweringGap));
    let mut state = Vue2LoweringState {
        hir: Hir::new(HirNodeKind::Root(vuec_ast::HirRoot), root_span.clone()),
        mir: Vue2Mir::new(Vue2MirKind::Root(vuec_ast::Vue2MirRoot), root_span),
        map: LoweringMap::default(),
        js,
        static_render_index: 0,
        once_id: 0,
        suppress_static_once_for: None,
    };
    state.map.record_ast_to_hir(ast.root, state.hir.root);
    state.map.record_hir_to_mir(state.hir.root, state.mir.root);

    if let Some(root) = ast.root_node() {
        lower_vue2_child_sequence(
            &root.children,
            ast,
            state.hir.root,
            state.mir.root,
            &mut state,
        );
    }

    Vue2LoweringResult {
        hir: state.hir,
        mir: state.mir,
        map: state.map,
        js: state.js,
    }
}

/// Marks static nodes and static roots in a Vue 2 element tree.
pub fn optimize(root: &mut Vue2Element, options: &Vue2CompileOptions) {
    mark_static_element(root, options);
    mark_static_roots(root, false);
}

/// Generates a Vue 2 style source code frame for a byte range.
pub fn generate_code_frame(source: &str, start: usize, end: usize) -> String {
    let source_lines = source
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .collect::<Vec<_>>();
    let mut count = 0usize;
    let mut rendered = Vec::new();
    for (index, line) in source_lines.iter().enumerate() {
        count += line.len() + 1;
        if count < start {
            continue;
        }

        let mut output_index = index.saturating_sub(2);
        while output_index <= index + 2 || end > count {
            let Some(output_line) = source_lines.get(output_index) else {
                output_index += 1;
                continue;
            };
            rendered.push(format!(
                "{}{}|  {}",
                output_index + 1,
                " ".repeat(3usize.saturating_sub((output_index + 1).to_string().len())),
                output_line
            ));
            let line_len = output_line.len();
            if output_index == index {
                let pad = start.saturating_sub(count - line.len() - 1);
                let width = if end > count {
                    line_len.saturating_sub(pad)
                } else {
                    end.saturating_sub(start)
                };
                rendered.push(format!("   |  {}{}", " ".repeat(pad), "^".repeat(width)));
            } else if output_index > index {
                if end > count {
                    let width = (end - count).min(line_len);
                    rendered.push(format!("   |  {}", "^".repeat(width)));
                }
                count += line_len + 1;
            }
            output_index += 1;
        }
        break;
    }
    rendered.join("\n")
}
