pub(crate) fn extend_vue3_type_context_from_external_imports(
    filename: &str,
    source: &str,
    source_type: oxc_span::SourceType,
    context: &mut Vue27TypeContext,
    type_resolver: &Vue3TypeResolverContext,
) -> bool {
    let mut seen = BTreeSet::new();
    let mut namespace_budget = Vue3NamespaceProjectionBudget::default();
    extend_vue3_type_context_from_external_imports_with_seen(
        filename,
        source,
        source_type,
        context,
        &mut seen,
        type_resolver,
        &mut namespace_budget,
    )
}

pub(crate) fn extend_vue3_type_context_from_external_imports_with_seen(
    filename: &str,
    source: &str,
    source_type: oxc_span::SourceType,
    context: &mut Vue27TypeContext,
    seen: &mut BTreeSet<PathBuf>,
    type_resolver: &Vue3TypeResolverContext,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> bool {
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, source, source_type)
        .with_options(oxc_parser::ParseOptions {
            parse_regular_expression: true,
            ..oxc_parser::ParseOptions::default()
        })
        .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return false;
    }
    if !namespace_budget.reserve(vue3_external_type_context_cache_cost(context)) {
        return false;
    }
    let mut working_context = context.clone();
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
            if !clear_vue3_failed_import_bindings(
                &mut working_context,
                specifiers,
                Some(import_source),
                namespace_budget,
            ) {
                return false;
            }
            continue;
        };
        let Some(imported_context) =
            vue3_external_type_context_from_path(&resolved, &mut *seen, type_resolver)
        else {
            if !clear_vue3_failed_import_bindings(
                &mut working_context,
                specifiers,
                None,
                namespace_budget,
            ) {
                return false;
            }
            continue;
        };
        let normalized = normalize_path_string(&resolved);
        for specifier in specifiers {
            let local = import_specifier_local_name(specifier);
            let imported = import_specifier_imported_name(specifier).unwrap_or("default");
            if imported == "*" {
                if !insert_vue3_external_namespace_types(
                    &mut working_context,
                    &imported_context,
                    local,
                    &normalized,
                    namespace_budget,
                ) {
                    return false;
                }
                continue;
            }
            if !insert_vue3_external_type_alias_and_namespace_members(
                &mut working_context,
                &imported_context,
                imported,
                local,
                &normalized,
                namespace_budget,
            ) {
                return false;
            }
        }
    }
    *context = working_context;
    true
}

