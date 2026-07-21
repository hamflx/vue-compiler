pub(crate) fn refresh_vue3_type_alias_declaration(
    source: &str,
    declaration: &TSTypeAliasDeclaration<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) -> bool {
    let name = declaration.id.name.to_string();
    let mut changed = false;

    changed |= refresh_vue3_generic_type_alias(source, declaration, analysis);

    match vue3_resolve_string_type_keys(&declaration.type_annotation, analysis) {
        Some(keys) => {
            if analysis.string_literal_type_declarations.get(&name) != Some(&keys) {
                analysis
                    .string_literal_type_declarations
                    .insert(name.clone(), keys);
                changed = true;
            }
        }
        None => {
            if analysis
                .string_literal_type_declarations
                .remove(&name)
                .is_some()
            {
                changed = true;
            }
        }
    }

    match vue3_resolve_ordered_string_type_keys(&declaration.type_annotation, analysis) {
        Some(keys) => {
            if analysis.ordered_string_literal_type_declarations.get(&name) != Some(&keys) {
                analysis
                    .ordered_string_literal_type_declarations
                    .insert(name.clone(), keys);
                changed = true;
            }
        }
        None => {
            if analysis
                .ordered_string_literal_type_declarations
                .remove(&name)
                .is_some()
            {
                changed = true;
            }
        }
    }

    match infer_vue3_keyof_runtime_type(&declaration.type_annotation, analysis) {
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

    let props_tuple = infer_vue3_tuple_runtime_type(
        &declaration.type_annotation,
        analysis,
        Vue3ArrayElementRuntimeMode::Props,
    );
    changed |= refresh_vue3_runtime_type_tuple_declaration(
        &mut analysis.tuple_runtime_type_declarations,
        &name,
        props_tuple,
    );

    let model_tuple = infer_vue3_tuple_runtime_type(
        &declaration.type_annotation,
        analysis,
        Vue3ArrayElementRuntimeMode::DefineModel,
    );
    changed |= refresh_vue3_runtime_type_tuple_declaration(
        &mut analysis.define_model_tuple_runtime_type_declarations,
        &name,
        model_tuple,
    );

    match infer_vue3_array_element_runtime_type(
        &declaration.type_annotation,
        analysis,
        Vue3ArrayElementRuntimeMode::Props,
    ) {
        Some(types) => {
            if analysis.array_element_runtime_type_declarations.get(&name) != Some(&types) {
                analysis
                    .array_element_runtime_type_declarations
                    .insert(name.clone(), types);
                changed = true;
            }
        }
        None => {
            if analysis
                .array_element_runtime_type_declarations
                .remove(&name)
                .is_some()
            {
                changed = true;
            }
        }
    }

    match infer_vue3_array_element_runtime_type(
        &declaration.type_annotation,
        analysis,
        Vue3ArrayElementRuntimeMode::DefineModel,
    ) {
        Some(types) => {
            if analysis
                .define_model_array_element_runtime_type_declarations
                .get(&name)
                != Some(&types)
            {
                analysis
                    .define_model_array_element_runtime_type_declarations
                    .insert(name.clone(), types);
                changed = true;
            }
        }
        None => {
            if analysis
                .define_model_array_element_runtime_type_declarations
                .remove(&name)
                .is_some()
            {
                changed = true;
            }
        }
    }

    let props_parameter_tuple = infer_vue3_function_parameter_tuple_runtime_type(
        &declaration.type_annotation,
        analysis,
        Vue3ArrayElementRuntimeMode::Props,
    );
    changed |= refresh_vue3_runtime_type_tuple_declaration(
        &mut analysis.parameter_tuple_runtime_type_declarations,
        &name,
        props_parameter_tuple,
    );

    let model_parameter_tuple = infer_vue3_function_parameter_tuple_runtime_type(
        &declaration.type_annotation,
        analysis,
        Vue3ArrayElementRuntimeMode::DefineModel,
    );
    changed |= refresh_vue3_runtime_type_tuple_declaration(
        &mut analysis.define_model_parameter_tuple_runtime_type_declarations,
        &name,
        model_parameter_tuple,
    );

    let props_constructor_parameter_tuple = infer_vue3_constructor_parameter_tuple_runtime_type(
        &declaration.type_annotation,
        analysis,
        Vue3ArrayElementRuntimeMode::Props,
    );
    changed |= refresh_vue3_runtime_type_tuple_declaration(
        &mut analysis.constructor_parameter_tuple_runtime_type_declarations,
        &name,
        props_constructor_parameter_tuple,
    );

    let model_constructor_parameter_tuple = infer_vue3_constructor_parameter_tuple_runtime_type(
        &declaration.type_annotation,
        analysis,
        Vue3ArrayElementRuntimeMode::DefineModel,
    );
    changed |= refresh_vue3_runtime_type_tuple_declaration(
        &mut analysis.define_model_constructor_parameter_tuple_runtime_type_declarations,
        &name,
        model_constructor_parameter_tuple,
    );

    let props_return_type = infer_vue3_return_runtime_type(
        &declaration.type_annotation,
        analysis,
        Vue3ArrayElementRuntimeMode::Props,
    );
    changed |= refresh_vue3_runtime_type_declaration(
        &mut analysis.return_type_runtime_type_declarations,
        &name,
        props_return_type,
    );

    let model_return_type = infer_vue3_return_runtime_type(
        &declaration.type_annotation,
        analysis,
        Vue3ArrayElementRuntimeMode::DefineModel,
    );
    changed |= refresh_vue3_runtime_type_declaration(
        &mut analysis.define_model_return_type_runtime_type_declarations,
        &name,
        model_return_type,
    );

    let runtime = infer_vue3_runtime_type(&declaration.type_annotation, analysis);
    if analysis.declared_types.get(&name) != Some(&runtime) {
        analysis.declared_types.insert(name.clone(), runtime);
        changed = true;
    }

    let model_runtime =
        infer_vue3_define_model_runtime_type(&declaration.type_annotation, analysis);
    if analysis.define_model_declared_types.get(&name) != Some(&model_runtime) {
        analysis
            .define_model_declared_types
            .insert(name.clone(), model_runtime);
        changed = true;
    }

    match vue3_resolve_projectable_props_type(source, &declaration.type_annotation, analysis) {
        Some(props) => {
            if analysis.props_type_declarations.get(&name) != Some(&props) {
                analysis.props_type_declarations.insert(name.clone(), props);
                changed = true;
            }
        }
        None => {
            if analysis.props_type_declarations.remove(&name).is_some() {
                changed = true;
            }
        }
    }

    match vue3_props_options_type_members(source, &declaration.type_annotation, analysis) {
        Some(props_options) => {
            if analysis.props_options_type_declarations.get(&name) != Some(&props_options) {
                analysis
                    .props_options_type_declarations
                    .insert(name.clone(), props_options);
                changed = true;
            }
        }
        None => {
            if analysis
                .props_options_type_declarations
                .remove(&name)
                .is_some()
            {
                changed = true;
            }
        }
    }

    let emits = vue3_resolve_emits_type(source, &declaration.type_annotation, analysis);
    match emits {
        Some(emits) if !emits.events.is_empty() => {
            if analysis.emits_type_declarations.get(&name) != Some(&emits) {
                analysis.emits_type_declarations.insert(name, emits);
                changed = true;
            }
        }
        _ => {
            if analysis.emits_type_declarations.remove(&name).is_some() {
                changed = true;
            }
        }
    }

    changed
}

pub(crate) fn register_vue3_type_alias_declaration(
    source: &str,
    declaration: &TSTypeAliasDeclaration<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    let name = declaration.id.name.to_string();
    register_vue3_local_type_name(analysis, &name);
    refresh_vue3_type_alias_declaration(source, declaration, analysis);
}

pub(crate) fn refresh_vue3_generic_type_alias(
    source: &str,
    declaration: &TSTypeAliasDeclaration<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) -> bool {
    let Some(type_parameters) = declaration.type_parameters.as_ref() else {
        return analysis
            .generic_type_aliases
            .remove(declaration.id.name.as_str())
            .is_some();
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
        return analysis
            .generic_type_aliases
            .remove(declaration.id.name.as_str())
            .is_some();
    }
    let alias = vue3_generic_type_alias(
        alias_source,
        Vue3GenericTypeAliasKind::TypeAlias,
        params,
    );
    if analysis
        .generic_type_aliases
        .get(declaration.id.name.as_str())
        != Some(&alias)
    {
        analysis
            .generic_type_aliases
            .insert(declaration.id.name.to_string(), alias);
        true
    } else {
        false
    }
}

pub(crate) fn vue3_generic_type_alias(
    source: String,
    kind: Vue3GenericTypeAliasKind,
    params: Vec<String>,
) -> Vue3GenericTypeAlias {
    Vue3GenericTypeAlias {
        source,
        kind,
        params,
        scope: Vue3GenericTypeScope::Local,
        interface_fragments: Vec::new(),
    }
}

impl Vue3GenericTypeEnvironment {
    pub(crate) fn from_analysis(analysis: &Vue3ScriptSetupAnalysis) -> Self {
        Self {
            definition_filename: analysis.type_filename.clone(),
            generic_type_aliases: analysis.generic_type_aliases.clone(),
            declared_types: analysis.declared_types.clone(),
            define_model_declared_types: analysis.define_model_declared_types.clone(),
            type_query_declared_types: analysis.type_query_declared_types.clone(),
            define_model_type_query_declared_types: analysis
                .define_model_type_query_declared_types
                .clone(),
            keyof_type_query_declared_types: analysis.keyof_type_query_declared_types.clone(),
            props_type_declarations: analysis.props_type_declarations.clone(),
            keyof_runtime_type_declarations: analysis.keyof_runtime_type_declarations.clone(),
            tuple_runtime_type_declarations: analysis.tuple_runtime_type_declarations.clone(),
            define_model_tuple_runtime_type_declarations: analysis
                .define_model_tuple_runtime_type_declarations
                .clone(),
            array_element_runtime_type_declarations: analysis
                .array_element_runtime_type_declarations
                .clone(),
            define_model_array_element_runtime_type_declarations: analysis
                .define_model_array_element_runtime_type_declarations
                .clone(),
            parameter_tuple_runtime_type_declarations: analysis
                .parameter_tuple_runtime_type_declarations
                .clone(),
            define_model_parameter_tuple_runtime_type_declarations: analysis
                .define_model_parameter_tuple_runtime_type_declarations
                .clone(),
            constructor_parameter_tuple_runtime_type_declarations: analysis
                .constructor_parameter_tuple_runtime_type_declarations
                .clone(),
            define_model_constructor_parameter_tuple_runtime_type_declarations: analysis
                .define_model_constructor_parameter_tuple_runtime_type_declarations
                .clone(),
            return_type_runtime_type_declarations: analysis
                .return_type_runtime_type_declarations
                .clone(),
            define_model_return_type_runtime_type_declarations: analysis
                .define_model_return_type_runtime_type_declarations
                .clone(),
            props_options_type_declarations: analysis.props_options_type_declarations.clone(),
            return_type_props_options_declarations: analysis
                .return_type_props_options_declarations
                .clone(),
            string_literal_type_declarations: analysis
                .string_literal_type_declarations
                .clone(),
            ordered_string_literal_type_declarations: analysis
                .ordered_string_literal_type_declarations
                .clone(),
            unresolved_import_sources: analysis.unresolved_import_sources.clone(),
            silent_unresolved_type_names: analysis.silent_unresolved_type_names.clone(),
        }
    }

    pub(crate) fn overlay_analysis(&self, analysis: &mut Vue3ScriptSetupAnalysis) {
        analysis
            .generic_type_aliases
            .extend(self.generic_type_aliases.clone());
        analysis.declared_types.extend(self.declared_types.clone());
        analysis
            .define_model_declared_types
            .extend(self.define_model_declared_types.clone());
        analysis
            .type_query_declared_types
            .extend(self.type_query_declared_types.clone());
        analysis
            .define_model_type_query_declared_types
            .extend(self.define_model_type_query_declared_types.clone());
        analysis
            .keyof_type_query_declared_types
            .extend(self.keyof_type_query_declared_types.clone());
        analysis
            .props_type_declarations
            .extend(self.props_type_declarations.clone());
        analysis
            .keyof_runtime_type_declarations
            .extend(self.keyof_runtime_type_declarations.clone());
        analysis
            .tuple_runtime_type_declarations
            .extend(self.tuple_runtime_type_declarations.clone());
        analysis
            .define_model_tuple_runtime_type_declarations
            .extend(self.define_model_tuple_runtime_type_declarations.clone());
        analysis
            .array_element_runtime_type_declarations
            .extend(self.array_element_runtime_type_declarations.clone());
        analysis
            .define_model_array_element_runtime_type_declarations
            .extend(self.define_model_array_element_runtime_type_declarations.clone());
        analysis
            .parameter_tuple_runtime_type_declarations
            .extend(self.parameter_tuple_runtime_type_declarations.clone());
        analysis
            .define_model_parameter_tuple_runtime_type_declarations
            .extend(self.define_model_parameter_tuple_runtime_type_declarations.clone());
        analysis
            .constructor_parameter_tuple_runtime_type_declarations
            .extend(self.constructor_parameter_tuple_runtime_type_declarations.clone());
        analysis
            .define_model_constructor_parameter_tuple_runtime_type_declarations
            .extend(
                self.define_model_constructor_parameter_tuple_runtime_type_declarations
                    .clone(),
            );
        analysis
            .return_type_runtime_type_declarations
            .extend(self.return_type_runtime_type_declarations.clone());
        analysis
            .define_model_return_type_runtime_type_declarations
            .extend(self.define_model_return_type_runtime_type_declarations.clone());
        analysis
            .props_options_type_declarations
            .extend(self.props_options_type_declarations.clone());
        analysis
            .return_type_props_options_declarations
            .extend(self.return_type_props_options_declarations.clone());
        analysis
            .string_literal_type_declarations
            .extend(self.string_literal_type_declarations.clone());
        analysis
            .ordered_string_literal_type_declarations
            .extend(self.ordered_string_literal_type_declarations.clone());
        analysis
            .unresolved_import_sources
            .extend(self.unresolved_import_sources.clone());
        analysis
            .silent_unresolved_type_names
            .extend(self.silent_unresolved_type_names.iter().cloned());
        analysis.type_filename = self.definition_filename.clone();
    }
}

pub(crate) fn finalize_vue3_local_generic_alias_scopes(analysis: &mut Vue3ScriptSetupAnalysis) {
    if !analysis
        .generic_type_aliases
        .values()
        .any(vue3_generic_alias_has_local_scope)
    {
        return;
    }
    let environment = std::sync::Arc::new(Vue3GenericTypeEnvironment::from_analysis(analysis));
    for alias in analysis.generic_type_aliases.values_mut() {
        if matches!(&alias.scope, Vue3GenericTypeScope::Local) {
            alias.scope = Vue3GenericTypeScope::Captured(environment.clone());
        }
        for fragment in &mut alias.interface_fragments {
            if matches!(&fragment.scope, Vue3GenericTypeScope::Local) {
                fragment.scope = Vue3GenericTypeScope::Captured(environment.clone());
            }
        }
    }
}

pub(crate) fn captured_vue3_generic_aliases_for_child_scope(
    analysis: &Vue3ScriptSetupAnalysis,
    names: &BTreeSet<String>,
) -> BTreeMap<String, Vue3GenericTypeAlias> {
    let mut aliases = names
        .iter()
        .filter_map(|name| {
            analysis
                .generic_type_aliases
                .get(name)
                .cloned()
                .map(|alias| (name.clone(), alias))
        })
        .collect::<BTreeMap<_, _>>();
    if !aliases
        .values()
        .any(vue3_generic_alias_has_local_scope)
    {
        return aliases;
    }
    let environment = std::sync::Arc::new(Vue3GenericTypeEnvironment::from_analysis(analysis));
    for alias in aliases.values_mut() {
        if matches!(&alias.scope, Vue3GenericTypeScope::Local) {
            alias.scope = Vue3GenericTypeScope::Captured(environment.clone());
        }
        for fragment in &mut alias.interface_fragments {
            if matches!(&fragment.scope, Vue3GenericTypeScope::Local) {
                fragment.scope = Vue3GenericTypeScope::Captured(environment.clone());
            }
        }
    }
    aliases
}

fn vue3_generic_alias_has_local_scope(alias: &Vue3GenericTypeAlias) -> bool {
    matches!(&alias.scope, Vue3GenericTypeScope::Local)
        || alias
            .interface_fragments
            .iter()
            .any(|fragment| matches!(&fragment.scope, Vue3GenericTypeScope::Local))
}

pub(crate) fn register_vue3_ts_enum_declaration(
    declaration: &TSEnumDeclaration<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    let name = declaration.id.name.to_string();
    let merge_existing = analysis.local_ts_enum_type_names.contains(&name);
    register_vue3_local_type_name(analysis, &name);
    let runtime = infer_vue3_enum_runtime_type(declaration);
    let mut merged_runtime = if merge_existing {
        analysis
            .declared_types
            .get(&name)
            .cloned()
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    for runtime_type in &runtime {
        push_unique(&mut merged_runtime, runtime_type);
    }
    analysis
        .declared_types
        .insert(name.clone(), merged_runtime.clone());
    analysis
        .define_model_declared_types
        .insert(name.clone(), merged_runtime);
    analysis.local_ts_enum_type_names.insert(name);
}

pub(crate) fn register_vue3_class_type_name(analysis: &mut Vue3ScriptSetupAnalysis, name: &str) {
    register_vue3_local_type_name(analysis, name);
    analysis
        .declared_types
        .insert(name.to_string(), vec!["Object".into()]);
    analysis
        .define_model_declared_types
        .insert(name.to_string(), vec!["Object".into()]);
}

pub(crate) fn register_vue3_declared_function_return_props_options(
    source: &str,
    function: &Function<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    let Some(id) = &function.id else {
        return;
    };
    register_vue3_function_return_projection(source, id.name.as_str(), function, analysis);
    capture_vue3_value_type_projection(analysis, id.name.as_str());
}

pub(crate) fn vue3_function_has_return_projection(function: &Function<'_>) -> bool {
    function.return_type.is_some() || infer_vue3_function_runtime_return_types(function).is_some()
}

pub(crate) fn vue3_function_value_return_type<'a>(
    expression: &'a Expression<'a>,
) -> Option<&'a TSType<'a>> {
    match unwrap_vue3_ts_expression(expression) {
        Expression::ArrowFunctionExpression(function) => function
            .return_type
            .as_ref()
            .map(|return_type| &return_type.type_annotation),
        Expression::FunctionExpression(function) => function
            .return_type
            .as_ref()
            .map(|return_type| &return_type.type_annotation),
        _ => None,
    }
}

pub(crate) fn vue3_function_value_has_return_projection(expression: &Expression<'_>) -> bool {
    match unwrap_vue3_ts_expression(expression) {
        Expression::ArrowFunctionExpression(function) => {
            function.return_type.is_some()
                || infer_vue3_arrow_function_runtime_return_types(function).is_some()
        }
        Expression::FunctionExpression(function) => vue3_function_has_return_projection(function),
        _ => false,
    }
}

pub(crate) fn vue3_default_export_function_value_has_return_projection(
    declaration: &ExportDefaultDeclarationKind<'_>,
) -> bool {
    declaration
        .as_expression()
        .is_some_and(vue3_function_value_has_return_projection)
}

pub(crate) fn vue3_default_export_static_runtime_props_options_is_projectable(
    declaration: &ExportDefaultDeclarationKind<'_>,
) -> bool {
    declaration
        .as_expression()
        .is_some_and(vue3_static_runtime_props_options_is_projectable)
}

pub(crate) fn vue3_variable_declarator_has_function_return_projection(
    declarator: &VariableDeclarator<'_>,
) -> bool {
    vue3_variable_declarator_function_return_type(declarator).is_some()
        || declarator
            .init
            .as_ref()
            .is_some_and(vue3_function_value_has_return_projection)
}

pub(crate) fn vue3_variable_declarator_has_type_projection(
    declarator: &VariableDeclarator<'_>,
) -> bool {
    vue3_variable_declarator_has_function_return_projection(declarator)
        || declarator
            .init
            .as_ref()
            .is_some_and(vue3_static_runtime_props_options_is_projectable)
}

pub(crate) fn vue3_static_runtime_props_options_is_projectable(
    expression: &Expression<'_>,
) -> bool {
    let Some(object) = vue3_static_runtime_props_options_object(expression) else {
        return false;
    };
    vue3_static_runtime_props_options_object_is_projectable(object)
}

pub(crate) fn vue3_static_runtime_props_options_object_is_projectable(
    object: &ObjectExpression<'_>,
) -> bool {
    let mut has_property = false;
    for property in &object.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            return false;
        };
        if property.computed || vue27_property_key_static_name(&property.key).is_none() {
            return false;
        }
        if !vue3_static_runtime_prop_option_is_projectable(&property.value) {
            return false;
        }
        has_property = true;
    }
    has_property
}

