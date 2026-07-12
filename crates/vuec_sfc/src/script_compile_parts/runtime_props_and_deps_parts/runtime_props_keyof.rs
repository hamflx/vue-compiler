pub(crate) fn vue3_runtime_props_from_signatures(
    source: &str,
    signatures: &[TSSignature<'_>],
    analysis: &Vue3ScriptSetupAnalysis,
) -> (Vec<Vue27RuntimeProp>, Vec<String>) {
    let mut props = Vec::new();
    let mut errors = Vec::new();
    for signature in signatures {
        match signature {
            TSSignature::TSPropertySignature(property) => {
                let Some(key) = vue3_props_type_signature_key(&property.key, property.computed)
                else {
                    errors.push(vue3_unsupported_computed_key_error());
                    continue;
                };
                let types = property
                    .type_annotation
                    .as_ref()
                    .map(|annotation| {
                        if vue3_type_annotation_has_vue_ignore(source, annotation) {
                            vec!["Unknown".into()]
                        } else {
                            infer_vue3_runtime_type(&annotation.type_annotation, analysis)
                        }
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
                                .get(annotation.span.start as usize..annotation.span.end as usize)
                                .map(ToOwned::to_owned)
                        },
                    ),
                    member_source: source
                        .get(property.span.start as usize..property.span.end as usize)
                        .map(ToOwned::to_owned),
                });
            }
            TSSignature::TSMethodSignature(method) => {
                let Some(key) = vue3_props_type_signature_key(&method.key, method.computed) else {
                    errors.push(vue3_unsupported_computed_key_error());
                    continue;
                };
                props.push(Vue27RuntimeProp {
                    key,
                    types: vec!["Function".into()],
                    required: !method.optional,
                    default: None,
                    is_method: true,
                    type_annotation_source: method.return_type.as_ref().and_then(|annotation| {
                        source
                            .get(annotation.span.start as usize..annotation.span.end as usize)
                            .map(ToOwned::to_owned)
                    }),
                    member_source: source
                        .get(method.span.start as usize..method.span.end as usize)
                        .map(ToOwned::to_owned),
                });
            }
            _ => {}
        }
    }
    (props, errors)
}

pub(crate) fn vue3_props_type_signature_key(
    key: &PropertyKey<'_>,
    computed: bool,
) -> Option<String> {
    match (computed, key) {
        (false, PropertyKey::StaticIdentifier(identifier)) => Some(identifier.name.to_string()),
        (false, PropertyKey::StringLiteral(literal)) => Some(literal.value.to_string()),
        (false, PropertyKey::NumericLiteral(literal)) => Some(literal.value.to_string()),
        (true, PropertyKey::TemplateLiteral(template)) if template.expressions.is_empty() => {
            let mut key = String::new();
            for quasi in &template.quasis {
                key.push_str(&vue3_template_value(quasi));
            }
            Some(key)
        }
        _ => None,
    }
}

pub(crate) fn vue3_template_value(quasi: &oxc_ast::ast::TemplateElement<'_>) -> String {
    quasi
        .value
        .cooked
        .as_ref()
        .map(|value| value.as_str().to_string())
        .unwrap_or_else(|| quasi.value.raw.as_str().to_string())
}

