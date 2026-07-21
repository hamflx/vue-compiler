pub(crate) fn sync_vue3_type_alias_from_analysis(
    target: &mut Vue3ScriptSetupAnalysis,
    source: &Vue3ScriptSetupAnalysis,
    source_name: &str,
    target_name: &str,
) -> bool {
    macro_rules! sync_entry {
        ($field:ident) => {
            sync_vue3_projection_entry(
                &mut target.$field,
                &source.$field,
                source_name,
                target_name,
            )
        };
    }

    let mut changed = false;
    changed |= sync_entry!(declared_types);
    changed |= sync_entry!(define_model_declared_types);
    changed |= sync_entry!(type_query_declared_types);
    changed |= sync_entry!(define_model_type_query_declared_types);
    changed |= sync_entry!(keyof_type_query_declared_types);
    changed |= sync_entry!(props_type_declarations);
    changed |= sync_entry!(keyof_runtime_type_declarations);
    changed |= sync_entry!(tuple_runtime_type_declarations);
    changed |= sync_entry!(define_model_tuple_runtime_type_declarations);
    changed |= sync_entry!(array_element_runtime_type_declarations);
    changed |= sync_entry!(define_model_array_element_runtime_type_declarations);
    changed |= sync_entry!(parameter_tuple_runtime_type_declarations);
    changed |= sync_entry!(define_model_parameter_tuple_runtime_type_declarations);
    changed |= sync_entry!(constructor_parameter_tuple_runtime_type_declarations);
    changed |= sync_entry!(define_model_constructor_parameter_tuple_runtime_type_declarations);
    changed |= sync_entry!(return_type_runtime_type_declarations);
    changed |= sync_entry!(define_model_return_type_runtime_type_declarations);
    changed |= sync_entry!(props_options_type_declarations);
    changed |= sync_entry!(return_type_props_options_declarations);
    changed |= sync_entry!(value_type_projections);
    changed |= sync_vue3_generic_projection_entry(
        &mut target.generic_type_aliases,
        &source.generic_type_aliases,
        source_name,
        target_name,
    );
    changed |= sync_entry!(string_literal_type_declarations);
    changed |= sync_entry!(ordered_string_literal_type_declarations);
    changed |= sync_entry!(emits_type_declarations);
    changed |= sync_entry!(type_sources);
    changed |= sync_entry!(type_direct_deps);
    changed |= sync_entry!(type_deps);
    changed |= sync_entry!(unresolved_import_sources);

    let source_is_silent = source.silent_unresolved_type_names.contains(source_name);
    let target_is_silent = target.silent_unresolved_type_names.contains(target_name);
    if source_is_silent != target_is_silent {
        if source_is_silent {
            target
                .silent_unresolved_type_names
                .insert(target_name.to_string());
        } else {
            target.silent_unresolved_type_names.remove(target_name);
        }
        changed = true;
    }
    changed
}

pub(crate) fn sync_vue3_type_alias_from_context(
    target: &mut Vue3ScriptSetupAnalysis,
    source: &Vue27TypeContext,
    source_name: &str,
    target_name: &str,
) -> bool {
    macro_rules! sync_entry {
        ($field:ident) => {
            sync_vue3_projection_entry(
                &mut target.$field,
                &source.$field,
                source_name,
                target_name,
            )
        };
    }

    let mut changed = false;
    changed |= sync_entry!(declared_types);
    changed |= sync_entry!(define_model_declared_types);
    changed |= sync_entry!(type_query_declared_types);
    changed |= sync_entry!(define_model_type_query_declared_types);
    changed |= sync_entry!(keyof_type_query_declared_types);
    changed |= sync_entry!(props_type_declarations);
    changed |= sync_entry!(keyof_runtime_type_declarations);
    changed |= sync_entry!(tuple_runtime_type_declarations);
    changed |= sync_entry!(define_model_tuple_runtime_type_declarations);
    changed |= sync_entry!(array_element_runtime_type_declarations);
    changed |= sync_entry!(define_model_array_element_runtime_type_declarations);
    changed |= sync_entry!(parameter_tuple_runtime_type_declarations);
    changed |= sync_entry!(define_model_parameter_tuple_runtime_type_declarations);
    changed |= sync_entry!(constructor_parameter_tuple_runtime_type_declarations);
    changed |= sync_entry!(define_model_constructor_parameter_tuple_runtime_type_declarations);
    changed |= sync_entry!(return_type_runtime_type_declarations);
    changed |= sync_entry!(define_model_return_type_runtime_type_declarations);
    changed |= sync_entry!(props_options_type_declarations);
    changed |= sync_entry!(return_type_props_options_declarations);
    changed |= sync_vue3_generic_projection_entry(
        &mut target.generic_type_aliases,
        &source.generic_type_aliases,
        source_name,
        target_name,
    );
    changed |= sync_entry!(string_literal_type_declarations);
    changed |= sync_entry!(ordered_string_literal_type_declarations);
    changed |= sync_entry!(emits_type_declarations);
    changed |= sync_entry!(type_sources);
    changed |= sync_entry!(type_direct_deps);
    changed |= sync_entry!(type_deps);
    changed |= sync_entry!(unresolved_import_sources);

    let source_is_silent = source.silent_unresolved_type_names.contains(source_name);
    let target_is_silent = target.silent_unresolved_type_names.contains(target_name);
    if source_is_silent != target_is_silent {
        if source_is_silent {
            target
                .silent_unresolved_type_names
                .insert(target_name.to_string());
        } else {
            target.silent_unresolved_type_names.remove(target_name);
        }
        changed = true;
    }
    changed
}

