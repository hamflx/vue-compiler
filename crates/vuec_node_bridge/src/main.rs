#![forbid(unsafe_code)]

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::io::{self, Read};
use vuec_ast::{NodeSpan, Vue3Ast, Vue3AstKind, Vue3Expression, Vue3Prop};
use vuec_html::{HtmlTokenKind, HtmlTokenizer};
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
            let ast = Vue3Dialect::base_parse(source.clone(), &options);
            Ok(vue3_parse_value(&ast, &source.source, source.base_offset))
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
            let ast = vuec_vue3_dom::parse(source.clone(), &options);
            Ok(vue3_parse_value(&ast, &source.source, source.base_offset))
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

fn vue3_parse_value(ast: &Vue3Ast, source: &str, base_offset: usize) -> Value {
    json!({
        "type": 0,
        "source": source,
        "children": vue3_root_children(ast, source, base_offset),
        "helpers": [],
        "components": [],
        "directives": [],
        "hoists": [],
        "imports": [],
        "cached": [],
        "temps": 0,
        "codegenNode": Value::Null,
        "loc": ast.root_node().map(|node| vue3_loc_value(source, base_offset, &node.span)).unwrap_or_else(vue3_loc_stub_value),
        "__vuecDiagnostics": vue3_parse_diagnostics(ast, source, base_offset),
    })
}

fn vue3_root_children(ast: &Vue3Ast, source: &str, base_offset: usize) -> Vec<Value> {
    ast.node(ast.root)
        .map(|root| {
            root.children
                .iter()
                .filter_map(|child_id| ast.node(*child_id))
                .map(|node| vue3_node_summary(ast, source, base_offset, node.id))
                .collect()
        })
        .unwrap_or_default()
}

fn vue3_node_summary(
    ast: &Vue3Ast,
    source: &str,
    base_offset: usize,
    node_id: vuec_ast::NodeId,
) -> Value {
    let Some(node) = ast.node(node_id) else {
        return Value::Null;
    };
    match &node.kind {
        Vue3AstKind::Root(_) => json!({
            "type": 0,
            "source": source,
            "children": node.children.iter().filter_map(|child_id| ast.node(*child_id)).map(|child| vue3_node_summary(ast, source, base_offset, child.id)).collect::<Vec<_>>(),
            "helpers": [],
            "components": [],
            "directives": [],
            "hoists": [],
            "imports": [],
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
            "children": node.children.iter().filter_map(|child_id| ast.node(*child_id)).map(|child| vue3_node_summary(ast, source, base_offset, child.id)).collect::<Vec<_>>(),
            "loc": vue3_loc_value(source, base_offset, &node.span),
            "codegenNode": Value::Null,
            "isSelfClosing": if element.self_closing { json!(true) } else { json!(null) },
        }),
        Vue3AstKind::Text(text) => json!({
            "type": 2,
            "content": text.value,
            "loc": vue3_loc_value(source, base_offset, &node.span),
        }),
        Vue3AstKind::Interpolation(interpolation) => json!({
            "type": 5,
            "content": vue3_expression_value(source, base_offset, &interpolation.expression, &node.span, false),
            "loc": vue3_loc_value(source, base_offset, &node.span),
        }),
        Vue3AstKind::Comment(comment) => json!({
            "type": 3,
            "content": comment.value,
            "loc": vue3_loc_value(source, base_offset, &node.span),
        }),
        _ => json!({
            "type": 7,
            "name": "unsupported",
            "exp": null,
            "loc": vue3_loc_value(source, base_offset, &node.span),
        }),
    }
}

fn vue3_parse_diagnostics(ast: &Vue3Ast, source: &str, base_offset: usize) -> Vec<Value> {
    let mut diagnostics = Vec::new();
    collect_invalid_lt_diagnostics(source, &mut diagnostics);
    collect_missing_interpolation_end_diagnostics(source, &mut diagnostics);
    collect_invalid_end_tag_diagnostics(ast, source, base_offset, &mut diagnostics);
    collect_missing_directive_name_diagnostics(ast, source, base_offset, &mut diagnostics);
    diagnostics
}

