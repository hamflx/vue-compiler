pub(crate) struct Vue3ScriptSetupAnalysisOptions {
    pub(crate) hoist_static_literals: bool,
    pub(crate) props_destructure: SfcPropsDestructureMode,
    pub(crate) is_prod: bool,
    pub(crate) custom_element: bool,
}

pub(crate) fn analyze_vue3_script_setup(
    filename: &str,
    descriptor: &SfcDescriptor,
    script_setup: &SfcBlock,
    normal_type_context: &Vue27TypeContext,
    normal_user_imports: &Vue3UserImports,
    type_resolver: &Vue3TypeResolverContext,
    options: Vue3ScriptSetupAnalysisOptions,
) -> Vue3ScriptSetupAnalysis {
    let Vue3ScriptSetupAnalysisOptions {
        hoist_static_literals,
        props_destructure,
        is_prod,
        custom_element,
    } = options;
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
        return Vue3ScriptSetupAnalysis {
            setup_content: source.to_string(),
            errors: parsed.errors.iter().map(ToString::to_string).collect(),
            ..Vue3ScriptSetupAnalysis::default()
        };
    }

    let mut type_context = normal_type_context.clone();
    extend_vue3_type_context_from_external_imports(
        filename,
        source,
        script_source_type_from_attrs(&script_setup.attrs),
        &mut type_context,
        type_resolver,
    );
    let mut type_analysis = Vue3ScriptSetupAnalysis {
        vue_import_aliases: normal_user_imports.vue_aliases(),
        declared_types: type_context.declared_types,
        define_model_declared_types: type_context.define_model_declared_types,
        type_query_declared_types: type_context.type_query_declared_types,
        define_model_type_query_declared_types: type_context.define_model_type_query_declared_types,
        keyof_type_query_declared_types: type_context.keyof_type_query_declared_types,
        props_type_declarations: type_context.props_type_declarations,
        keyof_runtime_type_declarations: type_context.keyof_runtime_type_declarations,
        tuple_runtime_type_declarations: type_context.tuple_runtime_type_declarations,
        define_model_tuple_runtime_type_declarations: type_context
            .define_model_tuple_runtime_type_declarations,
        array_element_runtime_type_declarations: type_context
            .array_element_runtime_type_declarations,
        define_model_array_element_runtime_type_declarations: type_context
            .define_model_array_element_runtime_type_declarations,
        parameter_tuple_runtime_type_declarations: type_context
            .parameter_tuple_runtime_type_declarations,
        define_model_parameter_tuple_runtime_type_declarations: type_context
            .define_model_parameter_tuple_runtime_type_declarations,
        constructor_parameter_tuple_runtime_type_declarations: type_context
            .constructor_parameter_tuple_runtime_type_declarations,
        define_model_constructor_parameter_tuple_runtime_type_declarations: type_context
            .define_model_constructor_parameter_tuple_runtime_type_declarations,
        return_type_runtime_type_declarations: type_context.return_type_runtime_type_declarations,
        define_model_return_type_runtime_type_declarations: type_context
            .define_model_return_type_runtime_type_declarations,
        props_options_type_declarations: type_context.props_options_type_declarations,
        return_type_props_options_declarations: type_context.return_type_props_options_declarations,
        generic_type_aliases: type_context.generic_type_aliases,
        string_literal_type_declarations: type_context.string_literal_type_declarations,
        ordered_string_literal_type_declarations: type_context
            .ordered_string_literal_type_declarations,
        emits_type_declarations: type_context.emits_type_declarations,
        type_sources: type_context.type_sources,
        type_direct_deps: type_context.type_direct_deps,
        type_deps: type_context.type_deps,
        unresolved_import_sources: type_context.unresolved_import_sources,
        silent_unresolved_type_names: type_context.silent_unresolved_type_names,
        type_filename: Some(filename.to_string()),
        type_resolver: type_resolver.clone(),
        ..Vue3ScriptSetupAnalysis::default()
    };
    collect_vue3_setup_import_aliases(
        &parsed.program.body,
        normal_user_imports,
        &mut type_analysis,
    );
    collect_vue3_declared_types_from_statements(source, &parsed.program.body, &mut type_analysis);
    collect_vue3_declared_type_deps_from_statements(&parsed.program.body, &mut type_analysis);
    collect_vue3_setup_local_bindings(
        &parsed.program.body,
        is_ts,
        hoist_static_literals,
        &mut type_analysis,
    );

    let mut edits = SourceEdits::new(source);
    let mut analysis = Vue3ScriptSetupAnalysis {
        declared_types: type_analysis.declared_types,
        define_model_declared_types: type_analysis.define_model_declared_types,
        type_query_declared_types: type_analysis.type_query_declared_types,
        define_model_type_query_declared_types: type_analysis
            .define_model_type_query_declared_types,
        keyof_type_query_declared_types: type_analysis.keyof_type_query_declared_types,
        props_type_declarations: type_analysis.props_type_declarations,
        keyof_runtime_type_declarations: type_analysis.keyof_runtime_type_declarations,
        tuple_runtime_type_declarations: type_analysis.tuple_runtime_type_declarations,
        define_model_tuple_runtime_type_declarations: type_analysis
            .define_model_tuple_runtime_type_declarations,
        array_element_runtime_type_declarations: type_analysis
            .array_element_runtime_type_declarations,
        define_model_array_element_runtime_type_declarations: type_analysis
            .define_model_array_element_runtime_type_declarations,
        parameter_tuple_runtime_type_declarations: type_analysis
            .parameter_tuple_runtime_type_declarations,
        define_model_parameter_tuple_runtime_type_declarations: type_analysis
            .define_model_parameter_tuple_runtime_type_declarations,
        constructor_parameter_tuple_runtime_type_declarations: type_analysis
            .constructor_parameter_tuple_runtime_type_declarations,
        define_model_constructor_parameter_tuple_runtime_type_declarations: type_analysis
            .define_model_constructor_parameter_tuple_runtime_type_declarations,
        return_type_runtime_type_declarations: type_analysis.return_type_runtime_type_declarations,
        define_model_return_type_runtime_type_declarations: type_analysis
            .define_model_return_type_runtime_type_declarations,
        props_options_type_declarations: type_analysis.props_options_type_declarations,
        return_type_props_options_declarations: type_analysis
            .return_type_props_options_declarations,
        generic_type_aliases: type_analysis.generic_type_aliases,
        string_literal_type_declarations: type_analysis.string_literal_type_declarations,
        ordered_string_literal_type_declarations: type_analysis
            .ordered_string_literal_type_declarations,
        emits_type_declarations: type_analysis.emits_type_declarations,
        type_sources: type_analysis.type_sources,
        type_direct_deps: type_analysis.type_direct_deps,
        type_deps: type_analysis.type_deps,
        unresolved_import_sources: type_analysis.unresolved_import_sources,
        silent_unresolved_type_names: type_analysis.silent_unresolved_type_names,
        type_filename: Some(filename.to_string()),
        type_resolver: type_resolver.clone(),
        local_setup_bindings: type_analysis.local_setup_bindings,
        local_setup_binding_types: type_analysis.local_setup_binding_types,
        vue_import_aliases: type_analysis.vue_import_aliases,
        ..Vue3ScriptSetupAnalysis::default()
    };
    let mut user_imports = normal_user_imports.clone();
    let mut module_chunks = Vec::new();
    for statement in &parsed.program.body {
        match statement {
            Statement::ImportDeclaration(import) => {
                let (start, end) = vue27_statement_span_with_trailing_ws(source, statement);
                let end = vue27_statement_span_with_trailing_comments(
                    source,
                    end,
                    &parsed.program.comments,
                );
                let source_value = import.source.value.as_str();
                let mut keep_specifier_indices = Vec::new();
                if let Some(specifiers) = &import.specifiers {
                    for (index, specifier) in specifiers.iter().enumerate() {
                        let local = import_specifier_local(specifier);
                        let imported = import_specifier_imported(specifier)
                            .unwrap_or_else(|| "default".into());
                        if let Some((imported, local)) =
                            vue3_import_specifier_compiler_macro(source_value, specifier)
                        {
                            analysis.removed_bindings.insert(local.clone());
                            if imported != local {
                                analysis.errors.push(format!(
                                    "`{imported}` is a compiler macro and cannot be aliased to a different name."
                                ));
                            }
                            continue;
                        }
                        let is_type = vue27_import_specifier_is_type(import, specifier);
                        let import_binding = Vue27ScriptImport {
                            local: local.clone(),
                            source: source_value.to_string(),
                            imported: imported.clone(),
                            is_type,
                        };
                        if let Some(existing) = user_imports.existing(&local) {
                            if existing.source == source_value
                                && existing.imported == imported
                                && existing.is_type == is_type
                            {
                                continue;
                            }
                            analysis
                                .errors
                                .push("different imports aliased to same local name.".into());
                        }
                        if source_value == "vue" {
                            analysis
                                .vue_import_aliases
                                .insert(imported.clone(), local.clone());
                        }
                        user_imports.record(import_binding.clone());
                        analysis.imports.push(import_binding);
                        keep_specifier_indices.push(index);
                    }
                }
                if let Some(import_source) = vue3_script_setup_kept_import_source(
                    source,
                    import,
                    source_value,
                    start,
                    end,
                    &keep_specifier_indices,
                ) {
                    module_chunks.push(Vue27ModuleChunk {
                        start,
                        content: import_source,
                    });
                } else if analysis.removed_leading_import_padding.is_none() {
                    if let Some(padding) =
                        vue3_removed_setup_import_leading_padding(source, statement)
                    {
                        analysis.removed_leading_import_padding = Some(padding);
                    }
                }
                edits.remove(start, end);
            }
            Statement::VariableDeclaration(declaration) => {
                if hoist_static_literals && vue3_variable_declaration_is_static_hoist(declaration) {
                    let (start, end) = vue27_statement_span_with_trailing_ws(source, statement);
                    if let Some(statement_source) = source.get(start..end) {
                        module_chunks.push(Vue27ModuleChunk {
                            start,
                            content: statement_source.to_string(),
                        });
                    }
                    analyze_vue3_setup_variable_declaration(
                        source,
                        declaration,
                        &mut edits,
                        &mut analysis,
                        props_destructure,
                        is_prod,
                        custom_element,
                        hoist_static_literals,
                    );
                    edits.remove(start, end);
                    continue;
                }
                analyze_vue3_setup_variable_declaration(
                    source,
                    declaration,
                    &mut edits,
                    &mut analysis,
                    props_destructure,
                    is_prod,
                    custom_element,
                    hoist_static_literals,
                );
            }
            Statement::FunctionDeclaration(function) if !function.declare => {
                if let Some(id) = &function.id {
                    push_unique(&mut analysis.return_bindings, id.name.as_str());
                    analysis
                        .setup_bindings
                        .insert(id.name.to_string(), "setup-const".into());
                }
            }
            Statement::ClassDeclaration(class) if !class.declare => {
                if let Some(id) = &class.id {
                    push_unique(&mut analysis.return_bindings, id.name.as_str());
                    analysis
                        .setup_bindings
                        .insert(id.name.to_string(), "setup-const".into());
                }
            }
            Statement::TSEnumDeclaration(declaration) if is_ts && !declaration.declare => {
                push_unique(&mut analysis.return_bindings, declaration.id.name.as_str());
                analysis.setup_bindings.insert(
                    declaration.id.name.to_string(),
                    vue3_ts_enum_binding_type(declaration).into(),
                );
                if hoist_static_literals && vue3_ts_enum_is_static_literal(declaration) {
                    let (start, end) = vue27_statement_span_with_trailing_ws(source, statement);
                    if let Some(statement_source) = source.get(start..end) {
                        module_chunks.push(Vue27ModuleChunk {
                            start,
                            content: statement_source.to_string(),
                        });
                    }
                    edits.remove(start, end);
                }
            }
            Statement::ExpressionStatement(statement) => {
                if let Expression::CallExpression(call) =
                    unwrap_vue3_ts_expression(&statement.expression)
                {
                    if is_call_named(call, "defineProps") {
                        collect_vue3_define_props_call(
                            source,
                            call,
                            &mut analysis,
                            is_prod,
                            custom_element,
                        );
                        edits.remove(statement.span.start as usize, statement.span.end as usize);
                    } else if is_call_named(call, "withDefaults")
                        && collect_vue3_with_defaults_call(
                            source,
                            call,
                            &mut analysis,
                            is_prod,
                            custom_element,
                        )
                    {
                        edits.remove(statement.span.start as usize, statement.span.end as usize);
                    } else if is_call_named(call, "defineEmits") {
                        collect_vue3_define_emits_call(source, call, None, &mut analysis);
                        edits.remove(statement.span.start as usize, statement.span.end as usize);
                    } else if is_call_named(call, "defineOptions") {
                        collect_vue3_define_options_call(source, call, &mut analysis);
                        edits.remove(statement.span.start as usize, statement.span.end as usize);
                    } else if is_call_named(call, "defineSlots") {
                        collect_vue3_define_slots_call(call, None, &mut edits, &mut analysis);
                        edits.remove(statement.span.start as usize, statement.span.end as usize);
                    } else if is_call_named(call, "defineModel") {
                        collect_vue3_define_model_call(
                            source,
                            call,
                            None,
                            &mut edits,
                            &mut analysis,
                        );
                    } else if is_call_named(call, "defineExpose") {
                        collect_vue3_define_expose_call(call, &mut edits, &mut analysis);
                    }
                }
            }
            _ if is_ts && vue27_statement_is_type_hoist(statement) => {
                let (start, end) = vue27_statement_span_with_trailing_ws(source, statement);
                if let Some(statement_source) = source.get(start..end) {
                    module_chunks.push(Vue27ModuleChunk {
                        start,
                        content: statement_source.to_string(),
                    });
                }
                edits.remove(start, end);
            }
            _ => {}
        }
    }

    if !analysis.props_destructured_bindings.is_empty() {
        check_vue3_define_props_destructure_default_types(&mut analysis);
        let mut rewrite = Vue3PropsDestructureRewriter::new(
            &analysis.props_destructured_bindings,
            &analysis.vue_import_aliases,
            &mut edits,
        );
        rewrite.walk_program(&parsed.program.body);
        analysis.errors.extend(rewrite.errors);
    }

    let mut await_rewrite = Vue3TopLevelAwaitRewriter::new(source, &mut edits);
    await_rewrite.walk_program(&parsed.program.body);
    analysis.has_top_level_await = await_rewrite.has_await;
    demote_vue3_reactive_const_v_model_bindings(
        descriptor,
        &mut analysis,
        &parsed.program.body,
        &mut edits,
    );

    module_chunks.sort_by_key(|chunk| chunk.start);
    analysis.module_content = module_chunks
        .iter()
        .map(|chunk| chunk.content.as_str())
        .collect::<String>();
    analysis.setup_content = edits.apply();
    if analysis.module_content.ends_with('\n') {
        if let Some(indent) = leading_blank_line_indent(&analysis.setup_content) {
            analysis.module_content.push_str(indent);
            analysis.setup_content = analysis.setup_content[indent.len()..].to_string();
        }
    }
    analysis
}

