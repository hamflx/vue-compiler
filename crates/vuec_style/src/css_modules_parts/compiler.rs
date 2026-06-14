use crate::*;

pub(crate) fn compile_css_modules(
    source: &str,
    hash_source: &str,
    options: &StyleCompileOptions,
) -> CssModulesCompileResult {
    let filename = options
        .filename
        .as_deref()
        .unwrap_or("style.css")
        .to_string();
    let scope_behaviour = CssModulesScopeBehaviour::from_options(
        &options.modules_options.scope_behaviour,
        &options.modules_options.global_module_paths,
        &filename,
    );
    let mut active_paths = Vec::new();
    compile_css_modules_file(
        source,
        hash_source,
        options,
        filename,
        scope_behaviour,
        false,
        &mut active_paths,
    )
}

pub(crate) fn compile_css_modules_file(
    source: &str,
    hash_source: &str,
    options: &StyleCompileOptions,
    filename: String,
    scope_behaviour: CssModulesScopeBehaviour,
    imported_dependency: bool,
    active_paths: &mut Vec<PathBuf>,
) -> CssModulesCompileResult {
    let active_path = css_module_active_path(&filename);
    let pushed_active = !active_paths.iter().any(|active| active == &active_path);
    if pushed_active {
        active_paths.push(active_path);
    }
    let mut context = CssModulesContext::new(
        options,
        filename,
        hash_source.to_string(),
        scope_behaviour,
        imported_dependency,
        active_paths,
    );
    let source = prepare_css_module_values(source, &mut context);
    let code = rewrite_css_modules_items(&source, &mut context, CssBlockContext::Root, false);
    let has_prepended_css = !context.prepended_css.is_empty();
    let code = context.finish_code(code);
    let raw_modules = context.raw_modules();
    let modules = context.modules();
    if pushed_active {
        context.active_paths.pop();
    }
    CssModulesCompileResult {
        code,
        raw_modules,
        modules,
        diagnostics: context.diagnostics.clone(),
        has_prepended_css,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CssModulesCompileResult {
    pub(crate) code: String,
    pub(crate) raw_modules: BTreeMap<String, String>,
    pub(crate) modules: BTreeMap<String, String>,
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) has_prepended_css: bool,
}

#[derive(Debug)]
pub(crate) struct CssModulesContext<'a> {
    pub(crate) options: &'a StyleCompileOptions,
    pub(crate) filename: String,
    pub(crate) hash_source: String,
    pub(crate) scope_behaviour: CssModulesScopeBehaviour,
    pub(crate) generate_scoped_name: Option<&'a str>,
    pub(crate) hash_prefix: &'a str,
    pub(crate) locals_convention: CssModulesLocalsConvention,
    pub(crate) export_globals: bool,
    pub(crate) imported_dependency: bool,
    pub(crate) raw_exports: Vec<CssModuleExport>,
    pub(crate) raw_export_index: BTreeMap<String, usize>,
    pub(crate) import_symbols: BTreeMap<String, CssModuleImportSymbol>,
    pub(crate) imported_modules: BTreeMap<String, CssModulesCompileResult>,
    pub(crate) prepended_css_has_nested_import: bool,
    pub(crate) value_placeholders: BTreeMap<String, String>,
    pub(crate) value_placeholder_modules: BTreeMap<String, String>,
    pub(crate) next_value_placeholder: usize,
    pub(crate) prepended_paths: BTreeSet<String>,
    pub(crate) prepended_css: Vec<String>,
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) active_paths: &'a mut Vec<PathBuf>,
}

