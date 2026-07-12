pub(crate) fn vue27_normal_script_type_context(descriptor: &SfcDescriptor) -> Vue27TypeContext {
    let Some(script) = descriptor.script.as_ref() else {
        return Vue27TypeContext::default();
    };
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        script.content.as_str(),
        script_source_type_from_attrs(&script.attrs),
    )
    .with_options(oxc_parser::ParseOptions {
        parse_regular_expression: true,
        ..oxc_parser::ParseOptions::default()
    })
    .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return Vue27TypeContext::default();
    }
    let mut analysis = Vue27ScriptSetupAnalysis::default();
    collect_vue27_declared_types_from_statements(
        script.content.as_str(),
        &parsed.program.body,
        &mut analysis,
    );
    Vue27TypeContext {
        declared_types: analysis.declared_types,
        define_model_declared_types: BTreeMap::new(),
        type_query_declared_types: BTreeMap::new(),
        define_model_type_query_declared_types: BTreeMap::new(),
        keyof_type_query_declared_types: BTreeMap::new(),
        props_type_declarations: analysis.props_type_declarations,
        keyof_runtime_type_declarations: BTreeMap::new(),
        tuple_runtime_type_declarations: BTreeMap::new(),
        define_model_tuple_runtime_type_declarations: BTreeMap::new(),
        array_element_runtime_type_declarations: BTreeMap::new(),
        define_model_array_element_runtime_type_declarations: BTreeMap::new(),
        parameter_tuple_runtime_type_declarations: BTreeMap::new(),
        define_model_parameter_tuple_runtime_type_declarations: BTreeMap::new(),
        constructor_parameter_tuple_runtime_type_declarations: BTreeMap::new(),
        define_model_constructor_parameter_tuple_runtime_type_declarations: BTreeMap::new(),
        return_type_runtime_type_declarations: BTreeMap::new(),
        define_model_return_type_runtime_type_declarations: BTreeMap::new(),
        props_options_type_declarations: BTreeMap::new(),
        return_type_props_options_declarations: BTreeMap::new(),
        generic_type_aliases: BTreeMap::new(),
        string_literal_type_declarations: BTreeMap::new(),
        ordered_string_literal_type_declarations: BTreeMap::new(),
        emits_type_declarations: analysis.emits_type_declarations,
        type_sources: BTreeMap::new(),
        type_direct_deps: BTreeMap::new(),
        type_deps: BTreeMap::new(),
        unresolved_import_sources: BTreeMap::new(),
        silent_unresolved_type_names: BTreeSet::new(),
    }
}

pub(crate) fn vue3_normal_script_type_context(
    descriptor: &SfcDescriptor,
    global_type_files: &[String],
    type_resolver: &Vue3TypeResolverContext,
) -> Vue27TypeContext {
    let mut context =
        vue3_global_type_context(&descriptor.filename, global_type_files, type_resolver);
    let Some(script) = descriptor.script.as_ref() else {
        return context;
    };
    extend_vue3_type_context_from_external_imports(
        &descriptor.filename,
        script.content.as_str(),
        script_source_type_from_attrs(&script.attrs),
        &mut context,
        type_resolver,
    );
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        script.content.as_str(),
        script_source_type_from_attrs(&script.attrs),
    )
    .with_options(oxc_parser::ParseOptions {
        parse_regular_expression: true,
        ..oxc_parser::ParseOptions::default()
    })
    .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return context;
    }
    let mut analysis = Vue3ScriptSetupAnalysis {
        declared_types: context.declared_types,
        define_model_declared_types: context.define_model_declared_types,
        type_query_declared_types: context.type_query_declared_types,
        define_model_type_query_declared_types: context.define_model_type_query_declared_types,
        keyof_type_query_declared_types: context.keyof_type_query_declared_types,
        props_type_declarations: context.props_type_declarations,
        keyof_runtime_type_declarations: context.keyof_runtime_type_declarations,
        tuple_runtime_type_declarations: context.tuple_runtime_type_declarations,
        define_model_tuple_runtime_type_declarations: context
            .define_model_tuple_runtime_type_declarations,
        array_element_runtime_type_declarations: context.array_element_runtime_type_declarations,
        define_model_array_element_runtime_type_declarations: context
            .define_model_array_element_runtime_type_declarations,
        parameter_tuple_runtime_type_declarations: context
            .parameter_tuple_runtime_type_declarations,
        define_model_parameter_tuple_runtime_type_declarations: context
            .define_model_parameter_tuple_runtime_type_declarations,
        constructor_parameter_tuple_runtime_type_declarations: context
            .constructor_parameter_tuple_runtime_type_declarations,
        define_model_constructor_parameter_tuple_runtime_type_declarations: context
            .define_model_constructor_parameter_tuple_runtime_type_declarations,
        return_type_runtime_type_declarations: context.return_type_runtime_type_declarations,
        define_model_return_type_runtime_type_declarations: context
            .define_model_return_type_runtime_type_declarations,
        props_options_type_declarations: context.props_options_type_declarations,
        return_type_props_options_declarations: context.return_type_props_options_declarations,
        generic_type_aliases: context.generic_type_aliases,
        string_literal_type_declarations: context.string_literal_type_declarations,
        ordered_string_literal_type_declarations: context.ordered_string_literal_type_declarations,
        emits_type_declarations: context.emits_type_declarations,
        type_sources: context.type_sources,
        type_direct_deps: context.type_direct_deps,
        type_deps: context.type_deps,
        unresolved_import_sources: context.unresolved_import_sources,
        silent_unresolved_type_names: context.silent_unresolved_type_names,
        type_filename: Some(descriptor.filename.clone()),
        type_resolver: type_resolver.clone(),
        ..Vue3ScriptSetupAnalysis::default()
    };
    collect_vue3_declared_types_from_statements(
        script.content.as_str(),
        &parsed.program.body,
        &mut analysis,
    );
    collect_vue3_declared_type_deps_from_statements(&parsed.program.body, &mut analysis);
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

