//! Native Node.js bindings for the Rust Vue compiler.
//!
//! This crate exposes the release-facing NAPI ABI used by `@vuec-rs/native`
//! and the official package-name aliases. Public functions serialize compiler
//! results as JSON strings so the JavaScript loader can project them into the
//! expected package API shapes.

#![deny(missing_docs)]
#![deny(unsafe_code)]

use napi::{bindgen_prelude::Unknown, Env, Result};
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
    SfcCompiler, SfcCssVarNameStyle, SfcScriptCompileOptions, SfcStyleCompileOptions,
    SfcTemplateCompileOptions, Vue27ParseComponentOptions, Vue27RewriteDefaultOptions, Vue27SfcPad,
    Vue27TemplatePreprocessOptions,
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

#[napi(js_name = "version")]
/// Returns the native package version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[napi(js_name = "compileVue2")]
/// Compiles a Vue 2 template and returns a JSON string result.
pub fn compile_vue2(env: Env, template: String, options: Option<Unknown>) -> Result<String> {
    let options = vue2_options(from_js_options(&env, options)?);
    let compiled = vuec_vue2::compile(&template, options.clone());
    to_json_string(vue2_compile_value(&compiled, &options))
}

#[napi(js_name = "compileToFunctionsVue2")]
/// Compiles a Vue 2 template to function-result fields as a JSON string.
pub fn compile_to_functions_vue2(
    env: Env,
    template: String,
    options: Option<Unknown>,
) -> Result<String> {
    to_json_string(vuec_vue2::compile_to_functions(
        &template,
        vue2_options(from_js_options(&env, options)?),
    ))
}

#[napi(js_name = "compileSsrVue2")]
/// Compiles a Vue 2 template for SSR and returns a JSON string result.
pub fn compile_ssr_vue2(env: Env, template: String, options: Option<Unknown>) -> Result<String> {
    let options = vue2_options(from_js_options(&env, options)?);
    let compiled = vuec_vue2::compile_ssr(&template, options.clone());
    to_json_string(vue2_compile_value(&compiled, &options))
}

#[napi(js_name = "generateCodeFrameVue2")]
/// Generates a Vue 2 compiler code frame.
pub fn generate_code_frame_vue2(source: String, start: u32, end: u32) -> String {
    vuec_vue2::generate_code_frame(&source, start as usize, end as usize)
}

#[napi(js_name = "callVue2Bridge")]
/// Calls Vue 2 Rust compiler bridge operations used by official source tests.
pub fn call_vue2_bridge(env: Env, command: String, payload: Unknown) -> Result<String> {
    let payload = from_js_options(&env, Some(payload))?;
    match command.as_str() {
        "vue2.generate" => {
            let options = vue2_options(payload.get("options").cloned().unwrap_or(Value::Null));
            let element = payload
                .get("ast")
                .filter(|ast| !ast.is_null())
                .map(|ast| serde_json::from_value::<Vue2Element>(ast.clone()))
                .transpose()
                .map_err(|err| {
                    napi::Error::from_reason(format!(
                        "failed to deserialize Vue 2 AST element for codegen: {err}"
                    ))
                })?;
            let generated = vuec_vue2::generate(element.as_ref(), &options);
            to_json_string(json!({
                "render": generated.render,
                "staticRenderFns": generated.static_render_fns,
                "static_render_fns": generated.static_render_fns,
            }))
        }
        "vue2.optimize" => {
            let options = vue2_options(payload.get("options").cloned().unwrap_or(Value::Null));
            let mut element = payload
                .get("ast")
                .filter(|ast| !ast.is_null())
                .map(|ast| serde_json::from_value::<Vue2Element>(ast.clone()))
                .transpose()
                .map_err(|err| {
                    napi::Error::from_reason(format!(
                        "failed to deserialize Vue 2 AST element for optimizer: {err}"
                    ))
                })?;
            if let Some(element) = element.as_mut() {
                vuec_vue2::optimize(element, &options);
            }
            let public = element
                .as_ref()
                .map(vue2_public_element_ast_value)
                .unwrap_or(Value::Null);
            to_json_string(json!({
                "ast": public,
                "ast_public": public,
                "element_public_ast": public,
                "element_ast": element,
            }))
        }
        "vue2.generateCodeFrame" => {
            let source = payload
                .get("source")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let start = payload.get("start").and_then(Value::as_u64).unwrap_or(0) as usize;
            let end = payload
                .get("end")
                .and_then(Value::as_u64)
                .unwrap_or(start as u64) as usize;
            to_json_string(vuec_vue2::generate_code_frame(source, start, end))
        }
        other => Err(napi::Error::from_reason(format!(
            "unsupported Vue 2 bridge command: {other}"
        ))),
    }
}

#[napi(js_name = "rewriteDefaultVue27")]
/// Rewrites a Vue 2.7 default export to an assigned variable.
pub fn rewrite_default_vue27(
    env: Env,
    source: String,
    variable: String,
    parser_plugins: Option<Unknown>,
) -> Result<String> {
    let plugin_options = vue27_rewrite_default_options(from_js_options(&env, parser_plugins)?);
    let compiler = SfcCompiler::new();
    Ok(compiler.rewrite_vue27_default(&source, &variable, plugin_options))
}

#[napi(js_name = "compileVue3Dom")]
/// Compiles a Vue 3 template for DOM rendering and returns a JSON string result.
pub fn compile_vue3_dom(env: Env, source: String, options: Option<Unknown>) -> Result<String> {
    let raw_options = from_js_options(&env, options)?;
    let template = template_source(&source, &raw_options);
    let mut core = vue3_options(Some(&raw_options));
    apply_napi_dom_parser_defaults(&mut core, Some(&raw_options));
    let default_options = DomCompilerOptions::default();
    let dom_options = DomCompilerOptions {
        core,
        transform_asset_urls: transform_asset_urls_enabled(
            &raw_options,
            default_options.transform_asset_urls,
        ),
        asset_url_options: asset_url_options(
            &raw_options,
            default_options.asset_url_options.clone(),
        ),
        ..default_options
    };
    to_json_string(compile_dom(template, dom_options))
}

#[napi(js_name = "parseVue3Dom")]
/// Parses a Vue 3 DOM template and returns public AST JSON.
pub fn parse_vue3_dom(env: Env, source: String, options: Option<Unknown>) -> Result<String> {
    let raw_options = from_js_options(&env, options)?;
    let template = template_source(&source, &raw_options);
    let mut core = vue3_options(Some(&raw_options));
    apply_napi_dom_parser_defaults(&mut core, Some(&raw_options));
    let default_options = DomCompilerOptions::default();
    let dom_options = DomCompilerOptions {
        core,
        transform_asset_urls: transform_asset_urls_enabled(
            &raw_options,
            default_options.transform_asset_urls,
        ),
        asset_url_options: asset_url_options(
            &raw_options,
            default_options.asset_url_options.clone(),
        ),
        ..default_options
    };
    let ast = parse_dom(template.clone(), &dom_options);
    to_json_string(vue3_public_parse_ast(
        &ast,
        &template.source,
        template.base_offset,
        &dom_options.core,
    ))
}

#[napi(js_name = "baseCompileVue3")]
/// Runs the Vue 3 compiler-core `baseCompile` compatible DOM path.
pub fn base_compile_vue3(env: Env, source: String, options: Option<Unknown>) -> Result<String> {
    compile_vue3_dom(env, source, options)
}

#[napi(js_name = "baseParseVue3")]
/// Parses a Vue 3 template through compiler-core and returns public AST JSON.
pub fn base_parse_vue3(env: Env, source: String, options: Option<Unknown>) -> Result<String> {
    let raw_options = from_js_options(&env, options)?;
    let template = template_source(&source, &raw_options);
    let options = vue3_options(Some(&raw_options));
    let ast = Vue3Dialect::base_parse(template.clone(), &options);
    to_json_string(vue3_public_parse_ast(
        &ast,
        &template.source,
        template.base_offset,
        &options,
    ))
}

#[napi(js_name = "generateVue3Core")]
/// Generates Vue 3 render code from a hydrated public AST value.
pub fn generate_vue3_core(env: Env, ast: Unknown, options: Option<Unknown>) -> Result<String> {
    let ast = from_js_options(&env, Some(ast))?;
    let options = vue3_options(Some(&from_js_options(&env, options)?));
    to_json_string(vuec_vue3_core::generate_public_ast(&ast, &options))
}

#[napi(js_name = "callVue3CoreProjection")]
/// Calls Rust-backed Vue 3 compiler-core public projection helpers.
pub fn call_vue3_core_projection(env: Env, command: String, payload: Unknown) -> Result<String> {
    let payload = from_js_options(&env, Some(payload))?;
    let value = match command.as_str() {
        "vue3.core.isMemberExpression" => vuec_vue3_core::is_member_expression_projection(&payload),
        "vue3.core.advancePositionWithClone" => {
            vuec_vue3_core::advance_position_with_clone_projection(&payload)
        }
        "vue3.core.advancePositionWithMutation" => {
            vuec_vue3_core::advance_position_with_mutation_projection(&payload)
        }
        "vue3.core.toValidAssetId" => vuec_vue3_core::to_valid_asset_id_projection(&payload),
        "vue3.core.getConstantType" => vuec_vue3_core::get_constant_type_projection(&payload),
        "vue3.core.cacheStatic" => vuec_vue3_core::cache_static_projection(&payload),
        "vue3.core.rootCodegen" => {
            vuec_vue3_core::root_codegen_projection(payload.get("root").unwrap_or(&payload))
        }
        "vue3.core.transformOnce" => vuec_vue3_core::transform_once_projection(&payload),
        "vue3.core.transformIf" => vuec_vue3_core::transform_if_projection(&payload),
        "vue3.core.transformFor" => vuec_vue3_core::transform_for_projection(&payload),
        "vue3.core.transformExpression" => {
            vuec_vue3_core::transform_expression_projection(&payload)
        }
        "vue3.core.processExpression" => vuec_vue3_core::process_expression_projection(&payload),
        "vue3.core.transformBind" => vuec_vue3_core::transform_bind_projection(&payload),
        "vue3.core.transformVBindShorthand" => {
            vuec_vue3_core::transform_v_bind_shorthand_projection(&payload)
        }
        "vue3.core.transformOn" => vuec_vue3_core::transform_on_projection(&payload),
        "vue3.core.transformModel" => vuec_vue3_core::transform_model_projection(&payload),
        "vue3.core.trackSlotScopes" => vuec_vue3_core::track_slot_scopes_projection(&payload),
        "vue3.core.trackVForSlotScopes" => {
            vuec_vue3_core::track_v_for_slot_scopes_projection(&payload)
        }
        "vue3.core.buildSlots" => vuec_vue3_core::build_slots_projection(&payload),
        "vue3.core.transformSlotOutlet" => {
            vuec_vue3_core::transform_slot_outlet_projection(&payload)
        }
        "vue3.core.resolveComponentType" => {
            vuec_vue3_core::resolve_component_type_projection(&payload)
        }
        "vue3.core.transformElementProps" => {
            vuec_vue3_core::transform_element_props_projection(&payload)
        }
        "vue3.core.transformElementChildren" => {
            vuec_vue3_core::transform_element_children_projection(&payload)
        }
        "vue3.core.transformText" => vuec_vue3_core::transform_text_projection(&payload),
        "vue3.core.buildDirectiveArgs" => vuec_vue3_core::build_directive_args_projection(&payload),
        "vue3.core.isInDestructureAssignment" => {
            vuec_vue3_core::is_in_destructure_assignment_projection(&payload)
        }
        "vue3.core.isReferencedIdentifier" => {
            vuec_vue3_core::is_referenced_identifier_projection(&payload)
        }
        "vue3.core.walkIdentifiers" => vuec_vue3_core::walk_identifiers_projection(&payload),
        other => {
            return Err(napi::Error::from_reason(format!(
                "unsupported Vue 3 compiler-core projection command: {other}"
            )));
        }
    };
    to_json_string(value)
}

#[napi(js_name = "callVue3DomProjection")]
/// Calls Rust-backed Vue 3 compiler-dom public projection helpers.
pub fn call_vue3_dom_projection(env: Env, command: String, payload: Unknown) -> Result<String> {
    let payload = from_js_options(&env, Some(payload))?;
    let value = match command.as_str() {
        "vue3.dom.transformStyle" => vuec_vue3_dom::transform_style_projection(&payload),
        "vue3.dom.ignoreSideEffectTags" => {
            vuec_vue3_dom::ignore_side_effect_tags_projection(&payload)
        }
        "vue3.dom.decodeHtmlBrowser" => vuec_vue3_dom::decode_html_browser_projection(&payload),
        "vue3.dom.transformVHtml" => vuec_vue3_dom::transform_v_html_projection(&payload),
        "vue3.dom.transformVText" => vuec_vue3_dom::transform_v_text_projection(&payload),
        "vue3.dom.transformShow" => vuec_vue3_dom::transform_show_projection(&payload),
        "vue3.dom.transformOn" => vuec_vue3_dom::transform_on_projection(&payload),
        "vue3.dom.transformModel" => vuec_vue3_dom::transform_model_projection(&payload),
        "vue3.dom.transformTransition" => vuec_vue3_dom::transform_transition_projection(&payload),
        "vue3.dom.validateHtmlNesting" => vuec_vue3_dom::validate_html_nesting_projection(&payload),
        "vue3.dom.isValidHTMLNesting" => vuec_vue3_dom::is_valid_html_nesting_projection(&payload),
        other => {
            return Err(napi::Error::from_reason(format!(
                "unsupported Vue 3 compiler-dom projection command: {other}"
            )));
        }
    };
    to_json_string(value)
}

