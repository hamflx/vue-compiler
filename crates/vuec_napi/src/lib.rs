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
use serde_json::Map;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use vuec_ast::{NodeSpan, Vue3Ast, Vue3AstKind, Vue3Expression, Vue3Prop};
use vuec_sfc::{
    SfcCompiler, SfcScriptCompileOptions, SfcStyleCompileOptions, SfcTemplateCompileOptions,
    Vue27ParseComponentOptions, Vue27RewriteDefaultOptions, Vue27SfcPad,
    Vue27TemplatePreprocessOptions,
};
use vuec_source::{FileId, Span};
use vuec_vue2::{
    Vue2CompileOptions, Vue2CompiledResult, Vue2Element, Vue2Error,
    Vue2SfcAssetUrlTransformOptions, Vue2Warning,
};
use vuec_vue3_core::{TemplateSource, Vue3CompilerOptions, Vue3Dialect};
use vuec_vue3_dom::{
    apply_dom_parser_defaults, compile as compile_dom, parse as parse_dom, DomCompilerOptions,
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
    apply_dom_parser_defaults(&mut core);
    let dom_options = DomCompilerOptions {
        core,
        ..DomCompilerOptions::default()
    };
    to_json_string(compile_dom(template, dom_options))
}

#[napi(js_name = "parseVue3Dom")]
/// Parses a Vue 3 DOM template and returns public AST JSON.
pub fn parse_vue3_dom(env: Env, source: String, options: Option<Unknown>) -> Result<String> {
    let raw_options = from_js_options(&env, options)?;
    let template = template_source(&source, &raw_options);
    let mut core = vue3_options(Some(&raw_options));
    apply_dom_parser_defaults(&mut core);
    let dom_options = DomCompilerOptions {
        core,
        ..DomCompilerOptions::default()
    };
    let ast = parse_dom(template.clone(), &dom_options);
    to_json_string(vue3_public_parse_ast(
        &ast,
        &template.source,
        template.base_offset,
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
    ))
}

#[napi(js_name = "generateVue3Core")]
/// Generates Vue 3 render code from a hydrated public AST value.
pub fn generate_vue3_core(env: Env, ast: Unknown, options: Option<Unknown>) -> Result<String> {
    let ast = from_js_options(&env, Some(ast))?;
    let options = vue3_options(Some(&from_js_options(&env, options)?));
    to_json_string(vuec_vue3_core::generate_public_ast(&ast, &options))
}

