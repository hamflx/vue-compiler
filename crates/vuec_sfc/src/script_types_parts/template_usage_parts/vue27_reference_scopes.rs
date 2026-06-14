pub(crate) fn vue27_expression_references_setup_local(
    expression: &Expression<'_>,
    setup_bindings: &BTreeSet<String>,
) -> bool {
    let mut scope = BTreeSet::new();
    vue27_expression_references_setup_local_with_scope(expression, setup_bindings, &mut scope)
}

pub(crate) fn vue27_expression_references_setup_local_with_scope(
    expression: &Expression<'_>,
    setup_bindings: &BTreeSet<String>,
    scope: &mut BTreeSet<String>,
) -> bool {
    match expression {
        Expression::Identifier(identifier) => {
            setup_bindings.contains(identifier.name.as_str())
                && !scope.contains(identifier.name.as_str())
        }
        Expression::ArrayExpression(array) => array.elements.iter().any(|element| match element {
            oxc_ast::ast::ArrayExpressionElement::SpreadElement(spread) => {
                vue27_expression_references_setup_local_with_scope(
                    &spread.argument,
                    setup_bindings,
                    scope,
                )
            }
            oxc_ast::ast::ArrayExpressionElement::Elision(_) => false,
            element => element.as_expression().is_some_and(|expression| {
                vue27_expression_references_setup_local_with_scope(
                    expression,
                    setup_bindings,
                    scope,
                )
            }),
        }),
        Expression::ObjectExpression(object) => {
            object.properties.iter().any(|property| match property {
                ObjectPropertyKind::ObjectProperty(property) => {
                    (property.computed
                        && vue27_property_key_references_setup_local(
                            &property.key,
                            setup_bindings,
                            scope,
                        ))
                        || vue27_expression_references_setup_local_with_scope(
                            &property.value,
                            setup_bindings,
                            scope,
                        )
                }
                ObjectPropertyKind::SpreadProperty(spread) => {
                    vue27_expression_references_setup_local_with_scope(
                        &spread.argument,
                        setup_bindings,
                        scope,
                    )
                }
            })
        }
        Expression::CallExpression(call) => {
            vue27_expression_references_setup_local_with_scope(&call.callee, setup_bindings, scope)
                || call.arguments.iter().any(|argument| {
                    vue27_argument_references_setup_local(argument, setup_bindings, scope)
                })
        }
        Expression::NewExpression(expression) => {
            vue27_expression_references_setup_local_with_scope(
                &expression.callee,
                setup_bindings,
                scope,
            ) || expression.arguments.iter().any(|argument| {
                vue27_argument_references_setup_local(argument, setup_bindings, scope)
            })
        }
        Expression::StaticMemberExpression(member) => {
            vue27_expression_references_setup_local_with_scope(
                &member.object,
                setup_bindings,
                scope,
            )
        }
        Expression::ComputedMemberExpression(member) => {
            vue27_expression_references_setup_local_with_scope(
                &member.object,
                setup_bindings,
                scope,
            ) || vue27_expression_references_setup_local_with_scope(
                &member.expression,
                setup_bindings,
                scope,
            )
        }
        Expression::PrivateFieldExpression(member) => {
            vue27_expression_references_setup_local_with_scope(
                &member.object,
                setup_bindings,
                scope,
            )
        }
        Expression::FunctionExpression(function) => {
            vue27_function_references_setup_local(function, setup_bindings, scope)
        }
        Expression::ArrowFunctionExpression(function) => {
            vue27_arrow_function_references_setup_local(function, setup_bindings, scope)
        }
        Expression::AssignmentExpression(assignment) => {
            vue27_assignment_target_references_setup_local(&assignment.left, setup_bindings, scope)
                || vue27_expression_references_setup_local_with_scope(
                    &assignment.right,
                    setup_bindings,
                    scope,
                )
        }
        Expression::UpdateExpression(update) => {
            vue27_simple_assignment_target_references_setup_local(
                &update.argument,
                setup_bindings,
                scope,
            )
        }
        Expression::UnaryExpression(unary) => vue27_expression_references_setup_local_with_scope(
            &unary.argument,
            setup_bindings,
            scope,
        ),
        Expression::AwaitExpression(expression) => {
            vue27_expression_references_setup_local_with_scope(
                &expression.argument,
                setup_bindings,
                scope,
            )
        }
        Expression::BinaryExpression(binary) => {
            vue27_expression_references_setup_local_with_scope(&binary.left, setup_bindings, scope)
                || vue27_expression_references_setup_local_with_scope(
                    &binary.right,
                    setup_bindings,
                    scope,
                )
        }
        Expression::PrivateInExpression(expression) => {
            vue27_expression_references_setup_local_with_scope(
                &expression.right,
                setup_bindings,
                scope,
            )
        }
        Expression::LogicalExpression(logical) => {
            vue27_expression_references_setup_local_with_scope(&logical.left, setup_bindings, scope)
                || vue27_expression_references_setup_local_with_scope(
                    &logical.right,
                    setup_bindings,
                    scope,
                )
        }
        Expression::ConditionalExpression(conditional) => {
            vue27_expression_references_setup_local_with_scope(
                &conditional.test,
                setup_bindings,
                scope,
            ) || vue27_expression_references_setup_local_with_scope(
                &conditional.consequent,
                setup_bindings,
                scope,
            ) || vue27_expression_references_setup_local_with_scope(
                &conditional.alternate,
                setup_bindings,
                scope,
            )
        }
        Expression::SequenceExpression(sequence) => sequence.expressions.iter().any(|expression| {
            vue27_expression_references_setup_local_with_scope(expression, setup_bindings, scope)
        }),
        Expression::TemplateLiteral(template) => template.expressions.iter().any(|expression| {
            vue27_expression_references_setup_local_with_scope(expression, setup_bindings, scope)
        }),
        Expression::TaggedTemplateExpression(template) => {
            vue27_expression_references_setup_local_with_scope(&template.tag, setup_bindings, scope)
                || template.quasi.expressions.iter().any(|expression| {
                    vue27_expression_references_setup_local_with_scope(
                        expression,
                        setup_bindings,
                        scope,
                    )
                })
        }
        Expression::ParenthesizedExpression(parenthesized) => {
            vue27_expression_references_setup_local_with_scope(
                &parenthesized.expression,
                setup_bindings,
                scope,
            )
        }
        Expression::TSAsExpression(expression) => {
            vue27_expression_references_setup_local_with_scope(
                &expression.expression,
                setup_bindings,
                scope,
            )
        }
        Expression::TSSatisfiesExpression(expression) => {
            vue27_expression_references_setup_local_with_scope(
                &expression.expression,
                setup_bindings,
                scope,
            )
        }
        Expression::TSTypeAssertion(expression) => {
            vue27_expression_references_setup_local_with_scope(
                &expression.expression,
                setup_bindings,
                scope,
            )
        }
        Expression::TSNonNullExpression(expression) => {
            vue27_expression_references_setup_local_with_scope(
                &expression.expression,
                setup_bindings,
                scope,
            )
        }
        Expression::TSInstantiationExpression(expression) => {
            vue27_expression_references_setup_local_with_scope(
                &expression.expression,
                setup_bindings,
                scope,
            )
        }
        Expression::ChainExpression(chain) => match &chain.expression {
            oxc_ast::ast::ChainElement::CallExpression(call) => {
                vue27_expression_references_setup_local_with_scope(
                    &call.callee,
                    setup_bindings,
                    scope,
                ) || call.arguments.iter().any(|argument| {
                    vue27_argument_references_setup_local(argument, setup_bindings, scope)
                })
            }
            oxc_ast::ast::ChainElement::TSNonNullExpression(expression) => {
                vue27_expression_references_setup_local_with_scope(
                    &expression.expression,
                    setup_bindings,
                    scope,
                )
            }
            oxc_ast::ast::ChainElement::StaticMemberExpression(member) => {
                vue27_expression_references_setup_local_with_scope(
                    &member.object,
                    setup_bindings,
                    scope,
                )
            }
            oxc_ast::ast::ChainElement::ComputedMemberExpression(member) => {
                vue27_expression_references_setup_local_with_scope(
                    &member.object,
                    setup_bindings,
                    scope,
                ) || vue27_expression_references_setup_local_with_scope(
                    &member.expression,
                    setup_bindings,
                    scope,
                )
            }
            oxc_ast::ast::ChainElement::PrivateFieldExpression(member) => {
                vue27_expression_references_setup_local_with_scope(
                    &member.object,
                    setup_bindings,
                    scope,
                )
            }
        },
        _ => false,
    }
}

