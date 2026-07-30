use crate::*;

pub(crate) const STYLE_PREPROCESS_MAX_NESTING_DEPTH: usize = 128;
pub(crate) const STYLE_PREPROCESS_NESTING_ERROR: &str =
    "style preprocessor nesting exceeds the maximum supported depth";

pub(crate) struct PreprocessResult {
    pub(crate) code: String,
    pub(crate) dependencies: Vec<String>,
}

pub(crate) fn preprocess_style(
    source: &str,
    options: &StyleCompileOptions,
) -> Result<PreprocessResult, StylePreprocessError> {
    let lang = options.preprocess_lang.as_deref();
    let Some(lang) = lang.filter(|lang| !lang.is_empty()) else {
        return Ok(PreprocessResult {
            code: source.to_string(),
            dependencies: Vec::new(),
        });
    };
    let prepared = apply_additional_style_data(source, &options.preprocess_options);
    let result = match lang.to_ascii_lowercase().as_str() {
        "css" => Ok(PreprocessResult {
            code: prepared.clone(),
            dependencies: Vec::new(),
        }),
        "less" => preprocess_less(&prepared, options),
        "scss" => preprocess_sass_with_grass(&prepared, options, grass::InputSyntax::Scss)
            .map_err(StylePreprocessError::unsupported)
            .map(|code| PreprocessResult {
                code,
                dependencies: sass_dependencies(source, options),
            }),
        "sass" => preprocess_sass_with_grass(&prepared, options, grass::InputSyntax::Sass)
            .map_err(StylePreprocessError::unsupported)
            .map(|code| PreprocessResult {
                code,
                dependencies: sass_dependencies(source, options),
            }),
        "styl" | "stylus" => preprocess_stylus(&prepared, options),
        _ => Err(StylePreprocessError::unsupported(format!(
            "unsupported style preprocessor `{lang}`"
        ))),
    }?;
    Ok(result)
}

pub(crate) fn apply_additional_style_data(
    source: &str,
    options: &StylePreprocessOptions,
) -> String {
    let Some(additional_data) = options
        .additional_data
        .as_deref()
        .filter(|data| !data.is_empty())
    else {
        return source.to_string();
    };
    let mut prepared = String::with_capacity(additional_data.len() + 1 + source.len());
    prepared.push_str(additional_data);
    if !additional_data.ends_with('\n') {
        prepared.push('\n');
    }
    prepared.push_str(source);
    prepared
}

pub(crate) fn preprocess_sass_with_grass(
    source: &str,
    options: &StyleCompileOptions,
    syntax: grass::InputSyntax,
) -> Result<String, String> {
    let entry_path = options.filename.as_deref().map(PathBuf::from);
    let virtual_fs = entry_path
        .as_ref()
        .map(|path| VirtualSassFs::new(path.clone(), source.as_bytes().to_vec()));
    let mut grass_options = grass::Options::default()
        .input_syntax(syntax)
        .quiet(true)
        .allows_charset(false);
    if let Some(virtual_fs) = virtual_fs.as_ref() {
        grass_options = grass_options.fs(virtual_fs);
    }
    if let Some(filename) = options.filename.as_deref() {
        if let Some(parent) = Path::new(filename).parent() {
            grass_options = grass_options.load_path(parent);
        }
    }
    for load_path in &options.preprocess_options.load_paths {
        grass_options = grass_options.load_path(load_path);
    }
    if let Some(path) = entry_path {
        grass::from_path(path, &grass_options).map_err(|error| error.to_string())
    } else {
        grass::from_string(source.to_string(), &grass_options).map_err(|error| error.to_string())
    }
}

#[derive(Debug)]
pub(crate) struct VirtualSassFs {
    pub(crate) entry_path: PathBuf,
    pub(crate) entry_source: Vec<u8>,
}

impl VirtualSassFs {
    pub(crate) fn new(entry_path: PathBuf, entry_source: Vec<u8>) -> Self {
        Self {
            entry_path,
            entry_source,
        }
    }

    pub(crate) fn is_entry(&self, path: &Path) -> bool {
        path == self.entry_path
    }
}

impl grass::Fs for VirtualSassFs {
    fn is_dir(&self, path: &Path) -> bool {
        path.is_dir()
    }

    fn is_file(&self, path: &Path) -> bool {
        self.is_entry(path) || path.is_file()
    }

    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        if self.is_entry(path) {
            Ok(self.entry_source.clone())
        } else {
            std::fs::read(path)
        }
    }

    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        if self.is_entry(path) {
            Ok(self.entry_path.clone())
        } else {
            std::fs::canonicalize(path)
        }
    }
}

