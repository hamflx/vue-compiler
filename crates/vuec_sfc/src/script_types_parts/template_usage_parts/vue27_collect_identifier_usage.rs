pub(crate) fn collect_vue27_statement_identifier_usage(
    statement: &Statement<'_>,
    value: &mut String,
) {
    match statement {
        Statement::BlockStatement(block) => {
            for statement in &block.body {
                collect_vue27_statement_identifier_usage(statement, value);
            }
        }
        Statement::ExpressionStatement(statement) => {
            collect_vue27_expression_identifier_usage(&statement.expression, value);
        }
        Statement::ReturnStatement(statement) => {
            if let Some(argument) = &statement.argument {
                collect_vue27_expression_identifier_usage(argument, value);
            }
        }
        Statement::VariableDeclaration(declaration) => {
            for declarator in &declaration.declarations {
                if let Some(init) = &declarator.init {
                    collect_vue27_expression_identifier_usage(init, value);
                }
            }
        }
        Statement::FunctionDeclaration(function) => {
            collect_vue27_function_identifier_usage(function, value);
        }
        Statement::IfStatement(statement) => {
            collect_vue27_expression_identifier_usage(&statement.test, value);
            collect_vue27_statement_identifier_usage(&statement.consequent, value);
            if let Some(alternate) = &statement.alternate {
                collect_vue27_statement_identifier_usage(alternate, value);
            }
        }
        Statement::ForStatement(statement) => {
            if let Some(init) = &statement.init {
                match init {
                    oxc_ast::ast::ForStatementInit::VariableDeclaration(declaration) => {
                        for declarator in &declaration.declarations {
                            if let Some(init) = &declarator.init {
                                collect_vue27_expression_identifier_usage(init, value);
                            }
                        }
                    }
                    _ => {
                        if let Some(expression) = init.as_expression() {
                            collect_vue27_expression_identifier_usage(expression, value);
                        }
                    }
                }
            }
            if let Some(test) = &statement.test {
                collect_vue27_expression_identifier_usage(test, value);
            }
            if let Some(update) = &statement.update {
                collect_vue27_expression_identifier_usage(update, value);
            }
            collect_vue27_statement_identifier_usage(&statement.body, value);
        }
        Statement::ForInStatement(statement) => {
            collect_vue27_expression_identifier_usage(&statement.right, value);
            collect_vue27_statement_identifier_usage(&statement.body, value);
        }
        Statement::ForOfStatement(statement) => {
            collect_vue27_expression_identifier_usage(&statement.right, value);
            collect_vue27_statement_identifier_usage(&statement.body, value);
        }
        Statement::WhileStatement(statement) => {
            collect_vue27_expression_identifier_usage(&statement.test, value);
            collect_vue27_statement_identifier_usage(&statement.body, value);
        }
        Statement::DoWhileStatement(statement) => {
            collect_vue27_statement_identifier_usage(&statement.body, value);
            collect_vue27_expression_identifier_usage(&statement.test, value);
        }
        Statement::SwitchStatement(statement) => {
            collect_vue27_expression_identifier_usage(&statement.discriminant, value);
            for case in &statement.cases {
                if let Some(test) = &case.test {
                    collect_vue27_expression_identifier_usage(test, value);
                }
                for statement in &case.consequent {
                    collect_vue27_statement_identifier_usage(statement, value);
                }
            }
        }
        Statement::ThrowStatement(statement) => {
            collect_vue27_expression_identifier_usage(&statement.argument, value);
        }
        Statement::TryStatement(statement) => {
            for statement in &statement.block.body {
                collect_vue27_statement_identifier_usage(statement, value);
            }
            if let Some(handler) = &statement.handler {
                for statement in &handler.body.body {
                    collect_vue27_statement_identifier_usage(statement, value);
                }
            }
            if let Some(finalizer) = &statement.finalizer {
                for statement in &finalizer.body {
                    collect_vue27_statement_identifier_usage(statement, value);
                }
            }
        }
        Statement::WithStatement(statement) => {
            collect_vue27_expression_identifier_usage(&statement.object, value);
            collect_vue27_statement_identifier_usage(&statement.body, value);
        }
        Statement::LabeledStatement(statement) => {
            collect_vue27_statement_identifier_usage(&statement.body, value);
        }
        _ => {}
    }
}

