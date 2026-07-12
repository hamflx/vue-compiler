//! Native Node.js bindings for the Rust Vue compiler.
//!
//! This crate exposes the release-facing NAPI ABI used by `@vuec-rs/native`
//! and the official package-name aliases. Public functions serialize compiler
//! results as JSON strings so the JavaScript loader can project them into the
//! expected package API shapes.

#![deny(missing_docs)]
#![deny(unsafe_code)]

use napi::{
    bindgen_prelude::{FromNapiValue, JsObjectValue, Object, Unknown},
    Env, JsValue, Result, Status, ValueType,
};
use napi_derive::napi;
use oxc_ast::ast::{
    Argument, ArrayExpressionElement, AssignmentTarget, BindingPattern, ChainElement, Expression,
    FormalParameter, ObjectPropertyKind, PropertyKey, SimpleAssignmentTarget, Statement,
};
use oxc_span::SourceType;
use serde_json::Map;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use vuec_ast::{NodeSpan, Vue3Ast, Vue3AstKind, Vue3Expression, Vue3Prop};
use vuec_html::{HtmlTokenKind, HtmlTokenizer};
use vuec_js::JsAstStore;
use vuec_sfc::{
    SfcCompiler, SfcCssVarNameStyle, SfcPropsDestructureMode, SfcScriptAstMode,
    SfcScriptCompileOptions, SfcStyleCompileOptions, SfcTemplateCompileOptions,
    Vue27ParseComponentOptions, Vue27PrefixIdentifiersOptions, Vue27RewriteDefaultOptions,
    Vue27SfcPad, Vue27TemplatePreprocessOptions, Vue3RewriteDefaultOptions, Vue3SfcPad,
    Vue3SfcParseOptions, Vue3SfcParseProjectionOptions,
};
use vuec_source::{FileId, Span};
use vuec_vue2::{
    Vue2CompileOptions, Vue2CompiledResult, Vue2Element, Vue2Error,
    Vue2SfcAssetUrlTransformOptions, Vue2Warning,
};
use vuec_vue3_core::{TemplateSource, Vue3CompilerOptions, Vue3Dialect};
use vuec_vue3_dom::{
    apply_dom_parser_defaults, compile as compile_dom, parse as parse_dom, AssetUrlOptions,
    DomCompilerOptions,
};
use vuec_vue3_ssr::{compile as compile_ssr, SsrCompilerOptions};

include!("napi_parts/exports.rs");
include!("napi_parts/common_options.rs");
include!("napi_parts/vue2_projection.rs");
include!("napi_parts/sfc_options.rs");
include!("napi_parts/sfc_projection.rs");
include!("napi_parts/parse_diagnostics.rs");
include!("napi_parts/vue3_projection.rs");
include!("napi_parts/js_ast_projection.rs");
include!("napi_parts/loc_options_manifest_tests.rs");
