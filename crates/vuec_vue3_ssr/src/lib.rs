#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use vuec_ast::{TemplateAttribute, Vue3AstKind, Vue3NodeKind};
use vuec_codegen::CodeWriter;
use vuec_vue3_core::{TemplateSource, Vue3CompilerOptions, Vue3Dialect};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SsrCompilerOptions {
    pub core: Vue3CompilerOptions,
    pub scope_id: Option<String>,
    pub slotted: bool,
}

impl Default for SsrCompilerOptions {
    fn default() -> Self {
        Self {
            core: Vue3CompilerOptions::default(),
            scope_id: None,
            slotted: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SsrTransformSummary {
    pub elements: usize,
    pub interpolations: usize,
    pub components: usize,
    pub slots: usize,
    pub teleports: usize,
    pub suspenses: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SsrCompileResult {
    pub code: String,
    pub map: Option<vuec_codegen::SourceMapArtifact>,
    pub ast_summary: String,
    pub diagnostics: Vec<String>,
    pub preamble: String,
}

pub fn compile(source: TemplateSource, options: SsrCompilerOptions) -> SsrCompileResult {
    let ast = Vue3Dialect::base_parse(source, &options.core);
    let summary = summarize_ssr(&ast.nodes.iter().map(|node| &node.kind).collect::<Vec<_>>());
    let has_slot = ast.nodes.iter().any(
        |node| matches!(node.kind, Vue3AstKind::Element(ref element) if element.tag == "slot"),
    );
    let mut writer = CodeWriter::new();
    if options.scope_id.is_some() {
        writer.push_line("const { mergeProps: _mergeProps } = require(\"vue\")");
        writer.push_line(
            "const { ssrRenderAttrs: _ssrRenderAttrs } = require(\"vue/server-renderer\")",
        );
        writer.push_line("");
    }
    if has_slot {
        writer.push_line(
            "const { ssrRenderSlot: _ssrRenderSlot } = require(\"vue/server-renderer\")",
        );
        writer.push_line("");
    }
    writer.push_line("function ssrRender(_ctx, _push, _parent, _attrs) {");
    writer.indent();
    if let Some(root) = ast.root_node() {
        render_ssr_children(&ast, &root.children, has_slot, &options, &mut writer);
    }
    writer.dedent();
    writer.push_line("}");
    SsrCompileResult {
        code: writer.finish(),
        map: None,
        ast_summary: format!(
            "ssr:elements={},interpolations={},components={},slots={},teleports={},suspenses={}",
            summary.elements,
            summary.interpolations,
            summary.components,
            summary.slots,
            summary.teleports,
            summary.suspenses
        ),
        diagnostics: Vec::new(),
        preamble: String::new(),
    }
}

pub fn summarize_ssr(nodes: &[&Vue3NodeKind]) -> SsrTransformSummary {
    let mut summary = SsrTransformSummary {
        elements: 0,
        interpolations: 0,
        components: 0,
        slots: 0,
        teleports: 0,
        suspenses: 0,
    };
    for node in nodes {
        match node {
            Vue3AstKind::Element(element) => {
                summary.elements += 1;
                if is_component(&element.tag) {
                    summary.components += 1;
                }
                match element.tag.as_str() {
                    "slot" => summary.slots += 1,
                    "teleport" | "Teleport" => summary.teleports += 1,
                    "suspense" | "Suspense" => summary.suspenses += 1,
                    _ => {}
                }
            }
            Vue3AstKind::Interpolation(_) => summary.interpolations += 1,
            _ => {}
        }
    }
    summary
}

fn render_start_tag(
    tag: &str,
    attributes: &[TemplateAttribute],
    self_closing: bool,
    options: &SsrCompilerOptions,
) -> String {
    let mut rendered = String::new();
    rendered.push('<');
    rendered.push_str(tag);
    if options.slotted {
        rendered.push_str(" data-vuec-slotted");
    }
    for attr in attributes {
        if attr.name.starts_with("v-") || attr.name.starts_with('@') || attr.name.starts_with(':') {
            continue;
        }
        rendered.push(' ');
        rendered.push_str(&attr.name);
        if let Some(value) = &attr.value {
            rendered.push_str("=\"");
            rendered.push_str(value);
            rendered.push('"');
        }
    }
    if let Some(scope_id) = &options.scope_id {
        rendered.push(' ');
        rendered.push_str(scope_id);
    }
    if self_closing {
        rendered.push_str("/>");
    } else {
        rendered.push('>');
    }
    rendered
}

fn render_ssr_children(
    ast: &vuec_ast::AstDocument<Vue3NodeKind>,
    children: &[vuec_ast::NodeId],
    has_slot: bool,
    options: &SsrCompilerOptions,
    writer: &mut CodeWriter,
) {
    for child_id in children {
        render_ssr_node(ast, *child_id, has_slot, options, writer);
    }
}

fn render_ssr_node(
    ast: &vuec_ast::AstDocument<Vue3NodeKind>,
    node_id: vuec_ast::NodeId,
    has_slot: bool,
    options: &SsrCompilerOptions,
    writer: &mut CodeWriter,
) {
    let Some(node) = ast.node(node_id) else {
        return;
    };
    match &node.kind {
        Vue3AstKind::Element(element) => {
            let tag = &element.tag;
            let attributes = element.template_attributes();
            if tag == "slot" && has_slot {
                writer.push_line(
                    "_ssrRenderSlot(_ctx.$slots, \"default\", {}, null, _push, _parent);",
                );
                return;
            }
            let rendered = render_start_tag(tag, &attributes, element.self_closing, options);
            writer.push_line(&format!("_push({rendered:?});"));
            if !element.self_closing {
                render_ssr_children(ast, &node.children, has_slot, options, writer);
                writer.push_line(&format!("_push({:?});", format!("</{tag}>")));
            }
        }
        Vue3AstKind::Text(text) => {
            writer.push_line(&format!("_push({:?});", text.value));
        }
        Vue3AstKind::Interpolation(interpolation) => {
            let expression = interpolation.expression.source_string();
            writer.push_line(&format!("_push(_ssrInterpolate({expression}));"));
        }
        Vue3AstKind::Comment(comment) => {
            writer.push_line(&format!(
                "_push({:?});",
                format!("<!--{}-->", comment.value)
            ));
        }
        Vue3AstKind::Root(_) => {
            render_ssr_children(ast, &node.children, has_slot, options, writer);
        }
        _ => {}
    }
}

fn is_component(tag: &str) -> bool {
    tag.chars().next().is_some_and(char::is_uppercase)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vuec_source::FileId;

    #[test]
    fn compiles_ssr_render_function() {
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: "<div>{{ msg }}</div><Teleport/>".into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            SsrCompilerOptions::default(),
        );
        assert!(result.code.contains("function ssrRender"));
        assert!(result.code.contains("_ssrInterpolate(msg)"));
        assert!(result.ast_summary.contains("teleports=1"));
    }

    #[test]
    fn scope_and_slotted_are_emitted() {
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: "<div class=\"a\"/>".into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            SsrCompilerOptions {
                scope_id: Some("data-v-x".into()),
                slotted: true,
                ..SsrCompilerOptions::default()
            },
        );
        assert!(result.code.contains("data-v-x"));
        assert!(result.code.contains("data-vuec-slotted"));
    }
}