pub(crate) fn vue3_static_runtime_prop_option_is_projectable(expression: &Expression<'_>) -> bool {
    if vue3_static_runtime_prop_type_expression_is_projectable(expression) {
        return true;
    }
    let Some(object) = vue3_static_runtime_props_options_object(expression) else {
        return false;
    };
    let mut has_runtime_option_key = false;
    for property in &object.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            return false;
        };
        if property.computed {
            return false;
        }
        let Some(key) = vue27_property_key_static_name(&property.key) else {
            return false;
        };
        match key.as_str() {
            "type" => {
                has_runtime_option_key = true;
            }
            "required" | "default" | "validator" => {
                has_runtime_option_key = true;
            }
            _ => {}
        }
    }
    has_runtime_option_key
}

pub(crate) fn vue3_static_runtime_prop_type_expression_is_projectable(
    expression: &Expression<'_>,
) -> bool {
    match expression {
        Expression::TSAsExpression(expression) => {
            vue3_static_runtime_prop_type_annotation_is_projectable(&expression.type_annotation)
                || vue3_static_runtime_prop_type_expression_is_projectable(&expression.expression)
        }
        Expression::TSTypeAssertion(expression) => {
            vue3_static_runtime_prop_type_annotation_is_projectable(&expression.type_annotation)
                || vue3_static_runtime_prop_type_expression_is_projectable(&expression.expression)
        }
        Expression::TSSatisfiesExpression(expression) => {
            vue3_static_runtime_prop_type_annotation_is_projectable(&expression.type_annotation)
                || vue3_static_runtime_prop_type_expression_is_projectable(&expression.expression)
        }
        Expression::TSNonNullExpression(expression) => {
            vue3_static_runtime_prop_type_expression_is_projectable(&expression.expression)
        }
        Expression::TSInstantiationExpression(expression) => {
            vue3_static_runtime_prop_type_expression_is_projectable(&expression.expression)
        }
        Expression::ParenthesizedExpression(expression) => {
            vue3_static_runtime_prop_type_expression_is_projectable(&expression.expression)
        }
        Expression::Identifier(identifier) => {
            vue3_return_expression_constructor_runtime_name(identifier.name.as_str()).is_some()
        }
        Expression::StaticMemberExpression(member) => {
            vue3_return_expression_constructor_runtime_name(member.property.name.as_str()).is_some()
        }
        Expression::NullLiteral(_) => true,
        Expression::ArrayExpression(array) => array.elements.iter().all(|element| match element {
            ArrayExpressionElement::SpreadElement(_) | ArrayExpressionElement::Elision(_) => false,
            element => element
                .as_expression()
                .is_some_and(vue3_static_runtime_prop_type_expression_is_projectable),
        }),
        _ => false,
    }
}

