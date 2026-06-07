//! Vue 3 SSR compiler facade.
//!
//! This crate wraps the Vue 3 core parser/lowering/codegen path with public SSR
//! defaults, asset URL handling, source maps, and compact transform summaries.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use vuec_ast::{Vue3Ast, Vue3AstKind, Vue3ImportItem, Vue3NodeKind};
use vuec_diagnostics::Diagnostic;
use vuec_vue3_asset::transform_asset_url_props;
/// Asset URL transform options re-exported for SSR compiler callers.
pub use vuec_vue3_asset::AssetUrlOptions;
use vuec_vue3_core::{
    generate_vue3_ssr_mir, lower_vue3_ast_to_ssr_mir, source_map_for_render,
    vue3_parser_diagnostics, CodegenResult, TemplateSource, Vue3CompilerOptions, Vue3Dialect,
};

/// Options for the Vue 3 SSR compiler facade.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SsrCompilerOptions {
    /// Shared Vue 3 compiler-core options.
    pub core: Vue3CompilerOptions,
    /// Optional scope id used by public SSR compile defaults.
    pub scope_id: Option<String>,
    /// Whether slotted scope markers should be emitted.
    pub slotted: bool,
    /// Whether `slotted` was explicitly provided by the caller.
    pub slotted_is_explicit: bool,
    /// Whether `core.mode` was explicitly provided by the caller.
    pub mode_is_explicit: bool,
    /// Whether static asset URL attributes should be transformed.
    pub transform_asset_urls: bool,
    /// Asset URL transform options.
    pub asset_url_options: AssetUrlOptions,
}

impl Default for SsrCompilerOptions {
    fn default() -> Self {
        Self {
            core: Vue3CompilerOptions::default(),
            scope_id: None,
            slotted: false,
            slotted_is_explicit: false,
            mode_is_explicit: false,
            transform_asset_urls: true,
            asset_url_options: AssetUrlOptions::default(),
        }
    }
}

/// Compact summary of SSR-relevant template nodes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SsrTransformSummary {
    /// Number of element nodes.
    pub elements: usize,
    /// Number of interpolation nodes.
    pub interpolations: usize,
    /// Number of component-like element nodes.
    pub components: usize,
    /// Number of slot outlet nodes.
    pub slots: usize,
    /// Number of teleport nodes.
    pub teleports: usize,
    /// Number of suspense nodes.
    pub suspenses: usize,
}

/// Public SSR compile result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SsrCompileResult {
    /// Generated SSR render code.
    pub code: String,
    /// Optional source map.
    pub map: Option<vuec_codegen::SourceMapArtifact>,
    /// Public AST helper names observed by official SSR compile callers.
    pub ast_helpers: Vec<String>,
    /// Compact AST/SSR transform summary.
    pub ast_summary: String,
    /// Diagnostics produced during compilation.
    pub diagnostics: Vec<Diagnostic>,
    /// Generated helper/import preamble.
    pub preamble: String,
}

/// Compiles a Vue 3 template into an SSR render function.
pub fn compile(source: TemplateSource, options: SsrCompilerOptions) -> SsrCompileResult {
    let mut options = options;
    normalize_public_ssr_compile_options(&mut options);
    let mut ast = Vue3Dialect::base_parse(source.clone(), &options.core);
    if options.transform_asset_urls {
        transform_ssr_asset_urls(&mut ast, &options);
    }
    let summary = summarize_ssr(&ast.nodes.iter().map(|node| &node.kind).collect::<Vec<_>>());
    let mut generated = generate_mir_ssr(&ast, &options);
    if options.core.source_map {
        generated.map = source_map_for_render(&generated.code, &ast, &source, &options.core);
    }
    let mut diagnostics = vue3_parser_diagnostics(&ast);
    diagnostics.extend(generated.diagnostics);
    SsrCompileResult {
        ast_helpers: vue3_ssr_public_ast_helpers(&generated.code, &generated.preamble),
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
        diagnostics,
        preamble: generated.preamble,
    }
}

