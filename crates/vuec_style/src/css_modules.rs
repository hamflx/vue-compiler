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

pub(crate) fn prepare_css_module_values(
    source: &str,
    context: &mut CssModulesContext<'_>,
) -> String {
    let mut output = String::with_capacity(source.len());
    let mut replacements = BTreeMap::new();
    let mut exports = BTreeMap::new();
    let mut import_index = 0usize;
    let mut index = 0usize;
    let mut drop_leading_whitespace = false;
    while index < source.len() {
        let Some(ch) = source[index..].chars().next() else {
            break;
        };
        if drop_leading_whitespace && ch.is_whitespace() {
            index += ch.len_utf8();
            continue;
        }
        drop_leading_whitespace = false;
        if source[index..].starts_with("/*") {
            let Some(end_offset) = source[index + 2..].find("*/") else {
                output.push_str(&source[index..]);
                break;
            };
            let end = index + 2 + end_offset + 2;
            output.push_str(&source[index..end]);
            index = end;
            continue;
        }
        if source[index..].starts_with(['\'', '"']) {
            let end = skip_css_string(source, index);
            output.push_str(&source[index..end]);
            index = end;
            continue;
        }
        if source[index..].starts_with("@value")
            && css_module_value_keyword_boundary(source, index + "@value".len())
        {
            if let Some(end) = css_module_value_statement_end(source, index + "@value".len()) {
                let statement = &source[index..end];
                if let Some(import) = parse_css_module_value_import_statement(statement) {
                    if register_css_module_value_import(
                        import,
                        context,
                        &mut replacements,
                        &mut exports,
                        &mut import_index,
                    ) {
                        if output.trim().is_empty() {
                            output.clear();
                            drop_leading_whitespace = true;
                        }
                        index = end;
                        continue;
                    }
                } else if let Some(value) =
                    parse_css_module_local_value_statement(statement, &replacements, &exports)
                {
                    if output.trim().is_empty() {
                        output.clear();
                        drop_leading_whitespace = true;
                    }
                    replacements.insert(value.name.clone(), value.replacement.clone());
                    exports.insert(value.name.clone(), value.export.clone());
                    context.set_raw_export_values(&value.name, vec![value.export]);
                    index = end;
                    continue;
                }
            }
        }
        output.push(ch);
        index += ch.len_utf8();
    }
    replace_css_module_values(&output, &replacements)
}

pub(crate) fn css_module_value_keyword_boundary(source: &str, index: usize) -> bool {
    source[index..]
        .chars()
        .next()
        .is_none_or(|ch| !is_css_module_identifier_continue(ch))
}

pub(crate) fn css_module_value_statement_end(source: &str, mut index: usize) -> Option<usize> {
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    while index < source.len() {
        if source[index..].starts_with("/*") {
            let end_offset = source[index + 2..].find("*/")?;
            index += 2 + end_offset + 2;
            continue;
        }
        if source[index..].starts_with(['\'', '"']) {
            index = skip_css_string(source, index);
            continue;
        }
        let ch = source[index..].chars().next()?;
        match ch {
            '(' => paren_depth += 1,
            ')' if paren_depth > 0 => paren_depth -= 1,
            '[' => bracket_depth += 1,
            ']' if bracket_depth > 0 => bracket_depth -= 1,
            ';' if paren_depth == 0 && bracket_depth == 0 => return Some(index + ch.len_utf8()),
            '{' if paren_depth == 0 && bracket_depth == 0 => return None,
            _ => {}
        }
        index += ch.len_utf8();
    }
    None
}

pub(crate) fn skip_css_string(source: &str, start: usize) -> usize {
    let Some(quote) = source[start..].chars().next() else {
        return start;
    };
    let mut index = start + quote.len_utf8();
    while index < source.len() {
        let Some(ch) = source[index..].chars().next() else {
            break;
        };
        index += ch.len_utf8();
        if ch == '\\' {
            if index < source.len() {
                index += source[index..].chars().next().map_or(0, char::len_utf8);
            }
            continue;
        }
        if ch == quote {
            return index;
        }
    }
    source.len()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CssModuleLocalValue {
    pub(crate) name: String,
    pub(crate) replacement: String,
    pub(crate) export: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CssModuleValueImport<'a> {
    pub(crate) import: &'a str,
    pub(crate) specs: Vec<CssModuleValueImportSpec<'a>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CssModuleValueImportSpec<'a> {
    pub(crate) remote: &'a str,
    pub(crate) local: &'a str,
}

pub(crate) fn parse_css_module_local_value_statement(
    statement: &str,
    replacements: &BTreeMap<String, String>,
    exports: &BTreeMap<String, String>,
) -> Option<CssModuleLocalValue> {
    let body = statement.strip_prefix("@value")?.strip_suffix(';')?.trim();
    let colon = find_top_level_colon(body)?;
    let name = body[..colon].trim();
    let value = body[colon + 1..].trim();
    if !is_css_module_value_name(name) || value.is_empty() {
        return None;
    }
    Some(CssModuleLocalValue {
        name: name.to_string(),
        replacement: replace_css_module_values(value, replacements),
        export: replace_css_module_values(value, exports),
    })
}

pub(crate) fn parse_css_module_value_import_statement(
    statement: &str,
) -> Option<CssModuleValueImport<'_>> {
    let body = statement.strip_prefix("@value")?.strip_suffix(';')?.trim();
    if find_top_level_colon(body).is_some() {
        return None;
    }
    let from = find_css_module_value_from_keyword(body)?;
    let specs = body[..from].trim();
    let import = body[from + "from".len()..].trim();
    if specs.is_empty() || import.is_empty() {
        return None;
    }
    let specs = split_selector_list(specs)
        .into_iter()
        .map(|spec| parse_css_module_value_import_spec(spec.trim()))
        .collect::<Option<Vec<_>>>()?;
    (!specs.is_empty()).then_some(CssModuleValueImport { import, specs })
}

pub(crate) fn find_css_module_value_from_keyword(source: &str) -> Option<usize> {
    let mut index = 0usize;
    while index < source.len() {
        if source[index..].starts_with("/*") {
            let end_offset = source[index + 2..].find("*/")?;
            index += 2 + end_offset + 2;
            continue;
        }
        if source[index..].starts_with(['\'', '"']) {
            index = skip_css_string(source, index);
            continue;
        }
        if source[index..].starts_with("from")
            && source[..index]
                .chars()
                .next_back()
                .is_none_or(|ch| !is_css_module_identifier_continue(ch))
            && css_module_value_keyword_boundary(source, index + "from".len())
        {
            return Some(index);
        }
        let ch = source[index..].chars().next()?;
        index += ch.len_utf8();
    }
    None
}

pub(crate) fn parse_css_module_value_import_spec(
    spec: &str,
) -> Option<CssModuleValueImportSpec<'_>> {
    let tokens = spec.split_whitespace().collect::<Vec<_>>();
    match tokens.as_slice() {
        [name] if is_css_module_value_name(name) => Some(CssModuleValueImportSpec {
            remote: name,
            local: name,
        }),
        [remote, keyword, local]
            if keyword.eq_ignore_ascii_case("as")
                && is_css_module_value_name(remote)
                && is_css_module_value_name(local) =>
        {
            Some(CssModuleValueImportSpec { remote, local })
        }
        _ => None,
    }
}

pub(crate) fn register_css_module_value_import(
    import: CssModuleValueImport<'_>,
    context: &mut CssModulesContext<'_>,
    replacements: &mut BTreeMap<String, String>,
    exports: &mut BTreeMap<String, String>,
    import_index: &mut usize,
) -> bool {
    let Some(result) = context.load_imported_module(import.import) else {
        return false;
    };
    for spec in import.specs {
        let (replacement, export) = if let Some(value) = result.raw_modules.get(spec.remote) {
            (value.clone(), value.clone())
        } else {
            (
                format!("i__const_{}_{}", spec.local, *import_index),
                "undefined".to_string(),
            )
        };
        let replacement = context.import_value_placeholder(replacement, export.clone());
        replacements.insert(spec.local.to_string(), replacement);
        exports.insert(spec.local.to_string(), export.clone());
        context.set_raw_export_values(spec.local, vec![export]);
        *import_index += 1;
    }
    true
}

pub(crate) fn is_css_module_value_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    is_css_module_identifier_start(first) && chars.all(is_css_module_identifier_continue)
}

pub(crate) fn replace_css_module_values(source: &str, values: &BTreeMap<String, String>) -> String {
    if values.is_empty() {
        return source.to_string();
    }
    let mut names = values.keys().map(String::as_str).collect::<Vec<_>>();
    names.sort_by_key(|name| std::cmp::Reverse(name.len()));
    let mut output = String::with_capacity(source.len());
    let mut index = 0usize;
    while index < source.len() {
        if source[index..].starts_with("/*") {
            let Some(end_offset) = source[index + 2..].find("*/") else {
                output.push_str(&source[index..]);
                break;
            };
            let end = index + 2 + end_offset + 2;
            output.push_str(&source[index..end]);
            index = end;
            continue;
        }
        if let Some(name) = names
            .iter()
            .copied()
            .find(|name| css_module_value_matches_at(source, index, name))
        {
            output.push_str(
                values
                    .get(name)
                    .expect("value name came from map keys")
                    .as_str(),
            );
            index += name.len();
            continue;
        }
        let Some(ch) = source[index..].chars().next() else {
            break;
        };
        output.push(ch);
        index += ch.len_utf8();
    }
    output
}

