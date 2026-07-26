pub(crate) fn vue27_emits_type_argument_is_supported(
    type_argument: &TSType<'_>,
    analysis: &Vue27ScriptSetupAnalysis,
) -> bool {
    match type_argument {
        TSType::TSFunctionType(_) | TSType::TSTypeLiteral(_) => true,
        TSType::TSTypeReference(reference) => vue27_ts_type_name_identifier(&reference.type_name)
            .is_some_and(|name| analysis.emits_type_declarations.contains_key(name)),
        _ => false,
    }
}

pub(crate) fn vue27_resolve_props_type<'a>(
    source: &str,
    type_argument: &'a TSType<'a>,
    analysis: &Vue27ScriptSetupAnalysis,
) -> Option<Vue27TypeMembers> {
    match type_argument {
        TSType::TSTypeLiteral(literal) => {
            Some(vue27_type_members_from_literal(source, literal, analysis))
        }
        TSType::TSTypeReference(reference) => {
            let name = vue27_ts_type_name_identifier(&reference.type_name)?;
            analysis.props_type_declarations.get(name).cloned()
        }
        _ => None,
    }
}

pub(crate) fn vue27_resolve_emits_type<'a>(
    source: &str,
    type_argument: &'a TSType<'a>,
    analysis: &Vue27ScriptSetupAnalysis,
) -> Option<Vue27EmitsType> {
    match type_argument {
        TSType::TSFunctionType(function) => Some(vue27_emits_type_from_function(source, function)),
        TSType::TSTypeLiteral(literal) => Some(vue27_emits_type_from_literal(source, literal)),
        TSType::TSTypeReference(reference) => {
            let name = vue27_ts_type_name_identifier(&reference.type_name)?;
            analysis.emits_type_declarations.get(name).cloned()
        }
        _ => None,
    }
}

pub(crate) fn vue27_type_members_from_literal(
    source: &str,
    literal: &TSTypeLiteral<'_>,
    analysis: &Vue27ScriptSetupAnalysis,
) -> Vue27TypeMembers {
    Vue27TypeMembers {
        source: source
            .get(literal.span.start as usize..literal.span.end as usize)
            .unwrap_or_default()
            .to_string(),
        members: vue27_runtime_props_from_signatures(source, &literal.members, analysis),
        errors: Vec::new(),
        interface_heritage: None,
    }
}

pub(crate) fn vue27_type_members_from_interface_body(
    source: &str,
    body: &TSInterfaceBody<'_>,
    analysis: &Vue27ScriptSetupAnalysis,
) -> Vue27TypeMembers {
    Vue27TypeMembers {
        source: source
            .get(body.span.start as usize..body.span.end as usize)
            .unwrap_or_default()
            .to_string(),
        members: vue27_runtime_props_from_signatures(source, &body.body, analysis),
        errors: Vec::new(),
        interface_heritage: None,
    }
}

