use crate::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedScriptContent {
    pub(crate) content: String,
    pub(crate) errors: Vec<String>,
    pub(crate) warnings: Vec<String>,
    pub(crate) bindings: BTreeMap<String, String>,
    pub(crate) props_aliases: BTreeMap<String, String>,
    pub(crate) imports: BTreeMap<String, SfcScriptImportBinding>,
    pub(crate) removed_bindings: BTreeSet<String>,
    pub(crate) deps: Vec<String>,
    pub(crate) map: Option<SourceMapArtifact>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Vue3InlineTemplateRender {
    pub(crate) preamble: String,
    pub(crate) code: String,
    pub(crate) ssr: bool,
    pub(crate) map: Option<SourceMapArtifact>,
    pub(crate) errors: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Vue3NormalScriptAnalysis {
    pub(crate) module_content: String,
    pub(crate) has_default_export: bool,
    pub(crate) has_default_export_name: bool,
    pub(crate) moved_after_setup: bool,
    pub(crate) errors: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Vue3ScriptSetupAnalysis {
    pub(crate) module_content: String,
    pub(crate) setup_content: String,
    pub(crate) removed_leading_import_padding: Option<String>,
    pub(crate) return_bindings: Vec<String>,
    pub(crate) imports: Vec<Vue27ScriptImport>,
    pub(crate) setup_bindings: BTreeMap<String, String>,
    pub(crate) removed_bindings: BTreeSet<String>,
    pub(crate) options_runtime: Option<String>,
    pub(crate) has_define_props: bool,
    pub(crate) has_define_options: bool,
    pub(crate) props_bindings: Vec<String>,
    pub(crate) props_runtime: Option<String>,
    pub(crate) props_type_runtime: bool,
    pub(crate) needs_merge_defaults: bool,
    pub(crate) emits_runtime: Option<String>,
    pub(crate) emit_binding: Option<String>,
    pub(crate) has_define_emits: bool,
    pub(crate) models: Vec<Vue3ModelDecl>,
    pub(crate) has_define_expose: bool,
    pub(crate) has_define_slots: bool,
    pub(crate) needs_use_slots: bool,
    pub(crate) has_top_level_await: bool,
    pub(crate) errors: Vec<String>,
    pub(crate) warnings: Vec<String>,
    pub(crate) demoted_reactive_bindings: BTreeSet<String>,
    pub(crate) local_setup_bindings: BTreeSet<String>,
    pub(crate) local_setup_binding_types: BTreeMap<String, String>,
    pub(crate) props_destructured_bindings: BTreeMap<String, String>,
    pub(crate) props_destructured_prop_order: Vec<String>,
    pub(crate) props_destructured_rest_id: Option<String>,
    pub(crate) props_destructured_defaults: BTreeMap<String, Vue3PropsDestructuredDefault>,
    pub(crate) props_destructured_default_order: Vec<String>,
    pub(crate) props_destructured_default_types: BTreeMap<String, String>,
    pub(crate) props_type_runtime_types: BTreeMap<String, Vec<String>>,
    pub(crate) deps: Vec<String>,
    pub(crate) vue_import_aliases: BTreeMap<String, String>,
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
    pub(crate) local_ts_enum_type_names: BTreeSet<String>,
    pub(crate) generic_type_parameter_names: BTreeSet<String>,
    pub(crate) type_sources: BTreeMap<String, String>,
    pub(crate) type_direct_deps: BTreeMap<String, Vec<String>>,
    pub(crate) type_deps: BTreeMap<String, BTreeSet<String>>,
    pub(crate) unresolved_import_sources: BTreeMap<String, String>,
    pub(crate) silent_unresolved_type_names: BTreeSet<String>,
    pub(crate) type_filename: Option<String>,
    pub(crate) type_seen: BTreeSet<String>,
    pub(crate) type_resolver: Vue3TypeResolverContext,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Vue3UserImports {
    pub(crate) imports: BTreeMap<String, Vue27ScriptImport>,
}

impl Vue3UserImports {
    pub(crate) fn record(&mut self, import: Vue27ScriptImport) {
        self.imports.entry(import.local.clone()).or_insert(import);
    }

    pub(crate) fn existing(&self, local: &str) -> Option<&Vue27ScriptImport> {
        self.imports.get(local)
    }

    pub(crate) fn vue_aliases(&self) -> BTreeMap<String, String> {
        self.imports
            .values()
            .filter(|import| import.source == "vue")
            .map(|import| (import.imported.clone(), import.local.clone()))
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Vue3ScriptSetupReturnBinding {
    pub(crate) name: String,
    pub(crate) kind: Vue3ScriptSetupReturnBindingKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Vue3ScriptSetupReturnBindingKind {
    Local,
    Import { source: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Vue3ModelDecl {
    pub(crate) name: String,
    pub(crate) prop_runtime: Option<String>,
    pub(crate) runtime_types: Option<Vec<String>>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Vue3DefineModelOptionsSplit {
    pub(crate) prop_option_ranges: Vec<(usize, usize)>,
    pub(crate) transformer_option_ranges: Vec<(usize, usize)>,
    pub(crate) remove_entire_call_options: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Vue3PropsDestructuredDefault {
    pub(crate) value: String,
    pub(crate) inferred_type: Option<String>,
    pub(crate) is_literal: bool,
    pub(crate) is_function: bool,
    pub(crate) is_identifier: bool,
}

pub(crate) fn vue3_resolve_type_projection(
    descriptor: &SfcDescriptor,
    options: &SfcScriptCompileOptions,
) -> Vue3ResolveTypeResult {
    let Some(script_setup) = descriptor.script_setup.as_ref() else {
        return Vue3ResolveTypeResult {
            errors: vec!["script setup block is missing".into()],
            ..Vue3ResolveTypeResult::default()
        };
    };
    let source = script_setup.content.as_str();
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
        return Vue3ResolveTypeResult {
            errors: parsed.errors.iter().map(ToString::to_string).collect(),
            ..Vue3ResolveTypeResult::default()
        };
    }

    let type_resolver = vue3_type_resolver_context_for_filename(&descriptor.filename);
    let normal_type_context =
        vue3_normal_script_type_context(descriptor, &options.global_type_files, &type_resolver);
    let normal_user_imports = vue3_normal_script_user_imports(descriptor);
    let mut type_context = normal_type_context.clone();
    extend_vue3_type_context_from_external_imports(
        &descriptor.filename,
        source,
        script_source_type_from_attrs(&script_setup.attrs),
        &mut type_context,
        &type_resolver,
    );
    let mut analysis = Vue3ScriptSetupAnalysis {
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
        type_filename: Some(descriptor.filename.clone()),
        type_resolver,
        ..Vue3ScriptSetupAnalysis::default()
    };
    collect_vue3_setup_import_aliases(&parsed.program.body, &normal_user_imports, &mut analysis);
    collect_vue3_declared_types_from_statements(source, &parsed.program.body, &mut analysis);
    collect_vue3_declared_type_deps_from_statements(&parsed.program.body, &mut analysis);

    let Some(type_argument) = vue3_first_define_props_type_argument(&parsed.program.body) else {
        return Vue3ResolveTypeResult {
            errors: vec!["defineProps() type argument is missing".into()],
            ..Vue3ResolveTypeResult::default()
        };
    };

    record_vue3_type_argument_deps(type_argument, &mut analysis);
    let calls = vue3_resolve_type_call_placeholders(source, type_argument, &analysis);
    let mut errors = Vec::new();
    let type_members = if calls.is_empty() {
        vue3_resolve_props_type_with_mode(
            source,
            type_argument,
            &analysis,
            Vue3PropsTypeResolveMode::Consumed,
        )
    } else {
        vue3_resolve_props_type(source, type_argument, &analysis)
    };
    let mut props = BTreeMap::new();
    let mut raw_props = BTreeMap::new();
    if let Some(type_members) = type_members {
        errors.extend(type_members.errors);
        for member in type_members.members {
            props.insert(member.key.clone(), member.types.clone());
            raw_props.insert(
                member.key.clone(),
                Vue3ResolveTypeRawProp {
                    types: member.types,
                    required: member.required,
                    optional: !member.required,
                    is_method: member.is_method,
                    type_annotation_source: member.type_annotation_source,
                    member_source: member.member_source,
                },
            );
        }
    }
    let raw = Vue3ResolveTypeRaw {
        props: raw_props,
        calls: calls.clone(),
    };
    Vue3ResolveTypeResult {
        props,
        calls,
        deps: analysis.deps.iter().cloned().collect(),
        raw,
        errors,
    }
}

pub(crate) fn vue3_first_define_props_type_argument<'a>(
    statements: &'a [Statement<'a>],
) -> Option<&'a TSType<'a>> {
    for statement in statements {
        let Statement::ExpressionStatement(statement) = statement else {
            continue;
        };
        let Expression::CallExpression(call) = unwrap_vue3_ts_expression(&statement.expression)
        else {
            continue;
        };
        if !is_call_named(call, "defineProps") {
            continue;
        }
        return call
            .type_arguments
            .as_ref()
            .and_then(|arguments| arguments.params.first());
    }
    None
}

pub(crate) fn vue3_resolve_type_call_placeholders(
    source: &str,
    type_argument: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Vec<Value> {
    let count = vue3_resolve_type_call_count(source, type_argument, analysis);
    (0..count).map(|_| json!({})).collect()
}

pub(crate) fn vue3_resolve_type_call_count(
    source: &str,
    type_argument: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> usize {
    match type_argument {
        TSType::TSFunctionType(_) => 1,
        TSType::TSTypeLiteral(literal) => literal
            .members
            .iter()
            .filter(|member| matches!(member, TSSignature::TSCallSignatureDeclaration(_)))
            .count(),
        TSType::TSTypeReference(_)
        | TSType::TSImportType(_)
        | TSType::TSIntersectionType(_)
        | TSType::TSParenthesizedType(_) => {
            vue3_resolve_emits_type(source, type_argument, analysis)
                .filter(|emits| emits.syntax.has_call_signature)
                .map(|emits| emits.call_count.max(1))
                .unwrap_or_default()
        }
        _ => 0,
    }
}

pub(crate) fn script_content(
    context: &mut Vue3ScriptCompileContext<'_>,
    options: &SfcScriptCompileOptions,
    base_bindings: &BTreeMap<String, String>,
    script_errors: &[String],
    template_usage_index: Option<&TemplateUsageIndex>,
) -> GeneratedScriptContent {
    {
        let descriptor = context.descriptor();
        let raw_content = context.raw_content();
        let Some(script_setup) = descriptor.script_setup.as_ref() else {
            let content =
                vue3_normal_script_content(descriptor, raw_content, options, base_bindings);
            return GeneratedScriptContent {
                map: options
                    .source_map
                    .then(|| vue3_compile_script_source_map(descriptor, &content, None)),
                content,
                errors: script_errors.to_vec(),
                warnings: Vec::new(),
                bindings: BTreeMap::new(),
                props_aliases: BTreeMap::new(),
                imports: BTreeMap::new(),
                removed_bindings: BTreeSet::new(),
                deps: Vec::new(),
            };
        };
        if !script_lang_is_js_like(&script_setup.attrs) && script_errors.is_empty() {
            return GeneratedScriptContent {
                map: options
                    .source_map
                    .then(|| vue3_compile_script_source_map(descriptor, raw_content, None)),
                content: raw_content.to_string(),
                errors: Vec::new(),
                warnings: Vec::new(),
                bindings: BTreeMap::new(),
                props_aliases: BTreeMap::new(),
                imports: BTreeMap::new(),
                removed_bindings: BTreeSet::new(),
                deps: Vec::new(),
            };
        }
        if !script_errors.is_empty() {
            return GeneratedScriptContent {
                map: options
                    .source_map
                    .then(|| vue3_compile_script_source_map(descriptor, raw_content, None)),
                content: raw_content.to_string(),
                errors: script_errors.to_vec(),
                warnings: Vec::new(),
                bindings: BTreeMap::new(),
                props_aliases: BTreeMap::new(),
                imports: BTreeMap::new(),
                removed_bindings: BTreeSet::new(),
                deps: Vec::new(),
            };
        }
    }

    let setup_analysis = context.script_setup_analysis();
    let normal_script = context.normal_script_analysis();
    let normal_script_return_bindings = context.normal_script_return_bindings();
    let script_binding_metadata = context.script_binding_metadata(&setup_analysis);
    let descriptor = context.descriptor();
    let filename = context.filename();
    let script_setup = descriptor
        .script_setup
        .as_ref()
        .expect("vue 3 script setup block");
    let is_ts = script_is_typescript(&script_setup.attrs)
        || descriptor
            .script
            .as_ref()
            .is_some_and(|script| script_is_typescript(&script.attrs));
    let return_bindings = vue3_script_setup_return_bindings(
        descriptor,
        &normal_script_return_bindings,
        &setup_analysis,
        is_ts,
        template_usage_index,
    );
    let template_binding_metadata = vue3_script_setup_template_binding_metadata(
        &normal_script_return_bindings,
        base_bindings,
        &script_binding_metadata,
        &setup_analysis,
    );
    let imports = vue3_script_setup_import_metadata(
        descriptor,
        &normal_script_return_bindings,
        &setup_analysis,
        is_ts,
        options.inline_template,
        template_usage_index,
    );
    let template_props_aliases = vue3_script_setup_template_props_aliases(&setup_analysis);
    let public_props_aliases = vue3_script_setup_public_props_aliases(&setup_analysis);
    let inline_render = vue3_inline_template_render(
        descriptor,
        options,
        &template_binding_metadata,
        &template_props_aliases,
        is_ts,
    );
    let css_vars_code = vue3_script_setup_css_vars_code(
        descriptor,
        options,
        &template_binding_metadata,
        &template_props_aliases,
    );
    let mut content = String::new();
    let has_helper_import = if let Some(import) = vue3_script_setup_helper_import(
        &setup_analysis,
        options,
        is_ts,
        css_vars_code.is_some(),
        inline_render
            .as_ref()
            .is_some_and(|render| render.preamble.contains("unref as _unref")),
    ) {
        append_vue3_module_chunk(&mut content, &import);
        true
    } else {
        false
    };
    if let Some(render) = inline_render.as_ref() {
        append_vue3_module_chunk(&mut content, &render.preamble);
        if !render.preamble.is_empty()
            && (!setup_analysis.module_content.is_empty()
                || !normal_script.module_content.is_empty())
            && !content.ends_with("\n\n")
        {
            content.push_str("\n\n");
        }
    }
    append_vue3_module_chunk(&mut content, &setup_analysis.module_content);
    if content.is_empty() && !normal_script.module_content.is_empty() {
        if setup_analysis.removed_leading_import_padding.is_some() {
            if let Some(padding) = vue3_trailing_blank_line_padding(&normal_script.module_content)
                .or(setup_analysis.removed_leading_import_padding.as_deref())
            {
                content.push_str(padding);
            }
        }
    }
    append_vue3_module_chunk(&mut content, &normal_script.module_content);
    if normal_script.module_content.is_empty()
        && setup_analysis.module_content.is_empty()
        && setup_analysis.setup_content.starts_with('\n')
    {
        if content.is_empty() {
            content.push('\n');
        } else if inline_render
            .as_ref()
            .is_some_and(|render| !render.preamble.is_empty())
        {
            content.push_str("\n\n\n");
        } else if inline_render.is_some() {
            content.push_str("\n\n");
        } else {
            content.push_str("\n\n");
        }
    }
    let moved_normal_script_had_pending_blank =
        normal_script.moved_after_setup && output_has_pending_blank_line(&content);
    if !content.is_empty()
        && !content.trim().is_empty()
        && inline_render.is_none()
        && (vue3_script_setup_needs_blank_before_export(&setup_analysis)
            || (has_helper_import
                && setup_analysis.module_content.is_empty()
                && normal_script.module_content.is_empty()))
    {
        ensure_vue3_blank_line_before_export(&mut content);
    }
    if normal_script.moved_after_setup
        && inline_render.is_none()
        && !normal_script.module_content.is_empty()
        && (normal_script.has_default_export || moved_normal_script_had_pending_blank)
    {
        ensure_vue3_moved_normal_script_gap_before_export(&mut content);
    }
    if content.is_empty()
        && setup_analysis.module_content.is_empty()
        && normal_script.module_content.is_empty()
        && !setup_analysis.setup_content.starts_with('\n')
        && descriptor.script.is_none()
    {
        content.push('\n');
    }
    let export = vue3_script_setup_export(
        &setup_analysis,
        &return_bindings,
        &script_binding_metadata,
        filename,
        &normal_script,
        is_ts,
        options.is_prod,
        inline_render.as_ref(),
        css_vars_code.as_deref(),
        options.emit_script_setup_marker,
        options.gen_default_as.as_deref(),
    );
    append_vue3_export_chunk(&mut content, &export);
    let mut bindings = BTreeMap::new();
    for import in normal_script_return_bindings
        .imports
        .iter()
        .chain(setup_analysis.imports.iter())
    {
        if !import.is_type {
            bindings.insert(
                import.local.clone(),
                vue3_script_import_binding_type(import).into(),
            );
        }
    }
    bindings.extend(script_binding_metadata);
    bindings.extend(setup_analysis.setup_bindings.clone());
    for prop in &setup_analysis.props_bindings {
        bindings
            .entry(prop.clone())
            .or_insert_with(|| "props".into());
    }
    let mut errors = normal_script.errors;
    errors.extend(setup_analysis.errors);
    if let Some(render) = inline_render.as_ref() {
        errors.extend(render.errors.clone());
    }
    let content = trim_trailing_blank_lines(&content).to_string();
    let map = options
        .source_map
        .then(|| vue3_compile_script_source_map(descriptor, &content, inline_render.as_ref()));
    GeneratedScriptContent {
        content,
        errors,
        warnings: setup_analysis.warnings,
        bindings,
        props_aliases: public_props_aliases,
        imports,
        removed_bindings: setup_analysis.removed_bindings,
        deps: setup_analysis.deps.iter().cloned().collect(),
        map,
    }
}

pub(crate) fn vue3_script_setup_template_binding_metadata(
    normal_script_return_bindings: &Vue27ScriptReturnBindings,
    base_bindings: &BTreeMap<String, String>,
    script_bindings: &BTreeMap<String, String>,
    setup_analysis: &Vue3ScriptSetupAnalysis,
) -> BTreeMap<String, String> {
    let mut bindings = base_bindings.clone();
    for import in normal_script_return_bindings
        .imports
        .iter()
        .chain(setup_analysis.imports.iter())
    {
        if !import.is_type {
            bindings.insert(
                import.local.clone(),
                vue3_script_import_binding_type(import).into(),
            );
        }
    }
    bindings.extend(script_bindings.clone());
    bindings.extend(setup_analysis.setup_bindings.clone());
    for prop in &setup_analysis.props_bindings {
        bindings
            .entry(prop.clone())
            .or_insert_with(|| "props".into());
    }
    for removed in &setup_analysis.removed_bindings {
        bindings.remove(removed);
    }
    bindings
}

pub(crate) fn vue3_compile_script_source_map(
    descriptor: &SfcDescriptor,
    generated: &str,
    inline_render: Option<&Vue3InlineTemplateRender>,
) -> SourceMapArtifact {
    let source_name = descriptor.filename.replace('\\', "/");
    let mut builder = SourceMapBuilder::new().file(source_name.clone());
    builder.add_source_content(source_name.clone(), descriptor.source.clone());
    match (descriptor.script.as_ref(), descriptor.script_setup.as_ref()) {
        (Some(script), Some(script_setup)) if script_setup.content_start < script.content_start => {
            add_script_block_source_mappings(
                &mut builder,
                descriptor,
                script_setup,
                generated,
                &source_name,
            );
            add_script_block_source_mappings(
                &mut builder,
                descriptor,
                script,
                generated,
                &source_name,
            );
        }
        (script, script_setup) => {
            if let Some(script) = script {
                add_script_block_source_mappings(
                    &mut builder,
                    descriptor,
                    script,
                    generated,
                    &source_name,
                );
            }
            if let Some(script_setup) = script_setup {
                add_script_block_source_mappings(
                    &mut builder,
                    descriptor,
                    script_setup,
                    generated,
                    &source_name,
                );
            }
        }
    }
    if let Some(render) = inline_render {
        add_inline_template_source_mappings(
            &mut builder,
            descriptor,
            render,
            generated,
            &source_name,
        );
    }
    builder.build()
}

pub(crate) fn add_script_block_source_mappings(
    builder: &mut SourceMapBuilder,
    descriptor: &SfcDescriptor,
    block: &SfcBlock,
    generated: &str,
    source_name: &str,
) {
    let mut generated_cursor = 0usize;
    let mut source_line_start = 0usize;
    for line in block.content.split_inclusive('\n') {
        let line_without_newline = line.trim_end_matches(['\r', '\n']);
        let trimmed = line_without_newline.trim();
        if !trimmed.is_empty() {
            let leading = line_without_newline.find(trimmed).unwrap_or(0);
            if let Some(relative_generated_start) = generated[generated_cursor..].find(trimmed) {
                let generated_start = generated_cursor + relative_generated_start;
                for (char_offset, ch) in trimmed.char_indices() {
                    if ch.is_whitespace() {
                        continue;
                    }
                    let generated_offset = generated_start + char_offset;
                    let source_offset =
                        block.content_start + source_line_start + leading + char_offset;
                    if let Some((generated_line, generated_column)) =
                        utf16_line_column_for_byte_offset(generated, generated_offset)
                    {
                        builder.add_mapping(
                            generated_line,
                            generated_column,
                            Some(Span::new(
                                descriptor.source_file,
                                source_offset,
                                source_offset,
                            )),
                            Some(source_name.to_string()),
                        );
                    }
                }
                generated_cursor = generated_start + trimmed.len();
            }
        }
        source_line_start += line.len();
    }
}

pub(crate) fn add_inline_template_source_mappings(
    builder: &mut SourceMapBuilder,
    descriptor: &SfcDescriptor,
    render: &Vue3InlineTemplateRender,
    generated: &str,
    source_name: &str,
) {
    let Some(render_map) = render.map.as_ref() else {
        return;
    };
    let Some(render_start) = generated.find(&render.code) else {
        return;
    };
    let Some((render_start_line, render_start_column)) =
        utf16_zero_based_line_column_for_byte_offset(generated, render_start)
    else {
        return;
    };
    let Ok(source_map) = render_map.to_oxc_source_map() else {
        return;
    };
    for token in source_map.get_tokens() {
        let generated_line = render_start_line + token.get_dst_line() as usize;
        let generated_column = if token.get_dst_line() == 0 {
            render_start_column + token.get_dst_col() as usize
        } else {
            token.get_dst_col() as usize
        };
        let Some(absolute) = byte_offset_at_utf16_line_column(
            &descriptor.source,
            token.get_src_line() as usize + 1,
            token.get_src_col() as usize,
        ) else {
            continue;
        };
        let name = token
            .get_name_id()
            .and_then(|name_id| source_map.get_name(name_id).map(ToString::to_string));
        builder.add_named_mapping(
            generated_line + 1,
            generated_column,
            Some(Span::new(descriptor.source_file, absolute, absolute)),
            Some(source_name.to_string()),
            name,
        );
    }
}

pub(crate) fn utf16_line_column_for_byte_offset(
    source: &str,
    offset: usize,
) -> Option<(usize, usize)> {
    utf16_zero_based_line_column_for_byte_offset(source, offset)
        .map(|(line, column)| (line + 1, column))
}

pub(crate) fn utf16_zero_based_line_column_for_byte_offset(
    source: &str,
    offset: usize,
) -> Option<(usize, usize)> {
    if offset > source.len() || !source.is_char_boundary(offset) {
        return None;
    }
    let mut line = 0usize;
    let mut column = 0usize;
    for ch in source[..offset].chars() {
        if ch == '\n' {
            line += 1;
            column = 0;
        } else {
            column += ch.len_utf16();
        }
    }
    Some((line, column))
}

pub(crate) fn vue3_inline_template_render(
    descriptor: &SfcDescriptor,
    options: &SfcScriptCompileOptions,
    binding_metadata: &BTreeMap<String, String>,
    props_aliases: &BTreeMap<String, String>,
    is_ts: bool,
) -> Option<Vue3InlineTemplateRender> {
    if !options.inline_template {
        return None;
    }
    let Some(template) = descriptor.template.as_ref() else {
        return Some(Vue3InlineTemplateRender {
            preamble: String::new(),
            code: "() => {}".into(),
            ssr: false,
            map: None,
            errors: Vec::new(),
        });
    };
    if template.attrs.src.is_some() {
        return Some(Vue3InlineTemplateRender {
            preamble: String::new(),
            code: "() => {}".into(),
            ssr: false,
            map: None,
            errors: Vec::new(),
        });
    }

    let scoped = descriptor.styles.iter().any(|style| style.attrs.scoped);
    let scope_id = scoped
        .then(|| vue3_compile_script_scope_id(options.id.as_deref()))
        .flatten();
    let mut core = Vue3CompilerOptions {
        prefix_identifiers: true,
        mode: "module".into(),
        hoist_static: true,
        cache_handlers: true,
        scope_id: scope_id.clone(),
        is_ts,
        source_map: options.source_map,
        source_map_source: options.source_map.then(|| descriptor.source.clone()),
        source_map_base_offset: 0,
        binding_metadata: binding_metadata.clone(),
        props_aliases: props_aliases.clone(),
        inline: true,
        ..Vue3CompilerOptions::default()
    };
    apply_dom_parser_defaults(&mut core);
    let template_source = TemplateSource {
        filename: descriptor.filename.clone(),
        source: template.content.clone(),
        file_id: descriptor.source_file,
        base_offset: template.content_start,
    };
    if options.inline_template_ssr {
        core.ssr_css_vars = vue3_inline_ssr_css_vars(descriptor, options);
        core.source_map = options.source_map;
        let result = compile_ssr(
            template_source,
            SsrCompilerOptions {
                core,
                scope_id,
                slotted: vue3_sfc_descriptor_has_slotted_styles(descriptor),
                slotted_is_explicit: true,
                mode_is_explicit: true,
                transform_asset_urls: true,
                asset_url_options: AssetUrlOptions::default(),
            },
        );
        let errors = result
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == Severity::Error)
            .map(|diagnostic| format!("{:?}: {}", diagnostic.code, diagnostic.message))
            .collect();
        return Some(Vue3InlineTemplateRender {
            preamble: result.preamble,
            code: result.code,
            ssr: true,
            map: result.map,
            errors,
        });
    }
    let result = compile_dom(
        template_source,
        DomCompilerOptions {
            core,
            transform_asset_urls: true,
            asset_url_options: AssetUrlOptions::default(),
            ..DomCompilerOptions::default()
        },
    );
    let errors = result
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .map(|diagnostic| format!("{:?}: {}", diagnostic.code, diagnostic.message))
        .collect();
    Some(Vue3InlineTemplateRender {
        preamble: result.preamble,
        code: result.code,
        ssr: false,
        map: result.map,
        errors,
    })
}

pub(crate) fn vue3_compile_script_scope_id(id: Option<&str>) -> Option<String> {
    id.map(|id| {
        if id.starts_with("data-v-") {
            id.to_string()
        } else {
            format!("data-v-{id}")
        }
    })
}

pub(crate) fn vue3_compile_script_short_id(id: Option<&str>) -> String {
    id.and_then(|id| id.strip_prefix("data-v-").or(Some(id)))
        .unwrap_or("")
        .to_string()
}

pub(crate) fn vue3_inline_ssr_css_vars(
    descriptor: &SfcDescriptor,
    options: &SfcScriptCompileOptions,
) -> Option<String> {
    let vars = vue3_css_vars(descriptor);
    if vars.is_empty() {
        return None;
    }
    let id = vue3_compile_script_short_id(options.id.as_deref());
    let entries = vars
        .iter()
        .map(|var| {
            let name = format!(
                ":--{}",
                gen_css_var_name_with_style(
                    &id,
                    var,
                    options.is_prod,
                    CssVarNameStyle::Vue3Escaped
                )
            );
            format!("\"{}\": ({})", escape_js_double(&name), var)
        })
        .collect::<Vec<_>>()
        .join(",\n  ");
    Some(format!("{{\n  {entries}\n}}"))
}

pub(crate) fn vue3_normal_script_content(
    descriptor: &SfcDescriptor,
    raw_content: &str,
    options: &SfcScriptCompileOptions,
    _base_bindings: &BTreeMap<String, String>,
) -> String {
    let Some(script) = descriptor.script.as_ref() else {
        return raw_content.to_string();
    };
    if !script_lang_is_js_like(&script.attrs) {
        return raw_content.to_string();
    }
    let css_vars = vue3_css_vars(descriptor);
    if css_vars.is_empty() && options.gen_default_as.is_none() {
        return raw_content.to_string();
    }

    let source = script.content.as_str();
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        source,
        script_source_type_from_attrs(&script.attrs),
    )
    .with_options(oxc_parser::ParseOptions {
        parse_regular_expression: true,
        ..oxc_parser::ParseOptions::default()
    })
    .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return raw_content.to_string();
    }

    let mut edits = SourceEdits::new(source);
    let default_export_name = options.gen_default_as.as_deref().unwrap_or("__default__");
    let mut has_default_export = false;
    for statement in &parsed.program.body {
        match statement {
            Statement::ExportDefaultDeclaration(declaration) => {
                has_default_export = true;
                rewrite_vue3_export_default(default_export_name, declaration, &mut edits);
            }
            Statement::ExportNamedDeclaration(declaration) => {
                if rewrite_vue3_compile_script_named_default_export(
                    source,
                    default_export_name,
                    declaration,
                    &mut edits,
                ) {
                    has_default_export = true;
                }
            }
            _ => {}
        }
    }
    if !has_default_export {
        edits.append(format!("\nconst {default_export_name} = {{}}"));
    }

    let content = trim_trailing_blank_lines(&edits.apply()).to_string();
    if css_vars.is_empty() || options.inline_template_ssr {
        return content;
    }
    let css_vars_code = vue3_normal_script_css_vars_code(&css_vars, options, default_export_name);
    if options.gen_default_as.is_some() {
        format!("{content}{css_vars_code}")
    } else {
        format!("{content}{css_vars_code}\nexport default __default__")
    }
}

pub(crate) fn vue3_normal_script_css_vars_code(
    css_vars: &[String],
    options: &SfcScriptCompileOptions,
    default_export_name: &str,
) -> String {
    format!(
        "\nimport {{ useCssVars as _useCssVars }} from {}\nconst __injectCSSVars__ = () => {{\n{}}}\nconst __setup__ = {default_export_name}.setup\n{default_export_name}.setup = __setup__\n  ? (props, ctx) => {{ __injectCSSVars__();return __setup__(props, ctx) }}\n  : __injectCSSVars__\n",
        vue3_script_setup_helper_import_source(options),
        vue3_css_vars_code(css_vars, options, &BTreeMap::new(), &BTreeMap::new())
    )
}

pub(crate) fn vue3_script_setup_css_vars_code(
    descriptor: &SfcDescriptor,
    options: &SfcScriptCompileOptions,
    binding_metadata: &BTreeMap<String, String>,
    props_aliases: &BTreeMap<String, String>,
) -> Option<String> {
    if options.inline_template_ssr {
        return None;
    }
    let css_vars = vue3_css_vars(descriptor);
    if css_vars.is_empty() {
        return None;
    }
    Some(vue3_css_vars_code(
        &css_vars,
        options,
        binding_metadata,
        props_aliases,
    ))
}

pub(crate) fn vue3_css_vars_code(
    css_vars: &[String],
    options: &SfcScriptCompileOptions,
    binding_metadata: &BTreeMap<String, String>,
    props_aliases: &BTreeMap<String, String>,
) -> String {
    let id = vue3_compile_script_short_id(options.id.as_deref());
    let vars = css_vars
        .iter()
        .map(|var| {
            let name = gen_css_var_name_with_style(
                &id,
                var,
                options.is_prod,
                CssVarNameStyle::Vue3Escaped,
            );
            format!("\"{}\": ({})", name, var)
        })
        .collect::<Vec<_>>()
        .join(",\n  ");
    let expression = format!("{{\n  {vars}\n}}");
    let prefixed = vue3_css_vars_expression_code(&expression, binding_metadata, props_aliases);
    format!("_useCssVars(_ctx => ({prefixed}))")
}

pub(crate) fn vue3_css_vars_expression_code(
    expression: &str,
    binding_metadata: &BTreeMap<String, String>,
    props_aliases: &BTreeMap<String, String>,
) -> String {
    let mut metadata = serde_json::Map::new();
    for (name, kind) in binding_metadata {
        metadata.insert(name.clone(), json!(kind));
    }
    if !props_aliases.is_empty() {
        metadata.insert("__propsAliases".to_string(), json!(props_aliases));
    }
    let projection = process_expression_projection(&json!({
        "node": {
            "type": 4,
            "content": expression,
            "isStatic": false,
            "loc": {
                "start": { "offset": 0, "line": 1, "column": 1 },
                "end": { "offset": expression.len(), "line": 1, "column": expression.len() + 1 },
                "source": expression,
            }
        },
        "context": {
            "prefixIdentifiers": true,
            "inline": true,
            "isTS": false,
            "identifiers": {},
            "bindingMetadata": metadata,
        }
    }));
    vue3_projection_code(&projection).unwrap_or_else(|| expression.to_string())
}

pub(crate) fn vue3_projection_code(value: &Value) -> Option<String> {
    match value.get("kind").and_then(Value::as_str) {
        Some("simple") => value
            .get("content")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        Some("compound") => {
            let children = value.get("children")?.as_array()?;
            let mut code = String::new();
            for child in children {
                if let Some(source) = child.as_str() {
                    code.push_str(source);
                } else if let Some(source) = vue3_projection_code(child) {
                    code.push_str(&source);
                }
            }
            Some(code)
        }
        _ => value.as_str().map(ToOwned::to_owned),
    }
}

pub(crate) fn vue3_script_setup_template_props_aliases(
    setup_analysis: &Vue3ScriptSetupAnalysis,
) -> BTreeMap<String, String> {
    setup_analysis.props_destructured_bindings.clone()
}

pub(crate) fn vue3_script_setup_public_props_aliases(
    setup_analysis: &Vue3ScriptSetupAnalysis,
) -> BTreeMap<String, String> {
    setup_analysis
        .props_destructured_bindings
        .iter()
        .filter(|(local, public_key)| *local != *public_key)
        .map(|(local, public_key)| (local.clone(), public_key.clone()))
        .collect()
}

pub(crate) fn vue3_script_setup_helper_import(
    setup_analysis: &Vue3ScriptSetupAnalysis,
    options: &SfcScriptCompileOptions,
    is_ts: bool,
    needs_css_vars: bool,
    inline_render_has_unref: bool,
) -> Option<String> {
    let mut helpers = Vec::new();
    if needs_css_vars {
        helpers.push("useCssVars as _useCssVars");
        if !inline_render_has_unref {
            helpers.push("unref as _unref");
        }
    }
    if setup_analysis.has_top_level_await {
        helpers.push("withAsyncContext as _withAsyncContext");
    }
    if !setup_analysis.models.is_empty() {
        helpers.push("useModel as _useModel");
    }
    if setup_analysis.needs_use_slots {
        helpers.push("useSlots as _useSlots");
    }
    if setup_analysis.needs_merge_defaults {
        helpers.push("mergeDefaults as _mergeDefaults");
    }
    if setup_analysis.props_destructured_rest_id.is_some() {
        helpers.push("createPropsRestProxy as _createPropsRestProxy");
    }
    if vue3_script_setup_needs_merge_models(setup_analysis) {
        helpers.push("mergeModels as _mergeModels");
    }
    if is_ts {
        helpers.push("defineComponent as _defineComponent");
    }
    if helpers.is_empty() {
        None
    } else {
        Some(format!(
            "import {{ {} }} from {}\n",
            helpers.join(", "),
            vue3_script_setup_helper_import_source(options)
        ))
    }
}

pub(crate) fn vue3_script_setup_helper_import_source(options: &SfcScriptCompileOptions) -> String {
    options
        .runtime_module_name
        .as_ref()
        .map(|source| format!("\"{}\"", escape_js_double(source)))
        .unwrap_or_else(|| "'vue'".to_string())
}

pub(crate) fn vue3_script_setup_return_bindings(
    descriptor: &SfcDescriptor,
    normal_script_return_bindings: &Vue27ScriptReturnBindings,
    setup_analysis: &Vue3ScriptSetupAnalysis,
    is_ts: bool,
    template_usage_index: Option<&TemplateUsageIndex>,
) -> Vec<Vue3ScriptSetupReturnBinding> {
    let mut bindings = Vec::new();
    for binding in &normal_script_return_bindings.bindings {
        push_unique_vue3_return_binding(
            &mut bindings,
            Vue3ScriptSetupReturnBinding {
                name: binding.clone(),
                kind: Vue3ScriptSetupReturnBindingKind::Local,
            },
        );
    }
    for binding in &setup_analysis.return_bindings {
        push_unique_vue3_return_binding(
            &mut bindings,
            Vue3ScriptSetupReturnBinding {
                name: binding.clone(),
                kind: Vue3ScriptSetupReturnBindingKind::Local,
            },
        );
    }
    for import in normal_script_return_bindings
        .imports
        .iter()
        .chain(setup_analysis.imports.iter())
    {
        if import.is_type {
            continue;
        }
        if vue3_script_setup_import_is_returned(descriptor, import, is_ts, template_usage_index) {
            push_unique_vue3_return_binding(
                &mut bindings,
                Vue3ScriptSetupReturnBinding {
                    name: import.local.clone(),
                    kind: Vue3ScriptSetupReturnBindingKind::Import {
                        source: import.source.clone(),
                    },
                },
            );
        }
    }
    bindings
}

pub(crate) fn vue3_script_setup_import_metadata(
    descriptor: &SfcDescriptor,
    normal_script_return_bindings: &Vue27ScriptReturnBindings,
    setup_analysis: &Vue3ScriptSetupAnalysis,
    is_ts: bool,
    inline_template: bool,
    template_usage_index: Option<&TemplateUsageIndex>,
) -> BTreeMap<String, SfcScriptImportBinding> {
    let mut imports = BTreeMap::new();
    for import in &normal_script_return_bindings.imports {
        vue3_insert_script_import_metadata(
            &mut imports,
            descriptor,
            import,
            false,
            is_ts,
            inline_template,
            template_usage_index,
        );
    }
    for import in &setup_analysis.imports {
        vue3_insert_script_import_metadata(
            &mut imports,
            descriptor,
            import,
            true,
            is_ts,
            inline_template,
            template_usage_index,
        );
    }
    imports
}

pub(crate) fn vue3_insert_script_import_metadata(
    imports: &mut BTreeMap<String, SfcScriptImportBinding>,
    descriptor: &SfcDescriptor,
    import: &Vue27ScriptImport,
    is_from_setup: bool,
    is_ts: bool,
    inline_template: bool,
    template_usage_index: Option<&TemplateUsageIndex>,
) {
    imports.entry(import.local.clone()).or_insert_with(|| {
        let is_used_in_template = vue3_script_import_is_used_in_template(
            descriptor,
            &import.local,
            is_ts,
            inline_template,
            template_usage_index,
        );
        SfcScriptImportBinding {
            is_type: import.is_type,
            imported: import.imported.clone(),
            local: import.local.clone(),
            source: import.source.clone(),
            is_from_setup,
            is_used_in_template,
        }
    });
}

pub(crate) fn vue3_script_import_is_used_in_template(
    descriptor: &SfcDescriptor,
    local: &str,
    is_ts: bool,
    inline_template: bool,
    template_usage_index: Option<&TemplateUsageIndex>,
) -> bool {
    if inline_template {
        return false;
    }
    if !is_ts {
        return true;
    }
    let Some(template) = descriptor.template.as_ref() else {
        return true;
    };
    if template.attrs.src.is_some() || template.attrs.lang.is_some() {
        return true;
    }
    template_usage_index
        .map(|index| index.contains(local))
        .unwrap_or_else(|| vue3_template_uses_identifier(&template.content, local, is_ts))
}

pub(crate) fn push_unique_vue3_return_binding(
    bindings: &mut Vec<Vue3ScriptSetupReturnBinding>,
    binding: Vue3ScriptSetupReturnBinding,
) {
    if bindings
        .iter()
        .any(|existing| existing.name == binding.name)
    {
        return;
    }
    bindings.push(binding);
}

pub(crate) fn vue3_script_setup_import_is_returned(
    descriptor: &SfcDescriptor,
    import: &Vue27ScriptImport,
    is_ts: bool,
    template_usage_index: Option<&TemplateUsageIndex>,
) -> bool {
    if import.source == "vue" {
        return true;
    }
    let Some(template) = descriptor.template.as_ref() else {
        return true;
    };
    if template.attrs.src.is_some() || template.attrs.lang.is_some() {
        return true;
    }
    template_usage_index
        .map(|index| index.contains(&import.local))
        .unwrap_or_else(|| vue3_template_uses_identifier(&template.content, &import.local, is_ts))
}

pub(crate) fn vue3_script_import_binding_type(import: &Vue27ScriptImport) -> &'static str {
    if import.imported == "*"
        || (import.imported == "default" && import.source.ends_with(".vue"))
        || import.source == "vue"
    {
        "setup-const"
    } else {
        "setup-maybe-ref"
    }
}

pub(crate) fn vue3_script_block_return_bindings(block: &SfcBlock) -> Vue27ScriptReturnBindings {
    if !script_lang_is_js_like(&block.attrs) {
        return Vue27ScriptReturnBindings::default();
    }
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        &block.content,
        script_source_type_from_attrs(&block.attrs),
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

pub(crate) fn vue3_script_compile_errors(
    descriptor: &SfcDescriptor,
    options: &SfcScriptCompileOptions,
) -> Vec<String> {
    let mut errors = Vec::new();
    let Some(script_setup) = descriptor.script_setup.as_ref() else {
        if let Some(script) = descriptor.script.as_ref() {
            errors.extend(vue3_deprecated_import_assert_syntax_errors(
                script,
                options.allow_deprecated_import_assert_syntax,
            ));
        }
        return errors;
    };
    if descriptor
        .script
        .as_ref()
        .is_some_and(|script| script.attrs.lang != script_setup.attrs.lang)
    {
        return vec!["<script> and <script setup> must have the same language type.".to_string()];
    }
    if !script_lang_is_js_like(&script_setup.attrs) {
        return errors;
    }
    if let Some(script) = descriptor.script.as_ref() {
        errors.extend(vue3_deprecated_import_assert_syntax_errors(
            script,
            options.allow_deprecated_import_assert_syntax,
        ));
    }
    errors.extend(vue3_deprecated_import_assert_syntax_errors(
        script_setup,
        options.allow_deprecated_import_assert_syntax,
    ));
    errors.extend(vue3_script_setup_module_export_errors(script_setup));
    errors
}

pub(crate) fn vue3_deprecated_import_assert_syntax_errors(
    block: &SfcBlock,
    allow_deprecated_import_assert_syntax: bool,
) -> Vec<String> {
    if allow_deprecated_import_assert_syntax || !script_lang_is_js_like(&block.attrs) {
        return Vec::new();
    }
    let source = block.content.as_str();
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        source,
        script_source_type_from_attrs(&block.attrs),
    )
    .with_options(oxc_parser::ParseOptions {
        parse_regular_expression: true,
        ..oxc_parser::ParseOptions::default()
    })
    .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return Vec::new();
    }
    parsed
        .program
        .body
        .iter()
        .filter_map(|statement| match statement {
            Statement::ImportDeclaration(declaration)
                if declaration
                    .with_clause
                    .as_ref()
                    .is_some_and(|clause| clause.keyword == WithClauseKeyword::Assert) =>
            {
                Some("The `assert` keyword in import attributes is deprecated. Use `with` instead, or enable the importAttributes parser plugin with deprecatedAssertSyntax.".to_string())
            }
            Statement::ExportNamedDeclaration(declaration)
                if declaration
                    .with_clause
                    .as_ref()
                    .is_some_and(|clause| clause.keyword == WithClauseKeyword::Assert) =>
            {
                Some("The `assert` keyword in export attributes is deprecated. Use `with` instead, or enable the importAttributes parser plugin with deprecatedAssertSyntax.".to_string())
            }
            Statement::ExportAllDeclaration(declaration)
                if declaration
                    .with_clause
                    .as_ref()
                    .is_some_and(|clause| clause.keyword == WithClauseKeyword::Assert) =>
            {
                Some("The `assert` keyword in export attributes is deprecated. Use `with` instead, or enable the importAttributes parser plugin with deprecatedAssertSyntax.".to_string())
            }
            _ => None,
        })
        .collect()
}

pub(crate) fn vue3_script_setup_module_export_errors(script_setup: &SfcBlock) -> Vec<String> {
    let source = script_setup.content.as_str();
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
        return Vec::new();
    }
    parsed
        .program
        .body
        .iter()
        .filter_map(|statement| match statement {
            Statement::ExportNamedDeclaration(declaration)
                if declaration.export_kind != ImportOrExportKind::Type =>
            {
                Some(vue27_script_setup_module_export_error())
            }
            Statement::ExportAllDeclaration(declaration)
                if declaration.export_kind != ImportOrExportKind::Type =>
            {
                Some(vue27_script_setup_module_export_error())
            }
            Statement::ExportDefaultDeclaration(_) => {
                Some(vue27_script_setup_module_export_error())
            }
            _ => None,
        })
        .collect()
}

pub(crate) fn analyze_vue3_script_setup(
    filename: &str,
    descriptor: &SfcDescriptor,
    script_setup: &SfcBlock,
    hoist_static_literals: bool,
    normal_type_context: &Vue27TypeContext,
    normal_user_imports: &Vue3UserImports,
    type_resolver: &Vue3TypeResolverContext,
    props_destructure: SfcPropsDestructureMode,
    is_prod: bool,
    custom_element: bool,
) -> Vue3ScriptSetupAnalysis {
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

pub(crate) fn analyze_vue3_setup_variable_declaration(
    source: &str,
    declaration: &VariableDeclaration<'_>,
    edits: &mut SourceEdits<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
    props_destructure: SfcPropsDestructureMode,
    is_prod: bool,
    custom_element: bool,
    literal_const_enabled: bool,
) {
    let mut macro_declarators = Vec::new();
    let is_all_static = vue3_variable_declaration_is_static_hoist(declaration);
    for (index, declarator) in declaration.declarations.iter().enumerate() {
        if let Some(Expression::CallExpression(call)) =
            declarator.init.as_ref().map(unwrap_vue3_ts_expression)
        {
            if is_call_named(call, "defineProps") {
                if matches!(declarator.id, BindingPattern::BindingIdentifier(_)) {
                    collect_vue3_define_props_call(source, call, analysis, is_prod, custom_element);
                    collect_pattern_bindings(&declarator.id, &mut analysis.return_bindings);
                    collect_pattern_binding_types(
                        &declarator.id,
                        "setup-reactive-const",
                        &mut analysis.setup_bindings,
                    );
                    edits.overwrite(call.span.start as usize, call.span.end as usize, "__props");
                } else {
                    match props_destructure {
                        SfcPropsDestructureMode::Enabled => {
                            let props_rest_id = collect_vue3_define_props_destructure_bindings(
                                source,
                                &declarator.id,
                                analysis,
                            );
                            collect_vue3_define_props_call(
                                source,
                                call,
                                analysis,
                                is_prod,
                                custom_element,
                            );
                            if let Some(rest_id) = props_rest_id {
                                rewrite_vue3_define_props_destructure_rest(
                                    &declarator.id,
                                    call,
                                    &rest_id,
                                    analysis,
                                    edits,
                                );
                            } else {
                                macro_declarators.push(index);
                            }
                        }
                        SfcPropsDestructureMode::Disabled => {
                            collect_vue3_define_props_call(
                                source,
                                call,
                                analysis,
                                is_prod,
                                custom_element,
                            );
                            collect_pattern_bindings(&declarator.id, &mut analysis.return_bindings);
                            collect_vue3_script_pattern_binding_types(
                                &declarator.id,
                                declaration.kind == VariableDeclarationKind::Const,
                                true,
                                &mut analysis.setup_bindings,
                            );
                            edits.overwrite(
                                call.span.start as usize,
                                call.span.end as usize,
                                "__props",
                            );
                        }
                        SfcPropsDestructureMode::Error => {
                            collect_vue3_define_props_call(
                                source,
                                call,
                                analysis,
                                is_prod,
                                custom_element,
                            );
                            analysis.errors.push(
                                "Props destructure is explicitly prohibited via config.".into(),
                            );
                            collect_pattern_bindings(&declarator.id, &mut analysis.return_bindings);
                            collect_vue3_script_pattern_binding_types(
                                &declarator.id,
                                declaration.kind == VariableDeclarationKind::Const,
                                true,
                                &mut analysis.setup_bindings,
                            );
                            edits.overwrite(
                                call.span.start as usize,
                                call.span.end as usize,
                                "__props",
                            );
                        }
                    }
                }
                continue;
            }
            if is_call_named(call, "withDefaults")
                && collect_vue3_with_defaults_call(source, call, analysis, is_prod, custom_element)
            {
                if matches!(declarator.id, BindingPattern::ObjectPattern(_)) {
                    analysis.warnings.push(
                        "withDefaults() is unnecessary when using destructure with defineProps().\nReactive destructure will be disabled when using withDefaults().\nPrefer using destructure default values, e.g. const { foo = 1 } = defineProps(...)."
                            .into(),
                    );
                }
                collect_pattern_bindings(&declarator.id, &mut analysis.return_bindings);
                collect_pattern_binding_types(
                    &declarator.id,
                    "setup-const",
                    &mut analysis.setup_bindings,
                );
                edits.overwrite(call.span.start as usize, call.span.end as usize, "__props");
                continue;
            }
            if is_call_named(call, "defineEmits") {
                let emit_binding =
                    first_pattern_binding(&declarator.id).unwrap_or_else(|| "emit".into());
                collect_vue3_define_emits_call(source, call, Some(&emit_binding), analysis);
                collect_pattern_bindings(&declarator.id, &mut analysis.return_bindings);
                collect_pattern_binding_types(
                    &declarator.id,
                    "setup-const",
                    &mut analysis.setup_bindings,
                );
                edits.overwrite(call.span.start as usize, call.span.end as usize, "__emit");
                continue;
            }
            if is_call_named(call, "defineOptions") {
                collect_vue3_define_options_call(source, call, analysis);
                analysis
                    .errors
                    .push("defineOptions() has no returning value, it cannot be assigned.".into());
                continue;
            }
            if is_call_named(call, "defineSlots") {
                collect_vue3_define_slots_call(call, Some(&declarator.id), edits, analysis);
                collect_pattern_bindings(&declarator.id, &mut analysis.return_bindings);
                collect_pattern_binding_types(
                    &declarator.id,
                    "setup-const",
                    &mut analysis.setup_bindings,
                );
                continue;
            }
            if is_call_named(call, "defineModel") {
                collect_vue3_define_model_call(source, call, Some(&declarator.id), edits, analysis);
                collect_pattern_bindings(&declarator.id, &mut analysis.return_bindings);
                collect_pattern_binding_types(
                    &declarator.id,
                    "setup-ref",
                    &mut analysis.setup_bindings,
                );
                continue;
            }
        }
        if matches!(declarator.id, BindingPattern::BindingIdentifier(_)) {
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
                &mut analysis.setup_bindings,
            );
        } else {
            collect_vue3_script_pattern_binding_types(
                &declarator.id,
                declaration.kind == VariableDeclarationKind::Const,
                false,
                &mut analysis.setup_bindings,
            );
        }
        collect_pattern_bindings(&declarator.id, &mut analysis.return_bindings);
    }
    remove_vue27_macro_declarators(declaration, &macro_declarators, edits);
}

pub(crate) fn collect_vue3_define_options_call(
    source: &str,
    call: &oxc_ast::ast::CallExpression<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    if analysis.has_define_options {
        analysis
            .errors
            .push("duplicate defineOptions() call".into());
    }
    if call.type_arguments.is_some() {
        analysis
            .errors
            .push("defineOptions() cannot accept type arguments".into());
    }
    let Some(argument) = call.arguments.first() else {
        return;
    };
    analysis.has_define_options = true;
    let expression = unwrap_vue3_ts_expression(argument.to_expression());
    check_vue3_define_options_keys(expression, analysis);
    analysis.options_runtime = source
        .get(expression.span().start as usize..expression.span().end as usize)
        .map(str::trim)
        .map(ToOwned::to_owned);
}

pub(crate) fn unwrap_vue3_ts_expression<'a>(expression: &'a Expression<'a>) -> &'a Expression<'a> {
    match expression {
        Expression::TSAsExpression(expression) => unwrap_vue3_ts_expression(&expression.expression),
        Expression::TSSatisfiesExpression(expression) => {
            unwrap_vue3_ts_expression(&expression.expression)
        }
        Expression::TSTypeAssertion(expression) => {
            unwrap_vue3_ts_expression(&expression.expression)
        }
        Expression::TSNonNullExpression(expression) => {
            unwrap_vue3_ts_expression(&expression.expression)
        }
        Expression::TSInstantiationExpression(expression) => {
            unwrap_vue3_ts_expression(&expression.expression)
        }
        Expression::ParenthesizedExpression(expression) => {
            unwrap_vue3_ts_expression(&expression.expression)
        }
        _ => expression,
    }
}

pub(crate) fn check_vue3_define_options_keys(
    expression: &Expression<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    let Expression::ObjectExpression(object) = expression else {
        return;
    };
    for property in &object.properties {
        let key = match property {
            ObjectPropertyKind::ObjectProperty(property) if !property.computed => {
                match &property.key {
                    PropertyKey::StaticIdentifier(identifier) => Some(identifier.name.to_string()),
                    _ => None,
                }
            }
            _ => None,
        };
        let Some(key) = key else {
            continue;
        };
        let replacement = match key.as_str() {
            "props" => Some("defineProps"),
            "emits" => Some("defineEmits"),
            "expose" => Some("defineExpose"),
            "slots" => Some("defineSlots"),
            _ => None,
        };
        if let Some(replacement) = replacement {
            analysis.errors.push(format!(
                "defineOptions() cannot be used to declare {key}. Use {replacement}() instead."
            ));
        }
    }
}

pub(crate) fn collect_vue3_define_slots_call(
    call: &oxc_ast::ast::CallExpression<'_>,
    binding: Option<&BindingPattern<'_>>,
    edits: &mut SourceEdits<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    if analysis.has_define_slots {
        analysis.errors.push("duplicate defineSlots() call".into());
    }
    analysis.has_define_slots = true;
    if !call.arguments.is_empty() {
        analysis
            .errors
            .push("defineSlots() cannot accept arguments".into());
    }
    if binding.is_some() {
        analysis.needs_use_slots = true;
        edits.overwrite(
            call.span.start as usize,
            call.span.end as usize,
            "_useSlots()",
        );
    }
}

pub(crate) fn collect_vue3_define_expose_call(
    call: &oxc_ast::ast::CallExpression<'_>,
    edits: &mut SourceEdits<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    if analysis.has_define_expose {
        analysis.errors.push("duplicate defineExpose() call".into());
    }
    analysis.has_define_expose = true;
    edits.overwrite(
        call.span.start as usize,
        call.callee.span().end as usize,
        "__expose",
    );
}

pub(crate) fn collect_vue3_define_model_call(
    source: &str,
    call: &oxc_ast::ast::CallExpression<'_>,
    binding: Option<&BindingPattern<'_>>,
    edits: &mut SourceEdits<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    if let Some(type_argument) = call
        .type_arguments
        .as_ref()
        .and_then(|arguments| arguments.params.first())
    {
        record_vue3_type_argument_deps(type_argument, analysis);
    }
    check_vue3_define_model_scope_reference(call, analysis);
    let model = vue3_define_model_decl(source, call, analysis);
    if analysis
        .models
        .iter()
        .any(|existing| existing.name == model.name)
    {
        analysis
            .errors
            .push(format!("duplicate model name \"{}\"", model.name));
    }
    push_unique(&mut analysis.props_bindings, &model.name);
    if let Some(binding) = binding.and_then(first_pattern_binding) {
        analysis
            .setup_bindings
            .insert(binding, "setup-ref".to_string());
    }
    rewrite_vue3_define_model_call(call, edits);
    analysis.models.push(model);
}

pub(crate) fn check_vue3_define_model_scope_reference(
    call: &oxc_ast::ast::CallExpression<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    let first_expression = call
        .arguments
        .first()
        .map(|argument| unwrap_vue3_ts_expression(argument.to_expression()));
    let has_name = first_expression.and_then(vue3_define_model_name).is_some();
    let options = if has_name {
        call.arguments.get(1)
    } else {
        call.arguments.first()
    };
    let Some(options) = options else {
        return;
    };
    let expression = unwrap_vue3_ts_expression(options.to_expression());
    if vue3_define_model_prop_options_reference_non_literal_setup_local(expression, analysis) {
        analysis
            .errors
            .push(vue3_invalid_scope_reference_error("defineModel"));
    }
}

pub(crate) fn vue3_define_model_prop_options_reference_non_literal_setup_local(
    expression: &Expression<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> bool {
    let Expression::ObjectExpression(object) = unwrap_vue3_ts_expression(expression) else {
        return false;
    };
    object.properties.iter().any(|property| match property {
        ObjectPropertyKind::ObjectProperty(property) => {
            if property.computed
                || matches!(property.key.static_name().as_deref(), Some("get" | "set"))
            {
                false
            } else {
                vue3_expression_references_non_literal_setup_local(&property.value, analysis)
            }
        }
        ObjectPropertyKind::SpreadProperty(_) => false,
    })
}

pub(crate) fn vue3_define_model_decl(
    source: &str,
    call: &oxc_ast::ast::CallExpression<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Vue3ModelDecl {
    let first_expression = call
        .arguments
        .first()
        .map(|argument| unwrap_vue3_ts_expression(argument.to_expression()));
    let (name, has_name) = first_expression
        .and_then(vue3_define_model_name)
        .map(|name| (name, true))
        .unwrap_or_else(|| ("modelValue".to_string(), false));
    let options = if has_name {
        call.arguments.get(1)
    } else {
        call.arguments.first()
    };
    let prop_runtime =
        options.and_then(|argument| vue3_define_model_prop_runtime(source, argument));
    let runtime_types = call
        .type_arguments
        .as_ref()
        .and_then(|arguments| arguments.params.first())
        .map(|type_argument| infer_vue3_define_model_runtime_type(type_argument, analysis));
    Vue3ModelDecl {
        name,
        prop_runtime,
        runtime_types,
    }
}

pub(crate) fn vue3_define_model_name(expression: &Expression<'_>) -> Option<String> {
    match expression {
        Expression::StringLiteral(literal) => Some(literal.value.to_string()),
        Expression::TemplateLiteral(literal)
            if literal.expressions.is_empty() && literal.quasis.len() == 1 =>
        {
            literal
                .quasis
                .first()
                .and_then(|quasi| quasi.value.cooked.as_ref())
                .map(|value| value.as_str().to_string())
        }
        _ => None,
    }
}

pub(crate) fn vue3_define_model_prop_runtime(
    source: &str,
    argument: &Argument<'_>,
) -> Option<String> {
    let expression = unwrap_vue3_ts_expression(argument.to_expression());
    let start = expression.span().start as usize;
    let end = expression.span().end as usize;
    let runtime = if let Some(split) = vue3_define_model_options_split(expression) {
        remove_source_ranges(source, start, end, &split.transformer_option_ranges)
            .or_else(|| source.get(start..end).map(ToOwned::to_owned))
    } else {
        source.get(start..end).map(ToOwned::to_owned)
    }?;
    let runtime = runtime.trim();
    if runtime.is_empty() {
        None
    } else {
        Some(runtime.to_string())
    }
}

pub(crate) fn rewrite_vue3_define_model_call(
    call: &oxc_ast::ast::CallExpression<'_>,
    edits: &mut SourceEdits<'_>,
) {
    let first_expression = call
        .arguments
        .first()
        .map(|argument| unwrap_vue3_ts_expression(argument.to_expression()));
    let has_name = first_expression.and_then(vue3_define_model_name).is_some();
    let options_index = if has_name { 1 } else { 0 };
    let options = call.arguments.get(options_index);
    let options_split = options.and_then(|argument| {
        vue3_define_model_options_split(unwrap_vue3_ts_expression(argument.to_expression()))
    });
    let options_removed = options_split
        .as_ref()
        .is_some_and(|split| split.remove_entire_call_options);
    if let Some(split) = options_split.as_ref() {
        if split.remove_entire_call_options {
            if has_name {
                if let (Some(previous), Some(options)) = (call.arguments.first(), options) {
                    edits.remove(
                        previous.to_expression().span().end as usize,
                        options.to_expression().span().end as usize,
                    );
                }
            } else if let Some(options) = options {
                let expression = options.to_expression();
                edits.remove(
                    expression.span().start as usize,
                    expression.span().end as usize,
                );
            }
        } else {
            for (start, end) in &split.prop_option_ranges {
                edits.remove(*start, *end);
            }
        }
    }
    edits.overwrite(
        call.callee.span().start as usize,
        call.callee.span().end as usize,
        "_useModel",
    );
    let Some(first_argument) = call.arguments.first() else {
        edits.prepend_right(call.span.end as usize - 1, r#"__props, "modelValue""#);
        return;
    };
    let first_start = first_argument.to_expression().span().start as usize;
    if has_name {
        edits.prepend_right(first_start, "__props, ");
        return;
    }
    let prefix = if options_removed {
        r#"__props, "modelValue""#
    } else {
        r#"__props, "modelValue", "#
    };
    edits.prepend_right(first_start, prefix);
}

pub(crate) fn vue3_define_model_options_split(
    expression: &Expression<'_>,
) -> Option<Vue3DefineModelOptionsSplit> {
    let Expression::ObjectExpression(object) = unwrap_vue3_ts_expression(expression) else {
        return None;
    };
    if object.properties.iter().any(|property| {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            return true;
        };
        property.computed
    }) {
        return None;
    }

    let mut split = Vue3DefineModelOptionsSplit::default();
    for (index, property) in object.properties.iter().enumerate() {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            return None;
        };
        let start = property.span.start as usize;
        let end = object
            .properties
            .get(index + 1)
            .map(|next| next.span().start as usize)
            .unwrap_or_else(|| (object.span.end as usize).saturating_sub(1));
        if matches!(property.key.static_name().as_deref(), Some("get" | "set")) {
            split.transformer_option_ranges.push((start, end));
        } else {
            split.prop_option_ranges.push((start, end));
        }
    }
    split.remove_entire_call_options = split.prop_option_ranges.len() == object.properties.len();
    Some(split)
}

pub(crate) fn remove_source_ranges(
    source: &str,
    start: usize,
    end: usize,
    ranges: &[(usize, usize)],
) -> Option<String> {
    let mut ranges = ranges.to_vec();
    ranges.sort_by_key(|range| range.0);
    let mut cursor = start;
    let mut output = String::new();
    for (range_start, range_end) in ranges {
        if range_start < cursor || range_end < range_start || range_end > end {
            return None;
        }
        output.push_str(source.get(cursor..range_start)?);
        cursor = range_end;
    }
    output.push_str(source.get(cursor..end)?);
    Some(output)
}

pub(crate) fn collect_vue3_define_props_call(
    source: &str,
    call: &oxc_ast::ast::CallExpression<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
    is_prod: bool,
    custom_element: bool,
) {
    collect_vue3_define_props_call_seen(analysis);
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
        collect_vue3_define_props_type(
            source,
            type_argument,
            None,
            analysis,
            is_prod,
            custom_element,
        );
        return;
    }
    let Some(argument) = call.arguments.first() else {
        return;
    };
    let expression = argument.to_expression();
    check_vue3_invalid_non_literal_scope_reference(expression, "defineProps", analysis);
    for key in vue3_runtime_prop_keys(expression) {
        push_unique(&mut analysis.props_bindings, &key);
    }
    let Some(runtime) = source
        .get(expression.span().start as usize..expression.span().end as usize)
        .map(ToOwned::to_owned)
    else {
        return;
    };
    analysis.props_runtime =
        if let Some(defaults) = vue3_props_destructured_runtime_defaults(analysis) {
            analysis.needs_merge_defaults = true;
            Some(format!(
                "/*@__PURE__*/_mergeDefaults({}, {})",
                runtime.trim(),
                defaults
            ))
        } else {
            Some(runtime)
        };
}

pub(crate) fn collect_vue3_define_props_call_seen(analysis: &mut Vue3ScriptSetupAnalysis) {
    if analysis.has_define_props {
        analysis.errors.push("duplicate defineProps() call".into());
    }
    analysis.has_define_props = true;
}

pub(crate) fn collect_vue3_with_defaults_call(
    source: &str,
    call: &oxc_ast::ast::CallExpression<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
    is_prod: bool,
    custom_element: bool,
) -> bool {
    let Some(define_props_call) = call.arguments.first().and_then(|argument| {
        match unwrap_vue3_ts_expression(argument.to_expression()) {
            Expression::CallExpression(call) if is_call_named(call, "defineProps") => Some(call),
            _ => None,
        }
    }) else {
        analysis
            .errors
            .push("withDefaults' first argument must be a defineProps call.".to_string());
        return true;
    };
    let Some(type_argument) = define_props_call
        .type_arguments
        .as_ref()
        .and_then(|arguments| arguments.params.first())
    else {
        collect_vue3_define_props_call(
            source,
            define_props_call,
            analysis,
            is_prod,
            custom_element,
        );
        analysis.errors.push(
            "withDefaults can only be used with type-based defineProps declaration.".to_string(),
        );
        return true;
    };
    collect_vue3_define_props_call_seen(analysis);
    if !define_props_call.arguments.is_empty() {
        analysis
            .errors
            .push(vue27_macro_type_and_runtime_error("defineProps"));
        analysis.errors.push(
            "withDefaults can only be used with type-based defineProps declaration.".to_string(),
        );
    }
    if call.arguments.get(1).is_none() {
        analysis
            .errors
            .push("The 2nd argument of withDefaults is required.".to_string());
    }
    let defaults = call
        .arguments
        .get(1)
        .and_then(|argument| {
            if vue3_expression_references_non_literal_setup_local(
                argument.to_expression(),
                analysis,
            ) {
                analysis.errors.push(
                    "`defineProps()` in <script setup> cannot reference locally declared variables because it will be hoisted outside of the setup() function. If your component options require initialization in the module scope, use a separate normal <script> to export the options instead."
                        .to_string(),
                );
            }
            vue3_runtime_defaults_from_argument(source, argument)
        });
    collect_vue3_define_props_type(
        source,
        type_argument,
        defaults,
        analysis,
        is_prod,
        custom_element,
    );
    true
}

pub(crate) fn collect_vue3_define_props_type(
    source: &str,
    type_argument: &TSType<'_>,
    defaults: Option<Vue27RuntimeDefaults>,
    analysis: &mut Vue3ScriptSetupAnalysis,
    is_prod: bool,
    custom_element: bool,
) {
    record_vue3_type_argument_deps(type_argument, analysis);
    let Some(type_members) = vue3_resolve_props_type_with_mode(
        source,
        type_argument,
        analysis,
        Vue3PropsTypeResolveMode::Consumed,
    ) else {
        return;
    };
    analysis.errors.extend(type_members.errors.clone());
    let default_map = defaults
        .as_ref()
        .and_then(|defaults| defaults.static_defaults.as_ref());
    let has_static_defaults = default_map.is_some();
    let dynamic_defaults = defaults
        .as_ref()
        .filter(|defaults| defaults.static_defaults.is_none());
    let mut props = Vec::new();
    for member in &type_members.members {
        let mut prop = member.clone();
        if let Some(default) =
            vue3_props_destructured_default_option(analysis, &prop.key, Some(prop.types.as_slice()))
        {
            prop.default = Some(default);
        } else if let Some(default) = default_map.and_then(|defaults| defaults.get(&prop.key)) {
            prop.default = Some(default.clone());
        }
        analysis
            .props_type_runtime_types
            .insert(prop.key.clone(), prop.types.clone());
        push_unique(&mut analysis.props_bindings, &prop.key);
        props.push(prop);
    }
    analysis.props_type_runtime = true;
    let props_runtime =
        gen_vue3_runtime_props(&props, is_prod, has_static_defaults, custom_element);
    analysis.props_runtime = if let Some(defaults) = dynamic_defaults {
        analysis.needs_merge_defaults = true;
        Some(format!(
            "/*@__PURE__*/_mergeDefaults({props_runtime}, {})",
            defaults.source
        ))
    } else {
        Some(props_runtime)
    };
}

pub(crate) fn vue3_resolve_props_type<'a>(
    source: &str,
    type_argument: &'a TSType<'a>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vue27TypeMembers> {
    vue3_resolve_props_type_with_mode(
        source,
        type_argument,
        analysis,
        Vue3PropsTypeResolveMode::Silent,
    )
}

pub(crate) fn vue3_resolve_props_type_with_mode<'a>(
    source: &str,
    type_argument: &'a TSType<'a>,
    analysis: &Vue3ScriptSetupAnalysis,
    mode: Vue3PropsTypeResolveMode,
) -> Option<Vue27TypeMembers> {
    match type_argument {
        TSType::TSTypeLiteral(literal) => {
            Some(vue3_type_members_from_literal(source, literal, analysis))
        }
        TSType::TSMappedType(mapped) => {
            vue3_type_members_from_mapped_type(source, mapped, analysis)
        }
        TSType::TSTypeReference(reference) => {
            let name = vue3_ts_type_name_key(&reference.type_name)?;
            match name.as_str() {
                "ExtractPropTypes" | "ExtractPublicPropTypes" => {
                    let ty = vue3_type_reference_first_type_argument(reference)?;
                    return vue3_resolve_extract_prop_types(source, ty, analysis);
                }
                "Partial" => {
                    let ty = vue3_type_reference_first_type_argument(reference)?;
                    return vue3_resolve_props_type_with_mode(source, ty, analysis, mode)
                        .map(vue3_type_members_optional);
                }
                "Required" => {
                    let ty = vue3_type_reference_first_type_argument(reference)?;
                    return vue3_resolve_props_type_with_mode(source, ty, analysis, mode)
                        .map(vue3_type_members_required);
                }
                "Readonly" => {
                    let ty = vue3_type_reference_first_type_argument(reference)?;
                    return vue3_resolve_props_type_with_mode(source, ty, analysis, mode);
                }
                "Record" => {
                    return vue3_type_members_from_record_type(source, reference, analysis);
                }
                "Pick" => {
                    let ty = vue3_type_reference_type_argument(reference, 0)?;
                    let keys = vue3_type_reference_type_argument(reference, 1)?;
                    let members = vue3_resolve_props_type_with_mode(source, ty, analysis, mode)?;
                    let keys = vue3_resolve_string_type_keys(keys, analysis)?;
                    return Some(vue3_type_members_pick(members, &keys));
                }
                "Omit" => {
                    let ty = vue3_type_reference_type_argument(reference, 0)?;
                    let keys = vue3_type_reference_type_argument(reference, 1)?;
                    let members = vue3_resolve_props_type_with_mode(source, ty, analysis, mode)?;
                    let keys = vue3_resolve_string_type_keys(keys, analysis)?;
                    return Some(vue3_type_members_omit(members, &keys));
                }
                _ => {}
            }
            if let Some(resolved) =
                vue3_resolve_generic_props_type_alias(source, reference, analysis)
            {
                return Some(resolved);
            }
            if let Some(members) = analysis.props_type_declarations.get(&name).cloned() {
                return Some(members);
            }
            if analysis.silent_unresolved_type_names.contains(&name) {
                return None;
            }
            if mode == Vue3PropsTypeResolveMode::Consumed {
                if let Some(import_source) = analysis.unresolved_import_sources.get(&name) {
                    return Some(vue3_type_members_empty(
                        source,
                        type_argument.span(),
                        vec![vue3_failed_import_source_error(import_source)],
                    ));
                }
                if !analysis.generic_type_parameter_names.contains(&name) {
                    return Some(vue3_type_members_empty(
                        source,
                        type_argument.span(),
                        vec![vue3_unresolvable_type_reference_error()],
                    ));
                }
            }
            None
        }
        TSType::TSUnionType(union) => {
            let (members, errors) = vue3_merge_props_type_members(
                union
                    .types
                    .iter()
                    .filter_map(|ty| vue3_resolve_props_type_with_mode(source, ty, analysis, mode)),
                false,
            );
            vue3_merged_type_members(source, union.span, members, errors)
        }
        TSType::TSIntersectionType(intersection) => {
            let (members, errors) = vue3_merge_props_type_members(
                intersection
                    .types
                    .iter()
                    .filter(|ty| {
                        !vue3_source_has_immediate_leading_vue_ignore_comment(
                            source,
                            ty.span().start as usize,
                        )
                    })
                    .filter_map(|ty| vue3_resolve_props_type_with_mode(source, ty, analysis, mode)),
                true,
            );
            vue3_merged_type_members(source, intersection.span, members, errors)
        }
        TSType::TSParenthesizedType(parenthesized) => vue3_resolve_props_type_with_mode(
            source,
            &parenthesized.type_annotation,
            analysis,
            mode,
        ),
        TSType::TSImportType(import_type) => {
            if import_type.source.value.as_str() == "vue"
                && import_type.qualifier.as_ref().is_some_and(|qualifier| {
                    matches!(
                        vue3_import_type_qualifier_key(qualifier).as_str(),
                        "ExtractPropTypes" | "ExtractPublicPropTypes"
                    )
                })
            {
                let ty = vue3_import_type_first_type_argument(import_type)?;
                return vue3_resolve_extract_prop_types(source, ty, analysis);
            }
            let Some(resolved) = vue3_resolve_import_type(import_type, analysis) else {
                return (mode == Vue3PropsTypeResolveMode::Consumed).then(|| {
                    vue3_type_members_empty(
                        source,
                        type_argument.span(),
                        vec![vue3_failed_import_source_error(
                            import_type.source.value.as_str(),
                        )],
                    )
                });
            };
            if let Some(members) = resolved
                .context
                .props_type_declarations
                .get(&resolved.name)
                .cloned()
            {
                return Some(members);
            }
            (mode == Vue3PropsTypeResolveMode::Consumed).then(|| {
                vue3_type_members_empty(
                    source,
                    type_argument.span(),
                    vec![vue3_unresolvable_type_reference_error()],
                )
            })
        }
        TSType::TSIndexedAccessType(indexed) => {
            vue3_resolve_indexed_access_props_type(source, indexed, analysis, mode)
        }
        _ => {
            if mode == Vue3PropsTypeResolveMode::Consumed {
                Some(vue3_type_members_empty(
                    source,
                    type_argument.span(),
                    vec![vue3_unresolvable_type_error(type_argument)],
                ))
            } else {
                None
            }
        }
    }
}

pub(crate) fn vue3_merged_type_members(
    source: &str,
    span: oxc_span::Span,
    members: Vec<Vue27RuntimeProp>,
    errors: Vec<String>,
) -> Option<Vue27TypeMembers> {
    if members.is_empty() && errors.is_empty() {
        None
    } else {
        Some(Vue27TypeMembers {
            source: source
                .get(span.start as usize..span.end as usize)
                .unwrap_or_default()
                .to_string(),
            members,
            errors,
        })
    }
}

pub(crate) fn vue3_type_members_empty(
    source: &str,
    span: oxc_span::Span,
    errors: Vec<String>,
) -> Vue27TypeMembers {
    Vue27TypeMembers {
        source: source
            .get(span.start as usize..span.end as usize)
            .unwrap_or_default()
            .to_string(),
        members: Vec::new(),
        errors,
    }
}

pub(crate) fn vue3_unresolvable_type_reference_error() -> String {
    "Unresolvable type reference or unsupported built-in utility type".to_string()
}

pub(crate) fn vue3_failed_import_source_error(source: &str) -> String {
    format!("Failed to resolve import source {source:?}.")
}

pub(crate) fn vue3_unsupported_computed_key_error() -> String {
    "Unsupported computed key in type referenced by a macro".to_string()
}

pub(crate) fn vue3_unsupported_index_type_error() -> String {
    "Unsupported type when resolving index type".to_string()
}

pub(crate) fn vue3_unresolvable_type_error(ty: &TSType<'_>) -> String {
    format!("Unresolvable type: {}", vue3_ts_type_kind_name(ty))
}

pub(crate) fn vue3_ts_type_kind_name(ty: &TSType<'_>) -> &'static str {
    match ty {
        TSType::TSAnyKeyword(_) => "TSAnyKeyword",
        TSType::TSBigIntKeyword(_) => "TSBigIntKeyword",
        TSType::TSBooleanKeyword(_) => "TSBooleanKeyword",
        TSType::TSIntrinsicKeyword(_) => "TSIntrinsicKeyword",
        TSType::TSNeverKeyword(_) => "TSNeverKeyword",
        TSType::TSNullKeyword(_) => "TSNullKeyword",
        TSType::TSNumberKeyword(_) => "TSNumberKeyword",
        TSType::TSObjectKeyword(_) => "TSObjectKeyword",
        TSType::TSStringKeyword(_) => "TSStringKeyword",
        TSType::TSSymbolKeyword(_) => "TSSymbolKeyword",
        TSType::TSThisType(_) => "TSThisType",
        TSType::TSUndefinedKeyword(_) => "TSUndefinedKeyword",
        TSType::TSUnknownKeyword(_) => "TSUnknownKeyword",
        TSType::TSVoidKeyword(_) => "TSVoidKeyword",
        TSType::TSLiteralType(_) => "TSLiteralType",
        TSType::TSTemplateLiteralType(_) => "TSTemplateLiteralType",
        TSType::TSTypeReference(_) => "TSTypeReference",
        TSType::TSTypeLiteral(_) => "TSTypeLiteral",
        TSType::TSArrayType(_) => "TSArrayType",
        TSType::TSTupleType(_) => "TSTupleType",
        TSType::TSUnionType(_) => "TSUnionType",
        TSType::TSIntersectionType(_) => "TSIntersectionType",
        TSType::TSParenthesizedType(_) => "TSParenthesizedType",
        TSType::TSFunctionType(_) => "TSFunctionType",
        TSType::TSConstructorType(_) => "TSConstructorType",
        TSType::TSTypeQuery(_) => "TSTypeQuery",
        TSType::TSTypeOperatorType(_) => "TSTypeOperatorType",
        TSType::TSIndexedAccessType(_) => "TSIndexedAccessType",
        TSType::TSMappedType(_) => "TSMappedType",
        TSType::TSConditionalType(_) => "TSConditionalType",
        TSType::TSInferType(_) => "TSInferType",
        TSType::TSImportType(_) => "TSImportType",
        TSType::TSNamedTupleMember(_) => "TSNamedTupleMember",
        TSType::TSTypePredicate(_) => "TSTypePredicate",
        TSType::JSDocNullableType(_) => "JSDocNullableType",
        TSType::JSDocNonNullableType(_) => "JSDocNonNullableType",
        TSType::JSDocUnknownType(_) => "JSDocUnknownType",
    }
}

pub(crate) fn vue3_merge_props_type_members(
    members: impl IntoIterator<Item = Vue27TypeMembers>,
    filter_duplicate_unknown: bool,
) -> (Vec<Vue27RuntimeProp>, Vec<String>) {
    let mut merged: Vec<Vue27RuntimeProp> = Vec::new();
    let mut errors = Vec::new();
    for type_members in members {
        errors.extend(type_members.errors);
        for prop in type_members.members {
            if let Some(index) = merged.iter().position(|existing| existing.key == prop.key) {
                let existing = &mut merged[index];
                let mut types = Vec::new();
                for runtime_type in existing.types.iter().chain(prop.types.iter()) {
                    if filter_duplicate_unknown && runtime_type == "Unknown" {
                        continue;
                    }
                    push_unique(&mut types, runtime_type);
                }
                if types.is_empty() {
                    types.push("Unknown".to_string());
                }
                existing.types = types;
                existing.required &= prop.required;
                continue;
            }
            merged.push(prop);
        }
    }
    (merged, errors)
}

pub(crate) fn vue3_resolve_projectable_props_type(
    source: &str,
    type_argument: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vue27TypeMembers> {
    vue3_resolve_props_type(source, type_argument, analysis)
}

pub(crate) fn vue3_resolve_extract_prop_types(
    source: &str,
    type_argument: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vue27TypeMembers> {
    vue3_resolve_props_options_type(source, type_argument, analysis)
}

pub(crate) fn vue3_resolve_props_options_type(
    source: &str,
    type_argument: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vue27TypeMembers> {
    match type_argument {
        TSType::TSTypeLiteral(_) => {
            vue3_props_options_type_members(source, type_argument, analysis)
        }
        TSType::TSTypeReference(reference) => {
            let name = vue3_ts_type_name_key(&reference.type_name)?;
            if name == "ReturnType" {
                let ty = vue3_type_reference_first_type_argument(reference)?;
                return vue3_resolve_return_type_props_options(source, ty, analysis);
            }
            analysis.props_options_type_declarations.get(&name).cloned()
        }
        TSType::TSTypeQuery(query) => vue3_type_query_props_options_declaration(query, analysis),
        TSType::TSParenthesizedType(parenthesized) => {
            vue3_resolve_props_options_type(source, &parenthesized.type_annotation, analysis)
        }
        TSType::TSImportType(import_type) => {
            let resolved = vue3_resolve_import_type(import_type, analysis)?;
            resolved
                .context
                .props_options_type_declarations
                .get(&resolved.name)
                .cloned()
        }
        _ => None,
    }
}

pub(crate) fn vue3_resolve_return_type_props_options(
    source: &str,
    type_argument: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vue27TypeMembers> {
    match type_argument {
        TSType::TSTypeQuery(query) => {
            vue3_type_query_return_props_options_declaration(query, analysis)
        }
        TSType::TSTypeReference(reference) => {
            let name = vue3_ts_type_name_key(&reference.type_name)?;
            analysis
                .return_type_props_options_declarations
                .get(&name)
                .cloned()
        }
        TSType::TSParenthesizedType(parenthesized) => {
            vue3_resolve_return_type_props_options(source, &parenthesized.type_annotation, analysis)
        }
        TSType::TSImportType(import_type) => {
            let resolved = vue3_resolve_import_type(import_type, analysis)?;
            resolved
                .context
                .return_type_props_options_declarations
                .get(&resolved.name)
                .cloned()
        }
        _ => vue3_props_options_type_members(source, type_argument, analysis),
    }
}

pub(crate) fn vue3_type_reference_first_type_argument<'a>(
    reference: &'a TSTypeReference<'a>,
) -> Option<&'a TSType<'a>> {
    vue3_type_reference_type_argument(reference, 0)
}

pub(crate) fn vue3_type_reference_type_argument<'a>(
    reference: &'a TSTypeReference<'a>,
    index: usize,
) -> Option<&'a TSType<'a>> {
    reference
        .type_arguments
        .as_ref()
        .and_then(|arguments| arguments.params.get(index))
}

pub(crate) fn vue3_import_type_first_type_argument<'a>(
    import_type: &'a TSImportType<'a>,
) -> Option<&'a TSType<'a>> {
    import_type
        .type_arguments
        .as_ref()
        .and_then(|arguments| arguments.params.first())
}

pub(crate) fn vue3_type_members_optional(mut members: Vue27TypeMembers) -> Vue27TypeMembers {
    for prop in &mut members.members {
        prop.required = false;
    }
    members
}

pub(crate) fn vue3_type_members_required(mut members: Vue27TypeMembers) -> Vue27TypeMembers {
    for prop in &mut members.members {
        prop.required = true;
    }
    members
}

pub(crate) fn vue3_type_members_pick(
    mut members: Vue27TypeMembers,
    keys: &BTreeSet<String>,
) -> Vue27TypeMembers {
    members.members.retain(|prop| keys.contains(&prop.key));
    members
}

pub(crate) fn vue3_type_members_omit(
    mut members: Vue27TypeMembers,
    keys: &BTreeSet<String>,
) -> Vue27TypeMembers {
    members.members.retain(|prop| !keys.contains(&prop.key));
    members
}

pub(crate) fn vue3_resolve_indexed_access_props_type(
    source: &str,
    indexed: &oxc_ast::ast::TSIndexedAccessType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    mode: Vue3PropsTypeResolveMode,
) -> Option<Vue27TypeMembers> {
    let object_members =
        vue3_resolve_props_type_with_mode(source, &indexed.object_type, analysis, mode)?;
    let keys = match vue3_indexed_access_member_keys(&indexed.index_type, &object_members, analysis)
    {
        Some(keys) => keys,
        None if mode == Vue3PropsTypeResolveMode::Consumed => {
            return Some(vue3_type_members_empty(
                source,
                indexed.index_type.span(),
                vec![vue3_unsupported_index_type_error()],
            ));
        }
        None => return None,
    };
    let mut projected = Vec::new();
    let mut errors = object_members.errors;
    for key in keys {
        let Some(prop) = object_members.members.iter().find(|prop| prop.key == key) else {
            continue;
        };
        if let Some(members) = vue3_resolve_props_type_from_runtime_prop(prop, analysis) {
            errors.extend(members.errors);
            projected.extend(members.members);
        }
    }
    vue3_merged_type_members(source, indexed.span, projected, errors)
}

pub(crate) fn vue3_indexed_access_member_keys(
    index_type: &TSType<'_>,
    members: &Vue27TypeMembers,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vec<String>> {
    if let Some(keys) = vue3_resolve_ordered_string_type_keys(index_type, analysis) {
        return Some(keys);
    }
    if vue3_indexed_access_is_string_key(index_type, analysis) {
        let mut keys = Vec::new();
        for prop in &members.members {
            push_unique(&mut keys, &prop.key);
        }
        return Some(keys);
    }
    None
}

pub(crate) fn vue3_indexed_access_is_string_key(
    index_type: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> bool {
    match index_type {
        TSType::TSStringKeyword(_) => true,
        TSType::TSParenthesizedType(parenthesized) => {
            vue3_indexed_access_is_string_key(&parenthesized.type_annotation, analysis)
        }
        TSType::TSTypeReference(reference) => vue3_ts_type_name_key(&reference.type_name)
            .and_then(|name| analysis.declared_types.get(&name))
            .is_some_and(|types| types.len() == 1 && types[0] == "String"),
        _ => false,
    }
}

pub(crate) fn vue3_resolve_props_type_from_runtime_prop(
    prop: &Vue27RuntimeProp,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vue27TypeMembers> {
    let type_source = prop
        .type_annotation_source
        .as_deref()
        .and_then(|annotation| annotation.strip_prefix(':').map(str::trim))
        .or(prop.type_annotation_source.as_deref())
        .map(str::trim)?;
    if type_source.is_empty() {
        return None;
    }
    let wrapped = format!("type __VuecResolved = {type_source}");
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, &wrapped, oxc_span::SourceType::ts())
        .with_options(oxc_parser::ParseOptions {
            parse_regular_expression: true,
            ..oxc_parser::ParseOptions::default()
        })
        .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return None;
    }
    for statement in &parsed.program.body {
        if let Statement::TSTypeAliasDeclaration(declaration) = statement {
            return vue3_resolve_props_type(&wrapped, &declaration.type_annotation, analysis);
        }
    }
    None
}

pub(crate) fn vue3_resolve_string_type_keys(
    ty: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<BTreeSet<String>> {
    Some(
        vue3_resolve_ordered_string_type_keys(ty, analysis)?
            .into_iter()
            .collect(),
    )
}

pub(crate) fn vue3_resolve_ordered_string_type_keys(
    ty: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vec<String>> {
    match ty {
        TSType::TSLiteralType(literal) => {
            vue3_literal_type_key(&literal.literal).map(|key| vec![key])
        }
        TSType::TSUnionType(union) => {
            let mut keys = Vec::new();
            for ty in &union.types {
                for key in vue3_resolve_ordered_string_type_keys(ty, analysis)? {
                    push_unique(&mut keys, &key);
                }
            }
            Some(keys)
        }
        TSType::TSTemplateLiteralType(template) => {
            vue3_resolve_template_literal_type_keys(template, analysis)
        }
        TSType::TSParenthesizedType(parenthesized) => {
            vue3_resolve_ordered_string_type_keys(&parenthesized.type_annotation, analysis)
        }
        TSType::TSTypeReference(reference) => {
            let name = vue3_ts_type_name_key(&reference.type_name)?;
            match name.as_str() {
                "Extract" => {
                    let source = vue3_type_reference_type_argument(reference, 0)?;
                    let filter = vue3_type_reference_type_argument(reference, 1)?;
                    let source_keys = vue3_resolve_ordered_string_type_keys(source, analysis)?;
                    let filter_keys = vue3_resolve_string_type_keys(filter, analysis)?;
                    Some(
                        source_keys
                            .into_iter()
                            .filter(|key| filter_keys.contains(key))
                            .collect(),
                    )
                }
                "Exclude" => {
                    let source = vue3_type_reference_type_argument(reference, 0)?;
                    let excluded = vue3_type_reference_type_argument(reference, 1)?;
                    let source_keys = vue3_resolve_ordered_string_type_keys(source, analysis)?;
                    let excluded_keys = vue3_resolve_string_type_keys(excluded, analysis)?;
                    Some(
                        source_keys
                            .into_iter()
                            .filter(|key| !excluded_keys.contains(key))
                            .collect(),
                    )
                }
                "Uppercase" | "Lowercase" | "Capitalize" | "Uncapitalize" => {
                    let ty = vue3_type_reference_first_type_argument(reference)?;
                    let mut keys = Vec::new();
                    for key in vue3_resolve_ordered_string_type_keys(ty, analysis)? {
                        let mapped = vue3_string_mapping_type(name.as_str(), &key);
                        push_unique(&mut keys, &mapped);
                    }
                    Some(keys)
                }
                _ => analysis
                    .ordered_string_literal_type_declarations
                    .get(&name)
                    .cloned()
                    .or_else(|| {
                        analysis
                            .string_literal_type_declarations
                            .get(&name)
                            .map(|keys| keys.iter().cloned().collect())
                    }),
            }
        }
        TSType::TSTypeOperatorType(operator)
            if operator.operator == TSTypeOperatorOperator::Keyof =>
        {
            let members = vue3_resolve_props_type("", &operator.type_annotation, analysis)?;
            let mut keys = Vec::new();
            for prop in members.members {
                push_unique(&mut keys, &prop.key);
            }
            Some(keys)
        }
        TSType::TSImportType(import_type) => {
            let resolved = vue3_resolve_import_type(import_type, analysis)?;
            resolved
                .context
                .ordered_string_literal_type_declarations
                .get(&resolved.name)
                .cloned()
                .or_else(|| {
                    resolved
                        .context
                        .string_literal_type_declarations
                        .get(&resolved.name)
                        .map(|keys| keys.iter().cloned().collect())
                })
        }
        _ => None,
    }
}

pub(crate) fn vue3_resolve_template_literal_type_keys(
    template: &TSTemplateLiteralType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vec<String>> {
    let mut values = vec![vue3_template_type_quasi_value(template.quasis.first()?)];
    for (index, ty) in template.types.iter().enumerate() {
        let keys = vue3_resolve_ordered_string_type_keys(ty, analysis)?;
        let suffix = vue3_template_type_quasi_value(template.quasis.get(index + 1)?);
        let mut next = Vec::new();
        for prefix in &values {
            for key in &keys {
                next.push(format!("{prefix}{key}{suffix}"));
            }
        }
        values = next;
    }
    Some(values)
}

pub(crate) fn vue3_template_type_quasi_value(quasi: &oxc_ast::ast::TemplateElement<'_>) -> String {
    quasi
        .value
        .cooked
        .as_ref()
        .map(|value| value.as_str().to_string())
        .unwrap_or_else(|| quasi.value.raw.as_str().to_string())
}

pub(crate) fn vue3_string_mapping_type(name: &str, value: &str) -> String {
    match name {
        "Uppercase" => value.to_uppercase(),
        "Lowercase" => value.to_lowercase(),
        "Capitalize" => {
            let mut chars = value.chars();
            let Some(first) = chars.next() else {
                return String::new();
            };
            let mut mapped = first.to_uppercase().collect::<String>();
            mapped.push_str(chars.as_str());
            mapped
        }
        "Uncapitalize" => {
            let mut chars = value.chars();
            let Some(first) = chars.next() else {
                return String::new();
            };
            let mut mapped = first.to_lowercase().collect::<String>();
            mapped.push_str(chars.as_str());
            mapped
        }
        _ => value.to_string(),
    }
}

pub(crate) fn vue3_literal_type_key(literal: &TSLiteral<'_>) -> Option<String> {
    match literal {
        TSLiteral::StringLiteral(literal) => Some(literal.value.to_string()),
        TSLiteral::NumericLiteral(literal) => Some(literal.value.to_string()),
        TSLiteral::BigIntLiteral(literal) => Some(literal.value.to_string()),
        _ => None,
    }
}

#[derive(Clone, Copy)]
pub(crate) enum Vue3ArrayElementRuntimeMode {
    Props,
    DefineModel,
}

pub(crate) fn infer_vue3_indexed_access_runtime_type(
    indexed: &oxc_ast::ast::TSIndexedAccessType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vec<String>> {
    if let Some(index) = vue3_indexed_access_runtime_index(&indexed.index_type, analysis) {
        match index {
            Vue3RuntimeIndex::Number => {
                if let Some(types) = infer_vue3_array_element_runtime_type(
                    &indexed.object_type,
                    analysis,
                    Vue3ArrayElementRuntimeMode::Props,
                ) {
                    return Some(types);
                }
            }
            Vue3RuntimeIndex::Numeric(index) => {
                if let Some(types) = infer_vue3_tuple_index_runtime_type(
                    &indexed.object_type,
                    index,
                    analysis,
                    Vue3ArrayElementRuntimeMode::Props,
                ) {
                    return Some(types);
                }
            }
        }
    }
    let members = vue3_resolve_props_type("", &indexed.object_type, analysis)?;
    let keys = vue3_indexed_access_member_keys(&indexed.index_type, &members, analysis)?;
    let mut types = Vec::new();
    for key in keys {
        let Some(prop) = members.members.iter().find(|prop| prop.key == key) else {
            continue;
        };
        if prop.is_method {
            push_unique(&mut types, "Unknown");
        } else {
            for runtime_type in &prop.types {
                push_unique(&mut types, runtime_type);
            }
        }
    }
    if types.is_empty() {
        None
    } else {
        Some(types)
    }
}

pub(crate) fn infer_vue3_define_model_indexed_access_runtime_type(
    indexed: &oxc_ast::ast::TSIndexedAccessType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vec<String>> {
    if let Some(index) = vue3_indexed_access_runtime_index(&indexed.index_type, analysis) {
        match index {
            Vue3RuntimeIndex::Number => {
                if let Some(types) = infer_vue3_array_element_runtime_type(
                    &indexed.object_type,
                    analysis,
                    Vue3ArrayElementRuntimeMode::DefineModel,
                ) {
                    return Some(types);
                }
            }
            Vue3RuntimeIndex::Numeric(index) => {
                if let Some(types) = infer_vue3_tuple_index_runtime_type(
                    &indexed.object_type,
                    index,
                    analysis,
                    Vue3ArrayElementRuntimeMode::DefineModel,
                ) {
                    return Some(types);
                }
            }
        }
    }
    infer_vue3_indexed_access_runtime_type(indexed, analysis)
}

#[derive(Clone, Copy)]
pub(crate) enum Vue3RuntimeIndex {
    Number,
    Numeric(usize),
}

pub(crate) fn vue3_indexed_access_runtime_index(
    index_type: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vue3RuntimeIndex> {
    match index_type {
        TSType::TSNumberKeyword(_) => Some(Vue3RuntimeIndex::Number),
        TSType::TSLiteralType(literal) => match &literal.literal {
            TSLiteral::NumericLiteral(literal)
                if literal.value.fract() == 0.0 && literal.value >= 0.0 =>
            {
                Some(Vue3RuntimeIndex::Numeric(literal.value as usize))
            }
            _ => None,
        },
        TSType::TSParenthesizedType(parenthesized) => {
            vue3_indexed_access_runtime_index(&parenthesized.type_annotation, analysis)
        }
        TSType::TSTypeReference(reference) => {
            let Some(name) = vue3_ts_type_name_key(&reference.type_name) else {
                return None;
            };
            if analysis
                .declared_types
                .get(&name)
                .is_some_and(|types| types.len() == 1 && types[0] == "Number")
            {
                Some(Vue3RuntimeIndex::Number)
            } else {
                None
            }
        }
        _ => None,
    }
}

pub(crate) fn infer_vue3_array_element_runtime_type(
    node: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vec<String>> {
    match node {
        TSType::TSArrayType(array) => vue3_non_empty_runtime_types(vue3_runtime_types_for_mode(
            &array.element_type,
            analysis,
            mode,
        )),
        TSType::TSTupleType(_) => {
            vue3_runtime_types_from_tuple(infer_vue3_tuple_runtime_type(node, analysis, mode)?)
        }
        TSType::TSNamedTupleMember(member) => {
            infer_vue3_tuple_element_runtime_type(&member.element_type, analysis, mode)
        }
        TSType::TSTypeReference(reference) => {
            let name = vue3_ts_type_name_key(&reference.type_name)?;
            if let Some(tuple) =
                infer_vue3_parameter_utility_tuple_runtime_type(&name, reference, analysis, mode)
            {
                return vue3_runtime_types_from_tuple(tuple);
            }
            if let Some(tuple) = vue3_tuple_declaration_for_mode(analysis, &name, mode) {
                return vue3_runtime_types_from_tuple(tuple);
            }
            if let Some(types) = match mode {
                Vue3ArrayElementRuntimeMode::Props => analysis
                    .array_element_runtime_type_declarations
                    .get(&name)
                    .cloned(),
                Vue3ArrayElementRuntimeMode::DefineModel => analysis
                    .define_model_array_element_runtime_type_declarations
                    .get(&name)
                    .cloned(),
            } {
                return Some(types);
            }
            match name.as_str() {
                "Array" | "ReadonlyArray" => {
                    let ty = vue3_type_reference_type_argument(reference, 0)?;
                    vue3_non_empty_runtime_types(vue3_runtime_types_for_mode(ty, analysis, mode))
                }
                _ => None,
            }
        }
        TSType::TSImportType(import_type) => {
            let resolved = vue3_resolve_import_type(import_type, analysis)?;
            if let Some(tuple) =
                vue3_tuple_declaration_for_context(&resolved.context, &resolved.name, mode)
            {
                return vue3_runtime_types_from_tuple(tuple);
            }
            match mode {
                Vue3ArrayElementRuntimeMode::Props => resolved
                    .context
                    .array_element_runtime_type_declarations
                    .get(&resolved.name)
                    .cloned(),
                Vue3ArrayElementRuntimeMode::DefineModel => resolved
                    .context
                    .define_model_array_element_runtime_type_declarations
                    .get(&resolved.name)
                    .cloned(),
            }
        }
        TSType::TSParenthesizedType(parenthesized) => {
            infer_vue3_array_element_runtime_type(&parenthesized.type_annotation, analysis, mode)
        }
        TSType::TSUnionType(union) => {
            let mut types = Vec::new();
            for ty in &union.types {
                for runtime_type in infer_vue3_array_element_runtime_type(ty, analysis, mode)? {
                    push_unique(&mut types, &runtime_type);
                }
            }
            vue3_non_empty_runtime_types(types)
        }
        TSType::TSIntersectionType(intersection) => {
            let mut types = Vec::new();
            for ty in &intersection.types {
                let Some(runtime_types) = infer_vue3_array_element_runtime_type(ty, analysis, mode)
                else {
                    continue;
                };
                for runtime_type in runtime_types {
                    if runtime_type != "Unknown" {
                        push_unique(&mut types, &runtime_type);
                    }
                }
            }
            vue3_non_empty_runtime_types(types)
        }
        _ => None,
    }
}

pub(crate) fn infer_vue3_tuple_index_runtime_type(
    node: &TSType<'_>,
    index: usize,
    analysis: &Vue3ScriptSetupAnalysis,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vec<String>> {
    let tuple = infer_vue3_tuple_runtime_type(node, analysis, mode)?;
    tuple
        .get(index)
        .cloned()
        .and_then(vue3_non_empty_runtime_types)
}

pub(crate) fn infer_vue3_tuple_runtime_type(
    node: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vue3RuntimeTypeTuple> {
    match node {
        TSType::TSTupleType(tuple) => {
            let mut elements = Vec::new();
            for element in &tuple.element_types {
                elements.push(infer_vue3_tuple_element_runtime_type(
                    element, analysis, mode,
                )?);
            }
            vue3_non_empty_runtime_tuple(elements)
        }
        TSType::TSNamedTupleMember(member) => Some(vec![infer_vue3_tuple_element_runtime_type(
            &member.element_type,
            analysis,
            mode,
        )?]),
        TSType::TSTypeReference(reference) => {
            let name = vue3_ts_type_name_key(&reference.type_name)?;
            infer_vue3_parameter_utility_tuple_runtime_type(&name, reference, analysis, mode)
                .or_else(|| vue3_tuple_declaration_for_mode(analysis, &name, mode))
        }
        TSType::TSImportType(import_type) => {
            let resolved = vue3_resolve_import_type(import_type, analysis)?;
            vue3_tuple_declaration_for_context(&resolved.context, &resolved.name, mode)
        }
        TSType::TSParenthesizedType(parenthesized) => {
            infer_vue3_tuple_runtime_type(&parenthesized.type_annotation, analysis, mode)
        }
        TSType::TSUnionType(union) => {
            let mut merged = Vec::new();
            for ty in &union.types {
                let tuple = infer_vue3_tuple_runtime_type(ty, analysis, mode)?;
                merge_vue3_runtime_type_tuple(&mut merged, tuple);
            }
            vue3_non_empty_runtime_tuple(merged)
        }
        TSType::TSIntersectionType(intersection) => {
            for ty in &intersection.types {
                if let Some(tuple) = infer_vue3_tuple_runtime_type(ty, analysis, mode) {
                    return Some(tuple);
                }
            }
            None
        }
        _ => None,
    }
}

pub(crate) fn infer_vue3_parameter_utility_tuple_runtime_type(
    name: &str,
    reference: &TSTypeReference<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vue3RuntimeTypeTuple> {
    let ty = vue3_type_reference_type_argument(reference, 0)?;
    match name {
        "Parameters" => infer_vue3_function_parameter_tuple_runtime_type(ty, analysis, mode),
        "ConstructorParameters" => {
            infer_vue3_constructor_parameter_tuple_runtime_type(ty, analysis, mode)
        }
        _ => None,
    }
}

pub(crate) fn infer_vue3_function_parameter_tuple_runtime_type(
    node: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vue3RuntimeTypeTuple> {
    match node {
        TSType::TSFunctionType(function) => {
            infer_vue3_formal_parameters_tuple_runtime_type(&function.params, analysis, mode)
        }
        TSType::TSTypeLiteral(literal) => {
            infer_vue3_function_parameter_tuple_runtime_type_from_signatures(
                &literal.members,
                analysis,
                mode,
            )
        }
        TSType::TSTypeReference(reference) => {
            let name = vue3_ts_type_name_key(&reference.type_name)?;
            vue3_parameter_tuple_declaration_for_mode(analysis, &name, mode)
        }
        TSType::TSImportType(import_type) => {
            let resolved = vue3_resolve_import_type(import_type, analysis)?;
            vue3_parameter_tuple_declaration_for_context(&resolved.context, &resolved.name, mode)
        }
        TSType::TSParenthesizedType(parenthesized) => {
            infer_vue3_function_parameter_tuple_runtime_type(
                &parenthesized.type_annotation,
                analysis,
                mode,
            )
        }
        TSType::TSUnionType(union) => {
            let mut merged = Vec::new();
            for ty in &union.types {
                let tuple = infer_vue3_function_parameter_tuple_runtime_type(ty, analysis, mode)?;
                merge_vue3_runtime_type_tuple(&mut merged, tuple);
            }
            vue3_non_empty_runtime_tuple(merged)
        }
        _ => None,
    }
}

pub(crate) fn infer_vue3_function_parameter_tuple_runtime_type_from_interfaces(
    source: &str,
    declarations: &[&TSInterfaceDeclaration<'_>],
    analysis: &Vue3ScriptSetupAnalysis,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vue3RuntimeTypeTuple> {
    let mut merged = Vec::new();
    for declaration in declarations {
        if let Some(tuple) = infer_vue3_function_parameter_tuple_runtime_type_from_signatures(
            &declaration.body.body,
            analysis,
            mode,
        ) {
            merge_vue3_runtime_type_tuple(&mut merged, tuple);
        }
        for heritage in &declaration.extends {
            if vue3_interface_heritage_has_vue_ignore(source, heritage) {
                continue;
            }
            if let Some(tuple) = infer_vue3_function_parameter_tuple_runtime_type_from_heritage(
                source, heritage, analysis, mode,
            ) {
                merge_vue3_runtime_type_tuple(&mut merged, tuple);
            }
        }
    }
    vue3_non_empty_runtime_tuple(merged)
}

pub(crate) fn infer_vue3_function_parameter_tuple_runtime_type_from_heritage(
    source: &str,
    heritage: &TSInterfaceHeritage<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vue3RuntimeTypeTuple> {
    let ty_source = vue3_interface_heritage_type_source(source, heritage)?;
    let wrapped = format!("type __VuecResolved = {ty_source}");
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, &wrapped, oxc_span::SourceType::ts())
        .with_options(oxc_parser::ParseOptions {
            parse_regular_expression: true,
            ..oxc_parser::ParseOptions::default()
        })
        .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return None;
    }
    for statement in &parsed.program.body {
        if let Statement::TSTypeAliasDeclaration(declaration) = statement {
            return infer_vue3_function_parameter_tuple_runtime_type(
                &declaration.type_annotation,
                analysis,
                mode,
            );
        }
    }
    None
}

pub(crate) fn infer_vue3_function_parameter_tuple_runtime_type_from_signatures(
    signatures: &[TSSignature<'_>],
    analysis: &Vue3ScriptSetupAnalysis,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vue3RuntimeTypeTuple> {
    let mut merged = Vec::new();
    for signature in signatures {
        if let TSSignature::TSCallSignatureDeclaration(signature) = signature {
            let tuple =
                infer_vue3_formal_parameters_tuple_runtime_type(&signature.params, analysis, mode)?;
            merge_vue3_runtime_type_tuple(&mut merged, tuple);
        }
    }
    vue3_non_empty_runtime_tuple(merged)
}

pub(crate) fn infer_vue3_constructor_parameter_tuple_runtime_type(
    node: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vue3RuntimeTypeTuple> {
    match node {
        TSType::TSConstructorType(constructor) => {
            infer_vue3_formal_parameters_tuple_runtime_type(&constructor.params, analysis, mode)
        }
        TSType::TSTypeLiteral(literal) => {
            infer_vue3_constructor_parameter_tuple_runtime_type_from_signatures(
                &literal.members,
                analysis,
                mode,
            )
        }
        TSType::TSTypeReference(reference) => {
            let name = vue3_ts_type_name_key(&reference.type_name)?;
            vue3_constructor_parameter_tuple_declaration_for_mode(analysis, &name, mode)
        }
        TSType::TSImportType(import_type) => {
            let resolved = vue3_resolve_import_type(import_type, analysis)?;
            vue3_constructor_parameter_tuple_declaration_for_context(
                &resolved.context,
                &resolved.name,
                mode,
            )
        }
        TSType::TSParenthesizedType(parenthesized) => {
            infer_vue3_constructor_parameter_tuple_runtime_type(
                &parenthesized.type_annotation,
                analysis,
                mode,
            )
        }
        TSType::TSUnionType(union) => {
            let mut merged = Vec::new();
            for ty in &union.types {
                let tuple =
                    infer_vue3_constructor_parameter_tuple_runtime_type(ty, analysis, mode)?;
                merge_vue3_runtime_type_tuple(&mut merged, tuple);
            }
            vue3_non_empty_runtime_tuple(merged)
        }
        _ => None,
    }
}

pub(crate) fn infer_vue3_constructor_parameter_tuple_runtime_type_from_interfaces(
    source: &str,
    declarations: &[&TSInterfaceDeclaration<'_>],
    analysis: &Vue3ScriptSetupAnalysis,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vue3RuntimeTypeTuple> {
    let mut merged = Vec::new();
    for declaration in declarations {
        if let Some(tuple) = infer_vue3_constructor_parameter_tuple_runtime_type_from_signatures(
            &declaration.body.body,
            analysis,
            mode,
        ) {
            merge_vue3_runtime_type_tuple(&mut merged, tuple);
        }
        for heritage in &declaration.extends {
            if vue3_interface_heritage_has_vue_ignore(source, heritage) {
                continue;
            }
            if let Some(tuple) = infer_vue3_constructor_parameter_tuple_runtime_type_from_heritage(
                source, heritage, analysis, mode,
            ) {
                merge_vue3_runtime_type_tuple(&mut merged, tuple);
            }
        }
    }
    vue3_non_empty_runtime_tuple(merged)
}

pub(crate) fn infer_vue3_constructor_parameter_tuple_runtime_type_from_heritage(
    source: &str,
    heritage: &TSInterfaceHeritage<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vue3RuntimeTypeTuple> {
    let ty_source = vue3_interface_heritage_type_source(source, heritage)?;
    let wrapped = format!("type __VuecResolved = {ty_source}");
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, &wrapped, oxc_span::SourceType::ts())
        .with_options(oxc_parser::ParseOptions {
            parse_regular_expression: true,
            ..oxc_parser::ParseOptions::default()
        })
        .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return None;
    }
    for statement in &parsed.program.body {
        if let Statement::TSTypeAliasDeclaration(declaration) = statement {
            return infer_vue3_constructor_parameter_tuple_runtime_type(
                &declaration.type_annotation,
                analysis,
                mode,
            );
        }
    }
    None
}

pub(crate) fn infer_vue3_constructor_parameter_tuple_runtime_type_from_signatures(
    signatures: &[TSSignature<'_>],
    analysis: &Vue3ScriptSetupAnalysis,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vue3RuntimeTypeTuple> {
    let mut merged = Vec::new();
    for signature in signatures {
        if let TSSignature::TSConstructSignatureDeclaration(signature) = signature {
            let tuple =
                infer_vue3_formal_parameters_tuple_runtime_type(&signature.params, analysis, mode)?;
            merge_vue3_runtime_type_tuple(&mut merged, tuple);
        }
    }
    vue3_non_empty_runtime_tuple(merged)
}

pub(crate) fn infer_vue3_return_runtime_type(
    node: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vec<String>> {
    match node {
        TSType::TSFunctionType(function) => vue3_non_empty_runtime_types(
            vue3_runtime_types_for_mode(&function.return_type.type_annotation, analysis, mode),
        ),
        TSType::TSTypeLiteral(literal) => {
            infer_vue3_return_runtime_type_from_signatures(&literal.members, analysis, mode)
        }
        TSType::TSTypeReference(reference) => {
            let name = vue3_ts_type_name_key(&reference.type_name)?;
            infer_vue3_generic_return_runtime_type(reference, analysis, mode)
                .or_else(|| vue3_return_type_declaration_for_mode(analysis, &name, mode))
        }
        TSType::TSImportType(import_type) => {
            let resolved = vue3_resolve_import_type(import_type, analysis)?;
            vue3_return_type_declaration_for_context(&resolved.context, &resolved.name, mode)
        }
        TSType::TSTypeQuery(query) => {
            vue3_return_type_declaration_for_type_query(query, analysis, mode)
        }
        TSType::TSParenthesizedType(parenthesized) => {
            infer_vue3_return_runtime_type(&parenthesized.type_annotation, analysis, mode)
        }
        TSType::TSUnionType(union) => {
            let mut types = Vec::new();
            for ty in &union.types {
                let runtime_types = infer_vue3_return_runtime_type(ty, analysis, mode)?;
                merge_vue3_runtime_types(&mut types, runtime_types);
            }
            vue3_non_empty_runtime_types(types)
        }
        TSType::TSIntersectionType(intersection) => {
            let mut types = Vec::new();
            for ty in &intersection.types {
                let Some(runtime_types) = infer_vue3_return_runtime_type(ty, analysis, mode) else {
                    continue;
                };
                merge_vue3_runtime_types(&mut types, runtime_types);
            }
            vue3_non_empty_runtime_types(types)
        }
        _ => None,
    }
}

pub(crate) fn infer_vue3_return_runtime_type_from_interfaces(
    source: &str,
    declarations: &[&TSInterfaceDeclaration<'_>],
    analysis: &Vue3ScriptSetupAnalysis,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vec<String>> {
    let mut types = Vec::new();
    for declaration in declarations {
        if let Some(runtime_types) =
            infer_vue3_return_runtime_type_from_signatures(&declaration.body.body, analysis, mode)
        {
            merge_vue3_runtime_types(&mut types, runtime_types);
        }
        for heritage in &declaration.extends {
            if vue3_interface_heritage_has_vue_ignore(source, heritage) {
                continue;
            }
            if let Some(runtime_types) =
                infer_vue3_return_runtime_type_from_heritage(source, heritage, analysis, mode)
            {
                merge_vue3_runtime_types(&mut types, runtime_types);
            }
        }
    }
    vue3_non_empty_runtime_types(types)
}

pub(crate) fn infer_vue3_return_runtime_type_from_heritage(
    source: &str,
    heritage: &TSInterfaceHeritage<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vec<String>> {
    let ty_source = vue3_interface_heritage_type_source(source, heritage)?;
    let wrapped = format!("type __VuecResolved = {ty_source}");
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, &wrapped, oxc_span::SourceType::ts())
        .with_options(oxc_parser::ParseOptions {
            parse_regular_expression: true,
            ..oxc_parser::ParseOptions::default()
        })
        .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return None;
    }
    for statement in &parsed.program.body {
        if let Statement::TSTypeAliasDeclaration(declaration) = statement {
            return infer_vue3_return_runtime_type(&declaration.type_annotation, analysis, mode);
        }
    }
    None
}

pub(crate) fn infer_vue3_return_runtime_type_from_signatures(
    signatures: &[TSSignature<'_>],
    analysis: &Vue3ScriptSetupAnalysis,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vec<String>> {
    let mut types = Vec::new();
    for signature in signatures {
        if let TSSignature::TSCallSignatureDeclaration(signature) = signature {
            let runtime_types = signature
                .return_type
                .as_ref()
                .map(|annotation| {
                    vue3_non_empty_runtime_types(vue3_runtime_types_for_mode(
                        &annotation.type_annotation,
                        analysis,
                        mode,
                    ))
                })
                .unwrap_or_else(|| Some(vec!["Unknown".into()]))?;
            merge_vue3_runtime_types(&mut types, runtime_types);
        }
    }
    vue3_non_empty_runtime_types(types)
}

pub(crate) fn infer_vue3_formal_parameters_tuple_runtime_type(
    parameters: &FormalParameters<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vue3RuntimeTypeTuple> {
    let mut tuple = Vec::new();
    for parameter in &parameters.items {
        let runtime_types = parameter
            .type_annotation
            .as_ref()
            .map(|annotation| {
                vue3_non_empty_runtime_types(vue3_runtime_types_for_mode(
                    &annotation.type_annotation,
                    analysis,
                    mode,
                ))
            })
            .unwrap_or_else(|| Some(vec!["Unknown".into()]))?;
        tuple.push(runtime_types);
    }
    if let Some(rest) = parameters.rest.as_ref() {
        let Some(annotation) = rest.type_annotation.as_ref() else {
            tuple.push(vec!["Unknown".into()]);
            return vue3_non_empty_runtime_tuple(tuple);
        };
        let runtime_types =
            infer_vue3_array_element_runtime_type(&annotation.type_annotation, analysis, mode)
                .or_else(|| {
                    vue3_non_empty_runtime_types(vue3_runtime_types_for_mode(
                        &annotation.type_annotation,
                        analysis,
                        mode,
                    ))
                })?;
        tuple.push(runtime_types);
    }
    vue3_non_empty_runtime_tuple(tuple)
}

pub(crate) fn vue3_tuple_declaration_for_mode(
    analysis: &Vue3ScriptSetupAnalysis,
    name: &str,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vue3RuntimeTypeTuple> {
    match mode {
        Vue3ArrayElementRuntimeMode::Props => {
            analysis.tuple_runtime_type_declarations.get(name).cloned()
        }
        Vue3ArrayElementRuntimeMode::DefineModel => analysis
            .define_model_tuple_runtime_type_declarations
            .get(name)
            .cloned(),
    }
}

pub(crate) fn vue3_tuple_declaration_for_context(
    context: &Vue27TypeContext,
    name: &str,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vue3RuntimeTypeTuple> {
    match mode {
        Vue3ArrayElementRuntimeMode::Props => {
            context.tuple_runtime_type_declarations.get(name).cloned()
        }
        Vue3ArrayElementRuntimeMode::DefineModel => context
            .define_model_tuple_runtime_type_declarations
            .get(name)
            .cloned(),
    }
}

pub(crate) fn vue3_parameter_tuple_declaration_for_mode(
    analysis: &Vue3ScriptSetupAnalysis,
    name: &str,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vue3RuntimeTypeTuple> {
    match mode {
        Vue3ArrayElementRuntimeMode::Props => analysis
            .parameter_tuple_runtime_type_declarations
            .get(name)
            .cloned(),
        Vue3ArrayElementRuntimeMode::DefineModel => analysis
            .define_model_parameter_tuple_runtime_type_declarations
            .get(name)
            .cloned(),
    }
}

pub(crate) fn vue3_parameter_tuple_declaration_for_context(
    context: &Vue27TypeContext,
    name: &str,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vue3RuntimeTypeTuple> {
    match mode {
        Vue3ArrayElementRuntimeMode::Props => context
            .parameter_tuple_runtime_type_declarations
            .get(name)
            .cloned(),
        Vue3ArrayElementRuntimeMode::DefineModel => context
            .define_model_parameter_tuple_runtime_type_declarations
            .get(name)
            .cloned(),
    }
}

pub(crate) fn vue3_constructor_parameter_tuple_declaration_for_mode(
    analysis: &Vue3ScriptSetupAnalysis,
    name: &str,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vue3RuntimeTypeTuple> {
    match mode {
        Vue3ArrayElementRuntimeMode::Props => analysis
            .constructor_parameter_tuple_runtime_type_declarations
            .get(name)
            .cloned(),
        Vue3ArrayElementRuntimeMode::DefineModel => analysis
            .define_model_constructor_parameter_tuple_runtime_type_declarations
            .get(name)
            .cloned(),
    }
}

pub(crate) fn vue3_constructor_parameter_tuple_declaration_for_context(
    context: &Vue27TypeContext,
    name: &str,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vue3RuntimeTypeTuple> {
    match mode {
        Vue3ArrayElementRuntimeMode::Props => context
            .constructor_parameter_tuple_runtime_type_declarations
            .get(name)
            .cloned(),
        Vue3ArrayElementRuntimeMode::DefineModel => context
            .define_model_constructor_parameter_tuple_runtime_type_declarations
            .get(name)
            .cloned(),
    }
}

pub(crate) fn vue3_return_type_declaration_for_mode(
    analysis: &Vue3ScriptSetupAnalysis,
    name: &str,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vec<String>> {
    match mode {
        Vue3ArrayElementRuntimeMode::Props => analysis
            .return_type_runtime_type_declarations
            .get(name)
            .cloned(),
        Vue3ArrayElementRuntimeMode::DefineModel => analysis
            .define_model_return_type_runtime_type_declarations
            .get(name)
            .cloned(),
    }
}

pub(crate) fn vue3_return_type_declaration_for_context(
    context: &Vue27TypeContext,
    name: &str,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vec<String>> {
    match mode {
        Vue3ArrayElementRuntimeMode::Props => context
            .return_type_runtime_type_declarations
            .get(name)
            .cloned(),
        Vue3ArrayElementRuntimeMode::DefineModel => context
            .define_model_return_type_runtime_type_declarations
            .get(name)
            .cloned(),
    }
}

pub(crate) fn merge_vue3_runtime_type_tuple(
    target: &mut Vue3RuntimeTypeTuple,
    source: Vue3RuntimeTypeTuple,
) {
    if target.len() < source.len() {
        target.resize_with(source.len(), Vec::new);
    }
    for (index, element) in source.into_iter().enumerate() {
        for runtime_type in element {
            push_unique(&mut target[index], &runtime_type);
        }
    }
}

pub(crate) fn merge_vue3_runtime_types(target: &mut Vec<String>, source: Vec<String>) {
    for runtime_type in source {
        push_unique(target, &runtime_type);
    }
}

pub(crate) fn infer_vue3_tuple_element_runtime_type(
    element: &TSTupleElement<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vec<String>> {
    match element {
        TSTupleElement::TSOptionalType(optional) => vue3_non_empty_runtime_types(
            vue3_runtime_types_for_mode(&optional.type_annotation, analysis, mode),
        ),
        TSTupleElement::TSRestType(rest) => {
            infer_vue3_array_element_runtime_type(&rest.type_annotation, analysis, mode).or_else(
                || {
                    vue3_non_empty_runtime_types(vue3_runtime_types_for_mode(
                        &rest.type_annotation,
                        analysis,
                        mode,
                    ))
                },
            )
        }
        TSTupleElement::TSNamedTupleMember(member) => {
            infer_vue3_tuple_element_runtime_type(&member.element_type, analysis, mode)
        }
        _ => {
            let ty = element.as_ts_type()?;
            vue3_non_empty_runtime_types(vue3_runtime_types_for_mode(ty, analysis, mode))
        }
    }
}

pub(crate) fn vue3_runtime_types_for_mode(
    node: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    mode: Vue3ArrayElementRuntimeMode,
) -> Vec<String> {
    match mode {
        Vue3ArrayElementRuntimeMode::Props => infer_vue3_runtime_type(node, analysis),
        Vue3ArrayElementRuntimeMode::DefineModel => {
            infer_vue3_define_model_runtime_type(node, analysis)
        }
    }
}

pub(crate) fn vue3_non_empty_runtime_types(types: Vec<String>) -> Option<Vec<String>> {
    if types.is_empty() {
        None
    } else {
        Some(types)
    }
}

pub(crate) fn vue3_non_empty_runtime_tuple(
    tuple: Vue3RuntimeTypeTuple,
) -> Option<Vue3RuntimeTypeTuple> {
    if tuple.is_empty() {
        None
    } else {
        Some(tuple)
    }
}

pub(crate) fn vue3_runtime_types_from_tuple(tuple: Vue3RuntimeTypeTuple) -> Option<Vec<String>> {
    let mut types = Vec::new();
    for element in tuple {
        for runtime_type in element {
            push_unique(&mut types, &runtime_type);
        }
    }
    vue3_non_empty_runtime_types(types)
}

pub(crate) fn vue3_scoped_analysis_for_generic_type_alias(
    source: &str,
    reference: &TSTypeReference<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<(String, Vue3ScriptSetupAnalysis)> {
    let name = vue3_ts_type_name_key(&reference.type_name)?;
    let alias = analysis.generic_type_aliases.get(&name)?;
    let type_arguments = reference.type_arguments.as_ref()?;
    if type_arguments.params.is_empty() {
        return None;
    }
    let mut scoped_analysis = analysis.clone();
    scoped_analysis
        .declared_types
        .extend(alias.declared_types.clone());
    scoped_analysis
        .define_model_declared_types
        .extend(alias.define_model_declared_types.clone());
    scoped_analysis
        .type_query_declared_types
        .extend(alias.type_query_declared_types.clone());
    scoped_analysis
        .define_model_type_query_declared_types
        .extend(alias.define_model_type_query_declared_types.clone());
    scoped_analysis
        .keyof_type_query_declared_types
        .extend(alias.keyof_type_query_declared_types.clone());
    scoped_analysis
        .props_type_declarations
        .extend(alias.props_type_declarations.clone());
    scoped_analysis
        .keyof_runtime_type_declarations
        .extend(alias.keyof_runtime_type_declarations.clone());
    scoped_analysis
        .tuple_runtime_type_declarations
        .extend(alias.tuple_runtime_type_declarations.clone());
    scoped_analysis
        .define_model_tuple_runtime_type_declarations
        .extend(alias.define_model_tuple_runtime_type_declarations.clone());
    scoped_analysis
        .array_element_runtime_type_declarations
        .extend(alias.array_element_runtime_type_declarations.clone());
    scoped_analysis
        .define_model_array_element_runtime_type_declarations
        .extend(
            alias
                .define_model_array_element_runtime_type_declarations
                .clone(),
        );
    scoped_analysis
        .parameter_tuple_runtime_type_declarations
        .extend(alias.parameter_tuple_runtime_type_declarations.clone());
    scoped_analysis
        .define_model_parameter_tuple_runtime_type_declarations
        .extend(
            alias
                .define_model_parameter_tuple_runtime_type_declarations
                .clone(),
        );
    scoped_analysis
        .constructor_parameter_tuple_runtime_type_declarations
        .extend(
            alias
                .constructor_parameter_tuple_runtime_type_declarations
                .clone(),
        );
    scoped_analysis
        .define_model_constructor_parameter_tuple_runtime_type_declarations
        .extend(
            alias
                .define_model_constructor_parameter_tuple_runtime_type_declarations
                .clone(),
        );
    scoped_analysis
        .return_type_runtime_type_declarations
        .extend(alias.return_type_runtime_type_declarations.clone());
    scoped_analysis
        .define_model_return_type_runtime_type_declarations
        .extend(
            alias
                .define_model_return_type_runtime_type_declarations
                .clone(),
        );
    scoped_analysis
        .props_options_type_declarations
        .extend(alias.props_options_type_declarations.clone());
    scoped_analysis
        .return_type_props_options_declarations
        .extend(alias.return_type_props_options_declarations.clone());
    scoped_analysis
        .string_literal_type_declarations
        .extend(alias.string_literal_type_declarations.clone());
    scoped_analysis
        .ordered_string_literal_type_declarations
        .extend(alias.ordered_string_literal_type_declarations.clone());
    scoped_analysis
        .unresolved_import_sources
        .extend(alias.unresolved_import_sources.clone());
    scoped_analysis
        .silent_unresolved_type_names
        .extend(alias.silent_unresolved_type_names.clone());
    scoped_analysis.generic_type_aliases.remove(&name);
    scoped_analysis
        .generic_type_parameter_names
        .extend(alias.params.iter().cloned());
    for (index, param) in alias.params.iter().enumerate() {
        let Some(argument) = type_arguments.params.get(index) else {
            continue;
        };
        if let Some(props) = vue3_resolve_props_type(source, argument, analysis) {
            scoped_analysis
                .props_type_declarations
                .insert(param.clone(), props);
        }
        if let Some(props_options) = vue3_resolve_props_options_type(source, argument, analysis) {
            scoped_analysis
                .props_options_type_declarations
                .insert(param.clone(), props_options);
        }
        if let Some(keys) = vue3_resolve_string_type_keys(argument, analysis) {
            scoped_analysis
                .string_literal_type_declarations
                .insert(param.clone(), keys);
        }
        if let Some(keys) = vue3_resolve_ordered_string_type_keys(argument, analysis) {
            scoped_analysis
                .ordered_string_literal_type_declarations
                .insert(param.clone(), keys);
        }
        if let Some(types) = infer_vue3_keyof_runtime_type(argument, analysis) {
            scoped_analysis
                .keyof_runtime_type_declarations
                .insert(param.clone(), types);
        }
        if let Some(tuple) =
            infer_vue3_tuple_runtime_type(argument, analysis, Vue3ArrayElementRuntimeMode::Props)
        {
            scoped_analysis
                .tuple_runtime_type_declarations
                .insert(param.clone(), tuple);
        }
        if let Some(tuple) = infer_vue3_tuple_runtime_type(
            argument,
            analysis,
            Vue3ArrayElementRuntimeMode::DefineModel,
        ) {
            scoped_analysis
                .define_model_tuple_runtime_type_declarations
                .insert(param.clone(), tuple);
        }
        if let Some(types) = infer_vue3_array_element_runtime_type(
            argument,
            analysis,
            Vue3ArrayElementRuntimeMode::Props,
        ) {
            scoped_analysis
                .array_element_runtime_type_declarations
                .insert(param.clone(), types);
        }
        if let Some(types) = infer_vue3_array_element_runtime_type(
            argument,
            analysis,
            Vue3ArrayElementRuntimeMode::DefineModel,
        ) {
            scoped_analysis
                .define_model_array_element_runtime_type_declarations
                .insert(param.clone(), types);
        }
        if let Some(tuple) = infer_vue3_function_parameter_tuple_runtime_type(
            argument,
            analysis,
            Vue3ArrayElementRuntimeMode::Props,
        ) {
            scoped_analysis
                .parameter_tuple_runtime_type_declarations
                .insert(param.clone(), tuple);
        }
        if let Some(tuple) = infer_vue3_function_parameter_tuple_runtime_type(
            argument,
            analysis,
            Vue3ArrayElementRuntimeMode::DefineModel,
        ) {
            scoped_analysis
                .define_model_parameter_tuple_runtime_type_declarations
                .insert(param.clone(), tuple);
        }
        if let Some(tuple) = infer_vue3_constructor_parameter_tuple_runtime_type(
            argument,
            analysis,
            Vue3ArrayElementRuntimeMode::Props,
        ) {
            scoped_analysis
                .constructor_parameter_tuple_runtime_type_declarations
                .insert(param.clone(), tuple);
        }
        if let Some(tuple) = infer_vue3_constructor_parameter_tuple_runtime_type(
            argument,
            analysis,
            Vue3ArrayElementRuntimeMode::DefineModel,
        ) {
            scoped_analysis
                .define_model_constructor_parameter_tuple_runtime_type_declarations
                .insert(param.clone(), tuple);
        }
        if let Some(types) =
            infer_vue3_return_runtime_type(argument, analysis, Vue3ArrayElementRuntimeMode::Props)
        {
            scoped_analysis
                .return_type_runtime_type_declarations
                .insert(param.clone(), types);
        }
        if let Some(types) = infer_vue3_return_runtime_type(
            argument,
            analysis,
            Vue3ArrayElementRuntimeMode::DefineModel,
        ) {
            scoped_analysis
                .define_model_return_type_runtime_type_declarations
                .insert(param.clone(), types);
        }
        scoped_analysis
            .declared_types
            .insert(param.clone(), infer_vue3_runtime_type(argument, analysis));
        scoped_analysis.define_model_declared_types.insert(
            param.clone(),
            infer_vue3_define_model_runtime_type(argument, analysis),
        );
        scoped_analysis
            .type_query_declared_types
            .insert(param.clone(), infer_vue3_runtime_type(argument, analysis));
        scoped_analysis
            .define_model_type_query_declared_types
            .insert(
                param.clone(),
                infer_vue3_define_model_runtime_type(argument, analysis),
            );
        if let Some(types) = infer_vue3_keyof_runtime_type(argument, analysis) {
            scoped_analysis
                .keyof_type_query_declared_types
                .insert(param.clone(), types);
        }
    }
    Some((alias.source.clone(), scoped_analysis))
}

pub(crate) fn vue3_resolve_generic_props_type_alias(
    source: &str,
    reference: &TSTypeReference<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vue27TypeMembers> {
    let (alias_source, scoped_analysis) =
        vue3_scoped_analysis_for_generic_type_alias(source, reference, analysis)?;
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        alias_source.as_str(),
        oxc_span::SourceType::ts(),
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
        match statement {
            Statement::TSTypeAliasDeclaration(declaration) => {
                return vue3_resolve_props_type(
                    alias_source.as_str(),
                    &declaration.type_annotation,
                    &scoped_analysis,
                );
            }
            Statement::TSInterfaceDeclaration(declaration) => {
                return Some(vue3_type_members_from_interface(
                    alias_source.as_str(),
                    declaration,
                    &scoped_analysis,
                ));
            }
            _ => {}
        }
    }
    None
}

pub(crate) fn infer_vue3_generic_type_alias_runtime_type(
    reference: &TSTypeReference<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vec<String>> {
    let (alias_source, scoped_analysis) =
        vue3_scoped_analysis_for_generic_type_alias("", reference, analysis)?;
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        alias_source.as_str(),
        oxc_span::SourceType::ts(),
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
        match statement {
            Statement::TSTypeAliasDeclaration(declaration) => {
                return Some(infer_vue3_runtime_type(
                    &declaration.type_annotation,
                    &scoped_analysis,
                ));
            }
            Statement::TSInterfaceDeclaration(declaration) => {
                return Some(infer_vue3_runtime_type_from_interface_declarations(&[
                    declaration,
                ]));
            }
            _ => {}
        }
    }
    None
}

pub(crate) fn infer_vue3_generic_define_model_runtime_type(
    reference: &TSTypeReference<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vec<String>> {
    let (alias_source, scoped_analysis) =
        vue3_scoped_analysis_for_generic_type_alias("", reference, analysis)?;
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        alias_source.as_str(),
        oxc_span::SourceType::ts(),
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
        match statement {
            Statement::TSTypeAliasDeclaration(declaration) => {
                return Some(infer_vue3_define_model_runtime_type(
                    &declaration.type_annotation,
                    &scoped_analysis,
                ));
            }
            Statement::TSInterfaceDeclaration(declaration) => {
                return Some(infer_vue3_runtime_type_from_interface_declarations(&[
                    declaration,
                ]));
            }
            _ => {}
        }
    }
    None
}

pub(crate) fn vue3_props_options_type_members(
    source: &str,
    ty: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vue27TypeMembers> {
    match ty {
        TSType::TSTypeLiteral(literal) => Some(Vue27TypeMembers {
            source: source
                .get(literal.span.start as usize..literal.span.end as usize)
                .unwrap_or_default()
                .to_string(),
            members: vue3_runtime_props_options_from_signatures(source, &literal.members, analysis),
            errors: Vec::new(),
        }),
        TSType::TSTypeReference(reference) => {
            let name = vue3_ts_type_name_key(&reference.type_name)?;
            analysis.props_options_type_declarations.get(&name).cloned()
        }
        TSType::TSImportType(import_type) => {
            let resolved = vue3_resolve_import_type(import_type, analysis)?;
            resolved
                .context
                .props_options_type_declarations
                .get(&resolved.name)
                .cloned()
        }
        TSType::TSParenthesizedType(parenthesized) => {
            vue3_props_options_type_members(source, &parenthesized.type_annotation, analysis)
        }
        _ => None,
    }
}

pub(crate) fn vue3_runtime_props_options_from_signatures(
    source: &str,
    signatures: &[TSSignature<'_>],
    analysis: &Vue3ScriptSetupAnalysis,
) -> Vec<Vue27RuntimeProp> {
    let mut props = Vec::new();
    for signature in signatures {
        let TSSignature::TSPropertySignature(property) = signature else {
            continue;
        };
        if property.computed {
            continue;
        }
        let Some(key) = vue27_property_key_static_name(&property.key) else {
            continue;
        };
        let Some(type_annotation) = property.type_annotation.as_ref() else {
            continue;
        };
        let (types, required) =
            vue3_reverse_infer_runtime_prop_option_type(&type_annotation.type_annotation, analysis)
                .unwrap_or_else(|| (vec!["null".into()], false));
        props.push(Vue27RuntimeProp {
            key,
            types,
            required,
            default: None,
            is_method: false,
            type_annotation_source: source
                .get(type_annotation.span.start as usize..type_annotation.span.end as usize)
                .map(ToOwned::to_owned),
            member_source: source
                .get(property.span.start as usize..property.span.end as usize)
                .map(ToOwned::to_owned),
        });
    }
    props
}

pub(crate) fn vue3_static_runtime_props_options_type_members(
    source: &str,
    expression: &Expression<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vue27TypeMembers> {
    let object = vue3_static_runtime_props_options_object(expression)?;
    let mut members = Vec::new();
    for property in &object.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            return None;
        };
        if property.computed {
            return None;
        }
        let prop = vue3_static_runtime_prop_from_object_property(source, property, analysis)?;
        members.push(prop);
    }
    if members.is_empty() {
        return None;
    }
    Some(Vue27TypeMembers {
        source: source
            .get(object.span.start as usize..object.span.end as usize)
            .unwrap_or_default()
            .to_string(),
        members,
        errors: Vec::new(),
    })
}

pub(crate) fn vue3_static_runtime_prop_from_object_property(
    source: &str,
    property: &ObjectProperty<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vue27RuntimeProp> {
    let key = vue27_property_key_static_name(&property.key)?;
    let (types, required, type_annotation_source) =
        vue3_static_runtime_prop_option_expression(source, &property.value, analysis)?;
    Some(Vue27RuntimeProp {
        key,
        types,
        required,
        default: None,
        is_method: false,
        type_annotation_source,
        member_source: source
            .get(property.span.start as usize..property.span.end as usize)
            .map(ToOwned::to_owned),
    })
}

pub(crate) fn vue3_static_runtime_prop_option_expression(
    source: &str,
    expression: &Expression<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<(Vec<String>, bool, Option<String>)> {
    if let Some((types, type_annotation_source)) =
        vue3_static_runtime_prop_type_expression(source, expression, analysis)
    {
        return Some((types, false, type_annotation_source));
    }

    let object = vue3_static_runtime_props_options_object(expression)?;
    let mut has_runtime_option_key = false;
    let mut types = None;
    let mut type_annotation_source = None;
    let mut required = false;
    for property in &object.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            return None;
        };
        if property.computed {
            return None;
        }
        let key = vue27_property_key_static_name(&property.key)?;
        match key.as_str() {
            "type" => {
                has_runtime_option_key = true;
                if let Some((resolved, source)) =
                    vue3_static_runtime_prop_type_expression(source, &property.value, analysis)
                {
                    types = Some(resolved);
                    type_annotation_source = source;
                }
            }
            "required" => {
                has_runtime_option_key = true;
                required = vue3_static_boolean_expression(&property.value).unwrap_or(false);
            }
            "default" | "validator" => {
                has_runtime_option_key = true;
            }
            _ => {}
        }
    }

    if !has_runtime_option_key {
        return None;
    }
    Some((
        types.unwrap_or_else(|| vec!["null".into()]),
        required,
        type_annotation_source,
    ))
}

pub(crate) fn vue3_static_runtime_prop_type_expression(
    source: &str,
    expression: &Expression<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<(Vec<String>, Option<String>)> {
    match expression {
        Expression::TSAsExpression(expression) => {
            if let Some(types) =
                vue3_reverse_infer_runtime_prop_type(&expression.type_annotation, analysis)
            {
                return Some((
                    types,
                    source
                        .get(
                            expression.type_annotation.span().start as usize
                                ..expression.type_annotation.span().end as usize,
                        )
                        .map(ToOwned::to_owned),
                ));
            }
            vue3_static_runtime_prop_type_expression(source, &expression.expression, analysis)
        }
        Expression::TSTypeAssertion(expression) => {
            if let Some(types) =
                vue3_reverse_infer_runtime_prop_type(&expression.type_annotation, analysis)
            {
                return Some((
                    types,
                    source
                        .get(
                            expression.type_annotation.span().start as usize
                                ..expression.type_annotation.span().end as usize,
                        )
                        .map(ToOwned::to_owned),
                ));
            }
            vue3_static_runtime_prop_type_expression(source, &expression.expression, analysis)
        }
        Expression::TSSatisfiesExpression(expression) => {
            if let Some(types) =
                vue3_reverse_infer_runtime_prop_type(&expression.type_annotation, analysis)
            {
                return Some((
                    types,
                    source
                        .get(
                            expression.type_annotation.span().start as usize
                                ..expression.type_annotation.span().end as usize,
                        )
                        .map(ToOwned::to_owned),
                ));
            }
            vue3_static_runtime_prop_type_expression(source, &expression.expression, analysis)
        }
        Expression::TSNonNullExpression(expression) => {
            vue3_static_runtime_prop_type_expression(source, &expression.expression, analysis)
        }
        Expression::TSInstantiationExpression(expression) => {
            vue3_static_runtime_prop_type_expression(source, &expression.expression, analysis)
        }
        Expression::ParenthesizedExpression(expression) => {
            vue3_static_runtime_prop_type_expression(source, &expression.expression, analysis)
        }
        Expression::Identifier(identifier) => {
            let name = vue3_return_expression_constructor_runtime_name(identifier.name.as_str())?;
            Some((vec![name.to_string()], None))
        }
        Expression::StaticMemberExpression(member) => {
            let name =
                vue3_return_expression_constructor_runtime_name(member.property.name.as_str())?;
            Some((vec![name.to_string()], None))
        }
        Expression::NullLiteral(_) => Some((vec!["null".into()], None)),
        Expression::ArrayExpression(array) => {
            let mut types = Vec::new();
            let mut type_annotation_source = None;
            for element in &array.elements {
                let expression = element.as_expression()?;
                let (element_types, element_type_annotation_source) =
                    vue3_static_runtime_prop_type_expression(source, expression, analysis)?;
                merge_vue3_runtime_types(&mut types, element_types);
                if type_annotation_source.is_none() {
                    type_annotation_source = element_type_annotation_source;
                }
            }
            vue3_non_empty_runtime_types(types).map(|types| (types, type_annotation_source))
        }
        _ => None,
    }
}

pub(crate) fn vue3_static_boolean_expression(expression: &Expression<'_>) -> Option<bool> {
    match unwrap_vue3_ts_expression(expression) {
        Expression::BooleanLiteral(literal) => Some(literal.value),
        _ => None,
    }
}

pub(crate) fn vue3_reverse_infer_runtime_prop_option_type(
    ty: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<(Vec<String>, bool)> {
    match ty {
        TSType::TSTypeLiteral(literal) => {
            let type_ty = vue3_static_property_type(literal, "type")?;
            let required = vue3_static_boolean_property_type(literal, "required").unwrap_or(false);
            let types = vue3_reverse_infer_runtime_prop_type(type_ty, analysis)
                .unwrap_or_else(|| vec!["null".into()]);
            Some((types, required))
        }
        _ => vue3_reverse_infer_runtime_prop_type(ty, analysis).map(|types| (types, false)),
    }
}

pub(crate) fn vue3_reverse_infer_runtime_prop_type(
    ty: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vec<String>> {
    match ty {
        TSType::TSTypeReference(reference) => {
            let name = vue3_ts_type_name_key(&reference.type_name)?;
            if let Some(ctor) = name.strip_suffix("Constructor") {
                return vue3_constructor_runtime_type(ctor);
            }
            if name == "PropType" {
                let ty = vue3_type_reference_first_type_argument(reference)?;
                return Some(infer_vue3_runtime_type(ty, analysis));
            }
            if let Some(type_arguments) = reference.type_arguments.as_ref() {
                for ty in &type_arguments.params {
                    if let Some(types) = vue3_reverse_infer_runtime_prop_type(ty, analysis) {
                        return Some(types);
                    }
                }
            }
            None
        }
        TSType::TSImportType(import_type) => {
            if let Some(type_arguments) = import_type.type_arguments.as_ref() {
                for ty in &type_arguments.params {
                    if let Some(types) = vue3_reverse_infer_runtime_prop_type(ty, analysis) {
                        return Some(types);
                    }
                }
            }
            None
        }
        TSType::TSParenthesizedType(parenthesized) => {
            vue3_reverse_infer_runtime_prop_type(&parenthesized.type_annotation, analysis)
        }
        _ => None,
    }
}

pub(crate) fn vue3_constructor_runtime_type(name: &str) -> Option<Vec<String>> {
    match name {
        "String" => Some(vec!["String".into()]),
        "Number" => Some(vec!["Number".into()]),
        "Boolean" => Some(vec!["Boolean".into()]),
        "Array" => Some(vec!["Array".into()]),
        "Object" => Some(vec!["Object".into()]),
        "Function" => Some(vec!["Function".into()]),
        "Set" => Some(vec!["Set".into()]),
        "Map" => Some(vec!["Map".into()]),
        "WeakSet" => Some(vec!["WeakSet".into()]),
        "WeakMap" => Some(vec!["WeakMap".into()]),
        "Date" => Some(vec!["Date".into()]),
        "Promise" => Some(vec!["Promise".into()]),
        _ => None,
    }
}

pub(crate) fn vue3_static_property_type<'a>(
    literal: &'a TSTypeLiteral<'a>,
    key: &str,
) -> Option<&'a TSType<'a>> {
    for member in &literal.members {
        let TSSignature::TSPropertySignature(property) = member else {
            continue;
        };
        if property.computed
            || vue27_property_key_static_name(&property.key).as_deref() != Some(key)
        {
            continue;
        }
        return property
            .type_annotation
            .as_ref()
            .map(|annotation| &annotation.type_annotation);
    }
    None
}

pub(crate) fn vue3_static_boolean_property_type(
    literal: &TSTypeLiteral<'_>,
    key: &str,
) -> Option<bool> {
    let TSType::TSLiteralType(literal) = vue3_static_property_type(literal, key)? else {
        return None;
    };
    let TSLiteral::BooleanLiteral(value) = &literal.literal else {
        return None;
    };
    Some(value.value)
}

pub(crate) fn record_vue3_type_argument_deps(
    type_argument: &TSType<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    for dependency in collect_vue3_type_argument_deps_ordered(type_argument, analysis) {
        push_unique(&mut analysis.deps, &dependency);
    }
}

pub(crate) fn collect_vue3_type_argument_deps_ordered(
    type_argument: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Vec<String> {
    let mut deps = Vec::new();
    collect_vue3_type_argument_deps_ordered_into(type_argument, analysis, &mut deps);
    deps
}

pub(crate) fn collect_vue3_type_argument_deps_ordered_into(
    type_argument: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    deps: &mut Vec<String>,
) {
    match type_argument {
        TSType::TSTypeReference(reference) => {
            if let Some(name) = vue3_ts_type_name_key(&reference.type_name) {
                collect_vue3_named_type_deps_ordered(&name, analysis, deps);
            }
            if let Some(type_arguments) = reference.type_arguments.as_ref() {
                for ty in &type_arguments.params {
                    collect_vue3_type_argument_deps_ordered_into(ty, analysis, deps);
                }
            }
        }
        TSType::TSTypeLiteral(literal) => {
            for signature in &literal.members {
                collect_vue3_signature_type_deps_ordered(signature, analysis, deps);
            }
        }
        TSType::TSUnionType(union) => {
            for ty in &union.types {
                collect_vue3_type_argument_deps_ordered_into(ty, analysis, deps);
            }
        }
        TSType::TSIntersectionType(intersection) => {
            for ty in &intersection.types {
                collect_vue3_type_argument_deps_ordered_into(ty, analysis, deps);
            }
        }
        TSType::TSArrayType(array) => {
            collect_vue3_type_argument_deps_ordered_into(&array.element_type, analysis, deps);
        }
        TSType::TSTupleType(tuple) => {
            for element in &tuple.element_types {
                collect_vue3_tuple_element_type_deps_ordered(element, analysis, deps);
            }
        }
        TSType::TSParenthesizedType(parenthesized) => {
            collect_vue3_type_argument_deps_ordered_into(
                &parenthesized.type_annotation,
                analysis,
                deps,
            );
        }
        TSType::TSTypeOperatorType(operator) => {
            collect_vue3_type_argument_deps_ordered_into(&operator.type_annotation, analysis, deps);
        }
        TSType::TSIndexedAccessType(indexed) => {
            collect_vue3_type_argument_deps_ordered_into(&indexed.object_type, analysis, deps);
            collect_vue3_type_argument_deps_ordered_into(&indexed.index_type, analysis, deps);
        }
        TSType::TSFunctionType(function) => {
            collect_vue3_formal_parameters_type_deps_ordered(&function.params, analysis, deps);
            collect_vue3_type_annotation_deps_ordered(&function.return_type, analysis, deps);
        }
        TSType::TSConstructorType(constructor) => {
            collect_vue3_formal_parameters_type_deps_ordered(&constructor.params, analysis, deps);
            collect_vue3_type_annotation_deps_ordered(&constructor.return_type, analysis, deps);
        }
        TSType::TSConditionalType(conditional) => {
            collect_vue3_type_argument_deps_ordered_into(&conditional.check_type, analysis, deps);
            collect_vue3_type_argument_deps_ordered_into(&conditional.extends_type, analysis, deps);
            collect_vue3_type_argument_deps_ordered_into(&conditional.true_type, analysis, deps);
            collect_vue3_type_argument_deps_ordered_into(&conditional.false_type, analysis, deps);
        }
        TSType::TSMappedType(mapped) => {
            collect_vue3_type_argument_deps_ordered_into(&mapped.constraint, analysis, deps);
            if let Some(name_type) = mapped.name_type.as_ref() {
                collect_vue3_type_argument_deps_ordered_into(name_type, analysis, deps);
            }
            if let Some(type_annotation) = mapped.type_annotation.as_ref() {
                collect_vue3_type_argument_deps_ordered_into(type_annotation, analysis, deps);
            }
        }
        TSType::TSTemplateLiteralType(template) => {
            for ty in &template.types {
                collect_vue3_type_argument_deps_ordered_into(ty, analysis, deps);
            }
        }
        TSType::TSNamedTupleMember(member) => {
            collect_vue3_tuple_element_type_deps_ordered(&member.element_type, analysis, deps);
        }
        TSType::TSTypePredicate(predicate) => {
            if let Some(type_annotation) = predicate.type_annotation.as_ref() {
                collect_vue3_type_annotation_deps_ordered(type_annotation, analysis, deps);
            }
        }
        TSType::TSTypeQuery(query) => {
            collect_vue3_type_query_deps_ordered(query, analysis, deps);
        }
        TSType::TSImportType(import_type) => {
            if let Some(resolved) = vue3_resolve_import_type(import_type, analysis) {
                push_unique(deps, &resolved.dependency);
                collect_vue3_context_type_deps_ordered(&resolved.context, &resolved.name, deps);
            }
            if let Some(type_arguments) = import_type.type_arguments.as_ref() {
                for ty in &type_arguments.params {
                    collect_vue3_type_argument_deps_ordered_into(ty, analysis, deps);
                }
            }
        }
        TSType::TSInferType(infer) => {
            if let Some(constraint) = infer.type_parameter.constraint.as_ref() {
                collect_vue3_type_argument_deps_ordered_into(constraint, analysis, deps);
            }
            if let Some(default) = infer.type_parameter.default.as_ref() {
                collect_vue3_type_argument_deps_ordered_into(default, analysis, deps);
            }
        }
        _ => {}
    }
}

pub(crate) fn collect_vue3_named_type_deps_ordered(
    name: &str,
    analysis: &Vue3ScriptSetupAnalysis,
    deps: &mut Vec<String>,
) {
    if let Some(dependency) = analysis.type_sources.get(name) {
        push_unique(deps, dependency);
    }
    if let Some(direct_dependencies) = analysis.type_direct_deps.get(name) {
        for dependency in direct_dependencies {
            push_unique(deps, dependency);
        }
    }
    if let Some(dependencies) = analysis.type_deps.get(name) {
        for dependency in dependencies {
            push_unique(deps, dependency);
        }
    }
}

pub(crate) fn collect_vue3_context_type_deps_ordered(
    context: &Vue27TypeContext,
    name: &str,
    deps: &mut Vec<String>,
) {
    if let Some(dependency) = context.type_sources.get(name) {
        push_unique(deps, dependency);
    }
    if let Some(direct_dependencies) = context.type_direct_deps.get(name) {
        for dependency in direct_dependencies {
            push_unique(deps, dependency);
        }
    }
    if let Some(dependencies) = context.type_deps.get(name) {
        for dependency in dependencies {
            push_unique(deps, dependency);
        }
    }
}

pub(crate) fn collect_vue3_signature_type_deps_ordered(
    signature: &TSSignature<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    deps: &mut Vec<String>,
) {
    match signature {
        TSSignature::TSPropertySignature(property) => {
            if let Some(type_annotation) = property.type_annotation.as_ref() {
                collect_vue3_type_annotation_deps_ordered(type_annotation, analysis, deps);
            }
        }
        TSSignature::TSMethodSignature(method) => {
            collect_vue3_formal_parameters_type_deps_ordered(&method.params, analysis, deps);
            if let Some(return_type) = method.return_type.as_ref() {
                collect_vue3_type_annotation_deps_ordered(return_type, analysis, deps);
            }
        }
        TSSignature::TSCallSignatureDeclaration(signature) => {
            collect_vue3_formal_parameters_type_deps_ordered(&signature.params, analysis, deps);
            if let Some(return_type) = signature.return_type.as_ref() {
                collect_vue3_type_annotation_deps_ordered(return_type, analysis, deps);
            }
        }
        TSSignature::TSConstructSignatureDeclaration(signature) => {
            collect_vue3_formal_parameters_type_deps_ordered(&signature.params, analysis, deps);
            if let Some(return_type) = signature.return_type.as_ref() {
                collect_vue3_type_annotation_deps_ordered(return_type, analysis, deps);
            }
        }
        TSSignature::TSIndexSignature(signature) => {
            for parameter in &signature.parameters {
                collect_vue3_type_annotation_deps_ordered(
                    &parameter.type_annotation,
                    analysis,
                    deps,
                );
            }
            collect_vue3_type_annotation_deps_ordered(&signature.type_annotation, analysis, deps);
        }
    }
}

pub(crate) fn collect_vue3_formal_parameters_type_deps_ordered(
    parameters: &FormalParameters<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    deps: &mut Vec<String>,
) {
    for parameter in &parameters.items {
        if let Some(type_annotation) = parameter.type_annotation.as_ref() {
            collect_vue3_type_annotation_deps_ordered(type_annotation, analysis, deps);
        }
    }
    if let Some(rest) = parameters.rest.as_ref() {
        if let Some(type_annotation) = rest.type_annotation.as_ref() {
            collect_vue3_type_annotation_deps_ordered(type_annotation, analysis, deps);
        }
    }
}

pub(crate) fn collect_vue3_type_annotation_deps_ordered(
    annotation: &TSTypeAnnotation<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    deps: &mut Vec<String>,
) {
    collect_vue3_type_argument_deps_ordered_into(&annotation.type_annotation, analysis, deps);
}

pub(crate) fn collect_vue3_tuple_element_type_deps_ordered(
    element: &TSTupleElement<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    deps: &mut Vec<String>,
) {
    match element {
        TSTupleElement::TSOptionalType(optional) => {
            collect_vue3_type_argument_deps_ordered_into(&optional.type_annotation, analysis, deps);
        }
        TSTupleElement::TSRestType(rest) => {
            collect_vue3_type_argument_deps_ordered_into(&rest.type_annotation, analysis, deps);
        }
        _ => {
            if let Some(ty) = element.as_ts_type() {
                collect_vue3_type_argument_deps_ordered_into(ty, analysis, deps);
            }
        }
    }
}

pub(crate) fn collect_vue3_type_argument_deps(
    type_argument: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> BTreeSet<String> {
    let mut deps = BTreeSet::new();
    collect_vue3_type_argument_deps_into(type_argument, analysis, &mut deps);
    deps
}

pub(crate) fn collect_vue3_type_argument_deps_into(
    type_argument: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    deps: &mut BTreeSet<String>,
) {
    match type_argument {
        TSType::TSTypeReference(reference) => {
            if let Some(name) = vue3_ts_type_name_key(&reference.type_name) {
                if let Some(dependencies) = analysis.type_deps.get(&name) {
                    deps.extend(dependencies.iter().cloned());
                } else if let Some(dependency) = analysis.type_sources.get(&name) {
                    deps.insert(dependency.clone());
                }
            }
            if let Some(type_arguments) = reference.type_arguments.as_ref() {
                for ty in &type_arguments.params {
                    collect_vue3_type_argument_deps_into(ty, analysis, deps);
                }
            }
        }
        TSType::TSTypeLiteral(literal) => {
            for signature in &literal.members {
                collect_vue3_signature_type_deps(signature, analysis, deps);
            }
        }
        TSType::TSUnionType(union) => {
            for ty in &union.types {
                collect_vue3_type_argument_deps_into(ty, analysis, deps);
            }
        }
        TSType::TSIntersectionType(intersection) => {
            for ty in &intersection.types {
                collect_vue3_type_argument_deps_into(ty, analysis, deps);
            }
        }
        TSType::TSArrayType(array) => {
            collect_vue3_type_argument_deps_into(&array.element_type, analysis, deps);
        }
        TSType::TSTupleType(tuple) => {
            for element in &tuple.element_types {
                collect_vue3_tuple_element_type_deps(element, analysis, deps);
            }
        }
        TSType::TSParenthesizedType(parenthesized) => {
            collect_vue3_type_argument_deps_into(&parenthesized.type_annotation, analysis, deps);
        }
        TSType::TSTypeOperatorType(operator) => {
            collect_vue3_type_argument_deps_into(&operator.type_annotation, analysis, deps);
        }
        TSType::TSIndexedAccessType(indexed) => {
            collect_vue3_type_argument_deps_into(&indexed.object_type, analysis, deps);
            collect_vue3_type_argument_deps_into(&indexed.index_type, analysis, deps);
        }
        TSType::TSFunctionType(function) => {
            collect_vue3_formal_parameters_type_deps(&function.params, analysis, deps);
            collect_vue3_type_annotation_deps(&function.return_type, analysis, deps);
        }
        TSType::TSConstructorType(constructor) => {
            collect_vue3_formal_parameters_type_deps(&constructor.params, analysis, deps);
            collect_vue3_type_annotation_deps(&constructor.return_type, analysis, deps);
        }
        TSType::TSConditionalType(conditional) => {
            collect_vue3_type_argument_deps_into(&conditional.check_type, analysis, deps);
            collect_vue3_type_argument_deps_into(&conditional.extends_type, analysis, deps);
            collect_vue3_type_argument_deps_into(&conditional.true_type, analysis, deps);
            collect_vue3_type_argument_deps_into(&conditional.false_type, analysis, deps);
        }
        TSType::TSMappedType(mapped) => {
            collect_vue3_type_argument_deps_into(&mapped.constraint, analysis, deps);
            if let Some(name_type) = mapped.name_type.as_ref() {
                collect_vue3_type_argument_deps_into(name_type, analysis, deps);
            }
            if let Some(type_annotation) = mapped.type_annotation.as_ref() {
                collect_vue3_type_argument_deps_into(type_annotation, analysis, deps);
            }
        }
        TSType::TSTemplateLiteralType(template) => {
            for ty in &template.types {
                collect_vue3_type_argument_deps_into(ty, analysis, deps);
            }
        }
        TSType::TSNamedTupleMember(member) => {
            collect_vue3_tuple_element_type_deps(&member.element_type, analysis, deps);
        }
        TSType::TSTypePredicate(predicate) => {
            if let Some(type_annotation) = predicate.type_annotation.as_ref() {
                collect_vue3_type_annotation_deps(type_annotation, analysis, deps);
            }
        }
        TSType::TSTypeQuery(query) => {
            collect_vue3_type_query_deps(query, analysis, deps);
        }
        TSType::TSImportType(import_type) => {
            if let Some(resolved) = vue3_resolve_import_type(import_type, analysis) {
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
            if let Some(type_arguments) = import_type.type_arguments.as_ref() {
                for ty in &type_arguments.params {
                    collect_vue3_type_argument_deps_into(ty, analysis, deps);
                }
            }
        }
        TSType::TSInferType(infer) => {
            if let Some(constraint) = infer.type_parameter.constraint.as_ref() {
                collect_vue3_type_argument_deps_into(constraint, analysis, deps);
            }
            if let Some(default) = infer.type_parameter.default.as_ref() {
                collect_vue3_type_argument_deps_into(default, analysis, deps);
            }
        }
        _ => {}
    }
}

pub(crate) fn collect_vue3_named_type_deps(
    name: &str,
    analysis: &Vue3ScriptSetupAnalysis,
    deps: &mut BTreeSet<String>,
) {
    if let Some(dependencies) = analysis.type_deps.get(name) {
        deps.extend(dependencies.iter().cloned());
    } else if let Some(dependency) = analysis.type_sources.get(name) {
        deps.insert(dependency.clone());
    }
}

pub(crate) fn collect_vue3_signature_type_deps(
    signature: &TSSignature<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    deps: &mut BTreeSet<String>,
) {
    match signature {
        TSSignature::TSPropertySignature(property) => {
            if let Some(type_annotation) = property.type_annotation.as_ref() {
                collect_vue3_type_annotation_deps(type_annotation, analysis, deps);
            }
        }
        TSSignature::TSMethodSignature(method) => {
            collect_vue3_formal_parameters_type_deps(&method.params, analysis, deps);
            if let Some(return_type) = method.return_type.as_ref() {
                collect_vue3_type_annotation_deps(return_type, analysis, deps);
            }
        }
        TSSignature::TSCallSignatureDeclaration(signature) => {
            collect_vue3_formal_parameters_type_deps(&signature.params, analysis, deps);
            if let Some(return_type) = signature.return_type.as_ref() {
                collect_vue3_type_annotation_deps(return_type, analysis, deps);
            }
        }
        TSSignature::TSConstructSignatureDeclaration(signature) => {
            collect_vue3_formal_parameters_type_deps(&signature.params, analysis, deps);
            if let Some(return_type) = signature.return_type.as_ref() {
                collect_vue3_type_annotation_deps(return_type, analysis, deps);
            }
        }
        TSSignature::TSIndexSignature(signature) => {
            for parameter in &signature.parameters {
                collect_vue3_type_annotation_deps(&parameter.type_annotation, analysis, deps);
            }
            collect_vue3_type_annotation_deps(&signature.type_annotation, analysis, deps);
        }
    }
}

pub(crate) fn collect_vue3_formal_parameters_type_deps(
    parameters: &FormalParameters<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    deps: &mut BTreeSet<String>,
) {
    for parameter in &parameters.items {
        if let Some(type_annotation) = parameter.type_annotation.as_ref() {
            collect_vue3_type_annotation_deps(type_annotation, analysis, deps);
        }
    }
    if let Some(rest) = parameters.rest.as_ref() {
        if let Some(type_annotation) = rest.type_annotation.as_ref() {
            collect_vue3_type_annotation_deps(type_annotation, analysis, deps);
        }
    }
}

pub(crate) fn collect_vue3_type_annotation_deps(
    annotation: &TSTypeAnnotation<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    deps: &mut BTreeSet<String>,
) {
    collect_vue3_type_argument_deps_into(&annotation.type_annotation, analysis, deps);
}

pub(crate) fn collect_vue3_tuple_element_type_deps(
    element: &TSTupleElement<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    deps: &mut BTreeSet<String>,
) {
    match element {
        TSTupleElement::TSOptionalType(optional) => {
            collect_vue3_type_argument_deps_into(&optional.type_annotation, analysis, deps);
        }
        TSTupleElement::TSRestType(rest) => {
            collect_vue3_type_argument_deps_into(&rest.type_annotation, analysis, deps);
        }
        _ => {
            if let Some(ty) = element.as_ts_type() {
                collect_vue3_type_argument_deps_into(ty, analysis, deps);
            }
        }
    }
}

pub(crate) fn vue3_type_members_from_literal(
    source: &str,
    literal: &TSTypeLiteral<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Vue27TypeMembers {
    let (members, errors) = vue3_runtime_props_from_signatures(source, &literal.members, analysis);
    Vue27TypeMembers {
        source: source
            .get(literal.span.start as usize..literal.span.end as usize)
            .unwrap_or_default()
            .to_string(),
        members,
        errors,
    }
}

pub(crate) fn vue3_type_members_from_mapped_type(
    source: &str,
    mapped: &TSMappedType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vue27TypeMembers> {
    let constraint_keys = vue3_resolve_ordered_string_type_keys(&mapped.constraint, analysis)?;
    let mut scoped_analysis = analysis.clone();
    let key_param = mapped.key.name.to_string();
    scoped_analysis.string_literal_type_declarations.insert(
        key_param.clone(),
        constraint_keys.iter().cloned().collect::<BTreeSet<_>>(),
    );
    scoped_analysis
        .ordered_string_literal_type_declarations
        .insert(key_param, constraint_keys.clone());

    let keys = match mapped.name_type.as_ref() {
        Some(name_type) => vue3_resolve_ordered_string_type_keys(name_type, &scoped_analysis)?,
        None => constraint_keys,
    };
    let types = mapped
        .type_annotation
        .as_ref()
        .map(|ty| infer_vue3_runtime_type(ty, &scoped_analysis))
        .unwrap_or_else(|| vec!["null".into()]);
    let required = !matches!(
        mapped.optional,
        Some(TSMappedTypeModifierOperator::True | TSMappedTypeModifierOperator::Plus)
    );
    let type_annotation_source = mapped.type_annotation.as_ref().and_then(|ty| {
        source
            .get(ty.span().start as usize..ty.span().end as usize)
            .map(ToOwned::to_owned)
    });
    let member_source = source
        .get(mapped.span.start as usize..mapped.span.end as usize)
        .map(ToOwned::to_owned);

    Some(Vue27TypeMembers {
        source: source
            .get(mapped.span.start as usize..mapped.span.end as usize)
            .unwrap_or_default()
            .to_string(),
        members: keys
            .into_iter()
            .map(|key| Vue27RuntimeProp {
                key,
                types: types.clone(),
                required,
                default: None,
                is_method: false,
                type_annotation_source: type_annotation_source.clone(),
                member_source: member_source.clone(),
            })
            .collect(),
        errors: Vec::new(),
    })
}

pub(crate) fn vue3_type_members_from_record_type(
    source: &str,
    reference: &TSTypeReference<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vue27TypeMembers> {
    let keys = vue3_resolve_ordered_string_type_keys(
        vue3_type_reference_type_argument(reference, 0)?,
        analysis,
    )?;
    let value = vue3_type_reference_type_argument(reference, 1)?;
    let types = infer_vue3_runtime_type(value, analysis);
    let span = reference.span();
    let type_annotation_source = source
        .get(value.span().start as usize..value.span().end as usize)
        .map(ToOwned::to_owned);
    let member_source = source
        .get(span.start as usize..span.end as usize)
        .map(ToOwned::to_owned);

    Some(Vue27TypeMembers {
        source: source
            .get(span.start as usize..span.end as usize)
            .unwrap_or_default()
            .to_string(),
        members: keys
            .into_iter()
            .map(|key| Vue27RuntimeProp {
                key,
                types: types.clone(),
                required: true,
                default: None,
                is_method: false,
                type_annotation_source: type_annotation_source.clone(),
                member_source: member_source.clone(),
            })
            .collect(),
        errors: Vec::new(),
    })
}

pub(crate) fn vue3_type_members_from_interface_body(
    source: &str,
    body: &TSInterfaceBody<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Vue27TypeMembers {
    let (members, errors) = vue3_runtime_props_from_signatures(source, &body.body, analysis);
    Vue27TypeMembers {
        source: source
            .get(body.span.start as usize..body.span.end as usize)
            .unwrap_or_default()
            .to_string(),
        members,
        errors,
    }
}

pub(crate) fn vue3_type_members_from_interface(
    source: &str,
    declaration: &TSInterfaceDeclaration<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Vue27TypeMembers {
    let mut members = vue3_type_members_from_interface_body(source, &declaration.body, analysis);
    for heritage in &declaration.extends {
        if vue3_interface_heritage_has_vue_ignore(source, heritage) {
            continue;
        }
        let Some(base) = vue3_resolve_interface_heritage_props_type(source, heritage, analysis)
        else {
            members.errors.push(vue3_failed_extends_base_type_error());
            continue;
        };
        members.errors.extend(base.errors);
        for prop in base.members {
            if !members.members.iter().any(|member| member.key == prop.key) {
                members.members.push(prop);
            }
        }
    }
    members
}

pub(crate) fn infer_vue3_runtime_type_from_interface_declarations(
    declarations: &[&TSInterfaceDeclaration<'_>],
) -> Vec<String> {
    let mut types = Vec::new();
    for declaration in declarations {
        for signature in &declaration.body.body {
            let runtime_type = match signature {
                TSSignature::TSCallSignatureDeclaration(_)
                | TSSignature::TSConstructSignatureDeclaration(_) => "Function",
                _ => "Object",
            };
            push_unique(&mut types, runtime_type);
        }
    }
    if types.is_empty() {
        vec!["Object".into()]
    } else {
        types
    }
}

pub(crate) fn vue3_type_members_from_interface_declarations(
    source: &str,
    declarations: &[&TSInterfaceDeclaration<'_>],
    analysis: &Vue3ScriptSetupAnalysis,
) -> Vue27TypeMembers {
    let source_text = vue3_interface_declarations_source(source, declarations);
    let (members, errors) = vue3_merge_props_type_members(
        declarations
            .iter()
            .map(|declaration| vue3_type_members_from_interface(source, declaration, analysis)),
        false,
    );
    Vue27TypeMembers {
        source: source_text,
        members,
        errors,
    }
}

pub(crate) fn vue3_interface_declarations_source(
    source: &str,
    declarations: &[&TSInterfaceDeclaration<'_>],
) -> String {
    declarations
        .iter()
        .filter_map(|declaration| {
            source.get(declaration.body.span.start as usize..declaration.body.span.end as usize)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn vue3_resolve_interface_heritage_props_type(
    source: &str,
    heritage: &TSInterfaceHeritage<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vue27TypeMembers> {
    let ty_source = vue3_interface_heritage_type_source(source, heritage)?;
    let wrapped = format!("type __VuecResolved = {ty_source}");
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, &wrapped, oxc_span::SourceType::ts())
        .with_options(oxc_parser::ParseOptions {
            parse_regular_expression: true,
            ..oxc_parser::ParseOptions::default()
        })
        .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return None;
    }
    for statement in &parsed.program.body {
        if let Statement::TSTypeAliasDeclaration(declaration) = statement {
            return vue3_resolve_props_type(&wrapped, &declaration.type_annotation, analysis);
        }
    }
    None
}

pub(crate) fn vue3_emits_type_from_interface(
    source: &str,
    declaration: &TSInterfaceDeclaration<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Vue27EmitsType {
    let mut emits = vue3_emits_type_from_interface_body(source, &declaration.body, analysis);
    for heritage in &declaration.extends {
        if vue3_interface_heritage_has_vue_ignore(source, heritage) {
            continue;
        }
        let Some(base) = vue3_resolve_interface_heritage_emits_type(source, heritage, analysis)
        else {
            continue;
        };
        emits.syntax.has_call_signature |= base.syntax.has_call_signature;
        emits.syntax.has_property |= base.syntax.has_property;
        emits.call_count += base.call_count;
        for event in base.events {
            push_unique(&mut emits.events, &event);
        }
    }
    emits
}

pub(crate) fn vue3_emits_type_from_interface_declarations(
    source: &str,
    declarations: &[&TSInterfaceDeclaration<'_>],
    analysis: &Vue3ScriptSetupAnalysis,
) -> Vue27EmitsType {
    let mut merged = Vue27EmitsType {
        source: vue3_interface_declarations_source(source, declarations),
        events: Vec::new(),
        syntax: Vue3EmitsTypeSyntax::default(),
        call_count: 0,
    };
    for declaration in declarations {
        let emits = vue3_emits_type_from_interface(source, declaration, analysis);
        merged.syntax.has_call_signature |= emits.syntax.has_call_signature;
        merged.syntax.has_property |= emits.syntax.has_property;
        merged.call_count += emits.call_count;
        for event in emits.events {
            push_unique(&mut merged.events, &event);
        }
    }
    merged
}

pub(crate) fn vue3_resolve_interface_heritage_emits_type(
    source: &str,
    heritage: &TSInterfaceHeritage<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vue27EmitsType> {
    let ty_source = vue3_interface_heritage_type_source(source, heritage)?;
    let wrapped = format!("type __VuecResolved = {ty_source}");
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, &wrapped, oxc_span::SourceType::ts())
        .with_options(oxc_parser::ParseOptions {
            parse_regular_expression: true,
            ..oxc_parser::ParseOptions::default()
        })
        .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return None;
    }
    for statement in &parsed.program.body {
        if let Statement::TSTypeAliasDeclaration(declaration) = statement {
            return vue3_resolve_emits_type(&wrapped, &declaration.type_annotation, analysis);
        }
    }
    None
}

pub(crate) fn vue3_interface_heritage_type_source(
    source: &str,
    heritage: &TSInterfaceHeritage<'_>,
) -> Option<String> {
    let start = heritage.expression.span().start as usize;
    let end = heritage
        .type_arguments
        .as_ref()
        .map(|arguments| arguments.span.end as usize)
        .unwrap_or(heritage.expression.span().end as usize);
    source.get(start..end).map(str::trim).map(ToOwned::to_owned)
}

pub(crate) fn vue3_interface_heritage_has_vue_ignore(
    source: &str,
    heritage: &TSInterfaceHeritage<'_>,
) -> bool {
    if source
        .get(heritage.span.start as usize..heritage.expression.span().start as usize)
        .is_some_and(|prefix| prefix.contains("@vue-ignore"))
    {
        return true;
    }
    vue3_source_has_immediate_leading_vue_ignore_comment(
        source,
        heritage.expression.span().start as usize,
    )
}

pub(crate) fn vue3_type_annotation_has_vue_ignore(
    source: &str,
    annotation: &TSTypeAnnotation<'_>,
) -> bool {
    let type_start = annotation.type_annotation.span().start as usize;
    if source
        .get(annotation.span.start as usize..type_start)
        .is_some_and(|prefix| prefix.contains("@vue-ignore"))
    {
        return true;
    }
    vue3_source_has_immediate_leading_vue_ignore_comment(source, type_start)
}

pub(crate) fn vue3_source_has_immediate_leading_vue_ignore_comment(
    source: &str,
    offset: usize,
) -> bool {
    let mut cursor = offset.min(source.len());
    loop {
        cursor = trim_ascii_whitespace_before(source, cursor);
        if cursor == 0 {
            return false;
        }
        let before = &source[..cursor];
        if before.ends_with("*/") {
            let Some(start) = before.rfind("/*") else {
                return false;
            };
            let comment = &source[start..cursor];
            if comment.contains("@vue-ignore") {
                return true;
            }
            cursor = start;
            continue;
        }
        let line_start = before.rfind('\n').map(|index| index + 1).unwrap_or(0);
        if let Some(start) = before[line_start..].rfind("//") {
            let comment_start = line_start + start;
            let comment = &source[comment_start..cursor];
            if comment.contains("@vue-ignore") {
                return true;
            }
            cursor = comment_start;
            continue;
        }
        return false;
    }
}

pub(crate) fn trim_ascii_whitespace_before(source: &str, mut cursor: usize) -> usize {
    while cursor > 0 && source.as_bytes()[cursor - 1].is_ascii_whitespace() {
        cursor -= 1;
    }
    cursor
}

pub(crate) fn vue3_failed_extends_base_type_error() -> String {
    "Failed to resolve extends base type.\nIf this previously worked in 3.2, you can instruct the compiler to ignore this extend by adding /* @vue-ignore */ before it, for example:\n\ninterface Props extends /* @vue-ignore */ Base {}\n\nNote: both in 3.2 or with the ignore, the properties in the base type are treated as fallthrough attrs at runtime.".to_string()
}

pub(crate) fn vue3_interface_heritage_name(heritage: &TSInterfaceHeritage<'_>) -> Option<String> {
    vue3_expression_type_name_key(&heritage.expression)
}

pub(crate) fn vue3_expression_type_name_key(expression: &Expression<'_>) -> Option<String> {
    match unwrap_vue3_ts_expression(expression) {
        Expression::Identifier(identifier) => Some(identifier.name.to_string()),
        Expression::StaticMemberExpression(member) => {
            let left = vue3_expression_type_name_key(&member.object)?;
            Some(format!("{left}.{}", member.property.name))
        }
        _ => None,
    }
}

pub(crate) fn vue3_runtime_props_from_signatures(
    source: &str,
    signatures: &[TSSignature<'_>],
    analysis: &Vue3ScriptSetupAnalysis,
) -> (Vec<Vue27RuntimeProp>, Vec<String>) {
    let mut props = Vec::new();
    let mut errors = Vec::new();
    for signature in signatures {
        match signature {
            TSSignature::TSPropertySignature(property) => {
                let Some(key) = vue3_props_type_signature_key(&property.key, property.computed)
                else {
                    errors.push(vue3_unsupported_computed_key_error());
                    continue;
                };
                let types = property
                    .type_annotation
                    .as_ref()
                    .map(|annotation| {
                        if vue3_type_annotation_has_vue_ignore(source, annotation) {
                            vec!["Unknown".into()]
                        } else {
                            infer_vue3_runtime_type(&annotation.type_annotation, analysis)
                        }
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
                                .get(annotation.span.start as usize..annotation.span.end as usize)
                                .map(ToOwned::to_owned)
                        },
                    ),
                    member_source: source
                        .get(property.span.start as usize..property.span.end as usize)
                        .map(ToOwned::to_owned),
                });
            }
            TSSignature::TSMethodSignature(method) => {
                let Some(key) = vue3_props_type_signature_key(&method.key, method.computed) else {
                    errors.push(vue3_unsupported_computed_key_error());
                    continue;
                };
                props.push(Vue27RuntimeProp {
                    key,
                    types: vec!["Function".into()],
                    required: !method.optional,
                    default: None,
                    is_method: true,
                    type_annotation_source: method.return_type.as_ref().and_then(|annotation| {
                        source
                            .get(annotation.span.start as usize..annotation.span.end as usize)
                            .map(ToOwned::to_owned)
                    }),
                    member_source: source
                        .get(method.span.start as usize..method.span.end as usize)
                        .map(ToOwned::to_owned),
                });
            }
            _ => {}
        }
    }
    (props, errors)
}

pub(crate) fn vue3_props_type_signature_key(
    key: &PropertyKey<'_>,
    computed: bool,
) -> Option<String> {
    match (computed, key) {
        (false, PropertyKey::StaticIdentifier(identifier)) => Some(identifier.name.to_string()),
        (false, PropertyKey::StringLiteral(literal)) => Some(literal.value.to_string()),
        (false, PropertyKey::NumericLiteral(literal)) => Some(literal.value.to_string()),
        (true, PropertyKey::TemplateLiteral(template)) if template.expressions.is_empty() => {
            let mut key = String::new();
            for quasi in &template.quasis {
                key.push_str(&vue3_template_value(quasi));
            }
            Some(key)
        }
        _ => None,
    }
}

pub(crate) fn vue3_template_value(quasi: &oxc_ast::ast::TemplateElement<'_>) -> String {
    quasi
        .value
        .cooked
        .as_ref()
        .map(|value| value.as_str().to_string())
        .unwrap_or_else(|| quasi.value.raw.as_str().to_string())
}

pub(crate) fn vue3_keyof_runtime_type_from_interface(
    source: &str,
    declaration: &TSInterfaceDeclaration<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vec<String>> {
    let mut types = vue3_keyof_runtime_type_from_signatures(&declaration.body.body, analysis)
        .unwrap_or_default();
    for heritage in &declaration.extends {
        if vue3_interface_heritage_has_vue_ignore(source, heritage) {
            continue;
        }
        let Some(base) =
            vue3_resolve_interface_heritage_keyof_runtime_type(source, heritage, analysis)
        else {
            continue;
        };
        for runtime_type in base {
            push_unique(&mut types, &runtime_type);
        }
    }
    if types.is_empty() {
        None
    } else {
        Some(types)
    }
}

pub(crate) fn vue3_keyof_runtime_type_from_interface_declarations(
    source: &str,
    declarations: &[&TSInterfaceDeclaration<'_>],
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vec<String>> {
    let mut types = Vec::new();
    for declaration in declarations {
        let Some(runtime_types) =
            vue3_keyof_runtime_type_from_interface(source, declaration, analysis)
        else {
            continue;
        };
        for runtime_type in runtime_types {
            push_unique(&mut types, &runtime_type);
        }
    }
    if types.is_empty() {
        None
    } else {
        Some(types)
    }
}

pub(crate) fn vue3_resolve_interface_heritage_keyof_runtime_type(
    source: &str,
    heritage: &TSInterfaceHeritage<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vec<String>> {
    let ty_source = vue3_interface_heritage_type_source(source, heritage)?;
    let wrapped = format!("type __VuecResolved = {ty_source}");
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, &wrapped, oxc_span::SourceType::ts())
        .with_options(oxc_parser::ParseOptions {
            parse_regular_expression: true,
            ..oxc_parser::ParseOptions::default()
        })
        .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return None;
    }
    for statement in &parsed.program.body {
        if let Statement::TSTypeAliasDeclaration(declaration) = statement {
            return infer_vue3_keyof_runtime_type(&declaration.type_annotation, analysis);
        }
    }
    None
}

pub(crate) fn vue3_keyof_runtime_type_from_signatures(
    signatures: &[TSSignature<'_>],
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vec<String>> {
    let mut types = Vec::new();
    for signature in signatures {
        match signature {
            TSSignature::TSPropertySignature(property) => {
                let runtime_type = if matches!(property.key, PropertyKey::NumericLiteral(_)) {
                    "Number"
                } else {
                    "String"
                };
                push_unique(&mut types, runtime_type);
            }
            TSSignature::TSIndexSignature(signature) => {
                let Some(parameter) = signature.parameters.first() else {
                    return None;
                };
                let runtime_types =
                    infer_vue3_runtime_type(&parameter.type_annotation.type_annotation, analysis);
                let Some(runtime_type) = runtime_types.first() else {
                    return None;
                };
                if runtime_type == "null" || runtime_type == "Unknown" {
                    return None;
                }
                push_unique(&mut types, runtime_type);
            }
            _ => push_unique(&mut types, "String"),
        }
    }
    if types.is_empty() {
        None
    } else {
        Some(types)
    }
}

pub(crate) fn vue3_keyof_runtime_type_from_runtime_props(
    props: &[Vue27RuntimeProp],
) -> Option<Vec<String>> {
    let mut types = Vec::new();
    for prop in props {
        let runtime_type = prop
            .member_source
            .as_deref()
            .map(str::trim_start)
            .filter(|source| source.chars().next().is_some_and(|ch| ch.is_ascii_digit()))
            .map(|_| "Number")
            .unwrap_or("String");
        push_unique(&mut types, runtime_type);
    }
    if types.is_empty() {
        None
    } else {
        Some(types)
    }
}

pub(crate) fn infer_vue3_keyof_runtime_type(
    node: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vec<String>> {
    match node {
        TSType::TSStringKeyword(_) => Some(vec!["String".into()]),
        TSType::TSNumberKeyword(_) => Some(vec!["Number".into()]),
        TSType::TSBooleanKeyword(_) => Some(vec!["Boolean".into()]),
        TSType::TSObjectKeyword(_) => Some(vec!["Object".into()]),
        TSType::TSFunctionType(_) | TSType::TSConstructorType(_) => Some(vec!["Function".into()]),
        TSType::TSArrayType(_) | TSType::TSTupleType(_) => Some(vec!["Array".into()]),
        TSType::TSSymbolKeyword(_) => Some(vec!["Symbol".into()]),
        TSType::TSAnyKeyword(_) => Some(vec!["String".into(), "Number".into(), "Symbol".into()]),
        TSType::TSNullKeyword(_) => Some(vec!["null".into()]),
        TSType::TSLiteralType(literal) => match &literal.literal {
            TSLiteral::StringLiteral(_) => Some(vec!["String".into()]),
            TSLiteral::BooleanLiteral(_) => Some(vec!["Boolean".into()]),
            TSLiteral::NumericLiteral(_) | TSLiteral::BigIntLiteral(_) => {
                Some(vec!["Number".into()])
            }
            _ => None,
        },
        TSType::TSTypeLiteral(literal) => {
            vue3_keyof_runtime_type_from_signatures(&literal.members, analysis)
        }
        TSType::TSTypeReference(reference) => {
            let name = vue3_ts_type_name_key(&reference.type_name)?;
            if let Some(types) = infer_vue3_generic_keyof_runtime_type(reference, analysis) {
                return Some(types);
            }
            if let Some(types) = analysis.keyof_runtime_type_declarations.get(&name) {
                return Some(types.clone());
            }
            match name.as_str() {
                "String"
                | "Array"
                | "ArrayLike"
                | "Parameters"
                | "ConstructorParameters"
                | "ReadonlyArray" => Some(vec!["String".into(), "Number".into()]),
                "Record" | "Partial" | "Required" | "Readonly" => {
                    let ty = vue3_type_reference_type_argument(reference, 0)?;
                    infer_vue3_keyof_runtime_type(ty, analysis)
                }
                "Pick" | "Extract" => {
                    let ty = vue3_type_reference_type_argument(reference, 1)?;
                    Some(infer_vue3_runtime_type(ty, analysis))
                }
                "Function" | "Object" | "Set" | "Map" | "WeakSet" | "WeakMap" | "Date"
                | "Promise" | "Error" | "Uppercase" | "Lowercase" | "Capitalize"
                | "Uncapitalize" | "ReadonlyMap" | "ReadonlySet" => Some(vec!["String".into()]),
                _ => None,
            }
        }
        TSType::TSImportType(import_type) => {
            let resolved = vue3_resolve_import_type(import_type, analysis)?;
            resolved
                .context
                .keyof_runtime_type_declarations
                .get(&resolved.name)
                .cloned()
        }
        TSType::TSTypeQuery(query) => {
            vue3_type_query_keyof_runtime_type_declaration(query, analysis)
        }
        TSType::TSParenthesizedType(parenthesized) => {
            infer_vue3_keyof_runtime_type(&parenthesized.type_annotation, analysis)
        }
        TSType::TSIndexedAccessType(indexed) => {
            if let Some(members) = vue3_resolve_indexed_access_props_type(
                "",
                indexed,
                analysis,
                Vue3PropsTypeResolveMode::Silent,
            ) {
                if let Some(types) = vue3_keyof_runtime_type_from_runtime_props(&members.members) {
                    return Some(types);
                }
            }
            infer_vue3_indexed_access_runtime_type(indexed, analysis)
        }
        TSType::TSUnionType(union) => {
            let mut types = Vec::new();
            for ty in &union.types {
                for runtime_type in infer_vue3_keyof_runtime_type(ty, analysis)? {
                    push_unique(&mut types, &runtime_type);
                }
            }
            if types.is_empty() {
                None
            } else {
                Some(types)
            }
        }
        TSType::TSIntersectionType(intersection) => {
            let mut types = Vec::new();
            for ty in &intersection.types {
                let Some(runtime_types) = infer_vue3_keyof_runtime_type(ty, analysis) else {
                    continue;
                };
                for runtime_type in runtime_types {
                    push_unique(&mut types, &runtime_type);
                }
            }
            if types.is_empty() {
                None
            } else {
                Some(types)
            }
        }
        TSType::TSTypeOperatorType(operator) => {
            infer_vue3_keyof_runtime_type(&operator.type_annotation, analysis)
        }
        _ => None,
    }
}

pub(crate) fn infer_vue3_runtime_type_from_signatures(
    signatures: &[TSSignature<'_>],
    empty_type: &str,
) -> Vec<String> {
    let mut types = Vec::new();
    for signature in signatures {
        let runtime_type = match signature {
            TSSignature::TSCallSignatureDeclaration(_)
            | TSSignature::TSConstructSignatureDeclaration(_) => "Function",
            _ => "Object",
        };
        push_unique(&mut types, runtime_type);
    }
    if types.is_empty() {
        vec![empty_type.into()]
    } else {
        types
    }
}

pub(crate) fn infer_vue3_generic_keyof_runtime_type(
    reference: &TSTypeReference<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vec<String>> {
    let (alias_source, scoped_analysis) =
        vue3_scoped_analysis_for_generic_type_alias("", reference, analysis)?;
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        alias_source.as_str(),
        oxc_span::SourceType::ts(),
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
        if let Statement::TSTypeAliasDeclaration(declaration) = statement {
            return infer_vue3_keyof_runtime_type(&declaration.type_annotation, &scoped_analysis);
        }
    }
    None
}

pub(crate) fn infer_vue3_generic_return_runtime_type(
    reference: &TSTypeReference<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vec<String>> {
    let (alias_source, scoped_analysis) =
        vue3_scoped_analysis_for_generic_type_alias("", reference, analysis)?;
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        alias_source.as_str(),
        oxc_span::SourceType::ts(),
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
        if let Statement::TSTypeAliasDeclaration(declaration) = statement {
            return infer_vue3_return_runtime_type(
                &declaration.type_annotation,
                &scoped_analysis,
                mode,
            );
        }
    }
    None
}

pub(crate) fn vue3_mapped_identity_runtime_type_parameter(
    mapped: &TSMappedType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<String> {
    if analysis.generic_type_parameter_names.is_empty() {
        return None;
    }
    let type_annotation = mapped.type_annotation.as_ref()?;
    let TSType::TSIndexedAccessType(indexed) = type_annotation else {
        return None;
    };
    let TSType::TSTypeOperatorType(operator) = &mapped.constraint else {
        return None;
    };
    if operator.operator != TSTypeOperatorOperator::Keyof {
        return None;
    }
    let TSType::TSTypeReference(constraint_reference) = &operator.type_annotation else {
        return None;
    };
    let target_name = vue27_ts_type_name_identifier(&constraint_reference.type_name)?;
    if !analysis.generic_type_parameter_names.contains(target_name) {
        return None;
    }
    let TSType::TSTypeReference(object_reference) = &indexed.object_type else {
        return None;
    };
    if vue27_ts_type_name_identifier(&object_reference.type_name)? != target_name {
        return None;
    }
    let TSType::TSTypeReference(index_reference) = &indexed.index_type else {
        return None;
    };
    if vue27_ts_type_name_identifier(&index_reference.type_name)? != mapped.key.name.as_str() {
        return None;
    }
    Some(target_name.to_string())
}

pub(crate) fn infer_vue3_runtime_type(
    node: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Vec<String> {
    match node {
        TSType::TSStringKeyword(_) => vec!["String".into()],
        TSType::TSNumberKeyword(_) => vec!["Number".into()],
        TSType::TSBooleanKeyword(_) => vec!["Boolean".into()],
        TSType::TSObjectKeyword(_) => vec!["Object".into()],
        TSType::TSTypeLiteral(literal) => {
            infer_vue3_runtime_type_from_signatures(&literal.members, "Object")
        }
        TSType::TSFunctionType(_) | TSType::TSConstructorType(_) => vec!["Function".into()],
        TSType::TSArrayType(_) | TSType::TSTupleType(_) => vec!["Array".into()],
        TSType::TSSymbolKeyword(_) => vec!["Symbol".into()],
        TSType::TSNullKeyword(_) => vec!["null".into()],
        TSType::TSAnyKeyword(_)
        | TSType::TSBigIntKeyword(_)
        | TSType::TSNeverKeyword(_)
        | TSType::TSUndefinedKeyword(_)
        | TSType::TSUnknownKeyword(_)
        | TSType::TSVoidKeyword(_) => vec!["Unknown".into()],
        TSType::TSLiteralType(literal) => match &literal.literal {
            TSLiteral::StringLiteral(_) => vec!["String".into()],
            TSLiteral::BooleanLiteral(_) => vec!["Boolean".into()],
            TSLiteral::NumericLiteral(_) | TSLiteral::BigIntLiteral(_) => vec!["Number".into()],
            _ => vec!["Unknown".into()],
        },
        TSType::TSTypeReference(reference) => {
            if let Some(name) = vue3_ts_type_name_key(&reference.type_name) {
                if let Some(types) = infer_vue3_generic_type_alias_runtime_type(reference, analysis)
                {
                    return types;
                }
                if let Some(types) = analysis.declared_types.get(&name) {
                    return types.clone();
                }
                if let Some(types) = infer_vue3_runtime_utility_type(&name, reference, analysis) {
                    return types;
                }
                match name.as_str() {
                    "Array" | "Function" | "Object" | "Set" | "Map" | "WeakSet" | "WeakMap"
                    | "Date" | "Promise" | "Error" => return vec![name],
                    "Record" | "Partial" | "Readonly" | "Pick" | "Omit" | "Required"
                    | "InstanceType" => return vec!["Object".into()],
                    _ => {}
                }
            }
            vec!["Unknown".into()]
        }
        TSType::TSImportType(import_type) => {
            if let Some(resolved) = vue3_resolve_import_type(import_type, analysis) {
                if let Some(types) = resolved.context.declared_types.get(&resolved.name) {
                    return types.clone();
                }
            }
            vec!["Unknown".into()]
        }
        TSType::TSTypeQuery(query) => {
            if let Some(types) = vue3_type_query_runtime_type_declaration(query, analysis) {
                return types;
            }
            vec!["Unknown".into()]
        }
        TSType::TSParenthesizedType(parenthesized) => {
            infer_vue3_runtime_type(&parenthesized.type_annotation, analysis)
        }
        TSType::TSIndexedAccessType(indexed) => {
            infer_vue3_indexed_access_runtime_type(indexed, analysis)
                .unwrap_or_else(|| vec!["Unknown".into()])
        }
        TSType::TSTypeOperatorType(operator) => {
            if operator.operator == TSTypeOperatorOperator::Keyof {
                return infer_vue3_keyof_runtime_type(&operator.type_annotation, analysis)
                    .unwrap_or_else(|| vec!["Unknown".into()]);
            }
            infer_vue3_runtime_type(&operator.type_annotation, analysis)
        }
        TSType::TSUnionType(union) => {
            let mut types = Vec::new();
            for ty in &union.types {
                for runtime_type in infer_vue3_runtime_type(ty, analysis) {
                    push_unique(&mut types, &runtime_type);
                }
            }
            types
        }
        TSType::TSIntersectionType(intersection) => {
            let mut types = Vec::new();
            for ty in &intersection.types {
                for runtime_type in infer_vue3_runtime_type(ty, analysis) {
                    if runtime_type != "Unknown" {
                        push_unique(&mut types, &runtime_type);
                    }
                }
            }
            if types.is_empty() {
                vec!["Unknown".into()]
            } else {
                types
            }
        }
        TSType::TSMappedType(mapped) => {
            if let Some(type_name) = vue3_mapped_identity_runtime_type_parameter(mapped, analysis) {
                if let Some(types) = analysis.declared_types.get(&type_name) {
                    return types.clone();
                }
            }
            vec!["Unknown".into()]
        }
        TSType::TSConditionalType(conditional) => infer_vue3_conditional_runtime_type(
            conditional,
            analysis,
            Vue3ArrayElementRuntimeMode::Props,
        )
        .unwrap_or_else(|| vec!["Unknown".into()]),
        _ => vec!["Unknown".into()],
    }
}

pub(crate) fn infer_vue3_runtime_utility_type(
    name: &str,
    reference: &TSTypeReference<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vec<String>> {
    match name {
        "NonNullable" => {
            let ty = vue3_type_reference_type_argument(reference, 0)?;
            let mut types = infer_vue3_runtime_type(ty, analysis);
            types.retain(|ty| ty != "null");
            Some(types)
        }
        "Extract" => {
            let ty = vue3_type_reference_type_argument(reference, 1)?;
            Some(infer_vue3_runtime_type(ty, analysis))
        }
        "Exclude" => {
            let ty = vue3_type_reference_type_argument(reference, 0)?;
            Some(infer_vue3_runtime_type(ty, analysis))
        }
        "OmitThisParameter" => {
            let ty = vue3_type_reference_type_argument(reference, 0)?;
            Some(infer_vue3_runtime_type(ty, analysis))
        }
        "ReturnType" => {
            let ty = vue3_type_reference_type_argument(reference, 0)?;
            infer_vue3_return_runtime_type(ty, analysis, Vue3ArrayElementRuntimeMode::Props)
        }
        "Uppercase" | "Lowercase" | "Capitalize" | "Uncapitalize" => Some(vec!["String".into()]),
        "Parameters" | "ConstructorParameters" | "ReadonlyArray" => Some(vec!["Array".into()]),
        "ReadonlyMap" => Some(vec!["Map".into()]),
        "ReadonlySet" => Some(vec!["Set".into()]),
        "Ref" | "ShallowRef" | "ComputedRef" | "WritableComputedRef" => Some(vec!["Object".into()]),
        "MaybeRef" | "MaybeRefOrGetter" => {
            let mut types = vec!["Object".to_string()];
            if name == "MaybeRefOrGetter" {
                push_unique(&mut types, "Function");
            }
            if let Some(ty) = vue3_type_reference_type_argument(reference, 0) {
                for runtime_type in infer_vue3_runtime_type(ty, analysis) {
                    push_unique(&mut types, &runtime_type);
                }
            }
            Some(types)
        }
        _ => None,
    }
}

pub(crate) fn infer_vue3_define_model_runtime_type(
    node: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Vec<String> {
    match node {
        TSType::TSStringKeyword(_) => vec!["String".into()],
        TSType::TSNumberKeyword(_) => vec!["Number".into()],
        TSType::TSBooleanKeyword(_) => vec!["Boolean".into()],
        TSType::TSObjectKeyword(_) => vec!["Object".into()],
        TSType::TSTypeLiteral(literal) => {
            infer_vue3_runtime_type_from_signatures(&literal.members, "Object")
        }
        TSType::TSIntersectionType(intersection) => {
            let mut types = Vec::new();
            for ty in &intersection.types {
                for runtime_type in infer_vue3_define_model_runtime_type(ty, analysis) {
                    if runtime_type != "Unknown" {
                        push_unique(&mut types, &runtime_type);
                    }
                }
            }
            if types.is_empty() {
                vec!["Unknown".into()]
            } else {
                types
            }
        }
        TSType::TSFunctionType(_) | TSType::TSConstructorType(_) => vec!["Function".into()],
        TSType::TSArrayType(_) | TSType::TSTupleType(_) => vec!["Array".into()],
        TSType::TSSymbolKeyword(_) => vec!["Symbol".into()],
        TSType::TSNullKeyword(_) => vec!["null".into()],
        TSType::TSAnyKeyword(_)
        | TSType::TSBigIntKeyword(_)
        | TSType::TSNeverKeyword(_)
        | TSType::TSUndefinedKeyword(_)
        | TSType::TSUnknownKeyword(_)
        | TSType::TSVoidKeyword(_) => vec!["Unknown".into()],
        TSType::TSLiteralType(literal) => match &literal.literal {
            TSLiteral::StringLiteral(_) => vec!["String".into()],
            TSLiteral::BooleanLiteral(_) => vec!["Boolean".into()],
            TSLiteral::NumericLiteral(_) | TSLiteral::BigIntLiteral(_) => vec!["Number".into()],
            _ => vec!["Unknown".into()],
        },
        TSType::TSTypeReference(reference) => {
            if let Some(name) = vue3_ts_type_name_key(&reference.type_name) {
                if let Some(types) =
                    infer_vue3_generic_define_model_runtime_type(reference, analysis)
                {
                    return types;
                }
                if let Some(types) = analysis.define_model_declared_types.get(&name) {
                    return types.clone();
                }
                if let Some(types) =
                    infer_vue3_define_model_runtime_utility_type(&name, reference, analysis)
                {
                    return types;
                }
                match name.as_str() {
                    "Array" | "Function" | "Object" | "Set" | "Map" | "WeakSet" | "WeakMap"
                    | "Date" | "Promise" | "Error" => return vec![name],
                    "Record" | "Partial" | "Readonly" | "Pick" | "Omit" | "Required"
                    | "InstanceType" => return vec!["Object".into()],
                    _ => {}
                }
            }
            vec!["Unknown".into()]
        }
        TSType::TSImportType(import_type) => {
            if let Some(resolved) = vue3_resolve_import_type(import_type, analysis) {
                if let Some(types) = resolved
                    .context
                    .define_model_declared_types
                    .get(&resolved.name)
                {
                    return types.clone();
                }
            }
            vec!["Unknown".into()]
        }
        TSType::TSTypeQuery(query) => {
            if let Some(types) =
                vue3_type_query_define_model_runtime_type_declaration(query, analysis)
            {
                return types;
            }
            vec!["Unknown".into()]
        }
        TSType::TSParenthesizedType(parenthesized) => {
            infer_vue3_define_model_runtime_type(&parenthesized.type_annotation, analysis)
        }
        TSType::TSIndexedAccessType(indexed) => {
            infer_vue3_define_model_indexed_access_runtime_type(indexed, analysis)
                .unwrap_or_else(|| vec!["Unknown".into()])
        }
        TSType::TSTypeOperatorType(operator) => {
            if operator.operator == TSTypeOperatorOperator::Keyof {
                return infer_vue3_keyof_runtime_type(&operator.type_annotation, analysis)
                    .unwrap_or_else(|| vec!["Unknown".into()]);
            }
            infer_vue3_define_model_runtime_type(&operator.type_annotation, analysis)
        }
        TSType::TSUnionType(union) => {
            let mut types = Vec::new();
            for ty in &union.types {
                for runtime_type in infer_vue3_define_model_runtime_type(ty, analysis) {
                    push_unique(&mut types, &runtime_type);
                }
            }
            types
        }
        TSType::TSMappedType(mapped) => {
            if let Some(type_name) = vue3_mapped_identity_runtime_type_parameter(mapped, analysis) {
                if let Some(types) = analysis.define_model_declared_types.get(&type_name) {
                    return types.clone();
                }
            }
            vec!["Unknown".into()]
        }
        TSType::TSConditionalType(conditional) => infer_vue3_conditional_runtime_type(
            conditional,
            analysis,
            Vue3ArrayElementRuntimeMode::DefineModel,
        )
        .unwrap_or_else(|| vec!["Unknown".into()]),
        _ => vec!["Unknown".into()],
    }
}

pub(crate) fn infer_vue3_conditional_runtime_type(
    conditional: &oxc_ast::ast::TSConditionalType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
    mode: Vue3ArrayElementRuntimeMode,
) -> Option<Vec<String>> {
    let outcome = vue3_static_conditional_type_outcome(
        &conditional.check_type,
        &conditional.extends_type,
        analysis,
    )?;
    let branch = match outcome {
        Vue3StaticConditionalTypeOutcome::True => &conditional.true_type,
        Vue3StaticConditionalTypeOutcome::False => &conditional.false_type,
    };
    vue3_non_empty_runtime_types(vue3_runtime_types_for_mode(branch, analysis, mode))
}

#[derive(Clone, Copy)]
pub(crate) enum Vue3StaticConditionalTypeOutcome {
    True,
    False,
}

pub(crate) fn vue3_static_conditional_type_outcome(
    check_type: &TSType<'_>,
    extends_type: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vue3StaticConditionalTypeOutcome> {
    let check_set = vue3_static_conditional_type_set(check_type, analysis)?;
    let extends_set = vue3_static_conditional_type_set(extends_type, analysis)?;
    if check_set
        .values
        .iter()
        .all(|value| extends_set.values.contains(value))
    {
        return Some(Vue3StaticConditionalTypeOutcome::True);
    }
    if !check_set.is_distributive
        && check_set
            .values
            .iter()
            .all(|value| !extends_set.values.contains(value))
    {
        return Some(Vue3StaticConditionalTypeOutcome::False);
    }
    None
}

pub(crate) struct Vue3StaticConditionalTypeSet {
    pub(crate) values: BTreeSet<String>,
    pub(crate) is_distributive: bool,
}

pub(crate) fn vue3_static_conditional_type_set(
    ty: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vue3StaticConditionalTypeSet> {
    match ty {
        TSType::TSLiteralType(literal) => Some(Vue3StaticConditionalTypeSet {
            values: vue3_static_conditional_literal_values(&literal.literal)?,
            is_distributive: false,
        }),
        TSType::TSTypeReference(reference) => {
            let name = vue3_ts_type_name_key(&reference.type_name)?;
            if let Some(keys) = analysis.ordered_string_literal_type_declarations.get(&name) {
                return Some(Vue3StaticConditionalTypeSet {
                    values: keys
                        .iter()
                        .map(|key| vue3_static_conditional_string_value(key))
                        .collect(),
                    is_distributive: false,
                });
            }
            if let Some(keys) = analysis.string_literal_type_declarations.get(&name) {
                return Some(Vue3StaticConditionalTypeSet {
                    values: keys
                        .iter()
                        .map(|key| vue3_static_conditional_string_value(key))
                        .collect(),
                    is_distributive: false,
                });
            }
            match name.as_str() {
                "Extract" | "Exclude" | "Uppercase" | "Lowercase" | "Capitalize"
                | "Uncapitalize" => Some(Vue3StaticConditionalTypeSet {
                    values: vue3_resolve_string_type_keys(ty, analysis)?
                        .into_iter()
                        .map(|key| vue3_static_conditional_string_value(&key))
                        .collect(),
                    is_distributive: false,
                }),
                _ => None,
            }
        }
        TSType::TSUnionType(union) => {
            let mut values = BTreeSet::new();
            for ty in &union.types {
                values.extend(vue3_static_conditional_type_set(ty, analysis)?.values);
            }
            Some(Vue3StaticConditionalTypeSet {
                values,
                is_distributive: true,
            })
        }
        TSType::TSParenthesizedType(parenthesized) => {
            vue3_static_conditional_type_set(&parenthesized.type_annotation, analysis)
        }
        TSType::TSTemplateLiteralType(template) => Some(Vue3StaticConditionalTypeSet {
            values: vue3_resolve_template_literal_type_keys(template, analysis)?
                .into_iter()
                .map(|key| vue3_static_conditional_string_value(&key))
                .collect(),
            is_distributive: false,
        }),
        TSType::TSImportType(import_type) => {
            let resolved = vue3_resolve_import_type(import_type, analysis)?;
            let values = resolved
                .context
                .ordered_string_literal_type_declarations
                .get(&resolved.name)
                .cloned()
                .or_else(|| {
                    resolved
                        .context
                        .string_literal_type_declarations
                        .get(&resolved.name)
                        .map(|keys| keys.iter().cloned().collect())
                })?;
            Some(Vue3StaticConditionalTypeSet {
                values: values
                    .into_iter()
                    .map(|key| vue3_static_conditional_string_value(&key))
                    .collect(),
                is_distributive: false,
            })
        }
        _ => None,
    }
}

pub(crate) fn vue3_static_conditional_literal_values(
    literal: &TSLiteral<'_>,
) -> Option<BTreeSet<String>> {
    match literal {
        TSLiteral::StringLiteral(literal) => {
            Some([vue3_static_conditional_string_value(literal.value.as_str())].into())
        }
        TSLiteral::BooleanLiteral(literal) => Some([format!("boolean:{}", literal.value)].into()),
        TSLiteral::NumericLiteral(literal) => Some([format!("number:{}", literal.value)].into()),
        TSLiteral::BigIntLiteral(literal) => Some([format!("bigint:{}", literal.value)].into()),
        _ => None,
    }
}

pub(crate) fn vue3_static_conditional_string_value(value: &str) -> String {
    format!("string:{value}")
}

pub(crate) fn infer_vue3_define_model_runtime_utility_type(
    name: &str,
    reference: &TSTypeReference<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vec<String>> {
    match name {
        "NonNullable" => {
            let ty = vue3_type_reference_type_argument(reference, 0)?;
            let mut types = infer_vue3_define_model_runtime_type(ty, analysis);
            types.retain(|ty| ty != "null");
            Some(types)
        }
        "Extract" => {
            let ty = vue3_type_reference_type_argument(reference, 1)?;
            Some(infer_vue3_define_model_runtime_type(ty, analysis))
        }
        "Exclude" => {
            let ty = vue3_type_reference_type_argument(reference, 0)?;
            Some(infer_vue3_define_model_runtime_type(ty, analysis))
        }
        "OmitThisParameter" => {
            let ty = vue3_type_reference_type_argument(reference, 0)?;
            Some(infer_vue3_define_model_runtime_type(ty, analysis))
        }
        "ReturnType" => {
            let ty = vue3_type_reference_type_argument(reference, 0)?;
            infer_vue3_return_runtime_type(ty, analysis, Vue3ArrayElementRuntimeMode::DefineModel)
        }
        "Uppercase" | "Lowercase" | "Capitalize" | "Uncapitalize" => Some(vec!["String".into()]),
        "Parameters" | "ConstructorParameters" | "ReadonlyArray" => Some(vec!["Array".into()]),
        "ReadonlyMap" => Some(vec!["Map".into()]),
        "ReadonlySet" => Some(vec!["Set".into()]),
        "Ref" | "ShallowRef" | "ComputedRef" | "WritableComputedRef" => Some(vec!["Object".into()]),
        "MaybeRef" | "MaybeRefOrGetter" => {
            let mut types = vec!["Object".to_string()];
            if name == "MaybeRefOrGetter" {
                push_unique(&mut types, "Function");
            }
            if let Some(ty) = vue3_type_reference_type_argument(reference, 0) {
                for runtime_type in infer_vue3_define_model_runtime_type(ty, analysis) {
                    push_unique(&mut types, &runtime_type);
                }
            }
            Some(types)
        }
        _ => None,
    }
}

pub(crate) fn gen_vue3_runtime_props(
    props: &[Vue27RuntimeProp],
    is_prod: bool,
    has_static_defaults: bool,
    custom_element: bool,
) -> String {
    let mut entries = Vec::new();
    for prop in props {
        let key = vue3_runtime_prop_key(&prop.key);
        let (types, skip_check) = vue3_runtime_prop_codegen_types(&prop.types);
        let type_string = vue27_runtime_type_string(&types);
        if !is_prod {
            let skip_check = if skip_check { ", skipCheck: true" } else { "" };
            entries.push(format!(
                "{key}: {{ type: {}, required: {}{}{} }}",
                type_string,
                prop.required,
                skip_check,
                prop.default
                    .as_ref()
                    .map(|default| format!(", {default}"))
                    .unwrap_or_default()
            ));
            continue;
        }
        let keep_prod_type = custom_element
            || types.iter().any(|ty| {
                ty == "Boolean"
                    || (ty == "Function" && (!has_static_defaults || prop.default.is_some()))
            });
        match (keep_prod_type, prop.default.as_ref()) {
            (true, Some(default)) => {
                if custom_element {
                    entries.push(format!("{key}: {{ {default}, type: {type_string} }}"));
                } else {
                    entries.push(format!("{key}: {{ type: {type_string}, {default} }}"));
                }
            }
            (true, None) => {
                if custom_element {
                    entries.push(format!("{key}: {{type: {type_string}}}"));
                } else {
                    entries.push(format!("{key}: {{ type: {type_string} }}"));
                }
            }
            (false, Some(default)) => {
                entries.push(format!("{key}: {{ {default} }}"));
            }
            (false, None) => {
                entries.push(format!("{key}: {{}}"));
            }
        }
    }
    format!("{{\n    {}\n  }}", entries.join(",\n    "))
}

pub(crate) fn vue3_runtime_prop_codegen_types(types: &[String]) -> (Vec<String>, bool) {
    let mut runtime_types = types.to_vec();
    let has_unknown = runtime_types.iter().any(|ty| ty == "Unknown");
    let has_boolean = runtime_types.iter().any(|ty| ty == "Boolean");
    let has_function = runtime_types.iter().any(|ty| ty == "Function");
    if has_unknown {
        if has_boolean || has_function {
            runtime_types.retain(|ty| ty != "Unknown");
            return (runtime_types, true);
        }
        runtime_types.clear();
        runtime_types.push("null".to_string());
    }
    (runtime_types, false)
}

pub(crate) fn vue3_runtime_prop_key(key: &str) -> String {
    if vue3_runtime_prop_key_needs_quote(key) {
        format!("\"{}\"", escape_js_double(key))
    } else {
        key.to_string()
    }
}

pub(crate) fn vue3_runtime_prop_key_needs_quote(key: &str) -> bool {
    key.chars().any(|ch| {
        matches!(
            ch,
            ' ' | '!'
                | '"'
                | '#'
                | '$'
                | '%'
                | '&'
                | '\''
                | '('
                | ')'
                | '*'
                | '+'
                | ','
                | '.'
                | '/'
                | ':'
                | ';'
                | '<'
                | '='
                | '>'
                | '?'
                | '@'
                | '['
                | '\\'
                | ']'
                | '^'
                | '`'
                | '{'
                | '|'
                | '}'
                | '~'
                | '-'
        )
    })
}

pub(crate) fn vue3_props_destructured_runtime_defaults(
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<String> {
    if analysis.props_destructured_default_order.is_empty() {
        return None;
    }
    let mut entries = Vec::new();
    for key in &analysis.props_destructured_default_order {
        let Some(default) = analysis.props_destructured_defaults.get(key) else {
            continue;
        };
        let final_key = vue3_runtime_prop_key(key);
        let value = vue3_props_destructured_default_value(default, None);
        let skip = if vue3_props_destructured_default_needs_skip_factory(default, None) {
            format!(", __skip_{final_key}: true")
        } else {
            String::new()
        };
        entries.push(format!("{final_key}: {value}{skip}"));
    }
    if entries.is_empty() {
        None
    } else {
        Some(format!("{{\n  {}\n}}", entries.join(",\n  ")))
    }
}

pub(crate) fn vue3_props_destructured_default_option(
    analysis: &Vue3ScriptSetupAnalysis,
    key: &str,
    inferred_types: Option<&[String]>,
) -> Option<String> {
    let default = analysis.props_destructured_defaults.get(key)?;
    let value = vue3_props_destructured_default_value(default, inferred_types);
    let skip = if vue3_props_destructured_default_needs_skip_factory(default, inferred_types) {
        ", skipFactory: true"
    } else {
        ""
    };
    Some(format!("default: {value}{skip}"))
}

pub(crate) fn vue3_props_destructured_default_value(
    default: &Vue3PropsDestructuredDefault,
    inferred_types: Option<&[String]>,
) -> String {
    let need_skip_factory =
        vue3_props_destructured_default_needs_skip_factory(default, inferred_types);
    let is_function_prop =
        inferred_types.is_some_and(|types| types.iter().any(|ty| ty == "Function"));
    if !need_skip_factory && !default.is_literal && !is_function_prop {
        format!("() => ({})", default.value)
    } else {
        default.value.clone()
    }
}

pub(crate) fn vue3_props_destructured_default_needs_skip_factory(
    default: &Vue3PropsDestructuredDefault,
    inferred_types: Option<&[String]>,
) -> bool {
    inferred_types.is_none() && (default.is_function || default.is_identifier)
}

pub(crate) fn rewrite_vue3_define_props_destructure_rest(
    pattern: &BindingPattern<'_>,
    call: &oxc_ast::ast::CallExpression<'_>,
    rest_id: &str,
    analysis: &Vue3ScriptSetupAnalysis,
    edits: &mut SourceEdits<'_>,
) {
    let excluded = analysis
        .props_destructured_prop_order
        .iter()
        .map(|name| format!("\"{}\"", escape_js_double(name)))
        .collect::<Vec<_>>()
        .join(",");
    edits.overwrite(
        pattern.span().start as usize,
        pattern.span().end as usize,
        rest_id,
    );
    edits.overwrite(
        call.span.start as usize,
        call.span.end as usize,
        format!("_createPropsRestProxy(__props, [{excluded}])"),
    );
}

pub(crate) fn is_ascii_js_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first == '$' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch == '$' || ch.is_ascii_alphanumeric())
}

pub(crate) fn collect_vue3_define_props_destructure_bindings(
    source: &str,
    pattern: &BindingPattern<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) -> Option<String> {
    match pattern {
        BindingPattern::ObjectPattern(pattern) => {
            for property in &pattern.properties {
                let key =
                    vue3_define_props_destructure_key(&property.key, property.computed, analysis);
                collect_vue3_define_props_destructure_property(
                    source,
                    key.as_deref(),
                    &property.value,
                    analysis,
                );
            }
            if let Some(rest) = &pattern.rest {
                if let Some(rest_id) = first_pattern_binding(&rest.argument) {
                    analysis.props_destructured_rest_id = Some(rest_id.clone());
                    push_unique(&mut analysis.return_bindings, &rest_id);
                    collect_pattern_binding_types(
                        &rest.argument,
                        "setup-reactive-const",
                        &mut analysis.setup_bindings,
                    );
                    return Some(rest_id);
                }
                collect_pattern_binding_types(
                    &rest.argument,
                    "setup-reactive-const",
                    &mut analysis.setup_bindings,
                );
            }
            None
        }
        BindingPattern::ArrayPattern(pattern) => {
            for element in pattern.elements.iter().flatten() {
                collect_pattern_binding_types(
                    element,
                    "props-aliased",
                    &mut analysis.setup_bindings,
                );
            }
            if let Some(rest) = &pattern.rest {
                collect_pattern_binding_types(
                    &rest.argument,
                    "setup-reactive-const",
                    &mut analysis.setup_bindings,
                );
            }
            None
        }
        BindingPattern::AssignmentPattern(pattern) => {
            collect_vue3_define_props_destructure_bindings(source, &pattern.left, analysis)
        }
        BindingPattern::BindingIdentifier(_) => None,
    }
}

pub(crate) fn vue3_define_props_destructure_key(
    key: &PropertyKey<'_>,
    computed: bool,
    analysis: &mut Vue3ScriptSetupAnalysis,
) -> Option<String> {
    let key = match key {
        PropertyKey::StaticIdentifier(identifier) if !computed => Some(identifier.name.to_string()),
        PropertyKey::StringLiteral(literal) => Some(literal.value.to_string()),
        PropertyKey::NumericLiteral(literal) => Some(literal.value.to_string()),
        _ => None,
    };
    if key.is_none() {
        analysis
            .errors
            .push("defineProps() destructure cannot use computed key.".into());
    }
    key
}

pub(crate) fn collect_vue3_define_props_destructure_property(
    source: &str,
    key: Option<&str>,
    value: &BindingPattern<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    match value {
        BindingPattern::BindingIdentifier(identifier) => {
            register_vue3_define_props_destructure_binding(key, identifier.name.as_str(), analysis);
        }
        BindingPattern::AssignmentPattern(pattern) => {
            if vue3_expression_references_non_literal_setup_local(&pattern.right, analysis) {
                analysis.errors.push(
                    "`defineProps()` in <script setup> cannot reference locally declared variables because it will be hoisted outside of the setup() function. If your component options require initialization in the module scope, use a separate normal <script> to export the options instead."
                        .into(),
                );
            }
            if let Some(key) = key {
                if let Some(default) =
                    vue3_props_destructured_default_from_expression(source, &pattern.right)
                {
                    if !analysis
                        .props_destructured_default_order
                        .iter()
                        .any(|existing| existing == key)
                    {
                        analysis
                            .props_destructured_default_order
                            .push(key.to_string());
                    }
                    if let Some(value_type) = default.inferred_type.as_ref() {
                        analysis
                            .props_destructured_default_types
                            .insert(key.to_string(), value_type.clone());
                    }
                    analysis
                        .props_destructured_defaults
                        .insert(key.to_string(), default);
                }
            }
            if let BindingPattern::BindingIdentifier(identifier) = &pattern.left {
                register_vue3_define_props_destructure_binding(
                    key,
                    identifier.name.as_str(),
                    analysis,
                );
            } else {
                analysis
                    .errors
                    .push("defineProps() destructure does not support nested patterns.".into());
                if let Some(local) = first_pattern_binding(&pattern.left) {
                    register_vue3_define_props_destructure_binding(key, &local, analysis);
                }
            }
        }
        _ => {
            analysis
                .errors
                .push("defineProps() destructure does not support nested patterns.".into());
            if let Some(local) = first_pattern_binding(value) {
                register_vue3_define_props_destructure_binding(key, &local, analysis);
            }
        }
    }
}

pub(crate) fn vue3_props_destructured_default_from_expression(
    source: &str,
    expression: &Expression<'_>,
) -> Option<Vue3PropsDestructuredDefault> {
    let value = source
        .get(expression.span().start as usize..expression.span().end as usize)?
        .to_string();
    let unwrapped = unwrap_vue3_ts_expression(expression);
    Some(Vue3PropsDestructuredDefault {
        value,
        inferred_type: infer_vue3_define_props_destructure_default_value_type(expression)
            .map(ToOwned::to_owned),
        is_literal: vue3_props_destructured_default_is_literal(unwrapped),
        is_function: matches!(
            unwrapped,
            Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_)
        ),
        is_identifier: matches!(unwrapped, Expression::Identifier(_)),
    })
}

pub(crate) fn vue3_props_destructured_default_is_literal(expression: &Expression<'_>) -> bool {
    matches!(
        expression,
        Expression::StringLiteral(_)
            | Expression::NumericLiteral(_)
            | Expression::BooleanLiteral(_)
            | Expression::NullLiteral(_)
            | Expression::RegExpLiteral(_)
            | Expression::BigIntLiteral(_)
            | Expression::TemplateLiteral(_)
    )
}

pub(crate) fn register_vue3_define_props_destructure_binding(
    key: Option<&str>,
    local: &str,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    let public_key = key.unwrap_or(local);
    push_unique(&mut analysis.props_destructured_prop_order, public_key);
    analysis
        .props_destructured_bindings
        .insert(local.to_string(), public_key.to_string());
    if key.is_some_and(|key| key == local) {
        analysis
            .setup_bindings
            .insert(local.to_string(), "props".into());
    } else {
        analysis
            .setup_bindings
            .insert(local.to_string(), "props-aliased".into());
    }
}

pub(crate) fn check_vue3_define_props_destructure_default_types(
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    for (key, value_type) in &analysis.props_destructured_default_types {
        let Some(prop_types) = analysis.props_type_runtime_types.get(key) else {
            continue;
        };
        if prop_types.is_empty()
            || prop_types.iter().any(|ty| ty == "null")
            || prop_types.iter().any(|ty| ty == "Unknown")
            || prop_types.iter().any(|ty| ty == value_type)
        {
            continue;
        }
        analysis.errors.push(format!(
            "Default value of prop \"{key}\" does not match declared type."
        ));
    }
}

pub(crate) fn infer_vue3_define_props_destructure_default_value_type(
    expression: &Expression<'_>,
) -> Option<&'static str> {
    match unwrap_vue3_ts_expression(expression) {
        Expression::StringLiteral(_) => Some("String"),
        Expression::NumericLiteral(_) => Some("Number"),
        Expression::BooleanLiteral(_) => Some("Boolean"),
        Expression::ObjectExpression(_) => Some("Object"),
        Expression::ArrayExpression(_) => Some("Array"),
        Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_) => {
            Some("Function")
        }
        _ => None,
    }
}

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

pub(crate) fn vue3_top_level_await_entry_statement(statement: &Statement<'_>) -> bool {
    match statement {
        Statement::VariableDeclaration(declaration) => !declaration.declare,
        Statement::BlockStatement(_)
        | Statement::BreakStatement(_)
        | Statement::ContinueStatement(_)
        | Statement::DebuggerStatement(_)
        | Statement::DoWhileStatement(_)
        | Statement::EmptyStatement(_)
        | Statement::ExpressionStatement(_)
        | Statement::ForInStatement(_)
        | Statement::ForOfStatement(_)
        | Statement::ForStatement(_)
        | Statement::IfStatement(_)
        | Statement::LabeledStatement(_)
        | Statement::ReturnStatement(_)
        | Statement::SwitchStatement(_)
        | Statement::ThrowStatement(_)
        | Statement::TryStatement(_)
        | Statement::WhileStatement(_)
        | Statement::WithStatement(_) => true,
        _ => false,
    }
}

pub(crate) fn contains_js_await_word(source: &str) -> bool {
    let bytes = source.as_bytes();
    let needle = b"await";
    if bytes.len() < needle.len() {
        return false;
    }
    bytes
        .windows(needle.len())
        .enumerate()
        .any(|(index, window)| {
            window == needle
                && !bytes
                    .get(index.wrapping_sub(1))
                    .is_some_and(|byte| is_js_identifier_byte(*byte))
                && !bytes
                    .get(index + needle.len())
                    .is_some_and(|byte| is_js_identifier_byte(*byte))
        })
}

pub(crate) fn is_js_identifier_byte(byte: u8) -> bool {
    byte == b'_' || byte == b'$' || byte.is_ascii_alphanumeric()
}

pub(crate) fn vue3_props_access_exp(prop: &str) -> String {
    if is_ascii_js_identifier(prop) {
        format!("__props.{prop}")
    } else {
        format!("__props[\"{}\"]", escape_js_double(prop))
    }
}

pub(crate) fn vue3_is_define_props_call(expression: &Expression<'_>) -> bool {
    matches!(unwrap_vue3_ts_expression(expression), Expression::CallExpression(call) if is_call_named(call, "defineProps"))
}

pub(crate) fn vue3_call_argument_expression<'a>(
    argument: &'a Argument<'a>,
) -> Option<&'a Expression<'a>> {
    match argument {
        Argument::SpreadElement(_) => None,
        _ => Some(argument.to_expression()),
    }
}

pub(crate) fn vue3_expression_references_non_literal_setup_local(
    expression: &Expression<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> bool {
    let non_literal_bindings = analysis
        .local_setup_binding_types
        .iter()
        .filter_map(|(name, binding_type)| {
            (binding_type != "literal-const").then_some(name.clone())
        })
        .collect::<BTreeSet<_>>();
    vue27_expression_references_setup_local(expression, &non_literal_bindings)
}

pub(crate) fn check_vue3_invalid_non_literal_scope_reference(
    expression: &Expression<'_>,
    macro_name: &str,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    if vue3_expression_references_non_literal_setup_local(expression, analysis) {
        analysis
            .errors
            .push(vue3_invalid_scope_reference_error(macro_name));
    }
}

pub(crate) fn vue3_invalid_scope_reference_error(macro_name: &str) -> String {
    format!(
        "`{macro_name}()` in <script setup> cannot reference locally declared variables because it will be hoisted outside of the setup() function. If your component options require initialization in the module scope, use a separate normal <script> to export the options instead."
    )
}

pub(crate) fn collect_vue3_define_emits_call(
    source: &str,
    call: &oxc_ast::ast::CallExpression<'_>,
    binding: Option<&str>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    if analysis.has_define_emits {
        analysis.errors.push("duplicate defineEmits() call".into());
    }
    analysis.has_define_emits = true;
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
        collect_vue3_define_emits_type(source, type_argument, analysis);
        return;
    }
    let Some(argument) = call.arguments.first() else {
        return;
    };
    let expression = argument.to_expression();
    check_vue3_invalid_non_literal_scope_reference(expression, "defineEmits", analysis);
    analysis.emits_runtime = source
        .get(expression.span().start as usize..expression.span().end as usize)
        .map(ToOwned::to_owned);
}

pub(crate) fn collect_vue3_define_emits_type(
    source: &str,
    type_argument: &TSType<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    record_vue3_type_argument_deps(type_argument, analysis);
    let Some(emits_type) = vue3_resolve_emits_type(source, type_argument, analysis) else {
        return;
    };
    if emits_type.syntax.has_call_signature && emits_type.syntax.has_property {
        analysis
            .errors
            .push("defineEmits() type cannot mixed call signature and property syntax.".into());
    }
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
}

pub(crate) fn vue3_resolve_emits_type<'a>(
    source: &str,
    type_argument: &'a TSType<'a>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vue27EmitsType> {
    match type_argument {
        TSType::TSFunctionType(function) => {
            Some(vue3_emits_type_from_function(source, function, analysis))
        }
        TSType::TSTypeLiteral(literal) => {
            Some(vue3_emits_type_from_literal(source, literal, analysis))
        }
        TSType::TSTypeReference(reference) => {
            let name = vue3_ts_type_name_key(&reference.type_name)?;
            analysis.emits_type_declarations.get(&name).cloned()
        }
        TSType::TSImportType(import_type) => {
            let resolved = vue3_resolve_import_type(import_type, analysis)?;
            resolved
                .context
                .emits_type_declarations
                .get(&resolved.name)
                .cloned()
        }
        TSType::TSIntersectionType(intersection) => {
            let mut events = Vec::new();
            let mut syntax = Vue3EmitsTypeSyntax::default();
            let mut call_count = 0usize;
            for ty in &intersection.types {
                let Some(resolved) = vue3_resolve_emits_type(source, ty, analysis) else {
                    continue;
                };
                syntax.has_call_signature |= resolved.syntax.has_call_signature;
                syntax.has_property |= resolved.syntax.has_property;
                call_count += resolved.call_count;
                for event in resolved.events {
                    push_unique(&mut events, &event);
                }
            }
            if events.is_empty() && call_count == 0 {
                None
            } else {
                Some(Vue27EmitsType {
                    source: source
                        .get(intersection.span.start as usize..intersection.span.end as usize)
                        .unwrap_or_default()
                        .to_string(),
                    events,
                    syntax,
                    call_count,
                })
            }
        }
        TSType::TSUnionType(union) => {
            let mut events = Vec::new();
            let mut syntax = Vue3EmitsTypeSyntax::default();
            let mut call_count = 0usize;
            for ty in &union.types {
                let Some(resolved) = vue3_resolve_emits_type(source, ty, analysis) else {
                    continue;
                };
                syntax.has_call_signature |= resolved.syntax.has_call_signature;
                syntax.has_property |= resolved.syntax.has_property;
                call_count += resolved.call_count;
                for event in resolved.events {
                    push_unique(&mut events, &event);
                }
            }
            if events.is_empty() && call_count == 0 {
                None
            } else {
                Some(Vue27EmitsType {
                    source: source
                        .get(union.span.start as usize..union.span.end as usize)
                        .unwrap_or_default()
                        .to_string(),
                    events,
                    syntax,
                    call_count,
                })
            }
        }
        TSType::TSParenthesizedType(parenthesized) => {
            vue3_resolve_emits_type(source, &parenthesized.type_annotation, analysis)
        }
        _ => None,
    }
}

pub(crate) fn vue3_runtime_prop_keys(expression: &Expression<'_>) -> Vec<String> {
    match expression {
        Expression::ObjectExpression(object) => object_expression_keys(object),
        Expression::ArrayExpression(array) => array
            .elements
            .iter()
            .filter_map(|element| match element.as_expression() {
                Some(Expression::StringLiteral(literal)) => Some(literal.value.to_string()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

pub(crate) fn vue3_setup_binding_type(
    kind: VariableDeclarationKind,
    init: Option<&Expression<'_>>,
    is_all_static: bool,
    literal_const_enabled: bool,
    vue_import_aliases: &BTreeMap<String, String>,
) -> &'static str {
    if kind != VariableDeclarationKind::Const {
        return "setup-let";
    }
    if literal_const_enabled && (is_all_static || init.is_some_and(vue3_is_static_node)) {
        return "literal-const";
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

pub(crate) fn vue3_ts_enum_binding_type(declaration: &TSEnumDeclaration<'_>) -> &'static str {
    if vue3_ts_enum_is_static_literal(declaration) {
        "literal-const"
    } else {
        "setup-const"
    }
}

pub(crate) fn vue3_ts_enum_is_static_literal(declaration: &TSEnumDeclaration<'_>) -> bool {
    if declaration
        .body
        .members
        .iter()
        .all(|member| member.initializer.as_ref().is_none_or(vue3_is_static_node))
    {
        true
    } else {
        false
    }
}

pub(crate) fn vue3_is_static_node(expression: &Expression<'_>) -> bool {
    match expression {
        Expression::UnaryExpression(expression) => vue3_is_static_node(&expression.argument),
        Expression::LogicalExpression(expression) => {
            vue3_is_static_node(&expression.left) && vue3_is_static_node(&expression.right)
        }
        Expression::BinaryExpression(expression) => {
            vue3_is_static_node(&expression.left) && vue3_is_static_node(&expression.right)
        }
        Expression::ConditionalExpression(expression) => {
            vue3_is_static_node(&expression.test)
                && vue3_is_static_node(&expression.consequent)
                && vue3_is_static_node(&expression.alternate)
        }
        Expression::SequenceExpression(expression) => {
            expression.expressions.iter().all(vue3_is_static_node)
        }
        Expression::TemplateLiteral(expression) => {
            expression.expressions.iter().all(vue3_is_static_node)
        }
        Expression::ParenthesizedExpression(expression) => {
            vue3_is_static_node(&expression.expression)
        }
        Expression::TSAsExpression(expression) => vue3_is_static_node(&expression.expression),
        Expression::TSSatisfiesExpression(expression) => {
            vue3_is_static_node(&expression.expression)
        }
        Expression::TSTypeAssertion(expression) => vue3_is_static_node(&expression.expression),
        Expression::TSNonNullExpression(expression) => vue3_is_static_node(&expression.expression),
        Expression::TSInstantiationExpression(expression) => {
            vue3_is_static_node(&expression.expression)
        }
        Expression::StringLiteral(_)
        | Expression::NumericLiteral(_)
        | Expression::BooleanLiteral(_)
        | Expression::NullLiteral(_)
        | Expression::BigIntLiteral(_) => true,
        _ => false,
    }
}

pub(crate) fn analyze_vue3_normal_script_for_setup(
    descriptor: &SfcDescriptor,
) -> Vue3NormalScriptAnalysis {
    let Some(script) = descriptor.script.as_ref() else {
        return Vue3NormalScriptAnalysis::default();
    };
    let moved_after_setup = descriptor
        .script_setup
        .as_ref()
        .is_some_and(|script_setup| script.content_start > script_setup.content_start);
    if !script_lang_is_js_like(&script.attrs) {
        return Vue3NormalScriptAnalysis {
            module_content: script.content.clone(),
            moved_after_setup,
            ..Vue3NormalScriptAnalysis::default()
        };
    }
    let source = script.content.as_str();
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        source,
        script_source_type_from_attrs(&script.attrs),
    )
    .with_options(oxc_parser::ParseOptions {
        parse_regular_expression: true,
        ..oxc_parser::ParseOptions::default()
    })
    .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return Vue3NormalScriptAnalysis {
            module_content: source.to_string(),
            moved_after_setup,
            errors: parsed.errors.iter().map(ToString::to_string).collect(),
            ..Vue3NormalScriptAnalysis::default()
        };
    }

    let mut edits = SourceEdits::new(source);
    let mut analysis = Vue3NormalScriptAnalysis {
        moved_after_setup,
        ..Vue3NormalScriptAnalysis::default()
    };
    for statement in &parsed.program.body {
        match statement {
            Statement::ExportDefaultDeclaration(declaration) => {
                analysis.has_default_export = true;
                analysis.has_default_export_name = default_export_has_name(declaration);
                rewrite_vue3_export_default("__default__", declaration, &mut edits);
            }
            Statement::ExportNamedDeclaration(declaration) => {
                if rewrite_vue3_compile_script_named_default_export(
                    source,
                    "__default__",
                    declaration,
                    &mut edits,
                ) {
                    analysis.has_default_export = true;
                }
            }
            _ => {}
        }
    }
    analysis.module_content = trim_trailing_blank_lines(&edits.apply()).to_string();
    analysis
}

pub(crate) fn rewrite_vue3_compile_script_named_default_export(
    input: &str,
    variable: &str,
    declaration: &ExportNamedDeclaration<'_>,
    edits: &mut SourceEdits,
) -> bool {
    let Some(specifier) = declaration
        .specifiers
        .iter()
        .find(|specifier| module_export_name(specifier.exported()) == Some("default"))
    else {
        return false;
    };

    if export_named_declaration_only_exports_default(declaration) {
        edits.remove(
            declaration.span.start as usize,
            declaration.span.end as usize,
        );
    } else {
        let end = specifier_end(
            input,
            specifier.span.end as usize,
            declaration.span.end as usize,
        );
        edits.remove(specifier.span.start as usize, end);
    }

    let local_name = module_export_name(specifier.local()).unwrap_or("default");
    if let Some(source) = declaration.source.as_ref() {
        let source_value = source.value.to_string();
        let local_source =
            &input[specifier.local().span().start as usize..specifier.local().span().end as usize];
        edits.prepend(format!(
            "import {{ {local_source} as {variable} }} from '{}'\n",
            source_value
        ));
    } else {
        edits.append(format!("\nconst {variable} = {local_name}\n"));
    }
    true
}

pub(crate) fn vue3_script_setup_export(
    setup_analysis: &Vue3ScriptSetupAnalysis,
    bindings: &[Vue3ScriptSetupReturnBinding],
    script_bindings: &BTreeMap<String, String>,
    filename: &str,
    normal_script: &Vue3NormalScriptAnalysis,
    is_ts: bool,
    is_prod: bool,
    inline_render: Option<&Vue3InlineTemplateRender>,
    css_vars_code: Option<&str>,
    emit_script_setup_marker: bool,
    gen_default_as: Option<&str>,
) -> String {
    let export_prefix = vue3_script_setup_default_export_prefix(gen_default_as);
    let runtime_options = vue3_script_setup_runtime_options(
        filename,
        normal_script,
        setup_analysis,
        is_prod,
        inline_render,
    );
    let setup_params = vue3_script_setup_params(setup_analysis, inline_render.is_some());
    let setup_body = vue3_script_setup_body(
        setup_analysis,
        bindings,
        script_bindings,
        inline_render,
        css_vars_code,
        emit_script_setup_marker,
        is_ts,
    );
    if is_ts {
        let options_spread = setup_analysis
            .options_runtime
            .as_ref()
            .map(|options| format!("\n  ...{options},"))
            .unwrap_or_default();
        let spread = if normal_script.has_default_export {
            "\n  ...__default__,"
        } else {
            ""
        };
        return format!(
            "{export_prefix} /*@__PURE__*/_defineComponent({{{spread}{options_spread}{runtime_options}\n  {async_prefix}setup({setup_params}) {{\n{setup_body}\n}}\n\n}})",
            async_prefix = vue3_script_setup_async_prefix(setup_analysis),
        );
    }
    if normal_script.has_default_export || setup_analysis.options_runtime.is_some() {
        let default_arg = if normal_script.has_default_export {
            "__default__, "
        } else {
            ""
        };
        let options_arg = setup_analysis
            .options_runtime
            .as_ref()
            .map(|options| format!("{options}, "))
            .unwrap_or_default();
        format!(
            "{export_prefix} /*@__PURE__*/Object.assign({default_arg}{options_arg}{{{runtime_options}\n  {async_prefix}setup({setup_params}) {{\n{setup_body}\n}}\n\n}})",
            async_prefix = vue3_script_setup_async_prefix(setup_analysis),
        )
    } else {
        format!(
            "{export_prefix} {{{runtime_options}\n  {async_prefix}setup({setup_params}) {{\n{setup_body}\n}}\n\n}}",
            async_prefix = vue3_script_setup_async_prefix(setup_analysis),
        )
    }
}

pub(crate) fn vue3_script_setup_default_export_prefix(gen_default_as: Option<&str>) -> String {
    gen_default_as
        .map(|name| format!("const {name} ="))
        .unwrap_or_else(|| "export default".to_string())
}

pub(crate) fn vue3_script_setup_async_prefix(
    setup_analysis: &Vue3ScriptSetupAnalysis,
) -> &'static str {
    if setup_analysis.has_top_level_await {
        "async "
    } else {
        ""
    }
}

pub(crate) fn vue3_script_setup_runtime_options(
    filename: &str,
    normal_script: &Vue3NormalScriptAnalysis,
    setup_analysis: &Vue3ScriptSetupAnalysis,
    is_prod: bool,
    inline_render: Option<&Vue3InlineTemplateRender>,
) -> String {
    let mut runtime_options = String::new();
    if !normal_script.has_default_export_name && should_infer_vue3_script_name(filename) {
        if let Some(name) = script_component_name(filename) {
            runtime_options.push_str(&format!("\n  __name: '{}',", escape_js_single(&name)));
        }
    }
    if inline_render.is_some_and(|render| render.ssr) {
        runtime_options.push_str("\n  __ssrInlineRender: true,");
    }
    if let Some(props) = vue3_script_setup_props_runtime(setup_analysis, is_prod) {
        runtime_options.push_str(&format!("\n  props: {},", props.trim()));
    }
    if let Some(emits) = vue3_script_setup_emits_runtime(setup_analysis) {
        runtime_options.push_str(&format!("\n  emits: {},", emits.trim()));
    }
    runtime_options
}

pub(crate) fn should_infer_vue3_script_name(filename: &str) -> bool {
    !filename.is_empty() && filename.replace('\\', "/") != "anonymous.vue"
}

pub(crate) fn vue3_script_setup_needs_merge_models(
    setup_analysis: &Vue3ScriptSetupAnalysis,
) -> bool {
    !setup_analysis.models.is_empty()
        && (setup_analysis.props_runtime.is_some() || setup_analysis.emits_runtime.is_some())
}

pub(crate) fn vue3_script_setup_props_runtime(
    setup_analysis: &Vue3ScriptSetupAnalysis,
    is_prod: bool,
) -> Option<String> {
    let props = setup_analysis.props_runtime.as_ref();
    let model_props = vue3_script_setup_model_props_runtime(&setup_analysis.models, is_prod);
    match (props, model_props) {
        (Some(props), Some(model_props)) => Some(format!(
            "/*@__PURE__*/_mergeModels({}, {})",
            props.trim(),
            model_props
        )),
        (Some(props), None) => Some(props.clone()),
        (None, Some(model_props)) => Some(model_props),
        (None, None) => None,
    }
}

pub(crate) fn vue3_script_setup_model_props_runtime(
    models: &[Vue3ModelDecl],
    is_prod: bool,
) -> Option<String> {
    if models.is_empty() {
        return None;
    }
    let mut entries = Vec::new();
    for model in models {
        entries.push(format!(
            "    \"{}\": {},",
            escape_js_double(&model.name),
            vue3_define_model_runtime_decl(model, is_prod)
        ));
        entries.push(format!(
            "    \"{}\": {{}},",
            escape_js_double(&vue3_model_modifiers_prop_name(&model.name))
        ));
    }
    Some(format!("{{\n{}\n  }}", entries.join("\n")))
}

pub(crate) fn vue3_define_model_runtime_decl(model: &Vue3ModelDecl, is_prod: bool) -> String {
    let mut runtime_types = model.runtime_types.clone();
    let has_runtime_options = model.prop_runtime.is_some();
    let mut skip_check = false;
    let mut codegen_options = String::new();

    if let Some(types) = runtime_types.as_mut() {
        let has_boolean = types.iter().any(|ty| ty == "Boolean");
        let has_function = types.iter().any(|ty| ty == "Function");
        let has_unknown = types.iter().any(|ty| ty == "Unknown");

        if has_unknown {
            if has_boolean || has_function {
                types.retain(|ty| ty != "Unknown");
                skip_check = true;
            } else {
                types.clear();
                types.push("null".to_string());
            }
        }

        if !is_prod {
            codegen_options = format!("type: {}", vue27_runtime_type_string(types));
            if skip_check {
                codegen_options.push_str(", skipCheck: true");
            }
        } else if has_boolean || (has_runtime_options && has_function) {
            codegen_options = format!("type: {}", vue27_runtime_type_string(types));
        }
    }

    match (codegen_options.is_empty(), model.prop_runtime.as_deref()) {
        (false, Some(runtime_options)) => {
            format!("{{ {codegen_options}, ...{runtime_options} }}")
        }
        (false, None) => format!("{{ {codegen_options} }}"),
        (true, Some(runtime_options)) => runtime_options.to_string(),
        (true, None) => "{}".to_string(),
    }
}

pub(crate) fn vue3_script_setup_emits_runtime(
    setup_analysis: &Vue3ScriptSetupAnalysis,
) -> Option<String> {
    let emits = setup_analysis.emits_runtime.as_ref();
    let model_emits = vue3_script_setup_model_emits_runtime(&setup_analysis.models);
    match (emits, model_emits) {
        (Some(emits), Some(model_emits)) => Some(format!(
            "/*@__PURE__*/_mergeModels({}, {})",
            emits.trim(),
            model_emits
        )),
        (Some(emits), None) => Some(emits.clone()),
        (None, Some(model_emits)) => Some(model_emits),
        (None, None) => None,
    }
}

pub(crate) fn vue3_script_setup_model_emits_runtime(models: &[Vue3ModelDecl]) -> Option<String> {
    if models.is_empty() {
        return None;
    }
    Some(format!(
        "[{}]",
        models
            .iter()
            .map(|model| format!("\"update:{}\"", escape_js_double(&model.name)))
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

pub(crate) fn vue3_model_modifiers_prop_name(name: &str) -> String {
    if name == "modelValue" {
        "modelModifiers".to_string()
    } else {
        format!("{name}Modifiers")
    }
}

pub(crate) fn vue3_script_setup_params(
    setup_analysis: &Vue3ScriptSetupAnalysis,
    inline_template: bool,
) -> String {
    let props = if setup_analysis.props_type_runtime {
        "__props: any"
    } else {
        "__props"
    };
    let mut context_parts = Vec::new();
    if setup_analysis.has_define_expose || !inline_template {
        context_parts.push("expose: __expose");
    }
    if setup_analysis.emit_binding.is_some() {
        context_parts.push("emit: __emit");
    }
    if context_parts.is_empty() {
        props.to_string()
    } else {
        format!("{props}, {{ {} }}", context_parts.join(", "))
    }
}

pub(crate) fn vue3_script_setup_body(
    setup_analysis: &Vue3ScriptSetupAnalysis,
    bindings: &[Vue3ScriptSetupReturnBinding],
    script_bindings: &BTreeMap<String, String>,
    inline_render: Option<&Vue3InlineTemplateRender>,
    css_vars_code: Option<&str>,
    emit_script_setup_marker: bool,
    is_ts: bool,
) -> String {
    let mut returned_binding_types = script_bindings.clone();
    returned_binding_types.extend(setup_analysis.setup_bindings.clone());
    let returned = script_setup_returned_bindings(bindings, &returned_binding_types);
    let mut body = String::new();
    if inline_render.is_none() && !setup_analysis.has_define_expose {
        body.push_str("  __expose();\n");
    }
    if setup_analysis.has_top_level_await {
        if !body.is_empty() && !body.ends_with("\n\n") {
            body.push('\n');
        }
        if is_ts {
            body.push_str("let __temp: any, __restore: any\n");
        } else {
            body.push_str("let __temp, __restore\n");
        }
    }
    let has_css_vars_code = css_vars_code.is_some();
    if let Some(css_vars_code) = css_vars_code {
        body.push('\n');
        body.push_str(css_vars_code);
        body.push_str("\n\n");
    }
    if setup_analysis.setup_content.is_empty() {
        if !has_css_vars_code {
            body.push('\n');
        }
    } else {
        let setup_content = if has_css_vars_code {
            setup_analysis
                .setup_content
                .strip_prefix('\n')
                .unwrap_or(&setup_analysis.setup_content)
        } else {
            &setup_analysis.setup_content
        };
        body.push_str(setup_content);
        if !setup_content.ends_with('\n') {
            body.push('\n');
        }
    }
    if let Some(render) = inline_render {
        body.push_str("return ");
        body.push_str(&render.code);
        return body;
    }
    body.push_str(vue3_return_separator(setup_analysis, &body));
    if emit_script_setup_marker {
        body.push_str(&format!(
            "const __returned__ = {returned}\nObject.defineProperty(__returned__, '__isScriptSetup', {{ enumerable: false, value: true }})\nreturn __returned__"
        ));
    } else {
        body.push_str(&format!("return {returned}"));
    }
    body
}

pub(crate) fn vue3_return_separator(
    setup_analysis: &Vue3ScriptSetupAnalysis,
    setup_body: &str,
) -> &'static str {
    if !setup_analysis.setup_content.starts_with('\n') {
        return "";
    }
    if setup_body.is_empty() {
        return "\n";
    }
    if setup_body.chars().all(|ch| matches!(ch, '\n' | '\r')) {
        return "";
    }
    if !setup_body.ends_with('\n') {
        return "\n";
    }
    let without_trailing_newlines = setup_body.trim_end_matches(['\n', '\r']);
    let Some(last_line) = without_trailing_newlines.rsplit('\n').next() else {
        return "";
    };
    if last_line.trim().is_empty() {
        ""
    } else {
        "\n"
    }
}

pub(crate) fn script_setup_returned_bindings(
    bindings: &[Vue3ScriptSetupReturnBinding],
    setup_bindings: &BTreeMap<String, String>,
) -> String {
    let returned = bindings
        .iter()
        .filter(|binding| {
            !binding.name.starts_with("import:") && !binding.name.starts_with("export:")
        })
        .map(|binding| vue3_script_setup_return_binding_source(binding, setup_bindings))
        .collect::<Vec<_>>()
        .join(", ");
    if returned.is_empty() {
        "{  }".to_string()
    } else {
        format!("{{ {returned} }}")
    }
}

pub(crate) fn vue3_script_setup_return_binding_source(
    binding: &Vue3ScriptSetupReturnBinding,
    setup_bindings: &BTreeMap<String, String>,
) -> String {
    match &binding.kind {
        Vue3ScriptSetupReturnBindingKind::Import { source }
            if source != "vue" && !source.ends_with(".vue") =>
        {
            format!("get {0}() {{ return {0} }}", binding.name)
        }
        _ if setup_bindings
            .get(&binding.name)
            .is_some_and(|binding_type| binding_type == "setup-let") =>
        {
            let set_arg = if binding.name == "v" { "_v" } else { "v" };
            format!(
                "get {0}() {{ return {0} }}, set {0}({1}) {{ {0} = {1} }}",
                binding.name, set_arg
            )
        }
        _ => binding.name.clone(),
    }
}

pub(crate) fn append_vue3_module_chunk(output: &mut String, chunk: &str) {
    let chunk = trim_trailing_blank_lines(chunk);
    if chunk.is_empty() {
        return;
    }
    if !output.is_empty() && !output.ends_with('\n') && !output_has_pending_blank_line(output) {
        output.push('\n');
    }
    output.push_str(chunk);
}

pub(crate) fn append_vue3_export_chunk(output: &mut String, chunk: &str) {
    let chunk = trim_trailing_blank_lines(chunk);
    if chunk.is_empty() {
        return;
    }
    if !output.is_empty() && !output.ends_with('\n') {
        output.push('\n');
    }
    output.push_str(chunk);
}

pub(crate) fn ensure_vue3_moved_normal_script_gap_before_export(output: &mut String) {
    if output.is_empty() {
        return;
    }
    if output.ends_with('\n') {
        output.push('\n');
    } else {
        output.push_str("\n\n");
    }
}

pub(crate) fn vue3_removed_setup_import_leading_padding(
    source: &str,
    statement: &Statement<'_>,
) -> Option<String> {
    let start = statement.span().start as usize;
    let leading = source.get(..start)?;
    if leading.is_empty() || !leading.trim().is_empty() {
        return None;
    }
    Some(leading.to_string())
}

pub(crate) fn vue3_trailing_blank_line_padding(value: &str) -> Option<&str> {
    let line_start = value.rfind('\n')?;
    let trailing = &value[line_start..];
    trailing.trim().is_empty().then_some(trailing)
}

pub(crate) fn vue3_script_setup_needs_blank_before_export(
    setup_analysis: &Vue3ScriptSetupAnalysis,
) -> bool {
    setup_analysis.setup_content.starts_with('\n')
        || (!setup_analysis.setup_content.is_empty()
            && setup_analysis.setup_content.trim().is_empty()
            && setup_analysis.setup_content.contains('\n'))
}

pub(crate) fn ensure_vue3_blank_line_before_export(output: &mut String) {
    if output.ends_with("\n\n") || output_has_pending_blank_line(output) {
        return;
    }
    if output.ends_with('\n') {
        output.push('\n');
    } else {
        output.push_str("\n\n");
    }
}

pub(crate) fn script_component_name(filename: &str) -> Option<String> {
    std::path::Path::new(filename)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(ToOwned::to_owned)
}

pub(crate) fn quoted_import_path(source: &str) -> Option<&str> {
    let start = source.find(['"', '\''])?;
    let quote = source[start..].chars().next()?;
    let rest = &source[start + quote.len_utf8()..];
    let end = rest.find(quote)?;
    Some(&rest[..end])
}

pub(crate) fn side_effect_tag_errors(source: &str) -> Vec<SfcTemplateError> {
    side_effect_tag_ranges(source)
        .into_iter()
        .filter_map(|(start, end, _)| {
            let start_pos = position_at(source, start)?;
            let end_pos = position_at(source, end)?;
            Some(SfcTemplateError {
                code: 64,
                message: "Tags with side effect (<script> and <style>) are ignored in client component templates.".into(),
                loc: SfcSourceLocation {
                    start: start_pos,
                    end: end_pos,
                    source: source[start..end].to_string(),
                },
            })
        })
        .collect()
}

pub(crate) fn side_effect_tag_ranges(source: &str) -> Vec<(usize, usize, &'static str)> {
    let mut ranges = Vec::new();
    for tag in ["script", "style"] {
        let mut cursor = 0usize;
        while let Some(start_offset) = source[cursor..].find(&format!("<{tag}")) {
            let start = cursor + start_offset;
            let Some(after_open_offset) = source[start..].find('>') else {
                break;
            };
            let after_open = start + after_open_offset + 1;
            let close_tag = format!("</{tag}>");
            let Some(close_offset) = source[after_open..].find(&close_tag) else {
                break;
            };
            let end = after_open + close_offset + close_tag.len();
            ranges.push((start, end, tag));
            cursor = end;
        }
    }
    ranges.sort_by_key(|(start, _, _)| *start);
    ranges
}
