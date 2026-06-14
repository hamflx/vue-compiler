pub(crate) fn vue3_props_destructured_runtime_defaults(
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<String> {
    if analysis.props_destructured_default_order.is_empty() {
        return None;
    }
    let mut entries = Vec::new();
    for key in &analysis.props_destructured_default_order {
        let Some(default) = analysis.props_destructured_defaults.get(key) else {
            continue;
        };
        let final_key = vue3_runtime_prop_key(key);
        let value = vue3_props_destructured_default_value(default, None);
        let skip = if vue3_props_destructured_default_needs_skip_factory(default, None) {
            format!(", __skip_{final_key}: true")
        } else {
            String::new()
        };
        entries.push(format!("{final_key}: {value}{skip}"));
    }
    if entries.is_empty() {
        None
    } else {
        Some(format!("{{\n  {}\n}}", entries.join(",\n  ")))
    }
}

pub(crate) fn vue3_props_destructured_default_option(
    analysis: &Vue3ScriptSetupAnalysis,
    key: &str,
    inferred_types: Option<&[String]>,
) -> Option<String> {
    let default = analysis.props_destructured_defaults.get(key)?;
    let value = vue3_props_destructured_default_value(default, inferred_types);
    let skip = if vue3_props_destructured_default_needs_skip_factory(default, inferred_types) {
        ", skipFactory: true"
    } else {
        ""
    };
    Some(format!("default: {value}{skip}"))
}

pub(crate) fn vue3_props_destructured_default_value(
    default: &Vue3PropsDestructuredDefault,
    inferred_types: Option<&[String]>,
) -> String {
    let need_skip_factory =
        vue3_props_destructured_default_needs_skip_factory(default, inferred_types);
    let is_function_prop =
        inferred_types.is_some_and(|types| types.iter().any(|ty| ty == "Function"));
    if !need_skip_factory && !default.is_literal && !is_function_prop {
        format!("() => ({})", default.value)
    } else {
        default.value.clone()
    }
}

pub(crate) fn vue3_props_destructured_default_needs_skip_factory(
    default: &Vue3PropsDestructuredDefault,
    inferred_types: Option<&[String]>,
) -> bool {
    inferred_types.is_none() && (default.is_function || default.is_identifier)
}

pub(crate) fn rewrite_vue3_define_props_destructure_rest(
    pattern: &BindingPattern<'_>,
    call: &oxc_ast::ast::CallExpression<'_>,
    rest_id: &str,
    analysis: &Vue3ScriptSetupAnalysis,
    edits: &mut SourceEdits<'_>,
) {
    let excluded = analysis
        .props_destructured_prop_order
        .iter()
        .map(|name| format!("\"{}\"", escape_js_double(name)))
        .collect::<Vec<_>>()
        .join(",");
    edits.overwrite(
        pattern.span().start as usize,
        pattern.span().end as usize,
        rest_id,
    );
    edits.overwrite(
        call.span.start as usize,
        call.span.end as usize,
        format!("_createPropsRestProxy(__props, [{excluded}])"),
    );
}

pub(crate) fn is_ascii_js_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first == '$' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch == '$' || ch.is_ascii_alphanumeric())
}

pub(crate) fn collect_vue3_define_props_destructure_bindings(
    source: &str,
    pattern: &BindingPattern<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) -> Option<String> {
    match pattern {
        BindingPattern::ObjectPattern(pattern) => {
            for property in &pattern.properties {
                let key =
                    vue3_define_props_destructure_key(&property.key, property.computed, analysis);
                collect_vue3_define_props_destructure_property(
                    source,
                    key.as_deref(),
                    &property.value,
                    analysis,
                );
            }
            if let Some(rest) = &pattern.rest {
                if let Some(rest_id) = first_pattern_binding(&rest.argument) {
                    analysis.props_destructured_rest_id = Some(rest_id.clone());
                    push_unique(&mut analysis.return_bindings, &rest_id);
                    collect_pattern_binding_types(
                        &rest.argument,
                        "setup-reactive-const",
                        &mut analysis.setup_bindings,
                    );
                    return Some(rest_id);
                }
                collect_pattern_binding_types(
                    &rest.argument,
                    "setup-reactive-const",
                    &mut analysis.setup_bindings,
                );
            }
            None
        }
        BindingPattern::ArrayPattern(pattern) => {
            for element in pattern.elements.iter().flatten() {
                collect_pattern_binding_types(
                    element,
                    "props-aliased",
                    &mut analysis.setup_bindings,
                );
            }
            if let Some(rest) = &pattern.rest {
                collect_pattern_binding_types(
                    &rest.argument,
                    "setup-reactive-const",
                    &mut analysis.setup_bindings,
                );
            }
            None
        }
        BindingPattern::AssignmentPattern(pattern) => {
            collect_vue3_define_props_destructure_bindings(source, &pattern.left, analysis)
        }
        BindingPattern::BindingIdentifier(_) => None,
    }
}

pub(crate) fn vue3_define_props_destructure_key(
    key: &PropertyKey<'_>,
    computed: bool,
    analysis: &mut Vue3ScriptSetupAnalysis,
) -> Option<String> {
    let key = match key {
        PropertyKey::StaticIdentifier(identifier) if !computed => Some(identifier.name.to_string()),
        PropertyKey::StringLiteral(literal) => Some(literal.value.to_string()),
        PropertyKey::NumericLiteral(literal) => Some(literal.value.to_string()),
        _ => None,
    };
    if key.is_none() {
        analysis
            .errors
            .push("defineProps() destructure cannot use computed key.".into());
    }
    key
}

