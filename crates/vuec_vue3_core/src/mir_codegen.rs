use crate::*;

pub(crate) fn render_helpers_from_code(order: &[RuntimeHelper], code: &str) -> Vec<RuntimeHelper> {
    let mut helpers = order
        .iter()
        .copied()
        .filter(|helper| code.contains(&helper_reference(*helper)))
        .collect::<Vec<_>>();
    apply_vue3_memo_helper_order(&mut helpers);
    helpers
}

pub(crate) struct Vue3DomMirCodegen<'a> {
    pub(crate) mir: &'a Vue3DomMir,
    pub(crate) js: &'a JsAstStore,
    pub(crate) options: &'a Vue3CompilerOptions,
}

include!("mir_codegen_parts/dom_entry.rs");
include!("mir_codegen_parts/dom_props_directives.rs");
include!("mir_codegen_parts/dom_slots.rs");
include!("mir_codegen_parts/dom_control_flow.rs");
include!("mir_codegen_parts/dom_js_scope.rs");

pub(crate) struct Vue3SsrMirCodegen<'a> {
    pub(crate) mir: &'a Vue3SsrMir,
    pub(crate) js: &'a JsAstStore,
    pub(crate) options: &'a Vue3CompilerOptions,
}

#[derive(Clone, Debug)]
pub(crate) struct SsrRootAttrs {
    pub(crate) attrs: Option<String>,
    pub(crate) css_vars: Option<String>,
    pub(crate) target_start: Option<usize>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SsrRootSpan {
    pub(crate) start: usize,
    pub(crate) attrs_index: Option<usize>,
}

include!("mir_codegen_parts/ssr_entry.rs");
include!("mir_codegen_parts/ssr_root_attrs.rs");
include!("mir_codegen_parts/ssr_helpers.rs");
include!("mir_codegen_parts/ssr_render_tree.rs");
include!("mir_codegen_parts/ssr_template_collect.rs");
include!("mir_codegen_parts/ssr_nodes_components.rs");
include!("mir_codegen_parts/ssr_component_props.rs");
include!("mir_codegen_parts/ssr_builtins_slots.rs");
include!("mir_codegen_parts/ssr_attrs.rs");
include!("mir_codegen_parts/ssr_control_flow.rs");
include!("mir_codegen_parts/ssr_dom_props.rs");
include!("mir_codegen_parts/ssr_js_scope.rs");

include!("mir_codegen_parts/ssr_template_helpers.rs");
