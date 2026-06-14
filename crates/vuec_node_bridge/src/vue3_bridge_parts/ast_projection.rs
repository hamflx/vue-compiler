pub(crate) fn vue3_for_parse_result_value(
    source: &str,
    base_offset: usize,
    dir: &vuec_ast::Vue3Directive,
    options: &Vue3CompilerOptions,
) -> Value {
    let expression = dir
        .exp
        .as_ref()
        .map(Vue3Expression::source_string)
        .unwrap_or_default();
    let Some((aliases, iterable)) = split_v_for_expression(&expression) else {
        return Value::Null;
    };
    let source_loc = dir
        .exp_span
        .and_then(|span| {
            let local_start = span.start.0.saturating_sub(base_offset);
            let local_end = span.end.0.saturating_sub(base_offset).min(source.len());
            source
                .get(local_start..local_end)
                .and_then(|slice| slice.find(iterable).map(|offset| local_start + offset))
                .map(|start| vue3_source_loc_value(source, start, start + iterable.len()))
        })
        .unwrap_or_else(vue3_loc_stub_value);
    let parts = split_v_for_aliases(aliases);
    json!({
        "source": vue3_simple_expression_with_ast_value(iterable, false, source_loc, options, Vue3ExpressionAstMode::Expression),
        "value": parts.first().map(|value| {
            vue3_simple_expression_with_ast_value(value, false, vue3_loc_stub_value(), options, Vue3ExpressionAstMode::Params)
        }),
        "key": parts.get(1).map(|value| {
            vue3_simple_expression_with_ast_value(value, false, vue3_loc_stub_value(), options, Vue3ExpressionAstMode::Expression)
        }),
        "index": parts.get(2).map(|value| {
            vue3_simple_expression_with_ast_value(value, false, vue3_loc_stub_value(), options, Vue3ExpressionAstMode::Expression)
        }),
        "finalized": false,
    })
}

pub(crate) fn vue3_simple_expression_with_ast_value(
    source: &str,
    is_static: bool,
    loc: Value,
    options: &Vue3CompilerOptions,
    ast_mode: Vue3ExpressionAstMode,
) -> Value {
    let mut value = vue3_simple_expression_value(source, is_static, loc);
    if let Some(ast_value) = vue3_expression_ast_value(source, is_static, options, ast_mode) {
        value["ast"] = ast_value;
    }
    value
}

pub(crate) fn vue3_expression_source_type(options: &Vue3CompilerOptions) -> SourceType {
    if options.is_ts
        || options
            .expression_plugins
            .iter()
            .any(|plugin| plugin == "typescript")
    {
        SourceType::ts()
    } else {
        SourceType::mjs()
    }
}

