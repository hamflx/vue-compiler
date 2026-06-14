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
