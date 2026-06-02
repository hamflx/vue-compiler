//! WebAssembly bindings for the Rust Vue compiler.
//!
//! This crate exposes the `@vuec-rs/wasm` JSON-string ABI plus Rust JSON helper
//! functions used by WASI and Node smoke tests. Public entry points delegate to
//! the Rust compiler crates and convert recoverable ABI errors into structured
//! JSON values.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

use serde::Serialize;
use serde_json::{json, Value};
#[cfg(panic = "unwind")]
use std::panic::{catch_unwind, AssertUnwindSafe};
use vuec_sfc::{
    SfcCompiler, SfcPropsDestructureMode, SfcScriptCompileOptions, SfcStyleCompileOptions,
    SfcTemplateCompileOptions,
};
use vuec_source::FileId;
use vuec_vue2::Vue2CompileOptions;
use vuec_vue3_core::{TemplateSource, Vue3CompilerOptions};
use vuec_vue3_dom::{apply_dom_parser_defaults, compile as compile_dom, DomCompilerOptions};
use vuec_vue3_ssr::{compile as compile_ssr, SsrCompilerOptions};
#[cfg(not(target_os = "wasi"))]
use wasm_bindgen::prelude::*;

#[cfg(all(target_arch = "wasm32", not(target_os = "wasi")))]
#[global_allocator]
static WASM_ALLOCATOR: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[cfg(all(target_arch = "wasm32", not(target_os = "wasi")))]
#[wasm_bindgen(start)]
/// Initializes browser/Node WASM runtime panic hooks.
pub fn init_wasm_runtime() {
    console_error_panic_hook::set_once();
}

#[cfg_attr(not(target_os = "wasi"), wasm_bindgen(js_name = version))]
/// Returns the WASM package version.
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[cfg_attr(not(target_os = "wasi"), wasm_bindgen(js_name = compileVue2))]
/// Compiles a Vue 2 template and returns a JSON string result.
pub fn compile_vue2(template: &str, options_json: Option<String>) -> String {
    wasm_json_boundary(|| {
        let options = parse_options(options_json)?;
        Ok(vuec_vue2::compile(template, vue2_options(&options)))
    })
}

#[cfg_attr(not(target_os = "wasi"), wasm_bindgen(js_name = compileVue3Dom))]
/// Compiles a Vue 3 template for DOM rendering and returns a JSON string result.
pub fn compile_vue3_dom(source: &str, options_json: Option<String>) -> String {
    wasm_json_boundary(|| {
        let options = parse_options(options_json)?;
        let template = template_source(source, &options);
        let mut core = vue3_options(&options);
        apply_dom_parser_defaults(&mut core);
        Ok(compile_dom(
            template,
            DomCompilerOptions {
                core,
                ..DomCompilerOptions::default()
            },
        ))
    })
}

#[cfg_attr(not(target_os = "wasi"), wasm_bindgen(js_name = compileVue3Ssr))]
/// Compiles a Vue 3 template for SSR and returns a JSON string result.
pub fn compile_vue3_ssr(source: &str, options_json: Option<String>) -> String {
    wasm_json_boundary(|| {
        let options = parse_options(options_json)?;
        let template = template_source(source, &options);
        let mut core = vue3_options(&options);
        apply_dom_parser_defaults(&mut core);
        Ok(compile_ssr(
            template,
            SsrCompilerOptions {
                scope_id: string_option(&options, "scopeId")
                    .or_else(|| string_option(&options, "scope_id")),
                slotted: bool_option(&options, "slotted", false),
                slotted_is_explicit: options.get("slotted").is_some(),
                mode_is_explicit: options.get("mode").is_some(),
                core,
                ..SsrCompilerOptions::default()
            },
        ))
    })
}

#[cfg_attr(not(target_os = "wasi"), wasm_bindgen(js_name = parseSfc))]
/// Parses a Vue SFC descriptor and returns it as a JSON string.
pub fn parse_sfc(source: &str, options_json: Option<String>) -> String {
    wasm_json_boundary(|| {
        let options = parse_options(options_json)?;
        let filename =
            string_option(&options, "filename").unwrap_or_else(|| "anonymous.vue".into());
        let mut compiler = SfcCompiler::new();
        Ok(compiler.parse(filename, source))
    })
}

#[cfg_attr(not(target_os = "wasi"), wasm_bindgen(js_name = compileSfcTemplate))]
/// Compiles the template block from a full SFC source.
pub fn compile_sfc_template(source: &str, options_json: Option<String>) -> String {
    wasm_json_boundary(|| {
        let options = parse_options(options_json)?;
        let filename =
            string_option(&options, "filename").unwrap_or_else(|| "anonymous.vue".into());
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename, source);
        Ok(compiler.compile_template(&descriptor, sfc_template_options(&options)))
    })
}

