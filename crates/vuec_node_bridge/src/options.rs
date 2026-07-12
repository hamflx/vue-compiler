use crate::*;

pub(crate) fn vue2_options(value: Option<&Value>) -> Vue2CompileOptions {
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

pub(crate) fn vue27_sfc_template_vue2_options(value: Option<&Value>) -> Vue2CompileOptions {
    let Some(value) = value else {
        return Vue2CompileOptions::default();
    };
    let compiler_value = value
        .get("compilerOptions")
        .or_else(|| value.get("compiler_options"))
        .filter(|value| value.is_object())
        .unwrap_or(value);
    let mut options = vue2_options(Some(compiler_value));
    options.output_source_range = bool_option(
        value,
        "outputSourceRange",
        bool_option(value, "output_source_range", options.output_source_range),
    );
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

pub(crate) fn vue27_template_preprocess_options(
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

pub(crate) fn vue3_template_preprocess_options(
    value: Option<&Value>,
    filename: &str,
) -> Vue3TemplatePreprocessOptions {
    let lang = value
        .and_then(|value| {
            value
                .get("preprocessLang")
                .or_else(|| value.get("preprocess_lang"))
        })
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    Vue3TemplatePreprocessOptions {
        lang,
        filename: Some(filename.to_string()),
    }
}

pub(crate) fn vue27_sfc_asset_url_options(value: &Value) -> Vue2SfcAssetUrlTransformOptions {
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

pub(crate) fn vue27_sfc_asset_url_tags(
    object: &Map<String, Value>,
) -> BTreeMap<String, Vec<String>> {
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

pub(crate) fn vue3_options(value: Option<&Value>) -> Vue3CompilerOptions {
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
    options.ssr_css_vars = vue3_ssr_css_vars_option(value);
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
            .filter_map(vue3_expression_plugin_name)
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
        options.sfc_plain_template_langs = vec!["pug".to_string(), "jade".to_string()];
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

pub(crate) fn vue3_expression_plugin_name(value: &Value) -> Option<&str> {
    value.as_str().or_else(|| {
        value
            .as_array()
            .and_then(|items| items.first())
            .and_then(Value::as_str)
    })
}

pub(crate) fn vue3_ssr_css_vars_option(value: &Value) -> Option<String> {
    match value.get("ssrCssVars") {
        Some(Value::String(source)) => Some(source.clone()),
        Some(Value::Array(items)) => {
            let vars = items.iter().filter_map(Value::as_str).collect::<Vec<_>>();
            if vars.is_empty() {
                return Some(String::new());
            }
            let id = value.get("id").and_then(Value::as_str).unwrap_or_default();
            let short_id = id.strip_prefix("data-v-").unwrap_or(id);
            let is_prod = bool_option(value, "isProd", bool_option(value, "is_prod", false));
            let entries = vars
                .iter()
                .map(|var| {
                    let name = format!(
                        ":--{}",
                        gen_css_var_name_with_style(
                            short_id,
                            var,
                            is_prod,
                            CssVarNameStyle::Vue3Escaped,
                        )
                    );
                    format!("{}: ({})", json!(name), var)
                })
                .collect::<Vec<_>>()
                .join(",\n  ");
            Some(format!("{{\n  {entries}\n}}"))
        }
        _ => None,
    }
}

pub(crate) fn vue3_namespace_option_value(value: &Value) -> Option<vuec_ast::HtmlNamespace> {
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

pub(crate) fn vue3_parse_mode_is_sfc(value: Option<&Value>) -> bool {
    value
        .and_then(|value| value.get("parseMode"))
        .and_then(Value::as_str)
        == Some("sfc")
}

pub(crate) fn string_array_option(value: &Value, name: &str) -> Vec<String> {
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

pub(crate) fn string_map_option(value: &Value, name: &str) -> Option<BTreeMap<String, String>> {
    value.get(name).and_then(Value::as_object).map(|object| {
        object
            .iter()
            .filter_map(|(key, value)| value.as_str().map(|value| (key.clone(), value.to_string())))
            .collect()
    })
}

pub(crate) fn transform_asset_urls_enabled(value: &Value, fallback: bool) -> bool {
    match value.get("transformAssetUrls") {
        Some(Value::Bool(enabled)) => *enabled,
        Some(Value::Object(_)) => true,
        _ => fallback,
    }
}

pub(crate) fn asset_url_options(value: &Value, mut options: AssetUrlOptions) -> AssetUrlOptions {
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

pub(crate) fn asset_url_tags(
    object: &serde_json::Map<String, Value>,
) -> BTreeMap<String, Vec<String>> {
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

pub(crate) fn sfc_template_options(value: Option<&Value>) -> SfcTemplateCompileOptions {
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
        transform_asset_urls_enabled(value, options.transform_asset_urls);
    options.asset_url_options = asset_url_options(value, options.asset_url_options);
    let explicit_scope_id = value
        .get("scopeId")
        .or_else(|| value.get("scope_id"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    options.scope_id = explicit_scope_id.or_else(|| {
        bool_option(value, "scoped", false).then(|| {
            let short_id = options
                .id
                .as_deref()
                .unwrap_or_default()
                .strip_prefix("data-v-")
                .unwrap_or_else(|| options.id.as_deref().unwrap_or_default());
            format!("data-v-{short_id}")
        })
    });
    options
}

pub(crate) fn sfc_script_options(value: Option<&Value>) -> SfcScriptCompileOptions {
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
    let nested_runtime_module_name = value
        .get("templateOptions")
        .or_else(|| value.get("template_options"))
        .and_then(|template_options| {
            template_options
                .get("compilerOptions")
                .or_else(|| template_options.get("compiler_options"))
        })
        .and_then(|compiler_options| {
            compiler_options
                .get("runtimeModuleName")
                .or_else(|| compiler_options.get("runtime_module_name"))
        })
        .and_then(Value::as_str);
    options.runtime_module_name = value
        .get("runtimeModuleName")
        .or_else(|| value.get("runtime_module_name"))
        .and_then(Value::as_str)
        .or(nested_runtime_module_name)
        .map(ToOwned::to_owned);
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
    options.custom_element = bool_option(
        value,
        "customElement",
        bool_option(
            value,
            "custom_element",
            bool_option(value, "__vuecCustomElement", options.custom_element),
        ),
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

pub(crate) fn sfc_style_options(value: Option<&Value>) -> SfcStyleCompileOptions {
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

pub(crate) fn vue27_parse_component_options(value: Option<&Value>) -> Vue27ParseComponentOptions {
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

pub(crate) fn vue27_rewrite_default_options(value: Option<&Value>) -> Vue27RewriteDefaultOptions {
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
            .any(|plugin| parser_plugin_name(plugin) == Some("typescript")),
        decorators: plugins.iter().any(|plugin| {
            matches!(
                parser_plugin_name(plugin),
                Some("decorators" | "decorators-legacy" | "decoratorAutoAccessors")
            )
        }),
    }
}

pub(crate) fn vue3_rewrite_default_options(value: Option<&Value>) -> Vue3RewriteDefaultOptions {
    let Some(value) = value else {
        return Vue3RewriteDefaultOptions::default();
    };
    let plugins = value
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_else(|| std::slice::from_ref(value));
    Vue3RewriteDefaultOptions {
        typescript: plugins
            .iter()
            .any(|plugin| parser_plugin_name(plugin) == Some("typescript")),
    }
}

pub(crate) fn parser_plugin_name(value: &Value) -> Option<&str> {
    value.as_str().or_else(|| {
        value
            .as_array()
            .and_then(|items| items.first())
            .and_then(Value::as_str)
    })
}

pub(crate) fn deprecated_import_assert_syntax_option(value: &Value) -> bool {
    value
        .get("babelParserPlugins")
        .or_else(|| value.get("babel_parser_plugins"))
        .or_else(|| value.get("parserPlugins"))
        .or_else(|| value.get("parser_plugins"))
        .is_some_and(deprecated_import_assert_syntax_plugin)
}

pub(crate) fn deprecated_import_assert_syntax_plugin(value: &Value) -> bool {
    let plugins = value
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_else(|| std::slice::from_ref(value));
    plugins.iter().any(|plugin| {
        parser_plugin_name(plugin) == Some("importAttributes")
            && plugin
                .as_array()
                .and_then(|items| items.get(1))
                .and_then(Value::as_object)
                .is_some_and(|options| {
                    options
                        .get("deprecatedAssertSyntax")
                        .or_else(|| options.get("deprecated_assert_syntax"))
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                })
    })
}

pub(crate) fn vue27_prefix_identifiers_options(value: &Value) -> Vue27PrefixIdentifiersOptions {
    Vue27PrefixIdentifiersOptions {
        is_functional: bool_option(value, "isFunctional", false),
        is_ts: bool_option(value, "isTS", false),
        bindings: json_string_map_option(value, "bindings").unwrap_or_default(),
    }
}

pub(crate) fn vue27_template_is_production(value: &Value) -> bool {
    bool_option(
        value,
        "isProduction",
        bool_option(value, "isProd", bool_option(value, "is_prod", false)),
    )
}

pub(crate) fn bool_option(value: &Value, name: &str, fallback: bool) -> bool {
    value.get(name).and_then(Value::as_bool).unwrap_or(fallback)
}

pub(crate) fn props_destructure_option(
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

pub(crate) fn sfc_script_ast_mode_option(
    value: &Value,
    fallback: SfcScriptAstMode,
) -> SfcScriptAstMode {
    value
        .get("__vuecScriptAstMode")
        .or_else(|| value.get("scriptAstMode"))
        .or_else(|| value.get("script_ast_mode"))
        .and_then(Value::as_str)
        .and_then(SfcScriptAstMode::from_option_str)
        .unwrap_or(fallback)
}

pub(crate) fn json_string_map_option(
    value: &Value,
    name: &str,
) -> Option<BTreeMap<String, String>> {
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
