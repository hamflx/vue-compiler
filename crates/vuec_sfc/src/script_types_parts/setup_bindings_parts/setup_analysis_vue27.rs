pub(crate) fn collect_vue3_setup_local_bindings(
    statements: &[Statement<'_>],
    is_ts: bool,
    literal_const_enabled: bool,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    for statement in statements {
        match statement {
            Statement::VariableDeclaration(declaration) if !declaration.declare => {
                let is_all_static = vue3_variable_declaration_is_static_hoist(declaration);
                for declarator in &declaration.declarations {
                    insert_pattern_bindings(&declarator.id, &mut analysis.local_setup_bindings);
                    let binding_type = vue3_setup_binding_type(
                        declaration.kind,
                        declarator.init.as_ref(),
                        is_all_static,
                        literal_const_enabled,
                        &analysis.vue_import_aliases,
                    );
                    collect_pattern_binding_types(
                        &declarator.id,
                        binding_type,
                        &mut analysis.local_setup_binding_types,
                    );
                }
            }
            Statement::FunctionDeclaration(function) if !function.declare => {
                if let Some(id) = &function.id {
                    analysis.local_setup_bindings.insert(id.name.to_string());
                    analysis
                        .local_setup_binding_types
                        .insert(id.name.to_string(), "setup-const".into());
                }
            }
            Statement::ClassDeclaration(class) if !class.declare => {
                if let Some(id) = &class.id {
                    analysis.local_setup_bindings.insert(id.name.to_string());
                    analysis
                        .local_setup_binding_types
                        .insert(id.name.to_string(), "setup-const".into());
                }
            }
            Statement::TSEnumDeclaration(declaration) if is_ts && !declaration.declare => {
                analysis
                    .local_setup_bindings
                    .insert(declaration.id.name.to_string());
                analysis.local_setup_binding_types.insert(
                    declaration.id.name.to_string(),
                    vue3_ts_enum_binding_type(declaration).into(),
                );
            }
            _ => {}
        }
    }
}

pub(crate) fn analyze_vue27_setup_variable_declaration(
    source: &str,
    declaration: &VariableDeclaration<'_>,
    edits: &mut SourceEdits<'_>,
    analysis: &mut Vue27ScriptSetupAnalysis,
    is_prod: bool,
) {
    let mut macro_declarators = Vec::new();
    for (index, declarator) in declaration.declarations.iter().enumerate() {
        if let Some(Expression::CallExpression(call)) = &declarator.init {
                if is_call_named(call, "defineProps") {
                    collect_define_props_call(source, call, None, analysis, is_prod);
                    collect_pattern_bindings(&declarator.id, &mut analysis.return_bindings);
                    analysis.setup_bindings.insert(
                        first_pattern_binding(&declarator.id).unwrap_or_else(|| "props".into()),
                        "setup-reactive-const".into(),
                    );
                    analysis
                        .setup_prelude
                        .push_str(&vue27_props_alias_declaration(source, &declarator.id));
                    macro_declarators.push(index);
                    continue;
                }
                if is_call_named(call, "withDefaults")
                    && collect_with_defaults_call(
                        source,
                        call,
                        Some(&declarator.id),
                        analysis,
                        is_prod,
                    )
                {
                    collect_pattern_bindings(&declarator.id, &mut analysis.return_bindings);
                    analysis.setup_bindings.insert(
                        first_pattern_binding(&declarator.id).unwrap_or_else(|| "props".into()),
                        "setup-const".into(),
                    );
                    macro_declarators.push(index);
                    continue;
                }
                if is_call_named(call, "defineEmits") {
                    let emit_binding =
                        first_pattern_binding(&declarator.id).unwrap_or_else(|| "emit".into());
                    collect_define_emits_call(source, call, Some(&emit_binding), analysis);
                    analysis
                        .setup_bindings
                        .insert(emit_binding.clone(), "setup-const".into());
                    collect_pattern_bindings(&declarator.id, &mut analysis.return_bindings);
                    macro_declarators.push(index);
                    continue;
                }
        }
        let binding_type =
            vue27_setup_binding_type(declaration.kind, declarator.init.as_ref(), analysis);
        collect_pattern_binding_types(&declarator.id, binding_type, &mut analysis.setup_bindings);
        collect_pattern_bindings(&declarator.id, &mut analysis.return_bindings);
    }
    remove_vue27_macro_declarators(declaration, &macro_declarators, edits);
}

pub(crate) fn hoist_vue27_setup_statement(
    source: &str,
    statement: &Statement<'_>,
    edits: &mut SourceEdits<'_>,
    analysis: &mut Vue27ScriptSetupAnalysis,
) {
    let (start, end) = vue27_statement_span_with_trailing_ws(source, statement);
    let source_text = source.get(start..end).unwrap_or_default();
    analysis.module_chunks.push(Vue27ModuleChunk {
        start,
        content: source_text.to_string(),
    });
    edits.remove(start, end);
}

pub(crate) fn vue27_statement_is_type_hoist(statement: &Statement<'_>) -> bool {
    match statement {
        Statement::TSTypeAliasDeclaration(_)
        | Statement::TSInterfaceDeclaration(_)
        | Statement::TSModuleDeclaration(_)
        | Statement::TSGlobalDeclaration(_)
        | Statement::TSImportEqualsDeclaration(_) => true,
        Statement::VariableDeclaration(declaration) => declaration.declare,
        Statement::FunctionDeclaration(function) => function.declare,
        Statement::ClassDeclaration(class) => class.declare,
        Statement::ExportNamedDeclaration(declaration) => {
            declaration.export_kind == ImportOrExportKind::Type
        }
        _ => false,
    }
}

pub(crate) fn vue27_statement_span_with_trailing_ws(
    source: &str,
    statement: &Statement<'_>,
) -> (usize, usize) {
    let start = statement.span().start as usize;
    let mut end = statement.span().end as usize;
    while source
        .get(end..)
        .and_then(|tail| tail.chars().next())
        .is_some_and(char::is_whitespace)
    {
        end += source[end..].chars().next().map_or(0, char::len_utf8);
    }
    (start, end)
}

pub(crate) fn vue27_statement_span_with_trailing_comments(
    source: &str,
    mut end: usize,
    comments: &[oxc_ast::ast::Comment],
) -> usize {
    let Some(comment) = comments
        .iter()
        .find(|comment| comment.is_trailing() && comment.span.start as usize >= end)
    else {
        return end;
    };
    if source
        .get(end..comment.span.start as usize)
        .is_none_or(|between| between.contains('\n'))
    {
        return end;
    }
    end = comment.span.end as usize;
    while source
        .get(end..)
        .and_then(|tail| tail.chars().next())
        .is_some_and(char::is_whitespace)
    {
        end += source[end..].chars().next().map_or(0, char::len_utf8);
    }
    end
}

pub(crate) fn vue27_setup_binding_type(
    kind: VariableDeclarationKind,
    init: Option<&Expression<'_>>,
    analysis: &Vue27ScriptSetupAnalysis,
) -> &'static str {
    if kind != VariableDeclarationKind::Const {
        return "setup-let";
    }
    if init.is_some_and(|init| {
        is_literal_expression(init) || is_call_expression_named(init, "defineProps")
    }) {
        return "setup-const";
    }
    if init.is_some_and(|init| is_vue27_ref_call(init, analysis)) {
        return "setup-ref";
    }
    "setup-maybe-ref"
}

pub(crate) fn is_vue27_ref_call(
    expression: &Expression<'_>,
    analysis: &Vue27ScriptSetupAnalysis,
) -> bool {
    let ref_name = analysis
        .user_import_aliases
        .get("ref")
        .map(String::as_str)
        .unwrap_or("ref");
    is_call_expression_named(expression, ref_name)
}

pub(crate) fn is_literal_expression(expression: &Expression<'_>) -> bool {
    matches!(
        expression,
        Expression::StringLiteral(_)
            | Expression::NumericLiteral(_)
            | Expression::BooleanLiteral(_)
            | Expression::NullLiteral(_)
            | Expression::RegExpLiteral(_)
            | Expression::BigIntLiteral(_)
    )
}

pub(crate) fn is_call_expression_named(expression: &Expression<'_>, name: &str) -> bool {
    matches!(expression, Expression::CallExpression(call) if is_call_named(call, name))
}

pub(crate) fn is_call_named(call: &oxc_ast::ast::CallExpression<'_>, name: &str) -> bool {
    matches!(&call.callee, Expression::Identifier(identifier) if identifier.name == name)
}