pub(crate) fn sync_vue3_type_alias_to_context(
    target: &mut Vue27TypeContext,
    source: &Vue3ScriptSetupAnalysis,
    source_name: &str,
    target_name: &str,
) -> bool {
    macro_rules! sync_entry {
        ($field:ident) => {
            sync_vue3_projection_entry(
                &mut target.$field,
                &source.$field,
                source_name,
                target_name,
            )
        };
    }

    let mut changed = false;
    changed |= sync_entry!(declared_types);
    changed |= sync_entry!(define_model_declared_types);
    changed |= sync_entry!(type_query_declared_types);
    changed |= sync_entry!(define_model_type_query_declared_types);
    changed |= sync_entry!(keyof_type_query_declared_types);
    changed |= sync_entry!(props_type_declarations);
    changed |= sync_entry!(keyof_runtime_type_declarations);
    changed |= sync_entry!(tuple_runtime_type_declarations);
    changed |= sync_entry!(define_model_tuple_runtime_type_declarations);
    changed |= sync_entry!(array_element_runtime_type_declarations);
    changed |= sync_entry!(define_model_array_element_runtime_type_declarations);
    changed |= sync_entry!(parameter_tuple_runtime_type_declarations);
    changed |= sync_entry!(define_model_parameter_tuple_runtime_type_declarations);
    changed |= sync_entry!(constructor_parameter_tuple_runtime_type_declarations);
    changed |= sync_entry!(define_model_constructor_parameter_tuple_runtime_type_declarations);
    changed |= sync_entry!(return_type_runtime_type_declarations);
    changed |= sync_entry!(define_model_return_type_runtime_type_declarations);
    changed |= sync_entry!(props_options_type_declarations);
    changed |= sync_entry!(return_type_props_options_declarations);
    changed |= sync_vue3_generic_projection_entry(
        &mut target.generic_type_aliases,
        &source.generic_type_aliases,
        source_name,
        target_name,
    );
    changed |= sync_entry!(string_literal_type_declarations);
    changed |= sync_entry!(ordered_string_literal_type_declarations);
    changed |= sync_entry!(emits_type_declarations);
    changed |= sync_entry!(type_sources);
    changed |= sync_entry!(type_direct_deps);
    changed |= sync_entry!(type_deps);
    changed |= sync_entry!(unresolved_import_sources);

    let source_is_silent = source.silent_unresolved_type_names.contains(source_name);
    let target_is_silent = target.silent_unresolved_type_names.contains(target_name);
    if source_is_silent != target_is_silent {
        if source_is_silent {
            target
                .silent_unresolved_type_names
                .insert(target_name.to_string());
        } else {
            target.silent_unresolved_type_names.remove(target_name);
        }
        changed = true;
    }
    changed
}

pub(crate) fn has_vue3_type_alias_projection(
    analysis: &Vue3ScriptSetupAnalysis,
    name: &str,
) -> bool {
    analysis.declared_types.contains_key(name)
        || analysis.define_model_declared_types.contains_key(name)
        || analysis.type_query_declared_types.contains_key(name)
        || analysis
            .define_model_type_query_declared_types
            .contains_key(name)
        || analysis.keyof_type_query_declared_types.contains_key(name)
        || analysis.props_type_declarations.contains_key(name)
        || analysis.keyof_runtime_type_declarations.contains_key(name)
        || analysis.tuple_runtime_type_declarations.contains_key(name)
        || analysis
            .define_model_tuple_runtime_type_declarations
            .contains_key(name)
        || analysis
            .array_element_runtime_type_declarations
            .contains_key(name)
        || analysis
            .define_model_array_element_runtime_type_declarations
            .contains_key(name)
        || analysis
            .parameter_tuple_runtime_type_declarations
            .contains_key(name)
        || analysis
            .define_model_parameter_tuple_runtime_type_declarations
            .contains_key(name)
        || analysis
            .constructor_parameter_tuple_runtime_type_declarations
            .contains_key(name)
        || analysis
            .define_model_constructor_parameter_tuple_runtime_type_declarations
            .contains_key(name)
        || analysis
            .return_type_runtime_type_declarations
            .contains_key(name)
        || analysis
            .define_model_return_type_runtime_type_declarations
            .contains_key(name)
        || analysis.props_options_type_declarations.contains_key(name)
        || analysis
            .return_type_props_options_declarations
            .contains_key(name)
        || analysis.value_type_projections.contains_key(name)
        || analysis.generic_type_aliases.contains_key(name)
        || analysis.string_literal_type_declarations.contains_key(name)
        || analysis
            .ordered_string_literal_type_declarations
            .contains_key(name)
        || analysis.emits_type_declarations.contains_key(name)
        || analysis.type_sources.contains_key(name)
        || analysis.type_direct_deps.contains_key(name)
        || analysis.type_deps.contains_key(name)
        || analysis.unresolved_import_sources.contains_key(name)
        || analysis.silent_unresolved_type_names.contains(name)
}

fn sync_vue3_projection_entry<T: Clone + PartialEq>(
    target: &mut BTreeMap<String, T>,
    source: &BTreeMap<String, T>,
    source_name: &str,
    target_name: &str,
) -> bool {
    match source.get(source_name) {
        Some(value) if target.get(target_name) == Some(value) => false,
        Some(value) => {
            target.insert(target_name.to_string(), value.clone());
            true
        }
        None => target.remove(target_name).is_some(),
    }
}

fn sync_vue3_generic_projection_entry(
    target: &mut BTreeMap<String, Vue3GenericTypeAlias>,
    source: &BTreeMap<String, Vue3GenericTypeAlias>,
    source_name: &str,
    target_name: &str,
) -> bool {
    match source.get(source_name) {
        Some(value)
            if target
                .get(target_name)
                .is_some_and(|existing| vue3_generic_type_alias_semantically_eq(existing, value)) =>
        {
            if target.get(target_name) != Some(value) {
                target.insert(target_name.to_string(), value.clone());
            }
            false
        }
        Some(value) => {
            target.insert(target_name.to_string(), value.clone());
            true
        }
        None => target.remove(target_name).is_some(),
    }
}

fn vue3_generic_type_alias_semantically_eq(
    left: &Vue3GenericTypeAlias,
    right: &Vue3GenericTypeAlias,
) -> bool {
    left.source == right.source
        && left.kind == right.kind
        && left.params == right.params
        && vue3_generic_type_scope_kinds_match(&left.scope, &right.scope)
        && left.interface_fragments.len() == right.interface_fragments.len()
        && left
            .interface_fragments
            .iter()
            .zip(&right.interface_fragments)
            .all(|(left, right)| {
                left.source == right.source
                    && vue3_generic_type_scope_kinds_match(&left.scope, &right.scope)
            })
}

