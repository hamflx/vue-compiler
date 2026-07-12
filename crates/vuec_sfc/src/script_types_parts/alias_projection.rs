pub(crate) fn insert_vue3_type_alias_from_analysis(
    target: &mut Vue3ScriptSetupAnalysis,
    source: &Vue3ScriptSetupAnalysis,
    source_name: &str,
    target_name: &str,
) {
    if let Some(value) = source.declared_types.get(source_name).cloned() {
        target.declared_types.insert(target_name.to_string(), value);
    }
    if let Some(value) = source.define_model_declared_types.get(source_name).cloned() {
        target
            .define_model_declared_types
            .insert(target_name.to_string(), value);
    }
    if let Some(value) = source.type_query_declared_types.get(source_name).cloned() {
        target
            .type_query_declared_types
            .insert(target_name.to_string(), value);
    }
    if let Some(value) = source
        .define_model_type_query_declared_types
        .get(source_name)
        .cloned()
    {
        target
            .define_model_type_query_declared_types
            .insert(target_name.to_string(), value);
    }
    if let Some(value) = source
        .keyof_type_query_declared_types
        .get(source_name)
        .cloned()
    {
        target
            .keyof_type_query_declared_types
            .insert(target_name.to_string(), value);
    }
    if let Some(value) = source.props_type_declarations.get(source_name).cloned() {
        target
            .props_type_declarations
            .insert(target_name.to_string(), value);
    }
    if let Some(value) = source
        .keyof_runtime_type_declarations
        .get(source_name)
        .cloned()
    {
        target
            .keyof_runtime_type_declarations
            .insert(target_name.to_string(), value);
    }
    if let Some(value) = source
        .tuple_runtime_type_declarations
        .get(source_name)
        .cloned()
    {
        target
            .tuple_runtime_type_declarations
            .insert(target_name.to_string(), value);
    }
    if let Some(value) = source
        .define_model_tuple_runtime_type_declarations
        .get(source_name)
        .cloned()
    {
        target
            .define_model_tuple_runtime_type_declarations
            .insert(target_name.to_string(), value);
    }
    if let Some(value) = source
        .array_element_runtime_type_declarations
        .get(source_name)
        .cloned()
    {
        target
            .array_element_runtime_type_declarations
            .insert(target_name.to_string(), value);
    }
    if let Some(value) = source
        .define_model_array_element_runtime_type_declarations
        .get(source_name)
        .cloned()
    {
        target
            .define_model_array_element_runtime_type_declarations
            .insert(target_name.to_string(), value);
    }
    if let Some(value) = source
        .parameter_tuple_runtime_type_declarations
        .get(source_name)
        .cloned()
    {
        target
            .parameter_tuple_runtime_type_declarations
            .insert(target_name.to_string(), value);
    }
    if let Some(value) = source
        .define_model_parameter_tuple_runtime_type_declarations
        .get(source_name)
        .cloned()
    {
        target
            .define_model_parameter_tuple_runtime_type_declarations
            .insert(target_name.to_string(), value);
    }
    if let Some(value) = source
        .constructor_parameter_tuple_runtime_type_declarations
        .get(source_name)
        .cloned()
    {
        target
            .constructor_parameter_tuple_runtime_type_declarations
            .insert(target_name.to_string(), value);
    }
    if let Some(value) = source
        .define_model_constructor_parameter_tuple_runtime_type_declarations
        .get(source_name)
        .cloned()
    {
        target
            .define_model_constructor_parameter_tuple_runtime_type_declarations
            .insert(target_name.to_string(), value);
    }
    if let Some(value) = source
        .return_type_runtime_type_declarations
        .get(source_name)
        .cloned()
    {
        target
            .return_type_runtime_type_declarations
            .insert(target_name.to_string(), value);
    }
    if let Some(value) = source
        .define_model_return_type_runtime_type_declarations
        .get(source_name)
        .cloned()
    {
        target
            .define_model_return_type_runtime_type_declarations
            .insert(target_name.to_string(), value);
    }
    if let Some(value) = source
        .props_options_type_declarations
        .get(source_name)
        .cloned()
    {
        target
            .props_options_type_declarations
            .insert(target_name.to_string(), value);
    }
    if let Some(value) = source
        .return_type_props_options_declarations
        .get(source_name)
        .cloned()
    {
        target
            .return_type_props_options_declarations
            .insert(target_name.to_string(), value);
    }
    if let Some(value) = source.generic_type_aliases.get(source_name).cloned() {
        target
            .generic_type_aliases
            .insert(target_name.to_string(), value);
    }
    if let Some(value) = source
        .string_literal_type_declarations
        .get(source_name)
        .cloned()
    {
        target
            .string_literal_type_declarations
            .insert(target_name.to_string(), value);
    }
    if let Some(value) = source
        .ordered_string_literal_type_declarations
        .get(source_name)
        .cloned()
    {
        target
            .ordered_string_literal_type_declarations
            .insert(target_name.to_string(), value);
    }
    if let Some(value) = source.emits_type_declarations.get(source_name).cloned() {
        target
            .emits_type_declarations
            .insert(target_name.to_string(), value);
    }
    if let Some(value) = source.type_sources.get(source_name).cloned() {
        target.type_sources.insert(target_name.to_string(), value);
    }
    if let Some(value) = source.type_direct_deps.get(source_name).cloned() {
        target
            .type_direct_deps
            .insert(target_name.to_string(), value);
    }
    if let Some(value) = source.type_deps.get(source_name).cloned() {
        target.type_deps.insert(target_name.to_string(), value);
    }
    if let Some(value) = source.unresolved_import_sources.get(source_name).cloned() {
        target
            .unresolved_import_sources
            .insert(target_name.to_string(), value);
    }
    if source.silent_unresolved_type_names.contains(source_name) {
        target
            .silent_unresolved_type_names
            .insert(target_name.to_string());
    }
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
) -> BTreeSet<String> {
    let names = imported
        .declared_types
        .keys()
        .chain(imported.define_model_declared_types.keys())
        .chain(imported.type_query_declared_types.keys())
        .chain(imported.define_model_type_query_declared_types.keys())
        .chain(imported.keyof_type_query_declared_types.keys())
        .chain(imported.props_type_declarations.keys())
        .chain(imported.keyof_runtime_type_declarations.keys())
        .chain(imported.tuple_runtime_type_declarations.keys())
        .chain(imported.define_model_tuple_runtime_type_declarations.keys())
        .chain(imported.array_element_runtime_type_declarations.keys())
        .chain(
            imported
                .define_model_array_element_runtime_type_declarations
                .keys(),
        )
        .chain(imported.parameter_tuple_runtime_type_declarations.keys())
        .chain(
            imported
                .define_model_parameter_tuple_runtime_type_declarations
                .keys(),
        )
        .chain(
            imported
                .constructor_parameter_tuple_runtime_type_declarations
                .keys(),
        )
        .chain(
            imported
                .define_model_constructor_parameter_tuple_runtime_type_declarations
                .keys(),
        )
        .chain(imported.return_type_runtime_type_declarations.keys())
        .chain(
            imported
                .define_model_return_type_runtime_type_declarations
                .keys(),
        )
        .chain(imported.props_options_type_declarations.keys())
        .chain(imported.return_type_props_options_declarations.keys())
        .chain(imported.generic_type_aliases.keys())
        .chain(imported.string_literal_type_declarations.keys())
        .chain(imported.ordered_string_literal_type_declarations.keys())
        .chain(imported.emits_type_declarations.keys())
        .filter(|&name| name != "default")
        .cloned()
        .collect::<BTreeSet<_>>();
    for name in &names {
        insert_vue3_re_exported_type_alias(analysis, imported, name, name, dependency);
    }
    names
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
        insert_vue3_external_generic_alias_string_key_helpers(context, imported, dependency);
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
) {
    for imported_name in vue3_type_context_names(imported) {
        let local_name = format!("{namespace}.{imported_name}");
        insert_vue3_external_type_alias(context, imported, &imported_name, &local_name, dependency);
    }
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
