use crate::*;
use std::io::Read;
use std::sync::Arc;

pub(crate) const CSS_MODULES_MAX_IMPORT_DEPTH: usize = 64;
pub(crate) const CSS_MODULES_MAX_IMPORT_FILES: usize = 512;
pub(crate) const CSS_MODULES_MAX_IMPORT_FILE_BYTES: usize = 8 * 1024 * 1024;
pub(crate) const CSS_MODULES_MAX_IMPORT_BYTES: usize = 64 * 1024 * 1024;
pub(crate) const CSS_MODULES_MAX_METADATA_FILE_BYTES: usize = 1024 * 1024;
pub(crate) const CSS_MODULES_MAX_METADATA_BYTES: usize = 8 * 1024 * 1024;
pub(crate) const CSS_MODULES_MAX_PATH_BYTES: usize = 32 * 1024;
pub(crate) const CSS_MODULES_MAX_PATH_PROBES: usize = 65_536;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CssModulesImportLimits {
    pub(crate) max_depth: usize,
    pub(crate) max_files: usize,
    pub(crate) max_file_bytes: usize,
    pub(crate) max_total_bytes: usize,
    pub(crate) max_metadata_file_bytes: usize,
    pub(crate) max_metadata_bytes: usize,
    pub(crate) max_path_bytes: usize,
    pub(crate) max_path_probes: usize,
}