fn normalize_public_ssr_compile_options(options: &mut SsrCompilerOptions) {
    options.core.ssr = true;
    options.core.prefix_identifiers = true;
    options.core.cache_handlers = false;
    options.core.hoist_static = false;
    if options.mode_is_explicit && options.core.mode == "function" {
        options.core.scope_id = None;
    } else if options.core.scope_id.is_none() {
        options.core.scope_id = options.scope_id.clone();
    }
    if options.slotted || (options.scope_id.is_some() && !options.slotted_is_explicit) {
        options.core.slotted = true;
    } else if options.slotted_is_explicit {
        options.core.slotted = options.slotted;
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

fn vue3_ssr_public_ast_helpers(code: &str, preamble: &str) -> Vec<String> {
    let probe = format!("{preamble}\n{code}");
    const VUE_HELPERS: &[&str] = &[
        "resolveDirective",
        "withDirectives",
        "setBlockTracking",
        "openBlock",
        "createElementVNode",
        "createElementBlock",
        "createCommentVNode",
        "createTextVNode",
        "BaseTransition",
        "Transition",
        "TransitionGroup",
        "Teleport",
        "Suspense",
        "KeepAlive",
        "Fragment",
        "toDisplayString",
        "renderList",
        "renderSlot",
        "normalizeClass",
        "normalizeProps",
        "normalizeStyle",
        "guardReactiveProps",
        "mergeProps",
        "resolveComponent",
        "resolveDynamicComponent",
        "withCtx",
        "createBlock",
        "createVNode",
        "createSlots",
        "createStaticVNode",
        "isMemoSame",
        "withMemo",
        "toHandlers",
        "camelize",
        "capitalize",
        "toHandlerKey",
        "pushScopeId",
        "popScopeId",
        "unref",
        "isRef",
    ];
    VUE_HELPERS
        .iter()
        .copied()
        .filter(|helper| probe.contains(&format!("_{helper}")))
        .map(ToOwned::to_owned)
        .collect()
}

/// Summarizes SSR-relevant nodes from a Vue 3 AST node-kind slice.
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
        assert!(result.code.contains("_ssrInterpolate(_ctx.msg)"));
        assert!(!result.code.contains("with (_ctx)"));
        assert!(result.ast_summary.contains("teleports=1"));
    }

    #[test]
    fn compile_uses_official_public_ssr_defaults() {
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: "<div>{{ msg }}</div>".into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            SsrCompilerOptions {
                core: Vue3CompilerOptions {
                    prefix_identifiers: false,
                    cache_handlers: true,
                    hoist_static: true,
                    scope_id: Some("data-v-x".into()),
                    ..Vue3CompilerOptions::default()
                },
                scope_id: Some("data-v-x".into()),
                ..SsrCompilerOptions::default()
            },
        );

        assert!(!result.code.contains("with (_ctx)"));
        assert!(result.code.contains("_ssrInterpolate(_ctx.msg)"));
        assert!(result.code.contains("_ssrRenderAttrs(_attrs)"));
        assert!(result.code.contains("data-v-x"));
        assert!(!result.code.contains("_hoisted_"));
        assert!(!result.code.contains("_cache["));
    }

    #[test]
    fn compile_ignores_scope_id_for_explicit_function_mode() {
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: "<div class=\"a\"></div>".into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            SsrCompilerOptions {
                core: Vue3CompilerOptions {
                    mode: "function".into(),
                    scope_id: Some("data-v-ignored".into()),
                    ..Vue3CompilerOptions::default()
                },
                scope_id: Some("data-v-ignored".into()),
                mode_is_explicit: true,
                ..SsrCompilerOptions::default()
            },
        );

        assert!(!result.code.contains("data-v-ignored"));
        assert!(result.code.contains("_ssrRenderAttrs(_mergeProps("));
    }

    #[test]
    fn module_scope_and_slotted_are_emitted() {
        let result = compile(
            TemplateSource {
                filename: "x.vue".into(),
                source: "<div class=\"a\"/>".into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            SsrCompilerOptions {
                core: Vue3CompilerOptions {
                    mode: "module".into(),
                    ..Vue3CompilerOptions::default()
                },
                scope_id: Some("data-v-x".into()),
                slotted: true,
                ..SsrCompilerOptions::default()
            },
        );
        assert!(result.code.contains("data-v-x"));
        assert!(!result.code.contains("data-vuec-slotted"));
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
        assert!(!result.code.contains("data-vuec-slotted"));
    }

    #[test]
    fn compile_includes_core_structural_parser_diagnostics() {
        let result = compile(
            TemplateSource {
                filename: "bad.vue".into(),
                source: "<div><span></div>".into(),
                file_id: FileId(0),
                base_offset: 0,
            },
            SsrCompilerOptions::default(),
        );

        assert!(result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "24"
                && diagnostic.message == "Element is missing end tag."));
    }
}