pub(crate) fn expression_ast_value(expression: &Expression<'_>) -> Value {
    match expression {
        Expression::ArrayExpression(array) => json!({
            "type": "ArrayExpression",
            "elements": array.elements.iter().map(array_element_ast_value).collect::<Vec<_>>(),
        }),
        Expression::ArrowFunctionExpression(function) => json!({
            "type": "ArrowFunctionExpression",
            "params": formal_parameters_ast_values(&function.params),
            "body": function_body_ast_value(&function.body),
        }),
        Expression::AssignmentExpression(assignment) => json!({
            "type": "AssignmentExpression",
            "left": assignment_target_ast_value(&assignment.left),
            "right": expression_ast_value(&assignment.right),
        }),
        Expression::AwaitExpression(await_expression) => json!({
            "type": "AwaitExpression",
            "argument": expression_ast_value(&await_expression.argument),
        }),
        Expression::BinaryExpression(binary) => json!({
            "type": "BinaryExpression",
            "left": expression_ast_value(&binary.left),
            "right": expression_ast_value(&binary.right),
        }),
        Expression::CallExpression(call) => json!({
            "type": "CallExpression",
            "callee": expression_ast_value(&call.callee),
            "arguments": call.arguments.iter().map(argument_ast_value).collect::<Vec<_>>(),
            "optional": call.optional,
        }),
        Expression::ChainExpression(chain) => json!({
            "type": "ChainExpression",
            "expression": chain_element_ast_value(&chain.expression),
        }),
        Expression::ConditionalExpression(conditional) => json!({
            "type": "ConditionalExpression",
            "test": expression_ast_value(&conditional.test),
            "consequent": expression_ast_value(&conditional.consequent),
            "alternate": expression_ast_value(&conditional.alternate),
        }),
        Expression::FunctionExpression(function) => json!({
            "type": "FunctionExpression",
            "params": formal_parameters_ast_values(&function.params),
            "body": function.body.as_ref().map(|body| function_body_ast_value(body)),
        }),
        Expression::Identifier(identifier) => identifier_reference_ast_value(identifier),
        Expression::ImportExpression(import_expression) => json!({
            "type": "ImportExpression",
            "source": expression_ast_value(&import_expression.source),
            "options": import_expression.options.as_ref().map(expression_ast_value),
        }),
        Expression::LogicalExpression(logical) => json!({
            "type": "LogicalExpression",
            "left": expression_ast_value(&logical.left),
            "right": expression_ast_value(&logical.right),
        }),
        Expression::ComputedMemberExpression(member) => computed_member_ast_value(member),
        Expression::StaticMemberExpression(member) => static_member_ast_value(member),
        Expression::PrivateFieldExpression(member) => private_field_ast_value(member),
        Expression::NewExpression(new_expression) => json!({
            "type": "NewExpression",
            "callee": expression_ast_value(&new_expression.callee),
            "arguments": new_expression.arguments.iter().map(argument_ast_value).collect::<Vec<_>>(),
        }),
        Expression::ObjectExpression(object) => json!({
            "type": "ObjectExpression",
            "properties": object.properties.iter().map(object_property_kind_ast_value).collect::<Vec<_>>(),
        }),
        Expression::ParenthesizedExpression(parenthesized) => {
            expression_ast_value(&parenthesized.expression)
        }
        Expression::PrivateInExpression(private_in) => json!({
            "type": "BinaryExpression",
            "right": expression_ast_value(&private_in.right),
        }),
        Expression::SequenceExpression(sequence) => json!({
            "type": "SequenceExpression",
            "expressions": sequence.expressions.iter().map(expression_ast_value).collect::<Vec<_>>(),
        }),
        Expression::TaggedTemplateExpression(tagged) => json!({
            "type": "TaggedTemplateExpression",
            "tag": expression_ast_value(&tagged.tag),
            "quasi": template_literal_ast_value(&tagged.quasi),
        }),
        Expression::TemplateLiteral(template) => template_literal_ast_value(template),
        Expression::ThisExpression(_) => json!({ "type": "ThisExpression" }),
        Expression::UnaryExpression(unary) => json!({
            "type": "UnaryExpression",
            "argument": expression_ast_value(&unary.argument),
        }),
        Expression::UpdateExpression(update) => json!({
            "type": "UpdateExpression",
            "argument": simple_assignment_target_ast_value(&update.argument),
        }),
        Expression::YieldExpression(yield_expression) => json!({
            "type": "YieldExpression",
            "argument": yield_expression.argument.as_ref().map(expression_ast_value),
        }),
        Expression::BooleanLiteral(literal) => json!({
            "type": "Literal",
            "value": literal.value,
        }),
        Expression::NullLiteral(_) => json!({
            "type": "Literal",
            "value": Value::Null,
        }),
        Expression::NumericLiteral(literal) => json!({
            "type": "Literal",
            "value": literal.value,
        }),
        Expression::StringLiteral(literal) => json!({
            "type": "Literal",
            "value": literal.value.as_str(),
        }),
        Expression::BigIntLiteral(literal) => json!({
            "type": "Literal",
            "value": literal.value.as_str(),
        }),
        Expression::RegExpLiteral(_) => json!({ "type": "Literal" }),
        Expression::TSAsExpression(expression) => {
            ts_expression_ast_value("TSAsExpression", &expression.expression)
        }
        Expression::TSSatisfiesExpression(expression) => {
            ts_expression_ast_value("TSSatisfiesExpression", &expression.expression)
        }
        Expression::TSTypeAssertion(expression) => {
            ts_expression_ast_value("TSTypeAssertion", &expression.expression)
        }
        Expression::TSNonNullExpression(expression) => {
            ts_expression_ast_value("TSNonNullExpression", &expression.expression)
        }
        Expression::TSInstantiationExpression(expression) => {
            ts_expression_ast_value("TSInstantiationExpression", &expression.expression)
        }
        _ => json!({ "type": "Expression" }),
    }
}

