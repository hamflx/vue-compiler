fn from_js_options(env: &Env, options: Option<Unknown>) -> Result<Value> {
    let Some(value) = options else {
        return Ok(Value::Null);
    };

    JsToJson::new(env).convert(value)
}

const JS_TO_JSON_MAX_DEPTH: usize = 512;
const JS_TO_JSON_MAX_NODES: usize = 100_000;
const JS_TO_JSON_MAX_ANCESTOR_COMPARISONS: usize = 1_000_000;

struct JsToJson<'env> {
    env: &'env Env,
    nodes: usize,
    ancestor_comparisons: usize,
}

impl<'env> JsToJson<'env> {
    fn new(env: &'env Env) -> Self {
        Self {
            env,
            nodes: 0,
            ancestor_comparisons: 0,
        }
    }

    fn convert<'value>(&mut self, value: Unknown<'value>) -> Result<Value> {
        self.convert_at(value, 0, &mut Vec::new())
    }

    fn convert_at<'value>(
        &mut self,
        value: Unknown<'value>,
        depth: usize,
        ancestors: &mut Vec<Unknown<'value>>,
    ) -> Result<Value> {
        self.enter_node(depth)?;

        let value_type = value.get_type()?;
        match value_type {
            ValueType::Null => Ok(Value::Null),
            ValueType::Boolean => bool::from_unknown(value).map(Value::Bool),
            ValueType::Number => self.convert_number(f64::from_unknown(value)?),
            ValueType::String => String::from_unknown(value).map(Value::String),
            ValueType::Object => self.convert_object(value, depth, ancestors),
            ValueType::Undefined => Err(napi::Error::new(
                Status::InvalidArg,
                "undefined cannot be represented as a serde_json::Value".to_owned(),
            )),
            ValueType::External | ValueType::Function | ValueType::Symbol => {
                Err(napi::Error::new(
                    Status::InvalidArg,
                    format!("typeof {value_type:?} value could not be deserialized"),
                ))
            }
            ValueType::Unknown => Err(js_to_json_error(
                "encountered an unsupported JavaScript value type",
            )),
        }
    }

    fn enter_node(&mut self, depth: usize) -> Result<()> {
        if depth > JS_TO_JSON_MAX_DEPTH {
            return Err(js_to_json_error(format!(
                "maximum nesting depth of {JS_TO_JSON_MAX_DEPTH} exceeded"
            )));
        }
        if self.nodes >= JS_TO_JSON_MAX_NODES {
            return Err(js_to_json_error(format!(
                "maximum node count of {JS_TO_JSON_MAX_NODES} exceeded"
            )));
        }
        self.nodes += 1;
        Ok(())
    }

    fn convert_number(&self, number: f64) -> Result<Value> {
        let number = if number.trunc() == number {
            if (0.0..=u32::MAX as f64).contains(&number) {
                Some(serde_json::Number::from(number as u32))
            } else if number < 0.0 && number >= i32::MIN as f64 {
                Some(serde_json::Number::from(number as i32))
            } else {
                serde_json::Number::from_f64(number)
            }
        } else {
            serde_json::Number::from_f64(number)
        };
        number
            .map(Value::Number)
            .ok_or_else(|| {
                napi::Error::new(
                    Status::InvalidArg,
                    "Failed to convert js number to serde_json::Number".to_owned(),
                )
            })
    }

    fn convert_object<'value>(
        &mut self,
        value: Unknown<'value>,
        depth: usize,
        ancestors: &mut Vec<Unknown<'value>>,
    ) -> Result<Value> {
        self.reject_cycle(value, ancestors)?;
        ancestors.push(value);

        let converted = (|| {
            let object = Object::from_unknown(value)?;
            if object.is_array()? {
                self.convert_array(&object, depth, ancestors)
            } else {
                self.convert_map(&object, depth, ancestors)
            }
        })();

        ancestors.pop();
        converted
    }

    fn reject_cycle<'value>(
        &mut self,
        value: Unknown<'value>,
        ancestors: &[Unknown<'value>],
    ) -> Result<()> {
        self.ancestor_comparisons = self
            .ancestor_comparisons
            .checked_add(ancestors.len())
            .ok_or_else(|| js_to_json_error("inspection work limit exceeded"))?;
        if self.ancestor_comparisons > JS_TO_JSON_MAX_ANCESTOR_COMPARISONS {
            return Err(js_to_json_error(format!(
                "maximum inspection work of {JS_TO_JSON_MAX_ANCESTOR_COMPARISONS} comparisons exceeded"
            )));
        }

        for ancestor in ancestors.iter().rev() {
            if self.env.strict_equals(*ancestor, value)? {
                return Err(js_to_json_error("circular reference detected"));
            }
        }
        Ok(())
    }

    fn convert_array<'value>(
        &mut self,
        array: &Object<'value>,
        depth: usize,
        ancestors: &mut Vec<Unknown<'value>>,
    ) -> Result<Value> {
        let len = array.get_array_length()? as usize;
        self.check_container_len(len)?;

        let mut converted = Vec::with_capacity(len);
        for index in 0..len {
            let child = array.get_element::<Unknown>(index as u32)?;
            converted.push(self.convert_at(child, depth + 1, ancestors)?);
        }
        Ok(Value::Array(converted))
    }

    fn convert_map<'value>(
        &mut self,
        object: &Object<'value>,
        depth: usize,
        ancestors: &mut Vec<Unknown<'value>>,
    ) -> Result<Value> {
        let property_names = object.get_property_names()?;
        let len = property_names.get_array_length()? as usize;
        self.check_container_len(len)?;

        let mut converted = Map::new();
        for index in 0..len {
            let key = property_names.get_element::<String>(index as u32)?;
            if let Some(child) = object.get::<Unknown>(&key)? {
                converted.insert(key, self.convert_at(child, depth + 1, ancestors)?);
            }
        }
        Ok(Value::Object(converted))
    }

    fn check_container_len(&self, len: usize) -> Result<()> {
        if len > JS_TO_JSON_MAX_NODES.saturating_sub(self.nodes) {
            return Err(js_to_json_error(format!(
                "maximum node count of {JS_TO_JSON_MAX_NODES} exceeded"
            )));
        }
        Ok(())
    }
}

fn js_to_json_error(reason: impl Into<String>) -> napi::Error {
    napi::Error::new(
        Status::InvalidArg,
        format!(
            "JavaScript value could not be deserialized safely: {}",
            reason.into()
        ),
    )
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
