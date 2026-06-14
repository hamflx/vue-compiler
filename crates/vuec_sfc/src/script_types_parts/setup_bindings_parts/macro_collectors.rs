pub(crate) fn collect_define_props_call(
    source: &str,
    call: &oxc_ast::ast::CallExpression<'_>,
    binding: Option<&BindingPattern<'_>>,
    analysis: &mut Vue27ScriptSetupAnalysis,
    is_prod: bool,
) {
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
        collect_define_props_type(source, type_argument, binding, None, analysis, is_prod);
        return;
    }
    if let Some(argument) = call.arguments.first() {
        let expression = argument.to_expression();
        check_vue27_invalid_scope_reference(expression, "defineProps", analysis);
        if let Expression::ObjectExpression(object) = expression {
            for key in object_expression_keys(object) {
                push_unique(&mut analysis.props_bindings, &key);
            }
        }
        let start = expression.span().start as usize;
        let end = expression.span().end as usize;
        analysis.props_runtime = source.get(start..end).map(ToOwned::to_owned);
    }
}

pub(crate) fn collect_with_defaults_call(
    source: &str,
    call: &oxc_ast::ast::CallExpression<'_>,
    binding: Option<&BindingPattern<'_>>,
    analysis: &mut Vue27ScriptSetupAnalysis,
    is_prod: bool,
) -> bool {
    let Some(define_props_call) =
        call.arguments
            .first()
            .and_then(|argument| match argument.to_expression() {
                Expression::CallExpression(call) if is_call_named(call, "defineProps") => {
                    Some(call)
                }
                _ => None,
            })
    else {
        return false;
    };
    let Some(type_argument) = define_props_call
        .type_arguments
        .as_ref()
        .and_then(|arguments| arguments.params.first())
    else {
        collect_define_props_call(source, define_props_call, binding, analysis, is_prod);
        return true;
    };
    let defaults = call.arguments.get(1).map(|argument| {
        check_vue27_invalid_scope_reference(argument.to_expression(), "defineProps", analysis);
        vue27_runtime_defaults_from_argument(source, argument)
    });
    collect_define_props_type(
        source,
        type_argument,
        binding,
        defaults.flatten(),
        analysis,
        is_prod,
    );
    true
}

pub(crate) fn collect_define_emits_call(
    source: &str,
    call: &oxc_ast::ast::CallExpression<'_>,
    binding: Option<&str>,
    analysis: &mut Vue27ScriptSetupAnalysis,
) {
    if analysis.emit_binding.is_none() {
        if let Some(binding) = binding {
            analysis.emit_binding = Some(binding.to_string());
        }
    }
    if let Some(type_argument) = call
        .type_arguments
        .as_ref()
        .and_then(|arguments| arguments.params.first())
    {
        if !call.arguments.is_empty() {
            analysis
                .errors
                .push(vue27_macro_type_and_runtime_error("defineEmits"));
        }
        collect_define_emits_type(source, type_argument, analysis);
        return;
    }
    let Some(argument) = call.arguments.first() else {
        return;
    };
    let expression = argument.to_expression();
    check_vue27_invalid_scope_reference(expression, "defineEmits", analysis);
    let start = expression.span().start as usize;
    let end = expression.span().end as usize;
    analysis.emits_runtime = source.get(start..end).map(ToOwned::to_owned);
}

pub(crate) fn collect_define_props_type(
    source: &str,
    type_argument: &TSType<'_>,
    binding: Option<&BindingPattern<'_>>,
    defaults: Option<Vue27RuntimeDefaults>,
    analysis: &mut Vue27ScriptSetupAnalysis,
    is_prod: bool,
) {
    let Some(type_members) = vue27_resolve_props_type(source, type_argument, analysis) else {
        return;
    };
    let default_map = defaults
        .as_ref()
        .and_then(|defaults| defaults.static_defaults.as_ref());
    let mut props = Vec::new();
    for member in &type_members.members {
        let mut prop = member.clone();
        if let Some(default) = default_map.and_then(|defaults| defaults.get(&prop.key)) {
            prop.default = Some(default.clone());
        }
        push_unique(&mut analysis.props_bindings, &prop.key);
        props.push(prop);
    }
    analysis.props_runtime_defaults = defaults;
    analysis.needs_merge_defaults = analysis
        .props_runtime_defaults
        .as_ref()
        .is_some_and(|defaults| defaults.static_defaults.is_none());
    analysis.props_type_runtime = true;
    analysis.props_type_source = Some(vue27_setup_props_type_source(
        source,
        type_argument,
        &type_members,
        analysis.props_runtime_defaults.as_ref(),
    ));
    analysis.props_runtime = Some(gen_vue27_runtime_props(
        &props,
        analysis.props_runtime_defaults.as_ref(),
        is_prod,
    ));
    if let Some(binding) = binding {
        analysis
            .setup_prelude
            .push_str(&vue27_props_type_assignment(
                source,
                binding,
                analysis.props_type_source.as_deref(),
            ));
    }
}

pub(crate) fn collect_define_emits_type(
    source: &str,
    type_argument: &TSType<'_>,
    analysis: &mut Vue27ScriptSetupAnalysis,
) {
    if !vue27_emits_type_argument_is_supported(type_argument, analysis) {
        analysis.errors.push(
            "type argument passed to defineEmits() must be a function type, a literal type with call signatures, or a reference to the above types."
                .to_string(),
        );
        return;
    }
    let Some(emits_type) = vue27_resolve_emits_type(source, type_argument, analysis) else {
        return;
    };
    if !emits_type.events.is_empty() {
        analysis.emits_runtime = Some(format!(
            "[{}]",
            emits_type
                .events
                .iter()
                .map(|name| format!("\"{}\"", escape_js_double(name)))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    analysis.emit_type_source = Some(emits_type.source);
}

pub(crate) fn vue27_script_setup_module_export_error() -> String {
    "<script setup> cannot contain ES module exports. If you are using a previous version of <script setup>, please consult the updated RFC at https://github.com/vuejs/rfcs/pull/227.".to_string()
}

pub(crate) fn vue27_macro_type_and_runtime_error(macro_name: &str) -> String {
    format!(
        "{macro_name}() cannot accept both type and non-type arguments at the same time. Use one or the other."
    )
}

pub(crate) fn check_vue27_invalid_scope_reference(
    expression: &Expression<'_>,
    macro_name: &str,
    analysis: &mut Vue27ScriptSetupAnalysis,
) {
    if vue27_expression_references_setup_local(expression, &analysis.local_setup_bindings) {
        analysis.errors.push(format!(
            "`{macro_name}()` in <script setup> cannot reference locally declared variables because it will be hoisted outside of the setup() function. If your component options require initialization in the module scope, use a separate normal <script> to export the options instead."
        ));
    }
}
