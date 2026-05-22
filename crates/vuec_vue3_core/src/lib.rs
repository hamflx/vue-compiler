#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use vuec_ast::{Vue3Ast, Vue3NodeKind};
use vuec_codegen::{CodeWriter, SourceMapArtifact, SourceMapBuilder};
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
        let root = ast.push(Vue3NodeKind::Root, Some(Span::new(source.file_id, 0, source.source.len())));
        ast.set_root(root);
        for (index, segment) in source.source.split('<').enumerate() {
            if index == 0 {
                if !segment.trim().is_empty() {
                    let id = ast.push(
                        Vue3NodeKind::Text {
                            value: segment.trim().to_string(),
                        },
                        Some(Span::new(source.file_id, 0, segment.len())),
                    );
                    ast.node_mut(root).unwrap().children.push(id);
                }
                continue;
            }
            if let Some(tag_end) = segment.find('>') {
                let tag = segment[..tag_end].split_whitespace().next().unwrap_or("").trim_matches('/');
                if !tag.is_empty() {
                    let id = ast.push(
                        Vue3NodeKind::Element {
                            tag: tag.to_string(),
                        },
                        Some(Span::new(source.file_id, 0, tag.len())),
                    );
                    ast.node_mut(root).unwrap().children.push(id);
                }
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
                            Vue3NodeKind::Element { tag } => {
                                writer.push_line(&format!("/* element:{tag} */ null,"));
                            }
                            Vue3NodeKind::Text { value } => {
                                writer.push_line(&format!("{value:?},"));
                            }
                            Vue3NodeKind::Interpolation { expression } => {
                                writer.push_line(&format!("_toDisplayString({expression}),"));
                            }
                            Vue3NodeKind::Comment { .. } | Vue3NodeKind::Directive { .. } | Vue3NodeKind::Root => {}
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