pub(crate) fn css_module_value_matches_at(source: &str, index: usize, name: &str) -> bool {
    if !source[index..].starts_with(name) {
        return false;
    }
    let before_is_ident = source[..index]
        .chars()
        .next_back()
        .is_some_and(is_css_module_identifier_continue);
    if before_is_ident {
        return false;
    }
    let end = index + name.len();
    source[end..]
        .chars()
        .next()
        .is_none_or(|ch| !is_css_module_identifier_continue(ch))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CssModulesScopeBehaviour {
    Local,
    Global,
}

impl CssModulesScopeBehaviour {
    pub(crate) fn from_options(
        scope_behaviour: &str,
        global_module_paths: &[String],
        filename: &str,
    ) -> Self {
        if scope_behaviour.eq_ignore_ascii_case("global")
            || css_module_filename_matches_global_pattern(filename, global_module_paths)
        {
            Self::Global
        } else {
            Self::Local
        }
    }
}

pub(crate) fn css_module_filename_matches_global_pattern(
    filename: &str,
    patterns: &[String],
) -> bool {
    if patterns.is_empty() {
        return false;
    }
    let normalized = filename.replace('\\', "/");
    patterns.iter().any(|pattern| {
        regex::Regex::new(pattern)
            .map(|compiled| compiled.is_match(filename) || compiled.is_match(&normalized))
            .unwrap_or(false)
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CssModulesLocalsConvention {
    AsIs,
    CamelCase,
    CamelCaseOnly,
    Dashes,
    DashesOnly,
}

impl CssModulesLocalsConvention {
    pub(crate) fn from_option(value: &str) -> Self {
        match value {
            "camelCase" | "camel-case" => Self::CamelCase,
            "camelCaseOnly" | "camel-case-only" => Self::CamelCaseOnly,
            "dashes" => Self::Dashes,
            "dashesOnly" | "dashes-only" => Self::DashesOnly,
            _ => Self::AsIs,
        }
    }
}

pub(crate) fn rewrite_css_modules_items(
    source: &str,
    context: &mut CssModulesContext<'_>,
    block_context: CssBlockContext,
    native_nested_rule: bool,
) -> String {
    let mut output = String::new();
    let mut cursor = 0usize;
    while cursor < source.len() {
        let output_len_before_whitespace = output.len();
        let whitespace_start = cursor;
        cursor = skip_css_whitespace(source, cursor);
        if cursor > whitespace_start {
            push_normalized_css_whitespace(&mut output, &source[whitespace_start..cursor]);
        }
        if cursor >= source.len() {
            break;
        }
        if source[cursor..].starts_with("/*") {
            let Some(end_offset) = source[cursor + 2..].find("*/") else {
                output.push_str(&source[cursor..]);
                break;
            };
            let end = cursor + 2 + end_offset + 2;
            output.push_str(&source[cursor..end]);
            cursor = end;
            continue;
        }

        let Some((delimiter, delimiter_ch)) = find_next_css_delimiter(source, cursor) else {
            output.push_str(&source[cursor..]);
            break;
        };
        let raw_prelude = &source[cursor..delimiter];
        let prelude_end = raw_prelude.trim_end().len();
        let prelude = raw_prelude[..prelude_end].trim();
        let brace_spacing = &raw_prelude[prelude_end..];
        if delimiter_ch == ';' {
            output.push_str(prelude);
            output.push(';');
            cursor = delimiter + 1;
            continue;
        }

        let Some(close) = find_matching_brace(source, delimiter) else {
            output.push_str(&source[cursor..]);
            break;
        };
        let body = &source[delimiter + 1..close];
        let compose_local_names = css_module_composable_local_names(prelude, context);
        if let Some(import) = parse_css_module_import_prelude(prelude) {
            output.truncate(output_len_before_whitespace);
            register_css_module_icss_imports(import, body, context);
            cursor = skip_css_whitespace(source, close + 1);
            continue;
        }
        if prelude == ":export" {
            output.truncate(output_len_before_whitespace);
            register_css_module_icss_exports(body, context);
            cursor = skip_css_whitespace(source, close + 1);
            continue;
        }
        let rewritten_prelude = if prelude.starts_with('@') {
            rewrite_css_module_at_rule_prelude(prelude, context)
        } else {
            rewrite_css_modules_prelude(prelude, context, block_context)
        };
        output.push_str(&rewritten_prelude);
        output.push_str(brace_spacing);
        output.push('{');
        if prelude.starts_with('@') {
            let next_context = if is_keyframes_at_rule(prelude) {
                CssBlockContext::Keyframes
            } else {
                CssBlockContext::Container
            };
            let rewritten_body =
                rewrite_css_modules_items(body, context, next_context, native_nested_rule);
            if css_block_contains_style_rules(&rewritten_body)
                || css_block_contains_at_rule_with_style_rules(&rewritten_body)
            {
                output.push('\n');
                output.push_str(rewritten_body.trim());
                output.push('\n');
            } else {
                output.push_str(&rewritten_body);
            }
        } else {
            output.push_str(&rewrite_css_module_rule_body(
                prelude,
                body,
                context,
                block_context,
                &compose_local_names,
                delimiter + 1,
                native_nested_rule,
            ));
        }
        output.push('}');
        cursor = close + 1;
    }
    output
}

pub(crate) fn rewrite_css_module_rule_body(
    prelude: &str,
    body: &str,
    context: &mut CssModulesContext<'_>,
    block_context: CssBlockContext,
    compose_local_names: &[String],
    body_offset: usize,
    native_nested_rule: bool,
) -> String {
    if !css_block_has_nested_block(body) {
        return rewrite_css_module_declarations(
            prelude,
            body,
            context,
            block_context,
            compose_local_names,
            body_offset,
            native_nested_rule,
        );
    }

    let mut output = String::new();
    let mut declarations = String::new();
    let mut declarations_offset = None;
    let mut cursor = 0usize;
    while cursor < body.len() {
        let whitespace_start = cursor;
        cursor = skip_css_whitespace(body, cursor);
        if cursor > whitespace_start {
            declarations_offset.get_or_insert(body_offset + whitespace_start);
            push_normalized_css_whitespace(&mut declarations, &body[whitespace_start..cursor]);
        }
        if cursor >= body.len() {
            break;
        }
        if body[cursor..].starts_with("/*") {
            let Some(end_offset) = body[cursor + 2..].find("*/") else {
                declarations_offset.get_or_insert(body_offset + cursor);
                declarations.push_str(&body[cursor..]);
                break;
            };
            let end = cursor + 2 + end_offset + 2;
            declarations_offset.get_or_insert(body_offset + cursor);
            declarations.push_str(&body[cursor..end]);
            cursor = end;
            continue;
        }

        let Some((delimiter, delimiter_ch)) = find_next_css_delimiter(body, cursor) else {
            declarations_offset.get_or_insert(body_offset + cursor);
            declarations.push_str(&body[cursor..]);
            break;
        };
        let raw_prelude = &body[cursor..delimiter];
        let prelude_end = raw_prelude.trim_end().len();
        let nested_prelude = raw_prelude[..prelude_end].trim();
        let brace_spacing = &raw_prelude[prelude_end..];
        if delimiter_ch == ';' {
            declarations_offset.get_or_insert(body_offset + cursor);
            declarations.push_str(nested_prelude);
            declarations.push(';');
            cursor = delimiter + 1;
            continue;
        }

        let Some(close) = find_matching_brace(body, delimiter) else {
            declarations.push_str(&body[cursor..]);
            break;
        };
        if css_prelude_is_block_declaration(nested_prelude) {
            let end = css_block_declaration_end(body, close);
            declarations_offset.get_or_insert(body_offset + cursor);
            declarations.push_str(&body[cursor..end]);
            cursor = end;
            continue;
        }

        flush_css_module_nested_declarations(
            &mut output,
            &mut declarations,
            context,
            prelude,
            compose_local_names,
            declarations_offset.take().unwrap_or(body_offset),
            native_nested_rule,
            true,
        );

        let nested_body = &body[delimiter + 1..close];
        let mut block = String::new();
        if nested_prelude.starts_with('@') {
            let rewritten_prelude = rewrite_css_module_at_rule_prelude(nested_prelude, context);
            let next_context = if is_keyframes_at_rule(nested_prelude) {
                CssBlockContext::Keyframes
            } else {
                CssBlockContext::Container
            };
            let nested_rewritten =
                rewrite_css_modules_items(nested_body, context, next_context, true);
            block.push_str(&rewritten_prelude);
            block.push_str(brace_spacing);
            block.push('{');
            if css_block_contains_style_rules(&nested_rewritten)
                || css_block_contains_at_rule_with_style_rules(&nested_rewritten)
            {
                block.push('\n');
                block.push_str(nested_rewritten.trim());
                block.push('\n');
            } else {
                block.push_str(&nested_rewritten);
            }
            block.push('}');
        } else {
            let nested_compose_local_names =
                css_module_composable_local_names(nested_prelude, context);
            let rewritten_prelude =
                rewrite_css_modules_prelude(nested_prelude, context, block_context);
            block.push_str(&rewritten_prelude);
            block.push_str(brace_spacing);
            block.push('{');
            block.push_str(&rewrite_css_module_rule_body(
                nested_prelude,
                nested_body,
                context,
                block_context,
                &nested_compose_local_names,
                body_offset + delimiter + 1,
                true,
            ));
            block.push('}');
        }

        let block = normalize_style_output(&block);
        if output.is_empty() || !output.ends_with('\n') {
            output.push('\n');
        }
        output.push_str(&block);
        cursor = close + 1;
    }

    flush_css_module_nested_declarations(
        &mut output,
        &mut declarations,
        context,
        prelude,
        compose_local_names,
        declarations_offset.take().unwrap_or(body_offset),
        native_nested_rule,
        false,
    );
    if !output.is_empty() {
        output.push('\n');
    }
    output
}

pub(crate) fn flush_css_module_nested_declarations(
    output: &mut String,
    declarations: &mut String,
    context: &mut CssModulesContext<'_>,
    prelude: &str,
    compose_local_names: &[String],
    body_offset: usize,
    native_nested_rule: bool,
    separate_before_next_block: bool,
) {
    if declarations.is_empty() {
        return;
    }
    let rewritten = rewrite_css_module_declarations(
        prelude,
        declarations,
        context,
        CssBlockContext::Container,
        compose_local_names,
        body_offset,
        native_nested_rule,
    );
    output.push_str(rewritten.trim_end());
    if separate_before_next_block && !output.ends_with('\n') {
        output.push('\n');
    }
    declarations.clear();
}

pub(crate) fn rewrite_css_modules_prelude(
    prelude: &str,
    context: &mut CssModulesContext<'_>,
    block_context: CssBlockContext,
) -> String {
    if prelude.starts_with('@') || matches!(block_context, CssBlockContext::Keyframes) {
        return prelude.to_string();
    }
    split_selector_list(prelude)
        .into_iter()
        .map(|part| rewrite_css_module_selector(part.trim(), context))
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn rewrite_css_module_at_rule_prelude(
    prelude: &str,
    context: &mut CssModulesContext<'_>,
) -> String {
    let Some((name, params)) = parse_at_rule(prelude) else {
        return replace_css_module_import_symbols_in_text(prelude, context);
    };
    if !is_keyframes_name(name) {
        return replace_css_module_import_symbols_in_text(prelude, context);
    }
    let Some((local, global)) = css_module_keyframes_local_name(params, context) else {
        return format!(
            "@{name} {}",
            css_module_unwrap_global_keyframes_name(params)
        );
    };
    if global {
        return format!("@{name} {local}");
    }
    let scoped = context.scoped_name(local);
    context.register_local(local, &scoped);
    format!("@{name} {scoped}")
}

pub(crate) fn css_module_keyframes_local_name<'a>(
    params: &'a str,
    context: &CssModulesContext<'_>,
) -> Option<(&'a str, bool)> {
    if let Some(inner) = parse_css_module_keyframes_pseudo(params, ":global") {
        return Some((inner, true));
    }
    if let Some(inner) = parse_css_module_keyframes_pseudo(params, ":local") {
        return Some((inner, false));
    }
    let params = params.trim();
    (!params.is_empty() && context.is_local_default()).then_some((params, false))
}

pub(crate) fn parse_css_module_keyframes_pseudo<'a>(
    params: &'a str,
    pseudo: &str,
) -> Option<&'a str> {
    let params = params.trim();
    let inner = params.strip_prefix(pseudo)?.strip_suffix(')')?;
    let inner = inner.strip_prefix('(')?.trim();
    (!inner.is_empty()).then_some(inner)
}

pub(crate) fn css_module_unwrap_global_keyframes_name(params: &str) -> String {
    parse_css_module_keyframes_pseudo(params, ":global")
        .unwrap_or(params)
        .to_string()
}

pub(crate) fn rewrite_css_module_selector(
    selector: &str,
    context: &mut CssModulesContext<'_>,
) -> String {
    let mut output = String::new();
    let mut cursor = 0usize;
    let mut default_local = context.is_local_default();
    while cursor < selector.len() {
        if let Some(global) =
            find_pseudo_function_from(selector, &[":global", "::v-global"], cursor)
        {
            output.push_str(&rewrite_css_module_default_segment(
                &selector[cursor..global.start],
                context,
                default_local,
            ));
            if let Some((open, close)) = global.parens {
                output.push_str(&rewrite_css_module_default_segment(
                    selector[open + 1..close].trim(),
                    context,
                    false,
                ));
                cursor = close + 1;
                continue;
            }
            cursor = global.end;
            default_local = false;
            continue;
        }
        if let Some(local) = find_pseudo_function_from(selector, &[":local", "::v-local"], cursor) {
            output.push_str(&rewrite_css_module_default_segment(
                &selector[cursor..local.start],
                context,
                default_local,
            ));
            if let Some((open, close)) = local.parens {
                output.push_str(&rewrite_css_module_default_segment(
                    selector[open + 1..close].trim(),
                    context,
                    true,
                ));
                cursor = close + 1;
                continue;
            }
            cursor = local.end;
            default_local = true;
            continue;
        }
        output.push_str(&rewrite_css_module_default_segment(
            &selector[cursor..],
            context,
            default_local,
        ));
        break;
    }
    output
}

pub(crate) fn rewrite_css_module_default_segment(
    segment: &str,
    context: &mut CssModulesContext<'_>,
    local: bool,
) -> String {
    if !local {
        if context.export_globals {
            register_css_module_globals(segment, context);
        }
        return segment.to_string();
    }
    let mut output = String::new();
    let mut cursor = 0usize;
    while let Some(token) = find_next_css_module_selector_token(segment, cursor) {
        output.push_str(&segment[cursor..token.start]);
        if context.import_symbol_is_imported(token.name) {
            if let Some(replacement) = context.import_symbol_value(token.name) {
                if replacement.starts_with('.') || replacement.starts_with('#') {
                    output.push_str(replacement);
                } else {
                    output.push(token.sigil);
                    output.push_str(replacement);
                }
            } else {
                output.push(token.sigil);
                output.push_str(token.name);
            }
            cursor = token.end;
            continue;
        }
        if let Some(replacement) = context.value_placeholder_replacement(token.name) {
            if replacement.starts_with('.') || replacement.starts_with('#') {
                output.push_str(replacement);
            } else {
                output.push(token.sigil);
                output.push_str(replacement);
            }
            cursor = token.end;
            continue;
        }
        let scoped = context.scoped_name(token.name);
        context.register_local(token.name, &scoped);
        output.push(token.sigil);
        output.push_str(&scoped);
        cursor = token.end;
    }
    output.push_str(&segment[cursor..]);
    output
}

pub(crate) fn register_css_module_globals(segment: &str, context: &mut CssModulesContext<'_>) {
    let mut cursor = 0usize;
    while let Some(token) = find_next_css_module_selector_token(segment, cursor) {
        context.register_global(token.name);
        cursor = token.end;
    }
}

pub(crate) fn register_css_module_icss_exports(body: &str, context: &mut CssModulesContext<'_>) {
    let mut segment_start = 0usize;
    for semicolon in top_level_semicolons(body) {
        register_css_module_icss_export_segment(&body[segment_start..semicolon], context);
        segment_start = semicolon + 1;
    }
    register_css_module_icss_export_segment(&body[segment_start..], context);
}

pub(crate) fn register_css_module_icss_export_segment(
    segment: &str,
    context: &mut CssModulesContext<'_>,
) {
    let Some(colon) = find_top_level_colon(segment) else {
        return;
    };
    let key = segment[..colon].trim();
    let value = segment[colon + 1..].trim();
    if key.is_empty() {
        return;
    }
    context.set_raw_export_values(key, vec![replace_css_module_export_symbols(value, context)]);
}

pub(crate) fn parse_css_module_import_prelude(prelude: &str) -> Option<&str> {
    let inner = prelude.strip_prefix(":import(")?.strip_suffix(')')?.trim();
    (!inner.is_empty()).then_some(inner)
}

pub(crate) fn register_css_module_icss_imports(
    import: &str,
    body: &str,
    context: &mut CssModulesContext<'_>,
) {
    let Some(result) = context.load_imported_module(import) else {
        return;
    };
    let mut segment_start = 0usize;
    for semicolon in top_level_semicolons(body) {
        register_css_module_icss_import_segment(
            &body[segment_start..semicolon],
            &result.raw_modules,
            context,
        );
        segment_start = semicolon + 1;
    }
    register_css_module_icss_import_segment(&body[segment_start..], &result.raw_modules, context);
}

pub(crate) fn register_css_module_icss_import_segment(
    segment: &str,
    modules: &BTreeMap<String, String>,
    context: &mut CssModulesContext<'_>,
) {
    let Some(colon) = find_top_level_colon(segment) else {
        return;
    };
    let local = segment[..colon].trim();
    let remote = segment[colon + 1..].trim();
    if local.is_empty() || remote.is_empty() {
        return;
    }
    let symbol = modules
        .get(remote)
        .cloned()
        .map(CssModuleImportSymbol::Found)
        .unwrap_or(CssModuleImportSymbol::Missing);
    context.import_symbols.insert(local.to_string(), symbol);
}

pub(crate) fn rewrite_css_module_declarations(
    prelude: &str,
    body: &str,
    context: &mut CssModulesContext<'_>,
    block_context: CssBlockContext,
    compose_local_names: &[String],
    body_offset: usize,
    native_nested_rule: bool,
) -> String {
    if matches!(block_context, CssBlockContext::Keyframes) {
        return body.to_string();
    }

    let mut output = String::new();
    let nested_compose_message =
        native_nested_rule.then(|| css_module_nested_compose_message(prelude, body, context));
    let mut nested_compose_reported = false;
    let mut segment_start = 0usize;
    for semicolon in top_level_semicolons(body) {
        rewrite_css_module_declaration_segment(
            &body[segment_start..semicolon],
            context,
            prelude,
            compose_local_names,
            body_offset + segment_start,
            true,
            nested_compose_message.as_deref(),
            &mut nested_compose_reported,
            &mut output,
        );
        segment_start = semicolon + 1;
    }
    rewrite_css_module_declaration_segment(
        &body[segment_start..],
        context,
        prelude,
        compose_local_names,
        body_offset + segment_start,
        false,
        nested_compose_message.as_deref(),
        &mut nested_compose_reported,
        &mut output,
    );
    output
}

pub(crate) fn rewrite_css_module_declaration_segment(
    segment: &str,
    context: &mut CssModulesContext<'_>,
    prelude: &str,
    compose_local_names: &[String],
    segment_offset: usize,
    has_semicolon: bool,
    nested_compose_message: Option<&str>,
    nested_compose_reported: &mut bool,
    output: &mut String,
) {
    let Some(colon) = find_top_level_colon(segment) else {
        output.push_str(segment);
        if has_semicolon {
            output.push(';');
        }
        return;
    };
    let prop = segment[..colon].trim();
    if !prop.eq_ignore_ascii_case("composes") && !prop.eq_ignore_ascii_case("compose-with") {
        let segment = rewrite_css_module_animation_declaration(segment, context);
        output.push_str(&replace_css_module_import_symbols(&segment, context));
        if has_semicolon {
            output.push(';');
        }
        return;
    }

    if let Some(message) = nested_compose_message {
        if !*nested_compose_reported {
            context.push_compose_diagnostic(
                message.to_string(),
                segment_offset,
                segment_offset + segment.len(),
            );
            *nested_compose_reported = true;
        }
        return;
    }

    if compose_local_names.is_empty() {
        let message = css_module_invalid_compose_selector_message(prelude, context);
        context.push_compose_diagnostic(message, segment_offset, segment_offset + segment.len());
        return;
    }
    match css_module_composed_values(&segment[colon + 1..], context, segment_offset + colon + 1) {
        CssModuleComposeResolution::Values(composed_values) => {
            if composed_values.is_empty() {
                output.push_str(segment);
                if has_semicolon {
                    output.push(';');
                }
                return;
            }

            for local_name in compose_local_names {
                context.compose(local_name, composed_values.clone());
            }
        }
        CssModuleComposeResolution::Unsupported => {
            output.push_str(segment);
            if has_semicolon {
                output.push(';');
            }
        }
        CssModuleComposeResolution::Invalid {
            class_name,
            start,
            end,
        } => {
            context.push_compose_diagnostic(
                format!("referenced class name \"{class_name}\" in {prop} not found"),
                start,
                end,
            );
        }
    }
}

pub(crate) fn rewrite_css_module_animation_declaration(
    segment: &str,
    context: &mut CssModulesContext<'_>,
) -> String {
    let Some(colon) = find_top_level_colon(segment) else {
        return segment.to_string();
    };
    let prop = segment[..colon].trim();
    if !is_animation_name_property(prop) && !is_animation_property(prop) {
        return segment.to_string();
    }
    let value_start = colon + 1;
    let value = &segment[value_start..];
    let leading_value_whitespace = value
        .char_indices()
        .find_map(|(index, ch)| (!ch.is_whitespace()).then_some(index))
        .unwrap_or(value.len());
    let value_prefix = &value[..leading_value_whitespace];
    let value_body = &value[leading_value_whitespace..];
    let rewritten = rewrite_css_module_animation_value(value_body.trim(), context);

    let mut output = String::new();
    output.push_str(&segment[..value_start]);
    output.push_str(value_prefix);
    output.push_str(&rewritten);
    output
}

pub(crate) fn rewrite_css_module_animation_value(
    value: &str,
    context: &mut CssModulesContext<'_>,
) -> String {
    split_selector_list(value)
        .into_iter()
        .map(|part| rewrite_css_module_animation_part(part.trim(), context))
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn rewrite_css_module_animation_part(
    part: &str,
    context: &mut CssModulesContext<'_>,
) -> String {
    let mut parsed_keywords = BTreeMap::new();
    tokenize_css_module_animation_part(part)
        .into_iter()
        .map(|token| rewrite_css_module_animation_token(token, context, &mut parsed_keywords))
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn rewrite_css_module_animation_token(
    token: &str,
    context: &mut CssModulesContext<'_>,
    parsed_keywords: &mut BTreeMap<String, usize>,
) -> String {
    if let Some(global) = parse_css_module_animation_function(token, "global") {
        return global.to_string();
    }
    if let Some(local) = parse_css_module_animation_function(token, "local") {
        return context.scoped_local_value(local);
    }
    if let Some(replacement) = context.value_placeholder_replacement(token) {
        return replacement.to_string();
    }
    if !context.is_local_default()
        || context.import_symbol_is_imported(token)
        || !is_css_module_animation_identifier(token)
    {
        return token.to_string();
    }
    let lower = token.to_ascii_lowercase();
    if let Some(limit) = css_module_animation_keyword_limit(&lower) {
        let count = parsed_keywords.entry(lower).or_insert(0);
        let should_localize = *count >= limit;
        *count = count.saturating_add(1);
        if !should_localize {
            return token.to_string();
        }
    }
    context.scoped_local_value(token)
}

pub(crate) fn tokenize_css_module_animation_part(part: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let mut state = CssScannerState::Normal;
    let mut paren_depth = 0usize;
    let mut token_start = None;
    let mut index = 0usize;
    while index < part.len() {
        let Some(ch) = part[index..].chars().next() else {
            break;
        };
        match state {
            CssScannerState::Normal => {
                if ch.is_whitespace() && paren_depth == 0 {
                    if let Some(start) = token_start.take() {
                        tokens.push(&part[start..index]);
                    }
                    index += ch.len_utf8();
                    continue;
                }
                if token_start.is_none() {
                    token_start = Some(index);
                }
                match ch {
                    '\'' => state = CssScannerState::SingleQuote,
                    '"' => state = CssScannerState::DoubleQuote,
                    '(' => paren_depth += 1,
                    ')' if paren_depth > 0 => paren_depth -= 1,
                    _ => {}
                }
            }
            CssScannerState::SingleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < part.len() {
                        index += part[index..].chars().next().map_or(0, char::len_utf8);
                    }
                    continue;
                }
                if ch == '\'' {
                    state = CssScannerState::Normal;
                }
            }
            CssScannerState::DoubleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < part.len() {
                        index += part[index..].chars().next().map_or(0, char::len_utf8);
                    }
                    continue;
                }
                if ch == '"' {
                    state = CssScannerState::Normal;
                }
            }
            CssScannerState::BlockComment => {}
        }
        index += ch.len_utf8();
    }
    if let Some(start) = token_start {
        tokens.push(&part[start..]);
    }
    tokens
}

pub(crate) fn parse_css_module_animation_function<'a>(
    token: &'a str,
    name: &str,
) -> Option<&'a str> {
    let inner = token
        .strip_prefix(name)?
        .strip_prefix('(')?
        .strip_suffix(')')?
        .trim();
    (!inner.is_empty()).then_some(inner)
}