pub(crate) fn vue27_runtime_props_from_signatures(
    source: &str,
    signatures: &[TSSignature<'_>],
    analysis: &Vue27ScriptSetupAnalysis,
) -> Vec<Vue27RuntimeProp> {
    let mut props = Vec::new();
    for signature in signatures {
        match signature {
            TSSignature::TSPropertySignature(property) if !property.computed => {
                if let Some(key) = vue27_property_key_static_name(&property.key) {
                    let types = property
                        .type_annotation
                        .as_ref()
                        .map(|annotation| {
                            infer_vue27_runtime_type(&annotation.type_annotation, analysis)
                        })
                        .unwrap_or_else(|| vec!["null".into()]);
                    props.push(Vue27RuntimeProp {
                        key,
                        types,
                        required: !property.optional,
                        default: None,
                        is_method: false,
                        type_annotation_source: property.type_annotation.as_ref().and_then(
                            |annotation| {
                                source
                                    .get(
                                        annotation.span.start as usize
                                            ..annotation.span.end as usize,
                                    )
                                    .map(ToOwned::to_owned)
                            },
                        ),
                        member_source: source
                            .get(property.span.start as usize..property.span.end as usize)
                            .map(ToOwned::to_owned),
                    });
                }
            }
            TSSignature::TSMethodSignature(method) if !method.computed => {
                if let Some(key) = vue27_property_key_static_name(&method.key) {
                    props.push(Vue27RuntimeProp {
                        key,
                        types: vec!["Function".into()],
                        required: !method.optional,
                        default: None,
                        is_method: true,
                        type_annotation_source: method.return_type.as_ref().and_then(
                            |annotation| {
                                source
                                    .get(
                                        annotation.span.start as usize
                                            ..annotation.span.end as usize,
                                    )
                                    .map(ToOwned::to_owned)
                            },
                        ),
                        member_source: source
                            .get(method.span.start as usize..method.span.end as usize)
                            .map(ToOwned::to_owned),
                    });
                }
            }
            _ => {}
        }
    }
    props
}

pub(crate) fn infer_vue27_runtime_type(
    node: &TSType<'_>,
    analysis: &Vue27ScriptSetupAnalysis,
) -> Vec<String> {
    match node {
        TSType::TSStringKeyword(_) => vec!["String".into()],
        TSType::TSNumberKeyword(_) => vec!["Number".into()],
        TSType::TSBooleanKeyword(_) => vec!["Boolean".into()],
        TSType::TSObjectKeyword(_) | TSType::TSTypeLiteral(_) | TSType::TSIntersectionType(_) => {
            vec!["Object".into()]
        }
        TSType::TSFunctionType(_) => vec!["Function".into()],
        TSType::TSArrayType(_) | TSType::TSTupleType(_) => vec!["Array".into()],
        TSType::TSSymbolKeyword(_) => vec!["Symbol".into()],
        TSType::TSLiteralType(literal) => match &literal.literal {
            TSLiteral::StringLiteral(_) => vec!["String".into()],
            TSLiteral::BooleanLiteral(_) => vec!["Boolean".into()],
            TSLiteral::NumericLiteral(_) | TSLiteral::BigIntLiteral(_) => vec!["Number".into()],
            _ => vec!["null".into()],
        },
        TSType::TSTypeReference(reference) => {
            if let Some(name) = vue27_ts_type_name_identifier(&reference.type_name) {
                if let Some(types) = analysis.declared_types.get(name) {
                    return types.clone();
                }
                match name {
                    "Array" | "Function" | "Object" | "Set" | "Map" | "WeakSet" | "WeakMap"
                    | "Date" | "Promise" => return vec![name.to_string()],
                    "Record" | "Partial" | "Readonly" | "Pick" | "Omit" | "Exclude" | "Extract"
                    | "Required" | "InstanceType" => return vec!["Object".into()],
                    _ => {}
                }
            }
            vec!["null".into()]
        }
        TSType::TSParenthesizedType(parenthesized) => {
            infer_vue27_runtime_type(&parenthesized.type_annotation, analysis)
        }
        TSType::TSUnionType(union) => {
            let mut types = Vec::new();
            for ty in &union.types {
                for runtime_type in infer_vue27_runtime_type(ty, analysis) {
                    push_unique(&mut types, &runtime_type);
                }
            }
            types
        }
        _ => vec!["null".into()],
    }
}

pub(crate) fn vue27_runtime_defaults_from_argument(
    source: &str,
    argument: &Argument<'_>,
) -> Option<Vue27RuntimeDefaults> {
    let expression = argument.to_expression();
    let source_text = source
        .get(expression.span().start as usize..expression.span().end as usize)?
        .to_string();
    let Expression::ObjectExpression(object) = expression else {
        return Some(Vue27RuntimeDefaults {
            source: source_text,
            static_defaults: None,
        });
    };
    let mut defaults = BTreeMap::new();
    for property in &object.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            return Some(Vue27RuntimeDefaults {
                source: source_text,
                static_defaults: None,
            });
        };
        if property.computed {
            return Some(Vue27RuntimeDefaults {
                source: source_text,
                static_defaults: None,
            });
        }
        if let Some(key) = vue27_property_key_static_name(&property.key) {
            let default_source = if property.method {
                vue27_function_body_source(source, &property.value)
                    .map(|body| format!("default() {body}"))
            } else {
                source
                    .get(property.value.span().start as usize..property.value.span().end as usize)
                    .map(|value| format!("default: {value}"))
            };
            if let Some(default_source) = default_source {
                defaults.insert(key, default_source);
            }
        }
    }
    Some(Vue27RuntimeDefaults {
        source: source_text,
        static_defaults: Some(defaults),
    })
}

