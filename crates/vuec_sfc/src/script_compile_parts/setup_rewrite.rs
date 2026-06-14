pub(crate) fn analyze_vue3_setup_variable_declaration(
    source: &str,
    declaration: &VariableDeclaration<'_>,
    edits: &mut SourceEdits<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
    props_destructure: SfcPropsDestructureMode,
    is_prod: bool,
    custom_element: bool,
    literal_const_enabled: bool,
) {
    let mut macro_declarators = Vec::new();
    let is_all_static = vue3_variable_declaration_is_static_hoist(declaration);
    for (index, declarator) in declaration.declarations.iter().enumerate() {
        if let Some(Expression::CallExpression(call)) =
            declarator.init.as_ref().map(unwrap_vue3_ts_expression)
        {
            if is_call_named(call, "defineProps") {
                if matches!(declarator.id, BindingPattern::BindingIdentifier(_)) {
                    collect_vue3_define_props_call(source, call, analysis, is_prod, custom_element);
                    collect_pattern_bindings(&declarator.id, &mut analysis.return_bindings);
                    collect_pattern_binding_types(
                        &declarator.id,
                        "setup-reactive-const",
                        &mut analysis.setup_bindings,
                    );
                    edits.overwrite(call.span.start as usize, call.span.end as usize, "__props");
                } else {
                    match props_destructure {
                        SfcPropsDestructureMode::Enabled => {
                            let props_rest_id = collect_vue3_define_props_destructure_bindings(
                                source,
                                &declarator.id,
                                analysis,
                            );
                            collect_vue3_define_props_call(
                                source,
                                call,
                                analysis,
                                is_prod,
                                custom_element,
                            );
                            if let Some(rest_id) = props_rest_id {
                                rewrite_vue3_define_props_destructure_rest(
                                    &declarator.id,
                                    call,
                                    &rest_id,
                                    analysis,
                                    edits,
                                );
                            } else {
                                macro_declarators.push(index);
                            }
                        }
                        SfcPropsDestructureMode::Disabled => {
                            collect_vue3_define_props_call(
                                source,
                                call,
                                analysis,
                                is_prod,
                                custom_element,
                            );
                            collect_pattern_bindings(&declarator.id, &mut analysis.return_bindings);
                            collect_vue3_script_pattern_binding_types(
                                &declarator.id,
                                declaration.kind == VariableDeclarationKind::Const,
                                true,
                                &mut analysis.setup_bindings,
                            );
                            edits.overwrite(
                                call.span.start as usize,
                                call.span.end as usize,
                                "__props",
                            );
                        }
                        SfcPropsDestructureMode::Error => {
                            collect_vue3_define_props_call(
                                source,
                                call,
                                analysis,
                                is_prod,
                                custom_element,
                            );
                            analysis.errors.push(
                                "Props destructure is explicitly prohibited via config.".into(),
                            );
                            collect_pattern_bindings(&declarator.id, &mut analysis.return_bindings);
                            collect_vue3_script_pattern_binding_types(
                                &declarator.id,
                                declaration.kind == VariableDeclarationKind::Const,
                                true,
                                &mut analysis.setup_bindings,
                            );
                            edits.overwrite(
                                call.span.start as usize,
                                call.span.end as usize,
                                "__props",
                            );
                        }
                    }
                }
                continue;
            }
            if is_call_named(call, "withDefaults")
                && collect_vue3_with_defaults_call(source, call, analysis, is_prod, custom_element)
            {
                if matches!(declarator.id, BindingPattern::ObjectPattern(_)) {
                    analysis.warnings.push(
                        "withDefaults() is unnecessary when using destructure with defineProps().\nReactive destructure will be disabled when using withDefaults().\nPrefer using destructure default values, e.g. const { foo = 1 } = defineProps(...)."
                            .into(),
                    );
                }
                collect_pattern_bindings(&declarator.id, &mut analysis.return_bindings);
                collect_pattern_binding_types(
                    &declarator.id,
                    "setup-const",
                    &mut analysis.setup_bindings,
                );
                edits.overwrite(call.span.start as usize, call.span.end as usize, "__props");
                continue;
            }
            if is_call_named(call, "defineEmits") {
                let emit_binding =
                    first_pattern_binding(&declarator.id).unwrap_or_else(|| "emit".into());
                collect_vue3_define_emits_call(source, call, Some(&emit_binding), analysis);
                collect_pattern_bindings(&declarator.id, &mut analysis.return_bindings);
                collect_pattern_binding_types(
                    &declarator.id,
                    "setup-const",
                    &mut analysis.setup_bindings,
                );
                edits.overwrite(call.span.start as usize, call.span.end as usize, "__emit");
                continue;
            }
            if is_call_named(call, "defineOptions") {
                collect_vue3_define_options_call(source, call, analysis);
                analysis
                    .errors
                    .push("defineOptions() has no returning value, it cannot be assigned.".into());
                continue;
            }
            if is_call_named(call, "defineSlots") {
                collect_vue3_define_slots_call(call, Some(&declarator.id), edits, analysis);
                collect_pattern_bindings(&declarator.id, &mut analysis.return_bindings);
                collect_pattern_binding_types(
                    &declarator.id,
                    "setup-const",
                    &mut analysis.setup_bindings,
                );
                continue;
            }
            if is_call_named(call, "defineModel") {
                collect_vue3_define_model_call(source, call, Some(&declarator.id), edits, analysis);
                collect_pattern_bindings(&declarator.id, &mut analysis.return_bindings);
                collect_pattern_binding_types(
                    &declarator.id,
                    "setup-ref",
                    &mut analysis.setup_bindings,
                );
                continue;
            }
        }
        if matches!(declarator.id, BindingPattern::BindingIdentifier(_)) {
            let binding_type = vue3_setup_binding_type(
                declaration.kind,
                declarator.init.as_ref(),
                is_all_static,
                literal_const_enabled,
                &analysis.vue_import_aliases,
            );
            collect_pattern_binding_types(
                &declarator.id,
                binding_type,
                &mut analysis.setup_bindings,
            );
        } else {
            collect_vue3_script_pattern_binding_types(
                &declarator.id,
                declaration.kind == VariableDeclarationKind::Const,
                false,
                &mut analysis.setup_bindings,
            );
        }
        collect_pattern_bindings(&declarator.id, &mut analysis.return_bindings);
    }
    remove_vue27_macro_declarators(declaration, &macro_declarators, edits);
}