pub(crate) fn is_css_module_animation_identifier(token: &str) -> bool {
    let mut chars = token.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if first == '-' {
        let Some(second) = chars.next() else {
            return false;
        };
        if second.is_ascii_digit() {
            return false;
        }
        if !is_css_module_identifier_start(second) && second != '-' {
            return false;
        }
    } else if !is_css_module_identifier_start(first) {
        return false;
    }
    chars.all(is_css_module_identifier_continue)
}

pub(crate) fn is_css_module_identifier_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic() || !ch.is_ascii()
}

pub(crate) fn is_css_module_identifier_continue(ch: char) -> bool {
    is_css_module_identifier_start(ch) || ch.is_ascii_digit() || ch == '-'
}

pub(crate) fn css_module_animation_keyword_limit(value: &str) -> Option<usize> {
    match value {
        "normal" | "reverse" | "alternate" | "alternate-reverse" | "forwards" | "backwards"
        | "both" | "infinite" | "paused" | "running" | "ease" | "ease-in" | "ease-out"
        | "ease-in-out" | "linear" | "step-end" | "step-start" => Some(1),
        "none" | "initial" | "inherit" | "unset" | "revert" | "revert-layer" => Some(usize::MAX),
        _ => None,
    }
}

pub(crate) fn css_module_invalid_compose_selector_message(
    prelude: &str,
    context: &CssModulesContext<'_>,
) -> String {
    let selector = css_module_localized_selector_for_message(prelude, context);
    format!("composition is only allowed when selector is single :local class name not in \"{selector}\"")
}

pub(crate) fn css_module_nested_compose_message(
    prelude: &str,
    body: &str,
    context: &CssModulesContext<'_>,
) -> String {
    let selector = css_module_localized_selector_for_message(prelude, context);
    let mut body = css_module_nested_compose_message_body(body);
    if !body.ends_with(';') {
        body.push(';');
    }
    format!("composition is not allowed in nested rule \n\n{selector} {{ {body}\n}}")
}

pub(crate) fn css_module_nested_compose_message_body(body: &str) -> String {
    let mut output = Vec::new();
    let mut segment_start = 0usize;
    for semicolon in top_level_semicolons(body) {
        let segment = css_module_nested_compose_message_segment(&body[segment_start..semicolon]);
        if !segment.is_empty() {
            output.push(format!("{segment};"));
        }
        segment_start = semicolon + 1;
    }
    let segment = css_module_nested_compose_message_segment(&body[segment_start..]);
    if !segment.is_empty() {
        output.push(segment);
    }
    output.join(" ")
}

pub(crate) fn css_module_nested_compose_message_segment(segment: &str) -> String {
    let segment = normalize_style_output(segment).trim().to_string();
    let Some(colon) = find_top_level_colon(&segment) else {
        return segment;
    };
    let prop = segment[..colon].trim();
    if !prop.eq_ignore_ascii_case("composes") && !prop.eq_ignore_ascii_case("compose-with") {
        return segment;
    }
    let value = css_module_nested_compose_message_value(segment[colon + 1..].trim());
    format!("{prop}: {value}")
}

