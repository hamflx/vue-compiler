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
    let inline_module_sources = descriptor
        .script
        .iter()
        .chain(&descriptor.script_setup)
        .map(|script| Vue3InlineModuleSource {
            filename: &descriptor.filename,
            source: script.content.as_str(),
            source_type: script_source_type_from_attrs(&script.attrs),
        })
        .collect::<Vec<_>>();
    let mut context = vue3_global_type_context_with_module_sources(
        &descriptor.filename,
        global_type_files,
        &inline_module_sources,
        type_resolver,
    );
    let Some(script) = descriptor.script.as_ref() else {
        return context;
    };
    if !extend_vue3_type_context_from_external_imports(
        &descriptor.filename,
        script.content.as_str(),
        script_source_type_from_attrs(&script.attrs),
        &mut context,
        type_resolver,
    ) {
        return context;
    }
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
    if analysis.type_dependency_work_exhausted {
        return Vue27TypeContext::default();
    }
    finalize_vue3_local_generic_alias_scopes(&mut analysis);
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

#[derive(Clone, Debug, PartialEq, Eq)]
struct Vue3GlobalTypeParameterSignature {
    source: Option<String>,
    scope_key: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Vue3GlobalInterfacePropertySignature {
    source: Option<String>,
    optional: bool,
    readonly: bool,
    method: bool,
    scope_key: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Vue3GlobalClassMemberKind {
    Property,
    Method,
    Getter,
    Setter,
    Index,
}

struct Vue3GlobalClassMemberSignature {
    kind: Vue3GlobalClassMemberKind,
    signature: Vue3GlobalInterfacePropertySignature,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Vue3GlobalInterfaceMemberKey {
    Named(String),
    Index(String),
    ComputedGlobal(String),
    ComputedScoped { binding: String, tail: String },
}

struct Vue3GlobalScopedRoots<'base> {
    import_names: BTreeSet<String>,
    module_local_names: BTreeSet<String>,
    global_names: BTreeSet<String>,
    global_root_names: BTreeSet<String>,
    global_value_names: BTreeSet<String>,
    identities: BTreeMap<String, String>,
    definition_key: String,
    ambient_declarations: bool,
    base_kinds: &'base Vue3GlobalDeclarationKinds,
}

#[derive(Clone, Copy)]
enum Vue3ModuleLocalGraphVisibility {
    Public,
    Private(u32),
}

type Vue3ModuleLocalGraphBindingScope = BTreeMap<String, String>;

impl Vue3GlobalScopedRoots<'_> {
    fn identity(&self, name: &str) -> Option<&str> {
        self.identities.get(name).map(String::as_str)
    }

    fn local_graph_name(&self, name: &str) -> String {
        format!("local:{}:{name}", self.definition_key)
    }

    fn local_private_graph_name(&self, block_start: u32, name: &str) -> String {
        format!("local:{}:block:{block_start}:{name}", self.definition_key)
    }
}

impl Vue3GlobalInterfacePropertySignature {
    fn work(&self) -> usize {
        self.source
            .as_ref()
            .map_or(0, String::len)
            .saturating_add(self.scope_key.as_ref().map_or(0, String::len))
            .saturating_add(std::mem::size_of::<Self>())
    }
}

impl Vue3GlobalInterfaceMemberKey {
    fn work(&self) -> usize {
        let string_work = match self {
            Self::Named(name) | Self::Index(name) | Self::ComputedGlobal(name) => name.len(),
            Self::ComputedScoped { binding, tail } => binding.len().saturating_add(tail.len()),
        };
        string_work.saturating_add(std::mem::size_of::<Self>())
    }
}

impl Vue3GlobalTypeParameterSignature {
    fn work(&self) -> usize {
        self.source
            .as_ref()
            .map_or(0, String::len)
            .saturating_add(self.scope_key.as_ref().map_or(0, String::len))
            .saturating_add(std::mem::size_of::<Self>())
    }
}

fn vue3_global_type_parameter_signatures_are_compatible(
    left: &Vue3GlobalTypeParameterSignature,
    right: &Vue3GlobalTypeParameterSignature,
) -> bool {
    if left.scope_key != right.scope_key {
        return false;
    }
    if left == right {
        return true;
    }
    let (Some(left), Some(right)) = (&left.source, &right.source) else {
        return false;
    };
    let source = format!(
        "interface __Vue3GlobalLeft{left} {{}}\ninterface __Vue3GlobalRight{right} {{}}"
    );
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, &source, oxc_span::SourceType::d_ts())
        .with_options(oxc_parser::ParseOptions {
            preserve_parens: false,
            ..oxc_parser::ParseOptions::default()
        })
        .parse();
    if parsed.panicked || !parsed.errors.is_empty() || parsed.program.body.len() != 2 {
        return false;
    }
    let (
        Statement::TSInterfaceDeclaration(left),
        Statement::TSInterfaceDeclaration(right),
    ) = (&parsed.program.body[0], &parsed.program.body[1])
    else {
        return false;
    };
    oxc_span::ContentEq::content_eq(&left.type_parameters, &right.type_parameters)
}

fn vue3_global_interface_property_signatures_are_compatible(
    left: &Vue3GlobalInterfacePropertySignature,
    right: &Vue3GlobalInterfacePropertySignature,
) -> bool {
    if left.optional != right.optional
        || left.readonly != right.readonly
        || left.method != right.method
        || left.scope_key != right.scope_key
    {
        return false;
    }
    if left.source == right.source {
        return true;
    }
    let (Some(left), Some(right)) = (&left.source, &right.source) else {
        return false;
    };
    let source = format!("type __Vue3GlobalLeft = {left}\ntype __Vue3GlobalRight = {right}");
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, &source, oxc_span::SourceType::d_ts())
        .with_options(oxc_parser::ParseOptions {
            preserve_parens: false,
            ..oxc_parser::ParseOptions::default()
        })
        .parse();
    if parsed.panicked || !parsed.errors.is_empty() || parsed.program.body.len() != 2 {
        return false;
    }
    let (
        Statement::TSTypeAliasDeclaration(left),
        Statement::TSTypeAliasDeclaration(right),
    ) = (&parsed.program.body[0], &parsed.program.body[1])
    else {
        return false;
    };
    oxc_span::ContentEq::content_eq(&left.type_annotation, &right.type_annotation)
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Vue3GlobalDeclarationKinds {
    namespace_names: BTreeSet<String>,
    interface_names: BTreeSet<String>,
    callable_interface_names: BTreeSet<String>,
    interface_property_signatures: BTreeMap<
        String,
        BTreeMap<Vue3GlobalInterfaceMemberKey, Vue3GlobalInterfacePropertySignature>,
    >,
    mergeable_type_parameters: BTreeMap<String, Vue3GlobalTypeParameterSignature>,
    enum_names: BTreeSet<String>,
    enum_constness: BTreeMap<String, bool>,
    enum_omitted_first_initializer_definitions: BTreeMap<String, String>,
    enum_member_definitions: BTreeMap<String, BTreeMap<String, String>>,
    class_names: BTreeSet<String>,
    type_alias_names: BTreeSet<String>,
    value_names: BTreeSet<String>,
    function_value_names: BTreeSet<String>,
    variable_value_names: BTreeSet<String>,
    value_type_projections: BTreeMap<String, Vue3ValueTypeProjection>,
    conflicting_type_names: BTreeSet<String>,
    blocked_type_names: BTreeSet<String>,
    conflicting_value_names: BTreeSet<String>,
    blocked_value_names: BTreeSet<String>,
    type_declaration_type_references: BTreeMap<String, BTreeSet<String>>,
    type_declaration_value_references: BTreeMap<String, BTreeSet<String>>,
    value_declaration_type_references: BTreeMap<String, BTreeSet<String>>,
    value_declaration_value_references: BTreeMap<String, BTreeSet<String>>,
    dual_space_names: BTreeSet<String>,
    declaration_counts: BTreeMap<String, usize>,
    class_counts: BTreeMap<String, usize>,
}

impl Vue3GlobalDeclarationKinds {
    fn extend(&mut self, source: &Self) {
        for name in source.declaration_counts.keys() {
            let count = self.declaration_counts.entry(name.clone()).or_default();
            *count = count.saturating_add(1);
        }
        for name in source.class_counts.keys() {
            let count = self.class_counts.entry(name.clone()).or_default();
            *count = count.saturating_add(1);
        }
        self.namespace_names
            .extend(source.namespace_names.iter().cloned());
        self.interface_names
            .extend(source.interface_names.iter().cloned());
        self.callable_interface_names
            .extend(source.callable_interface_names.iter().cloned());
        for (name, properties) in &source.interface_property_signatures {
            let target = self
                .interface_property_signatures
                .entry(name.clone())
                .or_default();
            for (property, signature) in properties {
                target
                    .entry(property.clone())
                    .or_insert_with(|| signature.clone());
            }
        }
        for (name, parameters) in &source.mergeable_type_parameters {
            if !self.mergeable_type_parameters.contains_key(name) {
                self.mergeable_type_parameters
                    .insert(name.clone(), parameters.clone());
            }
        }
        self.enum_names.extend(source.enum_names.iter().cloned());
        for (name, is_const) in &source.enum_constness {
            self.enum_constness
                .entry(name.clone())
                .or_insert(*is_const);
        }
        for (name, definition) in &source.enum_omitted_first_initializer_definitions {
            self.enum_omitted_first_initializer_definitions
                .entry(name.clone())
                .or_insert_with(|| definition.clone());
        }
        for (name, members) in &source.enum_member_definitions {
            let target = self
                .enum_member_definitions
                .entry(name.clone())
                .or_default();
            for (member, definition) in members {
                target
                    .entry(member.clone())
                    .or_insert_with(|| definition.clone());
            }
        }
        self.class_names.extend(source.class_names.iter().cloned());
        self.type_alias_names
            .extend(source.type_alias_names.iter().cloned());
        self.value_names.extend(source.value_names.iter().cloned());
        self.function_value_names
            .extend(source.function_value_names.iter().cloned());
        self.variable_value_names
            .extend(source.variable_value_names.iter().cloned());
        self.value_type_projections
            .extend(source.value_type_projections.clone());
        self.conflicting_type_names
            .extend(source.conflicting_type_names.iter().cloned());
        self.blocked_type_names
            .extend(source.blocked_type_names.iter().cloned());
        self.conflicting_value_names
            .extend(source.conflicting_value_names.iter().cloned());
        self.blocked_value_names
            .extend(source.blocked_value_names.iter().cloned());
        macro_rules! extend_reference_map {
            ($field:ident) => {
                for (name, references) in &source.$field {
                    self.$field
                        .entry(name.clone())
                        .or_default()
                        .extend(references.iter().cloned());
                }
            };
        }
        extend_reference_map!(type_declaration_type_references);
        extend_reference_map!(type_declaration_value_references);
        extend_reference_map!(value_declaration_type_references);
        extend_reference_map!(value_declaration_value_references);
        self.dual_space_names
            .extend(source.dual_space_names.iter().cloned());
    }

    fn work(&self) -> usize {
        let kind_work = self
            .namespace_names
            .iter()
            .chain(&self.interface_names)
            .chain(&self.callable_interface_names)
            .chain(&self.enum_names)
            .chain(&self.class_names)
            .chain(&self.type_alias_names)
            .chain(&self.value_names)
            .chain(&self.function_value_names)
            .chain(&self.variable_value_names)
            .chain(&self.conflicting_type_names)
            .chain(&self.blocked_type_names)
            .chain(&self.conflicting_value_names)
            .chain(&self.blocked_value_names)
            .chain(&self.dual_space_names)
            .fold(0usize, |work, name| {
                work.saturating_add(name.len()).saturating_add(1)
            });
        let parameter_work = self.mergeable_type_parameters.iter().fold(
            0usize,
            |work, (name, parameters)| {
                work
                    .saturating_add(name.len())
                    .saturating_add(parameters.work())
                    .saturating_add(1)
            },
        );
        let property_work = self.interface_property_signatures.iter().fold(
            0usize,
            |work, (name, properties)| {
                properties.iter().fold(
                    work.saturating_add(name.len()).saturating_add(1),
                    |work, (property, signature)| {
                        work
                            .saturating_add(property.work())
                            .saturating_add(signature.work())
                            .saturating_add(1)
                    },
                )
            },
        );
        let enum_member_work = self.enum_member_definitions.iter().fold(
            0usize,
            |work, (name, members)| {
                members.iter().fold(
                    work.saturating_add(name.len()).saturating_add(1),
                    |work, (member, definition)| {
                        work
                            .saturating_add(member.len())
                            .saturating_add(definition.len())
                            .saturating_add(2)
                    },
                )
            },
        );
        let enum_constness_work = self.enum_constness.keys().fold(0usize, |work, name| {
            work
                .saturating_add(name.len())
                .saturating_add(std::mem::size_of::<bool>())
                .saturating_add(1)
        });
        let enum_initializer_work = self
            .enum_omitted_first_initializer_definitions
            .iter()
            .fold(0usize, |work, (name, definition)| {
                work
                    .saturating_add(name.len())
                    .saturating_add(definition.len())
                    .saturating_add(2)
            });
        let value_work = self.value_type_projections.iter().fold(
            0usize,
            |work, (name, projection)| {
                work
                    .saturating_add(name.len())
                    .saturating_add(projection.work())
                    .saturating_add(1)
            },
        );
        let reference_work = self
            .type_declaration_type_references
            .iter()
            .chain(&self.type_declaration_value_references)
            .chain(&self.value_declaration_type_references)
            .chain(&self.value_declaration_value_references)
            .fold(0usize, |work, (name, references)| {
                references.iter().fold(
                    work.saturating_add(name.len()).saturating_add(1),
                    |work, reference| {
                        work.saturating_add(reference.len()).saturating_add(1)
                    },
                )
            });
        self.declaration_counts
            .keys()
            .fold(
                kind_work
                    .saturating_add(parameter_work)
                    .saturating_add(property_work)
                    .saturating_add(enum_constness_work)
                    .saturating_add(enum_initializer_work)
                    .saturating_add(enum_member_work)
                    .saturating_add(value_work)
                    .saturating_add(reference_work),
                |work, name| {
                    work
                        .saturating_add(name.len())
                        .saturating_add(std::mem::size_of::<usize>())
                },
            )
            .saturating_add(self.class_counts.keys().fold(0usize, |work, name| {
                work
                    .saturating_add(name.len())
                    .saturating_add(std::mem::size_of::<usize>())
            }))
    }

    fn finish_file_scan_work(&self) -> usize {
        let declaration_work = self
            .interface_names
            .iter()
            .chain(&self.enum_names)
            .chain(&self.class_names)
            .chain(&self.type_alias_names)
            .chain(&self.value_names)
            .fold(0usize, |work, name| {
                work
                    .saturating_add(name.len())
                    .saturating_add(std::mem::size_of::<usize>())
            });
        let class_work = self.class_names.iter().fold(0usize, |work, name| {
            work
                .saturating_add(name.len())
                .saturating_add(std::mem::size_of::<usize>())
        });
        let type_work = self
            .interface_names
            .iter()
            .chain(&self.enum_names)
            .chain(&self.class_names)
            .chain(&self.type_alias_names)
            .fold(0usize, |work, name| {
                work.saturating_add(name.len()).saturating_add(1)
            });
        let value_conflict_work = self
            .variable_value_names
            .intersection(&self.function_value_names)
            .chain(
                self.value_names
                    .iter()
                    .filter(|name| self.class_names.contains(*name) || self.enum_names.contains(*name)),
            )
            .fold(0usize, |work, name| {
                work.saturating_add(name.len()).saturating_add(1)
            });
        declaration_work
            .saturating_add(class_work)
            .saturating_add(type_work)
            .saturating_add(value_conflict_work)
    }

    fn finish_file_scan(&mut self) {
        for name in self
            .interface_names
            .iter()
            .chain(&self.enum_names)
            .chain(&self.class_names)
            .chain(&self.type_alias_names)
            .chain(&self.value_names)
        {
            self.declaration_counts.entry(name.clone()).or_insert(1);
        }
        for name in &self.class_names {
            self.class_counts.entry(name.clone()).or_insert(1);
        }
        self.conflicting_value_names.extend(
            self.variable_value_names
                .intersection(&self.function_value_names)
                .cloned(),
        );
        let type_names = self
            .interface_names
            .iter()
            .chain(&self.enum_names)
            .chain(&self.class_names)
            .chain(&self.type_alias_names)
            .cloned()
            .collect::<BTreeSet<_>>();
        for name in type_names {
            let has_alias = self.type_alias_names.contains(&name);
            let has_interface = self.interface_names.contains(&name);
            let has_enum = self.enum_names.contains(&name);
            let has_class = self.class_names.contains(&name);
            let has_value = self.value_names.contains(&name);
            if (has_alias && (has_interface || has_enum || has_class))
                || (has_enum && (has_interface || has_class))
                || (has_value && (has_alias || has_enum || has_class))
            {
                if has_value && (has_enum || has_class) {
                    self.conflicting_value_names.insert(name.clone());
                }
                self.conflicting_type_names.insert(name);
            }
        }
    }
}

struct Vue3GlobalTypeFile {
    path: PathBuf,
    source: std::sync::Arc<Vue3ExternalTypeSource>,
}

struct Vue3GlobalTypeFileProjection {
    context: Vue27TypeContext,
    kinds: Vue3GlobalDeclarationKinds,
}

#[derive(Clone, Copy)]
enum Vue3GlobalProjectionAccounting {
    ExternalContextBuild,
    InternalFixedPoint,
}

fn vue3_global_generic_propagation_horizon(
    context: &Vue27TypeContext,
    kinds: &Vue3GlobalDeclarationKinds,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<usize> {
    let root_work = context.generic_type_aliases.keys().fold(0usize, |work, name| {
        work
            .saturating_add(name.len())
            .saturating_add(std::mem::size_of::<(bool, String)>())
            .saturating_add(1)
    });
    let reference_node_work = kinds
        .type_declaration_type_references
        .values()
        .chain(kinds.type_declaration_value_references.values())
        .chain(kinds.value_declaration_type_references.values())
        .chain(kinds.value_declaration_value_references.values())
        .fold(0usize, |work, references| {
            work.saturating_add(
                references
                    .len()
                    .saturating_mul(std::mem::size_of::<(bool, String)>())
                    .saturating_mul(2),
            )
        });
    if !namespace_budget.reserve(
        kinds
            .work()
            .saturating_mul(2)
            .saturating_add(root_work.saturating_mul(2))
            .saturating_add(reference_node_work),
    ) {
        return None;
    }
    let mut reachable = BTreeSet::<(bool, String)>::new();
    let mut pending = Vec::new();
    for name in context.generic_type_aliases.keys() {
        let node = (false, name.clone());
        reachable.insert(node.clone());
        pending.push(node);
    }
    while let Some((value_space, name)) = pending.pop() {
        let (type_references, value_references) = if value_space {
            (
                kinds.value_declaration_type_references.get(&name),
                kinds.value_declaration_value_references.get(&name),
            )
        } else {
            (
                kinds.type_declaration_type_references.get(&name),
                kinds.type_declaration_value_references.get(&name),
            )
        };
        for (reference_value_space, references) in
            [(false, type_references), (true, value_references)]
        {
            let Some(references) = references else {
                continue;
            };
            for reference in references {
                let node = (reference_value_space, reference.clone());
                if reachable.insert(node.clone()) {
                    pending.push(node);
                }
            }
        }
    }
    Some(reachable.len().saturating_add(1).max(1))
}

fn vue3_global_generic_alias_payloads_eq(
    left: &BTreeMap<String, Vue3GenericTypeAlias>,
    right: &BTreeMap<String, Vue3GenericTypeAlias>,
) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|((left_name, left), (right_name, right))| {
            left_name == right_name && vue3_generic_type_alias_semantically_eq(left, right)
        })
}

fn vue3_global_generic_environment_stability_eq(
    left: &Vue3GenericTypeEnvironment,
    right: &Vue3GenericTypeEnvironment,
) -> bool {
    left.definition_filename == right.definition_filename
        && left.definition_resolution_mode == right.definition_resolution_mode
        && vue3_global_generic_alias_payloads_eq(
            &left.generic_type_aliases,
            &right.generic_type_aliases,
        )
        && left.declared_types == right.declared_types
        && left.define_model_declared_types == right.define_model_declared_types
        && left.type_query_declared_types == right.type_query_declared_types
        && left.define_model_type_query_declared_types
            == right.define_model_type_query_declared_types
        && left.keyof_type_query_declared_types == right.keyof_type_query_declared_types
        && left.props_type_declarations == right.props_type_declarations
        && left.keyof_runtime_type_declarations == right.keyof_runtime_type_declarations
        && left.tuple_runtime_type_declarations == right.tuple_runtime_type_declarations
        && left.define_model_tuple_runtime_type_declarations
            == right.define_model_tuple_runtime_type_declarations
        && left.array_element_runtime_type_declarations
            == right.array_element_runtime_type_declarations
        && left.define_model_array_element_runtime_type_declarations
            == right.define_model_array_element_runtime_type_declarations
        && left.parameter_tuple_runtime_type_declarations
            == right.parameter_tuple_runtime_type_declarations
        && left.define_model_parameter_tuple_runtime_type_declarations
            == right.define_model_parameter_tuple_runtime_type_declarations
        && left.constructor_parameter_tuple_runtime_type_declarations
            == right.constructor_parameter_tuple_runtime_type_declarations
        && left.define_model_constructor_parameter_tuple_runtime_type_declarations
            == right.define_model_constructor_parameter_tuple_runtime_type_declarations
        && left.return_type_runtime_type_declarations
            == right.return_type_runtime_type_declarations
        && left.define_model_return_type_runtime_type_declarations
            == right.define_model_return_type_runtime_type_declarations
        && left.props_options_type_declarations == right.props_options_type_declarations
        && left.return_type_props_options_declarations
            == right.return_type_props_options_declarations
        && left.string_literal_type_declarations == right.string_literal_type_declarations
        && left.ordered_string_literal_type_declarations
            == right.ordered_string_literal_type_declarations
        && left.unresolved_import_sources == right.unresolved_import_sources
        && left.silent_unresolved_type_names == right.silent_unresolved_type_names
}

fn vue3_global_generic_scope_stability_eq(
    left: &Vue3GenericTypeScope,
    right: &Vue3GenericTypeScope,
) -> bool {
    match (left, right) {
        (Vue3GenericTypeScope::Local, Vue3GenericTypeScope::Local) => true,
        (Vue3GenericTypeScope::Captured(left), Vue3GenericTypeScope::Captured(right)) => {
            std::sync::Arc::ptr_eq(left, right)
                || vue3_global_generic_environment_stability_eq(left, right)
        }
        _ => false,
    }
}

fn vue3_global_generic_alias_stability_eq(
    left: &Vue3GenericTypeAlias,
    right: &Vue3GenericTypeAlias,
) -> bool {
    vue3_generic_type_alias_semantically_eq(left, right)
        && vue3_global_generic_scope_stability_eq(&left.scope, &right.scope)
        && left
            .interface_fragments
            .iter()
            .zip(&right.interface_fragments)
            .all(|(left, right)| {
                vue3_global_generic_scope_stability_eq(&left.scope, &right.scope)
            })
}

fn vue3_global_generic_aliases_stability_eq(
    left: &BTreeMap<String, Vue3GenericTypeAlias>,
    right: &BTreeMap<String, Vue3GenericTypeAlias>,
) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|((left_name, left), (right_name, right))| {
                left_name == right_name && vue3_global_generic_alias_stability_eq(left, right)
            })
}

fn vue3_global_type_context_stability_eq(
    left: &Vue27TypeContext,
    right: &Vue27TypeContext,
) -> bool {
    left.declared_types == right.declared_types
        && left.define_model_declared_types == right.define_model_declared_types
        && left.type_query_declared_types == right.type_query_declared_types
        && left.define_model_type_query_declared_types
            == right.define_model_type_query_declared_types
        && left.keyof_type_query_declared_types == right.keyof_type_query_declared_types
        && left.props_type_declarations == right.props_type_declarations
        && left.keyof_runtime_type_declarations == right.keyof_runtime_type_declarations
        && left.tuple_runtime_type_declarations == right.tuple_runtime_type_declarations
        && left.define_model_tuple_runtime_type_declarations
            == right.define_model_tuple_runtime_type_declarations
        && left.array_element_runtime_type_declarations
            == right.array_element_runtime_type_declarations
        && left.define_model_array_element_runtime_type_declarations
            == right.define_model_array_element_runtime_type_declarations
        && left.parameter_tuple_runtime_type_declarations
            == right.parameter_tuple_runtime_type_declarations
        && left.define_model_parameter_tuple_runtime_type_declarations
            == right.define_model_parameter_tuple_runtime_type_declarations
        && left.constructor_parameter_tuple_runtime_type_declarations
            == right.constructor_parameter_tuple_runtime_type_declarations
        && left.define_model_constructor_parameter_tuple_runtime_type_declarations
            == right.define_model_constructor_parameter_tuple_runtime_type_declarations
        && left.return_type_runtime_type_declarations
            == right.return_type_runtime_type_declarations
        && left.define_model_return_type_runtime_type_declarations
            == right.define_model_return_type_runtime_type_declarations
        && left.props_options_type_declarations == right.props_options_type_declarations
        && left.return_type_props_options_declarations
            == right.return_type_props_options_declarations
        && vue3_global_generic_aliases_stability_eq(
            &left.generic_type_aliases,
            &right.generic_type_aliases,
        )
        && left.string_literal_type_declarations == right.string_literal_type_declarations
        && left.ordered_string_literal_type_declarations
            == right.ordered_string_literal_type_declarations
        && left.emits_type_declarations == right.emits_type_declarations
        && left.type_sources == right.type_sources
        && left.type_direct_deps == right.type_direct_deps
        && left.type_deps == right.type_deps
        && left.unresolved_import_sources == right.unresolved_import_sources
        && left.silent_unresolved_type_names == right.silent_unresolved_type_names
}

#[cfg(test)]
pub(crate) fn vue3_global_type_context(
    filename: &str,
    global_type_files: &[String],
    type_resolver: &Vue3TypeResolverContext,
) -> Vue27TypeContext {
    vue3_global_type_context_with_module_sources(filename, global_type_files, &[], type_resolver)
}

