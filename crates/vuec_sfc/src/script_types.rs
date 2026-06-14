use crate::*;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Vue27ScriptSetupAnalysis {
    pub(crate) module_content: String,
    pub(crate) hoisted_module_content: String,
    pub(crate) module_chunks: Vec<Vue27ModuleChunk>,
    pub(crate) setup_content: String,
    pub(crate) setup_prelude: String,
    pub(crate) return_bindings: Vec<String>,
    pub(crate) imports: Vec<Vue27ScriptImport>,
    pub(crate) removed_bindings: Vec<String>,
    pub(crate) normal_imports: Vec<Vue27ScriptImport>,
    pub(crate) local_setup_bindings: BTreeSet<String>,
    pub(crate) setup_bindings: BTreeMap<String, String>,
    pub(crate) props_bindings: Vec<String>,
    pub(crate) props_runtime: Option<String>,
    pub(crate) props_type_runtime: bool,
    pub(crate) errors: Vec<String>,
    pub(crate) props_type_source: Option<String>,
    pub(crate) props_runtime_defaults: Option<Vue27RuntimeDefaults>,
    pub(crate) emits_runtime: Option<String>,
    pub(crate) emit_binding: Option<String>,
    pub(crate) emit_type_source: Option<String>,
    pub(crate) needs_expose: bool,
    pub(crate) user_import_aliases: BTreeMap<String, String>,
    pub(crate) declared_types: BTreeMap<String, Vec<String>>,
    pub(crate) props_type_declarations: BTreeMap<String, Vue27TypeMembers>,
    pub(crate) emits_type_declarations: BTreeMap<String, Vue27EmitsType>,
    pub(crate) needs_merge_defaults: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Vue27RuntimeProp {
    pub(crate) key: String,
    pub(crate) types: Vec<String>,
    pub(crate) required: bool,
    pub(crate) default: Option<String>,
    pub(crate) is_method: bool,
    pub(crate) type_annotation_source: Option<String>,
    pub(crate) member_source: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Vue27RuntimeDefaults {
    pub(crate) source: String,
    pub(crate) static_defaults: Option<BTreeMap<String, String>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Vue27TypeMembers {
    pub(crate) source: String,
    pub(crate) members: Vec<Vue27RuntimeProp>,
    pub(crate) errors: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Vue3PropsTypeResolveMode {
    Silent,
    Consumed,
}

pub(crate) type Vue3RuntimeTypeTuple = Vec<Vec<String>>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Vue3GenericTypeAlias {
    pub(crate) source: String,
    pub(crate) kind: Vue3GenericTypeAliasKind,
    pub(crate) params: Vec<String>,
    pub(crate) declared_types: BTreeMap<String, Vec<String>>,
    pub(crate) define_model_declared_types: BTreeMap<String, Vec<String>>,
    pub(crate) type_query_declared_types: BTreeMap<String, Vec<String>>,
    pub(crate) define_model_type_query_declared_types: BTreeMap<String, Vec<String>>,
    pub(crate) keyof_type_query_declared_types: BTreeMap<String, Vec<String>>,
    pub(crate) props_type_declarations: BTreeMap<String, Vue27TypeMembers>,
    pub(crate) keyof_runtime_type_declarations: BTreeMap<String, Vec<String>>,
    pub(crate) tuple_runtime_type_declarations: BTreeMap<String, Vue3RuntimeTypeTuple>,
    pub(crate) define_model_tuple_runtime_type_declarations: BTreeMap<String, Vue3RuntimeTypeTuple>,
    pub(crate) array_element_runtime_type_declarations: BTreeMap<String, Vec<String>>,
    pub(crate) define_model_array_element_runtime_type_declarations: BTreeMap<String, Vec<String>>,
    pub(crate) parameter_tuple_runtime_type_declarations: BTreeMap<String, Vue3RuntimeTypeTuple>,
    pub(crate) define_model_parameter_tuple_runtime_type_declarations:
        BTreeMap<String, Vue3RuntimeTypeTuple>,
    pub(crate) constructor_parameter_tuple_runtime_type_declarations:
        BTreeMap<String, Vue3RuntimeTypeTuple>,
    pub(crate) define_model_constructor_parameter_tuple_runtime_type_declarations:
        BTreeMap<String, Vue3RuntimeTypeTuple>,
    pub(crate) return_type_runtime_type_declarations: BTreeMap<String, Vec<String>>,
    pub(crate) define_model_return_type_runtime_type_declarations: BTreeMap<String, Vec<String>>,
    pub(crate) props_options_type_declarations: BTreeMap<String, Vue27TypeMembers>,
    pub(crate) return_type_props_options_declarations: BTreeMap<String, Vue27TypeMembers>,
    pub(crate) string_literal_type_declarations: BTreeMap<String, BTreeSet<String>>,
    pub(crate) ordered_string_literal_type_declarations: BTreeMap<String, Vec<String>>,
    pub(crate) unresolved_import_sources: BTreeMap<String, String>,
    pub(crate) silent_unresolved_type_names: BTreeSet<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Vue3GenericTypeAliasKind {
    TypeAlias,
    Interface,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Vue27EmitsType {
    pub(crate) source: String,
    pub(crate) events: Vec<String>,
    pub(crate) syntax: Vue3EmitsTypeSyntax,
    pub(crate) call_count: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Vue3EmitsTypeSyntax {
    pub(crate) has_call_signature: bool,
    pub(crate) has_property: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Vue27TypeContext {
    pub(crate) declared_types: BTreeMap<String, Vec<String>>,
    pub(crate) define_model_declared_types: BTreeMap<String, Vec<String>>,
    pub(crate) type_query_declared_types: BTreeMap<String, Vec<String>>,
    pub(crate) define_model_type_query_declared_types: BTreeMap<String, Vec<String>>,
    pub(crate) keyof_type_query_declared_types: BTreeMap<String, Vec<String>>,
    pub(crate) props_type_declarations: BTreeMap<String, Vue27TypeMembers>,
    pub(crate) keyof_runtime_type_declarations: BTreeMap<String, Vec<String>>,
    pub(crate) tuple_runtime_type_declarations: BTreeMap<String, Vue3RuntimeTypeTuple>,
    pub(crate) define_model_tuple_runtime_type_declarations: BTreeMap<String, Vue3RuntimeTypeTuple>,
    pub(crate) array_element_runtime_type_declarations: BTreeMap<String, Vec<String>>,
    pub(crate) define_model_array_element_runtime_type_declarations: BTreeMap<String, Vec<String>>,
    pub(crate) parameter_tuple_runtime_type_declarations: BTreeMap<String, Vue3RuntimeTypeTuple>,
    pub(crate) define_model_parameter_tuple_runtime_type_declarations:
        BTreeMap<String, Vue3RuntimeTypeTuple>,
    pub(crate) constructor_parameter_tuple_runtime_type_declarations:
        BTreeMap<String, Vue3RuntimeTypeTuple>,
    pub(crate) define_model_constructor_parameter_tuple_runtime_type_declarations:
        BTreeMap<String, Vue3RuntimeTypeTuple>,
    pub(crate) return_type_runtime_type_declarations: BTreeMap<String, Vec<String>>,
    pub(crate) define_model_return_type_runtime_type_declarations: BTreeMap<String, Vec<String>>,
    pub(crate) props_options_type_declarations: BTreeMap<String, Vue27TypeMembers>,
    pub(crate) return_type_props_options_declarations: BTreeMap<String, Vue27TypeMembers>,
    pub(crate) generic_type_aliases: BTreeMap<String, Vue3GenericTypeAlias>,
    pub(crate) string_literal_type_declarations: BTreeMap<String, BTreeSet<String>>,
    pub(crate) ordered_string_literal_type_declarations: BTreeMap<String, Vec<String>>,
    pub(crate) emits_type_declarations: BTreeMap<String, Vue27EmitsType>,
    pub(crate) type_sources: BTreeMap<String, String>,
    pub(crate) type_direct_deps: BTreeMap<String, Vec<String>>,
    pub(crate) type_deps: BTreeMap<String, BTreeSet<String>>,
    pub(crate) unresolved_import_sources: BTreeMap<String, String>,
    pub(crate) silent_unresolved_type_names: BTreeSet<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Vue3TypeResolverContext {
    pub(crate) typescript_version: nodejs_semver::Version,
}

impl Default for Vue3TypeResolverContext {
    fn default() -> Self {
        Self {
            typescript_version: vue3_package_typescript_baseline_version(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Vue27ScriptSetupContext {
    pub(crate) normal_types: Vue27TypeContext,
    pub(crate) normal_imports: Vec<Vue27ScriptImport>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Vue27ScriptImport {
    pub(crate) local: String,
    pub(crate) source: String,
    pub(crate) imported: String,
    pub(crate) is_type: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Vue27ModuleChunk {
    pub(crate) start: usize,
    pub(crate) content: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Vue27ScriptReturnBindings {
    pub(crate) bindings: Vec<String>,
    pub(crate) imports: Vec<Vue27ScriptImport>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Vue27NormalScriptAnalysis {
    pub(crate) module_content: String,
    pub(crate) has_default_export: bool,
    pub(crate) has_default_export_name: bool,
}

pub(crate) fn analyze_vue27_script_setup(
    script_setup: &SfcBlock,
    is_prod: bool,
    setup_context: &Vue27ScriptSetupContext,
) -> Vue27ScriptSetupAnalysis {
    let source = script_setup.content.as_str();
    let is_ts = script_is_typescript(&script_setup.attrs);
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        source,
        script_source_type_from_attrs(&script_setup.attrs),
    )
    .with_options(oxc_parser::ParseOptions {
        parse_regular_expression: true,
        ..oxc_parser::ParseOptions::default()
    })
    .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return Vue27ScriptSetupAnalysis {
            setup_content: source.to_string(),
            ..Vue27ScriptSetupAnalysis::default()
        };
    }

    let mut edits = SourceEdits::new(source);
    let mut analysis = Vue27ScriptSetupAnalysis::default();
    analysis.normal_imports = setup_context.normal_imports.clone();
    analysis
        .declared_types
        .extend(setup_context.normal_types.declared_types.clone());
    analysis
        .props_type_declarations
        .extend(setup_context.normal_types.props_type_declarations.clone());
    analysis
        .emits_type_declarations
        .extend(setup_context.normal_types.emits_type_declarations.clone());
    collect_vue27_declared_types_from_statements(source, &parsed.program.body, &mut analysis);
    collect_vue27_setup_local_bindings(&parsed.program.body, is_ts, &mut analysis);
    for statement in &parsed.program.body {
        match statement {
            Statement::ImportDeclaration(import) => {
                let source_value = import.source.value.as_str();
                let mut kept_specifiers = Vec::new();
                let (statement_start, statement_end) =
                    vue27_statement_span_with_trailing_ws(source, statement);
                let statement_end = vue27_statement_span_with_trailing_comments(
                    source,
                    statement_end,
                    &parsed.program.comments,
                );
                if let Some(specifiers) = &import.specifiers {
                    for specifier in specifiers {
                        let local = import_specifier_local(specifier);
                        let imported = import_specifier_imported(specifier);
                        let dedupe_imported = import_specifier_setup_dedupe_imported(specifier);
                        if source_value == "vue" {
                            if let Some(imported) = dedupe_imported.as_deref() {
                                analysis
                                    .user_import_aliases
                                    .insert(imported.to_string(), local.clone());
                            }
                        }
                        if source_value == "vue"
                            && matches!(
                                imported.as_deref(),
                                Some("defineProps" | "defineEmits" | "defineExpose")
                            )
                        {
                            analysis.removed_bindings.push(local);
                        } else if vue27_import_already_declared_in_setup_context(
                            &analysis,
                            source_value,
                            &local,
                            dedupe_imported.as_deref(),
                        ) {
                            analysis.imports.push(Vue27ScriptImport {
                                local: local.clone(),
                                source: source_value.to_string(),
                                imported: imported.unwrap_or_else(|| "default".into()),
                                is_type: vue27_import_specifier_is_type(import, specifier),
                            });
                        } else if vue27_import_local_conflicts_in_setup_context(
                            &analysis,
                            source_value,
                            &local,
                            dedupe_imported.as_deref(),
                        ) {
                            analysis
                                .errors
                                .push("different imports aliased to same local name.".to_string());
                        } else {
                            if source_value == "vue" {
                                analysis
                                    .setup_bindings
                                    .insert(local.clone(), "setup-const".into());
                            } else {
                                analysis
                                    .setup_bindings
                                    .insert(local.clone(), "setup-maybe-ref".into());
                            }
                            analysis.imports.push(Vue27ScriptImport {
                                local: local.clone(),
                                source: source_value.to_string(),
                                imported: imported.unwrap_or_else(|| "default".into()),
                                is_type: vue27_import_specifier_is_type(import, specifier),
                            });
                            kept_specifiers.push(import_specifier_source(source, specifier));
                        }
                    }
                }
                if import.specifiers.is_none() {
                    if let Some(import_source) = source.get(statement_start..statement_end) {
                        analysis.module_chunks.push(Vue27ModuleChunk {
                            start: statement_start,
                            content: import_source.to_string(),
                        });
                    }
                    edits.remove(statement_start, statement_end);
                } else if kept_specifiers.is_empty() {
                    edits.remove(statement_start, statement_end);
                } else if kept_specifiers.len()
                    < import
                        .specifiers
                        .as_ref()
                        .map_or(0, |specifiers| specifiers.len())
                {
                    let trailing = source
                        .get(statement.span().end as usize..statement_end)
                        .unwrap_or_default();
                    analysis.module_chunks.push(Vue27ModuleChunk {
                        start: statement_start,
                        content: format!(
                            "import {{ {} }} from '{}'{}",
                            kept_specifiers.join(", "),
                            source_value,
                            trailing
                        ),
                    });
                    edits.remove(statement_start, statement_end);
                } else {
                    if let Some(import_source) = source.get(statement_start..statement_end) {
                        analysis.module_chunks.push(Vue27ModuleChunk {
                            start: statement_start,
                            content: import_source.to_string(),
                        });
                    }
                    edits.remove(statement_start, statement_end);
                }
            }
            Statement::ExportNamedDeclaration(declaration)
                if declaration.export_kind != ImportOrExportKind::Type =>
            {
                analysis
                    .errors
                    .push(vue27_script_setup_module_export_error());
            }
            Statement::ExportAllDeclaration(_) | Statement::ExportDefaultDeclaration(_) => {
                analysis
                    .errors
                    .push(vue27_script_setup_module_export_error());
            }
            Statement::VariableDeclaration(declaration) => {
                analyze_vue27_setup_variable_declaration(
                    source,
                    declaration,
                    &mut edits,
                    &mut analysis,
                    is_prod,
                );
            }
            Statement::FunctionDeclaration(function) => {
                if let Some(id) = &function.id {
                    push_unique(&mut analysis.return_bindings, id.name.as_str());
                    analysis
                        .setup_bindings
                        .insert(id.name.to_string(), "setup-const".into());
                }
            }
            Statement::ClassDeclaration(class) => {
                if let Some(id) = &class.id {
                    push_unique(&mut analysis.return_bindings, id.name.as_str());
                    analysis
                        .setup_bindings
                        .insert(id.name.to_string(), "setup-const".into());
                }
            }
            Statement::TSEnumDeclaration(declaration) if is_ts && !declaration.declare => {
                hoist_vue27_setup_statement(source, statement, &mut edits, &mut analysis);
                push_unique(&mut analysis.return_bindings, declaration.id.name.as_str());
                analysis
                    .setup_bindings
                    .insert(declaration.id.name.to_string(), "setup-const".into());
            }
            Statement::ExpressionStatement(statement) => {
                if let Expression::CallExpression(call) = &statement.expression {
                    if is_call_named(call, "defineProps") {
                        collect_define_props_call(source, call, None, &mut analysis, is_prod);
                        edits.remove(statement.span.start as usize, statement.span.end as usize);
                    } else if is_call_named(call, "withDefaults")
                        && collect_with_defaults_call(source, call, None, &mut analysis, is_prod)
                    {
                        edits.remove(statement.span.start as usize, statement.span.end as usize);
                    } else if is_call_named(call, "defineEmits") {
                        collect_define_emits_call(source, call, None, &mut analysis);
                        edits.remove(statement.span.start as usize, statement.span.end as usize);
                    } else if is_call_named(call, "defineExpose") {
                        analysis.needs_expose = true;
                        edits.overwrite(
                            call.span.start as usize,
                            call.callee.span().end as usize,
                            "expose",
                        );
                    }
                }
            }
            _ if is_ts && vue27_statement_is_type_hoist(statement) => {
                hoist_vue27_setup_statement(source, statement, &mut edits, &mut analysis);
            }
            _ => {}
        }
    }
    let content = edits.apply();
    let (module_content, setup_content) = split_vue27_setup_module_content(&content);
    if !module_content.is_empty() {
        analysis.module_chunks.push(Vue27ModuleChunk {
            start: usize::MAX,
            content: module_content,
        });
    }
    analysis.module_chunks.sort_by_key(|chunk| chunk.start);
    analysis.module_content = analysis
        .module_chunks
        .iter()
        .map(|chunk| chunk.content.as_str())
        .collect::<String>();
    analysis.setup_content = setup_content;
    if analysis.module_content.ends_with('\n') {
        if let Some(indent) = leading_blank_line_indent(&analysis.setup_content) {
            analysis.module_content.push_str(indent);
            analysis.setup_content = analysis.setup_content[indent.len()..].to_string();
        }
    }
    analysis
}

pub(crate) fn collect_vue27_declared_types_from_statements(
    source: &str,
    statements: &[Statement<'_>],
    analysis: &mut Vue27ScriptSetupAnalysis,
) {
    for statement in statements {
        collect_vue27_declared_type_from_statement(source, statement, analysis);
    }
}

pub(crate) fn collect_vue27_setup_local_bindings(
    statements: &[Statement<'_>],
    is_ts: bool,
    analysis: &mut Vue27ScriptSetupAnalysis,
) {
    for statement in statements {
        match statement {
            Statement::VariableDeclaration(declaration) if !declaration.declare => {
                for declarator in &declaration.declarations {
                    insert_pattern_bindings(&declarator.id, &mut analysis.local_setup_bindings);
                }
            }
            Statement::FunctionDeclaration(function) if !function.declare => {
                if let Some(id) = &function.id {
                    analysis.local_setup_bindings.insert(id.name.to_string());
                }
            }
            Statement::ClassDeclaration(class) if !class.declare => {
                if let Some(id) = &class.id {
                    analysis.local_setup_bindings.insert(id.name.to_string());
                }
            }
            Statement::TSEnumDeclaration(declaration) if is_ts && !declaration.declare => {
                analysis
                    .local_setup_bindings
                    .insert(declaration.id.name.to_string());
            }
            _ => {}
        }
    }
}

pub(crate) fn collect_vue27_declared_type_from_statement(
    source: &str,
    statement: &Statement<'_>,
    analysis: &mut Vue27ScriptSetupAnalysis,
) {
    match statement {
        Statement::TSInterfaceDeclaration(declaration) => {
            analysis
                .declared_types
                .insert(declaration.id.name.to_string(), vec!["Object".into()]);
            let props = vue27_type_members_from_interface_body(source, &declaration.body, analysis);
            analysis
                .props_type_declarations
                .insert(declaration.id.name.to_string(), props);
            let emits = vue27_emits_type_from_interface_body(source, &declaration.body);
            if !emits.events.is_empty() {
                analysis
                    .emits_type_declarations
                    .insert(declaration.id.name.to_string(), emits);
            }
        }
        Statement::TSTypeAliasDeclaration(declaration) => {
            let runtime = infer_vue27_runtime_type(&declaration.type_annotation, analysis);
            analysis
                .declared_types
                .insert(declaration.id.name.to_string(), runtime);
            match &declaration.type_annotation {
                TSType::TSTypeLiteral(literal) => {
                    let props = vue27_type_members_from_literal(source, literal, analysis);
                    analysis
                        .props_type_declarations
                        .insert(declaration.id.name.to_string(), props);
                    let emits = vue27_emits_type_from_literal(source, literal);
                    if !emits.events.is_empty() {
                        analysis
                            .emits_type_declarations
                            .insert(declaration.id.name.to_string(), emits);
                    }
                }
                TSType::TSFunctionType(function) => {
                    let emits = vue27_emits_type_from_function(source, function);
                    if !emits.events.is_empty() {
                        analysis
                            .emits_type_declarations
                            .insert(declaration.id.name.to_string(), emits);
                    }
                }
                _ => {}
            }
        }
        Statement::ExportNamedDeclaration(declaration) => {
            if let Some(declaration) = &declaration.declaration {
                collect_vue27_declared_type_from_declaration(source, declaration, analysis);
            }
        }
        _ => {}
    }
}

pub(crate) fn collect_vue27_declared_type_from_declaration(
    source: &str,
    declaration: &Declaration<'_>,
    analysis: &mut Vue27ScriptSetupAnalysis,
) {
    match declaration {
        Declaration::TSInterfaceDeclaration(declaration) => {
            analysis
                .declared_types
                .insert(declaration.id.name.to_string(), vec!["Object".into()]);
            let props = vue27_type_members_from_interface_body(source, &declaration.body, analysis);
            analysis
                .props_type_declarations
                .insert(declaration.id.name.to_string(), props);
            let emits = vue27_emits_type_from_interface_body(source, &declaration.body);
            if !emits.events.is_empty() {
                analysis
                    .emits_type_declarations
                    .insert(declaration.id.name.to_string(), emits);
            }
        }
        Declaration::TSTypeAliasDeclaration(declaration) => {
            let runtime = infer_vue27_runtime_type(&declaration.type_annotation, analysis);
            analysis
                .declared_types
                .insert(declaration.id.name.to_string(), runtime);
            match &declaration.type_annotation {
                TSType::TSTypeLiteral(literal) => {
                    let props = vue27_type_members_from_literal(source, literal, analysis);
                    analysis
                        .props_type_declarations
                        .insert(declaration.id.name.to_string(), props);
                    let emits = vue27_emits_type_from_literal(source, literal);
                    if !emits.events.is_empty() {
                        analysis
                            .emits_type_declarations
                            .insert(declaration.id.name.to_string(), emits);
                    }
                }
                TSType::TSFunctionType(function) => {
                    let emits = vue27_emits_type_from_function(source, function);
                    if !emits.events.is_empty() {
                        analysis
                            .emits_type_declarations
                            .insert(declaration.id.name.to_string(), emits);
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }
}

pub(crate) fn vue27_normal_script_type_context(descriptor: &SfcDescriptor) -> Vue27TypeContext {
    let Some(script) = descriptor.script.as_ref() else {
        return Vue27TypeContext::default();
    };
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        script.content.as_str(),
        script_source_type_from_attrs(&script.attrs),
    )
    .with_options(oxc_parser::ParseOptions {
        parse_regular_expression: true,
        ..oxc_parser::ParseOptions::default()
    })
    .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return Vue27TypeContext::default();
    }
    let mut analysis = Vue27ScriptSetupAnalysis::default();
    collect_vue27_declared_types_from_statements(
        script.content.as_str(),
        &parsed.program.body,
        &mut analysis,
    );
    Vue27TypeContext {
        declared_types: analysis.declared_types,
        define_model_declared_types: BTreeMap::new(),
        type_query_declared_types: BTreeMap::new(),
        define_model_type_query_declared_types: BTreeMap::new(),
        keyof_type_query_declared_types: BTreeMap::new(),
        props_type_declarations: analysis.props_type_declarations,
        keyof_runtime_type_declarations: BTreeMap::new(),
        tuple_runtime_type_declarations: BTreeMap::new(),
        define_model_tuple_runtime_type_declarations: BTreeMap::new(),
        array_element_runtime_type_declarations: BTreeMap::new(),
        define_model_array_element_runtime_type_declarations: BTreeMap::new(),
        parameter_tuple_runtime_type_declarations: BTreeMap::new(),
        define_model_parameter_tuple_runtime_type_declarations: BTreeMap::new(),
        constructor_parameter_tuple_runtime_type_declarations: BTreeMap::new(),
        define_model_constructor_parameter_tuple_runtime_type_declarations: BTreeMap::new(),
        return_type_runtime_type_declarations: BTreeMap::new(),
        define_model_return_type_runtime_type_declarations: BTreeMap::new(),
        props_options_type_declarations: BTreeMap::new(),
        return_type_props_options_declarations: BTreeMap::new(),
        generic_type_aliases: BTreeMap::new(),
        string_literal_type_declarations: BTreeMap::new(),
        ordered_string_literal_type_declarations: BTreeMap::new(),
        emits_type_declarations: analysis.emits_type_declarations,
        type_sources: BTreeMap::new(),
        type_direct_deps: BTreeMap::new(),
        type_deps: BTreeMap::new(),
        unresolved_import_sources: BTreeMap::new(),
        silent_unresolved_type_names: BTreeSet::new(),
    }
}

pub(crate) fn vue3_normal_script_type_context(
    descriptor: &SfcDescriptor,
    global_type_files: &[String],
    type_resolver: &Vue3TypeResolverContext,
) -> Vue27TypeContext {
    let mut context =
        vue3_global_type_context(&descriptor.filename, global_type_files, type_resolver);
    let Some(script) = descriptor.script.as_ref() else {
        return context;
    };
    extend_vue3_type_context_from_external_imports(
        &descriptor.filename,
        script.content.as_str(),
        script_source_type_from_attrs(&script.attrs),
        &mut context,
        type_resolver,
    );
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        script.content.as_str(),
        script_source_type_from_attrs(&script.attrs),
    )
    .with_options(oxc_parser::ParseOptions {
        parse_regular_expression: true,
        ..oxc_parser::ParseOptions::default()
    })
    .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return context;
    }
    let mut analysis = Vue3ScriptSetupAnalysis {
        declared_types: context.declared_types,
        define_model_declared_types: context.define_model_declared_types,
        type_query_declared_types: context.type_query_declared_types,
        define_model_type_query_declared_types: context.define_model_type_query_declared_types,
        keyof_type_query_declared_types: context.keyof_type_query_declared_types,
        props_type_declarations: context.props_type_declarations,
        keyof_runtime_type_declarations: context.keyof_runtime_type_declarations,
        tuple_runtime_type_declarations: context.tuple_runtime_type_declarations,
        define_model_tuple_runtime_type_declarations: context
            .define_model_tuple_runtime_type_declarations,
        array_element_runtime_type_declarations: context.array_element_runtime_type_declarations,
        define_model_array_element_runtime_type_declarations: context
            .define_model_array_element_runtime_type_declarations,
        parameter_tuple_runtime_type_declarations: context
            .parameter_tuple_runtime_type_declarations,
        define_model_parameter_tuple_runtime_type_declarations: context
            .define_model_parameter_tuple_runtime_type_declarations,
        constructor_parameter_tuple_runtime_type_declarations: context
            .constructor_parameter_tuple_runtime_type_declarations,
        define_model_constructor_parameter_tuple_runtime_type_declarations: context
            .define_model_constructor_parameter_tuple_runtime_type_declarations,
        return_type_runtime_type_declarations: context.return_type_runtime_type_declarations,
        define_model_return_type_runtime_type_declarations: context
            .define_model_return_type_runtime_type_declarations,
        props_options_type_declarations: context.props_options_type_declarations,
        return_type_props_options_declarations: context.return_type_props_options_declarations,
        generic_type_aliases: context.generic_type_aliases,
        string_literal_type_declarations: context.string_literal_type_declarations,
        ordered_string_literal_type_declarations: context.ordered_string_literal_type_declarations,
        emits_type_declarations: context.emits_type_declarations,
        type_sources: context.type_sources,
        type_direct_deps: context.type_direct_deps,
        type_deps: context.type_deps,
        unresolved_import_sources: context.unresolved_import_sources,
        silent_unresolved_type_names: context.silent_unresolved_type_names,
        type_filename: Some(descriptor.filename.clone()),
        type_resolver: type_resolver.clone(),
        ..Vue3ScriptSetupAnalysis::default()
    };
    collect_vue3_declared_types_from_statements(
        script.content.as_str(),
        &parsed.program.body,
        &mut analysis,
    );
    collect_vue3_declared_type_deps_from_statements(&parsed.program.body, &mut analysis);
    Vue27TypeContext {
        declared_types: analysis.declared_types,
        define_model_declared_types: analysis.define_model_declared_types,
        type_query_declared_types: analysis.type_query_declared_types,
        define_model_type_query_declared_types: analysis.define_model_type_query_declared_types,
        keyof_type_query_declared_types: analysis.keyof_type_query_declared_types,
        props_type_declarations: analysis.props_type_declarations,
        keyof_runtime_type_declarations: analysis.keyof_runtime_type_declarations,
        tuple_runtime_type_declarations: analysis.tuple_runtime_type_declarations,
        define_model_tuple_runtime_type_declarations: analysis
            .define_model_tuple_runtime_type_declarations,
        array_element_runtime_type_declarations: analysis.array_element_runtime_type_declarations,
        define_model_array_element_runtime_type_declarations: analysis
            .define_model_array_element_runtime_type_declarations,
        parameter_tuple_runtime_type_declarations: analysis
            .parameter_tuple_runtime_type_declarations,
        define_model_parameter_tuple_runtime_type_declarations: analysis
            .define_model_parameter_tuple_runtime_type_declarations,
        constructor_parameter_tuple_runtime_type_declarations: analysis
            .constructor_parameter_tuple_runtime_type_declarations,
        define_model_constructor_parameter_tuple_runtime_type_declarations: analysis
            .define_model_constructor_parameter_tuple_runtime_type_declarations,
        return_type_runtime_type_declarations: analysis.return_type_runtime_type_declarations,
        define_model_return_type_runtime_type_declarations: analysis
            .define_model_return_type_runtime_type_declarations,
        props_options_type_declarations: analysis.props_options_type_declarations,
        return_type_props_options_declarations: analysis.return_type_props_options_declarations,
        generic_type_aliases: analysis.generic_type_aliases,
        string_literal_type_declarations: analysis.string_literal_type_declarations,
        ordered_string_literal_type_declarations: analysis.ordered_string_literal_type_declarations,
        emits_type_declarations: analysis.emits_type_declarations,
        type_sources: analysis.type_sources,
        type_direct_deps: analysis.type_direct_deps,
        type_deps: analysis.type_deps,
        unresolved_import_sources: analysis.unresolved_import_sources,
        silent_unresolved_type_names: analysis.silent_unresolved_type_names,
    }
}

pub(crate) fn vue3_global_type_context(
    filename: &str,
    global_type_files: &[String],
    type_resolver: &Vue3TypeResolverContext,
) -> Vue27TypeContext {
    let mut context = Vue27TypeContext::default();
    for file in global_type_files {
        let path = normalize_path_components(PathBuf::from(file));
        let Some(global_context) =
            vue3_global_type_context_from_path(&path, &context, type_resolver)
        else {
            continue;
        };
        merge_vue3_type_context_missing(&mut context, global_context);
    }
    for path in vue3_tsconfig_global_type_files(filename, type_resolver) {
        let Some(global_context) =
            vue3_global_type_context_from_path(&path, &context, type_resolver)
        else {
            continue;
        };
        merge_vue3_type_context_missing(&mut context, global_context);
    }
    context
}

pub(crate) fn vue3_global_type_context_from_path(
    path: &Path,
    base_context: &Vue27TypeContext,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<Vue27TypeContext> {
    let source = vue3_external_type_source_from_path(path)?;
    let normalized = normalize_path_string(path);
    Some(vue3_global_type_context_from_source(
        &source.source,
        &normalized,
        source.source_type,
        base_context,
        type_resolver,
    ))
}

pub(crate) fn vue3_global_type_context_from_source(
    source: &str,
    filename: &str,
    source_type: oxc_span::SourceType,
    base_context: &Vue27TypeContext,
    type_resolver: &Vue3TypeResolverContext,
) -> Vue27TypeContext {
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, source, source_type)
        .with_options(oxc_parser::ParseOptions {
            parse_regular_expression: true,
            ..oxc_parser::ParseOptions::default()
        })
        .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return Vue27TypeContext::default();
    }

    let dependency = normalize_path_string(Path::new(filename));
    let mut seed_context = base_context.clone();
    extend_vue3_type_context_from_external_imports(
        filename,
        source,
        source_type,
        &mut seed_context,
        type_resolver,
    );
    let mut analysis = Vue3ScriptSetupAnalysis {
        declared_types: seed_context.declared_types,
        define_model_declared_types: seed_context.define_model_declared_types,
        type_query_declared_types: seed_context.type_query_declared_types,
        define_model_type_query_declared_types: seed_context.define_model_type_query_declared_types,
        keyof_type_query_declared_types: seed_context.keyof_type_query_declared_types,
        props_type_declarations: seed_context.props_type_declarations,
        keyof_runtime_type_declarations: seed_context.keyof_runtime_type_declarations,
        tuple_runtime_type_declarations: seed_context.tuple_runtime_type_declarations,
        define_model_tuple_runtime_type_declarations: seed_context
            .define_model_tuple_runtime_type_declarations,
        array_element_runtime_type_declarations: seed_context
            .array_element_runtime_type_declarations,
        define_model_array_element_runtime_type_declarations: seed_context
            .define_model_array_element_runtime_type_declarations,
        parameter_tuple_runtime_type_declarations: seed_context
            .parameter_tuple_runtime_type_declarations,
        define_model_parameter_tuple_runtime_type_declarations: seed_context
            .define_model_parameter_tuple_runtime_type_declarations,
        constructor_parameter_tuple_runtime_type_declarations: seed_context
            .constructor_parameter_tuple_runtime_type_declarations,
        define_model_constructor_parameter_tuple_runtime_type_declarations: seed_context
            .define_model_constructor_parameter_tuple_runtime_type_declarations,
        return_type_runtime_type_declarations: seed_context.return_type_runtime_type_declarations,
        define_model_return_type_runtime_type_declarations: seed_context
            .define_model_return_type_runtime_type_declarations,
        props_options_type_declarations: seed_context.props_options_type_declarations,
        return_type_props_options_declarations: seed_context.return_type_props_options_declarations,
        generic_type_aliases: seed_context.generic_type_aliases,
        string_literal_type_declarations: seed_context.string_literal_type_declarations,
        ordered_string_literal_type_declarations: seed_context
            .ordered_string_literal_type_declarations,
        emits_type_declarations: seed_context.emits_type_declarations,
        type_sources: seed_context.type_sources,
        type_direct_deps: seed_context.type_direct_deps,
        type_deps: seed_context.type_deps,
        unresolved_import_sources: seed_context.unresolved_import_sources,
        silent_unresolved_type_names: seed_context.silent_unresolved_type_names,
        type_filename: Some(filename.to_string()),
        type_resolver: type_resolver.clone(),
        ..Vue3ScriptSetupAnalysis::default()
    };
    let mut global_names =
        collect_vue3_global_types_from_statements(source, &parsed.program.body, &mut analysis);
    global_names.extend(project_vue3_global_type_re_exports(
        filename,
        &parsed.program.body,
        &mut analysis,
        type_resolver,
    ));
    let global_import_names = vue3_global_type_file_import_names(&parsed.program.body);
    collect_vue3_global_type_deps_from_statements(&parsed.program.body, &mut analysis);
    seed_vue3_external_type_deps(filename, &mut analysis);
    let mut context = Vue27TypeContext {
        declared_types: analysis.declared_types,
        define_model_declared_types: analysis.define_model_declared_types,
        type_query_declared_types: analysis.type_query_declared_types,
        define_model_type_query_declared_types: analysis.define_model_type_query_declared_types,
        keyof_type_query_declared_types: analysis.keyof_type_query_declared_types,
        props_type_declarations: analysis.props_type_declarations,
        keyof_runtime_type_declarations: analysis.keyof_runtime_type_declarations,
        tuple_runtime_type_declarations: analysis.tuple_runtime_type_declarations,
        define_model_tuple_runtime_type_declarations: analysis
            .define_model_tuple_runtime_type_declarations,
        array_element_runtime_type_declarations: analysis.array_element_runtime_type_declarations,
        define_model_array_element_runtime_type_declarations: analysis
            .define_model_array_element_runtime_type_declarations,
        parameter_tuple_runtime_type_declarations: analysis
            .parameter_tuple_runtime_type_declarations,
        define_model_parameter_tuple_runtime_type_declarations: analysis
            .define_model_parameter_tuple_runtime_type_declarations,
        constructor_parameter_tuple_runtime_type_declarations: analysis
            .constructor_parameter_tuple_runtime_type_declarations,
        define_model_constructor_parameter_tuple_runtime_type_declarations: analysis
            .define_model_constructor_parameter_tuple_runtime_type_declarations,
        return_type_runtime_type_declarations: analysis.return_type_runtime_type_declarations,
        define_model_return_type_runtime_type_declarations: analysis
            .define_model_return_type_runtime_type_declarations,
        props_options_type_declarations: analysis.props_options_type_declarations,
        return_type_props_options_declarations: analysis.return_type_props_options_declarations,
        generic_type_aliases: analysis.generic_type_aliases,
        string_literal_type_declarations: analysis.string_literal_type_declarations,
        ordered_string_literal_type_declarations: analysis.ordered_string_literal_type_declarations,
        emits_type_declarations: analysis.emits_type_declarations,
        type_sources: analysis.type_sources,
        type_direct_deps: analysis.type_direct_deps,
        type_deps: analysis.type_deps,
        unresolved_import_sources: analysis.unresolved_import_sources,
        silent_unresolved_type_names: analysis.silent_unresolved_type_names,
    };
    retain_vue3_type_context_names(&mut context, &global_names);
    context
        .silent_unresolved_type_names
        .extend(global_import_names);
    for name in vue3_type_context_names(&context) {
        context
            .type_sources
            .entry(name.clone())
            .or_insert_with(|| dependency.clone());
        context
            .type_deps
            .entry(name)
            .or_default()
            .insert(dependency.clone());
    }
    context
}

pub(crate) fn collect_vue3_global_types_from_statements(
    source: &str,
    statements: &[Statement<'_>],
    analysis: &mut Vue3ScriptSetupAnalysis,
) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    seed_vue3_global_namespace_type_names(statements, analysis);
    let is_ambient = !statements.iter().any(|statement| {
        matches!(
            statement,
            Statement::ImportDeclaration(_)
                | Statement::ExportAllDeclaration(_)
                | Statement::ExportDefaultDeclaration(_)
                | Statement::ExportNamedDeclaration(_)
        )
    });
    if is_ambient {
        for statement in statements {
            collect_vue3_predeclared_runtime_type_from_statement(statement, analysis);
        }
        for statement in statements {
            match statement {
                Statement::TSGlobalDeclaration(global) => {
                    names.extend(vue3_declared_type_names_from_statements(&global.body.body));
                    collect_vue3_declared_types_from_statements(
                        source,
                        &global.body.body,
                        analysis,
                    );
                }
                Statement::TSModuleDeclaration(declaration)
                    if vue3_ts_module_declaration_is_global(declaration) =>
                {
                    if let Some(body) = vue3_ts_module_declaration_block_body(declaration) {
                        names.extend(vue3_declared_type_names_from_statements(body));
                        collect_vue3_declared_types_from_statements(source, body, analysis);
                    }
                }
                _ if vue3_statement_is_declare_type(statement) => {
                    names.extend(vue3_declared_type_names_from_statement(statement));
                    collect_vue3_global_declared_type_from_statement(source, statement, analysis);
                }
                _ => {}
            }
        }
        return names;
    }
    for statement in statements {
        let Statement::TSGlobalDeclaration(global) = statement else {
            continue;
        };
        names.extend(vue3_declared_type_names_from_statements(&global.body.body));
        collect_vue3_declared_types_from_statements(source, &global.body.body, analysis);
    }
    names
}

pub(crate) fn project_vue3_global_type_re_exports(
    filename: &str,
    statements: &[Statement<'_>],
    analysis: &mut Vue3ScriptSetupAnalysis,
    type_resolver: &Vue3TypeResolverContext,
) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut seen = BTreeSet::new();
    for statement in statements {
        let Statement::TSGlobalDeclaration(global) = statement else {
            continue;
        };
        names.extend(project_vue3_type_re_exports(
            filename,
            &global.body.body,
            analysis,
            &mut seen,
            type_resolver,
        ));
        project_vue3_exported_type_specifiers(&global.body.body, analysis);
        names.extend(vue3_exported_type_names(&global.body.body));
    }
    names
}

pub(crate) fn vue3_global_type_file_import_names(statements: &[Statement<'_>]) -> BTreeSet<String> {
    if !statements.iter().any(|statement| {
        matches!(
            statement,
            Statement::TSGlobalDeclaration(_)
                | Statement::ExportAllDeclaration(_)
                | Statement::ExportDefaultDeclaration(_)
                | Statement::ExportNamedDeclaration(_)
        )
    }) {
        return BTreeSet::new();
    }
    let mut names = BTreeSet::new();
    for statement in statements {
        let Statement::ImportDeclaration(import) = statement else {
            continue;
        };
        let Some(specifiers) = &import.specifiers else {
            continue;
        };
        for specifier in specifiers {
            names.insert(import_specifier_local(specifier));
        }
    }
    names
}

pub(crate) fn collect_vue3_global_declared_type_from_statement(
    source: &str,
    statement: &Statement<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    match statement {
        Statement::TSEnumDeclaration(declaration) if declaration.declare => {
            register_vue3_ts_enum_declaration(declaration, analysis);
        }
        _ => collect_vue3_declared_type_from_statement(source, statement, analysis),
    }
}

pub(crate) fn collect_vue3_global_type_deps_from_statements(
    statements: &[Statement<'_>],
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    for statement in statements {
        match statement {
            Statement::TSGlobalDeclaration(global) => {
                collect_vue3_declared_type_deps_from_statements(&global.body.body, analysis);
            }
            Statement::TSModuleDeclaration(declaration)
                if vue3_ts_module_declaration_is_global(declaration) =>
            {
                if let Some(body) = vue3_ts_module_declaration_block_body(declaration) {
                    collect_vue3_declared_type_deps_from_statements(body, analysis);
                }
            }
            _ if vue3_statement_is_declare_type(statement) => {
                collect_vue3_declared_type_deps_from_statement(statement, analysis);
            }
            _ => {}
        }
    }
}

pub(crate) fn vue3_statement_is_declare_type(statement: &Statement<'_>) -> bool {
    match statement {
        Statement::TSInterfaceDeclaration(declaration) => declaration.declare,
        Statement::TSTypeAliasDeclaration(declaration) => declaration.declare,
        Statement::TSEnumDeclaration(declaration) => declaration.declare,
        Statement::FunctionDeclaration(function) => function.declare,
        Statement::VariableDeclaration(declaration) => declaration.declare,
        Statement::ClassDeclaration(declaration) => declaration.declare,
        Statement::TSModuleDeclaration(declaration) => declaration.declare,
        _ => false,
    }
}

pub(crate) fn vue3_declared_type_names_from_statements(
    statements: &[Statement<'_>],
) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for statement in statements {
        names.extend(vue3_declared_type_names_from_statement(statement));
    }
    names
}

pub(crate) fn vue3_declared_type_names_from_statement(
    statement: &Statement<'_>,
) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    match statement {
        Statement::TSInterfaceDeclaration(declaration) => {
            names.insert(declaration.id.name.to_string());
        }
        Statement::TSTypeAliasDeclaration(declaration) => {
            names.insert(declaration.id.name.to_string());
        }
        Statement::TSEnumDeclaration(declaration) => {
            names.insert(declaration.id.name.to_string());
        }
        Statement::FunctionDeclaration(function)
            if vue3_function_has_return_projection(function) =>
        {
            if let Some(id) = &function.id {
                names.insert(id.name.to_string());
            }
        }
        Statement::VariableDeclaration(declaration) if declaration.declare => {
            for declarator in &declaration.declarations {
                if let Some(name) = first_pattern_binding(&declarator.id) {
                    names.insert(name);
                }
            }
        }
        Statement::VariableDeclaration(declaration) => {
            for declarator in &declaration.declarations {
                if vue3_variable_declarator_has_type_projection(declarator) {
                    if let Some(name) = first_pattern_binding(&declarator.id) {
                        names.insert(name);
                    }
                }
            }
        }
        Statement::ClassDeclaration(declaration) => {
            if let Some(id) = &declaration.id {
                names.insert(id.name.to_string());
            }
        }
        Statement::TSModuleDeclaration(declaration) => {
            names.extend(vue3_namespace_declared_type_names(declaration));
        }
        Statement::ExportNamedDeclaration(declaration) => {
            if let Some(declaration) = &declaration.declaration {
                names.extend(vue3_declared_type_names_from_declaration(declaration));
            }
        }
        _ => {}
    }
    names
}

pub(crate) fn vue3_declared_type_names_from_declaration(
    declaration: &Declaration<'_>,
) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    match declaration {
        Declaration::TSInterfaceDeclaration(declaration) => {
            names.insert(declaration.id.name.to_string());
        }
        Declaration::TSTypeAliasDeclaration(declaration) => {
            names.insert(declaration.id.name.to_string());
        }
        Declaration::TSEnumDeclaration(declaration) => {
            names.insert(declaration.id.name.to_string());
        }
        Declaration::FunctionDeclaration(function)
            if vue3_function_has_return_projection(function) =>
        {
            if let Some(id) = &function.id {
                names.insert(id.name.to_string());
            }
        }
        Declaration::VariableDeclaration(declaration) if declaration.declare => {
            for declarator in &declaration.declarations {
                if let Some(name) = first_pattern_binding(&declarator.id) {
                    names.insert(name);
                }
            }
        }
        Declaration::VariableDeclaration(declaration) => {
            for declarator in &declaration.declarations {
                if vue3_variable_declarator_has_type_projection(declarator) {
                    if let Some(name) = first_pattern_binding(&declarator.id) {
                        names.insert(name);
                    }
                }
            }
        }
        Declaration::ClassDeclaration(declaration) => {
            if let Some(id) = &declaration.id {
                names.insert(id.name.to_string());
            }
        }
        Declaration::TSModuleDeclaration(declaration) => {
            names.extend(vue3_namespace_declared_type_names(declaration));
        }
        _ => {}
    }
    names
}

pub(crate) fn retain_vue3_type_context_names(
    context: &mut Vue27TypeContext,
    names: &BTreeSet<String>,
) {
    context
        .declared_types
        .retain(|name, _| names.contains(name));
    context
        .define_model_declared_types
        .retain(|name, _| names.contains(name));
    context
        .type_query_declared_types
        .retain(|name, _| names.contains(name));
    context
        .define_model_type_query_declared_types
        .retain(|name, _| names.contains(name));
    context
        .keyof_type_query_declared_types
        .retain(|name, _| names.contains(name));
    context
        .props_type_declarations
        .retain(|name, _| names.contains(name));
    context
        .keyof_runtime_type_declarations
        .retain(|name, _| names.contains(name));
    context
        .tuple_runtime_type_declarations
        .retain(|name, _| names.contains(name));
    context
        .define_model_tuple_runtime_type_declarations
        .retain(|name, _| names.contains(name));
    context
        .array_element_runtime_type_declarations
        .retain(|name, _| names.contains(name));
    context
        .define_model_array_element_runtime_type_declarations
        .retain(|name, _| names.contains(name));
    context
        .parameter_tuple_runtime_type_declarations
        .retain(|name, _| names.contains(name));
    context
        .define_model_parameter_tuple_runtime_type_declarations
        .retain(|name, _| names.contains(name));
    context
        .constructor_parameter_tuple_runtime_type_declarations
        .retain(|name, _| names.contains(name));
    context
        .define_model_constructor_parameter_tuple_runtime_type_declarations
        .retain(|name, _| names.contains(name));
    context
        .return_type_runtime_type_declarations
        .retain(|name, _| names.contains(name));
    context
        .define_model_return_type_runtime_type_declarations
        .retain(|name, _| names.contains(name));
    context
        .props_options_type_declarations
        .retain(|name, _| names.contains(name));
    context
        .return_type_props_options_declarations
        .retain(|name, _| names.contains(name));
    context
        .generic_type_aliases
        .retain(|name, _| names.contains(name));
    context
        .string_literal_type_declarations
        .retain(|name, _| names.contains(name));
    context
        .ordered_string_literal_type_declarations
        .retain(|name, _| names.contains(name));
    context
        .emits_type_declarations
        .retain(|name, _| names.contains(name));
    context.type_sources.retain(|name, _| names.contains(name));
    context
        .type_direct_deps
        .retain(|name, _| names.contains(name));
    context.type_deps.retain(|name, _| names.contains(name));
    context
        .unresolved_import_sources
        .retain(|name, _| names.contains(name));
    context
        .silent_unresolved_type_names
        .retain(|name| names.contains(name));
}

pub(crate) fn merge_vue3_type_context_missing(
    target: &mut Vue27TypeContext,
    source: Vue27TypeContext,
) {
    for (name, runtime) in source.declared_types {
        target.declared_types.entry(name).or_insert(runtime);
    }
    for (name, runtime) in source.define_model_declared_types {
        target
            .define_model_declared_types
            .entry(name)
            .or_insert(runtime);
    }
    for (name, runtime) in source.type_query_declared_types {
        target
            .type_query_declared_types
            .entry(name)
            .or_insert(runtime);
    }
    for (name, runtime) in source.define_model_type_query_declared_types {
        target
            .define_model_type_query_declared_types
            .entry(name)
            .or_insert(runtime);
    }
    for (name, runtime) in source.keyof_type_query_declared_types {
        target
            .keyof_type_query_declared_types
            .entry(name)
            .or_insert(runtime);
    }
    for (name, props) in source.props_type_declarations {
        target.props_type_declarations.entry(name).or_insert(props);
    }
    for (name, types) in source.keyof_runtime_type_declarations {
        target
            .keyof_runtime_type_declarations
            .entry(name)
            .or_insert(types);
    }
    for (name, tuple) in source.tuple_runtime_type_declarations {
        target
            .tuple_runtime_type_declarations
            .entry(name)
            .or_insert(tuple);
    }
    for (name, tuple) in source.define_model_tuple_runtime_type_declarations {
        target
            .define_model_tuple_runtime_type_declarations
            .entry(name)
            .or_insert(tuple);
    }
    for (name, types) in source.array_element_runtime_type_declarations {
        target
            .array_element_runtime_type_declarations
            .entry(name)
            .or_insert(types);
    }
    for (name, types) in source.define_model_array_element_runtime_type_declarations {
        target
            .define_model_array_element_runtime_type_declarations
            .entry(name)
            .or_insert(types);
    }
    for (name, tuple) in source.parameter_tuple_runtime_type_declarations {
        target
            .parameter_tuple_runtime_type_declarations
            .entry(name)
            .or_insert(tuple);
    }
    for (name, tuple) in source.define_model_parameter_tuple_runtime_type_declarations {
        target
            .define_model_parameter_tuple_runtime_type_declarations
            .entry(name)
            .or_insert(tuple);
    }
    for (name, tuple) in source.constructor_parameter_tuple_runtime_type_declarations {
        target
            .constructor_parameter_tuple_runtime_type_declarations
            .entry(name)
            .or_insert(tuple);
    }
    for (name, tuple) in source.define_model_constructor_parameter_tuple_runtime_type_declarations {
        target
            .define_model_constructor_parameter_tuple_runtime_type_declarations
            .entry(name)
            .or_insert(tuple);
    }
    for (name, types) in source.return_type_runtime_type_declarations {
        target
            .return_type_runtime_type_declarations
            .entry(name)
            .or_insert(types);
    }
    for (name, types) in source.define_model_return_type_runtime_type_declarations {
        target
            .define_model_return_type_runtime_type_declarations
            .entry(name)
            .or_insert(types);
    }
    for (name, props_options) in source.props_options_type_declarations {
        target
            .props_options_type_declarations
            .entry(name)
            .or_insert(props_options);
    }
    for (name, props_options) in source.return_type_props_options_declarations {
        target
            .return_type_props_options_declarations
            .entry(name)
            .or_insert(props_options);
    }
    for (name, alias) in source.generic_type_aliases {
        target.generic_type_aliases.entry(name).or_insert(alias);
    }
    for (name, keys) in source.string_literal_type_declarations {
        target
            .string_literal_type_declarations
            .entry(name)
            .or_insert(keys);
    }
    for (name, keys) in source.ordered_string_literal_type_declarations {
        target
            .ordered_string_literal_type_declarations
            .entry(name)
            .or_insert(keys);
    }
    for (name, emits) in source.emits_type_declarations {
        target.emits_type_declarations.entry(name).or_insert(emits);
    }
    for (name, type_source) in source.type_sources {
        target.type_sources.entry(name).or_insert(type_source);
    }
    for (name, deps) in source.type_direct_deps {
        target.type_direct_deps.entry(name).or_insert(deps);
    }
    for (name, deps) in source.type_deps {
        target.type_deps.entry(name).or_insert(deps);
    }
    for (name, import_source) in source.unresolved_import_sources {
        target
            .unresolved_import_sources
            .entry(name)
            .or_insert(import_source);
    }
    target
        .silent_unresolved_type_names
        .extend(source.silent_unresolved_type_names);
}

pub(crate) fn extend_vue3_type_context_from_external_imports(
    filename: &str,
    source: &str,
    source_type: oxc_span::SourceType,
    context: &mut Vue27TypeContext,
    type_resolver: &Vue3TypeResolverContext,
) {
    let mut seen = BTreeSet::new();
    extend_vue3_type_context_from_external_imports_with_seen(
        filename,
        source,
        source_type,
        context,
        &mut seen,
        type_resolver,
    );
}

pub(crate) fn extend_vue3_type_context_from_external_imports_with_seen(
    filename: &str,
    source: &str,
    source_type: oxc_span::SourceType,
    context: &mut Vue27TypeContext,
    seen: &mut BTreeSet<String>,
    type_resolver: &Vue3TypeResolverContext,
) {
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, source, source_type)
        .with_options(oxc_parser::ParseOptions {
            parse_regular_expression: true,
            ..oxc_parser::ParseOptions::default()
        })
        .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return;
    }
    for statement in &parsed.program.body {
        let Statement::ImportDeclaration(import) = statement else {
            continue;
        };
        let import_source = import.source.value.as_str();
        let Some(specifiers) = &import.specifiers else {
            continue;
        };
        let Some(resolved) = resolve_vue3_type_import(filename, import_source, type_resolver)
        else {
            for specifier in specifiers {
                context
                    .unresolved_import_sources
                    .insert(import_specifier_local(specifier), import_source.to_string());
            }
            continue;
        };
        let Some(imported_context) =
            vue3_external_type_context_from_path(&resolved, &mut *seen, type_resolver)
        else {
            continue;
        };
        let normalized = normalize_path_string(&resolved);
        for specifier in specifiers {
            let local = import_specifier_local(specifier);
            let imported = import_specifier_imported(specifier).unwrap_or_else(|| "default".into());
            if imported == "*" {
                insert_vue3_external_namespace_types(
                    context,
                    &imported_context,
                    &local,
                    &normalized,
                );
                continue;
            }
            insert_vue3_external_type_alias(
                context,
                &imported_context,
                &imported,
                &local,
                &normalized,
            );
        }
    }
}

pub(crate) fn vue3_external_type_context_from_source_with_type(
    source: &str,
    filename: &str,
    source_type: oxc_span::SourceType,
    seen: &mut BTreeSet<String>,
    type_resolver: &Vue3TypeResolverContext,
) -> Vue27TypeContext {
    if !seen.insert(filename.to_string()) {
        return Vue27TypeContext::default();
    }
    let context = vue3_external_type_context_from_source_inner(
        source,
        filename,
        source_type,
        seen,
        type_resolver,
    );
    seen.remove(filename);
    context
}

pub(crate) fn vue3_external_type_context_from_source_inner(
    source: &str,
    filename: &str,
    source_type: oxc_span::SourceType,
    seen: &mut BTreeSet<String>,
    type_resolver: &Vue3TypeResolverContext,
) -> Vue27TypeContext {
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, source, source_type)
        .with_options(oxc_parser::ParseOptions {
            parse_regular_expression: true,
            ..oxc_parser::ParseOptions::default()
        })
        .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return Vue27TypeContext::default();
    }
    let mut analysis = Vue3ScriptSetupAnalysis {
        type_filename: Some(filename.to_string()),
        type_seen: seen.clone(),
        type_resolver: type_resolver.clone(),
        ..Vue3ScriptSetupAnalysis::default()
    };
    let mut seed_context = Vue27TypeContext::default();
    extend_vue3_type_context_from_external_imports_with_seen(
        filename,
        source,
        source_type,
        &mut seed_context,
        seen,
        type_resolver,
    );
    analysis.declared_types = seed_context.declared_types;
    analysis.define_model_declared_types = seed_context.define_model_declared_types;
    analysis.type_query_declared_types = seed_context.type_query_declared_types;
    analysis.define_model_type_query_declared_types =
        seed_context.define_model_type_query_declared_types;
    analysis.keyof_type_query_declared_types = seed_context.keyof_type_query_declared_types;
    analysis.props_type_declarations = seed_context.props_type_declarations;
    analysis.keyof_runtime_type_declarations = seed_context.keyof_runtime_type_declarations;
    analysis.tuple_runtime_type_declarations = seed_context.tuple_runtime_type_declarations;
    analysis.define_model_tuple_runtime_type_declarations =
        seed_context.define_model_tuple_runtime_type_declarations;
    analysis.array_element_runtime_type_declarations =
        seed_context.array_element_runtime_type_declarations;
    analysis.define_model_array_element_runtime_type_declarations =
        seed_context.define_model_array_element_runtime_type_declarations;
    analysis.parameter_tuple_runtime_type_declarations =
        seed_context.parameter_tuple_runtime_type_declarations;
    analysis.define_model_parameter_tuple_runtime_type_declarations =
        seed_context.define_model_parameter_tuple_runtime_type_declarations;
    analysis.constructor_parameter_tuple_runtime_type_declarations =
        seed_context.constructor_parameter_tuple_runtime_type_declarations;
    analysis.define_model_constructor_parameter_tuple_runtime_type_declarations =
        seed_context.define_model_constructor_parameter_tuple_runtime_type_declarations;
    analysis.return_type_runtime_type_declarations =
        seed_context.return_type_runtime_type_declarations;
    analysis.define_model_return_type_runtime_type_declarations =
        seed_context.define_model_return_type_runtime_type_declarations;
    analysis.props_options_type_declarations = seed_context.props_options_type_declarations;
    analysis.return_type_props_options_declarations =
        seed_context.return_type_props_options_declarations;
    analysis.generic_type_aliases = seed_context.generic_type_aliases;
    analysis.string_literal_type_declarations = seed_context.string_literal_type_declarations;
    analysis.ordered_string_literal_type_declarations =
        seed_context.ordered_string_literal_type_declarations;
    analysis.emits_type_declarations = seed_context.emits_type_declarations;
    analysis.type_sources = seed_context.type_sources;
    analysis.type_direct_deps = seed_context.type_direct_deps;
    analysis.type_deps = seed_context.type_deps;
    analysis.unresolved_import_sources = seed_context.unresolved_import_sources;
    analysis.silent_unresolved_type_names = seed_context.silent_unresolved_type_names;
    collect_vue3_declared_types_from_statements(source, &parsed.program.body, &mut analysis);
    collect_vue3_declared_type_deps_from_statements(&parsed.program.body, &mut analysis);
    project_vue3_default_type_exports(source, &parsed.program.body, &mut analysis);
    seed_vue3_external_type_deps(filename, &mut analysis);
    let re_exported = project_vue3_type_re_exports(
        filename,
        &parsed.program.body,
        &mut analysis,
        seen,
        type_resolver,
    );
    project_vue3_exported_type_specifiers(&parsed.program.body, &mut analysis);
    let mut exported = vue3_exported_type_names(&parsed.program.body);
    exported.extend(re_exported);
    if !exported.is_empty() {
        analysis
            .declared_types
            .retain(|name, _| exported.contains(name));
        analysis
            .define_model_declared_types
            .retain(|name, _| exported.contains(name));
        analysis
            .type_query_declared_types
            .retain(|name, _| exported.contains(name));
        analysis
            .define_model_type_query_declared_types
            .retain(|name, _| exported.contains(name));
        analysis
            .keyof_type_query_declared_types
            .retain(|name, _| exported.contains(name));
        analysis
            .props_type_declarations
            .retain(|name, _| exported.contains(name));
        analysis
            .keyof_runtime_type_declarations
            .retain(|name, _| exported.contains(name));
        analysis
            .tuple_runtime_type_declarations
            .retain(|name, _| exported.contains(name));
        analysis
            .define_model_tuple_runtime_type_declarations
            .retain(|name, _| exported.contains(name));
        analysis
            .array_element_runtime_type_declarations
            .retain(|name, _| exported.contains(name));
        analysis
            .define_model_array_element_runtime_type_declarations
            .retain(|name, _| exported.contains(name));
        analysis
            .parameter_tuple_runtime_type_declarations
            .retain(|name, _| exported.contains(name));
        analysis
            .define_model_parameter_tuple_runtime_type_declarations
            .retain(|name, _| exported.contains(name));
        analysis
            .constructor_parameter_tuple_runtime_type_declarations
            .retain(|name, _| exported.contains(name));
        analysis
            .define_model_constructor_parameter_tuple_runtime_type_declarations
            .retain(|name, _| exported.contains(name));
        analysis
            .return_type_runtime_type_declarations
            .retain(|name, _| exported.contains(name));
        analysis
            .define_model_return_type_runtime_type_declarations
            .retain(|name, _| exported.contains(name));
        analysis
            .props_options_type_declarations
            .retain(|name, _| exported.contains(name));
        analysis
            .return_type_props_options_declarations
            .retain(|name, _| exported.contains(name));
        analysis
            .generic_type_aliases
            .retain(|name, _| exported.contains(name));
        analysis
            .string_literal_type_declarations
            .retain(|name, _| exported.contains(name));
        analysis
            .ordered_string_literal_type_declarations
            .retain(|name, _| exported.contains(name));
        analysis
            .emits_type_declarations
            .retain(|name, _| exported.contains(name));
        analysis
            .type_sources
            .retain(|name, _| exported.contains(name));
        analysis
            .type_direct_deps
            .retain(|name, _| exported.contains(name));
        analysis.type_deps.retain(|name, _| exported.contains(name));
        analysis
            .unresolved_import_sources
            .retain(|name, _| exported.contains(name));
        analysis
            .silent_unresolved_type_names
            .retain(|name| exported.contains(name));
    }
    Vue27TypeContext {
        declared_types: analysis.declared_types,
        define_model_declared_types: analysis.define_model_declared_types,
        type_query_declared_types: analysis.type_query_declared_types,
        define_model_type_query_declared_types: analysis.define_model_type_query_declared_types,
        keyof_type_query_declared_types: analysis.keyof_type_query_declared_types,
        props_type_declarations: analysis.props_type_declarations,
        keyof_runtime_type_declarations: analysis.keyof_runtime_type_declarations,
        tuple_runtime_type_declarations: analysis.tuple_runtime_type_declarations,
        define_model_tuple_runtime_type_declarations: analysis
            .define_model_tuple_runtime_type_declarations,
        array_element_runtime_type_declarations: analysis.array_element_runtime_type_declarations,
        define_model_array_element_runtime_type_declarations: analysis
            .define_model_array_element_runtime_type_declarations,
        parameter_tuple_runtime_type_declarations: analysis
            .parameter_tuple_runtime_type_declarations,
        define_model_parameter_tuple_runtime_type_declarations: analysis
            .define_model_parameter_tuple_runtime_type_declarations,
        constructor_parameter_tuple_runtime_type_declarations: analysis
            .constructor_parameter_tuple_runtime_type_declarations,
        define_model_constructor_parameter_tuple_runtime_type_declarations: analysis
            .define_model_constructor_parameter_tuple_runtime_type_declarations,
        return_type_runtime_type_declarations: analysis.return_type_runtime_type_declarations,
        define_model_return_type_runtime_type_declarations: analysis
            .define_model_return_type_runtime_type_declarations,
        props_options_type_declarations: analysis.props_options_type_declarations,
        return_type_props_options_declarations: analysis.return_type_props_options_declarations,
        generic_type_aliases: analysis.generic_type_aliases,
        string_literal_type_declarations: analysis.string_literal_type_declarations,
        ordered_string_literal_type_declarations: analysis.ordered_string_literal_type_declarations,
        emits_type_declarations: analysis.emits_type_declarations,
        type_sources: analysis.type_sources,
        type_direct_deps: analysis.type_direct_deps,
        type_deps: analysis.type_deps,
        unresolved_import_sources: analysis.unresolved_import_sources,
        silent_unresolved_type_names: analysis.silent_unresolved_type_names,
    }
}

pub(crate) fn seed_vue3_external_type_deps(filename: &str, analysis: &mut Vue3ScriptSetupAnalysis) {
    let dependency = normalize_path_string(Path::new(filename));
    let names = analysis
        .declared_types
        .keys()
        .chain(analysis.define_model_declared_types.keys())
        .chain(analysis.type_query_declared_types.keys())
        .chain(analysis.define_model_type_query_declared_types.keys())
        .chain(analysis.keyof_type_query_declared_types.keys())
        .chain(analysis.props_type_declarations.keys())
        .chain(analysis.keyof_runtime_type_declarations.keys())
        .chain(analysis.tuple_runtime_type_declarations.keys())
        .chain(analysis.define_model_tuple_runtime_type_declarations.keys())
        .chain(analysis.array_element_runtime_type_declarations.keys())
        .chain(
            analysis
                .define_model_array_element_runtime_type_declarations
                .keys(),
        )
        .chain(analysis.parameter_tuple_runtime_type_declarations.keys())
        .chain(
            analysis
                .define_model_parameter_tuple_runtime_type_declarations
                .keys(),
        )
        .chain(
            analysis
                .constructor_parameter_tuple_runtime_type_declarations
                .keys(),
        )
        .chain(
            analysis
                .define_model_constructor_parameter_tuple_runtime_type_declarations
                .keys(),
        )
        .chain(analysis.return_type_runtime_type_declarations.keys())
        .chain(
            analysis
                .define_model_return_type_runtime_type_declarations
                .keys(),
        )
        .chain(analysis.props_options_type_declarations.keys())
        .chain(analysis.return_type_props_options_declarations.keys())
        .chain(analysis.generic_type_aliases.keys())
        .chain(analysis.string_literal_type_declarations.keys())
        .chain(analysis.ordered_string_literal_type_declarations.keys())
        .chain(analysis.emits_type_declarations.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    for name in names {
        analysis
            .type_sources
            .insert(name.clone(), dependency.clone());
        analysis.type_direct_deps.entry(name.clone()).or_default();
        analysis
            .type_deps
            .entry(name)
            .or_default()
            .insert(dependency.clone());
    }
}

pub(crate) fn project_vue3_type_re_exports(
    filename: &str,
    statements: &[Statement<'_>],
    analysis: &mut Vue3ScriptSetupAnalysis,
    seen: &mut BTreeSet<String>,
    type_resolver: &Vue3TypeResolverContext,
) -> BTreeSet<String> {
    let mut exported_names = BTreeSet::new();
    for statement in statements {
        match statement {
            Statement::ExportNamedDeclaration(declaration) => {
                let Some(source) = declaration.source.as_ref() else {
                    continue;
                };
                let import_source = source.value.as_str();
                let Some(imported_context) = vue3_external_type_context_from_source(
                    filename,
                    import_source,
                    seen,
                    type_resolver,
                ) else {
                    continue;
                };
                let Some(dependency) =
                    resolve_vue3_type_import(filename, import_source, type_resolver)
                        .map(|path| normalize_path_string(&path))
                else {
                    continue;
                };
                for specifier in &declaration.specifiers {
                    let Some(imported) = module_export_name(specifier.local()) else {
                        continue;
                    };
                    let Some(exported) = module_export_name(specifier.exported()) else {
                        continue;
                    };
                    insert_vue3_re_exported_type_alias(
                        analysis,
                        &imported_context,
                        imported,
                        exported,
                        &dependency,
                    );
                    if vue3_type_context_has_name(&imported_context, imported) {
                        exported_names.insert(exported.to_string());
                    }
                }
            }
            Statement::ExportAllDeclaration(declaration) => {
                let import_source = declaration.source.value.as_str();
                let Some(imported_context) = vue3_external_type_context_from_source(
                    filename,
                    import_source,
                    seen,
                    type_resolver,
                ) else {
                    continue;
                };
                let Some(dependency) =
                    resolve_vue3_type_import(filename, import_source, type_resolver)
                        .map(|path| normalize_path_string(&path))
                else {
                    continue;
                };
                exported_names.extend(project_vue3_export_all_type_context(
                    analysis,
                    &imported_context,
                    &dependency,
                ));
            }
            _ => {}
        }
    }
    exported_names
}

pub(crate) fn vue3_external_type_context_from_source(
    filename: &str,
    source: &str,
    seen: &mut BTreeSet<String>,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<Vue27TypeContext> {
    let resolved = resolve_vue3_type_import(filename, source, type_resolver)?;
    vue3_external_type_context_from_path(&resolved, seen, type_resolver)
}

pub(crate) struct Vue3ExternalTypeSource {
    pub(crate) source: String,
    pub(crate) source_type: oxc_span::SourceType,
}

pub(crate) fn vue3_external_type_context_from_path(
    path: &Path,
    seen: &mut BTreeSet<String>,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<Vue27TypeContext> {
    let source = vue3_external_type_source_from_path(path)?;
    let normalized = normalize_path_string(path);
    Some(vue3_external_type_context_from_source_with_type(
        &source.source,
        &normalized,
        source.source_type,
        seen,
        type_resolver,
    ))
}

pub(crate) fn vue3_external_type_source_from_path(path: &Path) -> Option<Vue3ExternalTypeSource> {
    let source = std::fs::read_to_string(path).ok()?;
    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("vue"))
    {
        return Some(vue3_external_vue_type_source(path, &source));
    }
    Some(Vue3ExternalTypeSource {
        source,
        source_type: vue3_type_source_type(&normalize_path_string(path)),
    })
}

pub(crate) fn vue3_external_vue_type_source(path: &Path, source: &str) -> Vue3ExternalTypeSource {
    let mut sources = SourceMap::default();
    let source_file = sources.add_file(Some(path.to_path_buf()), source.to_string());
    let options = Vue3SfcParseOptions::default();
    let extracted = extract_sfc_blocks(
        source,
        source_file,
        SfcBlockContentMode::Vue3 { options: &options },
    );
    let descriptor = vue3_descriptor_from_blocks(
        normalize_path_string(path),
        source,
        source_file,
        extracted.blocks,
        &options,
    )
    .descriptor;
    let mut blocks = Vec::new();
    let mut source_type = oxc_span::SourceType::ts();
    for block in [descriptor.script.as_ref(), descriptor.script_setup.as_ref()]
        .into_iter()
        .flatten()
    {
        if block.attrs.lang.as_deref() == Some("tsx") {
            source_type = oxc_span::SourceType::tsx();
        }
        blocks.push(block.content.as_str());
    }
    Vue3ExternalTypeSource {
        source: blocks.join("\n"),
        source_type,
    }
}

pub(crate) struct Vue3ResolvedImportType {
    pub(crate) name: String,
    pub(crate) dependency: String,
    pub(crate) context: Vue27TypeContext,
}

pub(crate) fn vue3_resolve_import_type(
    import_type: &TSImportType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vue3ResolvedImportType> {
    let source = import_type.source.value.as_str();
    let name = vue3_import_type_qualifier_key(import_type.qualifier.as_ref()?);
    let filename = analysis.type_filename.as_deref()?;
    let resolved = resolve_vue3_type_import(filename, source, &analysis.type_resolver)?;
    let dependency = normalize_path_string(&resolved);
    let mut seen = analysis.type_seen.clone();
    let context =
        vue3_external_type_context_from_path(&resolved, &mut seen, &analysis.type_resolver)?;
    Some(Vue3ResolvedImportType {
        name,
        dependency,
        context,
    })
}

pub(crate) fn vue3_type_source_type(filename: &str) -> oxc_span::SourceType {
    oxc_span::SourceType::from_path(filename).unwrap_or_else(|_| oxc_span::SourceType::ts())
}

pub(crate) fn vue3_exported_type_names(statements: &[Statement<'_>]) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for statement in statements {
        match statement {
            Statement::ExportDefaultDeclaration(declaration) => {
                if vue3_default_export_may_be_type(declaration) {
                    names.insert("default".into());
                }
            }
            Statement::ExportNamedDeclaration(declaration) => {
                if let Some(declaration) = &declaration.declaration {
                    match declaration {
                        Declaration::TSInterfaceDeclaration(declaration) => {
                            names.insert(declaration.id.name.to_string());
                        }
                        Declaration::TSTypeAliasDeclaration(declaration) => {
                            names.insert(declaration.id.name.to_string());
                        }
                        Declaration::TSEnumDeclaration(declaration) => {
                            names.insert(declaration.id.name.to_string());
                        }
                        Declaration::FunctionDeclaration(function)
                            if vue3_function_has_return_projection(function) =>
                        {
                            if let Some(id) = &function.id {
                                names.insert(id.name.to_string());
                            }
                        }
                        Declaration::VariableDeclaration(declaration) if declaration.declare => {
                            for declarator in &declaration.declarations {
                                if let Some(name) = first_pattern_binding(&declarator.id) {
                                    names.insert(name);
                                }
                            }
                        }
                        Declaration::VariableDeclaration(declaration) => {
                            for declarator in &declaration.declarations {
                                if vue3_variable_declarator_has_type_projection(declarator) {
                                    if let Some(name) = first_pattern_binding(&declarator.id) {
                                        names.insert(name);
                                    }
                                }
                            }
                        }
                        Declaration::ClassDeclaration(declaration) => {
                            if let Some(id) = &declaration.id {
                                names.insert(id.name.to_string());
                            }
                        }
                        Declaration::TSModuleDeclaration(declaration) => {
                            names.extend(vue3_namespace_exported_type_names(declaration));
                        }
                        _ => {}
                    }
                }
                if declaration.source.is_none() {
                    for specifier in &declaration.specifiers {
                        if let Some(exported) = module_export_name(specifier.exported()) {
                            names.insert(exported.to_string());
                        }
                    }
                }
            }
            _ => {}
        }
    }
    names
}

pub(crate) fn vue3_default_export_may_be_type(declaration: &ExportDefaultDeclaration<'_>) -> bool {
    match &declaration.declaration {
        ExportDefaultDeclarationKind::TSInterfaceDeclaration(_)
        | ExportDefaultDeclarationKind::ClassDeclaration(_)
        | ExportDefaultDeclarationKind::Identifier(_) => true,
        ExportDefaultDeclarationKind::ObjectExpression(object) => {
            vue3_static_runtime_props_options_object_is_projectable(object)
        }
        ExportDefaultDeclarationKind::FunctionDeclaration(function) => {
            vue3_function_has_return_projection(function)
        }
        declaration => {
            vue3_default_export_function_value_has_return_projection(declaration)
                || vue3_default_export_static_runtime_props_options_is_projectable(declaration)
        }
    }
}

pub(crate) fn project_vue3_default_type_exports(
    source: &str,
    statements: &[Statement<'_>],
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    for statement in statements {
        let Statement::ExportDefaultDeclaration(declaration) = statement else {
            continue;
        };
        match &declaration.declaration {
            ExportDefaultDeclarationKind::TSInterfaceDeclaration(declaration) => {
                let name = declaration.id.name.to_string();
                let deps = collect_vue3_interface_type_deps(declaration, analysis);
                register_vue3_interface_declaration(source, declaration, analysis);
                insert_vue3_declared_type_deps(analysis, &name, deps);
                insert_vue3_local_type_alias(analysis, &name, "default");
            }
            ExportDefaultDeclarationKind::Identifier(identifier) => {
                insert_vue3_local_type_alias(analysis, identifier.name.as_str(), "default");
            }
            ExportDefaultDeclarationKind::FunctionDeclaration(function) => {
                if let Some(id) = &function.id {
                    let name = id.name.as_str();
                    register_vue3_function_return_projection(source, name, function, analysis);
                    if let Some(return_type) = function.return_type.as_ref() {
                        let deps =
                            collect_vue3_type_argument_deps(&return_type.type_annotation, analysis);
                        insert_vue3_declared_type_deps(analysis, name, deps);
                    }
                    insert_vue3_local_type_alias(analysis, name, "default");
                } else {
                    register_vue3_function_return_projection(source, "default", function, analysis);
                    if let Some(return_type) = function.return_type.as_ref() {
                        let deps =
                            collect_vue3_type_argument_deps(&return_type.type_annotation, analysis);
                        insert_vue3_declared_type_deps(analysis, "default", deps);
                    }
                }
            }
            declaration
                if vue3_default_export_function_value_has_return_projection(declaration) =>
            {
                if let Some(expression) = declaration.as_expression() {
                    register_vue3_function_value_expression_return_projection(
                        source, "default", expression, analysis,
                    );
                    if let Some(return_type) = vue3_function_value_return_type(expression) {
                        let deps = collect_vue3_type_argument_deps(return_type, analysis);
                        insert_vue3_declared_type_deps(analysis, "default", deps);
                    }
                }
            }
            declaration
                if vue3_default_export_static_runtime_props_options_is_projectable(declaration) =>
            {
                register_vue3_default_static_runtime_props_options(source, declaration, analysis);
            }
            ExportDefaultDeclarationKind::ClassDeclaration(class) => {
                if let Some(id) = &class.id {
                    register_vue3_class_type_name(analysis, id.name.as_str());
                    insert_vue3_local_type_alias(analysis, id.name.as_str(), "default");
                } else {
                    register_vue3_class_type_name(analysis, "default");
                }
            }
            _ => {}
        }
    }
}

pub(crate) fn project_vue3_exported_type_specifiers(
    statements: &[Statement<'_>],
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    for statement in statements {
        let Statement::ExportNamedDeclaration(declaration) = statement else {
            continue;
        };
        if declaration.source.is_some() {
            continue;
        }
        for specifier in &declaration.specifiers {
            let Some(local) = module_export_name(specifier.local()) else {
                continue;
            };
            let Some(exported) = module_export_name(specifier.exported()) else {
                continue;
            };
            if local == exported {
                continue;
            }
            insert_vue3_local_type_alias(analysis, local, exported);
        }
    }
}

pub(crate) fn project_vue3_namespace_declaration(
    source: &str,
    declaration: &TSModuleDeclaration<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    let Some(namespace) = vue3_ts_module_declaration_name(declaration) else {
        return;
    };
    project_vue3_namespace_declaration_with_prefix(source, declaration, &namespace, analysis);
}

pub(crate) fn project_vue3_namespace_declaration_with_prefix(
    source: &str,
    declaration: &TSModuleDeclaration<'_>,
    prefix: &str,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    let Some(body) = declaration.body.as_ref() else {
        return;
    };
    match body {
        TSModuleDeclarationBody::TSModuleBlock(block) => {
            let mut namespace_analysis = Vue3ScriptSetupAnalysis {
                declared_types: analysis.declared_types.clone(),
                define_model_declared_types: analysis.define_model_declared_types.clone(),
                type_query_declared_types: analysis.type_query_declared_types.clone(),
                define_model_type_query_declared_types: analysis
                    .define_model_type_query_declared_types
                    .clone(),
                keyof_type_query_declared_types: analysis.keyof_type_query_declared_types.clone(),
                props_type_declarations: analysis.props_type_declarations.clone(),
                keyof_runtime_type_declarations: analysis.keyof_runtime_type_declarations.clone(),
                tuple_runtime_type_declarations: analysis.tuple_runtime_type_declarations.clone(),
                define_model_tuple_runtime_type_declarations: analysis
                    .define_model_tuple_runtime_type_declarations
                    .clone(),
                array_element_runtime_type_declarations: analysis
                    .array_element_runtime_type_declarations
                    .clone(),
                define_model_array_element_runtime_type_declarations: analysis
                    .define_model_array_element_runtime_type_declarations
                    .clone(),
                parameter_tuple_runtime_type_declarations: analysis
                    .parameter_tuple_runtime_type_declarations
                    .clone(),
                define_model_parameter_tuple_runtime_type_declarations: analysis
                    .define_model_parameter_tuple_runtime_type_declarations
                    .clone(),
                constructor_parameter_tuple_runtime_type_declarations: analysis
                    .constructor_parameter_tuple_runtime_type_declarations
                    .clone(),
                define_model_constructor_parameter_tuple_runtime_type_declarations: analysis
                    .define_model_constructor_parameter_tuple_runtime_type_declarations
                    .clone(),
                return_type_runtime_type_declarations: analysis
                    .return_type_runtime_type_declarations
                    .clone(),
                define_model_return_type_runtime_type_declarations: analysis
                    .define_model_return_type_runtime_type_declarations
                    .clone(),
                props_options_type_declarations: analysis.props_options_type_declarations.clone(),
                return_type_props_options_declarations: analysis
                    .return_type_props_options_declarations
                    .clone(),
                generic_type_aliases: analysis.generic_type_aliases.clone(),
                string_literal_type_declarations: analysis.string_literal_type_declarations.clone(),
                ordered_string_literal_type_declarations: analysis
                    .ordered_string_literal_type_declarations
                    .clone(),
                emits_type_declarations: analysis.emits_type_declarations.clone(),
                type_sources: analysis.type_sources.clone(),
                type_direct_deps: analysis.type_direct_deps.clone(),
                type_deps: analysis.type_deps.clone(),
                unresolved_import_sources: analysis.unresolved_import_sources.clone(),
                silent_unresolved_type_names: analysis.silent_unresolved_type_names.clone(),
                type_filename: analysis.type_filename.clone(),
                type_seen: analysis.type_seen.clone(),
                ..Vue3ScriptSetupAnalysis::default()
            };
            seed_vue3_namespace_type_names(prefix, &block.body, &mut namespace_analysis);
            collect_vue3_declared_types_from_statements(
                source,
                &block.body,
                &mut namespace_analysis,
            );
            collect_vue3_declared_type_deps_from_statements(&block.body, &mut namespace_analysis);
            let names = if declaration.declare {
                vue3_declared_type_names_from_statements(&block.body)
            } else {
                vue3_exported_type_names(&block.body)
            };
            for name in names {
                let prefixed = format!("{prefix}.{name}");
                insert_vue3_type_alias_from_analysis(
                    analysis,
                    &namespace_analysis,
                    &name,
                    &prefixed,
                );
            }
        }
        TSModuleDeclarationBody::TSModuleDeclaration(nested) => {
            let Some(name) = vue3_ts_module_declaration_name(nested) else {
                return;
            };
            let prefix = format!("{prefix}.{name}");
            project_vue3_namespace_declaration_with_prefix(source, nested, &prefix, analysis);
        }
    }
}

pub(crate) fn seed_vue3_namespace_type_names(
    prefix: &str,
    statements: &[Statement<'_>],
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    for name in vue3_declared_type_names_from_statements(statements) {
        let prefixed = format!("{prefix}.{name}");
        for candidate in [name, prefixed] {
            analysis
                .declared_types
                .entry(candidate.clone())
                .or_insert_with(|| vec!["Object".into()]);
            analysis
                .define_model_declared_types
                .entry(candidate)
                .or_insert_with(|| vec!["Object".into()]);
        }
    }
}

pub(crate) fn seed_vue3_global_namespace_type_names(
    statements: &[Statement<'_>],
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    for statement in statements {
        match statement {
            Statement::TSGlobalDeclaration(global) => {
                seed_vue3_global_namespace_type_names(&global.body.body, analysis);
            }
            Statement::TSModuleDeclaration(declaration)
                if vue3_ts_module_declaration_is_global(declaration) =>
            {
                if let Some(body) = vue3_ts_module_declaration_block_body(declaration) {
                    seed_vue3_global_namespace_type_names(body, analysis);
                }
            }
            Statement::TSModuleDeclaration(declaration) => {
                seed_vue3_qualified_type_names(
                    vue3_namespace_declared_type_names(declaration),
                    analysis,
                );
            }
            Statement::ExportNamedDeclaration(declaration) => {
                if let Some(Declaration::TSModuleDeclaration(declaration)) =
                    declaration.declaration.as_ref()
                {
                    seed_vue3_qualified_type_names(
                        vue3_namespace_declared_type_names(declaration),
                        analysis,
                    );
                }
            }
            _ => {}
        }
    }
}

pub(crate) fn seed_vue3_qualified_type_names(
    names: BTreeSet<String>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    for name in names {
        analysis
            .declared_types
            .entry(name.clone())
            .or_insert_with(|| vec!["Object".into()]);
        analysis
            .define_model_declared_types
            .entry(name)
            .or_insert_with(|| vec!["Object".into()]);
    }
}

pub(crate) fn vue3_ts_module_declaration_name(
    declaration: &TSModuleDeclaration<'_>,
) -> Option<String> {
    match &declaration.id {
        TSModuleDeclarationName::Identifier(identifier) => Some(identifier.name.to_string()),
        TSModuleDeclarationName::StringLiteral(_) => None,
    }
}

pub(crate) fn vue3_ts_module_declaration_is_global(declaration: &TSModuleDeclaration<'_>) -> bool {
    vue3_ts_module_declaration_name(declaration).as_deref() == Some("global")
}

pub(crate) fn vue3_ts_module_declaration_block_body<'a>(
    declaration: &'a TSModuleDeclaration<'a>,
) -> Option<&'a [Statement<'a>]> {
    match declaration.body.as_ref()? {
        TSModuleDeclarationBody::TSModuleBlock(block) => Some(&block.body),
        TSModuleDeclarationBody::TSModuleDeclaration(_) => None,
    }
}

pub(crate) fn vue3_namespace_exported_type_names(
    declaration: &TSModuleDeclaration<'_>,
) -> BTreeSet<String> {
    let Some(namespace) = vue3_ts_module_declaration_name(declaration) else {
        return BTreeSet::new();
    };
    vue3_namespace_exported_type_names_with_prefix(declaration, &namespace)
}

pub(crate) fn vue3_namespace_declared_type_names(
    declaration: &TSModuleDeclaration<'_>,
) -> BTreeSet<String> {
    let Some(namespace) = vue3_ts_module_declaration_name(declaration) else {
        return BTreeSet::new();
    };
    vue3_namespace_declared_type_names_with_prefix(declaration, &namespace)
}

pub(crate) fn vue3_namespace_declared_type_names_with_prefix(
    declaration: &TSModuleDeclaration<'_>,
    prefix: &str,
) -> BTreeSet<String> {
    let Some(body) = declaration.body.as_ref() else {
        return BTreeSet::new();
    };
    match body {
        TSModuleDeclarationBody::TSModuleBlock(block) => {
            vue3_declared_type_names_from_statements(&block.body)
                .into_iter()
                .map(|name| format!("{prefix}.{name}"))
                .collect()
        }
        TSModuleDeclarationBody::TSModuleDeclaration(nested) => {
            let Some(name) = vue3_ts_module_declaration_name(nested) else {
                return BTreeSet::new();
            };
            let prefix = format!("{prefix}.{name}");
            vue3_namespace_declared_type_names_with_prefix(nested, &prefix)
        }
    }
}

pub(crate) fn vue3_namespace_exported_type_names_with_prefix(
    declaration: &TSModuleDeclaration<'_>,
    prefix: &str,
) -> BTreeSet<String> {
    let Some(body) = declaration.body.as_ref() else {
        return BTreeSet::new();
    };
    match body {
        TSModuleDeclarationBody::TSModuleBlock(block) => vue3_exported_type_names(&block.body)
            .into_iter()
            .map(|name| format!("{prefix}.{name}"))
            .collect(),
        TSModuleDeclarationBody::TSModuleDeclaration(nested) => {
            let Some(name) = vue3_ts_module_declaration_name(nested) else {
                return BTreeSet::new();
            };
            let prefix = format!("{prefix}.{name}");
            vue3_namespace_exported_type_names_with_prefix(nested, &prefix)
        }
    }
}

pub(crate) fn insert_vue3_type_alias_from_analysis(
    target: &mut Vue3ScriptSetupAnalysis,
    source: &Vue3ScriptSetupAnalysis,
    source_name: &str,
    target_name: &str,
) {
    if let Some(value) = source.declared_types.get(source_name).cloned() {
        target.declared_types.insert(target_name.to_string(), value);
    }
    if let Some(value) = source.define_model_declared_types.get(source_name).cloned() {
        target
            .define_model_declared_types
            .insert(target_name.to_string(), value);
    }
    if let Some(value) = source.type_query_declared_types.get(source_name).cloned() {
        target
            .type_query_declared_types
            .insert(target_name.to_string(), value);
    }
    if let Some(value) = source
        .define_model_type_query_declared_types
        .get(source_name)
        .cloned()
    {
        target
            .define_model_type_query_declared_types
            .insert(target_name.to_string(), value);
    }
    if let Some(value) = source
        .keyof_type_query_declared_types
        .get(source_name)
        .cloned()
    {
        target
            .keyof_type_query_declared_types
            .insert(target_name.to_string(), value);
    }
    if let Some(value) = source.props_type_declarations.get(source_name).cloned() {
        target
            .props_type_declarations
            .insert(target_name.to_string(), value);
    }
    if let Some(value) = source
        .keyof_runtime_type_declarations
        .get(source_name)
        .cloned()
    {
        target
            .keyof_runtime_type_declarations
            .insert(target_name.to_string(), value);
    }
    if let Some(value) = source
        .tuple_runtime_type_declarations
        .get(source_name)
        .cloned()
    {
        target
            .tuple_runtime_type_declarations
            .insert(target_name.to_string(), value);
    }
    if let Some(value) = source
        .define_model_tuple_runtime_type_declarations
        .get(source_name)
        .cloned()
    {
        target
            .define_model_tuple_runtime_type_declarations
            .insert(target_name.to_string(), value);
    }
    if let Some(value) = source
        .array_element_runtime_type_declarations
        .get(source_name)
        .cloned()
    {
        target
            .array_element_runtime_type_declarations
            .insert(target_name.to_string(), value);
    }
    if let Some(value) = source
        .define_model_array_element_runtime_type_declarations
        .get(source_name)
        .cloned()
    {
        target
            .define_model_array_element_runtime_type_declarations
            .insert(target_name.to_string(), value);
    }
    if let Some(value) = source
        .parameter_tuple_runtime_type_declarations
        .get(source_name)
        .cloned()
    {
        target
            .parameter_tuple_runtime_type_declarations
            .insert(target_name.to_string(), value);
    }
    if let Some(value) = source
        .define_model_parameter_tuple_runtime_type_declarations
        .get(source_name)
        .cloned()
    {
        target
            .define_model_parameter_tuple_runtime_type_declarations
            .insert(target_name.to_string(), value);
    }
    if let Some(value) = source
        .constructor_parameter_tuple_runtime_type_declarations
        .get(source_name)
        .cloned()
    {
        target
            .constructor_parameter_tuple_runtime_type_declarations
            .insert(target_name.to_string(), value);
    }
    if let Some(value) = source
        .define_model_constructor_parameter_tuple_runtime_type_declarations
        .get(source_name)
        .cloned()
    {
        target
            .define_model_constructor_parameter_tuple_runtime_type_declarations
            .insert(target_name.to_string(), value);
    }
    if let Some(value) = source
        .return_type_runtime_type_declarations
        .get(source_name)
        .cloned()
    {
        target
            .return_type_runtime_type_declarations
            .insert(target_name.to_string(), value);
    }
    if let Some(value) = source
        .define_model_return_type_runtime_type_declarations
        .get(source_name)
        .cloned()
    {
        target
            .define_model_return_type_runtime_type_declarations
            .insert(target_name.to_string(), value);
    }
    if let Some(value) = source
        .props_options_type_declarations
        .get(source_name)
        .cloned()
    {
        target
            .props_options_type_declarations
            .insert(target_name.to_string(), value);
    }
    if let Some(value) = source
        .return_type_props_options_declarations
        .get(source_name)
        .cloned()
    {
        target
            .return_type_props_options_declarations
            .insert(target_name.to_string(), value);
    }
    if let Some(value) = source.generic_type_aliases.get(source_name).cloned() {
        target
            .generic_type_aliases
            .insert(target_name.to_string(), value);
    }
    if let Some(value) = source
        .string_literal_type_declarations
        .get(source_name)
        .cloned()
    {
        target
            .string_literal_type_declarations
            .insert(target_name.to_string(), value);
    }
    if let Some(value) = source
        .ordered_string_literal_type_declarations
        .get(source_name)
        .cloned()
    {
        target
            .ordered_string_literal_type_declarations
            .insert(target_name.to_string(), value);
    }
    if let Some(value) = source.emits_type_declarations.get(source_name).cloned() {
        target
            .emits_type_declarations
            .insert(target_name.to_string(), value);
    }
    if let Some(value) = source.type_sources.get(source_name).cloned() {
        target.type_sources.insert(target_name.to_string(), value);
    }
    if let Some(value) = source.type_direct_deps.get(source_name).cloned() {
        target
            .type_direct_deps
            .insert(target_name.to_string(), value);
    }
    if let Some(value) = source.type_deps.get(source_name).cloned() {
        target.type_deps.insert(target_name.to_string(), value);
    }
    if let Some(value) = source.unresolved_import_sources.get(source_name).cloned() {
        target
            .unresolved_import_sources
            .insert(target_name.to_string(), value);
    }
    if source.silent_unresolved_type_names.contains(source_name) {
        target
            .silent_unresolved_type_names
            .insert(target_name.to_string());
    }
}

pub(crate) fn insert_vue3_local_type_alias(
    analysis: &mut Vue3ScriptSetupAnalysis,
    local_name: &str,
    exported_name: &str,
) {
    if let Some(value) = analysis.declared_types.get(local_name).cloned() {
        analysis
            .declared_types
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis
        .define_model_declared_types
        .get(local_name)
        .cloned()
    {
        analysis
            .define_model_declared_types
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis.type_query_declared_types.get(local_name).cloned() {
        analysis
            .type_query_declared_types
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis
        .define_model_type_query_declared_types
        .get(local_name)
        .cloned()
    {
        analysis
            .define_model_type_query_declared_types
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis
        .keyof_type_query_declared_types
        .get(local_name)
        .cloned()
    {
        analysis
            .keyof_type_query_declared_types
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis.props_type_declarations.get(local_name).cloned() {
        analysis
            .props_type_declarations
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis
        .keyof_runtime_type_declarations
        .get(local_name)
        .cloned()
    {
        analysis
            .keyof_runtime_type_declarations
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis
        .tuple_runtime_type_declarations
        .get(local_name)
        .cloned()
    {
        analysis
            .tuple_runtime_type_declarations
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis
        .define_model_tuple_runtime_type_declarations
        .get(local_name)
        .cloned()
    {
        analysis
            .define_model_tuple_runtime_type_declarations
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis
        .array_element_runtime_type_declarations
        .get(local_name)
        .cloned()
    {
        analysis
            .array_element_runtime_type_declarations
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis
        .define_model_array_element_runtime_type_declarations
        .get(local_name)
        .cloned()
    {
        analysis
            .define_model_array_element_runtime_type_declarations
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis
        .parameter_tuple_runtime_type_declarations
        .get(local_name)
        .cloned()
    {
        analysis
            .parameter_tuple_runtime_type_declarations
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis
        .define_model_parameter_tuple_runtime_type_declarations
        .get(local_name)
        .cloned()
    {
        analysis
            .define_model_parameter_tuple_runtime_type_declarations
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis
        .constructor_parameter_tuple_runtime_type_declarations
        .get(local_name)
        .cloned()
    {
        analysis
            .constructor_parameter_tuple_runtime_type_declarations
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis
        .define_model_constructor_parameter_tuple_runtime_type_declarations
        .get(local_name)
        .cloned()
    {
        analysis
            .define_model_constructor_parameter_tuple_runtime_type_declarations
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis
        .return_type_runtime_type_declarations
        .get(local_name)
        .cloned()
    {
        analysis
            .return_type_runtime_type_declarations
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis
        .define_model_return_type_runtime_type_declarations
        .get(local_name)
        .cloned()
    {
        analysis
            .define_model_return_type_runtime_type_declarations
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis
        .props_options_type_declarations
        .get(local_name)
        .cloned()
    {
        analysis
            .props_options_type_declarations
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis
        .return_type_props_options_declarations
        .get(local_name)
        .cloned()
    {
        analysis
            .return_type_props_options_declarations
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis.generic_type_aliases.get(local_name).cloned() {
        analysis
            .generic_type_aliases
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis
        .string_literal_type_declarations
        .get(local_name)
        .cloned()
    {
        analysis
            .string_literal_type_declarations
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis
        .ordered_string_literal_type_declarations
        .get(local_name)
        .cloned()
    {
        analysis
            .ordered_string_literal_type_declarations
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis.emits_type_declarations.get(local_name).cloned() {
        analysis
            .emits_type_declarations
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis.type_sources.get(local_name).cloned() {
        analysis
            .type_sources
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis.type_direct_deps.get(local_name).cloned() {
        analysis
            .type_direct_deps
            .insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis.type_deps.get(local_name).cloned() {
        analysis.type_deps.insert(exported_name.to_string(), value);
    }
    if let Some(value) = analysis.unresolved_import_sources.get(local_name).cloned() {
        analysis
            .unresolved_import_sources
            .insert(exported_name.to_string(), value);
    }
    if analysis.silent_unresolved_type_names.contains(local_name) {
        analysis
            .silent_unresolved_type_names
            .insert(exported_name.to_string());
    }
}

pub(crate) fn project_vue3_export_all_type_context(
    analysis: &mut Vue3ScriptSetupAnalysis,
    imported: &Vue27TypeContext,
    dependency: &str,
) -> BTreeSet<String> {
    let names = imported
        .declared_types
        .keys()
        .chain(imported.define_model_declared_types.keys())
        .chain(imported.type_query_declared_types.keys())
        .chain(imported.define_model_type_query_declared_types.keys())
        .chain(imported.keyof_type_query_declared_types.keys())
        .chain(imported.props_type_declarations.keys())
        .chain(imported.keyof_runtime_type_declarations.keys())
        .chain(imported.tuple_runtime_type_declarations.keys())
        .chain(imported.define_model_tuple_runtime_type_declarations.keys())
        .chain(imported.array_element_runtime_type_declarations.keys())
        .chain(
            imported
                .define_model_array_element_runtime_type_declarations
                .keys(),
        )
        .chain(imported.parameter_tuple_runtime_type_declarations.keys())
        .chain(
            imported
                .define_model_parameter_tuple_runtime_type_declarations
                .keys(),
        )
        .chain(
            imported
                .constructor_parameter_tuple_runtime_type_declarations
                .keys(),
        )
        .chain(
            imported
                .define_model_constructor_parameter_tuple_runtime_type_declarations
                .keys(),
        )
        .chain(imported.return_type_runtime_type_declarations.keys())
        .chain(
            imported
                .define_model_return_type_runtime_type_declarations
                .keys(),
        )
        .chain(imported.props_options_type_declarations.keys())
        .chain(imported.return_type_props_options_declarations.keys())
        .chain(imported.generic_type_aliases.keys())
        .chain(imported.string_literal_type_declarations.keys())
        .chain(imported.ordered_string_literal_type_declarations.keys())
        .chain(imported.emits_type_declarations.keys())
        .cloned()
        .filter(|name| name != "default")
        .collect::<BTreeSet<_>>();
    for name in &names {
        insert_vue3_re_exported_type_alias(analysis, imported, &name, &name, dependency);
    }
    names
}

pub(crate) fn insert_vue3_re_exported_type_alias(
    analysis: &mut Vue3ScriptSetupAnalysis,
    imported: &Vue27TypeContext,
    imported_name: &str,
    exported_name: &str,
    dependency: &str,
) {
    if let Some(runtime) = imported.declared_types.get(imported_name) {
        analysis
            .declared_types
            .insert(exported_name.to_string(), runtime.clone());
    }
    if let Some(runtime) = imported.define_model_declared_types.get(imported_name) {
        analysis
            .define_model_declared_types
            .insert(exported_name.to_string(), runtime.clone());
    }
    if let Some(runtime) = imported.type_query_declared_types.get(imported_name) {
        analysis
            .type_query_declared_types
            .insert(exported_name.to_string(), runtime.clone());
    }
    if let Some(runtime) = imported
        .define_model_type_query_declared_types
        .get(imported_name)
    {
        analysis
            .define_model_type_query_declared_types
            .insert(exported_name.to_string(), runtime.clone());
    }
    if let Some(runtime) = imported.keyof_type_query_declared_types.get(imported_name) {
        analysis
            .keyof_type_query_declared_types
            .insert(exported_name.to_string(), runtime.clone());
    }
    if let Some(props) = imported.props_type_declarations.get(imported_name) {
        analysis
            .props_type_declarations
            .insert(exported_name.to_string(), props.clone());
    }
    if let Some(types) = imported.keyof_runtime_type_declarations.get(imported_name) {
        analysis
            .keyof_runtime_type_declarations
            .insert(exported_name.to_string(), types.clone());
    }
    if let Some(tuple) = imported.tuple_runtime_type_declarations.get(imported_name) {
        analysis
            .tuple_runtime_type_declarations
            .insert(exported_name.to_string(), tuple.clone());
    }
    if let Some(tuple) = imported
        .define_model_tuple_runtime_type_declarations
        .get(imported_name)
    {
        analysis
            .define_model_tuple_runtime_type_declarations
            .insert(exported_name.to_string(), tuple.clone());
    }
    if let Some(types) = imported
        .array_element_runtime_type_declarations
        .get(imported_name)
    {
        analysis
            .array_element_runtime_type_declarations
            .insert(exported_name.to_string(), types.clone());
    }
    if let Some(types) = imported
        .define_model_array_element_runtime_type_declarations
        .get(imported_name)
    {
        analysis
            .define_model_array_element_runtime_type_declarations
            .insert(exported_name.to_string(), types.clone());
    }
    if let Some(tuple) = imported
        .parameter_tuple_runtime_type_declarations
        .get(imported_name)
    {
        analysis
            .parameter_tuple_runtime_type_declarations
            .insert(exported_name.to_string(), tuple.clone());
    }
    if let Some(tuple) = imported
        .define_model_parameter_tuple_runtime_type_declarations
        .get(imported_name)
    {
        analysis
            .define_model_parameter_tuple_runtime_type_declarations
            .insert(exported_name.to_string(), tuple.clone());
    }
    if let Some(tuple) = imported
        .constructor_parameter_tuple_runtime_type_declarations
        .get(imported_name)
    {
        analysis
            .constructor_parameter_tuple_runtime_type_declarations
            .insert(exported_name.to_string(), tuple.clone());
    }
    if let Some(tuple) = imported
        .define_model_constructor_parameter_tuple_runtime_type_declarations
        .get(imported_name)
    {
        analysis
            .define_model_constructor_parameter_tuple_runtime_type_declarations
            .insert(exported_name.to_string(), tuple.clone());
    }
    if let Some(types) = imported
        .return_type_runtime_type_declarations
        .get(imported_name)
    {
        analysis
            .return_type_runtime_type_declarations
            .insert(exported_name.to_string(), types.clone());
    }
    if let Some(types) = imported
        .define_model_return_type_runtime_type_declarations
        .get(imported_name)
    {
        analysis
            .define_model_return_type_runtime_type_declarations
            .insert(exported_name.to_string(), types.clone());
    }
    if let Some(props_options) = imported.props_options_type_declarations.get(imported_name) {
        analysis
            .props_options_type_declarations
            .insert(exported_name.to_string(), props_options.clone());
    }
    if let Some(props_options) = imported
        .return_type_props_options_declarations
        .get(imported_name)
    {
        analysis
            .return_type_props_options_declarations
            .insert(exported_name.to_string(), props_options.clone());
    }
    if let Some(alias) = imported.generic_type_aliases.get(imported_name) {
        analysis
            .generic_type_aliases
            .insert(exported_name.to_string(), alias.clone());
    }
    if let Some(keys) = imported.string_literal_type_declarations.get(imported_name) {
        analysis
            .string_literal_type_declarations
            .insert(exported_name.to_string(), keys.clone());
    }
    if let Some(keys) = imported
        .ordered_string_literal_type_declarations
        .get(imported_name)
    {
        analysis
            .ordered_string_literal_type_declarations
            .insert(exported_name.to_string(), keys.clone());
    }
    if let Some(emits) = imported.emits_type_declarations.get(imported_name) {
        analysis
            .emits_type_declarations
            .insert(exported_name.to_string(), emits.clone());
    }
    if vue3_type_context_has_name(imported, imported_name) {
        analysis
            .type_sources
            .insert(exported_name.to_string(), dependency.to_string());
        analysis.type_direct_deps.insert(
            exported_name.to_string(),
            vue3_direct_type_deps(imported, imported_name, dependency),
        );
        let mut deps = imported
            .type_deps
            .get(imported_name)
            .cloned()
            .unwrap_or_default();
        deps.insert(dependency.to_string());
        analysis.type_deps.insert(exported_name.to_string(), deps);
    }
    if let Some(import_source) = imported.unresolved_import_sources.get(imported_name) {
        analysis
            .unresolved_import_sources
            .insert(exported_name.to_string(), import_source.clone());
    }
    if imported
        .silent_unresolved_type_names
        .contains(imported_name)
    {
        analysis
            .silent_unresolved_type_names
            .insert(exported_name.to_string());
    }
}

pub(crate) fn insert_vue3_external_type_alias(
    context: &mut Vue27TypeContext,
    imported: &Vue27TypeContext,
    imported_name: &str,
    local_name: &str,
    dependency: &str,
) {
    if let Some(runtime) = imported.declared_types.get(imported_name) {
        context
            .declared_types
            .insert(local_name.to_string(), runtime.clone());
    }
    if let Some(runtime) = imported.define_model_declared_types.get(imported_name) {
        context
            .define_model_declared_types
            .insert(local_name.to_string(), runtime.clone());
    }
    if let Some(runtime) = imported.type_query_declared_types.get(imported_name) {
        context
            .type_query_declared_types
            .insert(local_name.to_string(), runtime.clone());
    }
    if let Some(runtime) = imported
        .define_model_type_query_declared_types
        .get(imported_name)
    {
        context
            .define_model_type_query_declared_types
            .insert(local_name.to_string(), runtime.clone());
    }
    if let Some(runtime) = imported.keyof_type_query_declared_types.get(imported_name) {
        context
            .keyof_type_query_declared_types
            .insert(local_name.to_string(), runtime.clone());
    }
    if let Some(props) = imported.props_type_declarations.get(imported_name) {
        context
            .props_type_declarations
            .insert(local_name.to_string(), props.clone());
    }
    if let Some(types) = imported.keyof_runtime_type_declarations.get(imported_name) {
        context
            .keyof_runtime_type_declarations
            .insert(local_name.to_string(), types.clone());
    }
    if let Some(tuple) = imported.tuple_runtime_type_declarations.get(imported_name) {
        context
            .tuple_runtime_type_declarations
            .insert(local_name.to_string(), tuple.clone());
    }
    if let Some(tuple) = imported
        .define_model_tuple_runtime_type_declarations
        .get(imported_name)
    {
        context
            .define_model_tuple_runtime_type_declarations
            .insert(local_name.to_string(), tuple.clone());
    }
    if let Some(types) = imported
        .array_element_runtime_type_declarations
        .get(imported_name)
    {
        context
            .array_element_runtime_type_declarations
            .insert(local_name.to_string(), types.clone());
    }
    if let Some(types) = imported
        .define_model_array_element_runtime_type_declarations
        .get(imported_name)
    {
        context
            .define_model_array_element_runtime_type_declarations
            .insert(local_name.to_string(), types.clone());
    }
    if let Some(tuple) = imported
        .parameter_tuple_runtime_type_declarations
        .get(imported_name)
    {
        context
            .parameter_tuple_runtime_type_declarations
            .insert(local_name.to_string(), tuple.clone());
    }
    if let Some(tuple) = imported
        .define_model_parameter_tuple_runtime_type_declarations
        .get(imported_name)
    {
        context
            .define_model_parameter_tuple_runtime_type_declarations
            .insert(local_name.to_string(), tuple.clone());
    }
    if let Some(tuple) = imported
        .constructor_parameter_tuple_runtime_type_declarations
        .get(imported_name)
    {
        context
            .constructor_parameter_tuple_runtime_type_declarations
            .insert(local_name.to_string(), tuple.clone());
    }
    if let Some(tuple) = imported
        .define_model_constructor_parameter_tuple_runtime_type_declarations
        .get(imported_name)
    {
        context
            .define_model_constructor_parameter_tuple_runtime_type_declarations
            .insert(local_name.to_string(), tuple.clone());
    }
    if let Some(types) = imported
        .return_type_runtime_type_declarations
        .get(imported_name)
    {
        context
            .return_type_runtime_type_declarations
            .insert(local_name.to_string(), types.clone());
    }
    if let Some(types) = imported
        .define_model_return_type_runtime_type_declarations
        .get(imported_name)
    {
        context
            .define_model_return_type_runtime_type_declarations
            .insert(local_name.to_string(), types.clone());
    }
    if let Some(props_options) = imported.props_options_type_declarations.get(imported_name) {
        context
            .props_options_type_declarations
            .insert(local_name.to_string(), props_options.clone());
    }
    if let Some(props_options) = imported
        .return_type_props_options_declarations
        .get(imported_name)
    {
        context
            .return_type_props_options_declarations
            .insert(local_name.to_string(), props_options.clone());
    }
    if let Some(alias) = imported.generic_type_aliases.get(imported_name) {
        context
            .generic_type_aliases
            .insert(local_name.to_string(), alias.clone());
        insert_vue3_external_generic_alias_string_key_helpers(context, imported, dependency);
    }
    if let Some(keys) = imported.string_literal_type_declarations.get(imported_name) {
        context
            .string_literal_type_declarations
            .insert(local_name.to_string(), keys.clone());
    }
    if let Some(keys) = imported
        .ordered_string_literal_type_declarations
        .get(imported_name)
    {
        context
            .ordered_string_literal_type_declarations
            .insert(local_name.to_string(), keys.clone());
    }
    if let Some(emits) = imported.emits_type_declarations.get(imported_name) {
        context
            .emits_type_declarations
            .insert(local_name.to_string(), emits.clone());
    }
    if imported.declared_types.contains_key(imported_name)
        || imported
            .define_model_declared_types
            .contains_key(imported_name)
        || imported
            .type_query_declared_types
            .contains_key(imported_name)
        || imported
            .define_model_type_query_declared_types
            .contains_key(imported_name)
        || imported
            .keyof_type_query_declared_types
            .contains_key(imported_name)
        || imported.props_type_declarations.contains_key(imported_name)
        || imported
            .keyof_runtime_type_declarations
            .contains_key(imported_name)
        || imported
            .tuple_runtime_type_declarations
            .contains_key(imported_name)
        || imported
            .define_model_tuple_runtime_type_declarations
            .contains_key(imported_name)
        || imported
            .array_element_runtime_type_declarations
            .contains_key(imported_name)
        || imported
            .define_model_array_element_runtime_type_declarations
            .contains_key(imported_name)
        || imported
            .parameter_tuple_runtime_type_declarations
            .contains_key(imported_name)
        || imported
            .define_model_parameter_tuple_runtime_type_declarations
            .contains_key(imported_name)
        || imported
            .constructor_parameter_tuple_runtime_type_declarations
            .contains_key(imported_name)
        || imported
            .define_model_constructor_parameter_tuple_runtime_type_declarations
            .contains_key(imported_name)
        || imported
            .return_type_runtime_type_declarations
            .contains_key(imported_name)
        || imported
            .define_model_return_type_runtime_type_declarations
            .contains_key(imported_name)
        || imported
            .props_options_type_declarations
            .contains_key(imported_name)
        || imported
            .return_type_props_options_declarations
            .contains_key(imported_name)
        || imported.generic_type_aliases.contains_key(imported_name)
        || imported
            .string_literal_type_declarations
            .contains_key(imported_name)
        || imported
            .ordered_string_literal_type_declarations
            .contains_key(imported_name)
        || imported.emits_type_declarations.contains_key(imported_name)
    {
        context
            .type_sources
            .insert(local_name.to_string(), dependency.to_string());
        context.type_direct_deps.insert(
            local_name.to_string(),
            vue3_direct_type_deps(imported, imported_name, dependency),
        );
        let mut deps = imported
            .type_deps
            .get(imported_name)
            .cloned()
            .unwrap_or_default();
        deps.insert(dependency.to_string());
        context.type_deps.insert(local_name.to_string(), deps);
    }
    if let Some(import_source) = imported.unresolved_import_sources.get(imported_name) {
        context
            .unresolved_import_sources
            .insert(local_name.to_string(), import_source.clone());
    }
    if imported
        .silent_unresolved_type_names
        .contains(imported_name)
    {
        context
            .silent_unresolved_type_names
            .insert(local_name.to_string());
    }
}

pub(crate) fn insert_vue3_external_generic_alias_string_key_helpers(
    context: &mut Vue27TypeContext,
    imported: &Vue27TypeContext,
    dependency: &str,
) {
    for (name, keys) in &imported.string_literal_type_declarations {
        context
            .string_literal_type_declarations
            .entry(name.clone())
            .or_insert_with(|| keys.clone());
        insert_vue3_external_helper_type_dep(context, imported, name, dependency);
    }
    for (name, keys) in &imported.ordered_string_literal_type_declarations {
        context
            .ordered_string_literal_type_declarations
            .entry(name.clone())
            .or_insert_with(|| keys.clone());
        insert_vue3_external_helper_type_dep(context, imported, name, dependency);
    }
    for (name, types) in &imported.keyof_runtime_type_declarations {
        context
            .keyof_runtime_type_declarations
            .entry(name.clone())
            .or_insert_with(|| types.clone());
        insert_vue3_external_helper_type_dep(context, imported, name, dependency);
    }
    for (name, tuple) in &imported.tuple_runtime_type_declarations {
        context
            .tuple_runtime_type_declarations
            .entry(name.clone())
            .or_insert_with(|| tuple.clone());
        insert_vue3_external_helper_type_dep(context, imported, name, dependency);
    }
    for (name, tuple) in &imported.define_model_tuple_runtime_type_declarations {
        context
            .define_model_tuple_runtime_type_declarations
            .entry(name.clone())
            .or_insert_with(|| tuple.clone());
        insert_vue3_external_helper_type_dep(context, imported, name, dependency);
    }
    for (name, types) in &imported.array_element_runtime_type_declarations {
        context
            .array_element_runtime_type_declarations
            .entry(name.clone())
            .or_insert_with(|| types.clone());
        insert_vue3_external_helper_type_dep(context, imported, name, dependency);
    }
    for (name, types) in &imported.define_model_array_element_runtime_type_declarations {
        context
            .define_model_array_element_runtime_type_declarations
            .entry(name.clone())
            .or_insert_with(|| types.clone());
        insert_vue3_external_helper_type_dep(context, imported, name, dependency);
    }
    for (name, tuple) in &imported.parameter_tuple_runtime_type_declarations {
        context
            .parameter_tuple_runtime_type_declarations
            .entry(name.clone())
            .or_insert_with(|| tuple.clone());
        insert_vue3_external_helper_type_dep(context, imported, name, dependency);
    }
    for (name, tuple) in &imported.define_model_parameter_tuple_runtime_type_declarations {
        context
            .define_model_parameter_tuple_runtime_type_declarations
            .entry(name.clone())
            .or_insert_with(|| tuple.clone());
        insert_vue3_external_helper_type_dep(context, imported, name, dependency);
    }
    for (name, tuple) in &imported.constructor_parameter_tuple_runtime_type_declarations {
        context
            .constructor_parameter_tuple_runtime_type_declarations
            .entry(name.clone())
            .or_insert_with(|| tuple.clone());
        insert_vue3_external_helper_type_dep(context, imported, name, dependency);
    }
    for (name, tuple) in
        &imported.define_model_constructor_parameter_tuple_runtime_type_declarations
    {
        context
            .define_model_constructor_parameter_tuple_runtime_type_declarations
            .entry(name.clone())
            .or_insert_with(|| tuple.clone());
        insert_vue3_external_helper_type_dep(context, imported, name, dependency);
    }
    for (name, types) in &imported.return_type_runtime_type_declarations {
        context
            .return_type_runtime_type_declarations
            .entry(name.clone())
            .or_insert_with(|| types.clone());
        insert_vue3_external_helper_type_dep(context, imported, name, dependency);
    }
    for (name, types) in &imported.define_model_return_type_runtime_type_declarations {
        context
            .define_model_return_type_runtime_type_declarations
            .entry(name.clone())
            .or_insert_with(|| types.clone());
        insert_vue3_external_helper_type_dep(context, imported, name, dependency);
    }
}

pub(crate) fn insert_vue3_external_helper_type_dep(
    context: &mut Vue27TypeContext,
    imported: &Vue27TypeContext,
    name: &str,
    dependency: &str,
) {
    context
        .type_sources
        .entry(name.to_string())
        .or_insert_with(|| dependency.to_string());
    context
        .type_direct_deps
        .entry(name.to_string())
        .or_insert_with(|| vue3_direct_type_deps(imported, name, dependency));
    let mut deps = imported.type_deps.get(name).cloned().unwrap_or_default();
    deps.insert(dependency.to_string());
    context.type_deps.entry(name.to_string()).or_insert(deps);
}

pub(crate) fn vue3_direct_type_deps(
    imported: &Vue27TypeContext,
    imported_name: &str,
    dependency: &str,
) -> Vec<String> {
    let mut deps = Vec::new();
    push_unique(&mut deps, dependency);
    if let Some(imported_deps) = imported.type_direct_deps.get(imported_name) {
        for imported_dep in imported_deps {
            push_unique(&mut deps, imported_dep);
        }
    }
    deps
}

pub(crate) fn insert_vue3_external_namespace_types(
    context: &mut Vue27TypeContext,
    imported: &Vue27TypeContext,
    namespace: &str,
    dependency: &str,
) {
    for imported_name in vue3_type_context_names(imported) {
        let local_name = format!("{namespace}.{imported_name}");
        insert_vue3_external_type_alias(context, imported, &imported_name, &local_name, dependency);
    }
}

pub(crate) fn vue3_type_context_names(context: &Vue27TypeContext) -> BTreeSet<String> {
    context
        .declared_types
        .keys()
        .chain(context.define_model_declared_types.keys())
        .chain(context.type_query_declared_types.keys())
        .chain(context.define_model_type_query_declared_types.keys())
        .chain(context.keyof_type_query_declared_types.keys())
        .chain(context.props_type_declarations.keys())
        .chain(context.keyof_runtime_type_declarations.keys())
        .chain(context.tuple_runtime_type_declarations.keys())
        .chain(context.define_model_tuple_runtime_type_declarations.keys())
        .chain(context.array_element_runtime_type_declarations.keys())
        .chain(
            context
                .define_model_array_element_runtime_type_declarations
                .keys(),
        )
        .chain(context.parameter_tuple_runtime_type_declarations.keys())
        .chain(
            context
                .define_model_parameter_tuple_runtime_type_declarations
                .keys(),
        )
        .chain(
            context
                .constructor_parameter_tuple_runtime_type_declarations
                .keys(),
        )
        .chain(
            context
                .define_model_constructor_parameter_tuple_runtime_type_declarations
                .keys(),
        )
        .chain(context.return_type_runtime_type_declarations.keys())
        .chain(
            context
                .define_model_return_type_runtime_type_declarations
                .keys(),
        )
        .chain(context.props_options_type_declarations.keys())
        .chain(context.return_type_props_options_declarations.keys())
        .chain(context.generic_type_aliases.keys())
        .chain(context.string_literal_type_declarations.keys())
        .chain(context.ordered_string_literal_type_declarations.keys())
        .chain(context.emits_type_declarations.keys())
        .cloned()
        .collect()
}

pub(crate) fn vue3_type_context_has_name(context: &Vue27TypeContext, name: &str) -> bool {
    context.declared_types.contains_key(name)
        || context.define_model_declared_types.contains_key(name)
        || context.type_query_declared_types.contains_key(name)
        || context
            .define_model_type_query_declared_types
            .contains_key(name)
        || context.keyof_type_query_declared_types.contains_key(name)
        || context.props_type_declarations.contains_key(name)
        || context.keyof_runtime_type_declarations.contains_key(name)
        || context.tuple_runtime_type_declarations.contains_key(name)
        || context
            .define_model_tuple_runtime_type_declarations
            .contains_key(name)
        || context
            .array_element_runtime_type_declarations
            .contains_key(name)
        || context
            .define_model_array_element_runtime_type_declarations
            .contains_key(name)
        || context
            .parameter_tuple_runtime_type_declarations
            .contains_key(name)
        || context
            .define_model_parameter_tuple_runtime_type_declarations
            .contains_key(name)
        || context
            .constructor_parameter_tuple_runtime_type_declarations
            .contains_key(name)
        || context
            .define_model_constructor_parameter_tuple_runtime_type_declarations
            .contains_key(name)
        || context
            .return_type_runtime_type_declarations
            .contains_key(name)
        || context
            .define_model_return_type_runtime_type_declarations
            .contains_key(name)
        || context.props_options_type_declarations.contains_key(name)
        || context
            .return_type_props_options_declarations
            .contains_key(name)
        || context.generic_type_aliases.contains_key(name)
        || context.string_literal_type_declarations.contains_key(name)
        || context
            .ordered_string_literal_type_declarations
            .contains_key(name)
        || context.emits_type_declarations.contains_key(name)
}

pub(crate) fn vue3_type_import_source_is_relative(source: &str) -> bool {
    source.starts_with("./") || source.starts_with("../")
}

pub(crate) fn resolve_vue3_type_import(
    filename: &str,
    source: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    if vue3_type_import_source_is_relative(source) {
        return resolve_vue3_relative_type_import(filename, source, type_resolver);
    }
    if let Some(resolved) = resolve_vue3_tsconfig_type_import(filename, source, type_resolver) {
        return Some(resolved);
    }
    resolve_vue3_bare_type_import(filename, source, type_resolver)
}

pub(crate) fn resolve_vue3_relative_type_import(
    filename: &str,
    source: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    let base = Path::new(filename)
        .parent()
        .unwrap_or_else(|| Path::new(""));
    let candidate = normalize_path_components(base.join(source));
    resolve_vue3_type_import_path(&candidate, type_resolver)
}

#[derive(Clone, Debug)]
pub(crate) struct Vue3TsconfigPathMapping {
    pub(crate) pattern: String,
    pub(crate) targets: Vec<String>,
    pub(crate) target_base_dir: PathBuf,
    pub(crate) template_config_dir: PathBuf,
}

#[derive(Clone, Debug)]
pub(crate) struct Vue3TsconfigPathMatch<'a> {
    pub(crate) mapping: &'a Vue3TsconfigPathMapping,
    pub(crate) capture: String,
    pub(crate) score: usize,
    pub(crate) order: usize,
}

pub(crate) fn resolve_vue3_tsconfig_type_import(
    filename: &str,
    source: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    for config_path in vue3_tsconfig_search_paths(filename) {
        let config_dir = config_path
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_path_buf();
        let mut seen = BTreeSet::new();
        let mappings =
            vue3_tsconfig_path_mappings_from_config(&config_path, &config_dir, &mut seen);
        if let Some(resolved) =
            resolve_vue3_tsconfig_path_mappings(&mappings, source, type_resolver)
        {
            return Some(resolved);
        }
    }
    None
}

pub(crate) fn vue3_tsconfig_search_paths(filename: &str) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let mut current = Path::new(filename).parent();
    while let Some(dir) = current {
        let candidate = normalize_path_components(dir.join("tsconfig.json"));
        if candidate.is_file() {
            paths.push(candidate);
        }
        current = dir.parent();
    }
    paths
}

pub(crate) fn vue3_tsconfig_path_mappings_from_config(
    config_path: &Path,
    template_config_dir: &Path,
    seen: &mut BTreeSet<String>,
) -> Vec<Vue3TsconfigPathMapping> {
    let normalized = normalize_path_string(config_path);
    if !seen.insert(normalized) {
        return Vec::new();
    }
    let Ok(source) = std::fs::read_to_string(config_path) else {
        return Vec::new();
    };
    let Some(value) = vue3_parse_tsconfig_jsonc(&source) else {
        return Vec::new();
    };
    let config_dir = config_path.parent().unwrap_or_else(|| Path::new(""));
    let mut mappings = Vec::new();
    for extended in vue3_tsconfig_extends_paths(&value, config_dir) {
        mappings.extend(vue3_tsconfig_path_mappings_from_config(
            &extended,
            template_config_dir,
            seen,
        ));
    }
    let direct = vue3_tsconfig_direct_path_mappings(&value, config_dir, template_config_dir);
    if !direct.is_empty() {
        let direct_patterns = direct
            .iter()
            .map(|mapping| mapping.pattern.as_str())
            .collect::<BTreeSet<_>>();
        mappings.retain(|mapping| !direct_patterns.contains(mapping.pattern.as_str()));
        mappings.extend(direct);
    }
    for reference in vue3_tsconfig_reference_paths(&value, config_dir) {
        let reference_dir = reference.parent().unwrap_or_else(|| Path::new(""));
        mappings.extend(vue3_tsconfig_path_mappings_from_config(
            &reference,
            reference_dir,
            seen,
        ));
    }
    mappings
}

pub(crate) fn vue3_tsconfig_global_type_files(
    filename: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut seen_configs = BTreeSet::new();
    let mut seen_files = BTreeSet::new();
    for config_path in vue3_tsconfig_search_paths(filename) {
        let config_dir = config_path.parent().unwrap_or_else(|| Path::new(""));
        vue3_tsconfig_global_type_files_from_config(
            &config_path,
            config_dir,
            &mut seen_configs,
            &mut seen_files,
            &mut files,
            type_resolver,
        );
    }
    files
}

pub(crate) fn vue3_tsconfig_global_type_files_from_config(
    config_path: &Path,
    template_config_dir: &Path,
    seen_configs: &mut BTreeSet<String>,
    seen_files: &mut BTreeSet<String>,
    files: &mut Vec<PathBuf>,
    type_resolver: &Vue3TypeResolverContext,
) {
    let normalized = normalize_path_string(config_path);
    if !seen_configs.insert(normalized) {
        return;
    }
    let Ok(source) = std::fs::read_to_string(config_path) else {
        return;
    };
    let Some(value) = vue3_parse_tsconfig_jsonc(&source) else {
        return;
    };
    let config_dir = config_path.parent().unwrap_or_else(|| Path::new(""));
    for extended in vue3_tsconfig_extends_paths(&value, config_dir) {
        vue3_tsconfig_global_type_files_from_config(
            &extended,
            template_config_dir,
            seen_configs,
            seen_files,
            files,
            type_resolver,
        );
    }
    for file in vue3_tsconfig_direct_global_type_files(
        &value,
        config_dir,
        template_config_dir,
        type_resolver,
    ) {
        let normalized = normalize_path_string(&file);
        if seen_files.insert(normalized) {
            files.push(file);
        }
    }
    for reference in vue3_tsconfig_reference_paths(&value, config_dir) {
        let reference_dir = reference.parent().unwrap_or_else(|| Path::new(""));
        vue3_tsconfig_global_type_files_from_config(
            &reference,
            reference_dir,
            seen_configs,
            seen_files,
            files,
            type_resolver,
        );
    }
}

pub(crate) fn vue3_tsconfig_direct_global_type_files(
    value: &serde_json::Value,
    config_dir: &Path,
    template_config_dir: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for target in vue3_tsconfig_string_array(value.get("files")) {
        let path = vue3_tsconfig_target_path(config_dir, template_config_dir, &target, "");
        if vue3_tsconfig_global_type_file_is_supported(&path) {
            files.push(path);
        }
    }
    for target in vue3_tsconfig_string_array(value.get("include")) {
        files.extend(vue3_tsconfig_include_global_type_files(
            config_dir,
            template_config_dir,
            &target,
        ));
    }
    files.extend(vue3_tsconfig_compiler_option_global_type_files(
        value,
        config_dir,
        template_config_dir,
        type_resolver,
    ));
    files
}

pub(crate) fn vue3_tsconfig_string_array(value: Option<&serde_json::Value>) -> Vec<String> {
    value
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .map(str::to_string)
        .collect()
}

pub(crate) fn vue3_tsconfig_compiler_option_global_type_files(
    value: &serde_json::Value,
    config_dir: &Path,
    template_config_dir: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Vec<PathBuf> {
    let compiler_options = value
        .get("compilerOptions")
        .and_then(serde_json::Value::as_object);
    let has_configured_type_roots =
        compiler_options.is_some_and(|options| options.get("typeRoots").is_some());
    let configured_type_roots =
        vue3_tsconfig_string_array(compiler_options.and_then(|options| options.get("typeRoots")))
            .into_iter()
            .filter_map(|target| {
                let path = vue3_tsconfig_target_path(config_dir, template_config_dir, &target, "");
                path.is_dir().then_some(path)
            })
            .collect::<Vec<_>>();
    let type_roots = if has_configured_type_roots {
        configured_type_roots
    } else {
        vue3_tsconfig_default_type_roots(config_dir)
    };
    if compiler_options.is_some_and(|options| options.get("types").is_some()) {
        let types =
            vue3_tsconfig_string_array(compiler_options.and_then(|options| options.get("types")));
        return types
            .into_iter()
            .flat_map(|type_name| {
                vue3_tsconfig_named_type_global_type_files(&type_roots, &type_name, type_resolver)
            })
            .collect();
    }
    type_roots
        .into_iter()
        .flat_map(|type_root| {
            vue3_tsconfig_all_type_root_global_type_files(&type_root, type_resolver)
        })
        .collect()
}

pub(crate) fn vue3_tsconfig_default_type_roots(config_dir: &Path) -> Vec<PathBuf> {
    vue3_node_modules_search_paths_from_dir(config_dir)
        .into_iter()
        .map(|node_modules| normalize_path_components(node_modules.join("@types")))
        .filter(|path| path.is_dir())
        .collect()
}

pub(crate) fn vue3_tsconfig_named_type_global_type_files(
    type_roots: &[PathBuf],
    type_name: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Vec<PathBuf> {
    if !vue3_tsconfig_type_name_is_safe(type_name) {
        return Vec::new();
    }
    type_roots
        .iter()
        .flat_map(|type_root| vue3_tsconfig_type_name_package_dirs(type_root, type_name))
        .filter_map(|package_dir| {
            vue3_tsconfig_type_package_global_type_file(&package_dir, type_resolver)
        })
        .collect()
}

pub(crate) fn vue3_tsconfig_type_name_is_safe(type_name: &str) -> bool {
    !type_name.is_empty()
        && !type_name.contains(':')
        && !type_name.contains('\\')
        && !Path::new(type_name).is_absolute()
        && !type_name
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
}

pub(crate) fn vue3_tsconfig_type_name_package_dirs(
    type_root: &Path,
    type_name: &str,
) -> Vec<PathBuf> {
    if let Some(scoped) = type_name.strip_prefix('@') {
        let parts = scoped.split('/').collect::<Vec<_>>();
        if parts.len() == 2 {
            return vec![
                normalize_path_components(type_root.join(format!("@{}", parts[0])).join(parts[1])),
                normalize_path_components(type_root.join(parts[0]).join(parts[1])),
                normalize_path_components(type_root.join(format!("{}__{}", parts[0], parts[1]))),
            ];
        }
    }
    vec![normalize_path_components(type_root.join(type_name))]
}

pub(crate) fn vue3_tsconfig_all_type_root_global_type_files(
    type_root: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(type_root) else {
        return Vec::new();
    };
    let mut entries = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    entries.sort();
    let mut files = Vec::new();
    for entry in entries {
        let name = entry
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if !entry.is_dir() || name.is_empty() || name.starts_with('.') {
            continue;
        }
        if name.starts_with('@') {
            files.extend(vue3_tsconfig_all_scoped_type_root_global_type_files(
                &entry,
                type_resolver,
            ));
        } else if let Some(file) =
            vue3_tsconfig_type_package_global_type_file(&entry, type_resolver)
        {
            files.push(file);
        }
    }
    files
}

pub(crate) fn vue3_tsconfig_all_scoped_type_root_global_type_files(
    scope_dir: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(scope_dir) else {
        return Vec::new();
    };
    let mut entries = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    entries.sort();
    entries
        .into_iter()
        .filter(|entry| entry.is_dir())
        .filter(|entry| {
            entry
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| !name.is_empty() && !name.starts_with('.'))
        })
        .filter_map(|entry| vue3_tsconfig_type_package_global_type_file(&entry, type_resolver))
        .collect()
}

pub(crate) fn vue3_tsconfig_type_package_global_type_file(
    package_dir: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    let path = resolve_vue3_package_type_entry(package_dir, None, type_resolver)?;
    vue3_tsconfig_global_type_file_is_supported(&path).then_some(path)
}

pub(crate) fn vue3_tsconfig_global_type_file_is_supported(path: &Path) -> bool {
    path.is_file()
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".d.ts"))
}

pub(crate) fn vue3_tsconfig_include_global_type_files(
    config_dir: &Path,
    template_config_dir: &Path,
    target: &str,
) -> Vec<PathBuf> {
    if !vue3_tsconfig_include_can_match_global_type_files(target) {
        return Vec::new();
    }
    if !target.contains('*') && !target.contains('?') {
        let path = vue3_tsconfig_target_path(config_dir, template_config_dir, target, "");
        if vue3_tsconfig_global_type_file_is_supported(&path) {
            return vec![path];
        }
        if path.is_dir() {
            let mut files = Vec::new();
            vue3_collect_global_type_files_from_dir(&path, &mut files);
            return files;
        }
        return Vec::new();
    }
    let Some(root) = vue3_tsconfig_include_root_path(config_dir, template_config_dir, target)
    else {
        return Vec::new();
    };
    let pattern = vue3_tsconfig_include_pattern(config_dir, template_config_dir, target);
    let mut files = Vec::new();
    vue3_collect_global_type_files_from_dir(&root, &mut files);
    files
        .into_iter()
        .filter(|file| vue3_tsconfig_glob_matches(&pattern, &normalize_path_string(file)))
        .collect()
}

pub(crate) fn vue3_tsconfig_include_can_match_global_type_files(target: &str) -> bool {
    let file_pattern = target.rsplit('/').next().unwrap_or(target);
    if !file_pattern.contains('.') {
        return true;
    }
    file_pattern.ends_with(".d.ts") || file_pattern.ends_with(".ts")
}

pub(crate) fn vue3_tsconfig_include_pattern(
    config_dir: &Path,
    template_config_dir: &Path,
    target: &str,
) -> String {
    let target = target.replace(
        "${configDir}",
        normalize_path_string(template_config_dir).as_str(),
    );
    let path = Path::new(&target);
    if path.is_absolute() {
        normalize_path_string(&normalize_path_components(PathBuf::from(target)))
    } else {
        normalize_path_string(&normalize_path_components(config_dir.join(target)))
    }
}

pub(crate) fn vue3_tsconfig_include_root_path(
    config_dir: &Path,
    template_config_dir: &Path,
    target: &str,
) -> Option<PathBuf> {
    if target.is_empty() || target.contains('\\') || target.contains(':') {
        return None;
    }
    let root = target
        .split('/')
        .take_while(|segment| !segment.contains('*') && !segment.contains('?'))
        .filter(|segment| !segment.is_empty() && *segment != ".")
        .collect::<Vec<_>>();
    if root.iter().any(|segment| *segment == "..") {
        return None;
    }
    let root = if root.is_empty() {
        ".".to_string()
    } else {
        root.join("/")
    };
    let path = vue3_tsconfig_target_path(config_dir, template_config_dir, &root, "");
    path.is_dir().then_some(path)
}

pub(crate) fn vue3_collect_global_type_files_from_dir(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    entries.sort();
    for entry in entries {
        let name = entry
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if name == "node_modules" || name.starts_with('.') {
            continue;
        }
        if entry.is_dir() {
            vue3_collect_global_type_files_from_dir(&entry, files);
        } else if vue3_tsconfig_global_type_file_is_supported(&entry) {
            files.push(normalize_path_components(entry));
        }
    }
}

pub(crate) fn vue3_tsconfig_glob_matches(pattern: &str, path: &str) -> bool {
    let pattern = pattern.replace('\\', "/");
    let path = path.replace('\\', "/");
    let pattern_parts = pattern.split('/').collect::<Vec<_>>();
    let path_parts = path.split('/').collect::<Vec<_>>();
    vue3_tsconfig_glob_parts_match(&pattern_parts, &path_parts)
}

pub(crate) fn vue3_tsconfig_glob_parts_match(pattern: &[&str], path: &[&str]) -> bool {
    if pattern.is_empty() {
        return path.is_empty();
    }
    if pattern[0] == "**" {
        return vue3_tsconfig_glob_parts_match(&pattern[1..], path)
            || (!path.is_empty() && vue3_tsconfig_glob_parts_match(pattern, &path[1..]));
    }
    if path.is_empty() || !vue3_tsconfig_glob_segment_match(pattern[0], path[0]) {
        return false;
    }
    vue3_tsconfig_glob_parts_match(&pattern[1..], &path[1..])
}

pub(crate) fn vue3_tsconfig_glob_segment_match(pattern: &str, text: &str) -> bool {
    let pattern = pattern.chars().collect::<Vec<_>>();
    let text = text.chars().collect::<Vec<_>>();
    let mut previous = vec![false; text.len() + 1];
    previous[0] = true;
    for pattern_ch in pattern {
        let mut current = vec![false; text.len() + 1];
        if pattern_ch == '*' {
            current[0] = previous[0];
            for index in 1..=text.len() {
                current[index] = previous[index] || current[index - 1];
            }
        } else {
            for index in 1..=text.len() {
                current[index] =
                    previous[index - 1] && (pattern_ch == '?' || pattern_ch == text[index - 1]);
            }
        }
        previous = current;
    }
    previous[text.len()]
}

pub(crate) fn vue3_parse_tsconfig_jsonc(source: &str) -> Option<serde_json::Value> {
    serde_json::from_str::<serde_json::Value>(source)
        .ok()
        .or_else(|| {
            let normalized = vue3_normalize_tsconfig_jsonc(source);
            serde_json::from_str::<serde_json::Value>(&normalized).ok()
        })
}

pub(crate) fn vue3_normalize_tsconfig_jsonc(source: &str) -> String {
    let without_comments = vue3_strip_jsonc_comments(source);
    vue3_strip_jsonc_trailing_commas(&without_comments)
}

pub(crate) fn vue3_strip_jsonc_comments(source: &str) -> String {
    let mut output = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;
    while let Some(ch) = chars.next() {
        if in_string {
            output.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        if ch == '"' {
            in_string = true;
            output.push(ch);
            continue;
        }
        if ch != '/' {
            output.push(ch);
            continue;
        }
        match chars.peek().copied() {
            Some('/') => {
                chars.next();
                output.push(' ');
                output.push(' ');
                while let Some(comment) = chars.next() {
                    if comment == '\n' || comment == '\r' {
                        output.push(comment);
                        break;
                    }
                    output.push(' ');
                }
            }
            Some('*') => {
                chars.next();
                output.push(' ');
                output.push(' ');
                let mut prev_star = false;
                while let Some(comment) = chars.next() {
                    let ends_comment = prev_star && comment == '/';
                    if comment == '\n' || comment == '\r' {
                        output.push(comment);
                    } else {
                        output.push(' ');
                    }
                    if ends_comment {
                        break;
                    }
                    prev_star = comment == '*';
                }
            }
            _ => output.push(ch),
        }
    }
    output
}

pub(crate) fn vue3_strip_jsonc_trailing_commas(source: &str) -> String {
    let mut output = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;
    while let Some(ch) = chars.next() {
        if in_string {
            output.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        if ch == '"' {
            in_string = true;
            output.push(ch);
            continue;
        }
        if ch != ',' {
            output.push(ch);
            continue;
        }
        let mut lookahead = chars.clone();
        while lookahead.peek().is_some_and(|next| next.is_whitespace()) {
            lookahead.next();
        }
        if lookahead
            .peek()
            .is_some_and(|next| matches!(*next, '}' | ']'))
        {
            continue;
        }
        output.push(ch);
    }
    output
}

pub(crate) fn vue3_tsconfig_extends_paths(
    value: &serde_json::Value,
    config_dir: &Path,
) -> Vec<PathBuf> {
    match value.get("extends") {
        Some(serde_json::Value::String(target)) => {
            vue3_resolve_tsconfig_extends_path(config_dir, target)
                .into_iter()
                .collect()
        }
        Some(serde_json::Value::Array(targets)) => targets
            .iter()
            .filter_map(serde_json::Value::as_str)
            .filter_map(|target| vue3_resolve_tsconfig_extends_path(config_dir, target))
            .collect(),
        _ => Vec::new(),
    }
}

pub(crate) fn vue3_tsconfig_reference_paths(
    value: &serde_json::Value,
    config_dir: &Path,
) -> Vec<PathBuf> {
    value
        .get("references")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|reference| reference.get("path").and_then(serde_json::Value::as_str))
        .filter_map(|target| vue3_resolve_tsconfig_path(config_dir, target))
        .collect()
}

pub(crate) fn vue3_resolve_tsconfig_extends_path(
    config_dir: &Path,
    target: &str,
) -> Option<PathBuf> {
    if vue3_tsconfig_path_is_relative(target) || Path::new(target).is_absolute() {
        return vue3_resolve_tsconfig_path(config_dir, target);
    }
    resolve_vue3_package_tsconfig_extends(config_dir, target)
}

pub(crate) fn vue3_resolve_tsconfig_path(config_dir: &Path, target: &str) -> Option<PathBuf> {
    if !vue3_tsconfig_path_is_relative(target) && !Path::new(target).is_absolute() {
        return None;
    }
    let candidate = if Path::new(target).is_absolute() {
        normalize_path_components(PathBuf::from(target))
    } else {
        normalize_path_components(config_dir.join(target))
    };
    resolve_vue3_tsconfig_candidate_path(&candidate, false)
}

pub(crate) fn vue3_tsconfig_path_is_relative(target: &str) -> bool {
    target.starts_with("./") || target.starts_with("../")
}

pub(crate) fn resolve_vue3_tsconfig_candidate_path(
    candidate: &Path,
    include_index: bool,
) -> Option<PathBuf> {
    let mut candidates = if candidate.extension().is_some() {
        vec![candidate.to_path_buf()]
    } else {
        vec![
            path_with_extension(candidate, "json"),
            candidate.join("tsconfig.json"),
        ]
    };
    if include_index && candidate.extension().is_none() {
        candidates.push(candidate.join("index.json"));
    }
    candidates.into_iter().find(|path| path.is_file())
}

pub(crate) fn resolve_vue3_package_tsconfig_extends(
    config_dir: &Path,
    target: &str,
) -> Option<PathBuf> {
    let (package_name, subpath) = vue3_package_import_parts(target)?;
    for node_modules in vue3_node_modules_search_paths_from_dir(config_dir) {
        let package_dir = normalize_path_components(node_modules.join(&package_name));
        if !package_dir.is_dir() {
            continue;
        }
        if let Some(resolved) =
            resolve_vue3_package_tsconfig_entry(&package_dir, subpath.as_deref())
        {
            return Some(resolved);
        }
    }
    None
}

pub(crate) fn resolve_vue3_package_tsconfig_entry(
    package_dir: &Path,
    subpath: Option<&str>,
) -> Option<PathBuf> {
    if let Some(subpath) = subpath {
        return vue3_package_tsconfig_subpath(package_dir, subpath);
    }
    vue3_package_json_tsconfig_entry(package_dir)
        .or_else(|| resolve_vue3_tsconfig_candidate_path(&package_dir.join("tsconfig"), false))
        .or_else(|| {
            let index = package_dir.join("index.json");
            index.is_file().then_some(index)
        })
}

pub(crate) fn vue3_package_tsconfig_subpath(package_dir: &Path, subpath: &str) -> Option<PathBuf> {
    if !vue3_package_tsconfig_subpath_is_safe(subpath) {
        return None;
    }
    let candidate = normalize_path_components(package_dir.join(subpath));
    resolve_vue3_tsconfig_candidate_path(&candidate, true)
}

pub(crate) fn vue3_package_json_tsconfig_entry(package_dir: &Path) -> Option<PathBuf> {
    let package_json = package_dir.join("package.json");
    let source = std::fs::read_to_string(package_json).ok()?;
    let value = serde_json::from_str::<serde_json::Value>(&source).ok()?;
    let target = value.get("tsconfig").and_then(serde_json::Value::as_str)?;
    if !vue3_package_tsconfig_target_is_safe(target) {
        return None;
    }
    let target = target.trim_start_matches("./");
    let candidate = normalize_path_components(package_dir.join(target));
    resolve_vue3_tsconfig_candidate_path(&candidate, true)
}

pub(crate) fn vue3_package_tsconfig_subpath_is_safe(subpath: &str) -> bool {
    !subpath.is_empty()
        && !subpath.contains(':')
        && !subpath
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        && Path::new(subpath).components().all(|component| {
            matches!(
                component,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )
        })
}

pub(crate) fn vue3_package_tsconfig_target_is_safe(target: &str) -> bool {
    !target.is_empty()
        && !target.contains(':')
        && !Path::new(target).is_absolute()
        && Path::new(target).components().all(|component| {
            matches!(
                component,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )
        })
}

pub(crate) fn vue3_tsconfig_direct_path_mappings(
    value: &serde_json::Value,
    config_dir: &Path,
    template_config_dir: &Path,
) -> Vec<Vue3TsconfigPathMapping> {
    let Some(compiler_options) = value
        .get("compilerOptions")
        .and_then(serde_json::Value::as_object)
    else {
        return Vec::new();
    };
    let target_base_dir = compiler_options
        .get("baseUrl")
        .and_then(serde_json::Value::as_str)
        .map(|base_url| vue3_tsconfig_target_path(config_dir, template_config_dir, base_url, ""))
        .unwrap_or_else(|| config_dir.to_path_buf());
    let Some(paths) = compiler_options
        .get("paths")
        .and_then(serde_json::Value::as_object)
    else {
        return Vec::new();
    };
    paths
        .iter()
        .filter_map(|(pattern, targets)| {
            let targets = vue3_tsconfig_path_target_values(targets);
            (!targets.is_empty()).then(|| Vue3TsconfigPathMapping {
                pattern: pattern.clone(),
                targets,
                target_base_dir: target_base_dir.clone(),
                template_config_dir: template_config_dir.to_path_buf(),
            })
        })
        .collect()
}

pub(crate) fn vue3_tsconfig_path_target_values(value: &serde_json::Value) -> Vec<String> {
    match value {
        serde_json::Value::Array(targets) => targets
            .iter()
            .filter_map(serde_json::Value::as_str)
            .map(str::to_string)
            .collect(),
        serde_json::Value::String(target) => vec![target.to_string()],
        _ => Vec::new(),
    }
}

pub(crate) fn resolve_vue3_tsconfig_path_mappings(
    mappings: &[Vue3TsconfigPathMapping],
    source: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    let mut matches = mappings
        .iter()
        .enumerate()
        .filter_map(|(order, mapping)| {
            vue3_tsconfig_path_pattern_capture(&mapping.pattern, source).map(|(score, capture)| {
                Vue3TsconfigPathMatch {
                    mapping,
                    capture,
                    score,
                    order,
                }
            })
        })
        .collect::<Vec<_>>();
    matches.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.order.cmp(&right.order))
    });
    for matched in matches {
        for target in &matched.mapping.targets {
            let candidate = vue3_tsconfig_target_path(
                &matched.mapping.target_base_dir,
                &matched.mapping.template_config_dir,
                target,
                &matched.capture,
            );
            if let Some(resolved) = resolve_vue3_type_import_path(&candidate, type_resolver) {
                return Some(resolved);
            }
        }
    }
    None
}

pub(crate) fn vue3_tsconfig_path_pattern_capture(
    pattern: &str,
    source: &str,
) -> Option<(usize, String)> {
    let Some(star) = pattern.find('*') else {
        return (pattern == source).then(|| (usize::MAX, String::new()));
    };
    if pattern[star + 1..].contains('*') {
        return None;
    }
    let prefix = &pattern[..star];
    let suffix = &pattern[star + 1..];
    if !source.starts_with(prefix)
        || !source.ends_with(suffix)
        || source.len() < prefix.len() + suffix.len()
    {
        return None;
    }
    Some((
        prefix.len() + suffix.len(),
        source[prefix.len()..source.len() - suffix.len()].to_string(),
    ))
}

pub(crate) fn vue3_tsconfig_target_path(
    target_base_dir: &Path,
    template_config_dir: &Path,
    target: &str,
    capture: &str,
) -> PathBuf {
    let target = target.replace('*', capture);
    let target = target.replace(
        "${configDir}",
        normalize_path_string(template_config_dir).as_str(),
    );
    let path = Path::new(&target);
    if path.is_absolute() {
        normalize_path_components(PathBuf::from(target))
    } else {
        normalize_path_components(target_base_dir.join(target))
    }
}

pub(crate) fn resolve_vue3_bare_type_import(
    filename: &str,
    source: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    let (package_name, subpath) = vue3_package_import_parts(source)?;
    for node_modules in vue3_node_modules_search_paths(filename) {
        let package_dir = node_modules.join(&package_name);
        if package_dir.is_dir() {
            if let Some(resolved) =
                resolve_vue3_package_type_entry(&package_dir, subpath.as_deref(), type_resolver)
            {
                return Some(resolved);
            }
        }
        let types_package_dir = node_modules.join(vue3_at_types_package_name(&package_name));
        if types_package_dir.is_dir() {
            if let Some(resolved) = resolve_vue3_package_type_entry(
                &types_package_dir,
                subpath.as_deref(),
                type_resolver,
            ) {
                return Some(resolved);
            }
        }
    }
    None
}

pub(crate) fn vue3_package_import_parts(source: &str) -> Option<(String, Option<String>)> {
    if source.is_empty()
        || source.starts_with('.')
        || source.starts_with('/')
        || source.starts_with('#')
        || source.contains(':')
    {
        return None;
    }
    let parts = source.split('/').collect::<Vec<_>>();
    if parts.first().is_some_and(|part| part.starts_with('@')) {
        if parts.len() < 2 || parts[0].len() <= 1 || parts[1].is_empty() {
            return None;
        }
        let package_name = format!("{}/{}", parts[0], parts[1]);
        let subpath = (parts.len() > 2).then(|| parts[2..].join("/"));
        return Some((package_name, subpath));
    }
    let package_name = parts.first().filter(|part| !part.is_empty())?.to_string();
    let subpath = (parts.len() > 1).then(|| parts[1..].join("/"));
    Some((package_name, subpath))
}

pub(crate) fn vue3_node_modules_search_paths(filename: &str) -> Vec<PathBuf> {
    let Some(current) = Path::new(filename).parent() else {
        return Vec::new();
    };
    vue3_node_modules_search_paths_from_dir(current)
}

pub(crate) fn vue3_node_modules_search_paths_from_dir(start_dir: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let mut current = Some(start_dir);
    while let Some(dir) = current {
        paths.push(normalize_path_components(dir.join("node_modules")));
        current = dir.parent();
    }
    paths
}

pub(crate) fn vue3_type_resolver_context_for_filename(filename: &str) -> Vue3TypeResolverContext {
    Vue3TypeResolverContext {
        typescript_version: vue3_typescript_version_for_filename(filename)
            .unwrap_or_else(vue3_package_typescript_baseline_version),
    }
}

pub(crate) fn vue3_typescript_version_for_filename(
    filename: &str,
) -> Option<nodejs_semver::Version> {
    vue3_node_modules_search_paths(filename)
        .into_iter()
        .find_map(|node_modules| {
            vue3_typescript_version_from_package_json(
                &node_modules.join("typescript").join("package.json"),
            )
        })
}

pub(crate) fn vue3_typescript_version_from_package_json(
    package_json: &Path,
) -> Option<nodejs_semver::Version> {
    let source = std::fs::read_to_string(package_json).ok()?;
    let package = serde_json::from_str::<serde_json::Value>(&source).ok()?;
    let version = package.get("version")?.as_str()?.trim();
    nodejs_semver::Version::parse(version).ok()
}

pub(crate) fn vue3_at_types_package_name(package_name: &str) -> PathBuf {
    if let Some(scoped) = package_name.strip_prefix('@') {
        return PathBuf::from("@types").join(scoped.replace('/', "__"));
    }
    PathBuf::from("@types").join(package_name)
}

pub(crate) fn resolve_vue3_package_type_entry(
    package_dir: &Path,
    subpath: Option<&str>,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    match resolve_vue3_package_json_type_entry(package_dir, subpath, type_resolver) {
        Vue3PackageJsonTypeResolution::Resolved(path) => return Some(path),
        Vue3PackageJsonTypeResolution::Blocked => return None,
        Vue3PackageJsonTypeResolution::NoPackageJson
        | Vue3PackageJsonTypeResolution::NoPackageTypeEntry => {}
    }
    let candidate = subpath
        .map(|subpath| package_dir.join(subpath))
        .unwrap_or_else(|| package_dir.to_path_buf());
    resolve_vue3_type_import_path(&candidate, type_resolver)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Vue3PackageJsonTypeResolution {
    NoPackageJson,
    NoPackageTypeEntry,
    Resolved(PathBuf),
    Blocked,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Vue3PackageJsonTypeManifest {
    #[serde(default)]
    pub(crate) exports: Option<serde_json::Value>,
    #[serde(default)]
    pub(crate) types: Option<serde_json::Value>,
    #[serde(default)]
    pub(crate) typings: Option<serde_json::Value>,
    #[serde(default, rename = "typesVersions")]
    pub(crate) types_versions: Vue3PackageTypesVersions,
}

#[derive(Debug, Default)]
pub(crate) struct Vue3PackageTypesVersions(Vec<Vue3PackageTypesVersionEntry>);

#[derive(Debug)]
pub(crate) struct Vue3PackageTypesVersionEntry {
    pub(crate) selector: String,
    pub(crate) mappings: Vue3PackageTypesVersionMappings,
}

#[derive(Debug, Default)]
pub(crate) struct Vue3PackageTypesVersionMappings(Vec<(String, serde_json::Value)>);

impl<'de> Deserialize<'de> for Vue3PackageTypesVersions {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct TypesVersionsVisitor;

        impl<'de> Visitor<'de> for TypesVersionsVisitor {
            type Value = Vue3PackageTypesVersions;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a package.json typesVersions object")
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut entries = Vec::new();
                while let Some(selector) = map.next_key::<String>()? {
                    let mappings = map.next_value::<Vue3PackageTypesVersionMappings>()?;
                    if !mappings.0.is_empty() {
                        entries.push(Vue3PackageTypesVersionEntry { selector, mappings });
                    }
                }
                Ok(Vue3PackageTypesVersions(entries))
            }

            fn visit_bool<E>(self, _: bool) -> Result<Self::Value, E> {
                Ok(Vue3PackageTypesVersions::default())
            }

            fn visit_i64<E>(self, _: i64) -> Result<Self::Value, E> {
                Ok(Vue3PackageTypesVersions::default())
            }

            fn visit_u64<E>(self, _: u64) -> Result<Self::Value, E> {
                Ok(Vue3PackageTypesVersions::default())
            }

            fn visit_f64<E>(self, _: f64) -> Result<Self::Value, E> {
                Ok(Vue3PackageTypesVersions::default())
            }

            fn visit_str<E>(self, _: &str) -> Result<Self::Value, E> {
                Ok(Vue3PackageTypesVersions::default())
            }

            fn visit_none<E>(self) -> Result<Self::Value, E> {
                Ok(Vue3PackageTypesVersions::default())
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E> {
                Ok(Vue3PackageTypesVersions::default())
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                while seq.next_element::<IgnoredAny>()?.is_some() {}
                Ok(Vue3PackageTypesVersions::default())
            }
        }

        deserializer.deserialize_any(TypesVersionsVisitor)
    }
}

impl<'de> Deserialize<'de> for Vue3PackageTypesVersionMappings {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct TypesVersionMappingsVisitor;

        impl<'de> Visitor<'de> for TypesVersionMappingsVisitor {
            type Value = Vue3PackageTypesVersionMappings;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a package.json typesVersions mapping object")
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut mappings = Vec::new();
                while let Some(pattern) = map.next_key::<String>()? {
                    mappings.push((pattern, map.next_value()?));
                }
                Ok(Vue3PackageTypesVersionMappings(mappings))
            }

            fn visit_bool<E>(self, _: bool) -> Result<Self::Value, E> {
                Ok(Vue3PackageTypesVersionMappings::default())
            }

            fn visit_i64<E>(self, _: i64) -> Result<Self::Value, E> {
                Ok(Vue3PackageTypesVersionMappings::default())
            }

            fn visit_u64<E>(self, _: u64) -> Result<Self::Value, E> {
                Ok(Vue3PackageTypesVersionMappings::default())
            }

            fn visit_f64<E>(self, _: f64) -> Result<Self::Value, E> {
                Ok(Vue3PackageTypesVersionMappings::default())
            }

            fn visit_str<E>(self, _: &str) -> Result<Self::Value, E> {
                Ok(Vue3PackageTypesVersionMappings::default())
            }

            fn visit_none<E>(self) -> Result<Self::Value, E> {
                Ok(Vue3PackageTypesVersionMappings::default())
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E> {
                Ok(Vue3PackageTypesVersionMappings::default())
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                while seq.next_element::<IgnoredAny>()?.is_some() {}
                Ok(Vue3PackageTypesVersionMappings::default())
            }
        }

        deserializer.deserialize_any(TypesVersionMappingsVisitor)
    }
}

pub(crate) fn resolve_vue3_package_json_type_entry(
    package_dir: &Path,
    subpath: Option<&str>,
    type_resolver: &Vue3TypeResolverContext,
) -> Vue3PackageJsonTypeResolution {
    let package_json = package_dir.join("package.json");
    let Ok(source) = std::fs::read_to_string(package_json) else {
        return Vue3PackageJsonTypeResolution::NoPackageJson;
    };
    let Ok(manifest) = serde_json::from_str::<Vue3PackageJsonTypeManifest>(&source) else {
        return Vue3PackageJsonTypeResolution::NoPackageJson;
    };
    if let Some(exports) = &manifest.exports {
        if let Some(target) = vue3_package_exports_type_target(exports, subpath) {
            if let Some(resolved) =
                vue3_package_export_type_path(package_dir, &target, type_resolver)
            {
                return Vue3PackageJsonTypeResolution::Resolved(resolved);
            }
            return Vue3PackageJsonTypeResolution::Blocked;
        }
        if subpath.is_some() {
            return Vue3PackageJsonTypeResolution::Blocked;
        }
    }
    let root_type_target = if subpath.is_none() {
        manifest
            .types
            .as_ref()
            .and_then(serde_json::Value::as_str)
            .or_else(|| {
                manifest
                    .typings
                    .as_ref()
                    .and_then(serde_json::Value::as_str)
            })
    } else {
        None
    };
    if let Some(resolved) = vue3_package_types_versions_type_path(
        package_dir,
        &manifest.types_versions,
        subpath,
        root_type_target,
        type_resolver,
    ) {
        return Vue3PackageJsonTypeResolution::Resolved(resolved);
    }
    if subpath.is_none() {
        if let Some(target) = root_type_target {
            if let Some(resolved) = vue3_package_type_field_path(package_dir, target, type_resolver)
            {
                return Vue3PackageJsonTypeResolution::Resolved(resolved);
            }
        }
    }
    Vue3PackageJsonTypeResolution::NoPackageTypeEntry
}

pub(crate) fn vue3_package_exports_type_target(
    exports: &serde_json::Value,
    subpath: Option<&str>,
) -> Option<String> {
    let key = subpath
        .map(|subpath| format!("./{}", subpath.trim_start_matches("./")))
        .unwrap_or_else(|| ".".into());
    let target = if key == "." {
        exports
            .get(".")
            .or_else(|| vue3_package_exports_is_condition_map(exports).then_some(exports))
            .and_then(vue3_package_export_target_value)
    } else {
        exports
            .get(&key)
            .and_then(vue3_package_export_target_value)
            .or_else(|| vue3_package_exports_pattern_target(exports, &key))
    }?;
    Some(target)
}

pub(crate) fn vue3_package_exports_is_condition_map(exports: &serde_json::Value) -> bool {
    exports
        .as_object()
        .is_none_or(|object| !object.keys().any(|key| key == "." || key.starts_with("./")))
}

pub(crate) fn vue3_package_exports_pattern_target(
    exports: &serde_json::Value,
    key: &str,
) -> Option<String> {
    let object = exports.as_object()?;
    for (pattern, target) in object {
        let Some(capture) = vue3_package_export_pattern_capture(pattern, key) else {
            continue;
        };
        let target = vue3_package_export_target_value(target)?;
        return Some(target.replace('*', &capture));
    }
    None
}

pub(crate) fn vue3_package_export_target_value(value: &serde_json::Value) -> Option<String> {
    if let Some(value) = value.as_str() {
        return Some(value.to_string());
    }
    let object = value.as_object()?;
    for condition in ["types", "typings"] {
        if let Some(target) = object
            .get(condition)
            .and_then(vue3_package_export_target_value)
        {
            return Some(target);
        }
    }
    for condition in ["import", "require", "node", "default"] {
        if let Some(target) = object
            .get(condition)
            .and_then(vue3_package_export_target_value)
        {
            return Some(target);
        }
    }
    None
}

pub(crate) fn vue3_package_export_pattern_capture(pattern: &str, key: &str) -> Option<String> {
    let star = pattern.find('*')?;
    let prefix = &pattern[..star];
    let suffix = &pattern[star + 1..];
    if !key.starts_with(prefix) || !key.ends_with(suffix) || key.len() < prefix.len() + suffix.len()
    {
        return None;
    }
    Some(key[prefix.len()..key.len() - suffix.len()].to_string())
}

pub(crate) fn vue3_package_export_type_path(
    package_dir: &Path,
    target: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    if !target.starts_with("./") {
        return None;
    }
    vue3_package_type_target_path(package_dir, target, type_resolver)
}

pub(crate) fn vue3_package_types_versions_type_path(
    package_dir: &Path,
    types_versions: &Vue3PackageTypesVersions,
    subpath: Option<&str>,
    root_type_target: Option<&str>,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    let mappings = vue3_package_types_versions_mapping(types_versions, type_resolver)?;
    let source = subpath
        .map(|subpath| subpath.trim_start_matches("./").to_string())
        .or_else(|| root_type_target.map(|target| target.trim_start_matches("./").to_string()))
        .unwrap_or_else(|| "index.d.ts".to_string());
    let mut matches = mappings
        .0
        .iter()
        .enumerate()
        .filter_map(|(order, (pattern, targets))| {
            let targets = vue3_tsconfig_path_target_values(targets);
            if targets.is_empty() {
                return None;
            }
            vue3_tsconfig_path_pattern_capture(pattern, &source)
                .map(|(score, capture)| (score, order, capture, targets))
        })
        .collect::<Vec<_>>();
    matches.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
    for (_, _, capture, targets) in matches {
        for target in targets {
            let target = target.replace('*', &capture);
            if let Some(resolved) =
                vue3_package_type_field_path(package_dir, &target, type_resolver)
            {
                return Some(resolved);
            }
        }
    }
    None
}

pub(crate) fn vue3_package_types_versions_mapping<'a>(
    types_versions: &'a Vue3PackageTypesVersions,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<&'a Vue3PackageTypesVersionMappings> {
    types_versions
        .0
        .iter()
        .find(|entry| {
            vue3_package_types_version_selector_matches_version(
                &entry.selector,
                &type_resolver.typescript_version,
            )
        })
        .map(|entry| &entry.mappings)
}

#[cfg(test)]
pub(crate) fn vue3_package_types_version_selector_matches(selector: &str) -> bool {
    vue3_package_types_version_selector_matches_version(
        selector,
        &vue3_package_typescript_baseline_version(),
    )
}

pub(crate) fn vue3_package_types_version_selector_matches_version(
    selector: &str,
    typescript_version: &nodejs_semver::Version,
) -> bool {
    let selector = selector.trim();
    if selector.is_empty() {
        return false;
    }
    nodejs_semver::Range::parse(selector).is_ok_and(|range| range.satisfies(typescript_version))
}

pub(crate) fn vue3_package_typescript_baseline_version() -> nodejs_semver::Version {
    // Bounded SFC resolver baseline for the locked Vue 3 compiler-sfc harness.
    (5, 0, 0).into()
}

pub(crate) fn vue3_package_type_field_path(
    package_dir: &Path,
    target: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    if Path::new(target).is_absolute() || target.starts_with("../") {
        return None;
    }
    vue3_package_type_target_path(package_dir, target.trim_start_matches("./"), type_resolver)
}

pub(crate) fn vue3_package_type_target_path(
    package_dir: &Path,
    target: &str,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    let candidate = normalize_path_components(package_dir.join(target));
    if candidate
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| matches!(extension, "js" | "jsx" | "mjs" | "cjs"))
    {
        let extension = candidate
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default();
        let stem = candidate.with_extension("");
        if let Some(resolved) = vue3_ts_resolution_candidates(&stem, extension)
            .into_iter()
            .find(|candidate| candidate.exists())
        {
            return Some(resolved);
        }
    }
    resolve_vue3_type_import_path(&candidate, type_resolver)
}

pub(crate) fn resolve_vue3_type_import_path(
    candidate: &Path,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<PathBuf> {
    let extension = candidate
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default();
    let stem = candidate.with_extension("");
    let mut candidates = Vec::new();
    if !extension.is_empty() {
        if !matches!(
            extension,
            "ts" | "tsx" | "mts" | "cts" | "js" | "jsx" | "mjs" | "cjs"
        ) {
            candidates.push(arbitrary_extension_type_candidate(&stem, extension));
        }
        if matches!(extension, "js" | "jsx" | "mjs" | "cjs") {
            candidates.extend(vue3_ts_resolution_candidates(&stem, extension));
        }
        candidates.push(candidate.to_path_buf());
    } else {
        if candidate.is_dir() {
            match resolve_vue3_package_json_type_entry(candidate, None, type_resolver) {
                Vue3PackageJsonTypeResolution::Resolved(path) => return Some(path),
                Vue3PackageJsonTypeResolution::Blocked => return None,
                Vue3PackageJsonTypeResolution::NoPackageJson
                | Vue3PackageJsonTypeResolution::NoPackageTypeEntry => {}
            }
        }
        candidates.extend(vue3_ts_resolution_candidates(candidate, extension));
        candidates.push(candidate.join("index.ts"));
        candidates.push(candidate.join("index.tsx"));
        candidates.push(candidate.join("index.d.ts"));
    }
    candidates.into_iter().find(|candidate| candidate.exists())
}

pub(crate) fn arbitrary_extension_type_candidate(stem: &Path, extension: &str) -> PathBuf {
    let Some(file_name) = stem.file_name().and_then(|name| name.to_str()) else {
        return stem.with_extension(format!("d.{extension}.ts"));
    };
    let mut candidate = stem.to_path_buf();
    candidate.set_file_name(format!("{file_name}.d.{extension}.ts"));
    candidate
}

pub(crate) fn vue3_ts_resolution_candidates(base: &Path, extension: &str) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if extension == "mjs" {
        candidates.push(path_with_extension(base, "mts"));
        candidates.push(path_with_extension(base, "d.mts"));
    } else if extension == "cjs" {
        candidates.push(path_with_extension(base, "cts"));
        candidates.push(path_with_extension(base, "d.cts"));
    }
    candidates.push(path_with_extension(base, "ts"));
    candidates.push(path_with_extension(base, "tsx"));
    candidates.push(path_with_extension(base, "d.ts"));
    if extension.is_empty() {
        candidates.push(path_with_extension(base, "mts"));
        candidates.push(path_with_extension(base, "d.mts"));
        candidates.push(path_with_extension(base, "cts"));
        candidates.push(path_with_extension(base, "d.cts"));
    }
    candidates
}

pub(crate) fn path_with_extension(path: &Path, extension: &str) -> PathBuf {
    let mut path = path.to_path_buf();
    path.set_extension(extension);
    path
}

pub(crate) fn normalize_path_components(path: PathBuf) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

pub(crate) fn normalize_path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub(crate) fn vue3_normal_script_user_imports(descriptor: &SfcDescriptor) -> Vue3UserImports {
    let Some(script) = descriptor.script.as_ref() else {
        return Vue3UserImports::default();
    };
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        script.content.as_str(),
        script_source_type_from_attrs(&script.attrs),
    )
    .with_options(oxc_parser::ParseOptions {
        parse_regular_expression: true,
        ..oxc_parser::ParseOptions::default()
    })
    .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return Vue3UserImports::default();
    }
    let mut user_imports = Vue3UserImports::default();
    for statement in &parsed.program.body {
        let Statement::ImportDeclaration(import) = statement else {
            continue;
        };
        let source = import.source.value.as_str();
        if let Some(specifiers) = &import.specifiers {
            for specifier in specifiers {
                if let Some(imported) = import_specifier_imported(specifier) {
                    user_imports.record(Vue27ScriptImport {
                        local: import_specifier_local(specifier),
                        source: source.to_string(),
                        imported,
                        is_type: vue27_import_specifier_is_type(import, specifier),
                    });
                }
            }
        }
    }
    user_imports
}

pub(crate) fn collect_vue3_declared_types_from_statements(
    source: &str,
    statements: &[Statement<'_>],
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    for statement in statements {
        collect_vue3_predeclared_runtime_type_from_statement(statement, analysis);
    }
    for statement in statements {
        collect_vue3_declared_type_from_statement(source, statement, analysis);
    }
    refresh_vue3_declared_type_declarations_from_statements(source, statements, analysis);
}

pub(crate) fn collect_vue3_predeclared_runtime_type_from_statement(
    statement: &Statement<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    match statement {
        Statement::ClassDeclaration(declaration) => {
            if let Some(id) = &declaration.id {
                register_vue3_class_type_name(analysis, id.name.as_str());
            }
        }
        Statement::TSEnumDeclaration(declaration) if !declaration.declare => {
            register_vue3_ts_enum_declaration(declaration, analysis);
        }
        Statement::ExportNamedDeclaration(declaration) => {
            if let Some(declaration) = &declaration.declaration {
                match declaration {
                    Declaration::ClassDeclaration(declaration) => {
                        if let Some(id) = &declaration.id {
                            register_vue3_class_type_name(analysis, id.name.as_str());
                        }
                    }
                    Declaration::TSEnumDeclaration(declaration) if !declaration.declare => {
                        register_vue3_ts_enum_declaration(declaration, analysis);
                    }
                    _ => {}
                }
            }
        }
        Statement::ExportDefaultDeclaration(declaration) => {
            if let ExportDefaultDeclarationKind::ClassDeclaration(class) = &declaration.declaration
            {
                if let Some(id) = &class.id {
                    register_vue3_class_type_name(analysis, id.name.as_str());
                }
            }
        }
        _ => {}
    }
}

pub(crate) fn collect_vue3_declared_type_deps_from_statements(
    statements: &[Statement<'_>],
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    for statement in statements {
        collect_vue3_declared_type_deps_from_statement(statement, analysis);
    }
}

pub(crate) fn collect_vue3_declared_type_deps_from_statement(
    statement: &Statement<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    match statement {
        Statement::TSInterfaceDeclaration(declaration) => {
            let deps = collect_vue3_interface_type_deps(declaration, analysis);
            insert_vue3_declared_type_deps(analysis, declaration.id.name.as_str(), deps);
        }
        Statement::TSTypeAliasDeclaration(declaration) => {
            let deps = collect_vue3_type_argument_deps(&declaration.type_annotation, analysis);
            insert_vue3_declared_type_deps(analysis, declaration.id.name.as_str(), deps);
        }
        Statement::FunctionDeclaration(function) if function.return_type.is_some() => {
            if let (Some(id), Some(return_type)) = (&function.id, function.return_type.as_ref()) {
                let deps = collect_vue3_type_argument_deps(&return_type.type_annotation, analysis);
                insert_vue3_declared_type_deps(analysis, id.name.as_str(), deps);
            }
        }
        Statement::VariableDeclaration(declaration) if declaration.declare => {
            for declarator in &declaration.declarations {
                let Some(name) = first_pattern_binding(&declarator.id) else {
                    continue;
                };
                let Some(type_annotation) = declarator.type_annotation.as_ref() else {
                    continue;
                };
                let deps =
                    collect_vue3_type_argument_deps(&type_annotation.type_annotation, analysis);
                insert_vue3_declared_type_deps(analysis, &name, deps);
            }
        }
        Statement::VariableDeclaration(declaration) => {
            collect_vue3_function_value_return_type_deps_from_variable(declaration, analysis);
            collect_vue3_static_runtime_props_options_deps_from_variable(declaration, analysis);
        }
        Statement::ExportNamedDeclaration(declaration) => {
            if let Some(declaration) = &declaration.declaration {
                collect_vue3_declared_type_deps_from_declaration(declaration, analysis);
            }
        }
        Statement::ExportDefaultDeclaration(declaration) => {
            if let ExportDefaultDeclarationKind::TSInterfaceDeclaration(declaration) =
                &declaration.declaration
            {
                let deps = collect_vue3_interface_type_deps(declaration, analysis);
                insert_vue3_declared_type_deps(analysis, declaration.id.name.as_str(), deps);
            }
        }
        _ => {}
    }
}

pub(crate) fn collect_vue3_declared_type_deps_from_declaration(
    declaration: &Declaration<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    match declaration {
        Declaration::TSInterfaceDeclaration(declaration) => {
            let deps = collect_vue3_interface_type_deps(declaration, analysis);
            insert_vue3_declared_type_deps(analysis, declaration.id.name.as_str(), deps);
        }
        Declaration::TSTypeAliasDeclaration(declaration) => {
            let deps = collect_vue3_type_argument_deps(&declaration.type_annotation, analysis);
            insert_vue3_declared_type_deps(analysis, declaration.id.name.as_str(), deps);
        }
        Declaration::FunctionDeclaration(function) if function.return_type.is_some() => {
            if let (Some(id), Some(return_type)) = (&function.id, function.return_type.as_ref()) {
                let deps = collect_vue3_type_argument_deps(&return_type.type_annotation, analysis);
                insert_vue3_declared_type_deps(analysis, id.name.as_str(), deps);
            }
        }
        Declaration::VariableDeclaration(declaration) if declaration.declare => {
            for declarator in &declaration.declarations {
                let Some(name) = first_pattern_binding(&declarator.id) else {
                    continue;
                };
                let Some(type_annotation) = declarator.type_annotation.as_ref() else {
                    continue;
                };
                let deps =
                    collect_vue3_type_argument_deps(&type_annotation.type_annotation, analysis);
                insert_vue3_declared_type_deps(analysis, &name, deps);
            }
        }
        Declaration::VariableDeclaration(declaration) => {
            collect_vue3_function_value_return_type_deps_from_variable(declaration, analysis);
            collect_vue3_static_runtime_props_options_deps_from_variable(declaration, analysis);
        }
        _ => {}
    }
}

pub(crate) fn collect_vue3_interface_type_deps(
    declaration: &TSInterfaceDeclaration<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> BTreeSet<String> {
    let mut deps = BTreeSet::new();
    for heritage in &declaration.extends {
        if let Some(name) = vue3_interface_heritage_name(heritage) {
            collect_vue3_named_type_deps(&name, analysis, &mut deps);
        }
        if let Some(type_arguments) = heritage.type_arguments.as_ref() {
            for ty in &type_arguments.params {
                collect_vue3_type_argument_deps_into(ty, analysis, &mut deps);
            }
        }
    }
    for signature in &declaration.body.body {
        collect_vue3_signature_type_deps(signature, analysis, &mut deps);
    }
    deps
}

pub(crate) fn insert_vue3_declared_type_deps(
    analysis: &mut Vue3ScriptSetupAnalysis,
    name: &str,
    deps: BTreeSet<String>,
) {
    if !deps.is_empty() {
        analysis
            .type_deps
            .entry(name.to_string())
            .or_default()
            .extend(deps);
    }
}

pub(crate) fn collect_vue3_declared_type_from_statement(
    source: &str,
    statement: &Statement<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    match statement {
        Statement::TSInterfaceDeclaration(declaration) => {
            register_vue3_interface_declaration(source, declaration, analysis);
        }
        Statement::TSTypeAliasDeclaration(declaration) => {
            register_vue3_type_alias_declaration(source, declaration, analysis);
        }
        Statement::FunctionDeclaration(function)
            if vue3_function_has_return_projection(function) =>
        {
            register_vue3_declared_function_return_props_options(source, function, analysis);
        }
        Statement::VariableDeclaration(declaration) if declaration.declare => {
            register_vue3_declared_variable_props_options(source, declaration, analysis);
        }
        Statement::VariableDeclaration(declaration) => {
            register_vue3_function_value_return_props_options(source, declaration, analysis);
            register_vue3_static_runtime_props_options(source, declaration, analysis);
        }
        Statement::ExportNamedDeclaration(declaration) => {
            if let Some(declaration) = &declaration.declaration {
                collect_vue3_declared_type_from_declaration(source, declaration, analysis);
            }
        }
        Statement::ClassDeclaration(declaration) => {
            if let Some(id) = &declaration.id {
                register_vue3_class_type_name(analysis, id.name.as_str());
            }
        }
        Statement::TSEnumDeclaration(declaration) if !declaration.declare => {
            register_vue3_ts_enum_declaration(declaration, analysis);
        }
        Statement::TSModuleDeclaration(declaration) => {
            project_vue3_namespace_declaration(source, declaration, analysis);
        }
        _ => {}
    }
}

pub(crate) fn collect_vue3_declared_type_from_declaration(
    source: &str,
    declaration: &Declaration<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    match declaration {
        Declaration::TSInterfaceDeclaration(declaration) => {
            register_vue3_interface_declaration(source, declaration, analysis);
        }
        Declaration::TSTypeAliasDeclaration(declaration) => {
            register_vue3_type_alias_declaration(source, declaration, analysis);
        }
        Declaration::FunctionDeclaration(function)
            if vue3_function_has_return_projection(function) =>
        {
            register_vue3_declared_function_return_props_options(source, function, analysis);
        }
        Declaration::VariableDeclaration(declaration) if declaration.declare => {
            register_vue3_declared_variable_props_options(source, declaration, analysis);
        }
        Declaration::VariableDeclaration(declaration) => {
            register_vue3_function_value_return_props_options(source, declaration, analysis);
            register_vue3_static_runtime_props_options(source, declaration, analysis);
        }
        Declaration::TSModuleDeclaration(declaration) => {
            project_vue3_namespace_declaration(source, declaration, analysis);
        }
        Declaration::ClassDeclaration(declaration) => {
            if let Some(id) = &declaration.id {
                register_vue3_class_type_name(analysis, id.name.as_str());
            }
        }
        Declaration::TSEnumDeclaration(declaration) if !declaration.declare => {
            register_vue3_ts_enum_declaration(declaration, analysis);
        }
        _ => {}
    }
}

pub(crate) fn refresh_vue3_declared_type_declarations_from_statements(
    source: &str,
    statements: &[Statement<'_>],
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    let limit = count_vue3_refreshable_type_declarations_in_statements(statements);
    for _ in 0..limit {
        if !refresh_vue3_declared_type_declarations_from_statements_once(
            source, statements, analysis,
        ) {
            break;
        }
    }
}

pub(crate) fn refresh_vue3_declared_type_declarations_from_statements_once(
    source: &str,
    statements: &[Statement<'_>],
    analysis: &mut Vue3ScriptSetupAnalysis,
) -> bool {
    let mut changed = false;
    let interface_declarations = vue3_interface_declarations_by_name(statements);
    let mut refreshed_interfaces = BTreeSet::new();
    for statement in statements {
        changed |= refresh_vue3_declared_type_declaration_from_statement(
            source,
            statement,
            &interface_declarations,
            &mut refreshed_interfaces,
            analysis,
        );
    }
    changed
}

pub(crate) fn vue3_interface_declarations_by_name<'a>(
    statements: &'a [Statement<'a>],
) -> BTreeMap<String, Vec<&'a TSInterfaceDeclaration<'a>>> {
    let mut declarations = BTreeMap::new();
    for statement in statements {
        collect_vue3_interface_declarations_from_statement(statement, &mut declarations);
    }
    declarations
}

pub(crate) fn collect_vue3_interface_declarations_from_statement<'a>(
    statement: &'a Statement<'a>,
    declarations: &mut BTreeMap<String, Vec<&'a TSInterfaceDeclaration<'a>>>,
) {
    match statement {
        Statement::TSInterfaceDeclaration(declaration) => {
            declarations
                .entry(declaration.id.name.to_string())
                .or_default()
                .push(declaration);
        }
        Statement::ExportNamedDeclaration(declaration) => {
            if let Some(Declaration::TSInterfaceDeclaration(declaration)) = &declaration.declaration
            {
                declarations
                    .entry(declaration.id.name.to_string())
                    .or_default()
                    .push(declaration);
            }
        }
        _ => {}
    }
}

pub(crate) fn refresh_vue3_declared_type_declaration_from_statement(
    source: &str,
    statement: &Statement<'_>,
    interface_declarations: &BTreeMap<String, Vec<&TSInterfaceDeclaration<'_>>>,
    refreshed_interfaces: &mut BTreeSet<String>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) -> bool {
    match statement {
        Statement::TSInterfaceDeclaration(declaration) => refresh_vue3_interface_declaration_group(
            source,
            declaration,
            interface_declarations,
            refreshed_interfaces,
            analysis,
        ),
        Statement::TSTypeAliasDeclaration(declaration) => {
            refresh_vue3_type_alias_declaration(source, declaration, analysis)
        }
        Statement::ExportNamedDeclaration(declaration) => {
            declaration.declaration.as_ref().is_some_and(|declaration| {
                refresh_vue3_declared_type_declaration_from_declaration(
                    source,
                    declaration,
                    interface_declarations,
                    refreshed_interfaces,
                    analysis,
                )
            })
        }
        _ => false,
    }
}

pub(crate) fn refresh_vue3_declared_type_declaration_from_declaration(
    source: &str,
    declaration: &Declaration<'_>,
    interface_declarations: &BTreeMap<String, Vec<&TSInterfaceDeclaration<'_>>>,
    refreshed_interfaces: &mut BTreeSet<String>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) -> bool {
    match declaration {
        Declaration::TSInterfaceDeclaration(declaration) => {
            refresh_vue3_interface_declaration_group(
                source,
                declaration,
                interface_declarations,
                refreshed_interfaces,
                analysis,
            )
        }
        Declaration::TSTypeAliasDeclaration(declaration) => {
            refresh_vue3_type_alias_declaration(source, declaration, analysis)
        }
        _ => false,
    }
}

pub(crate) fn count_vue3_refreshable_type_declarations_in_statements(
    statements: &[Statement<'_>],
) -> usize {
    statements
        .iter()
        .map(count_vue3_refreshable_type_declarations_in_statement)
        .sum()
}

pub(crate) fn count_vue3_refreshable_type_declarations_in_statement(
    statement: &Statement<'_>,
) -> usize {
    match statement {
        Statement::TSInterfaceDeclaration(_) | Statement::TSTypeAliasDeclaration(_) => 1,
        Statement::ExportNamedDeclaration(declaration) => declaration
            .declaration
            .as_ref()
            .map(count_vue3_refreshable_type_declarations_in_declaration)
            .unwrap_or_default(),
        _ => 0,
    }
}

pub(crate) fn count_vue3_refreshable_type_declarations_in_declaration(
    declaration: &Declaration<'_>,
) -> usize {
    match declaration {
        Declaration::TSInterfaceDeclaration(_) | Declaration::TSTypeAliasDeclaration(_) => 1,
        _ => 0,
    }
}

pub(crate) fn register_vue3_interface_declaration(
    source: &str,
    declaration: &TSInterfaceDeclaration<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    let name = declaration.id.name.to_string();
    register_vue3_local_type_name(analysis, &name);
    analysis
        .declared_types
        .insert(name.clone(), vec!["Object".into()]);
    analysis
        .define_model_declared_types
        .insert(name.clone(), vec!["Object".into()]);
    refresh_vue3_interface_declaration(source, declaration, analysis);
}

pub(crate) fn refresh_vue3_interface_declaration(
    source: &str,
    declaration: &TSInterfaceDeclaration<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) -> bool {
    let mut changed = refresh_vue3_generic_interface_declaration(source, declaration, analysis);
    changed |= refresh_vue3_merged_interface_declarations(source, &[declaration], analysis);
    changed
}

pub(crate) fn refresh_vue3_generic_interface_declaration(
    source: &str,
    declaration: &TSInterfaceDeclaration<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) -> bool {
    let name = declaration.id.name.as_str();
    let Some(type_parameters) = declaration.type_parameters.as_ref() else {
        if analysis
            .generic_type_aliases
            .get(name)
            .is_some_and(|alias| alias.kind == Vue3GenericTypeAliasKind::Interface)
        {
            analysis.generic_type_aliases.remove(name);
            return true;
        }
        return false;
    };
    let params = type_parameters
        .params
        .iter()
        .map(|param| param.name.name.to_string())
        .collect::<Vec<_>>();
    let alias_source = source
        .get(declaration.span.start as usize..declaration.span.end as usize)
        .unwrap_or_default()
        .to_string();
    if params.is_empty() || alias_source.is_empty() {
        return false;
    }
    let alias = vue3_generic_type_alias(
        alias_source,
        Vue3GenericTypeAliasKind::Interface,
        params,
        analysis,
    );
    if analysis.generic_type_aliases.get(name) != Some(&alias) {
        analysis
            .generic_type_aliases
            .insert(name.to_string(), alias);
        true
    } else {
        false
    }
}

pub(crate) fn refresh_vue3_non_generic_interface_declaration(
    source: &str,
    declaration: &TSInterfaceDeclaration<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) -> bool {
    refresh_vue3_merged_interface_declarations(source, &[declaration], analysis)
}

pub(crate) fn refresh_vue3_interface_declaration_group(
    source: &str,
    declaration: &TSInterfaceDeclaration<'_>,
    interface_declarations: &BTreeMap<String, Vec<&TSInterfaceDeclaration<'_>>>,
    refreshed_interfaces: &mut BTreeSet<String>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) -> bool {
    let name = declaration.id.name.to_string();
    if !refreshed_interfaces.insert(name.clone()) {
        return false;
    }
    let Some(declarations) = interface_declarations.get(&name) else {
        return refresh_vue3_non_generic_interface_declaration(source, declaration, analysis);
    };
    let mut changed = refresh_vue3_generic_interface_declaration(source, declaration, analysis);
    changed |= refresh_vue3_merged_interface_declarations(source, declarations, analysis);
    changed
}

pub(crate) fn refresh_vue3_merged_interface_declarations(
    source: &str,
    declarations: &[&TSInterfaceDeclaration<'_>],
    analysis: &mut Vue3ScriptSetupAnalysis,
) -> bool {
    let Some(first) = declarations.first() else {
        return false;
    };
    let name = first.id.name.to_string();
    let mut changed = false;
    let runtime = infer_vue3_runtime_type_from_interface_declarations(declarations);
    if analysis.declared_types.get(&name) != Some(&runtime) {
        analysis
            .declared_types
            .insert(name.clone(), runtime.clone());
        changed = true;
    }
    if analysis.define_model_declared_types.get(&name) != Some(&runtime) {
        analysis
            .define_model_declared_types
            .insert(name.clone(), runtime);
        changed = true;
    }
    let props = vue3_type_members_from_interface_declarations(source, declarations, analysis);
    if analysis.props_type_declarations.get(&name) != Some(&props) {
        analysis.props_type_declarations.insert(name.clone(), props);
        changed = true;
    }
    match vue3_keyof_runtime_type_from_interface_declarations(source, declarations, analysis) {
        Some(types) => {
            if analysis.keyof_runtime_type_declarations.get(&name) != Some(&types) {
                analysis
                    .keyof_runtime_type_declarations
                    .insert(name.clone(), types);
                changed = true;
            }
        }
        None => {
            if analysis
                .keyof_runtime_type_declarations
                .remove(&name)
                .is_some()
            {
                changed = true;
            }
        }
    }
    let props_parameter_tuple = infer_vue3_function_parameter_tuple_runtime_type_from_interfaces(
        source,
        declarations,
        analysis,
        Vue3ArrayElementRuntimeMode::Props,
    );
    changed |= refresh_vue3_runtime_type_tuple_declaration(
        &mut analysis.parameter_tuple_runtime_type_declarations,
        &name,
        props_parameter_tuple,
    );

    let model_parameter_tuple = infer_vue3_function_parameter_tuple_runtime_type_from_interfaces(
        source,
        declarations,
        analysis,
        Vue3ArrayElementRuntimeMode::DefineModel,
    );
    changed |= refresh_vue3_runtime_type_tuple_declaration(
        &mut analysis.define_model_parameter_tuple_runtime_type_declarations,
        &name,
        model_parameter_tuple,
    );

    let props_constructor_parameter_tuple =
        infer_vue3_constructor_parameter_tuple_runtime_type_from_interfaces(
            source,
            declarations,
            analysis,
            Vue3ArrayElementRuntimeMode::Props,
        );
    changed |= refresh_vue3_runtime_type_tuple_declaration(
        &mut analysis.constructor_parameter_tuple_runtime_type_declarations,
        &name,
        props_constructor_parameter_tuple,
    );

    let model_constructor_parameter_tuple =
        infer_vue3_constructor_parameter_tuple_runtime_type_from_interfaces(
            source,
            declarations,
            analysis,
            Vue3ArrayElementRuntimeMode::DefineModel,
        );
    changed |= refresh_vue3_runtime_type_tuple_declaration(
        &mut analysis.define_model_constructor_parameter_tuple_runtime_type_declarations,
        &name,
        model_constructor_parameter_tuple,
    );

    let props_return_type = infer_vue3_return_runtime_type_from_interfaces(
        source,
        declarations,
        analysis,
        Vue3ArrayElementRuntimeMode::Props,
    );
    changed |= refresh_vue3_runtime_type_declaration(
        &mut analysis.return_type_runtime_type_declarations,
        &name,
        props_return_type,
    );

    let model_return_type = infer_vue3_return_runtime_type_from_interfaces(
        source,
        declarations,
        analysis,
        Vue3ArrayElementRuntimeMode::DefineModel,
    );
    changed |= refresh_vue3_runtime_type_declaration(
        &mut analysis.define_model_return_type_runtime_type_declarations,
        &name,
        model_return_type,
    );

    let emits = vue3_emits_type_from_interface_declarations(source, declarations, analysis);
    if !emits.events.is_empty() {
        if analysis.emits_type_declarations.get(&name) != Some(&emits) {
            analysis.emits_type_declarations.insert(name, emits);
            changed = true;
        }
    } else if analysis.emits_type_declarations.remove(&name).is_some() {
        changed = true;
    }
    changed
}

pub(crate) fn refresh_vue3_runtime_type_tuple_declaration(
    declarations: &mut BTreeMap<String, Vue3RuntimeTypeTuple>,
    name: &str,
    tuple: Option<Vue3RuntimeTypeTuple>,
) -> bool {
    match tuple {
        Some(tuple) => {
            if declarations.get(name) != Some(&tuple) {
                declarations.insert(name.to_string(), tuple);
                return true;
            }
        }
        None => {
            if declarations.remove(name).is_some() {
                return true;
            }
        }
    }
    false
}

pub(crate) fn refresh_vue3_runtime_type_declaration(
    declarations: &mut BTreeMap<String, Vec<String>>,
    name: &str,
    types: Option<Vec<String>>,
) -> bool {
    match types {
        Some(types) => {
            if declarations.get(name) != Some(&types) {
                declarations.insert(name.to_string(), types);
                return true;
            }
        }
        None => {
            if declarations.remove(name).is_some() {
                return true;
            }
        }
    }
    false
}

pub(crate) fn refresh_vue3_type_alias_declaration(
    source: &str,
    declaration: &TSTypeAliasDeclaration<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) -> bool {
    let name = declaration.id.name.to_string();
    let mut changed = false;

    changed |= refresh_vue3_generic_type_alias(source, declaration, analysis);

    match vue3_resolve_string_type_keys(&declaration.type_annotation, analysis) {
        Some(keys) => {
            if analysis.string_literal_type_declarations.get(&name) != Some(&keys) {
                analysis
                    .string_literal_type_declarations
                    .insert(name.clone(), keys);
                changed = true;
            }
        }
        None => {
            if analysis
                .string_literal_type_declarations
                .remove(&name)
                .is_some()
            {
                changed = true;
            }
        }
    }

    match vue3_resolve_ordered_string_type_keys(&declaration.type_annotation, analysis) {
        Some(keys) => {
            if analysis.ordered_string_literal_type_declarations.get(&name) != Some(&keys) {
                analysis
                    .ordered_string_literal_type_declarations
                    .insert(name.clone(), keys);
                changed = true;
            }
        }
        None => {
            if analysis
                .ordered_string_literal_type_declarations
                .remove(&name)
                .is_some()
            {
                changed = true;
            }
        }
    }

    match infer_vue3_keyof_runtime_type(&declaration.type_annotation, analysis) {
        Some(types) => {
            if analysis.keyof_runtime_type_declarations.get(&name) != Some(&types) {
                analysis
                    .keyof_runtime_type_declarations
                    .insert(name.clone(), types);
                changed = true;
            }
        }
        None => {
            if analysis
                .keyof_runtime_type_declarations
                .remove(&name)
                .is_some()
            {
                changed = true;
            }
        }
    }

    let props_tuple = infer_vue3_tuple_runtime_type(
        &declaration.type_annotation,
        analysis,
        Vue3ArrayElementRuntimeMode::Props,
    );
    changed |= refresh_vue3_runtime_type_tuple_declaration(
        &mut analysis.tuple_runtime_type_declarations,
        &name,
        props_tuple,
    );

    let model_tuple = infer_vue3_tuple_runtime_type(
        &declaration.type_annotation,
        analysis,
        Vue3ArrayElementRuntimeMode::DefineModel,
    );
    changed |= refresh_vue3_runtime_type_tuple_declaration(
        &mut analysis.define_model_tuple_runtime_type_declarations,
        &name,
        model_tuple,
    );

    match infer_vue3_array_element_runtime_type(
        &declaration.type_annotation,
        analysis,
        Vue3ArrayElementRuntimeMode::Props,
    ) {
        Some(types) => {
            if analysis.array_element_runtime_type_declarations.get(&name) != Some(&types) {
                analysis
                    .array_element_runtime_type_declarations
                    .insert(name.clone(), types);
                changed = true;
            }
        }
        None => {
            if analysis
                .array_element_runtime_type_declarations
                .remove(&name)
                .is_some()
            {
                changed = true;
            }
        }
    }

    match infer_vue3_array_element_runtime_type(
        &declaration.type_annotation,
        analysis,
        Vue3ArrayElementRuntimeMode::DefineModel,
    ) {
        Some(types) => {
            if analysis
                .define_model_array_element_runtime_type_declarations
                .get(&name)
                != Some(&types)
            {
                analysis
                    .define_model_array_element_runtime_type_declarations
                    .insert(name.clone(), types);
                changed = true;
            }
        }
        None => {
            if analysis
                .define_model_array_element_runtime_type_declarations
                .remove(&name)
                .is_some()
            {
                changed = true;
            }
        }
    }

    let props_parameter_tuple = infer_vue3_function_parameter_tuple_runtime_type(
        &declaration.type_annotation,
        analysis,
        Vue3ArrayElementRuntimeMode::Props,
    );
    changed |= refresh_vue3_runtime_type_tuple_declaration(
        &mut analysis.parameter_tuple_runtime_type_declarations,
        &name,
        props_parameter_tuple,
    );

    let model_parameter_tuple = infer_vue3_function_parameter_tuple_runtime_type(
        &declaration.type_annotation,
        analysis,
        Vue3ArrayElementRuntimeMode::DefineModel,
    );
    changed |= refresh_vue3_runtime_type_tuple_declaration(
        &mut analysis.define_model_parameter_tuple_runtime_type_declarations,
        &name,
        model_parameter_tuple,
    );

    let props_constructor_parameter_tuple = infer_vue3_constructor_parameter_tuple_runtime_type(
        &declaration.type_annotation,
        analysis,
        Vue3ArrayElementRuntimeMode::Props,
    );
    changed |= refresh_vue3_runtime_type_tuple_declaration(
        &mut analysis.constructor_parameter_tuple_runtime_type_declarations,
        &name,
        props_constructor_parameter_tuple,
    );

    let model_constructor_parameter_tuple = infer_vue3_constructor_parameter_tuple_runtime_type(
        &declaration.type_annotation,
        analysis,
        Vue3ArrayElementRuntimeMode::DefineModel,
    );
    changed |= refresh_vue3_runtime_type_tuple_declaration(
        &mut analysis.define_model_constructor_parameter_tuple_runtime_type_declarations,
        &name,
        model_constructor_parameter_tuple,
    );

    let props_return_type = infer_vue3_return_runtime_type(
        &declaration.type_annotation,
        analysis,
        Vue3ArrayElementRuntimeMode::Props,
    );
    changed |= refresh_vue3_runtime_type_declaration(
        &mut analysis.return_type_runtime_type_declarations,
        &name,
        props_return_type,
    );

    let model_return_type = infer_vue3_return_runtime_type(
        &declaration.type_annotation,
        analysis,
        Vue3ArrayElementRuntimeMode::DefineModel,
    );
    changed |= refresh_vue3_runtime_type_declaration(
        &mut analysis.define_model_return_type_runtime_type_declarations,
        &name,
        model_return_type,
    );

    let runtime = infer_vue3_runtime_type(&declaration.type_annotation, analysis);
    if analysis.declared_types.get(&name) != Some(&runtime) {
        analysis.declared_types.insert(name.clone(), runtime);
        changed = true;
    }

    let model_runtime =
        infer_vue3_define_model_runtime_type(&declaration.type_annotation, analysis);
    if analysis.define_model_declared_types.get(&name) != Some(&model_runtime) {
        analysis
            .define_model_declared_types
            .insert(name.clone(), model_runtime);
        changed = true;
    }

    match vue3_resolve_projectable_props_type(source, &declaration.type_annotation, analysis) {
        Some(props) => {
            if analysis.props_type_declarations.get(&name) != Some(&props) {
                analysis.props_type_declarations.insert(name.clone(), props);
                changed = true;
            }
        }
        None => {
            if analysis.props_type_declarations.remove(&name).is_some() {
                changed = true;
            }
        }
    }

    let emits = vue3_resolve_emits_type(source, &declaration.type_annotation, analysis);
    match emits {
        Some(emits) if !emits.events.is_empty() => {
            if analysis.emits_type_declarations.get(&name) != Some(&emits) {
                analysis.emits_type_declarations.insert(name, emits);
                changed = true;
            }
        }
        _ => {
            if analysis.emits_type_declarations.remove(&name).is_some() {
                changed = true;
            }
        }
    }

    changed
}

pub(crate) fn register_vue3_type_alias_declaration(
    source: &str,
    declaration: &TSTypeAliasDeclaration<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    let name = declaration.id.name.to_string();
    register_vue3_local_type_name(analysis, &name);
    refresh_vue3_type_alias_declaration(source, declaration, analysis);
}

pub(crate) fn refresh_vue3_generic_type_alias(
    source: &str,
    declaration: &TSTypeAliasDeclaration<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) -> bool {
    let Some(type_parameters) = declaration.type_parameters.as_ref() else {
        return analysis
            .generic_type_aliases
            .remove(declaration.id.name.as_str())
            .is_some();
    };
    let params = type_parameters
        .params
        .iter()
        .map(|param| param.name.name.to_string())
        .collect::<Vec<_>>();
    let alias_source = source
        .get(declaration.span.start as usize..declaration.span.end as usize)
        .unwrap_or_default()
        .to_string();
    if params.is_empty() || alias_source.is_empty() {
        return analysis
            .generic_type_aliases
            .remove(declaration.id.name.as_str())
            .is_some();
    }
    let alias = vue3_generic_type_alias(
        alias_source,
        Vue3GenericTypeAliasKind::TypeAlias,
        params,
        analysis,
    );
    if analysis
        .generic_type_aliases
        .get(declaration.id.name.as_str())
        != Some(&alias)
    {
        analysis
            .generic_type_aliases
            .insert(declaration.id.name.to_string(), alias);
        true
    } else {
        false
    }
}

pub(crate) fn vue3_generic_type_alias(
    source: String,
    kind: Vue3GenericTypeAliasKind,
    params: Vec<String>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Vue3GenericTypeAlias {
    Vue3GenericTypeAlias {
        source,
        kind,
        params,
        declared_types: analysis.declared_types.clone(),
        define_model_declared_types: analysis.define_model_declared_types.clone(),
        type_query_declared_types: analysis.type_query_declared_types.clone(),
        define_model_type_query_declared_types: analysis
            .define_model_type_query_declared_types
            .clone(),
        keyof_type_query_declared_types: analysis.keyof_type_query_declared_types.clone(),
        props_type_declarations: analysis.props_type_declarations.clone(),
        keyof_runtime_type_declarations: analysis.keyof_runtime_type_declarations.clone(),
        tuple_runtime_type_declarations: analysis.tuple_runtime_type_declarations.clone(),
        define_model_tuple_runtime_type_declarations: analysis
            .define_model_tuple_runtime_type_declarations
            .clone(),
        array_element_runtime_type_declarations: analysis
            .array_element_runtime_type_declarations
            .clone(),
        define_model_array_element_runtime_type_declarations: analysis
            .define_model_array_element_runtime_type_declarations
            .clone(),
        parameter_tuple_runtime_type_declarations: analysis
            .parameter_tuple_runtime_type_declarations
            .clone(),
        define_model_parameter_tuple_runtime_type_declarations: analysis
            .define_model_parameter_tuple_runtime_type_declarations
            .clone(),
        constructor_parameter_tuple_runtime_type_declarations: analysis
            .constructor_parameter_tuple_runtime_type_declarations
            .clone(),
        define_model_constructor_parameter_tuple_runtime_type_declarations: analysis
            .define_model_constructor_parameter_tuple_runtime_type_declarations
            .clone(),
        return_type_runtime_type_declarations: analysis
            .return_type_runtime_type_declarations
            .clone(),
        define_model_return_type_runtime_type_declarations: analysis
            .define_model_return_type_runtime_type_declarations
            .clone(),
        props_options_type_declarations: analysis.props_options_type_declarations.clone(),
        return_type_props_options_declarations: analysis
            .return_type_props_options_declarations
            .clone(),
        string_literal_type_declarations: analysis.string_literal_type_declarations.clone(),
        ordered_string_literal_type_declarations: analysis
            .ordered_string_literal_type_declarations
            .clone(),
        unresolved_import_sources: analysis.unresolved_import_sources.clone(),
        silent_unresolved_type_names: analysis.silent_unresolved_type_names.clone(),
    }
}

pub(crate) fn register_vue3_ts_enum_declaration(
    declaration: &TSEnumDeclaration<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    let name = declaration.id.name.to_string();
    let merge_existing = analysis.local_ts_enum_type_names.contains(&name);
    register_vue3_local_type_name(analysis, &name);
    let runtime = infer_vue3_enum_runtime_type(declaration);
    let mut merged_runtime = if merge_existing {
        analysis
            .declared_types
            .get(&name)
            .cloned()
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    for runtime_type in &runtime {
        push_unique(&mut merged_runtime, runtime_type);
    }
    analysis
        .declared_types
        .insert(name.clone(), merged_runtime.clone());
    analysis
        .define_model_declared_types
        .insert(name.clone(), merged_runtime);
    analysis.local_ts_enum_type_names.insert(name);
}

pub(crate) fn register_vue3_class_type_name(analysis: &mut Vue3ScriptSetupAnalysis, name: &str) {
    register_vue3_local_type_name(analysis, name);
    analysis
        .declared_types
        .insert(name.to_string(), vec!["Object".into()]);
    analysis
        .define_model_declared_types
        .insert(name.to_string(), vec!["Object".into()]);
}

pub(crate) fn register_vue3_declared_function_return_props_options(
    source: &str,
    function: &Function<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    let Some(id) = &function.id else {
        return;
    };
    register_vue3_function_return_projection(source, id.name.as_str(), function, analysis);
}

pub(crate) fn vue3_function_has_return_projection(function: &Function<'_>) -> bool {
    function.return_type.is_some() || infer_vue3_function_runtime_return_types(function).is_some()
}

pub(crate) fn vue3_function_value_return_type<'a>(
    expression: &'a Expression<'a>,
) -> Option<&'a TSType<'a>> {
    match unwrap_vue3_ts_expression(expression) {
        Expression::ArrowFunctionExpression(function) => function
            .return_type
            .as_ref()
            .map(|return_type| &return_type.type_annotation),
        Expression::FunctionExpression(function) => function
            .return_type
            .as_ref()
            .map(|return_type| &return_type.type_annotation),
        _ => None,
    }
}

pub(crate) fn vue3_function_value_has_return_projection(expression: &Expression<'_>) -> bool {
    match unwrap_vue3_ts_expression(expression) {
        Expression::ArrowFunctionExpression(function) => {
            function.return_type.is_some()
                || infer_vue3_arrow_function_runtime_return_types(function).is_some()
        }
        Expression::FunctionExpression(function) => vue3_function_has_return_projection(function),
        _ => false,
    }
}

pub(crate) fn vue3_default_export_function_value_has_return_projection(
    declaration: &ExportDefaultDeclarationKind<'_>,
) -> bool {
    declaration
        .as_expression()
        .is_some_and(vue3_function_value_has_return_projection)
}

pub(crate) fn vue3_default_export_static_runtime_props_options_is_projectable(
    declaration: &ExportDefaultDeclarationKind<'_>,
) -> bool {
    declaration
        .as_expression()
        .is_some_and(vue3_static_runtime_props_options_is_projectable)
}

pub(crate) fn vue3_variable_declarator_has_function_return_projection(
    declarator: &VariableDeclarator<'_>,
) -> bool {
    vue3_variable_declarator_function_return_type(declarator).is_some()
        || declarator
            .init
            .as_ref()
            .is_some_and(vue3_function_value_has_return_projection)
}

pub(crate) fn vue3_variable_declarator_has_type_projection(
    declarator: &VariableDeclarator<'_>,
) -> bool {
    vue3_variable_declarator_has_function_return_projection(declarator)
        || declarator
            .init
            .as_ref()
            .is_some_and(vue3_static_runtime_props_options_is_projectable)
}

pub(crate) fn vue3_static_runtime_props_options_is_projectable(
    expression: &Expression<'_>,
) -> bool {
    let Some(object) = vue3_static_runtime_props_options_object(expression) else {
        return false;
    };
    vue3_static_runtime_props_options_object_is_projectable(object)
}

pub(crate) fn vue3_static_runtime_props_options_object_is_projectable(
    object: &ObjectExpression<'_>,
) -> bool {
    let mut has_property = false;
    for property in &object.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            return false;
        };
        if property.computed || vue27_property_key_static_name(&property.key).is_none() {
            return false;
        }
        if !vue3_static_runtime_prop_option_is_projectable(&property.value) {
            return false;
        }
        has_property = true;
    }
    has_property
}

pub(crate) fn vue3_static_runtime_prop_option_is_projectable(expression: &Expression<'_>) -> bool {
    if vue3_static_runtime_prop_type_expression_is_projectable(expression) {
        return true;
    }
    let Some(object) = vue3_static_runtime_props_options_object(expression) else {
        return false;
    };
    let mut has_runtime_option_key = false;
    for property in &object.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            return false;
        };
        if property.computed {
            return false;
        }
        let Some(key) = vue27_property_key_static_name(&property.key) else {
            return false;
        };
        match key.as_str() {
            "type" => {
                has_runtime_option_key = true;
            }
            "required" | "default" | "validator" => {
                has_runtime_option_key = true;
            }
            _ => {}
        }
    }
    has_runtime_option_key
}

pub(crate) fn vue3_static_runtime_prop_type_expression_is_projectable(
    expression: &Expression<'_>,
) -> bool {
    match expression {
        Expression::TSAsExpression(expression) => {
            vue3_static_runtime_prop_type_annotation_is_projectable(&expression.type_annotation)
                || vue3_static_runtime_prop_type_expression_is_projectable(&expression.expression)
        }
        Expression::TSTypeAssertion(expression) => {
            vue3_static_runtime_prop_type_annotation_is_projectable(&expression.type_annotation)
                || vue3_static_runtime_prop_type_expression_is_projectable(&expression.expression)
        }
        Expression::TSSatisfiesExpression(expression) => {
            vue3_static_runtime_prop_type_annotation_is_projectable(&expression.type_annotation)
                || vue3_static_runtime_prop_type_expression_is_projectable(&expression.expression)
        }
        Expression::TSNonNullExpression(expression) => {
            vue3_static_runtime_prop_type_expression_is_projectable(&expression.expression)
        }
        Expression::TSInstantiationExpression(expression) => {
            vue3_static_runtime_prop_type_expression_is_projectable(&expression.expression)
        }
        Expression::ParenthesizedExpression(expression) => {
            vue3_static_runtime_prop_type_expression_is_projectable(&expression.expression)
        }
        Expression::Identifier(identifier) => {
            vue3_return_expression_constructor_runtime_name(identifier.name.as_str()).is_some()
        }
        Expression::StaticMemberExpression(member) => {
            vue3_return_expression_constructor_runtime_name(member.property.name.as_str()).is_some()
        }
        Expression::NullLiteral(_) => true,
        Expression::ArrayExpression(array) => array.elements.iter().all(|element| match element {
            ArrayExpressionElement::SpreadElement(_) | ArrayExpressionElement::Elision(_) => false,
            element => element
                .as_expression()
                .is_some_and(vue3_static_runtime_prop_type_expression_is_projectable),
        }),
        _ => false,
    }
}

pub(crate) fn vue3_static_runtime_prop_type_annotation_is_projectable(ty: &TSType<'_>) -> bool {
    match ty {
        TSType::TSTypeReference(reference) => {
            let Some(name) = vue3_ts_type_name_key(&reference.type_name) else {
                return false;
            };
            name == "PropType" || name.ends_with("Constructor")
        }
        TSType::TSImportType(import_type) => import_type.type_arguments.is_some(),
        TSType::TSParenthesizedType(parenthesized) => {
            vue3_static_runtime_prop_type_annotation_is_projectable(&parenthesized.type_annotation)
        }
        _ => false,
    }
}

pub(crate) fn vue3_static_runtime_props_options_object<'a>(
    expression: &'a Expression<'a>,
) -> Option<&'a ObjectExpression<'a>> {
    match unwrap_vue3_ts_expression(expression) {
        Expression::ObjectExpression(object) => Some(object),
        _ => None,
    }
}

pub(crate) fn vue3_variable_declarator_function_return_type<'a>(
    declarator: &'a VariableDeclarator<'a>,
) -> Option<&'a TSType<'a>> {
    if let Some(type_annotation) = declarator.type_annotation.as_ref() {
        if let TSType::TSFunctionType(function) = &type_annotation.type_annotation {
            return Some(&function.return_type.type_annotation);
        }
    }
    declarator
        .init
        .as_ref()
        .and_then(vue3_function_value_return_type)
}

pub(crate) fn infer_vue3_function_runtime_return_types(
    function: &Function<'_>,
) -> Option<Vec<String>> {
    let body = function.body.as_ref()?;
    infer_vue3_function_body_runtime_return_types(body)
}

pub(crate) fn infer_vue3_arrow_function_runtime_return_types(
    function: &ArrowFunctionExpression<'_>,
) -> Option<Vec<String>> {
    if let Some(expression) = function.get_expression() {
        return infer_vue3_return_expression_runtime_types(expression);
    }
    infer_vue3_function_body_runtime_return_types(&function.body)
}

pub(crate) fn infer_vue3_function_body_runtime_return_types(
    body: &FunctionBody<'_>,
) -> Option<Vec<String>> {
    infer_vue3_return_statement_list_runtime_types(&body.statements)
}

pub(crate) fn infer_vue3_return_statement_list_runtime_types(
    statements: &[Statement<'_>],
) -> Option<Vec<String>> {
    let [statement] = statements else {
        return None;
    };
    infer_vue3_return_statement_runtime_types(statement)
}

pub(crate) fn infer_vue3_return_statement_runtime_types(
    statement: &Statement<'_>,
) -> Option<Vec<String>> {
    match statement {
        Statement::ReturnStatement(statement) => statement
            .argument
            .as_ref()
            .and_then(infer_vue3_return_expression_runtime_types),
        Statement::BlockStatement(block) => {
            infer_vue3_return_statement_list_runtime_types(&block.body)
        }
        Statement::IfStatement(statement) => {
            let alternate = statement.alternate.as_ref()?;
            let mut types = infer_vue3_return_statement_runtime_types(&statement.consequent)?;
            let alternate_types = infer_vue3_return_statement_runtime_types(alternate)?;
            merge_vue3_runtime_types(&mut types, alternate_types);
            vue3_non_empty_runtime_types(types)
        }
        _ => None,
    }
}

pub(crate) fn infer_vue3_return_expression_runtime_types(
    expression: &Expression<'_>,
) -> Option<Vec<String>> {
    match unwrap_vue3_ts_expression(expression) {
        Expression::StringLiteral(_) | Expression::TemplateLiteral(_) => {
            Some(vec!["String".into()])
        }
        Expression::NumericLiteral(_) => Some(vec!["Number".into()]),
        Expression::BooleanLiteral(_) => Some(vec!["Boolean".into()]),
        Expression::NullLiteral(_) => Some(vec!["null".into()]),
        Expression::ArrayExpression(_) => Some(vec!["Array".into()]),
        Expression::ObjectExpression(_) => Some(vec!["Object".into()]),
        Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_) => {
            Some(vec!["Function".into()])
        }
        Expression::ConditionalExpression(expression) => {
            let mut types = infer_vue3_return_expression_runtime_types(&expression.consequent)?;
            let alternate_types =
                infer_vue3_return_expression_runtime_types(&expression.alternate)?;
            merge_vue3_runtime_types(&mut types, alternate_types);
            vue3_non_empty_runtime_types(types)
        }
        Expression::NewExpression(expression) => {
            let name = vue3_new_expression_runtime_constructor_name(&expression.callee)?;
            Some(vec![name.to_string()])
        }
        _ => None,
    }
}

pub(crate) fn vue3_new_expression_runtime_constructor_name(
    expression: &Expression<'_>,
) -> Option<&'static str> {
    let name = match unwrap_vue3_ts_expression(expression) {
        Expression::Identifier(identifier) => identifier.name.as_str(),
        Expression::StaticMemberExpression(member) => member.property.name.as_str(),
        _ => return None,
    };
    vue3_return_expression_constructor_runtime_name(name)
}

pub(crate) fn vue3_return_expression_constructor_runtime_name(name: &str) -> Option<&'static str> {
    match name {
        "String" => Some("String"),
        "Number" => Some("Number"),
        "Boolean" => Some("Boolean"),
        "Array" => Some("Array"),
        "Object" => Some("Object"),
        "Function" => Some("Function"),
        "Date" => Some("Date"),
        "Error" => Some("Error"),
        "Map" => Some("Map"),
        "Set" => Some("Set"),
        "WeakMap" => Some("WeakMap"),
        "WeakSet" => Some("WeakSet"),
        "Promise" => Some("Promise"),
        _ => None,
    }
}

pub(crate) fn register_vue3_function_return_projection(
    source: &str,
    name: &str,
    function: &Function<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    if let Some(return_type) = function.return_type.as_ref() {
        register_vue3_declared_return_props_options(
            source,
            name,
            &return_type.type_annotation,
            analysis,
        );
        return;
    }
    if let Some(types) = infer_vue3_function_runtime_return_types(function) {
        register_vue3_declared_return_runtime_types(name, types, analysis);
    }
}

pub(crate) fn register_vue3_function_value_expression_return_projection(
    source: &str,
    name: &str,
    expression: &Expression<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    match unwrap_vue3_ts_expression(expression) {
        Expression::ArrowFunctionExpression(function) => {
            if let Some(return_type) = function.return_type.as_ref() {
                register_vue3_declared_return_props_options(
                    source,
                    name,
                    &return_type.type_annotation,
                    analysis,
                );
                return;
            }
            if let Some(types) = infer_vue3_arrow_function_runtime_return_types(function) {
                register_vue3_declared_return_runtime_types(name, types, analysis);
            }
        }
        Expression::FunctionExpression(function) => {
            register_vue3_function_return_projection(source, name, function, analysis);
        }
        _ => {}
    }
}

pub(crate) fn register_vue3_function_value_return_props_options(
    source: &str,
    declaration: &VariableDeclaration<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    for declarator in &declaration.declarations {
        let Some(name) = first_pattern_binding(&declarator.id) else {
            continue;
        };
        if let Some(return_type) = vue3_variable_declarator_function_return_type(declarator) {
            register_vue3_declared_return_props_options(source, &name, return_type, analysis);
            continue;
        }
        if let Some(init) = declarator.init.as_ref() {
            register_vue3_function_value_expression_return_projection(
                source, &name, init, analysis,
            );
        }
    }
}

pub(crate) fn register_vue3_static_runtime_props_options(
    source: &str,
    declaration: &VariableDeclaration<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    for declarator in &declaration.declarations {
        let Some(name) = first_pattern_binding(&declarator.id) else {
            continue;
        };
        let Some(init) = declarator.init.as_ref() else {
            continue;
        };
        let Some(props_options) =
            vue3_static_runtime_props_options_type_members(source, init, analysis)
        else {
            continue;
        };
        register_vue3_local_type_name(analysis, &name);
        analysis
            .props_options_type_declarations
            .insert(name.to_string(), props_options);
    }
}

pub(crate) fn register_vue3_default_static_runtime_props_options(
    source: &str,
    declaration: &ExportDefaultDeclarationKind<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    let Some(expression) = declaration.as_expression() else {
        return;
    };
    let Some(props_options) =
        vue3_static_runtime_props_options_type_members(source, expression, analysis)
    else {
        return;
    };
    let deps = collect_vue3_static_runtime_props_options_deps(expression, analysis);
    register_vue3_local_type_name(analysis, "default");
    analysis
        .props_options_type_declarations
        .insert("default".into(), props_options);
    insert_vue3_declared_type_deps(analysis, "default", deps);
}

pub(crate) fn collect_vue3_function_value_return_type_deps_from_variable(
    declaration: &VariableDeclaration<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    for declarator in &declaration.declarations {
        let Some(name) = first_pattern_binding(&declarator.id) else {
            continue;
        };
        let Some(return_type) = vue3_variable_declarator_function_return_type(declarator) else {
            continue;
        };
        let deps = collect_vue3_type_argument_deps(return_type, analysis);
        insert_vue3_declared_type_deps(analysis, &name, deps);
    }
}

pub(crate) fn collect_vue3_static_runtime_props_options_deps_from_variable(
    declaration: &VariableDeclaration<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    for declarator in &declaration.declarations {
        let Some(name) = first_pattern_binding(&declarator.id) else {
            continue;
        };
        let Some(init) = declarator.init.as_ref() else {
            continue;
        };
        if !analysis.props_options_type_declarations.contains_key(&name) {
            continue;
        }
        let deps = collect_vue3_static_runtime_props_options_deps(init, analysis);
        insert_vue3_declared_type_deps(analysis, &name, deps);
    }
}

pub(crate) fn collect_vue3_static_runtime_props_options_deps(
    expression: &Expression<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> BTreeSet<String> {
    let mut deps = BTreeSet::new();
    collect_vue3_static_runtime_props_options_deps_into(expression, analysis, &mut deps);
    deps
}

pub(crate) fn collect_vue3_static_runtime_props_options_deps_into(
    expression: &Expression<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    deps: &mut BTreeSet<String>,
) {
    match expression {
        Expression::TSAsExpression(expression) => {
            collect_vue3_type_argument_deps_into(&expression.type_annotation, analysis, deps);
            collect_vue3_static_runtime_props_options_deps_into(
                &expression.expression,
                analysis,
                deps,
            );
        }
        Expression::TSTypeAssertion(expression) => {
            collect_vue3_type_argument_deps_into(&expression.type_annotation, analysis, deps);
            collect_vue3_static_runtime_props_options_deps_into(
                &expression.expression,
                analysis,
                deps,
            );
        }
        Expression::TSSatisfiesExpression(expression) => {
            collect_vue3_type_argument_deps_into(&expression.type_annotation, analysis, deps);
            collect_vue3_static_runtime_props_options_deps_into(
                &expression.expression,
                analysis,
                deps,
            );
        }
        Expression::TSInstantiationExpression(expression) => {
            for ty in &expression.type_arguments.params {
                collect_vue3_type_argument_deps_into(ty, analysis, deps);
            }
            collect_vue3_static_runtime_props_options_deps_into(
                &expression.expression,
                analysis,
                deps,
            );
        }
        Expression::TSNonNullExpression(expression) => {
            collect_vue3_static_runtime_props_options_deps_into(
                &expression.expression,
                analysis,
                deps,
            );
        }
        Expression::ParenthesizedExpression(expression) => {
            collect_vue3_static_runtime_props_options_deps_into(
                &expression.expression,
                analysis,
                deps,
            );
        }
        Expression::ArrayExpression(array) => {
            for element in &array.elements {
                let Some(expression) = element.as_expression() else {
                    continue;
                };
                collect_vue3_static_runtime_props_options_deps_into(expression, analysis, deps);
            }
        }
        Expression::ObjectExpression(object) => {
            for property in &object.properties {
                let ObjectPropertyKind::ObjectProperty(property) = property else {
                    continue;
                };
                collect_vue3_static_runtime_props_options_deps_into(
                    &property.value,
                    analysis,
                    deps,
                );
            }
        }
        _ => {}
    }
}

pub(crate) fn register_vue3_declared_variable_props_options(
    source: &str,
    declaration: &VariableDeclaration<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    for declarator in &declaration.declarations {
        let Some(name) = first_pattern_binding(&declarator.id) else {
            continue;
        };
        let Some(type_annotation) = declarator.type_annotation.as_ref() else {
            continue;
        };
        register_vue3_local_type_name(analysis, &name);
        register_vue3_declared_type_query_runtime_types(
            &name,
            &type_annotation.type_annotation,
            analysis,
        );
        register_vue3_declared_callable_return_runtime_types(
            &name,
            &type_annotation.type_annotation,
            analysis,
        );
        match &type_annotation.type_annotation {
            TSType::TSTypeLiteral(_) => {
                if let Some(props_options) = vue3_props_options_type_members(
                    source,
                    &type_annotation.type_annotation,
                    analysis,
                ) {
                    analysis
                        .props_options_type_declarations
                        .insert(name.to_string(), props_options);
                }
            }
            TSType::TSFunctionType(function) => {
                if let Some(props_options) = vue3_props_options_type_members(
                    source,
                    &function.return_type.type_annotation,
                    analysis,
                ) {
                    analysis
                        .return_type_props_options_declarations
                        .insert(name.to_string(), props_options);
                }
            }
            _ => {}
        }
    }
}

pub(crate) fn register_vue3_declared_type_query_runtime_types(
    name: &str,
    ty: &TSType<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    analysis
        .type_query_declared_types
        .insert(name.to_string(), infer_vue3_runtime_type(ty, analysis));
    analysis.define_model_type_query_declared_types.insert(
        name.to_string(),
        infer_vue3_define_model_runtime_type(ty, analysis),
    );
    if let Some(types) = infer_vue3_keyof_runtime_type(ty, analysis) {
        analysis
            .keyof_type_query_declared_types
            .insert(name.to_string(), types);
    }
}

pub(crate) fn register_vue3_declared_return_props_options(
    source: &str,
    name: &str,
    ty: &TSType<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    register_vue3_local_type_name(analysis, name);
    register_vue3_declared_return_annotation_runtime_types(name, ty, analysis);
    if let Some(props_options) = vue3_props_options_type_members(source, ty, analysis) {
        analysis
            .return_type_props_options_declarations
            .insert(name.to_string(), props_options);
    }
}

pub(crate) fn register_vue3_declared_return_annotation_runtime_types(
    name: &str,
    ty: &TSType<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    if let Some(types) = vue3_non_empty_runtime_types(infer_vue3_runtime_type(ty, analysis)) {
        analysis
            .return_type_runtime_type_declarations
            .insert(name.to_string(), types);
    }
    if let Some(types) =
        vue3_non_empty_runtime_types(infer_vue3_define_model_runtime_type(ty, analysis))
    {
        analysis
            .define_model_return_type_runtime_type_declarations
            .insert(name.to_string(), types);
    }
}

pub(crate) fn register_vue3_declared_return_runtime_types(
    name: &str,
    types: Vec<String>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    register_vue3_local_type_name(analysis, name);
    let Some(types) = vue3_non_empty_runtime_types(types) else {
        return;
    };
    analysis
        .return_type_runtime_type_declarations
        .insert(name.to_string(), types.clone());
    analysis
        .define_model_return_type_runtime_type_declarations
        .insert(name.to_string(), types);
}

pub(crate) fn register_vue3_declared_callable_return_runtime_types(
    name: &str,
    ty: &TSType<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    if let Some(types) =
        infer_vue3_return_runtime_type(ty, analysis, Vue3ArrayElementRuntimeMode::Props)
    {
        analysis
            .return_type_runtime_type_declarations
            .insert(name.to_string(), types);
    }
    if let Some(types) =
        infer_vue3_return_runtime_type(ty, analysis, Vue3ArrayElementRuntimeMode::DefineModel)
    {
        analysis
            .define_model_return_type_runtime_type_declarations
            .insert(name.to_string(), types);
    }
}

pub(crate) fn infer_vue3_enum_runtime_type(declaration: &TSEnumDeclaration<'_>) -> Vec<String> {
    let mut types = Vec::new();
    for member in &declaration.body.members {
        match member.initializer.as_ref() {
            Some(Expression::StringLiteral(_)) => push_unique(&mut types, "String"),
            Some(Expression::NumericLiteral(_)) => push_unique(&mut types, "Number"),
            _ => {}
        }
    }
    if types.is_empty() {
        vec!["Number".into()]
    } else {
        types
    }
}

pub(crate) fn register_vue3_local_type_name(analysis: &mut Vue3ScriptSetupAnalysis, name: &str) {
    analysis.type_sources.remove(name);
    analysis.type_direct_deps.remove(name);
    analysis.type_deps.remove(name);
    analysis.unresolved_import_sources.remove(name);
    analysis.silent_unresolved_type_names.remove(name);
    analysis.type_query_declared_types.remove(name);
    analysis.define_model_type_query_declared_types.remove(name);
    analysis.keyof_type_query_declared_types.remove(name);
    analysis.props_options_type_declarations.remove(name);
    analysis.return_type_runtime_type_declarations.remove(name);
    analysis
        .define_model_return_type_runtime_type_declarations
        .remove(name);
    analysis.return_type_props_options_declarations.remove(name);
    analysis.generic_type_aliases.remove(name);
    analysis.string_literal_type_declarations.remove(name);
    analysis
        .ordered_string_literal_type_declarations
        .remove(name);
    analysis.local_ts_enum_type_names.remove(name);
}

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
        if let Some(init) = &declarator.init {
            if let Expression::CallExpression(call) = init {
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

pub(crate) fn collect_define_props_call(
    source: &str,
    call: &oxc_ast::ast::CallExpression<'_>,
    binding: Option<&BindingPattern<'_>>,
    analysis: &mut Vue27ScriptSetupAnalysis,
    is_prod: bool,
) {
    if let Some(type_argument) = call
        .type_arguments
        .as_ref()
        .and_then(|arguments| arguments.params.first())
    {
        if !call.arguments.is_empty() {
            analysis
                .errors
                .push(vue27_macro_type_and_runtime_error("defineProps"));
        }
        collect_define_props_type(source, type_argument, binding, None, analysis, is_prod);
        return;
    }
    if let Some(argument) = call.arguments.first() {
        let expression = argument.to_expression();
        check_vue27_invalid_scope_reference(expression, "defineProps", analysis);
        if let Expression::ObjectExpression(object) = expression {
            for key in object_expression_keys(object) {
                push_unique(&mut analysis.props_bindings, &key);
            }
        }
        let start = expression.span().start as usize;
        let end = expression.span().end as usize;
        analysis.props_runtime = source.get(start..end).map(ToOwned::to_owned);
    }
}

pub(crate) fn collect_with_defaults_call(
    source: &str,
    call: &oxc_ast::ast::CallExpression<'_>,
    binding: Option<&BindingPattern<'_>>,
    analysis: &mut Vue27ScriptSetupAnalysis,
    is_prod: bool,
) -> bool {
    let Some(define_props_call) =
        call.arguments
            .first()
            .and_then(|argument| match argument.to_expression() {
                Expression::CallExpression(call) if is_call_named(call, "defineProps") => {
                    Some(call)
                }
                _ => None,
            })
    else {
        return false;
    };
    let Some(type_argument) = define_props_call
        .type_arguments
        .as_ref()
        .and_then(|arguments| arguments.params.first())
    else {
        collect_define_props_call(source, define_props_call, binding, analysis, is_prod);
        return true;
    };
    let defaults = call.arguments.get(1).map(|argument| {
        check_vue27_invalid_scope_reference(argument.to_expression(), "defineProps", analysis);
        vue27_runtime_defaults_from_argument(source, argument)
    });
    collect_define_props_type(
        source,
        type_argument,
        binding,
        defaults.flatten(),
        analysis,
        is_prod,
    );
    true
}

pub(crate) fn collect_define_emits_call(
    source: &str,
    call: &oxc_ast::ast::CallExpression<'_>,
    binding: Option<&str>,
    analysis: &mut Vue27ScriptSetupAnalysis,
) {
    if analysis.emit_binding.is_none() {
        if let Some(binding) = binding {
            analysis.emit_binding = Some(binding.to_string());
        }
    }
    if let Some(type_argument) = call
        .type_arguments
        .as_ref()
        .and_then(|arguments| arguments.params.first())
    {
        if !call.arguments.is_empty() {
            analysis
                .errors
                .push(vue27_macro_type_and_runtime_error("defineEmits"));
        }
        collect_define_emits_type(source, type_argument, analysis);
        return;
    }
    let Some(argument) = call.arguments.first() else {
        return;
    };
    let expression = argument.to_expression();
    check_vue27_invalid_scope_reference(expression, "defineEmits", analysis);
    let start = expression.span().start as usize;
    let end = expression.span().end as usize;
    analysis.emits_runtime = source.get(start..end).map(ToOwned::to_owned);
}

pub(crate) fn collect_define_props_type(
    source: &str,
    type_argument: &TSType<'_>,
    binding: Option<&BindingPattern<'_>>,
    defaults: Option<Vue27RuntimeDefaults>,
    analysis: &mut Vue27ScriptSetupAnalysis,
    is_prod: bool,
) {
    let Some(type_members) = vue27_resolve_props_type(source, type_argument, analysis) else {
        return;
    };
    let default_map = defaults
        .as_ref()
        .and_then(|defaults| defaults.static_defaults.as_ref());
    let mut props = Vec::new();
    for member in &type_members.members {
        let mut prop = member.clone();
        if let Some(default) = default_map.and_then(|defaults| defaults.get(&prop.key)) {
            prop.default = Some(default.clone());
        }
        push_unique(&mut analysis.props_bindings, &prop.key);
        props.push(prop);
    }
    analysis.props_runtime_defaults = defaults;
    analysis.needs_merge_defaults = analysis
        .props_runtime_defaults
        .as_ref()
        .is_some_and(|defaults| defaults.static_defaults.is_none());
    analysis.props_type_runtime = true;
    analysis.props_type_source = Some(vue27_setup_props_type_source(
        source,
        type_argument,
        &type_members,
        analysis.props_runtime_defaults.as_ref(),
    ));
    analysis.props_runtime = Some(gen_vue27_runtime_props(
        &props,
        analysis.props_runtime_defaults.as_ref(),
        is_prod,
    ));
    if let Some(binding) = binding {
        analysis
            .setup_prelude
            .push_str(&vue27_props_type_assignment(
                source,
                binding,
                analysis.props_type_source.as_deref(),
            ));
    }
}

pub(crate) fn collect_define_emits_type(
    source: &str,
    type_argument: &TSType<'_>,
    analysis: &mut Vue27ScriptSetupAnalysis,
) {
    if !vue27_emits_type_argument_is_supported(type_argument, analysis) {
        analysis.errors.push(
            "type argument passed to defineEmits() must be a function type, a literal type with call signatures, or a reference to the above types."
                .to_string(),
        );
        return;
    }
    let Some(emits_type) = vue27_resolve_emits_type(source, type_argument, analysis) else {
        return;
    };
    if !emits_type.events.is_empty() {
        analysis.emits_runtime = Some(format!(
            "[{}]",
            emits_type
                .events
                .iter()
                .map(|name| format!("\"{}\"", escape_js_double(name)))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    analysis.emit_type_source = Some(emits_type.source);
}

pub(crate) fn vue27_script_setup_module_export_error() -> String {
    "<script setup> cannot contain ES module exports. If you are using a previous version of <script setup>, please consult the updated RFC at https://github.com/vuejs/rfcs/pull/227.".to_string()
}

pub(crate) fn vue27_macro_type_and_runtime_error(macro_name: &str) -> String {
    format!(
        "{macro_name}() cannot accept both type and non-type arguments at the same time. Use one or the other."
    )
}

pub(crate) fn check_vue27_invalid_scope_reference(
    expression: &Expression<'_>,
    macro_name: &str,
    analysis: &mut Vue27ScriptSetupAnalysis,
) {
    if vue27_expression_references_setup_local(expression, &analysis.local_setup_bindings) {
        analysis.errors.push(format!(
            "`{macro_name}()` in <script setup> cannot reference locally declared variables because it will be hoisted outside of the setup() function. If your component options require initialization in the module scope, use a separate normal <script> to export the options instead."
        ));
    }
}

pub(crate) fn vue27_emits_type_argument_is_supported(
    type_argument: &TSType<'_>,
    analysis: &Vue27ScriptSetupAnalysis,
) -> bool {
    match type_argument {
        TSType::TSFunctionType(_) | TSType::TSTypeLiteral(_) => true,
        TSType::TSTypeReference(reference) => vue27_ts_type_name_identifier(&reference.type_name)
            .is_some_and(|name| analysis.emits_type_declarations.contains_key(name)),
        _ => false,
    }
}

pub(crate) fn vue27_resolve_props_type<'a>(
    source: &str,
    type_argument: &'a TSType<'a>,
    analysis: &Vue27ScriptSetupAnalysis,
) -> Option<Vue27TypeMembers> {
    match type_argument {
        TSType::TSTypeLiteral(literal) => {
            Some(vue27_type_members_from_literal(source, literal, analysis))
        }
        TSType::TSTypeReference(reference) => {
            let name = vue27_ts_type_name_identifier(&reference.type_name)?;
            analysis.props_type_declarations.get(name).cloned()
        }
        _ => None,
    }
}

pub(crate) fn vue27_resolve_emits_type<'a>(
    source: &str,
    type_argument: &'a TSType<'a>,
    analysis: &Vue27ScriptSetupAnalysis,
) -> Option<Vue27EmitsType> {
    match type_argument {
        TSType::TSFunctionType(function) => Some(vue27_emits_type_from_function(source, function)),
        TSType::TSTypeLiteral(literal) => Some(vue27_emits_type_from_literal(source, literal)),
        TSType::TSTypeReference(reference) => {
            let name = vue27_ts_type_name_identifier(&reference.type_name)?;
            analysis.emits_type_declarations.get(name).cloned()
        }
        _ => None,
    }
}

pub(crate) fn vue27_type_members_from_literal(
    source: &str,
    literal: &TSTypeLiteral<'_>,
    analysis: &Vue27ScriptSetupAnalysis,
) -> Vue27TypeMembers {
    Vue27TypeMembers {
        source: source
            .get(literal.span.start as usize..literal.span.end as usize)
            .unwrap_or_default()
            .to_string(),
        members: vue27_runtime_props_from_signatures(source, &literal.members, analysis),
        errors: Vec::new(),
    }
}

pub(crate) fn vue27_type_members_from_interface_body(
    source: &str,
    body: &TSInterfaceBody<'_>,
    analysis: &Vue27ScriptSetupAnalysis,
) -> Vue27TypeMembers {
    Vue27TypeMembers {
        source: source
            .get(body.span.start as usize..body.span.end as usize)
            .unwrap_or_default()
            .to_string(),
        members: vue27_runtime_props_from_signatures(source, &body.body, analysis),
        errors: Vec::new(),
    }
}

pub(crate) fn vue27_runtime_props_from_signatures(
    source: &str,
    signatures: &[TSSignature<'_>],
    analysis: &Vue27ScriptSetupAnalysis,
) -> Vec<Vue27RuntimeProp> {
    let mut props = Vec::new();
    for signature in signatures {
        match signature {
            TSSignature::TSPropertySignature(property) if !property.computed => {
                if let Some(key) = vue27_property_key_static_name(&property.key) {
                    let types = property
                        .type_annotation
                        .as_ref()
                        .map(|annotation| {
                            infer_vue27_runtime_type(&annotation.type_annotation, analysis)
                        })
                        .unwrap_or_else(|| vec!["null".into()]);
                    props.push(Vue27RuntimeProp {
                        key,
                        types,
                        required: !property.optional,
                        default: None,
                        is_method: false,
                        type_annotation_source: property.type_annotation.as_ref().and_then(
                            |annotation| {
                                source
                                    .get(
                                        annotation.span.start as usize
                                            ..annotation.span.end as usize,
                                    )
                                    .map(ToOwned::to_owned)
                            },
                        ),
                        member_source: source
                            .get(property.span.start as usize..property.span.end as usize)
                            .map(ToOwned::to_owned),
                    });
                }
            }
            TSSignature::TSMethodSignature(method) if !method.computed => {
                if let Some(key) = vue27_property_key_static_name(&method.key) {
                    props.push(Vue27RuntimeProp {
                        key,
                        types: vec!["Function".into()],
                        required: !method.optional,
                        default: None,
                        is_method: true,
                        type_annotation_source: method.return_type.as_ref().and_then(
                            |annotation| {
                                source
                                    .get(
                                        annotation.span.start as usize
                                            ..annotation.span.end as usize,
                                    )
                                    .map(ToOwned::to_owned)
                            },
                        ),
                        member_source: source
                            .get(method.span.start as usize..method.span.end as usize)
                            .map(ToOwned::to_owned),
                    });
                }
            }
            _ => {}
        }
    }
    props
}

pub(crate) fn infer_vue27_runtime_type(
    node: &TSType<'_>,
    analysis: &Vue27ScriptSetupAnalysis,
) -> Vec<String> {
    match node {
        TSType::TSStringKeyword(_) => vec!["String".into()],
        TSType::TSNumberKeyword(_) => vec!["Number".into()],
        TSType::TSBooleanKeyword(_) => vec!["Boolean".into()],
        TSType::TSObjectKeyword(_) | TSType::TSTypeLiteral(_) | TSType::TSIntersectionType(_) => {
            vec!["Object".into()]
        }
        TSType::TSFunctionType(_) => vec!["Function".into()],
        TSType::TSArrayType(_) | TSType::TSTupleType(_) => vec!["Array".into()],
        TSType::TSSymbolKeyword(_) => vec!["Symbol".into()],
        TSType::TSLiteralType(literal) => match &literal.literal {
            TSLiteral::StringLiteral(_) => vec!["String".into()],
            TSLiteral::BooleanLiteral(_) => vec!["Boolean".into()],
            TSLiteral::NumericLiteral(_) | TSLiteral::BigIntLiteral(_) => vec!["Number".into()],
            _ => vec!["null".into()],
        },
        TSType::TSTypeReference(reference) => {
            if let Some(name) = vue27_ts_type_name_identifier(&reference.type_name) {
                if let Some(types) = analysis.declared_types.get(name) {
                    return types.clone();
                }
                match name {
                    "Array" | "Function" | "Object" | "Set" | "Map" | "WeakSet" | "WeakMap"
                    | "Date" | "Promise" => return vec![name.to_string()],
                    "Record" | "Partial" | "Readonly" | "Pick" | "Omit" | "Exclude" | "Extract"
                    | "Required" | "InstanceType" => return vec!["Object".into()],
                    _ => {}
                }
            }
            vec!["null".into()]
        }
        TSType::TSParenthesizedType(parenthesized) => {
            infer_vue27_runtime_type(&parenthesized.type_annotation, analysis)
        }
        TSType::TSUnionType(union) => {
            let mut types = Vec::new();
            for ty in &union.types {
                for runtime_type in infer_vue27_runtime_type(ty, analysis) {
                    push_unique(&mut types, &runtime_type);
                }
            }
            types
        }
        _ => vec!["null".into()],
    }
}

pub(crate) fn vue27_runtime_defaults_from_argument(
    source: &str,
    argument: &Argument<'_>,
) -> Option<Vue27RuntimeDefaults> {
    let expression = argument.to_expression();
    let source_text = source
        .get(expression.span().start as usize..expression.span().end as usize)?
        .to_string();
    let Expression::ObjectExpression(object) = expression else {
        return Some(Vue27RuntimeDefaults {
            source: source_text,
            static_defaults: None,
        });
    };
    let mut defaults = BTreeMap::new();
    for property in &object.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            return Some(Vue27RuntimeDefaults {
                source: source_text,
                static_defaults: None,
            });
        };
        if property.computed {
            return Some(Vue27RuntimeDefaults {
                source: source_text,
                static_defaults: None,
            });
        }
        if let Some(key) = vue27_property_key_static_name(&property.key) {
            let default_source = if property.method {
                vue27_function_body_source(source, &property.value)
                    .map(|body| format!("default() {body}"))
            } else {
                source
                    .get(property.value.span().start as usize..property.value.span().end as usize)
                    .map(|value| format!("default: {value}"))
            };
            if let Some(default_source) = default_source {
                defaults.insert(key, default_source);
            }
        }
    }
    Some(Vue27RuntimeDefaults {
        source: source_text,
        static_defaults: Some(defaults),
    })
}

pub(crate) fn vue27_function_body_source<'a>(
    source: &'a str,
    expression: &Expression<'_>,
) -> Option<&'a str> {
    match expression {
        Expression::FunctionExpression(function) => function
            .body
            .as_ref()
            .and_then(|body| source.get(body.span.start as usize..body.span.end as usize)),
        _ => source.get(expression.span().start as usize..expression.span().end as usize),
    }
}

pub(crate) fn vue3_runtime_defaults_from_argument(
    source: &str,
    argument: &Argument<'_>,
) -> Option<Vue27RuntimeDefaults> {
    let expression = argument.to_expression();
    let source_text = source
        .get(expression.span().start as usize..expression.span().end as usize)?
        .to_string();
    let Expression::ObjectExpression(object) = expression else {
        return Some(Vue27RuntimeDefaults {
            source: source_text,
            static_defaults: None,
        });
    };
    let mut defaults = BTreeMap::new();
    for property in &object.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            return Some(Vue27RuntimeDefaults {
                source: source_text,
                static_defaults: None,
            });
        };
        let Some(key) = vue3_runtime_default_property_key(&property.key, property.computed) else {
            return Some(Vue27RuntimeDefaults {
                source: source_text,
                static_defaults: None,
            });
        };
        let default_source = if property.method || property.kind != PropertyKind::Init {
            vue3_runtime_default_method_source(source, property)
        } else {
            source
                .get(property.value.span().start as usize..property.value.span().end as usize)
                .map(|value| format!("default: {value}"))
        };
        if let Some(default_source) = default_source {
            defaults.insert(key, default_source);
        }
    }
    Some(Vue27RuntimeDefaults {
        source: source_text,
        static_defaults: Some(defaults),
    })
}

pub(crate) fn vue3_runtime_default_property_key(
    key: &PropertyKey<'_>,
    computed: bool,
) -> Option<String> {
    if !computed {
        return vue27_property_key_static_name(key);
    }
    match key {
        PropertyKey::StringLiteral(literal) => Some(literal.value.to_string()),
        PropertyKey::NumericLiteral(literal) => Some(literal.value.to_string()),
        PropertyKey::TemplateLiteral(template) if template.expressions.is_empty() => {
            let mut key = String::new();
            for quasi in &template.quasis {
                key.push_str(&vue3_template_value(quasi));
            }
            Some(key)
        }
        _ => None,
    }
}

pub(crate) fn vue3_runtime_default_method_source(
    source: &str,
    property: &ObjectProperty<'_>,
) -> Option<String> {
    let Expression::FunctionExpression(function) = &property.value else {
        return source
            .get(property.value.span().start as usize..property.value.span().end as usize)
            .map(|value| format!("default: {value}"));
    };
    let body = function
        .body
        .as_ref()
        .and_then(|body| source.get(body.span.start as usize..body.span.end as usize))?;
    match property.kind {
        PropertyKind::Get => Some(format!("get default() {body}")),
        PropertyKind::Set => {
            let params = vue3_function_params_source(source, function)?;
            Some(format!("set default{params} {body}"))
        }
        PropertyKind::Init => {
            let params = vue3_function_params_source(source, function)?;
            let async_prefix = if function.r#async { "async " } else { "" };
            let generator_prefix = if function.generator { "*" } else { "" };
            Some(format!(
                "{async_prefix}{generator_prefix}default{params} {body}"
            ))
        }
    }
}

pub(crate) fn vue3_function_params_source<'a>(
    source: &'a str,
    function: &Function<'_>,
) -> Option<&'a str> {
    source.get(function.params.span.start as usize..function.params.span.end as usize)
}

pub(crate) fn vue27_setup_props_type_source(
    source: &str,
    type_argument: &TSType<'_>,
    type_members: &Vue27TypeMembers,
    defaults: Option<&Vue27RuntimeDefaults>,
) -> String {
    let Some(defaults) = defaults.and_then(|defaults| defaults.static_defaults.as_ref()) else {
        if !type_members.source.is_empty() {
            return type_members.source.clone();
        }
        return source
            .get(type_argument.span().start as usize..type_argument.span().end as usize)
            .unwrap_or_default()
            .to_string();
    };
    let mut parts = Vec::new();
    for prop in &type_members.members {
        if defaults.contains_key(&prop.key) {
            if let Some(type_annotation) = &prop.type_annotation_source {
                parts.push(format!(
                    "{}{}{}",
                    prop.key,
                    if prop.is_method { "()" } else { "" },
                    type_annotation
                ));
            }
        } else if let Some(member_source) = vue27_prop_member_type_source(prop) {
            parts.push(member_source);
        }
    }
    format!("{{ {} }}", parts.join(", "))
}

pub(crate) fn vue27_prop_member_type_source(prop: &Vue27RuntimeProp) -> Option<String> {
    let member_source = prop.member_source.as_deref()?;
    let type_annotation = prop.type_annotation_source.as_deref()?;
    let end = member_source.find(type_annotation)? + type_annotation.len();
    Some(member_source[..end].trim().to_string())
}

pub(crate) fn gen_vue27_runtime_props(
    props: &[Vue27RuntimeProp],
    defaults: Option<&Vue27RuntimeDefaults>,
    is_prod: bool,
) -> String {
    let mut entries = Vec::new();
    for prop in props {
        let type_string = vue27_runtime_type_string(&prop.types);
        if !is_prod {
            entries.push(format!(
                "{}: {{ type: {}, required: {}{} }}",
                prop.key,
                type_string,
                prop.required,
                prop.default
                    .as_ref()
                    .map(|default| format!(", {default}"))
                    .unwrap_or_default()
            ));
        } else if prop
            .types
            .iter()
            .any(|ty| ty == "Boolean" || (prop.default.is_some() && ty == "Function"))
        {
            entries.push(format!(
                "{}: {{ type: {}{} }}",
                prop.key,
                type_string,
                prop.default
                    .as_ref()
                    .map(|default| format!(", {default}"))
                    .unwrap_or_default()
            ));
        } else {
            entries.push(format!(
                "{}: {}",
                prop.key,
                prop.default
                    .as_ref()
                    .map(|default| format!("{{ {default} }}"))
                    .unwrap_or_else(|| "null".into())
            ));
        }
    }
    let props_decl = format!("{{\n    {}\n  }}", entries.join(",\n    "));
    if let Some(defaults) = defaults {
        if defaults.static_defaults.is_none() {
            return format!("_mergeDefaults({props_decl}, {})", defaults.source);
        }
    }
    props_decl
}

pub(crate) fn vue27_runtime_type_string(types: &[String]) -> String {
    if types.len() > 1 {
        format!("[{}]", types.join(", "))
    } else {
        types.first().cloned().unwrap_or_else(|| "null".into())
    }
}

pub(crate) fn vue27_props_type_assignment(
    source: &str,
    binding: &BindingPattern<'_>,
    type_source: Option<&str>,
) -> String {
    let binding_source = source
        .get(binding.span().start as usize..binding.span().end as usize)
        .unwrap_or("props")
        .trim();
    let cast = type_source
        .filter(|value| !value.is_empty())
        .map(|value| format!(" as {value}"))
        .unwrap_or_default();
    format!("\nconst {binding_source} = __props{cast};\n")
}

pub(crate) fn vue27_emits_type_from_function(
    source: &str,
    function: &TSFunctionType<'_>,
) -> Vue27EmitsType {
    let mut events = Vec::new();
    collect_vue27_emits_from_parameters(&function.params.items, &mut events);
    Vue27EmitsType {
        source: source
            .get(function.span.start as usize..function.span.end as usize)
            .unwrap_or_default()
            .to_string(),
        events,
        syntax: Vue3EmitsTypeSyntax {
            has_call_signature: true,
            has_property: false,
        },
        call_count: 1,
    }
}

pub(crate) fn vue27_emits_type_from_literal(
    source: &str,
    literal: &TSTypeLiteral<'_>,
) -> Vue27EmitsType {
    let mut events = Vec::new();
    let mut syntax = Vue3EmitsTypeSyntax::default();
    let mut call_count = 0usize;
    for member in &literal.members {
        if let TSSignature::TSCallSignatureDeclaration(signature) = member {
            syntax.has_call_signature = true;
            call_count += 1;
            collect_vue27_emits_from_parameters(&signature.params.items, &mut events);
        }
    }
    Vue27EmitsType {
        source: source
            .get(literal.span.start as usize..literal.span.end as usize)
            .unwrap_or_default()
            .to_string(),
        events,
        syntax,
        call_count,
    }
}

pub(crate) fn vue27_emits_type_from_interface_body(
    source: &str,
    body: &TSInterfaceBody<'_>,
) -> Vue27EmitsType {
    let mut events = Vec::new();
    let mut syntax = Vue3EmitsTypeSyntax::default();
    let mut call_count = 0usize;
    for member in &body.body {
        if let TSSignature::TSCallSignatureDeclaration(signature) = member {
            syntax.has_call_signature = true;
            call_count += 1;
            collect_vue27_emits_from_parameters(&signature.params.items, &mut events);
        }
    }
    Vue27EmitsType {
        source: source
            .get(body.span.start as usize..body.span.end as usize)
            .unwrap_or_default()
            .to_string(),
        events,
        syntax,
        call_count,
    }
}

pub(crate) fn vue3_emits_type_from_function(
    source: &str,
    function: &TSFunctionType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Vue27EmitsType {
    let mut events = Vec::new();
    collect_vue3_emits_from_parameters(&function.params.items, &mut events, analysis);
    Vue27EmitsType {
        source: source
            .get(function.span.start as usize..function.span.end as usize)
            .unwrap_or_default()
            .to_string(),
        events,
        syntax: Vue3EmitsTypeSyntax {
            has_call_signature: true,
            has_property: false,
        },
        call_count: 1,
    }
}

pub(crate) fn vue3_emits_type_from_literal(
    source: &str,
    literal: &TSTypeLiteral<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Vue27EmitsType {
    let mut events = Vec::new();
    let mut syntax = Vue3EmitsTypeSyntax::default();
    let mut call_count = 0usize;
    for member in &literal.members {
        collect_vue3_emits_type_member(
            source,
            member,
            &mut events,
            &mut syntax,
            &mut call_count,
            analysis,
        );
    }
    Vue27EmitsType {
        source: source
            .get(literal.span.start as usize..literal.span.end as usize)
            .unwrap_or_default()
            .to_string(),
        events,
        syntax,
        call_count,
    }
}

pub(crate) fn vue3_emits_type_from_interface_body(
    source: &str,
    body: &TSInterfaceBody<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Vue27EmitsType {
    let mut events = Vec::new();
    let mut syntax = Vue3EmitsTypeSyntax::default();
    let mut call_count = 0usize;
    for member in &body.body {
        collect_vue3_emits_type_member(
            source,
            member,
            &mut events,
            &mut syntax,
            &mut call_count,
            analysis,
        );
    }
    Vue27EmitsType {
        source: source
            .get(body.span.start as usize..body.span.end as usize)
            .unwrap_or_default()
            .to_string(),
        events,
        syntax,
        call_count,
    }
}

pub(crate) fn collect_vue3_emits_type_member(
    _source: &str,
    member: &TSSignature<'_>,
    events: &mut Vec<String>,
    syntax: &mut Vue3EmitsTypeSyntax,
    call_count: &mut usize,
    analysis: &Vue3ScriptSetupAnalysis,
) {
    match member {
        TSSignature::TSCallSignatureDeclaration(signature) => {
            syntax.has_call_signature = true;
            *call_count += 1;
            collect_vue3_emits_from_parameters(&signature.params.items, events, analysis);
        }
        TSSignature::TSPropertySignature(property) if !property.computed => {
            if let Some(key) = vue27_property_key_static_name(&property.key) {
                syntax.has_property = true;
                push_unique(events, &key);
            }
        }
        _ => {}
    }
}

pub(crate) fn collect_vue3_emits_from_parameters(
    parameters: &[FormalParameter<'_>],
    names: &mut Vec<String>,
    analysis: &Vue3ScriptSetupAnalysis,
) {
    let Some(parameter) = parameters.first() else {
        return;
    };
    let Some(annotation) = parameter.type_annotation.as_ref() else {
        return;
    };
    collect_vue3_emits_from_type(&annotation.type_annotation, names, analysis);
}

pub(crate) fn collect_vue3_emits_from_type(
    ty: &TSType<'_>,
    names: &mut Vec<String>,
    analysis: &Vue3ScriptSetupAnalysis,
) {
    match ty {
        TSType::TSLiteralType(literal) => {
            if let Some(name) = vue3_literal_type_key(&literal.literal) {
                push_unique(names, &name);
            }
        }
        TSType::TSUnionType(union) => {
            for ty in &union.types {
                collect_vue3_emits_from_type(ty, names, analysis);
            }
        }
        TSType::TSParenthesizedType(parenthesized) => {
            collect_vue3_emits_from_type(&parenthesized.type_annotation, names, analysis);
        }
        TSType::TSTypeReference(reference) => {
            if let Some(name) = vue3_ts_type_name_key(&reference.type_name) {
                if let Some(keys) = analysis.ordered_string_literal_type_declarations.get(&name) {
                    for key in keys {
                        push_unique(names, key);
                    }
                }
            }
        }
        TSType::TSImportType(import_type) => {
            if let Some(resolved) = vue3_resolve_import_type(import_type, analysis) {
                if let Some(keys) = resolved
                    .context
                    .ordered_string_literal_type_declarations
                    .get(&resolved.name)
                {
                    for key in keys {
                        push_unique(names, key);
                    }
                }
            }
        }
        _ => {}
    }
}

pub(crate) fn collect_vue27_emits_from_parameters(
    parameters: &[FormalParameter<'_>],
    names: &mut Vec<String>,
) {
    let Some(parameter) = parameters.first() else {
        return;
    };
    let Some(annotation) = parameter.type_annotation.as_ref() else {
        return;
    };
    collect_vue27_emits_from_type(&annotation.type_annotation, names);
}

pub(crate) fn collect_vue27_emits_from_type(ty: &TSType<'_>, names: &mut Vec<String>) {
    match ty {
        TSType::TSLiteralType(literal) => {
            if let Some(name) = vue27_literal_event_name(&literal.literal) {
                push_unique(names, &name);
            }
        }
        TSType::TSUnionType(union) => {
            for ty in &union.types {
                collect_vue27_emits_from_type(ty, names);
            }
        }
        _ => {}
    }
}

pub(crate) fn vue27_literal_event_name(literal: &TSLiteral<'_>) -> Option<String> {
    match literal {
        TSLiteral::StringLiteral(literal) => Some(literal.value.to_string()),
        TSLiteral::BooleanLiteral(literal) => Some(literal.value.to_string()),
        TSLiteral::NumericLiteral(literal) => Some(literal.value.to_string()),
        TSLiteral::BigIntLiteral(literal) => Some(literal.value.to_string()),
        _ => None,
    }
}

pub(crate) fn vue27_property_key_static_name(key: &PropertyKey<'_>) -> Option<String> {
    key.static_name().map(|name| name.into_owned())
}

pub(crate) fn vue27_ts_type_name_identifier<'a>(name: &'a TSTypeName<'a>) -> Option<&'a str> {
    match name {
        TSTypeName::IdentifierReference(identifier) => Some(identifier.name.as_str()),
        _ => None,
    }
}

pub(crate) fn vue3_ts_type_name_key(name: &TSTypeName<'_>) -> Option<String> {
    match name {
        TSTypeName::IdentifierReference(identifier) => Some(identifier.name.to_string()),
        TSTypeName::QualifiedName(qualified) => {
            let left = vue3_ts_type_name_key(&qualified.left)?;
            Some(format!("{left}.{}", qualified.right.name))
        }
        TSTypeName::ThisExpression(_) => None,
    }
}

pub(crate) fn vue3_type_query_name_key(query: &TSTypeQuery<'_>) -> Option<String> {
    match &query.expr_name {
        TSTypeQueryExprName::IdentifierReference(identifier) => Some(identifier.name.to_string()),
        TSTypeQueryExprName::QualifiedName(qualified) => {
            let left = vue3_ts_type_name_key(&qualified.left)?;
            Some(format!("{left}.{}", qualified.right.name))
        }
        TSTypeQueryExprName::TSImportType(_) | TSTypeQueryExprName::ThisExpression(_) => None,
    }
}

pub(crate) fn vue3_resolve_type_query_import_type(
    query: &TSTypeQuery<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vue3ResolvedImportType> {
    match &query.expr_name {
        TSTypeQueryExprName::TSImportType(import_type) => {
            vue3_resolve_import_type(import_type, analysis)
        }
        _ => None,
    }
}

pub(crate) fn vue3_type_query_props_options_declaration(
    query: &TSTypeQuery<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vue27TypeMembers> {
    if let Some(name) = vue3_type_query_name_key(query) {
        return analysis.props_options_type_declarations.get(&name).cloned();
    }
    let resolved = vue3_resolve_type_query_import_type(query, analysis)?;
    resolved
        .context
        .props_options_type_declarations
        .get(&resolved.name)
        .cloned()
}

pub(crate) fn vue3_type_query_return_props_options_declaration(
    query: &TSTypeQuery<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vue27TypeMembers> {
    if let Some(name) = vue3_type_query_name_key(query) {
        return analysis
            .return_type_props_options_declarations
            .get(&name)
            .cloned();
    }
    let resolved = vue3_resolve_type_query_import_type(query, analysis)?;
    resolved
        .context
        .return_type_props_options_declarations
        .get(&resolved.name)
        .cloned()
}

pub(crate) fn vue3_type_query_runtime_type_declaration(
    query: &TSTypeQuery<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vec<String>> {
    if let Some(name) = vue3_type_query_name_key(query) {
        return analysis.type_query_declared_types.get(&name).cloned();
    }
    let resolved = vue3_resolve_type_query_import_type(query, analysis)?;
    resolved
        .context
        .type_query_declared_types
        .get(&resolved.name)
        .cloned()
}

pub(crate) fn vue3_type_query_define_model_runtime_type_declaration(
    query: &TSTypeQuery<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vec<String>> {
    if let Some(name) = vue3_type_query_name_key(query) {
        return analysis
            .define_model_type_query_declared_types
            .get(&name)
            .cloned();
    }
    let resolved = vue3_resolve_type_query_import_type(query, analysis)?;
    resolved
        .context
        .define_model_type_query_declared_types
        .get(&resolved.name)
        .cloned()
}

pub(crate) fn vue3_type_query_keyof_runtime_type_declaration(
    query: &TSTypeQuery<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vec<String>> {
    if let Some(name) = vue3_type_query_name_key(query) {
        return analysis.keyof_type_query_declared_types.get(&name).cloned();
    }
    let resolved = vue3_resolve_type_query_import_type(query, analysis)?;
    resolved
        .context
        .keyof_type_query_declared_types
        .get(&resolved.name)
        .cloned()
}

pub(crate) fn vue3_return_type_declaration_for_type_query(
    query: &TSTypeQuery<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vec<String>> {
    if let Some(name) = vue3_type_query_name_key(query) {
        return vue3_return_type_declaration_for_mode(analysis, &name, mode);
    }
    let resolved = vue3_resolve_type_query_import_type(query, analysis)?;
    vue3_return_type_declaration_for_context(&resolved.context, &resolved.name, mode)
}

pub(crate) fn collect_vue3_type_query_deps(
    query: &TSTypeQuery<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    deps: &mut BTreeSet<String>,
) {
    if let Some(name) = vue3_type_query_name_key(query) {
        collect_vue3_named_type_deps(&name, analysis, deps);
    } else if let Some(resolved) = vue3_resolve_type_query_import_type(query, analysis) {
        deps.extend(
            resolved
                .context
                .type_deps
                .get(&resolved.name)
                .cloned()
                .unwrap_or_default(),
        );
        deps.insert(resolved.dependency);
    }
    if let Some(type_arguments) = query.type_arguments.as_ref() {
        for ty in &type_arguments.params {
            collect_vue3_type_argument_deps_into(ty, analysis, deps);
        }
    }
}

pub(crate) fn collect_vue3_type_query_deps_ordered(
    query: &TSTypeQuery<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    deps: &mut Vec<String>,
) {
    if let Some(name) = vue3_type_query_name_key(query) {
        collect_vue3_named_type_deps_ordered(&name, analysis, deps);
    } else if let Some(resolved) = vue3_resolve_type_query_import_type(query, analysis) {
        push_unique(deps, &resolved.dependency);
        collect_vue3_context_type_deps_ordered(&resolved.context, &resolved.name, deps);
    }
    if let Some(type_arguments) = query.type_arguments.as_ref() {
        for ty in &type_arguments.params {
            collect_vue3_type_argument_deps_ordered_into(ty, analysis, deps);
        }
    }
}

pub(crate) fn vue3_import_type_qualifier_key(qualifier: &TSImportTypeQualifier<'_>) -> String {
    match qualifier {
        TSImportTypeQualifier::Identifier(identifier) => identifier.name.to_string(),
        TSImportTypeQualifier::QualifiedName(qualified) => {
            let left = vue3_import_type_qualifier_key(&qualified.left);
            format!("{left}.{}", qualified.right.name)
        }
    }
}

pub(crate) fn escape_js_double(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

pub(crate) fn first_pattern_binding(pattern: &BindingPattern<'_>) -> Option<String> {
    match pattern {
        BindingPattern::BindingIdentifier(identifier) => Some(identifier.name.to_string()),
        BindingPattern::ObjectPattern(pattern) => pattern
            .properties
            .iter()
            .find_map(|property| first_pattern_binding(&property.value))
            .or_else(|| {
                pattern
                    .rest
                    .as_ref()
                    .and_then(|rest| first_pattern_binding(&rest.argument))
            }),
        BindingPattern::ArrayPattern(pattern) => pattern
            .elements
            .iter()
            .flatten()
            .find_map(first_pattern_binding)
            .or_else(|| {
                pattern
                    .rest
                    .as_ref()
                    .and_then(|rest| first_pattern_binding(&rest.argument))
            }),
        BindingPattern::AssignmentPattern(pattern) => first_pattern_binding(&pattern.left),
    }
}

pub(crate) fn vue27_props_alias_declaration(source: &str, pattern: &BindingPattern<'_>) -> String {
    let pattern_source = source
        .get(pattern.span().start as usize..pattern.span().end as usize)
        .map(str::trim)
        .filter(|source| !source.is_empty());
    if let Some(pattern_source) = pattern_source {
        format!("\nconst {pattern_source} = __props;\n")
    } else {
        String::new()
    }
}

pub(crate) fn remove_vue27_macro_declarators(
    declaration: &VariableDeclaration<'_>,
    macro_indices: &[usize],
    edits: &mut SourceEdits<'_>,
) {
    if macro_indices.is_empty() {
        return;
    }
    if macro_indices.len() == declaration.declarations.len() {
        edits.remove(
            declaration.span.start as usize,
            declaration.span.end as usize,
        );
        return;
    }
    let mut spans = Vec::new();
    for index in macro_indices {
        let declarator = &declaration.declarations[*index];
        let (start, end) = if *index == 0 {
            (
                declarator.span.start as usize,
                declaration.declarations[index + 1].span.start as usize,
            )
        } else {
            (
                declaration.declarations[index - 1].span.end as usize,
                declarator.span.end as usize,
            )
        };
        spans.push((start, end));
    }
    spans.sort_unstable();
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (start, end) in spans {
        if let Some((_, last_end)) = merged.last_mut() {
            if start <= *last_end {
                *last_end = (*last_end).max(end);
                continue;
            }
        }
        merged.push((start, end));
    }
    for (start, end) in merged {
        edits.remove(start, end);
    }
}

pub(crate) fn object_expression_keys(object: &ObjectExpression<'_>) -> Vec<String> {
    object
        .properties
        .iter()
        .filter_map(|property| property.as_property())
        .filter(|property| !property.computed)
        .filter_map(|property| property.key.static_name().map(|name| name.into_owned()))
        .collect()
}

pub(crate) fn import_specifier_local(specifier: &ImportDeclarationSpecifier<'_>) -> String {
    match specifier {
        ImportDeclarationSpecifier::ImportSpecifier(specifier) => specifier.local.name.to_string(),
        ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
            specifier.local.name.to_string()
        }
        ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
            specifier.local.name.to_string()
        }
    }
}

pub(crate) fn import_specifier_imported(
    specifier: &ImportDeclarationSpecifier<'_>,
) -> Option<String> {
    match specifier {
        ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
            Some(specifier.imported.name().to_string())
        }
        ImportDeclarationSpecifier::ImportDefaultSpecifier(_) => Some("default".into()),
        ImportDeclarationSpecifier::ImportNamespaceSpecifier(_) => Some("*".into()),
    }
}

pub(crate) fn vue27_import_specifier_is_type(
    import: &oxc_ast::ast::ImportDeclaration<'_>,
    specifier: &ImportDeclarationSpecifier<'_>,
) -> bool {
    import.import_kind == ImportOrExportKind::Type
        || matches!(
            specifier,
            ImportDeclarationSpecifier::ImportSpecifier(specifier)
                if specifier.import_kind == ImportOrExportKind::Type
        )
}

pub(crate) fn import_specifier_setup_dedupe_imported(
    specifier: &ImportDeclarationSpecifier<'_>,
) -> Option<String> {
    match specifier {
        ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
            Some(specifier.imported.name().to_string())
        }
        ImportDeclarationSpecifier::ImportNamespaceSpecifier(_) => Some("*".into()),
        ImportDeclarationSpecifier::ImportDefaultSpecifier(_) => None,
    }
}

pub(crate) fn vue27_import_already_declared_in_setup_context(
    analysis: &Vue27ScriptSetupAnalysis,
    source: &str,
    local: &str,
    imported: Option<&str>,
) -> bool {
    analysis.normal_imports.iter().any(|existing| {
        existing.local == local
            && existing.source == source
            && existing.imported == imported.unwrap_or("default")
    })
}

pub(crate) fn vue27_import_local_conflicts_in_setup_context(
    analysis: &Vue27ScriptSetupAnalysis,
    source: &str,
    local: &str,
    imported: Option<&str>,
) -> bool {
    analysis.normal_imports.iter().any(|existing| {
        existing.local == local
            && (existing.source != source || existing.imported != imported.unwrap_or("default"))
    })
}

pub(crate) fn import_specifier_source(
    source: &str,
    specifier: &ImportDeclarationSpecifier<'_>,
) -> String {
    source[specifier.span().start as usize..specifier.span().end as usize].to_string()
}

pub(crate) fn vue3_script_setup_kept_import_source(
    source: &str,
    import: &oxc_ast::ast::ImportDeclaration<'_>,
    source_value: &str,
    statement_start: usize,
    statement_end: usize,
    keep_specifier_indices: &[usize],
) -> Option<String> {
    let Some(specifiers) = import.specifiers.as_ref() else {
        return source
            .get(statement_start..statement_end)
            .map(ToOwned::to_owned);
    };
    let kept = specifiers
        .iter()
        .enumerate()
        .filter(|(index, _)| keep_specifier_indices.contains(index))
        .map(|(_, specifier)| specifier)
        .collect::<Vec<_>>();
    if kept.is_empty() {
        return None;
    }
    if kept.len() == specifiers.len() {
        return source
            .get(statement_start..statement_end)
            .map(ToOwned::to_owned);
    }
    let trailing = source
        .get(import.span().end as usize..statement_end)
        .unwrap_or_default();
    let mut default_import = None;
    let mut namespace_import = None;
    let mut named_imports = Vec::new();
    for specifier in kept {
        let specifier_source = import_specifier_source(source, specifier);
        match specifier {
            ImportDeclarationSpecifier::ImportDefaultSpecifier(_) => {
                default_import = Some(specifier_source);
            }
            ImportDeclarationSpecifier::ImportNamespaceSpecifier(_) => {
                namespace_import = Some(specifier_source);
            }
            ImportDeclarationSpecifier::ImportSpecifier(_) => {
                named_imports.push(specifier_source);
            }
        }
    }
    let mut import_clause = String::new();
    if let Some(default_import) = default_import {
        import_clause.push_str(&default_import);
    }
    if let Some(namespace_import) = namespace_import {
        if !import_clause.is_empty() {
            import_clause.push_str(", ");
        }
        import_clause.push_str(&namespace_import);
    }
    if !named_imports.is_empty() {
        if !import_clause.is_empty() {
            import_clause.push_str(", ");
        }
        import_clause.push_str("{ ");
        import_clause.push_str(&named_imports.join(", "));
        import_clause.push_str(" }");
    }
    Some(format!(
        "import {import_clause} from '{source_value}'{trailing}"
    ))
}

pub(crate) fn collect_vue3_setup_import_aliases(
    statements: &[Statement<'_>],
    normal_user_imports: &Vue3UserImports,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    let mut user_imports = normal_user_imports.clone();
    for statement in statements {
        let Statement::ImportDeclaration(import) = statement else {
            continue;
        };
        let source = import.source.value.as_str();
        let Some(specifiers) = &import.specifiers else {
            continue;
        };
        for specifier in specifiers {
            if vue3_import_specifier_compiler_macro(source, specifier).is_some() {
                continue;
            }
            let local = import_specifier_local(specifier);
            if user_imports.existing(&local).is_some() {
                continue;
            }
            user_imports.record(Vue27ScriptImport {
                local,
                source: source.to_string(),
                imported: import_specifier_imported(specifier).unwrap_or_else(|| "default".into()),
                is_type: vue27_import_specifier_is_type(import, specifier),
            });
        }
    }
    analysis.vue_import_aliases = user_imports.vue_aliases();
}

pub(crate) fn vue3_import_specifier_compiler_macro(
    source: &str,
    specifier: &ImportDeclarationSpecifier<'_>,
) -> Option<(String, String)> {
    if source != "vue" {
        return None;
    }
    let ImportDeclarationSpecifier::ImportSpecifier(specifier) = specifier else {
        return None;
    };
    let imported = specifier.imported.name();
    if !matches!(
        imported.as_str(),
        "defineProps"
            | "defineEmits"
            | "defineExpose"
            | "defineOptions"
            | "defineModel"
            | "defineSlots"
            | "withDefaults"
    ) {
        return None;
    }
    Some((imported.to_string(), specifier.local.name.to_string()))
}

pub(crate) fn vue27_template_uses_identifier(template: &str, local: &str, is_ts: bool) -> bool {
    let usage = vue27_template_usage_check_string(template, is_ts);
    identifier_usage_contains(&usage, local)
}

pub(crate) fn vue3_template_uses_identifier(template: &str, local: &str, is_ts: bool) -> bool {
    let usage = vue3_template_usage_check_string(template, is_ts);
    identifier_usage_contains(&usage, local)
}

pub(crate) fn vue3_template_usage_check_string(template: &str, is_ts: bool) -> String {
    let mut code = String::new();
    for token in HtmlTokenizer::new(template).tokenize() {
        match token.kind {
            HtmlTokenKind::StartTag {
                name, attributes, ..
            } => {
                collect_vue3_template_component_usage(&mut code, &name);
                for attribute in attributes {
                    collect_vue3_template_attribute_usage(&mut code, &attribute, is_ts);
                }
            }
            HtmlTokenKind::Text(text) => {
                collect_vue27_template_text_usage(&mut code, &text, is_ts);
            }
            _ => {}
        }
    }
    code.push(';');
    code
}

pub(crate) fn collect_vue3_template_component_usage(code: &mut String, name: &str) {
    let tag = name
        .split_once('.')
        .map(|(base, _)| base.trim())
        .unwrap_or(name);
    if tag.is_empty() || vue3_template_is_builtin_tag(tag) || vue27_template_is_reserved_tag(tag) {
        return;
    }
    let camel = vue27_camelize(tag);
    code.push(',');
    code.push_str(&camel);
    code.push(',');
    code.push_str(&vue27_capitalize(&camel));
}

pub(crate) fn collect_vue3_template_attribute_usage(
    code: &mut String,
    attr: &HtmlAttribute,
    is_ts: bool,
) {
    let name = attr.name.as_str();
    if vue3_template_is_directive_attr(name) {
        let base_name = vue27_template_directive_base_name(name);
        if !vue27_template_is_builtin_dir(&base_name) {
            code.push_str(",v");
            code.push_str(&vue27_capitalize(&vue27_camelize(&base_name)));
        }
        if let Some(arg) = vue3_template_dynamic_argument(name) {
            code.push(',');
            code.push_str(&vue27_process_template_exp(arg, is_ts, None));
        }
        if let Some(value) = attr.value.as_deref() {
            code.push(',');
            code.push_str(&vue27_process_template_exp(value, is_ts, Some(&base_name)));
        } else if base_name == "bind" {
            if let Some(arg) = vue3_template_static_bind_argument(name) {
                code.push(',');
                code.push_str(&vue27_camelize(arg));
            }
        }
    } else if name == "ref" {
        if let Some(value) = attr.value.as_deref() {
            code.push(',');
            code.push_str(value);
        }
    }
}

pub(crate) fn vue3_template_is_directive_attr(name: &str) -> bool {
    vue27_template_is_directive_attr(name) || name.starts_with('.')
}

pub(crate) fn vue3_template_dynamic_argument(name: &str) -> Option<&str> {
    let start = name.find('[')?;
    let rest = &name[start + 1..];
    let end = rest.find(']')?;
    Some(&rest[..end])
}

pub(crate) fn vue3_template_static_bind_argument(name: &str) -> Option<&str> {
    if vue3_template_dynamic_argument(name).is_some() {
        return None;
    }
    let raw = if let Some(arg) = name.strip_prefix(':') {
        arg
    } else if let Some(arg) = name.strip_prefix('.') {
        arg
    } else if let Some(arg) = name.strip_prefix("v-bind:") {
        arg
    } else {
        return None;
    };
    raw.split('.').next().filter(|arg| !arg.is_empty())
}

pub(crate) fn vue3_template_is_builtin_tag(name: &str) -> bool {
    vue27_template_is_builtin_tag(name)
        || matches!(
            name,
            "Teleport"
                | "teleport"
                | "Suspense"
                | "suspense"
                | "KeepAlive"
                | "keep-alive"
                | "BaseTransition"
                | "base-transition"
                | "Transition"
                | "transition"
                | "TransitionGroup"
                | "transition-group"
        )
}

pub(crate) fn vue27_template_usage_check_string(template: &str, is_ts: bool) -> String {
    let mut code = String::new();
    for token in HtmlTokenizer::new(template).tokenize() {
        match token.kind {
            HtmlTokenKind::StartTag {
                name, attributes, ..
            } => {
                if !vue27_template_is_builtin_tag(&name) && !vue27_template_is_reserved_tag(&name) {
                    let camel = vue27_camelize(&name);
                    code.push(',');
                    code.push_str(&camel);
                    code.push(',');
                    code.push_str(&vue27_capitalize(&camel));
                }
                for attribute in attributes {
                    collect_vue27_template_attribute_usage(&mut code, &attribute, is_ts);
                }
            }
            HtmlTokenKind::Text(text) => {
                collect_vue27_template_text_usage(&mut code, &text, is_ts);
            }
            _ => {}
        }
    }
    code.push(';');
    code
}

pub(crate) fn collect_vue27_template_attribute_usage(
    code: &mut String,
    attr: &HtmlAttribute,
    is_ts: bool,
) {
    let name = attr.name.as_str();
    if vue27_template_is_directive_attr(name) {
        let base_name = vue27_template_directive_base_name(name);
        if !vue27_template_is_builtin_dir(&base_name) {
            code.push_str(",v");
            code.push_str(&vue27_capitalize(&vue27_camelize(&base_name)));
        }
        if let Some(value) = attr.value.as_deref() {
            code.push(',');
            code.push_str(&vue27_process_template_exp(value, is_ts, Some(&base_name)));
        }
    } else if name == "ref" {
        if let Some(value) = attr.value.as_deref() {
            code.push(',');
            code.push_str(value);
        }
    }
}

pub(crate) fn collect_vue27_template_text_usage(code: &mut String, text: &str, is_ts: bool) {
    let mut rest = text;
    while let Some(start) = rest.find("{{") {
        let after_start = &rest[start + 2..];
        let Some(end) = after_start.find("}}") else {
            break;
        };
        let expression = after_start[..end].trim();
        if !expression.is_empty() {
            code.push(',');
            code.push_str(&vue27_process_template_exp(expression, is_ts, None));
        }
        rest = &after_start[end + 2..];
    }
}

pub(crate) fn vue27_template_directive_base_name(name: &str) -> String {
    let body = if let Some(value) = name.strip_prefix("v-") {
        value
    } else if name.starts_with('@') {
        return "on".into();
    } else if name.starts_with('#') {
        return "slot".into();
    } else if name.starts_with(':') {
        return "bind".into();
    } else {
        name
    };
    body.split([':', '.', '['])
        .next()
        .filter(|value| !value.is_empty())
        .unwrap_or(body)
        .to_string()
}

pub(crate) fn vue27_template_is_directive_attr(name: &str) -> bool {
    name.starts_with("v-")
        || name.starts_with(':')
        || name.starts_with('@')
        || name.starts_with('#')
}

pub(crate) fn vue27_template_is_builtin_dir(name: &str) -> bool {
    matches!(
        name,
        "text"
            | "html"
            | "show"
            | "if"
            | "else"
            | "else-if"
            | "for"
            | "on"
            | "bind"
            | "model"
            | "slot"
            | "pre"
            | "cloak"
            | "once"
            | "memo"
    )
}

pub(crate) fn vue27_template_is_builtin_tag(name: &str) -> bool {
    matches!(name, "slot" | "component")
}

pub(crate) fn vue27_template_is_reserved_tag(name: &str) -> bool {
    const RESERVED: &str = concat!(
        "html,body,base,head,link,meta,style,title,address,article,aside,footer,header,h1,h2,h3,h4,h5,h6,",
        "nav,section,div,dd,dl,dt,figcaption,figure,picture,hr,img,li,main,ol,p,pre,ul,a,b,abbr,bdi,bdo,",
        "br,cite,code,data,dfn,em,i,kbd,mark,q,rp,rt,ruby,s,samp,small,span,strong,sub,sup,time,u,var,wbr,",
        "area,audio,map,track,video,embed,object,param,source,canvas,script,noscript,del,ins,caption,col,",
        "colgroup,table,thead,tbody,td,th,tr,button,datalist,fieldset,form,input,label,legend,meter,optgroup,",
        "option,output,progress,select,textarea,details,dialog,menu,menuitem,summary,content,element,shadow,",
        "template,blockquote,iframe,tfoot,svg,animate,circle,clippath,cursor,defs,desc,ellipse,filter,font-face,",
        "foreignObject,g,glyph,image,line,marker,mask,missing-glyph,path,pattern,polygon,polyline,rect,switch,",
        "symbol,text,textpath,tspan,use,view"
    );
    RESERVED
        .split(',')
        .any(|tag| tag.eq_ignore_ascii_case(name))
}

pub(crate) fn vue27_process_template_exp(
    exp: &str,
    is_ts: bool,
    directive: Option<&str>,
) -> String {
    if is_ts && vue27_template_exp_has_ts_syntax(exp) {
        if directive == Some("slot") {
            return vue27_extract_js_identifiers(&format!("({exp})=>{{}}"));
        }
        if directive == Some("on") {
            return vue27_extract_js_identifiers(&format!("()=>{{return {exp}}}"));
        }
        if directive == Some("for") {
            if let Some((left, right)) = vue27_split_for_expression(exp) {
                let mut value = vue27_extract_js_identifiers(&format!("({left})=>{{}}"));
                value.push_str(&vue27_extract_js_identifiers(right));
                return value;
            }
        }
        return vue27_extract_js_identifiers(exp);
    }
    let identifiers = vue27_extract_js_identifiers(exp);
    if identifiers.is_empty() {
        vue27_strip_template_expression_strings(exp)
    } else {
        identifiers
    }
}

pub(crate) fn vue27_template_exp_has_ts_syntax(exp: &str) -> bool {
    exp.contains(':') || exp.contains('<') || exp.split_whitespace().any(|part| part == "as")
}

pub(crate) fn vue27_split_for_expression(exp: &str) -> Option<(&str, &str)> {
    for keyword in [" in ", " of "] {
        if let Some(index) = exp.find(keyword) {
            let left = exp[..index].trim();
            let right = exp[index + keyword.len()..].trim();
            if !left.is_empty() && !right.is_empty() {
                return Some((left, right));
            }
        }
    }
    None
}

pub(crate) fn vue27_extract_js_identifiers(exp: &str) -> String {
    let allocator = oxc_allocator::Allocator::default();
    let parse_options = oxc_parser::ParseOptions {
        parse_regular_expression: true,
        ..oxc_parser::ParseOptions::default()
    };
    if let Ok(expression) = oxc_parser::Parser::new(
        &allocator,
        exp,
        oxc_span::SourceType::ts().with_module(false),
    )
    .with_options(parse_options)
    .parse_expression()
    {
        let mut value = String::new();
        collect_vue27_expression_identifier_usage(&expression, &mut value);
        return value;
    }
    let parsed = oxc_parser::Parser::new(
        &allocator,
        exp,
        oxc_span::SourceType::ts().with_module(false),
    )
    .with_options(parse_options)
    .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return String::new();
    }
    let mut value = String::new();
    for statement in &parsed.program.body {
        collect_vue27_statement_identifier_usage(statement, &mut value);
    }
    value
}

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
                        while let Some((_, expr_ch)) = chars.next() {
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
    value.trim_end_matches(|ch| matches!(ch, '\n' | '\r'))
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