pub(crate) fn sass_dependencies(source: &str, options: &StyleCompileOptions) -> Vec<String> {
    let Some(filename) = options.filename.as_deref() else {
        return Vec::new();
    };
    let base_dir = Path::new(filename)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default();
    let mut dependencies = Vec::new();
    for import in sass_imports(source) {
        for candidate in sass_import_candidates(&base_dir, &import) {
            if candidate.exists() {
                dependencies.push(normalize_native_dependency_path(
                    &std::fs::canonicalize(&candidate).unwrap_or(candidate),
                ));
                break;
            }
        }
    }
    dependencies.sort();
    dependencies.dedup();
    dependencies
}

pub(crate) fn sass_imports(source: &str) -> Vec<String> {
    let mut imports = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with("@import")
            && !trimmed.starts_with("@use")
            && !trimmed.starts_with("@forward")
        {
            continue;
        }
        if let Some(path) = quoted_style_import_path(trimmed) {
            if !is_css_import(&path) {
                imports.push(path);
            }
        }
    }
    imports
}

pub(crate) fn quoted_style_import_path(source: &str) -> Option<String> {
    let start = source.find(['"', '\''])?;
    let quote = source[start..].chars().next()?;
    let rest = &source[start + quote.len_utf8()..];
    let end = rest.find(quote)?;
    Some(rest[..end].to_string())
}

pub(crate) fn is_css_import(path: &str) -> bool {
    path.starts_with("http://")
        || path.starts_with("https://")
        || path.ends_with(".css")
        || path.starts_with("url(")
}

pub(crate) fn sass_import_candidates(base_dir: &Path, import: &str) -> Vec<PathBuf> {
    let import_path = Path::new(import);
    let base = if import_path.is_absolute() {
        PathBuf::from(import_path)
    } else {
        base_dir.join(import_path)
    };
    let mut candidates = Vec::new();
    let has_extension = base.extension().is_some();
    if has_extension {
        candidates.push(base.clone());
        if let (Some(parent), Some(file_name)) = (base.parent(), base.file_name()) {
            candidates.push(parent.join(format!("_{}", file_name.to_string_lossy())));
        }
        return candidates;
    }
    for extension in ["scss", "sass", "css"] {
        candidates.push(base.with_extension(extension));
        if let (Some(parent), Some(file_name)) = (base.parent(), base.file_name()) {
            let partial = parent.join(format!("_{}.{}", file_name.to_string_lossy(), extension));
            candidates.push(partial);
        }
    }
    candidates
}

pub(crate) fn normalize_dependency_path(path: &Path) -> String {
    let mut value = path.to_string_lossy().replace('\\', "/");
    if let Some(stripped) = value.strip_prefix("//?/") {
        value = stripped.to_string();
    }
    value
}

pub(crate) fn normalize_native_dependency_path(path: &Path) -> String {
    let mut value = path.to_string_lossy().to_string();
    if let Some(stripped) = value.strip_prefix(r"\\?\") {
        value = stripped.to_string();
    } else if let Some(stripped) = value.strip_prefix("//?/") {
        value = stripped.to_string();
    }
    value
}

