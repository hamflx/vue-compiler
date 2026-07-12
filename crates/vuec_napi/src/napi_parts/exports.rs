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

#[napi(js_name = "rewriteDefaultVue3")]
/// Rewrites a Vue 3 default export to an assigned variable.
pub fn rewrite_default_vue3(
    env: Env,
    source: String,
    variable: String,
    parser_plugins: Option<Unknown>,
) -> Result<String> {
    let plugin_options = vue3_rewrite_default_options(from_js_options(&env, parser_plugins)?);
    let compiler = SfcCompiler::new();
    compiler
        .rewrite_vue3_default(&source, &variable, plugin_options)
        .map_err(napi::Error::from_reason)
}

#[napi(js_name = "compileVue3Dom")]
/// Compiles a Vue 3 template for DOM rendering and returns a JSON string result.
pub fn compile_vue3_dom(env: Env, source: String, options: Option<Unknown>) -> Result<String> {
    let raw_options = from_js_options(&env, options)?;
    let template = template_source(&source, &raw_options);
    let mut core = vue3_options(Some(&raw_options))?;
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
    let mut core = vue3_options(Some(&raw_options))?;
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
    let options = vue3_options(Some(&raw_options))?;
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
    let raw_options = from_js_options(&env, options)?;
    let options = vue3_options(Some(&raw_options))?;
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
        "vue3.core.stringifyStatic" => vuec_vue3_core::stringify_static_projection(&payload),
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
    let mut core = vue3_options(Some(&raw_options))?;
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
    };
    to_json_string(compile_ssr(template, ssr_options))
}

#[napi(js_name = "parseSfc")]
/// Parses a Vue SFC descriptor and returns it as JSON.
pub fn parse_sfc(env: Env, source: String, options: Option<Unknown>) -> Result<String> {
    let raw_options = from_js_options(&env, options)?;
    let filename = string_option(&raw_options, "filename", "anonymous.vue");
    let mut compiler = SfcCompiler::new();
    let parse_options = vue3_sfc_parse_options(&raw_options);
    let result = compiler.parse_vue3_with_options(filename, &source, parse_options.clone());
    let projection_options = vue3_sfc_parse_projection_options(&raw_options, &parse_options);
    let mut value = vuec_sfc::vue3_sfc_descriptor_value(&result.descriptor, &projection_options);
    vue3_sfc_attach_template_ast(&mut value, &result.descriptor, &raw_options)?;
    to_json_string(value)
}

#[napi(js_name = "parseSfcResult")]
/// Parses a Vue 3 SFC and returns the official public parse-result shape.
pub fn parse_sfc_result(env: Env, source: String, options: Option<Unknown>) -> Result<String> {
    let raw_options = from_js_options(&env, options)?;
    let filename = string_option(&raw_options, "filename", "anonymous.vue");
    let mut compiler = SfcCompiler::new();
    let parse_options = vue3_sfc_parse_options(&raw_options);
    let result = compiler.parse_vue3_with_options(filename, &source, parse_options.clone());
    let projection_options = vue3_sfc_parse_projection_options(&raw_options, &parse_options);
    let mut value = vuec_sfc::vue3_sfc_parse_result_value(&result, &projection_options);
    if let Some(descriptor_value) = value.get_mut("descriptor") {
        vue3_sfc_attach_template_ast(descriptor_value, &result.descriptor, &raw_options)?;
    }
    to_json_string(value)
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
    let result = compile_sfc_template_result(
        &source,
        filename,
        sfc_template_options(Some(&raw_options)),
    );
    to_json_string(result)
}

fn compile_sfc_template_result(
    source: &str,
    filename: String,
    options: SfcTemplateCompileOptions,
) -> vuec_sfc::SfcTemplateCompileResult {
    let mut compiler = SfcCompiler::new();
    let parsed = compiler.parse_vue3(filename, source);
    compiler.compile_parsed_vue3_template(&parsed, options)
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
    let result = compile_sfc_script_result(
        &source,
        filename,
        sfc_script_options(Some(&raw_options)),
    );
    to_json_string(result)
}

fn compile_sfc_script_result(
    source: &str,
    filename: String,
    options: SfcScriptCompileOptions,
) -> vuec_sfc::SfcScriptBlock {
    let mut compiler = SfcCompiler::new();
    let parsed = compiler.parse_vue3(filename, source);
    compiler.compile_parsed_vue3_script(&parsed, options)
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
    let compile_options = vue27_template_vue2_options(raw_options.clone());
    let output_source_range = compile_options.output_source_range;
    let compiled = vuec_vue2::compile(&preprocessed.source, compile_options);
    to_json_string(json!({
        "ast": null,
        "code": compiler.vue27_sfc_template_code(
            &compiled.render,
            &compiled.static_render_fns,
            vue27_prefix_identifiers_options(&raw_options),
            vue27_template_is_production(&raw_options),
        ),
        "source": source,
        "tips": vue2_tips_value(&compiled.tips, output_source_range),
        "errors": vue2_errors_value(&compiled.errors, output_source_range),
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
    let result = compile_sfc_style_result(
        &source,
        filename,
        sfc_style_options(Some(&raw_options)),
    );
    to_json_string(result)
}

fn compile_sfc_style_result(
    source: &str,
    filename: String,
    options: SfcStyleCompileOptions,
) -> vuec_sfc::SfcStyleCompileResult {
    let mut compiler = SfcCompiler::new();
    let parsed = compiler.parse_vue3(filename, source);
    compiler.compile_parsed_vue3_style(&parsed, options)
}
