use crate::*;

/// Vue 3 compiler-core dialect entry point.
pub struct Vue3Dialect;

impl Vue3Dialect {
    /// Parses a Vue 3 template into the canonical arena AST.
    pub fn base_parse(source: TemplateSource, options: &Vue3CompilerOptions) -> Vue3Ast {
        let interpolation_open = options
            .delimiters
            .as_ref()
            .map(|[open, _]| open.as_str())
            .unwrap_or("{{");
        let node_capacity = vuec_ast::template_node_capacity_hint_with_interpolation(
            &source.source,
            interpolation_open,
        );
        let mut ast = Vue3Ast::with_capacity(
            Vue3NodeKind::root(),
            Some(Span::new(
                source.file_id,
                source.base_offset,
                source.base_offset + source.source.len(),
            )),
            node_capacity,
        );
        let root = ast.root;
        let mut stack = vec![root];
        let mut v_pre_depth = 0usize;
        let mut malformed_start_depth = 0usize;
        let mut namespace_stack = vec![options.root_namespace];
        let mut tokenizer = if let Some([open, close]) = &options.delimiters {
            HtmlTokenizer::new(&source.source).with_interpolation_delimiters(open, close)
        } else {
            HtmlTokenizer::new(&source.source)
        };
        loop {
            if v_pre_depth > 0 {
                tokenizer.set_interpolation_delimiters("", "");
            } else if let Some([open, close]) = &options.delimiters {
                tokenizer.set_interpolation_delimiters(open, close);
            } else {
                tokenizer.set_interpolation_delimiters("{{", "}}");
            }
            let token = tokenizer.next_token();
            let eof = matches!(token.kind, HtmlTokenKind::Eof);
            let current_parent = *stack.last().unwrap_or(&root);
            let current_namespace = namespace_stack
                .last()
                .copied()
                .unwrap_or(vuec_ast::HtmlNamespace::Html);
            match token.kind {
                HtmlTokenKind::Text(text) => {
                    if malformed_start_depth > 0 {
                        extend_open_element_spans_to(
                            &mut ast,
                            &stack,
                            source.base_offset + token.end,
                        );
                        continue;
                    }
                    if v_pre_depth > 0 {
                        push_text(
                            &mut ast,
                            current_parent,
                            source.file_id,
                            source.base_offset + token.start,
                            &text,
                        );
                    } else {
                        push_text_and_interpolations(
                            &mut ast,
                            current_parent,
                            source.file_id,
                            source.base_offset + token.start,
                            &text,
                            options,
                        );
                    }
                }
                HtmlTokenKind::Comment(value) => {
                    extend_open_element_spans_to(&mut ast, &stack, source.base_offset + token.end);
                    if !options.comments {
                        continue;
                    }
                    let incomplete = source.source[token.start..].starts_with("<!--")
                        && token.end == source.source.len()
                        && !source.source[token.start..token.end].ends_with("-->");
                    if incomplete && value.is_empty() {
                        continue;
                    }
                    let comment_end = if incomplete {
                        token.end + "-->".len()
                    } else {
                        token.end
                    };
                    let _id = ast.push_child(
                        current_parent,
                        Vue3NodeKind::comment(value),
                        Some(Span::new(
                            source.file_id,
                            source.base_offset + token.start,
                            source.base_offset + comment_end,
                        )),
                    );
                }
                HtmlTokenKind::StartTag {
                    name,
                    attributes,
                    self_closing,
                } => {
                    let incomplete =
                        vue3_start_tag_is_incomplete(&source.source, token.start, token.end);
                    if incomplete && token.end == source.source.len() {
                        push_vue3_parser_diagnostic(
                            &mut ast,
                            Vue3ErrorCode::EofInTag,
                            source.file_id,
                            source.base_offset + token.end,
                        );
                    }
                    if incomplete
                        && token.end == source.source.len()
                        && !stack_is_root_only(&stack, root)
                    {
                        malformed_start_depth += 1;
                        push_incomplete_start_tag_recovery_text(
                            &mut ast,
                            current_parent,
                            &source,
                            token.start,
                            token.end,
                        );
                        extend_open_element_spans_to(
                            &mut ast,
                            &stack,
                            source.base_offset + token.end,
                        );
                        continue;
                    }
                    let is_void = options.void_tags.iter().any(|candidate| candidate == &name);
                    let namespace = vue3_element_namespace(
                        &ast,
                        current_parent,
                        &name,
                        current_namespace,
                        options,
                    );
                    let starts_v_pre =
                        v_pre_depth == 0 && attributes.iter().any(|attr| attr.name == "v-pre");
                    let in_v_pre = v_pre_depth > 0 || starts_v_pre;
                    let raw_text_kind = vue3_raw_text_kind(&name, namespace, in_v_pre);
                    let sfc_plain_template = vue3_is_sfc_plain_template(
                        &name,
                        current_parent,
                        root,
                        &attributes,
                        options,
                    );
                    let sfc_custom_block =
                        vue3_is_sfc_custom_block(&name, current_parent, root, options);
                    let id = ast.push_child(
                        current_parent,
                        vue3_element_kind(
                            name.clone(),
                            attributes,
                            self_closing,
                            options,
                            source.file_id,
                            source.base_offset,
                            in_v_pre,
                            namespace,
                        ),
                        Some(Span::new(
                            source.file_id,
                            source.base_offset + token.start,
                            source.base_offset + token.end,
                        )),
                    );
                    if sfc_plain_template || sfc_custom_block {
                        if let Some((text_end, end_tag_end)) =
                            find_matching_raw_text_end(&source.source, token.end, &name)
                        {
                            push_raw_text(
                                &mut ast,
                                id,
                                source.file_id,
                                source.base_offset + token.end,
                                &source.source[token.end..text_end],
                            );
                            if let Some(node) = ast.node_mut(id) {
                                if let Some(span) = node.span.source_mut() {
                                    span.end =
                                        vuec_source::BytePos(source.base_offset + end_tag_end);
                                }
                            }
                            tokenizer.set_cursor(end_tag_end);
                        }
                    } else if !self_closing && !is_void {
                        stack.push(id);
                        namespace_stack.push(namespace);
                        if in_v_pre {
                            v_pre_depth += 1;
                        }
                        if let Some(kind) = raw_text_kind {
                            if let Some((text_end, end_tag_end)) =
                                find_matching_raw_text_end(&source.source, token.end, &name)
                            {
                                let text = &source.source[token.end..text_end];
                                match kind {
                                    HtmlTextMode::Data => push_text_and_interpolations(
                                        &mut ast,
                                        id,
                                        source.file_id,
                                        source.base_offset + token.end,
                                        text,
                                        options,
                                    ),
                                    HtmlTextMode::RcData => push_text_and_interpolations(
                                        &mut ast,
                                        id,
                                        source.file_id,
                                        source.base_offset + token.end,
                                        text,
                                        options,
                                    ),
                                    HtmlTextMode::RawText => push_raw_text(
                                        &mut ast,
                                        id,
                                        source.file_id,
                                        source.base_offset + token.end,
                                        text,
                                    ),
                                }
                                if let Some(node) = ast.node_mut(id) {
                                    if let Some(span) = node.span.source_mut() {
                                        span.end =
                                            vuec_source::BytePos(source.base_offset + end_tag_end);
                                    }
                                }
                                tokenizer.set_cursor(end_tag_end);
                                stack.pop();
                                namespace_stack.pop();
                                if in_v_pre && v_pre_depth > 0 {
                                    v_pre_depth -= 1;
                                }
                            }
                        }
                    }
                }
                HtmlTokenKind::EndTag { name } => {
                    if name.is_empty() {
                        if vue3_empty_end_tag_should_be_text(&source.source, token.start, token.end)
                        {
                            push_text(
                                &mut ast,
                                current_parent,
                                source.file_id,
                                source.base_offset + token.start,
                                &source.source[token.start..token.end],
                            );
                        }
                        extend_open_element_spans_to(
                            &mut ast,
                            &stack,
                            source.base_offset + token.end,
                        );
                        continue;
                    }
                    if current_parent_raw_text_ignores_end_tag(&ast, current_parent, &name) {
                        push_text(
                            &mut ast,
                            current_parent,
                            source.file_id,
                            source.base_offset + token.start,
                            &source.source[token.start..token.end],
                        );
                        extend_open_element_spans_to(
                            &mut ast,
                            &stack,
                            source.base_offset + token.end,
                        );
                        continue;
                    }
                    if malformed_start_depth > 0 {
                        malformed_start_depth -= 1;
                        extend_open_element_spans_to(
                            &mut ast,
                            &stack,
                            source.base_offset + token.end,
                        );
                        continue;
                    }
                    if !stack_has_matching_element(&ast, &stack, &name) {
                        push_vue3_parser_diagnostic(
                            &mut ast,
                            Vue3ErrorCode::XInvalidEndTag,
                            source.file_id,
                            source.base_offset + token.start,
                        );
                        extend_open_element_spans_to(
                            &mut ast,
                            &stack,
                            source.base_offset + token.end,
                        );
                        continue;
                    }
                    while stack.len() > 1 {
                        let Some(node_id) = stack.pop() else {
                            break;
                        };
                        if namespace_stack.len() > 1 {
                            namespace_stack.pop();
                        }
                        if let Some(node) = ast.node(node_id) {
                            if matches!(&node.kind, Vue3AstKind::Element(element) if element.tag.eq_ignore_ascii_case(&name))
                            {
                                if let Some(node) = ast.node_mut(node_id) {
                                    if let Some(span) = node.span.source_mut() {
                                        span.end =
                                            vuec_source::BytePos(source.base_offset + token.end);
                                    }
                                }
                                if v_pre_depth > 0 {
                                    v_pre_depth -= 1;
                                }
                                break;
                            } else {
                                let missing_start = ast
                                    .node(node_id)
                                    .and_then(|node| node.span.source().map(|span| span.start.0))
                                    .unwrap_or(source.base_offset + token.start);
                                push_vue3_parser_diagnostic(
                                    &mut ast,
                                    Vue3ErrorCode::XMissingEndTag,
                                    source.file_id,
                                    missing_start,
                                );
                                if let Some(node) = ast.node_mut(node_id) {
                                    if let Some(span) = node.span.source_mut() {
                                        span.end =
                                            vuec_source::BytePos(source.base_offset + token.start);
                                    }
                                }
                                if v_pre_depth > 0 {
                                    v_pre_depth -= 1;
                                }
                            }
                        }
                    }
                }
                HtmlTokenKind::Cdata(text) => {
                    extend_open_element_spans_to(&mut ast, &stack, source.base_offset + token.end);
                    if current_namespace != vuec_ast::HtmlNamespace::Html {
                        push_text(
                            &mut ast,
                            current_parent,
                            source.file_id,
                            source.base_offset + token.start + "<![CDATA[".len(),
                            &text,
                        );
                    }
                }
                HtmlTokenKind::BogusQuestionTag => {
                    extend_open_element_spans_to(&mut ast, &stack, source.base_offset + token.end);
                }
                HtmlTokenKind::Doctype(_) | HtmlTokenKind::Eof => {}
            }
            if eof {
                let missing_starts = stack
                    .iter()
                    .copied()
                    .skip(1)
                    .filter_map(|node_id| {
                        ast.node(node_id)
                            .and_then(|node| node.span.source().map(|span| span.start.0))
                    })
                    .collect::<Vec<_>>();
                for start in missing_starts {
                    push_vue3_parser_diagnostic(
                        &mut ast,
                        Vue3ErrorCode::XMissingEndTag,
                        source.file_id,
                        start,
                    );
                }
                extend_open_element_spans_to(
                    &mut ast,
                    &stack,
                    source.base_offset + source.source.len(),
                );
                break;
            }
        }
        normalize_vue3_parse_text(&mut ast, options);
        ast
    }

