#![deny(unsafe_code)]

use napi::{bindgen_prelude::Unknown, Env, Result};
use napi_derive::napi;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use vuec_ast::{NodeSpan, Vue3Ast, Vue3AstKind, Vue3Expression, Vue3Prop};
use vuec_sfc::{
    SfcCompiler, SfcScriptCompileOptions, SfcStyleCompileOptions, SfcTemplateCompileOptions,
    Vue27RewriteDefaultOptions,
};
use vuec_source::{FileId, Span};
use vuec_vue2::Vue2CompileOptions;
use vuec_vue3_core::{TemplateSource, Vue3CompilerOptions, Vue3Dialect};
use vuec_vue3_dom::{
    apply_dom_parser_defaults, compile as compile_dom, parse as parse_dom, DomCompilerOptions,
};
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

#[napi(js_name = "compileSsrVue2")]
pub fn compile_ssr_vue2(env: Env, template: String, options: Option<Unknown>) -> Result<String> {
    to_json_string(vuec_vue2::compile_ssr(
        &template,
        vue2_options(from_js_options(&env, options)?),
    ))
}

#[napi(js_name = "generateCodeFrameVue2")]
pub fn generate_code_frame_vue2(source: String, start: u32, end: u32) -> String {
    vuec_vue2::generate_code_frame(&source, start as usize, end as usize)
}

#[napi(js_name = "rewriteDefaultVue27")]
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
pub fn base_compile_vue3(env: Env, source: String, options: Option<Unknown>) -> Result<String> {
    compile_vue3_dom(env, source, options)
}

#[napi(js_name = "baseParseVue3")]
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
pub fn generate_vue3_core(env: Env, ast: Unknown, options: Option<Unknown>) -> Result<String> {
    let ast = from_js_options(&env, Some(ast))?;
    let options = vue3_options(Some(&from_js_options(&env, options)?));
    to_json_string(vuec_vue3_core::generate_public_ast(&ast, &options))
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
                "rewriteDefaultVue27",
                "baseCompileVue3",
                "baseParseVue3",
                "generateVue3Core",
                "compileVue3Dom",
                "parseVue3Dom",
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
}
