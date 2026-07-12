pub(crate) fn vue27_strip_template_expression_strings(exp: &str) -> String {
    let mut output = String::new();
    let mut chars = exp.char_indices().peekable();
    while let Some((_, ch)) = chars.next() {
        match ch {
            '\'' | '"' => {
                while let Some((_, inner)) = chars.next() {
                    if inner == '\\' {
                        let _ = chars.next();
                    } else if inner == ch {
                        break;
                    }
                }
            }
            '`' => {
                let mut template_expr = String::new();
                while let Some((_, inner)) = chars.next() {
                    if inner == '\\' {
                        let _ = chars.next();
                    } else if inner == '`' {
                        break;
                    } else if inner == '$' && chars.peek().is_some_and(|(_, next)| *next == '{') {
                        let _ = chars.next();
                        let mut depth = 1usize;
                        for (_, expr_ch) in chars.by_ref() {
                            if expr_ch == '{' {
                                depth += 1;
                                template_expr.push(expr_ch);
                            } else if expr_ch == '}' {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                }
                                template_expr.push(expr_ch);
                            } else {
                                template_expr.push(expr_ch);
                            }
                        }
                        template_expr.push(',');
                    }
                }
                output.push_str(&template_expr);
            }
            _ => output.push(ch),
        }
    }
    output
}

pub(crate) fn identifier_usage_contains(usage: &str, local: &str) -> bool {
    if local.is_empty() {
        return false;
    }
    let mut search_start = 0usize;
    while let Some(index) = usage[search_start..].find(local) {
        let start = search_start + index;
        let end = start + local.len();
        let before = usage[..start].chars().next_back();
        let after = usage[end..].chars().next();
        if !before.is_some_and(is_identifier_usage_char)
            && !after.is_some_and(is_identifier_usage_char)
        {
            return true;
        }
        search_start = end;
    }
    false
}

pub(crate) fn is_identifier_usage_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '$')
}

pub(crate) fn vue27_camelize(value: &str) -> String {
    let mut output = String::new();
    let mut uppercase_next = false;
    for ch in value.chars() {
        if ch == '-' {
            uppercase_next = true;
        } else if uppercase_next {
            output.extend(ch.to_uppercase());
            uppercase_next = false;
        } else {
            output.push(ch);
        }
    }
    output
}

pub(crate) fn vue27_capitalize(value: &str) -> String {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    first.to_uppercase().chain(chars).collect()
}

pub(crate) fn split_vue27_setup_module_content(content: &str) -> (String, String) {
    let mut module = String::new();
    let mut setup = String::new();
    let mut last_module_indent = "";
    for line in split_inclusive_lines(content) {
        let line_without_newline = line.trim_end_matches(['\n', '\r']);
        let trimmed = line_without_newline.trim_start();
        if trimmed.starts_with("import ") {
            if !module.is_empty() && !module.ends_with('\n') {
                module.push('\n');
            }
            if module.is_empty() {
                module.push_str(trimmed);
            } else {
                module.push_str(line_without_newline);
            }
            module.push('\n');
            last_module_indent =
                &line_without_newline[..line_without_newline.len() - trimmed.len()];
        } else {
            setup.push_str(line);
        }
    }
    if !last_module_indent.is_empty() {
        module.push_str(last_module_indent);
    }
    (module, setup)
}

pub(crate) fn split_inclusive_lines(value: &str) -> Vec<&str> {
    if value.is_empty() {
        return Vec::new();
    }
    let mut lines = value.split_inclusive('\n').collect::<Vec<_>>();
    if value.ends_with("\n\n") {
        lines.push("");
    }
    lines
}

pub(crate) fn leading_blank_line_indent(value: &str) -> Option<&str> {
    let line_end = value.find('\n').unwrap_or(value.len());
    let first_line = &value[..line_end];
    if first_line.is_empty() || first_line.trim().is_empty() {
        Some(first_line)
    } else {
        None
    }
}

pub(crate) fn vue27_normal_script_binding_metadata(
    descriptor: &SfcDescriptor,
) -> BTreeMap<String, String> {
    let mut bindings = vue27_script_options_binding_metadata(descriptor);
    bindings.insert("__isScriptSetup".into(), "false".into());
    bindings
}

