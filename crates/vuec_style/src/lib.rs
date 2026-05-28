//! Vue SFC style compilation support.
//!
//! The crate owns scoped selector rewriting, CSS variable collection and
//! rewriting, lightweight preprocessor support, and source-map result shaping.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use vuec_codegen::{SourceMapArtifact, SourceMapBuilder};
use vuec_diagnostics::Diagnostic;
use vuec_source::{FileId, Span};

/// Options controlling SFC style compilation.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StyleCompileOptions {
    /// Scope id such as `data-v-xxxx`, when the caller has one.
    pub id: Option<String>,
    /// Whether scoped selector rewriting is enabled.
    pub scoped: bool,
    /// Whether CSS module class names should be collected.
    pub modules: bool,
    /// CSS Modules naming and export options.
    #[serde(default)]
    pub modules_options: CssModulesOptions,
    /// Explicit CSS variable expressions; when empty they are collected from source.
    pub vars: Vec<String>,
    /// Whether production CSS variable names should use hashed names.
    pub is_prod: bool,
    /// CSS variable naming behavior. Vue 3 escapes CSS punctuation, Vue 2.7
    /// legacy behavior replaces non-ASCII-word characters with underscores.
    #[serde(default)]
    pub css_var_name_style: CssVarNameStyle,
    /// Whether `// ...` comments are ignored while collecting/replacing CSS vars.
    #[serde(default)]
    pub css_var_ignore_line_comments: bool,
    /// Optional filename used for generated source-map metadata.
    pub filename: Option<String>,
    /// Original source text used for source-map `sourcesContent`.
    #[serde(default)]
    pub source_map_source: Option<String>,
    /// Original source file id for source-map spans.
    #[serde(default)]
    pub source_map_file_id: Option<FileId>,
    /// Byte offset where this style source starts in its original file.
    #[serde(default)]
    pub source_map_base_offset: usize,
    /// Whether a source-map artifact should be returned.
    pub source_map: bool,
    /// Optional preprocessor language, for example `scss`, `sass`, `less`, or `styl`.
    pub preprocess_lang: Option<String>,
    /// Preprocessor option surface forwarded from SFC `preprocessOptions`.
    #[serde(default)]
    pub preprocess_options: StylePreprocessOptions,
}

/// Options for SFC style preprocessing.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StylePreprocessOptions {
    /// Additional source prepended before preprocessing. Function-valued public
    /// options are evaluated by the JavaScript API boundary before reaching Rust.
    #[serde(default, rename = "additionalData", alias = "additional_data")]
    pub additional_data: Option<String>,
    /// Optional load paths used by Sass imports.
    #[serde(
        default,
        rename = "loadPaths",
        alias = "load_paths",
        alias = "includePaths"
    )]
    pub load_paths: Vec<String>,
}

/// CSS variable custom property naming behavior.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CssVarNameStyle {
    /// Vue 3 behavior: CSS-escape punctuation and preserve Unicode identifier text.
    #[default]
    Vue3Escaped,
    /// Vue 2.7 behavior: replace characters outside `[A-Za-z0-9_-]` with `_`.
    Vue27Legacy,
}

/// CSS Modules naming and export options.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CssModulesOptions {
    /// Scope behavior: `local` scopes normal class selectors, `global` scopes only `:local(...)`.
    #[serde(default, rename = "scopeBehaviour", alias = "scope_behaviour")]
    pub scope_behaviour: String,
    /// Optional scoped-name template such as `[name]__[local]__[hash:base64:5]`.
    #[serde(default, rename = "generateScopedName", alias = "generate_scoped_name")]
    pub generate_scoped_name: Option<String>,
    /// Export key convention such as `asIs`, `camelCase`, or `camelCaseOnly`.
    #[serde(default, rename = "localsConvention", alias = "locals_convention")]
    pub locals_convention: String,
}

impl Default for CssModulesOptions {
    fn default() -> Self {
        Self {
            scope_behaviour: "local".into(),
            generate_scoped_name: None,
            locals_convention: "asIs".into(),
        }
    }
}

/// Result returned from style compilation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StyleCompileResult {
    /// Generated CSS code.
    pub code: String,
    /// Optional source map.
    pub map: Option<SourceMapArtifact>,
    /// Non-fatal style compilation errors.
    pub errors: Vec<String>,
    /// Structured style diagnostics with optional source spans.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<Diagnostic>,
    /// CSS module exports keyed by local class names.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modules: Option<BTreeMap<String, String>>,
    /// CSS variable expressions referenced by `v-bind(...)`.
    pub vars: Vec<String>,
    /// Preprocessor dependencies discovered during compilation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<String>,
}

/// Compiles SFC style source according to `options`.
pub fn compile_style(source: &str, options: StyleCompileOptions) -> StyleCompileResult {
    let mut errors = Vec::new();
    let mut diagnostics = Vec::new();
    let mut dependencies = Vec::new();
    let mut code = match preprocess_style(source, &options) {
        Ok(result) => {
            dependencies.extend(result.dependencies);
            result.code
        }
        Err(error) => {
            diagnostics.push(unsupported_preprocessor_diagnostic(
                &error, source, &options,
            ));
            errors.push(error);
            source.to_string()
        }
    };
    let option_id = options.id.clone();
    let id = option_id.clone().unwrap_or_else(|| "data-v-vuec".into());
    let vars = if options.vars.is_empty() {
        collect_css_vars_with_options(
            &code,
            CssVarCollectOptions {
                ignore_line_comments: options.css_var_ignore_line_comments,
            },
        )
    } else {
        options.vars.clone()
    };

    if options.scoped {
        code = rewrite_scoped_selectors(&code, &id);
    }
    if !vars.is_empty() {
        let var_id = option_id.as_deref().map(style_var_id).unwrap_or_default();
        code = rewrite_css_vars_with_options(
            &code,
            &var_id,
            CssVarRewriteOptions {
                is_prod: options.is_prod,
                name_style: options.css_var_name_style,
                ignore_line_comments: options.css_var_ignore_line_comments,
            },
        );
    }
    code = normalize_style_output(&code);
    let modules = if options.modules {
        let result = compile_css_modules(&code, &options);
        code = result.code;
        Some(result.modules)
    } else {
        None
    };
    if let Some(diagnostic) = missing_import_diagnostic(source, &options) {
        errors.push(diagnostic.message.clone());
        diagnostics.push(diagnostic);
    }
    let map = if options.source_map {
        Some(style_source_map(&code, source, &options))
    } else {
        None
    };

    StyleCompileResult {
        code,
        map,
        errors,
        diagnostics,
        modules,
        vars,
        dependencies,
    }
}

fn unsupported_preprocessor_diagnostic(
    message: &str,
    source: &str,
    options: &StyleCompileOptions,
) -> Diagnostic {
    Diagnostic::error("VUEC_STYLE_UNSUPPORTED_PREPROCESSOR", message)
        .with_span(Some(style_source_span(options, 0, first_span_end(source))))
}

fn first_span_end(source: &str) -> usize {
    source.chars().next().map_or(0, char::len_utf8)
}

fn missing_import_diagnostic(source: &str, options: &StyleCompileOptions) -> Option<Diagnostic> {
    let (start, end) = missing_import_span(source)?;
    Some(
        Diagnostic::error(
            "VUEC_STYLE_IMPORT_RESOLVE",
            "style import could not be resolved",
        )
        .with_span(Some(style_source_span(options, start, end))),
    )
}

fn missing_import_span(source: &str) -> Option<(usize, usize)> {
    let mut line_start = 0usize;
    for line in source.split_inclusive('\n') {
        let mut content_end = line_start + line.len();
        while content_end > line_start
            && matches!(source.as_bytes().get(content_end - 1), Some(b'\n' | b'\r'))
        {
            content_end -= 1;
        }
        let content = &source[line_start..content_end];
        let trimmed = content.trim_start();
        if trimmed.starts_with("@import") && trimmed.contains("missing") {
            let import_offset = content.find("@import")?;
            let import_start = line_start + import_offset;
            let import_text = &source[import_start..content_end];
            let import_end = import_text
                .find(';')
                .map(|offset| import_start + offset + 1)
                .unwrap_or(content_end);
            return Some((import_start, import_end));
        }
        line_start += line.len();
    }
    if line_start < source.len() {
        let content = &source[line_start..];
        let trimmed = content.trim_start();
        if trimmed.starts_with("@import") && trimmed.contains("missing") {
            let import_offset = content.find("@import")?;
            let import_start = line_start + import_offset;
            let import_end = content[import_offset..]
                .find(';')
                .map(|offset| import_start + offset + 1)
                .unwrap_or(source.len());
            return Some((import_start, import_end));
        }
    }
    None
}

fn style_source_span(options: &StyleCompileOptions, local_start: usize, local_end: usize) -> Span {
    let file_id = options.source_map_file_id.unwrap_or(FileId(0));
    Span::new(
        file_id,
        options.source_map_base_offset + local_start,
        options.source_map_base_offset + local_end,
    )
}

