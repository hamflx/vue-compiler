#[derive(Clone, Copy)]
pub(crate) enum Vue3ArrayElementRuntimeMode {
    Props,
    DefineModel,
}

pub(crate) fn infer_vue3_indexed_access_runtime_type(
    indexed: &oxc_ast::ast::TSIndexedAccessType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vec<String>> {
    if let Some(index) = vue3_indexed_access_runtime_index(&indexed.index_type, analysis) {
        match index {
            Vue3RuntimeIndex::Number => {
                if let Some(types) = infer_vue3_array_element_runtime_type(
                    &indexed.object_type,
                    analysis,
                    Vue3ArrayElementRuntimeMode::Props,
                ) {
                    return Some(types);
                }
            }
            Vue3RuntimeIndex::Numeric(index) => {
                if let Some(types) = infer_vue3_tuple_index_runtime_type(
                    &indexed.object_type,
                    index,
                    analysis,
                    Vue3ArrayElementRuntimeMode::Props,
                ) {
                    return Some(types);
                }
            }
        }
    }
    let members = vue3_resolve_props_type("", &indexed.object_type, analysis)?;
    let keys = vue3_indexed_access_member_keys(&indexed.index_type, &members, analysis)?;
    let mut types = Vec::new();
    for key in keys {
        let Some(prop) = members.members.iter().find(|prop| prop.key == key) else {
            continue;
        };
        if prop.is_method {
            push_unique(&mut types, "Unknown");
        } else {
            for runtime_type in &prop.types {
                push_unique(&mut types, runtime_type);
            }
        }
    }
    if types.is_empty() {
        None
    } else {
        Some(types)
    }
}

pub(crate) fn infer_vue3_define_model_indexed_access_runtime_type(
    indexed: &oxc_ast::ast::TSIndexedAccessType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vec<String>> {
    if let Some(index) = vue3_indexed_access_runtime_index(&indexed.index_type, analysis) {
        match index {
            Vue3RuntimeIndex::Number => {
                if let Some(types) = infer_vue3_array_element_runtime_type(
                    &indexed.object_type,
                    analysis,
                    Vue3ArrayElementRuntimeMode::DefineModel,
                ) {
                    return Some(types);
                }
            }
            Vue3RuntimeIndex::Numeric(index) => {
                if let Some(types) = infer_vue3_tuple_index_runtime_type(
                    &indexed.object_type,
                    index,
                    analysis,
                    Vue3ArrayElementRuntimeMode::DefineModel,
                ) {
                    return Some(types);
                }
            }
        }
    }
    infer_vue3_indexed_access_runtime_type(indexed, analysis)
}

#[derive(Clone, Copy)]
pub(crate) enum Vue3RuntimeIndex {
    Number,
    Numeric(usize),
}

pub(crate) fn vue3_indexed_access_runtime_index(
    index_type: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vue3RuntimeIndex> {
    match index_type {
        TSType::TSNumberKeyword(_) => Some(Vue3RuntimeIndex::Number),
        TSType::TSLiteralType(literal) => match &literal.literal {
            TSLiteral::NumericLiteral(literal)
                if literal.value.fract() == 0.0 && literal.value >= 0.0 =>
            {
                Some(Vue3RuntimeIndex::Numeric(literal.value as usize))
            }
            _ => None,
        },
        TSType::TSParenthesizedType(parenthesized) => {
            vue3_indexed_access_runtime_index(&parenthesized.type_annotation, analysis)
        }
        TSType::TSTypeReference(reference) => {
            let Some(name) = vue3_ts_type_name_key(&reference.type_name) else {
                return None;
            };
            if analysis
                .declared_types
                .get(&name)
                .is_some_and(|types| types.len() == 1 && types[0] == "Number")
            {
                Some(Vue3RuntimeIndex::Number)
            } else {
                None
            }
        }
        _ => None,
    }
}

pub(crate) fn infer_vue3_array_element_runtime_type(
    node: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vec<String>> {
    match node {
        TSType::TSArrayType(array) => vue3_non_empty_runtime_types(vue3_runtime_types_for_mode(
            &array.element_type,
            analysis,
            mode,
        )),
        TSType::TSTupleType(_) => {
            vue3_runtime_types_from_tuple(infer_vue3_tuple_runtime_type(node, analysis, mode)?)
        }
        TSType::TSNamedTupleMember(member) => {
            infer_vue3_tuple_element_runtime_type(&member.element_type, analysis, mode)
        }
        TSType::TSTypeReference(reference) => {
            let name = vue3_ts_type_name_key(&reference.type_name)?;
            if let Some(tuple) =
                infer_vue3_parameter_utility_tuple_runtime_type(&name, reference, analysis, mode)
            {
                return vue3_runtime_types_from_tuple(tuple);
            }
            if let Some(tuple) = vue3_tuple_declaration_for_mode(analysis, &name, mode) {
                return vue3_runtime_types_from_tuple(tuple);
            }
            if let Some(types) = match mode {
                Vue3ArrayElementRuntimeMode::Props => analysis
                    .array_element_runtime_type_declarations
                    .get(&name)
                    .cloned(),
                Vue3ArrayElementRuntimeMode::DefineModel => analysis
                    .define_model_array_element_runtime_type_declarations
                    .get(&name)
                    .cloned(),
            } {
                return Some(types);
            }
            match name.as_str() {
                "Array" | "ReadonlyArray" => {
                    let ty = vue3_type_reference_type_argument(reference, 0)?;
                    vue3_non_empty_runtime_types(vue3_runtime_types_for_mode(ty, analysis, mode))
                }
                _ => None,
            }
        }
        TSType::TSImportType(import_type) => {
            let resolved = vue3_resolve_import_type(import_type, analysis)?;
            if let Some(tuple) =
                vue3_tuple_declaration_for_context(&resolved.context, &resolved.name, mode)
            {
                return vue3_runtime_types_from_tuple(tuple);
            }
            match mode {
                Vue3ArrayElementRuntimeMode::Props => resolved
                    .context
                    .array_element_runtime_type_declarations
                    .get(&resolved.name)
                    .cloned(),
                Vue3ArrayElementRuntimeMode::DefineModel => resolved
                    .context
                    .define_model_array_element_runtime_type_declarations
                    .get(&resolved.name)
                    .cloned(),
            }
        }
        TSType::TSParenthesizedType(parenthesized) => {
            infer_vue3_array_element_runtime_type(&parenthesized.type_annotation, analysis, mode)
        }
        TSType::TSUnionType(union) => {
            let mut types = Vec::new();
            for ty in &union.types {
                for runtime_type in infer_vue3_array_element_runtime_type(ty, analysis, mode)? {
                    push_unique(&mut types, &runtime_type);
                }
            }
            vue3_non_empty_runtime_types(types)
        }
        TSType::TSIntersectionType(intersection) => {
            let mut types = Vec::new();
            for ty in &intersection.types {
                let Some(runtime_types) = infer_vue3_array_element_runtime_type(ty, analysis, mode)
                else {
                    continue;
                };
                for runtime_type in runtime_types {
                    if runtime_type != "Unknown" {
                        push_unique(&mut types, &runtime_type);
                    }
                }
            }
            vue3_non_empty_runtime_types(types)
        }
        _ => None,
    }
}

pub(crate) fn infer_vue3_tuple_index_runtime_type(
    node: &TSType<'_>,
    index: usize,
    analysis: &Vue3ScriptSetupAnalysis,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vec<String>> {
    let tuple = infer_vue3_tuple_runtime_type(node, analysis, mode)?;
    tuple
        .get(index)
        .cloned()
        .and_then(vue3_non_empty_runtime_types)
}

pub(crate) fn infer_vue3_tuple_runtime_type(
    node: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vue3RuntimeTypeTuple> {
    match node {
        TSType::TSTupleType(tuple) => {
            let mut elements = Vec::new();
            for element in &tuple.element_types {
                elements.push(infer_vue3_tuple_element_runtime_type(
                    element, analysis, mode,
                )?);
            }
            vue3_non_empty_runtime_tuple(elements)
        }
        TSType::TSNamedTupleMember(member) => Some(vec![infer_vue3_tuple_element_runtime_type(
            &member.element_type,
            analysis,
            mode,
        )?]),
        TSType::TSTypeReference(reference) => {
            let name = vue3_ts_type_name_key(&reference.type_name)?;
            infer_vue3_parameter_utility_tuple_runtime_type(&name, reference, analysis, mode)
                .or_else(|| vue3_tuple_declaration_for_mode(analysis, &name, mode))
        }
        TSType::TSImportType(import_type) => {
            let resolved = vue3_resolve_import_type(import_type, analysis)?;
            vue3_tuple_declaration_for_context(&resolved.context, &resolved.name, mode)
        }
        TSType::TSParenthesizedType(parenthesized) => {
            infer_vue3_tuple_runtime_type(&parenthesized.type_annotation, analysis, mode)
        }
        TSType::TSUnionType(union) => {
            let mut merged = Vec::new();
            for ty in &union.types {
                let tuple = infer_vue3_tuple_runtime_type(ty, analysis, mode)?;
                merge_vue3_runtime_type_tuple(&mut merged, tuple);
            }
            vue3_non_empty_runtime_tuple(merged)
        }
        TSType::TSIntersectionType(intersection) => {
            for ty in &intersection.types {
                if let Some(tuple) = infer_vue3_tuple_runtime_type(ty, analysis, mode) {
                    return Some(tuple);
                }
            }
            None
        }
        _ => None,
    }
}

pub(crate) fn infer_vue3_parameter_utility_tuple_runtime_type(
    name: &str,
    reference: &TSTypeReference<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vue3RuntimeTypeTuple> {
    let ty = vue3_type_reference_type_argument(reference, 0)?;
    match name {
        "Parameters" => infer_vue3_function_parameter_tuple_runtime_type(ty, analysis, mode),
        "ConstructorParameters" => {
            infer_vue3_constructor_parameter_tuple_runtime_type(ty, analysis, mode)
        }
        _ => None,
    }
}

pub(crate) fn infer_vue3_function_parameter_tuple_runtime_type(
    node: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vue3RuntimeTypeTuple> {
    match node {
        TSType::TSFunctionType(function) => {
            infer_vue3_formal_parameters_tuple_runtime_type(&function.params, analysis, mode)
        }
        TSType::TSTypeLiteral(literal) => {
            infer_vue3_function_parameter_tuple_runtime_type_from_signatures(
                &literal.members,
                analysis,
                mode,
            )
        }
        TSType::TSTypeReference(reference) => {
            let name = vue3_ts_type_name_key(&reference.type_name)?;
            vue3_parameter_tuple_declaration_for_mode(analysis, &name, mode)
        }
        TSType::TSImportType(import_type) => {
            let resolved = vue3_resolve_import_type(import_type, analysis)?;
            vue3_parameter_tuple_declaration_for_context(&resolved.context, &resolved.name, mode)
        }
        TSType::TSParenthesizedType(parenthesized) => {
            infer_vue3_function_parameter_tuple_runtime_type(
                &parenthesized.type_annotation,
                analysis,
                mode,
            )
        }
        TSType::TSUnionType(union) => {
            let mut merged = Vec::new();
            for ty in &union.types {
                let tuple = infer_vue3_function_parameter_tuple_runtime_type(ty, analysis, mode)?;
                merge_vue3_runtime_type_tuple(&mut merged, tuple);
            }
            vue3_non_empty_runtime_tuple(merged)
        }
        _ => None,
    }
}

pub(crate) fn infer_vue3_function_parameter_tuple_runtime_type_from_interfaces(
    source: &str,
    declarations: &[&TSInterfaceDeclaration<'_>],
    analysis: &Vue3ScriptSetupAnalysis,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vue3RuntimeTypeTuple> {
    let mut merged = Vec::new();
    for declaration in declarations {
        if let Some(tuple) = infer_vue3_function_parameter_tuple_runtime_type_from_signatures(
            &declaration.body.body,
            analysis,
            mode,
        ) {
            merge_vue3_runtime_type_tuple(&mut merged, tuple);
        }
        for heritage in &declaration.extends {
            if vue3_interface_heritage_has_vue_ignore(source, heritage) {
                continue;
            }
            if let Some(tuple) = infer_vue3_function_parameter_tuple_runtime_type_from_heritage(
                source, heritage, analysis, mode,
            ) {
                merge_vue3_runtime_type_tuple(&mut merged, tuple);
            }
        }
    }
    vue3_non_empty_runtime_tuple(merged)
}

pub(crate) fn infer_vue3_function_parameter_tuple_runtime_type_from_heritage(
    source: &str,
    heritage: &TSInterfaceHeritage<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vue3RuntimeTypeTuple> {
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
            return infer_vue3_function_parameter_tuple_runtime_type(
                &declaration.type_annotation,
                analysis,
                mode,
            );
        }
    }
    None
}

pub(crate) fn infer_vue3_function_parameter_tuple_runtime_type_from_signatures(
    signatures: &[TSSignature<'_>],
    analysis: &Vue3ScriptSetupAnalysis,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vue3RuntimeTypeTuple> {
    let mut merged = Vec::new();
    for signature in signatures {
        if let TSSignature::TSCallSignatureDeclaration(signature) = signature {
            let tuple =
                infer_vue3_formal_parameters_tuple_runtime_type(&signature.params, analysis, mode)?;
            merge_vue3_runtime_type_tuple(&mut merged, tuple);
        }
    }
    vue3_non_empty_runtime_tuple(merged)
}

pub(crate) fn infer_vue3_constructor_parameter_tuple_runtime_type(
    node: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vue3RuntimeTypeTuple> {
    match node {
        TSType::TSConstructorType(constructor) => {
            infer_vue3_formal_parameters_tuple_runtime_type(&constructor.params, analysis, mode)
        }
        TSType::TSTypeLiteral(literal) => {
            infer_vue3_constructor_parameter_tuple_runtime_type_from_signatures(
                &literal.members,
                analysis,
                mode,
            )
        }
        TSType::TSTypeReference(reference) => {
            let name = vue3_ts_type_name_key(&reference.type_name)?;
            vue3_constructor_parameter_tuple_declaration_for_mode(analysis, &name, mode)
        }
        TSType::TSImportType(import_type) => {
            let resolved = vue3_resolve_import_type(import_type, analysis)?;
            vue3_constructor_parameter_tuple_declaration_for_context(
                &resolved.context,
                &resolved.name,
                mode,
            )
        }
        TSType::TSParenthesizedType(parenthesized) => {
            infer_vue3_constructor_parameter_tuple_runtime_type(
                &parenthesized.type_annotation,
                analysis,
                mode,
            )
        }
        TSType::TSUnionType(union) => {
            let mut merged = Vec::new();
            for ty in &union.types {
                let tuple =
                    infer_vue3_constructor_parameter_tuple_runtime_type(ty, analysis, mode)?;
                merge_vue3_runtime_type_tuple(&mut merged, tuple);
            }
            vue3_non_empty_runtime_tuple(merged)
        }
        _ => None,
    }
}

pub(crate) fn infer_vue3_constructor_parameter_tuple_runtime_type_from_interfaces(
    source: &str,
    declarations: &[&TSInterfaceDeclaration<'_>],
    analysis: &Vue3ScriptSetupAnalysis,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vue3RuntimeTypeTuple> {
    let mut merged = Vec::new();
    for declaration in declarations {
        if let Some(tuple) = infer_vue3_constructor_parameter_tuple_runtime_type_from_signatures(
            &declaration.body.body,
            analysis,
            mode,
        ) {
            merge_vue3_runtime_type_tuple(&mut merged, tuple);
        }
        for heritage in &declaration.extends {
            if vue3_interface_heritage_has_vue_ignore(source, heritage) {
                continue;
            }
            if let Some(tuple) = infer_vue3_constructor_parameter_tuple_runtime_type_from_heritage(
                source, heritage, analysis, mode,
            ) {
                merge_vue3_runtime_type_tuple(&mut merged, tuple);
            }
        }
    }
    vue3_non_empty_runtime_tuple(merged)
}

pub(crate) fn infer_vue3_constructor_parameter_tuple_runtime_type_from_heritage(
    source: &str,
    heritage: &TSInterfaceHeritage<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vue3RuntimeTypeTuple> {
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
            return infer_vue3_constructor_parameter_tuple_runtime_type(
                &declaration.type_annotation,
                analysis,
                mode,
            );
        }
    }
    None
}

pub(crate) fn infer_vue3_constructor_parameter_tuple_runtime_type_from_signatures(
    signatures: &[TSSignature<'_>],
    analysis: &Vue3ScriptSetupAnalysis,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vue3RuntimeTypeTuple> {
    let mut merged = Vec::new();
    for signature in signatures {
        if let TSSignature::TSConstructSignatureDeclaration(signature) = signature {
            let tuple =
                infer_vue3_formal_parameters_tuple_runtime_type(&signature.params, analysis, mode)?;
            merge_vue3_runtime_type_tuple(&mut merged, tuple);
        }
    }
    vue3_non_empty_runtime_tuple(merged)
}