pub(crate) fn vue3_normal_script_options_binding_metadata(
    descriptor: &SfcDescriptor,
) -> Option<BTreeMap<String, String>> {
    let script = descriptor.script.as_ref()?;
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        &script.content,
        script_source_type_from_attrs(&script.attrs),
    )
    .with_options(oxc_parser::ParseOptions {
        parse_regular_expression: true,
        ..oxc_parser::ParseOptions::default()
    })
    .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return None;
    }
    for statement in &parsed.program.body {
        if let Statement::ExportDefaultDeclaration(default) = statement {
            if let ExportDefaultDeclarationKind::ObjectExpression(object) = &default.declaration {
                let mut bindings = BTreeMap::new();
                analyze_vue3_options_bindings(object, &mut bindings);
                return Some(bindings);
            }
        }
    }
    None
}

pub(crate) fn vue27_script_options_binding_metadata(
    descriptor: &SfcDescriptor,
) -> BTreeMap<String, String> {
    let Some(script) = descriptor.script.as_ref() else {
        return BTreeMap::new();
    };
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        &script.content,
        script_source_type_from_attrs(&script.attrs),
    )
    .with_options(oxc_parser::ParseOptions {
        parse_regular_expression: true,
        ..oxc_parser::ParseOptions::default()
    })
    .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return BTreeMap::new();
    }
    let mut bindings = BTreeMap::new();
    for statement in &parsed.program.body {
        if let Statement::ExportDefaultDeclaration(default) = statement {
            match &default.declaration {
                ExportDefaultDeclarationKind::ObjectExpression(object) => {
                    analyze_vue27_options_bindings(object, &mut bindings);
                }
                ExportDefaultDeclarationKind::CallExpression(call) => {
                    if let Some(argument) = call.arguments.first() {
                        if let Expression::ObjectExpression(object) = argument.to_expression() {
                            analyze_vue27_options_bindings(object, &mut bindings);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    bindings
}

pub(crate) fn vue27_script_setup_script_bindings(
    descriptor: &SfcDescriptor,
) -> BTreeMap<String, String> {
    let Some(script) = descriptor.script.as_ref() else {
        return BTreeMap::new();
    };
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        &script.content,
        script_source_type_from_attrs(&script.attrs),
    )
    .with_options(oxc_parser::ParseOptions {
        parse_regular_expression: true,
        ..oxc_parser::ParseOptions::default()
    })
    .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return BTreeMap::new();
    }
    let mut bindings = BTreeMap::new();
    for statement in &parsed.program.body {
        collect_vue27_top_level_script_binding(statement, &mut bindings);
    }
    bindings
}

pub(crate) fn vue27_script_setup_script_return_bindings(
    descriptor: &SfcDescriptor,
) -> Vue27ScriptReturnBindings {
    let Some(script) = descriptor.script.as_ref() else {
        return Vue27ScriptReturnBindings::default();
    };
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        &script.content,
        script_source_type_from_attrs(&script.attrs),
    )
    .with_options(oxc_parser::ParseOptions {
        parse_regular_expression: true,
        ..oxc_parser::ParseOptions::default()
    })
    .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return Vue27ScriptReturnBindings::default();
    }
    let mut result = Vue27ScriptReturnBindings::default();
    for statement in &parsed.program.body {
        collect_vue27_top_level_script_return_binding(statement, &mut result);
    }
    result
}

pub(crate) fn collect_vue27_top_level_script_return_binding(
    statement: &Statement<'_>,
    result: &mut Vue27ScriptReturnBindings,
) {
    match statement {
        Statement::ImportDeclaration(import) => {
            collect_vue27_import_return_bindings(import, &mut result.imports);
        }
        Statement::VariableDeclaration(declaration) if !declaration.declare => {
            collect_pattern_return_bindings_from_declaration(declaration, &mut result.bindings);
        }
        Statement::FunctionDeclaration(function) if !function.declare => {
            if let Some(id) = &function.id {
                push_unique(&mut result.bindings, id.name.as_str());
            }
        }
        Statement::ClassDeclaration(class) if !class.declare => {
            if let Some(id) = &class.id {
                push_unique(&mut result.bindings, id.name.as_str());
            }
        }
        Statement::TSEnumDeclaration(declaration) if !declaration.declare => {
            push_unique(&mut result.bindings, declaration.id.name.as_str());
        }
        Statement::ExportNamedDeclaration(declaration)
            if declaration.export_kind == ImportOrExportKind::Value =>
        {
            if let Some(declaration) = &declaration.declaration {
                collect_vue27_declaration_return_bindings(declaration, &mut result.bindings);
            }
        }
        _ => {}
    }
}

pub(crate) fn collect_vue27_import_return_bindings(
    import: &oxc_ast::ast::ImportDeclaration<'_>,
    imports: &mut Vec<Vue27ScriptImport>,
) {
    let Some(specifiers) = &import.specifiers else {
        return;
    };
    let source = import.source.value.as_str();
    for specifier in specifiers {
        imports.push(Vue27ScriptImport {
            local: import_specifier_local(specifier),
            source: source.to_string(),
            imported: import_specifier_imported(specifier).unwrap_or_else(|| "default".into()),
            is_type: vue27_import_specifier_is_type(import, specifier),
        });
    }
}

pub(crate) fn collect_vue27_declaration_return_bindings(
    declaration: &Declaration<'_>,
    bindings: &mut Vec<String>,
) {
    match declaration {
        Declaration::VariableDeclaration(declaration) if !declaration.declare => {
            collect_pattern_return_bindings_from_declaration(declaration, bindings);
        }
        Declaration::FunctionDeclaration(function) if !function.declare => {
            if let Some(id) = &function.id {
                push_unique(bindings, id.name.as_str());
            }
        }
        Declaration::ClassDeclaration(class) if !class.declare => {
            if let Some(id) = &class.id {
                push_unique(bindings, id.name.as_str());
            }
        }
        Declaration::TSEnumDeclaration(declaration) if !declaration.declare => {
            push_unique(bindings, declaration.id.name.as_str());
        }
        _ => {}
    }
}

pub(crate) fn collect_vue27_top_level_script_binding(
    statement: &Statement<'_>,
    bindings: &mut BTreeMap<String, String>,
) {
    match statement {
        Statement::ImportDeclaration(import) => {
            let source = import.source.value.as_str();
            if let Some(specifiers) = &import.specifiers {
                for specifier in specifiers {
                    let local = import_specifier_local(specifier);
                    let imported = import_specifier_imported(specifier);
                    let binding_type = if matches!(imported.as_deref(), Some("*"))
                        || (matches!(imported.as_deref(), Some("default"))
                            && source.ends_with(".vue"))
                        || source == "vue"
                    {
                        "setup-const"
                    } else {
                        "setup-maybe-ref"
                    };
                    bindings.insert(local, binding_type.into());
                }
            }
        }
        Statement::VariableDeclaration(declaration) if !declaration.declare => {
            collect_vue27_script_declaration_bindings(declaration, bindings);
        }
        Statement::FunctionDeclaration(function) if !function.declare => {
            if let Some(id) = &function.id {
                bindings.insert(id.name.to_string(), "setup-const".into());
            }
        }
        Statement::ClassDeclaration(class) if !class.declare => {
            if let Some(id) = &class.id {
                bindings.insert(id.name.to_string(), "setup-const".into());
            }
        }
        Statement::TSEnumDeclaration(declaration) if !declaration.declare => {
            bindings.insert(declaration.id.name.to_string(), "setup-const".into());
        }
        Statement::ExportNamedDeclaration(declaration)
            if declaration.export_kind == ImportOrExportKind::Value =>
        {
            if let Some(declaration) = &declaration.declaration {
                match declaration {
                    oxc_ast::ast::Declaration::VariableDeclaration(declaration)
                        if !declaration.declare =>
                    {
                        collect_vue27_script_declaration_bindings(declaration, bindings);
                    }
                    oxc_ast::ast::Declaration::FunctionDeclaration(function)
                        if !function.declare =>
                    {
                        if let Some(id) = &function.id {
                            bindings.insert(id.name.to_string(), "setup-const".into());
                        }
                    }
                    oxc_ast::ast::Declaration::ClassDeclaration(class) if !class.declare => {
                        if let Some(id) = &class.id {
                            bindings.insert(id.name.to_string(), "setup-const".into());
                        }
                    }
                    oxc_ast::ast::Declaration::TSEnumDeclaration(declaration)
                        if !declaration.declare =>
                    {
                        bindings.insert(declaration.id.name.to_string(), "setup-const".into());
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
}

pub(crate) fn collect_vue27_script_declaration_bindings(
    declaration: &VariableDeclaration<'_>,
    bindings: &mut BTreeMap<String, String>,
) {
    let binding_type = if declaration.kind == VariableDeclarationKind::Const {
        "setup-const"
    } else {
        "setup-let"
    };
    for declarator in &declaration.declarations {
        collect_pattern_binding_types(&declarator.id, binding_type, bindings);
    }
}

pub(crate) fn vue3_script_setup_script_binding_metadata(
    descriptor: &SfcDescriptor,
    vue_import_aliases: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let Some(script) = descriptor.script.as_ref() else {
        return BTreeMap::new();
    };
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        &script.content,
        script_source_type_from_attrs(&script.attrs),
    )
    .with_options(oxc_parser::ParseOptions {
        parse_regular_expression: true,
        ..oxc_parser::ParseOptions::default()
    })
    .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return BTreeMap::new();
    }
    let mut bindings = BTreeMap::new();
    for statement in &parsed.program.body {
        collect_vue3_top_level_script_binding(statement, vue_import_aliases, &mut bindings);
    }
    bindings
}

pub(crate) fn collect_vue3_top_level_script_binding(
    statement: &Statement<'_>,
    vue_import_aliases: &BTreeMap<String, String>,
    bindings: &mut BTreeMap<String, String>,
) {
    match statement {
        Statement::VariableDeclaration(declaration) if !declaration.declare => {
            collect_vue3_script_variable_declaration_bindings(
                declaration,
                vue_import_aliases,
                bindings,
            );
        }
        Statement::FunctionDeclaration(function) if !function.declare => {
            if let Some(id) = &function.id {
                bindings.insert(id.name.to_string(), "setup-const".into());
            }
        }
        Statement::ClassDeclaration(class) if !class.declare => {
            if let Some(id) = &class.id {
                bindings.insert(id.name.to_string(), "setup-const".into());
            }
        }
        Statement::TSEnumDeclaration(declaration) if !declaration.declare => {
            bindings.insert(
                declaration.id.name.to_string(),
                vue3_ts_enum_binding_type(declaration).into(),
            );
        }
        Statement::ExportNamedDeclaration(declaration)
            if declaration.export_kind == ImportOrExportKind::Value =>
        {
            if let Some(declaration) = &declaration.declaration {
                collect_vue3_script_declaration_binding(declaration, vue_import_aliases, bindings);
            }
        }
        _ => {}
    }
}

pub(crate) fn collect_vue3_script_declaration_binding(
    declaration: &Declaration<'_>,
    vue_import_aliases: &BTreeMap<String, String>,
    bindings: &mut BTreeMap<String, String>,
) {
    match declaration {
        Declaration::VariableDeclaration(declaration) if !declaration.declare => {
            collect_vue3_script_variable_declaration_bindings(
                declaration,
                vue_import_aliases,
                bindings,
            );
        }
        Declaration::FunctionDeclaration(function) if !function.declare => {
            if let Some(id) = &function.id {
                bindings.insert(id.name.to_string(), "setup-const".into());
            }
        }
        Declaration::ClassDeclaration(class) if !class.declare => {
            if let Some(id) = &class.id {
                bindings.insert(id.name.to_string(), "setup-const".into());
            }
        }
        Declaration::TSEnumDeclaration(declaration) if !declaration.declare => {
            bindings.insert(
                declaration.id.name.to_string(),
                vue3_ts_enum_binding_type(declaration).into(),
            );
        }
        _ => {}
    }
}

pub(crate) fn collect_vue3_script_variable_declaration_bindings(
    declaration: &VariableDeclaration<'_>,
    vue_import_aliases: &BTreeMap<String, String>,
    bindings: &mut BTreeMap<String, String>,
) {
    let is_const = declaration.kind == VariableDeclarationKind::Const;
    let is_all_literal = is_const
        && declaration.declarations.iter().all(|declarator| {
            matches!(declarator.id, BindingPattern::BindingIdentifier(_))
                && declarator.init.as_ref().is_some_and(vue3_is_static_node)
        });
    for declarator in &declaration.declarations {
        if matches!(declarator.id, BindingPattern::BindingIdentifier(_)) {
            collect_pattern_binding_types(
                &declarator.id,
                vue3_script_binding_type(
                    declaration.kind,
                    declarator.init.as_ref(),
                    is_all_literal,
                    vue_import_aliases,
                ),
                bindings,
            );
        } else {
            let is_const_macro_call = is_const
                && declarator.init.as_ref().is_some_and(|init| {
                    vue3_is_call_named_any(
                        init,
                        &["defineProps", "defineEmits", "withDefaults", "defineSlots"],
                    )
                });
            collect_vue3_script_pattern_binding_types(
                &declarator.id,
                is_const,
                is_const_macro_call,
                bindings,
            );
        }
    }
}

pub(crate) fn collect_vue3_script_pattern_binding_types(
    pattern: &BindingPattern<'_>,
    is_const: bool,
    is_define_call: bool,
    bindings: &mut BTreeMap<String, String>,
) {
    match pattern {
        BindingPattern::BindingIdentifier(identifier) => {
            bindings.insert(
                identifier.name.to_string(),
                vue3_script_pattern_binding_type(is_const, is_define_call).into(),
            );
        }
        BindingPattern::ObjectPattern(pattern) => {
            for property in &pattern.properties {
                let child_is_define_call = if matches!(
                    property.value,
                    BindingPattern::ObjectPattern(_) | BindingPattern::ArrayPattern(_)
                ) {
                    false
                } else {
                    is_define_call
                };
                collect_vue3_script_pattern_binding_types(
                    &property.value,
                    is_const,
                    child_is_define_call,
                    bindings,
                );
            }
            if let Some(rest) = &pattern.rest {
                collect_vue3_script_rest_binding_type(&rest.argument, is_const, bindings);
            }
        }
        BindingPattern::ArrayPattern(pattern) => {
            for element in pattern.elements.iter().flatten() {
                let child_is_define_call = if matches!(
                    element,
                    BindingPattern::ObjectPattern(_) | BindingPattern::ArrayPattern(_)
                ) {
                    false
                } else {
                    is_define_call
                };
                collect_vue3_script_pattern_binding_types(
                    element,
                    is_const,
                    child_is_define_call,
                    bindings,
                );
            }
            if let Some(rest) = &pattern.rest {
                collect_vue3_script_rest_binding_type(&rest.argument, is_const, bindings);
            }
        }
        BindingPattern::AssignmentPattern(pattern) => {
            if matches!(pattern.left, BindingPattern::BindingIdentifier(_)) {
                collect_vue3_script_pattern_binding_types(
                    &pattern.left,
                    is_const,
                    is_define_call,
                    bindings,
                );
            } else {
                collect_vue3_script_pattern_binding_types(&pattern.left, is_const, false, bindings);
            }
        }
    }
}

pub(crate) fn collect_vue3_script_rest_binding_type(
    pattern: &BindingPattern<'_>,
    is_const: bool,
    bindings: &mut BTreeMap<String, String>,
) {
    collect_pattern_binding_types(
        pattern,
        if is_const { "setup-const" } else { "setup-let" },
        bindings,
    );
}

pub(crate) fn vue3_script_pattern_binding_type(
    is_const: bool,
    is_define_call: bool,
) -> &'static str {
    if is_define_call {
        "setup-const"
    } else if is_const {
        "setup-maybe-ref"
    } else {
        "setup-let"
    }
}

pub(crate) fn vue3_script_binding_type(
    kind: VariableDeclarationKind,
    init: Option<&Expression<'_>>,
    is_all_literal: bool,
    vue_import_aliases: &BTreeMap<String, String>,
) -> &'static str {
    if kind != VariableDeclarationKind::Const {
        return "setup-let";
    }
    if is_all_literal || init.is_some_and(vue3_is_static_node) {
        return "literal-const";
    }
    if init.is_some_and(|init| vue3_is_call_named_any(init, &["defineProps"])) {
        return "setup-reactive-const";
    }
    if init.is_some_and(|init| {
        vue3_is_call_named_any(init, &["defineEmits", "withDefaults", "defineSlots"])
    }) {
        return "setup-const";
    }
    if init.is_some_and(|init| {
        vue3_is_call_named_alias(init, vue_import_aliases.get("reactive").map(String::as_str))
    }) {
        return "setup-reactive-const";
    }
    if init.is_some_and(|init| vue3_can_never_be_ref(init, vue_import_aliases)) {
        return "setup-const";
    }
    if init.is_some_and(|init| vue3_is_ref_like_call(init, vue_import_aliases)) {
        return "setup-ref";
    }
    "setup-maybe-ref"
}

pub(crate) fn vue3_can_never_be_ref(
    expression: &Expression<'_>,
    vue_import_aliases: &BTreeMap<String, String>,
) -> bool {
    let expression = unwrap_vue3_ts_expression(expression);
    if vue3_is_call_named_alias(
        expression,
        vue_import_aliases.get("reactive").map(String::as_str),
    ) {
        return true;
    }
    match expression {
        Expression::UnaryExpression(_)
        | Expression::BinaryExpression(_)
        | Expression::ArrayExpression(_)
        | Expression::ObjectExpression(_)
        | Expression::FunctionExpression(_)
        | Expression::ArrowFunctionExpression(_)
        | Expression::UpdateExpression(_)
        | Expression::ClassExpression(_)
        | Expression::TaggedTemplateExpression(_)
        | Expression::TemplateLiteral(_)
        | Expression::StringLiteral(_)
        | Expression::NumericLiteral(_)
        | Expression::BooleanLiteral(_)
        | Expression::NullLiteral(_)
        | Expression::BigIntLiteral(_)
        | Expression::RegExpLiteral(_) => true,
        Expression::SequenceExpression(expression) => expression
            .expressions
            .last()
            .is_some_and(|expression| vue3_can_never_be_ref(expression, vue_import_aliases)),
        _ => false,
    }
}

pub(crate) fn vue3_is_ref_like_call(
    expression: &Expression<'_>,
    vue_import_aliases: &BTreeMap<String, String>,
) -> bool {
    let expression = unwrap_vue3_ts_expression(expression);
    if vue3_is_call_named_any(expression, &["defineModel"]) {
        return true;
    }
    [
        "ref",
        "computed",
        "shallowRef",
        "customRef",
        "toRef",
        "useTemplateRef",
    ]
    .iter()
    .any(|imported| {
        vue3_is_call_named_alias(
            expression,
            vue_import_aliases.get(*imported).map(String::as_str),
        )
    })
}

pub(crate) fn vue3_is_call_named_any(expression: &Expression<'_>, names: &[&str]) -> bool {
    let expression = unwrap_vue3_ts_expression(expression);
    matches!(expression, Expression::CallExpression(call) if names.iter().any(|name| is_call_named(call, name)))
}

pub(crate) fn vue3_is_call_named_alias(expression: &Expression<'_>, name: Option<&str>) -> bool {
    let Some(name) = name else {
        return false;
    };
    matches!(unwrap_vue3_ts_expression(expression), Expression::CallExpression(call) if is_call_named(call, name))
}

pub(crate) fn collect_pattern_return_bindings_from_declaration(
    declaration: &VariableDeclaration<'_>,
    bindings: &mut Vec<String>,
) {
    for declarator in &declaration.declarations {
        collect_pattern_bindings(&declarator.id, bindings);
    }
}

pub(crate) fn analyze_vue27_options_bindings(
    object: &ObjectExpression<'_>,
    bindings: &mut BTreeMap<String, String>,
) {
    for property in &object.properties {
        let Some(property) = property.as_property() else {
            continue;
        };
        let Some(key) = property.key.static_name().map(|name| name.into_owned()) else {
            continue;
        };
        match key.as_str() {
            "props" => {
                if let Expression::ObjectExpression(props) = &property.value {
                    for key in object_expression_keys(props) {
                        bindings.insert(key, "props".into());
                    }
                } else if let Expression::ArrayExpression(array) = &property.value {
                    for element in &array.elements {
                        if let Some(Expression::StringLiteral(literal)) = element.as_expression() {
                            bindings.insert(literal.value.to_string(), "props".into());
                        }
                    }
                }
            }
            "computed" | "methods" => {
                if let Expression::ObjectExpression(values) = &property.value {
                    for key in object_expression_keys(values) {
                        bindings.insert(key, "options".into());
                    }
                }
            }
            "inject" => {
                collect_vue27_object_or_array_keys(&property.value, bindings, "options");
            }
            _ => {
                if let Expression::ObjectExpression(_) = &property.value {
                    continue;
                }
            }
        }
        if key == "setup" || key == "data" {
            collect_returned_object_keys(&property.value, key.as_str(), bindings);
        }
    }
}

pub(crate) fn analyze_vue3_options_bindings(
    object: &ObjectExpression<'_>,
    bindings: &mut BTreeMap<String, String>,
) {
    for property in &object.properties {
        let Some(property) = property.as_property() else {
            continue;
        };
        let Some(key) = vue3_normal_option_identifier_key(property) else {
            continue;
        };
        match key {
            "props" => {
                collect_vue27_object_or_array_keys(&property.value, bindings, "props");
            }
            "inject" => {
                collect_vue27_object_or_array_keys(&property.value, bindings, "options");
            }
            "computed" | "methods" => {
                if let Expression::ObjectExpression(values) = &property.value {
                    for key in object_expression_keys(values) {
                        bindings.insert(key, "options".into());
                    }
                }
            }
            "setup" | "data" if property.method => {
                collect_returned_object_keys(&property.value, key, bindings);
            }
            _ => {}
        }
    }
}

pub(crate) fn vue3_normal_option_identifier_key<'a>(
    property: &'a ObjectProperty<'_>,
) -> Option<&'a str> {
    if property.computed {
        return None;
    }
    match &property.key {
        PropertyKey::StaticIdentifier(identifier) => Some(identifier.name.as_str()),
        _ => None,
    }
}

