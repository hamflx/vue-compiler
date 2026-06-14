#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Vue3TopLevelAwaitScopeEntry {
    pub(crate) expression_start: Option<usize>,
}

pub(crate) struct Vue3TopLevelAwaitRewriter<'a, 'source> {
    pub(crate) source: &'source str,
    pub(crate) edits: &'a mut SourceEdits<'source>,
    pub(crate) scopes: Vec<Vec<Vue3TopLevelAwaitScopeEntry>>,
    pub(crate) has_await: bool,
}

impl<'a, 'source> Vue3TopLevelAwaitRewriter<'a, 'source> {
    pub(crate) fn new(source: &'source str, edits: &'a mut SourceEdits<'source>) -> Self {
        Self {
            source,
            edits,
            scopes: Vec::new(),
            has_await: false,
        }
    }

    pub(crate) fn walk_program(&mut self, statements: &[Statement<'_>]) {
        self.push_statement_scope(statements);
        for statement in statements {
            if vue3_top_level_await_entry_statement(statement) {
                self.walk_statement(statement);
            }
        }
        self.pop_statement_scope();
    }

    pub(crate) fn walk_statement(&mut self, statement: &Statement<'_>) {
        match statement {
            Statement::BlockStatement(block) => {
                self.push_statement_scope(&block.body);
                for statement in &block.body {
                    self.walk_statement(statement);
                }
                self.pop_statement_scope();
            }
            Statement::ExpressionStatement(statement) => {
                self.walk_expression(&statement.expression, true);
            }
            Statement::VariableDeclaration(declaration) if !declaration.declare => {
                self.walk_variable_declaration(declaration);
            }
            Statement::IfStatement(statement) => {
                self.walk_expression(&statement.test, false);
                self.walk_statement(&statement.consequent);
                if let Some(alternate) = &statement.alternate {
                    self.walk_statement(alternate);
                }
            }
            Statement::ForStatement(statement) => {
                if let Some(init) = &statement.init {
                    match init {
                        ForStatementInit::VariableDeclaration(declaration) => {
                            self.walk_variable_declaration(declaration);
                        }
                        _ => {
                            if let Some(expression) = init.as_expression() {
                                self.walk_expression(expression, false);
                            }
                        }
                    }
                }
                if let Some(test) = &statement.test {
                    self.walk_expression(test, false);
                }
                if let Some(update) = &statement.update {
                    self.walk_expression(update, false);
                }
                self.walk_statement(&statement.body);
            }
            Statement::ForInStatement(statement) => {
                self.walk_for_statement_left(&statement.left);
                self.walk_expression(&statement.right, false);
                self.walk_statement(&statement.body);
            }
            Statement::ForOfStatement(statement) => {
                self.walk_for_statement_left(&statement.left);
                self.walk_expression(&statement.right, false);
                self.walk_statement(&statement.body);
            }
            Statement::WhileStatement(statement) => {
                self.walk_expression(&statement.test, false);
                self.walk_statement(&statement.body);
            }
            Statement::DoWhileStatement(statement) => {
                self.walk_statement(&statement.body);
                self.walk_expression(&statement.test, false);
            }
            Statement::SwitchStatement(statement) => {
                self.walk_expression(&statement.discriminant, false);
                for case in &statement.cases {
                    if let Some(test) = &case.test {
                        self.walk_expression(test, false);
                    }
                    self.push_statement_scope(&case.consequent);
                    for statement in &case.consequent {
                        self.walk_statement(statement);
                    }
                    self.pop_statement_scope();
                }
            }
            Statement::ThrowStatement(statement) => {
                self.walk_expression(&statement.argument, false);
            }
            Statement::TryStatement(statement) => {
                self.push_statement_scope(&statement.block.body);
                for statement in &statement.block.body {
                    self.walk_statement(statement);
                }
                self.pop_statement_scope();
                if let Some(handler) = &statement.handler {
                    self.push_statement_scope(&handler.body.body);
                    for statement in &handler.body.body {
                        self.walk_statement(statement);
                    }
                    self.pop_statement_scope();
                }
                if let Some(finalizer) = &statement.finalizer {
                    self.push_statement_scope(&finalizer.body);
                    for statement in &finalizer.body {
                        self.walk_statement(statement);
                    }
                    self.pop_statement_scope();
                }
            }
            Statement::LabeledStatement(statement) => {
                self.walk_statement(&statement.body);
            }
            Statement::ReturnStatement(statement) => {
                if let Some(argument) = &statement.argument {
                    self.walk_expression(argument, false);
                }
            }
            Statement::WithStatement(statement) => {
                self.walk_expression(&statement.object, false);
                self.walk_statement(&statement.body);
            }
            _ => {}
        }
    }

    pub(crate) fn walk_variable_declaration(&mut self, declaration: &VariableDeclaration<'_>) {
        if declaration.declare {
            return;
        }
        for declarator in &declaration.declarations {
            self.walk_binding_pattern(&declarator.id);
            if let Some(init) = &declarator.init {
                self.walk_expression(init, false);
            }
        }
    }

    pub(crate) fn walk_expression(
        &mut self,
        expression: &Expression<'_>,
        is_expression_statement: bool,
    ) {
        match expression {
            Expression::AwaitExpression(expression) => {
                self.process_await(expression, is_expression_statement);
                self.walk_expression(&expression.argument, false);
            }
            Expression::ArrayExpression(array) => {
                for element in &array.elements {
                    match element {
                        ArrayExpressionElement::SpreadElement(spread) => {
                            self.walk_expression(&spread.argument, false);
                        }
                        ArrayExpressionElement::Elision(_) => {}
                        element => {
                            if let Some(expression) = element.as_expression() {
                                self.walk_expression(expression, false);
                            }
                        }
                    }
                }
            }
            Expression::ObjectExpression(object) => {
                for property in &object.properties {
                    self.walk_object_property_kind(property);
                }
            }
            Expression::CallExpression(call) => {
                self.walk_expression(&call.callee, false);
                for argument in &call.arguments {
                    self.walk_argument(argument);
                }
            }
            Expression::NewExpression(expression) => {
                self.walk_expression(&expression.callee, false);
                for argument in &expression.arguments {
                    self.walk_argument(argument);
                }
            }
            Expression::StaticMemberExpression(member) => {
                self.walk_expression(&member.object, false);
            }
            Expression::ComputedMemberExpression(member) => {
                self.walk_expression(&member.object, false);
                self.walk_expression(&member.expression, false);
            }
            Expression::PrivateFieldExpression(member) => {
                self.walk_expression(&member.object, false);
            }
            Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_) => {}
            Expression::AssignmentExpression(assignment) => {
                self.walk_assignment_target(&assignment.left);
                self.walk_expression(&assignment.right, false);
            }
            Expression::UpdateExpression(update) => {
                self.walk_simple_assignment_target(&update.argument);
            }
            Expression::UnaryExpression(expression) => {
                self.walk_expression(&expression.argument, false);
            }
            Expression::BinaryExpression(expression) => {
                self.walk_expression(&expression.left, false);
                self.walk_expression(&expression.right, false);
            }
            Expression::PrivateInExpression(expression) => {
                self.walk_expression(&expression.right, false);
            }
            Expression::LogicalExpression(expression) => {
                self.walk_expression(&expression.left, false);
                self.walk_expression(&expression.right, false);
            }
            Expression::ConditionalExpression(expression) => {
                self.walk_expression(&expression.test, false);
                self.walk_expression(&expression.consequent, false);
                self.walk_expression(&expression.alternate, false);
            }
            Expression::SequenceExpression(expression) => {
                for expression in &expression.expressions {
                    self.walk_expression(expression, false);
                }
            }
            Expression::TemplateLiteral(expression) => {
                for expression in &expression.expressions {
                    self.walk_expression(expression, false);
                }
            }
            Expression::TaggedTemplateExpression(expression) => {
                self.walk_expression(&expression.tag, false);
                for expression in &expression.quasi.expressions {
                    self.walk_expression(expression, false);
                }
            }
            Expression::ParenthesizedExpression(expression) => {
                self.walk_expression(&expression.expression, false);
            }
            Expression::ClassExpression(class) => {
                self.walk_class(class);
            }
            Expression::ImportExpression(expression) => {
                self.walk_expression(&expression.source, false);
                if let Some(options) = &expression.options {
                    self.walk_expression(options, false);
                }
            }
            Expression::TSAsExpression(expression) => {
                self.walk_expression(&expression.expression, false);
            }
            Expression::TSSatisfiesExpression(expression) => {
                self.walk_expression(&expression.expression, false);
            }
            Expression::TSTypeAssertion(expression) => {
                self.walk_expression(&expression.expression, false);
            }
            Expression::TSNonNullExpression(expression) => {
                self.walk_expression(&expression.expression, false);
            }
            Expression::TSInstantiationExpression(expression) => {
                self.walk_expression(&expression.expression, false);
            }
            Expression::ChainExpression(chain) => match &chain.expression {
                oxc_ast::ast::ChainElement::CallExpression(call) => {
                    self.walk_expression(&call.callee, false);
                    for argument in &call.arguments {
                        self.walk_argument(argument);
                    }
                }
                oxc_ast::ast::ChainElement::TSNonNullExpression(expression) => {
                    self.walk_expression(&expression.expression, false);
                }
                oxc_ast::ast::ChainElement::StaticMemberExpression(member) => {
                    self.walk_expression(&member.object, false);
                }
                oxc_ast::ast::ChainElement::ComputedMemberExpression(member) => {
                    self.walk_expression(&member.object, false);
                    self.walk_expression(&member.expression, false);
                }
                oxc_ast::ast::ChainElement::PrivateFieldExpression(member) => {
                    self.walk_expression(&member.object, false);
                }
            },
            _ => {}
        }
    }