fn collect_invalid_lt_diagnostics(source: &str, diagnostics: &mut Vec<Value>) {
    for token in HtmlTokenizer::new(source).tokenize() {
        let HtmlTokenKind::Text(text) = token.kind else {
            continue;
        };
        let interpolation_ranges = default_interpolation_ranges(&text);
        let mut cursor = 0usize;
        while let Some(offset) = text[cursor..].find('<') {
            let local_index = cursor + offset;
            cursor = local_index + 1;
            if interpolation_ranges
                .iter()
                .any(|(start, end)| local_index >= *start && local_index < *end)
            {
                continue;
            }
            let global_index = token.start + local_index;
            if source
                .as_bytes()
                .get(global_index + 1)
                .is_some_and(|next| !matches!(*next, b'/' | b'!' | b'A'..=b'Z' | b'a'..=b'z'))
            {
                diagnostics.push(vue3_error_value(
                    12,
                    vue3_source_loc_value(source, global_index, global_index),
                ));
            }
        }
    }
}

fn default_interpolation_ranges(text: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut cursor = 0usize;
    while let Some(open_offset) = text[cursor..].find("{{") {
        let open = cursor + open_offset;
        let inner_start = open + 2;
        let Some(close_offset) = text[inner_start..].find("}}") else {
            break;
        };
        let close = inner_start + close_offset + 2;
        ranges.push((open, close));
        cursor = close;
    }
    ranges
}

fn collect_missing_interpolation_end_diagnostics(source: &str, diagnostics: &mut Vec<Value>) {
    let mut cursor = 0usize;
    while let Some(open_offset) = source[cursor..].find("{{") {
        let open = cursor + open_offset;
        let inner_start = open + 2;
        if let Some(close_offset) = source[inner_start..].find("}}") {
            cursor = inner_start + close_offset + 2;
        } else {
            diagnostics.push(vue3_error_value(
                25,
                vue3_source_loc_value(source, source.len(), source.len()),
            ));
            break;
        }
    }
}

fn collect_invalid_end_tag_diagnostics(
    ast: &Vue3Ast,
    source: &str,
    _base_offset: usize,
    diagnostics: &mut Vec<Value>,
) {
    let mut stack = Vec::<String>::new();
    for token in HtmlTokenizer::new(source).tokenize() {
        match token.kind {
            HtmlTokenKind::StartTag {
                name, self_closing, ..
            } => {
                if !self_closing
                    && ast.nodes.iter().any(|node| {
                        matches!(&node.kind, Vue3AstKind::Element(element) if element.tag.eq_ignore_ascii_case(&name))
                    })
                {
                    stack.push(name);
                }
            }
            HtmlTokenKind::EndTag { name } => {
                if stack
                    .last()
                    .is_some_and(|open| open.eq_ignore_ascii_case(&name))
                {
                    stack.pop();
                } else if !stack.iter().any(|open| open.eq_ignore_ascii_case(&name)) {
                    diagnostics.push(vue3_error_value(
                        23,
                        vue3_source_loc_value(source, token.start, token.start),
                    ));
                } else {
                    while let Some(open) = stack.pop() {
                        if open.eq_ignore_ascii_case(&name) {
                            break;
                        }
                    }
                }
            }
            HtmlTokenKind::Text(_)
            | HtmlTokenKind::Comment(_)
            | HtmlTokenKind::Cdata(_)
            | HtmlTokenKind::Doctype(_)
            | HtmlTokenKind::Eof => {}
        }
    }
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

fn vue3_prop_value(source: &str, base_offset: usize, prop: &Vue3Prop) -> Value {
    match prop {
        Vue3Prop::Attribute(attr) => vue3_attribute_value(source, base_offset, attr),
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
                    .map(|span| vue3_source_span_value(source, base_offset, *span))
                    .unwrap_or_else(vue3_loc_stub_value);
                vue3_simple_expression_value(
                    modifier,
                    true,
                    loc,
                )
            }).collect::<Vec<_>>(),
            "loc": dir.span.map(|span| vue3_source_span_value(source, base_offset, span)).unwrap_or_else(vue3_loc_stub_value),
        }),
    }
}

