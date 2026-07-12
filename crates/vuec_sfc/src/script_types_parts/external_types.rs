pub(crate) fn extend_vue3_type_context_from_external_imports(
    filename: &str,
    source: &str,
    source_type: oxc_span::SourceType,
    context: &mut Vue27TypeContext,
    type_resolver: &Vue3TypeResolverContext,
) {
    let mut seen = BTreeSet::new();
    extend_vue3_type_context_from_external_imports_with_seen(
        filename,
        source,
        source_type,
        context,
        &mut seen,
        type_resolver,
    );
}

pub(crate) fn extend_vue3_type_context_from_external_imports_with_seen(
    filename: &str,
    source: &str,
    source_type: oxc_span::SourceType,
    context: &mut Vue27TypeContext,
    seen: &mut BTreeSet<PathBuf>,
    type_resolver: &Vue3TypeResolverContext,
) {
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, source, source_type)
        .with_options(oxc_parser::ParseOptions {
            parse_regular_expression: true,
            ..oxc_parser::ParseOptions::default()
        })
        .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return;
    }
    for statement in &parsed.program.body {
        let Statement::ImportDeclaration(import) = statement else {
            continue;
        };
        let import_source = import.source.value.as_str();
        let Some(specifiers) = &import.specifiers else {
            continue;
        };
        let Some(resolved) = resolve_vue3_type_import(filename, import_source, type_resolver)
        else {
            for specifier in specifiers {
                context
                    .unresolved_import_sources
                    .insert(import_specifier_local(specifier), import_source.to_string());
            }
            continue;
        };
        let Some(imported_context) =
            vue3_external_type_context_from_path(&resolved, &mut *seen, type_resolver)
        else {
            continue;
        };
        let normalized = normalize_path_string(&resolved);
        for specifier in specifiers {
            let local = import_specifier_local(specifier);
            let imported = import_specifier_imported(specifier).unwrap_or_else(|| "default".into());
            if imported == "*" {
                insert_vue3_external_namespace_types(
                    context,
                    &imported_context,
                    &local,
                    &normalized,
                );
                continue;
            }
            insert_vue3_external_type_alias(
                context,
                &imported_context,
                &imported,
                &local,
                &normalized,
            );
        }
    }
}