pub(crate) fn vue3_keyof_runtime_type_from_interface(
    source: &str,
    declaration: &TSInterfaceDeclaration<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vec<String>> {
    let mut types = vue3_keyof_runtime_type_from_signatures(&declaration.body.body, analysis)
        .unwrap_or_default();
    for heritage in &declaration.extends {
        if vue3_interface_heritage_has_vue_ignore(source, heritage) {
            continue;
        }
        let Some(base) =
            vue3_resolve_interface_heritage_keyof_runtime_type(source, heritage, analysis)
        else {
            continue;
        };
        for runtime_type in base {
            push_unique(&mut types, &runtime_type);
        }
    }
    if types.is_empty() {
        None
    } else {
        Some(types)
    }
}

pub(crate) fn vue3_keyof_runtime_type_from_interface_declarations(
    source: &str,
    declarations: &[&TSInterfaceDeclaration<'_>],
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vec<String>> {
    let mut types = Vec::new();
    for declaration in declarations {
        let Some(runtime_types) =
            vue3_keyof_runtime_type_from_interface(source, declaration, analysis)
        else {
            continue;
        };
        for runtime_type in runtime_types {
            push_unique(&mut types, &runtime_type);
        }
    }
    if types.is_empty() {
        None
    } else {
        Some(types)
    }
}

pub(crate) fn vue3_resolve_interface_heritage_keyof_runtime_type(
    source: &str,
    heritage: &TSInterfaceHeritage<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vec<String>> {
    let ty_source = vue3_interface_heritage_type_source(source, heritage)?;
    let wrapped = format!("type __VuecResolved = {ty_source}");
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, &wrapped, oxc_span::SourceType::ts())
        .with_options(oxc_parser::ParseOptions {
            parse_regular_expression: true,
            ..oxc_parser::ParseOptions::default()
        })
        .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return None;
    }
    for statement in &parsed.program.body {
        if let Statement::TSTypeAliasDeclaration(declaration) = statement {
            return infer_vue3_keyof_runtime_type(&declaration.type_annotation, analysis);
        }
    }
    None
}

pub(crate) fn vue3_keyof_runtime_type_from_signatures(
    signatures: &[TSSignature<'_>],
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vec<String>> {
    let mut types = Vec::new();
    for signature in signatures {
        match signature {
            TSSignature::TSPropertySignature(property) => {
                let runtime_type = if matches!(property.key, PropertyKey::NumericLiteral(_)) {
                    "Number"
                } else {
                    "String"
                };
                push_unique(&mut types, runtime_type);
            }
            TSSignature::TSIndexSignature(signature) => {
                let parameter = signature.parameters.first()?;
                let runtime_types =
                    infer_vue3_runtime_type(&parameter.type_annotation.type_annotation, analysis);
                let runtime_type = runtime_types.first()?;
                if runtime_type == "null" || runtime_type == "Unknown" {
                    return None;
                }
                push_unique(&mut types, runtime_type);
            }
            _ => push_unique(&mut types, "String"),
        }
    }
    if types.is_empty() {
        None
    } else {
        Some(types)
    }
}

pub(crate) fn vue3_keyof_runtime_type_from_runtime_props(
    props: &[Vue27RuntimeProp],
) -> Option<Vec<String>> {
    let mut types = Vec::new();
    for prop in props {
        let runtime_type = prop
            .member_source
            .as_deref()
            .map(str::trim_start)
            .filter(|source| source.chars().next().is_some_and(|ch| ch.is_ascii_digit()))
            .map(|_| "Number")
            .unwrap_or("String");
        push_unique(&mut types, runtime_type);
    }
    if types.is_empty() {
        None
    } else {
        Some(types)
    }
}

pub(crate) fn infer_vue3_keyof_runtime_type(
    node: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vec<String>> {
    match node {
        TSType::TSStringKeyword(_) => Some(vec!["String".into()]),
        TSType::TSNumberKeyword(_) => Some(vec!["Number".into()]),
        TSType::TSBooleanKeyword(_) => Some(vec!["Boolean".into()]),
        TSType::TSObjectKeyword(_) => Some(vec!["Object".into()]),
        TSType::TSFunctionType(_) | TSType::TSConstructorType(_) => Some(vec!["Function".into()]),
        TSType::TSArrayType(_) | TSType::TSTupleType(_) => Some(vec!["Array".into()]),
        TSType::TSSymbolKeyword(_) => Some(vec!["Symbol".into()]),
        TSType::TSAnyKeyword(_) => Some(vec!["String".into(), "Number".into(), "Symbol".into()]),
        TSType::TSNullKeyword(_) => Some(vec!["null".into()]),
        TSType::TSLiteralType(literal) => match &literal.literal {
            TSLiteral::StringLiteral(_) => Some(vec!["String".into()]),
            TSLiteral::BooleanLiteral(_) => Some(vec!["Boolean".into()]),
            TSLiteral::NumericLiteral(_) | TSLiteral::BigIntLiteral(_) => {
                Some(vec!["Number".into()])
            }
            _ => None,
        },
        TSType::TSTypeLiteral(literal) => {
            vue3_keyof_runtime_type_from_signatures(&literal.members, analysis)
        }
        TSType::TSTypeReference(reference) => {
            let name = vue3_ts_type_name_key(&reference.type_name)?;
            if let Some(types) = infer_vue3_generic_keyof_runtime_type(reference, analysis) {
                return Some(types);
            }
            if let Some(types) = analysis.keyof_runtime_type_declarations.get(&name) {
                return Some(types.clone());
            }
            match name.as_str() {
                "String"
                | "Array"
                | "ArrayLike"
                | "Parameters"
                | "ConstructorParameters"
                | "ReadonlyArray" => Some(vec!["String".into(), "Number".into()]),
                "Record" | "Partial" | "Required" | "Readonly" => {
                    let ty = vue3_type_reference_type_argument(reference, 0)?;
                    infer_vue3_keyof_runtime_type(ty, analysis)
                }
                "Pick" | "Extract" => {
                    let ty = vue3_type_reference_type_argument(reference, 1)?;
                    Some(infer_vue3_runtime_type(ty, analysis))
                }
                "Function" | "Object" | "Set" | "Map" | "WeakSet" | "WeakMap" | "Date"
                | "Promise" | "Error" | "Uppercase" | "Lowercase" | "Capitalize"
                | "Uncapitalize" | "ReadonlyMap" | "ReadonlySet" => Some(vec!["String".into()]),
                _ => None,
            }
        }
        TSType::TSImportType(import_type) => {
            let resolved = vue3_resolve_import_type(import_type, analysis)?;
            resolved
                .context
                .keyof_runtime_type_declarations
                .get(&resolved.name)
                .cloned()
        }
        TSType::TSTypeQuery(query) => {
            vue3_type_query_keyof_runtime_type_declaration(query, analysis)
        }
        TSType::TSParenthesizedType(parenthesized) => {
            infer_vue3_keyof_runtime_type(&parenthesized.type_annotation, analysis)
        }
        TSType::TSIndexedAccessType(indexed) => {
            if let Some(members) = vue3_resolve_indexed_access_props_type(
                "",
                indexed,
                analysis,
                Vue3PropsTypeResolveMode::Silent,
            ) {
                if let Some(types) = vue3_keyof_runtime_type_from_runtime_props(&members.members) {
                    return Some(types);
                }
            }
            infer_vue3_indexed_access_runtime_type(indexed, analysis)
        }
        TSType::TSUnionType(union) => {
            let mut types = Vec::new();
            for ty in &union.types {
                for runtime_type in infer_vue3_keyof_runtime_type(ty, analysis)? {
                    push_unique(&mut types, &runtime_type);
                }
            }
            if types.is_empty() {
                None
            } else {
                Some(types)
            }
        }
        TSType::TSIntersectionType(intersection) => {
            let mut types = Vec::new();
            for ty in &intersection.types {
                let Some(runtime_types) = infer_vue3_keyof_runtime_type(ty, analysis) else {
                    continue;
                };
                for runtime_type in runtime_types {
                    push_unique(&mut types, &runtime_type);
                }
            }
            if types.is_empty() {
                None
            } else {
                Some(types)
            }
        }
        TSType::TSTypeOperatorType(operator) => {
            infer_vue3_keyof_runtime_type(&operator.type_annotation, analysis)
        }
        _ => None,
    }
}
