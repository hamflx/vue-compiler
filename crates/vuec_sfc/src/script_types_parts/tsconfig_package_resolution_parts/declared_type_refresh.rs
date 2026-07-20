pub(crate) fn register_vue3_interface_declaration(
    source: &str,
    declaration: &TSInterfaceDeclaration<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    let name = declaration.id.name.to_string();
    register_vue3_local_type_name(analysis, &name);
    analysis
        .declared_types
        .insert(name.clone(), vec!["Object".into()]);
    analysis
        .define_model_declared_types
        .insert(name.clone(), vec!["Object".into()]);
    refresh_vue3_interface_declaration(source, declaration, analysis);
}

pub(crate) fn refresh_vue3_interface_declaration(
    source: &str,
    declaration: &TSInterfaceDeclaration<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) -> bool {
    let mut changed = refresh_vue3_generic_interface_declaration(source, declaration, analysis);
    changed |= refresh_vue3_merged_interface_declarations(source, &[declaration], analysis);
    changed
}

pub(crate) fn refresh_vue3_generic_interface_declaration(
    source: &str,
    declaration: &TSInterfaceDeclaration<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) -> bool {
    let name = declaration.id.name.as_str();
    let Some(type_parameters) = declaration.type_parameters.as_ref() else {
        if analysis
            .generic_type_aliases
            .get(name)
            .is_some_and(|alias| alias.kind == Vue3GenericTypeAliasKind::Interface)
        {
            analysis.generic_type_aliases.remove(name);
            return true;
        }
        return false;
    };
    let params = type_parameters
        .params
        .iter()
        .map(|param| param.name.name.to_string())
        .collect::<Vec<_>>();
    let alias_source = source
        .get(declaration.span.start as usize..declaration.span.end as usize)
        .unwrap_or_default()
        .to_string();
    if params.is_empty() || alias_source.is_empty() {
        return false;
    }
    let alias = vue3_generic_type_alias(
        alias_source,
        Vue3GenericTypeAliasKind::Interface,
        params,
    );
    if analysis.generic_type_aliases.get(name) != Some(&alias) {
        analysis
            .generic_type_aliases
            .insert(name.to_string(), alias);
        true
    } else {
        false
    }
}

pub(crate) fn refresh_vue3_non_generic_interface_declaration(
    source: &str,
    declaration: &TSInterfaceDeclaration<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) -> bool {
    refresh_vue3_merged_interface_declarations(source, &[declaration], analysis)
}

pub(crate) fn refresh_vue3_interface_declaration_group(
    source: &str,
    declaration: &TSInterfaceDeclaration<'_>,
    interface_declarations: &BTreeMap<String, Vec<&TSInterfaceDeclaration<'_>>>,
    refreshed_interfaces: &mut BTreeSet<String>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) -> bool {
    let name = declaration.id.name.to_string();
    if !refreshed_interfaces.insert(name.clone()) {
        return false;
    }
    let Some(declarations) = interface_declarations.get(&name) else {
        return refresh_vue3_non_generic_interface_declaration(source, declaration, analysis);
    };
    let mut changed =
        refresh_vue3_generic_interface_declaration_group(source, declarations, analysis);
    changed |= refresh_vue3_merged_interface_declarations(source, declarations, analysis);
    changed
}