    /// Runs Vue 3 transform passes over a parsed AST.
    pub fn transform(ast: &mut Vue3Ast, ctx: &mut TransformContext, options: &Vue3CompilerOptions) {
        let root_id = ast.root;
        let mut helpers = ctx.helpers.clone();
        let mut components = BTreeSet::new();
        let mut directives = BTreeSet::new();
        let mut has_element = false;
        let mut has_nested_element = false;
        let mut has_interpolation = false;
        let mut has_text_call = false;
        let mut has_fragment = false;
        let mut has_render_list = false;
        let mut has_normalize_class = false;
        let mut has_component = false;
        let mut has_component_slots = false;
        let mut has_dynamic_component_slots = false;
        let mut has_memo = false;
        let mut has_for_memo = false;
        let mut has_once = false;
        let mut walk = vec![(root_id, true)];
        while let Some((node_id, is_root)) = walk.pop() {
            if let Some(node) = ast.node(node_id) {
                if child_sequence_needs_text_vnode(ast, &node.children) {
                    has_text_call = true;
                }
                for child_id in node.children.clone() {
                    if let Some(child) = ast.node(child_id) {
                        match &child.kind {
                            Vue3AstKind::Element(element) => {
                                has_element = true;
                                if element.tag == "slot" {
                                    helpers.insert(RuntimeHelper::Vue3RenderSlot);
                                }
                                if element.tag_type == Vue3ElementType::Component {
                                    has_component = true;
                                    collect_vue3_component_asset(
                                        element,
                                        options,
                                        &mut components,
                                        &mut helpers,
                                    );
                                    let slot_analysis = analyze_component_slots(ast, child_id);
                                    if slot_analysis.has_slots {
                                        has_component_slots = true;
                                    }
                                    if slot_analysis.has_dynamic_slots {
                                        has_dynamic_component_slots = true;
                                    }
                                }
                                if !is_root {
                                    has_nested_element = true;
                                }
                                for prop in &element.props {
                                    if let Vue3Prop::Directive(dir) = prop {
                                        collect_vue3_runtime_directive_asset(
                                            dir,
                                            options,
                                            &mut directives,
                                            &mut helpers,
                                        );
                                        collect_vue3_binding_rewrite_helpers(
                                            dir,
                                            options,
                                            &mut helpers,
                                        );
                                        if dir.name == "model"
                                            && vue3_dom_model_kind(element).is_some()
                                        {
                                            helpers.insert(RuntimeHelper::Vue3WithDirectives);
                                            if render_model_assignment_for_directive(dir, options)
                                                .contains("_isRef(")
                                            {
                                                helpers.insert(RuntimeHelper::Vue3IsRef);
                                            }
                                            helpers.insert(vue3_dom_model_runtime_helper(
                                                vue3_dom_model_kind(element).unwrap(),
                                            ));
                                        }
                                        if dir.name == "model"
                                            && element.tag_type == Vue3ElementType::Component
                                        {
                                            let value = dir
                                                .exp
                                                .as_ref()
                                                .map(Vue3Expression::source_string)
                                                .unwrap_or_default();
                                            let scope = RenderScope::default();
                                            for helper in vue3_for_helpers_for_content(
                                                &rewrite_expression_with_scope(
                                                    &value, options, &scope,
                                                ),
                                            ) {
                                                if helper == "UNREF" {
                                                    helpers.insert(RuntimeHelper::Vue3Unref);
                                                }
                                            }
                                            if render_model_assignment_for_directive(dir, options)
                                                .contains("_isRef(")
                                            {
                                                helpers.insert(RuntimeHelper::Vue3IsRef);
                                            }
                                        }
                                        if dir.name == "memo" {
                                            has_memo = true;
                                            if directive_by_name(element, "for").is_some() {
                                                has_for_memo = true;
                                            }
                                        }
                                        if dir.name == "once" {
                                            has_once = true;
                                        }
                                        match dir.name.as_str() {
                                            "for" => {
                                                has_fragment = true;
                                                has_render_list = true;
                                            }
                                            "else" | "else-if" => {
                                                has_fragment = true;
                                            }
                                            "if" => {
                                                helpers
                                                    .insert(RuntimeHelper::Vue3CreateCommentVNode);
                                            }
                                            "bind"
                                                if dir.arg.as_ref().is_some_and(|arg| {
                                                    arg.source_string() == "class"
                                                }) =>
                                            {
                                                has_normalize_class = true;
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                                walk.push((child_id, false));
                            }
                            Vue3AstKind::Interpolation(_) => {
                                has_interpolation = true;
                            }
                            Vue3AstKind::Text(_) => {}
                            _ => {}
                        }
                    }
                }
            }
        }
        if has_element {
            helpers.insert(RuntimeHelper::Vue3OpenBlock);
            helpers.insert(RuntimeHelper::Vue3CreateElementBlock);
        }
        if has_nested_element || root_needs_fragment_block(ast) {
            helpers.insert(RuntimeHelper::Vue3CreateElementVNode);
        }
        if has_text_call {
            helpers.insert(RuntimeHelper::Vue3CreateTextVNode);
        }
        if has_fragment || root_needs_fragment_block(ast) {
            helpers.insert(RuntimeHelper::Vue3Fragment);
        }
        if has_render_list {
            helpers.insert(RuntimeHelper::Vue3RenderList);
        }
        if has_normalize_class {
            helpers.insert(RuntimeHelper::Vue3NormalizeClass);
        }
        if has_component {
            helpers.insert(RuntimeHelper::Vue3OpenBlock);
            helpers.insert(RuntimeHelper::Vue3CreateBlock);
        }
        if !components.is_empty() {
            helpers.insert(RuntimeHelper::Vue3ResolveComponent);
        }
        if has_component_slots {
            helpers.insert(RuntimeHelper::Vue3WithCtx);
        }
        if has_dynamic_component_slots {
            helpers.insert(RuntimeHelper::Vue3CreateSlots);
        }
        if has_interpolation {
            helpers.insert(RuntimeHelper::Vue3ToDisplayString);
        }
        if has_for_memo {
            helpers.insert(RuntimeHelper::Vue3IsMemoSame);
        }
        if has_memo {
            helpers.insert(RuntimeHelper::Vue3WithMemo);
        }
        if has_once {
            helpers.insert(RuntimeHelper::Vue3SetBlockTracking);
        }
        if let Some(root) = ast.root_node_mut() {
            if let Vue3AstKind::Root(root) = &mut root.kind {
                root.helpers = helpers.clone();
                root.components = components;
                root.directives = directives;
            }
        }
        for helper in helpers {
            ctx.add_helper(helper);
        }
    }

    /// Generates render code from a transformed Vue 3 AST.
    pub fn generate(
        ast: &Vue3Ast,
        options: &Vue3CompilerOptions,
        ctx: &TransformContext,
    ) -> CodegenResult {
        let mut writer = CodeWriter::new();
        let mut preamble = String::new();
        let root_id = ast.root;
        if let Some(root) = ast.node(root_id) {
            let components = vue3_codegen_components(ast);
            let component_declarations = render_component_declarations(&components);
            let directives = vue3_codegen_directives(ast);
            let directive_declarations = render_directive_declarations(&directives);
            let imports = vue3_codegen_imports(ast);
            let mut memo_index = MemoIndex::default();
            let static_hoists = collect_static_hoists(ast, options);
            let scope = RenderScope::default().with_static_hoists(static_hoists.clone());
            let expr = render_root_expr(ast, &root.children, options, &scope, &mut memo_index);
            let declarations = component_declarations
                .iter()
                .chain(directive_declarations.iter())
                .cloned()
                .collect::<Vec<_>>();
            let static_hoist_declarations = static_hoist_declarations(ast, options, &static_hoists);
            let helper_declarations = declarations
                .iter()
                .chain(static_hoist_declarations.iter())
                .cloned()
                .collect::<Vec<_>>();
            let mut helpers = vue3_codegen_helpers(
                ast,
                ctx,
                &helper_declarations,
                &expr,
                !components.is_empty(),
                options.stringify_static_preserve_helpers,
            );
            if options.inline {
                inline_preamble_helpers(&mut helpers, &expr);
                if !helpers.is_empty() {
                    preamble = format!(
                        "import {{ {} }} from \"vue\"\n\n",
                        import_helper_aliases(&helpers)
                    );
                }
                writer.push_line(&format!("({}) => {{", render_args(options)));
            } else if options.mode == "module" {
                if !helpers.is_empty() {
                    writer.push_line(&format!(
                        "import {{ {} }} from \"vue\"",
                        import_helper_aliases(&helpers)
                    ));
                }
                for import in &imports {
                    writer.push_line(import);
                }
                if !imports.is_empty() {
                    writer.newline();
                    writer.newline();
                } else if !helpers.is_empty() {
                    writer.newline();
                }
                for hoist in &static_hoist_declarations {
                    writer.push_line(hoist);
                }
                if !static_hoist_declarations.is_empty() {
                    writer.newline();
                }
                writer.push_line("export function render(_ctx, _cache) {");
            } else if options.prefix_identifiers {
                if !helpers.is_empty() {
                    writer.push_line(&format!("const {{ {} }} = Vue", helper_aliases(&helpers)));
                    writer.newline();
                }
                for hoist in &static_hoist_declarations {
                    writer.push_line(hoist);
                }
                if !static_hoist_declarations.is_empty() {
                    writer.newline();
                }
                writer.push_line(&format!(
                    "return function render({}) {{",
                    render_args(options)
                ));
            } else if options.mode == "function" {
                writer.push_line("const _Vue = Vue");
                writer.newline();
                writer.push_line(&format!(
                    "return function render({}) {{",
                    render_args(options)
                ));
            } else {
                writer.push_line(&format!(
                    "export function render({}) {{",
                    render_args(options)
                ));
            }
            writer.indent();
            if !options.inline && !options.prefix_identifiers && options.mode != "module" {
                writer.push_line("with (_ctx) {");
                writer.indent();
                if !helpers.is_empty() {
                    writer.push_line(&format!("const {{ {} }} = _Vue", helper_aliases(&helpers)));
                    writer.newline();
                }
            }
            for declaration in &component_declarations {
                writer.push_line(declaration);
            }
            for declaration in &directive_declarations {
                writer.push_line(declaration);
            }
            if !component_declarations.is_empty() || !directive_declarations.is_empty() {
                writer.newline();
            }
            if (options.inline || (!options.prefix_identifiers && options.mode != "module"))
                && !static_hoist_declarations.is_empty()
            {
                for hoist in &static_hoist_declarations {
                    writer.push_line(hoist);
                }
                writer.newline();
            }
            writer.push_line(&format!("return {}", expr));
            if !options.inline && !options.prefix_identifiers && options.mode != "module" {
                writer.dedent();
                writer.push_line("}");
            }
            writer.dedent();
            writer.push_line("}");
        }
        let code = writer.finish().trim_end().to_string();
        CodegenResult {
            code,
            map: None,
            ast_summary: format!("nodes={}", ast.len()),
            diagnostics: Vec::new(),
            preamble,
        }
    }

    /// Parses, transforms, and generates Vue 3 compiler-core output.
    pub fn base_compile(source: TemplateSource, options: Vue3CompilerOptions) -> CodegenResult {
        let mut ast = Self::base_parse(source.clone(), &options);
        let mut ctx = TransformContext::default();
        Self::transform(&mut ast, &mut ctx, &options);
        Self::finish_compile(ast, source, options, ctx)
    }

    /// Finishes compilation from an already parsed/transformed AST.
    pub fn finish_compile(
        ast: Vue3Ast,
        source: TemplateSource,
        options: Vue3CompilerOptions,
        ctx: TransformContext,
    ) -> CodegenResult {
        let mut result = Self::generate(&ast, &options, &ctx);
        if options.source_map {
            result.map = source_map_for_render(&result.code, &ast, &source, &options);
        }
        result.diagnostics = vue3_parser_diagnostics(&ast);
        result
            .diagnostics
            .extend(expression_diagnostics(&ast, &options));
        result.diagnostics.extend(ctx.diagnostics.into_vec());
        result
    }

    /// Compiles a template with Vue 3 DOM output conventions.
    pub fn compile_dom(source: TemplateSource, options: Vue3CompilerOptions) -> CodegenResult {
        let mut result = Self::base_compile(source, options);
        if result.code.is_empty() {
            result.code = "/* dom */".into();
        }
        result
    }

    /// Compiles a template with Vue 3 SSR output conventions.
    pub fn compile_ssr(source: TemplateSource, options: Vue3CompilerOptions) -> CodegenResult {
        let mut result = Self::base_compile(source, options);
        if !result.code.starts_with("/* ssr */") {
            result.code = format!("/* ssr */\n{}", result.code);
        }
        result
    }

    /// Lowers a Vue 3 AST to shared HIR and Vue 3 DOM MIR.
    pub fn lower_to_dom_mir(ast: &Vue3Ast, options: &Vue3CompilerOptions) -> Vue3DomLoweringResult {
        lower_vue3_ast_to_dom_mir(ast, options)
    }

    /// Lowers a Vue 3 AST to shared HIR and Vue 3 SSR MIR.
    pub fn lower_to_ssr_mir(ast: &Vue3Ast, options: &Vue3CompilerOptions) -> Vue3SsrLoweringResult {
        lower_vue3_ast_to_ssr_mir(ast, options)
    }
}