#[cfg_attr(not(target_os = "wasi"), wasm_bindgen(js_name = compileSfcTemplateSource))]
/// Compiles standalone SFC template source.
pub fn compile_sfc_template_source(source: &str, options_json: Option<String>) -> String {
    wasm_json_boundary(|| {
        let options = parse_options(options_json)?;
        let filename =
            string_option(&options, "filename").unwrap_or_else(|| "template.vue.html".into());
        let compiler = SfcCompiler::new();
        Ok(compiler.compile_template_source(filename, source, sfc_template_options(&options)))
    })
}

#[cfg_attr(not(target_os = "wasi"), wasm_bindgen(js_name = compileSfcScript))]
/// Compiles script blocks from a full SFC source.
pub fn compile_sfc_script(source: &str, options_json: Option<String>) -> String {
    wasm_json_boundary(|| {
        let options = parse_options(options_json)?;
        let filename =
            string_option(&options, "filename").unwrap_or_else(|| "anonymous.vue".into());
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename, source);
        Ok(compiler.compile_script(&descriptor, sfc_script_options(&options)))
    })
}

#[cfg_attr(not(target_os = "wasi"), wasm_bindgen(js_name = compileSfcStyle))]
/// Compiles style blocks from a full SFC source.
pub fn compile_sfc_style(source: &str, options_json: Option<String>) -> String {
    wasm_json_boundary(|| {
        let options = parse_options(options_json)?;
        let filename =
            string_option(&options, "filename").unwrap_or_else(|| "anonymous.vue".into());
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename, source);
        Ok(compiler.compile_style(&descriptor, sfc_style_options(&options)))
    })
}

/// Compiles a Vue 2 template and returns a JSON value for Rust-side callers.
pub fn compile_vue2_json(template: &str, options: Value) -> Value {
    serde_json::to_value(vuec_vue2::compile(template, vue2_options(&options)))
        .unwrap_or_else(serialization_error_value)
}

/// Compiles a Vue 3 DOM template and returns a JSON value for Rust-side callers.
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
    .unwrap_or_else(serialization_error_value)
}