pub(crate) fn collect_vue27_object_or_array_keys(
    expression: &Expression<'_>,
    bindings: &mut BTreeMap<String, String>,
    binding_type: &str,
) {
    match expression {
        Expression::ObjectExpression(object) => {
            for key in object_expression_keys(object) {
                bindings.insert(key, binding_type.to_string());
            }
        }
        Expression::ArrayExpression(array) => {
            for element in &array.elements {
                if let Some(Expression::StringLiteral(literal)) = element.as_expression() {
                    bindings.insert(literal.value.to_string(), binding_type.to_string());
                }
            }
        }
        _ => {}
    }
}

pub(crate) fn collect_returned_object_keys(
    expression: &Expression<'_>,
    option_key: &str,
    bindings: &mut BTreeMap<String, String>,
) {
    let body = match expression {
        Expression::FunctionExpression(function) => {
            function.body.as_ref().map(|body| &body.statements)
        }
        Expression::ArrowFunctionExpression(function) => Some(&function.body.statements),
        _ => None,
    };
    let Some(body) = body else {
        return;
    };
    for statement in body {
        if let Statement::ReturnStatement(statement) = statement {
            if let Some(Expression::ObjectExpression(object)) = &statement.argument {
                for key in object_expression_keys(object) {
                    bindings.insert(
                        key,
                        if option_key == "setup" {
                            "setup-maybe-ref".into()
                        } else {
                            "data".into()
                        },
                    );
                }
            }
        }
    }
}

