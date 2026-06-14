pub(crate) fn vue27_ts_type_name_identifier<'a>(name: &'a TSTypeName<'a>) -> Option<&'a str> {
    match name {
        TSTypeName::IdentifierReference(identifier) => Some(identifier.name.as_str()),
        _ => None,
    }
}

pub(crate) fn vue3_ts_type_name_key(name: &TSTypeName<'_>) -> Option<String> {
    match name {
        TSTypeName::IdentifierReference(identifier) => Some(identifier.name.to_string()),
        TSTypeName::QualifiedName(qualified) => {
            let left = vue3_ts_type_name_key(&qualified.left)?;
            Some(format!("{left}.{}", qualified.right.name))
        }
        TSTypeName::ThisExpression(_) => None,
    }
}

pub(crate) fn vue3_type_query_name_key(query: &TSTypeQuery<'_>) -> Option<String> {
    match &query.expr_name {
        TSTypeQueryExprName::IdentifierReference(identifier) => Some(identifier.name.to_string()),
        TSTypeQueryExprName::QualifiedName(qualified) => {
            let left = vue3_ts_type_name_key(&qualified.left)?;
            Some(format!("{left}.{}", qualified.right.name))
        }
        TSTypeQueryExprName::TSImportType(_) | TSTypeQueryExprName::ThisExpression(_) => None,
    }
}

pub(crate) fn vue3_resolve_type_query_import_type(
    query: &TSTypeQuery<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vue3ResolvedImportType> {
    match &query.expr_name {
        TSTypeQueryExprName::TSImportType(import_type) => {
            vue3_resolve_import_type(import_type, analysis)
        }
        _ => None,
    }
}

pub(crate) fn vue3_type_query_props_options_declaration(
    query: &TSTypeQuery<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vue27TypeMembers> {
    if let Some(name) = vue3_type_query_name_key(query) {
        return analysis.props_options_type_declarations.get(&name).cloned();
    }
    let resolved = vue3_resolve_type_query_import_type(query, analysis)?;
    resolved
        .context
        .props_options_type_declarations
        .get(&resolved.name)
        .cloned()
}

pub(crate) fn vue3_type_query_return_props_options_declaration(
    query: &TSTypeQuery<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vue27TypeMembers> {
    if let Some(name) = vue3_type_query_name_key(query) {
        return analysis
            .return_type_props_options_declarations
            .get(&name)
            .cloned();
    }
    let resolved = vue3_resolve_type_query_import_type(query, analysis)?;
    resolved
        .context
        .return_type_props_options_declarations
        .get(&resolved.name)
        .cloned()
}

pub(crate) fn vue3_type_query_runtime_type_declaration(
    query: &TSTypeQuery<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vec<String>> {
    if let Some(name) = vue3_type_query_name_key(query) {
        return analysis.type_query_declared_types.get(&name).cloned();
    }
    let resolved = vue3_resolve_type_query_import_type(query, analysis)?;
    resolved
        .context
        .type_query_declared_types
        .get(&resolved.name)
        .cloned()
}

pub(crate) fn vue3_type_query_define_model_runtime_type_declaration(
    query: &TSTypeQuery<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vec<String>> {
    if let Some(name) = vue3_type_query_name_key(query) {
        return analysis
            .define_model_type_query_declared_types
            .get(&name)
            .cloned();
    }
    let resolved = vue3_resolve_type_query_import_type(query, analysis)?;
    resolved
        .context
        .define_model_type_query_declared_types
        .get(&resolved.name)
        .cloned()
}

pub(crate) fn vue3_type_query_keyof_runtime_type_declaration(
    query: &TSTypeQuery<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vec<String>> {
    if let Some(name) = vue3_type_query_name_key(query) {
        return analysis.keyof_type_query_declared_types.get(&name).cloned();
    }
    let resolved = vue3_resolve_type_query_import_type(query, analysis)?;
    resolved
        .context
        .keyof_type_query_declared_types
        .get(&resolved.name)
        .cloned()
}

pub(crate) fn vue3_return_type_declaration_for_type_query(
    query: &TSTypeQuery<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vec<String>> {
    if let Some(name) = vue3_type_query_name_key(query) {
        return vue3_return_type_declaration_for_mode(analysis, &name, mode);
    }
    let resolved = vue3_resolve_type_query_import_type(query, analysis)?;
    vue3_return_type_declaration_for_context(&resolved.context, &resolved.name, mode)
}

pub(crate) fn collect_vue3_type_query_deps(
    query: &TSTypeQuery<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    deps: &mut BTreeSet<String>,
) {
    if let Some(name) = vue3_type_query_name_key(query) {
        collect_vue3_named_type_deps(&name, analysis, deps);
    } else if let Some(resolved) = vue3_resolve_type_query_import_type(query, analysis) {
        deps.extend(
            resolved
                .context
                .type_deps
                .get(&resolved.name)
                .cloned()
                .unwrap_or_default(),
        );
        deps.insert(resolved.dependency);
    }
    if let Some(type_arguments) = query.type_arguments.as_ref() {
        for ty in &type_arguments.params {
            collect_vue3_type_argument_deps_into(ty, analysis, deps);
        }
    }
}

pub(crate) fn collect_vue3_type_query_deps_ordered(
    query: &TSTypeQuery<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    deps: &mut Vec<String>,
) {
    if let Some(name) = vue3_type_query_name_key(query) {
        collect_vue3_named_type_deps_ordered(&name, analysis, deps);
    } else if let Some(resolved) = vue3_resolve_type_query_import_type(query, analysis) {
        push_unique(deps, &resolved.dependency);
        collect_vue3_context_type_deps_ordered(&resolved.context, &resolved.name, deps);
    }
    if let Some(type_arguments) = query.type_arguments.as_ref() {
        for ty in &type_arguments.params {
            collect_vue3_type_argument_deps_ordered_into(ty, analysis, deps);
        }
    }
}

pub(crate) fn vue3_import_type_qualifier_key(qualifier: &TSImportTypeQualifier<'_>) -> String {
    match qualifier {
        TSImportTypeQualifier::Identifier(identifier) => identifier.name.to_string(),
        TSImportTypeQualifier::QualifiedName(qualified) => {
            let left = vue3_import_type_qualifier_key(&qualified.left);
            format!("{left}.{}", qualified.right.name)
        }
    }
}

pub(crate) fn escape_js_double(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
