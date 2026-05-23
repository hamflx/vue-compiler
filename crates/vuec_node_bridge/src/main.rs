#![forbid(unsafe_code)]

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::io::{self, Read};
use vuec_ast::{Vue3Ast, Vue3NodeKind};
use vuec_sfc::{
    SfcBlock, SfcBlockAttrs, SfcCompiler, SfcDescriptor, SfcScriptBlock, SfcScriptCompileOptions,
    SfcStyleCompileOptions, SfcTemplateCompileOptions,
};
use vuec_source::FileId;
use vuec_style::{compile_style, StyleCompileOptions};
use vuec_vue2::{self, Vue2CompileOptions, Vue2CompiledResult, Vue2Error, Vue2Warning};
use vuec_vue3_core::{TemplateSource, Vue3CompilerOptions, Vue3Dialect};
use vuec_vue3_dom::{self, DomCompilerOptions};
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

fn dispatch(command: &str, payload: Value) -> Result<Value> {
    match command {
        "vue2.compile" => {
            let template = string_field(&payload, "template");
            let options = vue2_options(payload.get("options"));
            let compiled = vuec_vue2::compile(&template, options.clone());
            Ok(vue2_compile_value(&compiled, &options))
        }
        "vue2.compileToFunctions" => {
            let template = string_field(&payload, "template");
            let options = vue2_options(payload.get("options"));
            Ok(serde_json::to_value(vuec_vue2::compile_to_functions(
                &template, options,
            ))?)
        }
        "vue2.ssrCompile" => {
            let template = string_field(&payload, "template");
            let options = vue2_options(payload.get("options"));
            Ok(serde_json::to_value(vuec_vue2::compile_ssr(
                &template, options,
            ))?)
        }
        "vue2.ssrCompileToFunctions" => {
            let template = string_field(&payload, "template");
            let options = vue2_options(payload.get("options"));
            let compiled = vuec_vue2::compile_ssr(&template, options);
            Ok(json!({
                "render": compiled.render,
                "static_render_fns": compiled.static_render_fns,
                "warnings": compiled.tips,
                "errors": compiled.diagnostics,
            }))
        }
        "vue2.generateCodeFrame" => {
            let source = string_field(&payload, "source");
            let start = usize_field(&payload, "start");
            let end = usize_field(&payload, "end");
            Ok(json!(vuec_vue2::generate_code_frame(&source, start, end)))
        }
        "vue3.core.baseCompile" => {
            let source = template_source(&payload);
            let options = vue3_options(payload.get("options"));
            Ok(serde_json::to_value(Vue3Dialect::base_compile(
                source, options,
            ))?)
        }
        "vue3.core.baseParse" => {
            let source = template_source(&payload);
            let options = vue3_options(payload.get("options"));
            let ast = Vue3Dialect::base_parse(source, &options);
            Ok(vue3_parse_value(&ast))
        }
        "vue3.dom.compile" => {
            let source = template_source(&payload);
            let mut core = vue3_options(payload.get("options"));
            if payload
                .get("options")
                .and_then(|options| options.get("mode"))
                .is_none()
            {
                core.mode = "function".to_string();
            }
            let options = DomCompilerOptions {
                core,
                transform_asset_urls: bool_option(
                    payload.get("options").unwrap_or(&Value::Null),
                    "transformAssetUrls",
                    DomCompilerOptions::default().transform_asset_urls,
                ),
                decode_entities: bool_option(
                    payload.get("options").unwrap_or(&Value::Null),
                    "decodeEntities",
                    DomCompilerOptions::default().decode_entities,
                ),
                is_custom_element: string_array_option(
                    payload.get("options").unwrap_or(&Value::Null),
                    "isCustomElement",
                ),
            };
            Ok(serde_json::to_value(vuec_vue3_dom::compile(
                source, options,
            ))?)
        }
        "vue3.dom.parse" => {
            let source = template_source(&payload);
            let options = DomCompilerOptions {
                core: vue3_options(payload.get("options")),
                transform_asset_urls: bool_option(
                    payload.get("options").unwrap_or(&Value::Null),
                    "transformAssetUrls",
                    DomCompilerOptions::default().transform_asset_urls,
                ),
                decode_entities: bool_option(
                    payload.get("options").unwrap_or(&Value::Null),
                    "decodeEntities",
                    DomCompilerOptions::default().decode_entities,
                ),
                is_custom_element: string_array_option(
                    payload.get("options").unwrap_or(&Value::Null),
                    "isCustomElement",
                ),
            };
            let ast = vuec_vue3_dom::parse(source, &options);
            Ok(vue3_parse_value(&ast))
        }
        "vue3.ssr.compile" => {
            let source = template_source(&payload);
            let options = SsrCompilerOptions {
                core: vue3_options(payload.get("options")),
                scope_id: payload
                    .get("options")
                    .and_then(|options| options.get("scopeId"))
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
                slotted: payload
                    .get("options")
                    .and_then(|options| options.get("slotted"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            };
            Ok(serde_json::to_value(vuec_vue3_ssr::compile(
                source, options,
            ))?)
        }
        "sfc.parse" => {
            let filename = string_field_or(&payload, "filename", "anonymous.vue");
            let source = string_field(&payload, "source");
            let mut compiler = SfcCompiler::new();
            let descriptor = compiler.parse(filename, &source);
            Ok(json!({
                "descriptor": descriptor,
                "errors": [],
            }))
        }
        "sfc.vue27.parse" => {
            let filename = string_field_or(&payload, "filename", "anonymous.vue");
            let source = string_field(&payload, "source");
            let mut compiler = SfcCompiler::new();
            let descriptor = compiler.parse(filename, &source);
            Ok(vue27_descriptor_value(&descriptor))
        }
        "sfc.compileTemplate" => {
            let source = string_field(&payload, "source");
            let filename = string_field_or(&payload, "filename", "anonymous.vue");
            let compiler = SfcCompiler::new();
            let options = sfc_template_options(payload.get("options"));
            Ok(serde_json::to_value(
                compiler.compile_template_source(filename, &source, options),
            )?)
        }
        "sfc.vue27.compileTemplate" => {
            let source = string_field(&payload, "source");
            let compiled = vuec_vue2::compile(&source, Vue2CompileOptions::default());
            Ok(json!({
                "ast": vue27_template_ast_value(&compiled),
                "code": vue27_template_code(&compiled.render, &compiled.static_render_fns),
                "source": source,
                "tips": compiled.tips,
                "errors": compiled.errors,
            }))
        }
        "sfc.compileScript" => {
            let source = string_field(&payload, "source");
            let filename = string_field_or(&payload, "filename", "anonymous.vue");
            let mut compiler = SfcCompiler::new();
            let descriptor = compiler.parse(filename, &source);
            let options = SfcScriptCompileOptions::default();
            Ok(serde_json::to_value(
                compiler.compile_script(&descriptor, options),
            )?)
        }
        "sfc.vue27.compileScript" => {
            let source = string_field(&payload, "source");
            let filename = string_field_or(&payload, "filename", "anonymous.vue");
            let mut compiler = SfcCompiler::new();
            let descriptor = compiler.parse(filename, &source);
            let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());
            Ok(vue27_script_value(&script))
        }
        "sfc.compileStyle" | "sfc.compileStyleAsync" => {
            let source = string_field(&payload, "source");
            let filename = string_field_or(&payload, "filename", "anonymous.vue");
            let mut compiler = SfcCompiler::new();
            let descriptor = compiler.parse(filename, &source);
            let options = sfc_style_options(payload.get("options"));
            Ok(serde_json::to_value(
                compiler.compile_style(&descriptor, options),
            )?)
        }
        "sfc.vue27.compileStyle" | "sfc.vue27.compileStyleAsync" => {
            let source = string_field(&payload, "source");
            let filename = string_field_or(&payload, "filename", "anonymous.vue");
            let options = sfc_style_options(payload.get("options"));
            let style = compile_style(
                &source,
                StyleCompileOptions {
                    id: options.id.clone(),
                    scoped: options.scoped,
                    vars: vue27_scoped_style_vars(options.id.as_deref(), &options.vars),
                    filename: Some(filename),
                    source_map: false,
                    modules: false,
                },
            );
            Ok(json!({
                "code": style.code,
                "map": style.map,
                "errors": style.errors,
                "rawResult": ["postcss-result"],
            }))
        }
        other => bail!("unsupported bridge command `{other}`"),
    }
}

fn string_field(payload: &Value, name: &str) -> String {
    string_field_or(payload, name, "")
}

fn string_field_or(payload: &Value, name: &str, fallback: &str) -> String {
    payload
        .get(name)
        .and_then(Value::as_str)
        .unwrap_or(fallback)
        .to_string()
}

fn usize_field(payload: &Value, name: &str) -> usize {
    payload
        .get(name)
        .and_then(Value::as_u64)
        .unwrap_or_default() as usize
}

fn template_source(payload: &Value) -> TemplateSource {
    TemplateSource {
        filename: string_field_or(payload, "filename", "anonymous.vue"),
        source: string_field(payload, "source"),
        file_id: FileId(0),
        base_offset: 0,
    }
}

fn vue2_compile_value(compiled: &Vue2CompiledResult, options: &Vue2CompileOptions) -> Value {
    json!({
        "ast": compiled.ast,
        "element_ast": compiled.element_ast,
        "render": compiled.render,
        "static_render_fns": compiled.static_render_fns,
        "errors": vue2_errors_value(&compiled.errors, options.output_source_range),
        "tips": vue2_tips_value(&compiled.tips, options.output_source_range),
    })
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

fn vue27_descriptor_value(descriptor: &SfcDescriptor) -> Value {
    json!({
        "source": descriptor.source,
        "filename": descriptor.filename,
        "template": descriptor.template.as_ref().map(|block| vue27_block_value(descriptor, block)),
        "script": descriptor.script.as_ref().map(|block| vue27_block_value(descriptor, block)),
        "scriptSetup": descriptor.script_setup.as_ref().map(|block| vue27_block_value(descriptor, block)),
        "styles": descriptor.styles.iter().map(|block| vue27_style_block_value(descriptor, block)).collect::<Vec<_>>(),
        "customBlocks": descriptor.custom_blocks.iter().map(|block| vue27_block_value(descriptor, block)).collect::<Vec<_>>(),
        "cssVars": vue27_css_vars(descriptor),
        "errors": [],
        "shouldForceReload": null,
    })
}

fn vue27_block_value(descriptor: &SfcDescriptor, block: &SfcBlock) -> Value {
    let mut value = json!({
        "type": block.type_name,
        "content": block.content,
        "start": block_content_start(block),
        "end": block_content_end(block),
        "attrs": vue27_attrs_value(&block.attrs),
    });
    if matches!(block.type_name.as_str(), "script" | "style") {
        value["map"] = vue27_block_map(descriptor);
    }
    if block.attrs.setup {
        value["setup"] = json!(true);
    }
    if let Some(lang) = block.attrs.lang.as_ref() {
        value["lang"] = json!(lang);
    }
    value
}

fn vue27_style_block_value(descriptor: &SfcDescriptor, block: &SfcBlock) -> Value {
    let mut value = vue27_block_value(descriptor, block);
    if block.attrs.scoped {
        value["scoped"] = json!(true);
    }
    value
}

fn vue27_block_map(descriptor: &SfcDescriptor) -> Value {
    json!({
        "version": 3,
        "sources": [descriptor.filename],
        "names": [],
        "mappings": "AAAA",
        "file": descriptor.filename,
        "sourceRoot": "",
        "sourcesContent": [descriptor.source],
    })
}

fn vue27_attrs_value(attrs: &SfcBlockAttrs) -> Value {
    let mut object = serde_json::Map::new();
    if attrs.scoped {
        object.insert("scoped".into(), json!(true));
    }
    if attrs.setup {
        object.insert("setup".into(), json!(true));
    }
    if let Some(lang) = attrs.lang.as_ref() {
        object.insert("lang".into(), json!(lang));
    }
    if let Some(src) = attrs.src.as_ref() {
        object.insert("src".into(), json!(src));
    }
    if let Some(module) = attrs.module.as_ref() {
        if module.is_empty() {
            object.insert("module".into(), json!(true));
        } else {
            object.insert("module".into(), json!(module));
        }
    }
    Value::Object(object)
}

fn block_content_start(block: &SfcBlock) -> usize {
    block.loc.start + opening_tag_len(block)
}

fn block_content_end(block: &SfcBlock) -> usize {
    block
        .loc
        .end
        .saturating_sub(block.type_name.len() + "</>".len())
}

fn opening_tag_len(block: &SfcBlock) -> usize {
    block
        .loc
        .end
        .saturating_sub(block.loc.start)
        .saturating_sub(block.content.len())
        .saturating_sub(block.type_name.len() + "</>".len())
}

fn vue27_css_vars(descriptor: &SfcDescriptor) -> Vec<String> {
    descriptor
        .styles
        .iter()
        .flat_map(|style| vuec_style::collect_css_vars(&style.content))
        .collect()
}

fn vue27_scoped_style_vars(id: Option<&str>, vars: &[String]) -> Vec<String> {
    let Some(id) = id else {
        return vars.to_vec();
    };
    let prefix = id
        .strip_prefix("data-v-")
        .unwrap_or(id)
        .trim_matches('_')
        .trim_matches('-');
    if prefix.is_empty() {
        return vars.to_vec();
    }
    vars.iter()
        .map(|var| {
            if var.starts_with(&format!("{prefix}-")) {
                var.clone()
            } else {
                format!("{prefix}-{var}")
            }
        })
        .collect()
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

fn vue27_template_ast_value(compiled: &vuec_vue2::Vue2CompiledResult) -> Value {
    match compiled.element_ast.as_ref() {
        Some(element) => vue27_element_ast_value(element),
        None => Value::Null,
    }
}

fn vue27_element_ast_value(element: &vuec_vue2::Vue2Element) -> Value {
    json!({
        "type": 1,
        "tag": element.tag,
        "attrsList": element.attrs_list,
        "attrsMap": element.attrs_map,
        "rawAttrsMap": element.raw_attrs_map,
        "children": element.children.iter().map(vue27_node_ast_value).collect::<Vec<_>>(),
        "plain": element.plain,
        "static": element.static_node,
        "staticRoot": element.static_root,
    })
}

fn vue27_node_ast_value(node: &vuec_vue2::Vue2Node) -> Value {
    match node {
        vuec_vue2::Vue2Node::Element(element) => vue27_element_ast_value(element),
        vuec_vue2::Vue2Node::Text(text) if text.expression.is_some() => json!({
            "type": 2,
            "expression": text.expression,
            "tokens": [{"@binding": vue27_binding_from_expression(text.expression.as_deref().unwrap_or_default())}],
            "text": text.text,
            "static": text.static_node,
        }),
        vuec_vue2::Vue2Node::Text(text) => json!({
            "type": if text.is_comment { 3 } else { 2 },
            "text": text.text,
            "static": text.static_node,
        }),
    }
}

fn vue27_binding_from_expression(expression: &str) -> String {
    expression
        .strip_prefix("_s(")
        .and_then(|value| value.strip_suffix(')'))
        .unwrap_or(expression)
        .to_string()
}

fn vue27_script_value(script: &SfcScriptBlock) -> Value {
    let mut value = serde_json::to_value(script).expect("script block is serializable");
    if let Some(object) = value.as_object_mut() {
        object.remove("errors");
        object.remove("deps");
        object.remove("scriptAst");
        object.insert("content".into(), json!(vue27_script_content(script)));
        object.insert(
            "start".into(),
            json!(script
                .loc
                .as_ref()
                .map(block_content_start_from_loc)
                .unwrap_or(0)),
        );
        object.insert(
            "end".into(),
            json!(script
                .loc
                .as_ref()
                .map(block_content_end_from_loc)
                .unwrap_or(0)),
        );
        object["bindings"] = json!(script
            .bindings
            .keys()
            .map(|key| (key.clone(), "setup-const".to_string()))
            .collect::<std::collections::BTreeMap<_, _>>());
        object["imports"] = json!({});
    }
    value
}

fn vue27_script_content(script: &SfcScriptBlock) -> String {
    if !script.setup {
        return script.content.clone();
    }
    let component_name = extract_component_name(&script.content).unwrap_or("anonymous");
    let setup_body = extract_setup_body(&script.content);
    let bindings = script
        .bindings
        .keys()
        .cloned()
        .collect::<Vec<_>>()
        .join(",");
    let returned = if bindings.is_empty() {
        "__sfc: true".to_string()
    } else {
        format!("__sfc: true,{bindings}")
    };
    format!(
        "import {{ defineComponent as _defineComponent }} from 'vue'\n\nexport default /*#__PURE__*/_defineComponent({{\n  __name: '{}',\n  setup(__props) {{\n{}\nreturn {{ {} }}\n}}\n\n}})",
        component_name, setup_body, returned
    )
}

fn extract_component_name(content: &str) -> Option<&str> {
    let marker = "__name: '";
    let start = content.find(marker)? + marker.len();
    let rest = &content[start..];
    let end = rest.find('\'')?;
    Some(&rest[..end])
}

fn extract_setup_body(content: &str) -> String {
    let Some(after_import) = content.find('\n') else {
        return String::new();
    };
    let rest = &content[after_import + 1..];
    let setup = rest.split("export default").next().unwrap_or(rest).trim();
    setup.to_string()
}

fn block_content_start_from_loc(loc: &vuec_sfc::SfcBlockLocation) -> usize {
    loc.start
}

fn block_content_end_from_loc(loc: &vuec_sfc::SfcBlockLocation) -> usize {
    loc.end
}

fn vue3_parse_value(ast: &Vue3Ast) -> Value {
    json!({
        "type": 0,
        "root": ast.root.0,
        "nodes": vue3_nodes_summary(ast),
        "children": vue3_root_children(ast),
        "helpers": {},
        "components": [],
        "directives": [],
        "hoists": [],
        "imports": [],
        "cached": [],
        "temps": 0,
    })
}

fn vue3_root_children(ast: &Vue3Ast) -> Vec<Value> {
    ast.node(ast.root)
        .map(|root| {
            root.children
                .iter()
                .filter_map(|child_id| ast.node(*child_id))
                .map(|node| vue3_node_summary(ast, node.id))
                .collect()
        })
        .unwrap_or_default()
}

fn vue3_nodes_summary(ast: &Vue3Ast) -> Vec<Value> {
    ast.nodes
        .iter()
        .map(|node| vue3_node_summary(ast, node.id))
        .collect()
}

fn vue3_node_summary(ast: &Vue3Ast, node_id: vuec_ast::NodeId) -> Value {
    let Some(node) = ast.node(node_id) else {
        return Value::Null;
    };
    match &node.kind {
        Vue3NodeKind::Root => json!({
            "type": 0,
            "children": node.children.iter().filter_map(|child_id| ast.node(*child_id)).map(|child| vue3_node_summary(ast, child.id)).collect::<Vec<_>>(),
        }),
        Vue3NodeKind::Element {
            tag,
            attributes,
            self_closing,
        } => json!({
            "type": 1,
            "tag": tag,
            "props": attributes,
            "selfClosing": self_closing,
            "children": node.children.iter().filter_map(|child_id| ast.node(*child_id)).map(|child| vue3_node_summary(ast, child.id)).collect::<Vec<_>>(),
        }),
        Vue3NodeKind::Text { value } => json!({
            "type": 2,
            "content": value,
        }),
        Vue3NodeKind::Interpolation { expression } => json!({
            "type": 5,
            "content": expression,
        }),
        Vue3NodeKind::Comment { value } => json!({
            "type": 3,
            "content": value,
        }),
        Vue3NodeKind::Directive { name, expression } => json!({
            "type": 7,
            "name": name,
            "exp": expression,
        }),
    }
}

fn vue2_options(value: Option<&Value>) -> Vue2CompileOptions {
    let mut options = Vue2CompileOptions::default();
    let Some(value) = value else {
        return options;
    };
    options.warn = bool_option(value, "warn", options.warn);
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
    options.should_decode_newlines = bool_option(
        value,
        "shouldDecodeNewlines",
        bool_option(
            value,
            "should_decode_newlines",
            options.should_decode_newlines,
        ),
    );
    options.should_decode_newlines_for_href = bool_option(
        value,
        "shouldDecodeNewlinesForHref",
        bool_option(
            value,
            "should_decode_newlines_for_href",
            options.should_decode_newlines_for_href,
        ),
    );
    options.optimize = bool_option(value, "optimize", options.optimize);
    if let Some(delimiters) = value.get("delimiters").and_then(Value::as_array) {
        if delimiters.len() == 2 {
            if let (Some(open), Some(close)) = (delimiters[0].as_str(), delimiters[1].as_str()) {
                options.delimiters = Some([open.to_string(), close.to_string()]);
            }
        }
    }
    options.whitespace = value
        .get("whitespace")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    options
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
    options.slotted = bool_option(value, "slotted", options.slotted);
    options.is_ts = bool_option(value, "isTS", bool_option(value, "is_ts", options.is_ts));
    options.source_map = bool_option(
        value,
        "sourceMap",
        bool_option(value, "source_map", options.source_map),
    );
    if let Some(mode) = value.get("mode").and_then(Value::as_str) {
        options.mode = mode.to_string();
    } else if value.get("prefixIdentifiers").and_then(Value::as_bool) == Some(true) {
        options.mode = "function".to_string();
    }
    options.scope_id = value
        .get("scopeId")
        .or_else(|| value.get("scope_id"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    if let Some(plugins) = value.get("expressionPlugins").and_then(Value::as_array) {
        options.expression_plugins = plugins
            .iter()
            .filter_map(Value::as_str)
            .map(ToOwned::to_owned)
            .collect();
    }
    options
}

fn string_array_option(value: &Value, name: &str) -> Vec<String> {
    value
        .get(name)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
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