pub(crate) fn statement_ast_value(statement: &Statement<'_>) -> Value {
    match statement {
        Statement::BlockStatement(block) => json!({
            "type": "BlockStatement",
            "body": block.body.iter().map(statement_ast_value).collect::<Vec<_>>(),
        }),
        Statement::DoWhileStatement(statement) => json!({
            "type": "DoWhileStatement",
            "body": statement_ast_value(&statement.body),
            "test": expression_ast_value(&statement.test),
        }),
        Statement::ExpressionStatement(statement) => json!({
            "type": "ExpressionStatement",
            "expression": expression_ast_value(&statement.expression),
        }),
        Statement::ForStatement(statement) => json!({
            "type": "ForStatement",
            "test": statement.test.as_ref().map(expression_ast_value),
            "update": statement.update.as_ref().map(expression_ast_value),
            "body": statement_ast_value(&statement.body),
        }),
        Statement::IfStatement(statement) => json!({
            "type": "IfStatement",
            "test": expression_ast_value(&statement.test),
            "consequent": statement_ast_value(&statement.consequent),
            "alternate": statement.alternate.as_ref().map(statement_ast_value),
        }),
        Statement::ReturnStatement(statement) => json!({
            "type": "ReturnStatement",
            "argument": statement.argument.as_ref().map(expression_ast_value),
        }),
        Statement::ThrowStatement(statement) => json!({
            "type": "ThrowStatement",
            "argument": expression_ast_value(&statement.argument),
        }),
        Statement::VariableDeclaration(declaration) => json!({
            "type": "VariableDeclaration",
            "declarations": declaration.declarations.iter().map(|declarator| json!({
                "type": "VariableDeclarator",
                "id": binding_pattern_ast_value(&declarator.id),
                "init": declarator.init.as_ref().map(expression_ast_value),
            })).collect::<Vec<_>>(),
        }),
        Statement::WhileStatement(statement) => json!({
            "type": "WhileStatement",
            "test": expression_ast_value(&statement.test),
            "body": statement_ast_value(&statement.body),
        }),
        _ => json!({ "type": statement_type_name(statement) }),
    }
}