#[napi(js_name = "compileVue3Ssr")]
/// Compiles a Vue 3 template for SSR and returns a JSON string result.
pub fn compile_vue3_ssr(env: Env, source: String, options: Option<Unknown>) -> Result<String> {
    let raw_options = from_js_options(&env, options)?;
    let template = template_source(&source, &raw_options);
    let mut core = vue3_options(Some(&raw_options));
    apply_napi_dom_parser_defaults(&mut core, Some(&raw_options));
    let default_options = SsrCompilerOptions::default();
    let ssr_options = SsrCompilerOptions {
        core,
        scope_id: raw_options
            .get("scopeId")
            .or_else(|| raw_options.get("scope_id"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        slotted: bool_option(&raw_options, "slotted", false),
        slotted_is_explicit: raw_options.get("slotted").is_some(),
        mode_is_explicit: raw_options.get("mode").is_some(),
        transform_asset_urls: transform_asset_urls_enabled(
            &raw_options,
            default_options.transform_asset_urls,
        ),
        asset_url_options: asset_url_options(
            &raw_options,
            default_options.asset_url_options.clone(),
        ),
        ..default_options
    };
    to_json_string(compile_ssr(template, ssr_options))
}

#[napi(js_name = "parseSfc")]
/// Parses a Vue SFC descriptor and returns it as JSON.
pub fn parse_sfc(env: Env, source: String, options: Option<Unknown>) -> Result<String> {
    let raw_options = from_js_options(&env, options)?;
    let filename = string_option(&raw_options, "filename", "anonymous.vue");
    let mut compiler = SfcCompiler::new();
    to_json_string(compiler.parse(filename, &source))
}

#[napi(js_name = "parseVue27SfcComponent")]
/// Parses a Vue 2.7 SFC through `parseComponent` semantics and returns JSON.
pub fn parse_vue27_sfc_component(
    env: Env,
    source: String,
    options: Option<Unknown>,
) -> Result<String> {
    let raw_options = from_js_options(&env, options)?;
    let filename = string_option(&raw_options, "filename", "anonymous.vue");
    let mut compiler = SfcCompiler::new();
    let result = compiler.parse_vue27_component_with_filename(
        filename,
        &source,
        vue27_parse_component_options(&raw_options),
    );
    to_json_string(result)
}

#[napi(js_name = "compileSfcTemplate")]
/// Compiles the template block from a full SFC source.
pub fn compile_sfc_template(env: Env, source: String, options: Option<Unknown>) -> Result<String> {
    let raw_options = from_js_options(&env, options)?;
    let filename = string_option(&raw_options, "filename", "anonymous.vue");
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(filename, &source);
    let result = compiler.compile_template(&descriptor, sfc_template_options(Some(&raw_options)));
    to_json_string(result)
}

#[napi(js_name = "compileSfcTemplateSource")]
/// Compiles standalone SFC template source.
pub fn compile_sfc_template_source(
    env: Env,
    source: String,
    options: Option<Unknown>,
) -> Result<String> {
    let raw_options = from_js_options(&env, options)?;
    let filename = string_option(&raw_options, "filename", "template.vue.html");
    let compiler = SfcCompiler::new();
    let result = compiler.compile_template_source(
        filename,
        &source,
        sfc_template_options(Some(&raw_options)),
    );
    to_json_string(result)
}

#[napi(js_name = "compileSfcScript")]
/// Compiles script blocks from a full Vue 3 SFC source.
pub fn compile_sfc_script(env: Env, source: String, options: Option<Unknown>) -> Result<String> {
    let raw_options = from_js_options(&env, options)?;
    let filename = string_option(&raw_options, "filename", "anonymous.vue");
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(filename, &source);
    let result = compiler.compile_script(&descriptor, sfc_script_options(Some(&raw_options)));
    to_json_string(result)
}

#[napi(js_name = "compileVue27SfcTemplate")]
/// Compiles a Vue 2.7 SFC template source and returns official-style JSON.
pub fn compile_vue27_sfc_template(
    env: Env,
    source: String,
    options: Option<Unknown>,
) -> Result<String> {
    let raw_options = from_js_options(&env, options)?;
    let filename = string_option(&raw_options, "filename", "anonymous.vue");
    let compiler = SfcCompiler::new();
    let preprocessed = compiler.preprocess_vue27_template(
        &source,
        vue27_template_preprocess_options(&raw_options, &filename),
    );
    if !preprocessed.errors.is_empty() || !preprocessed.tips.is_empty() {
        return to_json_string(json!({
            "ast": {},
            "code": "var render = function () {}\nvar staticRenderFns = []\n",
            "source": source,
            "tips": preprocessed.tips,
            "errors": preprocessed.errors,
        }));
    }
    let compiled = vuec_vue2::compile(
        &preprocessed.source,
        vue27_template_vue2_options(raw_options.clone()),
    );
    to_json_string(json!({
        "ast": null,
        "code": vue27_template_code(&compiled.render, &compiled.static_render_fns),
        "source": source,
        "tips": compiled.tips,
        "errors": compiled.errors,
    }))
}

#[napi(js_name = "compileVue27SfcScript")]
/// Compiles script blocks from a Vue 2.7 SFC source.
pub fn compile_vue27_sfc_script(
    env: Env,
    source: String,
    options: Option<Unknown>,
) -> Result<String> {
    let raw_options = from_js_options(&env, options)?;
    let filename = string_option(&raw_options, "filename", "anonymous.vue");
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(filename, &source);
    let result = compiler.compile_vue27_script(&descriptor, sfc_script_options(Some(&raw_options)));
    to_json_string(result)
}

#[napi(js_name = "compileSfcStyle")]
/// Compiles style blocks from a full SFC source.
pub fn compile_sfc_style(env: Env, source: String, options: Option<Unknown>) -> Result<String> {
    let raw_options = from_js_options(&env, options)?;
    let filename = string_option(&raw_options, "filename", "anonymous.vue");
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(filename, &source);
    let result = compiler.compile_style(&descriptor, sfc_style_options(Some(&raw_options)));
    to_json_string(result)
}

fn from_js_options(env: &Env, options: Option<Unknown>) -> Result<Value> {
    options
        .map(|value| env.from_js_value(value))
        .transpose()
        .map(|value| value.unwrap_or(Value::Null))
}

fn to_json_string<T: serde::Serialize>(value: T) -> Result<String> {
    serde_json::to_string(&value).map_err(|err| napi::Error::from_reason(err.to_string()))
}

fn template_source(source: &str, options: &Value) -> TemplateSource {
    TemplateSource {
        filename: string_option(options, "filename", "anonymous.vue"),
        source: source.into(),
        file_id: FileId(0),
        base_offset: options
            .get("__vuecTemplateBaseOffset")
            .or_else(|| options.get("__vuecBaseOffset"))
            .and_then(Value::as_u64)
            .unwrap_or_default() as usize,
    }
}

fn apply_napi_dom_parser_defaults(core: &mut Vue3CompilerOptions, options: Option<&Value>) {
    let explicit_void_tags = napi_option_has(options, "__vuecVoidTags");
    let explicit_pre_tags = napi_option_has(options, "__vuecPreTags");
    let explicit_ignore_newline_tags = napi_option_has(options, "__vuecIgnoreNewlineTags");
    let explicit_native_tags = napi_option_has(options, "__vuecNativeTags");
    let void_tags = core.void_tags.clone();
    let pre_tags = core.pre_tags.clone();
    let ignore_newline_tags = core.ignore_newline_tags.clone();
    let native_tags = core.native_tags.clone();

    apply_dom_parser_defaults(core);

    if explicit_void_tags {
        core.void_tags = void_tags;
    }
    if explicit_pre_tags {
        core.pre_tags = pre_tags;
    }
    if explicit_ignore_newline_tags {
        core.ignore_newline_tags = ignore_newline_tags;
    }
    if explicit_native_tags {
        core.native_tags = native_tags;
    }
}

fn napi_option_has(options: Option<&Value>, name: &str) -> bool {
    options.is_some_and(|options| options.get(name).is_some())
}

fn vue2_options(value: Value) -> Vue2CompileOptions {
    let mut options = Vue2CompileOptions::default();
    let Value::Object(_) = value else {
        return options;
    };
    options.warn = bool_option(&value, "warn", options.warn);
    options.output_source_range = bool_option(
        &value,
        "outputSourceRange",
        bool_option(&value, "output_source_range", options.output_source_range),
    );
    options.comments = bool_option(&value, "comments", options.comments);
    options.preserve_whitespace = bool_option(
        &value,
        "preserveWhitespace",
        bool_option(&value, "preserve_whitespace", options.preserve_whitespace),
    );
    options.should_decode_newlines = bool_option(
        &value,
        "shouldDecodeNewlines",
        bool_option(
            &value,
            "should_decode_newlines",
            options.should_decode_newlines,
        ),
    );
    options.should_decode_newlines_for_href = bool_option(
        &value,
        "shouldDecodeNewlinesForHref",
        bool_option(
            &value,
            "should_decode_newlines_for_href",
            options.should_decode_newlines_for_href,
        ),
    );
    options.optimize = bool_option(&value, "optimize", options.optimize);
    options.disable_default_must_use_prop = bool_option(
        &value,
        "__vuecDisableDefaultMustUseProp",
        bool_option(
            &value,
            "disable_default_must_use_prop",
            options.disable_default_must_use_prop,
        ),
    );
    if let Some(delimiters) = value.get("delimiters").and_then(Value::as_array) {
        if delimiters.len() == 2 {
            if let (Some(open), Some(close)) = (delimiters[0].as_str(), delimiters[1].as_str()) {
                options.delimiters = Some([open.into(), close.into()]);
            }
        }
    }
    options.whitespace = value
        .get("whitespace")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    if let Some(namespaces) = string_map_option(&value, "__vuecTagNamespaces") {
        options.tag_namespaces = namespaces;
        options.use_default_tag_namespaces = false;
    }
    options.use_default_tag_namespaces = bool_option(
        &value,
        "__vuecUseDefaultTagNamespaces",
        bool_option(
            &value,
            "use_default_tag_namespaces",
            options.use_default_tag_namespaces,
        ),
    );
    if value.get("__vuecReservedTags").is_some() {
        options.reserved_tags = Some(string_array_option(&value, "__vuecReservedTags"));
        options.use_default_reserved_tags = false;
    }
    options.use_default_reserved_tags = bool_option(
        &value,
        "__vuecUseDefaultReservedTags",
        bool_option(
            &value,
            "use_default_reserved_tags",
            options.use_default_reserved_tags,
        ),
    );
    if let Some(bindings) = string_map_option(&value, "bindings") {
        options.bindings = bindings;
    }
    if let Some(bindings) = value.get("bindings") {
        options.bindings_is_script_setup = bindings
            .get("__isScriptSetup")
            .and_then(Value::as_bool)
            .unwrap_or(options.bindings_is_script_setup);
    }
    options
}

fn vue2_compile_value(compiled: &Vue2CompiledResult, options: &Vue2CompileOptions) -> Value {
    json!({
        "ast": vue2_public_ast_value(compiled),
        "ast_document": compiled.ast,
        "element_ast": compiled.element_ast,
        "ast_public": vue2_public_ast_value(compiled),
        "element_public_ast": vue2_public_ast_value(compiled),
        "render": compiled.render,
        "staticRenderFns": compiled.static_render_fns,
        "static_render_fns": compiled.static_render_fns,
        "errors": vue2_errors_value(&compiled.errors, options.output_source_range),
        "tips": vue2_tips_value(&compiled.tips, options.output_source_range),
        "diagnostics": compiled.diagnostics,
    })
}

fn vue2_public_ast_value(compiled: &Vue2CompiledResult) -> Value {
    compiled
        .element_ast
        .as_ref()
        .map(vue2_public_element_ast_value)
        .unwrap_or(Value::Null)
}

fn vue2_public_element_ast_value(element: &Vue2Element) -> Value {
    let mut object = Map::new();
    object.insert("type".into(), json!(1));
    object.insert("tag".into(), json!(element.tag));
    if let Some(ns) = element.ns.as_ref() {
        object.insert("ns".into(), json!(ns));
    }
    object.insert(
        "attrsList".into(),
        Value::Array(
            element
                .raw_attrs_list
                .iter()
                .map(vue2_public_raw_attr_value)
                .collect(),
        ),
    );
    object.insert("attrsMap".into(), json!(element.attrs_map));
    object.insert(
        "rawAttrsMap".into(),
        Value::Object(
            element
                .raw_attrs_map
                .iter()
                .map(|(name, attr)| (name.clone(), vue2_public_raw_attr_value(attr)))
                .collect(),
        ),
    );
    if !element.attrs.is_empty() {
        object.insert(
            "attrs".into(),
            Value::Array(element.attrs.iter().map(vue2_public_attr_value).collect()),
        );
    }
    if !element.props.is_empty() {
        object.insert(
            "props".into(),
            Value::Array(element.props.iter().map(vue2_public_attr_value).collect()),
        );
    }
    if !element.dynamic_attrs.is_empty() {
        object.insert(
            "dynamicAttrs".into(),
            Value::Array(
                element
                    .dynamic_attrs
                    .iter()
                    .map(vue2_public_attr_value)
                    .collect(),
            ),
        );
    }
    if !element.directives.is_empty() {
        object.insert(
            "directives".into(),
            Value::Array(
                element
                    .directives
                    .iter()
                    .map(vue2_public_directive_value)
                    .collect(),
            ),
        );
    }
    if !element.events.is_empty() {
        object.insert("events".into(), vue2_public_events_value(&element.events));
    }
    if !element.native_events.is_empty() {
        object.insert(
            "nativeEvents".into(),
            vue2_public_events_value(&element.native_events),
        );
    }
    object.insert(
        "children".into(),
        Value::Array(
            element
                .children
                .iter()
                .map(vue2_public_node_ast_value)
                .collect(),
        ),
    );
    object.insert("plain".into(), json!(element.plain));
    insert_true(&mut object, "forbidden", element.forbidden);
    insert_true(&mut object, "pre", element.pre);
    insert_true(&mut object, "once", element.once);
    insert_true(&mut object, "hasBindings", element.has_bindings);
    insert_optional_string(&mut object, "if", element.if_exp.as_ref());
    insert_optional_string(&mut object, "elseif", element.elseif.as_ref());
    insert_true(&mut object, "else", element.else_branch);
    if !element.if_conditions.is_empty() {
        object.insert(
            "ifConditions".into(),
            Value::Array(
                element
                    .if_conditions
                    .iter()
                    .map(vue2_public_if_condition_value)
                    .collect(),
            ),
        );
    }
    insert_optional_string(&mut object, "for", element.for_exp.as_ref());
    insert_optional_string(&mut object, "alias", element.alias.as_ref());
    insert_optional_string(&mut object, "iterator1", element.iterator1.as_ref());
    insert_optional_string(&mut object, "iterator2", element.iterator2.as_ref());
    insert_optional_string(&mut object, "key", element.key.as_ref());
    insert_optional_string(&mut object, "ref", element.ref_name.as_ref());
    insert_true(&mut object, "refInFor", element.ref_in_for);
    insert_optional_string(&mut object, "slotName", element.slot_name.as_ref());
    insert_optional_string(&mut object, "slotTarget", element.slot_target.as_ref());
    insert_true(
        &mut object,
        "slotTargetDynamic",
        element.slot_target_dynamic,
    );
    insert_optional_string(&mut object, "slotScope", element.slot_scope.as_ref());
    insert_true(&mut object, "slotNewSyntax", element.slot_new_syntax);
    if !element.scoped_slots.is_empty() {
        object.insert(
            "scopedSlots".into(),
            Value::Object(
                element
                    .scoped_slots
                    .iter()
                    .map(|(name, slot)| {
                        (
                            vue2_public_slot_key(name),
                            vue2_public_element_ast_value(slot),
                        )
                    })
                    .collect(),
            ),
        );
    }
    insert_optional_string(&mut object, "component", element.component.as_ref());
    insert_true(&mut object, "inlineTemplate", element.inline_template);
    insert_optional_string(&mut object, "staticClass", element.static_class.as_ref());
    insert_optional_string(&mut object, "classBinding", element.class_binding.as_ref());
    insert_optional_string(&mut object, "staticStyle", element.static_style.as_ref());
    insert_optional_string(&mut object, "styleBinding", element.style_binding.as_ref());
    if let Some(model) = element.model.as_ref() {
        object.insert("model".into(), json!(model));
    }
    if let Some(wrap_data) = element.wrap_data.as_ref() {
        object.insert("wrapData".into(), json!(wrap_data));
    }
    insert_optional_string(
        &mut object,
        "wrapListeners",
        element.wrap_listeners.as_ref(),
    );
    if let Some(validate) = element.validate.as_ref() {
        object.insert("validate".into(), json!(validate));
    }
    if !element.validators.is_empty() {
        object.insert("validators".into(), json!(element.validators));
    }
    object.insert("static".into(), json!(element.static_node));
    object.insert("staticRoot".into(), json!(element.static_root));
    object.insert("staticInFor".into(), json!(element.static_in_for));
    Value::Object(object)
}

fn vue2_public_node_ast_value(node: &vuec_vue2::Vue2Node) -> Value {
    match node {
        vuec_vue2::Vue2Node::Element(element) => vue2_public_element_ast_value(element),
        vuec_vue2::Vue2Node::Text(text) => {
            let mut object = Map::new();
            if let Some(expression) = text.expression.as_ref() {
                object.insert("type".into(), json!(2));
                object.insert("expression".into(), json!(expression));
                object.insert(
                    "tokens".into(),
                    json!([{ "@binding": vue27_binding_from_expression(expression) }]),
                );
            } else {
                object.insert("type".into(), json!(3));
            }
            object.insert("text".into(), json!(text.text));
            if text.is_comment {
                object.insert("isComment".into(), json!(true));
            }
            object.insert("static".into(), json!(text.static_node));
            Value::Object(object)
        }
    }
}

fn vue2_public_raw_attr_value(attr: &vuec_vue2::Vue2Attribute) -> Value {
    json!({
        "name": attr.name,
        "value": attr.value,
    })
}

fn vue2_public_attr_value(attr: &vuec_vue2::Vue2Attribute) -> Value {
    json!({
        "name": attr.name,
        "value": attr.value,
        "dynamic": attr.dynamic,
    })
}

fn vue2_public_directive_value(directive: &vuec_vue2::Vue2Directive) -> Value {
    let mut object = Map::new();
    object.insert("name".into(), json!(directive.name));
    object.insert("rawName".into(), json!(directive.raw_name));
    if let Some(value) = directive.value.as_ref() {
        object.insert("value".into(), json!(value));
    }
    if let Some(arg) = directive.arg.as_ref() {
        object.insert("arg".into(), json!(arg));
    }
    insert_true(&mut object, "isDynamicArg", directive.is_dynamic_arg);
    if !directive.modifiers.is_empty() {
        object.insert("modifiers".into(), json!(directive.modifiers));
    }
    Value::Object(object)
}

fn vue2_public_events_value(events: &BTreeMap<String, Vec<vuec_vue2::Vue2EventHandler>>) -> Value {
    Value::Object(
        events
            .iter()
            .map(|(name, handlers)| {
                let value = if handlers.len() == 1 {
                    vue2_public_event_handler_value(&handlers[0])
                } else {
                    Value::Array(
                        handlers
                            .iter()
                            .map(vue2_public_event_handler_value)
                            .collect(),
                    )
                };
                (name.clone(), value)
            })
            .collect(),
    )
}

fn vue2_public_event_handler_value(handler: &vuec_vue2::Vue2EventHandler) -> Value {
    let mut object = Map::new();
    object.insert("value".into(), json!(handler.value));
    insert_true(&mut object, "dynamic", handler.dynamic);
    if !handler.modifier_order.is_empty() {
        object.insert("modifierOrder".into(), json!(handler.modifier_order));
    }
    insert_true(
        &mut object,
        "hasModifierObject",
        handler.has_modifier_object,
    );
    if !handler.modifiers.is_empty() {
        object.insert("modifiers".into(), json!(handler.modifiers));
    }
    Value::Object(object)
}

fn vue2_public_if_condition_value(condition: &vuec_vue2::Vue2IfCondition) -> Value {
    json!({
        "exp": condition.exp,
        "block": vue2_public_element_ast_value(&condition.block),
    })
}

fn vue2_public_slot_key(name: &str) -> String {
    name.strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(name)
        .to_string()
}

fn vue27_binding_from_expression(expression: &str) -> String {
    let trimmed = expression.trim();
    if is_simple_identifier(trimmed) {
        return trimmed.to_string();
    }
    trimmed.to_string()
}

fn insert_optional_string(object: &mut Map<String, Value>, key: &str, value: Option<&String>) {
    if let Some(value) = value {
        object.insert(key.into(), json!(value));
    }
}

fn insert_true(object: &mut Map<String, Value>, key: &str, value: bool) {
    if value {
        object.insert(key.into(), json!(true));
    }
}

fn vue2_errors_value(errors: &[Vue2Error], output_source_range: bool) -> Value {
    if output_source_range {
        json!(errors)
    } else {
        json!(errors
            .iter()
            .map(|error| error.msg.clone())
            .collect::<Vec<_>>())
    }
}

fn vue2_tips_value(tips: &[Vue2Warning], output_source_range: bool) -> Value {
    if output_source_range {
        json!(tips)
    } else {
        json!(tips.iter().map(|tip| tip.msg.clone()).collect::<Vec<_>>())
    }
}

fn vue27_template_vue2_options(value: Value) -> Vue2CompileOptions {
    let mut options = vue2_options(value.clone());
    if let Some(bindings) = string_map_option(&value, "bindings") {
        options.bindings = bindings;
    }
    if let Some(bindings) = value.get("bindings") {
        options.bindings_is_script_setup = bindings
            .get("__isScriptSetup")
            .and_then(Value::as_bool)
            .unwrap_or(options.bindings_is_script_setup);
    }
    if vue27_transform_asset_urls_enabled(&value, false) {
        options.sfc_asset_url_transform = Some(vue27_sfc_asset_url_options(&value));
    }
    options
}

fn vue27_transform_asset_urls_enabled(value: &Value, fallback: bool) -> bool {
    match value.get("transformAssetUrls") {
        Some(Value::Bool(enabled)) => *enabled,
        Some(Value::Object(_)) => true,
        _ => fallback,
    }
}

fn vue27_sfc_asset_url_options(value: &Value) -> Vue2SfcAssetUrlTransformOptions {
    let mut options = Vue2SfcAssetUrlTransformOptions::default();
    if let Some(extra) = value.get("transformAssetUrlsOptions") {
        if let Some(base) = extra.get("base") {
            options.base = if base.is_null() {
                None
            } else {
                base.as_str().map(ToOwned::to_owned)
            };
        }
        options.include_absolute = bool_option(extra, "includeAbsolute", options.include_absolute);
    }
    match value.get("transformAssetUrls") {
        Some(Value::Object(object)) => {
            if !object.contains_key("base")
                && !object.contains_key("includeAbsolute")
                && !object.contains_key("tags")
            {
                let tags = vue27_sfc_asset_url_tags(object);
                if !tags.is_empty() {
                    let mut merged = vuec_vue2::vue2_sfc_default_asset_url_tags();
                    for (tag, attrs) in tags {
                        merged.insert(tag, attrs);
                    }
                    options.tags = merged;
                }
            } else if let Some(tags) = object.get("tags").and_then(Value::as_object) {
                let parsed = vue27_sfc_asset_url_tags(tags);
                if !parsed.is_empty() {
                    options.tags = parsed;
                }
            }
            if let Some(base) = object.get("base") {
                options.base = if base.is_null() {
                    None
                } else {
                    base.as_str().map(ToOwned::to_owned)
                };
            }
            options.include_absolute = object
                .get("includeAbsolute")
                .and_then(Value::as_bool)
                .unwrap_or(options.include_absolute);
        }
        Some(Value::Bool(_)) | None => {}
        _ => {}
    }
    options
}

fn vue27_sfc_asset_url_tags(object: &Map<String, Value>) -> BTreeMap<String, Vec<String>> {
    object
        .iter()
        .filter_map(|(tag, attrs)| match attrs {
            Value::String(attr) => Some((tag.clone(), vec![attr.clone()])),
            Value::Array(items) => {
                let attrs = items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(ToOwned::to_owned)
                    .collect::<Vec<_>>();
                (!attrs.is_empty()).then_some((tag.clone(), attrs))
            }
            _ => None,
        })
        .collect()
}

fn transform_asset_urls_enabled(value: &Value, fallback: bool) -> bool {
    transform_asset_urls_enabled_from(transform_asset_urls_value(value), fallback)
}

fn transform_asset_urls_enabled_with_compiler_fallback(value: &Value, fallback: bool) -> bool {
    transform_asset_urls_enabled_from(
        transform_asset_urls_value_with_compiler_fallback(value),
        fallback,
    )
}

fn transform_asset_urls_enabled_from(value: Option<&Value>, fallback: bool) -> bool {
    match value {
        Some(Value::Bool(enabled)) => *enabled,
        Some(Value::Object(_)) => true,
        _ => fallback,
    }
}

fn asset_url_options(value: &Value, mut options: AssetUrlOptions) -> AssetUrlOptions {
    let Some(raw) = transform_asset_urls_value(value) else {
        return options;
    };
    parse_asset_url_options(raw, &mut options);
    options
}

fn asset_url_options_with_compiler_fallback(
    value: &Value,
    mut options: AssetUrlOptions,
) -> AssetUrlOptions {
    let Some(raw) = transform_asset_urls_value_with_compiler_fallback(value) else {
        return options;
    };
    parse_asset_url_options(raw, &mut options);
    options
}

fn parse_asset_url_options(raw: &Value, options: &mut AssetUrlOptions) {
    match raw {
        Value::Bool(_) => {}
        Value::Object(object) => {
            if let Some(base) = object.get("base") {
                options.base = if base.is_null() {
                    None
                } else {
                    base.as_str().map(ToOwned::to_owned)
                };
            }
            options.include_absolute =
                bool_option(raw, "includeAbsolute", options.include_absolute);
            if let Some(tags) = object.get("tags").and_then(Value::as_object) {
                options.tags = asset_url_tags(tags);
            } else if !object.contains_key("base")
                && !object.contains_key("includeAbsolute")
                && !object.contains_key("tags")
                && object
                    .iter()
                    .any(|(_, value)| matches!(value, Value::String(_) | Value::Array(_)))
            {
                options.tags = asset_url_tags(object);
            }
        }
        _ => {}
    }
}

fn transform_asset_urls_value(value: &Value) -> Option<&Value> {
    value.get("transformAssetUrls")
}

fn transform_asset_urls_value_with_compiler_fallback(value: &Value) -> Option<&Value> {
    value.get("transformAssetUrls").or_else(|| {
        value
            .get("compilerOptions")
            .and_then(|compiler_options| compiler_options.get("transformAssetUrls"))
    })
}

fn asset_url_tags(object: &Map<String, Value>) -> BTreeMap<String, Vec<String>> {
    object
        .iter()
        .filter_map(|(tag, attrs)| match attrs {
            Value::String(attr) => Some((tag.clone(), vec![attr.clone()])),
            Value::Array(items) => {
                let attrs = items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(ToOwned::to_owned)
                    .collect::<Vec<_>>();
                (!attrs.is_empty()).then_some((tag.clone(), attrs))
            }
            _ => None,
        })
        .collect()
}

fn vue27_parse_component_options(value: &Value) -> Vue27ParseComponentOptions {
    let mut options = Vue27ParseComponentOptions::default();
    options.output_source_range = bool_option(value, "outputSourceRange", false);
    if let Some(deindent) = value.get("deindent").and_then(Value::as_bool) {
        options.deindent = Some(deindent);
    }
    options.pad = match value.get("pad") {
        Some(Value::Bool(true)) => Vue27SfcPad::True,
        Some(Value::String(value)) if value == "line" => Vue27SfcPad::Line,
        Some(Value::String(value)) if value == "space" => Vue27SfcPad::Space,
        _ => Vue27SfcPad::False,
    };
    options
}

fn vue27_template_preprocess_options(
    value: &Value,
    filename: &str,
) -> Vue27TemplatePreprocessOptions {
    Vue27TemplatePreprocessOptions {
        lang: value
            .get("preprocessLang")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        filename: Some(filename.to_string()),
    }
}

fn vue27_template_code(render: &str, static_render_fns: &[String]) -> String {
    format!(
        "var render = function render() {{\n  var _vm = this,\n    _c = _vm._self._c\n  return {}\n}}\nvar staticRenderFns = [{}]\nrender._withStripped = true\n",
        vue27_template_expr(render),
        static_render_fns
            .iter()
            .map(|render| format!("function(){{{render}}}"))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn vue27_template_expr(render: &str) -> String {
    let inner = render
        .strip_prefix("with(this){return ")
        .and_then(|value| value.strip_suffix('}'))
        .unwrap_or(render);
    let mut code = inner.to_string();
    for (from, to) in [
        ("_c(", "_c("),
        ("_v(", "_vm._v("),
        ("_s(", "_vm._s("),
        ("_l(", "_vm._l("),
        ("_e(", "_vm._e("),
        ("_m(", "_vm._m("),
        ("_t(", "_vm._t("),
    ] {
        code = code.replace(from, to);
    }
    code = prefix_simple_identifier_args(&code, "_vm._s(");
    code = code.replace("_c('", "_c(\"");
    code = code.replace("',", "\", ");
    code = code.replace("')", "\")");
    code = code.replace("{attrs:{", "{attrs: {");
    code = code.replace("{domProps:{", "{domProps: {");
    for key in ["href", "src", "srcset"] {
        code = code.replace(&format!("\"{key}\":"), &format!("{key}: "));
    }
    code
}

fn prefix_simple_identifier_args(source: &str, callee: &str) -> String {
    let mut output = String::new();
    let mut rest = source;
    while let Some(index) = rest.find(callee) {
        output.push_str(&rest[..index + callee.len()]);
        rest = &rest[index + callee.len()..];
        let Some(end) = rest.find(')') else {
            output.push_str(rest);
            return output;
        };
        let arg = &rest[..end];
        if is_simple_identifier(arg) {
            output.push_str("_vm.");
        }
        output.push_str(arg);
        output.push(')');
        rest = &rest[end + 1..];
    }
    output.push_str(rest);
    output
}

fn is_simple_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first == '$' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch == '$' || ch.is_ascii_alphanumeric())
}

fn vue27_rewrite_default_options(value: Value) -> Vue27RewriteDefaultOptions {
    let plugins = match &value {
        Value::Array(values) => values.as_slice(),
        Value::Null => &[],
        other => std::slice::from_ref(other),
    };
    Vue27RewriteDefaultOptions {
        typescript: plugins
            .iter()
            .any(|plugin| plugin.as_str() == Some("typescript")),
        decorators: plugins.iter().any(|plugin| {
            matches!(
                plugin.as_str(),
                Some("decorators" | "decorators-legacy" | "decoratorAutoAccessors")
            )
        }),
    }
}

fn vue3_options(value: Option<&Value>) -> Vue3CompilerOptions {
    let mut options = Vue3CompilerOptions::default();
    let Some(value) = value else {
        return options;
    };
    options.prefix_identifiers = bool_option(
        value,
        "prefixIdentifiers",
        bool_option(value, "prefix_identifiers", options.prefix_identifiers),
    );
    options.hoist_static = bool_option(
        value,
        "hoistStatic",
        bool_option(value, "hoist_static", options.hoist_static),
    );
    options.cache_handlers = bool_option(
        value,
        "cacheHandlers",
        bool_option(value, "cache_handlers", options.cache_handlers),
    );
    options.source_map = bool_option(
        value,
        "sourceMap",
        bool_option(value, "source_map", options.source_map),
    );
    options.source_map_source = value
        .get("__vuecSourceMapSource")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    options.source_map_base_offset = value
        .get("__vuecSourceMapBaseOffset")
        .and_then(Value::as_u64)
        .unwrap_or_default() as usize;
    options.ssr_css_vars = value
        .get("ssrCssVars")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    options.stringify_static = bool_option(
        value,
        "stringifyStatic",
        bool_option(value, "stringify_static", options.stringify_static),
    );
    options.stringify_static_preserve_helpers = bool_option(
        value,
        "__vuecStringifyStaticPreserveHelpers",
        bool_option(
            value,
            "stringify_static_preserve_helpers",
            options.stringify_static_preserve_helpers,
        ),
    );
    options.slotted = bool_option(value, "slotted", options.slotted);
    options.inline = bool_option(value, "inline", options.inline);
    options.ssr = bool_option(value, "ssr", options.ssr);
    options.optimize_imports = bool_option(value, "optimizeImports", options.optimize_imports);
    options.is_ts = bool_option(value, "isTS", bool_option(value, "is_ts", options.is_ts));
    options.comments = bool_option(value, "comments", options.comments);
    options.scope_id = value
        .get("scopeId")
        .or_else(|| value.get("scope_id"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    if let Some(mode) = value.get("mode").and_then(Value::as_str) {
        options.mode = mode.into();
    } else if options.prefix_identifiers {
        options.mode = "function".into();
    }
    if let Some(plugins) = value.get("expressionPlugins").and_then(Value::as_array) {
        options.expression_plugins = plugins
            .iter()
            .filter_map(Value::as_str)
            .map(ToOwned::to_owned)
            .collect();
    }
    if let Some(delimiters) = value.get("delimiters").and_then(Value::as_array) {
        if delimiters.len() == 2 {
            if let (Some(open), Some(close)) = (delimiters[0].as_str(), delimiters[1].as_str()) {
                options.delimiters = Some([open.into(), close.into()]);
            }
        }
    }
    if let Some(whitespace) = value.get("whitespace").and_then(Value::as_str) {
        options.whitespace = whitespace.into();
    }
    if vue3_parse_mode_is_sfc(Some(value)) {
        options.sfc_parse_mode = true;
        options.sfc_plain_template_langs = vec!["pug".to_string()];
    }
    options.void_tags = string_array_option(value, "__vuecVoidTags");
    options.pre_tags = string_array_option(value, "__vuecPreTags");
    options.ignore_newline_tags = string_array_option(value, "__vuecIgnoreNewlineTags");
    if let Some(namespaces) = value.get("__vuecNamespaces").and_then(Value::as_object) {
        options.namespaces = namespaces
            .iter()
            .filter_map(|(tag, namespace)| {
                vue3_namespace_option_value(namespace).map(|namespace| (tag.clone(), namespace))
            })
            .collect();
    }
    if let Some(namespace) = value
        .get("__vuecRootNamespace")
        .or_else(|| value.get("ns"))
        .and_then(vue3_namespace_option_value)
    {
        options.root_namespace = namespace;
    }
    options.dom_namespaces = bool_option(value, "__vuecDomNamespaces", options.dom_namespaces);
    if let Some(native_tags) = value.get("__vuecNativeTags").and_then(Value::as_array) {
        options.native_tags = Some(
            native_tags
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect(),
        );
    }
    options.custom_elements = string_array_option(value, "__vuecCustomElements");
    options.built_in_components = string_array_option(value, "__vuecBuiltInComponents");
    if let Some(metadata) = value.get("bindingMetadata").and_then(Value::as_object) {
        for (key, value) in metadata {
            if key == "__propsAliases" {
                if let Some(aliases) = value.as_object() {
                    options.props_aliases = aliases
                        .iter()
                        .filter_map(|(alias, source)| {
                            source
                                .as_str()
                                .map(|source| (alias.clone(), source.to_string()))
                        })
                        .collect();
                }
            } else if let Some(kind) = value.as_str() {
                options
                    .binding_metadata
                    .insert(key.clone(), kind.to_string());
            }
        }
    }
    options
}

fn vue3_namespace_option_value(value: &Value) -> Option<vuec_ast::HtmlNamespace> {
    match value {
        Value::Number(number) if number.as_u64() == Some(1) => Some(vuec_ast::HtmlNamespace::Svg),
        Value::Number(number) if number.as_u64() == Some(2) => {
            Some(vuec_ast::HtmlNamespace::MathMl)
        }
        Value::Number(number) if number.as_u64() == Some(0) => Some(vuec_ast::HtmlNamespace::Html),
        Value::String(value) if value.eq_ignore_ascii_case("svg") => {
            Some(vuec_ast::HtmlNamespace::Svg)
        }
        Value::String(value) if value.eq_ignore_ascii_case("math") => {
            Some(vuec_ast::HtmlNamespace::MathMl)
        }
        Value::String(value) if value.eq_ignore_ascii_case("mathml") => {
            Some(vuec_ast::HtmlNamespace::MathMl)
        }
        Value::String(value) if value.eq_ignore_ascii_case("html") => {
            Some(vuec_ast::HtmlNamespace::Html)
        }
        _ => None,
    }
}

fn vue3_parse_mode_is_sfc(value: Option<&Value>) -> bool {
    value
        .and_then(|value| value.get("parseMode"))
        .and_then(Value::as_str)
        == Some("sfc")
}

fn vue3_public_parse_ast(
    ast: &Vue3Ast,
    source: &str,
    base_offset: usize,
    options: &Vue3CompilerOptions,
) -> Value {
    json!({
        "type": 0,
        "source": source,
        "children": vue3_public_children(ast, ast.root, source, base_offset, options),
        "helpers": [],
        "components": [],
        "directives": [],
        "hoists": [],
        "imports": vue3_public_root_imports(ast),
        "cached": [],
        "temps": 0,
        "codegenNode": Value::Null,
        "loc": ast.root_node().map(|node| vue3_loc_value(source, base_offset, &node.span)).unwrap_or_else(vue3_loc_stub_value),
        "__vuecDiagnostics": vue3_parse_diagnostics(ast, source, base_offset, options),
    })
}

fn vue3_parse_diagnostics(
    ast: &Vue3Ast,
    source: &str,
    base_offset: usize,
    options: &Vue3CompilerOptions,
) -> Vec<Value> {
    let mut diagnostics = Vec::new();
    collect_html_parse_error_diagnostics(source, options, &mut diagnostics);
    collect_invalid_lt_diagnostics(ast, source, base_offset, options, &mut diagnostics);
    collect_missing_interpolation_end_diagnostics(source, options, &mut diagnostics);
    collect_invalid_end_tag_diagnostics(ast, source, base_offset, options, &mut diagnostics);
    collect_missing_directive_name_diagnostics(ast, source, base_offset, &mut diagnostics);
    diagnostics
}

fn collect_html_parse_error_diagnostics(
    source: &str,
    options: &Vue3CompilerOptions,
    diagnostics: &mut Vec<Value>,
) {
    if source.ends_with('<') {
        diagnostics.push(vue3_error_value(
            5,
            vue3_source_loc_value(source, source.len(), source.len()),
        ));
    } else if source.ends_with("</") && source.len() <= 2 {
        diagnostics.push(vue3_error_value(
            5,
            vue3_source_loc_value(source, source.len(), source.len()),
        ));
    }
    collect_missing_end_tag_name_diagnostics(source, diagnostics);

    let mut stack = Vec::<OpenDiagnosticElement>::new();
    let mut v_pre_depth = 0usize;
    let mut tokenizer = HtmlTokenizer::new(source);
    loop {
        if v_pre_depth > 0 {
            tokenizer.set_interpolation_delimiters("", "");
        } else if let Some([open, close]) = &options.delimiters {
            tokenizer.set_interpolation_delimiters(open, close);
        } else {
            tokenizer.set_interpolation_delimiters("{{", "}}");
        }
        let token = tokenizer.next_token();
        let eof = matches!(token.kind, HtmlTokenKind::Eof);
        match token.kind {
            HtmlTokenKind::StartTag {
                name,
                attributes,
                self_closing,
            } => {
                let incomplete = tag_token_is_incomplete(source, token.start, token.end);
                collect_start_tag_parse_errors(
                    source,
                    token.start,
                    token.end,
                    &attributes,
                    diagnostics,
                );
                if incomplete && token.end == source.len() {
                    diagnostics.push(vue3_error_value(
                        9,
                        vue3_source_loc_value(source, source.len(), source.len()),
                    ));
                } else if !self_closing && !vue3_is_void_tag(options, &name) {
                    let starts_v_pre =
                        v_pre_depth == 0 && attributes.iter().any(|attr| attr.name == "v-pre");
                    let in_v_pre = v_pre_depth > 0 || starts_v_pre;
                    let namespace =
                        vue3_diagnostic_tag_namespace(options, &name, &attributes, stack.last());
                    let raw_text_kind =
                        vuec_vue3_core::vue3_raw_text_kind(&name, namespace, in_v_pre);
                    let raw_tag = name.clone();
                    let sfc_raw_text =
                        sfc_diagnostic_raw_text_block(options, stack.len(), &raw_tag, &attributes);
                    stack.push(OpenDiagnosticElement {
                        name,
                        start: token.start,
                        namespace,
                        attributes,
                        in_v_pre,
                    });
                    if in_v_pre {
                        v_pre_depth += 1;
                    }
                    if raw_text_kind.is_some() || sfc_raw_text {
                        if let Some((_text_end, end_tag_end)) =
                            vuec_vue3_core::find_matching_raw_text_end(source, token.end, &raw_tag)
                        {
                            tokenizer.set_cursor(end_tag_end);
                            if let Some(open) = stack.pop() {
                                if open.in_v_pre && v_pre_depth > 0 {
                                    v_pre_depth -= 1;
                                }
                            }
                        }
                    }
                }
            }
            HtmlTokenKind::EndTag { name } => {
                if name.is_empty() {
                    if token.end == source.len()
                        && tag_token_is_incomplete(source, token.start, token.end)
                    {
                        let code = if source[token.start..token.end]
                            .as_bytes()
                            .get(2)
                            .is_some_and(u8::is_ascii_whitespace)
                        {
                            9
                        } else {
                            5
                        };
                        diagnostics.push(vue3_error_value(
                            code,
                            vue3_source_loc_value(source, source.len(), source.len()),
                        ));
                    } else {
                        pop_diagnostic_stack_until(&mut stack, &name, &mut v_pre_depth);
                    }
                } else if tag_token_is_incomplete(source, token.start, token.end) {
                    diagnostics.push(vue3_error_value(
                        9,
                        vue3_source_loc_value(source, source.len(), source.len()),
                    ));
                } else {
                    pop_diagnostic_stack_until(&mut stack, &name, &mut v_pre_depth);
                }
            }
            HtmlTokenKind::Comment(_) => {
                if source[token.start..].starts_with("<!--")
                    && token.end == source.len()
                    && !source[token.start..token.end].ends_with("-->")
                {
                    diagnostics.push(vue3_error_value(
                        7,
                        vue3_source_loc_value(source, source.len(), source.len()),
                    ));
                }
            }
            HtmlTokenKind::Cdata(_) => {
                if stack
                    .last()
                    .is_none_or(|open| open.namespace == vuec_ast::HtmlNamespace::Html)
                {
                    diagnostics.push(vue3_error_value(
                        1,
                        vue3_source_loc_value(source, token.start, token.start),
                    ));
                }
                if source[token.start..].starts_with("<![CDATA[")
                    && token.end == source.len()
                    && !source[token.start..token.end].ends_with("]]>")
                {
                    diagnostics.push(vue3_error_value(
                        6,
                        vue3_source_loc_value(source, source.len(), source.len()),
                    ));
                }
            }
            HtmlTokenKind::BogusQuestionTag => {
                diagnostics.push(vue3_error_value(
                    21,
                    vue3_source_loc_value(source, token.start + 1, token.start + 1),
                ));
            }
            HtmlTokenKind::Text(_) | HtmlTokenKind::Doctype(_) | HtmlTokenKind::Eof => {}
        }
        if eof {
            break;
        }
    }
}

struct OpenDiagnosticElement {
    name: String,
    start: usize,
    namespace: vuec_ast::HtmlNamespace,
    attributes: Vec<vuec_html::HtmlAttribute>,
    in_v_pre: bool,
}

fn sfc_diagnostic_raw_text_block(
    options: &Vue3CompilerOptions,
    depth: usize,
    tag: &str,
    attributes: &[vuec_html::HtmlAttribute],
) -> bool {
    if !options.sfc_parse_mode || depth != 0 {
        return false;
    }
    tag != "template" || sfc_plain_template_attrs(attributes, options)
}

fn sfc_plain_template_element(
    element: &vuec_ast::Vue3Element,
    options: &Vue3CompilerOptions,
) -> bool {
    if element.tag != "template" {
        return false;
    }
    element.props.iter().any(|prop| {
        matches!(
            prop,
            Vue3Prop::Attribute(attr)
                if attr.name == "lang"
                    && attr
                        .value
                        .as_deref()
                        .is_some_and(|lang| sfc_plain_template_lang(lang, options))
        )
    })
}

fn sfc_plain_template_attrs(
    attributes: &[vuec_html::HtmlAttribute],
    options: &Vue3CompilerOptions,
) -> bool {
    attributes.iter().any(|attr| {
        attr.name == "lang"
            && attr
                .value
                .as_deref()
                .is_some_and(|lang| sfc_plain_template_lang(lang, options))
    })
}

fn sfc_plain_template_lang(lang: &str, options: &Vue3CompilerOptions) -> bool {
    !lang.is_empty()
        && options
            .sfc_plain_template_langs
            .iter()
            .any(|candidate| candidate == lang)
}

fn vue3_diagnostic_tag_namespace(
    options: &Vue3CompilerOptions,
    tag: &str,
    attributes: &[vuec_html::HtmlAttribute],
    parent: Option<&OpenDiagnosticElement>,
) -> vuec_ast::HtmlNamespace {
    if let Some(namespace) = options.namespaces.get(tag).copied() {
        return namespace;
    }
    let mut namespace = parent
        .map(|open| open.namespace)
        .unwrap_or(options.root_namespace);
    if options.dom_namespaces {
        if let Some(parent) = parent {
            if namespace == vuec_ast::HtmlNamespace::MathMl {
                if parent.name == "annotation-xml" {
                    if tag == "svg" {
                        return vuec_ast::HtmlNamespace::Svg;
                    }
                    if diagnostic_attrs_have_value(
                        &parent.attributes,
                        "encoding",
                        &["text/html", "application/xhtml+xml"],
                    ) {
                        namespace = vuec_ast::HtmlNamespace::Html;
                    }
                } else if vue3_mathml_text_integration_point(&parent.name)
                    && tag != "mglyph"
                    && tag != "malignmark"
                {
                    namespace = vuec_ast::HtmlNamespace::Html;
                }
            } else if namespace == vuec_ast::HtmlNamespace::Svg
                && matches!(parent.name.as_str(), "foreignObject" | "desc" | "title")
            {
                namespace = vuec_ast::HtmlNamespace::Html;
            }
        }
        if namespace == vuec_ast::HtmlNamespace::Html {
            if tag == "svg" {
                return vuec_ast::HtmlNamespace::Svg;
            }
            if tag == "math" {
                return vuec_ast::HtmlNamespace::MathMl;
            }
        }
    }
    let _ = attributes;
    namespace
}

fn vue3_mathml_text_integration_point(tag: &str) -> bool {
    matches!(tag, "mi" | "mo" | "mn" | "ms" | "mtext")
}

fn diagnostic_attrs_have_value(
    attributes: &[vuec_html::HtmlAttribute],
    name: &str,
    values: &[&str],
) -> bool {
    attributes.iter().any(|attr| {
        attr.name == name
            && attr
                .value
                .as_deref()
                .is_some_and(|value| values.iter().any(|candidate| *candidate == value))
    })
}

fn pop_diagnostic_stack_until(
    stack: &mut Vec<OpenDiagnosticElement>,
    name: &str,
    v_pre_depth: &mut usize,
) {
    while let Some(open) = stack.pop() {
        if open.in_v_pre && *v_pre_depth > 0 {
            *v_pre_depth -= 1;
        }
        if open.name.eq_ignore_ascii_case(name) {
            break;
        }
    }
}

fn tag_token_is_incomplete(source: &str, start: usize, end: usize) -> bool {
    source
        .get(start..end)
        .is_some_and(|slice| !slice.ends_with('>'))
}

fn tag_token_is_incomplete_at_eof(source: &str, start: usize, end: usize) -> bool {
    end == source.len() && tag_token_is_incomplete(source, start, end)
}

fn collect_missing_end_tag_name_diagnostics(source: &str, diagnostics: &mut Vec<Value>) {
    let mut cursor = 0usize;
    while let Some(offset) = source[cursor..].find("</>") {
        let start = cursor + offset;
        diagnostics.push(vue3_error_value(
            14,
            vue3_source_loc_value(source, start + 2, start + 2),
        ));
        cursor = start + 3;
    }
}

fn collect_start_tag_parse_errors(
    source: &str,
    start: usize,
    end: usize,
    attributes: &[vuec_html::HtmlAttribute],
    diagnostics: &mut Vec<Value>,
) {
    collect_unexpected_equals_before_attribute_name(source, start, end, attributes, diagnostics);
    collect_unexpected_solidus_in_tag(source, start, end, attributes, diagnostics);

    let mut seen_attrs = Vec::<String>::new();
    for attr in attributes {
        if attr.name.starts_with('=') {
            diagnostics.push(vue3_error_value(
                19,
                vue3_source_loc_value(source, attr.name_start, attr.name_start),
            ));
        }

        if seen_attrs.iter().any(|seen| seen == &attr.name) {
            diagnostics.push(vue3_error_value(
                2,
                vue3_source_loc_value(source, attr.name_start, attr.name_start),
            ));
        } else {
            seen_attrs.push(attr.name.clone());
        }

        if let Some(offset) = attr
            .name
            .char_indices()
            .find_map(|(index, ch)| matches!(ch, '"' | '\'' | '<').then_some(index))
        {
            let absolute = attr.name_start + offset;
            diagnostics.push(vue3_error_value(
                17,
                vue3_source_loc_value(source, absolute, absolute),
            ));
        }

        if attr.name.contains('[') && !attr.name.contains(']') {
            diagnostics.push(vue3_error_value(
                27,
                vue3_source_loc_value(source, attr.name_end, attr.name_end),
            ));
        }

        if attr.value.as_deref() == Some("")
            && matches!(attr.quote, Some(vuec_html::HtmlQuoteKind::Unquoted))
            && attr
                .value_start
                .and_then(|value_start| source.as_bytes().get(value_start).copied())
                == Some(b'>')
        {
            let offset = attr.value_start.unwrap_or(attr.end);
            diagnostics.push(vue3_error_value(
                13,
                vue3_source_loc_value(source, offset, offset),
            ));
        }

        if matches!(attr.quote, Some(vuec_html::HtmlQuoteKind::Unquoted)) {
            if let (Some(value_start), Some(value_end)) =
                (attr.value_content_start, attr.value_content_end)
            {
                if let Some(offset) =
                    first_unexpected_unquoted_attribute_value_char(source, value_start, value_end)
                {
                    diagnostics.push(vue3_error_value(
                        18,
                        vue3_source_loc_value(source, offset, offset),
                    ));
                }
            }
        }
    }
}

fn collect_unexpected_equals_before_attribute_name(
    source: &str,
    start: usize,
    end: usize,
    attributes: &[vuec_html::HtmlAttribute],
    diagnostics: &mut Vec<Value>,
) {
    for offset in start..end {
        if source.as_bytes().get(offset) != Some(&b'=') {
            continue;
        }
        if attributes
            .iter()
            .any(|attr| offset >= attr.start && offset < attr.end)
        {
            continue;
        }
        diagnostics.push(vue3_error_value(
            19,
            vue3_source_loc_value(source, offset, offset),
        ));
    }
}

fn collect_unexpected_solidus_in_tag(
    source: &str,
    start: usize,
    end: usize,
    attributes: &[vuec_html::HtmlAttribute],
    diagnostics: &mut Vec<Value>,
) {
    for offset in start..end {
        if source.as_bytes().get(offset) != Some(&b'/') {
            continue;
        }
        if offset == start + 1 {
            continue;
        }
        if attributes.iter().any(|attr| {
            attr.value_content_start
                .zip(attr.value_content_end)
                .is_some_and(|(value_start, value_end)| offset >= value_start && offset < value_end)
        }) {
            continue;
        }
        if source.as_bytes().get(offset + 1) == Some(&b'>') {
            continue;
        }
        diagnostics.push(vue3_error_value(
            22,
            vue3_source_loc_value(source, offset, offset),
        ));
    }
}

fn first_unexpected_unquoted_attribute_value_char(
    source: &str,
    start: usize,
    end: usize,
) -> Option<usize> {
    source
        .get(start..end)?
        .char_indices()
        .find_map(|(index, ch)| matches!(ch, '"' | '\'' | '<' | '=' | '`').then_some(start + index))
}

fn collect_invalid_lt_diagnostics(
    ast: &Vue3Ast,
    source: &str,
    base_offset: usize,
    options: &Vue3CompilerOptions,
    diagnostics: &mut Vec<Value>,
) {
    for node in &ast.nodes {
        let Vue3AstKind::Text(_) = &node.kind else {
            continue;
        };
        if text_has_raw_text_parent(ast, node.id) || text_has_sfc_raw_parent(ast, node.id, options)
        {
            continue;
        }
        let Some(span) = node.span.source() else {
            continue;
        };
        let start = span.start.0.saturating_sub(base_offset);
        let end = span.end.0.saturating_sub(base_offset).min(source.len());
        let Some(slice) = source.get(start..end) else {
            continue;
        };
        let mut cursor = 0usize;
        while let Some(offset) = slice[cursor..].find('<') {
            let local_index = cursor + offset;
            cursor = local_index + 1;
            let global_index = start + local_index;
            match source.as_bytes().get(global_index + 1).copied() {
                Some(b'?') => diagnostics.push(vue3_error_value(
                    21,
                    vue3_source_loc_value(source, global_index + 1, global_index + 1),
                )),
                Some(b'/')
                    if source
                        .as_bytes()
                        .get(global_index + 2)
                        .is_some_and(u8::is_ascii_whitespace) =>
                {
                    diagnostics.push(vue3_error_value(
                        23,
                        vue3_source_loc_value(source, global_index, global_index),
                    ));
                }
                Some(next) if !matches!(next, b'/' | b'!' | b'A'..=b'Z' | b'a'..=b'z') => {
                    diagnostics.push(vue3_error_value(
                        12,
                        vue3_source_loc_value(source, global_index, global_index),
                    ));
                }
                _ => {}
            }
        }
    }
}

fn text_has_raw_text_parent(ast: &Vue3Ast, node_id: vuec_ast::NodeId) -> bool {
    let Some(parent_id) = ast.node(node_id).and_then(|node| node.parent) else {
        return false;
    };
    ast.node(parent_id).is_some_and(|node| {
        matches!(
            &node.kind,
            Vue3AstKind::Element(element)
                if element.ns == vuec_ast::HtmlNamespace::Html
                    && matches!(element.tag.as_str(), "textarea" | "title" | "style" | "script")
        )
    })
}

fn text_has_sfc_raw_parent(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    options: &Vue3CompilerOptions,
) -> bool {
    if !options.sfc_parse_mode {
        return false;
    }
    let Some(parent_id) = ast.node(node_id).and_then(|node| node.parent) else {
        return false;
    };
    let Some(parent) = ast.node(parent_id) else {
        return false;
    };
    let Some(root) = ast.node(ast.root) else {
        return false;
    };
    parent.parent == Some(ast.root)
        && root.children.contains(&parent_id)
        && matches!(
            &parent.kind,
            Vue3AstKind::Element(element)
                if element.tag != "template" || sfc_plain_template_element(element, options)
        )
}

fn collect_missing_interpolation_end_diagnostics(
    source: &str,
    options: &Vue3CompilerOptions,
    diagnostics: &mut Vec<Value>,
) {
    let mut stack = Vec::<OpenDiagnosticElement>::new();
    let mut v_pre_depth = 0usize;
    let mut tokenizer = HtmlTokenizer::new(source);
    loop {
        if v_pre_depth > 0 {
            tokenizer.set_interpolation_delimiters("", "");
        } else if let Some([open, close]) = &options.delimiters {
            tokenizer.set_interpolation_delimiters(open, close);
        } else {
            tokenizer.set_interpolation_delimiters("{{", "}}");
        }
        let token = tokenizer.next_token();
        let eof = matches!(token.kind, HtmlTokenKind::Eof);
        match token.kind {
            HtmlTokenKind::Text(text) if v_pre_depth == 0 => {
                collect_missing_interpolation_end_in_text(source, token.start, &text, diagnostics);
            }
            HtmlTokenKind::StartTag {
                name,
                attributes,
                self_closing,
            } => {
                let starts_v_pre =
                    v_pre_depth == 0 && attributes.iter().any(|attr| attr.name == "v-pre");
                let in_v_pre = v_pre_depth > 0 || starts_v_pre;
                let is_void = vue3_is_void_tag(options, &name);
                let namespace =
                    vue3_diagnostic_tag_namespace(options, &name, &attributes, stack.last());
                let raw_text_kind = vuec_vue3_core::vue3_raw_text_kind(&name, namespace, in_v_pre);
                if !self_closing && !is_void {
                    let raw_tag = name.clone();
                    let sfc_raw_text =
                        sfc_diagnostic_raw_text_block(options, stack.len(), &raw_tag, &attributes);
                    stack.push(OpenDiagnosticElement {
                        name,
                        start: token.start,
                        namespace,
                        attributes,
                        in_v_pre,
                    });
                    if in_v_pre {
                        v_pre_depth += 1;
                    }
                    if raw_text_kind.is_some() || sfc_raw_text {
                        if let Some((_text_end, end_tag_end)) =
                            vuec_vue3_core::find_matching_raw_text_end(source, token.end, &raw_tag)
                        {
                            tokenizer.set_cursor(end_tag_end);
                            if let Some(open) = stack.pop() {
                                if open.in_v_pre && v_pre_depth > 0 {
                                    v_pre_depth -= 1;
                                }
                            }
                        }
                    }
                }
            }
            HtmlTokenKind::EndTag { name } => {
                if !name.is_empty() {
                    while let Some(open) = stack.pop() {
                        if open.in_v_pre && v_pre_depth > 0 {
                            v_pre_depth -= 1;
                        }
                        if open.name.eq_ignore_ascii_case(&name) {
                            break;
                        }
                    }
                }
            }
            HtmlTokenKind::Cdata(_)
            | HtmlTokenKind::Text(_)
            | HtmlTokenKind::Comment(_)
            | HtmlTokenKind::BogusQuestionTag
            | HtmlTokenKind::Doctype(_)
            | HtmlTokenKind::Eof => {}
        }
        if eof {
            break;
        }
    }
}

fn collect_missing_interpolation_end_in_text(
    source: &str,
    token_start: usize,
    text: &str,
    diagnostics: &mut Vec<Value>,
) {
    let mut cursor = 0usize;
    while let Some(open_offset) = text[cursor..].find("{{") {
        let open = cursor + open_offset;
        let inner_start = open + 2;
        if let Some(close_offset) = text[inner_start..].find("}}") {
            cursor = inner_start + close_offset + 2;
        } else {
            let global_open = token_start + open;
            diagnostics.push(vue3_error_value(
                25,
                vue3_source_loc_value(source, global_open, global_open),
            ));
            break;
        }
    }
}

fn collect_invalid_end_tag_diagnostics(
    ast: &Vue3Ast,
    source: &str,
    _base_offset: usize,
    options: &Vue3CompilerOptions,
    diagnostics: &mut Vec<Value>,
) {
    let _ = ast;
    let mut stack = Vec::<OpenDiagnosticElement>::new();
    let mut v_pre_depth = 0usize;
    let mut tokenizer = HtmlTokenizer::new(source);
    loop {
        if v_pre_depth > 0 {
            tokenizer.set_interpolation_delimiters("", "");
        } else if let Some([open, close]) = &options.delimiters {
            tokenizer.set_interpolation_delimiters(open, close);
        } else {
            tokenizer.set_interpolation_delimiters("{{", "}}");
        }
        let token = tokenizer.next_token();
        let eof = matches!(token.kind, HtmlTokenKind::Eof);
        match token.kind {
            HtmlTokenKind::StartTag {
                name,
                attributes,
                self_closing,
            } => {
                let starts_v_pre =
                    v_pre_depth == 0 && attributes.iter().any(|attr| attr.name == "v-pre");
                let in_v_pre = v_pre_depth > 0 || starts_v_pre;
                let namespace =
                    vue3_diagnostic_tag_namespace(options, &name, &attributes, stack.last());
                let raw_text_kind = vuec_vue3_core::vue3_raw_text_kind(&name, namespace, in_v_pre);
                if !self_closing
                    && !vue3_is_void_tag(options, &name)
                    && !tag_token_is_incomplete_at_eof(source, token.start, token.end)
                {
                    let raw_tag = name.clone();
                    let sfc_raw_text =
                        sfc_diagnostic_raw_text_block(options, stack.len(), &raw_tag, &attributes);
                    stack.push(OpenDiagnosticElement {
                        name,
                        start: token.start,
                        namespace,
                        attributes,
                        in_v_pre,
                    });
                    if in_v_pre {
                        v_pre_depth += 1;
                    }
                    if raw_text_kind.is_some() || sfc_raw_text {
                        if let Some((_text_end, end_tag_end)) =
                            vuec_vue3_core::find_matching_raw_text_end(source, token.end, &raw_tag)
                        {
                            tokenizer.set_cursor(end_tag_end);
                            if let Some(open) = stack.pop() {
                                if open.in_v_pre && v_pre_depth > 0 {
                                    v_pre_depth -= 1;
                                }
                            }
                        }
                    }
                }
            }
            HtmlTokenKind::EndTag { name } => {
                if name.is_empty() {
                    if tag_token_is_incomplete(source, token.start, token.end) {
                        continue;
                    }
                    if source[token.start..token.end]
                        .as_bytes()
                        .get(2)
                        .is_some_and(u8::is_ascii_whitespace)
                    {
                        diagnostics.push(vue3_error_value(
                            23,
                            vue3_source_loc_value(source, token.start, token.start),
                        ));
                    }
                    continue;
                }
                if tag_token_is_incomplete(source, token.start, token.end) {
                    continue;
                }
                if stack
                    .last()
                    .is_some_and(|open| open.name.eq_ignore_ascii_case(&name))
                {
                    if stack.pop().is_some_and(|open| open.in_v_pre) && v_pre_depth > 0 {
                        v_pre_depth -= 1;
                    }
                } else if let Some(matching_index) = stack
                    .iter()
                    .rposition(|open| open.name.eq_ignore_ascii_case(&name))
                {
                    while stack.len() > matching_index + 1 {
                        if let Some(open) = stack.pop() {
                            if open.in_v_pre && v_pre_depth > 0 {
                                v_pre_depth -= 1;
                            }
                            if !open.in_v_pre {
                                diagnostics.push(vue3_error_value(
                                    24,
                                    vue3_source_loc_value(source, open.start, open.start),
                                ));
                            }
                        }
                    }
                    if stack.pop().is_some_and(|open| open.in_v_pre) && v_pre_depth > 0 {
                        v_pre_depth -= 1;
                    }
                } else if !stack
                    .last()
                    .is_some_and(|open| raw_text_tag_ignores_end_tag(&open.name, &name))
                {
                    diagnostics.push(vue3_error_value(
                        23,
                        vue3_source_loc_value(source, token.start, token.start),
                    ));
                }
            }
            HtmlTokenKind::Text(_)
            | HtmlTokenKind::Comment(_)
            | HtmlTokenKind::Cdata(_)
            | HtmlTokenKind::BogusQuestionTag
            | HtmlTokenKind::Doctype(_)
            | HtmlTokenKind::Eof => {}
        }
        if eof {
            break;
        }
    }
    while let Some(open) = stack.pop() {
        if !open.in_v_pre {
            diagnostics.push(vue3_error_value(
                24,
                vue3_source_loc_value(source, open.start, open.start),
            ));
        }
    }
}

fn raw_text_tag_ignores_end_tag(open: &str, close: &str) -> bool {
    matches!(open, "textarea" | "title") && !open.eq_ignore_ascii_case(close)
}

fn vue3_is_void_tag(options: &Vue3CompilerOptions, tag: &str) -> bool {
    options
        .void_tags
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(tag))
}

fn collect_missing_directive_name_diagnostics(
    ast: &Vue3Ast,
    source: &str,
    base_offset: usize,
    diagnostics: &mut Vec<Value>,
) {
    for node in &ast.nodes {
        let Vue3AstKind::Element(element) = &node.kind else {
            continue;
        };
        for prop in &element.props {
            let Vue3Prop::Attribute(attr) = prop else {
                continue;
            };
            if attr.name == "v-" {
                let loc = attr
                    .span
                    .map(|span| vue3_source_span_value(source, base_offset, span))
                    .unwrap_or_else(vue3_loc_stub_value);
                diagnostics.push(vue3_error_value(26, loc));
            }
        }
    }
}

fn vue3_error_value(code: u8, loc: Value) -> Value {
    json!({
        "code": code,
        "message": vue3_parse_error_message(code),
        "loc": loc,
    })
}

fn vue3_parse_error_message(code: u8) -> &'static str {
    match code {
        0 => "Illegal comment.",
        1 => "CDATA section is allowed only in XML context.",
        2 => "Duplicate attribute.",
        3 => "End tag cannot have attributes.",
        4 => "Illegal '/' in tags.",
        5 => "Unexpected EOF in tag.",
        6 => "Unexpected EOF in CDATA section.",
        7 => "Unexpected EOF in comment.",
        8 => "Unexpected EOF in script.",
        9 => "Unexpected EOF in tag.",
        10 => "Incorrectly closed comment.",
        11 => "Incorrectly opened comment.",
        12 => "Illegal tag name. Use '&lt;' to print '<'.",
        13 => "Attribute value was expected.",
        14 => "End tag name was expected.",
        15 => "Whitespace was expected.",
        16 => "Unexpected '<!--' in comment.",
        17 => "Attribute name cannot contain U+0022 (\"), U+0027 ('), and U+003C (<).",
        18 => {
            "Unquoted attribute value cannot contain U+0022 (\"), U+0027 ('), U+003C (<), U+003D (=), and U+0060 (`)."
        }
        19 => "Attribute name cannot start with '='.",
        20 => "Unexpected null character.",
        21 => "'<?' is allowed only in XML context.",
        22 => "Illegal '/' in tags.",
        23 => "Invalid end tag.",
        24 => "Element is missing end tag.",
        25 => "Interpolation end sign was not found.",
        26 => "Legal directive name was expected.",
        27 => {
            "End bracket for dynamic directive argument was not found. Note that dynamic directive argument cannot contain spaces."
        }
        _ => "Vue compiler parse error",
    }
}

fn vue3_public_root_imports(ast: &Vue3Ast) -> Vec<Value> {
    ast.root_node()
        .and_then(|node| match &node.kind {
            Vue3AstKind::Root(root) => Some(&root.imports),
            _ => None,
        })
        .into_iter()
        .flatten()
        .map(|import| {
            json!({
                "exp": vue3_simple_expression_value(&import.name, false, vue3_loc_stub_value()),
                "path": import.path,
            })
        })
        .collect()
}

fn vue3_public_children(
    ast: &Vue3Ast,
    parent: vuec_ast::NodeId,
    source: &str,
    base_offset: usize,
    options: &Vue3CompilerOptions,
) -> Vec<Value> {
    ast.node(parent)
        .map(|node| {
            node.children
                .iter()
                .filter_map(|child| vue3_public_node(ast, *child, source, base_offset, options))
                .collect()
        })
        .unwrap_or_default()
}

fn vue3_public_node(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    source: &str,
    base_offset: usize,
    options: &Vue3CompilerOptions,
) -> Option<Value> {
    let node = ast.node(node_id)?;
    Some(match &node.kind {
        Vue3AstKind::Root(root) => json!({
            "type": 0,
            "source": source,
            "children": vue3_public_children(ast, node_id, source, base_offset, options),
            "helpers": [],
            "components": [],
            "directives": [],
            "hoists": [],
            "imports": root.imports.iter().map(|import| json!({
                "exp": vue3_simple_expression_value(&import.name, false, vue3_loc_stub_value()),
                "path": import.path,
            })).collect::<Vec<_>>(),
            "cached": [],
            "temps": 0,
            "codegenNode": Value::Null,
            "loc": vue3_loc_value(source, base_offset, &node.span),
        }),
        Vue3AstKind::Element(element) => {
            let mut value = json!({
                "type": 1,
                "tag": element.tag,
                "ns": vue3_namespace_value(element.ns),
                "tagType": vue3_element_type_value(element.tag_type),
                "props": element.props.iter().map(|prop| vue3_prop_value(source, base_offset, prop, options)).collect::<Vec<_>>(),
                "children": vue3_public_children(ast, node_id, source, base_offset, options),
                "loc": vue3_loc_value(source, base_offset, &node.span),
                "codegenNode": Value::Null,
                "isSelfClosing": if element.self_closing { json!(true) } else { Value::Null },
            });
            if options.sfc_parse_mode {
                value["innerLoc"] = vue3_inner_loc_value(ast, source, base_offset, node_id);
            }
            value
        }
        Vue3AstKind::Text(text) => json!({
            "type": 2,
            "content": text.value,
            "loc": vue3_text_loc_value(source, base_offset, &node.span),
        }),
        Vue3AstKind::Comment(comment) => json!({
            "type": 3,
            "content": comment.value,
            "loc": vue3_loc_value(source, base_offset, &node.span),
        }),
        Vue3AstKind::Interpolation(interpolation) => json!({
            "type": 5,
            "content": vue3_expression_value(source, base_offset, &interpolation.expression, &node.span, false, options, Vue3ExpressionAstMode::Expression),
            "loc": vue3_loc_value(source, base_offset, &node.span),
        }),
        _ => json!({
            "type": 7,
            "name": "unsupported",
            "exp": Value::Null,
            "loc": vue3_loc_value(source, base_offset, &node.span),
        }),
    })
}

fn vue3_prop_value(
    source: &str,
    base_offset: usize,
    prop: &Vue3Prop,
    options: &Vue3CompilerOptions,
) -> Value {
    match prop {
        Vue3Prop::Attribute(attr) => json!({
            "type": 6,
            "name": attr.name,
            "nameLoc": attr.name_span.map(|span| vue3_source_span_value(source, base_offset, span)).unwrap_or_else(vue3_loc_stub_value),
            "value": attr.value.as_ref().map(|value| json!({
                "type": 2,
                "content": value,
                "loc": attr.value_span.map(|span| vue3_source_span_value(source, base_offset, span)).unwrap_or_else(vue3_loc_stub_value),
            })),
            "loc": attr.span.map(|span| vue3_source_span_value(source, base_offset, span)).unwrap_or_else(vue3_loc_stub_value),
        }),
        Vue3Prop::Directive(dir) => {
            let exp_mode = match dir.name.as_str() {
                "on" => Vue3ExpressionAstMode::Statements,
                "slot" => Vue3ExpressionAstMode::Params,
                _ => Vue3ExpressionAstMode::Expression,
            };
            let mut value = json!({
                "type": 7,
                "name": dir.name,
                "rawName": dir.raw_name,
                "exp": dir.exp.as_ref().map(|exp| vue3_expression_value_with_mode(source, base_offset, exp, &span_to_node_span(dir.exp_span), false, Vue3ExpressionProjectionMode::Exact, options, exp_mode)),
                "arg": dir.arg.as_ref().map(|arg| vue3_expression_value_with_mode(source, base_offset, arg, &span_to_node_span(dir.arg_span), !dir.is_dynamic_arg, Vue3ExpressionProjectionMode::ExactLocTrimContent, options, Vue3ExpressionAstMode::Expression)),
                "modifiers": dir.modifiers.iter().enumerate().map(|(index, modifier)| {
                    let loc = dir
                        .modifier_spans
                        .get(index)
                        .map(|span| vue3_loc_value(source, base_offset, span))
                        .unwrap_or_else(vue3_loc_stub_value);
                    vue3_simple_expression_value(
                        modifier,
                        !matches!(dir.modifier_spans.get(index), Some(NodeSpan::Missing { .. })),
                        loc,
                    )
                }).collect::<Vec<_>>(),
                "loc": dir.span.map(|span| vue3_source_span_value(source, base_offset, span)).unwrap_or_else(vue3_loc_stub_value),
            });
            if dir.name == "for" {
                value["forParseResult"] =
                    vue3_for_parse_result_value(source, base_offset, dir, options);
            }
            value
        }
    }
}

fn vue3_inner_loc_value(
    ast: &Vue3Ast,
    source: &str,
    base_offset: usize,
    node_id: vuec_ast::NodeId,
) -> Value {
    let Some(node) = ast.node(node_id) else {
        return vue3_loc_stub_value();
    };
    let Some(span) = node.span.source() else {
        return vue3_loc_stub_value();
    };
    let element_start = span.start.0.saturating_sub(base_offset);
    let element_end = span.end.0.saturating_sub(base_offset).min(source.len());
    let open_end = vue3_open_tag_end(source, element_start, element_end).unwrap_or(element_start);
    let inner_end = vue3_close_tag_start(source, open_end, element_end).unwrap_or_else(|| {
        node.children
            .last()
            .and_then(|child_id| ast.node(*child_id))
            .and_then(|child| child.span.source())
            .map(|child_span| {
                child_span
                    .end
                    .0
                    .saturating_sub(base_offset)
                    .min(source.len())
            })
            .unwrap_or(open_end)
    });
    vue3_source_loc_value(source, open_end, inner_end)
}

fn vue3_open_tag_end(source: &str, start: usize, end: usize) -> Option<usize> {
    let mut quote = None;
    for (offset, ch) in source.get(start..end)?.char_indices() {
        match (quote, ch) {
            (Some(active), current) if current == active => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(ch),
            (None, '>') => return Some(start + offset + 1),
            (None, _) => {}
        }
    }
    None
}

fn vue3_close_tag_start(source: &str, open_end: usize, element_end: usize) -> Option<usize> {
    let mut cursor = open_end.min(source.len());
    let end = element_end.min(source.len());
    let mut close_start = None;
    while cursor < end {
        let Some(offset) = source.get(cursor..end)?.find("</") else {
            break;
        };
        close_start = Some(cursor + offset);
        cursor += offset + "</".len();
    }
    close_start
}

fn span_to_node_span(span: Option<Span>) -> NodeSpan {
    span.map(NodeSpan::from)
        .unwrap_or_else(|| NodeSpan::Missing {
            reason: vuec_ast::MissingSpanReason::Synthetic,
        })
}

fn vue3_expression_value(
    source: &str,
    base_offset: usize,
    expression: &Vue3Expression,
    fallback_span: &NodeSpan,
    is_static: bool,
    options: &Vue3CompilerOptions,
    ast_mode: Vue3ExpressionAstMode,
) -> Value {
    vue3_expression_value_with_mode(
        source,
        base_offset,
        expression,
        fallback_span,
        is_static,
        Vue3ExpressionProjectionMode::Trim,
        options,
        ast_mode,
    )
}

#[derive(Clone, Copy)]
enum Vue3ExpressionProjectionMode {
    Trim,
    ExactLocTrimContent,
    Exact,
}

#[derive(Clone, Copy)]
enum Vue3ExpressionAstMode {
    Expression,
    Params,
    Statements,
}

fn vue3_expression_value_with_mode(
    source_text: &str,
    base_offset: usize,
    expression: &Vue3Expression,
    fallback_span: &NodeSpan,
    is_static: bool,
    mode: Vue3ExpressionProjectionMode,
    options: &Vue3CompilerOptions,
    ast_mode: Vue3ExpressionAstMode,
) -> Value {
    let source = expression.source_string();
    let loc = match mode {
        Vue3ExpressionProjectionMode::Trim => {
            vue3_expression_loc(source_text, base_offset, fallback_span, &source)
        }
        Vue3ExpressionProjectionMode::ExactLocTrimContent | Vue3ExpressionProjectionMode::Exact => {
            vue3_loc_value(source_text, base_offset, fallback_span)
        }
    };
    let content = match mode {
        Vue3ExpressionProjectionMode::Exact => source,
        Vue3ExpressionProjectionMode::Trim | Vue3ExpressionProjectionMode::ExactLocTrimContent => {
            source.trim().to_string()
        }
    };
    let mut value = vue3_simple_expression_value(&content, is_static, loc);
    if let Some(ast_value) = vue3_expression_ast_value(&content, is_static, options, ast_mode) {
        value["ast"] = ast_value;
    }
    value
}

fn vue3_simple_expression_value(content: &str, is_static: bool, loc: Value) -> Value {
    json!({
        "type": 4,
        "content": content,
        "isStatic": is_static,
        "constType": if is_static { 3 } else { 0 },
        "loc": loc,
    })
}

fn vue3_expression_ast_value(
    source: &str,
    is_static: bool,
    options: &Vue3CompilerOptions,
    mode: Vue3ExpressionAstMode,
) -> Option<Value> {
    if is_static || !options.prefix_identifiers || source.trim().is_empty() {
        return None;
    }
    let trimmed = source.trim();
    if is_simple_identifier(trimmed) {
        return Some(Value::Null);
    }
    let store = JsAstStore::new();
    let source_type = vue3_expression_source_type(options);
    match mode {
        Vue3ExpressionAstMode::Expression => {
            let expression_source = format!("({trimmed})");
            store
                .parse_expression(&expression_source, source_type)
                .ok()
                .map(|expression| expression_ast_value(&expression))
        }
        Vue3ExpressionAstMode::Params => {
            let expression_source = format!("({trimmed})=>{{}}");
            store
                .parse_expression(&expression_source, source_type)
                .ok()
                .map(|expression| expression_ast_value(&expression))
        }
        Vue3ExpressionAstMode::Statements => {
            let program_source = format!(" {trimmed} ");
            let program = store.parse_program(&program_source, source_type);
            Some(json!({
                "type": "Program",
                "body": program.program.body.iter().map(statement_ast_value).collect::<Vec<_>>(),
            }))
        }
    }
}

fn vue3_for_parse_result_value(
    source: &str,
    base_offset: usize,
    dir: &vuec_ast::Vue3Directive,
    options: &Vue3CompilerOptions,
) -> Value {
    let expression = dir
        .exp
        .as_ref()
        .map(Vue3Expression::source_string)
        .unwrap_or_default();
    let Some((aliases, iterable)) = split_v_for_expression(&expression) else {
        return Value::Null;
    };
    let source_loc = dir
        .exp_span
        .and_then(|span| {
            let local_start = span.start.0.saturating_sub(base_offset);
            let local_end = span.end.0.saturating_sub(base_offset).min(source.len());
            source
                .get(local_start..local_end)
                .and_then(|slice| slice.find(iterable).map(|offset| local_start + offset))
                .map(|start| vue3_source_loc_value(source, start, start + iterable.len()))
        })
        .unwrap_or_else(vue3_loc_stub_value);
    let parts = split_v_for_aliases(aliases);
    json!({
        "source": vue3_simple_expression_with_ast_value(iterable, false, source_loc, options, Vue3ExpressionAstMode::Expression),
        "value": parts.first().map(|value| {
            vue3_simple_expression_with_ast_value(value, false, vue3_loc_stub_value(), options, Vue3ExpressionAstMode::Params)
        }),
        "key": parts.get(1).map(|value| {
            vue3_simple_expression_with_ast_value(value, false, vue3_loc_stub_value(), options, Vue3ExpressionAstMode::Expression)
        }),
        "index": parts.get(2).map(|value| {
            vue3_simple_expression_with_ast_value(value, false, vue3_loc_stub_value(), options, Vue3ExpressionAstMode::Expression)
        }),
        "finalized": false,
    })
}

fn vue3_simple_expression_with_ast_value(
    source: &str,
    is_static: bool,
    loc: Value,
    options: &Vue3CompilerOptions,
    ast_mode: Vue3ExpressionAstMode,
) -> Value {
    let mut value = vue3_simple_expression_value(source, is_static, loc);
    if let Some(ast_value) = vue3_expression_ast_value(source, is_static, options, ast_mode) {
        value["ast"] = ast_value;
    }
    value
}

fn vue3_expression_source_type(options: &Vue3CompilerOptions) -> SourceType {
    if options.is_ts
        || options
            .expression_plugins
            .iter()
            .any(|plugin| plugin == "typescript")
    {
        SourceType::ts()
    } else {
        SourceType::mjs()
    }
}

fn expression_ast_value(expression: &Expression<'_>) -> Value {
    match expression {
        Expression::ArrayExpression(array) => json!({
            "type": "ArrayExpression",
            "elements": array.elements.iter().map(array_element_ast_value).collect::<Vec<_>>(),
        }),
        Expression::ArrowFunctionExpression(function) => json!({
            "type": "ArrowFunctionExpression",
            "params": formal_parameters_ast_values(&function.params),
            "body": function_body_ast_value(&function.body),
        }),
        Expression::AssignmentExpression(assignment) => json!({
            "type": "AssignmentExpression",
            "left": assignment_target_ast_value(&assignment.left),
            "right": expression_ast_value(&assignment.right),
        }),
        Expression::AwaitExpression(await_expression) => json!({
            "type": "AwaitExpression",
            "argument": expression_ast_value(&await_expression.argument),
        }),
        Expression::BinaryExpression(binary) => json!({
            "type": "BinaryExpression",
            "left": expression_ast_value(&binary.left),
            "right": expression_ast_value(&binary.right),
        }),
        Expression::CallExpression(call) => json!({
            "type": "CallExpression",
            "callee": expression_ast_value(&call.callee),
            "arguments": call.arguments.iter().map(argument_ast_value).collect::<Vec<_>>(),
            "optional": call.optional,
        }),
        Expression::ChainExpression(chain) => json!({
            "type": "ChainExpression",
            "expression": chain_element_ast_value(&chain.expression),
        }),
        Expression::ConditionalExpression(conditional) => json!({
            "type": "ConditionalExpression",
            "test": expression_ast_value(&conditional.test),
            "consequent": expression_ast_value(&conditional.consequent),
            "alternate": expression_ast_value(&conditional.alternate),
        }),
        Expression::FunctionExpression(function) => json!({
            "type": "FunctionExpression",
            "params": formal_parameters_ast_values(&function.params),
            "body": function.body.as_ref().map(|body| function_body_ast_value(body)),
        }),
        Expression::Identifier(identifier) => identifier_reference_ast_value(identifier),
        Expression::ImportExpression(import_expression) => json!({
            "type": "ImportExpression",
            "source": expression_ast_value(&import_expression.source),
            "options": import_expression.options.as_ref().map(expression_ast_value),
        }),
        Expression::LogicalExpression(logical) => json!({
            "type": "LogicalExpression",
            "left": expression_ast_value(&logical.left),
            "right": expression_ast_value(&logical.right),
        }),
        Expression::ComputedMemberExpression(member) => computed_member_ast_value(member),
        Expression::StaticMemberExpression(member) => static_member_ast_value(member),
        Expression::PrivateFieldExpression(member) => private_field_ast_value(member),
        Expression::NewExpression(new_expression) => json!({
            "type": "NewExpression",
            "callee": expression_ast_value(&new_expression.callee),
            "arguments": new_expression.arguments.iter().map(argument_ast_value).collect::<Vec<_>>(),
        }),
        Expression::ObjectExpression(object) => json!({
            "type": "ObjectExpression",
            "properties": object.properties.iter().map(object_property_kind_ast_value).collect::<Vec<_>>(),
        }),
        Expression::ParenthesizedExpression(parenthesized) => {
            expression_ast_value(&parenthesized.expression)
        }
        Expression::PrivateInExpression(private_in) => json!({
            "type": "BinaryExpression",
            "right": expression_ast_value(&private_in.right),
        }),
        Expression::SequenceExpression(sequence) => json!({
            "type": "SequenceExpression",
            "expressions": sequence.expressions.iter().map(expression_ast_value).collect::<Vec<_>>(),
        }),
        Expression::TaggedTemplateExpression(tagged) => json!({
            "type": "TaggedTemplateExpression",
            "tag": expression_ast_value(&tagged.tag),
            "quasi": template_literal_ast_value(&tagged.quasi),
        }),
        Expression::TemplateLiteral(template) => template_literal_ast_value(template),
        Expression::ThisExpression(_) => json!({ "type": "ThisExpression" }),
        Expression::UnaryExpression(unary) => json!({
            "type": "UnaryExpression",
            "argument": expression_ast_value(&unary.argument),
        }),
        Expression::UpdateExpression(update) => json!({
            "type": "UpdateExpression",
            "argument": simple_assignment_target_ast_value(&update.argument),
        }),
        Expression::YieldExpression(yield_expression) => json!({
            "type": "YieldExpression",
            "argument": yield_expression.argument.as_ref().map(expression_ast_value),
        }),
        Expression::BooleanLiteral(literal) => json!({
            "type": "Literal",
            "value": literal.value,
        }),
        Expression::NullLiteral(_) => json!({
            "type": "Literal",
            "value": Value::Null,
        }),
        Expression::NumericLiteral(literal) => json!({
            "type": "Literal",
            "value": literal.value,
        }),
        Expression::StringLiteral(literal) => json!({
            "type": "Literal",
            "value": literal.value.as_str(),
        }),
        Expression::BigIntLiteral(literal) => json!({
            "type": "Literal",
            "value": literal.value.as_str(),
        }),
        Expression::RegExpLiteral(_) => json!({ "type": "Literal" }),
        Expression::TSAsExpression(expression) => {
            ts_expression_ast_value("TSAsExpression", &expression.expression)
        }
        Expression::TSSatisfiesExpression(expression) => {
            ts_expression_ast_value("TSSatisfiesExpression", &expression.expression)
        }
        Expression::TSTypeAssertion(expression) => {
            ts_expression_ast_value("TSTypeAssertion", &expression.expression)
        }
        Expression::TSNonNullExpression(expression) => {
            ts_expression_ast_value("TSNonNullExpression", &expression.expression)
        }
        Expression::TSInstantiationExpression(expression) => {
            ts_expression_ast_value("TSInstantiationExpression", &expression.expression)
        }
        _ => json!({ "type": "Expression" }),
    }
}

fn statement_ast_value(statement: &Statement<'_>) -> Value {
    match statement {
        Statement::BlockStatement(block) => json!({
            "type": "BlockStatement",
            "body": block.body.iter().map(statement_ast_value).collect::<Vec<_>>(),
        }),
        Statement::DoWhileStatement(statement) => json!({
            "type": "DoWhileStatement",
            "body": statement_ast_value(&statement.body),
            "test": expression_ast_value(&statement.test),
        }),
        Statement::ExpressionStatement(statement) => json!({
            "type": "ExpressionStatement",
            "expression": expression_ast_value(&statement.expression),
        }),
        Statement::ForStatement(statement) => json!({
            "type": "ForStatement",
            "test": statement.test.as_ref().map(expression_ast_value),
            "update": statement.update.as_ref().map(expression_ast_value),
            "body": statement_ast_value(&statement.body),
        }),
        Statement::IfStatement(statement) => json!({
            "type": "IfStatement",
            "test": expression_ast_value(&statement.test),
            "consequent": statement_ast_value(&statement.consequent),
            "alternate": statement.alternate.as_ref().map(statement_ast_value),
        }),
        Statement::ReturnStatement(statement) => json!({
            "type": "ReturnStatement",
            "argument": statement.argument.as_ref().map(expression_ast_value),
        }),
        Statement::ThrowStatement(statement) => json!({
            "type": "ThrowStatement",
            "argument": expression_ast_value(&statement.argument),
        }),
        Statement::VariableDeclaration(declaration) => json!({
            "type": "VariableDeclaration",
            "declarations": declaration.declarations.iter().map(|declarator| json!({
                "type": "VariableDeclarator",
                "id": binding_pattern_ast_value(&declarator.id),
                "init": declarator.init.as_ref().map(expression_ast_value),
            })).collect::<Vec<_>>(),
        }),
        Statement::WhileStatement(statement) => json!({
            "type": "WhileStatement",
            "test": expression_ast_value(&statement.test),
            "body": statement_ast_value(&statement.body),
        }),
        _ => json!({ "type": statement_type_name(statement) }),
    }
}

fn statement_type_name(statement: &Statement<'_>) -> &'static str {
    match statement {
        Statement::BlockStatement(_) => "BlockStatement",
        Statement::BreakStatement(_) => "BreakStatement",
        Statement::ContinueStatement(_) => "ContinueStatement",
        Statement::DebuggerStatement(_) => "DebuggerStatement",
        Statement::DoWhileStatement(_) => "DoWhileStatement",
        Statement::EmptyStatement(_) => "EmptyStatement",
        Statement::ForInStatement(_) => "ForInStatement",
        Statement::ForOfStatement(_) => "ForOfStatement",
        Statement::ForStatement(_) => "ForStatement",
        Statement::IfStatement(_) => "IfStatement",
        Statement::ReturnStatement(_) => "ReturnStatement",
        Statement::SwitchStatement(_) => "SwitchStatement",
        Statement::ThrowStatement(_) => "ThrowStatement",
        Statement::TryStatement(_) => "TryStatement",
        Statement::VariableDeclaration(_) => "VariableDeclaration",
        Statement::WhileStatement(_) => "WhileStatement",
        _ => "Statement",
    }
}

fn identifier_reference_ast_value(identifier: &oxc_ast::ast::IdentifierReference<'_>) -> Value {
    json!({
        "type": "Identifier",
        "name": identifier.name.as_str(),
    })
}

fn identifier_name_ast_value(identifier: &oxc_ast::ast::IdentifierName<'_>) -> Value {
    json!({
        "type": "Identifier",
        "name": identifier.name.as_str(),
    })
}

fn private_identifier_ast_value(identifier: &oxc_ast::ast::PrivateIdentifier<'_>) -> Value {
    json!({
        "type": "PrivateName",
        "name": identifier.name.as_str(),
    })
}

fn computed_member_ast_value(member: &oxc_ast::ast::ComputedMemberExpression<'_>) -> Value {
    json!({
        "type": "MemberExpression",
        "object": expression_ast_value(&member.object),
        "property": expression_ast_value(&member.expression),
        "computed": true,
        "optional": member.optional,
    })
}

fn static_member_ast_value(member: &oxc_ast::ast::StaticMemberExpression<'_>) -> Value {
    json!({
        "type": "MemberExpression",
        "object": expression_ast_value(&member.object),
        "property": identifier_name_ast_value(&member.property),
        "computed": false,
        "optional": member.optional,
    })
}

fn private_field_ast_value(member: &oxc_ast::ast::PrivateFieldExpression<'_>) -> Value {
    json!({
        "type": "MemberExpression",
        "object": expression_ast_value(&member.object),
        "property": private_identifier_ast_value(&member.field),
        "computed": false,
        "optional": member.optional,
    })
}

fn template_literal_ast_value(template: &oxc_ast::ast::TemplateLiteral<'_>) -> Value {
    json!({
        "type": "TemplateLiteral",
        "expressions": template.expressions.iter().map(expression_ast_value).collect::<Vec<_>>(),
    })
}

fn ts_expression_ast_value(kind: &str, expression: &Expression<'_>) -> Value {
    json!({
        "type": kind,
        "expression": expression_ast_value(expression),
    })
}

fn array_element_ast_value(element: &ArrayExpressionElement<'_>) -> Value {
    match element {
        ArrayExpressionElement::SpreadElement(spread) => json!({
            "type": "SpreadElement",
            "argument": expression_ast_value(&spread.argument),
        }),
        ArrayExpressionElement::Elision(_) => Value::Null,
        _ => element
            .as_expression()
            .map(expression_ast_value)
            .unwrap_or_else(|| json!({ "type": "Expression" })),
    }
}

fn argument_ast_value(argument: &Argument<'_>) -> Value {
    match argument {
        Argument::SpreadElement(spread) => json!({
            "type": "SpreadElement",
            "argument": expression_ast_value(&spread.argument),
        }),
        _ => argument
            .as_expression()
            .map(expression_ast_value)
            .unwrap_or_else(|| json!({ "type": "Expression" })),
    }
}

fn object_property_kind_ast_value(property: &ObjectPropertyKind<'_>) -> Value {
    match property {
        ObjectPropertyKind::ObjectProperty(property) => json!({
            "type": "ObjectProperty",
            "key": property_key_ast_value(&property.key),
            "value": expression_ast_value(&property.value),
            "computed": property.computed,
            "shorthand": property.shorthand,
        }),
        ObjectPropertyKind::SpreadProperty(spread) => json!({
            "type": "SpreadElement",
            "argument": expression_ast_value(&spread.argument),
        }),
    }
}

fn property_key_ast_value(key: &PropertyKey<'_>) -> Value {
    match key {
        PropertyKey::StaticIdentifier(identifier) => identifier_name_ast_value(identifier),
        PropertyKey::PrivateIdentifier(identifier) => private_identifier_ast_value(identifier),
        _ => key
            .as_expression()
            .map(expression_ast_value)
            .unwrap_or_else(|| json!({ "type": "Identifier", "name": "" })),
    }
}

fn chain_element_ast_value(element: &ChainElement<'_>) -> Value {
    match element {
        ChainElement::CallExpression(call) => json!({
            "type": "CallExpression",
            "callee": expression_ast_value(&call.callee),
            "arguments": call.arguments.iter().map(argument_ast_value).collect::<Vec<_>>(),
            "optional": call.optional,
        }),
        ChainElement::ComputedMemberExpression(member) => computed_member_ast_value(member),
        ChainElement::StaticMemberExpression(member) => static_member_ast_value(member),
        ChainElement::PrivateFieldExpression(member) => private_field_ast_value(member),
        ChainElement::TSNonNullExpression(expression) => {
            ts_expression_ast_value("TSNonNullExpression", &expression.expression)
        }
    }
}

fn assignment_target_ast_value(target: &AssignmentTarget<'_>) -> Value {
    match target {
        AssignmentTarget::AssignmentTargetIdentifier(identifier) => {
            identifier_reference_ast_value(identifier)
        }
        AssignmentTarget::ComputedMemberExpression(member) => computed_member_ast_value(member),
        AssignmentTarget::StaticMemberExpression(member) => static_member_ast_value(member),
        AssignmentTarget::PrivateFieldExpression(member) => private_field_ast_value(member),
        AssignmentTarget::TSAsExpression(expression) => {
            ts_expression_ast_value("TSAsExpression", &expression.expression)
        }
        AssignmentTarget::TSSatisfiesExpression(expression) => {
            ts_expression_ast_value("TSSatisfiesExpression", &expression.expression)
        }
        AssignmentTarget::TSNonNullExpression(expression) => {
            ts_expression_ast_value("TSNonNullExpression", &expression.expression)
        }
        AssignmentTarget::TSTypeAssertion(expression) => {
            ts_expression_ast_value("TSTypeAssertion", &expression.expression)
        }
        AssignmentTarget::ArrayAssignmentTarget(target) => json!({
            "type": "ArrayPattern",
            "elements": target.elements.iter().map(|element| {
                element
                    .as_ref()
                    .map(assignment_target_maybe_default_ast_value)
                    .unwrap_or(Value::Null)
            }).collect::<Vec<_>>(),
            "rest": target.rest.as_ref().map(|rest| json!({
                "type": "RestElement",
                "argument": assignment_target_ast_value(&rest.target),
            })),
        }),
        AssignmentTarget::ObjectAssignmentTarget(target) => json!({
            "type": "ObjectPattern",
            "properties": target.properties.iter().map(assignment_target_property_ast_value).collect::<Vec<_>>(),
            "rest": target.rest.as_ref().map(|rest| json!({
                "type": "RestElement",
                "argument": assignment_target_ast_value(&rest.target),
            })),
        }),
    }
}

fn simple_assignment_target_ast_value(target: &SimpleAssignmentTarget<'_>) -> Value {
    match target {
        SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) => {
            identifier_reference_ast_value(identifier)
        }
        SimpleAssignmentTarget::ComputedMemberExpression(member) => {
            computed_member_ast_value(member)
        }
        SimpleAssignmentTarget::StaticMemberExpression(member) => static_member_ast_value(member),
        SimpleAssignmentTarget::PrivateFieldExpression(member) => private_field_ast_value(member),
        SimpleAssignmentTarget::TSAsExpression(expression) => {
            ts_expression_ast_value("TSAsExpression", &expression.expression)
        }
        SimpleAssignmentTarget::TSSatisfiesExpression(expression) => {
            ts_expression_ast_value("TSSatisfiesExpression", &expression.expression)
        }
        SimpleAssignmentTarget::TSNonNullExpression(expression) => {
            ts_expression_ast_value("TSNonNullExpression", &expression.expression)
        }
        SimpleAssignmentTarget::TSTypeAssertion(expression) => {
            ts_expression_ast_value("TSTypeAssertion", &expression.expression)
        }
    }
}

fn assignment_target_maybe_default_ast_value(
    target: &oxc_ast::ast::AssignmentTargetMaybeDefault<'_>,
) -> Value {
    match target {
        oxc_ast::ast::AssignmentTargetMaybeDefault::AssignmentTargetWithDefault(target) => json!({
            "type": "AssignmentPattern",
            "left": assignment_target_ast_value(&target.binding),
            "right": expression_ast_value(&target.init),
        }),
        oxc_ast::ast::AssignmentTargetMaybeDefault::AssignmentTargetIdentifier(identifier) => {
            identifier_reference_ast_value(identifier)
        }
        oxc_ast::ast::AssignmentTargetMaybeDefault::ComputedMemberExpression(member) => {
            computed_member_ast_value(member)
        }
        oxc_ast::ast::AssignmentTargetMaybeDefault::StaticMemberExpression(member) => {
            static_member_ast_value(member)
        }
        oxc_ast::ast::AssignmentTargetMaybeDefault::PrivateFieldExpression(member) => {
            private_field_ast_value(member)
        }
        oxc_ast::ast::AssignmentTargetMaybeDefault::TSAsExpression(expression) => {
            ts_expression_ast_value("TSAsExpression", &expression.expression)
        }
        oxc_ast::ast::AssignmentTargetMaybeDefault::TSSatisfiesExpression(expression) => {
            ts_expression_ast_value("TSSatisfiesExpression", &expression.expression)
        }
        oxc_ast::ast::AssignmentTargetMaybeDefault::TSNonNullExpression(expression) => {
            ts_expression_ast_value("TSNonNullExpression", &expression.expression)
        }
        oxc_ast::ast::AssignmentTargetMaybeDefault::TSTypeAssertion(expression) => {
            ts_expression_ast_value("TSTypeAssertion", &expression.expression)
        }
        oxc_ast::ast::AssignmentTargetMaybeDefault::ArrayAssignmentTarget(target) => json!({
            "type": "ArrayPattern",
            "elements": target.elements.iter().map(|element| {
                element
                    .as_ref()
                    .map(assignment_target_maybe_default_ast_value)
                    .unwrap_or(Value::Null)
            }).collect::<Vec<_>>(),
        }),
        oxc_ast::ast::AssignmentTargetMaybeDefault::ObjectAssignmentTarget(target) => json!({
            "type": "ObjectPattern",
            "properties": target.properties.iter().map(assignment_target_property_ast_value).collect::<Vec<_>>(),
        }),
    }
}

fn assignment_target_property_ast_value(
    property: &oxc_ast::ast::AssignmentTargetProperty<'_>,
) -> Value {
    match property {
        oxc_ast::ast::AssignmentTargetProperty::AssignmentTargetPropertyIdentifier(property) => {
            let mut value = json!({
                "type": "ObjectProperty",
                "key": identifier_reference_ast_value(&property.binding),
                "value": identifier_reference_ast_value(&property.binding),
                "computed": false,
                "shorthand": true,
            });
            if let Some(init) = &property.init {
                value["value"] = json!({
                    "type": "AssignmentPattern",
                    "left": identifier_reference_ast_value(&property.binding),
                    "right": expression_ast_value(init),
                });
            }
            value
        }
        oxc_ast::ast::AssignmentTargetProperty::AssignmentTargetPropertyProperty(property) => {
            json!({
                "type": "ObjectProperty",
                "key": property_key_ast_value(&property.name),
                "value": assignment_target_maybe_default_ast_value(&property.binding),
                "computed": property.computed,
                "shorthand": false,
            })
        }
    }
}

fn formal_parameters_ast_values(parameters: &oxc_ast::ast::FormalParameters<'_>) -> Vec<Value> {
    let mut params = parameters
        .items
        .iter()
        .map(formal_parameter_ast_value)
        .collect::<Vec<_>>();
    if let Some(rest) = &parameters.rest {
        params.push(json!({
            "type": "RestElement",
            "argument": binding_pattern_ast_value(&rest.rest.argument),
        }));
    }
    params
}

fn formal_parameter_ast_value(parameter: &FormalParameter<'_>) -> Value {
    let pattern = binding_pattern_ast_value(&parameter.pattern);
    match &parameter.initializer {
        Some(initializer) => json!({
            "type": "AssignmentPattern",
            "left": pattern,
            "right": expression_ast_value(initializer),
        }),
        None => pattern,
    }
}

fn function_body_ast_value(body: &oxc_ast::ast::FunctionBody<'_>) -> Value {
    json!({
        "type": "BlockStatement",
        "body": body.statements.iter().map(statement_ast_value).collect::<Vec<_>>(),
    })
}

fn binding_pattern_ast_value(pattern: &BindingPattern<'_>) -> Value {
    match pattern {
        BindingPattern::BindingIdentifier(identifier) => json!({
            "type": "Identifier",
            "name": identifier.name.as_str(),
        }),
        BindingPattern::ObjectPattern(pattern) => {
            let mut properties = pattern
                .properties
                .iter()
                .map(binding_property_ast_value)
                .collect::<Vec<_>>();
            if let Some(rest) = &pattern.rest {
                properties.push(json!({
                    "type": "RestElement",
                    "argument": binding_pattern_ast_value(&rest.argument),
                }));
            }
            json!({
                "type": "ObjectPattern",
                "properties": properties,
            })
        }
        BindingPattern::ArrayPattern(pattern) => {
            let mut elements = pattern
                .elements
                .iter()
                .map(|element| {
                    element
                        .as_ref()
                        .map(binding_pattern_ast_value)
                        .unwrap_or(Value::Null)
                })
                .collect::<Vec<_>>();
            if let Some(rest) = &pattern.rest {
                elements.push(json!({
                    "type": "RestElement",
                    "argument": binding_pattern_ast_value(&rest.argument),
                }));
            }
            json!({
                "type": "ArrayPattern",
                "elements": elements,
            })
        }
        BindingPattern::AssignmentPattern(pattern) => json!({
            "type": "AssignmentPattern",
            "left": binding_pattern_ast_value(&pattern.left),
            "right": expression_ast_value(&pattern.right),
        }),
    }
}

fn binding_property_ast_value(property: &oxc_ast::ast::BindingProperty<'_>) -> Value {
    json!({
        "type": "ObjectProperty",
        "key": property_key_ast_value(&property.key),
        "value": binding_pattern_ast_value(&property.value),
        "computed": property.computed,
        "shorthand": property.shorthand,
    })
}

fn split_v_for_expression(source: &str) -> Option<(&str, &str)> {
    let mut depth = 0usize;
    let mut index = 0usize;
    while index < source.len() {
        let ch = source[index..].chars().next()?;
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ' ' if depth == 0 => {
                let rest = &source[index..];
                if rest.starts_with(" in ") {
                    return Some((source[..index].trim(), source[index + 4..].trim()));
                }
                if rest.starts_with(" of ") {
                    return Some((source[..index].trim(), source[index + 4..].trim()));
                }
            }
            _ => {}
        }
        index += ch.len_utf8();
    }
    None
}

fn split_v_for_aliases(source: &str) -> Vec<String> {
    let aliases = source
        .trim()
        .strip_prefix('(')
        .and_then(|value| value.strip_suffix(')'))
        .unwrap_or_else(|| source.trim());
    split_top_level_csv(aliases)
        .into_iter()
        .map(ToOwned::to_owned)
        .collect()
}

fn split_top_level_csv(source: &str) -> Vec<&str> {
    let mut items = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (index, ch) in source.char_indices() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                let item = source[start..index].trim();
                if !item.is_empty() {
                    items.push(item);
                }
                start = index + 1;
            }
            _ => {}
        }
    }
    let tail = source[start..].trim();
    if !tail.is_empty() {
        items.push(tail);
    }
    items
}