pub(crate) fn collect_vue3_define_options_call(
    source: &str,
    call: &oxc_ast::ast::CallExpression<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    if analysis.has_define_options {
        analysis
            .errors
            .push("duplicate defineOptions() call".into());
    }
    if call.type_arguments.is_some() {
        analysis
            .errors
            .push("defineOptions() cannot accept type arguments".into());
    }
    let Some(argument) = call.arguments.first() else {
        return;
    };
    analysis.has_define_options = true;
    let expression = unwrap_vue3_ts_expression(argument.to_expression());
    check_vue3_define_options_keys(expression, analysis);
    analysis.options_runtime = source
        .get(expression.span().start as usize..expression.span().end as usize)
        .map(str::trim)
        .map(ToOwned::to_owned);
}

pub(crate) fn unwrap_vue3_ts_expression<'a>(expression: &'a Expression<'a>) -> &'a Expression<'a> {
    match expression {
        Expression::TSAsExpression(expression) => unwrap_vue3_ts_expression(&expression.expression),
        Expression::TSSatisfiesExpression(expression) => {
            unwrap_vue3_ts_expression(&expression.expression)
        }
        Expression::TSTypeAssertion(expression) => {
            unwrap_vue3_ts_expression(&expression.expression)
        }
        Expression::TSNonNullExpression(expression) => {
            unwrap_vue3_ts_expression(&expression.expression)
        }
        Expression::TSInstantiationExpression(expression) => {
            unwrap_vue3_ts_expression(&expression.expression)
        }
        Expression::ParenthesizedExpression(expression) => {
            unwrap_vue3_ts_expression(&expression.expression)
        }
        _ => expression,
    }
}

pub(crate) fn check_vue3_define_options_keys(
    expression: &Expression<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    let Expression::ObjectExpression(object) = expression else {
        return;
    };
    for property in &object.properties {
        let key = match property {
            ObjectPropertyKind::ObjectProperty(property) if !property.computed => {
                match &property.key {
                    PropertyKey::StaticIdentifier(identifier) => Some(identifier.name.to_string()),
                    _ => None,
                }
            }
            _ => None,
        };
        let Some(key) = key else {
            continue;
        };
        let replacement = match key.as_str() {
            "props" => Some("defineProps"),
            "emits" => Some("defineEmits"),
            "expose" => Some("defineExpose"),
            "slots" => Some("defineSlots"),
            _ => None,
        };
        if let Some(replacement) = replacement {
            analysis.errors.push(format!(
                "defineOptions() cannot be used to declare {key}. Use {replacement}() instead."
            ));
        }
    }
}