pub(crate) fn statement_type_name(statement: &Statement<'_>) -> &'static str {
    match statement {
        Statement::BlockStatement(_) => "BlockStatement",
        Statement::BreakStatement(_) => "BreakStatement",
        Statement::ContinueStatement(_) => "ContinueStatement",
        Statement::DebuggerStatement(_) => "DebuggerStatement",
        Statement::DoWhileStatement(_) => "DoWhileStatement",
        Statement::EmptyStatement(_) => "EmptyStatement",
        Statement::ExpressionStatement(_) => "ExpressionStatement",
        Statement::ForInStatement(_) => "ForInStatement",
        Statement::ForOfStatement(_) => "ForOfStatement",
        Statement::ForStatement(_) => "ForStatement",
        Statement::IfStatement(_) => "IfStatement",
        Statement::ReturnStatement(_) => "ReturnStatement",
        Statement::SwitchStatement(_) => "SwitchStatement",
        Statement::ThrowStatement(_) => "ThrowStatement",
        Statement::TryStatement(_) => "TryStatement",
        Statement::VariableDeclaration(_) => "VariableDeclaration",
        Statement::WhileStatement(_) => "WhileStatement",
        _ => "Statement",
    }
}

pub(crate) fn identifier_reference_ast_value(
    identifier: &oxc_ast::ast::IdentifierReference<'_>,
) -> Value {
    json!({
        "type": "Identifier",
        "name": identifier.name.as_str(),
    })
}

pub(crate) fn identifier_name_ast_value(identifier: &oxc_ast::ast::IdentifierName<'_>) -> Value {
    json!({
        "type": "Identifier",
        "name": identifier.name.as_str(),
    })
}

pub(crate) fn private_identifier_ast_value(
    identifier: &oxc_ast::ast::PrivateIdentifier<'_>,
) -> Value {
    json!({
        "type": "PrivateName",
        "name": identifier.name.as_str(),
    })
}

pub(crate) fn computed_member_ast_value(
    member: &oxc_ast::ast::ComputedMemberExpression<'_>,
) -> Value {
    json!({
        "type": "MemberExpression",
        "object": expression_ast_value(&member.object),
        "property": expression_ast_value(&member.expression),
        "computed": true,
        "optional": member.optional,
    })
}

pub(crate) fn static_member_ast_value(member: &oxc_ast::ast::StaticMemberExpression<'_>) -> Value {
    json!({
        "type": "MemberExpression",
        "object": expression_ast_value(&member.object),
        "property": identifier_name_ast_value(&member.property),
        "computed": false,
        "optional": member.optional,
    })
}

pub(crate) fn private_field_ast_value(member: &oxc_ast::ast::PrivateFieldExpression<'_>) -> Value {
    json!({
        "type": "MemberExpression",
        "object": expression_ast_value(&member.object),
        "property": private_identifier_ast_value(&member.field),
        "computed": false,
        "optional": member.optional,
    })
}

pub(crate) fn template_literal_ast_value(template: &oxc_ast::ast::TemplateLiteral<'_>) -> Value {
    json!({
        "type": "TemplateLiteral",
        "expressions": template.expressions.iter().map(expression_ast_value).collect::<Vec<_>>(),
    })
}

pub(crate) fn ts_expression_ast_value(kind: &str, expression: &Expression<'_>) -> Value {
    json!({
        "type": kind,
        "expression": expression_ast_value(expression),
    })
}

pub(crate) fn array_element_ast_value(element: &ArrayExpressionElement<'_>) -> Value {
    match element {
        ArrayExpressionElement::SpreadElement(spread) => json!({
            "type": "SpreadElement",
            "argument": expression_ast_value(&spread.argument),
        }),
        ArrayExpressionElement::Elision(_) => Value::Null,
        _ => element
            .as_expression()
            .map(expression_ast_value)
            .unwrap_or_else(|| json!({ "type": "Expression" })),
    }
}

pub(crate) fn argument_ast_value(argument: &Argument<'_>) -> Value {
    match argument {
        Argument::SpreadElement(spread) => json!({
            "type": "SpreadElement",
            "argument": expression_ast_value(&spread.argument),
        }),
        _ => argument
            .as_expression()
            .map(expression_ast_value)
            .unwrap_or_else(|| json!({ "type": "Expression" })),
    }
}

pub(crate) fn object_property_kind_ast_value(property: &ObjectPropertyKind<'_>) -> Value {
    match property {
        ObjectPropertyKind::ObjectProperty(property) => json!({
            "type": "ObjectProperty",
            "key": property_key_ast_value(&property.key),
            "value": expression_ast_value(&property.value),
            "computed": property.computed,
            "shorthand": property.shorthand,
        }),
        ObjectPropertyKind::SpreadProperty(spread) => json!({
            "type": "SpreadElement",
            "argument": expression_ast_value(&spread.argument),
        }),
    }
}

pub(crate) fn property_key_ast_value(key: &PropertyKey<'_>) -> Value {
    match key {
        PropertyKey::StaticIdentifier(identifier) => identifier_name_ast_value(identifier),
        PropertyKey::PrivateIdentifier(identifier) => private_identifier_ast_value(identifier),
        _ => key
            .as_expression()
            .map(expression_ast_value)
            .unwrap_or_else(|| json!({ "type": "Identifier", "name": "" })),
    }
}

pub(crate) fn chain_element_ast_value(element: &ChainElement<'_>) -> Value {
    match element {
        ChainElement::CallExpression(call) => json!({
            "type": "CallExpression",
            "callee": expression_ast_value(&call.callee),
            "arguments": call.arguments.iter().map(argument_ast_value).collect::<Vec<_>>(),
            "optional": call.optional,
        }),
        ChainElement::ComputedMemberExpression(member) => computed_member_ast_value(member),
        ChainElement::StaticMemberExpression(member) => static_member_ast_value(member),
        ChainElement::PrivateFieldExpression(member) => private_field_ast_value(member),
        ChainElement::TSNonNullExpression(expression) => {
            ts_expression_ast_value("TSNonNullExpression", &expression.expression)
        }
    }
}

pub(crate) fn assignment_target_ast_value(target: &AssignmentTarget<'_>) -> Value {
    match target {
        AssignmentTarget::AssignmentTargetIdentifier(identifier) => {
            identifier_reference_ast_value(identifier)
        }
        AssignmentTarget::ComputedMemberExpression(member) => computed_member_ast_value(member),
        AssignmentTarget::StaticMemberExpression(member) => static_member_ast_value(member),
        AssignmentTarget::PrivateFieldExpression(member) => private_field_ast_value(member),
        AssignmentTarget::TSAsExpression(expression) => {
            ts_expression_ast_value("TSAsExpression", &expression.expression)
        }
        AssignmentTarget::TSSatisfiesExpression(expression) => {
            ts_expression_ast_value("TSSatisfiesExpression", &expression.expression)
        }
        AssignmentTarget::TSNonNullExpression(expression) => {
            ts_expression_ast_value("TSNonNullExpression", &expression.expression)
        }
        AssignmentTarget::TSTypeAssertion(expression) => {
            ts_expression_ast_value("TSTypeAssertion", &expression.expression)
        }
        AssignmentTarget::ArrayAssignmentTarget(target) => json!({
            "type": "ArrayPattern",
            "elements": target.elements.iter().map(|element| {
                element
                    .as_ref()
                    .map(assignment_target_maybe_default_ast_value)
                    .unwrap_or(Value::Null)
            }).collect::<Vec<_>>(),
            "rest": target.rest.as_ref().map(|rest| json!({
                "type": "RestElement",
                "argument": assignment_target_ast_value(&rest.target),
            })),
        }),
        AssignmentTarget::ObjectAssignmentTarget(target) => json!({
            "type": "ObjectPattern",
            "properties": target.properties.iter().map(assignment_target_property_ast_value).collect::<Vec<_>>(),
            "rest": target.rest.as_ref().map(|rest| json!({
                "type": "RestElement",
                "argument": assignment_target_ast_value(&rest.target),
            })),
        }),
    }
}

pub(crate) fn simple_assignment_target_ast_value(target: &SimpleAssignmentTarget<'_>) -> Value {
    match target {
        SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) => {
            identifier_reference_ast_value(identifier)
        }
        SimpleAssignmentTarget::ComputedMemberExpression(member) => {
            computed_member_ast_value(member)
        }
        SimpleAssignmentTarget::StaticMemberExpression(member) => static_member_ast_value(member),
        SimpleAssignmentTarget::PrivateFieldExpression(member) => private_field_ast_value(member),
        SimpleAssignmentTarget::TSAsExpression(expression) => {
            ts_expression_ast_value("TSAsExpression", &expression.expression)
        }
        SimpleAssignmentTarget::TSSatisfiesExpression(expression) => {
            ts_expression_ast_value("TSSatisfiesExpression", &expression.expression)
        }
        SimpleAssignmentTarget::TSNonNullExpression(expression) => {
            ts_expression_ast_value("TSNonNullExpression", &expression.expression)
        }
        SimpleAssignmentTarget::TSTypeAssertion(expression) => {
            ts_expression_ast_value("TSTypeAssertion", &expression.expression)
        }
    }
}

