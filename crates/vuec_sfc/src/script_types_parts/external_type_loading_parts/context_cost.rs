fn vue3_external_string_collection_cost<'a>(
    values: impl IntoIterator<Item = &'a String>,
) -> usize {
    values.into_iter().fold(0usize, |cost, value| {
        cost.saturating_add(value.len())
    })
}

fn vue3_external_string_map_cost<V>(
    values: &BTreeMap<String, V>,
    value_cost: impl Fn(&V) -> usize,
) -> usize {
    values.iter().fold(0usize, |cost, (key, value)| {
        cost.saturating_add(key.len())
            .saturating_add(value_cost(value))
    })
}

fn vue3_external_string_vec_cost(values: &[String]) -> usize {
    vue3_external_string_collection_cost(values)
}

fn vue3_external_string_set_cost(values: &BTreeSet<String>) -> usize {
    vue3_external_string_collection_cost(values)
}

fn vue3_external_runtime_prop_cache_cost(prop: &Vue27RuntimeProp) -> usize {
    prop.key
        .len()
        .saturating_add(vue3_external_string_vec_cost(&prop.types))
        .saturating_add(prop.default.as_ref().map_or(0, String::len))
        .saturating_add(
            prop.type_annotation_source
                .as_ref()
                .map_or(0, String::len),
        )
        .saturating_add(prop.member_source.as_ref().map_or(0, String::len))
}

fn vue3_external_type_members_cache_cost(members: &Vue27TypeMembers) -> usize {
    members
        .source
        .len()
        .saturating_add(
            members
                .members
                .iter()
                .fold(0usize, |cost, prop| {
                    cost.saturating_add(vue3_external_runtime_prop_cache_cost(prop))
                }),
        )
        .saturating_add(vue3_external_string_vec_cost(&members.errors))
}

fn vue3_external_runtime_tuple_cache_cost(tuple: &Vue3RuntimeTypeTuple) -> usize {
    tuple.iter().fold(0usize, |cost, item| {
        cost.saturating_add(vue3_external_string_vec_cost(item))
    })
}

fn vue3_external_named_projection_value_cost<T>(
    name: &str,
    value: Option<&T>,
    value_cost: impl FnOnce(&T) -> usize,
) -> usize {
    value.map_or(0, |value| name.len().saturating_add(value_cost(value)))
}

fn vue3_external_named_projection_value_len_cost<T>(
    name_len: usize,
    value: Option<&T>,
    value_cost: impl FnOnce(&T) -> usize,
) -> usize {
    value.map_or(0, |value| name_len.saturating_add(value_cost(value)))
}

fn vue3_external_generic_alias_payload_cost(alias: &Vue3GenericTypeAlias) -> usize {
    alias.interface_fragments.iter().fold(
        alias
            .source
            .len()
            .saturating_add(vue3_external_string_vec_cost(&alias.params)),
        |cost, fragment| cost.saturating_add(fragment.source.len()),
    )
}