impl Default for CssModulesImportLimits {
    fn default() -> Self {
        Self {
            max_depth: CSS_MODULES_MAX_IMPORT_DEPTH,
            max_files: CSS_MODULES_MAX_IMPORT_FILES,
            max_file_bytes: CSS_MODULES_MAX_IMPORT_FILE_BYTES,
            max_total_bytes: CSS_MODULES_MAX_IMPORT_BYTES,
            max_metadata_file_bytes: CSS_MODULES_MAX_METADATA_FILE_BYTES,
            max_metadata_bytes: CSS_MODULES_MAX_METADATA_BYTES,
            max_path_bytes: CSS_MODULES_MAX_PATH_BYTES,
            max_path_probes: CSS_MODULES_MAX_PATH_PROBES,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CssModulesImportError {
    pub(crate) message: String,
}

#[derive(Debug)]
pub(crate) struct CssModulesImportState {
    pub(crate) limits: CssModulesImportLimits,
    pub(crate) active_paths: Vec<PathBuf>,
    pub(crate) imported_files: usize,
    pub(crate) imported_bytes: usize,
    pub(crate) metadata_bytes: usize,
    pub(crate) path_probes: usize,
    pub(crate) error: Option<CssModulesImportError>,
}

impl CssModulesImportState {
    pub(crate) fn new(limits: CssModulesImportLimits) -> Self {
        Self {
            limits,
            active_paths: Vec::new(),
            imported_files: 0,
            imported_bytes: 0,
            metadata_bytes: 0,
            path_probes: 0,
            error: None,
        }
    }

    pub(crate) fn fail(&mut self, message: impl Into<String>) {
        if self.error.is_none() {
            self.error = Some(CssModulesImportError {
                message: message.into(),
            });
        }
    }

    pub(crate) fn validate_path(&mut self, path: &Path, description: &str) -> bool {
        if self.error.is_some() {
            return false;
        }
        let bytes = path.as_os_str().as_encoded_bytes().len();
        if bytes > self.limits.max_path_bytes {
            self.fail(format!(
                "{description} exceeds the maximum of {} bytes",
                self.limits.max_path_bytes
            ));
            return false;
        }
        true
    }

    pub(crate) fn claim_path_probe(&mut self, path: &Path, description: &str) -> bool {
        if !self.validate_path(path, description) {
            return false;
        }
        if self.path_probes >= self.limits.max_path_probes {
            self.fail(format!(
                "CSS Modules import resolution exceeds the maximum of {} path probes",
                self.limits.max_path_probes
            ));
            return false;
        }
        self.path_probes += 1;
        true
    }

    pub(crate) fn is_file(&mut self, path: &Path) -> bool {
        self.claim_path_probe(path, "CSS Modules import resolution path") && path.is_file()
    }

    pub(crate) fn is_dir(&mut self, path: &Path) -> bool {
        self.claim_path_probe(path, "CSS Modules import resolution path") && path.is_dir()
    }

    pub(crate) fn canonicalize(&mut self, path: &Path) -> Option<PathBuf> {
        if !self.claim_path_probe(path, "CSS Modules import resolution path") {
            return None;
        }
        std::fs::canonicalize(path).ok()
    }

    pub(crate) fn read_module(&mut self, path: &Path) -> Option<String> {
        if !self.validate_path(path, "CSS Modules import path") {
            return None;
        }
        if self.imported_files >= self.limits.max_files {
            self.fail(format!(
                "CSS Modules imports exceed the maximum file count of {}",
                self.limits.max_files
            ));
            return None;
        }
        let remaining = self
            .limits
            .max_total_bytes
            .saturating_sub(self.imported_bytes);
        let read_limit = self
            .limits
            .max_file_bytes
            .min(remaining)
            .saturating_add(1);
        let file = std::fs::File::open(path).ok()?;
        let mut bytes = Vec::new();
        let read_result = file
            .take(u64::try_from(read_limit).unwrap_or(u64::MAX))
            .read_to_end(&mut bytes);
        if bytes.len() > self.limits.max_file_bytes {
            self.fail(format!(
                "CSS Modules import exceeds the maximum of {} bytes: {}",
                self.limits.max_file_bytes,
                path.to_string_lossy()
            ));
            return None;
        }
        let imported_bytes = self.imported_bytes.checked_add(bytes.len()).or_else(|| {
            self.fail("CSS Modules import byte count overflowed");
            None
        })?;
        if imported_bytes > self.limits.max_total_bytes {
            self.fail(format!(
                "CSS Modules imports exceed the maximum total of {} bytes",
                self.limits.max_total_bytes
            ));
            return None;
        }
        self.imported_files += 1;
        self.imported_bytes = imported_bytes;
        read_result.ok()?;
        String::from_utf8(bytes).ok()
    }

    pub(crate) fn read_metadata(&mut self, path: &Path) -> Option<String> {
        if !self.claim_path_probe(path, "CSS Modules package metadata path") {
            return None;
        }
        let remaining = self
            .limits
            .max_metadata_bytes
            .saturating_sub(self.metadata_bytes);
        let read_limit = self
            .limits
            .max_metadata_file_bytes
            .min(remaining)
            .saturating_add(1);
        let file = std::fs::File::open(path).ok()?;
        let mut bytes = Vec::new();
        let read_result = file
            .take(u64::try_from(read_limit).unwrap_or(u64::MAX))
            .read_to_end(&mut bytes);
        if bytes.len() > self.limits.max_metadata_file_bytes {
            self.fail(format!(
                "CSS Modules package metadata exceeds the maximum of {} bytes: {}",
                self.limits.max_metadata_file_bytes,
                path.to_string_lossy()
            ));
            return None;
        }
        let metadata_bytes = self.metadata_bytes.checked_add(bytes.len()).or_else(|| {
            self.fail("CSS Modules package metadata byte count overflowed");
            None
        })?;
        if metadata_bytes > self.limits.max_metadata_bytes {
            self.fail(format!(
                "CSS Modules package metadata exceeds the maximum total of {} bytes",
                self.limits.max_metadata_bytes
            ));
            return None;
        }
        self.metadata_bytes = metadata_bytes;
        read_result.ok()?;
        String::from_utf8(bytes).ok()
    }
}

pub(crate) fn compile_css_modules(
    source: &str,
    hash_source: &str,
    options: &StyleCompileOptions,
) -> CssModulesCompileResult {
    compile_css_modules_with_limits(
        source,
        hash_source,
        options,
        CssModulesImportLimits::default(),
    )
}

pub(crate) fn compile_css_modules_with_limits(
    source: &str,
    hash_source: &str,
    options: &StyleCompileOptions,
    limits: CssModulesImportLimits,
) -> CssModulesCompileResult {
    let mut load_state = CssModulesImportState::new(limits);
    let filename = options.filename.as_deref().unwrap_or("style.css");
    if !load_state.validate_path(Path::new(filename), "CSS Modules entry path") {
        return css_modules_import_limit_result(source, options, load_state.error);
    }
    let active_path = load_state
        .canonicalize(Path::new(filename))
        .unwrap_or_else(|| PathBuf::from(filename));
    if load_state.error.is_some()
        || !load_state.validate_path(&active_path, "CSS Modules resolved entry path")
    {
        return css_modules_import_limit_result(source, options, load_state.error);
    }
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
    load_state.active_paths.push(active_path);
    let mut result = compile_css_modules_file(
        source,
        hash_source,
        options,
        filename,
        scope_behaviour,
        false,
        &mut load_state,
    );
    load_state.active_paths.pop();
    if let Some(error) = load_state.error {
        result.diagnostics.push(css_modules_import_diagnostic(
            source,
            options,
            error,
        ));
    }
    result
}

fn css_modules_import_limit_result(
    source: &str,
    options: &StyleCompileOptions,
    error: Option<CssModulesImportError>,
) -> CssModulesCompileResult {
    CssModulesCompileResult {
        code: String::new(),
        raw_modules: BTreeMap::new(),
        modules: BTreeMap::new(),
        diagnostics: error
            .map(|error| css_modules_import_diagnostic(source, options, error))
            .into_iter()
            .collect(),
        has_prepended_css: false,
    }
}

fn css_modules_import_diagnostic(
    source: &str,
    options: &StyleCompileOptions,
    error: CssModulesImportError,
) -> Diagnostic {
    Diagnostic::error("VUEC_STYLE_MODULE_LIMIT", error.message).with_span(Some(style_source_span(
        options,
        0,
        first_span_end(source),
    )))
}

pub(crate) fn compile_css_modules_file(
    source: &str,
    hash_source: &str,
    options: &StyleCompileOptions,
    filename: String,
    scope_behaviour: CssModulesScopeBehaviour,
    imported_dependency: bool,
    load_state: &mut CssModulesImportState,
) -> CssModulesCompileResult {
    let mut context = CssModulesContext::new(
        options,
        filename,
        hash_source.to_string(),
        scope_behaviour,
        imported_dependency,
        load_state,
    );
    let source = prepare_css_module_values(source, &mut context);
    let code = rewrite_css_modules_items(&source, &mut context, CssBlockContext::Root, false);
    let has_prepended_css = !context.prepended_css.is_empty();
    let code = context.finish_code(code);
    let raw_modules = context.raw_modules();
    let modules = context.modules();
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
    pub(crate) imported_modules: BTreeMap<String, Arc<CssModulesCompileResult>>,
    pub(crate) prepended_css_has_nested_import: bool,
    pub(crate) value_placeholders: BTreeMap<String, String>,
    pub(crate) value_placeholder_modules: BTreeMap<String, String>,
    pub(crate) next_value_placeholder: usize,
    pub(crate) prepended_paths: BTreeSet<String>,
    pub(crate) prepended_css: Vec<String>,
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) load_state: &'a mut CssModulesImportState,
}

impl<'a> CssModulesContext<'a> {
    pub(crate) fn new(
        options: &'a StyleCompileOptions,
        filename: String,
        hash_source: String,
        scope_behaviour: CssModulesScopeBehaviour,
        imported_dependency: bool,
        load_state: &'a mut CssModulesImportState,
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
            load_state,
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
        replace_css_module_value_symbols(&source, &self.value_placeholders)
    }

    pub(crate) fn load_imported_module(
        &mut self,
        import: &str,
    ) -> Option<Arc<CssModulesCompileResult>> {
        if self.load_state.error.is_some() {
            return None;
        }
        let resolved = resolve_css_module_import(import, &self.filename, self.load_state)?;
        let normalized = normalize_dependency_path(&resolved.path);
        if let Some(result) = self.imported_modules.get(&normalized) {
            return Some(Arc::clone(result));
        }
        if self
            .load_state
            .active_paths
            .iter()
            .any(|active| active == &resolved.path)
        {
            return None;
        }
        let imported_depth = self.load_state.active_paths.len().saturating_sub(1);
        if imported_depth >= self.load_state.limits.max_depth {
            self.load_state.fail(format!(
                "CSS Modules imports exceed the maximum depth of {}",
                self.load_state.limits.max_depth
            ));
            return None;
        }
        let source = self.load_state.read_module(&resolved.path)?;
        let normalized_source = normalize_style_output(&source);
        self.load_state.active_paths.push(resolved.path.clone());
        let result = Arc::new(compile_css_modules_file(
            &normalized_source,
            &source,
            self.options,
            resolved.logical_filename,
            self.scope_behaviour,
            true,
            self.load_state,
        ));
        self.load_state.active_paths.pop();
        if self.load_state.error.is_some() {
            return None;
        }
        if self.prepended_paths.insert(normalized.clone()) && !result.code.is_empty() {
            if result.has_prepended_css {
                self.prepended_css_has_nested_import = true;
            }
            self.prepended_css.push(result.code.clone());
        }
        self.imported_modules
            .insert(normalized, Arc::clone(&result));
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