fn vue3_external_type_context_from_source_inner(
    source: &str,
    filename: &str,
    source_type: oxc_span::SourceType,
    seen: &mut BTreeSet<PathBuf>,
    type_resolver: &Vue3TypeResolverContext,
) -> Vue27TypeContext {
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, source, source_type)
        .with_options(oxc_parser::ParseOptions {
            parse_regular_expression: true,
            ..oxc_parser::ParseOptions::default()
        })
        .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return Vue27TypeContext::default();
    }
    let mut analysis = Vue3ScriptSetupAnalysis {
        type_filename: Some(filename.to_string()),
        type_seen: seen.clone(),
        type_resolver: type_resolver.clone(),
        ..Vue3ScriptSetupAnalysis::default()
    };
    let mut seed_context = Vue27TypeContext::default();
    extend_vue3_type_context_from_external_imports_with_seen(
        filename,
        source,
        source_type,
        &mut seed_context,
        seen,
        type_resolver,
    );
    analysis.declared_types = seed_context.declared_types;
    analysis.define_model_declared_types = seed_context.define_model_declared_types;
    analysis.type_query_declared_types = seed_context.type_query_declared_types;
    analysis.define_model_type_query_declared_types =
        seed_context.define_model_type_query_declared_types;
    analysis.keyof_type_query_declared_types = seed_context.keyof_type_query_declared_types;
    analysis.props_type_declarations = seed_context.props_type_declarations;
    analysis.keyof_runtime_type_declarations = seed_context.keyof_runtime_type_declarations;
    analysis.tuple_runtime_type_declarations = seed_context.tuple_runtime_type_declarations;
    analysis.define_model_tuple_runtime_type_declarations =
        seed_context.define_model_tuple_runtime_type_declarations;
    analysis.array_element_runtime_type_declarations =
        seed_context.array_element_runtime_type_declarations;
    analysis.define_model_array_element_runtime_type_declarations =
        seed_context.define_model_array_element_runtime_type_declarations;
    analysis.parameter_tuple_runtime_type_declarations =
        seed_context.parameter_tuple_runtime_type_declarations;
    analysis.define_model_parameter_tuple_runtime_type_declarations =
        seed_context.define_model_parameter_tuple_runtime_type_declarations;
    analysis.constructor_parameter_tuple_runtime_type_declarations =
        seed_context.constructor_parameter_tuple_runtime_type_declarations;
    analysis.define_model_constructor_parameter_tuple_runtime_type_declarations =
        seed_context.define_model_constructor_parameter_tuple_runtime_type_declarations;
    analysis.return_type_runtime_type_declarations =
        seed_context.return_type_runtime_type_declarations;
    analysis.define_model_return_type_runtime_type_declarations =
        seed_context.define_model_return_type_runtime_type_declarations;
    analysis.props_options_type_declarations = seed_context.props_options_type_declarations;
    analysis.return_type_props_options_declarations =
        seed_context.return_type_props_options_declarations;
    analysis.generic_type_aliases = seed_context.generic_type_aliases;
    analysis.string_literal_type_declarations = seed_context.string_literal_type_declarations;
    analysis.ordered_string_literal_type_declarations =
        seed_context.ordered_string_literal_type_declarations;
    analysis.emits_type_declarations = seed_context.emits_type_declarations;
    analysis.type_sources = seed_context.type_sources;
    analysis.type_direct_deps = seed_context.type_direct_deps;
    analysis.type_deps = seed_context.type_deps;
    analysis.unresolved_import_sources = seed_context.unresolved_import_sources;
    analysis.silent_unresolved_type_names = seed_context.silent_unresolved_type_names;
    collect_vue3_declared_types_from_statements(source, &parsed.program.body, &mut analysis);
    collect_vue3_declared_type_deps_from_statements(&parsed.program.body, &mut analysis);
    project_vue3_default_type_exports(source, &parsed.program.body, &mut analysis);
    seed_vue3_external_type_deps(filename, &mut analysis);
    let re_exported = project_vue3_type_re_exports(
        filename,
        &parsed.program.body,
        &mut analysis,
        seen,
        type_resolver,
    );
    project_vue3_exported_type_specifiers(&parsed.program.body, &mut analysis);
    let mut exported = vue3_exported_type_names(&parsed.program.body);
    exported.extend(re_exported);
    if !exported.is_empty() {
        analysis
            .declared_types
            .retain(|name, _| exported.contains(name));
        analysis
            .define_model_declared_types
            .retain(|name, _| exported.contains(name));
        analysis
            .type_query_declared_types
            .retain(|name, _| exported.contains(name));
        analysis
            .define_model_type_query_declared_types
            .retain(|name, _| exported.contains(name));
        analysis
            .keyof_type_query_declared_types
            .retain(|name, _| exported.contains(name));
        analysis
            .props_type_declarations
            .retain(|name, _| exported.contains(name));
        analysis
            .keyof_runtime_type_declarations
            .retain(|name, _| exported.contains(name));
        analysis
            .tuple_runtime_type_declarations
            .retain(|name, _| exported.contains(name));
        analysis
            .define_model_tuple_runtime_type_declarations
            .retain(|name, _| exported.contains(name));
        analysis
            .array_element_runtime_type_declarations
            .retain(|name, _| exported.contains(name));
        analysis
            .define_model_array_element_runtime_type_declarations
            .retain(|name, _| exported.contains(name));
        analysis
            .parameter_tuple_runtime_type_declarations
            .retain(|name, _| exported.contains(name));
        analysis
            .define_model_parameter_tuple_runtime_type_declarations
            .retain(|name, _| exported.contains(name));
        analysis
            .constructor_parameter_tuple_runtime_type_declarations
            .retain(|name, _| exported.contains(name));
        analysis
            .define_model_constructor_parameter_tuple_runtime_type_declarations
            .retain(|name, _| exported.contains(name));
        analysis
            .return_type_runtime_type_declarations
            .retain(|name, _| exported.contains(name));
        analysis
            .define_model_return_type_runtime_type_declarations
            .retain(|name, _| exported.contains(name));
        analysis
            .props_options_type_declarations
            .retain(|name, _| exported.contains(name));
        analysis
            .return_type_props_options_declarations
            .retain(|name, _| exported.contains(name));
        analysis
            .generic_type_aliases
            .retain(|name, _| exported.contains(name));
        analysis
            .string_literal_type_declarations
            .retain(|name, _| exported.contains(name));
        analysis
            .ordered_string_literal_type_declarations
            .retain(|name, _| exported.contains(name));
        analysis
            .emits_type_declarations
            .retain(|name, _| exported.contains(name));
        analysis
            .type_sources
            .retain(|name, _| exported.contains(name));
        analysis
            .type_direct_deps
            .retain(|name, _| exported.contains(name));
        analysis.type_deps.retain(|name, _| exported.contains(name));
        analysis
            .unresolved_import_sources
            .retain(|name, _| exported.contains(name));
        analysis
            .silent_unresolved_type_names
            .retain(|name| exported.contains(name));
    }
    Vue27TypeContext {
        declared_types: analysis.declared_types,
        define_model_declared_types: analysis.define_model_declared_types,
        type_query_declared_types: analysis.type_query_declared_types,
        define_model_type_query_declared_types: analysis.define_model_type_query_declared_types,
        keyof_type_query_declared_types: analysis.keyof_type_query_declared_types,
        props_type_declarations: analysis.props_type_declarations,
        keyof_runtime_type_declarations: analysis.keyof_runtime_type_declarations,
        tuple_runtime_type_declarations: analysis.tuple_runtime_type_declarations,
        define_model_tuple_runtime_type_declarations: analysis
            .define_model_tuple_runtime_type_declarations,
        array_element_runtime_type_declarations: analysis.array_element_runtime_type_declarations,
        define_model_array_element_runtime_type_declarations: analysis
            .define_model_array_element_runtime_type_declarations,
        parameter_tuple_runtime_type_declarations: analysis
            .parameter_tuple_runtime_type_declarations,
        define_model_parameter_tuple_runtime_type_declarations: analysis
            .define_model_parameter_tuple_runtime_type_declarations,
        constructor_parameter_tuple_runtime_type_declarations: analysis
            .constructor_parameter_tuple_runtime_type_declarations,
        define_model_constructor_parameter_tuple_runtime_type_declarations: analysis
            .define_model_constructor_parameter_tuple_runtime_type_declarations,
        return_type_runtime_type_declarations: analysis.return_type_runtime_type_declarations,
        define_model_return_type_runtime_type_declarations: analysis
            .define_model_return_type_runtime_type_declarations,
        props_options_type_declarations: analysis.props_options_type_declarations,
        return_type_props_options_declarations: analysis.return_type_props_options_declarations,
        generic_type_aliases: analysis.generic_type_aliases,
        string_literal_type_declarations: analysis.string_literal_type_declarations,
        ordered_string_literal_type_declarations: analysis.ordered_string_literal_type_declarations,
        emits_type_declarations: analysis.emits_type_declarations,
        type_sources: analysis.type_sources,
        type_direct_deps: analysis.type_direct_deps,
        type_deps: analysis.type_deps,
        unresolved_import_sources: analysis.unresolved_import_sources,
        silent_unresolved_type_names: analysis.silent_unresolved_type_names,
    }
}