pub(crate) fn vue3_global_type_context(
    filename: &str,
    global_type_files: &[String],
    type_resolver: &Vue3TypeResolverContext,
) -> Vue27TypeContext {
    let mut context = Vue27TypeContext::default();
    let mut seen = BTreeSet::new();
    let explicit_paths = global_type_files
        .iter()
        .map(|file| normalize_path_components(PathBuf::from(file)));
    for path in explicit_paths.chain(vue3_tsconfig_global_type_files(filename, type_resolver)) {
        if !seen.insert(vue3_external_type_context_cache_key(
            &path,
            &type_resolver.typescript_version,
        )) {
            continue;
        }
        let Some(global_context) =
            vue3_global_type_context_from_path(&path, &context, type_resolver)
        else {
            continue;
        };
        merge_vue3_type_context_missing(&mut context, global_context);
    }
    context
}

pub(crate) fn vue3_global_type_context_from_path(
    path: &Path,
    base_context: &Vue27TypeContext,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<Vue27TypeContext> {
    if !type_resolver
        .external_type_session
        .has_context_build_capacity()
    {
        return None;
    }
    let source = vue3_external_global_type_source_from_path(path, type_resolver)?;
    let initial_weight = source
        .source
        .len()
        .saturating_add(vue3_external_type_context_cache_cost(base_context));
    if !type_resolver
        .external_type_session
        .begin_uncached_context_load(initial_weight)
    {
        return None;
    }
    let normalized = normalize_path_string(path);
    let context = vue3_global_type_context_from_source(
        &source.source,
        &normalized,
        source.source_type,
        base_context,
        type_resolver,
    );
    type_resolver
        .external_type_session
        .finish_uncached_context_load(context)
}

pub(crate) fn vue3_global_type_context_from_source(
    source: &str,
    filename: &str,
    source_type: oxc_span::SourceType,
    base_context: &Vue27TypeContext,
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

    let dependency = normalize_path_string(Path::new(filename));
    let mut seed_context = base_context.clone();
    extend_vue3_type_context_from_external_imports(
        filename,
        source,
        source_type,
        &mut seed_context,
        type_resolver,
    );
    let mut analysis = Vue3ScriptSetupAnalysis {
        declared_types: seed_context.declared_types,
        define_model_declared_types: seed_context.define_model_declared_types,
        type_query_declared_types: seed_context.type_query_declared_types,
        define_model_type_query_declared_types: seed_context.define_model_type_query_declared_types,
        keyof_type_query_declared_types: seed_context.keyof_type_query_declared_types,
        props_type_declarations: seed_context.props_type_declarations,
        keyof_runtime_type_declarations: seed_context.keyof_runtime_type_declarations,
        tuple_runtime_type_declarations: seed_context.tuple_runtime_type_declarations,
        define_model_tuple_runtime_type_declarations: seed_context
            .define_model_tuple_runtime_type_declarations,
        array_element_runtime_type_declarations: seed_context
            .array_element_runtime_type_declarations,
        define_model_array_element_runtime_type_declarations: seed_context
            .define_model_array_element_runtime_type_declarations,
        parameter_tuple_runtime_type_declarations: seed_context
            .parameter_tuple_runtime_type_declarations,
        define_model_parameter_tuple_runtime_type_declarations: seed_context
            .define_model_parameter_tuple_runtime_type_declarations,
        constructor_parameter_tuple_runtime_type_declarations: seed_context
            .constructor_parameter_tuple_runtime_type_declarations,
        define_model_constructor_parameter_tuple_runtime_type_declarations: seed_context
            .define_model_constructor_parameter_tuple_runtime_type_declarations,
        return_type_runtime_type_declarations: seed_context.return_type_runtime_type_declarations,
        define_model_return_type_runtime_type_declarations: seed_context
            .define_model_return_type_runtime_type_declarations,
        props_options_type_declarations: seed_context.props_options_type_declarations,
        return_type_props_options_declarations: seed_context.return_type_props_options_declarations,
        generic_type_aliases: seed_context.generic_type_aliases,
        string_literal_type_declarations: seed_context.string_literal_type_declarations,
        ordered_string_literal_type_declarations: seed_context
            .ordered_string_literal_type_declarations,
        emits_type_declarations: seed_context.emits_type_declarations,
        type_sources: seed_context.type_sources,
        type_direct_deps: seed_context.type_direct_deps,
        type_deps: seed_context.type_deps,
        unresolved_import_sources: seed_context.unresolved_import_sources,
        silent_unresolved_type_names: seed_context.silent_unresolved_type_names,
        type_filename: Some(filename.to_string()),
        type_resolver: type_resolver.clone(),
        ..Vue3ScriptSetupAnalysis::default()
    };
    let mut global_names =
        collect_vue3_global_types_from_statements(source, &parsed.program.body, &mut analysis);
    global_names.extend(project_vue3_global_type_re_exports(
        filename,
        &parsed.program.body,
        &mut analysis,
        type_resolver,
    ));
    let global_import_names = vue3_global_type_file_import_names(&parsed.program.body);
    collect_vue3_global_type_deps_from_statements(&parsed.program.body, &mut analysis);
    seed_vue3_external_type_deps(filename, &mut analysis);
    let mut context = Vue27TypeContext {
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
    };
    retain_vue3_type_context_names(&mut context, &global_names);
    context
        .silent_unresolved_type_names
        .extend(global_import_names);
    for name in vue3_type_context_names(&context) {
        context
            .type_sources
            .entry(name.clone())
            .or_insert_with(|| dependency.clone());
        context
            .type_deps
            .entry(name)
            .or_default()
            .insert(dependency.clone());
    }
    context
}

pub(crate) fn collect_vue3_global_types_from_statements(
    source: &str,
    statements: &[Statement<'_>],
    analysis: &mut Vue3ScriptSetupAnalysis,
) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    seed_vue3_global_namespace_type_names(statements, analysis);
    let is_ambient = !statements.iter().any(|statement| {
        matches!(
            statement,
            Statement::ImportDeclaration(_)
                | Statement::ExportAllDeclaration(_)
                | Statement::ExportDefaultDeclaration(_)
                | Statement::ExportNamedDeclaration(_)
        )
    });
    if is_ambient {
        for statement in statements {
            collect_vue3_predeclared_runtime_type_from_statement(statement, analysis);
        }
        for statement in statements {
            match statement {
                Statement::TSGlobalDeclaration(global) => {
                    names.extend(vue3_declared_type_names_from_statements(&global.body.body));
                    collect_vue3_declared_types_from_statements(
                        source,
                        &global.body.body,
                        analysis,
                    );
                }
                Statement::TSModuleDeclaration(declaration)
                    if vue3_ts_module_declaration_is_global(declaration) =>
                {
                    if let Some(body) = vue3_ts_module_declaration_block_body(declaration) {
                        names.extend(vue3_declared_type_names_from_statements(body));
                        collect_vue3_declared_types_from_statements(source, body, analysis);
                    }
                }
                _ if vue3_statement_is_declare_type(statement) => {
                    names.extend(vue3_declared_type_names_from_statement(statement));
                    collect_vue3_global_declared_type_from_statement(source, statement, analysis);
                }
                _ => {}
            }
        }
        return names;
    }
    for statement in statements {
        let Statement::TSGlobalDeclaration(global) = statement else {
            continue;
        };
        names.extend(vue3_declared_type_names_from_statements(&global.body.body));
        collect_vue3_declared_types_from_statements(source, &global.body.body, analysis);
    }
    names
}

pub(crate) fn project_vue3_global_type_re_exports(
    filename: &str,
    statements: &[Statement<'_>],
    analysis: &mut Vue3ScriptSetupAnalysis,
    type_resolver: &Vue3TypeResolverContext,
) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut seen = BTreeSet::new();
    for statement in statements {
        let Statement::TSGlobalDeclaration(global) = statement else {
            continue;
        };
        names.extend(project_vue3_type_re_exports(
            filename,
            &global.body.body,
            analysis,
            &mut seen,
            type_resolver,
        ));
        project_vue3_exported_type_specifiers(&global.body.body, analysis);
        names.extend(vue3_exported_type_names(&global.body.body));
    }
    names
}

pub(crate) fn vue3_global_type_file_import_names(statements: &[Statement<'_>]) -> BTreeSet<String> {
    if !statements.iter().any(|statement| {
        matches!(
            statement,
            Statement::TSGlobalDeclaration(_)
                | Statement::ExportAllDeclaration(_)
                | Statement::ExportDefaultDeclaration(_)
                | Statement::ExportNamedDeclaration(_)
        )
    }) {
        return BTreeSet::new();
    }
    let mut names = BTreeSet::new();
    for statement in statements {
        let Statement::ImportDeclaration(import) = statement else {
            continue;
        };
        let Some(specifiers) = &import.specifiers else {
            continue;
        };
        for specifier in specifiers {
            names.insert(import_specifier_local(specifier));
        }
    }
    names
}

pub(crate) fn collect_vue3_global_declared_type_from_statement(
    source: &str,
    statement: &Statement<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    match statement {
        Statement::TSEnumDeclaration(declaration) if declaration.declare => {
            register_vue3_ts_enum_declaration(declaration, analysis);
        }
        _ => collect_vue3_declared_type_from_statement(source, statement, analysis),
    }
}

pub(crate) fn collect_vue3_global_type_deps_from_statements(
    statements: &[Statement<'_>],
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    for statement in statements {
        match statement {
            Statement::TSGlobalDeclaration(global) => {
                collect_vue3_declared_type_deps_from_statements(&global.body.body, analysis);
            }
            Statement::TSModuleDeclaration(declaration)
                if vue3_ts_module_declaration_is_global(declaration) =>
            {
                if let Some(body) = vue3_ts_module_declaration_block_body(declaration) {
                    collect_vue3_declared_type_deps_from_statements(body, analysis);
                }
            }
            _ if vue3_statement_is_declare_type(statement) => {
                collect_vue3_declared_type_deps_from_statement(statement, analysis);
            }
            _ => {}
        }
    }
}

pub(crate) fn vue3_statement_is_declare_type(statement: &Statement<'_>) -> bool {
    match statement {
        Statement::TSInterfaceDeclaration(declaration) => declaration.declare,
        Statement::TSTypeAliasDeclaration(declaration) => declaration.declare,
        Statement::TSEnumDeclaration(declaration) => declaration.declare,
        Statement::FunctionDeclaration(function) => function.declare,
        Statement::VariableDeclaration(declaration) => declaration.declare,
        Statement::ClassDeclaration(declaration) => declaration.declare,
        Statement::TSModuleDeclaration(declaration) => declaration.declare,
        _ => false,
    }
}

pub(crate) fn vue3_declared_type_names_from_statements(
    statements: &[Statement<'_>],
) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for statement in statements {
        names.extend(vue3_declared_type_names_from_statement(statement));
    }
    names
}

pub(crate) fn vue3_declared_type_names_from_statement(
    statement: &Statement<'_>,
) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    match statement {
        Statement::TSInterfaceDeclaration(declaration) => {
            names.insert(declaration.id.name.to_string());
        }
        Statement::TSTypeAliasDeclaration(declaration) => {
            names.insert(declaration.id.name.to_string());
        }
        Statement::TSEnumDeclaration(declaration) => {
            names.insert(declaration.id.name.to_string());
        }
        Statement::FunctionDeclaration(function)
            if vue3_function_has_return_projection(function) =>
        {
            if let Some(id) = &function.id {
                names.insert(id.name.to_string());
            }
        }
        Statement::VariableDeclaration(declaration) if declaration.declare => {
            for declarator in &declaration.declarations {
                if let Some(name) = first_pattern_binding(&declarator.id) {
                    names.insert(name);
                }
            }
        }
        Statement::VariableDeclaration(declaration) => {
            for declarator in &declaration.declarations {
                if vue3_variable_declarator_has_type_projection(declarator) {
                    if let Some(name) = first_pattern_binding(&declarator.id) {
                        names.insert(name);
                    }
                }
            }
        }
        Statement::ClassDeclaration(declaration) => {
            if let Some(id) = &declaration.id {
                names.insert(id.name.to_string());
            }
        }
        Statement::TSModuleDeclaration(declaration) => {
            names.extend(vue3_namespace_declared_type_names(declaration));
        }
        Statement::ExportNamedDeclaration(declaration) => {
            if let Some(declaration) = &declaration.declaration {
                names.extend(vue3_declared_type_names_from_declaration(declaration));
            }
        }
        _ => {}
    }
    names
}

pub(crate) fn vue3_declared_type_names_from_declaration(
    declaration: &Declaration<'_>,
) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
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
            names.extend(vue3_namespace_declared_type_names(declaration));
        }
        _ => {}
    }
    names
}

