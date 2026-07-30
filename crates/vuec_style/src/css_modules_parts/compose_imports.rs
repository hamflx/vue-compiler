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
                let Some(result) = context.load_imported_module(import) else {
                    return unsupported_css_module_compose();
                };
                for token in &tokens[..from_index] {
                    let values = css_module_external_composed_values(token.value, &result);
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
    result: &CssModulesCompileResult,
) -> Vec<String> {
    result
        .raw_modules
        .get(class_name)
        .map(|value| value.split_whitespace().map(ToOwned::to_owned).collect())
        .unwrap_or_else(|| vec!["undefined".to_string()])
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
    load_state: &mut CssModulesImportState,
) -> Option<CssModuleResolvedImport> {
    if !load_state.validate_path(
        Path::new(import.trim()),
        "CSS Modules import specifier",
    ) {
        return None;
    }
    let import = unquote_css_module_path(import);
    let import_path = Path::new(&import);
    let importer_dir = Path::new(filename)
        .parent()
        .unwrap_or_else(|| Path::new(""));
    if import_path.is_absolute() {
        return css_module_resolved_import(
            import_path.to_path_buf(),
            import_path.to_path_buf(),
            load_state,
        );
    }

    if is_relative_css_module_import(&import) {
        let logical = importer_dir.join(import_path);
        return css_module_resolved_import(logical.clone(), logical, load_state);
    }

    let resolved = resolve_css_module_node_modules_import(&import, importer_dir, load_state);
    if resolved.is_some() || load_state.error.is_some() {
        return resolved;
    }
    if !is_safe_css_module_bare_fallback_path(&import, import_path) {
        return None;
    }
    let logical = importer_dir.join(import_path);
    css_module_resolved_import(logical.clone(), logical, load_state)
}

fn is_safe_css_module_bare_fallback_path(import: &str, path: &Path) -> bool {
    !import.is_empty()
        && !import.contains('\\')
        && path
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)))
}

pub(crate) fn css_module_resolved_import(
    path: PathBuf,
    logical_filename: PathBuf,
    load_state: &mut CssModulesImportState,
) -> Option<CssModuleResolvedImport> {
    if !load_state.validate_path(&logical_filename, "CSS Modules logical import path")
        || !load_state.is_file(&path)
    {
        return None;
    }
    let path = load_state.canonicalize(&path).unwrap_or(path);
    if load_state.error.is_some()
        || !load_state.validate_path(&path, "CSS Modules resolved import path")
    {
        return None;
    }
    Some(CssModuleResolvedImport {
        path,
        logical_filename: logical_filename.to_string_lossy().to_string(),
    })
}

pub(crate) fn is_relative_css_module_import(import: &str) -> bool {
    import.starts_with("./") || import.starts_with("../") || import == "." || import == ".."
}

pub(crate) fn resolve_css_module_node_modules_import(
    import: &str,
    importer_dir: &Path,
    load_state: &mut CssModulesImportState,
) -> Option<CssModuleResolvedImport> {
    let (package_name, subpath) = split_css_module_package_specifier(import)?;
    let start = if importer_dir.as_os_str().is_empty() {
        Path::new(".")
    } else {
        importer_dir
    };
    for dir in start.ancestors() {
        let package_dir = dir.join("node_modules").join(&package_name);
        if !load_state.is_dir(&package_dir) {
            if load_state.error.is_some() {
                return None;
            }
            continue;
        }
        let path = if subpath.as_os_str().is_empty() {
            css_module_package_main_file(&package_dir, load_state)?
        } else {
            match css_module_package_exports_file(&package_dir, &subpath, load_state) {
                CssModulePackageExportsResolution::Resolved(path) => path,
                CssModulePackageExportsResolution::Blocked => return None,
                CssModulePackageExportsResolution::NoExports => package_dir.join(&subpath),
            }
        };
        if load_state.error.is_some() {
            return None;
        }
        let logical = importer_dir.join(import);
        if let Some(resolved) = css_module_resolved_import(path, logical, load_state) {
            return Some(resolved);
        }
        if load_state.error.is_some() {
            return None;
        }
    }
    None
}

pub(crate) fn split_css_module_package_specifier(import: &str) -> Option<(String, PathBuf)> {
    if import.is_empty() || import.starts_with('/') || import.starts_with('\\') {
        return None;
    }
    let mut parts = import.split('/');
    if import.starts_with('@') {
        let scope = parts.next()?;
        let name = parts.next()?;
        if scope.len() <= 1
            || !is_safe_css_module_package_segment(scope)
            || !is_safe_css_module_package_segment(name)
        {
            return None;
        }
        let package = format!("{scope}/{name}");
        let subpath = css_module_package_subpath(parts)?;
        Some((package, subpath))
    } else {
        let package = parts.next()?;
        if !is_safe_css_module_package_segment(package) {
            return None;
        }
        let subpath = css_module_package_subpath(parts)?;
        Some((package.to_string(), subpath))
    }
}

fn css_module_package_subpath<'a>(parts: impl Iterator<Item = &'a str>) -> Option<PathBuf> {
    let mut subpath = PathBuf::new();
    for part in parts {
        if !is_safe_css_module_package_segment(part) {
            return None;
        }
        subpath.push(part);
    }
    Some(subpath)
}

fn is_safe_css_module_package_segment(segment: &str) -> bool {
    if segment.is_empty() || segment.contains('\\') {
        return false;
    }
    let mut components = Path::new(segment).components();
    matches!(components.next(), Some(std::path::Component::Normal(_)))
        && components.next().is_none()
}