fn clear_vue3_failed_import_bindings(
    context: &mut Vue27TypeContext,
    specifiers: &[ImportDeclarationSpecifier<'_>],
    unresolved_source: Option<&str>,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> bool {
    for specifier in specifiers {
        let local = import_specifier_local_name(specifier);
        if !reserve_vue3_external_import_binding_clear(context, local, namespace_budget) {
            return false;
        }
        if let Some(source) = unresolved_source {
            let metadata_work = local
                .len()
                .saturating_add(source.len())
                .saturating_add(64);
            if !namespace_budget.reserve(metadata_work) {
                return false;
            }
        }
    }
    for specifier in specifiers {
        let local = import_specifier_local_name(specifier);
        clear_vue3_external_import_binding(context, local);
        if let Some(source) = unresolved_source {
            context
                .unresolved_import_sources
                .insert(local.to_string(), source.to_string());
        }
    }
    true
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
    let mut namespace_budget = Vue3NamespaceProjectionBudget::default();
    if !extend_vue3_type_context_from_external_imports_with_seen(
        filename,
        source,
        source_type,
        &mut seed_context,
        seen,
        type_resolver,
        &mut namespace_budget,
    ) {
        return Vue27TypeContext::default();
    }
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
    collect_vue3_declared_types_from_statements_with_namespace_budget(
        source,
        &parsed.program.body,
        source_type.is_typescript_definition(),
        0,
        &mut analysis,
        &mut namespace_budget,
    );
    if namespace_budget.is_exhausted() || analysis.type_dependency_work_exhausted {
        return Vue27TypeContext::default();
    }
    collect_vue3_declared_type_deps_from_statements(&parsed.program.body, &mut analysis);
    if analysis.type_dependency_work_exhausted {
        return Vue27TypeContext::default();
    }
    project_vue3_default_type_exports(source, &parsed.program.body, &mut analysis);
    finalize_vue3_local_generic_alias_scopes(&mut analysis);
    seed_vue3_external_type_deps(filename, &mut analysis);
    let Some(re_exported) = project_vue3_type_re_exports(
        filename,
        &parsed.program.body,
        &mut analysis,
        seen,
        type_resolver,
        &mut namespace_budget,
    ) else {
        return Vue27TypeContext::default();
    };
    if project_vue3_exported_type_specifiers_with_budget(
        &parsed.program.body,
        &mut analysis,
        &mut namespace_budget,
    )
    .is_none()
    {
        return Vue27TypeContext::default();
    }
    let Some(mut exported) =
        vue3_exported_type_names_with_budget(&parsed.program.body, &mut namespace_budget)
    else {
        return Vue27TypeContext::default();
    };
    let Some(namespace_specifier_names) =
        project_vue3_exported_namespace_specifiers_with_budget(
            &parsed.program.body,
            source_type.is_typescript_definition(),
            &mut analysis,
            &mut namespace_budget,
        )
    else {
        return Vue27TypeContext::default();
    };
    exported.extend(namespace_specifier_names);
    exported.extend(re_exported);
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
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<BTreeSet<String>> {
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
                    let names = insert_vue3_re_exported_type_alias_and_namespace_members(
                        analysis,
                        &resolved_external.context,
                        imported,
                        exported,
                        &resolved_external.dependency,
                        namespace_budget,
                    )?;
                    exported_names.extend(names);
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
                    let names = project_vue3_export_all_type_context(
                        analysis,
                        &resolved_external.context,
                        &resolved_external.dependency,
                        namespace_budget,
                    )?;
                    exported_names.extend(names);
            }
            _ => {}
        }
    }
    Some(exported_names)
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
    let mut owner = match type_resolver
        .external_type_session
        .begin_context_load(&cache_key)
    {
        Vue3ExternalTypeContextLoad::Ready(context) => return Some(context),
        Vue3ExternalTypeContextLoad::Wait(waiter) => return waiter.wait(),
        Vue3ExternalTypeContextLoad::Failed => return None,
        Vue3ExternalTypeContextLoad::Start(owner) => owner,
    };
    seen.insert(identity.clone());
    let Some(source) = vue3_external_type_source_from_path(path, type_resolver) else {
        seen.remove(&identity);
        return owner.complete(None);
    };
    if !owner.reserve_build_weight(source.source.len()) {
        seen.remove(&identity);
        return None;
    }
    let normalized = normalize_path_string(path);
    let context = vue3_external_type_context_from_source_inner(
        &source.source,
        &normalized,
        source.source_type,
        seen,
        type_resolver,
    );
    seen.remove(&identity);
    owner.complete(Some(context))
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

fn vue3_exported_type_names_with_budget(
    statements: &[Statement<'_>],
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<BTreeSet<String>> {
    let mut names = BTreeSet::new();
    for statement in statements {
        match statement {
            Statement::ExportDefaultDeclaration(declaration)
                if vue3_default_export_may_be_type(declaration) =>
            {
                insert_vue3_declared_type_name_with_budget(
                    &mut names,
                    "default",
                    namespace_budget,
                )?;
            }
            Statement::ExportNamedDeclaration(declaration) => {
                if let Some(declaration) = &declaration.declaration {
                    match declaration {
                        Declaration::TSInterfaceDeclaration(declaration) => {
                            insert_vue3_declared_type_name_with_budget(
                                &mut names,
                                declaration.id.name.as_str(),
                                namespace_budget,
                            )?;
                        }
                        Declaration::TSTypeAliasDeclaration(declaration) => {
                            insert_vue3_declared_type_name_with_budget(
                                &mut names,
                                declaration.id.name.as_str(),
                                namespace_budget,
                            )?;
                        }
                        Declaration::TSEnumDeclaration(declaration) => {
                            insert_vue3_declared_type_name_with_budget(
                                &mut names,
                                declaration.id.name.as_str(),
                                namespace_budget,
                            )?;
                        }
                        Declaration::FunctionDeclaration(function)
                            if vue3_function_has_return_projection(function) =>
                        {
                            if let Some(id) = &function.id {
                                insert_vue3_declared_type_name_with_budget(
                                    &mut names,
                                    id.name.as_str(),
                                    namespace_budget,
                                )?;
                            }
                        }
                        Declaration::VariableDeclaration(declaration) if declaration.declare => {
                            for declarator in &declaration.declarations {
                                if let Some(name) = first_pattern_binding_name(&declarator.id) {
                                    insert_vue3_declared_type_name_with_budget(
                                        &mut names,
                                        name,
                                        namespace_budget,
                                    )?;
                                }
                            }
                        }
                        Declaration::VariableDeclaration(declaration) => {
                            for declarator in &declaration.declarations {
                                if vue3_variable_declarator_has_type_projection(declarator) {
                                    if let Some(name) = first_pattern_binding_name(&declarator.id) {
                                        insert_vue3_declared_type_name_with_budget(
                                            &mut names,
                                            name,
                                            namespace_budget,
                                        )?;
                                    }
                                }
                            }
                        }
                        Declaration::ClassDeclaration(declaration) => {
                            if let Some(id) = &declaration.id {
                                insert_vue3_declared_type_name_with_budget(
                                    &mut names,
                                    id.name.as_str(),
                                    namespace_budget,
                                )?;
                            }
                        }
                        Declaration::TSModuleDeclaration(declaration) => {
                            names.extend(vue3_namespace_exported_type_names_with_budget(
                                declaration,
                                namespace_budget,
                            )?);
                        }
                        _ => {}
                    }
                }
                if declaration.source.is_none() {
                    for specifier in &declaration.specifiers {
                        if let Some(exported) = module_export_name(specifier.exported()) {
                            insert_vue3_declared_type_name_with_budget(
                                &mut names,
                                exported,
                                namespace_budget,
                            )?;
                        }
                    }
                }
            }
            _ => {}
        }
    }
    Some(names)
}

fn project_vue3_exported_namespace_specifiers_with_budget(
    statements: &[Statement<'_>],
    ambient: bool,
    analysis: &mut Vue3ScriptSetupAnalysis,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<BTreeSet<String>> {
    let mut namespace_members = BTreeMap::<String, BTreeSet<String>>::new();
    let mut exported_names = BTreeSet::new();
    for statement in statements {
        let Some(declaration) = vue3_namespace_declaration_from_statement(statement) else {
            continue;
        };
        let Some(namespace) = vue3_ts_module_declaration_name_ref(declaration) else {
            continue;
        };
        let names = vue3_namespace_visible_type_names_with_budget(
            declaration,
            ambient,
            namespace_budget,
        )?;
        if matches!(statement, Statement::ExportNamedDeclaration(_)) {
            for name in &names {
                insert_vue3_declared_type_name_with_budget(
                    &mut exported_names,
                    name,
                    namespace_budget,
                )?;
            }
        }
        if !namespace_members.contains_key(namespace) {
            if !namespace_budget.reserve(namespace.len().saturating_add(1)) {
                return None;
            }
            namespace_members.insert(namespace.to_string(), BTreeSet::new());
        }
        namespace_members
            .get_mut(namespace)
            .expect("namespace entry was inserted")
            .extend(names);
    }

    let mut aliases = BTreeSet::new();
    for statement in statements {
        let Statement::ExportNamedDeclaration(export) = statement else {
            continue;
        };
        if export.source.is_some() {
            continue;
        }
        for specifier in &export.specifiers {
            let Some(local) = module_export_name(specifier.local()) else {
                continue;
            };
            if !namespace_members.contains_key(local) {
                continue;
            }
            let Some(exported) = module_export_name(specifier.exported()) else {
                continue;
            };
            if !namespace_budget.reserve(
                local
                    .len()
                    .saturating_add(exported.len())
                    .saturating_add(1),
            ) {
                return None;
            }
            aliases.insert((local.to_string(), exported.to_string()));
        }
    }

    let mut projections = BTreeSet::new();
    for (local, exported) in aliases {
        let Some(source_names) = namespace_members.get(&local) else {
            continue;
        };
        for source_name in source_names {
            let Some(member_name) = source_name
                .strip_prefix(&local)
                .and_then(|suffix| suffix.strip_prefix('.'))
            else {
                continue;
            };
            let target_name = if local == exported {
                if !namespace_budget.reserve(source_name.len().saturating_add(1)) {
                    return None;
                }
                source_name.clone()
            } else {
                reserve_vue3_qualified_namespace_name(
                    &exported,
                    member_name,
                    namespace_budget,
                )?
            };
            if source_name != &target_name {
                if !namespace_budget.reserve(
                    source_name
                        .len()
                        .saturating_add(target_name.len())
                        .saturating_add(2),
                ) {
                    return None;
                }
                projections.insert((source_name.clone(), target_name.clone()));
            }
            exported_names.insert(target_name);
        }
    }

    let mut source_projection = Vue3ScriptSetupAnalysis::default();
    if !namespace_budget.reserve(
        projections
            .len()
            .saturating_mul(std::mem::size_of::<&String>()),
    ) {
        return None;
    }
    let source_names = projections
        .iter()
        .map(|(source_name, _)| source_name)
        .collect::<BTreeSet<_>>();
    for source_name in source_names {
        sync_vue3_namespace_type_projection(
            &mut source_projection,
            analysis,
            source_name,
            source_name,
            namespace_budget,
        )?;
    }
    for (source_name, target_name) in projections {
        sync_vue3_namespace_type_projection(
            analysis,
            &source_projection,
            &source_name,
            &target_name,
            namespace_budget,
        )?;
    }
    Some(exported_names)
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

fn project_vue3_exported_type_specifiers_with_budget(
    statements: &[Statement<'_>],
    analysis: &mut Vue3ScriptSetupAnalysis,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<()> {
    let mut projections = BTreeSet::new();
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
            if !has_vue3_type_alias_projection(analysis, local) {
                continue;
            }
            if !namespace_budget.reserve(
                local
                    .len()
                    .saturating_add(exported.len())
                    .saturating_add(1),
            ) {
                return None;
            }
            projections.insert((local.to_string(), exported.to_string()));
        }
    }

    let mut source_projection = Vue3ScriptSetupAnalysis::default();
    if !namespace_budget.reserve(
        projections
            .len()
            .saturating_mul(std::mem::size_of::<&String>()),
    ) {
        return None;
    }
    let source_names = projections
        .iter()
        .map(|(source_name, _)| source_name)
        .collect::<BTreeSet<_>>();
    for source_name in source_names {
        sync_vue3_namespace_type_projection(
            &mut source_projection,
            analysis,
            source_name,
            source_name,
            namespace_budget,
        )?;
    }
    for (source_name, target_name) in projections {
        sync_vue3_namespace_type_projection(
            analysis,
            &source_projection,
            &source_name,
            &target_name,
            namespace_budget,
        )?;
    }
    Some(())
}

const VUE3_MAX_NAMESPACE_PROJECTION_DEPTH: usize = 64;
const VUE3_MAX_NAMESPACE_PROJECTION_WORK: usize = 16 * 1024 * 1024;

pub(crate) struct Vue3NamespaceProjectionBudget {
    remaining_work: usize,
    exhausted: bool,
}

impl Default for Vue3NamespaceProjectionBudget {
    fn default() -> Self {
        Self {
            remaining_work: VUE3_MAX_NAMESPACE_PROJECTION_WORK,
            exhausted: false,
        }
    }
}

impl Vue3NamespaceProjectionBudget {
    fn reserve(&mut self, work: usize) -> bool {
        let Some(remaining_work) = self.remaining_work.checked_sub(work) else {
            self.remaining_work = 0;
            self.exhausted = true;
            return false;
        };
        self.remaining_work = remaining_work;
        true
    }

    pub(crate) fn is_exhausted(&self) -> bool {
        self.exhausted
    }
}

fn sync_vue3_namespace_type_projection(
    target: &mut Vue3ScriptSetupAnalysis,
    source: &Vue3ScriptSetupAnalysis,
    source_name: &str,
    target_name: &str,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<bool> {
    if !namespace_budget.reserve(vue3_type_alias_projection_work(
        source,
        source_name,
        target_name,
    )) {
        return None;
    }
    Some(sync_vue3_type_alias_from_analysis(
        target,
        source,
        source_name,
        target_name,
    ))
}

pub(crate) fn project_vue3_namespace_declaration(
    source: &str,
    declaration: &TSModuleDeclaration<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    let Some(namespace) = vue3_ts_module_declaration_name(declaration) else {
        return;
    };
    let mut namespace_budget = Vue3NamespaceProjectionBudget::default();
    if !validate_vue3_namespace_declaration_structure(
        declaration,
        1,
        &mut namespace_budget,
    ) {
        return;
    }
    let Some(public_names) = vue3_namespace_visible_type_names_with_budget(
        declaration,
        false,
        &mut namespace_budget,
    ) else {
        return;
    };
    let mergeable_scan_work = public_names.iter().fold(0usize, |work, name| {
        work.saturating_add(name.len()).saturating_add(1)
    });
    if !namespace_budget.reserve(mergeable_scan_work.saturating_mul(3)) {
        return;
    }
    let mergeable_names = vue3_namespace_visible_mergeable_names(
        declaration,
        false,
        Vue3NamespaceMergeKind::Interface,
    )
    .into_iter()
    .chain(vue3_namespace_visible_mergeable_names(
        declaration,
        false,
        Vue3NamespaceMergeKind::Enum,
    ))
    .chain(vue3_namespace_visible_mergeable_names(
        declaration,
        false,
        Vue3NamespaceMergeKind::Class,
    ))
    .collect::<BTreeSet<_>>();
    let mut projection = Vue3ScriptSetupAnalysis::default();
    project_vue3_namespace_declaration_with_prefix(
        source,
        declaration,
        &namespace,
        declaration.declare,
        1,
        &mergeable_names,
        analysis,
        &mut projection,
        &mut namespace_budget,
    );
    if namespace_budget.exhausted {
        return;
    }
    let final_projection_work = public_names.iter().fold(0usize, |work, name| {
        work.saturating_add(vue3_type_alias_projection_work(
            &projection,
            name,
            name,
        ))
    });
    if !namespace_budget.reserve(final_projection_work) {
        return;
    }
    for name in public_names {
        sync_vue3_type_alias_from_analysis(analysis, &projection, &name, &name);
    }
}

pub(crate) fn project_vue3_namespace_groups_from_statements_with_budget(
    source: &str,
    statements: &[Statement<'_>],
    ambient: bool,
    namespace_depth: usize,
    analysis: &mut Vue3ScriptSetupAnalysis,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> bool {
    project_vue3_namespace_groups_from_statement_groups_with_budget(
        source,
        &[statements],
        ambient,
        namespace_depth,
        analysis,
        namespace_budget,
    )
}

pub(crate) fn project_vue3_namespace_groups_from_statement_groups_with_budget(
    source: &str,
    statement_groups: &[&[Statement<'_>]],
    ambient: bool,
    namespace_depth: usize,
    analysis: &mut Vue3ScriptSetupAnalysis,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> bool {
    if !namespace_budget.reserve(vue3_type_analysis_clone_work(analysis)) {
        return false;
    }
    let mut working_analysis = analysis.clone();
    let Some(changed) =
        converge_vue3_namespace_groups_from_statement_groups_in_place_with_budget(
            source,
            statement_groups,
            ambient,
            namespace_depth,
            &mut working_analysis,
            namespace_budget,
        )
    else {
        namespace_budget.exhausted = true;
        return false;
    };
    *analysis = working_analysis;
    changed
}

fn converge_vue3_namespace_groups_from_statement_groups_in_place_with_budget(
    source: &str,
    statement_groups: &[&[Statement<'_>]],
    ambient: bool,
    namespace_depth: usize,
    analysis: &mut Vue3ScriptSetupAnalysis,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<bool> {
    for statements in statement_groups {
        if !validate_vue3_namespace_structure(statements, namespace_depth, namespace_budget) {
            return None;
        }
    }
    let namespace_steps = statement_groups
        .iter()
        .fold(0usize, |steps, statements| {
            steps.saturating_add(count_vue3_namespace_projection_steps(statements))
        });
    let refresh_steps = statement_groups
        .iter()
        .fold(0usize, |count, statements| {
            count.saturating_add(count_vue3_refreshable_type_declarations_in_statements(
                statements,
            ))
        });
    if namespace_steps == 0 && refresh_steps == 0 {
        return Some(false);
    }
    for statements in statement_groups {
        if !seed_vue3_namespace_public_type_names(
            statements,
            ambient,
            analysis,
            namespace_budget,
        ) {
            return None;
        }
    }
    let limit = namespace_steps
        .saturating_add(refresh_steps)
        .saturating_add(1);
    let mut converged = false;
    let mut any_changed = false;
    for _ in 0..limit {
        let statement_count = statement_groups
            .iter()
            .fold(1usize, |count, statements| {
                count.saturating_add(statements.len())
            });
        let outer_work = statement_count.saturating_mul(statement_count);
        if !namespace_budget.reserve(outer_work) {
            return None;
        }
        let mut changed = project_vue3_namespace_groups_from_statement_groups_once(
            source,
            statement_groups,
            ambient,
            namespace_depth,
            analysis,
            namespace_budget,
        );
        if namespace_budget.exhausted {
            return None;
        }
        changed |= refresh_vue3_declared_type_declarations_from_statement_groups_once(
            source,
            statement_groups,
            analysis,
        );
        changed |= collect_vue3_declared_type_deps_from_statement_groups(
            statement_groups,
            analysis,
        );
        if analysis.type_dependency_work_exhausted {
            namespace_budget.exhausted = true;
            return None;
        }
        any_changed |= changed;
        if !changed {
            converged = true;
            break;
        }
    }
    if converged {
        Some(any_changed)
    } else {
        None
    }
}

fn project_vue3_namespace_groups_from_statement_groups_once(
    source: &str,
    statement_groups: &[&[Statement<'_>]],
    ambient: bool,
    namespace_depth: usize,
    analysis: &mut Vue3ScriptSetupAnalysis,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> bool {
    let mut groups = BTreeMap::<String, Vec<&TSModuleDeclaration<'_>>>::new();
    for statements in statement_groups {
        for statement in *statements {
            let declaration = match statement {
                Statement::TSModuleDeclaration(declaration)
                    if !vue3_ts_module_declaration_is_global(declaration) =>
                {
                    Some(declaration)
                }
                Statement::ExportNamedDeclaration(export) => match export.declaration.as_ref() {
                    Some(Declaration::TSModuleDeclaration(declaration))
                        if !vue3_ts_module_declaration_is_global(declaration) =>
                    {
                        Some(declaration)
                    }
                    _ => None,
                },
                _ => None,
            };
            let Some(declaration) = declaration else {
                continue;
            };
            let Some(namespace) = vue3_ts_module_declaration_name(declaration) else {
                continue;
            };
            groups.entry(namespace).or_default().push(declaration);
        }
    }

    let mut changed = false;
    for (namespace, declarations) in groups {
        let mut projections = Vec::with_capacity(declarations.len());
        let mut contribution_indexes = BTreeMap::<String, Vec<usize>>::new();
        for declaration in declarations {
            let Some(declaration_public_names) = vue3_namespace_visible_type_names_with_budget(
                declaration,
                ambient,
                namespace_budget,
            ) else {
                return changed;
            };
            let mergeable_scan_work =
                declaration_public_names
                    .iter()
                    .fold(0usize, |work, name| {
                        work.saturating_add(name.len()).saturating_add(1)
                    });
            if !namespace_budget.reserve(mergeable_scan_work.saturating_mul(3)) {
                return changed;
            }
            let interface_names = vue3_namespace_visible_mergeable_names(
                declaration,
                ambient,
                Vue3NamespaceMergeKind::Interface,
            );
            let enum_names = vue3_namespace_visible_mergeable_names(
                declaration,
                ambient,
                Vue3NamespaceMergeKind::Enum,
            );
            let class_names = vue3_namespace_visible_mergeable_names(
                declaration,
                ambient,
                Vue3NamespaceMergeKind::Class,
            );
            let mergeable_names = interface_names
                .iter()
                .chain(&enum_names)
                .chain(&class_names)
                .cloned()
                .collect::<BTreeSet<_>>();
            let mut projection = Vue3ScriptSetupAnalysis::default();
            project_vue3_namespace_declaration_with_prefix(
                source,
                declaration,
                &namespace,
                ambient || declaration.declare,
                namespace_depth.saturating_add(1),
                &mergeable_names,
                analysis,
                &mut projection,
                namespace_budget,
            );
            let projection_index = projections.len();
            for name in &declaration_public_names {
                contribution_indexes
                    .entry(name.clone())
                    .or_default()
                    .push(projection_index);
            }
            projections.push(Vue3NamespaceBlockProjection {
                interface_names,
                enum_names,
                class_names,
                analysis: projection,
            });
            if namespace_budget.exhausted {
                return changed;
            }
        }

        for (name, indexes) in contribution_indexes {
            let contributors = indexes
                .iter()
                .map(|index| &projections[*index])
                .collect::<Vec<_>>();
            let interface_contributors = contributors
                .iter()
                .copied()
                .filter(|projection| projection.interface_names.contains(&name))
                .collect::<Vec<_>>();
            let enum_contributors = contributors
                .iter()
                .copied()
                .filter(|projection| projection.enum_names.contains(&name))
                .collect::<Vec<_>>();
            let class_contributors = contributors
                .iter()
                .copied()
                .filter(|projection| projection.class_names.contains(&name))
                .collect::<Vec<_>>();
            let merges_interfaces = interface_contributors.len() > 1
                && interface_contributors.len() == contributors.len();
            let merges_class_and_interfaces = !interface_contributors.is_empty()
                && class_contributors.len() == 1
                && interface_contributors.len().saturating_add(class_contributors.len())
                    == contributors.len();
            if merges_interfaces || merges_class_and_interfaces {
                for contributor in &contributors {
                    if !namespace_budget.reserve(vue3_type_alias_projection_work(
                        &contributor.analysis,
                        &name,
                        &name,
                    )) {
                        return changed;
                    }
                }
                let merged = merge_vue3_namespace_declaration_projections(&contributors, &name);
                let Some(sync_changed) = sync_vue3_namespace_type_projection(
                    analysis,
                    &merged,
                    &name,
                    &name,
                    namespace_budget,
                ) else {
                    return changed;
                };
                changed |= sync_changed;
            } else if enum_contributors.len() > 1
                && enum_contributors.len() == contributors.len()
            {
                for contributor in &enum_contributors {
                    if !namespace_budget.reserve(vue3_type_alias_projection_work(
                        &contributor.analysis,
                        &name,
                        &name,
                    )) {
                        return changed;
                    }
                }
                let merged =
                    merge_vue3_namespace_declaration_projections(&enum_contributors, &name);
                let Some(sync_changed) = sync_vue3_namespace_type_projection(
                    analysis,
                    &merged,
                    &name,
                    &name,
                    namespace_budget,
                ) else {
                    return changed;
                };
                changed |= sync_changed;
                changed |= analysis.local_ts_enum_type_names.insert(name.clone());
            } else if let Some(projection) = contributors.last() {
                let Some(sync_changed) = sync_vue3_namespace_type_projection(
                    analysis,
                    &projection.analysis,
                    &name,
                    &name,
                    namespace_budget,
                ) else {
                    return changed;
                };
                changed |= sync_changed;
            }
        }
    }
    changed
}

struct Vue3NamespaceBlockProjection {
    interface_names: BTreeSet<String>,
    enum_names: BTreeSet<String>,
    class_names: BTreeSet<String>,
    analysis: Vue3ScriptSetupAnalysis,
}

fn project_vue3_namespace_declaration_with_prefix(
    source: &str,
    declaration: &TSModuleDeclaration<'_>,
    prefix: &str,
    ambient: bool,
    namespace_depth: usize,
    mergeable_names: &BTreeSet<String>,
    analysis: &Vue3ScriptSetupAnalysis,
    projection: &mut Vue3ScriptSetupAnalysis,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> bool {
    if namespace_depth > VUE3_MAX_NAMESPACE_PROJECTION_DEPTH {
        namespace_budget.exhausted = true;
        return false;
    }
    let Some(body) = declaration.body.as_ref() else {
        return false;
    };
    match body {
        TSModuleDeclarationBody::TSModuleBlock(block) => {
            let statement_count = block.body.len().saturating_add(1);
            let block_work = statement_count
                .saturating_mul(statement_count)
                .saturating_mul(2);
            if !namespace_budget.reserve(block_work) {
                return false;
            }
            let Some(referenced_names) =
                vue3_namespace_referenced_names(&block.body, namespace_budget)
            else {
                return false;
            };
            let Some(mut namespace_analysis) = vue3_namespace_child_analysis(
                analysis,
                &referenced_names,
                namespace_budget,
            ) else {
                return false;
            };
            for local_name in &referenced_names {
                let public_name = format!("{prefix}.{local_name}");
                if !has_vue3_type_alias_projection(analysis, &public_name) {
                    continue;
                }
                if sync_vue3_namespace_type_projection(
                    &mut namespace_analysis,
                    analysis,
                    &public_name,
                    local_name,
                    namespace_budget,
                )
                .is_none()
                {
                    return false;
                }
                if analysis.local_ts_enum_type_names.contains(&public_name) {
                    namespace_analysis
                        .local_ts_enum_type_names
                        .insert(local_name.clone());
                }
            }
            if !seed_vue3_namespace_type_names(
                prefix,
                &block.body,
                &mut namespace_analysis,
                namespace_budget,
            ) {
                return false;
            }
            collect_vue3_declared_types_from_statements_with_namespace_budget(
                source,
                &block.body,
                ambient,
                namespace_depth,
                &mut namespace_analysis,
                namespace_budget,
            );
            if namespace_budget.exhausted {
                return false;
            }
            collect_vue3_declared_type_deps_from_statements(&block.body, &mut namespace_analysis);
            if namespace_analysis.type_dependency_work_exhausted {
                namespace_budget.exhausted = true;
                return false;
            }
            if !namespace_budget.reserve(vue3_local_generic_scope_capture_work(
                &namespace_analysis,
            )) {
                return false;
            }
            finalize_vue3_local_generic_alias_scopes(&mut namespace_analysis);
            let local_mergeable_names = mergeable_names
                .iter()
                .filter_map(|name| name.strip_prefix(&format!("{prefix}.")))
                .filter(|name| !name.contains('.'))
                .map(str::to_string)
                .collect::<BTreeSet<_>>();
            let excluded_interfaces = local_mergeable_names
                .iter()
                .filter(|name| !namespace_analysis.local_ts_enum_type_names.contains(*name))
                .cloned()
                .collect::<BTreeSet<_>>();
            let mut local_mergeable_projection = Vue3ScriptSetupAnalysis::default();
            for name in &local_mergeable_names {
                if sync_vue3_namespace_type_projection(
                    &mut local_mergeable_projection,
                    &namespace_analysis,
                    name,
                    name,
                    namespace_budget,
                )
                .is_none()
                {
                    return false;
                }
            }
            for name in &local_mergeable_names {
                let public_name = format!("{prefix}.{name}");
                if !has_vue3_type_alias_projection(analysis, &public_name) {
                    continue;
                }
                if sync_vue3_namespace_type_projection(
                    &mut namespace_analysis,
                    analysis,
                    &public_name,
                    name,
                    namespace_budget,
                )
                .is_none()
                {
                    return false;
                }
                if analysis.local_ts_enum_type_names.contains(&public_name) {
                    namespace_analysis.local_ts_enum_type_names.insert(name.clone());
                }
            }
            refresh_vue3_declared_type_declarations_excluding_interfaces(
                source,
                &block.body,
                &excluded_interfaces,
                &mut namespace_analysis,
            );
            collect_vue3_declared_type_deps_from_statements(
                &block.body,
                &mut namespace_analysis,
            );
            if namespace_analysis.type_dependency_work_exhausted {
                namespace_budget.exhausted = true;
                return false;
            }
            if !namespace_budget.reserve(vue3_local_generic_scope_capture_work(
                &namespace_analysis,
            )) {
                return false;
            }
            finalize_vue3_local_generic_alias_scopes(&mut namespace_analysis);
            let names = if ambient {
                let Some(names) = vue3_declared_type_names_from_statements_with_budget(
                    &block.body,
                    namespace_budget,
                ) else {
                    return false;
                };
                names
            } else {
                let Some(names) =
                    vue3_exported_type_names_with_budget(&block.body, namespace_budget)
                else {
                    return false;
                };
                names
            };
            let mut changed = false;
            for name in names {
                let prefixed = format!("{prefix}.{name}");
                let source_analysis = if local_mergeable_names.contains(&name) {
                    &local_mergeable_projection
                } else {
                    &namespace_analysis
                };
                let Some(sync_changed) = sync_vue3_namespace_type_projection(
                    projection,
                    source_analysis,
                    &name,
                    &prefixed,
                    namespace_budget,
                ) else {
                    return false;
                };
                changed |= sync_changed;
            }
            changed
        }
        TSModuleDeclarationBody::TSModuleDeclaration(nested) => {
            let Some(name) = vue3_ts_module_declaration_name(nested) else {
                return false;
            };
            let prefix = format!("{prefix}.{name}");
            project_vue3_namespace_declaration_with_prefix(
                source,
                nested,
                &prefix,
                ambient || nested.declare,
                namespace_depth.saturating_add(1),
                mergeable_names,
                analysis,
                projection,
                namespace_budget,
            )
        }
    }
}

fn vue3_namespace_child_analysis(
    analysis: &Vue3ScriptSetupAnalysis,
    referenced_names: &BTreeSet<String>,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<Vue3ScriptSetupAnalysis> {
    let mut child = Vue3ScriptSetupAnalysis {
        type_filename: analysis.type_filename.clone(),
        type_seen: analysis.type_seen.clone(),
        type_resolver: analysis.type_resolver.clone(),
        ..Vue3ScriptSetupAnalysis::default()
    };
    let captures_generic_aliases = referenced_names
        .iter()
        .any(|name| analysis.generic_type_aliases.contains_key(name));
    let captured_aliases = if captures_generic_aliases {
        if !namespace_budget.reserve(vue3_generic_alias_capture_work(
            analysis,
            referenced_names,
        )) {
            return None;
        }
        captured_vue3_generic_aliases_for_child_scope(analysis, referenced_names)
    } else {
        BTreeMap::new()
    };
    for name in referenced_names {
        sync_vue3_namespace_type_projection(
            &mut child,
            analysis,
            name,
            name,
            namespace_budget,
        )?;
    }
    child.generic_type_aliases.extend(captured_aliases);
    Some(child)
}

fn vue3_namespace_referenced_names(
    statements: &[Statement<'_>],
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<BTreeSet<String>> {
    let mut collector = Vue3NamespaceReferenceCollector {
        names: BTreeSet::new(),
        namespace_path: Vec::new(),
        namespace_budget,
    };
    for statement in statements {
        oxc_ast_visit::Visit::visit_statement(&mut collector, statement);
        if collector.namespace_budget.exhausted {
            return None;
        }
    }
    Some(collector.names)
}

struct Vue3NamespaceReferenceCollector<'budget> {
    names: BTreeSet<String>,
    namespace_path: Vec<String>,
    namespace_budget: &'budget mut Vue3NamespaceProjectionBudget,
}

impl<'a> oxc_ast_visit::Visit<'a> for Vue3NamespaceReferenceCollector<'_> {
    fn visit_identifier_reference(
        &mut self,
        identifier: &oxc_ast::ast::IdentifierReference<'a>,
    ) {
        if self.namespace_budget.exhausted {
            return;
        }
        self.insert_name(identifier.name.as_str());
        if self.namespace_budget.exhausted {
            return;
        }
        oxc_ast_visit::walk::walk_identifier_reference(self, identifier);
    }

    fn visit_ts_type_reference(&mut self, reference: &TSTypeReference<'a>) {
        if self.namespace_budget.exhausted {
            return;
        }
        if let Some(name) = vue3_ts_type_name_key(&reference.type_name) {
            self.insert_name(&name);
        }
        if self.namespace_budget.exhausted {
            return;
        }
        oxc_ast_visit::walk::walk_ts_type_reference(self, reference);
    }

    fn visit_ts_type_query(&mut self, query: &TSTypeQuery<'a>) {
        if self.namespace_budget.exhausted {
            return;
        }
        if let Some(name) = vue3_type_query_name_key(query) {
            self.insert_name(&name);
        }
        if self.namespace_budget.exhausted {
            return;
        }
        oxc_ast_visit::walk::walk_ts_type_query(self, query);
    }

    fn visit_ts_interface_heritage(&mut self, heritage: &TSInterfaceHeritage<'a>) {
        if self.namespace_budget.exhausted {
            return;
        }
        if let Some(name) = vue3_interface_heritage_name(heritage) {
            self.insert_name(&name);
        }
        if self.namespace_budget.exhausted {
            return;
        }
        oxc_ast_visit::walk::walk_ts_interface_heritage(self, heritage);
    }

    fn visit_ts_module_declaration(&mut self, declaration: &TSModuleDeclaration<'a>) {
        if self.namespace_budget.exhausted {
            return;
        }
        let Some(name) = vue3_ts_module_declaration_name(declaration) else {
            oxc_ast_visit::walk::walk_ts_module_declaration(self, declaration);
            return;
        };
        self.namespace_path.push(name);
        oxc_ast_visit::walk::walk_ts_module_declaration(self, declaration);
        self.namespace_path.pop();
    }
}

impl Vue3NamespaceReferenceCollector<'_> {
    fn insert_name(&mut self, name: &str) {
        if !self.namespace_budget.reserve(name.len().saturating_add(1)) {
            return;
        }
        self.names.insert(name.to_string());
        for length in 1..=self.namespace_path.len() {
            let path_length = self.namespace_path[..length]
                .iter()
                .fold(0usize, |size, segment| size.saturating_add(segment.len()))
                .saturating_add(length.saturating_sub(1));
            let qualified_length = path_length.saturating_add(name.len()).saturating_add(1);
            if !self
                .namespace_budget
                .reserve(qualified_length.saturating_add(1))
            {
                return;
            }
            let mut qualified = String::with_capacity(qualified_length);
            for segment in &self.namespace_path[..length] {
                if !qualified.is_empty() {
                    qualified.push('.');
                }
                qualified.push_str(segment);
            }
            qualified.push('.');
            qualified.push_str(name);
            self.names.insert(qualified);
        }
    }
}

fn merge_vue3_namespace_declaration_projections(
    projections: &[&Vue3NamespaceBlockProjection],
    name: &str,
) -> Vue3ScriptSetupAnalysis {
    let Some(last) = projections.last() else {
        return Vue3ScriptSetupAnalysis::default();
    };
    let mut merged = Vue3ScriptSetupAnalysis::default();
    sync_vue3_type_alias_from_analysis(&mut merged, &last.analysis, name, name);

    macro_rules! merge_vector_entry {
        ($field:ident) => {{
            let mut found = false;
            let mut values = Vec::new();
            for projection in projections {
                if let Some(source_values) = projection.analysis.$field.get(name) {
                    found = true;
                    for value in source_values {
                        if !values.contains(value) {
                            values.push(value.clone());
                        }
                    }
                }
            }
            if found {
                merged.$field.insert(name.to_string(), values);
            } else {
                merged.$field.remove(name);
            }
        }};
    }

    macro_rules! merge_set_entry {
        ($field:ident) => {{
            let mut found = false;
            let mut values = BTreeSet::new();
            for projection in projections {
                if let Some(source_values) = projection.analysis.$field.get(name) {
                    found = true;
                    values.extend(source_values.iter().cloned());
                }
            }
            if found {
                merged.$field.insert(name.to_string(), values);
            } else {
                merged.$field.remove(name);
            }
        }};
    }

    macro_rules! merge_members_entry {
        ($field:ident) => {{
            let values = projections
                .iter()
                .filter_map(|projection| projection.analysis.$field.get(name).cloned())
                .collect::<Vec<_>>();
            if values.is_empty() {
                merged.$field.remove(name);
            } else {
                let mut source_parts = Vec::new();
                for value in &values {
                    if !source_parts.contains(&value.source) {
                        source_parts.push(value.source.clone());
                    }
                }
                let (members, errors) = vue3_merge_props_type_members(values, false);
                merged.$field.insert(
                    name.to_string(),
                    Vue27TypeMembers {
                        source: source_parts.join("\n"),
                        members,
                        errors,
                    },
                );
            }
        }};
    }

    merge_vector_entry!(declared_types);
    merge_vector_entry!(define_model_declared_types);
    merge_vector_entry!(type_query_declared_types);
    merge_vector_entry!(define_model_type_query_declared_types);
    merge_vector_entry!(keyof_type_query_declared_types);
    merge_members_entry!(props_type_declarations);
    merge_vector_entry!(keyof_runtime_type_declarations);
    merge_vector_entry!(tuple_runtime_type_declarations);
    merge_vector_entry!(define_model_tuple_runtime_type_declarations);
    merge_vector_entry!(array_element_runtime_type_declarations);
    merge_vector_entry!(define_model_array_element_runtime_type_declarations);
    merge_vector_entry!(parameter_tuple_runtime_type_declarations);
    merge_vector_entry!(define_model_parameter_tuple_runtime_type_declarations);
    merge_vector_entry!(constructor_parameter_tuple_runtime_type_declarations);
    merge_vector_entry!(define_model_constructor_parameter_tuple_runtime_type_declarations);
    merge_vector_entry!(return_type_runtime_type_declarations);
    merge_vector_entry!(define_model_return_type_runtime_type_declarations);
    merge_members_entry!(props_options_type_declarations);
    merge_members_entry!(return_type_props_options_declarations);
    let aliases = projections
        .iter()
        .filter_map(|projection| projection.analysis.generic_type_aliases.get(name))
        .collect::<Vec<_>>();
    let interface_contributor_count = projections
        .iter()
        .filter(|projection| projection.interface_names.contains(name))
        .count();
    if !aliases.is_empty() && aliases.len() == interface_contributor_count {
        let first = aliases[0];
        if first.kind == Vue3GenericTypeAliasKind::Interface
            && aliases
                .iter()
                .all(|alias| alias.kind == first.kind && alias.params == first.params)
        {
            let mut alias = aliases[aliases.len() - 1].clone();
            let mut fragments = Vec::new();
            for contributor in aliases {
                if contributor.interface_fragments.is_empty() {
                    fragments.push(Vue3GenericInterfaceFragment {
                        source: contributor.source.clone(),
                        scope: contributor.scope.clone(),
                    });
                } else {
                    fragments.extend(contributor.interface_fragments.iter().cloned());
                }
            }
            alias.source.clear();
            alias.interface_fragments = fragments;
            merged.generic_type_aliases.insert(name.to_string(), alias);
        }
    }
    merge_set_entry!(string_literal_type_declarations);
    merge_vector_entry!(ordered_string_literal_type_declarations);
    merge_vector_entry!(type_direct_deps);
    merge_set_entry!(type_deps);

    let emits = projections
        .iter()
        .filter_map(|projection| projection.analysis.emits_type_declarations.get(name))
        .collect::<Vec<_>>();
    if emits.is_empty() {
        merged.emits_type_declarations.remove(name);
    } else {
        let mut source_parts = Vec::new();
        let mut events = Vec::new();
        let mut syntax = Vue3EmitsTypeSyntax::default();
        let mut call_count = 0usize;
        for emit in emits {
            if !source_parts.contains(&emit.source) {
                source_parts.push(emit.source.clone());
            }
            for event in &emit.events {
                push_unique(&mut events, event);
            }
            syntax.has_call_signature |= emit.syntax.has_call_signature;
            syntax.has_property |= emit.syntax.has_property;
            call_count = call_count.saturating_add(emit.call_count);
        }
        merged.emits_type_declarations.insert(
            name.to_string(),
            Vue27EmitsType {
                source: source_parts.join("\n"),
                events,
                syntax,
                call_count,
            },
        );
    }

    if projections
        .iter()
        .any(|projection| projection.analysis.silent_unresolved_type_names.contains(name))
    {
        merged.silent_unresolved_type_names.insert(name.to_string());
    } else {
        merged.silent_unresolved_type_names.remove(name);
    }
    merged
}

fn validate_vue3_namespace_structure(
    statements: &[Statement<'_>],
    parent_depth: usize,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> bool {
    let roots = statements
        .iter()
        .filter_map(vue3_namespace_declaration_from_statement)
        .map(|declaration| (declaration, parent_depth.saturating_add(1)))
        .collect::<Vec<_>>();
    validate_vue3_namespace_declarations(roots, namespace_budget)
}

fn validate_vue3_namespace_declaration_structure(
    declaration: &TSModuleDeclaration<'_>,
    depth: usize,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> bool {
    validate_vue3_namespace_declarations(vec![(declaration, depth)], namespace_budget)
}

fn validate_vue3_namespace_declarations<'a>(
    mut pending: Vec<(&'a TSModuleDeclaration<'a>, usize)>,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> bool {
    while let Some((declaration, depth)) = pending.pop() {
        if depth > VUE3_MAX_NAMESPACE_PROJECTION_DEPTH || !namespace_budget.reserve(1) {
            namespace_budget.exhausted = true;
            return false;
        }
        match declaration.body.as_ref() {
            Some(TSModuleDeclarationBody::TSModuleDeclaration(nested)) => {
                pending.push((nested, depth.saturating_add(1)));
            }
            Some(TSModuleDeclarationBody::TSModuleBlock(block)) => {
                pending.extend(
                    block
                        .body
                        .iter()
                        .filter_map(vue3_namespace_declaration_from_statement)
                        .map(|nested| (nested, depth.saturating_add(1))),
                );
            }
            None => {}
        }
    }
    true
}

fn count_vue3_namespace_projection_steps(statements: &[Statement<'_>]) -> usize {
    let mut pending = statements
        .iter()
        .filter_map(vue3_namespace_declaration_from_statement)
        .collect::<Vec<_>>();
    let mut count = 0usize;
    while let Some(declaration) = pending.pop() {
        count = count.saturating_add(1);
        match declaration.body.as_ref() {
            Some(TSModuleDeclarationBody::TSModuleDeclaration(nested)) => {
                pending.push(nested);
            }
            Some(TSModuleDeclarationBody::TSModuleBlock(block)) => {
                for statement in &block.body {
                    if let Some(nested) = vue3_namespace_declaration_from_statement(statement) {
                        pending.push(nested);
                    } else {
                        count = count.saturating_add(
                            vue3_namespace_projection_statement_step_bound(statement),
                        );
                    }
                }
            }
            None => {}
        }
    }
    count
}

fn vue3_namespace_projection_statement_step_bound(statement: &Statement<'_>) -> usize {
    match statement {
        Statement::VariableDeclaration(declaration) => declaration.declarations.len(),
        Statement::ExportNamedDeclaration(export) => export.declaration.as_ref().map_or(0, |decl| {
            if let Declaration::VariableDeclaration(declaration) = decl {
                declaration.declarations.len()
            } else {
                1
            }
        }),
        Statement::TSInterfaceDeclaration(_)
        | Statement::TSTypeAliasDeclaration(_)
        | Statement::TSEnumDeclaration(_)
        | Statement::FunctionDeclaration(_)
        | Statement::ClassDeclaration(_) => 1,
        _ => 0,
    }
}

pub(crate) fn seed_vue3_namespace_public_type_names(
    statements: &[Statement<'_>],
    ambient: bool,
    analysis: &mut Vue3ScriptSetupAnalysis,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> bool {
    for statement in statements {
        let Some(declaration) = vue3_namespace_declaration_from_statement(statement) else {
            continue;
        };
        let Some(names) =
            vue3_namespace_visible_type_names_with_budget(declaration, ambient, namespace_budget)
        else {
            return false;
        };
        let seed_work = names.iter().fold(0usize, |work, name| {
            work.saturating_add(name.len()).saturating_add(1)
        });
        if !namespace_budget.reserve(seed_work.saturating_mul(2)) {
            return false;
        }
        seed_vue3_qualified_type_names(
            names,
            analysis,
        );
    }
    true
}

fn vue3_namespace_declaration_from_statement<'a>(
    statement: &'a Statement<'a>,
) -> Option<&'a TSModuleDeclaration<'a>> {
    match statement {
        Statement::TSModuleDeclaration(declaration)
            if !vue3_ts_module_declaration_is_global(declaration) =>
        {
            Some(declaration)
        }
        Statement::ExportNamedDeclaration(export) => match export.declaration.as_ref() {
            Some(Declaration::TSModuleDeclaration(declaration))
                if !vue3_ts_module_declaration_is_global(declaration) =>
            {
                Some(declaration)
            }
            _ => None,
        },
        _ => None,
    }
}

fn vue3_namespace_visible_type_names_with_budget(
    declaration: &TSModuleDeclaration<'_>,
    ambient: bool,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<BTreeSet<String>> {
    if ambient || declaration.declare {
        vue3_namespace_declared_type_names_with_budget(declaration, namespace_budget)
    } else {
        vue3_namespace_exported_type_names_with_budget(declaration, namespace_budget)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Vue3NamespaceMergeKind {
    Interface,
    Enum,
    Class,
}

fn vue3_namespace_visible_mergeable_names(
    declaration: &TSModuleDeclaration<'_>,
    ambient: bool,
    kind: Vue3NamespaceMergeKind,
) -> BTreeSet<String> {
    let Some(namespace) = vue3_ts_module_declaration_name(declaration) else {
        return BTreeSet::new();
    };
    vue3_namespace_visible_mergeable_names_with_prefix(
        declaration,
        &namespace,
        ambient || declaration.declare,
        kind,
    )
}

fn vue3_namespace_visible_mergeable_names_with_prefix(
    declaration: &TSModuleDeclaration<'_>,
    prefix: &str,
    ambient: bool,
    kind: Vue3NamespaceMergeKind,
) -> BTreeSet<String> {
    let Some(body) = declaration.body.as_ref() else {
        return BTreeSet::new();
    };
    match body {
        TSModuleDeclarationBody::TSModuleBlock(block) => vue3_visible_mergeable_names_from_statements(
            &block.body,
            prefix,
            ambient,
            kind,
        ),
        TSModuleDeclarationBody::TSModuleDeclaration(nested) => {
            let Some(name) = vue3_ts_module_declaration_name(nested) else {
                return BTreeSet::new();
            };
            let prefix = format!("{prefix}.{name}");
            vue3_namespace_visible_mergeable_names_with_prefix(
                nested,
                &prefix,
                ambient || nested.declare,
                kind,
            )
        }
    }
}

fn vue3_visible_mergeable_names_from_statements(
    statements: &[Statement<'_>],
    prefix: &str,
    ambient: bool,
    kind: Vue3NamespaceMergeKind,
) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for statement in statements {
        match statement {
            Statement::TSInterfaceDeclaration(declaration)
                if ambient && kind == Vue3NamespaceMergeKind::Interface =>
            {
                names.insert(format!("{prefix}.{}", declaration.id.name));
            }
            Statement::TSEnumDeclaration(declaration)
                if ambient && kind == Vue3NamespaceMergeKind::Enum =>
            {
                names.insert(format!("{prefix}.{}", declaration.id.name));
            }
            Statement::ClassDeclaration(declaration)
                if ambient && kind == Vue3NamespaceMergeKind::Class =>
            {
                if let Some(id) = &declaration.id {
                    names.insert(format!("{prefix}.{}", id.name));
                }
            }
            Statement::TSModuleDeclaration(declaration) if ambient => {
                let Some(name) = vue3_ts_module_declaration_name(declaration) else {
                    continue;
                };
                names.extend(vue3_namespace_visible_mergeable_names_with_prefix(
                    declaration,
                    &format!("{prefix}.{name}"),
                    true,
                    kind,
                ));
            }
            Statement::ExportNamedDeclaration(export) => match export.declaration.as_ref() {
                Some(Declaration::TSInterfaceDeclaration(declaration))
                    if kind == Vue3NamespaceMergeKind::Interface =>
                {
                    names.insert(format!("{prefix}.{}", declaration.id.name));
                }
                Some(Declaration::TSEnumDeclaration(declaration))
                    if kind == Vue3NamespaceMergeKind::Enum =>
                {
                    names.insert(format!("{prefix}.{}", declaration.id.name));
                }
                Some(Declaration::ClassDeclaration(declaration))
                    if kind == Vue3NamespaceMergeKind::Class =>
                {
                    if let Some(id) = &declaration.id {
                        names.insert(format!("{prefix}.{}", id.name));
                    }
                }
                Some(Declaration::TSModuleDeclaration(declaration)) => {
                    let Some(name) = vue3_ts_module_declaration_name(declaration) else {
                        continue;
                    };
                    names.extend(vue3_namespace_visible_mergeable_names_with_prefix(
                        declaration,
                        &format!("{prefix}.{name}"),
                        ambient || declaration.declare,
                        kind,
                    ));
                }
                _ => {}
            },
            _ => {}
        }
    }
    names
}

pub(crate) fn seed_vue3_namespace_type_names(
    prefix: &str,
    statements: &[Statement<'_>],
    analysis: &mut Vue3ScriptSetupAnalysis,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> bool {
    let Some(names) =
        vue3_declared_type_names_from_statements_with_budget(statements, namespace_budget)
    else {
        return false;
    };
    for name in names {
        let Some(prefixed) =
            reserve_vue3_qualified_namespace_name(prefix, &name, namespace_budget)
        else {
            return false;
        };
        let key_work = name
            .len()
            .saturating_add(1)
            .saturating_add(prefixed.len())
            .saturating_add(1)
            .saturating_mul(2);
        if !namespace_budget.reserve(key_work) {
            return false;
        }
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
    true
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
    vue3_ts_module_declaration_name_ref(declaration).map(str::to_string)
}

pub(crate) fn vue3_ts_module_declaration_name_ref<'a>(
    declaration: &'a TSModuleDeclaration<'_>,
) -> Option<&'a str> {
    match &declaration.id {
        TSModuleDeclarationName::Identifier(identifier) => Some(identifier.name.as_str()),
        TSModuleDeclarationName::StringLiteral(_) => None,
    }
}

pub(crate) fn vue3_ts_module_declaration_is_global(declaration: &TSModuleDeclaration<'_>) -> bool {
    vue3_ts_module_declaration_name_ref(declaration) == Some("global")
}

pub(crate) fn vue3_ts_module_declaration_block_body<'a>(
    declaration: &'a TSModuleDeclaration<'a>,
) -> Option<&'a [Statement<'a>]> {
    match declaration.body.as_ref()? {
        TSModuleDeclarationBody::TSModuleBlock(block) => Some(&block.body),
        TSModuleDeclarationBody::TSModuleDeclaration(_) => None,
    }
}

fn vue3_namespace_exported_type_names_with_budget(
    declaration: &TSModuleDeclaration<'_>,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<BTreeSet<String>> {
    let Some(namespace) = vue3_ts_module_declaration_name_ref(declaration) else {
        return Some(BTreeSet::new());
    };
    vue3_namespace_exported_type_names_with_prefix_and_budget(
        declaration,
        namespace,
        namespace_budget,
    )
}

pub(crate) fn vue3_namespace_declared_type_names_with_budget(
    declaration: &TSModuleDeclaration<'_>,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<BTreeSet<String>> {
    let Some(namespace) = vue3_ts_module_declaration_name_ref(declaration) else {
        return Some(BTreeSet::new());
    };
    vue3_namespace_declared_type_names_with_prefix_and_budget(
        declaration,
        namespace,
        namespace_budget,
    )
}

fn vue3_namespace_declared_type_names_with_prefix_and_budget(
    declaration: &TSModuleDeclaration<'_>,
    prefix: &str,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<BTreeSet<String>> {
    if !namespace_budget.reserve(prefix.len().saturating_add(1)) {
        return None;
    }
    let mut names = BTreeSet::new();
    let mut pending = vec![(declaration, prefix.to_string(), 1usize)];
    while let Some((declaration, prefix, depth)) = pending.pop() {
        if depth > VUE3_MAX_NAMESPACE_PROJECTION_DEPTH {
            continue;
        }
        match declaration.body.as_ref() {
            Some(TSModuleDeclarationBody::TSModuleBlock(block)) => {
                for statement in &block.body {
                    if let Some(nested) = vue3_namespace_declaration_from_statement(statement) {
                        if let Some(name) = vue3_ts_module_declaration_name_ref(nested) {
                            let prefix = reserve_vue3_qualified_namespace_name(
                                &prefix,
                                name,
                                namespace_budget,
                            )?;
                            pending.push((
                                nested,
                                prefix,
                                depth.saturating_add(1),
                            ));
                        }
                        continue;
                    }
                    for name in vue3_declared_type_names_from_statement_with_budget(
                        statement,
                        namespace_budget,
                    )? {
                        names.insert(reserve_vue3_qualified_namespace_name(
                            &prefix,
                            &name,
                            namespace_budget,
                        )?);
                    }
                }
            }
            Some(TSModuleDeclarationBody::TSModuleDeclaration(nested)) => {
                if let Some(name) = vue3_ts_module_declaration_name_ref(nested) {
                    let prefix = reserve_vue3_qualified_namespace_name(
                        &prefix,
                        name,
                        namespace_budget,
                    )?;
                    pending.push((
                        nested,
                        prefix,
                        depth.saturating_add(1),
                    ));
                }
            }
            None => {}
        }
    }
    Some(names)
}

fn vue3_namespace_exported_type_names_with_prefix_and_budget(
    declaration: &TSModuleDeclaration<'_>,
    prefix: &str,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<BTreeSet<String>> {
    if !namespace_budget.reserve(prefix.len().saturating_add(1)) {
        return None;
    }
    let mut names = BTreeSet::new();
    let mut pending = vec![(declaration, prefix.to_string(), 1usize)];
    while let Some((declaration, prefix, depth)) = pending.pop() {
        if depth > VUE3_MAX_NAMESPACE_PROJECTION_DEPTH {
            continue;
        }
        match declaration.body.as_ref() {
            Some(TSModuleDeclarationBody::TSModuleBlock(block)) => {
                for statement in &block.body {
                    let exported_nested = match statement {
                        Statement::ExportNamedDeclaration(export) => export
                            .declaration
                            .as_ref()
                            .and_then(|declaration| match declaration {
                                Declaration::TSModuleDeclaration(declaration) => {
                                    Some(declaration)
                                }
                                _ => None,
                            }),
                        _ => None,
                    };
                    if let Some(nested) = exported_nested {
                        if let Some(name) = vue3_ts_module_declaration_name_ref(nested) {
                            let prefix = reserve_vue3_qualified_namespace_name(
                                &prefix,
                                name,
                                namespace_budget,
                            )?;
                            pending.push((
                                nested,
                                prefix,
                                depth.saturating_add(1),
                            ));
                        }
                        continue;
                    }
                    for name in vue3_exported_type_names_with_budget(
                        std::slice::from_ref(statement),
                        namespace_budget,
                    )? {
                        names.insert(reserve_vue3_qualified_namespace_name(
                            &prefix,
                            &name,
                            namespace_budget,
                        )?);
                    }
                }
            }
            Some(TSModuleDeclarationBody::TSModuleDeclaration(nested)) => {
                if let Some(name) = vue3_ts_module_declaration_name_ref(nested) {
                    let prefix = reserve_vue3_qualified_namespace_name(
                        &prefix,
                        name,
                        namespace_budget,
                    )?;
                    pending.push((
                        nested,
                        prefix,
                        depth.saturating_add(1),
                    ));
                }
            }
            None => {}
        }
    }
    Some(names)
}

fn reserve_vue3_qualified_namespace_name(
    prefix: &str,
    name: &str,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<String> {
    let length = prefix.len().saturating_add(name.len()).saturating_add(1);
    if !namespace_budget.reserve(length.saturating_add(1)) {
        return None;
    }
    let mut qualified = String::with_capacity(length);
    qualified.push_str(prefix);
    qualified.push('.');
    qualified.push_str(name);
    Some(qualified)
}

#[cfg(test)]
mod namespace_projection_budget_tests {
    use super::*;

    fn budget(limit: usize) -> Vue3NamespaceProjectionBudget {
        Vue3NamespaceProjectionBudget {
            remaining_work: limit,
            exhausted: false,
        }
    }

    #[test]
    fn namespace_projection_budget_honors_exact_and_overflow_boundaries() {
        let mut budget = budget(3);
        assert!(budget.reserve(2));
        assert!(budget.reserve(1));
        assert_eq!(budget.remaining_work, 0);
        assert!(!budget.reserve(1));
        assert!(budget.exhausted);
    }

    #[test]
    fn namespace_projection_depth_honors_exact_and_overflow_boundaries() {
        for (depth, expected) in [(64, true), (65, false)] {
            let source = format!(
                "namespace {} {{ export interface Props {{ value: string }} }}",
                (0..depth).map(|_| "N").collect::<Vec<_>>().join(".")
            );
            let allocator = oxc_allocator::Allocator::default();
            let parsed = oxc_parser::Parser::new(
                &allocator,
                &source,
                oxc_span::SourceType::ts(),
            )
            .parse();
            assert!(!parsed.panicked && parsed.errors.is_empty());
            let mut budget = budget(1024);
            assert_eq!(
                validate_vue3_namespace_structure(&parsed.program.body, 0, &mut budget),
                expected
            );
            assert_eq!(budget.exhausted, !expected);
        }
    }

    #[test]
    fn nested_namespace_projection_exhaustion_discards_partial_results() {
        let source = r#"
export namespace Root {
  export namespace Child {
    export interface Props { value: string }
  }
}
"#;
        let allocator = oxc_allocator::Allocator::default();
        let parsed = oxc_parser::Parser::new(
            &allocator,
            source,
            oxc_span::SourceType::ts(),
        )
        .parse();
        assert!(!parsed.panicked && parsed.errors.is_empty());
        let mut analysis = Vue3ScriptSetupAnalysis::default();
        let mut budget = budget(18);

        project_vue3_namespace_groups_from_statements_with_budget(
            source,
            &parsed.program.body,
            false,
            0,
            &mut analysis,
            &mut budget,
        );

        assert!(budget.exhausted);
        assert!(!has_vue3_type_alias_projection(
            &analysis,
            "Root.Child.Props"
        ));
    }

    #[test]
    fn named_namespace_import_projection_is_bounded_and_transactional() {
        const LIMIT: usize = 1024 * 1024;

        let dir = tempfile::tempdir().expect("temp dir");
        let types = dir.path().join("types.ts");
        std::fs::write(
            &types,
            "export namespace N { export interface Props { value: string } }",
        )
        .expect("write namespace import type");
        let filename = dir.path().join("Comp.ts");
        let source = "import type { N as First, N as Second } from './types'";
        let resolver = Vue3TypeResolverContext::default();
        let imported = vue3_external_type_context_from_path(
            &types,
            &mut BTreeSet::new(),
            &resolver,
        )
        .expect("load namespace import context");
        assert!(vue3_type_context_names(&imported)
            .iter()
            .all(|name| imported.type_sources.contains_key(name)));

        let mut expected = Vue27TypeContext::default();
        expected
            .declared_types
            .insert("Stable".into(), vec!["String".into()]);
        let mut measured_context = expected.clone();
        let mut measured_budget = budget(LIMIT);
        assert!(extend_vue3_type_context_from_external_imports_with_seen(
            &filename.to_string_lossy(),
            source,
            oxc_span::SourceType::ts(),
            &mut measured_context,
            &mut BTreeSet::new(),
            &resolver,
            &mut measured_budget,
        ));
        let total_work = LIMIT.saturating_sub(measured_budget.remaining_work);
        assert!(total_work > 0);
        assert!(measured_context
            .props_type_declarations
            .contains_key("First.Props"));
        assert!(measured_context
            .props_type_declarations
            .contains_key("Second.Props"));

        let mut context = expected.clone();
        let mut overflow_budget = budget(total_work.saturating_sub(1));
        assert!(!extend_vue3_type_context_from_external_imports_with_seen(
            &filename.to_string_lossy(),
            source,
            oxc_span::SourceType::ts(),
            &mut context,
            &mut BTreeSet::new(),
            &resolver,
            &mut overflow_budget,
        ));
        assert_eq!(context, expected);

        let mut exact_context = expected;
        let mut exact_budget = budget(total_work);
        assert!(extend_vue3_type_context_from_external_imports_with_seen(
            &filename.to_string_lossy(),
            source,
            oxc_span::SourceType::ts(),
            &mut exact_context,
            &mut BTreeSet::new(),
            &resolver,
            &mut exact_budget,
        ));
        assert_eq!(exact_budget.remaining_work, 0);
        assert!(exact_context
            .props_type_declarations
            .contains_key("First.Props"));
        assert!(exact_context
            .props_type_declarations
            .contains_key("Second.Props"));
    }

    #[test]
    fn global_namespace_budget_is_shared_and_transactional() {
        const LIMIT: usize = 1024 * 1024;
        let one_block = r#"
export {}
declare global {
  namespace One { interface Props { value: string } }
}
"#;
        let allocator = oxc_allocator::Allocator::default();
        let parsed = oxc_parser::Parser::new(
            &allocator,
            one_block,
            oxc_span::SourceType::ts(),
        )
        .parse();
        assert!(!parsed.panicked && parsed.errors.is_empty());
        let mut measured_analysis = Vue3ScriptSetupAnalysis::default();
        let mut measured_budget = budget(LIMIT);
        assert!(collect_vue3_global_types_from_statements_with_budget(
            one_block,
            &parsed.program.body,
            false,
            &Vue27TypeContext::default(),
            &mut measured_analysis,
            &mut measured_budget,
        )
        .is_some());
        let one_block_work = LIMIT.saturating_sub(measured_budget.remaining_work);
        assert!(one_block_work > 0);

        let two_blocks = r#"
export {}
declare global {
  namespace One { interface Props { value: string } }
}
declare global {
  namespace Two { interface Props { value: string } }
}
"#;
        let allocator = oxc_allocator::Allocator::default();
        let parsed = oxc_parser::Parser::new(
            &allocator,
            two_blocks,
            oxc_span::SourceType::ts(),
        )
        .parse();
        assert!(!parsed.panicked && parsed.errors.is_empty());
        let mut analysis = Vue3ScriptSetupAnalysis::default();
        analysis
            .declared_types
            .insert("Existing".into(), vec!["String".into()]);
        let expected = analysis.clone();
        let mut bounded_budget = budget(one_block_work);

        assert!(collect_vue3_global_types_from_statements_with_budget(
            two_blocks,
            &parsed.program.body,
            false,
            &Vue27TypeContext::default(),
            &mut analysis,
            &mut bounded_budget,
        )
        .is_none());
        assert!(bounded_budget.exhausted);
        assert_eq!(analysis, expected);
    }
}