pub(crate) fn retain_vue3_type_context_names(
    context: &mut Vue27TypeContext,
    names: &BTreeSet<String>,
) {
    context
        .declared_types
        .retain(|name, _| names.contains(name));
    context
        .define_model_declared_types
        .retain(|name, _| names.contains(name));
    context
        .type_query_declared_types
        .retain(|name, _| names.contains(name));
    context
        .define_model_type_query_declared_types
        .retain(|name, _| names.contains(name));
    context
        .keyof_type_query_declared_types
        .retain(|name, _| names.contains(name));
    context
        .props_type_declarations
        .retain(|name, _| names.contains(name));
    context
        .keyof_runtime_type_declarations
        .retain(|name, _| names.contains(name));
    context
        .tuple_runtime_type_declarations
        .retain(|name, _| names.contains(name));
    context
        .define_model_tuple_runtime_type_declarations
        .retain(|name, _| names.contains(name));
    context
        .array_element_runtime_type_declarations
        .retain(|name, _| names.contains(name));
    context
        .define_model_array_element_runtime_type_declarations
        .retain(|name, _| names.contains(name));
    context
        .parameter_tuple_runtime_type_declarations
        .retain(|name, _| names.contains(name));
    context
        .define_model_parameter_tuple_runtime_type_declarations
        .retain(|name, _| names.contains(name));
    context
        .constructor_parameter_tuple_runtime_type_declarations
        .retain(|name, _| names.contains(name));
    context
        .define_model_constructor_parameter_tuple_runtime_type_declarations
        .retain(|name, _| names.contains(name));
    context
        .return_type_runtime_type_declarations
        .retain(|name, _| names.contains(name));
    context
        .define_model_return_type_runtime_type_declarations
        .retain(|name, _| names.contains(name));
    context
        .props_options_type_declarations
        .retain(|name, _| names.contains(name));
    context
        .return_type_props_options_declarations
        .retain(|name, _| names.contains(name));
    context
        .generic_type_aliases
        .retain(|name, _| names.contains(name));
    context
        .string_literal_type_declarations
        .retain(|name, _| names.contains(name));
    context
        .ordered_string_literal_type_declarations
        .retain(|name, _| names.contains(name));
    context
        .emits_type_declarations
        .retain(|name, _| names.contains(name));
    context.type_sources.retain(|name, _| names.contains(name));
    context
        .type_direct_deps
        .retain(|name, _| names.contains(name));
    context.type_deps.retain(|name, _| names.contains(name));
    context
        .unresolved_import_sources
        .retain(|name, _| names.contains(name));
    context
        .silent_unresolved_type_names
        .retain(|name| names.contains(name));
}