pub(crate) fn vue3_type_alias_projection_work(
    analysis: &Vue3ScriptSetupAnalysis,
    source_name: &str,
    target_name: &str,
) -> usize {
    let name = source_name;
    let work = [
        vue3_external_named_projection_value_cost(
            name,
            analysis.declared_types.get(name),
            |values| vue3_external_string_vec_cost(values),
        ),
        vue3_external_named_projection_value_cost(
            name,
            analysis.define_model_declared_types.get(name),
            |values| vue3_external_string_vec_cost(values),
        ),
        vue3_external_named_projection_value_cost(
            name,
            analysis.type_query_declared_types.get(name),
            |values| vue3_external_string_vec_cost(values),
        ),
        vue3_external_named_projection_value_cost(
            name,
            analysis.define_model_type_query_declared_types.get(name),
            |values| vue3_external_string_vec_cost(values),
        ),
        vue3_external_named_projection_value_cost(
            name,
            analysis.keyof_type_query_declared_types.get(name),
            |values| vue3_external_string_vec_cost(values),
        ),
        vue3_external_named_projection_value_cost(
            name,
            analysis.props_type_declarations.get(name),
            vue3_external_type_members_cache_cost,
        ),
        vue3_external_named_projection_value_cost(
            name,
            analysis.keyof_runtime_type_declarations.get(name),
            |values| vue3_external_string_vec_cost(values),
        ),
        vue3_external_named_projection_value_cost(
            name,
            analysis.tuple_runtime_type_declarations.get(name),
            vue3_external_runtime_tuple_cache_cost,
        ),
        vue3_external_named_projection_value_cost(
            name,
            analysis
                .define_model_tuple_runtime_type_declarations
                .get(name),
            vue3_external_runtime_tuple_cache_cost,
        ),
        vue3_external_named_projection_value_cost(
            name,
            analysis.array_element_runtime_type_declarations.get(name),
            |values| vue3_external_string_vec_cost(values),
        ),
        vue3_external_named_projection_value_cost(
            name,
            analysis
                .define_model_array_element_runtime_type_declarations
                .get(name),
            |values| vue3_external_string_vec_cost(values),
        ),
        vue3_external_named_projection_value_cost(
            name,
            analysis.parameter_tuple_runtime_type_declarations.get(name),
            vue3_external_runtime_tuple_cache_cost,
        ),
        vue3_external_named_projection_value_cost(
            name,
            analysis
                .define_model_parameter_tuple_runtime_type_declarations
                .get(name),
            vue3_external_runtime_tuple_cache_cost,
        ),
        vue3_external_named_projection_value_cost(
            name,
            analysis
                .constructor_parameter_tuple_runtime_type_declarations
                .get(name),
            vue3_external_runtime_tuple_cache_cost,
        ),
        vue3_external_named_projection_value_cost(
            name,
            analysis
                .define_model_constructor_parameter_tuple_runtime_type_declarations
                .get(name),
            vue3_external_runtime_tuple_cache_cost,
        ),
        vue3_external_named_projection_value_cost(
            name,
            analysis.return_type_runtime_type_declarations.get(name),
            |values| vue3_external_string_vec_cost(values),
        ),
        vue3_external_named_projection_value_cost(
            name,
            analysis
                .define_model_return_type_runtime_type_declarations
                .get(name),
            |values| vue3_external_string_vec_cost(values),
        ),
        vue3_external_named_projection_value_cost(
            name,
            analysis.props_options_type_declarations.get(name),
            vue3_external_type_members_cache_cost,
        ),
        vue3_external_named_projection_value_cost(
            name,
            analysis.return_type_props_options_declarations.get(name),
            vue3_external_type_members_cache_cost,
        ),
        vue3_external_named_projection_value_cost(
            name,
            analysis.generic_type_aliases.get(name),
            vue3_external_generic_alias_payload_cost,
        ),
        vue3_external_named_projection_value_cost(
            name,
            analysis.string_literal_type_declarations.get(name),
            vue3_external_string_set_cost,
        ),
        vue3_external_named_projection_value_cost(
            name,
            analysis.ordered_string_literal_type_declarations.get(name),
            |values| vue3_external_string_vec_cost(values),
        ),
        vue3_external_named_projection_value_cost(
            name,
            analysis.emits_type_declarations.get(name),
            vue3_external_emits_cache_cost,
        ),
        vue3_external_named_projection_value_cost(
            name,
            analysis.type_sources.get(name),
            String::len,
        ),
        vue3_external_named_projection_value_cost(
            name,
            analysis.type_direct_deps.get(name),
            |values| vue3_external_string_vec_cost(values),
        ),
        vue3_external_named_projection_value_cost(
            name,
            analysis.type_deps.get(name),
            vue3_external_string_set_cost,
        ),
        vue3_external_named_projection_value_cost(
            name,
            analysis.unresolved_import_sources.get(name),
            String::len,
        ),
        if analysis.silent_unresolved_type_names.contains(name) {
            name.len()
        } else {
            0
        },
    ]
    .into_iter()
    .fold(0usize, usize::saturating_add);
    if work == 0 {
        0
    } else {
        work.saturating_add(
            target_name
                .len()
                .saturating_sub(source_name.len())
                .saturating_mul(32),
        )
    }
}