pub(crate) fn assignment_target_maybe_default_ast_value(
    target: &oxc_ast::ast::AssignmentTargetMaybeDefault<'_>,
) -> Value {
    match target {
        oxc_ast::ast::AssignmentTargetMaybeDefault::AssignmentTargetWithDefault(target) => json!({
            "type": "AssignmentPattern",
            "left": assignment_target_ast_value(&target.binding),
            "right": expression_ast_value(&target.init),
        }),
        oxc_ast::ast::AssignmentTargetMaybeDefault::AssignmentTargetIdentifier(identifier) => {
            identifier_reference_ast_value(identifier)
        }
        oxc_ast::ast::AssignmentTargetMaybeDefault::ComputedMemberExpression(member) => {
            computed_member_ast_value(member)
        }
        oxc_ast::ast::AssignmentTargetMaybeDefault::StaticMemberExpression(member) => {
            static_member_ast_value(member)
        }
        oxc_ast::ast::AssignmentTargetMaybeDefault::PrivateFieldExpression(member) => {
            private_field_ast_value(member)
        }
        oxc_ast::ast::AssignmentTargetMaybeDefault::TSAsExpression(expression) => {
            ts_expression_ast_value("TSAsExpression", &expression.expression)
        }
        oxc_ast::ast::AssignmentTargetMaybeDefault::TSSatisfiesExpression(expression) => {
            ts_expression_ast_value("TSSatisfiesExpression", &expression.expression)
        }
        oxc_ast::ast::AssignmentTargetMaybeDefault::TSNonNullExpression(expression) => {
            ts_expression_ast_value("TSNonNullExpression", &expression.expression)
        }
        oxc_ast::ast::AssignmentTargetMaybeDefault::TSTypeAssertion(expression) => {
            ts_expression_ast_value("TSTypeAssertion", &expression.expression)
        }
        oxc_ast::ast::AssignmentTargetMaybeDefault::ArrayAssignmentTarget(target) => json!({
            "type": "ArrayPattern",
            "elements": target.elements.iter().map(|element| {
                element
                    .as_ref()
                    .map(assignment_target_maybe_default_ast_value)
                    .unwrap_or(Value::Null)
            }).collect::<Vec<_>>(),
        }),
        oxc_ast::ast::AssignmentTargetMaybeDefault::ObjectAssignmentTarget(target) => json!({
            "type": "ObjectPattern",
            "properties": target.properties.iter().map(assignment_target_property_ast_value).collect::<Vec<_>>(),
        }),
    }
}

