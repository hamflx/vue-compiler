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
    span.source()
        .map(|span| vue3_source_span_value(source, base_offset, span))
        .unwrap_or_else(vue3_loc_stub_value)
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

fn vue3_source_span_value(source: &str, base_offset: usize, span: Span) -> Value {
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

fn vue3_signed_position(source: &str, offset: isize) -> Value {
    if offset >= 0 {
        return vue3_position(source, offset as usize);
    }
    let _ = source;
    json!({
        "offset": offset,
        "line": 1,
        "column": 1isize + offset,
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
    options.source_map = bool_option(
        value,
        "sourceMap",
        bool_option(value, "source_map", options.source_map),
    );
    options.transform_asset_urls =
        transform_asset_urls_enabled_with_compiler_fallback(value, options.transform_asset_urls);
    options.asset_url_options =
        asset_url_options_with_compiler_fallback(value, options.asset_url_options);
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
    options.gen_default_as = value
        .get("genDefaultAs")
        .or_else(|| value.get("gen_default_as"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    options.hoist_static = bool_option(
        value,
        "hoistStatic",
        bool_option(value, "hoist_static", options.hoist_static),
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
    options.script_ast_mode = sfc_script_ast_mode_option(value, options.script_ast_mode);
    options.allow_deprecated_import_assert_syntax = deprecated_import_assert_syntax_option(value);
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
            "vue27Legacy" | "vue27_legacy" | "legacy" => SfcCssVarNameStyle::Vue27Legacy,
            _ => SfcCssVarNameStyle::Vue3Escaped,
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

fn sfc_script_ast_mode_option(value: &Value, fallback: SfcScriptAstMode) -> SfcScriptAstMode {
    value
        .get("__vuecScriptAstMode")
        .or_else(|| value.get("scriptAstMode"))
        .or_else(|| value.get("script_ast_mode"))
        .and_then(Value::as_str)
        .and_then(SfcScriptAstMode::from_option_str)
        .unwrap_or(fallback)
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
/// Returns the native binding API manifest as JSON.
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
                "callVue2Bridge",
                "rewriteDefaultVue27",
                "rewriteDefaultVue3",
                "baseCompileVue3",
                "baseParseVue3",
                "generateVue3Core",
                "callVue3CoreProjection",
                "callVue3DomProjection",
                "compileVue3Dom",
                "parseVue3Dom",
                "compileVue3Ssr",
                "parseSfc",
                "parseSfcResult",
                "parseVue27SfcComponent",
                "compileSfcTemplate",
                "compileSfcTemplateSource",
                "compileSfcScript",
                "compileVue27SfcTemplate",
                "compileVue27SfcScript",
                "compileSfcStyle"
            ]
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_manifest_lists_public_package_exports() {
        let manifest: Value = serde_json::from_str(&api_manifest().unwrap()).unwrap();
        let exports = manifest["exports"].as_array().unwrap();

        assert!(exports.contains(&json!("parseSfc")));
        assert!(exports.contains(&json!("parseSfcResult")));
        assert!(exports.contains(&json!("parseVue27SfcComponent")));
    }

    #[test]
    fn vue3_options_accepts_public_keys() {
        let options = vue3_options(Some(&json!({
            "mode": "module",
            "prefixIdentifiers": true,
            "sourceMap": true,
            "ssrCssVars": "{ \"--x\": (foo) }",
            "scopeId": "data-v-test"
        })))
        .expect("valid Vue 3 options");
        assert_eq!(options.mode, "module");
        assert!(options.prefix_identifiers);
        assert!(options.source_map);
        assert_eq!(options.ssr_css_vars.as_deref(), Some("{ \"--x\": (foo) }"));
        assert_eq!(options.scope_id.as_deref(), Some("data-v-test"));
    }

    #[test]
    fn vue3_options_validate_compiler_mode() {
        for mode in ["function", "module"] {
            let options = vue3_options(Some(&json!({ "mode": mode })))
                .expect("supported Vue 3 compiler mode");
            assert_eq!(options.mode, mode);
        }

        for mode in [json!("invalid"), json!(42)] {
            let error = vue3_options(Some(&json!({ "mode": mode })))
                .expect_err("unsupported Vue 3 compiler mode");
            assert_eq!(error.status, Status::InvalidArg);
            assert!(error.reason.contains("expected \"function\" or \"module\""));
        }
    }

    #[test]
    fn vue3_options_accepts_napi_predicate_projection_keys() {
        let options = vue3_options(Some(&json!({
            "__vuecVoidTags": ["img"],
            "__vuecNativeTags": ["div"],
            "__vuecCustomElements": ["x-thing"],
            "__vuecBuiltInComponents": ["Transition"],
            "parseMode": "sfc"
        })))
        .expect("valid Vue 3 options");
        assert_eq!(options.void_tags, vec!["img"]);
        assert_eq!(options.native_tags, Some(vec!["div".into()]));
        assert_eq!(options.custom_elements, vec!["x-thing"]);
        assert_eq!(options.built_in_components, vec!["Transition"]);
        assert!(options.sfc_parse_mode);
        assert_eq!(options.sfc_plain_template_langs, vec!["pug", "jade"]);
    }

    #[test]
    fn vue3_dom_options_accept_asset_url_projection_keys() {
        let default_options = DomCompilerOptions::default();
        let options = json!({
            "transformAssetUrls": {
                "base": "/cdn",
                "includeAbsolute": true,
                "tags": {
                    "foo": ["bar"],
                    "picture": "srcset"
                }
            }
        });

        assert!(transform_asset_urls_enabled(
            &options,
            default_options.transform_asset_urls
        ));
        let parsed = asset_url_options(&options, default_options.asset_url_options);
        assert_eq!(parsed.base.as_deref(), Some("/cdn"));
        assert!(parsed.include_absolute);
        assert_eq!(parsed.tags.get("foo"), Some(&vec!["bar".to_string()]));
        assert_eq!(
            parsed.tags.get("picture"),
            Some(&vec!["srcset".to_string()])
        );
    }

    #[test]
    fn vue3_sfc_template_options_accept_asset_url_projection_keys() {
        let options = sfc_template_options(Some(&json!({
            "sourceMap": false,
            "transformAssetUrls": {
                "foo": ["bar"]
            }
        })));

        assert!(!options.source_map);
        assert!(options.transform_asset_urls);
        assert_eq!(
            options.asset_url_options.tags.get("foo"),
            Some(&vec!["bar".to_string()])
        );
    }

    #[test]
    fn vue3_sfc_template_options_use_compiler_asset_fallback() {
        let options = sfc_template_options(Some(&json!({
            "compilerOptions": {
                "transformAssetUrls": false
            }
        })));

        assert!(!options.transform_asset_urls);
    }

    #[test]
    fn vue3_sfc_script_options_accept_inline_template_ssr() {
        let options = sfc_script_options(Some(&json!({
            "id": "xxxxxxxx",
            "inlineTemplate": true,
            "source_map": false,
            "propsDestructure": "error",
            "globalTypeFiles": ["global.d.ts"],
            "genDefaultAs": "_sfc_",
            "babelParserPlugins": [
                ["importAttributes", { "deprecatedAssertSyntax": true }]
            ],
            "templateOptions": {
                "ssr": true
            }
        })));

        assert_eq!(options.id.as_deref(), Some("xxxxxxxx"));
        assert!(options.inline_template);
        assert!(options.inline_template_ssr);
        assert!(!options.source_map);
        assert_eq!(options.props_destructure, SfcPropsDestructureMode::Error);
        assert_eq!(options.global_type_files, vec!["global.d.ts"]);
        assert_eq!(options.gen_default_as.as_deref(), Some("_sfc_"));
        assert!(options.allow_deprecated_import_assert_syntax);

        let disabled = sfc_script_options(Some(&json!({
            "props_destructure": false,
            "global_type_files": ["ambient.d.ts"],
            "gen_default_as": "script"
        })));
        assert_eq!(
            disabled.props_destructure,
            SfcPropsDestructureMode::Disabled
        );
        assert_eq!(disabled.global_type_files, vec!["ambient.d.ts"]);
        assert_eq!(disabled.gen_default_as.as_deref(), Some("script"));
    }

    #[test]
    fn vue3_sfc_script_options_accept_internal_ast_mode() {
        assert_eq!(
            sfc_script_options(Some(&json!({
                "__vuecScriptAstMode": "none"
            })))
            .script_ast_mode,
            SfcScriptAstMode::None
        );
        assert_eq!(
            sfc_script_options(Some(&json!({
                "scriptAstMode": "topLevel"
            })))
            .script_ast_mode,
            SfcScriptAstMode::TopLevel
        );
        assert_eq!(
            sfc_script_options(Some(&json!({
                "script_ast_mode": "unknown"
            })))
            .script_ast_mode,
            SfcScriptAstMode::Full
        );
    }

    #[test]
    fn full_sfc_compile_bindings_preserve_descriptor_parse_errors() {
        let source = "<template><div/></template><template><span/></template><script>const one = 1</script><script>const two = 2</script><style>.a{}</style>";

        let template = serde_json::to_value(compile_sfc_template_result(
            source,
            "Duplicate.vue".into(),
            SfcTemplateCompileOptions {
                source_map: false,
                ..SfcTemplateCompileOptions::default()
            },
        ))
        .unwrap();
        assert_eq!(
            template["errors"][0]["message"],
            json!("Single file component can contain only one <template> element")
        );
        assert_eq!(template["errors"][0]["code"], json!(0));
        assert!(template["map"].is_null());

        let script = serde_json::to_value(compile_sfc_script_result(
            source,
            "Duplicate.vue".into(),
            SfcScriptCompileOptions::default(),
        ))
        .unwrap();
        assert_eq!(
            script["errors"][1],
            json!("Single file component can contain only one <script> element")
        );

        let style = serde_json::to_value(compile_sfc_style_result(
            source,
            "Duplicate.vue".into(),
            SfcStyleCompileOptions::default(),
        ))
        .unwrap();
        assert_eq!(
            style["errors"][0],
            json!("Single file component can contain only one <template> element")
        );
        assert_eq!(style["diagnostics"][0]["code"], json!("VUEC_SFC_PARSE"));
    }

    #[test]
    fn vue3_sfc_parse_result_attaches_template_ast_to_descriptor() {
        let mut compiler = SfcCompiler::new();
        let result = compiler.parse_vue3(
            "Functional.vue",
            r#"<template functional="x"><div/></template>"#,
        );
        let mut value = vuec_sfc::vue3_sfc_parse_result_value(
            &result,
            &Vue3SfcParseProjectionOptions::default(),
        );
        if let Some(descriptor_value) = value.get_mut("descriptor") {
            vue3_sfc_attach_template_ast(descriptor_value, &result.descriptor, &Value::Null)
                .expect("valid template options");
        }

        assert_eq!(
            value["descriptor"]["template"]["ast"]["children"][0]["tag"],
            json!("div")
        );
        assert_eq!(
            value["errors"][0]["loc"]["source"],
            json!("functional=\"x\"")
        );

        let src_result = compiler.parse_vue3("Src.vue", "<template src></template>");
        let mut src_value = vuec_sfc::vue3_sfc_parse_result_value(
            &src_result,
            &Vue3SfcParseProjectionOptions::default(),
        );
        if let Some(descriptor_value) = src_value.get_mut("descriptor") {
            vue3_sfc_attach_template_ast(descriptor_value, &src_result.descriptor, &Value::Null)
                .expect("valid template options");
        }
        assert!(src_value["descriptor"]["template"].get("ast").is_none());
    }

    #[test]
    fn vue3_dom_options_ignore_compiler_asset_fallback() {
        let fallback = DomCompilerOptions::default().transform_asset_urls;
        let options = json!({
            "compilerOptions": {
                "transformAssetUrls": false
            }
        });

        assert_eq!(transform_asset_urls_enabled(&options, fallback), fallback);
    }

    #[test]
    fn vue3_parse_diagnostics_include_public_messages() {
        let source = "<div>";
        let options = Vue3CompilerOptions::default();
        let template = template_source(source, &Value::Null);
        let ast = Vue3Dialect::base_parse(template, &options);
        let diagnostics = vue3_parse_diagnostics(&ast, source, 0, &options);
        assert_eq!(diagnostics[0]["code"], json!(24));
        assert_eq!(
            diagnostics[0]["message"],
            json!("Element is missing end tag.")
        );
    }

    #[test]
    fn vue3_parse_projects_interpolation_expression_ast() {
        let options = vue3_prefix_identifier_options();
        let ast = vue3_public_parse_for_test("{{ a + b }}", &options);

        assert_eq!(
            ast["children"][0]["content"]["ast"]["type"],
            json!("BinaryExpression")
        );
    }

    #[test]
    fn vue3_parse_projects_directive_expression_ast() {
        let options = vue3_prefix_identifier_options();
        let ast =
            vue3_public_parse_for_test(r#"<div :[key+1]="foo()" @click="a++;b++" />"#, &options);

        let props = &ast["children"][0]["props"];
        assert_eq!(props[0]["arg"]["ast"]["type"], json!("BinaryExpression"));
        assert_eq!(props[0]["exp"]["ast"]["type"], json!("CallExpression"));
        assert_eq!(props[1]["exp"]["ast"]["type"], json!("Program"));
        assert_eq!(
            props[1]["exp"]["ast"]["body"][0]["type"],
            json!("ExpressionStatement")
        );
        assert_eq!(
            props[1]["exp"]["ast"]["body"][1]["type"],
            json!("ExpressionStatement")
        );
    }

    #[test]
    fn vue3_parse_projects_slot_params_ast() {
        let options = vue3_prefix_identifier_options();
        let ast = vue3_public_parse_for_test(r#"<Comp #foo="{ a, b }" />"#, &options);

        assert_eq!(
            ast["children"][0]["props"][0]["exp"]["ast"]["type"],
            json!("ArrowFunctionExpression")
        );
    }

    #[test]
    fn vue3_parse_projects_v_for_parse_result_ast() {
        let options = vue3_prefix_identifier_options();
        let ast = vue3_public_parse_for_test(
            r#"<div v-for="({ a, b }, key, index) of a.b" />"#,
            &options,
        );

        let result = &ast["children"][0]["props"][0]["forParseResult"];
        assert_eq!(result["source"]["ast"]["type"], json!("MemberExpression"));
        assert_eq!(
            result["value"]["ast"]["type"],
            json!("ArrowFunctionExpression")
        );
        assert!(result["key"]["ast"].is_null());
        assert!(result["index"]["ast"].is_null());
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

    #[test]
    fn vue27_template_vue2_options_reads_compiler_options_and_public_ranges() {
        let options = vue27_template_vue2_options(json!({
            "compilerOptions": {
                "comments": true,
                "outputSourceRange": false,
                "preserveWhitespace": false
            },
            "outputSourceRange": true,
            "bindings": {
                "Foo": "setup-const",
                "__isScriptSetup": true
            }
        }));

        assert!(options.comments);
        assert!(options.output_source_range);
        assert!(!options.preserve_whitespace);
        assert_eq!(
            options.bindings.get("Foo").map(String::as_str),
            Some("setup-const")
        );
        assert!(options.bindings_is_script_setup);
    }

    #[test]
    fn vue2_ranged_tips_use_public_message_range_shape() {
        let tips = vec![Vue2Warning {
            msg: "component lists rendered with v-for should have explicit keys.".into(),
            start: Some(24),
            end: Some(46),
            tip: true,
        }];

        let ranged = vue2_tips_value(&tips, true);
        assert_eq!(
            ranged,
            json!([{
                "msg": "component lists rendered with v-for should have explicit keys.",
                "start": 24,
                "end": 46
            }])
        );

        let plain = vue2_tips_value(&tips, false);
        assert_eq!(
            plain,
            json!(["component lists rendered with v-for should have explicit keys."])
        );
    }

    #[test]
    fn vue27_sfc_template_code_wraps_vue2_render_shape() {
        let code = SfcCompiler::new().vue27_sfc_template_code(
            "with(this){return _c('div',[_v(_s(msg))])}",
            &[],
            Vue27PrefixIdentifiersOptions::default(),
            false,
        );
        assert!(code.contains("var _vm=this"));
        assert!(code.contains("return _c('div',[_vm._v(_vm._s(_vm.msg))])"));
        assert!(code.contains("render._withStripped = true"));
    }

    fn vue3_prefix_identifier_options() -> Vue3CompilerOptions {
        Vue3CompilerOptions {
            prefix_identifiers: true,
            ..Vue3CompilerOptions::default()
        }
    }

    fn vue3_public_parse_for_test(source: &str, options: &Vue3CompilerOptions) -> Value {
        let template = template_source(source, &Value::Null);
        let ast = Vue3Dialect::base_parse(template.clone(), options);
        vue3_public_parse_ast(&ast, &template.source, template.base_offset, options)
    }
}