pub(crate) fn collect_vue3_define_slots_call(
    call: &oxc_ast::ast::CallExpression<'_>,
    binding: Option<&BindingPattern<'_>>,
    edits: &mut SourceEdits<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    if analysis.has_define_slots {
        analysis.errors.push("duplicate defineSlots() call".into());
    }
    analysis.has_define_slots = true;
    if !call.arguments.is_empty() {
        analysis
            .errors
            .push("defineSlots() cannot accept arguments".into());
    }
    if binding.is_some() {
        analysis.needs_use_slots = true;
        edits.overwrite(
            call.span.start as usize,
            call.span.end as usize,
            "_useSlots()",
        );
    }
}

pub(crate) fn collect_vue3_define_expose_call(
    call: &oxc_ast::ast::CallExpression<'_>,
    edits: &mut SourceEdits<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    if analysis.has_define_expose {
        analysis.errors.push("duplicate defineExpose() call".into());
    }
    analysis.has_define_expose = true;
    edits.overwrite(
        call.span.start as usize,
        call.callee.span().end as usize,
        "__expose",
    );
}

pub(crate) fn collect_vue3_define_model_call(
    source: &str,
    call: &oxc_ast::ast::CallExpression<'_>,
    binding: Option<&BindingPattern<'_>>,
    edits: &mut SourceEdits<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    if let Some(type_argument) = call
        .type_arguments
        .as_ref()
        .and_then(|arguments| arguments.params.first())
    {
        record_vue3_type_argument_deps(type_argument, analysis);
    }
    check_vue3_define_model_scope_reference(call, analysis);
    let model = vue3_define_model_decl(source, call, analysis);
    if analysis
        .models
        .iter()
        .any(|existing| existing.name == model.name)
    {
        analysis
            .errors
            .push(format!("duplicate model name \"{}\"", model.name));
    }
    push_unique(&mut analysis.props_bindings, &model.name);
    if let Some(binding) = binding.and_then(first_pattern_binding) {
        analysis
            .setup_bindings
            .insert(binding, "setup-ref".to_string());
    }
    rewrite_vue3_define_model_call(call, edits);
    analysis.models.push(model);
}

pub(crate) fn check_vue3_define_model_scope_reference(
    call: &oxc_ast::ast::CallExpression<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    let first_expression = call
        .arguments
        .first()
        .map(|argument| unwrap_vue3_ts_expression(argument.to_expression()));
    let has_name = first_expression.and_then(vue3_define_model_name).is_some();
    let options = if has_name {
        call.arguments.get(1)
    } else {
        call.arguments.first()
    };
    let Some(options) = options else {
        return;
    };
    let expression = unwrap_vue3_ts_expression(options.to_expression());
    if vue3_define_model_prop_options_reference_non_literal_setup_local(expression, analysis) {
        analysis
            .errors
            .push(vue3_invalid_scope_reference_error("defineModel"));
    }
}

