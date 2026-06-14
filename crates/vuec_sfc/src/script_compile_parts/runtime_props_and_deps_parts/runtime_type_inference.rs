pub(crate) fn infer_vue3_runtime_type_from_signatures(
    signatures: &[TSSignature<'_>],
    empty_type: &str,
) -> Vec<String> {
    let mut types = Vec::new();
    for signature in signatures {
        let runtime_type = match signature {
            TSSignature::TSCallSignatureDeclaration(_)
            | TSSignature::TSConstructSignatureDeclaration(_) => "Function",
            _ => "Object",
        };
        push_unique(&mut types, runtime_type);
    }
    if types.is_empty() {
        vec![empty_type.into()]
    } else {
        types
    }
}

pub(crate) fn infer_vue3_generic_keyof_runtime_type(
    reference: &TSTypeReference<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vec<String>> {
    let (alias_source, scoped_analysis) =
        vue3_scoped_analysis_for_generic_type_alias("", reference, analysis)?;
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        alias_source.as_str(),
        oxc_span::SourceType::ts(),
    )
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
            return infer_vue3_keyof_runtime_type(&declaration.type_annotation, &scoped_analysis);
        }
    }
    None
}

pub(crate) fn infer_vue3_generic_return_runtime_type(
    reference: &TSTypeReference<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vec<String>> {
    let (alias_source, scoped_analysis) =
        vue3_scoped_analysis_for_generic_type_alias("", reference, analysis)?;
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        alias_source.as_str(),
        oxc_span::SourceType::ts(),
    )
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
            return infer_vue3_return_runtime_type(
                &declaration.type_annotation,
                &scoped_analysis,
                mode,
            );
        }
    }
    None
}