    pub(crate) fn walk_argument(&mut self, argument: &Argument<'_>) {
        match argument {
            Argument::SpreadElement(spread) => self.walk_expression(&spread.argument, false),
            _ => self.walk_expression(argument.to_expression(), false),
        }
    }

    pub(crate) fn walk_object_property_kind(&mut self, property: &ObjectPropertyKind<'_>) {
        match property {
            ObjectPropertyKind::ObjectProperty(property) => {
                if property.method {
                    return;
                }
                if property.computed {
                    self.walk_property_key(&property.key);
                }
                self.walk_expression(&property.value, false);
            }
            ObjectPropertyKind::SpreadProperty(spread) => {
                self.walk_expression(&spread.argument, false);
            }
        }
    }

    pub(crate) fn walk_property_key(&mut self, key: &PropertyKey<'_>) {
        match key {
            PropertyKey::StaticIdentifier(_) | PropertyKey::PrivateIdentifier(_) => {}
            _ => self.walk_expression(key.to_expression(), false),
        }
    }

    pub(crate) fn walk_class(&mut self, class: &oxc_ast::ast::Class<'_>) {
        if let Some(super_class) = &class.super_class {
            self.walk_expression(super_class, false);
        }
        for element in &class.body.body {
            match element {
                ClassElement::StaticBlock(block) => {
                    self.push_statement_scope(&block.body);
                    for statement in &block.body {
                        self.walk_statement(statement);
                    }
                    self.pop_statement_scope();
                }
                ClassElement::PropertyDefinition(property) => {
                    if property.computed {
                        self.walk_property_key(&property.key);
                    }
                    if let Some(value) = &property.value {
                        self.walk_expression(value, false);
                    }
                }
                ClassElement::AccessorProperty(property) => {
                    if property.computed {
                        self.walk_property_key(&property.key);
                    }
                }
                ClassElement::MethodDefinition(_) | ClassElement::TSIndexSignature(_) => {}
            }
        }
    }