pub(crate) fn vue27_argument_references_setup_local(
    argument: &Argument<'_>,
    setup_bindings: &BTreeSet<String>,
    scope: &mut BTreeSet<String>,
) -> bool {
    match argument {
        Argument::SpreadElement(spread) => vue27_expression_references_setup_local_with_scope(
            &spread.argument,
            setup_bindings,
            scope,
        ),
        _ => vue27_expression_references_setup_local_with_scope(
            argument.to_expression(),
            setup_bindings,
            scope,
        ),
    }
}

pub(crate) fn vue27_property_key_references_setup_local(
    key: &PropertyKey<'_>,
    setup_bindings: &BTreeSet<String>,
    scope: &mut BTreeSet<String>,
) -> bool {
    match key {
        PropertyKey::StaticIdentifier(_) | PropertyKey::PrivateIdentifier(_) => false,
        _ => vue27_expression_references_setup_local_with_scope(
            key.to_expression(),
            setup_bindings,
            scope,
        ),
    }
}

pub(crate) fn vue27_function_references_setup_local(
    function: &Function<'_>,
    setup_bindings: &BTreeSet<String>,
    scope: &mut BTreeSet<String>,
) -> bool {
    let mut function_scope = scope.clone();
    if let Some(id) = &function.id {
        function_scope.insert(id.name.to_string());
    }
    insert_formal_parameter_bindings(&function.params, &mut function_scope);
    function.params.items.iter().any(|param| {
        param.initializer.as_ref().is_some_and(|initializer| {
            vue27_expression_references_setup_local_with_scope(initializer, setup_bindings, scope)
        })
    }) || function.body.as_ref().is_some_and(|body| {
        body.statements.iter().any(|statement| {
            vue27_statement_references_setup_local(statement, setup_bindings, &mut function_scope)
        })
    })
}

