pub(crate) fn collect_vue3_declared_types_from_statements(
    source: &str,
    statements: &[Statement<'_>],
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    for statement in statements {
        collect_vue3_predeclared_runtime_type_from_statement(statement, analysis);
    }
    for statement in statements {
        if !vue3_statement_has_deferred_type_scope(statement) {
            collect_vue3_declared_type_from_statement(source, statement, analysis);
        }
    }
    refresh_vue3_declared_type_declarations_from_statements(source, statements, analysis);
    if !statements
        .iter()
        .any(vue3_statement_has_deferred_type_scope)
    {
        return;
    }
    collect_vue3_declared_type_deps_from_statements(statements, analysis);
    for statement in statements {
        if vue3_statement_has_deferred_type_scope(statement) {
            collect_vue3_declared_type_from_statement(source, statement, analysis);
        }
    }
    refresh_vue3_declared_type_declarations_from_statements(source, statements, analysis);
}

pub(crate) fn vue3_statement_has_deferred_type_scope(statement: &Statement<'_>) -> bool {
    match statement {
        Statement::TSGlobalDeclaration(_) | Statement::TSModuleDeclaration(_) => true,
        Statement::ExportNamedDeclaration(declaration) => matches!(
            declaration.declaration.as_ref(),
            Some(Declaration::TSModuleDeclaration(_))
        ),
        _ => false,
    }
}

pub(crate) fn collect_vue3_predeclared_runtime_type_from_statement(
    statement: &Statement<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    match statement {
        Statement::ClassDeclaration(declaration) => {
            if let Some(id) = &declaration.id {
                register_vue3_class_type_name(analysis, id.name.as_str());
            }
        }
        Statement::TSEnumDeclaration(declaration) if !declaration.declare => {
            register_vue3_ts_enum_declaration(declaration, analysis);
        }
        Statement::ExportNamedDeclaration(declaration) => {
            if let Some(declaration) = &declaration.declaration {
                match declaration {
                    Declaration::ClassDeclaration(declaration) => {
                        if let Some(id) = &declaration.id {
                            register_vue3_class_type_name(analysis, id.name.as_str());
                        }
                    }
                    Declaration::TSEnumDeclaration(declaration) if !declaration.declare => {
                        register_vue3_ts_enum_declaration(declaration, analysis);
                    }
                    _ => {}
                }
            }
        }
        Statement::ExportDefaultDeclaration(declaration) => {
            if let ExportDefaultDeclarationKind::ClassDeclaration(class) = &declaration.declaration
            {
                if let Some(id) = &class.id {
                    register_vue3_class_type_name(analysis, id.name.as_str());
                }
            }
        }
        _ => {}
    }
}

pub(crate) fn collect_vue3_declared_type_deps_from_statements(
    statements: &[Statement<'_>],
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    for statement in statements {
        collect_vue3_declared_type_deps_from_statement(statement, analysis);
    }
}

pub(crate) fn collect_vue3_declared_type_deps_from_statement(
    statement: &Statement<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    match statement {
        Statement::TSInterfaceDeclaration(declaration) => {
            let deps = collect_vue3_interface_type_deps(declaration, analysis);
            insert_vue3_declared_type_deps(analysis, declaration.id.name.as_str(), deps);
        }
        Statement::TSTypeAliasDeclaration(declaration) => {
            let deps = collect_vue3_type_argument_deps(&declaration.type_annotation, analysis);
            insert_vue3_declared_type_deps(analysis, declaration.id.name.as_str(), deps);
        }
        Statement::FunctionDeclaration(function) if function.return_type.is_some() => {
            if let (Some(id), Some(return_type)) = (&function.id, function.return_type.as_ref()) {
                let deps = collect_vue3_type_argument_deps(&return_type.type_annotation, analysis);
                insert_vue3_declared_type_deps(analysis, id.name.as_str(), deps);
            }
        }
        Statement::VariableDeclaration(declaration) if declaration.declare => {
            for declarator in &declaration.declarations {
                let Some(name) = first_pattern_binding(&declarator.id) else {
                    continue;
                };
                let Some(type_annotation) = declarator.type_annotation.as_ref() else {
                    continue;
                };
                let deps =
                    collect_vue3_type_argument_deps(&type_annotation.type_annotation, analysis);
                insert_vue3_declared_type_deps(analysis, &name, deps);
            }
        }
        Statement::VariableDeclaration(declaration) => {
            collect_vue3_function_value_return_type_deps_from_variable(declaration, analysis);
            collect_vue3_static_runtime_props_options_deps_from_variable(declaration, analysis);
        }
        Statement::ExportNamedDeclaration(declaration) => {
            if let Some(declaration) = &declaration.declaration {
                collect_vue3_declared_type_deps_from_declaration(declaration, analysis);
            }
        }
        Statement::ExportDefaultDeclaration(declaration) => {
            if let ExportDefaultDeclarationKind::TSInterfaceDeclaration(declaration) =
                &declaration.declaration
            {
                let deps = collect_vue3_interface_type_deps(declaration, analysis);
                insert_vue3_declared_type_deps(analysis, declaration.id.name.as_str(), deps);
            }
        }
        _ => {}
    }
}

pub(crate) fn collect_vue3_declared_type_deps_from_declaration(
    declaration: &Declaration<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    match declaration {
        Declaration::TSInterfaceDeclaration(declaration) => {
            let deps = collect_vue3_interface_type_deps(declaration, analysis);
            insert_vue3_declared_type_deps(analysis, declaration.id.name.as_str(), deps);
        }
        Declaration::TSTypeAliasDeclaration(declaration) => {
            let deps = collect_vue3_type_argument_deps(&declaration.type_annotation, analysis);
            insert_vue3_declared_type_deps(analysis, declaration.id.name.as_str(), deps);
        }
        Declaration::FunctionDeclaration(function) if function.return_type.is_some() => {
            if let (Some(id), Some(return_type)) = (&function.id, function.return_type.as_ref()) {
                let deps = collect_vue3_type_argument_deps(&return_type.type_annotation, analysis);
                insert_vue3_declared_type_deps(analysis, id.name.as_str(), deps);
            }
        }
        Declaration::VariableDeclaration(declaration) if declaration.declare => {
            for declarator in &declaration.declarations {
                let Some(name) = first_pattern_binding(&declarator.id) else {
                    continue;
                };
                let Some(type_annotation) = declarator.type_annotation.as_ref() else {
                    continue;
                };
                let deps =
                    collect_vue3_type_argument_deps(&type_annotation.type_annotation, analysis);
                insert_vue3_declared_type_deps(analysis, &name, deps);
            }
        }
        Declaration::VariableDeclaration(declaration) => {
            collect_vue3_function_value_return_type_deps_from_variable(declaration, analysis);
            collect_vue3_static_runtime_props_options_deps_from_variable(declaration, analysis);
        }
        _ => {}
    }
}

pub(crate) fn collect_vue3_interface_type_deps(
    declaration: &TSInterfaceDeclaration<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> BTreeSet<String> {
    let mut deps = BTreeSet::new();
    for heritage in &declaration.extends {
        if let Some(name) = vue3_interface_heritage_name(heritage) {
            collect_vue3_named_type_deps(&name, analysis, &mut deps);
        }
        if let Some(type_arguments) = heritage.type_arguments.as_ref() {
            for ty in &type_arguments.params {
                collect_vue3_type_argument_deps_into(ty, analysis, &mut deps);
            }
        }
    }
    for signature in &declaration.body.body {
        collect_vue3_signature_type_deps(signature, analysis, &mut deps);
    }
    deps
}

pub(crate) fn insert_vue3_declared_type_deps(
    analysis: &mut Vue3ScriptSetupAnalysis,
    name: &str,
    deps: BTreeSet<String>,
) {
    if !deps.is_empty() {
        analysis
            .type_deps
            .entry(name.to_string())
            .or_default()
            .extend(deps);
    }
}

pub(crate) fn collect_vue3_declared_type_from_statement(
    source: &str,
    statement: &Statement<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    match statement {
        Statement::TSInterfaceDeclaration(declaration) => {
            register_vue3_interface_declaration(source, declaration, analysis);
        }
        Statement::TSTypeAliasDeclaration(declaration) => {
            register_vue3_type_alias_declaration(source, declaration, analysis);
        }
        Statement::FunctionDeclaration(function)
            if vue3_function_has_return_projection(function) =>
        {
            register_vue3_declared_function_return_props_options(source, function, analysis);
        }
        Statement::VariableDeclaration(declaration) if declaration.declare => {
            register_vue3_declared_variable_props_options(source, declaration, analysis);
        }
        Statement::VariableDeclaration(declaration) => {
            register_vue3_function_value_return_props_options(source, declaration, analysis);
            register_vue3_static_runtime_props_options(source, declaration, analysis);
        }
        Statement::ExportNamedDeclaration(declaration) => {
            if let Some(declaration) = &declaration.declaration {
                collect_vue3_declared_type_from_declaration(source, declaration, analysis);
            }
        }
        Statement::ClassDeclaration(declaration) => {
            if let Some(id) = &declaration.id {
                register_vue3_class_type_name(analysis, id.name.as_str());
            }
        }
        Statement::TSEnumDeclaration(declaration) if !declaration.declare => {
            register_vue3_ts_enum_declaration(declaration, analysis);
        }
        Statement::TSModuleDeclaration(declaration) => {
            project_vue3_namespace_declaration(source, declaration, analysis);
        }
        _ => {}
    }
}

pub(crate) fn collect_vue3_declared_type_from_declaration(
    source: &str,
    declaration: &Declaration<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    match declaration {
        Declaration::TSInterfaceDeclaration(declaration) => {
            register_vue3_interface_declaration(source, declaration, analysis);
        }
        Declaration::TSTypeAliasDeclaration(declaration) => {
            register_vue3_type_alias_declaration(source, declaration, analysis);
        }
        Declaration::FunctionDeclaration(function)
            if vue3_function_has_return_projection(function) =>
        {
            register_vue3_declared_function_return_props_options(source, function, analysis);
        }
        Declaration::VariableDeclaration(declaration) if declaration.declare => {
            register_vue3_declared_variable_props_options(source, declaration, analysis);
        }
        Declaration::VariableDeclaration(declaration) => {
            register_vue3_function_value_return_props_options(source, declaration, analysis);
            register_vue3_static_runtime_props_options(source, declaration, analysis);
        }
        Declaration::TSModuleDeclaration(declaration) => {
            project_vue3_namespace_declaration(source, declaration, analysis);
        }
        Declaration::ClassDeclaration(declaration) => {
            if let Some(id) = &declaration.id {
                register_vue3_class_type_name(analysis, id.name.as_str());
            }
        }
        Declaration::TSEnumDeclaration(declaration) if !declaration.declare => {
            register_vue3_ts_enum_declaration(declaration, analysis);
        }
        _ => {}
    }
}

pub(crate) fn refresh_vue3_declared_type_declarations_from_statements(
    source: &str,
    statements: &[Statement<'_>],
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    let limit = count_vue3_refreshable_type_declarations_in_statements(statements);
    for _ in 0..limit {
        if !refresh_vue3_declared_type_declarations_from_statements_once(
            source, statements, analysis,
        ) {
            break;
        }
    }
}

pub(crate) fn refresh_vue3_declared_type_declarations_from_statements_once(
    source: &str,
    statements: &[Statement<'_>],
    analysis: &mut Vue3ScriptSetupAnalysis,
) -> bool {
    let mut changed = false;
    let interface_declarations = vue3_interface_declarations_by_name(statements);
    let mut refreshed_interfaces = BTreeSet::new();
    for statement in statements {
        changed |= refresh_vue3_declared_type_declaration_from_statement(
            source,
            statement,
            &interface_declarations,
            &mut refreshed_interfaces,
            analysis,
        );
    }
    changed
}

pub(crate) fn vue3_interface_declarations_by_name<'a>(
    statements: &'a [Statement<'a>],
) -> BTreeMap<String, Vec<&'a TSInterfaceDeclaration<'a>>> {
    let mut declarations = BTreeMap::new();
    for statement in statements {
        collect_vue3_interface_declarations_from_statement(statement, &mut declarations);
    }
    declarations
}

pub(crate) fn collect_vue3_interface_declarations_from_statement<'a>(
    statement: &'a Statement<'a>,
    declarations: &mut BTreeMap<String, Vec<&'a TSInterfaceDeclaration<'a>>>,
) {
    match statement {
        Statement::TSInterfaceDeclaration(declaration) => {
            declarations
                .entry(declaration.id.name.to_string())
                .or_default()
                .push(declaration);
        }
        Statement::ExportNamedDeclaration(declaration) => {
            if let Some(Declaration::TSInterfaceDeclaration(declaration)) = &declaration.declaration
            {
                declarations
                    .entry(declaration.id.name.to_string())
                    .or_default()
                    .push(declaration);
            }
        }
        _ => {}
    }
}

pub(crate) fn refresh_vue3_declared_type_declaration_from_statement(
    source: &str,
    statement: &Statement<'_>,
    interface_declarations: &BTreeMap<String, Vec<&TSInterfaceDeclaration<'_>>>,
    refreshed_interfaces: &mut BTreeSet<String>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) -> bool {
    match statement {
        Statement::TSInterfaceDeclaration(declaration) => refresh_vue3_interface_declaration_group(
            source,
            declaration,
            interface_declarations,
            refreshed_interfaces,
            analysis,
        ),
        Statement::TSTypeAliasDeclaration(declaration) => {
            refresh_vue3_type_alias_declaration(source, declaration, analysis)
        }
        Statement::ExportNamedDeclaration(declaration) => {
            declaration.declaration.as_ref().is_some_and(|declaration| {
                refresh_vue3_declared_type_declaration_from_declaration(
                    source,
                    declaration,
                    interface_declarations,
                    refreshed_interfaces,
                    analysis,
                )
            })
        }
        _ => false,
    }
}

pub(crate) fn refresh_vue3_declared_type_declaration_from_declaration(
    source: &str,
    declaration: &Declaration<'_>,
    interface_declarations: &BTreeMap<String, Vec<&TSInterfaceDeclaration<'_>>>,
    refreshed_interfaces: &mut BTreeSet<String>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) -> bool {
    match declaration {
        Declaration::TSInterfaceDeclaration(declaration) => {
            refresh_vue3_interface_declaration_group(
                source,
                declaration,
                interface_declarations,
                refreshed_interfaces,
                analysis,
            )
        }
        Declaration::TSTypeAliasDeclaration(declaration) => {
            refresh_vue3_type_alias_declaration(source, declaration, analysis)
        }
        _ => false,
    }
}

pub(crate) fn count_vue3_refreshable_type_declarations_in_statements(
    statements: &[Statement<'_>],
) -> usize {
    statements
        .iter()
        .map(count_vue3_refreshable_type_declarations_in_statement)
        .sum()
}

pub(crate) fn count_vue3_refreshable_type_declarations_in_statement(
    statement: &Statement<'_>,
) -> usize {
    match statement {
        Statement::TSInterfaceDeclaration(_) | Statement::TSTypeAliasDeclaration(_) => 1,
        Statement::ExportNamedDeclaration(declaration) => declaration
            .declaration
            .as_ref()
            .map(count_vue3_refreshable_type_declarations_in_declaration)
            .unwrap_or_default(),
        _ => 0,
    }
}

pub(crate) fn count_vue3_refreshable_type_declarations_in_declaration(
    declaration: &Declaration<'_>,
) -> usize {
    match declaration {
        Declaration::TSInterfaceDeclaration(_) | Declaration::TSTypeAliasDeclaration(_) => 1,
        _ => 0,
    }
}
