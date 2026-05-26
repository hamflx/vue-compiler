#![forbid(unsafe_code)]

use serde::Serialize;
use serde_json::{json, Value};
use vuec_sfc::{
    SfcCompiler, SfcScriptCompileOptions, SfcStyleCompileOptions, SfcTemplateCompileOptions,
};
use vuec_source::FileId;
use vuec_vue2::Vue2CompileOptions;
use vuec_vue3_core::{TemplateSource, Vue3CompilerOptions};
use vuec_vue3_dom::{apply_dom_parser_defaults, compile as compile_dom, DomCompilerOptions};
use vuec_vue3_ssr::{compile as compile_ssr, SsrCompilerOptions};
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[global_allocator]
static WASM_ALLOCATOR: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn init_wasm_runtime() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen(js_name = version)]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[wasm_bindgen(js_name = compileVue2)]
pub fn compile_vue2(template: &str, options_json: Option<String>) -> String {
    let options = parse_options(options_json);
    result_to_json(vuec_vue2::compile(template, vue2_options(&options)))
}

#[wasm_bindgen(js_name = compileVue3Dom)]
pub fn compile_vue3_dom(source: &str, options_json: Option<String>) -> String {
    let options = parse_options(options_json);
    let template = template_source(source, &options);
    let mut core = vue3_options(&options);
    apply_dom_parser_defaults(&mut core);
    result_to_json(compile_dom(
        template,
        DomCompilerOptions {
            core,
            ..DomCompilerOptions::default()
        },
    ))
}

#[wasm_bindgen(js_name = compileVue3Ssr)]
pub fn compile_vue3_ssr(source: &str, options_json: Option<String>) -> String {
    let options = parse_options(options_json);
    let template = template_source(source, &options);
    let mut core = vue3_options(&options);
    apply_dom_parser_defaults(&mut core);
    result_to_json(compile_ssr(
        template,
        SsrCompilerOptions {
            scope_id: string_option(&options, "scopeId")
                .or_else(|| string_option(&options, "scope_id")),
            slotted: bool_option(&options, "slotted", false),
            mode_is_explicit: options.get("mode").is_some(),
            core,
            ..SsrCompilerOptions::default()
        },
    ))
}

#[wasm_bindgen(js_name = parseSfc)]
pub fn parse_sfc(source: &str, options_json: Option<String>) -> String {
    let options = parse_options(options_json);
    let filename = string_option(&options, "filename").unwrap_or_else(|| "anonymous.vue".into());
    let mut compiler = SfcCompiler::new();
    result_to_json(compiler.parse(filename, source))
}

#[wasm_bindgen(js_name = compileSfcTemplate)]
pub fn compile_sfc_template(source: &str, options_json: Option<String>) -> String {
    let options = parse_options(options_json);
    let filename = string_option(&options, "filename").unwrap_or_else(|| "anonymous.vue".into());
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(filename, source);
    result_to_json(compiler.compile_template(&descriptor, sfc_template_options(&options)))
}

#[wasm_bindgen(js_name = compileSfcTemplateSource)]
pub fn compile_sfc_template_source(source: &str, options_json: Option<String>) -> String {
    let options = parse_options(options_json);
    let filename =
        string_option(&options, "filename").unwrap_or_else(|| "template.vue.html".into());
    let compiler = SfcCompiler::new();
    result_to_json(compiler.compile_template_source(
        filename,
        source,
        sfc_template_options(&options),
    ))
}

#[wasm_bindgen(js_name = compileSfcScript)]
pub fn compile_sfc_script(source: &str, options_json: Option<String>) -> String {
    let options = parse_options(options_json);
    let filename = string_option(&options, "filename").unwrap_or_else(|| "anonymous.vue".into());
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(filename, source);
    result_to_json(compiler.compile_script(&descriptor, sfc_script_options(&options)))
}

#[wasm_bindgen(js_name = compileSfcStyle)]
pub fn compile_sfc_style(source: &str, options_json: Option<String>) -> String {
    let options = parse_options(options_json);
    let filename = string_option(&options, "filename").unwrap_or_else(|| "anonymous.vue".into());
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(filename, source);
    result_to_json(compiler.compile_style(&descriptor, sfc_style_options(&options)))
}

pub fn compile_vue2_json(template: &str, options: Value) -> Value {
    serde_json::to_value(vuec_vue2::compile(template, vue2_options(&options)))
        .unwrap_or_else(error_value)
}

pub fn compile_vue3_dom_json(source: &str, options: Value) -> Value {
    let template = template_source(source, &options);
    let mut core = vue3_options(&options);
    apply_dom_parser_defaults(&mut core);
    serde_json::to_value(compile_dom(
        template,
        DomCompilerOptions {
            core,
            ..DomCompilerOptions::default()
        },
    ))
    .unwrap_or_else(error_value)
}

pub fn compile_sfc_template_json(source: &str, options: Value) -> Value {
    let filename = string_option(&options, "filename").unwrap_or_else(|| "anonymous.vue".into());
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(filename, source);
    serde_json::to_value(compiler.compile_template(&descriptor, sfc_template_options(&options)))
        .unwrap_or_else(error_value)
}