pub(crate) fn vue3_external_type_alias_projection_work(
    context: &Vue27TypeContext,
    source_name: &str,
    target_name_len: usize,
    dependency: &str,
) -> usize {
    macro_rules! projection_cost {
        ($field:ident, $value_cost:expr) => {
            vue3_external_named_projection_value_len_cost(
                target_name_len,
                context.$field.get(source_name),
                $value_cost,
            )
        };
    }

    let mut work = [
        projection_cost!(declared_types, |values| vue3_external_string_vec_cost(
            values
        )),
        projection_cost!(define_model_declared_types, |values| {
            vue3_external_string_vec_cost(values)
        }),
        projection_cost!(type_query_declared_types, |values| {
            vue3_external_string_vec_cost(values)
        }),
        projection_cost!(define_model_type_query_declared_types, |values| {
            vue3_external_string_vec_cost(values)
        }),
        projection_cost!(keyof_type_query_declared_types, |values| {
            vue3_external_string_vec_cost(values)
        }),
        projection_cost!(
            props_type_declarations,
            vue3_external_type_members_cache_cost
        ),
        projection_cost!(keyof_runtime_type_declarations, |values| {
            vue3_external_string_vec_cost(values)
        }),
        projection_cost!(
            tuple_runtime_type_declarations,
            vue3_external_runtime_tuple_cache_cost
        ),
        projection_cost!(
            define_model_tuple_runtime_type_declarations,
            vue3_external_runtime_tuple_cache_cost
        ),
        projection_cost!(array_element_runtime_type_declarations, |values| {
            vue3_external_string_vec_cost(values)
        }),
        projection_cost!(
            define_model_array_element_runtime_type_declarations,
            |values| vue3_external_string_vec_cost(values)
        ),
        projection_cost!(
            parameter_tuple_runtime_type_declarations,
            vue3_external_runtime_tuple_cache_cost
        ),
        projection_cost!(
            define_model_parameter_tuple_runtime_type_declarations,
            vue3_external_runtime_tuple_cache_cost
        ),
        projection_cost!(
            constructor_parameter_tuple_runtime_type_declarations,
            vue3_external_runtime_tuple_cache_cost
        ),
        projection_cost!(
            define_model_constructor_parameter_tuple_runtime_type_declarations,
            vue3_external_runtime_tuple_cache_cost
        ),
        projection_cost!(return_type_runtime_type_declarations, |values| {
            vue3_external_string_vec_cost(values)
        }),
        projection_cost!(
            define_model_return_type_runtime_type_declarations,
            |values| vue3_external_string_vec_cost(values)
        ),
        projection_cost!(
            props_options_type_declarations,
            vue3_external_type_members_cache_cost
        ),
        projection_cost!(
            return_type_props_options_declarations,
            vue3_external_type_members_cache_cost
        ),
        projection_cost!(generic_type_aliases, |alias| {
            vue3_external_generic_alias_payload_cost(alias)
        }),
        projection_cost!(string_literal_type_declarations, |values| {
            vue3_external_string_set_cost(values)
        }),
        projection_cost!(ordered_string_literal_type_declarations, |values| {
            vue3_external_string_vec_cost(values)
        }),
        projection_cost!(emits_type_declarations, vue3_external_emits_cache_cost),
        projection_cost!(unresolved_import_sources, String::len),
        if context.silent_unresolved_type_names.contains(source_name) {
            target_name_len
        } else {
            0
        },
    ]
    .into_iter()
    .fold(0usize, usize::saturating_add);

    if context.type_sources.contains_key(source_name) {
        let direct_deps = context
            .type_direct_deps
            .get(source_name)
            .map_or(0, |deps| vue3_external_string_vec_cost(deps));
        let deps = context
            .type_deps
            .get(source_name)
            .map_or(0, vue3_external_string_set_cost);
        work = work
            .saturating_add(target_name_len.saturating_mul(3))
            .saturating_add(dependency.len().saturating_mul(3))
            .saturating_add(direct_deps)
            .saturating_add(deps);
    }

    work.max(
        source_name
            .len()
            .saturating_add(target_name_len)
            .saturating_add(64),
    )
}

pub(crate) fn vue3_external_generic_alias_helpers_projection_work(
    context: &Vue27TypeContext,
    dependency: &str,
) -> usize {
    vue3_external_type_context_cache_cost(context)
        .saturating_add(
            dependency
                .len()
                .saturating_mul(context.type_sources.len())
                .saturating_mul(3),
        )
        .max(64)
}

