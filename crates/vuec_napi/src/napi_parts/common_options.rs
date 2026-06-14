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
        base_offset: options
            .get("__vuecTemplateBaseOffset")
            .or_else(|| options.get("__vuecBaseOffset"))
            .and_then(Value::as_u64)
            .unwrap_or_default() as usize,
    }
}

fn apply_napi_dom_parser_defaults(core: &mut Vue3CompilerOptions, options: Option<&Value>) {
    let explicit_void_tags = napi_option_has(options, "__vuecVoidTags");
    let explicit_pre_tags = napi_option_has(options, "__vuecPreTags");
    let explicit_ignore_newline_tags = napi_option_has(options, "__vuecIgnoreNewlineTags");
    let explicit_native_tags = napi_option_has(options, "__vuecNativeTags");
    let void_tags = core.void_tags.clone();
    let pre_tags = core.pre_tags.clone();
    let ignore_newline_tags = core.ignore_newline_tags.clone();
    let native_tags = core.native_tags.clone();

    apply_dom_parser_defaults(core);

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

fn napi_option_has(options: Option<&Value>, name: &str) -> bool {
    options.is_some_and(|options| options.get(name).is_some())
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