pub(crate) fn assignment_target_property_ast_value(
    property: &oxc_ast::ast::AssignmentTargetProperty<'_>,
) -> Value {
    match property {
        oxc_ast::ast::AssignmentTargetProperty::AssignmentTargetPropertyIdentifier(property) => {
            let mut value = json!({
                "type": "ObjectProperty",
                "key": identifier_reference_ast_value(&property.binding),
                "value": identifier_reference_ast_value(&property.binding),
                "computed": false,
                "shorthand": true,
            });
            if let Some(init) = &property.init {
                value["value"] = json!({
                    "type": "AssignmentPattern",
                    "left": identifier_reference_ast_value(&property.binding),
                    "right": expression_ast_value(init),
                });
            }
            value
        }
        oxc_ast::ast::AssignmentTargetProperty::AssignmentTargetPropertyProperty(property) => {
            json!({
                "type": "ObjectProperty",
                "key": property_key_ast_value(&property.name),
                "value": assignment_target_maybe_default_ast_value(&property.binding),
                "computed": property.computed,
                "shorthand": false,
            })
        }
    }
}

pub(crate) fn formal_parameters_ast_values(
    parameters: &oxc_ast::ast::FormalParameters<'_>,
) -> Vec<Value> {
    let mut params = parameters
        .items
        .iter()
        .map(formal_parameter_ast_value)
        .collect::<Vec<_>>();
    if let Some(rest) = &parameters.rest {
        params.push(json!({
            "type": "RestElement",
            "argument": binding_pattern_ast_value(&rest.rest.argument),
        }));
    }
    params
}

pub(crate) fn formal_parameter_ast_value(parameter: &FormalParameter<'_>) -> Value {
    let pattern = binding_pattern_ast_value(&parameter.pattern);
    match &parameter.initializer {
        Some(initializer) => json!({
            "type": "AssignmentPattern",
            "left": pattern,
            "right": expression_ast_value(initializer),
        }),
        None => pattern,
    }
}

pub(crate) fn function_body_ast_value(body: &oxc_ast::ast::FunctionBody<'_>) -> Value {
    json!({
        "type": "BlockStatement",
        "body": body.statements.iter().map(statement_ast_value).collect::<Vec<_>>(),
    })
}

