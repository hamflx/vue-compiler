//! Vue 3 compiler-core implementation.
//!
//! This crate owns Vue 3 template parsing, transform orchestration, exact
//! render-code generation, structural AST -> HIR -> target MIR lowering, and
//! Rust-backed public projection helpers used by the compatibility bridge.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};

use oxc_ast::ast::{
    ArrayExpressionElement, BinaryOperator, BindingPattern, ChainElement, Expression,
    ObjectPropertyKind, PropertyKey, PropertyKind, UnaryOperator,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use vuec_ast::{
    Hir, HirBinding, HirConstness, HirDirectiveUse, HirElement, HirEvent, HirExpr, HirFor,
    HirFragment, HirIf, HirIfBranch, HirInterpolation, HirNodeKind, HirObjectBinding,
    HirObjectListeners, HirPropSegment, HirProps, HirRoot, HirSlotDecl, HirSlotOutlet,
    HirStaticAttr, HirTag, JsExprId, JsPatternId, LoweringMap, MirChildren, MirExpr,
    MissingSpanReason, NodeId, NodeSpan, QuoteKind, RuntimeHelper, Vue3Ast, Vue3AstKind,
    Vue3Directive, Vue3DomBinding, Vue3DomClickEvent, Vue3DomContent, Vue3DomDirective,
    Vue3DomEvent, Vue3DomEventCache, Vue3DomMir, Vue3DomMirKind, Vue3DomModel, Vue3DomModelKind,
    Vue3DomObjectBinding, Vue3DomObjectListeners, Vue3DomPropSegment, Vue3DomProps,
    Vue3DomPropsNormalize, Vue3DomRoot, Vue3DomSlotName, Vue3DomStaticAttr, Vue3DomTag,
    Vue3Element, Vue3ElementType, Vue3Expression, Vue3ForMemo, Vue3ForMir, Vue3NodeKind,
    Vue3ParserDiagnostic, Vue3PatchFlags, Vue3Prop, Vue3Root, Vue3SlotFlag, Vue3SsrAttrs,
    Vue3SsrComponent, Vue3SsrContent, Vue3SsrFor, Vue3SsrMir, Vue3SsrMirKind, Vue3SsrModel,
    Vue3SsrModelKind, Vue3SsrRoot, Vue3SsrSuspense, Vue3SsrTeleport, Vue3VNodeCall,
};
use vuec_codegen::{CodeWriter, SourceMapArtifact, SourceMapSegment};
use vuec_diagnostics::{Diagnostic, Vue3ErrorCode};
pub use vuec_html::find_matching_raw_text_end;
use vuec_html::{
    decode_html_attr_entities, decode_html_text_entities, raw_text_mode_for_tag,
    resolve_html_namespace, HtmlTextMode, HtmlTokenKind, HtmlTokenizer,
};
use vuec_js::{
    js_error_to_vue3_invalid_expression_diagnostic,
    js_program_errors_to_vue3_invalid_expression_diagnostic, JsAstStore,
};
use vuec_pass::TransformContext;
use vuec_source::{FileId, Span};

mod types;
pub use types::*;

mod dialect;
pub use dialect::Vue3Dialect;

mod lowering;
pub(crate) use lowering::*;
pub use lowering::{lower_vue3_ast_to_dom_mir, lower_vue3_ast_to_ssr_mir, root_codegen_projection};

mod projection;
pub(crate) use projection::*;
pub use projection::{
    advance_position_with_clone_projection, advance_position_with_mutation_projection,
    build_directive_args_projection, build_slots_projection, cache_static_projection,
    extract_identifiers_projection, get_constant_type_projection, is_function_type_projection,
    is_in_destructure_assignment_projection, is_member_expression_projection,
    is_referenced_identifier_projection, is_static_property_projection, model_is_member_expression,
    process_expression_projection, resolve_component_type_projection, stringify_static_projection,
    to_valid_asset_id_projection, track_slot_scopes_projection, track_v_for_slot_scopes_projection,
    transform_bind_projection, transform_element_children_projection,
    transform_element_props_projection, transform_expression_projection, transform_for_projection,
    transform_if_projection, transform_memo_projection, transform_model_projection,
    transform_on_projection, transform_once_projection, transform_slot_outlet_projection,
    transform_text_projection, transform_v_bind_shorthand_projection, vue3_raw_text_kind,
    walk_identifiers_projection,
};

mod mir_codegen;
pub(crate) use mir_codegen::*;

mod codegen;
pub(crate) use codegen::*;
pub use codegen::{
    source_map_for_render, vue3_element_codegen_patch_flag, vue3_expression_diagnostics,
    vue3_parser_diagnostics,
};

mod public_codegen;
pub(crate) use public_codegen::*;
pub use public_codegen::{base_compile, compile_dom, compile_ssr, generate_public_ast};

#[cfg(test)]
mod tests;
