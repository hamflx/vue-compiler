#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use vuec_ast::{RuntimeHelper, TemplateAttribute, Vue3Ast, Vue3NodeKind};
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
}

impl Default for Vue3CompilerOptions {
    fn default() -> Self {
        Self {
            prefix_identifiers: false,
            mode: "module".into(),
            hoist_static: false,
            cache_handlers: false,
            scope_id: None,
            slotted: false,
            is_ts: false,
            expression_plugins: Vec::new(),
            source_map: false,
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

pub struct Vue3Dialect;

impl Vue3Dialect {
    pub fn base_parse(source: TemplateSource, _options: &Vue3CompilerOptions) -> Vue3Ast {
        let mut ast = Vue3Ast::new(
            Vue3NodeKind::Root,
            Some(Span::new(
                source.file_id,
                source.base_offset,
                source.base_offset + source.source.len(),
            )),
        );
        let root = ast.root;
        let mut stack = vec![root];
        let tokens = HtmlTokenizer::new(&source.source).tokenize();
        for token in tokens {
            let current_parent = *stack.last().unwrap_or(&root);
            match token.kind {
                HtmlTokenKind::Text(text) => push_text_and_interpolations(
                    &mut ast,
                    current_parent,
                    source.file_id,
                    source.base_offset + token.start,
                    &text,
                ),
                HtmlTokenKind::Comment(value) => {
                    let _id = ast.push_child(
                        current_parent,
                        Vue3NodeKind::Comment { value },
                        Some(Span::new(
                            source.file_id,
                            source.base_offset + token.start,
                            source.base_offset + token.end,
                        )),
                    );
                }
                HtmlTokenKind::StartTag {
                    name,
                    attributes,
                    self_closing,
                } => {
                    let id = ast.push_child(
                        current_parent,
                        Vue3NodeKind::Element {
                            tag: name,
                            attributes: attributes
                                .into_iter()
                                .map(|attr| TemplateAttribute {
                                    name: attr.name,
                                    value: attr.value,
                                })
                                .collect(),
                            self_closing,
                        },
                        Some(Span::new(
                            source.file_id,
                            source.base_offset + token.start,
                            source.base_offset + token.end,
                        )),
                    );
                    if !self_closing {
                        stack.push(id);
                    }
                }
                HtmlTokenKind::EndTag { name } => {
                    while stack.len() > 1 {
                        let Some(node_id) = stack.pop() else {
                            break;
                        };
                        if let Some(node) = ast.node(node_id) {
                            if matches!(&node.kind, Vue3NodeKind::Element { tag, .. } if tag == &name)
                            {
                                if let Some(node) = ast.node_mut(node_id) {
                                    if let Some(span) = node.span.source_mut() {
                                        span.end = vuec_source::BytePos(token.end);
                                    }
                                }
                                break;
                            }
                        }
                    }
                }
                HtmlTokenKind::Cdata(text) => {
                    push_text_and_interpolations(
                        &mut ast,
                        current_parent,
                        source.file_id,
                        source.base_offset + token.start,
                        &text,
                    );
                }
                HtmlTokenKind::Doctype(_) | HtmlTokenKind::Eof => {}
            }
        }
        ast
    }

    pub fn transform(ast: &mut Vue3Ast, ctx: &mut TransformContext) {
        let root_id = ast.root;
        let mut has_element = false;
        let mut has_nested_element = false;
        let mut has_interpolation = false;
        let mut walk = vec![(root_id, true)];
        while let Some((node_id, is_root)) = walk.pop() {
            if let Some(node) = ast.node(node_id) {
                for child_id in node.children.clone() {
                    if let Some(child) = ast.node(child_id) {
                        match &child.kind {
                            Vue3NodeKind::Element { .. } => {
                                has_element = true;
                                if matches!(
                                    &child.kind,
                                    Vue3NodeKind::Element { tag, .. } if tag == "slot"
                                ) {
                                    ctx.add_helper(RuntimeHelper::Vue3RenderSlot);
                                }
                                if !is_root {
                                    has_nested_element = true;
                                }
                                walk.push((child_id, false));
                            }
                            Vue3NodeKind::Interpolation { .. } => {
                                has_interpolation = true;
                            }
                            Vue3NodeKind::Text { .. } => {}
                            Vue3NodeKind::Comment { .. }
                            | Vue3NodeKind::Directive { .. }
                            | Vue3NodeKind::Root => {}
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
        if has_interpolation {
            ctx.add_helper(RuntimeHelper::Vue3ToDisplayString);
        }
    }

    pub fn generate(
        ast: &Vue3Ast,
        options: &Vue3CompilerOptions,
        ctx: &TransformContext,
    ) -> CodegenResult {
        let mut writer = CodeWriter::new();
        let helper_order = [
            RuntimeHelper::Vue3ToDisplayString,
            RuntimeHelper::Vue3CreateElementVNode,
            RuntimeHelper::Vue3RenderSlot,
            RuntimeHelper::Vue3OpenBlock,
            RuntimeHelper::Vue3CreateElementBlock,
        ];
        let root_id = ast.root;
        if let Some(root) = ast.node(root_id) {
            let helpers = render_helpers(&helper_order, ctx);
            if options.mode == "module" {
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
                writer.push_line("return function render(_ctx, _cache) {");
            } else if options.mode == "function" {
                writer.push_line("const _Vue = Vue");
                writer.newline();
                writer.push_line("return function render(_ctx, _cache) {");
            } else {
                writer.push_line("export function render(_ctx, _cache) {");
            }
            writer.indent();
            if !options.prefix_identifiers && options.mode != "module" {
                writer.push_line("with (_ctx) {");
                writer.indent();
                if !helpers.is_empty() {
                    writer.push_line(&format!("const {{ {} }} = _Vue", helper_aliases(&helpers)));
                    writer.newline();
                }
            }
            let expr = if root.children.len() == 1 {
                render_node_expr(ast, root.children[0], options, NodeRenderMode::Root)
            } else {
                render_children_array(ast, &root.children, options, true)
            };
            writer.push_line(&format!("return {}", expr));
            if !options.prefix_identifiers && options.mode != "module" {
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
            result.map = source_map_for_render(&result.code, &ast, &source);
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

fn render_helpers(order: &[RuntimeHelper], ctx: &TransformContext) -> Vec<RuntimeHelper> {
    order
        .iter()
        .copied()
        .filter(|helper| ctx.helpers.contains(helper))
        .collect()
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
        RuntimeHelper::Vue3ToDisplayString => "toDisplayString",
        RuntimeHelper::Vue3RenderList => "renderList",
        RuntimeHelper::Vue3RenderSlot => "renderSlot",
    }
}

fn source_map_for_render(
    code: &str,
    ast: &Vue3Ast,
    source: &TemplateSource,
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
        &mut names,
        &mut segments,
    );
    if segments.is_empty() {
        return None;
    }
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
    names: &mut Vec<String>,
    segments: &mut Vec<SourceMapSegment>,
    cursor: &mut usize,
) {
    let Some(node) = ast.node(node_id) else {
        return;
    };
    match &node.kind {
        Vue3NodeKind::Element { .. } => {
            add_vnode_mapping(code, node, base_offset, source, segments, cursor);
            for child_id in &node.children {
                collect_node_source_map(
                    code,
                    ast,
                    *child_id,
                    base_offset,
                    source,
                    names,
                    segments,
                    cursor,
                );
            }
        }
        Vue3NodeKind::Interpolation { .. } => {
            add_interpolation_mapping(code, node, base_offset, source, names, segments, cursor);
        }
        Vue3NodeKind::Root
        | Vue3NodeKind::Text { .. }
        | Vue3NodeKind::Comment { .. }
        | Vue3NodeKind::Directive { .. } => {}
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
        Vue3NodeKind::Element { tag, .. } => tag,
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
    names: &mut Vec<String>,
    segments: &mut Vec<SourceMapSegment>,
    cursor: &mut usize,
) {
    let Vue3NodeKind::Interpolation { expression } = &node.kind else {
        return;
    };
    let Some(span) = node.span.source() else {
        return;
    };
    let name = expression.trim().to_string();
    let name_index = if let Some(index) = names.iter().position(|existing| existing == &name) {
        index
    } else {
        names.push(name.clone());
        names.len() - 1
    };
    let local_start = span.start.0.saturating_sub(base_offset);
    let local_end = span.end.0.saturating_sub(base_offset);
    let Some(original_start) = source[local_start..local_end]
        .find(expression.trim())
        .map(|offset| local_start + offset)
    else {
        return;
    };
    let Some(start) = loc_for_offset(source, original_start) else {
        return;
    };
    let Some(end) = loc_for_offset(source, original_start + expression.trim().len()) else {
        return;
    };
    let needle = format!("_ctx.{name}");
    if let Some(offset) = find_code_offset(code, &needle, *cursor) {
        if let Some((line, column)) = loc_for_offset(code, offset) {
            segments.push(SourceMapSegment {
                generated_line: line,
                generated_column: column,
                original_line: start.0,
                original_column: start.1,
                name_index: Some(name_index),
            });
            segments.push(SourceMapSegment {
                generated_line: line,
                generated_column: column + needle.encode_utf16().count() as u32,
                original_line: end.0,
                original_column: end.1,
                name_index: None,
            });
            *cursor = offset + needle.len();
        }
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
    let rendered = children
        .iter()
        .filter_map(|child_id| ast.node(*child_id))
        .map(|child| {
            render_node_expr(
                ast,
                child.id,
                options,
                if is_root {
                    NodeRenderMode::Root
                } else {
                    NodeRenderMode::Child
                },
            )
        })
        .collect::<Vec<_>>();
    format!("[{}]", rendered.join(", "))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NodeRenderMode {
    Root,
    Child,
    Cached,
}

fn render_node_expr(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    options: &Vue3CompilerOptions,
    mode: NodeRenderMode,
) -> String {
    let Some(node) = ast.node(node_id) else {
        return "null".into();
    };
    match &node.kind {
        Vue3NodeKind::Root => render_children_array(ast, &node.children, options, true),
        Vue3NodeKind::Text { value } => quote_text(value),
        Vue3NodeKind::Interpolation { expression } => {
            format!(
                "_toDisplayString({})",
                render_expression(expression, options)
            )
        }
        Vue3NodeKind::Comment { value } => format!("/*{}*/", value),
        Vue3NodeKind::Directive { .. } => "null".into(),
        Vue3NodeKind::Element {
            tag,
            attributes,
            self_closing: _,
        } => {
            if tag == "slot" {
                return render_slot_outlet(attributes, options);
            }
            let helper = if mode == NodeRenderMode::Root {
                "_createElementBlock"
            } else {
                "_createElementVNode"
            };
            let props = render_props(attributes, options);
            let children = render_element_children(ast, &node.children, options, mode);
            let patch_flag = if mode == NodeRenderMode::Cached {
                ", -1 /* CACHED */"
            } else if tag != "template" && has_dynamic_children(ast, &node.children) {
                ", 1 /* TEXT */"
            } else {
                ""
            };
            let attrs = if props.is_empty() {
                "null".into()
            } else {
                props
            };
            let children_arg = if children.is_empty() {
                String::new()
            } else if mode == NodeRenderMode::Root && tag == "template" && children.starts_with('[')
            {
                format!(", {children}")
            } else {
                format!(", {children}")
            };
            if mode == NodeRenderMode::Root {
                format!(
                    "(_openBlock(), {}({}, {}{}{}))",
                    helper,
                    quote_string(tag),
                    attrs,
                    children_arg,
                    patch_flag
                )
            } else {
                format!(
                    "{}({}, {}{}{})",
                    helper,
                    quote_string(tag),
                    attrs,
                    children_arg,
                    patch_flag
                )
            }
        }
    }
}

fn render_slot_outlet(attributes: &[TemplateAttribute], options: &Vue3CompilerOptions) -> String {
    let name = attributes
        .iter()
        .find(|attr| attr.name == "name")
        .and_then(|attr| attr.value.as_deref())
        .unwrap_or("default");
    let slots = if options.prefix_identifiers || options.mode == "module" {
        "_ctx.$slots"
    } else {
        "$slots"
    };
    format!("_renderSlot({}, {})", slots, quote_string(name))
}

fn render_element_children(
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
    options: &Vue3CompilerOptions,
    parent_mode: NodeRenderMode,
) -> String {
    let child_nodes = children
        .iter()
        .filter_map(|child_id| ast.node(*child_id))
        .filter(|child| !matches!(child.kind, Vue3NodeKind::Comment { .. }))
        .collect::<Vec<_>>();
    if options.hoist_static
        && parent_mode == NodeRenderMode::Root
        && should_cache_children(&child_nodes)
    {
        let rendered = child_nodes
            .iter()
            .map(|child| render_node_expr(ast, child.id, options, NodeRenderMode::Cached))
            .collect::<Vec<_>>();
        if !rendered.is_empty() {
            return format!(
                "[...(_cache[0] || (_cache[0] = [{}]))]",
                rendered.join(", ")
            );
        }
    }
    let rendered = child_nodes
        .iter()
        .map(|child| render_node_expr(ast, child.id, options, NodeRenderMode::Child))
        .collect::<Vec<_>>();
    if rendered.is_empty() {
        String::new()
    } else if rendered.len() == 1
        && (parent_mode != NodeRenderMode::Root
            || matches!(
                child_nodes[0].kind,
                Vue3NodeKind::Text { .. } | Vue3NodeKind::Interpolation { .. }
            ))
    {
        rendered.into_iter().next().unwrap()
    } else {
        if rendered.len() == 1 {
            format!("[\n  {}\n]", rendered[0])
        } else {
            format!("[{}]", rendered.join(", "))
        }
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
        Vue3NodeKind::Element {
            tag,
            attributes,
            ..
        } if tag != "slot"
            && attributes.iter().all(|attr| {
                !attr.name.starts_with("v-")
                    && !attr.name.starts_with('@')
                    && !attr.name.starts_with(':')
            })
    )
}

fn has_dynamic_children(ast: &Vue3Ast, children: &[vuec_ast::NodeId]) -> bool {
    children.iter().any(|child_id| {
        ast.node(*child_id).is_some_and(|child| {
            matches!(child.kind, Vue3NodeKind::Interpolation { .. })
                || matches!(&child.kind, Vue3NodeKind::Element { .. } if has_dynamic_children(ast, &child.children))
        })
    })
}

fn render_props(attributes: &[TemplateAttribute], _options: &Vue3CompilerOptions) -> String {
    let props = attributes
        .iter()
        .filter(|attr| {
            !attr.name.starts_with("v-")
                && !attr.name.starts_with('@')
                && !attr.name.starts_with(':')
        })
        .map(|attr| match &attr.value {
            Some(value) => format!("{}: {}", json_key(&attr.name), quote_string(value)),
            None => format!("{}: true", json_key(&attr.name)),
        })
        .collect::<Vec<_>>();
    if props.is_empty() {
        String::new()
    } else {
        format!("{{ {} }}", props.join(", "))
    }
}

fn render_expression(expression: &str, options: &Vue3CompilerOptions) -> String {
    let expression = expression.trim();
    if !options.prefix_identifiers {
        return expression.to_string();
    }
    if expression
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '$' || ch == '.')
    {
        format!("_ctx.{expression}")
    } else {
        expression.to_string()
    }
}

fn expression_diagnostics(ast: &Vue3Ast, options: &Vue3CompilerOptions) -> Vec<String> {
    let store = JsAstStore::new();
    let source_type = expression_source_type(options);
    ast.nodes
        .iter()
        .filter_map(|node| match &node.kind {
            Vue3NodeKind::Interpolation { expression } => Some(expression.as_str()),
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
) {
    let mut cursor = 0usize;
    while let Some(open) = text[cursor..].find("{{") {
        let open = cursor + open;
        if open > cursor {
            push_text(
                ast,
                parent,
                file_id,
                token_start + cursor,
                &text[cursor..open],
            );
        }
        let expression_start = open + 2;
        let Some(close_offset) = text[expression_start..].find("}}") else {
            push_text(ast, parent, file_id, token_start + open, &text[open..]);
            return;
        };
        let close = expression_start + close_offset;
        let expression = text[expression_start..close].trim().to_string();
        let _id = ast.push_child(
            parent,
            Vue3NodeKind::Interpolation { expression },
            Some(Span::new(
                file_id,
                token_start + open,
                token_start + close + 2,
            )),
        );
        cursor = close + 2;
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
    let _id = ast.push_child(
        parent,
        Vue3NodeKind::Text {
            value: text.to_string(),
        },
        Some(Span::new(file_id, start, start + text.len())),
    );
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
}
