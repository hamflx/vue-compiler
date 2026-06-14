pub(crate) fn collect_vue27_argument_identifier_usage(argument: &Argument<'_>, value: &mut String) {
    match argument {
        Argument::SpreadElement(spread) => {
            collect_vue27_expression_identifier_usage(&spread.argument, value);
        }
        _ => collect_vue27_expression_identifier_usage(argument.to_expression(), value),
    }
}

pub(crate) fn collect_vue27_property_key_identifier_usage(
    key: &PropertyKey<'_>,
    value: &mut String,
) {
    match key {
        PropertyKey::StaticIdentifier(_) | PropertyKey::PrivateIdentifier(_) => {}
        _ => collect_vue27_expression_identifier_usage(key.to_expression(), value),
    }
}

pub(crate) fn collect_vue27_function_identifier_usage(function: &Function<'_>, value: &mut String) {
    for param in &function.params.items {
        if let Some(initializer) = &param.initializer {
            collect_vue27_expression_identifier_usage(initializer, value);
        }
    }
    if let Some(body) = &function.body {
        for statement in &body.statements {
            collect_vue27_statement_identifier_usage(statement, value);
        }
    }
}

pub(crate) fn collect_vue27_arrow_function_identifier_usage(
    function: &ArrowFunctionExpression<'_>,
    value: &mut String,
) {
    for param in &function.params.items {
        if let Some(initializer) = &param.initializer {
            collect_vue27_expression_identifier_usage(initializer, value);
        }
    }
    for statement in &function.body.statements {
        collect_vue27_statement_identifier_usage(statement, value);
    }
}

pub(crate) fn collect_vue27_assignment_target_identifier_usage(
    target: &AssignmentTarget<'_>,
    value: &mut String,
) {
    match target {
        AssignmentTarget::AssignmentTargetIdentifier(identifier) => {
            push_vue27_identifier_usage(value, identifier.name.as_str());
        }
        AssignmentTarget::StaticMemberExpression(member) => {
            collect_vue27_expression_identifier_usage(&member.object, value);
        }
        AssignmentTarget::ComputedMemberExpression(member) => {
            collect_vue27_expression_identifier_usage(&member.object, value);
            collect_vue27_expression_identifier_usage(&member.expression, value);
        }
        AssignmentTarget::PrivateFieldExpression(member) => {
            collect_vue27_expression_identifier_usage(&member.object, value);
        }
        AssignmentTarget::TSAsExpression(expression) => {
            collect_vue27_expression_identifier_usage(&expression.expression, value);
        }
        AssignmentTarget::TSSatisfiesExpression(expression) => {
            collect_vue27_expression_identifier_usage(&expression.expression, value);
        }
        AssignmentTarget::TSNonNullExpression(expression) => {
            collect_vue27_expression_identifier_usage(&expression.expression, value);
        }
        AssignmentTarget::TSTypeAssertion(expression) => {
            collect_vue27_expression_identifier_usage(&expression.expression, value);
        }
        AssignmentTarget::ArrayAssignmentTarget(target) => {
            for element in target.elements.iter().flatten() {
                collect_vue27_assignment_target_maybe_default_identifier_usage(element, value);
            }
            if let Some(rest) = &target.rest {
                collect_vue27_assignment_target_identifier_usage(&rest.target, value);
            }
        }
        AssignmentTarget::ObjectAssignmentTarget(target) => {
            for property in &target.properties {
                match property {
                    oxc_ast::ast::AssignmentTargetProperty::AssignmentTargetPropertyIdentifier(
                        property,
                    ) => {
                        push_vue27_identifier_usage(value, property.binding.name.as_str());
                        if let Some(init) = &property.init {
                            collect_vue27_expression_identifier_usage(init, value);
                        }
                    }
                    oxc_ast::ast::AssignmentTargetProperty::AssignmentTargetPropertyProperty(
                        property,
                    ) => {
                        if property.computed {
                            collect_vue27_property_key_identifier_usage(&property.name, value);
                        }
                        collect_vue27_assignment_target_maybe_default_identifier_usage(
                            &property.binding,
                            value,
                        );
                    }
                }
            }
            if let Some(rest) = &target.rest {
                collect_vue27_assignment_target_identifier_usage(&rest.target, value);
            }
        }
    }
}

pub(crate) fn collect_vue27_assignment_target_maybe_default_identifier_usage(
    target: &oxc_ast::ast::AssignmentTargetMaybeDefault<'_>,
    value: &mut String,
) {
    match target {
        oxc_ast::ast::AssignmentTargetMaybeDefault::AssignmentTargetWithDefault(target) => {
            collect_vue27_assignment_target_identifier_usage(&target.binding, value);
            collect_vue27_expression_identifier_usage(&target.init, value);
        }
        _ => {
            if let Some(target) = target.as_assignment_target() {
                collect_vue27_assignment_target_identifier_usage(target, value);
            }
        }
    }
}

pub(crate) fn collect_vue27_simple_assignment_target_identifier_usage(
    target: &SimpleAssignmentTarget<'_>,
    value: &mut String,
) {
    match target {
        SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) => {
            push_vue27_identifier_usage(value, identifier.name.as_str());
        }
        SimpleAssignmentTarget::StaticMemberExpression(member) => {
            collect_vue27_expression_identifier_usage(&member.object, value);
        }
        SimpleAssignmentTarget::ComputedMemberExpression(member) => {
            collect_vue27_expression_identifier_usage(&member.object, value);
            collect_vue27_expression_identifier_usage(&member.expression, value);
        }
        SimpleAssignmentTarget::PrivateFieldExpression(member) => {
            collect_vue27_expression_identifier_usage(&member.object, value);
        }
        SimpleAssignmentTarget::TSAsExpression(expression) => {
            collect_vue27_expression_identifier_usage(&expression.expression, value);
        }
        SimpleAssignmentTarget::TSSatisfiesExpression(expression) => {
            collect_vue27_expression_identifier_usage(&expression.expression, value);
        }
        SimpleAssignmentTarget::TSNonNullExpression(expression) => {
            collect_vue27_expression_identifier_usage(&expression.expression, value);
        }
        SimpleAssignmentTarget::TSTypeAssertion(expression) => {
            collect_vue27_expression_identifier_usage(&expression.expression, value);
        }
    }
}

pub(crate) fn push_vue27_identifier_usage(value: &mut String, name: &str) {
    value.push(',');
    value.push_str(name);
}