impl<'a> CssModulesContext<'a> {
    pub(crate) fn new(
        options: &'a StyleCompileOptions,
        filename: String,
        hash_source: String,
        scope_behaviour: CssModulesScopeBehaviour,
        imported_dependency: bool,
        active_paths: &'a mut Vec<PathBuf>,
    ) -> Self {
        Self {
            options,
            filename,
            hash_source,
            scope_behaviour,
            generate_scoped_name: options.modules_options.generate_scoped_name.as_deref(),
            hash_prefix: &options.modules_options.hash_prefix,
            locals_convention: CssModulesLocalsConvention::from_option(
                &options.modules_options.locals_convention,
            ),
            export_globals: options.modules_options.export_globals,
            imported_dependency,
            raw_exports: Vec::new(),
            raw_export_index: BTreeMap::new(),
            import_symbols: BTreeMap::new(),
            imported_modules: BTreeMap::new(),
            prepended_css_has_nested_import: false,
            value_placeholders: BTreeMap::new(),
            value_placeholder_modules: BTreeMap::new(),
            next_value_placeholder: 0,
            prepended_paths: BTreeSet::new(),
            prepended_css: Vec::new(),
            diagnostics: Vec::new(),
            active_paths,
        }
    }

    pub(crate) fn is_local_default(&self) -> bool {
        matches!(self.scope_behaviour, CssModulesScopeBehaviour::Local)
    }

    pub(crate) fn scoped_name(&self, local: &str) -> String {
        if let Some(pattern) = self.generate_scoped_name {
            return format_css_module_pattern(pattern, &self.filename, local, self.hash_prefix);
        }
        format_css_module_default_scoped_name(local, &self.hash_source)
    }

    pub(crate) fn register_local(&mut self, local: &str, scoped: &str) {
        self.push_raw_export_value(local, scoped);
    }

    pub(crate) fn register_global(&mut self, name: &str) {
        self.set_raw_export_values(name, vec![name.to_string()]);
    }

    pub(crate) fn compose(&mut self, local: &str, values: Vec<String>) {
        for value in values {
            self.push_raw_export_value(local, &value);
        }
    }

    pub(crate) fn scoped_local_value(&mut self, local: &str) -> String {
        let scoped = self.scoped_name(local);
        self.register_local(local, &scoped);
        scoped
    }

    pub(crate) fn raw_export_values(&self, local: &str) -> Option<Vec<String>> {
        self.raw_export_index
            .get(local)
            .map(|index| self.raw_exports[*index].values.clone())
    }

    pub(crate) fn raw_modules(&self) -> BTreeMap<String, String> {
        self.raw_exports
            .iter()
            .map(|export| (export.local.clone(), export.values.join(" ")))
            .collect()
    }

    pub(crate) fn import_symbol_value(&self, local: &str) -> Option<&str> {
        match self.import_symbols.get(local)? {
            CssModuleImportSymbol::Found(value) => Some(value),
            CssModuleImportSymbol::Missing => None,
        }
    }

    pub(crate) fn import_symbol_module_value(&self, local: &str) -> Option<String> {
        match self.import_symbols.get(local)? {
            CssModuleImportSymbol::Found(value) => Some(value.clone()),
            CssModuleImportSymbol::Missing => Some("undefined".to_string()),
        }
    }

    pub(crate) fn import_symbol_is_imported(&self, local: &str) -> bool {
        self.import_symbols.contains_key(local)
    }

    pub(crate) fn push_raw_export_value(&mut self, local: &str, value: &str) {
        if let Some(index) = self.raw_export_index.get(local).copied() {
            let export = &mut self.raw_exports[index];
            if !export.values.iter().any(|existing| existing == value) {
                export.values.push(value.to_string());
            }
            return;
        }

        let index = self.raw_exports.len();
        self.raw_exports.push(CssModuleExport {
            local: local.to_string(),
            values: vec![value.to_string()],
        });
        self.raw_export_index.insert(local.to_string(), index);
    }

    pub(crate) fn set_raw_export_values(&mut self, local: &str, values: Vec<String>) {
        if let Some(index) = self.raw_export_index.get(local).copied() {
            self.raw_exports[index].values = values;
            return;
        }

        let index = self.raw_exports.len();
        self.raw_exports.push(CssModuleExport {
            local: local.to_string(),
            values,
        });
        self.raw_export_index.insert(local.to_string(), index);
    }

    pub(crate) fn modules(&self) -> BTreeMap<String, String> {
        let mut modules = BTreeMap::new();
        for export in &self.raw_exports {
            let value = export.values.join(" ");
            self.register_module_export(&mut modules, &export.local, &value);
        }
        modules
    }