pub(crate) fn vue3_static_runtime_prop_type_annotation_is_projectable(ty: &TSType<'_>) -> bool {
    match ty {
        TSType::TSTypeReference(reference) => {
            let Some(name) = vue3_ts_type_name_key(&reference.type_name) else {
                return false;
            };
            name == "PropType" || name.ends_with("Constructor")
        }
        TSType::TSImportType(import_type) => import_type.type_arguments.is_some(),
        TSType::TSParenthesizedType(parenthesized) => {
            vue3_static_runtime_prop_type_annotation_is_projectable(&parenthesized.type_annotation)
        }
        _ => false,
    }
}

pub(crate) fn vue3_static_runtime_props_options_object<'a>(
    expression: &'a Expression<'a>,
) -> Option<&'a ObjectExpression<'a>> {
    match unwrap_vue3_ts_expression(expression) {
        Expression::ObjectExpression(object) => Some(object),
        _ => None,
    }
}

pub(crate) fn vue3_variable_declarator_function_return_type<'a>(
    declarator: &'a VariableDeclarator<'a>,
) -> Option<&'a TSType<'a>> {
    if let Some(type_annotation) = declarator.type_annotation.as_ref() {
        if let TSType::TSFunctionType(function) = &type_annotation.type_annotation {
            return Some(&function.return_type.type_annotation);
        }
    }
    declarator
        .init
        .as_ref()
        .and_then(vue3_function_value_return_type)
}