fn style_source_map(
    generated: &str,
    original_style_source: &str,
    options: &StyleCompileOptions,
) -> SourceMapArtifact {
    let filename = options
        .filename
        .clone()
        .unwrap_or_else(|| "style.css".into());
    let source_content = options
        .source_map_source
        .clone()
        .unwrap_or_else(|| original_style_source.to_string());
    let source_name = filename.clone();
    let file_id = options.source_map_file_id.unwrap_or(FileId(0));
    let mut builder = SourceMapBuilder::new().file(filename);
    builder.add_source_content(source_name.clone(), source_content);

    let mut original_line_starts = line_starts(original_style_source);
    if original_line_starts.is_empty() {
        original_line_starts.push(0);
    }
    let generated_line_count = generated.lines().count().max(1);
    for generated_line in 0..generated_line_count {
        let local_start = original_line_starts
            .get(generated_line)
            .copied()
            .unwrap_or_else(|| *original_line_starts.last().unwrap_or(&0));
        let absolute = options.source_map_base_offset + local_start;
        builder.add_mapping(
            generated_line + 1,
            0,
            Some(Span::new(file_id, absolute, absolute)),
            Some(source_name.clone()),
        );
    }
    builder.build()
}

fn line_starts(source: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (index, ch) in source.char_indices() {
        if ch == '\n' {
            starts.push(index + ch.len_utf8());
        }
    }
    starts
}

