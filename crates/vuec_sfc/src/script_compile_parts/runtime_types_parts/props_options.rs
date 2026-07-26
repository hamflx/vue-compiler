pub(crate) fn vue3_props_options_type_members(
    source: &str,
    ty: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vue27TypeMembers> {
    match ty {
        TSType::TSTypeLiteral(literal) => Some(Vue27TypeMembers {
            source: source
                .get(literal.span.start as usize..literal.span.end as usize)
                .unwrap_or_default()
                .to_string(),
            members: vue3_runtime_props_options_from_signatures(source, &literal.members, analysis),
            errors: Vec::new(),
            interface_heritage: None,
        }),
        TSType::TSTypeReference(reference) => {
            let name = vue3_ts_type_name_key(&reference.type_name)?;
            analysis
                .props_options_type_declarations
                .get(&name)
                .or_else(|| analysis.props_type_declarations.get(&name))
                .cloned()
        }
        TSType::TSImportType(import_type) => {
            let resolved = vue3_resolve_import_type(import_type, analysis)?;
            resolved
                .context
                .props_options_type_declarations
                .get(&resolved.name)
                .cloned()
        }
        TSType::TSParenthesizedType(parenthesized) => {
            vue3_props_options_type_members(source, &parenthesized.type_annotation, analysis)
        }
        _ => None,
    }
}

pub(crate) fn vue3_runtime_props_options_from_signatures(
    source: &str,
    signatures: &[TSSignature<'_>],
    analysis: &Vue3ScriptSetupAnalysis,
) -> Vec<Vue27RuntimeProp> {
    let mut props = Vec::new();
    for signature in signatures {
        let TSSignature::TSPropertySignature(property) = signature else {
            continue;
        };
        if property.computed {
            continue;
        }
        let Some(key) = vue27_property_key_static_name(&property.key) else {
            continue;
        };
        let Some(type_annotation) = property.type_annotation.as_ref() else {
            continue;
        };
        let (types, required) =
            vue3_reverse_infer_runtime_prop_option_type(&type_annotation.type_annotation, analysis)
                .unwrap_or_else(|| (vec!["null".into()], false));
        props.push(Vue27RuntimeProp {
            key,
            types,
            required,
            default: None,
            is_method: false,
            type_annotation_source: source
                .get(type_annotation.span.start as usize..type_annotation.span.end as usize)
                .map(ToOwned::to_owned),
            member_source: source
                .get(property.span.start as usize..property.span.end as usize)
                .map(ToOwned::to_owned),
        });
    }
    props
}

pub(crate) fn vue3_static_runtime_props_options_type_members(
    source: &str,
    expression: &Expression<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vue27TypeMembers> {
    let object = vue3_static_runtime_props_options_object(expression)?;
    let mut members = Vec::new();
    for property in &object.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            return None;
        };
        if property.computed {
            return None;
        }
        let prop = vue3_static_runtime_prop_from_object_property(source, property, analysis)?;
        members.push(prop);
    }
    if members.is_empty() {
        return None;
    }
    Some(Vue27TypeMembers {
        source: source
            .get(object.span.start as usize..object.span.end as usize)
            .unwrap_or_default()
            .to_string(),
        members,
        errors: Vec::new(),
        interface_heritage: None,
    })
}

pub(crate) fn vue3_static_runtime_prop_from_object_property(
    source: &str,
    property: &ObjectProperty<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vue27RuntimeProp> {
    let key = vue27_property_key_static_name(&property.key)?;
    let (types, required, type_annotation_source) =
        vue3_static_runtime_prop_option_expression(source, &property.value, analysis)?;
    Some(Vue27RuntimeProp {
        key,
        types,
        required,
        default: None,
        is_method: false,
        type_annotation_source,
        member_source: source
            .get(property.span.start as usize..property.span.end as usize)
            .map(ToOwned::to_owned),
    })
}

pub(crate) fn vue3_static_runtime_prop_option_expression(
    source: &str,
    expression: &Expression<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<(Vec<String>, bool, Option<String>)> {
    if let Some((types, type_annotation_source)) =
        vue3_static_runtime_prop_type_expression(source, expression, analysis)
    {
        return Some((types, false, type_annotation_source));
    }

    let object = vue3_static_runtime_props_options_object(expression)?;
    let mut has_runtime_option_key = false;
    let mut types = None;
    let mut type_annotation_source = None;
    let mut required = false;
    for property in &object.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            return None;
        };
        if property.computed {
            return None;
        }
        let key = vue27_property_key_static_name(&property.key)?;
        match key.as_str() {
            "type" => {
                has_runtime_option_key = true;
                if let Some((resolved, source)) =
                    vue3_static_runtime_prop_type_expression(source, &property.value, analysis)
                {
                    types = Some(resolved);
                    type_annotation_source = source;
                }
            }
            "required" => {
                has_runtime_option_key = true;
                required = vue3_static_boolean_expression(&property.value).unwrap_or(false);
            }
            "default" | "validator" => {
                has_runtime_option_key = true;
            }
            _ => {}
        }
    }

    if !has_runtime_option_key {
        return None;
    }
    Some((
        types.unwrap_or_else(|| vec!["null".into()]),
        required,
        type_annotation_source,
    ))
}