pub(crate) fn infer_vue3_function_runtime_return_types(
    function: &Function<'_>,
) -> Option<Vec<String>> {
    let body = function.body.as_ref()?;
    infer_vue3_function_body_runtime_return_types(body)
}

pub(crate) fn infer_vue3_arrow_function_runtime_return_types(
    function: &ArrowFunctionExpression<'_>,
) -> Option<Vec<String>> {
    if let Some(expression) = function.get_expression() {
        return infer_vue3_return_expression_runtime_types(expression);
    }
    infer_vue3_function_body_runtime_return_types(&function.body)
}

pub(crate) fn infer_vue3_function_body_runtime_return_types(
    body: &FunctionBody<'_>,
) -> Option<Vec<String>> {
    infer_vue3_return_statement_list_runtime_types(&body.statements)
}

pub(crate) fn infer_vue3_return_statement_list_runtime_types(
    statements: &[Statement<'_>],
) -> Option<Vec<String>> {
    let [statement] = statements else {
        return None;
    };
    infer_vue3_return_statement_runtime_types(statement)
}

pub(crate) fn infer_vue3_return_statement_runtime_types(
    statement: &Statement<'_>,
) -> Option<Vec<String>> {
    match statement {
        Statement::ReturnStatement(statement) => statement
            .argument
            .as_ref()
            .and_then(infer_vue3_return_expression_runtime_types),
        Statement::BlockStatement(block) => {
            infer_vue3_return_statement_list_runtime_types(&block.body)
        }
        Statement::IfStatement(statement) => {
            let alternate = statement.alternate.as_ref()?;
            let mut types = infer_vue3_return_statement_runtime_types(&statement.consequent)?;
            let alternate_types = infer_vue3_return_statement_runtime_types(alternate)?;
            merge_vue3_runtime_types(&mut types, alternate_types);
            vue3_non_empty_runtime_types(types)
        }
        _ => None,
    }
}

pub(crate) fn infer_vue3_return_expression_runtime_types(
    expression: &Expression<'_>,
) -> Option<Vec<String>> {
    match unwrap_vue3_ts_expression(expression) {
        Expression::StringLiteral(_) | Expression::TemplateLiteral(_) => {
            Some(vec!["String".into()])
        }
        Expression::NumericLiteral(_) => Some(vec!["Number".into()]),
        Expression::BooleanLiteral(_) => Some(vec!["Boolean".into()]),
        Expression::NullLiteral(_) => Some(vec!["null".into()]),
        Expression::ArrayExpression(_) => Some(vec!["Array".into()]),
        Expression::ObjectExpression(_) => Some(vec!["Object".into()]),
        Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_) => {
            Some(vec!["Function".into()])
        }
        Expression::ConditionalExpression(expression) => {
            let mut types = infer_vue3_return_expression_runtime_types(&expression.consequent)?;
            let alternate_types =
                infer_vue3_return_expression_runtime_types(&expression.alternate)?;
            merge_vue3_runtime_types(&mut types, alternate_types);
            vue3_non_empty_runtime_types(types)
        }
        Expression::NewExpression(expression) => {
            let name = vue3_new_expression_runtime_constructor_name(&expression.callee)?;
            Some(vec![name.to_string()])
        }
        _ => None,
    }
}

