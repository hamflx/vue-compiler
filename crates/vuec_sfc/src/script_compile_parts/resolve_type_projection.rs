pub(crate) fn vue3_resolve_type_projection(
    descriptor: &SfcDescriptor,
    options: &SfcScriptCompileOptions,
) -> Vue3ResolveTypeResult {
    let Some(script_setup) = descriptor.script_setup.as_ref() else {
        return Vue3ResolveTypeResult {
            errors: vec!["script setup block is missing".into()],
            ..Vue3ResolveTypeResult::default()
        };
    };
    let source = script_setup.content.as_str();
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        source,
        script_source_type_from_attrs(&script_setup.attrs),
    )
    .with_options(oxc_parser::ParseOptions {
        parse_regular_expression: true,
        ..oxc_parser::ParseOptions::default()
    })
    .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return Vue3ResolveTypeResult {
            errors: parsed.errors.iter().map(ToString::to_string).collect(),
            ..Vue3ResolveTypeResult::default()
        };
    }

    let type_resolver = vue3_type_resolver_context_for_filename(&descriptor.filename);
    let normal_type_context =
        vue3_normal_script_type_context(descriptor, &options.global_type_files, &type_resolver);
    let normal_user_imports = vue3_normal_script_user_imports(descriptor);
    let mut type_context = normal_type_context.clone();
    extend_vue3_type_context_from_external_imports(
        &descriptor.filename,
        source,
        script_source_type_from_attrs(&script_setup.attrs),
        &mut type_context,
        &type_resolver,
    );
    let mut analysis = Vue3ScriptSetupAnalysis {
        vue_import_aliases: normal_user_imports.vue_aliases(),
        declared_types: type_context.declared_types,
        define_model_declared_types: type_context.define_model_declared_types,
        type_query_declared_types: type_context.type_query_declared_types,
        define_model_type_query_declared_types: type_context.define_model_type_query_declared_types,
        keyof_type_query_declared_types: type_context.keyof_type_query_declared_types,
        props_type_declarations: type_context.props_type_declarations,
        keyof_runtime_type_declarations: type_context.keyof_runtime_type_declarations,
        tuple_runtime_type_declarations: type_context.tuple_runtime_type_declarations,
        define_model_tuple_runtime_type_declarations: type_context
            .define_model_tuple_runtime_type_declarations,
        array_element_runtime_type_declarations: type_context
            .array_element_runtime_type_declarations,
        define_model_array_element_runtime_type_declarations: type_context
            .define_model_array_element_runtime_type_declarations,
        parameter_tuple_runtime_type_declarations: type_context
            .parameter_tuple_runtime_type_declarations,
        define_model_parameter_tuple_runtime_type_declarations: type_context
            .define_model_parameter_tuple_runtime_type_declarations,
        constructor_parameter_tuple_runtime_type_declarations: type_context
            .constructor_parameter_tuple_runtime_type_declarations,
        define_model_constructor_parameter_tuple_runtime_type_declarations: type_context
            .define_model_constructor_parameter_tuple_runtime_type_declarations,
        return_type_runtime_type_declarations: type_context.return_type_runtime_type_declarations,
        define_model_return_type_runtime_type_declarations: type_context
            .define_model_return_type_runtime_type_declarations,
        props_options_type_declarations: type_context.props_options_type_declarations,
        return_type_props_options_declarations: type_context.return_type_props_options_declarations,
        generic_type_aliases: type_context.generic_type_aliases,
        string_literal_type_declarations: type_context.string_literal_type_declarations,
        ordered_string_literal_type_declarations: type_context
            .ordered_string_literal_type_declarations,
        emits_type_declarations: type_context.emits_type_declarations,
        type_sources: type_context.type_sources,
        type_direct_deps: type_context.type_direct_deps,
        type_deps: type_context.type_deps,
        unresolved_import_sources: type_context.unresolved_import_sources,
        silent_unresolved_type_names: type_context.silent_unresolved_type_names,
        type_filename: Some(descriptor.filename.clone()),
        type_resolver,
        ..Vue3ScriptSetupAnalysis::default()
    };
    collect_vue3_setup_import_aliases(&parsed.program.body, &normal_user_imports, &mut analysis);
    collect_vue3_declared_types_from_statements(source, &parsed.program.body, &mut analysis);
    collect_vue3_declared_type_deps_from_statements(&parsed.program.body, &mut analysis);

    let Some(type_argument) = vue3_first_define_props_type_argument(&parsed.program.body) else {
        return Vue3ResolveTypeResult {
            errors: vec!["defineProps() type argument is missing".into()],
            ..Vue3ResolveTypeResult::default()
        };
    };

    record_vue3_type_argument_deps(type_argument, &mut analysis);
    let calls = vue3_resolve_type_call_placeholders(source, type_argument, &analysis);
    let mut errors = Vec::new();
    let type_members = if calls.is_empty() {
        vue3_resolve_props_type_with_mode(
            source,
            type_argument,
            &analysis,
            Vue3PropsTypeResolveMode::Consumed,
        )
    } else {
        vue3_resolve_props_type(source, type_argument, &analysis)
    };
    let mut props = BTreeMap::new();
    let mut raw_props = BTreeMap::new();
    if let Some(type_members) = type_members {
        errors.extend(type_members.errors);
        for member in type_members.members {
            props.insert(member.key.clone(), member.types.clone());
            raw_props.insert(
                member.key.clone(),
                Vue3ResolveTypeRawProp {
                    types: member.types,
                    required: member.required,
                    optional: !member.required,
                    is_method: member.is_method,
                    type_annotation_source: member.type_annotation_source,
                    member_source: member.member_source,
                },
            );
        }
    }
    let raw = Vue3ResolveTypeRaw {
        props: raw_props,
        calls: calls.clone(),
    };
    Vue3ResolveTypeResult {
        props,
        calls,
        deps: analysis.deps.to_vec(),
        raw,
        errors,
    }
}

pub(crate) fn vue3_first_define_props_type_argument<'a>(
    statements: &'a [Statement<'a>],
) -> Option<&'a TSType<'a>> {
    for statement in statements {
        let Statement::ExpressionStatement(statement) = statement else {
            continue;
        };
        let Expression::CallExpression(call) = unwrap_vue3_ts_expression(&statement.expression)
        else {
            continue;
        };
        if !is_call_named(call, "defineProps") {
            continue;
        }
        return call
            .type_arguments
            .as_ref()
            .and_then(|arguments| arguments.params.first());
    }
    None
}

pub(crate) fn vue3_resolve_type_call_placeholders(
    source: &str,
    type_argument: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Vec<Value> {
    let count = vue3_resolve_type_call_count(source, type_argument, analysis);
    (0..count).map(|_| json!({})).collect()
}

pub(crate) fn vue3_resolve_type_call_count(
    source: &str,
    type_argument: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> usize {
    match type_argument {
        TSType::TSFunctionType(_) => 1,
        TSType::TSTypeLiteral(literal) => literal
            .members
            .iter()
            .filter(|member| matches!(member, TSSignature::TSCallSignatureDeclaration(_)))
            .count(),
        TSType::TSTypeReference(_)
        | TSType::TSImportType(_)
        | TSType::TSIntersectionType(_)
        | TSType::TSParenthesizedType(_) => {
            vue3_resolve_emits_type(source, type_argument, analysis)
                .filter(|emits| emits.syntax.has_call_signature)
                .map(|emits| emits.call_count.max(1))
                .unwrap_or_default()
        }
        _ => 0,
    }
}