fn refresh_vue3_generic_interface_declaration_group(
    source: &str,
    declarations: &[&TSInterfaceDeclaration<'_>],
    analysis: &mut Vue3ScriptSetupAnalysis,
) -> bool {
    let Some(first) = declarations.first() else {
        return false;
    };
    if declarations.len() == 1 {
        return refresh_vue3_generic_interface_declaration(source, first, analysis);
    }
    let Some(type_parameters) = first.type_parameters.as_ref() else {
        return refresh_vue3_generic_interface_declaration(source, first, analysis);
    };
    let params = type_parameters
        .params
        .iter()
        .map(|param| param.name.name.to_string())
        .collect::<Vec<_>>();
    if params.is_empty()
        || !declarations.iter().all(|declaration| {
            declaration.type_parameters.as_ref().is_some_and(|parameters| {
                parameters
                    .params
                    .iter()
                    .map(|param| param.name.name.as_str())
                    .eq(params.iter().map(String::as_str))
            })
        })
    {
        return refresh_vue3_generic_interface_declaration(source, first, analysis);
    }
    let fragments = declarations
        .iter()
        .filter_map(|declaration| {
            source
                .get(declaration.span.start as usize..declaration.span.end as usize)
                .filter(|source| !source.is_empty())
                .map(|source| Vue3GenericInterfaceFragment {
                    source: source.to_string(),
                    scope: Vue3GenericTypeScope::Local,
                })
        })
        .collect::<Vec<_>>();
    if fragments.len() != declarations.len() {
        return refresh_vue3_generic_interface_declaration(source, first, analysis);
    }
    let name = first.id.name.as_str();
    let mut alias = vue3_generic_type_alias(
        String::new(),
        Vue3GenericTypeAliasKind::Interface,
        params,
    );
    alias.interface_fragments = fragments;
    if analysis.generic_type_aliases.get(name) == Some(&alias) {
        false
    } else {
        analysis.generic_type_aliases.insert(name.to_string(), alias);
        true
    }
}

pub(crate) fn refresh_vue3_merged_interface_declarations(
    source: &str,
    declarations: &[&TSInterfaceDeclaration<'_>],
    analysis: &mut Vue3ScriptSetupAnalysis,
) -> bool {
    let Some(first) = declarations.first() else {
        return false;
    };
    let name = first.id.name.to_string();
    let mut changed = false;
    let runtime = infer_vue3_runtime_type_from_interface_declarations(declarations);
    if analysis.declared_types.get(&name) != Some(&runtime) {
        analysis
            .declared_types
            .insert(name.clone(), runtime.clone());
        changed = true;
    }
    if analysis.define_model_declared_types.get(&name) != Some(&runtime) {
        analysis
            .define_model_declared_types
            .insert(name.clone(), runtime);
        changed = true;
    }
    let props = vue3_type_members_from_interface_declarations(source, declarations, analysis);
    if analysis.props_type_declarations.get(&name) != Some(&props) {
        analysis.props_type_declarations.insert(name.clone(), props);
        changed = true;
    }
    match vue3_keyof_runtime_type_from_interface_declarations(source, declarations, analysis) {
        Some(types) => {
            if analysis.keyof_runtime_type_declarations.get(&name) != Some(&types) {
                analysis
                    .keyof_runtime_type_declarations
                    .insert(name.clone(), types);
                changed = true;
            }
        }
        None => {
            if analysis
                .keyof_runtime_type_declarations
                .remove(&name)
                .is_some()
            {
                changed = true;
            }
        }
    }
    let props_parameter_tuple = infer_vue3_function_parameter_tuple_runtime_type_from_interfaces(
        source,
        declarations,
        analysis,
        Vue3ArrayElementRuntimeMode::Props,
    );
    changed |= refresh_vue3_runtime_type_tuple_declaration(
        &mut analysis.parameter_tuple_runtime_type_declarations,
        &name,
        props_parameter_tuple,
    );

    let model_parameter_tuple = infer_vue3_function_parameter_tuple_runtime_type_from_interfaces(
        source,
        declarations,
        analysis,
        Vue3ArrayElementRuntimeMode::DefineModel,
    );
    changed |= refresh_vue3_runtime_type_tuple_declaration(
        &mut analysis.define_model_parameter_tuple_runtime_type_declarations,
        &name,
        model_parameter_tuple,
    );

    let props_constructor_parameter_tuple =
        infer_vue3_constructor_parameter_tuple_runtime_type_from_interfaces(
            source,
            declarations,
            analysis,
            Vue3ArrayElementRuntimeMode::Props,
        );
    changed |= refresh_vue3_runtime_type_tuple_declaration(
        &mut analysis.constructor_parameter_tuple_runtime_type_declarations,
        &name,
        props_constructor_parameter_tuple,
    );

    let model_constructor_parameter_tuple =
        infer_vue3_constructor_parameter_tuple_runtime_type_from_interfaces(
            source,
            declarations,
            analysis,
            Vue3ArrayElementRuntimeMode::DefineModel,
        );
    changed |= refresh_vue3_runtime_type_tuple_declaration(
        &mut analysis.define_model_constructor_parameter_tuple_runtime_type_declarations,
        &name,
        model_constructor_parameter_tuple,
    );

    let props_return_type = infer_vue3_return_runtime_type_from_interfaces(
        source,
        declarations,
        analysis,
        Vue3ArrayElementRuntimeMode::Props,
    );
    changed |= refresh_vue3_runtime_type_declaration(
        &mut analysis.return_type_runtime_type_declarations,
        &name,
        props_return_type,
    );

    let model_return_type = infer_vue3_return_runtime_type_from_interfaces(
        source,
        declarations,
        analysis,
        Vue3ArrayElementRuntimeMode::DefineModel,
    );
    changed |= refresh_vue3_runtime_type_declaration(
        &mut analysis.define_model_return_type_runtime_type_declarations,
        &name,
        model_return_type,
    );

    let emits = vue3_emits_type_from_interface_declarations(source, declarations, analysis);
    if !emits.events.is_empty() {
        if analysis.emits_type_declarations.get(&name) != Some(&emits) {
            analysis.emits_type_declarations.insert(name, emits);
            changed = true;
        }
    } else if analysis.emits_type_declarations.remove(&name).is_some() {
        changed = true;
    }
    changed
}

pub(crate) fn refresh_vue3_runtime_type_tuple_declaration(
    declarations: &mut BTreeMap<String, Vue3RuntimeTypeTuple>,
    name: &str,
    tuple: Option<Vue3RuntimeTypeTuple>,
) -> bool {
    match tuple {
        Some(tuple) => {
            if declarations.get(name) != Some(&tuple) {
                declarations.insert(name.to_string(), tuple);
                return true;
            }
        }
        None => {
            if declarations.remove(name).is_some() {
                return true;
            }
        }
    }
    false
}

pub(crate) fn refresh_vue3_runtime_type_declaration(
    declarations: &mut BTreeMap<String, Vec<String>>,
    name: &str,
    types: Option<Vec<String>>,
) -> bool {
    match types {
        Some(types) => {
            if declarations.get(name) != Some(&types) {
                declarations.insert(name.to_string(), types);
                return true;
            }
        }
        None => {
            if declarations.remove(name).is_some() {
                return true;
            }
        }
    }
    false
}