pub(crate) fn vue3_new_expression_runtime_constructor_name(
    expression: &Expression<'_>,
) -> Option<&'static str> {
    let name = match unwrap_vue3_ts_expression(expression) {
        Expression::Identifier(identifier) => identifier.name.as_str(),
        Expression::StaticMemberExpression(member) => member.property.name.as_str(),
        _ => return None,
    };
    vue3_return_expression_constructor_runtime_name(name)
}

pub(crate) fn vue3_return_expression_constructor_runtime_name(name: &str) -> Option<&'static str> {
    match name {
        "String" => Some("String"),
        "Number" => Some("Number"),
        "Boolean" => Some("Boolean"),
        "Array" => Some("Array"),
        "Object" => Some("Object"),
        "Function" => Some("Function"),
        "Date" => Some("Date"),
        "Error" => Some("Error"),
        "Map" => Some("Map"),
        "Set" => Some("Set"),
        "WeakMap" => Some("WeakMap"),
        "WeakSet" => Some("WeakSet"),
        "Promise" => Some("Promise"),
        _ => None,
    }
}

pub(crate) fn register_vue3_function_return_projection(
    source: &str,
    name: &str,
    function: &Function<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    if let Some(return_type) = function.return_type.as_ref() {
        register_vue3_declared_return_props_options(
            source,
            name,
            &return_type.type_annotation,
            analysis,
        );
        return;
    }
    if let Some(types) = infer_vue3_function_runtime_return_types(function) {
        register_vue3_declared_return_runtime_types(name, types, analysis);
    }
}