pub(crate) fn vue27_function_body_source<'a>(
    source: &'a str,
    expression: &Expression<'_>,
) -> Option<&'a str> {
    match expression {
        Expression::FunctionExpression(function) => function
            .body
            .as_ref()
            .and_then(|body| source.get(body.span.start as usize..body.span.end as usize)),
        _ => source.get(expression.span().start as usize..expression.span().end as usize),
    }
}

pub(crate) fn vue3_runtime_defaults_from_argument(
    source: &str,
    argument: &Argument<'_>,
) -> Option<Vue27RuntimeDefaults> {
    let expression = argument.to_expression();
    let source_text = source
        .get(expression.span().start as usize..expression.span().end as usize)?
        .to_string();
    let Expression::ObjectExpression(object) = expression else {
        return Some(Vue27RuntimeDefaults {
            source: source_text,
            static_defaults: None,
        });
    };
    let mut defaults = BTreeMap::new();
    for property in &object.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            return Some(Vue27RuntimeDefaults {
                source: source_text,
                static_defaults: None,
            });
        };
        let Some(key) = vue3_runtime_default_property_key(&property.key, property.computed) else {
            return Some(Vue27RuntimeDefaults {
                source: source_text,
                static_defaults: None,
            });
        };
        let default_source = if property.method || property.kind != PropertyKind::Init {
            vue3_runtime_default_method_source(source, property)
        } else {
            source
                .get(property.value.span().start as usize..property.value.span().end as usize)
                .map(|value| format!("default: {value}"))
        };
        if let Some(default_source) = default_source {
            defaults.insert(key, default_source);
        }
    }
    Some(Vue27RuntimeDefaults {
        source: source_text,
        static_defaults: Some(defaults),
    })
}

pub(crate) fn vue3_runtime_default_property_key(
    key: &PropertyKey<'_>,
    computed: bool,
) -> Option<String> {
    if !computed {
        return vue27_property_key_static_name(key);
    }
    match key {
        PropertyKey::StringLiteral(literal) => Some(literal.value.to_string()),
        PropertyKey::NumericLiteral(literal) => Some(literal.value.to_string()),
        PropertyKey::TemplateLiteral(template) if template.expressions.is_empty() => {
            let mut key = String::new();
            for quasi in &template.quasis {
                key.push_str(&vue3_template_value(quasi));
            }
            Some(key)
        }
        _ => None,
    }
}

pub(crate) fn vue3_runtime_default_method_source(
    source: &str,
    property: &ObjectProperty<'_>,
) -> Option<String> {
    let Expression::FunctionExpression(function) = &property.value else {
        return source
            .get(property.value.span().start as usize..property.value.span().end as usize)
            .map(|value| format!("default: {value}"));
    };
    let body = function
        .body
        .as_ref()
        .and_then(|body| source.get(body.span.start as usize..body.span.end as usize))?;
    match property.kind {
        PropertyKind::Get => Some(format!("get default() {body}")),
        PropertyKind::Set => {
            let params = vue3_function_params_source(source, function)?;
            Some(format!("set default{params} {body}"))
        }
        PropertyKind::Init => {
            let params = vue3_function_params_source(source, function)?;
            let async_prefix = if function.r#async { "async " } else { "" };
            let generator_prefix = if function.generator { "*" } else { "" };
            Some(format!(
                "{async_prefix}{generator_prefix}default{params} {body}"
            ))
        }
    }
}