fn vue3_namespace_value(namespace: vuec_ast::HtmlNamespace) -> u8 {
    match namespace {
        vuec_ast::HtmlNamespace::Html => 0,
        vuec_ast::HtmlNamespace::Svg => 1,
        vuec_ast::HtmlNamespace::MathMl => 2,
    }
}

fn vue3_element_type_value(tag_type: vuec_ast::Vue3ElementType) -> u8 {
    match tag_type {
        vuec_ast::Vue3ElementType::Element => 0,
        vuec_ast::Vue3ElementType::Component => 1,
        vuec_ast::Vue3ElementType::SlotOutlet => 2,
        vuec_ast::Vue3ElementType::Template => 3,
    }
}

fn vue3_expression_loc(
    source: &str,
    base_offset: usize,
    fallback_span: &NodeSpan,
    expression: &str,
) -> Value {
    let Some(span) = fallback_span.source() else {
        return vue3_loc_stub_value();
    };
    let local_span_start = span.start.0.saturating_sub(base_offset);
    let local_span_end = span.end.0.saturating_sub(base_offset).min(source.len());
    let node_source = source
        .get(local_span_start..local_span_end)
        .unwrap_or_default();
    if let Some((inner_start, inner_end)) =
        default_interpolation_inner_trimmed_span(source, local_span_start, local_span_end)
    {
        return vue3_source_loc_value(source, inner_start, inner_end);
    }
    let trimmed = expression.trim();
    if trimmed.is_empty() {
        let inner_start = if node_source.starts_with("{{") {
            local_span_start + "{{".len()
        } else {
            local_span_start
        };
        return vue3_source_loc_value(source, inner_start, inner_start);
    }
    if let Some(local_start) = node_source.find(trimmed) {
        let start = local_span_start + local_start;
        return vue3_source_loc_value(source, start, start + trimmed.len());
    }
    vue3_loc_value(source, base_offset, fallback_span)
}