pub(crate) fn css_module_nested_compose_message_value(value: &str) -> String {
    let mut output = Vec::new();
    for part in value.split(',') {
        let tokens = css_module_compose_tokens(part, value, 0);
        if let Some(from_index) = tokens.iter().position(|token| token.value == "from") {
            if from_index > 0
                && from_index + 2 == tokens.len()
                && tokens[from_index + 1].value == "global"
            {
                output.push(
                    tokens[..from_index]
                        .iter()
                        .map(|token| {
                            if parse_css_module_global_compose(token.value).is_some() {
                                token.value.to_string()
                            } else {
                                format!("global({})", token.value)
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(" "),
                );
                continue;
            }
        }
        output.push(part.trim().to_string());
    }
    output.join(", ")
}

pub(crate) fn css_module_localized_selector_for_message(
    prelude: &str,
    context: &CssModulesContext<'_>,
) -> String {
    split_selector_list(prelude)
        .into_iter()
        .map(|selector| css_module_localized_selector_part_for_message(selector.trim(), context))
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn css_module_localized_selector_part_for_message(
    selector: &str,
    context: &CssModulesContext<'_>,
) -> String {
    if !context.is_local_default() {
        return selector.to_string();
    }
    css_module_localized_selector_segment_for_message(selector, true)
}

pub(crate) fn css_module_localized_selector_segment_for_message(
    selector: &str,
    mut default_local: bool,
) -> String {
    let mut output = String::new();
    let mut cursor = 0usize;
    while cursor < selector.len() {
        if let Some(global) =
            find_pseudo_function_from(selector, &[":global", "::v-global"], cursor)
        {
            output.push_str(&css_module_localized_default_segment_for_message(
                &selector[cursor..global.start],
                default_local,
            ));
            if let Some((open, close)) = global.parens {
                output.push_str(":global(");
                output.push_str(selector[open + 1..close].trim());
                output.push(')');
                cursor = close + 1;
                continue;
            }
            output.push_str(&selector[global.start..global.end]);
            cursor = global.end;
            default_local = false;
            continue;
        }
        if let Some(local) = find_pseudo_function_from(selector, &[":local", "::v-local"], cursor) {
            output.push_str(&css_module_localized_default_segment_for_message(
                &selector[cursor..local.start],
                default_local,
            ));
            if let Some((open, close)) = local.parens {
                output.push_str(":local(");
                output.push_str(selector[open + 1..close].trim());
                output.push(')');
                cursor = close + 1;
                continue;
            }
            output.push_str(&selector[local.start..local.end]);
            cursor = local.end;
            default_local = true;
            continue;
        }
        output.push_str(&css_module_localized_default_segment_for_message(
            &selector[cursor..],
            default_local,
        ));
        break;
    }
    output
}

pub(crate) fn css_module_localized_default_segment_for_message(
    segment: &str,
    local: bool,
) -> String {
    if !local {
        return segment.to_string();
    }
    let mut output = String::new();
    let mut cursor = 0usize;
    while let Some(token) = find_next_css_module_selector_token(segment, cursor) {
        output.push_str(&segment[cursor..token.start]);
        output.push_str(":local(");
        output.push(token.sigil);
        output.push_str(token.name);
        output.push(')');
        cursor = token.end;
    }
    output.push_str(&segment[cursor..]);
    output
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum CssModuleComposeResolution {
    Values(Vec<String>),
    Unsupported,
    Invalid {
        class_name: String,
        start: usize,
        end: usize,
    },
}

pub(crate) fn unsupported_css_module_compose() -> CssModuleComposeResolution {
    CssModuleComposeResolution::Unsupported
}

pub(crate) fn invalid_css_module_compose(
    class_name: &str,
    start: usize,
    end: usize,
) -> CssModuleComposeResolution {
    CssModuleComposeResolution::Invalid {
        class_name: class_name.to_string(),
        start,
        end,
    }
}

pub(crate) fn css_module_composed_values(
    value: &str,
    context: &mut CssModulesContext<'_>,
    value_offset: usize,
) -> CssModuleComposeResolution {
    let mut composed = Vec::new();
    for part in value.split(',') {
        let tokens = css_module_compose_tokens(part, value, value_offset);
        if let Some(from_index) = tokens.iter().position(|token| token.value == "from") {
            if from_index == 0 || from_index + 2 != tokens.len() {
                return unsupported_css_module_compose();
            }
            let import = tokens[from_index + 1].value;
            if import == "global" {
                for token in &tokens[..from_index] {
                    push_unique_css_module_value(&mut composed, token.value.to_string());
                }
            } else {
                for token in &tokens[..from_index] {
                    let Some(values) =
                        css_module_external_composed_values(token.value, import, context)
                    else {
                        return unsupported_css_module_compose();
                    };
                    for value in values {
                        push_unique_css_module_value(&mut composed, value);
                    }
                }
            }
            continue;
        }
        for token in tokens {
            let class_name = token.value;
            if let Some(global) = parse_css_module_global_compose(class_name) {
                push_unique_css_module_value(&mut composed, global);
            } else if let Some(values) = context.raw_export_values(class_name) {
                for value in values {
                    push_unique_css_module_value(&mut composed, value);
                }
            } else if let Some(value) = context.value_placeholder_module_value(class_name) {
                push_unique_css_module_value(&mut composed, value.to_string());
            } else if let Some(value) = context.import_symbol_module_value(class_name) {
                push_unique_css_module_value(&mut composed, value);
            } else if class_name.starts_with('"') || class_name.starts_with('\'') {
                return unsupported_css_module_compose();
            } else {
                return invalid_css_module_compose(class_name, token.start, token.end);
            }
        }
    }
    CssModuleComposeResolution::Values(composed)
}

#[derive(Debug)]
pub(crate) struct CssModuleComposeToken<'a> {
    pub(crate) value: &'a str,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

pub(crate) fn css_module_compose_tokens<'a>(
    part: &'a str,
    value: &'a str,
    value_offset: usize,
) -> Vec<CssModuleComposeToken<'a>> {
    let part_offset = part.as_ptr() as usize - value.as_ptr() as usize;
    let mut tokens = Vec::new();
    let mut cursor = 0usize;
    while cursor < part.len() {
        cursor = skip_css_whitespace(part, cursor);
        if cursor >= part.len() {
            break;
        }
        let start = cursor;
        while cursor < part.len() {
            let Some(ch) = part[cursor..].chars().next() else {
                break;
            };
            if ch.is_whitespace() {
                break;
            }
            cursor += ch.len_utf8();
        }
        tokens.push(CssModuleComposeToken {
            value: &part[start..cursor],
            start: value_offset + part_offset + start,
            end: value_offset + part_offset + cursor,
        });
    }
    tokens
}

pub(crate) fn css_module_composable_local_names(
    prelude: &str,
    context: &CssModulesContext<'_>,
) -> Vec<String> {
    if prelude.starts_with('@') {
        return Vec::new();
    }
    let mut names = Vec::new();
    for selector in split_selector_list(prelude) {
        let Some(name) =
            css_module_composable_local_name(selector.trim(), context.is_local_default())
        else {
            return Vec::new();
        };
        names.push(name);
    }
    names
}

pub(crate) fn css_module_composable_local_name(
    selector: &str,
    default_local: bool,
) -> Option<String> {
    if let Some(local) = find_pseudo_function(selector, &[":local", "::v-local"]) {
        if local.start == 0 && local.end == selector.len() {
            let (open, close) = local.parens?;
            return css_module_single_class_selector_name(selector[open + 1..close].trim());
        }
    }
    if default_local {
        css_module_single_class_selector_name(selector)
    } else {
        None
    }
}

pub(crate) fn css_module_single_class_selector_name(selector: &str) -> Option<String> {
    let token = find_next_css_module_selector_token(selector, 0)?;
    (token.sigil == '.' && token.start == 0 && token.end == selector.len())
        .then(|| token.name.to_string())
}

pub(crate) fn css_module_external_composed_values(
    class_name: &str,
    import: &str,
    context: &mut CssModulesContext<'_>,
) -> Option<Vec<String>> {
    let result = context.load_imported_module(import)?;
    Some(
        result
            .raw_modules
            .get(class_name)
            .map(|value| value.split_whitespace().map(ToOwned::to_owned).collect())
            .unwrap_or_else(|| vec!["undefined".to_string()]),
    )
}

pub(crate) fn parse_css_module_global_compose(value: &str) -> Option<String> {
    let inner = value.strip_prefix("global(")?.strip_suffix(')')?;
    if inner.is_empty() {
        None
    } else {
        Some(inner.to_string())
    }
}

pub(crate) fn push_unique_css_module_value(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

pub(crate) fn replace_css_module_import_symbols(
    segment: &str,
    context: &CssModulesContext<'_>,
) -> String {
    if context.import_symbols.is_empty() {
        return segment.to_string();
    }
    let Some(colon) = find_top_level_colon(segment) else {
        return segment.to_string();
    };
    let value = &segment[colon + 1..];
    let replaced = replace_css_module_import_symbols_in_text(value, context);
    let mut output = String::new();
    output.push_str(&segment[..colon + 1]);
    output.push_str(&replaced);
    output
}

pub(crate) fn replace_css_module_import_symbols_in_text(
    source: &str,
    context: &CssModulesContext<'_>,
) -> String {
    if context.import_symbols.is_empty() {
        return source.to_string();
    }
    let symbols = context
        .import_symbols
        .iter()
        .filter_map(|(name, symbol)| match symbol {
            CssModuleImportSymbol::Found(value) => Some((name.clone(), value.clone())),
            CssModuleImportSymbol::Missing => None,
        })
        .collect::<BTreeMap<_, _>>();
    replace_css_module_value_symbols(source, &symbols)
}

pub(crate) fn replace_css_module_export_symbols(
    source: &str,
    context: &CssModulesContext<'_>,
) -> String {
    if context.import_symbols.is_empty() {
        return source.to_string();
    }
    let symbols = context
        .import_symbols
        .iter()
        .map(|(name, symbol)| {
            let value = match symbol {
                CssModuleImportSymbol::Found(value) => value.clone(),
                CssModuleImportSymbol::Missing => "undefined".to_string(),
            };
            (name.clone(), value)
        })
        .collect::<BTreeMap<_, _>>();
    replace_css_module_value_symbols(source, &symbols)
}

pub(crate) fn replace_css_module_value_symbols(
    value: &str,
    symbols: &BTreeMap<String, String>,
) -> String {
    let mut output = String::new();
    let mut cursor = 0usize;
    while cursor < value.len() {
        let Some((start, end, token)) = find_next_css_module_symbol(value, cursor) else {
            output.push_str(&value[cursor..]);
            break;
        };
        output.push_str(&value[cursor..start]);
        if let Some(replacement) = symbols.get(token) {
            output.push_str(replacement);
        } else {
            output.push_str(token);
        }
        cursor = end;
    }
    output
}

pub(crate) fn find_next_css_module_symbol(
    source: &str,
    mut cursor: usize,
) -> Option<(usize, usize, &str)> {
    while cursor < source.len() {
        let ch = source[cursor..].chars().next()?;
        if ch == '$' || ch == '_' || ch == '-' || ch.is_ascii_alphanumeric() {
            let start = cursor;
            cursor += ch.len_utf8();
            while cursor < source.len() {
                let next = source[cursor..].chars().next()?;
                if next == '_' || next == '-' || next.is_ascii_alphanumeric() {
                    cursor += next.len_utf8();
                } else {
                    break;
                }
            }
            return Some((start, cursor, &source[start..cursor]));
        }
        cursor += ch.len_utf8();
    }
    None
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CssModuleResolvedImport {
    pub(crate) path: PathBuf,
    pub(crate) logical_filename: String,
}

pub(crate) fn resolve_css_module_import(
    import: &str,
    filename: &str,
) -> Option<CssModuleResolvedImport> {
    let import = unquote_css_module_path(import);
    let import_path = Path::new(&import);
    let importer_dir = Path::new(filename)
        .parent()
        .unwrap_or_else(|| Path::new(""));
    if import_path.is_absolute() {
        return css_module_resolved_import(import_path.to_path_buf(), import_path.to_path_buf());
    }

    if is_relative_css_module_import(&import) {
        let logical = importer_dir.join(import_path);
        return css_module_resolved_import(logical.clone(), logical);
    }

    resolve_css_module_node_modules_import(&import, importer_dir).or_else(|| {
        let logical = importer_dir.join(import_path);
        css_module_resolved_import(logical.clone(), logical)
    })
}

pub(crate) fn css_module_resolved_import(
    path: PathBuf,
    logical_filename: PathBuf,
) -> Option<CssModuleResolvedImport> {
    if !path.is_file() {
        return None;
    }
    Some(CssModuleResolvedImport {
        path: std::fs::canonicalize(&path).unwrap_or(path),
        logical_filename: logical_filename.to_string_lossy().to_string(),
    })
}

pub(crate) fn is_relative_css_module_import(import: &str) -> bool {
    import.starts_with("./") || import.starts_with("../") || import == "." || import == ".."
}

pub(crate) fn resolve_css_module_node_modules_import(
    import: &str,
    importer_dir: &Path,
) -> Option<CssModuleResolvedImport> {
    let (package_name, subpath) = split_css_module_package_specifier(import)?;
    for dir in css_module_import_ancestor_dirs(importer_dir) {
        let package_dir = dir.join("node_modules").join(&package_name);
        if !package_dir.is_dir() {
            continue;
        }
        let path = if subpath.as_os_str().is_empty() {
            css_module_package_main_file(&package_dir)?
        } else {
            match css_module_package_exports_file(&package_dir, &subpath) {
                CssModulePackageExportsResolution::Resolved(path) => path,
                CssModulePackageExportsResolution::Blocked => return None,
                CssModulePackageExportsResolution::NoExports => package_dir.join(&subpath),
            }
        };
        let logical = importer_dir.join(import);
        if let Some(resolved) = css_module_resolved_import(path, logical) {
            return Some(resolved);
        }
    }
    None
}

pub(crate) fn split_css_module_package_specifier(import: &str) -> Option<(String, PathBuf)> {
    if import.is_empty() || import.starts_with('/') || import.starts_with('\\') {
        return None;
    }
    let parts = import.split('/').collect::<Vec<_>>();
    if parts.first().is_some_and(|part| part.is_empty()) {
        return None;
    }
    if import.starts_with('@') {
        let scope = *parts.first()?;
        let name = *parts.get(1)?;
        if scope.len() <= 1 || name.is_empty() {
            return None;
        }
        let package = format!("{scope}/{name}");
        let subpath = parts.iter().skip(2).collect::<PathBuf>();
        Some((package, subpath))
    } else {
        let package = (*parts.first()?).to_string();
        let subpath = parts.iter().skip(1).collect::<PathBuf>();
        Some((package, subpath))
    }
}

pub(crate) fn css_module_import_ancestor_dirs(importer_dir: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let start = if importer_dir.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        importer_dir.to_path_buf()
    };
    for ancestor in start.ancestors() {
        dirs.push(ancestor.to_path_buf());
    }
    dirs
}

pub(crate) fn css_module_package_main_file(package_dir: &Path) -> Option<PathBuf> {
    match css_module_package_exports_file(package_dir, Path::new("")) {
        CssModulePackageExportsResolution::Resolved(path) => return Some(path),
        CssModulePackageExportsResolution::Blocked => return None,
        CssModulePackageExportsResolution::NoExports => {}
    }
    let package_json = package_dir.join("package.json");
    if let Ok(source) = std::fs::read_to_string(package_json) {
        if let Ok(value) = serde_json::from_str::<CssModulePackageJson>(&source) {
            if let Some(main) = value.main {
                let candidate = package_dir.join(main);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    let index_css = package_dir.join("index.css");
    index_css.is_file().then_some(index_css)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CssModulePackageExportsResolution {
    NoExports,
    Resolved(PathBuf),
    Blocked,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CssModulePackageJson {
    #[serde(default)]
    pub(crate) main: Option<String>,
    #[serde(default)]
    pub(crate) exports: Option<CssModulePackageJsonValue>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum CssModulePackageJsonValue {
    String(String),
    Object(CssModulePackageJsonObject),
    Other,
}

impl CssModulePackageJsonValue {
    pub(crate) fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            Self::Object(_) | Self::Other => None,
        }
    }

    pub(crate) fn entries(&self) -> Option<&[(String, CssModulePackageJsonValue)]> {
        match self {
            Self::Object(object) => Some(&object.0),
            Self::String(_) | Self::Other => None,
        }
    }

    pub(crate) fn get(&self, key: &str) -> Option<&Self> {
        self.entries()?
            .iter()
            .find_map(|(entry_key, value)| (entry_key == key).then_some(value))
    }
}

impl From<serde_json::Value> for CssModulePackageJsonValue {
    fn from(value: serde_json::Value) -> Self {
        match value {
            serde_json::Value::String(value) => Self::String(value),
            serde_json::Value::Object(object) => Self::Object(CssModulePackageJsonObject(
                object
                    .into_iter()
                    .map(|(key, value)| (key, Self::from(value)))
                    .collect(),
            )),
            serde_json::Value::Null
            | serde_json::Value::Bool(_)
            | serde_json::Value::Number(_)
            | serde_json::Value::Array(_) => Self::Other,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct CssModulePackageJsonObject(Vec<(String, CssModulePackageJsonValue)>);

impl<'de> Deserialize<'de> for CssModulePackageJsonObject {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct OrderedObjectVisitor;

        impl<'de> serde::de::Visitor<'de> for OrderedObjectVisitor {
            type Value = CssModulePackageJsonObject;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an object")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut entries = Vec::with_capacity(map.size_hint().unwrap_or(0));
                while let Some((key, value)) =
                    map.next_entry::<String, CssModulePackageJsonValue>()?
                {
                    entries.push((key, value));
                }
                Ok(CssModulePackageJsonObject(entries))
            }
        }

        deserializer.deserialize_map(OrderedObjectVisitor)
    }
}

pub(crate) fn css_module_package_exports_file(
    package_dir: &Path,
    subpath: &Path,
) -> CssModulePackageExportsResolution {
    let package_json = package_dir.join("package.json");
    let Ok(source) = std::fs::read_to_string(package_json) else {
        return CssModulePackageExportsResolution::NoExports;
    };
    let Ok(value) = serde_json::from_str::<CssModulePackageJson>(&source) else {
        return CssModulePackageExportsResolution::NoExports;
    };
    let Some(exports) = value.exports.as_ref() else {
        return CssModulePackageExportsResolution::NoExports;
    };
    let target = if subpath.as_os_str().is_empty() {
        css_module_package_exports_root_target(exports)
    } else {
        let key = format!(
            "./{}",
            subpath
                .to_string_lossy()
                .replace('\\', "/")
                .trim_start_matches("./")
        );
        css_module_package_exports_subpath_target(exports, &key)
    };
    let Some(target) = target else {
        return CssModulePackageExportsResolution::Blocked;
    };
    let Some(path) = css_module_package_export_target(package_dir, &target) else {
        return CssModulePackageExportsResolution::Blocked;
    };
    CssModulePackageExportsResolution::Resolved(path)
}

pub(crate) fn css_module_package_exports_root_target(
    exports: &CssModulePackageJsonValue,
) -> Option<String> {
    css_module_package_export_target_value(exports).or_else(|| {
        exports
            .get(".")
            .and_then(css_module_package_export_target_value)
    })
}

pub(crate) fn css_module_package_exports_subpath_target(
    exports: &CssModulePackageJsonValue,
    key: &str,
) -> Option<String> {
    if let Some(target) = exports
        .get(key)
        .and_then(css_module_package_export_target_value)
    {
        return Some(target);
    }
    for (pattern, target) in exports.entries()? {
        let Some(capture) = css_module_package_export_pattern_capture(pattern, key) else {
            continue;
        };
        let target = css_module_package_export_target_value(target)?;
        return Some(target.replace('*', &capture));
    }
    None
}

pub(crate) fn css_module_package_export_target_value(
    value: &CssModulePackageJsonValue,
) -> Option<String> {
    if let Some(value) = value.as_str() {
        return Some(value.to_string());
    }
    for (condition, target) in value.entries()? {
        if matches!(condition.as_str(), "require" | "node" | "default") {
            if let Some(target) = css_module_package_export_target_value(target) {
                return Some(target);
            }
        }
    }
    None
}

pub(crate) fn css_module_package_export_pattern_capture(
    pattern: &str,
    key: &str,
) -> Option<String> {
    let star = pattern.find('*')?;
    let prefix = &pattern[..star];
    let suffix = &pattern[star + 1..];
    if !key.starts_with(prefix) || !key.ends_with(suffix) || key.len() < prefix.len() + suffix.len()
    {
        return None;
    }
    Some(key[prefix.len()..key.len() - suffix.len()].to_string())
}

pub(crate) fn css_module_package_export_target(
    package_dir: &Path,
    target: &str,
) -> Option<PathBuf> {
    if !target.starts_with("./") {
        return None;
    }
    Some(package_dir.join(target))
}

pub(crate) fn unquote_css_module_path(value: &str) -> String {
    let value = value.trim();
    if value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')))
    {
        value[1..value.len() - 1].to_string()
    } else {
        value.to_string()
    }
}

pub(crate) fn css_module_active_path(filename: &str) -> PathBuf {
    std::fs::canonicalize(filename).unwrap_or_else(|_| PathBuf::from(filename))
}

pub(crate) fn find_pseudo_function_from(
    selector: &str,
    names: &[&str],
    start: usize,
) -> Option<SelectorMatch> {
    find_pseudo_function(&selector[start..], names).map(|matched| SelectorMatch {
        start: start + matched.start,
        end: start + matched.end,
        parens: matched
            .parens
            .map(|(open, close)| (start + open, start + close)),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CssModuleSelectorToken<'a> {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) sigil: char,
    pub(crate) name: &'a str,
}

pub(crate) fn find_next_css_module_selector_token(
    source: &str,
    start: usize,
) -> Option<CssModuleSelectorToken<'_>> {
    let mut state = SelectorScannerState::Normal;
    let mut index = start;
    while index < source.len() {
        let ch = source[index..].chars().next()?;
        match state {
            SelectorScannerState::Normal => match ch {
                '\'' => state = SelectorScannerState::SingleQuote,
                '"' => state = SelectorScannerState::DoubleQuote,
                '[' => {
                    let Some(end) = find_matching_selector_bracket(source, index) else {
                        return None;
                    };
                    index = end + 1;
                    continue;
                }
                '.' | '#' => {
                    let name_start = index + 1;
                    let name_end = consume_css_module_class_name(source, name_start);
                    if name_end > name_start {
                        return Some(CssModuleSelectorToken {
                            start: index,
                            end: name_end,
                            sigil: ch,
                            name: &source[name_start..name_end],
                        });
                    }
                }
                _ => {}
            },
            SelectorScannerState::SingleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < source.len() {
                        index += source[index..].chars().next().map_or(0, char::len_utf8);
                    }
                    continue;
                }
                if ch == '\'' {
                    state = SelectorScannerState::Normal;
                }
            }
            SelectorScannerState::DoubleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < source.len() {
                        index += source[index..].chars().next().map_or(0, char::len_utf8);
                    }
                    continue;
                }
                if ch == '"' {
                    state = SelectorScannerState::Normal;
                }
            }
        }
        index += ch.len_utf8();
    }
    None
}

pub(crate) fn consume_css_module_class_name(source: &str, mut index: usize) -> usize {
    while index < source.len() {
        let Some(ch) = source[index..].chars().next() else {
            break;
        };
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            index += ch.len_utf8();
        } else {
            break;
        }
    }
    index
}

pub(crate) fn format_css_module_default_scoped_name(local: &str, css: &str) -> String {
    let selector = format!(".{local}");
    let index = css.find(&selector).unwrap_or(0);
    let line_number = css[..index].split(['\r', '\n']).count();
    let hash = css_module_default_hash(css);
    format!("_{local}_{hash}_{line_number}")
}

pub(crate) fn css_module_default_hash(css: &str) -> String {
    let codes = css.encode_utf16().collect::<Vec<_>>();
    let mut hash = 5381u32;
    for code in codes.iter().rev() {
        hash = hash.wrapping_mul(33) ^ (*code as u32);
    }
    let mut base36 = encode_base36_u32(hash);
    base36.truncate(5);
    base36
}

pub(crate) fn encode_base36_u32(mut value: u32) -> String {
    if value == 0 {
        return "0".into();
    }
    let mut digits = Vec::new();
    while value > 0 {
        let digit = value % 36;
        digits.push(char::from_digit(digit, 36).expect("base36 digit"));
        value /= 36;
    }
    digits.iter().rev().collect()
}

pub(crate) fn format_css_module_pattern(
    pattern: &str,
    filename: &str,
    local: &str,
    hash_prefix: &str,
) -> String {
    let file_stem = Path::new(filename)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("style");
    let mut output = pattern
        .replace("[name]", file_stem)
        .replace("[local]", local);
    output = replace_css_module_hash_patterns(&output, filename, local, hash_prefix);
    sanitize_css_module_generic_name(&output)
}

pub(crate) fn replace_css_module_hash_patterns(
    pattern: &str,
    filename: &str,
    local: &str,
    hash_prefix: &str,
) -> String {
    let mut output = String::new();
    let mut cursor = 0usize;
    while let Some(start_offset) = pattern[cursor..].find('[') {
        let start = cursor + start_offset;
        let Some(end_offset) = pattern[start + 1..].find(']') else {
            break;
        };
        let end = start + 1 + end_offset;
        let token = &pattern[start + 1..end];
        let replacement = css_module_hash_pattern_replacement(token, filename, local, hash_prefix);
        output.push_str(&pattern[cursor..start]);
        if let Some(replacement) = replacement {
            output.push_str(&replacement);
        } else {
            output.push_str(&pattern[start..=end]);
        }
        cursor = end + 1;
    }
    output.push_str(&pattern[cursor..]);
    output
}

pub(crate) fn css_module_hash_pattern_replacement(
    token: &str,
    filename: &str,
    local: &str,
    hash_prefix: &str,
) -> Option<String> {
    let parts = token.split(':').collect::<Vec<_>>();
    let (hash_index, digest_index, length_index) = match parts.as_slice() {
        ["hash"] | ["contenthash"] => (0usize, None, None),
        ["hash", _] | ["contenthash", _] => (0usize, Some(1usize), None),
        ["hash", _, _] | ["contenthash", _, _] => (0usize, Some(1usize), Some(2usize)),
        [_, "hash"] | [_, "contenthash"] => (1usize, None, None),
        [_, "hash", _] | [_, "contenthash", _] => (1usize, Some(2usize), None),
        [_, "hash", _, _] | [_, "contenthash", _, _] => (1usize, Some(2usize), Some(3usize)),
        _ => return None,
    };
    let algorithm = if hash_index == 0 {
        "xxhash64"
    } else {
        parts[0]
    };
    if !algorithm.eq_ignore_ascii_case("xxhash64") {
        return None;
    }
    let digest = digest_index.map(|index| parts[index]).unwrap_or("hex");
    let max_length = length_index.and_then(|index| parts[index].parse::<usize>().ok());
    Some(css_module_template_hash(
        filename,
        local,
        hash_prefix,
        digest,
        max_length,
    ))
}

pub(crate) fn css_module_template_hash(
    filename: &str,
    local: &str,
    hash_prefix: &str,
    digest: &str,
    max_length: Option<usize>,
) -> String {
    let relative = css_module_hash_resource_path(filename);
    let content = format!("{hash_prefix}{relative}\0{local}");
    let hash = xxhash64(content.as_bytes());
    let mut output = if digest.eq_ignore_ascii_case("base64") {
        base64_encode(&hash.to_be_bytes())
    } else {
        format!("{hash:016x}")
    };
    if let Some(max_length) = max_length {
        output.truncate(max_length);
    }
    output
}

pub(crate) fn css_module_hash_resource_path(filename: &str) -> String {
    let path = Path::new(filename);
    let relative = if path.is_absolute() {
        std::env::current_dir()
            .ok()
            .and_then(|cwd| path.strip_prefix(cwd).ok().map(Path::to_path_buf))
            .unwrap_or_else(|| path.to_path_buf())
    } else {
        path.to_path_buf()
    };
    relative.to_string_lossy().replace('\\', "/")
}

pub(crate) fn sanitize_css_module_generic_name(value: &str) -> String {
    let mut output = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || (ch as u32) >= 0x00a0 {
            output.push(ch);
        } else {
            output.push('-');
        }
    }
    if css_module_generic_name_needs_prefix(&output) {
        output.insert(0, '_');
    }
    output
}

pub(crate) fn css_module_generic_name_needs_prefix(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if first.is_ascii_digit() {
        return true;
    }
    if first != '-' {
        return false;
    }
    matches!(chars.next(), Some('-') | Some('0'..='9'))
}

pub(crate) fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::new();
    let mut cursor = 0usize;
    while cursor + 3 <= bytes.len() {
        let chunk = ((bytes[cursor] as u32) << 16)
            | ((bytes[cursor + 1] as u32) << 8)
            | bytes[cursor + 2] as u32;
        output.push(TABLE[((chunk >> 18) & 0x3f) as usize] as char);
        output.push(TABLE[((chunk >> 12) & 0x3f) as usize] as char);
        output.push(TABLE[((chunk >> 6) & 0x3f) as usize] as char);
        output.push(TABLE[(chunk & 0x3f) as usize] as char);
        cursor += 3;
    }
    let remaining = bytes.len() - cursor;
    if remaining == 1 {
        let chunk = (bytes[cursor] as u32) << 16;
        output.push(TABLE[((chunk >> 18) & 0x3f) as usize] as char);
        output.push(TABLE[((chunk >> 12) & 0x3f) as usize] as char);
        output.push('=');
        output.push('=');
    } else if remaining == 2 {
        let chunk = ((bytes[cursor] as u32) << 16) | ((bytes[cursor + 1] as u32) << 8);
        output.push(TABLE[((chunk >> 18) & 0x3f) as usize] as char);
        output.push(TABLE[((chunk >> 12) & 0x3f) as usize] as char);
        output.push(TABLE[((chunk >> 6) & 0x3f) as usize] as char);
        output.push('=');
    }
    output
}

pub(crate) fn xxhash64(input: &[u8]) -> u64 {
    const PRIME64_1: u64 = 11_400_714_785_074_694_791;
    const PRIME64_2: u64 = 14_029_467_366_897_019_727;
    const PRIME64_3: u64 = 1_609_587_929_392_839_161;
    const PRIME64_4: u64 = 9_650_029_242_287_828_579;
    const PRIME64_5: u64 = 2_870_177_450_012_600_261;

    let mut cursor = 0usize;
    let mut hash;
    if input.len() >= 32 {
        let mut v1 = PRIME64_1.wrapping_add(PRIME64_2);
        let mut v2 = PRIME64_2;
        let mut v3 = 0u64;
        let mut v4 = 0u64.wrapping_sub(PRIME64_1);
        while cursor + 32 <= input.len() {
            v1 = xxhash64_round(v1, read_u64_le(input, cursor));
            cursor += 8;
            v2 = xxhash64_round(v2, read_u64_le(input, cursor));
            cursor += 8;
            v3 = xxhash64_round(v3, read_u64_le(input, cursor));
            cursor += 8;
            v4 = xxhash64_round(v4, read_u64_le(input, cursor));
            cursor += 8;
        }
        hash = v1
            .rotate_left(1)
            .wrapping_add(v2.rotate_left(7))
            .wrapping_add(v3.rotate_left(12))
            .wrapping_add(v4.rotate_left(18));
        hash = xxhash64_merge_round(hash, v1);
        hash = xxhash64_merge_round(hash, v2);
        hash = xxhash64_merge_round(hash, v3);
        hash = xxhash64_merge_round(hash, v4);
    } else {
        hash = PRIME64_5;
    }

    hash = hash.wrapping_add(input.len() as u64);
    while cursor + 8 <= input.len() {
        let lane = xxhash64_round(0, read_u64_le(input, cursor));
        hash ^= lane;
        hash = hash
            .rotate_left(27)
            .wrapping_mul(PRIME64_1)
            .wrapping_add(PRIME64_4);
        cursor += 8;
    }
    if cursor + 4 <= input.len() {
        hash ^= (read_u32_le(input, cursor) as u64).wrapping_mul(PRIME64_1);
        hash = hash
            .rotate_left(23)
            .wrapping_mul(PRIME64_2)
            .wrapping_add(PRIME64_3);
        cursor += 4;
    }
    while cursor < input.len() {
        hash ^= (input[cursor] as u64).wrapping_mul(PRIME64_5);
        hash = hash.rotate_left(11).wrapping_mul(PRIME64_1);
        cursor += 1;
    }
    hash ^= hash >> 33;
    hash = hash.wrapping_mul(PRIME64_2);
    hash ^= hash >> 29;
    hash = hash.wrapping_mul(PRIME64_3);
    hash ^ (hash >> 32)
}

pub(crate) fn xxhash64_round(accumulator: u64, input: u64) -> u64 {
    const PRIME64_1: u64 = 11_400_714_785_074_694_791;
    const PRIME64_2: u64 = 14_029_467_366_897_019_727;
    accumulator
        .wrapping_add(input.wrapping_mul(PRIME64_2))
        .rotate_left(31)
        .wrapping_mul(PRIME64_1)
}

pub(crate) fn xxhash64_merge_round(accumulator: u64, value: u64) -> u64 {
    const PRIME64_1: u64 = 11_400_714_785_074_694_791;
    const PRIME64_4: u64 = 9_650_029_242_287_828_579;
    (accumulator ^ xxhash64_round(0, value))
        .wrapping_mul(PRIME64_1)
        .wrapping_add(PRIME64_4)
}

pub(crate) fn read_u64_le(input: &[u8], start: usize) -> u64 {
    u64::from_le_bytes(input[start..start + 8].try_into().expect("u64 lane"))
}

pub(crate) fn read_u32_le(input: &[u8], start: usize) -> u32 {
    u32::from_le_bytes(input[start..start + 4].try_into().expect("u32 lane"))
}

pub(crate) fn camel_case_css_module_key(value: &str) -> String {
    let mut output = String::new();
    let mut uppercase_next = false;
    for ch in value.chars() {
        if ch == '-' || ch == '_' {
            uppercase_next = true;
            continue;
        }
        if uppercase_next {
            for upper in ch.to_uppercase() {
                output.push(upper);
            }
            uppercase_next = false;
        } else {
            output.push(ch);
        }
    }
    output
}

pub(crate) fn dashes_css_module_key(value: &str) -> String {
    let mut output = String::new();
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '-' {
            output.push(ch);
            continue;
        }

        let mut dashes = String::from("-");
        while chars.next_if_eq(&'-').is_some() {
            dashes.push('-');
        }
        if let Some(next) = chars.next_if(|next| next.is_ascii_alphanumeric() || *next == '_') {
            for upper in next.to_uppercase() {
                output.push(upper);
            }
        } else {
            output.push_str(&dashes);
        }
    }
    output
}

pub(crate) fn skip_css_whitespace(source: &str, mut cursor: usize) -> usize {
    while cursor < source.len() {
        let Some(ch) = source[cursor..].chars().next() else {
            break;
        };
        if !ch.is_whitespace() {
            break;
        }
        cursor += ch.len_utf8();
    }
    cursor
}

pub(crate) fn push_normalized_css_whitespace(output: &mut String, whitespace: &str) {
    if whitespace.contains('\n') || whitespace.contains('\r') {
        output.push('\n');
    } else {
        output.push_str(whitespace);
    }
}

pub(crate) fn find_next_css_delimiter(source: &str, start: usize) -> Option<(usize, char)> {
    let mut state = CssScannerState::Normal;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut index = start;
    while index < source.len() {
        let ch = source[index..].chars().next()?;
        match state {
            CssScannerState::Normal => {
                if source[index..].starts_with("/*") {
                    state = CssScannerState::BlockComment;
                    index += 2;
                    continue;
                }
                match ch {
                    '\'' => state = CssScannerState::SingleQuote,
                    '"' => state = CssScannerState::DoubleQuote,
                    '(' => paren_depth += 1,
                    ')' if paren_depth > 0 => paren_depth -= 1,
                    '[' => bracket_depth += 1,
                    ']' if bracket_depth > 0 => bracket_depth -= 1,
                    '{' | ';' if paren_depth == 0 && bracket_depth == 0 => {
                        return Some((index, ch));
                    }
                    _ => {}
                }
            }
            CssScannerState::SingleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < source.len() {
                        index += source[index..].chars().next()?.len_utf8();
                    }
                    continue;
                }
                if ch == '\'' {
                    state = CssScannerState::Normal;
                }
            }
            CssScannerState::DoubleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < source.len() {
                        index += source[index..].chars().next()?.len_utf8();
                    }
                    continue;
                }
                if ch == '"' {
                    state = CssScannerState::Normal;
                }
            }
            CssScannerState::BlockComment => {
                if source[index..].starts_with("*/") {
                    state = CssScannerState::Normal;
                    index += 2;
                    continue;
                }
            }
        }
        index += ch.len_utf8();
    }
    None
}