pub(crate) fn vue27_arrow_function_references_setup_local(
    function: &ArrowFunctionExpression<'_>,
    setup_bindings: &BTreeSet<String>,
    scope: &mut BTreeSet<String>,
) -> bool {
    let mut function_scope = scope.clone();
    insert_formal_parameter_bindings(&function.params, &mut function_scope);
    function.params.items.iter().any(|param| {
        param.initializer.as_ref().is_some_and(|initializer| {
            vue27_expression_references_setup_local_with_scope(initializer, setup_bindings, scope)
        })
    }) || function.body.statements.iter().any(|statement| {
        vue27_statement_references_setup_local(statement, setup_bindings, &mut function_scope)
    })
}

pub(crate) fn vue27_statement_references_setup_local(
    statement: &Statement<'_>,
    setup_bindings: &BTreeSet<String>,
    scope: &mut BTreeSet<String>,
) -> bool {
    match statement {
        Statement::BlockStatement(block) => {
            let mut block_scope = scope.clone();
            insert_vue27_block_declarations(&block.body, &mut block_scope);
            block.body.iter().any(|statement| {
                vue27_statement_references_setup_local(statement, setup_bindings, &mut block_scope)
            })
        }
        Statement::ExpressionStatement(statement) => {
            vue27_expression_references_setup_local_with_scope(
                &statement.expression,
                setup_bindings,
                scope,
            )
        }
        Statement::ReturnStatement(statement) => {
            statement.argument.as_ref().is_some_and(|argument| {
                vue27_expression_references_setup_local_with_scope(argument, setup_bindings, scope)
            })
        }
        Statement::VariableDeclaration(declaration) => {
            declaration.declarations.iter().any(|declarator| {
                declarator.init.as_ref().is_some_and(|init| {
                    vue27_expression_references_setup_local_with_scope(init, setup_bindings, scope)
                })
            })
        }
        Statement::FunctionDeclaration(function) => {
            vue27_function_references_setup_local(function, setup_bindings, scope)
        }
        Statement::IfStatement(statement) => {
            vue27_expression_references_setup_local_with_scope(
                &statement.test,
                setup_bindings,
                scope,
            ) || vue27_statement_references_setup_local(
                &statement.consequent,
                setup_bindings,
                scope,
            ) || statement.alternate.as_ref().is_some_and(|alternate| {
                vue27_statement_references_setup_local(alternate, setup_bindings, scope)
            })
        }
        Statement::ForStatement(statement) => {
            let init_refs = statement.init.as_ref().is_some_and(|init| match init {
                oxc_ast::ast::ForStatementInit::VariableDeclaration(declaration) => {
                    declaration.declarations.iter().any(|declarator| {
                        declarator.init.as_ref().is_some_and(|init| {
                            vue27_expression_references_setup_local_with_scope(
                                init,
                                setup_bindings,
                                scope,
                            )
                        })
                    })
                }
                _ => init.as_expression().is_some_and(|expression| {
                    vue27_expression_references_setup_local_with_scope(
                        expression,
                        setup_bindings,
                        scope,
                    )
                }),
            });
            init_refs
                || statement.test.as_ref().is_some_and(|test| {
                    vue27_expression_references_setup_local_with_scope(test, setup_bindings, scope)
                })
                || statement.update.as_ref().is_some_and(|update| {
                    vue27_expression_references_setup_local_with_scope(
                        update,
                        setup_bindings,
                        scope,
                    )
                })
                || vue27_statement_references_setup_local(&statement.body, setup_bindings, scope)
        }
        Statement::ForInStatement(statement) => {
            vue27_expression_references_setup_local_with_scope(
                &statement.right,
                setup_bindings,
                scope,
            ) || vue27_statement_references_setup_local(&statement.body, setup_bindings, scope)
        }
        Statement::ForOfStatement(statement) => {
            vue27_expression_references_setup_local_with_scope(
                &statement.right,
                setup_bindings,
                scope,
            ) || vue27_statement_references_setup_local(&statement.body, setup_bindings, scope)
        }
        Statement::WhileStatement(statement) => {
            vue27_expression_references_setup_local_with_scope(
                &statement.test,
                setup_bindings,
                scope,
            ) || vue27_statement_references_setup_local(&statement.body, setup_bindings, scope)
        }
        Statement::DoWhileStatement(statement) => {
            vue27_statement_references_setup_local(&statement.body, setup_bindings, scope)
                || vue27_expression_references_setup_local_with_scope(
                    &statement.test,
                    setup_bindings,
                    scope,
                )
        }
        Statement::SwitchStatement(statement) => {
            vue27_expression_references_setup_local_with_scope(
                &statement.discriminant,
                setup_bindings,
                scope,
            ) || statement.cases.iter().any(|case| {
                case.test.as_ref().is_some_and(|test| {
                    vue27_expression_references_setup_local_with_scope(test, setup_bindings, scope)
                }) || case.consequent.iter().any(|statement| {
                    vue27_statement_references_setup_local(statement, setup_bindings, scope)
                })
            })
        }
        Statement::ThrowStatement(statement) => vue27_expression_references_setup_local_with_scope(
            &statement.argument,
            setup_bindings,
            scope,
        ),
        Statement::TryStatement(statement) => {
            statement.block.body.iter().any(|statement| {
                vue27_statement_references_setup_local(statement, setup_bindings, scope)
            }) || statement.handler.as_ref().is_some_and(|handler| {
                handler.body.body.iter().any(|statement| {
                    vue27_statement_references_setup_local(statement, setup_bindings, scope)
                })
            }) || statement.finalizer.as_ref().is_some_and(|finalizer| {
                finalizer.body.iter().any(|statement| {
                    vue27_statement_references_setup_local(statement, setup_bindings, scope)
                })
            })
        }
        Statement::WithStatement(statement) => {
            vue27_expression_references_setup_local_with_scope(
                &statement.object,
                setup_bindings,
                scope,
            ) || vue27_statement_references_setup_local(&statement.body, setup_bindings, scope)
        }
        Statement::LabeledStatement(statement) => {
            vue27_statement_references_setup_local(&statement.body, setup_bindings, scope)
        }
        _ => false,
    }
}

