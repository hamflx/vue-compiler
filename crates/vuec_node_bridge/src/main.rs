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
use vuec_html::{HtmlTokenKind, HtmlTokenizer};
use vuec_js::JsAstStore;
use vuec_sfc::{
    SfcAttrValue, SfcBlock, SfcBlockAttrs, SfcCompiler, SfcDescriptor, SfcScriptBlock,
    SfcScriptCompileOptions, SfcStyleCompileOptions, SfcTemplateCompileOptions,
    Vue27ParseComponentOptions, Vue27PrefixIdentifiersOptions, Vue27RewriteDefaultOptions,
    Vue27SfcPad, Vue27TemplatePreprocessOptions, Vue3SfcPad, Vue3SfcParseOptions,
    Vue3SfcParseProjectionOptions,
};
use vuec_source::FileId;
use vuec_style::{compile_style, CssVarNameStyle, StyleCompileOptions};
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
        "vue2.generate" => {
            let options = vue2_options(payload.get("options"));
            let element = payload
                .get("ast")
                .filter(|ast| !ast.is_null())
                .map(|ast| serde_json::from_value::<Vue2Element>(ast.clone()))
                .transpose()
                .context("failed to deserialize Vue 2 AST element for codegen")?;
            let generated = vuec_vue2::generate(element.as_ref(), &options);
            Ok(json!({
                "render": generated.render,
                "staticRenderFns": generated.static_render_fns,
                "static_render_fns": generated.static_render_fns,
            }))
        }
        "vue2.optimize" => {
            let options = vue2_options(payload.get("options"));
            let mut element = payload
                .get("ast")
                .filter(|ast| !ast.is_null())
                .map(|ast| serde_json::from_value::<Vue2Element>(ast.clone()))
                .transpose()
                .context("failed to deserialize Vue 2 AST element for optimizer")?;
            if let Some(element) = element.as_mut() {
                vuec_vue2::optimize(element, &options);
            }
            let public = element
                .as_ref()
                .map(vue2_public_element_ast_value)
                .unwrap_or(Value::Null);
            Ok(json!({
                "ast": public,
                "ast_public": public,
                "element_public_ast": public,
                "element_ast": element,
            }))
        }
        "vue3.core.baseCompile" => {
            let source = template_source(&payload);
            let options = vue3_options(payload.get("options"));
            Ok(vue3_base_compile_value(source, options))
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
                false,
            ))
        }
        "vue3.core.generate" => {
            let options = vue3_options(payload.get("options"));
            Ok(serde_json::to_value(vuec_vue3_core::generate_public_ast(
                payload.get("ast").unwrap_or(&Value::Null),
                &options,
            ))?)
        }
        "vue3.core.rootCodegen" => Ok(vuec_vue3_core::root_codegen_projection(
            payload.get("root").unwrap_or(&payload),
        )),
        "vue3.core.cacheStatic" => Ok(vuec_vue3_core::cache_static_projection(&payload)),
        "vue3.core.stringifyStatic" => Ok(vuec_vue3_core::stringify_static_projection(&payload)),
        "vue3.core.getConstantType" => Ok(vuec_vue3_core::get_constant_type_projection(&payload)),
        "vue3.core.isMemberExpression" => {
            Ok(vuec_vue3_core::is_member_expression_projection(&payload))
        }
        "vue3.core.isFunctionType" => Ok(vuec_vue3_core::is_function_type_projection(&payload)),
        "vue3.core.advancePositionWithClone" => Ok(
            vuec_vue3_core::advance_position_with_clone_projection(&payload),
        ),
        "vue3.core.advancePositionWithMutation" => Ok(
            vuec_vue3_core::advance_position_with_mutation_projection(&payload),
        ),
        "vue3.core.toValidAssetId" => Ok(vuec_vue3_core::to_valid_asset_id_projection(&payload)),
        "vue3.core.extractIdentifiers" => {
            Ok(vuec_vue3_core::extract_identifiers_projection(&payload))
        }
        "vue3.core.isStaticProperty" => Ok(vuec_vue3_core::is_static_property_projection(&payload)),
        "vue3.core.isInDestructureAssignment" => Ok(
            vuec_vue3_core::is_in_destructure_assignment_projection(&payload),
        ),
        "vue3.core.isReferencedIdentifier" => Ok(
            vuec_vue3_core::is_referenced_identifier_projection(&payload),
        ),
        "vue3.core.walkIdentifiers" => Ok(vuec_vue3_core::walk_identifiers_projection(&payload)),
        "vue3.core.processExpression" => {
            Ok(vuec_vue3_core::process_expression_projection(&payload))
        }
        "vue3.core.transformExpression" => {
            Ok(vuec_vue3_core::transform_expression_projection(&payload))
        }
        "vue3.core.transformOn" => Ok(vuec_vue3_core::transform_on_projection(&payload)),
        "vue3.core.transformBind" => Ok(vuec_vue3_core::transform_bind_projection(&payload)),
        "vue3.core.transformVBindShorthand" => Ok(
            vuec_vue3_core::transform_v_bind_shorthand_projection(&payload),
        ),
        "vue3.core.transformMemo" => Ok(vuec_vue3_core::transform_memo_projection(&payload)),
        "vue3.core.transformOnce" => Ok(vuec_vue3_core::transform_once_projection(&payload)),
        "vue3.core.transformModel" => Ok(vuec_vue3_core::transform_model_projection(&payload)),
        "vue3.core.transformIf" => Ok(vuec_vue3_core::transform_if_projection(&payload)),
        "vue3.core.transformFor" => Ok(vuec_vue3_core::transform_for_projection(&payload)),
        "vue3.core.trackSlotScopes" => Ok(vuec_vue3_core::track_slot_scopes_projection(&payload)),
        "vue3.core.trackVForSlotScopes" => {
            Ok(vuec_vue3_core::track_v_for_slot_scopes_projection(&payload))
        }
        "vue3.core.transformSlotOutlet" => {
            Ok(vuec_vue3_core::transform_slot_outlet_projection(&payload))
        }
        "vue3.core.buildSlots" => Ok(vuec_vue3_core::build_slots_projection(&payload)),
        "vue3.core.resolveComponentType" => {
            Ok(vuec_vue3_core::resolve_component_type_projection(&payload))
        }
        "vue3.core.transformElementProps" => {
            Ok(vuec_vue3_core::transform_element_props_projection(&payload))
        }
        "vue3.core.buildDirectiveArgs" => {
            Ok(vuec_vue3_core::build_directive_args_projection(&payload))
        }
        "vue3.core.transformElementChildren" => Ok(
            vuec_vue3_core::transform_element_children_projection(&payload),
        ),
        "vue3.core.transformText" => Ok(vuec_vue3_core::transform_text_projection(&payload)),
        "vue3.dom.transformStyle" => Ok(vuec_vue3_dom::transform_style_projection(&payload)),
        "vue3.dom.ignoreSideEffectTags" => {
            Ok(vuec_vue3_dom::ignore_side_effect_tags_projection(&payload))
        }
        "vue3.dom.decodeHtmlBrowser" => Ok(vuec_vue3_dom::decode_html_browser_projection(&payload)),
        "vue3.dom.transformVHtml" => Ok(vuec_vue3_dom::transform_v_html_projection(&payload)),
        "vue3.dom.transformVText" => Ok(vuec_vue3_dom::transform_v_text_projection(&payload)),
        "vue3.dom.transformShow" => Ok(vuec_vue3_dom::transform_show_projection(&payload)),
        "vue3.dom.transformOn" => Ok(vuec_vue3_dom::transform_on_projection(&payload)),
        "vue3.dom.transformModel" => Ok(vuec_vue3_dom::transform_model_projection(&payload)),
        "vue3.dom.transformTransition" => {
            Ok(vuec_vue3_dom::transform_transition_projection(&payload))
        }
        "vue3.dom.validateHtmlNesting" => {
            Ok(vuec_vue3_dom::validate_html_nesting_projection(&payload))
        }
        "vue3.dom.isValidHTMLNesting" => {
            Ok(vuec_vue3_dom::is_valid_html_nesting_projection(&payload))
        }
        "vue3.dom.compile" => {
            let source = template_source(&payload);
            let mut core = vue3_options(payload.get("options"));
            let default_options = DomCompilerOptions::default();
            apply_bridge_dom_parser_defaults(&mut core, payload.get("options"));
            if payload
                .get("options")
                .and_then(|options| options.get("mode"))
                .is_none()
            {
                core.mode = "function".to_string();
            }
            let options = DomCompilerOptions {
                core,
                transform_asset_urls: transform_asset_urls_enabled(
                    payload.get("options").unwrap_or(&Value::Null),
                    default_options.transform_asset_urls,
                ),
                asset_url_options: asset_url_options(
                    payload.get("options").unwrap_or(&Value::Null),
                    default_options.asset_url_options,
                ),
                decode_entities: bool_option(
                    payload.get("options").unwrap_or(&Value::Null),
                    "decodeEntities",
                    default_options.decode_entities,
                ),
                is_custom_element: string_array_option(
                    payload.get("options").unwrap_or(&Value::Null),
                    "isCustomElement",
                ),
            };
            let result = vuec_vue3_dom::compile(source.clone(), options);
            Ok(vue3_compile_value(result, &source))
        }
        "vue3.dom.parse" => {
            let source = template_source(&payload);
            let mut core = vue3_options(payload.get("options"));
            let default_options = DomCompilerOptions::default();
            apply_bridge_dom_parser_defaults(&mut core, payload.get("options"));
            let options = DomCompilerOptions {
                core,
                transform_asset_urls: transform_asset_urls_enabled(
                    payload.get("options").unwrap_or(&Value::Null),
                    default_options.transform_asset_urls,
                ),
                asset_url_options: asset_url_options(
                    payload.get("options").unwrap_or(&Value::Null),
                    default_options.asset_url_options,
                ),
                decode_entities: bool_option(
                    payload.get("options").unwrap_or(&Value::Null),
                    "decodeEntities",
                    default_options.decode_entities,
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
                false,
            ))
        }
        "vue3.ssr.compile" => {
            let source = template_source(&payload);
            let default_options = SsrCompilerOptions::default();
            let mut core = vue3_options(payload.get("options"));
            apply_bridge_dom_parser_defaults(&mut core, payload.get("options"));
            let options = SsrCompilerOptions {
                core,
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
                slotted_is_explicit: bridge_option_has(payload.get("options"), "slotted"),
                mode_is_explicit: bridge_option_has(payload.get("options"), "mode"),
                transform_asset_urls: transform_asset_urls_enabled(
                    payload.get("options").unwrap_or(&Value::Null),
                    default_options.transform_asset_urls,
                ),
                asset_url_options: asset_url_options(
                    payload.get("options").unwrap_or(&Value::Null),
                    default_options.asset_url_options,
                ),
            };
            let result = vuec_vue3_ssr::compile(source.clone(), options);
            Ok(vue3_ssr_compile_value(result, &source))
        }
        "sfc.parse" => {
            let filename = string_field_or(&payload, "filename", "anonymous.vue");
            let source = string_field(&payload, "source");
            let mut compiler = SfcCompiler::new();
            let parse_options = vue3_sfc_parse_options(payload.get("options"));
            let result = compiler.parse_vue3_with_options(filename, &source, parse_options.clone());
            let projection_options =
                vue3_sfc_parse_projection_options(payload.get("options"), &parse_options);
            let mut value = vuec_sfc::vue3_sfc_parse_result_value(&result, &projection_options);
            vue3_sfc_attach_template_ast(&mut value, &result.descriptor, payload.get("options"));
            Ok(value)
        }
        "sfc.vue27.parse" => {
            let filename = string_field_or(&payload, "filename", "anonymous.vue");
            let source = string_field(&payload, "source");
            let mut compiler = SfcCompiler::new();
            let descriptor = compiler.parse(filename, &source);
            Ok(vue27_descriptor_value(&descriptor))
        }
        "sfc.vue27.parseComponent" => {
            let source = string_field(&payload, "source");
            let mut compiler = SfcCompiler::new();
            let result = compiler.parse_vue27_component(
                &source,
                vue27_parse_component_options(payload.get("options")),
            );
            Ok(vue27_parse_component_value(
                &result.descriptor,
                &result.errors,
            ))
        }
        "sfc.vue27.rewriteDefault" => {
            let source = string_field(&payload, "source");
            let variable = string_field_or(&payload, "variable", "script");
            let compiler = SfcCompiler::new();
            Ok(json!(compiler.rewrite_vue27_default(
                &source,
                &variable,
                vue27_rewrite_default_options(payload.get("plugins")),
            )))
        }
        "sfc.vue27.prefixIdentifiers" => {
            let source = string_field(&payload, "source");
            let compiler = SfcCompiler::new();
            Ok(json!(compiler.prefix_vue27_identifiers(
                &source,
                vue27_prefix_identifiers_options(&payload),
            )))
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
            let filename = string_field_or(&payload, "filename", "template.vue.html");
            let options = vue27_sfc_template_vue2_options(payload.get("options"));
            let compiler = SfcCompiler::new();
            let preprocessed = compiler.preprocess_vue27_template(
                &source,
                vue27_template_preprocess_options(payload.get("options"), &filename),
            );
            if !preprocessed.errors.is_empty() || !preprocessed.tips.is_empty() {
                return Ok(json!({
                    "ast": {},
                    "code": "var render = function () {}\nvar staticRenderFns = []\n",
                    "source": source,
                    "tips": preprocessed.tips,
                    "errors": preprocessed.errors,
                }));
            }
            let compiled = vuec_vue2::compile(&preprocessed.source, options);
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
            let descriptor = compiler
                .parse_vue27_component_with_filename(
                    filename,
                    &source,
                    Vue27ParseComponentOptions::default(),
                )
                .descriptor;
            let script = compiler
                .compile_vue27_script(&descriptor, sfc_script_options(payload.get("options")));
            if let Some(error) = script.errors.first() {
                bail!("{error}");
            }
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
            let mut options = sfc_style_options(payload.get("options"));
            options.scoped = payload
                .get("options")
                .and_then(|value| value.get("scoped"))
                .and_then(Value::as_bool)
                .unwrap_or(true);
            let style = compile_style(
                &source,
                StyleCompileOptions {
                    id: options.id.clone(),
                    scoped: options.scoped,
                    vars: options.vars.clone(),
                    is_prod: options.is_prod,
                    css_var_name_style: CssVarNameStyle::Vue27Legacy,
                    css_var_ignore_line_comments: false,
                    filename: Some(filename),
                    source_map_source: Some(source.clone()),
                    source_map_file_id: Some(vuec_source::FileId(0)),
                    source_map_base_offset: 0,
                    source_map: options.source_map,
                    modules: options.modules,
                    modules_options: options.modules_options.clone(),
                    preprocess_lang: options.preprocess_lang,
                    preprocess_options: options.preprocess_options,
                    warn_deprecated_scoped_selectors: false,
                },
            );
            let mut value = json!({
                "code": style.code,
                "map": style.map,
                "errors": style.errors,
                "rawResult": ["postcss-result"],
                "dependencies": style.dependencies,
            });
            if !style.diagnostics.is_empty() {
                value["diagnostics"] = json!(style.diagnostics);
            }
            if let Some(modules) = style.modules {
                value["modules"] = json!(modules);
            }
            Ok(value)
        }
        other => bail!("unsupported bridge command `{other}`"),
    }
}