pub(crate) fn find_matching_brace(source: &str, open: usize) -> Option<usize> {
    let mut state = CssScannerState::Normal;
    let mut depth = 0usize;
    let mut index = open;
    while index < source.len() {
        let ch = source[index..].chars().next()?;
        match state {
            CssScannerState::Normal => {
                if source[index..].starts_with("/*") {
                    state = CssScannerState::BlockComment;
                    index += 2;
                    continue;
                }
                match ch {
                    '\'' => state = CssScannerState::SingleQuote,
                    '"' => state = CssScannerState::DoubleQuote,
                    '{' => depth += 1,
                    '}' => {
                        depth = depth.saturating_sub(1);
                        if depth == 0 {
                            return Some(index);
                        }
                    }
                    _ => {}
                }
            }
            CssScannerState::SingleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < source.len() {
                        index += source[index..].chars().next()?.len_utf8();
                    }
                    continue;
                }
                if ch == '\'' {
                    state = CssScannerState::Normal;
                }
            }
            CssScannerState::DoubleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < source.len() {
                        index += source[index..].chars().next()?.len_utf8();
                    }
                    continue;
                }
                if ch == '"' {
                    state = CssScannerState::Normal;
                }
            }
            CssScannerState::BlockComment => {
                if source[index..].starts_with("*/") {
                    state = CssScannerState::Normal;
                    index += 2;
                    continue;
                }
            }
        }
        index += ch.len_utf8();
    }
    None
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CssScannerState {
    Normal,
    SingleQuote,
    DoubleQuote,
    BlockComment,
}

pub(crate) fn collect_scoped_keyframes(source: &str, short_id: &str) -> Vec<(String, String)> {
    let mut keyframes = Vec::new();
    collect_scoped_keyframes_in(source, short_id, &mut keyframes);
    keyframes
}

pub(crate) fn collect_scoped_keyframes_in(
    source: &str,
    short_id: &str,
    keyframes: &mut Vec<(String, String)>,
) {
    let mut cursor = 0usize;
    while cursor < source.len() {
        cursor = skip_css_whitespace(source, cursor);
        if cursor >= source.len() {
            break;
        }
        if source[cursor..].starts_with("/*") {
            let Some(end_offset) = source[cursor + 2..].find("*/") else {
                break;
            };
            cursor += 2 + end_offset + 2;
            continue;
        }
        let Some((delimiter, delimiter_ch)) = find_next_css_delimiter(source, cursor) else {
            break;
        };
        if delimiter_ch == ';' {
            cursor = delimiter + 1;
            continue;
        }
        let Some(close) = find_matching_brace(source, delimiter) else {
            break;
        };
        let prelude = source[cursor..delimiter].trim();
        if let Some((name, params)) = parse_at_rule(prelude) {
            if is_keyframes_name(name) && !params.ends_with(&format!("-{short_id}")) {
                let renamed = format!("{params}-{short_id}");
                if !keyframes.iter().any(|(raw, _)| raw == params) {
                    keyframes.push((params.to_string(), renamed));
                }
            } else {
                collect_scoped_keyframes_in(&source[delimiter + 1..close], short_id, keyframes);
            }
        } else {
            collect_scoped_keyframes_in(&source[delimiter + 1..close], short_id, keyframes);
        }
        cursor = close + 1;
    }
}

pub(crate) fn rewrite_at_rule_prelude(prelude: &str, keyframes: &[(String, String)]) -> String {
    let Some((name, params)) = parse_at_rule(prelude) else {
        return prelude.to_string();
    };
    if !is_keyframes_name(name) {
        return prelude.to_string();
    }
    let Some(renamed) = lookup_keyframe_name(params, keyframes) else {
        return prelude.to_string();
    };
    format!("@{name} {renamed}")
}

pub(crate) fn parse_at_rule(prelude: &str) -> Option<(&str, &str)> {
    let prelude = prelude.trim();
    let rest = prelude.strip_prefix('@')?;
    let name_end = rest
        .char_indices()
        .find_map(|(index, ch)| ch.is_whitespace().then_some(index))
        .unwrap_or(rest.len());
    Some((&rest[..name_end], rest[name_end..].trim()))
}

pub(crate) fn is_keyframes_at_rule(prelude: &str) -> bool {
    parse_at_rule(prelude)
        .map(|(name, _)| is_keyframes_name(name))
        .unwrap_or(false)
}

pub(crate) fn is_keyframes_name(name: &str) -> bool {
    name.ends_with("keyframes")
}

pub(crate) fn lookup_keyframe_name<'a>(
    name: &str,
    keyframes: &'a [(String, String)],
) -> Option<&'a String> {
    keyframes
        .iter()
        .find_map(|(raw, rewritten)| (raw == name).then_some(rewritten))
}

pub(crate) fn rewrite_animation_declarations(
    source: &str,
    keyframes: &[(String, String)],
) -> String {
    if keyframes.is_empty() {
        return source.to_string();
    }

    let mut output = String::new();
    let mut segment_start = 0usize;
    for semicolon in top_level_semicolons(source) {
        output.push_str(&rewrite_declaration_segment(
            &source[segment_start..semicolon],
            keyframes,
        ));
        output.push(';');
        segment_start = semicolon + 1;
    }
    output.push_str(&rewrite_declaration_segment(
        &source[segment_start..],
        keyframes,
    ));
    output
}

pub(crate) fn top_level_semicolons(source: &str) -> Vec<usize> {
    let mut semicolons = Vec::new();
    let mut state = CssScannerState::Normal;
    let mut paren_depth = 0usize;
    let mut index = 0usize;
    while index < source.len() {
        let Some(ch) = source[index..].chars().next() else {
            break;
        };
        match state {
            CssScannerState::Normal => {
                if source[index..].starts_with("/*") {
                    state = CssScannerState::BlockComment;
                    index += 2;
                    continue;
                }
                match ch {
                    '\'' => state = CssScannerState::SingleQuote,
                    '"' => state = CssScannerState::DoubleQuote,
                    '(' => paren_depth += 1,
                    ')' if paren_depth > 0 => paren_depth -= 1,
                    ';' if paren_depth == 0 => semicolons.push(index),
                    _ => {}
                }
            }
            CssScannerState::SingleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < source.len() {
                        index += source[index..].chars().next().map_or(0, char::len_utf8);
                    }
                    continue;
                }
                if ch == '\'' {
                    state = CssScannerState::Normal;
                }
            }
            CssScannerState::DoubleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < source.len() {
                        index += source[index..].chars().next().map_or(0, char::len_utf8);
                    }
                    continue;
                }
                if ch == '"' {
                    state = CssScannerState::Normal;
                }
            }
            CssScannerState::BlockComment => {
                if source[index..].starts_with("*/") {
                    state = CssScannerState::Normal;
                    index += 2;
                    continue;
                }
            }
        }
        index += ch.len_utf8();
    }
    semicolons
}

pub(crate) fn rewrite_declaration_segment(segment: &str, keyframes: &[(String, String)]) -> String {
    let Some(colon) = find_top_level_colon(segment) else {
        return segment.to_string();
    };
    let prop = segment[..colon].trim();
    let value_start = colon + 1;
    let value = &segment[value_start..];
    let leading_value_whitespace = value
        .char_indices()
        .find_map(|(index, ch)| (!ch.is_whitespace()).then_some(index))
        .unwrap_or(value.len());
    let value_prefix = &value[..leading_value_whitespace];
    let value_body = &value[leading_value_whitespace..];
    let rewritten = if is_animation_name_property(prop) {
        rewrite_animation_name_value(value_body.trim(), keyframes)
    } else if is_animation_property(prop) {
        rewrite_animation_value(value_body.trim(), keyframes)
    } else {
        return segment.to_string();
    };

    let mut output = String::new();
    output.push_str(&segment[..value_start]);
    output.push_str(value_prefix);
    output.push_str(&rewritten);
    output
}

pub(crate) fn find_top_level_colon(source: &str) -> Option<usize> {
    let mut state = CssScannerState::Normal;
    let mut paren_depth = 0usize;
    let mut index = 0usize;
    while index < source.len() {
        let ch = source[index..].chars().next()?;
        match state {
            CssScannerState::Normal => {
                if source[index..].starts_with("/*") {
                    state = CssScannerState::BlockComment;
                    index += 2;
                    continue;
                }
                match ch {
                    '\'' => state = CssScannerState::SingleQuote,
                    '"' => state = CssScannerState::DoubleQuote,
                    '(' => paren_depth += 1,
                    ')' if paren_depth > 0 => paren_depth -= 1,
                    ':' if paren_depth == 0 => return Some(index),
                    _ => {}
                }
            }
            CssScannerState::SingleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < source.len() {
                        index += source[index..].chars().next()?.len_utf8();
                    }
                    continue;
                }
                if ch == '\'' {
                    state = CssScannerState::Normal;
                }
            }
            CssScannerState::DoubleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < source.len() {
                        index += source[index..].chars().next()?.len_utf8();
                    }
                    continue;
                }
                if ch == '"' {
                    state = CssScannerState::Normal;
                }
            }
            CssScannerState::BlockComment => {
                if source[index..].starts_with("*/") {
                    state = CssScannerState::Normal;
                    index += 2;
                    continue;
                }
            }
        }
        index += ch.len_utf8();
    }
    None
}

pub(crate) fn is_animation_name_property(prop: &str) -> bool {
    let prop = prop.trim().to_ascii_lowercase();
    prop == "animation-name" || (prop.starts_with('-') && prop.ends_with("-animation-name"))
}

pub(crate) fn is_animation_property(prop: &str) -> bool {
    let prop = prop.trim().to_ascii_lowercase();
    prop == "animation" || (prop.starts_with('-') && prop.ends_with("-animation"))
}

pub(crate) fn rewrite_animation_name_value(value: &str, keyframes: &[(String, String)]) -> String {
    value
        .split(',')
        .map(|part| {
            let trimmed = part.trim();
            lookup_keyframe_name(trimmed, keyframes)
                .cloned()
                .unwrap_or_else(|| trimmed.to_string())
        })
        .collect::<Vec<_>>()
        .join(",")
}

pub(crate) fn rewrite_animation_value(value: &str, keyframes: &[(String, String)]) -> String {
    value
        .split(',')
        .map(|part| {
            let trimmed = part.trim();
            let mut values = trimmed.split_whitespace().collect::<Vec<_>>();
            let Some(index) = values
                .iter()
                .position(|value| lookup_keyframe_name(value, keyframes).is_some())
            else {
                return part.to_string();
            };
            let rewritten = lookup_keyframe_name(values[index], keyframes)
                .expect("checked above")
                .as_str();
            values[index] = rewritten;
            values.join(" ")
        })
        .collect::<Vec<_>>()
        .join(",")
}