    pub(crate) fn finish_code(&self, code: String) -> String {
        let mut output = String::new();
        for css in &self.prepended_css {
            if css.is_empty() {
                continue;
            }
            if !self.imported_dependency && !output.is_empty() && !output.ends_with('\n') {
                output.push('\n');
            }
            output.push_str(css);
        }
        if !self.imported_dependency
            && !self.prepended_css_has_nested_import
            && !output.is_empty()
            && !code.is_empty()
            && !output.ends_with('\n')
        {
            output.push('\n');
        }
        output.push_str(&code);
        self.replace_value_placeholders(output)
    }

    pub(crate) fn import_value_placeholder(
        &mut self,
        replacement: String,
        module_value: String,
    ) -> String {
        let placeholder = format!("__vuec_value_{}", self.next_value_placeholder);
        self.next_value_placeholder += 1;
        self.value_placeholders
            .insert(placeholder.clone(), replacement);
        self.value_placeholder_modules
            .insert(placeholder.clone(), module_value);
        placeholder
    }

    pub(crate) fn value_placeholder_replacement(&self, placeholder: &str) -> Option<&str> {
        self.value_placeholders.get(placeholder).map(String::as_str)
    }

    pub(crate) fn value_placeholder_module_value(&self, placeholder: &str) -> Option<&str> {
        self.value_placeholder_modules
            .get(placeholder)
            .map(String::as_str)
    }

    pub(crate) fn replace_value_placeholders(&self, source: String) -> String {
        if self.value_placeholders.is_empty() {
            return source;
        }
        let mut output = source;
        for (placeholder, value) in &self.value_placeholders {
            output = output.replace(placeholder, value);
        }
        output
    }

    pub(crate) fn load_imported_module(&mut self, import: &str) -> Option<CssModulesCompileResult> {
        let resolved = resolve_css_module_import(import, &self.filename)?;
        let normalized = normalize_dependency_path(&resolved.path);
        if let Some(result) = self.imported_modules.get(&normalized) {
            return Some(result.clone());
        }
        if self
            .active_paths
            .iter()
            .any(|active| active == &resolved.path)
        {
            return None;
        }
        let source = std::fs::read_to_string(&resolved.path).ok()?;
        let normalized_source = normalize_style_output(&source);
        let result = compile_css_modules_file(
            &normalized_source,
            &source,
            self.options,
            resolved.logical_filename,
            self.scope_behaviour,
            true,
            self.active_paths,
        );
        if self.prepended_paths.insert(normalized.clone()) && !result.code.is_empty() {
            if result.has_prepended_css {
                self.prepended_css_has_nested_import = true;
            }
            self.prepended_css.push(result.code.clone());
        }
        self.imported_modules.insert(normalized, result.clone());
        Some(result)
    }

    pub(crate) fn push_compose_diagnostic(&mut self, message: String, start: usize, end: usize) {
        self.diagnostics.push(
            Diagnostic::error("VUEC_STYLE_MODULE_COMPOSE", message)
                .with_span(Some(style_source_span(self.options, start, end))),
        );
    }

    pub(crate) fn register_module_export(
        &self,
        modules: &mut BTreeMap<String, String>,
        local: &str,
        value: &str,
    ) {
        match self.locals_convention {
            CssModulesLocalsConvention::AsIs => {
                modules.insert(local.to_string(), value.to_string());
            }
            CssModulesLocalsConvention::CamelCase => {
                modules.insert(local.to_string(), value.to_string());
                modules.insert(camel_case_css_module_key(local), value.to_string());
            }
            CssModulesLocalsConvention::CamelCaseOnly => {
                modules.insert(camel_case_css_module_key(local), value.to_string());
            }
            CssModulesLocalsConvention::Dashes => {
                modules.insert(local.to_string(), value.to_string());
                modules.insert(dashes_css_module_key(local), value.to_string());
            }
            CssModulesLocalsConvention::DashesOnly => {
                modules.insert(dashes_css_module_key(local), value.to_string());
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct CssModuleExport {
    pub(crate) local: String,
    pub(crate) values: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CssModuleImportSymbol {
    Found(String),
    Missing,
}