pub(crate) fn collect_vue27_expression_identifier_usage(
    expression: &Expression<'_>,
    value: &mut String,
) {
    match expression {
        Expression::Identifier(identifier) => {
            push_vue27_identifier_usage(value, identifier.name.as_str())
        }
        Expression::ArrayExpression(array) => {
            for element in &array.elements {
                match element {
                    oxc_ast::ast::ArrayExpressionElement::SpreadElement(spread) => {
                        collect_vue27_expression_identifier_usage(&spread.argument, value);
                    }
                    oxc_ast::ast::ArrayExpressionElement::Elision(_) => {}
                    element => {
                        if let Some(expression) = element.as_expression() {
                            collect_vue27_expression_identifier_usage(expression, value);
                        }
                    }
                }
            }
        }
        Expression::ObjectExpression(object) => {
            for property in &object.properties {
                match property {
                    ObjectPropertyKind::ObjectProperty(property) => {
                        if property.computed {
                            collect_vue27_property_key_identifier_usage(&property.key, value);
                        }
                        collect_vue27_expression_identifier_usage(&property.value, value);
                    }
                    ObjectPropertyKind::SpreadProperty(spread) => {
                        collect_vue27_expression_identifier_usage(&spread.argument, value);
                    }
                }
            }
        }
        Expression::CallExpression(call) => {
            collect_vue27_expression_identifier_usage(&call.callee, value);
            for argument in &call.arguments {
                collect_vue27_argument_identifier_usage(argument, value);
            }
        }
        Expression::NewExpression(expression) => {
            collect_vue27_expression_identifier_usage(&expression.callee, value);
            for argument in &expression.arguments {
                collect_vue27_argument_identifier_usage(argument, value);
            }
        }
        Expression::StaticMemberExpression(member) => {
            collect_vue27_expression_identifier_usage(&member.object, value);
        }
        Expression::ComputedMemberExpression(member) => {
            collect_vue27_expression_identifier_usage(&member.object, value);
            collect_vue27_expression_identifier_usage(&member.expression, value);
        }
        Expression::PrivateFieldExpression(member) => {
            collect_vue27_expression_identifier_usage(&member.object, value);
        }
        Expression::FunctionExpression(function) => {
            collect_vue27_function_identifier_usage(function, value);
        }
        Expression::ArrowFunctionExpression(function) => {
            collect_vue27_arrow_function_identifier_usage(function, value);
        }
        Expression::AssignmentExpression(assignment) => {
            collect_vue27_assignment_target_identifier_usage(&assignment.left, value);
            collect_vue27_expression_identifier_usage(&assignment.right, value);
        }
        Expression::UpdateExpression(update) => {
            collect_vue27_simple_assignment_target_identifier_usage(&update.argument, value);
        }
        Expression::UnaryExpression(unary) => {
            collect_vue27_expression_identifier_usage(&unary.argument, value);
        }
        Expression::AwaitExpression(expression) => {
            collect_vue27_expression_identifier_usage(&expression.argument, value);
        }
        Expression::BinaryExpression(binary) => {
            collect_vue27_expression_identifier_usage(&binary.left, value);
            collect_vue27_expression_identifier_usage(&binary.right, value);
        }
        Expression::PrivateInExpression(expression) => {
            collect_vue27_expression_identifier_usage(&expression.right, value);
        }
        Expression::LogicalExpression(logical) => {
            collect_vue27_expression_identifier_usage(&logical.left, value);
            collect_vue27_expression_identifier_usage(&logical.right, value);
        }
        Expression::ConditionalExpression(conditional) => {
            collect_vue27_expression_identifier_usage(&conditional.test, value);
            collect_vue27_expression_identifier_usage(&conditional.consequent, value);
            collect_vue27_expression_identifier_usage(&conditional.alternate, value);
        }
        Expression::SequenceExpression(sequence) => {
            for expression in &sequence.expressions {
                collect_vue27_expression_identifier_usage(expression, value);
            }
        }
        Expression::TemplateLiteral(template) => {
            for expression in &template.expressions {
                collect_vue27_expression_identifier_usage(expression, value);
            }
        }
        Expression::TaggedTemplateExpression(template) => {
            collect_vue27_expression_identifier_usage(&template.tag, value);
            for expression in &template.quasi.expressions {
                collect_vue27_expression_identifier_usage(expression, value);
            }
        }
        Expression::ParenthesizedExpression(parenthesized) => {
            collect_vue27_expression_identifier_usage(&parenthesized.expression, value);
        }
        Expression::TSAsExpression(expression) => {
            collect_vue27_expression_identifier_usage(&expression.expression, value);
        }
        Expression::TSSatisfiesExpression(expression) => {
            collect_vue27_expression_identifier_usage(&expression.expression, value);
        }
        Expression::TSTypeAssertion(expression) => {
            collect_vue27_expression_identifier_usage(&expression.expression, value);
        }
        Expression::TSNonNullExpression(expression) => {
            collect_vue27_expression_identifier_usage(&expression.expression, value);
        }
        Expression::TSInstantiationExpression(expression) => {
            collect_vue27_expression_identifier_usage(&expression.expression, value);
        }
        Expression::ChainExpression(chain) => match &chain.expression {
            oxc_ast::ast::ChainElement::CallExpression(call) => {
                collect_vue27_expression_identifier_usage(&call.callee, value);
                for argument in &call.arguments {
                    collect_vue27_argument_identifier_usage(argument, value);
                }
            }
            oxc_ast::ast::ChainElement::TSNonNullExpression(expression) => {
                collect_vue27_expression_identifier_usage(&expression.expression, value);
            }
            oxc_ast::ast::ChainElement::StaticMemberExpression(member) => {
                collect_vue27_expression_identifier_usage(&member.object, value);
            }
            oxc_ast::ast::ChainElement::ComputedMemberExpression(member) => {
                collect_vue27_expression_identifier_usage(&member.object, value);
                collect_vue27_expression_identifier_usage(&member.expression, value);
            }
            oxc_ast::ast::ChainElement::PrivateFieldExpression(member) => {
                collect_vue27_expression_identifier_usage(&member.object, value);
            }
        },
        _ => {}
    }
}