pub(crate) fn rewrite_selector_list(selector: &str, scope_id: &str) -> String {
    rewrite_selector_list_for_rule(selector, scope_id, false, false).selector
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SelectorRewriteResult {
    pub(crate) selector: String,
    pub(crate) deep_passthrough: bool,
}

pub(crate) fn rewrite_selector_list_for_rule(
    selector: &str,
    scope_id: &str,
    rule_has_nested_block: bool,
    rule_has_direct_nested_rule: bool,
) -> SelectorRewriteResult {
    let mut deep_passthrough = false;
    let parts = split_selector_list(selector);
    let mut rewritten = String::new();
    for (index, part) in parts.into_iter().enumerate() {
        let trimmed = part.trim();
        let result = rewrite_single_selector_for_rule(
            trimmed,
            scope_id,
            rule_has_nested_block,
            rule_has_direct_nested_rule,
        );
        deep_passthrough |= result.deep_passthrough;

        if index > 0 {
            rewritten.push(',');
            if selector_list_branch_preserves_leading_whitespace(trimmed, &result.selector) {
                rewritten.push_str(selector_leading_whitespace(part));
            }
        }
        rewritten.push_str(&result.selector);
    }
    let selector = if selector.ends_with(' ') {
        format!("{rewritten} ")
    } else {
        rewritten
    };
    SelectorRewriteResult {
        selector,
        deep_passthrough,
    }
}

pub(crate) fn selector_list_branch_preserves_leading_whitespace(
    original: &str,
    rewritten: &str,
) -> bool {
    if original.is_empty() || rewritten.starts_with('[') || original.starts_with('*') {
        return false;
    }
    if original.starts_with(">>>") || original.starts_with("/deep/") {
        return false;
    }
    !match_selector_pseudo_function(
        original,
        0,
        &[
            ":global",
            "::v-global",
            ":slotted",
            "::v-slotted",
            ":deep",
            "::v-deep",
        ],
    )
    .is_some()
}

pub(crate) fn selector_leading_whitespace(selector: &str) -> &str {
    let end = selector
        .char_indices()
        .find_map(|(index, ch)| (!ch.is_whitespace()).then_some(index))
        .unwrap_or(selector.len());
    &selector[..end]
}

pub(crate) fn rewrite_selector_branches(selector: &str, scope_id: &str) -> String {
    let parts = split_selector_list(selector);
    let mut rewritten = String::new();
    for (index, part) in parts.into_iter().enumerate() {
        let trimmed = part.trim();
        let branch = rewrite_single_selector(trimmed, scope_id);
        if index > 0 {
            rewritten.push(',');
            if selector_list_branch_preserves_leading_whitespace(trimmed, &branch) {
                rewritten.push_str(selector_leading_whitespace(part));
            }
        }
        rewritten.push_str(&branch);
    }
    rewritten
}

pub(crate) fn rewrite_direct_nested_parent_selector(selector: &str) -> String {
    if !direct_nested_parent_selector_needs_rewrite(selector) {
        return selector.to_string();
    }
    let parts = split_selector_list(selector);
    let mut rewritten = String::new();
    for (index, part) in parts.into_iter().enumerate() {
        let trimmed = part.trim();
        let branch = rewrite_direct_nested_parent_selector_branch(trimmed);
        if index > 0 {
            rewritten.push(',');
            if selector_list_branch_preserves_leading_whitespace(trimmed, &branch) {
                rewritten.push_str(selector_leading_whitespace(part));
            }
        }
        rewritten.push_str(&branch);
    }
    rewritten
}

pub(crate) fn direct_nested_parent_selector_needs_rewrite(selector: &str) -> bool {
    split_selector_list(selector)
        .into_iter()
        .any(|part| direct_nested_parent_selector_branch_needs_rewrite(part.trim()))
}

pub(crate) fn direct_nested_parent_selector_branch_needs_rewrite(selector: &str) -> bool {
    let normalized_selector = normalize_selector_comments(selector);
    let selector = normalized_selector.trim();
    if selector.is_empty() {
        return false;
    }
    let stripped = strip_leading_universal_selector(selector);
    if stripped != selector {
        return true;
    }
    if rewrite_scope_anchored_deep_container_branch(selector) != selector {
        return true;
    }
    direct_nested_parent_container_selector_needs_rewrite(selector)
}

pub(crate) fn rewrite_direct_nested_parent_selector_branch(selector: &str) -> String {
    let normalized_selector = normalize_selector_comments(selector);
    let selector = strip_leading_universal_selector(normalized_selector.trim());
    let selector = rewrite_scope_anchored_deep_container_branch(selector);
    rewrite_direct_nested_parent_container_selector(&selector).unwrap_or(selector)
}

pub(crate) fn direct_nested_parent_container_selector_needs_rewrite(selector: &str) -> bool {
    let Some(target) = scoped_container_injection_target(selector) else {
        return false;
    };
    let Some((open, close)) = target.parens else {
        return false;
    };
    let Some(name) = matched_selector_name(selector, target.start, &[":is", ":where"]) else {
        return false;
    };
    let suffix = &selector[close + 1..];
    if !suffix.trim().is_empty() && !selector_suffix_is_pseudo_only(suffix) {
        return false;
    }
    matches!(name, ":is" | ":where")
        && direct_nested_parent_selector_needs_rewrite(&selector[open + 1..close])
}

pub(crate) fn rewrite_direct_nested_parent_container_selector(selector: &str) -> Option<String> {
    let target = scoped_container_injection_target(selector)?;
    let (open, close) = target.parens?;
    let name = matched_selector_name(selector, target.start, &[":is", ":where"])?;
    let suffix = &selector[close + 1..];
    if !suffix.trim().is_empty() && !selector_suffix_is_pseudo_only(suffix) {
        return None;
    }
    let rewritten_inner = rewrite_direct_nested_parent_selector(&selector[open + 1..close]);
    Some(format!(
        "{}{name}({rewritten_inner}){suffix}",
        &selector[..target.start]
    ))
}

pub(crate) fn rewrite_slotted_inner_selector(selector: &str, scope_id: &str) -> String {
    rewrite_scoped_container_injection_target_with(selector, scope_id, rewrite_selector_branches)
        .unwrap_or_else(|| inject_scope_attribute(selector, scope_id))
}

pub(crate) fn rewrite_scoped_container_injection_target_with(
    selector: &str,
    scope_id: &str,
    rewrite_branches: fn(&str, &str) -> String,
) -> Option<String> {
    let selector = strip_leading_universal_selector(selector.trim());
    let target = scoped_container_injection_target(selector)?;
    let (open, close) = target.parens?;
    let name = matched_selector_name(selector, target.start, &[":is", ":where"])?;
    let rewritten_inner = rewrite_branches(&selector[open + 1..close], scope_id);

    Some(format!(
        "{}{name}({rewritten_inner}){}",
        &selector[..target.start],
        &selector[close + 1..]
    ))
}

pub(crate) fn rewrite_single_selector(selector: &str, scope_id: &str) -> String {
    rewrite_single_selector_for_rule(selector, scope_id, false, false).selector
}

pub(crate) fn rewrite_single_selector_for_rule(
    selector: &str,
    scope_id: &str,
    rule_has_nested_block: bool,
    rule_has_direct_nested_rule: bool,
) -> SelectorRewriteResult {
    rewrite_single_selector_with_options(
        selector,
        scope_id,
        rule_has_nested_block,
        rule_has_direct_nested_rule,
        false,
    )
}

pub(crate) fn rewrite_single_selector_branch(
    selector: &str,
    scope_id: &str,
    rule_has_nested_block: bool,
    rule_has_direct_nested_rule: bool,
) -> SelectorRewriteResult {
    rewrite_single_selector_with_options(
        selector,
        scope_id,
        rule_has_nested_block,
        rule_has_direct_nested_rule,
        true,
    )
}

pub(crate) fn rewrite_single_selector_with_options(
    selector: &str,
    scope_id: &str,
    rule_has_nested_block: bool,
    rule_has_direct_nested_rule: bool,
    in_container_branch: bool,
) -> SelectorRewriteResult {
    let normalized_selector = normalize_selector_comments(selector);
    let selector = normalized_selector.trim();
    if selector.is_empty() {
        return SelectorRewriteResult {
            selector: selector.to_string(),
            deep_passthrough: false,
        };
    }
    if let Some(global) = find_top_level_pseudo_function(selector, &[":global", "::v-global"]) {
        if let Some((open, close)) = global.parens {
            return SelectorRewriteResult {
                selector: first_selector_branch(selector[open + 1..close].trim())
                    .trim()
                    .to_string(),
                deep_passthrough: false,
            };
        }
    }
    if let Some(deep) = find_deep_combinator(selector) {
        return SelectorRewriteResult {
            selector: rewrite_deep_selector(
                &selector[..deep.start],
                &selector[deep.end..],
                scope_id,
            ),
            deep_passthrough: false,
        };
    }
    if let Some(rewritten) = rewrite_deep_container_selector_for_rule(
        selector,
        scope_id,
        rule_has_nested_block,
        rule_has_direct_nested_rule,
    ) {
        return rewritten;
    }
    if let Some(deep) = find_top_level_pseudo_function(selector, &[":deep", "::v-deep"]) {
        if let Some((open, close)) = deep.parens {
            let mut rhs = first_selector_branch(selector[open + 1..close].trim())
                .trim()
                .to_string();
            rhs.push_str(&selector[close + 1..]);
            return SelectorRewriteResult {
                selector: rewrite_deep_selector(&selector[..deep.start], &rhs, scope_id),
                deep_passthrough: !in_container_branch,
            };
        }
        return SelectorRewriteResult {
            selector: rewrite_deep_selector(
                &selector[..deep.start],
                &selector[deep.end..],
                scope_id,
            ),
            deep_passthrough: !in_container_branch,
        };
    }
    if let Some(rewritten) = rewrite_slotted_selector(selector, scope_id) {
        return SelectorRewriteResult {
            selector: rewritten,
            deep_passthrough: false,
        };
    }
    if let Some(rewritten) = rewrite_scoped_container_injection_target(selector, scope_id) {
        return SelectorRewriteResult {
            selector: rewritten,
            deep_passthrough: false,
        };
    }
    SelectorRewriteResult {
        selector: inject_scope_attribute(selector, scope_id),
        deep_passthrough: false,
    }
}

pub(crate) fn rewrite_scoped_container_injection_target(
    selector: &str,
    scope_id: &str,
) -> Option<String> {
    rewrite_scoped_container_injection_target_with(selector, scope_id, rewrite_selector_branches)
}

pub(crate) fn scoped_container_injection_target(selector: &str) -> Option<SelectorMatch> {
    let mut state = SelectorScannerState::Normal;
    let mut target = None;
    let mut has_target = false;
    let mut index = 0usize;
    while index < selector.len() {
        let ch = selector[index..].chars().next()?;
        match state {
            SelectorScannerState::Normal => match ch {
                '\'' => state = SelectorScannerState::SingleQuote,
                '"' => state = SelectorScannerState::DoubleQuote,
                '\\' => {
                    let end = consume_selector_token(selector, index);
                    target = None;
                    has_target = true;
                    index = end;
                    continue;
                }
                '/' if selector[index..].starts_with("/*") => {
                    index = skip_selector_comment(selector, index);
                    continue;
                }
                '&' => {
                    if !has_target {
                        has_target = true;
                        target = None;
                    }
                }
                '[' => {
                    let end = find_matching_selector_bracket(selector, index)
                        .unwrap_or(selector.len().saturating_sub(1));
                    target = None;
                    has_target = true;
                    index = end + 1;
                    continue;
                }
                ':' => {
                    if let Some(pseudo) =
                        match_selector_pseudo_function(selector, index, &[":is", ":where"])
                    {
                        if !has_target {
                            target = Some(pseudo);
                            has_target = true;
                        }
                        index = pseudo.end;
                        continue;
                    }
                    index = skip_selector_pseudo(selector, index);
                    continue;
                }
                '>' | '+' | '~' | ',' => {}
                '*' => {
                    if !has_target {
                        has_target = true;
                        target = None;
                    }
                }
                _ if ch.is_whitespace() => {}
                _ if is_selector_ident_start(ch) || ch == '.' || ch == '#' => {
                    let end = consume_selector_token(selector, index);
                    target = None;
                    has_target = true;
                    index = end;
                    continue;
                }
                _ => {}
            },
            SelectorScannerState::SingleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < selector.len() {
                        index += selector[index..].chars().next().map_or(0, char::len_utf8);
                    }
                    continue;
                }
                if ch == '\'' {
                    state = SelectorScannerState::Normal;
                }
            }
            SelectorScannerState::DoubleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < selector.len() {
                        index += selector[index..].chars().next().map_or(0, char::len_utf8);
                    }
                    continue;
                }
                if ch == '"' {
                    state = SelectorScannerState::Normal;
                }
            }
        }
        index += ch.len_utf8();
    }
    target
}

pub(crate) fn rewrite_deep_container_selector_for_rule(
    selector: &str,
    scope_id: &str,
    rule_has_nested_block: bool,
    rule_has_direct_nested_rule: bool,
) -> Option<SelectorRewriteResult> {
    let names = [":is", ":where", ":not", ":has"];
    let container = find_top_level_pseudo_function(selector, &names)?;
    let (open, close) = container.parens?;
    let inner = &selector[open + 1..close];
    if !selector_has_deep(inner) {
        return None;
    }

    let name = matched_selector_name(selector, container.start, &names)?;
    let prefix = &selector[..container.start];
    let suffix = &selector[close + 1..];
    let has_scope_anchor = selector_scope_anchor_before(selector, container.start);
    let has_trailing_nodes = !suffix.trim().is_empty();
    let branches = split_selector_list(inner)
        .into_iter()
        .map(str::trim)
        .collect::<Vec<_>>();
    let has_deep = branches.iter().any(|branch| selector_has_deep(branch));
    let has_normal = branches.iter().any(|branch| !selector_has_deep(branch));
    let first_branch_has_deep = branches
        .first()
        .is_some_and(|branch| selector_has_deep(branch));
    let can_split = matches!(name, ":is" | ":where" | ":has");
    let should_split =
        can_split && has_deep && has_normal && !has_scope_anchor && has_trailing_nodes;

    if name == ":not" && has_deep && has_normal && !has_scope_anchor && has_trailing_nodes {
        return None;
    }

    if rule_has_direct_nested_rule
        && has_deep
        && !first_branch_has_deep
        && (!has_trailing_nodes || has_scope_anchor)
    {
        let rewritten_inner = rewrite_direct_nested_first_normal_deep_container_inner_branches(
            inner,
            scope_id,
            rule_has_nested_block,
            rule_has_direct_nested_rule,
        );
        let mut rewritten = format!("{prefix}{name}({rewritten_inner}){suffix}");
        if has_scope_anchor {
            rewritten = inject_scope_before_container(&rewritten, container.start, scope_id);
        } else {
            rewritten = inject_scope_after_container_pseudo(&rewritten, name, scope_id);
        }
        return Some(SelectorRewriteResult {
            selector: rewritten,
            deep_passthrough: true,
        });
    }

    if should_split {
        let mut deep_passthrough = false;
        let mut selector = String::new();
        let first_normal_direct_nested =
            rule_has_direct_nested_rule && has_deep && !first_branch_has_deep;
        let mut seen_deep = false;
        for (index, part) in split_selector_list(inner).into_iter().enumerate() {
            let branch = part.trim();
            let branch_has_deep = selector_has_deep(branch);
            let branch_is_first_deep = first_normal_direct_nested && !seen_deep && branch_has_deep;
            let branch_before_first_deep =
                first_normal_direct_nested && !seen_deep && !branch_has_deep;
            if branch_has_deep {
                seen_deep = true;
            }
            let rewritten = if branch_before_first_deep {
                let branch = rewrite_direct_nested_first_normal_split_branch(branch, name, suffix);
                format!("{prefix}{name}({branch}){suffix}")
            } else if branch_has_deep {
                let result = rewrite_single_selector_branch(
                    branch,
                    scope_id,
                    rule_has_nested_block,
                    rule_has_direct_nested_rule,
                );
                deep_passthrough = true;
                let mut rewritten = format!("{prefix}{name}({}){suffix}", result.selector);
                if rule_has_direct_nested_rule {
                    rewritten = inject_scope_after_container_pseudo(&rewritten, name, scope_id);
                }
                rewritten
            } else if matches!(name, ":is" | ":where") && selector_suffix_is_pseudo_only(suffix) {
                let result = if rule_has_direct_nested_rule {
                    rewrite_direct_nested_deep_container_branch(
                        branch,
                        scope_id,
                        rule_has_nested_block,
                        rule_has_direct_nested_rule,
                    )
                } else {
                    rewrite_single_selector_branch(
                        branch,
                        scope_id,
                        rule_has_nested_block,
                        rule_has_direct_nested_rule,
                    )
                };
                format!("{prefix}{name}({}){suffix}", result.selector)
            } else {
                let branch_selector = format!("{prefix}{name}({branch}){suffix}");
                inject_scope_attribute(&branch_selector, scope_id)
            };
            if index > 0 {
                selector.push(',');
                if matches!(name, ":is" | ":where") {
                    selector.push(' ');
                } else if name == ":has" && !selector_suffix_is_pseudo_only(suffix) {
                    selector.push(' ');
                } else if name == ":has"
                    && selector_suffix_is_pseudo_only(suffix)
                    && branch_has_deep
                {
                    selector.push(' ');
                } else if branch_is_first_deep {
                    selector.push(' ');
                } else {
                    let preserve_leading = if matches!(name, ":is" | ":where")
                        && selector_suffix_is_pseudo_only(suffix)
                        && !branch_has_deep
                    {
                        !branch.is_empty() && !rewritten.starts_with('[')
                    } else {
                        selector_list_branch_preserves_leading_whitespace(branch, &rewritten)
                    };
                    if preserve_leading {
                        selector.push_str(selector_leading_whitespace(part));
                    }
                }
            }
            selector.push_str(&rewritten);
        }
        return Some(SelectorRewriteResult {
            selector,
            deep_passthrough,
        });
    }

    if has_scope_anchor && !rule_has_direct_nested_rule {
        let rewritten_inner = rewrite_scope_anchored_deep_container_inner_branches(inner);
        let rewritten = format!("{prefix}{name}({rewritten_inner}){suffix}");
        return Some(SelectorRewriteResult {
            selector: inject_scope_before_container(&rewritten, container.start, scope_id),
            deep_passthrough: true,
        });
    }

    let (rewritten_inner, deep_passthrough) = rewrite_scoped_deep_container_inner_branches(
        inner,
        scope_id,
        rule_has_nested_block,
        rule_has_direct_nested_rule,
    );
    let mut rewritten = format!("{prefix}{name}({rewritten_inner}){suffix}");
    if has_scope_anchor {
        rewritten = inject_scope_before_container(&rewritten, container.start, scope_id);
    } else if rule_has_direct_nested_rule && deep_passthrough {
        rewritten = inject_scope_after_container_pseudo(&rewritten, name, scope_id);
    }
    Some(SelectorRewriteResult {
        selector: rewritten,
        deep_passthrough,
    })
}