pub(crate) fn vue3_define_model_prop_options_reference_non_literal_setup_local(
    expression: &Expression<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> bool {
    let Expression::ObjectExpression(object) = unwrap_vue3_ts_expression(expression) else {
        return false;
    };
    object.properties.iter().any(|property| match property {
        ObjectPropertyKind::ObjectProperty(property) => {
            if property.computed
                || matches!(property.key.static_name().as_deref(), Some("get" | "set"))
            {
                false
            } else {
                vue3_expression_references_non_literal_setup_local(&property.value, analysis)
            }
        }
        ObjectPropertyKind::SpreadProperty(_) => false,
    })
}

pub(crate) fn vue3_define_model_decl(
    source: &str,
    call: &oxc_ast::ast::CallExpression<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Vue3ModelDecl {
    let first_expression = call
        .arguments
        .first()
        .map(|argument| unwrap_vue3_ts_expression(argument.to_expression()));
    let (name, has_name) = first_expression
        .and_then(vue3_define_model_name)
        .map(|name| (name, true))
        .unwrap_or_else(|| ("modelValue".to_string(), false));
    let options = if has_name {
        call.arguments.get(1)
    } else {
        call.arguments.first()
    };
    let prop_runtime =
        options.and_then(|argument| vue3_define_model_prop_runtime(source, argument));
    let runtime_types = call
        .type_arguments
        .as_ref()
        .and_then(|arguments| arguments.params.first())
        .map(|type_argument| infer_vue3_define_model_runtime_type(type_argument, analysis));
    Vue3ModelDecl {
        name,
        prop_runtime,
        runtime_types,
    }
}

pub(crate) fn vue3_define_model_name(expression: &Expression<'_>) -> Option<String> {
    match expression {
        Expression::StringLiteral(literal) => Some(literal.value.to_string()),
        Expression::TemplateLiteral(literal)
            if literal.expressions.is_empty() && literal.quasis.len() == 1 =>
        {
            literal
                .quasis
                .first()
                .and_then(|quasi| quasi.value.cooked.as_ref())
                .map(|value| value.as_str().to_string())
        }
        _ => None,
    }
}

pub(crate) fn vue3_define_model_prop_runtime(
    source: &str,
    argument: &Argument<'_>,
) -> Option<String> {
    let expression = unwrap_vue3_ts_expression(argument.to_expression());
    let start = expression.span().start as usize;
    let end = expression.span().end as usize;
    let runtime = if let Some(split) = vue3_define_model_options_split(expression) {
        remove_source_ranges(source, start, end, &split.transformer_option_ranges)
            .or_else(|| source.get(start..end).map(ToOwned::to_owned))
    } else {
        source.get(start..end).map(ToOwned::to_owned)
    }?;
    let runtime = runtime.trim();
    if runtime.is_empty() {
        None
    } else {
        Some(runtime.to_string())
    }
}

pub(crate) fn rewrite_vue3_define_model_call(
    call: &oxc_ast::ast::CallExpression<'_>,
    edits: &mut SourceEdits<'_>,
) {
    let first_expression = call
        .arguments
        .first()
        .map(|argument| unwrap_vue3_ts_expression(argument.to_expression()));
    let has_name = first_expression.and_then(vue3_define_model_name).is_some();
    let options_index = if has_name { 1 } else { 0 };
    let options = call.arguments.get(options_index);
    let options_split = options.and_then(|argument| {
        vue3_define_model_options_split(unwrap_vue3_ts_expression(argument.to_expression()))
    });
    let options_removed = options_split
        .as_ref()
        .is_some_and(|split| split.remove_entire_call_options);
    if let Some(split) = options_split.as_ref() {
        if split.remove_entire_call_options {
            if has_name {
                if let (Some(previous), Some(options)) = (call.arguments.first(), options) {
                    edits.remove(
                        previous.to_expression().span().end as usize,
                        options.to_expression().span().end as usize,
                    );
                }
            } else if let Some(options) = options {
                let expression = options.to_expression();
                edits.remove(
                    expression.span().start as usize,
                    expression.span().end as usize,
                );
            }
        } else {
            for (start, end) in &split.prop_option_ranges {
                edits.remove(*start, *end);
            }
        }
    }
    edits.overwrite(
        call.callee.span().start as usize,
        call.callee.span().end as usize,
        "_useModel",
    );
    let Some(first_argument) = call.arguments.first() else {
        edits.prepend_right(call.span.end as usize - 1, r#"__props, "modelValue""#);
        return;
    };
    let first_start = first_argument.to_expression().span().start as usize;
    if has_name {
        edits.prepend_right(first_start, "__props, ");
        return;
    }
    let prefix = if options_removed {
        r#"__props, "modelValue""#
    } else {
        r#"__props, "modelValue", "#
    };
    edits.prepend_right(first_start, prefix);
}

pub(crate) fn vue3_define_model_options_split(
    expression: &Expression<'_>,
) -> Option<Vue3DefineModelOptionsSplit> {
    let Expression::ObjectExpression(object) = unwrap_vue3_ts_expression(expression) else {
        return None;
    };
    if object.properties.iter().any(|property| {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            return true;
        };
        property.computed
    }) {
        return None;
    }

    let mut split = Vue3DefineModelOptionsSplit::default();
    for (index, property) in object.properties.iter().enumerate() {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            return None;
        };
        let start = property.span.start as usize;
        let end = object
            .properties
            .get(index + 1)
            .map(|next| next.span().start as usize)
            .unwrap_or_else(|| (object.span.end as usize).saturating_sub(1));
        if matches!(property.key.static_name().as_deref(), Some("get" | "set")) {
            split.transformer_option_ranges.push((start, end));
        } else {
            split.prop_option_ranges.push((start, end));
        }
    }
    split.remove_entire_call_options = split.prop_option_ranges.len() == object.properties.len();
    Some(split)
}

pub(crate) fn remove_source_ranges(
    source: &str,
    start: usize,
    end: usize,
    ranges: &[(usize, usize)],
) -> Option<String> {
    let mut ranges = ranges.to_vec();
    ranges.sort_by_key(|range| range.0);
    let mut cursor = start;
    let mut output = String::new();
    for (range_start, range_end) in ranges {
        if range_start < cursor || range_end < range_start || range_end > end {
            return None;
        }
        output.push_str(source.get(cursor..range_start)?);
        cursor = range_end;
    }
    output.push_str(source.get(cursor..end)?);
    Some(output)
}

pub(crate) fn collect_vue3_define_props_call(
    source: &str,
    call: &oxc_ast::ast::CallExpression<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
    is_prod: bool,
    custom_element: bool,
) {
    collect_vue3_define_props_call_seen(analysis);
    if let Some(type_argument) = call
        .type_arguments
        .as_ref()
        .and_then(|arguments| arguments.params.first())
    {
        if !call.arguments.is_empty() {
            analysis
                .errors
                .push(vue27_macro_type_and_runtime_error("defineProps"));
        }
        collect_vue3_define_props_type(
            source,
            type_argument,
            None,
            analysis,
            is_prod,
            custom_element,
        );
        return;
    }
    let Some(argument) = call.arguments.first() else {
        return;
    };
    let expression = argument.to_expression();
    check_vue3_invalid_non_literal_scope_reference(expression, "defineProps", analysis);
    for key in vue3_runtime_prop_keys(expression) {
        push_unique(&mut analysis.props_bindings, &key);
    }
    let Some(runtime) = source
        .get(expression.span().start as usize..expression.span().end as usize)
        .map(ToOwned::to_owned)
    else {
        return;
    };
    analysis.props_runtime =
        if let Some(defaults) = vue3_props_destructured_runtime_defaults(analysis) {
            analysis.needs_merge_defaults = true;
            Some(format!(
                "/*@__PURE__*/_mergeDefaults({}, {})",
                runtime.trim(),
                defaults
            ))
        } else {
            Some(runtime)
        };
}

pub(crate) fn collect_vue3_define_props_call_seen(analysis: &mut Vue3ScriptSetupAnalysis) {
    if analysis.has_define_props {
        analysis.errors.push("duplicate defineProps() call".into());
    }
    analysis.has_define_props = true;
}

pub(crate) fn collect_vue3_with_defaults_call(
    source: &str,
    call: &oxc_ast::ast::CallExpression<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
    is_prod: bool,
    custom_element: bool,
) -> bool {
    let Some(define_props_call) = call.arguments.first().and_then(|argument| {
        match unwrap_vue3_ts_expression(argument.to_expression()) {
            Expression::CallExpression(call) if is_call_named(call, "defineProps") => Some(call),
            _ => None,
        }
    }) else {
        analysis
            .errors
            .push("withDefaults' first argument must be a defineProps call.".to_string());
        return true;
    };
    let Some(type_argument) = define_props_call
        .type_arguments
        .as_ref()
        .and_then(|arguments| arguments.params.first())
    else {
        collect_vue3_define_props_call(
            source,
            define_props_call,
            analysis,
            is_prod,
            custom_element,
        );
        analysis.errors.push(
            "withDefaults can only be used with type-based defineProps declaration.".to_string(),
        );
        return true;
    };
    collect_vue3_define_props_call_seen(analysis);
    if !define_props_call.arguments.is_empty() {
        analysis
            .errors
            .push(vue27_macro_type_and_runtime_error("defineProps"));
        analysis.errors.push(
            "withDefaults can only be used with type-based defineProps declaration.".to_string(),
        );
    }
    if call.arguments.get(1).is_none() {
        analysis
            .errors
            .push("The 2nd argument of withDefaults is required.".to_string());
    }
    let defaults = call
        .arguments
        .get(1)
        .and_then(|argument| {
            if vue3_expression_references_non_literal_setup_local(
                argument.to_expression(),
                analysis,
            ) {
                analysis.errors.push(
                    "`defineProps()` in <script setup> cannot reference locally declared variables because it will be hoisted outside of the setup() function. If your component options require initialization in the module scope, use a separate normal <script> to export the options instead."
                        .to_string(),
                );
            }
            vue3_runtime_defaults_from_argument(source, argument)
        });
    collect_vue3_define_props_type(
        source,
        type_argument,
        defaults,
        analysis,
        is_prod,
        custom_element,
    );
    true
}

pub(crate) fn collect_vue3_define_props_type(
    source: &str,
    type_argument: &TSType<'_>,
    defaults: Option<Vue27RuntimeDefaults>,
    analysis: &mut Vue3ScriptSetupAnalysis,
    is_prod: bool,
    custom_element: bool,
) {
    record_vue3_type_argument_deps(type_argument, analysis);
    let Some(type_members) = vue3_resolve_props_type_with_mode(
        source,
        type_argument,
        analysis,
        Vue3PropsTypeResolveMode::Consumed,
    ) else {
        return;
    };
    analysis.errors.extend(type_members.errors.clone());
    let default_map = defaults
        .as_ref()
        .and_then(|defaults| defaults.static_defaults.as_ref());
    let has_static_defaults = default_map.is_some();
    let dynamic_defaults = defaults
        .as_ref()
        .filter(|defaults| defaults.static_defaults.is_none());
    let mut props = Vec::new();
    for member in &type_members.members {
        let mut prop = member.clone();
        if let Some(default) =
            vue3_props_destructured_default_option(analysis, &prop.key, Some(prop.types.as_slice()))
        {
            prop.default = Some(default);
        } else if let Some(default) = default_map.and_then(|defaults| defaults.get(&prop.key)) {
            prop.default = Some(default.clone());
        }
        analysis
            .props_type_runtime_types
            .insert(prop.key.clone(), prop.types.clone());
        push_unique(&mut analysis.props_bindings, &prop.key);
        props.push(prop);
    }
    analysis.props_type_runtime = true;
    let props_runtime =
        gen_vue3_runtime_props(&props, is_prod, has_static_defaults, custom_element);
    analysis.props_runtime = if let Some(defaults) = dynamic_defaults {
        analysis.needs_merge_defaults = true;
        Some(format!(
            "/*@__PURE__*/_mergeDefaults({props_runtime}, {})",
            defaults.source
        ))
    } else {
        Some(props_runtime)
    };
}

pub(crate) fn vue3_resolve_props_type<'a>(
    source: &str,
    type_argument: &'a TSType<'a>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vue27TypeMembers> {
    vue3_resolve_props_type_with_mode(
        source,
        type_argument,
        analysis,
        Vue3PropsTypeResolveMode::Silent,
    )
}

pub(crate) fn vue3_resolve_props_type_with_mode<'a>(
    source: &str,
    type_argument: &'a TSType<'a>,
    analysis: &Vue3ScriptSetupAnalysis,
    mode: Vue3PropsTypeResolveMode,
) -> Option<Vue27TypeMembers> {
    match type_argument {
        TSType::TSTypeLiteral(literal) => {
            Some(vue3_type_members_from_literal(source, literal, analysis))
        }
        TSType::TSMappedType(mapped) => {
            vue3_type_members_from_mapped_type(source, mapped, analysis)
        }
        TSType::TSTypeReference(reference) => {
            let name = vue3_ts_type_name_key(&reference.type_name)?;
            match name.as_str() {
                "ExtractPropTypes" | "ExtractPublicPropTypes" => {
                    let ty = vue3_type_reference_first_type_argument(reference)?;
                    return vue3_resolve_extract_prop_types(source, ty, analysis);
                }
                "Partial" => {
                    let ty = vue3_type_reference_first_type_argument(reference)?;
                    return vue3_resolve_props_type_with_mode(source, ty, analysis, mode)
                        .map(vue3_type_members_optional);
                }
                "Required" => {
                    let ty = vue3_type_reference_first_type_argument(reference)?;
                    return vue3_resolve_props_type_with_mode(source, ty, analysis, mode)
                        .map(vue3_type_members_required);
                }
                "Readonly" => {
                    let ty = vue3_type_reference_first_type_argument(reference)?;
                    return vue3_resolve_props_type_with_mode(source, ty, analysis, mode);
                }
                "Record" => {
                    return vue3_type_members_from_record_type(source, reference, analysis);
                }
                "Pick" => {
                    let ty = vue3_type_reference_type_argument(reference, 0)?;
                    let keys = vue3_type_reference_type_argument(reference, 1)?;
                    let members = vue3_resolve_props_type_with_mode(source, ty, analysis, mode)?;
                    let keys = vue3_resolve_string_type_keys(keys, analysis)?;
                    return Some(vue3_type_members_pick(members, &keys));
                }
                "Omit" => {
                    let ty = vue3_type_reference_type_argument(reference, 0)?;
                    let keys = vue3_type_reference_type_argument(reference, 1)?;
                    let members = vue3_resolve_props_type_with_mode(source, ty, analysis, mode)?;
                    let keys = vue3_resolve_string_type_keys(keys, analysis)?;
                    return Some(vue3_type_members_omit(members, &keys));
                }
                _ => {}
            }
            if let Some(resolved) =
                vue3_resolve_generic_props_type_alias(source, reference, analysis)
            {
                return Some(resolved);
            }
            if let Some(members) = analysis.props_type_declarations.get(&name).cloned() {
                return Some(members);
            }
            if analysis.silent_unresolved_type_names.contains(&name) {
                return None;
            }
            if mode == Vue3PropsTypeResolveMode::Consumed {
                if let Some(import_source) = analysis.unresolved_import_sources.get(&name) {
                    return Some(vue3_type_members_empty(
                        source,
                        type_argument.span(),
                        vec![vue3_failed_import_source_error(import_source)],
                    ));
                }
                if !analysis.generic_type_parameter_names.contains(&name) {
                    return Some(vue3_type_members_empty(
                        source,
                        type_argument.span(),
                        vec![vue3_unresolvable_type_reference_error()],
                    ));
                }
            }
            None
        }
        TSType::TSUnionType(union) => {
            let (members, errors) = vue3_merge_props_type_members(
                union
                    .types
                    .iter()
                    .filter_map(|ty| vue3_resolve_props_type_with_mode(source, ty, analysis, mode)),
                false,
            );
            vue3_merged_type_members(source, union.span, members, errors)
        }
        TSType::TSIntersectionType(intersection) => {
            let (members, errors) = vue3_merge_props_type_members(
                intersection
                    .types
                    .iter()
                    .filter(|ty| {
                        !vue3_source_has_immediate_leading_vue_ignore_comment(
                            source,
                            ty.span().start as usize,
                        )
                    })
                    .filter_map(|ty| vue3_resolve_props_type_with_mode(source, ty, analysis, mode)),
                true,
            );
            vue3_merged_type_members(source, intersection.span, members, errors)
        }
        TSType::TSParenthesizedType(parenthesized) => vue3_resolve_props_type_with_mode(
            source,
            &parenthesized.type_annotation,
            analysis,
            mode,
        ),
        TSType::TSImportType(import_type) => {
            if import_type.source.value.as_str() == "vue"
                && import_type.qualifier.as_ref().is_some_and(|qualifier| {
                    matches!(
                        vue3_import_type_qualifier_key(qualifier).as_str(),
                        "ExtractPropTypes" | "ExtractPublicPropTypes"
                    )
                })
            {
                let ty = vue3_import_type_first_type_argument(import_type)?;
                return vue3_resolve_extract_prop_types(source, ty, analysis);
            }
            let Some(resolved) = vue3_resolve_import_type(import_type, analysis) else {
                return (mode == Vue3PropsTypeResolveMode::Consumed).then(|| {
                    vue3_type_members_empty(
                        source,
                        type_argument.span(),
                        vec![vue3_failed_import_source_error(
                            import_type.source.value.as_str(),
                        )],
                    )
                });
            };
            if let Some(members) = resolved
                .context
                .props_type_declarations
                .get(&resolved.name)
                .cloned()
            {
                return Some(members);
            }
            (mode == Vue3PropsTypeResolveMode::Consumed).then(|| {
                vue3_type_members_empty(
                    source,
                    type_argument.span(),
                    vec![vue3_unresolvable_type_reference_error()],
                )
            })
        }
        TSType::TSIndexedAccessType(indexed) => {
            vue3_resolve_indexed_access_props_type(source, indexed, analysis, mode)
        }
        _ => {
            if mode == Vue3PropsTypeResolveMode::Consumed {
                Some(vue3_type_members_empty(
                    source,
                    type_argument.span(),
                    vec![vue3_unresolvable_type_error(type_argument)],
                ))
            } else {
                None
            }
        }
    }
}
