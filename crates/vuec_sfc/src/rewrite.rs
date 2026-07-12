use crate::*;

pub(crate) fn rewrite_vue27_default(
    input: &str,
    variable: &str,
    options: Vue27RewriteDefaultOptions,
) -> String {
    let allocator = oxc_allocator::Allocator::default();
    let source_type = if options.typescript {
        oxc_span::SourceType::ts()
    } else {
        oxc_span::SourceType::mjs()
    };
    let parsed = oxc_parser::Parser::new(&allocator, input, source_type)
        .with_options(oxc_parser::ParseOptions {
            parse_regular_expression: true,
            ..oxc_parser::ParseOptions::default()
        })
        .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        if !options.typescript {
            let ts_parsed = oxc_parser::Parser::new(&allocator, input, oxc_span::SourceType::ts())
                .with_options(oxc_parser::ParseOptions {
                    parse_regular_expression: true,
                    ..oxc_parser::ParseOptions::default()
                })
                .parse();
            if !ts_parsed.panicked && ts_parsed.errors.is_empty() {
                return rewrite_vue27_default_from_program(
                    input,
                    variable,
                    &ts_parsed.program.body,
                );
            }
        }
        return rewrite_vue27_default_lexical(input, variable);
    }

    rewrite_vue27_default_from_program(input, variable, &parsed.program.body)
}

pub(crate) fn rewrite_vue3_default(
    input: &str,
    variable: &str,
    options: Vue3RewriteDefaultOptions,
) -> Result<String, String> {
    let allocator = oxc_allocator::Allocator::default();
    let source_type = if options.typescript {
        oxc_span::SourceType::ts()
    } else {
        oxc_span::SourceType::mjs()
    };
    let parsed = oxc_parser::Parser::new(&allocator, input, source_type)
        .with_options(oxc_parser::ParseOptions {
            parse_regular_expression: true,
            ..oxc_parser::ParseOptions::default()
        })
        .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return Err(parsed
            .errors
            .first()
            .map(ToString::to_string)
            .unwrap_or_else(|| "failed to parse default export".into()));
    }
    if !options.typescript {
        if let Some(offset) = vue3_typescript_default_export_start(&parsed.program.body) {
            let (line, column) = line_column(input, offset);
            return Err(format!(
                "Unexpected reserved word 'interface'. ({line}:{column})"
            ));
        }
    }

    Ok(rewrite_vue3_default_from_program(
        input,
        variable,
        &parsed.program.body,
    ))
}