    pub(crate) fn walk_for_statement_left(&mut self, left: &ForStatementLeft<'_>) {
        match left {
            ForStatementLeft::VariableDeclaration(declaration) => {
                self.walk_variable_declaration(declaration);
            }
            _ => {
                if let Some(target) = left.as_assignment_target() {
                    self.walk_assignment_target(target);
                }
            }
        }
    }

    pub(crate) fn walk_assignment_target(&mut self, target: &AssignmentTarget<'_>) {
        match target {
            AssignmentTarget::StaticMemberExpression(member) => {
                self.walk_expression(&member.object, false);
            }
            AssignmentTarget::ComputedMemberExpression(member) => {
                self.walk_expression(&member.object, false);
                self.walk_expression(&member.expression, false);
            }
            AssignmentTarget::PrivateFieldExpression(member) => {
                self.walk_expression(&member.object, false);
            }
            AssignmentTarget::TSAsExpression(expression) => {
                self.walk_expression(&expression.expression, false);
            }
            AssignmentTarget::TSSatisfiesExpression(expression) => {
                self.walk_expression(&expression.expression, false);
            }
            AssignmentTarget::TSNonNullExpression(expression) => {
                self.walk_expression(&expression.expression, false);
            }
            AssignmentTarget::TSTypeAssertion(expression) => {
                self.walk_expression(&expression.expression, false);
            }
            AssignmentTarget::ArrayAssignmentTarget(target) => {
                for element in target.elements.iter().flatten() {
                    self.walk_assignment_target_maybe_default(element);
                }
                if let Some(rest) = &target.rest {
                    self.walk_assignment_target(&rest.target);
                }
            }
            AssignmentTarget::ObjectAssignmentTarget(target) => {
                for property in &target.properties {
                    self.walk_assignment_target_property(property);
                }
                if let Some(rest) = &target.rest {
                    self.walk_assignment_target(&rest.target);
                }
            }
            AssignmentTarget::AssignmentTargetIdentifier(_) => {}
        }
    }

