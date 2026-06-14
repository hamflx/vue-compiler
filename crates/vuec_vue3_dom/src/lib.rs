//! Vue 3 DOM compiler facade and DOM-specific template normalization.
//!
//! This crate wraps `vuec_vue3_core` with browser DOM defaults, asset URL
//! handling, directive summaries, entity decoding, and an incremental parsed AST
//! cache. It does not own the canonical AST/HIR/MIR schema.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use vuec_ast::{
    HtmlNamespace, NodeId, RuntimeHelper, TemplateAttribute, Vue3Ast, Vue3AstKind, Vue3Element,
    Vue3ElementType, Vue3ImportItem, Vue3Prop, Vue3Root,
};
use vuec_diagnostics::{Diagnostic, Vue3ErrorCode};
use vuec_html::{decode_html_attr_entities, decode_html_text_entities};
use vuec_pass::TransformContext;
use vuec_vue3_asset::transform_asset_url_props;
/// Asset URL transform options re-exported for DOM compiler callers.
pub use vuec_vue3_asset::AssetUrlOptions;
use vuec_vue3_core::{CodegenResult, TemplateSource, Vue3CompilerOptions, Vue3Dialect};

include!("dom_parts/types.rs");
include!("dom_parts/compiler.rs");
include!("dom_parts/projections.rs");
include!("dom_parts/projection_helpers.rs");
include!("dom_parts/directives.rs");
include!("dom_parts/diagnostics.rs");
include!("dom_parts/entities_and_tests.rs");