pub(crate) fn rewrite_vue27_default_from_program(
    input: &str,
    variable: &str,
    body: &[Statement<'_>],
) -> String {
    let mut edits = SourceEdits::new(input);
    let mut found_default = false;
    for statement in body {
        match statement {
            Statement::ExportDefaultDeclaration(declaration) => {
                found_default = true;
                rewrite_export_default(input, variable, declaration, &mut edits);
            }
            Statement::ExportNamedDeclaration(declaration)
                if rewrite_named_default_exports(input, variable, declaration, &mut edits) =>
            {
                found_default = true;
            }
            _ => {}
        }
    }
    if !found_default {
        edits.append(format!("\nconst {variable} = {{}}"));
    }
    edits.apply()
}

pub(crate) fn rewrite_vue3_default_from_program(
    input: &str,
    variable: &str,
    body: &[Statement<'_>],
) -> String {
    let mut edits = SourceEdits::new(input);
    let mut found_default = false;
    for statement in body {
        match statement {
            Statement::ExportDefaultDeclaration(declaration) => {
                found_default = true;
                rewrite_vue3_export_default(variable, declaration, &mut edits);
            }
            Statement::ExportNamedDeclaration(declaration)
                if rewrite_vue3_named_default_exports(input, variable, declaration, &mut edits) =>
            {
                found_default = true;
            }
            _ => {}
        }
    }
    if !found_default {
        edits.append(format!("\nconst {variable} = {{}}"));
    }
    edits.apply()
}

pub(crate) fn vue3_typescript_default_export_start(body: &[Statement<'_>]) -> Option<usize> {
    body.iter().find_map(|statement| match statement {
        Statement::ExportDefaultDeclaration(declaration) => match &declaration.declaration {
            ExportDefaultDeclarationKind::TSInterfaceDeclaration(declaration) => {
                Some(declaration.span.start as usize)
            }
            _ => None,
        },
        _ => None,
    })
}

pub(crate) fn line_column(input: &str, offset: usize) -> (usize, usize) {
    let mut line = 1usize;
    let mut column = 0usize;
    for ch in input[..offset.min(input.len())].chars() {
        if ch == '\n' {
            line += 1;
            column = 0;
        } else {
            column += 1;
        }
    }
    (line, column)
}

pub(crate) fn rewrite_export_default(
    input: &str,
    variable: &str,
    declaration: &ExportDefaultDeclaration<'_>,
    edits: &mut SourceEdits,
) {
    match &declaration.declaration {
        ExportDefaultDeclarationKind::ClassDeclaration(class) => {
            if let Some(id) = &class.id {
                let fast_candidate = source_with_overwrite(
                    input,
                    declaration.span.start as usize,
                    id.span.start as usize,
                    "class ",
                );
                if has_vue27_default_export_like(input)
                    && has_vue27_default_export_like(&fast_candidate)
                {
                    let replace_start = class
                        .decorators
                        .last()
                        .map(|decorator| decorator.span.end as usize)
                        .unwrap_or(declaration.span.start as usize);
                    edits.overwrite(replace_start, id.span.start as usize, " class ");
                } else {
                    edits.overwrite(
                        declaration.span.start as usize,
                        id.span.start as usize,
                        "class ",
                    );
                }
                edits.append(format!("\nconst {variable} = {}", id.name));
                return;
            }
        }
        ExportDefaultDeclarationKind::FunctionDeclaration(function) => {
            if let Some(id) = &function.id {
                edits.overwrite(
                    declaration.span.start as usize,
                    function.span.start as usize,
                    "",
                );
                edits.append(format!("\nconst {variable} = {}", id.name));
                return;
            }
        }
        _ => {}
    }

    edits.overwrite(
        declaration.span.start as usize,
        export_default_declaration_value_start(input, declaration),
        format!("const {variable} ="),
    );
}

pub(crate) fn rewrite_vue3_export_default(
    variable: &str,
    declaration: &ExportDefaultDeclaration<'_>,
    edits: &mut SourceEdits,
) {
    if let ExportDefaultDeclarationKind::ClassDeclaration(class) = &declaration.declaration {
        if let Some(id) = &class.id {
            let replace_start = class
                .decorators
                .last()
                .map(|decorator| decorator.span.end as usize)
                .unwrap_or(declaration.span.start as usize);
            edits.overwrite(replace_start, id.span.start as usize, " class ");
            edits.append(format!("\nconst {variable} = {}", id.name));
            return;
        }
    }

    edits.overwrite(
        declaration.span.start as usize,
        declaration.declaration.span().start as usize,
        format!("const {variable} = "),
    );
}

pub(crate) fn export_default_declaration_value_start(
    input: &str,
    declaration: &ExportDefaultDeclaration<'_>,
) -> usize {
    let start = declaration.span.start as usize;
    let end = declaration.declaration.span().start as usize;
    let segment = &input[start..end.min(input.len())];
    segment
        .find("default")
        .map(|offset| start + offset + "default".len())
        .unwrap_or(end)
}

pub(crate) fn rewrite_named_default_exports(
    input: &str,
    variable: &str,
    declaration: &ExportNamedDeclaration<'_>,
    edits: &mut SourceEdits,
) -> bool {
    let mut found = false;
    for specifier in &declaration.specifiers {
        if module_export_name(specifier.exported()) != Some("default") {
            continue;
        }
        found = true;
        let local_name = module_export_name(specifier.local()).unwrap_or("default");
        if let Some(source) = declaration.source.as_ref() {
            let source_value = source.value.to_string();
            if local_name == "default" {
                let end = specifier_end(
                    input,
                    specifier.local().span().end as usize,
                    declaration.span.end as usize,
                );
                edits.prepend(format!(
                    "import {{ default as __VUE_DEFAULT__ }} from '{}'\n",
                    source_value
                ));
                edits.overwrite(specifier.span.start as usize, end, "");
                edits.append(format!("\nconst {variable} = __VUE_DEFAULT__"));
            } else {
                let end = specifier_end(
                    input,
                    specifier.exported().span().end as usize,
                    declaration.span.end as usize,
                );
                edits.prepend(format!("import {{ {local_name} }} from '{source_value}'\n"));
                edits.overwrite(specifier.span.start as usize, end, "");
                edits.append(format!("\nconst {variable} = {local_name}"));
            }
        } else {
            let end = specifier_end(
                input,
                specifier.span.end as usize,
                declaration.span.end as usize,
            );
            edits.overwrite(specifier.span.start as usize, end, "");
            edits.append(format!("\nconst {variable} = {local_name}"));
        }
    }
    found
}

pub(crate) fn rewrite_vue3_named_default_exports(
    input: &str,
    variable: &str,
    declaration: &ExportNamedDeclaration<'_>,
    edits: &mut SourceEdits,
) -> bool {
    let mut found = false;
    for specifier in &declaration.specifiers {
        if module_export_name(specifier.exported()) != Some("default") {
            continue;
        }
        found = true;
        let local_name = module_export_name(specifier.local()).unwrap_or("default");
        if let Some(source) = declaration.source.as_ref() {
            let source_value = source.value.to_string();
            if local_name == "default" {
                let end = specifier_end(
                    input,
                    specifier.local().span().end as usize,
                    declaration.span.end as usize,
                );
                edits.prepend(format!(
                    "import {{ default as __VUE_DEFAULT__ }} from '{}'\n",
                    source_value
                ));
                edits.remove(specifier.span.start as usize, end);
                edits.append(format!("\nconst {variable} = __VUE_DEFAULT__"));
            } else {
                let end = specifier_end(
                    input,
                    specifier.exported().span().end as usize,
                    declaration.span.end as usize,
                );
                let local_source = &input[specifier.local().span().start as usize
                    ..specifier.local().span().end as usize];
                edits.prepend(format!(
                    "import {{ {local_source} as __VUE_DEFAULT__ }} from '{}'\n",
                    source_value
                ));
                edits.remove(specifier.span.start as usize, end);
                edits.append(format!("\nconst {variable} = __VUE_DEFAULT__"));
            }
        } else {
            let end = specifier_end(
                input,
                specifier.span.end as usize,
                declaration.span.end as usize,
            );
            edits.remove(specifier.span.start as usize, end);
            edits.append(format!("\nconst {variable} = {local_name}"));
        }
    }
    found
}

pub(crate) fn export_named_declaration_only_exports_default(
    declaration: &ExportNamedDeclaration<'_>,
) -> bool {
    !declaration.specifiers.is_empty()
        && declaration
            .specifiers
            .iter()
            .all(|specifier| module_export_name(specifier.exported()) == Some("default"))
}

pub(crate) trait ExportSpecifierAccess<'a> {
    fn local(&self) -> &ModuleExportName<'a>;
    fn exported(&self) -> &ModuleExportName<'a>;
}

impl<'a> ExportSpecifierAccess<'a> for ExportSpecifier<'a> {
    fn local(&self) -> &ModuleExportName<'a> {
        &self.local
    }

    fn exported(&self) -> &ModuleExportName<'a> {
        &self.exported
    }
}

