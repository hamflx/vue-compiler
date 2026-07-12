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

fn vue3_external_generic_alias_cache_cost(alias: &Vue3GenericTypeAlias) -> usize {
    [
        alias.source.len(),
        vue3_external_string_vec_cost(&alias.params),
        vue3_external_string_map_cost(&alias.declared_types, |types| {
            vue3_external_string_vec_cost(types)
        }),
        vue3_external_string_map_cost(&alias.define_model_declared_types, |types| {
            vue3_external_string_vec_cost(types)
        }),
        vue3_external_string_map_cost(&alias.type_query_declared_types, |types| {
            vue3_external_string_vec_cost(types)
        }),
        vue3_external_string_map_cost(&alias.define_model_type_query_declared_types, |types| {
            vue3_external_string_vec_cost(types)
        }),
        vue3_external_string_map_cost(&alias.keyof_type_query_declared_types, |types| {
            vue3_external_string_vec_cost(types)
        }),
        vue3_external_string_map_cost(
            &alias.props_type_declarations,
            vue3_external_type_members_cache_cost,
        ),
        vue3_external_string_map_cost(&alias.keyof_runtime_type_declarations, |types| {
            vue3_external_string_vec_cost(types)
        }),
        vue3_external_string_map_cost(
            &alias.tuple_runtime_type_declarations,
            vue3_external_runtime_tuple_cache_cost,
        ),
        vue3_external_string_map_cost(
            &alias.define_model_tuple_runtime_type_declarations,
            vue3_external_runtime_tuple_cache_cost,
        ),
        vue3_external_string_map_cost(&alias.array_element_runtime_type_declarations, |types| {
            vue3_external_string_vec_cost(types)
        }),
        vue3_external_string_map_cost(
            &alias.define_model_array_element_runtime_type_declarations,
            |types| vue3_external_string_vec_cost(types),
        ),
        vue3_external_string_map_cost(
            &alias.parameter_tuple_runtime_type_declarations,
            vue3_external_runtime_tuple_cache_cost,
        ),
        vue3_external_string_map_cost(
            &alias.define_model_parameter_tuple_runtime_type_declarations,
            vue3_external_runtime_tuple_cache_cost,
        ),
        vue3_external_string_map_cost(
            &alias.constructor_parameter_tuple_runtime_type_declarations,
            vue3_external_runtime_tuple_cache_cost,
        ),
        vue3_external_string_map_cost(
            &alias.define_model_constructor_parameter_tuple_runtime_type_declarations,
            vue3_external_runtime_tuple_cache_cost,
        ),
        vue3_external_string_map_cost(&alias.return_type_runtime_type_declarations, |types| {
            vue3_external_string_vec_cost(types)
        }),
        vue3_external_string_map_cost(
            &alias.define_model_return_type_runtime_type_declarations,
            |types| vue3_external_string_vec_cost(types),
        ),
        vue3_external_string_map_cost(
            &alias.props_options_type_declarations,
            vue3_external_type_members_cache_cost,
        ),
        vue3_external_string_map_cost(
            &alias.return_type_props_options_declarations,
            vue3_external_type_members_cache_cost,
        ),
        vue3_external_string_map_cost(&alias.string_literal_type_declarations, |values| {
            vue3_external_string_set_cost(values)
        }),
        vue3_external_string_map_cost(
            &alias.ordered_string_literal_type_declarations,
            |values| vue3_external_string_vec_cost(values),
        ),
        vue3_external_string_map_cost(&alias.unresolved_import_sources, String::len),
        vue3_external_string_set_cost(&alias.silent_unresolved_type_names),
    ]
    .into_iter()
    .fold(0usize, usize::saturating_add)
}

fn vue3_external_emits_cache_cost(emits: &Vue27EmitsType) -> usize {
    emits
        .source
        .len()
        .saturating_add(vue3_external_string_vec_cost(&emits.events))
}

fn vue3_external_type_context_cache_cost(context: &Vue27TypeContext) -> usize {
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
        vue3_external_string_map_cost(
            &context.generic_type_aliases,
            vue3_external_generic_alias_cache_cost,
        ),
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