pub(crate) fn vue3_external_generic_environment_cache_cost(
    environment: &Vue3GenericTypeEnvironment,
) -> usize {
    [
        environment.definition_filename.as_ref().map_or(0, String::len),
        vue3_external_string_map_cost(
            &environment.generic_type_aliases,
            vue3_external_generic_alias_payload_cost,
        ),
        vue3_external_string_map_cost(&environment.declared_types, |types| {
            vue3_external_string_vec_cost(types)
        }),
        vue3_external_string_map_cost(&environment.define_model_declared_types, |types| {
            vue3_external_string_vec_cost(types)
        }),
        vue3_external_string_map_cost(&environment.type_query_declared_types, |types| {
            vue3_external_string_vec_cost(types)
        }),
        vue3_external_string_map_cost(
            &environment.define_model_type_query_declared_types,
            |types| vue3_external_string_vec_cost(types),
        ),
        vue3_external_string_map_cost(
            &environment.keyof_type_query_declared_types,
            |types| vue3_external_string_vec_cost(types),
        ),
        vue3_external_string_map_cost(
            &environment.props_type_declarations,
            vue3_external_type_members_cache_cost,
        ),
        vue3_external_string_map_cost(&environment.keyof_runtime_type_declarations, |types| {
            vue3_external_string_vec_cost(types)
        }),
        vue3_external_string_map_cost(
            &environment.tuple_runtime_type_declarations,
            vue3_external_runtime_tuple_cache_cost,
        ),
        vue3_external_string_map_cost(
            &environment.define_model_tuple_runtime_type_declarations,
            vue3_external_runtime_tuple_cache_cost,
        ),
        vue3_external_string_map_cost(
            &environment.array_element_runtime_type_declarations,
            |types| vue3_external_string_vec_cost(types),
        ),
        vue3_external_string_map_cost(
            &environment.define_model_array_element_runtime_type_declarations,
            |types| vue3_external_string_vec_cost(types),
        ),
        vue3_external_string_map_cost(
            &environment.parameter_tuple_runtime_type_declarations,
            vue3_external_runtime_tuple_cache_cost,
        ),
        vue3_external_string_map_cost(
            &environment.define_model_parameter_tuple_runtime_type_declarations,
            vue3_external_runtime_tuple_cache_cost,
        ),
        vue3_external_string_map_cost(
            &environment.constructor_parameter_tuple_runtime_type_declarations,
            vue3_external_runtime_tuple_cache_cost,
        ),
        vue3_external_string_map_cost(
            &environment.define_model_constructor_parameter_tuple_runtime_type_declarations,
            vue3_external_runtime_tuple_cache_cost,
        ),
        vue3_external_string_map_cost(
            &environment.return_type_runtime_type_declarations,
            |types| vue3_external_string_vec_cost(types),
        ),
        vue3_external_string_map_cost(
            &environment.define_model_return_type_runtime_type_declarations,
            |types| vue3_external_string_vec_cost(types),
        ),
        vue3_external_string_map_cost(
            &environment.props_options_type_declarations,
            vue3_external_type_members_cache_cost,
        ),
        vue3_external_string_map_cost(
            &environment.return_type_props_options_declarations,
            vue3_external_type_members_cache_cost,
        ),
        vue3_external_string_map_cost(
            &environment.string_literal_type_declarations,
            vue3_external_string_set_cost,
        ),
        vue3_external_string_map_cost(
            &environment.ordered_string_literal_type_declarations,
            |values| vue3_external_string_vec_cost(values),
        ),
        vue3_external_string_map_cost(&environment.unresolved_import_sources, String::len),
        vue3_external_string_set_cost(&environment.silent_unresolved_type_names),
    ]
    .into_iter()
    .fold(0usize, usize::saturating_add)
}

