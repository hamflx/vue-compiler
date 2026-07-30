use crate::*;

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
    /// Whether scoped CSS deprecated deep syntax should produce warning diagnostics.
    #[serde(default)]
    pub warn_deprecated_scoped_selectors: bool,
}

/// Options for SFC style preprocessing.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StylePreprocessOptions {
    /// Additional source prepended before preprocessing. Function-valued public
    /// options are evaluated by the JavaScript API boundary before reaching Rust.
    #[serde(default, rename = "additionalData", alias = "additional_data")]
    pub additional_data: Option<String>,
    /// Optional load paths used to resolve preprocessor imports.
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
    /// Optional prefix included in template hash generation.
    #[serde(default, rename = "hashPrefix", alias = "hash_prefix")]
    pub hash_prefix: String,
    /// Export key convention such as `asIs`, `camelCase`, or `camelCaseOnly`.
    #[serde(default, rename = "localsConvention", alias = "locals_convention")]
    pub locals_convention: String,
    /// Whether global class selectors are included in the module export map.
    #[serde(default, rename = "exportGlobals", alias = "export_globals")]
    pub export_globals: bool,
    /// File-name patterns whose CSS Modules default scope should be global.
    #[serde(default, rename = "globalModulePaths", alias = "global_module_paths")]
    pub global_module_paths: Vec<String>,
}

impl Default for CssModulesOptions {
    fn default() -> Self {
        Self {
            scope_behaviour: "local".into(),
            generate_scoped_name: None,
            hash_prefix: String::new(),
            locals_convention: "asIs".into(),
            export_globals: false,
            global_module_paths: Vec::new(),
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