pub(crate) fn vue27_assignment_target_references_setup_local(
    target: &AssignmentTarget<'_>,
    setup_bindings: &BTreeSet<String>,
    scope: &mut BTreeSet<String>,
) -> bool {
    match target {
        AssignmentTarget::AssignmentTargetIdentifier(identifier) => {
            setup_bindings.contains(identifier.name.as_str())
                && !scope.contains(identifier.name.as_str())
        }
        AssignmentTarget::StaticMemberExpression(member) => {
            vue27_expression_references_setup_local_with_scope(
                &member.object,
                setup_bindings,
                scope,
            )
        }
        AssignmentTarget::ComputedMemberExpression(member) => {
            vue27_expression_references_setup_local_with_scope(
                &member.object,
                setup_bindings,
                scope,
            ) || vue27_expression_references_setup_local_with_scope(
                &member.expression,
                setup_bindings,
                scope,
            )
        }
        AssignmentTarget::PrivateFieldExpression(member) => {
            vue27_expression_references_setup_local_with_scope(
                &member.object,
                setup_bindings,
                scope,
            )
        }
        AssignmentTarget::TSAsExpression(expression) => {
            vue27_expression_references_setup_local_with_scope(
                &expression.expression,
                setup_bindings,
                scope,
            )
        }
        AssignmentTarget::TSSatisfiesExpression(expression) => {
            vue27_expression_references_setup_local_with_scope(
                &expression.expression,
                setup_bindings,
                scope,
            )
        }
        AssignmentTarget::TSNonNullExpression(expression) => {
            vue27_expression_references_setup_local_with_scope(
                &expression.expression,
                setup_bindings,
                scope,
            )
        }
        AssignmentTarget::TSTypeAssertion(expression) => {
            vue27_expression_references_setup_local_with_scope(
                &expression.expression,
                setup_bindings,
                scope,
            )
        }
        AssignmentTarget::ArrayAssignmentTarget(target) => {
            target.elements.iter().any(|element| {
                element.as_ref().is_some_and(|element| {
                    vue27_assignment_target_maybe_default_references_setup_local(
                        element,
                        setup_bindings,
                        scope,
                    )
                })
            }) || target.rest.as_ref().is_some_and(|rest| {
                vue27_assignment_target_references_setup_local(&rest.target, setup_bindings, scope)
            })
        }
        AssignmentTarget::ObjectAssignmentTarget(target) => {
            target.properties.iter().any(|property| match property {
                oxc_ast::ast::AssignmentTargetProperty::AssignmentTargetPropertyIdentifier(
                    property,
                ) => {
                    (setup_bindings.contains(property.binding.name.as_str())
                        && !scope.contains(property.binding.name.as_str()))
                        || property.init.as_ref().is_some_and(|init| {
                            vue27_expression_references_setup_local_with_scope(
                                init,
                                setup_bindings,
                                scope,
                            )
                        })
                }
                oxc_ast::ast::AssignmentTargetProperty::AssignmentTargetPropertyProperty(
                    property,
                ) => {
                    (property.computed
                        && vue27_property_key_references_setup_local(
                            &property.name,
                            setup_bindings,
                            scope,
                        ))
                        || vue27_assignment_target_maybe_default_references_setup_local(
                            &property.binding,
                            setup_bindings,
                            scope,
                        )
                }
            }) || target.rest.as_ref().is_some_and(|rest| {
                vue27_assignment_target_references_setup_local(&rest.target, setup_bindings, scope)
            })
        }
    }
}