    pub(crate) fn walk_simple_assignment_target(&mut self, target: &SimpleAssignmentTarget<'_>) {
        match target {
            SimpleAssignmentTarget::StaticMemberExpression(member) => {
                self.walk_expression(&member.object, false);
            }
            SimpleAssignmentTarget::ComputedMemberExpression(member) => {
                self.walk_expression(&member.object, false);
                self.walk_expression(&member.expression, false);
            }
            SimpleAssignmentTarget::PrivateFieldExpression(member) => {
                self.walk_expression(&member.object, false);
            }
            SimpleAssignmentTarget::TSAsExpression(expression) => {
                self.walk_expression(&expression.expression, false);
            }
            SimpleAssignmentTarget::TSSatisfiesExpression(expression) => {
                self.walk_expression(&expression.expression, false);
            }
            SimpleAssignmentTarget::TSNonNullExpression(expression) => {
                self.walk_expression(&expression.expression, false);
            }
            SimpleAssignmentTarget::TSTypeAssertion(expression) => {
                self.walk_expression(&expression.expression, false);
            }
            SimpleAssignmentTarget::AssignmentTargetIdentifier(_) => {}
        }
    }

    pub(crate) fn walk_assignment_target_maybe_default(
        &mut self,
        target: &oxc_ast::ast::AssignmentTargetMaybeDefault<'_>,
    ) {
        match target {
            oxc_ast::ast::AssignmentTargetMaybeDefault::AssignmentTargetWithDefault(target) => {
                self.walk_assignment_target(&target.binding);
                self.walk_expression(&target.init, false);
            }
            _ => {
                if let Some(target) = target.as_assignment_target() {
                    self.walk_assignment_target(target);
                }
            }
        }
    }