pub(crate) fn seed_vue3_external_type_deps(filename: &str, analysis: &mut Vue3ScriptSetupAnalysis) {
    let dependency = normalize_path_string(Path::new(filename));
    let names = analysis
        .declared_types
        .keys()
        .chain(analysis.define_model_declared_types.keys())
        .chain(analysis.type_query_declared_types.keys())
        .chain(analysis.define_model_type_query_declared_types.keys())
        .chain(analysis.keyof_type_query_declared_types.keys())
        .chain(analysis.props_type_declarations.keys())
        .chain(analysis.keyof_runtime_type_declarations.keys())
        .chain(analysis.tuple_runtime_type_declarations.keys())
        .chain(analysis.define_model_tuple_runtime_type_declarations.keys())
        .chain(analysis.array_element_runtime_type_declarations.keys())
        .chain(
            analysis
                .define_model_array_element_runtime_type_declarations
                .keys(),
        )
        .chain(analysis.parameter_tuple_runtime_type_declarations.keys())
        .chain(
            analysis
                .define_model_parameter_tuple_runtime_type_declarations
                .keys(),
        )
        .chain(
            analysis
                .constructor_parameter_tuple_runtime_type_declarations
                .keys(),
        )
        .chain(
            analysis
                .define_model_constructor_parameter_tuple_runtime_type_declarations
                .keys(),
        )
        .chain(analysis.return_type_runtime_type_declarations.keys())
        .chain(
            analysis
                .define_model_return_type_runtime_type_declarations
                .keys(),
        )
        .chain(analysis.props_options_type_declarations.keys())
        .chain(analysis.return_type_props_options_declarations.keys())
        .chain(analysis.generic_type_aliases.keys())
        .chain(analysis.string_literal_type_declarations.keys())
        .chain(analysis.ordered_string_literal_type_declarations.keys())
        .chain(analysis.emits_type_declarations.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    for name in names {
        analysis
            .type_sources
            .insert(name.clone(), dependency.clone());
        analysis.type_direct_deps.entry(name.clone()).or_default();
        analysis
            .type_deps
            .entry(name)
            .or_default()
            .insert(dependency.clone());
    }
}

pub(crate) fn project_vue3_type_re_exports(
    filename: &str,
    statements: &[Statement<'_>],
    analysis: &mut Vue3ScriptSetupAnalysis,
    seen: &mut BTreeSet<PathBuf>,
    type_resolver: &Vue3TypeResolverContext,
) -> BTreeSet<String> {
    let mut exported_names = BTreeSet::new();
    for statement in statements {
        match statement {
            Statement::ExportNamedDeclaration(declaration) => {
                let Some(source) = declaration.source.as_ref() else {
                    continue;
                };
                let import_source = source.value.as_str();
                let Some(resolved_external) = vue3_external_type_context_from_source(
                    filename,
                    import_source,
                    seen,
                    type_resolver,
                ) else {
                    continue;
                };
                for specifier in &declaration.specifiers {
                    let Some(imported) = module_export_name(specifier.local()) else {
                        continue;
                    };
                    let Some(exported) = module_export_name(specifier.exported()) else {
                        continue;
                    };
                    insert_vue3_re_exported_type_alias(
                        analysis,
                        &resolved_external.context,
                        imported,
                        exported,
                        &resolved_external.dependency,
                    );
                    if vue3_type_context_has_name(&resolved_external.context, imported) {
                        exported_names.insert(exported.to_string());
                    }
                }
            }
            Statement::ExportAllDeclaration(declaration) => {
                let import_source = declaration.source.value.as_str();
                let Some(resolved_external) = vue3_external_type_context_from_source(
                    filename,
                    import_source,
                    seen,
                    type_resolver,
                ) else {
                    continue;
                };
                exported_names.extend(project_vue3_export_all_type_context(
                    analysis,
                    &resolved_external.context,
                    &resolved_external.dependency,
                ));
            }
            _ => {}
        }
    }
    exported_names
}

pub(crate) struct Vue3ResolvedExternalTypeContext {
    pub(crate) dependency: String,
    pub(crate) context: std::sync::Arc<Vue27TypeContext>,
}

pub(crate) fn vue3_external_type_context_from_source(
    filename: &str,
    source: &str,
    seen: &mut BTreeSet<PathBuf>,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<Vue3ResolvedExternalTypeContext> {
    let resolved = resolve_vue3_type_import(filename, source, type_resolver)?;
    let dependency = normalize_path_string(&resolved);
    let context = vue3_external_type_context_from_path(&resolved, seen, type_resolver)?;
    Some(Vue3ResolvedExternalTypeContext {
        dependency,
        context,
    })
}

pub(crate) fn vue3_external_type_context_from_path(
    path: &Path,
    seen: &mut BTreeSet<PathBuf>,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<std::sync::Arc<Vue27TypeContext>> {
    let identity = vue3_external_type_path_identity(path);
    if seen.len() >= VUE3_EXTERNAL_TYPE_MAX_ACTIVE_FILES || seen.contains(&identity) {
        type_resolver
            .external_type_session
            .record_context_failure();
        return None;
    }
    let cache_key =
        vue3_external_type_context_cache_key(path, &type_resolver.typescript_version);
    let failure_epoch = match type_resolver
        .external_type_session
        .begin_context_load(&cache_key)
    {
        Vue3ExternalTypeContextLoad::Ready(context) => return Some(context),
        Vue3ExternalTypeContextLoad::Failed => return None,
        Vue3ExternalTypeContextLoad::Start { failure_epoch } => failure_epoch,
    };
    seen.insert(identity.clone());
    let Some(source) = vue3_external_type_source_from_path(path, type_resolver) else {
        seen.remove(&identity);
        return type_resolver.external_type_session.finish_context_load(
            cache_key,
            None,
            failure_epoch,
        );
    };
    if !type_resolver
        .external_type_session
        .reserve_context_build_weight(&cache_key, source.source.len())
    {
        seen.remove(&identity);
        return None;
    }
    let normalized = normalize_path_string(path);
    let context = std::sync::Arc::new(vue3_external_type_context_from_source_inner(
        &source.source,
        &normalized,
        source.source_type,
        seen,
        type_resolver,
    ));
    seen.remove(&identity);
    type_resolver.external_type_session.finish_context_load(
        cache_key,
        Some(context),
        failure_epoch,
    )
}

pub(crate) struct Vue3ResolvedImportType {
    pub(crate) name: String,
    pub(crate) dependency: String,
    pub(crate) context: std::sync::Arc<Vue27TypeContext>,
}

pub(crate) fn vue3_resolve_import_type(
    import_type: &TSImportType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vue3ResolvedImportType> {
    let source = import_type.source.value.as_str();
    let name = vue3_import_type_qualifier_key(import_type.qualifier.as_ref()?);
    let filename = analysis.type_filename.as_deref()?;
    let resolved = resolve_vue3_type_import(filename, source, &analysis.type_resolver)?;
    let dependency = normalize_path_string(&resolved);
    let mut seen = analysis.type_seen.clone();
    let context =
        vue3_external_type_context_from_path(&resolved, &mut seen, &analysis.type_resolver)?;
    Some(Vue3ResolvedImportType {
        name,
        dependency,
        context,
    })
}

pub(crate) fn vue3_exported_type_names(statements: &[Statement<'_>]) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for statement in statements {
        match statement {
            Statement::ExportDefaultDeclaration(declaration)
                if vue3_default_export_may_be_type(declaration) =>
            {
                names.insert("default".into());
            }
            Statement::ExportNamedDeclaration(declaration) => {
                if let Some(declaration) = &declaration.declaration {
                    match declaration {
                        Declaration::TSInterfaceDeclaration(declaration) => {
                            names.insert(declaration.id.name.to_string());
                        }
                        Declaration::TSTypeAliasDeclaration(declaration) => {
                            names.insert(declaration.id.name.to_string());
                        }
                        Declaration::TSEnumDeclaration(declaration) => {
                            names.insert(declaration.id.name.to_string());
                        }
                        Declaration::FunctionDeclaration(function)
                            if vue3_function_has_return_projection(function) =>
                        {
                            if let Some(id) = &function.id {
                                names.insert(id.name.to_string());
                            }
                        }
                        Declaration::VariableDeclaration(declaration) if declaration.declare => {
                            for declarator in &declaration.declarations {
                                if let Some(name) = first_pattern_binding(&declarator.id) {
                                    names.insert(name);
                                }
                            }
                        }
                        Declaration::VariableDeclaration(declaration) => {
                            for declarator in &declaration.declarations {
                                if vue3_variable_declarator_has_type_projection(declarator) {
                                    if let Some(name) = first_pattern_binding(&declarator.id) {
                                        names.insert(name);
                                    }
                                }
                            }
                        }
                        Declaration::ClassDeclaration(declaration) => {
                            if let Some(id) = &declaration.id {
                                names.insert(id.name.to_string());
                            }
                        }
                        Declaration::TSModuleDeclaration(declaration) => {
                            names.extend(vue3_namespace_exported_type_names(declaration));
                        }
                        _ => {}
                    }
                }
                if declaration.source.is_none() {
                    for specifier in &declaration.specifiers {
                        if let Some(exported) = module_export_name(specifier.exported()) {
                            names.insert(exported.to_string());
                        }
                    }
                }
            }
            _ => {}
        }
    }
    names
}

pub(crate) fn vue3_default_export_may_be_type(declaration: &ExportDefaultDeclaration<'_>) -> bool {
    match &declaration.declaration {
        ExportDefaultDeclarationKind::TSInterfaceDeclaration(_)
        | ExportDefaultDeclarationKind::ClassDeclaration(_)
        | ExportDefaultDeclarationKind::Identifier(_) => true,
        ExportDefaultDeclarationKind::ObjectExpression(object) => {
            vue3_static_runtime_props_options_object_is_projectable(object)
        }
        ExportDefaultDeclarationKind::FunctionDeclaration(function) => {
            vue3_function_has_return_projection(function)
        }
        declaration => {
            vue3_default_export_function_value_has_return_projection(declaration)
                || vue3_default_export_static_runtime_props_options_is_projectable(declaration)
        }
    }
}

pub(crate) fn project_vue3_default_type_exports(
    source: &str,
    statements: &[Statement<'_>],
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    for statement in statements {
        let Statement::ExportDefaultDeclaration(declaration) = statement else {
            continue;
        };
        match &declaration.declaration {
            ExportDefaultDeclarationKind::TSInterfaceDeclaration(declaration) => {
                let name = declaration.id.name.to_string();
                let deps = collect_vue3_interface_type_deps(declaration, analysis);
                register_vue3_interface_declaration(source, declaration, analysis);
                insert_vue3_declared_type_deps(analysis, &name, deps);
                insert_vue3_local_type_alias(analysis, &name, "default");
            }
            ExportDefaultDeclarationKind::Identifier(identifier) => {
                insert_vue3_local_type_alias(analysis, identifier.name.as_str(), "default");
            }
            ExportDefaultDeclarationKind::FunctionDeclaration(function) => {
                if let Some(id) = &function.id {
                    let name = id.name.as_str();
                    register_vue3_function_return_projection(source, name, function, analysis);
                    if let Some(return_type) = function.return_type.as_ref() {
                        let deps =
                            collect_vue3_type_argument_deps(&return_type.type_annotation, analysis);
                        insert_vue3_declared_type_deps(analysis, name, deps);
                    }
                    insert_vue3_local_type_alias(analysis, name, "default");
                } else {
                    register_vue3_function_return_projection(source, "default", function, analysis);
                    if let Some(return_type) = function.return_type.as_ref() {
                        let deps =
                            collect_vue3_type_argument_deps(&return_type.type_annotation, analysis);
                        insert_vue3_declared_type_deps(analysis, "default", deps);
                    }
                }
            }
            declaration
                if vue3_default_export_function_value_has_return_projection(declaration) =>
            {
                if let Some(expression) = declaration.as_expression() {
                    register_vue3_function_value_expression_return_projection(
                        source, "default", expression, analysis,
                    );
                    if let Some(return_type) = vue3_function_value_return_type(expression) {
                        let deps = collect_vue3_type_argument_deps(return_type, analysis);
                        insert_vue3_declared_type_deps(analysis, "default", deps);
                    }
                }
            }
            declaration
                if vue3_default_export_static_runtime_props_options_is_projectable(declaration) =>
            {
                register_vue3_default_static_runtime_props_options(source, declaration, analysis);
            }
            ExportDefaultDeclarationKind::ClassDeclaration(class) => {
                if let Some(id) = &class.id {
                    register_vue3_class_type_name(analysis, id.name.as_str());
                    insert_vue3_local_type_alias(analysis, id.name.as_str(), "default");
                } else {
                    register_vue3_class_type_name(analysis, "default");
                }
            }
            _ => {}
        }
    }
}

pub(crate) fn project_vue3_exported_type_specifiers(
    statements: &[Statement<'_>],
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    for statement in statements {
        let Statement::ExportNamedDeclaration(declaration) = statement else {
            continue;
        };
        if declaration.source.is_some() {
            continue;
        }
        for specifier in &declaration.specifiers {
            let Some(local) = module_export_name(specifier.local()) else {
                continue;
            };
            let Some(exported) = module_export_name(specifier.exported()) else {
                continue;
            };
            if local == exported {
                continue;
            }
            insert_vue3_local_type_alias(analysis, local, exported);
        }
    }
}

pub(crate) fn project_vue3_namespace_declaration(
    source: &str,
    declaration: &TSModuleDeclaration<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    let Some(namespace) = vue3_ts_module_declaration_name(declaration) else {
        return;
    };
    project_vue3_namespace_declaration_with_prefix(source, declaration, &namespace, analysis);
}

pub(crate) fn project_vue3_namespace_declaration_with_prefix(
    source: &str,
    declaration: &TSModuleDeclaration<'_>,
    prefix: &str,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    let Some(body) = declaration.body.as_ref() else {
        return;
    };
    match body {
        TSModuleDeclarationBody::TSModuleBlock(block) => {
            let mut namespace_analysis = Vue3ScriptSetupAnalysis {
                declared_types: analysis.declared_types.clone(),
                define_model_declared_types: analysis.define_model_declared_types.clone(),
                type_query_declared_types: analysis.type_query_declared_types.clone(),
                define_model_type_query_declared_types: analysis
                    .define_model_type_query_declared_types
                    .clone(),
                keyof_type_query_declared_types: analysis.keyof_type_query_declared_types.clone(),
                props_type_declarations: analysis.props_type_declarations.clone(),
                keyof_runtime_type_declarations: analysis.keyof_runtime_type_declarations.clone(),
                tuple_runtime_type_declarations: analysis.tuple_runtime_type_declarations.clone(),
                define_model_tuple_runtime_type_declarations: analysis
                    .define_model_tuple_runtime_type_declarations
                    .clone(),
                array_element_runtime_type_declarations: analysis
                    .array_element_runtime_type_declarations
                    .clone(),
                define_model_array_element_runtime_type_declarations: analysis
                    .define_model_array_element_runtime_type_declarations
                    .clone(),
                parameter_tuple_runtime_type_declarations: analysis
                    .parameter_tuple_runtime_type_declarations
                    .clone(),
                define_model_parameter_tuple_runtime_type_declarations: analysis
                    .define_model_parameter_tuple_runtime_type_declarations
                    .clone(),
                constructor_parameter_tuple_runtime_type_declarations: analysis
                    .constructor_parameter_tuple_runtime_type_declarations
                    .clone(),
                define_model_constructor_parameter_tuple_runtime_type_declarations: analysis
                    .define_model_constructor_parameter_tuple_runtime_type_declarations
                    .clone(),
                return_type_runtime_type_declarations: analysis
                    .return_type_runtime_type_declarations
                    .clone(),
                define_model_return_type_runtime_type_declarations: analysis
                    .define_model_return_type_runtime_type_declarations
                    .clone(),
                props_options_type_declarations: analysis.props_options_type_declarations.clone(),
                return_type_props_options_declarations: analysis
                    .return_type_props_options_declarations
                    .clone(),
                generic_type_aliases: analysis.generic_type_aliases.clone(),
                string_literal_type_declarations: analysis.string_literal_type_declarations.clone(),
                ordered_string_literal_type_declarations: analysis
                    .ordered_string_literal_type_declarations
                    .clone(),
                emits_type_declarations: analysis.emits_type_declarations.clone(),
                type_sources: analysis.type_sources.clone(),
                type_direct_deps: analysis.type_direct_deps.clone(),
                type_deps: analysis.type_deps.clone(),
                unresolved_import_sources: analysis.unresolved_import_sources.clone(),
                silent_unresolved_type_names: analysis.silent_unresolved_type_names.clone(),
                type_filename: analysis.type_filename.clone(),
                type_seen: analysis.type_seen.clone(),
                type_resolver: analysis.type_resolver.clone(),
                ..Vue3ScriptSetupAnalysis::default()
            };
            seed_vue3_namespace_type_names(prefix, &block.body, &mut namespace_analysis);
            collect_vue3_declared_types_from_statements(
                source,
                &block.body,
                &mut namespace_analysis,
            );
            collect_vue3_declared_type_deps_from_statements(&block.body, &mut namespace_analysis);
            let names = if declaration.declare {
                vue3_declared_type_names_from_statements(&block.body)
            } else {
                vue3_exported_type_names(&block.body)
            };
            for name in names {
                let prefixed = format!("{prefix}.{name}");
                insert_vue3_type_alias_from_analysis(
                    analysis,
                    &namespace_analysis,
                    &name,
                    &prefixed,
                );
            }
        }
        TSModuleDeclarationBody::TSModuleDeclaration(nested) => {
            let Some(name) = vue3_ts_module_declaration_name(nested) else {
                return;
            };
            let prefix = format!("{prefix}.{name}");
            project_vue3_namespace_declaration_with_prefix(source, nested, &prefix, analysis);
        }
    }
}

pub(crate) fn seed_vue3_namespace_type_names(
    prefix: &str,
    statements: &[Statement<'_>],
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    for name in vue3_declared_type_names_from_statements(statements) {
        let prefixed = format!("{prefix}.{name}");
        for candidate in [name, prefixed] {
            analysis
                .declared_types
                .entry(candidate.clone())
                .or_insert_with(|| vec!["Object".into()]);
            analysis
                .define_model_declared_types
                .entry(candidate)
                .or_insert_with(|| vec!["Object".into()]);
        }
    }
}

pub(crate) fn seed_vue3_global_namespace_type_names(
    statements: &[Statement<'_>],
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    for statement in statements {
        match statement {
            Statement::TSGlobalDeclaration(global) => {
                seed_vue3_global_namespace_type_names(&global.body.body, analysis);
            }
            Statement::TSModuleDeclaration(declaration)
                if vue3_ts_module_declaration_is_global(declaration) =>
            {
                if let Some(body) = vue3_ts_module_declaration_block_body(declaration) {
                    seed_vue3_global_namespace_type_names(body, analysis);
                }
            }
            Statement::TSModuleDeclaration(declaration) => {
                seed_vue3_qualified_type_names(
                    vue3_namespace_declared_type_names(declaration),
                    analysis,
                );
            }
            Statement::ExportNamedDeclaration(declaration) => {
                if let Some(Declaration::TSModuleDeclaration(declaration)) =
                    declaration.declaration.as_ref()
                {
                    seed_vue3_qualified_type_names(
                        vue3_namespace_declared_type_names(declaration),
                        analysis,
                    );
                }
            }
            _ => {}
        }
    }
}

pub(crate) fn seed_vue3_qualified_type_names(
    names: BTreeSet<String>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    for name in names {
        analysis
            .declared_types
            .entry(name.clone())
            .or_insert_with(|| vec!["Object".into()]);
        analysis
            .define_model_declared_types
            .entry(name)
            .or_insert_with(|| vec!["Object".into()]);
    }
}

pub(crate) fn vue3_ts_module_declaration_name(
    declaration: &TSModuleDeclaration<'_>,
) -> Option<String> {
    match &declaration.id {
        TSModuleDeclarationName::Identifier(identifier) => Some(identifier.name.to_string()),
        TSModuleDeclarationName::StringLiteral(_) => None,
    }
}

pub(crate) fn vue3_ts_module_declaration_is_global(declaration: &TSModuleDeclaration<'_>) -> bool {
    vue3_ts_module_declaration_name(declaration).as_deref() == Some("global")
}

pub(crate) fn vue3_ts_module_declaration_block_body<'a>(
    declaration: &'a TSModuleDeclaration<'a>,
) -> Option<&'a [Statement<'a>]> {
    match declaration.body.as_ref()? {
        TSModuleDeclarationBody::TSModuleBlock(block) => Some(&block.body),
        TSModuleDeclarationBody::TSModuleDeclaration(_) => None,
    }
}

pub(crate) fn vue3_namespace_exported_type_names(
    declaration: &TSModuleDeclaration<'_>,
) -> BTreeSet<String> {
    let Some(namespace) = vue3_ts_module_declaration_name(declaration) else {
        return BTreeSet::new();
    };
    vue3_namespace_exported_type_names_with_prefix(declaration, &namespace)
}

pub(crate) fn vue3_namespace_declared_type_names(
    declaration: &TSModuleDeclaration<'_>,
) -> BTreeSet<String> {
    let Some(namespace) = vue3_ts_module_declaration_name(declaration) else {
        return BTreeSet::new();
    };
    vue3_namespace_declared_type_names_with_prefix(declaration, &namespace)
}

pub(crate) fn vue3_namespace_declared_type_names_with_prefix(
    declaration: &TSModuleDeclaration<'_>,
    prefix: &str,
) -> BTreeSet<String> {
    let Some(body) = declaration.body.as_ref() else {
        return BTreeSet::new();
    };
    match body {
        TSModuleDeclarationBody::TSModuleBlock(block) => {
            vue3_declared_type_names_from_statements(&block.body)
                .into_iter()
                .map(|name| format!("{prefix}.{name}"))
                .collect()
        }
        TSModuleDeclarationBody::TSModuleDeclaration(nested) => {
            let Some(name) = vue3_ts_module_declaration_name(nested) else {
                return BTreeSet::new();
            };
            let prefix = format!("{prefix}.{name}");
            vue3_namespace_declared_type_names_with_prefix(nested, &prefix)
        }
    }
}

pub(crate) fn vue3_namespace_exported_type_names_with_prefix(
    declaration: &TSModuleDeclaration<'_>,
    prefix: &str,
) -> BTreeSet<String> {
    let Some(body) = declaration.body.as_ref() else {
        return BTreeSet::new();
    };
    match body {
        TSModuleDeclarationBody::TSModuleBlock(block) => vue3_exported_type_names(&block.body)
            .into_iter()
            .map(|name| format!("{prefix}.{name}"))
            .collect(),
        TSModuleDeclarationBody::TSModuleDeclaration(nested) => {
            let Some(name) = vue3_ts_module_declaration_name(nested) else {
                return BTreeSet::new();
            };
            let prefix = format!("{prefix}.{name}");
            vue3_namespace_exported_type_names_with_prefix(nested, &prefix)
        }
    }
}