pub(crate) fn register_vue3_function_value_expression_return_projection(
    source: &str,
    name: &str,
    expression: &Expression<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    match unwrap_vue3_ts_expression(expression) {
        Expression::ArrowFunctionExpression(function) => {
            if let Some(return_type) = function.return_type.as_ref() {
                register_vue3_declared_return_props_options(
                    source,
                    name,
                    &return_type.type_annotation,
                    analysis,
                );
                return;
            }
            if let Some(types) = infer_vue3_arrow_function_runtime_return_types(function) {
                register_vue3_declared_return_runtime_types(name, types, analysis);
            }
        }
        Expression::FunctionExpression(function) => {
            register_vue3_function_return_projection(source, name, function, analysis);
        }
        _ => {}
    }
}

pub(crate) fn register_vue3_function_value_return_props_options(
    source: &str,
    declaration: &VariableDeclaration<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    for declarator in &declaration.declarations {
        let Some(name) = first_pattern_binding(&declarator.id) else {
            continue;
        };
        if let Some(return_type) = vue3_variable_declarator_function_return_type(declarator) {
            register_vue3_declared_return_props_options(source, &name, return_type, analysis);
            continue;
        }
        if let Some(init) = declarator.init.as_ref() {
            register_vue3_function_value_expression_return_projection(
                source, &name, init, analysis,
            );
        }
    }
}