fn parse_options(options_json: Option<String>) -> Value {
    options_json
        .filter(|json| !json.trim().is_empty())
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or(Value::Null)
}

fn result_to_json<T: Serialize>(value: T) -> String {
    serde_json::to_string(&value)
        .unwrap_or_else(|err| serde_json::to_string(&error_value(err)).unwrap_or_default())
}

fn error_value(error: impl std::fmt::Display) -> Value {
    json!({
        "errors": [format!("serialization failed: {error}")],
    })
}

fn template_source(source: &str, options: &Value) -> TemplateSource {
    TemplateSource {
        filename: string_option(options, "filename").unwrap_or_else(|| "anonymous.vue".into()),
        source: source.into(),
        file_id: FileId(0),
        base_offset: 0,
    }
}

fn vue2_options(value: &Value) -> Vue2CompileOptions {
    let mut options = Vue2CompileOptions::default();
    options.output_source_range = bool_option(
        value,
        "outputSourceRange",
        bool_option(value, "output_source_range", options.output_source_range),
    );
    options.comments = bool_option(value, "comments", options.comments);
    options.preserve_whitespace = bool_option(
        value,
        "preserveWhitespace",
        bool_option(value, "preserve_whitespace", options.preserve_whitespace),
    );
    options.optimize = bool_option(value, "optimize", options.optimize);
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
    options
}

fn vue3_options(value: &Value) -> Vue3CompilerOptions {
    let mut options = Vue3CompilerOptions::default();
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
    options.comments = bool_option(value, "comments", options.comments);
    options.scope_id = string_option(value, "scopeId").or_else(|| string_option(value, "scope_id"));
    if let Some(mode) = string_option(value, "mode") {
        options.mode = mode;
    } else if options.prefix_identifiers {
        options.mode = "function".into();
    }
    if let Some(whitespace) = string_option(value, "whitespace") {
        options.whitespace = whitespace;
    }
    if let Some(delimiters) = value.get("delimiters").and_then(Value::as_array) {
        if delimiters.len() == 2 {
            if let (Some(open), Some(close)) = (delimiters[0].as_str(), delimiters[1].as_str()) {
                options.delimiters = Some([open.into(), close.into()]);
            }
        }
    }
    options
}

fn sfc_template_options(value: &Value) -> SfcTemplateCompileOptions {
    let mut options = SfcTemplateCompileOptions::default();
    options.id = string_option(value, "id");
    options.ssr = bool_option(value, "ssr", options.ssr);
    options.slotted = bool_option(value, "slotted", options.slotted);
    options.is_prod = bool_option(
        value,
        "isProd",
        bool_option(value, "is_prod", options.is_prod),
    );
    options.scope_id = string_option(value, "scopeId").or_else(|| string_option(value, "scope_id"));
    options.transform_asset_urls = bool_option(
        value,
        "transformAssetUrls",
        bool_option(value, "transform_asset_urls", options.transform_asset_urls),
    );
    options
}

fn sfc_script_options(value: &Value) -> SfcScriptCompileOptions {
    let mut options = SfcScriptCompileOptions::default();
    options.id = string_option(value, "id");
    options.inline_template = bool_option(
        value,
        "inlineTemplate",
        bool_option(value, "inline_template", options.inline_template),
    );
    options.is_prod = bool_option(
        value,
        "isProd",
        bool_option(value, "is_prod", options.is_prod),
    );
    options
}

fn sfc_style_options(value: &Value) -> SfcStyleCompileOptions {
    let mut options = SfcStyleCompileOptions::default();
    options.id = string_option(value, "id");
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
    options.preprocess_lang =
        string_option(value, "preprocessLang").or_else(|| string_option(value, "preprocess_lang"));
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

fn string_option(value: &Value, name: &str) -> Option<String> {
    value
        .get(name)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_vue3_template_to_json_boundary() {
        let value = compile_vue3_dom_json(
            "<div>{{ msg }}</div>",
            json!({ "mode": "module", "prefixIdentifiers": true, "sourceMap": true }),
        );
        assert!(value["code"]
            .as_str()
            .unwrap_or_default()
            .contains("_toDisplayString(_ctx.msg)"));
        assert_eq!(value["map"]["version"], 3);
    }

    #[test]
    fn compiles_sfc_template_to_json_boundary() {
        let value = compile_sfc_template_json(
            "<template><div>{{ msg }}</div></template>",
            json!({ "filename": "App.vue" }),
        );
        assert!(value["code"]
            .as_str()
            .unwrap_or_default()
            .contains("export function render"));
    }

    #[test]
    fn wasm_exports_return_json_strings_without_js_runtime() {
        let value: Value =
            serde_json::from_str(&compile_vue2("<p>{{ msg }}</p>", None)).expect("json");
        assert!(value["render"]
            .as_str()
            .unwrap_or_default()
            .contains("_s(msg)"));
    }
}
