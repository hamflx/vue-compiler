pub(crate) fn first_pattern_binding(pattern: &BindingPattern<'_>) -> Option<String> {
    first_pattern_binding_name(pattern).map(str::to_string)
}

pub(crate) fn first_pattern_binding_name<'a>(
    pattern: &'a BindingPattern<'_>,
) -> Option<&'a str> {
    match pattern {
        BindingPattern::BindingIdentifier(identifier) => Some(identifier.name.as_str()),
        BindingPattern::ObjectPattern(pattern) => pattern
            .properties
            .iter()
            .find_map(|property| first_pattern_binding_name(&property.value))
            .or_else(|| {
                pattern
                    .rest
                    .as_ref()
                    .and_then(|rest| first_pattern_binding_name(&rest.argument))
            }),
        BindingPattern::ArrayPattern(pattern) => pattern
            .elements
            .iter()
            .flatten()
            .find_map(first_pattern_binding_name)
            .or_else(|| {
                pattern
                    .rest
                    .as_ref()
                    .and_then(|rest| first_pattern_binding_name(&rest.argument))
            }),
        BindingPattern::AssignmentPattern(pattern) => first_pattern_binding_name(&pattern.left),
    }
}

pub(crate) fn vue27_props_alias_declaration(source: &str, pattern: &BindingPattern<'_>) -> String {
    let pattern_source = source
        .get(pattern.span().start as usize..pattern.span().end as usize)
        .map(str::trim)
        .filter(|source| !source.is_empty());
    if let Some(pattern_source) = pattern_source {
        format!("\nconst {pattern_source} = __props;\n")
    } else {
        String::new()
    }
}

pub(crate) fn remove_vue27_macro_declarators(
    declaration: &VariableDeclaration<'_>,
    macro_indices: &[usize],
    edits: &mut SourceEdits<'_>,
) {
    if macro_indices.is_empty() {
        return;
    }
    if macro_indices.len() == declaration.declarations.len() {
        edits.remove(
            declaration.span.start as usize,
            declaration.span.end as usize,
        );
        return;
    }
    let mut spans = Vec::new();
    for index in macro_indices {
        let declarator = &declaration.declarations[*index];
        let (start, end) = if *index == 0 {
            (
                declarator.span.start as usize,
                declaration.declarations[index + 1].span.start as usize,
            )
        } else {
            (
                declaration.declarations[index - 1].span.end as usize,
                declarator.span.end as usize,
            )
        };
        spans.push((start, end));
    }
    spans.sort_unstable();
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (start, end) in spans {
        if let Some((_, last_end)) = merged.last_mut() {
            if start <= *last_end {
                *last_end = (*last_end).max(end);
                continue;
            }
        }
        merged.push((start, end));
    }
    for (start, end) in merged {
        edits.remove(start, end);
    }
}

pub(crate) fn object_expression_keys(object: &ObjectExpression<'_>) -> Vec<String> {
    object
        .properties
        .iter()
        .filter_map(|property| property.as_property())
        .filter(|property| !property.computed)
        .filter_map(|property| property.key.static_name().map(|name| name.into_owned()))
        .collect()
}

pub(crate) fn import_specifier_local(specifier: &ImportDeclarationSpecifier<'_>) -> String {
    import_specifier_local_name(specifier).to_string()
}

pub(crate) fn import_specifier_local_name<'a>(
    specifier: &'a ImportDeclarationSpecifier<'_>,
) -> &'a str {
    match specifier {
        ImportDeclarationSpecifier::ImportSpecifier(specifier) => specifier.local.name.as_str(),
        ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
            specifier.local.name.as_str()
        }
        ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
            specifier.local.name.as_str()
        }
    }
}

pub(crate) fn import_specifier_imported(
    specifier: &ImportDeclarationSpecifier<'_>,
) -> Option<String> {
    import_specifier_imported_name(specifier).map(str::to_string)
}

pub(crate) fn import_specifier_imported_name<'a>(
    specifier: &'a ImportDeclarationSpecifier<'a>,
) -> Option<&'a str> {
    match specifier {
        ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
            module_export_name(&specifier.imported)
        }
        ImportDeclarationSpecifier::ImportDefaultSpecifier(_) => Some("default"),
        ImportDeclarationSpecifier::ImportNamespaceSpecifier(_) => Some("*"),
    }
}

pub(crate) fn vue27_import_specifier_is_type(
    import: &oxc_ast::ast::ImportDeclaration<'_>,
    specifier: &ImportDeclarationSpecifier<'_>,
) -> bool {
    import.import_kind == ImportOrExportKind::Type
        || matches!(
            specifier,
            ImportDeclarationSpecifier::ImportSpecifier(specifier)
                if specifier.import_kind == ImportOrExportKind::Type
        )
}

pub(crate) fn import_specifier_setup_dedupe_imported(
    specifier: &ImportDeclarationSpecifier<'_>,
) -> Option<String> {
    match specifier {
        ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
            Some(specifier.imported.name().to_string())
        }
        ImportDeclarationSpecifier::ImportNamespaceSpecifier(_) => Some("*".into()),
        ImportDeclarationSpecifier::ImportDefaultSpecifier(_) => None,
    }
}