pub(crate) fn binding_pattern_ast_value(pattern: &BindingPattern<'_>) -> Value {
    match pattern {
        BindingPattern::BindingIdentifier(identifier) => json!({
            "type": "Identifier",
            "name": identifier.name.as_str(),
        }),
        BindingPattern::ObjectPattern(pattern) => {
            let mut properties = pattern
                .properties
                .iter()
                .map(binding_property_ast_value)
                .collect::<Vec<_>>();
            if let Some(rest) = &pattern.rest {
                properties.push(json!({
                    "type": "RestElement",
                    "argument": binding_pattern_ast_value(&rest.argument),
                }));
            }
            json!({
                "type": "ObjectPattern",
                "properties": properties,
            })
        }
        BindingPattern::ArrayPattern(pattern) => {
            let mut elements = pattern
                .elements
                .iter()
                .map(|element| {
                    element
                        .as_ref()
                        .map(binding_pattern_ast_value)
                        .unwrap_or(Value::Null)
                })
                .collect::<Vec<_>>();
            if let Some(rest) = &pattern.rest {
                elements.push(json!({
                    "type": "RestElement",
                    "argument": binding_pattern_ast_value(&rest.argument),
                }));
            }
            json!({
                "type": "ArrayPattern",
                "elements": elements,
            })
        }
        BindingPattern::AssignmentPattern(pattern) => json!({
            "type": "AssignmentPattern",
            "left": binding_pattern_ast_value(&pattern.left),
            "right": expression_ast_value(&pattern.right),
        }),
    }
}

pub(crate) fn binding_property_ast_value(property: &oxc_ast::ast::BindingProperty<'_>) -> Value {
    json!({
        "type": "ObjectProperty",
        "key": property_key_ast_value(&property.key),
        "value": binding_pattern_ast_value(&property.value),
        "computed": property.computed,
        "shorthand": property.shorthand,
    })
}

pub(crate) fn split_v_for_expression(source: &str) -> Option<(&str, &str)> {
    let mut depth = 0usize;
    let mut index = 0usize;
    while index < source.len() {
        let ch = source[index..].chars().next()?;
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ' ' if depth == 0 => {
                let rest = &source[index..];
                if rest.starts_with(" in ") {
                    return Some((source[..index].trim(), source[index + 4..].trim()));
                }
                if rest.starts_with(" of ") {
                    return Some((source[..index].trim(), source[index + 4..].trim()));
                }
            }
            _ => {}
        }
        index += ch.len_utf8();
    }
    None
}

pub(crate) fn split_v_for_aliases(source: &str) -> Vec<String> {
    let aliases = source
        .trim()
        .strip_prefix('(')
        .and_then(|value| value.strip_suffix(')'))
        .unwrap_or_else(|| source.trim());
    split_top_level_csv(aliases)
        .into_iter()
        .map(ToOwned::to_owned)
        .collect()
}

pub(crate) fn split_top_level_csv(source: &str) -> Vec<&str> {
    let mut items = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (index, ch) in source.char_indices() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                let item = source[start..index].trim();
                if !item.is_empty() {
                    items.push(item);
                }
                start = index + 1;
            }
            _ => {}
        }
    }
    let tail = source[start..].trim();
    if !tail.is_empty() {
        items.push(tail);
    }
    items
}

pub(crate) fn vue3_expression_loc(
    source: &str,
    base_offset: usize,
    fallback_span: &NodeSpan,
    expression: &str,
) -> Value {
    let Some(span) = fallback_span.source() else {
        return vue3_loc_stub_value();
    };
    let local_span_start = span.start.0.saturating_sub(base_offset);
    let local_span_end = span.end.0.saturating_sub(base_offset).min(source.len());
    let node_source = source
        .get(local_span_start..local_span_end)
        .unwrap_or_default();
    if let Some((inner_start, inner_end)) =
        default_interpolation_inner_trimmed_span(source, local_span_start, local_span_end)
    {
        return vue3_source_loc_value(source, inner_start, inner_end);
    }
    let trimmed = expression.trim();
    if trimmed.is_empty() {
        let inner_start = if node_source.starts_with("{{") {
            local_span_start + "{{".len()
        } else {
            local_span_start
        };
        return vue3_source_loc_value(source, inner_start, inner_start);
    }
    if let Some(local_start) = node_source.find(trimmed) {
        let start = local_span_start + local_start;
        return vue3_source_loc_value(source, start, start + trimmed.len());
    }
    vue3_loc_value(source, base_offset, fallback_span)
}