fn vue3_attribute_value(source: &str, base_offset: usize, attr: &vuec_ast::Vue3Attribute) -> Value {
    json!({
        "type": 6,
        "name": attr.name,
        "nameLoc": attr.name_span.map(|span| vue3_source_span_value(source, base_offset, span)).unwrap_or_else(vue3_loc_stub_value),
        "value": attr.value.as_ref().map(|value| json!({
            "type": 2,
            "content": value,
            "loc": attr.value_span.map(|span| vue3_source_span_value(source, base_offset, span)).unwrap_or_else(vue3_loc_stub_value),
        })),
        "loc": attr.span.map(|span| vue3_source_span_value(source, base_offset, span)).unwrap_or_else(vue3_loc_stub_value),
    })
}

fn span_to_node_span(span: Option<vuec_source::Span>) -> NodeSpan {
    span.map(NodeSpan::from)
        .unwrap_or_else(|| NodeSpan::missing(vuec_ast::MissingSpanReason::Synthetic))
}

fn vue3_expression_value(
    source_text: &str,
    base_offset: usize,
    expression: &Vue3Expression,
    fallback_span: &NodeSpan,
    is_static: bool,
) -> Value {
    let source = expression.source_string();
    let loc = vue3_expression_loc(source_text, base_offset, fallback_span, &source);
    vue3_simple_expression_value(&source, is_static, loc)
}

fn vue3_simple_expression_value(source: &str, is_static: bool, loc: Value) -> Value {
    json!({
        "type": 4,
        "loc": loc,
        "content": source.trim(),
        "isStatic": is_static,
        "constType": if is_static { 3 } else { 0 },
    })
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
    let trimmed = expression.trim();
    if trimmed.is_empty() {
        return vue3_loc_value(source, base_offset, fallback_span);
    }
    let local_span_start = span.start.0.saturating_sub(base_offset);
    let local_span_end = span.end.0.saturating_sub(base_offset).min(source.len());
    let node_source = source
        .get(local_span_start..local_span_end)
        .unwrap_or_default();
    if let Some(local_start) = node_source.find(trimmed) {
        let start = local_span_start + local_start;
        return vue3_source_loc_value(source, start, start + trimmed.len());
    }
    vue3_loc_value(source, base_offset, fallback_span)
}

fn vue3_loc_value(source: &str, base_offset: usize, span: &NodeSpan) -> Value {
    let Some(span) = span.source() else {
        return vue3_loc_stub_value();
    };
    vue3_source_span_value(source, base_offset, span)
}

fn vue3_source_span_value(source: &str, base_offset: usize, span: vuec_source::Span) -> Value {
    let start = span.start.0.saturating_sub(base_offset);
    let end = span.end.0.saturating_sub(base_offset);
    vue3_source_loc_value(source, start, end)
}

fn vue3_source_loc_value(source: &str, start: usize, end: usize) -> Value {
    let local_start = start.min(source.len());
    let local_end = end.min(source.len());
    let start_pos = vue3_position(&source, local_start);
    let end_pos = vue3_position(&source, local_end);
    json!({
        "start": start_pos,
        "end": end_pos,
        "source": source.get(local_start..local_end).unwrap_or_default(),
    })
}

fn vue3_position(source: &str, offset: usize) -> Value {
    let mut line = 1usize;
    let mut column = 1usize;
    let mut index = 0usize;
    for ch in source.chars() {
        if index >= offset {
            break;
        }
        index += ch.len_utf8();
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    json!({
        "offset": offset,
        "line": line,
        "column": column,
    })
}

fn vue3_loc_stub_value() -> Value {
    json!({
        "start": { "offset": 0, "line": 1, "column": 1 },
        "end": { "offset": 0, "line": 1, "column": 1 },
        "source": "",
    })
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
    options.comments = bool_option(value, "comments", options.comments);
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
    if let Some(delimiters) = value.get("delimiters").and_then(Value::as_array) {
        if delimiters.len() == 2 {
            if let (Some(open), Some(close)) = (delimiters[0].as_str(), delimiters[1].as_str()) {
                options.delimiters = Some([open.to_string(), close.to_string()]);
            }
        }
    }
    options.void_tags = string_array_option(value, "__vuecVoidTags");
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
