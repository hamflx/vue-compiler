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
    pub(crate) interface_heritage: Option<Vue3InterfaceHeritageEvidence>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Vue3InterfaceHeritageEvidence {
    pub(crate) own_members:
        BTreeMap<String, BTreeSet<Vue3InterfaceHeritageMemberEvidence>>,
    pub(crate) inherited_members:
        BTreeMap<String, BTreeSet<Vue3InterfaceHeritageMemberEvidence>>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Vue3InterfaceHeritageMemberEvidence {
    pub(crate) exact_primitive_types: Option<BTreeSet<String>>,
    pub(crate) required: Option<bool>,
}

impl Vue3InterfaceHeritageEvidence {
    pub(crate) fn work(&self) -> usize {
        [&self.own_members, &self.inherited_members]
            .into_iter()
            .flat_map(|members| members.iter())
            .fold(0usize, |work, (key, members)| {
                members.iter().fold(
                    work.saturating_add(key.len()).saturating_add(1),
                    |work, member| {
                        let work = work
                            .saturating_add(std::mem::size_of::<
                                Vue3InterfaceHeritageMemberEvidence,
                            >())
                            .saturating_add(1);
                        member.exact_primitive_types.as_ref().map_or(work, |types| {
                            types.iter().fold(work, |work, ty| {
                                work.saturating_add(ty.len()).saturating_add(1)
                            })
                        })
                    },
                )
            })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Vue3ValueTypeProjection {
    pub(crate) type_query_declared_types: Option<Vec<String>>,
    pub(crate) define_model_type_query_declared_types: Option<Vec<String>>,
    pub(crate) keyof_type_query_declared_types: Option<Vec<String>>,
    pub(crate) return_type_runtime_type_declarations: Option<Vec<String>>,
    pub(crate) define_model_return_type_runtime_type_declarations: Option<Vec<String>>,
    pub(crate) props_options_type_declarations: Option<Vue27TypeMembers>,
    pub(crate) return_type_props_options_declarations: Option<Vue27TypeMembers>,
    pub(crate) unresolved_import_sources: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Vue3PropsTypeResolveMode {
    Silent,
    Consumed,
}

pub(crate) type Vue3RuntimeTypeTuple = Vec<Vec<String>>;

#[derive(Clone)]
pub(crate) struct Vue3GenericTypeAlias {
    pub(crate) source: String,
    pub(crate) kind: Vue3GenericTypeAliasKind,
    pub(crate) params: Vec<String>,
    pub(crate) scope: Vue3GenericTypeScope,
    pub(crate) interface_fragments: Vec<Vue3GenericInterfaceFragment>,
}

impl std::fmt::Debug for Vue3GenericTypeAlias {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Vue3GenericTypeAlias")
            .field("source", &self.source)
            .field("kind", &self.kind)
            .field("params", &self.params)
            .field("scope", &self.scope)
            .field("interface_fragments", &self.interface_fragments)
            .finish()
    }
}

impl PartialEq for Vue3GenericTypeAlias {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source
            && self.kind == other.kind
            && self.params == other.params
            && self.scope == other.scope
            && self.interface_fragments == other.interface_fragments
    }
}

impl Eq for Vue3GenericTypeAlias {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Vue3GenericInterfaceFragment {
    pub(crate) source: String,
    pub(crate) scope: Vue3GenericTypeScope,
}

#[derive(Clone)]
pub(crate) enum Vue3GenericTypeScope {
    Local,
    Captured(std::sync::Arc<Vue3GenericTypeEnvironment>),
}

impl std::fmt::Debug for Vue3GenericTypeScope {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Local => formatter.write_str("Local"),
            Self::Captured(environment) => formatter
                .debug_tuple("Captured")
                .field(&(std::sync::Arc::as_ptr(environment) as usize))
                .finish(),
        }
    }
}

impl PartialEq for Vue3GenericTypeScope {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Local, Self::Local) => true,
            (Self::Captured(left), Self::Captured(right)) => std::sync::Arc::ptr_eq(left, right),
            _ => false,
        }
    }
}