fn default_interpolation_inner_trimmed_span(
    source: &str,
    start: usize,
    end: usize,
) -> Option<(usize, usize)> {
    let slice = source.get(start..end)?;
    if !slice.starts_with("{{") || !slice.ends_with("}}") {
        return None;
    }
    let mut inner_start = start + "{{".len();
    let mut inner_end = end.saturating_sub("}}".len());
    while inner_start < inner_end
        && source
            .get(inner_start..inner_end)
            .and_then(|value| value.chars().next())
            .is_some_and(char::is_whitespace)
    {
        let ch = source[inner_start..inner_end].chars().next()?;
        inner_start += ch.len_utf8();
    }
    while inner_end > inner_start
        && source
            .get(inner_start..inner_end)
            .and_then(|value| value.chars().next_back())
            .is_some_and(char::is_whitespace)
    {
        let ch = source[inner_start..inner_end].chars().next_back()?;
        inner_end -= ch.len_utf8();
    }
    Some((inner_start, inner_end))
}

fn vue3_loc_value(source: &str, base_offset: usize, span: &NodeSpan) -> Value {
    span.source()
        .map(|span| vue3_source_span_value(source, base_offset, span))
        .unwrap_or_else(vue3_loc_stub_value)
}

fn vue3_text_loc_value(source: &str, base_offset: usize, span: &NodeSpan) -> Value {
    let Some(source_span) = span.source() else {
        return vue3_loc_stub_value();
    };
    let start = source_span.start.0.saturating_sub(base_offset);
    let end = source_span.end.0.saturating_sub(base_offset);
    if end == source.len()
        && source_span.end.0 >= source_span.start.0
        && source
            .get(start..end)
            .is_some_and(|slice| slice == "/" && source.ends_with('/'))
        && source[..start].rfind('<').is_some_and(|tag_start| {
            source
                .get(tag_start..)
                .is_some_and(|slice| slice.starts_with('<') && !slice.contains('>'))
        })
    {
        return vue3_source_signed_start_loc_value(source, -1, end);
    }
    vue3_source_span_value(source, base_offset, source_span)
}