pub(crate) fn rewrite_scoped_deep_container_inner_branches(
    inner: &str,
    scope_id: &str,
    rule_has_nested_block: bool,
    rule_has_direct_nested_rule: bool,
) -> (String, bool) {
    let mut rewritten = String::new();
    let mut deep_passthrough = false;
    for (index, part) in split_selector_list(inner).into_iter().enumerate() {
        let trimmed = part.trim();
        if selector_has_deep(trimmed) {
            deep_passthrough = true;
        }
        let result = if rule_has_direct_nested_rule {
            rewrite_direct_nested_deep_container_branch(
                trimmed,
                scope_id,
                rule_has_nested_block,
                rule_has_direct_nested_rule,
            )
        } else {
            rewrite_single_selector_branch(
                trimmed,
                scope_id,
                rule_has_nested_block,
                rule_has_direct_nested_rule,
            )
        };
        if index > 0 {
            rewritten.push(',');
            if selector_list_branch_preserves_leading_whitespace(trimmed, &result.selector) {
                rewritten.push_str(selector_leading_whitespace(part));
            }
        }
        rewritten.push_str(&result.selector);
    }
    (rewritten, deep_passthrough)
}

pub(crate) fn rewrite_direct_nested_first_normal_split_branch(
    branch: &str,
    name: &str,
    suffix: &str,
) -> String {
    if matches!(name, ":is" | ":where") && selector_suffix_is_pseudo_only(suffix) {
        rewrite_direct_nested_parent_selector_branch(branch)
    } else {
        branch.to_string()
    }
}

pub(crate) fn rewrite_direct_nested_first_normal_deep_container_inner_branches(
    inner: &str,
    scope_id: &str,
    rule_has_nested_block: bool,
    rule_has_direct_nested_rule: bool,
) -> String {
    let mut rewritten = String::new();
    let mut seen_deep = false;
    for (index, part) in split_selector_list(inner).into_iter().enumerate() {
        let trimmed = part.trim();
        let branch_has_deep = selector_has_deep(trimmed);
        if branch_has_deep {
            seen_deep = true;
        }
        let branch = if seen_deep {
            rewrite_direct_nested_deep_container_branch(
                trimmed,
                scope_id,
                rule_has_nested_block,
                rule_has_direct_nested_rule,
            )
            .selector
        } else {
            rewrite_direct_nested_parent_selector_branch(trimmed)
        };
        if index > 0 {
            rewritten.push(',');
            if selector_list_branch_preserves_leading_whitespace(trimmed, &branch) {
                rewritten.push_str(selector_leading_whitespace(part));
            }
        }
        rewritten.push_str(&branch);
    }
    rewritten
}

pub(crate) fn rewrite_direct_nested_deep_container_branch(
    selector: &str,
    scope_id: &str,
    rule_has_nested_block: bool,
    rule_has_direct_nested_rule: bool,
) -> SelectorRewriteResult {
    if let Some(slotted) = rewrite_slotted_selector_with_prefix_scope(selector, scope_id) {
        return SelectorRewriteResult {
            selector: slotted,
            deep_passthrough: false,
        };
    }
    rewrite_single_selector_branch(
        selector,
        scope_id,
        rule_has_nested_block,
        rule_has_direct_nested_rule,
    )
}

pub(crate) fn deep_container_direct_nested_wraps_parent_declarations(selector: &str) -> bool {
    let names = [":is", ":where", ":not", ":has"];
    let Some(container) = find_top_level_pseudo_function(selector, &names) else {
        return false;
    };
    let Some((open, close)) = container.parens else {
        return false;
    };
    let inner = &selector[open + 1..close];
    if !selector_has_deep(inner) {
        return false;
    }
    split_selector_list(inner)
        .first()
        .is_some_and(|branch| !selector_has_deep(branch.trim()))
}

pub(crate) fn rewrite_scope_anchored_deep_container_inner_branches(inner: &str) -> String {
    let mut rewritten = String::new();
    for (index, part) in split_selector_list(inner).into_iter().enumerate() {
        let trimmed = part.trim();
        let branch = rewrite_scope_anchored_deep_container_branch(trimmed);
        if index > 0 {
            rewritten.push(',');
            if selector_list_branch_preserves_leading_whitespace(trimmed, &branch) {
                rewritten.push_str(selector_leading_whitespace(part));
            }
        }
        rewritten.push_str(&branch);
    }
    rewritten
}

pub(crate) fn rewrite_scope_anchored_deep_container_branch(selector: &str) -> String {
    let names = [
        ":global",
        "::v-global",
        ":slotted",
        "::v-slotted",
        ":deep",
        "::v-deep",
    ];
    let Some(special) = find_top_level_pseudo_function(selector, &names) else {
        return selector.to_string();
    };
    let Some((open, close)) = special.parens else {
        return selector.to_string();
    };
    let name = matched_selector_name(selector, special.start, &names).unwrap_or_default();
    let inner = first_selector_branch(selector[open + 1..close].trim()).trim();
    if matches!(name, ":global" | "::v-global") {
        return inner.to_string();
    }
    if matches!(name, ":slotted" | "::v-slotted") {
        return replace_slotted_pseudo_without_scope(selector, special, inner);
    }
    replace_deep_pseudo_without_scope(selector, special, inner)
}

pub(crate) fn selector_suffix_is_pseudo_only(suffix: &str) -> bool {
    let suffix = suffix.trim();
    if suffix.is_empty() {
        return false;
    }
    let mut index = 0usize;
    while index < suffix.len() {
        let Some(ch) = suffix[index..].chars().next() else {
            break;
        };
        if ch != ':' {
            return false;
        }
        index += ch.len_utf8();
        if suffix[index..].starts_with(':') {
            index += ':'.len_utf8();
        }
        let name_start = index;
        while index < suffix.len() {
            let Some(ch) = suffix[index..].chars().next() else {
                break;
            };
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                index += ch.len_utf8();
            } else {
                break;
            }
        }
        if index == name_start {
            return false;
        }
        let open = skip_selector_whitespace(suffix, index);
        if suffix[open..].starts_with('(') {
            let Some(close) = find_matching_selector_paren(suffix, open) else {
                return false;
            };
            index = close + 1;
        }
    }
    index == suffix.len()
}

pub(crate) fn replace_slotted_pseudo_without_scope(
    selector: &str,
    slotted: SelectorMatch,
    inner: &str,
) -> String {
    let Some((_, close)) = slotted.parens else {
        return selector.to_string();
    };
    let prefix = &selector[..slotted.start];
    let inner = strip_leading_universal_selector(inner);
    let inner = if prefix.is_empty() {
        inner.trim_start()
    } else {
        inner
    };
    format!("{prefix}{inner}{}", &selector[close + 1..])
}

pub(crate) fn replace_deep_pseudo_without_scope(
    selector: &str,
    deep: SelectorMatch,
    inner: &str,
) -> String {
    let Some((_, close)) = deep.parens else {
        return selector.to_string();
    };
    let mut suffix = String::new();
    suffix.push_str(inner);
    suffix.push_str(&selector[close + 1..]);
    let suffix = suffix.trim_start();
    let prefix = selector[..deep.start].trim_end();
    if suffix.is_empty() {
        prefix.to_string()
    } else if prefix.is_empty() {
        format!(" {suffix}")
    } else {
        format!("{prefix} {suffix}")
    }
}

pub(crate) fn selector_scope_anchor_before(selector: &str, end: usize) -> bool {
    let prefix = &selector[..end];
    selector_injection_index(prefix).is_some()
}

pub(crate) fn inject_scope_before_container(
    selector: &str,
    container_start: usize,
    scope_id: &str,
) -> String {
    let prefix = &selector[..container_start];
    let trimmed_prefix_end = prefix.trim_end().len();
    let trailing = &prefix[trimmed_prefix_end..];
    let scoped_prefix = inject_scope_attribute(&prefix[..trimmed_prefix_end], scope_id);
    format!("{scoped_prefix}{trailing}{}", &selector[container_start..])
}

pub(crate) fn selector_has_deep(selector: &str) -> bool {
    find_deep_combinator(selector).is_some()
        || find_pseudo_function(selector, &[":deep", "::v-deep"]).is_some()
}

pub(crate) fn selector_has_deep_pseudo(selector: &str) -> bool {
    find_pseudo_function(selector, &[":deep", "::v-deep"]).is_some()
}

pub(crate) fn collect_selector_list_deprecation_warnings(
    selector: &str,
    warnings: &mut Vec<String>,
) {
    for part in split_selector_list(selector) {
        collect_selector_deprecation_warnings(part.trim(), warnings);
    }
}

pub(crate) fn collect_selector_deprecation_warnings(selector: &str, warnings: &mut Vec<String>) {
    let mut state = SelectorScannerState::Normal;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut index = 0usize;
    while index < selector.len() {
        let Some(ch) = selector[index..].chars().next() else {
            break;
        };
        match state {
            SelectorScannerState::Normal => match ch {
                '\'' => state = SelectorScannerState::SingleQuote,
                '"' => state = SelectorScannerState::DoubleQuote,
                '\\' => {
                    index = consume_selector_escape(selector, index);
                    continue;
                }
                '/' if selector[index..].starts_with("/*") => {
                    index = skip_selector_comment(selector, index);
                    continue;
                }
                '[' => bracket_depth += 1,
                ']' if bracket_depth > 0 => bracket_depth -= 1,
                '(' => paren_depth += 1,
                ')' if paren_depth > 0 => paren_depth -= 1,
                _ if bracket_depth == 0 && paren_depth == 0 => {
                    if selector[index..].starts_with(">>>")
                        || selector[index..].starts_with("/deep/")
                    {
                        warnings.push(DEPRECATED_DEEP_COMBINATOR_MESSAGE.to_string());
                        return;
                    }
                    if let Some(deep) =
                        match_selector_pseudo_function(selector, index, &[":deep", "::v-deep"])
                    {
                        if deep.parens.is_none() {
                            let value =
                                matched_selector_name(selector, deep.start, &[":deep", "::v-deep"])
                                    .unwrap_or(":deep");
                            warnings.push(deprecated_deep_pseudo_message(value));
                        }
                        return;
                    }
                    if match_selector_pseudo_function(selector, index, &[":global", "::v-global"])
                        .is_some()
                    {
                        return;
                    }
                    if let Some(slotted) = match_selector_pseudo_function(
                        selector,
                        index,
                        &[":slotted", "::v-slotted"],
                    ) {
                        if let Some((open, close)) = slotted.parens {
                            let inner = first_selector_branch(selector[open + 1..close].trim());
                            collect_selector_deprecation_warnings(inner.trim(), warnings);
                        }
                        return;
                    }
                    if let Some(container) = match_selector_pseudo_function(
                        selector,
                        index,
                        &[":is", ":where", ":not", ":has"],
                    ) {
                        if let Some((open, close)) = container.parens {
                            for branch in split_selector_list(&selector[open + 1..close]) {
                                let branch = branch.trim();
                                if selector_has_deep_pseudo(branch) {
                                    collect_selector_deprecation_warnings(branch, warnings);
                                }
                            }
                        }
                        index = container.end;
                        continue;
                    }
                }
                _ => {}
            },
            SelectorScannerState::SingleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < selector.len() {
                        index += selector[index..].chars().next().map_or(0, char::len_utf8);
                    }
                    continue;
                }
                if ch == '\'' {
                    state = SelectorScannerState::Normal;
                }
            }
            SelectorScannerState::DoubleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < selector.len() {
                        index += selector[index..].chars().next().map_or(0, char::len_utf8);
                    }
                    continue;
                }
                if ch == '"' {
                    state = SelectorScannerState::Normal;
                }
            }
        }
        index += ch.len_utf8();
    }
}

pub(crate) fn matched_selector_name<'a>(
    selector: &str,
    start: usize,
    names: &'a [&str],
) -> Option<&'a str> {
    names
        .iter()
        .copied()
        .find(|name| selector[start..].starts_with(name))
}

pub(crate) fn rewrite_slotted_selector(selector: &str, scope_id: &str) -> Option<String> {
    let slotted = find_top_level_pseudo_function(selector, &[":slotted", "::v-slotted"])?;
    let (open, close) = slotted.parens?;
    let inner = first_selector_branch(selector[open + 1..close].trim()).trim();
    let mut rewritten = String::new();
    let prefix = &selector[..slotted.start];
    rewritten.push_str(prefix);
    let trim_leading_combinator_space = prefix.is_empty();
    if inner.is_empty() {
        rewritten.push_str(&format!("[{scope_id}-s]"));
    } else {
        let scoped_inner = rewrite_slotted_inner_selector(inner, &format!("{scope_id}-s"));
        if trim_leading_combinator_space {
            rewritten.push_str(scoped_inner.trim_start());
        } else {
            rewritten.push_str(&scoped_inner);
        }
    }
    rewritten.push_str(&selector[close + 1..]);
    Some(rewritten)
}

pub(crate) fn rewrite_slotted_selector_with_prefix_scope(
    selector: &str,
    scope_id: &str,
) -> Option<String> {
    let slotted = find_top_level_pseudo_function(selector, &[":slotted", "::v-slotted"])?;
    let (open, close) = slotted.parens?;
    let inner = first_selector_branch(selector[open + 1..close].trim()).trim();
    let prefix = &selector[..slotted.start];
    let prefix_trimmed = prefix.trim_end();
    let prefix_spacing = &prefix[prefix_trimmed.len()..];
    let scoped_prefix = if prefix_trimmed.is_empty() {
        format!("[{scope_id}]")
    } else {
        inject_scope_attribute(prefix_trimmed, scope_id)
    };
    let slotted_scope = format!("{scope_id}-s");
    let scoped_inner = if inner.is_empty() {
        format!("[{slotted_scope}]")
    } else {
        rewrite_slotted_inner_selector(strip_leading_universal_selector(inner), &slotted_scope)
    };
    let inner = if prefix_trimmed.is_empty() {
        scoped_inner.trim_start()
    } else {
        scoped_inner.as_str()
    };
    Some(format!(
        "{scoped_prefix}{prefix_spacing}{inner}{}",
        &selector[close + 1..]
    ))
}

pub(crate) fn split_selector_list(selector: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut state = SelectorScannerState::Normal;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut start = 0usize;
    let mut index = 0usize;
    while index < selector.len() {
        let Some(ch) = selector[index..].chars().next() else {
            break;
        };
        match state {
            SelectorScannerState::Normal => match ch {
                '\'' => state = SelectorScannerState::SingleQuote,
                '"' => state = SelectorScannerState::DoubleQuote,
                '\\' => {
                    index = consume_selector_escape(selector, index);
                    continue;
                }
                '/' if selector[index..].starts_with("/*") => {
                    index = skip_selector_comment(selector, index);
                    continue;
                }
                '(' => paren_depth += 1,
                ')' if paren_depth > 0 => paren_depth -= 1,
                '[' => bracket_depth += 1,
                ']' if bracket_depth > 0 => bracket_depth -= 1,
                ',' if paren_depth == 0 && bracket_depth == 0 => {
                    parts.push(&selector[start..index]);
                    start = index + ch.len_utf8();
                }
                _ => {}
            },
            SelectorScannerState::SingleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < selector.len() {
                        index += selector[index..].chars().next().map_or(0, char::len_utf8);
                    }
                    continue;
                }
                if ch == '\'' {
                    state = SelectorScannerState::Normal;
                }
            }
            SelectorScannerState::DoubleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < selector.len() {
                        index += selector[index..].chars().next().map_or(0, char::len_utf8);
                    }
                    continue;
                }
                if ch == '"' {
                    state = SelectorScannerState::Normal;
                }
            }
        }
        index += ch.len_utf8();
    }
    parts.push(&selector[start..]);
    parts
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SelectorScannerState {
    Normal,
    SingleQuote,
    DoubleQuote,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SelectorMatch {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) parens: Option<(usize, usize)>,
}

pub(crate) fn find_pseudo_function(selector: &str, names: &[&str]) -> Option<SelectorMatch> {
    let mut state = SelectorScannerState::Normal;
    let mut bracket_depth = 0usize;
    let mut index = 0usize;
    while index < selector.len() {
        let ch = selector[index..].chars().next()?;
        match state {
            SelectorScannerState::Normal => match ch {
                '\'' => state = SelectorScannerState::SingleQuote,
                '"' => state = SelectorScannerState::DoubleQuote,
                '\\' => {
                    index = consume_selector_escape(selector, index);
                    continue;
                }
                '/' if selector[index..].starts_with("/*") => {
                    index = skip_selector_comment(selector, index);
                    continue;
                }
                '[' => bracket_depth += 1,
                ']' if bracket_depth > 0 => bracket_depth -= 1,
                _ if bracket_depth == 0 => {
                    for name in names {
                        if selector[index..].starts_with(name)
                            && selector_name_boundary(selector, index + name.len())
                        {
                            let end = index + name.len();
                            let open = skip_selector_whitespace(selector, end);
                            let parens = if selector[open..].starts_with('(') {
                                find_matching_selector_paren(selector, open)
                                    .map(|close| (open, close))
                            } else {
                                None
                            };
                            let match_end = parens.map(|(_, close)| close + 1).unwrap_or(end);
                            return Some(SelectorMatch {
                                start: index,
                                end: match_end,
                                parens,
                            });
                        }
                    }
                }
                _ => {}
            },
            SelectorScannerState::SingleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < selector.len() {
                        index += selector[index..].chars().next().map_or(0, char::len_utf8);
                    }
                    continue;
                }
                if ch == '\'' {
                    state = SelectorScannerState::Normal;
                }
            }
            SelectorScannerState::DoubleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < selector.len() {
                        index += selector[index..].chars().next().map_or(0, char::len_utf8);
                    }
                    continue;
                }
                if ch == '"' {
                    state = SelectorScannerState::Normal;
                }
            }
        }
        index += ch.len_utf8();
    }
    None
}