pub(crate) fn vue27_import_already_declared_in_setup_context(
    analysis: &Vue27ScriptSetupAnalysis,
    source: &str,
    local: &str,
    imported: Option<&str>,
) -> bool {
    analysis.normal_imports.iter().any(|existing| {
        existing.local == local
            && existing.source == source
            && existing.imported == imported.unwrap_or("default")
    })
}

pub(crate) fn vue27_import_local_conflicts_in_setup_context(
    analysis: &Vue27ScriptSetupAnalysis,
    source: &str,
    local: &str,
    imported: Option<&str>,
) -> bool {
    analysis.normal_imports.iter().any(|existing| {
        existing.local == local
            && (existing.source != source || existing.imported != imported.unwrap_or("default"))
    })
}

pub(crate) fn import_specifier_source(
    source: &str,
    specifier: &ImportDeclarationSpecifier<'_>,
) -> String {
    source[specifier.span().start as usize..specifier.span().end as usize].to_string()
}

pub(crate) fn vue3_script_setup_kept_import_source(
    source: &str,
    import: &oxc_ast::ast::ImportDeclaration<'_>,
    source_value: &str,
    statement_start: usize,
    statement_end: usize,
    keep_specifier_indices: &[usize],
) -> Option<String> {
    let Some(specifiers) = import.specifiers.as_ref() else {
        return source
            .get(statement_start..statement_end)
            .map(ToOwned::to_owned);
    };
    let kept = specifiers
        .iter()
        .enumerate()
        .filter(|(index, _)| keep_specifier_indices.contains(index))
        .map(|(_, specifier)| specifier)
        .collect::<Vec<_>>();
    if kept.is_empty() {
        return None;
    }
    if kept.len() == specifiers.len() {
        return source
            .get(statement_start..statement_end)
            .map(ToOwned::to_owned);
    }
    let trailing = source
        .get(import.span().end as usize..statement_end)
        .unwrap_or_default();
    let mut default_import = None;
    let mut namespace_import = None;
    let mut named_imports = Vec::new();
    for specifier in kept {
        let specifier_source = import_specifier_source(source, specifier);
        match specifier {
            ImportDeclarationSpecifier::ImportDefaultSpecifier(_) => {
                default_import = Some(specifier_source);
            }
            ImportDeclarationSpecifier::ImportNamespaceSpecifier(_) => {
                namespace_import = Some(specifier_source);
            }
            ImportDeclarationSpecifier::ImportSpecifier(_) => {
                named_imports.push(specifier_source);
            }
        }
    }
    let mut import_clause = String::new();
    if let Some(default_import) = default_import {
        import_clause.push_str(&default_import);
    }
    if let Some(namespace_import) = namespace_import {
        if !import_clause.is_empty() {
            import_clause.push_str(", ");
        }
        import_clause.push_str(&namespace_import);
    }
    if !named_imports.is_empty() {
        if !import_clause.is_empty() {
            import_clause.push_str(", ");
        }
        import_clause.push_str("{ ");
        import_clause.push_str(&named_imports.join(", "));
        import_clause.push_str(" }");
    }
    Some(format!(
        "import {import_clause} from '{source_value}'{trailing}"
    ))
}

pub(crate) fn collect_vue3_setup_import_aliases(
    statements: &[Statement<'_>],
    normal_user_imports: &Vue3UserImports,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    let mut user_imports = normal_user_imports.clone();
    for statement in statements {
        let Statement::ImportDeclaration(import) = statement else {
            continue;
        };
        let source = import.source.value.as_str();
        let Some(specifiers) = &import.specifiers else {
            continue;
        };
        for specifier in specifiers {
            if vue3_import_specifier_compiler_macro(source, specifier).is_some() {
                continue;
            }
            let local = import_specifier_local(specifier);
            if user_imports.existing(&local).is_some() {
                continue;
            }
            user_imports.record(Vue27ScriptImport {
                local,
                source: source.to_string(),
                imported: import_specifier_imported(specifier).unwrap_or_else(|| "default".into()),
                is_type: vue27_import_specifier_is_type(import, specifier),
            });
        }
    }
    analysis.vue_import_aliases = user_imports.vue_aliases();
}

pub(crate) fn vue3_import_specifier_compiler_macro(
    source: &str,
    specifier: &ImportDeclarationSpecifier<'_>,
) -> Option<(String, String)> {
    if source != "vue" {
        return None;
    }
    let ImportDeclarationSpecifier::ImportSpecifier(specifier) = specifier else {
        return None;
    };
    let imported = specifier.imported.name();
    if !matches!(
        imported.as_str(),
        "defineProps"
            | "defineEmits"
            | "defineExpose"
            | "defineOptions"
            | "defineModel"
            | "defineSlots"
            | "withDefaults"
    ) {
        return None;
    }
    Some((imported.to_string(), specifier.local.name.to_string()))
}
