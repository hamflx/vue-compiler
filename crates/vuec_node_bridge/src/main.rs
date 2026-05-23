#![forbid(unsafe_code)]

use anyhow::{bail, Context, Result};
use oxc_ast::ast::{Expression, Statement};
use oxc_span::SourceType;
use serde_json::{json, Value};
use std::io::{self, Read};
use vuec_ast::{NodeSpan, Vue3Ast, Vue3AstKind, Vue3Expression, Vue3Prop};
use vuec_html::{HtmlTokenKind, HtmlTokenizer};
use vuec_js::JsAstStore;
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
            let include_sfc_inner_loc = vue3_parse_mode_is_sfc(payload.get("options"));
            Ok(vue3_parse_value(
                &ast,
                &source.source,
                source.base_offset,
                include_sfc_inner_loc,
                &options,
            ))
        }
        "vue3.core.rootCodegen" => Ok(vuec_vue3_core::root_codegen_projection(
            payload.get("root").unwrap_or(&payload),
        )),
        "vue3.core.transformModel" => Ok(vuec_vue3_core::transform_model_projection(&payload)),
        "vue3.core.transformIf" => Ok(vuec_vue3_core::transform_if_projection(&payload)),
        "vue3.core.resolveComponentType" => {
            Ok(vuec_vue3_core::resolve_component_type_projection(&payload))
        }
        "vue3.core.transformElementProps" => {
            Ok(vuec_vue3_core::transform_element_props_projection(&payload))
        }
        "vue3.core.buildDirectiveArgs" => {
            Ok(vuec_vue3_core::build_directive_args_projection(&payload))
        }
        "vue3.dom.transformStyle" => Ok(vuec_vue3_dom::transform_style_projection(&payload)),
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
            let include_sfc_inner_loc = vue3_parse_mode_is_sfc(payload.get("options"));
            Ok(vue3_parse_value(
                &ast,
                &source.source,
                source.base_offset,
                include_sfc_inner_loc,
                &options.core,
            ))
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
        filename: payload
            .get("filename")
            .or_else(|| {
                payload
                    .get("options")
                    .and_then(|options| options.get("filename"))
            })
            .and_then(Value::as_str)
            .unwrap_or("anonymous.vue")
            .to_string(),
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

fn vue3_parse_value(
    ast: &Vue3Ast,
    source: &str,
    base_offset: usize,
    include_sfc_inner_loc: bool,
    options: &Vue3CompilerOptions,
) -> Value {
    json!({
        "type": 0,
        "source": source,
        "children": vue3_root_children(ast, source, base_offset, include_sfc_inner_loc, options),
        "helpers": [],
        "components": [],
        "directives": [],
        "hoists": [],
        "imports": [],
        "cached": [],
        "temps": 0,
        "codegenNode": Value::Null,
        "loc": ast.root_node().map(|node| vue3_loc_value(source, base_offset, &node.span)).unwrap_or_else(vue3_loc_stub_value),
        "__vuecDiagnostics": vue3_parse_diagnostics(ast, source, base_offset, options),
    })
}

