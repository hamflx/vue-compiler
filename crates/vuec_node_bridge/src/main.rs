//! JSON stdin/stdout bridge used by generated Node package aliases.
//!
//! This binary is an API/import adapter and test-runner support boundary. It
//! hydrates JSON requests from generated JavaScript aliases, calls Rust compiler
//! crates, and serializes public projection results back to Node without making
//! the bridge itself the source of compiler semantics.

#![forbid(unsafe_code)]

use anyhow::{bail, Context, Result};
use oxc_ast::ast::{
    Argument, ArrayExpressionElement, AssignmentTarget, BindingPattern, ChainElement, Expression,
    FormalParameter, ObjectPropertyKind, PropertyKey, SimpleAssignmentTarget, Statement,
};
use oxc_span::SourceType;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::io::{self, Read};
use vuec_ast::{NodeSpan, Vue3Ast, Vue3AstKind, Vue3Expression, Vue3ImportItem, Vue3Prop};
use vuec_bridge_registry::bridge_command;
use vuec_html::{HtmlTokenKind, HtmlTokenizer};
use vuec_js::JsAstStore;
use vuec_sfc::{
    SfcAttrValue, SfcBlock, SfcBlockAttrs, SfcCompiler, SfcDescriptor, SfcPropsDestructureMode,
    SfcScriptAstMode, SfcScriptBlock, SfcScriptCompileOptions, SfcStyleCompileOptions,
    SfcTemplateCompileOptions, Vue27ParseComponentOptions, Vue27PrefixIdentifiersOptions,
    Vue27RewriteDefaultOptions, Vue27SfcPad, Vue27TemplatePreprocessOptions,
    Vue3RewriteDefaultOptions, Vue3SfcPad, Vue3SfcParseOptions, Vue3SfcParseProjectionOptions,
    Vue3TemplatePreprocessOptions,
};
use vuec_source::FileId;
use vuec_style::{
    compile_style, gen_css_var_name_with_style, CssVarNameStyle, StyleCompileOptions,
};
use vuec_vue2::{
    self, Vue2CompileOptions, Vue2CompiledResult, Vue2Element, Vue2Error,
    Vue2SfcAssetUrlTransformOptions, Vue2Warning,
};
use vuec_vue3_core::{TemplateSource, Vue3CompilerOptions, Vue3Dialect};
use vuec_vue3_dom::{self, AssetUrlOptions, DomCompilerOptions};
use vuec_vue3_ssr::{self, SsrCompilerOptions};

fn main() {
    if let Err(err) = run() {
        eprintln!("{err:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let command = std::env::args()
        .nth(1)
        .context("missing bridge command argument")?;
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .context("failed to read bridge stdin")?;
    let payload = if input.trim().is_empty() {
        Value::Null
    } else {
        serde_json::from_str(&input).context("failed to parse bridge JSON payload")?
    };
    let output = dispatch(&command, payload)?;
    println!("{}", serde_json::to_string(&output)?);
    Ok(())
}

mod dispatch;
use dispatch::dispatch;

mod payload;
use payload::*;

mod vue2_projection;
use vue2_projection::*;

mod sfc_projection;
use sfc_projection::*;

mod vue3_bridge;
use vue3_bridge::*;

mod options;
use options::*;

#[cfg(test)]
mod tests;