pub(crate) fn module_export_name<'a>(name: &'a ModuleExportName<'a>) -> Option<&'a str> {
    match name {
        ModuleExportName::IdentifierName(identifier) => Some(identifier.name.as_str()),
        ModuleExportName::IdentifierReference(identifier) => Some(identifier.name.as_str()),
        ModuleExportName::StringLiteral(literal) => Some(literal.value.as_str()),
    }
}

pub(crate) fn specifier_end(input: &str, mut end: usize, node_end: usize) -> usize {
    let node_end = node_end.min(input.len());
    let old_end = end;
    let mut has_comma = false;
    while end < node_end {
        let Some(ch) = input[end..].chars().next() else {
            break;
        };
        if ch.is_whitespace() {
            end += ch.len_utf8();
        } else if ch == ',' {
            end += ch.len_utf8();
            has_comma = true;
            break;
        } else {
            break;
        }
    }
    if has_comma {
        end
    } else {
        old_end
    }
}

pub(crate) fn rewrite_vue27_default_lexical(input: &str, variable: &str) -> String {
    let Some(default_start) = find_export_default_keyword(input) else {
        return format!("{input}\nconst {variable} = {{}}");
    };
    let value_start = default_start + "default".len();
    let export_start = input[..default_start]
        .rfind("export")
        .unwrap_or(default_start);
    let mut output = String::new();
    output.push_str(&input[..export_start]);
    output.push_str(&format!("const {variable} ="));
    output.push_str(&input[value_start..]);
    output
}