pub(crate) fn collect_pattern_binding_types(
    pattern: &BindingPattern<'_>,
    binding_type: &str,
    bindings: &mut BTreeMap<String, String>,
) {
    match pattern {
        BindingPattern::BindingIdentifier(identifier) => {
            bindings.insert(identifier.name.to_string(), binding_type.to_string());
        }
        BindingPattern::ObjectPattern(pattern) => {
            for property in &pattern.properties {
                collect_pattern_binding_types(&property.value, binding_type, bindings);
            }
            if let Some(rest) = &pattern.rest {
                collect_pattern_binding_types(&rest.argument, binding_type, bindings);
            }
        }
        BindingPattern::ArrayPattern(pattern) => {
            for element in pattern.elements.iter().flatten() {
                collect_pattern_binding_types(element, binding_type, bindings);
            }
            if let Some(rest) = &pattern.rest {
                collect_pattern_binding_types(&rest.argument, binding_type, bindings);
            }
        }
        BindingPattern::AssignmentPattern(pattern) => {
            collect_pattern_binding_types(&pattern.left, binding_type, bindings);
        }
    }
}

pub(crate) fn insert_pattern_bindings(
    pattern: &BindingPattern<'_>,
    bindings: &mut BTreeSet<String>,
) {
    match pattern {
        BindingPattern::BindingIdentifier(identifier) => {
            bindings.insert(identifier.name.to_string());
        }
        BindingPattern::ObjectPattern(pattern) => {
            for property in &pattern.properties {
                insert_pattern_bindings(&property.value, bindings);
            }
            if let Some(rest) = &pattern.rest {
                insert_pattern_bindings(&rest.argument, bindings);
            }
        }
        BindingPattern::ArrayPattern(pattern) => {
            for element in pattern.elements.iter().flatten() {
                insert_pattern_bindings(element, bindings);
            }
            if let Some(rest) = &pattern.rest {
                insert_pattern_bindings(&rest.argument, bindings);
            }
        }
        BindingPattern::AssignmentPattern(pattern) => {
            insert_pattern_bindings(&pattern.left, bindings);
        }
    }
}