pub(crate) fn vue3_function_params_source<'a>(
    source: &'a str,
    function: &Function<'_>,
) -> Option<&'a str> {
    source.get(function.params.span.start as usize..function.params.span.end as usize)
}

pub(crate) fn vue27_setup_props_type_source(
    source: &str,
    type_argument: &TSType<'_>,
    type_members: &Vue27TypeMembers,
    defaults: Option<&Vue27RuntimeDefaults>,
) -> String {
    let Some(defaults) = defaults.and_then(|defaults| defaults.static_defaults.as_ref()) else {
        if !type_members.source.is_empty() {
            return type_members.source.clone();
        }
        return source
            .get(type_argument.span().start as usize..type_argument.span().end as usize)
            .unwrap_or_default()
            .to_string();
    };
    let mut parts = Vec::new();
    for prop in &type_members.members {
        if defaults.contains_key(&prop.key) {
            if let Some(type_annotation) = &prop.type_annotation_source {
                parts.push(format!(
                    "{}{}{}",
                    prop.key,
                    if prop.is_method { "()" } else { "" },
                    type_annotation
                ));
            }
        } else if let Some(member_source) = vue27_prop_member_type_source(prop) {
            parts.push(member_source);
        }
    }
    format!("{{ {} }}", parts.join(", "))
}

pub(crate) fn vue27_prop_member_type_source(prop: &Vue27RuntimeProp) -> Option<String> {
    let member_source = prop.member_source.as_deref()?;
    let type_annotation = prop.type_annotation_source.as_deref()?;
    let end = member_source.find(type_annotation)? + type_annotation.len();
    Some(member_source[..end].trim().to_string())
}

pub(crate) fn gen_vue27_runtime_props(
    props: &[Vue27RuntimeProp],
    defaults: Option<&Vue27RuntimeDefaults>,
    is_prod: bool,
) -> String {
    let mut entries = Vec::new();
    for prop in props {
        let type_string = vue27_runtime_type_string(&prop.types);
        if !is_prod {
            entries.push(format!(
                "{}: {{ type: {}, required: {}{} }}",
                prop.key,
                type_string,
                prop.required,
                prop.default
                    .as_ref()
                    .map(|default| format!(", {default}"))
                    .unwrap_or_default()
            ));
        } else if prop
            .types
            .iter()
            .any(|ty| ty == "Boolean" || (prop.default.is_some() && ty == "Function"))
        {
            entries.push(format!(
                "{}: {{ type: {}{} }}",
                prop.key,
                type_string,
                prop.default
                    .as_ref()
                    .map(|default| format!(", {default}"))
                    .unwrap_or_default()
            ));
        } else {
            entries.push(format!(
                "{}: {}",
                prop.key,
                prop.default
                    .as_ref()
                    .map(|default| format!("{{ {default} }}"))
                    .unwrap_or_else(|| "null".into())
            ));
        }
    }
    let props_decl = format!("{{\n    {}\n  }}", entries.join(",\n    "));
    if let Some(defaults) = defaults {
        if defaults.static_defaults.is_none() {
            return format!("_mergeDefaults({props_decl}, {})", defaults.source);
        }
    }
    props_decl
}

pub(crate) fn vue27_runtime_type_string(types: &[String]) -> String {
    if types.len() > 1 {
        format!("[{}]", types.join(", "))
    } else {
        types.first().cloned().unwrap_or_else(|| "null".into())
    }
}

pub(crate) fn vue27_props_type_assignment(
    source: &str,
    binding: &BindingPattern<'_>,
    type_source: Option<&str>,
) -> String {
    let binding_source = source
        .get(binding.span().start as usize..binding.span().end as usize)
        .unwrap_or("props")
        .trim();
    let cast = type_source
        .filter(|value| !value.is_empty())
        .map(|value| format!(" as {value}"))
        .unwrap_or_default();
    format!("\nconst {binding_source} = __props{cast};\n")
}