pub(crate) fn register_vue3_static_runtime_props_options(
    source: &str,
    declaration: &VariableDeclaration<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    for declarator in &declaration.declarations {
        let Some(name) = first_pattern_binding(&declarator.id) else {
            continue;
        };
        let Some(init) = declarator.init.as_ref() else {
            continue;
        };
        let Some(props_options) =
            vue3_static_runtime_props_options_type_members(source, init, analysis)
        else {
            continue;
        };
        register_vue3_local_type_name(analysis, &name);
        analysis
            .props_options_type_declarations
            .insert(name.to_string(), props_options);
    }
    for declarator in &declaration.declarations {
        if !vue3_variable_declarator_has_type_projection(declarator) {
            continue;
        }
        if let Some(name) = first_pattern_binding_name(&declarator.id) {
            capture_vue3_value_type_projection(analysis, name);
        }
    }
}

pub(crate) fn register_vue3_default_static_runtime_props_options(
    source: &str,
    declaration: &ExportDefaultDeclarationKind<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    let Some(expression) = declaration.as_expression() else {
        return;
    };
    let Some(props_options) =
        vue3_static_runtime_props_options_type_members(source, expression, analysis)
    else {
        return;
    };
    let deps = collect_vue3_static_runtime_props_options_deps(expression, analysis);
    register_vue3_local_type_name(analysis, "default");
    analysis
        .props_options_type_declarations
        .insert("default".into(), props_options);
    insert_vue3_declared_type_deps(analysis, "default", deps);
}

