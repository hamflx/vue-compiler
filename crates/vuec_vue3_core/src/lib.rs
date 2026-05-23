#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use vuec_ast::{TemplateAttribute, Vue3Ast, Vue3NodeKind};
use vuec_codegen::{CodeWriter, SourceMapArtifact};
use vuec_html::{HtmlTokenKind, HtmlTokenizer};
use vuec_pass::TransformContext;
use vuec_source::{FileId, Span};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateSource {
    pub filename: String,
    pub source: String,
    pub file_id: FileId,
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
        let mut ast = Vue3Ast::new();
        let root = ast.push(
            Vue3NodeKind::Root,
            Some(Span::new(source.file_id, 0, source.source.len())),
        );
        ast.set_root(root);
        let mut stack = vec![root];
        let tokens = HtmlTokenizer::new(&source.source).tokenize();
        for token in tokens {
            let current_parent = *stack.last().unwrap_or(&root);
            match token.kind {
                HtmlTokenKind::Text(text) => {
                    push_text_and_interpolations(
                        &mut ast,
                        current_parent,
                        source.file_id,
                        token.start,
                        &text,
                    )
                }
                HtmlTokenKind::Comment(value) => {
                    let id = ast.push(
                        Vue3NodeKind::Comment { value },
                        Some(Span::new(source.file_id, token.start, token.end)),
                    );
                    ast.node_mut(current_parent).unwrap().children.push(id);
                }
                HtmlTokenKind::StartTag {
                    name,
                    attributes,
                    self_closing,
                } => {
                    let id = ast.push(
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
                        Some(Span::new(source.file_id, token.start, token.end)),
                    );
                    ast.node_mut(current_parent).unwrap().children.push(id);
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
                        token.start,
                        &text,
                    );
                }
                HtmlTokenKind::Doctype(_) | HtmlTokenKind::Eof => {}
            }
        }
        ast
    }

    pub fn transform(ast: &mut Vue3Ast, ctx: &mut TransformContext) {
        if let Some(root_id) = ast.root {
            let mut has_element = false;
            let mut has_nested_element = false;
            let mut has_text = false;
            let mut walk = vec![(root_id, true)];
            while let Some((node_id, is_root)) = walk.pop() {
                if let Some(node) = ast.node(node_id) {
                    for child_id in node.children.clone() {
                        if let Some(child) = ast.node(child_id) {
                            match &child.kind {
                                Vue3NodeKind::Element { .. } => {
                                    has_element = true;
                                    if !is_root {
                                        has_nested_element = true;
                                    }
                                    walk.push((child_id, false));
                                }
                                Vue3NodeKind::Text { .. } | Vue3NodeKind::Interpolation { .. } => {
                                    has_text = true;
                                }
                                Vue3NodeKind::Comment { .. } | Vue3NodeKind::Directive { .. } | Vue3NodeKind::Root => {}
                            }
                        }
                    }
                }
            }
            if has_element {
                ctx.add_helper("openBlock");
                ctx.add_helper("createElementBlock");
            }
            if has_nested_element {
                ctx.add_helper("createElementVNode");
            }
            if has_text {
                ctx.add_helper("toDisplayString");
            }
        }
    }

    pub fn generate(
        ast: &Vue3Ast,
        options: &Vue3CompilerOptions,
        ctx: &TransformContext,
    ) -> CodegenResult {
        let mut writer = CodeWriter::new();
        let helper_order = [
            "toDisplayString",
            "createElementVNode",
            "openBlock",
            "createElementBlock",
        ];
        if let Some(root_id) = ast.root {
            if let Some(root) = ast.node(root_id) {
                let helpers = render_helpers(&helper_order, ctx);
                if options.prefix_identifiers {
                    if !helpers.is_empty() {
                        writer.push_line(&format!(
                            "const {{ {} }} = Vue",
                            helper_aliases(&helpers)
                        ));
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
                if !options.prefix_identifiers {
                    writer.push_line("with (_ctx) {");
                    writer.indent();
                    if !helpers.is_empty() {
                        writer.push_line(&format!(
                            "const {{ {} }} = _Vue",
                            helper_aliases(&helpers)
                        ));
                        writer.newline();
                    }
                }
                let expr = if root.children.len() == 1 {
                    render_node_expr(ast, root.children[0], options, true)
                } else {
                    render_children_array(ast, &root.children, options, true)
                };
                writer.push_line(&format!("return {}", expr));
                if !options.prefix_identifiers {
                    writer.dedent();
                    writer.push_line("}");
                }
                writer.dedent();
                writer.push_line("}");
            }
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
        let mut result = Self::generate(&ast, &options, &ctx);
        result.diagnostics = ctx
            .diagnostics
            .into_vec()
            .into_iter()
            .map(|diagnostic| diagnostic.message)
            .collect();
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

fn render_helpers<'a>(order: &'a [&'a str], ctx: &TransformContext) -> Vec<&'a str> {
    order
        .iter()
        .copied()
        .filter(|helper| ctx.helpers.contains(*helper))
        .collect()
}

fn helper_aliases(helpers: &[&str]) -> String {
    helpers
        .iter()
        .map(|helper| format!("{helper}: _{helper}"))
        .collect::<Vec<_>>()
        .join(", ")
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
        .map(|child| render_node_expr(ast, child.id, options, is_root))
        .collect::<Vec<_>>();
    format!("[{}]", rendered.join(", "))
}

fn render_node_expr(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    options: &Vue3CompilerOptions,
    is_root: bool,
) -> String {
    let Some(node) = ast.node(node_id) else {
        return "null".into();
    };
    match &node.kind {
        Vue3NodeKind::Root => render_children_array(ast, &node.children, options, true),
        Vue3NodeKind::Text { value } => quote_text(value),
        Vue3NodeKind::Interpolation { expression } => {
            format!("_toDisplayString({})", render_expression(expression, options))
        }
        Vue3NodeKind::Comment { value } => format!("/*{}*/", value),
        Vue3NodeKind::Directive { .. } => "null".into(),
        Vue3NodeKind::Element {
            tag,
            attributes,
            self_closing: _,
        } => {
            let helper = if is_root {
                "_createElementBlock"
            } else {
                "_createElementVNode"
            };
            let props = render_props(attributes, options);
            let children = render_element_children(ast, &node.children, options, is_root);
            let patch_flag = if has_dynamic_children(ast, &node.children) {
                ", 1 /* TEXT */"
            } else {
                ""
            };
            let attrs = if props.is_empty() { "null".into() } else { props };
            let children_arg = if children.is_empty() {
                String::new()
            } else {
                format!(", {children}")
            };
            if is_root {
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

fn render_element_children(
    ast: &Vue3Ast,
    children: &[vuec_ast::NodeId],
    options: &Vue3CompilerOptions,
    parent_is_root: bool,
) -> String {
    let child_nodes = children
        .iter()
        .filter_map(|child_id| ast.node(*child_id))
        .filter(|child| !matches!(child.kind, Vue3NodeKind::Comment { .. }))
        .collect::<Vec<_>>();
    let rendered = child_nodes
        .iter()
        .map(|child| render_node_expr(ast, child.id, options, false))
        .collect::<Vec<_>>();
    if rendered.is_empty() {
        String::new()
    } else if rendered.len() == 1
        && (!parent_is_root
            || matches!(
                child_nodes[0].kind,
                Vue3NodeKind::Text { .. } | Vue3NodeKind::Interpolation { .. }
            ))
    {
        rendered.into_iter().next().unwrap()
    } else {
        format!("[{}]", rendered.join(", "))
    }
}

fn has_dynamic_children(ast: &Vue3Ast, children: &[vuec_ast::NodeId]) -> bool {
    children.iter().any(|child_id| {
        ast.node(*child_id).is_some_and(|child| {
            matches!(
                child.kind,
                Vue3NodeKind::Text { .. } | Vue3NodeKind::Interpolation { .. }
            ) || matches!(&child.kind, Vue3NodeKind::Element { .. } if has_dynamic_children(ast, &child.children))
        })
    })
}

fn render_props(attributes: &[TemplateAttribute], _options: &Vue3CompilerOptions) -> String {
    let props = attributes
        .iter()
        .filter(|attr| !attr.name.starts_with("v-") && !attr.name.starts_with('@') && !attr.name.starts_with(':'))
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
    if expression.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '$' || ch == '.') {
        format!("_ctx.{expression}")
    } else {
        expression.to_string()
    }
}

fn quote_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".into())
}

fn json_key(key: &str) -> String {
    if key.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '$') {
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
        let id = ast.push(
            Vue3NodeKind::Interpolation { expression },
            Some(Span::new(
                file_id,
                token_start + open,
                token_start + close + 2,
            )),
        );
        ast.node_mut(parent).unwrap().children.push(id);
        cursor = close + 2;
    }
    if cursor < text.len() {
        push_text(ast, parent, file_id, token_start + cursor, &text[cursor..]);
    }
}

fn push_text(ast: &mut Vue3Ast, parent: vuec_ast::NodeId, file_id: FileId, start: usize, text: &str) {
    if text.is_empty() {
        return;
    }
    let id = ast.push(
        Vue3NodeKind::Text {
            value: text.to_string(),
        },
        Some(Span::new(file_id, start, start + text.len())),
    );
    ast.node_mut(parent).unwrap().children.push(id);
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
        };
        let result = compile_ssr(source, Vue3CompilerOptions::default());
        assert!(result.code.starts_with("/* ssr */"));
    }
}
