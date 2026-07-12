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

fn vue3_external_generic_environment_cache_cost(
    environment: &Vue3GenericTypeEnvironment,
) -> usize {
    [
        environment.definition_filename.as_ref().map_or(0, String::len),
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

fn vue3_external_generic_aliases_cache_cost(
    aliases: &BTreeMap<String, Vue3GenericTypeAlias>,
) -> usize {
    let mut environments = BTreeSet::new();
    aliases.iter().fold(0usize, |cost, (name, alias)| {
        let cost = cost
            .saturating_add(name.len())
            .saturating_add(alias.source.len())
            .saturating_add(vue3_external_string_vec_cost(&alias.params));
        let Vue3GenericTypeScope::Captured(environment) = &alias.scope else {
            debug_assert!(false, "cached generic alias retained a local scope");
            return cost;
        };
        let identity = std::sync::Arc::as_ptr(environment) as usize;
        if environments.insert(identity) {
            cost.saturating_add(vue3_external_generic_environment_cache_cost(environment))
        } else {
            cost
        }
    })
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