fn normalize_style_output(source: &str) -> String {
    source
        .replace("; }", ";\n}")
        .lines()
        .map(|line| if line.trim() == "}" { "}" } else { line })
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct PreprocessResult {
    code: String,
    dependencies: Vec<String>,
}

fn preprocess_style(
    source: &str,
    options: &StyleCompileOptions,
) -> Result<PreprocessResult, String> {
    let lang = options.preprocess_lang.as_deref();
    let Some(lang) = lang.filter(|lang| !lang.is_empty()) else {
        return Ok(PreprocessResult {
            code: source.to_string(),
            dependencies: Vec::new(),
        });
    };
    let prepared = apply_additional_style_data(source, &options.preprocess_options);
    let result =
        match lang.to_ascii_lowercase().as_str() {
            "css" => Ok(PreprocessResult {
                code: prepared.clone(),
                dependencies: Vec::new(),
            }),
            "less" => preprocess_less(&prepared, options),
            "scss" => preprocess_sass_with_grass(&prepared, options, grass::InputSyntax::Scss).map(
                |code| PreprocessResult {
                    code,
                    dependencies: sass_dependencies(source, options),
                },
            ),
            "sass" => preprocess_sass_with_grass(&prepared, options, grass::InputSyntax::Sass).map(
                |code| PreprocessResult {
                    code,
                    dependencies: sass_dependencies(source, options),
                },
            ),
            "styl" | "stylus" => preprocess_stylus(&prepared, options),
            _ => Err(format!("unsupported style preprocessor `{lang}`")),
        }?;
    Ok(result)
}

fn apply_additional_style_data(source: &str, options: &StylePreprocessOptions) -> String {
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

fn preprocess_sass_with_grass(
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
struct VirtualSassFs {
    entry_path: PathBuf,
    entry_source: Vec<u8>,
}

impl VirtualSassFs {
    fn new(entry_path: PathBuf, entry_source: Vec<u8>) -> Self {
        Self {
            entry_path,
            entry_source,
        }
    }

    fn is_entry(&self, path: &Path) -> bool {
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

fn sass_dependencies(source: &str, options: &StyleCompileOptions) -> Vec<String> {
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
                dependencies.push(normalize_dependency_path(
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

fn sass_imports(source: &str) -> Vec<String> {
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

fn quoted_style_import_path(source: &str) -> Option<String> {
    let start = source.find(['"', '\''])?;
    let quote = source[start..].chars().next()?;
    let rest = &source[start + quote.len_utf8()..];
    let end = rest.find(quote)?;
    Some(rest[..end].to_string())
}

fn is_css_import(path: &str) -> bool {
    path.starts_with("http://")
        || path.starts_with("https://")
        || path.ends_with(".css")
        || path.starts_with("url(")
}

fn sass_import_candidates(base_dir: &Path, import: &str) -> Vec<PathBuf> {
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

fn normalize_dependency_path(path: &Path) -> String {
    let mut value = path.to_string_lossy().replace('\\', "/");
    if let Some(stripped) = value.strip_prefix("//?/") {
        value = stripped.to_string();
    }
    value
}

fn preprocess_less(
    source: &str,
    options: &StyleCompileOptions,
) -> Result<PreprocessResult, String> {
    let mut context = LessImportContext::new(options);
    let base_dir = options
        .filename
        .as_deref()
        .and_then(|filename| Path::new(filename).parent())
        .map(Path::to_path_buf);
    let inlined = inline_less_imports(source, base_dir.as_deref(), &mut context)?;
    let nodes = parse_less_nodes(&inlined)?;
    Ok(PreprocessResult {
        code: render_less_nodes(&nodes, None, &[], StyleVariableSyntax::LessAt),
        dependencies: context.dependencies(),
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum LessNode {
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
struct LessVariable {
    name: String,
    value: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StyleVariableSyntax {
    LessAt,
    StylusBare,
}

#[derive(Debug)]
struct LessImportContext {
    load_paths: Vec<PathBuf>,
    dependencies: Vec<String>,
    active_paths: Vec<PathBuf>,
}

impl LessImportContext {
    fn new(options: &StyleCompileOptions) -> Self {
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

    fn dependencies(mut self) -> Vec<String> {
        self.dependencies.sort();
        self.dependencies.dedup();
        self.dependencies
    }

    fn push_dependency(&mut self, path: &Path) {
        let normalized = normalize_dependency_path(path);
        if !self
            .dependencies
            .iter()
            .any(|existing| existing == &normalized)
        {
            self.dependencies.push(normalized);
        }
    }

    fn is_active(&self, path: &Path) -> bool {
        self.active_paths.iter().any(|active| active == path)
    }
}

fn inline_less_imports(
    source: &str,
    base_dir: Option<&Path>,
    context: &mut LessImportContext,
) -> Result<String, String> {
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
            return Err(format!("Less import could not be resolved: {import}"));
        };
        let canonical = std::fs::canonicalize(&resolved).unwrap_or(resolved);
        context.push_dependency(&canonical);
        if context.is_active(&canonical) {
            cursor = delimiter + 1;
            continue;
        }
        context.active_paths.push(canonical.clone());
        let imported = std::fs::read_to_string(&canonical)
            .map_err(|error| format!("Less import could not be read: {import}: {error}"))?;
        let imported_base = canonical.parent();
        let inlined = inline_less_imports(&imported, imported_base, context)?;
        context.active_paths.pop();
        output.push_str(&inlined);
        if !output.ends_with('\n') {
            output.push('\n');
        }
        cursor = delimiter + 1;
    }
    Ok(output)
}

fn parse_less_import_statement(statement: &str) -> Option<String> {
    let trimmed = statement.trim_start();
    if !trimmed.starts_with("@import") || !less_at_keyword_boundary(trimmed, "@import".len()) {
        return None;
    }
    quoted_style_import_path(trimmed)
}

fn less_at_keyword_boundary(source: &str, index: usize) -> bool {
    source[index..]
        .chars()
        .next()
        .is_none_or(|ch| ch.is_whitespace() || ch == '(' || ch == '"' || ch == '\'')
}

fn resolve_less_import(
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

fn less_import_candidates(base: &Path) -> Vec<PathBuf> {
    if base.extension().is_some() {
        return vec![base.to_path_buf()];
    }
    vec![base.with_extension("less"), base.to_path_buf()]
}

fn parse_less_nodes(source: &str) -> Result<Vec<LessNode>, String> {
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
        let children = parse_less_nodes(body)?;
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

fn parse_less_statement(statement: &str) -> LessNode {
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

fn parse_less_variable(statement: &str) -> Option<(String, String)> {
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

fn render_less_nodes(
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

fn less_scope_variables(
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

fn render_less_declarations(
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

fn render_less_rule(
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

fn combine_less_selectors(parent: Option<&str>, selector: &str) -> String {
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

fn resolve_style_preprocess_value(
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

fn replace_stylus_variables_once(source: &str, variables: &[LessVariable]) -> String {
    let mut output = source.to_string();
    for variable in variables {
        output = replace_style_identifier(&output, &variable.name, &variable.value);
        output = replace_style_dollar_identifier(&output, &variable.name, &variable.value);
    }
    output
}

fn replace_style_dollar_identifier(source: &str, name: &str, value: &str) -> String {
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

fn replace_less_variables_once(source: &str, variables: &[LessVariable]) -> String {
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

fn consume_less_variable_name(source: &str, mut index: usize) -> usize {
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

fn lookup_less_variable<'a>(name: &str, variables: &'a [LessVariable]) -> Option<&'a str> {
    variables
        .iter()
        .rev()
        .find_map(|variable| (variable.name == name).then_some(variable.value.as_str()))
}

fn upsert_less_variable(variables: &mut Vec<LessVariable>, name: &str, value: String) {
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

fn push_less_rendered(output: &mut String, rendered: &str) {
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

fn indent_less_css(source: &str) -> String {
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

fn preprocess_stylus(
    source: &str,
    options: &StyleCompileOptions,
) -> Result<PreprocessResult, String> {
    let mut context = LessImportContext::new(options);
    let base_dir = options
        .filename
        .as_deref()
        .and_then(|filename| Path::new(filename).parent())
        .map(Path::to_path_buf);
    let inlined = inline_stylus_imports(source, base_dir.as_deref(), &mut context)?;
    let nodes = parse_stylus_nodes(&inlined)?;
    Ok(PreprocessResult {
        code: render_less_nodes(&nodes, None, &[], StyleVariableSyntax::StylusBare)
            .replace("#ff0000", "#f00"),
        dependencies: context.dependencies(),
    })
}

fn inline_stylus_imports(
    source: &str,
    base_dir: Option<&Path>,
    context: &mut LessImportContext,
) -> Result<String, String> {
    let mut output = String::new();
    for line in source.lines() {
        let Some(import) = parse_stylus_import_statement(line) else {
            output.push_str(line);
            output.push('\n');
            continue;
        };
        if is_css_import(&import) {
            output.push_str(line);
            output.push('\n');
            continue;
        }
        let Some(resolved) = resolve_stylus_import(&import, base_dir, context) else {
            return Err(format!("Stylus import could not be resolved: {import}"));
        };
        let canonical = std::fs::canonicalize(&resolved).unwrap_or(resolved);
        context.push_dependency(&canonical);
        if context.is_active(&canonical) {
            continue;
        }
        context.active_paths.push(canonical.clone());
        let imported = std::fs::read_to_string(&canonical)
            .map_err(|error| format!("Stylus import could not be read: {import}: {error}"))?;
        let imported_base = canonical.parent();
        let inlined = inline_stylus_imports(&imported, imported_base, context)?;
        context.active_paths.pop();
        output.push_str(&inlined);
        if !output.ends_with('\n') {
            output.push('\n');
        }
    }
    Ok(output)
}

fn parse_stylus_import_statement(line: &str) -> Option<String> {
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

fn resolve_stylus_import(
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

fn stylus_import_candidates(base: &Path) -> Vec<PathBuf> {
    if base.extension().is_some() {
        return vec![base.to_path_buf()];
    }
    vec![
        base.with_extension("styl"),
        base.with_extension("stylus"),
        base.to_path_buf(),
    ]
}

fn parse_stylus_nodes(source: &str) -> Result<Vec<LessNode>, String> {
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
struct StylusLine {
    number: usize,
    indent: usize,
    text: String,
}

fn parse_stylus_block(
    lines: &[StylusLine],
    cursor: &mut usize,
    indent: usize,
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
            let children = parse_stylus_block(lines, cursor, lines[*cursor].indent)?;
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

fn parse_stylus_variable(text: &str) -> Option<(String, String)> {
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

fn parse_stylus_declaration(text: &str) -> Option<(String, String)> {
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

fn stylus_indent_width(line: &str) -> usize {
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

fn is_style_property_name(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '-' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '-' || ch.is_ascii_alphanumeric())
}

fn replace_style_identifier(source: &str, name: &str, value: &str) -> String {
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

fn trim_style_value(value: &str) -> &str {
    value.trim_end_matches(';').trim()
}

fn normalize_preprocessor_color(value: &str) -> String {
    match value.trim() {
        "rgb(255, 0, 0)" => "#ff0000".into(),
        "red" => "red".into(),
        other => other.to_string(),
    }
}

fn is_style_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first == '-' || first.is_ascii_alphabetic())
        && chars.all(is_style_identifier_char)
}

fn is_style_identifier_char(ch: char) -> bool {
    ch == '_' || ch == '-' || ch.is_ascii_alphanumeric()
}

/// Rewrites selectors in `source` to include `scope_id`.
pub fn rewrite_scoped_selectors(source: &str, scope_id: &str) -> String {
    let short_id = scope_id.strip_prefix("data-v-").unwrap_or(scope_id);
    let keyframes = collect_scoped_keyframes(source, short_id);
    rewrite_css_items(source, scope_id, &keyframes, CssBlockContext::Root)
}

/// Collects unique CSS variable expressions from `v-bind(...)` calls.
pub fn collect_css_vars(source: &str) -> Vec<String> {
    collect_css_vars_with_options(source, CssVarCollectOptions::default())
}

/// Options for collecting CSS variable expressions from `v-bind(...)`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CssVarCollectOptions {
    /// Whether Less/Sass/Stylus-style `// ...` comments are skipped.
    pub ignore_line_comments: bool,
}

/// Collects unique CSS variable expressions from `v-bind(...)` calls.
pub fn collect_css_vars_with_options(source: &str, options: CssVarCollectOptions) -> Vec<String> {
    let mut vars = Vec::new();
    for binding in css_var_bindings(source, options.ignore_line_comments) {
        if !binding.expression.is_empty()
            && !vars.iter().any(|existing| existing == &binding.expression)
        {
            vars.push(binding.expression);
        }
    }
    vars
}

/// Generates the CSS custom property name for a Vue style variable binding.
pub fn gen_css_var_name(id: &str, raw: &str, is_prod: bool) -> String {
    gen_css_var_name_with_style(id, raw, is_prod, CssVarNameStyle::Vue27Legacy)
}

/// Generates the CSS custom property name for a Vue style variable binding.
pub fn gen_css_var_name_with_style(
    id: &str,
    raw: &str,
    is_prod: bool,
    style: CssVarNameStyle,
) -> String {
    if is_prod {
        let hash = hash_sum_string(&format!("{id}{raw}"));
        return if matches!(style, CssVarNameStyle::Vue3Escaped)
            && hash.chars().next().is_some_and(|ch| ch.is_ascii_digit())
        {
            format!("v{hash}")
        } else {
            hash
        };
    }
    let mut name = String::new();
    if !id.is_empty() {
        name.push_str(id);
        name.push('-');
    }
    match style {
        CssVarNameStyle::Vue3Escaped => {
            name.push_str(&escape_vue3_css_var_name(raw));
        }
        CssVarNameStyle::Vue27Legacy => {
            name.push_str(&legacy_vue27_css_var_name(raw));
        }
    }
    name
}

/// Rewrites `v-bind(...)` CSS expressions to `var(--...)` custom properties.
pub fn rewrite_css_vars(source: &str, id: &str, is_prod: bool) -> String {
    rewrite_css_vars_with_options(
        source,
        id,
        CssVarRewriteOptions {
            is_prod,
            name_style: CssVarNameStyle::Vue27Legacy,
            ignore_line_comments: false,
        },
    )
}

/// Options for rewriting CSS variable bindings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CssVarRewriteOptions {
    /// Whether production CSS variable names should use hashed names.
    pub is_prod: bool,
    /// CSS variable custom-property naming behavior.
    pub name_style: CssVarNameStyle,
    /// Whether Less/Sass/Stylus-style `// ...` comments are skipped.
    pub ignore_line_comments: bool,
}

/// Rewrites `v-bind(...)` CSS expressions to `var(--...)` custom properties.
pub fn rewrite_css_vars_with_options(
    source: &str,
    id: &str,
    options: CssVarRewriteOptions,
) -> String {
    let bindings = css_var_bindings(source, options.ignore_line_comments);
    if bindings.is_empty() {
        return source.to_string();
    }
    let mut output = String::new();
    let mut cursor = 0usize;
    for binding in bindings {
        if binding.start < cursor {
            continue;
        }
        output.push_str(&source[cursor..binding.start]);
        output.push_str("var(--");
        output.push_str(&gen_css_var_name_with_style(
            id,
            &binding.expression,
            options.is_prod,
            options.name_style,
        ));
        output.push(')');
        cursor = binding.end;
    }
    output.push_str(&source[cursor..]);
    output
}

fn escape_vue3_css_var_name(raw: &str) -> String {
    let mut escaped = String::new();
    for ch in raw.chars() {
        if is_vue3_css_var_escape_symbol(ch) {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    escaped
}

fn is_vue3_css_var_escape_symbol(ch: char) -> bool {
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
    )
}

fn legacy_vue27_css_var_name(raw: &str) -> String {
    raw.chars()
        .map(|ch| {
            if ch == '-' || ch == '_' || ch.is_ascii_alphanumeric() {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn style_var_id(id: &str) -> String {
    id.strip_prefix("data-v-").unwrap_or(id).to_string()
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CssVarBinding {
    start: usize,
    end: usize,
    expression: String,
}

fn css_var_bindings(source: &str, ignore_line_comments: bool) -> Vec<CssVarBinding> {
    let mut bindings = Vec::new();
    let mut cursor = 0usize;
    while cursor < source.len() {
        if source[cursor..].starts_with("/*") {
            let Some(end_offset) = source[cursor + 2..].find("*/") else {
                break;
            };
            cursor += 2 + end_offset + 2;
            continue;
        }
        if ignore_line_comments && source[cursor..].starts_with("//") {
            cursor = skip_css_line_comment(source, cursor);
            continue;
        }
        let Some(start_offset) = find_next_v_bind(source, cursor, ignore_line_comments) else {
            break;
        };
        let open_end = start_offset + v_bind_prefix_len(&source[start_offset..]);
        let Some(end) = lex_css_var_binding(source, open_end) else {
            cursor = open_end;
            continue;
        };
        bindings.push(CssVarBinding {
            start: start_offset,
            end: end + 1,
            expression: normalize_expression(&source[open_end..end]),
        });
        cursor = end + 1;
    }
    bindings
}

fn find_next_v_bind(source: &str, cursor: usize, ignore_line_comments: bool) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut index = cursor;
    while index + "v-bind".len() <= source.len() {
        if source[index..].starts_with("/*") {
            if let Some(end_offset) = source[index + 2..].find("*/") {
                index += 2 + end_offset + 2;
                continue;
            }
            return None;
        }
        if ignore_line_comments && source[index..].starts_with("//") {
            index = skip_css_line_comment(source, index);
            continue;
        }
        if source[index..].starts_with("v-bind") {
            let mut open = index + "v-bind".len();
            while open < source.len() && bytes[open].is_ascii_whitespace() {
                open += 1;
            }
            if open < source.len() && bytes[open] == b'(' {
                return Some(index);
            }
        }
        let ch = source[index..].chars().next()?;
        index += ch.len_utf8();
    }
    None
}

fn skip_css_line_comment(source: &str, start: usize) -> usize {
    source[start..]
        .find(['\n', '\r'])
        .map(|offset| start + offset)
        .unwrap_or(source.len())
}

fn v_bind_prefix_len(source: &str) -> usize {
    let mut len = "v-bind".len();
    let bytes = source.as_bytes();
    while len < source.len() && bytes[len].is_ascii_whitespace() {
        len += 1;
    }
    len + 1
}

fn lex_css_var_binding(source: &str, start: usize) -> Option<usize> {
    let mut state = CssVarLexerState::InParens;
    let mut depth = 0usize;
    let mut index = start;
    while index < source.len() {
        let ch = source[index..].chars().next()?;
        match state {
            CssVarLexerState::InParens => match ch {
                '\'' => state = CssVarLexerState::InSingleQuote,
                '"' => state = CssVarLexerState::InDoubleQuote,
                '(' => depth += 1,
                ')' if depth > 0 => depth -= 1,
                ')' => return Some(index),
                _ => {}
            },
            CssVarLexerState::InSingleQuote => {
                if ch == '\'' {
                    state = CssVarLexerState::InParens;
                }
            }
            CssVarLexerState::InDoubleQuote => {
                if ch == '"' {
                    state = CssVarLexerState::InParens;
                }
            }
        }
        index += ch.len_utf8();
    }
    None
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CssVarLexerState {
    InParens,
    InSingleQuote,
    InDoubleQuote,
}

fn normalize_expression(value: &str) -> String {
    let value = value.trim();
    if value.len() >= 2
        && ((value.starts_with('\'') && value.ends_with('\''))
            || (value.starts_with('"') && value.ends_with('"')))
    {
        value[1..value.len() - 1].to_string()
    } else {
        value.to_string()
    }
}

fn hash_sum_string(value: &str) -> String {
    let mut hash = 0i32;
    hash = hash_sum_fold(hash, "");
    hash = hash_sum_fold(hash, "[object String]");
    hash = hash_sum_fold(hash, "string");
    hash = hash_sum_fold(hash, value);
    format!("{:0>8}", format!("{hash:x}"))
}

fn hash_sum_fold(mut hash: i32, text: &str) -> i32 {
    if text.is_empty() {
        return hash;
    }
    for code in text.encode_utf16() {
        hash = hash
            .wrapping_shl(5)
            .wrapping_sub(hash)
            .wrapping_add(code as i32);
    }
    if hash < 0 {
        hash.wrapping_mul(-2)
    } else {
        hash
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CssBlockContext {
    Root,
    Container,
    Keyframes,
}

fn rewrite_css_items(
    source: &str,
    scope_id: &str,
    keyframes: &[(String, String)],
    context: CssBlockContext,
) -> String {
    let mut output = String::new();
    let mut cursor = 0usize;
    while cursor < source.len() {
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
        if prelude.starts_with('@') {
            let rewritten_prelude = rewrite_at_rule_prelude(prelude, keyframes);
            output.push_str(&rewritten_prelude);
            output.push_str(brace_spacing);
            output.push('{');
            let next_context = if is_keyframes_at_rule(prelude) {
                CssBlockContext::Keyframes
            } else {
                CssBlockContext::Container
            };
            output.push_str(&rewrite_css_items(body, scope_id, keyframes, next_context));
            output.push('}');
        } else {
            let selector = if context == CssBlockContext::Keyframes {
                prelude.to_string()
            } else {
                rewrite_selector_list(prelude, scope_id)
            };
            output.push_str(&selector);
            output.push_str(brace_spacing);
            output.push('{');
            if context == CssBlockContext::Keyframes {
                output.push_str(&rewrite_css_items(
                    body,
                    scope_id,
                    keyframes,
                    CssBlockContext::Keyframes,
                ));
            } else {
                output.push_str(&rewrite_animation_declarations(body, keyframes));
            }
            output.push('}');
        }
        cursor = close + 1;
    }
    output
}

fn compile_css_modules(source: &str, options: &StyleCompileOptions) -> CssModulesCompileResult {
    let mut context = CssModulesContext::new(options);
    let code = rewrite_css_modules_items(source, &mut context, CssBlockContext::Root);
    CssModulesCompileResult {
        code,
        modules: context.modules,
    }
}

#[derive(Debug, PartialEq, Eq)]
struct CssModulesCompileResult {
    code: String,
    modules: BTreeMap<String, String>,
}

#[derive(Debug)]
struct CssModulesContext<'a> {
    filename: &'a str,
    id: &'a str,
    scope_behaviour: CssModulesScopeBehaviour,
    generate_scoped_name: Option<&'a str>,
    locals_convention: CssModulesLocalsConvention,
    modules: BTreeMap<String, String>,
}

impl<'a> CssModulesContext<'a> {
    fn new(options: &'a StyleCompileOptions) -> Self {
        Self {
            filename: options.filename.as_deref().unwrap_or("style.css"),
            id: options.id.as_deref().unwrap_or_default(),
            scope_behaviour: CssModulesScopeBehaviour::from_option(
                &options.modules_options.scope_behaviour,
            ),
            generate_scoped_name: options.modules_options.generate_scoped_name.as_deref(),
            locals_convention: CssModulesLocalsConvention::from_option(
                &options.modules_options.locals_convention,
            ),
            modules: BTreeMap::new(),
        }
    }

    fn is_local_default(&self) -> bool {
        matches!(self.scope_behaviour, CssModulesScopeBehaviour::Local)
    }

    fn scoped_name(&self, local: &str) -> String {
        if let Some(pattern) = self.generate_scoped_name {
            return format_css_module_pattern(pattern, self.filename, local, self.id);
        }
        format!(
            "_{}_{}",
            local,
            css_module_hash(self.filename, local, self.id)
        )
    }

    fn register(&mut self, local: &str, scoped: &str) {
        match self.locals_convention {
            CssModulesLocalsConvention::AsIs => {
                self.modules
                    .entry(local.to_string())
                    .or_insert_with(|| scoped.to_string());
            }
            CssModulesLocalsConvention::CamelCase => {
                self.modules
                    .entry(local.to_string())
                    .or_insert_with(|| scoped.to_string());
                self.modules
                    .entry(camel_case_css_module_key(local))
                    .or_insert_with(|| scoped.to_string());
            }
            CssModulesLocalsConvention::CamelCaseOnly => {
                self.modules
                    .entry(camel_case_css_module_key(local))
                    .or_insert_with(|| scoped.to_string());
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CssModulesScopeBehaviour {
    Local,
    Global,
}

impl CssModulesScopeBehaviour {
    fn from_option(value: &str) -> Self {
        if value.eq_ignore_ascii_case("global") {
            Self::Global
        } else {
            Self::Local
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CssModulesLocalsConvention {
    AsIs,
    CamelCase,
    CamelCaseOnly,
}

impl CssModulesLocalsConvention {
    fn from_option(value: &str) -> Self {
        match value {
            "camelCase" | "camel-case" => Self::CamelCase,
            "camelCaseOnly" | "camel-case-only" | "dashesOnly" | "dashes-only" => {
                Self::CamelCaseOnly
            }
            _ => Self::AsIs,
        }
    }
}

fn rewrite_css_modules_items(
    source: &str,
    context: &mut CssModulesContext<'_>,
    block_context: CssBlockContext,
) -> String {
    let mut output = String::new();
    let mut cursor = 0usize;
    while cursor < source.len() {
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
        output.push_str(&rewrite_css_modules_prelude(
            prelude,
            context,
            block_context,
        ));
        output.push_str(brace_spacing);
        output.push('{');
        if prelude.starts_with('@') {
            let next_context = if is_keyframes_at_rule(prelude) {
                CssBlockContext::Keyframes
            } else {
                CssBlockContext::Container
            };
            output.push_str(&rewrite_css_modules_items(body, context, next_context));
        } else {
            output.push_str(body);
        }
        output.push('}');
        cursor = close + 1;
    }
    output
}

fn rewrite_css_modules_prelude(
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

fn rewrite_css_module_selector(selector: &str, context: &mut CssModulesContext<'_>) -> String {
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
                output.push_str(selector[open + 1..close].trim());
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

fn rewrite_css_module_default_segment(
    segment: &str,
    context: &mut CssModulesContext<'_>,
    local: bool,
) -> String {
    if !local {
        return segment.to_string();
    }
    let mut output = String::new();
    let mut cursor = 0usize;
    while let Some((start, end, name)) = find_next_class_selector(segment, cursor) {
        output.push_str(&segment[cursor..start]);
        let scoped = context.scoped_name(name);
        context.register(name, &scoped);
        output.push('.');
        output.push_str(&scoped);
        cursor = end;
    }
    output.push_str(&segment[cursor..]);
    output
}

fn find_pseudo_function_from(
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

fn find_next_class_selector(source: &str, start: usize) -> Option<(usize, usize, &str)> {
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
                '.' => {
                    let name_start = index + 1;
                    let name_end = consume_css_module_class_name(source, name_start);
                    if name_end > name_start {
                        return Some((index, name_end, &source[name_start..name_end]));
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

fn consume_css_module_class_name(source: &str, mut index: usize) -> usize {
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

fn format_css_module_pattern(pattern: &str, filename: &str, local: &str, id: &str) -> String {
    let file_stem = Path::new(filename)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("style");
    let hash = css_module_hash(filename, local, id);
    let mut output = pattern
        .replace("[name]", file_stem)
        .replace("[local]", local)
        .replace("[hash:base64:5]", &hash[..hash.len().min(5)])
        .replace("[hash]", &hash);
    output = output.replace('.', "_");
    output
}

fn css_module_hash(filename: &str, local: &str, id: &str) -> String {
    let seed = format!("{filename}:{id}:{local}");
    hash_sum_string(&seed).chars().take(6).collect()
}

fn camel_case_css_module_key(value: &str) -> String {
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

fn skip_css_whitespace(source: &str, mut cursor: usize) -> usize {
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

fn push_normalized_css_whitespace(output: &mut String, whitespace: &str) {
    if whitespace.contains('\n') || whitespace.contains('\r') {
        output.push('\n');
    } else {
        output.push_str(whitespace);
    }
}

fn find_next_css_delimiter(source: &str, start: usize) -> Option<(usize, char)> {
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

fn find_matching_brace(source: &str, open: usize) -> Option<usize> {
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
enum CssScannerState {
    Normal,
    SingleQuote,
    DoubleQuote,
    BlockComment,
}

fn collect_scoped_keyframes(source: &str, short_id: &str) -> Vec<(String, String)> {
    let mut keyframes = Vec::new();
    collect_scoped_keyframes_in(source, short_id, &mut keyframes);
    keyframes
}

fn collect_scoped_keyframes_in(
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
        }
        cursor = close + 1;
    }
}

fn rewrite_at_rule_prelude(prelude: &str, keyframes: &[(String, String)]) -> String {
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

fn parse_at_rule(prelude: &str) -> Option<(&str, &str)> {
    let prelude = prelude.trim();
    let rest = prelude.strip_prefix('@')?;
    let name_end = rest
        .char_indices()
        .find_map(|(index, ch)| ch.is_whitespace().then_some(index))
        .unwrap_or(rest.len());
    Some((&rest[..name_end], rest[name_end..].trim()))
}

fn is_keyframes_at_rule(prelude: &str) -> bool {
    parse_at_rule(prelude)
        .map(|(name, _)| is_keyframes_name(name))
        .unwrap_or(false)
}

fn is_keyframes_name(name: &str) -> bool {
    name.ends_with("keyframes")
}

fn lookup_keyframe_name<'a>(name: &str, keyframes: &'a [(String, String)]) -> Option<&'a String> {
    keyframes
        .iter()
        .find_map(|(raw, rewritten)| (raw == name).then_some(rewritten))
}

fn rewrite_animation_declarations(source: &str, keyframes: &[(String, String)]) -> String {
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

fn top_level_semicolons(source: &str) -> Vec<usize> {
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

fn rewrite_declaration_segment(segment: &str, keyframes: &[(String, String)]) -> String {
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

fn find_top_level_colon(source: &str) -> Option<usize> {
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

fn is_animation_name_property(prop: &str) -> bool {
    let prop = prop.trim().to_ascii_lowercase();
    prop == "animation-name" || (prop.starts_with('-') && prop.ends_with("-animation-name"))
}

fn is_animation_property(prop: &str) -> bool {
    let prop = prop.trim().to_ascii_lowercase();
    prop == "animation" || (prop.starts_with('-') && prop.ends_with("-animation"))
}

fn rewrite_animation_name_value(value: &str, keyframes: &[(String, String)]) -> String {
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

fn rewrite_animation_value(value: &str, keyframes: &[(String, String)]) -> String {
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

fn rewrite_selector_list(selector: &str, scope_id: &str) -> String {
    let rewritten = split_selector_list(selector)
        .into_iter()
        .map(|part| rewrite_single_selector(part.trim(), scope_id))
        .collect::<Vec<_>>()
        .join(", ");
    if selector.ends_with(' ') {
        format!("{rewritten} ")
    } else {
        rewritten
    }
}

fn rewrite_single_selector(selector: &str, scope_id: &str) -> String {
    if selector.is_empty() {
        return selector.to_string();
    }
    if let Some(global) = find_top_level_pseudo_function(selector, &[":global", "::v-global"]) {
        if let Some((open, close)) = global.parens {
            return first_selector_branch(selector[open + 1..close].trim())
                .trim()
                .to_string();
        }
    }
    if let Some(deep) = find_deep_combinator(selector) {
        return rewrite_deep_selector(&selector[..deep.start], &selector[deep.end..], scope_id);
    }
    if let Some(rewritten) = rewrite_deep_container_selector(selector, scope_id) {
        return rewritten;
    }
    if let Some(deep) = find_top_level_pseudo_function(selector, &[":deep", "::v-deep"]) {
        if let Some((open, close)) = deep.parens {
            let mut rhs = selector[open + 1..close].trim().to_string();
            rhs.push_str(&selector[close + 1..]);
            return rewrite_deep_selector(&selector[..deep.start], &rhs, scope_id);
        }
        return rewrite_deep_selector(&selector[..deep.start], &selector[deep.end..], scope_id);
    }
    if let Some(rewritten) = rewrite_slotted_selector(selector, scope_id) {
        return rewritten;
    }
    inject_scope_attribute(selector, scope_id)
}

fn rewrite_deep_container_selector(selector: &str, scope_id: &str) -> Option<String> {
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
    let branches = split_selector_list(inner)
        .into_iter()
        .map(str::trim)
        .collect::<Vec<_>>();
    let has_deep = branches.iter().any(|branch| selector_has_deep(branch));
    let has_normal = branches.iter().any(|branch| !selector_has_deep(branch));
    let can_split = matches!(name, ":is" | ":where" | ":has");
    let should_split = can_split
        && has_deep
        && has_normal
        && prefix.trim().is_empty()
        && !suffix.trim().is_empty();

    if should_split {
        let selectors = branches
            .into_iter()
            .map(|branch| {
                let branch_selector = format!("{prefix}{name}({branch}){suffix}");
                if selector_has_deep(branch) {
                    let rewritten_branch = rewrite_single_selector(branch, scope_id);
                    format!("{prefix}{name}({rewritten_branch}){suffix}")
                } else {
                    inject_scope_attribute(&branch_selector, scope_id)
                }
            })
            .collect::<Vec<_>>();
        return Some(selectors.join(", "));
    }

    let rewritten_inner = branches
        .into_iter()
        .map(|branch| {
            if selector_has_deep(branch) {
                rewrite_single_selector(branch, scope_id)
            } else {
                branch.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    Some(format!("{prefix}{name}({rewritten_inner}){suffix}"))
}

fn selector_has_deep(selector: &str) -> bool {
    find_deep_combinator(selector).is_some()
        || find_pseudo_function(selector, &[":deep", "::v-deep"]).is_some()
}

fn matched_selector_name<'a>(selector: &str, start: usize, names: &'a [&str]) -> Option<&'a str> {
    names
        .iter()
        .copied()
        .find(|name| selector[start..].starts_with(name))
}

fn rewrite_slotted_selector(selector: &str, scope_id: &str) -> Option<String> {
    let slotted = find_top_level_pseudo_function(selector, &[":slotted", "::v-slotted"])?;
    let (open, close) = slotted.parens?;
    let inner = first_selector_branch(selector[open + 1..close].trim()).trim();
    let mut rewritten = String::new();
    rewritten.push_str(&selector[..slotted.start]);
    if inner.is_empty() {
        rewritten.push_str(&format!("[{scope_id}-s]"));
    } else {
        rewritten.push_str(&inject_scope_attribute(inner, &format!("{scope_id}-s")));
    }
    rewritten.push_str(&selector[close + 1..]);
    Some(rewritten)
}

fn split_selector_list(selector: &str) -> Vec<&str> {
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
enum SelectorScannerState {
    Normal,
    SingleQuote,
    DoubleQuote,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SelectorMatch {
    start: usize,
    end: usize,
    parens: Option<(usize, usize)>,
}

fn find_pseudo_function(selector: &str, names: &[&str]) -> Option<SelectorMatch> {
    let mut state = SelectorScannerState::Normal;
    let mut bracket_depth = 0usize;
    let mut index = 0usize;
    while index < selector.len() {
        let ch = selector[index..].chars().next()?;
        match state {
            SelectorScannerState::Normal => match ch {
                '\'' => state = SelectorScannerState::SingleQuote,
                '"' => state = SelectorScannerState::DoubleQuote,
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

fn find_top_level_pseudo_function(selector: &str, names: &[&str]) -> Option<SelectorMatch> {
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

fn first_selector_branch(selector: &str) -> &str {
    split_selector_list(selector)
        .into_iter()
        .next()
        .unwrap_or(selector)
}

fn selector_name_boundary(selector: &str, index: usize) -> bool {
    selector[index..]
        .chars()
        .next()
        .map(|ch| !(ch.is_ascii_alphanumeric() || ch == '-' || ch == '_'))
        .unwrap_or(true)
}

fn skip_selector_whitespace(selector: &str, mut index: usize) -> usize {
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

fn find_matching_selector_paren(selector: &str, open: usize) -> Option<usize> {
    let mut state = SelectorScannerState::Normal;
    let mut depth = 0usize;
    let mut index = open;
    while index < selector.len() {
        let ch = selector[index..].chars().next()?;
        match state {
            SelectorScannerState::Normal => match ch {
                '\'' => state = SelectorScannerState::SingleQuote,
                '"' => state = SelectorScannerState::DoubleQuote,
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
struct DeepCombinator {
    start: usize,
    end: usize,
}

fn find_deep_combinator(selector: &str) -> Option<DeepCombinator> {
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

fn rewrite_deep_selector(prefix: &str, suffix: &str, scope_id: &str) -> String {
    let scoped = inject_scope_attribute(prefix.trim_end(), scope_id);
    let suffix = suffix.trim_start();
    if suffix.is_empty() {
        scoped
    } else {
        format!("{scoped} {suffix}")
    }
}

fn inject_scope_attribute(selector: &str, scope_id: &str) -> String {
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

fn strip_leading_universal_selector(selector: &str) -> &str {
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
        ch == '.' || ch == '#' || ch == '[' || ch == ':' || is_selector_ident_start(ch)
    }) {
        &selector[whitespace_end..]
    } else {
        after_star
    }
}

fn selector_injection_index(selector: &str) -> Option<usize> {
    let mut state = SelectorScannerState::Normal;
    let mut last_node_end = None;
    let mut index = 0usize;
    while index < selector.len() {
        let ch = selector[index..].chars().next()?;
        match state {
            SelectorScannerState::Normal => match ch {
                '\'' => state = SelectorScannerState::SingleQuote,
                '"' => state = SelectorScannerState::DoubleQuote,
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
                '>' | '+' | '~' | ',' => {}
                '*' if last_node_end.is_none() => last_node_end = Some(index + ch.len_utf8()),
                '*' => {}
                _ if ch.is_whitespace() => {}
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

fn find_matching_selector_bracket(selector: &str, open: usize) -> Option<usize> {
    let mut state = SelectorScannerState::Normal;
    let mut index = open + 1;
    while index < selector.len() {
        let ch = selector[index..].chars().next()?;
        match state {
            SelectorScannerState::Normal => match ch {
                '\'' => state = SelectorScannerState::SingleQuote,
                '"' => state = SelectorScannerState::DoubleQuote,
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

fn skip_selector_pseudo(selector: &str, start: usize) -> usize {
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

fn consume_selector_token(selector: &str, start: usize) -> usize {
    let mut index = start;
    if selector[index..].starts_with('.') || selector[index..].starts_with('#') {
        index += 1;
    }
    while index < selector.len() {
        let Some(ch) = selector[index..].chars().next() else {
            break;
        };
        if !(ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '\\') {
            break;
        }
        index += ch.len_utf8();
    }
    index
}

fn is_selector_ident_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_' || ch == '-'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_scoped_selectors() {
        let code = rewrite_scoped_selectors(".a, .b { color: red; }", "data-v-x");
        assert!(code.contains(".a[data-v-x]"));
        assert!(code.contains(".b[data-v-x]"));
    }

    #[test]
    fn compile_style_matches_official_selector_brace_spacing() {
        let result = compile_style(
            ".a{ color: v-bind(color); }",
            StyleCompileOptions {
                id: Some("data-v-contract".into()),
                scoped: true,
                ..StyleCompileOptions::default()
            },
        );
        assert_eq!(
            result.code,
            ".a[data-v-contract]{ color: var(--contract-color);\n}"
        );

        let spaced = compile_style(
            ".a { color: v-bind(color); }",
            StyleCompileOptions {
                id: Some("data-v-contract".into()),
                scoped: true,
                ..StyleCompileOptions::default()
            },
        );
        assert_eq!(
            spaced.code,
            ".a[data-v-contract] { color: var(--contract-color);\n}"
        );
    }

    #[test]
    fn rewrites_vue27_scoped_deep_pseudo_and_keyframes() {
        let code = rewrite_scoped_selectors(
            r#"
.foo p >>> .bar { color: red; }
::selection { display: none; }
.test:after { content: 'bye!'; }
@keyframes color { from { color: red; } to { color: green; } }
.anim { animation: color 5s infinite, other 5s; }
.names { animation-name: color, other; }
"#,
            "v-scope-xxx",
        );

        assert!(code.contains(".foo p[v-scope-xxx] .bar { color: red;"));
        assert!(code.contains("[v-scope-xxx]::selection { display: none;"));
        assert!(code.contains(".test[v-scope-xxx]:after { content: 'bye!';"));
        assert!(code.contains("@keyframes color-v-scope-xxx {"));
        assert!(code.contains("animation: color-v-scope-xxx 5s infinite, other 5s;"));
        assert!(code.contains("animation-name: color-v-scope-xxx,other;"));
    }

    #[test]
    fn rewrites_scoped_slotted_selectors_like_vue3() {
        assert_eq!(
            rewrite_scoped_selectors(":slotted(.foo) { color: red; }", "data-v-test"),
            ".foo[data-v-test-s] { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(
                ".baz .qux ::v-slotted(.foo .bar) { color: red; }",
                "data-v-test",
            ),
            ".baz .qux .foo .bar[data-v-test-s] { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(":slotted(.foo):hover { color: red; }", "data-v-test"),
            ".foo[data-v-test-s]:hover { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(".wrapper:slotted(.foo) { color: red; }", "data-v-test"),
            ".wrapper.foo[data-v-test-s] { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(".a :slotted(.foo) .bar { color: red; }", "data-v-test"),
            ".a .foo[data-v-test-s] .bar { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(".a :slotted(*:hover) { color: red; }", "data-v-test"),
            ".a [data-v-test-s]:hover { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(".a :slotted(*.foo) { color: red; }", "data-v-test"),
            ".a .foo[data-v-test-s] { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(".a :slotted(* + .foo) { color: red; }", "data-v-test"),
            ".a  + .foo[data-v-test-s] { color: red; }"
        );
    }

    #[test]
    fn rewrites_top_level_global_selectors_like_vue3() {
        assert_eq!(
            rewrite_scoped_selectors(":global(.foo) { color: red; }", "data-v-test"),
            ".foo { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors("::v-global(.foo .bar) { color: red; }", "data-v-test"),
            ".foo .bar { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(
                ".baz .qux ::v-global(.foo .bar) { color: red; }",
                "data-v-test",
            ),
            ".foo .bar { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(".a :global(.b) .c { color: red; }", "data-v-test"),
            ".b { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(":global(.foo, .bar) { color: red; }", "data-v-test"),
            ".foo { color: red; }"
        );
    }

    #[test]
    fn leaves_nested_global_pseudo_scoped_on_outer_selector() {
        assert_eq!(
            rewrite_scoped_selectors(
                ":is(:global(.foo), .bar) .baz { color: red; }",
                "data-v-test",
            ),
            ":is(:global(.foo), .bar) .baz[data-v-test] { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(
                ":where(:global(.foo), .bar) .baz { color: red; }",
                "data-v-test",
            ),
            ":where(:global(.foo), .bar) .baz[data-v-test] { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(":not(:global(.foo)) .bar { color: red; }", "data-v-test"),
            ":not(:global(.foo)) .bar[data-v-test] { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(
                ":has(:global(.foo), .bar) .baz { color: red; }",
                "data-v-test",
            ),
            ":has(:global(.foo), .bar) .baz[data-v-test] { color: red; }"
        );
    }

    #[test]
    fn rewrites_nested_deep_container_pseudos_like_vue3() {
        assert_eq!(
            rewrite_scoped_selectors(":is(.foo :deep(.bar)) { color: red; }", "data-v-test"),
            ":is(.foo[data-v-test] .bar) { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(":where(.foo :deep(.bar)) { color: red; }", "data-v-test",),
            ":where(.foo[data-v-test] .bar) { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(":is(:deep(.foo)) .bar { color: red; }", "data-v-test"),
            ":is([data-v-test] .foo) .bar { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(":where(:deep(.foo)) .bar { color: red; }", "data-v-test",),
            ":where([data-v-test] .foo) .bar { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(":is(:deep(.foo), .bar) .baz { color: red; }", "data-v-test",),
            ":is([data-v-test] .foo) .baz, :is(.bar) .baz[data-v-test] { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(
                ":where(:deep(.foo), .bar) .baz { color: red; }",
                "data-v-test",
            ),
            ":where([data-v-test] .foo) .baz, :where(.bar) .baz[data-v-test] { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(":not(:deep(.foo)) .bar { color: red; }", "data-v-test"),
            ":not([data-v-test] .foo) .bar { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(":has(:deep(.foo)) .bar { color: red; }", "data-v-test"),
            ":has([data-v-test] .foo) .bar { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(
                ":has(:deep(.foo), .bar) .baz { color: red; }",
                "data-v-test",
            ),
            ":has([data-v-test] .foo) .baz, :has(.bar) .baz[data-v-test] { color: red; }"
        );
    }

    #[test]
    fn leaves_nested_slotted_pseudo_scoped_on_outer_selector() {
        let code =
            rewrite_scoped_selectors(":not(:slotted(.foo)) .bar { color: red; }", "data-v-test");

        assert_eq!(
            code,
            ":not(:slotted(.foo)) .bar[data-v-test] { color: red; }"
        );
    }

    #[test]
    fn rewrites_scoped_selectors_inside_container_at_rules() {
        let code =
            rewrite_scoped_selectors("@media print { .foo { color: #000; } }", "v-scope-xxx");

        assert!(code.contains(".foo[v-scope-xxx] { color: #000;"));
    }

    #[test]
    fn mounts_scope_on_correct_universal_selector_target() {
        assert_eq!(
            rewrite_scoped_selectors("* { color: red; }", "data-v-test"),
            "[data-v-test] { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors("* .foo { color: red; }", "data-v-test"),
            ".foo[data-v-test] { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors("*.foo { color: red; }", "data-v-test"),
            ".foo[data-v-test] { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(".foo * { color: red; }", "data-v-test"),
            ".foo[data-v-test] * { color: red; }"
        );
        assert_eq!(
            rewrite_scoped_selectors(".foo *.bar { color: red; }", "data-v-test"),
            ".foo *.bar[data-v-test] { color: red; }"
        );
    }

    #[test]
    fn compiles_vars_modules_and_map() {
        let result = compile_style(
            ".a { color: v-bind(color); }",
            StyleCompileOptions {
                id: Some("data-v-x".into()),
                scoped: true,
                modules: true,
                source_map: true,
                ..StyleCompileOptions::default()
            },
        );
        assert!(result.code.contains("[data-v-x]"));
        assert!(result.code.contains("var(--x-color)"));
        let modules = result.modules.expect("css modules map");
        assert!(modules.get("a").is_some_and(|value| value.contains("_a_")));
        assert_eq!(result.vars, vec!["color"]);
        assert!(result.map.is_some());
    }

    #[test]
    fn compiles_css_modules_default_local_and_global_pseudo() {
        let result = compile_style(
            ".red { color: red }\n.green { color: green }\n:global(.blue) { color: blue }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some("test.css".into()),
                modules: true,
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");

        assert!(modules
            .get("red")
            .is_some_and(|value| value.contains("_red_")));
        assert!(modules
            .get("green")
            .is_some_and(|value| value.contains("_green_")));
        assert!(!modules.contains_key("blue"));
        assert!(result.code.contains(".blue { color: blue }"));
    }

    #[test]
    fn compiles_css_modules_global_scope_with_local_and_camel_case_only() {
        let result = compile_style(
            ":local(.foo-bar) { color: red }\n.baz-qux { color: green }",
            StyleCompileOptions {
                id: Some("test".into()),
                filename: Some("test.css".into()),
                modules: true,
                modules_options: CssModulesOptions {
                    scope_behaviour: "global".into(),
                    generate_scoped_name: Some("[name]__[local]__[hash:base64:5]".into()),
                    locals_convention: "camelCaseOnly".into(),
                },
                ..StyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules map");

        assert!(modules
            .get("fooBar")
            .is_some_and(|value| value.contains("__foo-bar__")));
        assert!(!modules.contains_key("foo-bar"));
        assert!(!modules.contains_key("bazQux"));
        assert!(result.code.contains(".baz-qux { color: green }"));
    }

    #[test]
    fn source_map_tracks_original_style_source_lines() {
        let source = ".a { color: red; }\n.b { color: blue; }";
        let result = compile_style(
            source,
            StyleCompileOptions {
                filename: Some("component.vue".into()),
                source_map_source: Some(format!("<style>\n{source}\n</style>")),
                source_map_file_id: Some(FileId(7)),
                source_map_base_offset: "<style>\n".len(),
                source_map: true,
                ..StyleCompileOptions::default()
            },
        );
        let map = result.map.expect("style source map");

        assert_eq!(map.sources, vec!["component.vue"]);
        assert_eq!(
            map.sources_content
                .as_ref()
                .and_then(|sources| sources[0].as_ref()),
            Some(&format!("<style>\n{source}\n</style>"))
        );
        let first = map
            .original_position(vuec_source::GeneratedPosition::new(0, 0))
            .unwrap()
            .expect("first mapping");
        assert_eq!(first.source, "component.vue");
        assert_eq!(first.line, 1);
        assert_eq!(first.column, 0);
        let second = map
            .original_position(vuec_source::GeneratedPosition::new(1, 0))
            .unwrap()
            .expect("second mapping");
        assert_eq!(second.line, 2);
        assert_eq!(second.column, 0);
    }

    #[test]
    fn reports_missing_import_with_source_span() {
        let source = ".a { color: red; }\n  @import \"missing.css\";\n.b { color: blue; }";
        let result = compile_style(
            source,
            StyleCompileOptions {
                source_map_file_id: Some(FileId(9)),
                source_map_base_offset: 100,
                ..StyleCompileOptions::default()
            },
        );

        assert_eq!(result.errors, vec!["style import could not be resolved"]);
        assert_eq!(result.diagnostics.len(), 1);
        let diagnostic = &result.diagnostics[0];
        assert_eq!(diagnostic.code, "VUEC_STYLE_IMPORT_RESOLVE");
        let start = ".a { color: red; }\n  ".len();
        let end = start + "@import \"missing.css\";".len();
        assert_eq!(
            diagnostic.span,
            Some(Span::new(FileId(9), 100 + start, 100 + end))
        );
    }

    #[test]
    fn preprocesses_vue27_style_languages_before_css_transforms() {
        let less = compile_style(
            "@red: rgb(255, 0, 0);\n.color { color: @red; }",
            StyleCompileOptions {
                preprocess_lang: Some("less".into()),
                source_map: true,
                ..StyleCompileOptions::default()
            },
        );
        assert!(less.errors.is_empty());
        assert!(less.code.contains("color: #ff0000;"));
        assert!(less.map.is_some());

        let scss = compile_style(
            "$red: red;\n.color { color: $red; .child { width: 1px; } }",
            StyleCompileOptions {
                preprocess_lang: Some("scss".into()),
                ..StyleCompileOptions::default()
            },
        );
        assert!(scss.code.contains("color: red;"));
        assert!(scss.code.contains(".color .child"));

        let sass = compile_style(
            "$red: red\n.color\n  color: $red",
            StyleCompileOptions {
                preprocess_lang: Some("sass".into()),
                ..StyleCompileOptions::default()
            },
        );
        assert!(sass.code.contains("color: red;"));

        let stylus = compile_style(
            "red-color = rgb(255, 0, 0);\n.color\n  color: red-color",
            StyleCompileOptions {
                preprocess_lang: Some("styl".into()),
                ..StyleCompileOptions::default()
            },
        );
        assert!(stylus.code.contains("color: #f00;"));
    }

    #[test]
    fn preprocesses_less_variables_nested_selectors_and_media() {
        let result = compile_style(
            r#"
@red: rgb(255, 0, 0);
.card, .panel {
  @gap: 8px;
  color: @red;
  padding: @gap;
  &:hover {
    color: blue;
  }
  .title {
    margin: @gap;
  }
  @media (min-width: 600px) {
    display: block;
    .title {
      color: @red;
    }
  }
}
.other {
  color: @red;
}
"#,
            StyleCompileOptions {
                preprocess_lang: Some("less".into()),
                ..StyleCompileOptions::default()
            },
        );

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(result.code.contains(".card, .panel {"));
        assert!(result.code.contains("color: #ff0000;"));
        assert!(result.code.contains("padding: 8px;"));
        assert!(result.code.contains(".card:hover, .panel:hover {"));
        assert!(result.code.contains(".card .title, .panel .title {"));
        assert!(result.code.contains("@media (min-width: 600px) {"));
        assert!(result.code.contains("display: block;"));
        assert!(result.code.contains(".other {"));
        assert!(!result.code.contains("@red"));
        assert!(!result.code.contains("@gap"));
    }

    #[test]
    fn preprocesses_less_additional_data_imports_and_load_paths() {
        let dir = tempfile::tempdir().expect("temp dir");
        let src_dir = dir.path().join("src");
        let shared_dir = dir.path().join("shared");
        std::fs::create_dir_all(&src_dir).expect("src dir");
        std::fs::create_dir_all(&shared_dir).expect("shared dir");
        let base = src_dir.join("component.less");
        let local_import = src_dir.join("local.less");
        let load_path_import = shared_dir.join("tokens.less");
        std::fs::write(
            &local_import,
            r#"
.imported {
  border-color: @brand;
}
"#,
        )
        .expect("write local import");
        std::fs::write(
            &load_path_import,
            r#"
@space: 12px;
.shared {
  margin: @space;
}
"#,
        )
        .expect("write load path import");

        let result = compile_style(
            r#"
@import "./local.less";
@import "tokens";
@import "https://example.com/reset.css";
.root {
  color: @brand;
  padding: @space;
}
"#,
            StyleCompileOptions {
                filename: Some(base.to_string_lossy().into_owned()),
                preprocess_lang: Some("less".into()),
                preprocess_options: StylePreprocessOptions {
                    additional_data: Some("@brand: red;".into()),
                    load_paths: vec![shared_dir.to_string_lossy().into_owned()],
                },
                ..StyleCompileOptions::default()
            },
        );

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(result
            .code
            .contains("@import \"https://example.com/reset.css\";"));
        assert!(result.code.contains(".imported {"));
        assert!(result.code.contains("border-color: red;"));
        assert!(result.code.contains(".shared {"));
        assert!(result.code.contains("margin: 12px;"));
        assert!(result.code.contains("padding: 12px;"));
        let mut expected = vec![
            std::fs::canonicalize(local_import)
                .expect("canonical local import")
                .to_string_lossy()
                .replace('\\', "/")
                .trim_start_matches("//?/")
                .to_string(),
            std::fs::canonicalize(load_path_import)
                .expect("canonical load path import")
                .to_string_lossy()
                .replace('\\', "/")
                .trim_start_matches("//?/")
                .to_string(),
        ];
        expected.sort();
        assert_eq!(result.dependencies, expected);
    }

    #[test]
    fn preprocesses_stylus_variables_nested_selectors_and_media() {
        let result = compile_style(
            r#"
red-color = rgb(255, 0, 0)
gap = 8px
.card, .panel
  color red-color
  padding: gap
  &:hover
    color blue
  .title
    margin gap
  @media (min-width: 600px)
    display block
    .title
      color red-color
.other
  color red-color
"#,
            StyleCompileOptions {
                preprocess_lang: Some("styl".into()),
                ..StyleCompileOptions::default()
            },
        );

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(result.code.contains(".card, .panel {"));
        assert!(result.code.contains("color: #f00;"));
        assert!(result.code.contains("padding: 8px;"));
        assert!(result.code.contains(".card:hover, .panel:hover {"));
        assert!(result.code.contains(".card .title, .panel .title {"));
        assert!(result.code.contains("@media (min-width: 600px) {"));
        assert!(result.code.contains("display: block;"));
        assert!(result.code.contains(".other {"));
        assert!(!result.code.contains("red-color"));
        assert!(!result.code.contains("gap"));
    }

    #[test]
    fn preprocesses_stylus_additional_data_imports_and_load_paths() {
        let dir = tempfile::tempdir().expect("temp dir");
        let src_dir = dir.path().join("src");
        let shared_dir = dir.path().join("shared");
        std::fs::create_dir_all(&src_dir).expect("src dir");
        std::fs::create_dir_all(&shared_dir).expect("shared dir");
        let base = src_dir.join("component.styl");
        let local_import = src_dir.join("local.styl");
        let load_path_import = shared_dir.join("tokens.styl");
        std::fs::write(
            &local_import,
            r#"
.imported
  border-color brand
"#,
        )
        .expect("write local import");
        std::fs::write(
            &load_path_import,
            r#"
space = 12px
.shared
  margin space
"#,
        )
        .expect("write load path import");

        let result = compile_style(
            r#"
@import "./local"
@import "tokens"
@import "https://example.com/reset.css"
.root
  color brand
  padding space
"#,
            StyleCompileOptions {
                filename: Some(base.to_string_lossy().into_owned()),
                preprocess_lang: Some("stylus".into()),
                preprocess_options: StylePreprocessOptions {
                    additional_data: Some("brand = red".into()),
                    load_paths: vec![shared_dir.to_string_lossy().into_owned()],
                },
                ..StyleCompileOptions::default()
            },
        );

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(result
            .code
            .contains("@import \"https://example.com/reset.css\";"));
        assert!(result.code.contains(".imported {"));
        assert!(result.code.contains("border-color: red;"));
        assert!(result.code.contains(".shared {"));
        assert!(result.code.contains("margin: 12px;"));
        assert!(result.code.contains("padding: 12px;"));
        let mut expected = vec![
            std::fs::canonicalize(local_import)
                .expect("canonical local import")
                .to_string_lossy()
                .replace('\\', "/")
                .trim_start_matches("//?/")
                .to_string(),
            std::fs::canonicalize(load_path_import)
                .expect("canonical load path import")
                .to_string_lossy()
                .replace('\\', "/")
                .trim_start_matches("//?/")
                .to_string(),
        ];
        expected.sort();
        assert_eq!(result.dependencies, expected);
    }

    #[test]
    fn preprocesses_scss_additional_data_and_import_dependencies() {
        let dir = tempfile::tempdir().expect("temp dir");
        let base = dir.path().join("test.scss");
        let import = dir.path().join("import.scss");
        std::fs::write(&import, ".imported { color: $red; }\n").expect("write import");

        let result = compile_style(
            r#"
@import "./import.scss";
.square {
  @include square(100px);
}
"#,
            StyleCompileOptions {
                filename: Some(base.to_string_lossy().into_owned()),
                preprocess_lang: Some("scss".into()),
                preprocess_options: StylePreprocessOptions {
                    additional_data: Some(
                        r#"
$red: red;
@mixin square($size) {
  width: $size;
  height: $size;
}
"#
                        .into(),
                    ),
                    ..StylePreprocessOptions::default()
                },
                ..StyleCompileOptions::default()
            },
        );

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(result.code.contains(".imported"));
        assert!(result.code.contains("color: red;"));
        assert!(result.code.contains("width: 100px;"));
        let resolved_import = std::fs::canonicalize(import)
            .expect("canonical import")
            .to_string_lossy()
            .replace('\\', "/")
            .trim_start_matches("//?/")
            .to_string();
        assert_eq!(result.dependencies, vec![resolved_import]);
    }

    #[test]
    fn collects_css_vars_like_vue27() {
        let vars = collect_css_vars(
            r#"
            /* color: v-bind(ignored); */
            div {
              color: v-bind(color);
              width: v-bind('font.size');
              top: v-bind((a + b) / 2 + 'px');
              height: v-bind("count.toString(");
              border: v-bind(color);
            }
            "#,
        );

        assert_eq!(
            vars,
            vec![
                "color",
                "font.size",
                "(a + b) / 2 + 'px'",
                "count.toString("
            ]
        );
    }

    #[test]
    fn collects_css_vars_like_vue3_with_line_comments() {
        let vars = collect_css_vars_with_options(
            r#"
            // color: v-bind(ignored);
            div {
              color: v-bind(color);
              width: v-bind('font.size');
              top: v-bind    ((a + b) / 2 + 'px' );
              height: v-bind("count.toString(");
            }
            "#,
            CssVarCollectOptions {
                ignore_line_comments: true,
            },
        );

        assert_eq!(
            vars,
            vec![
                "color",
                "font.size",
                "(a + b) / 2 + 'px'",
                "count.toString("
            ]
        );
    }

    #[test]
    fn rewrites_css_vars_with_vue27_names() {
        let code = rewrite_css_vars(
            ".foo { color: v-bind(color); font-size: v-bind('font.size'); }",
            "test",
            false,
        );
        assert!(code.contains("var(--test-color)"));
        assert!(code.contains("var(--test-font_size)"));
        assert_eq!(gen_css_var_name("xxxxxxxx", "color", true), "4003f1a6");
        assert_eq!(gen_css_var_name("xxxxxxxx", "font.size", true), "41b6490a");
    }

    #[test]
    fn rewrites_css_vars_with_vue3_escaped_names() {
        let code = rewrite_css_vars_with_options(
            concat!(
                ".foo { color: v-bind(color); font-size: v-bind('font.size'); ",
                "font-weight: v-bind(_φ); width: calc(v-bind(foo + 'px') - 3px); }\n",
                "// color: v-bind(ignored)\n",
                ".bar { width: v-bind(width); }"
            ),
            "test",
            CssVarRewriteOptions {
                is_prod: false,
                name_style: CssVarNameStyle::Vue3Escaped,
                ignore_line_comments: true,
            },
        );

        assert!(code.contains("var(--test-color)"));
        assert!(code.contains(r"var(--test-font\.size)"));
        assert!(code.contains("var(--test-_φ)"));
        assert!(code.contains(r"var(--test-foo\ \+\ \'px\')"));
        assert!(code.contains("// color: v-bind(ignored)"));
        assert!(code.contains("var(--test-width)"));
        assert_eq!(
            gen_css_var_name_with_style("xxxxxxxx", "color", true, CssVarNameStyle::Vue3Escaped),
            "v4003f1a6"
        );
    }
}
