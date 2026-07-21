const VUE3_MAX_DECLARED_TYPE_DEPENDENCY_WORK: usize = 64 * 1024 * 1024;

pub(crate) fn collect_vue3_declared_types_from_statements(
    source: &str,
    statements: &[Statement<'_>],
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    let mut namespace_budget = Vue3NamespaceProjectionBudget::default();
    collect_vue3_declared_types_from_statements_with_namespace_budget(
        source,
        statements,
        false,
        0,
        analysis,
        &mut namespace_budget,
    );
}

pub(crate) fn collect_vue3_declared_types_from_statements_with_namespace_budget(
    source: &str,
    statements: &[Statement<'_>],
    ambient: bool,
    namespace_depth: usize,
    analysis: &mut Vue3ScriptSetupAnalysis,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
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
    if analysis.type_dependency_work_exhausted {
        namespace_budget.exhausted = true;
        return;
    }
    project_vue3_namespace_groups_from_statements_with_budget(
        source,
        statements,
        ambient,
        namespace_depth,
        analysis,
        namespace_budget,
    );
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
) -> bool {
    collect_vue3_declared_type_deps_from_statement_groups(&[statements], analysis)
}

pub(crate) fn collect_vue3_declared_type_deps_from_statement_groups(
    statement_groups: &[&[Statement<'_>]],
    analysis: &mut Vue3ScriptSetupAnalysis,
) -> bool {
    collect_vue3_declared_type_deps_from_statement_groups_with_limit(
        statement_groups,
        analysis,
        VUE3_MAX_DECLARED_TYPE_DEPENDENCY_WORK,
        &BTreeSet::new(),
    )
}

pub(crate) fn collect_vue3_declared_type_deps_from_statement_groups_excluding_names(
    statement_groups: &[&[Statement<'_>]],
    excluded_names: &BTreeSet<String>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) -> bool {
    collect_vue3_declared_type_deps_from_statement_groups_with_limit(
        statement_groups,
        analysis,
        VUE3_MAX_DECLARED_TYPE_DEPENDENCY_WORK,
        excluded_names,
    )
}

fn collect_vue3_declared_type_deps_from_statement_groups_with_limit(
    statement_groups: &[&[Statement<'_>]],
    analysis: &mut Vue3ScriptSetupAnalysis,
    mut remaining_work: usize,
    excluded_names: &BTreeSet<String>,
) -> bool {
    if analysis.type_dependency_work_exhausted {
        return false;
    }
    let mut names = BTreeSet::new();
    let mut syntax_work = 0usize;
    for statements in statement_groups {
        names.extend(vue3_declared_type_dependency_names_from_statements(
            statements,
        ));
        syntax_work = syntax_work.saturating_add(
            statements
                .first()
                .zip(statements.last())
                .map_or(0usize, |(first, last)| {
                    (last.span().end as usize).saturating_sub(first.span().start as usize)
                }),
        );
    }
    names.retain(|name| !excluded_names.contains(name));
    syntax_work = syntax_work.saturating_add(names.len());
    let mut changed = false;
    for _ in 0..names.len().saturating_add(1) {
        let dependency_work = names.iter().fold(0usize, |work, name| {
            analysis.type_deps.get(name).map_or(work, |deps| {
                deps.iter().fold(work, |work, dependency| {
                    work.saturating_add(dependency.len()).saturating_add(1)
                })
            })
        });
        let iteration_work = syntax_work.saturating_add(dependency_work);
        let Some(next_remaining_work) = remaining_work.checked_sub(iteration_work) else {
            analysis.type_dependency_work_exhausted = true;
            record_vue3_conservative_type_dependencies(analysis);
            changed = true;
            break;
        };
        remaining_work = next_remaining_work;
        let iteration_changed = collect_vue3_declared_type_deps_from_statement_groups_once(
            statement_groups,
            &names,
            analysis,
        );
        changed |= iteration_changed;
        if !iteration_changed {
            break;
        }
    }
    changed
}

fn record_vue3_conservative_type_dependencies(analysis: &mut Vue3ScriptSetupAnalysis) {
    let dependencies = analysis
        .type_sources
        .values()
        .cloned()
        .chain(
            analysis
                .type_direct_deps
                .values()
                .flat_map(|dependencies| dependencies.iter().cloned()),
        )
        .chain(
            analysis
                .type_deps
                .values()
                .flat_map(|dependencies| dependencies.iter().cloned()),
        )
        .collect::<BTreeSet<_>>();
    for dependency in dependencies {
        push_unique(&mut analysis.deps, &dependency);
    }
}

fn collect_vue3_declared_type_deps_from_statement_groups_once(
    statement_groups: &[&[Statement<'_>]],
    names: &BTreeSet<String>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) -> bool {
    let mut next = BTreeMap::new();
    for statements in statement_groups {
        for statement in *statements {
            collect_vue3_declared_type_deps_from_statement_into(statement, analysis, &mut next);
        }
    }
    let changed = names
        .iter()
        .any(|name| analysis.type_deps.get(name) != next.get(name));
    for name in names {
        match next.remove(name) {
            Some(deps) => {
                analysis.type_deps.insert(name.clone(), deps);
            }
            None => {
                analysis.type_deps.remove(name);
            }
        }
    }
    changed
}

fn vue3_declared_type_dependency_names_from_statements(
    statements: &[Statement<'_>],
) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for statement in statements {
        if !vue3_statement_has_deferred_type_scope(statement) {
            names.extend(vue3_declared_type_names_from_statement(statement));
        }
        if let Statement::ExportDefaultDeclaration(export) = statement {
            if let ExportDefaultDeclarationKind::TSInterfaceDeclaration(declaration) =
                &export.declaration
            {
                names.insert(declaration.id.name.to_string());
            }
        }
    }
    names
}

fn collect_vue3_declared_type_deps_from_statement_into(
    statement: &Statement<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    target: &mut BTreeMap<String, BTreeSet<String>>,
) {
    match statement {
        Statement::TSInterfaceDeclaration(declaration) => {
            let deps = collect_vue3_interface_type_deps(declaration, analysis);
            insert_vue3_declared_type_deps_into(target, declaration.id.name.as_str(), deps);
        }
        Statement::TSTypeAliasDeclaration(declaration) => {
            let deps = collect_vue3_type_argument_deps(&declaration.type_annotation, analysis);
            insert_vue3_declared_type_deps_into(target, declaration.id.name.as_str(), deps);
        }
        Statement::FunctionDeclaration(function) if function.return_type.is_some() => {
            if let (Some(id), Some(return_type)) = (&function.id, function.return_type.as_ref()) {
                let deps = collect_vue3_type_argument_deps(&return_type.type_annotation, analysis);
                insert_vue3_declared_type_deps_into(target, id.name.as_str(), deps);
            }
        }
        Statement::VariableDeclaration(declaration) if declaration.declare => {
            collect_vue3_declared_variable_type_deps_into(declaration, analysis, target);
        }
        Statement::VariableDeclaration(declaration) => {
            collect_vue3_projected_variable_type_deps_into(declaration, analysis, target);
        }
        Statement::ExportNamedDeclaration(export) => {
            if let Some(declaration) = &export.declaration {
                collect_vue3_declared_type_deps_from_declaration_into(
                    declaration,
                    analysis,
                    target,
                );
            }
        }
        Statement::ExportDefaultDeclaration(export) => {
            if let ExportDefaultDeclarationKind::TSInterfaceDeclaration(declaration) =
                &export.declaration
            {
                let deps = collect_vue3_interface_type_deps(declaration, analysis);
                insert_vue3_declared_type_deps_into(target, declaration.id.name.as_str(), deps);
            }
        }
        _ => {}
    }
}

fn collect_vue3_declared_type_deps_from_declaration_into(
    declaration: &Declaration<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    target: &mut BTreeMap<String, BTreeSet<String>>,
) {
    match declaration {
        Declaration::TSInterfaceDeclaration(declaration) => {
            let deps = collect_vue3_interface_type_deps(declaration, analysis);
            insert_vue3_declared_type_deps_into(target, declaration.id.name.as_str(), deps);
        }
        Declaration::TSTypeAliasDeclaration(declaration) => {
            let deps = collect_vue3_type_argument_deps(&declaration.type_annotation, analysis);
            insert_vue3_declared_type_deps_into(target, declaration.id.name.as_str(), deps);
        }
        Declaration::FunctionDeclaration(function) if function.return_type.is_some() => {
            if let (Some(id), Some(return_type)) = (&function.id, function.return_type.as_ref()) {
                let deps = collect_vue3_type_argument_deps(&return_type.type_annotation, analysis);
                insert_vue3_declared_type_deps_into(target, id.name.as_str(), deps);
            }
        }
        Declaration::VariableDeclaration(declaration) if declaration.declare => {
            collect_vue3_declared_variable_type_deps_into(declaration, analysis, target);
        }
        Declaration::VariableDeclaration(declaration) => {
            collect_vue3_projected_variable_type_deps_into(declaration, analysis, target);
        }
        _ => {}
    }
}

fn collect_vue3_declared_variable_type_deps_into(
    declaration: &VariableDeclaration<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    target: &mut BTreeMap<String, BTreeSet<String>>,
) {
    for declarator in &declaration.declarations {
        let Some(name) = first_pattern_binding(&declarator.id) else {
            continue;
        };
        let Some(type_annotation) = declarator.type_annotation.as_ref() else {
            continue;
        };
        let deps = collect_vue3_type_argument_deps(&type_annotation.type_annotation, analysis);
        insert_vue3_declared_type_deps_into(target, &name, deps);
    }
}

fn collect_vue3_projected_variable_type_deps_into(
    declaration: &VariableDeclaration<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    target: &mut BTreeMap<String, BTreeSet<String>>,
) {
    for declarator in &declaration.declarations {
        let Some(name) = first_pattern_binding(&declarator.id) else {
            continue;
        };
        if let Some(return_type) = vue3_variable_declarator_function_return_type(declarator) {
            let deps = collect_vue3_type_argument_deps(return_type, analysis);
            insert_vue3_declared_type_deps_into(target, &name, deps);
        }
        let Some(init) = declarator.init.as_ref() else {
            continue;
        };
        if analysis.props_options_type_declarations.contains_key(&name) {
            let deps = collect_vue3_static_runtime_props_options_deps(init, analysis);
            insert_vue3_declared_type_deps_into(target, &name, deps);
        }
    }
}

fn insert_vue3_declared_type_deps_into(
    target: &mut BTreeMap<String, BTreeSet<String>>,
    name: &str,
    deps: BTreeSet<String>,
) {
    if !deps.is_empty() {
        target.entry(name.to_string()).or_default().extend(deps);
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

pub(crate) fn refresh_vue3_declared_type_declarations_excluding_interfaces(
    source: &str,
    statements: &[Statement<'_>],
    excluded_interfaces: &BTreeSet<String>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    refresh_vue3_declared_type_declarations_from_statement_groups_excluding_interfaces(
        source,
        &[statements],
        excluded_interfaces,
        analysis,
    );
}

pub(crate) fn refresh_vue3_declared_type_declarations_from_statement_groups_excluding_interfaces(
    source: &str,
    statement_groups: &[&[Statement<'_>]],
    excluded_interfaces: &BTreeSet<String>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) -> bool {
    let limit = statement_groups.iter().fold(0usize, |count, statements| {
        count.saturating_add(count_vue3_refreshable_type_declarations_in_statements(
            statements,
        ))
    });
    let mut interface_declarations = BTreeMap::new();
    for statements in statement_groups {
        for statement in *statements {
            collect_vue3_interface_declarations_from_statement(
                statement,
                &mut interface_declarations,
            );
        }
    }
    let mut any_changed = false;
    for _ in 0..limit {
        let mut refreshed_interfaces = BTreeSet::new();
        let mut changed = false;
        for statements in statement_groups {
            for statement in *statements {
                if vue3_statement_declares_interface_named(statement, excluded_interfaces) {
                    continue;
                }
                changed |= refresh_vue3_declared_type_declaration_from_statement(
                    source,
                    statement,
                    &interface_declarations,
                    &mut refreshed_interfaces,
                    analysis,
                );
            }
        }
        if !changed {
            break;
        }
        any_changed = true;
    }
    any_changed
}

fn vue3_statement_declares_interface_named(
    statement: &Statement<'_>,
    names: &BTreeSet<String>,
) -> bool {
    match statement {
        Statement::TSInterfaceDeclaration(declaration) => {
            names.contains(declaration.id.name.as_str())
        }
        Statement::ExportNamedDeclaration(export) => export
            .declaration
            .as_ref()
            .is_some_and(|declaration| match declaration {
                Declaration::TSInterfaceDeclaration(declaration) => {
                    names.contains(declaration.id.name.as_str())
                }
                _ => false,
            }),
        _ => false,
    }
}

pub(crate) fn refresh_vue3_declared_type_declarations_from_statements_once(
    source: &str,
    statements: &[Statement<'_>],
    analysis: &mut Vue3ScriptSetupAnalysis,
) -> bool {
    refresh_vue3_declared_type_declarations_from_statement_groups_once(
        source,
        &[statements],
        analysis,
    )
}

pub(crate) fn refresh_vue3_declared_type_declarations_from_statement_groups_once(
    source: &str,
    statement_groups: &[&[Statement<'_>]],
    analysis: &mut Vue3ScriptSetupAnalysis,
) -> bool {
    let mut changed = false;
    let mut interface_declarations = BTreeMap::new();
    for statements in statement_groups {
        for statement in *statements {
            collect_vue3_interface_declarations_from_statement(
                statement,
                &mut interface_declarations,
            );
        }
    }
    let mut refreshed_interfaces = BTreeSet::new();
    for statements in statement_groups {
        for statement in *statements {
            changed |= refresh_vue3_declared_type_declaration_from_statement(
                source,
                statement,
                &interface_declarations,
                &mut refreshed_interfaces,
                analysis,
            );
        }
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
        Statement::FunctionDeclaration(function)
            if vue3_function_has_return_projection(function) =>
        {
            refresh_vue3_named_type_projections(
                vue3_declared_type_names_from_statement(statement),
                analysis,
                |analysis| {
                    register_vue3_declared_function_return_props_options(
                        source, function, analysis,
                    );
                },
            )
        }
        Statement::VariableDeclaration(declaration) => refresh_vue3_variable_type_projections(
            source,
            statement,
            declaration,
            analysis,
        ),
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
        Declaration::FunctionDeclaration(function)
            if vue3_function_has_return_projection(function) =>
        {
            refresh_vue3_named_type_projections(
                vue3_declared_type_names_from_declaration(declaration),
                analysis,
                |analysis| {
                    register_vue3_declared_function_return_props_options(
                        source, function, analysis,
                    );
                },
            )
        }
        Declaration::VariableDeclaration(variable) => {
            let names = vue3_declared_type_names_from_declaration(declaration);
            refresh_vue3_named_type_projections(names, analysis, |analysis| {
                if variable.declare {
                    register_vue3_declared_variable_props_options(source, variable, analysis);
                } else {
                    register_vue3_function_value_return_props_options(
                        source, variable, analysis,
                    );
                    register_vue3_static_runtime_props_options(source, variable, analysis);
                }
            })
        }
        _ => false,
    }
}

fn refresh_vue3_variable_type_projections(
    source: &str,
    statement: &Statement<'_>,
    declaration: &VariableDeclaration<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) -> bool {
    refresh_vue3_named_type_projections(
        vue3_declared_type_names_from_statement(statement),
        analysis,
        |analysis| {
            if declaration.declare {
                register_vue3_declared_variable_props_options(source, declaration, analysis);
            } else {
                register_vue3_function_value_return_props_options(
                    source,
                    declaration,
                    analysis,
                );
                register_vue3_static_runtime_props_options(source, declaration, analysis);
            }
        },
    )
}

fn refresh_vue3_named_type_projections(
    names: BTreeSet<String>,
    analysis: &mut Vue3ScriptSetupAnalysis,
    refresh: impl FnOnce(&mut Vue3ScriptSetupAnalysis),
) -> bool {
    let mut previous = names
        .iter()
        .map(|name| {
            let mut projection = Vue3ScriptSetupAnalysis::default();
            sync_vue3_type_alias_from_analysis(&mut projection, analysis, name, name);
            (name.clone(), projection)
        })
        .collect::<BTreeMap<_, _>>();
    refresh(analysis);
    previous.iter_mut().any(|(name, projection)| {
        sync_vue3_type_alias_from_analysis(projection, analysis, name, name)
    })
}

pub(crate) fn count_vue3_refreshable_type_declarations_in_statements(
    statements: &[Statement<'_>],
) -> usize {
    statements
        .iter()
        .map(count_vue3_refreshable_type_declarations_in_statement)
        .fold(0usize, usize::saturating_add)
}

pub(crate) fn count_vue3_refreshable_type_declarations_in_statement(
    statement: &Statement<'_>,
) -> usize {
    match statement {
        Statement::TSInterfaceDeclaration(_) | Statement::TSTypeAliasDeclaration(_) => 1,
        Statement::FunctionDeclaration(function)
            if vue3_function_has_return_projection(function) =>
        {
            usize::from(function.id.is_some())
        }
        Statement::VariableDeclaration(_) => {
            vue3_declared_type_names_from_statement(statement).len()
        }
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
        Declaration::FunctionDeclaration(function)
            if vue3_function_has_return_projection(function) =>
        {
            usize::from(function.id.is_some())
        }
        Declaration::VariableDeclaration(_) => {
            vue3_declared_type_names_from_declaration(declaration).len()
        }
        _ => 0,
    }
}

#[cfg(test)]
mod declared_type_dependency_work_tests {
    use super::*;

    #[test]
    fn dependency_work_exhaustion_preserves_state_and_records_conservative_deps() {
        let source = "type Props = Imported";
        let allocator = oxc_allocator::Allocator::default();
        let parsed = oxc_parser::Parser::new(
            &allocator,
            source,
            oxc_span::SourceType::ts(),
        )
        .parse();
        assert!(!parsed.panicked && parsed.errors.is_empty());
        let mut analysis = Vue3ScriptSetupAnalysis::default();
        analysis
            .type_sources
            .insert("Imported".into(), "leaf.ts".into());
        analysis
            .type_deps
            .insert("Stable".into(), ["stable.ts".into()].into_iter().collect());
        let expected_type_deps = analysis.type_deps.clone();

        assert!(
            collect_vue3_declared_type_deps_from_statement_groups_with_limit(
                &[&parsed.program.body],
                &mut analysis,
                0,
                &BTreeSet::new(),
            )
        );

        assert!(analysis.type_dependency_work_exhausted);
        assert_eq!(analysis.type_deps, expected_type_deps);
        assert_eq!(
            analysis.deps.iter().cloned().collect::<BTreeSet<_>>(),
            ["leaf.ts".to_string(), "stable.ts".to_string()]
                .into_iter()
                .collect()
        );
    }
}