fn vue3_global_type_context_with_module_sources(
    filename: &str,
    global_type_files: &[String],
    inline_module_sources: &[Vue3InlineModuleSource<'_>],
    type_resolver: &Vue3TypeResolverContext,
) -> Vue27TypeContext {
    let mut seen = BTreeSet::new();
    let explicit_paths = global_type_files
        .iter()
        .map(|file| normalize_path_components(PathBuf::from(file)));
    let mut paths = Vec::new();
    for path in explicit_paths.chain(vue3_tsconfig_global_type_files(filename, type_resolver)) {
        if !seen.insert(vue3_external_type_context_cache_key(
            &path,
            type_resolver,
        )) {
            continue;
        }
        paths.push(path);
    }
    let mut files = Vec::with_capacity(paths.len());
    for path in paths {
        let Some(source) = vue3_external_global_type_source_from_path(&path, type_resolver) else {
            return Vue27TypeContext::default();
        };
        files.push(Vue3GlobalTypeFile { path, source });
    }
    let Some(additional_global_paths) = vue3_reachable_global_augmentation_files(
        filename,
        &files,
        inline_module_sources,
        type_resolver,
    ) else {
        return Vue27TypeContext::default();
    };
    for path in additional_global_paths {
        if seen.insert(vue3_external_type_context_cache_key(
            &path,
            type_resolver,
        )) {
            let Some(source) = vue3_external_global_type_source_from_path(&path, type_resolver)
            else {
                return Vue27TypeContext::default();
            };
            files.push(Vue3GlobalTypeFile { path, source });
        }
    }

    let mut context = Vue27TypeContext::default();
    let mut kinds = Vue3GlobalDeclarationKinds::default();
    let mut accepted = Vec::new();
    let mut merge_budget = Vue3NamespaceProjectionBudget::default();
    for file in files {
        if !reserve_vue3_global_type_file_rebuild_work(&file, &context, &mut merge_budget) {
            return Vue27TypeContext::default();
        }
        let Some(projection) = vue3_global_type_projection_from_file(
            &file,
            &context,
            &kinds,
            type_resolver,
            Vue3GlobalProjectionAccounting::ExternalContextBuild,
            &mut merge_budget,
        ) else {
            return Vue27TypeContext::default();
        };
        if projection.context.type_sources.is_empty()
            && projection.context.unresolved_import_sources.is_empty()
            && projection.context.silent_unresolved_type_names.is_empty()
            && projection.kinds.declaration_counts.is_empty()
        {
            continue;
        }
        if !merge_vue3_global_type_file_projection(
            &mut context,
            &mut kinds,
            projection,
            &mut merge_budget,
        ) {
            return Vue27TypeContext::default();
        }
        accepted.push(file);
    }
    if accepted.len() < 2 {
        return if apply_vue3_global_interface_heritage_conflicts(
            &mut context,
            &mut kinds,
            &mut merge_budget,
        ) {
            context
        } else {
            Vue27TypeContext::default()
        };
    }

    let has_generic_declarations = !context.generic_type_aliases.is_empty();
    let convergence_limit = context
        .type_sources
        .len()
        .saturating_add(accepted.len())
        .saturating_add(1);
    // A Jacobi round advances one type/value declaration edge. Fresh captured
    // environments retain older Arc DAGs, so pointer equality cannot detect
    // convergence; require enough shallow-stable rounds to cover every graph
    // node reachable from a generic declaration instead.
    let generic_propagation_horizon = if has_generic_declarations {
        let Some(horizon) = vue3_global_generic_propagation_horizon(
            &context,
            &kinds,
            &mut merge_budget,
        ) else {
            return Vue27TypeContext::default();
        };
        horizon.min(convergence_limit)
    } else {
        1
    };
    let mut stable_rounds = 0usize;
    let mut converged = has_generic_declarations;
    for _ in 0..convergence_limit {
        let clone_work = vue3_external_type_context_shallow_clone_work(&context)
            .saturating_add(kinds.work());
        if !merge_budget.reserve(clone_work) {
            return Vue27TypeContext::default();
        }
        let base_context = context.clone();
        let base_kinds = kinds.clone();
        let mut next_context = Vue27TypeContext::default();
        let mut next_kinds = Vue3GlobalDeclarationKinds::default();
        let mut completed = true;
        for file in &accepted {
            if !reserve_vue3_global_type_file_rebuild_work(
                file,
                &base_context,
                &mut merge_budget,
            ) {
                completed = false;
                break;
            }
            let Some(projection) = vue3_global_type_projection_from_file(
                file,
                &base_context,
                &base_kinds,
                type_resolver,
                Vue3GlobalProjectionAccounting::InternalFixedPoint,
                &mut merge_budget,
            ) else {
                completed = false;
                break;
            };
            if !merge_vue3_global_type_file_projection(
                &mut next_context,
                &mut next_kinds,
                projection,
                &mut merge_budget,
            ) {
                completed = false;
                break;
            }
        }
        if !completed {
            return Vue27TypeContext::default();
        }
        let comparison_work = if has_generic_declarations {
            vue3_external_type_context_stability_comparison_work(&context)
                .saturating_add(vue3_external_type_context_stability_comparison_work(
                    &next_context,
                ))
        } else {
            vue3_external_type_context_cache_cost(&context)
                .saturating_add(vue3_external_type_context_cache_cost(&next_context))
        }
        .saturating_add(kinds.work())
        .saturating_add(next_kinds.work());
        if !merge_budget.reserve(comparison_work) {
            return Vue27TypeContext::default();
        }
        let stable = next_kinds == kinds
            && if has_generic_declarations {
                vue3_global_type_context_stability_eq(&context, &next_context)
            } else {
                next_context == context
            };
        if stable {
            stable_rounds = stable_rounds.saturating_add(1);
        } else {
            stable_rounds = 0;
        };
        context = next_context;
        kinds = next_kinds;
        if stable_rounds >= generic_propagation_horizon {
            converged = true;
            break;
        }
    }
    if converged
        && apply_vue3_global_interface_heritage_conflicts(
            &mut context,
            &mut kinds,
            &mut merge_budget,
        )
    {
        context
    } else {
        Vue27TypeContext::default()
    }
}

fn apply_vue3_global_interface_heritage_conflicts(
    context: &mut Vue27TypeContext,
    kinds: &mut Vue3GlobalDeclarationKinds,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> bool {
    let comparison_work = kinds.interface_names.iter().fold(0usize, |work, name| {
        let heritage_work = context
            .props_type_declarations
            .get(name)
            .and_then(|members| members.interface_heritage.as_ref())
            .map_or(0, Vue3InterfaceHeritageEvidence::work);
        work
            .saturating_add(name.len())
            .saturating_add(heritage_work.saturating_mul(2))
            .saturating_add(1)
    });
    if !namespace_budget.reserve(comparison_work) {
        return false;
    }
    let root_type_conflicts = kinds
        .interface_names
        .iter()
        .filter(|name| {
            !kinds.class_names.contains(*name)
                && context
                    .props_type_declarations
                    .get(*name)
                    .and_then(|members| members.interface_heritage.as_ref())
                    .is_some_and(vue3_interface_heritage_has_proven_conflict)
        })
        .cloned()
        .collect::<BTreeSet<_>>();
    if root_type_conflicts.is_empty() {
        return true;
    }
    let Some((mut blocked_type_names, mut blocked_value_names)) =
        vue3_global_blocked_declaration_spaces(
            kinds,
            &Vue3GlobalDeclarationKinds::default(),
            &root_type_conflicts,
            &BTreeSet::new(),
            namespace_budget,
        )
    else {
        return false;
    };
    blocked_type_names.retain(|name| kinds.declaration_counts.contains_key(name));
    blocked_value_names.retain(|name| kinds.declaration_counts.contains_key(name));
    let type_mutation_work = blocked_type_names.iter().fold(0usize, |work, name| {
        let preserved_value_work = (!blocked_value_names.contains(name)
            && kinds.value_names.contains(name))
        .then(|| kinds.value_type_projections.get(name))
        .flatten()
        .map_or(0, Vue3ValueTypeProjection::work);
        work
            .saturating_add(vue3_external_type_alias_projection_work(
                context,
                name,
                name.len(),
                "",
            ))
            .saturating_add(name.len().saturating_mul(9))
            .saturating_add(preserved_value_work.saturating_mul(2))
            .saturating_add(1)
    });
    let value_mutation_work = blocked_value_names.iter().fold(0usize, |work, name| {
        work
            .saturating_add(vue3_external_type_alias_projection_work(
                context,
                name,
                name.len(),
                "",
            ))
            .saturating_add(name.len().saturating_mul(4))
            .saturating_add(1)
    });
    let conflict_tracking_work = root_type_conflicts
        .iter()
        .chain(&blocked_type_names)
        .chain(&blocked_value_names)
        .fold(0usize, |work, name| {
            work.saturating_add(name.len()).saturating_add(1)
        });
    let mutation_work = type_mutation_work
        .saturating_add(value_mutation_work)
        .saturating_add(conflict_tracking_work);
    if !namespace_budget.reserve(mutation_work) {
        return false;
    }
    let preserved_value_projections = blocked_type_names
        .difference(&blocked_value_names)
        .filter(|name| kinds.value_names.contains(*name))
        .map(|name| {
            (
                name.clone(),
                kinds
                    .value_type_projections
                    .get(name)
                    .cloned()
                    .unwrap_or_default(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    for name in &blocked_type_names {
        clear_vue3_conflicting_global_type_projection(context, name);
        if let Some(projection) = preserved_value_projections.get(name) {
            projection.apply(context, name);
        }
        context.silent_unresolved_type_names.insert(name.clone());
    }
    for name in blocked_value_names.difference(&blocked_type_names) {
        clear_vue3_conflicting_global_value_projection(context, name);
    }
    kinds
        .conflicting_type_names
        .extend(root_type_conflicts.iter().cloned());
    kinds.blocked_type_names.extend(
        blocked_type_names
            .into_iter()
            .filter(|name| !root_type_conflicts.contains(name)),
    );
    kinds.blocked_value_names.extend(blocked_value_names);
    true
}

fn reserve_vue3_global_type_file_rebuild_work(
    file: &Vue3GlobalTypeFile,
    base_context: &Vue27TypeContext,
    merge_budget: &mut Vue3NamespaceProjectionBudget,
) -> bool {
    merge_budget.reserve(
        file.source
            .source
            .len()
            .saturating_add(vue3_external_type_context_cache_cost(base_context)),
    )
}

#[cfg(test)]
pub(crate) fn vue3_global_type_context_from_path(
    path: &Path,
    base_context: &Vue27TypeContext,
    type_resolver: &Vue3TypeResolverContext,
) -> Option<Vue27TypeContext> {
    let source = vue3_external_global_type_source_from_path(path, type_resolver)?;
    let mut namespace_budget = Vue3NamespaceProjectionBudget::default();
    vue3_global_type_projection_from_file(
        &Vue3GlobalTypeFile {
            path: path.to_path_buf(),
            source,
        },
        base_context,
        &Vue3GlobalDeclarationKinds::default(),
        type_resolver,
        Vue3GlobalProjectionAccounting::ExternalContextBuild,
        &mut namespace_budget,
    )
    .map(|projection| projection.context)
}

fn vue3_global_type_projection_from_file(
    file: &Vue3GlobalTypeFile,
    base_context: &Vue27TypeContext,
    base_kinds: &Vue3GlobalDeclarationKinds,
    type_resolver: &Vue3TypeResolverContext,
    accounting: Vue3GlobalProjectionAccounting,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<Vue3GlobalTypeFileProjection> {
    if matches!(
        accounting,
        Vue3GlobalProjectionAccounting::ExternalContextBuild
    ) {
        if !type_resolver
            .external_type_session
            .has_context_build_capacity()
        {
            return None;
        }
        let initial_weight = file
            .source
            .source
            .len()
            .saturating_add(vue3_external_type_context_cache_cost(base_context));
        if !type_resolver
            .external_type_session
            .begin_uncached_context_load(initial_weight)
        {
            return None;
        }
    }
    let normalized = normalize_path_string(&file.path);
    let projection = vue3_global_type_projection_from_source(
        &file.source.source,
        &normalized,
        file.source.source_type,
        file.source.resolution_mode,
        base_context,
        base_kinds,
        type_resolver,
        namespace_budget,
    )?;
    let context = match accounting {
        Vue3GlobalProjectionAccounting::ExternalContextBuild => type_resolver
            .external_type_session
            .finish_uncached_context_load(projection.context)?,
        Vue3GlobalProjectionAccounting::InternalFixedPoint => projection.context,
    };
    Some(Vue3GlobalTypeFileProjection {
        context,
        kinds: projection.kinds,
    })
}

fn vue3_global_type_projection_from_source(
    source: &str,
    filename: &str,
    source_type: oxc_span::SourceType,
    static_resolution_mode: Vue3TypeResolutionMode,
    base_context: &Vue27TypeContext,
    base_kinds: &Vue3GlobalDeclarationKinds,
    type_resolver: &Vue3TypeResolverContext,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<Vue3GlobalTypeFileProjection> {
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, source, source_type)
        .with_options(oxc_parser::ParseOptions {
            parse_regular_expression: true,
            preserve_parens: false,
            ..oxc_parser::ParseOptions::default()
    })
    .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return None;
    }
    let format_forces_module_scope =
        source_type.is_commonjs() && !source_type.is_typescript_definition();
    let program_is_global_script = !format_forces_module_scope
        && !vue3_javascript_statements_have_commonjs_module_indicator(
            &parsed.program.body,
            source_type,
        )
        && (source_type.is_typescript_definition() || parsed.program.source_type.is_script())
        && vue3_statements_are_ambient_global_scope(&parsed.program.body);

    let dependency = normalize_path_string(Path::new(filename));
    if !namespace_budget.reserve(vue3_external_type_context_cache_cost(base_context)) {
        return None;
    }
    let mut seed_context = base_context.clone();
    let mut seen = BTreeSet::new();
    if !extend_vue3_type_context_from_external_imports_with_seen_and_mode(
        filename,
        source,
        source_type,
        static_resolution_mode,
        &mut seed_context,
        &mut seen,
        type_resolver,
        namespace_budget,
    ) {
        return None;
    }
    let mut kinds = vue3_global_declaration_kinds(
        source,
        &dependency,
        &parsed.program.body,
        source_type.is_typescript_definition(),
        program_is_global_script,
        static_resolution_mode,
        type_resolver,
        base_kinds,
        namespace_budget,
    )?;
    let enum_comparison_work = kinds.enum_names.iter().fold(0usize, |work, name| {
        work.saturating_add(vue3_global_enum_comparison_work(
            base_kinds,
            &kinds,
            name,
        ))
    });
    if !namespace_budget.reserve(enum_comparison_work) {
        return None;
    }
    let has_cross_file_declaration_merge = kinds
        .declaration_counts
        .keys()
        .any(|name| {
            base_kinds.declaration_counts.get(name).copied().unwrap_or(0) > 1
                && vue3_global_declaration_kinds_can_merge_with_file(
                    base_kinds,
                    &kinds,
                    name,
                )
        });
    let isolated_cross_space_names = vue3_global_cross_space_declaration_names(base_kinds, &kinds);
    let isolation_work = isolated_cross_space_names.iter().fold(0usize, |work, name| {
        work.saturating_add(vue3_external_type_alias_projection_work(
            &seed_context,
            name,
            name.len(),
            "",
        ))
    });
    if !namespace_budget.reserve(isolation_work) {
        return None;
    }
    let empty_projection = Vue3ScriptSetupAnalysis::default();
    for name in &isolated_cross_space_names {
        sync_vue3_type_alias_to_context(&mut seed_context, &empty_projection, name, name);
    }
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
        type_resolution_mode: static_resolution_mode,
        type_resolver: type_resolver.clone(),
        ..Vue3ScriptSetupAnalysis::default()
    };
    analysis
        .local_ts_enum_type_names
        .extend(base_kinds.enum_names.iter().cloned());
    let (mut global_names, global_import_names) =
        collect_vue3_global_types_from_statements_with_budget_and_kinds(
            source,
            &parsed.program.body,
            source_type.is_typescript_definition(),
            program_is_global_script,
            base_context,
            base_kinds,
            &kinds,
            &mut analysis,
            namespace_budget,
        )?;
    if !namespace_budget.reserve(vue3_local_generic_scope_capture_work(&analysis)) {
        return None;
    }
    finalize_vue3_local_generic_alias_scopes(&mut analysis);
    let re_exported = project_vue3_global_type_re_exports(
        filename,
        &parsed.program.body,
        static_resolution_mode,
        &mut analysis,
        type_resolver,
        namespace_budget,
    )?;
    global_names.extend(re_exported);
    collect_vue3_global_type_deps_from_statements(&parsed.program.body, &mut analysis);
    if analysis.type_dependency_work_exhausted {
        return None;
    }
    if has_cross_file_declaration_merge {
        refresh_vue3_cross_file_global_declarations(
            source,
            &parsed.program.body,
            base_context,
            base_kinds,
            &kinds,
            &mut analysis,
            namespace_budget,
        )?;
        if !namespace_budget.reserve(vue3_local_generic_scope_capture_work(&analysis)) {
            return None;
        }
        finalize_vue3_local_generic_alias_scopes(&mut analysis);
    }
    seed_vue3_external_type_deps(filename, &mut analysis);
    let value_projection_work = kinds.value_names.iter().fold(0usize, |work, name| {
        work
            .saturating_add(name.len())
            .saturating_add(
                analysis
                    .value_type_projections
                    .get(name)
                    .map_or(std::mem::size_of::<Vue3ValueTypeProjection>(), |projection| {
                        projection.work()
                    }),
            )
    });
    if !namespace_budget.reserve(value_projection_work) {
        return None;
    }
    for name in &kinds.value_names {
        kinds.value_type_projections.insert(
            name.clone(),
            analysis
                .value_type_projections
                .get(name)
                .cloned()
                .unwrap_or_default(),
        );
    }
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
    for name in kinds.declaration_counts.keys() {
        context
            .type_deps
            .entry(name.clone())
            .or_default()
            .insert(dependency.clone());
    }
    Some(Vue3GlobalTypeFileProjection { context, kinds })
}

fn vue3_global_declaration_kinds(
    source: &str,
    definition_key: &str,
    statements: &[Statement<'_>],
    is_typescript_definition: bool,
    program_is_global_script: bool,
    static_resolution_mode: Vue3TypeResolutionMode,
    type_resolver: &Vue3TypeResolverContext,
    base_kinds: &Vue3GlobalDeclarationKinds,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<Vue3GlobalDeclarationKinds> {
    if !namespace_budget.reserve(source.len().saturating_add(statements.len())) {
        return None;
    }
    let is_ambient = program_is_global_script;
    let import_names =
        vue3_global_type_file_import_names_with_budget(statements, namespace_budget)?;
    if !namespace_budget.reserve(definition_key.len().saturating_add(1)) {
        return None;
    }
    let mut scoped_roots = Vue3GlobalScopedRoots {
        import_names,
        module_local_names: BTreeSet::new(),
        global_names: BTreeSet::new(),
        global_root_names: BTreeSet::new(),
        global_value_names: BTreeSet::new(),
        identities: vue3_global_type_file_import_scope_identities_with_budget(
            statements,
            definition_key,
            static_resolution_mode,
            type_resolver,
            namespace_budget,
        )?,
        definition_key: definition_key.to_string(),
        ambient_declarations: is_typescript_definition,
        base_kinds,
    };
    if !is_ambient {
        let (_, mut lexical_roots) =
            vue3_module_lexical_type_names_with_budget(statements, namespace_budget)?;
        lexical_roots.extend(vue3_module_lexical_value_root_names_with_budget(
            statements,
            namespace_budget,
        )?);
        let directly_exported_roots =
            vue3_module_direct_exported_root_names_with_budget(statements, namespace_budget)?;
        scoped_roots.module_local_names =
            vue3_module_local_graph_names_with_budget(
                statements,
                is_typescript_definition,
                namespace_budget,
            )?;
        for root in lexical_roots {
            if !namespace_budget.reserve(
                root.len().saturating_mul(2)
                    .saturating_add(definition_key.len())
                    .saturating_add(8),
            ) {
                return None;
            }
            let identity = if directly_exported_roots.contains(&root) {
                format!("{definition_key}#{root}")
            } else {
                format!("local:{definition_key}#{root}")
            };
            scoped_roots.identities.insert(root, identity);
        }
    }
    let groups = vue3_global_declaration_statement_groups_with_budget(
        statements,
        is_ambient,
        namespace_budget,
    )?;
    for group in &groups {
        let (names, roots) =
            vue3_module_lexical_type_names_with_budget(group, namespace_budget)?;
        scoped_roots.global_names.extend(names);
        scoped_roots.global_root_names.extend(roots);
        collect_vue3_global_value_names_from_statements(
            group,
            None,
            0,
            &mut scoped_roots.global_value_names,
            namespace_budget,
        )?;
    }
    let mut kinds = Vue3GlobalDeclarationKinds::default();
    for (index, group) in groups.iter().enumerate() {
        let namespace_ambient = !is_ambient || index != 0 || is_typescript_definition;
        collect_vue3_global_declaration_kinds_from_statements(
            source,
            group,
            None,
            true,
            namespace_ambient,
            0,
            &scoped_roots,
            &mut kinds,
            namespace_budget,
        )?;
    }
    if !is_ambient {
        for statement in statements {
            let is_global_container = matches!(statement, Statement::TSGlobalDeclaration(_))
                || matches!(
                    statement,
                    Statement::TSModuleDeclaration(declaration)
                        if vue3_ts_module_declaration_is_global(declaration)
                )
                || matches!(
                    statement,
                    Statement::ExportNamedDeclaration(export)
                        if matches!(
                            export.declaration.as_ref(),
                            Some(Declaration::TSModuleDeclaration(declaration))
                                if vue3_ts_module_declaration_is_global(declaration)
                        )
                );
            if is_global_container {
                continue;
            }
            collect_vue3_global_declaration_references_from_statement(
                statement,
                None,
                &scoped_roots,
                base_kinds,
                true,
                &[],
                &mut kinds,
                namespace_budget,
            )?;
        }
    }
    for group in groups {
        collect_vue3_global_declaration_references_from_statements(
            group,
            None,
            0,
            &scoped_roots,
            base_kinds,
            false,
            &[],
            &mut kinds,
            namespace_budget,
        )?;
    }
    let scope_key_work = kinds
        .mergeable_type_parameters
        .values()
        .filter(|signature| signature.scope_key.as_deref() == Some(""))
        .fold(0usize, |work, _| {
            work.saturating_add(definition_key.len()).saturating_add(1)
        })
        .saturating_add(
            kinds
                .interface_property_signatures
                .values()
                .flat_map(BTreeMap::values)
                .filter(|signature| signature.scope_key.as_deref() == Some(""))
                .fold(0usize, |work, _| {
                    work.saturating_add(definition_key.len()).saturating_add(1)
                }),
        );
    if !namespace_budget.reserve(scope_key_work) {
        return None;
    }
    for signature in kinds.mergeable_type_parameters.values_mut() {
        if signature.scope_key.as_deref() == Some("") {
            signature.scope_key = Some(definition_key.to_string());
        }
    }
    for signature in kinds
        .interface_property_signatures
        .values_mut()
        .flat_map(BTreeMap::values_mut)
    {
        if signature.scope_key.as_deref() == Some("") {
            signature.scope_key = Some(definition_key.to_string());
        }
    }
    if !namespace_budget.reserve(kinds.finish_file_scan_work()) {
        return None;
    }
    kinds.finish_file_scan();
    Some(kinds)
}

fn vue3_module_direct_exported_root_names_with_budget(
    statements: &[Statement<'_>],
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<BTreeSet<String>> {
    let mut names = BTreeSet::new();
    for statement in statements {
        let Statement::ExportNamedDeclaration(export) = statement else {
            continue;
        };
        let Some(declaration) = export.declaration.as_ref() else {
            continue;
        };
        let mut insert = |name: &str| {
            if names.contains(name) {
                return true;
            }
            if !namespace_budget.reserve(name.len().saturating_add(1)) {
                return false;
            }
            names.insert(name.to_string());
            true
        };
        let inserted = match declaration {
            Declaration::VariableDeclaration(declaration) => declaration
                .declarations
                .iter()
                .filter_map(|declarator| first_pattern_binding_name(&declarator.id))
                .all(&mut insert),
            Declaration::FunctionDeclaration(declaration) => declaration
                .id
                .as_ref()
                .is_none_or(|id| insert(id.name.as_str())),
            Declaration::ClassDeclaration(declaration) => declaration
                .id
                .as_ref()
                .is_none_or(|id| insert(id.name.as_str())),
            Declaration::TSInterfaceDeclaration(declaration) => {
                insert(declaration.id.name.as_str())
            }
            Declaration::TSTypeAliasDeclaration(declaration) => {
                insert(declaration.id.name.as_str())
            }
            Declaration::TSEnumDeclaration(declaration) => insert(declaration.id.name.as_str()),
            Declaration::TSModuleDeclaration(declaration) => {
                vue3_ts_module_declaration_name(declaration)
                    .as_deref()
                    .is_none_or(&mut insert)
            }
            _ => true,
        };
        if !inserted {
            return None;
        }
    }
    Some(names)
}

fn collect_vue3_global_value_names_from_statements(
    statements: &[Statement<'_>],
    prefix: Option<&str>,
    depth: usize,
    names: &mut BTreeSet<String>,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<()> {
    if depth > VUE3_MAX_NAMESPACE_PROJECTION_DEPTH {
        namespace_budget.exhausted = true;
        return None;
    }
    for statement in statements {
        match statement {
            Statement::FunctionDeclaration(function) => {
                if let Some(id) = &function.id {
                    names.insert(reserve_vue3_global_declaration_name(
                        prefix,
                        id.name.as_str(),
                        namespace_budget,
                    )?);
                }
            }
            Statement::VariableDeclaration(declaration) => {
                for declarator in &declaration.declarations {
                    if let Some(name) = first_pattern_binding_name(&declarator.id) {
                        names.insert(reserve_vue3_global_declaration_name(
                            prefix,
                            name,
                            namespace_budget,
                        )?);
                    }
                }
            }
            Statement::TSEnumDeclaration(declaration) => {
                names.insert(reserve_vue3_global_declaration_name(
                    prefix,
                    declaration.id.name.as_str(),
                    namespace_budget,
                )?);
            }
            Statement::ClassDeclaration(declaration) => {
                if let Some(id) = &declaration.id {
                    names.insert(reserve_vue3_global_declaration_name(
                        prefix,
                        id.name.as_str(),
                        namespace_budget,
                    )?);
                }
            }
            Statement::TSModuleDeclaration(declaration)
                if !vue3_ts_module_declaration_is_global(declaration) =>
            {
                collect_vue3_global_value_names_from_namespace(
                    declaration,
                    prefix,
                    depth.saturating_add(1),
                    names,
                    namespace_budget,
                )?;
            }
            Statement::ExportNamedDeclaration(export) => match export.declaration.as_ref() {
                Some(Declaration::FunctionDeclaration(function)) => {
                    if let Some(id) = &function.id {
                        names.insert(reserve_vue3_global_declaration_name(
                            prefix,
                            id.name.as_str(),
                            namespace_budget,
                        )?);
                    }
                }
                Some(Declaration::VariableDeclaration(declaration)) => {
                    for declarator in &declaration.declarations {
                        if let Some(name) = first_pattern_binding_name(&declarator.id) {
                            names.insert(reserve_vue3_global_declaration_name(
                                prefix,
                                name,
                                namespace_budget,
                            )?);
                        }
                    }
                }
                Some(Declaration::TSEnumDeclaration(declaration)) => {
                    names.insert(reserve_vue3_global_declaration_name(
                        prefix,
                        declaration.id.name.as_str(),
                        namespace_budget,
                    )?);
                }
                Some(Declaration::ClassDeclaration(declaration)) => {
                    if let Some(id) = &declaration.id {
                        names.insert(reserve_vue3_global_declaration_name(
                            prefix,
                            id.name.as_str(),
                            namespace_budget,
                        )?);
                    }
                }
                Some(Declaration::TSModuleDeclaration(declaration))
                    if !vue3_ts_module_declaration_is_global(declaration) =>
                {
                    collect_vue3_global_value_names_from_namespace(
                        declaration,
                        prefix,
                        depth.saturating_add(1),
                        names,
                        namespace_budget,
                    )?;
                }
                _ => {}
            },
            _ => {}
        }
    }
    Some(())
}

fn collect_vue3_global_value_names_from_namespace(
    declaration: &TSModuleDeclaration<'_>,
    prefix: Option<&str>,
    depth: usize,
    names: &mut BTreeSet<String>,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<()> {
    if depth > VUE3_MAX_NAMESPACE_PROJECTION_DEPTH {
        namespace_budget.exhausted = true;
        return None;
    }
    let Some(name) = vue3_ts_module_declaration_name(declaration) else {
        return Some(());
    };
    let qualified = reserve_vue3_global_declaration_name(prefix, &name, namespace_budget)?;
    if let Some(body) = declaration.body.as_ref() {
        match body {
            TSModuleDeclarationBody::TSModuleBlock(block) => {
                collect_vue3_global_value_names_from_statements(
                    &block.body,
                    Some(&qualified),
                    depth,
                    names,
                    namespace_budget,
                )?;
            }
            TSModuleDeclarationBody::TSModuleDeclaration(nested) => {
                collect_vue3_global_value_names_from_namespace(
                    nested,
                    Some(&qualified),
                    depth.saturating_add(1),
                    names,
                    namespace_budget,
                )?;
            }
        }
    }
    names.insert(qualified);
    Some(())
}

fn collect_vue3_global_declaration_kinds_from_statements(
    source: &str,
    statements: &[Statement<'_>],
    prefix: Option<&str>,
    members_are_ambient: bool,
    namespace_is_ambient: bool,
    depth: usize,
    scoped_roots: &Vue3GlobalScopedRoots,
    kinds: &mut Vue3GlobalDeclarationKinds,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<()> {
    if depth > VUE3_MAX_NAMESPACE_PROJECTION_DEPTH {
        namespace_budget.exhausted = true;
        return None;
    }
    for statement in statements {
        match statement {
            Statement::TSTypeAliasDeclaration(declaration) if members_are_ambient => {
                insert_vue3_global_type_alias_declaration_kind(
                    prefix,
                    declaration.id.name.as_str(),
                    kinds,
                    namespace_budget,
                )?;
            }
            Statement::FunctionDeclaration(function) if members_are_ambient => {
                if let Some(id) = &function.id {
                    insert_vue3_global_function_declaration_kind(
                        prefix,
                        id.name.as_str(),
                        kinds,
                        namespace_budget,
                    )?;
                }
            }
            Statement::VariableDeclaration(declaration) if members_are_ambient => {
                collect_vue3_global_variable_declaration_kind_names(
                    declaration,
                    prefix,
                    kinds,
                    namespace_budget,
                )?;
            }
            Statement::TSInterfaceDeclaration(declaration) if members_are_ambient => {
                insert_vue3_global_interface_declaration_kind(
                    source,
                    declaration,
                    prefix,
                    scoped_roots,
                    kinds,
                    namespace_budget,
                )?;
            }
            Statement::TSEnumDeclaration(declaration) if members_are_ambient => {
                insert_vue3_global_enum_declaration_kind(
                    declaration,
                    prefix,
                    scoped_roots,
                    kinds,
                    namespace_budget,
                )?;
            }
            Statement::ClassDeclaration(declaration) if members_are_ambient => {
                insert_vue3_global_class_declaration_kind(
                    source,
                    declaration,
                    prefix,
                    scoped_roots,
                    kinds,
                    namespace_budget,
                )?;
            }
            Statement::TSModuleDeclaration(declaration)
                if !vue3_ts_module_declaration_is_global(declaration) =>
            {
                collect_vue3_global_declaration_kinds_from_namespace(
                    source,
                    declaration,
                    prefix,
                    namespace_is_ambient || declaration.declare,
                    depth.saturating_add(1),
                    scoped_roots,
                    kinds,
                    namespace_budget,
                )?;
            }
            Statement::ExportNamedDeclaration(export) => match export.declaration.as_ref() {
                Some(Declaration::TSTypeAliasDeclaration(declaration)) => {
                    insert_vue3_global_type_alias_declaration_kind(
                        prefix,
                        declaration.id.name.as_str(),
                        kinds,
                        namespace_budget,
                    )?;
                }
                Some(Declaration::FunctionDeclaration(function)) => {
                    if let Some(id) = &function.id {
                        insert_vue3_global_function_declaration_kind(
                            prefix,
                            id.name.as_str(),
                            kinds,
                            namespace_budget,
                        )?;
                    }
                }
                Some(Declaration::VariableDeclaration(declaration)) => {
                    collect_vue3_global_variable_declaration_kind_names(
                        declaration,
                        prefix,
                        kinds,
                        namespace_budget,
                    )?;
                }
                Some(Declaration::TSInterfaceDeclaration(declaration)) => {
                    insert_vue3_global_interface_declaration_kind(
                        source,
                        declaration,
                        prefix,
                        scoped_roots,
                        kinds,
                        namespace_budget,
                    )?;
                }
                Some(Declaration::TSEnumDeclaration(declaration)) => {
                    insert_vue3_global_enum_declaration_kind(
                        declaration,
                        prefix,
                        scoped_roots,
                        kinds,
                        namespace_budget,
                    )?;
                }
                Some(Declaration::ClassDeclaration(declaration)) => {
                    insert_vue3_global_class_declaration_kind(
                        source,
                        declaration,
                        prefix,
                        scoped_roots,
                        kinds,
                        namespace_budget,
                    )?;
                }
                Some(Declaration::TSModuleDeclaration(declaration))
                    if !vue3_ts_module_declaration_is_global(declaration) =>
                {
                    collect_vue3_global_declaration_kinds_from_namespace(
                        source,
                        declaration,
                        prefix,
                        namespace_is_ambient || declaration.declare,
                        depth.saturating_add(1),
                        scoped_roots,
                        kinds,
                        namespace_budget,
                    )?;
                }
                _ => {}
            },
            _ => {}
        }
    }
    Some(())
}

#[allow(clippy::too_many_arguments)]
fn collect_vue3_global_declaration_references_from_statements(
    statements: &[Statement<'_>],
    prefix: Option<&str>,
    depth: usize,
    scoped_roots: &Vue3GlobalScopedRoots,
    base_kinds: &Vue3GlobalDeclarationKinds,
    include_module_local: bool,
    local_binding_scopes: &[Vue3ModuleLocalGraphBindingScope],
    kinds: &mut Vue3GlobalDeclarationKinds,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<()> {
    if depth > VUE3_MAX_NAMESPACE_PROJECTION_DEPTH {
        namespace_budget.exhausted = true;
        return None;
    }
    for statement in statements {
        let namespace = match statement {
            Statement::TSModuleDeclaration(declaration)
                if !vue3_ts_module_declaration_is_global(declaration) =>
            {
                Some(declaration.as_ref())
            }
            Statement::ExportNamedDeclaration(export) => export
                .declaration
                .as_ref()
                .and_then(|declaration| match declaration {
                    Declaration::TSModuleDeclaration(declaration)
                        if !vue3_ts_module_declaration_is_global(declaration) =>
                    {
                        Some(declaration.as_ref())
                    }
                    _ => None,
                }),
            _ => None,
        };
        if let Some(namespace) = namespace {
            collect_vue3_global_declaration_references_from_namespace(
                namespace,
                prefix,
                depth.saturating_add(1),
                scoped_roots,
                base_kinds,
                include_module_local,
                local_binding_scopes,
                kinds,
                namespace_budget,
            )?;
            continue;
        }
        collect_vue3_global_declaration_references_from_statement(
            statement,
            prefix,
            scoped_roots,
            base_kinds,
            include_module_local,
            local_binding_scopes,
            kinds,
            namespace_budget,
        )?;
    }
    Some(())
}

#[allow(clippy::too_many_arguments)]
fn collect_vue3_global_declaration_references_from_namespace(
    declaration: &TSModuleDeclaration<'_>,
    prefix: Option<&str>,
    depth: usize,
    scoped_roots: &Vue3GlobalScopedRoots,
    base_kinds: &Vue3GlobalDeclarationKinds,
    include_module_local: bool,
    local_binding_scopes: &[Vue3ModuleLocalGraphBindingScope],
    kinds: &mut Vue3GlobalDeclarationKinds,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<()> {
    let Some(name) = vue3_ts_module_declaration_name(declaration) else {
        return Some(());
    };
    let qualified = reserve_vue3_global_declaration_name(prefix, &name, namespace_budget)?;
    let Some(body) = declaration.body.as_ref() else {
        return Some(());
    };
    match body {
        TSModuleDeclarationBody::TSModuleBlock(block) => {
            collect_vue3_global_declaration_references_from_statements(
                &block.body,
                Some(&qualified),
                depth,
                scoped_roots,
                base_kinds,
                include_module_local,
                local_binding_scopes,
                kinds,
                namespace_budget,
            )
        }
        TSModuleDeclarationBody::TSModuleDeclaration(nested) => {
            collect_vue3_global_declaration_references_from_namespace(
                nested,
                Some(&qualified),
                depth.saturating_add(1),
                scoped_roots,
                base_kinds,
                include_module_local,
                local_binding_scopes,
                kinds,
                namespace_budget,
            )
        }
    }
}

fn reserve_vue3_module_local_graph_name(
    name: &str,
    visibility: Vue3ModuleLocalGraphVisibility,
    scoped_roots: &Vue3GlobalScopedRoots,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<String> {
    let work = scoped_roots
        .definition_key
        .len()
        .saturating_add(name.len())
        .saturating_add(match visibility {
            Vue3ModuleLocalGraphVisibility::Public => 8,
            Vue3ModuleLocalGraphVisibility::Private(_) => 32,
        });
    if !namespace_budget.reserve(work) {
        return None;
    }
    Some(match visibility {
        Vue3ModuleLocalGraphVisibility::Public => scoped_roots.local_graph_name(name),
        Vue3ModuleLocalGraphVisibility::Private(block_start) => {
            scoped_roots.local_private_graph_name(block_start, name)
        }
    })
}

fn insert_vue3_module_local_graph_binding(
    bindings: &mut Vue3ModuleLocalGraphBindingScope,
    name: String,
    graph_name: String,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<()> {
    if bindings.contains_key(&name) {
        return Some(());
    }
    if !namespace_budget.reserve(std::mem::size_of::<(String, String)>()) {
        return None;
    }
    bindings.insert(name, graph_name);
    Some(())
}

fn resolve_vue3_module_local_graph_name(
    name: &str,
    local_binding_scopes: &[Vue3ModuleLocalGraphBindingScope],
    scoped_roots: &Vue3GlobalScopedRoots,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<Option<String>> {
    if let Some(graph_name) = local_binding_scopes
        .iter()
        .rev()
        .find_map(|bindings| bindings.get(name))
    {
        if !namespace_budget.reserve(graph_name.len().saturating_add(1)) {
            return None;
        }
        return Some(Some(graph_name.clone()));
    }
    if scoped_roots.module_local_names.contains(name) {
        return reserve_vue3_module_local_graph_name(
            name,
            Vue3ModuleLocalGraphVisibility::Public,
            scoped_roots,
            namespace_budget,
        )
        .map(Some);
    }
    Some(None)
}

fn vue3_module_local_namespace_member_visibility(
    statement: &Statement<'_>,
    ambient: bool,
    inherited: Vue3ModuleLocalGraphVisibility,
    block_start: u32,
) -> Vue3ModuleLocalGraphVisibility {
    if ambient || matches!(statement, Statement::ExportNamedDeclaration(_)) {
        inherited
    } else {
        Vue3ModuleLocalGraphVisibility::Private(block_start)
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_vue3_module_local_namespace_declaration_references(
    declaration: &TSModuleDeclaration<'_>,
    prefix: Option<&str>,
    depth: usize,
    ambient: bool,
    visibility: Vue3ModuleLocalGraphVisibility,
    scoped_roots: &Vue3GlobalScopedRoots,
    base_kinds: &Vue3GlobalDeclarationKinds,
    local_binding_scopes: &mut Vec<Vue3ModuleLocalGraphBindingScope>,
    kinds: &mut Vue3GlobalDeclarationKinds,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<()> {
    if depth > VUE3_MAX_NAMESPACE_PROJECTION_DEPTH {
        namespace_budget.exhausted = true;
        return None;
    }
    let Some(name) = vue3_ts_module_declaration_name(declaration) else {
        return Some(());
    };
    let qualified = reserve_vue3_global_declaration_name(prefix, &name, namespace_budget)?;
    let Some(body) = declaration.body.as_ref() else {
        return Some(());
    };
    match body {
        TSModuleDeclarationBody::TSModuleBlock(block) => {
            let block_start = block.span.start;
            let mut bindings = Vue3ModuleLocalGraphBindingScope::new();
            for statement in &block.body {
                if let Some(nested) = vue3_namespace_declaration_from_statement(statement) {
                    let nested_ambient = ambient || nested.declare;
                    let nested_visibility = vue3_module_local_namespace_member_visibility(
                        statement,
                        ambient,
                        visibility,
                        block_start,
                    );
                    if let Some(nested_name) = vue3_ts_module_declaration_name_ref(nested) {
                        let qualified_name = reserve_vue3_global_declaration_name(
                            Some(&qualified),
                            nested_name,
                            namespace_budget,
                        )?;
                        let graph_name = reserve_vue3_module_local_graph_name(
                            &qualified_name,
                            nested_visibility,
                            scoped_roots,
                            namespace_budget,
                        )?;
                        insert_vue3_module_local_graph_binding(
                            &mut bindings,
                            qualified_name,
                            graph_name,
                            namespace_budget,
                        )?;
                    }
                    for nested_name in vue3_namespace_visible_type_names_with_budget(
                        nested,
                        nested_ambient,
                        namespace_budget,
                    )? {
                        let qualified_name = reserve_vue3_global_declaration_name(
                            Some(&qualified),
                            &nested_name,
                            namespace_budget,
                        )?;
                        let graph_name = reserve_vue3_module_local_graph_name(
                            &qualified_name,
                            nested_visibility,
                            scoped_roots,
                            namespace_budget,
                        )?;
                        insert_vue3_module_local_graph_binding(
                            &mut bindings,
                            qualified_name,
                            graph_name,
                            namespace_budget,
                        )?;
                    }
                    continue;
                }
                let member_visibility = vue3_module_local_namespace_member_visibility(
                    statement,
                    ambient,
                    visibility,
                    block_start,
                );
                for local_name in vue3_declared_type_names_from_statement_with_budget(
                    statement,
                    namespace_budget,
                )? {
                    let qualified_name = reserve_vue3_global_declaration_name(
                        Some(&qualified),
                        &local_name,
                        namespace_budget,
                    )?;
                    let graph_name = reserve_vue3_module_local_graph_name(
                        &qualified_name,
                        member_visibility,
                        scoped_roots,
                        namespace_budget,
                    )?;
                    insert_vue3_module_local_graph_binding(
                        &mut bindings,
                        qualified_name,
                        graph_name,
                        namespace_budget,
                    )?;
                }
            }
            if !namespace_budget.reserve(std::mem::size_of::<
                Vue3ModuleLocalGraphBindingScope,
            >()) {
                return None;
            }
            local_binding_scopes.push(bindings);
            for statement in &block.body {
                if let Some(nested) = vue3_namespace_declaration_from_statement(statement) {
                    let nested_visibility = vue3_module_local_namespace_member_visibility(
                        statement,
                        ambient,
                        visibility,
                        block_start,
                    );
                    collect_vue3_module_local_namespace_declaration_references(
                        nested,
                        Some(&qualified),
                        depth.saturating_add(1),
                        ambient || nested.declare,
                        nested_visibility,
                        scoped_roots,
                        base_kinds,
                        local_binding_scopes,
                        kinds,
                        namespace_budget,
                    )?;
                    continue;
                }
                collect_vue3_global_declaration_references_from_statement(
                    statement,
                    Some(&qualified),
                    scoped_roots,
                    base_kinds,
                    true,
                    local_binding_scopes,
                    kinds,
                    namespace_budget,
                )?;
            }
            local_binding_scopes.pop();
            Some(())
        }
        TSModuleDeclarationBody::TSModuleDeclaration(nested) => {
            collect_vue3_module_local_namespace_declaration_references(
                nested,
                Some(&qualified),
                depth.saturating_add(1),
                ambient || nested.declare,
                visibility,
                scoped_roots,
                base_kinds,
                local_binding_scopes,
                kinds,
                namespace_budget,
            )
        }
    }
}

fn collect_vue3_global_declaration_references_from_statement(
    statement: &Statement<'_>,
    prefix: Option<&str>,
    scoped_roots: &Vue3GlobalScopedRoots,
    base_kinds: &Vue3GlobalDeclarationKinds,
    include_module_local: bool,
    local_binding_scopes: &[Vue3ModuleLocalGraphBindingScope],
    kinds: &mut Vue3GlobalDeclarationKinds,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<()> {
    let variable = match statement {
        Statement::VariableDeclaration(declaration) => Some(declaration.as_ref()),
        Statement::ExportNamedDeclaration(export) => {
            export.declaration.as_ref().and_then(|declaration| {
                if let Declaration::VariableDeclaration(declaration) = declaration {
                    Some(declaration.as_ref())
                } else {
                    None
                }
            })
        }
        _ => None,
    };
    if let Some(variable) = variable {
        return collect_vue3_global_variable_declaration_references(
            variable,
            prefix,
            scoped_roots,
            base_kinds,
            include_module_local,
            local_binding_scopes,
            kinds,
            namespace_budget,
        );
    }
    if let Some(namespace) = vue3_namespace_declaration_from_statement(statement) {
        if include_module_local {
            let mut namespace_binding_scopes = Vec::new();
            return collect_vue3_module_local_namespace_declaration_references(
                namespace,
                prefix,
                1,
                scoped_roots.ambient_declarations || namespace.declare,
                Vue3ModuleLocalGraphVisibility::Public,
                scoped_roots,
                base_kinds,
                &mut namespace_binding_scopes,
                kinds,
                namespace_budget,
            );
        }
        return Some(());
    }
    let local_names = vue3_declared_type_names_from_statement_with_budget(
        statement,
        namespace_budget,
    )?;
    if local_names.is_empty() {
        return Some(());
    }
    let mut collector = Vue3GlobalDeclarationReferenceCollector {
        type_names: BTreeSet::new(),
        value_names: BTreeSet::new(),
        bound_type_names: Vec::new(),
        bound_value_names: Vec::new(),
        active_type_names: BTreeMap::new(),
        active_value_names: BTreeMap::new(),
        namespace_budget,
    };
    oxc_ast_visit::Visit::visit_statement(&mut collector, statement);
    if collector.namespace_budget.is_exhausted() {
        return None;
    }
    let type_references = qualify_vue3_global_declaration_references(
        &collector.type_names,
        prefix,
        false,
        scoped_roots,
        base_kinds,
        kinds,
        include_module_local,
        local_binding_scopes,
        collector.namespace_budget,
    )?;
    let value_references = qualify_vue3_global_declaration_references(
        &collector.value_names,
        prefix,
        true,
        scoped_roots,
        base_kinds,
        kinds,
        include_module_local,
        local_binding_scopes,
        collector.namespace_budget,
    )?;
    let reference_output_work = type_references
        .iter()
        .chain(&value_references)
        .fold(0usize, |work, reference| {
            work.saturating_add(reference.len()).saturating_add(1)
        })
        .saturating_mul(local_names.len());
    if !collector.namespace_budget.reserve(reference_output_work) {
        return None;
    }
    for local_name in local_names {
        let mut name = reserve_vue3_global_declaration_name(
            prefix,
            &local_name,
            collector.namespace_budget,
        )?;
        let local_graph_name = if include_module_local {
            resolve_vue3_module_local_graph_name(
                &name,
                local_binding_scopes,
                scoped_roots,
                collector.namespace_budget,
            )?
        } else {
            None
        };
        if let Some(local_graph_name) = local_graph_name {
            name = local_graph_name;
        } else if !vue3_global_declaration_kinds_have_type(kinds, &name)
            && !kinds.value_names.contains(&name)
        {
            continue;
        }
        if include_module_local && vue3_statement_declares_dual_space(statement) {
            if !collector.namespace_budget.reserve(
                name.len()
                    .saturating_add(std::mem::size_of::<String>())
                    .saturating_add(1),
            ) {
                return None;
            }
            kinds.dual_space_names.insert(name.clone());
        }
        let declares_value = vue3_statement_declares_global_value(statement);
        let has_type_references = !type_references.is_empty();
        let has_value_references = !value_references.is_empty();
        match (has_type_references, has_value_references) {
            (true, true) => {
                if !collector
                    .namespace_budget
                    .reserve(name.len().saturating_add(1))
                {
                    return None;
                }
                let type_target = if declares_value {
                    &mut kinds.value_declaration_type_references
                } else {
                    &mut kinds.type_declaration_type_references
                };
                type_target
                    .entry(name.clone())
                    .or_default()
                    .extend(type_references.iter().cloned());
                let value_target = if declares_value {
                    &mut kinds.value_declaration_value_references
                } else {
                    &mut kinds.type_declaration_value_references
                };
                value_target
                    .entry(name)
                    .or_default()
                    .extend(value_references.iter().cloned());
            }
            (true, false) => {
                let target = if declares_value {
                    &mut kinds.value_declaration_type_references
                } else {
                    &mut kinds.type_declaration_type_references
                };
                target
                    .entry(name)
                    .or_default()
                    .extend(type_references.iter().cloned());
            }
            (false, true) => {
                let target = if declares_value {
                    &mut kinds.value_declaration_value_references
                } else {
                    &mut kinds.type_declaration_value_references
                };
                target
                    .entry(name)
                    .or_default()
                    .extend(value_references.iter().cloned());
            }
            (false, false) => {}
        }
    }
    Some(())
}

fn collect_vue3_global_variable_declaration_references(
    declaration: &VariableDeclaration<'_>,
    prefix: Option<&str>,
    scoped_roots: &Vue3GlobalScopedRoots,
    base_kinds: &Vue3GlobalDeclarationKinds,
    include_module_local: bool,
    local_binding_scopes: &[Vue3ModuleLocalGraphBindingScope],
    kinds: &mut Vue3GlobalDeclarationKinds,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<()> {
    for declarator in &declaration.declarations {
        let Some(local_name) = first_pattern_binding_name(&declarator.id) else {
            continue;
        };
        let mut name =
            reserve_vue3_global_declaration_name(prefix, local_name, namespace_budget)?;
        let local_graph_name = if include_module_local {
            resolve_vue3_module_local_graph_name(
                &name,
                local_binding_scopes,
                scoped_roots,
                namespace_budget,
            )?
        } else {
            None
        };
        if let Some(local_graph_name) = local_graph_name {
            name = local_graph_name;
        } else if !kinds.value_names.contains(&name) {
            continue;
        }
        let mut collector = Vue3GlobalDeclarationReferenceCollector {
            type_names: BTreeSet::new(),
            value_names: BTreeSet::new(),
            bound_type_names: Vec::new(),
            bound_value_names: Vec::new(),
            active_type_names: BTreeMap::new(),
            active_value_names: BTreeMap::new(),
            namespace_budget,
        };
        oxc_ast_visit::Visit::visit_variable_declarator(&mut collector, declarator);
        if collector.namespace_budget.is_exhausted() {
            return None;
        }
        let type_references = qualify_vue3_global_declaration_references(
            &collector.type_names,
            prefix,
            false,
            scoped_roots,
            base_kinds,
            kinds,
            include_module_local,
            local_binding_scopes,
            collector.namespace_budget,
        )?;
        let value_references = qualify_vue3_global_declaration_references(
            &collector.value_names,
            prefix,
            true,
            scoped_roots,
            base_kinds,
            kinds,
            include_module_local,
            local_binding_scopes,
            collector.namespace_budget,
        )?;
        let clone_work = type_references
            .iter()
            .chain(&value_references)
            .fold(0usize, |work, reference| {
                work.saturating_add(reference.len()).saturating_add(1)
            });
        if !collector.namespace_budget.reserve(clone_work) {
            return None;
        }
        let has_type_references = !type_references.is_empty();
        let has_value_references = !value_references.is_empty();
        match (has_type_references, has_value_references) {
            (true, true) => {
                if !collector
                    .namespace_budget
                    .reserve(name.len().saturating_add(1))
                {
                    return None;
                }
                kinds
                    .value_declaration_type_references
                    .entry(name.clone())
                    .or_default()
                    .extend(type_references);
                kinds
                    .value_declaration_value_references
                    .entry(name)
                    .or_default()
                    .extend(value_references);
            }
            (true, false) => {
                kinds
                    .value_declaration_type_references
                    .entry(name)
                    .or_default()
                    .extend(type_references);
            }
            (false, true) => {
                kinds
                    .value_declaration_value_references
                    .entry(name)
                    .or_default()
                    .extend(value_references);
            }
            (false, false) => {}
        }
    }
    Some(())
}

fn vue3_statement_declares_global_value(statement: &Statement<'_>) -> bool {
    match statement {
        Statement::FunctionDeclaration(_) | Statement::VariableDeclaration(_) => true,
        Statement::ExportNamedDeclaration(export) => matches!(
            export.declaration.as_ref(),
            Some(Declaration::FunctionDeclaration(_) | Declaration::VariableDeclaration(_))
        ),
        _ => false,
    }
}

fn vue3_statement_declares_dual_space(statement: &Statement<'_>) -> bool {
    match statement {
        Statement::ClassDeclaration(_) | Statement::TSEnumDeclaration(_) => true,
        Statement::ExportNamedDeclaration(export) => matches!(
            export.declaration.as_ref(),
            Some(Declaration::ClassDeclaration(_) | Declaration::TSEnumDeclaration(_))
        ),
        _ => false,
    }
}

struct Vue3GlobalDeclarationReferenceCollector<'budget> {
    type_names: BTreeSet<String>,
    value_names: BTreeSet<String>,
    bound_type_names: Vec<BTreeSet<String>>,
    bound_value_names: Vec<BTreeSet<String>>,
    active_type_names: BTreeMap<String, usize>,
    active_value_names: BTreeMap<String, usize>,
    namespace_budget: &'budget mut Vue3NamespaceProjectionBudget,
}

struct Vue3InferTypeNameCollector<'budget> {
    names: BTreeSet<String>,
    namespace_budget: &'budget mut Vue3NamespaceProjectionBudget,
}

impl<'a> oxc_ast_visit::Visit<'a> for Vue3InferTypeNameCollector<'_> {
    fn visit_ts_infer_type(&mut self, infer: &oxc_ast::ast::TSInferType<'a>) {
        let name = infer.type_parameter.name.name.as_str();
        if !self.names.contains(name) {
            if !self.namespace_budget.reserve(name.len().saturating_add(1)) {
                return;
            }
            self.names.insert(name.to_string());
        }
        oxc_ast_visit::walk::walk_ts_infer_type(self, infer);
    }
}

fn vue3_infer_type_names_with_budget(
    ty: &TSType<'_>,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<BTreeSet<String>> {
    let span = ty.span();
    if !namespace_budget.reserve(
        (span.end as usize)
            .saturating_sub(span.start as usize)
            .saturating_add(1),
    ) {
        return None;
    }
    let mut collector = Vue3InferTypeNameCollector {
        names: BTreeSet::new(),
        namespace_budget,
    };
    oxc_ast_visit::Visit::visit_ts_type(&mut collector, ty);
    (!collector.namespace_budget.is_exhausted()).then_some(collector.names)
}

impl Vue3GlobalDeclarationReferenceCollector<'_> {
    fn push_binding_scope(&mut self) {
        self.bound_type_names.push(BTreeSet::new());
        self.bound_value_names.push(BTreeSet::new());
    }

    fn pop_binding_scope(&mut self) {
        if let Some(names) = self.bound_type_names.pop() {
            for name in names {
                match self.active_type_names.get(&name).copied() {
                    Some(1) => {
                        self.active_type_names.remove(&name);
                    }
                    Some(count) => {
                        self.active_type_names.insert(name, count - 1);
                    }
                    None => {}
                }
            }
        }
        if let Some(names) = self.bound_value_names.pop() {
            for name in names {
                match self.active_value_names.get(&name).copied() {
                    Some(1) => {
                        self.active_value_names.remove(&name);
                    }
                    Some(count) => {
                        self.active_value_names.insert(name, count - 1);
                    }
                    None => {}
                }
            }
        }
    }

    fn insert_binding(&mut self, name: &str, type_space: bool) {
        let already_in_scope = if type_space {
            self.bound_type_names
                .last()
                .is_some_and(|names| names.contains(name))
        } else {
            self.bound_value_names
                .last()
                .is_some_and(|names| names.contains(name))
        };
        if already_in_scope {
            return;
        }
        if !self
            .namespace_budget
            .reserve(name.len().saturating_add(1).saturating_mul(2))
        {
            return;
        }
        if type_space {
            let Some(scope) = self.bound_type_names.last_mut() else {
                return;
            };
            scope.insert(name.to_string());
            let count = self.active_type_names.entry(name.to_string()).or_default();
            *count = count.saturating_add(1);
        } else {
            let Some(scope) = self.bound_value_names.last_mut() else {
                return;
            };
            scope.insert(name.to_string());
            let count = self.active_value_names.entry(name.to_string()).or_default();
            *count = count.saturating_add(1);
        }
    }

    fn insert_reference(&mut self, name: String, value_space: bool) {
        if self.namespace_budget.is_exhausted() {
            return;
        }
        let root = name.split('.').next().unwrap_or(name.as_str());
        let is_bound = if value_space {
            self.active_value_names.contains_key(root)
        } else {
            self.active_type_names.contains_key(root)
        };
        if is_bound {
            return;
        }
        if !self.namespace_budget.reserve(name.len().saturating_add(1)) {
            return;
        }
        if value_space {
            self.value_names.insert(name);
        } else {
            self.type_names.insert(name);
        }
    }

    fn insert_computed_property_reference(&mut self, key: &PropertyKey<'_>, computed: bool) {
        if !computed || self.namespace_budget.is_exhausted() {
            return;
        }
        let Some(expression) = key.as_expression() else {
            return;
        };
        let span = expression.span();
        if !self.namespace_budget.reserve(
            (span.end as usize)
                .saturating_sub(span.start as usize)
                .saturating_add(std::mem::size_of::<String>())
                .saturating_add(1),
        ) {
            return;
        }
        if let Some(name) = vue3_expression_type_name_key(expression) {
            self.insert_reference(name, true);
        }
    }

    fn is_binding_scope(kind: &oxc_ast::ast_kind::AstKind<'_>) -> bool {
        matches!(
            kind,
            oxc_ast::ast_kind::AstKind::Function(_)
                | oxc_ast::ast_kind::AstKind::ArrowFunctionExpression(_)
                | oxc_ast::ast_kind::AstKind::Class(_)
                | oxc_ast::ast_kind::AstKind::TSCallSignatureDeclaration(_)
                | oxc_ast::ast_kind::AstKind::TSMethodSignature(_)
                | oxc_ast::ast_kind::AstKind::TSConstructSignatureDeclaration(_)
                | oxc_ast::ast_kind::AstKind::TSFunctionType(_)
                | oxc_ast::ast_kind::AstKind::TSConstructorType(_)
        )
    }
}

impl<'a> oxc_ast_visit::Visit<'a> for Vue3GlobalDeclarationReferenceCollector<'_> {
    fn enter_node(&mut self, kind: oxc_ast::ast_kind::AstKind<'a>) {
        if Self::is_binding_scope(&kind) {
            self.push_binding_scope();
        }
    }

    fn leave_node(&mut self, kind: oxc_ast::ast_kind::AstKind<'a>) {
        if Self::is_binding_scope(&kind) {
            self.pop_binding_scope();
        }
    }

    fn visit_binding_identifier(&mut self, identifier: &oxc_ast::ast::BindingIdentifier<'a>) {
        self.insert_binding(identifier.name.as_str(), false);
    }

    fn visit_ts_property_signature(&mut self, property: &oxc_ast::ast::TSPropertySignature<'a>) {
        self.insert_computed_property_reference(&property.key, property.computed);
        oxc_ast_visit::walk::walk_ts_property_signature(self, property);
    }

    fn visit_ts_method_signature(&mut self, method: &oxc_ast::ast::TSMethodSignature<'a>) {
        self.insert_computed_property_reference(&method.key, method.computed);
        oxc_ast_visit::walk::walk_ts_method_signature(self, method);
    }

    fn visit_property_definition(&mut self, property: &oxc_ast::ast::PropertyDefinition<'a>) {
        self.insert_computed_property_reference(&property.key, property.computed);
        oxc_ast_visit::walk::walk_property_definition(self, property);
    }

    fn visit_method_definition(&mut self, method: &oxc_ast::ast::MethodDefinition<'a>) {
        self.insert_computed_property_reference(&method.key, method.computed);
        oxc_ast_visit::walk::walk_method_definition(self, method);
    }

    fn visit_ts_type_parameter(&mut self, parameter: &oxc_ast::ast::TSTypeParameter<'a>) {
        self.insert_binding(parameter.name.name.as_str(), true);
        if let Some(constraint) = &parameter.constraint {
            oxc_ast_visit::Visit::visit_ts_type(self, constraint);
        }
        if let Some(default) = &parameter.default {
            oxc_ast_visit::Visit::visit_ts_type(self, default);
        }
    }

    fn visit_ts_type_parameter_declaration(
        &mut self,
        declaration: &oxc_ast::ast::TSTypeParameterDeclaration<'a>,
    ) {
        for parameter in &declaration.params {
            self.insert_binding(parameter.name.name.as_str(), true);
        }
        for parameter in &declaration.params {
            if let Some(constraint) = &parameter.constraint {
                oxc_ast_visit::Visit::visit_ts_type(self, constraint);
            }
            if let Some(default) = &parameter.default {
                oxc_ast_visit::Visit::visit_ts_type(self, default);
            }
        }
    }

    fn visit_ts_type_alias_declaration(&mut self, alias: &TSTypeAliasDeclaration<'a>) {
        self.push_binding_scope();
        if let Some(parameters) = &alias.type_parameters {
            oxc_ast_visit::Visit::visit_ts_type_parameter_declaration(self, parameters);
        }
        oxc_ast_visit::Visit::visit_ts_type(self, &alias.type_annotation);
        self.pop_binding_scope();
    }

    fn visit_ts_interface_declaration(&mut self, interface: &TSInterfaceDeclaration<'a>) {
        self.push_binding_scope();
        if let Some(parameters) = &interface.type_parameters {
            oxc_ast_visit::Visit::visit_ts_type_parameter_declaration(self, parameters);
        }
        for heritage in &interface.extends {
            oxc_ast_visit::Visit::visit_ts_interface_heritage(self, heritage);
        }
        oxc_ast_visit::Visit::visit_ts_interface_body(self, &interface.body);
        self.pop_binding_scope();
    }

    fn visit_ts_mapped_type(&mut self, mapped: &oxc_ast::ast::TSMappedType<'a>) {
        self.push_binding_scope();
        self.insert_binding(mapped.key.name.as_str(), true);
        oxc_ast_visit::Visit::visit_ts_type(self, &mapped.constraint);
        if let Some(name_type) = &mapped.name_type {
            oxc_ast_visit::Visit::visit_ts_type(self, name_type);
        }
        if let Some(annotation) = &mapped.type_annotation {
            oxc_ast_visit::Visit::visit_ts_type(self, annotation);
        }
        self.pop_binding_scope();
    }

    fn visit_ts_conditional_type(&mut self, conditional: &oxc_ast::ast::TSConditionalType<'a>) {
        oxc_ast_visit::Visit::visit_ts_type(self, &conditional.check_type);
        let Some(infer_names) = vue3_infer_type_names_with_budget(
            &conditional.extends_type,
            self.namespace_budget,
        ) else {
            return;
        };
        self.push_binding_scope();
        for name in infer_names {
            self.insert_binding(&name, true);
        }
        oxc_ast_visit::Visit::visit_ts_type(self, &conditional.extends_type);
        oxc_ast_visit::Visit::visit_ts_type(self, &conditional.true_type);
        self.pop_binding_scope();
        oxc_ast_visit::Visit::visit_ts_type(self, &conditional.false_type);
    }

    fn visit_ts_type_name(&mut self, name: &TSTypeName<'a>) {
        if self.namespace_budget.is_exhausted() {
            return;
        }
        if let Some(name) = vue3_ts_type_name_key(name) {
            self.insert_reference(name, false);
        }
    }

    fn visit_ts_type_query(&mut self, query: &TSTypeQuery<'a>) {
        if self.namespace_budget.is_exhausted() {
            return;
        }
        if let Some(name) = vue3_type_query_name_key(query) {
            self.insert_reference(name, true);
        }
        if let Some(arguments) = &query.type_arguments {
            oxc_ast_visit::Visit::visit_ts_type_parameter_instantiation(self, arguments);
        }
    }

    fn visit_ts_interface_heritage(&mut self, heritage: &TSInterfaceHeritage<'a>) {
        if self.namespace_budget.is_exhausted() {
            return;
        }
        if let Some(name) = vue3_interface_heritage_name(heritage) {
            self.insert_reference(name, false);
        }
        if let Some(arguments) = &heritage.type_arguments {
            oxc_ast_visit::Visit::visit_ts_type_parameter_instantiation(self, arguments);
        }
    }

    fn visit_class(&mut self, class: &oxc_ast::ast::Class<'a>) {
        if self.namespace_budget.is_exhausted() {
            return;
        }
        if let Some(name) = class
            .super_class
            .as_ref()
            .and_then(vue3_expression_type_name_key)
        {
            self.insert_reference(name, true);
        }
        oxc_ast_visit::walk::walk_class(self, class);
    }
}

fn qualify_vue3_global_declaration_references(
    references: &BTreeSet<String>,
    prefix: Option<&str>,
    value_space: bool,
    scoped_roots: &Vue3GlobalScopedRoots,
    base_kinds: &Vue3GlobalDeclarationKinds,
    kinds: &Vue3GlobalDeclarationKinds,
    include_module_local: bool,
    local_binding_scopes: &[Vue3ModuleLocalGraphBindingScope],
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<BTreeSet<String>> {
    let segments = prefix
        .map(|prefix| prefix.split('.').collect::<Vec<_>>())
        .unwrap_or_default();
    let mut qualified = BTreeSet::new();
    for reference in references {
        let mut resolved = None;
        for length in (1..=segments.len()).rev() {
            let path_length = segments[..length]
                .iter()
                .fold(0usize, |work, segment| work.saturating_add(segment.len()))
                .saturating_add(length.saturating_sub(1));
            let candidate_length = path_length
                .saturating_add(reference.len())
                .saturating_add(1);
            if !namespace_budget.reserve(candidate_length.saturating_add(1)) {
                return None;
            }
            let mut candidate = String::with_capacity(candidate_length);
            for segment in &segments[..length] {
                if !candidate.is_empty() {
                    candidate.push('.');
                }
                candidate.push_str(segment);
            }
            candidate.push('.');
            candidate.push_str(reference);
            if include_module_local {
                if let Some(local_graph_name) = resolve_vue3_module_local_graph_name(
                    &candidate,
                    local_binding_scopes,
                    scoped_roots,
                    namespace_budget,
                )? {
                    resolved = Some(local_graph_name);
                    break;
                }
                if value_space {
                    if let Some(local_graph_name) =
                        resolve_vue3_module_local_dual_space_member_root(
                            &candidate,
                            local_binding_scopes,
                            scoped_roots,
                            kinds,
                            namespace_budget,
                        )?
                    {
                        resolved = Some(local_graph_name);
                        break;
                    }
                }
                continue;
            }
            if value_space {
                if let Some(root_length) =
                    vue3_global_dual_space_member_root(kinds, &candidate).map(str::len)
                {
                    candidate.truncate(root_length);
                    resolved = Some(candidate);
                    break;
                }
                if let Some(root_length) =
                    vue3_global_dual_space_member_root(base_kinds, &candidate).map(str::len)
                {
                    candidate.truncate(root_length);
                    resolved = Some(candidate);
                    break;
                }
            }
            if vue3_global_declaration_reference_target_exists(kinds, &candidate, value_space)
                || (vue3_global_namespace_contains_reference(kinds, &candidate)
                    && vue3_global_declaration_reference_target_exists(
                        base_kinds,
                        &candidate,
                        value_space,
                    ))
            {
                resolved = Some(candidate);
                break;
            }
            let reference_root_length = reference.find('.').unwrap_or(reference.len());
            let namespace_binding_length = path_length
                .saturating_add(reference_root_length)
                .saturating_add(1);
            let namespace_binding = &candidate[..namespace_binding_length];
            if kinds.namespace_names.contains(namespace_binding)
                || base_kinds.namespace_names.contains(namespace_binding)
            {
                resolved = Some(candidate);
                break;
            }
        }
        let resolved = match resolved {
            Some(resolved) => resolved,
            None => {
                if value_space {
                    if let Some(root) = vue3_global_dual_space_member_root(kinds, reference) {
                        if !namespace_budget.reserve(root.len().saturating_add(1)) {
                            return None;
                        }
                        qualified.insert(root.to_string());
                        continue;
                    }
                    if let Some(local_graph_name) =
                        resolve_vue3_module_local_dual_space_member_root(
                            reference,
                            local_binding_scopes,
                            scoped_roots,
                            kinds,
                            namespace_budget,
                        )?
                    {
                        qualified.insert(local_graph_name);
                        continue;
                    }
                }
                if vue3_global_declaration_reference_target_exists(
                    kinds,
                    reference,
                    value_space,
                ) || vue3_global_namespace_contains_reference(kinds, reference)
                {
                    if !namespace_budget.reserve(reference.len().saturating_add(1)) {
                        return None;
                    }
                    reference.clone()
                } else if let Some(local_graph_name) = resolve_vue3_module_local_graph_name(
                    reference,
                    local_binding_scopes,
                    scoped_roots,
                    namespace_budget,
                )? {
                    local_graph_name
                } else {
                    let root = reference.split('.').next().unwrap_or(reference);
                    if scoped_roots.import_names.contains(root) {
                        continue;
                    }
                    if value_space {
                        if let Some(root) =
                            vue3_global_dual_space_member_root(base_kinds, reference)
                        {
                            if !namespace_budget.reserve(root.len().saturating_add(1)) {
                                return None;
                            }
                            qualified.insert(root.to_string());
                            continue;
                        }
                    }
                    if !namespace_budget.reserve(reference.len().saturating_add(1)) {
                        return None;
                    }
                    reference.clone()
                }
            }
        };
        qualified.insert(resolved);
    }
    Some(qualified)
}

fn resolve_vue3_module_local_dual_space_member_root(
    name: &str,
    local_binding_scopes: &[Vue3ModuleLocalGraphBindingScope],
    scoped_roots: &Vue3GlobalScopedRoots,
    kinds: &Vue3GlobalDeclarationKinds,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<Option<String>> {
    for (index, _) in name.match_indices('.').rev() {
        let Some(graph_name) = resolve_vue3_module_local_graph_name(
            &name[..index],
            local_binding_scopes,
            scoped_roots,
            namespace_budget,
        )? else {
            continue;
        };
        if kinds.dual_space_names.contains(&graph_name) {
            return Some(Some(graph_name));
        }
    }
    Some(None)
}

fn vue3_global_dual_space_member_root<'a>(
    kinds: &Vue3GlobalDeclarationKinds,
    name: &'a str,
) -> Option<&'a str> {
    name.match_indices('.').rev().find_map(|(index, _)| {
        let root = &name[..index];
        (kinds.class_names.contains(root) || kinds.enum_names.contains(root)).then_some(root)
    })
}

fn vue3_global_declaration_reference_target_exists(
    kinds: &Vue3GlobalDeclarationKinds,
    name: &str,
    value_space: bool,
) -> bool {
    if value_space {
        kinds.value_names.contains(name)
            || kinds.class_names.contains(name)
            || kinds.enum_names.contains(name)
    } else {
        vue3_global_declaration_kinds_have_type(kinds, name)
    }
}

fn vue3_global_namespace_contains_reference(
    kinds: &Vue3GlobalDeclarationKinds,
    name: &str,
) -> bool {
    kinds.namespace_names.contains(name)
        || name
            .match_indices('.')
            .any(|(index, _)| kinds.namespace_names.contains(&name[..index]))
}

fn collect_vue3_global_variable_declaration_kind_names(
    declaration: &VariableDeclaration<'_>,
    prefix: Option<&str>,
    kinds: &mut Vue3GlobalDeclarationKinds,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<()> {
    for declarator in &declaration.declarations {
        let Some(name) = first_pattern_binding_name(&declarator.id) else {
            continue;
        };
        let name = reserve_vue3_global_declaration_name(prefix, name, namespace_budget)?;
        if kinds.function_value_names.contains(&name)
            || !kinds.variable_value_names.insert(name.clone())
        {
            kinds.conflicting_value_names.insert(name.clone());
        }
        kinds.value_names.insert(name);
    }
    Some(())
}

fn insert_vue3_global_function_declaration_kind(
    prefix: Option<&str>,
    name: &str,
    kinds: &mut Vue3GlobalDeclarationKinds,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<()> {
    let name = reserve_vue3_global_declaration_name(prefix, name, namespace_budget)?;
    if kinds.variable_value_names.contains(&name) {
        kinds.conflicting_value_names.insert(name.clone());
    }
    kinds.value_names.insert(name.clone());
    kinds.function_value_names.insert(name);
    Some(())
}

fn insert_vue3_global_interface_declaration_kind(
    source: &str,
    declaration: &TSInterfaceDeclaration<'_>,
    prefix: Option<&str>,
    scoped_roots: &Vue3GlobalScopedRoots,
    kinds: &mut Vue3GlobalDeclarationKinds,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<()> {
    insert_vue3_global_mergeable_declaration_kind(
        source,
        &mut kinds.interface_names,
        prefix,
        declaration.id.name.as_str(),
        declaration.type_parameters.as_deref(),
        true,
        scoped_roots,
        &mut kinds.mergeable_type_parameters,
        &mut kinds.conflicting_type_names,
        namespace_budget,
    )?;
    if declaration
        .body
        .body
        .iter()
        .any(|member| matches!(member, TSSignature::TSCallSignatureDeclaration(_)))
    {
        let name = reserve_vue3_global_declaration_name(
            prefix,
            declaration.id.name.as_str(),
            namespace_budget,
        )?;
        kinds.callable_interface_names.insert(name);
    }
    insert_vue3_global_interface_property_signatures(
        source,
        declaration,
        prefix,
        scoped_roots,
        kinds,
        namespace_budget,
    )?;
    Some(())
}

fn vue3_global_interface_member_key(
    key: &PropertyKey<'_>,
    computed: bool,
    prefix: Option<&str>,
    scoped_roots: &Vue3GlobalScopedRoots,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<Option<Vue3GlobalInterfaceMemberKey>> {
    let span = key.span();
    if !namespace_budget.reserve(
        (span.end as usize)
            .saturating_sub(span.start as usize)
            .saturating_add(std::mem::size_of::<String>())
            .saturating_add(1),
    ) {
        return None;
    }
    if let Some(name) = key.static_name() {
        if !namespace_budget.reserve(std::mem::size_of::<Vue3GlobalInterfaceMemberKey>()) {
            return None;
        }
        return Some(Some(Vue3GlobalInterfaceMemberKey::Named(
            name.into_owned(),
        )));
    }
    if !computed {
        return Some(None);
    }
    let Some(expression) = key.as_expression() else {
        return Some(None);
    };
    let Some(reference) = vue3_expression_type_name_key(expression) else {
        return Some(None);
    };
    let root_length = reference.find('.').unwrap_or(reference.len());
    let root = &reference[..root_length];
    let mut current_prefix = prefix;
    while let Some(current) = current_prefix {
        let candidate = reserve_vue3_global_declaration_name(
            Some(current),
            &reference,
            namespace_budget,
        )?;
        let namespace_binding_length = current
            .len()
            .saturating_add(root_length)
            .saturating_add(1);
        let namespace_binding = &candidate[..namespace_binding_length];
        if scoped_roots.global_value_names.contains(&candidate)
            || vue3_global_declaration_reference_target_exists(
                scoped_roots.base_kinds,
                &candidate,
                true,
            )
            || scoped_roots.global_value_names.contains(namespace_binding)
            || scoped_roots
                .base_kinds
                .namespace_names
                .contains(namespace_binding)
        {
            if !namespace_budget.reserve(std::mem::size_of::<Vue3GlobalInterfaceMemberKey>()) {
                return None;
            }
            return Some(Some(Vue3GlobalInterfaceMemberKey::ComputedGlobal(
                candidate,
            )));
        }
        current_prefix = current.rsplit_once('.').map(|(parent, _)| parent);
    }
    if scoped_roots.global_value_names.contains(&reference)
        || scoped_roots.global_value_names.contains(root)
    {
        if !namespace_budget.reserve(std::mem::size_of::<Vue3GlobalInterfaceMemberKey>()) {
            return None;
        }
        return Some(Some(Vue3GlobalInterfaceMemberKey::ComputedGlobal(
            reference,
        )));
    }
    if let Some(identity) = scoped_roots.identity(root) {
        let tail = &reference[root_length..];
        if !identity.starts_with("local:") {
            if let (Some(source), Some(export_path)) =
                (identity.strip_suffix("#*"), tail.strip_prefix('.'))
            {
                let exported = export_path.split('.').next().unwrap_or(export_path);
                let canonical_tail = &tail[exported.len().saturating_add(1)..];
                if !namespace_budget.reserve(
                    source
                        .len()
                        .saturating_add(exported.len())
                        .saturating_add(canonical_tail.len())
                        .saturating_add(std::mem::size_of::<Vue3GlobalInterfaceMemberKey>())
                        .saturating_add(3),
                ) {
                    return None;
                }
                return Some(Some(Vue3GlobalInterfaceMemberKey::ComputedScoped {
                    binding: format!("{source}#{exported}"),
                    tail: canonical_tail.to_string(),
                }));
            }
        }
        if !namespace_budget.reserve(
            identity
                .len()
                .saturating_add(tail.len())
                .saturating_add(std::mem::size_of::<Vue3GlobalInterfaceMemberKey>())
                .saturating_add(2),
        ) {
            return None;
        }
        return Some(Some(Vue3GlobalInterfaceMemberKey::ComputedScoped {
            binding: identity.to_string(),
            tail: tail.to_string(),
        }));
    }
    if vue3_global_declaration_reference_target_exists(
        scoped_roots.base_kinds,
        &reference,
        true,
    ) || scoped_roots.base_kinds.namespace_names.contains(root)
    {
        if !namespace_budget.reserve(std::mem::size_of::<Vue3GlobalInterfaceMemberKey>()) {
            return None;
        }
        return Some(Some(Vue3GlobalInterfaceMemberKey::ComputedGlobal(
            reference,
        )));
    }
    if !namespace_budget.reserve(std::mem::size_of::<Vue3GlobalInterfaceMemberKey>()) {
        return None;
    }
    Some(Some(Vue3GlobalInterfaceMemberKey::ComputedGlobal(
        reference,
    )))
}

fn insert_vue3_global_interface_property_signatures(
    source: &str,
    declaration: &TSInterfaceDeclaration<'_>,
    prefix: Option<&str>,
    scoped_roots: &Vue3GlobalScopedRoots,
    kinds: &mut Vue3GlobalDeclarationKinds,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<()> {
    let name = reserve_vue3_global_declaration_name(
        prefix,
        declaration.id.name.as_str(),
        namespace_budget,
    )?;
    let parameter_work = declaration
        .type_parameters
        .iter()
        .flat_map(|parameters| &parameters.params)
        .fold(0usize, |work, parameter| {
            work
                .saturating_add(parameter.name.name.len())
                .saturating_add(1)
        });
    if !namespace_budget.reserve(parameter_work) {
        return None;
    }
    let parameter_names = declaration
        .type_parameters
        .iter()
        .flat_map(|parameters| &parameters.params)
        .map(|parameter| parameter.name.name.to_string())
        .collect::<BTreeSet<_>>();
    let mut properties = kinds
        .interface_property_signatures
        .remove(&name)
        .unwrap_or_default();
    let mut conflicting = kinds.conflicting_type_names.contains(&name);
    for member in &declaration.body.body {
        if let TSSignature::TSIndexSignature(index) = member {
            let Some((property_name, signature)) = vue3_global_index_signature(
                source,
                index,
                &parameter_names,
                prefix,
                scoped_roots,
                namespace_budget,
            )?
            else {
                conflicting = true;
                continue;
            };
            merge_vue3_global_member_signature(
                property_name,
                signature,
                &mut properties,
                &mut conflicting,
                namespace_budget,
            )?;
            continue;
        }
        if let TSSignature::TSMethodSignature(method) = member {
            let Some(property_name) = vue3_global_interface_member_key(
                &method.key,
                method.computed,
                prefix,
                scoped_roots,
                namespace_budget,
            )?
            else {
                if method.computed {
                    conflicting = true;
                }
                continue;
            };
            let signature = Vue3GlobalInterfacePropertySignature {
                source: None,
                optional: method.optional,
                readonly: false,
                method: true,
                scope_key: None,
            };
            if !namespace_budget.reserve(
                property_name
                    .work()
                    .saturating_add(signature.work())
                    .saturating_add(1),
            ) {
                return None;
            }
            merge_vue3_global_member_signature(
                property_name,
                signature,
                &mut properties,
                &mut conflicting,
                namespace_budget,
            )?;
            continue;
        }
        let TSSignature::TSPropertySignature(property) = member else {
            continue;
        };
        let Some(property_name) = vue3_global_interface_member_key(
            &property.key,
            property.computed,
            prefix,
            scoped_roots,
            namespace_budget,
        )?
        else {
            if property.computed {
                conflicting = true;
            }
            continue;
        };
        let ty = property
            .type_annotation
            .as_ref()
            .map(|annotation| &annotation.type_annotation);
        let type_source = match ty {
            Some(ty) => {
                let span = ty.span();
                Some(source.get(span.start as usize..span.end as usize)?)
            }
            None => None,
        };
        let scope_key = match ty {
            Some(ty) => vue3_type_scope_key(
                ty,
                &parameter_names,
                scoped_roots,
                prefix,
                namespace_budget,
            )?,
            None => None,
        };
        let signature_work = type_source
            .map_or(0, str::len)
            .saturating_add(scope_key.as_ref().map_or(0, String::len))
            .saturating_add(std::mem::size_of::<Vue3GlobalInterfacePropertySignature>());
        if !namespace_budget.reserve(
            property_name
                .work()
                .saturating_add(signature_work)
                .saturating_add(1),
        ) {
            return None;
        }
        let signature = Vue3GlobalInterfacePropertySignature {
            source: type_source.map(str::to_string),
            optional: property.optional,
            readonly: property.readonly,
            method: false,
            scope_key,
        };
        merge_vue3_global_member_signature(
            property_name,
            signature,
            &mut properties,
            &mut conflicting,
            namespace_budget,
        )?;
    }
    if conflicting {
        kinds.conflicting_type_names.insert(name.clone());
    }
    if !properties.is_empty() {
        kinds.interface_property_signatures.insert(name, properties);
    }
    Some(())
}

fn vue3_global_index_signature(
    source: &str,
    index: &oxc_ast::ast::TSIndexSignature<'_>,
    parameter_names: &BTreeSet<String>,
    prefix: Option<&str>,
    scoped_roots: &Vue3GlobalScopedRoots,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<Option<(
    Vue3GlobalInterfaceMemberKey,
    Vue3GlobalInterfacePropertySignature,
)>> {
    let Some(parameter) = index.parameters.first() else {
        return Some(None);
    };
    let key_type = &parameter.type_annotation.type_annotation;
    let key_span = key_type.span();
    let key_source = source.get(key_span.start as usize..key_span.end as usize)?;
    if !namespace_budget.reserve(
        key_source
            .len()
            .saturating_add(std::mem::size_of::<Vue3GlobalInterfaceMemberKey>())
            .saturating_add(1),
    ) {
        return None;
    }
    let property_name = Vue3GlobalInterfaceMemberKey::Index(key_source.to_string());
    let ty = &index.type_annotation.type_annotation;
    let span = ty.span();
    let type_source = source.get(span.start as usize..span.end as usize)?;
    let scope_key = vue3_type_scope_key(
        ty,
        parameter_names,
        scoped_roots,
        prefix,
        namespace_budget,
    )?;
    let signature = Vue3GlobalInterfacePropertySignature {
        source: Some(type_source.to_string()),
        optional: false,
        readonly: index.readonly,
        method: false,
        scope_key,
    };
    if !namespace_budget.reserve(
        property_name
            .work()
            .saturating_add(signature.work())
            .saturating_add(1),
    ) {
        return None;
    }
    Some(Some((property_name, signature)))
}

fn merge_vue3_global_member_signature(
    property_name: Vue3GlobalInterfaceMemberKey,
    signature: Vue3GlobalInterfacePropertySignature,
    properties: &mut BTreeMap<
        Vue3GlobalInterfaceMemberKey,
        Vue3GlobalInterfacePropertySignature,
    >,
    conflicting: &mut bool,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<()> {
    if *conflicting {
        return Some(());
    }
    if let Some(existing) = properties.get(&property_name) {
        if !namespace_budget.reserve(
            existing
                .work()
                .saturating_add(signature.work())
                .saturating_add(property_name.work()),
        ) {
            return None;
        }
        if matches!(property_name, Vue3GlobalInterfaceMemberKey::Index(_))
            || !vue3_global_interface_property_signatures_are_compatible(existing, &signature)
        {
            *conflicting = true;
        }
    } else {
        properties.insert(property_name, signature);
    }
    Some(())
}

fn vue3_enum_member_name<'a>(
    member: &'a oxc_ast::ast::TSEnumMemberName<'a>,
) -> Option<&'a str> {
    match member {
        oxc_ast::ast::TSEnumMemberName::Identifier(identifier) => Some(identifier.name.as_str()),
        oxc_ast::ast::TSEnumMemberName::String(literal)
        | oxc_ast::ast::TSEnumMemberName::ComputedString(literal) => {
            Some(literal.value.as_str())
        }
        oxc_ast::ast::TSEnumMemberName::ComputedTemplateString(template)
            if template.expressions.is_empty() =>
        {
            template.quasis.first().map(|quasi| {
                quasi
                    .value
                    .cooked
                    .as_ref()
                    .unwrap_or(&quasi.value.raw)
                    .as_str()
            })
        }
        oxc_ast::ast::TSEnumMemberName::ComputedTemplateString(_) => None,
    }
}

fn insert_vue3_global_enum_declaration_kind(
    declaration: &TSEnumDeclaration<'_>,
    prefix: Option<&str>,
    scoped_roots: &Vue3GlobalScopedRoots,
    kinds: &mut Vue3GlobalDeclarationKinds,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<()> {
    let name = reserve_vue3_global_declaration_name(
        prefix,
        declaration.id.name.as_str(),
        namespace_budget,
    )?;
    if !kinds.enum_names.contains(&name) {
        if !namespace_budget.reserve(name.len().saturating_add(1)) {
            return None;
        }
        kinds.enum_names.insert(name.clone());
    }
    match kinds.enum_constness.get(&name) {
        Some(is_const) if *is_const != declaration.r#const => {
            if !namespace_budget.reserve(name.len().saturating_mul(2).saturating_add(2)) {
                return None;
            }
            kinds.conflicting_type_names.insert(name.clone());
            kinds.conflicting_value_names.insert(name.clone());
        }
        Some(_) => {}
        None => {
            if !namespace_budget.reserve(
                name.len()
                    .saturating_add(std::mem::size_of::<bool>())
                    .saturating_add(1),
            ) {
                return None;
            }
            kinds.enum_constness.insert(name.clone(), declaration.r#const);
        }
    }
    if declaration
        .body
        .members
        .first()
        .is_some_and(|member| member.initializer.is_none())
    {
        if kinds
            .enum_omitted_first_initializer_definitions
            .contains_key(&name)
        {
            if !namespace_budget.reserve(name.len().saturating_mul(2).saturating_add(2)) {
                return None;
            }
            kinds.conflicting_type_names.insert(name.clone());
            kinds.conflicting_value_names.insert(name.clone());
        } else {
            if !namespace_budget.reserve(
                name.len()
                    .saturating_add(scoped_roots.definition_key.len())
                    .saturating_add(2),
            ) {
                return None;
            }
            kinds.enum_omitted_first_initializer_definitions.insert(
                name.clone(),
                scoped_roots.definition_key.clone(),
            );
        }
    }
    if !kinds.enum_member_definitions.contains_key(&name) {
        if !namespace_budget.reserve(name.len().saturating_add(1)) {
            return None;
        }
        kinds
            .enum_member_definitions
            .insert(name.clone(), BTreeMap::new());
    }
    let members = kinds
        .enum_member_definitions
        .get_mut(&name)
        .expect("enum member map must exist after insertion");
    let mut duplicate = false;
    for member in &declaration.body.members {
        let Some(member_name) = vue3_enum_member_name(&member.id) else {
            duplicate = true;
            continue;
        };
        if members.contains_key(member_name) {
            duplicate = true;
            continue;
        }
        if !namespace_budget.reserve(
            member_name
                .len()
                .saturating_add(scoped_roots.definition_key.len())
                .saturating_add(2),
        ) {
            return None;
        }
        members.insert(
            member_name.to_string(),
            scoped_roots.definition_key.clone(),
        );
    }
    if duplicate {
        if !namespace_budget.reserve(name.len().saturating_mul(2).saturating_add(2)) {
            return None;
        }
        kinds.conflicting_type_names.insert(name.clone());
        kinds.conflicting_value_names.insert(name);
    }
    Some(())
}

fn insert_vue3_global_class_declaration_kind(
    source: &str,
    declaration: &oxc_ast::ast::Class<'_>,
    prefix: Option<&str>,
    scoped_roots: &Vue3GlobalScopedRoots,
    kinds: &mut Vue3GlobalDeclarationKinds,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<()> {
    let Some(id) = &declaration.id else {
        return Some(());
    };
    insert_vue3_global_mergeable_declaration_kind(
        source,
        &mut kinds.class_names,
        prefix,
        id.name.as_str(),
        declaration.type_parameters.as_deref(),
        false,
        scoped_roots,
        &mut kinds.mergeable_type_parameters,
        &mut kinds.conflicting_type_names,
        namespace_budget,
    )?;
    insert_vue3_global_class_member_signatures(
        source,
        declaration,
        prefix,
        scoped_roots,
        kinds,
        namespace_budget,
    )
}

fn insert_vue3_global_class_member_signatures(
    source: &str,
    declaration: &oxc_ast::ast::Class<'_>,
    prefix: Option<&str>,
    scoped_roots: &Vue3GlobalScopedRoots,
    kinds: &mut Vue3GlobalDeclarationKinds,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<()> {
    let Some(id) = &declaration.id else {
        return Some(());
    };
    let name = reserve_vue3_global_declaration_name(prefix, id.name.as_str(), namespace_budget)?;
    let parameter_work = declaration
        .type_parameters
        .iter()
        .flat_map(|parameters| &parameters.params)
        .fold(0usize, |work, parameter| {
            work
                .saturating_add(parameter.name.name.len())
                .saturating_add(1)
        });
    if !namespace_budget.reserve(parameter_work) {
        return None;
    }
    let parameter_names = declaration
        .type_parameters
        .iter()
        .flat_map(|parameters| &parameters.params)
        .map(|parameter| parameter.name.name.to_string())
        .collect::<BTreeSet<_>>();
    let mut properties = kinds
        .interface_property_signatures
        .remove(&name)
        .unwrap_or_default();
    let mut conflicting = kinds.conflicting_type_names.contains(&name);
    let mut class_members = BTreeMap::<
        Vue3GlobalInterfaceMemberKey,
        Vue3GlobalClassMemberSignature,
    >::new();
    for member in &declaration.body.body {
        if let ClassElement::TSIndexSignature(index) = member {
            if index.r#static {
                continue;
            }
            let Some((property_name, signature)) = vue3_global_index_signature(
                source,
                index,
                &parameter_names,
                prefix,
                scoped_roots,
                namespace_budget,
            )?
            else {
                conflicting = true;
                continue;
            };
            insert_vue3_global_class_member_signature(
                property_name,
                Vue3GlobalClassMemberSignature {
                    kind: Vue3GlobalClassMemberKind::Index,
                    signature,
                },
                &mut class_members,
                &mut conflicting,
                namespace_budget,
            )?;
            continue;
        }
        let (key, computed, optional, readonly, member_kind, ty) = match member {
            ClassElement::PropertyDefinition(property)
                if !property.r#static
                    && !matches!(
                        property.accessibility,
                        Some(
                            oxc_ast::ast::TSAccessibility::Private
                                | oxc_ast::ast::TSAccessibility::Protected
                        )
                    ) =>
            {
                (
                    &property.key,
                    property.computed,
                    property.optional,
                    property.readonly,
                    Vue3GlobalClassMemberKind::Property,
                    property
                        .type_annotation
                        .as_ref()
                        .map(|annotation| &annotation.type_annotation),
                )
            }
            ClassElement::AccessorProperty(property)
                if !property.r#static
                    && !matches!(
                        property.accessibility,
                        Some(
                            oxc_ast::ast::TSAccessibility::Private
                                | oxc_ast::ast::TSAccessibility::Protected
                        )
                    ) =>
            {
                (
                    &property.key,
                    property.computed,
                    false,
                    false,
                    Vue3GlobalClassMemberKind::Property,
                    property
                        .type_annotation
                        .as_ref()
                        .map(|annotation| &annotation.type_annotation),
                )
            }
            ClassElement::MethodDefinition(method)
                if !method.r#static
                    && method.kind != oxc_ast::ast::MethodDefinitionKind::Constructor
                    && !matches!(
                        method.accessibility,
                        Some(
                            oxc_ast::ast::TSAccessibility::Private
                                | oxc_ast::ast::TSAccessibility::Protected
                        )
                    ) => match method.kind {
                oxc_ast::ast::MethodDefinitionKind::Method => (
                    &method.key,
                    method.computed,
                    method.optional,
                    false,
                    Vue3GlobalClassMemberKind::Method,
                    None,
                ),
                oxc_ast::ast::MethodDefinitionKind::Get => (
                    &method.key,
                    method.computed,
                    false,
                    false,
                    Vue3GlobalClassMemberKind::Getter,
                    method
                        .value
                        .return_type
                        .as_ref()
                        .map(|annotation| &annotation.type_annotation),
                ),
                oxc_ast::ast::MethodDefinitionKind::Set => (
                    &method.key,
                    method.computed,
                    false,
                    false,
                    Vue3GlobalClassMemberKind::Setter,
                    method
                        .value
                        .params
                        .items
                        .first()
                        .and_then(|parameter| parameter.type_annotation.as_ref())
                        .map(|annotation| &annotation.type_annotation),
                ),
                oxc_ast::ast::MethodDefinitionKind::Constructor => unreachable!(),
            },
            _ => continue,
        };
        let Some(property_name) = vue3_global_interface_member_key(
            key,
            computed,
            prefix,
            scoped_roots,
            namespace_budget,
        )?
        else {
            if computed {
                conflicting = true;
            }
            continue;
        };
        let type_source = match ty {
            Some(ty) => {
                let span = ty.span();
                Some(source.get(span.start as usize..span.end as usize)?)
            }
            None => None,
        };
        let scope_key = match ty {
            Some(ty) => vue3_type_scope_key(
                ty,
                &parameter_names,
                scoped_roots,
                prefix,
                namespace_budget,
            )?,
            None => None,
        };
        let signature_work = type_source
            .map_or(0, str::len)
            .saturating_add(scope_key.as_ref().map_or(0, String::len))
            .saturating_add(std::mem::size_of::<Vue3GlobalInterfacePropertySignature>());
        if !namespace_budget.reserve(
            property_name
                .work()
                .saturating_add(signature_work)
                .saturating_add(1),
        ) {
            return None;
        }
        let signature = Vue3GlobalInterfacePropertySignature {
            source: type_source.map(str::to_string),
            optional,
            readonly,
            method: member_kind == Vue3GlobalClassMemberKind::Method,
            scope_key,
        };
        let member = Vue3GlobalClassMemberSignature {
            kind: member_kind,
            signature,
        };
        insert_vue3_global_class_member_signature(
            property_name,
            member,
            &mut class_members,
            &mut conflicting,
            namespace_budget,
        )?;
    }
    for (property_name, member) in class_members {
        if !namespace_budget.reserve(
            property_name
                .work()
                .saturating_add(member.signature.work())
                .saturating_add(1),
        ) {
            return None;
        }
        merge_vue3_global_member_signature(
            property_name,
            member.signature,
            &mut properties,
            &mut conflicting,
            namespace_budget,
        )?;
    }
    if conflicting {
        kinds.conflicting_type_names.insert(name.clone());
    }
    if !properties.is_empty() {
        kinds.interface_property_signatures.insert(name, properties);
    }
    Some(())
}

fn insert_vue3_global_class_member_signature(
    property_name: Vue3GlobalInterfaceMemberKey,
    member: Vue3GlobalClassMemberSignature,
    class_members: &mut BTreeMap<
        Vue3GlobalInterfaceMemberKey,
        Vue3GlobalClassMemberSignature,
    >,
    conflicting: &mut bool,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<()> {
    let member_kind = member.kind;
    match class_members.entry(property_name) {
        std::collections::btree_map::Entry::Vacant(entry) => {
            entry.insert(member);
        }
        std::collections::btree_map::Entry::Occupied(mut entry) => {
            match (entry.get().kind, member_kind) {
                (Vue3GlobalClassMemberKind::Method, Vue3GlobalClassMemberKind::Method) => {
                    if !namespace_budget.reserve(
                        entry
                            .key()
                            .work()
                            .saturating_add(entry.get().signature.work())
                            .saturating_add(member.signature.work()),
                    ) {
                        return None;
                    }
                    if !vue3_global_interface_property_signatures_are_compatible(
                        &entry.get().signature,
                        &member.signature,
                    ) {
                        *conflicting = true;
                    }
                }
                (Vue3GlobalClassMemberKind::Getter, Vue3GlobalClassMemberKind::Setter) => {}
                (Vue3GlobalClassMemberKind::Setter, Vue3GlobalClassMemberKind::Getter) => {
                    entry.insert(member);
                }
                _ => *conflicting = true,
            }
        }
    }
    Some(())
}

#[allow(clippy::too_many_arguments)]
fn insert_vue3_global_mergeable_declaration_kind(
    source: &str,
    names: &mut BTreeSet<String>,
    prefix: Option<&str>,
    name: &str,
    type_parameters: Option<&oxc_ast::ast::TSTypeParameterDeclaration<'_>>,
    allow_duplicate: bool,
    scoped_roots: &Vue3GlobalScopedRoots,
    signatures: &mut BTreeMap<String, Vue3GlobalTypeParameterSignature>,
    conflicts: &mut BTreeSet<String>,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<()> {
    let name = reserve_vue3_global_declaration_name(prefix, name, namespace_budget)?;
    let duplicate = names.contains(&name);
    let source = match type_parameters {
        Some(type_parameters) => {
            let span = type_parameters.span;
            Some(source.get(span.start as usize..span.end as usize)?)
        }
        None => None,
    };
    let scope_key = match type_parameters {
        Some(parameters) => vue3_type_parameters_scope_key(
            parameters,
            scoped_roots,
            prefix,
            namespace_budget,
        )?,
        None => None,
    };
    if !namespace_budget.reserve(
        source
            .map_or(0, str::len)
            .saturating_add(scope_key.as_ref().map_or(0, String::len))
            .saturating_add(std::mem::size_of::<Vue3GlobalTypeParameterSignature>()),
    ) {
        return None;
    }
    let signature = Vue3GlobalTypeParameterSignature {
        source: source.map(str::to_string),
        scope_key,
    };
    if duplicate && !allow_duplicate && !conflicts.contains(&name) {
        if !namespace_budget.reserve(
            name.len()
                .saturating_add(std::mem::size_of::<String>())
                .saturating_add(1),
        ) {
            return None;
        }
        conflicts.insert(name.clone());
    }
    if !conflicts.contains(&name) {
        if let Some(existing) = signatures.get(&name) {
            if !namespace_budget.reserve(
                existing
                    .work()
                    .saturating_add(signature.work())
                    .saturating_add(name.len()),
            ) {
                return None;
            }
            if !vue3_global_type_parameter_signatures_are_compatible(existing, &signature) {
                conflicts.insert(name.clone());
            }
        } else {
            signatures.insert(name.clone(), signature);
        }
    }
    names.insert(name);
    Some(())
}

fn vue3_type_parameters_scope_key(
    parameters: &oxc_ast::ast::TSTypeParameterDeclaration<'_>,
    scoped_roots: &Vue3GlobalScopedRoots,
    declaration_prefix: Option<&str>,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<Option<String>> {
    let parameter_work = parameters.params.iter().fold(0usize, |work, parameter| {
        work
            .saturating_add(parameter.name.name.len())
            .saturating_add(std::mem::size_of::<String>())
            .saturating_add(1)
    });
    if !namespace_budget.reserve(parameter_work) {
        return None;
    }
    let parameter_names = parameters
        .params
        .iter()
        .map(|parameter| parameter.name.name.to_string())
        .collect::<BTreeSet<_>>();
    let mut collector = Vue3TypeParameterScopeReferenceCollector {
        parameter_names: &parameter_names,
        scoped_roots,
        declaration_prefix,
        scope_keys: BTreeSet::new(),
        bound_type_names: Vec::new(),
        active_type_names: BTreeMap::new(),
        bound_value_names: Vec::new(),
        active_value_names: BTreeMap::new(),
        namespace_budget,
    };
    for parameter in &parameters.params {
        for ty in parameter
            .constraint
            .iter()
            .chain(parameter.default.iter())
        {
            oxc_ast_visit::Visit::visit_ts_type(&mut collector, ty);
        }
    }
    collector.scope_key()
}

fn vue3_type_scope_key(
    ty: &TSType<'_>,
    parameter_names: &BTreeSet<String>,
    scoped_roots: &Vue3GlobalScopedRoots,
    declaration_prefix: Option<&str>,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<Option<String>> {
    let mut collector = Vue3TypeParameterScopeReferenceCollector {
        parameter_names,
        scoped_roots,
        declaration_prefix,
        scope_keys: BTreeSet::new(),
        bound_type_names: Vec::new(),
        active_type_names: BTreeMap::new(),
        bound_value_names: Vec::new(),
        active_value_names: BTreeMap::new(),
        namespace_budget,
    };
    oxc_ast_visit::Visit::visit_ts_type(&mut collector, ty);
    collector.scope_key()
}

struct Vue3TypeParameterScopeReferenceCollector<'roots, 'budget> {
    parameter_names: &'roots BTreeSet<String>,
    scoped_roots: &'roots Vue3GlobalScopedRoots<'roots>,
    declaration_prefix: Option<&'roots str>,
    scope_keys: BTreeSet<String>,
    bound_type_names: Vec<BTreeSet<String>>,
    active_type_names: BTreeMap<String, usize>,
    bound_value_names: Vec<BTreeSet<String>>,
    active_value_names: BTreeMap<String, usize>,
    namespace_budget: &'budget mut Vue3NamespaceProjectionBudget,
}

impl Vue3TypeParameterScopeReferenceCollector<'_, '_> {
    fn scope_key(self) -> Option<Option<String>> {
        if self.namespace_budget.is_exhausted() {
            return None;
        }
        if self.scope_keys.is_empty() {
            return Some(None);
        }
        let length = self
            .scope_keys
            .iter()
            .fold(0usize, |length, key| length.saturating_add(key.len()))
            .saturating_add(self.scope_keys.len().saturating_sub(1));
        if !self.namespace_budget.reserve(length.saturating_add(1)) {
            return None;
        }
        let mut scope_key = String::with_capacity(length);
        for key in self.scope_keys {
            if !scope_key.is_empty() {
                scope_key.push('|');
            }
            scope_key.push_str(&key);
        }
        Some(Some(scope_key))
    }

    fn push_scope(&mut self) {
        self.bound_type_names.push(BTreeSet::new());
        self.bound_value_names.push(BTreeSet::new());
    }

    fn pop_scope(&mut self) {
        let Some(names) = self.bound_type_names.pop() else {
            return;
        };
        for name in names {
            match self.active_type_names.get(&name).copied() {
                Some(1) => {
                    self.active_type_names.remove(&name);
                }
                Some(count) => {
                    self.active_type_names.insert(name, count - 1);
                }
                None => {}
            }
        }
        let Some(names) = self.bound_value_names.pop() else {
            return;
        };
        for name in names {
            match self.active_value_names.get(&name).copied() {
                Some(1) => {
                    self.active_value_names.remove(&name);
                }
                Some(count) => {
                    self.active_value_names.insert(name, count - 1);
                }
                None => {}
            }
        }
    }

    fn insert_binding(&mut self, name: &str) {
        if self.namespace_budget.is_exhausted() {
            return;
        }
        let Some(scope) = self.bound_type_names.last_mut() else {
            return;
        };
        if scope.contains(name) {
            return;
        }
        if !self.namespace_budget.reserve(
            name.len()
                .saturating_mul(2)
                .saturating_add(std::mem::size_of::<String>().saturating_mul(2)),
        ) {
            return;
        }
        if !scope.insert(name.to_string()) {
            return;
        }
        let count = self.active_type_names.entry(name.to_string()).or_default();
        *count = count.saturating_add(1);
    }

    fn insert_value_binding(&mut self, name: &str) {
        if self.namespace_budget.is_exhausted() {
            return;
        }
        let Some(scope) = self.bound_value_names.last_mut() else {
            return;
        };
        if scope.contains(name) {
            return;
        }
        if !self.namespace_budget.reserve(
            name.len()
                .saturating_mul(2)
                .saturating_add(std::mem::size_of::<String>().saturating_mul(2)),
        ) {
            return;
        }
        if !scope.insert(name.to_string()) {
            return;
        }
        let count = self.active_value_names.entry(name.to_string()).or_default();
        *count = count.saturating_add(1);
    }

    fn reference_is_global(&mut self, name: &str, value_space: bool) -> bool {
        if let Some(prefix) = self.declaration_prefix {
            let mut prefix = Some(prefix);
            while let Some(current) = prefix {
                let candidate_length = current.len().saturating_add(name.len()).saturating_add(1);
                if !self
                    .namespace_budget
                    .reserve(candidate_length.saturating_add(1))
                {
                    return false;
                }
                let candidate = format!("{current}.{name}");
                if self.scoped_roots.global_names.contains(&candidate)
                    || vue3_global_declaration_reference_target_exists(
                        self.scoped_roots.base_kinds,
                        &candidate,
                        value_space,
                    )
                {
                    return true;
                }
                prefix = current.rsplit_once('.').map(|(parent, _)| parent);
            }
        }
        let root = name.split('.').next().unwrap_or(name);
        if self.scoped_roots.global_names.contains(name)
            || self.scoped_roots.global_root_names.contains(root)
        {
            return true;
        }
        if self.scoped_roots.identity(root).is_some() {
            return false;
        }
        vue3_global_declaration_reference_target_exists(
            self.scoped_roots.base_kinds,
            name,
            value_space,
        )
    }

    fn insert_scope_identity(&mut self, identity: &str) {
        if self.scope_keys.contains(identity) {
            return;
        }
        if !self.namespace_budget.reserve(
            identity
                .len()
                .saturating_add(std::mem::size_of::<String>())
                .saturating_add(1),
        ) {
            return;
        }
        self.scope_keys.insert(identity.to_string());
    }

    fn is_binding_scope(kind: &oxc_ast::ast_kind::AstKind<'_>) -> bool {
        matches!(
            kind,
            oxc_ast::ast_kind::AstKind::TSCallSignatureDeclaration(_)
                | oxc_ast::ast_kind::AstKind::TSMethodSignature(_)
                | oxc_ast::ast_kind::AstKind::TSConstructSignatureDeclaration(_)
                | oxc_ast::ast_kind::AstKind::TSFunctionType(_)
                | oxc_ast::ast_kind::AstKind::TSConstructorType(_)
        )
    }
}

impl<'a> oxc_ast_visit::Visit<'a> for Vue3TypeParameterScopeReferenceCollector<'_, '_> {
    fn enter_node(&mut self, kind: oxc_ast::ast_kind::AstKind<'a>) {
        if Self::is_binding_scope(&kind) {
            self.push_scope();
        }
    }

    fn leave_node(&mut self, kind: oxc_ast::ast_kind::AstKind<'a>) {
        if Self::is_binding_scope(&kind) {
            self.pop_scope();
        }
    }

    fn visit_binding_identifier(&mut self, identifier: &oxc_ast::ast::BindingIdentifier<'a>) {
        self.insert_value_binding(identifier.name.as_str());
    }

    fn visit_ts_type_parameter_declaration(
        &mut self,
        declaration: &oxc_ast::ast::TSTypeParameterDeclaration<'a>,
    ) {
        for parameter in &declaration.params {
            self.insert_binding(parameter.name.name.as_str());
        }
        for parameter in &declaration.params {
            if let Some(constraint) = &parameter.constraint {
                oxc_ast_visit::Visit::visit_ts_type(self, constraint);
            }
            if let Some(default) = &parameter.default {
                oxc_ast_visit::Visit::visit_ts_type(self, default);
            }
        }
    }

    fn visit_ts_mapped_type(&mut self, mapped: &oxc_ast::ast::TSMappedType<'a>) {
        self.push_scope();
        self.insert_binding(mapped.key.name.as_str());
        oxc_ast_visit::Visit::visit_ts_type(self, &mapped.constraint);
        if let Some(name_type) = &mapped.name_type {
            oxc_ast_visit::Visit::visit_ts_type(self, name_type);
        }
        if let Some(annotation) = &mapped.type_annotation {
            oxc_ast_visit::Visit::visit_ts_type(self, annotation);
        }
        self.pop_scope();
    }

    fn visit_ts_conditional_type(&mut self, conditional: &oxc_ast::ast::TSConditionalType<'a>) {
        oxc_ast_visit::Visit::visit_ts_type(self, &conditional.check_type);
        let Some(infer_names) = vue3_infer_type_names_with_budget(
            &conditional.extends_type,
            self.namespace_budget,
        ) else {
            return;
        };
        self.push_scope();
        for name in infer_names {
            self.insert_binding(&name);
        }
        oxc_ast_visit::Visit::visit_ts_type(self, &conditional.extends_type);
        oxc_ast_visit::Visit::visit_ts_type(self, &conditional.true_type);
        self.pop_scope();
        oxc_ast_visit::Visit::visit_ts_type(self, &conditional.false_type);
    }

    fn visit_ts_type_reference(&mut self, reference: &TSTypeReference<'a>) {
        if let Some(name) = vue3_ts_type_name_key(&reference.type_name) {
            let root = name.split('.').next().unwrap_or(name.as_str());
            if !self.parameter_names.contains(root)
                && !self.active_type_names.contains_key(root)
                && !self.reference_is_global(&name, false)
            {
                if let Some(identity) = self.scoped_roots.identity(root) {
                    if !self.scope_keys.contains(identity)
                        && self.namespace_budget.reserve(
                            identity
                                .len()
                                .saturating_add(std::mem::size_of::<String>())
                                .saturating_add(1),
                        )
                    {
                        self.scope_keys.insert(identity.to_string());
                    }
                }
            }
        }
        oxc_ast_visit::walk::walk_ts_type_reference(self, reference);
    }

    fn visit_ts_type_query(&mut self, query: &TSTypeQuery<'a>) {
        if let Some(name) = vue3_type_query_name_key(query) {
            let root = name.split('.').next().unwrap_or(name.as_str());
            if !self.active_value_names.contains_key(root)
                && !self.reference_is_global(&name, true)
            {
                if let Some(identity) = self.scoped_roots.identity(root) {
                    if !self.scope_keys.contains(identity)
                        && self.namespace_budget.reserve(
                            identity
                                .len()
                                .saturating_add(std::mem::size_of::<String>())
                                .saturating_add(1),
                        )
                    {
                        self.scope_keys.insert(identity.to_string());
                    }
                }
            }
        }
        oxc_ast_visit::walk::walk_ts_type_query(self, query);
    }

    fn visit_ts_import_type(&mut self, import: &TSImportType<'a>) {
        let source = import.source.value.as_str();
        if !self.namespace_budget.reserve(
            self.scoped_roots
                .definition_key
                .len()
                .saturating_add(source.len())
                .saturating_mul(2)
                .saturating_add(32),
        ) {
            return;
        }
        let importer = Path::new(&self.scoped_roots.definition_key);
        let identity = if source.starts_with('.') {
            let directory = importer.parent().unwrap_or_else(|| Path::new(""));
            normalize_path_string(&normalize_path_components(directory.join(source)))
        } else {
            format!("{}:{source}", self.scoped_roots.definition_key)
        };
        self.insert_scope_identity(&format!("import:{identity}"));
        oxc_ast_visit::walk::walk_ts_import_type(self, import);
    }
}

fn collect_vue3_global_declaration_kinds_from_namespace(
    source: &str,
    declaration: &TSModuleDeclaration<'_>,
    prefix: Option<&str>,
    ambient: bool,
    depth: usize,
    scoped_roots: &Vue3GlobalScopedRoots,
    kinds: &mut Vue3GlobalDeclarationKinds,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<()> {
    let Some(name) = vue3_ts_module_declaration_name(declaration) else {
        return Some(());
    };
    let qualified = reserve_vue3_global_declaration_name(prefix, &name, namespace_budget)?;
    if !kinds.namespace_names.contains(&qualified) {
        if !namespace_budget.reserve(qualified.len().saturating_add(1)) {
            return None;
        }
        kinds.namespace_names.insert(qualified.clone());
    }
    let Some(body) = declaration.body.as_ref() else {
        return Some(());
    };
    match body {
        TSModuleDeclarationBody::TSModuleBlock(block) => {
            collect_vue3_global_declaration_kinds_from_statements(
                source,
                &block.body,
                Some(&qualified),
                ambient,
                ambient,
                depth,
                scoped_roots,
                kinds,
                namespace_budget,
            )
        }
        TSModuleDeclarationBody::TSModuleDeclaration(nested) => {
            collect_vue3_global_declaration_kinds_from_namespace(
                source,
                nested,
                Some(&qualified),
                ambient || nested.declare,
                depth.saturating_add(1),
                scoped_roots,
                kinds,
                namespace_budget,
            )
        }
    }
}

fn insert_vue3_global_type_alias_declaration_kind(
    prefix: Option<&str>,
    name: &str,
    kinds: &mut Vue3GlobalDeclarationKinds,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<()> {
    let name = reserve_vue3_global_declaration_name(prefix, name, namespace_budget)?;
    if kinds.type_alias_names.contains(&name) {
        if !namespace_budget.reserve(std::mem::size_of::<String>()) {
            return None;
        }
        kinds.conflicting_type_names.insert(name);
    } else {
        kinds.type_alias_names.insert(name);
    }
    Some(())
}

fn reserve_vue3_global_declaration_name(
    prefix: Option<&str>,
    name: &str,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<String> {
    let length = prefix.map_or(name.len(), |prefix| {
        prefix.len().saturating_add(name.len()).saturating_add(1)
    });
    if !namespace_budget.reserve(length.saturating_add(1)) {
        return None;
    }
    Some(match prefix {
        Some(prefix) => format!("{prefix}.{name}"),
        None => name.to_string(),
    })
}

fn refresh_vue3_cross_file_global_declarations(
    source: &str,
    statements: &[Statement<'_>],
    base_context: &Vue27TypeContext,
    base_kinds: &Vue3GlobalDeclarationKinds,
    kinds: &Vue3GlobalDeclarationKinds,
    analysis: &mut Vue3ScriptSetupAnalysis,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<()> {
    let is_ambient = vue3_statements_are_ambient_global_scope(statements);
    let statement_groups = vue3_global_declaration_statement_groups_with_budget(
        statements,
        is_ambient,
        namespace_budget,
    )?;
    let refreshable_count = statement_groups.iter().fold(0usize, |count, group| {
        count.saturating_add(group.len())
    });
    if !namespace_budget.reserve(
        refreshable_count
            .saturating_add(1)
            .saturating_mul(refreshable_count.saturating_add(1)),
    ) {
        return None;
    }

    let empty = BTreeSet::new();
    refresh_vue3_declared_type_declarations_from_statement_groups_excluding_interfaces(
        source,
        &statement_groups,
        &empty,
        analysis,
    );

    let signature_comparison_work = kinds
        .interface_names
        .iter()
        .chain(&kinds.class_names)
        .fold(0usize, |work, name| {
            work.saturating_add(vue3_global_type_parameter_comparison_work(
                base_kinds,
                kinds,
                name,
            ))
        })
        .saturating_add(kinds.enum_names.iter().fold(0usize, |work, name| {
            work.saturating_add(vue3_global_enum_comparison_work(
                base_kinds,
                kinds,
                name,
            ))
        }));
    if !namespace_budget.reserve(signature_comparison_work) {
        return None;
    }
    let merge_names = kinds
        .interface_names
        .iter()
        .chain(&kinds.enum_names)
        .chain(&kinds.class_names)
        .filter(|name| {
            base_kinds.declaration_counts.get(*name).copied().unwrap_or(0) > 1
                && vue3_global_declaration_kinds_can_merge_with_file(base_kinds, kinds, name)
                && vue3_global_mergeable_type_parameters_are_compatible(
                    base_kinds, kinds, name,
                )
        })
        .cloned()
        .collect::<BTreeSet<_>>();
    if merge_names.is_empty() {
        return Some(());
    }

    let mut local_projection = Vue3ScriptSetupAnalysis::default();
    let mut merged_projection = Vue3ScriptSetupAnalysis::default();
    for name in &merge_names {
        let work = vue3_type_alias_projection_work(analysis, name, name);
        if !namespace_budget.reserve(work) {
            return None;
        }
        sync_vue3_type_alias_from_analysis(&mut local_projection, analysis, name, name);
        let merged = merge_vue3_global_named_analysis_and_context_projection(
            analysis,
            base_context,
            kinds,
            base_kinds,
            name,
            namespace_budget,
        )?;
        sync_vue3_type_alias_from_analysis(&mut merged_projection, &merged, name, name);
        sync_vue3_type_alias_from_analysis(analysis, &merged, name, name);
    }

    let excluded_interfaces = merge_names
        .iter()
        .filter(|name| kinds.interface_names.contains(*name))
        .cloned()
        .collect::<BTreeSet<_>>();
    refresh_vue3_declared_type_declarations_from_statement_groups_excluding_interfaces(
        source,
        &statement_groups,
        &excluded_interfaces,
        analysis,
    );
    converge_vue3_global_namespaces_with_pinned_declarations(
        source,
        &statement_groups,
        &merge_names,
        &merged_projection,
        &excluded_interfaces,
        analysis,
        namespace_budget,
    )?;
    if !namespace_budget.reserve(vue3_local_generic_scope_capture_work(analysis)) {
        return None;
    }
    finalize_vue3_local_generic_alias_scopes(analysis);
    for name in &merge_names {
        let work = vue3_type_alias_projection_work(&local_projection, name, name);
        if !namespace_budget.reserve(work) {
            return None;
        }
        sync_vue3_type_alias_from_analysis(analysis, &local_projection, name, name);
    }
    Some(())
}

fn converge_vue3_global_namespaces_with_pinned_declarations(
    source: &str,
    statement_groups: &[&[Statement<'_>]],
    pinned_names: &BTreeSet<String>,
    pinned_projection: &Vue3ScriptSetupAnalysis,
    excluded_interfaces: &BTreeSet<String>,
    analysis: &mut Vue3ScriptSetupAnalysis,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<()> {
    for statements in statement_groups {
        if !validate_vue3_namespace_structure(statements, 0, namespace_budget) {
            return None;
        }
        if !seed_vue3_namespace_public_type_names(statements, true, analysis, namespace_budget) {
            return None;
        }
    }
    let limit = statement_groups
        .iter()
        .fold(1usize, |steps, statements| {
            steps
                .saturating_add(count_vue3_namespace_projection_steps(statements))
                .saturating_add(count_vue3_refreshable_type_declarations_in_statements(
                    statements,
                ))
        });
    for _ in 0..limit {
        let statement_count = statement_groups.iter().fold(1usize, |count, statements| {
            count.saturating_add(statements.len())
        });
        if !namespace_budget.reserve(statement_count.saturating_mul(statement_count)) {
            return None;
        }
        let mut changed = project_vue3_namespace_groups_from_statement_groups_once(
            source,
            statement_groups,
            true,
            0,
            analysis,
            namespace_budget,
        );
        if namespace_budget.is_exhausted() {
            return None;
        }
        for name in pinned_names {
            changed |= sync_vue3_type_alias_from_analysis(
                analysis,
                pinned_projection,
                name,
                name,
            );
        }
        changed |= refresh_vue3_declared_type_declarations_from_statement_groups_excluding_interfaces(
            source,
            statement_groups,
            excluded_interfaces,
            analysis,
        );
        changed |= collect_vue3_declared_type_deps_from_statement_groups_excluding_names(
            statement_groups,
            pinned_names,
            analysis,
        );
        if analysis.type_dependency_work_exhausted {
            namespace_budget.exhausted = true;
            return None;
        }
        if !changed {
            return Some(());
        }
    }
    // Re-projecting a pinned namespace intentionally toggles its raw and merged
    // views, so `changed` need not become false. The declaration-count limit is
    // the same finite graph bound used by the ordinary namespace fixed point.
    Some(())
}

fn vue3_global_declaration_kinds_can_merge(
    left: &Vue3GlobalDeclarationKinds,
    right: &Vue3GlobalDeclarationKinds,
    name: &str,
) -> bool {
    if vue3_global_declaration_kinds_is_blocked(left, name)
        || vue3_global_declaration_kinds_is_blocked(right, name)
    {
        return false;
    }
    vue3_global_declaration_flags_can_merge(
        left.type_alias_names.contains(name),
        left.interface_names.contains(name),
        left.enum_names.contains(name),
        left.class_names.contains(name),
        right.type_alias_names.contains(name),
        right.interface_names.contains(name),
        right.enum_names.contains(name),
        right.class_names.contains(name),
    ) && vue3_global_enum_members_are_compatible(left, right, name)
}

fn vue3_global_declaration_kinds_is_blocked(
    kinds: &Vue3GlobalDeclarationKinds,
    name: &str,
) -> bool {
    kinds.conflicting_type_names.contains(name) || kinds.blocked_type_names.contains(name)
}

fn vue3_global_declaration_kinds_have_type(
    kinds: &Vue3GlobalDeclarationKinds,
    name: &str,
) -> bool {
    kinds.interface_names.contains(name)
        || kinds.enum_names.contains(name)
        || kinds.class_names.contains(name)
        || kinds.type_alias_names.contains(name)
}

fn vue3_global_cross_space_declaration_names(
    base: &Vue3GlobalDeclarationKinds,
    file: &Vue3GlobalDeclarationKinds,
) -> BTreeSet<String> {
    file.value_names
        .iter()
        .filter(|name| {
            base.type_alias_names.contains(*name)
                || base.enum_names.contains(*name)
                || base.class_names.contains(*name)
        })
        .chain(
            file.type_alias_names
                .iter()
                .chain(&file.enum_names)
                .chain(&file.class_names)
                .filter(|name| base.value_names.contains(*name)),
        )
        .cloned()
        .collect()
}

fn vue3_global_declaration_kinds_can_merge_with_file(
    base: &Vue3GlobalDeclarationKinds,
    file: &Vue3GlobalDeclarationKinds,
    name: &str,
) -> bool {
    if vue3_global_declaration_kinds_is_blocked(base, name)
        || vue3_global_declaration_kinds_is_blocked(file, name)
    {
        return false;
    }
    let base_class_is_only_this_file = file.class_names.contains(name)
        && base.class_counts.get(name).copied().unwrap_or(0) == 1;
    vue3_global_declaration_flags_can_merge(
        base.type_alias_names.contains(name),
        base.interface_names.contains(name),
        base.enum_names.contains(name),
        base.class_names.contains(name) && !base_class_is_only_this_file,
        file.type_alias_names.contains(name),
        file.interface_names.contains(name),
        file.enum_names.contains(name),
        file.class_names.contains(name),
    ) && vue3_global_enum_members_are_compatible(base, file, name)
}

fn vue3_global_enum_members_are_compatible(
    left: &Vue3GlobalDeclarationKinds,
    right: &Vue3GlobalDeclarationKinds,
    name: &str,
) -> bool {
    if matches!(
        (left.enum_constness.get(name), right.enum_constness.get(name)),
        (Some(left), Some(right)) if left != right
    ) {
        return false;
    }
    if matches!(
        (
            left.enum_omitted_first_initializer_definitions.get(name),
            right.enum_omitted_first_initializer_definitions.get(name),
        ),
        (Some(left), Some(right)) if left != right
    ) {
        return false;
    }
    let (Some(left), Some(right)) = (
        left.enum_member_definitions.get(name),
        right.enum_member_definitions.get(name),
    ) else {
        return true;
    };
    left.iter().all(|(member, left_definition)| {
        right
            .get(member)
            .is_none_or(|right_definition| right_definition == left_definition)
    })
}

fn vue3_global_enum_comparison_work(
    left: &Vue3GlobalDeclarationKinds,
    right: &Vue3GlobalDeclarationKinds,
    name: &str,
) -> usize {
    let constness_work = usize::from(left.enum_constness.contains_key(name))
        .saturating_add(usize::from(right.enum_constness.contains_key(name)))
        .saturating_mul(std::mem::size_of::<bool>());
    let initializer_work = left
        .enum_omitted_first_initializer_definitions
        .get(name)
        .map_or(0, String::len)
        .saturating_add(
            right
                .enum_omitted_first_initializer_definitions
                .get(name)
                .map_or(0, String::len),
        );
    let member_work = left
        .enum_member_definitions
        .get(name)
        .into_iter()
        .chain(right.enum_member_definitions.get(name))
        .flat_map(BTreeMap::iter)
        .fold(0usize, |work, (member, definition)| {
            work
                .saturating_add(member.len())
                .saturating_add(definition.len())
                .saturating_add(2)
        });
    name.len()
        .saturating_add(constness_work)
        .saturating_add(initializer_work)
        .saturating_add(member_work)
        .saturating_add(1)
}

fn vue3_global_declaration_flags_can_merge(
    left_type_alias: bool,
    left_interface: bool,
    left_enum: bool,
    left_class: bool,
    right_type_alias: bool,
    right_interface: bool,
    right_enum: bool,
    right_class: bool,
) -> bool {
    if left_type_alias || right_type_alias {
        return false;
    }
    let merges_enums = left_enum
        && right_enum
        && !left_interface
        && !right_interface
        && !left_class
        && !right_class;
    let merges_interfaces = left_interface
        && right_interface
        && !left_enum
        && !right_enum
        && !(left_class && right_class);
    let merges_class_and_interface = !left_enum
        && !right_enum
        && (left_interface || right_interface)
        && (left_class || right_class)
        && !(left_class && right_class);
    merges_enums || merges_interfaces || merges_class_and_interface
}

fn vue3_global_mergeable_type_parameters_are_compatible(
    left: &Vue3GlobalDeclarationKinds,
    right: &Vue3GlobalDeclarationKinds,
    name: &str,
) -> bool {
    let left_is_mergeable = left.interface_names.contains(name) || left.class_names.contains(name);
    let right_is_mergeable =
        right.interface_names.contains(name) || right.class_names.contains(name);
    if !left_is_mergeable || !right_is_mergeable {
        return true;
    }
    match (
        left.mergeable_type_parameters.get(name),
        right.mergeable_type_parameters.get(name),
    ) {
        (Some(left), Some(right)) => {
            vue3_global_type_parameter_signatures_are_compatible(left, right)
        }
        _ => false,
    }
}

fn vue3_global_type_parameter_comparison_work(
    left: &Vue3GlobalDeclarationKinds,
    right: &Vue3GlobalDeclarationKinds,
    name: &str,
) -> usize {
    left.mergeable_type_parameters
        .get(name)
        .map_or(0, Vue3GlobalTypeParameterSignature::work)
        .saturating_add(
            right
                .mergeable_type_parameters
                .get(name)
                .map_or(0, Vue3GlobalTypeParameterSignature::work),
        )
        .saturating_add(name.len())
}

fn vue3_global_interface_property_comparison_work(
    left: &Vue3GlobalDeclarationKinds,
    right: &Vue3GlobalDeclarationKinds,
    name: &str,
) -> usize {
    let Some(left) = left.interface_property_signatures.get(name) else {
        return 0;
    };
    let Some(right) = right.interface_property_signatures.get(name) else {
        return 0;
    };
    left.iter().fold(name.len(), |work, (property, signature)| {
        let work = work.saturating_add(property.work()).saturating_add(1);
        right.get(property).map_or(work, |other| {
            work
                .saturating_add(signature.work())
                .saturating_add(other.work())
                .saturating_add(1)
        })
    })
}

fn vue3_global_interface_properties_are_compatible(
    left: &Vue3GlobalDeclarationKinds,
    right: &Vue3GlobalDeclarationKinds,
    name: &str,
) -> bool {
    let Some(left) = left.interface_property_signatures.get(name) else {
        return true;
    };
    let Some(right) = right.interface_property_signatures.get(name) else {
        return true;
    };
    left.iter().all(|(property, signature)| {
        right.get(property).is_none_or(|other| {
            !matches!(property, Vue3GlobalInterfaceMemberKey::Index(_))
                && vue3_global_interface_property_signatures_are_compatible(signature, other)
        })
    })
}

fn vue3_global_function_value_projection_comparison_work(
    left: &Vue3GlobalDeclarationKinds,
    right: &Vue3GlobalDeclarationKinds,
    name: &str,
) -> usize {
    left.value_type_projections
        .get(name)
        .map_or(0, Vue3ValueTypeProjection::work)
        .saturating_add(
            right
                .value_type_projections
                .get(name)
                .map_or(0, Vue3ValueTypeProjection::work),
        )
        .saturating_add(name.len())
}

fn vue3_global_function_value_projections_are_compatible(
    left: &Vue3GlobalDeclarationKinds,
    right: &Vue3GlobalDeclarationKinds,
    name: &str,
) -> bool {
    left.value_type_projections.get(name) == right.value_type_projections.get(name)
}

fn merge_vue3_global_named_analysis_and_context_projection(
    analysis: &Vue3ScriptSetupAnalysis,
    context: &Vue27TypeContext,
    analysis_kinds: &Vue3GlobalDeclarationKinds,
    context_kinds: &Vue3GlobalDeclarationKinds,
    name: &str,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<Vue3ScriptSetupAnalysis> {
    let context_work = vue3_external_type_alias_projection_work(context, name, name.len(), "");
    let analysis_work = vue3_type_alias_projection_work(analysis, name, name);
    if !namespace_budget.reserve(
        context_work
            .saturating_add(analysis_work)
            .saturating_mul(2),
    ) {
        return None;
    }
    let mut context_analysis = Vue3ScriptSetupAnalysis::default();
    sync_vue3_type_alias_from_context(&mut context_analysis, context, name, name);
    let mut local_analysis = Vue3ScriptSetupAnalysis::default();
    sync_vue3_type_alias_from_analysis(&mut local_analysis, analysis, name, name);
    let context_projection = vue3_global_namespace_block_projection(
        context_analysis,
        context_kinds,
        name,
    );
    let local_projection =
        vue3_global_namespace_block_projection(local_analysis, analysis_kinds, name);
    Some(merge_vue3_namespace_declaration_projections(
        &[&context_projection, &local_projection],
        name,
    ))
}

fn vue3_global_namespace_block_projection(
    analysis: Vue3ScriptSetupAnalysis,
    kinds: &Vue3GlobalDeclarationKinds,
    name: &str,
) -> Vue3NamespaceBlockProjection {
    Vue3NamespaceBlockProjection {
        interface_names: kinds
            .interface_names
            .contains(name)
            .then(|| name.to_string())
            .into_iter()
            .collect(),
        enum_names: kinds
            .enum_names
            .contains(name)
            .then(|| name.to_string())
            .into_iter()
            .collect(),
        class_names: kinds
            .class_names
            .contains(name)
            .then(|| name.to_string())
            .into_iter()
            .collect(),
        analysis,
    }
}

#[derive(Default)]
struct Vue3GlobalNamedDependencies {
    type_source: Option<String>,
    type_direct_deps: Vec<String>,
    type_deps: BTreeSet<String>,
}

impl Vue3ValueTypeProjection {
    fn from_analysis(analysis: &Vue3ScriptSetupAnalysis, name: &str) -> Self {
        Self {
            type_query_declared_types: analysis.type_query_declared_types.get(name).cloned(),
            define_model_type_query_declared_types: analysis
                .define_model_type_query_declared_types
                .get(name)
                .cloned(),
            keyof_type_query_declared_types: analysis
                .keyof_type_query_declared_types
                .get(name)
                .cloned(),
            return_type_runtime_type_declarations: analysis
                .return_type_runtime_type_declarations
                .get(name)
                .cloned(),
            define_model_return_type_runtime_type_declarations: analysis
                .define_model_return_type_runtime_type_declarations
                .get(name)
                .cloned(),
            props_options_type_declarations: analysis
                .props_options_type_declarations
                .get(name)
                .cloned(),
            return_type_props_options_declarations: analysis
                .return_type_props_options_declarations
                .get(name)
                .cloned(),
            unresolved_import_sources: analysis.unresolved_import_sources.get(name).cloned(),
        }
    }

    fn work(&self) -> usize {
        self.type_query_declared_types
            .as_deref()
            .map_or(0, vue3_external_string_vec_cost)
            .saturating_add(
                self.define_model_type_query_declared_types
                    .as_deref()
                    .map_or(0, vue3_external_string_vec_cost),
            )
            .saturating_add(
                self.keyof_type_query_declared_types
                    .as_deref()
                    .map_or(0, vue3_external_string_vec_cost),
            )
            .saturating_add(
                self.return_type_runtime_type_declarations
                    .as_deref()
                    .map_or(0, vue3_external_string_vec_cost),
            )
            .saturating_add(
                self.define_model_return_type_runtime_type_declarations
                    .as_deref()
                    .map_or(0, vue3_external_string_vec_cost),
            )
            .saturating_add(
                self.props_options_type_declarations
                    .as_ref()
                    .map_or(0, vue3_external_type_members_cache_cost),
            )
            .saturating_add(
                self.return_type_props_options_declarations
                    .as_ref()
                    .map_or(0, vue3_external_type_members_cache_cost),
            )
            .saturating_add(
                self.unresolved_import_sources
                    .as_ref()
                    .map_or(0, String::len),
            )
            .saturating_add(std::mem::size_of::<Self>())
    }

    fn apply_to_analysis(&self, analysis: &mut Vue3ScriptSetupAnalysis, name: &str) {
        macro_rules! sync_entry {
            ($field:ident) => {
                match &self.$field {
                    Some(value) => {
                        analysis.$field.insert(name.to_string(), value.clone());
                    }
                    None => {
                        analysis.$field.remove(name);
                    }
                }
            };
        }

        sync_entry!(type_query_declared_types);
        sync_entry!(define_model_type_query_declared_types);
        sync_entry!(keyof_type_query_declared_types);
        sync_entry!(return_type_runtime_type_declarations);
        sync_entry!(define_model_return_type_runtime_type_declarations);
        sync_entry!(props_options_type_declarations);
        sync_entry!(return_type_props_options_declarations);
        sync_entry!(unresolved_import_sources);
    }

    fn apply(&self, context: &mut Vue27TypeContext, name: &str) {
        macro_rules! sync_entry {
            ($field:ident) => {
                match &self.$field {
                    Some(value) => {
                        context.$field.insert(name.to_string(), value.clone());
                    }
                    None => {
                        context.$field.remove(name);
                    }
                }
            };
        }

        sync_entry!(type_query_declared_types);
        sync_entry!(define_model_type_query_declared_types);
        sync_entry!(keyof_type_query_declared_types);
        sync_entry!(return_type_runtime_type_declarations);
        sync_entry!(define_model_return_type_runtime_type_declarations);
        sync_entry!(props_options_type_declarations);
        sync_entry!(return_type_props_options_declarations);
        sync_entry!(unresolved_import_sources);
    }

    fn merge_present(&self, context: &mut Vue27TypeContext, name: &str) {
        macro_rules! merge_entry {
            ($field:ident) => {
                if let Some(value) = &self.$field {
                    context.$field.insert(name.to_string(), value.clone());
                }
            };
        }

        merge_entry!(type_query_declared_types);
        merge_entry!(define_model_type_query_declared_types);
        merge_entry!(keyof_type_query_declared_types);
        merge_entry!(return_type_runtime_type_declarations);
        merge_entry!(define_model_return_type_runtime_type_declarations);
        merge_entry!(props_options_type_declarations);
        merge_entry!(return_type_props_options_declarations);
        merge_entry!(unresolved_import_sources);
    }
}

pub(crate) fn capture_vue3_value_type_projection(
    analysis: &mut Vue3ScriptSetupAnalysis,
    name: &str,
) {
    let projection = Vue3ValueTypeProjection::from_analysis(analysis, name);
    analysis
        .value_type_projections
        .insert(name.to_string(), projection);
}

impl Vue3GlobalNamedDependencies {
    fn apply(&self, context: &mut Vue27TypeContext, name: &str) {
        match &self.type_source {
            Some(source) => {
                context.type_sources.insert(name.to_string(), source.clone());
            }
            None => {
                context.type_sources.remove(name);
            }
        }
        if self.type_direct_deps.is_empty() {
            context.type_direct_deps.remove(name);
        } else {
            context
                .type_direct_deps
                .insert(name.to_string(), self.type_direct_deps.clone());
        }
        if self.type_deps.is_empty() {
            context.type_deps.remove(name);
        } else {
            context
                .type_deps
                .insert(name.to_string(), self.type_deps.clone());
        }
    }
}

fn vue3_global_named_dependency_projection_work(
    context: &Vue27TypeContext,
    name: &str,
) -> usize {
    context
        .type_sources
        .get(name)
        .map_or(0, String::len)
        .saturating_add(
            context
                .type_direct_deps
                .get(name)
                .map_or(0, |dependencies| {
                    dependencies.iter().fold(0usize, |work, dependency| {
                        work.saturating_add(dependency.len()).saturating_add(1)
                    })
                }),
        )
        .saturating_add(context.type_deps.get(name).map_or(0, |dependencies| {
            dependencies.iter().fold(0usize, |work, dependency| {
                work.saturating_add(dependency.len()).saturating_add(1)
            })
        }))
}

fn vue3_global_blocked_declaration_spaces(
    target: &Vue3GlobalDeclarationKinds,
    source: &Vue3GlobalDeclarationKinds,
    type_roots: &BTreeSet<String>,
    value_roots: &BTreeSet<String>,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<(BTreeSet<String>, BTreeSet<String>)> {
    let graph_work = [
        &target.type_declaration_type_references,
        &source.type_declaration_type_references,
        &target.type_declaration_value_references,
        &source.type_declaration_value_references,
        &target.value_declaration_type_references,
        &source.value_declaration_type_references,
        &target.value_declaration_value_references,
        &source.value_declaration_value_references,
    ]
        .into_iter()
        .flat_map(|references| references.iter())
        .fold(0usize, |work, (name, references)| {
            references.iter().fold(
                work.saturating_add(name.len()).saturating_add(1),
                |work, reference| {
                    work
                        .saturating_add(name.len())
                        .saturating_add(reference.len())
                        .saturating_add(2)
                },
            )
        });
    let root_work = type_roots
        .iter()
        .chain(value_roots)
        .fold(0usize, |work, name| {
            work.saturating_add(name.len()).saturating_add(1)
        });
    let dual_space_work = target
        .dual_space_names
        .iter()
        .chain(&source.dual_space_names)
        .chain(&target.class_names)
        .chain(&source.class_names)
        .chain(&target.enum_names)
        .chain(&source.enum_names)
        .fold(0usize, |work, name| {
            work.saturating_add(name.len()).saturating_add(1)
        });
    if !namespace_budget.reserve(
        graph_work
            .saturating_mul(6)
            .saturating_add(root_work.saturating_mul(4))
            .saturating_add(dual_space_work.saturating_mul(4)),
    ) {
        return None;
    }
    macro_rules! build_reverse_map {
        ($field:ident) => {{
            let mut reverse = BTreeMap::<String, BTreeSet<String>>::new();
            for (name, references) in target.$field.iter().chain(&source.$field) {
                for reference in references {
                    reverse
                        .entry(reference.clone())
                        .or_default()
                        .insert(name.clone());
                }
            }
            reverse
        }};
    }
    let reverse_type_to_types = build_reverse_map!(type_declaration_type_references);
    let reverse_value_to_types = build_reverse_map!(type_declaration_value_references);
    let reverse_type_to_values = build_reverse_map!(value_declaration_type_references);
    let reverse_value_to_values = build_reverse_map!(value_declaration_value_references);
    let mut blocked_types = type_roots.clone();
    let mut blocked_values = value_roots.clone();
    let mut pending = type_roots
        .iter()
        .cloned()
        .map(|name| (name, true))
        .chain(value_roots.iter().cloned().map(|name| (name, false)))
        .collect::<Vec<_>>();
    let mut processed_types = BTreeSet::new();
    let mut processed_values = BTreeSet::new();
    while let Some((name, type_space)) = pending.pop() {
        let processed = if type_space {
            &mut processed_types
        } else {
            &mut processed_values
        };
        if !processed.insert(name.clone()) {
            continue;
        }
        let is_dual_space = target.dual_space_names.contains(&name)
            || source.dual_space_names.contains(&name)
            || target.class_names.contains(&name)
            || source.class_names.contains(&name)
            || target.enum_names.contains(&name)
            || source.enum_names.contains(&name);
        if is_dual_space {
            if type_space {
                if blocked_values.insert(name.clone()) {
                    pending.push((name.clone(), false));
                }
            } else if blocked_types.insert(name.clone()) {
                pending.push((name.clone(), true));
            }
        }
        let (type_dependents, value_dependents) = if type_space {
            (
                reverse_type_to_types.get(&name),
                reverse_type_to_values.get(&name),
            )
        } else {
            (
                reverse_value_to_types.get(&name),
                reverse_value_to_values.get(&name),
            )
        };
        if let Some(dependents) = type_dependents {
            for dependent in dependents {
                if blocked_types.insert(dependent.clone()) {
                    pending.push((dependent.clone(), true));
                }
            }
        }
        if let Some(dependents) = value_dependents {
            for dependent in dependents {
                if blocked_values.insert(dependent.clone()) {
                    pending.push((dependent.clone(), false));
                }
            }
        }
    }
    Some((blocked_types, blocked_values))
}

fn vue3_merge_global_named_dependencies(
    target: &Vue27TypeContext,
    source: &Vue27TypeContext,
    name: &str,
) -> Vue3GlobalNamedDependencies {
    let type_source = target
        .type_sources
        .get(name)
        .into_iter()
        .chain(source.type_sources.get(name))
        .min()
        .cloned();
    let type_direct_deps = target
        .type_direct_deps
        .get(name)
        .into_iter()
        .chain(source.type_direct_deps.get(name))
        .flatten()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut type_deps = target
        .type_deps
        .get(name)
        .into_iter()
        .chain(source.type_deps.get(name))
        .flatten()
        .cloned()
        .collect::<BTreeSet<_>>();
    type_deps.extend(
        target
            .type_sources
            .get(name)
            .into_iter()
            .chain(source.type_sources.get(name))
            .cloned(),
    );
    Vue3GlobalNamedDependencies {
        type_source,
        type_direct_deps,
        type_deps,
    }
}

fn merge_vue3_global_type_file_projection(
    target: &mut Vue27TypeContext,
    target_kinds: &mut Vue3GlobalDeclarationKinds,
    source: Vue3GlobalTypeFileProjection,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> bool {
    let Vue3GlobalTypeFileProjection {
        context: source_context,
        kinds: source_kinds,
    } = source;
    let merge_work = vue3_external_type_context_cache_cost(&source_context)
        .saturating_add(source_kinds.work());
    if !namespace_budget.reserve(merge_work) {
        return false;
    }
    let planning_name_work = target_kinds
        .declaration_counts
        .keys()
        .chain(source_kinds.declaration_counts.keys())
        .fold(0usize, |work, name| {
            work.saturating_add(name.len()).saturating_add(1)
        })
        .saturating_mul(16);
    if !namespace_budget.reserve(planning_name_work) {
        return false;
    }

    let source_type_names = source_kinds
        .interface_names
        .iter()
        .chain(&source_kinds.enum_names)
        .chain(&source_kinds.class_names)
        .chain(&source_kinds.type_alias_names)
        .cloned()
        .collect::<BTreeSet<_>>();
    let source_names = vue3_type_context_names(&source_context);
    let signature_comparison_work = source_type_names.iter().fold(0usize, |work, name| {
        if vue3_global_declaration_kinds_have_type(target_kinds, name) {
            work
                .saturating_add(vue3_global_type_parameter_comparison_work(
                    target_kinds,
                    &source_kinds,
                    name,
                ))
                .saturating_add(vue3_global_interface_property_comparison_work(
                    target_kinds,
                    &source_kinds,
                    name,
                ))
                .saturating_add(
                    vue3_global_enum_comparison_work(target_kinds, &source_kinds, name)
                        .saturating_mul(2),
                )
        } else {
            work
        }
    });
    let function_comparison_work = source_kinds
        .function_value_names
        .iter()
        .filter(|name| target_kinds.function_value_names.contains(*name))
        .fold(0usize, |work, name| {
            work.saturating_add(vue3_global_function_value_projection_comparison_work(
                target_kinds,
                &source_kinds,
                name,
            ))
        });
    if !namespace_budget.reserve(signature_comparison_work) {
        return false;
    }
    if !namespace_budget.reserve(function_comparison_work) {
        return false;
    }
    let incompatible_type_parameter_names = source_type_names
        .iter()
        .filter(|name| {
            vue3_global_declaration_kinds_have_type(target_kinds, name)
                && !vue3_global_mergeable_type_parameters_are_compatible(
                    target_kinds,
                    &source_kinds,
                    name,
                )
        })
        .cloned()
        .collect::<BTreeSet<_>>();
    let incompatible_interface_property_names = source_type_names
        .iter()
        .filter(|name| {
            !vue3_global_interface_properties_are_compatible(
                target_kinds,
                &source_kinds,
                name,
            )
        })
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut root_type_conflict_names = target_kinds.conflicting_type_names.clone();
    root_type_conflict_names.extend(source_kinds.conflicting_type_names.iter().cloned());
    root_type_conflict_names.extend(incompatible_interface_property_names.iter().cloned());
    root_type_conflict_names.extend(
        source_kinds
            .value_names
            .iter()
            .filter(|name| {
                target_kinds.type_alias_names.contains(*name)
                    || target_kinds.enum_names.contains(*name)
                    || target_kinds.class_names.contains(*name)
            })
            .cloned(),
    );
    root_type_conflict_names.extend(
        target_kinds
            .callable_interface_names
            .iter()
            .chain(&source_kinds.callable_interface_names)
            .filter(|name| {
                target_kinds.value_names.contains(*name)
                    || source_kinds.value_names.contains(*name)
            })
            .cloned(),
    );
    for name in &source_type_names {
        if target_kinds.conflicting_type_names.contains(name)
            || (target_kinds.value_names.contains(name)
                && (source_kinds.type_alias_names.contains(name)
                    || source_kinds.enum_names.contains(name)
                    || source_kinds.class_names.contains(name)))
            || (!target_kinds.blocked_type_names.contains(name)
                && vue3_global_declaration_kinds_have_type(target_kinds, name)
                && (!vue3_global_declaration_kinds_can_merge(
                    target_kinds,
                    &source_kinds,
                    name,
                ) || incompatible_type_parameter_names.contains(name)))
        {
            root_type_conflict_names.insert(name.clone());
        }
    }
    let mut root_value_conflict_names = target_kinds.conflicting_value_names.clone();
    root_value_conflict_names.extend(source_kinds.conflicting_value_names.iter().cloned());
    root_value_conflict_names.extend(
        source_kinds
            .variable_value_names
            .iter()
            .filter(|name| target_kinds.value_names.contains(*name))
            .cloned(),
    );
    root_value_conflict_names.extend(
        source_kinds
            .value_names
            .iter()
            .filter(|name| {
                target_kinds.class_names.contains(*name)
                    || target_kinds.enum_names.contains(*name)
            })
            .cloned(),
    );
    root_value_conflict_names.extend(
        target_kinds
            .value_names
            .iter()
            .filter(|name| {
                source_kinds.class_names.contains(*name)
                    || source_kinds.enum_names.contains(*name)
            })
            .cloned(),
    );
    root_value_conflict_names.extend(
        target_kinds
            .variable_value_names
            .iter()
            .filter(|name| source_kinds.value_names.contains(*name))
            .cloned(),
    );
    root_value_conflict_names.extend(
        source_kinds
            .function_value_names
            .iter()
            .filter(|name| {
                target_kinds.function_value_names.contains(*name)
                    && !vue3_global_function_value_projections_are_compatible(
                        target_kinds,
                        &source_kinds,
                        name,
                    )
            })
            .cloned(),
    );
    root_value_conflict_names.extend(root_type_conflict_names.iter().filter(|name| {
        target_kinds.class_names.contains(*name)
            || target_kinds.enum_names.contains(*name)
            || source_kinds.class_names.contains(*name)
            || source_kinds.enum_names.contains(*name)
    }).cloned());
    let mut blocked_type_roots = root_type_conflict_names.clone();
    blocked_type_roots.extend(target_kinds.blocked_type_names.iter().cloned());
    blocked_type_roots.extend(source_kinds.blocked_type_names.iter().cloned());
    let mut blocked_value_roots = root_value_conflict_names.clone();
    blocked_value_roots.extend(target_kinds.blocked_value_names.iter().cloned());
    blocked_value_roots.extend(source_kinds.blocked_value_names.iter().cloned());
    let Some((mut blocked_type_names, mut blocked_value_names)) =
        vue3_global_blocked_declaration_spaces(
        target_kinds,
        &source_kinds,
        &blocked_type_roots,
        &blocked_value_roots,
        namespace_budget,
    ) else {
        return false;
    };
    blocked_type_names.retain(|name| {
        target_kinds.declaration_counts.contains_key(name)
            || source_kinds.declaration_counts.contains_key(name)
    });
    blocked_value_names.retain(|name| {
        target_kinds.declaration_counts.contains_key(name)
            || source_kinds.declaration_counts.contains_key(name)
    });

    let merge_names = source_kinds
        .interface_names
        .iter()
        .chain(&source_kinds.enum_names)
        .chain(&source_kinds.class_names)
        .filter(|name| {
            !blocked_type_names.contains(*name)
                && !vue3_global_declaration_kinds_is_blocked(target_kinds, name)
                && vue3_type_context_has_name(target, name)
                && vue3_global_declaration_kinds_can_merge(target_kinds, &source_kinds, name)
                && !incompatible_type_parameter_names.contains(*name)
        })
        .cloned()
        .collect::<BTreeSet<_>>();

    let value_projection_names = source_kinds
        .value_names
        .iter()
        .cloned()
        .chain(
            blocked_type_names
                .iter()
                .filter(|name| {
                    !blocked_value_names.contains(*name)
                        && target_kinds.value_names.contains(*name)
                })
                .cloned(),
        )
        .collect::<BTreeSet<_>>();
    let dependency_names = source_names
        .iter()
        .filter(|name| {
            vue3_type_context_has_name(target, name)
                || target_kinds.conflicting_type_names.contains(*name)
        })
        .cloned()
        .chain(
            source_kinds
                .declaration_counts
                .keys()
                .filter(|name| target_kinds.declaration_counts.contains_key(*name))
                .cloned(),
        )
        .chain(blocked_type_names.iter().cloned())
        .chain(blocked_value_names.iter().cloned())
        .collect::<BTreeSet<_>>();
    let named_merge_work = merge_names.iter().fold(0usize, |work, name| {
        let target_work = vue3_external_type_alias_projection_work(target, name, name.len(), "");
        let source_work =
            vue3_external_type_alias_projection_work(&source_context, name, name.len(), "");
        work.saturating_add(
            target_work
                .saturating_add(source_work)
                .saturating_mul(2),
        )
    });
    let conflict_names = blocked_type_names
        .iter()
        .chain(&blocked_value_names)
        .cloned()
        .collect::<BTreeSet<_>>();
    let conflict_work = conflict_names.iter().fold(0usize, |work, name| {
        work.saturating_add(
            vue3_external_type_alias_projection_work(target, name, name.len(), "")
                .saturating_add(vue3_external_type_alias_projection_work(
                    &source_context,
                    name,
                    name.len(),
                    "",
                )),
        )
    });
    let value_sync_work = value_projection_names.iter().fold(0usize, |work, name| {
        let projection_work = if source_kinds.value_names.contains(name) {
            source_kinds
                .value_type_projections
                .get(name)
                .map_or(std::mem::size_of::<Vue3ValueTypeProjection>(), |projection| {
                    projection.work()
                })
        } else {
            target_kinds
                .value_type_projections
                .get(name)
                .map_or(std::mem::size_of::<Vue3ValueTypeProjection>(), |projection| {
                    projection.work()
                })
        };
        work
            .saturating_add(name.len().saturating_mul(9))
            .saturating_add(projection_work.saturating_mul(2))
    });
    let dependency_work = dependency_names.iter().fold(0usize, |work, name| {
        let input_work = vue3_global_named_dependency_projection_work(target, name)
            .saturating_add(vue3_global_named_dependency_projection_work(
                &source_context,
                name,
        ));
        work
            .saturating_add(name.len().saturating_mul(4))
            .saturating_add(input_work.saturating_mul(2))
    });
    if !namespace_budget.reserve(
        named_merge_work
            .saturating_add(conflict_work)
            .saturating_add(value_sync_work)
            .saturating_add(dependency_work),
    ) {
        return false;
    }
    let dependency_projections = dependency_names
        .iter()
        .map(|name| {
            (
                name.clone(),
                vue3_merge_global_named_dependencies(target, &source_context, name),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let value_projections = value_projection_names
        .iter()
        .map(|name| {
            let projection = if source_kinds.value_names.contains(name) {
                source_kinds.value_type_projections.get(name)
            } else {
                target_kinds.value_type_projections.get(name)
            };
            (
                name.clone(),
                projection.cloned().unwrap_or_default(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    for name in merge_names {
        let mut target_analysis = Vue3ScriptSetupAnalysis::default();
        sync_vue3_type_alias_from_context(&mut target_analysis, target, &name, &name);
        let mut source_analysis = Vue3ScriptSetupAnalysis::default();
        sync_vue3_type_alias_from_context(
            &mut source_analysis,
            &source_context,
            &name,
            &name,
        );
        let target_projection = vue3_global_namespace_block_projection(
            target_analysis,
            target_kinds,
            &name,
        );
        let source_projection =
            vue3_global_namespace_block_projection(source_analysis, &source_kinds, &name);
        let merged = merge_vue3_namespace_declaration_projections(
            &[&target_projection, &source_projection],
            &name,
        );
        sync_vue3_type_alias_to_context(target, &merged, &name, &name);
    }
    merge_vue3_type_context_missing(target, source_context);
    for (name, projection) in &value_projections {
        if blocked_type_names.contains(name)
            || blocked_value_names.contains(name)
            || !source_kinds.value_names.contains(name)
        {
            continue;
        }
        let preserves_compatible_type_space = target_kinds.interface_names.contains(name)
            || source_kinds.interface_names.contains(name);
        if preserves_compatible_type_space {
            projection.merge_present(target, name);
        } else {
            projection.apply(target, name);
        }
    }
    for (name, projection) in &dependency_projections {
        projection.apply(target, name);
    }
    for name in &blocked_type_names {
        clear_vue3_conflicting_global_type_projection(target, name);
        if !blocked_value_names.contains(name) {
            if let Some(projection) = value_projections.get(name) {
                projection.apply(target, name);
            }
        }
        target.silent_unresolved_type_names.insert(name.clone());
    }
    for name in blocked_value_names.difference(&blocked_type_names) {
        clear_vue3_conflicting_global_value_projection(target, name);
    }
    target_kinds.extend(&source_kinds);
    target_kinds
        .conflicting_type_names
        .extend(root_type_conflict_names.iter().cloned());
    target_kinds.blocked_type_names.extend(
        blocked_type_names
            .into_iter()
            .filter(|name| !root_type_conflict_names.contains(name)),
    );
    target_kinds
        .conflicting_value_names
        .extend(root_value_conflict_names.iter().cloned());
    target_kinds.blocked_value_names.extend(
        blocked_value_names
            .into_iter()
            .filter(|name| !root_value_conflict_names.contains(name)),
    );
    true
}

fn clear_vue3_conflicting_global_type_projection(
    context: &mut Vue27TypeContext,
    name: &str,
) {
    macro_rules! remove_entry {
        ($field:ident) => {
            context.$field.remove(name);
        };
    }

    remove_entry!(declared_types);
    remove_entry!(define_model_declared_types);
    remove_entry!(props_type_declarations);
    remove_entry!(keyof_runtime_type_declarations);
    remove_entry!(tuple_runtime_type_declarations);
    remove_entry!(define_model_tuple_runtime_type_declarations);
    remove_entry!(array_element_runtime_type_declarations);
    remove_entry!(define_model_array_element_runtime_type_declarations);
    remove_entry!(parameter_tuple_runtime_type_declarations);
    remove_entry!(define_model_parameter_tuple_runtime_type_declarations);
    remove_entry!(constructor_parameter_tuple_runtime_type_declarations);
    remove_entry!(define_model_constructor_parameter_tuple_runtime_type_declarations);
    remove_entry!(generic_type_aliases);
    remove_entry!(string_literal_type_declarations);
    remove_entry!(ordered_string_literal_type_declarations);
    remove_entry!(emits_type_declarations);
    remove_entry!(type_query_declared_types);
    remove_entry!(define_model_type_query_declared_types);
    remove_entry!(keyof_type_query_declared_types);
    remove_entry!(return_type_runtime_type_declarations);
    remove_entry!(define_model_return_type_runtime_type_declarations);
    remove_entry!(props_options_type_declarations);
    remove_entry!(return_type_props_options_declarations);
    remove_entry!(unresolved_import_sources);
}

fn clear_vue3_conflicting_global_value_projection(
    context: &mut Vue27TypeContext,
    name: &str,
) {
    macro_rules! remove_entry {
        ($field:ident) => {
            context.$field.remove(name);
        };
    }

    remove_entry!(type_query_declared_types);
    remove_entry!(define_model_type_query_declared_types);
    remove_entry!(keyof_type_query_declared_types);
    remove_entry!(return_type_runtime_type_declarations);
    remove_entry!(define_model_return_type_runtime_type_declarations);
    remove_entry!(props_options_type_declarations);
    remove_entry!(return_type_props_options_declarations);
    remove_entry!(unresolved_import_sources);
}

#[cfg(test)]
mod vue3_global_merge_budget_tests {
    use super::*;

    fn budget(limit: usize) -> Vue3NamespaceProjectionBudget {
        Vue3NamespaceProjectionBudget {
            remaining_work: limit,
            exhausted: false,
        }
    }

    fn alias_projection(runtime: &str, dependency: &str) -> Vue3GlobalTypeFileProjection {
        let name = "Shared".to_string();
        let dependency = dependency.to_string();
        let mut context = Vue27TypeContext::default();
        context
            .declared_types
            .insert(name.clone(), vec![runtime.to_string()]);
        context
            .type_sources
            .insert(name.clone(), dependency.clone());
        context
            .type_direct_deps
            .insert(name.clone(), vec![dependency.clone()]);
        context
            .type_deps
            .insert(name.clone(), BTreeSet::from([dependency]));
        let mut kinds = Vue3GlobalDeclarationKinds::default();
        kinds.type_alias_names.insert(name);
        kinds.finish_file_scan();
        Vue3GlobalTypeFileProjection { context, kinds }
    }

    fn alias_target() -> (Vue27TypeContext, Vue3GlobalDeclarationKinds) {
        let projection = alias_projection("String", "target.d.ts");
        (projection.context, projection.kinds)
    }

    fn interface_heritage_conflict_target(
    ) -> (Vue27TypeContext, Vue3GlobalDeclarationKinds) {
        let name = "Conflict".to_string();
        let mut context = Vue27TypeContext::default();
        context.props_type_declarations.insert(
            name.clone(),
            Vue27TypeMembers {
                source: "interface Conflict extends Left, Right {}".to_string(),
                members: Vec::new(),
                errors: Vec::new(),
                interface_heritage: Some(Vue3InterfaceHeritageEvidence {
                    own_members: BTreeMap::new(),
                    inherited_members: BTreeMap::from([(
                        "value".to_string(),
                        BTreeSet::from([
                            Vue3InterfaceHeritageMemberEvidence {
                                exact_primitive_types: Some(BTreeSet::from([
                                    "number".to_string(),
                                ])),
                                required: Some(true),
                            },
                            Vue3InterfaceHeritageMemberEvidence {
                                exact_primitive_types: Some(BTreeSet::from([
                                    "string".to_string(),
                                ])),
                                required: Some(true),
                            },
                        ]),
                    )]),
                }),
            },
        );
        context
            .type_sources
            .insert(name.clone(), "global.d.ts".to_string());
        let mut kinds = Vue3GlobalDeclarationKinds::default();
        kinds.interface_names.insert(name);
        kinds.finish_file_scan();
        (context, kinds)
    }

    fn captured_generic_alias(
        environment: Vue3GenericTypeEnvironment,
    ) -> Vue3GenericTypeAlias {
        Vue3GenericTypeAlias {
            source: "type Box<T> = T".to_string(),
            kind: Vue3GenericTypeAliasKind::TypeAlias,
            params: vec!["T".to_string()],
            scope: Vue3GenericTypeScope::Captured(std::sync::Arc::new(environment)),
            interface_fragments: Vec::new(),
        }
    }

    fn leading_work(source: &Vue3GlobalTypeFileProjection) -> usize {
        vue3_external_type_context_cache_cost(&source.context)
            .saturating_add(source.kinds.work())
    }

    #[test]
    fn global_merge_budget_is_exact_and_failure_atomic() {
        let source = alias_projection("Number", "source.d.ts");
        let leading = leading_work(&source);
        let (mut target, mut kinds) = alias_target();
        let mut measured = budget(usize::MAX);
        assert!(merge_vue3_global_type_file_projection(
            &mut target,
            &mut kinds,
            source,
            &mut measured,
        ));
        let required = usize::MAX - measured.remaining_work;
        assert!(required > leading);

        let (mut target, mut kinds) = alias_target();
        let mut exact = budget(required);
        assert!(merge_vue3_global_type_file_projection(
            &mut target,
            &mut kinds,
            alias_projection("Number", "source.d.ts"),
            &mut exact,
        ));
        assert_eq!(exact.remaining_work, 0);
        assert!(!exact.exhausted);

        let (mut target, mut kinds) = alias_target();
        let target_snapshot = target.clone();
        let kinds_snapshot = kinds.clone();
        let mut short = budget(required - 1);
        assert!(!merge_vue3_global_type_file_projection(
            &mut target,
            &mut kinds,
            alias_projection("Number", "source.d.ts"),
            &mut short,
        ));
        assert_eq!(target, target_snapshot);
        assert_eq!(kinds, kinds_snapshot);
        assert_eq!(short.remaining_work, 0);
        assert!(short.exhausted);
    }

    #[test]
    fn global_merge_budget_is_shared_across_atomic_merges() {
        let (mut measured_target, mut measured_kinds) = alias_target();
        let mut measured = budget(usize::MAX);
        let before_first = measured.remaining_work;
        assert!(merge_vue3_global_type_file_projection(
            &mut measured_target,
            &mut measured_kinds,
            alias_projection("Number", "source-a.d.ts"),
            &mut measured,
        ));
        let first_work = before_first - measured.remaining_work;
        let before_second = measured.remaining_work;
        let second_source = alias_projection("Boolean", "source-b.d.ts");
        let second_leading = leading_work(&second_source);
        assert!(merge_vue3_global_type_file_projection(
            &mut measured_target,
            &mut measured_kinds,
            second_source,
            &mut measured,
        ));
        let second_work = before_second - measured.remaining_work;
        assert!(second_work > second_leading);

        let (mut target, mut kinds) = alias_target();
        let mut shared = budget(first_work.saturating_add(second_work).saturating_sub(1));
        assert!(merge_vue3_global_type_file_projection(
            &mut target,
            &mut kinds,
            alias_projection("Number", "source-a.d.ts"),
            &mut shared,
        ));
        let target_snapshot = target.clone();
        let kinds_snapshot = kinds.clone();
        assert!(!merge_vue3_global_type_file_projection(
            &mut target,
            &mut kinds,
            alias_projection("Boolean", "source-b.d.ts"),
            &mut shared,
        ));
        assert_eq!(target, target_snapshot);
        assert_eq!(kinds, kinds_snapshot);
        assert_eq!(shared.remaining_work, 0);
        assert!(shared.exhausted);
    }

    #[test]
    fn interface_heritage_conflict_budget_is_exact_and_failure_atomic() {
        let (mut measured_context, mut measured_kinds) =
            interface_heritage_conflict_target();
        let mut measured = budget(usize::MAX);
        assert!(apply_vue3_global_interface_heritage_conflicts(
            &mut measured_context,
            &mut measured_kinds,
            &mut measured,
        ));
        let required = usize::MAX - measured.remaining_work;
        assert!(required > 1);
        assert!(!vue3_type_context_has_name(&measured_context, "Conflict"));
        assert!(measured_context
            .silent_unresolved_type_names
            .contains("Conflict"));

        let (mut exact_context, mut exact_kinds) = interface_heritage_conflict_target();
        let mut exact = budget(required);
        assert!(apply_vue3_global_interface_heritage_conflicts(
            &mut exact_context,
            &mut exact_kinds,
            &mut exact,
        ));
        assert_eq!(exact.remaining_work, 0);
        assert!(!exact.exhausted);

        let (mut short_context, mut short_kinds) = interface_heritage_conflict_target();
        let context_snapshot = short_context.clone();
        let kinds_snapshot = short_kinds.clone();
        let mut short = budget(required - 1);
        assert!(!apply_vue3_global_interface_heritage_conflicts(
            &mut short_context,
            &mut short_kinds,
            &mut short,
        ));
        assert_eq!(short_context, context_snapshot);
        assert_eq!(short_kinds, kinds_snapshot);
        assert_eq!(short.remaining_work, 0);
        assert!(short.exhausted);
    }

    #[test]
    fn generic_global_stability_compares_captured_environment_semantics() {
        let environment = Vue3GenericTypeEnvironment {
            definition_filename: Some("global.d.ts".to_string()),
            declared_types: BTreeMap::from([(
                "Leaf".to_string(),
                vec!["String".to_string()],
            )]),
            ..Vue3GenericTypeEnvironment::default()
        };
        let mut left = Vue27TypeContext::default();
        left.generic_type_aliases
            .insert("Box".to_string(), captured_generic_alias(environment.clone()));
        let mut right = Vue27TypeContext::default();
        right
            .generic_type_aliases
            .insert("Box".to_string(), captured_generic_alias(environment));

        assert_ne!(left, right);
        assert!(vue3_global_type_context_stability_eq(&left, &right));

        let Vue3GenericTypeScope::Captured(environment) = &mut right
            .generic_type_aliases
            .get_mut("Box")
            .expect("right alias")
            .scope
        else {
            panic!("captured right alias");
        };
        std::sync::Arc::make_mut(environment)
            .declared_types
            .insert("Leaf".to_string(), vec!["Number".to_string()]);
        assert!(!vue3_global_type_context_stability_eq(&left, &right));

        {
            let Vue3GenericTypeScope::Captured(environment) = &mut right
                .generic_type_aliases
                .get_mut("Box")
                .expect("right alias")
                .scope
            else {
                panic!("captured right alias");
            };
            let environment = std::sync::Arc::make_mut(environment);
            environment.declared_types = BTreeMap::from([(
                "Leaf".to_string(),
                vec!["String".to_string()],
            )]);
            environment.definition_filename = Some("other.d.ts".to_string());
        }
        assert!(!vue3_global_type_context_stability_eq(&left, &right));

        let Vue3GenericTypeScope::Captured(environment) = &mut right
            .generic_type_aliases
            .get_mut("Box")
            .expect("right alias")
            .scope
        else {
            panic!("captured right alias");
        };
        let environment = std::sync::Arc::make_mut(environment);
        environment.definition_filename = Some("global.d.ts".to_string());
        environment.definition_resolution_mode = Vue3TypeResolutionMode::Require;
        assert!(!vue3_global_type_context_stability_eq(&left, &right));
    }

    #[test]
    fn generic_global_propagation_horizon_traverses_type_and_value_spaces() {
        let mut context = Vue27TypeContext::default();
        context.generic_type_aliases.insert(
            "Root".to_string(),
            Vue3GenericTypeAlias {
                source: "type Root<T> = ReturnType<typeof factory>".to_string(),
                kind: Vue3GenericTypeAliasKind::TypeAlias,
                params: vec!["T".to_string()],
                scope: Vue3GenericTypeScope::Local,
                interface_fragments: Vec::new(),
            },
        );
        let mut kinds = Vue3GlobalDeclarationKinds::default();
        kinds
            .type_declaration_value_references
            .insert("Root".to_string(), BTreeSet::from(["factory".to_string()]));
        kinds
            .value_declaration_type_references
            .insert("factory".to_string(), BTreeSet::from(["Middle".to_string()]));
        kinds.type_declaration_type_references.insert(
            "Middle".to_string(),
            BTreeSet::from(["Leaf".to_string()]),
        );
        kinds.type_declaration_type_references.insert(
            "Leaf".to_string(),
            BTreeSet::from(["Root".to_string()]),
        );
        let mut measured = budget(usize::MAX);
        assert_eq!(
            vue3_global_generic_propagation_horizon(&context, &kinds, &mut measured),
            Some(5),
        );
        let required = usize::MAX - measured.remaining_work;
        assert!(required > 0);

        let mut exact = budget(required);
        assert_eq!(
            vue3_global_generic_propagation_horizon(&context, &kinds, &mut exact),
            Some(5),
        );
        assert_eq!(exact.remaining_work, 0);
        assert!(!exact.exhausted);

        let mut short = budget(required - 1);
        assert_eq!(
            vue3_global_generic_propagation_horizon(&context, &kinds, &mut short),
            None,
        );
        assert_eq!(short.remaining_work, 0);
        assert!(short.exhausted);
    }

    #[test]
    fn reachable_global_augmentation_loading_is_budget_atomic() {
        let dir = tempfile::tempdir().expect("temp dir");
        let base = dir.path().join("base.d.ts");
        let augmentation = dir.path().join("augmentation.ts");
        let base_source = "interface Base { base: string }";
        let augmentation_source =
            "export {}; declare global { interface Augmented { value: number } }";
        std::fs::write(&base, base_source).expect("write base global type");
        std::fs::write(&augmentation, augmentation_source)
            .expect("write imported global augmentation");
        let filename = dir.path().join("Comp.vue").to_string_lossy().to_string();
        let files = [base.to_string_lossy().to_string()];
        let roots = [Vue3InlineModuleSource {
            filename: &filename,
            source: "import './augmentation'",
            source_type: oxc_span::SourceType::ts(),
        }];
        let total_bytes = base_source.len().saturating_add(augmentation_source.len());

        let exact_resolver = Vue3TypeResolverContext {
            external_type_session: Vue3ExternalTypeLoadSession::with_limits(
                Vue3ExternalTypeLoadLimits {
                    max_global_files: 2,
                    max_global_bytes: total_bytes,
                    ..Vue3ExternalTypeLoadLimits::default()
                },
            ),
            ..Vue3TypeResolverContext::default()
        };
        let exact = vue3_global_type_context_with_module_sources(
            &filename,
            &files,
            &roots,
            &exact_resolver,
        );
        assert!(vue3_type_context_has_name(&exact, "Base"));
        assert!(vue3_type_context_has_name(&exact, "Augmented"));
        let exact_stats = exact_resolver.external_type_session.stats();
        assert_eq!(exact_stats.global_files_read, 2);
        assert_eq!(exact_stats.global_bytes, total_bytes);
        assert_eq!(exact_stats.import_files_read, 0);

        for limits in [
            Vue3ExternalTypeLoadLimits {
                max_global_files: 1,
                max_global_bytes: total_bytes,
                ..Vue3ExternalTypeLoadLimits::default()
            },
            Vue3ExternalTypeLoadLimits {
                max_global_files: 2,
                max_global_bytes: total_bytes - 1,
                ..Vue3ExternalTypeLoadLimits::default()
            },
        ] {
            let resolver = Vue3TypeResolverContext {
                external_type_session: Vue3ExternalTypeLoadSession::with_limits(limits),
                ..Vue3TypeResolverContext::default()
            };
            let context = vue3_global_type_context_with_module_sources(
                &filename,
                &files,
                &roots,
                &resolver,
            );
            assert_eq!(context, Vue27TypeContext::default());
            assert_eq!(resolver.external_type_session.stats().import_files_read, 0);
        }
    }
}

#[cfg(test)]
fn collect_vue3_global_types_from_statements_with_budget(
    source: &str,
    statements: &[Statement<'_>],
    implicitly_ambient: bool,
    base_context: &Vue27TypeContext,
    analysis: &mut Vue3ScriptSetupAnalysis,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<(BTreeSet<String>, BTreeSet<String>)> {
    let program_is_global_script = vue3_statements_are_ambient_global_scope(statements);
    collect_vue3_global_types_from_statements_with_budget_and_kinds(
        source,
        statements,
        implicitly_ambient,
        program_is_global_script,
        base_context,
        &Vue3GlobalDeclarationKinds::default(),
        &Vue3GlobalDeclarationKinds::default(),
        analysis,
        namespace_budget,
    )
}

fn collect_vue3_global_types_from_statements_with_budget_and_kinds(
    source: &str,
    statements: &[Statement<'_>],
    implicitly_ambient: bool,
    program_is_global_script: bool,
    base_context: &Vue27TypeContext,
    base_kinds: &Vue3GlobalDeclarationKinds,
    kinds: &Vue3GlobalDeclarationKinds,
    analysis: &mut Vue3ScriptSetupAnalysis,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<(BTreeSet<String>, BTreeSet<String>)> {
    if !namespace_budget.reserve(vue3_type_analysis_clone_work(analysis)) {
        return None;
    }
    let mut working_analysis = analysis.clone();
    let names = collect_vue3_global_types_from_statements_inner(
        source,
        statements,
        implicitly_ambient,
        program_is_global_script,
        base_context,
        &mut working_analysis,
        namespace_budget,
    )?;
    refresh_vue3_cross_file_global_declarations(
        source,
        statements,
        base_context,
        base_kinds,
        kinds,
        &mut working_analysis,
        namespace_budget,
    )?;
    *analysis = working_analysis;
    Some(names)
}

fn collect_vue3_global_types_from_statements_inner(
    source: &str,
    statements: &[Statement<'_>],
    implicitly_ambient: bool,
    program_is_global_script: bool,
    base_context: &Vue27TypeContext,
    analysis: &mut Vue3ScriptSetupAnalysis,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<(BTreeSet<String>, BTreeSet<String>)> {
    let mut names = BTreeSet::new();
    let module_import_names =
        vue3_global_type_file_import_names_with_budget(statements, namespace_budget)?;
    let is_ambient = program_is_global_script;
    if is_ambient {
        for statement in statements {
            collect_vue3_predeclared_runtime_type_from_statement(statement, analysis);
        }
        for statement in statements {
            if !vue3_statement_has_deferred_type_scope(statement) {
                collect_vue3_ambient_global_type_from_statement(
                    source,
                    statement,
                    implicitly_ambient,
                    &mut names,
                    analysis,
                    namespace_budget,
                )?;
            }
        }
        refresh_vue3_declared_type_declarations_from_statements(source, statements, analysis);
        if !statements
            .iter()
            .any(vue3_statement_has_deferred_type_scope)
        {
            return Some((names, module_import_names));
        }
        collect_vue3_declared_type_deps_from_statements(statements, analysis);
        if analysis.type_dependency_work_exhausted {
            return None;
        }
        for statement in statements {
            if vue3_statement_has_deferred_type_scope(statement) {
                collect_vue3_ambient_global_type_from_statement(
                    source,
                    statement,
                    implicitly_ambient,
                    &mut names,
                    analysis,
                    namespace_budget,
                )?;
            }
        }
        let statement_groups = vue3_global_declaration_statement_groups_with_budget(
            statements,
            true,
            namespace_budget,
        )?;
        project_vue3_namespace_groups_from_statement_groups_with_budget(
            source,
            &statement_groups,
            implicitly_ambient,
            0,
            analysis,
            namespace_budget,
        );
        if namespace_budget.is_exhausted() {
            return None;
        }
        refresh_vue3_declared_type_declarations_from_statements(source, statements, analysis);
        return Some((names, module_import_names));
    }
    let statement_groups = vue3_global_declaration_statement_groups_with_budget(
        statements,
        false,
        namespace_budget,
    )?;
    let mut global_root_names = BTreeSet::new();
    for group in &statement_groups {
        let (group_names, group_roots) =
            vue3_module_lexical_type_names_with_budget(group, namespace_budget)?;
        names.extend(group_names);
        global_root_names.extend(group_roots);
    }
    let (module_names, module_root_names) =
        vue3_module_lexical_type_names_with_budget(statements, namespace_budget)?;
    let mut shadowed_roots = BTreeSet::new();
    for root in global_root_names {
        if module_root_names.contains(&root) || module_import_names.contains(&root) {
            if !namespace_budget.reserve(root.len().saturating_add(1)) {
                return None;
            }
            shadowed_roots.insert(root);
        }
    }

    remove_vue3_shadowed_base_type_projections(
        analysis,
        base_context,
        &module_root_names,
        &module_import_names,
        namespace_budget,
    )?;

    collect_vue3_declared_types_from_statements_with_namespace_budget(
        source,
        statements,
        implicitly_ambient,
        0,
        analysis,
        namespace_budget,
    );
    if namespace_budget.is_exhausted() || analysis.type_dependency_work_exhausted {
        return None;
    }

    let shadowed_scope_names = vue3_shadowed_scope_projection_names_with_budget(
        analysis,
        base_context,
        &names,
        &module_names,
        &shadowed_roots,
        namespace_budget,
    )?;
    let mut module_shadow_projection = Vue3ScriptSetupAnalysis::default();
    sync_vue3_scope_type_projections(
        &mut module_shadow_projection,
        analysis,
        &shadowed_scope_names,
        namespace_budget,
    )?;
    restore_vue3_global_base_type_projections(
        analysis,
        base_context,
        &shadowed_scope_names,
        namespace_budget,
    )?;

    for group in &statement_groups {
        collect_vue3_declared_types_from_statements_with_namespace_budget(
            source,
            group,
            true,
            0,
            analysis,
            namespace_budget,
        );
        if namespace_budget.is_exhausted() {
            return None;
        }
    }
    converge_vue3_namespace_groups_from_statement_groups_in_place_with_budget(
        source,
        &statement_groups,
        true,
        0,
        analysis,
        namespace_budget,
    )?;
    if namespace_budget.is_exhausted() {
        return None;
    }

    let mut global_shadow_projection = Vue3ScriptSetupAnalysis::default();
    sync_vue3_scope_type_projections(
        &mut global_shadow_projection,
        analysis,
        &shadowed_scope_names,
        namespace_budget,
    )?;
    let convergence_limit = module_names
        .len()
        .saturating_add(names.len())
        .saturating_add(1);
    let statement_count = statement_groups
        .iter()
        .fold(statements.len().saturating_add(1), |count, group| {
            count.saturating_add(group.len())
        });
    let iteration_work = statement_count.saturating_mul(statement_count);
    let mut converged = false;
    for _ in 0..convergence_limit {
        if !namespace_budget.reserve(iteration_work) {
            return None;
        }
        sync_vue3_scope_type_projections(
            analysis,
            &module_shadow_projection,
            &shadowed_scope_names,
            namespace_budget,
        )?;
        let mut changed =
            converge_vue3_namespace_groups_from_statement_groups_in_place_with_budget(
            source,
            &[statements],
            implicitly_ambient,
            0,
            analysis,
            namespace_budget,
        )?;
        if namespace_budget.is_exhausted() || analysis.type_dependency_work_exhausted {
            return None;
        }
        sync_vue3_scope_type_projections(
            &mut module_shadow_projection,
            analysis,
            &shadowed_scope_names,
            namespace_budget,
        )?;

        sync_vue3_scope_type_projections(
            analysis,
            &global_shadow_projection,
            &shadowed_scope_names,
            namespace_budget,
        )?;
        changed |= converge_vue3_namespace_groups_from_statement_groups_in_place_with_budget(
            source,
            &statement_groups,
            true,
            0,
            analysis,
            namespace_budget,
        )?;
        if namespace_budget.is_exhausted() || analysis.type_dependency_work_exhausted {
            return None;
        }
        sync_vue3_scope_type_projections(
            &mut global_shadow_projection,
            analysis,
            &shadowed_scope_names,
            namespace_budget,
        )?;
        if !changed {
            converged = true;
            break;
        }
    }
    if !converged {
        return None;
    }
    Some((names, module_import_names))
}

fn vue3_shadowed_scope_projection_names_with_budget(
    analysis: &Vue3ScriptSetupAnalysis,
    base_context: &Vue27TypeContext,
    global_names: &BTreeSet<String>,
    module_names: &BTreeSet<String>,
    shadowed_roots: &BTreeSet<String>,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<BTreeSet<String>> {
    let mut names = BTreeSet::new();
    for name in global_names
        .iter()
        .chain(module_names)
        .chain(analysis.type_sources.keys())
        .chain(analysis.unresolved_import_sources.keys())
        .chain(&analysis.silent_unresolved_type_names)
        .chain(&analysis.local_ts_enum_type_names)
        .chain(base_context.type_sources.keys())
        .chain(base_context.unresolved_import_sources.keys())
        .chain(&base_context.silent_unresolved_type_names)
    {
        let root = name.split('.').next().unwrap_or(name);
        if !shadowed_roots.contains(root) || names.contains(name) {
            continue;
        }
        if !namespace_budget.reserve(name.len().saturating_add(1)) {
            return None;
        }
        names.insert(name.clone());
    }
    Some(names)
}

fn remove_vue3_shadowed_base_type_projections(
    analysis: &mut Vue3ScriptSetupAnalysis,
    base_context: &Vue27TypeContext,
    module_root_names: &BTreeSet<String>,
    module_import_names: &BTreeSet<String>,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<()> {
    let empty = Vue3ScriptSetupAnalysis::default();
    for name in base_context
        .type_sources
        .keys()
        .chain(
            base_context
                .unresolved_import_sources
                .keys()
                .filter(|name| !base_context.type_sources.contains_key(*name)),
        )
        .chain(base_context.silent_unresolved_type_names.iter().filter(|name| {
            !base_context.type_sources.contains_key(*name)
                && !base_context.unresolved_import_sources.contains_key(*name)
        }))
    {
        let root = name.split('.').next().unwrap_or(name);
        if !module_root_names.contains(root) && !module_import_names.contains(root) {
            continue;
        }
        let work = vue3_type_alias_projection_work(analysis, name, name)
            .saturating_add(vue3_external_type_alias_projection_work(
                base_context,
                name,
                name.len(),
                "",
            ));
        if !namespace_budget.reserve(work) {
            return None;
        }
        let mut base_projection = Vue3ScriptSetupAnalysis::default();
        sync_vue3_type_alias_from_context(&mut base_projection, base_context, name, name);
        if !sync_vue3_type_alias_from_analysis(&mut base_projection, analysis, name, name) {
            sync_vue3_type_alias_from_analysis(analysis, &empty, name, name);
            analysis.local_ts_enum_type_names.remove(name);
        }
    }
    Some(())
}

fn sync_vue3_scope_type_projections(
    target: &mut Vue3ScriptSetupAnalysis,
    source: &Vue3ScriptSetupAnalysis,
    names: &BTreeSet<String>,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<()> {
    for name in names {
        let work = vue3_type_alias_projection_work(source, name, name)
            .saturating_add(vue3_type_alias_projection_work(target, name, name))
            .max(name.len().saturating_mul(2).saturating_add(64));
        if !namespace_budget.reserve(work) {
            return None;
        }
        sync_vue3_type_alias_from_analysis(target, source, name, name);
        if source.local_ts_enum_type_names.contains(name) {
            target.local_ts_enum_type_names.insert(name.clone());
        } else {
            target.local_ts_enum_type_names.remove(name);
        }
    }
    Some(())
}

fn restore_vue3_global_base_type_projections(
    analysis: &mut Vue3ScriptSetupAnalysis,
    base_context: &Vue27TypeContext,
    names: &BTreeSet<String>,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<()> {
    for name in names {
        let work = vue3_type_alias_projection_work(analysis, name, name).saturating_add(
            vue3_external_type_alias_projection_work(base_context, name, name.len(), ""),
        );
        if !namespace_budget.reserve(work) {
            return None;
        }
        sync_vue3_type_alias_from_context(analysis, base_context, name, name);
        analysis.local_ts_enum_type_names.remove(name);
    }
    Some(())
}

fn vue3_module_local_graph_names_with_budget(
    statements: &[Statement<'_>],
    is_typescript_definition: bool,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<BTreeSet<String>> {
    let mut names = BTreeSet::new();
    for statement in statements {
        let is_global = match statement {
            Statement::TSGlobalDeclaration(_) => true,
            Statement::TSModuleDeclaration(declaration) => {
                vue3_ts_module_declaration_is_global(declaration)
            }
            Statement::ExportNamedDeclaration(export) => export
                .declaration
                .as_ref()
                .is_some_and(|declaration| match declaration {
                    Declaration::TSModuleDeclaration(declaration) => {
                        vue3_ts_module_declaration_is_global(declaration)
                    }
                    _ => false,
                }),
            _ => false,
        };
        if is_global {
            continue;
        }
        if let Some(namespace) = vue3_namespace_declaration_from_statement(statement) {
            names.extend(vue3_namespace_visible_type_names_with_budget(
                namespace,
                is_typescript_definition || namespace.declare,
                namespace_budget,
            )?);
            continue;
        }
        names.extend(vue3_declared_type_names_from_statement_with_budget(
            statement,
            namespace_budget,
        )?);
    }
    Some(names)
}

fn vue3_module_lexical_type_names_with_budget(
    statements: &[Statement<'_>],
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<(BTreeSet<String>, BTreeSet<String>)> {
    let mut names = BTreeSet::new();
    let mut roots = BTreeSet::new();
    for statement in statements {
        let is_global = match statement {
            Statement::TSGlobalDeclaration(_) => true,
            Statement::TSModuleDeclaration(declaration) => {
                vue3_ts_module_declaration_is_global(declaration)
            }
            Statement::ExportNamedDeclaration(export) => export
                .declaration
                .as_ref()
                .is_some_and(|declaration| match declaration {
                    Declaration::TSModuleDeclaration(declaration) => {
                        vue3_ts_module_declaration_is_global(declaration)
                    }
                    _ => false,
                }),
            _ => false,
        };
        if is_global {
            continue;
        }
        let statement_names =
            vue3_declared_type_names_from_statement_with_budget(statement, namespace_budget)?;
        for name in statement_names {
            let root = name.split('.').next().unwrap_or(&name);
            if !roots.contains(root) {
                if !namespace_budget.reserve(root.len().saturating_add(1)) {
                    return None;
                }
                roots.insert(root.to_string());
            }
            names.insert(name);
        }
        let namespace_root = match statement {
            Statement::TSModuleDeclaration(declaration) => {
                vue3_ts_module_declaration_name_ref(declaration)
            }
            Statement::ExportNamedDeclaration(export) => {
                export.declaration.as_ref().and_then(|declaration| {
                    if let Declaration::TSModuleDeclaration(declaration) = declaration {
                        vue3_ts_module_declaration_name_ref(declaration)
                    } else {
                        None
                    }
                })
            }
            _ => None,
        };
        if let Some(root) = namespace_root {
            if !roots.contains(root) {
                if !namespace_budget.reserve(root.len().saturating_add(1)) {
                    return None;
                }
                roots.insert(root.to_string());
            }
        }
    }
    Some((names, roots))
}

fn vue3_module_lexical_value_root_names_with_budget(
    statements: &[Statement<'_>],
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<BTreeSet<String>> {
    let mut roots = BTreeSet::new();
    for statement in statements {
        let is_global = match statement {
            Statement::TSGlobalDeclaration(_) => true,
            Statement::TSModuleDeclaration(declaration) => {
                vue3_ts_module_declaration_is_global(declaration)
            }
            Statement::ExportNamedDeclaration(export) => export
                .declaration
                .as_ref()
                .is_some_and(|declaration| match declaration {
                    Declaration::TSModuleDeclaration(declaration) => {
                        vue3_ts_module_declaration_is_global(declaration)
                    }
                    _ => false,
                }),
            _ => false,
        };
        if is_global {
            continue;
        }
        match statement {
            Statement::FunctionDeclaration(function) => {
                if let Some(id) = &function.id {
                    insert_vue3_module_lexical_value_root(
                        id.name.as_str(),
                        &mut roots,
                        namespace_budget,
                    )?;
                }
            }
            Statement::VariableDeclaration(declaration) => {
                for declarator in &declaration.declarations {
                    if let Some(name) = first_pattern_binding_name(&declarator.id) {
                        insert_vue3_module_lexical_value_root(
                            name,
                            &mut roots,
                            namespace_budget,
                        )?;
                    }
                }
            }
            Statement::TSEnumDeclaration(declaration) => {
                insert_vue3_module_lexical_value_root(
                    declaration.id.name.as_str(),
                    &mut roots,
                    namespace_budget,
                )?;
            }
            Statement::ClassDeclaration(declaration) => {
                if let Some(id) = &declaration.id {
                    insert_vue3_module_lexical_value_root(
                        id.name.as_str(),
                        &mut roots,
                        namespace_budget,
                    )?;
                }
            }
            Statement::TSModuleDeclaration(declaration) => {
                if let Some(name) = vue3_ts_module_declaration_name_ref(declaration) {
                    insert_vue3_module_lexical_value_root(
                        name,
                        &mut roots,
                        namespace_budget,
                    )?;
                }
            }
            Statement::ExportNamedDeclaration(export) => match export.declaration.as_ref() {
                Some(Declaration::FunctionDeclaration(function)) => {
                    if let Some(id) = &function.id {
                        insert_vue3_module_lexical_value_root(
                            id.name.as_str(),
                            &mut roots,
                            namespace_budget,
                        )?;
                    }
                }
                Some(Declaration::VariableDeclaration(declaration)) => {
                    for declarator in &declaration.declarations {
                        if let Some(name) = first_pattern_binding_name(&declarator.id) {
                            insert_vue3_module_lexical_value_root(
                                name,
                                &mut roots,
                                namespace_budget,
                            )?;
                        }
                    }
                }
                Some(Declaration::TSEnumDeclaration(declaration)) => {
                    insert_vue3_module_lexical_value_root(
                        declaration.id.name.as_str(),
                        &mut roots,
                        namespace_budget,
                    )?;
                }
                Some(Declaration::ClassDeclaration(declaration)) => {
                    if let Some(id) = &declaration.id {
                        insert_vue3_module_lexical_value_root(
                            id.name.as_str(),
                            &mut roots,
                            namespace_budget,
                        )?;
                    }
                }
                Some(Declaration::TSModuleDeclaration(declaration)) => {
                    if let Some(name) = vue3_ts_module_declaration_name_ref(declaration) {
                        insert_vue3_module_lexical_value_root(
                            name,
                            &mut roots,
                            namespace_budget,
                        )?;
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
    Some(roots)
}

fn insert_vue3_module_lexical_value_root(
    name: &str,
    roots: &mut BTreeSet<String>,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<()> {
    if roots.contains(name) {
        return Some(());
    }
    if !namespace_budget.reserve(name.len().saturating_add(1)) {
        return None;
    }
    roots.insert(name.to_string());
    Some(())
}

fn vue3_global_declaration_statement_groups<'a>(
    statements: &'a [Statement<'a>],
) -> Vec<&'a [Statement<'a>]> {
    let mut groups = Vec::new();
    for statement in statements {
        match statement {
            Statement::TSGlobalDeclaration(global) => groups.push(global.body.body.as_slice()),
            Statement::TSModuleDeclaration(declaration)
                if vue3_ts_module_declaration_is_global(declaration) =>
            {
                if let Some(body) = vue3_ts_module_declaration_block_body(declaration) {
                    groups.push(body);
                }
            }
            _ => {}
        }
    }
    groups
}

fn vue3_global_declaration_statement_groups_with_budget<'a>(
    statements: &'a [Statement<'a>],
    include_root: bool,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<Vec<&'a [Statement<'a>]>> {
    let group_count = statements.iter().fold(usize::from(include_root), |count, statement| {
        let has_group = match statement {
            Statement::TSGlobalDeclaration(_) => true,
            Statement::TSModuleDeclaration(declaration)
                if vue3_ts_module_declaration_is_global(declaration) =>
            {
                vue3_ts_module_declaration_block_body(declaration).is_some()
            }
            _ => false,
        };
        count.saturating_add(usize::from(has_group))
    });
    let work = statements.len().saturating_add(
        group_count.saturating_mul(std::mem::size_of::<&[()]>()),
    );
    if !namespace_budget.reserve(work) {
        return None;
    }
    let mut groups = Vec::with_capacity(group_count);
    if include_root {
        groups.push(statements);
    }
    for statement in statements {
        match statement {
            Statement::TSGlobalDeclaration(global) => groups.push(global.body.body.as_slice()),
            Statement::TSModuleDeclaration(declaration)
                if vue3_ts_module_declaration_is_global(declaration) =>
            {
                if let Some(body) = vue3_ts_module_declaration_block_body(declaration) {
                    groups.push(body);
                }
            }
            _ => {}
        }
    }
    Some(groups)
}

fn vue3_statements_are_ambient_global_scope(statements: &[Statement<'_>]) -> bool {
    !statements.iter().any(|statement| {
        matches!(
            statement,
            Statement::ImportDeclaration(_)
                | Statement::ExportAllDeclaration(_)
                | Statement::ExportDefaultDeclaration(_)
                | Statement::ExportNamedDeclaration(_)
                | Statement::TSExportAssignment(_)
                | Statement::TSNamespaceExportDeclaration(_)
        )
            || matches!(
                statement,
                Statement::TSImportEqualsDeclaration(declaration)
                    if matches!(
                        &declaration.module_reference,
                        oxc_ast::ast::TSModuleReference::ExternalModuleReference(_)
                    )
            )
    })
}

fn collect_vue3_ambient_global_type_from_statement(
    source: &str,
    statement: &Statement<'_>,
    implicitly_ambient: bool,
    names: &mut BTreeSet<String>,
    analysis: &mut Vue3ScriptSetupAnalysis,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<()> {
    match statement {
        Statement::TSGlobalDeclaration(global) => {
            names.extend(vue3_declared_type_names_from_statements_with_budget(
                &global.body.body,
                namespace_budget,
            )?);
            collect_vue3_declared_types_from_statements_with_namespace_budget(
                source,
                &global.body.body,
                true,
                0,
                analysis,
                namespace_budget,
            );
        }
        Statement::TSModuleDeclaration(declaration)
            if vue3_ts_module_declaration_is_global(declaration) =>
        {
            if let Some(body) = vue3_ts_module_declaration_block_body(declaration) {
                names.extend(vue3_declared_type_names_from_statements_with_budget(
                    body,
                    namespace_budget,
                )?);
                collect_vue3_declared_types_from_statements_with_namespace_budget(
                    source,
                    body,
                    true,
                    0,
                    analysis,
                    namespace_budget,
                );
            }
        }
        Statement::TSModuleDeclaration(declaration)
            if declaration.declare || implicitly_ambient =>
        {
            names.extend(vue3_namespace_declared_type_names_with_budget(
                declaration,
                namespace_budget,
            )?);
        }
        Statement::TSModuleDeclaration(declaration) => {
            names.extend(vue3_namespace_exported_type_names_with_budget(
                declaration,
                namespace_budget,
            )?);
        }
        _ if vue3_statement_is_declare_type(statement)
            || vue3_statement_is_implicit_ambient_type(statement) =>
        {
            names.extend(vue3_declared_type_names_from_statement_with_budget(
                statement,
                namespace_budget,
            )?);
            collect_vue3_global_declared_type_from_statement(source, statement, analysis);
        }
        _ => {}
    }
    if namespace_budget.is_exhausted() {
        return None;
    }
    Some(())
}

pub(crate) fn project_vue3_global_type_re_exports(
    filename: &str,
    statements: &[Statement<'_>],
    static_resolution_mode: Vue3TypeResolutionMode,
    analysis: &mut Vue3ScriptSetupAnalysis,
    type_resolver: &Vue3TypeResolverContext,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<BTreeSet<String>> {
    let mut names = BTreeSet::new();
    let mut seen = BTreeSet::new();
    for statement in statements {
        let Statement::TSGlobalDeclaration(global) = statement else {
            continue;
        };
        names.extend(project_vue3_type_re_exports(
            filename,
            &global.body.body,
            static_resolution_mode,
            analysis,
            &mut seen,
            type_resolver,
            namespace_budget,
        )?);
        project_vue3_exported_type_specifiers_with_budget(
            &global.body.body,
            analysis,
            namespace_budget,
        )?;
        names.extend(vue3_exported_type_names_with_budget(
            &global.body.body,
            namespace_budget,
        )?);
    }
    Some(names)
}

fn vue3_global_type_file_import_names_with_budget(
    statements: &[Statement<'_>],
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<BTreeSet<String>> {
    if !statements.iter().any(|statement| {
        matches!(
            statement,
            Statement::TSGlobalDeclaration(_)
                | Statement::ExportAllDeclaration(_)
                | Statement::ExportDefaultDeclaration(_)
                | Statement::ExportNamedDeclaration(_)
        )
    }) {
        return Some(BTreeSet::new());
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
            let local = match specifier {
                ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
                    specifier.local.name.as_str()
                }
                ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
                    specifier.local.name.as_str()
                }
                ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
                    specifier.local.name.as_str()
                }
            };
            if names.contains(local) {
                continue;
            }
            if !namespace_budget.reserve(local.len().saturating_add(1)) {
                return None;
            }
            names.insert(local.to_string());
        }
    }
    Some(names)
}

fn vue3_global_type_file_import_scope_identities_with_budget(
    statements: &[Statement<'_>],
    definition_key: &str,
    static_resolution_mode: Vue3TypeResolutionMode,
    type_resolver: &Vue3TypeResolverContext,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<BTreeMap<String, String>> {
    let importer = Path::new(definition_key);
    let importer_directory = importer.parent().unwrap_or_else(|| Path::new(""));
    let mut identities = BTreeMap::new();
    for statement in statements {
        let Statement::ImportDeclaration(import) = statement else {
            continue;
        };
        let Some(specifiers) = &import.specifiers else {
            continue;
        };
        let source = import.source.value.as_str();
        let resolution_mode = vue3_declaration_resolution_mode(
            import.import_kind,
            import.with_clause.as_deref(),
            static_resolution_mode,
        );
        let resolved = resolve_vue3_type_import_with_mode(
            definition_key,
            source,
            resolution_mode,
            type_resolver,
        );
        let source_key = match resolved {
            Some(resolved) => {
                if !namespace_budget.reserve(
                    resolved
                        .as_os_str()
                        .as_encoded_bytes()
                        .len()
                        .saturating_mul(2)
                        .saturating_add(32),
                ) {
                    return None;
                }
                normalize_path_string(&resolved)
            }
            None if source.starts_with('.') => {
                if !namespace_budget.reserve(
                    definition_key
                        .len()
                        .saturating_add(source.len())
                        .saturating_mul(2)
                        .saturating_add(32),
                ) {
                    return None;
                }
                normalize_path_string(&normalize_path_components(importer_directory.join(source)))
            }
            None => {
                if !namespace_budget.reserve(
                    definition_key
                        .len()
                        .saturating_add(source.len())
                        .saturating_add(12),
                ) {
                    return None;
                }
                format!("unresolved:{definition_key}:{source}")
            }
        };
        for specifier in specifiers {
            let local = import_specifier_local_name(specifier);
            let imported = import_specifier_imported_name(specifier).unwrap_or("default");
            let work = source_key
                .len()
                .saturating_add(local.len())
                .saturating_add(imported.len())
                .saturating_add(3);
            if !namespace_budget.reserve(work) {
                return None;
            }
            identities.insert(local.to_string(), format!("{source_key}#{imported}"));
        }
    }
    Some(identities)
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
    let mut statement_groups = vue3_global_declaration_statement_groups(statements);
    if vue3_statements_are_ambient_global_scope(statements) {
        statement_groups.insert(0, statements);
    }
    collect_vue3_declared_type_deps_from_statement_groups(&statement_groups, analysis);
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

fn vue3_statement_is_implicit_ambient_type(statement: &Statement<'_>) -> bool {
    matches!(
        statement,
        Statement::TSInterfaceDeclaration(_)
            | Statement::TSTypeAliasDeclaration(_)
            | Statement::TSEnumDeclaration(_)
            | Statement::FunctionDeclaration(_)
            | Statement::VariableDeclaration(_)
            | Statement::ClassDeclaration(_)
    )
}

pub(crate) fn vue3_declared_type_names_from_statements_with_budget(
    statements: &[Statement<'_>],
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<BTreeSet<String>> {
    let mut names = BTreeSet::new();
    for statement in statements {
        names.extend(vue3_declared_type_names_from_statement_with_budget(
            statement,
            namespace_budget,
        )?);
    }
    Some(names)
}

pub(crate) fn vue3_declared_type_names_from_statement(
    statement: &Statement<'_>,
) -> BTreeSet<String> {
    let mut namespace_budget = Vue3NamespaceProjectionBudget::default();
    vue3_declared_type_names_from_statement_with_budget(statement, &mut namespace_budget)
        .unwrap_or_default()
}

fn vue3_declared_type_names_from_statement_with_budget(
    statement: &Statement<'_>,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<BTreeSet<String>> {
    let mut names = BTreeSet::new();
    match statement {
        Statement::TSInterfaceDeclaration(declaration) => {
            insert_vue3_declared_type_name_with_budget(
                &mut names,
                declaration.id.name.as_str(),
                namespace_budget,
            )?;
        }
        Statement::TSTypeAliasDeclaration(declaration) => {
            insert_vue3_declared_type_name_with_budget(
                &mut names,
                declaration.id.name.as_str(),
                namespace_budget,
            )?;
        }
        Statement::TSEnumDeclaration(declaration) => {
            insert_vue3_declared_type_name_with_budget(
                &mut names,
                declaration.id.name.as_str(),
                namespace_budget,
            )?;
        }
        Statement::FunctionDeclaration(function)
            if vue3_function_has_return_projection(function) =>
        {
            if let Some(id) = &function.id {
                insert_vue3_declared_type_name_with_budget(
                    &mut names,
                    id.name.as_str(),
                    namespace_budget,
                )?;
            }
        }
        Statement::VariableDeclaration(declaration) if declaration.declare => {
            for declarator in &declaration.declarations {
                if let Some(name) = first_pattern_binding_name(&declarator.id) {
                    insert_vue3_declared_type_name_with_budget(
                        &mut names,
                        name,
                        namespace_budget,
                    )?;
                }
            }
        }
        Statement::VariableDeclaration(declaration) => {
            for declarator in &declaration.declarations {
                if vue3_variable_declarator_has_type_projection(declarator) {
                    if let Some(name) = first_pattern_binding_name(&declarator.id) {
                        insert_vue3_declared_type_name_with_budget(
                            &mut names,
                            name,
                            namespace_budget,
                        )?;
                    }
                }
            }
        }
        Statement::ClassDeclaration(declaration) => {
            if let Some(id) = &declaration.id {
                insert_vue3_declared_type_name_with_budget(
                    &mut names,
                    id.name.as_str(),
                    namespace_budget,
                )?;
            }
        }
        Statement::TSModuleDeclaration(declaration) => {
            names.extend(vue3_namespace_declared_type_names_with_budget(
                declaration,
                namespace_budget,
            )?);
        }
        Statement::ExportNamedDeclaration(declaration) => {
            if let Some(declaration) = &declaration.declaration {
                names.extend(vue3_declared_type_names_from_declaration_with_budget(
                    declaration,
                    namespace_budget,
                )?);
            }
        }
        _ => {}
    }
    Some(names)
}

fn insert_vue3_declared_type_name_with_budget(
    names: &mut BTreeSet<String>,
    name: &str,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<()> {
    if names.contains(name) {
        return Some(());
    }
    if !namespace_budget.reserve(name.len().saturating_add(1)) {
        return None;
    }
    names.insert(name.to_string());
    Some(())
}

pub(crate) fn vue3_declared_type_names_from_declaration(
    declaration: &Declaration<'_>,
) -> BTreeSet<String> {
    let mut namespace_budget = Vue3NamespaceProjectionBudget::default();
    vue3_declared_type_names_from_declaration_with_budget(declaration, &mut namespace_budget)
        .unwrap_or_default()
}

fn vue3_declared_type_names_from_declaration_with_budget(
    declaration: &Declaration<'_>,
    namespace_budget: &mut Vue3NamespaceProjectionBudget,
) -> Option<BTreeSet<String>> {
    let mut names = BTreeSet::new();
    match declaration {
        Declaration::TSInterfaceDeclaration(declaration) => {
            insert_vue3_declared_type_name_with_budget(
                &mut names,
                declaration.id.name.as_str(),
                namespace_budget,
            )?;
        }
        Declaration::TSTypeAliasDeclaration(declaration) => {
            insert_vue3_declared_type_name_with_budget(
                &mut names,
                declaration.id.name.as_str(),
                namespace_budget,
            )?;
        }
        Declaration::TSEnumDeclaration(declaration) => {
            insert_vue3_declared_type_name_with_budget(
                &mut names,
                declaration.id.name.as_str(),
                namespace_budget,
            )?;
        }
        Declaration::FunctionDeclaration(function)
            if vue3_function_has_return_projection(function) =>
        {
            if let Some(id) = &function.id {
                insert_vue3_declared_type_name_with_budget(
                    &mut names,
                    id.name.as_str(),
                    namespace_budget,
                )?;
            }
        }
        Declaration::VariableDeclaration(declaration) if declaration.declare => {
            for declarator in &declaration.declarations {
                if let Some(name) = first_pattern_binding_name(&declarator.id) {
                    insert_vue3_declared_type_name_with_budget(
                        &mut names,
                        name,
                        namespace_budget,
                    )?;
                }
            }
        }
        Declaration::VariableDeclaration(declaration) => {
            for declarator in &declaration.declarations {
                if vue3_variable_declarator_has_type_projection(declarator) {
                    if let Some(name) = first_pattern_binding_name(&declarator.id) {
                        insert_vue3_declared_type_name_with_budget(
                            &mut names,
                            name,
                            namespace_budget,
                        )?;
                    }
                }
            }
        }
        Declaration::ClassDeclaration(declaration) => {
            if let Some(id) = &declaration.id {
                insert_vue3_declared_type_name_with_budget(
                    &mut names,
                    id.name.as_str(),
                    namespace_budget,
                )?;
            }
        }
        Declaration::TSModuleDeclaration(declaration) => {
            names.extend(vue3_namespace_declared_type_names_with_budget(
                declaration,
                namespace_budget,
            )?);
        }
        _ => {}
    }
    Some(names)
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