pub(crate) fn merge_vue3_type_context_missing(
    target: &mut Vue27TypeContext,
    source: Vue27TypeContext,
) {
    for (name, runtime) in source.declared_types {
        target.declared_types.entry(name).or_insert(runtime);
    }
    for (name, runtime) in source.define_model_declared_types {
        target
            .define_model_declared_types
            .entry(name)
            .or_insert(runtime);
    }
    for (name, runtime) in source.type_query_declared_types {
        target
            .type_query_declared_types
            .entry(name)
            .or_insert(runtime);
    }
    for (name, runtime) in source.define_model_type_query_declared_types {
        target
            .define_model_type_query_declared_types
            .entry(name)
            .or_insert(runtime);
    }
    for (name, runtime) in source.keyof_type_query_declared_types {
        target
            .keyof_type_query_declared_types
            .entry(name)
            .or_insert(runtime);
    }
    for (name, props) in source.props_type_declarations {
        target.props_type_declarations.entry(name).or_insert(props);
    }
    for (name, types) in source.keyof_runtime_type_declarations {
        target
            .keyof_runtime_type_declarations
            .entry(name)
            .or_insert(types);
    }
    for (name, tuple) in source.tuple_runtime_type_declarations {
        target
            .tuple_runtime_type_declarations
            .entry(name)
            .or_insert(tuple);
    }
    for (name, tuple) in source.define_model_tuple_runtime_type_declarations {
        target
            .define_model_tuple_runtime_type_declarations
            .entry(name)
            .or_insert(tuple);
    }
    for (name, types) in source.array_element_runtime_type_declarations {
        target
            .array_element_runtime_type_declarations
            .entry(name)
            .or_insert(types);
    }
    for (name, types) in source.define_model_array_element_runtime_type_declarations {
        target
            .define_model_array_element_runtime_type_declarations
            .entry(name)
            .or_insert(types);
    }
    for (name, tuple) in source.parameter_tuple_runtime_type_declarations {
        target
            .parameter_tuple_runtime_type_declarations
            .entry(name)
            .or_insert(tuple);
    }
    for (name, tuple) in source.define_model_parameter_tuple_runtime_type_declarations {
        target
            .define_model_parameter_tuple_runtime_type_declarations
            .entry(name)
            .or_insert(tuple);
    }
    for (name, tuple) in source.constructor_parameter_tuple_runtime_type_declarations {
        target
            .constructor_parameter_tuple_runtime_type_declarations
            .entry(name)
            .or_insert(tuple);
    }
    for (name, tuple) in source.define_model_constructor_parameter_tuple_runtime_type_declarations {
        target
            .define_model_constructor_parameter_tuple_runtime_type_declarations
            .entry(name)
            .or_insert(tuple);
    }
    for (name, types) in source.return_type_runtime_type_declarations {
        target
            .return_type_runtime_type_declarations
            .entry(name)
            .or_insert(types);
    }
    for (name, types) in source.define_model_return_type_runtime_type_declarations {
        target
            .define_model_return_type_runtime_type_declarations
            .entry(name)
            .or_insert(types);
    }
    for (name, props_options) in source.props_options_type_declarations {
        target
            .props_options_type_declarations
            .entry(name)
            .or_insert(props_options);
    }
    for (name, props_options) in source.return_type_props_options_declarations {
        target
            .return_type_props_options_declarations
            .entry(name)
            .or_insert(props_options);
    }
    for (name, alias) in source.generic_type_aliases {
        target.generic_type_aliases.entry(name).or_insert(alias);
    }
    for (name, keys) in source.string_literal_type_declarations {
        target
            .string_literal_type_declarations
            .entry(name)
            .or_insert(keys);
    }
    for (name, keys) in source.ordered_string_literal_type_declarations {
        target
            .ordered_string_literal_type_declarations
            .entry(name)
            .or_insert(keys);
    }
    for (name, emits) in source.emits_type_declarations {
        target.emits_type_declarations.entry(name).or_insert(emits);
    }
    for (name, type_source) in source.type_sources {
        target.type_sources.entry(name).or_insert(type_source);
    }
    for (name, deps) in source.type_direct_deps {
        target.type_direct_deps.entry(name).or_insert(deps);
    }
    for (name, deps) in source.type_deps {
        target.type_deps.entry(name).or_insert(deps);
    }
    for (name, import_source) in source.unresolved_import_sources {
        target
            .unresolved_import_sources
            .entry(name)
            .or_insert(import_source);
    }
    target
        .silent_unresolved_type_names
        .extend(source.silent_unresolved_type_names);
}
