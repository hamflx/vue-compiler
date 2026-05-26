#![deny(unsafe_code)]

use napi::{bindgen_prelude::Unknown, Env, Result};
use napi_derive::napi;
use serde_json::{json, Value};
use vuec_sfc::{
    SfcCompiler, SfcScriptCompileOptions, SfcStyleCompileOptions, SfcTemplateCompileOptions,
};
use vuec_source::FileId;
use vuec_vue2::Vue2CompileOptions;
use vuec_vue3_core::{TemplateSource, Vue3CompilerOptions};
use vuec_vue3_dom::{apply_dom_parser_defaults, compile as compile_dom, DomCompilerOptions};
use vuec_vue3_ssr::{compile as compile_ssr, SsrCompilerOptions};

#[napi(js_name = "version")]
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[napi(js_name = "compileVue2")]
pub fn compile_vue2(env: Env, template: String, options: Option<Unknown>) -> Result<String> {
    to_json_string(vuec_vue2::compile(
        &template,
        vue2_options(from_js_options(&env, options)?),
    ))
}

#[napi(js_name = "compileToFunctionsVue2")]
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

#[napi(js_name = "compileVue3Dom")]
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

#[napi(js_name = "baseCompileVue3")]
pub fn base_compile_vue3(env: Env, source: String, options: Option<Unknown>) -> Result<String> {
    compile_vue3_dom(env, source, options)
}

#[napi(js_name = "compileVue3Ssr")]
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
pub fn parse_sfc(env: Env, source: String, options: Option<Unknown>) -> Result<String> {
    let raw_options = from_js_options(&env, options)?;
    let filename = string_option(&raw_options, "filename", "anonymous.vue");
    let mut compiler = SfcCompiler::new();
    to_json_string(compiler.parse(filename, &source))
}

#[napi(js_name = "compileSfcTemplate")]
pub fn compile_sfc_template(env: Env, source: String, options: Option<Unknown>) -> Result<String> {
    let raw_options = from_js_options(&env, options)?;
    let filename = string_option(&raw_options, "filename", "anonymous.vue");
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(filename, &source);
    let result = compiler.compile_template(&descriptor, sfc_template_options(Some(&raw_options)));
    to_json_string(result)
}

#[napi(js_name = "compileSfcScript")]
pub fn compile_sfc_script(env: Env, source: String, options: Option<Unknown>) -> Result<String> {
    let raw_options = from_js_options(&env, options)?;
    let filename = string_option(&raw_options, "filename", "anonymous.vue");
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(filename, &source);
    let result = compiler.compile_script(&descriptor, sfc_script_options(Some(&raw_options)));
    to_json_string(result)
}

#[napi(js_name = "compileSfcStyle")]
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
    serde_json::from_value::<Vue2CompileOptions>(value).unwrap_or_default()
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
    options.slotted = bool_option(value, "slotted", options.slotted);
    options.inline = bool_option(value, "inline", options.inline);
    options.ssr = bool_option(value, "ssr", options.ssr);
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
    options
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

#[napi(js_name = "apiManifest")]
pub fn api_manifest() -> Result<String> {
    to_json_string(json!({
            "package": "@vuec-rs/native",
            "version": env!("CARGO_PKG_VERSION"),
            "exports": [
                "version",
                "compileVue2",
                "compileToFunctionsVue2",
                "baseCompileVue3",
                "compileVue3Dom",
                "compileVue3Ssr",
                "parseSfc",
                "compileSfcTemplate",
                "compileSfcScript",
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
}