fn vue3_analysis_generic_environment_cache_cost(analysis: &Vue3ScriptSetupAnalysis) -> usize {
    [
        analysis.type_filename.as_ref().map_or(0, String::len),
        vue3_external_string_map_cost(
            &analysis.generic_type_aliases,
            vue3_external_generic_alias_payload_cost,
        ),
        vue3_external_string_map_cost(&analysis.declared_types, |types| {
            vue3_external_string_vec_cost(types)
        }),
        vue3_external_string_map_cost(&analysis.define_model_declared_types, |types| {
            vue3_external_string_vec_cost(types)
        }),
        vue3_external_string_map_cost(&analysis.type_query_declared_types, |types| {
            vue3_external_string_vec_cost(types)
        }),
        vue3_external_string_map_cost(
            &analysis.define_model_type_query_declared_types,
            |types| vue3_external_string_vec_cost(types),
        ),
        vue3_external_string_map_cost(
            &analysis.keyof_type_query_declared_types,
            |types| vue3_external_string_vec_cost(types),
        ),
        vue3_external_string_map_cost(
            &analysis.props_type_declarations,
            vue3_external_type_members_cache_cost,
        ),
        vue3_external_string_map_cost(&analysis.keyof_runtime_type_declarations, |types| {
            vue3_external_string_vec_cost(types)
        }),
        vue3_external_string_map_cost(
            &analysis.tuple_runtime_type_declarations,
            vue3_external_runtime_tuple_cache_cost,
        ),
        vue3_external_string_map_cost(
            &analysis.define_model_tuple_runtime_type_declarations,
            vue3_external_runtime_tuple_cache_cost,
        ),
        vue3_external_string_map_cost(
            &analysis.array_element_runtime_type_declarations,
            |types| vue3_external_string_vec_cost(types),
        ),
        vue3_external_string_map_cost(
            &analysis.define_model_array_element_runtime_type_declarations,
            |types| vue3_external_string_vec_cost(types),
        ),
        vue3_external_string_map_cost(
            &analysis.parameter_tuple_runtime_type_declarations,
            vue3_external_runtime_tuple_cache_cost,
        ),
        vue3_external_string_map_cost(
            &analysis.define_model_parameter_tuple_runtime_type_declarations,
            vue3_external_runtime_tuple_cache_cost,
        ),
        vue3_external_string_map_cost(
            &analysis.constructor_parameter_tuple_runtime_type_declarations,
            vue3_external_runtime_tuple_cache_cost,
        ),
        vue3_external_string_map_cost(
            &analysis.define_model_constructor_parameter_tuple_runtime_type_declarations,
            vue3_external_runtime_tuple_cache_cost,
        ),
        vue3_external_string_map_cost(
            &analysis.return_type_runtime_type_declarations,
            |types| vue3_external_string_vec_cost(types),
        ),
        vue3_external_string_map_cost(
            &analysis.define_model_return_type_runtime_type_declarations,
            |types| vue3_external_string_vec_cost(types),
        ),
        vue3_external_string_map_cost(
            &analysis.props_options_type_declarations,
            vue3_external_type_members_cache_cost,
        ),
        vue3_external_string_map_cost(
            &analysis.return_type_props_options_declarations,
            vue3_external_type_members_cache_cost,
        ),
        vue3_external_string_map_cost(
            &analysis.string_literal_type_declarations,
            vue3_external_string_set_cost,
        ),
        vue3_external_string_map_cost(
            &analysis.ordered_string_literal_type_declarations,
            |values| vue3_external_string_vec_cost(values),
        ),
        vue3_external_string_map_cost(&analysis.unresolved_import_sources, String::len),
        vue3_external_string_set_cost(&analysis.silent_unresolved_type_names),
    ]
    .into_iter()
    .fold(0usize, usize::saturating_add)
}

pub(crate) fn vue3_generic_alias_capture_work(
    analysis: &Vue3ScriptSetupAnalysis,
    names: &BTreeSet<String>,
) -> usize {
    let mut captures_local_scope = false;
    let payload = names
        .iter()
        .filter_map(|name| {
            analysis
                .generic_type_aliases
                .get(name)
                .map(|alias| (name, alias))
        })
        .fold(0usize, |cost, (name, alias)| {
            captures_local_scope |= matches!(&alias.scope, Vue3GenericTypeScope::Local)
                || alias
                    .interface_fragments
                    .iter()
                    .any(|fragment| matches!(&fragment.scope, Vue3GenericTypeScope::Local));
            cost.saturating_add(name.len())
                .saturating_add(vue3_external_generic_alias_payload_cost(alias))
        });
    if captures_local_scope {
        payload.saturating_add(vue3_analysis_generic_environment_cache_cost(analysis))
    } else {
        payload
    }
}

