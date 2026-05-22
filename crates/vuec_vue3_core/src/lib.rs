#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use vuec_ast::{TemplateAttribute, Vue3Ast, Vue3NodeKind};
use vuec_codegen::{CodeWriter, SourceMapArtifact, SourceMapBuilder};
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
        let tokens = HtmlTokenizer::new(&source.source).tokenize();
        for token in tokens {
            match token.kind {
                HtmlTokenKind::Text(text) => {
                    push_text_and_interpolations(&mut ast, root, source.file_id, token.start, &text)
                }
                HtmlTokenKind::Comment(value) => {
                    let id = ast.push(
                        Vue3NodeKind::Comment { value },
                        Some(Span::new(source.file_id, token.start, token.end)),
                    );
                    ast.node_mut(root).unwrap().children.push(id);
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
                    ast.node_mut(root).unwrap().children.push(id);
                }
                HtmlTokenKind::EndTag { .. }
                | HtmlTokenKind::Cdata(_)
                | HtmlTokenKind::Doctype(_)
                | HtmlTokenKind::Eof => {}
            }
        }
        ast
    }

    pub fn transform(ast: &mut Vue3Ast, ctx: &mut TransformContext) {
        if let Some(root_id) = ast.root {
            if let Some(root) = ast.node_mut(root_id) {
                for child_id in root.children.clone() {
                    if let Some(child) = ast.node_mut(child_id) {
                        match child.kind {
                            Vue3NodeKind::Element { .. } => {
                                ctx.add_helper("createElementVNode");
                            }
                            Vue3NodeKind::Text { .. } | Vue3NodeKind::Interpolation { .. } => {
                                ctx.add_helper("toDisplayString");
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    pub fn generate(ast: &Vue3Ast, _options: &Vue3CompilerOptions) -> CodegenResult {
        let mut writer = CodeWriter::new();
        let mut map = SourceMapBuilder::new().file("generated.js");
        if let Some(root_id) = ast.root {
            if let Some(root) = ast.node(root_id) {
                writer.push_line("function render(_ctx, _cache) {");
                writer.indent();
                writer.push_line("return [");
                writer.indent();
                for child_id in &root.children {
                    if let Some(child) = ast.node(*child_id) {
                        match &child.kind {
                            Vue3NodeKind::Element {
                                tag,
                                attributes,
                                self_closing,
                            } => {
                                let attrs = attributes
                                    .iter()
                                    .map(|attr| match &attr.value {
                                        Some(value) => format!("{}={value:?}", attr.name),
                                        None => attr.name.clone(),
                                    })
                                    .collect::<Vec<_>>()
                                    .join(" ");
                                writer.push_line(&format!(
                                    "/* element:{tag} attrs:{attrs} self_closing:{self_closing} */ null,"
                                ));
                            }
                            Vue3NodeKind::Text { value } => {
                                writer.push_line(&format!("{value:?},"));
                            }
                            Vue3NodeKind::Interpolation { expression } => {
                                writer.push_line(&format!("_toDisplayString({expression}),"));
                            }
                            Vue3NodeKind::Comment { .. }
                            | Vue3NodeKind::Directive { .. }
                            | Vue3NodeKind::Root => {}
                        }
                        if let Some(span) = child.span {
                            map.add_mapping(1, 0, Some(span), Some("source.vue".into()));
                        }
                    }
                }
                writer.dedent();
                writer.push_line("];");
                writer.dedent();
                writer.push_line("}");
            }
        }
        let code = writer.finish();
        CodegenResult {
            code,
            map: Some(map.build()),
            ast_summary: format!("nodes={}", ast.len()),
            diagnostics: Vec::new(),
        }
    }

    pub fn base_compile(source: TemplateSource, options: Vue3CompilerOptions) -> CodegenResult {
        let mut ast = Self::base_parse(source.clone(), &options);
        let mut ctx = TransformContext::default();
        Self::transform(&mut ast, &mut ctx);
        let mut result = Self::generate(&ast, &options);
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

fn push_text_and_interpolations(
    ast: &mut Vue3Ast,
    root: vuec_ast::NodeId,
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
                root,
                file_id,
                token_start + cursor,
                &text[cursor..open],
            );
        }
        let expression_start = open + 2;
        let Some(close_offset) = text[expression_start..].find("}}") else {
            push_text(ast, root, file_id, token_start + open, &text[open..]);
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
        ast.node_mut(root).unwrap().children.push(id);
        cursor = close + 2;
    }
    if cursor < text.len() {
        push_text(ast, root, file_id, token_start + cursor, &text[cursor..]);
    }
}

fn push_text(ast: &mut Vue3Ast, root: vuec_ast::NodeId, file_id: FileId, start: usize, text: &str) {
    if text.is_empty() {
        return;
    }
    let id = ast.push(
        Vue3NodeKind::Text {
            value: text.to_string(),
        },
        Some(Span::new(file_id, start, start + text.len())),
    );
    ast.node_mut(root).unwrap().children.push(id);
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