fn apply_bridge_dom_parser_defaults(core: &mut Vue3CompilerOptions, options: Option<&Value>) {
    let explicit_void_tags = bridge_option_has(options, "__vuecVoidTags");
    let explicit_pre_tags = bridge_option_has(options, "__vuecPreTags");
    let explicit_ignore_newline_tags = bridge_option_has(options, "__vuecIgnoreNewlineTags");
    let explicit_native_tags = bridge_option_has(options, "__vuecNativeTags");
    let void_tags = core.void_tags.clone();
    let pre_tags = core.pre_tags.clone();
    let ignore_newline_tags = core.ignore_newline_tags.clone();
    let native_tags = core.native_tags.clone();

    vuec_vue3_dom::apply_dom_parser_defaults(core);

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

fn bridge_option_has(options: Option<&Value>, name: &str) -> bool {
    options.is_some_and(|options| options.get(name).is_some())
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
    let filename = template_filename(payload);
    if let Some(source) = template_source_from_ast_payload(payload, filename.clone()) {
        return source;
    }
    TemplateSource {
        filename,
        source: string_field(payload, "source"),
        file_id: FileId(0),
        base_offset: 0,
    }
}

fn template_filename(payload: &Value) -> String {
    payload
        .get("filename")
        .or_else(|| {
            payload
                .get("options")
                .and_then(|options| options.get("filename"))
        })
        .and_then(Value::as_str)
        .unwrap_or("anonymous.vue")
        .to_string()
}

fn template_source_from_ast_payload(payload: &Value, filename: String) -> Option<TemplateSource> {
    let ast = payload.get("ast")?;
    let children = ast.get("children").and_then(Value::as_array)?;
    let source = ast
        .get("source")
        .or_else(|| payload.get("source"))
        .and_then(Value::as_str)
        .unwrap_or("");
    if children.is_empty() {
        return Some(TemplateSource {
            filename,
            source: String::new(),
            file_id: FileId(0),
            base_offset: 0,
        });
    }
    let mut start = usize::MAX;
    let mut end = 0usize;
    for child in children {
        if let Some((child_start, child_end)) =
            child.get("loc").and_then(|loc| loc_byte_range(source, loc))
        {
            start = start.min(child_start);
            end = end.max(child_end);
        }
    }
    if start == usize::MAX || end < start {
        return None;
    }
    Some(TemplateSource {
        filename,
        source: source.get(start..end).unwrap_or_default().to_string(),
        file_id: FileId(0),
        base_offset: start,
    })
}

fn loc_byte_range(source: &str, loc: &Value) -> Option<(usize, usize)> {
    let start = loc_offset(loc, "start")?;
    let end = loc_offset(loc, "end")?;
    if end < start {
        return None;
    }
    let loc_source = loc.get("source").and_then(Value::as_str);
    let byte_range = source.get(start..end).map(|slice| ((start, end), slice));
    if let Some(((byte_start, byte_end), slice)) = byte_range {
        if loc_source.is_none_or(|expected| expected == slice) {
            return Some((byte_start, byte_end));
        }
    }
    let utf16_range =
        utf16_offset_to_byte_index(source, start).zip(utf16_offset_to_byte_index(source, end));
    if let Some((utf16_start, utf16_end)) = utf16_range {
        if utf16_end >= utf16_start {
            if let Some(slice) = source.get(utf16_start..utf16_end) {
                if loc_source.is_none_or(|expected| expected == slice) {
                    return Some((utf16_start, utf16_end));
                }
            }
        }
    }
    byte_range
        .map(|((byte_start, byte_end), _)| (byte_start, byte_end))
        .or(utf16_range.filter(|(utf16_start, utf16_end)| utf16_end >= utf16_start))
}

fn loc_offset(loc: &Value, name: &str) -> Option<usize> {
    loc.get(name)?
        .get("offset")?
        .as_u64()
        .map(|offset| offset as usize)
}

fn utf16_offset_to_byte_index(source: &str, offset: usize) -> Option<usize> {
    let mut utf16_units = 0usize;
    for (byte_index, ch) in source.char_indices() {
        if utf16_units == offset {
            return Some(byte_index);
        }
        if utf16_units > offset {
            return None;
        }
        utf16_units += ch.len_utf16();
    }
    (utf16_units == offset).then_some(source.len())
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
    })
}