fn vue3_source_span_value(source: &str, base_offset: usize, span: Span) -> Value {
    let start = span.start.0.saturating_sub(base_offset);
    let end = span.end.0.saturating_sub(base_offset);
    vue3_source_loc_value(source, start, end)
}

fn vue3_source_signed_start_loc_value(source: &str, start: isize, end: usize) -> Value {
    let local_start = if start < 0 && end <= source.len() {
        end.saturating_sub(1)
    } else {
        start.max(0) as usize
    };
    let local_end = end.min(source.len()).max(local_start);
    json!({
        "start": vue3_signed_position(source, start),
        "end": vue3_position(source, end),
        "source": source.get(local_start..local_end).unwrap_or_default(),
    })
}

fn vue3_source_loc_value(source: &str, start: usize, end: usize) -> Value {
    let local_start = start.min(source.len());
    let local_end = end.min(source.len()).max(local_start);
    json!({
        "start": vue3_position(source, start),
        "end": vue3_position(source, end),
        "source": source.get(local_start..local_end).unwrap_or_default(),
    })
}

fn vue3_position(source: &str, offset: usize) -> Value {
    let mut line = 1usize;
    let mut column = 1usize;
    let mut byte_index = 0usize;
    let mut utf16_offset = 0usize;
    for ch in source.chars() {
        if byte_index >= offset {
            break;
        }
        byte_index += ch.len_utf8();
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += ch.len_utf16();
        }
        utf16_offset += ch.len_utf16();
    }
    if offset > byte_index {
        let extra = offset - byte_index;
        column += extra;
        utf16_offset += extra;
    }
    json!({
        "offset": utf16_offset,
        "line": line,
        "column": column,
    })
}