pub(crate) fn demote_vue3_reactive_const_v_model_bindings(
    descriptor: &SfcDescriptor,
    analysis: &mut Vue3ScriptSetupAnalysis,
    statements: &[Statement<'_>],
    edits: &mut SourceEdits<'_>,
) {
    let v_model_ids = vue3_template_v_model_identifiers(descriptor);
    if v_model_ids.is_empty() {
        return;
    }
    let to_demote = v_model_ids
        .into_iter()
        .filter(|id| {
            analysis
                .setup_bindings
                .get(id)
                .is_some_and(|binding| binding == "setup-reactive-const")
        })
        .collect::<BTreeSet<_>>();
    if to_demote.is_empty() {
        return;
    }
    for statement in statements {
        let Statement::VariableDeclaration(declaration) = statement else {
            continue;
        };
        if declaration.declare || declaration.kind != VariableDeclarationKind::Const {
            continue;
        }
        let mut demoted = Vec::new();
        for declarator in &declaration.declarations {
            let BindingPattern::BindingIdentifier(identifier) = &declarator.id else {
                continue;
            };
            if to_demote.contains(identifier.name.as_str()) {
                demoted.push(identifier.name.to_string());
            }
        }
        if demoted.is_empty() {
            continue;
        }
        edits.overwrite(
            declaration.span.start as usize,
            declaration.span.start as usize + "const".len(),
            "let",
        );
        for id in demoted {
            analysis
                .setup_bindings
                .insert(id.clone(), "setup-let".into());
            analysis.demoted_reactive_bindings.insert(id.clone());
            analysis.warnings.push(format!(
                "`v-model` cannot update a `const` reactive binding `{id}`. The compiler has transformed it to `let` to make the update work."
            ));
        }
    }
}

pub(crate) fn vue3_template_v_model_identifiers(descriptor: &SfcDescriptor) -> BTreeSet<String> {
    let Some(template) = descriptor.template.as_ref() else {
        return BTreeSet::new();
    };
    if template.attrs.src.is_some() {
        return BTreeSet::new();
    }
    let mut identifiers = BTreeSet::new();
    for token in HtmlTokenizer::new(&template.content).tokenize() {
        let HtmlTokenKind::StartTag { attributes, .. } = token.kind else {
            continue;
        };
        for attribute in attributes {
            let name = attribute.name.as_str();
            if !vue3_template_is_directive_attr(name)
                || vue27_template_directive_base_name(name) != "model"
            {
                continue;
            }
            let Some(value) = attribute.value.as_deref().map(str::trim) else {
                continue;
            };
            if value != "undefined" && is_ascii_js_identifier(value) {
                identifiers.insert(value.to_string());
            }
        }
    }
    identifiers
}

pub(crate) fn vue3_variable_declaration_is_static_hoist(
    declaration: &VariableDeclaration<'_>,
) -> bool {
    declaration.kind == VariableDeclarationKind::Const
        && declaration.declarations.iter().all(|declarator| {
            matches!(declarator.id, BindingPattern::BindingIdentifier(_))
                && declarator.init.as_ref().is_some_and(vue3_is_static_node)
        })
}
