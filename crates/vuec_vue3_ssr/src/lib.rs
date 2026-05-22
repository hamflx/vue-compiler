#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use vuec_ast::{TemplateAttribute, Vue3NodeKind};
use vuec_codegen::{CodeWriter, SourceMapBuilder};
use vuec_vue3_core::{CodegenResult, TemplateSource, Vue3CompilerOptions, Vue3Dialect};

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

pub fn compile(source: TemplateSource, options: SsrCompilerOptions) -> CodegenResult {
    let ast = Vue3Dialect::base_parse(source, &options.core);
    let summary = summarize_ssr(&ast.nodes.iter().map(|node| &node.kind).collect::<Vec<_>>());
    let mut writer = CodeWriter::new();
    writer.push_line("function ssrRender(_ctx, _push, _parent, _attrs) {");
    writer.indent();
    for node in &ast.nodes {
        match &node.kind {
            Vue3NodeKind::Element {
                tag,
                attributes,
                self_closing,
            } => {
                writer.push_line(&format!(
                    "_push({:?});",
                    render_start_tag(tag, attributes, *self_closing, &options)
                ));
            }
            Vue3NodeKind::Text { value } => {
                writer.push_line(&format!("_push({value:?});"));
            }
            Vue3NodeKind::Interpolation { expression } => {
                writer.push_line(&format!("_push(_ssrInterpolate({expression}));"));
            }
            Vue3NodeKind::Comment { value } => {
                writer.push_line(&format!("_push({:?});", format!("<!--{value}-->")));
            }
            Vue3NodeKind::Directive { .. } | Vue3NodeKind::Root => {}
        }
    }
    writer.dedent();
    writer.push_line("}");
    CodegenResult {
        code: writer.finish(),
        map: Some(SourceMapBuilder::new().file("ssr.js").build()),
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
            Vue3NodeKind::Element { tag, .. } => {
                summary.elements += 1;
                if is_component(tag) {
                    summary.components += 1;
                }
                match tag.as_str() {
                    "slot" => summary.slots += 1,
                    "teleport" | "Teleport" => summary.teleports += 1,
                    "suspense" | "Suspense" => summary.suspenses += 1,
                    _ => {}
                }
            }
            Vue3NodeKind::Interpolation { .. } => summary.interpolations += 1,
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
    if let Some(scope_id) = &options.scope_id {
        rendered.push(' ');
        rendered.push_str(scope_id);
    }
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
    if self_closing {
        rendered.push_str("/>");
    } else {
        rendered.push('>');
    }
    rendered
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
