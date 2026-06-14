pub(crate) fn vue3_static_runtime_prop_type_expression(
    source: &str,
    expression: &Expression<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<(Vec<String>, Option<String>)> {
    match expression {
        Expression::TSAsExpression(expression) => {
            if let Some(types) =
                vue3_reverse_infer_runtime_prop_type(&expression.type_annotation, analysis)
            {
                return Some((
                    types,
                    source
                        .get(
                            expression.type_annotation.span().start as usize
                                ..expression.type_annotation.span().end as usize,
                        )
                        .map(ToOwned::to_owned),
                ));
            }
            vue3_static_runtime_prop_type_expression(source, &expression.expression, analysis)
        }
        Expression::TSTypeAssertion(expression) => {
            if let Some(types) =
                vue3_reverse_infer_runtime_prop_type(&expression.type_annotation, analysis)
            {
                return Some((
                    types,
                    source
                        .get(
                            expression.type_annotation.span().start as usize
                                ..expression.type_annotation.span().end as usize,
                        )
                        .map(ToOwned::to_owned),
                ));
            }
            vue3_static_runtime_prop_type_expression(source, &expression.expression, analysis)
        }
        Expression::TSSatisfiesExpression(expression) => {
            if let Some(types) =
                vue3_reverse_infer_runtime_prop_type(&expression.type_annotation, analysis)
            {
                return Some((
                    types,
                    source
                        .get(
                            expression.type_annotation.span().start as usize
                                ..expression.type_annotation.span().end as usize,
                        )
                        .map(ToOwned::to_owned),
                ));
            }
            vue3_static_runtime_prop_type_expression(source, &expression.expression, analysis)
        }
        Expression::TSNonNullExpression(expression) => {
            vue3_static_runtime_prop_type_expression(source, &expression.expression, analysis)
        }
        Expression::TSInstantiationExpression(expression) => {
            vue3_static_runtime_prop_type_expression(source, &expression.expression, analysis)
        }
        Expression::ParenthesizedExpression(expression) => {
            vue3_static_runtime_prop_type_expression(source, &expression.expression, analysis)
        }
        Expression::Identifier(identifier) => {
            let name = vue3_return_expression_constructor_runtime_name(identifier.name.as_str())?;
            Some((vec![name.to_string()], None))
        }
        Expression::StaticMemberExpression(member) => {
            let name =
                vue3_return_expression_constructor_runtime_name(member.property.name.as_str())?;
            Some((vec![name.to_string()], None))
        }
        Expression::NullLiteral(_) => Some((vec!["null".into()], None)),
        Expression::ArrayExpression(array) => {
            let mut types = Vec::new();
            let mut type_annotation_source = None;
            for element in &array.elements {
                let expression = element.as_expression()?;
                let (element_types, element_type_annotation_source) =
                    vue3_static_runtime_prop_type_expression(source, expression, analysis)?;
                merge_vue3_runtime_types(&mut types, element_types);
                if type_annotation_source.is_none() {
                    type_annotation_source = element_type_annotation_source;
                }
            }
            vue3_non_empty_runtime_types(types).map(|types| (types, type_annotation_source))
        }
        _ => None,
    }
}

pub(crate) fn vue3_static_boolean_expression(expression: &Expression<'_>) -> Option<bool> {
    match unwrap_vue3_ts_expression(expression) {
        Expression::BooleanLiteral(literal) => Some(literal.value),
        _ => None,
    }
}

pub(crate) fn vue3_reverse_infer_runtime_prop_option_type(
    ty: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<(Vec<String>, bool)> {
    match ty {
        TSType::TSTypeLiteral(literal) => {
            let type_ty = vue3_static_property_type(literal, "type")?;
            let required = vue3_static_boolean_property_type(literal, "required").unwrap_or(false);
            let types = vue3_reverse_infer_runtime_prop_type(type_ty, analysis)
                .unwrap_or_else(|| vec!["null".into()]);
            Some((types, required))
        }
        _ => vue3_reverse_infer_runtime_prop_type(ty, analysis).map(|types| (types, false)),
    }
}

pub(crate) fn vue3_reverse_infer_runtime_prop_type(
    ty: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vec<String>> {
    match ty {
        TSType::TSTypeReference(reference) => {
            let name = vue3_ts_type_name_key(&reference.type_name)?;
            if let Some(ctor) = name.strip_suffix("Constructor") {
                return vue3_constructor_runtime_type(ctor);
            }
            if name == "PropType" {
                let ty = vue3_type_reference_first_type_argument(reference)?;
                return Some(infer_vue3_runtime_type(ty, analysis));
            }
            if let Some(type_arguments) = reference.type_arguments.as_ref() {
                for ty in &type_arguments.params {
                    if let Some(types) = vue3_reverse_infer_runtime_prop_type(ty, analysis) {
                        return Some(types);
                    }
                }
            }
            None
        }
        TSType::TSImportType(import_type) => {
            if let Some(type_arguments) = import_type.type_arguments.as_ref() {
                for ty in &type_arguments.params {
                    if let Some(types) = vue3_reverse_infer_runtime_prop_type(ty, analysis) {
                        return Some(types);
                    }
                }
            }
            None
        }
        TSType::TSParenthesizedType(parenthesized) => {
            vue3_reverse_infer_runtime_prop_type(&parenthesized.type_annotation, analysis)
        }
        _ => None,
    }
}

pub(crate) fn vue3_constructor_runtime_type(name: &str) -> Option<Vec<String>> {
    match name {
        "String" => Some(vec!["String".into()]),
        "Number" => Some(vec!["Number".into()]),
        "Boolean" => Some(vec!["Boolean".into()]),
        "Array" => Some(vec!["Array".into()]),
        "Object" => Some(vec!["Object".into()]),
        "Function" => Some(vec!["Function".into()]),
        "Set" => Some(vec!["Set".into()]),
        "Map" => Some(vec!["Map".into()]),
        "WeakSet" => Some(vec!["WeakSet".into()]),
        "WeakMap" => Some(vec!["WeakMap".into()]),
        "Date" => Some(vec!["Date".into()]),
        "Promise" => Some(vec!["Promise".into()]),
        _ => None,
    }
}

pub(crate) fn vue3_static_property_type<'a>(
    literal: &'a TSTypeLiteral<'a>,
    key: &str,
) -> Option<&'a TSType<'a>> {
    for member in &literal.members {
        let TSSignature::TSPropertySignature(property) = member else {
            continue;
        };
        if property.computed
            || vue27_property_key_static_name(&property.key).as_deref() != Some(key)
        {
            continue;
        }
        return property
            .type_annotation
            .as_ref()
            .map(|annotation| &annotation.type_annotation);
    }
    None
}

pub(crate) fn vue3_static_boolean_property_type(
    literal: &TSTypeLiteral<'_>,
    key: &str,
) -> Option<bool> {
    let TSType::TSLiteralType(literal) = vue3_static_property_type(literal, key)? else {
        return None;
    };
    let TSLiteral::BooleanLiteral(value) = &literal.literal else {
        return None;
    };
    Some(value.value)
}
