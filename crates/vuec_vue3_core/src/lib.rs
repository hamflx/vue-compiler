#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use oxc_ast::ast::{BindingPattern, ChainElement, Expression};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use vuec_ast::{
    MissingSpanReason, NodeSpan, QuoteKind, RuntimeHelper, Vue3Ast, Vue3AstKind, Vue3Directive,
    Vue3Element, Vue3ElementType, Vue3Expression, Vue3NodeKind, Vue3Prop,
};
use vuec_codegen::{CodeWriter, SourceMapArtifact, SourceMapSegment};
use vuec_html::{HtmlTokenKind, HtmlTokenizer};
use vuec_js::JsAstStore;
use vuec_pass::TransformContext;
use vuec_source::{FileId, Span};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateSource {
    pub filename: String,
    pub source: String,
    pub file_id: FileId,
    pub base_offset: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3CompilerOptions {
    pub prefix_identifiers: bool,
    pub mode: String,
    pub hoist_static: bool,
    pub cache_handlers: bool,
    pub scope_id: Option<String>,
    pub slotted: bool,
    pub is_ts: bool,
    pub expression_plugins: Vec<String>,
    pub source_map: bool,
    pub comments: bool,
    pub delimiters: Option<[String; 2]>,
    pub void_tags: Vec<String>,
    pub native_tags: Option<Vec<String>>,
    pub custom_elements: Vec<String>,
    pub built_in_components: Vec<String>,
    pub namespaces: BTreeMap<String, vuec_ast::HtmlNamespace>,
    pub root_namespace: vuec_ast::HtmlNamespace,
    pub dom_namespaces: bool,
    pub whitespace: String,
    pub pre_tags: Vec<String>,
    pub ignore_newline_tags: Vec<String>,
    pub binding_metadata: BTreeMap<String, String>,
    pub props_aliases: BTreeMap<String, String>,
    pub inline: bool,
}

impl Default for Vue3CompilerOptions {
    fn default() -> Self {
        Self {
            prefix_identifiers: false,
            mode: "function".into(),
            hoist_static: false,
            cache_handlers: false,
            scope_id: None,
            slotted: false,
            is_ts: false,
            expression_plugins: Vec::new(),
            source_map: false,
            comments: true,
            delimiters: None,
            void_tags: Vec::new(),
            native_tags: None,
            custom_elements: Vec::new(),
            built_in_components: Vec::new(),
            namespaces: BTreeMap::new(),
            root_namespace: vuec_ast::HtmlNamespace::Html,
            dom_namespaces: false,
            whitespace: "condense".into(),
            pre_tags: Vec::new(),
            ignore_newline_tags: Vec::new(),
            binding_metadata: BTreeMap::new(),
            props_aliases: BTreeMap::new(),
            inline: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodegenResult {
    pub code: String,
    pub map: Option<SourceMapArtifact>,
    pub ast_summary: String,
    pub diagnostics: Vec<String>,
    pub preamble: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Vue3RawTextKind {
    RcData,
    RawText,
}

pub struct Vue3Dialect;

impl Vue3Dialect {
    pub fn base_parse(source: TemplateSource, options: &Vue3CompilerOptions) -> Vue3Ast {
        let mut ast = Vue3Ast::new(
            Vue3NodeKind::root(),
            Some(Span::new(
                source.file_id,
                source.base_offset,
                source.base_offset + source.source.len(),
            )),
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
                    if !self_closing && !is_void {
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
                                    Vue3RawTextKind::RcData => push_text_and_interpolations(
                                        &mut ast,
                                        id,
                                        source.file_id,
                                        source.base_offset + token.end,
                                        text,
                                        options,
                                    ),
                                    Vue3RawTextKind::RawText => push_raw_text(
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
                            } else if let Some(node) = ast.node_mut(node_id) {
                                if let Some(span) = node.span.source_mut() {
                                    span.end =
                                        vuec_source::BytePos(source.base_offset + token.start);
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

    pub fn transform(ast: &mut Vue3Ast, ctx: &mut TransformContext) {
        let root_id = ast.root;
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
                                    ctx.add_helper(RuntimeHelper::Vue3RenderSlot);
                                }
                                if element.tag_type == Vue3ElementType::Component {
                                    has_component = true;
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
                                        if dir.name == "memo" {
                                            has_memo = true;
                                            if directive_by_name(element, "for").is_some() {
                                                has_for_memo = true;
                                            }
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
                                                ctx.add_helper(
                                                    RuntimeHelper::Vue3CreateCommentVNode,
                                                );
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
            ctx.add_helper(RuntimeHelper::Vue3OpenBlock);
            ctx.add_helper(RuntimeHelper::Vue3CreateElementBlock);
        }
        if has_nested_element {
            ctx.add_helper(RuntimeHelper::Vue3CreateElementVNode);
        }
        if has_text_call {
            ctx.add_helper(RuntimeHelper::Vue3CreateTextVNode);
        }
        if has_fragment {
            ctx.add_helper(RuntimeHelper::Vue3Fragment);
        }
        if has_render_list {
            ctx.add_helper(RuntimeHelper::Vue3RenderList);
        }
        if has_normalize_class {
            ctx.add_helper(RuntimeHelper::Vue3NormalizeClass);
        }
        if has_component {
            ctx.add_helper(RuntimeHelper::Vue3ResolveComponent);
            ctx.add_helper(RuntimeHelper::Vue3OpenBlock);
            ctx.add_helper(RuntimeHelper::Vue3CreateBlock);
        }
        if has_component_slots {
            ctx.add_helper(RuntimeHelper::Vue3WithCtx);
        }
        if has_dynamic_component_slots {
            ctx.add_helper(RuntimeHelper::Vue3CreateSlots);
        }
        if has_interpolation {
            ctx.add_helper(RuntimeHelper::Vue3ToDisplayString);
        }
        if has_for_memo {
            ctx.add_helper(RuntimeHelper::Vue3IsMemoSame);
        }
        if has_memo {
            ctx.add_helper(RuntimeHelper::Vue3WithMemo);
        }
    }

    pub fn generate(
        ast: &Vue3Ast,
        options: &Vue3CompilerOptions,
        ctx: &TransformContext,
    ) -> CodegenResult {
        let mut writer = CodeWriter::new();
        let root_id = ast.root;
        if let Some(root) = ast.node(root_id) {
            let components = collect_component_tags(ast);
            let component_declarations = components
                .iter()
                .map(|component| {
                    format!(
                        "const {} = _resolveComponent({})",
                        component_asset_id(component),
                        quote_string(component)
                    )
                })
                .collect::<Vec<_>>();
            let expr = if root.children.len() == 1 {
                render_node_expr(ast, root.children[0], options, NodeRenderMode::Root)
            } else {
                render_children_array(ast, &root.children, options, true)
            };
            let helper_probe = format!("{}\n{}", component_declarations.join("\n"), expr);
            let mut helpers =
                render_helpers_from_code(vue3_helper_order(!components.is_empty()), &helper_probe);
            let needs_comment_helper = helper_probe.contains("_createCommentVNode(")
                || helper_probe.contains("? (_openBlock()")
                || helper_probe.contains("? _withMemo(");
            if needs_comment_helper && !helpers.contains(&RuntimeHelper::Vue3CreateCommentVNode) {
                helpers.push(RuntimeHelper::Vue3CreateCommentVNode);
            }
            if ctx.helpers.contains(&RuntimeHelper::Vue3WithMemo)
                && !helpers.contains(&RuntimeHelper::Vue3WithMemo)
            {
                helpers.push(RuntimeHelper::Vue3WithMemo);
            }
            sort_helpers_by_order(&mut helpers, vue3_helper_order(!components.is_empty()));
            apply_vue3_memo_helper_order(&mut helpers);
            if options.inline {
                writer.push_line("(_ctx, _cache) => {");
            } else if options.mode == "module" {
                if !helpers.is_empty() {
                    writer.push_line(&format!(
                        "import {{ {} }} from \"vue\"",
                        import_helper_aliases(&helpers)
                    ));
                    writer.newline();
                }
                writer.push_line("export function render(_ctx, _cache) {");
            } else if options.prefix_identifiers {
                if !helpers.is_empty() {
                    writer.push_line(&format!("const {{ {} }} = Vue", helper_aliases(&helpers)));
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
            if !component_declarations.is_empty() {
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
            preamble: String::new(),
        }
    }

    pub fn base_compile(source: TemplateSource, options: Vue3CompilerOptions) -> CodegenResult {
        let mut ast = Self::base_parse(source.clone(), &options);
        let mut ctx = TransformContext::default();
        Self::transform(&mut ast, &mut ctx);
        Self::finish_compile(ast, source, options, ctx)
    }

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
        result.diagnostics = expression_diagnostics(&ast, &options);
        result.diagnostics.extend(
            ctx.diagnostics
                .into_vec()
                .into_iter()
                .map(|diagnostic| diagnostic.message),
        );
        result
    }

    pub fn compile_dom(source: TemplateSource, options: Vue3CompilerOptions) -> CodegenResult {
        let mut result = Self::base_compile(source, options);
        if result.code.is_empty() {
            result.code = "/* dom */".into();
        }
        result
    }

    pub fn compile_ssr(source: TemplateSource, options: Vue3CompilerOptions) -> CodegenResult {
        let mut result = Self::base_compile(source, options);
        if !result.code.starts_with("/* ssr */") {
            result.code = format!("/* ssr */\n{}", result.code);
        }
        result
    }
}

pub fn root_codegen_projection(root: &Value) -> Value {
    let children = root
        .get("children")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    match children {
        [] => json!({ "kind": "none" }),
        [_] => root_single_child_codegen_projection(children),
        _ => json!({
            "kind": "fragment",
            "patchFlag": root_fragment_patch_flag(children),
        }),
    }
}

fn root_single_child_codegen_projection(children: &[Value]) -> Value {
    if let Some((index, child)) = single_element_root(children) {
        if child
            .get("codegenNode")
            .is_some_and(|value| !value.is_null())
        {
            return json!({
                "kind": "childCodegen",
                "index": index,
                "asBlock": child
                    .get("codegenNode")
                    .and_then(json_node_type)
                    == Some(13),
            });
        }
    }
    json!({ "kind": "child", "index": 0 })
}

fn single_element_root(children: &[Value]) -> Option<(usize, &Value)> {
    let mut element = None;
    for (index, child) in children.iter().enumerate() {
        if json_node_type(child) == Some(3) {
            continue;
        }
        if json_node_type(child) != Some(1) || json_u64(child, "tagType") == Some(2) {
            return None;
        }
        if element.replace((index, child)).is_some() {
            return None;
        }
    }
    element
}

fn root_fragment_patch_flag(children: &[Value]) -> u16 {
    let visible = children
        .iter()
        .filter(|child| json_node_type(child) != Some(3))
        .count();
    if visible == 1
        && children
            .iter()
            .any(|child| json_node_type(child) == Some(3))
    {
        64 | 2048
    } else {
        64
    }
}

fn json_node_type(value: &Value) -> Option<u64> {
    json_u64(value, "type")
}

fn json_u64(value: &Value, key: &str) -> Option<u64> {
    value.get(key).and_then(Value::as_u64)
}

fn json_usize(value: &Value, key: &str) -> Option<usize> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .map(|value| value as usize)
}

const VUE3_CONSTANT_NOT: u8 = 0;
const VUE3_CONSTANT_CAN_SKIP_PATCH: u8 = 1;
const VUE3_CONSTANT_CAN_CACHE: u8 = 2;
const VUE3_CONSTANT_CAN_STRINGIFY: u8 = 3;

pub fn get_constant_type_projection(payload: &Value) -> Value {
    let node = payload.get("node").unwrap_or(&Value::Null);
    let context = payload.get("context").unwrap_or(&Value::Null);
    json!({
        "constantType": vue3_constant_type(node, context),
    })
}

pub fn is_member_expression_projection(payload: &Value) -> Value {
    let node = payload.get("node").unwrap_or(&Value::Null);
    let context = payload.get("context").unwrap_or(&Value::Null);
    let source = model_expression_source(node);
    let mode = json_str(payload, "mode").unwrap_or("node");
    let is_member = if mode == "browser" {
        transform_on_is_member_expression_lexer(&source)
    } else {
        transform_on_is_member_expression(&source, context)
    };
    json!({
        "isMemberExpression": is_member,
    })
}

pub fn cache_static_projection(payload: &Value) -> Value {
    let root = payload.get("root").unwrap_or(&Value::Null);
    let context = payload.get("context").unwrap_or(&Value::Null);
    let children = root
        .get("children")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let do_not_hoist_root = vue3_single_element_root(children).is_some();
    let mut state = Vue3CacheStaticState::default();
    vue3_cache_static_walk(
        children,
        vec!["children".to_string()],
        None,
        root,
        context,
        do_not_hoist_root,
        &mut state,
    );
    json!({
        "operations": state.operations,
    })
}

#[derive(Default)]
struct Vue3CacheStaticState {
    operations: Vec<Value>,
}

fn vue3_cache_static_walk(
    children: &[Value],
    children_path: Vec<String>,
    parent_path: Option<Vec<String>>,
    parent: &Value,
    context: &Value,
    do_not_hoist_node: bool,
    state: &mut Vue3CacheStaticState,
) {
    let mut to_cache = Vec::<usize>::new();

    for (index, child) in children.iter().enumerate() {
        let child_path = vue3_path_child(&children_path, index);
        if json_node_type(child) == Some(1) && json_u64(child, "tagType") == Some(0) {
            let constant_type = if do_not_hoist_node {
                VUE3_CONSTANT_NOT
            } else {
                vue3_constant_type(child, context)
            };
            if constant_type > VUE3_CONSTANT_NOT {
                if constant_type >= VUE3_CONSTANT_CAN_CACHE {
                    if vue3_should_downgrade_static_block(child) {
                        state.operations.push(json!({
                            "kind": "setBlock",
                            "path": vue3_codegen_path(&child_path),
                            "isBlock": false,
                        }));
                    }
                    state.operations.push(json!({
                        "kind": "setPatchFlag",
                        "path": vue3_codegen_path(&child_path),
                        "patchFlag": -1,
                    }));
                    to_cache.push(index);
                    continue;
                }
            } else {
                vue3_project_prop_hoists(child, &child_path, context, state);
            }
        } else if json_node_type(child) == Some(12) {
            let constant_type = if do_not_hoist_node {
                VUE3_CONSTANT_NOT
            } else {
                vue3_constant_type(child, context)
            };
            if constant_type >= VUE3_CONSTANT_CAN_CACHE {
                state.operations.push(json!({
                    "kind": "appendTextCallPatchFlag",
                    "path": vue3_codegen_path(&child_path),
                    "patchFlag": "-1 /* CACHED */",
                }));
                to_cache.push(index);
                continue;
            }
        }

        match json_node_type(child) {
            Some(1) => {
                let child_children = child
                    .get("children")
                    .and_then(Value::as_array)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]);
                vue3_cache_static_walk(
                    child_children,
                    vue3_path_push(&child_path, "children"),
                    Some(child_path.clone()),
                    child,
                    context,
                    false,
                    state,
                );
            }
            Some(11) => {
                let for_children = child
                    .get("children")
                    .and_then(Value::as_array)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]);
                vue3_cache_static_walk(
                    for_children,
                    vue3_path_push(&child_path, "children"),
                    Some(child_path.clone()),
                    child,
                    context,
                    for_children.len() == 1,
                    state,
                );
            }
            Some(9) => {
                if let Some(branches) = child.get("branches").and_then(Value::as_array) {
                    for (branch_index, branch) in branches.iter().enumerate() {
                        let branch_children = branch
                            .get("children")
                            .and_then(Value::as_array)
                            .map(Vec::as_slice)
                            .unwrap_or(&[]);
                        vue3_cache_static_walk(
                            branch_children,
                            vue3_path_push(
                                &vue3_path_child(
                                    &vue3_path_push(&child_path, "branches"),
                                    branch_index,
                                ),
                                "children",
                            ),
                            Some(vue3_path_child(
                                &vue3_path_push(&child_path, "branches"),
                                branch_index,
                            )),
                            branch,
                            context,
                            branch_children.len() == 1,
                            state,
                        );
                    }
                }
            }
            _ => {}
        }
    }

    if vue3_can_cache_children_array(&to_cache, children, parent) {
        let target = if json_u64(parent, "tagType") == Some(0) {
            Some(json!({
                "kind": "cacheChildrenArray",
                "path": vue3_path_push(
                    &vue3_codegen_path(parent_path.as_deref().unwrap_or(&[])),
                    "children"
                ),
                "childrenPath": children_path,
                "needArraySpread": true,
            }))
        } else if json_u64(parent, "tagType") == Some(1) {
            Some(json!({
                "kind": "cacheSlotReturns",
                "ownerPath": parent_path,
                "slot": { "kind": "static", "name": "default" },
                "needArraySpread": true,
            }))
        } else if json_u64(parent, "tagType") == Some(3) {
            parent_path.as_ref().and_then(|template_path| {
                let slot = vue3_template_slot_projection(parent)?;
                Some(json!({
                    "kind": "cacheSlotReturns",
                    "ownerPath": vue3_parent_path(template_path),
                    "slot": slot,
                    "needArraySpread": true,
                }))
            })
        } else {
            None
        };
        if let Some(operation) = target {
            state.operations.push(operation);
            return;
        }
    }

    for index in to_cache {
        state.operations.push(json!({
            "kind": "cacheCodegen",
            "path": vue3_codegen_path(&vue3_path_child(&children_path, index)),
        }));
    }
}

fn vue3_project_prop_hoists(
    node: &Value,
    child_path: &[String],
    context: &Value,
    state: &mut Vue3CacheStaticState,
) {
    let Some(codegen_node) = node.get("codegenNode") else {
        return;
    };
    if json_node_type(codegen_node) != Some(13) {
        return;
    }
    let flag = codegen_node.get("patchFlag");
    let patch_flag_allows_props = flag.is_none_or(Value::is_null)
        || flag.and_then(Value::as_i64) == Some(512)
        || flag.and_then(Value::as_i64) == Some(1);
    if patch_flag_allows_props
        && vue3_generated_props_constant_type(node, context) >= VUE3_CONSTANT_CAN_CACHE
        && !codegen_node.get("props").is_none_or(Value::is_null)
    {
        state.operations.push(json!({
            "kind": "hoistProps",
            "path": vue3_path_push(&vue3_codegen_path(child_path), "props"),
        }));
    }
    if !codegen_node.get("dynamicProps").is_none_or(Value::is_null) {
        state.operations.push(json!({
            "kind": "hoistDynamicProps",
            "path": vue3_path_push(&vue3_codegen_path(child_path), "dynamicProps"),
        }));
    }
}

fn vue3_can_cache_children_array(to_cache: &[usize], children: &[Value], parent: &Value) -> bool {
    if to_cache.len() != children.len() || children.is_empty() || json_node_type(parent) != Some(1)
    {
        return false;
    }
    match json_u64(parent, "tagType") {
        Some(0) => {
            let Some(codegen_node) = parent.get("codegenNode") else {
                return false;
            };
            json_node_type(codegen_node) == Some(13)
                && codegen_node
                    .get("children")
                    .and_then(Value::as_array)
                    .is_some()
        }
        Some(1) => parent.get("codegenNode").is_some_and(|codegen_node| {
            json_node_type(codegen_node) == Some(13)
                && vue3_codegen_has_object_slots(codegen_node)
                && vue3_slot_returns_len(
                    codegen_node,
                    &json!({ "kind": "static", "name": "default" }),
                ) == Some(children.len())
        }),
        Some(3) => true,
        _ => false,
    }
}

fn vue3_constant_type(node: &Value, context: &Value) -> u8 {
    match json_node_type(node) {
        Some(1) => vue3_element_constant_type(node, context),
        Some(2) | Some(3) => VUE3_CONSTANT_CAN_STRINGIFY,
        Some(9) | Some(10) | Some(11) => VUE3_CONSTANT_NOT,
        Some(5) | Some(12) => node
            .get("content")
            .map(|content| vue3_constant_type(content, context))
            .unwrap_or(VUE3_CONSTANT_NOT),
        Some(4) => json_u64(node, "constType")
            .map(|value| value as u8)
            .unwrap_or_else(|| {
                if json_bool(node, "isStatic") {
                    VUE3_CONSTANT_CAN_STRINGIFY
                } else {
                    VUE3_CONSTANT_NOT
                }
            }),
        Some(8) => vue3_compound_constant_type(node, context),
        Some(20) => VUE3_CONSTANT_CAN_CACHE,
        _ => VUE3_CONSTANT_NOT,
    }
}

fn vue3_element_constant_type(node: &Value, context: &Value) -> u8 {
    if json_u64(node, "tagType") != Some(0) {
        return VUE3_CONSTANT_NOT;
    }
    let Some(codegen_node) = node.get("codegenNode") else {
        return VUE3_CONSTANT_NOT;
    };
    if json_node_type(codegen_node) != Some(13) {
        return VUE3_CONSTANT_NOT;
    }
    if json_bool(codegen_node, "isBlock")
        && !matches!(
            json_str(node, "tag"),
            Some("svg" | "foreignObject" | "math")
        )
    {
        return VUE3_CONSTANT_NOT;
    }
    if !codegen_node.get("patchFlag").is_none_or(Value::is_null) {
        return VUE3_CONSTANT_NOT;
    }

    let mut return_type = VUE3_CONSTANT_CAN_STRINGIFY;
    let generated_props_type = vue3_generated_props_constant_type(node, context);
    if generated_props_type == VUE3_CONSTANT_NOT {
        return VUE3_CONSTANT_NOT;
    }
    return_type = return_type.min(generated_props_type);

    for child in node
        .get("children")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
    {
        let child_type = vue3_constant_type(child, context);
        if child_type == VUE3_CONSTANT_NOT {
            return VUE3_CONSTANT_NOT;
        }
        return_type = return_type.min(child_type);
    }

    if return_type > VUE3_CONSTANT_CAN_SKIP_PATCH {
        for prop in node
            .get("props")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[])
        {
            if json_node_type(prop) == Some(7)
                && json_str(prop, "name") == Some("bind")
                && prop.get("exp").is_some_and(|exp| !exp.is_null())
            {
                let exp_type = vue3_constant_type(prop.get("exp").unwrap_or(&Value::Null), context);
                if exp_type == VUE3_CONSTANT_NOT {
                    return VUE3_CONSTANT_NOT;
                }
                return_type = return_type.min(exp_type);
            }
        }
    }

    if json_bool(codegen_node, "isBlock")
        && node
            .get("props")
            .and_then(Value::as_array)
            .is_some_and(|props| props.iter().any(|prop| json_node_type(prop) == Some(7)))
    {
        return VUE3_CONSTANT_NOT;
    }

    return_type
}

fn vue3_compound_constant_type(node: &Value, context: &Value) -> u8 {
    let mut return_type = VUE3_CONSTANT_CAN_STRINGIFY;
    for child in node
        .get("children")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
    {
        if child.is_string() {
            continue;
        }
        let child_type = vue3_constant_type(child, context);
        if child_type == VUE3_CONSTANT_NOT {
            return VUE3_CONSTANT_NOT;
        }
        return_type = return_type.min(child_type);
    }
    return_type
}

fn vue3_generated_props_constant_type(node: &Value, context: &Value) -> u8 {
    let Some(props) = node
        .get("codegenNode")
        .and_then(|codegen| codegen.get("props"))
    else {
        return VUE3_CONSTANT_CAN_STRINGIFY;
    };
    if json_node_type(props) != Some(15) {
        return VUE3_CONSTANT_NOT;
    }
    let mut return_type = VUE3_CONSTANT_CAN_STRINGIFY;
    for prop in props
        .get("properties")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
    {
        let key_type = prop
            .get("key")
            .map(|key| vue3_constant_type(key, context))
            .unwrap_or(VUE3_CONSTANT_NOT);
        if key_type == VUE3_CONSTANT_NOT {
            return VUE3_CONSTANT_NOT;
        }
        return_type = return_type.min(key_type);

        let value = prop.get("value").unwrap_or(&Value::Null);
        let value_type = if json_node_type(value) == Some(4) {
            vue3_constant_type(value, context)
        } else if json_node_type(value) == Some(14) {
            vue3_helper_call_constant_type(value, context)
        } else {
            VUE3_CONSTANT_NOT
        };
        if value_type == VUE3_CONSTANT_NOT {
            return VUE3_CONSTANT_NOT;
        }
        return_type = return_type.min(value_type);
    }
    return_type
}

fn vue3_helper_call_constant_type(value: &Value, context: &Value) -> u8 {
    if json_node_type(value) != Some(14) || !vue3_allow_hoisted_helper_call(value) {
        return VUE3_CONSTANT_NOT;
    }
    let Some(arg) = value
        .get("arguments")
        .and_then(Value::as_array)
        .and_then(|arguments| arguments.first())
    else {
        return VUE3_CONSTANT_NOT;
    };
    if json_node_type(arg) == Some(4) {
        vue3_constant_type(arg, context)
    } else if json_node_type(arg) == Some(14) {
        vue3_helper_call_constant_type(arg, context)
    } else {
        VUE3_CONSTANT_NOT
    }
}

fn vue3_allow_hoisted_helper_call(value: &Value) -> bool {
    value
        .get("callee")
        .and_then(Value::as_str)
        .is_some_and(|callee| {
            matches!(
                callee,
                "NORMALIZE_CLASS" | "NORMALIZE_STYLE" | "NORMALIZE_PROPS" | "GUARD_REACTIVE_PROPS"
            )
        })
}

fn vue3_should_downgrade_static_block(node: &Value) -> bool {
    let Some(codegen_node) = node.get("codegenNode") else {
        return false;
    };
    json_bool(codegen_node, "isBlock")
        && matches!(
            json_str(node, "tag"),
            Some("svg" | "foreignObject" | "math")
        )
        && !node
            .get("props")
            .and_then(Value::as_array)
            .is_some_and(|props| props.iter().any(|prop| json_node_type(prop) == Some(7)))
}

fn vue3_single_element_root(children: &[Value]) -> Option<&Value> {
    let non_comments = children
        .iter()
        .filter(|child| json_node_type(child) != Some(3))
        .collect::<Vec<_>>();
    match non_comments.as_slice() {
        [node] if json_node_type(node) == Some(1) && json_u64(node, "tagType") != Some(2) => {
            Some(*node)
        }
        _ => None,
    }
}

fn vue3_path_child(path: &[String], index: usize) -> Vec<String> {
    let mut out = path.to_vec();
    out.push(index.to_string());
    out
}

fn vue3_path_push(path: &[String], key: &str) -> Vec<String> {
    let mut out = path.to_vec();
    out.push(key.to_string());
    out
}

fn vue3_parent_path(path: &[String]) -> Vec<String> {
    let mut out = path.to_vec();
    out.pop();
    out.pop();
    out
}

fn vue3_codegen_path(path: &[String]) -> Vec<String> {
    vue3_path_push(path, "codegenNode")
}

fn vue3_template_slot_projection(node: &Value) -> Option<Value> {
    let dir = node
        .get("props")
        .and_then(Value::as_array)?
        .iter()
        .find(|prop| json_str(prop, "name") == Some("slot"))?;
    let arg = dir.get("arg")?;
    if json_bool(arg, "isStatic") {
        Some(json!({
            "kind": "static",
            "name": json_str(arg, "content").unwrap_or("default"),
        }))
    } else {
        Some(json!({
            "kind": "dynamic",
            "node": arg,
        }))
    }
}

fn vue3_codegen_has_object_slots(codegen_node: &Value) -> bool {
    codegen_node
        .get("children")
        .is_some_and(|children| json_node_type(children) == Some(15))
}

fn vue3_slot_returns_len(codegen_node: &Value, slot: &Value) -> Option<usize> {
    let properties = codegen_node
        .get("children")?
        .get("properties")?
        .as_array()?;
    let property = properties
        .iter()
        .find(|property| vue3_slot_property_matches(property, slot))?;
    property
        .get("value")?
        .get("returns")?
        .as_array()
        .map(Vec::len)
}

fn vue3_slot_property_matches(property: &Value, slot: &Value) -> bool {
    let Some(key) = property.get("key") else {
        return false;
    };
    if json_str(slot, "kind") == Some("static") {
        let name = json_str(slot, "name").unwrap_or("default");
        return json_str(key, "content") == Some(name);
    }
    if json_str(slot, "kind") == Some("dynamic") {
        return property.get("key") == slot.get("node");
    }
    false
}

pub fn transform_model_projection(payload: &Value) -> Value {
    let dir = payload.get("dir").unwrap_or(&Value::Null);
    let node = payload.get("node").unwrap_or(&Value::Null);
    let context = payload.get("context").unwrap_or(&Value::Null);
    let Some(exp) = dir.get("exp").filter(|value| !value.is_null()) else {
        return json!({ "errors": [41], "props": [] });
    };

    let raw_exp = exp
        .get("loc")
        .and_then(|loc| loc.get("source"))
        .and_then(Value::as_str)
        .unwrap_or_else(|| json_str(exp, "content").unwrap_or(""))
        .trim();
    let exp_string = json_str(exp, "content").unwrap_or(raw_exp);
    let binding_type = context
        .get("bindingMetadata")
        .and_then(|metadata| metadata.get(raw_exp))
        .and_then(Value::as_str);

    if matches!(binding_type, Some("props" | "props-aliased")) {
        return json!({ "errors": [44], "props": [] });
    }
    if matches!(binding_type, Some("literal-const" | "setup-const")) {
        return json!({ "errors": [45], "props": [] });
    }

    let maybe_ref = json_bool(context, "inline")
        && matches!(
            binding_type,
            Some("setup-let" | "setup-ref" | "setup-maybe-ref")
        );
    if exp_string.trim().is_empty() || (!model_is_member_expression(raw_exp) && !maybe_ref) {
        return json!({ "errors": [42], "props": [] });
    }
    if json_bool(context, "prefixIdentifiers")
        && is_simple_identifier_ascii(exp_string)
        && context_identifier_count(context, exp_string) > 0
    {
        return json!({ "errors": [43], "props": [] });
    }

    let arg = dir.get("arg").filter(|value| !value.is_null());
    let event_arg = if json_bool(context, "isTS") {
        "($event: any)"
    } else {
        "$event"
    };
    let assignment = model_assignment_projection(exp, raw_exp, event_arg, binding_type, maybe_ref);
    let mut props = vec![
        json!({
            "kind": "modelValue",
            "key": model_prop_name_projection(arg),
            "value": { "kind": "node", "path": "dir.exp" },
            "dynamic": true,
        }),
        json!({
            "kind": "modelUpdate",
            "key": model_event_name_projection(arg),
            "value": assignment,
            "cache": should_cache_model_update(exp, context),
            "dynamic": !should_cache_model_update(exp, context),
            "hydrate": model_update_needs_hydration_event(arg, node),
        }),
    ];

    if dir
        .get("modifiers")
        .and_then(Value::as_array)
        .is_some_and(|modifiers| !modifiers.is_empty())
        && json_u64(node, "tagType") == Some(1)
    {
        props.push(json!({
            "kind": "modelModifiers",
            "key": model_modifiers_key_projection(arg),
            "value": model_modifiers_expression(dir),
            "dynamic": false,
        }));
    }

    json!({
        "errors": [],
        "props": props,
    })
}

pub fn transform_on_projection(payload: &Value) -> Value {
    let dir = payload.get("dir").unwrap_or(&Value::Null);
    let node = payload.get("node").unwrap_or(&Value::Null);
    let context = payload.get("context").unwrap_or(&Value::Null);
    let arg = dir.get("arg").filter(|value| !value.is_null());
    let mut errors = Vec::<Value>::new();

    if dir.get("exp").is_none_or(Value::is_null)
        && dir
            .get("modifiers")
            .and_then(Value::as_array)
            .is_none_or(Vec::is_empty)
    {
        errors.push(json!({ "code": 35, "loc": "dir" }));
    }

    let event_name = transform_on_event_name_projection(arg, node, &mut errors);
    let handler = transform_on_handler_projection(dir, node, context);
    let cache = json_bool(&handler, "cache");
    let value = handler
        .get("value")
        .cloned()
        .unwrap_or_else(|| transform_on_empty_handler_projection(dir));

    json!({
        "errors": errors,
        "props": [{
            "key": event_name,
            "value": value,
            "cache": cache,
            "valueConstant": transform_on_projection_const_type(&value) > 0,
            "handlerKey": true,
            "dynamicKey": arg.is_some_and(|arg| !json_bool(arg, "isStatic")),
            "ignoreDynamicKeyForNormalize": true,
        }],
    })
}

pub fn transform_if_projection(payload: &Value) -> Value {
    if json_str(payload, "phase") == Some("branchCodegen") {
        return transform_if_branch_codegen_projection(payload);
    }
    transform_if_process_projection(payload)
}

pub fn transform_for_projection(payload: &Value) -> Value {
    if json_str(payload, "phase") == Some("codegen") {
        return transform_for_codegen_projection(payload);
    }
    if json_str(payload, "phase") == Some("exitCodegen") {
        return transform_for_exit_codegen_projection(payload);
    }

    let dir = payload.get("dir").unwrap_or(&Value::Null);
    let node = payload.get("node").unwrap_or(&Value::Null);
    let context = payload.get("context").unwrap_or(&Value::Null);
    let mut errors = Vec::<Value>::new();
    let Some(exp) = dir.get("exp").filter(|value| !value.is_null()) else {
        errors.push(json!({ "code": 31, "loc": "dir" }));
        return json!({ "errors": errors });
    };
    let raw = json_str(exp, "content")
        .or_else(|| exp.get("loc").and_then(|loc| json_str(loc, "source")))
        .unwrap_or("");
    let Some(parsed) = parse_vue3_for_expression(raw) else {
        errors.push(json!({ "code": 32, "loc": "dir" }));
        return json!({ "errors": errors });
    };

    let mut source = vue3_for_expression_projection(
        &parsed.source.content,
        exp,
        parsed.source.start,
        parsed.source.end,
        Vue3ForAstMode::Expression,
    );
    let mut value = parsed.value.as_ref().map(|part| {
        vue3_for_expression_projection(
            &part.content,
            exp,
            part.start,
            part.end,
            Vue3ForAstMode::Params,
        )
    });
    let mut key = parsed.key.as_ref().map(|part| {
        vue3_for_expression_projection(
            &part.content,
            exp,
            part.start,
            part.end,
            Vue3ForAstMode::Params,
        )
    });
    let mut index = parsed.index.as_ref().map(|part| {
        vue3_for_expression_projection(
            &part.content,
            exp,
            part.start,
            part.end,
            Vue3ForAstMode::Params,
        )
    });

    if json_bool(context, "prefixIdentifiers") {
        let options = vue3_options_from_transform_context(context);
        let locals = transform_context_locals(context);
        source = vue3_for_rewrite_projection_node(
            &parsed.source.content,
            &options,
            &locals,
            source["loc"].clone(),
            Vue3ForAstMode::Expression,
            false,
        );
        let scoped = parsed
            .all_alias_locals()
            .into_iter()
            .chain(locals)
            .collect::<Vec<_>>();
        if let Some(part) = parsed.value.as_ref() {
            value = Some(vue3_for_rewrite_projection_node(
                &part.content,
                &options,
                &scoped,
                value
                    .as_ref()
                    .and_then(|node| node.get("loc"))
                    .cloned()
                    .unwrap_or(Value::Null),
                Vue3ForAstMode::Params,
                true,
            ));
        }
        if let Some(part) = parsed.key.as_ref() {
            key = Some(vue3_for_rewrite_projection_node(
                &part.content,
                &options,
                &scoped,
                key.as_ref()
                    .and_then(|node| node.get("loc"))
                    .cloned()
                    .unwrap_or(Value::Null),
                Vue3ForAstMode::Params,
                true,
            ));
        }
        if let Some(part) = parsed.index.as_ref() {
            index = Some(vue3_for_rewrite_projection_node(
                &part.content,
                &options,
                &scoped,
                index
                    .as_ref()
                    .and_then(|node| node.get("loc"))
                    .cloned()
                    .unwrap_or(Value::Null),
                Vue3ForAstMode::Params,
                true,
            ));
        }
    }

    let parse_result = json!({
        "source": source,
        "value": value,
        "key": key,
        "index": index,
        "finalized": true,
    });
    let template_key_errors = vue3_for_template_key_errors(node);

    json!({
        "errors": errors,
        "parseResult": parse_result,
        "locals": parsed.all_alias_locals(),
        "children": if json_u64(node, "tagType") == Some(3) { "template" } else { "self" },
        "templateKeyErrors": template_key_errors,
    })
}

pub fn track_slot_scopes_projection(payload: &Value) -> Value {
    let node = payload.get("node").unwrap_or(&Value::Null);
    let Some(slot) = vue3_slot_directive(node, false) else {
        return json!({ "track": false });
    };
    let locals = slot
        .get("exp")
        .filter(|exp| !exp.is_null())
        .map(vue3_slot_param_locals)
        .unwrap_or_default();
    json!({
        "track": true,
        "slotProps": slot.get("exp").cloned().unwrap_or(Value::Null),
        "locals": locals,
    })
}

pub fn track_v_for_slot_scopes_projection(payload: &Value) -> Value {
    let node = payload.get("node").unwrap_or(&Value::Null);
    if json_node_type(node) != Some(1)
        || json_u64(node, "tagType") != Some(3)
        || vue3_slot_directive(node, true).is_none()
    {
        return json!({ "track": false });
    }
    let Some(dir) = vue3_directive(node, "for", true) else {
        return json!({ "track": false });
    };
    let context = payload.get("context").unwrap_or(&Value::Null);
    let projection = vue3_for_parse_result_projection(node, dir, context);
    if projection.get("parseResult").is_none() {
        return json!({ "track": false, "errors": projection.get("errors").cloned().unwrap_or_else(|| json!([])) });
    }
    json!({
        "track": true,
        "dir": dir,
        "parseResult": projection["parseResult"].clone(),
        "locals": projection["locals"].clone(),
    })
}

pub fn build_slots_projection(payload: &Value) -> Value {
    let node = payload.get("node").unwrap_or(&Value::Null);
    let context = payload.get("context").unwrap_or(&Value::Null);
    let children = node
        .get("children")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let mut properties = Vec::<Value>::new();
    let mut dynamic_slots = Vec::<Value>::new();
    let mut errors = Vec::<Value>::new();
    let mut has_dynamic_slots = json_usize(context, "vSlotDepth").unwrap_or_default() > 0
        || json_usize(context, "vForDepth").unwrap_or_default() > 0;

    if !json_bool(context, "ssr") && json_bool(context, "prefixIdentifiers") {
        has_dynamic_slots = vue3_component_slot_scope_ref(node, children, context);
    }

    let on_component_slot = vue3_slot_directive(node, true);
    if let Some(slot) = on_component_slot {
        if slot
            .get("arg")
            .is_some_and(|arg| !json_bool(arg, "isStatic"))
        {
            has_dynamic_slots = true;
        }
        properties.push(json!({
            "kind": "property",
            "key": vue3_slot_name_projection(slot, context),
            "params": slot.get("exp").cloned().unwrap_or(Value::Null),
            "indices": vue3_all_child_indices(children),
            "loc": node.get("loc").cloned().unwrap_or(Value::Null),
        }));
    }

    let mut has_template_slots = false;
    let mut has_named_default_slot = false;
    let mut implicit_default_indices = Vec::<usize>::new();
    let mut seen_slot_names = Vec::<String>::new();
    let mut conditional_branch_index = 0usize;

    for (index, child) in children.iter().enumerate() {
        let Some(slot_dir) = vue3_template_slot_directive(child) else {
            if json_node_type(child) != Some(3) {
                implicit_default_indices.push(index);
            }
            continue;
        };

        if on_component_slot.is_some() {
            errors.push(
                json!({ "code": 37, "loc": slot_dir.get("loc").cloned().unwrap_or(Value::Null) }),
            );
            break;
        }

        has_template_slots = true;
        let slot_name = vue3_slot_name_projection(slot_dir, context);
        let static_slot_name = vue3_static_slot_name(slot_dir);
        if static_slot_name.is_none() {
            has_dynamic_slots = true;
        }
        let slot = vue3_slot_function_projection(slot_dir, &[index], child);

        if let Some(if_dir) = vue3_directive(child, "if", false) {
            has_dynamic_slots = true;
            dynamic_slots.push(json!({
                "kind": "conditional",
                "test": vue3_slot_condition_projection(if_dir, context),
                "consequent": vue3_dynamic_slot_projection(slot_name, slot, Some(conditional_branch_index)),
                "alternate": vue3_default_fallback_projection(),
            }));
            conditional_branch_index += 1;
            continue;
        }

        if let Some(else_dir) = vue3_else_slot_directive(child) {
            if let Some(previous) = vue3_previous_non_comment_or_whitespace(children, index) {
                if vue3_template_has_if_like_slot_directive(previous) {
                    let alternate = if json_str(else_dir, "name") == Some("else-if") {
                        json!({
                            "kind": "conditional",
                            "test": vue3_slot_condition_projection(else_dir, context),
                            "consequent": vue3_dynamic_slot_projection(slot_name, slot, Some(conditional_branch_index)),
                            "alternate": vue3_default_fallback_projection(),
                        })
                    } else {
                        vue3_dynamic_slot_projection(
                            slot_name,
                            slot,
                            Some(conditional_branch_index),
                        )
                    };
                    vue3_append_slot_conditional_alternate(&mut dynamic_slots, alternate);
                    conditional_branch_index += 1;
                } else {
                    errors.push(json!({ "code": 30, "loc": else_dir.get("loc").cloned().unwrap_or(Value::Null) }));
                }
            } else {
                errors.push(json!({ "code": 30, "loc": else_dir.get("loc").cloned().unwrap_or(Value::Null) }));
            }
            continue;
        }

        if let Some(for_dir) = vue3_directive(child, "for", true) {
            has_dynamic_slots = true;
            let parsed_projection = vue3_slot_for_parse_result_projection(child, for_dir, context);
            if let Some(parse_result) = parsed_projection.get("parseResult") {
                dynamic_slots.push(json!({
                    "kind": "for",
                    "source": parse_result["source"].clone(),
                    "params": {
                        "value": parse_result["value"].clone(),
                        "key": parse_result["key"].clone(),
                        "index": parse_result["index"].clone(),
                    },
                    "slot": vue3_dynamic_slot_projection(slot_name, slot, None),
                }));
            } else {
                errors.push(json!({ "code": 32, "loc": for_dir.get("loc").cloned().unwrap_or(Value::Null) }));
            }
            continue;
        }

        if let Some(name) = static_slot_name {
            if seen_slot_names.iter().any(|seen| seen == &name) {
                errors.push(json!({ "code": 38, "loc": slot_dir.get("loc").cloned().unwrap_or(Value::Null) }));
                continue;
            }
            if name == "default" {
                has_named_default_slot = true;
            }
            seen_slot_names.push(name);
        }
        properties.push(json!({
            "kind": "property",
            "key": slot_name,
            "params": slot_dir.get("exp").cloned().unwrap_or(Value::Null),
            "indices": [index],
            "unwrapTemplate": true,
            "loc": child.get("loc").cloned().unwrap_or_else(|| node.get("loc").cloned().unwrap_or(Value::Null)),
        }));
    }

    if on_component_slot.is_none() {
        if !has_template_slots {
            properties.push(json!({
                "kind": "property",
                "key": vue3_static_slot_key("default"),
                "params": Value::Null,
                "indices": vue3_all_child_indices(children),
                "loc": node.get("loc").cloned().unwrap_or(Value::Null),
                "nonScoped": true,
            }));
        } else if !implicit_default_indices.is_empty()
            && !vue3_all_indices_are_whitespace_text(children, &implicit_default_indices)
        {
            if has_named_default_slot {
                if let Some(child) = implicit_default_indices
                    .first()
                    .and_then(|index| children.get(*index))
                {
                    errors.push(json!({ "code": 39, "loc": child.get("loc").cloned().unwrap_or(Value::Null) }));
                }
            } else {
                properties.push(json!({
                    "kind": "property",
                    "key": vue3_static_slot_key("default"),
                    "params": Value::Null,
                    "indices": implicit_default_indices,
                    "loc": node.get("loc").cloned().unwrap_or(Value::Null),
                    "nonScoped": true,
                }));
            }
        }
    }

    let slot_flag = if has_dynamic_slots {
        2
    } else if vue3_has_forwarded_slots(children) {
        3
    } else {
        1
    };

    json!({
        "properties": properties,
        "dynamicSlots": dynamic_slots,
        "slotFlag": slot_flag,
        "slotFlagText": vue3_slot_flag_text(slot_flag),
        "hasDynamicSlots": has_dynamic_slots,
        "errors": errors,
    })
}

fn vue3_for_parse_result_projection(node: &Value, dir: &Value, context: &Value) -> Value {
    transform_for_projection(&json!({
        "node": node,
        "dir": dir,
        "context": context,
    }))
}

fn vue3_slot_for_parse_result_projection(node: &Value, dir: &Value, context: &Value) -> Value {
    if let Some(parse_result) = dir.get("forParseResult").filter(|value| !value.is_null()) {
        return json!({
            "parseResult": {
                "source": parse_result.get("source").cloned().unwrap_or(Value::Null),
                "value": parse_result.get("value").cloned().unwrap_or(Value::Null),
                "key": parse_result.get("key").cloned().unwrap_or(Value::Null),
                "index": parse_result.get("index").cloned().unwrap_or(Value::Null),
                "finalized": parse_result.get("finalized").and_then(Value::as_bool).unwrap_or(true),
            }
        });
    }
    vue3_for_parse_result_projection(node, dir, context)
}

fn vue3_directive<'a>(node: &'a Value, name: &str, allow_empty: bool) -> Option<&'a Value> {
    node.get("props")
        .and_then(Value::as_array)?
        .iter()
        .find(|prop| {
            json_node_type(prop) == Some(7)
                && json_str(prop, "name") == Some(name)
                && (allow_empty || prop.get("exp").is_some_and(|exp| !exp.is_null()))
        })
}

fn vue3_slot_directive(node: &Value, allow_empty: bool) -> Option<&Value> {
    vue3_directive(node, "slot", allow_empty)
}

fn vue3_template_slot_directive(node: &Value) -> Option<&Value> {
    if json_node_type(node) == Some(1) && json_u64(node, "tagType") == Some(3) {
        vue3_slot_directive(node, true)
    } else {
        None
    }
}

fn vue3_else_slot_directive(node: &Value) -> Option<&Value> {
    node.get("props")
        .and_then(Value::as_array)?
        .iter()
        .find(|prop| {
            json_node_type(prop) == Some(7)
                && matches!(json_str(prop, "name"), Some("else") | Some("else-if"))
        })
}

fn vue3_template_has_if_like_slot_directive(node: &Value) -> bool {
    vue3_template_slot_directive(node).is_some()
        && node
            .get("props")
            .and_then(Value::as_array)
            .is_some_and(|props| {
                props.iter().any(|prop| {
                    json_node_type(prop) == Some(7)
                        && matches!(json_str(prop, "name"), Some("if") | Some("else-if"))
                })
            })
}

fn vue3_previous_non_comment_or_whitespace(children: &[Value], index: usize) -> Option<&Value> {
    children[..index]
        .iter()
        .rev()
        .find(|child| !vue3_is_comment_or_whitespace(child))
}

fn vue3_is_comment_or_whitespace(node: &Value) -> bool {
    json_node_type(node) == Some(3) || vue3_is_whitespace_text(node)
}

fn vue3_is_whitespace_text(node: &Value) -> bool {
    match json_node_type(node) {
        Some(2) => json_str(node, "content").is_some_and(|content| {
            content
                .chars()
                .all(|ch| matches!(ch, '\t' | '\r' | '\n' | '\u{000C}' | ' '))
        }),
        Some(12) => node.get("content").is_some_and(vue3_is_whitespace_text),
        _ => false,
    }
}

fn vue3_all_indices_are_whitespace_text(children: &[Value], indices: &[usize]) -> bool {
    indices
        .iter()
        .filter_map(|index| children.get(*index))
        .all(vue3_is_whitespace_text)
}

fn vue3_all_child_indices(children: &[Value]) -> Vec<usize> {
    (0..children.len()).collect()
}

fn vue3_slot_name_projection(slot: &Value, context: &Value) -> Value {
    let Some(arg) = slot.get("arg").filter(|arg| !arg.is_null()) else {
        return vue3_static_slot_key("default");
    };
    if json_bool(arg, "isStatic") {
        return vue3_static_slot_key(json_str(arg, "content").unwrap_or("default"));
    }
    let _ = context;
    arg.clone()
}

fn vue3_static_slot_name(slot: &Value) -> Option<String> {
    let Some(arg) = slot.get("arg").filter(|arg| !arg.is_null()) else {
        return Some("default".to_string());
    };
    json_bool(arg, "isStatic").then(|| json_str(arg, "content").unwrap_or("default").to_string())
}

fn vue3_static_slot_key(name: &str) -> Value {
    json!({
        "kind": "simple",
        "content": name,
        "isStatic": true,
        "constType": 3,
    })
}

fn vue3_slot_param_locals(exp: &Value) -> Vec<String> {
    let source = model_expression_source(exp);
    vue3_for_alias_locals(source.trim())
}

fn vue3_slot_condition_projection(dir: &Value, context: &Value) -> Value {
    let Some(exp) = dir.get("exp").filter(|exp| !exp.is_null()) else {
        return json!({ "kind": "undefined" });
    };
    let _ = context;
    exp.clone()
}

fn vue3_slot_function_projection(slot_dir: &Value, indices: &[usize], child: &Value) -> Value {
    json!({
        "kind": "slotFunction",
        "params": slot_dir.get("exp").cloned().unwrap_or(Value::Null),
        "indices": indices,
        "unwrapTemplate": true,
        "loc": child.get("loc").cloned().unwrap_or(Value::Null),
    })
}

fn vue3_dynamic_slot_projection(name: Value, slot: Value, key: Option<usize>) -> Value {
    let mut value = json!({
        "kind": "dynamicSlot",
        "name": name,
        "slot": slot,
    });
    if let Some(key) = key {
        value["key"] = json!(key.to_string());
    }
    value
}

fn vue3_default_fallback_projection() -> Value {
    json!({
        "kind": "simple",
        "content": "undefined",
        "isStatic": false,
        "constType": 0,
    })
}

fn vue3_append_slot_conditional_alternate(dynamic_slots: &mut [Value], alternate: Value) {
    let Some(last) = dynamic_slots.last_mut() else {
        return;
    };
    let mut target = last;
    loop {
        let nested = target
            .get("alternate")
            .and_then(|value| value.get("kind"))
            .and_then(Value::as_str)
            == Some("conditional");
        if !nested {
            target["alternate"] = alternate;
            break;
        }
        target = target.get_mut("alternate").expect("checked alternate");
    }
}

fn vue3_slot_flag_text(flag: u8) -> &'static str {
    match flag {
        1 => "STABLE",
        2 => "DYNAMIC",
        3 => "FORWARDED",
        _ => "",
    }
}

fn vue3_has_forwarded_slots(children: &[Value]) -> bool {
    children.iter().any(|child| match json_node_type(child) {
        Some(1) => {
            json_u64(child, "tagType") == Some(2)
                || child
                    .get("children")
                    .and_then(Value::as_array)
                    .is_some_and(|children| vue3_has_forwarded_slots(children))
        }
        Some(9) => child
            .get("branches")
            .and_then(Value::as_array)
            .is_some_and(|branches| vue3_has_forwarded_slots(branches)),
        Some(10) | Some(11) => child
            .get("children")
            .and_then(Value::as_array)
            .is_some_and(|children| vue3_has_forwarded_slots(children)),
        _ => false,
    })
}

fn vue3_component_slot_scope_ref(node: &Value, children: &[Value], context: &Value) -> bool {
    let mut names = transform_context_locals(context);
    if let Some(slot) = vue3_slot_directive(node, false) {
        if let Some(exp) = slot.get("exp").filter(|exp| !exp.is_null()) {
            let slot_locals = vue3_slot_param_locals(exp);
            names.retain(|name| !slot_locals.iter().any(|local| local == name));
        }
    }
    if names.is_empty() {
        return false;
    }
    node.get("props")
        .and_then(Value::as_array)
        .is_some_and(|props| {
            props.iter().any(|prop| {
                json_str(prop, "name") == Some("slot")
                    && (prop
                        .get("arg")
                        .is_some_and(|arg| vue3_node_source_contains_any(arg, &names))
                        || prop
                            .get("exp")
                            .is_some_and(|exp| vue3_node_source_contains_any(exp, &names)))
            })
        })
        || children
            .iter()
            .any(|child| vue3_node_source_contains_any(child, &names))
}

fn vue3_node_source_contains_any(node: &Value, names: &[String]) -> bool {
    if node.is_null() {
        return false;
    }
    if json_node_type(node) != Some(11) {
        if let Some(content) = json_str(node, "content") {
            if names
                .iter()
                .any(|name| source_contains_identifier(content, name))
            {
                return true;
            }
        }
    }
    if !matches!(
        json_node_type(node),
        Some(1) | Some(9) | Some(10) | Some(11)
    ) {
        if let Some(source) = node.get("loc").and_then(|loc| json_str(loc, "source")) {
            if names
                .iter()
                .any(|name| source_contains_identifier(source, name))
            {
                return true;
            }
        }
    }
    if let Some(children) = node.get("children").and_then(Value::as_array) {
        if children
            .iter()
            .any(|child| vue3_node_source_contains_any(child, names))
        {
            return true;
        }
    }
    if matches!(json_node_type(node), Some(1)) {
        if let Some(props) = node.get("props").and_then(Value::as_array) {
            if props
                .iter()
                .filter(|prop| json_str(prop, "name") != Some("for"))
                .any(|prop| vue3_node_source_contains_any(prop, names))
            {
                return true;
            }
        }
    }
    false
}

fn source_contains_identifier(source: &str, name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let mut search_start = 0usize;
    while let Some(offset) = source[search_start..].find(name) {
        let start = search_start + offset;
        let end = start + name.len();
        let before = source[..start].chars().next_back();
        let after = source[end..].chars().next();
        if before.is_none_or(|ch| !is_identifier_continue(ch))
            && after.is_none_or(|ch| !is_identifier_continue(ch))
        {
            return true;
        }
        search_start = end;
    }
    false
}

fn transform_for_codegen_projection(payload: &Value) -> Value {
    let node = payload.get("node").unwrap_or(&Value::Null);
    let for_node = payload.get("forNode").unwrap_or(&Value::Null);
    let context = payload.get("context").unwrap_or(&Value::Null);
    let source = for_node.get("source").unwrap_or(&Value::Null);
    let is_stable_fragment = json_node_type(source) == Some(4)
        && json_u64(source, "constType").is_some_and(|value| value > 0);
    let key_projection = vue3_for_key_property_projection(node, context);
    json!({
        "keyProperty": key_projection,
        "fragmentFlag": if is_stable_fragment {
            64
        } else if !key_projection.is_null() {
            128
        } else {
            256
        },
        "disableTracking": !is_stable_fragment,
        "isStableFragment": is_stable_fragment,
    })
}

fn transform_for_exit_codegen_projection(payload: &Value) -> Value {
    let node = payload.get("node").unwrap_or(&Value::Null);
    let for_node = payload.get("forNode").unwrap_or(&Value::Null);
    let children = for_node
        .get("children")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    if vue3_for_is_slot_outlet_summary(node) {
        return json!({ "kind": "slotOutlet", "path": "node" });
    }
    if json_u64(node, "tagType") == Some(3)
        && node
            .get("children")
            .and_then(Value::as_array)
            .is_some_and(|children| {
                children.len() == 1 && vue3_for_is_slot_outlet_summary(&children[0])
            })
    {
        return json!({ "kind": "slotOutlet", "path": "templateChild", "index": 0 });
    }
    let need_fragment_wrapper =
        children.len() != 1 || children.first().and_then(json_node_type) != Some(1);
    if need_fragment_wrapper {
        return json!({ "kind": "fragmentWrapper", "patchFlag": 64 });
    }
    json!({
        "kind": "singleElement",
        "childBlockIsBlock": !json_bool(payload, "isStableFragment"),
    })
}

fn vue3_for_is_slot_outlet_summary(node: &Value) -> bool {
    json_node_type(node) == Some(1) && json_u64(node, "tagType") == Some(2)
}

fn vue3_for_key_property_projection(node: &Value, context: &Value) -> Value {
    let Some((prop, is_directive)) = vue3_for_key_prop(node) else {
        return Value::Null;
    };
    let value = if is_directive {
        let Some(exp) = prop.get("exp").filter(|value| !value.is_null()) else {
            return Value::Null;
        };
        let raw = json_str(exp, "content")
            .or_else(|| exp.get("loc").and_then(|loc| json_str(loc, "source")))
            .unwrap_or("");
        if json_bool(context, "prefixIdentifiers") {
            let options = vue3_options_from_transform_context(context);
            let locals = transform_context_locals(context);
            vue3_for_rewrite_projection_node(
                raw,
                &options,
                &locals,
                exp.get("loc").cloned().unwrap_or(Value::Null),
                Vue3ForAstMode::Expression,
                false,
            )
        } else {
            vue3_for_expression_projection(raw, exp, 0, raw.len(), Vue3ForAstMode::Expression)
        }
    } else {
        let Some(value) = prop.get("value").filter(|value| !value.is_null()) else {
            return Value::Null;
        };
        let content = json_str(value, "content").unwrap_or("");
        json!({
            "kind": "simple",
            "content": content,
            "isStatic": true,
            "constType": 3,
            "loc": value.get("loc").cloned().unwrap_or_else(|| prop.get("loc").cloned().unwrap_or(Value::Null)),
            "astMode": "expression",
        })
    };
    json!({ "value": value })
}

fn vue3_for_key_prop(node: &Value) -> Option<(&Value, bool)> {
    node.get("props")
        .and_then(Value::as_array)?
        .iter()
        .find_map(|prop| match json_node_type(prop) {
            Some(6) if json_str(prop, "name") == Some("key") => Some((prop, false)),
            Some(7)
                if json_str(prop, "name") == Some("bind")
                    && prop.get("arg").is_some_and(|arg| {
                        json_str(arg, "content") == Some("key") && json_bool(arg, "isStatic")
                    }) =>
            {
                Some((prop, true))
            }
            _ => None,
        })
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Vue3ForParsed {
    source: Vue3ForPart,
    value: Option<Vue3ForPart>,
    key: Option<Vue3ForPart>,
    index: Option<Vue3ForPart>,
}

impl Vue3ForParsed {
    fn all_alias_locals(&self) -> Vec<String> {
        let mut locals = Vec::new();
        for part in [&self.value, &self.key, &self.index].into_iter().flatten() {
            for local in vue3_for_alias_locals(&part.content) {
                if !locals.iter().any(|existing| existing == &local) {
                    locals.push(local);
                }
            }
        }
        locals
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Vue3ForPart {
    content: String,
    start: usize,
    end: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Vue3ForAstMode {
    Expression,
    Params,
}

fn parse_vue3_for_expression(source: &str) -> Option<Vue3ForParsed> {
    let Vue3ForMatch { lhs_end, rhs_start } = find_vue3_for_match(source)?;
    let rhs_end = trim_end_offset(source, rhs_start, source.len());
    if rhs_start >= rhs_end {
        return None;
    }
    let (alias_start, alias_end) = vue3_for_alias_content_span(source, 0, lhs_end);
    let aliases = split_vue3_for_aliases(source, alias_start, alias_end);
    Some(Vue3ForParsed {
        source: Vue3ForPart {
            content: source[rhs_start..rhs_end].to_string(),
            start: rhs_start,
            end: rhs_end,
        },
        value: aliases.first().and_then(|segment| segment.part(source)),
        key: aliases.get(1).and_then(|segment| segment.part(source)),
        index: aliases.get(2).and_then(|segment| segment.part(source)),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Vue3ForMatch {
    lhs_end: usize,
    rhs_start: usize,
}

fn find_vue3_for_match(source: &str) -> Option<Vue3ForMatch> {
    for (operator_start, _) in source.char_indices() {
        let operator_len = if source[operator_start..].starts_with("in") {
            2
        } else if source[operator_start..].starts_with("of") {
            2
        } else {
            continue;
        };
        if operator_start == 0 || !previous_char_is_whitespace(source, operator_start) {
            continue;
        }
        let after_operator = operator_start + operator_len;
        if !source[after_operator..]
            .chars()
            .next()
            .is_some_and(char::is_whitespace)
        {
            continue;
        }
        let Some(rhs_start) = source[after_operator..]
            .char_indices()
            .find(|(_, ch)| !ch.is_whitespace())
            .map(|(index, _)| after_operator + index)
        else {
            continue;
        };
        let lhs_end = trim_end_offset(source, 0, operator_start);
        return Some(Vue3ForMatch { lhs_end, rhs_start });
    }
    None
}

fn previous_char_is_whitespace(source: &str, offset: usize) -> bool {
    source[..offset]
        .chars()
        .next_back()
        .is_some_and(char::is_whitespace)
}

fn vue3_for_alias_content_span(source: &str, start: usize, end: usize) -> (usize, usize) {
    let mut start = trim_start_offset(source, start, end);
    let mut end = trim_end_offset(source, start, end);
    if source[start..end].starts_with('(') && source[start..end].ends_with(')') {
        start += '('.len_utf8();
        end = end.saturating_sub(')'.len_utf8());
    }
    start = trim_start_offset(source, start, end);
    end = trim_end_offset(source, start, end);
    (start, end)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Vue3ForAliasSegment {
    start: usize,
    end: usize,
}

impl Vue3ForAliasSegment {
    fn part(&self, source: &str) -> Option<Vue3ForPart> {
        let start = trim_start_offset(source, self.start, self.end);
        let end = trim_end_offset(source, start, self.end);
        (start < end).then(|| Vue3ForPart {
            content: source[start..end].to_string(),
            start,
            end,
        })
    }
}

fn split_vue3_for_aliases(
    source: &str,
    alias_start: usize,
    alias_end: usize,
) -> Vec<Vue3ForAliasSegment> {
    let mut items = Vec::new();
    let mut depth = 0usize;
    let mut quote = None::<char>;
    let mut start = alias_start;
    let mut escaped = false;
    for (index, ch) in source[alias_start..alias_end].char_indices() {
        let index = alias_start + index;
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' | '`' => quote = Some(ch),
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                items.push(Vue3ForAliasSegment { start, end: index });
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }
    items.push(Vue3ForAliasSegment {
        start,
        end: alias_end,
    });
    items
}

fn trim_start_offset(source: &str, start: usize, end: usize) -> usize {
    source[start..end]
        .char_indices()
        .find(|(_, ch)| !ch.is_whitespace())
        .map(|(index, _)| start + index)
        .unwrap_or(end)
}

fn trim_end_offset(source: &str, start: usize, end: usize) -> usize {
    let mut trimmed = end;
    for (index, ch) in source[start..end].char_indices().rev() {
        if !ch.is_whitespace() {
            trimmed = start + index + ch.len_utf8();
            break;
        }
        trimmed = start + index;
    }
    trimmed
}

fn vue3_for_expression_projection(
    content: &str,
    exp: &Value,
    start: usize,
    end: usize,
    ast_mode: Vue3ForAstMode,
) -> Value {
    json!({
        "kind": "simple",
        "content": content,
        "isStatic": false,
        "constType": 0,
        "loc": vue3_for_exp_loc(exp, start, end),
        "astMode": vue3_for_ast_mode_name(ast_mode),
    })
}

fn vue3_for_rewrite_projection_node(
    raw: &str,
    options: &Vue3CompilerOptions,
    locals: &[String],
    loc: Value,
    ast_mode: Vue3ForAstMode,
    force_compound_for_complex: bool,
) -> Value {
    let rewritten = if locals.is_empty() {
        rewrite_js_like_expression(raw, options)
    } else {
        rewrite_js_like_expression_with_locals(raw, options, locals)
    };
    let children = vue3_for_compound_children(raw, options, locals, ast_mode, &loc);
    let is_simple = is_simple_identifier_ascii(raw.trim())
        || children.is_empty()
        || (!force_compound_for_complex && rewritten == raw.trim());
    if is_simple {
        return vue3_for_simple_projection(
            rewritten.trim(),
            loc,
            vue3_for_const_type(rewritten.trim()),
            ast_mode,
        );
    }
    let helpers = vue3_for_helpers_for_content(&rewritten);
    let mut value = json!({
        "kind": "compound",
        "children": children,
        "loc": loc,
        "astMode": vue3_for_ast_mode_name(ast_mode),
    });
    if !helpers.is_empty() {
        value["helpers"] = json!(helpers);
    }
    value
}

fn vue3_for_simple_projection(
    content: &str,
    loc: Value,
    const_type: u8,
    ast_mode: Vue3ForAstMode,
) -> Value {
    let mut value = json!({
        "kind": "simple",
        "content": content,
        "isStatic": false,
        "constType": const_type,
        "loc": loc,
        "astMode": vue3_for_ast_mode_name(ast_mode),
    });
    let helpers = vue3_for_helpers_for_content(content);
    if !helpers.is_empty() {
        value["helpers"] = json!(helpers);
    }
    value
}

fn vue3_for_compound_children(
    raw: &str,
    options: &Vue3CompilerOptions,
    locals: &[String],
    ast_mode: Vue3ForAstMode,
    loc: &Value,
) -> Vec<Value> {
    let mut children = Vec::new();
    let mut quote = None::<char>;
    let mut escaped = false;
    let mut index = 0usize;
    let mut last = 0usize;
    let chars = raw.char_indices().collect::<Vec<_>>();
    while index < chars.len() {
        let start = chars[index].0;
        let ch = chars[index].1;
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
            index += 1;
            continue;
        }
        if matches!(ch, '\'' | '"' | '`') {
            quote = Some(ch);
            index += 1;
            continue;
        }
        if !is_identifier_start(ch) {
            index += 1;
            continue;
        }
        index += 1;
        while index < chars.len() && is_identifier_continue(chars[index].1) {
            index += 1;
        }
        let end = chars.get(index).map_or(raw.len(), |(offset, _)| *offset);
        let ident = &raw[start..end];
        let Some(replacement) = vue3_for_identifier_projection_content(
            raw, start, end, ident, options, locals, ast_mode,
        ) else {
            continue;
        };
        if last < start {
            children.push(json!(raw[last..start]));
        }
        children.push(vue3_for_simple_projection(
            &replacement,
            vue3_for_child_loc(loc, raw, start, end),
            if replacement == ident {
                3
            } else {
                vue3_for_const_type(&replacement)
            },
            ast_mode,
        ));
        last = end;
    }
    if last < raw.len() {
        children.push(json!(raw[last..].to_string()));
    }
    children
}

fn vue3_for_identifier_projection_content(
    raw: &str,
    start: usize,
    end: usize,
    ident: &str,
    options: &Vue3CompilerOptions,
    locals: &[String],
    ast_mode: Vue3ForAstMode,
) -> Option<String> {
    if is_keyword(ident) || is_global_or_literal(ident) {
        return None;
    }
    let prev = previous_non_ws(raw, start);
    let next = next_non_ws(raw, end);
    if next == Some(':') {
        return None;
    }
    if prev == Some('.') {
        return Some(ident.to_string());
    }
    if locals.iter().any(|local| local == ident) {
        return Some(ident.to_string());
    }
    if ast_mode == Vue3ForAstMode::Params && next == Some('=') {
        return Some(ident.to_string());
    }
    Some(rewrite_identifier(ident, options))
}

fn previous_non_ws(source: &str, offset: usize) -> Option<char> {
    source[..offset]
        .chars()
        .rev()
        .find(|ch| !ch.is_whitespace())
}

fn vue3_for_exp_loc(exp: &Value, start: usize, end: usize) -> Value {
    let loc = exp.get("loc").unwrap_or(&Value::Null);
    let source = json_str(loc, "source")
        .or_else(|| json_str(exp, "content"))
        .unwrap_or("");
    vue3_for_loc_from_start(loc.get("start").unwrap_or(&Value::Null), source, start, end)
}

fn vue3_for_child_loc(parent_loc: &Value, source: &str, start: usize, end: usize) -> Value {
    vue3_for_loc_from_start(
        parent_loc.get("start").unwrap_or(&Value::Null),
        source,
        start,
        end,
    )
}

fn vue3_for_loc_from_start(start_pos: &Value, source: &str, start: usize, end: usize) -> Value {
    let start = start.min(source.len());
    let end = end.min(source.len()).max(start);
    json!({
        "start": vue3_for_advance_position(start_pos, source, start),
        "end": vue3_for_advance_position(start_pos, source, end),
        "source": source.get(start..end).unwrap_or_default(),
    })
}

fn vue3_for_advance_position(start_pos: &Value, source: &str, amount: usize) -> Value {
    let mut offset = start_pos
        .get("offset")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    let mut line = start_pos.get("line").and_then(Value::as_i64).unwrap_or(1);
    let mut column = start_pos.get("column").and_then(Value::as_i64).unwrap_or(1);
    let mut index = 0usize;
    for ch in source.chars() {
        if index >= amount {
            break;
        }
        let len = ch.len_utf8();
        if index + len > amount {
            offset += (amount - index) as i64;
            column += (amount - index) as i64;
            return json!({ "offset": offset, "line": line, "column": column });
        }
        index += len;
        offset += len as i64;
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    if amount > index {
        offset += (amount - index) as i64;
        column += (amount - index) as i64;
    }
    json!({ "offset": offset, "line": line, "column": column })
}

fn vue3_for_ast_mode_name(mode: Vue3ForAstMode) -> &'static str {
    match mode {
        Vue3ForAstMode::Expression => "expression",
        Vue3ForAstMode::Params => "params",
    }
}

fn vue3_for_const_type(content: &str) -> u8 {
    let content = content.trim();
    if matches!(content, "true" | "false" | "null") {
        return 3;
    }
    if (content.starts_with('"') && content.ends_with('"'))
        || (content.starts_with('\'') && content.ends_with('\''))
        || content.parse::<f64>().is_ok()
    {
        return 3;
    }
    0
}

fn vue3_for_helpers_for_content(content: &str) -> Vec<&'static str> {
    let mut helpers = Vec::new();
    if content.contains("_unref(") {
        helpers.push("UNREF");
    }
    if content.contains("_isRef(") {
        helpers.push("IS_REF");
    }
    helpers
}

fn vue3_for_alias_locals(alias: &str) -> Vec<String> {
    let store = JsAstStore::new();
    let wrapped = format!("({alias})=>{{}}");
    if let Ok(Expression::ArrowFunctionExpression(function)) =
        store.parse_expression(&wrapped, oxc_span::SourceType::ts())
    {
        let mut locals = Vec::new();
        for param in &function.params.items {
            collect_vue3_for_binding_pattern(&param.pattern, &mut locals);
        }
        if let Some(rest) = &function.params.rest {
            collect_vue3_for_binding_pattern(&rest.rest.argument, &mut locals);
        }
        locals.sort();
        locals.dedup();
        return locals;
    }
    extract_v_for_alias_locals(alias)
}

fn collect_vue3_for_binding_pattern(pattern: &BindingPattern<'_>, locals: &mut Vec<String>) {
    match pattern {
        BindingPattern::BindingIdentifier(identifier) => {
            locals.push(identifier.name.to_string());
        }
        BindingPattern::ObjectPattern(object) => {
            for property in &object.properties {
                collect_vue3_for_binding_pattern(&property.value, locals);
            }
            if let Some(rest) = &object.rest {
                collect_vue3_for_binding_pattern(&rest.argument, locals);
            }
        }
        BindingPattern::ArrayPattern(array) => {
            for element in array.elements.iter().flatten() {
                collect_vue3_for_binding_pattern(element, locals);
            }
            if let Some(rest) = &array.rest {
                collect_vue3_for_binding_pattern(&rest.argument, locals);
            }
        }
        BindingPattern::AssignmentPattern(assignment) => {
            collect_vue3_for_binding_pattern(&assignment.left, locals);
        }
    }
}

fn vue3_for_template_key_errors(node: &Value) -> Vec<Value> {
    if json_u64(node, "tagType") != Some(3) {
        return Vec::new();
    }
    node.get("children")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|child| json_node_type(child) == Some(1))
        .filter(|child| !vue3_for_child_has_structural_directive(child))
        .filter_map(vue3_for_child_key_loc)
        .take(1)
        .map(|loc| json!({ "code": 33, "loc": loc }))
        .collect()
}

fn vue3_for_child_has_structural_directive(node: &Value) -> bool {
    node.get("props")
        .and_then(Value::as_array)
        .is_some_and(|props| {
            props.iter().any(|prop| {
                json_node_type(prop) == Some(7)
                    && matches!(
                        json_str(prop, "name"),
                        Some("for" | "if" | "else" | "else-if")
                    )
            })
        })
}

fn vue3_for_child_key_loc(node: &Value) -> Option<Value> {
    node.get("props")
        .and_then(Value::as_array)?
        .iter()
        .find(|prop| match json_node_type(prop) {
            Some(6) => json_str(prop, "name") == Some("key"),
            Some(7) => {
                json_str(prop, "name") == Some("bind")
                    && prop.get("arg").is_some_and(|arg| {
                        json_str(arg, "content") == Some("key") && json_bool(arg, "isStatic")
                    })
            }
            _ => false,
        })
        .and_then(|prop| prop.get("loc").cloned())
}

pub fn resolve_component_type_projection(payload: &Value) -> Value {
    let node = payload.get("node").unwrap_or(&Value::Null);
    let context = payload.get("context").unwrap_or(&Value::Null);
    let ssr = json_bool(payload, "ssr");
    let mut tag = json_str(node, "tag").unwrap_or("").to_string();
    let is_explicit_dynamic = matches!(tag.as_str(), "component" | "Component");
    let is_prop = resolve_component_is_prop(node);

    if let Some(is_prop) = is_prop {
        if is_explicit_dynamic || json_bool(context, "compatIsOnElement") {
            if let Some(exp) = resolve_component_is_prop_expression(is_prop, context) {
                return json!({
                    "kind": "dynamic",
                    "helper": "RESOLVE_DYNAMIC_COMPONENT",
                    "argument": exp,
                });
            }
        } else if json_node_type(is_prop) == Some(6)
            && is_prop
                .get("value")
                .and_then(|value| json_str(value, "content"))
                .is_some_and(|value| value.starts_with("vue:"))
        {
            tag = is_prop
                .get("value")
                .and_then(|value| json_str(value, "content"))
                .map(|value| value[4..].to_string())
                .unwrap_or(tag);
        }
    }

    if let Some(helper) = vue3_core_component_helper(&tag) {
        return json!({
            "kind": "helper",
            "helper": helper,
            "registerHelper": !ssr,
        });
    }
    if let Some(projection) = context
        .get("builtInComponents")
        .and_then(Value::as_array)
        .and_then(|components| {
            components.iter().find_map(|component| {
                if component.as_str() == Some(&tag) {
                    return Some(json!({
                        "kind": "helper",
                        "helper": tag,
                        "registerHelper": !ssr,
                    }));
                }
                let component_tag = component.get("tag").and_then(Value::as_str)?;
                (component_tag == tag).then(|| {
                    json!({
                        "kind": "helper",
                        "helperName": component.get("helperName").and_then(Value::as_str).unwrap_or(component_tag),
                        "registerHelper": !ssr,
                    })
                })
            })
        })
    {
        return projection;
    }

    if let Some(from_setup) = resolve_setup_reference(&tag, context) {
        return from_setup;
    }
    if let Some(dot_index) = tag.find('.') {
        if dot_index > 0 {
            if let Some(mut namespace) = resolve_setup_reference(&tag[..dot_index], context) {
                if let Some(content) = json_str(&namespace, "content") {
                    let resolved = format!("{}{}", content, &tag[dot_index..]);
                    namespace["content"] = json!(resolved);
                    return namespace;
                }
            }
        }
    }

    let self_name = json_str(context, "selfName");
    let component_name =
        if self_name.is_some_and(|self_name| capitalize(&camelize(&tag)) == self_name) {
            format!("{tag}__self")
        } else {
            tag.clone()
        };
    json!({
        "kind": "asset",
        "helper": "RESOLVE_COMPONENT",
        "component": component_name,
        "assetId": component_asset_id(&tag),
    })
}

pub fn transform_element_props_projection(payload: &Value) -> Value {
    let props = payload
        .get("props")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let context = payload.get("context").unwrap_or(&Value::Null);
    let has_children = json_bool(payload, "hasChildren");
    let is_component = json_bool(payload, "isComponent");
    let is_dynamic_component = json_bool(payload, "isDynamicComponent");
    let in_ssr = json_bool(context, "inSSR");
    let in_v_for = context
        .get("vForDepth")
        .and_then(Value::as_u64)
        .is_some_and(|depth| depth > 0);
    let inline_template_refs = inline_template_ref_projections(props, context);
    let mut patch_flag = 0u16;
    let mut dynamic_prop_names = Vec::<String>::new();
    let mut has_ref = false;
    let mut has_class_binding = false;
    let mut has_style_binding = false;
    let mut has_hydration_event_binding = false;
    let mut has_dynamic_keys = false;
    let mut has_vnode_hook = false;
    let mut should_use_block = false;
    let mut normalize_props = false;
    let mut guard_reactive_props = false;
    let mut normalize_class = false;
    let mut normalize_style = false;
    let mut has_runtime_directives = false;
    let mut has_dynamic_object = false;
    let mut has_normalize_dynamic_keys = false;
    let ref_for_marker = in_v_for
        && props.iter().any(|prop| {
            (matches!(
                json_str(prop, "kind"),
                Some("attribute") | Some("directiveProp")
            ) && json_str(prop, "name") == Some("ref"))
                || json_str(prop, "kind") == Some("objectBind")
        });

    for prop in props {
        match json_str(prop, "kind") {
            Some("attribute") => {
                if json_str(prop, "name") == Some("ref") {
                    has_ref = true;
                }
            }
            Some("objectBind") => {
                has_dynamic_keys = true;
                has_normalize_dynamic_keys = true;
                has_dynamic_object = true;
            }
            Some("objectOn") => {
                has_dynamic_keys = true;
                has_normalize_dynamic_keys = true;
                has_dynamic_object = true;
            }
            Some("runtimeDirective") => {
                has_runtime_directives = true;
                if has_children {
                    should_use_block = true;
                }
            }
            Some("directiveProp") => {
                if json_bool(prop, "dynamicKey") {
                    has_dynamic_keys = true;
                    if !json_bool(prop, "ignoreDynamicKeyForNormalize") {
                        has_normalize_dynamic_keys = true;
                    }
                } else if let Some(name) = json_str(prop, "name") {
                    let value_constant = json_bool(prop, "valueConstant");
                    let value_cached = json_bool(prop, "valueCached");
                    let is_event = prop_name_is_event_handler(name);
                    if is_event
                        && (!is_component || is_dynamic_component)
                        && name.to_ascii_lowercase() != "onclick"
                        && name != "onUpdate:modelValue"
                        && !prop_name_is_reserved(name)
                    {
                        has_hydration_event_binding = true;
                    }
                    if is_event && prop_name_is_reserved(name) {
                        has_vnode_hook = true;
                    }
                    if !value_cached && !value_constant {
                        if name == "ref" {
                            has_ref = true;
                        } else if name == "class" {
                            has_class_binding = true;
                        } else if name == "style" {
                            has_style_binding = true;
                        } else if name != "key"
                            && !dynamic_prop_names.iter().any(|existing| existing == name)
                        {
                            dynamic_prop_names.push(name.to_string());
                        }
                        if is_component
                            && matches!(name, "class" | "style")
                            && !dynamic_prop_names.iter().any(|existing| existing == name)
                        {
                            dynamic_prop_names.push(name.to_string());
                        }
                    }
                }
                if json_bool(prop, "propModifier") {
                    patch_flag |= 32;
                }
                if json_bool(prop, "forceBlock") {
                    should_use_block = true;
                }
            }
            _ => {}
        }
    }

    if has_dynamic_keys {
        patch_flag |= 16;
    } else {
        if has_class_binding && !is_component {
            patch_flag |= 2;
        }
        if has_style_binding && !is_component {
            patch_flag |= 4;
        }
        if !dynamic_prop_names.is_empty() {
            patch_flag |= 8;
        }
        if has_hydration_event_binding {
            patch_flag |= 32;
        }
    }

    if !should_use_block
        && (patch_flag == 0 || patch_flag == 32)
        && (has_ref || has_vnode_hook || has_runtime_directives)
    {
        patch_flag |= 512;
    }

    if !in_ssr {
        normalize_class = has_class_binding || props.iter().any(prop_requires_normalize_class);
        normalize_style = has_style_binding
            || props.iter().any(prop_requires_normalize_style)
            || props
                .iter()
                .filter(|prop| prop_output_name(prop) == Some("style"))
                .count()
                > 1;
        if has_dynamic_object {
            normalize_props = true;
            guard_reactive_props = true;
        } else if has_normalize_dynamic_keys {
            normalize_props = true;
        }
    }

    json!({
        "patchFlag": patch_flag,
        "dynamicPropNames": dynamic_prop_names,
        "shouldUseBlock": should_use_block,
        "normalizeProps": normalize_props,
        "guardReactiveProps": guard_reactive_props,
        "normalizeClass": normalize_class,
        "normalizeStyle": normalize_style,
        "refForMarker": ref_for_marker,
        "inlineTemplateRefs": inline_template_refs,
    })
}

pub fn build_directive_args_projection(payload: &Value) -> Value {
    let dir = payload.get("dir").unwrap_or(&Value::Null);
    let need_runtime = payload.get("needRuntime").unwrap_or(&Value::Null);
    let runtime = if let Some(helper) = need_runtime.get("helper").and_then(Value::as_str) {
        json!({ "kind": "helper", "helper": helper })
    } else if let Some(helper_name) = need_runtime.get("helperName").and_then(Value::as_str) {
        json!({ "kind": "helper", "helperName": helper_name })
    } else {
        json!({
            "kind": "asset",
            "name": json_str(dir, "name").unwrap_or(""),
        })
    };
    let modifiers = dir
        .get("modifiers")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
        .iter()
        .filter_map(|modifier| {
            modifier
                .as_str()
                .or_else(|| modifier.get("content").and_then(Value::as_str))
                .map(|name| json!({ "name": name }))
        })
        .collect::<Vec<_>>();
    json!({
        "runtime": runtime,
        "includeExp": dir.get("exp").is_some_and(|exp| !exp.is_null()),
        "includeArg": dir.get("arg").is_some_and(|arg| !arg.is_null()),
        "modifiers": modifiers,
    })
}

pub fn transform_element_children_projection(payload: &Value) -> Value {
    let tag = json_str(payload, "tag").unwrap_or("");
    let children = payload
        .get("children")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    match tag {
        "SUSPENSE" | "BASE_TRANSITION" => {
            let slots = component_slot_projections(children);
            json!({
                "kind": "slots",
                "slots": slots,
                "slotFlag": "1 /* STABLE */",
                "patchFlag": null,
                "shouldUseBlock": tag == "SUSPENSE",
            })
        }
        "KEEP_ALIVE" => json!({
            "kind": "children",
            "patchFlag": 1024,
            "shouldUseBlock": true,
        }),
        _ => json!({ "kind": "default" }),
    }
}

fn component_slot_projections(children: &[Value]) -> Vec<Value> {
    let mut slots = Vec::new();
    let mut plain_indices = Vec::new();
    for (index, child) in children.iter().enumerate() {
        if json_str(child, "tag") == Some("template") {
            if let Some(slot_name) = template_slot_name(child) {
                slots.push(json!({
                    "name": slot_name,
                    "indices": [index],
                    "unwrapTemplate": true,
                }));
                continue;
            }
        }
        plain_indices.push(index);
    }
    if !plain_indices.is_empty() {
        slots.insert(
            0,
            json!({
                "name": "default",
                "indices": plain_indices,
                "unwrapTemplate": false,
            }),
        );
    }
    slots
}

fn template_slot_name(node: &Value) -> Option<&str> {
    node.get("props")
        .and_then(Value::as_array)?
        .iter()
        .find_map(|prop| {
            if json_str(prop, "name") == Some("slot") {
                prop.get("arg")
                    .and_then(|arg| arg.get("content"))
                    .and_then(Value::as_str)
            } else {
                None
            }
        })
}

fn inline_template_ref_projections(props: &[Value], context: &Value) -> Vec<Value> {
    if !json_bool(context, "inline") {
        return Vec::new();
    }
    let Some(binding_metadata) = context.get("bindingMetadata").and_then(Value::as_object) else {
        return Vec::new();
    };
    props
        .iter()
        .filter_map(|prop| {
            if json_str(prop, "kind") != Some("attribute") || json_str(prop, "name") != Some("ref")
            {
                return None;
            }
            let content = json_str(prop, "value")?;
            let binding = binding_metadata.get(content).and_then(Value::as_str)?;
            if matches!(binding, "setup-let" | "setup-ref" | "setup-maybe-ref") {
                Some(json!({ "content": content }))
            } else {
                None
            }
        })
        .collect()
}

fn prop_requires_normalize_style(prop: &Value) -> bool {
    json_str(prop, "kind") == Some("directiveProp")
        && json_str(prop, "name") == Some("style")
        && (json_bool(prop, "valueStartsWithArray")
            || prop.get("valueType").and_then(Value::as_u64) == Some(17))
}

fn prop_requires_normalize_class(prop: &Value) -> bool {
    json_str(prop, "kind") == Some("directiveProp")
        && json_str(prop, "name") == Some("class")
        && !json_bool(prop, "valueStatic")
}

fn prop_output_name(prop: &Value) -> Option<&str> {
    match json_str(prop, "kind") {
        Some("attribute") | Some("directiveProp") => json_str(prop, "name"),
        _ => None,
    }
}

fn prop_name_is_event_handler(name: &str) -> bool {
    name.starts_with("on")
        && name
            .chars()
            .nth(2)
            .is_some_and(|ch| !matches!(ch, 'a'..='z' | '-' | ':'))
}

fn prop_name_is_reserved(name: &str) -> bool {
    matches!(name, "key" | "ref" | "ref_for" | "ref_key")
        || name.starts_with("onVnode")
        || name.starts_with("onUpdate:")
}

fn resolve_component_is_prop(node: &Value) -> Option<&Value> {
    node.get("props")
        .and_then(Value::as_array)?
        .iter()
        .find(|prop| {
            if json_node_type(prop) == Some(6) {
                json_str(prop, "name") == Some("is")
            } else {
                json_str(prop, "name") == Some("bind")
                    && prop.get("arg").is_some_and(|arg| {
                        json_bool(arg, "isStatic") && json_str(arg, "content") == Some("is")
                    })
            }
        })
}

fn resolve_component_is_prop_expression(prop: &Value, context: &Value) -> Option<Value> {
    if json_node_type(prop) == Some(6) {
        return prop
            .get("value")
            .and_then(|value| json_str(value, "content").map(|content| (value, content)))
            .map(|(value, content)| {
                json!({
                    "kind": "simple",
                    "content": content,
                    "isStatic": true,
                    "constType": 3,
                    "loc": value.get("loc").cloned().unwrap_or(Value::Null),
                })
            });
    }

    if let Some(exp) = prop.get("exp").filter(|value| !value.is_null()) {
        return Some(exp.clone());
    }

    let content = if json_bool(context, "prefixIdentifiers") {
        rewrite_js_like_expression("is", &vue3_options_from_transform_context(context))
    } else {
        "is".to_string()
    };
    Some(json!({
        "kind": "simple",
        "content": content,
        "isStatic": false,
        "constType": 0,
        "loc": prop
            .get("arg")
            .and_then(|arg| arg.get("loc"))
            .cloned()
            .unwrap_or(Value::Null),
    }))
}

fn vue3_core_component_helper(tag: &str) -> Option<&'static str> {
    match tag {
        "Teleport" | "teleport" => Some("TELEPORT"),
        "Suspense" | "suspense" => Some("SUSPENSE"),
        "KeepAlive" | "keep-alive" => Some("KEEP_ALIVE"),
        "BaseTransition" | "base-transition" => Some("BASE_TRANSITION"),
        _ => None,
    }
}

fn resolve_setup_reference(name: &str, context: &Value) -> Option<Value> {
    let bindings = context.get("bindingMetadata")?;
    if context.get("isScriptSetup").and_then(Value::as_bool) == Some(false) {
        return None;
    }

    let camel_name = camelize(name);
    let pascal_name = capitalize(&camel_name);
    let from_const = binding_with_type(
        bindings,
        &[name, &camel_name, &pascal_name],
        &["setup-const", "setup-reactive-const", "literal-const"],
    );
    if let Some(name) = from_const {
        return Some(json!({
            "kind": "expression",
            "content": if json_bool(context, "inline") {
                name.to_string()
            } else {
                format!("$setup[{}]", quote_string(name))
            },
        }));
    }

    let from_maybe_ref = binding_with_type(
        bindings,
        &[name, &camel_name, &pascal_name],
        &["setup-let", "setup-ref", "setup-maybe-ref"],
    );
    if let Some(name) = from_maybe_ref {
        return Some(json!({
            "kind": "expression",
            "content": if json_bool(context, "inline") {
                format!("_unref({name})")
            } else {
                format!("$setup[{}]", quote_string(name))
            },
            "helpers": if json_bool(context, "inline") {
                json!(["UNREF"])
            } else {
                json!([])
            },
        }));
    }

    let from_props = binding_with_type(bindings, &[name, &camel_name, &pascal_name], &["props"]);
    if let Some(name) = from_props {
        return Some(json!({
            "kind": "expression",
            "content": format!(
                "_unref({}[{}])",
                if json_bool(context, "inline") { "__props" } else { "$props" },
                quote_string(name),
            ),
            "helpers": ["UNREF"],
        }));
    }

    None
}

fn binding_with_type<'a>(
    bindings: &'a Value,
    names: &[&'a str],
    types: &[&str],
) -> Option<&'a str> {
    names.iter().copied().find(|name| {
        bindings
            .get(*name)
            .and_then(Value::as_str)
            .is_some_and(|binding_type| types.contains(&binding_type))
    })
}

fn transform_if_process_projection(payload: &Value) -> Value {
    let dir = payload.get("dir").unwrap_or(&Value::Null);
    let node = payload.get("node").unwrap_or(&Value::Null);
    let context = payload.get("context").unwrap_or(&Value::Null);
    let siblings = payload
        .get("siblings")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let node_index = payload
        .get("nodeIndex")
        .and_then(Value::as_u64)
        .map(|index| index as usize);
    let dir_name = json_str(dir, "name").unwrap_or("");
    let mut errors = Vec::<Value>::new();
    let condition = transform_if_condition_projection(dir, node, context, &mut errors);
    let branch = json!({
        "condition": condition,
        "children": if json_u64(node, "tagType") == Some(3) && !json_node_has_directive(node, "for") {
            "template"
        } else {
            "self"
        },
        "isTemplateIf": json_u64(node, "tagType") == Some(3),
    });

    if dir_name == "if" {
        return json!({
            "errors": errors,
            "branch": branch,
            "action": {
                "kind": "create",
                "keyBase": node_index
                    .map(|index| transform_if_previous_key_base(siblings, index))
                    .unwrap_or_default(),
            },
        });
    }

    let Some(node_index) = node_index else {
        errors.push(json!({ "code": 30, "loc": "node" }));
        return json!({
            "errors": errors,
            "branch": branch,
            "action": { "kind": "noop" },
        });
    };

    let mut remove_indices = Vec::<usize>::new();
    let mut comment_indices = Vec::<usize>::new();
    let mut scan_index = node_index as isize - 1;
    while scan_index >= 0 {
        let index = scan_index as usize;
        let sibling = &siblings[index];
        if transform_if_is_comment_or_whitespace(sibling) {
            remove_indices.push(index);
            if json_node_type(sibling) == Some(3) {
                comment_indices.insert(0, index);
            }
            scan_index -= 1;
            continue;
        }

        if json_node_type(sibling) == Some(9) {
            if transform_if_last_branch_is_else(sibling) {
                errors.push(json!({ "code": 30, "loc": "node" }));
            }
            let current_key = payload.get("currentUserKey").unwrap_or(&Value::Null);
            if !current_key.is_null() {
                for branch in sibling
                    .get("branches")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    if transform_if_same_key(
                        branch.get("userKey").unwrap_or(&Value::Null),
                        current_key,
                    ) {
                        errors.push(json!({ "code": 29, "loc": "userKey" }));
                    }
                }
            }
            let parent = payload.get("parent").unwrap_or(&Value::Null);
            if transform_if_parent_is_transition(parent) {
                comment_indices.clear();
            }
            return json!({
                "errors": errors,
                "branch": branch,
                "action": {
                    "kind": "append",
                    "targetIndex": index,
                    "removeIndices": remove_indices,
                    "commentIndices": comment_indices,
                },
            });
        }

        errors.push(json!({ "code": 30, "loc": "node" }));
        return json!({
            "errors": errors,
            "branch": branch,
            "action": { "kind": "noop" },
        });
    }

    errors.push(json!({ "code": 30, "loc": "node" }));
    json!({
        "errors": errors,
        "branch": branch,
        "action": { "kind": "noop" },
    })
}

fn transform_if_condition_projection(
    dir: &Value,
    node: &Value,
    context: &Value,
    errors: &mut Vec<Value>,
) -> Value {
    if json_str(dir, "name") == Some("else") {
        return Value::Null;
    }
    let exp = dir.get("exp").filter(|value| !value.is_null());
    let raw_content = exp.and_then(|exp| json_str(exp, "content")).unwrap_or("");
    let missing = exp.is_none() || raw_content.trim().is_empty();
    if missing {
        errors.push(json!({ "code": 28, "loc": "dir" }));
        return json!({
            "kind": "simple",
            "content": "true",
            "isStatic": false,
            "constType": 0,
            "loc": exp
                .and_then(|exp| exp.get("loc"))
                .or_else(|| node.get("loc"))
                .cloned()
                .unwrap_or(Value::Null),
        });
    }

    if !json_bool(context, "prefixIdentifiers") {
        return Value::Null;
    }

    let options = vue3_options_from_transform_context(context);
    let locals = transform_context_locals(context);
    let rewritten = if locals.is_empty() {
        rewrite_js_like_expression(raw_content, &options)
    } else {
        rewrite_js_like_expression_with_locals(raw_content, &options, &locals)
    };
    json!({
        "kind": "simple",
        "content": rewritten,
        "isStatic": false,
        "constType": 0,
        "loc": exp
            .and_then(|exp| exp.get("loc"))
            .cloned()
            .unwrap_or(Value::Null),
    })
}

fn transform_if_branch_codegen_projection(payload: &Value) -> Value {
    let branch = payload.get("branch").unwrap_or(&Value::Null);
    let children = branch
        .get("children")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let first = children.first();
    let need_fragment_wrapper = children.len() != 1
        || first
            .and_then(|child| json_node_type(child))
            .is_some_and(|node_type| node_type != 1);
    if need_fragment_wrapper {
        if children.len() == 1 && first.and_then(|child| json_node_type(child)) == Some(11) {
            return json!({ "kind": "for" });
        }
        let mut patch_flag = 64u16;
        if !json_bool(branch, "isTemplateIf")
            && children
                .iter()
                .filter(|child| json_node_type(child) != Some(3))
                .count()
                == 1
        {
            patch_flag |= 2048;
        }
        return json!({
            "kind": "fragment",
            "patchFlag": patch_flag,
        });
    }

    json!({
        "kind": "single",
        "convertToBlock": first
            .and_then(|child| json_u64(child, "memoedCodegenType"))
            == Some(13),
    })
}

fn transform_if_previous_key_base(siblings: &[Value], node_index: usize) -> usize {
    siblings
        .iter()
        .take(node_index)
        .filter(|sibling| json_node_type(sibling) == Some(9))
        .map(|sibling| {
            sibling
                .get("branches")
                .and_then(Value::as_array)
                .map(Vec::len)
                .unwrap_or_default()
        })
        .sum()
}

fn transform_if_is_comment_or_whitespace(node: &Value) -> bool {
    match json_node_type(node) {
        Some(3) => true,
        Some(2) => {
            let content_is_ascii_whitespace =
                json_str(node, "content").is_some_and(transform_if_is_ascii_html_whitespace);
            let loc_is_ascii_whitespace = json_str(node, "locSource")
                .map(transform_if_is_ascii_html_whitespace)
                .unwrap_or(true);
            content_is_ascii_whitespace && loc_is_ascii_whitespace
        }
        Some(12) => node
            .get("content")
            .is_some_and(transform_if_is_comment_or_whitespace),
        _ => false,
    }
}

fn transform_if_is_ascii_html_whitespace(content: &str) -> bool {
    content
        .bytes()
        .all(|byte| matches!(byte, b'\t' | b'\n' | b'\x0C' | b'\r' | b' '))
}

fn transform_if_last_branch_is_else(if_node: &Value) -> bool {
    if_node
        .get("branches")
        .and_then(Value::as_array)
        .and_then(|branches| branches.last())
        .is_some_and(|branch| !json_bool(branch, "hasCondition"))
}

fn transform_if_same_key(a: &Value, b: &Value) -> bool {
    if a.is_null() || b.is_null() || json_node_type(a) != json_node_type(b) {
        return false;
    }
    match json_node_type(a) {
        Some(6) => {
            a.get("value").and_then(|value| json_str(value, "content"))
                == b.get("value").and_then(|value| json_str(value, "content"))
        }
        Some(7) => {
            let a_exp = a.get("exp").unwrap_or(&Value::Null);
            let b_exp = b.get("exp").unwrap_or(&Value::Null);
            json_node_type(a_exp) == json_node_type(b_exp)
                && json_bool(a_exp, "isStatic") == json_bool(b_exp, "isStatic")
                && json_str(a_exp, "content") == json_str(b_exp, "content")
        }
        _ => false,
    }
}

fn transform_if_parent_is_transition(parent: &Value) -> bool {
    json_node_type(parent) == Some(1)
        && matches!(json_str(parent, "tag"), Some("transition" | "Transition"))
}

fn json_node_has_directive(node: &Value, name: &str) -> bool {
    node.get("props")
        .and_then(Value::as_array)
        .is_some_and(|props| {
            props
                .iter()
                .any(|prop| json_node_type(prop) == Some(7) && json_str(prop, "name") == Some(name))
        })
}

fn vue3_options_from_transform_context(context: &Value) -> Vue3CompilerOptions {
    let mut options = Vue3CompilerOptions {
        prefix_identifiers: json_bool(context, "prefixIdentifiers"),
        inline: json_bool(context, "inline"),
        is_ts: json_bool(context, "isTS"),
        ..Vue3CompilerOptions::default()
    };
    if let Some(metadata) = context.get("bindingMetadata").and_then(Value::as_object) {
        for (key, value) in metadata {
            if key == "__propsAliases" {
                if let Some(aliases) = value.as_object() {
                    options.props_aliases = aliases
                        .iter()
                        .filter_map(|(alias, source)| {
                            source
                                .as_str()
                                .map(|source| (alias.clone(), source.to_string()))
                        })
                        .collect();
                }
            } else if let Some(kind) = value.as_str() {
                options
                    .binding_metadata
                    .insert(key.clone(), kind.to_string());
            }
        }
    }
    options
}

fn transform_context_locals(context: &Value) -> Vec<String> {
    context
        .get("identifiers")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|identifiers| identifiers.iter())
        .filter(|(_, count)| count.as_i64().unwrap_or_default() > 0)
        .map(|(name, _)| name.clone())
        .collect()
}

fn model_assignment_projection(
    exp: &Value,
    raw_exp: &str,
    event_arg: &str,
    binding_type: Option<&str>,
    maybe_ref: bool,
) -> Value {
    if maybe_ref {
        if binding_type == Some("setup-ref") {
            return json!({
                "kind": "compound",
                "children": [
                    format!("{event_arg} => (("),
                    { "kind": "simple", "content": raw_exp, "isStatic": false, "loc": exp.get("loc").cloned().unwrap_or(Value::Null) },
                    ").value = $event)"
                ]
            });
        }
        let alt_assignment = if binding_type == Some("setup-let") {
            format!("{raw_exp} = $event")
        } else {
            "null".to_string()
        };
        return json!({
            "kind": "compound",
            "children": [
                format!("{event_arg} => (_isRef({raw_exp}) ? ("),
                { "kind": "simple", "content": raw_exp, "isStatic": false, "loc": exp.get("loc").cloned().unwrap_or(Value::Null) },
                format!(").value = $event : {alt_assignment})")
            ],
            "helpers": ["IS_REF"]
        });
    }

    json!({
        "kind": "compound",
        "children": [
            format!("{event_arg} => (("),
            { "kind": "node", "path": "dir.exp" },
            ") = $event)"
        ]
    })
}

fn model_prop_name_projection(arg: Option<&Value>) -> Value {
    match arg {
        Some(_) => json!({ "kind": "node", "path": "dir.arg" }),
        None => json!({ "kind": "static", "content": "modelValue" }),
    }
}

fn model_event_name_projection(arg: Option<&Value>) -> Value {
    match arg {
        Some(arg) if json_bool(arg, "isStatic") => json!({
            "kind": "static",
            "content": format!("onUpdate:{}", camelize(json_str(arg, "content").unwrap_or(""))),
        }),
        Some(_) => json!({
            "kind": "compound",
            "children": [
                "\"onUpdate:\" + ",
                { "kind": "node", "path": "dir.arg" }
            ],
        }),
        None => json!({ "kind": "static", "content": "onUpdate:modelValue" }),
    }
}

fn model_update_needs_hydration_event(arg: Option<&Value>, node: &Value) -> bool {
    arg.is_some_and(|arg| json_bool(arg, "isStatic")) && json_u64(node, "tagType") != Some(1)
}

fn model_modifiers_key_projection(arg: Option<&Value>) -> Value {
    match arg {
        Some(arg) if json_bool(arg, "isStatic") => json!({
            "kind": "static",
            "content": format!("{}Modifiers", json_str(arg, "content").unwrap_or("")),
        }),
        Some(_) => json!({
            "kind": "compound",
            "children": [
                { "kind": "node", "path": "dir.arg" },
                " + \"Modifiers\""
            ],
        }),
        None => json!({ "kind": "static", "content": "modelModifiers" }),
    }
}

fn model_modifiers_expression(dir: &Value) -> Value {
    let modifiers = dir
        .get("modifiers")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|modifier| json_str(modifier, "content"))
        .map(|modifier| {
            if is_simple_identifier_ascii(modifier) {
                format!("{modifier}: true")
            } else {
                format!("{}: true", quote_string(modifier))
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    json!({
        "kind": "simple",
        "content": format!("{{ {modifiers} }}"),
        "isStatic": false,
        "constType": 2,
    })
}

fn should_cache_model_update(exp: &Value, context: &Value) -> bool {
    json_bool(context, "prefixIdentifiers")
        && json_bool(context, "cacheHandlers")
        && !json_bool(context, "inVOnce")
        && !model_has_scope_ref(exp, context)
}

fn model_has_scope_ref(exp: &Value, context: &Value) -> bool {
    let source = model_expression_source(exp);
    context
        .get("identifiers")
        .and_then(Value::as_object)
        .is_some_and(|identifiers| {
            identifiers.iter().any(|(name, count)| {
                count.as_i64().unwrap_or_default() > 0 && source.contains(name)
            })
        })
}

fn model_expression_source(exp: &Value) -> String {
    if let Some(content) = json_str(exp, "content") {
        return content.to_string();
    }
    if let Some(children) = exp.get("children").and_then(Value::as_array) {
        return children
            .iter()
            .map(model_expression_child_source)
            .collect::<String>();
    }
    exp.get("loc")
        .and_then(|loc| loc.get("source"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn model_expression_child_source(child: &Value) -> String {
    child
        .as_str()
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| model_expression_source(child))
}

fn transform_on_event_name_projection(
    arg: Option<&Value>,
    node: &Value,
    errors: &mut Vec<Value>,
) -> Value {
    let Some(arg) = arg else {
        return json!({ "kind": "static", "content": "on" });
    };
    if json_node_type(arg) == Some(4) {
        if json_bool(arg, "isStatic") {
            let mut raw_name = json_str(arg, "content").unwrap_or("").to_string();
            if raw_name.starts_with("vnode") {
                errors.push(json!({ "code": 52, "loc": "arg" }));
            }
            if let Some(rest) = raw_name.strip_prefix("vue:") {
                raw_name = format!("vnode-{rest}");
            }
            let event_string = if json_u64(node, "tagType") != Some(0)
                || raw_name.starts_with("vnode")
                || !raw_name.chars().any(|ch| ch.is_ascii_uppercase())
            {
                to_handler_key(&camelize(&raw_name))
            } else {
                format!("on:{raw_name}")
            };
            return json!({
                "kind": "simple",
                "content": event_string,
                "isStatic": true,
                "loc": arg.get("loc").cloned().unwrap_or(Value::Null),
            });
        }
        return json!({
            "kind": "compound",
            "children": [
                { "kind": "helperString", "helper": "TO_HANDLER_KEY" },
                { "kind": "node", "path": "dir.arg" },
                ")",
            ],
        });
    }
    json!({
        "kind": "compound",
        "children": [
            { "kind": "helperString", "helper": "TO_HANDLER_KEY" },
            { "kind": "node", "path": "dir.arg.children" },
            ")",
        ],
        "loc": arg.get("loc").cloned().unwrap_or(Value::Null),
    })
}

fn transform_on_handler_projection(dir: &Value, node: &Value, context: &Value) -> Value {
    let Some(exp) = dir.get("exp").filter(|value| !value.is_null()) else {
        return json!({ "cache": json_bool(context, "cacheHandlers") && !json_bool(context, "inVOnce") });
    };
    let raw = transform_on_expression_source(exp);
    if raw.trim().is_empty() {
        return json!({ "cache": json_bool(context, "cacheHandlers") && !json_bool(context, "inVOnce") });
    }

    let is_member = transform_on_is_member_expression(&raw, context);
    let is_fn = transform_on_is_fn_expression(&raw, context);
    let is_inline = !is_member && !is_fn;
    let has_multiple_statements = raw.contains(';');
    let mut processed = json!({ "kind": "node", "path": "dir.exp" });
    let mut should_cache = false;

    if json_bool(context, "prefixIdentifiers") {
        let options = vue3_options_from_transform_context(context);
        let mut locals = transform_context_locals(context);
        if is_inline {
            locals.push("$event".to_string());
        }
        processed = transform_on_rewrite_expression_node(
            &raw,
            exp,
            &options,
            &locals,
            has_multiple_statements,
        );
        should_cache = json_bool(context, "cacheHandlers")
            && !json_bool(context, "inVOnce")
            && transform_on_projection_const_type(&processed) == 0
            && !(is_member && json_u64(node, "tagType") == Some(1))
            && !transform_on_has_scope_ref(&processed, context);
        if should_cache && is_member {
            processed = transform_on_member_invocation_projection(processed);
        }
    }

    if is_inline || (should_cache && is_member) {
        processed = transform_on_wrap_handler_projection(
            processed,
            is_inline,
            has_multiple_statements,
            json_bool(context, "isTS"),
        );
    }

    json!({
        "value": processed,
        "cache": should_cache,
        "isInlineStatement": is_inline,
        "isMemberExpression": is_member,
        "isFunctionExpression": is_fn,
    })
}

fn transform_on_empty_handler_projection(dir: &Value) -> Value {
    json!({
        "kind": "simple",
        "content": "() => {}",
        "isStatic": false,
        "loc": dir.get("loc").cloned().unwrap_or(Value::Null),
    })
}

fn transform_on_rewrite_expression_node(
    raw: &str,
    exp: &Value,
    options: &Vue3CompilerOptions,
    locals: &[String],
    as_raw_statements: bool,
) -> Value {
    let trimmed = raw.trim();
    let loc = exp.get("loc").cloned().unwrap_or(Value::Null);
    let mut effective_locals = locals.to_vec();
    effective_locals.extend(transform_on_root_function_locals(raw));
    effective_locals.sort();
    effective_locals.dedup();
    let rewritten = if effective_locals.is_empty() {
        rewrite_js_like_expression(raw, options)
    } else {
        rewrite_js_like_expression_with_locals(raw, options, &effective_locals)
    };
    let children = vue3_for_compound_children(
        raw,
        options,
        &effective_locals,
        Vue3ForAstMode::Expression,
        &loc,
    );
    let const_type = transform_on_const_type(trimmed, rewritten.trim(), options);
    if is_simple_identifier_ascii(trimmed) || (children.is_empty() && !as_raw_statements) {
        return transform_on_simple_projection(rewritten.trim(), exp, const_type);
    }
    let helpers = vue3_for_helpers_for_content(&rewritten);
    let mut value = json!({
        "kind": "compound",
        "children": children,
        "loc": loc,
        "constType": const_type,
    });
    if !helpers.is_empty() {
        value["helpers"] = json!(helpers);
    }
    value
}

fn transform_on_simple_projection(content: &str, exp: &Value, const_type: u8) -> Value {
    let mut value = json!({
        "kind": "simple",
        "content": content,
        "isStatic": false,
        "constType": const_type,
        "loc": exp.get("loc").cloned().unwrap_or(Value::Null),
    });
    let helpers = vue3_for_helpers_for_content(content);
    if !helpers.is_empty() {
        value["helpers"] = json!(helpers);
    }
    value
}

fn transform_on_const_type(raw: &str, rewritten: &str, options: &Vue3CompilerOptions) -> u8 {
    if is_simple_identifier_ascii(raw)
        && matches!(
            options.binding_metadata.get(raw).map(String::as_str),
            Some("setup-const" | "literal-const")
        )
    {
        return 1;
    }
    vue3_for_const_type(rewritten)
}

fn transform_on_member_invocation_projection(processed: Value) -> Value {
    match json_str(&processed, "kind") {
        Some("simple") => {
            let content = json_str(&processed, "content").unwrap_or("").to_string();
            let mut next = processed;
            next["content"] = json!(format!("{content} && {content}(...args)"));
            next
        }
        Some("compound") => {
            let children = processed
                .get("children")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let mut next_children = children.clone();
            next_children.push(json!(" && "));
            next_children.extend(children);
            next_children.push(json!("(...args)"));
            let mut next = processed;
            next["children"] = json!(next_children);
            next
        }
        _ => processed,
    }
}

fn transform_on_wrap_handler_projection(
    processed: Value,
    is_inline: bool,
    has_multiple_statements: bool,
    is_ts: bool,
) -> Value {
    let param = if is_inline {
        if is_ts {
            "($event: any)"
        } else {
            "$event"
        }
    } else if is_ts {
        "\n//@ts-ignore\n(...args)"
    } else {
        "(...args)"
    };
    json!({
        "kind": "compound",
        "children": [
            format!("{param} => {}", if has_multiple_statements { "{" } else { "(" }),
            processed,
            if has_multiple_statements { "}" } else { ")" },
        ],
    })
}

fn transform_on_expression_source(exp: &Value) -> String {
    if let Some(content) = json_str(exp, "content") {
        return content.to_string();
    }
    exp.get("loc")
        .and_then(|loc| json_str(loc, "source"))
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| model_expression_source(exp))
}

fn transform_on_projection_const_type(projection: &Value) -> u64 {
    projection
        .get("constType")
        .and_then(Value::as_u64)
        .unwrap_or_default()
}

fn transform_on_has_scope_ref(exp: &Value, context: &Value) -> bool {
    let source = model_expression_source(exp);
    context
        .get("identifiers")
        .and_then(Value::as_object)
        .is_some_and(|identifiers| {
            identifiers.iter().any(|(name, count)| {
                count.as_i64().unwrap_or_default() > 0 && source_contains_identifier(&source, name)
            })
        })
}

fn transform_on_is_member_expression(expression: &str, context: &Value) -> bool {
    let store = JsAstStore::new();
    let wrapped = format!("({})", expression.trim());
    match store.parse_expression(&wrapped, transform_on_source_type(context)) {
        Ok(expression) => transform_on_expression_is_member(&expression),
        Err(_) if json_bool(context, "allowLexerFallback") => {
            transform_on_is_member_expression_lexer(expression)
        }
        Err(_) => false,
    }
}

fn transform_on_expression_is_member(expression: &Expression<'_>) -> bool {
    match expression {
        Expression::Identifier(identifier) => identifier.name != "undefined",
        Expression::ComputedMemberExpression(_)
        | Expression::StaticMemberExpression(_)
        | Expression::PrivateFieldExpression(_) => true,
        Expression::ChainExpression(chain) => {
            transform_on_chain_element_is_member(&chain.expression)
        }
        Expression::ParenthesizedExpression(expression) => {
            transform_on_expression_is_member(&expression.expression)
        }
        Expression::TSAsExpression(expression) => {
            transform_on_expression_is_member(&expression.expression)
        }
        Expression::TSSatisfiesExpression(expression) => {
            transform_on_expression_is_member(&expression.expression)
        }
        Expression::TSTypeAssertion(expression) => {
            transform_on_expression_is_member(&expression.expression)
        }
        Expression::TSNonNullExpression(expression) => {
            transform_on_expression_is_member(&expression.expression)
        }
        Expression::TSInstantiationExpression(expression) => {
            transform_on_expression_is_member(&expression.expression)
        }
        _ => false,
    }
}

fn transform_on_chain_element_is_member(element: &ChainElement<'_>) -> bool {
    matches!(
        element,
        ChainElement::ComputedMemberExpression(_)
            | ChainElement::StaticMemberExpression(_)
            | ChainElement::PrivateFieldExpression(_)
            | ChainElement::TSNonNullExpression(_)
    )
}

fn transform_on_is_fn_expression(expression: &str, context: &Value) -> bool {
    let trimmed = expression.trim_start();
    if transform_on_is_fn_expression_lexer(trimmed) {
        return true;
    }
    let store = JsAstStore::new();
    store
        .parse_expression(expression.trim(), transform_on_source_type(context))
        .map(|expression| transform_on_expression_is_fn(&expression))
        .unwrap_or(false)
}

fn transform_on_expression_is_fn(expression: &Expression<'_>) -> bool {
    match expression {
        Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_) => true,
        Expression::TSAsExpression(expression) => {
            transform_on_expression_is_fn(&expression.expression)
        }
        Expression::TSSatisfiesExpression(expression) => {
            transform_on_expression_is_fn(&expression.expression)
        }
        Expression::TSTypeAssertion(expression) => {
            transform_on_expression_is_fn(&expression.expression)
        }
        Expression::TSNonNullExpression(expression) => {
            transform_on_expression_is_fn(&expression.expression)
        }
        Expression::TSInstantiationExpression(expression) => {
            transform_on_expression_is_fn(&expression.expression)
        }
        _ => false,
    }
}

fn transform_on_is_fn_expression_lexer(expression: &str) -> bool {
    expression.starts_with("function")
        || expression.starts_with("async function")
        || expression
            .find("=>")
            .is_some_and(|index| transform_on_arrow_prefix_is_fn_like(&expression[..index]))
}

fn transform_on_arrow_prefix_is_fn_like(prefix: &str) -> bool {
    let prefix = prefix.trim();
    let prefix = prefix.strip_prefix("async").unwrap_or(prefix).trim();
    if prefix.starts_with('(') {
        return prefix.ends_with(')');
    }
    is_simple_identifier_ascii(prefix)
}

fn transform_on_root_function_locals(expression: &str) -> Vec<String> {
    let store = JsAstStore::new();
    store
        .parse_expression(expression.trim(), oxc_span::SourceType::ts())
        .map(|expression| {
            let mut locals = Vec::new();
            transform_on_collect_root_function_locals(&expression, &mut locals);
            locals.sort();
            locals.dedup();
            locals
        })
        .unwrap_or_else(|_| transform_on_root_function_locals_lexer(expression))
}

fn transform_on_collect_root_function_locals(
    expression: &Expression<'_>,
    locals: &mut Vec<String>,
) {
    match expression {
        Expression::ArrowFunctionExpression(function) => {
            for param in &function.params.items {
                collect_vue3_for_binding_pattern(&param.pattern, locals);
            }
            if let Some(rest) = &function.params.rest {
                collect_vue3_for_binding_pattern(&rest.rest.argument, locals);
            }
        }
        Expression::FunctionExpression(function) => {
            for param in &function.params.items {
                collect_vue3_for_binding_pattern(&param.pattern, locals);
            }
            if let Some(rest) = &function.params.rest {
                collect_vue3_for_binding_pattern(&rest.rest.argument, locals);
            }
        }
        Expression::TSAsExpression(expression) => {
            transform_on_collect_root_function_locals(&expression.expression, locals)
        }
        Expression::TSSatisfiesExpression(expression) => {
            transform_on_collect_root_function_locals(&expression.expression, locals)
        }
        Expression::TSTypeAssertion(expression) => {
            transform_on_collect_root_function_locals(&expression.expression, locals)
        }
        Expression::TSNonNullExpression(expression) => {
            transform_on_collect_root_function_locals(&expression.expression, locals)
        }
        Expression::TSInstantiationExpression(expression) => {
            transform_on_collect_root_function_locals(&expression.expression, locals)
        }
        _ => {}
    }
}

fn transform_on_root_function_locals_lexer(expression: &str) -> Vec<String> {
    let trimmed = expression.trim_start();
    let Some(arrow_index) = trimmed.find("=>") else {
        return Vec::new();
    };
    let mut params = trimmed[..arrow_index].trim();
    params = params.strip_prefix("async").unwrap_or(params).trim();
    if params.starts_with('(') && params.ends_with(')') {
        params = &params[1..params.len() - 1];
    }
    split_top_level_like(params, ',')
        .into_iter()
        .flat_map(extract_slot_params)
        .collect()
}

fn transform_on_source_type(context: &Value) -> oxc_span::SourceType {
    let _ = context;
    oxc_span::SourceType::ts()
}

fn transform_on_is_member_expression_lexer(expression: &str) -> bool {
    let path = normalize_member_expression_whitespace(expression.trim());
    if path.is_empty() {
        return false;
    }
    let mut depth_square = 0usize;
    let mut depth_paren = 0usize;
    let mut quote = None::<char>;
    let mut escaped = false;
    let mut chars = path.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' | '`' => quote = Some(ch),
            '[' => depth_square += 1,
            ']' => {
                if depth_square == 0 {
                    return false;
                }
                depth_square -= 1;
            }
            '(' => depth_paren += 1,
            ')' => {
                if chars.peek().is_none() {
                    return false;
                }
                if depth_paren == 0 {
                    return false;
                }
                depth_paren -= 1;
            }
            _ if depth_square == 0 && depth_paren == 0 => {
                let valid = if index == 0 {
                    is_identifier_start(ch)
                } else {
                    is_identifier_continue(ch) || matches!(ch, '.' | '?')
                };
                if !valid {
                    return false;
                }
            }
            _ => {}
        }
    }
    depth_square == 0 && depth_paren == 0 && quote.is_none()
}

fn normalize_member_expression_whitespace(expression: &str) -> String {
    let mut output = String::new();
    let chars = expression.chars().collect::<Vec<_>>();
    for (index, ch) in chars.iter().copied().enumerate() {
        if ch.is_whitespace() {
            let prev = chars[..index]
                .iter()
                .rev()
                .find(|candidate| !candidate.is_whitespace())
                .copied();
            let next = chars[index + 1..]
                .iter()
                .find(|candidate| !candidate.is_whitespace())
                .copied();
            if matches!(prev, Some('.' | '[')) || matches!(next, Some('.' | '[')) {
                continue;
            }
        }
        output.push(ch);
    }
    output
}

fn model_is_member_expression(expression: &str) -> bool {
    let store = JsAstStore::new();
    store
        .parse_expression(expression, oxc_span::SourceType::mjs())
        .map(|expression| match expression {
            Expression::Identifier(_) => true,
            Expression::ComputedMemberExpression(_)
            | Expression::StaticMemberExpression(_)
            | Expression::PrivateFieldExpression(_) => true,
            Expression::ChainExpression(chain) => model_chain_element_is_member(&chain.expression),
            _ => false,
        })
        .unwrap_or(false)
}

fn model_chain_element_is_member(element: &ChainElement<'_>) -> bool {
    matches!(
        element,
        ChainElement::ComputedMemberExpression(_)
            | ChainElement::StaticMemberExpression(_)
            | ChainElement::PrivateFieldExpression(_)
    )
}

fn context_identifier_count<'a>(context: &'a Value, name: &str) -> i64 {
    context
        .get("identifiers")
        .and_then(|identifiers| identifiers.get(name))
        .and_then(Value::as_i64)
        .unwrap_or_default()
}

fn camelize(value: &str) -> String {
    let mut output = String::new();
    let mut uppercase_next = false;
    for ch in value.chars() {
        if uppercase_next {
            output.extend(ch.to_uppercase());
            uppercase_next = false;
        } else if ch == '-' {
            uppercase_next = true;
        } else {
            output.push(ch);
        }
    }
    output
}

fn to_handler_key(value: &str) -> String {
    if value.is_empty() {
        String::new()
    } else {
        format!("on{}", capitalize(value))
    }
}

fn is_simple_identifier_ascii(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first == '$' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch == '$' || ch.is_ascii_alphanumeric())
}

fn json_str<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn json_bool(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn vue3_element_kind(
    tag: String,
    attributes: Vec<vuec_html::HtmlAttribute>,
    self_closing: bool,
    options: &Vue3CompilerOptions,
    file_id: FileId,
    base_offset: usize,
    in_v_pre: bool,
    namespace: vuec_ast::HtmlNamespace,
) -> Vue3NodeKind {
    let props = attributes
        .into_iter()
        .filter(|attr| !(in_v_pre && attr.name == "v-pre"))
        .map(|attr| {
            if in_v_pre {
                vue3_attribute_from_attr(attr, file_id, base_offset)
            } else {
                vue3_prop_from_attr(attr, file_id, base_offset)
            }
        })
        .collect::<Vec<_>>();
    let tag_type = if in_v_pre {
        Vue3ElementType::Element
    } else {
        vue3_tag_type(&tag, &props, options)
    };
    Vue3NodeKind::Element(Vue3Element {
        tag,
        tag_type,
        ns: namespace,
        props,
        self_closing,
        codegen_node: None,
        ssr_codegen_node: None,
    })
}

fn vue3_element_namespace(
    ast: &Vue3Ast,
    parent_id: vuec_ast::NodeId,
    tag: &str,
    parent: vuec_ast::HtmlNamespace,
    options: &Vue3CompilerOptions,
) -> vuec_ast::HtmlNamespace {
    if let Some(namespace) = options.namespaces.get(tag).copied() {
        return namespace;
    }
    let mut namespace = parent;
    if options.dom_namespaces {
        if let Some(parent_element) = ast.node(parent_id).and_then(|node| match &node.kind {
            Vue3AstKind::Element(element) => Some(element),
            _ => None,
        }) {
            if namespace == vuec_ast::HtmlNamespace::MathMl {
                if parent_element.tag == "annotation-xml" {
                    if tag == "svg" {
                        return vuec_ast::HtmlNamespace::Svg;
                    }
                    if vue3_element_has_attr_value(
                        parent_element,
                        "encoding",
                        &["text/html", "application/xhtml+xml"],
                    ) {
                        namespace = vuec_ast::HtmlNamespace::Html;
                    }
                } else if vue3_mathml_text_integration_point(&parent_element.tag)
                    && tag != "mglyph"
                    && tag != "malignmark"
                {
                    namespace = vuec_ast::HtmlNamespace::Html;
                }
            } else if namespace == vuec_ast::HtmlNamespace::Svg
                && matches!(
                    parent_element.tag.as_str(),
                    "foreignObject" | "desc" | "title"
                )
            {
                namespace = vuec_ast::HtmlNamespace::Html;
            }
        }
        if namespace == vuec_ast::HtmlNamespace::Html {
            if tag == "svg" {
                return vuec_ast::HtmlNamespace::Svg;
            }
            if tag == "math" {
                return vuec_ast::HtmlNamespace::MathMl;
            }
        }
    }
    namespace
}

fn vue3_mathml_text_integration_point(tag: &str) -> bool {
    matches!(tag, "mi" | "mo" | "mn" | "ms" | "mtext")
}

fn vue3_element_has_attr_value(
    element: &vuec_ast::Vue3Element,
    name: &str,
    values: &[&str],
) -> bool {
    element.props.iter().any(|prop| {
        matches!(
            prop,
            Vue3Prop::Attribute(attr)
                if attr.name == name
                    && attr
                        .value
                        .as_deref()
                        .is_some_and(|value| values.iter().any(|candidate| *candidate == value))
        )
    })
}

fn vue3_tag_type(tag: &str, props: &[Vue3Prop], options: &Vue3CompilerOptions) -> Vue3ElementType {
    if options
        .custom_elements
        .iter()
        .any(|candidate| candidate == tag)
    {
        return Vue3ElementType::Element;
    }
    if tag == "slot" {
        return Vue3ElementType::SlotOutlet;
    }
    if tag == "template" {
        return if props.iter().any(
            |prop| matches!(prop, Vue3Prop::Directive(dir) if is_template_directive(&dir.name)),
        ) {
            Vue3ElementType::Template
        } else {
            Vue3ElementType::Element
        };
    }
    if options
        .built_in_components
        .iter()
        .any(|candidate| candidate == tag)
    {
        return Vue3ElementType::Component;
    }
    if vue3_core_component_helper(tag).is_some() || matches!(tag, "component" | "Component") {
        return Vue3ElementType::Component;
    }
    if props.iter().any(|prop| {
        matches!(
            prop,
            Vue3Prop::Attribute(attr)
                if attr.name == "is"
                    && attr
                        .value
                        .as_deref()
                        .is_some_and(|value| value.starts_with("vue:"))
        )
    }) {
        return Vue3ElementType::Component;
    }
    if options
        .native_tags
        .as_ref()
        .is_some_and(|native_tags| !native_tags.iter().any(|candidate| candidate == tag))
    {
        return Vue3ElementType::Component;
    }
    if tag.chars().next().is_some_and(|ch| ch.is_ascii_uppercase()) {
        return Vue3ElementType::Component;
    }
    Vue3ElementType::Element
}

fn is_template_directive(name: &str) -> bool {
    matches!(name, "if" | "else" | "else-if" | "for" | "slot")
}

fn vue3_prop_from_attr(
    attr: vuec_html::HtmlAttribute,
    file_id: FileId,
    base_offset: usize,
) -> Vue3Prop {
    let parsed_attr = vue3_attr_from_html(attr, file_id, base_offset);
    let attr = parsed_attr.attr;
    if let Some(parsed) = parse_vue3_directive(&attr.name, attr.name_span) {
        let (directive_name, arg, modifiers, is_dynamic_arg, arg_span, modifier_spans) = parsed;
        Vue3Prop::Directive(Vue3Directive {
            name: directive_name,
            raw_name: attr.name,
            arg: arg.map(Vue3Expression::Raw),
            exp: attr.value.map(Vue3Expression::Raw),
            modifiers,
            is_dynamic_arg,
            span: attr.span,
            arg_span,
            exp_span: parsed_attr.value_content_span.or(attr.value_span),
            modifier_spans,
        })
    } else {
        Vue3Prop::Attribute(attr)
    }
}

fn vue3_attribute_from_attr(
    attr: vuec_html::HtmlAttribute,
    file_id: FileId,
    base_offset: usize,
) -> Vue3Prop {
    Vue3Prop::Attribute(vue3_attr_from_html(attr, file_id, base_offset).attr)
}

struct ParsedVue3Attribute {
    attr: vuec_ast::Vue3Attribute,
    value_content_span: Option<Span>,
}

fn vue3_attr_from_html(
    attr: vuec_html::HtmlAttribute,
    file_id: FileId,
    base_offset: usize,
) -> ParsedVue3Attribute {
    let span = Some(Span::new(
        file_id,
        base_offset + attr.start,
        base_offset + attr.end,
    ));
    let name_span = Some(Span::new(
        file_id,
        base_offset + attr.name_start,
        base_offset + attr.name_end,
    ));
    let value_span = attr
        .value_start
        .zip(attr.value_end)
        .map(|(start, end)| Span::new(file_id, base_offset + start, base_offset + end));
    let value_content_span = attr
        .value_content_start
        .zip(attr.value_content_end)
        .map(|(start, end)| Span::new(file_id, base_offset + start, base_offset + end));
    let quote = attr.quote.map(|quote| match quote {
        vuec_html::HtmlQuoteKind::Double => QuoteKind::Double,
        vuec_html::HtmlQuoteKind::Single => QuoteKind::Single,
        vuec_html::HtmlQuoteKind::Unquoted => QuoteKind::Unquoted,
    });
    ParsedVue3Attribute {
        attr: vuec_ast::Vue3Attribute {
            name: attr.name,
            value: attr.value,
            span,
            name_span,
            value_span,
            quote,
        },
        value_content_span,
    }
}

fn parse_vue3_directive(
    raw: &str,
    name_span: Option<Span>,
) -> Option<(
    String,
    Option<String>,
    Vec<String>,
    bool,
    Option<Span>,
    Vec<NodeSpan>,
)> {
    let mut body = raw;
    let mut name = None;
    let mut arg_offset = 0usize;
    if let Some(rest) = raw.strip_prefix("v-") {
        if let Some((head, tail)) = rest.split_once(':') {
            name = Some(head.to_string());
            body = tail;
            arg_offset = 2 + head.len() + 1;
        } else {
            let mut parts = split_directive_parts(rest, false);
            let directive = parts.next().unwrap_or_default();
            if directive.is_empty() {
                return None;
            }
            let modifiers = parts.collect::<Vec<_>>();
            let modifier_spans = directive_modifier_spans(raw, &modifiers, name_span);
            return Some((
                directive.to_string(),
                None,
                modifiers.into_iter().map(ToOwned::to_owned).collect(),
                false,
                None,
                modifier_spans,
            ));
        }
    } else if let Some(rest) = raw.strip_prefix(':') {
        name = Some("bind".to_string());
        body = rest;
        arg_offset = 1;
    } else if let Some(rest) = raw.strip_prefix('@') {
        name = Some("on".to_string());
        body = rest;
        arg_offset = 1;
    } else if let Some(rest) = raw.strip_prefix('#') {
        name = Some("slot".to_string());
        body = rest;
        arg_offset = 1;
    } else if let Some(rest) = raw.strip_prefix('.') {
        name = Some("bind".to_string());
        body = rest;
        arg_offset = 1;
    }
    let name = name?;
    if name.is_empty() {
        return None;
    }
    let preserve_arg_dots = name == "slot";
    let mut parts = split_directive_parts(body, preserve_arg_dots);
    let raw_arg = parts.next().unwrap_or_default();
    let modifiers = if raw.starts_with('.') {
        let mut values = vec!["prop".to_string()];
        values.extend(parts.map(ToOwned::to_owned));
        values
    } else {
        parts.map(ToOwned::to_owned).collect::<Vec<_>>()
    };
    let (arg, is_dynamic) = if raw_arg.starts_with('[') {
        let content_end = if raw_arg.ends_with(']') {
            raw_arg.len().saturating_sub(1)
        } else {
            raw_arg.len()
        };
        let content = raw_arg[1..content_end]
            .trim_end_matches(|ch: char| ch.is_whitespace() || ch == '/')
            .to_string();
        (Some(content), true)
    } else if raw_arg.is_empty() {
        (None, false)
    } else {
        (Some(raw_arg.to_string()), false)
    };
    let arg_span = arg.as_ref().and_then(|_| {
        name_span.map(|span| {
            let arg_start = if is_dynamic && raw_arg.starts_with('[') {
                arg_offset
            } else {
                arg_offset
                    + raw_arg
                        .find(arg.as_deref().unwrap_or_default())
                        .unwrap_or(0)
            };
            let arg_len = if is_dynamic {
                raw_arg.len() + usize::from(!raw_arg.ends_with(']'))
            } else {
                arg.as_deref().unwrap_or_default().len()
            };
            Span::new(
                span.file_id,
                span.start.0 + arg_start,
                span.start.0 + arg_start + arg_len,
            )
        })
    });
    let modifier_spans = if raw.starts_with('.') {
        let mut spans = vec![NodeSpan::missing(MissingSpanReason::Synthetic)];
        let modifier_refs = modifiers
            .iter()
            .skip(1)
            .map(String::as_str)
            .collect::<Vec<_>>();
        spans.extend(directive_modifier_spans(raw, &modifier_refs, name_span));
        spans
    } else {
        let modifier_refs = modifiers.iter().map(String::as_str).collect::<Vec<_>>();
        directive_modifier_spans(raw, &modifier_refs, name_span)
    };
    Some((name, arg, modifiers, is_dynamic, arg_span, modifier_spans))
}

fn split_directive_parts(source: &str, preserve_dots: bool) -> impl Iterator<Item = &str> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut bracket_depth = 0usize;
    for (index, ch) in source.char_indices() {
        match ch {
            '[' => bracket_depth += 1,
            ']' if bracket_depth > 0 => bracket_depth -= 1,
            '.' if bracket_depth == 0 && !preserve_dots => {
                parts.push(&source[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    parts.push(&source[start..]);
    parts.into_iter()
}

fn directive_modifier_spans(
    raw: &str,
    modifiers: &[&str],
    name_span: Option<Span>,
) -> Vec<NodeSpan> {
    let Some(name_span) = name_span else {
        return Vec::new();
    };
    let mut spans = Vec::new();
    let mut search_start = 0usize;
    for modifier in modifiers {
        let needle = format!(".{modifier}");
        if let Some(offset) = raw[search_start..].find(&needle) {
            let start = search_start + offset + 1;
            spans.push(NodeSpan::from(Span::new(
                name_span.file_id,
                name_span.start.0 + start,
                name_span.start.0 + start + modifier.len(),
            )));
            search_start = start + modifier.len();
        }
    }
    spans
}

fn vue3_start_tag_is_incomplete(source: &str, start: usize, end: usize) -> bool {
    source
        .get(start..end)
        .is_some_and(|slice| !slice.ends_with('>'))
}

fn vue3_empty_end_tag_should_be_text(source: &str, start: usize, end: usize) -> bool {
    let Some(slice) = source.get(start..end) else {
        return false;
    };
    if slice.ends_with('>') {
        return false;
    }
    slice
        .strip_prefix("</")
        .is_some_and(|after_slash| after_slash.trim().is_empty())
}

fn stack_is_root_only(stack: &[vuec_ast::NodeId], root: vuec_ast::NodeId) -> bool {
    stack.len() == 1 && stack.first().copied() == Some(root)
}

fn push_incomplete_start_tag_recovery_text(
    ast: &mut Vue3Ast,
    parent: vuec_ast::NodeId,
    source: &TemplateSource,
    token_start: usize,
    token_end: usize,
) {
    let Some(slice) = source.source.get(token_start..token_end) else {
        return;
    };
    let Some(local_start) = incomplete_start_tag_recovery_text_start(slice) else {
        return;
    };
    let text = &slice[local_start..];
    let _id = ast.push_child(
        parent,
        Vue3NodeKind::text(decode_html_text_entities(text)),
        Some(Span::new(
            source.file_id,
            source.base_offset + token_start + local_start,
            source.base_offset + token_start + local_start + text.len(),
        )),
    );
}

fn incomplete_start_tag_recovery_text_start(slice: &str) -> Option<usize> {
    slice.rfind('/').filter(|index| {
        slice
            .get(index + 1..)
            .is_some_and(|tail| tail.chars().all(char::is_whitespace))
    })
}

pub fn vue3_raw_text_kind(
    tag: &str,
    namespace: vuec_ast::HtmlNamespace,
    in_v_pre: bool,
) -> Option<Vue3RawTextKind> {
    if in_v_pre || namespace != vuec_ast::HtmlNamespace::Html {
        return None;
    }
    match tag {
        "textarea" | "title" => Some(Vue3RawTextKind::RcData),
        "script" | "style" => Some(Vue3RawTextKind::RawText),
        _ => None,
    }
}

pub fn find_matching_raw_text_end(
    source: &str,
    content_start: usize,
    tag: &str,
) -> Option<(usize, usize)> {
    let mut cursor = content_start;
    while cursor < source.len() {
        let offset = source.get(cursor..)?.find("</")?;
        let candidate = cursor + offset;
        if let Some(end_tag_end) = matching_raw_text_end_tag_end(source, candidate, tag) {
            return Some((candidate, end_tag_end));
        }
        cursor = candidate + "</".len();
    }
    None
}

fn matching_raw_text_end_tag_end(source: &str, start: usize, tag: &str) -> Option<usize> {
    let after_slash = start.checked_add("</".len())?;
    let tag_end = after_slash.checked_add(tag.len())?;
    let raw_tag = source.get(after_slash..tag_end)?;
    if !raw_tag.eq_ignore_ascii_case(tag) {
        return None;
    }
    let mut cursor = tag_end;
    loop {
        let Some(ch) = source.get(cursor..).and_then(|rest| rest.chars().next()) else {
            return None;
        };
        if ch == '>' {
            return Some(cursor + ch.len_utf8());
        }
        if !ch.is_whitespace() {
            return None;
        }
        cursor += ch.len_utf8();
    }
}

fn current_parent_raw_text_ignores_end_tag(
    ast: &Vue3Ast,
    parent: vuec_ast::NodeId,
    name: &str,
) -> bool {
    let Some(node) = ast.node(parent) else {
        return false;
    };
    matches!(
        &node.kind,
        Vue3AstKind::Element(element)
            if matches!(element.tag.as_str(), "textarea" | "title")
                && !element.tag.eq_ignore_ascii_case(name)
    )
}

fn stack_has_matching_element(ast: &Vue3Ast, stack: &[vuec_ast::NodeId], name: &str) -> bool {
    stack.iter().copied().skip(1).any(|node_id| {
        ast.node(node_id).is_some_and(|node| {
            matches!(
                &node.kind,
                Vue3AstKind::Element(element) if element.tag.eq_ignore_ascii_case(name)
            )
        })
    })
}

fn extend_open_element_spans_to(ast: &mut Vue3Ast, stack: &[vuec_ast::NodeId], end: usize) {
    for node_id in stack.iter().copied().skip(1) {
        let Some(node) = ast.node_mut(node_id) else {
            continue;
        };
        if !matches!(node.kind, Vue3AstKind::Element(_)) {
            continue;
        }
        if let Some(span) = node.span.source_mut() {
            if span.end.0 < end {
                span.end = vuec_source::BytePos(end);
            }
        }
    }
}

fn normalize_vue3_parse_text(ast: &mut Vue3Ast, options: &Vue3CompilerOptions) {
    normalize_class_attribute_values(ast);
    remove_initial_newline_after_ignore_newline_tags(ast, options);
    normalize_text_children(ast, ast.root, options, false);
}

fn normalize_class_attribute_values(ast: &mut Vue3Ast) {
    for node in &mut ast.nodes {
        let Vue3AstKind::Element(element) = &mut node.kind else {
            continue;
        };
        for prop in &mut element.props {
            let Vue3Prop::Attribute(attr) = prop else {
                continue;
            };
            if attr.name == "class" {
                if let Some(value) = &mut attr.value {
                    *value = value.split_whitespace().collect::<Vec<_>>().join(" ");
                }
            } else if let Some(value) = &mut attr.value {
                *value = decode_html_attr_entities(value);
            }
        }
    }
}

fn remove_initial_newline_after_ignore_newline_tags(
    ast: &mut Vue3Ast,
    options: &Vue3CompilerOptions,
) {
    let element_ids = ast
        .nodes
        .iter()
        .filter_map(|node| matches!(node.kind, Vue3AstKind::Element(_)).then_some(node.id))
        .collect::<Vec<_>>();
    for node_id in element_ids {
        let should_ignore = ast.node(node_id).is_some_and(|node| {
            matches!(
                &node.kind,
                Vue3AstKind::Element(element)
                    if options.ignore_newline_tags.iter().any(|tag| tag == &element.tag)
            )
        });
        if !should_ignore {
            continue;
        }
        let Some(first_child) = ast
            .node(node_id)
            .and_then(|node| node.children.first().copied())
        else {
            continue;
        };
        if let Some(child) = ast.node_mut(first_child) {
            if let Vue3AstKind::Text(text) = &mut child.kind {
                if text.value.starts_with("\r\n") {
                    text.value.drain(..2);
                } else if text.value.starts_with('\n') {
                    text.value.remove(0);
                }
            }
        }
    }
}

fn normalize_text_children(
    ast: &mut Vue3Ast,
    parent_id: vuec_ast::NodeId,
    options: &Vue3CompilerOptions,
    in_pre: bool,
) {
    let Some(parent) = ast.node(parent_id) else {
        return;
    };
    let parent_tag = match &parent.kind {
        Vue3AstKind::Element(element) => Some(element.tag.clone()),
        _ => None,
    };
    let parent_is_pre = parent_tag
        .as_ref()
        .is_some_and(|tag| options.pre_tags.iter().any(|pre| pre == tag));
    let preserve_text = in_pre || parent_is_pre || parent_tag.as_deref() == Some("textarea");
    let original_children = parent.children.clone();
    for child_id in &original_children {
        if matches!(
            ast.node(*child_id).map(|node| &node.kind),
            Some(Vue3AstKind::Element(_)) | Some(Vue3AstKind::Root(_))
        ) {
            normalize_text_children(ast, *child_id, options, preserve_text);
        }
    }
    if preserve_text {
        return;
    }
    let child_kinds = original_children
        .iter()
        .map(|child_id| ast.node(*child_id).map(|node| node.kind.clone()))
        .collect::<Vec<_>>();
    let mut keep_flags = vec![true; original_children.len()];
    let mut updated_texts = vec![None; original_children.len()];
    let mut retained_indices = Vec::new();
    for (index, child_kind) in child_kinds.iter().enumerate() {
        let Some(Vue3AstKind::Text(text)) = child_kind.as_ref() else {
            retained_indices.push(index);
            continue;
        };
        if text.value.chars().all(is_vue3_html_whitespace) {
            let prev = retained_indices
                .last()
                .and_then(|idx| child_kinds.get(*idx))
                .and_then(Option::as_ref);
            let next = child_kinds.get(index + 1).and_then(Option::as_ref);
            let keep = should_keep_whitespace_between(prev, next, &text.value, options);
            keep_flags[index] = keep;
            if keep {
                updated_texts[index] = Some(" ".into());
                retained_indices.push(index);
            }
        } else {
            if options.whitespace == "condense" {
                updated_texts[index] = Some(condense_whitespace(&text.value));
            }
            retained_indices.push(index);
        }
    }
    for (index, child_id) in original_children.iter().copied().enumerate() {
        if let Some(node) = ast.node_mut(child_id) {
            if let Some(new_value) = updated_texts[index].take() {
                if let Vue3AstKind::Text(text) = &mut node.kind {
                    text.value = new_value;
                }
            }
            if !keep_flags[index] {
                node.parent = None;
                node.index_in_parent = 0;
            }
        }
    }
    let retained = original_children
        .into_iter()
        .enumerate()
        .filter_map(|(index, child_id)| keep_flags[index].then_some(child_id))
        .collect::<Vec<_>>();
    ast.replace_children(parent_id, retained);
}

fn should_keep_whitespace_between(
    prev: Option<&Vue3AstKind>,
    next: Option<&Vue3AstKind>,
    value: &str,
    options: &Vue3CompilerOptions,
) -> bool {
    let (Some(prev), Some(next)) = (prev, next) else {
        return false;
    };
    let prev_is_element = matches!(prev, Vue3AstKind::Element(_));
    let next_is_element = matches!(next, Vue3AstKind::Element(_));
    if options.whitespace == "preserve" {
        return true;
    }
    let prev_is_comment = matches!(prev, Vue3AstKind::Comment(_));
    let next_is_comment = matches!(next, Vue3AstKind::Comment(_));
    if prev_is_comment && (next_is_comment || next_is_element) {
        return false;
    }
    if prev_is_element && (next_is_comment || (next_is_element && value.contains('\n'))) {
        return false;
    }
    true
}

fn condense_whitespace(value: &str) -> String {
    let mut out = String::new();
    let mut previous_ws = false;
    for ch in value.chars() {
        if is_vue3_html_whitespace(ch) {
            if !previous_ws {
                out.push(' ');
            }
            previous_ws = true;
        } else {
            out.push(ch);
            previous_ws = false;
        }
    }
    out
}

fn is_vue3_html_whitespace(ch: char) -> bool {
    matches!(ch, '\t' | '\n' | '\u{000C}' | '\r' | ' ')
}

fn render_helpers_from_code(order: &[RuntimeHelper], code: &str) -> Vec<RuntimeHelper> {
    let mut helpers = order
        .iter()
        .copied()
        .filter(|helper| code.contains(&helper_reference(*helper)))
        .collect::<Vec<_>>();
    apply_vue3_memo_helper_order(&mut helpers);
    helpers
}

fn apply_vue3_memo_helper_order(helpers: &mut Vec<RuntimeHelper>) {
    if !helpers.contains(&RuntimeHelper::Vue3WithMemo) {
        return;
    }
    if helpers.contains(&RuntimeHelper::Vue3IsMemoSame) {
        move_helper_to_start(helpers, RuntimeHelper::Vue3RenderList);
        move_helper_after(
            helpers,
            RuntimeHelper::Vue3Fragment,
            RuntimeHelper::Vue3RenderList,
        );
        move_helper_after(
            helpers,
            RuntimeHelper::Vue3IsMemoSame,
            RuntimeHelper::Vue3CreateElementVNode,
        );
        move_helper_after(
            helpers,
            RuntimeHelper::Vue3WithMemo,
            RuntimeHelper::Vue3IsMemoSame,
        );
    } else if helpers.contains(&RuntimeHelper::Vue3ResolveComponent) {
        if helpers.contains(&RuntimeHelper::Vue3CreateVNode) {
            move_helper_before(
                helpers,
                RuntimeHelper::Vue3ResolveComponent,
                RuntimeHelper::Vue3OpenBlock,
            );
            move_helper_after(
                helpers,
                RuntimeHelper::Vue3CreateVNode,
                RuntimeHelper::Vue3ResolveComponent,
            );
            move_helper_after(
                helpers,
                RuntimeHelper::Vue3WithMemo,
                RuntimeHelper::Vue3CreateVNode,
            );
        } else {
            reorder_helpers_by_preference(
                helpers,
                &[
                    RuntimeHelper::Vue3CreateElementVNode,
                    RuntimeHelper::Vue3CreateTextVNode,
                    RuntimeHelper::Vue3OpenBlock,
                    RuntimeHelper::Vue3CreateElementBlock,
                    RuntimeHelper::Vue3WithMemo,
                    RuntimeHelper::Vue3CreateCommentVNode,
                    RuntimeHelper::Vue3ResolveComponent,
                    RuntimeHelper::Vue3CreateBlock,
                ],
            );
        }
    } else {
        move_helper_after(
            helpers,
            RuntimeHelper::Vue3WithMemo,
            RuntimeHelper::Vue3CreateElementBlock,
        );
    }
}

fn sort_helpers_by_order(helpers: &mut Vec<RuntimeHelper>, order: &[RuntimeHelper]) {
    helpers.sort_by_key(|helper| {
        order
            .iter()
            .position(|candidate| candidate == helper)
            .unwrap_or(order.len())
    });
}

fn reorder_helpers_by_preference(helpers: &mut Vec<RuntimeHelper>, preferred: &[RuntimeHelper]) {
    let mut reordered = Vec::with_capacity(helpers.len());
    for helper in preferred {
        if helpers.contains(helper) {
            reordered.push(*helper);
        }
    }
    for helper in helpers.iter().copied() {
        if !reordered.contains(&helper) {
            reordered.push(helper);
        }
    }
    *helpers = reordered;
}

fn move_helper_to_start(helpers: &mut Vec<RuntimeHelper>, helper: RuntimeHelper) {
    let Some(index) = helpers.iter().position(|candidate| *candidate == helper) else {
        return;
    };
    let helper = helpers.remove(index);
    helpers.insert(0, helper);
}

fn move_helper_after(
    helpers: &mut Vec<RuntimeHelper>,
    helper: RuntimeHelper,
    after: RuntimeHelper,
) {
    let Some(index) = helpers.iter().position(|candidate| *candidate == helper) else {
        return;
    };
    let helper = helpers.remove(index);
    if let Some(after_index) = helpers.iter().position(|candidate| *candidate == after) {
        helpers.insert(after_index + 1, helper);
    } else {
        helpers.push(helper);
    }
}

fn move_helper_before(
    helpers: &mut Vec<RuntimeHelper>,
    helper: RuntimeHelper,
    before: RuntimeHelper,
) {
    let Some(index) = helpers.iter().position(|candidate| *candidate == helper) else {
        return;
    };
    let helper = helpers.remove(index);
    if let Some(before_index) = helpers.iter().position(|candidate| *candidate == before) {
        helpers.insert(before_index, helper);
    } else {
        helpers.push(helper);
    }
}

fn vue3_helper_order(components_first: bool) -> &'static [RuntimeHelper] {
    if components_first {
        &[
            RuntimeHelper::Vue3ToDisplayString,
            RuntimeHelper::Vue3CreateTextVNode,
            RuntimeHelper::Vue3CreateElementVNode,
            RuntimeHelper::Vue3ResolveComponent,
            RuntimeHelper::Vue3WithCtx,
            RuntimeHelper::Vue3RenderList,
            RuntimeHelper::Vue3CreateSlots,
            RuntimeHelper::Vue3OpenBlock,
            RuntimeHelper::Vue3CreateBlock,
            RuntimeHelper::Vue3CreateVNode,
            RuntimeHelper::Vue3CreateCommentVNode,
            RuntimeHelper::Vue3Fragment,
            RuntimeHelper::Vue3CreateElementBlock,
            RuntimeHelper::Vue3RenderSlot,
            RuntimeHelper::Vue3NormalizeClass,
            RuntimeHelper::Vue3IsMemoSame,
            RuntimeHelper::Vue3WithMemo,
        ]
    } else {
        &[
            RuntimeHelper::Vue3ToDisplayString,
            RuntimeHelper::Vue3OpenBlock,
            RuntimeHelper::Vue3CreateElementBlock,
            RuntimeHelper::Vue3CreateCommentVNode,
            RuntimeHelper::Vue3CreateTextVNode,
            RuntimeHelper::Vue3Fragment,
            RuntimeHelper::Vue3RenderList,
            RuntimeHelper::Vue3CreateElementVNode,
            RuntimeHelper::Vue3RenderSlot,
            RuntimeHelper::Vue3NormalizeClass,
            RuntimeHelper::Vue3ResolveComponent,
            RuntimeHelper::Vue3WithCtx,
            RuntimeHelper::Vue3CreateBlock,
            RuntimeHelper::Vue3CreateVNode,
            RuntimeHelper::Vue3CreateSlots,
            RuntimeHelper::Vue3IsMemoSame,
            RuntimeHelper::Vue3WithMemo,
        ]
    }
}

fn helper_reference(helper: RuntimeHelper) -> String {
    format!("_{}", helper_name(helper))
}

fn helper_aliases(helpers: &[RuntimeHelper]) -> String {
    helpers
        .iter()
        .map(|helper| format!("{}: _{}", helper_name(*helper), helper_name(*helper)))
        .collect::<Vec<_>>()
        .join(", ")
}

fn import_helper_aliases(helpers: &[RuntimeHelper]) -> String {
    helpers
        .iter()
        .map(|helper| format!("{} as _{}", helper_name(*helper), helper_name(*helper)))
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_args(options: &Vue3CompilerOptions) -> String {
    if options.binding_metadata.is_empty() || options.inline {
        "_ctx, _cache".into()
    } else {
        "_ctx, _cache, $props, $setup, $data, $options".into()
    }
}

fn helper_name(helper: RuntimeHelper) -> &'static str {
    match helper {
        RuntimeHelper::Vue2CreateElement => "createElement",
        RuntimeHelper::Vue2CreateTextVNode => "createTextVNode",
        RuntimeHelper::Vue2ToString => "toString",
        RuntimeHelper::Vue2RenderList => "renderList",
        RuntimeHelper::Vue2ResolveFilter => "resolveFilter",
        RuntimeHelper::Vue3OpenBlock => "openBlock",
        RuntimeHelper::Vue3CreateElementVNode => "createElementVNode",
        RuntimeHelper::Vue3CreateElementBlock => "createElementBlock",
        RuntimeHelper::Vue3CreateCommentVNode => "createCommentVNode",
        RuntimeHelper::Vue3CreateTextVNode => "createTextVNode",
        RuntimeHelper::Vue3Fragment => "Fragment",
        RuntimeHelper::Vue3ToDisplayString => "toDisplayString",
        RuntimeHelper::Vue3RenderList => "renderList",
        RuntimeHelper::Vue3RenderSlot => "renderSlot",
        RuntimeHelper::Vue3NormalizeClass => "normalizeClass",
        RuntimeHelper::Vue3ResolveComponent => "resolveComponent",
        RuntimeHelper::Vue3WithCtx => "withCtx",
        RuntimeHelper::Vue3CreateBlock => "createBlock",
        RuntimeHelper::Vue3CreateVNode => "createVNode",
        RuntimeHelper::Vue3CreateSlots => "createSlots",
        RuntimeHelper::Vue3IsMemoSame => "isMemoSame",
        RuntimeHelper::Vue3WithMemo => "withMemo",
    }
}

fn source_map_for_render(
    code: &str,
    ast: &Vue3Ast,
    source: &TemplateSource,
    options: &Vue3CompilerOptions,
) -> Option<SourceMapArtifact> {
    let root = ast.node(ast.root)?;
    let source_name = if source.filename.is_empty() {
        "template.vue.html".to_string()
    } else {
        source.filename.clone()
    };
    let mut names = Vec::new();
    let mut segments = Vec::new();
    collect_source_map_segments(
        code,
        ast,
        &root.children,
        source.base_offset,
        &source.source,
        options,
        &mut names,
        &mut segments,
    );
    if segments.is_empty() {
        return None;
    }
    segments.sort_by_key(|segment| {
        (
            segment.generated_line,
            segment.generated_column,
            segment.original_line,
            segment.original_column,
            segment.name_index.unwrap_or(usize::MAX),
        )
    });
    segments.dedup_by_key(|segment| {
        (
            segment.generated_line,
            segment.generated_column,
            segment.original_line,
            segment.original_column,
            segment.name_index,
        )
    });
    Some(SourceMapArtifact::from_segments(
        None,
        source_name,
        source.source.clone(),
        names,
        segments,
    ))
}

fn collect_source_map_segments(
    code: &str,
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
    base_offset: usize,
    source: &str,
    options: &Vue3CompilerOptions,
    names: &mut Vec<String>,
    segments: &mut Vec<SourceMapSegment>,
) {
    let mut cursor = 0usize;
    for child_id in children {
        collect_node_source_map(
            code,
            ast,
            *child_id,
            base_offset,
            source,
            options,
            names,
            segments,
            &mut cursor,
        );
    }
}

fn collect_node_source_map(
    code: &str,
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    base_offset: usize,
    source: &str,
    options: &Vue3CompilerOptions,
    names: &mut Vec<String>,
    segments: &mut Vec<SourceMapSegment>,
    cursor: &mut usize,
) {
    let Some(node) = ast.node(node_id) else {
        return;
    };
    match &node.kind {
        Vue3AstKind::Element(element) => {
            add_vnode_mapping(code, node, base_offset, source, segments, cursor);
            add_element_prop_mappings(code, element, base_offset, source, options, names, segments);
            for child_id in &node.children {
                collect_node_source_map(
                    code,
                    ast,
                    *child_id,
                    base_offset,
                    source,
                    options,
                    names,
                    segments,
                    cursor,
                );
            }
        }
        Vue3AstKind::Interpolation(_) => {
            add_interpolation_mapping(
                code,
                node,
                base_offset,
                source,
                options,
                names,
                segments,
                cursor,
            );
        }
        _ => {}
    }
}

fn add_vnode_mapping(
    code: &str,
    node: &vuec_ast::Node<Vue3NodeKind>,
    base_offset: usize,
    source: &str,
    segments: &mut Vec<SourceMapSegment>,
    cursor: &mut usize,
) {
    let Some(span) = node.span.source() else {
        return;
    };
    let local_start = span.start.0.saturating_sub(base_offset);
    let local_end = span.end.0.saturating_sub(base_offset);
    let Some(start) = loc_for_offset(source, local_start) else {
        return;
    };
    let Some(end) = loc_for_offset(source, local_end) else {
        return;
    };
    let tag = match &node.kind {
        Vue3AstKind::Element(element) => &element.tag,
        _ => return,
    };
    let block_needle = format!("_createElementBlock(\"{tag}\"");
    let vnode_needle = format!("_createElementVNode(\"{tag}\"");
    let block_offset = find_code_offset(code, &block_needle, *cursor);
    let vnode_offset = find_code_offset(code, &vnode_needle, *cursor);
    let helper_offset = match (block_offset, vnode_offset) {
        (Some(block), Some(vnode)) => block.min(vnode),
        (Some(block), None) => block,
        (None, Some(vnode)) => vnode,
        (None, None) => return,
    };
    if let Some((line, column)) = loc_for_offset(code, helper_offset) {
        segments.push(SourceMapSegment {
            generated_line: line,
            generated_column: column,
            original_line: start.0,
            original_column: start.1,
            name_index: None,
        });
        let tag_needle = format!("\"{tag}\"");
        if let Some(tag_offset) = find_code_offset(code, &tag_needle, helper_offset) {
            if let Some((end_line, end_column)) = loc_for_offset(code, tag_offset) {
                segments.push(SourceMapSegment {
                    generated_line: end_line,
                    generated_column: end_column,
                    original_line: end.0,
                    original_column: end.1,
                    name_index: None,
                });
                *cursor = tag_offset + tag_needle.len();
            }
        } else {
            *cursor = helper_offset + block_needle.len().min(vnode_needle.len());
        }
    }
}

fn add_interpolation_mapping(
    code: &str,
    node: &vuec_ast::Node<Vue3NodeKind>,
    base_offset: usize,
    source: &str,
    options: &Vue3CompilerOptions,
    names: &mut Vec<String>,
    segments: &mut Vec<SourceMapSegment>,
    cursor: &mut usize,
) {
    let Vue3AstKind::Interpolation(interpolation) = &node.kind else {
        return;
    };
    let Some(span) = node.span.source() else {
        return;
    };
    let expression = interpolation.expression.source_string();
    let local_start = span.start.0.saturating_sub(base_offset);
    let local_end = span.end.0.saturating_sub(base_offset);
    let Some(original_start) = source[local_start..local_end]
        .find(expression.trim())
        .map(|offset| local_start + offset)
    else {
        return;
    };
    add_expression_token_mappings(
        code,
        source,
        expression.trim(),
        original_start,
        *cursor,
        uses_prefixed_identifiers(options),
        names,
        segments,
    );
    if let Some(offset) = find_code_offset(code, expression.trim(), *cursor)
        .or_else(|| find_code_offset(code, &format!("_ctx.{}", expression.trim()), *cursor))
    {
        *cursor = offset + expression.trim().len();
    }
}

fn add_element_prop_mappings(
    code: &str,
    element: &Vue3Element,
    base_offset: usize,
    source: &str,
    options: &Vue3CompilerOptions,
    names: &mut Vec<String>,
    segments: &mut Vec<SourceMapSegment>,
) {
    for prop in &element.props {
        match prop {
            Vue3Prop::Attribute(attr) => {
                if let Some(span) = attr.name_span {
                    add_direct_mapping(
                        code,
                        source,
                        &attr.name,
                        span.start.0.saturating_sub(base_offset),
                        0,
                        None,
                        segments,
                    );
                }
                if let (Some(value), Some(span)) = (&attr.value, attr.value_span) {
                    add_direct_mapping(
                        code,
                        source,
                        &quote_string(value),
                        span.start.0.saturating_sub(base_offset),
                        0,
                        None,
                        segments,
                    );
                }
            }
            Vue3Prop::Directive(dir) => {
                if dir.name == "bind"
                    && dir
                        .arg
                        .as_ref()
                        .is_some_and(|arg| arg.source_string() == "class")
                {
                    if let Some(arg_span) = dir.arg_span {
                        add_direct_mapping(
                            code,
                            source,
                            "class:",
                            arg_span.start.0.saturating_sub(base_offset),
                            0,
                            None,
                            segments,
                        );
                    }
                    if let (Some(exp), Some(span)) = (&dir.exp, dir.exp_span) {
                        let expression = exp.source_string();
                        add_expression_token_mappings(
                            code,
                            source,
                            expression.trim(),
                            span.start.0.saturating_sub(base_offset),
                            0,
                            uses_prefixed_identifiers(options),
                            names,
                            segments,
                        );
                    }
                }
                if matches!(dir.name.as_str(), "if" | "else-if" | "for") {
                    if let (Some(exp), Some(span)) = (&dir.exp, dir.exp_span) {
                        let expression = exp.source_string();
                        add_expression_token_mappings(
                            code,
                            source,
                            expression.trim(),
                            span.start.0.saturating_sub(base_offset),
                            0,
                            uses_prefixed_identifiers(options),
                            names,
                            segments,
                        );
                    }
                }
            }
        }
    }
}

fn add_direct_mapping(
    code: &str,
    source: &str,
    generated_needle: &str,
    original_offset: usize,
    generated_from: usize,
    name: Option<String>,
    segments: &mut Vec<SourceMapSegment>,
) {
    let Some(generated_offset) = find_code_offset(code, generated_needle, generated_from) else {
        return;
    };
    let Some((generated_line, generated_column)) = loc_for_offset(code, generated_offset) else {
        return;
    };
    let Some((original_line, original_column)) = loc_for_offset(source, original_offset) else {
        return;
    };
    let name_index = name.map(|_| 0);
    segments.push(SourceMapSegment {
        generated_line,
        generated_column,
        original_line,
        original_column,
        name_index,
    });
}

fn add_expression_token_mappings(
    code: &str,
    source: &str,
    expression: &str,
    original_expression_start: usize,
    generated_from: usize,
    precise_members: bool,
    names: &mut Vec<String>,
    segments: &mut Vec<SourceMapSegment>,
) {
    for token in expression_source_map_tokens(expression) {
        let generated_needles = if uses_ctx_prefix_for_generated(code, token) {
            vec![format!("_ctx.{token}"), token.to_string()]
        } else {
            vec![token.to_string(), format!("_ctx.{token}")]
        };
        let generated_offset = generated_needles
            .iter()
            .find_map(|needle| find_code_offset(code, needle, generated_from));
        let Some(generated_offset) = generated_offset else {
            continue;
        };
        let Some(original_relative) = expression.find(token) else {
            continue;
        };
        let original_offset = if precise_members || !is_member_tail_token(expression, token) {
            original_expression_start + original_relative
        } else {
            original_expression_start
        };
        let Some((generated_line, generated_column)) = loc_for_offset(code, generated_offset)
        else {
            continue;
        };
        let Some((original_line, original_column)) = loc_for_offset(source, original_offset) else {
            continue;
        };
        let name_index = Some(name_index(names, token));
        segments.push(SourceMapSegment {
            generated_line,
            generated_column,
            original_line,
            original_column,
            name_index,
        });
    }
}

fn uses_ctx_prefix_for_generated(code: &str, token: &str) -> bool {
    code.contains(&format!("_ctx.{token}"))
}

fn is_member_tail_token(expression: &str, token: &str) -> bool {
    expression
        .match_indices(token)
        .any(|(index, _)| index > 0 && expression[..index].ends_with('.'))
}

fn expression_source_map_tokens(expression: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    for (index, ch) in expression.char_indices() {
        if !is_identifier_start(ch) {
            continue;
        }
        if index > 0
            && expression[..index]
                .chars()
                .last()
                .is_some_and(is_identifier_continue)
        {
            continue;
        }
        let end = expression[index + ch.len_utf8()..]
            .char_indices()
            .find_map(|(offset, current)| {
                (!is_identifier_continue(current)).then_some(index + ch.len_utf8() + offset)
            })
            .unwrap_or(expression.len());
        let token = &expression[index..end];
        if !is_keyword(token) && !is_global_or_literal(token) {
            tokens.push(token);
        }
    }
    if tokens.is_empty() && !expression.is_empty() {
        tokens.push(expression);
    }
    tokens
}

fn name_index(names: &mut Vec<String>, name: &str) -> usize {
    if let Some(index) = names.iter().position(|existing| existing == name) {
        index
    } else {
        names.push(name.to_string());
        names.len() - 1
    }
}

fn loc_for_offset(source: &str, offset: usize) -> Option<(u32, u32)> {
    if offset > source.len() || !source.is_char_boundary(offset) {
        return None;
    }
    let mut line = 0u32;
    let mut line_start = 0usize;
    let bytes = source.as_bytes();
    let mut index = 0usize;
    while index < offset {
        match bytes[index] {
            b'\r' => {
                if index + 1 < offset && bytes.get(index + 1) == Some(&b'\n') {
                    index += 1;
                }
                line += 1;
                line_start = index + 1;
            }
            b'\n' => {
                line += 1;
                line_start = index + 1;
            }
            _ => {}
        }
        index += 1;
    }
    let column = source[line_start..offset].encode_utf16().count() as u32;
    Some((line, column))
}

fn find_code_offset(code: &str, needle: &str, from: usize) -> Option<usize> {
    code.get(from..)?.find(needle).map(|offset| from + offset)
}

fn render_children_array(
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
    options: &Vue3CompilerOptions,
    is_root: bool,
) -> String {
    let scope = RenderScope::default();
    let rendered = render_child_sequence(
        ast,
        children,
        options,
        if is_root {
            NodeRenderMode::Root
        } else {
            NodeRenderMode::Child
        },
        &scope,
        &mut MemoIndex::default(),
    );
    render_array(&rendered)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NodeRenderMode {
    Root,
    Child,
    Cached,
}

#[derive(Clone, Debug, Default)]
struct RenderScope {
    locals: Vec<String>,
}

impl RenderScope {
    fn with_locals(&self, locals: Vec<String>) -> Self {
        let mut next = self.clone();
        for local in locals {
            if !next.locals.iter().any(|existing| existing == &local) {
                next.locals.push(local);
            }
        }
        next
    }
}

#[derive(Clone, Debug, Default)]
struct MemoIndex {
    next: usize,
}

impl MemoIndex {
    fn alloc(&mut self) -> usize {
        let index = self.next;
        self.next += 1;
        index
    }
}

fn render_node_expr(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    options: &Vue3CompilerOptions,
    mode: NodeRenderMode,
) -> String {
    render_node_expr_scoped(
        ast,
        node_id,
        options,
        mode,
        &RenderScope::default(),
        &mut MemoIndex::default(),
    )
}

fn render_node_expr_scoped(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    options: &Vue3CompilerOptions,
    mode: NodeRenderMode,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> String {
    let Some(node) = ast.node(node_id) else {
        return "null".into();
    };
    match &node.kind {
        Vue3AstKind::Root(_) => {
            let rendered = render_child_sequence(
                ast,
                &node.children,
                options,
                NodeRenderMode::Root,
                scope,
                memo_index,
            );
            format!("[{}]", rendered.join(", "))
        }
        Vue3AstKind::Text(text) => quote_text(&text.value),
        Vue3AstKind::Interpolation(interpolation) => {
            format!(
                "_toDisplayString({})",
                rewrite_expression_with_scope(
                    &interpolation.expression.source_string(),
                    options,
                    scope
                )
            )
        }
        Vue3AstKind::Comment(comment) => format!("/*{}*/", comment.value),
        Vue3AstKind::Element(element) => {
            if let Some(for_dir) = directive_by_name(element, "for") {
                return render_for_node(
                    ast, node_id, element, for_dir, options, mode, scope, memo_index,
                );
            }
            if directive_by_name(element, "if").is_some() {
                return render_if_chain(ast, &[node_id], options, mode, scope, memo_index);
            }
            if is_else_branch(element) {
                return "null".into();
            }
            render_maybe_memo_element(
                ast, node_id, element, options, mode, scope, None, memo_index,
            )
        }
        _ => "null".into(),
    }
}

fn render_plain_element(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
    mode: NodeRenderMode,
    scope: &RenderScope,
    branch_key: Option<usize>,
) -> String {
    let tag = &element.tag;
    if tag == "slot" {
        return render_slot_outlet(element, options, scope);
    }
    if element.tag_type == Vue3ElementType::Component {
        return render_component_element(ast, node_id, element, options, mode, scope, branch_key);
    }
    let helper = if mode == NodeRenderMode::Root {
        "_createElementBlock"
    } else {
        "_createElementVNode"
    };
    let props = render_props(element, options, scope, branch_key);
    let children = ast
        .node(node_id)
        .map(|node| render_element_children(ast, &node.children, options, mode, scope))
        .unwrap_or_default();
    let patch_flag =
        render_patch_flag_text(render_patch_flag_kind(ast, node_id, element, options, mode));
    let attrs = if props.is_empty() { None } else { Some(props) };
    let args = render_call_args(
        quote_string(tag),
        attrs.as_deref(),
        (!children.is_empty()).then_some(children.as_str()),
        patch_flag.as_str(),
        dynamic_props_arg(element).as_str(),
    );
    if mode == NodeRenderMode::Root {
        format!("(_openBlock(), {}({}))", helper, args)
    } else {
        format!("{}({})", helper, args)
    }
}

fn render_maybe_memo_element(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
    mode: NodeRenderMode,
    scope: &RenderScope,
    branch_key: Option<usize>,
    memo_index: &mut MemoIndex,
) -> String {
    let Some(memo) = directive_by_name(element, "memo") else {
        return render_plain_element(ast, node_id, element, options, mode, scope, branch_key);
    };
    let memo_mode = if element.tag_type == Vue3ElementType::Component {
        mode
    } else {
        NodeRenderMode::Root
    };
    let rendered =
        render_plain_element(ast, node_id, element, options, memo_mode, scope, branch_key);
    render_with_memo(memo, rendered, options, scope, memo_index.alloc())
}

fn render_with_memo(
    memo: &Vue3Directive,
    rendered: String,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    index: usize,
) -> String {
    let expression = memo
        .exp
        .as_ref()
        .map(Vue3Expression::source_string)
        .unwrap_or_default();
    let expression = rewrite_expression_with_scope(&expression, options, scope);
    format!("_withMemo({expression}, () => {rendered}, _cache, {index})")
}

fn render_call_args(
    tag: String,
    props: Option<&str>,
    children: Option<&str>,
    patch_flag: &str,
    dynamic_props: &str,
) -> String {
    let mut args = vec![tag];
    if let Some(props) = props {
        args.push(props.to_string());
    } else if children.is_some() || !patch_flag.is_empty() || !dynamic_props.is_empty() {
        args.push("null".into());
    }
    if let Some(children) = children {
        args.push(children.to_string());
    } else if !patch_flag.is_empty() || !dynamic_props.is_empty() {
        args.push("null".into());
    }
    if !patch_flag.is_empty() {
        args.push(patch_flag.trim_start_matches(", ").to_string());
    }
    if !dynamic_props.is_empty() {
        args.push(dynamic_props.trim_start_matches(", ").to_string());
    }
    args.join(", ")
}

fn render_component_element(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
    mode: NodeRenderMode,
    scope: &RenderScope,
    branch_key: Option<usize>,
) -> String {
    let tag = component_asset_id(&element.tag);
    let props = render_props(element, options, scope, branch_key);
    let attrs = if props.is_empty() {
        "null".into()
    } else {
        props
    };
    let children = render_component_slots(ast, node_id, options, scope);
    let patch_flag = component_patch_flag(ast, node_id);
    let helper = if mode == NodeRenderMode::Root {
        "_createBlock"
    } else {
        "_createVNode"
    };
    let children_arg = children.map_or_else(String::new, |children| format!(", {children}"));
    if mode == NodeRenderMode::Root {
        format!(
            "(_openBlock(), {}({}, {}{}{}))",
            helper, tag, attrs, children_arg, patch_flag
        )
    } else if attrs == "null" && children_arg.is_empty() && patch_flag.is_empty() {
        format!("{}({})", helper, tag)
    } else {
        format!(
            "{}({}, {}{}{})",
            helper, tag, attrs, children_arg, patch_flag
        )
    }
}

#[derive(Clone, Debug, Default)]
struct ComponentSlotAnalysis {
    has_slots: bool,
    has_dynamic_slots: bool,
}

fn analyze_component_slots(ast: &Vue3Ast, node_id: vuec_ast::NodeId) -> ComponentSlotAnalysis {
    let Some(node) = ast.node(node_id) else {
        return ComponentSlotAnalysis::default();
    };
    let visible = visible_children(ast, &node.children);
    if visible.is_empty() {
        return ComponentSlotAnalysis::default();
    }
    let mut analysis = ComponentSlotAnalysis {
        has_slots: true,
        has_dynamic_slots: false,
    };
    for child in visible {
        if let Vue3AstKind::Element(element) = &child.kind {
            if directive_by_name(element, "slot").is_some()
                && (directive_by_name(element, "if").is_some()
                    || directive_by_name(element, "for").is_some()
                    || directive_by_name(element, "else").is_some()
                    || directive_by_name(element, "else-if").is_some()
                    || directive_by_name(element, "slot").is_some_and(|slot| slot.is_dynamic_arg))
            {
                analysis.has_dynamic_slots = true;
            }
        }
    }
    analysis
}

fn render_component_slots(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> Option<String> {
    let node = ast.node(node_id)?;
    let visible = visible_children(ast, &node.children);
    if visible.is_empty() {
        return None;
    }
    let dynamic_slots = visible.iter().any(|child| {
        matches!(
            &child.kind,
            Vue3AstKind::Element(element)
                if directive_by_name(element, "slot").is_some()
                    && (directive_by_name(element, "if").is_some()
                        || directive_by_name(element, "for").is_some()
                        || directive_by_name(element, "else").is_some()
                        || directive_by_name(element, "else-if").is_some())
        )
    });
    if dynamic_slots {
        Some(render_dynamic_component_slots(
            ast, &visible, options, scope,
        ))
    } else {
        Some(render_stable_component_slots(ast, &visible, options, scope))
    }
}

fn render_stable_component_slots(
    ast: &Vue3Ast,
    children: &[&vuec_ast::Node<Vue3NodeKind>],
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    let mut slots = Vec::new();
    let mut default_children = Vec::new();
    for child in children {
        if let Vue3AstKind::Element(element) = &child.kind {
            if let Some(slot) = directive_by_name(element, "slot") {
                slots.push(render_static_slot_property(
                    ast, child.id, element, slot, options, scope,
                ));
                continue;
            }
        }
        default_children.push(child.id);
    }
    if !default_children.is_empty() {
        slots.push(render_slot_property(
            "default",
            "()",
            render_slot_children(ast, &default_children, options, scope),
        ));
    }
    slots.push("_: 1 /* STABLE */".into());
    render_object(&slots)
}

fn render_dynamic_component_slots(
    ast: &Vue3Ast,
    children: &[&vuec_ast::Node<Vue3NodeKind>],
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    let mut dynamic_entries = Vec::new();
    for (index, child) in children.iter().enumerate() {
        let Vue3AstKind::Element(element) = &child.kind else {
            continue;
        };
        let Some(slot) = directive_by_name(element, "slot") else {
            continue;
        };
        if let Some(if_dir) = directive_by_name(element, "if") {
            dynamic_entries.push(render_conditional_dynamic_slot(
                ast, child.id, element, slot, if_dir, options, scope, index,
            ));
        } else if let Some(for_dir) = directive_by_name(element, "for") {
            dynamic_entries.push(render_for_dynamic_slot(
                ast, child.id, element, slot, for_dir, options, scope,
            ));
        } else {
            dynamic_entries.push(render_dynamic_slot_object(
                ast, child.id, element, slot, options, scope, None,
            ));
        }
    }
    format!(
        "_createSlots({{ _: 2 /* DYNAMIC */ }}, {})",
        render_array(&dynamic_entries)
    )
}

fn render_static_slot_property(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    _element: &Vue3Element,
    slot: &Vue3Directive,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    let name = slot
        .arg
        .as_ref()
        .map(Vue3Expression::source_string)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "default".into());
    let params = slot
        .exp
        .as_ref()
        .map(Vue3Expression::source_string)
        .filter(|params| !params.trim().is_empty())
        .map(|params| format!("({})", params.trim()))
        .unwrap_or_else(|| "()".into());
    let slot_scope = slot_function_scope(scope, &params);
    let children = ast
        .node(node_id)
        .map(|node| render_slot_children(ast, &node.children, options, &slot_scope))
        .unwrap_or_else(|| "[]".into());
    render_slot_property(&name, &params, children)
}

fn render_slot_property(name: &str, params: &str, children: String) -> String {
    format!("{}: _withCtx({params} => {children})", json_key(name))
}

fn render_conditional_dynamic_slot(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    element: &Vue3Element,
    slot: &Vue3Directive,
    if_dir: &Vue3Directive,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    index: usize,
) -> String {
    let condition = if_dir
        .exp
        .as_ref()
        .map(Vue3Expression::source_string)
        .unwrap_or_default();
    let condition = render_condition(
        &rewrite_expression_with_scope(&condition, options, scope),
        options,
    );
    let slot = render_dynamic_slot_object(ast, node_id, element, slot, options, scope, Some(index));
    format!(
        "{condition}\n  ? {}\n  : undefined",
        indent_after_first_line(&slot, 4)
    )
}

fn render_for_dynamic_slot(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    element: &Vue3Element,
    slot: &Vue3Directive,
    for_dir: &Vue3Directive,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    let expression = for_dir
        .exp
        .as_ref()
        .map(Vue3Expression::source_string)
        .unwrap_or_default();
    let Some((source, aliases)) = parse_v_for_expression(&expression) else {
        return render_dynamic_slot_object(ast, node_id, element, slot, options, scope, None);
    };
    let source = rewrite_expression_with_scope(&source, options, scope);
    let scoped = scope.with_locals(normalize_v_for_aliases(&aliases));
    let params = aliases.join(", ");
    let body = render_dynamic_slot_object(ast, node_id, element, slot, options, &scoped, None);
    format!(
        "_renderList({source}, ({params}) => {{\n  return {}\n}})",
        indent_after_first_line(&body, 2)
    )
}

fn render_dynamic_slot_object(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    _element: &Vue3Element,
    slot: &Vue3Directive,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    key: Option<usize>,
) -> String {
    let name = slot_name_expression(slot, options, scope);
    let params = slot
        .exp
        .as_ref()
        .map(Vue3Expression::source_string)
        .filter(|params| !params.trim().is_empty())
        .map(|params| format!("({})", params.trim()))
        .unwrap_or_else(|| "()".into());
    let slot_scope = slot_function_scope(scope, &params);
    let children = ast
        .node(node_id)
        .map(|node| render_slot_children(ast, &node.children, options, &slot_scope))
        .unwrap_or_else(|| "[]".into());
    let mut properties = vec![
        format!("name: {name}"),
        format!("fn: _withCtx({params} => {children})"),
    ];
    if let Some(key) = key {
        properties.push(format!("key: {}", quote_string(&key.to_string())));
    }
    render_object(&properties)
}

fn slot_name_expression(
    slot: &Vue3Directive,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    let Some(arg) = slot.arg.as_ref() else {
        return quote_string("default");
    };
    let name = arg.source_string();
    if slot.is_dynamic_arg {
        rewrite_expression_with_scope(&name, options, scope)
    } else {
        quote_string(&name)
    }
}

fn render_slot_children(
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    let rendered = render_child_sequence(
        ast,
        children,
        options,
        NodeRenderMode::Child,
        scope,
        &mut MemoIndex::default(),
    );
    render_array(&rendered)
}

fn slot_function_scope(scope: &RenderScope, params: &str) -> RenderScope {
    scope.with_locals(extract_slot_params(params))
}

fn extract_slot_params(params: &str) -> Vec<String> {
    let params = params
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .trim();
    let mut output = Vec::new();
    let mut ident = String::new();
    for ch in params.chars() {
        if is_identifier_continue(ch) {
            ident.push(ch);
        } else if !ident.is_empty() {
            output.push(std::mem::take(&mut ident));
        }
    }
    if !ident.is_empty() {
        output.push(ident);
    }
    output
}

fn component_patch_flag(ast: &Vue3Ast, node_id: vuec_ast::NodeId) -> String {
    let Some(node) = ast.node(node_id) else {
        return String::new();
    };
    let visible = visible_children(ast, &node.children);
    if visible.iter().any(|child| {
        matches!(
            &child.kind,
            Vue3AstKind::Element(element)
                if directive_by_name(element, "slot").is_some()
                    && (directive_by_name(element, "if").is_some()
                        || directive_by_name(element, "for").is_some()
                        || directive_by_name(element, "else").is_some()
                        || directive_by_name(element, "else-if").is_some())
        )
    }) {
        ", 1024 /* DYNAMIC_SLOTS */".into()
    } else {
        String::new()
    }
}

fn visible_children<'a>(
    ast: &'a Vue3Ast,
    children: &[vuec_ast::NodeId],
) -> Vec<&'a vuec_ast::Node<Vue3NodeKind>> {
    children
        .iter()
        .filter_map(|child_id| ast.node(*child_id))
        .filter(|child| match &child.kind {
            Vue3AstKind::Comment(_) => false,
            Vue3AstKind::Text(text) => !text.value.trim().is_empty(),
            _ => true,
        })
        .collect()
}

fn collect_component_tags(ast: &Vue3Ast) -> Vec<String> {
    let mut tags = Vec::new();
    for node in &ast.nodes {
        if let Vue3AstKind::Element(element) = &node.kind {
            if element.tag_type == Vue3ElementType::Component
                && !tags.iter().any(|tag| tag == &element.tag)
            {
                tags.push(element.tag.clone());
            }
        }
    }
    tags
}

fn component_asset_id(tag: &str) -> String {
    format!("_component_{}", to_valid_asset_part(tag))
}

fn to_valid_asset_part(value: &str) -> String {
    let mut output = String::new();
    for (index, ch) in value.chars().enumerate() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            output.push(ch);
        } else if ch == '-' {
            output.push('_');
        } else {
            output.push_str(&(ch as u32).to_string());
        }
        if index == 0 && ch.is_ascii_digit() {
            output.insert(0, '_');
        }
    }
    output
}

fn render_slot_outlet(
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    let name = element
        .props
        .iter()
        .find_map(|prop| match prop {
            Vue3Prop::Attribute(attr) if attr.name == "name" => attr.value.as_deref(),
            _ => None,
        })
        .map(quote_string)
        .or_else(|| {
            directive_by_name(element, "bind").and_then(|dir| {
                let arg = dir.arg.as_ref()?.source_string();
                (arg == "name").then(|| {
                    rewrite_expression_with_scope(
                        &dir.exp
                            .as_ref()
                            .map(Vue3Expression::source_string)
                            .unwrap_or_default(),
                        options,
                        scope,
                    )
                })
            })
        })
        .unwrap_or_else(|| quote_string("default"));
    let slots = if options.prefix_identifiers || options.mode == "module" {
        "_ctx.$slots"
    } else {
        "$slots"
    };
    format!("_renderSlot({}, {})", slots, name)
}

fn render_element_children(
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
    options: &Vue3CompilerOptions,
    parent_mode: NodeRenderMode,
    scope: &RenderScope,
) -> String {
    let child_nodes = children
        .iter()
        .filter_map(|child_id| ast.node(*child_id))
        .filter(|child| !matches!(child.kind, Vue3AstKind::Comment(_)))
        .collect::<Vec<_>>();
    if options.hoist_static
        && parent_mode == NodeRenderMode::Root
        && should_cache_children(&child_nodes)
    {
        let rendered = child_nodes
            .iter()
            .map(|child| {
                render_node_expr_scoped(
                    ast,
                    child.id,
                    options,
                    NodeRenderMode::Cached,
                    scope,
                    &mut MemoIndex::default(),
                )
            })
            .collect::<Vec<_>>();
        if !rendered.is_empty() {
            return format!(
                "[...(_cache[0] || (_cache[0] = [{}]))]",
                rendered.join(", ")
            );
        }
    }
    if child_nodes.iter().all(|child| {
        matches!(
            child.kind,
            Vue3AstKind::Text(_) | Vue3AstKind::Interpolation(_)
        )
    }) {
        return render_text_sequence_expr(ast, children, options, scope);
    }
    let rendered = render_child_sequence(
        ast,
        children,
        options,
        NodeRenderMode::Child,
        scope,
        &mut MemoIndex::default(),
    );
    if rendered.is_empty() {
        String::new()
    } else if rendered.len() == 1
        && child_nodes.first().is_some_and(|child| is_text_like(child))
        && parent_mode != NodeRenderMode::Root
    {
        rendered.into_iter().next().unwrap()
    } else {
        render_array(&rendered)
    }
}

fn render_child_sequence(
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
    options: &Vue3CompilerOptions,
    mode: NodeRenderMode,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> Vec<String> {
    let mut rendered = Vec::new();
    let mut index = 0usize;
    while index < children.len() {
        let child_id = children[index];
        let Some(child) = ast.node(child_id) else {
            index += 1;
            continue;
        };
        if matches!(child.kind, Vue3AstKind::Comment(_)) {
            index += 1;
            continue;
        }
        if is_text_like(child) {
            let start = index;
            index += 1;
            while index < children.len()
                && ast
                    .node(children[index])
                    .is_some_and(|candidate| is_text_like(candidate))
            {
                index += 1;
            }
            rendered.push(render_text_vnode(
                ast,
                &children[start..index],
                options,
                scope,
            ));
            continue;
        }
        if let Vue3AstKind::Element(element) = &child.kind {
            if directive_by_name(element, "if").is_some() {
                let mut branch_ids = vec![child_id];
                index += 1;
                while index < children.len() {
                    let Some(candidate) = ast.node(children[index]) else {
                        index += 1;
                        continue;
                    };
                    if matches!(candidate.kind, Vue3AstKind::Comment(_)) {
                        index += 1;
                        continue;
                    }
                    if let Vue3AstKind::Element(candidate_element) = &candidate.kind {
                        if is_else_branch(candidate_element) {
                            branch_ids.push(children[index]);
                            index += 1;
                            continue;
                        }
                    }
                    break;
                }
                rendered.push(render_if_chain(
                    ast,
                    &branch_ids,
                    options,
                    mode,
                    scope,
                    memo_index,
                ));
                continue;
            }
            if is_else_branch(element) {
                index += 1;
                continue;
            }
        }
        rendered.push(render_node_expr_scoped(
            ast, child_id, options, mode, scope, memo_index,
        ));
        index += 1;
    }
    rendered
}

fn is_text_like(node: &vuec_ast::Node<Vue3NodeKind>) -> bool {
    matches!(
        node.kind,
        Vue3AstKind::Text(_) | Vue3AstKind::Interpolation(_)
    )
}

fn render_text_sequence_expr(
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    children
        .iter()
        .filter_map(|child_id| ast.node(*child_id))
        .filter_map(|child| match &child.kind {
            Vue3AstKind::Text(text) => Some(quote_text(&text.value)),
            Vue3AstKind::Interpolation(interpolation) => Some(format!(
                "_toDisplayString({})",
                rewrite_expression_with_scope(
                    &interpolation.expression.source_string(),
                    options,
                    scope
                )
            )),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(" + ")
}

fn render_text_vnode(
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    let expression = render_text_sequence_expr(ast, children, options, scope);
    let has_interpolation = children.iter().any(|child_id| {
        ast.node(*child_id)
            .is_some_and(|child| matches!(child.kind, Vue3AstKind::Interpolation(_)))
    });
    if has_interpolation {
        format!("_createTextVNode({}, 1 /* TEXT */)", expression)
    } else {
        format!("_createTextVNode({})", expression)
    }
}

fn render_if_chain(
    ast: &Vue3Ast,
    branch_ids: &[vuec_ast::NodeId],
    options: &Vue3CompilerOptions,
    mode: NodeRenderMode,
    scope: &RenderScope,
    memo_index: &mut MemoIndex,
) -> String {
    fn render_branch(
        ast: &Vue3Ast,
        branch_ids: &[vuec_ast::NodeId],
        index: usize,
        options: &Vue3CompilerOptions,
        mode: NodeRenderMode,
        scope: &RenderScope,
        memo_index: &mut MemoIndex,
    ) -> String {
        let Some(branch_id) = branch_ids.get(index).copied() else {
            return "_createCommentVNode(\"v-if\", true)".into();
        };
        let Some(node) = ast.node(branch_id) else {
            return "_createCommentVNode(\"v-if\", true)".into();
        };
        let Some(element) = (match &node.kind {
            Vue3AstKind::Element(element) => Some(element),
            _ => None,
        }) else {
            return render_node_expr_scoped(ast, branch_id, options, mode, scope, memo_index);
        };
        let branch_expr = render_if_branch_expr(
            ast, branch_id, element, options, mode, scope, index, memo_index,
        );
        let condition = if index == 0 {
            directive_by_name(element, "if")
        } else {
            directive_by_name(element, "else-if")
        };
        if let Some(condition) = condition.and_then(|dir| dir.exp.as_ref()) {
            let condition = render_condition(
                &rewrite_expression_with_scope(&condition.source_string(), options, scope),
                options,
            );
            let alternate =
                render_branch(ast, branch_ids, index + 1, options, mode, scope, memo_index);
            format!(
                "{condition}\n  ? {}\n  : {}",
                indent_after_first_line(&branch_expr, 4),
                indent_after_first_line(&alternate, 4)
            )
        } else {
            branch_expr
        }
    }
    render_branch(ast, branch_ids, 0, options, mode, scope, memo_index)
}

fn render_if_branch_expr(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
    _mode: NodeRenderMode,
    scope: &RenderScope,
    branch_key: usize,
    memo_index: &mut MemoIndex,
) -> String {
    if element.tag == "template" {
        let children = ast
            .node(node_id)
            .map(|node| render_fragment_children(ast, &node.children, options, scope))
            .unwrap_or_else(|| "[]".into());
        return format!(
            "(_openBlock(), _createElementBlock(_Fragment, {{ key: {branch_key} }}, {children}, 64 /* STABLE_FRAGMENT */))"
        );
    }
    render_maybe_memo_element(
        ast,
        node_id,
        element,
        options,
        NodeRenderMode::Root,
        scope,
        Some(branch_key),
        memo_index,
    )
}

fn render_fragment_children(
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    let rendered = render_child_sequence(
        ast,
        children,
        options,
        NodeRenderMode::Child,
        scope,
        &mut MemoIndex::default(),
    );
    render_array(&rendered)
}

fn render_for_node(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    element: &Vue3Element,
    directive: &Vue3Directive,
    options: &Vue3CompilerOptions,
    _mode: NodeRenderMode,
    scope: &RenderScope,
    _memo_index: &mut MemoIndex,
) -> String {
    let Some(expression) = directive.exp.as_ref().map(Vue3Expression::source_string) else {
        return render_plain_element(
            ast,
            node_id,
            element,
            options,
            NodeRenderMode::Root,
            scope,
            None,
        );
    };
    let parsed = parse_v_for_expression(&expression);
    let Some((source, aliases)) = parsed else {
        return render_plain_element(
            ast,
            node_id,
            element,
            options,
            NodeRenderMode::Root,
            scope,
            None,
        );
    };
    let source = rewrite_expression_with_scope(&source, options, scope);
    let scoped = scope.with_locals(normalize_v_for_aliases(&aliases));
    let params = aliases.join(", ");
    let body = render_v_for_body(ast, node_id, element, options, &scoped);
    let Some(memo) = directive_by_name(element, "memo") else {
        let fragment_flag = v_for_fragment_patch_flag(element);
        let body = indent_after_first_line(&body, 2);
        return format!(
            "(_openBlock(true), _createElementBlock(_Fragment, null, _renderList({source}, ({params}) => {{\n  return {body}\n}}), {fragment_flag}))"
        );
    };
    let params = format!("{params}, __, ___, _cached");
    let memo_expression = memo
        .exp
        .as_ref()
        .map(Vue3Expression::source_string)
        .unwrap_or_default();
    let memo_expression = rewrite_expression_with_scope(&memo_expression, options, &scoped);
    let key = v_for_key_expression(element, options, &scoped);
    let guard = key.map_or_else(
        || format!("_cached && _cached.el && _isMemoSame(_cached, _memo)"),
        |key| {
            format!("_cached && _cached.el && _cached.key === {key} && _isMemoSame(_cached, _memo)")
        },
    );
    let body = indent_after_first_line(&body, 2);
    format!(
        "(_openBlock(true), _createElementBlock(_Fragment, null, _renderList({source}, ({params}) => {{\n  const _memo = ({memo_expression})\n  if ({guard}) return _cached\n  const _item = {body}\n  _item.memo = _memo\n  return _item\n}}, _cache, 0), 128 /* KEYED_FRAGMENT */))"
    )
}

fn render_v_for_body(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    if element.tag == "template" {
        let Some(node) = ast.node(node_id) else {
            return "null".into();
        };
        let visible = visible_children(ast, &node.children);
        if visible.len() == 1 {
            if let Some(child) = visible.first() {
                if let Vue3AstKind::Element(child_element) = &child.kind {
                    let key = v_for_key_expression(element, options, scope);
                    let body = render_plain_element(
                        ast,
                        child.id,
                        child_element,
                        options,
                        NodeRenderMode::Root,
                        scope,
                        None,
                    );
                    return inject_key_into_vnode_call(&body, key.as_deref());
                }
            }
        }
        let key = v_for_key_expression(element, options, scope);
        let children = render_fragment_children(ast, &node.children, options, scope);
        let props = key
            .map(|key| format!("{{ key: {key} }}"))
            .unwrap_or_else(|| "null".into());
        return format!(
            "(_openBlock(), _createElementBlock(_Fragment, {props}, {children}, 64 /* STABLE_FRAGMENT */))"
        );
    }
    render_plain_element(
        ast,
        node_id,
        element,
        options,
        NodeRenderMode::Root,
        scope,
        None,
    )
}

fn v_for_fragment_patch_flag(element: &Vue3Element) -> &'static str {
    if v_for_key_expression(
        element,
        &Vue3CompilerOptions::default(),
        &RenderScope::default(),
    )
    .is_some()
    {
        "128 /* KEYED_FRAGMENT */"
    } else {
        "256 /* UNKEYED_FRAGMENT */"
    }
}

fn v_for_key_expression(
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> Option<String> {
    element.props.iter().find_map(|prop| match prop {
        Vue3Prop::Directive(dir)
            if dir.name == "bind"
                && dir
                    .arg
                    .as_ref()
                    .is_some_and(|arg| arg.source_string() == "key") =>
        {
            dir.exp
                .as_ref()
                .map(Vue3Expression::source_string)
                .filter(|value| !value.trim().is_empty())
                .map(|value| rewrite_expression_with_scope(&value, options, scope))
        }
        Vue3Prop::Attribute(attr) if attr.name == "key" => {
            attr.value.as_ref().map(|value| quote_string(value))
        }
        _ => None,
    })
}

fn inject_key_into_vnode_call(body: &str, key: Option<&str>) -> String {
    let Some(key) = key else {
        return body.to_string();
    };
    if body.contains(" key: ") || body.contains("{ key:") {
        return body.to_string();
    }
    let Some(start) = body.find("_createElementBlock(") else {
        return body.to_string();
    };
    let args_start = start + "_createElementBlock(".len();
    let Some(first_comma) = find_top_level_comma(body, args_start) else {
        return body.to_string();
    };
    let Some(close) = find_matching_call_close(body, args_start) else {
        return body.to_string();
    };
    if body[first_comma + 1..close].trim().is_empty() {
        let mut output = body.to_string();
        output.insert_str(first_comma, &format!(", {{ key: {key} }}"));
        return output;
    }
    let second_arg_start = first_comma + 1;
    let second_arg_end = find_top_level_comma(body, second_arg_start).unwrap_or(close);
    let second_arg = body[second_arg_start..second_arg_end].trim();
    if second_arg == "null" {
        let mut output = body.to_string();
        output.replace_range(
            second_arg_start..second_arg_end,
            &format!(" {{ key: {key} }}"),
        );
        return output;
    }
    body.to_string()
}

fn find_top_level_comma(value: &str, start: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut quote = None;
    let chars = value.char_indices().skip_while(|(index, _)| *index < start);
    for (index, ch) in chars {
        if let Some(active_quote) = quote {
            if ch == active_quote {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' | '`' => quote = Some(ch),
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => {
                if depth == 0 {
                    return None;
                }
                depth -= 1;
            }
            ',' if depth == 0 => return Some(index),
            _ => {}
        }
    }
    None
}

fn find_matching_call_close(value: &str, start: usize) -> Option<usize> {
    let mut depth = 1usize;
    let mut quote = None;
    let chars = value.char_indices().skip_while(|(index, _)| *index < start);
    for (index, ch) in chars {
        if let Some(active_quote) = quote {
            if ch == active_quote {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' | '`' => quote = Some(ch),
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

fn render_array(items: &[String]) -> String {
    if items.is_empty() {
        "[]".into()
    } else {
        format!(
            "[\n{}\n]",
            items
                .iter()
                .map(|item| indent_lines(item, 2))
                .collect::<Vec<_>>()
                .join(",\n")
        )
    }
}

fn indent_lines(value: &str, spaces: usize) -> String {
    let prefix = " ".repeat(spaces);
    value
        .lines()
        .map(|line| format!("{prefix}{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn indent_after_first_line(value: &str, spaces: usize) -> String {
    let prefix = " ".repeat(spaces);
    let mut lines = value.lines();
    let Some(first) = lines.next() else {
        return String::new();
    };
    let mut output = first.to_string();
    for line in lines {
        output.push('\n');
        output.push_str(&prefix);
        output.push_str(line);
    }
    output
}

fn render_condition(condition: &str, options: &Vue3CompilerOptions) -> String {
    if uses_prefixed_identifiers(options) {
        format!("({condition})")
    } else {
        condition.to_string()
    }
}

fn should_cache_children(children: &[&vuec_ast::Node<Vue3NodeKind>]) -> bool {
    !children.is_empty()
        && children
            .iter()
            .all(|child| is_static_element_for_cache(child))
}

fn is_static_element_for_cache(node: &vuec_ast::Node<Vue3NodeKind>) -> bool {
    matches!(
        &node.kind,
        Vue3AstKind::Element(element) if element.tag != "slot"
            && element.props.iter().all(|prop| {
                matches!(prop, Vue3Prop::Attribute(_))
            })
    )
}

fn has_dynamic_text_child(ast: &Vue3Ast, children: &[vuec_ast::NodeId]) -> bool {
    children.iter().any(|child_id| {
        ast.node(*child_id)
            .is_some_and(|child| matches!(child.kind, Vue3AstKind::Interpolation(_)))
    })
}

fn child_sequence_needs_text_vnode(ast: &Vue3Ast, children: &[vuec_ast::NodeId]) -> bool {
    let visible = children
        .iter()
        .filter_map(|child_id| ast.node(*child_id))
        .filter(|child| !matches!(child.kind, Vue3AstKind::Comment(_)))
        .collect::<Vec<_>>();
    visible.len() > 1 && !visible.iter().all(|child| is_text_like(child))
}

fn children_literal_const_only(
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
    options: &Vue3CompilerOptions,
) -> bool {
    let mut has_interpolation = false;
    for child_id in children {
        let Some(child) = ast.node(*child_id) else {
            continue;
        };
        match &child.kind {
            Vue3AstKind::Interpolation(interpolation) => {
                has_interpolation = true;
                let expression = interpolation.expression.source_string();
                if options
                    .binding_metadata
                    .get(expression.trim())
                    .map(String::as_str)
                    != Some("literal-const")
                {
                    return false;
                }
            }
            Vue3AstKind::Text(text) if text.value.trim().is_empty() => {}
            Vue3AstKind::Comment(_) => {}
            _ => return false,
        }
    }
    has_interpolation
}

fn render_patch_flag_kind(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
    mode: NodeRenderMode,
) -> Option<i32> {
    let children = ast
        .node(node_id)
        .map(|node| node.children.as_slice())
        .unwrap_or(&[]);
    if mode == NodeRenderMode::Cached {
        Some(-1)
    } else if has_class_binding(element) {
        Some(2)
    } else if has_dynamic_non_key_props(element) {
        Some(8)
    } else if element.tag != "template"
        && !children_literal_const_only(ast, children, options)
        && has_dynamic_text_child(ast, children)
    {
        Some(1)
    } else if has_vnode_hook(element) {
        Some(512)
    } else {
        None
    }
}

fn render_patch_flag_text(flag: Option<i32>) -> String {
    match flag {
        Some(-1) => ", -1 /* CACHED */".into(),
        Some(1) => ", 1 /* TEXT */".into(),
        Some(2) => ", 2 /* CLASS */".into(),
        Some(8) => ", 8 /* PROPS */".into(),
        Some(512) => ", 512 /* NEED_PATCH */".into(),
        Some(flag) => format!(", {flag}"),
        None => String::new(),
    }
}

pub fn vue3_element_codegen_patch_flag(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    options: &Vue3CompilerOptions,
    is_root: bool,
) -> Option<i32> {
    let Some(node) = ast.node(node_id) else {
        return None;
    };
    let Vue3AstKind::Element(element) = &node.kind else {
        return None;
    };
    let mode = if is_root {
        NodeRenderMode::Root
    } else {
        NodeRenderMode::Child
    };
    render_patch_flag_kind(ast, node_id, element, options, mode)
}

fn render_props(
    element: &Vue3Element,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
    branch_key: Option<usize>,
) -> String {
    let dynamic_event = has_dynamic_non_key_props(element);
    let mut props = Vec::new();
    if let Some(key) = branch_key {
        props.push(format!("key: {key}"));
    }
    props.extend(element.props.iter().filter_map(|prop| match prop {
        Vue3Prop::Attribute(attr) => match &attr.value {
            Some(value) => Some(format!("{}: {}", json_key(&attr.name), quote_string(value))),
            None => Some(format!("{}: true", json_key(&attr.name))),
        },
        Vue3Prop::Directive(dir) if dir.name == "on" => {
            let event = dir
                .arg
                .as_ref()
                .map(Vue3Expression::source_string)
                .unwrap_or_default();
            let value = dir
                .exp
                .as_ref()
                .map(Vue3Expression::source_string)
                .unwrap_or_default();
            Some(format!(
                "{}: {}",
                json_key(&event_handler_prop_name(element, &event)),
                rewrite_handler_expression_with_scope(&value, options, scope)
            ))
        }
        Vue3Prop::Directive(dir) if dir.name == "bind" => {
            let arg = dir
                .arg
                .as_ref()
                .map(Vue3Expression::source_string)
                .unwrap_or_default();
            let value = dir
                .exp
                .as_ref()
                .map(Vue3Expression::source_string)
                .unwrap_or_default();
            if arg.is_empty() || value.is_empty() {
                return None;
            }
            let expression = rewrite_expression_with_scope(&value, options, scope);
            if arg == "class" {
                Some(format!("class: _normalizeClass({expression})"))
            } else if dir.is_dynamic_arg {
                Some(format!(
                    "[{}]: {}",
                    rewrite_expression_with_scope(&arg, options, scope),
                    expression
                ))
            } else {
                Some(format!("{}: {}", json_key(&arg), expression))
            }
        }
        _ => None,
    }));
    if props.is_empty() {
        String::new()
    } else if dynamic_event {
        format!("{{\n  {}\n}}", props.join(",\n  "))
    } else if props.len() > 1 || has_class_binding(element) {
        render_object(&props)
    } else if props.len() == 1
        && props
            .first()
            .is_some_and(|prop| prop.starts_with("key: ") && prop.contains('('))
    {
        render_object(&props)
    } else {
        format!("{{ {} }}", props.join(", "))
    }
}

fn render_object(properties: &[String]) -> String {
    if properties.is_empty() {
        "{}".into()
    } else {
        format!(
            "{{\n{}\n}}",
            properties
                .iter()
                .map(|property| indent_lines(property, 2))
                .collect::<Vec<_>>()
                .join(",\n")
        )
    }
}

fn has_class_binding(element: &Vue3Element) -> bool {
    element.props.iter().any(|prop| {
        matches!(
            prop,
            Vue3Prop::Directive(dir)
                if dir.name == "bind"
                    && dir
                        .arg
                        .as_ref()
                        .is_some_and(|arg| arg.source_string() == "class")
        )
    })
}

fn has_dynamic_non_key_props(element: &Vue3Element) -> bool {
    element.props.iter().any(|prop| {
        matches!(
            prop,
            Vue3Prop::Directive(dir)
                if (dir.name == "on" && !event_directive_is_vnode_hook(dir))
                    || (dir.name == "bind"
                        && dir
                            .arg
                            .as_ref()
                            .is_none_or(|arg| arg.source_string() != "class" && arg.source_string() != "key"))
        )
    })
}

fn has_vnode_hook(element: &Vue3Element) -> bool {
    element.props.iter().any(|prop| {
        matches!(
            prop,
            Vue3Prop::Directive(dir) if dir.name == "on" && event_directive_is_vnode_hook(dir)
        )
    })
}

fn dynamic_props_arg(element: &Vue3Element) -> String {
    let props = element
        .props
        .iter()
        .filter_map(|prop| match prop {
            Vue3Prop::Directive(dir) if dir.name == "on" && !event_directive_is_vnode_hook(dir) => {
                let event = dir
                    .arg
                    .as_ref()
                    .map(Vue3Expression::source_string)
                    .unwrap_or_default();
                Some(event_handler_prop_name(element, &event))
            }
            Vue3Prop::Directive(dir)
                if dir.name == "bind" && !has_class_bind_dir(dir) && !has_key_bind_dir(dir) =>
            {
                dir.arg.as_ref().map(Vue3Expression::source_string)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    if props.is_empty() {
        String::new()
    } else {
        format!(
            ", [{}]",
            props
                .iter()
                .map(|prop| quote_string(prop))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

fn event_directive_is_vnode_hook(dir: &Vue3Directive) -> bool {
    dir.arg
        .as_ref()
        .is_some_and(|arg| arg.source_string().starts_with("vue:"))
}

fn event_handler_prop_name(element: &Vue3Element, event: &str) -> String {
    let raw_name = if let Some(hook) = event.strip_prefix("vue:") {
        format!("vnode-{hook}")
    } else {
        event.to_string()
    };
    if element.tag_type != Vue3ElementType::Element
        || raw_name.starts_with("vnode")
        || !raw_name.chars().any(|ch| ch.is_ascii_uppercase())
    {
        format!("on{}", capitalize(&camelize(&raw_name)))
    } else {
        format!("on:{raw_name}")
    }
}

fn has_class_bind_dir(dir: &Vue3Directive) -> bool {
    dir.arg
        .as_ref()
        .is_some_and(|arg| arg.source_string() == "class")
}

fn has_key_bind_dir(dir: &Vue3Directive) -> bool {
    dir.arg
        .as_ref()
        .is_some_and(|arg| arg.source_string() == "key")
}

fn directive_by_name<'a>(element: &'a Vue3Element, name: &str) -> Option<&'a Vue3Directive> {
    element.props.iter().find_map(|prop| match prop {
        Vue3Prop::Directive(dir) if dir.name == name => Some(dir),
        _ => None,
    })
}

fn is_else_branch(element: &Vue3Element) -> bool {
    directive_by_name(element, "else").is_some() || directive_by_name(element, "else-if").is_some()
}

fn parse_v_for_expression(expression: &str) -> Option<(String, Vec<String>)> {
    let expression = expression.trim();
    let (raw_aliases, source) = expression
        .split_once(" in ")
        .or_else(|| expression.split_once(" of "))?;
    let raw_aliases = raw_aliases
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')');
    let aliases = split_top_level_like(raw_aliases, ',')
        .into_iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    if aliases.is_empty() {
        None
    } else {
        Some((source.trim().to_string(), aliases))
    }
}

fn normalize_v_for_aliases(aliases: &[String]) -> Vec<String> {
    aliases
        .iter()
        .flat_map(|alias| extract_v_for_alias_locals(alias))
        .collect()
}

fn extract_v_for_alias_locals(alias: &str) -> Vec<String> {
    let alias = alias.trim();
    if alias.starts_with('{') || alias.starts_with('[') {
        return extract_destructure_alias_locals(alias);
    }
    if alias
        .chars()
        .next()
        .is_some_and(|ch| is_identifier_start(ch))
    {
        vec![alias.to_string()]
    } else {
        Vec::new()
    }
}

fn extract_destructure_alias_locals(alias: &str) -> Vec<String> {
    let trimmed = alias
        .trim()
        .trim_start_matches('{')
        .trim_start_matches('[')
        .trim_end_matches('}')
        .trim_end_matches(']');
    split_top_level_like(trimmed, ',')
        .into_iter()
        .flat_map(|part| extract_slot_params(part))
        .collect()
}

fn split_top_level_like(source: &str, separator: char) -> Vec<&str> {
    let mut items = Vec::new();
    let mut depth = 0usize;
    let mut quote = None;
    let mut start = 0usize;
    for (index, ch) in source.char_indices() {
        if let Some(active_quote) = quote {
            if ch == active_quote {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' | '`' => quote = Some(ch),
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            _ if ch == separator && depth == 0 => {
                let item = source[start..index].trim();
                if !item.is_empty() {
                    items.push(item);
                }
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }
    let item = source[start..].trim();
    if !item.is_empty() {
        items.push(item);
    }
    items
}

fn capitalize(value: &str) -> String {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    format!(
        "{}{}",
        first.to_ascii_uppercase(),
        chars.collect::<String>()
    )
}

fn rewrite_handler_expression_with_scope(
    expression: &str,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    normalize_handler_indent(&rewrite_expression_with_scope(expression, options, scope))
}

fn rewrite_expression_with_scope(
    expression: &str,
    options: &Vue3CompilerOptions,
    scope: &RenderScope,
) -> String {
    let expression = expression.trim();
    if !uses_prefixed_identifiers(options) {
        return expression.to_string();
    }
    if scope.locals.is_empty() {
        rewrite_js_like_expression(expression, options)
    } else {
        rewrite_js_like_expression_with_locals(expression, options, &scope.locals)
    }
}

fn uses_prefixed_identifiers(options: &Vue3CompilerOptions) -> bool {
    options.prefix_identifiers || options.mode == "module"
}

fn normalize_handler_indent(expression: &str) -> String {
    let mut lines = expression.lines();
    let Some(first) = lines.next() else {
        return String::new();
    };
    let mut normalized = String::from(first);
    for line in lines {
        normalized.push('\n');
        normalized.push_str(line.strip_prefix("  ").unwrap_or(line));
    }
    normalized
}

fn rewrite_js_like_expression(expression: &str, options: &Vue3CompilerOptions) -> String {
    let mut output = String::new();
    rewrite_js_like_expression_into(expression, options, Vec::new(), &mut output);
    output
}

fn rewrite_js_like_expression_with_locals(
    expression: &str,
    options: &Vue3CompilerOptions,
    locals: &[String],
) -> String {
    let mut output = String::new();
    rewrite_js_like_expression_into(expression, options, locals.to_vec(), &mut output);
    output
}

fn rewrite_js_like_expression_into(
    expression: &str,
    options: &Vue3CompilerOptions,
    root_locals: Vec<String>,
    output: &mut String,
) {
    let mut scopes = vec![Scope {
        locals: root_locals,
    }];
    let mut previous = TokenKind::Other;
    let mut pending_decl: Option<DeclKind> = None;
    let mut pending_function_params = false;
    let mut last_keyword: Option<String> = None;
    let mut paren_depth = 0usize;
    let mut for_pending = false;
    let mut for_header_depth: Option<usize> = None;
    let mut pending_for_block_locals = Vec::<String>::new();
    let mut catch_pending = false;
    let mut catch_param_depth: Option<usize> = None;
    let mut pending_catch_locals = Vec::<String>::new();
    let chars = expression.char_indices().collect::<Vec<_>>();
    let mut index = 0usize;
    while index < chars.len() {
        let byte = chars[index].0;
        let ch = chars[index].1;
        if ch == '\'' || ch == '"' || ch == '`' {
            let quote = ch;
            output.push(ch);
            index += 1;
            while index < chars.len() {
                let current = chars[index].1;
                output.push(current);
                index += 1;
                if current == '\\' && index < chars.len() {
                    output.push(chars[index].1);
                    index += 1;
                    continue;
                }
                if current == quote {
                    break;
                }
            }
            previous = TokenKind::Other;
            continue;
        }
        if is_identifier_start(ch) {
            let start = byte;
            index += 1;
            while index < chars.len() && is_identifier_continue(chars[index].1) {
                index += 1;
            }
            let end = chars
                .get(index)
                .map_or(expression.len(), |(offset, _)| *offset);
            let ident = &expression[start..end];
            let next = next_non_ws(expression, end);
            let prev = previous;
            if is_keyword(ident) {
                output.push_str(ident);
                match ident {
                    "var" => pending_decl = Some(DeclKind::Var),
                    "let" | "const" => pending_decl = Some(DeclKind::Block),
                    "function" => pending_function_params = true,
                    "for" => for_pending = true,
                    "in" | "of" => pending_decl = None,
                    "catch" => catch_pending = true,
                    _ => {}
                }
                last_keyword = Some(ident.to_string());
                previous = TokenKind::Keyword;
                continue;
            }
            if catch_param_depth.is_some() {
                if next != Some(':') {
                    pending_catch_locals.push(ident.to_string());
                }
                output.push_str(ident);
                previous = TokenKind::Identifier;
                continue;
            }
            if pending_decl.is_some()
                && matches!(
                    prev,
                    TokenKind::Keyword | TokenKind::Comma | TokenKind::OpenParen
                )
            {
                if pending_decl == Some(DeclKind::Var) {
                    if let Some(scope) = scopes.first_mut() {
                        scope.locals.push(ident.to_string());
                    }
                } else if for_header_depth.is_some() {
                    pending_for_block_locals.push(ident.to_string());
                } else if let Some(scope) = scopes.last_mut() {
                    scope.locals.push(ident.to_string());
                }
                output.push_str(ident);
                previous = TokenKind::Identifier;
                continue;
            }
            let skip_property = matches!(prev, TokenKind::Dot)
                || (next == Some(':') && last_keyword.as_deref() != Some("case"))
                || (pending_function_params
                    && matches!(prev, TokenKind::OpenParen | TokenKind::Comma));
            if skip_property
                || is_global_or_literal(ident)
                || is_local(&scopes, ident)
                || pending_for_block_locals.iter().any(|local| local == ident)
            {
                output.push_str(ident);
            } else {
                output.push_str(&rewrite_identifier(ident, options));
            }
            previous = TokenKind::Identifier;
            continue;
        }
        output.push(ch);
        match ch {
            '{' => {
                if !pending_for_block_locals.is_empty() {
                    scopes.push(Scope {
                        locals: std::mem::take(&mut pending_for_block_locals),
                    });
                } else if !pending_catch_locals.is_empty() {
                    scopes.push(Scope {
                        locals: std::mem::take(&mut pending_catch_locals),
                    });
                } else {
                    scopes.push(Scope::default());
                }
                previous = TokenKind::Other;
            }
            '}' => {
                if scopes.len() > 1 {
                    scopes.pop();
                }
                previous = TokenKind::Other;
            }
            '(' => {
                paren_depth += 1;
                if for_pending {
                    for_header_depth = Some(paren_depth);
                    for_pending = false;
                }
                if catch_pending {
                    catch_param_depth = Some(paren_depth);
                    catch_pending = false;
                }
                previous = TokenKind::OpenParen;
            }
            ')' => {
                if catch_param_depth == Some(paren_depth) {
                    catch_param_depth = None;
                }
                if for_header_depth == Some(paren_depth) {
                    for_header_depth = None;
                }
                paren_depth = paren_depth.saturating_sub(1);
                pending_function_params = false;
                previous = TokenKind::Other;
            }
            ',' => previous = TokenKind::Comma,
            '.' => previous = TokenKind::Dot,
            ';' => {
                pending_decl = None;
                previous = TokenKind::Other;
            }
            _ if ch.is_whitespace() => {}
            _ => {
                if ch != ':' {
                    last_keyword = None;
                }
                previous = TokenKind::Other;
            }
        }
        index += 1;
    }
}

#[derive(Clone, Debug, Default)]
struct Scope {
    locals: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DeclKind {
    Var,
    Block,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TokenKind {
    Identifier,
    Keyword,
    OpenParen,
    Comma,
    Dot,
    Other,
}

fn is_local(scopes: &[Scope], ident: &str) -> bool {
    scopes
        .iter()
        .rev()
        .any(|scope| scope.locals.iter().any(|local| local == ident))
}

fn next_non_ws(source: &str, offset: usize) -> Option<char> {
    source.get(offset..)?.chars().find(|ch| !ch.is_whitespace())
}

fn is_identifier_start(ch: char) -> bool {
    ch == '_' || ch == '$' || ch.is_ascii_alphabetic()
}

fn is_identifier_continue(ch: char) -> bool {
    is_identifier_start(ch) || ch.is_ascii_digit()
}

fn is_keyword(value: &str) -> bool {
    matches!(
        value,
        "const"
            | "let"
            | "var"
            | "function"
            | "return"
            | "if"
            | "else"
            | "for"
            | "in"
            | "of"
            | "try"
            | "catch"
            | "throw"
            | "new"
            | "switch"
            | "case"
            | "break"
            | "continue"
            | "async"
            | "await"
    )
}

fn is_global_or_literal(value: &str) -> bool {
    matches!(
        value,
        "true"
            | "false"
            | "null"
            | "undefined"
            | "this"
            | "Infinity"
            | "NaN"
            | "Math"
            | "Number"
            | "Date"
            | "Array"
            | "Object"
            | "Boolean"
            | "String"
            | "RegExp"
            | "Map"
            | "Set"
            | "WeakMap"
            | "WeakSet"
            | "JSON"
            | "Intl"
            | "BigInt"
            | "console"
            | "Error"
            | "TypeError"
            | "Symbol"
            | "Promise"
            | "Reflect"
            | "globalThis"
    )
}

fn rewrite_identifier(ident: &str, options: &Vue3CompilerOptions) -> String {
    match options.binding_metadata.get(ident).map(String::as_str) {
        Some("setup-ref") if options.inline => format!("{ident}.value"),
        Some("setup-maybe-ref") if options.inline => format!("_unref({ident})"),
        Some("setup-const" | "literal-const" | "setup-reactive-const") if options.inline => {
            ident.to_string()
        }
        Some("props") if options.inline => format!("__props.{ident}"),
        Some("props-aliased") if options.inline => {
            let source = options
                .props_aliases
                .get(ident)
                .map_or(ident, String::as_str);
            format!("__props[{}]", quote_string(source))
        }
        Some("props-aliased") => {
            let source = options
                .props_aliases
                .get(ident)
                .map_or(ident, String::as_str);
            format!("$props[{}]", quote_string(source))
        }
        Some("data" | "options") if options.inline => format!("_ctx.{ident}"),
        Some(kind) if kind.starts_with("setup") || kind == "literal-const" => {
            format!("$setup.{ident}")
        }
        Some(kind) => format!("${kind}.{ident}"),
        None => format!("_ctx.{ident}"),
    }
}

fn expression_diagnostics(ast: &Vue3Ast, options: &Vue3CompilerOptions) -> Vec<String> {
    let store = JsAstStore::new();
    let source_type = expression_source_type(options);
    ast.nodes
        .iter()
        .filter_map(|node| match &node.kind {
            Vue3AstKind::Interpolation(interpolation) => {
                Some(interpolation.expression.source_string())
            }
            _ => None,
        })
        .filter_map(|expression| {
            store
                .parse_expression(expression.trim(), source_type)
                .err()
                .map(|err| err.message().to_string())
        })
        .collect()
}

fn expression_source_type(options: &Vue3CompilerOptions) -> oxc_span::SourceType {
    if options.is_ts
        || options
            .expression_plugins
            .iter()
            .any(|plugin| plugin == "typescript")
    {
        oxc_span::SourceType::ts()
    } else {
        oxc_span::SourceType::mjs()
    }
}

fn quote_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".into())
}

fn json_key(key: &str) -> String {
    if key
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '$')
    {
        key.to_string()
    } else {
        quote_string(key)
    }
}

fn quote_text(value: &str) -> String {
    quote_string(value)
}

fn push_text_and_interpolations(
    ast: &mut Vue3Ast,
    parent: vuec_ast::NodeId,
    file_id: FileId,
    token_start: usize,
    text: &str,
    options: &Vue3CompilerOptions,
) {
    let (open_delimiter, close_delimiter) = options
        .delimiters
        .as_ref()
        .map_or(("{{", "}}"), |items| (items[0].as_str(), items[1].as_str()));
    if open_delimiter.is_empty() || close_delimiter.is_empty() {
        push_text(ast, parent, file_id, token_start, text);
        return;
    }
    let mut cursor = 0usize;
    while let Some(open) = text[cursor..].find(open_delimiter) {
        let open = cursor + open;
        let expression_start = open + open_delimiter.len();
        let Some(close_offset) = text[expression_start..].find(close_delimiter) else {
            push_text(ast, parent, file_id, token_start + cursor, &text[cursor..]);
            return;
        };
        if open > cursor {
            push_text(
                ast,
                parent,
                file_id,
                token_start + cursor,
                &text[cursor..open],
            );
        }
        let close = expression_start + close_offset;
        let expression = decode_html_text_entities(text[expression_start..close].trim());
        let _id = ast.push_child(
            parent,
            Vue3NodeKind::interpolation(expression),
            Some(Span::new(
                file_id,
                token_start + open,
                token_start + close + close_delimiter.len(),
            )),
        );
        cursor = close + close_delimiter.len();
    }
    if cursor < text.len() {
        push_text(ast, parent, file_id, token_start + cursor, &text[cursor..]);
    }
}

fn push_text(
    ast: &mut Vue3Ast,
    parent: vuec_ast::NodeId,
    file_id: FileId,
    start: usize,
    text: &str,
) {
    if text.is_empty() {
        return;
    }
    let decoded = decode_html_text_entities(text);
    let previous = ast
        .node(parent)
        .and_then(|node| node.children.last().copied());
    if let Some(previous) = previous {
        if let Some(node) = ast.node_mut(previous) {
            if let Vue3AstKind::Text(existing) = &mut node.kind {
                existing.value.push_str(&decoded);
                if let Some(span) = node.span.source_mut() {
                    span.end = vuec_source::BytePos(start + text.len());
                }
                return;
            }
        }
    }
    let _id = ast.push_child(
        parent,
        Vue3NodeKind::text(decoded),
        Some(Span::new(file_id, start, start + text.len())),
    );
}

fn push_raw_text(
    ast: &mut Vue3Ast,
    parent: vuec_ast::NodeId,
    file_id: FileId,
    start: usize,
    text: &str,
) {
    if text.is_empty() {
        return;
    }
    let previous = ast
        .node(parent)
        .and_then(|node| node.children.last().copied());
    if let Some(previous) = previous {
        if let Some(node) = ast.node_mut(previous) {
            if let Vue3AstKind::Text(existing) = &mut node.kind {
                existing.value.push_str(text);
                if let Some(span) = node.span.source_mut() {
                    span.end = vuec_source::BytePos(start + text.len());
                }
                return;
            }
        }
    }
    let _id = ast.push_child(
        parent,
        Vue3NodeKind::text(text),
        Some(Span::new(file_id, start, start + text.len())),
    );
}

fn decode_html_text_entities(text: &str) -> String {
    if !text.contains('&') {
        return text.to_string();
    }
    decode_html_entities(text, HtmlEntityDecodeMode::Text)
}

fn decode_html_attr_entities(text: &str) -> String {
    if !text.contains('&') {
        return text.to_string();
    }
    decode_html_entities(text, HtmlEntityDecodeMode::Attribute)
}

#[derive(Clone, Copy)]
enum HtmlEntityDecodeMode {
    Text,
    Attribute,
}

fn decode_html_entities(text: &str, mode: HtmlEntityDecodeMode) -> String {
    let mut output = String::with_capacity(text.len());
    let mut cursor = 0usize;
    while cursor < text.len() {
        let Some(offset) = text[cursor..].find('&') else {
            output.push_str(&text[cursor..]);
            break;
        };
        let amp = cursor + offset;
        output.push_str(&text[cursor..amp]);
        if let Some((decoded, consumed)) = decode_html_entity_at(&text[amp..], mode) {
            output.push(decoded);
            cursor = amp + consumed;
        } else {
            output.push('&');
            cursor = amp + 1;
        }
    }
    output
}

fn decode_html_entity_at(value: &str, mode: HtmlEntityDecodeMode) -> Option<(char, usize)> {
    if let Some(decoded) = decode_numeric_html_entity_at(value) {
        return Some(decoded);
    }
    const NAMED: [(&str, char); 7] = [
        ("amp", '&'),
        ("lt", '<'),
        ("gt", '>'),
        ("nbsp", '\u{00a0}'),
        ("apos", '\''),
        ("quot", '"'),
        ("Eacute", '\u{00c9}'),
    ];
    for (name, decoded) in NAMED {
        let prefix = format!("&{name}");
        if !value.starts_with(&prefix) {
            continue;
        }
        let after_name = prefix.len();
        if value.as_bytes().get(after_name) == Some(&b';') {
            return Some((decoded, after_name + 1));
        }
        if matches!(mode, HtmlEntityDecodeMode::Text) && matches!(name, "amp" | "lt" | "gt") {
            return Some((decoded, after_name));
        }
        if matches!(mode, HtmlEntityDecodeMode::Attribute)
            && name == "amp"
            && value
                .as_bytes()
                .get(after_name)
                .is_some_and(|byte| !byte.is_ascii_alphanumeric() && *byte != b'=')
        {
            return Some((decoded, after_name));
        }
    }
    None
}

fn decode_numeric_html_entity_at(value: &str) -> Option<(char, usize)> {
    let rest = value.strip_prefix("&#")?;
    let (radix, digits_start) = match rest.as_bytes().first().copied() {
        Some(b'x' | b'X') => (16, "&#x".len()),
        _ => (10, "&#".len()),
    };
    let mut digits_end = digits_start;
    while let Some(byte) = value.as_bytes().get(digits_end).copied() {
        let is_digit = if radix == 16 {
            byte.is_ascii_hexdigit()
        } else {
            byte.is_ascii_digit()
        };
        if !is_digit {
            break;
        }
        digits_end += 1;
    }
    if digits_end == digits_start {
        return None;
    }
    let raw = u32::from_str_radix(&value[digits_start..digits_end], radix).ok()?;
    let consumed = digits_end + usize::from(value.as_bytes().get(digits_end) == Some(&b';'));
    Some((html_numeric_entity_char(raw), consumed))
}

fn html_numeric_entity_char(value: u32) -> char {
    match value {
        0x00 => '\u{fffd}',
        0x80 => '\u{20ac}',
        0x82 => '\u{201a}',
        0x83 => '\u{0192}',
        0x84 => '\u{201e}',
        0x85 => '\u{2026}',
        0x86 => '\u{2020}',
        0x87 => '\u{2021}',
        0x88 => '\u{02c6}',
        0x89 => '\u{2030}',
        0x8a => '\u{0160}',
        0x8b => '\u{2039}',
        0x8c => '\u{0152}',
        0x8e => '\u{017d}',
        0x91 => '\u{2018}',
        0x92 => '\u{2019}',
        0x93 => '\u{201c}',
        0x94 => '\u{201d}',
        0x95 => '\u{2022}',
        0x96 => '\u{2013}',
        0x97 => '\u{2014}',
        0x98 => '\u{02dc}',
        0x99 => '\u{2122}',
        0x9a => '\u{0161}',
        0x9b => '\u{203a}',
        0x9c => '\u{0153}',
        0x9e => '\u{017e}',
        0x9f => '\u{0178}',
        value => char::from_u32(value).unwrap_or('\u{fffd}'),
    }
}

pub fn base_compile(source: TemplateSource, options: Vue3CompilerOptions) -> CodegenResult {
    Vue3Dialect::base_compile(source, options)
}

pub fn compile_dom(source: TemplateSource, options: Vue3CompilerOptions) -> CodegenResult {
    Vue3Dialect::compile_dom(source, options)
}

pub fn compile_ssr(source: TemplateSource, options: Vue3CompilerOptions) -> CodegenResult {
    Vue3Dialect::compile_ssr(source, options)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_transform_generate_roundtrip() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<div>hello</div>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let result = base_compile(source, Vue3CompilerOptions::default());
        assert!(result.code.contains("render"));
        assert!(result.ast_summary.contains("nodes="));
    }

    #[test]
    fn ssr_wraps_code() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<div/>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let result = compile_ssr(source, Vue3CompilerOptions::default());
        assert!(result.code.starts_with("/* ssr */"));
    }

    #[test]
    fn template_base_offset_maps_nodes_to_original_file_spans() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<div>{{ msg }}</div>".into(),
            file_id: FileId(7),
            base_offset: 42,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let root = ast.root_node().expect("root");
        assert_eq!(root.span.source(), Some(Span::new(FileId(7), 42, 62)));
        let element = ast.node(root.children[0]).expect("element");
        assert_eq!(element.span.source(), Some(Span::new(FileId(7), 42, 62)));
        let interpolation = ast.node(element.children[0]).expect("interpolation child");
        assert_eq!(
            interpolation.span.source(),
            Some(Span::new(FileId(7), 47, 56))
        );
    }

    #[test]
    fn base_compile_uses_binding_metadata_for_prefixed_interpolations() {
        let mut options = Vue3CompilerOptions {
            prefix_identifiers: true,
            mode: "function".into(),
            ..Vue3CompilerOptions::default()
        };
        options
            .binding_metadata
            .insert("props".into(), "props".into());
        options
            .binding_metadata
            .insert("setup".into(), "setup-maybe-ref".into());
        options
            .binding_metadata
            .insert("literal".into(), "literal-const".into());
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<div>{{ props }} {{ setup }} {{ literal }}</div>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let result = base_compile(source, options);
        assert!(result.code.contains("$props.props"));
        assert!(result.code.contains("$setup.setup"));
        assert!(result.code.contains("$setup.literal"));
    }

    #[test]
    fn base_compile_rewrites_event_handler_statement_scopes() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<div @click=\"() => {\n        for (const x in list) {\n          log(x)\n        }\n        error(x)\n      }\"/>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let result = base_compile(
            source,
            Vue3CompilerOptions {
                prefix_identifiers: true,
                mode: "function".into(),
                ..Vue3CompilerOptions::default()
            },
        );
        assert!(result.code.contains("for (const x in _ctx.list)"));
        assert!(result.code.contains("_ctx.log(x)"));
        assert!(result.code.contains("_ctx.error(_ctx.x)"));
    }

    #[test]
    fn base_compile_marks_vnode_hook_need_patch() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div @vue:updated="foo" />"#.into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let result = base_compile(
            source,
            Vue3CompilerOptions {
                prefix_identifiers: true,
                cache_handlers: true,
                mode: "function".into(),
                ..Vue3CompilerOptions::default()
            },
        );
        assert!(result.code.contains("onVnodeUpdated: _ctx.foo"));
        assert!(result.code.contains("512 /* NEED_PATCH */"));
        assert!(!result.code.contains("onVue:updated"));
        assert!(!result.code.contains(r#"["onVnodeUpdated"]"#));
    }

    #[test]
    fn base_compile_generates_core_integration_directives() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div id="foo" :class="bar.baz">
  {{ world.burn() }}
  <div v-if="ok">yes</div>
  <template v-else>no</template>
  <div v-for="(value, index) in list"><span>{{ value + index }}</span></div>
</div>"#
                .into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let result = base_compile(
            source,
            Vue3CompilerOptions {
                source_map: true,
                ..Vue3CompilerOptions::default()
            },
        );
        assert!(result.code.contains("class: _normalizeClass(bar.baz)"));
        assert!(result.code.contains("ok\n        ? (_openBlock()"));
        assert!(result.code.contains("_renderList(list, (value, index) =>"));
        assert!(result
            .code
            .contains("_toDisplayString(value + index), 1 /* TEXT */"));
        let map = result.map.expect("source map");
        assert_eq!(map.sources, vec!["foo.vue"]);
        assert_eq!(
            map.sources_content,
            Some(vec![Some(
                r#"<div id="foo" :class="bar.baz">
  {{ world.burn() }}
  <div v-if="ok">yes</div>
  <template v-else>no</template>
  <div v-for="(value, index) in list"><span>{{ value + index }}</span></div>
</div>"#
                    .into()
            )])
        );
    }

    #[test]
    fn base_compile_keeps_v_for_aliases_local_when_prefixed() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div v-for="(value, index) in list">{{ value + index }}</div>"#.into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let result = base_compile(
            source,
            Vue3CompilerOptions {
                prefix_identifiers: true,
                mode: "function".into(),
                ..Vue3CompilerOptions::default()
            },
        );
        assert!(result
            .code
            .contains("_renderList(_ctx.list, (value, index) =>"));
        assert!(result.code.contains("_toDisplayString(value + index)"));
        assert!(!result.code.contains("_ctx.value + _ctx.index"));
    }

    #[test]
    fn base_compile_wraps_v_memo_nodes_with_runtime_helper() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div><div v-memo="[x]"></div></div>"#.into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let result = base_compile(
            source,
            Vue3CompilerOptions {
                prefix_identifiers: true,
                mode: "module".into(),
                ..Vue3CompilerOptions::default()
            },
        );
        assert!(result.code.contains("withMemo as _withMemo"));
        assert!(result.code.contains(
            r#"_withMemo([_ctx.x], () => (_openBlock(), _createElementBlock("div")), _cache, 0)"#
        ));
    }

    #[test]
    fn base_compile_generates_v_for_memo_cache_path() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div><div v-for="{ x, y } in list" :key="x" v-memo="[x, y === z]"><span>foobar</span></div></div>"#.into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let result = base_compile(
            source,
            Vue3CompilerOptions {
                prefix_identifiers: true,
                mode: "module".into(),
                ..Vue3CompilerOptions::default()
            },
        );
        assert!(result.code.contains("isMemoSame as _isMemoSame"));
        assert!(result
            .code
            .contains("_renderList(_ctx.list, ({ x, y }, __, ___, _cached) =>"));
        assert!(result.code.contains("const _memo = ([x, y === _ctx.z])"));
        assert!(result
            .code
            .contains("_cached.key === x && _isMemoSame(_cached, _memo)"));
        assert!(result.code.contains("_item.memo = _memo"));
        assert!(!result.code.contains("_ctx.x, _ctx.y"));
    }

    #[test]
    fn base_compile_wraps_component_default_slot_with_ctx() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<Child><div/></Child>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let result = base_compile(
            source,
            Vue3CompilerOptions {
                mode: "module".into(),
                scope_id: Some("test".into()),
                ..Vue3CompilerOptions::default()
            },
        );
        assert!(result
            .code
            .contains("const _component_Child = _resolveComponent(\"Child\")"));
        assert!(result.code.contains("default: _withCtx(() => ["));
        assert!(result.code.contains("_createElementVNode(\"div\")"));
    }

    #[test]
    fn base_compile_wraps_named_component_slots_with_ctx() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<Child>
        <template #foo="{ msg }">{{ msg }}</template>
        <template #bar><div/></template>
      </Child>"#
                .into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let result = base_compile(
            source,
            Vue3CompilerOptions {
                mode: "module".into(),
                scope_id: Some("test".into()),
                ..Vue3CompilerOptions::default()
            },
        );
        assert!(result.code.contains("foo: _withCtx(({ msg }) => ["));
        assert!(result
            .code
            .contains("_createTextVNode(_toDisplayString(msg), 1 /* TEXT */)"));
        assert!(result.code.contains("bar: _withCtx(() => ["));
        assert!(result.code.contains("_: 1 /* STABLE */"));
    }

    #[test]
    fn base_compile_wraps_dynamic_component_slots_with_create_slots() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<Child>
        <template #foo v-if="ok"><div/></template>
        <template v-for="i in list" #[i]><div/></template>
      </Child>"#
                .into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let result = base_compile(
            source,
            Vue3CompilerOptions {
                mode: "module".into(),
                scope_id: Some("test".into()),
                ..Vue3CompilerOptions::default()
            },
        );
        assert!(result
            .code
            .contains("_createSlots({ _: 2 /* DYNAMIC */ }, ["));
        assert!(result.code.contains("name: \"foo\""));
        assert!(result.code.contains("fn: _withCtx(() => ["));
        assert!(result.code.contains("name: i"));
        assert!(result.code.contains(", 1024 /* DYNAMIC_SLOTS */"));
    }

    #[test]
    fn root_codegen_projection_uses_child_for_slot_outlet() {
        let root = json!({
            "children": [{
                "type": 1,
                "tagType": 2,
                "codegenNode": { "type": 14 }
            }]
        });

        assert_eq!(
            root_codegen_projection(&root),
            json!({ "kind": "child", "index": 0 })
        );
    }

    #[test]
    fn root_codegen_projection_uses_single_element_codegen_as_block() {
        let root = json!({
            "children": [{
                "type": 1,
                "tagType": 0,
                "codegenNode": { "type": 13 }
            }]
        });

        assert_eq!(
            root_codegen_projection(&root),
            json!({ "kind": "childCodegen", "index": 0, "asBlock": true })
        );
    }

    #[test]
    fn root_codegen_projection_preserves_non_element_child() {
        let root = json!({ "children": [{ "type": 11, "codegenNode": { "type": 13 } }] });

        assert_eq!(
            root_codegen_projection(&root),
            json!({ "kind": "child", "index": 0 })
        );
    }

    #[test]
    fn root_codegen_projection_marks_single_visible_root_fragment() {
        let root = json!({
            "children": [
                { "type": 3 },
                { "type": 1, "tagType": 0 },
                { "type": 3 }
            ]
        });

        assert_eq!(
            root_codegen_projection(&root),
            json!({ "kind": "fragment", "patchFlag": 2112 })
        );
    }

    #[test]
    fn get_constant_type_projection_handles_static_interpolation_and_props() {
        let interpolation = get_constant_type_projection(&json!({
            "node": {
                "type": 5,
                "content": { "type": 4, "content": "1", "constType": 3 }
            },
            "context": {}
        }));
        assert_eq!(interpolation["constantType"], json!(3));

        let static_props = get_constant_type_projection(&json!({
            "node": {
                "type": 1,
                "tag": "div",
                "tagType": 0,
                "props": [],
                "children": [],
                "codegenNode": {
                    "type": 13,
                    "isBlock": false,
                    "props": {
                        "type": 15,
                        "properties": [{
                            "type": 16,
                            "key": { "type": 4, "content": "id", "isStatic": true },
                            "value": { "type": 4, "content": "foo", "isStatic": true }
                        }]
                    }
                }
            },
            "context": {}
        }));
        assert_eq!(static_props["constantType"], json!(3));
    }

    #[test]
    fn cache_static_projection_caches_static_child_arrays() {
        let projection = cache_static_projection(&json!({
            "root": {
                "children": [{
                    "type": 1,
                    "tag": "div",
                    "tagType": 0,
                    "props": [],
                    "children": [
                        {
                            "type": 1,
                            "tag": "span",
                            "tagType": 0,
                            "props": [],
                            "children": [],
                            "codegenNode": { "type": 13, "isBlock": false }
                        },
                        {
                            "type": 1,
                            "tag": "i",
                            "tagType": 0,
                            "props": [],
                            "children": [],
                            "codegenNode": { "type": 13, "isBlock": false }
                        }
                    ],
                    "codegenNode": {
                        "type": 13,
                        "isBlock": true,
                        "children": [{ "type": 1 }, { "type": 1 }]
                    }
                }]
            },
            "context": {}
        }));

        assert_eq!(
            projection["operations"],
            json!([
                {
                    "kind": "setPatchFlag",
                    "path": ["children", "0", "children", "0", "codegenNode"],
                    "patchFlag": -1
                },
                {
                    "kind": "setPatchFlag",
                    "path": ["children", "0", "children", "1", "codegenNode"],
                    "patchFlag": -1
                },
                {
                    "kind": "cacheChildrenArray",
                    "path": ["children", "0", "codegenNode", "children"],
                    "childrenPath": ["children", "0", "children"],
                    "needArraySpread": true
                }
            ])
        );
    }

    #[test]
    fn cache_static_projection_hoists_props_and_dynamic_props() {
        let projection = cache_static_projection(&json!({
            "root": {
                "children": [{
                    "type": 1,
                    "tag": "div",
                    "tagType": 0,
                    "props": [],
                    "children": [
                        {
                            "type": 1,
                            "tag": "span",
                            "tagType": 0,
                            "props": [],
                            "children": [],
                            "codegenNode": {
                                "type": 13,
                                "patchFlag": 512,
                                "props": {
                                    "type": 15,
                                    "properties": [{
                                        "type": 16,
                                        "key": { "type": 4, "content": "id", "isStatic": true },
                                        "value": { "type": 4, "content": "foo", "isStatic": true }
                                    }]
                                }
                            }
                        },
                        {
                            "type": 1,
                            "tag": "p",
                            "tagType": 0,
                            "props": [],
                            "children": [],
                            "codegenNode": {
                                "type": 13,
                                "patchFlag": 8,
                                "dynamicProps": "[\"foo\"]"
                            }
                        }
                    ],
                    "codegenNode": { "type": 13, "isBlock": true }
                }]
            },
            "context": {}
        }));

        assert_eq!(
            projection["operations"],
            json!([
                {
                    "kind": "hoistProps",
                    "path": ["children", "0", "children", "0", "codegenNode", "props"]
                },
                {
                    "kind": "hoistDynamicProps",
                    "path": ["children", "0", "children", "1", "codegenNode", "dynamicProps"]
                }
            ])
        );
    }

    #[test]
    fn cache_static_projection_caches_dynamic_template_slot_returns() {
        let dynamic_slot = json!({
            "type": 8,
            "children": ["foo + ", { "type": 4, "content": "bar", "constType": 0 }]
        });
        let projection = cache_static_projection(&json!({
            "root": {
                "children": [{
                    "type": 1,
                    "tag": "Comp",
                    "tagType": 1,
                    "props": [],
                    "children": [{
                        "type": 1,
                        "tag": "template",
                        "tagType": 3,
                        "props": [{
                            "type": 7,
                            "name": "slot",
                            "arg": dynamic_slot
                        }],
                        "children": [{
                            "type": 1,
                            "tag": "span",
                            "tagType": 0,
                            "props": [],
                            "children": [],
                            "codegenNode": { "type": 13, "isBlock": false }
                        }]
                    }],
                    "codegenNode": {
                        "type": 13,
                        "children": {
                            "type": 15,
                            "properties": [{
                                "key": dynamic_slot,
                                "value": {
                                    "type": 18,
                                    "returns": [{ "type": 1 }]
                                }
                            }]
                        }
                    }
                }]
            },
            "context": {}
        }));

        assert_eq!(projection["operations"][0]["kind"], json!("setPatchFlag"));
        assert_eq!(
            projection["operations"][1],
            json!({
                "kind": "cacheSlotReturns",
                "ownerPath": ["children", "0"],
                "slot": {
                    "kind": "dynamic",
                    "node": dynamic_slot
                },
                "needArraySpread": true
            })
        );
    }

    #[test]
    fn cache_static_projection_downgrades_static_svg_blocks_except_with_directives() {
        let static_svg = cache_static_projection(&json!({
            "root": {
                "children": [{
                    "type": 1,
                    "tag": "div",
                    "tagType": 0,
                    "props": [],
                    "children": [{
                        "type": 1,
                        "tag": "svg",
                        "tagType": 0,
                        "props": [],
                        "children": [],
                        "codegenNode": { "type": 13, "isBlock": true }
                    }],
                    "codegenNode": {
                        "type": 13,
                        "isBlock": true,
                        "children": [{ "type": 1 }]
                    }
                }]
            },
            "context": {}
        }));
        assert_eq!(
            static_svg["operations"][0],
            json!({
                "kind": "setBlock",
                "path": ["children", "0", "children", "0", "codegenNode"],
                "isBlock": false
            })
        );

        let svg_with_directive = cache_static_projection(&json!({
            "root": {
                "children": [{
                    "type": 1,
                    "tag": "div",
                    "tagType": 0,
                    "props": [],
                    "children": [{
                        "type": 1,
                        "tag": "svg",
                        "tagType": 0,
                        "props": [{ "type": 7, "name": "foo" }],
                        "children": [{
                            "type": 1,
                            "tag": "path",
                            "tagType": 0,
                            "props": [],
                            "children": [],
                            "codegenNode": { "type": 13, "isBlock": false }
                        }],
                        "codegenNode": {
                            "type": 13,
                            "isBlock": true,
                            "children": [{ "type": 1 }]
                        }
                    }],
                    "codegenNode": { "type": 13, "isBlock": true }
                }]
            },
            "context": {}
        }));
        let svg_codegen_path = json!(["children", "0", "children", "0", "codegenNode"]);
        assert!(svg_with_directive["operations"]
            .as_array()
            .expect("operations")
            .iter()
            .all(|operation| operation["path"] != svg_codegen_path));
        assert_eq!(
            svg_with_directive["operations"][1],
            json!({
                "kind": "cacheChildrenArray",
                "path": ["children", "0", "children", "0", "codegenNode", "children"],
                "childrenPath": ["children", "0", "children", "0", "children"],
                "needArraySpread": true
            })
        );
    }

    #[test]
    fn transform_for_projection_preserves_skipped_alias_slots_and_locs() {
        let source = "<span v-for=\"( item,, index ) in items\" />";
        let exp_start = source.find("( item").unwrap();
        let projection = transform_for_projection(&json!({
            "dir": {
                "exp": {
                    "content": "( item,, index ) in items",
                    "loc": {
                        "start": { "offset": exp_start, "line": 1, "column": exp_start + 1 },
                        "end": { "offset": exp_start + "( item,, index ) in items".len(), "line": 1, "column": exp_start + "( item,, index ) in items".len() + 1 },
                        "source": "( item,, index ) in items"
                    }
                },
                "loc": { "source": "v-for=\"( item,, index ) in items\"" }
            },
            "node": { "type": 1, "tagType": 0, "children": [] },
            "context": {}
        }));

        assert_eq!(projection["parseResult"]["value"]["content"], json!("item"));
        assert!(projection["parseResult"]["key"].is_null());
        assert_eq!(
            projection["parseResult"]["index"]["content"],
            json!("index")
        );
        assert_eq!(
            projection["parseResult"]["source"]["content"],
            json!("items")
        );
        assert_eq!(
            projection["parseResult"]["index"]["loc"]["start"]["offset"],
            json!(source.find("index").unwrap())
        );
    }

    #[test]
    fn transform_for_projection_reports_missing_and_malformed_expression() {
        let missing = transform_for_projection(&json!({
            "dir": { "loc": { "source": "v-for" } },
            "node": { "type": 1, "tagType": 0 },
            "context": {}
        }));
        assert_eq!(missing["errors"], json!([{ "code": 31, "loc": "dir" }]));

        let malformed = transform_for_projection(&json!({
            "dir": {
                "exp": {
                    "content": "item in",
                    "loc": { "start": { "offset": 0, "line": 1, "column": 1 }, "source": "item in" }
                },
                "loc": { "source": "v-for=\"item in\"" }
            },
            "node": { "type": 1, "tagType": 0 },
            "context": {}
        }));
        assert_eq!(malformed["errors"], json!([{ "code": 32, "loc": "dir" }]));
    }

    #[test]
    fn transform_for_projection_prefixes_source_and_alias_defaults() {
        let projection = transform_for_projection(&json!({
            "dir": {
                "exp": {
                    "content": "({ foo = bar, baz: [qux = quux] }) in list.concat([foo])",
                    "loc": {
                        "start": { "offset": 0, "line": 1, "column": 1 },
                        "end": { "offset": 58, "line": 1, "column": 59 },
                        "source": "({ foo = bar, baz: [qux = quux] }) in list.concat([foo])"
                    }
                },
                "loc": { "source": "v-for" }
            },
            "node": { "type": 1, "tagType": 0, "children": [] },
            "context": { "prefixIdentifiers": true, "identifiers": {}, "bindingMetadata": {} }
        }));

        assert_eq!(
            projection["parseResult"]["source"]["kind"],
            json!("compound")
        );
        assert_eq!(
            projection["parseResult"]["source"]["children"][0]["content"],
            json!("_ctx.list")
        );
        assert_eq!(
            projection["parseResult"]["value"]["kind"],
            json!("compound")
        );
        let value = &projection["parseResult"]["value"]["children"];
        assert_eq!(value[1]["content"], json!("foo"));
        assert_eq!(value[3]["content"], json!("_ctx.bar"));
        assert_eq!(value[5]["content"], json!("qux"));
        assert_eq!(value[7]["content"], json!("_ctx.quux"));
        assert_eq!(projection["locals"], json!(["foo", "qux"]));
    }

    #[test]
    fn transform_for_projection_reports_template_child_key_placement() {
        let projection = transform_for_projection(&json!({
            "dir": {
                "exp": {
                    "content": "item in items",
                    "loc": { "start": { "offset": 0, "line": 1, "column": 1 }, "source": "item in items" }
                },
                "loc": { "source": "v-for" }
            },
            "node": {
                "type": 1,
                "tagType": 3,
                "children": [{
                    "type": 1,
                    "tag": "div",
                    "props": [{
                        "type": 7,
                        "name": "bind",
                        "arg": { "type": 4, "content": "key", "isStatic": true },
                        "loc": { "source": ":key=\"item.id\"" }
                    }]
                }]
            },
            "context": {}
        }));
        assert_eq!(
            projection["templateKeyErrors"],
            json!([{ "code": 33, "loc": { "source": ":key=\"item.id\"" } }])
        );
    }

    #[test]
    fn build_slots_projection_tracks_slot_locals_and_dynamic_slots() {
        let projection = build_slots_projection(&json!({
            "node": {
                "type": 1,
                "tagType": 1,
                "props": [{
                    "type": 7,
                    "name": "slot",
                    "exp": {
                        "type": 8,
                        "children": [
                            "{ ",
                            { "type": 4, "content": "foo", "isStatic": false },
                            " }"
                        ],
                        "loc": { "source": "{ foo }" }
                    }
                }],
                "children": [
                    { "type": 5, "content": { "type": 4, "content": "foo", "isStatic": false } }
                ],
                "loc": { "source": "<Comp/>" }
            },
            "context": { "prefixIdentifiers": true, "identifiers": {}, "bindingMetadata": {} }
        }));

        assert_eq!(
            projection["properties"][0]["key"]["content"],
            json!("default")
        );
        assert_eq!(projection["properties"][0]["indices"], json!([0]));
        assert_eq!(projection["hasDynamicSlots"], json!(false));

        let tracking = track_slot_scopes_projection(&json!({
            "node": {
                "type": 1,
                "tagType": 1,
                "props": [{
                    "type": 7,
                    "name": "slot",
                    "exp": {
                        "type": 8,
                        "children": [
                            "{ ",
                            { "type": 4, "content": "foo", "isStatic": false },
                            " }"
                        ],
                        "loc": { "source": "{ foo }" }
                    }
                }]
            }
        }));
        assert_eq!(tracking["locals"], json!(["foo"]));
    }

    #[test]
    fn build_slots_projection_lowers_if_and_for_dynamic_slots() {
        let if_projection = build_slots_projection(&json!({
            "node": {
                "type": 1,
                "tagType": 1,
                "props": [],
                "children": [{
                    "type": 1,
                    "tag": "template",
                    "tagType": 3,
                    "props": [
                        { "type": 7, "name": "slot", "arg": { "type": 4, "content": "one", "isStatic": true }, "loc": { "source": "#one" } },
                        { "type": 7, "name": "if", "exp": { "type": 4, "content": "_ctx.ok", "isStatic": false }, "loc": { "source": "v-if=\"ok\"" } }
                    ],
                    "children": [{ "type": 2, "content": "hello" }]
                }]
            },
            "context": { "prefixIdentifiers": true, "identifiers": {}, "bindingMetadata": {} }
        }));
        assert_eq!(
            if_projection["dynamicSlots"][0]["kind"],
            json!("conditional")
        );
        assert_eq!(
            if_projection["dynamicSlots"][0]["test"]["content"],
            json!("_ctx.ok")
        );

        let for_projection = build_slots_projection(&json!({
            "node": {
                "type": 1,
                "tagType": 1,
                "props": [],
                "children": [{
                    "type": 1,
                    "tag": "template",
                    "tagType": 3,
                    "props": [
                        { "type": 7, "name": "slot", "arg": { "type": 4, "content": "name", "isStatic": false }, "loc": { "source": "#[name]" } },
                        {
                            "type": 7,
                            "name": "for",
                            "exp": { "type": 4, "content": "name in list", "loc": { "source": "name in list", "start": { "offset": 0, "line": 1, "column": 1 } } },
                            "forParseResult": {
                                "source": { "type": 4, "content": "_ctx.list", "isStatic": false },
                                "value": { "type": 4, "content": "name", "isStatic": false },
                                "key": null,
                                "index": null
                            }
                        }
                    ],
                    "children": [{ "type": 5, "content": { "type": 4, "content": "name", "isStatic": false } }]
                }]
            },
            "context": { "prefixIdentifiers": true, "identifiers": {}, "bindingMetadata": {} }
        }));
        assert_eq!(for_projection["dynamicSlots"][0]["kind"], json!("for"));
        assert_eq!(
            for_projection["dynamicSlots"][0]["source"]["content"],
            json!("_ctx.list")
        );
        assert_eq!(
            for_projection["dynamicSlots"][0]["slot"]["name"]["content"],
            json!("name")
        );
    }

    #[test]
    fn transform_on_projection_projects_dynamic_event_key_and_prefixes_handler() {
        let projection = transform_on_projection(&json!({
            "dir": {
                "arg": { "type": 4, "content": "_ctx.event", "isStatic": false },
                "exp": { "type": 4, "content": "handler", "loc": { "source": "handler" } },
                "modifiers": []
            },
            "node": { "tagType": 0 },
            "context": {
                "prefixIdentifiers": true,
                "identifiers": {},
                "bindingMetadata": {}
            }
        }));

        assert_eq!(
            projection["props"][0]["key"],
            json!({
                "kind": "compound",
                "children": [
                    { "kind": "helperString", "helper": "TO_HANDLER_KEY" },
                    { "kind": "node", "path": "dir.arg" },
                    ")"
                ]
            })
        );
        assert_eq!(
            projection["props"][0]["value"]["content"],
            json!("_ctx.handler")
        );
        assert_eq!(projection["props"][0]["dynamicKey"], json!(true));
        assert_eq!(
            projection["props"][0]["ignoreDynamicKeyForNormalize"],
            json!(true)
        );
    }

    #[test]
    fn transform_on_projection_wraps_inline_statements_and_caches_members() {
        let inline = transform_on_projection(&json!({
            "dir": {
                "arg": { "type": 4, "content": "click", "isStatic": true },
                "exp": { "type": 4, "content": "foo($event)", "loc": { "source": "foo($event)" } },
                "modifiers": []
            },
            "node": { "tagType": 0 },
            "context": {
                "prefixIdentifiers": true,
                "cacheHandlers": true,
                "identifiers": {},
                "bindingMetadata": {}
            }
        }));
        assert_eq!(inline["props"][0]["cache"], json!(true));
        assert_eq!(
            inline["props"][0]["value"]["children"][0],
            json!("$event => (")
        );

        let member = transform_on_projection(&json!({
            "dir": {
                "arg": { "type": 4, "content": "click", "isStatic": true },
                "exp": { "type": 4, "content": "foo", "loc": { "source": "foo" } },
                "modifiers": []
            },
            "node": { "tagType": 0 },
            "context": {
                "prefixIdentifiers": true,
                "cacheHandlers": true,
                "identifiers": {},
                "bindingMetadata": {}
            }
        }));
        assert_eq!(member["props"][0]["cache"], json!(true));
        assert_eq!(
            member["props"][0]["value"]["children"][1]["content"],
            json!("_ctx.foo && _ctx.foo(...args)")
        );

        let component_member = transform_on_projection(&json!({
            "dir": {
                "arg": { "type": 4, "content": "click", "isStatic": true },
                "exp": { "type": 4, "content": "foo", "loc": { "source": "foo" } },
                "modifiers": []
            },
            "node": { "tagType": 1 },
            "context": {
                "prefixIdentifiers": true,
                "cacheHandlers": true,
                "identifiers": {},
                "bindingMetadata": {}
            }
        }));
        assert_eq!(component_member["props"][0]["cache"], json!(false));
    }

    #[test]
    fn transform_element_props_projection_keeps_dynamic_handlers_unwrapped_for_normalize() {
        let projection = transform_element_props_projection(&json!({
            "props": [{
                "kind": "directiveProp",
                "dynamicKey": true,
                "ignoreDynamicKeyForNormalize": true,
                "valueConstant": false
            }],
            "context": {},
            "isComponent": false
        }));

        assert_eq!(projection["patchFlag"], json!(16));
        assert_eq!(projection["normalizeProps"], json!(false));
    }

    #[test]
    fn transform_on_projection_marks_setup_const_handlers_constant() {
        let projection = transform_on_projection(&json!({
            "dir": {
                "arg": { "type": 4, "content": "keydown", "isStatic": true },
                "exp": { "type": 4, "content": "foo", "loc": { "source": "foo" } },
                "modifiers": []
            },
            "node": { "tagType": 0 },
            "context": {
                "prefixIdentifiers": true,
                "bindingMetadata": { "foo": "setup-const" }
            }
        }));

        assert_eq!(
            projection["props"][0]["value"]["content"],
            json!("$setup.foo")
        );
        assert_eq!(projection["props"][0]["value"]["constType"], json!(1));
        assert_eq!(projection["props"][0]["valueConstant"], json!(true));
    }

    #[test]
    fn transform_model_projection_emits_model_value_and_update_props() {
        let projection = transform_model_projection(&json!({
            "dir": {
                "exp": {
                    "type": 4,
                    "content": "model",
                    "loc": { "source": "model" }
                },
                "modifiers": []
            },
            "node": { "tagType": 0 },
            "context": {}
        }));

        assert_eq!(
            projection["props"][0]["key"],
            json!({ "kind": "static", "content": "modelValue" })
        );
        assert_eq!(projection["props"][0]["dynamic"], json!(true));
        assert_eq!(
            projection["props"][1]["key"],
            json!({ "kind": "static", "content": "onUpdate:modelValue" })
        );
        assert_eq!(
            projection["props"][1]["value"]["children"][0],
            json!("$event => ((")
        );
    }

    #[test]
    fn transform_model_projection_handles_dynamic_argument() {
        let projection = transform_model_projection(&json!({
            "dir": {
                "exp": {
                    "type": 4,
                    "content": "_ctx.model",
                    "loc": { "source": "model" }
                },
                "arg": {
                    "type": 4,
                    "content": "_ctx.value",
                    "isStatic": false
                },
                "modifiers": []
            },
            "node": { "tagType": 0 },
            "context": { "prefixIdentifiers": true }
        }));

        assert_eq!(
            projection["props"][0]["key"],
            json!({ "kind": "node", "path": "dir.arg" })
        );
        assert_eq!(
            projection["props"][1]["key"],
            json!({
                "kind": "compound",
                "children": ["\"onUpdate:\" + ", { "kind": "node", "path": "dir.arg" }]
            })
        );
    }

    #[test]
    fn transform_model_projection_reports_invalid_expression_errors() {
        let no_expression = transform_model_projection(&json!({
            "dir": { "modifiers": [] },
            "node": { "tagType": 0 },
            "context": {}
        }));
        assert_eq!(no_expression["errors"], json!([41]));

        let malformed = transform_model_projection(&json!({
            "dir": {
                "exp": {
                    "type": 4,
                    "content": "a + b",
                    "loc": { "source": "a + b" }
                },
                "modifiers": []
            },
            "node": { "tagType": 0 },
            "context": {}
        }));
        assert_eq!(malformed["errors"], json!([42]));
    }

    #[test]
    fn transform_model_projection_tracks_cache_and_scope_refs() {
        let cached = transform_model_projection(&json!({
            "dir": {
                "exp": {
                    "type": 4,
                    "content": "_ctx.foo",
                    "loc": { "source": "foo" }
                },
                "modifiers": []
            },
            "node": { "tagType": 0 },
            "context": {
                "prefixIdentifiers": true,
                "cacheHandlers": true,
                "identifiers": {}
            }
        }));
        assert_eq!(cached["props"][1]["cache"], json!(true));
        assert_eq!(cached["props"][1]["dynamic"], json!(false));

        let scoped = transform_model_projection(&json!({
            "dir": {
                "exp": {
                    "type": 8,
                    "loc": { "source": "foo[i]" },
                    "children": [
                        { "type": 4, "content": "_ctx.foo" },
                        "[",
                        { "type": 4, "content": "i" },
                        "]"
                    ]
                },
                "modifiers": []
            },
            "node": { "tagType": 0 },
            "context": {
                "prefixIdentifiers": true,
                "cacheHandlers": true,
                "identifiers": { "i": 1 }
            }
        }));
        assert_eq!(scoped["props"][1]["cache"], json!(false));
        assert_eq!(scoped["props"][1]["dynamic"], json!(true));
    }

    #[test]
    fn transform_model_projection_generates_component_modifiers() {
        let projection = transform_model_projection(&json!({
            "dir": {
                "exp": {
                    "type": 4,
                    "content": "foo",
                    "loc": { "source": "foo" }
                },
                "arg": {
                    "type": 4,
                    "content": "bar",
                    "isStatic": true
                },
                "modifiers": [
                    { "content": "trim" },
                    { "content": "bar-baz" }
                ]
            },
            "node": { "tagType": 1 },
            "context": {}
        }));

        assert_eq!(
            projection["props"][2]["key"],
            json!({ "kind": "static", "content": "barModifiers" })
        );
        assert_eq!(
            projection["props"][2]["value"]["content"],
            json!("{ trim: true, \"bar-baz\": true }")
        );
    }

    #[test]
    fn transform_model_projection_marks_static_argument_hydration_event() {
        let projection = transform_model_projection(&json!({
            "dir": {
                "exp": {
                    "type": 4,
                    "content": "model",
                    "loc": { "source": "model" }
                },
                "arg": {
                    "type": 4,
                    "content": "foo-value",
                    "isStatic": true
                },
                "modifiers": []
            },
            "node": { "tagType": 0 },
            "context": {}
        }));

        assert_eq!(projection["props"][1]["hydrate"], json!(true));
    }

    #[test]
    fn resolve_component_type_projection_uses_setup_bindings() {
        let projection = resolve_component_type_projection(&json!({
            "node": { "type": 1, "tag": "Example", "tagType": 1, "props": [] },
            "context": {
                "bindingMetadata": { "Example": "setup-maybe-ref" },
                "inline": true
            }
        }));

        assert_eq!(projection["kind"], json!("expression"));
        assert_eq!(projection["content"], json!("_unref(Example)"));
        assert_eq!(projection["helpers"], json!(["UNREF"]));
    }

    #[test]
    fn resolve_component_type_projection_handles_namespaced_props_binding() {
        let projection = resolve_component_type_projection(&json!({
            "node": { "type": 1, "tag": "Foo.Example", "tagType": 1, "props": [] },
            "context": {
                "bindingMetadata": { "Foo": "props" },
                "inline": false
            }
        }));

        assert_eq!(projection["kind"], json!("expression"));
        assert_eq!(
            projection["content"],
            json!("_unref($props[\"Foo\"]).Example")
        );
    }

    #[test]
    fn resolve_component_type_projection_marks_self_reference_asset() {
        let projection = resolve_component_type_projection(&json!({
            "node": { "type": 1, "tag": "Example", "tagType": 1, "props": [] },
            "context": { "selfName": "Example" }
        }));

        assert_eq!(projection["kind"], json!("asset"));
        assert_eq!(projection["component"], json!("Example__self"));
        assert_eq!(projection["assetId"], json!("_component_Example"));
    }

    #[test]
    fn resolve_component_type_projection_handles_dynamic_component_is() {
        let projection = resolve_component_type_projection(&json!({
            "node": {
                "type": 1,
                "tag": "component",
                "tagType": 1,
                "props": [
                    {
                        "type": 7,
                        "name": "bind",
                        "arg": { "type": 4, "content": "is", "isStatic": true },
                        "exp": { "type": 4, "content": "foo", "isStatic": false }
                    }
                ]
            },
            "context": {}
        }));

        assert_eq!(projection["kind"], json!("dynamic"));
        assert_eq!(projection["helper"], json!("RESOLVE_DYNAMIC_COMPONENT"));
        assert_eq!(projection["argument"]["content"], json!("foo"));
    }

    #[test]
    fn resolve_component_type_projection_casts_vue_is_attribute() {
        let projection = resolve_component_type_projection(&json!({
            "node": {
                "type": 1,
                "tag": "div",
                "tagType": 1,
                "props": [
                    {
                        "type": 6,
                        "name": "is",
                        "value": { "content": "vue:foo" }
                    }
                ]
            },
            "context": {}
        }));

        assert_eq!(projection["kind"], json!("asset"));
        assert_eq!(projection["component"], json!("foo"));
        assert_eq!(projection["assetId"], json!("_component_foo"));
    }

    #[test]
    fn base_parse_classifies_lowercase_builtins_and_dynamic_component_as_components() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<teleport/><suspense/><keep-alive/><base-transition/><component/>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let root = ast.root_node().expect("root");
        let tags = root
            .children
            .iter()
            .map(|id| ast.node(*id).expect("element"))
            .map(|node| match &node.kind {
                Vue3AstKind::Element(element) => (&element.tag, element.tag_type),
                _ => panic!("expected element"),
            })
            .collect::<Vec<_>>();

        assert!(tags
            .iter()
            .all(|(_, tag_type)| *tag_type == Vue3ElementType::Component));
    }

    #[test]
    fn transform_element_props_projection_flags_class_style_and_dynamic_props() {
        let projection = transform_element_props_projection(&json!({
            "props": [
                { "kind": "directiveProp", "name": "class", "valueConstant": false },
                { "kind": "directiveProp", "name": "style", "valueConstant": false },
                { "kind": "directiveProp", "name": "foo", "valueConstant": false }
            ],
            "context": {},
            "isComponent": false
        }));

        assert_eq!(projection["patchFlag"], json!(14));
        assert_eq!(projection["dynamicPropNames"], json!(["foo"]));
        assert_eq!(projection["normalizeClass"], json!(true));
        assert_eq!(projection["normalizeStyle"], json!(true));
    }

    #[test]
    fn transform_element_props_projection_normalizes_style_arrays() {
        let array_literal = transform_element_props_projection(&json!({
            "props": [
                {
                    "kind": "directiveProp",
                    "name": "style",
                    "valueConstant": true,
                    "valueStartsWithArray": true
                }
            ],
            "context": {},
            "isComponent": false
        }));
        assert_eq!(array_literal["normalizeStyle"], json!(true));

        let merged_style = transform_element_props_projection(&json!({
            "props": [
                { "kind": "attribute", "name": "style" },
                {
                    "kind": "directiveProp",
                    "name": "style",
                    "valueConstant": true
                }
            ],
            "context": {},
            "isComponent": false
        }));
        assert_eq!(merged_style["normalizeStyle"], json!(true));
    }

    #[test]
    fn transform_element_props_projection_wraps_object_bind_props() {
        let projection = transform_element_props_projection(&json!({
            "props": [{ "kind": "objectBind" }],
            "context": {},
            "isComponent": false
        }));

        assert_eq!(projection["patchFlag"], json!(16));
        assert_eq!(projection["normalizeProps"], json!(true));
        assert_eq!(projection["guardReactiveProps"], json!(true));
    }

    #[test]
    fn transform_element_props_projection_marks_ref_and_runtime_directives_need_patch() {
        let ref_projection = transform_element_props_projection(&json!({
            "props": [{ "kind": "attribute", "name": "ref" }],
            "context": {},
            "isComponent": false
        }));
        assert_eq!(ref_projection["patchFlag"], json!(512));

        let runtime_projection = transform_element_props_projection(&json!({
            "props": [{ "kind": "runtimeDirective" }],
            "context": {},
            "isComponent": false
        }));
        assert_eq!(runtime_projection["patchFlag"], json!(512));
    }

    #[test]
    fn transform_element_props_projection_marks_ref_for_in_v_for_scope() {
        let static_ref = transform_element_props_projection(&json!({
            "props": [{ "kind": "attribute", "name": "ref" }],
            "context": { "vForDepth": 1 },
            "isComponent": false
        }));
        assert_eq!(static_ref["refForMarker"], json!(true));

        let dynamic_ref = transform_element_props_projection(&json!({
            "props": [
                {
                    "kind": "directiveProp",
                    "name": "ref",
                    "valueConstant": false
                }
            ],
            "context": { "vForDepth": 1 },
            "isComponent": false
        }));
        assert_eq!(dynamic_ref["refForMarker"], json!(true));

        let object_bind = transform_element_props_projection(&json!({
            "props": [{ "kind": "objectBind" }],
            "context": { "vForDepth": 1 },
            "isComponent": false
        }));
        assert_eq!(object_bind["refForMarker"], json!(true));

        let outside_for = transform_element_props_projection(&json!({
            "props": [{ "kind": "attribute", "name": "ref" }],
            "context": {},
            "isComponent": false
        }));
        assert_eq!(outside_for["refForMarker"], json!(false));
    }

    #[test]
    fn transform_element_props_projection_forces_blocks_for_selected_props() {
        let key_projection = transform_element_props_projection(&json!({
            "props": [
                {
                    "kind": "directiveProp",
                    "name": "key",
                    "forceBlock": true
                }
            ],
            "context": {},
            "isComponent": false
        }));
        assert_eq!(key_projection["shouldUseBlock"], json!(true));

        let vnode_hook_projection = transform_element_props_projection(&json!({
            "props": [
                {
                    "kind": "directiveProp",
                    "forceBlock": true
                }
            ],
            "context": {},
            "isComponent": false
        }));
        assert_eq!(vnode_hook_projection["shouldUseBlock"], json!(true));
    }

    #[test]
    fn transform_element_props_projection_projects_inline_template_ref_keys() {
        let projection = transform_element_props_projection(&json!({
            "props": [{ "kind": "attribute", "name": "ref", "value": "input" }],
            "context": {
                "inline": true,
                "bindingMetadata": {
                    "input": "setup-ref"
                }
            },
            "isComponent": false
        }));

        assert_eq!(
            projection["inlineTemplateRefs"],
            json!([{ "content": "input" }])
        );

        let outside_inline = transform_element_props_projection(&json!({
            "props": [{ "kind": "attribute", "name": "ref", "value": "input" }],
            "context": {
                "bindingMetadata": {
                    "input": "setup-ref"
                }
            },
            "isComponent": false
        }));
        assert_eq!(outside_inline["inlineTemplateRefs"], json!([]));
    }

    #[test]
    fn build_directive_args_projection_keeps_runtime_directive_shape() {
        let projection = build_directive_args_projection(&json!({
            "dir": {
                "name": "baz",
                "exp": { "type": 4, "content": "y" },
                "arg": { "type": 4, "content": "arg", "isStatic": false },
                "modifiers": ["mod", "mad"]
            }
        }));

        assert_eq!(
            projection,
            json!({
                "runtime": {
                    "kind": "asset",
                    "name": "baz"
                },
                "includeExp": true,
                "includeArg": true,
                "modifiers": [
                    { "name": "mod" },
                    { "name": "mad" }
                ]
            })
        );
    }

    #[test]
    fn transform_element_children_projection_lowers_builtin_component_children() {
        let suspense = transform_element_children_projection(&json!({
            "tag": "SUSPENSE",
            "children": [
                { "type": 2, "content": "foo" }
            ]
        }));
        assert_eq!(suspense["kind"], json!("slots"));
        assert_eq!(suspense["slots"][0]["name"], json!("default"));
        assert_eq!(suspense["shouldUseBlock"], json!(true));

        let suspense_templates = transform_element_children_projection(&json!({
            "tag": "SUSPENSE",
            "children": [
                {
                    "type": 1,
                    "tag": "template",
                    "props": [
                        {
                            "name": "slot",
                            "arg": { "content": "fallback" }
                        }
                    ]
                }
            ]
        }));
        assert_eq!(suspense_templates["slots"][0]["name"], json!("fallback"));
        assert_eq!(
            suspense_templates["slots"][0]["unwrapTemplate"],
            json!(true)
        );

        let keep_alive = transform_element_children_projection(&json!({
            "tag": "KEEP_ALIVE",
            "children": [
                { "type": 1, "tag": "span" }
            ]
        }));
        assert_eq!(keep_alive["kind"], json!("children"));
        assert_eq!(keep_alive["patchFlag"], json!(1024));
        assert_eq!(keep_alive["shouldUseBlock"], json!(true));
    }

    #[test]
    fn transform_element_props_projection_marks_hydration_event_without_props_for_constants() {
        let projection = transform_element_props_projection(&json!({
            "props": [
                {
                    "kind": "directiveProp",
                    "name": "onKeydown",
                    "valueConstant": true
                }
            ],
            "context": {},
            "isComponent": false
        }));

        assert_eq!(projection["patchFlag"], json!(32));
        assert_eq!(projection["dynamicPropNames"], json!([]));
    }

    #[test]
    fn base_parse_decodes_builtin_text_entities() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "&gt;&lt;&amp;&apos;&quot;&nbsp;&foo;".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let root = ast.root_node().expect("root");
        let text = ast.node(root.children[0]).expect("text");
        assert!(matches!(
            &text.kind,
            Vue3AstKind::Text(value) if value.value == "><&'\"\u{00a0}&foo;"
        ));
        assert_eq!(text.span.source(), Some(Span::new(FileId(0), 0, 36)));
    }

    #[test]
    fn base_parse_preserves_nbsp_as_non_whitespace_default_child() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source:
                "<Comp>\n        \u{00a0}\n        <template #one>foo</template>\n      </Comp>"
                    .into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let root = ast.root_node().expect("root");
        let comp_id = root.children[0];
        let comp = ast.node(comp_id).expect("component");
        assert!(matches!(
            ast.node(comp.children[0]).map(|node| &node.kind),
            Some(Vue3AstKind::Text(text)) if text.value.contains('\u{00a0}')
        ));
    }

    #[test]
    fn scope_ref_identifier_matching_uses_boundaries() {
        assert!(source_contains_identifier("fn(i)", "i"));
        assert!(!source_contains_identifier("click", "i"));
        assert!(!source_contains_identifier("_ctx.list", "i"));
    }

    #[test]
    fn base_parse_preserves_raw_content_inside_v_pre() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div v-pre :id="foo"><Comp/>{{ bar }}</div><div :id="foo"><Comp/>{{ bar }}</div>"#.into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let root = ast.root_node().expect("root");
        let with_pre = ast.node(root.children[0]).expect("v-pre div");
        let Vue3AstKind::Element(with_pre_element) = &with_pre.kind else {
            panic!("expected element");
        };
        assert_eq!(with_pre_element.props.len(), 1);
        assert!(matches!(
            &with_pre_element.props[0],
            Vue3Prop::Attribute(attr) if attr.name == ":id" && attr.value.as_deref() == Some("foo")
        ));
        let raw_component = ast.node(with_pre.children[0]).expect("raw component");
        assert!(matches!(
            &raw_component.kind,
            Vue3AstKind::Element(element)
                if element.tag == "Comp" && element.tag_type == Vue3ElementType::Element
        ));
        let raw_text = ast.node(with_pre.children[1]).expect("raw interpolation");
        assert!(matches!(
            &raw_text.kind,
            Vue3AstKind::Text(text) if text.value == "{{ bar }}"
        ));

        let without_pre = ast.node(root.children[1]).expect("normal div");
        let Vue3AstKind::Element(without_pre_element) = &without_pre.kind else {
            panic!("expected element");
        };
        assert!(matches!(
            &without_pre_element.props[0],
            Vue3Prop::Directive(dir) if dir.name == "bind"
        ));
        let component = ast.node(without_pre.children[0]).expect("component");
        assert!(matches!(
            &component.kind,
            Vue3AstKind::Element(element)
                if element.tag == "Comp" && element.tag_type == Vue3ElementType::Component
        ));
        let interpolation = ast.node(without_pre.children[1]).expect("interpolation");
        assert!(matches!(interpolation.kind, Vue3AstKind::Interpolation(_)));
    }

    #[test]
    fn base_parse_splits_half_open_interpolations_inside_v_pre() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<div v-pre><span>{{ number </span><span>}}</span></div>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let root = ast.root_node().expect("root");
        let div = ast.node(root.children[0]).expect("div");
        let first_span = ast.node(div.children[0]).expect("first span");
        let second_span = ast.node(div.children[1]).expect("second span");

        assert!(matches!(
            &ast.node(first_span.children[0]).expect("first text").kind,
            Vue3AstKind::Text(text) if text.value == "{{ number "
        ));
        assert!(matches!(
            &ast.node(second_span.children[0]).expect("second text").kind,
            Vue3AstKind::Text(text) if text.value == "}}"
        ));
    }

    #[test]
    fn base_parse_preserves_inter_element_whitespace_in_preserve_mode() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<div/> \n <div/>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(
            source,
            &Vue3CompilerOptions {
                whitespace: "preserve".into(),
                ..Vue3CompilerOptions::default()
            },
        );
        let root = ast.root_node().expect("root");
        assert_eq!(root.children.len(), 3);
        assert!(matches!(
            &ast.node(root.children[1]).expect("whitespace text").kind,
            Vue3AstKind::Text(text) if text.value == " "
        ));
    }

    #[test]
    fn base_parse_preserves_text_inside_configured_pre_tag() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<pre>\n  foo  bar  </pre><span>\n  foo   bar</span>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(
            source,
            &Vue3CompilerOptions {
                pre_tags: vec!["pre".into()],
                ignore_newline_tags: vec!["pre".into()],
                ..Vue3CompilerOptions::default()
            },
        );
        let root = ast.root_node().expect("root");
        let pre = ast.node(root.children[0]).expect("pre");
        let span = ast.node(root.children[1]).expect("span");
        assert!(matches!(
            &ast.node(pre.children[0]).expect("pre text").kind,
            Vue3AstKind::Text(text) if text.value == "  foo  bar  "
        ));
        assert!(matches!(
            &ast.node(span.children[0]).expect("span text").kind,
            Vue3AstKind::Text(text) if text.value == " foo bar"
        ));
    }

    #[test]
    fn base_parse_extends_open_element_spans_to_eof() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<template><div>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let root = ast.root_node().expect("root");
        let template = ast.node(root.children[0]).expect("template");
        let div = ast.node(template.children[0]).expect("div");
        assert_eq!(template.span.source(), Some(Span::new(FileId(0), 0, 15)));
        assert_eq!(div.span.source(), Some(Span::new(FileId(0), 10, 15)));
    }

    #[test]
    fn base_parse_recovers_from_incomplete_child_start_tag_at_eof() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<template><div id=abc /".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let root = ast.root_node().expect("root");
        let template = ast.node(root.children[0]).expect("template");
        assert_eq!(template.span.source(), Some(Span::new(FileId(0), 0, 23)));
        assert_eq!(template.children.len(), 1);
        assert!(matches!(
            &ast.node(template.children[0]).expect("recovered text").kind,
            Vue3AstKind::Text(text) if text.value == "/"
        ));
    }

    #[test]
    fn base_parse_treats_empty_incomplete_end_tag_as_text() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<template></".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let root = ast.root_node().expect("root");
        let template = ast.node(root.children[0]).expect("template");
        assert_eq!(template.span.source(), Some(Span::new(FileId(0), 0, 12)));
        assert!(matches!(
            &ast.node(template.children[0]).expect("end tag text").kind,
            Vue3AstKind::Text(text) if text.value == "</"
        ));
    }

    #[test]
    fn base_parse_uses_configured_namespace_for_cdata_text() {
        let mut namespaces = BTreeMap::new();
        namespaces.insert("svg".into(), vuec_ast::HtmlNamespace::Svg);
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<template><svg><![CDATA[cdata]]></svg></template>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(
            source,
            &Vue3CompilerOptions {
                namespaces,
                ..Vue3CompilerOptions::default()
            },
        );
        let root = ast.root_node().expect("root");
        let template = ast.node(root.children[0]).expect("template");
        let svg = ast.node(template.children[0]).expect("svg");
        assert!(matches!(
            &svg.kind,
            Vue3AstKind::Element(element) if element.ns == vuec_ast::HtmlNamespace::Svg
        ));
        assert!(matches!(
            &ast.node(svg.children[0]).expect("cdata text").kind,
            Vue3AstKind::Text(text) if text.value == "cdata"
        ));
        assert_eq!(
            ast.node(svg.children[0]).expect("cdata text").span.source(),
            Some(Span::new(FileId(0), 24, 29))
        );
    }

    #[test]
    fn base_parse_drops_cdata_children_in_html_namespace() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<template><![CDATA[cdata]]></template>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let root = ast.root_node().expect("root");
        let template = ast.node(root.children[0]).expect("template");
        assert!(template.children.is_empty());
    }

    #[test]
    fn base_parse_keeps_non_matching_end_tag_as_text_in_textarea() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<textarea></div></textarea>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let root = ast.root_node().expect("root");
        let textarea = ast.node(root.children[0]).expect("textarea");
        assert_eq!(textarea.span.source(), Some(Span::new(FileId(0), 0, 27)));
        assert!(matches!(
            &ast.node(textarea.children[0]).expect("raw end tag text").kind,
            Vue3AstKind::Text(text) if text.value == "</div>"
        ));
        assert_eq!(
            ast.node(textarea.children[0])
                .expect("raw end tag text")
                .span
                .source(),
            Some(Span::new(FileId(0), 10, 16))
        );
    }

    #[test]
    fn base_parse_extends_open_span_across_invalid_end_tags() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<template></div></template>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let root = ast.root_node().expect("root");
        let template = ast.node(root.children[0]).expect("template");
        assert_eq!(template.span.source(), Some(Span::new(FileId(0), 0, 27)));
        assert!(template.children.is_empty());
    }

    #[test]
    fn base_parse_treats_html_textarea_and_style_as_special_text_modes() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<textarea>some<div>text</div>and<!--comment--></textarea><style>&amp;</style>"
                .into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let root = ast.root_node().expect("root");
        let textarea = ast.node(root.children[0]).expect("textarea");
        let style = ast.node(root.children[1]).expect("style");

        assert_eq!(textarea.children.len(), 1);
        assert!(matches!(
            &ast.node(textarea.children[0]).expect("textarea text").kind,
            Vue3AstKind::Text(text) if text.value == "some<div>text</div>and<!--comment-->"
        ));
        assert_eq!(style.children.len(), 1);
        assert!(matches!(
            &ast.node(style.children[0]).expect("style text").kind,
            Vue3AstKind::Text(text) if text.value == "&amp;"
        ));
    }

    #[test]
    fn base_parse_textarea_decodes_entities_and_supports_interpolation() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<textarea>\n<div>{{ a &lt; b }}</textarea>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(
            source,
            &Vue3CompilerOptions {
                ignore_newline_tags: vec!["textarea".into()],
                ..Vue3CompilerOptions::default()
            },
        );
        let root = ast.root_node().expect("root");
        let textarea = ast.node(root.children[0]).expect("textarea");

        assert_eq!(textarea.children.len(), 2);
        assert!(matches!(
            &ast.node(textarea.children[0]).expect("textarea text").kind,
            Vue3AstKind::Text(text) if text.value == "<div>"
        ));
        assert_eq!(
            ast.node(textarea.children[0])
                .expect("textarea text")
                .span
                .source(),
            Some(Span::new(FileId(0), 10, 16))
        );
        assert!(matches!(
            &ast.node(textarea.children[1]).expect("interpolation").kind,
            Vue3AstKind::Interpolation(interpolation)
                if interpolation.expression.source_string() == "a < b"
        ));
    }

    #[test]
    fn base_parse_decodes_dom_text_and_attribute_entity_compatibility() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: r#"<div a="&ampersand;" b="&amp;ersand;" c="&amp!">&ampersand;&#x86;</div>"#
                .into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(source, &Vue3CompilerOptions::default());
        let root = ast.root_node().expect("root");
        let div = ast.node(root.children[0]).expect("div");
        let Vue3AstKind::Element(element) = &div.kind else {
            panic!("expected element");
        };

        assert!(matches!(
            &element.props[0],
            Vue3Prop::Attribute(attr) if attr.value.as_deref() == Some("&ampersand;")
        ));
        assert!(matches!(
            &element.props[1],
            Vue3Prop::Attribute(attr) if attr.value.as_deref() == Some("&ersand;")
        ));
        assert!(matches!(
            &element.props[2],
            Vue3Prop::Attribute(attr) if attr.value.as_deref() == Some("&!")
        ));
        assert!(matches!(
            &ast.node(div.children[0]).expect("text").kind,
            Vue3AstKind::Text(text) if text.value == "&ersand;\u{2020}"
        ));
    }

    #[test]
    fn base_parse_applies_dom_namespace_rules_without_static_namespace_map() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: concat!(
                "<svg><foreignObject><test/></foreignObject></svg>",
                "<math><mtext><test/></mtext><mtext><malignmark/></mtext></math>",
            )
            .into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(
            source,
            &Vue3CompilerOptions {
                dom_namespaces: true,
                ..Vue3CompilerOptions::default()
            },
        );
        let root = ast.root_node().expect("root");
        let svg = ast.node(root.children[0]).expect("svg");
        let math = ast.node(root.children[1]).expect("math");
        let foreign_object = ast.node(svg.children[0]).expect("foreignObject");
        let svg_test = ast.node(foreign_object.children[0]).expect("svg test");
        let mtext_html = ast.node(math.children[0]).expect("mtext html");
        let mtext_math = ast.node(math.children[1]).expect("mtext math");
        let math_test = ast.node(mtext_html.children[0]).expect("math test");
        let malignmark = ast.node(mtext_math.children[0]).expect("malignmark");

        assert!(matches!(
            &svg.kind,
            Vue3AstKind::Element(element) if element.ns == vuec_ast::HtmlNamespace::Svg
        ));
        assert!(matches!(
            &svg_test.kind,
            Vue3AstKind::Element(element) if element.ns == vuec_ast::HtmlNamespace::Html
        ));
        assert!(matches!(
            &math.kind,
            Vue3AstKind::Element(element) if element.ns == vuec_ast::HtmlNamespace::MathMl
        ));
        assert!(matches!(
            &math_test.kind,
            Vue3AstKind::Element(element) if element.ns == vuec_ast::HtmlNamespace::Html
        ));
        assert!(matches!(
            &malignmark.kind,
            Vue3AstKind::Element(element) if element.ns == vuec_ast::HtmlNamespace::MathMl
        ));
    }

    #[test]
    fn base_parse_uses_root_namespace_for_dom_integration_rules() {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: "<foreignObject><test/></foreignObject><script><g/><g/></script>".into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let ast = Vue3Dialect::base_parse(
            source,
            &Vue3CompilerOptions {
                root_namespace: vuec_ast::HtmlNamespace::Svg,
                dom_namespaces: true,
                ..Vue3CompilerOptions::default()
            },
        );
        let root = ast.root_node().expect("root");
        let foreign_object = ast.node(root.children[0]).expect("foreignObject");
        let script = ast.node(root.children[1]).expect("script");
        let test = ast.node(foreign_object.children[0]).expect("test");

        assert!(matches!(
            &foreign_object.kind,
            Vue3AstKind::Element(element) if element.ns == vuec_ast::HtmlNamespace::Svg
        ));
        assert!(matches!(
            &test.kind,
            Vue3AstKind::Element(element) if element.ns == vuec_ast::HtmlNamespace::Html
        ));
        assert_eq!(script.children.len(), 2);
        assert!(script.children.iter().all(|child| {
            matches!(
                ast.node(*child).map(|node| &node.kind),
                Some(Vue3AstKind::Element(_))
            )
        }));
    }
}