pub(crate) fn default_interpolation_inner_trimmed_span(
    source: &str,
    start: usize,
    end: usize,
) -> Option<(usize, usize)> {
    let slice = source.get(start..end)?;
    if !slice.starts_with("{{") || !slice.ends_with("}}") {
        return None;
    }
    let mut inner_start = start + "{{".len();
    let mut inner_end = end.saturating_sub("}}".len());
    while inner_start < inner_end
        && source
            .get(inner_start..inner_end)
            .and_then(|value| value.chars().next())
            .is_some_and(char::is_whitespace)
    {
        let ch = source[inner_start..inner_end].chars().next()?;
        inner_start += ch.len_utf8();
    }
    while inner_end > inner_start
        && source
            .get(inner_start..inner_end)
            .and_then(|value| value.chars().next_back())
            .is_some_and(char::is_whitespace)
    {
        let ch = source[inner_start..inner_end].chars().next_back()?;
        inner_end -= ch.len_utf8();
    }
    Some((inner_start, inner_end))
}

pub(crate) fn vue3_loc_value(source: &str, base_offset: usize, span: &NodeSpan) -> Value {
    let Some(span) = span.source() else {
        return vue3_loc_stub_value();
    };
    vue3_source_span_value(source, base_offset, span)
}

pub(crate) fn vue3_text_loc_value(source: &str, base_offset: usize, span: &NodeSpan) -> Value {
    let Some(source_span) = span.source() else {
        return vue3_loc_stub_value();
    };
    let start = source_span.start.0.saturating_sub(base_offset);
    let end = source_span.end.0.saturating_sub(base_offset);
    if end == source.len()
        && source_span.end.0 >= source_span.start.0
        && source
            .get(start..end)
            .is_some_and(|slice| slice == "/" && source.ends_with('/'))
        && source[..start].rfind('<').is_some_and(|tag_start| {
            source
                .get(tag_start..)
                .is_some_and(|slice| slice.starts_with('<') && !slice.contains('>'))
        })
    {
        return vue3_source_signed_start_loc_value(source, -1, end);
    }
    vue3_source_span_value(source, base_offset, source_span)
}

pub(crate) fn vue3_source_span_value(
    source: &str,
    base_offset: usize,
    span: vuec_source::Span,
) -> Value {
    let start = span.start.0.saturating_sub(base_offset);
    let end = span.end.0.saturating_sub(base_offset);
    vue3_source_loc_value(source, start, end)
}

pub(crate) fn vue3_source_signed_start_loc_value(source: &str, start: isize, end: usize) -> Value {
    let local_start = if start < 0 && end <= source.len() {
        end.saturating_sub(1)
    } else {
        start.max(0) as usize
    };
    let local_end = end.min(source.len()).max(local_start);
    json!({
        "start": vue3_signed_position(source, start),
        "end": vue3_position(source, end),
        "source": source.get(local_start..local_end).unwrap_or_default(),
    })
}

pub(crate) fn vue3_source_loc_value(source: &str, start: usize, end: usize) -> Value {
    let local_start = start.min(source.len());
    let local_end = end.min(source.len()).max(local_start);
    let start_pos = vue3_position(source, start);
    let end_pos = vue3_position(source, end);
    json!({
        "start": start_pos,
        "end": end_pos,
        "source": source.get(local_start..local_end).unwrap_or_default(),
    })
}

pub(crate) fn vue3_position(source: &str, offset: usize) -> Value {
    let mut line = 1usize;
    let mut column = 1usize;
    let mut index = 0usize;
    let mut utf16_offset = 0usize;
    for ch in source.chars() {
        if index >= offset {
            break;
        }
        index += ch.len_utf8();
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += ch.len_utf16();
        }
        utf16_offset += ch.len_utf16();
    }
    if offset > index {
        let extra = offset - index;
        column += extra;
        utf16_offset += extra;
    }
    json!({
        "offset": utf16_offset,
        "line": line,
        "column": column,
    })
}

pub(crate) fn vue3_signed_position(source: &str, offset: isize) -> Value {
    if offset >= 0 {
        return vue3_position(source, offset as usize);
    }
    json!({
        "offset": offset,
        "line": 1,
        "column": 1isize + offset,
    })
}

pub(crate) fn vue3_loc_stub_value() -> Value {
    json!({
        "start": { "offset": 0, "line": 1, "column": 1 },
        "end": { "offset": 0, "line": 1, "column": 1 },
        "source": "",
    })
}