pub(crate) fn css_module_package_main_file(
    package_dir: &Path,
    load_state: &mut CssModulesImportState,
) -> Option<PathBuf> {
    let package = read_css_module_package_json(package_dir, load_state);
    if load_state.error.is_some() {
        return None;
    }
    if let Some(package) = package {
        match css_module_package_exports_file_from_json(
            package_dir,
            Path::new(""),
            &package,
            load_state,
        ) {
            CssModulePackageExportsResolution::Resolved(path) => return Some(path),
            CssModulePackageExportsResolution::Blocked => return None,
            CssModulePackageExportsResolution::NoExports => {}
        }
        if let Some(main) = package.main {
            let main = Path::new(&main);
            if !is_safe_css_module_package_relative_path(main)
                || !load_state.validate_path(main, "CSS Modules package main path")
            {
                return None;
            }
            let candidate = package_dir.join(main);
            if load_state.is_file(&candidate) {
                return Some(candidate);
            }
        }
    }
    let index_css = package_dir.join("index.css");
    load_state.is_file(&index_css).then_some(index_css)
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
    load_state: &mut CssModulesImportState,
) -> CssModulePackageExportsResolution {
    let Some(package) = read_css_module_package_json(package_dir, load_state) else {
        return CssModulePackageExportsResolution::NoExports;
    };
    css_module_package_exports_file_from_json(package_dir, subpath, &package, load_state)
}

fn read_css_module_package_json(
    package_dir: &Path,
    load_state: &mut CssModulesImportState,
) -> Option<CssModulePackageJson> {
    let package_json = package_dir.join("package.json");
    let source = load_state.read_metadata(&package_json)?;
    serde_json::from_str(&source).ok()
}

fn css_module_package_exports_file_from_json(
    package_dir: &Path,
    subpath: &Path,
    package: &CssModulePackageJson,
    load_state: &mut CssModulesImportState,
) -> CssModulePackageExportsResolution {
    let Some(exports) = package.exports.as_ref() else {
        return CssModulePackageExportsResolution::NoExports;
    };
    let target = if subpath.as_os_str().is_empty() {
        css_module_package_exports_root_target(exports, load_state)
    } else {
        let key = format!(
            "./{}",
            subpath
                .to_string_lossy()
                .replace('\\', "/")
                .trim_start_matches("./")
        );
        if !load_state.validate_path(Path::new(&key), "CSS Modules package export key") {
            return CssModulePackageExportsResolution::Blocked;
        }
        css_module_package_exports_subpath_target(exports, &key, load_state)
    };
    let Some(target) = target else {
        return CssModulePackageExportsResolution::Blocked;
    };
    let Some(path) = css_module_package_export_target(package_dir, &target, load_state) else {
        return CssModulePackageExportsResolution::Blocked;
    };
    CssModulePackageExportsResolution::Resolved(path)
}

pub(crate) fn css_module_package_exports_root_target(
    exports: &CssModulePackageJsonValue,
    load_state: &mut CssModulesImportState,
) -> Option<String> {
    let target = css_module_package_export_target_value(exports).or_else(|| {
        exports.get(".").and_then(css_module_package_export_target_value)
    })?;
    if !load_state.validate_path(Path::new(target), "CSS Modules package export target") {
        return None;
    }
    Some(target.to_string())
}

pub(crate) fn css_module_package_exports_subpath_target(
    exports: &CssModulePackageJsonValue,
    key: &str,
    load_state: &mut CssModulesImportState,
) -> Option<String> {
    if let Some(target) = exports
        .get(key)
        .and_then(css_module_package_export_target_value)
    {
        if !load_state.validate_path(Path::new(target), "CSS Modules package export target") {
            return None;
        }
        return Some(target.to_string());
    }
    for (pattern, target) in exports.entries()? {
        let Some(capture) = css_module_package_export_pattern_capture(pattern, key) else {
            continue;
        };
        let target = css_module_package_export_target_value(target)?;
        let stars = target.as_bytes().iter().filter(|byte| **byte == b'*').count();
        let Some(replaced_bytes) = stars
            .checked_mul(capture.len())
            .and_then(|bytes| bytes.checked_add(target.len().saturating_sub(stars)))
        else {
            load_state.fail("CSS Modules package export target size overflowed");
            return None;
        };
        if replaced_bytes > load_state.limits.max_path_bytes {
            load_state.fail(format!(
                "CSS Modules package export target exceeds the maximum of {} bytes",
                load_state.limits.max_path_bytes
            ));
            return None;
        }
        return Some(target.replace('*', capture));
    }
    None
}

pub(crate) fn css_module_package_export_target_value(
    value: &CssModulePackageJsonValue,
) -> Option<&str> {
    if let Some(value) = value.as_str() {
        return Some(value);
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

pub(crate) fn css_module_package_export_pattern_capture<'a>(
    pattern: &str,
    key: &'a str,
) -> Option<&'a str> {
    let star = pattern.find('*')?;
    let prefix = &pattern[..star];
    let suffix = &pattern[star + 1..];
    if !key.starts_with(prefix) || !key.ends_with(suffix) || key.len() < prefix.len() + suffix.len()
    {
        return None;
    }
    Some(&key[prefix.len()..key.len() - suffix.len()])
}

pub(crate) fn css_module_package_export_target(
    package_dir: &Path,
    target: &str,
    load_state: &mut CssModulesImportState,
) -> Option<PathBuf> {
    let target_path = Path::new(target);
    if !target.starts_with("./") || !is_safe_css_module_package_relative_path(target_path) {
        return None;
    }
    let path = package_dir.join(target_path);
    load_state
        .validate_path(&path, "CSS Modules package export path")
        .then_some(path)
}

fn is_safe_css_module_package_relative_path(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && !path.is_absolute()
        && path.components().all(|component| {
            matches!(
                component,
                std::path::Component::CurDir | std::path::Component::Normal(_)
            )
        })
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
                    let end = find_matching_selector_bracket(source, index)?;
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
