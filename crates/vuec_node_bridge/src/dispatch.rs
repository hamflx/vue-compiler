use crate::*;

pub(crate) fn dispatch(command: &str, payload: Value) -> Result<Value> {
    bridge_command(command).with_context(|| {
        format!("bridge command `{command}` is missing from vuec_bridge_registry")
    })?;
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
        "sfc.templateUtils.isRelativeUrl" => {
            let url = string_field(&payload, "url");
            Ok(json!(vuec_vue3_asset::is_relative_url(&url)))
        }
        "sfc.templateUtils.isExternalUrl" => {
            let url = string_field(&payload, "url");
            Ok(json!(vuec_vue3_asset::is_external_url(&url)))
        }
        "sfc.templateUtils.isDataUrl" => {
            let url = string_field(&payload, "url");
            Ok(json!(vuec_vue3_asset::is_data_url(&url)))
        }
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
        "vue3.core.transformOnSuite" => Ok(vue3_core_transform_on_suite_value(&payload)),
        "vue3.core.transformForSuite" => Ok(vue3_core_transform_for_suite_value(&payload)),
        "vue3.core.transformModelSuite" => Ok(vue3_core_transform_model_suite_value(&payload)),
        "vue3.core.transformBindSuite" => Ok(vue3_core_transform_bind_suite_value(&payload)),
        "vue3.core.transformOnceSuite" => Ok(vue3_core_transform_once_suite_value(&payload)),
        "vue3.core.transformIfSuite" => Ok(vue3_core_transform_if_suite_value(&payload)),
        "vue3.core.transformSlotSuite" => Ok(vue3_core_transform_slot_suite_value(&payload)),
        "vue3.core.transformElementSuite" => Ok(vue3_core_transform_element_suite_value(&payload)),
        "vue3.core.transformSuite" => Ok(vue3_core_transform_suite_value(&payload)),
        "vue3.core.cacheStaticSuite" => Ok(vue3_core_cache_static_suite_value(&payload)),
        "vue3.core.transformSlotOutletSuite" => {
            Ok(vue3_core_transform_slot_outlet_suite_value(&payload))
        }
        "vue3.core.transformExpressionSuite" => {
            Ok(vue3_core_transform_expression_suite_value(&payload))
        }
        "vue3.core.transformTextSuite" => Ok(vue3_core_transform_text_suite_value(&payload)),
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
            let options = vue27_parse_component_options(payload.get("options"));
            let output_source_range = options.output_source_range;
            let result = compiler.parse_vue27_component_with_filename(filename, &source, options);
            Ok(vue27_parse_component_value(
                &result.descriptor,
                &result.errors,
                output_source_range,
            ))
        }
        "sfc.vue27.parseComponent" => {
            let source = string_field(&payload, "source");
            let mut compiler = SfcCompiler::new();
            let options = vue27_parse_component_options(payload.get("options"));
            let output_source_range = options.output_source_range;
            let result = compiler.parse_vue27_component(&source, options);
            Ok(vue27_parse_component_value(
                &result.descriptor,
                &result.errors,
                output_source_range,
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
        "sfc.rewriteDefault" => {
            let source = string_field(&payload, "source");
            let variable = string_field_or(&payload, "variable", "script");
            let compiler = SfcCompiler::new();
            let rewritten = compiler
                .rewrite_vue3_default(
                    &source,
                    &variable,
                    vue3_rewrite_default_options(payload.get("plugins")),
                )
                .map_err(anyhow::Error::msg)?;
            Ok(json!(rewritten))
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
            let template_options_value = payload
                .get("bridgeOptions")
                .or_else(|| payload.get("options"));
            let options = sfc_template_options(template_options_value);
            let preprocessed = compiler.preprocess_vue3_template(
                &source,
                vue3_template_preprocess_options(payload.get("options"), &filename),
            );
            if !preprocessed.errors.is_empty() || !preprocessed.tips.is_empty() {
                return Ok(json!({
                    "ast": {},
                    "code": "export default function render() {}",
                    "source": source,
                    "tips": preprocessed.tips,
                    "errors": preprocessed.errors,
                }));
            }
            let compile_source = if payload
                .get("options")
                .and_then(|options| options.get("preprocessLang"))
                .is_some()
            {
                &preprocessed.source
            } else {
                &source
            };
            if payload.get("ast").is_some() {
                return Ok(vue3_sfc_compile_template_value(
                    &payload,
                    &filename,
                    compile_source,
                    &source,
                    &options,
                ));
            }
            Ok(serde_json::to_value(compiler.compile_template_source(
                filename,
                compile_source,
                options,
            ))?)
        }
        "sfc.vue27.compileTemplate" => {
            let source = string_field(&payload, "source");
            let filename = string_field_or(&payload, "filename", "template.vue.html");
            let raw_options = payload.get("options").unwrap_or(&Value::Null);
            let options = vue27_sfc_template_vue2_options(payload.get("options"));
            let output_source_range = options.output_source_range;
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
            let tips = vue2_tips_value(&compiled.tips, output_source_range);
            let errors = vue2_errors_value(&compiled.errors, output_source_range);
            if !compiled.errors.is_empty() {
                return Ok(json!({
                    "ast": vue27_template_ast_value(&compiled),
                    "code": "var render = function () {}\nvar staticRenderFns = []\n",
                    "source": source,
                    "tips": tips,
                    "errors": errors,
                }));
            }
            Ok(json!({
                "ast": vue27_template_ast_value(&compiled),
                "code": compiler.vue27_sfc_template_code(
                    &compiled.render,
                    &compiled.static_render_fns,
                    vue27_prefix_identifiers_options(raw_options),
                    vue27_template_is_production(raw_options),
                ),
                "source": source,
                "tips": tips,
                "errors": errors,
            }))
        }
        "sfc.compileScript" => {
            let source = string_field(&payload, "source");
            let filename = string_field_or(&payload, "filename", "anonymous.vue");
            let mut compiler = SfcCompiler::new();
            let descriptor = compiler.parse(filename, &source);
            let options = sfc_script_options(payload.get("options"));
            Ok(serde_json::to_value(
                compiler.compile_script(&descriptor, options),
            )?)
        }
        "sfc.resolveType" => {
            let code = string_field(&payload, "code");
            let filename = string_field_or(&payload, "filename", "anonymous.vue");
            let options = sfc_script_options(payload.get("options"));
            let mut compiler = SfcCompiler::new();
            Ok(serde_json::to_value(
                compiler.resolve_vue3_type(filename, &code, options),
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
            let source = payload
                .get("source")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let filename = string_field_or(&payload, "filename", "anonymous.vue");
            let options = sfc_style_options(payload.get("options"));
            let style = compile_style(
                source,
                StyleCompileOptions {
                    id: options.id.clone(),
                    scoped: options.scoped,
                    vars: options.vars.clone(),
                    is_prod: options.is_prod,
                    css_var_name_style: options.css_var_name_style,
                    css_var_ignore_line_comments: options.css_var_ignore_line_comments,
                    filename: Some(filename),
                    source_map_source: None,
                    source_map_file_id: Some(vuec_source::FileId(0)),
                    source_map_base_offset: 0,
                    source_map: options.source_map,
                    modules: options.modules,
                    modules_options: options.modules_options.clone(),
                    preprocess_lang: options.preprocess_lang,
                    preprocess_options: options.preprocess_options,
                    warn_deprecated_scoped_selectors: options.warn_deprecated_scoped_selectors,
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
        "sfc.vue27.compileStyle" | "sfc.vue27.compileStyleAsync" => {
            let source = payload
                .get("source")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let filename = string_field_or(&payload, "filename", "anonymous.vue");
            let mut options = sfc_style_options(payload.get("options"));
            options.scoped = payload
                .get("options")
                .and_then(|value| value.get("scoped"))
                .and_then(Value::as_bool)
                .unwrap_or(true);
            let style = compile_style(
                source,
                StyleCompileOptions {
                    id: options.id.clone(),
                    scoped: options.scoped,
                    vars: options.vars.clone(),
                    is_prod: options.is_prod,
                    css_var_name_style: CssVarNameStyle::Vue27Legacy,
                    css_var_ignore_line_comments: false,
                    filename: Some(filename),
                    source_map_source: None,
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