#[napi(js_name = "compileVue3Ssr")]
/// Compiles a Vue 3 template for SSR and returns a JSON string result.
pub fn compile_vue3_ssr(env: Env, source: String, options: Option<Unknown>) -> Result<String> {
    let raw_options = from_js_options(&env, options)?;
    let template = template_source(&source, &raw_options);
    let mut core = vue3_options(Some(&raw_options));
    apply_dom_parser_defaults(&mut core);
    let ssr_options = SsrCompilerOptions {
        core,
        scope_id: raw_options
            .get("scopeId")
            .or_else(|| raw_options.get("scope_id"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        slotted: bool_option(&raw_options, "slotted", false),
        mode_is_explicit: raw_options.get("mode").is_some(),
        ..SsrCompilerOptions::default()
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
        base_offset: 0,
    }
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
    options.stringify_static = bool_option(
        value,
        "stringifyStatic",
        bool_option(value, "stringify_static", options.stringify_static),
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

fn vue3_public_parse_ast(ast: &Vue3Ast, source: &str, base_offset: usize) -> Value {
    json!({
        "type": 0,
        "source": source,
        "children": vue3_public_children(ast, ast.root, source, base_offset),
        "helpers": [],
        "components": [],
        "directives": [],
        "hoists": [],
        "imports": vue3_public_root_imports(ast),
        "cached": [],
        "temps": 0,
        "codegenNode": Value::Null,
        "loc": ast.root_node().map(|node| vue3_loc_value(source, base_offset, &node.span)).unwrap_or_else(vue3_loc_stub_value),
    })
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
) -> Vec<Value> {
    ast.node(parent)
        .map(|node| {
            node.children
                .iter()
                .filter_map(|child| vue3_public_node(ast, *child, source, base_offset))
                .collect()
        })
        .unwrap_or_default()
}

fn vue3_public_node(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    source: &str,
    base_offset: usize,
) -> Option<Value> {
    let node = ast.node(node_id)?;
    Some(match &node.kind {
        Vue3AstKind::Root(root) => json!({
            "type": 0,
            "source": source,
            "children": vue3_public_children(ast, node_id, source, base_offset),
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
        Vue3AstKind::Element(element) => json!({
            "type": 1,
            "tag": element.tag,
            "ns": vue3_namespace_value(element.ns),
            "tagType": vue3_element_type_value(element.tag_type),
            "props": element.props.iter().map(|prop| vue3_prop_value(source, base_offset, prop)).collect::<Vec<_>>(),
            "children": vue3_public_children(ast, node_id, source, base_offset),
            "loc": vue3_loc_value(source, base_offset, &node.span),
            "codegenNode": Value::Null,
            "isSelfClosing": if element.self_closing { json!(true) } else { Value::Null },
        }),
        Vue3AstKind::Text(text) => json!({
            "type": 2,
            "content": text.value,
            "loc": vue3_loc_value(source, base_offset, &node.span),
        }),
        Vue3AstKind::Comment(comment) => json!({
            "type": 3,
            "content": comment.value,
            "loc": vue3_loc_value(source, base_offset, &node.span),
        }),
        Vue3AstKind::Interpolation(interpolation) => json!({
            "type": 5,
            "content": vue3_expression_value(source, base_offset, &interpolation.expression, &node.span, false),
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

fn vue3_prop_value(source: &str, base_offset: usize, prop: &Vue3Prop) -> Value {
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
        Vue3Prop::Directive(dir) => json!({
            "type": 7,
            "name": dir.name,
            "rawName": dir.raw_name,
            "exp": dir.exp.as_ref().map(|exp| vue3_expression_value(source, base_offset, exp, &span_to_node_span(dir.exp_span), false)),
            "arg": dir.arg.as_ref().map(|arg| vue3_expression_value(source, base_offset, arg, &span_to_node_span(dir.arg_span), !dir.is_dynamic_arg)),
            "modifiers": dir.modifiers.iter().enumerate().map(|(index, modifier)| {
                let loc = dir
                    .modifier_spans
                    .get(index)
                    .map(|span| vue3_loc_value(source, base_offset, span))
                    .unwrap_or_else(vue3_loc_stub_value);
                vue3_simple_expression_value(modifier, true, loc)
            }).collect::<Vec<_>>(),
            "loc": dir.span.map(|span| vue3_source_span_value(source, base_offset, span)).unwrap_or_else(vue3_loc_stub_value),
        }),
    }
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
) -> Value {
    let raw = expression.source_string();
    let content = raw.trim();
    let loc = vue3_expression_loc(source, base_offset, fallback_span, content);
    vue3_simple_expression_value(content, is_static, loc)
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
    trimmed: &str,
) -> Value {
    let Some(span) = fallback_span.source() else {
        return vue3_loc_stub_value();
    };
    let start = span.start.0.saturating_sub(base_offset);
    let end = span.end.0.saturating_sub(base_offset).min(source.len());
    let Some(slice) = source.get(start..end) else {
        return vue3_loc_value(source, base_offset, fallback_span);
    };
    if let Some(offset) = slice.find(trimmed) {
        let local_start = start + offset;
        return vue3_source_loc_value(source, local_start, local_start + trimmed.len());
    }
    vue3_loc_value(source, base_offset, fallback_span)
}

fn vue3_loc_value(source: &str, base_offset: usize, span: &NodeSpan) -> Value {
    span.source()
        .map(|span| vue3_source_span_value(source, base_offset, span))
        .unwrap_or_else(vue3_loc_stub_value)
}

fn vue3_source_span_value(source: &str, base_offset: usize, span: Span) -> Value {
    let start = span.start.0.saturating_sub(base_offset);
    let end = span.end.0.saturating_sub(base_offset);
    vue3_source_loc_value(source, start, end)
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
    options.is_prod = bool_option(
        value,
        "isProd",
        bool_option(value, "is_prod", options.is_prod),
    );
    options.source_map = bool_option(
        value,
        "sourceMap",
        bool_option(value, "source_map", options.source_map),
    );
    options.preprocess_lang = value
        .get("preprocessLang")
        .or_else(|| value.get("preprocess_lang"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
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
            "scopeId": "data-v-test"
        })));
        assert_eq!(options.mode, "module");
        assert!(options.prefix_identifiers);
        assert!(options.source_map);
        assert_eq!(options.scope_id.as_deref(), Some("data-v-test"));
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
}