pub(crate) fn vue27_assignment_target_maybe_default_references_setup_local(
    target: &oxc_ast::ast::AssignmentTargetMaybeDefault<'_>,
    setup_bindings: &BTreeSet<String>,
    scope: &mut BTreeSet<String>,
) -> bool {
    match target {
        oxc_ast::ast::AssignmentTargetMaybeDefault::AssignmentTargetWithDefault(target) => {
            vue27_assignment_target_references_setup_local(&target.binding, setup_bindings, scope)
                || vue27_expression_references_setup_local_with_scope(
                    &target.init,
                    setup_bindings,
                    scope,
                )
        }
        _ => target.as_assignment_target().is_some_and(|target| {
            vue27_assignment_target_references_setup_local(target, setup_bindings, scope)
        }),
    }
}

pub(crate) fn vue27_simple_assignment_target_references_setup_local(
    target: &SimpleAssignmentTarget<'_>,
    setup_bindings: &BTreeSet<String>,
    scope: &mut BTreeSet<String>,
) -> bool {
    match target {
        SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) => {
            setup_bindings.contains(identifier.name.as_str())
                && !scope.contains(identifier.name.as_str())
        }
        SimpleAssignmentTarget::StaticMemberExpression(member) => {
            vue27_expression_references_setup_local_with_scope(
                &member.object,
                setup_bindings,
                scope,
            )
        }
        SimpleAssignmentTarget::ComputedMemberExpression(member) => {
            vue27_expression_references_setup_local_with_scope(
                &member.object,
                setup_bindings,
                scope,
            ) || vue27_expression_references_setup_local_with_scope(
                &member.expression,
                setup_bindings,
                scope,
            )
        }
        SimpleAssignmentTarget::PrivateFieldExpression(member) => {
            vue27_expression_references_setup_local_with_scope(
                &member.object,
                setup_bindings,
                scope,
            )
        }
        SimpleAssignmentTarget::TSAsExpression(expression) => {
            vue27_expression_references_setup_local_with_scope(
                &expression.expression,
                setup_bindings,
                scope,
            )
        }
        SimpleAssignmentTarget::TSSatisfiesExpression(expression) => {
            vue27_expression_references_setup_local_with_scope(
                &expression.expression,
                setup_bindings,
                scope,
            )
        }
        SimpleAssignmentTarget::TSNonNullExpression(expression) => {
            vue27_expression_references_setup_local_with_scope(
                &expression.expression,
                setup_bindings,
                scope,
            )
        }
        SimpleAssignmentTarget::TSTypeAssertion(expression) => {
            vue27_expression_references_setup_local_with_scope(
                &expression.expression,
                setup_bindings,
                scope,
            )
        }
    }
}