fn vue3_root_children(
    ast: &Vue3Ast,
    source: &str,
    base_offset: usize,
    include_sfc_inner_loc: bool,
    options: &Vue3CompilerOptions,
) -> Vec<Value> {
    ast.node(ast.root)
        .map(|root| {
            root.children
                .iter()
                .filter_map(|child_id| ast.node(*child_id))
                .map(|node| {
                    vue3_node_summary(
                        ast,
                        source,
                        base_offset,
                        node.id,
                        include_sfc_inner_loc,
                        options,
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

fn vue3_node_summary(
    ast: &Vue3Ast,
    source: &str,
    base_offset: usize,
    node_id: vuec_ast::NodeId,
    include_sfc_inner_loc: bool,
    options: &Vue3CompilerOptions,
) -> Value {
    let Some(node) = ast.node(node_id) else {
        return Value::Null;
    };
    match &node.kind {
        Vue3AstKind::Root(_) => json!({
            "type": 0,
            "source": source,
            "children": node.children.iter().filter_map(|child_id| ast.node(*child_id)).map(|child| vue3_node_summary(ast, source, base_offset, child.id, include_sfc_inner_loc, options)).collect::<Vec<_>>(),
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
        Vue3AstKind::Element(element) => {
            let mut value = json!({
                "type": 1,
                "tag": element.tag,
                "ns": vue3_namespace_value(element.ns),
                "tagType": vue3_element_type_value(element.tag_type),
                "props": element.props.iter().map(|prop| vue3_prop_value(source, base_offset, prop, options)).collect::<Vec<_>>(),
                "children": node.children.iter().filter_map(|child_id| ast.node(*child_id)).map(|child| vue3_node_summary(ast, source, base_offset, child.id, include_sfc_inner_loc, options)).collect::<Vec<_>>(),
                "loc": vue3_loc_value(source, base_offset, &node.span),
                "codegenNode": Value::Null,
                "isSelfClosing": if element.self_closing { json!(true) } else { json!(null) },
            });
            if include_sfc_inner_loc {
                value["innerLoc"] = vue3_inner_loc_value(ast, source, base_offset, node_id);
            }
            value
        }
        Vue3AstKind::Text(text) => json!({
            "type": 2,
            "content": text.value,
            "loc": vue3_text_loc_value(source, base_offset, &node.span),
        }),
        Vue3AstKind::Interpolation(interpolation) => json!({
            "type": 5,
            "content": vue3_expression_value(source, base_offset, &interpolation.expression, &node.span, false, options, Vue3ExpressionAstMode::Expression),
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

fn vue3_parse_diagnostics(
    ast: &Vue3Ast,
    source: &str,
    base_offset: usize,
    options: &Vue3CompilerOptions,
) -> Vec<Value> {
    let mut diagnostics = Vec::new();
    collect_html_parse_error_diagnostics(source, options, &mut diagnostics);
    collect_invalid_lt_diagnostics(source, &mut diagnostics);
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
    for token in HtmlTokenizer::new(source).tokenize() {
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
                    let namespace =
                        vue3_tag_namespace(options, &name, stack.last().map(|open| open.namespace));
                    stack.push(OpenDiagnosticElement { name, namespace });
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
                        pop_diagnostic_stack_until(&mut stack, &name);
                    }
                } else if tag_token_is_incomplete(source, token.start, token.end) {
                    diagnostics.push(vue3_error_value(
                        9,
                        vue3_source_loc_value(source, source.len(), source.len()),
                    ));
                } else {
                    pop_diagnostic_stack_until(&mut stack, &name);
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
    }
}

struct OpenDiagnosticElement {
    name: String,
    namespace: vuec_ast::HtmlNamespace,
}

fn vue3_tag_namespace(
    options: &Vue3CompilerOptions,
    tag: &str,
    parent: Option<vuec_ast::HtmlNamespace>,
) -> vuec_ast::HtmlNamespace {
    options
        .namespaces
        .get(tag)
        .copied()
        .unwrap_or_else(|| parent.unwrap_or(vuec_ast::HtmlNamespace::Html))
}

fn pop_diagnostic_stack_until(stack: &mut Vec<OpenDiagnosticElement>, name: &str) {
    while let Some(open) = stack.pop() {
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

fn collect_missing_interpolation_end_diagnostics(
    source: &str,
    options: &Vue3CompilerOptions,
    diagnostics: &mut Vec<Value>,
) {
    let mut stack = Vec::<String>::new();
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
                let is_void = vue3_is_void_tag(options, &name);
                let starts_v_pre =
                    v_pre_depth == 0 && attributes.iter().any(|attr| attr.name == "v-pre");
                let in_v_pre = v_pre_depth > 0 || starts_v_pre;
                if !self_closing && !is_void {
                    stack.push(name);
                    if in_v_pre {
                        v_pre_depth += 1;
                    }
                }
            }
            HtmlTokenKind::EndTag { name } => {
                if !name.is_empty() {
                    while let Some(open) = stack.pop() {
                        let was_in_v_pre = v_pre_depth > 0;
                        if was_in_v_pre {
                            v_pre_depth -= 1;
                        }
                        if open.eq_ignore_ascii_case(&name) {
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
    let mut stack = Vec::<OpenElement>::new();
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
                if !self_closing
                    && !vue3_is_void_tag(options, &name)
                    && !tag_token_is_incomplete_at_eof(source, token.start, token.end)
                {
                    stack.push(OpenElement {
                        name,
                        start: token.start,
                        in_v_pre,
                    });
                    if in_v_pre {
                        v_pre_depth += 1;
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

struct OpenElement {
    name: String,
    start: usize,
    in_v_pre: bool,
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

fn vue3_prop_value(
    source: &str,
    base_offset: usize,
    prop: &Vue3Prop,
    options: &Vue3CompilerOptions,
) -> Value {
    match prop {
        Vue3Prop::Attribute(attr) => vue3_attribute_value(source, base_offset, attr),
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
    let inner_end = node
        .children
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
        .unwrap_or(open_end);
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
    options: &Vue3CompilerOptions,
    ast_mode: Vue3ExpressionAstMode,
) -> Value {
    vue3_expression_value_with_mode(
        source_text,
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

fn vue3_simple_expression_value(source: &str, is_static: bool, loc: Value) -> Value {
    json!({
        "type": 4,
        "loc": loc,
        "content": source,
        "isStatic": is_static,
        "constType": if is_static { 3 } else { 0 },
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
                .map(|expression| json!({ "type": expression_type_name(&expression) }))
        }
        Vue3ExpressionAstMode::Params => {
            let expression_source = format!("({trimmed})=>{{}}");
            store
                .parse_expression(&expression_source, source_type)
                .ok()
                .map(|expression| json!({ "type": expression_type_name(&expression) }))
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

fn expression_type_name(expression: &Expression<'_>) -> &'static str {
    match expression {
        Expression::ArrayExpression(_) => "ArrayExpression",
        Expression::ArrowFunctionExpression(_) => "ArrowFunctionExpression",
        Expression::AssignmentExpression(_) => "AssignmentExpression",
        Expression::AwaitExpression(_) => "AwaitExpression",
        Expression::BinaryExpression(_) => "BinaryExpression",
        Expression::CallExpression(_) => "CallExpression",
        Expression::ChainExpression(_) => "ChainExpression",
        Expression::ConditionalExpression(_) => "ConditionalExpression",
        Expression::FunctionExpression(_) => "FunctionExpression",
        Expression::Identifier(_) => "Identifier",
        Expression::LogicalExpression(_) => "LogicalExpression",
        Expression::ComputedMemberExpression(_)
        | Expression::StaticMemberExpression(_)
        | Expression::PrivateFieldExpression(_) => "MemberExpression",
        Expression::ObjectExpression(_) => "ObjectExpression",
        Expression::ParenthesizedExpression(parenthesized) => {
            expression_type_name(&parenthesized.expression)
        }
        Expression::SequenceExpression(_) => "SequenceExpression",
        Expression::TemplateLiteral(_) => "TemplateLiteral",
        Expression::ThisExpression(_) => "ThisExpression",
        Expression::UnaryExpression(_) => "UnaryExpression",
        Expression::UpdateExpression(_) => "UpdateExpression",
        _ => "Expression",
    }
}

fn statement_ast_value(statement: &Statement<'_>) -> Value {
    json!({ "type": statement_type_name(statement) })
}

fn statement_type_name(statement: &Statement<'_>) -> &'static str {
    match statement {
        Statement::BlockStatement(_) => "BlockStatement",
        Statement::BreakStatement(_) => "BreakStatement",
        Statement::ContinueStatement(_) => "ContinueStatement",
        Statement::DebuggerStatement(_) => "DebuggerStatement",
        Statement::DoWhileStatement(_) => "DoWhileStatement",
        Statement::EmptyStatement(_) => "EmptyStatement",
        Statement::ExpressionStatement(_) => "ExpressionStatement",
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

fn vue3_loc_value(source: &str, base_offset: usize, span: &NodeSpan) -> Value {
    let Some(span) = span.source() else {
        return vue3_loc_stub_value();
    };
    vue3_source_span_value(source, base_offset, span)
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

fn vue3_source_span_value(source: &str, base_offset: usize, span: vuec_source::Span) -> Value {
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
    let start_pos = vue3_position(source, start);
    let end_pos = vue3_position(source, end);
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
    if offset > index {
        column += offset - index;
    }
    json!({
        "offset": offset,
        "line": line,
        "column": column,
    })
}

fn vue3_signed_position(source: &str, offset: isize) -> Value {
    if offset >= 0 {
        return vue3_position(source, offset as usize);
    }
    json!({
        "offset": offset,
        "line": 1,
        "column": 1isize + offset,
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
    options.inline = bool_option(value, "inline", options.inline);
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
    if let Some(whitespace) = value.get("whitespace").and_then(Value::as_str) {
        options.whitespace = whitespace.to_string();
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