    pub(crate) fn walk_assignment_target_property(
        &mut self,
        property: &oxc_ast::ast::AssignmentTargetProperty<'_>,
    ) {
        match property {
            oxc_ast::ast::AssignmentTargetProperty::AssignmentTargetPropertyIdentifier(
                property,
            ) => {
                if let Some(init) = &property.init {
                    self.walk_expression(init, false);
                }
            }
            oxc_ast::ast::AssignmentTargetProperty::AssignmentTargetPropertyProperty(property) => {
                if property.computed {
                    self.walk_property_key(&property.name);
                }
                self.walk_assignment_target_maybe_default(&property.binding);
            }
        }
    }

    pub(crate) fn walk_binding_pattern(&mut self, pattern: &BindingPattern<'_>) {
        match pattern {
            BindingPattern::BindingIdentifier(_) => {}
            BindingPattern::ObjectPattern(pattern) => {
                for property in &pattern.properties {
                    if property.computed {
                        self.walk_property_key(&property.key);
                    }
                    self.walk_binding_pattern(&property.value);
                }
                if let Some(rest) = &pattern.rest {
                    self.walk_binding_pattern(&rest.argument);
                }
            }
            BindingPattern::ArrayPattern(pattern) => {
                for element in pattern.elements.iter().flatten() {
                    self.walk_binding_pattern(element);
                }
                if let Some(rest) = &pattern.rest {
                    self.walk_binding_pattern(&rest.argument);
                }
            }
            BindingPattern::AssignmentPattern(pattern) => {
                self.walk_binding_pattern(&pattern.left);
                self.walk_expression(&pattern.right, false);
            }
        }
    }

    pub(crate) fn process_await(
        &mut self,
        expression: &oxc_ast::ast::AwaitExpression<'_>,
        is_expression_statement: bool,
    ) {
        self.has_await = true;
        let await_start = expression.span.start as usize;
        let await_end = expression.span.end as usize;
        let argument_start = expression.argument.span().start as usize;
        let argument_end = expression.argument.span().end as usize;
        if await_start > argument_start || argument_end > self.source.len() {
            return;
        }
        let contains_nested_await = self
            .source
            .get(argument_start..argument_end)
            .is_some_and(contains_js_await_word);
        let semi = if self.needs_semicolon(await_start) {
            ";"
        } else {
            ""
        };
        let async_prefix = if contains_nested_await { "async " } else { "" };
        self.edits.overwrite(
            await_start,
            argument_start,
            format!("{semi}(\n  ([__temp,__restore] = _withAsyncContext({async_prefix}() => "),
        );
        let assignment = if is_expression_statement {
            ""
        } else {
            "__temp = "
        };
        let tail = if is_expression_statement {
            String::new()
        } else {
            ",\n  __temp".to_string()
        };
        self.edits.append_left(
            await_end,
            format!(")),\n  {assignment}await __temp,\n  __restore(){tail}\n)"),
        );
    }

    pub(crate) fn needs_semicolon(&self, await_start: usize) -> bool {
        let is_root_scope = self.scopes.len() == 1;
        self.scopes.last().is_some_and(|scope| {
            scope.iter().enumerate().any(|(index, entry)| {
                entry.expression_start == Some(await_start) && (is_root_scope || index > 0)
            })
        })
    }

    pub(crate) fn push_statement_scope(&mut self, statements: &[Statement<'_>]) {
        self.scopes.push(
            statements
                .iter()
                .map(|statement| Vue3TopLevelAwaitScopeEntry {
                    expression_start: match statement {
                        Statement::ExpressionStatement(statement) => {
                            Some(statement.expression.span().start as usize)
                        }
                        _ => None,
                    },
                })
                .collect(),
        );
    }

    pub(crate) fn pop_statement_scope(&mut self) {
        self.scopes.pop();
    }
}