pub(crate) fn insert_formal_parameter_bindings(
    params: &oxc_ast::ast::FormalParameters<'_>,
    bindings: &mut BTreeSet<String>,
) {
    for param in &params.items {
        insert_pattern_bindings(&param.pattern, bindings);
    }
    if let Some(rest) = &params.rest {
        insert_pattern_bindings(&rest.rest.argument, bindings);
    }
}

pub(crate) fn insert_vue27_block_declarations(
    statements: &[Statement<'_>],
    bindings: &mut BTreeSet<String>,
) {
    for statement in statements {
        match statement {
            Statement::VariableDeclaration(declaration) if !declaration.declare => {
                for declarator in &declaration.declarations {
                    insert_pattern_bindings(&declarator.id, bindings);
                }
            }
            Statement::FunctionDeclaration(function) if !function.declare => {
                if let Some(id) = &function.id {
                    bindings.insert(id.name.to_string());
                }
            }
            Statement::ClassDeclaration(class) if !class.declare => {
                if let Some(id) = &class.id {
                    bindings.insert(id.name.to_string());
                }
            }
            _ => {}
        }
    }
}

pub(crate) fn collect_pattern_bindings(pattern: &BindingPattern<'_>, bindings: &mut Vec<String>) {
    match pattern {
        BindingPattern::BindingIdentifier(identifier) => {
            push_unique(bindings, identifier.name.as_str());
        }
        BindingPattern::ObjectPattern(pattern) => {
            for property in &pattern.properties {
                collect_pattern_bindings(&property.value, bindings);
            }
            if let Some(rest) = &pattern.rest {
                collect_pattern_bindings(&rest.argument, bindings);
            }
        }
        BindingPattern::ArrayPattern(pattern) => {
            for element in pattern.elements.iter().flatten() {
                collect_pattern_bindings(element, bindings);
            }
            if let Some(rest) = &pattern.rest {
                collect_pattern_bindings(&rest.argument, bindings);
            }
        }
        BindingPattern::AssignmentPattern(pattern) => {
            collect_pattern_bindings(&pattern.left, bindings);
        }
    }
}

pub(crate) fn push_unique(values: &mut Vec<String>, value: &str) {
    if !values.iter().any(|existing| existing == value) {
        values.push(value.to_string());
    }
}

pub(crate) fn trim_trailing_blank_lines(value: &str) -> &str {
    value.trim_end_matches(['\n', '\r'])
}

pub(crate) fn script_is_typescript(attrs: &SfcBlockAttrs) -> bool {
    matches!(attrs.lang.as_deref(), Some("ts" | "tsx"))
}

pub(crate) fn merge_template_errors(
    mut first: Vec<SfcTemplateError>,
    second: Vec<SfcTemplateError>,
) -> Vec<SfcTemplateError> {
    for error in second {
        if !first.iter().any(|existing| {
            existing.code == error.code
                && existing.loc.start.offset == error.loc.start.offset
                && existing.loc.end.offset == error.loc.end.offset
        }) {
            first.push(error);
        }
    }
    first
}

pub(crate) fn sfc_template_errors_from_diagnostics(
    diagnostics: &[Diagnostic],
    source: &str,
) -> Vec<SfcTemplateError> {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .filter_map(|diagnostic| sfc_template_error_from_diagnostic(diagnostic, source))
        .collect()
}

pub(crate) fn sfc_template_error_from_diagnostic(
    diagnostic: &Diagnostic,
    source: &str,
) -> Option<SfcTemplateError> {
    let span = diagnostic.span?;
    let start = span.start.0.min(source.len());
    let end = span.end.0.min(source.len()).max(start);
    Some(SfcTemplateError {
        code: diagnostic.code.parse().unwrap_or(0),
        message: diagnostic.message.clone(),
        loc: SfcSourceLocation {
            start: position_at(source, start)?,
            end: position_at(source, end)?,
            source: source.get(start..end).unwrap_or_default().to_string(),
        },
    })
}
