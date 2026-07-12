fn vue27_template_vue2_options(value: Value) -> Vue2CompileOptions {
    let compiler_value = value
        .get("compilerOptions")
        .filter(|value| value.is_object())
        .cloned()
        .unwrap_or_else(|| value.clone());
    let mut options = vue2_options(compiler_value);
    options.output_source_range = bool_option(
        &value,
        "outputSourceRange",
        bool_option(&value, "output_source_range", options.output_source_range),
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
    if vue27_transform_asset_urls_enabled(&value, false) {
        options.sfc_asset_url_transform = Some(vue27_sfc_asset_url_options(&value));
    }
    options
}

fn vue27_transform_asset_urls_enabled(value: &Value, fallback: bool) -> bool {
    match value.get("transformAssetUrls") {
        Some(Value::Bool(enabled)) => *enabled,
        Some(Value::Object(_)) => true,
        _ => fallback,
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

fn transform_asset_urls_enabled(value: &Value, fallback: bool) -> bool {
    transform_asset_urls_enabled_from(transform_asset_urls_value(value), fallback)
}

fn transform_asset_urls_enabled_with_compiler_fallback(value: &Value, fallback: bool) -> bool {
    transform_asset_urls_enabled_from(
        transform_asset_urls_value_with_compiler_fallback(value),
        fallback,
    )
}

fn transform_asset_urls_enabled_from(value: Option<&Value>, fallback: bool) -> bool {
    match value {
        Some(Value::Bool(enabled)) => *enabled,
        Some(Value::Object(_)) => true,
        _ => fallback,
    }
}

fn asset_url_options(value: &Value, mut options: AssetUrlOptions) -> AssetUrlOptions {
    let Some(raw) = transform_asset_urls_value(value) else {
        return options;
    };
    parse_asset_url_options(raw, &mut options);
    options
}

fn asset_url_options_with_compiler_fallback(
    value: &Value,
    mut options: AssetUrlOptions,
) -> AssetUrlOptions {
    let Some(raw) = transform_asset_urls_value_with_compiler_fallback(value) else {
        return options;
    };
    parse_asset_url_options(raw, &mut options);
    options
}

fn parse_asset_url_options(raw: &Value, options: &mut AssetUrlOptions) {
    match raw {
        Value::Bool(_) => {}
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
            } else if !object.contains_key("base")
                && !object.contains_key("includeAbsolute")
                && !object.contains_key("tags")
                && object
                    .iter()
                    .any(|(_, value)| matches!(value, Value::String(_) | Value::Array(_)))
            {
                options.tags = asset_url_tags(object);
            }
        }
        _ => {}
    }
}

fn transform_asset_urls_value(value: &Value) -> Option<&Value> {
    value.get("transformAssetUrls")
}

fn transform_asset_urls_value_with_compiler_fallback(value: &Value) -> Option<&Value> {
    value.get("transformAssetUrls").or_else(|| {
        value
            .get("compilerOptions")
            .and_then(|compiler_options| compiler_options.get("transformAssetUrls"))
    })
}

fn asset_url_tags(object: &Map<String, Value>) -> BTreeMap<String, Vec<String>> {
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

fn vue27_parse_component_options(value: &Value) -> Vue27ParseComponentOptions {
    Vue27ParseComponentOptions {
        output_source_range: bool_option(value, "outputSourceRange", false),
        deindent: value.get("deindent").and_then(Value::as_bool),
        pad: match value.get("pad") {
            Some(Value::Bool(true)) => Vue27SfcPad::True,
            Some(Value::String(value)) if value == "line" => Vue27SfcPad::Line,
            Some(Value::String(value)) if value == "space" => Vue27SfcPad::Space,
            _ => Vue27SfcPad::False,
        },
    }
}

fn vue27_template_preprocess_options(
    value: &Value,
    filename: &str,
) -> Vue27TemplatePreprocessOptions {
    Vue27TemplatePreprocessOptions {
        lang: value
            .get("preprocessLang")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        filename: Some(filename.to_string()),
    }
}

fn vue27_prefix_identifiers_options(value: &Value) -> Vue27PrefixIdentifiersOptions {
    Vue27PrefixIdentifiersOptions {
        is_functional: bool_option(value, "isFunctional", false),
        is_ts: bool_option(value, "isTS", false),
        bindings: string_map_option(value, "bindings").unwrap_or_default(),
    }
}

fn vue27_template_is_production(value: &Value) -> bool {
    bool_option(
        value,
        "isProduction",
        bool_option(value, "isProd", bool_option(value, "is_prod", false)),
    )
}

fn is_simple_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first == '$' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch == '$' || ch.is_ascii_alphanumeric())
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
            .any(|plugin| parser_plugin_name(plugin) == Some("typescript")),
        decorators: plugins.iter().any(|plugin| {
            matches!(
                parser_plugin_name(plugin),
                Some("decorators" | "decorators-legacy" | "decoratorAutoAccessors")
            )
        }),
    }
}

fn vue3_rewrite_default_options(value: Value) -> Vue3RewriteDefaultOptions {
    let plugins = match &value {
        Value::Array(values) => values.as_slice(),
        Value::Null => &[],
        other => std::slice::from_ref(other),
    };
    Vue3RewriteDefaultOptions {
        typescript: plugins
            .iter()
            .any(|plugin| parser_plugin_name(plugin) == Some("typescript")),
    }
}

fn parser_plugin_name(value: &Value) -> Option<&str> {
    value.as_str().or_else(|| {
        value
            .as_array()
            .and_then(|items| items.first())
            .and_then(Value::as_str)
    })
}

fn deprecated_import_assert_syntax_option(value: &Value) -> bool {
    value
        .get("babelParserPlugins")
        .or_else(|| value.get("babel_parser_plugins"))
        .or_else(|| value.get("parserPlugins"))
        .or_else(|| value.get("parser_plugins"))
        .is_some_and(deprecated_import_assert_syntax_plugin)
}

fn deprecated_import_assert_syntax_plugin(value: &Value) -> bool {
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
    options.source_map_source = value
        .get("__vuecSourceMapSource")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    options.source_map_base_offset = value
        .get("__vuecSourceMapBaseOffset")
        .and_then(Value::as_u64)
        .unwrap_or_default() as usize;
    options.ssr_css_vars = value
        .get("ssrCssVars")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
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

fn vue3_sfc_parse_options(value: &Value) -> Vue3SfcParseOptions {
    let mut options = Vue3SfcParseOptions::default();
    options.ignore_empty = bool_option(value, "ignoreEmpty", options.ignore_empty);
    options.pad = vue3_sfc_pad_option(value.get("pad"));
    options
}

fn vue3_sfc_parse_projection_options(
    value: &Value,
    parse_options: &Vue3SfcParseOptions,
) -> Vue3SfcParseProjectionOptions {
    Vue3SfcParseProjectionOptions {
        pad: parse_options.pad.clone(),
        source_map: bool_option(value, "sourceMap", true),
        source_root: value
            .get("sourceRoot")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
    }
}

fn vue3_sfc_pad_option(value: Option<&Value>) -> Vue3SfcPad {
    match value {
        Some(Value::Bool(true)) => Vue3SfcPad::Line,
        Some(Value::String(value)) if value == "line" => Vue3SfcPad::Line,
        Some(Value::String(value)) if value == "space" => Vue3SfcPad::Space,
        _ => Vue3SfcPad::False,
    }
}