pub(crate) fn collect_vue3_static_runtime_props_options_deps(
    expression: &Expression<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> BTreeSet<String> {
    let mut deps = BTreeSet::new();
    collect_vue3_static_runtime_props_options_deps_into(expression, analysis, &mut deps);
    deps
}

pub(crate) fn collect_vue3_static_runtime_props_options_deps_into(
    expression: &Expression<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    deps: &mut BTreeSet<String>,
) {
    match expression {
        Expression::TSAsExpression(expression) => {
            collect_vue3_type_argument_deps_into(&expression.type_annotation, analysis, deps);
            collect_vue3_static_runtime_props_options_deps_into(
                &expression.expression,
                analysis,
                deps,
            );
        }
        Expression::TSTypeAssertion(expression) => {
            collect_vue3_type_argument_deps_into(&expression.type_annotation, analysis, deps);
            collect_vue3_static_runtime_props_options_deps_into(
                &expression.expression,
                analysis,
                deps,
            );
        }
        Expression::TSSatisfiesExpression(expression) => {
            collect_vue3_type_argument_deps_into(&expression.type_annotation, analysis, deps);
            collect_vue3_static_runtime_props_options_deps_into(
                &expression.expression,
                analysis,
                deps,
            );
        }
        Expression::TSInstantiationExpression(expression) => {
            for ty in &expression.type_arguments.params {
                collect_vue3_type_argument_deps_into(ty, analysis, deps);
            }
            collect_vue3_static_runtime_props_options_deps_into(
                &expression.expression,
                analysis,
                deps,
            );
        }
        Expression::TSNonNullExpression(expression) => {
            collect_vue3_static_runtime_props_options_deps_into(
                &expression.expression,
                analysis,
                deps,
            );
        }
        Expression::ParenthesizedExpression(expression) => {
            collect_vue3_static_runtime_props_options_deps_into(
                &expression.expression,
                analysis,
                deps,
            );
        }
        Expression::ArrayExpression(array) => {
            for element in &array.elements {
                let Some(expression) = element.as_expression() else {
                    continue;
                };
                collect_vue3_static_runtime_props_options_deps_into(expression, analysis, deps);
            }
        }
        Expression::ObjectExpression(object) => {
            for property in &object.properties {
                let ObjectPropertyKind::ObjectProperty(property) = property else {
                    continue;
                };
                collect_vue3_static_runtime_props_options_deps_into(
                    &property.value,
                    analysis,
                    deps,
                );
            }
        }
        _ => {}
    }
}

pub(crate) fn register_vue3_declared_variable_props_options(
    source: &str,
    declaration: &VariableDeclaration<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    for declarator in &declaration.declarations {
        let Some(name) = first_pattern_binding(&declarator.id) else {
            continue;
        };
        let Some(type_annotation) = declarator.type_annotation.as_ref() else {
            continue;
        };
        let ty = &type_annotation.type_annotation;
        let projection = Vue3ValueTypeProjection {
            type_query_declared_types: Some(infer_vue3_runtime_type(ty, analysis)),
            define_model_type_query_declared_types: Some(
                infer_vue3_define_model_runtime_type(ty, analysis),
            ),
            keyof_type_query_declared_types: infer_vue3_keyof_runtime_type(ty, analysis),
            return_type_runtime_type_declarations: infer_vue3_return_runtime_type(
                ty,
                analysis,
                Vue3ArrayElementRuntimeMode::Props,
            ),
            define_model_return_type_runtime_type_declarations: infer_vue3_return_runtime_type(
                ty,
                analysis,
                Vue3ArrayElementRuntimeMode::DefineModel,
            ),
            props_options_type_declarations: matches!(ty, TSType::TSTypeLiteral(_))
                .then(|| vue3_props_options_type_members(source, ty, analysis))
                .flatten(),
            return_type_props_options_declarations: match ty {
                TSType::TSFunctionType(function) => vue3_props_options_type_members(
                    source,
                    &function.return_type.type_annotation,
                    analysis,
                ),
                _ => None,
            },
            unresolved_import_sources: None,
        };
        register_vue3_local_type_name(analysis, &name);
        projection.apply_to_analysis(analysis, &name);
        analysis.value_type_projections.insert(name, projection);
    }
}

pub(crate) fn register_vue3_declared_return_props_options(
    source: &str,
    name: &str,
    ty: &TSType<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    let projection = Vue3ValueTypeProjection {
        return_type_runtime_type_declarations: vue3_non_empty_runtime_types(
            infer_vue3_runtime_type(ty, analysis),
        ),
        define_model_return_type_runtime_type_declarations: vue3_non_empty_runtime_types(
            infer_vue3_define_model_runtime_type(ty, analysis),
        ),
        return_type_props_options_declarations: vue3_props_options_type_members(
            source, ty, analysis,
        ),
        ..Vue3ValueTypeProjection::default()
    };
    register_vue3_local_type_name(analysis, name);
    projection.apply_to_analysis(analysis, name);
    analysis
        .value_type_projections
        .insert(name.to_string(), projection);
}

pub(crate) fn register_vue3_declared_return_runtime_types(
    name: &str,
    types: Vec<String>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    register_vue3_local_type_name(analysis, name);
    let Some(types) = vue3_non_empty_runtime_types(types) else {
        return;
    };
    analysis
        .return_type_runtime_type_declarations
        .insert(name.to_string(), types.clone());
    analysis
        .define_model_return_type_runtime_type_declarations
        .insert(name.to_string(), types);
}

pub(crate) fn infer_vue3_enum_runtime_type(declaration: &TSEnumDeclaration<'_>) -> Vec<String> {
    let mut types = Vec::new();
    for member in &declaration.body.members {
        match member.initializer.as_ref() {
            Some(Expression::StringLiteral(_)) => push_unique(&mut types, "String"),
            Some(Expression::NumericLiteral(_)) => push_unique(&mut types, "Number"),
            _ => {}
        }
    }
    if types.is_empty() {
        vec!["Number".into()]
    } else {
        types
    }
}

pub(crate) fn register_vue3_local_type_name(analysis: &mut Vue3ScriptSetupAnalysis, name: &str) {
    analysis.type_sources.remove(name);
    analysis.type_direct_deps.remove(name);
    analysis.type_deps.remove(name);
    analysis.unresolved_import_sources.remove(name);
    analysis.silent_unresolved_type_names.remove(name);
    analysis.type_query_declared_types.remove(name);
    analysis.define_model_type_query_declared_types.remove(name);
    analysis.keyof_type_query_declared_types.remove(name);
    analysis.props_options_type_declarations.remove(name);
    analysis.return_type_runtime_type_declarations.remove(name);
    analysis
        .define_model_return_type_runtime_type_declarations
        .remove(name);
    analysis.return_type_props_options_declarations.remove(name);
    analysis.generic_type_aliases.remove(name);
    analysis.string_literal_type_declarations.remove(name);
    analysis
        .ordered_string_literal_type_declarations
        .remove(name);
    analysis.local_ts_enum_type_names.remove(name);
}
