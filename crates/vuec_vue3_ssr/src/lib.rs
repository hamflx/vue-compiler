#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use vuec_ast::{Vue3Ast, Vue3AstKind, Vue3ImportItem, Vue3NodeKind};
use vuec_diagnostics::Diagnostic;
use vuec_vue3_asset::transform_asset_url_props;
pub use vuec_vue3_asset::AssetUrlOptions;
use vuec_vue3_core::{
    generate_vue3_ssr_mir, lower_vue3_ast_to_ssr_mir, source_map_for_render, CodegenResult,
    TemplateSource, Vue3CompilerOptions, Vue3Dialect,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SsrCompilerOptions {
    pub core: Vue3CompilerOptions,
    pub scope_id: Option<String>,
    pub slotted: bool,
    pub transform_asset_urls: bool,
    pub asset_url_options: AssetUrlOptions,
}

impl Default for SsrCompilerOptions {
    fn default() -> Self {
        Self {
            core: Vue3CompilerOptions::default(),
            scope_id: None,
            slotted: false,
            transform_asset_urls: true,
            asset_url_options: AssetUrlOptions::default(),
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
    pub diagnostics: Vec<Diagnostic>,
    pub preamble: String,
}

pub fn compile(source: TemplateSource, options: SsrCompilerOptions) -> SsrCompileResult {
    let mut options = options;
    options.core.ssr = true;
    if options.core.scope_id.is_none() {
        options.core.scope_id = options.scope_id.clone();
    }
    if options.slotted {
        options.core.slotted = true;
    }
    let mut ast = Vue3Dialect::base_parse(source.clone(), &options.core);
    if options.transform_asset_urls {
        transform_ssr_asset_urls(&mut ast, &options);
    }
    let summary = summarize_ssr(&ast.nodes.iter().map(|node| &node.kind).collect::<Vec<_>>());
    let mut generated = generate_mir_ssr(&ast, &options);
    if options.core.source_map {
        generated.map = source_map_for_render(&generated.code, &ast, &source, &options.core);
    }
    SsrCompileResult {
        code: generated.code,
        map: generated.map,
        ast_summary: format!(
            "ssr:elements={},interpolations={},components={},slots={},teleports={},suspenses={}",
            summary.elements,
            summary.interpolations,
            summary.components,
            summary.slots,
            summary.teleports,
            summary.suspenses
        ),
        diagnostics: generated.diagnostics,
        preamble: generated.preamble,
    }
}

fn transform_ssr_asset_urls(ast: &mut Vue3Ast, options: &SsrCompilerOptions) {
    let mut asset_imports = Vec::<Vue3ImportItem>::new();
    for node in &mut ast.nodes {
        if let Vue3AstKind::Element(element) = &mut node.kind {
            transform_asset_url_props(
                &element.tag,
                &mut element.props,
                &options.asset_url_options,
                options.core.mode == "module",
                &mut asset_imports,
            );
        }
    }
    if asset_imports.is_empty() {
        return;
    }
    if let Some(root_node) = ast.root_node_mut() {
        if let Vue3AstKind::Root(root) = &mut root_node.kind {
            root.imports = asset_imports;
        }
    }
}

fn generate_mir_ssr(ast: &Vue3Ast, options: &SsrCompilerOptions) -> CodegenResult {
    let lowering = lower_vue3_ast_to_ssr_mir(ast, &options.core);
    generate_vue3_ssr_mir(&lowering.mir, &lowering.js, &options.core)
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

    #[test]
    fn compile_transforms_asset_urls_to_imports_in_module_mode() {
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: r#"<img src="./logo.png" srcset="./logo.png 2x">"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            SsrCompilerOptions {
                core: Vue3CompilerOptions {
                    mode: "module".into(),
                    prefix_identifiers: true,
                    ..Vue3CompilerOptions::default()
                },
                ..SsrCompilerOptions::default()
            },
        );

        assert!(result.code.contains("import _imports_0 from './logo.png'"));
        assert!(result.code.contains("src: _imports_0"));
        assert!(result.code.contains("srcset: _imports_0 + ' 2x'"));
        assert!(result.code.contains("_attrs"));
        assert!(result.code.contains("_ssrRenderAttrs(_mergeProps("));
        assert!(!result.code.contains("_ctx._imports_"));
        assert!(result.ast_summary.contains("elements=1"));
    }

    #[test]
    fn compile_respects_disabled_asset_url_transform() {
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: r#"<img src="./logo.png">"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            SsrCompilerOptions {
                core: Vue3CompilerOptions {
                    mode: "module".into(),
                    prefix_identifiers: true,
                    ..Vue3CompilerOptions::default()
                },
                transform_asset_urls: false,
                ..SsrCompilerOptions::default()
            },
        );

        assert!(!result.code.contains("import _imports_0"));
        assert!(result.code.contains(r#"src: "./logo.png""#));
        assert!(result.code.contains("_attrs"));
    }

    #[test]
    fn compile_keeps_scope_and_slotted_on_asset_import_mir_path() {
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: r#"<img src="./logo.png">"#.into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            SsrCompilerOptions {
                core: Vue3CompilerOptions {
                    mode: "module".into(),
                    prefix_identifiers: true,
                    scope_id: Some("data-v-x".into()),
                    slotted: true,
                    ..Vue3CompilerOptions::default()
                },
                scope_id: Some("data-v-x".into()),
                slotted: true,
                ..SsrCompilerOptions::default()
            },
        );

        assert!(result.code.contains("data-v-x"));
        assert!(result.code.contains("data-vuec-slotted"));
    }
}
