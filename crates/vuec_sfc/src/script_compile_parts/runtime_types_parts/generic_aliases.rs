pub(crate) fn vue3_scoped_analysis_for_generic_type_alias(
    source: &str,
    reference: &TSTypeReference<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<(String, Vue3ScriptSetupAnalysis)> {
    let name = vue3_ts_type_name_key(&reference.type_name)?;
    let alias = analysis.generic_type_aliases.get(&name)?;
    let type_arguments = reference.type_arguments.as_ref()?;
    if type_arguments.params.is_empty() {
        return None;
    }
    let mut scoped_analysis = analysis.clone();
    scoped_analysis
        .declared_types
        .extend(alias.declared_types.clone());
    scoped_analysis
        .define_model_declared_types
        .extend(alias.define_model_declared_types.clone());
    scoped_analysis
        .type_query_declared_types
        .extend(alias.type_query_declared_types.clone());
    scoped_analysis
        .define_model_type_query_declared_types
        .extend(alias.define_model_type_query_declared_types.clone());
    scoped_analysis
        .keyof_type_query_declared_types
        .extend(alias.keyof_type_query_declared_types.clone());
    scoped_analysis
        .props_type_declarations
        .extend(alias.props_type_declarations.clone());
    scoped_analysis
        .keyof_runtime_type_declarations
        .extend(alias.keyof_runtime_type_declarations.clone());
    scoped_analysis
        .tuple_runtime_type_declarations
        .extend(alias.tuple_runtime_type_declarations.clone());
    scoped_analysis
        .define_model_tuple_runtime_type_declarations
        .extend(alias.define_model_tuple_runtime_type_declarations.clone());
    scoped_analysis
        .array_element_runtime_type_declarations
        .extend(alias.array_element_runtime_type_declarations.clone());
    scoped_analysis
        .define_model_array_element_runtime_type_declarations
        .extend(
            alias
                .define_model_array_element_runtime_type_declarations
                .clone(),
        );
    scoped_analysis
        .parameter_tuple_runtime_type_declarations
        .extend(alias.parameter_tuple_runtime_type_declarations.clone());
    scoped_analysis
        .define_model_parameter_tuple_runtime_type_declarations
        .extend(
            alias
                .define_model_parameter_tuple_runtime_type_declarations
                .clone(),
        );
    scoped_analysis
        .constructor_parameter_tuple_runtime_type_declarations
        .extend(
            alias
                .constructor_parameter_tuple_runtime_type_declarations
                .clone(),
        );
    scoped_analysis
        .define_model_constructor_parameter_tuple_runtime_type_declarations
        .extend(
            alias
                .define_model_constructor_parameter_tuple_runtime_type_declarations
                .clone(),
        );
    scoped_analysis
        .return_type_runtime_type_declarations
        .extend(alias.return_type_runtime_type_declarations.clone());
    scoped_analysis
        .define_model_return_type_runtime_type_declarations
        .extend(
            alias
                .define_model_return_type_runtime_type_declarations
                .clone(),
        );
    scoped_analysis
        .props_options_type_declarations
        .extend(alias.props_options_type_declarations.clone());
    scoped_analysis
        .return_type_props_options_declarations
        .extend(alias.return_type_props_options_declarations.clone());
    scoped_analysis
        .string_literal_type_declarations
        .extend(alias.string_literal_type_declarations.clone());
    scoped_analysis
        .ordered_string_literal_type_declarations
        .extend(alias.ordered_string_literal_type_declarations.clone());
    scoped_analysis
        .unresolved_import_sources
        .extend(alias.unresolved_import_sources.clone());
    scoped_analysis
        .silent_unresolved_type_names
        .extend(alias.silent_unresolved_type_names.clone());
    scoped_analysis.generic_type_aliases.remove(&name);
    scoped_analysis
        .generic_type_parameter_names
        .extend(alias.params.iter().cloned());
    for (index, param) in alias.params.iter().enumerate() {
        let Some(argument) = type_arguments.params.get(index) else {
            continue;
        };
        if let Some(props) = vue3_resolve_props_type(source, argument, analysis) {
            scoped_analysis
                .props_type_declarations
                .insert(param.clone(), props);
        }
        if let Some(props_options) = vue3_resolve_props_options_type(source, argument, analysis) {
            scoped_analysis
                .props_options_type_declarations
                .insert(param.clone(), props_options);
        }
        if let Some(keys) = vue3_resolve_string_type_keys(argument, analysis) {
            scoped_analysis
                .string_literal_type_declarations
                .insert(param.clone(), keys);
        }
        if let Some(keys) = vue3_resolve_ordered_string_type_keys(argument, analysis) {
            scoped_analysis
                .ordered_string_literal_type_declarations
                .insert(param.clone(), keys);
        }
        if let Some(types) = infer_vue3_keyof_runtime_type(argument, analysis) {
            scoped_analysis
                .keyof_runtime_type_declarations
                .insert(param.clone(), types);
        }
        if let Some(tuple) =
            infer_vue3_tuple_runtime_type(argument, analysis, Vue3ArrayElementRuntimeMode::Props)
        {
            scoped_analysis
                .tuple_runtime_type_declarations
                .insert(param.clone(), tuple);
        }
        if let Some(tuple) = infer_vue3_tuple_runtime_type(
            argument,
            analysis,
            Vue3ArrayElementRuntimeMode::DefineModel,
        ) {
            scoped_analysis
                .define_model_tuple_runtime_type_declarations
                .insert(param.clone(), tuple);
        }
        if let Some(types) = infer_vue3_array_element_runtime_type(
            argument,
            analysis,
            Vue3ArrayElementRuntimeMode::Props,
        ) {
            scoped_analysis
                .array_element_runtime_type_declarations
                .insert(param.clone(), types);
        }
        if let Some(types) = infer_vue3_array_element_runtime_type(
            argument,
            analysis,
            Vue3ArrayElementRuntimeMode::DefineModel,
        ) {
            scoped_analysis
                .define_model_array_element_runtime_type_declarations
                .insert(param.clone(), types);
        }
        if let Some(tuple) = infer_vue3_function_parameter_tuple_runtime_type(
            argument,
            analysis,
            Vue3ArrayElementRuntimeMode::Props,
        ) {
            scoped_analysis
                .parameter_tuple_runtime_type_declarations
                .insert(param.clone(), tuple);
        }
        if let Some(tuple) = infer_vue3_function_parameter_tuple_runtime_type(
            argument,
            analysis,
            Vue3ArrayElementRuntimeMode::DefineModel,
        ) {
            scoped_analysis
                .define_model_parameter_tuple_runtime_type_declarations
                .insert(param.clone(), tuple);
        }
        if let Some(tuple) = infer_vue3_constructor_parameter_tuple_runtime_type(
            argument,
            analysis,
            Vue3ArrayElementRuntimeMode::Props,
        ) {
            scoped_analysis
                .constructor_parameter_tuple_runtime_type_declarations
                .insert(param.clone(), tuple);
        }
        if let Some(tuple) = infer_vue3_constructor_parameter_tuple_runtime_type(
            argument,
            analysis,
            Vue3ArrayElementRuntimeMode::DefineModel,
        ) {
            scoped_analysis
                .define_model_constructor_parameter_tuple_runtime_type_declarations
                .insert(param.clone(), tuple);
        }
        if let Some(types) =
            infer_vue3_return_runtime_type(argument, analysis, Vue3ArrayElementRuntimeMode::Props)
        {
            scoped_analysis
                .return_type_runtime_type_declarations
                .insert(param.clone(), types);
        }
        if let Some(types) = infer_vue3_return_runtime_type(
            argument,
            analysis,
            Vue3ArrayElementRuntimeMode::DefineModel,
        ) {
            scoped_analysis
                .define_model_return_type_runtime_type_declarations
                .insert(param.clone(), types);
        }
        scoped_analysis
            .declared_types
            .insert(param.clone(), infer_vue3_runtime_type(argument, analysis));
        scoped_analysis.define_model_declared_types.insert(
            param.clone(),
            infer_vue3_define_model_runtime_type(argument, analysis),
        );
        scoped_analysis
            .type_query_declared_types
            .insert(param.clone(), infer_vue3_runtime_type(argument, analysis));
        scoped_analysis
            .define_model_type_query_declared_types
            .insert(
                param.clone(),
                infer_vue3_define_model_runtime_type(argument, analysis),
            );
        if let Some(types) = infer_vue3_keyof_runtime_type(argument, analysis) {
            scoped_analysis
                .keyof_type_query_declared_types
                .insert(param.clone(), types);
        }
    }
    Some((alias.source.clone(), scoped_analysis))
}

pub(crate) fn vue3_resolve_generic_props_type_alias(
    source: &str,
    reference: &TSTypeReference<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vue27TypeMembers> {
    let (alias_source, scoped_analysis) =
        vue3_scoped_analysis_for_generic_type_alias(source, reference, analysis)?;
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
        match statement {
            Statement::TSTypeAliasDeclaration(declaration) => {
                return vue3_resolve_props_type(
                    alias_source.as_str(),
                    &declaration.type_annotation,
                    &scoped_analysis,
                );
            }
            Statement::TSInterfaceDeclaration(declaration) => {
                return Some(vue3_type_members_from_interface(
                    alias_source.as_str(),
                    declaration,
                    &scoped_analysis,
                ));
            }
            _ => {}
        }
    }
    None
}

pub(crate) fn infer_vue3_generic_type_alias_runtime_type(
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
        match statement {
            Statement::TSTypeAliasDeclaration(declaration) => {
                return Some(infer_vue3_runtime_type(
                    &declaration.type_annotation,
                    &scoped_analysis,
                ));
            }
            Statement::TSInterfaceDeclaration(declaration) => {
                return Some(infer_vue3_runtime_type_from_interface_declarations(&[
                    declaration,
                ]));
            }
            _ => {}
        }
    }
    None
}

pub(crate) fn infer_vue3_generic_define_model_runtime_type(
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
        match statement {
            Statement::TSTypeAliasDeclaration(declaration) => {
                return Some(infer_vue3_define_model_runtime_type(
                    &declaration.type_annotation,
                    &scoped_analysis,
                ));
            }
            Statement::TSInterfaceDeclaration(declaration) => {
                return Some(infer_vue3_runtime_type_from_interface_declarations(&[
                    declaration,
                ]));
            }
            _ => {}
        }
    }
    None
}
