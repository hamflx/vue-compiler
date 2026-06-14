use crate::*;

pub(crate) fn sfc_script_ast_body(
    store: &JsAstStore,
    id: JsProgramId,
    source: &str,
    mode: SfcScriptAstMode,
) -> Vec<Value> {
    let context = SfcScriptAstProjectionContext::new(source);
    store
        .parse_registered_program(id)
        .ok()
        .map(|parsed| {
            parsed
                .program
                .body
                .iter()
                .map(|statement| match mode {
                    SfcScriptAstMode::None => Value::Null,
                    SfcScriptAstMode::TopLevel => sfc_script_ast_base_value(
                        &context,
                        sfc_script_statement_type_name(statement),
                        statement.span(),
                    ),
                    SfcScriptAstMode::Full => sfc_script_statement_ast_value(&context, statement),
                })
                .filter(|value| !value.is_null())
                .collect()
        })
        .unwrap_or_default()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SfcScriptAstProjectionContext<'a> {
    pub(crate) source: &'a str,
    pub(crate) line_index: SfcScriptLineIndex,
}

impl<'a> SfcScriptAstProjectionContext<'a> {
    pub(crate) fn new(source: &'a str) -> Self {
        Self {
            source,
            line_index: SfcScriptLineIndex::new(source),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SfcScriptLineIndex {
    pub(crate) break_offsets: Vec<usize>,
    pub(crate) line_starts: Vec<usize>,
}

impl SfcScriptLineIndex {
    pub(crate) fn new(source: &str) -> Self {
        let mut break_offsets = Vec::new();
        let mut line_starts = vec![0];
        let bytes = source.as_bytes();
        let mut index = 0usize;
        while index < bytes.len() {
            match bytes[index] {
                b'\r' => {
                    break_offsets.push(index);
                    if bytes.get(index + 1) == Some(&b'\n') {
                        index += 1;
                    }
                    line_starts.push(index + 1);
                }
                b'\n' => {
                    break_offsets.push(index);
                    line_starts.push(index + 1);
                }
                _ => {}
            }
            index += 1;
        }
        Self {
            break_offsets,
            line_starts,
        }
    }

    pub(crate) fn position_at(&self, source: &str, offset: usize) -> Option<SfcPosition> {
        if offset > source.len() || !source.is_char_boundary(offset) {
            return None;
        }
        let line_index = self
            .break_offsets
            .partition_point(|break_offset| *break_offset < offset);
        let bytes = source.as_bytes();
        let line_start = if offset > 0
            && offset < bytes.len()
            && bytes[offset] == b'\n'
            && bytes[offset - 1] == b'\r'
        {
            offset
        } else {
            self.line_starts.get(line_index).copied().unwrap_or(0)
        };
        Some(SfcPosition {
            column: source[line_start..offset].encode_utf16().count() + 1,
            line: line_index + 1,
            offset,
        })
    }
}

pub(crate) fn sfc_script_statement_ast_value(
    context: &SfcScriptAstProjectionContext<'_>,
    statement: &Statement<'_>,
) -> Value {
    let mut value = sfc_script_ast_base_value(
        context,
        sfc_script_statement_type_name(statement),
        statement.span(),
    );
    match statement {
        Statement::BlockStatement(block) => {
            value["body"] = json!(block
                .body
                .iter()
                .map(|statement| sfc_script_statement_ast_value(context, statement))
                .collect::<Vec<_>>());
        }
        Statement::ExpressionStatement(statement) => {
            value["expression"] = sfc_script_expression_ast_value(context, &statement.expression);
        }
        Statement::IfStatement(statement) => {
            value["test"] = sfc_script_expression_ast_value(context, &statement.test);
            value["consequent"] = sfc_script_statement_ast_value(context, &statement.consequent);
            value["alternate"] = statement
                .alternate
                .as_ref()
                .map(|statement| sfc_script_statement_ast_value(context, statement))
                .unwrap_or(Value::Null);
        }
        Statement::ReturnStatement(statement) => {
            value["argument"] = statement
                .argument
                .as_ref()
                .map(|argument| sfc_script_expression_ast_value(context, argument))
                .unwrap_or(Value::Null);
        }
        Statement::ThrowStatement(statement) => {
            value["argument"] = sfc_script_expression_ast_value(context, &statement.argument);
        }
        Statement::VariableDeclaration(declaration) => {
            sfc_script_add_variable_declaration_ast_fields(context, &mut value, declaration);
        }
        Statement::FunctionDeclaration(function) => {
            sfc_script_add_function_ast_fields(context, &mut value, function);
        }
        Statement::ClassDeclaration(class) => {
            sfc_script_add_class_ast_fields(context, &mut value, class);
        }
        Statement::ImportDeclaration(import) => {
            value["moduleSource"] = sfc_script_string_literal_ast_value(context, &import.source);
            value["importKind"] = json!(sfc_script_import_export_kind(import.import_kind));
            value["specifiers"] = json!(import
                .specifiers
                .as_ref()
                .map(|specifiers| {
                    specifiers
                        .iter()
                        .map(|specifier| sfc_script_import_specifier_ast_value(context, specifier))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default());
        }
        Statement::ExportDefaultDeclaration(declaration) => {
            value["declaration"] =
                sfc_script_export_default_declaration_ast_value(context, declaration);
        }
        Statement::ExportNamedDeclaration(declaration) => {
            value["exportKind"] = json!(sfc_script_import_export_kind(declaration.export_kind));
            value["moduleSource"] = declaration
                .source
                .as_ref()
                .map(|source_literal| sfc_script_string_literal_ast_value(context, source_literal))
                .unwrap_or(Value::Null);
            value["declaration"] = declaration
                .declaration
                .as_ref()
                .map(|declaration| sfc_script_declaration_ast_value(context, declaration))
                .unwrap_or(Value::Null);
            value["specifiers"] = json!(declaration
                .specifiers
                .iter()
                .map(|specifier| sfc_script_export_specifier_ast_value(context, specifier))
                .collect::<Vec<_>>());
        }
        Statement::ExportAllDeclaration(declaration) => {
            value["exportKind"] = json!(sfc_script_import_export_kind(declaration.export_kind));
            value["moduleSource"] =
                sfc_script_string_literal_ast_value(context, &declaration.source);
            value["exported"] = declaration
                .exported
                .as_ref()
                .map(|exported| sfc_script_module_export_name_ast_value(context, exported))
                .unwrap_or(Value::Null);
        }
        _ => {}
    }
    value
}

pub(crate) fn sfc_script_ast_base_value(
    context: &SfcScriptAstProjectionContext<'_>,
    type_name: &str,
    span: oxc_span::Span,
) -> Value {
    let start = span.start as usize;
    let end = span.end as usize;
    json!({
        "type": type_name,
        "start": start,
        "end": end,
        "loc": sfc_script_loc_value(context, start, end),
        "source": sfc_script_source_slice(context.source, start, end),
    })
}

pub(crate) fn sfc_script_loc_value(
    context: &SfcScriptAstProjectionContext<'_>,
    start: usize,
    end: usize,
) -> Value {
    json!({
        "start": context.line_index.position_at(context.source, start).unwrap_or(SfcPosition {
            line: 1,
            column: 1,
            offset: start,
        }),
        "end": context.line_index.position_at(context.source, end).unwrap_or(SfcPosition {
            line: 1,
            column: 1,
            offset: end,
        }),
    })
}

pub(crate) fn sfc_script_source_slice(source: &str, start: usize, end: usize) -> String {
    source.get(start..end).unwrap_or_default().to_string()
}

pub(crate) fn sfc_script_statement_type_name(statement: &Statement<'_>) -> &'static str {
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
        Statement::FunctionDeclaration(_) => "FunctionDeclaration",
        Statement::IfStatement(_) => "IfStatement",
        Statement::ImportDeclaration(_) => "ImportDeclaration",
        Statement::LabeledStatement(_) => "LabeledStatement",
        Statement::ReturnStatement(_) => "ReturnStatement",
        Statement::SwitchStatement(_) => "SwitchStatement",
        Statement::ThrowStatement(_) => "ThrowStatement",
        Statement::TryStatement(_) => "TryStatement",
        Statement::VariableDeclaration(_) => "VariableDeclaration",
        Statement::WhileStatement(_) => "WhileStatement",
        Statement::WithStatement(_) => "WithStatement",
        Statement::ClassDeclaration(_) => "ClassDeclaration",
        Statement::TSEnumDeclaration(_) => "TSEnumDeclaration",
        Statement::TSInterfaceDeclaration(_) => "TSInterfaceDeclaration",
        Statement::TSTypeAliasDeclaration(_) => "TSTypeAliasDeclaration",
        Statement::TSModuleDeclaration(_) => "TSModuleDeclaration",
        Statement::TSImportEqualsDeclaration(_) => "TSImportEqualsDeclaration",
        Statement::TSExportAssignment(_) => "TSExportAssignment",
        Statement::TSNamespaceExportDeclaration(_) => "TSNamespaceExportDeclaration",
        Statement::TSGlobalDeclaration(_) => "TSGlobalDeclaration",
        Statement::ExportAllDeclaration(_) => "ExportAllDeclaration",
        Statement::ExportDefaultDeclaration(_) => "ExportDefaultDeclaration",
        Statement::ExportNamedDeclaration(_) => "ExportNamedDeclaration",
    }
}

pub(crate) fn sfc_script_expression_ast_value(
    context: &SfcScriptAstProjectionContext<'_>,
    expression: &Expression<'_>,
) -> Value {
    let mut value = sfc_script_ast_base_value(
        context,
        sfc_script_expression_type_name(expression),
        expression.span(),
    );
    match expression {
        Expression::Identifier(identifier) => {
            value["name"] = json!(identifier.name.as_str());
        }
        Expression::StringLiteral(literal) => {
            value["value"] = json!(literal.value.as_str());
        }
        Expression::NumericLiteral(literal) => {
            value["value"] = json!(literal.value);
        }
        Expression::BooleanLiteral(literal) => {
            value["value"] = json!(literal.value);
        }
        Expression::NullLiteral(_) => {
            value["value"] = Value::Null;
        }
        Expression::ObjectExpression(object) => {
            value["properties"] = json!(object
                .properties
                .iter()
                .map(|property| sfc_script_object_property_ast_value(context, property))
                .collect::<Vec<_>>());
        }
        Expression::ArrayExpression(array) => {
            value["elements"] = json!(array
                .elements
                .iter()
                .map(|element| sfc_script_array_element_ast_value(context, element))
                .collect::<Vec<_>>());
        }
        Expression::CallExpression(call) => {
            value["callee"] = sfc_script_expression_ast_value(context, &call.callee);
            value["arguments"] = json!(call
                .arguments
                .iter()
                .map(|argument| sfc_script_argument_ast_value(context, argument))
                .collect::<Vec<_>>());
            value["optional"] = json!(call.optional);
        }
        Expression::StaticMemberExpression(member) => {
            value["type"] = json!("MemberExpression");
            value["object"] = sfc_script_expression_ast_value(context, &member.object);
            value["property"] = sfc_script_identifier_name_ast_value(context, &member.property);
            value["computed"] = json!(false);
            value["optional"] = json!(member.optional);
        }
        Expression::ComputedMemberExpression(member) => {
            value["type"] = json!("MemberExpression");
            value["object"] = sfc_script_expression_ast_value(context, &member.object);
            value["property"] = sfc_script_expression_ast_value(context, &member.expression);
            value["computed"] = json!(true);
            value["optional"] = json!(member.optional);
        }
        Expression::FunctionExpression(function) => {
            sfc_script_add_function_ast_fields(context, &mut value, function);
        }
        Expression::ClassExpression(class) => {
            sfc_script_add_class_ast_fields(context, &mut value, class);
        }
        Expression::ParenthesizedExpression(parenthesized) => {
            value["expression"] =
                sfc_script_expression_ast_value(context, &parenthesized.expression);
        }
        _ => {}
    }
    value
}

pub(crate) fn sfc_script_expression_type_name(expression: &Expression<'_>) -> &'static str {
    match expression {
        Expression::ArrayExpression(_) => "ArrayExpression",
        Expression::ArrowFunctionExpression(_) => "ArrowFunctionExpression",
        Expression::AssignmentExpression(_) => "AssignmentExpression",
        Expression::AwaitExpression(_) => "AwaitExpression",
        Expression::BigIntLiteral(_) => "BigIntLiteral",
        Expression::BinaryExpression(_) => "BinaryExpression",
        Expression::BooleanLiteral(_) => "BooleanLiteral",
        Expression::CallExpression(_) => "CallExpression",
        Expression::ChainExpression(_) => "ChainExpression",
        Expression::ClassExpression(_) => "ClassExpression",
        Expression::ComputedMemberExpression(_) => "MemberExpression",
        Expression::ConditionalExpression(_) => "ConditionalExpression",
        Expression::FunctionExpression(_) => "FunctionExpression",
        Expression::Identifier(_) => "Identifier",
        Expression::ImportExpression(_) => "ImportExpression",
        Expression::LogicalExpression(_) => "LogicalExpression",
        Expression::NewExpression(_) => "NewExpression",
        Expression::NullLiteral(_) => "NullLiteral",
        Expression::NumericLiteral(_) => "NumericLiteral",
        Expression::ObjectExpression(_) => "ObjectExpression",
        Expression::ParenthesizedExpression(_) => "ParenthesizedExpression",
        Expression::PrivateFieldExpression(_) => "MemberExpression",
        Expression::PrivateInExpression(_) => "PrivateInExpression",
        Expression::RegExpLiteral(_) => "RegExpLiteral",
        Expression::SequenceExpression(_) => "SequenceExpression",
        Expression::StaticMemberExpression(_) => "MemberExpression",
        Expression::StringLiteral(_) => "StringLiteral",
        Expression::Super(_) => "Super",
        Expression::TaggedTemplateExpression(_) => "TaggedTemplateExpression",
        Expression::TemplateLiteral(_) => "TemplateLiteral",
        Expression::ThisExpression(_) => "ThisExpression",
        Expression::UnaryExpression(_) => "UnaryExpression",
        Expression::UpdateExpression(_) => "UpdateExpression",
        Expression::YieldExpression(_) => "YieldExpression",
        Expression::JSXElement(_) => "JSXElement",
        Expression::JSXFragment(_) => "JSXFragment",
        Expression::MetaProperty(_) => "MetaProperty",
        Expression::TSAsExpression(_) => "TSAsExpression",
        Expression::TSInstantiationExpression(_) => "TSInstantiationExpression",
        Expression::TSNonNullExpression(_) => "TSNonNullExpression",
        Expression::TSSatisfiesExpression(_) => "TSSatisfiesExpression",
        Expression::TSTypeAssertion(_) => "TSTypeAssertion",
        Expression::V8IntrinsicExpression(_) => "V8IntrinsicExpression",
    }
}

pub(crate) fn sfc_script_add_variable_declaration_ast_fields(
    context: &SfcScriptAstProjectionContext<'_>,
    value: &mut Value,
    declaration: &VariableDeclaration<'_>,
) {
    value["kind"] = json!(sfc_script_variable_kind(declaration.kind));
    value["declarations"] = json!(declaration
        .declarations
        .iter()
        .map(|declarator| {
            let mut value =
                sfc_script_ast_base_value(context, "VariableDeclarator", declarator.span);
            value["id"] = sfc_script_binding_pattern_ast_value(context, &declarator.id);
            value["init"] = declarator
                .init
                .as_ref()
                .map(|init| sfc_script_expression_ast_value(context, init))
                .unwrap_or(Value::Null);
            value
        })
        .collect::<Vec<_>>());
}

pub(crate) fn sfc_script_variable_kind(kind: VariableDeclarationKind) -> &'static str {
    match kind {
        VariableDeclarationKind::Var => "var",
        VariableDeclarationKind::Let => "let",
        VariableDeclarationKind::Const => "const",
        VariableDeclarationKind::Using => "using",
        VariableDeclarationKind::AwaitUsing => "await using",
    }
}

pub(crate) fn sfc_script_add_function_ast_fields(
    context: &SfcScriptAstProjectionContext<'_>,
    value: &mut Value,
    function: &Function<'_>,
) {
    value["id"] = function
        .id
        .as_ref()
        .map(|id| sfc_script_binding_identifier_ast_value(context, id))
        .unwrap_or(Value::Null);
    value["params"] = json!(function
        .params
        .items
        .iter()
        .map(|parameter| sfc_script_formal_parameter_ast_value(context, parameter))
        .collect::<Vec<_>>());
    value["generator"] = json!(function.generator);
    value["async"] = json!(function.r#async);
}

pub(crate) fn sfc_script_add_class_ast_fields(
    context: &SfcScriptAstProjectionContext<'_>,
    value: &mut Value,
    class: &oxc_ast::ast::Class<'_>,
) {
    value["id"] = class
        .id
        .as_ref()
        .map(|id| sfc_script_binding_identifier_ast_value(context, id))
        .unwrap_or(Value::Null);
    value["superClass"] = class
        .super_class
        .as_ref()
        .map(|super_class| sfc_script_expression_ast_value(context, super_class))
        .unwrap_or(Value::Null);
}

pub(crate) fn sfc_script_declaration_ast_value(
    context: &SfcScriptAstProjectionContext<'_>,
    declaration: &Declaration<'_>,
) -> Value {
    match declaration {
        Declaration::VariableDeclaration(declaration) => {
            let mut value =
                sfc_script_ast_base_value(context, "VariableDeclaration", declaration.span);
            sfc_script_add_variable_declaration_ast_fields(context, &mut value, declaration);
            value
        }
        Declaration::FunctionDeclaration(function) => {
            let mut value =
                sfc_script_ast_base_value(context, "FunctionDeclaration", function.span);
            sfc_script_add_function_ast_fields(context, &mut value, function);
            value
        }
        Declaration::ClassDeclaration(class) => {
            let mut value = sfc_script_ast_base_value(context, "ClassDeclaration", class.span);
            sfc_script_add_class_ast_fields(context, &mut value, class);
            value
        }
        Declaration::TSTypeAliasDeclaration(declaration) => {
            sfc_script_ast_base_value(context, "TSTypeAliasDeclaration", declaration.span)
        }
        Declaration::TSInterfaceDeclaration(declaration) => {
            sfc_script_ast_base_value(context, "TSInterfaceDeclaration", declaration.span)
        }
        Declaration::TSEnumDeclaration(declaration) => {
            sfc_script_ast_base_value(context, "TSEnumDeclaration", declaration.span)
        }
        Declaration::TSModuleDeclaration(declaration) => {
            sfc_script_ast_base_value(context, "TSModuleDeclaration", declaration.span)
        }
        Declaration::TSGlobalDeclaration(declaration) => {
            sfc_script_ast_base_value(context, "TSGlobalDeclaration", declaration.span)
        }
        Declaration::TSImportEqualsDeclaration(declaration) => {
            sfc_script_ast_base_value(context, "TSImportEqualsDeclaration", declaration.span)
        }
    }
}

pub(crate) fn sfc_script_export_default_declaration_ast_value(
    context: &SfcScriptAstProjectionContext<'_>,
    declaration: &ExportDefaultDeclaration<'_>,
) -> Value {
    match &declaration.declaration {
        ExportDefaultDeclarationKind::FunctionDeclaration(function) => {
            let mut value =
                sfc_script_ast_base_value(context, "FunctionDeclaration", function.span);
            sfc_script_add_function_ast_fields(context, &mut value, function);
            value
        }
        ExportDefaultDeclarationKind::ClassDeclaration(class) => {
            let mut value = sfc_script_ast_base_value(context, "ClassDeclaration", class.span);
            sfc_script_add_class_ast_fields(context, &mut value, class);
            value
        }
        ExportDefaultDeclarationKind::TSInterfaceDeclaration(declaration) => {
            sfc_script_ast_base_value(context, "TSInterfaceDeclaration", declaration.span)
        }
        _ => {
            if let Some(expression) = declaration.declaration.as_expression() {
                sfc_script_expression_ast_value(context, expression)
            } else {
                sfc_script_ast_base_value(context, "Declaration", declaration.declaration.span())
            }
        }
    }
}

pub(crate) fn sfc_script_import_specifier_ast_value(
    context: &SfcScriptAstProjectionContext<'_>,
    specifier: &ImportDeclarationSpecifier<'_>,
) -> Value {
    match specifier {
        ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
            let mut value = sfc_script_ast_base_value(context, "ImportSpecifier", specifier.span);
            value["imported"] =
                sfc_script_module_export_name_ast_value(context, &specifier.imported);
            value["local"] = sfc_script_binding_identifier_ast_value(context, &specifier.local);
            value["importKind"] = json!(sfc_script_import_export_kind(specifier.import_kind));
            value
        }
        ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
            let mut value =
                sfc_script_ast_base_value(context, "ImportDefaultSpecifier", specifier.span);
            value["local"] = sfc_script_binding_identifier_ast_value(context, &specifier.local);
            value
        }
        ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
            let mut value =
                sfc_script_ast_base_value(context, "ImportNamespaceSpecifier", specifier.span);
            value["local"] = sfc_script_binding_identifier_ast_value(context, &specifier.local);
            value
        }
    }
}

pub(crate) fn sfc_script_export_specifier_ast_value(
    context: &SfcScriptAstProjectionContext<'_>,
    specifier: &ExportSpecifier<'_>,
) -> Value {
    let mut value = sfc_script_ast_base_value(context, "ExportSpecifier", specifier.span);
    value["local"] = sfc_script_module_export_name_ast_value(context, &specifier.local);
    value["exported"] = sfc_script_module_export_name_ast_value(context, &specifier.exported);
    value["exportKind"] = json!(sfc_script_import_export_kind(specifier.export_kind));
    value
}

pub(crate) fn sfc_script_import_export_kind(kind: ImportOrExportKind) -> &'static str {
    match kind {
        ImportOrExportKind::Value => "value",
        ImportOrExportKind::Type => "type",
    }
}

pub(crate) fn sfc_script_binding_pattern_ast_value(
    context: &SfcScriptAstProjectionContext<'_>,
    pattern: &BindingPattern<'_>,
) -> Value {
    match pattern {
        BindingPattern::BindingIdentifier(identifier) => {
            sfc_script_binding_identifier_ast_value(context, identifier)
        }
        BindingPattern::ObjectPattern(pattern) => {
            let mut value = sfc_script_ast_base_value(context, "ObjectPattern", pattern.span);
            value["properties"] = json!(pattern
                .properties
                .iter()
                .map(|property| {
                    let mut value = sfc_script_ast_base_value(context, "Property", property.span);
                    value["key"] = sfc_script_property_key_ast_value(context, &property.key);
                    value["value"] = sfc_script_binding_pattern_ast_value(context, &property.value);
                    value["computed"] = json!(property.computed);
                    value["shorthand"] = json!(property.shorthand);
                    value
                })
                .collect::<Vec<_>>());
            value
        }
        BindingPattern::ArrayPattern(pattern) => {
            let mut value = sfc_script_ast_base_value(context, "ArrayPattern", pattern.span);
            value["elements"] = json!(pattern
                .elements
                .iter()
                .map(|element| {
                    element
                        .as_ref()
                        .map(|element| sfc_script_binding_pattern_ast_value(context, element))
                        .unwrap_or(Value::Null)
                })
                .collect::<Vec<_>>());
            value
        }
        BindingPattern::AssignmentPattern(pattern) => {
            let mut value = sfc_script_ast_base_value(context, "AssignmentPattern", pattern.span);
            value["left"] = sfc_script_binding_pattern_ast_value(context, &pattern.left);
            value["right"] = sfc_script_expression_ast_value(context, &pattern.right);
            value
        }
    }
}

pub(crate) fn sfc_script_binding_identifier_ast_value(
    context: &SfcScriptAstProjectionContext<'_>,
    identifier: &oxc_ast::ast::BindingIdentifier<'_>,
) -> Value {
    let mut value = sfc_script_ast_base_value(context, "Identifier", identifier.span);
    value["name"] = json!(identifier.name.as_str());
    value
}

pub(crate) fn sfc_script_identifier_name_ast_value(
    context: &SfcScriptAstProjectionContext<'_>,
    identifier: &oxc_ast::ast::IdentifierName<'_>,
) -> Value {
    let mut value = sfc_script_ast_base_value(context, "Identifier", identifier.span);
    value["name"] = json!(identifier.name.as_str());
    value
}

pub(crate) fn sfc_script_module_export_name_ast_value(
    context: &SfcScriptAstProjectionContext<'_>,
    name: &ModuleExportName<'_>,
) -> Value {
    match name {
        ModuleExportName::IdentifierName(identifier) => {
            sfc_script_identifier_name_ast_value(context, identifier)
        }
        ModuleExportName::IdentifierReference(identifier) => {
            let mut value = sfc_script_ast_base_value(context, "Identifier", identifier.span);
            value["name"] = json!(identifier.name.as_str());
            value
        }
        ModuleExportName::StringLiteral(literal) => {
            sfc_script_string_literal_ast_value(context, literal)
        }
    }
}

pub(crate) fn sfc_script_string_literal_ast_value(
    context: &SfcScriptAstProjectionContext<'_>,
    literal: &oxc_ast::ast::StringLiteral<'_>,
) -> Value {
    let mut value = sfc_script_ast_base_value(context, "StringLiteral", literal.span);
    value["value"] = json!(literal.value.as_str());
    value
}

pub(crate) fn sfc_script_formal_parameter_ast_value(
    context: &SfcScriptAstProjectionContext<'_>,
    parameter: &FormalParameter<'_>,
) -> Value {
    let mut value = sfc_script_binding_pattern_ast_value(context, &parameter.pattern);
    if let Some(initializer) = &parameter.initializer {
        let mut assignment =
            sfc_script_ast_base_value(context, "AssignmentPattern", parameter.span);
        assignment["left"] = value;
        assignment["right"] = sfc_script_expression_ast_value(context, initializer);
        value = assignment;
    }
    value
}

pub(crate) fn sfc_script_object_property_ast_value(
    context: &SfcScriptAstProjectionContext<'_>,
    property: &ObjectPropertyKind<'_>,
) -> Value {
    match property {
        ObjectPropertyKind::ObjectProperty(property) => {
            let mut value = sfc_script_ast_base_value(context, "Property", property.span);
            value["key"] = sfc_script_property_key_ast_value(context, &property.key);
            value["value"] = sfc_script_expression_ast_value(context, &property.value);
            value["computed"] = json!(property.computed);
            value["shorthand"] = json!(property.shorthand);
            value
        }
        ObjectPropertyKind::SpreadProperty(spread) => {
            let mut value = sfc_script_ast_base_value(context, "SpreadElement", spread.span);
            value["argument"] = sfc_script_expression_ast_value(context, &spread.argument);
            value
        }
    }
}

pub(crate) fn sfc_script_property_key_ast_value(
    context: &SfcScriptAstProjectionContext<'_>,
    key: &PropertyKey<'_>,
) -> Value {
    match key {
        PropertyKey::StaticIdentifier(identifier) => {
            sfc_script_identifier_name_ast_value(context, identifier)
        }
        PropertyKey::PrivateIdentifier(identifier) => {
            let mut value = sfc_script_ast_base_value(context, "PrivateName", identifier.span);
            value["name"] = json!(identifier.name.as_str());
            value
        }
        _ => key
            .as_expression()
            .map(|expression| sfc_script_expression_ast_value(context, expression))
            .unwrap_or_else(|| sfc_script_ast_base_value(context, "Identifier", key.span())),
    }
}

pub(crate) fn sfc_script_array_element_ast_value(
    context: &SfcScriptAstProjectionContext<'_>,
    element: &ArrayExpressionElement<'_>,
) -> Value {
    match element {
        ArrayExpressionElement::SpreadElement(spread) => {
            let mut value = sfc_script_ast_base_value(context, "SpreadElement", spread.span);
            value["argument"] = sfc_script_expression_ast_value(context, &spread.argument);
            value
        }
        ArrayExpressionElement::Elision(_) => Value::Null,
        _ => element
            .as_expression()
            .map(|expression| sfc_script_expression_ast_value(context, expression))
            .unwrap_or_else(|| sfc_script_ast_base_value(context, "Expression", element.span())),
    }
}

pub(crate) fn sfc_script_argument_ast_value(
    context: &SfcScriptAstProjectionContext<'_>,
    argument: &Argument<'_>,
) -> Value {
    match argument {
        Argument::SpreadElement(spread) => {
            let mut value = sfc_script_ast_base_value(context, "SpreadElement", spread.span);
            value["argument"] = sfc_script_expression_ast_value(context, &spread.argument);
            value
        }
        _ => argument
            .as_expression()
            .map(|expression| sfc_script_expression_ast_value(context, expression))
            .unwrap_or_else(|| sfc_script_ast_base_value(context, "Expression", argument.span())),
    }
}

pub(crate) fn position_at(source: &str, offset: usize) -> Option<SfcPosition> {
    if offset > source.len() || !source.is_char_boundary(offset) {
        return None;
    }
    let mut line = 1usize;
    let mut line_start = 0usize;
    let bytes = source.as_bytes();
    let mut index = 0usize;
    while index < offset {
        match bytes[index] {
            b'\r' => {
                if index + 1 < offset && bytes.get(index + 1) == Some(&b'\n') {
                    index += 1;
                }
                line += 1;
                line_start = index + 1;
            }
            b'\n' => {
                line += 1;
                line_start = index + 1;
            }
            _ => {}
        }
        index += 1;
    }
    Some(SfcPosition {
        column: source[line_start..offset].encode_utf16().count() + 1,
        line,
        offset,
    })
}

pub(crate) fn script_source_type(descriptor: &SfcDescriptor) -> oxc_span::SourceType {
    let lang = descriptor
        .script_setup
        .as_ref()
        .or(descriptor.script.as_ref())
        .and_then(|block| block.attrs.lang.as_deref());
    match lang {
        Some("tsx") => oxc_span::SourceType::tsx(),
        Some("ts") => oxc_span::SourceType::ts(),
        Some("jsx") => oxc_span::SourceType::jsx(),
        _ => oxc_span::SourceType::mjs(),
    }
}

pub(crate) fn script_source_type_from_attrs(attrs: &SfcBlockAttrs) -> oxc_span::SourceType {
    match attrs.lang.as_deref() {
        Some("tsx") => oxc_span::SourceType::tsx(),
        Some("ts") => oxc_span::SourceType::ts(),
        Some("jsx") => oxc_span::SourceType::jsx(),
        _ => oxc_span::SourceType::mjs(),
    }
}

pub(crate) fn script_lang_is_js_like(attrs: &SfcBlockAttrs) -> bool {
    matches!(
        attrs.lang.as_deref(),
        None | Some("js" | "jsx" | "ts" | "tsx")
    )
}

pub(crate) fn script_mode(attrs: &SfcBlockAttrs) -> JsParseMode {
    if matches!(attrs.lang.as_deref(), Some("ts" | "tsx")) {
        JsParseMode::TypeScript
    } else {
        JsParseMode::ScriptModule
    }
}