pub(crate) fn match_selector_pseudo_function(
    selector: &str,
    index: usize,
    names: &[&str],
) -> Option<SelectorMatch> {
    for name in names {
        if selector[index..].starts_with(name)
            && selector_name_boundary(selector, index + name.len())
        {
            let end = index + name.len();
            let open = skip_selector_whitespace(selector, end);
            let parens = if selector[open..].starts_with('(') {
                find_matching_selector_paren(selector, open).map(|close| (open, close))
            } else {
                None
            };
            let match_end = parens.map(|(_, close)| close + 1).unwrap_or(end);
            return Some(SelectorMatch {
                start: index,
                end: match_end,
                parens,
            });
        }
    }
    None
}

pub(crate) fn find_top_level_pseudo_function(
    selector: &str,
    names: &[&str],
) -> Option<SelectorMatch> {
    let mut state = SelectorScannerState::Normal;
    let mut bracket_depth = 0usize;
    let mut paren_depth = 0usize;
    let mut index = 0usize;
    while index < selector.len() {
        let ch = selector[index..].chars().next()?;
        match state {
            SelectorScannerState::Normal => match ch {
                '\'' => state = SelectorScannerState::SingleQuote,
                '"' => state = SelectorScannerState::DoubleQuote,
                '\\' => {
                    index = consume_selector_escape(selector, index);
                    continue;
                }
                '/' if selector[index..].starts_with("/*") => {
                    index = skip_selector_comment(selector, index);
                    continue;
                }
                '[' => bracket_depth += 1,
                ']' if bracket_depth > 0 => bracket_depth -= 1,
                '(' => paren_depth += 1,
                ')' if paren_depth > 0 => paren_depth -= 1,
                _ if bracket_depth == 0 && paren_depth == 0 => {
                    for name in names {
                        if selector[index..].starts_with(name)
                            && selector_name_boundary(selector, index + name.len())
                        {
                            let end = index + name.len();
                            let open = skip_selector_whitespace(selector, end);
                            let parens = if selector[open..].starts_with('(') {
                                find_matching_selector_paren(selector, open)
                                    .map(|close| (open, close))
                            } else {
                                None
                            };
                            let match_end = parens.map(|(_, close)| close + 1).unwrap_or(end);
                            return Some(SelectorMatch {
                                start: index,
                                end: match_end,
                                parens,
                            });
                        }
                    }
                }
                _ => {}
            },
            SelectorScannerState::SingleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < selector.len() {
                        index += selector[index..].chars().next().map_or(0, char::len_utf8);
                    }
                    continue;
                }
                if ch == '\'' {
                    state = SelectorScannerState::Normal;
                }
            }
            SelectorScannerState::DoubleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < selector.len() {
                        index += selector[index..].chars().next().map_or(0, char::len_utf8);
                    }
                    continue;
                }
                if ch == '"' {
                    state = SelectorScannerState::Normal;
                }
            }
        }
        index += ch.len_utf8();
    }
    None
}

pub(crate) fn first_selector_branch(selector: &str) -> &str {
    split_selector_list(selector)
        .into_iter()
        .next()
        .unwrap_or(selector)
}

pub(crate) fn selector_name_boundary(selector: &str, index: usize) -> bool {
    selector[index..]
        .chars()
        .next()
        .map(|ch| !(ch.is_ascii_alphanumeric() || ch == '-' || ch == '_'))
        .unwrap_or(true)
}

pub(crate) fn skip_selector_whitespace(selector: &str, mut index: usize) -> usize {
    while index < selector.len() {
        let Some(ch) = selector[index..].chars().next() else {
            break;
        };
        if !ch.is_whitespace() {
            break;
        }
        index += ch.len_utf8();
    }
    index
}

pub(crate) fn find_matching_selector_paren(selector: &str, open: usize) -> Option<usize> {
    let mut state = SelectorScannerState::Normal;
    let mut depth = 0usize;
    let mut index = open;
    while index < selector.len() {
        let ch = selector[index..].chars().next()?;
        match state {
            SelectorScannerState::Normal => match ch {
                '\'' => state = SelectorScannerState::SingleQuote,
                '"' => state = SelectorScannerState::DoubleQuote,
                '\\' => {
                    index = consume_selector_escape(selector, index);
                    continue;
                }
                '/' if selector[index..].starts_with("/*") => {
                    index = skip_selector_comment(selector, index);
                    continue;
                }
                '(' => depth += 1,
                ')' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return Some(index);
                    }
                }
                _ => {}
            },
            SelectorScannerState::SingleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < selector.len() {
                        index += selector[index..].chars().next().map_or(0, char::len_utf8);
                    }
                    continue;
                }
                if ch == '\'' {
                    state = SelectorScannerState::Normal;
                }
            }
            SelectorScannerState::DoubleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < selector.len() {
                        index += selector[index..].chars().next().map_or(0, char::len_utf8);
                    }
                    continue;
                }
                if ch == '"' {
                    state = SelectorScannerState::Normal;
                }
            }
        }
        index += ch.len_utf8();
    }
    None
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DeepCombinator {
    pub(crate) start: usize,
    pub(crate) end: usize,
}

pub(crate) fn find_deep_combinator(selector: &str) -> Option<DeepCombinator> {
    let mut state = SelectorScannerState::Normal;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut index = 0usize;
    while index < selector.len() {
        let ch = selector[index..].chars().next()?;
        match state {
            SelectorScannerState::Normal => match ch {
                '\'' => state = SelectorScannerState::SingleQuote,
                '"' => state = SelectorScannerState::DoubleQuote,
                '\\' => {
                    index = consume_selector_escape(selector, index);
                    continue;
                }
                '/' if selector[index..].starts_with("/*") => {
                    index = skip_selector_comment(selector, index);
                    continue;
                }
                '(' => paren_depth += 1,
                ')' if paren_depth > 0 => paren_depth -= 1,
                '[' => bracket_depth += 1,
                ']' if bracket_depth > 0 => bracket_depth -= 1,
                _ if paren_depth == 0 && bracket_depth == 0 => {
                    if selector[index..].starts_with(">>>") {
                        return Some(DeepCombinator {
                            start: index,
                            end: index + 3,
                        });
                    }
                    if selector[index..].starts_with("/deep/") {
                        return Some(DeepCombinator {
                            start: index,
                            end: index + "/deep/".len(),
                        });
                    }
                }
                _ => {}
            },
            SelectorScannerState::SingleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < selector.len() {
                        index += selector[index..].chars().next().map_or(0, char::len_utf8);
                    }
                    continue;
                }
                if ch == '\'' {
                    state = SelectorScannerState::Normal;
                }
            }
            SelectorScannerState::DoubleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < selector.len() {
                        index += selector[index..].chars().next().map_or(0, char::len_utf8);
                    }
                    continue;
                }
                if ch == '"' {
                    state = SelectorScannerState::Normal;
                }
            }
        }
        index += ch.len_utf8();
    }
    None
}

pub(crate) fn rewrite_deep_selector(prefix: &str, suffix: &str, scope_id: &str) -> String {
    let scoped = inject_scope_attribute(prefix.trim_end(), scope_id);
    let suffix = suffix.trim_start();
    if suffix.is_empty() {
        scoped
    } else {
        format!("{scoped} {suffix}")
    }
}

pub(crate) fn rewrite_deep_combinator_selector_without_scope(selector: &str) -> Option<String> {
    let deep = find_deep_combinator(selector)?;
    let prefix = selector[..deep.start].trim_end();
    let suffix = selector[deep.end..].trim_start();
    if suffix.is_empty() {
        Some(prefix.to_string())
    } else if prefix.is_empty() {
        Some(suffix.to_string())
    } else {
        Some(format!("{prefix} {suffix}"))
    }
}

pub(crate) fn inject_scope_after_container_pseudo(
    selector: &str,
    name: &str,
    scope_id: &str,
) -> String {
    let Some(container) = find_top_level_pseudo_function(selector, &[name]) else {
        return inject_scope_attribute(selector, scope_id);
    };
    let index = container.end;
    let mut rewritten = String::new();
    rewritten.push_str(&selector[..index]);
    rewritten.push('[');
    rewritten.push_str(scope_id);
    rewritten.push(']');
    rewritten.push_str(&selector[index..]);
    rewritten
}

pub(crate) fn inject_scope_attribute(selector: &str, scope_id: &str) -> String {
    let selector = strip_leading_universal_selector(selector.trim());
    let Some(index) = selector_injection_index(selector) else {
        return format!("[{scope_id}]{selector}");
    };
    let mut rewritten = String::new();
    let mut prefix_end = index;
    let mut removed_universal = false;
    if let Some(stripped) = selector[..index].strip_suffix('*') {
        if selector[index..].starts_with(['.', '#', ':', '[']) {
            prefix_end = stripped.len();
            removed_universal = true;
        }
    }
    if removed_universal {
        rewritten.push_str(&selector[..prefix_end]);
    } else {
        rewritten.push_str(selector[..prefix_end].trim_end());
    }
    rewritten.push('[');
    rewritten.push_str(scope_id);
    rewritten.push(']');
    rewritten.push_str(&selector[index..]);
    rewritten
}

pub(crate) fn strip_leading_universal_selector(selector: &str) -> &str {
    let Some(after_star) = selector.strip_prefix('*') else {
        return selector;
    };
    if after_star.is_empty() {
        return "";
    }
    if let Some(first) = after_star.chars().next() {
        if !first.is_whitespace() {
            return after_star;
        }
    }
    let whitespace_end = skip_selector_whitespace(selector, '*'.len_utf8());
    if whitespace_end >= selector.len() {
        return "";
    }
    let next = selector[whitespace_end..].chars().next();
    if next.is_some_and(|ch| {
        ch == '.'
            || ch == '#'
            || ch == '['
            || ch == ':'
            || ch == '\\'
            || is_selector_ident_start(ch)
    }) {
        &selector[whitespace_end..]
    } else {
        after_star
    }
}

pub(crate) fn selector_injection_index(selector: &str) -> Option<usize> {
    let mut state = SelectorScannerState::Normal;
    let mut last_node_end = None;
    let mut index = 0usize;
    while index < selector.len() {
        let ch = selector[index..].chars().next()?;
        match state {
            SelectorScannerState::Normal => match ch {
                '\'' => state = SelectorScannerState::SingleQuote,
                '"' => state = SelectorScannerState::DoubleQuote,
                '\\' => {
                    let end = consume_selector_token(selector, index);
                    last_node_end = Some(end);
                    index = end;
                    continue;
                }
                '/' if selector[index..].starts_with("/*") => {
                    let end = skip_selector_comment(selector, index);
                    last_node_end = Some(end);
                    index = end;
                    continue;
                }
                '[' => {
                    let Some(end) = find_matching_selector_bracket(selector, index) else {
                        return last_node_end.or(Some(selector.len()));
                    };
                    last_node_end = Some(end + 1);
                    index = end + 1;
                    continue;
                }
                ':' => {
                    let end = skip_selector_pseudo(selector, index);
                    index = end;
                    continue;
                }
                '(' => {
                    if let Some(close) = find_matching_selector_paren(selector, index) {
                        last_node_end = Some(close + 1);
                        index = close + 1;
                        continue;
                    }
                }
                '>' | '+' | '~' | ',' => {}
                '*' if last_node_end.is_none() => last_node_end = Some(index + ch.len_utf8()),
                '*' => {}
                _ if ch.is_whitespace() => {}
                '&' => {
                    last_node_end = Some(index + ch.len_utf8());
                }
                _ if is_selector_ident_start(ch) || ch == '.' || ch == '#' => {
                    let end = consume_selector_token(selector, index);
                    last_node_end = Some(end);
                    index = end;
                    continue;
                }
                _ => {}
            },
            SelectorScannerState::SingleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < selector.len() {
                        index += selector[index..].chars().next().map_or(0, char::len_utf8);
                    }
                    continue;
                }
                if ch == '\'' {
                    state = SelectorScannerState::Normal;
                }
            }
            SelectorScannerState::DoubleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < selector.len() {
                        index += selector[index..].chars().next().map_or(0, char::len_utf8);
                    }
                    continue;
                }
                if ch == '"' {
                    state = SelectorScannerState::Normal;
                }
            }
        }
        index += ch.len_utf8();
    }
    last_node_end
}

pub(crate) fn find_matching_selector_bracket(selector: &str, open: usize) -> Option<usize> {
    let mut state = SelectorScannerState::Normal;
    let mut index = open + 1;
    while index < selector.len() {
        let ch = selector[index..].chars().next()?;
        match state {
            SelectorScannerState::Normal => match ch {
                '\'' => state = SelectorScannerState::SingleQuote,
                '"' => state = SelectorScannerState::DoubleQuote,
                '\\' => {
                    index = consume_selector_escape(selector, index);
                    continue;
                }
                '/' if selector[index..].starts_with("/*") => {
                    index = skip_selector_comment(selector, index);
                    continue;
                }
                ']' => return Some(index),
                _ => {}
            },
            SelectorScannerState::SingleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < selector.len() {
                        index += selector[index..].chars().next().map_or(0, char::len_utf8);
                    }
                    continue;
                }
                if ch == '\'' {
                    state = SelectorScannerState::Normal;
                }
            }
            SelectorScannerState::DoubleQuote => {
                if ch == '\\' {
                    index += ch.len_utf8();
                    if index < selector.len() {
                        index += selector[index..].chars().next().map_or(0, char::len_utf8);
                    }
                    continue;
                }
                if ch == '"' {
                    state = SelectorScannerState::Normal;
                }
            }
        }
        index += ch.len_utf8();
    }
    None
}

pub(crate) fn skip_selector_pseudo(selector: &str, start: usize) -> usize {
    let mut index = start;
    if selector[index..].starts_with("::") {
        index += 2;
    } else {
        index += 1;
    }
    while index < selector.len() {
        let Some(ch) = selector[index..].chars().next() else {
            break;
        };
        if !(ch.is_ascii_alphanumeric() || ch == '-' || ch == '_') {
            break;
        }
        index += ch.len_utf8();
    }
    let open = skip_selector_whitespace(selector, index);
    if open < selector.len() && selector[open..].starts_with('(') {
        if let Some(close) = find_matching_selector_paren(selector, open) {
            return close + 1;
        }
    }
    index
}

pub(crate) fn consume_selector_token(selector: &str, start: usize) -> usize {
    let mut index = start;
    if selector[index..].starts_with('.') || selector[index..].starts_with('#') {
        index += 1;
    }
    while index < selector.len() {
        let Some(ch) = selector[index..].chars().next() else {
            break;
        };
        if ch == '\\' {
            index = consume_selector_escape(selector, index);
            continue;
        }
        if !is_selector_ident_continue(ch) {
            break;
        }
        index += ch.len_utf8();
    }
    index
}

pub(crate) fn is_selector_ident_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_' || ch == '-' || !ch.is_ascii()
}

pub(crate) fn is_selector_ident_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || !ch.is_ascii()
}

pub(crate) fn normalize_selector_comments(selector: &str) -> String {
    let mut output = String::with_capacity(selector.len());
    let mut state = SelectorScannerState::Normal;
    let mut index = 0usize;
    while index < selector.len() {
        let Some(ch) = selector[index..].chars().next() else {
            break;
        };
        match state {
            SelectorScannerState::Normal => match ch {
                '\'' => {
                    state = SelectorScannerState::SingleQuote;
                    output.push(ch);
                    index += ch.len_utf8();
                }
                '"' => {
                    state = SelectorScannerState::DoubleQuote;
                    output.push(ch);
                    index += ch.len_utf8();
                }
                '/' if selector[index..].starts_with("/*") => {
                    let end = skip_selector_comment(selector, index);
                    let before_is_whitespace =
                        output.chars().next_back().is_some_and(char::is_whitespace);
                    let after_is_whitespace = if end < selector.len() {
                        selector[end..]
                            .chars()
                            .next()
                            .is_some_and(char::is_whitespace)
                    } else {
                        false
                    };
                    if !before_is_whitespace && !after_is_whitespace {
                        output.push_str(&selector[index..end]);
                    }
                    index = end;
                }
                _ => {
                    output.push(ch);
                    index += ch.len_utf8();
                }
            },
            SelectorScannerState::SingleQuote => {
                output.push(ch);
                index += ch.len_utf8();
                if ch == '\\' {
                    if index < selector.len() {
                        if let Some(next) = selector[index..].chars().next() {
                            output.push(next);
                            index += next.len_utf8();
                        }
                    }
                    continue;
                }
                if ch == '\'' {
                    state = SelectorScannerState::Normal;
                }
            }
            SelectorScannerState::DoubleQuote => {
                output.push(ch);
                index += ch.len_utf8();
                if ch == '\\' {
                    if index < selector.len() {
                        if let Some(next) = selector[index..].chars().next() {
                            output.push(next);
                            index += next.len_utf8();
                        }
                    }
                    continue;
                }
                if ch == '"' {
                    state = SelectorScannerState::Normal;
                }
            }
        }
    }
    output
}

pub(crate) fn skip_selector_comment(selector: &str, start: usize) -> usize {
    selector[start + 2..]
        .find("*/")
        .map(|offset| start + 2 + offset + 2)
        .unwrap_or(selector.len())
}

pub(crate) fn consume_selector_escape(selector: &str, start: usize) -> usize {
    let mut index = start + '\\'.len_utf8();
    if index >= selector.len() {
        return index;
    }

    let mut hex_digits = 0usize;
    while index < selector.len() && hex_digits < 6 {
        let Some(ch) = selector[index..].chars().next() else {
            break;
        };
        if !ch.is_ascii_hexdigit() {
            break;
        }
        index += ch.len_utf8();
        hex_digits += 1;
    }

    if hex_digits > 0 {
        if index < selector.len() {
            if let Some(ch) = selector[index..].chars().next() {
                if ch.is_whitespace() {
                    index += ch.len_utf8();
                }
            }
        }
        return index;
    }

    index + selector[index..].chars().next().map_or(0, char::len_utf8)
}
