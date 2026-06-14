pub(crate) struct Vue3PropsDestructureRewriter<'a, 'source> {
    pub(crate) props_destructured_bindings: &'a BTreeMap<String, String>,
    pub(crate) vue_import_aliases: &'a BTreeMap<String, String>,
    pub(crate) edits: &'a mut SourceEdits<'source>,
    pub(crate) scopes: Vec<BTreeMap<String, bool>>,
    pub(crate) errors: Vec<String>,
}

impl<'a, 'source> Vue3PropsDestructureRewriter<'a, 'source> {
    pub(crate) fn new(
        props_destructured_bindings: &'a BTreeMap<String, String>,
        vue_import_aliases: &'a BTreeMap<String, String>,
        edits: &'a mut SourceEdits<'source>,
    ) -> Self {
        let root_scope = props_destructured_bindings
            .keys()
            .map(|local| (local.clone(), true))
            .collect::<BTreeMap<_, _>>();
        Self {
            props_destructured_bindings,
            vue_import_aliases,
            edits,
            scopes: vec![root_scope],
            errors: Vec::new(),
        }
    }

    pub(crate) fn walk_program(&mut self, statements: &[Statement<'_>]) {
        self.mark_block_declarations(statements, true);
        for statement in statements {
            self.walk_statement(statement, true);
        }
    }

    pub(crate) fn walk_statement(&mut self, statement: &Statement<'_>, is_root: bool) {
        match statement {
            Statement::BlockStatement(block) => {
                self.push_scope();
                self.mark_block_declarations(&block.body, false);
                for statement in &block.body {
                    self.walk_statement(statement, false);
                }
                self.pop_scope();
            }
            Statement::ExpressionStatement(statement) => {
                self.walk_expression(&statement.expression);
            }
            Statement::ReturnStatement(statement) => {
                if let Some(argument) = &statement.argument {
                    self.walk_expression(argument);
                }
            }
            Statement::VariableDeclaration(declaration) => {
                self.mark_variable_declaration(declaration, is_root);
                for declarator in &declaration.declarations {
                    if let Some(init) = &declarator.init {
                        self.walk_expression(init);
                    }
                }
            }
            Statement::FunctionDeclaration(function) => self.walk_function(function),
            Statement::IfStatement(statement) => {
                self.walk_expression(&statement.test);
                self.walk_statement(&statement.consequent, false);
                if let Some(alternate) = &statement.alternate {
                    self.walk_statement(alternate, false);
                }
            }
            Statement::ForStatement(statement) => {
                self.push_scope();
                if let Some(init) = &statement.init {
                    match init {
                        oxc_ast::ast::ForStatementInit::VariableDeclaration(declaration) => {
                            self.mark_variable_declaration(declaration, false);
                            for declarator in &declaration.declarations {
                                if let Some(init) = &declarator.init {
                                    self.walk_expression(init);
                                }
                            }
                        }
                        _ => {
                            if let Some(expression) = init.as_expression() {
                                self.walk_expression(expression);
                            }
                        }
                    }
                }
                if let Some(test) = &statement.test {
                    self.walk_expression(test);
                }
                if let Some(update) = &statement.update {
                    self.walk_expression(update);
                }
                self.walk_statement(&statement.body, false);
                self.pop_scope();
            }
            Statement::ForInStatement(statement) => {
                self.push_scope();
                self.mark_for_iteration_left(&statement.left);
                self.walk_expression(&statement.right);
                self.walk_statement(&statement.body, false);
                self.pop_scope();
            }
            Statement::ForOfStatement(statement) => {
                self.push_scope();
                self.mark_for_iteration_left(&statement.left);
                self.walk_expression(&statement.right);
                self.walk_statement(&statement.body, false);
                self.pop_scope();
            }
            Statement::WhileStatement(statement) => {
                self.walk_expression(&statement.test);
                self.walk_statement(&statement.body, false);
            }
            Statement::DoWhileStatement(statement) => {
                self.walk_statement(&statement.body, false);
                self.walk_expression(&statement.test);
            }
            Statement::SwitchStatement(statement) => {
                self.walk_expression(&statement.discriminant);
                for case in &statement.cases {
                    if let Some(test) = &case.test {
                        self.walk_expression(test);
                    }
                    self.push_scope();
                    self.mark_block_declarations(&case.consequent, false);
                    for statement in &case.consequent {
                        self.walk_statement(statement, false);
                    }
                    self.pop_scope();
                }
            }
            Statement::ThrowStatement(statement) => {
                self.walk_expression(&statement.argument);
            }
            Statement::TryStatement(statement) => {
                self.push_scope();
                self.mark_block_declarations(&statement.block.body, false);
                for statement in &statement.block.body {
                    self.walk_statement(statement, false);
                }
                self.pop_scope();
                if let Some(handler) = &statement.handler {
                    self.push_scope();
                    if let Some(param) = &handler.param {
                        self.mark_binding_pattern(&param.pattern);
                    }
                    self.mark_block_declarations(&handler.body.body, false);
                    for statement in &handler.body.body {
                        self.walk_statement(statement, false);
                    }
                    self.pop_scope();
                }
                if let Some(finalizer) = &statement.finalizer {
                    self.push_scope();
                    self.mark_block_declarations(&finalizer.body, false);
                    for statement in &finalizer.body {
                        self.walk_statement(statement, false);
                    }
                    self.pop_scope();
                }
            }
            Statement::LabeledStatement(statement) => {
                self.walk_statement(&statement.body, false);
            }
            _ => {}
        }
    }

    pub(crate) fn walk_expression(&mut self, expression: &Expression<'_>) {
        match expression {
            Expression::Identifier(identifier) => {
                self.rewrite_identifier_reference(
                    identifier.name.as_str(),
                    identifier.span.start as usize,
                    identifier.span.end as usize,
                );
            }
            Expression::ArrayExpression(array) => {
                for element in &array.elements {
                    match element {
                        oxc_ast::ast::ArrayExpressionElement::SpreadElement(spread) => {
                            self.walk_expression(&spread.argument);
                        }
                        oxc_ast::ast::ArrayExpressionElement::Elision(_) => {}
                        element => {
                            if let Some(expression) = element.as_expression() {
                                self.walk_expression(expression);
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
                self.check_call_usage(call);
                self.walk_expression(&call.callee);
                for argument in &call.arguments {
                    self.walk_argument(argument);
                }
            }
            Expression::NewExpression(expression) => {
                self.walk_expression(&expression.callee);
                for argument in &expression.arguments {
                    self.walk_argument(argument);
                }
            }
            Expression::StaticMemberExpression(member) => {
                self.walk_expression(&member.object);
            }
            Expression::ComputedMemberExpression(member) => {
                self.walk_expression(&member.object);
                self.walk_expression(&member.expression);
            }
            Expression::PrivateFieldExpression(member) => {
                self.walk_expression(&member.object);
            }
            Expression::FunctionExpression(function) => self.walk_function(function),
            Expression::ArrowFunctionExpression(function) => self.walk_arrow_function(function),
            Expression::AssignmentExpression(assignment) => {
                self.check_assignment_target(&assignment.left);
                self.walk_assignment_target(&assignment.left);
                self.walk_expression(&assignment.right);
            }
            Expression::UpdateExpression(update) => {
                self.check_simple_assignment_target(&update.argument);
                self.walk_simple_assignment_target(&update.argument);
            }
            Expression::UnaryExpression(expression) => self.walk_expression(&expression.argument),
            Expression::AwaitExpression(expression) => self.walk_expression(&expression.argument),
            Expression::BinaryExpression(expression) => {
                self.walk_expression(&expression.left);
                self.walk_expression(&expression.right);
            }
            Expression::PrivateInExpression(expression) => {
                self.walk_expression(&expression.right);
            }
            Expression::LogicalExpression(expression) => {
                self.walk_expression(&expression.left);
                self.walk_expression(&expression.right);
            }
            Expression::ConditionalExpression(expression) => {
                self.walk_expression(&expression.test);
                self.walk_expression(&expression.consequent);
                self.walk_expression(&expression.alternate);
            }
            Expression::SequenceExpression(expression) => {
                for expression in &expression.expressions {
                    self.walk_expression(expression);
                }
            }
            Expression::TemplateLiteral(expression) => {
                for expression in &expression.expressions {
                    self.walk_expression(expression);
                }
            }
            Expression::TaggedTemplateExpression(expression) => {
                self.walk_expression(&expression.tag);
                for expression in &expression.quasi.expressions {
                    self.walk_expression(expression);
                }
            }
            Expression::ParenthesizedExpression(expression) => {
                self.walk_expression(&expression.expression);
            }
            Expression::TSAsExpression(expression) => self.walk_expression(&expression.expression),
            Expression::TSSatisfiesExpression(expression) => {
                self.walk_expression(&expression.expression);
            }
            Expression::TSTypeAssertion(expression) => {
                self.walk_expression(&expression.expression);
            }
            Expression::TSNonNullExpression(expression) => {
                self.walk_expression(&expression.expression);
            }
            Expression::TSInstantiationExpression(expression) => {
                self.walk_expression(&expression.expression);
            }
            Expression::ChainExpression(chain) => match &chain.expression {
                oxc_ast::ast::ChainElement::CallExpression(call) => {
                    self.check_call_usage(call);
                    self.walk_expression(&call.callee);
                    for argument in &call.arguments {
                        self.walk_argument(argument);
                    }
                }
                oxc_ast::ast::ChainElement::TSNonNullExpression(expression) => {
                    self.walk_expression(&expression.expression);
                }
                oxc_ast::ast::ChainElement::StaticMemberExpression(member) => {
                    self.walk_expression(&member.object);
                }
                oxc_ast::ast::ChainElement::ComputedMemberExpression(member) => {
                    self.walk_expression(&member.object);
                    self.walk_expression(&member.expression);
                }
                oxc_ast::ast::ChainElement::PrivateFieldExpression(member) => {
                    self.walk_expression(&member.object);
                }
            },
            _ => {}
        }
    }

    pub(crate) fn walk_argument(&mut self, argument: &Argument<'_>) {
        match argument {
            Argument::SpreadElement(spread) => self.walk_expression(&spread.argument),
            _ => self.walk_expression(argument.to_expression()),
        }
    }

    pub(crate) fn walk_object_property_kind(&mut self, property: &ObjectPropertyKind<'_>) {
        match property {
            ObjectPropertyKind::ObjectProperty(property) => {
                if property.computed {
                    self.walk_property_key(&property.key);
                }
                if property.shorthand {
                    if let Expression::Identifier(identifier) = &property.value {
                        if let Some(public_name) =
                            self.active_prop_public_name(identifier.name.as_str())
                        {
                            self.edits.append_left(
                                identifier.span.end as usize,
                                format!(": {}", vue3_props_access_exp(public_name)),
                            );
                            return;
                        }
                    }
                }
                self.walk_expression(&property.value);
            }
            ObjectPropertyKind::SpreadProperty(spread) => {
                self.walk_expression(&spread.argument);
            }
        }
    }

    pub(crate) fn walk_property_key(&mut self, key: &PropertyKey<'_>) {
        match key {
            PropertyKey::StaticIdentifier(_) | PropertyKey::PrivateIdentifier(_) => {}
            _ => self.walk_expression(key.to_expression()),
        }
    }

    pub(crate) fn walk_function(&mut self, function: &Function<'_>) {
        self.push_scope();
        if let Some(id) = &function.id {
            self.mark_local(id.name.as_str());
        }
        for param in &function.params.items {
            self.mark_binding_pattern(&param.pattern);
            if let Some(initializer) = &param.initializer {
                self.walk_expression(initializer);
            }
        }
        if let Some(rest) = &function.params.rest {
            self.mark_binding_pattern(&rest.rest.argument);
        }
        if let Some(body) = &function.body {
            self.mark_block_declarations(&body.statements, false);
            for statement in &body.statements {
                self.walk_statement(statement, false);
            }
        }
        self.pop_scope();
    }

    pub(crate) fn walk_arrow_function(&mut self, function: &ArrowFunctionExpression<'_>) {
        self.push_scope();
        for param in &function.params.items {
            self.mark_binding_pattern(&param.pattern);
            if let Some(initializer) = &param.initializer {
                self.walk_expression(initializer);
            }
        }
        if let Some(rest) = &function.params.rest {
            self.mark_binding_pattern(&rest.rest.argument);
        }
        self.mark_block_declarations(&function.body.statements, false);
        for statement in &function.body.statements {
            self.walk_statement(statement, false);
        }
        self.pop_scope();
    }

    pub(crate) fn walk_assignment_target(&mut self, target: &AssignmentTarget<'_>) {
        match target {
            AssignmentTarget::AssignmentTargetIdentifier(_) => {}
            AssignmentTarget::StaticMemberExpression(member) => {
                self.walk_expression(&member.object);
            }
            AssignmentTarget::ComputedMemberExpression(member) => {
                self.walk_expression(&member.object);
                self.walk_expression(&member.expression);
            }
            AssignmentTarget::PrivateFieldExpression(member) => {
                self.walk_expression(&member.object);
            }
            _ => {}
        }
    }

    pub(crate) fn walk_simple_assignment_target(&mut self, target: &SimpleAssignmentTarget<'_>) {
        match target {
            SimpleAssignmentTarget::AssignmentTargetIdentifier(_) => {}
            SimpleAssignmentTarget::StaticMemberExpression(member) => {
                self.walk_expression(&member.object);
            }
            SimpleAssignmentTarget::ComputedMemberExpression(member) => {
                self.walk_expression(&member.object);
                self.walk_expression(&member.expression);
            }
            SimpleAssignmentTarget::PrivateFieldExpression(member) => {
                self.walk_expression(&member.object);
            }
            _ => {}
        }
    }

    pub(crate) fn check_call_usage(&mut self, call: &oxc_ast::ast::CallExpression<'_>) {
        for method in ["watch", "toRef"] {
            if !self.is_call_named_or_alias(call, method) {
                continue;
            }
            let Some(argument) = call
                .arguments
                .first()
                .and_then(vue3_call_argument_expression)
                .map(unwrap_vue3_ts_expression)
            else {
                continue;
            };
            let Expression::Identifier(identifier) = argument else {
                continue;
            };
            if self.is_active_prop_binding(identifier.name.as_str()) {
                self.errors.push(format!(
                    "\"{}\" is a destructured prop and should not be passed directly to {}(). Pass a getter () => {} instead.",
                    identifier.name, method, identifier.name
                ));
            }
        }
    }

    pub(crate) fn is_call_named_or_alias(
        &self,
        call: &oxc_ast::ast::CallExpression<'_>,
        method: &str,
    ) -> bool {
        let expected = self
            .vue_import_aliases
            .get(method)
            .map(String::as_str)
            .unwrap_or(method);
        matches!(&call.callee, Expression::Identifier(identifier) if identifier.name == expected)
    }

    pub(crate) fn check_assignment_target(&mut self, target: &AssignmentTarget<'_>) {
        if let AssignmentTarget::AssignmentTargetIdentifier(identifier) = target {
            if self.is_active_prop_binding(identifier.name.as_str()) {
                self.errors
                    .push("Cannot assign to destructured props as they are readonly.".into());
            }
        }
    }

    pub(crate) fn check_simple_assignment_target(&mut self, target: &SimpleAssignmentTarget<'_>) {
        if let SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) = target {
            if self.is_active_prop_binding(identifier.name.as_str()) {
                self.errors
                    .push("Cannot assign to destructured props as they are readonly.".into());
            }
        }
    }

    pub(crate) fn mark_block_declarations(&mut self, statements: &[Statement<'_>], is_root: bool) {
        for statement in statements {
            match statement {
                Statement::VariableDeclaration(declaration) if !declaration.declare => {
                    self.mark_variable_declaration(declaration, is_root);
                }
                Statement::FunctionDeclaration(function) if !function.declare => {
                    if let Some(id) = &function.id {
                        self.mark_local(id.name.as_str());
                    }
                }
                Statement::ClassDeclaration(class) if !class.declare => {
                    if let Some(id) = &class.id {
                        self.mark_local(id.name.as_str());
                    }
                }
                _ => {}
            }
        }
    }

    pub(crate) fn mark_variable_declaration(
        &mut self,
        declaration: &VariableDeclaration<'_>,
        is_root: bool,
    ) {
        if declaration.declare {
            return;
        }
        for declarator in &declaration.declarations {
            if is_root
                && declarator
                    .init
                    .as_ref()
                    .is_some_and(vue3_is_define_props_call)
            {
                continue;
            }
            self.mark_binding_pattern(&declarator.id);
        }
    }

    pub(crate) fn mark_for_iteration_left(&mut self, left: &oxc_ast::ast::ForStatementLeft<'_>) {
        match left {
            oxc_ast::ast::ForStatementLeft::VariableDeclaration(declaration) => {
                self.mark_variable_declaration(declaration, false);
            }
            _ => {
                if let Some(target) = left.as_assignment_target() {
                    self.mark_assignment_target_as_local(target);
                }
            }
        }
    }

    pub(crate) fn mark_assignment_target_as_local(&mut self, target: &AssignmentTarget<'_>) {
        if let AssignmentTarget::AssignmentTargetIdentifier(identifier) = target {
            self.mark_local(identifier.name.as_str());
        }
    }

    pub(crate) fn mark_binding_pattern(&mut self, pattern: &BindingPattern<'_>) {
        match pattern {
            BindingPattern::BindingIdentifier(identifier) => {
                self.mark_local(identifier.name.as_str());
            }
            BindingPattern::ObjectPattern(pattern) => {
                for property in &pattern.properties {
                    self.mark_binding_pattern(&property.value);
                }
                if let Some(rest) = &pattern.rest {
                    self.mark_binding_pattern(&rest.argument);
                }
            }
            BindingPattern::ArrayPattern(pattern) => {
                for element in pattern.elements.iter().flatten() {
                    self.mark_binding_pattern(element);
                }
                if let Some(rest) = &pattern.rest {
                    self.mark_binding_pattern(&rest.argument);
                }
            }
            BindingPattern::AssignmentPattern(pattern) => {
                self.mark_binding_pattern(&pattern.left);
                self.walk_expression(&pattern.right);
            }
        }
    }

    pub(crate) fn push_scope(&mut self) {
        self.scopes.push(BTreeMap::new());
    }

    pub(crate) fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    pub(crate) fn mark_local(&mut self, name: &str) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string(), false);
        }
    }

    pub(crate) fn is_active_prop_binding(&self, name: &str) -> bool {
        self.active_prop_public_name(name).is_some()
    }

    pub(crate) fn active_prop_public_name(&self, name: &str) -> Option<&str> {
        let is_active = self
            .scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name))
            .copied()
            .unwrap_or(false);
        if !is_active {
            return None;
        }
        self.props_destructured_bindings
            .get(name)
            .map(String::as_str)
    }

    pub(crate) fn rewrite_identifier_reference(&mut self, name: &str, start: usize, end: usize) {
        let Some(public_name) = self.active_prop_public_name(name) else {
            return;
        };
        self.edits
            .overwrite(start, end, vue3_props_access_exp(public_name));
    }
}