pub(crate) fn vue3_local_generic_scope_capture_work(
    analysis: &Vue3ScriptSetupAnalysis,
) -> usize {
    let captures_local_scope = analysis.generic_type_aliases.values().any(|alias| {
        matches!(&alias.scope, Vue3GenericTypeScope::Local)
            || alias
                .interface_fragments
                .iter()
                .any(|fragment| matches!(&fragment.scope, Vue3GenericTypeScope::Local))
    });
    if captures_local_scope {
        vue3_analysis_generic_environment_cache_cost(analysis)
    } else {
        0
    }
}

fn vue3_external_generic_aliases_cache_cost(
    aliases: &BTreeMap<String, Vue3GenericTypeAlias>,
) -> usize {
    let mut cost = vue3_external_string_map_cost(aliases, vue3_external_generic_alias_payload_cost);
    let mut environments = BTreeSet::new();
    let mut pending = Vec::new();
    for alias in aliases.values() {
        collect_vue3_generic_alias_captured_environments(alias, &mut pending);
    }
    while let Some(environment) = pending.pop() {
        let identity = std::sync::Arc::as_ptr(environment) as usize;
        if !environments.insert(identity) {
            continue;
        }
        cost = cost.saturating_add(vue3_external_generic_environment_cache_cost(environment));
        for alias in environment.generic_type_aliases.values() {
            collect_vue3_generic_alias_captured_environments(alias, &mut pending);
        }
    }
    cost
}

fn collect_vue3_generic_alias_captured_environments<'a>(
    alias: &'a Vue3GenericTypeAlias,
    environments: &mut Vec<&'a std::sync::Arc<Vue3GenericTypeEnvironment>>,
) {
    if let Vue3GenericTypeScope::Captured(environment) = &alias.scope {
        environments.push(environment);
    }
    for fragment in &alias.interface_fragments {
        if let Vue3GenericTypeScope::Captured(environment) = &fragment.scope {
            environments.push(environment);
        }
    }
}

fn vue3_external_emits_cache_cost(emits: &Vue27EmitsType) -> usize {
    emits
        .source
        .len()
        .saturating_add(vue3_external_string_vec_cost(&emits.events))
}

