//! Vue single-file component compiler implementation.
//!
//! This crate owns SFC descriptor parsing, Vue 2.7 `parseComponent`
//! projection, Vue 3 template/script/style compile entry points, Vue 2.7
//! SFC helper APIs, descriptor caching, and source-map/error shapes shared by
//! the CLI, NAPI, WASM, and package-alias layers.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

use oxc_ast::ast::{
    Argument, ArrayExpressionElement, ArrowFunctionExpression, AssignmentTarget, BindingPattern,
    ClassElement, Declaration, ExportDefaultDeclaration, ExportDefaultDeclarationKind,
    ExportNamedDeclaration, ExportSpecifier, Expression, ForStatementInit, ForStatementLeft,
    FormalParameter, FormalParameters, Function, FunctionBody, ImportDeclarationSpecifier,
    ImportExpression, ImportOrExportKind, ModuleExportName, ObjectExpression, ObjectProperty,
    ObjectPropertyKind, PropertyKey, PropertyKind, SimpleAssignmentTarget, Statement,
    TSEnumDeclaration, TSExternalModuleReference, TSFunctionType, TSImportType,
    TSImportTypeQualifier, TSInterfaceBody, TSInterfaceDeclaration, TSInterfaceHeritage, TSLiteral,
    TSMappedType, TSMappedTypeModifierOperator, TSModuleDeclaration, TSModuleDeclarationBody,
    TSModuleDeclarationName, TSSignature, TSTemplateLiteralType, TSTupleElement, TSType,
    TSTypeAliasDeclaration, TSTypeAnnotation, TSTypeLiteral, TSTypeName, TSTypeOperatorOperator,
    TSTypeQuery, TSTypeQueryExprName, TSTypeReference, VariableDeclaration,
    VariableDeclarationKind, VariableDeclarator, WithClauseKeyword, WithStatement,
};
use oxc_span::GetSpan;
use serde::de::{IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{json, Value};
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use vuec_ast::JsProgramId;
use vuec_codegen::{SourceMapArtifact, SourceMapBuilder};
use vuec_diagnostics::{Diagnostic, Severity};
use vuec_html::{
    decode_html_attr_entities, HtmlAttribute, HtmlQuoteKind, HtmlTokenKind, HtmlTokenizer,
};
use vuec_js::{JsAstStore, JsParseMode};
use vuec_source::{FileId, SourceMap, Span};
pub use vuec_style::CssVarNameStyle as SfcCssVarNameStyle;
use vuec_style::{
    collect_css_vars_with_options, compile_style, gen_css_var_name_with_style, CssModulesOptions,
    CssVarCollectOptions, CssVarNameStyle, StyleCompileOptions, StylePreprocessOptions,
};
use vuec_vue3_core::{process_expression_projection, TemplateSource, Vue3CompilerOptions};
use vuec_vue3_dom::{
    apply_dom_parser_defaults, compile as compile_dom, AssetUrlOptions, DomCompilerOptions,
};
use vuec_vue3_ssr::{compile as compile_ssr, SsrCompilerOptions};

mod types;
pub use types::*;

mod context;
pub use context::SfcScriptAstMode;
pub(crate) use context::*;

mod compiler;

mod descriptor;
pub(crate) use descriptor::*;
pub use descriptor::{
    vue3_sfc_descriptor_value, vue3_sfc_parse_diagnostics, vue3_sfc_parse_result_value,
};

mod rewrite;
pub(crate) use rewrite::*;

mod style;
pub(crate) use style::*;

mod script_types;
pub(crate) use script_types::*;

mod template;
pub(crate) use template::*;

mod script_compile;
pub(crate) use script_compile::*;

mod script_ast;
pub(crate) use script_ast::*;

#[cfg(test)]
mod tests;