fn vue3_generic_type_scope_kinds_match(
    left: &Vue3GenericTypeScope,
    right: &Vue3GenericTypeScope,
) -> bool {
    matches!(
        (left, right),
        (Vue3GenericTypeScope::Local, Vue3GenericTypeScope::Local)
            | (
                Vue3GenericTypeScope::Captured(_),
                Vue3GenericTypeScope::Captured(_)
            )
    )
}

pub(crate) fn insert_vue3_local_type_alias(
    analysis: &mut Vue3ScriptSetupAnalysis,
    local_name: &str,
    exported_name: &str,
) {
    if let Some(value) = analysis.declared_types.get(local_name).cloned() {
        analysis
            .declared_types
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis
        .define_model_declared_types
        .get(local_name)
        .cloned()
    {
        analysis
            .define_model_declared_types
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis.type_query_declared_types.get(local_name).cloned() {
        analysis
            .type_query_declared_types
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis
        .define_model_type_query_declared_types
        .get(local_name)
        .cloned()
    {
        analysis
            .define_model_type_query_declared_types
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis
        .keyof_type_query_declared_types
        .get(local_name)
        .cloned()
    {
        analysis
            .keyof_type_query_declared_types
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis.props_type_declarations.get(local_name).cloned() {
        analysis
            .props_type_declarations
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis
        .keyof_runtime_type_declarations
        .get(local_name)
        .cloned()
    {
        analysis
            .keyof_runtime_type_declarations
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis
        .tuple_runtime_type_declarations
        .get(local_name)
        .cloned()
    {
        analysis
            .tuple_runtime_type_declarations
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis
        .define_model_tuple_runtime_type_declarations
        .get(local_name)
        .cloned()
    {
        analysis
            .define_model_tuple_runtime_type_declarations
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis
        .array_element_runtime_type_declarations
        .get(local_name)
        .cloned()
    {
        analysis
            .array_element_runtime_type_declarations
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis
        .define_model_array_element_runtime_type_declarations
        .get(local_name)
        .cloned()
    {
        analysis
            .define_model_array_element_runtime_type_declarations
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis
        .parameter_tuple_runtime_type_declarations
        .get(local_name)
        .cloned()
    {
        analysis
            .parameter_tuple_runtime_type_declarations
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis
        .define_model_parameter_tuple_runtime_type_declarations
        .get(local_name)
        .cloned()
    {
        analysis
            .define_model_parameter_tuple_runtime_type_declarations
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis
        .constructor_parameter_tuple_runtime_type_declarations
        .get(local_name)
        .cloned()
    {
        analysis
            .constructor_parameter_tuple_runtime_type_declarations
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis
        .define_model_constructor_parameter_tuple_runtime_type_declarations
        .get(local_name)
        .cloned()
    {
        analysis
            .define_model_constructor_parameter_tuple_runtime_type_declarations
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis
        .return_type_runtime_type_declarations
        .get(local_name)
        .cloned()
    {
        analysis
            .return_type_runtime_type_declarations
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis
        .define_model_return_type_runtime_type_declarations
        .get(local_name)
        .cloned()
    {
        analysis
            .define_model_return_type_runtime_type_declarations
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis
        .props_options_type_declarations
        .get(local_name)
        .cloned()
    {
        analysis
            .props_options_type_declarations
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis
        .return_type_props_options_declarations
        .get(local_name)
        .cloned()
    {
        analysis
            .return_type_props_options_declarations
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis.value_type_projections.get(local_name).cloned() {
        analysis
            .value_type_projections
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis.generic_type_aliases.get(local_name).cloned() {
        analysis
            .generic_type_aliases
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis
        .string_literal_type_declarations
        .get(local_name)
        .cloned()
    {
        analysis
            .string_literal_type_declarations
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis
        .ordered_string_literal_type_declarations
        .get(local_name)
        .cloned()
    {
        analysis
            .ordered_string_literal_type_declarations
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis.emits_type_declarations.get(local_name).cloned() {
        analysis
            .emits_type_declarations
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis.type_sources.get(local_name).cloned() {
        analysis
            .type_sources
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis.type_direct_deps.get(local_name).cloned() {
        analysis
            .type_direct_deps
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis.type_deps.get(local_name).cloned() {
        analysis.type_deps.insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis.unresolved_import_sources.get(local_name).cloned() {
        analysis
            .unresolved_import_sources
            .insert(exported_name.to_string(), value);
    }
    if analysis.silent_unresolved_type_names.contains(local_name) {
        analysis
            .silent_unresolved_type_names
            .insert(exported_name.to_string());
    }
}

pub(crate) fn project_vue3_export_all_type_context(
    analysis: &mut Vue3ScriptSetupAnalysis,
    imported: &Vue27TypeContext,
    dependency: &str,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<BTreeSet<String>> {
    for name in imported.type_sources.keys() {
        if name == "default" {
            continue;
        }
        if !reserve_vue3_external_type_alias_projection(
            imported,
            name,
            name.len(),
            dependency,
            namespace_budget,
        ) {
            return None;
        }
    }
    let mut names = BTreeSet::new();
    for name in imported.type_sources.keys() {
        if name == "default" {
            continue;
        }
        insert_vue3_re_exported_type_alias(analysis, imported, name, name, dependency);
        names.insert(name.clone());
    }
    Some(names)
}

pub(crate) fn insert_vue3_re_exported_type_alias(
    analysis: &mut Vue3ScriptSetupAnalysis,
    imported: &Vue27TypeContext,
    imported_name: &str,
    exported_name: &str,
    dependency: &str,
) {
    if let Some(runtime) = imported.declared_types.get(imported_name) {
        analysis
            .declared_types
            .insert(exported_name.to_string(), runtime.clone());
    }
    if let Some(runtime) = imported.define_model_declared_types.get(imported_name) {
        analysis
            .define_model_declared_types
            .insert(exported_name.to_string(), runtime.clone());
    }
    if let Some(runtime) = imported.type_query_declared_types.get(imported_name) {
        analysis
            .type_query_declared_types
            .insert(exported_name.to_string(), runtime.clone());
    }
    if let Some(runtime) = imported
        .define_model_type_query_declared_types
        .get(imported_name)
    {
        analysis
            .define_model_type_query_declared_types
            .insert(exported_name.to_string(), runtime.clone());
    }
    if let Some(runtime) = imported.keyof_type_query_declared_types.get(imported_name) {
        analysis
            .keyof_type_query_declared_types
            .insert(exported_name.to_string(), runtime.clone());
    }
    if let Some(props) = imported.props_type_declarations.get(imported_name) {
        analysis
            .props_type_declarations
            .insert(exported_name.to_string(), props.clone());
    }
    if let Some(types) = imported.keyof_runtime_type_declarations.get(imported_name) {
        analysis
            .keyof_runtime_type_declarations
            .insert(exported_name.to_string(), types.clone());
    }
    if let Some(tuple) = imported.tuple_runtime_type_declarations.get(imported_name) {
        analysis
            .tuple_runtime_type_declarations
            .insert(exported_name.to_string(), tuple.clone());
    }
    if let Some(tuple) = imported
        .define_model_tuple_runtime_type_declarations
        .get(imported_name)
    {
        analysis
            .define_model_tuple_runtime_type_declarations
            .insert(exported_name.to_string(), tuple.clone());
    }
    if let Some(types) = imported
        .array_element_runtime_type_declarations
        .get(imported_name)
    {
        analysis
            .array_element_runtime_type_declarations
            .insert(exported_name.to_string(), types.clone());
    }
    if let Some(types) = imported
        .define_model_array_element_runtime_type_declarations
        .get(imported_name)
    {
        analysis
            .define_model_array_element_runtime_type_declarations
            .insert(exported_name.to_string(), types.clone());
    }
    if let Some(tuple) = imported
        .parameter_tuple_runtime_type_declarations
        .get(imported_name)
    {
        analysis
            .parameter_tuple_runtime_type_declarations
            .insert(exported_name.to_string(), tuple.clone());
    }
    if let Some(tuple) = imported
        .define_model_parameter_tuple_runtime_type_declarations
        .get(imported_name)
    {
        analysis
            .define_model_parameter_tuple_runtime_type_declarations
            .insert(exported_name.to_string(), tuple.clone());
    }
    if let Some(tuple) = imported
        .constructor_parameter_tuple_runtime_type_declarations
        .get(imported_name)
    {
        analysis
            .constructor_parameter_tuple_runtime_type_declarations
            .insert(exported_name.to_string(), tuple.clone());
    }
    if let Some(tuple) = imported
        .define_model_constructor_parameter_tuple_runtime_type_declarations
        .get(imported_name)
    {
        analysis
            .define_model_constructor_parameter_tuple_runtime_type_declarations
            .insert(exported_name.to_string(), tuple.clone());
    }
    if let Some(types) = imported
        .return_type_runtime_type_declarations
        .get(imported_name)
    {
        analysis
            .return_type_runtime_type_declarations
            .insert(exported_name.to_string(), types.clone());
    }
    if let Some(types) = imported
        .define_model_return_type_runtime_type_declarations
        .get(imported_name)
    {
        analysis
            .define_model_return_type_runtime_type_declarations
            .insert(exported_name.to_string(), types.clone());
    }
    if let Some(props_options) = imported.props_options_type_declarations.get(imported_name) {
        analysis
            .props_options_type_declarations
            .insert(exported_name.to_string(), props_options.clone());
    }
    if let Some(props_options) = imported
        .return_type_props_options_declarations
        .get(imported_name)
    {
        analysis
            .return_type_props_options_declarations
            .insert(exported_name.to_string(), props_options.clone());
    }
    if let Some(alias) = imported.generic_type_aliases.get(imported_name) {
        analysis
            .generic_type_aliases
            .insert(exported_name.to_string(), alias.clone());
    }
    if let Some(keys) = imported.string_literal_type_declarations.get(imported_name) {
        analysis
            .string_literal_type_declarations
            .insert(exported_name.to_string(), keys.clone());
    }
    if let Some(keys) = imported
        .ordered_string_literal_type_declarations
        .get(imported_name)
    {
        analysis
            .ordered_string_literal_type_declarations
            .insert(exported_name.to_string(), keys.clone());
    }
    if let Some(emits) = imported.emits_type_declarations.get(imported_name) {
        analysis
            .emits_type_declarations
            .insert(exported_name.to_string(), emits.clone());
    }
    if vue3_type_context_has_name(imported, imported_name) {
        analysis
            .type_sources
            .insert(exported_name.to_string(), dependency.to_string());
        analysis.type_direct_deps.insert(
            exported_name.to_string(),
            vue3_direct_type_deps(imported, imported_name, dependency),
        );
        let mut deps = imported
            .type_deps
            .get(imported_name)
            .cloned()
            .unwrap_or_default();
        deps.insert(dependency.to_string());
        analysis.type_deps.insert(exported_name.to_string(), deps);
    }
    if let Some(import_source) = imported.unresolved_import_sources.get(imported_name) {
        analysis
            .unresolved_import_sources
            .insert(exported_name.to_string(), import_source.clone());
    }
    if imported
        .silent_unresolved_type_names
        .contains(imported_name)
    {
        analysis
            .silent_unresolved_type_names
            .insert(exported_name.to_string());
    }
}

pub(crate) fn insert_vue3_re_exported_type_alias_and_namespace_members(
    analysis: &mut Vue3ScriptSetupAnalysis,
    imported: &Vue27TypeContext,
    imported_name: &str,
    exported_name: &str,
    dependency: &str,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<BTreeSet<String>> {
    if vue3_type_context_has_name(imported, imported_name)
        && !reserve_vue3_external_type_alias_projection(
            imported,
            imported_name,
            exported_name.len(),
            dependency,
            namespace_budget,
        )
    {
        return None;
    }
    let imported_prefix = format!("{imported_name}.");
    for imported_member in imported.type_sources.range(imported_prefix.clone()..) {
        let imported_member = imported_member.0;
        let Some(member) = imported_member.strip_prefix(&imported_prefix) else {
            break;
        };
        if !reserve_vue3_external_type_alias_projection(
            imported,
            imported_member,
            exported_name
                .len()
                .saturating_add(member.len())
                .saturating_add(1),
            dependency,
            namespace_budget,
        ) {
            return None;
        }
    }
    let mut exported_names = BTreeSet::new();
    if vue3_type_context_has_name(imported, imported_name) {
        insert_vue3_re_exported_type_alias(
            analysis,
            imported,
            imported_name,
            exported_name,
            dependency,
        );
        exported_names.insert(exported_name.to_string());
    }
    for imported_member in imported.type_sources.range(imported_prefix.clone()..) {
        let imported_member = imported_member.0;
        let Some(member) = imported_member.strip_prefix(&imported_prefix) else {
            break;
        };
        let exported_member = format!("{exported_name}.{member}");
        insert_vue3_re_exported_type_alias(
            analysis,
            imported,
            imported_member,
            &exported_member,
            dependency,
        );
        exported_names.insert(exported_member);
    }
    Some(exported_names)
}

pub(crate) fn insert_vue3_external_type_alias(
    context: &mut Vue27TypeContext,
    imported: &Vue27TypeContext,
    imported_name: &str,
    local_name: &str,
    dependency: &str,
) {
    if let Some(runtime) = imported.declared_types.get(imported_name) {
        context
            .declared_types
            .insert(local_name.to_string(), runtime.clone());
    }
    if let Some(runtime) = imported.define_model_declared_types.get(imported_name) {
        context
            .define_model_declared_types
            .insert(local_name.to_string(), runtime.clone());
    }
    if let Some(runtime) = imported.type_query_declared_types.get(imported_name) {
        context
            .type_query_declared_types
            .insert(local_name.to_string(), runtime.clone());
    }
    if let Some(runtime) = imported
        .define_model_type_query_declared_types
        .get(imported_name)
    {
        context
            .define_model_type_query_declared_types
            .insert(local_name.to_string(), runtime.clone());
    }
    if let Some(runtime) = imported.keyof_type_query_declared_types.get(imported_name) {
        context
            .keyof_type_query_declared_types
            .insert(local_name.to_string(), runtime.clone());
    }
    if let Some(props) = imported.props_type_declarations.get(imported_name) {
        context
            .props_type_declarations
            .insert(local_name.to_string(), props.clone());
    }
    if let Some(types) = imported.keyof_runtime_type_declarations.get(imported_name) {
        context
            .keyof_runtime_type_declarations
            .insert(local_name.to_string(), types.clone());
    }
    if let Some(tuple) = imported.tuple_runtime_type_declarations.get(imported_name) {
        context
            .tuple_runtime_type_declarations
            .insert(local_name.to_string(), tuple.clone());
    }
    if let Some(tuple) = imported
        .define_model_tuple_runtime_type_declarations
        .get(imported_name)
    {
        context
            .define_model_tuple_runtime_type_declarations
            .insert(local_name.to_string(), tuple.clone());
    }
    if let Some(types) = imported
        .array_element_runtime_type_declarations
        .get(imported_name)
    {
        context
            .array_element_runtime_type_declarations
            .insert(local_name.to_string(), types.clone());
    }
    if let Some(types) = imported
        .define_model_array_element_runtime_type_declarations
        .get(imported_name)
    {
        context
            .define_model_array_element_runtime_type_declarations
            .insert(local_name.to_string(), types.clone());
    }
    if let Some(tuple) = imported
        .parameter_tuple_runtime_type_declarations
        .get(imported_name)
    {
        context
            .parameter_tuple_runtime_type_declarations
            .insert(local_name.to_string(), tuple.clone());
    }
    if let Some(tuple) = imported
        .define_model_parameter_tuple_runtime_type_declarations
        .get(imported_name)
    {
        context
            .define_model_parameter_tuple_runtime_type_declarations
            .insert(local_name.to_string(), tuple.clone());
    }
    if let Some(tuple) = imported
        .constructor_parameter_tuple_runtime_type_declarations
        .get(imported_name)
    {
        context
            .constructor_parameter_tuple_runtime_type_declarations
            .insert(local_name.to_string(), tuple.clone());
    }
    if let Some(tuple) = imported
        .define_model_constructor_parameter_tuple_runtime_type_declarations
        .get(imported_name)
    {
        context
            .define_model_constructor_parameter_tuple_runtime_type_declarations
            .insert(local_name.to_string(), tuple.clone());
    }
    if let Some(types) = imported
        .return_type_runtime_type_declarations
        .get(imported_name)
    {
        context
            .return_type_runtime_type_declarations
            .insert(local_name.to_string(), types.clone());
    }
    if let Some(types) = imported
        .define_model_return_type_runtime_type_declarations
        .get(imported_name)
    {
        context
            .define_model_return_type_runtime_type_declarations
            .insert(local_name.to_string(), types.clone());
    }
    if let Some(props_options) = imported.props_options_type_declarations.get(imported_name) {
        context
            .props_options_type_declarations
            .insert(local_name.to_string(), props_options.clone());
    }
    if let Some(props_options) = imported
        .return_type_props_options_declarations
        .get(imported_name)
    {
        context
            .return_type_props_options_declarations
            .insert(local_name.to_string(), props_options.clone());
    }
    if let Some(alias) = imported.generic_type_aliases.get(imported_name) {
        context
            .generic_type_aliases
            .insert(local_name.to_string(), alias.clone());
    }
    if let Some(keys) = imported.string_literal_type_declarations.get(imported_name) {
        context
            .string_literal_type_declarations
            .insert(local_name.to_string(), keys.clone());
    }
    if let Some(keys) = imported
        .ordered_string_literal_type_declarations
        .get(imported_name)
    {
        context
            .ordered_string_literal_type_declarations
            .insert(local_name.to_string(), keys.clone());
    }
    if let Some(emits) = imported.emits_type_declarations.get(imported_name) {
        context
            .emits_type_declarations
            .insert(local_name.to_string(), emits.clone());
    }
    if imported.declared_types.contains_key(imported_name)
        || imported
            .define_model_declared_types
            .contains_key(imported_name)
        || imported
            .type_query_declared_types
            .contains_key(imported_name)
        || imported
            .define_model_type_query_declared_types
            .contains_key(imported_name)
        || imported
            .keyof_type_query_declared_types
            .contains_key(imported_name)
        || imported.props_type_declarations.contains_key(imported_name)
        || imported
            .keyof_runtime_type_declarations
            .contains_key(imported_name)
        || imported
            .tuple_runtime_type_declarations
            .contains_key(imported_name)
        || imported
            .define_model_tuple_runtime_type_declarations
            .contains_key(imported_name)
        || imported
            .array_element_runtime_type_declarations
            .contains_key(imported_name)
        || imported
            .define_model_array_element_runtime_type_declarations
            .contains_key(imported_name)
        || imported
            .parameter_tuple_runtime_type_declarations
            .contains_key(imported_name)
        || imported
            .define_model_parameter_tuple_runtime_type_declarations
            .contains_key(imported_name)
        || imported
            .constructor_parameter_tuple_runtime_type_declarations
            .contains_key(imported_name)
        || imported
            .define_model_constructor_parameter_tuple_runtime_type_declarations
            .contains_key(imported_name)
        || imported
            .return_type_runtime_type_declarations
            .contains_key(imported_name)
        || imported
            .define_model_return_type_runtime_type_declarations
            .contains_key(imported_name)
        || imported
            .props_options_type_declarations
            .contains_key(imported_name)
        || imported
            .return_type_props_options_declarations
            .contains_key(imported_name)
        || imported.generic_type_aliases.contains_key(imported_name)
        || imported
            .string_literal_type_declarations
            .contains_key(imported_name)
        || imported
            .ordered_string_literal_type_declarations
            .contains_key(imported_name)
        || imported.emits_type_declarations.contains_key(imported_name)
    {
        context
            .type_sources
            .insert(local_name.to_string(), dependency.to_string());
        context.type_direct_deps.insert(
            local_name.to_string(),
            vue3_direct_type_deps(imported, imported_name, dependency),
        );
        let mut deps = imported
            .type_deps
            .get(imported_name)
            .cloned()
            .unwrap_or_default();
        deps.insert(dependency.to_string());
        context.type_deps.insert(local_name.to_string(), deps);
    }
    if let Some(import_source) = imported.unresolved_import_sources.get(imported_name) {
        context
            .unresolved_import_sources
            .insert(local_name.to_string(), import_source.clone());
    }
    if imported
        .silent_unresolved_type_names
        .contains(imported_name)
    {
        context
            .silent_unresolved_type_names
            .insert(local_name.to_string());
    }
}

pub(crate) fn insert_vue3_external_type_alias_and_namespace_members(
    context: &mut Vue27TypeContext,
    imported: &Vue27TypeContext,
    imported_name: &str,
    local_name: &str,
    dependency: &str,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> bool {
    if !reserve_vue3_external_import_binding_clear(context, local_name, namespace_budget) {
        return false;
    }
    if !reserve_vue3_external_type_alias_projection(
        imported,
        imported_name,
        local_name.len(),
        dependency,
        namespace_budget,
    ) {
        return false;
    }
    let mut has_generic_alias = imported.generic_type_aliases.contains_key(imported_name);
    let imported_prefix = format!("{imported_name}.");
    for imported_member in imported.type_sources.range(imported_prefix.clone()..) {
        let imported_member = imported_member.0;
        let Some(member) = imported_member.strip_prefix(&imported_prefix) else {
            break;
        };
        has_generic_alias |= imported.generic_type_aliases.contains_key(imported_member);
        if !reserve_vue3_external_type_alias_projection(
            imported,
            imported_member,
            local_name
                .len()
                .saturating_add(member.len())
                .saturating_add(1),
            dependency,
            namespace_budget,
        ) {
            return false;
        }
    }
    if has_generic_alias
        && !namespace_budget.reserve(vue3_external_generic_alias_helpers_projection_work(
            imported, dependency,
        ))
    {
        return false;
    }
    clear_vue3_external_import_binding(context, local_name);
    insert_vue3_external_type_alias(
        context,
        imported,
        imported_name,
        local_name,
        dependency,
    );
    for imported_member in imported.type_sources.range(imported_prefix.clone()..) {
        let imported_member = imported_member.0;
        let Some(member) = imported_member.strip_prefix(&imported_prefix) else {
            break;
        };
        let local_member = format!("{local_name}.{member}");
        insert_vue3_external_type_alias(
            context,
            imported,
            imported_member,
            &local_member,
            dependency,
        );
    }
    if has_generic_alias {
        insert_vue3_external_generic_alias_string_key_helpers(context, imported, dependency);
    }
    true
}

fn reserve_vue3_external_type_alias_projection(
    context: &Vue27TypeContext,
    source_name: &str,
    target_name_len: usize,
    dependency: &str,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> bool {
    namespace_budget.reserve(vue3_external_type_alias_projection_work(
        context,
        source_name,
        target_name_len,
        dependency,
    ))
}

pub(crate) fn insert_vue3_external_generic_alias_string_key_helpers(
    context: &mut Vue27TypeContext,
    imported: &Vue27TypeContext,
    dependency: &str,
) {
    for (name, keys) in &imported.string_literal_type_declarations {
        context
            .string_literal_type_declarations
            .entry(name.clone())
            .or_insert_with(|| keys.clone());
        insert_vue3_external_helper_type_dep(context, imported, name, dependency);
    }
    for (name, keys) in &imported.ordered_string_literal_type_declarations {
        context
            .ordered_string_literal_type_declarations
            .entry(name.clone())
            .or_insert_with(|| keys.clone());
        insert_vue3_external_helper_type_dep(context, imported, name, dependency);
    }
    for (name, types) in &imported.keyof_runtime_type_declarations {
        context
            .keyof_runtime_type_declarations
            .entry(name.clone())
            .or_insert_with(|| types.clone());
        insert_vue3_external_helper_type_dep(context, imported, name, dependency);
    }
    for (name, tuple) in &imported.tuple_runtime_type_declarations {
        context
            .tuple_runtime_type_declarations
            .entry(name.clone())
            .or_insert_with(|| tuple.clone());
        insert_vue3_external_helper_type_dep(context, imported, name, dependency);
    }
    for (name, tuple) in &imported.define_model_tuple_runtime_type_declarations {
        context
            .define_model_tuple_runtime_type_declarations
            .entry(name.clone())
            .or_insert_with(|| tuple.clone());
        insert_vue3_external_helper_type_dep(context, imported, name, dependency);
    }
    for (name, types) in &imported.array_element_runtime_type_declarations {
        context
            .array_element_runtime_type_declarations
            .entry(name.clone())
            .or_insert_with(|| types.clone());
        insert_vue3_external_helper_type_dep(context, imported, name, dependency);
    }
    for (name, types) in &imported.define_model_array_element_runtime_type_declarations {
        context
            .define_model_array_element_runtime_type_declarations
            .entry(name.clone())
            .or_insert_with(|| types.clone());
        insert_vue3_external_helper_type_dep(context, imported, name, dependency);
    }
    for (name, tuple) in &imported.parameter_tuple_runtime_type_declarations {
        context
            .parameter_tuple_runtime_type_declarations
            .entry(name.clone())
            .or_insert_with(|| tuple.clone());
        insert_vue3_external_helper_type_dep(context, imported, name, dependency);
    }
    for (name, tuple) in &imported.define_model_parameter_tuple_runtime_type_declarations {
        context
            .define_model_parameter_tuple_runtime_type_declarations
            .entry(name.clone())
            .or_insert_with(|| tuple.clone());
        insert_vue3_external_helper_type_dep(context, imported, name, dependency);
    }
    for (name, tuple) in &imported.constructor_parameter_tuple_runtime_type_declarations {
        context
            .constructor_parameter_tuple_runtime_type_declarations
            .entry(name.clone())
            .or_insert_with(|| tuple.clone());
        insert_vue3_external_helper_type_dep(context, imported, name, dependency);
    }
    for (name, tuple) in
        &imported.define_model_constructor_parameter_tuple_runtime_type_declarations
    {
        context
            .define_model_constructor_parameter_tuple_runtime_type_declarations
            .entry(name.clone())
            .or_insert_with(|| tuple.clone());
        insert_vue3_external_helper_type_dep(context, imported, name, dependency);
    }
    for (name, types) in &imported.return_type_runtime_type_declarations {
        context
            .return_type_runtime_type_declarations
            .entry(name.clone())
            .or_insert_with(|| types.clone());
        insert_vue3_external_helper_type_dep(context, imported, name, dependency);
    }
    for (name, types) in &imported.define_model_return_type_runtime_type_declarations {
        context
            .define_model_return_type_runtime_type_declarations
            .entry(name.clone())
            .or_insert_with(|| types.clone());
        insert_vue3_external_helper_type_dep(context, imported, name, dependency);
    }
}

pub(crate) fn insert_vue3_external_helper_type_dep(
    context: &mut Vue27TypeContext,
    imported: &Vue27TypeContext,
    name: &str,
    dependency: &str,
) {
    context
        .type_sources
        .entry(name.to_string())
        .or_insert_with(|| dependency.to_string());
    context
        .type_direct_deps
        .entry(name.to_string())
        .or_insert_with(|| vue3_direct_type_deps(imported, name, dependency));
    let mut deps = imported.type_deps.get(name).cloned().unwrap_or_default();
    deps.insert(dependency.to_string());
    context.type_deps.entry(name.to_string()).or_insert(deps);
}

pub(crate) fn vue3_direct_type_deps(
    imported: &Vue27TypeContext,
    imported_name: &str,
    dependency: &str,
) -> Vec<String> {
    let mut deps = Vec::new();
    push_unique(&mut deps, dependency);
    if let Some(imported_deps) = imported.type_direct_deps.get(imported_name) {
        for imported_dep in imported_deps {
            push_unique(&mut deps, imported_dep);
        }
    }
    deps
}

pub(crate) fn insert_vue3_external_namespace_types(
    context: &mut Vue27TypeContext,
    imported: &Vue27TypeContext,
    namespace: &str,
    dependency: &str,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> bool {
    if !reserve_vue3_external_import_binding_clear(context, namespace, namespace_budget) {
        return false;
    }
    let mut has_generic_alias = false;
    for imported_name in imported.type_sources.keys() {
        has_generic_alias |= imported.generic_type_aliases.contains_key(imported_name);
        if !reserve_vue3_external_type_alias_projection(
            imported,
            imported_name,
            namespace
                .len()
                .saturating_add(imported_name.len())
                .saturating_add(1),
            dependency,
            namespace_budget,
        ) {
            return false;
        }
    }
    if has_generic_alias
        && !namespace_budget.reserve(vue3_external_generic_alias_helpers_projection_work(
            imported, dependency,
        ))
    {
        return false;
    }
    clear_vue3_external_import_binding(context, namespace);
    for imported_name in imported.type_sources.keys() {
        let local_name = format!("{namespace}.{imported_name}");
        insert_vue3_external_type_alias(context, imported, imported_name, &local_name, dependency);
    }
    if has_generic_alias {
        insert_vue3_external_generic_alias_string_key_helpers(context, imported, dependency);
    }
    true
}

fn reserve_vue3_external_import_binding_clear(
    context: &Vue27TypeContext,
    local_name: &str,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> bool {
    const TYPE_CONTEXT_ALIAS_COLLECTIONS: usize = 28;

    let scan_work = context
        .type_sources
        .keys()
        .chain(context.unresolved_import_sources.keys())
        .chain(&context.silent_unresolved_type_names)
        .fold(local_name.len().saturating_add(2), |work, name| {
            work.saturating_add(name.len()).saturating_add(1)
        });
    let clear_work = context.type_sources.keys().fold(0usize, |work, name| {
        if vue3_type_name_is_in_import_binding(name, local_name) {
            work.saturating_add(vue3_external_type_alias_projection_work(
                context,
                name,
                name.len(),
                "",
            ))
        } else {
            work
        }
    });
    namespace_budget.reserve(
        scan_work
            .saturating_mul(TYPE_CONTEXT_ALIAS_COLLECTIONS)
            .saturating_add(clear_work),
    )
}

fn clear_vue3_external_import_binding(context: &mut Vue27TypeContext, local_name: &str) {
    macro_rules! clear_field {
        ($field:ident) => {
            context
                .$field
                .retain(|name, _| !vue3_type_name_is_in_import_binding(name, local_name));
        };
    }

    clear_field!(declared_types);
    clear_field!(define_model_declared_types);
    clear_field!(type_query_declared_types);
    clear_field!(define_model_type_query_declared_types);
    clear_field!(keyof_type_query_declared_types);
    clear_field!(props_type_declarations);
    clear_field!(keyof_runtime_type_declarations);
    clear_field!(tuple_runtime_type_declarations);
    clear_field!(define_model_tuple_runtime_type_declarations);
    clear_field!(array_element_runtime_type_declarations);
    clear_field!(define_model_array_element_runtime_type_declarations);
    clear_field!(parameter_tuple_runtime_type_declarations);
    clear_field!(define_model_parameter_tuple_runtime_type_declarations);
    clear_field!(constructor_parameter_tuple_runtime_type_declarations);
    clear_field!(define_model_constructor_parameter_tuple_runtime_type_declarations);
    clear_field!(return_type_runtime_type_declarations);
    clear_field!(define_model_return_type_runtime_type_declarations);
    clear_field!(props_options_type_declarations);
    clear_field!(return_type_props_options_declarations);
    clear_field!(generic_type_aliases);
    clear_field!(string_literal_type_declarations);
    clear_field!(ordered_string_literal_type_declarations);
    clear_field!(emits_type_declarations);
    clear_field!(type_sources);
    clear_field!(type_direct_deps);
    clear_field!(type_deps);
    clear_field!(unresolved_import_sources);
    context
        .silent_unresolved_type_names
        .retain(|name| !vue3_type_name_is_in_import_binding(name, local_name));
}

fn vue3_type_name_is_in_import_binding(name: &str, local_name: &str) -> bool {
    name == local_name
        || name
            .strip_prefix(local_name)
            .is_some_and(|suffix| suffix.starts_with('.'))
}

pub(crate) fn vue3_type_context_names(context: &Vue27TypeContext) -> BTreeSet<String> {
    context
        .declared_types
        .keys()
        .chain(context.define_model_declared_types.keys())
        .chain(context.type_query_declared_types.keys())
        .chain(context.define_model_type_query_declared_types.keys())
        .chain(context.keyof_type_query_declared_types.keys())
        .chain(context.props_type_declarations.keys())
        .chain(context.keyof_runtime_type_declarations.keys())
        .chain(context.tuple_runtime_type_declarations.keys())
        .chain(context.define_model_tuple_runtime_type_declarations.keys())
        .chain(context.array_element_runtime_type_declarations.keys())
        .chain(
            context
                .define_model_array_element_runtime_type_declarations
                .keys(),
        )
        .chain(context.parameter_tuple_runtime_type_declarations.keys())
        .chain(
            context
                .define_model_parameter_tuple_runtime_type_declarations
                .keys(),
        )
        .chain(
            context
                .constructor_parameter_tuple_runtime_type_declarations
                .keys(),
        )
        .chain(
            context
                .define_model_constructor_parameter_tuple_runtime_type_declarations
                .keys(),
        )
        .chain(context.return_type_runtime_type_declarations.keys())
        .chain(
            context
                .define_model_return_type_runtime_type_declarations
                .keys(),
        )
        .chain(context.props_options_type_declarations.keys())
        .chain(context.return_type_props_options_declarations.keys())
        .chain(context.generic_type_aliases.keys())
        .chain(context.string_literal_type_declarations.keys())
        .chain(context.ordered_string_literal_type_declarations.keys())
        .chain(context.emits_type_declarations.keys())
        .cloned()
        .collect()
}

pub(crate) fn vue3_type_context_has_name(context: &Vue27TypeContext, name: &str) -> bool {
    context.declared_types.contains_key(name)
        || context.define_model_declared_types.contains_key(name)
        || context.type_query_declared_types.contains_key(name)
        || context
            .define_model_type_query_declared_types
            .contains_key(name)
        || context.keyof_type_query_declared_types.contains_key(name)
        || context.props_type_declarations.contains_key(name)
        || context.keyof_runtime_type_declarations.contains_key(name)
        || context.tuple_runtime_type_declarations.contains_key(name)
        || context
            .define_model_tuple_runtime_type_declarations
            .contains_key(name)
        || context
            .array_element_runtime_type_declarations
            .contains_key(name)
        || context
            .define_model_array_element_runtime_type_declarations
            .contains_key(name)
        || context
            .parameter_tuple_runtime_type_declarations
            .contains_key(name)
        || context
            .define_model_parameter_tuple_runtime_type_declarations
            .contains_key(name)
        || context
            .constructor_parameter_tuple_runtime_type_declarations
            .contains_key(name)
        || context
            .define_model_constructor_parameter_tuple_runtime_type_declarations
            .contains_key(name)
        || context
            .return_type_runtime_type_declarations
            .contains_key(name)
        || context
            .define_model_return_type_runtime_type_declarations
            .contains_key(name)
        || context.props_options_type_declarations.contains_key(name)
        || context
            .return_type_props_options_declarations
            .contains_key(name)
        || context.generic_type_aliases.contains_key(name)
        || context.string_literal_type_declarations.contains_key(name)
        || context
            .ordered_string_literal_type_declarations
            .contains_key(name)
        || context.emits_type_declarations.contains_key(name)
}

pub(crate) fn vue3_type_import_source_is_relative(source: &str) -> bool {
    source.starts_with("./") || source.starts_with("../")
}