fn vue3_external_type_context_cache_cost(context: &Vue27TypeContext) -> usize {
    let generic_aliases_cost =
        vue3_external_generic_aliases_cache_cost(&context.generic_type_aliases);
    [
        vue3_external_string_map_cost(&context.declared_types, |types| {
            vue3_external_string_vec_cost(types)
        }),
        vue3_external_string_map_cost(&context.define_model_declared_types, |types| {
            vue3_external_string_vec_cost(types)
        }),
        vue3_external_string_map_cost(&context.type_query_declared_types, |types| {
            vue3_external_string_vec_cost(types)
        }),
        vue3_external_string_map_cost(&context.define_model_type_query_declared_types, |types| {
            vue3_external_string_vec_cost(types)
        }),
        vue3_external_string_map_cost(&context.keyof_type_query_declared_types, |types| {
            vue3_external_string_vec_cost(types)
        }),
        vue3_external_string_map_cost(
            &context.props_type_declarations,
            vue3_external_type_members_cache_cost,
        ),
        vue3_external_string_map_cost(&context.keyof_runtime_type_declarations, |types| {
            vue3_external_string_vec_cost(types)
        }),
        vue3_external_string_map_cost(
            &context.tuple_runtime_type_declarations,
            vue3_external_runtime_tuple_cache_cost,
        ),
        vue3_external_string_map_cost(
            &context.define_model_tuple_runtime_type_declarations,
            vue3_external_runtime_tuple_cache_cost,
        ),
        vue3_external_string_map_cost(&context.array_element_runtime_type_declarations, |types| {
            vue3_external_string_vec_cost(types)
        }),
        vue3_external_string_map_cost(
            &context.define_model_array_element_runtime_type_declarations,
            |types| vue3_external_string_vec_cost(types),
        ),
        vue3_external_string_map_cost(
            &context.parameter_tuple_runtime_type_declarations,
            vue3_external_runtime_tuple_cache_cost,
        ),
        vue3_external_string_map_cost(
            &context.define_model_parameter_tuple_runtime_type_declarations,
            vue3_external_runtime_tuple_cache_cost,
        ),
        vue3_external_string_map_cost(
            &context.constructor_parameter_tuple_runtime_type_declarations,
            vue3_external_runtime_tuple_cache_cost,
        ),
        vue3_external_string_map_cost(
            &context.define_model_constructor_parameter_tuple_runtime_type_declarations,
            vue3_external_runtime_tuple_cache_cost,
        ),
        vue3_external_string_map_cost(&context.return_type_runtime_type_declarations, |types| {
            vue3_external_string_vec_cost(types)
        }),
        vue3_external_string_map_cost(
            &context.define_model_return_type_runtime_type_declarations,
            |types| vue3_external_string_vec_cost(types),
        ),
        vue3_external_string_map_cost(
            &context.props_options_type_declarations,
            vue3_external_type_members_cache_cost,
        ),
        vue3_external_string_map_cost(
            &context.return_type_props_options_declarations,
            vue3_external_type_members_cache_cost,
        ),
        generic_aliases_cost,
        vue3_external_string_map_cost(&context.string_literal_type_declarations, |values| {
            vue3_external_string_set_cost(values)
        }),
        vue3_external_string_map_cost(
            &context.ordered_string_literal_type_declarations,
            |values| vue3_external_string_vec_cost(values),
        ),
        vue3_external_string_map_cost(
            &context.emits_type_declarations,
            vue3_external_emits_cache_cost,
        ),
        vue3_external_string_map_cost(&context.type_sources, String::len),
        vue3_external_string_map_cost(&context.type_direct_deps, |deps| {
            vue3_external_string_vec_cost(deps)
        }),
        vue3_external_string_map_cost(&context.type_deps, |deps| {
            vue3_external_string_set_cost(deps)
        }),
        vue3_external_string_map_cost(&context.unresolved_import_sources, String::len),
        vue3_external_string_set_cost(&context.silent_unresolved_type_names),
    ]
    .into_iter()
    .fold(0usize, usize::saturating_add)
}

#[cfg(test)]
mod generic_environment_cache_cost_tests {
    use super::*;

    fn alias(
        source: &str,
        scope: Vue3GenericTypeScope,
    ) -> Vue3GenericTypeAlias {
        Vue3GenericTypeAlias {
            source: source.into(),
            kind: Vue3GenericTypeAliasKind::TypeAlias,
            params: vec!["T".into()],
            scope,
            interface_fragments: Vec::new(),
        }
    }

    #[test]
    fn generic_alias_cache_cost_walks_and_deduplicates_environment_dags() {
        let mut leaf = Vue3GenericTypeEnvironment::default();
        leaf.declared_types
            .insert("Leaf".into(), vec!["String".into()]);
        let leaf = std::sync::Arc::new(leaf);

        let mut inner = Vue3GenericTypeEnvironment::default();
        inner.generic_type_aliases.insert(
            "First".into(),
            alias("type First<T> = T", Vue3GenericTypeScope::Captured(leaf.clone())),
        );
        inner.generic_type_aliases.insert(
            "Second".into(),
            alias("type Second<T> = T", Vue3GenericTypeScope::Captured(leaf.clone())),
        );
        let inner = std::sync::Arc::new(inner);

        let mut root = alias(
            "type Root<T> = First<T>",
            Vue3GenericTypeScope::Captured(inner.clone()),
        );
        root.interface_fragments.push(Vue3GenericInterfaceFragment {
            source: "interface Root<T> { value: T }".into(),
            scope: Vue3GenericTypeScope::Captured(leaf.clone()),
        });
        let aliases = [("Root".to_string(), root)].into_iter().collect();

        let expected = vue3_external_string_map_cost(
            &aliases,
            vue3_external_generic_alias_payload_cost,
        )
        .saturating_add(vue3_external_generic_environment_cache_cost(&inner))
        .saturating_add(vue3_external_generic_environment_cache_cost(&leaf));
        assert_eq!(vue3_external_generic_aliases_cache_cost(&aliases), expected);
    }
}
