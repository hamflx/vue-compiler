#[derive(Clone, Copy)]
pub(crate) enum Vue3StaticConditionalTypeOutcome {
    True,
    False,
}

pub(crate) fn vue3_static_conditional_type_outcome(
    check_type: &TSType<'_>,
    extends_type: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vue3StaticConditionalTypeOutcome> {
    let check_set = vue3_static_conditional_type_set(check_type, analysis)?;
    let extends_set = vue3_static_conditional_type_set(extends_type, analysis)?;
    if check_set
        .values
        .iter()
        .all(|value| extends_set.values.contains(value))
    {
        return Some(Vue3StaticConditionalTypeOutcome::True);
    }
    if !check_set.is_distributive
        && check_set
            .values
            .iter()
            .all(|value| !extends_set.values.contains(value))
    {
        return Some(Vue3StaticConditionalTypeOutcome::False);
    }
    None
}

pub(crate) struct Vue3StaticConditionalTypeSet {
    pub(crate) values: BTreeSet<String>,
    pub(crate) is_distributive: bool,
}

pub(crate) fn vue3_static_conditional_type_set(
    ty: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vue3StaticConditionalTypeSet> {
    match ty {
        TSType::TSLiteralType(literal) => Some(Vue3StaticConditionalTypeSet {
            values: vue3_static_conditional_literal_values(&literal.literal)?,
            is_distributive: false,
        }),
        TSType::TSTypeReference(reference) => {
            let name = vue3_ts_type_name_key(&reference.type_name)?;
            if let Some(keys) = analysis.ordered_string_literal_type_declarations.get(&name) {
                return Some(Vue3StaticConditionalTypeSet {
                    values: keys
                        .iter()
                        .map(|key| vue3_static_conditional_string_value(key))
                        .collect(),
                    is_distributive: false,
                });
            }
            if let Some(keys) = analysis.string_literal_type_declarations.get(&name) {
                return Some(Vue3StaticConditionalTypeSet {
                    values: keys
                        .iter()
                        .map(|key| vue3_static_conditional_string_value(key))
                        .collect(),
                    is_distributive: false,
                });
            }
            match name.as_str() {
                "Extract" | "Exclude" | "Uppercase" | "Lowercase" | "Capitalize"
                | "Uncapitalize" => Some(Vue3StaticConditionalTypeSet {
                    values: vue3_resolve_string_type_keys(ty, analysis)?
                        .into_iter()
                        .map(|key| vue3_static_conditional_string_value(&key))
                        .collect(),
                    is_distributive: false,
                }),
                _ => None,
            }
        }
        TSType::TSUnionType(union) => {
            let mut values = BTreeSet::new();
            for ty in &union.types {
                values.extend(vue3_static_conditional_type_set(ty, analysis)?.values);
            }
            Some(Vue3StaticConditionalTypeSet {
                values,
                is_distributive: true,
            })
        }
        TSType::TSParenthesizedType(parenthesized) => {
            vue3_static_conditional_type_set(&parenthesized.type_annotation, analysis)
        }
        TSType::TSTemplateLiteralType(template) => Some(Vue3StaticConditionalTypeSet {
            values: vue3_resolve_template_literal_type_keys(template, analysis)?
                .into_iter()
                .map(|key| vue3_static_conditional_string_value(&key))
                .collect(),
            is_distributive: false,
        }),
        TSType::TSImportType(import_type) => {
            let resolved = vue3_resolve_import_type(import_type, analysis)?;
            let values = resolved
                .context
                .ordered_string_literal_type_declarations
                .get(&resolved.name)
                .cloned()
                .or_else(|| {
                    resolved
                        .context
                        .string_literal_type_declarations
                        .get(&resolved.name)
                        .map(|keys| keys.iter().cloned().collect())
                })?;
            Some(Vue3StaticConditionalTypeSet {
                values: values
                    .into_iter()
                    .map(|key| vue3_static_conditional_string_value(&key))
                    .collect(),
                is_distributive: false,
            })
        }
        _ => None,
    }
}

pub(crate) fn vue3_static_conditional_literal_values(
    literal: &TSLiteral<'_>,
) -> Option<BTreeSet<String>> {
    match literal {
        TSLiteral::StringLiteral(literal) => {
            Some([vue3_static_conditional_string_value(literal.value.as_str())].into())
        }
        TSLiteral::BooleanLiteral(literal) => Some([format!("boolean:{}", literal.value)].into()),
        TSLiteral::NumericLiteral(literal) => Some([format!("number:{}", literal.value)].into()),
        TSLiteral::BigIntLiteral(literal) => Some([format!("bigint:{}", literal.value)].into()),
        _ => None,
    }
}

pub(crate) fn vue3_static_conditional_string_value(value: &str) -> String {
    format!("string:{value}")
}

pub(crate) fn infer_vue3_define_model_runtime_utility_type(
    name: &str,
    reference: &TSTypeReference<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vec<String>> {
    match name {
        "NonNullable" => {
            let ty = vue3_type_reference_type_argument(reference, 0)?;
            let mut types = infer_vue3_define_model_runtime_type(ty, analysis);
            types.retain(|ty| ty != "null");
            Some(types)
        }
        "Extract" => {
            let ty = vue3_type_reference_type_argument(reference, 1)?;
            Some(infer_vue3_define_model_runtime_type(ty, analysis))
        }
        "Exclude" => {
            let ty = vue3_type_reference_type_argument(reference, 0)?;
            Some(infer_vue3_define_model_runtime_type(ty, analysis))
        }
        "OmitThisParameter" => {
            let ty = vue3_type_reference_type_argument(reference, 0)?;
            Some(infer_vue3_define_model_runtime_type(ty, analysis))
        }
        "ReturnType" => {
            let ty = vue3_type_reference_type_argument(reference, 0)?;
            infer_vue3_return_runtime_type(ty, analysis, Vue3ArrayElementRuntimeMode::DefineModel)
        }
        "Uppercase" | "Lowercase" | "Capitalize" | "Uncapitalize" => Some(vec!["String".into()]),
        "Parameters" | "ConstructorParameters" | "ReadonlyArray" => Some(vec!["Array".into()]),
        "ReadonlyMap" => Some(vec!["Map".into()]),
        "ReadonlySet" => Some(vec!["Set".into()]),
        "Ref" | "ShallowRef" | "ComputedRef" | "WritableComputedRef" => Some(vec!["Object".into()]),
        "MaybeRef" | "MaybeRefOrGetter" => {
            let mut types = vec!["Object".to_string()];
            if name == "MaybeRefOrGetter" {
                push_unique(&mut types, "Function");
            }
            if let Some(ty) = vue3_type_reference_type_argument(reference, 0) {
                for runtime_type in infer_vue3_define_model_runtime_type(ty, analysis) {
                    push_unique(&mut types, &runtime_type);
                }
            }
            Some(types)
        }
        _ => None,
    }
}

pub(crate) fn gen_vue3_runtime_props(
    props: &[Vue27RuntimeProp],
    is_prod: bool,
    has_static_defaults: bool,
    custom_element: bool,
) -> String {
    let mut entries = Vec::new();
    for prop in props {
        let key = vue3_runtime_prop_key(&prop.key);
        let (types, skip_check) = vue3_runtime_prop_codegen_types(&prop.types);
        let type_string = vue27_runtime_type_string(&types);
        if !is_prod {
            let skip_check = if skip_check { ", skipCheck: true" } else { "" };
            entries.push(format!(
                "{key}: {{ type: {}, required: {}{}{} }}",
                type_string,
                prop.required,
                skip_check,
                prop.default
                    .as_ref()
                    .map(|default| format!(", {default}"))
                    .unwrap_or_default()
            ));
            continue;
        }
        let keep_prod_type = custom_element
            || types.iter().any(|ty| {
                ty == "Boolean"
                    || (ty == "Function" && (!has_static_defaults || prop.default.is_some()))
            });
        match (keep_prod_type, prop.default.as_ref()) {
            (true, Some(default)) => {
                if custom_element {
                    entries.push(format!("{key}: {{ {default}, type: {type_string} }}"));
                } else {
                    entries.push(format!("{key}: {{ type: {type_string}, {default} }}"));
                }
            }
            (true, None) => {
                if custom_element {
                    entries.push(format!("{key}: {{type: {type_string}}}"));
                } else {
                    entries.push(format!("{key}: {{ type: {type_string} }}"));
                }
            }
            (false, Some(default)) => {
                entries.push(format!("{key}: {{ {default} }}"));
            }
            (false, None) => {
                entries.push(format!("{key}: {{}}"));
            }
        }
    }
    format!("{{\n    {}\n  }}", entries.join(",\n    "))
}

pub(crate) fn vue3_runtime_prop_codegen_types(types: &[String]) -> (Vec<String>, bool) {
    let mut runtime_types = types.to_vec();
    let has_unknown = runtime_types.iter().any(|ty| ty == "Unknown");
    let has_boolean = runtime_types.iter().any(|ty| ty == "Boolean");
    let has_function = runtime_types.iter().any(|ty| ty == "Function");
    if has_unknown {
        if has_boolean || has_function {
            runtime_types.retain(|ty| ty != "Unknown");
            return (runtime_types, true);
        }
        runtime_types.clear();
        runtime_types.push("null".to_string());
    }
    (runtime_types, false)
}

pub(crate) fn vue3_runtime_prop_key(key: &str) -> String {
    if vue3_runtime_prop_key_needs_quote(key) {
        format!("\"{}\"", escape_js_double(key))
    } else {
        key.to_string()
    }
}

pub(crate) fn vue3_runtime_prop_key_needs_quote(key: &str) -> bool {
    key.chars().any(|ch| {
        matches!(
            ch,
            ' ' | '!'
                | '"'
                | '#'
                | '$'
                | '%'
                | '&'
                | '\''
                | '('
                | ')'
                | '*'
                | '+'
                | ','
                | '.'
                | '/'
                | ':'
                | ';'
                | '<'
                | '='
                | '>'
                | '?'
                | '@'
                | '['
                | '\\'
                | ']'
                | '^'
                | '`'
                | '{'
                | '|'
                | '}'
                | '~'
                | '-'
        )
    })
}