pub(crate) fn preprocess_less(
    source: &str,
    options: &StyleCompileOptions,
) -> Result<PreprocessResult, StylePreprocessError> {
    let mut context = LessImportContext::new(options);
    let base_dir = options
        .filename
        .as_deref()
        .and_then(|filename| Path::new(filename).parent())
        .map(Path::to_path_buf);
    let inlined = inline_less_imports(source, base_dir.as_deref(), &mut context, true)?;
    let nodes = parse_less_nodes(&inlined).map_err(StylePreprocessError::unsupported)?;
    Ok(PreprocessResult {
        code: render_less_nodes(&nodes, None, &[], StyleVariableSyntax::LessAt),
        dependencies: context.dependencies(),
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum LessNode {
    Rule {
        selector: String,
        children: Vec<LessNode>,
    },
    AtRuleBlock {
        prelude: String,
        children: Vec<LessNode>,
    },
    AtRuleStatement(String),
    Declaration {
        name: String,
        value: String,
    },
    Variable {
        name: String,
        value: String,
    },
    Comment,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LessVariable {
    pub(crate) name: String,
    pub(crate) value: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StyleVariableSyntax {
    LessAt,
    StylusBare,
}

#[derive(Debug)]
pub(crate) struct LessImportContext {
    pub(crate) load_paths: Vec<PathBuf>,
    pub(crate) dependencies: Vec<String>,
    pub(crate) active_paths: Vec<PathBuf>,
}

impl LessImportContext {
    pub(crate) fn new(options: &StyleCompileOptions) -> Self {
        Self {
            load_paths: options
                .preprocess_options
                .load_paths
                .iter()
                .map(PathBuf::from)
                .collect(),
            dependencies: Vec::new(),
            active_paths: Vec::new(),
        }
    }

    pub(crate) fn dependencies(mut self) -> Vec<String> {
        self.dependencies.sort();
        self.dependencies.dedup();
        self.dependencies
    }

    pub(crate) fn push_dependency(&mut self, path: &Path) {
        let normalized = normalize_dependency_path(path);
        if !self
            .dependencies
            .iter()
            .any(|existing| existing == &normalized)
        {
            self.dependencies.push(normalized);
        }
    }

    pub(crate) fn is_active(&self, path: &Path) -> bool {
        self.active_paths.iter().any(|active| active == path)
    }
}

pub(crate) fn inline_less_imports(
    source: &str,
    base_dir: Option<&Path>,
    context: &mut LessImportContext,
    spans_apply_to_source: bool,
) -> Result<String, StylePreprocessError> {
    let mut output = String::new();
    let mut cursor = 0usize;
    while cursor < source.len() {
        let Some((delimiter, delimiter_ch)) = find_next_css_delimiter(source, cursor) else {
            output.push_str(&source[cursor..]);
            break;
        };
        if delimiter_ch == '{' {
            let Some(close) = find_matching_brace(source, delimiter) else {
                output.push_str(&source[cursor..]);
                break;
            };
            output.push_str(&source[cursor..=close]);
            cursor = close + 1;
            continue;
        }

        let statement = &source[cursor..=delimiter];
        let prelude = &source[cursor..delimiter];
        let import_span = if spans_apply_to_source {
            prelude
                .find("@import")
                .map(|start| (cursor + start, delimiter + delimiter_ch.len_utf8()))
        } else {
            None
        };
        let Some(import) = parse_less_import_statement(prelude) else {
            output.push_str(statement);
            cursor = delimiter + 1;
            continue;
        };
        if is_css_import(&import) {
            output.push_str(statement);
            cursor = delimiter + 1;
            continue;
        }

        let leading_whitespace_len = prelude.len() - prelude.trim_start().len();
        output.push_str(&prelude[..leading_whitespace_len]);
        let Some(resolved) = resolve_less_import(&import, base_dir, context) else {
            return Err(StylePreprocessError::import_resolve(
                format!("Less import could not be resolved: {import}"),
                import_span,
            ));
        };
        let canonical = std::fs::canonicalize(&resolved).unwrap_or(resolved);
        context.push_dependency(&canonical);
        if context.is_active(&canonical) {
            cursor = delimiter + 1;
            continue;
        }
        context.active_paths.push(canonical.clone());
        let imported = std::fs::read_to_string(&canonical).map_err(|error| {
            StylePreprocessError::import_resolve(
                format!("Less import could not be read: {import}: {error}"),
                import_span,
            )
        })?;
        let imported_base = canonical.parent();
        let inlined = inline_less_imports(&imported, imported_base, context, false)?;
        context.active_paths.pop();
        output.push_str(&inlined);
        if !output.ends_with('\n') {
            output.push('\n');
        }
        cursor = delimiter + 1;
    }
    Ok(output)
}

pub(crate) fn parse_less_import_statement(statement: &str) -> Option<String> {
    let trimmed = statement.trim_start();
    if !trimmed.starts_with("@import") || !less_at_keyword_boundary(trimmed, "@import".len()) {
        return None;
    }
    quoted_style_import_path(trimmed)
}

pub(crate) fn less_at_keyword_boundary(source: &str, index: usize) -> bool {
    source[index..]
        .chars()
        .next()
        .is_none_or(|ch| ch.is_whitespace() || ch == '(' || ch == '"' || ch == '\'')
}

pub(crate) fn resolve_less_import(
    import: &str,
    base_dir: Option<&Path>,
    context: &LessImportContext,
) -> Option<PathBuf> {
    let import_path = Path::new(import);
    let mut bases = Vec::new();
    if import_path.is_absolute() {
        bases.push(PathBuf::from(import_path));
    } else {
        if let Some(base_dir) = base_dir {
            bases.push(base_dir.join(import_path));
        }
        bases.extend(context.load_paths.iter().map(|base| base.join(import_path)));
    }
    for base in bases {
        for candidate in less_import_candidates(&base) {
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

pub(crate) fn less_import_candidates(base: &Path) -> Vec<PathBuf> {
    if base.extension().is_some() {
        return vec![base.to_path_buf()];
    }
    vec![base.with_extension("less"), base.to_path_buf()]
}

pub(crate) fn parse_less_nodes(source: &str) -> Result<Vec<LessNode>, String> {
    parse_less_nodes_at_depth(source, 0)
}

fn parse_less_nodes_at_depth(source: &str, depth: usize) -> Result<Vec<LessNode>, String> {
    let mut nodes = Vec::new();
    let mut cursor = 0usize;
    while cursor < source.len() {
        let whitespace_start = cursor;
        cursor = skip_css_whitespace(source, cursor);
        if cursor >= source.len() {
            break;
        }
        if source[cursor..].starts_with("//") {
            cursor = skip_css_line_comment(source, cursor);
            continue;
        }
        if source[cursor..].starts_with("/*") {
            let Some(end_offset) = source[cursor + 2..].find("*/") else {
                nodes.push(LessNode::Comment);
                break;
            };
            let end = cursor + 2 + end_offset + 2;
            nodes.push(LessNode::Comment);
            cursor = end;
            continue;
        }
        if cursor > whitespace_start && source[whitespace_start..cursor].contains('\n') {
            nodes.push(LessNode::Comment);
        }

        let Some((delimiter, delimiter_ch)) = find_next_css_delimiter(source, cursor) else {
            let tail = source[cursor..].trim();
            if !tail.is_empty() {
                return Err(format!("invalid Less statement `{tail}`"));
            }
            break;
        };
        let raw_prelude = &source[cursor..delimiter];
        let prelude = raw_prelude.trim();
        if delimiter_ch == ';' {
            if !prelude.is_empty() {
                nodes.push(parse_less_statement(prelude));
            }
            cursor = delimiter + 1;
            continue;
        }

        let Some(close) = find_matching_brace(source, delimiter) else {
            return Err(format!("unclosed Less block `{prelude}`"));
        };
        let body = &source[delimiter + 1..close];
        if depth >= STYLE_PREPROCESS_MAX_NESTING_DEPTH {
            return Err(STYLE_PREPROCESS_NESTING_ERROR.to_string());
        }
        let children = parse_less_nodes_at_depth(body, depth + 1)?;
        if prelude.starts_with('@') {
            nodes.push(LessNode::AtRuleBlock {
                prelude: prelude.to_string(),
                children,
            });
        } else {
            nodes.push(LessNode::Rule {
                selector: prelude.to_string(),
                children,
            });
        }
        cursor = close + 1;
    }
    Ok(nodes)
}

pub(crate) fn parse_less_statement(statement: &str) -> LessNode {
    if let Some((name, value)) = parse_less_variable(statement) {
        return LessNode::Variable { name, value };
    }
    if statement.starts_with('@') {
        return LessNode::AtRuleStatement(statement.to_string());
    }
    if let Some(colon) = find_top_level_colon(statement) {
        return LessNode::Declaration {
            name: statement[..colon].trim().to_string(),
            value: statement[colon + 1..].trim().to_string(),
        };
    }
    LessNode::AtRuleStatement(statement.to_string())
}

pub(crate) fn parse_less_variable(statement: &str) -> Option<(String, String)> {
    let trimmed = statement.trim();
    let rest = trimmed.strip_prefix('@')?;
    let colon = find_top_level_colon(trimmed)?;
    let name = &trimmed['@'.len_utf8()..colon];
    if name.is_empty() || !is_style_identifier(name.trim()) {
        return None;
    }
    if rest[..name.len()].chars().any(char::is_whitespace) {
        return None;
    }
    Some((
        name.trim().to_string(),
        trim_style_value(trimmed[colon + 1..].trim()).to_string(),
    ))
}

pub(crate) fn render_less_nodes(
    nodes: &[LessNode],
    parent_selector: Option<&str>,
    inherited_variables: &[LessVariable],
    variable_syntax: StyleVariableSyntax,
) -> String {
    let variables = less_scope_variables(nodes, inherited_variables, variable_syntax);
    let mut output = String::new();
    if let Some(selector) = parent_selector {
        let rendered = render_less_declarations(selector, nodes, &variables, variable_syntax);
        push_less_rendered(&mut output, &rendered);
    }
    for node in nodes {
        match node {
            LessNode::Rule { selector, children } => {
                let full_selector = combine_less_selectors(parent_selector, selector);
                let rendered =
                    render_less_rule(&full_selector, children, &variables, variable_syntax);
                push_less_rendered(&mut output, &rendered);
            }
            LessNode::AtRuleBlock { prelude, children } => {
                let rendered_children =
                    render_less_nodes(children, parent_selector, &variables, variable_syntax);
                if !rendered_children.trim().is_empty() {
                    push_less_rendered(
                        &mut output,
                        &format!("{prelude} {{\n{}\n}}", indent_less_css(&rendered_children)),
                    );
                }
            }
            LessNode::AtRuleStatement(statement) if parent_selector.is_none() => {
                push_less_rendered(&mut output, &format!("{statement};"));
            }
            LessNode::Declaration { .. } | LessNode::Variable { .. } | LessNode::Comment => {}
            LessNode::AtRuleStatement(_) => {}
        }
    }
    output
}

pub(crate) fn less_scope_variables(
    nodes: &[LessNode],
    inherited_variables: &[LessVariable],
    variable_syntax: StyleVariableSyntax,
) -> Vec<LessVariable> {
    let mut variables = inherited_variables.to_vec();
    for node in nodes {
        if let LessNode::Variable { name, value } = node {
            upsert_less_variable(&mut variables, name, value.clone());
        }
    }
    let snapshot = variables.clone();
    for variable in &mut variables {
        variable.value =
            resolve_style_preprocess_value(&variable.value, &snapshot, variable_syntax);
    }
    variables
}

pub(crate) fn render_less_declarations(
    selector: &str,
    children: &[LessNode],
    variables: &[LessVariable],
    variable_syntax: StyleVariableSyntax,
) -> String {
    let declarations = children
        .iter()
        .filter_map(|node| match node {
            LessNode::Declaration { name, value } => {
                let value = resolve_style_preprocess_value(value, variables, variable_syntax);
                Some((name.as_str(), normalize_preprocessor_color(&value)))
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    if declarations.is_empty() {
        return String::new();
    }

    let mut output = String::new();
    output.push_str(selector);
    output.push_str(" {\n");
    for (name, value) in declarations {
        output.push_str("  ");
        output.push_str(name);
        output.push_str(": ");
        output.push_str(&value);
        output.push_str(";\n");
    }
    output.push('}');
    output
}

pub(crate) fn render_less_rule(
    selector: &str,
    children: &[LessNode],
    variables: &[LessVariable],
    variable_syntax: StyleVariableSyntax,
) -> String {
    let variables = less_scope_variables(children, variables, variable_syntax);
    let mut output = String::new();
    let declarations = render_less_declarations(selector, children, &variables, variable_syntax);
    push_less_rendered(&mut output, &declarations);

    for child in children {
        match child {
            LessNode::Rule {
                selector: child_selector,
                children,
            } => {
                let nested_selector = combine_less_selectors(Some(selector), child_selector);
                let rendered =
                    render_less_rule(&nested_selector, children, &variables, variable_syntax);
                push_less_rendered(&mut output, &rendered);
            }
            LessNode::AtRuleBlock { prelude, children } => {
                let rendered_children =
                    render_less_nodes(children, Some(selector), &variables, variable_syntax);
                if !rendered_children.trim().is_empty() {
                    push_less_rendered(
                        &mut output,
                        &format!("{prelude} {{\n{}\n}}", indent_less_css(&rendered_children)),
                    );
                }
            }
            LessNode::AtRuleStatement(statement) => {
                push_less_rendered(&mut output, &format!("{statement};"));
            }
            LessNode::Declaration { .. } | LessNode::Variable { .. } | LessNode::Comment => {}
        }
    }
    output
}

pub(crate) fn combine_less_selectors(parent: Option<&str>, selector: &str) -> String {
    let selector = selector.trim();
    let Some(parent) = parent.map(str::trim).filter(|parent| !parent.is_empty()) else {
        return selector.to_string();
    };
    let mut selectors = Vec::new();
    for parent_branch in split_selector_list(parent) {
        let parent_branch = parent_branch.trim();
        for child_branch in split_selector_list(selector) {
            let child_branch = child_branch.trim();
            if child_branch.contains('&') {
                selectors.push(child_branch.replace('&', parent_branch));
            } else {
                selectors.push(format!("{parent_branch} {child_branch}"));
            }
        }
    }
    selectors.join(", ")
}

pub(crate) fn resolve_style_preprocess_value(
    value: &str,
    variables: &[LessVariable],
    variable_syntax: StyleVariableSyntax,
) -> String {
    let mut output = value.to_string();
    for _ in 0..8 {
        let rewritten = match variable_syntax {
            StyleVariableSyntax::LessAt => replace_less_variables_once(&output, variables),
            StyleVariableSyntax::StylusBare => replace_stylus_variables_once(&output, variables),
        };
        if rewritten == output {
            return rewritten;
        }
        output = rewritten;
    }
    output
}

pub(crate) fn replace_stylus_variables_once(source: &str, variables: &[LessVariable]) -> String {
    let mut output = source.to_string();
    for variable in variables {
        output = replace_style_identifier(&output, &variable.name, &variable.value);
        output = replace_style_dollar_identifier(&output, &variable.name, &variable.value);
    }
    output
}

pub(crate) fn replace_style_dollar_identifier(source: &str, name: &str, value: &str) -> String {
    let needle = format!("${name}");
    let mut output = String::new();
    let mut cursor = 0usize;
    while let Some(relative) = source[cursor..].find(&needle) {
        let start = cursor + relative;
        let end = start + needle.len();
        let after = source[end..].chars().next();
        if after.is_none_or(|ch| !is_style_identifier_char(ch)) {
            output.push_str(&source[cursor..start]);
            output.push_str(value);
            cursor = end;
        } else {
            output.push_str(&source[cursor..end]);
            cursor = end;
        }
    }
    output.push_str(&source[cursor..]);
    output
}

pub(crate) fn replace_less_variables_once(source: &str, variables: &[LessVariable]) -> String {
    let mut output = String::new();
    let mut cursor = 0usize;
    while cursor < source.len() {
        let Some(relative) = source[cursor..].find('@') else {
            output.push_str(&source[cursor..]);
            break;
        };
        let start = cursor + relative;
        output.push_str(&source[cursor..start]);
        let name_start = start + '@'.len_utf8();
        let name_end = consume_less_variable_name(source, name_start);
        if name_end == name_start {
            output.push('@');
            cursor = name_start;
            continue;
        }
        let name = &source[name_start..name_end];
        if let Some(value) = lookup_less_variable(name, variables) {
            output.push_str(value);
        } else {
            output.push_str(&source[start..name_end]);
        }
        cursor = name_end;
    }
    output
}

pub(crate) fn consume_less_variable_name(source: &str, mut index: usize) -> usize {
    while index < source.len() {
        let Some(ch) = source[index..].chars().next() else {
            break;
        };
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            index += ch.len_utf8();
        } else {
            break;
        }
    }
    index
}

pub(crate) fn lookup_less_variable<'a>(
    name: &str,
    variables: &'a [LessVariable],
) -> Option<&'a str> {
    variables
        .iter()
        .rev()
        .find_map(|variable| (variable.name == name).then_some(variable.value.as_str()))
}

pub(crate) fn upsert_less_variable(variables: &mut Vec<LessVariable>, name: &str, value: String) {
    if let Some(variable) = variables
        .iter_mut()
        .rev()
        .find(|variable| variable.name == name)
    {
        variable.value = value;
    } else {
        variables.push(LessVariable {
            name: name.to_string(),
            value,
        });
    }
}

pub(crate) fn push_less_rendered(output: &mut String, rendered: &str) {
    if rendered.trim().is_empty() {
        return;
    }
    if !output.is_empty() && !output.ends_with('\n') {
        output.push('\n');
    }
    if !output.is_empty() {
        output.push('\n');
    }
    output.push_str(rendered.trim());
}

pub(crate) fn indent_less_css(source: &str) -> String {
    source
        .lines()
        .map(|line| {
            if line.trim().is_empty() {
                String::new()
            } else {
                format!("  {line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn preprocess_stylus(
    source: &str,
    options: &StyleCompileOptions,
) -> Result<PreprocessResult, StylePreprocessError> {
    let mut context = LessImportContext::new(options);
    let base_dir = options
        .filename
        .as_deref()
        .and_then(|filename| Path::new(filename).parent())
        .map(Path::to_path_buf);
    let inlined = inline_stylus_imports(source, base_dir.as_deref(), &mut context, true)?;
    let nodes = parse_stylus_nodes(&inlined).map_err(StylePreprocessError::unsupported)?;
    Ok(PreprocessResult {
        code: render_less_nodes(&nodes, None, &[], StyleVariableSyntax::StylusBare)
            .replace("#ff0000", "#f00"),
        dependencies: context.dependencies(),
    })
}

pub(crate) fn inline_stylus_imports(
    source: &str,
    base_dir: Option<&Path>,
    context: &mut LessImportContext,
    spans_apply_to_source: bool,
) -> Result<String, StylePreprocessError> {
    let mut output = String::new();
    let mut line_start = 0usize;
    for line in source.split_inclusive('\n') {
        let content = line.trim_end_matches(['\r', '\n']);
        let Some(import) = parse_stylus_import_statement(line) else {
            output.push_str(line);
            if !line.ends_with('\n') {
                output.push('\n');
            }
            line_start += line.len();
            continue;
        };
        let import_span = if spans_apply_to_source {
            content
                .find("@import")
                .map(|start| (line_start + start, line_start + content.len()))
        } else {
            None
        };
        if is_css_import(&import) {
            output.push_str(line);
            if !line.ends_with('\n') {
                output.push('\n');
            }
            line_start += line.len();
            continue;
        }
        let Some(resolved) = resolve_stylus_import(&import, base_dir, context) else {
            return Err(StylePreprocessError::import_resolve(
                format!("Stylus import could not be resolved: {import}"),
                import_span,
            ));
        };
        let canonical = std::fs::canonicalize(&resolved).unwrap_or(resolved);
        context.push_dependency(&canonical);
        if context.is_active(&canonical) {
            line_start += line.len();
            continue;
        }
        context.active_paths.push(canonical.clone());
        let imported = std::fs::read_to_string(&canonical).map_err(|error| {
            StylePreprocessError::import_resolve(
                format!("Stylus import could not be read: {import}: {error}"),
                import_span,
            )
        })?;
        let imported_base = canonical.parent();
        let inlined = inline_stylus_imports(&imported, imported_base, context, false)?;
        context.active_paths.pop();
        output.push_str(&inlined);
        if !output.ends_with('\n') {
            output.push('\n');
        }
        line_start += line.len();
    }
    Ok(output)
}

pub(crate) fn parse_stylus_import_statement(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with("@import") || !less_at_keyword_boundary(trimmed, "@import".len()) {
        return None;
    }
    if let Some(path) = quoted_style_import_path(trimmed) {
        return Some(path);
    }
    let rest = trimmed["@import".len()..].trim();
    if rest.is_empty() {
        return None;
    }
    Some(rest.trim_end_matches(';').trim().to_string())
}

pub(crate) fn resolve_stylus_import(
    import: &str,
    base_dir: Option<&Path>,
    context: &LessImportContext,
) -> Option<PathBuf> {
    let import_path = Path::new(import);
    let mut bases = Vec::new();
    if import_path.is_absolute() {
        bases.push(PathBuf::from(import_path));
    } else {
        if let Some(base_dir) = base_dir {
            bases.push(base_dir.join(import_path));
        }
        bases.extend(context.load_paths.iter().map(|base| base.join(import_path)));
    }
    for base in bases {
        for candidate in stylus_import_candidates(&base) {
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

pub(crate) fn stylus_import_candidates(base: &Path) -> Vec<PathBuf> {
    if base.extension().is_some() {
        return vec![base.to_path_buf()];
    }
    vec![
        base.with_extension("styl"),
        base.with_extension("stylus"),
        base.to_path_buf(),
    ]
}

pub(crate) fn parse_stylus_nodes(source: &str) -> Result<Vec<LessNode>, String> {
    let lines = source
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("//") {
                return None;
            }
            Some(StylusLine {
                number: index + 1,
                indent: stylus_indent_width(line),
                text: trimmed.trim_end_matches(';').trim().to_string(),
            })
        })
        .collect::<Vec<_>>();
    let mut cursor = 0usize;
    parse_stylus_block(&lines, &mut cursor, 0)
}

#[derive(Debug)]
pub(crate) struct StylusLine {
    pub(crate) number: usize,
    pub(crate) indent: usize,
    pub(crate) text: String,
}

pub(crate) fn parse_stylus_block(
    lines: &[StylusLine],
    cursor: &mut usize,
    indent: usize,
) -> Result<Vec<LessNode>, String> {
    parse_stylus_block_at_depth(lines, cursor, indent, 0)
}

fn parse_stylus_block_at_depth(
    lines: &[StylusLine],
    cursor: &mut usize,
    indent: usize,
    depth: usize,
) -> Result<Vec<LessNode>, String> {
    let mut nodes = Vec::new();
    while *cursor < lines.len() {
        let line = &lines[*cursor];
        if line.indent < indent {
            break;
        }
        if line.indent > indent {
            return Err(format!(
                "unexpected Stylus indentation on line {}",
                line.number
            ));
        }
        let text = line.text.trim();
        *cursor += 1;
        if let Some((name, value)) = parse_stylus_variable(text) {
            nodes.push(LessNode::Variable { name, value });
            continue;
        }
        let has_children = *cursor < lines.len() && lines[*cursor].indent > line.indent;
        if has_children {
            if depth >= STYLE_PREPROCESS_MAX_NESTING_DEPTH {
                return Err(STYLE_PREPROCESS_NESTING_ERROR.to_string());
            }
            let children =
                parse_stylus_block_at_depth(lines, cursor, lines[*cursor].indent, depth + 1)?;
            if text.starts_with('@') {
                nodes.push(LessNode::AtRuleBlock {
                    prelude: text.to_string(),
                    children,
                });
            } else {
                nodes.push(LessNode::Rule {
                    selector: text.to_string(),
                    children,
                });
            }
            continue;
        }
        if let Some((name, value)) = parse_stylus_declaration(text) {
            nodes.push(LessNode::Declaration { name, value });
            continue;
        }
        if text.starts_with('@') && !text.starts_with("@media") && !text.starts_with("@supports") {
            nodes.push(LessNode::AtRuleStatement(text.to_string()));
            continue;
        }
        if text.starts_with('@') {
            nodes.push(LessNode::AtRuleStatement(text.to_string()));
        } else {
            nodes.push(LessNode::Rule {
                selector: text.to_string(),
                children: Vec::new(),
            });
        }
    }
    Ok(nodes)
}

pub(crate) fn parse_stylus_variable(text: &str) -> Option<(String, String)> {
    let (raw_name, raw_value) = text.split_once('=')?;
    let name = raw_name.trim().trim_start_matches('$');
    if !is_style_identifier(name) {
        return None;
    }
    Some((
        name.to_string(),
        trim_style_value(raw_value.trim()).to_string(),
    ))
}

pub(crate) fn parse_stylus_declaration(text: &str) -> Option<(String, String)> {
    if let Some((name, value)) = text.split_once(':') {
        let name = name.trim();
        if is_style_property_name(name) {
            return Some((name.to_string(), trim_style_value(value.trim()).to_string()));
        }
    }
    let mut parts = text.splitn(2, char::is_whitespace);
    let name = parts.next()?.trim();
    let value = parts.next()?.trim();
    if is_style_property_name(name) && !value.is_empty() {
        return Some((name.to_string(), trim_style_value(value).to_string()));
    }
    None
}

pub(crate) fn stylus_indent_width(line: &str) -> usize {
    let mut width = 0usize;
    for ch in line.chars() {
        match ch {
            ' ' => width += 1,
            '\t' => width += 2,
            _ => break,
        }
    }
    width
}

pub(crate) fn is_style_property_name(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '-' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '-' || ch.is_ascii_alphanumeric())
}

pub(crate) fn replace_style_identifier(source: &str, name: &str, value: &str) -> String {
    let mut output = String::new();
    let mut cursor = 0usize;
    while let Some(relative) = source[cursor..].find(name) {
        let start = cursor + relative;
        let end = start + name.len();
        let before = source[..start].chars().next_back();
        let after = source[end..].chars().next();
        if before.is_none_or(|ch| !is_style_identifier_char(ch))
            && after.is_none_or(|ch| !is_style_identifier_char(ch))
        {
            output.push_str(&source[cursor..start]);
            output.push_str(value);
            cursor = end;
        } else {
            output.push_str(&source[cursor..end]);
            cursor = end;
        }
    }
    output.push_str(&source[cursor..]);
    output
}

pub(crate) fn trim_style_value(value: &str) -> &str {
    value.trim_end_matches(';').trim()
}

pub(crate) fn normalize_preprocessor_color(value: &str) -> String {
    match value.trim() {
        "rgb(255, 0, 0)" => "#ff0000".into(),
        "red" => "red".into(),
        other => other.to_string(),
    }
}

pub(crate) fn is_style_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first == '-' || first.is_ascii_alphabetic())
        && chars.all(is_style_identifier_char)
}

pub(crate) fn is_style_identifier_char(ch: char) -> bool {
    ch == '_' || ch == '-' || ch.is_ascii_alphanumeric()
}