fn vue2_public_ast_value(compiled: &Vue2CompiledResult) -> Value {
    match compiled.element_ast.as_ref() {
        Some(element) => vue2_public_element_ast_value(element),
        None => Value::Null,
    }
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

fn vue27_parse_component_value(
    descriptor: &SfcDescriptor,
    errors: &[vuec_sfc::Vue27SfcParseError],
) -> Value {
    let mut value = vue27_descriptor_value(descriptor);
    value["errors"] = json!(errors);
    value
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
        "start": block.content_start,
        "end": block.content_end,
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
    if let Some(src) = block.attrs.src.as_ref() {
        value["src"] = json!(src);
    }
    if let Some(module) = block.attrs.module.as_ref() {
        if module.is_empty() {
            value["module"] = json!(true);
        } else {
            value["module"] = json!(module);
        }
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
    for (name, value) in &attrs.raw {
        object.insert(
            name.clone(),
            match value {
                SfcAttrValue::Bool(value) => json!(value),
                SfcAttrValue::String(value) => json!(value),
            },
        );
    }
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

fn vue27_css_vars(descriptor: &SfcDescriptor) -> Vec<String> {
    let mut vars = Vec::new();
    for style in &descriptor.styles {
        for var in vuec_style::collect_css_vars(&style.content) {
            if !vars.iter().any(|existing| existing == &var) {
                vars.push(var);
            }
        }
    }
    vars
}

fn vue3_sfc_parse_options(value: Option<&Value>) -> Vue3SfcParseOptions {
    let mut options = Vue3SfcParseOptions::default();
    let Some(value) = value else {
        return options;
    };
    options.ignore_empty = bool_option(value, "ignoreEmpty", options.ignore_empty);
    options.pad = vue3_sfc_pad_option(value.get("pad"));
    options
}

fn vue3_sfc_parse_projection_options(
    value: Option<&Value>,
    parse_options: &Vue3SfcParseOptions,
) -> Vue3SfcParseProjectionOptions {
    let mut options = Vue3SfcParseProjectionOptions::default();
    options.pad = parse_options.pad.clone();
    let Some(value) = value else {
        return options;
    };
    options.source_map = bool_option(value, "sourceMap", true);
    options.source_root = value
        .get("sourceRoot")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    options
}

fn vue3_sfc_pad_option(value: Option<&Value>) -> Vue3SfcPad {
    match value {
        Some(Value::Bool(true)) => Vue3SfcPad::Line,
        Some(Value::String(value)) if value == "line" => Vue3SfcPad::Line,
        Some(Value::String(value)) if value == "space" => Vue3SfcPad::Space,
        _ => Vue3SfcPad::False,
    }
}

fn vue3_sfc_attach_template_ast(
    result: &mut Value,
    descriptor: &SfcDescriptor,
    parse_options: Option<&Value>,
) {
    let Some(template) = descriptor.template.as_ref() else {
        return;
    };
    if template.attrs.has_src_attr() {
        return;
    }
    let ast = vue3_sfc_template_ast_value(descriptor, template, parse_options);
    if let Some(template_value) = result
        .get_mut("descriptor")
        .and_then(|descriptor| descriptor.get_mut("template"))
        .and_then(Value::as_object_mut)
    {
        template_value.insert("ast".into(), ast);
    }
}

fn vue3_sfc_template_ast_value(
    descriptor: &SfcDescriptor,
    template: &SfcBlock,
    parse_options: Option<&Value>,
) -> Value {
    let null = Value::Null;
    let template_options = parse_options
        .and_then(|options| options.get("templateParseOptions"))
        .unwrap_or(&null);
    let mut core = vue3_options(Some(template_options));
    core.prefix_identifiers = true;
    apply_bridge_dom_parser_defaults(&mut core, Some(template_options));
    let default_options = DomCompilerOptions::default();
    let dom_options = DomCompilerOptions {
        core,
        transform_asset_urls: false,
        asset_url_options: default_options.asset_url_options.clone(),
        decode_entities: bool_option(
            template_options,
            "decodeEntities",
            default_options.decode_entities,
        ),
        is_custom_element: string_array_option(template_options, "isCustomElement"),
    };
    let source = TemplateSource {
        filename: descriptor.filename.clone(),
        source: template.content.clone(),
        file_id: descriptor.source_file,
        base_offset: template.content_start,
    };
    let ast = vuec_vue3_dom::parse(source, &dom_options);
    let mut value = vue3_parse_value(&ast, &descriptor.source, 0, false, &dom_options.core, false);
    if let Some(object) = value.as_object_mut() {
        object.insert("source".into(), json!(descriptor.source));
        object.insert("loc".into(), vue3_loc_stub_value());
        object.remove("__vuecDiagnostics");
    }
    value
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
        object.insert("content".into(), json!(script.content.clone()));
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
        object["bindings"] = json!(script.bindings);
        object["imports"] = json!({});
    }
    value
}

fn block_content_start_from_loc(loc: &vuec_sfc::SfcBlockLocation) -> usize {
    loc.start
}

fn block_content_end_from_loc(loc: &vuec_sfc::SfcBlockLocation) -> usize {
    loc.end
}

fn vue3_base_compile_value(source: TemplateSource, options: Vue3CompilerOptions) -> Value {
    let mut ast = Vue3Dialect::base_parse(source.clone(), &options);
    let mut ctx = vuec_pass::TransformContext::default();
    Vue3Dialect::transform(&mut ast, &mut ctx, &options);
    let result = Vue3Dialect::finish_compile(ast.clone(), source.clone(), options.clone(), ctx);
    let ast_value = vue3_parse_value(
        &ast,
        &source.source,
        source.base_offset,
        false,
        &options,
        true,
    );
    json!({
        "ast": ast_value,
        "code": result.code,
        "preamble": result.preamble,
        "map": result.map,
        "diagnostics": vue3_compile_diagnostics_value(
            &result.diagnostics,
            &source.source,
            source.base_offset,
        ),
    })
}

fn vue3_compile_value(result: vuec_vue3_core::CodegenResult, source: &TemplateSource) -> Value {
    json!({
        "code": result.code,
        "map": result.map,
        "ast_summary": result.ast_summary,
        "diagnostics": vue3_compile_diagnostics_value(
            &result.diagnostics,
            &source.source,
            source.base_offset,
        ),
        "preamble": result.preamble,
    })
}

fn vue3_ssr_compile_value(
    result: vuec_vue3_ssr::SsrCompileResult,
    source: &TemplateSource,
) -> Value {
    json!({
        "code": result.code,
        "map": result.map,
        "ast_helpers": result.ast_helpers,
        "ast_summary": result.ast_summary,
        "diagnostics": vue3_compile_diagnostics_value(
            &result.diagnostics,
            &source.source,
            source.base_offset,
        ),
        "preamble": result.preamble,
    })
}

fn vue3_compile_diagnostics_value(
    diagnostics: &[vuec_diagnostics::Diagnostic],
    source: &str,
    base_offset: usize,
) -> Vec<Value> {
    diagnostics
        .iter()
        .map(|diagnostic| {
            json!({
                "code": diagnostic.code.parse::<u32>().ok().unwrap_or(0),
                "message": diagnostic.message,
                "loc": diagnostic.span.map(|span| vue3_source_span_value(source, base_offset, span)).unwrap_or_else(vue3_loc_stub_value),
            })
        })
        .collect()
}

fn vue3_parse_value(
    ast: &Vue3Ast,
    source: &str,
    base_offset: usize,
    include_sfc_inner_loc: bool,
    options: &Vue3CompilerOptions,
    include_codegen: bool,
) -> Value {
    let imports = vue3_root_imports_value(ast);
    json!({
        "type": 0,
        "source": source,
        "children": vue3_root_children(ast, source, base_offset, include_sfc_inner_loc, options, include_codegen),
        "helpers": [],
        "components": [],
        "directives": [],
        "hoists": [],
        "imports": imports,
        "cached": [],
        "temps": 0,
        "codegenNode": Value::Null,
        "loc": ast.root_node().map(|node| vue3_loc_value(source, base_offset, &node.span)).unwrap_or_else(vue3_loc_stub_value),
        "__vuecDiagnostics": vue3_parse_diagnostics(ast, source, base_offset, options),
    })
}

fn vue3_root_imports_value(ast: &Vue3Ast) -> Vec<Value> {
    ast.root_node()
        .and_then(|node| match &node.kind {
            Vue3AstKind::Root(root) => Some(&root.imports),
            _ => None,
        })
        .into_iter()
        .flatten()
        .map(vue3_import_item_value)
        .collect()
}

fn vue3_import_item_value(import: &Vue3ImportItem) -> Value {
    json!({
        "exp": {
            "type": 4,
            "content": import.name,
            "isStatic": false,
            "constType": 3,
            "loc": vue3_loc_stub_value(),
        },
        "path": import.path,
    })
}

fn vue3_root_children(
    ast: &Vue3Ast,
    source: &str,
    base_offset: usize,
    include_sfc_inner_loc: bool,
    options: &Vue3CompilerOptions,
    include_codegen: bool,
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
                        include_codegen,
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
    include_codegen: bool,
) -> Value {
    let Some(node) = ast.node(node_id) else {
        return Value::Null;
    };
    match &node.kind {
        Vue3AstKind::Root(root) => json!({
            "type": 0,
            "source": source,
            "children": node.children.iter().filter_map(|child_id| ast.node(*child_id)).map(|child| vue3_node_summary(ast, source, base_offset, child.id, include_sfc_inner_loc, options, include_codegen)).collect::<Vec<_>>(),
            "helpers": [],
            "components": [],
            "directives": [],
            "hoists": [],
            "imports": root.imports.iter().map(vue3_import_item_value).collect::<Vec<_>>(),
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
                "children": node.children.iter().filter_map(|child_id| ast.node(*child_id)).map(|child| vue3_node_summary(ast, source, base_offset, child.id, include_sfc_inner_loc, options, include_codegen)).collect::<Vec<_>>(),
                "loc": vue3_loc_value(source, base_offset, &node.span),
                "codegenNode": Value::Null,
                "isSelfClosing": if element.self_closing { json!(true) } else { json!(null) },
            });
            if include_codegen {
                value["codegenNode"] =
                    vue3_element_codegen_value(ast, node_id, source, base_offset, element, options);
            }
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
    collect_invalid_lt_diagnostics(ast, source, base_offset, options, &mut diagnostics);
    collect_missing_interpolation_end_diagnostics(source, options, &mut diagnostics);
    collect_invalid_end_tag_diagnostics(ast, source, base_offset, options, &mut diagnostics);
    collect_missing_directive_name_diagnostics(ast, source, base_offset, &mut diagnostics);
    diagnostics
}

fn vue3_element_codegen_value(
    ast: &Vue3Ast,
    node_id: vuec_ast::NodeId,
    source: &str,
    base_offset: usize,
    element: &vuec_ast::Vue3Element,
    options: &Vue3CompilerOptions,
) -> Value {
    if element.tag_type != vuec_ast::Vue3ElementType::Element {
        return Value::Null;
    }
    let is_root = ast.node(node_id).and_then(|node| node.parent) == Some(ast.root);
    let patch_flag =
        vuec_vue3_core::vue3_element_codegen_patch_flag(ast, node_id, options, is_root);
    json!({
        "type": 13,
        "tag": format!("\"{}\"", element.tag),
        "props": Value::Null,
        "children": Value::Null,
        "patchFlag": patch_flag,
        "dynamicProps": Value::Null,
        "directives": Value::Null,
        "isBlock": is_root,
        "disableTracking": false,
        "isComponent": false,
        "loc": ast.node(node_id).map(|node| vue3_loc_value(source, base_offset, &node.span)).unwrap_or_else(vue3_loc_stub_value),
    })
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
    let mut utf16_offset = 0usize;
    for ch in source.chars() {
        if index >= offset {
            break;
        }
        index += ch.len_utf8();
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += ch.len_utf16();
        }
        utf16_offset += ch.len_utf16();
    }
    if offset > index {
        let extra = offset - index;
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
    options.disable_default_must_use_prop = bool_option(
        value,
        "__vuecDisableDefaultMustUseProp",
        bool_option(value, "disable_default_must_use_prop", false),
    );
    if let Some(namespaces) = string_map_option(value, "__vuecTagNamespaces") {
        options.tag_namespaces = namespaces;
        options.use_default_tag_namespaces = false;
    }
    options.use_default_tag_namespaces = bool_option(
        value,
        "__vuecUseDefaultTagNamespaces",
        bool_option(
            value,
            "use_default_tag_namespaces",
            options.use_default_tag_namespaces,
        ),
    );
    if value.get("__vuecReservedTags").is_some() {
        options.reserved_tags = Some(string_array_option(value, "__vuecReservedTags"));
        options.use_default_reserved_tags = false;
    }
    options.use_default_reserved_tags = bool_option(
        value,
        "__vuecUseDefaultReservedTags",
        bool_option(
            value,
            "use_default_reserved_tags",
            options.use_default_reserved_tags,
        ),
    );
    if let Some(bindings) = string_map_option(value, "bindings") {
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

fn vue27_sfc_template_vue2_options(value: Option<&Value>) -> Vue2CompileOptions {
    let mut options = Vue2CompileOptions::default();
    let Some(value) = value else {
        return options;
    };
    options.bindings = string_map_option(value, "bindings").unwrap_or_default();
    if let Some(bindings) = value.get("bindings") {
        options.bindings_is_script_setup = bindings
            .get("__isScriptSetup")
            .and_then(Value::as_bool)
            .unwrap_or(options.bindings_is_script_setup);
    }
    if transform_asset_urls_enabled(value, false) {
        options.sfc_asset_url_transform = Some(vue27_sfc_asset_url_options(value));
    }
    options
}

fn vue27_template_preprocess_options(
    value: Option<&Value>,
    filename: &str,
) -> Vue27TemplatePreprocessOptions {
    let lang = value
        .and_then(|value| value.get("preprocessLang"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    Vue27TemplatePreprocessOptions {
        lang,
        filename: Some(filename.to_string()),
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
    options.stringify_static = bool_option(
        value,
        "stringifyStatic",
        bool_option(
            value,
            "__vuecStringifyStatic",
            bool_option(value, "stringify_static", options.stringify_static),
        ),
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
    options.cache_handlers = bool_option(
        value,
        "cacheHandlers",
        bool_option(value, "cache_handlers", options.cache_handlers),
    );
    options.slotted = bool_option(value, "slotted", options.slotted);
    options.inline = bool_option(value, "inline", options.inline);
    options.ssr = bool_option(value, "ssr", options.ssr);
    options.optimize_imports = bool_option(value, "optimizeImports", options.optimize_imports);
    options.is_ts = bool_option(value, "isTS", bool_option(value, "is_ts", options.is_ts));
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
        .unwrap_or(0) as usize;
    options.ssr_css_vars = value
        .get("ssrCssVars")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
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

fn string_map_option(value: &Value, name: &str) -> Option<BTreeMap<String, String>> {
    value.get(name).and_then(Value::as_object).map(|object| {
        object
            .iter()
            .filter_map(|(key, value)| value.as_str().map(|value| (key.clone(), value.to_string())))
            .collect()
    })
}

fn transform_asset_urls_enabled(value: &Value, fallback: bool) -> bool {
    match value.get("transformAssetUrls") {
        Some(Value::Bool(enabled)) => *enabled,
        Some(Value::Object(_)) => true,
        _ => fallback,
    }
}

fn asset_url_options(value: &Value, mut options: AssetUrlOptions) -> AssetUrlOptions {
    let Some(raw) = value.get("transformAssetUrls") else {
        return options;
    };
    match raw {
        Value::Bool(_) => options,
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
            } else if object
                .iter()
                .any(|(_, value)| matches!(value, Value::Array(_)))
            {
                options.tags = asset_url_tags(object);
            }
            options
        }
        _ => options,
    }
}

fn asset_url_tags(object: &serde_json::Map<String, Value>) -> BTreeMap<String, Vec<String>> {
    object
        .iter()
        .filter_map(|(tag, attrs)| {
            let attrs = attrs
                .as_array()?
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>();
            Some((tag.clone(), attrs))
        })
        .collect()
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
        transform_asset_urls_enabled(value, options.transform_asset_urls);
    options.asset_url_options = asset_url_options(value, options.asset_url_options);
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
            "vue27Legacy" | "vue27_legacy" | "legacy" => CssVarNameStyle::Vue27Legacy,
            _ => CssVarNameStyle::Vue3Escaped,
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

fn vue27_parse_component_options(value: Option<&Value>) -> Vue27ParseComponentOptions {
    let mut options = Vue27ParseComponentOptions::default();
    let Some(value) = value else {
        return options;
    };
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

fn vue27_rewrite_default_options(value: Option<&Value>) -> Vue27RewriteDefaultOptions {
    let Some(value) = value else {
        return Vue27RewriteDefaultOptions::default();
    };
    let plugins = value
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_else(|| std::slice::from_ref(value));
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

fn vue27_prefix_identifiers_options(value: &Value) -> Vue27PrefixIdentifiersOptions {
    Vue27PrefixIdentifiersOptions {
        is_functional: bool_option(value, "isFunctional", false),
        is_ts: bool_option(value, "isTS", false),
        bindings: json_string_map_option(value, "bindings").unwrap_or_default(),
    }
}

fn bool_option(value: &Value, name: &str, fallback: bool) -> bool {
    value.get(name).and_then(Value::as_bool).unwrap_or(fallback)
}

fn json_string_map_option(value: &Value, name: &str) -> Option<BTreeMap<String, String>> {
    value.get(name).and_then(Value::as_object).map(|object| {
        object
            .iter()
            .filter_map(|(key, value)| match value {
                Value::String(value) => Some((key.clone(), value.clone())),
                Value::Bool(value) => Some((key.clone(), value.to_string())),
                _ => None,
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn first_projected_prop(source_text: &str) -> Value {
        let source = TemplateSource {
            filename: "foo.vue".into(),
            source: source_text.into(),
            file_id: FileId(0),
            base_offset: 0,
        };
        let options = Vue3CompilerOptions::default();
        let ast = Vue3Dialect::base_parse(source.clone(), &options);
        let projected = vue3_parse_value(
            &ast,
            &source.source,
            source.base_offset,
            false,
            &options,
            false,
        );
        projected["children"][0]["props"][0].clone()
    }

    #[test]
    fn vue3_directive_projection_preserves_dynamic_arg_exp_and_modifier_exactness() {
        let directive = first_projected_prop(r#"<div v-bind:[foo].camel="  bar  "/>"#);

        assert_eq!(directive["name"], json!("bind"));
        assert_eq!(directive["rawName"], json!("v-bind:[foo].camel"));
        assert_eq!(directive["arg"]["content"], json!("foo"));
        assert_eq!(directive["arg"]["isStatic"], json!(false));
        assert_eq!(directive["arg"]["loc"]["source"], json!("[foo]"));
        assert_eq!(directive["exp"]["content"], json!("  bar  "));
        assert_eq!(directive["exp"]["loc"]["source"], json!("  bar  "));
        assert_eq!(directive["modifiers"][0]["content"], json!("camel"));
        assert_eq!(directive["modifiers"][0]["isStatic"], json!(true));
        assert_eq!(directive["modifiers"][0]["loc"]["source"], json!("camel"));
    }

    #[test]
    fn vue3_directive_projection_preserves_prop_shorthand_synthetic_modifier_shape() {
        let directive = first_projected_prop(r#"<div .foo="bar"/>"#);

        assert_eq!(directive["name"], json!("bind"));
        assert_eq!(directive["rawName"], json!(".foo"));
        assert_eq!(directive["arg"]["content"], json!("foo"));
        assert_eq!(directive["arg"]["isStatic"], json!(true));
        assert_eq!(directive["arg"]["loc"]["source"], json!("foo"));
        assert_eq!(directive["exp"]["content"], json!("bar"));
        assert_eq!(directive["modifiers"][0]["content"], json!("prop"));
        assert_eq!(directive["modifiers"][0]["isStatic"], json!(false));
        assert_eq!(directive["modifiers"][0]["loc"]["source"], json!(""));
    }

    #[test]
    fn vue27_bridge_compile_style_rewrites_css_vars_with_default_scope() {
        let compiled = dispatch(
            "sfc.vue27.compileStyle",
            json!({
                "source": ".foo { color: v-bind(color); font-size: v-bind('font.size'); }",
                "filename": "test.css",
                "options": {
                    "id": "data-v-test"
                }
            }),
        )
        .expect("vue27 style");

        let code = compiled["code"].as_str().unwrap_or("");
        assert!(code.contains(".foo[data-v-test]"));
        assert!(code.contains("var(--test-color)"));
        assert!(code.contains("var(--test-font_size)"));
    }

    #[test]
    fn vue27_bridge_parse_collects_comment_separated_css_vars() {
        let parsed = dispatch(
            "sfc.vue27.parse",
            json!({
                "source": r#"<style>.foo { color: v-bind/**/(color); font-size: v-bind /*x*/ ('font.size'); }</style>"#,
                "filename": "test.vue"
            }),
        )
        .expect("vue27 parse");

        assert_eq!(parsed["cssVars"], json!(["color", "font.size"]));
    }

    #[test]
    fn vue3_sfc_bridge_parse_projects_public_descriptor_shape() {
        let parsed = dispatch(
            "sfc.parse",
            json!({
                "source": concat!(
                    "<template><div>{{ msg }}</div></template>",
                    "<script setup lang=\"ts\">const msg: string = 'hi'</script>",
                    "<style scoped>.a{color:v-bind(color)}</style>",
                    "<i18n lang=\"json\">{\"en\":\"hi\"}</i18n>"
                ),
                "filename": "Comp.vue",
                "options": {
                    "sourceMap": false
                }
            }),
        )
        .expect("vue3 sfc parse");

        let descriptor = &parsed["descriptor"];
        assert_eq!(descriptor["template"]["type"], json!("template"));
        assert_eq!(
            descriptor["template"]["loc"]["source"],
            json!("<div>{{ msg }}</div>")
        );
        assert_eq!(
            descriptor["template"]["ast"]["source"],
            descriptor["source"]
        );
        assert_eq!(
            descriptor["template"]["ast"]["children"][0]["tag"],
            json!("div")
        );
        assert_eq!(descriptor["scriptSetup"]["setup"], json!(true));
        assert_eq!(descriptor["scriptSetup"]["lang"], json!("ts"));
        assert_eq!(descriptor["styles"][0]["scoped"], json!(true));
        assert_eq!(descriptor["cssVars"], json!(["color"]));
        assert_eq!(descriptor["customBlocks"][0]["type"], json!("i18n"));
        assert!(descriptor.get("script_setup").is_none());
        assert_eq!(parsed["errors"], json!([]));
    }

    #[test]
    fn vue3_sfc_bridge_parse_returns_descriptor_validation_errors() {
        let parsed = dispatch(
            "sfc.parse",
            json!({
                "source": concat!(
                    "<template>a</template>",
                    "<template>b</template>",
                    "<script src=\"x\"></script>",
                    "<script setup>ok</script>"
                ),
                "filename": "Dup.vue"
            }),
        )
        .expect("vue3 sfc parse");

        let descriptor = &parsed["descriptor"];
        assert_eq!(descriptor["template"]["content"], json!("a"));
        assert!(descriptor["script"].is_null());
        assert_eq!(descriptor["scriptSetup"]["content"], json!("ok"));
        assert_eq!(
            parsed["errors"][0]["message"],
            json!("Single file component can contain only one <template> element")
        );
        assert_eq!(
            parsed["errors"][0]["loc"]["source"],
            json!("<template>b</template>")
        );
        assert_eq!(
            parsed["errors"][1]["message"],
            json!("<script> cannot use the \"src\" attribute when <script setup> is also present because they must be processed together.")
        );
    }

    #[test]
    fn vue3_sfc_bridge_parse_preserves_src_presence_and_functional_template_error() {
        let src_parsed = dispatch(
            "sfc.parse",
            json!({
                "source": "<template src></template><script src></script><style src></style>",
                "filename": "BoolSrc.vue"
            }),
        )
        .expect("vue3 sfc parse");

        let descriptor = &src_parsed["descriptor"];
        assert_eq!(descriptor["template"]["attrs"]["src"], json!(true));
        assert!(descriptor["template"].get("src").is_none());
        assert!(descriptor["template"].get("map").is_none());
        assert!(descriptor["template"].get("ast").is_none());
        assert_eq!(descriptor["script"]["attrs"]["src"], json!(true));
        assert_eq!(descriptor["styles"][0]["attrs"]["src"], json!(true));
        assert_eq!(src_parsed["errors"], json!([]));

        let functional = dispatch(
            "sfc.parse",
            json!({
                "source": r#"<template functional="x"><div/></template>"#,
                "filename": "Functional.vue"
            }),
        )
        .expect("vue3 sfc parse");
        assert_eq!(
            functional["errors"][0]["message"],
            json!("<template functional> is no longer supported in Vue 3, since functional components no longer have significant performance difference from stateful ones. Just use a normal <template> instead.")
        );
        assert_eq!(
            functional["errors"][0]["loc"]["source"],
            json!("functional=\"x\"")
        );
    }

    #[test]
    fn vue3_sfc_bridge_parse_decodes_attrs_and_duplicate_attr_errors() {
        let parsed = dispatch(
            "sfc.parse",
            json!({
                "source": r#"<template a="1" a="&amp;">x</template><style module="m&amp;n" setup>.a{}</style>"#,
                "filename": "Attrs.vue",
                "options": {
                    "sourceMap": false
                }
            }),
        )
        .expect("vue3 sfc parse");

        let descriptor = &parsed["descriptor"];
        assert_eq!(descriptor["template"]["attrs"]["a"], json!("&"));
        assert_eq!(descriptor["styles"][0]["module"], json!("m&n"));
        assert!(descriptor["styles"][0].get("setup").is_none());
        assert_eq!(
            parsed["errors"][0]["message"],
            json!("Duplicate attribute.")
        );
        assert_eq!(parsed["errors"][0]["loc"]["start"]["offset"], json!(16));
    }

    #[test]
    fn vue3_sfc_bridge_parse_applies_padding_and_ignore_empty_options() {
        let parsed = dispatch(
            "sfc.parse",
            json!({
                "source": concat!(
                    "<template lang=\"pug\">\n  div\n</template>\n",
                    "<script>\nconst a = 1\n</script>\n",
                    "<style> </style>"
                ),
                "filename": "Pad.vue",
                "options": {
                    "pad": "line",
                    "ignoreEmpty": false,
                    "sourceMap": false
                }
            }),
        )
        .expect("vue3 sfc parse");

        let descriptor = &parsed["descriptor"];
        assert_eq!(descriptor["template"]["content"], json!("\ndiv\n"));
        assert_eq!(
            descriptor["script"]["content"],
            json!("//\n//\n//\n\nconst a = 1\n")
        );
        assert_eq!(descriptor["styles"][0]["content"], json!("\n\n\n\n\n\n "));
    }

    #[test]
    fn vue27_bridge_compile_script_passes_css_var_options() {
        let compiled = dispatch(
            "sfc.vue27.compileScript",
            json!({
                "source": "<script>const a = 1</script><style>div{ color: v-bind(color); }</style>",
                "filename": "test.vue",
                "options": {
                    "id": "xxxxxxxx",
                    "isProd": true
                }
            }),
        )
        .expect("vue27 script");

        let content = compiled["content"].as_str().unwrap_or("");
        assert!(content.contains("\"4003f1a6\": (_vm.color)"));
        assert!(content.contains("export default __default__"));
    }

    #[test]
    fn vue27_bridge_compile_script_preserves_script_ast_and_internal_binding_flag() {
        let compiled = dispatch(
            "sfc.vue27.compileScript",
            json!({
                "source": "<script>export default { props: ['foo'] }</script>",
                "filename": "test.vue",
                "options": {}
            }),
        )
        .expect("vue27 script");

        assert!(compiled["scriptAst"].as_array().is_some());
        assert_eq!(compiled["bindings"]["foo"], json!("props"));
        assert_eq!(compiled["bindings"]["__isScriptSetup"], json!("false"));
    }

    #[test]
    fn vue3_dom_bridge_uses_dom_builtin_defaults() {
        let parsed = dispatch(
            "vue3.dom.parse",
            json!({ "source": "<transition/><transition-group/>", "options": {} }),
        )
        .expect("dom parse");

        assert_eq!(parsed["children"][0]["tagType"], json!(1));
        assert_eq!(parsed["children"][1]["tagType"], json!(1));

        let compiled = dispatch(
            "vue3.dom.compile",
            json!({ "source": "<transition><div/><div/></transition>", "options": {} }),
        )
        .expect("dom compile");

        assert!(compiled["code"]
            .as_str()
            .unwrap_or("")
            .contains("_Transition"));
        assert!(compiled["diagnostics"]
            .as_array()
            .unwrap_or(&Vec::new())
            .iter()
            .any(
                |diagnostic| diagnostic.get("message").and_then(Value::as_str)
                    == Some("<Transition> expects exactly one child element or component.")
            ));
    }

    #[test]
    fn vue3_dom_bridge_projects_compile_diagnostic_objects() {
        let compiled = dispatch(
            "vue3.dom.compile",
            json!({
                "source": r#"<div :bar="a[" v-model="baz"/>"#,
                "options": {
                    "mode": "module",
                    "prefixIdentifiers": true
                }
            }),
        )
        .expect("dom compile");

        let diagnostics = compiled["diagnostics"].as_array().expect("diagnostics");
        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0]["code"], json!(46));
        assert!(diagnostics[0]["message"]
            .as_str()
            .unwrap_or("")
            .contains("Error parsing JavaScript expression: Unexpected token"));
        assert_eq!(diagnostics[0]["loc"]["start"]["offset"], json!(13));
        assert_eq!(diagnostics[1]["code"], json!(58));
        assert_eq!(diagnostics[1]["loc"]["source"], json!("v-model=\"baz\""));
    }

    #[test]
    fn vue3_dom_bridge_projects_template_expression_public_ast() {
        let parsed = dispatch(
            "vue3.dom.parse",
            json!({
                "source": concat!(
                    r#"<FooBar #[foo.slotName] :class="[cond ? '' : bar(), 'default']">"#,
                    r#"{{ `${VAR}VAR2${VAR3}` }}{{ Foo.Bar.Baz }}"#,
                    r#"</FooBar>"#
                ),
                "options": {
                    "mode": "module",
                    "prefixIdentifiers": true
                }
            }),
        )
        .expect("dom parse");

        let node = &parsed["children"][0];
        let dynamic_arg = &node["props"][0]["arg"]["ast"];
        assert_eq!(dynamic_arg["type"], json!("MemberExpression"));
        assert_eq!(dynamic_arg["object"]["name"], json!("foo"));
        assert_eq!(dynamic_arg["property"]["name"], json!("slotName"));

        let class_exp = &node["props"][1]["exp"]["ast"];
        assert_eq!(class_exp["type"], json!("ArrayExpression"));
        assert_eq!(
            class_exp["elements"][0]["type"],
            json!("ConditionalExpression")
        );
        assert_eq!(class_exp["elements"][0]["test"]["name"], json!("cond"));
        assert_eq!(
            class_exp["elements"][0]["alternate"]["callee"]["name"],
            json!("bar")
        );

        let template_literal = &node["children"][0]["content"]["ast"];
        assert_eq!(template_literal["type"], json!("TemplateLiteral"));
        assert_eq!(template_literal["expressions"][0]["name"], json!("VAR"));
        assert_eq!(template_literal["expressions"][1]["name"], json!("VAR3"));

        let member = &node["children"][1]["content"]["ast"];
        assert_eq!(member["type"], json!("MemberExpression"));
        assert_eq!(member["object"]["object"]["name"], json!("Foo"));
        assert_eq!(member["object"]["property"]["name"], json!("Bar"));
        assert_eq!(member["property"]["name"], json!("Baz"));
    }

    #[test]
    fn vue3_dom_bridge_compile_ast_slices_sfc_template_children() {
        let source =
            "<template><div>{{ msg }}</div></template><script>boom()</script><style>.x{}</style>";
        let compiled = dispatch(
            "vue3.dom.compile",
            json!({
                "source": source,
                "ast": {
                    "type": 0,
                    "source": source,
                    "children": [{
                        "type": 1,
                        "tag": "div",
                        "loc": {
                            "start": { "offset": 10 },
                            "end": { "offset": 30 },
                            "source": "<div>{{ msg }}</div>"
                        }
                    }]
                },
                "options": {
                    "mode": "module",
                    "prefixIdentifiers": true,
                    "sourceMap": true,
                    "__vuecSourceMapSource": source,
                    "__vuecSourceMapBaseOffset": 0
                }
            }),
        )
        .expect("dom compile");

        let code = compiled["code"].as_str().unwrap_or("");
        assert!(code.contains("_ctx.msg"));
        assert!(!compiled["diagnostics"]
            .as_array()
            .unwrap_or(&Vec::new())
            .iter()
            .any(|diagnostic| diagnostic
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("")
                .contains("side effect")));
        assert_eq!(compiled["map"]["sourcesContent"][0], source);
        assert!(compiled["map"]["mappings"].as_str().unwrap_or("").len() > 4);
    }

    #[test]
    fn vue3_ssr_bridge_compile_ast_slices_sfc_template_children() {
        let source =
            "<template><div>{{ msg }}</div></template><script>boom()</script><style>.x{}</style>";
        let compiled = dispatch(
            "vue3.ssr.compile",
            json!({
                "source": source,
                "ast": {
                    "type": 0,
                    "source": source,
                    "children": [{
                        "type": 1,
                        "tag": "div",
                        "loc": {
                            "start": { "offset": 10 },
                            "end": { "offset": 30 },
                            "source": "<div>{{ msg }}</div>"
                        }
                    }]
                },
                "options": {
                    "mode": "module",
                    "prefixIdentifiers": true,
                    "sourceMap": true,
                    "__vuecSourceMapSource": source,
                    "__vuecSourceMapBaseOffset": 0
                }
            }),
        )
        .expect("ssr compile");

        let code = compiled["code"].as_str().unwrap_or("");
        assert!(code.contains("_ssrInterpolate(_ctx.msg)"));
        assert!(!code.contains("boom"));
        assert_eq!(compiled["map"]["sources"], json!(["anonymous.vue"]));
        assert_eq!(compiled["map"]["sourcesContent"][0], source);
        assert!(compiled["map"]["mappings"].as_str().unwrap_or("").len() > 4);
    }

    #[test]
    fn vue3_ssr_bridge_uses_public_compile_defaults() {
        let compiled = dispatch(
            "vue3.ssr.compile",
            json!({
                "source": "<div>{{ msg }}</div>",
                "options": {
                    "prefixIdentifiers": false,
                    "cacheHandlers": true,
                    "hoistStatic": true,
                    "scopeId": "data-v-x"
                }
            }),
        )
        .expect("ssr compile");

        let code = compiled["code"].as_str().unwrap_or("");
        assert!(!code.contains("with (_ctx)"));
        assert!(code.contains("_ssrInterpolate(_ctx.msg)"));
        assert!(code.contains("_ssrRenderAttrs(_attrs)"));
        assert!(code.contains("data-v-x"));
        assert!(!code.contains("_hoisted_"));
        assert!(!code.contains("_cache["));
    }

    #[test]
    fn vue3_ssr_bridge_ignores_scope_id_for_explicit_function_mode() {
        let compiled = dispatch(
            "vue3.ssr.compile",
            json!({
                "source": "<div class=\"a\"></div>",
                "options": {
                    "mode": "function",
                    "scopeId": "data-v-ignored"
                }
            }),
        )
        .expect("ssr compile");

        let code = compiled["code"].as_str().unwrap_or("");
        assert!(!code.contains("data-v-ignored"));
        assert!(code.contains("_ssrRenderAttrs(_mergeProps("));
    }

    #[test]
    fn vue3_dom_bridge_uses_dom_namespace_defaults() {
        let parsed = dispatch(
            "vue3.dom.parse",
            json!({ "source": "<svg><rect/></svg><math><ms>1</ms></math>", "options": {} }),
        )
        .expect("dom parse");

        assert_eq!(parsed["children"][0]["ns"], json!(1));
        assert_eq!(parsed["children"][0]["children"][0]["ns"], json!(1));
        assert_eq!(parsed["children"][1]["ns"], json!(2));
        assert_eq!(parsed["children"][1]["children"][0]["ns"], json!(2));
    }

    #[test]
    fn vue3_dom_bridge_sfc_inner_loc_ends_at_closing_tag_start() {
        let parsed = dispatch(
            "vue3.dom.parse",
            json!({
                "source": "<template>\n<div></div>\n</template>",
                "options": {
                    "parseMode": "sfc"
                }
            }),
        )
        .expect("dom parse");

        let template = &parsed["children"][0];
        assert_eq!(template["innerLoc"]["source"], json!("\n<div></div>\n"));
        assert_eq!(template["innerLoc"]["start"]["offset"], json!(10));
        assert_eq!(template["innerLoc"]["end"]["offset"], json!(23));
    }

    #[test]
    fn vue3_dom_bridge_sfc_inner_loc_offsets_are_utf16() {
        let source = r#"<script>import { "😏" as foo } from './foo'</script><script setup>import { "😏" as foo } from './foo'</script>"#;
        let parsed = dispatch(
            "vue3.dom.parse",
            json!({
                "source": source,
                "options": {
                    "parseMode": "sfc"
                }
            }),
        )
        .expect("dom parse");

        let script = &parsed["children"][0];
        let script_setup = &parsed["children"][1];
        assert_eq!(
            script["innerLoc"]["source"],
            json!(r#"import { "😏" as foo } from './foo'"#)
        );
        assert_eq!(script["innerLoc"]["start"]["offset"], json!(8));
        assert_eq!(script["innerLoc"]["end"]["offset"], json!(43));
        assert_eq!(
            script_setup["innerLoc"]["source"],
            json!(r#"import { "😏" as foo } from './foo'"#)
        );
        assert_eq!(script_setup["innerLoc"]["start"]["offset"], json!(66));
        assert_eq!(script_setup["innerLoc"]["end"]["offset"], json!(101));
    }

    #[test]
    fn vue3_core_bridge_projects_public_utils() {
        let position = dispatch(
            "vue3.core.advancePositionWithClone",
            json!({
                "pos": { "offset": 0, "line": 1, "column": 1 },
                "source": "foo\nbar",
                "numberOfCharacters": 4,
            }),
        )
        .expect("position projection");
        assert_eq!(position, json!({ "offset": 4, "line": 2, "column": 1 }));

        let asset = dispatch(
            "vue3.core.toValidAssetId",
            json!({
                "name": "test-测试-1",
                "type": "component",
            }),
        )
        .expect("asset id projection");
        assert_eq!(asset["id"], json!("_component_test_2797935797_1"));
    }

    #[test]
    fn vue3_dom_bridge_sfc_plain_template_lang_keeps_raw_text() {
        let parsed = dispatch(
            "vue3.dom.parse",
            json!({
                "source": "<template lang=\"pug\">p(v-if=\"1 < 2\") test <div/></template>",
                "options": {
                    "parseMode": "sfc"
                }
            }),
        )
        .expect("dom parse");

        let template = &parsed["children"][0];
        assert_eq!(template["children"].as_array().unwrap().len(), 1);
        assert_eq!(
            template["children"][0]["content"],
            json!("p(v-if=\"1 < 2\") test <div/>")
        );
        assert!(parsed["__vuecDiagnostics"].as_array().unwrap().is_empty());
    }

    #[test]
    fn vue3_dom_bridge_sfc_parse_uses_dom_void_tag_defaults() {
        let parsed = dispatch(
            "vue3.dom.parse",
            json!({
                "source": "<template><input></template>",
                "options": {
                    "parseMode": "sfc"
                }
            }),
        )
        .expect("dom parse");

        let input = &parsed["children"][0]["children"][0];
        assert_eq!(input["tag"], json!("input"));
        assert_eq!(input["children"].as_array().unwrap().len(), 0);
        assert!(parsed["__vuecDiagnostics"].as_array().unwrap().is_empty());
    }

    #[test]
    fn vue3_dom_bridge_sfc_custom_blocks_are_raw_text() {
        let parsed = dispatch(
            "vue3.dom.parse",
            json!({
                "source": "<template><input></template><foo> <-& </foo>",
                "options": {
                    "parseMode": "sfc"
                }
            }),
        )
        .expect("dom parse");

        let custom_block = &parsed["children"][1];
        assert_eq!(custom_block["tag"], json!("foo"));
        assert_eq!(custom_block["children"].as_array().unwrap().len(), 1);
        assert_eq!(custom_block["children"][0]["content"], json!(" <-& "));
        assert!(parsed["__vuecDiagnostics"].as_array().unwrap().is_empty());
    }

    #[test]
    fn vue3_dom_bridge_sfc_parse_classifies_non_native_tags_as_components() {
        let parsed = dispatch(
            "vue3.dom.parse",
            json!({
                "source": "<template><hello/></template>",
                "options": {
                    "parseMode": "sfc"
                }
            }),
        )
        .expect("dom parse");

        let hello = &parsed["children"][0]["children"][0];
        assert_eq!(hello["tag"], json!("hello"));
        assert_eq!(hello["tagType"], json!(1));

        let custom = dispatch(
            "vue3.dom.parse",
            json!({
                "source": "<template><hello/></template>",
                "options": {
                    "parseMode": "sfc",
                    "__vuecCustomElements": ["hello"]
                }
            }),
        )
        .expect("dom parse");
        assert_eq!(custom["children"][0]["children"][0]["tagType"], json!(0));
    }

    #[test]
    fn vue3_dom_bridge_respects_explicit_empty_dom_parser_predicates() {
        let parsed = dispatch(
            "vue3.dom.parse",
            json!({
                "source": "<input><hello/>",
                "options": {
                    "__vuecVoidTags": [],
                    "__vuecNativeTags": []
                }
            }),
        )
        .expect("dom parse");

        assert_eq!(parsed["children"][0]["children"][0]["tag"], json!("hello"));
        assert!(parsed["__vuecDiagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| diagnostic["code"] == json!(24)));
    }

    #[test]
    fn vue3_dom_bridge_parses_asset_url_options() {
        let compiled = dispatch(
            "vue3.dom.compile",
            json!({
                "source": r#"<img src="./bar.png"><img src="~bar.png">"#,
                "options": {
                    "transformAssetUrls": {
                        "base": "/foo"
                    }
                }
            }),
        )
        .expect("dom compile");

        let code = compiled["code"].as_str().unwrap_or("");
        assert!(code.contains(r#"src: "/foo/bar.png""#));
        assert!(code.contains(r#"src: "~bar.png""#));

        let disabled = dispatch(
            "vue3.dom.compile",
            json!({
                "source": r#"<img src="./bar.png">"#,
                "options": {
                    "transformAssetUrls": false
                }
            }),
        )
        .expect("dom compile");

        assert!(disabled["code"]
            .as_str()
            .unwrap_or("")
            .contains(r#"src: "./bar.png""#));
    }

    #[test]
    fn vue27_bridge_compile_template_transforms_asset_urls() {
        let compiled = dispatch(
            "sfc.vue27.compileTemplate",
            json!({
                "source": r#"<div><img src="./logo.png" srcset="./logo.png 2x"><svg><use href="~@svg/file.svg#fragment"/></svg></div>"#,
                "options": {
                    "transformAssetUrls": {
                        "use": "href"
                    }
                }
            }),
        )
        .expect("vue27 sfc compileTemplate");

        let code = compiled["code"].as_str().unwrap_or("");
        assert!(code.contains(r#"src: require("./logo.png")"#));
        assert!(code.contains(r#"srcset: require("./logo.png") + " 2x""#));
        assert!(code.contains(r##"href: require("@svg/file.svg") + "#fragment""##));
    }

    #[test]
    fn vue27_bridge_compile_template_asset_options_support_base_and_absolute_urls() {
        let compiled = dispatch(
            "sfc.vue27.compileTemplate",
            json!({
                "source": r#"<div><img src="./logo.png"><img src="/logo.png"><img src="@/logo.png"></div>"#,
                "options": {
                    "transformAssetUrls": true,
                    "transformAssetUrlsOptions": {
                        "base": "/base/",
                        "includeAbsolute": true
                    }
                }
            }),
        )
        .expect("vue27 sfc compileTemplate");

        let code = compiled["code"].as_str().unwrap_or("");
        assert!(code.contains(r#""src":"/base/logo.png""#));
        assert!(code.contains(r#""src":require("/logo.png")"#));
        assert!(code.contains(r#""src":require("@/logo.png")"#));
    }

    #[test]
    fn vue27_bridge_compile_template_preprocesses_pug_and_reports_missing_lang() {
        let compiled = dispatch(
            "sfc.vue27.compileTemplate",
            json!({
                "source": "body\n h1 Pug Examples\n div.container\n   p Cool Pug example!\n",
                "filename": "example.vue",
                "options": {
                    "preprocessLang": "pug"
                }
            }),
        )
        .expect("vue27 sfc compileTemplate");

        assert!(compiled["errors"].as_array().unwrap().is_empty());
        let code = compiled["code"].as_str().unwrap_or("");
        assert!(code.contains("_c('body'"));
        assert!(code.contains("staticClass:\"container\""));

        let missing = dispatch(
            "sfc.vue27.compileTemplate",
            json!({
                "source": "",
                "filename": "example.vue",
                "options": {
                    "preprocessLang": "unknownLang"
                }
            }),
        )
        .expect("vue27 sfc compileTemplate");

        assert_eq!(missing["errors"].as_array().unwrap().len(), 1);
        assert_eq!(missing["tips"].as_array().unwrap().len(), 1);
        assert!(missing["errors"][0]
            .as_str()
            .unwrap_or("")
            .contains("unknownLang"));
        assert_eq!(
            missing["code"],
            json!("var render = function () {}\nvar staticRenderFns = []\n")
        );
    }

    #[test]
    fn vue3_dom_bridge_projects_asset_url_imports() {
        let compiled = dispatch(
            "vue3.dom.compile",
            json!({
                "source": r#"<img src="./bar.png" srcset="./bar.png 2x">"#,
                "options": {
                    "mode": "module"
                }
            }),
        )
        .expect("dom compile");

        let code = compiled["code"].as_str().unwrap_or("");
        assert!(code.contains("import _imports_0 from './bar.png'"));
        assert!(code.contains("src: _imports_0"));
        assert!(code.contains("srcset: _imports_0 + ' 2x'"));
        assert!(!code.contains("_ctx._imports_"));

        let parsed = dispatch(
            "vue3.dom.parse",
            json!({
                "source": r#"<img src="./bar.png">"#,
                "options": {
                    "mode": "module"
                }
            }),
        )
        .expect("dom parse");

        assert_eq!(parsed["imports"], json!([]));
    }

    #[test]
    fn vue3_dom_bridge_stringifies_static_children_from_sentinel_option() {
        let compiled = dispatch(
            "vue3.dom.compile",
            json!({
                "source": format!("<div>{}</div>", r#"<span class="foo"/>"#.repeat(5)),
                "options": {
                    "prefixIdentifiers": true,
                    "hoistStatic": true,
                    "__vuecStringifyStatic": true
                }
            }),
        )
        .expect("dom compile");

        let code = compiled["code"].as_str().unwrap_or("");
        assert!(code.contains("createStaticVNode"));
        assert!(code.contains("_createStaticVNode(\"<span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span><span class=\\\"foo\\\"></span>\", 5)"));
    }

    #[test]
    fn vue3_ssr_bridge_projects_asset_url_imports() {
        let compiled = dispatch(
            "vue3.ssr.compile",
            json!({
                "source": r#"<img src="./bar.png" srcset="./bar.png 2x">"#,
                "options": {
                    "mode": "module",
                    "prefixIdentifiers": true
                }
            }),
        )
        .expect("ssr compile");

        let code = compiled["code"].as_str().unwrap_or("");
        assert!(code.contains("import _imports_0 from './bar.png'"));
        assert!(code.contains("src: _imports_0"));
        assert!(code.contains("srcset: _imports_0 + ' 2x'"));
        assert!(code.contains("_ssrRenderAttrs(_mergeProps("));
        assert!(code.contains("_attrs"));
        assert!(!code.contains("_ctx._imports_"));

        let disabled = dispatch(
            "vue3.ssr.compile",
            json!({
                "source": r#"<img src="./bar.png">"#,
                "options": {
                    "mode": "module",
                    "prefixIdentifiers": true,
                    "transformAssetUrls": false
                }
            }),
        )
        .expect("ssr compile");

        let disabled_code = disabled["code"].as_str().unwrap_or("");
        assert!(!disabled_code.contains("import _imports_0"));
        assert!(disabled_code.contains(r#"src: "./bar.png""#));
        assert!(disabled_code.contains("_attrs"));
    }

    #[test]
    fn vue3_ssr_bridge_uses_dom_parser_defaults_for_components() {
        let compiled = dispatch(
            "vue3.ssr.compile",
            json!({
                "source": r#"<router-link><img src="./logo.png"></router-link>"#,
                "options": {
                    "mode": "module",
                    "prefixIdentifiers": true
                }
            }),
        )
        .expect("ssr compile");

        let code = compiled["code"].as_str().unwrap_or("");
        assert!(code.contains("resolveComponent as _resolveComponent"));
        assert!(code.contains("const _component_router_link = _resolveComponent(\"router-link\")"));
        assert!(code.contains("_push(_ssrRenderComponent(_component_router_link, _attrs, {"));
        assert!(code.contains("_createVNode(\"img\", { src: _imports_0 })"));
    }
}