pub(crate) fn find_export_default_keyword(input: &str) -> Option<usize> {
    let mut index = 0usize;
    while index < input.len() {
        let next = input[index..].find("export")? + index;
        if is_word_boundary(input, next, "export")
            && input[next + "export".len()..]
                .trim_start()
                .starts_with("default")
        {
            let default_start = next
                + "export".len()
                + input[next + "export".len()..]
                    .len()
                    .saturating_sub(input[next + "export".len()..].trim_start().len());
            if is_word_boundary(input, default_start, "default") {
                return Some(default_start);
            }
        }
        index = next + "export".len();
    }
    None
}

pub(crate) fn is_word_boundary(input: &str, start: usize, word: &str) -> bool {
    let before = input[..start].chars().next_back();
    let after = input[start + word.len()..].chars().next();
    !before.is_some_and(is_identifier_continue) && !after.is_some_and(is_identifier_continue)
}

pub(crate) fn is_identifier_continue(ch: char) -> bool {
    ch == '_' || ch == '$' || ch.is_ascii_alphanumeric()
}

pub(crate) fn prefix_vue27_identifiers(
    input: &str,
    options: Vue27PrefixIdentifiersOptions,
) -> String {
    let allocator = oxc_allocator::Allocator::default();
    let source_type = if options.is_ts {
        oxc_span::SourceType::ts()
    } else {
        oxc_span::SourceType::script()
    };
    let parsed = oxc_parser::Parser::new(&allocator, input, source_type)
        .with_options(oxc_parser::ParseOptions {
            parse_regular_expression: true,
            ..oxc_parser::ParseOptions::default()
        })
        .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return input.to_string();
    }

    let mut edits = SourceEdits::new(input);
    let mut context = PrefixIdentifiersContext {
        input,
        options,
        locals: Vec::new(),
        edits: &mut edits,
    };
    for statement in &parsed.program.body {
        context.walk_statement(statement);
    }
    edits.apply()
}