/// Compiles an SFC template and returns a JSON value for Rust-side callers.
pub fn compile_sfc_template_json(source: &str, options: Value) -> Value {
    let filename = string_option(&options, "filename").unwrap_or_else(|| "anonymous.vue".into());
    let mut compiler = SfcCompiler::new();
    let descriptor = compiler.parse(filename, source);
    serde_json::to_value(compiler.compile_template(&descriptor, sfc_template_options(&options)))
        .unwrap_or_else(serialization_error_value)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct WasmBoundaryError {
    code: &'static str,
    message: String,
}

impl WasmBoundaryError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

fn wasm_json_boundary<T>(operation: impl FnOnce() -> Result<T, WasmBoundaryError>) -> String
where
    T: Serialize,
{
    #[cfg(panic = "unwind")]
    {
        match catch_unwind(AssertUnwindSafe(operation)) {
            Ok(Ok(value)) => result_to_json(value),
            Ok(Err(error)) => value_to_json(boundary_error_value(error.code, error.message)),
            Err(payload) => value_to_json(boundary_error_value(
                "VUEC_WASM_PANIC",
                format!("compiler panicked: {}", panic_payload_message(payload)),
            )),
        }
    }

    #[cfg(not(panic = "unwind"))]
    {
        match operation() {
            Ok(value) => result_to_json(value),
            Err(error) => value_to_json(boundary_error_value(error.code, error.message)),
        }
    }
}

fn parse_options(options_json: Option<String>) -> Result<Value, WasmBoundaryError> {
    let Some(json) = options_json.filter(|json| !json.trim().is_empty()) else {
        return Ok(Value::Null);
    };
    serde_json::from_str(&json).map_err(|err| {
        WasmBoundaryError::new(
            "VUEC_WASM_INVALID_OPTIONS_JSON",
            format!("invalid options JSON: {err}"),
        )
    })
}

fn result_to_json<T: Serialize>(value: T) -> String {
    serde_json::to_string(&value)
        .unwrap_or_else(|err| value_to_json(serialization_error_value(err)))
}

fn value_to_json(value: Value) -> String {
    serde_json::to_string(&value).unwrap_or_else(|_| "{}".into())
}

fn serialization_error_value(error: impl std::fmt::Display) -> Value {
    boundary_error_value(
        "VUEC_WASM_SERIALIZE",
        format!("serialization failed: {error}"),
    )
}

fn boundary_error_value(code: &'static str, message: impl Into<String>) -> Value {
    let message = message.into();
    json!({
        "errors": [{
            "code": code,
            "message": message,
        }],
        "diagnostics": [{
            "severity": "error",
            "code": code,
            "message": message,
        }],
    })
}

#[cfg(panic = "unwind")]
fn panic_payload_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).into()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "unknown panic payload".into()
    }
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
    options.stringify_static_preserve_helpers = bool_option(
        value,
        "__vuecStringifyStaticPreserveHelpers",
        bool_option(
            value,
            "stringify_static_preserve_helpers",
            options.stringify_static_preserve_helpers,
        ),
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
    let nested_template_ssr = value
        .get("templateOptions")
        .or_else(|| value.get("template_options"))
        .and_then(|template_options| template_options.get("ssr"))
        .and_then(Value::as_bool)
        .unwrap_or(options.inline_template_ssr);
    options.inline_template_ssr = bool_option(
        value,
        "inlineTemplateSsr",
        bool_option(value, "inline_template_ssr", nested_template_ssr),
    );
    options.source_map = bool_option(
        value,
        "sourceMap",
        bool_option(value, "source_map", options.source_map),
    );
    options.props_destructure = props_destructure_option(value, options.props_destructure);
    options.global_type_files = string_array_option(value, "globalTypeFiles");
    if options.global_type_files.is_empty() {
        options.global_type_files = string_array_option(value, "global_type_files");
    }
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

fn sfc_style_options(value: &Value) -> SfcStyleCompileOptions {
    let mut options = SfcStyleCompileOptions::default();
    options.id = string_option(value, "id");
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
    options.source_map = bool_option(
        value,
        "sourceMap",
        bool_option(value, "source_map", options.source_map),
    );
    options.preprocess_lang =
        string_option(value, "preprocessLang").or_else(|| string_option(value, "preprocess_lang"));
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

fn props_destructure_option(
    value: &Value,
    fallback: SfcPropsDestructureMode,
) -> SfcPropsDestructureMode {
    match value
        .get("propsDestructure")
        .or_else(|| value.get("props_destructure"))
    {
        Some(Value::Bool(false)) => SfcPropsDestructureMode::Disabled,
        Some(Value::Bool(true)) => SfcPropsDestructureMode::Enabled,
        Some(Value::String(mode)) if mode == "error" => SfcPropsDestructureMode::Error,
        _ => fallback,
    }
}

fn string_option(value: &Value, name: &str) -> Option<String> {
    value
        .get(name)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
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
    fn sfc_script_options_accept_inline_template_ssr() {
        let options = sfc_script_options(&json!({
            "id": "xxxxxxxx",
            "inlineTemplate": true,
            "sourceMap": false,
            "propsDestructure": "error",
            "globalTypeFiles": ["global.d.ts"],
            "templateOptions": {
                "ssr": true
            }
        }));

        assert_eq!(options.id.as_deref(), Some("xxxxxxxx"));
        assert!(options.inline_template);
        assert!(options.inline_template_ssr);
        assert!(!options.source_map);
        assert_eq!(options.props_destructure, SfcPropsDestructureMode::Error);
        assert_eq!(options.global_type_files, vec!["global.d.ts"]);

        let disabled = sfc_script_options(&json!({
            "props_destructure": false,
            "global_type_files": ["ambient.d.ts"]
        }));
        assert_eq!(
            disabled.props_destructure,
            SfcPropsDestructureMode::Disabled
        );
        assert_eq!(disabled.global_type_files, vec!["ambient.d.ts"]);
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

    #[test]
    fn wasm_exports_report_invalid_options_json() {
        let value: Value =
            serde_json::from_str(&compile_vue3_dom("<div/>", Some("{not json".into())))
                .expect("json");
        assert_eq!(value["errors"][0]["code"], "VUEC_WASM_INVALID_OPTIONS_JSON");
        assert!(value["errors"][0]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("invalid options JSON"));
    }

    #[test]
    #[cfg(panic = "unwind")]
    fn wasm_boundary_converts_panics_to_json_errors() {
        let value: Value = serde_json::from_str(&wasm_json_boundary(|| -> Result<Value, _> {
            panic!("wasm boundary panic")
        }))
        .expect("json");
        assert_eq!(value["errors"][0]["code"], "VUEC_WASM_PANIC");
        assert!(value["errors"][0]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("wasm boundary panic"));
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod browser_tests {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn browser_compiles_vue3_dom_template() {
        let value: Value = serde_json::from_str(&compile_vue3_dom(
            "<div>{{ msg }}</div>",
            Some(
                json!({ "mode": "module", "prefixIdentifiers": true, "sourceMap": true })
                    .to_string(),
            ),
        ))
        .expect("json");
        assert!(value["code"]
            .as_str()
            .unwrap_or_default()
            .contains("_toDisplayString(_ctx.msg)"));
        assert_eq!(value["map"]["version"], 3);
    }

    #[wasm_bindgen_test]
    fn browser_compiles_sfc_template() {
        let value: Value = serde_json::from_str(&compile_sfc_template(
            "<template><div>{{ msg }}</div></template>",
            Some(json!({ "filename": "Browser.vue" }).to_string()),
        ))
        .expect("json");
        assert!(value["code"]
            .as_str()
            .unwrap_or_default()
            .contains("export function render"));
    }

    #[wasm_bindgen_test]
    fn browser_reports_invalid_options_json() {
        let value: Value =
            serde_json::from_str(&compile_vue3_dom("<div/>", Some("{not json".into())))
                .expect("json");
        assert_eq!(value["errors"][0]["code"], "VUEC_WASM_INVALID_OPTIONS_JSON");
        assert_eq!(value["diagnostics"][0]["severity"], "error");
    }
}
