pub(crate) fn infer_vue3_return_runtime_type(
    node: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vec<String>> {
    match node {
        TSType::TSFunctionType(function) => vue3_non_empty_runtime_types(
            vue3_runtime_types_for_mode(&function.return_type.type_annotation, analysis, mode),
        ),
        TSType::TSTypeLiteral(literal) => {
            infer_vue3_return_runtime_type_from_signatures(&literal.members, analysis, mode)
        }
        TSType::TSTypeReference(reference) => {
            let name = vue3_ts_type_name_key(&reference.type_name)?;
            infer_vue3_generic_return_runtime_type(reference, analysis, mode)
                .or_else(|| vue3_return_type_declaration_for_mode(analysis, &name, mode))
        }
        TSType::TSImportType(import_type) => {
            let resolved = vue3_resolve_import_type(import_type, analysis)?;
            vue3_return_type_declaration_for_context(&resolved.context, &resolved.name, mode)
        }
        TSType::TSTypeQuery(query) => {
            vue3_return_type_declaration_for_type_query(query, analysis, mode)
        }
        TSType::TSParenthesizedType(parenthesized) => {
            infer_vue3_return_runtime_type(&parenthesized.type_annotation, analysis, mode)
        }
        TSType::TSUnionType(union) => {
            let mut types = Vec::new();
            for ty in &union.types {
                let runtime_types = infer_vue3_return_runtime_type(ty, analysis, mode)?;
                merge_vue3_runtime_types(&mut types, runtime_types);
            }
            vue3_non_empty_runtime_types(types)
        }
        TSType::TSIntersectionType(intersection) => {
            let mut types = Vec::new();
            for ty in &intersection.types {
                let Some(runtime_types) = infer_vue3_return_runtime_type(ty, analysis, mode) else {
                    continue;
                };
                merge_vue3_runtime_types(&mut types, runtime_types);
            }
            vue3_non_empty_runtime_types(types)
        }
        _ => None,
    }
}

pub(crate) fn infer_vue3_return_runtime_type_from_interfaces(
    source: &str,
    declarations: &[&TSInterfaceDeclaration<'_>],
    analysis: &Vue3ScriptSetupAnalysis,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vec<String>> {
    let mut types = Vec::new();
    for declaration in declarations {
        if let Some(runtime_types) =
            infer_vue3_return_runtime_type_from_signatures(&declaration.body.body, analysis, mode)
        {
            merge_vue3_runtime_types(&mut types, runtime_types);
        }
        for heritage in &declaration.extends {
            if vue3_interface_heritage_has_vue_ignore(source, heritage) {
                continue;
            }
            if let Some(runtime_types) =
                infer_vue3_return_runtime_type_from_heritage(source, heritage, analysis, mode)
            {
                merge_vue3_runtime_types(&mut types, runtime_types);
            }
        }
    }
    vue3_non_empty_runtime_types(types)
}

pub(crate) fn infer_vue3_return_runtime_type_from_heritage(
    source: &str,
    heritage: &TSInterfaceHeritage<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    mode: Vue3ArrayElementRuntimeMode,
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
            return infer_vue3_return_runtime_type(&declaration.type_annotation, analysis, mode);
        }
    }
    None
}

pub(crate) fn infer_vue3_return_runtime_type_from_signatures(
    signatures: &[TSSignature<'_>],
    analysis: &Vue3ScriptSetupAnalysis,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vec<String>> {
    let mut types = Vec::new();
    for signature in signatures {
        if let TSSignature::TSCallSignatureDeclaration(signature) = signature {
            let runtime_types = signature
                .return_type
                .as_ref()
                .map(|annotation| {
                    vue3_non_empty_runtime_types(vue3_runtime_types_for_mode(
                        &annotation.type_annotation,
                        analysis,
                        mode,
                    ))
                })
                .unwrap_or_else(|| Some(vec!["Unknown".into()]))?;
            merge_vue3_runtime_types(&mut types, runtime_types);
        }
    }
    vue3_non_empty_runtime_types(types)
}

pub(crate) fn infer_vue3_formal_parameters_tuple_runtime_type(
    parameters: &FormalParameters<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vue3RuntimeTypeTuple> {
    let mut tuple = Vec::new();
    for parameter in &parameters.items {
        let runtime_types = parameter
            .type_annotation
            .as_ref()
            .map(|annotation| {
                vue3_non_empty_runtime_types(vue3_runtime_types_for_mode(
                    &annotation.type_annotation,
                    analysis,
                    mode,
                ))
            })
            .unwrap_or_else(|| Some(vec!["Unknown".into()]))?;
        tuple.push(runtime_types);
    }
    if let Some(rest) = parameters.rest.as_ref() {
        let Some(annotation) = rest.type_annotation.as_ref() else {
            tuple.push(vec!["Unknown".into()]);
            return vue3_non_empty_runtime_tuple(tuple);
        };
        let runtime_types =
            infer_vue3_array_element_runtime_type(&annotation.type_annotation, analysis, mode)
                .or_else(|| {
                    vue3_non_empty_runtime_types(vue3_runtime_types_for_mode(
                        &annotation.type_annotation,
                        analysis,
                        mode,
                    ))
                })?;
        tuple.push(runtime_types);
    }
    vue3_non_empty_runtime_tuple(tuple)
}

pub(crate) fn vue3_tuple_declaration_for_mode(
    analysis: &Vue3ScriptSetupAnalysis,
    name: &str,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vue3RuntimeTypeTuple> {
    match mode {
        Vue3ArrayElementRuntimeMode::Props => {
            analysis.tuple_runtime_type_declarations.get(name).cloned()
        }
        Vue3ArrayElementRuntimeMode::DefineModel => analysis
            .define_model_tuple_runtime_type_declarations
            .get(name)
            .cloned(),
    }
}

pub(crate) fn vue3_tuple_declaration_for_context(
    context: &Vue27TypeContext,
    name: &str,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vue3RuntimeTypeTuple> {
    match mode {
        Vue3ArrayElementRuntimeMode::Props => {
            context.tuple_runtime_type_declarations.get(name).cloned()
        }
        Vue3ArrayElementRuntimeMode::DefineModel => context
            .define_model_tuple_runtime_type_declarations
            .get(name)
            .cloned(),
    }
}

pub(crate) fn vue3_parameter_tuple_declaration_for_mode(
    analysis: &Vue3ScriptSetupAnalysis,
    name: &str,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vue3RuntimeTypeTuple> {
    match mode {
        Vue3ArrayElementRuntimeMode::Props => analysis
            .parameter_tuple_runtime_type_declarations
            .get(name)
            .cloned(),
        Vue3ArrayElementRuntimeMode::DefineModel => analysis
            .define_model_parameter_tuple_runtime_type_declarations
            .get(name)
            .cloned(),
    }
}

pub(crate) fn vue3_parameter_tuple_declaration_for_context(
    context: &Vue27TypeContext,
    name: &str,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vue3RuntimeTypeTuple> {
    match mode {
        Vue3ArrayElementRuntimeMode::Props => context
            .parameter_tuple_runtime_type_declarations
            .get(name)
            .cloned(),
        Vue3ArrayElementRuntimeMode::DefineModel => context
            .define_model_parameter_tuple_runtime_type_declarations
            .get(name)
            .cloned(),
    }
}

pub(crate) fn vue3_constructor_parameter_tuple_declaration_for_mode(
    analysis: &Vue3ScriptSetupAnalysis,
    name: &str,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vue3RuntimeTypeTuple> {
    match mode {
        Vue3ArrayElementRuntimeMode::Props => analysis
            .constructor_parameter_tuple_runtime_type_declarations
            .get(name)
            .cloned(),
        Vue3ArrayElementRuntimeMode::DefineModel => analysis
            .define_model_constructor_parameter_tuple_runtime_type_declarations
            .get(name)
            .cloned(),
    }
}

pub(crate) fn vue3_constructor_parameter_tuple_declaration_for_context(
    context: &Vue27TypeContext,
    name: &str,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vue3RuntimeTypeTuple> {
    match mode {
        Vue3ArrayElementRuntimeMode::Props => context
            .constructor_parameter_tuple_runtime_type_declarations
            .get(name)
            .cloned(),
        Vue3ArrayElementRuntimeMode::DefineModel => context
            .define_model_constructor_parameter_tuple_runtime_type_declarations
            .get(name)
            .cloned(),
    }
}

pub(crate) fn vue3_return_type_declaration_for_mode(
    analysis: &Vue3ScriptSetupAnalysis,
    name: &str,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vec<String>> {
    match mode {
        Vue3ArrayElementRuntimeMode::Props => analysis
            .return_type_runtime_type_declarations
            .get(name)
            .cloned(),
        Vue3ArrayElementRuntimeMode::DefineModel => analysis
            .define_model_return_type_runtime_type_declarations
            .get(name)
            .cloned(),
    }
}

pub(crate) fn vue3_return_type_declaration_for_context(
    context: &Vue27TypeContext,
    name: &str,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vec<String>> {
    match mode {
        Vue3ArrayElementRuntimeMode::Props => context
            .return_type_runtime_type_declarations
            .get(name)
            .cloned(),
        Vue3ArrayElementRuntimeMode::DefineModel => context
            .define_model_return_type_runtime_type_declarations
            .get(name)
            .cloned(),
    }
}

pub(crate) fn merge_vue3_runtime_type_tuple(
    target: &mut Vue3RuntimeTypeTuple,
    source: Vue3RuntimeTypeTuple,
) {
    if target.len() < source.len() {
        target.resize_with(source.len(), Vec::new);
    }
    for (index, element) in source.into_iter().enumerate() {
        for runtime_type in element {
            push_unique(&mut target[index], &runtime_type);
        }
    }
}

pub(crate) fn merge_vue3_runtime_types(target: &mut Vec<String>, source: Vec<String>) {
    for runtime_type in source {
        push_unique(target, &runtime_type);
    }
}

pub(crate) fn infer_vue3_tuple_element_runtime_type(
    element: &TSTupleElement<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vec<String>> {
    match element {
        TSTupleElement::TSOptionalType(optional) => vue3_non_empty_runtime_types(
            vue3_runtime_types_for_mode(&optional.type_annotation, analysis, mode),
        ),
        TSTupleElement::TSRestType(rest) => {
            infer_vue3_array_element_runtime_type(&rest.type_annotation, analysis, mode).or_else(
                || {
                    vue3_non_empty_runtime_types(vue3_runtime_types_for_mode(
                        &rest.type_annotation,
                        analysis,
                        mode,
                    ))
                },
            )
        }
        TSTupleElement::TSNamedTupleMember(member) => {
            infer_vue3_tuple_element_runtime_type(&member.element_type, analysis, mode)
        }
        _ => {
            let ty = element.as_ts_type()?;
            vue3_non_empty_runtime_types(vue3_runtime_types_for_mode(ty, analysis, mode))
        }
    }
}

pub(crate) fn vue3_runtime_types_for_mode(
    node: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    mode: Vue3ArrayElementRuntimeMode,
) -> Vec<String> {
    match mode {
        Vue3ArrayElementRuntimeMode::Props => infer_vue3_runtime_type(node, analysis),
        Vue3ArrayElementRuntimeMode::DefineModel => {
            infer_vue3_define_model_runtime_type(node, analysis)
        }
    }
}

pub(crate) fn vue3_non_empty_runtime_types(types: Vec<String>) -> Option<Vec<String>> {
    if types.is_empty() {
        None
    } else {
        Some(types)
    }
}

pub(crate) fn vue3_non_empty_runtime_tuple(
    tuple: Vue3RuntimeTypeTuple,
) -> Option<Vue3RuntimeTypeTuple> {
    if tuple.is_empty() {
        None
    } else {
        Some(tuple)
    }
}

pub(crate) fn vue3_runtime_types_from_tuple(tuple: Vue3RuntimeTypeTuple) -> Option<Vec<String>> {
    let mut types = Vec::new();
    for element in tuple {
        for runtime_type in element {
            push_unique(&mut types, &runtime_type);
        }
    }
    vue3_non_empty_runtime_types(types)
}
