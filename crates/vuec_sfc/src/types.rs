use crate::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Parsed SFC block such as `template`, `script`, `style`, or a custom block.
pub struct SfcBlock {
    /// Normalized block type name.
    pub type_name: String,
    /// Block content after the selected SFC parse mode is applied.
    pub content: String,
    /// Parsed block attributes.
    pub attrs: SfcBlockAttrs,
    /// Source location for the block content.
    pub loc: SfcBlockLocation,
    /// Raw content start byte offset in the full SFC source.
    #[serde(skip)]
    pub content_start: usize,
    /// Raw content end byte offset in the full SFC source.
    #[serde(skip)]
    pub content_end: usize,
    /// Additional original source-map column offset after descriptor-level content transforms.
    #[serde(skip)]
    pub source_map_column_offset: usize,
    /// Whether an empty block must be preserved because parser recovery produced it.
    #[serde(skip)]
    pub preserve_empty: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
/// Parsed SFC block attributes.
pub struct SfcBlockAttrs {
    /// Optional `lang` attribute.
    pub lang: Option<String>,
    /// Optional external `src` attribute.
    pub src: Option<String>,
    /// Whether the block has the `scoped` attribute.
    pub scoped: bool,
    /// Optional CSS modules attribute value.
    pub module: Option<String>,
    /// Whether the script block is `setup`.
    pub setup: bool,
    /// Optional Vue 3 generic attribute value.
    pub generic: Option<String>,
    /// Raw attributes keyed by attribute name.
    pub raw: BTreeMap<String, SfcAttrValue>,
    /// Source ranges for raw attributes keyed by attribute name.
    #[serde(skip)]
    pub ranges: BTreeMap<String, (usize, usize)>,
    /// Source offsets for duplicated attributes after the first occurrence.
    #[serde(skip)]
    pub duplicate_attr_starts: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
/// Raw SFC attribute value.
pub enum SfcAttrValue {
    /// Boolean attribute value.
    Bool(bool),
    /// String attribute value.
    String(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Byte-range location for an SFC block.
pub struct SfcBlockLocation {
    /// Start byte offset.
    pub start: usize,
    /// End byte offset.
    pub end: usize,
    /// Source file identity.
    pub source_file: FileId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Parsed SFC descriptor.
pub struct SfcDescriptor {
    /// Logical filename.
    pub filename: String,
    /// Full SFC source text.
    pub source: String,
    /// Source file identity.
    pub source_file: FileId,
    /// Optional template block.
    pub template: Option<SfcBlock>,
    /// Optional normal script block.
    pub script: Option<SfcBlock>,
    /// Optional script setup block.
    pub script_setup: Option<SfcBlock>,
    /// Style blocks in source order.
    pub styles: Vec<SfcBlock>,
    /// Custom blocks in source order.
    pub custom_blocks: Vec<SfcBlock>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Vue 3 SFC `parse()` result before public JSON projection.
pub struct Vue3SfcParseResult {
    /// Parsed and validated descriptor.
    pub descriptor: SfcDescriptor,
    /// Parse diagnostics emitted by Vue 3 SFC descriptor validation.
    pub errors: Vec<Vue3SfcParseError>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Vue 3 SFC parse diagnostic.
pub struct Vue3SfcParseError {
    /// Error message.
    pub message: String,
    /// Optional full block location that caused the error.
    pub loc: Option<SfcBlockLocation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Vue 3 SFC `parse()` options that affect descriptor block selection/content.
pub struct Vue3SfcParseOptions {
    /// Padding mode for non-template blocks.
    pub pad: Vue3SfcPad,
    /// Whether empty non-template blocks without `src` are ignored.
    pub ignore_empty: bool,
}

impl Default for Vue3SfcParseOptions {
    fn default() -> Self {
        Self {
            pad: Vue3SfcPad::False,
            ignore_empty: true,
        }
    }
}

impl SfcBlockAttrs {
    /// Whether a `src` attribute was present, regardless of whether it has a string value.
    pub fn has_src_attr(&self) -> bool {
        self.raw.contains_key("src")
    }

    /// Whether a `src` attribute has a non-empty string value.
    pub fn has_non_empty_src(&self) -> bool {
        self.src.as_deref().is_some_and(|src| !src.is_empty())
    }

    pub(crate) fn attr_location(
        &self,
        name: &str,
        source_file: FileId,
    ) -> Option<SfcBlockLocation> {
        let (start, end) = *self.ranges.get(name)?;
        Some(SfcBlockLocation {
            start,
            end,
            source_file,
        })
    }

    pub(crate) fn duplicate_attr_errors(&self, source_file: FileId) -> Vec<Vue3SfcParseError> {
        self.duplicate_attr_starts
            .iter()
            .map(|offset| Vue3SfcParseError {
                message: "Duplicate attribute.".into(),
                loc: Some(SfcBlockLocation {
                    start: *offset,
                    end: *offset,
                    source_file,
                }),
            })
            .collect()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
/// Vue 3 SFC block padding mode.
pub enum Vue3SfcPad {
    /// Do not pad block content.
    #[default]
    False,
    /// Pad non-template blocks by preserving generated line numbers.
    Line,
    /// Pad non-template blocks by preserving original text width as spaces.
    Space,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Options for projecting Vue 3 `parse()` descriptor output.
pub struct Vue3SfcParseProjectionOptions {
    /// Whether template/style/script source maps are emitted.
    pub source_map: bool,
    /// Source-map source root.
    pub source_root: String,
    /// Padding mode used when the descriptor was parsed.
    pub pad: Vue3SfcPad,
}

impl Default for Vue3SfcParseProjectionOptions {
    fn default() -> Self {
        Self {
            source_map: true,
            source_root: String::new(),
            pad: Vue3SfcPad::False,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Vue 2.7 `parseComponent` options.
pub struct Vue27ParseComponentOptions {
    /// Padding mode applied to block content.
    pub pad: Vue27SfcPad,
    /// Optional deindent behavior override.
    pub deindent: Option<bool>,
    /// Whether parse errors include source ranges.
    pub output_source_range: bool,
}

impl Default for Vue27ParseComponentOptions {
    fn default() -> Self {
        Self {
            pad: Vue27SfcPad::False,
            deindent: None,
            output_source_range: false,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
/// Vue 2.7 SFC block padding mode.
pub enum Vue27SfcPad {
    /// Do not pad block content.
    #[default]
    False,
    /// Pad with line comments.
    True,
    /// Pad with newlines.
    Line,
    /// Pad with spaces.
    Space,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Vue 2.7 `parseComponent` result.
pub struct Vue27ParseComponentResult {
    /// Parsed SFC descriptor.
    pub descriptor: SfcDescriptor,
    /// Parse errors in Vue 2.7 public shape.
    pub errors: Vec<Vue27SfcParseError>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Vue 2.7 SFC parse error.
pub struct Vue27SfcParseError {
    /// Error message.
    pub msg: String,
    /// Optional start byte offset.
    pub start: Option<usize>,
    /// Optional end byte offset.
    pub end: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Options for compiling an SFC template block.
pub struct SfcTemplateCompileOptions {
    /// Optional SFC scope id base.
    pub id: Option<String>,
    /// Whether SSR template output should be generated.
    pub ssr: bool,
    /// Optional scoped-style scope id.
    pub scope_id: Option<String>,
    /// Whether slotted scope markers should be emitted.
    pub slotted: bool,
    /// Whether production compile behavior is requested.
    pub is_prod: bool,
    /// Whether static template subtrees should be hoisted.
    #[serde(default = "default_template_hoist_static")]
    pub hoist_static: bool,
    /// Whether hoisted static subtrees should be stringified.
    #[serde(default)]
    pub stringify_static: bool,
    /// Whether asset URLs should be transformed.
    pub transform_asset_urls: bool,
    /// Asset URL transform options.
    pub asset_url_options: AssetUrlOptions,
}

impl Default for SfcTemplateCompileOptions {
    fn default() -> Self {
        Self {
            id: None,
            ssr: false,
            scope_id: None,
            slotted: false,
            is_prod: false,
            hoist_static: true,
            stringify_static: false,
            transform_asset_urls: true,
            asset_url_options: AssetUrlOptions::default(),
        }
    }
}

pub(crate) fn default_template_hoist_static() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Options for compiling SFC script blocks.
pub struct SfcScriptCompileOptions {
    /// Optional SFC scope id base.
    pub id: Option<String>,
    /// Whether inline template codegen should be folded into the script.
    pub inline_template: bool,
    /// Whether inline template codegen should target SSR.
    #[serde(default)]
    pub inline_template_ssr: bool,
    /// Whether compileScript should return a source map.
    #[serde(default = "default_script_source_map")]
    pub source_map: bool,
    /// Reactive props destructure mode for Vue 3 `defineProps()` destructures.
    #[serde(default)]
    pub props_destructure: SfcPropsDestructureMode,
    /// Explicit global type files used by Vue 3 `compileScript` type resolution.
    #[serde(default)]
    pub global_type_files: Vec<String>,
    /// Runtime module name used for Vue helper imports generated by `compileScript`.
    pub runtime_module_name: Option<String>,
    /// Optional variable target for default export generation instead of `export default`.
    pub gen_default_as: Option<String>,
    /// Whether setup-only static constants may be hoisted to module scope.
    #[serde(default = "default_script_hoist_static")]
    pub hoist_static: bool,
    /// Legacy ref sugar option.
    pub ref_sugar: bool,
    /// Whether production compile behavior is requested.
    pub is_prod: bool,
    /// Whether the current SFC is compiled as a Vue custom element.
    pub custom_element: bool,
    /// Whether script setup returns should include the internal non-enumerable marker.
    pub emit_script_setup_marker: bool,
    /// Whether deprecated `import ... assert {}` syntax is accepted.
    #[serde(default)]
    pub allow_deprecated_import_assert_syntax: bool,
    /// Internal public AST projection mode. Public package APIs keep the default full mode.
    #[serde(skip)]
    pub script_ast_mode: SfcScriptAstMode,
}

impl Default for SfcScriptCompileOptions {
    fn default() -> Self {
        Self {
            id: None,
            inline_template: false,
            inline_template_ssr: false,
            source_map: true,
            props_destructure: SfcPropsDestructureMode::default(),
            global_type_files: Vec::new(),
            runtime_module_name: None,
            gen_default_as: None,
            hoist_static: true,
            ref_sugar: false,
            is_prod: false,
            custom_element: false,
            emit_script_setup_marker: true,
            allow_deprecated_import_assert_syntax: false,
            script_ast_mode: SfcScriptAstMode::Full,
        }
    }
}

pub(crate) fn default_script_source_map() -> bool {
    true
}

pub(crate) fn default_script_hoist_static() -> bool {
    true
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
/// Vue 3 reactive props destructure option.
pub enum SfcPropsDestructureMode {
    /// Enable reactive `defineProps()` destructure rewriting.
    #[default]
    Enabled,
    /// Keep pre-3.5 behavior by treating destructured props as local setup bindings.
    Disabled,
    /// Report an error whenever `defineProps()` is destructured.
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Options for compiling SFC style blocks.
pub struct SfcStyleCompileOptions {
    /// Optional SFC scope id base.
    pub id: Option<String>,
    /// Whether scoped style rewriting should be applied.
    pub scoped: bool,
    /// CSS vars to inject or rewrite.
    pub vars: Vec<String>,
    /// Whether CSS modules should be enabled for direct style source calls.
    pub modules: bool,
    /// CSS Modules naming and export options.
    pub modules_options: CssModulesOptions,
    /// Whether production compile behavior is requested.
    pub is_prod: bool,
    /// CSS variable naming behavior used by style compilation.
    pub css_var_name_style: CssVarNameStyle,
    /// Whether `// ...` comments are ignored while collecting/replacing CSS vars.
    pub css_var_ignore_line_comments: bool,
    /// Optional preprocessor language override.
    pub preprocess_lang: Option<String>,
    /// Preprocessor option surface forwarded to style compilation.
    #[serde(default)]
    pub preprocess_options: StylePreprocessOptions,
    /// Whether Vue 3 scoped CSS deprecated deep syntax should produce warnings.
    #[serde(default = "default_warn_deprecated_scoped_selectors")]
    pub warn_deprecated_scoped_selectors: bool,
    /// Whether source maps should be generated.
    pub source_map: bool,
}

impl Default for SfcStyleCompileOptions {
    fn default() -> Self {
        Self {
            id: None,
            scoped: false,
            vars: Vec::new(),
            modules: false,
            modules_options: CssModulesOptions::default(),
            is_prod: false,
            css_var_name_style: CssVarNameStyle::Vue3Escaped,
            css_var_ignore_line_comments: true,
            preprocess_lang: None,
            preprocess_options: StylePreprocessOptions::default(),
            warn_deprecated_scoped_selectors: true,
            source_map: false,
        }
    }
}

pub(crate) fn default_warn_deprecated_scoped_selectors() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Result of compiling an SFC template block.
pub struct SfcTemplateCompileResult {
    /// Generated render code.
    pub code: String,
    /// Optional source map artifact.
    pub map: Option<SourceMapArtifact>,
    /// Template compile errors.
    pub errors: Vec<SfcTemplateError>,
    /// Binding names visible to the template.
    pub bindings: Vec<String>,
    /// Deterministic AST summary.
    pub ast_summary: String,
    /// Serialized public AST or AST marker.
    pub ast: String,
    /// Generated import/helper preamble.
    pub preamble: String,
    /// Template source text used for compilation.
    pub source: String,
    /// Template compile tips.
    pub tips: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
/// Vue 2.7 template preprocessing options.
pub struct Vue27TemplatePreprocessOptions {
    /// Optional template language.
    pub lang: Option<String>,
    /// Optional filename.
    pub filename: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Vue 2.7 template preprocessing result.
pub struct Vue27TemplatePreprocessResult {
    /// Preprocessed template source.
    pub source: String,
    /// Preprocess errors.
    pub errors: Vec<String>,
    /// Preprocess tips.
    pub tips: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
/// Vue 3 template preprocessing options.
pub struct Vue3TemplatePreprocessOptions {
    /// Optional template language.
    pub lang: Option<String>,
    /// Optional filename.
    pub filename: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Vue 3 template preprocessing result.
pub struct Vue3TemplatePreprocessResult {
    /// Preprocessed template source.
    pub source: String,
    /// Preprocess errors.
    pub errors: Vec<String>,
    /// Preprocess tips.
    pub tips: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// SFC template compile error.
pub struct SfcTemplateError {
    /// Numeric compiler error code.
    pub code: u32,
    /// Compiler diagnostic message.
    pub message: String,
    /// Source location for the error.
    pub loc: SfcSourceLocation,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// SFC source range.
pub struct SfcSourceLocation {
    /// Start position.
    pub start: SfcPosition,
    /// End position.
    pub end: SfcPosition,
    /// Source slice covered by this range.
    pub source: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// One-based line/column plus byte offset position.
pub struct SfcPosition {
    /// One-based UTF-16 column.
    pub column: usize,
    /// One-based line number.
    pub line: usize,
    /// Zero-based byte offset.
    pub offset: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Result of compiling SFC script blocks.
pub struct SfcScriptBlock {
    /// Public block type.
    #[serde(rename = "type")]
    pub type_name: String,
    /// Generated or normalized script content.
    pub content: String,
    /// Optional script block source location.
    pub loc: Option<SfcBlockLocation>,
    /// Script block attributes.
    pub attrs: SfcBlockAttrs,
    /// Whether this represents script setup output.
    pub setup: bool,
    /// Optional script language.
    pub lang: Option<String>,
    /// Binding metadata keyed by binding name.
    pub bindings: BTreeMap<String, String>,
    /// Destructured props aliases keyed by local binding name.
    #[serde(
        rename = "propsAliases",
        default,
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub props_aliases: BTreeMap<String, String>,
    /// Imported binding metadata keyed by local binding name.
    pub imports: BTreeMap<String, SfcScriptImportBinding>,
    /// Script compile errors.
    pub errors: Vec<String>,
    /// Script compile warnings.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    /// Optional source map artifact.
    pub map: Option<SourceMapArtifact>,
    /// Public normal script AST statement projection.
    #[serde(rename = "scriptAst", default, skip_serializing_if = "Vec::is_empty")]
    pub script_ast: Vec<Value>,
    /// Public script setup AST statement projection.
    #[serde(
        rename = "scriptSetupAst",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub script_setup_ast: Vec<Value>,
    /// External dependencies discovered by script compilation.
    pub deps: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
/// Public Vue 3 SFC `resolveType` projection used by Rust-backed conformance helpers.
pub struct Vue3ResolveTypeResult {
    /// Runtime constructor names inferred for each resolved prop.
    pub props: BTreeMap<String, Vec<String>>,
    /// Resolved call signatures represented as public placeholders.
    pub calls: Vec<Value>,
    /// External dependencies consumed by the resolved type argument.
    pub deps: Vec<String>,
    /// Raw type members projected in a shape compatible with Vue's internal helper.
    pub raw: Vue3ResolveTypeRaw,
    /// Type-resolution diagnostics.
    pub errors: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
/// Raw Vue 3 SFC `resolveType` member projection.
pub struct Vue3ResolveTypeRaw {
    /// Raw prop members keyed by public prop name.
    pub props: BTreeMap<String, Vue3ResolveTypeRawProp>,
    /// Raw call signature placeholders.
    pub calls: Vec<Value>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Raw Vue 3 SFC `resolveType` prop member projection.
pub struct Vue3ResolveTypeRawProp {
    /// Runtime constructor names inferred for this member.
    pub types: Vec<String>,
    /// Whether the type member is required.
    pub required: bool,
    /// Whether the type member is optional.
    pub optional: bool,
    /// Whether the type member came from method syntax.
    #[serde(rename = "isMethod")]
    pub is_method: bool,
    /// Type annotation source when available.
    #[serde(
        rename = "typeAnnotationSource",
        skip_serializing_if = "Option::is_none"
    )]
    pub type_annotation_source: Option<String>,
    /// Full member source when available.
    #[serde(rename = "memberSource", skip_serializing_if = "Option::is_none")]
    pub member_source: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Public `compileScript().imports` metadata for one imported binding.
pub struct SfcScriptImportBinding {
    /// Whether this is a type-only import.
    #[serde(rename = "isType")]
    pub is_type: bool,
    /// Imported export name: named export, `default`, or `*`.
    pub imported: String,
    /// Local binding name.
    pub local: String,
    /// Import source string.
    pub source: String,
    /// Whether the binding came from `<script setup>`.
    #[serde(rename = "isFromSetup")]
    pub is_from_setup: bool,
    /// Whether the binding is used by the template.
    #[serde(rename = "isUsedInTemplate")]
    pub is_used_in_template: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Result of compiling SFC style blocks.
pub struct SfcStyleCompileResult {
    /// Generated CSS.
    pub code: String,
    /// Optional source map artifact.
    pub map: Option<SourceMapArtifact>,
    /// Style compile errors.
    pub errors: Vec<String>,
    /// Structured style diagnostics with spans in the original SFC source.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<Diagnostic>,
    /// External style dependencies.
    pub dependencies: Vec<String>,
    /// CSS module exports keyed by local class names.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modules: Option<BTreeMap<String, String>>,
    /// Raw PostCSS result marker data.
    #[serde(rename = "rawResult")]
    pub raw_result: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
/// Vue 2.7 `rewriteDefault` options.
pub struct Vue27RewriteDefaultOptions {
    /// Whether input should be parsed as TypeScript.
    pub typescript: bool,
    /// Whether decorator syntax is enabled.
    #[serde(default)]
    pub decorators: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
/// Vue 3 `rewriteDefault` options.
pub struct Vue3RewriteDefaultOptions {
    /// Whether input should be parsed as TypeScript.
    pub typescript: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
/// Vue 2.7 template identifier prefixing options.
pub struct Vue27PrefixIdentifiersOptions {
    /// Whether the template belongs to a functional component.
    pub is_functional: bool,
    /// Whether expressions should be parsed as TypeScript.
    pub is_ts: bool,
    /// Binding metadata keyed by binding name.
    pub bindings: BTreeMap<String, String>,
}

/// Stateful SFC compiler facade.
pub struct SfcCompiler {
    pub(crate) sources: SourceMap,
    pub(crate) js: JsAstStore,
    pub(crate) descriptor_cache: BTreeMap<SfcCacheKey, SfcDescriptorCacheEntry>,
    pub(crate) cache_stats: SfcCacheStats,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
/// SFC descriptor cache statistics.
pub struct SfcCacheStats {
    /// Number of descriptor cache hits.
    pub descriptor_hits: u64,
    /// Number of descriptor cache misses.
    pub descriptor_misses: u64,
    /// Number of stale descriptor cache entries invalidated.
    pub descriptor_invalidations: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct SfcCacheKey {
    pub(crate) filename: String,
    pub(crate) source_hash: u64,
    pub(crate) mode: SfcParseCacheMode,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SfcDescriptorCacheEntry {
    pub(crate) descriptor: SfcDescriptor,
    pub(crate) vue3_errors: Vec<Vue3SfcParseError>,
    pub(crate) vue27_errors: Vec<Vue27SfcParseError>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum SfcParseCacheMode {
    Vue3 {
        pad: Vue3SfcPad,
        ignore_empty: bool,
    },
    Vue27 {
        pad: Vue27SfcPad,
        deindent: Option<bool>,
        output_source_range: bool,
    },
}

impl SfcCacheKey {
    pub(crate) fn new(filename: String, source: &str, mode: SfcParseCacheMode) -> Self {
        Self {
            filename,
            source_hash: source_hash(source),
            mode,
        }
    }
}

pub(crate) fn source_hash(source: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    hasher.finish()
}