pub(crate) fn collect_vue3_define_props_destructure_property(
    source: &str,
    key: Option<&str>,
    value: &BindingPattern<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    match value {
        BindingPattern::BindingIdentifier(identifier) => {
            register_vue3_define_props_destructure_binding(key, identifier.name.as_str(), analysis);
        }
        BindingPattern::AssignmentPattern(pattern) => {
            if vue3_expression_references_non_literal_setup_local(&pattern.right, analysis) {
                analysis.errors.push(
                    "`defineProps()` in <script setup> cannot reference locally declared variables because it will be hoisted outside of the setup() function. If your component options require initialization in the module scope, use a separate normal <script> to export the options instead."
                        .into(),
                );
            }
            if let Some(key) = key {
                if let Some(default) =
                    vue3_props_destructured_default_from_expression(source, &pattern.right)
                {
                    if !analysis
                        .props_destructured_default_order
                        .iter()
                        .any(|existing| existing == key)
                    {
                        analysis
                            .props_destructured_default_order
                            .push(key.to_string());
                    }
                    if let Some(value_type) = default.inferred_type.as_ref() {
                        analysis
                            .props_destructured_default_types
                            .insert(key.to_string(), value_type.clone());
                    }
                    analysis
                        .props_destructured_defaults
                        .insert(key.to_string(), default);
                }
            }
            if let BindingPattern::BindingIdentifier(identifier) = &pattern.left {
                register_vue3_define_props_destructure_binding(
                    key,
                    identifier.name.as_str(),
                    analysis,
                );
            } else {
                analysis
                    .errors
                    .push("defineProps() destructure does not support nested patterns.".into());
                if let Some(local) = first_pattern_binding(&pattern.left) {
                    register_vue3_define_props_destructure_binding(key, &local, analysis);
                }
            }
        }
        _ => {
            analysis
                .errors
                .push("defineProps() destructure does not support nested patterns.".into());
            if let Some(local) = first_pattern_binding(value) {
                register_vue3_define_props_destructure_binding(key, &local, analysis);
            }
        }
    }
}

pub(crate) fn vue3_props_destructured_default_from_expression(
    source: &str,
    expression: &Expression<'_>,
) -> Option<Vue3PropsDestructuredDefault> {
    let value = source
        .get(expression.span().start as usize..expression.span().end as usize)?
        .to_string();
    let unwrapped = unwrap_vue3_ts_expression(expression);
    Some(Vue3PropsDestructuredDefault {
        value,
        inferred_type: infer_vue3_define_props_destructure_default_value_type(expression)
            .map(ToOwned::to_owned),
        is_literal: vue3_props_destructured_default_is_literal(unwrapped),
        is_function: matches!(
            unwrapped,
            Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_)
        ),
        is_identifier: matches!(unwrapped, Expression::Identifier(_)),
    })
}

pub(crate) fn vue3_props_destructured_default_is_literal(expression: &Expression<'_>) -> bool {
    matches!(
        expression,
        Expression::StringLiteral(_)
            | Expression::NumericLiteral(_)
            | Expression::BooleanLiteral(_)
            | Expression::NullLiteral(_)
            | Expression::RegExpLiteral(_)
            | Expression::BigIntLiteral(_)
            | Expression::TemplateLiteral(_)
    )
}

pub(crate) fn register_vue3_define_props_destructure_binding(
    key: Option<&str>,
    local: &str,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    let public_key = key.unwrap_or(local);
    push_unique(&mut analysis.props_destructured_prop_order, public_key);
    analysis
        .props_destructured_bindings
        .insert(local.to_string(), public_key.to_string());
    if key.is_some_and(|key| key == local) {
        analysis
            .setup_bindings
            .insert(local.to_string(), "props".into());
    } else {
        analysis
            .setup_bindings
            .insert(local.to_string(), "props-aliased".into());
    }
}

pub(crate) fn check_vue3_define_props_destructure_default_types(
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    for (key, value_type) in &analysis.props_destructured_default_types {
        let Some(prop_types) = analysis.props_type_runtime_types.get(key) else {
            continue;
        };
        if prop_types.is_empty()
            || prop_types.iter().any(|ty| ty == "null")
            || prop_types.iter().any(|ty| ty == "Unknown")
            || prop_types.iter().any(|ty| ty == value_type)
        {
            continue;
        }
        analysis.errors.push(format!(
            "Default value of prop \"{key}\" does not match declared type."
        ));
    }
}

pub(crate) fn infer_vue3_define_props_destructure_default_value_type(
    expression: &Expression<'_>,
) -> Option<&'static str> {
    match unwrap_vue3_ts_expression(expression) {
        Expression::StringLiteral(_) => Some("String"),
        Expression::NumericLiteral(_) => Some("Number"),
        Expression::BooleanLiteral(_) => Some("Boolean"),
        Expression::ObjectExpression(_) => Some("Object"),
        Expression::ArrayExpression(_) => Some("Array"),
        Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_) => {
            Some("Function")
        }
        _ => None,
    }
}