pub(crate) fn vue27_sfc_template_code(
    render: &str,
    static_render_fns: &[String],
    options: Vue27PrefixIdentifiersOptions,
    is_production: bool,
) -> String {
    let render_args = if options.is_functional { "_c,_vm" } else { "" };
    let prefixed_render = vue27_named_render_function(
        &vue27_prefix_anonymous_function(
            &format!("function ({render_args}){{{render}\n}}"),
            options.clone(),
        ),
        render_args,
    );
    let prefixed_static = static_render_fns
        .iter()
        .map(|render| {
            vue27_prefix_anonymous_function(
                &format!("function ({render_args}){{{render}\n}}"),
                options.clone(),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let mut code = format!(
        "var __render__ = {prefixed_render}\nvar __staticRenderFns__ = [{prefixed_static}]\n"
    );
    code = code.replace(" __render__ ", " render ");
    code = code.replace(" __staticRenderFns__ ", " staticRenderFns ");
    if !is_production {
        code.push_str("render._withStripped = true");
    }
    code
}

pub(crate) fn vue27_named_render_function(function: &str, render_args: &str) -> String {
    let anonymous_prefix = format!("function ({render_args})");
    let named_prefix = format!("function render({render_args})");
    if let Some(body) = function.strip_prefix(&anonymous_prefix) {
        format!("{named_prefix}{body}")
    } else {
        function.to_string()
    }
}

pub(crate) fn vue27_prefix_anonymous_function(
    source: &str,
    options: Vue27PrefixIdentifiersOptions,
) -> String {
    let prefixed = prefix_vue27_identifiers(&format!("({source})"), options);
    prefixed
        .strip_prefix('(')
        .and_then(|value| value.strip_suffix(')'))
        .unwrap_or(&prefixed)
        .to_string()
}

pub(crate) struct PrefixIdentifiersContext<'a, 'b> {
    pub(crate) input: &'a str,
    pub(crate) options: Vue27PrefixIdentifiersOptions,
    pub(crate) locals: Vec<BTreeMap<String, usize>>,
    pub(crate) edits: &'b mut SourceEdits<'a>,
}

impl PrefixIdentifiersContext<'_, '_> {
    pub(crate) fn walk_statement(&mut self, statement: &Statement<'_>) {
        match statement {
            Statement::WithStatement(statement) => self.walk_with_statement(statement),
            Statement::BlockStatement(block) => {
                self.push_scope();
                self.mark_block_declarations(&block.body);
                for statement in &block.body {
                    self.walk_statement(statement);
                }
                self.pop_scope();
            }
            Statement::ExpressionStatement(statement) => {
                self.walk_expression(&statement.expression)
            }
            Statement::ReturnStatement(statement) => {
                if let Some(argument) = &statement.argument {
                    self.walk_expression(argument);
                }
            }
            Statement::VariableDeclaration(declaration) => {
                self.mark_variable_declaration(declaration);
                for declarator in &declaration.declarations {
                    if let Some(init) = &declarator.init {
                        self.walk_expression(init);
                    }
                }
            }
            Statement::FunctionDeclaration(function) => self.walk_function(function),
            Statement::IfStatement(statement) => {
                self.walk_expression(&statement.test);
                self.walk_statement(&statement.consequent);
                if let Some(alternate) = &statement.alternate {
                    self.walk_statement(alternate);
                }
            }
            Statement::ForStatement(statement) => {
                self.push_scope();
                if let Some(init) = &statement.init {
                    match init {
                        oxc_ast::ast::ForStatementInit::VariableDeclaration(declaration) => {
                            self.mark_variable_declaration(declaration);
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
                self.walk_statement(&statement.body);
                self.pop_scope();
            }
            Statement::ForInStatement(statement) => {
                self.push_scope();
                self.walk_for_iteration_left(&statement.left);
                self.walk_expression(&statement.right);
                self.walk_statement(&statement.body);
                self.pop_scope();
            }
            Statement::ForOfStatement(statement) => {
                self.push_scope();
                self.walk_for_iteration_left(&statement.left);
                self.walk_expression(&statement.right);
                self.walk_statement(&statement.body);
                self.pop_scope();
            }
            _ => {}
        }
    }

    pub(crate) fn walk_with_statement(&mut self, statement: &WithStatement<'_>) {
        if !self.options.is_functional {
            self.edits.prepend_right(
                statement.span.start as usize,
                if self.is_script_setup() {
                    "var _vm=this,_c=_vm._self._c,_setup=_vm._self._setupProxy;"
                } else {
                    "var _vm=this,_c=_vm._self._c;"
                },
            );
        }
        let Some(body_start) = self.with_body_content_start(statement) else {
            self.walk_statement(&statement.body);
            return;
        };
        self.edits.remove(statement.span.start as usize, body_start);
        self.edits.remove(
            statement.span.end.saturating_sub(1) as usize,
            statement.span.end as usize,
        );
        self.walk_statement(&statement.body);
    }

    pub(crate) fn with_body_content_start(&self, statement: &WithStatement<'_>) -> Option<usize> {
        let start = statement.body.span().start as usize;
        let body_source = self.input.get(start..)?;
        body_source.find('{').map(|offset| start + offset + 1)
    }

    pub(crate) fn walk_for_iteration_left(&mut self, left: &oxc_ast::ast::ForStatementLeft<'_>) {
        match left {
            oxc_ast::ast::ForStatementLeft::VariableDeclaration(declaration) => {
                self.mark_variable_declaration(declaration);
                for declarator in &declaration.declarations {
                    if let Some(init) = &declarator.init {
                        self.walk_expression(init);
                    }
                }
            }
            _ => {
                if let Some(target) = left.as_assignment_target() {
                    self.mark_assignment_target_as_local(target);
                }
            }
        }
    }

    pub(crate) fn walk_expression(&mut self, expression: &Expression<'_>) {
        match expression {
            Expression::Identifier(identifier) => self.prefix_identifier(
                identifier.name.as_str(),
                identifier.span.start as usize,
                PrefixParent::Reference,
            ),
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
            Expression::CallExpression(call) => {
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
            Expression::ArrayExpression(array) => {
                for element in &array.elements {
                    match element {
                        oxc_ast::ast::ArrayExpressionElement::SpreadElement(spread) => {
                            self.walk_expression(&spread.argument)
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
            Expression::FunctionExpression(function) => self.walk_function(function),
            Expression::ArrowFunctionExpression(function) => self.walk_arrow_function(function),
            Expression::AssignmentExpression(assignment) => {
                self.walk_assignment_target(&assignment.left);
                self.walk_expression(&assignment.right);
            }
            Expression::UpdateExpression(update) => {
                self.walk_simple_assignment_target(&update.argument);
            }
            Expression::UnaryExpression(unary) => self.walk_expression(&unary.argument),
            Expression::BinaryExpression(binary) => {
                self.walk_expression(&binary.left);
                self.walk_expression(&binary.right);
            }
            Expression::LogicalExpression(logical) => {
                self.walk_expression(&logical.left);
                self.walk_expression(&logical.right);
            }
            Expression::ConditionalExpression(conditional) => {
                self.walk_expression(&conditional.test);
                self.walk_expression(&conditional.consequent);
                self.walk_expression(&conditional.alternate);
            }
            Expression::SequenceExpression(sequence) => {
                for expression in &sequence.expressions {
                    self.walk_expression(expression);
                }
            }
            Expression::TemplateLiteral(template) => {
                for expression in &template.expressions {
                    self.walk_expression(expression);
                }
            }
            Expression::TaggedTemplateExpression(template) => {
                self.walk_expression(&template.tag);
                for expression in &template.quasi.expressions {
                    self.walk_expression(expression);
                }
            }
            Expression::ParenthesizedExpression(parenthesized) => {
                self.walk_expression(&parenthesized.expression);
            }
            Expression::TSAsExpression(expression) => self.walk_expression(&expression.expression),
            Expression::TSSatisfiesExpression(expression) => {
                self.walk_expression(&expression.expression)
            }
            Expression::TSNonNullExpression(expression) => {
                self.walk_expression(&expression.expression)
            }
            Expression::ChainExpression(chain) => match &chain.expression {
                oxc_ast::ast::ChainElement::CallExpression(call) => {
                    self.walk_expression(&call.callee);
                    for argument in &call.arguments {
                        self.walk_argument(argument);
                    }
                }
                oxc_ast::ast::ChainElement::StaticMemberExpression(member) => {
                    self.walk_expression(&member.object)
                }
                oxc_ast::ast::ChainElement::ComputedMemberExpression(member) => {
                    self.walk_expression(&member.object);
                    self.walk_expression(&member.expression);
                }
                oxc_ast::ast::ChainElement::PrivateFieldExpression(member) => {
                    self.walk_expression(&member.object)
                }
                _ => {}
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
            ObjectPropertyKind::ObjectProperty(property) => self.walk_object_property(property),
            ObjectPropertyKind::SpreadProperty(spread) => self.walk_expression(&spread.argument),
        }
    }

    pub(crate) fn walk_object_property(&mut self, property: &ObjectProperty<'_>) {
        if property.computed {
            self.walk_property_key(&property.key);
        }
        if property.shorthand {
            if let Expression::Identifier(identifier) = &property.value {
                self.prefix_identifier(
                    identifier.name.as_str(),
                    identifier.span.end as usize,
                    PrefixParent::ShorthandPropertyValue,
                );
                return;
            }
        }
        self.walk_expression(&property.value);
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
            self.mark_block_declarations(&body.statements);
            for statement in &body.statements {
                self.walk_statement(statement);
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
        self.mark_block_declarations(&function.body.statements);
        for statement in &function.body.statements {
            self.walk_statement(statement);
        }
        self.pop_scope();
    }

    pub(crate) fn walk_assignment_target(&mut self, target: &AssignmentTarget<'_>) {
        match target {
            AssignmentTarget::AssignmentTargetIdentifier(identifier) => self.prefix_identifier(
                identifier.name.as_str(),
                identifier.span.start as usize,
                PrefixParent::Reference,
            ),
            AssignmentTarget::StaticMemberExpression(member) => {
                self.walk_expression(&member.object)
            }
            AssignmentTarget::ComputedMemberExpression(member) => {
                self.walk_expression(&member.object);
                self.walk_expression(&member.expression);
            }
            AssignmentTarget::PrivateFieldExpression(member) => {
                self.walk_expression(&member.object)
            }
            _ => {}
        }
    }

    pub(crate) fn walk_simple_assignment_target(&mut self, target: &SimpleAssignmentTarget<'_>) {
        match target {
            SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) => self
                .prefix_identifier(
                    identifier.name.as_str(),
                    identifier.span.start as usize,
                    PrefixParent::Reference,
                ),
            SimpleAssignmentTarget::StaticMemberExpression(member) => {
                self.walk_expression(&member.object)
            }
            SimpleAssignmentTarget::ComputedMemberExpression(member) => {
                self.walk_expression(&member.object);
                self.walk_expression(&member.expression);
            }
            SimpleAssignmentTarget::PrivateFieldExpression(member) => {
                self.walk_expression(&member.object)
            }
            _ => {}
        }
    }

    pub(crate) fn mark_assignment_target_as_local(&mut self, target: &AssignmentTarget<'_>) {
        if let AssignmentTarget::AssignmentTargetIdentifier(identifier) = target {
            self.mark_local(identifier.name.as_str());
        }
    }

    pub(crate) fn mark_block_declarations(&mut self, statements: &[Statement<'_>]) {
        for statement in statements {
            match statement {
                Statement::VariableDeclaration(declaration) => {
                    self.mark_variable_declaration(declaration);
                }
                Statement::FunctionDeclaration(function) => {
                    if let Some(id) = &function.id {
                        self.mark_local(id.name.as_str());
                    }
                }
                Statement::ClassDeclaration(class) => {
                    if let Some(id) = &class.id {
                        self.mark_local(id.name.as_str());
                    }
                }
                _ => {}
            }
        }
    }

    pub(crate) fn mark_variable_declaration(&mut self, declaration: &VariableDeclaration<'_>) {
        for declarator in &declaration.declarations {
            self.mark_binding_pattern(&declarator.id);
        }
    }

    pub(crate) fn mark_binding_pattern(&mut self, pattern: &BindingPattern<'_>) {
        match pattern {
            BindingPattern::BindingIdentifier(identifier) => {
                self.mark_local(identifier.name.as_str())
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

    pub(crate) fn prefix_identifier(&mut self, name: &str, offset: usize, parent: PrefixParent) {
        if do_not_prefix_vue27(name) || self.is_local(name) {
            return;
        }
        let prefix = self.prefix_for(name);
        match parent {
            PrefixParent::Reference => self.edits.prepend_right(offset, prefix),
            PrefixParent::ShorthandPropertyValue => {
                self.edits.append_left(offset, format!(": {prefix}{name}"))
            }
        }
    }

    pub(crate) fn prefix_for(&self, name: &str) -> &'static str {
        if self.is_script_setup()
            && self
                .options
                .bindings
                .get(name)
                .is_some_and(|binding| binding.starts_with("setup"))
        {
            "_setup."
        } else {
            "_vm."
        }
    }

    pub(crate) fn is_script_setup(&self) -> bool {
        !matches!(
            self.options
                .bindings
                .get("__isScriptSetup")
                .map(String::as_str),
            Some("false")
        ) && !self.options.bindings.is_empty()
    }

    pub(crate) fn push_scope(&mut self) {
        self.locals.push(BTreeMap::new());
    }

    pub(crate) fn pop_scope(&mut self) {
        self.locals.pop();
    }

    pub(crate) fn mark_local(&mut self, name: &str) {
        if let Some(scope) = self.locals.last_mut() {
            *scope.entry(name.to_string()).or_insert(0) += 1;
        }
    }

    pub(crate) fn is_local(&self, name: &str) -> bool {
        self.locals
            .iter()
            .rev()
            .any(|scope| scope.get(name).is_some_and(|count| *count > 0))
    }
}

#[derive(Clone, Copy)]
pub(crate) enum PrefixParent {
    Reference,
    ShorthandPropertyValue,
}

pub(crate) fn do_not_prefix_vue27(name: &str) -> bool {
    matches!(
        name,
        "Infinity"
            | "undefined"
            | "NaN"
            | "isFinite"
            | "isNaN"
            | "parseFloat"
            | "parseInt"
            | "decodeURI"
            | "decodeURIComponent"
            | "encodeURI"
            | "encodeURIComponent"
            | "Math"
            | "Number"
            | "Date"
            | "Array"
            | "Object"
            | "Boolean"
            | "String"
            | "RegExp"
            | "Map"
            | "Set"
            | "JSON"
            | "Intl"
            | "require"
            | "arguments"
            | "_c"
    )
}

pub(crate) fn source_with_overwrite(
    input: &str,
    start: usize,
    end: usize,
    replacement: &str,
) -> String {
    let start = start.min(input.len());
    let end = end.min(input.len()).max(start);
    let mut output = String::new();
    output.push_str(&input[..start]);
    output.push_str(replacement);
    output.push_str(&input[end..]);
    output
}

pub(crate) fn has_vue27_default_export_like(input: &str) -> bool {
    let mut index = 0usize;
    while let Some(offset) = input[index..].find("export") {
        let export_start = index + offset;
        if is_vue27_export_boundary(input, export_start)
            && input[export_start..].contains("default")
        {
            return true;
        }
        index = export_start + "export".len();
    }
    false
}

pub(crate) fn is_vue27_export_boundary(input: &str, export_start: usize) -> bool {
    let prefix = &input[..export_start];
    let Some(non_space) = prefix
        .char_indices()
        .rev()
        .find(|(_, ch)| !matches!(ch, ' ' | '\t' | '\r'))
    else {
        return true;
    };
    matches!(non_space.1, '\n' | ';')
}

#[derive(Debug)]
pub(crate) struct SourceEdits<'a> {
    pub(crate) input: &'a str,
    pub(crate) edits: Vec<SourceEdit>,
    pub(crate) prepend: String,
    pub(crate) append: String,
}

#[derive(Debug)]
pub(crate) struct SourceEdit {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) replacement: String,
}

impl<'a> SourceEdits<'a> {
    pub(crate) fn new(input: &'a str) -> Self {
        Self {
            input,
            edits: Vec::new(),
            prepend: String::new(),
            append: String::new(),
        }
    }

    pub(crate) fn overwrite(&mut self, start: usize, end: usize, replacement: impl Into<String>) {
        self.edits.push(SourceEdit {
            start,
            end,
            replacement: replacement.into(),
        });
    }

    pub(crate) fn remove(&mut self, start: usize, end: usize) {
        self.overwrite(start, end, "");
    }

    pub(crate) fn prepend_right(&mut self, offset: usize, value: impl Into<String>) {
        self.overwrite(offset, offset, value);
    }

    pub(crate) fn append_left(&mut self, offset: usize, value: impl Into<String>) {
        self.overwrite(offset, offset, value);
    }

    pub(crate) fn prepend(&mut self, value: impl AsRef<str>) {
        self.prepend.push_str(value.as_ref());
    }

    pub(crate) fn append(&mut self, value: impl AsRef<str>) {
        self.append.push_str(value.as_ref());
    }

    pub(crate) fn apply(mut self) -> String {
        self.edits.sort_by_key(|edit| (edit.start, edit.end));
        let mut output = String::new();
        output.push_str(&self.prepend);
        let mut cursor = 0usize;
        for edit in self.edits {
            if edit.start < cursor {
                continue;
            }
            output.push_str(&self.input[cursor..edit.start.min(self.input.len())]);
            output.push_str(&edit.replacement);
            cursor = edit.end.min(self.input.len());
        }
        output.push_str(&self.input[cursor..]);
        output.push_str(&self.append);
        output
    }
}