impl Eq for Vue3GenericTypeScope {}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Vue3GenericTypeEnvironment {
    pub(crate) definition_filename: Option<String>,
    pub(crate) definition_resolution_mode: Vue3TypeResolutionMode,
    pub(crate) generic_type_aliases: BTreeMap<String, Vue3GenericTypeAlias>,
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Vue3TypeModuleResolutionKind {
    Classic,
    #[default]
    Node10,
    Node16,
    NodeNext,
    Bundler,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Vue3TypeModuleKind {
    Classic,
    #[default]
    CommonJs,
    EcmaScript,
    Node16,
    NodeNext,
    Preserve,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Vue3PackageJsonResolutionFeatures {
    pub(crate) imports: bool,
    pub(crate) imports_pattern_root: bool,
    pub(crate) self_name: bool,
    pub(crate) exports: bool,
}

impl Vue3PackageJsonResolutionFeatures {
    fn all(typescript_version: &nodejs_semver::Version) -> Self {
        Self {
            imports: true,
            imports_pattern_root: typescript_version >= &(6, 0, 0).into(),
            self_name: true,
            exports: true,
        }
    }
}

impl Vue3TypeModuleResolutionKind {
    fn default_package_json_features(
        self,
        typescript_version: &nodejs_semver::Version,
    ) -> Vue3PackageJsonResolutionFeatures {
        match self {
            Self::Node16 if typescript_version >= &(4, 7, 0).into() => {
                Vue3PackageJsonResolutionFeatures {
                    imports: true,
                    imports_pattern_root: false,
                    self_name: true,
                    exports: true,
                }
            }
            Self::NodeNext if typescript_version >= &(4, 7, 0).into() => {
                Vue3PackageJsonResolutionFeatures::all(typescript_version)
            }
            Self::Bundler if typescript_version >= &(5, 0, 0).into() => {
                Vue3PackageJsonResolutionFeatures::all(typescript_version)
            }
            Self::Classic | Self::Node10 | Self::Node16 | Self::NodeNext | Self::Bundler => {
                Vue3PackageJsonResolutionFeatures::default()
            }
        }
    }

    pub(crate) fn uses_node_esm_specifier_rules(
        self,
        resolution_mode: Vue3TypeResolutionMode,
        typescript_version: &nodejs_semver::Version,
    ) -> bool {
        typescript_version >= &(4, 7, 0).into()
            && matches!(self, Self::Node16 | Self::NodeNext)
            && resolution_mode == Vue3TypeResolutionMode::Import
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Vue3TypeResolverContext {
    pub(crate) typescript_version: nodejs_semver::Version,
    pub(crate) module_resolution: Vue3TypeModuleResolutionKind,
    pub(crate) module: Option<Vue3TypeModuleKind>,
    pub(crate) allow_js: bool,
    pub(crate) resolve_package_json_exports: Option<bool>,
    pub(crate) resolve_package_json_imports: Option<bool>,
    pub(crate) active_package_json_features: Option<Vue3PackageJsonResolutionFeatures>,
    pub(crate) module_suffixes: std::sync::Arc<[String]>,
    pub(crate) external_type_session: Vue3ExternalTypeLoadSession,
}

impl Vue3TypeResolverContext {
    pub(crate) fn effective_module(&self) -> Vue3TypeModuleKind {
        self.module.unwrap_or(match self.module_resolution {
            Vue3TypeModuleResolutionKind::Classic => Vue3TypeModuleKind::Classic,
            Vue3TypeModuleResolutionKind::Node10 => Vue3TypeModuleKind::CommonJs,
            Vue3TypeModuleResolutionKind::Node16 => Vue3TypeModuleKind::Node16,
            Vue3TypeModuleResolutionKind::NodeNext => Vue3TypeModuleKind::NodeNext,
            Vue3TypeModuleResolutionKind::Bundler => Vue3TypeModuleKind::Preserve,
        })
    }

    fn configured_package_json_features(&self) -> Vue3PackageJsonResolutionFeatures {
        let mut features = self
            .module_resolution
            .default_package_json_features(&self.typescript_version);
        if self.typescript_version >= (5, 0, 0).into()
            && self.module_resolution == Vue3TypeModuleResolutionKind::Bundler
        {
            if let Some(enabled) = self.resolve_package_json_exports {
                features.exports = enabled;
            }
            if let Some(enabled) = self.resolve_package_json_imports {
                features.imports = enabled;
            }
        }
        features
    }

    fn type_reference_package_json_features(&self) -> Vue3PackageJsonResolutionFeatures {
        let mut features = self
            .module_resolution
            .default_package_json_features(&self.typescript_version);
        if self.typescript_version >= (5, 0, 0).into() {
            if let Some(enabled) = self.resolve_package_json_exports {
                features.exports = enabled;
            }
            if let Some(enabled) = self.resolve_package_json_imports {
                features.imports = enabled;
            }
        }
        features
    }

    pub(crate) fn package_json_features(&self) -> Vue3PackageJsonResolutionFeatures {
        self.active_package_json_features
            .unwrap_or_else(|| self.configured_package_json_features())
    }

    pub(crate) fn package_json_features_for_request(
        &self,
        explicit_mode: bool,
    ) -> Vue3PackageJsonResolutionFeatures {
        if let Some(features) = self.active_package_json_features {
            return features;
        }
        if explicit_mode
            && self.typescript_version >= (5, 3, 0).into()
            && self.module_resolution == Vue3TypeModuleResolutionKind::Node10
        {
            Vue3PackageJsonResolutionFeatures::all(&self.typescript_version)
        } else {
            self.configured_package_json_features()
        }
    }

    pub(crate) fn package_json_features_for_type_reference(
        &self,
        mode_present: bool,
    ) -> Vue3PackageJsonResolutionFeatures {
        if let Some(features) = self.active_package_json_features {
            return features;
        }
        if mode_present && self.typescript_version >= (5, 3, 0).into() {
            Vue3PackageJsonResolutionFeatures::all(&self.typescript_version)
        } else {
            self.type_reference_package_json_features()
        }
    }
}

impl PartialEq for Vue3TypeResolverContext {
    fn eq(&self, other: &Self) -> bool {
        self.typescript_version == other.typescript_version
            && self.module_resolution == other.module_resolution
            && self.effective_module() == other.effective_module()
            && self.allow_js == other.allow_js
            && self.package_json_features() == other.package_json_features()
            && self.package_json_features_for_type_reference(false)
                == other.package_json_features_for_type_reference(false)
            && self.module_suffixes == other.module_suffixes
            && self.external_type_session.limits() == other.external_type_session.limits()
    }
}

impl Eq for Vue3TypeResolverContext {}

pub(crate) fn vue3_default_module_suffixes() -> std::sync::Arc<[String]> {
    std::sync::Arc::from([String::new()])
}

impl Default for Vue3TypeResolverContext {
    fn default() -> Self {
        Self {
            typescript_version: vue3_package_typescript_baseline_version(),
            module_resolution: Vue3TypeModuleResolutionKind::default(),
            module: None,
            allow_js: false,
            resolve_package_json_exports: None,
            resolve_package_json_imports: None,
            active_package_json_features: None,
            module_suffixes: vue3_default_module_suffixes(),
            external_type_session: Vue3ExternalTypeLoadSession::default(),
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
    let mut analysis = Vue27ScriptSetupAnalysis {
        normal_imports: setup_context.normal_imports.clone(),
        ..Vue27ScriptSetupAnalysis::default()
    };
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