pub(crate) fn vue3_mapped_identity_runtime_type_parameter(
    mapped: &TSMappedType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<String> {
    if analysis.generic_type_parameter_names.is_empty() {
        return None;
    }
    let type_annotation = mapped.type_annotation.as_ref()?;
    let TSType::TSIndexedAccessType(indexed) = type_annotation else {
        return None;
    };
    let TSType::TSTypeOperatorType(operator) = &mapped.constraint else {
        return None;
    };
    if operator.operator != TSTypeOperatorOperator::Keyof {
        return None;
    }
    let TSType::TSTypeReference(constraint_reference) = &operator.type_annotation else {
        return None;
    };
    let target_name = vue27_ts_type_name_identifier(&constraint_reference.type_name)?;
    if !analysis.generic_type_parameter_names.contains(target_name) {
        return None;
    }
    let TSType::TSTypeReference(object_reference) = &indexed.object_type else {
        return None;
    };
    if vue27_ts_type_name_identifier(&object_reference.type_name)? != target_name {
        return None;
    }
    let TSType::TSTypeReference(index_reference) = &indexed.index_type else {
        return None;
    };
    if vue27_ts_type_name_identifier(&index_reference.type_name)? != mapped.key.name.as_str() {
        return None;
    }
    Some(target_name.to_string())
}

pub(crate) fn infer_vue3_runtime_type(
    node: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Vec<String> {
    match node {
        TSType::TSStringKeyword(_) => vec!["String".into()],
        TSType::TSNumberKeyword(_) => vec!["Number".into()],
        TSType::TSBooleanKeyword(_) => vec!["Boolean".into()],
        TSType::TSObjectKeyword(_) => vec!["Object".into()],
        TSType::TSTypeLiteral(literal) => {
            infer_vue3_runtime_type_from_signatures(&literal.members, "Object")
        }
        TSType::TSFunctionType(_) | TSType::TSConstructorType(_) => vec!["Function".into()],
        TSType::TSArrayType(_) | TSType::TSTupleType(_) => vec!["Array".into()],
        TSType::TSSymbolKeyword(_) => vec!["Symbol".into()],
        TSType::TSNullKeyword(_) => vec!["null".into()],
        TSType::TSAnyKeyword(_)
        | TSType::TSBigIntKeyword(_)
        | TSType::TSNeverKeyword(_)
        | TSType::TSUndefinedKeyword(_)
        | TSType::TSUnknownKeyword(_)
        | TSType::TSVoidKeyword(_) => vec!["Unknown".into()],
        TSType::TSLiteralType(literal) => match &literal.literal {
            TSLiteral::StringLiteral(_) => vec!["String".into()],
            TSLiteral::BooleanLiteral(_) => vec!["Boolean".into()],
            TSLiteral::NumericLiteral(_) | TSLiteral::BigIntLiteral(_) => vec!["Number".into()],
            _ => vec!["Unknown".into()],
        },
        TSType::TSTypeReference(reference) => {
            if let Some(name) = vue3_ts_type_name_key(&reference.type_name) {
                if let Some(types) = infer_vue3_generic_type_alias_runtime_type(reference, analysis)
                {
                    return types;
                }
                if let Some(types) = analysis.declared_types.get(&name) {
                    return types.clone();
                }
                if let Some(types) = infer_vue3_runtime_utility_type(&name, reference, analysis) {
                    return types;
                }
                match name.as_str() {
                    "Array" | "Function" | "Object" | "Set" | "Map" | "WeakSet" | "WeakMap"
                    | "Date" | "Promise" | "Error" => return vec![name],
                    "Record" | "Partial" | "Readonly" | "Pick" | "Omit" | "Required"
                    | "InstanceType" => return vec!["Object".into()],
                    _ => {}
                }
            }
            vec!["Unknown".into()]
        }
        TSType::TSImportType(import_type) => {
            if let Some(resolved) = vue3_resolve_import_type(import_type, analysis) {
                if let Some(types) = resolved.context.declared_types.get(&resolved.name) {
                    return types.clone();
                }
            }
            vec!["Unknown".into()]
        }
        TSType::TSTypeQuery(query) => {
            if let Some(types) = vue3_type_query_runtime_type_declaration(query, analysis) {
                return types;
            }
            vec!["Unknown".into()]
        }
        TSType::TSParenthesizedType(parenthesized) => {
            infer_vue3_runtime_type(&parenthesized.type_annotation, analysis)
        }
        TSType::TSIndexedAccessType(indexed) => {
            infer_vue3_indexed_access_runtime_type(indexed, analysis)
                .unwrap_or_else(|| vec!["Unknown".into()])
        }
        TSType::TSTypeOperatorType(operator) => {
            if operator.operator == TSTypeOperatorOperator::Keyof {
                return infer_vue3_keyof_runtime_type(&operator.type_annotation, analysis)
                    .unwrap_or_else(|| vec!["Unknown".into()]);
            }
            infer_vue3_runtime_type(&operator.type_annotation, analysis)
        }
        TSType::TSUnionType(union) => {
            let mut types = Vec::new();
            for ty in &union.types {
                for runtime_type in infer_vue3_runtime_type(ty, analysis) {
                    push_unique(&mut types, &runtime_type);
                }
            }
            types
        }
        TSType::TSIntersectionType(intersection) => {
            let mut types = Vec::new();
            for ty in &intersection.types {
                for runtime_type in infer_vue3_runtime_type(ty, analysis) {
                    if runtime_type != "Unknown" {
                        push_unique(&mut types, &runtime_type);
                    }
                }
            }
            if types.is_empty() {
                vec!["Unknown".into()]
            } else {
                types
            }
        }
        TSType::TSMappedType(mapped) => {
            if let Some(type_name) = vue3_mapped_identity_runtime_type_parameter(mapped, analysis) {
                if let Some(types) = analysis.declared_types.get(&type_name) {
                    return types.clone();
                }
            }
            vec!["Unknown".into()]
        }
        TSType::TSConditionalType(conditional) => infer_vue3_conditional_runtime_type(
            conditional,
            analysis,
            Vue3ArrayElementRuntimeMode::Props,
        )
        .unwrap_or_else(|| vec!["Unknown".into()]),
        _ => vec!["Unknown".into()],
    }
}

pub(crate) fn infer_vue3_runtime_utility_type(
    name: &str,
    reference: &TSTypeReference<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vec<String>> {
    match name {
        "NonNullable" => {
            let ty = vue3_type_reference_type_argument(reference, 0)?;
            let mut types = infer_vue3_runtime_type(ty, analysis);
            types.retain(|ty| ty != "null");
            Some(types)
        }
        "Extract" => {
            let ty = vue3_type_reference_type_argument(reference, 1)?;
            Some(infer_vue3_runtime_type(ty, analysis))
        }
        "Exclude" => {
            let ty = vue3_type_reference_type_argument(reference, 0)?;
            Some(infer_vue3_runtime_type(ty, analysis))
        }
        "OmitThisParameter" => {
            let ty = vue3_type_reference_type_argument(reference, 0)?;
            Some(infer_vue3_runtime_type(ty, analysis))
        }
        "ReturnType" => {
            let ty = vue3_type_reference_type_argument(reference, 0)?;
            infer_vue3_return_runtime_type(ty, analysis, Vue3ArrayElementRuntimeMode::Props)
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
                for runtime_type in infer_vue3_runtime_type(ty, analysis) {
                    push_unique(&mut types, &runtime_type);
                }
            }
            Some(types)
        }
        _ => None,
    }
}

pub(crate) fn infer_vue3_define_model_runtime_type(
    node: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Vec<String> {
    match node {
        TSType::TSStringKeyword(_) => vec!["String".into()],
        TSType::TSNumberKeyword(_) => vec!["Number".into()],
        TSType::TSBooleanKeyword(_) => vec!["Boolean".into()],
        TSType::TSObjectKeyword(_) => vec!["Object".into()],
        TSType::TSTypeLiteral(literal) => {
            infer_vue3_runtime_type_from_signatures(&literal.members, "Object")
        }
        TSType::TSIntersectionType(intersection) => {
            let mut types = Vec::new();
            for ty in &intersection.types {
                for runtime_type in infer_vue3_define_model_runtime_type(ty, analysis) {
                    if runtime_type != "Unknown" {
                        push_unique(&mut types, &runtime_type);
                    }
                }
            }
            if types.is_empty() {
                vec!["Unknown".into()]
            } else {
                types
            }
        }
        TSType::TSFunctionType(_) | TSType::TSConstructorType(_) => vec!["Function".into()],
        TSType::TSArrayType(_) | TSType::TSTupleType(_) => vec!["Array".into()],
        TSType::TSSymbolKeyword(_) => vec!["Symbol".into()],
        TSType::TSNullKeyword(_) => vec!["null".into()],
        TSType::TSAnyKeyword(_)
        | TSType::TSBigIntKeyword(_)
        | TSType::TSNeverKeyword(_)
        | TSType::TSUndefinedKeyword(_)
        | TSType::TSUnknownKeyword(_)
        | TSType::TSVoidKeyword(_) => vec!["Unknown".into()],
        TSType::TSLiteralType(literal) => match &literal.literal {
            TSLiteral::StringLiteral(_) => vec!["String".into()],
            TSLiteral::BooleanLiteral(_) => vec!["Boolean".into()],
            TSLiteral::NumericLiteral(_) | TSLiteral::BigIntLiteral(_) => vec!["Number".into()],
            _ => vec!["Unknown".into()],
        },
        TSType::TSTypeReference(reference) => {
            if let Some(name) = vue3_ts_type_name_key(&reference.type_name) {
                if let Some(types) =
                    infer_vue3_generic_define_model_runtime_type(reference, analysis)
                {
                    return types;
                }
                if let Some(types) = analysis.define_model_declared_types.get(&name) {
                    return types.clone();
                }
                if let Some(types) =
                    infer_vue3_define_model_runtime_utility_type(&name, reference, analysis)
                {
                    return types;
                }
                match name.as_str() {
                    "Array" | "Function" | "Object" | "Set" | "Map" | "WeakSet" | "WeakMap"
                    | "Date" | "Promise" | "Error" => return vec![name],
                    "Record" | "Partial" | "Readonly" | "Pick" | "Omit" | "Required"
                    | "InstanceType" => return vec!["Object".into()],
                    _ => {}
                }
            }
            vec!["Unknown".into()]
        }
        TSType::TSImportType(import_type) => {
            if let Some(resolved) = vue3_resolve_import_type(import_type, analysis) {
                if let Some(types) = resolved
                    .context
                    .define_model_declared_types
                    .get(&resolved.name)
                {
                    return types.clone();
                }
            }
            vec!["Unknown".into()]
        }
        TSType::TSTypeQuery(query) => {
            if let Some(types) =
                vue3_type_query_define_model_runtime_type_declaration(query, analysis)
            {
                return types;
            }
            vec!["Unknown".into()]
        }
        TSType::TSParenthesizedType(parenthesized) => {
            infer_vue3_define_model_runtime_type(&parenthesized.type_annotation, analysis)
        }
        TSType::TSIndexedAccessType(indexed) => {
            infer_vue3_define_model_indexed_access_runtime_type(indexed, analysis)
                .unwrap_or_else(|| vec!["Unknown".into()])
        }
        TSType::TSTypeOperatorType(operator) => {
            if operator.operator == TSTypeOperatorOperator::Keyof {
                return infer_vue3_keyof_runtime_type(&operator.type_annotation, analysis)
                    .unwrap_or_else(|| vec!["Unknown".into()]);
            }
            infer_vue3_define_model_runtime_type(&operator.type_annotation, analysis)
        }
        TSType::TSUnionType(union) => {
            let mut types = Vec::new();
            for ty in &union.types {
                for runtime_type in infer_vue3_define_model_runtime_type(ty, analysis) {
                    push_unique(&mut types, &runtime_type);
                }
            }
            types
        }
        TSType::TSMappedType(mapped) => {
            if let Some(type_name) = vue3_mapped_identity_runtime_type_parameter(mapped, analysis) {
                if let Some(types) = analysis.define_model_declared_types.get(&type_name) {
                    return types.clone();
                }
            }
            vec!["Unknown".into()]
        }
        TSType::TSConditionalType(conditional) => infer_vue3_conditional_runtime_type(
            conditional,
            analysis,
            Vue3ArrayElementRuntimeMode::DefineModel,
        )
        .unwrap_or_else(|| vec!["Unknown".into()]),
        _ => vec!["Unknown".into()],
    }
}

pub(crate) fn infer_vue3_conditional_runtime_type(
    conditional: &oxc_ast::ast::TSConditionalType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vec<String>> {
    let outcome = vue3_static_conditional_type_outcome(
        &conditional.check_type,
        &conditional.extends_type,
        analysis,
    )?;
    let branch = match outcome {
        Vue3StaticConditionalTypeOutcome::True => &conditional.true_type,
        Vue3StaticConditionalTypeOutcome::False => &conditional.false_type,
    };
    vue3_non_empty_runtime_types(vue3_runtime_types_for_mode(branch, analysis, mode))
}