fn vue3_signed_position(source: &str, offset: isize) -> Value {
    if offset >= 0 {
        return vue3_position(source, offset as usize);
    }
    let _ = source;
    json!({
        "offset": offset,
        "line": 1,
        "column": 1isize + offset,
    })
}

fn vue3_loc_stub_value() -> Value {
    json!({
        "start": { "line": 1, "column": 1, "offset": 0 },
        "end": { "line": 1, "column": 1, "offset": 0 },
        "source": "",
    })
}

fn sfc_template_options(value: Option<&Value>) -> SfcTemplateCompileOptions {
    let mut options = SfcTemplateCompileOptions::default();
    let Some(value) = value else {
        return options;
    };
    options.id = value
        .get("id")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    options.ssr = bool_option(value, "ssr", options.ssr);
    options.slotted = bool_option(value, "slotted", options.slotted);
    options.is_prod = bool_option(
        value,
        "isProd",
        bool_option(value, "is_prod", options.is_prod),
    );
    options.transform_asset_urls =
        transform_asset_urls_enabled_with_compiler_fallback(value, options.transform_asset_urls);
    options.asset_url_options =
        asset_url_options_with_compiler_fallback(value, options.asset_url_options);
    options.scope_id = value
        .get("scopeId")
        .or_else(|| value.get("scope_id"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    options
}

fn sfc_script_options(value: Option<&Value>) -> SfcScriptCompileOptions {
    let mut options = SfcScriptCompileOptions::default();
    let Some(value) = value else {
        return options;
    };
    options.id = value
        .get("id")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    options.inline_template = bool_option(
        value,
        "inlineTemplate",
        bool_option(value, "inline_template", options.inline_template),
    );
    options.ref_sugar = bool_option(
        value,
        "refSugar",
        bool_option(value, "ref_sugar", options.ref_sugar),
    );
    options.is_prod = bool_option(
        value,
        "isProd",
        bool_option(value, "is_prod", options.is_prod),
    );
    options.emit_script_setup_marker = bool_option(
        value,
        "__vuecEmitScriptSetupMarker",
        bool_option(
            value,
            "emit_script_setup_marker",
            options.emit_script_setup_marker,
        ),
    );
    options
}

fn sfc_style_options(value: Option<&Value>) -> SfcStyleCompileOptions {
    let mut options = SfcStyleCompileOptions::default();
    let Some(value) = value else {
        return options;
    };
    options.id = value
        .get("id")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    options.scoped = bool_option(value, "scoped", options.scoped);
    options.modules = bool_option(
        value,
        "modules",
        bool_option(value, "module", options.modules),
    );
    if let Some(modules_options) = value
        .get("modulesOptions")
        .or_else(|| value.get("modules_options"))
    {
        if let Ok(parsed) = serde_json::from_value(modules_options.clone()) {
            options.modules_options = parsed;
        }
    }
    options.is_prod = bool_option(
        value,
        "isProd",
        bool_option(value, "is_prod", options.is_prod),
    );
    if let Some(style) = value
        .get("__vuecCssVarNameStyle")
        .or_else(|| value.get("cssVarNameStyle"))
        .or_else(|| value.get("css_var_name_style"))
        .and_then(Value::as_str)
    {
        options.css_var_name_style = match style {
            "vue27Legacy" | "vue27_legacy" | "legacy" => SfcCssVarNameStyle::Vue27Legacy,
            _ => SfcCssVarNameStyle::Vue3Escaped,
        };
    }
    options.css_var_ignore_line_comments = bool_option(
        value,
        "__vuecCssVarIgnoreLineComments",
        bool_option(
            value,
            "cssVarIgnoreLineComments",
            bool_option(
                value,
                "css_var_ignore_line_comments",
                options.css_var_ignore_line_comments,
            ),
        ),
    );
    options.source_map = value.get("map").is_some_and(|map| !map.is_null())
        || bool_option(
            value,
            "sourceMap",
            bool_option(value, "source_map", options.source_map),
        );
    options.preprocess_lang = value
        .get("preprocessLang")
        .or_else(|| value.get("preprocess_lang"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    if let Some(preprocess_options) = value
        .get("preprocessOptions")
        .or_else(|| value.get("preprocess_options"))
    {
        if let Ok(parsed) = serde_json::from_value(preprocess_options.clone()) {
            options.preprocess_options = parsed;
        }
    }
    options.warn_deprecated_scoped_selectors = bool_option(
        value,
        "__vuecWarnDeprecatedScopedSelectors",
        bool_option(
            value,
            "warnDeprecatedScopedSelectors",
            bool_option(
                value,
                "warn_deprecated_scoped_selectors",
                options.warn_deprecated_scoped_selectors,
            ),
        ),
    );
    options.vars = value
        .get("vars")
        .and_then(Value::as_array)
        .map(|vars| {
            vars.iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default();
    options
}

fn bool_option(value: &Value, name: &str, fallback: bool) -> bool {
    value.get(name).and_then(Value::as_bool).unwrap_or(fallback)
}

fn string_option(value: &Value, name: &str, fallback: &str) -> String {
    value
        .get(name)
        .and_then(Value::as_str)
        .unwrap_or(fallback)
        .to_string()
}

fn string_map_option(value: &Value, name: &str) -> Option<BTreeMap<String, String>> {
    value.get(name).and_then(Value::as_object).map(|object| {
        object
            .iter()
            .filter_map(|(key, value)| value.as_str().map(|value| (key.clone(), value.into())))
            .collect()
    })
}

fn string_array_option(value: &Value, name: &str) -> Vec<String> {
    value
        .get(name)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(ToOwned::to_owned)
        .collect()
}

#[napi(js_name = "apiManifest")]
/// Returns the native binding API manifest as JSON.
pub fn api_manifest() -> Result<String> {
    to_json_string(json!({
            "package": "@vuec-rs/native",
            "version": env!("CARGO_PKG_VERSION"),
            "exports": [
                "version",
                "compileVue2",
                "compileToFunctionsVue2",
                "compileSsrVue2",
                "generateCodeFrameVue2",
                "callVue2Bridge",
                "rewriteDefaultVue27",
                "baseCompileVue3",
                "baseParseVue3",
                "generateVue3Core",
                "callVue3CoreProjection",
                "callVue3DomProjection",
                "compileVue3Dom",
                "parseVue3Dom",
                "compileVue3Ssr",
                "parseSfc",
                "parseVue27SfcComponent",
                "compileSfcTemplate",
                "compileSfcTemplateSource",
                "compileSfcScript",
                "compileVue27SfcTemplate",
                "compileVue27SfcScript",
                "compileSfcStyle"
            ]
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vue3_options_accepts_public_keys() {
        let options = vue3_options(Some(&json!({
            "mode": "module",
            "prefixIdentifiers": true,
            "sourceMap": true,
            "ssrCssVars": "{ \"--x\": (foo) }",
            "scopeId": "data-v-test"
        })));
        assert_eq!(options.mode, "module");
        assert!(options.prefix_identifiers);
        assert!(options.source_map);
        assert_eq!(options.ssr_css_vars.as_deref(), Some("{ \"--x\": (foo) }"));
        assert_eq!(options.scope_id.as_deref(), Some("data-v-test"));
    }

    #[test]
    fn vue3_options_accepts_napi_predicate_projection_keys() {
        let options = vue3_options(Some(&json!({
            "__vuecVoidTags": ["img"],
            "__vuecNativeTags": ["div"],
            "__vuecCustomElements": ["x-thing"],
            "__vuecBuiltInComponents": ["Transition"],
            "parseMode": "sfc"
        })));
        assert_eq!(options.void_tags, vec!["img"]);
        assert_eq!(options.native_tags, Some(vec!["div".into()]));
        assert_eq!(options.custom_elements, vec!["x-thing"]);
        assert_eq!(options.built_in_components, vec!["Transition"]);
        assert!(options.sfc_parse_mode);
        assert_eq!(options.sfc_plain_template_langs, vec!["pug"]);
    }

    #[test]
    fn vue3_dom_options_accept_asset_url_projection_keys() {
        let default_options = DomCompilerOptions::default();
        let options = json!({
            "transformAssetUrls": {
                "base": "/cdn",
                "includeAbsolute": true,
                "tags": {
                    "foo": ["bar"],
                    "picture": "srcset"
                }
            }
        });

        assert!(transform_asset_urls_enabled(
            &options,
            default_options.transform_asset_urls
        ));
        let parsed = asset_url_options(&options, default_options.asset_url_options);
        assert_eq!(parsed.base.as_deref(), Some("/cdn"));
        assert!(parsed.include_absolute);
        assert_eq!(parsed.tags.get("foo"), Some(&vec!["bar".to_string()]));
        assert_eq!(
            parsed.tags.get("picture"),
            Some(&vec!["srcset".to_string()])
        );
    }

    #[test]
    fn vue3_sfc_template_options_accept_asset_url_projection_keys() {
        let options = sfc_template_options(Some(&json!({
            "transformAssetUrls": {
                "foo": ["bar"]
            }
        })));

        assert!(options.transform_asset_urls);
        assert_eq!(
            options.asset_url_options.tags.get("foo"),
            Some(&vec!["bar".to_string()])
        );
    }

    #[test]
    fn vue3_sfc_template_options_use_compiler_asset_fallback() {
        let options = sfc_template_options(Some(&json!({
            "compilerOptions": {
                "transformAssetUrls": false
            }
        })));

        assert!(!options.transform_asset_urls);
    }

    #[test]
    fn vue3_dom_options_ignore_compiler_asset_fallback() {
        let fallback = DomCompilerOptions::default().transform_asset_urls;
        let options = json!({
            "compilerOptions": {
                "transformAssetUrls": false
            }
        });

        assert_eq!(transform_asset_urls_enabled(&options, fallback), fallback);
    }

    #[test]
    fn vue3_parse_diagnostics_include_public_messages() {
        let source = "<div>";
        let options = Vue3CompilerOptions::default();
        let template = template_source(source, &Value::Null);
        let ast = Vue3Dialect::base_parse(template, &options);
        let diagnostics = vue3_parse_diagnostics(&ast, source, 0, &options);
        assert_eq!(diagnostics[0]["code"], json!(24));
        assert_eq!(
            diagnostics[0]["message"],
            json!("Element is missing end tag.")
        );
    }

    #[test]
    fn vue3_parse_projects_interpolation_expression_ast() {
        let options = vue3_prefix_identifier_options();
        let ast = vue3_public_parse_for_test("{{ a + b }}", &options);

        assert_eq!(
            ast["children"][0]["content"]["ast"]["type"],
            json!("BinaryExpression")
        );
    }

    #[test]
    fn vue3_parse_projects_directive_expression_ast() {
        let options = vue3_prefix_identifier_options();
        let ast =
            vue3_public_parse_for_test(r#"<div :[key+1]="foo()" @click="a++;b++" />"#, &options);

        let props = &ast["children"][0]["props"];
        assert_eq!(props[0]["arg"]["ast"]["type"], json!("BinaryExpression"));
        assert_eq!(props[0]["exp"]["ast"]["type"], json!("CallExpression"));
        assert_eq!(props[1]["exp"]["ast"]["type"], json!("Program"));
        assert_eq!(
            props[1]["exp"]["ast"]["body"][0]["type"],
            json!("ExpressionStatement")
        );
        assert_eq!(
            props[1]["exp"]["ast"]["body"][1]["type"],
            json!("ExpressionStatement")
        );
    }

    #[test]
    fn vue3_parse_projects_slot_params_ast() {
        let options = vue3_prefix_identifier_options();
        let ast = vue3_public_parse_for_test(r#"<Comp #foo="{ a, b }" />"#, &options);

        assert_eq!(
            ast["children"][0]["props"][0]["exp"]["ast"]["type"],
            json!("ArrowFunctionExpression")
        );
    }

    #[test]
    fn vue3_parse_projects_v_for_parse_result_ast() {
        let options = vue3_prefix_identifier_options();
        let ast = vue3_public_parse_for_test(
            r#"<div v-for="({ a, b }, key, index) of a.b" />"#,
            &options,
        );

        let result = &ast["children"][0]["props"][0]["forParseResult"];
        assert_eq!(result["source"]["ast"]["type"], json!("MemberExpression"));
        assert_eq!(
            result["value"]["ast"]["type"],
            json!("ArrowFunctionExpression")
        );
        assert!(result["key"]["ast"].is_null());
        assert!(result["index"]["ast"].is_null());
    }

    #[test]
    fn vue2_options_accepts_sparse_public_keys() {
        let options = vue2_options(json!({
            "comments": true,
            "delimiters": ["[[", "]]"],
            "whitespace": "condense",
            "preserveWhitespace": false,
            "shouldDecodeNewlinesForHref": true
        }));
        assert!(options.comments);
        assert_eq!(options.delimiters, Some(["[[".into(), "]]".into()]));
        assert_eq!(options.whitespace.as_deref(), Some("condense"));
        assert!(!options.preserve_whitespace);
        assert!(options.should_decode_newlines_for_href);
        assert!(options.warn);
        assert!(options.optimize);
    }

    #[test]
    fn vue27_sfc_template_code_wraps_vue2_render_shape() {
        let code = vue27_template_code("with(this){return _c('div',[_v(_s(msg))])}", &[]);
        assert!(code.contains("var _vm = this"));
        assert!(code.contains("return _c(\"div\", [_vm._v(_vm._s(_vm.msg))])"));
        assert!(code.contains("render._withStripped = true"));
    }

    fn vue3_prefix_identifier_options() -> Vue3CompilerOptions {
        Vue3CompilerOptions {
            prefix_identifiers: true,
            ..Vue3CompilerOptions::default()
        }
    }

    fn vue3_public_parse_for_test(source: &str, options: &Vue3CompilerOptions) -> Value {
        let template = template_source(source, &Value::Null);
        let ast = Vue3Dialect::base_parse(template.clone(), options);
        vue3_public_parse_ast(&ast, &template.source, template.base_offset, options)
    }
}
