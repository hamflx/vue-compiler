//! Vue single-file component compiler implementation.
//!
//! This crate owns SFC descriptor parsing, Vue 2.7 `parseComponent`
//! projection, Vue 3 template/script/style compile entry points, Vue 2.7
//! SFC helper APIs, descriptor caching, and source-map/error shapes shared by
//! the CLI, NAPI, WASM, and package-alias layers.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

use oxc_ast::ast::{
    Argument, ArrayExpressionElement, ArrowFunctionExpression, AssignmentTarget, BindingPattern,
    ClassElement, Declaration, ExportDefaultDeclaration, ExportDefaultDeclarationKind,
    ExportNamedDeclaration, ExportSpecifier, Expression, ForStatementInit, ForStatementLeft,
    FormalParameter, Function, ImportDeclarationSpecifier, ImportOrExportKind, ModuleExportName,
    ObjectExpression, ObjectProperty, ObjectPropertyKind, PropertyKey, SimpleAssignmentTarget,
    Statement, TSEnumDeclaration, TSFunctionType, TSInterfaceBody, TSLiteral, TSSignature, TSType,
    TSTypeLiteral, TSTypeName, VariableDeclaration, VariableDeclarationKind, WithStatement,
};
use oxc_span::GetSpan;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};
use vuec_codegen::{SourceMapArtifact, SourceMapBuilder};
use vuec_diagnostics::{Diagnostic, Severity};
use vuec_html::{
    decode_html_attr_entities, HtmlAttribute, HtmlQuoteKind, HtmlTokenKind, HtmlTokenizer,
};
use vuec_js::{JsAstStore, JsParseMode};
use vuec_source::{FileId, SourceMap, Span};
pub use vuec_style::CssVarNameStyle as SfcCssVarNameStyle;
use vuec_style::{
    collect_css_vars_with_options, compile_style, gen_css_var_name_with_style, CssModulesOptions,
    CssVarCollectOptions, CssVarNameStyle, StyleCompileOptions, StylePreprocessOptions,
};
use vuec_vue3_core::{TemplateSource, Vue3CompilerOptions};
use vuec_vue3_dom::{
    apply_dom_parser_defaults, compile as compile_dom, AssetUrlOptions, DomCompilerOptions,
};
use vuec_vue3_ssr::{compile as compile_ssr, SsrCompilerOptions};

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

    fn attr_location(&self, name: &str, source_file: FileId) -> Option<SfcBlockLocation> {
        let (start, end) = *self.ranges.get(name)?;
        Some(SfcBlockLocation {
            start,
            end,
            source_file,
        })
    }

    fn duplicate_attr_errors(&self, source_file: FileId) -> Vec<Vue3SfcParseError> {
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
            transform_asset_urls: true,
            asset_url_options: AssetUrlOptions::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Options for compiling SFC script blocks.
pub struct SfcScriptCompileOptions {
    /// Optional SFC scope id base.
    pub id: Option<String>,
    /// Whether inline template codegen should be folded into the script.
    pub inline_template: bool,
    /// Runtime module name used for Vue helper imports generated by `compileScript`.
    pub runtime_module_name: Option<String>,
    /// Legacy ref sugar option.
    pub ref_sugar: bool,
    /// Whether production compile behavior is requested.
    pub is_prod: bool,
    /// Whether Vue 2.7 script setup returns should include the internal `__sfc` marker.
    pub emit_script_setup_marker: bool,
}

impl Default for SfcScriptCompileOptions {
    fn default() -> Self {
        Self {
            id: None,
            inline_template: false,
            runtime_module_name: None,
            ref_sugar: false,
            is_prod: false,
            emit_script_setup_marker: true,
        }
    }
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

fn default_warn_deprecated_scoped_selectors() -> bool {
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// SFC template compile error.
pub struct SfcTemplateError {
    /// Numeric compiler error code.
    pub code: u32,
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
    /// Imported binding names.
    pub imports: Vec<String>,
    /// Script compile errors.
    pub errors: Vec<String>,
    /// Optional source map artifact.
    pub map: Option<SourceMapArtifact>,
    /// Registered normal script AST ids.
    #[serde(rename = "scriptAst")]
    pub script_ast: Vec<String>,
    /// Registered script setup AST ids.
    #[serde(rename = "scriptSetupAst")]
    pub script_setup_ast: Vec<String>,
    /// External dependencies discovered by script compilation.
    pub deps: Vec<String>,
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
    sources: SourceMap,
    js: JsAstStore,
    descriptor_cache: BTreeMap<SfcCacheKey, SfcDescriptorCacheEntry>,
    cache_stats: SfcCacheStats,
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
struct SfcCacheKey {
    filename: String,
    source_hash: u64,
    mode: SfcParseCacheMode,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SfcDescriptorCacheEntry {
    descriptor: SfcDescriptor,
    vue3_errors: Vec<Vue3SfcParseError>,
    vue27_errors: Vec<Vue27SfcParseError>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum SfcParseCacheMode {
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
    fn new(filename: String, source: &str, mode: SfcParseCacheMode) -> Self {
        Self {
            filename,
            source_hash: source_hash(source),
            mode,
        }
    }
}

fn source_hash(source: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    hasher.finish()
}

impl SfcCompiler {
    /// Creates a new SFC compiler facade.
    pub fn new() -> Self {
        Self {
            sources: SourceMap::default(),
            js: JsAstStore::new(),
            descriptor_cache: BTreeMap::new(),
            cache_stats: SfcCacheStats::default(),
        }
    }

    /// Parses an SFC descriptor using Vue 3-style descriptor rules.
    pub fn parse(&mut self, filename: impl Into<String>, source: &str) -> SfcDescriptor {
        self.parse_vue3(filename, source).descriptor
    }

    /// Parses an SFC descriptor and returns Vue 3 public `parse()` diagnostics.
    pub fn parse_vue3(&mut self, filename: impl Into<String>, source: &str) -> Vue3SfcParseResult {
        self.parse_vue3_with_options(filename, source, Vue3SfcParseOptions::default())
    }

    /// Parses an SFC descriptor and returns Vue 3 public diagnostics with parse options.
    pub fn parse_vue3_with_options(
        &mut self,
        filename: impl Into<String>,
        source: &str,
        options: Vue3SfcParseOptions,
    ) -> Vue3SfcParseResult {
        let filename = filename.into();
        let mode = SfcParseCacheMode::Vue3 {
            pad: options.pad.clone(),
            ignore_empty: options.ignore_empty,
        };
        let key = SfcCacheKey::new(filename.clone(), source, mode);
        if let Some(entry) = self.descriptor_cache.get(&key) {
            self.cache_stats.descriptor_hits += 1;
            return Vue3SfcParseResult {
                descriptor: entry.descriptor.clone(),
                errors: entry.vue3_errors.clone(),
            };
        }
        self.invalidate_stale_descriptor_entries(&filename, &key.mode);
        self.cache_stats.descriptor_misses += 1;
        let source_file = self.sources.add_file(
            Some(std::path::PathBuf::from(&filename)),
            source.to_string(),
        );
        let extracted = extract_sfc_blocks(
            source,
            source_file,
            SfcBlockContentMode::Vue3 { options: &options },
        );
        let mut result =
            vue3_descriptor_from_blocks(filename, source, source_file, extracted.blocks, &options);
        if !extracted.vue3_errors.is_empty() {
            let mut errors = extracted.vue3_errors;
            errors.extend(result.errors);
            result.errors = errors;
        }
        let cached_errors = result.errors.clone();
        self.descriptor_cache.insert(
            key,
            SfcDescriptorCacheEntry {
                descriptor: result.descriptor.clone(),
                vue3_errors: cached_errors,
                vue27_errors: Vec::new(),
            },
        );
        result
    }

    /// Parses an anonymous Vue 2.7 SFC component.
    pub fn parse_vue27_component(
        &mut self,
        source: &str,
        options: Vue27ParseComponentOptions,
    ) -> Vue27ParseComponentResult {
        self.parse_vue27_component_with_filename("anonymous.vue", source, options)
    }

    /// Parses a named Vue 2.7 SFC component.
    pub fn parse_vue27_component_with_filename(
        &mut self,
        filename: impl Into<String>,
        source: &str,
        options: Vue27ParseComponentOptions,
    ) -> Vue27ParseComponentResult {
        let filename = filename.into();
        let mode = SfcParseCacheMode::Vue27 {
            pad: options.pad.clone(),
            deindent: options.deindent,
            output_source_range: options.output_source_range,
        };
        let key = SfcCacheKey::new(filename.clone(), source, mode);
        if let Some(entry) = self.descriptor_cache.get(&key) {
            self.cache_stats.descriptor_hits += 1;
            return Vue27ParseComponentResult {
                descriptor: entry.descriptor.clone(),
                errors: project_vue27_errors(
                    entry.vue27_errors.clone(),
                    options.output_source_range,
                ),
            };
        }
        self.invalidate_stale_descriptor_entries(&filename, &key.mode);
        self.cache_stats.descriptor_misses += 1;
        let source_file = self.sources.add_file(
            Some(std::path::PathBuf::from(&filename)),
            source.to_string(),
        );
        let extracted = extract_sfc_blocks(
            source,
            source_file,
            SfcBlockContentMode::Vue27 { options: &options },
        );
        let descriptor = descriptor_from_blocks(filename, source, source_file, extracted.blocks);
        let cached_errors = extracted.errors.clone();
        self.descriptor_cache.insert(
            key,
            SfcDescriptorCacheEntry {
                descriptor: descriptor.clone(),
                vue3_errors: Vec::new(),
                vue27_errors: cached_errors,
            },
        );

        Vue27ParseComponentResult {
            descriptor,
            errors: project_vue27_errors(extracted.errors, options.output_source_range),
        }
    }

    /// Compiles the descriptor's template block.
    pub fn compile_template(
        &self,
        descriptor: &SfcDescriptor,
        options: SfcTemplateCompileOptions,
    ) -> SfcTemplateCompileResult {
        let Some(template) = descriptor.template.as_ref() else {
            return SfcTemplateCompileResult {
                code: String::new(),
                map: None,
                errors: vec![SfcTemplateError {
                    code: 0,
                    loc: SfcSourceLocation {
                        start: SfcPosition {
                            column: 1,
                            line: 1,
                            offset: 0,
                        },
                        end: SfcPosition {
                            column: 1,
                            line: 1,
                            offset: 0,
                        },
                        source: String::new(),
                    },
                }],
                bindings: Vec::new(),
                ast_summary: "missing-template".into(),
                ast: String::new(),
                preamble: String::new(),
                source: String::new(),
                tips: Vec::new(),
            };
        };
        let mut core = Vue3CompilerOptions {
            prefix_identifiers: true,
            mode: "module".into(),
            hoist_static: true,
            cache_handlers: true,
            scope_id: options.scope_id.clone(),
            slotted: options.slotted,
            source_map: true,
            ..Vue3CompilerOptions::default()
        };
        apply_dom_parser_defaults(&mut core);
        let source = TemplateSource {
            filename: descriptor.filename.clone(),
            source: template.content.clone(),
            file_id: descriptor.source_file,
            base_offset: template.loc.start,
        };
        if options.ssr {
            let result = compile_ssr(
                source,
                SsrCompilerOptions {
                    core,
                    scope_id: options.scope_id.clone(),
                    slotted: options.slotted,
                    slotted_is_explicit: true,
                    mode_is_explicit: true,
                    transform_asset_urls: options.transform_asset_urls,
                    asset_url_options: options.asset_url_options.clone(),
                },
            );
            let ast_summary = result.ast_summary;
            return SfcTemplateCompileResult {
                code: result.code,
                map: result.map,
                errors: sfc_template_errors_from_diagnostics(
                    &result.diagnostics,
                    &template.content,
                ),
                bindings: Vec::new(),
                ast_summary: ast_summary.clone(),
                ast: format!("ast:{ast_summary}"),
                preamble: result.preamble,
                source: template.content.clone(),
                tips: Vec::new(),
            };
        } else {
            let result = compile_dom(
                source,
                DomCompilerOptions {
                    core,
                    transform_asset_urls: options.transform_asset_urls,
                    asset_url_options: options.asset_url_options.clone(),
                    ..DomCompilerOptions::default()
                },
            );
            let ast_summary = result.ast_summary;
            return SfcTemplateCompileResult {
                code: result.code,
                map: result.map,
                errors: sfc_template_errors_from_diagnostics(
                    &result.diagnostics,
                    &template.content,
                ),
                bindings: Vec::new(),
                ast_summary: ast_summary.clone(),
                ast: format!("ast:{ast_summary}"),
                preamble: result.preamble,
                source: template.content.clone(),
                tips: Vec::new(),
            };
        }
    }

    /// Compiles standalone template source through the SFC template path.
    pub fn compile_template_source(
        &self,
        filename: impl Into<String>,
        source: &str,
        options: SfcTemplateCompileOptions,
    ) -> SfcTemplateCompileResult {
        let filename = filename.into();
        let raw_source = source.to_string();
        let side_effect_errors = side_effect_tag_errors(source);
        let mut core = Vue3CompilerOptions {
            prefix_identifiers: true,
            mode: "module".into(),
            hoist_static: true,
            cache_handlers: true,
            scope_id: options.scope_id.clone(),
            slotted: options.slotted,
            source_map: true,
            ..Vue3CompilerOptions::default()
        };
        apply_dom_parser_defaults(&mut core);
        let template_source = TemplateSource {
            filename: filename.clone(),
            source: raw_source.clone(),
            file_id: FileId(0),
            base_offset: 0,
        };
        if options.ssr {
            let result = compile_ssr(
                template_source,
                SsrCompilerOptions {
                    core,
                    scope_id: options.scope_id.clone(),
                    slotted: options.slotted,
                    slotted_is_explicit: true,
                    mode_is_explicit: true,
                    transform_asset_urls: options.transform_asset_urls,
                    asset_url_options: options.asset_url_options.clone(),
                },
            );
            return SfcTemplateCompileResult {
                code: result.code,
                map: result.map,
                errors: merge_template_errors(
                    side_effect_errors,
                    sfc_template_errors_from_diagnostics(&result.diagnostics, &raw_source),
                ),
                bindings: Vec::new(),
                ast_summary: result.ast_summary.clone(),
                ast: json!({
                    "type": 0,
                    "source": raw_source,
                    "transformed": true,
                })
                .to_string(),
                preamble: result.preamble,
                source: raw_source,
                tips: Vec::new(),
            };
        }
        let result = compile_dom(
            template_source,
            DomCompilerOptions {
                core,
                transform_asset_urls: options.transform_asset_urls,
                asset_url_options: options.asset_url_options,
                ..DomCompilerOptions::default()
            },
        );
        SfcTemplateCompileResult {
            code: result.code,
            map: result.map,
            errors: merge_template_errors(
                side_effect_errors,
                sfc_template_errors_from_diagnostics(&result.diagnostics, &raw_source),
            ),
            bindings: Vec::new(),
            ast_summary: result.ast_summary.clone(),
            ast: json!({
                "type": 0,
                "source": raw_source.clone(),
                "transformed": true,
            })
            .to_string(),
            preamble: result.preamble,
            source: raw_source,
            tips: Vec::new(),
        }
    }

    /// Compiles Vue 3 SFC script blocks.
    pub fn compile_script(
        &mut self,
        descriptor: &SfcDescriptor,
        options: SfcScriptCompileOptions,
    ) -> SfcScriptBlock {
        let mut raw_content = String::new();
        let mut script_ast = Vec::new();
        let mut script_setup_ast = Vec::new();
        let source_type = script_source_type(descriptor);
        if let Some(script) = descriptor.script.as_ref() {
            raw_content.push_str(&script.content);
            let id = self.js.register_program(
                script.content.clone(),
                Span::new(descriptor.source_file, script.loc.start, script.loc.end),
                script_mode(&script.attrs),
                source_type,
            );
            script_ast.push(format!("JsProgramId({})", id.0));
        }
        if let Some(script_setup) = descriptor.script_setup.as_ref() {
            if !raw_content.is_empty() {
                raw_content.push('\n');
            }
            raw_content.push_str(&script_setup.content);
            let id = self.js.register_program(
                script_setup.content.clone(),
                Span::new(
                    descriptor.source_file,
                    script_setup.loc.start,
                    script_setup.loc.end,
                ),
                script_mode(&script_setup.attrs),
                source_type,
            );
            script_setup_ast.push(format!("JsProgramId({})", id.0));
        }
        let summary = self.js.summarize_program(&raw_content, source_type);
        let imports = summary.imports;
        let attrs = descriptor
            .script
            .as_ref()
            .or(descriptor.script_setup.as_ref())
            .map(|block| block.attrs.clone())
            .unwrap_or_default();
        let generated_content = script_content(
            descriptor,
            &raw_content,
            descriptor.filename.as_str(),
            &options,
            &script_bindings(&summary.bindings),
        );
        let mut bindings = script_bindings(&summary.bindings);
        bindings.extend(generated_content.bindings.clone());
        for removed in &generated_content.removed_bindings {
            bindings.remove(removed);
        }
        let mut errors = summary.errors;
        errors.extend(generated_content.errors);
        SfcScriptBlock {
            type_name: "script".into(),
            content: generated_content.content,
            loc: descriptor
                .script
                .as_ref()
                .or(descriptor.script_setup.as_ref())
                .map(|block| block.loc.clone()),
            attrs,
            setup: descriptor.script_setup.is_some(),
            lang: descriptor
                .script_setup
                .as_ref()
                .or(descriptor.script.as_ref())
                .and_then(|block| block.attrs.lang.clone()),
            bindings,
            imports,
            errors,
            map: None,
            script_ast,
            script_setup_ast,
            deps: Vec::new(),
        }
    }

    /// Compiles Vue 2.7 SFC script blocks.
    pub fn compile_vue27_script(
        &mut self,
        descriptor: &SfcDescriptor,
        options: SfcScriptCompileOptions,
    ) -> SfcScriptBlock {
        let mut raw_content = String::new();
        let mut script_ast = Vec::new();
        let mut script_setup_ast = Vec::new();
        let source_type = script_source_type(descriptor);
        if let Some(script) = descriptor.script.as_ref() {
            raw_content.push_str(&script.content);
            let id = self.js.register_program(
                script.content.clone(),
                Span::new(descriptor.source_file, script.loc.start, script.loc.end),
                script_mode(&script.attrs),
                source_type,
            );
            script_ast.push(format!("JsProgramId({})", id.0));
        }
        if let Some(script_setup) = descriptor.script_setup.as_ref() {
            if !raw_content.is_empty() {
                raw_content.push('\n');
            }
            raw_content.push_str(&script_setup.content);
            let id = self.js.register_program(
                script_setup.content.clone(),
                Span::new(
                    descriptor.source_file,
                    script_setup.loc.start,
                    script_setup.loc.end,
                ),
                script_mode(&script_setup.attrs),
                source_type,
            );
            script_setup_ast.push(format!("JsProgramId({})", id.0));
        }
        let summary = self.js.summarize_program(&raw_content, source_type);
        let css_vars = descriptor_css_vars(
            descriptor,
            CssVarCollectOptions {
                ignore_line_comments: false,
            },
        );
        let script_errors = vue27_script_compile_errors(descriptor);
        let content = vue27_script_content(descriptor, &options, &css_vars);
        let bindings = if descriptor.script_setup.is_some() {
            vue27_setup_binding_metadata(descriptor)
        } else {
            vue27_normal_script_binding_metadata(descriptor)
        };
        let attrs = descriptor
            .script
            .as_ref()
            .or(descriptor.script_setup.as_ref())
            .map(|block| block.attrs.clone())
            .unwrap_or_default();

        SfcScriptBlock {
            type_name: "script".into(),
            content,
            loc: descriptor
                .script
                .as_ref()
                .or(descriptor.script_setup.as_ref())
                .map(|block| block.loc.clone()),
            attrs,
            setup: descriptor.script_setup.is_some(),
            lang: descriptor
                .script_setup
                .as_ref()
                .or(descriptor.script.as_ref())
                .and_then(|block| block.attrs.lang.clone()),
            bindings,
            imports: summary.imports,
            errors: if script_errors.is_empty() {
                summary.errors
            } else {
                script_errors
            },
            map: None,
            script_ast,
            script_setup_ast,
            deps: Vec::new(),
        }
    }

    /// Compiles all style blocks in a descriptor.
    pub fn compile_style(
        &self,
        descriptor: &SfcDescriptor,
        options: SfcStyleCompileOptions,
    ) -> SfcStyleCompileResult {
        let mut code = String::new();
        let mut errors = Vec::new();
        let mut diagnostics = Vec::new();
        let mut dependencies = Vec::new();
        let mut modules = BTreeMap::new();
        let mut has_modules_result = false;
        let mut raw_result = Vec::new();
        let mut map_builder = options.source_map.then(|| {
            let mut builder = SourceMapBuilder::new().file(descriptor.filename.clone());
            builder.add_source_content(descriptor.filename.clone(), descriptor.source.clone());
            builder
        });
        let mut generated_line_offset = 0u32;
        for style in &descriptor.styles {
            let result = compile_style(
                &style.content,
                StyleCompileOptions {
                    id: options.id.clone(),
                    scoped: options.scoped || style.attrs.scoped,
                    modules: options.modules || style.attrs.module.is_some(),
                    modules_options: options.modules_options.clone(),
                    vars: options.vars.clone(),
                    is_prod: options.is_prod,
                    css_var_name_style: options.css_var_name_style,
                    css_var_ignore_line_comments: options.css_var_ignore_line_comments,
                    filename: Some(descriptor.filename.clone()),
                    source_map_source: Some(descriptor.source.clone()),
                    source_map_file_id: Some(descriptor.source_file),
                    source_map_base_offset: style.content_start,
                    source_map: options.source_map,
                    preprocess_lang: style
                        .attrs
                        .lang
                        .clone()
                        .or_else(|| options.preprocess_lang.clone()),
                    preprocess_options: options.preprocess_options.clone(),
                    warn_deprecated_scoped_selectors: options.warn_deprecated_scoped_selectors,
                },
            );
            let needs_join_newline = !code.is_empty() && !result.code.is_empty();
            if needs_join_newline {
                code.push('\n');
            }
            code.push_str(&result.code);
            errors.extend(result.errors);
            diagnostics.extend(result.diagnostics);
            if let Some(result_modules) = result.modules {
                has_modules_result = true;
                modules.extend(result_modules);
            }
            if let Some(builder) = map_builder.as_mut() {
                add_style_block_mappings(
                    builder,
                    descriptor,
                    style,
                    &result.code,
                    generated_line_offset,
                );
            }
            if !result.code.is_empty() {
                generated_line_offset += generated_line_count(&result.code);
            }
            if needs_join_newline {
                generated_line_offset += 1;
            }
            dependencies.extend(style_src_dependency(style));
            if result.dependencies.is_empty() {
                dependencies.extend(style_import_dependencies(style));
            }
            dependencies.extend(result.dependencies);
            raw_result.push("postcss-result".to_string());
        }
        dependencies.sort();
        dependencies.dedup();
        let map = map_builder.map(SourceMapBuilder::build);
        let modules = has_modules_result.then_some(modules);
        SfcStyleCompileResult {
            code,
            map,
            errors,
            diagnostics,
            dependencies,
            modules,
            raw_result,
        }
    }

    /// Rewrites Vue 2.7 default exports to an assigned variable.
    pub fn rewrite_vue27_default(
        &self,
        input: &str,
        variable: &str,
        options: Vue27RewriteDefaultOptions,
    ) -> String {
        rewrite_vue27_default(input, variable, options)
    }

    /// Rewrites Vue 3 default exports to an assigned variable.
    pub fn rewrite_vue3_default(
        &self,
        input: &str,
        variable: &str,
        options: Vue3RewriteDefaultOptions,
    ) -> Result<String, String> {
        rewrite_vue3_default(input, variable, options)
    }

    /// Prefixes Vue 2.7 template identifiers for render-function generation.
    pub fn prefix_vue27_identifiers(
        &self,
        input: &str,
        options: Vue27PrefixIdentifiersOptions,
    ) -> String {
        prefix_vue27_identifiers(input, options)
    }

    /// Preprocesses Vue 2.7 template source.
    pub fn preprocess_vue27_template(
        &self,
        source: &str,
        options: Vue27TemplatePreprocessOptions,
    ) -> Vue27TemplatePreprocessResult {
        preprocess_vue27_template(source, options)
    }

    /// Returns the JavaScript side store used by SFC script compilation.
    pub fn js(&self) -> &JsAstStore {
        &self.js
    }

    /// Returns descriptor cache statistics.
    pub fn cache_stats(&self) -> SfcCacheStats {
        self.cache_stats.clone()
    }

    /// Returns the number of cached descriptors.
    pub fn descriptor_cache_len(&self) -> usize {
        self.descriptor_cache.len()
    }

    fn invalidate_stale_descriptor_entries(&mut self, filename: &str, mode: &SfcParseCacheMode) {
        let before = self.descriptor_cache.len();
        self.descriptor_cache
            .retain(|key, _| key.filename != filename || &key.mode != mode);
        let removed = before.saturating_sub(self.descriptor_cache.len());
        self.cache_stats.descriptor_invalidations += removed as u64;
    }
}

impl Default for SfcCompiler {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug)]
struct ExtractedSfcBlocks {
    blocks: Vec<SfcBlock>,
    vue3_errors: Vec<Vue3SfcParseError>,
    errors: Vec<Vue27SfcParseError>,
}

#[derive(Clone, Copy)]
enum SfcBlockContentMode<'a> {
    Vue3 {
        options: &'a Vue3SfcParseOptions,
    },
    Vue27 {
        options: &'a Vue27ParseComponentOptions,
    },
}

impl SfcBlockContentMode<'_> {
    fn is_vue3(&self) -> bool {
        matches!(self, SfcBlockContentMode::Vue3 { .. })
    }

    fn decodes_attr_entities(&self) -> bool {
        matches!(self, SfcBlockContentMode::Vue3 { .. })
    }
}

struct OpenSfcBlock {
    type_name: String,
    attrs: SfcBlockAttrs,
    start: usize,
    open_end: usize,
    self_closing: bool,
}

fn vue3_descriptor_from_blocks(
    filename: String,
    source: &str,
    source_file: FileId,
    blocks: Vec<SfcBlock>,
    options: &Vue3SfcParseOptions,
) -> Vue3SfcParseResult {
    let mut descriptor = SfcDescriptor {
        filename,
        source: source.to_string(),
        source_file,
        template: None,
        script: None,
        script_setup: None,
        styles: Vec::new(),
        custom_blocks: Vec::new(),
    };
    let mut errors = Vec::new();
    let mut has_template_or_script_candidate = false;
    let mut has_script_setup_candidate = false;

    for block in blocks {
        errors.extend(block.attrs.duplicate_attr_errors(source_file));
        if options.ignore_empty
            && block.type_name != "template"
            && !block.attrs.has_src_attr()
            && block.content.trim().is_empty()
            && !block.preserve_empty
        {
            continue;
        }
        match block.type_name.as_str() {
            "template" => {
                has_template_or_script_candidate = true;
                if descriptor.template.is_some() {
                    errors.push(vue3_sfc_parse_block_error(
                        "Single file component can contain only one <template> element",
                        &block,
                    ));
                } else {
                    if let Some(error) = vue3_sfc_functional_template_error(&block) {
                        errors.push(error);
                    }
                    descriptor.template = Some(block);
                }
            }
            "script" => {
                has_template_or_script_candidate = true;
                if block.attrs.setup {
                    if descriptor.script_setup.is_some() {
                        errors.push(vue3_sfc_parse_block_error(
                            "Single file component can contain only one <script setup> element",
                            &block,
                        ));
                    } else {
                        has_script_setup_candidate = true;
                        descriptor.script_setup = Some(block);
                    }
                } else if descriptor.script.is_some() {
                    errors.push(vue3_sfc_parse_block_error(
                        "Single file component can contain only one <script> element",
                        &block,
                    ));
                } else {
                    descriptor.script = Some(block);
                }
            }
            "style" => descriptor.styles.push(block),
            _ => descriptor.custom_blocks.push(block),
        }
    }

    if descriptor
        .script_setup
        .as_ref()
        .is_some_and(|script_setup| script_setup.attrs.has_non_empty_src())
    {
        errors.push(vue3_sfc_parse_error(
            "<script setup> cannot use the \"src\" attribute because its syntax will be ambiguous outside of the component.",
        ));
        descriptor.script_setup = None;
    }
    if has_script_setup_candidate
        && descriptor
            .script
            .as_ref()
            .is_some_and(|script| script.attrs.has_non_empty_src())
    {
        errors.push(vue3_sfc_parse_error(
            "<script> cannot use the \"src\" attribute when <script setup> is also present because they must be processed together.",
        ));
        descriptor.script = None;
    }
    if !has_template_or_script_candidate {
        errors.push(vue3_sfc_parse_error(format!(
            "At least one <template> or <script> is required in a single file component. {}",
            descriptor.filename
        )));
    }
    vue3_dedent_pug_template(&mut descriptor);

    Vue3SfcParseResult { descriptor, errors }
}

fn vue3_sfc_parse_error(message: impl Into<String>) -> Vue3SfcParseError {
    Vue3SfcParseError {
        message: message.into(),
        loc: None,
    }
}

fn vue3_sfc_parse_syntax_error(
    message: impl Into<String>,
    offset: usize,
    source_file: FileId,
) -> Vue3SfcParseError {
    Vue3SfcParseError {
        message: message.into(),
        loc: Some(SfcBlockLocation {
            start: offset,
            end: offset,
            source_file,
        }),
    }
}

fn vue3_sfc_missing_end_tag_error(start: usize, source_file: FileId) -> Vue3SfcParseError {
    vue3_sfc_parse_syntax_error("Element is missing end tag.", start, source_file)
}

fn vue3_sfc_invalid_end_tag_error(start: usize, source_file: FileId) -> Vue3SfcParseError {
    vue3_sfc_parse_syntax_error("Invalid end tag.", start, source_file)
}

fn vue3_sfc_cdata_error(start: usize, source_file: FileId) -> Vue3SfcParseError {
    vue3_sfc_parse_syntax_error(
        "CDATA section is allowed only in XML context.",
        start,
        source_file,
    )
}

fn vue3_sfc_parse_block_error(message: impl Into<String>, block: &SfcBlock) -> Vue3SfcParseError {
    Vue3SfcParseError {
        message: message.into(),
        loc: Some(block.loc.clone()),
    }
}

fn vue3_sfc_functional_template_error(block: &SfcBlock) -> Option<Vue3SfcParseError> {
    if !block.attrs.raw.contains_key("functional") {
        return None;
    }
    Some(Vue3SfcParseError {
        message: "<template functional> is no longer supported in Vue 3, since functional components no longer have significant performance difference from stateful ones. Just use a normal <template> instead.".into(),
        loc: block
            .attrs
            .attr_location("functional", block.loc.source_file),
    })
}

fn vue3_dedent_pug_template(descriptor: &mut SfcDescriptor) {
    let Some(template) = descriptor.template.as_mut() else {
        return;
    };
    if !matches!(template.attrs.lang.as_deref(), Some("pug" | "jade")) {
        return;
    }
    let (content, column_offset) = vue3_dedent_template_content(&template.content);
    template.content = content;
    template.source_map_column_offset = column_offset;
}

fn vue3_dedent_template_content(source: &str) -> (String, usize) {
    let lines = source.split('\n').collect::<Vec<_>>();
    let mut min_indent = usize::MAX;
    for line in &lines {
        if line.trim().is_empty() {
            continue;
        }
        let indent = line.chars().take_while(|ch| ch.is_whitespace()).count();
        min_indent = min_indent.min(indent);
    }
    if min_indent == usize::MAX || min_indent == 0 {
        return (source.to_string(), 0);
    }
    (
        lines
            .iter()
            .map(|line| strip_chars(line, min_indent))
            .collect::<Vec<_>>()
            .join("\n"),
        min_indent,
    )
}

fn descriptor_from_blocks(
    filename: String,
    source: &str,
    source_file: FileId,
    blocks: Vec<SfcBlock>,
) -> SfcDescriptor {
    let mut descriptor = SfcDescriptor {
        filename,
        source: source.to_string(),
        source_file,
        template: None,
        script: None,
        script_setup: None,
        styles: Vec::new(),
        custom_blocks: Vec::new(),
    };

    for block in blocks {
        match block.type_name.as_str() {
            "template" => descriptor.template = Some(block),
            "script" => {
                if block.attrs.setup {
                    descriptor.script_setup = Some(block);
                } else {
                    descriptor.script = Some(block);
                }
            }
            "style" => descriptor.styles.push(block),
            _ => descriptor.custom_blocks.push(block),
        }
    }

    descriptor
}

/// Projects a Rust SFC descriptor into the Vue 3 public `parse()` result shape.
pub fn vue3_sfc_parse_result_value(
    result: &Vue3SfcParseResult,
    options: &Vue3SfcParseProjectionOptions,
) -> serde_json::Value {
    json!({
        "descriptor": vue3_sfc_descriptor_value(&result.descriptor, options),
        "errors": result.errors.iter().map(|error| vue3_sfc_parse_error_value(&result.descriptor, error)).collect::<Vec<_>>(),
    })
}

/// Projects a Rust SFC descriptor into the Vue 3 public descriptor shape.
pub fn vue3_sfc_descriptor_value(
    descriptor: &SfcDescriptor,
    options: &Vue3SfcParseProjectionOptions,
) -> serde_json::Value {
    json!({
        "filename": descriptor.filename,
        "source": descriptor.source,
        "template": descriptor.template.as_ref().map(|block| vue3_sfc_block_value(descriptor, block, options, true)),
        "script": descriptor.script.as_ref().map(|block| vue3_sfc_block_value(descriptor, block, options, true)),
        "scriptSetup": descriptor.script_setup.as_ref().map(|block| vue3_sfc_block_value(descriptor, block, options, false)),
        "styles": descriptor.styles.iter().map(|block| vue3_sfc_block_value(descriptor, block, options, true)).collect::<Vec<_>>(),
        "customBlocks": descriptor.custom_blocks.iter().map(|block| vue3_sfc_block_value(descriptor, block, options, true)).collect::<Vec<_>>(),
        "cssVars": descriptor_css_vars(descriptor, CssVarCollectOptions::default()),
        "slotted": vue3_sfc_descriptor_has_slotted_styles(descriptor),
        "shouldForceReload": serde_json::Value::Null,
    })
}

fn vue3_sfc_block_value(
    descriptor: &SfcDescriptor,
    block: &SfcBlock,
    options: &Vue3SfcParseProjectionOptions,
    include_map: bool,
) -> serde_json::Value {
    let mut value = serde_json::Map::new();
    value.insert("type".into(), json!(block.type_name));
    value.insert("content".into(), json!(block.content));
    value.insert("loc".into(), vue3_sfc_block_loc_value(descriptor, block));
    value.insert("attrs".into(), vue3_sfc_attrs_value(&block.attrs));

    if block.type_name == "script" && block.attrs.setup {
        let setup = block
            .attrs
            .raw
            .get("setup")
            .unwrap_or(&SfcAttrValue::Bool(true));
        value.insert("setup".into(), vue3_sfc_attr_value(setup));
    }
    if let Some(lang) = block.attrs.lang.as_ref() {
        value.insert("lang".into(), json!(lang));
    }
    if let Some(src) = block.attrs.src.as_ref() {
        value.insert("src".into(), json!(src));
    }
    if block.type_name == "style" && block.attrs.scoped {
        value.insert("scoped".into(), json!(true));
    }
    if block.type_name == "style" {
        if let Some(module) = block.attrs.raw.get("module") {
            value.insert("module".into(), vue3_sfc_attr_value(module));
        } else if let Some(module) = block.attrs.module.as_ref() {
            value.insert(
                "module".into(),
                if module.is_empty() {
                    json!(true)
                } else {
                    json!(module)
                },
            );
        }
    }
    if options.source_map && include_map && !block.attrs.has_src_attr() {
        value.insert(
            "map".into(),
            vue3_sfc_block_map_value(descriptor, block, options),
        );
    }

    serde_json::Value::Object(value)
}

fn vue3_sfc_attrs_value(attrs: &SfcBlockAttrs) -> serde_json::Value {
    let mut object = serde_json::Map::new();
    for (name, value) in &attrs.raw {
        object.insert(name.clone(), vue3_sfc_attr_value(value));
    }
    serde_json::Value::Object(object)
}

fn vue3_sfc_attr_value(value: &SfcAttrValue) -> serde_json::Value {
    match value {
        SfcAttrValue::Bool(value) => json!(value),
        SfcAttrValue::String(value) if value.is_empty() => json!(true),
        SfcAttrValue::String(value) => json!(value),
    }
}

fn vue3_sfc_block_loc_value(descriptor: &SfcDescriptor, block: &SfcBlock) -> serde_json::Value {
    let start = block.content_start.min(descriptor.source.len());
    let end = block.content_end.min(descriptor.source.len()).max(start);
    json!({
        "start": vue3_sfc_position_value(&descriptor.source, start),
        "end": vue3_sfc_position_value(&descriptor.source, end),
        "source": descriptor.source.get(start..end).unwrap_or(&block.content),
    })
}

fn vue3_sfc_position_value(source: &str, offset: usize) -> serde_json::Value {
    let mut line = 1usize;
    let mut column = 1usize;
    let mut byte_index = 0usize;
    let mut utf16_offset = 0usize;
    for ch in source.chars() {
        if byte_index >= offset {
            break;
        }
        byte_index += ch.len_utf8();
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += ch.len_utf16();
        }
        utf16_offset += ch.len_utf16();
    }
    if offset > byte_index {
        let extra = offset - byte_index;
        column += extra;
        utf16_offset += extra;
    }
    json!({
        "column": column,
        "line": line,
        "offset": utf16_offset,
    })
}

fn vue3_sfc_block_map_value(
    descriptor: &SfcDescriptor,
    block: &SfcBlock,
    options: &Vue3SfcParseProjectionOptions,
) -> serde_json::Value {
    let filename = descriptor.filename.replace('\\', "/");
    let mut builder = SourceMapBuilder::new().file(filename.clone());
    builder.add_source_content(filename.clone(), descriptor.source.clone());
    let block_start = vue3_sfc_position_value(&descriptor.source, block.content_start);
    let line_offset = if !options.pad.is_enabled() || block.type_name == "template" {
        block_start
            .get("line")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(1)
            .saturating_sub(1) as usize
    } else {
        0
    };
    for (line_index, line) in block.content.split('\n').enumerate() {
        if vue3_sfc_source_map_line_is_empty(line) {
            continue;
        }
        let original_line = line_index + 1 + line_offset;
        let mut generated_column = 0usize;
        for ch in line.chars() {
            if !ch.is_whitespace() {
                let original_column = generated_column + block.source_map_column_offset;
                if let Some(absolute) = byte_offset_at_utf16_line_column(
                    &descriptor.source,
                    original_line,
                    original_column,
                ) {
                    builder.add_mapping(
                        line_index + 1,
                        generated_column,
                        Some(Span::new(descriptor.source_file, absolute, absolute)),
                        Some(filename.clone()),
                    );
                }
            }
            generated_column += ch.len_utf16();
        }
    }
    let mut value = serde_json::to_value(builder.build()).unwrap_or_else(|_| {
        json!({
            "version": 3,
            "sources": [filename],
            "names": [],
            "mappings": "",
            "file": descriptor.filename.replace('\\', "/"),
            "sourcesContent": [descriptor.source],
        })
    });
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "sourceRoot".into(),
            json!(options.source_root.replace('\\', "/")),
        );
    }
    value
}

fn vue3_sfc_source_map_line_is_empty(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.is_empty() || trimmed == "//"
}

fn byte_offset_at_utf16_line_column(source: &str, line: usize, column: usize) -> Option<usize> {
    if line == 0 {
        return None;
    }
    let mut current_line = 1usize;
    let mut line_start = 0usize;
    let bytes = source.as_bytes();
    let mut index = 0usize;
    while current_line < line && index < source.len() {
        match bytes[index] {
            b'\r' => {
                if index + 1 < source.len() && bytes[index + 1] == b'\n' {
                    index += 2;
                } else {
                    index += 1;
                }
                current_line += 1;
                line_start = index;
            }
            b'\n' => {
                index += 1;
                current_line += 1;
                line_start = index;
            }
            _ => index += 1,
        }
    }
    if current_line != line {
        return None;
    }
    let line_end = source[line_start..]
        .find(['\r', '\n'])
        .map(|offset| line_start + offset)
        .unwrap_or(source.len());
    let mut current_column = 0usize;
    let mut cursor = line_start;
    while cursor <= line_end {
        if current_column == column {
            return Some(cursor);
        }
        if cursor == line_end {
            break;
        }
        let ch = source[cursor..line_end].chars().next()?;
        current_column += ch.len_utf16();
        cursor += ch.len_utf8();
        if current_column > column {
            return None;
        }
    }
    (current_column == column).then_some(cursor)
}

fn vue3_sfc_descriptor_has_slotted_styles(descriptor: &SfcDescriptor) -> bool {
    descriptor.styles.iter().any(|style| {
        style.attrs.scoped
            && (style.content.contains(":slotted(") || style.content.contains("::v-slotted("))
    })
}

fn vue3_sfc_parse_error_value(
    descriptor: &SfcDescriptor,
    error: &Vue3SfcParseError,
) -> serde_json::Value {
    let mut value = serde_json::Map::new();
    value.insert("message".into(), json!(error.message));
    if let Some(loc) = error.loc.as_ref() {
        let start = loc.start.min(descriptor.source.len());
        let end = if loc.end == 0 {
            start
        } else {
            loc.end.min(descriptor.source.len()).max(start)
        };
        value.insert(
            "loc".into(),
            json!({
                "start": vue3_sfc_position_value(&descriptor.source, start),
                "end": vue3_sfc_position_value(&descriptor.source, end),
                "source": descriptor.source.get(start..end).unwrap_or_default(),
            }),
        );
    }
    serde_json::Value::Object(value)
}

fn project_vue27_errors(
    errors: Vec<Vue27SfcParseError>,
    output_source_range: bool,
) -> Vec<Vue27SfcParseError> {
    if output_source_range {
        return errors;
    }
    errors
        .into_iter()
        .map(|error| Vue27SfcParseError {
            msg: error.msg,
            start: None,
            end: None,
        })
        .collect()
}

fn extract_sfc_blocks(
    source: &str,
    source_file: FileId,
    mode: SfcBlockContentMode<'_>,
) -> ExtractedSfcBlocks {
    let mut blocks = Vec::new();
    let mut vue3_errors = Vec::new();
    let mut errors = Vec::new();
    let mut stack: Vec<(String, usize, usize)> = Vec::new();
    let mut current_block: Option<OpenSfcBlock> = None;
    let mut depth = 0usize;
    let mut malformed_tail_start = None;
    let mut vue3_terminal_root_cdata_start = None;
    let mut vue3_terminal_root_invalid_end_start = None;
    let mut tokenizer = HtmlTokenizer::new(source);

    loop {
        let token = tokenizer.next_token();
        match token.kind {
            HtmlTokenKind::StartTag {
                name,
                attributes,
                self_closing,
            } => {
                let is_vue3_template_content = current_block
                    .as_ref()
                    .is_some_and(|block| block.type_name == "template");
                if mode.is_vue3() && (depth == 0 || is_vue3_template_content) {
                    vue3_collect_sfc_attr_syntax_errors(
                        &attributes,
                        source_file,
                        depth > 0 && is_vue3_template_content,
                        &mut vue3_errors,
                    );
                    vue3_terminal_root_cdata_start = None;
                    vue3_terminal_root_invalid_end_start = None;
                }
                if depth == 0 {
                    current_block = Some(OpenSfcBlock {
                        type_name: name.clone(),
                        attrs: attrs_from_html(&attributes, mode.decodes_attr_entities()),
                        start: token.start,
                        open_end: token.end,
                        self_closing,
                    });
                }

                if !self_closing {
                    if depth == 0 && is_plain_text_sfc_tag(&name) {
                        consume_plain_text_element(
                            source,
                            source_file,
                            mode,
                            &mut tokenizer,
                            &mut blocks,
                            &mut vue3_errors,
                            &mut current_block,
                            token.end,
                        );
                        depth = 0;
                    } else {
                        stack.push((name, token.start, token.end));
                        depth += 1;
                    }
                } else if depth == 0 {
                    if let Some(open) = current_block.take() {
                        blocks.push(finish_sfc_block(
                            source,
                            source_file,
                            mode,
                            open,
                            0,
                            token.end,
                            false,
                        ));
                    }
                }
            }
            HtmlTokenKind::EndTag { name } => {
                if depth == 0 {
                    if mode.is_vue3() && !name.is_empty() {
                        vue3_errors.push(vue3_sfc_invalid_end_tag_error(token.start, source_file));
                    }
                    continue;
                }
                let Some(pos) = matching_open_pos(&stack, &name, mode.is_vue3()) else {
                    if name.is_empty() {
                        malformed_tail_start.get_or_insert(token.start);
                    } else if name.eq_ignore_ascii_case("br") && depth == 0 {
                        current_block = Some(OpenSfcBlock {
                            type_name: name,
                            attrs: SfcBlockAttrs::default(),
                            start: token.start,
                            open_end: token.end,
                            self_closing: true,
                        });
                    } else if mode.is_vue3()
                        && current_block
                            .as_ref()
                            .is_some_and(|block| block.type_name == "template")
                    {
                        vue3_errors.push(vue3_sfc_invalid_end_tag_error(token.start, source_file));
                        if depth == 1 {
                            vue3_terminal_root_invalid_end_start = Some(token.start);
                            vue3_terminal_root_cdata_start = None;
                        }
                    }
                    continue;
                };
                let mut emitted_vue3_missing_child = false;
                while stack.len() > pos + 1 {
                    if let Some((tag, start, end)) = stack.pop() {
                        if mode.is_vue3()
                            && current_block
                                .as_ref()
                                .is_some_and(|block| block.type_name == "template")
                            && !emitted_vue3_missing_child
                        {
                            vue3_errors.push(vue3_sfc_missing_end_tag_error(start, source_file));
                            emitted_vue3_missing_child = true;
                        }
                        errors.push(Vue27SfcParseError {
                            msg: format!("tag <{tag}> has no matching end tag."),
                            start: Some(start),
                            end: Some(end),
                        });
                        depth = depth.saturating_sub(1);
                    }
                }
                stack.pop();
                if depth == 1 {
                    if let Some(open) = current_block.take() {
                        let content_end =
                            if mode.is_vue3() && open.type_name == "template" && pos == 0 {
                                vue3_terminal_root_cdata_start
                                    .take()
                                    .or_else(|| vue3_terminal_root_invalid_end_start.take())
                                    .unwrap_or(token.start)
                            } else {
                                token.start
                            };
                        blocks.push(finish_sfc_block(
                            source,
                            source_file,
                            mode,
                            open,
                            content_end,
                            token.end,
                            false,
                        ));
                    }
                }
                depth = depth.saturating_sub(1);
            }
            HtmlTokenKind::BogusQuestionTag => {
                if mode.is_vue3()
                    && current_block
                        .as_ref()
                        .is_none_or(|block| block.type_name == "template")
                {
                    vue3_errors.push(vue3_sfc_parse_syntax_error(
                        "'<?' is allowed only in XML context.",
                        token.start.saturating_add(1),
                        source_file,
                    ));
                }
            }
            HtmlTokenKind::Cdata(_) => {
                if mode.is_vue3()
                    && current_block
                        .as_ref()
                        .is_some_and(|block| block.type_name == "template")
                {
                    vue3_errors.push(vue3_sfc_cdata_error(token.start, source_file));
                    if depth == 1 {
                        vue3_terminal_root_cdata_start = Some(token.start);
                        vue3_terminal_root_invalid_end_start = None;
                    }
                }
            }
            HtmlTokenKind::Eof => {
                let is_vue3_template = mode.is_vue3()
                    && current_block
                        .as_ref()
                        .is_some_and(|block| block.type_name == "template");
                let is_vue3 = mode.is_vue3();
                while let Some((tag, start, end)) = stack.pop() {
                    if is_vue3 && (is_vue3_template || stack.is_empty()) {
                        vue3_errors.push(vue3_sfc_missing_end_tag_error(start, source_file));
                    }
                    errors.push(Vue27SfcParseError {
                        msg: format!("tag <{tag}> has no matching end tag."),
                        start: Some(start),
                        end: Some(end),
                    });
                    if stack.is_empty() {
                        if let Some(open) = current_block.take() {
                            let fallback_end = if mode.is_vue3() {
                                open.open_end
                            } else {
                                malformed_tail_start.unwrap_or_else(|| {
                                    malformed_tail_content_end(source, &open, token.start)
                                })
                            };
                            blocks.push(finish_sfc_block(
                                source,
                                source_file,
                                mode,
                                open,
                                fallback_end,
                                token.end,
                                mode.is_vue3(),
                            ));
                        }
                    }
                }
                break;
            }
            HtmlTokenKind::Text(_) | HtmlTokenKind::Comment(_) | HtmlTokenKind::Doctype(_) => {
                vue3_terminal_root_cdata_start = None;
                vue3_terminal_root_invalid_end_start = None;
            }
        }
    }

    blocks.sort_by_key(|block| block.loc.start);
    ExtractedSfcBlocks {
        blocks,
        vue3_errors,
        errors,
    }
}

fn consume_plain_text_element(
    source: &str,
    source_file: FileId,
    mode: SfcBlockContentMode<'_>,
    tokenizer: &mut HtmlTokenizer<'_>,
    blocks: &mut Vec<SfcBlock>,
    vue3_errors: &mut Vec<Vue3SfcParseError>,
    current_block: &mut Option<OpenSfcBlock>,
    content_start: usize,
) {
    let Some(open) = current_block.take() else {
        return;
    };
    let lower_name = open.type_name.to_ascii_lowercase();
    let rest = &source[content_start..];
    let needle = format!("</{lower_name}");
    if let Some(close_offset) = find_ascii_case_insensitive(rest, &needle) {
        let close_start = content_start + close_offset;
        let close_end = source[close_start..]
            .find('>')
            .map(|offset| close_start + offset + 1)
            .unwrap_or(source.len());
        tokenizer.set_cursor(close_end);
        blocks.push(finish_sfc_block(
            source,
            source_file,
            mode,
            open,
            close_start,
            close_end,
            false,
        ));
    } else {
        tokenizer.set_cursor(source.len());
        let content_end = if mode.is_vue3() {
            vue3_errors.push(vue3_sfc_missing_end_tag_error(open.start, source_file));
            open.open_end
        } else {
            source.len()
        };
        blocks.push(finish_sfc_block(
            source,
            source_file,
            mode,
            open,
            content_end,
            source.len(),
            mode.is_vue3(),
        ));
    }
}

fn finish_sfc_block(
    source: &str,
    source_file: FileId,
    mode: SfcBlockContentMode<'_>,
    open: OpenSfcBlock,
    content_end: usize,
    close_end: usize,
    preserve_empty: bool,
) -> SfcBlock {
    let content_start = open.open_end.min(source.len());
    let raw_end = content_end.min(source.len()).max(content_start);
    let mut content = source[content_start..raw_end].to_string();
    match mode {
        SfcBlockContentMode::Vue3 { options } => {
            if open.type_name != "template" && options.pad.is_enabled() {
                content = vue3_pad_content(source, &open, &options.pad) + &content;
            }
        }
        SfcBlockContentMode::Vue27 { options } => {
            if should_vue27_deindent(&open, options) {
                content = deindent(&content);
            }
            if open.type_name != "template" && options.pad.is_enabled() {
                content = vue27_pad_content(source, &open, &options.pad) + &content;
            }
        }
    }

    SfcBlock {
        type_name: open.type_name,
        content,
        attrs: open.attrs,
        loc: SfcBlockLocation {
            start: open.start,
            end: if open.self_closing { 0 } else { close_end },
            source_file,
        },
        content_start,
        content_end: raw_end,
        source_map_column_offset: 0,
        preserve_empty,
    }
}

fn matching_open_pos(
    stack: &[(String, usize, usize)],
    name: &str,
    vue3_sfc_mode: bool,
) -> Option<usize> {
    let lower_name = name.to_ascii_lowercase();
    stack.iter().enumerate().rposition(|(index, (tag, _, _))| {
        if vue3_sfc_mode && index == 0 && has_ascii_uppercase(tag) {
            return false;
        }
        tag.to_ascii_lowercase() == lower_name
    })
}

fn has_ascii_uppercase(source: &str) -> bool {
    source.bytes().any(|byte| byte.is_ascii_uppercase())
}

fn malformed_tail_content_end(source: &str, open: &OpenSfcBlock, fallback: usize) -> usize {
    let fallback = fallback.min(source.len());
    let tail = &source[open.open_end.min(source.len())..fallback];
    let Some(last_lt) = tail.rfind('<') else {
        return fallback;
    };
    let absolute = open.open_end + last_lt;
    if source[absolute..fallback].contains('>') {
        return fallback;
    }
    absolute
}

fn vue3_collect_sfc_attr_syntax_errors(
    attributes: &[HtmlAttribute],
    source_file: FileId,
    include_duplicates: bool,
    errors: &mut Vec<Vue3SfcParseError>,
) {
    let mut seen = BTreeSet::new();
    for attribute in attributes {
        if include_duplicates && !seen.insert(attribute.name.as_str()) {
            errors.push(vue3_sfc_parse_syntax_error(
                "Duplicate attribute.",
                attribute.name_start,
                source_file,
            ));
        }
        if attribute.name.starts_with('=') {
            errors.push(vue3_sfc_parse_syntax_error(
                "Attribute name cannot start with '='.",
                attribute.name_start,
                source_file,
            ));
        }
        if matches!(attribute.quote, Some(HtmlQuoteKind::Unquoted))
            && attribute.value_content_start == attribute.value_content_end
        {
            let offset = attribute.value_start.unwrap_or(attribute.name_end);
            errors.push(vue3_sfc_parse_syntax_error(
                "Attribute value was expected.",
                offset,
                source_file,
            ));
        }
    }
}

fn attrs_from_html(attributes: &[HtmlAttribute], decode_entities: bool) -> SfcBlockAttrs {
    let mut attrs = SfcBlockAttrs::default();
    for attribute in attributes {
        let value = attribute
            .value
            .as_ref()
            .map(|value| {
                let value = if decode_entities {
                    decode_html_attr_entities(value)
                } else {
                    value.clone()
                };
                SfcAttrValue::String(value)
            })
            .unwrap_or(SfcAttrValue::Bool(true));
        if attrs.raw.contains_key(&attribute.name) {
            attrs.duplicate_attr_starts.push(attribute.name_start);
        }
        attrs.raw.insert(attribute.name.clone(), value.clone());
        attrs
            .ranges
            .insert(attribute.name.clone(), (attribute.start, attribute.end));
        match attribute.name.as_str() {
            "lang" => {
                if let SfcAttrValue::String(value) = value {
                    attrs.lang = Some(value);
                }
            }
            "src" => {
                if let SfcAttrValue::String(value) = value {
                    attrs.src = Some(value);
                }
            }
            "scoped" => {
                attrs.scoped = true;
            }
            "setup" => {
                attrs.setup = true;
            }
            "generic" => {
                if let SfcAttrValue::String(value) = value {
                    attrs.generic = Some(value);
                }
            }
            "module" => {
                attrs.module = Some(match value {
                    SfcAttrValue::String(value) => value,
                    SfcAttrValue::Bool(_) => String::new(),
                });
            }
            _ => {}
        }
    }
    attrs
}

fn is_plain_text_sfc_tag(name: &str) -> bool {
    matches!(name, "script" | "style")
}

fn should_vue27_deindent(block: &OpenSfcBlock, options: &Vue27ParseComponentOptions) -> bool {
    if options.deindent == Some(true) {
        return true;
    }
    if options.deindent == Some(false) {
        return false;
    }
    !(block.type_name == "script"
        && block
            .attrs
            .lang
            .as_deref()
            .is_none_or(|lang| matches!(lang, "js" | "jsx" | "ts" | "tsx")))
}

fn deindent(source: &str) -> String {
    if !source
        .chars()
        .next()
        .is_some_and(|ch| matches!(ch, '\r' | '\n' | ' ' | '\t'))
    {
        return source.to_string();
    }
    let mut indent_char = None;
    let mut min_indent = usize::MAX;
    let lines = split_preserving_no_cr(source);
    for line in &lines {
        if line.chars().all(char::is_whitespace) {
            continue;
        }
        match indent_char {
            None => {
                let Some(ch) = line.chars().next() else {
                    continue;
                };
                if ch != ' ' && ch != '\t' {
                    return source.to_string();
                }
                indent_char = Some(ch);
                min_indent = min_indent.min(line.chars().take_while(|value| *value == ch).count());
            }
            Some(ch) => {
                min_indent = min_indent.min(line.chars().take_while(|value| *value == ch).count());
            }
        }
    }
    if min_indent == usize::MAX || min_indent == 0 {
        return source.to_string();
    }
    lines
        .iter()
        .map(|line| strip_chars(line, min_indent))
        .collect::<Vec<_>>()
        .join("\n")
}

fn split_preserving_no_cr(source: &str) -> Vec<String> {
    source
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line).to_string())
        .collect()
}

fn strip_chars(source: &str, count: usize) -> String {
    let mut cursor = 0usize;
    for _ in 0..count {
        let Some(ch) = source[cursor..].chars().next() else {
            return String::new();
        };
        cursor += ch.len_utf8();
    }
    source[cursor..].to_string()
}

impl Vue27SfcPad {
    fn is_enabled(&self) -> bool {
        !matches!(self, Vue27SfcPad::False)
    }
}

impl Vue3SfcPad {
    fn is_enabled(&self) -> bool {
        !matches!(self, Vue3SfcPad::False)
    }
}

fn vue3_pad_content(source: &str, block: &OpenSfcBlock, pad: &Vue3SfcPad) -> String {
    if matches!(pad, Vue3SfcPad::Space) {
        return source[..block.open_end]
            .chars()
            .map(|ch| if matches!(ch, '\n' | '\r') { ch } else { ' ' })
            .collect();
    }
    let offset = source[..block.open_end].split('\n').count();
    let pad_char = if block.type_name == "script" && block.attrs.lang.is_none() {
        "//\n"
    } else {
        "\n"
    };
    pad_char.repeat(offset.saturating_sub(1))
}

fn vue27_pad_content(source: &str, block: &OpenSfcBlock, pad: &Vue27SfcPad) -> String {
    if matches!(pad, Vue27SfcPad::Space) {
        return source[..block.open_end]
            .chars()
            .map(|ch| if matches!(ch, '\n' | '\r') { ch } else { ' ' })
            .collect();
    }
    let offset = source[..block.open_end].split('\n').count();
    let pad_char = if block.type_name == "script" && block.attrs.lang.is_none() {
        "//\n"
    } else {
        "\n"
    };
    pad_char.repeat(offset.saturating_sub(1))
}

fn find_ascii_case_insensitive(haystack: &str, needle: &str) -> Option<usize> {
    haystack
        .to_ascii_lowercase()
        .find(&needle.to_ascii_lowercase())
}

fn rewrite_vue27_default(
    input: &str,
    variable: &str,
    options: Vue27RewriteDefaultOptions,
) -> String {
    let allocator = oxc_allocator::Allocator::default();
    let source_type = if options.typescript {
        oxc_span::SourceType::ts()
    } else {
        oxc_span::SourceType::mjs()
    };
    let parsed = oxc_parser::Parser::new(&allocator, input, source_type)
        .with_options(oxc_parser::ParseOptions {
            parse_regular_expression: true,
            ..oxc_parser::ParseOptions::default()
        })
        .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        if !options.typescript {
            let ts_parsed = oxc_parser::Parser::new(&allocator, input, oxc_span::SourceType::ts())
                .with_options(oxc_parser::ParseOptions {
                    parse_regular_expression: true,
                    ..oxc_parser::ParseOptions::default()
                })
                .parse();
            if !ts_parsed.panicked && ts_parsed.errors.is_empty() {
                return rewrite_vue27_default_from_program(
                    input,
                    variable,
                    &ts_parsed.program.body,
                );
            }
        }
        return rewrite_vue27_default_lexical(input, variable);
    }

    rewrite_vue27_default_from_program(input, variable, &parsed.program.body)
}

fn rewrite_vue3_default(
    input: &str,
    variable: &str,
    options: Vue3RewriteDefaultOptions,
) -> Result<String, String> {
    let allocator = oxc_allocator::Allocator::default();
    let source_type = if options.typescript {
        oxc_span::SourceType::ts()
    } else {
        oxc_span::SourceType::mjs()
    };
    let parsed = oxc_parser::Parser::new(&allocator, input, source_type)
        .with_options(oxc_parser::ParseOptions {
            parse_regular_expression: true,
            ..oxc_parser::ParseOptions::default()
        })
        .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return Err(parsed
            .errors
            .first()
            .map(ToString::to_string)
            .unwrap_or_else(|| "failed to parse default export".into()));
    }
    if !options.typescript {
        if let Some(offset) = vue3_typescript_default_export_start(&parsed.program.body) {
            let (line, column) = line_column(input, offset);
            return Err(format!(
                "Unexpected reserved word 'interface'. ({line}:{column})"
            ));
        }
    }

    Ok(rewrite_vue3_default_from_program(
        input,
        variable,
        &parsed.program.body,
    ))
}

fn rewrite_vue27_default_from_program(
    input: &str,
    variable: &str,
    body: &[Statement<'_>],
) -> String {
    let mut edits = SourceEdits::new(input);
    let mut found_default = false;
    for statement in body {
        match statement {
            Statement::ExportDefaultDeclaration(declaration) => {
                found_default = true;
                rewrite_export_default(input, variable, declaration, &mut edits);
            }
            Statement::ExportNamedDeclaration(declaration) => {
                if rewrite_named_default_exports(input, variable, declaration, &mut edits) {
                    found_default = true;
                }
            }
            _ => {}
        }
    }
    if !found_default {
        edits.append(format!("\nconst {variable} = {{}}"));
    }
    edits.apply()
}

fn rewrite_vue3_default_from_program(
    input: &str,
    variable: &str,
    body: &[Statement<'_>],
) -> String {
    let mut edits = SourceEdits::new(input);
    let mut found_default = false;
    for statement in body {
        match statement {
            Statement::ExportDefaultDeclaration(declaration) => {
                found_default = true;
                rewrite_vue3_export_default(variable, declaration, &mut edits);
            }
            Statement::ExportNamedDeclaration(declaration) => {
                if rewrite_vue3_named_default_exports(input, variable, declaration, &mut edits) {
                    found_default = true;
                }
            }
            _ => {}
        }
    }
    if !found_default {
        edits.append(format!("\nconst {variable} = {{}}"));
    }
    edits.apply()
}

fn vue3_typescript_default_export_start(body: &[Statement<'_>]) -> Option<usize> {
    body.iter().find_map(|statement| match statement {
        Statement::ExportDefaultDeclaration(declaration) => match &declaration.declaration {
            ExportDefaultDeclarationKind::TSInterfaceDeclaration(declaration) => {
                Some(declaration.span.start as usize)
            }
            _ => None,
        },
        _ => None,
    })
}

fn line_column(input: &str, offset: usize) -> (usize, usize) {
    let mut line = 1usize;
    let mut column = 0usize;
    for ch in input[..offset.min(input.len())].chars() {
        if ch == '\n' {
            line += 1;
            column = 0;
        } else {
            column += 1;
        }
    }
    (line, column)
}

fn rewrite_export_default(
    input: &str,
    variable: &str,
    declaration: &ExportDefaultDeclaration<'_>,
    edits: &mut SourceEdits,
) {
    match &declaration.declaration {
        ExportDefaultDeclarationKind::ClassDeclaration(class) => {
            if let Some(id) = &class.id {
                let fast_candidate = source_with_overwrite(
                    input,
                    declaration.span.start as usize,
                    id.span.start as usize,
                    "class ",
                );
                if has_vue27_default_export_like(input)
                    && has_vue27_default_export_like(&fast_candidate)
                {
                    let replace_start = class
                        .decorators
                        .last()
                        .map(|decorator| decorator.span.end as usize)
                        .unwrap_or(declaration.span.start as usize);
                    edits.overwrite(replace_start, id.span.start as usize, " class ");
                } else {
                    edits.overwrite(
                        declaration.span.start as usize,
                        id.span.start as usize,
                        "class ",
                    );
                }
                edits.append(format!("\nconst {variable} = {}", id.name));
                return;
            }
        }
        ExportDefaultDeclarationKind::FunctionDeclaration(function) => {
            if let Some(id) = &function.id {
                edits.overwrite(
                    declaration.span.start as usize,
                    function.span.start as usize,
                    "",
                );
                edits.append(format!("\nconst {variable} = {}", id.name));
                return;
            }
        }
        _ => {}
    }

    edits.overwrite(
        declaration.span.start as usize,
        export_default_declaration_value_start(input, declaration),
        format!("const {variable} ="),
    );
}

fn rewrite_vue3_export_default(
    variable: &str,
    declaration: &ExportDefaultDeclaration<'_>,
    edits: &mut SourceEdits,
) {
    if let ExportDefaultDeclarationKind::ClassDeclaration(class) = &declaration.declaration {
        if let Some(id) = &class.id {
            let replace_start = class
                .decorators
                .last()
                .map(|decorator| decorator.span.end as usize)
                .unwrap_or(declaration.span.start as usize);
            edits.overwrite(replace_start, id.span.start as usize, " class ");
            edits.append(format!("\nconst {variable} = {}", id.name));
            return;
        }
    }

    edits.overwrite(
        declaration.span.start as usize,
        declaration.declaration.span().start as usize,
        format!("const {variable} = "),
    );
}

fn export_default_declaration_value_start(
    input: &str,
    declaration: &ExportDefaultDeclaration<'_>,
) -> usize {
    let start = declaration.span.start as usize;
    let end = declaration.declaration.span().start as usize;
    let segment = &input[start..end.min(input.len())];
    segment
        .find("default")
        .map(|offset| start + offset + "default".len())
        .unwrap_or(end)
}

fn rewrite_named_default_exports(
    input: &str,
    variable: &str,
    declaration: &ExportNamedDeclaration<'_>,
    edits: &mut SourceEdits,
) -> bool {
    let mut found = false;
    for specifier in &declaration.specifiers {
        if module_export_name(specifier.exported()) != Some("default") {
            continue;
        }
        found = true;
        let local_name = module_export_name(specifier.local()).unwrap_or("default");
        if let Some(source) = declaration.source.as_ref() {
            let source_value = source.value.to_string();
            if local_name == "default" {
                let end = specifier_end(
                    input,
                    specifier.local().span().end as usize,
                    declaration.span.end as usize,
                );
                edits.prepend(format!(
                    "import {{ default as __VUE_DEFAULT__ }} from '{}'\n",
                    source_value
                ));
                edits.overwrite(specifier.span.start as usize, end, "");
                edits.append(format!("\nconst {variable} = __VUE_DEFAULT__"));
            } else {
                let end = specifier_end(
                    input,
                    specifier.exported().span().end as usize,
                    declaration.span.end as usize,
                );
                edits.prepend(format!("import {{ {local_name} }} from '{source_value}'\n"));
                edits.overwrite(specifier.span.start as usize, end, "");
                edits.append(format!("\nconst {variable} = {local_name}"));
            }
        } else {
            let end = specifier_end(
                input,
                specifier.span.end as usize,
                declaration.span.end as usize,
            );
            edits.overwrite(specifier.span.start as usize, end, "");
            edits.append(format!("\nconst {variable} = {local_name}"));
        }
    }
    found
}

fn rewrite_vue3_named_default_exports(
    input: &str,
    variable: &str,
    declaration: &ExportNamedDeclaration<'_>,
    edits: &mut SourceEdits,
) -> bool {
    let mut found = false;
    for specifier in &declaration.specifiers {
        if module_export_name(specifier.exported()) != Some("default") {
            continue;
        }
        found = true;
        let local_name = module_export_name(specifier.local()).unwrap_or("default");
        if let Some(source) = declaration.source.as_ref() {
            let source_value = source.value.to_string();
            if local_name == "default" {
                let end = specifier_end(
                    input,
                    specifier.local().span().end as usize,
                    declaration.span.end as usize,
                );
                edits.prepend(format!(
                    "import {{ default as __VUE_DEFAULT__ }} from '{}'\n",
                    source_value
                ));
                edits.remove(specifier.span.start as usize, end);
                edits.append(format!("\nconst {variable} = __VUE_DEFAULT__"));
            } else {
                let end = specifier_end(
                    input,
                    specifier.exported().span().end as usize,
                    declaration.span.end as usize,
                );
                let local_source = &input[specifier.local().span().start as usize
                    ..specifier.local().span().end as usize];
                edits.prepend(format!(
                    "import {{ {local_source} as __VUE_DEFAULT__ }} from '{}'\n",
                    source_value
                ));
                edits.remove(specifier.span.start as usize, end);
                edits.append(format!("\nconst {variable} = __VUE_DEFAULT__"));
            }
        } else {
            let end = specifier_end(
                input,
                specifier.span.end as usize,
                declaration.span.end as usize,
            );
            edits.remove(specifier.span.start as usize, end);
            edits.append(format!("\nconst {variable} = {local_name}"));
        }
    }
    found
}

fn export_named_declaration_only_exports_default(declaration: &ExportNamedDeclaration<'_>) -> bool {
    !declaration.specifiers.is_empty()
        && declaration
            .specifiers
            .iter()
            .all(|specifier| module_export_name(specifier.exported()) == Some("default"))
}

trait ExportSpecifierAccess<'a> {
    fn local(&self) -> &ModuleExportName<'a>;
    fn exported(&self) -> &ModuleExportName<'a>;
}

impl<'a> ExportSpecifierAccess<'a> for ExportSpecifier<'a> {
    fn local(&self) -> &ModuleExportName<'a> {
        &self.local
    }

    fn exported(&self) -> &ModuleExportName<'a> {
        &self.exported
    }
}

fn module_export_name<'a>(name: &'a ModuleExportName<'a>) -> Option<&'a str> {
    match name {
        ModuleExportName::IdentifierName(identifier) => Some(identifier.name.as_str()),
        ModuleExportName::IdentifierReference(identifier) => Some(identifier.name.as_str()),
        ModuleExportName::StringLiteral(literal) => Some(literal.value.as_str()),
    }
}

fn specifier_end(input: &str, mut end: usize, node_end: usize) -> usize {
    let node_end = node_end.min(input.len());
    let old_end = end;
    let mut has_comma = false;
    while end < node_end {
        let Some(ch) = input[end..].chars().next() else {
            break;
        };
        if ch.is_whitespace() {
            end += ch.len_utf8();
        } else if ch == ',' {
            end += ch.len_utf8();
            has_comma = true;
            break;
        } else if ch == '}' {
            break;
        } else {
            break;
        }
    }
    if has_comma {
        end
    } else {
        old_end
    }
}

fn rewrite_vue27_default_lexical(input: &str, variable: &str) -> String {
    let Some(default_start) = find_export_default_keyword(input) else {
        return format!("{input}\nconst {variable} = {{}}");
    };
    let value_start = default_start + "default".len();
    let export_start = input[..default_start]
        .rfind("export")
        .unwrap_or(default_start);
    let mut output = String::new();
    output.push_str(&input[..export_start]);
    output.push_str(&format!("const {variable} ="));
    output.push_str(&input[value_start..]);
    output
}

fn find_export_default_keyword(input: &str) -> Option<usize> {
    let mut index = 0usize;
    while index < input.len() {
        let next = input[index..].find("export")? + index;
        if is_word_boundary(input, next, "export")
            && input[next + "export".len()..]
                .trim_start()
                .starts_with("default")
        {
            let default_start = next
                + "export".len()
                + input[next + "export".len()..]
                    .len()
                    .saturating_sub(input[next + "export".len()..].trim_start().len());
            if is_word_boundary(input, default_start, "default") {
                return Some(default_start);
            }
        }
        index = next + "export".len();
    }
    None
}

fn is_word_boundary(input: &str, start: usize, word: &str) -> bool {
    let before = input[..start].chars().next_back();
    let after = input[start + word.len()..].chars().next();
    !before.is_some_and(is_identifier_continue) && !after.is_some_and(is_identifier_continue)
}

fn is_identifier_continue(ch: char) -> bool {
    ch == '_' || ch == '$' || ch.is_ascii_alphanumeric()
}

fn prefix_vue27_identifiers(input: &str, options: Vue27PrefixIdentifiersOptions) -> String {
    let allocator = oxc_allocator::Allocator::default();
    let source_type = if options.is_ts {
        oxc_span::SourceType::ts()
    } else {
        oxc_span::SourceType::script()
    };
    let parsed = oxc_parser::Parser::new(&allocator, input, source_type)
        .with_options(oxc_parser::ParseOptions {
            parse_regular_expression: true,
            ..oxc_parser::ParseOptions::default()
        })
        .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return input.to_string();
    }

    let mut edits = SourceEdits::new(input);
    let mut context = PrefixIdentifiersContext {
        input,
        options,
        locals: Vec::new(),
        edits: &mut edits,
    };
    for statement in &parsed.program.body {
        context.walk_statement(statement);
    }
    edits.apply()
}

struct PrefixIdentifiersContext<'a, 'b> {
    input: &'a str,
    options: Vue27PrefixIdentifiersOptions,
    locals: Vec<BTreeMap<String, usize>>,
    edits: &'b mut SourceEdits<'a>,
}

impl PrefixIdentifiersContext<'_, '_> {
    fn walk_statement(&mut self, statement: &Statement<'_>) {
        match statement {
            Statement::WithStatement(statement) => self.walk_with_statement(statement),
            Statement::BlockStatement(block) => {
                self.push_scope();
                self.mark_block_declarations(&block.body);
                for statement in &block.body {
                    self.walk_statement(statement);
                }
                self.pop_scope();
            }
            Statement::ExpressionStatement(statement) => {
                self.walk_expression(&statement.expression)
            }
            Statement::ReturnStatement(statement) => {
                if let Some(argument) = &statement.argument {
                    self.walk_expression(argument);
                }
            }
            Statement::VariableDeclaration(declaration) => {
                self.mark_variable_declaration(declaration);
                for declarator in &declaration.declarations {
                    if let Some(init) = &declarator.init {
                        self.walk_expression(init);
                    }
                }
            }
            Statement::FunctionDeclaration(function) => self.walk_function(function),
            Statement::IfStatement(statement) => {
                self.walk_expression(&statement.test);
                self.walk_statement(&statement.consequent);
                if let Some(alternate) = &statement.alternate {
                    self.walk_statement(alternate);
                }
            }
            Statement::ForStatement(statement) => {
                self.push_scope();
                if let Some(init) = &statement.init {
                    match init {
                        oxc_ast::ast::ForStatementInit::VariableDeclaration(declaration) => {
                            self.mark_variable_declaration(declaration);
                            for declarator in &declaration.declarations {
                                if let Some(init) = &declarator.init {
                                    self.walk_expression(init);
                                }
                            }
                        }
                        _ => {
                            if let Some(expression) = init.as_expression() {
                                self.walk_expression(expression);
                            }
                        }
                    }
                }
                if let Some(test) = &statement.test {
                    self.walk_expression(test);
                }
                if let Some(update) = &statement.update {
                    self.walk_expression(update);
                }
                self.walk_statement(&statement.body);
                self.pop_scope();
            }
            Statement::ForInStatement(statement) => {
                self.push_scope();
                self.walk_for_iteration_left(&statement.left);
                self.walk_expression(&statement.right);
                self.walk_statement(&statement.body);
                self.pop_scope();
            }
            Statement::ForOfStatement(statement) => {
                self.push_scope();
                self.walk_for_iteration_left(&statement.left);
                self.walk_expression(&statement.right);
                self.walk_statement(&statement.body);
                self.pop_scope();
            }
            _ => {}
        }
    }

    fn walk_with_statement(&mut self, statement: &WithStatement<'_>) {
        if !self.options.is_functional {
            self.edits.prepend_right(
                statement.span.start as usize,
                if self.is_script_setup() {
                    "var _vm=this,_c=_vm._self._c,_setup=_vm._self._setupProxy;"
                } else {
                    "var _vm=this,_c=_vm._self._c;"
                },
            );
        }
        let Some(body_start) = self.with_body_content_start(statement) else {
            self.walk_statement(&statement.body);
            return;
        };
        self.edits.remove(statement.span.start as usize, body_start);
        self.edits.remove(
            statement.span.end.saturating_sub(1) as usize,
            statement.span.end as usize,
        );
        self.walk_statement(&statement.body);
    }

    fn with_body_content_start(&self, statement: &WithStatement<'_>) -> Option<usize> {
        let start = statement.body.span().start as usize;
        let body_source = self.input.get(start..)?;
        body_source.find('{').map(|offset| start + offset + 1)
    }

    fn walk_for_iteration_left(&mut self, left: &oxc_ast::ast::ForStatementLeft<'_>) {
        match left {
            oxc_ast::ast::ForStatementLeft::VariableDeclaration(declaration) => {
                self.mark_variable_declaration(declaration);
                for declarator in &declaration.declarations {
                    if let Some(init) = &declarator.init {
                        self.walk_expression(init);
                    }
                }
            }
            _ => {
                if let Some(target) = left.as_assignment_target() {
                    self.mark_assignment_target_as_local(target);
                }
            }
        }
    }

    fn walk_expression(&mut self, expression: &Expression<'_>) {
        match expression {
            Expression::Identifier(identifier) => self.prefix_identifier(
                identifier.name.as_str(),
                identifier.span.start as usize,
                PrefixParent::Reference,
            ),
            Expression::StaticMemberExpression(member) => {
                self.walk_expression(&member.object);
            }
            Expression::ComputedMemberExpression(member) => {
                self.walk_expression(&member.object);
                self.walk_expression(&member.expression);
            }
            Expression::PrivateFieldExpression(member) => {
                self.walk_expression(&member.object);
            }
            Expression::CallExpression(call) => {
                self.walk_expression(&call.callee);
                for argument in &call.arguments {
                    self.walk_argument(argument);
                }
            }
            Expression::ArrayExpression(array) => {
                for element in &array.elements {
                    match element {
                        oxc_ast::ast::ArrayExpressionElement::SpreadElement(spread) => {
                            self.walk_expression(&spread.argument)
                        }
                        oxc_ast::ast::ArrayExpressionElement::Elision(_) => {}
                        element => {
                            if let Some(expression) = element.as_expression() {
                                self.walk_expression(expression);
                            }
                        }
                    }
                }
            }
            Expression::ObjectExpression(object) => {
                for property in &object.properties {
                    self.walk_object_property_kind(property);
                }
            }
            Expression::FunctionExpression(function) => self.walk_function(function),
            Expression::ArrowFunctionExpression(function) => self.walk_arrow_function(function),
            Expression::AssignmentExpression(assignment) => {
                self.walk_assignment_target(&assignment.left);
                self.walk_expression(&assignment.right);
            }
            Expression::UpdateExpression(update) => {
                self.walk_simple_assignment_target(&update.argument);
            }
            Expression::UnaryExpression(unary) => self.walk_expression(&unary.argument),
            Expression::BinaryExpression(binary) => {
                self.walk_expression(&binary.left);
                self.walk_expression(&binary.right);
            }
            Expression::LogicalExpression(logical) => {
                self.walk_expression(&logical.left);
                self.walk_expression(&logical.right);
            }
            Expression::ConditionalExpression(conditional) => {
                self.walk_expression(&conditional.test);
                self.walk_expression(&conditional.consequent);
                self.walk_expression(&conditional.alternate);
            }
            Expression::SequenceExpression(sequence) => {
                for expression in &sequence.expressions {
                    self.walk_expression(expression);
                }
            }
            Expression::ParenthesizedExpression(parenthesized) => {
                self.walk_expression(&parenthesized.expression);
            }
            Expression::TSAsExpression(expression) => self.walk_expression(&expression.expression),
            Expression::TSSatisfiesExpression(expression) => {
                self.walk_expression(&expression.expression)
            }
            Expression::TSNonNullExpression(expression) => {
                self.walk_expression(&expression.expression)
            }
            Expression::ChainExpression(chain) => match &chain.expression {
                oxc_ast::ast::ChainElement::CallExpression(call) => {
                    self.walk_expression(&call.callee);
                    for argument in &call.arguments {
                        self.walk_argument(argument);
                    }
                }
                oxc_ast::ast::ChainElement::StaticMemberExpression(member) => {
                    self.walk_expression(&member.object)
                }
                oxc_ast::ast::ChainElement::ComputedMemberExpression(member) => {
                    self.walk_expression(&member.object);
                    self.walk_expression(&member.expression);
                }
                oxc_ast::ast::ChainElement::PrivateFieldExpression(member) => {
                    self.walk_expression(&member.object)
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn walk_argument(&mut self, argument: &Argument<'_>) {
        match argument {
            Argument::SpreadElement(spread) => self.walk_expression(&spread.argument),
            _ => self.walk_expression(argument.to_expression()),
        }
    }

    fn walk_object_property_kind(&mut self, property: &ObjectPropertyKind<'_>) {
        if let ObjectPropertyKind::ObjectProperty(property) = property {
            self.walk_object_property(property);
        }
    }

    fn walk_object_property(&mut self, property: &ObjectProperty<'_>) {
        if property.computed {
            self.walk_property_key(&property.key);
        }
        if property.shorthand {
            if let Expression::Identifier(identifier) = &property.value {
                self.prefix_identifier(
                    identifier.name.as_str(),
                    identifier.span.end as usize,
                    PrefixParent::ShorthandPropertyValue,
                );
                return;
            }
        }
        self.walk_expression(&property.value);
    }

    fn walk_property_key(&mut self, key: &PropertyKey<'_>) {
        match key {
            PropertyKey::StaticIdentifier(_) | PropertyKey::PrivateIdentifier(_) => {}
            _ => self.walk_expression(key.to_expression()),
        }
    }

    fn walk_function(&mut self, function: &Function<'_>) {
        self.push_scope();
        if let Some(id) = &function.id {
            self.mark_local(id.name.as_str());
        }
        for param in &function.params.items {
            self.mark_binding_pattern(&param.pattern);
            if let Some(initializer) = &param.initializer {
                self.walk_expression(initializer);
            }
        }
        if let Some(rest) = &function.params.rest {
            self.mark_binding_pattern(&rest.rest.argument);
        }
        if let Some(body) = &function.body {
            self.mark_block_declarations(&body.statements);
            for statement in &body.statements {
                self.walk_statement(statement);
            }
        }
        self.pop_scope();
    }

    fn walk_arrow_function(&mut self, function: &ArrowFunctionExpression<'_>) {
        self.push_scope();
        for param in &function.params.items {
            self.mark_binding_pattern(&param.pattern);
            if let Some(initializer) = &param.initializer {
                self.walk_expression(initializer);
            }
        }
        if let Some(rest) = &function.params.rest {
            self.mark_binding_pattern(&rest.rest.argument);
        }
        self.mark_block_declarations(&function.body.statements);
        for statement in &function.body.statements {
            self.walk_statement(statement);
        }
        self.pop_scope();
    }

    fn walk_assignment_target(&mut self, target: &AssignmentTarget<'_>) {
        match target {
            AssignmentTarget::AssignmentTargetIdentifier(identifier) => self.prefix_identifier(
                identifier.name.as_str(),
                identifier.span.start as usize,
                PrefixParent::Reference,
            ),
            AssignmentTarget::StaticMemberExpression(member) => {
                self.walk_expression(&member.object)
            }
            AssignmentTarget::ComputedMemberExpression(member) => {
                self.walk_expression(&member.object);
                self.walk_expression(&member.expression);
            }
            AssignmentTarget::PrivateFieldExpression(member) => {
                self.walk_expression(&member.object)
            }
            _ => {}
        }
    }

    fn walk_simple_assignment_target(&mut self, target: &SimpleAssignmentTarget<'_>) {
        match target {
            SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) => self
                .prefix_identifier(
                    identifier.name.as_str(),
                    identifier.span.start as usize,
                    PrefixParent::Reference,
                ),
            SimpleAssignmentTarget::StaticMemberExpression(member) => {
                self.walk_expression(&member.object)
            }
            SimpleAssignmentTarget::ComputedMemberExpression(member) => {
                self.walk_expression(&member.object);
                self.walk_expression(&member.expression);
            }
            SimpleAssignmentTarget::PrivateFieldExpression(member) => {
                self.walk_expression(&member.object)
            }
            _ => {}
        }
    }

    fn mark_assignment_target_as_local(&mut self, target: &AssignmentTarget<'_>) {
        if let AssignmentTarget::AssignmentTargetIdentifier(identifier) = target {
            self.mark_local(identifier.name.as_str());
        }
    }

    fn mark_block_declarations(&mut self, statements: &[Statement<'_>]) {
        for statement in statements {
            match statement {
                Statement::VariableDeclaration(declaration) => {
                    self.mark_variable_declaration(declaration);
                }
                Statement::FunctionDeclaration(function) => {
                    if let Some(id) = &function.id {
                        self.mark_local(id.name.as_str());
                    }
                }
                Statement::ClassDeclaration(class) => {
                    if let Some(id) = &class.id {
                        self.mark_local(id.name.as_str());
                    }
                }
                _ => {}
            }
        }
    }

    fn mark_variable_declaration(&mut self, declaration: &VariableDeclaration<'_>) {
        for declarator in &declaration.declarations {
            self.mark_binding_pattern(&declarator.id);
        }
    }

    fn mark_binding_pattern(&mut self, pattern: &BindingPattern<'_>) {
        match pattern {
            BindingPattern::BindingIdentifier(identifier) => {
                self.mark_local(identifier.name.as_str())
            }
            BindingPattern::ObjectPattern(pattern) => {
                for property in &pattern.properties {
                    self.mark_binding_pattern(&property.value);
                }
                if let Some(rest) = &pattern.rest {
                    self.mark_binding_pattern(&rest.argument);
                }
            }
            BindingPattern::ArrayPattern(pattern) => {
                for element in pattern.elements.iter().flatten() {
                    self.mark_binding_pattern(element);
                }
                if let Some(rest) = &pattern.rest {
                    self.mark_binding_pattern(&rest.argument);
                }
            }
            BindingPattern::AssignmentPattern(pattern) => {
                self.mark_binding_pattern(&pattern.left);
                self.walk_expression(&pattern.right);
            }
        }
    }

    fn prefix_identifier(&mut self, name: &str, offset: usize, parent: PrefixParent) {
        if do_not_prefix_vue27(name) || self.is_local(name) {
            return;
        }
        let prefix = self.prefix_for(name);
        match parent {
            PrefixParent::Reference => self.edits.prepend_right(offset, prefix),
            PrefixParent::ShorthandPropertyValue => {
                self.edits.append_left(offset, format!(": {prefix}{name}"))
            }
        }
    }

    fn prefix_for(&self, name: &str) -> &'static str {
        if self.is_script_setup()
            && self
                .options
                .bindings
                .get(name)
                .is_some_and(|binding| binding.starts_with("setup"))
        {
            "_setup."
        } else {
            "_vm."
        }
    }

    fn is_script_setup(&self) -> bool {
        !matches!(
            self.options
                .bindings
                .get("__isScriptSetup")
                .map(String::as_str),
            Some("false")
        ) && !self.options.bindings.is_empty()
    }

    fn push_scope(&mut self) {
        self.locals.push(BTreeMap::new());
    }

    fn pop_scope(&mut self) {
        self.locals.pop();
    }

    fn mark_local(&mut self, name: &str) {
        if let Some(scope) = self.locals.last_mut() {
            *scope.entry(name.to_string()).or_insert(0) += 1;
        }
    }

    fn is_local(&self, name: &str) -> bool {
        self.locals
            .iter()
            .rev()
            .any(|scope| scope.get(name).is_some_and(|count| *count > 0))
    }
}

#[derive(Clone, Copy)]
enum PrefixParent {
    Reference,
    ShorthandPropertyValue,
}

fn do_not_prefix_vue27(name: &str) -> bool {
    matches!(
        name,
        "Infinity"
            | "undefined"
            | "NaN"
            | "isFinite"
            | "isNaN"
            | "parseFloat"
            | "parseInt"
            | "decodeURI"
            | "decodeURIComponent"
            | "encodeURI"
            | "encodeURIComponent"
            | "Math"
            | "Number"
            | "Date"
            | "Array"
            | "Object"
            | "Boolean"
            | "String"
            | "RegExp"
            | "Map"
            | "Set"
            | "JSON"
            | "Intl"
            | "require"
            | "arguments"
            | "_c"
    )
}

fn source_with_overwrite(input: &str, start: usize, end: usize, replacement: &str) -> String {
    let start = start.min(input.len());
    let end = end.min(input.len()).max(start);
    let mut output = String::new();
    output.push_str(&input[..start]);
    output.push_str(replacement);
    output.push_str(&input[end..]);
    output
}

fn has_vue27_default_export_like(input: &str) -> bool {
    let mut index = 0usize;
    while let Some(offset) = input[index..].find("export") {
        let export_start = index + offset;
        if is_vue27_export_boundary(input, export_start)
            && input[export_start..].contains("default")
        {
            return true;
        }
        index = export_start + "export".len();
    }
    false
}

fn is_vue27_export_boundary(input: &str, export_start: usize) -> bool {
    let prefix = &input[..export_start];
    let Some(non_space) = prefix
        .char_indices()
        .rev()
        .find(|(_, ch)| !matches!(ch, ' ' | '\t' | '\r'))
    else {
        return true;
    };
    matches!(non_space.1, '\n' | ';')
}

#[derive(Debug)]
struct SourceEdits<'a> {
    input: &'a str,
    edits: Vec<SourceEdit>,
    prepend: String,
    append: String,
}

#[derive(Debug)]
struct SourceEdit {
    start: usize,
    end: usize,
    replacement: String,
}

impl<'a> SourceEdits<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input,
            edits: Vec::new(),
            prepend: String::new(),
            append: String::new(),
        }
    }

    fn overwrite(&mut self, start: usize, end: usize, replacement: impl Into<String>) {
        self.edits.push(SourceEdit {
            start,
            end,
            replacement: replacement.into(),
        });
    }

    fn remove(&mut self, start: usize, end: usize) {
        self.overwrite(start, end, "");
    }

    fn prepend_right(&mut self, offset: usize, value: impl Into<String>) {
        self.overwrite(offset, offset, value);
    }

    fn append_left(&mut self, offset: usize, value: impl Into<String>) {
        self.overwrite(offset, offset, value);
    }

    fn prepend(&mut self, value: impl AsRef<str>) {
        self.prepend.push_str(value.as_ref());
    }

    fn append(&mut self, value: impl AsRef<str>) {
        self.append.push_str(value.as_ref());
    }

    fn apply(mut self) -> String {
        self.edits.sort_by_key(|edit| (edit.start, edit.end));
        let mut output = String::new();
        output.push_str(&self.prepend);
        let mut cursor = 0usize;
        for edit in self.edits {
            if edit.start < cursor {
                continue;
            }
            output.push_str(&self.input[cursor..edit.start.min(self.input.len())]);
            output.push_str(&edit.replacement);
            cursor = edit.end.min(self.input.len());
        }
        output.push_str(&self.input[cursor..]);
        output.push_str(&self.append);
        output
    }
}

fn style_src_dependency(style: &SfcBlock) -> Vec<String> {
    style.attrs.src.iter().cloned().collect()
}

fn style_import_dependencies(style: &SfcBlock) -> Vec<String> {
    let mut dependencies = Vec::new();
    for line in style.content.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("@import") {
            continue;
        }
        if let Some(dep) = quoted_import_path(trimmed) {
            dependencies.push(dep.to_string());
        }
    }
    dependencies
}

fn descriptor_css_vars(descriptor: &SfcDescriptor, options: CssVarCollectOptions) -> Vec<String> {
    let mut vars = Vec::new();
    for style in &descriptor.styles {
        for var in collect_css_vars_with_options(&style.content, options) {
            if !vars.iter().any(|existing| existing == &var) {
                vars.push(var);
            }
        }
    }
    vars
}

fn add_style_block_mappings(
    builder: &mut SourceMapBuilder,
    descriptor: &SfcDescriptor,
    style: &SfcBlock,
    generated_code: &str,
    generated_line_offset: u32,
) {
    if generated_code.is_empty() {
        return;
    }
    let original_line_starts = style_line_starts(&style.content);
    let generated_lines = generated_line_count(generated_code).max(1);
    for generated_line in 0..generated_lines {
        let local_start = original_line_starts
            .get(generated_line as usize)
            .copied()
            .unwrap_or_else(|| *original_line_starts.last().unwrap_or(&0));
        let absolute = style.content_start + local_start;
        builder.add_mapping(
            generated_line_offset as usize + generated_line as usize + 1,
            0,
            Some(Span::new(descriptor.source_file, absolute, absolute)),
            Some(descriptor.filename.clone()),
        );
    }
}

fn style_line_starts(source: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (index, ch) in source.char_indices() {
        if ch == '\n' {
            starts.push(index + ch.len_utf8());
        }
    }
    starts
}

fn generated_line_count(source: &str) -> u32 {
    source.lines().count().max(1) as u32
}

fn vue27_script_content(
    descriptor: &SfcDescriptor,
    options: &SfcScriptCompileOptions,
    css_vars: &[String],
) -> String {
    if let Some(script_setup) = descriptor.script_setup.as_ref() {
        return vue27_script_setup_content(descriptor, script_setup, options, css_vars);
    }
    let Some(script) = descriptor.script.as_ref() else {
        return String::new();
    };
    if css_vars.is_empty() {
        return script.content.clone();
    }
    let scope_id = vue27_scope_id(options.id.as_deref());
    let bindings = vue27_normal_script_binding_metadata(descriptor);
    let content = rewrite_vue27_default(
        &script.content,
        "__default__",
        Vue27RewriteDefaultOptions {
            typescript: script_is_typescript(&script.attrs),
            decorators: script_is_typescript(&script.attrs),
        },
    );
    format!(
        "{}{}\nexport default __default__",
        content,
        gen_vue27_normal_script_css_vars_code(css_vars, &bindings, &scope_id, options.is_prod)
    )
}

fn vue27_script_compile_errors(descriptor: &SfcDescriptor) -> Vec<String> {
    let Some(script_setup) = descriptor.script_setup.as_ref() else {
        return Vec::new();
    };
    if descriptor
        .script
        .as_ref()
        .is_some_and(|script| script.attrs.lang != script_setup.attrs.lang)
    {
        return vec!["<script> and <script setup> must have the same language type.".to_string()];
    }
    let setup_context = vue27_script_setup_context(descriptor);
    analyze_vue27_script_setup(script_setup, false, &setup_context).errors
}

fn vue27_script_setup_content(
    descriptor: &SfcDescriptor,
    script_setup: &SfcBlock,
    options: &SfcScriptCompileOptions,
    css_vars: &[String],
) -> String {
    let scope_id = vue27_scope_id(options.id.as_deref());
    let setup_context = vue27_script_setup_context(descriptor);
    let analysis = analyze_vue27_script_setup(script_setup, options.is_prod, &setup_context);
    let normal_script = analyze_vue27_normal_script_for_setup(descriptor);
    let bindings = vue27_setup_binding_metadata(descriptor);
    let is_ts = script_is_typescript(&script_setup.attrs);
    let css_vars_code = if css_vars.is_empty() {
        String::new()
    } else {
        format!(
            "\n{}\n",
            gen_vue27_css_vars_code(css_vars, &bindings, &scope_id, options.is_prod)
        )
    };
    let return_bindings = vue27_script_setup_return_bindings(descriptor, &analysis, is_ts);
    let returned = if return_bindings.is_empty() {
        if options.emit_script_setup_marker {
            "{ __sfc: true, }".to_string()
        } else {
            "{  }".to_string()
        }
    } else if options.emit_script_setup_marker {
        format!("{{ __sfc: true,{} }}", return_bindings.join(", "))
    } else {
        format!("{{ {} }}", return_bindings.join(", "))
    };
    let helper_import = if css_vars.is_empty() {
        ""
    } else {
        "import { useCssVars as _useCssVars } from 'vue'\n"
    };
    let helper_import = if analysis.needs_merge_defaults {
        format!("import {{ mergeDefaults as _mergeDefaults }} from 'vue'\n{helper_import}")
    } else {
        helper_import.to_string()
    };
    let runtime_options = vue27_script_setup_runtime_options(descriptor, &analysis, &normal_script);
    let setup_params = vue27_script_setup_params(&analysis, is_ts);
    let setup_prefix = format!(
        "{}{}{}",
        css_vars_code, analysis.setup_prelude, analysis.setup_content
    );
    let return_separator = vue27_return_separator(&setup_prefix);
    let setup_body = format!("{setup_prefix}{return_separator}return {returned}");
    let export_prefix = vue27_script_setup_export_prefix(
        &normal_script,
        &runtime_options,
        is_ts,
        &setup_params,
        &setup_body,
    );
    let helper_import = if is_ts {
        if analysis.needs_merge_defaults {
            helper_import.replace(
                "import { mergeDefaults as _mergeDefaults } from 'vue'\n",
                "import { mergeDefaults as _mergeDefaults, defineComponent as _defineComponent } from 'vue'\n",
            )
        } else {
            "import { defineComponent as _defineComponent } from 'vue'\n".to_string()
                + &helper_import
        }
    } else {
        helper_import
    };
    let normal_script_after_setup = descriptor
        .script
        .as_ref()
        .is_some_and(|script| script.content_start > script_setup.content_start);
    let mut content = helper_import;
    let mut first_module_chunk = true;
    if normal_script_after_setup {
        append_vue27_module_chunk(
            &mut content,
            &normal_script.module_content,
            first_module_chunk,
            false,
        );
        first_module_chunk = first_module_chunk && normal_script.module_content.is_empty();
        append_vue27_module_chunk(
            &mut content,
            &analysis.module_content,
            first_module_chunk,
            normal_script.has_default_export,
        );
    } else {
        append_vue27_module_chunk(
            &mut content,
            &analysis.module_content,
            first_module_chunk,
            false,
        );
        first_module_chunk = first_module_chunk && analysis.module_content.is_empty();
        append_vue27_module_chunk(
            &mut content,
            &normal_script.module_content,
            first_module_chunk,
            false,
        );
    }
    content.push_str(&export_prefix);
    content.trim().to_string()
}

fn append_vue27_module_chunk(
    output: &mut String,
    chunk: &str,
    first_module_chunk: bool,
    blank_between_plain_chunks: bool,
) {
    if chunk.is_empty() {
        return;
    }
    let chunk = if output.is_empty() {
        chunk
    } else {
        let mut chunk = chunk;
        let pending_blank = output_has_pending_blank_line(output);
        if first_module_chunk && output.ends_with('\n') && chunk.starts_with('\n') {
            chunk = &chunk[1..];
        } else if pending_blank && chunk.starts_with('\n') {
            chunk = &chunk[1..];
        }
        if !output.ends_with('\n') && !chunk.starts_with('\n') {
            output.push('\n');
            if !first_module_chunk && blank_between_plain_chunks && !pending_blank {
                output.push('\n');
            }
        } else if output.ends_with('\n')
            && !chunk.starts_with('\n')
            && !first_module_chunk
            && blank_between_plain_chunks
            && !pending_blank
        {
            output.push('\n');
        } else if !output.ends_with('\n')
            && chunk.starts_with('\n')
            && !first_module_chunk
            && blank_between_plain_chunks
            && !pending_blank
        {
            if !chunk.starts_with("\n\n") {
                output.push_str("\n\n");
            }
        }
        chunk
    };
    if first_module_chunk && output_has_pending_blank_line(output) {
        output.push_str(chunk.strip_prefix('\n').unwrap_or(chunk));
    } else {
        output.push_str(chunk);
    }
}

fn output_has_pending_blank_line(output: &str) -> bool {
    if output.is_empty() {
        return false;
    }
    let current = if output.ends_with('\n') {
        let without_final_newline = &output[..output.len() - 1];
        let line_start = without_final_newline
            .rfind('\n')
            .map_or(0, |index| index + 1);
        &without_final_newline[line_start..]
    } else {
        let line_start = output.rfind('\n').map_or(0, |index| index + 1);
        &output[line_start..]
    };
    current.trim().is_empty()
}

fn vue27_script_setup_runtime_options(
    descriptor: &SfcDescriptor,
    analysis: &Vue27ScriptSetupAnalysis,
    normal_script: &Vue27NormalScriptAnalysis,
) -> String {
    let mut runtime_options = String::new();
    if !normal_script.has_default_export_name {
        if let Some(name) = vue27_inferred_component_name(&descriptor.filename) {
            runtime_options.push_str(&format!("\n  __name: '{}',", escape_js_single(&name)));
        }
    }
    if let Some(props) = analysis.props_runtime.as_ref() {
        runtime_options.push_str(&format!("\n  props: {},", props.trim()));
    }
    if let Some(emits) = analysis.emits_runtime.as_ref() {
        runtime_options.push_str(&format!("\n  emits: {},", emits.trim()));
    }
    runtime_options
}

fn vue27_script_setup_return_bindings(
    descriptor: &SfcDescriptor,
    analysis: &Vue27ScriptSetupAnalysis,
    is_ts: bool,
) -> Vec<String> {
    let script_returns = vue27_script_setup_script_return_bindings(descriptor);
    let mut bindings = script_returns.bindings;
    for value in &analysis.return_bindings {
        push_unique(&mut bindings, value);
    }
    for import in &script_returns.imports {
        if import.is_type {
            continue;
        }
        if vue27_script_setup_import_is_returned(descriptor, import, is_ts) {
            push_unique(&mut bindings, &import.local);
        }
    }
    for import in &analysis.imports {
        if import.is_type {
            continue;
        }
        if vue27_script_setup_import_is_returned(descriptor, import, is_ts) {
            push_unique(&mut bindings, &import.local);
        }
    }
    bindings
        .into_iter()
        .filter(|name| {
            !analysis
                .removed_bindings
                .iter()
                .any(|removed| removed == name)
        })
        .collect()
}

fn vue27_script_setup_import_is_returned(
    descriptor: &SfcDescriptor,
    import: &Vue27ScriptImport,
    is_ts: bool,
) -> bool {
    let Some(template) = descriptor.template.as_ref() else {
        return true;
    };
    if template.attrs.src.is_some() || template.attrs.lang.is_some() {
        return true;
    }
    vue27_template_uses_identifier(&template.content, &import.local, is_ts)
}

fn vue27_script_setup_params(analysis: &Vue27ScriptSetupAnalysis, is_ts: bool) -> String {
    let props_param = if is_ts && analysis.props_type_runtime {
        "__props: any"
    } else {
        "__props"
    };
    let mut context_parts = Vec::new();
    if let Some(binding) = analysis.emit_binding.as_deref() {
        if binding == "emit" {
            context_parts.push("emit".to_string());
        } else {
            context_parts.push(format!("emit: {binding}"));
        }
    }
    if analysis.needs_expose {
        context_parts.push("expose".to_string());
    }
    if context_parts.is_empty() {
        props_param.to_string()
    } else if is_ts {
        if let Some(emit_type_source) = analysis.emit_type_source.as_deref() {
            format!(
                "{props_param}, {{ {} }}: {{ emit: ({emit_type_source}), expose: any, slots: any, attrs: any }}",
                context_parts.join(", ")
            )
        } else {
            format!("{props_param}, {{ {} }}", context_parts.join(", "))
        }
    } else {
        format!("{props_param}, {{ {} }}", context_parts.join(", "))
    }
}

fn vue27_return_separator(setup_prefix: &str) -> &'static str {
    if setup_prefix.is_empty() {
        return "\n\n\n\n";
    }
    if setup_prefix.chars().all(|ch| matches!(ch, '\n' | '\r')) {
        let newlines = setup_prefix.chars().filter(|ch| *ch == '\n').count();
        return if newlines <= 1 { "\n\n" } else { "\n" };
    }
    if !setup_prefix.ends_with('\n') {
        return "\n";
    }
    let without_trailing_newlines = setup_prefix.trim_end_matches(['\n', '\r']);
    let Some(last_line) = without_trailing_newlines.rsplit('\n').next() else {
        return "";
    };
    if last_line.trim().is_empty() {
        ""
    } else {
        "\n"
    }
}

fn vue27_script_setup_export_prefix(
    normal_script: &Vue27NormalScriptAnalysis,
    runtime_options: &str,
    is_ts: bool,
    setup_params: &str,
    setup_body: &str,
) -> String {
    if is_ts {
        let spread = if normal_script.has_default_export {
            "\n  ...__default__,"
        } else {
            ""
        };
        return format!(
            "\nexport default /*#__PURE__*/_defineComponent({{{spread}{runtime_options}\n  setup({setup_params}) {{\n{setup_body}\n}}\n\n}})"
        );
    }
    if normal_script.has_default_export {
        format!(
            "\nexport default /*#__PURE__*/Object.assign(__default__, {{{runtime_options}\n  setup({setup_params}) {{\n{setup_body}\n}}\n\n}})"
        )
    } else {
        format!("\nexport default {{{runtime_options}\n  setup({setup_params}) {{\n{setup_body}\n}}\n\n}}")
    }
}

fn vue27_inferred_component_name(filename: &str) -> Option<String> {
    if filename.is_empty() || filename == "anonymous.vue" {
        return None;
    }
    let name = filename
        .rsplit(['/', '\\'])
        .next()
        .and_then(|file| file.rsplit_once('.').map(|(stem, _)| stem))
        .filter(|stem| !stem.is_empty())?;
    Some(name.to_string())
}

fn escape_js_single(value: &str) -> String {
    value.replace('\\', "\\\\").replace('\'', "\\'")
}

fn analyze_vue27_normal_script_for_setup(descriptor: &SfcDescriptor) -> Vue27NormalScriptAnalysis {
    let Some(script) = descriptor.script.as_ref() else {
        return Vue27NormalScriptAnalysis::default();
    };
    let source = script.content.as_str();
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        source,
        script_source_type_from_attrs(&script.attrs),
    )
    .with_options(oxc_parser::ParseOptions {
        parse_regular_expression: true,
        ..oxc_parser::ParseOptions::default()
    })
    .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return Vue27NormalScriptAnalysis {
            module_content: source.to_string(),
            ..Vue27NormalScriptAnalysis::default()
        };
    }

    let mut edits = SourceEdits::new(source);
    let mut named_default_exports = Vec::new();
    let mut analysis = Vue27NormalScriptAnalysis::default();
    for statement in &parsed.program.body {
        match statement {
            Statement::ExportDefaultDeclaration(declaration) => {
                analysis.has_default_export = true;
                analysis.has_default_export_name = default_export_has_name(declaration);
                edits.overwrite(
                    declaration.span.start as usize,
                    declaration.declaration.span().start as usize,
                    "const __default__ = ",
                );
            }
            Statement::ExportNamedDeclaration(declaration) => {
                if rewrite_named_default_exports(source, "__default__", declaration, &mut edits) {
                    analysis.has_default_export = true;
                    if export_named_declaration_only_exports_default(declaration) {
                        named_default_exports.push((
                            declaration.span.start as usize,
                            declaration.span.end as usize,
                        ));
                    }
                }
            }
            _ => {}
        }
    }
    for (start, end) in named_default_exports {
        edits.remove(start, end);
    }
    analysis.module_content = trim_trailing_blank_lines(&edits.apply()).to_string();
    if analysis.module_content.starts_with('\n') {
        analysis.module_content.insert(0, '\n');
    }
    analysis
}

fn default_export_has_name(declaration: &ExportDefaultDeclaration<'_>) -> bool {
    match &declaration.declaration {
        ExportDefaultDeclarationKind::ObjectExpression(object) => {
            object_expression_has_static_key(object, "name")
        }
        ExportDefaultDeclarationKind::CallExpression(call) => {
            call.arguments.first().is_some_and(|argument| {
                matches!(argument.to_expression(), Expression::ObjectExpression(object) if object_expression_has_static_key(object, "name"))
            })
        }
        _ => false,
    }
}

fn object_expression_has_static_key(object: &ObjectExpression<'_>, key: &str) -> bool {
    object
        .properties
        .iter()
        .filter_map(|property| property.as_property())
        .filter(|property| !property.computed)
        .any(|property| property.key.static_name().as_deref() == Some(key))
}

fn vue27_scope_id(id: Option<&str>) -> String {
    id.and_then(|id| id.strip_prefix("data-v-").or(Some(id)))
        .unwrap_or("")
        .to_string()
}

fn gen_vue27_normal_script_css_vars_code(
    css_vars: &[String],
    bindings: &BTreeMap<String, String>,
    id: &str,
    is_prod: bool,
) -> String {
    format!(
        "\nimport {{ useCssVars as _useCssVars }} from 'vue'\nconst __injectCSSVars__ = () => {{\n{}}}\nconst __setup__ = __default__.setup\n__default__.setup = __setup__\n  ? (props, ctx) => {{ __injectCSSVars__();return __setup__(props, ctx) }}\n  : __injectCSSVars__\n",
        gen_vue27_css_vars_code(css_vars, bindings, id, is_prod)
    )
}

fn gen_vue27_css_vars_code(
    css_vars: &[String],
    bindings: &BTreeMap<String, String>,
    id: &str,
    is_prod: bool,
) -> String {
    let vars = css_vars
        .iter()
        .map(|var| {
            format!(
                "\"{}\": ({})",
                gen_css_var_name_with_style(id, var, is_prod, CssVarNameStyle::Vue27Legacy),
                var
            )
        })
        .collect::<Vec<_>>()
        .join(",\n  ");
    let expression = format!("({{\n  {vars}\n}})");
    let prefixed = prefix_vue27_identifiers(
        &expression,
        Vue27PrefixIdentifiersOptions {
            is_functional: false,
            is_ts: false,
            bindings: bindings.clone(),
        },
    );
    format!("_useCssVars((_vm, _setup) => {prefixed})")
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Vue27ScriptSetupAnalysis {
    module_content: String,
    hoisted_module_content: String,
    module_chunks: Vec<Vue27ModuleChunk>,
    setup_content: String,
    setup_prelude: String,
    return_bindings: Vec<String>,
    imports: Vec<Vue27ScriptImport>,
    removed_bindings: Vec<String>,
    normal_imports: Vec<Vue27ScriptImport>,
    local_setup_bindings: BTreeSet<String>,
    setup_bindings: BTreeMap<String, String>,
    props_bindings: Vec<String>,
    props_runtime: Option<String>,
    props_type_runtime: bool,
    errors: Vec<String>,
    props_type_source: Option<String>,
    props_runtime_defaults: Option<Vue27RuntimeDefaults>,
    emits_runtime: Option<String>,
    emit_binding: Option<String>,
    emit_type_source: Option<String>,
    needs_expose: bool,
    user_import_aliases: BTreeMap<String, String>,
    declared_types: BTreeMap<String, Vec<String>>,
    props_type_declarations: BTreeMap<String, Vue27TypeMembers>,
    emits_type_declarations: BTreeMap<String, Vue27EmitsType>,
    needs_merge_defaults: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Vue27RuntimeProp {
    key: String,
    types: Vec<String>,
    required: bool,
    default: Option<String>,
    is_method: bool,
    type_annotation_source: Option<String>,
    member_source: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Vue27RuntimeDefaults {
    source: String,
    static_defaults: Option<BTreeMap<String, String>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Vue27TypeMembers {
    source: String,
    members: Vec<Vue27RuntimeProp>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Vue27EmitsType {
    source: String,
    events: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Vue27TypeContext {
    declared_types: BTreeMap<String, Vec<String>>,
    define_model_declared_types: BTreeMap<String, Vec<String>>,
    props_type_declarations: BTreeMap<String, Vue27TypeMembers>,
    emits_type_declarations: BTreeMap<String, Vue27EmitsType>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Vue27ScriptSetupContext {
    normal_types: Vue27TypeContext,
    normal_imports: Vec<Vue27ScriptImport>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Vue27ScriptImport {
    local: String,
    source: String,
    imported: String,
    is_type: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Vue27ModuleChunk {
    start: usize,
    content: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Vue27ScriptReturnBindings {
    bindings: Vec<String>,
    imports: Vec<Vue27ScriptImport>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Vue27NormalScriptAnalysis {
    module_content: String,
    has_default_export: bool,
    has_default_export_name: bool,
}

fn analyze_vue27_script_setup(
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
    let mut analysis = Vue27ScriptSetupAnalysis::default();
    analysis.normal_imports = setup_context.normal_imports.clone();
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

fn collect_vue27_declared_types_from_statements(
    source: &str,
    statements: &[Statement<'_>],
    analysis: &mut Vue27ScriptSetupAnalysis,
) {
    for statement in statements {
        collect_vue27_declared_type_from_statement(source, statement, analysis);
    }
}

fn collect_vue27_setup_local_bindings(
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

fn collect_vue27_declared_type_from_statement(
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

fn collect_vue27_declared_type_from_declaration(
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

fn vue27_script_setup_context(descriptor: &SfcDescriptor) -> Vue27ScriptSetupContext {
    Vue27ScriptSetupContext {
        normal_types: vue27_normal_script_type_context(descriptor),
        normal_imports: vue27_script_setup_script_return_bindings(descriptor).imports,
    }
}

fn vue27_normal_script_type_context(descriptor: &SfcDescriptor) -> Vue27TypeContext {
    let Some(script) = descriptor.script.as_ref() else {
        return Vue27TypeContext::default();
    };
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        script.content.as_str(),
        script_source_type_from_attrs(&script.attrs),
    )
    .with_options(oxc_parser::ParseOptions {
        parse_regular_expression: true,
        ..oxc_parser::ParseOptions::default()
    })
    .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return Vue27TypeContext::default();
    }
    let mut analysis = Vue27ScriptSetupAnalysis::default();
    collect_vue27_declared_types_from_statements(
        script.content.as_str(),
        &parsed.program.body,
        &mut analysis,
    );
    Vue27TypeContext {
        declared_types: analysis.declared_types,
        define_model_declared_types: BTreeMap::new(),
        props_type_declarations: analysis.props_type_declarations,
        emits_type_declarations: analysis.emits_type_declarations,
    }
}

fn vue3_normal_script_type_context(descriptor: &SfcDescriptor) -> Vue27TypeContext {
    let Some(script) = descriptor.script.as_ref() else {
        return Vue27TypeContext::default();
    };
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        script.content.as_str(),
        script_source_type_from_attrs(&script.attrs),
    )
    .with_options(oxc_parser::ParseOptions {
        parse_regular_expression: true,
        ..oxc_parser::ParseOptions::default()
    })
    .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return Vue27TypeContext::default();
    }
    let mut analysis = Vue3ScriptSetupAnalysis::default();
    collect_vue3_declared_types_from_statements(
        script.content.as_str(),
        &parsed.program.body,
        &mut analysis,
    );
    Vue27TypeContext {
        declared_types: analysis.declared_types,
        define_model_declared_types: analysis.define_model_declared_types,
        props_type_declarations: analysis.props_type_declarations,
        emits_type_declarations: analysis.emits_type_declarations,
    }
}

fn vue3_normal_script_vue_import_aliases(descriptor: &SfcDescriptor) -> BTreeMap<String, String> {
    let Some(script) = descriptor.script.as_ref() else {
        return BTreeMap::new();
    };
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        script.content.as_str(),
        script_source_type_from_attrs(&script.attrs),
    )
    .with_options(oxc_parser::ParseOptions {
        parse_regular_expression: true,
        ..oxc_parser::ParseOptions::default()
    })
    .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return BTreeMap::new();
    }
    let mut aliases = BTreeMap::new();
    for statement in &parsed.program.body {
        let Statement::ImportDeclaration(import) = statement else {
            continue;
        };
        if import.source.value.as_str() != "vue" {
            continue;
        }
        if let Some(specifiers) = &import.specifiers {
            for specifier in specifiers {
                if let Some(imported) = import_specifier_imported(specifier) {
                    aliases.insert(imported, import_specifier_local(specifier));
                }
            }
        }
    }
    aliases
}

fn collect_vue3_declared_types_from_statements(
    source: &str,
    statements: &[Statement<'_>],
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    for statement in statements {
        collect_vue3_declared_type_from_statement(source, statement, analysis);
    }
}

fn collect_vue3_declared_type_from_statement(
    source: &str,
    statement: &Statement<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    match statement {
        Statement::TSInterfaceDeclaration(declaration) => {
            analysis
                .declared_types
                .insert(declaration.id.name.to_string(), vec!["Object".into()]);
            analysis
                .define_model_declared_types
                .insert(declaration.id.name.to_string(), vec!["Object".into()]);
            let props = vue3_type_members_from_interface_body(source, &declaration.body, analysis);
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
            let runtime = infer_vue3_runtime_type(&declaration.type_annotation, analysis);
            let model_runtime =
                infer_vue3_define_model_runtime_type(&declaration.type_annotation, analysis);
            analysis
                .declared_types
                .insert(declaration.id.name.to_string(), runtime);
            analysis
                .define_model_declared_types
                .insert(declaration.id.name.to_string(), model_runtime);
            match &declaration.type_annotation {
                TSType::TSTypeLiteral(literal) => {
                    let props = vue3_type_members_from_literal(source, literal, analysis);
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
                collect_vue3_declared_type_from_declaration(source, declaration, analysis);
            }
        }
        _ => {}
    }
}

fn collect_vue3_declared_type_from_declaration(
    source: &str,
    declaration: &Declaration<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    match declaration {
        Declaration::TSInterfaceDeclaration(declaration) => {
            analysis
                .declared_types
                .insert(declaration.id.name.to_string(), vec!["Object".into()]);
            analysis
                .define_model_declared_types
                .insert(declaration.id.name.to_string(), vec!["Object".into()]);
            let props = vue3_type_members_from_interface_body(source, &declaration.body, analysis);
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
            let runtime = infer_vue3_runtime_type(&declaration.type_annotation, analysis);
            let model_runtime =
                infer_vue3_define_model_runtime_type(&declaration.type_annotation, analysis);
            analysis
                .declared_types
                .insert(declaration.id.name.to_string(), runtime);
            analysis
                .define_model_declared_types
                .insert(declaration.id.name.to_string(), model_runtime);
            match &declaration.type_annotation {
                TSType::TSTypeLiteral(literal) => {
                    let props = vue3_type_members_from_literal(source, literal, analysis);
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

fn collect_vue3_setup_local_bindings(
    statements: &[Statement<'_>],
    is_ts: bool,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    for statement in statements {
        match statement {
            Statement::VariableDeclaration(declaration) if !declaration.declare => {
                for declarator in &declaration.declarations {
                    insert_pattern_bindings(&declarator.id, &mut analysis.local_setup_bindings);
                    let binding_type =
                        vue3_setup_binding_type(declaration.kind, declarator.init.as_ref());
                    collect_pattern_binding_types(
                        &declarator.id,
                        binding_type,
                        &mut analysis.local_setup_binding_types,
                    );
                }
            }
            Statement::FunctionDeclaration(function) if !function.declare => {
                if let Some(id) = &function.id {
                    analysis.local_setup_bindings.insert(id.name.to_string());
                    analysis
                        .local_setup_binding_types
                        .insert(id.name.to_string(), "setup-const".into());
                }
            }
            Statement::ClassDeclaration(class) if !class.declare => {
                if let Some(id) = &class.id {
                    analysis.local_setup_bindings.insert(id.name.to_string());
                    analysis
                        .local_setup_binding_types
                        .insert(id.name.to_string(), "setup-const".into());
                }
            }
            Statement::TSEnumDeclaration(declaration) if is_ts && !declaration.declare => {
                analysis
                    .local_setup_bindings
                    .insert(declaration.id.name.to_string());
                analysis.local_setup_binding_types.insert(
                    declaration.id.name.to_string(),
                    vue3_ts_enum_binding_type(declaration).into(),
                );
            }
            _ => {}
        }
    }
}

fn analyze_vue27_setup_variable_declaration(
    source: &str,
    declaration: &VariableDeclaration<'_>,
    edits: &mut SourceEdits<'_>,
    analysis: &mut Vue27ScriptSetupAnalysis,
    is_prod: bool,
) {
    let mut macro_declarators = Vec::new();
    for (index, declarator) in declaration.declarations.iter().enumerate() {
        if let Some(init) = &declarator.init {
            if let Expression::CallExpression(call) = init {
                if is_call_named(call, "defineProps") {
                    collect_define_props_call(source, call, None, analysis, is_prod);
                    collect_pattern_bindings(&declarator.id, &mut analysis.return_bindings);
                    analysis.setup_bindings.insert(
                        first_pattern_binding(&declarator.id).unwrap_or_else(|| "props".into()),
                        "setup-reactive-const".into(),
                    );
                    analysis
                        .setup_prelude
                        .push_str(&vue27_props_alias_declaration(source, &declarator.id));
                    macro_declarators.push(index);
                    continue;
                }
                if is_call_named(call, "withDefaults")
                    && collect_with_defaults_call(
                        source,
                        call,
                        Some(&declarator.id),
                        analysis,
                        is_prod,
                    )
                {
                    collect_pattern_bindings(&declarator.id, &mut analysis.return_bindings);
                    analysis.setup_bindings.insert(
                        first_pattern_binding(&declarator.id).unwrap_or_else(|| "props".into()),
                        "setup-const".into(),
                    );
                    macro_declarators.push(index);
                    continue;
                }
                if is_call_named(call, "defineEmits") {
                    let emit_binding =
                        first_pattern_binding(&declarator.id).unwrap_or_else(|| "emit".into());
                    collect_define_emits_call(source, call, Some(&emit_binding), analysis);
                    analysis
                        .setup_bindings
                        .insert(emit_binding.clone(), "setup-const".into());
                    collect_pattern_bindings(&declarator.id, &mut analysis.return_bindings);
                    macro_declarators.push(index);
                    continue;
                }
            }
        }
        let binding_type =
            vue27_setup_binding_type(declaration.kind, declarator.init.as_ref(), analysis);
        collect_pattern_binding_types(&declarator.id, binding_type, &mut analysis.setup_bindings);
        collect_pattern_bindings(&declarator.id, &mut analysis.return_bindings);
    }
    remove_vue27_macro_declarators(declaration, &macro_declarators, edits);
}

fn hoist_vue27_setup_statement(
    source: &str,
    statement: &Statement<'_>,
    edits: &mut SourceEdits<'_>,
    analysis: &mut Vue27ScriptSetupAnalysis,
) {
    let (start, end) = vue27_statement_span_with_trailing_ws(source, statement);
    let source_text = source.get(start..end).unwrap_or_default();
    analysis.module_chunks.push(Vue27ModuleChunk {
        start,
        content: source_text.to_string(),
    });
    edits.remove(start, end);
}

fn vue27_statement_is_type_hoist(statement: &Statement<'_>) -> bool {
    match statement {
        Statement::TSTypeAliasDeclaration(_)
        | Statement::TSInterfaceDeclaration(_)
        | Statement::TSModuleDeclaration(_)
        | Statement::TSGlobalDeclaration(_)
        | Statement::TSImportEqualsDeclaration(_) => true,
        Statement::VariableDeclaration(declaration) => declaration.declare,
        Statement::FunctionDeclaration(function) => function.declare,
        Statement::ClassDeclaration(class) => class.declare,
        Statement::ExportNamedDeclaration(declaration) => {
            declaration.export_kind == ImportOrExportKind::Type
        }
        _ => false,
    }
}

fn vue27_statement_span_with_trailing_ws(
    source: &str,
    statement: &Statement<'_>,
) -> (usize, usize) {
    let start = statement.span().start as usize;
    let mut end = statement.span().end as usize;
    while source
        .get(end..)
        .and_then(|tail| tail.chars().next())
        .is_some_and(char::is_whitespace)
    {
        end += source[end..].chars().next().map_or(0, char::len_utf8);
    }
    (start, end)
}

fn vue27_statement_span_with_trailing_comments(
    source: &str,
    mut end: usize,
    comments: &[oxc_ast::ast::Comment],
) -> usize {
    let Some(comment) = comments
        .iter()
        .find(|comment| comment.is_trailing() && comment.span.start as usize >= end)
    else {
        return end;
    };
    if source
        .get(end..comment.span.start as usize)
        .is_none_or(|between| between.contains('\n'))
    {
        return end;
    }
    end = comment.span.end as usize;
    while source
        .get(end..)
        .and_then(|tail| tail.chars().next())
        .is_some_and(char::is_whitespace)
    {
        end += source[end..].chars().next().map_or(0, char::len_utf8);
    }
    end
}

fn vue27_setup_binding_type(
    kind: VariableDeclarationKind,
    init: Option<&Expression<'_>>,
    analysis: &Vue27ScriptSetupAnalysis,
) -> &'static str {
    if kind != VariableDeclarationKind::Const {
        return "setup-let";
    }
    if init.is_some_and(|init| {
        is_literal_expression(init) || is_call_expression_named(init, "defineProps")
    }) {
        return "setup-const";
    }
    if init.is_some_and(|init| is_vue27_ref_call(init, analysis)) {
        return "setup-ref";
    }
    "setup-maybe-ref"
}

fn is_vue27_ref_call(expression: &Expression<'_>, analysis: &Vue27ScriptSetupAnalysis) -> bool {
    let ref_name = analysis
        .user_import_aliases
        .get("ref")
        .map(String::as_str)
        .unwrap_or("ref");
    is_call_expression_named(expression, ref_name)
}

fn is_literal_expression(expression: &Expression<'_>) -> bool {
    matches!(
        expression,
        Expression::StringLiteral(_)
            | Expression::NumericLiteral(_)
            | Expression::BooleanLiteral(_)
            | Expression::NullLiteral(_)
            | Expression::RegExpLiteral(_)
            | Expression::BigIntLiteral(_)
    )
}

fn is_call_expression_named(expression: &Expression<'_>, name: &str) -> bool {
    matches!(expression, Expression::CallExpression(call) if is_call_named(call, name))
}

fn is_call_named(call: &oxc_ast::ast::CallExpression<'_>, name: &str) -> bool {
    matches!(&call.callee, Expression::Identifier(identifier) if identifier.name == name)
}

fn collect_define_props_call(
    source: &str,
    call: &oxc_ast::ast::CallExpression<'_>,
    binding: Option<&BindingPattern<'_>>,
    analysis: &mut Vue27ScriptSetupAnalysis,
    is_prod: bool,
) {
    if let Some(type_argument) = call
        .type_arguments
        .as_ref()
        .and_then(|arguments| arguments.params.first())
    {
        if !call.arguments.is_empty() {
            analysis
                .errors
                .push(vue27_macro_type_and_runtime_error("defineProps"));
        }
        collect_define_props_type(source, type_argument, binding, None, analysis, is_prod);
        return;
    }
    if let Some(argument) = call.arguments.first() {
        let expression = argument.to_expression();
        check_vue27_invalid_scope_reference(expression, "defineProps", analysis);
        if let Expression::ObjectExpression(object) = expression {
            for key in object_expression_keys(object) {
                push_unique(&mut analysis.props_bindings, &key);
            }
        }
        let start = expression.span().start as usize;
        let end = expression.span().end as usize;
        analysis.props_runtime = source.get(start..end).map(ToOwned::to_owned);
    }
}

fn collect_with_defaults_call(
    source: &str,
    call: &oxc_ast::ast::CallExpression<'_>,
    binding: Option<&BindingPattern<'_>>,
    analysis: &mut Vue27ScriptSetupAnalysis,
    is_prod: bool,
) -> bool {
    let Some(define_props_call) =
        call.arguments
            .first()
            .and_then(|argument| match argument.to_expression() {
                Expression::CallExpression(call) if is_call_named(call, "defineProps") => {
                    Some(call)
                }
                _ => None,
            })
    else {
        return false;
    };
    let Some(type_argument) = define_props_call
        .type_arguments
        .as_ref()
        .and_then(|arguments| arguments.params.first())
    else {
        collect_define_props_call(source, define_props_call, binding, analysis, is_prod);
        return true;
    };
    let defaults = call.arguments.get(1).map(|argument| {
        check_vue27_invalid_scope_reference(argument.to_expression(), "defineProps", analysis);
        vue27_runtime_defaults_from_argument(source, argument)
    });
    collect_define_props_type(
        source,
        type_argument,
        binding,
        defaults.flatten(),
        analysis,
        is_prod,
    );
    true
}

fn collect_define_emits_call(
    source: &str,
    call: &oxc_ast::ast::CallExpression<'_>,
    binding: Option<&str>,
    analysis: &mut Vue27ScriptSetupAnalysis,
) {
    if analysis.emit_binding.is_none() {
        if let Some(binding) = binding {
            analysis.emit_binding = Some(binding.to_string());
        }
    }
    if let Some(type_argument) = call
        .type_arguments
        .as_ref()
        .and_then(|arguments| arguments.params.first())
    {
        if !call.arguments.is_empty() {
            analysis
                .errors
                .push(vue27_macro_type_and_runtime_error("defineEmits"));
        }
        collect_define_emits_type(source, type_argument, analysis);
        return;
    }
    let Some(argument) = call.arguments.first() else {
        return;
    };
    let expression = argument.to_expression();
    check_vue27_invalid_scope_reference(expression, "defineEmits", analysis);
    let start = expression.span().start as usize;
    let end = expression.span().end as usize;
    analysis.emits_runtime = source.get(start..end).map(ToOwned::to_owned);
}

fn collect_define_props_type(
    source: &str,
    type_argument: &TSType<'_>,
    binding: Option<&BindingPattern<'_>>,
    defaults: Option<Vue27RuntimeDefaults>,
    analysis: &mut Vue27ScriptSetupAnalysis,
    is_prod: bool,
) {
    let Some(type_members) = vue27_resolve_props_type(source, type_argument, analysis) else {
        return;
    };
    let default_map = defaults
        .as_ref()
        .and_then(|defaults| defaults.static_defaults.as_ref());
    let mut props = Vec::new();
    for member in &type_members.members {
        let mut prop = member.clone();
        if let Some(default) = default_map.and_then(|defaults| defaults.get(&prop.key)) {
            prop.default = Some(default.clone());
        }
        push_unique(&mut analysis.props_bindings, &prop.key);
        props.push(prop);
    }
    analysis.props_runtime_defaults = defaults;
    analysis.needs_merge_defaults = analysis
        .props_runtime_defaults
        .as_ref()
        .is_some_and(|defaults| defaults.static_defaults.is_none());
    analysis.props_type_runtime = true;
    analysis.props_type_source = Some(vue27_setup_props_type_source(
        source,
        type_argument,
        &type_members,
        analysis.props_runtime_defaults.as_ref(),
    ));
    analysis.props_runtime = Some(gen_vue27_runtime_props(
        &props,
        analysis.props_runtime_defaults.as_ref(),
        is_prod,
    ));
    if let Some(binding) = binding {
        analysis
            .setup_prelude
            .push_str(&vue27_props_type_assignment(
                source,
                binding,
                analysis.props_type_source.as_deref(),
            ));
    }
}

fn collect_define_emits_type(
    source: &str,
    type_argument: &TSType<'_>,
    analysis: &mut Vue27ScriptSetupAnalysis,
) {
    if !vue27_emits_type_argument_is_supported(type_argument, analysis) {
        analysis.errors.push(
            "type argument passed to defineEmits() must be a function type, a literal type with call signatures, or a reference to the above types."
                .to_string(),
        );
        return;
    }
    let Some(emits_type) = vue27_resolve_emits_type(source, type_argument, analysis) else {
        return;
    };
    if !emits_type.events.is_empty() {
        analysis.emits_runtime = Some(format!(
            "[{}]",
            emits_type
                .events
                .iter()
                .map(|name| format!("\"{}\"", escape_js_double(name)))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    analysis.emit_type_source = Some(emits_type.source);
}

fn vue27_script_setup_module_export_error() -> String {
    "<script setup> cannot contain ES module exports. If you are using a previous version of <script setup>, please consult the updated RFC at https://github.com/vuejs/rfcs/pull/227.".to_string()
}

fn vue27_macro_type_and_runtime_error(macro_name: &str) -> String {
    format!(
        "{macro_name}() cannot accept both type and non-type arguments at the same time. Use one or the other."
    )
}

fn check_vue27_invalid_scope_reference(
    expression: &Expression<'_>,
    macro_name: &str,
    analysis: &mut Vue27ScriptSetupAnalysis,
) {
    if vue27_expression_references_setup_local(expression, &analysis.local_setup_bindings) {
        analysis.errors.push(format!(
            "`{macro_name}()` in <script setup> cannot reference locally declared variables because it will be hoisted outside of the setup() function. If your component options require initialization in the module scope, use a separate normal <script> to export the options instead."
        ));
    }
}

fn vue27_emits_type_argument_is_supported(
    type_argument: &TSType<'_>,
    analysis: &Vue27ScriptSetupAnalysis,
) -> bool {
    match type_argument {
        TSType::TSFunctionType(_) | TSType::TSTypeLiteral(_) => true,
        TSType::TSTypeReference(reference) => vue27_ts_type_name_identifier(&reference.type_name)
            .is_some_and(|name| analysis.emits_type_declarations.contains_key(name)),
        _ => false,
    }
}

fn vue27_resolve_props_type<'a>(
    source: &str,
    type_argument: &'a TSType<'a>,
    analysis: &Vue27ScriptSetupAnalysis,
) -> Option<Vue27TypeMembers> {
    match type_argument {
        TSType::TSTypeLiteral(literal) => {
            Some(vue27_type_members_from_literal(source, literal, analysis))
        }
        TSType::TSTypeReference(reference) => {
            let name = vue27_ts_type_name_identifier(&reference.type_name)?;
            analysis.props_type_declarations.get(name).cloned()
        }
        _ => None,
    }
}

fn vue27_resolve_emits_type<'a>(
    source: &str,
    type_argument: &'a TSType<'a>,
    analysis: &Vue27ScriptSetupAnalysis,
) -> Option<Vue27EmitsType> {
    match type_argument {
        TSType::TSFunctionType(function) => Some(vue27_emits_type_from_function(source, function)),
        TSType::TSTypeLiteral(literal) => Some(vue27_emits_type_from_literal(source, literal)),
        TSType::TSTypeReference(reference) => {
            let name = vue27_ts_type_name_identifier(&reference.type_name)?;
            analysis.emits_type_declarations.get(name).cloned()
        }
        _ => None,
    }
}

fn vue27_type_members_from_literal(
    source: &str,
    literal: &TSTypeLiteral<'_>,
    analysis: &Vue27ScriptSetupAnalysis,
) -> Vue27TypeMembers {
    Vue27TypeMembers {
        source: source
            .get(literal.span.start as usize..literal.span.end as usize)
            .unwrap_or_default()
            .to_string(),
        members: vue27_runtime_props_from_signatures(source, &literal.members, analysis),
    }
}

fn vue27_type_members_from_interface_body(
    source: &str,
    body: &TSInterfaceBody<'_>,
    analysis: &Vue27ScriptSetupAnalysis,
) -> Vue27TypeMembers {
    Vue27TypeMembers {
        source: source
            .get(body.span.start as usize..body.span.end as usize)
            .unwrap_or_default()
            .to_string(),
        members: vue27_runtime_props_from_signatures(source, &body.body, analysis),
    }
}

fn vue27_runtime_props_from_signatures(
    source: &str,
    signatures: &[TSSignature<'_>],
    analysis: &Vue27ScriptSetupAnalysis,
) -> Vec<Vue27RuntimeProp> {
    let mut props = Vec::new();
    for signature in signatures {
        match signature {
            TSSignature::TSPropertySignature(property) if !property.computed => {
                if let Some(key) = vue27_property_key_static_name(&property.key) {
                    let types = property
                        .type_annotation
                        .as_ref()
                        .map(|annotation| {
                            infer_vue27_runtime_type(&annotation.type_annotation, analysis)
                        })
                        .unwrap_or_else(|| vec!["null".into()]);
                    props.push(Vue27RuntimeProp {
                        key,
                        types,
                        required: !property.optional,
                        default: None,
                        is_method: false,
                        type_annotation_source: property.type_annotation.as_ref().and_then(
                            |annotation| {
                                source
                                    .get(
                                        annotation.span.start as usize
                                            ..annotation.span.end as usize,
                                    )
                                    .map(ToOwned::to_owned)
                            },
                        ),
                        member_source: source
                            .get(property.span.start as usize..property.span.end as usize)
                            .map(ToOwned::to_owned),
                    });
                }
            }
            TSSignature::TSMethodSignature(method) if !method.computed => {
                if let Some(key) = vue27_property_key_static_name(&method.key) {
                    props.push(Vue27RuntimeProp {
                        key,
                        types: vec!["Function".into()],
                        required: !method.optional,
                        default: None,
                        is_method: true,
                        type_annotation_source: method.return_type.as_ref().and_then(
                            |annotation| {
                                source
                                    .get(
                                        annotation.span.start as usize
                                            ..annotation.span.end as usize,
                                    )
                                    .map(ToOwned::to_owned)
                            },
                        ),
                        member_source: source
                            .get(method.span.start as usize..method.span.end as usize)
                            .map(ToOwned::to_owned),
                    });
                }
            }
            _ => {}
        }
    }
    props
}

fn infer_vue27_runtime_type(node: &TSType<'_>, analysis: &Vue27ScriptSetupAnalysis) -> Vec<String> {
    match node {
        TSType::TSStringKeyword(_) => vec!["String".into()],
        TSType::TSNumberKeyword(_) => vec!["Number".into()],
        TSType::TSBooleanKeyword(_) => vec!["Boolean".into()],
        TSType::TSObjectKeyword(_) | TSType::TSTypeLiteral(_) | TSType::TSIntersectionType(_) => {
            vec!["Object".into()]
        }
        TSType::TSFunctionType(_) => vec!["Function".into()],
        TSType::TSArrayType(_) | TSType::TSTupleType(_) => vec!["Array".into()],
        TSType::TSSymbolKeyword(_) => vec!["Symbol".into()],
        TSType::TSLiteralType(literal) => match &literal.literal {
            TSLiteral::StringLiteral(_) => vec!["String".into()],
            TSLiteral::BooleanLiteral(_) => vec!["Boolean".into()],
            TSLiteral::NumericLiteral(_) | TSLiteral::BigIntLiteral(_) => vec!["Number".into()],
            _ => vec!["null".into()],
        },
        TSType::TSTypeReference(reference) => {
            if let Some(name) = vue27_ts_type_name_identifier(&reference.type_name) {
                if let Some(types) = analysis.declared_types.get(name) {
                    return types.clone();
                }
                match name {
                    "Array" | "Function" | "Object" | "Set" | "Map" | "WeakSet" | "WeakMap"
                    | "Date" | "Promise" => return vec![name.to_string()],
                    "Record" | "Partial" | "Readonly" | "Pick" | "Omit" | "Exclude" | "Extract"
                    | "Required" | "InstanceType" => return vec!["Object".into()],
                    _ => {}
                }
            }
            vec!["null".into()]
        }
        TSType::TSParenthesizedType(parenthesized) => {
            infer_vue27_runtime_type(&parenthesized.type_annotation, analysis)
        }
        TSType::TSUnionType(union) => {
            let mut types = Vec::new();
            for ty in &union.types {
                for runtime_type in infer_vue27_runtime_type(ty, analysis) {
                    push_unique(&mut types, &runtime_type);
                }
            }
            types
        }
        _ => vec!["null".into()],
    }
}

fn vue27_runtime_defaults_from_argument(
    source: &str,
    argument: &Argument<'_>,
) -> Option<Vue27RuntimeDefaults> {
    let expression = argument.to_expression();
    let source_text = source
        .get(expression.span().start as usize..expression.span().end as usize)?
        .to_string();
    let Expression::ObjectExpression(object) = expression else {
        return Some(Vue27RuntimeDefaults {
            source: source_text,
            static_defaults: None,
        });
    };
    let mut defaults = BTreeMap::new();
    for property in &object.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            return Some(Vue27RuntimeDefaults {
                source: source_text,
                static_defaults: None,
            });
        };
        if property.computed {
            return Some(Vue27RuntimeDefaults {
                source: source_text,
                static_defaults: None,
            });
        }
        if let Some(key) = vue27_property_key_static_name(&property.key) {
            let default_source = if property.method {
                vue27_function_body_source(source, &property.value)
                    .map(|body| format!("default() {body}"))
            } else {
                source
                    .get(property.value.span().start as usize..property.value.span().end as usize)
                    .map(|value| format!("default: {value}"))
            };
            if let Some(default_source) = default_source {
                defaults.insert(key, default_source);
            }
        }
    }
    Some(Vue27RuntimeDefaults {
        source: source_text,
        static_defaults: Some(defaults),
    })
}

fn vue27_function_body_source<'a>(source: &'a str, expression: &Expression<'_>) -> Option<&'a str> {
    match expression {
        Expression::FunctionExpression(function) => function
            .body
            .as_ref()
            .and_then(|body| source.get(body.span.start as usize..body.span.end as usize)),
        _ => source.get(expression.span().start as usize..expression.span().end as usize),
    }
}

fn vue27_setup_props_type_source(
    source: &str,
    type_argument: &TSType<'_>,
    type_members: &Vue27TypeMembers,
    defaults: Option<&Vue27RuntimeDefaults>,
) -> String {
    let Some(defaults) = defaults.and_then(|defaults| defaults.static_defaults.as_ref()) else {
        if !type_members.source.is_empty() {
            return type_members.source.clone();
        }
        return source
            .get(type_argument.span().start as usize..type_argument.span().end as usize)
            .unwrap_or_default()
            .to_string();
    };
    let mut parts = Vec::new();
    for prop in &type_members.members {
        if defaults.contains_key(&prop.key) {
            if let Some(type_annotation) = &prop.type_annotation_source {
                parts.push(format!(
                    "{}{}{}",
                    prop.key,
                    if prop.is_method { "()" } else { "" },
                    type_annotation
                ));
            }
        } else if let Some(member_source) = vue27_prop_member_type_source(prop) {
            parts.push(member_source);
        }
    }
    format!("{{ {} }}", parts.join(", "))
}

fn vue27_prop_member_type_source(prop: &Vue27RuntimeProp) -> Option<String> {
    let member_source = prop.member_source.as_deref()?;
    let type_annotation = prop.type_annotation_source.as_deref()?;
    let end = member_source.find(type_annotation)? + type_annotation.len();
    Some(member_source[..end].trim().to_string())
}

fn gen_vue27_runtime_props(
    props: &[Vue27RuntimeProp],
    defaults: Option<&Vue27RuntimeDefaults>,
    is_prod: bool,
) -> String {
    let mut entries = Vec::new();
    for prop in props {
        let type_string = vue27_runtime_type_string(&prop.types);
        if !is_prod {
            entries.push(format!(
                "{}: {{ type: {}, required: {}{} }}",
                prop.key,
                type_string,
                prop.required,
                prop.default
                    .as_ref()
                    .map(|default| format!(", {default}"))
                    .unwrap_or_default()
            ));
        } else if prop
            .types
            .iter()
            .any(|ty| ty == "Boolean" || (prop.default.is_some() && ty == "Function"))
        {
            entries.push(format!(
                "{}: {{ type: {}{} }}",
                prop.key,
                type_string,
                prop.default
                    .as_ref()
                    .map(|default| format!(", {default}"))
                    .unwrap_or_default()
            ));
        } else {
            entries.push(format!(
                "{}: {}",
                prop.key,
                prop.default
                    .as_ref()
                    .map(|default| format!("{{ {default} }}"))
                    .unwrap_or_else(|| "null".into())
            ));
        }
    }
    let props_decl = format!("{{\n    {}\n  }}", entries.join(",\n    "));
    if let Some(defaults) = defaults {
        if defaults.static_defaults.is_none() {
            return format!("_mergeDefaults({props_decl}, {})", defaults.source);
        }
    }
    props_decl
}

fn vue27_runtime_type_string(types: &[String]) -> String {
    if types.len() > 1 {
        format!("[{}]", types.join(", "))
    } else {
        types.first().cloned().unwrap_or_else(|| "null".into())
    }
}

fn vue27_props_type_assignment(
    source: &str,
    binding: &BindingPattern<'_>,
    type_source: Option<&str>,
) -> String {
    let binding_source = source
        .get(binding.span().start as usize..binding.span().end as usize)
        .unwrap_or("props")
        .trim();
    let cast = type_source
        .filter(|value| !value.is_empty())
        .map(|value| format!(" as {value}"))
        .unwrap_or_default();
    format!("\nconst {binding_source} = __props{cast};\n")
}

fn vue27_emits_type_from_function(source: &str, function: &TSFunctionType<'_>) -> Vue27EmitsType {
    let mut events = Vec::new();
    collect_vue27_emits_from_parameters(&function.params.items, &mut events);
    Vue27EmitsType {
        source: source
            .get(function.span.start as usize..function.span.end as usize)
            .unwrap_or_default()
            .to_string(),
        events,
    }
}

fn vue27_emits_type_from_literal(source: &str, literal: &TSTypeLiteral<'_>) -> Vue27EmitsType {
    let mut events = Vec::new();
    for member in &literal.members {
        if let TSSignature::TSCallSignatureDeclaration(signature) = member {
            collect_vue27_emits_from_parameters(&signature.params.items, &mut events);
        }
    }
    Vue27EmitsType {
        source: source
            .get(literal.span.start as usize..literal.span.end as usize)
            .unwrap_or_default()
            .to_string(),
        events,
    }
}

fn vue27_emits_type_from_interface_body(
    source: &str,
    body: &TSInterfaceBody<'_>,
) -> Vue27EmitsType {
    let mut events = Vec::new();
    for member in &body.body {
        if let TSSignature::TSCallSignatureDeclaration(signature) = member {
            collect_vue27_emits_from_parameters(&signature.params.items, &mut events);
        }
    }
    Vue27EmitsType {
        source: source
            .get(body.span.start as usize..body.span.end as usize)
            .unwrap_or_default()
            .to_string(),
        events,
    }
}

fn collect_vue27_emits_from_parameters(
    parameters: &[FormalParameter<'_>],
    names: &mut Vec<String>,
) {
    let Some(parameter) = parameters.first() else {
        return;
    };
    let Some(annotation) = parameter.type_annotation.as_ref() else {
        return;
    };
    collect_vue27_emits_from_type(&annotation.type_annotation, names);
}

fn collect_vue27_emits_from_type(ty: &TSType<'_>, names: &mut Vec<String>) {
    match ty {
        TSType::TSLiteralType(literal) => {
            if let Some(name) = vue27_literal_event_name(&literal.literal) {
                push_unique(names, &name);
            }
        }
        TSType::TSUnionType(union) => {
            for ty in &union.types {
                collect_vue27_emits_from_type(ty, names);
            }
        }
        _ => {}
    }
}

fn vue27_literal_event_name(literal: &TSLiteral<'_>) -> Option<String> {
    match literal {
        TSLiteral::StringLiteral(literal) => Some(literal.value.to_string()),
        TSLiteral::BooleanLiteral(literal) => Some(literal.value.to_string()),
        TSLiteral::NumericLiteral(literal) => Some(literal.value.to_string()),
        TSLiteral::BigIntLiteral(literal) => Some(literal.value.to_string()),
        _ => None,
    }
}

fn vue27_property_key_static_name(key: &PropertyKey<'_>) -> Option<String> {
    key.static_name().map(|name| name.into_owned())
}

fn vue27_ts_type_name_identifier<'a>(name: &'a TSTypeName<'a>) -> Option<&'a str> {
    match name {
        TSTypeName::IdentifierReference(identifier) => Some(identifier.name.as_str()),
        _ => None,
    }
}

fn escape_js_double(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn first_pattern_binding(pattern: &BindingPattern<'_>) -> Option<String> {
    match pattern {
        BindingPattern::BindingIdentifier(identifier) => Some(identifier.name.to_string()),
        BindingPattern::ObjectPattern(pattern) => pattern
            .properties
            .iter()
            .find_map(|property| first_pattern_binding(&property.value))
            .or_else(|| {
                pattern
                    .rest
                    .as_ref()
                    .and_then(|rest| first_pattern_binding(&rest.argument))
            }),
        BindingPattern::ArrayPattern(pattern) => pattern
            .elements
            .iter()
            .flatten()
            .find_map(first_pattern_binding)
            .or_else(|| {
                pattern
                    .rest
                    .as_ref()
                    .and_then(|rest| first_pattern_binding(&rest.argument))
            }),
        BindingPattern::AssignmentPattern(pattern) => first_pattern_binding(&pattern.left),
    }
}

fn vue27_props_alias_declaration(source: &str, pattern: &BindingPattern<'_>) -> String {
    let pattern_source = source
        .get(pattern.span().start as usize..pattern.span().end as usize)
        .map(str::trim)
        .filter(|source| !source.is_empty());
    if let Some(pattern_source) = pattern_source {
        format!("\nconst {pattern_source} = __props;\n")
    } else {
        String::new()
    }
}

fn remove_vue27_macro_declarators(
    declaration: &VariableDeclaration<'_>,
    macro_indices: &[usize],
    edits: &mut SourceEdits<'_>,
) {
    if macro_indices.is_empty() {
        return;
    }
    if macro_indices.len() == declaration.declarations.len() {
        edits.remove(
            declaration.span.start as usize,
            declaration.span.end as usize,
        );
        return;
    }
    let mut spans = Vec::new();
    for index in macro_indices {
        let declarator = &declaration.declarations[*index];
        let (start, end) = if *index == 0 {
            (
                declarator.span.start as usize,
                declaration.declarations[index + 1].span.start as usize,
            )
        } else {
            (
                declaration.declarations[index - 1].span.end as usize,
                declarator.span.end as usize,
            )
        };
        spans.push((start, end));
    }
    spans.sort_unstable();
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (start, end) in spans {
        if let Some((_, last_end)) = merged.last_mut() {
            if start <= *last_end {
                *last_end = (*last_end).max(end);
                continue;
            }
        }
        merged.push((start, end));
    }
    for (start, end) in merged {
        edits.remove(start, end);
    }
}

fn object_expression_keys(object: &ObjectExpression<'_>) -> Vec<String> {
    object
        .properties
        .iter()
        .filter_map(|property| property.as_property())
        .filter(|property| !property.computed)
        .filter_map(|property| property.key.static_name().map(|name| name.into_owned()))
        .collect()
}

fn import_specifier_local(specifier: &ImportDeclarationSpecifier<'_>) -> String {
    match specifier {
        ImportDeclarationSpecifier::ImportSpecifier(specifier) => specifier.local.name.to_string(),
        ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
            specifier.local.name.to_string()
        }
        ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
            specifier.local.name.to_string()
        }
    }
}

fn import_specifier_imported(specifier: &ImportDeclarationSpecifier<'_>) -> Option<String> {
    match specifier {
        ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
            Some(specifier.imported.name().to_string())
        }
        ImportDeclarationSpecifier::ImportDefaultSpecifier(_) => Some("default".into()),
        ImportDeclarationSpecifier::ImportNamespaceSpecifier(_) => Some("*".into()),
    }
}

fn vue27_import_specifier_is_type(
    import: &oxc_ast::ast::ImportDeclaration<'_>,
    specifier: &ImportDeclarationSpecifier<'_>,
) -> bool {
    import.import_kind == ImportOrExportKind::Type
        || matches!(
            specifier,
            ImportDeclarationSpecifier::ImportSpecifier(specifier)
                if specifier.import_kind == ImportOrExportKind::Type
        )
}

fn import_specifier_setup_dedupe_imported(
    specifier: &ImportDeclarationSpecifier<'_>,
) -> Option<String> {
    match specifier {
        ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
            Some(specifier.imported.name().to_string())
        }
        ImportDeclarationSpecifier::ImportNamespaceSpecifier(_) => Some("*".into()),
        ImportDeclarationSpecifier::ImportDefaultSpecifier(_) => None,
    }
}

fn vue27_import_already_declared_in_setup_context(
    analysis: &Vue27ScriptSetupAnalysis,
    source: &str,
    local: &str,
    imported: Option<&str>,
) -> bool {
    analysis.normal_imports.iter().any(|existing| {
        existing.local == local
            && existing.source == source
            && existing.imported == imported.unwrap_or("default")
    })
}

fn vue27_import_local_conflicts_in_setup_context(
    analysis: &Vue27ScriptSetupAnalysis,
    source: &str,
    local: &str,
    imported: Option<&str>,
) -> bool {
    analysis.normal_imports.iter().any(|existing| {
        existing.local == local
            && (existing.source != source || existing.imported != imported.unwrap_or("default"))
    })
}

fn import_specifier_source(source: &str, specifier: &ImportDeclarationSpecifier<'_>) -> String {
    source[specifier.span().start as usize..specifier.span().end as usize].to_string()
}

fn vue3_script_setup_kept_import_source(
    source: &str,
    import: &oxc_ast::ast::ImportDeclaration<'_>,
    source_value: &str,
    statement_start: usize,
    statement_end: usize,
) -> Option<String> {
    let Some(specifiers) = import.specifiers.as_ref() else {
        return source
            .get(statement_start..statement_end)
            .map(ToOwned::to_owned);
    };
    let kept = specifiers
        .iter()
        .filter(|specifier| vue3_import_specifier_compiler_macro(source_value, specifier).is_none())
        .collect::<Vec<_>>();
    if kept.is_empty() {
        return None;
    }
    if kept.len() == specifiers.len() {
        return source
            .get(statement_start..statement_end)
            .map(ToOwned::to_owned);
    }
    let trailing = source
        .get(import.span().end as usize..statement_end)
        .unwrap_or_default();
    let mut default_import = None;
    let mut namespace_import = None;
    let mut named_imports = Vec::new();
    for specifier in kept {
        let specifier_source = import_specifier_source(source, specifier);
        match specifier {
            ImportDeclarationSpecifier::ImportDefaultSpecifier(_) => {
                default_import = Some(specifier_source);
            }
            ImportDeclarationSpecifier::ImportNamespaceSpecifier(_) => {
                namespace_import = Some(specifier_source);
            }
            ImportDeclarationSpecifier::ImportSpecifier(_) => {
                named_imports.push(specifier_source);
            }
        }
    }
    let mut import_clause = String::new();
    if let Some(default_import) = default_import {
        import_clause.push_str(&default_import);
    }
    if let Some(namespace_import) = namespace_import {
        if !import_clause.is_empty() {
            import_clause.push_str(", ");
        }
        import_clause.push_str(&namespace_import);
    }
    if !named_imports.is_empty() {
        if !import_clause.is_empty() {
            import_clause.push_str(", ");
        }
        import_clause.push_str("{ ");
        import_clause.push_str(&named_imports.join(", "));
        import_clause.push_str(" }");
    }
    Some(format!(
        "import {import_clause} from '{source_value}'{trailing}"
    ))
}

fn vue3_import_specifier_compiler_macro(
    source: &str,
    specifier: &ImportDeclarationSpecifier<'_>,
) -> Option<(String, String)> {
    if source != "vue" {
        return None;
    }
    let ImportDeclarationSpecifier::ImportSpecifier(specifier) = specifier else {
        return None;
    };
    let imported = specifier.imported.name();
    if !matches!(
        imported.as_str(),
        "defineProps"
            | "defineEmits"
            | "defineExpose"
            | "defineOptions"
            | "defineModel"
            | "defineSlots"
            | "withDefaults"
    ) {
        return None;
    }
    Some((imported.to_string(), specifier.local.name.to_string()))
}

fn vue27_template_uses_identifier(template: &str, local: &str, is_ts: bool) -> bool {
    let usage = vue27_template_usage_check_string(template, is_ts);
    identifier_usage_contains(&usage, local)
}

fn vue3_template_uses_identifier(template: &str, local: &str, is_ts: bool) -> bool {
    let usage = vue3_template_usage_check_string(template, is_ts);
    identifier_usage_contains(&usage, local)
}

fn vue3_template_usage_check_string(template: &str, is_ts: bool) -> String {
    let mut code = String::new();
    for token in HtmlTokenizer::new(template).tokenize() {
        match token.kind {
            HtmlTokenKind::StartTag {
                name, attributes, ..
            } => {
                collect_vue3_template_component_usage(&mut code, &name);
                for attribute in attributes {
                    collect_vue3_template_attribute_usage(&mut code, &attribute, is_ts);
                }
            }
            HtmlTokenKind::Text(text) => {
                collect_vue27_template_text_usage(&mut code, &text, is_ts);
            }
            _ => {}
        }
    }
    code.push(';');
    code
}

fn collect_vue3_template_component_usage(code: &mut String, name: &str) {
    let tag = name
        .split_once('.')
        .map(|(base, _)| base.trim())
        .unwrap_or(name);
    if tag.is_empty() || vue3_template_is_builtin_tag(tag) || vue27_template_is_reserved_tag(tag) {
        return;
    }
    let camel = vue27_camelize(tag);
    code.push(',');
    code.push_str(&camel);
    code.push(',');
    code.push_str(&vue27_capitalize(&camel));
}

fn collect_vue3_template_attribute_usage(code: &mut String, attr: &HtmlAttribute, is_ts: bool) {
    let name = attr.name.as_str();
    if vue3_template_is_directive_attr(name) {
        let base_name = vue27_template_directive_base_name(name);
        if !vue27_template_is_builtin_dir(&base_name) {
            code.push_str(",v");
            code.push_str(&vue27_capitalize(&vue27_camelize(&base_name)));
        }
        if let Some(arg) = vue3_template_dynamic_argument(name) {
            code.push(',');
            code.push_str(&vue27_process_template_exp(arg, is_ts, None));
        }
        if let Some(value) = attr.value.as_deref() {
            code.push(',');
            code.push_str(&vue27_process_template_exp(value, is_ts, Some(&base_name)));
        } else if base_name == "bind" {
            if let Some(arg) = vue3_template_static_bind_argument(name) {
                code.push(',');
                code.push_str(&vue27_camelize(arg));
            }
        }
    } else if name == "ref" {
        if let Some(value) = attr.value.as_deref() {
            code.push(',');
            code.push_str(value);
        }
    }
}

fn vue3_template_is_directive_attr(name: &str) -> bool {
    vue27_template_is_directive_attr(name) || name.starts_with('.')
}

fn vue3_template_dynamic_argument(name: &str) -> Option<&str> {
    let start = name.find('[')?;
    let rest = &name[start + 1..];
    let end = rest.find(']')?;
    Some(&rest[..end])
}

fn vue3_template_static_bind_argument(name: &str) -> Option<&str> {
    if vue3_template_dynamic_argument(name).is_some() {
        return None;
    }
    let raw = if let Some(arg) = name.strip_prefix(':') {
        arg
    } else if let Some(arg) = name.strip_prefix('.') {
        arg
    } else if let Some(arg) = name.strip_prefix("v-bind:") {
        arg
    } else {
        return None;
    };
    raw.split('.').next().filter(|arg| !arg.is_empty())
}

fn vue3_template_is_builtin_tag(name: &str) -> bool {
    vue27_template_is_builtin_tag(name)
        || matches!(
            name,
            "Teleport"
                | "teleport"
                | "Suspense"
                | "suspense"
                | "KeepAlive"
                | "keep-alive"
                | "BaseTransition"
                | "base-transition"
                | "Transition"
                | "transition"
                | "TransitionGroup"
                | "transition-group"
        )
}

fn vue27_template_usage_check_string(template: &str, is_ts: bool) -> String {
    let mut code = String::new();
    for token in HtmlTokenizer::new(template).tokenize() {
        match token.kind {
            HtmlTokenKind::StartTag {
                name, attributes, ..
            } => {
                if !vue27_template_is_builtin_tag(&name) && !vue27_template_is_reserved_tag(&name) {
                    let camel = vue27_camelize(&name);
                    code.push(',');
                    code.push_str(&camel);
                    code.push(',');
                    code.push_str(&vue27_capitalize(&camel));
                }
                for attribute in attributes {
                    collect_vue27_template_attribute_usage(&mut code, &attribute, is_ts);
                }
            }
            HtmlTokenKind::Text(text) => {
                collect_vue27_template_text_usage(&mut code, &text, is_ts);
            }
            _ => {}
        }
    }
    code.push(';');
    code
}

fn collect_vue27_template_attribute_usage(code: &mut String, attr: &HtmlAttribute, is_ts: bool) {
    let name = attr.name.as_str();
    if vue27_template_is_directive_attr(name) {
        let base_name = vue27_template_directive_base_name(name);
        if !vue27_template_is_builtin_dir(&base_name) {
            code.push_str(",v");
            code.push_str(&vue27_capitalize(&vue27_camelize(&base_name)));
        }
        if let Some(value) = attr.value.as_deref() {
            code.push(',');
            code.push_str(&vue27_process_template_exp(value, is_ts, Some(&base_name)));
        }
    } else if name == "ref" {
        if let Some(value) = attr.value.as_deref() {
            code.push(',');
            code.push_str(value);
        }
    }
}

fn collect_vue27_template_text_usage(code: &mut String, text: &str, is_ts: bool) {
    let mut rest = text;
    while let Some(start) = rest.find("{{") {
        let after_start = &rest[start + 2..];
        let Some(end) = after_start.find("}}") else {
            break;
        };
        let expression = after_start[..end].trim();
        if !expression.is_empty() {
            code.push(',');
            code.push_str(&vue27_process_template_exp(expression, is_ts, None));
        }
        rest = &after_start[end + 2..];
    }
}

fn vue27_template_directive_base_name(name: &str) -> String {
    let body = if let Some(value) = name.strip_prefix("v-") {
        value
    } else if name.starts_with('@') {
        return "on".into();
    } else if name.starts_with('#') {
        return "slot".into();
    } else if name.starts_with(':') {
        return "bind".into();
    } else {
        name
    };
    body.split([':', '.', '['])
        .next()
        .filter(|value| !value.is_empty())
        .unwrap_or(body)
        .to_string()
}

fn vue27_template_is_directive_attr(name: &str) -> bool {
    name.starts_with("v-")
        || name.starts_with(':')
        || name.starts_with('@')
        || name.starts_with('#')
}

fn vue27_template_is_builtin_dir(name: &str) -> bool {
    matches!(
        name,
        "text"
            | "html"
            | "show"
            | "if"
            | "else"
            | "else-if"
            | "for"
            | "on"
            | "bind"
            | "model"
            | "slot"
            | "pre"
            | "cloak"
            | "once"
            | "memo"
    )
}

fn vue27_template_is_builtin_tag(name: &str) -> bool {
    matches!(name, "slot" | "component")
}

fn vue27_template_is_reserved_tag(name: &str) -> bool {
    const RESERVED: &str = concat!(
        "html,body,base,head,link,meta,style,title,address,article,aside,footer,header,h1,h2,h3,h4,h5,h6,",
        "nav,section,div,dd,dl,dt,figcaption,figure,picture,hr,img,li,main,ol,p,pre,ul,a,b,abbr,bdi,bdo,",
        "br,cite,code,data,dfn,em,i,kbd,mark,q,rp,rt,ruby,s,samp,small,span,strong,sub,sup,time,u,var,wbr,",
        "area,audio,map,track,video,embed,object,param,source,canvas,script,noscript,del,ins,caption,col,",
        "colgroup,table,thead,tbody,td,th,tr,button,datalist,fieldset,form,input,label,legend,meter,optgroup,",
        "option,output,progress,select,textarea,details,dialog,menu,menuitem,summary,content,element,shadow,",
        "template,blockquote,iframe,tfoot,svg,animate,circle,clippath,cursor,defs,desc,ellipse,filter,font-face,",
        "foreignObject,g,glyph,image,line,marker,mask,missing-glyph,path,pattern,polygon,polyline,rect,switch,",
        "symbol,text,textpath,tspan,use,view"
    );
    RESERVED
        .split(',')
        .any(|tag| tag.eq_ignore_ascii_case(name))
}

fn vue27_process_template_exp(exp: &str, is_ts: bool, directive: Option<&str>) -> String {
    if is_ts && vue27_template_exp_has_ts_syntax(exp) {
        if directive == Some("slot") {
            return vue27_extract_js_identifiers(&format!("({exp})=>{{}}"));
        }
        if directive == Some("on") {
            return vue27_extract_js_identifiers(&format!("()=>{{return {exp}}}"));
        }
        if directive == Some("for") {
            if let Some((left, right)) = vue27_split_for_expression(exp) {
                let mut value = vue27_extract_js_identifiers(&format!("({left})=>{{}}"));
                value.push_str(&vue27_extract_js_identifiers(right));
                return value;
            }
        }
        return vue27_extract_js_identifiers(exp);
    }
    vue27_strip_template_expression_strings(exp)
}

fn vue27_template_exp_has_ts_syntax(exp: &str) -> bool {
    exp.contains(':') || exp.contains('<') || exp.split_whitespace().any(|part| part == "as")
}

fn vue27_split_for_expression(exp: &str) -> Option<(&str, &str)> {
    for keyword in [" in ", " of "] {
        if let Some(index) = exp.find(keyword) {
            let left = exp[..index].trim();
            let right = exp[index + keyword.len()..].trim();
            if !left.is_empty() && !right.is_empty() {
                return Some((left, right));
            }
        }
    }
    None
}

fn vue27_extract_js_identifiers(exp: &str) -> String {
    let allocator = oxc_allocator::Allocator::default();
    let parse_options = oxc_parser::ParseOptions {
        parse_regular_expression: true,
        ..oxc_parser::ParseOptions::default()
    };
    if let Ok(expression) = oxc_parser::Parser::new(
        &allocator,
        exp,
        oxc_span::SourceType::ts().with_module(false),
    )
    .with_options(parse_options)
    .parse_expression()
    {
        let mut value = String::new();
        collect_vue27_expression_identifier_usage(&expression, &mut value);
        return value;
    }
    let parsed = oxc_parser::Parser::new(
        &allocator,
        exp,
        oxc_span::SourceType::ts().with_module(false),
    )
    .with_options(parse_options)
    .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return String::new();
    }
    let mut value = String::new();
    for statement in &parsed.program.body {
        collect_vue27_statement_identifier_usage(statement, &mut value);
    }
    value
}

fn collect_vue27_statement_identifier_usage(statement: &Statement<'_>, value: &mut String) {
    match statement {
        Statement::BlockStatement(block) => {
            for statement in &block.body {
                collect_vue27_statement_identifier_usage(statement, value);
            }
        }
        Statement::ExpressionStatement(statement) => {
            collect_vue27_expression_identifier_usage(&statement.expression, value);
        }
        Statement::ReturnStatement(statement) => {
            if let Some(argument) = &statement.argument {
                collect_vue27_expression_identifier_usage(argument, value);
            }
        }
        Statement::VariableDeclaration(declaration) => {
            for declarator in &declaration.declarations {
                if let Some(init) = &declarator.init {
                    collect_vue27_expression_identifier_usage(init, value);
                }
            }
        }
        Statement::FunctionDeclaration(function) => {
            collect_vue27_function_identifier_usage(function, value);
        }
        Statement::IfStatement(statement) => {
            collect_vue27_expression_identifier_usage(&statement.test, value);
            collect_vue27_statement_identifier_usage(&statement.consequent, value);
            if let Some(alternate) = &statement.alternate {
                collect_vue27_statement_identifier_usage(alternate, value);
            }
        }
        Statement::ForStatement(statement) => {
            if let Some(init) = &statement.init {
                match init {
                    oxc_ast::ast::ForStatementInit::VariableDeclaration(declaration) => {
                        for declarator in &declaration.declarations {
                            if let Some(init) = &declarator.init {
                                collect_vue27_expression_identifier_usage(init, value);
                            }
                        }
                    }
                    _ => {
                        if let Some(expression) = init.as_expression() {
                            collect_vue27_expression_identifier_usage(expression, value);
                        }
                    }
                }
            }
            if let Some(test) = &statement.test {
                collect_vue27_expression_identifier_usage(test, value);
            }
            if let Some(update) = &statement.update {
                collect_vue27_expression_identifier_usage(update, value);
            }
            collect_vue27_statement_identifier_usage(&statement.body, value);
        }
        Statement::ForInStatement(statement) => {
            collect_vue27_expression_identifier_usage(&statement.right, value);
            collect_vue27_statement_identifier_usage(&statement.body, value);
        }
        Statement::ForOfStatement(statement) => {
            collect_vue27_expression_identifier_usage(&statement.right, value);
            collect_vue27_statement_identifier_usage(&statement.body, value);
        }
        Statement::WhileStatement(statement) => {
            collect_vue27_expression_identifier_usage(&statement.test, value);
            collect_vue27_statement_identifier_usage(&statement.body, value);
        }
        Statement::DoWhileStatement(statement) => {
            collect_vue27_statement_identifier_usage(&statement.body, value);
            collect_vue27_expression_identifier_usage(&statement.test, value);
        }
        Statement::SwitchStatement(statement) => {
            collect_vue27_expression_identifier_usage(&statement.discriminant, value);
            for case in &statement.cases {
                if let Some(test) = &case.test {
                    collect_vue27_expression_identifier_usage(test, value);
                }
                for statement in &case.consequent {
                    collect_vue27_statement_identifier_usage(statement, value);
                }
            }
        }
        Statement::ThrowStatement(statement) => {
            collect_vue27_expression_identifier_usage(&statement.argument, value);
        }
        Statement::TryStatement(statement) => {
            for statement in &statement.block.body {
                collect_vue27_statement_identifier_usage(statement, value);
            }
            if let Some(handler) = &statement.handler {
                for statement in &handler.body.body {
                    collect_vue27_statement_identifier_usage(statement, value);
                }
            }
            if let Some(finalizer) = &statement.finalizer {
                for statement in &finalizer.body {
                    collect_vue27_statement_identifier_usage(statement, value);
                }
            }
        }
        Statement::WithStatement(statement) => {
            collect_vue27_expression_identifier_usage(&statement.object, value);
            collect_vue27_statement_identifier_usage(&statement.body, value);
        }
        Statement::LabeledStatement(statement) => {
            collect_vue27_statement_identifier_usage(&statement.body, value);
        }
        _ => {}
    }
}

fn collect_vue27_expression_identifier_usage(expression: &Expression<'_>, value: &mut String) {
    match expression {
        Expression::Identifier(identifier) => {
            push_vue27_identifier_usage(value, identifier.name.as_str())
        }
        Expression::ArrayExpression(array) => {
            for element in &array.elements {
                match element {
                    oxc_ast::ast::ArrayExpressionElement::SpreadElement(spread) => {
                        collect_vue27_expression_identifier_usage(&spread.argument, value);
                    }
                    oxc_ast::ast::ArrayExpressionElement::Elision(_) => {}
                    element => {
                        if let Some(expression) = element.as_expression() {
                            collect_vue27_expression_identifier_usage(expression, value);
                        }
                    }
                }
            }
        }
        Expression::ObjectExpression(object) => {
            for property in &object.properties {
                match property {
                    ObjectPropertyKind::ObjectProperty(property) => {
                        if property.computed {
                            collect_vue27_property_key_identifier_usage(&property.key, value);
                        }
                        collect_vue27_expression_identifier_usage(&property.value, value);
                    }
                    ObjectPropertyKind::SpreadProperty(spread) => {
                        collect_vue27_expression_identifier_usage(&spread.argument, value);
                    }
                }
            }
        }
        Expression::CallExpression(call) => {
            collect_vue27_expression_identifier_usage(&call.callee, value);
            for argument in &call.arguments {
                collect_vue27_argument_identifier_usage(argument, value);
            }
        }
        Expression::NewExpression(expression) => {
            collect_vue27_expression_identifier_usage(&expression.callee, value);
            for argument in &expression.arguments {
                collect_vue27_argument_identifier_usage(argument, value);
            }
        }
        Expression::StaticMemberExpression(member) => {
            collect_vue27_expression_identifier_usage(&member.object, value);
        }
        Expression::ComputedMemberExpression(member) => {
            collect_vue27_expression_identifier_usage(&member.object, value);
            collect_vue27_expression_identifier_usage(&member.expression, value);
        }
        Expression::PrivateFieldExpression(member) => {
            collect_vue27_expression_identifier_usage(&member.object, value);
        }
        Expression::FunctionExpression(function) => {
            collect_vue27_function_identifier_usage(function, value);
        }
        Expression::ArrowFunctionExpression(function) => {
            collect_vue27_arrow_function_identifier_usage(function, value);
        }
        Expression::AssignmentExpression(assignment) => {
            collect_vue27_assignment_target_identifier_usage(&assignment.left, value);
            collect_vue27_expression_identifier_usage(&assignment.right, value);
        }
        Expression::UpdateExpression(update) => {
            collect_vue27_simple_assignment_target_identifier_usage(&update.argument, value);
        }
        Expression::UnaryExpression(unary) => {
            collect_vue27_expression_identifier_usage(&unary.argument, value);
        }
        Expression::AwaitExpression(expression) => {
            collect_vue27_expression_identifier_usage(&expression.argument, value);
        }
        Expression::BinaryExpression(binary) => {
            collect_vue27_expression_identifier_usage(&binary.left, value);
            collect_vue27_expression_identifier_usage(&binary.right, value);
        }
        Expression::PrivateInExpression(expression) => {
            collect_vue27_expression_identifier_usage(&expression.right, value);
        }
        Expression::LogicalExpression(logical) => {
            collect_vue27_expression_identifier_usage(&logical.left, value);
            collect_vue27_expression_identifier_usage(&logical.right, value);
        }
        Expression::ConditionalExpression(conditional) => {
            collect_vue27_expression_identifier_usage(&conditional.test, value);
            collect_vue27_expression_identifier_usage(&conditional.consequent, value);
            collect_vue27_expression_identifier_usage(&conditional.alternate, value);
        }
        Expression::SequenceExpression(sequence) => {
            for expression in &sequence.expressions {
                collect_vue27_expression_identifier_usage(expression, value);
            }
        }
        Expression::TemplateLiteral(template) => {
            for expression in &template.expressions {
                collect_vue27_expression_identifier_usage(expression, value);
            }
        }
        Expression::TaggedTemplateExpression(template) => {
            collect_vue27_expression_identifier_usage(&template.tag, value);
            for expression in &template.quasi.expressions {
                collect_vue27_expression_identifier_usage(expression, value);
            }
        }
        Expression::ParenthesizedExpression(parenthesized) => {
            collect_vue27_expression_identifier_usage(&parenthesized.expression, value);
        }
        Expression::TSAsExpression(expression) => {
            collect_vue27_expression_identifier_usage(&expression.expression, value);
        }
        Expression::TSSatisfiesExpression(expression) => {
            collect_vue27_expression_identifier_usage(&expression.expression, value);
        }
        Expression::TSTypeAssertion(expression) => {
            collect_vue27_expression_identifier_usage(&expression.expression, value);
        }
        Expression::TSNonNullExpression(expression) => {
            collect_vue27_expression_identifier_usage(&expression.expression, value);
        }
        Expression::TSInstantiationExpression(expression) => {
            collect_vue27_expression_identifier_usage(&expression.expression, value);
        }
        Expression::ChainExpression(chain) => match &chain.expression {
            oxc_ast::ast::ChainElement::CallExpression(call) => {
                collect_vue27_expression_identifier_usage(&call.callee, value);
                for argument in &call.arguments {
                    collect_vue27_argument_identifier_usage(argument, value);
                }
            }
            oxc_ast::ast::ChainElement::TSNonNullExpression(expression) => {
                collect_vue27_expression_identifier_usage(&expression.expression, value);
            }
            oxc_ast::ast::ChainElement::StaticMemberExpression(member) => {
                collect_vue27_expression_identifier_usage(&member.object, value);
            }
            oxc_ast::ast::ChainElement::ComputedMemberExpression(member) => {
                collect_vue27_expression_identifier_usage(&member.object, value);
                collect_vue27_expression_identifier_usage(&member.expression, value);
            }
            oxc_ast::ast::ChainElement::PrivateFieldExpression(member) => {
                collect_vue27_expression_identifier_usage(&member.object, value);
            }
        },
        _ => {}
    }
}

fn vue27_expression_references_setup_local(
    expression: &Expression<'_>,
    setup_bindings: &BTreeSet<String>,
) -> bool {
    let mut scope = BTreeSet::new();
    vue27_expression_references_setup_local_with_scope(expression, setup_bindings, &mut scope)
}

fn vue27_expression_references_setup_local_with_scope(
    expression: &Expression<'_>,
    setup_bindings: &BTreeSet<String>,
    scope: &mut BTreeSet<String>,
) -> bool {
    match expression {
        Expression::Identifier(identifier) => {
            setup_bindings.contains(identifier.name.as_str())
                && !scope.contains(identifier.name.as_str())
        }
        Expression::ArrayExpression(array) => array.elements.iter().any(|element| match element {
            oxc_ast::ast::ArrayExpressionElement::SpreadElement(spread) => {
                vue27_expression_references_setup_local_with_scope(
                    &spread.argument,
                    setup_bindings,
                    scope,
                )
            }
            oxc_ast::ast::ArrayExpressionElement::Elision(_) => false,
            element => element.as_expression().is_some_and(|expression| {
                vue27_expression_references_setup_local_with_scope(
                    expression,
                    setup_bindings,
                    scope,
                )
            }),
        }),
        Expression::ObjectExpression(object) => {
            object.properties.iter().any(|property| match property {
                ObjectPropertyKind::ObjectProperty(property) => {
                    (property.computed
                        && vue27_property_key_references_setup_local(
                            &property.key,
                            setup_bindings,
                            scope,
                        ))
                        || vue27_expression_references_setup_local_with_scope(
                            &property.value,
                            setup_bindings,
                            scope,
                        )
                }
                ObjectPropertyKind::SpreadProperty(spread) => {
                    vue27_expression_references_setup_local_with_scope(
                        &spread.argument,
                        setup_bindings,
                        scope,
                    )
                }
            })
        }
        Expression::CallExpression(call) => {
            vue27_expression_references_setup_local_with_scope(&call.callee, setup_bindings, scope)
                || call.arguments.iter().any(|argument| {
                    vue27_argument_references_setup_local(argument, setup_bindings, scope)
                })
        }
        Expression::NewExpression(expression) => {
            vue27_expression_references_setup_local_with_scope(
                &expression.callee,
                setup_bindings,
                scope,
            ) || expression.arguments.iter().any(|argument| {
                vue27_argument_references_setup_local(argument, setup_bindings, scope)
            })
        }
        Expression::StaticMemberExpression(member) => {
            vue27_expression_references_setup_local_with_scope(
                &member.object,
                setup_bindings,
                scope,
            )
        }
        Expression::ComputedMemberExpression(member) => {
            vue27_expression_references_setup_local_with_scope(
                &member.object,
                setup_bindings,
                scope,
            ) || vue27_expression_references_setup_local_with_scope(
                &member.expression,
                setup_bindings,
                scope,
            )
        }
        Expression::PrivateFieldExpression(member) => {
            vue27_expression_references_setup_local_with_scope(
                &member.object,
                setup_bindings,
                scope,
            )
        }
        Expression::FunctionExpression(function) => {
            vue27_function_references_setup_local(function, setup_bindings, scope)
        }
        Expression::ArrowFunctionExpression(function) => {
            vue27_arrow_function_references_setup_local(function, setup_bindings, scope)
        }
        Expression::AssignmentExpression(assignment) => {
            vue27_assignment_target_references_setup_local(&assignment.left, setup_bindings, scope)
                || vue27_expression_references_setup_local_with_scope(
                    &assignment.right,
                    setup_bindings,
                    scope,
                )
        }
        Expression::UpdateExpression(update) => {
            vue27_simple_assignment_target_references_setup_local(
                &update.argument,
                setup_bindings,
                scope,
            )
        }
        Expression::UnaryExpression(unary) => vue27_expression_references_setup_local_with_scope(
            &unary.argument,
            setup_bindings,
            scope,
        ),
        Expression::AwaitExpression(expression) => {
            vue27_expression_references_setup_local_with_scope(
                &expression.argument,
                setup_bindings,
                scope,
            )
        }
        Expression::BinaryExpression(binary) => {
            vue27_expression_references_setup_local_with_scope(&binary.left, setup_bindings, scope)
                || vue27_expression_references_setup_local_with_scope(
                    &binary.right,
                    setup_bindings,
                    scope,
                )
        }
        Expression::PrivateInExpression(expression) => {
            vue27_expression_references_setup_local_with_scope(
                &expression.right,
                setup_bindings,
                scope,
            )
        }
        Expression::LogicalExpression(logical) => {
            vue27_expression_references_setup_local_with_scope(&logical.left, setup_bindings, scope)
                || vue27_expression_references_setup_local_with_scope(
                    &logical.right,
                    setup_bindings,
                    scope,
                )
        }
        Expression::ConditionalExpression(conditional) => {
            vue27_expression_references_setup_local_with_scope(
                &conditional.test,
                setup_bindings,
                scope,
            ) || vue27_expression_references_setup_local_with_scope(
                &conditional.consequent,
                setup_bindings,
                scope,
            ) || vue27_expression_references_setup_local_with_scope(
                &conditional.alternate,
                setup_bindings,
                scope,
            )
        }
        Expression::SequenceExpression(sequence) => sequence.expressions.iter().any(|expression| {
            vue27_expression_references_setup_local_with_scope(expression, setup_bindings, scope)
        }),
        Expression::TemplateLiteral(template) => template.expressions.iter().any(|expression| {
            vue27_expression_references_setup_local_with_scope(expression, setup_bindings, scope)
        }),
        Expression::TaggedTemplateExpression(template) => {
            vue27_expression_references_setup_local_with_scope(&template.tag, setup_bindings, scope)
                || template.quasi.expressions.iter().any(|expression| {
                    vue27_expression_references_setup_local_with_scope(
                        expression,
                        setup_bindings,
                        scope,
                    )
                })
        }
        Expression::ParenthesizedExpression(parenthesized) => {
            vue27_expression_references_setup_local_with_scope(
                &parenthesized.expression,
                setup_bindings,
                scope,
            )
        }
        Expression::TSAsExpression(expression) => {
            vue27_expression_references_setup_local_with_scope(
                &expression.expression,
                setup_bindings,
                scope,
            )
        }
        Expression::TSSatisfiesExpression(expression) => {
            vue27_expression_references_setup_local_with_scope(
                &expression.expression,
                setup_bindings,
                scope,
            )
        }
        Expression::TSTypeAssertion(expression) => {
            vue27_expression_references_setup_local_with_scope(
                &expression.expression,
                setup_bindings,
                scope,
            )
        }
        Expression::TSNonNullExpression(expression) => {
            vue27_expression_references_setup_local_with_scope(
                &expression.expression,
                setup_bindings,
                scope,
            )
        }
        Expression::TSInstantiationExpression(expression) => {
            vue27_expression_references_setup_local_with_scope(
                &expression.expression,
                setup_bindings,
                scope,
            )
        }
        Expression::ChainExpression(chain) => match &chain.expression {
            oxc_ast::ast::ChainElement::CallExpression(call) => {
                vue27_expression_references_setup_local_with_scope(
                    &call.callee,
                    setup_bindings,
                    scope,
                ) || call.arguments.iter().any(|argument| {
                    vue27_argument_references_setup_local(argument, setup_bindings, scope)
                })
            }
            oxc_ast::ast::ChainElement::TSNonNullExpression(expression) => {
                vue27_expression_references_setup_local_with_scope(
                    &expression.expression,
                    setup_bindings,
                    scope,
                )
            }
            oxc_ast::ast::ChainElement::StaticMemberExpression(member) => {
                vue27_expression_references_setup_local_with_scope(
                    &member.object,
                    setup_bindings,
                    scope,
                )
            }
            oxc_ast::ast::ChainElement::ComputedMemberExpression(member) => {
                vue27_expression_references_setup_local_with_scope(
                    &member.object,
                    setup_bindings,
                    scope,
                ) || vue27_expression_references_setup_local_with_scope(
                    &member.expression,
                    setup_bindings,
                    scope,
                )
            }
            oxc_ast::ast::ChainElement::PrivateFieldExpression(member) => {
                vue27_expression_references_setup_local_with_scope(
                    &member.object,
                    setup_bindings,
                    scope,
                )
            }
        },
        _ => false,
    }
}

fn vue27_argument_references_setup_local(
    argument: &Argument<'_>,
    setup_bindings: &BTreeSet<String>,
    scope: &mut BTreeSet<String>,
) -> bool {
    match argument {
        Argument::SpreadElement(spread) => vue27_expression_references_setup_local_with_scope(
            &spread.argument,
            setup_bindings,
            scope,
        ),
        _ => vue27_expression_references_setup_local_with_scope(
            argument.to_expression(),
            setup_bindings,
            scope,
        ),
    }
}

fn vue27_property_key_references_setup_local(
    key: &PropertyKey<'_>,
    setup_bindings: &BTreeSet<String>,
    scope: &mut BTreeSet<String>,
) -> bool {
    match key {
        PropertyKey::StaticIdentifier(_) | PropertyKey::PrivateIdentifier(_) => false,
        _ => vue27_expression_references_setup_local_with_scope(
            key.to_expression(),
            setup_bindings,
            scope,
        ),
    }
}

fn vue27_function_references_setup_local(
    function: &Function<'_>,
    setup_bindings: &BTreeSet<String>,
    scope: &mut BTreeSet<String>,
) -> bool {
    let mut function_scope = scope.clone();
    if let Some(id) = &function.id {
        function_scope.insert(id.name.to_string());
    }
    insert_formal_parameter_bindings(&function.params, &mut function_scope);
    function.params.items.iter().any(|param| {
        param.initializer.as_ref().is_some_and(|initializer| {
            vue27_expression_references_setup_local_with_scope(initializer, setup_bindings, scope)
        })
    }) || function.body.as_ref().is_some_and(|body| {
        body.statements.iter().any(|statement| {
            vue27_statement_references_setup_local(statement, setup_bindings, &mut function_scope)
        })
    })
}

fn vue27_arrow_function_references_setup_local(
    function: &ArrowFunctionExpression<'_>,
    setup_bindings: &BTreeSet<String>,
    scope: &mut BTreeSet<String>,
) -> bool {
    let mut function_scope = scope.clone();
    insert_formal_parameter_bindings(&function.params, &mut function_scope);
    function.params.items.iter().any(|param| {
        param.initializer.as_ref().is_some_and(|initializer| {
            vue27_expression_references_setup_local_with_scope(initializer, setup_bindings, scope)
        })
    }) || function.body.statements.iter().any(|statement| {
        vue27_statement_references_setup_local(statement, setup_bindings, &mut function_scope)
    })
}

fn vue27_statement_references_setup_local(
    statement: &Statement<'_>,
    setup_bindings: &BTreeSet<String>,
    scope: &mut BTreeSet<String>,
) -> bool {
    match statement {
        Statement::BlockStatement(block) => {
            let mut block_scope = scope.clone();
            insert_vue27_block_declarations(&block.body, &mut block_scope);
            block.body.iter().any(|statement| {
                vue27_statement_references_setup_local(statement, setup_bindings, &mut block_scope)
            })
        }
        Statement::ExpressionStatement(statement) => {
            vue27_expression_references_setup_local_with_scope(
                &statement.expression,
                setup_bindings,
                scope,
            )
        }
        Statement::ReturnStatement(statement) => {
            statement.argument.as_ref().is_some_and(|argument| {
                vue27_expression_references_setup_local_with_scope(argument, setup_bindings, scope)
            })
        }
        Statement::VariableDeclaration(declaration) => {
            declaration.declarations.iter().any(|declarator| {
                declarator.init.as_ref().is_some_and(|init| {
                    vue27_expression_references_setup_local_with_scope(init, setup_bindings, scope)
                })
            })
        }
        Statement::FunctionDeclaration(function) => {
            vue27_function_references_setup_local(function, setup_bindings, scope)
        }
        Statement::IfStatement(statement) => {
            vue27_expression_references_setup_local_with_scope(
                &statement.test,
                setup_bindings,
                scope,
            ) || vue27_statement_references_setup_local(
                &statement.consequent,
                setup_bindings,
                scope,
            ) || statement.alternate.as_ref().is_some_and(|alternate| {
                vue27_statement_references_setup_local(alternate, setup_bindings, scope)
            })
        }
        Statement::ForStatement(statement) => {
            let init_refs = statement.init.as_ref().is_some_and(|init| match init {
                oxc_ast::ast::ForStatementInit::VariableDeclaration(declaration) => {
                    declaration.declarations.iter().any(|declarator| {
                        declarator.init.as_ref().is_some_and(|init| {
                            vue27_expression_references_setup_local_with_scope(
                                init,
                                setup_bindings,
                                scope,
                            )
                        })
                    })
                }
                _ => init.as_expression().is_some_and(|expression| {
                    vue27_expression_references_setup_local_with_scope(
                        expression,
                        setup_bindings,
                        scope,
                    )
                }),
            });
            init_refs
                || statement.test.as_ref().is_some_and(|test| {
                    vue27_expression_references_setup_local_with_scope(test, setup_bindings, scope)
                })
                || statement.update.as_ref().is_some_and(|update| {
                    vue27_expression_references_setup_local_with_scope(
                        update,
                        setup_bindings,
                        scope,
                    )
                })
                || vue27_statement_references_setup_local(&statement.body, setup_bindings, scope)
        }
        Statement::ForInStatement(statement) => {
            vue27_expression_references_setup_local_with_scope(
                &statement.right,
                setup_bindings,
                scope,
            ) || vue27_statement_references_setup_local(&statement.body, setup_bindings, scope)
        }
        Statement::ForOfStatement(statement) => {
            vue27_expression_references_setup_local_with_scope(
                &statement.right,
                setup_bindings,
                scope,
            ) || vue27_statement_references_setup_local(&statement.body, setup_bindings, scope)
        }
        Statement::WhileStatement(statement) => {
            vue27_expression_references_setup_local_with_scope(
                &statement.test,
                setup_bindings,
                scope,
            ) || vue27_statement_references_setup_local(&statement.body, setup_bindings, scope)
        }
        Statement::DoWhileStatement(statement) => {
            vue27_statement_references_setup_local(&statement.body, setup_bindings, scope)
                || vue27_expression_references_setup_local_with_scope(
                    &statement.test,
                    setup_bindings,
                    scope,
                )
        }
        Statement::SwitchStatement(statement) => {
            vue27_expression_references_setup_local_with_scope(
                &statement.discriminant,
                setup_bindings,
                scope,
            ) || statement.cases.iter().any(|case| {
                case.test.as_ref().is_some_and(|test| {
                    vue27_expression_references_setup_local_with_scope(test, setup_bindings, scope)
                }) || case.consequent.iter().any(|statement| {
                    vue27_statement_references_setup_local(statement, setup_bindings, scope)
                })
            })
        }
        Statement::ThrowStatement(statement) => vue27_expression_references_setup_local_with_scope(
            &statement.argument,
            setup_bindings,
            scope,
        ),
        Statement::TryStatement(statement) => {
            statement.block.body.iter().any(|statement| {
                vue27_statement_references_setup_local(statement, setup_bindings, scope)
            }) || statement.handler.as_ref().is_some_and(|handler| {
                handler.body.body.iter().any(|statement| {
                    vue27_statement_references_setup_local(statement, setup_bindings, scope)
                })
            }) || statement.finalizer.as_ref().is_some_and(|finalizer| {
                finalizer.body.iter().any(|statement| {
                    vue27_statement_references_setup_local(statement, setup_bindings, scope)
                })
            })
        }
        Statement::WithStatement(statement) => {
            vue27_expression_references_setup_local_with_scope(
                &statement.object,
                setup_bindings,
                scope,
            ) || vue27_statement_references_setup_local(&statement.body, setup_bindings, scope)
        }
        Statement::LabeledStatement(statement) => {
            vue27_statement_references_setup_local(&statement.body, setup_bindings, scope)
        }
        _ => false,
    }
}

fn vue27_assignment_target_references_setup_local(
    target: &AssignmentTarget<'_>,
    setup_bindings: &BTreeSet<String>,
    scope: &mut BTreeSet<String>,
) -> bool {
    match target {
        AssignmentTarget::AssignmentTargetIdentifier(identifier) => {
            setup_bindings.contains(identifier.name.as_str())
                && !scope.contains(identifier.name.as_str())
        }
        AssignmentTarget::StaticMemberExpression(member) => {
            vue27_expression_references_setup_local_with_scope(
                &member.object,
                setup_bindings,
                scope,
            )
        }
        AssignmentTarget::ComputedMemberExpression(member) => {
            vue27_expression_references_setup_local_with_scope(
                &member.object,
                setup_bindings,
                scope,
            ) || vue27_expression_references_setup_local_with_scope(
                &member.expression,
                setup_bindings,
                scope,
            )
        }
        AssignmentTarget::PrivateFieldExpression(member) => {
            vue27_expression_references_setup_local_with_scope(
                &member.object,
                setup_bindings,
                scope,
            )
        }
        AssignmentTarget::TSAsExpression(expression) => {
            vue27_expression_references_setup_local_with_scope(
                &expression.expression,
                setup_bindings,
                scope,
            )
        }
        AssignmentTarget::TSSatisfiesExpression(expression) => {
            vue27_expression_references_setup_local_with_scope(
                &expression.expression,
                setup_bindings,
                scope,
            )
        }
        AssignmentTarget::TSNonNullExpression(expression) => {
            vue27_expression_references_setup_local_with_scope(
                &expression.expression,
                setup_bindings,
                scope,
            )
        }
        AssignmentTarget::TSTypeAssertion(expression) => {
            vue27_expression_references_setup_local_with_scope(
                &expression.expression,
                setup_bindings,
                scope,
            )
        }
        AssignmentTarget::ArrayAssignmentTarget(target) => {
            target.elements.iter().any(|element| {
                element.as_ref().is_some_and(|element| {
                    vue27_assignment_target_maybe_default_references_setup_local(
                        element,
                        setup_bindings,
                        scope,
                    )
                })
            }) || target.rest.as_ref().is_some_and(|rest| {
                vue27_assignment_target_references_setup_local(&rest.target, setup_bindings, scope)
            })
        }
        AssignmentTarget::ObjectAssignmentTarget(target) => {
            target.properties.iter().any(|property| match property {
                oxc_ast::ast::AssignmentTargetProperty::AssignmentTargetPropertyIdentifier(
                    property,
                ) => {
                    (setup_bindings.contains(property.binding.name.as_str())
                        && !scope.contains(property.binding.name.as_str()))
                        || property.init.as_ref().is_some_and(|init| {
                            vue27_expression_references_setup_local_with_scope(
                                init,
                                setup_bindings,
                                scope,
                            )
                        })
                }
                oxc_ast::ast::AssignmentTargetProperty::AssignmentTargetPropertyProperty(
                    property,
                ) => {
                    (property.computed
                        && vue27_property_key_references_setup_local(
                            &property.name,
                            setup_bindings,
                            scope,
                        ))
                        || vue27_assignment_target_maybe_default_references_setup_local(
                            &property.binding,
                            setup_bindings,
                            scope,
                        )
                }
            }) || target.rest.as_ref().is_some_and(|rest| {
                vue27_assignment_target_references_setup_local(&rest.target, setup_bindings, scope)
            })
        }
    }
}

fn vue27_assignment_target_maybe_default_references_setup_local(
    target: &oxc_ast::ast::AssignmentTargetMaybeDefault<'_>,
    setup_bindings: &BTreeSet<String>,
    scope: &mut BTreeSet<String>,
) -> bool {
    match target {
        oxc_ast::ast::AssignmentTargetMaybeDefault::AssignmentTargetWithDefault(target) => {
            vue27_assignment_target_references_setup_local(&target.binding, setup_bindings, scope)
                || vue27_expression_references_setup_local_with_scope(
                    &target.init,
                    setup_bindings,
                    scope,
                )
        }
        _ => target.as_assignment_target().is_some_and(|target| {
            vue27_assignment_target_references_setup_local(target, setup_bindings, scope)
        }),
    }
}

fn vue27_simple_assignment_target_references_setup_local(
    target: &SimpleAssignmentTarget<'_>,
    setup_bindings: &BTreeSet<String>,
    scope: &mut BTreeSet<String>,
) -> bool {
    match target {
        SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) => {
            setup_bindings.contains(identifier.name.as_str())
                && !scope.contains(identifier.name.as_str())
        }
        SimpleAssignmentTarget::StaticMemberExpression(member) => {
            vue27_expression_references_setup_local_with_scope(
                &member.object,
                setup_bindings,
                scope,
            )
        }
        SimpleAssignmentTarget::ComputedMemberExpression(member) => {
            vue27_expression_references_setup_local_with_scope(
                &member.object,
                setup_bindings,
                scope,
            ) || vue27_expression_references_setup_local_with_scope(
                &member.expression,
                setup_bindings,
                scope,
            )
        }
        SimpleAssignmentTarget::PrivateFieldExpression(member) => {
            vue27_expression_references_setup_local_with_scope(
                &member.object,
                setup_bindings,
                scope,
            )
        }
        SimpleAssignmentTarget::TSAsExpression(expression) => {
            vue27_expression_references_setup_local_with_scope(
                &expression.expression,
                setup_bindings,
                scope,
            )
        }
        SimpleAssignmentTarget::TSSatisfiesExpression(expression) => {
            vue27_expression_references_setup_local_with_scope(
                &expression.expression,
                setup_bindings,
                scope,
            )
        }
        SimpleAssignmentTarget::TSNonNullExpression(expression) => {
            vue27_expression_references_setup_local_with_scope(
                &expression.expression,
                setup_bindings,
                scope,
            )
        }
        SimpleAssignmentTarget::TSTypeAssertion(expression) => {
            vue27_expression_references_setup_local_with_scope(
                &expression.expression,
                setup_bindings,
                scope,
            )
        }
    }
}

fn collect_vue27_argument_identifier_usage(argument: &Argument<'_>, value: &mut String) {
    match argument {
        Argument::SpreadElement(spread) => {
            collect_vue27_expression_identifier_usage(&spread.argument, value);
        }
        _ => collect_vue27_expression_identifier_usage(argument.to_expression(), value),
    }
}

fn collect_vue27_property_key_identifier_usage(key: &PropertyKey<'_>, value: &mut String) {
    match key {
        PropertyKey::StaticIdentifier(_) | PropertyKey::PrivateIdentifier(_) => {}
        _ => collect_vue27_expression_identifier_usage(key.to_expression(), value),
    }
}

fn collect_vue27_function_identifier_usage(function: &Function<'_>, value: &mut String) {
    for param in &function.params.items {
        if let Some(initializer) = &param.initializer {
            collect_vue27_expression_identifier_usage(initializer, value);
        }
    }
    if let Some(body) = &function.body {
        for statement in &body.statements {
            collect_vue27_statement_identifier_usage(statement, value);
        }
    }
}

fn collect_vue27_arrow_function_identifier_usage(
    function: &ArrowFunctionExpression<'_>,
    value: &mut String,
) {
    for param in &function.params.items {
        if let Some(initializer) = &param.initializer {
            collect_vue27_expression_identifier_usage(initializer, value);
        }
    }
    for statement in &function.body.statements {
        collect_vue27_statement_identifier_usage(statement, value);
    }
}

fn collect_vue27_assignment_target_identifier_usage(
    target: &AssignmentTarget<'_>,
    value: &mut String,
) {
    match target {
        AssignmentTarget::AssignmentTargetIdentifier(identifier) => {
            push_vue27_identifier_usage(value, identifier.name.as_str());
        }
        AssignmentTarget::StaticMemberExpression(member) => {
            collect_vue27_expression_identifier_usage(&member.object, value);
        }
        AssignmentTarget::ComputedMemberExpression(member) => {
            collect_vue27_expression_identifier_usage(&member.object, value);
            collect_vue27_expression_identifier_usage(&member.expression, value);
        }
        AssignmentTarget::PrivateFieldExpression(member) => {
            collect_vue27_expression_identifier_usage(&member.object, value);
        }
        AssignmentTarget::TSAsExpression(expression) => {
            collect_vue27_expression_identifier_usage(&expression.expression, value);
        }
        AssignmentTarget::TSSatisfiesExpression(expression) => {
            collect_vue27_expression_identifier_usage(&expression.expression, value);
        }
        AssignmentTarget::TSNonNullExpression(expression) => {
            collect_vue27_expression_identifier_usage(&expression.expression, value);
        }
        AssignmentTarget::TSTypeAssertion(expression) => {
            collect_vue27_expression_identifier_usage(&expression.expression, value);
        }
        AssignmentTarget::ArrayAssignmentTarget(target) => {
            for element in target.elements.iter().flatten() {
                collect_vue27_assignment_target_maybe_default_identifier_usage(element, value);
            }
            if let Some(rest) = &target.rest {
                collect_vue27_assignment_target_identifier_usage(&rest.target, value);
            }
        }
        AssignmentTarget::ObjectAssignmentTarget(target) => {
            for property in &target.properties {
                match property {
                    oxc_ast::ast::AssignmentTargetProperty::AssignmentTargetPropertyIdentifier(
                        property,
                    ) => {
                        push_vue27_identifier_usage(value, property.binding.name.as_str());
                        if let Some(init) = &property.init {
                            collect_vue27_expression_identifier_usage(init, value);
                        }
                    }
                    oxc_ast::ast::AssignmentTargetProperty::AssignmentTargetPropertyProperty(
                        property,
                    ) => {
                        if property.computed {
                            collect_vue27_property_key_identifier_usage(&property.name, value);
                        }
                        collect_vue27_assignment_target_maybe_default_identifier_usage(
                            &property.binding,
                            value,
                        );
                    }
                }
            }
            if let Some(rest) = &target.rest {
                collect_vue27_assignment_target_identifier_usage(&rest.target, value);
            }
        }
    }
}

fn collect_vue27_assignment_target_maybe_default_identifier_usage(
    target: &oxc_ast::ast::AssignmentTargetMaybeDefault<'_>,
    value: &mut String,
) {
    match target {
        oxc_ast::ast::AssignmentTargetMaybeDefault::AssignmentTargetWithDefault(target) => {
            collect_vue27_assignment_target_identifier_usage(&target.binding, value);
            collect_vue27_expression_identifier_usage(&target.init, value);
        }
        _ => {
            if let Some(target) = target.as_assignment_target() {
                collect_vue27_assignment_target_identifier_usage(target, value);
            }
        }
    }
}

fn collect_vue27_simple_assignment_target_identifier_usage(
    target: &SimpleAssignmentTarget<'_>,
    value: &mut String,
) {
    match target {
        SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) => {
            push_vue27_identifier_usage(value, identifier.name.as_str());
        }
        SimpleAssignmentTarget::StaticMemberExpression(member) => {
            collect_vue27_expression_identifier_usage(&member.object, value);
        }
        SimpleAssignmentTarget::ComputedMemberExpression(member) => {
            collect_vue27_expression_identifier_usage(&member.object, value);
            collect_vue27_expression_identifier_usage(&member.expression, value);
        }
        SimpleAssignmentTarget::PrivateFieldExpression(member) => {
            collect_vue27_expression_identifier_usage(&member.object, value);
        }
        SimpleAssignmentTarget::TSAsExpression(expression) => {
            collect_vue27_expression_identifier_usage(&expression.expression, value);
        }
        SimpleAssignmentTarget::TSSatisfiesExpression(expression) => {
            collect_vue27_expression_identifier_usage(&expression.expression, value);
        }
        SimpleAssignmentTarget::TSNonNullExpression(expression) => {
            collect_vue27_expression_identifier_usage(&expression.expression, value);
        }
        SimpleAssignmentTarget::TSTypeAssertion(expression) => {
            collect_vue27_expression_identifier_usage(&expression.expression, value);
        }
    }
}

fn push_vue27_identifier_usage(value: &mut String, name: &str) {
    value.push(',');
    value.push_str(name);
}

fn vue27_strip_template_expression_strings(exp: &str) -> String {
    let mut output = String::new();
    let mut chars = exp.char_indices().peekable();
    while let Some((_, ch)) = chars.next() {
        match ch {
            '\'' | '"' => {
                while let Some((_, inner)) = chars.next() {
                    if inner == '\\' {
                        let _ = chars.next();
                    } else if inner == ch {
                        break;
                    }
                }
            }
            '`' => {
                let mut template_expr = String::new();
                while let Some((_, inner)) = chars.next() {
                    if inner == '\\' {
                        let _ = chars.next();
                    } else if inner == '`' {
                        break;
                    } else if inner == '$' && chars.peek().is_some_and(|(_, next)| *next == '{') {
                        let _ = chars.next();
                        let mut depth = 1usize;
                        while let Some((_, expr_ch)) = chars.next() {
                            if expr_ch == '{' {
                                depth += 1;
                                template_expr.push(expr_ch);
                            } else if expr_ch == '}' {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                }
                                template_expr.push(expr_ch);
                            } else {
                                template_expr.push(expr_ch);
                            }
                        }
                        template_expr.push(',');
                    }
                }
                output.push_str(&template_expr);
            }
            _ => output.push(ch),
        }
    }
    output
}

fn identifier_usage_contains(usage: &str, local: &str) -> bool {
    if local.is_empty() {
        return false;
    }
    let mut search_start = 0usize;
    while let Some(index) = usage[search_start..].find(local) {
        let start = search_start + index;
        let end = start + local.len();
        let before = usage[..start].chars().next_back();
        let after = usage[end..].chars().next();
        if !before.is_some_and(is_identifier_usage_char)
            && !after.is_some_and(is_identifier_usage_char)
        {
            return true;
        }
        search_start = end;
    }
    false
}

fn is_identifier_usage_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '$')
}

fn vue27_camelize(value: &str) -> String {
    let mut output = String::new();
    let mut uppercase_next = false;
    for ch in value.chars() {
        if ch == '-' {
            uppercase_next = true;
        } else if uppercase_next {
            output.extend(ch.to_uppercase());
            uppercase_next = false;
        } else {
            output.push(ch);
        }
    }
    output
}

fn vue27_capitalize(value: &str) -> String {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    first.to_uppercase().chain(chars).collect()
}

fn split_vue27_setup_module_content(content: &str) -> (String, String) {
    let mut module = String::new();
    let mut setup = String::new();
    let mut last_module_indent = "";
    for line in split_inclusive_lines(content) {
        let line_without_newline = line.trim_end_matches(['\n', '\r']);
        let trimmed = line_without_newline.trim_start();
        if trimmed.starts_with("import ") {
            if !module.is_empty() && !module.ends_with('\n') {
                module.push('\n');
            }
            if module.is_empty() {
                module.push_str(trimmed);
            } else {
                module.push_str(line_without_newline);
            }
            module.push('\n');
            last_module_indent =
                &line_without_newline[..line_without_newline.len() - trimmed.len()];
        } else {
            setup.push_str(line);
        }
    }
    if !last_module_indent.is_empty() {
        module.push_str(last_module_indent);
    }
    (module, setup)
}

fn split_inclusive_lines(value: &str) -> Vec<&str> {
    if value.is_empty() {
        return Vec::new();
    }
    let mut lines = value.split_inclusive('\n').collect::<Vec<_>>();
    if value.ends_with("\n\n") {
        lines.push("");
    }
    lines
}

fn leading_blank_line_indent(value: &str) -> Option<&str> {
    let line_end = value.find('\n').unwrap_or(value.len());
    let first_line = &value[..line_end];
    if first_line.is_empty() || first_line.trim().is_empty() {
        Some(first_line)
    } else {
        None
    }
}

fn vue27_setup_binding_metadata(descriptor: &SfcDescriptor) -> BTreeMap<String, String> {
    let mut bindings = descriptor
        .script_setup
        .as_ref()
        .map(|script_setup| {
            let setup_context = vue27_script_setup_context(descriptor);
            analyze_vue27_script_setup(script_setup, false, &setup_context)
        })
        .map(|analysis| {
            let mut bindings = vue27_script_setup_script_bindings(descriptor);
            bindings.extend(analysis.setup_bindings);
            for prop in analysis.props_bindings {
                bindings.insert(prop, "props".into());
            }
            bindings
        })
        .unwrap_or_default();
    bindings.insert("__isScriptSetup".into(), "true".into());
    bindings
}

fn vue27_normal_script_binding_metadata(descriptor: &SfcDescriptor) -> BTreeMap<String, String> {
    let mut bindings = vue27_script_options_binding_metadata(descriptor);
    bindings.insert("__isScriptSetup".into(), "false".into());
    bindings
}

fn vue27_script_options_binding_metadata(descriptor: &SfcDescriptor) -> BTreeMap<String, String> {
    let Some(script) = descriptor.script.as_ref() else {
        return BTreeMap::new();
    };
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        &script.content,
        script_source_type_from_attrs(&script.attrs),
    )
    .with_options(oxc_parser::ParseOptions {
        parse_regular_expression: true,
        ..oxc_parser::ParseOptions::default()
    })
    .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return BTreeMap::new();
    }
    let mut bindings = BTreeMap::new();
    for statement in &parsed.program.body {
        if let Statement::ExportDefaultDeclaration(default) = statement {
            match &default.declaration {
                ExportDefaultDeclarationKind::ObjectExpression(object) => {
                    analyze_vue27_options_bindings(object, &mut bindings);
                }
                ExportDefaultDeclarationKind::CallExpression(call) => {
                    if let Some(argument) = call.arguments.first() {
                        if let Expression::ObjectExpression(object) = argument.to_expression() {
                            analyze_vue27_options_bindings(object, &mut bindings);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    bindings
}

fn vue27_script_setup_script_bindings(descriptor: &SfcDescriptor) -> BTreeMap<String, String> {
    let Some(script) = descriptor.script.as_ref() else {
        return BTreeMap::new();
    };
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        &script.content,
        script_source_type_from_attrs(&script.attrs),
    )
    .with_options(oxc_parser::ParseOptions {
        parse_regular_expression: true,
        ..oxc_parser::ParseOptions::default()
    })
    .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return BTreeMap::new();
    }
    let mut bindings = BTreeMap::new();
    for statement in &parsed.program.body {
        collect_vue27_top_level_script_binding(statement, &mut bindings);
    }
    bindings
}

fn vue27_script_setup_script_return_bindings(
    descriptor: &SfcDescriptor,
) -> Vue27ScriptReturnBindings {
    let Some(script) = descriptor.script.as_ref() else {
        return Vue27ScriptReturnBindings::default();
    };
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        &script.content,
        script_source_type_from_attrs(&script.attrs),
    )
    .with_options(oxc_parser::ParseOptions {
        parse_regular_expression: true,
        ..oxc_parser::ParseOptions::default()
    })
    .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return Vue27ScriptReturnBindings::default();
    }
    let mut result = Vue27ScriptReturnBindings::default();
    for statement in &parsed.program.body {
        collect_vue27_top_level_script_return_binding(statement, &mut result);
    }
    result
}

fn collect_vue27_top_level_script_return_binding(
    statement: &Statement<'_>,
    result: &mut Vue27ScriptReturnBindings,
) {
    match statement {
        Statement::ImportDeclaration(import) => {
            collect_vue27_import_return_bindings(import, &mut result.imports);
        }
        Statement::VariableDeclaration(declaration) if !declaration.declare => {
            collect_pattern_return_bindings_from_declaration(declaration, &mut result.bindings);
        }
        Statement::FunctionDeclaration(function) if !function.declare => {
            if let Some(id) = &function.id {
                push_unique(&mut result.bindings, id.name.as_str());
            }
        }
        Statement::ClassDeclaration(class) if !class.declare => {
            if let Some(id) = &class.id {
                push_unique(&mut result.bindings, id.name.as_str());
            }
        }
        Statement::TSEnumDeclaration(declaration) if !declaration.declare => {
            push_unique(&mut result.bindings, declaration.id.name.as_str());
        }
        Statement::ExportNamedDeclaration(declaration)
            if declaration.export_kind == ImportOrExportKind::Value =>
        {
            if let Some(declaration) = &declaration.declaration {
                collect_vue27_declaration_return_bindings(declaration, &mut result.bindings);
            }
        }
        _ => {}
    }
}

fn collect_vue27_import_return_bindings(
    import: &oxc_ast::ast::ImportDeclaration<'_>,
    imports: &mut Vec<Vue27ScriptImport>,
) {
    let Some(specifiers) = &import.specifiers else {
        return;
    };
    let source = import.source.value.as_str();
    for specifier in specifiers {
        imports.push(Vue27ScriptImport {
            local: import_specifier_local(specifier),
            source: source.to_string(),
            imported: import_specifier_imported(specifier).unwrap_or_else(|| "default".into()),
            is_type: vue27_import_specifier_is_type(import, specifier),
        });
    }
}

fn collect_vue27_declaration_return_bindings(
    declaration: &Declaration<'_>,
    bindings: &mut Vec<String>,
) {
    match declaration {
        Declaration::VariableDeclaration(declaration) if !declaration.declare => {
            collect_pattern_return_bindings_from_declaration(declaration, bindings);
        }
        Declaration::FunctionDeclaration(function) if !function.declare => {
            if let Some(id) = &function.id {
                push_unique(bindings, id.name.as_str());
            }
        }
        Declaration::ClassDeclaration(class) if !class.declare => {
            if let Some(id) = &class.id {
                push_unique(bindings, id.name.as_str());
            }
        }
        Declaration::TSEnumDeclaration(declaration) if !declaration.declare => {
            push_unique(bindings, declaration.id.name.as_str());
        }
        _ => {}
    }
}

fn collect_vue27_top_level_script_binding(
    statement: &Statement<'_>,
    bindings: &mut BTreeMap<String, String>,
) {
    match statement {
        Statement::ImportDeclaration(import) => {
            let source = import.source.value.as_str();
            if let Some(specifiers) = &import.specifiers {
                for specifier in specifiers {
                    let local = import_specifier_local(specifier);
                    let imported = import_specifier_imported(specifier);
                    let binding_type = if matches!(imported.as_deref(), Some("*"))
                        || (matches!(imported.as_deref(), Some("default"))
                            && source.ends_with(".vue"))
                        || source == "vue"
                    {
                        "setup-const"
                    } else {
                        "setup-maybe-ref"
                    };
                    bindings.insert(local, binding_type.into());
                }
            }
        }
        Statement::VariableDeclaration(declaration) if !declaration.declare => {
            collect_vue27_script_declaration_bindings(declaration, bindings);
        }
        Statement::FunctionDeclaration(function) if !function.declare => {
            if let Some(id) = &function.id {
                bindings.insert(id.name.to_string(), "setup-const".into());
            }
        }
        Statement::ClassDeclaration(class) if !class.declare => {
            if let Some(id) = &class.id {
                bindings.insert(id.name.to_string(), "setup-const".into());
            }
        }
        Statement::TSEnumDeclaration(declaration) if !declaration.declare => {
            bindings.insert(declaration.id.name.to_string(), "setup-const".into());
        }
        Statement::ExportNamedDeclaration(declaration)
            if declaration.export_kind == ImportOrExportKind::Value =>
        {
            if let Some(declaration) = &declaration.declaration {
                match declaration {
                    oxc_ast::ast::Declaration::VariableDeclaration(declaration)
                        if !declaration.declare =>
                    {
                        collect_vue27_script_declaration_bindings(declaration, bindings);
                    }
                    oxc_ast::ast::Declaration::FunctionDeclaration(function)
                        if !function.declare =>
                    {
                        if let Some(id) = &function.id {
                            bindings.insert(id.name.to_string(), "setup-const".into());
                        }
                    }
                    oxc_ast::ast::Declaration::ClassDeclaration(class) if !class.declare => {
                        if let Some(id) = &class.id {
                            bindings.insert(id.name.to_string(), "setup-const".into());
                        }
                    }
                    oxc_ast::ast::Declaration::TSEnumDeclaration(declaration)
                        if !declaration.declare =>
                    {
                        bindings.insert(declaration.id.name.to_string(), "setup-const".into());
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
}

fn collect_vue27_script_declaration_bindings(
    declaration: &VariableDeclaration<'_>,
    bindings: &mut BTreeMap<String, String>,
) {
    let binding_type = if declaration.kind == VariableDeclarationKind::Const {
        "setup-const"
    } else {
        "setup-let"
    };
    for declarator in &declaration.declarations {
        collect_pattern_binding_types(&declarator.id, binding_type, bindings);
    }
}

fn collect_pattern_return_bindings_from_declaration(
    declaration: &VariableDeclaration<'_>,
    bindings: &mut Vec<String>,
) {
    for declarator in &declaration.declarations {
        collect_pattern_bindings(&declarator.id, bindings);
    }
}

fn analyze_vue27_options_bindings(
    object: &ObjectExpression<'_>,
    bindings: &mut BTreeMap<String, String>,
) {
    for property in &object.properties {
        let Some(property) = property.as_property() else {
            continue;
        };
        let Some(key) = property.key.static_name().map(|name| name.into_owned()) else {
            continue;
        };
        match key.as_str() {
            "props" => {
                if let Expression::ObjectExpression(props) = &property.value {
                    for key in object_expression_keys(props) {
                        bindings.insert(key, "props".into());
                    }
                } else if let Expression::ArrayExpression(array) = &property.value {
                    for element in &array.elements {
                        if let Some(Expression::StringLiteral(literal)) = element.as_expression() {
                            bindings.insert(literal.value.to_string(), "props".into());
                        }
                    }
                }
            }
            "computed" | "methods" => {
                if let Expression::ObjectExpression(values) = &property.value {
                    for key in object_expression_keys(values) {
                        bindings.insert(key, "options".into());
                    }
                }
            }
            "inject" => {
                collect_vue27_object_or_array_keys(&property.value, bindings, "options");
            }
            _ => {
                if let Expression::ObjectExpression(_) = &property.value {
                    continue;
                }
            }
        }
        if key == "setup" || key == "data" {
            collect_returned_object_keys(&property.value, key.as_str(), bindings);
        }
    }
}

fn collect_vue27_object_or_array_keys(
    expression: &Expression<'_>,
    bindings: &mut BTreeMap<String, String>,
    binding_type: &str,
) {
    match expression {
        Expression::ObjectExpression(object) => {
            for key in object_expression_keys(object) {
                bindings.insert(key, binding_type.to_string());
            }
        }
        Expression::ArrayExpression(array) => {
            for element in &array.elements {
                if let Some(Expression::StringLiteral(literal)) = element.as_expression() {
                    bindings.insert(literal.value.to_string(), binding_type.to_string());
                }
            }
        }
        _ => {}
    }
}

fn collect_returned_object_keys(
    expression: &Expression<'_>,
    option_key: &str,
    bindings: &mut BTreeMap<String, String>,
) {
    let body = match expression {
        Expression::FunctionExpression(function) => {
            function.body.as_ref().map(|body| &body.statements)
        }
        Expression::ArrowFunctionExpression(function) => Some(&function.body.statements),
        _ => None,
    };
    let Some(body) = body else {
        return;
    };
    for statement in body {
        if let Statement::ReturnStatement(statement) = statement {
            if let Some(Expression::ObjectExpression(object)) = &statement.argument {
                for key in object_expression_keys(object) {
                    bindings.insert(
                        key,
                        if option_key == "setup" {
                            "setup-maybe-ref".into()
                        } else {
                            "data".into()
                        },
                    );
                }
            }
        }
    }
}

fn collect_pattern_binding_types(
    pattern: &BindingPattern<'_>,
    binding_type: &str,
    bindings: &mut BTreeMap<String, String>,
) {
    match pattern {
        BindingPattern::BindingIdentifier(identifier) => {
            bindings.insert(identifier.name.to_string(), binding_type.to_string());
        }
        BindingPattern::ObjectPattern(pattern) => {
            for property in &pattern.properties {
                collect_pattern_binding_types(&property.value, binding_type, bindings);
            }
            if let Some(rest) = &pattern.rest {
                collect_pattern_binding_types(&rest.argument, binding_type, bindings);
            }
        }
        BindingPattern::ArrayPattern(pattern) => {
            for element in pattern.elements.iter().flatten() {
                collect_pattern_binding_types(element, binding_type, bindings);
            }
            if let Some(rest) = &pattern.rest {
                collect_pattern_binding_types(&rest.argument, binding_type, bindings);
            }
        }
        BindingPattern::AssignmentPattern(pattern) => {
            collect_pattern_binding_types(&pattern.left, binding_type, bindings);
        }
    }
}

fn insert_pattern_bindings(pattern: &BindingPattern<'_>, bindings: &mut BTreeSet<String>) {
    match pattern {
        BindingPattern::BindingIdentifier(identifier) => {
            bindings.insert(identifier.name.to_string());
        }
        BindingPattern::ObjectPattern(pattern) => {
            for property in &pattern.properties {
                insert_pattern_bindings(&property.value, bindings);
            }
            if let Some(rest) = &pattern.rest {
                insert_pattern_bindings(&rest.argument, bindings);
            }
        }
        BindingPattern::ArrayPattern(pattern) => {
            for element in pattern.elements.iter().flatten() {
                insert_pattern_bindings(element, bindings);
            }
            if let Some(rest) = &pattern.rest {
                insert_pattern_bindings(&rest.argument, bindings);
            }
        }
        BindingPattern::AssignmentPattern(pattern) => {
            insert_pattern_bindings(&pattern.left, bindings);
        }
    }
}

fn insert_formal_parameter_bindings(
    params: &oxc_ast::ast::FormalParameters<'_>,
    bindings: &mut BTreeSet<String>,
) {
    for param in &params.items {
        insert_pattern_bindings(&param.pattern, bindings);
    }
    if let Some(rest) = &params.rest {
        insert_pattern_bindings(&rest.rest.argument, bindings);
    }
}

fn insert_vue27_block_declarations(statements: &[Statement<'_>], bindings: &mut BTreeSet<String>) {
    for statement in statements {
        match statement {
            Statement::VariableDeclaration(declaration) if !declaration.declare => {
                for declarator in &declaration.declarations {
                    insert_pattern_bindings(&declarator.id, bindings);
                }
            }
            Statement::FunctionDeclaration(function) if !function.declare => {
                if let Some(id) = &function.id {
                    bindings.insert(id.name.to_string());
                }
            }
            Statement::ClassDeclaration(class) if !class.declare => {
                if let Some(id) = &class.id {
                    bindings.insert(id.name.to_string());
                }
            }
            _ => {}
        }
    }
}

fn collect_pattern_bindings(pattern: &BindingPattern<'_>, bindings: &mut Vec<String>) {
    match pattern {
        BindingPattern::BindingIdentifier(identifier) => {
            push_unique(bindings, identifier.name.as_str());
        }
        BindingPattern::ObjectPattern(pattern) => {
            for property in &pattern.properties {
                collect_pattern_bindings(&property.value, bindings);
            }
            if let Some(rest) = &pattern.rest {
                collect_pattern_bindings(&rest.argument, bindings);
            }
        }
        BindingPattern::ArrayPattern(pattern) => {
            for element in pattern.elements.iter().flatten() {
                collect_pattern_bindings(element, bindings);
            }
            if let Some(rest) = &pattern.rest {
                collect_pattern_bindings(&rest.argument, bindings);
            }
        }
        BindingPattern::AssignmentPattern(pattern) => {
            collect_pattern_bindings(&pattern.left, bindings);
        }
    }
}

fn push_unique(values: &mut Vec<String>, value: &str) {
    if !values.iter().any(|existing| existing == value) {
        values.push(value.to_string());
    }
}

fn trim_trailing_blank_lines(value: &str) -> &str {
    value.trim_end_matches(|ch| matches!(ch, '\n' | '\r'))
}

fn script_is_typescript(attrs: &SfcBlockAttrs) -> bool {
    matches!(attrs.lang.as_deref(), Some("ts" | "tsx"))
}

fn merge_template_errors(
    mut first: Vec<SfcTemplateError>,
    second: Vec<SfcTemplateError>,
) -> Vec<SfcTemplateError> {
    for error in second {
        if !first.iter().any(|existing| {
            existing.code == error.code
                && existing.loc.start.offset == error.loc.start.offset
                && existing.loc.end.offset == error.loc.end.offset
        }) {
            first.push(error);
        }
    }
    first
}

fn sfc_template_errors_from_diagnostics(
    diagnostics: &[Diagnostic],
    source: &str,
) -> Vec<SfcTemplateError> {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .filter_map(|diagnostic| sfc_template_error_from_diagnostic(diagnostic, source))
        .collect()
}

fn sfc_template_error_from_diagnostic(
    diagnostic: &Diagnostic,
    source: &str,
) -> Option<SfcTemplateError> {
    let span = diagnostic.span?;
    let start = span.start.0.min(source.len());
    let end = span.end.0.min(source.len()).max(start);
    Some(SfcTemplateError {
        code: diagnostic.code.parse().unwrap_or(0),
        loc: SfcSourceLocation {
            start: position_at(source, start)?,
            end: position_at(source, end)?,
            source: source.get(start..end).unwrap_or_default().to_string(),
        },
    })
}

fn preprocess_vue27_template(
    source: &str,
    options: Vue27TemplatePreprocessOptions,
) -> Vue27TemplatePreprocessResult {
    let Some(lang) = options.lang.as_deref().filter(|lang| !lang.is_empty()) else {
        return Vue27TemplatePreprocessResult {
            source: source.to_string(),
            errors: Vec::new(),
            tips: Vec::new(),
        };
    };
    match lang.to_ascii_lowercase().as_str() {
        "html" => Vue27TemplatePreprocessResult {
            source: source.to_string(),
            errors: Vec::new(),
            tips: Vec::new(),
        },
        "pug" | "jade" => match compile_vue27_pug_template(source) {
            Ok(source) => Vue27TemplatePreprocessResult {
                source,
                errors: Vec::new(),
                tips: Vec::new(),
            },
            Err(error) => Vue27TemplatePreprocessResult {
                source: source.to_string(),
                errors: vec![error],
                tips: Vec::new(),
            },
        },
        _ => {
            let filename = options.filename.unwrap_or_else(|| "anonymous.vue".into());
            Vue27TemplatePreprocessResult {
                source: source.to_string(),
                tips: vec![format!(
                    "Component {filename} uses lang {lang} for template. Please install the language preprocessor."
                )],
                errors: vec![format!(
                    "Component {filename} uses lang {lang} for template, however it is not installed."
                )],
            }
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Vue27PugNode {
    tag: String,
    attrs: Vec<Vue27PugAttr>,
    text: Option<String>,
    children: Vec<Vue27PugNode>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Vue27PugAttr {
    name: String,
    value: Option<String>,
}

fn compile_vue27_pug_template(source: &str) -> Result<String, String> {
    let mut roots = Vec::new();
    let mut stack: Vec<(usize, Vec<usize>)> = Vec::new();
    for (line_index, line) in source.lines().enumerate() {
        let trimmed_end = line.trim_end();
        let content = trimmed_end.trim_start();
        if content.is_empty() || content.starts_with("//") {
            continue;
        }
        let indent = vue27_pug_indent(trimmed_end);
        let node = parse_vue27_pug_line(content).map_err(|error| {
            format!(
                "Pug template parse error on line {}: {error}",
                line_index + 1
            )
        })?;
        while stack
            .last()
            .is_some_and(|(parent_indent, _)| *parent_indent >= indent)
        {
            stack.pop();
        }
        let parent_path = stack
            .last()
            .map(|(_, path)| path.clone())
            .unwrap_or_default();
        let children = vue27_pug_children_at_path(&mut roots, &parent_path);
        let index = children.len();
        children.push(node);
        let mut path = parent_path;
        path.push(index);
        stack.push((indent, path));
    }
    Ok(render_vue27_pug_nodes(&roots))
}

fn vue27_pug_indent(line: &str) -> usize {
    line.chars()
        .take_while(|ch| *ch == ' ' || *ch == '\t')
        .map(|ch| if ch == '\t' { 2 } else { 1 })
        .sum()
}

fn vue27_pug_children_at_path<'a>(
    roots: &'a mut Vec<Vue27PugNode>,
    path: &[usize],
) -> &'a mut Vec<Vue27PugNode> {
    let mut current = roots;
    for &index in path {
        current = &mut current[index].children;
    }
    current
}

fn parse_vue27_pug_line(source: &str) -> Result<Vue27PugNode, String> {
    if let Some(text) = source.strip_prefix('|') {
        return Ok(Vue27PugNode {
            tag: "span".into(),
            text: Some(text.trim_start().to_string()),
            ..Vue27PugNode::default()
        });
    }
    let mut rest = source;
    let tag = if rest.starts_with('.') || rest.starts_with('#') {
        "div".to_string()
    } else {
        let (name, tail) = take_vue27_pug_name(rest);
        if name.is_empty() {
            return Err("expected tag name".into());
        }
        rest = tail;
        name.to_string()
    };
    let mut attrs = Vec::new();
    let mut shorthand_classes = Vec::new();
    let mut shorthand_id = None;
    loop {
        if let Some(tail) = rest.strip_prefix('.') {
            let (name, next) = take_vue27_pug_name(tail);
            if name.is_empty() {
                return Err("expected class name".into());
            }
            shorthand_classes.push(name.to_string());
            rest = next;
        } else if let Some(tail) = rest.strip_prefix('#') {
            let (name, next) = take_vue27_pug_name(tail);
            if name.is_empty() {
                return Err("expected id".into());
            }
            shorthand_id = Some(name.to_string());
            rest = next;
        } else {
            break;
        }
    }
    if rest.starts_with('(') {
        let (raw_attrs, tail) = take_vue27_pug_attrs(rest)?;
        attrs.extend(parse_vue27_pug_attrs(raw_attrs));
        rest = tail;
    }
    if let Some(id) = shorthand_id {
        if !attrs.iter().any(|attr| attr.name == "id") {
            attrs.push(Vue27PugAttr {
                name: "id".into(),
                value: Some(id),
            });
        }
    }
    if !shorthand_classes.is_empty() {
        let shorthand = shorthand_classes.join(" ");
        if let Some(class_attr) = attrs.iter_mut().find(|attr| attr.name == "class") {
            let existing = class_attr.value.get_or_insert_with(String::new);
            if existing.is_empty() {
                existing.push_str(&shorthand);
            } else {
                existing.push(' ');
                existing.push_str(&shorthand);
            }
        } else {
            attrs.push(Vue27PugAttr {
                name: "class".into(),
                value: Some(shorthand),
            });
        }
    }
    let text = rest.trim_start();
    Ok(Vue27PugNode {
        tag,
        attrs,
        text: (!text.is_empty()).then(|| text.to_string()),
        children: Vec::new(),
    })
}

fn take_vue27_pug_name(source: &str) -> (&str, &str) {
    let end = source
        .char_indices()
        .find_map(|(index, ch)| {
            (!(ch == '-' || ch == '_' || ch == ':' || ch.is_ascii_alphanumeric())).then_some(index)
        })
        .unwrap_or(source.len());
    (&source[..end], &source[end..])
}

fn take_vue27_pug_attrs(source: &str) -> Result<(&str, &str), String> {
    let mut depth = 0usize;
    let mut quote = None;
    for (index, ch) in source.char_indices() {
        if let Some(current_quote) = quote {
            if ch == current_quote {
                quote = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' => quote = Some(ch),
            '(' => depth += 1,
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Ok((&source[1..index], &source[index + ch.len_utf8()..]));
                }
            }
            _ => {}
        }
    }
    Err("missing closing attribute paren".into())
}

fn parse_vue27_pug_attrs(source: &str) -> Vec<Vue27PugAttr> {
    split_vue27_pug_attrs(source)
        .into_iter()
        .filter_map(|raw| {
            let raw = raw.trim();
            if raw.is_empty() {
                return None;
            }
            let Some((name, value)) = raw.split_once('=') else {
                return Some(Vue27PugAttr {
                    name: raw.to_string(),
                    value: None,
                });
            };
            Some(Vue27PugAttr {
                name: name.trim().to_string(),
                value: Some(trim_vue27_pug_attr_value(value.trim()).to_string()),
            })
        })
        .collect()
}

fn split_vue27_pug_attrs(source: &str) -> Vec<&str> {
    let mut attrs = Vec::new();
    let mut start = 0usize;
    let mut quote = None;
    for (index, ch) in source.char_indices() {
        if let Some(current_quote) = quote {
            if ch == current_quote {
                quote = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' => quote = Some(ch),
            ',' => {
                attrs.push(&source[start..index]);
                start = index + ch.len_utf8();
            }
            ch if ch.is_whitespace() => {
                let raw = &source[start..index];
                if raw.contains('=') {
                    attrs.push(raw);
                    start = index + ch.len_utf8();
                }
            }
            _ => {}
        }
    }
    if start <= source.len() {
        attrs.push(&source[start..]);
    }
    attrs
}

fn trim_vue27_pug_attr_value(source: &str) -> &str {
    source
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or_else(|| {
            source
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
        })
        .unwrap_or(source)
}

fn render_vue27_pug_nodes(nodes: &[Vue27PugNode]) -> String {
    nodes.iter().map(render_vue27_pug_node).collect()
}

fn render_vue27_pug_node(node: &Vue27PugNode) -> String {
    let mut output = String::new();
    output.push('<');
    output.push_str(&node.tag);
    for attr in &node.attrs {
        output.push(' ');
        output.push_str(&attr.name);
        if let Some(value) = attr.value.as_ref() {
            output.push_str("=\"");
            output.push_str(&escape_vue27_pug_attr(value));
            output.push('"');
        }
    }
    output.push('>');
    if let Some(text) = node.text.as_ref() {
        output.push_str(&escape_vue27_pug_text(text));
    }
    output.push_str(&render_vue27_pug_nodes(&node.children));
    output.push_str("</");
    output.push_str(&node.tag);
    output.push('>');
    output
}

fn escape_vue27_pug_text(source: &str) -> String {
    source
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_vue27_pug_attr(source: &str) -> String {
    source.replace('&', "&amp;").replace('"', "&quot;")
}

fn script_bindings(names: &[String]) -> BTreeMap<String, String> {
    names
        .iter()
        .filter(|name| !name.starts_with("import:") && !name.starts_with("export:"))
        .map(|name| (name.clone(), "literal-const".to_string()))
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct GeneratedScriptContent {
    content: String,
    errors: Vec<String>,
    bindings: BTreeMap<String, String>,
    removed_bindings: BTreeSet<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Vue3InlineTemplateRender {
    preamble: String,
    code: String,
    errors: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Vue3NormalScriptAnalysis {
    module_content: String,
    has_default_export: bool,
    has_default_export_name: bool,
    errors: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Vue3ScriptSetupAnalysis {
    module_content: String,
    setup_content: String,
    return_bindings: Vec<String>,
    imports: Vec<Vue27ScriptImport>,
    setup_bindings: BTreeMap<String, String>,
    removed_bindings: BTreeSet<String>,
    options_runtime: Option<String>,
    has_define_props: bool,
    has_define_options: bool,
    props_bindings: Vec<String>,
    props_runtime: Option<String>,
    props_type_runtime: bool,
    needs_merge_defaults: bool,
    emits_runtime: Option<String>,
    emit_binding: Option<String>,
    has_define_emits: bool,
    models: Vec<Vue3ModelDecl>,
    has_define_expose: bool,
    has_define_slots: bool,
    needs_use_slots: bool,
    has_top_level_await: bool,
    errors: Vec<String>,
    local_setup_bindings: BTreeSet<String>,
    local_setup_binding_types: BTreeMap<String, String>,
    props_destructured_bindings: BTreeMap<String, String>,
    props_destructured_prop_order: Vec<String>,
    props_destructured_rest_id: Option<String>,
    props_destructured_defaults: BTreeMap<String, Vue3PropsDestructuredDefault>,
    props_destructured_default_order: Vec<String>,
    props_destructured_default_types: BTreeMap<String, String>,
    props_type_runtime_types: BTreeMap<String, Vec<String>>,
    vue_import_aliases: BTreeMap<String, String>,
    declared_types: BTreeMap<String, Vec<String>>,
    define_model_declared_types: BTreeMap<String, Vec<String>>,
    props_type_declarations: BTreeMap<String, Vue27TypeMembers>,
    emits_type_declarations: BTreeMap<String, Vue27EmitsType>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Vue3ScriptSetupReturnBinding {
    name: String,
    kind: Vue3ScriptSetupReturnBindingKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Vue3ScriptSetupReturnBindingKind {
    Local,
    Import { source: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Vue3ModelDecl {
    name: String,
    prop_runtime: Option<String>,
    runtime_types: Option<Vec<String>>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Vue3DefineModelOptionsSplit {
    prop_option_ranges: Vec<(usize, usize)>,
    transformer_option_ranges: Vec<(usize, usize)>,
    remove_entire_call_options: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Vue3PropsDestructuredDefault {
    value: String,
    inferred_type: Option<String>,
    is_literal: bool,
    is_function: bool,
    is_identifier: bool,
}

fn script_content(
    descriptor: &SfcDescriptor,
    raw_content: &str,
    filename: &str,
    options: &SfcScriptCompileOptions,
    base_bindings: &BTreeMap<String, String>,
) -> GeneratedScriptContent {
    let Some(script_setup) = descriptor.script_setup.as_ref() else {
        return GeneratedScriptContent {
            content: raw_content.to_string(),
            errors: Vec::new(),
            bindings: BTreeMap::new(),
            removed_bindings: BTreeSet::new(),
        };
    };

    let normal_script = analyze_vue3_normal_script_for_setup(descriptor);
    let normal_type_context = vue3_normal_script_type_context(descriptor);
    let normal_vue_import_aliases = vue3_normal_script_vue_import_aliases(descriptor);
    let setup_analysis = analyze_vue3_script_setup(
        script_setup,
        descriptor.script.is_none(),
        &normal_type_context,
        &normal_vue_import_aliases,
        options.is_prod,
    );
    let is_ts = script_is_typescript(&script_setup.attrs)
        || descriptor
            .script
            .as_ref()
            .is_some_and(|script| script_is_typescript(&script.attrs));
    let return_bindings = vue3_script_setup_return_bindings(descriptor, &setup_analysis, is_ts);
    let template_binding_metadata =
        vue3_script_setup_template_binding_metadata(descriptor, base_bindings, &setup_analysis);
    let template_props_aliases = vue3_script_setup_template_props_aliases(&setup_analysis);
    let inline_render = vue3_inline_template_render(
        descriptor,
        options,
        &template_binding_metadata,
        &template_props_aliases,
        is_ts,
    );
    let mut content = String::new();
    if let Some(render) = inline_render.as_ref() {
        append_vue3_module_chunk(&mut content, &render.preamble);
    }
    if let Some(import) = vue3_script_setup_helper_import(&setup_analysis, options, is_ts) {
        append_vue3_module_chunk(&mut content, &import);
    }
    append_vue3_module_chunk(&mut content, &normal_script.module_content);
    append_vue3_module_chunk(&mut content, &setup_analysis.module_content);
    append_vue3_module_chunk(
        &mut content,
        &vue3_script_setup_export(
            &setup_analysis,
            &return_bindings,
            filename,
            &normal_script,
            is_ts,
            options.is_prod,
            inline_render.as_ref(),
        ),
    );
    let mut bindings = BTreeMap::new();
    let script_returns = descriptor
        .script
        .as_ref()
        .map(vue3_script_block_return_bindings)
        .unwrap_or_default();
    for import in script_returns
        .imports
        .iter()
        .chain(setup_analysis.imports.iter())
    {
        if !import.is_type {
            bindings.insert(
                import.local.clone(),
                vue3_script_import_binding_type(import).into(),
            );
        }
    }
    bindings.extend(setup_analysis.setup_bindings.clone());
    for prop in &setup_analysis.props_bindings {
        bindings
            .entry(prop.clone())
            .or_insert_with(|| "props".into());
    }
    let mut errors = normal_script.errors;
    errors.extend(setup_analysis.errors);
    if let Some(render) = inline_render.as_ref() {
        errors.extend(render.errors.clone());
    }
    GeneratedScriptContent {
        content: content.trim().to_string(),
        errors,
        bindings,
        removed_bindings: setup_analysis.removed_bindings,
    }
}

fn vue3_script_setup_template_binding_metadata(
    descriptor: &SfcDescriptor,
    base_bindings: &BTreeMap<String, String>,
    setup_analysis: &Vue3ScriptSetupAnalysis,
) -> BTreeMap<String, String> {
    let mut bindings = base_bindings.clone();
    let script_returns = descriptor
        .script
        .as_ref()
        .map(vue3_script_block_return_bindings)
        .unwrap_or_default();
    for import in script_returns
        .imports
        .iter()
        .chain(setup_analysis.imports.iter())
    {
        if !import.is_type {
            bindings.insert(
                import.local.clone(),
                vue3_script_import_binding_type(import).into(),
            );
        }
    }
    bindings.extend(setup_analysis.setup_bindings.clone());
    for prop in &setup_analysis.props_bindings {
        bindings
            .entry(prop.clone())
            .or_insert_with(|| "props".into());
    }
    for removed in &setup_analysis.removed_bindings {
        bindings.remove(removed);
    }
    bindings
}

fn vue3_inline_template_render(
    descriptor: &SfcDescriptor,
    options: &SfcScriptCompileOptions,
    binding_metadata: &BTreeMap<String, String>,
    props_aliases: &BTreeMap<String, String>,
    is_ts: bool,
) -> Option<Vue3InlineTemplateRender> {
    if !options.inline_template {
        return None;
    }
    let Some(template) = descriptor.template.as_ref() else {
        return Some(Vue3InlineTemplateRender {
            preamble: String::new(),
            code: "() => {}".into(),
            errors: Vec::new(),
        });
    };
    if template.attrs.src.is_some() {
        return Some(Vue3InlineTemplateRender {
            preamble: String::new(),
            code: "() => {}".into(),
            errors: Vec::new(),
        });
    }

    let mut core = Vue3CompilerOptions {
        prefix_identifiers: true,
        mode: "module".into(),
        hoist_static: true,
        cache_handlers: true,
        scope_id: options.id.as_ref().map(|id| format!("data-v-{id}")),
        is_ts,
        source_map: false,
        binding_metadata: binding_metadata.clone(),
        props_aliases: props_aliases.clone(),
        inline: true,
        ..Vue3CompilerOptions::default()
    };
    apply_dom_parser_defaults(&mut core);
    let result = compile_dom(
        TemplateSource {
            filename: descriptor.filename.clone(),
            source: template.content.clone(),
            file_id: descriptor.source_file,
            base_offset: template.loc.start,
        },
        DomCompilerOptions {
            core,
            transform_asset_urls: true,
            asset_url_options: AssetUrlOptions::default(),
            ..DomCompilerOptions::default()
        },
    );
    let errors = result
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .map(|diagnostic| format!("{:?}: {}", diagnostic.code, diagnostic.message))
        .collect();
    Some(Vue3InlineTemplateRender {
        preamble: result.preamble,
        code: result.code,
        errors,
    })
}

fn vue3_script_setup_template_props_aliases(
    setup_analysis: &Vue3ScriptSetupAnalysis,
) -> BTreeMap<String, String> {
    setup_analysis.props_destructured_bindings.clone()
}

fn vue3_script_setup_helper_import(
    setup_analysis: &Vue3ScriptSetupAnalysis,
    options: &SfcScriptCompileOptions,
    is_ts: bool,
) -> Option<String> {
    let mut helpers = Vec::new();
    if setup_analysis.has_top_level_await {
        helpers.push("withAsyncContext as _withAsyncContext");
    }
    if !setup_analysis.models.is_empty() {
        helpers.push("useModel as _useModel");
    }
    if setup_analysis.needs_use_slots {
        helpers.push("useSlots as _useSlots");
    }
    if setup_analysis.needs_merge_defaults {
        helpers.push("mergeDefaults as _mergeDefaults");
    }
    if setup_analysis.props_destructured_rest_id.is_some() {
        helpers.push("createPropsRestProxy as _createPropsRestProxy");
    }
    if vue3_script_setup_needs_merge_models(setup_analysis) {
        helpers.push("mergeModels as _mergeModels");
    }
    if is_ts {
        helpers.push("defineComponent as _defineComponent");
    }
    if helpers.is_empty() {
        None
    } else {
        Some(format!(
            "import {{ {} }} from {}\n",
            helpers.join(", "),
            vue3_script_setup_helper_import_source(options)
        ))
    }
}

fn vue3_script_setup_helper_import_source(options: &SfcScriptCompileOptions) -> String {
    options
        .runtime_module_name
        .as_ref()
        .map(|source| format!("\"{}\"", escape_js_double(source)))
        .unwrap_or_else(|| "'vue'".to_string())
}

fn vue3_script_setup_return_bindings(
    descriptor: &SfcDescriptor,
    setup_analysis: &Vue3ScriptSetupAnalysis,
    is_ts: bool,
) -> Vec<Vue3ScriptSetupReturnBinding> {
    let script_returns = descriptor
        .script
        .as_ref()
        .map(vue3_script_block_return_bindings)
        .unwrap_or_default();

    let mut bindings = Vec::new();
    for binding in script_returns.bindings {
        push_unique_vue3_return_binding(
            &mut bindings,
            Vue3ScriptSetupReturnBinding {
                name: binding,
                kind: Vue3ScriptSetupReturnBindingKind::Local,
            },
        );
    }
    for binding in &setup_analysis.return_bindings {
        push_unique_vue3_return_binding(
            &mut bindings,
            Vue3ScriptSetupReturnBinding {
                name: binding.clone(),
                kind: Vue3ScriptSetupReturnBindingKind::Local,
            },
        );
    }
    for import in script_returns
        .imports
        .iter()
        .chain(setup_analysis.imports.iter())
    {
        if import.is_type {
            continue;
        }
        if vue3_script_setup_import_is_returned(descriptor, import, is_ts) {
            push_unique_vue3_return_binding(
                &mut bindings,
                Vue3ScriptSetupReturnBinding {
                    name: import.local.clone(),
                    kind: Vue3ScriptSetupReturnBindingKind::Import {
                        source: import.source.clone(),
                    },
                },
            );
        }
    }
    bindings
}

fn push_unique_vue3_return_binding(
    bindings: &mut Vec<Vue3ScriptSetupReturnBinding>,
    binding: Vue3ScriptSetupReturnBinding,
) {
    if bindings
        .iter()
        .any(|existing| existing.name == binding.name)
    {
        return;
    }
    bindings.push(binding);
}

fn vue3_script_setup_import_is_returned(
    descriptor: &SfcDescriptor,
    import: &Vue27ScriptImport,
    is_ts: bool,
) -> bool {
    let Some(template) = descriptor.template.as_ref() else {
        return true;
    };
    if template.attrs.src.is_some() || template.attrs.lang.is_some() {
        return true;
    }
    vue3_template_uses_identifier(&template.content, &import.local, is_ts)
}

fn vue3_script_import_binding_type(import: &Vue27ScriptImport) -> &'static str {
    if import.imported == "*"
        || (import.imported == "default" && import.source.ends_with(".vue"))
        || import.source == "vue"
    {
        "setup-const"
    } else {
        "setup-maybe-ref"
    }
}

fn vue3_script_block_return_bindings(block: &SfcBlock) -> Vue27ScriptReturnBindings {
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        &block.content,
        script_source_type_from_attrs(&block.attrs),
    )
    .with_options(oxc_parser::ParseOptions {
        parse_regular_expression: true,
        ..oxc_parser::ParseOptions::default()
    })
    .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return Vue27ScriptReturnBindings::default();
    }
    let mut result = Vue27ScriptReturnBindings::default();
    for statement in &parsed.program.body {
        collect_vue27_top_level_script_return_binding(statement, &mut result);
    }
    result
}

fn analyze_vue3_script_setup(
    script_setup: &SfcBlock,
    hoist_static_literals: bool,
    normal_type_context: &Vue27TypeContext,
    normal_vue_import_aliases: &BTreeMap<String, String>,
    is_prod: bool,
) -> Vue3ScriptSetupAnalysis {
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
        return Vue3ScriptSetupAnalysis {
            setup_content: source.to_string(),
            errors: parsed.errors.iter().map(ToString::to_string).collect(),
            ..Vue3ScriptSetupAnalysis::default()
        };
    }

    let mut type_analysis = Vue3ScriptSetupAnalysis {
        declared_types: normal_type_context.declared_types.clone(),
        define_model_declared_types: normal_type_context.define_model_declared_types.clone(),
        props_type_declarations: normal_type_context.props_type_declarations.clone(),
        emits_type_declarations: normal_type_context.emits_type_declarations.clone(),
        ..Vue3ScriptSetupAnalysis::default()
    };
    collect_vue3_declared_types_from_statements(source, &parsed.program.body, &mut type_analysis);
    collect_vue3_setup_local_bindings(&parsed.program.body, is_ts, &mut type_analysis);

    let mut edits = SourceEdits::new(source);
    let mut analysis = Vue3ScriptSetupAnalysis {
        declared_types: type_analysis.declared_types,
        define_model_declared_types: type_analysis.define_model_declared_types,
        props_type_declarations: type_analysis.props_type_declarations,
        emits_type_declarations: type_analysis.emits_type_declarations,
        local_setup_bindings: type_analysis.local_setup_bindings,
        local_setup_binding_types: type_analysis.local_setup_binding_types,
        vue_import_aliases: normal_vue_import_aliases.clone(),
        ..Vue3ScriptSetupAnalysis::default()
    };
    let mut module_chunks = Vec::new();
    for statement in &parsed.program.body {
        match statement {
            Statement::ImportDeclaration(import) => {
                let (start, end) = vue27_statement_span_with_trailing_ws(source, statement);
                let end = vue27_statement_span_with_trailing_comments(
                    source,
                    end,
                    &parsed.program.comments,
                );
                let source_value = import.source.value.as_str();
                if let Some(specifiers) = &import.specifiers {
                    for specifier in specifiers {
                        if let Some((imported, local)) =
                            vue3_import_specifier_compiler_macro(source_value, specifier)
                        {
                            analysis.removed_bindings.insert(local.clone());
                            if imported != local {
                                analysis.errors.push(format!(
                                    "`{imported}` is a compiler macro and cannot be aliased to a different name."
                                ));
                            }
                            continue;
                        }
                        if source_value == "vue" {
                            if let Some(imported) = import_specifier_imported(specifier) {
                                analysis
                                    .vue_import_aliases
                                    .insert(imported, import_specifier_local(specifier));
                            }
                        }
                        analysis.imports.push(Vue27ScriptImport {
                            local: import_specifier_local(specifier),
                            source: source_value.to_string(),
                            imported: import_specifier_imported(specifier)
                                .unwrap_or_else(|| "default".into()),
                            is_type: vue27_import_specifier_is_type(import, specifier),
                        });
                    }
                }
                if let Some(import_source) =
                    vue3_script_setup_kept_import_source(source, import, source_value, start, end)
                {
                    module_chunks.push(Vue27ModuleChunk {
                        start,
                        content: import_source,
                    });
                }
                edits.remove(start, end);
            }
            Statement::VariableDeclaration(declaration) => {
                if hoist_static_literals && vue3_variable_declaration_is_static_hoist(declaration) {
                    let (start, end) = vue27_statement_span_with_trailing_ws(source, statement);
                    if let Some(statement_source) = source.get(start..end) {
                        module_chunks.push(Vue27ModuleChunk {
                            start,
                            content: statement_source.to_string(),
                        });
                    }
                    analyze_vue3_setup_variable_declaration(
                        source,
                        declaration,
                        &mut edits,
                        &mut analysis,
                        is_prod,
                    );
                    edits.remove(start, end);
                    continue;
                }
                analyze_vue3_setup_variable_declaration(
                    source,
                    declaration,
                    &mut edits,
                    &mut analysis,
                    is_prod,
                );
            }
            Statement::FunctionDeclaration(function) if !function.declare => {
                if let Some(id) = &function.id {
                    push_unique(&mut analysis.return_bindings, id.name.as_str());
                    analysis
                        .setup_bindings
                        .insert(id.name.to_string(), "setup-const".into());
                }
            }
            Statement::ClassDeclaration(class) if !class.declare => {
                if let Some(id) = &class.id {
                    push_unique(&mut analysis.return_bindings, id.name.as_str());
                    analysis
                        .setup_bindings
                        .insert(id.name.to_string(), "setup-const".into());
                }
            }
            Statement::ExpressionStatement(statement) => {
                if let Expression::CallExpression(call) = &statement.expression {
                    if is_call_named(call, "defineProps") {
                        collect_vue3_define_props_call(source, call, &mut analysis, is_prod);
                        edits.remove(statement.span.start as usize, statement.span.end as usize);
                    } else if is_call_named(call, "withDefaults")
                        && collect_vue3_with_defaults_call(source, call, &mut analysis, is_prod)
                    {
                        edits.remove(statement.span.start as usize, statement.span.end as usize);
                    } else if is_call_named(call, "defineEmits") {
                        collect_vue3_define_emits_call(source, call, None, &mut analysis);
                        edits.remove(statement.span.start as usize, statement.span.end as usize);
                    } else if is_call_named(call, "defineOptions") {
                        collect_vue3_define_options_call(source, call, &mut analysis);
                        edits.remove(statement.span.start as usize, statement.span.end as usize);
                    } else if is_call_named(call, "defineSlots") {
                        collect_vue3_define_slots_call(call, None, &mut edits, &mut analysis);
                        edits.remove(statement.span.start as usize, statement.span.end as usize);
                    } else if is_call_named(call, "defineModel") {
                        collect_vue3_define_model_call(
                            source,
                            call,
                            None,
                            &mut edits,
                            &mut analysis,
                        );
                    } else if is_call_named(call, "defineExpose") {
                        collect_vue3_define_expose_call(call, &mut edits, &mut analysis);
                    }
                }
            }
            _ if is_ts && vue27_statement_is_type_hoist(statement) => {
                let (start, end) = vue27_statement_span_with_trailing_ws(source, statement);
                if let Some(statement_source) = source.get(start..end) {
                    module_chunks.push(Vue27ModuleChunk {
                        start,
                        content: statement_source.to_string(),
                    });
                }
                edits.remove(start, end);
            }
            _ => {}
        }
    }

    if !analysis.props_destructured_bindings.is_empty() {
        check_vue3_define_props_destructure_default_types(&mut analysis);
        let mut rewrite = Vue3PropsDestructureRewriter::new(
            &analysis.props_destructured_bindings,
            &analysis.vue_import_aliases,
            &mut edits,
        );
        rewrite.walk_program(&parsed.program.body);
        analysis.errors.extend(rewrite.errors);
    }

    let mut await_rewrite = Vue3TopLevelAwaitRewriter::new(source, &mut edits);
    await_rewrite.walk_program(&parsed.program.body);
    analysis.has_top_level_await = await_rewrite.has_await;

    module_chunks.sort_by_key(|chunk| chunk.start);
    analysis.module_content = module_chunks
        .iter()
        .map(|chunk| chunk.content.as_str())
        .collect::<String>();
    analysis.setup_content = edits.apply();
    if analysis.module_content.ends_with('\n') {
        if let Some(indent) = leading_blank_line_indent(&analysis.setup_content) {
            analysis.module_content.push_str(indent);
            analysis.setup_content = analysis.setup_content[indent.len()..].to_string();
        }
    }
    analysis
}

fn vue3_variable_declaration_is_static_hoist(declaration: &VariableDeclaration<'_>) -> bool {
    declaration.kind == VariableDeclarationKind::Const
        && declaration
            .declarations
            .iter()
            .all(|declarator| declarator.init.as_ref().is_some_and(is_literal_expression))
}

fn analyze_vue3_setup_variable_declaration(
    source: &str,
    declaration: &VariableDeclaration<'_>,
    edits: &mut SourceEdits<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
    is_prod: bool,
) {
    let mut macro_declarators = Vec::new();
    for (index, declarator) in declaration.declarations.iter().enumerate() {
        if let Some(Expression::CallExpression(call)) = &declarator.init {
            if is_call_named(call, "defineProps") {
                if matches!(declarator.id, BindingPattern::BindingIdentifier(_)) {
                    collect_vue3_define_props_call(source, call, analysis, is_prod);
                    collect_pattern_bindings(&declarator.id, &mut analysis.return_bindings);
                    collect_pattern_binding_types(
                        &declarator.id,
                        "setup-reactive-const",
                        &mut analysis.setup_bindings,
                    );
                    edits.overwrite(call.span.start as usize, call.span.end as usize, "__props");
                } else {
                    let props_rest_id = collect_vue3_define_props_destructure_bindings(
                        source,
                        &declarator.id,
                        analysis,
                    );
                    collect_vue3_define_props_call(source, call, analysis, is_prod);
                    if let Some(rest_id) = props_rest_id {
                        rewrite_vue3_define_props_destructure_rest(
                            &declarator.id,
                            call,
                            &rest_id,
                            analysis,
                            edits,
                        );
                    } else {
                        macro_declarators.push(index);
                    }
                }
                continue;
            }
            if is_call_named(call, "withDefaults")
                && collect_vue3_with_defaults_call(source, call, analysis, is_prod)
            {
                collect_pattern_bindings(&declarator.id, &mut analysis.return_bindings);
                collect_pattern_binding_types(
                    &declarator.id,
                    "setup-const",
                    &mut analysis.setup_bindings,
                );
                edits.overwrite(call.span.start as usize, call.span.end as usize, "__props");
                continue;
            }
            if is_call_named(call, "defineEmits") {
                let emit_binding =
                    first_pattern_binding(&declarator.id).unwrap_or_else(|| "emit".into());
                collect_vue3_define_emits_call(source, call, Some(&emit_binding), analysis);
                collect_pattern_bindings(&declarator.id, &mut analysis.return_bindings);
                collect_pattern_binding_types(
                    &declarator.id,
                    "setup-const",
                    &mut analysis.setup_bindings,
                );
                edits.overwrite(call.span.start as usize, call.span.end as usize, "__emit");
                continue;
            }
            if is_call_named(call, "defineOptions") {
                collect_vue3_define_options_call(source, call, analysis);
                analysis
                    .errors
                    .push("defineOptions() has no returning value, it cannot be assigned.".into());
                continue;
            }
            if is_call_named(call, "defineSlots") {
                collect_vue3_define_slots_call(call, Some(&declarator.id), edits, analysis);
                collect_pattern_bindings(&declarator.id, &mut analysis.return_bindings);
                collect_pattern_binding_types(
                    &declarator.id,
                    "setup-const",
                    &mut analysis.setup_bindings,
                );
                continue;
            }
            if is_call_named(call, "defineModel") {
                collect_vue3_define_model_call(source, call, Some(&declarator.id), edits, analysis);
                collect_pattern_bindings(&declarator.id, &mut analysis.return_bindings);
                collect_pattern_binding_types(
                    &declarator.id,
                    "setup-ref",
                    &mut analysis.setup_bindings,
                );
                continue;
            }
        }
        let binding_type = vue3_setup_binding_type(declaration.kind, declarator.init.as_ref());
        collect_pattern_binding_types(&declarator.id, binding_type, &mut analysis.setup_bindings);
        collect_pattern_bindings(&declarator.id, &mut analysis.return_bindings);
    }
    remove_vue27_macro_declarators(declaration, &macro_declarators, edits);
}

fn collect_vue3_define_options_call(
    source: &str,
    call: &oxc_ast::ast::CallExpression<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    if analysis.has_define_options {
        analysis
            .errors
            .push("duplicate defineOptions() call".into());
    }
    if call.type_arguments.is_some() {
        analysis
            .errors
            .push("defineOptions() cannot accept type arguments".into());
    }
    let Some(argument) = call.arguments.first() else {
        return;
    };
    analysis.has_define_options = true;
    let expression = unwrap_vue3_ts_expression(argument.to_expression());
    check_vue3_define_options_keys(expression, analysis);
    analysis.options_runtime = source
        .get(expression.span().start as usize..expression.span().end as usize)
        .map(str::trim)
        .map(ToOwned::to_owned);
}

fn unwrap_vue3_ts_expression<'a>(expression: &'a Expression<'a>) -> &'a Expression<'a> {
    match expression {
        Expression::TSAsExpression(expression) => unwrap_vue3_ts_expression(&expression.expression),
        Expression::TSSatisfiesExpression(expression) => {
            unwrap_vue3_ts_expression(&expression.expression)
        }
        Expression::TSTypeAssertion(expression) => {
            unwrap_vue3_ts_expression(&expression.expression)
        }
        Expression::TSNonNullExpression(expression) => {
            unwrap_vue3_ts_expression(&expression.expression)
        }
        Expression::TSInstantiationExpression(expression) => {
            unwrap_vue3_ts_expression(&expression.expression)
        }
        Expression::ParenthesizedExpression(expression) => {
            unwrap_vue3_ts_expression(&expression.expression)
        }
        _ => expression,
    }
}

fn check_vue3_define_options_keys(
    expression: &Expression<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    let Expression::ObjectExpression(object) = expression else {
        return;
    };
    for property in &object.properties {
        let key = match property {
            ObjectPropertyKind::ObjectProperty(property) if !property.computed => {
                match &property.key {
                    PropertyKey::StaticIdentifier(identifier) => Some(identifier.name.to_string()),
                    _ => None,
                }
            }
            _ => None,
        };
        let Some(key) = key else {
            continue;
        };
        let replacement = match key.as_str() {
            "props" => Some("defineProps"),
            "emits" => Some("defineEmits"),
            "expose" => Some("defineExpose"),
            "slots" => Some("defineSlots"),
            _ => None,
        };
        if let Some(replacement) = replacement {
            analysis.errors.push(format!(
                "defineOptions() cannot be used to declare {key}. Use {replacement}() instead."
            ));
        }
    }
}

fn collect_vue3_define_slots_call(
    call: &oxc_ast::ast::CallExpression<'_>,
    binding: Option<&BindingPattern<'_>>,
    edits: &mut SourceEdits<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    if analysis.has_define_slots {
        analysis.errors.push("duplicate defineSlots() call".into());
    }
    analysis.has_define_slots = true;
    if !call.arguments.is_empty() {
        analysis
            .errors
            .push("defineSlots() cannot accept arguments".into());
    }
    if binding.is_some() {
        analysis.needs_use_slots = true;
        edits.overwrite(
            call.span.start as usize,
            call.span.end as usize,
            "_useSlots()",
        );
    }
}

fn collect_vue3_define_expose_call(
    call: &oxc_ast::ast::CallExpression<'_>,
    edits: &mut SourceEdits<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    if analysis.has_define_expose {
        analysis.errors.push("duplicate defineExpose() call".into());
    }
    analysis.has_define_expose = true;
    edits.overwrite(
        call.span.start as usize,
        call.callee.span().end as usize,
        "__expose",
    );
}

fn collect_vue3_define_model_call(
    source: &str,
    call: &oxc_ast::ast::CallExpression<'_>,
    binding: Option<&BindingPattern<'_>>,
    edits: &mut SourceEdits<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    let model = vue3_define_model_decl(source, call, analysis);
    if analysis
        .models
        .iter()
        .any(|existing| existing.name == model.name)
    {
        analysis
            .errors
            .push(format!("duplicate model name \"{}\"", model.name));
    }
    push_unique(&mut analysis.props_bindings, &model.name);
    if let Some(binding) = binding.and_then(first_pattern_binding) {
        analysis
            .setup_bindings
            .insert(binding, "setup-ref".to_string());
    }
    rewrite_vue3_define_model_call(call, edits);
    analysis.models.push(model);
}

fn vue3_define_model_decl(
    source: &str,
    call: &oxc_ast::ast::CallExpression<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Vue3ModelDecl {
    let first_expression = call
        .arguments
        .first()
        .map(|argument| unwrap_vue3_ts_expression(argument.to_expression()));
    let (name, has_name) = first_expression
        .and_then(vue3_define_model_name)
        .map(|name| (name, true))
        .unwrap_or_else(|| ("modelValue".to_string(), false));
    let options = if has_name {
        call.arguments.get(1)
    } else {
        call.arguments.first()
    };
    let prop_runtime =
        options.and_then(|argument| vue3_define_model_prop_runtime(source, argument));
    let runtime_types = call
        .type_arguments
        .as_ref()
        .and_then(|arguments| arguments.params.first())
        .map(|type_argument| infer_vue3_define_model_runtime_type(type_argument, analysis));
    Vue3ModelDecl {
        name,
        prop_runtime,
        runtime_types,
    }
}

fn vue3_define_model_name(expression: &Expression<'_>) -> Option<String> {
    match expression {
        Expression::StringLiteral(literal) => Some(literal.value.to_string()),
        Expression::TemplateLiteral(literal)
            if literal.expressions.is_empty() && literal.quasis.len() == 1 =>
        {
            literal
                .quasis
                .first()
                .and_then(|quasi| quasi.value.cooked.as_ref())
                .map(|value| value.as_str().to_string())
        }
        _ => None,
    }
}

fn vue3_define_model_prop_runtime(source: &str, argument: &Argument<'_>) -> Option<String> {
    let expression = unwrap_vue3_ts_expression(argument.to_expression());
    let start = expression.span().start as usize;
    let end = expression.span().end as usize;
    let runtime = if let Some(split) = vue3_define_model_options_split(expression) {
        remove_source_ranges(source, start, end, &split.transformer_option_ranges)
            .or_else(|| source.get(start..end).map(ToOwned::to_owned))
    } else {
        source.get(start..end).map(ToOwned::to_owned)
    }?;
    let runtime = runtime.trim();
    if runtime.is_empty() {
        None
    } else {
        Some(runtime.to_string())
    }
}

fn rewrite_vue3_define_model_call(
    call: &oxc_ast::ast::CallExpression<'_>,
    edits: &mut SourceEdits<'_>,
) {
    let first_expression = call
        .arguments
        .first()
        .map(|argument| unwrap_vue3_ts_expression(argument.to_expression()));
    let has_name = first_expression.and_then(vue3_define_model_name).is_some();
    let options_index = if has_name { 1 } else { 0 };
    let options = call.arguments.get(options_index);
    let options_split = options.and_then(|argument| {
        vue3_define_model_options_split(unwrap_vue3_ts_expression(argument.to_expression()))
    });
    let options_removed = options_split
        .as_ref()
        .is_some_and(|split| split.remove_entire_call_options);
    if let Some(split) = options_split.as_ref() {
        if split.remove_entire_call_options {
            if has_name {
                if let (Some(previous), Some(options)) = (call.arguments.first(), options) {
                    edits.remove(
                        previous.to_expression().span().end as usize,
                        options.to_expression().span().end as usize,
                    );
                }
            } else if let Some(options) = options {
                let expression = options.to_expression();
                edits.remove(
                    expression.span().start as usize,
                    expression.span().end as usize,
                );
            }
        } else {
            for (start, end) in &split.prop_option_ranges {
                edits.remove(*start, *end);
            }
        }
    }
    edits.overwrite(
        call.callee.span().start as usize,
        call.callee.span().end as usize,
        "_useModel",
    );
    let Some(first_argument) = call.arguments.first() else {
        edits.prepend_right(call.span.end as usize - 1, r#"__props, "modelValue""#);
        return;
    };
    let first_start = first_argument.to_expression().span().start as usize;
    if has_name {
        edits.prepend_right(first_start, "__props, ");
        return;
    }
    let prefix = if options_removed {
        r#"__props, "modelValue""#
    } else {
        r#"__props, "modelValue", "#
    };
    edits.prepend_right(first_start, prefix);
}

fn vue3_define_model_options_split(
    expression: &Expression<'_>,
) -> Option<Vue3DefineModelOptionsSplit> {
    let Expression::ObjectExpression(object) = unwrap_vue3_ts_expression(expression) else {
        return None;
    };
    if object.properties.iter().any(|property| {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            return true;
        };
        property.computed
    }) {
        return None;
    }

    let mut split = Vue3DefineModelOptionsSplit::default();
    for (index, property) in object.properties.iter().enumerate() {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            return None;
        };
        let start = property.span.start as usize;
        let end = object
            .properties
            .get(index + 1)
            .map(|next| next.span().start as usize)
            .unwrap_or_else(|| (object.span.end as usize).saturating_sub(1));
        if matches!(property.key.static_name().as_deref(), Some("get" | "set")) {
            split.transformer_option_ranges.push((start, end));
        } else {
            split.prop_option_ranges.push((start, end));
        }
    }
    split.remove_entire_call_options = split.prop_option_ranges.len() == object.properties.len();
    Some(split)
}

fn remove_source_ranges(
    source: &str,
    start: usize,
    end: usize,
    ranges: &[(usize, usize)],
) -> Option<String> {
    let mut ranges = ranges.to_vec();
    ranges.sort_by_key(|range| range.0);
    let mut cursor = start;
    let mut output = String::new();
    for (range_start, range_end) in ranges {
        if range_start < cursor || range_end < range_start || range_end > end {
            return None;
        }
        output.push_str(source.get(cursor..range_start)?);
        cursor = range_end;
    }
    output.push_str(source.get(cursor..end)?);
    Some(output)
}

fn collect_vue3_define_props_call(
    source: &str,
    call: &oxc_ast::ast::CallExpression<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
    is_prod: bool,
) {
    collect_vue3_define_props_call_seen(analysis);
    if let Some(type_argument) = call
        .type_arguments
        .as_ref()
        .and_then(|arguments| arguments.params.first())
    {
        if !call.arguments.is_empty() {
            analysis
                .errors
                .push(vue27_macro_type_and_runtime_error("defineProps"));
        }
        collect_vue3_define_props_type(source, type_argument, None, analysis, is_prod);
        return;
    }
    let Some(argument) = call.arguments.first() else {
        return;
    };
    let expression = argument.to_expression();
    for key in vue3_runtime_prop_keys(expression) {
        push_unique(&mut analysis.props_bindings, &key);
    }
    let Some(runtime) = source
        .get(expression.span().start as usize..expression.span().end as usize)
        .map(ToOwned::to_owned)
    else {
        return;
    };
    analysis.props_runtime =
        if let Some(defaults) = vue3_props_destructured_runtime_defaults(analysis) {
            analysis.needs_merge_defaults = true;
            Some(format!(
                "/*@__PURE__*/_mergeDefaults({}, {})",
                runtime.trim(),
                defaults
            ))
        } else {
            Some(runtime)
        };
}

fn collect_vue3_define_props_call_seen(analysis: &mut Vue3ScriptSetupAnalysis) {
    if analysis.has_define_props {
        analysis.errors.push("duplicate defineProps() call".into());
    }
    analysis.has_define_props = true;
}

fn collect_vue3_with_defaults_call(
    source: &str,
    call: &oxc_ast::ast::CallExpression<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
    is_prod: bool,
) -> bool {
    let Some(define_props_call) =
        call.arguments
            .first()
            .and_then(|argument| match argument.to_expression() {
                Expression::CallExpression(call) if is_call_named(call, "defineProps") => {
                    Some(call)
                }
                _ => None,
            })
    else {
        analysis
            .errors
            .push("withDefaults' first argument must be a defineProps call.".to_string());
        return true;
    };
    let Some(type_argument) = define_props_call
        .type_arguments
        .as_ref()
        .and_then(|arguments| arguments.params.first())
    else {
        collect_vue3_define_props_call(source, define_props_call, analysis, is_prod);
        analysis.errors.push(
            "withDefaults can only be used with type-based defineProps declaration.".to_string(),
        );
        return true;
    };
    collect_vue3_define_props_call_seen(analysis);
    if !define_props_call.arguments.is_empty() {
        analysis
            .errors
            .push(vue27_macro_type_and_runtime_error("defineProps"));
        analysis.errors.push(
            "withDefaults can only be used with type-based defineProps declaration.".to_string(),
        );
    }
    if call.arguments.get(1).is_none() {
        analysis
            .errors
            .push("The 2nd argument of withDefaults is required.".to_string());
    }
    let defaults = call
        .arguments
        .get(1)
        .and_then(|argument| {
            if vue3_expression_references_non_literal_setup_local(
                argument.to_expression(),
                analysis,
            ) {
                analysis.errors.push(
                    "`defineProps()` in <script setup> cannot reference locally declared variables because it will be hoisted outside of the setup() function. If your component options require initialization in the module scope, use a separate normal <script> to export the options instead."
                        .to_string(),
                );
            }
            vue27_runtime_defaults_from_argument(source, argument)
        });
    collect_vue3_define_props_type(source, type_argument, defaults, analysis, is_prod);
    true
}

fn collect_vue3_define_props_type(
    source: &str,
    type_argument: &TSType<'_>,
    defaults: Option<Vue27RuntimeDefaults>,
    analysis: &mut Vue3ScriptSetupAnalysis,
    is_prod: bool,
) {
    let Some(type_members) = vue3_resolve_props_type(source, type_argument, analysis) else {
        return;
    };
    let default_map = defaults
        .as_ref()
        .and_then(|defaults| defaults.static_defaults.as_ref());
    let has_static_defaults = default_map.is_some();
    let dynamic_defaults = defaults
        .as_ref()
        .filter(|defaults| defaults.static_defaults.is_none());
    let mut props = Vec::new();
    for member in &type_members.members {
        let mut prop = member.clone();
        if let Some(default) =
            vue3_props_destructured_default_option(analysis, &prop.key, Some(prop.types.as_slice()))
        {
            prop.default = Some(default);
        } else if let Some(default) = default_map.and_then(|defaults| defaults.get(&prop.key)) {
            prop.default = Some(default.clone());
        }
        analysis
            .props_type_runtime_types
            .insert(prop.key.clone(), prop.types.clone());
        push_unique(&mut analysis.props_bindings, &prop.key);
        props.push(prop);
    }
    analysis.props_type_runtime = true;
    let props_runtime = gen_vue3_runtime_props(&props, is_prod, has_static_defaults);
    analysis.props_runtime = if let Some(defaults) = dynamic_defaults {
        analysis.needs_merge_defaults = true;
        Some(format!(
            "/*@__PURE__*/_mergeDefaults({props_runtime}, {})",
            defaults.source
        ))
    } else {
        Some(props_runtime)
    };
}

fn vue3_resolve_props_type<'a>(
    source: &str,
    type_argument: &'a TSType<'a>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vue27TypeMembers> {
    match type_argument {
        TSType::TSTypeLiteral(literal) => {
            Some(vue3_type_members_from_literal(source, literal, analysis))
        }
        TSType::TSTypeReference(reference) => {
            let name = vue27_ts_type_name_identifier(&reference.type_name)?;
            analysis.props_type_declarations.get(name).cloned()
        }
        _ => None,
    }
}

fn vue3_type_members_from_literal(
    source: &str,
    literal: &TSTypeLiteral<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Vue27TypeMembers {
    Vue27TypeMembers {
        source: source
            .get(literal.span.start as usize..literal.span.end as usize)
            .unwrap_or_default()
            .to_string(),
        members: vue3_runtime_props_from_signatures(source, &literal.members, analysis),
    }
}

fn vue3_type_members_from_interface_body(
    source: &str,
    body: &TSInterfaceBody<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Vue27TypeMembers {
    Vue27TypeMembers {
        source: source
            .get(body.span.start as usize..body.span.end as usize)
            .unwrap_or_default()
            .to_string(),
        members: vue3_runtime_props_from_signatures(source, &body.body, analysis),
    }
}

fn vue3_runtime_props_from_signatures(
    source: &str,
    signatures: &[TSSignature<'_>],
    analysis: &Vue3ScriptSetupAnalysis,
) -> Vec<Vue27RuntimeProp> {
    let mut props = Vec::new();
    for signature in signatures {
        match signature {
            TSSignature::TSPropertySignature(property) if !property.computed => {
                if let Some(key) = vue27_property_key_static_name(&property.key) {
                    let types = property
                        .type_annotation
                        .as_ref()
                        .map(|annotation| {
                            infer_vue3_runtime_type(&annotation.type_annotation, analysis)
                        })
                        .unwrap_or_else(|| vec!["null".into()]);
                    props.push(Vue27RuntimeProp {
                        key,
                        types,
                        required: !property.optional,
                        default: None,
                        is_method: false,
                        type_annotation_source: property.type_annotation.as_ref().and_then(
                            |annotation| {
                                source
                                    .get(
                                        annotation.span.start as usize
                                            ..annotation.span.end as usize,
                                    )
                                    .map(ToOwned::to_owned)
                            },
                        ),
                        member_source: source
                            .get(property.span.start as usize..property.span.end as usize)
                            .map(ToOwned::to_owned),
                    });
                }
            }
            TSSignature::TSMethodSignature(method) if !method.computed => {
                if let Some(key) = vue27_property_key_static_name(&method.key) {
                    props.push(Vue27RuntimeProp {
                        key,
                        types: vec!["Function".into()],
                        required: !method.optional,
                        default: None,
                        is_method: true,
                        type_annotation_source: method.return_type.as_ref().and_then(
                            |annotation| {
                                source
                                    .get(
                                        annotation.span.start as usize
                                            ..annotation.span.end as usize,
                                    )
                                    .map(ToOwned::to_owned)
                            },
                        ),
                        member_source: source
                            .get(method.span.start as usize..method.span.end as usize)
                            .map(ToOwned::to_owned),
                    });
                }
            }
            _ => {}
        }
    }
    props
}

fn infer_vue3_runtime_type(node: &TSType<'_>, analysis: &Vue3ScriptSetupAnalysis) -> Vec<String> {
    match node {
        TSType::TSStringKeyword(_) => vec!["String".into()],
        TSType::TSNumberKeyword(_) => vec!["Number".into()],
        TSType::TSBooleanKeyword(_) => vec!["Boolean".into()],
        TSType::TSObjectKeyword(_) | TSType::TSTypeLiteral(_) | TSType::TSIntersectionType(_) => {
            vec!["Object".into()]
        }
        TSType::TSFunctionType(_) | TSType::TSConstructorType(_) => vec!["Function".into()],
        TSType::TSArrayType(_) | TSType::TSTupleType(_) => vec!["Array".into()],
        TSType::TSSymbolKeyword(_) => vec!["Symbol".into()],
        TSType::TSAnyKeyword(_)
        | TSType::TSBigIntKeyword(_)
        | TSType::TSNeverKeyword(_)
        | TSType::TSNullKeyword(_)
        | TSType::TSUndefinedKeyword(_)
        | TSType::TSUnknownKeyword(_)
        | TSType::TSVoidKeyword(_) => vec!["null".into()],
        TSType::TSLiteralType(literal) => match &literal.literal {
            TSLiteral::StringLiteral(_) => vec!["String".into()],
            TSLiteral::BooleanLiteral(_) => vec!["Boolean".into()],
            TSLiteral::NumericLiteral(_) => vec!["Number".into()],
            _ => vec!["null".into()],
        },
        TSType::TSTypeReference(reference) => {
            if let Some(name) = vue27_ts_type_name_identifier(&reference.type_name) {
                if let Some(types) = analysis.declared_types.get(name) {
                    return types.clone();
                }
                match name {
                    "Array" | "Function" | "Object" | "Set" | "Map" | "WeakSet" | "WeakMap"
                    | "Date" | "Promise" => return vec![name.to_string()],
                    "Record" | "Partial" | "Readonly" | "Pick" | "Omit" | "Exclude" | "Extract"
                    | "Required" | "InstanceType" => return vec!["Object".into()],
                    _ => {}
                }
            }
            vec!["null".into()]
        }
        TSType::TSParenthesizedType(parenthesized) => {
            infer_vue3_runtime_type(&parenthesized.type_annotation, analysis)
        }
        TSType::TSUnionType(union) => {
            let mut types = Vec::new();
            for ty in &union.types {
                for runtime_type in infer_vue3_runtime_type(ty, analysis) {
                    push_unique(&mut types, &runtime_type);
                }
            }
            types
        }
        _ => vec!["null".into()],
    }
}

fn infer_vue3_define_model_runtime_type(
    node: &TSType<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Vec<String> {
    match node {
        TSType::TSStringKeyword(_) => vec!["String".into()],
        TSType::TSNumberKeyword(_) => vec!["Number".into()],
        TSType::TSBooleanKeyword(_) => vec!["Boolean".into()],
        TSType::TSObjectKeyword(_) => vec!["Object".into()],
        TSType::TSTypeLiteral(literal) => {
            let mut types = Vec::new();
            for member in &literal.members {
                let runtime_type = match member {
                    TSSignature::TSCallSignatureDeclaration(_)
                    | TSSignature::TSConstructSignatureDeclaration(_) => "Function",
                    _ => "Object",
                };
                push_unique(&mut types, runtime_type);
            }
            if types.is_empty() {
                vec!["Object".into()]
            } else {
                types
            }
        }
        TSType::TSIntersectionType(intersection) => {
            let mut types = Vec::new();
            for ty in &intersection.types {
                for runtime_type in infer_vue3_define_model_runtime_type(ty, analysis) {
                    if runtime_type != "Unknown" {
                        push_unique(&mut types, &runtime_type);
                    }
                }
            }
            if types.is_empty() {
                vec!["Unknown".into()]
            } else {
                types
            }
        }
        TSType::TSFunctionType(_) | TSType::TSConstructorType(_) => vec!["Function".into()],
        TSType::TSArrayType(_) | TSType::TSTupleType(_) => vec!["Array".into()],
        TSType::TSSymbolKeyword(_) => vec!["Symbol".into()],
        TSType::TSNullKeyword(_) => vec!["null".into()],
        TSType::TSAnyKeyword(_)
        | TSType::TSBigIntKeyword(_)
        | TSType::TSNeverKeyword(_)
        | TSType::TSUndefinedKeyword(_)
        | TSType::TSUnknownKeyword(_)
        | TSType::TSVoidKeyword(_) => vec!["Unknown".into()],
        TSType::TSLiteralType(literal) => match &literal.literal {
            TSLiteral::StringLiteral(_) => vec!["String".into()],
            TSLiteral::BooleanLiteral(_) => vec!["Boolean".into()],
            TSLiteral::NumericLiteral(_) | TSLiteral::BigIntLiteral(_) => vec!["Number".into()],
            _ => vec!["Unknown".into()],
        },
        TSType::TSTypeReference(reference) => {
            if let Some(name) = vue27_ts_type_name_identifier(&reference.type_name) {
                if let Some(types) = analysis.define_model_declared_types.get(name) {
                    return types.clone();
                }
                match name {
                    "Array" | "Function" | "Object" | "Set" | "Map" | "WeakSet" | "WeakMap"
                    | "Date" | "Promise" => return vec![name.to_string()],
                    "Record" | "Partial" | "Readonly" | "Pick" | "Omit" | "Exclude" | "Extract"
                    | "Required" | "InstanceType" => return vec!["Object".into()],
                    _ => {}
                }
            }
            vec!["Unknown".into()]
        }
        TSType::TSParenthesizedType(parenthesized) => {
            infer_vue3_define_model_runtime_type(&parenthesized.type_annotation, analysis)
        }
        TSType::TSUnionType(union) => {
            let mut types = Vec::new();
            for ty in &union.types {
                for runtime_type in infer_vue3_define_model_runtime_type(ty, analysis) {
                    push_unique(&mut types, &runtime_type);
                }
            }
            types
        }
        _ => vec!["Unknown".into()],
    }
}

fn gen_vue3_runtime_props(
    props: &[Vue27RuntimeProp],
    is_prod: bool,
    has_static_defaults: bool,
) -> String {
    let mut entries = Vec::new();
    for prop in props {
        let key = vue3_runtime_prop_key(&prop.key);
        let type_string = vue27_runtime_type_string(&prop.types);
        if !is_prod {
            entries.push(format!(
                "{key}: {{ type: {}, required: {}{} }}",
                type_string,
                prop.required,
                prop.default
                    .as_ref()
                    .map(|default| format!(", {default}"))
                    .unwrap_or_default()
            ));
            continue;
        }
        let keep_prod_type = prop.types.iter().any(|ty| {
            ty == "Boolean"
                || (ty == "Function" && (!has_static_defaults || prop.default.is_some()))
        });
        match (keep_prod_type, prop.default.as_ref()) {
            (true, Some(default)) => {
                entries.push(format!("{key}: {{ type: {type_string}, {default} }}"));
            }
            (true, None) => {
                entries.push(format!("{key}: {{ type: {type_string} }}"));
            }
            (false, Some(default)) => {
                entries.push(format!("{key}: {{ {default} }}"));
            }
            (false, None) => {
                entries.push(format!("{key}: {{}}"));
            }
        }
    }
    format!("{{\n    {}\n  }}", entries.join(",\n    "))
}

fn vue3_runtime_prop_key(key: &str) -> String {
    if is_ascii_js_identifier(key) {
        key.to_string()
    } else {
        format!("\"{}\"", escape_js_double(key))
    }
}

fn vue3_props_destructured_runtime_defaults(analysis: &Vue3ScriptSetupAnalysis) -> Option<String> {
    if analysis.props_destructured_default_order.is_empty() {
        return None;
    }
    let mut entries = Vec::new();
    for key in &analysis.props_destructured_default_order {
        let Some(default) = analysis.props_destructured_defaults.get(key) else {
            continue;
        };
        let final_key = vue3_runtime_prop_key(key);
        let value = vue3_props_destructured_default_value(default, None);
        let skip = if vue3_props_destructured_default_needs_skip_factory(default, None) {
            format!(", __skip_{final_key}: true")
        } else {
            String::new()
        };
        entries.push(format!("{final_key}: {value}{skip}"));
    }
    if entries.is_empty() {
        None
    } else {
        Some(format!("{{\n  {}\n}}", entries.join(",\n  ")))
    }
}

fn vue3_props_destructured_default_option(
    analysis: &Vue3ScriptSetupAnalysis,
    key: &str,
    inferred_types: Option<&[String]>,
) -> Option<String> {
    let default = analysis.props_destructured_defaults.get(key)?;
    let value = vue3_props_destructured_default_value(default, inferred_types);
    let skip = if vue3_props_destructured_default_needs_skip_factory(default, inferred_types) {
        ", skipFactory: true"
    } else {
        ""
    };
    Some(format!("default: {value}{skip}"))
}

fn vue3_props_destructured_default_value(
    default: &Vue3PropsDestructuredDefault,
    inferred_types: Option<&[String]>,
) -> String {
    let need_skip_factory =
        vue3_props_destructured_default_needs_skip_factory(default, inferred_types);
    let is_function_prop =
        inferred_types.is_some_and(|types| types.iter().any(|ty| ty == "Function"));
    if !need_skip_factory && !default.is_literal && !is_function_prop {
        format!("() => ({})", default.value)
    } else {
        default.value.clone()
    }
}

fn vue3_props_destructured_default_needs_skip_factory(
    default: &Vue3PropsDestructuredDefault,
    inferred_types: Option<&[String]>,
) -> bool {
    inferred_types.is_none() && (default.is_function || default.is_identifier)
}

fn rewrite_vue3_define_props_destructure_rest(
    pattern: &BindingPattern<'_>,
    call: &oxc_ast::ast::CallExpression<'_>,
    rest_id: &str,
    analysis: &Vue3ScriptSetupAnalysis,
    edits: &mut SourceEdits<'_>,
) {
    let excluded = analysis
        .props_destructured_prop_order
        .iter()
        .map(|name| format!("\"{}\"", escape_js_double(name)))
        .collect::<Vec<_>>()
        .join(",");
    edits.overwrite(
        pattern.span().start as usize,
        pattern.span().end as usize,
        rest_id,
    );
    edits.overwrite(
        call.span.start as usize,
        call.span.end as usize,
        format!("_createPropsRestProxy(__props, [{excluded}])"),
    );
}

fn is_ascii_js_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first == '$' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch == '$' || ch.is_ascii_alphanumeric())
}

fn collect_vue3_define_props_destructure_bindings(
    source: &str,
    pattern: &BindingPattern<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) -> Option<String> {
    match pattern {
        BindingPattern::ObjectPattern(pattern) => {
            for property in &pattern.properties {
                let key =
                    vue3_define_props_destructure_key(&property.key, property.computed, analysis);
                collect_vue3_define_props_destructure_property(
                    source,
                    key.as_deref(),
                    &property.value,
                    analysis,
                );
            }
            if let Some(rest) = &pattern.rest {
                if let Some(rest_id) = first_pattern_binding(&rest.argument) {
                    analysis.props_destructured_rest_id = Some(rest_id.clone());
                    push_unique(&mut analysis.return_bindings, &rest_id);
                    collect_pattern_binding_types(
                        &rest.argument,
                        "setup-reactive-const",
                        &mut analysis.setup_bindings,
                    );
                    return Some(rest_id);
                }
                collect_pattern_binding_types(
                    &rest.argument,
                    "setup-reactive-const",
                    &mut analysis.setup_bindings,
                );
            }
            None
        }
        BindingPattern::ArrayPattern(pattern) => {
            for element in pattern.elements.iter().flatten() {
                collect_pattern_binding_types(
                    element,
                    "props-aliased",
                    &mut analysis.setup_bindings,
                );
            }
            if let Some(rest) = &pattern.rest {
                collect_pattern_binding_types(
                    &rest.argument,
                    "setup-reactive-const",
                    &mut analysis.setup_bindings,
                );
            }
            None
        }
        BindingPattern::AssignmentPattern(pattern) => {
            collect_vue3_define_props_destructure_bindings(source, &pattern.left, analysis)
        }
        BindingPattern::BindingIdentifier(_) => None,
    }
}

fn vue3_define_props_destructure_key(
    key: &PropertyKey<'_>,
    computed: bool,
    analysis: &mut Vue3ScriptSetupAnalysis,
) -> Option<String> {
    let key = match key {
        PropertyKey::StaticIdentifier(identifier) if !computed => Some(identifier.name.to_string()),
        PropertyKey::StringLiteral(literal) => Some(literal.value.to_string()),
        PropertyKey::NumericLiteral(literal) => Some(literal.value.to_string()),
        _ => None,
    };
    if key.is_none() {
        analysis
            .errors
            .push("defineProps() destructure cannot use computed key.".into());
    }
    key
}

fn collect_vue3_define_props_destructure_property(
    source: &str,
    key: Option<&str>,
    value: &BindingPattern<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    match value {
        BindingPattern::BindingIdentifier(identifier) => {
            register_vue3_define_props_destructure_binding(key, identifier.name.as_str(), analysis);
        }
        BindingPattern::AssignmentPattern(pattern) => {
            if vue3_expression_references_non_literal_setup_local(&pattern.right, analysis) {
                analysis.errors.push(
                    "`defineProps()` in <script setup> cannot reference locally declared variables because it will be hoisted outside of the setup() function. If your component options require initialization in the module scope, use a separate normal <script> to export the options instead."
                        .into(),
                );
            }
            if let Some(key) = key {
                if let Some(default) =
                    vue3_props_destructured_default_from_expression(source, &pattern.right)
                {
                    if !analysis
                        .props_destructured_default_order
                        .iter()
                        .any(|existing| existing == key)
                    {
                        analysis
                            .props_destructured_default_order
                            .push(key.to_string());
                    }
                    if let Some(value_type) = default.inferred_type.as_ref() {
                        analysis
                            .props_destructured_default_types
                            .insert(key.to_string(), value_type.clone());
                    }
                    analysis
                        .props_destructured_defaults
                        .insert(key.to_string(), default);
                }
            }
            if let BindingPattern::BindingIdentifier(identifier) = &pattern.left {
                register_vue3_define_props_destructure_binding(
                    key,
                    identifier.name.as_str(),
                    analysis,
                );
            } else {
                analysis
                    .errors
                    .push("defineProps() destructure does not support nested patterns.".into());
                if let Some(local) = first_pattern_binding(&pattern.left) {
                    register_vue3_define_props_destructure_binding(key, &local, analysis);
                }
            }
        }
        _ => {
            analysis
                .errors
                .push("defineProps() destructure does not support nested patterns.".into());
            if let Some(local) = first_pattern_binding(value) {
                register_vue3_define_props_destructure_binding(key, &local, analysis);
            }
        }
    }
}

fn vue3_props_destructured_default_from_expression(
    source: &str,
    expression: &Expression<'_>,
) -> Option<Vue3PropsDestructuredDefault> {
    let value = source
        .get(expression.span().start as usize..expression.span().end as usize)?
        .to_string();
    let unwrapped = unwrap_vue3_ts_expression(expression);
    Some(Vue3PropsDestructuredDefault {
        value,
        inferred_type: infer_vue3_define_props_destructure_default_value_type(expression)
            .map(ToOwned::to_owned),
        is_literal: vue3_props_destructured_default_is_literal(unwrapped),
        is_function: matches!(
            unwrapped,
            Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_)
        ),
        is_identifier: matches!(unwrapped, Expression::Identifier(_)),
    })
}

fn vue3_props_destructured_default_is_literal(expression: &Expression<'_>) -> bool {
    matches!(
        expression,
        Expression::StringLiteral(_)
            | Expression::NumericLiteral(_)
            | Expression::BooleanLiteral(_)
            | Expression::NullLiteral(_)
            | Expression::RegExpLiteral(_)
            | Expression::BigIntLiteral(_)
            | Expression::TemplateLiteral(_)
    )
}

fn register_vue3_define_props_destructure_binding(
    key: Option<&str>,
    local: &str,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    let public_key = key.unwrap_or(local);
    push_unique(&mut analysis.props_destructured_prop_order, public_key);
    analysis
        .props_destructured_bindings
        .insert(local.to_string(), public_key.to_string());
    if key.is_some_and(|key| key == local) {
        analysis
            .setup_bindings
            .insert(local.to_string(), "props".into());
    } else {
        analysis
            .setup_bindings
            .insert(local.to_string(), "props-aliased".into());
    }
}

fn check_vue3_define_props_destructure_default_types(analysis: &mut Vue3ScriptSetupAnalysis) {
    for (key, value_type) in &analysis.props_destructured_default_types {
        let Some(prop_types) = analysis.props_type_runtime_types.get(key) else {
            continue;
        };
        if prop_types.is_empty()
            || prop_types.iter().any(|ty| ty == "null")
            || prop_types.iter().any(|ty| ty == value_type)
        {
            continue;
        }
        analysis.errors.push(format!(
            "Default value of prop \"{key}\" does not match declared type."
        ));
    }
}

fn infer_vue3_define_props_destructure_default_value_type(
    expression: &Expression<'_>,
) -> Option<&'static str> {
    match unwrap_vue3_ts_expression(expression) {
        Expression::StringLiteral(_) => Some("String"),
        Expression::NumericLiteral(_) => Some("Number"),
        Expression::BooleanLiteral(_) => Some("Boolean"),
        Expression::ObjectExpression(_) => Some("Object"),
        Expression::ArrayExpression(_) => Some("Array"),
        Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_) => {
            Some("Function")
        }
        _ => None,
    }
}

struct Vue3PropsDestructureRewriter<'a, 'source> {
    props_destructured_bindings: &'a BTreeMap<String, String>,
    vue_import_aliases: &'a BTreeMap<String, String>,
    edits: &'a mut SourceEdits<'source>,
    scopes: Vec<BTreeMap<String, bool>>,
    errors: Vec<String>,
}

impl<'a, 'source> Vue3PropsDestructureRewriter<'a, 'source> {
    fn new(
        props_destructured_bindings: &'a BTreeMap<String, String>,
        vue_import_aliases: &'a BTreeMap<String, String>,
        edits: &'a mut SourceEdits<'source>,
    ) -> Self {
        let root_scope = props_destructured_bindings
            .keys()
            .map(|local| (local.clone(), true))
            .collect::<BTreeMap<_, _>>();
        Self {
            props_destructured_bindings,
            vue_import_aliases,
            edits,
            scopes: vec![root_scope],
            errors: Vec::new(),
        }
    }

    fn walk_program(&mut self, statements: &[Statement<'_>]) {
        self.mark_block_declarations(statements, true);
        for statement in statements {
            self.walk_statement(statement, true);
        }
    }

    fn walk_statement(&mut self, statement: &Statement<'_>, is_root: bool) {
        match statement {
            Statement::BlockStatement(block) => {
                self.push_scope();
                self.mark_block_declarations(&block.body, false);
                for statement in &block.body {
                    self.walk_statement(statement, false);
                }
                self.pop_scope();
            }
            Statement::ExpressionStatement(statement) => {
                self.walk_expression(&statement.expression);
            }
            Statement::ReturnStatement(statement) => {
                if let Some(argument) = &statement.argument {
                    self.walk_expression(argument);
                }
            }
            Statement::VariableDeclaration(declaration) => {
                self.mark_variable_declaration(declaration, is_root);
                for declarator in &declaration.declarations {
                    if let Some(init) = &declarator.init {
                        self.walk_expression(init);
                    }
                }
            }
            Statement::FunctionDeclaration(function) => self.walk_function(function),
            Statement::IfStatement(statement) => {
                self.walk_expression(&statement.test);
                self.walk_statement(&statement.consequent, false);
                if let Some(alternate) = &statement.alternate {
                    self.walk_statement(alternate, false);
                }
            }
            Statement::ForStatement(statement) => {
                self.push_scope();
                if let Some(init) = &statement.init {
                    match init {
                        oxc_ast::ast::ForStatementInit::VariableDeclaration(declaration) => {
                            self.mark_variable_declaration(declaration, false);
                            for declarator in &declaration.declarations {
                                if let Some(init) = &declarator.init {
                                    self.walk_expression(init);
                                }
                            }
                        }
                        _ => {
                            if let Some(expression) = init.as_expression() {
                                self.walk_expression(expression);
                            }
                        }
                    }
                }
                if let Some(test) = &statement.test {
                    self.walk_expression(test);
                }
                if let Some(update) = &statement.update {
                    self.walk_expression(update);
                }
                self.walk_statement(&statement.body, false);
                self.pop_scope();
            }
            Statement::ForInStatement(statement) => {
                self.push_scope();
                self.mark_for_iteration_left(&statement.left);
                self.walk_expression(&statement.right);
                self.walk_statement(&statement.body, false);
                self.pop_scope();
            }
            Statement::ForOfStatement(statement) => {
                self.push_scope();
                self.mark_for_iteration_left(&statement.left);
                self.walk_expression(&statement.right);
                self.walk_statement(&statement.body, false);
                self.pop_scope();
            }
            Statement::WhileStatement(statement) => {
                self.walk_expression(&statement.test);
                self.walk_statement(&statement.body, false);
            }
            Statement::DoWhileStatement(statement) => {
                self.walk_statement(&statement.body, false);
                self.walk_expression(&statement.test);
            }
            Statement::SwitchStatement(statement) => {
                self.walk_expression(&statement.discriminant);
                for case in &statement.cases {
                    if let Some(test) = &case.test {
                        self.walk_expression(test);
                    }
                    self.push_scope();
                    self.mark_block_declarations(&case.consequent, false);
                    for statement in &case.consequent {
                        self.walk_statement(statement, false);
                    }
                    self.pop_scope();
                }
            }
            Statement::ThrowStatement(statement) => {
                self.walk_expression(&statement.argument);
            }
            Statement::TryStatement(statement) => {
                self.push_scope();
                self.mark_block_declarations(&statement.block.body, false);
                for statement in &statement.block.body {
                    self.walk_statement(statement, false);
                }
                self.pop_scope();
                if let Some(handler) = &statement.handler {
                    self.push_scope();
                    if let Some(param) = &handler.param {
                        self.mark_binding_pattern(&param.pattern);
                    }
                    self.mark_block_declarations(&handler.body.body, false);
                    for statement in &handler.body.body {
                        self.walk_statement(statement, false);
                    }
                    self.pop_scope();
                }
                if let Some(finalizer) = &statement.finalizer {
                    self.push_scope();
                    self.mark_block_declarations(&finalizer.body, false);
                    for statement in &finalizer.body {
                        self.walk_statement(statement, false);
                    }
                    self.pop_scope();
                }
            }
            Statement::LabeledStatement(statement) => {
                self.walk_statement(&statement.body, false);
            }
            _ => {}
        }
    }

    fn walk_expression(&mut self, expression: &Expression<'_>) {
        match expression {
            Expression::Identifier(identifier) => {
                self.rewrite_identifier_reference(
                    identifier.name.as_str(),
                    identifier.span.start as usize,
                    identifier.span.end as usize,
                );
            }
            Expression::ArrayExpression(array) => {
                for element in &array.elements {
                    match element {
                        oxc_ast::ast::ArrayExpressionElement::SpreadElement(spread) => {
                            self.walk_expression(&spread.argument);
                        }
                        oxc_ast::ast::ArrayExpressionElement::Elision(_) => {}
                        element => {
                            if let Some(expression) = element.as_expression() {
                                self.walk_expression(expression);
                            }
                        }
                    }
                }
            }
            Expression::ObjectExpression(object) => {
                for property in &object.properties {
                    self.walk_object_property_kind(property);
                }
            }
            Expression::CallExpression(call) => {
                self.check_call_usage(call);
                self.walk_expression(&call.callee);
                for argument in &call.arguments {
                    self.walk_argument(argument);
                }
            }
            Expression::NewExpression(expression) => {
                self.walk_expression(&expression.callee);
                for argument in &expression.arguments {
                    self.walk_argument(argument);
                }
            }
            Expression::StaticMemberExpression(member) => {
                self.walk_expression(&member.object);
            }
            Expression::ComputedMemberExpression(member) => {
                self.walk_expression(&member.object);
                self.walk_expression(&member.expression);
            }
            Expression::PrivateFieldExpression(member) => {
                self.walk_expression(&member.object);
            }
            Expression::FunctionExpression(function) => self.walk_function(function),
            Expression::ArrowFunctionExpression(function) => self.walk_arrow_function(function),
            Expression::AssignmentExpression(assignment) => {
                self.check_assignment_target(&assignment.left);
                self.walk_assignment_target(&assignment.left);
                self.walk_expression(&assignment.right);
            }
            Expression::UpdateExpression(update) => {
                self.check_simple_assignment_target(&update.argument);
                self.walk_simple_assignment_target(&update.argument);
            }
            Expression::UnaryExpression(expression) => self.walk_expression(&expression.argument),
            Expression::AwaitExpression(expression) => self.walk_expression(&expression.argument),
            Expression::BinaryExpression(expression) => {
                self.walk_expression(&expression.left);
                self.walk_expression(&expression.right);
            }
            Expression::PrivateInExpression(expression) => {
                self.walk_expression(&expression.right);
            }
            Expression::LogicalExpression(expression) => {
                self.walk_expression(&expression.left);
                self.walk_expression(&expression.right);
            }
            Expression::ConditionalExpression(expression) => {
                self.walk_expression(&expression.test);
                self.walk_expression(&expression.consequent);
                self.walk_expression(&expression.alternate);
            }
            Expression::SequenceExpression(expression) => {
                for expression in &expression.expressions {
                    self.walk_expression(expression);
                }
            }
            Expression::TemplateLiteral(expression) => {
                for expression in &expression.expressions {
                    self.walk_expression(expression);
                }
            }
            Expression::TaggedTemplateExpression(expression) => {
                self.walk_expression(&expression.tag);
                for expression in &expression.quasi.expressions {
                    self.walk_expression(expression);
                }
            }
            Expression::ParenthesizedExpression(expression) => {
                self.walk_expression(&expression.expression);
            }
            Expression::TSAsExpression(expression) => self.walk_expression(&expression.expression),
            Expression::TSSatisfiesExpression(expression) => {
                self.walk_expression(&expression.expression);
            }
            Expression::TSTypeAssertion(expression) => {
                self.walk_expression(&expression.expression);
            }
            Expression::TSNonNullExpression(expression) => {
                self.walk_expression(&expression.expression);
            }
            Expression::TSInstantiationExpression(expression) => {
                self.walk_expression(&expression.expression);
            }
            Expression::ChainExpression(chain) => match &chain.expression {
                oxc_ast::ast::ChainElement::CallExpression(call) => {
                    self.check_call_usage(call);
                    self.walk_expression(&call.callee);
                    for argument in &call.arguments {
                        self.walk_argument(argument);
                    }
                }
                oxc_ast::ast::ChainElement::TSNonNullExpression(expression) => {
                    self.walk_expression(&expression.expression);
                }
                oxc_ast::ast::ChainElement::StaticMemberExpression(member) => {
                    self.walk_expression(&member.object);
                }
                oxc_ast::ast::ChainElement::ComputedMemberExpression(member) => {
                    self.walk_expression(&member.object);
                    self.walk_expression(&member.expression);
                }
                oxc_ast::ast::ChainElement::PrivateFieldExpression(member) => {
                    self.walk_expression(&member.object);
                }
            },
            _ => {}
        }
    }

    fn walk_argument(&mut self, argument: &Argument<'_>) {
        match argument {
            Argument::SpreadElement(spread) => self.walk_expression(&spread.argument),
            _ => self.walk_expression(argument.to_expression()),
        }
    }

    fn walk_object_property_kind(&mut self, property: &ObjectPropertyKind<'_>) {
        match property {
            ObjectPropertyKind::ObjectProperty(property) => {
                if property.computed {
                    self.walk_property_key(&property.key);
                }
                if property.shorthand {
                    if let Expression::Identifier(identifier) = &property.value {
                        if let Some(public_name) =
                            self.active_prop_public_name(identifier.name.as_str())
                        {
                            self.edits.append_left(
                                identifier.span.end as usize,
                                format!(": {}", vue3_props_access_exp(public_name)),
                            );
                            return;
                        }
                    }
                }
                self.walk_expression(&property.value);
            }
            ObjectPropertyKind::SpreadProperty(spread) => {
                self.walk_expression(&spread.argument);
            }
        }
    }

    fn walk_property_key(&mut self, key: &PropertyKey<'_>) {
        match key {
            PropertyKey::StaticIdentifier(_) | PropertyKey::PrivateIdentifier(_) => {}
            _ => self.walk_expression(key.to_expression()),
        }
    }

    fn walk_function(&mut self, function: &Function<'_>) {
        self.push_scope();
        if let Some(id) = &function.id {
            self.mark_local(id.name.as_str());
        }
        for param in &function.params.items {
            self.mark_binding_pattern(&param.pattern);
            if let Some(initializer) = &param.initializer {
                self.walk_expression(initializer);
            }
        }
        if let Some(rest) = &function.params.rest {
            self.mark_binding_pattern(&rest.rest.argument);
        }
        if let Some(body) = &function.body {
            self.mark_block_declarations(&body.statements, false);
            for statement in &body.statements {
                self.walk_statement(statement, false);
            }
        }
        self.pop_scope();
    }

    fn walk_arrow_function(&mut self, function: &ArrowFunctionExpression<'_>) {
        self.push_scope();
        for param in &function.params.items {
            self.mark_binding_pattern(&param.pattern);
            if let Some(initializer) = &param.initializer {
                self.walk_expression(initializer);
            }
        }
        if let Some(rest) = &function.params.rest {
            self.mark_binding_pattern(&rest.rest.argument);
        }
        self.mark_block_declarations(&function.body.statements, false);
        for statement in &function.body.statements {
            self.walk_statement(statement, false);
        }
        self.pop_scope();
    }

    fn walk_assignment_target(&mut self, target: &AssignmentTarget<'_>) {
        match target {
            AssignmentTarget::AssignmentTargetIdentifier(_) => {}
            AssignmentTarget::StaticMemberExpression(member) => {
                self.walk_expression(&member.object);
            }
            AssignmentTarget::ComputedMemberExpression(member) => {
                self.walk_expression(&member.object);
                self.walk_expression(&member.expression);
            }
            AssignmentTarget::PrivateFieldExpression(member) => {
                self.walk_expression(&member.object);
            }
            _ => {}
        }
    }

    fn walk_simple_assignment_target(&mut self, target: &SimpleAssignmentTarget<'_>) {
        match target {
            SimpleAssignmentTarget::AssignmentTargetIdentifier(_) => {}
            SimpleAssignmentTarget::StaticMemberExpression(member) => {
                self.walk_expression(&member.object);
            }
            SimpleAssignmentTarget::ComputedMemberExpression(member) => {
                self.walk_expression(&member.object);
                self.walk_expression(&member.expression);
            }
            SimpleAssignmentTarget::PrivateFieldExpression(member) => {
                self.walk_expression(&member.object);
            }
            _ => {}
        }
    }

    fn check_call_usage(&mut self, call: &oxc_ast::ast::CallExpression<'_>) {
        for method in ["watch", "toRef"] {
            if !self.is_call_named_or_alias(call, method) {
                continue;
            }
            let Some(argument) = call
                .arguments
                .first()
                .and_then(vue3_call_argument_expression)
                .map(unwrap_vue3_ts_expression)
            else {
                continue;
            };
            let Expression::Identifier(identifier) = argument else {
                continue;
            };
            if self.is_active_prop_binding(identifier.name.as_str()) {
                self.errors.push(format!(
                    "\"{}\" is a destructured prop and should not be passed directly to {}(). Pass a getter () => {} instead.",
                    identifier.name, method, identifier.name
                ));
            }
        }
    }

    fn is_call_named_or_alias(
        &self,
        call: &oxc_ast::ast::CallExpression<'_>,
        method: &str,
    ) -> bool {
        let expected = self
            .vue_import_aliases
            .get(method)
            .map(String::as_str)
            .unwrap_or(method);
        matches!(&call.callee, Expression::Identifier(identifier) if identifier.name == expected)
    }

    fn check_assignment_target(&mut self, target: &AssignmentTarget<'_>) {
        if let AssignmentTarget::AssignmentTargetIdentifier(identifier) = target {
            if self.is_active_prop_binding(identifier.name.as_str()) {
                self.errors
                    .push("Cannot assign to destructured props as they are readonly.".into());
            }
        }
    }

    fn check_simple_assignment_target(&mut self, target: &SimpleAssignmentTarget<'_>) {
        if let SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) = target {
            if self.is_active_prop_binding(identifier.name.as_str()) {
                self.errors
                    .push("Cannot assign to destructured props as they are readonly.".into());
            }
        }
    }

    fn mark_block_declarations(&mut self, statements: &[Statement<'_>], is_root: bool) {
        for statement in statements {
            match statement {
                Statement::VariableDeclaration(declaration) if !declaration.declare => {
                    self.mark_variable_declaration(declaration, is_root);
                }
                Statement::FunctionDeclaration(function) if !function.declare => {
                    if let Some(id) = &function.id {
                        self.mark_local(id.name.as_str());
                    }
                }
                Statement::ClassDeclaration(class) if !class.declare => {
                    if let Some(id) = &class.id {
                        self.mark_local(id.name.as_str());
                    }
                }
                _ => {}
            }
        }
    }

    fn mark_variable_declaration(&mut self, declaration: &VariableDeclaration<'_>, is_root: bool) {
        if declaration.declare {
            return;
        }
        for declarator in &declaration.declarations {
            if is_root
                && declarator
                    .init
                    .as_ref()
                    .is_some_and(vue3_is_define_props_call)
            {
                continue;
            }
            self.mark_binding_pattern(&declarator.id);
        }
    }

    fn mark_for_iteration_left(&mut self, left: &oxc_ast::ast::ForStatementLeft<'_>) {
        match left {
            oxc_ast::ast::ForStatementLeft::VariableDeclaration(declaration) => {
                self.mark_variable_declaration(declaration, false);
            }
            _ => {
                if let Some(target) = left.as_assignment_target() {
                    self.mark_assignment_target_as_local(target);
                }
            }
        }
    }

    fn mark_assignment_target_as_local(&mut self, target: &AssignmentTarget<'_>) {
        if let AssignmentTarget::AssignmentTargetIdentifier(identifier) = target {
            self.mark_local(identifier.name.as_str());
        }
    }

    fn mark_binding_pattern(&mut self, pattern: &BindingPattern<'_>) {
        match pattern {
            BindingPattern::BindingIdentifier(identifier) => {
                self.mark_local(identifier.name.as_str());
            }
            BindingPattern::ObjectPattern(pattern) => {
                for property in &pattern.properties {
                    self.mark_binding_pattern(&property.value);
                }
                if let Some(rest) = &pattern.rest {
                    self.mark_binding_pattern(&rest.argument);
                }
            }
            BindingPattern::ArrayPattern(pattern) => {
                for element in pattern.elements.iter().flatten() {
                    self.mark_binding_pattern(element);
                }
                if let Some(rest) = &pattern.rest {
                    self.mark_binding_pattern(&rest.argument);
                }
            }
            BindingPattern::AssignmentPattern(pattern) => {
                self.mark_binding_pattern(&pattern.left);
                self.walk_expression(&pattern.right);
            }
        }
    }

    fn push_scope(&mut self) {
        self.scopes.push(BTreeMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn mark_local(&mut self, name: &str) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string(), false);
        }
    }

    fn is_active_prop_binding(&self, name: &str) -> bool {
        self.active_prop_public_name(name).is_some()
    }

    fn active_prop_public_name(&self, name: &str) -> Option<&str> {
        let is_active = self
            .scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name))
            .copied()
            .unwrap_or(false);
        if !is_active {
            return None;
        }
        self.props_destructured_bindings
            .get(name)
            .map(String::as_str)
    }

    fn rewrite_identifier_reference(&mut self, name: &str, start: usize, end: usize) {
        let Some(public_name) = self.active_prop_public_name(name) else {
            return;
        };
        self.edits
            .overwrite(start, end, vue3_props_access_exp(public_name));
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Vue3TopLevelAwaitScopeEntry {
    expression_start: Option<usize>,
}

struct Vue3TopLevelAwaitRewriter<'a, 'source> {
    source: &'source str,
    edits: &'a mut SourceEdits<'source>,
    scopes: Vec<Vec<Vue3TopLevelAwaitScopeEntry>>,
    has_await: bool,
}

impl<'a, 'source> Vue3TopLevelAwaitRewriter<'a, 'source> {
    fn new(source: &'source str, edits: &'a mut SourceEdits<'source>) -> Self {
        Self {
            source,
            edits,
            scopes: Vec::new(),
            has_await: false,
        }
    }

    fn walk_program(&mut self, statements: &[Statement<'_>]) {
        self.push_statement_scope(statements);
        for statement in statements {
            if vue3_top_level_await_entry_statement(statement) {
                self.walk_statement(statement);
            }
        }
        self.pop_statement_scope();
    }

    fn walk_statement(&mut self, statement: &Statement<'_>) {
        match statement {
            Statement::BlockStatement(block) => {
                self.push_statement_scope(&block.body);
                for statement in &block.body {
                    self.walk_statement(statement);
                }
                self.pop_statement_scope();
            }
            Statement::ExpressionStatement(statement) => {
                self.walk_expression(&statement.expression, true);
            }
            Statement::VariableDeclaration(declaration) if !declaration.declare => {
                self.walk_variable_declaration(declaration);
            }
            Statement::IfStatement(statement) => {
                self.walk_expression(&statement.test, false);
                self.walk_statement(&statement.consequent);
                if let Some(alternate) = &statement.alternate {
                    self.walk_statement(alternate);
                }
            }
            Statement::ForStatement(statement) => {
                if let Some(init) = &statement.init {
                    match init {
                        ForStatementInit::VariableDeclaration(declaration) => {
                            self.walk_variable_declaration(declaration);
                        }
                        _ => {
                            if let Some(expression) = init.as_expression() {
                                self.walk_expression(expression, false);
                            }
                        }
                    }
                }
                if let Some(test) = &statement.test {
                    self.walk_expression(test, false);
                }
                if let Some(update) = &statement.update {
                    self.walk_expression(update, false);
                }
                self.walk_statement(&statement.body);
            }
            Statement::ForInStatement(statement) => {
                self.walk_for_statement_left(&statement.left);
                self.walk_expression(&statement.right, false);
                self.walk_statement(&statement.body);
            }
            Statement::ForOfStatement(statement) => {
                self.walk_for_statement_left(&statement.left);
                self.walk_expression(&statement.right, false);
                self.walk_statement(&statement.body);
            }
            Statement::WhileStatement(statement) => {
                self.walk_expression(&statement.test, false);
                self.walk_statement(&statement.body);
            }
            Statement::DoWhileStatement(statement) => {
                self.walk_statement(&statement.body);
                self.walk_expression(&statement.test, false);
            }
            Statement::SwitchStatement(statement) => {
                self.walk_expression(&statement.discriminant, false);
                for case in &statement.cases {
                    if let Some(test) = &case.test {
                        self.walk_expression(test, false);
                    }
                    self.push_statement_scope(&case.consequent);
                    for statement in &case.consequent {
                        self.walk_statement(statement);
                    }
                    self.pop_statement_scope();
                }
            }
            Statement::ThrowStatement(statement) => {
                self.walk_expression(&statement.argument, false);
            }
            Statement::TryStatement(statement) => {
                self.push_statement_scope(&statement.block.body);
                for statement in &statement.block.body {
                    self.walk_statement(statement);
                }
                self.pop_statement_scope();
                if let Some(handler) = &statement.handler {
                    self.push_statement_scope(&handler.body.body);
                    for statement in &handler.body.body {
                        self.walk_statement(statement);
                    }
                    self.pop_statement_scope();
                }
                if let Some(finalizer) = &statement.finalizer {
                    self.push_statement_scope(&finalizer.body);
                    for statement in &finalizer.body {
                        self.walk_statement(statement);
                    }
                    self.pop_statement_scope();
                }
            }
            Statement::LabeledStatement(statement) => {
                self.walk_statement(&statement.body);
            }
            Statement::ReturnStatement(statement) => {
                if let Some(argument) = &statement.argument {
                    self.walk_expression(argument, false);
                }
            }
            Statement::WithStatement(statement) => {
                self.walk_expression(&statement.object, false);
                self.walk_statement(&statement.body);
            }
            _ => {}
        }
    }

    fn walk_variable_declaration(&mut self, declaration: &VariableDeclaration<'_>) {
        if declaration.declare {
            return;
        }
        for declarator in &declaration.declarations {
            self.walk_binding_pattern(&declarator.id);
            if let Some(init) = &declarator.init {
                self.walk_expression(init, false);
            }
        }
    }

    fn walk_expression(&mut self, expression: &Expression<'_>, is_expression_statement: bool) {
        match expression {
            Expression::AwaitExpression(expression) => {
                self.process_await(expression, is_expression_statement);
                self.walk_expression(&expression.argument, false);
            }
            Expression::ArrayExpression(array) => {
                for element in &array.elements {
                    match element {
                        ArrayExpressionElement::SpreadElement(spread) => {
                            self.walk_expression(&spread.argument, false);
                        }
                        ArrayExpressionElement::Elision(_) => {}
                        element => {
                            if let Some(expression) = element.as_expression() {
                                self.walk_expression(expression, false);
                            }
                        }
                    }
                }
            }
            Expression::ObjectExpression(object) => {
                for property in &object.properties {
                    self.walk_object_property_kind(property);
                }
            }
            Expression::CallExpression(call) => {
                self.walk_expression(&call.callee, false);
                for argument in &call.arguments {
                    self.walk_argument(argument);
                }
            }
            Expression::NewExpression(expression) => {
                self.walk_expression(&expression.callee, false);
                for argument in &expression.arguments {
                    self.walk_argument(argument);
                }
            }
            Expression::StaticMemberExpression(member) => {
                self.walk_expression(&member.object, false);
            }
            Expression::ComputedMemberExpression(member) => {
                self.walk_expression(&member.object, false);
                self.walk_expression(&member.expression, false);
            }
            Expression::PrivateFieldExpression(member) => {
                self.walk_expression(&member.object, false);
            }
            Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_) => {}
            Expression::AssignmentExpression(assignment) => {
                self.walk_assignment_target(&assignment.left);
                self.walk_expression(&assignment.right, false);
            }
            Expression::UpdateExpression(update) => {
                self.walk_simple_assignment_target(&update.argument);
            }
            Expression::UnaryExpression(expression) => {
                self.walk_expression(&expression.argument, false);
            }
            Expression::BinaryExpression(expression) => {
                self.walk_expression(&expression.left, false);
                self.walk_expression(&expression.right, false);
            }
            Expression::PrivateInExpression(expression) => {
                self.walk_expression(&expression.right, false);
            }
            Expression::LogicalExpression(expression) => {
                self.walk_expression(&expression.left, false);
                self.walk_expression(&expression.right, false);
            }
            Expression::ConditionalExpression(expression) => {
                self.walk_expression(&expression.test, false);
                self.walk_expression(&expression.consequent, false);
                self.walk_expression(&expression.alternate, false);
            }
            Expression::SequenceExpression(expression) => {
                for expression in &expression.expressions {
                    self.walk_expression(expression, false);
                }
            }
            Expression::TemplateLiteral(expression) => {
                for expression in &expression.expressions {
                    self.walk_expression(expression, false);
                }
            }
            Expression::TaggedTemplateExpression(expression) => {
                self.walk_expression(&expression.tag, false);
                for expression in &expression.quasi.expressions {
                    self.walk_expression(expression, false);
                }
            }
            Expression::ParenthesizedExpression(expression) => {
                self.walk_expression(&expression.expression, false);
            }
            Expression::ClassExpression(class) => {
                self.walk_class(class);
            }
            Expression::ImportExpression(expression) => {
                self.walk_expression(&expression.source, false);
                if let Some(options) = &expression.options {
                    self.walk_expression(options, false);
                }
            }
            Expression::TSAsExpression(expression) => {
                self.walk_expression(&expression.expression, false);
            }
            Expression::TSSatisfiesExpression(expression) => {
                self.walk_expression(&expression.expression, false);
            }
            Expression::TSTypeAssertion(expression) => {
                self.walk_expression(&expression.expression, false);
            }
            Expression::TSNonNullExpression(expression) => {
                self.walk_expression(&expression.expression, false);
            }
            Expression::TSInstantiationExpression(expression) => {
                self.walk_expression(&expression.expression, false);
            }
            Expression::ChainExpression(chain) => match &chain.expression {
                oxc_ast::ast::ChainElement::CallExpression(call) => {
                    self.walk_expression(&call.callee, false);
                    for argument in &call.arguments {
                        self.walk_argument(argument);
                    }
                }
                oxc_ast::ast::ChainElement::TSNonNullExpression(expression) => {
                    self.walk_expression(&expression.expression, false);
                }
                oxc_ast::ast::ChainElement::StaticMemberExpression(member) => {
                    self.walk_expression(&member.object, false);
                }
                oxc_ast::ast::ChainElement::ComputedMemberExpression(member) => {
                    self.walk_expression(&member.object, false);
                    self.walk_expression(&member.expression, false);
                }
                oxc_ast::ast::ChainElement::PrivateFieldExpression(member) => {
                    self.walk_expression(&member.object, false);
                }
            },
            _ => {}
        }
    }

    fn walk_argument(&mut self, argument: &Argument<'_>) {
        match argument {
            Argument::SpreadElement(spread) => self.walk_expression(&spread.argument, false),
            _ => self.walk_expression(argument.to_expression(), false),
        }
    }

    fn walk_object_property_kind(&mut self, property: &ObjectPropertyKind<'_>) {
        match property {
            ObjectPropertyKind::ObjectProperty(property) => {
                if property.method {
                    return;
                }
                if property.computed {
                    self.walk_property_key(&property.key);
                }
                self.walk_expression(&property.value, false);
            }
            ObjectPropertyKind::SpreadProperty(spread) => {
                self.walk_expression(&spread.argument, false);
            }
        }
    }

    fn walk_property_key(&mut self, key: &PropertyKey<'_>) {
        match key {
            PropertyKey::StaticIdentifier(_) | PropertyKey::PrivateIdentifier(_) => {}
            _ => self.walk_expression(key.to_expression(), false),
        }
    }

    fn walk_class(&mut self, class: &oxc_ast::ast::Class<'_>) {
        if let Some(super_class) = &class.super_class {
            self.walk_expression(super_class, false);
        }
        for element in &class.body.body {
            match element {
                ClassElement::StaticBlock(block) => {
                    self.push_statement_scope(&block.body);
                    for statement in &block.body {
                        self.walk_statement(statement);
                    }
                    self.pop_statement_scope();
                }
                ClassElement::PropertyDefinition(property) => {
                    if property.computed {
                        self.walk_property_key(&property.key);
                    }
                    if let Some(value) = &property.value {
                        self.walk_expression(value, false);
                    }
                }
                ClassElement::AccessorProperty(property) => {
                    if property.computed {
                        self.walk_property_key(&property.key);
                    }
                }
                ClassElement::MethodDefinition(_) | ClassElement::TSIndexSignature(_) => {}
            }
        }
    }

    fn walk_for_statement_left(&mut self, left: &ForStatementLeft<'_>) {
        match left {
            ForStatementLeft::VariableDeclaration(declaration) => {
                self.walk_variable_declaration(declaration);
            }
            _ => {
                if let Some(target) = left.as_assignment_target() {
                    self.walk_assignment_target(target);
                }
            }
        }
    }

    fn walk_assignment_target(&mut self, target: &AssignmentTarget<'_>) {
        match target {
            AssignmentTarget::StaticMemberExpression(member) => {
                self.walk_expression(&member.object, false);
            }
            AssignmentTarget::ComputedMemberExpression(member) => {
                self.walk_expression(&member.object, false);
                self.walk_expression(&member.expression, false);
            }
            AssignmentTarget::PrivateFieldExpression(member) => {
                self.walk_expression(&member.object, false);
            }
            AssignmentTarget::TSAsExpression(expression) => {
                self.walk_expression(&expression.expression, false);
            }
            AssignmentTarget::TSSatisfiesExpression(expression) => {
                self.walk_expression(&expression.expression, false);
            }
            AssignmentTarget::TSNonNullExpression(expression) => {
                self.walk_expression(&expression.expression, false);
            }
            AssignmentTarget::TSTypeAssertion(expression) => {
                self.walk_expression(&expression.expression, false);
            }
            AssignmentTarget::ArrayAssignmentTarget(target) => {
                for element in target.elements.iter().flatten() {
                    self.walk_assignment_target_maybe_default(element);
                }
                if let Some(rest) = &target.rest {
                    self.walk_assignment_target(&rest.target);
                }
            }
            AssignmentTarget::ObjectAssignmentTarget(target) => {
                for property in &target.properties {
                    self.walk_assignment_target_property(property);
                }
                if let Some(rest) = &target.rest {
                    self.walk_assignment_target(&rest.target);
                }
            }
            AssignmentTarget::AssignmentTargetIdentifier(_) => {}
        }
    }

    fn walk_simple_assignment_target(&mut self, target: &SimpleAssignmentTarget<'_>) {
        match target {
            SimpleAssignmentTarget::StaticMemberExpression(member) => {
                self.walk_expression(&member.object, false);
            }
            SimpleAssignmentTarget::ComputedMemberExpression(member) => {
                self.walk_expression(&member.object, false);
                self.walk_expression(&member.expression, false);
            }
            SimpleAssignmentTarget::PrivateFieldExpression(member) => {
                self.walk_expression(&member.object, false);
            }
            SimpleAssignmentTarget::TSAsExpression(expression) => {
                self.walk_expression(&expression.expression, false);
            }
            SimpleAssignmentTarget::TSSatisfiesExpression(expression) => {
                self.walk_expression(&expression.expression, false);
            }
            SimpleAssignmentTarget::TSNonNullExpression(expression) => {
                self.walk_expression(&expression.expression, false);
            }
            SimpleAssignmentTarget::TSTypeAssertion(expression) => {
                self.walk_expression(&expression.expression, false);
            }
            SimpleAssignmentTarget::AssignmentTargetIdentifier(_) => {}
        }
    }

    fn walk_assignment_target_maybe_default(
        &mut self,
        target: &oxc_ast::ast::AssignmentTargetMaybeDefault<'_>,
    ) {
        match target {
            oxc_ast::ast::AssignmentTargetMaybeDefault::AssignmentTargetWithDefault(target) => {
                self.walk_assignment_target(&target.binding);
                self.walk_expression(&target.init, false);
            }
            _ => {
                if let Some(target) = target.as_assignment_target() {
                    self.walk_assignment_target(target);
                }
            }
        }
    }

    fn walk_assignment_target_property(
        &mut self,
        property: &oxc_ast::ast::AssignmentTargetProperty<'_>,
    ) {
        match property {
            oxc_ast::ast::AssignmentTargetProperty::AssignmentTargetPropertyIdentifier(
                property,
            ) => {
                if let Some(init) = &property.init {
                    self.walk_expression(init, false);
                }
            }
            oxc_ast::ast::AssignmentTargetProperty::AssignmentTargetPropertyProperty(property) => {
                if property.computed {
                    self.walk_property_key(&property.name);
                }
                self.walk_assignment_target_maybe_default(&property.binding);
            }
        }
    }

    fn walk_binding_pattern(&mut self, pattern: &BindingPattern<'_>) {
        match pattern {
            BindingPattern::BindingIdentifier(_) => {}
            BindingPattern::ObjectPattern(pattern) => {
                for property in &pattern.properties {
                    if property.computed {
                        self.walk_property_key(&property.key);
                    }
                    self.walk_binding_pattern(&property.value);
                }
                if let Some(rest) = &pattern.rest {
                    self.walk_binding_pattern(&rest.argument);
                }
            }
            BindingPattern::ArrayPattern(pattern) => {
                for element in pattern.elements.iter().flatten() {
                    self.walk_binding_pattern(element);
                }
                if let Some(rest) = &pattern.rest {
                    self.walk_binding_pattern(&rest.argument);
                }
            }
            BindingPattern::AssignmentPattern(pattern) => {
                self.walk_binding_pattern(&pattern.left);
                self.walk_expression(&pattern.right, false);
            }
        }
    }

    fn process_await(
        &mut self,
        expression: &oxc_ast::ast::AwaitExpression<'_>,
        is_expression_statement: bool,
    ) {
        self.has_await = true;
        let await_start = expression.span.start as usize;
        let await_end = expression.span.end as usize;
        let argument_start = expression.argument.span().start as usize;
        let argument_end = expression.argument.span().end as usize;
        if await_start > argument_start || argument_end > self.source.len() {
            return;
        }
        let contains_nested_await = self
            .source
            .get(argument_start..argument_end)
            .is_some_and(contains_js_await_word);
        let semi = if self.needs_semicolon(await_start) {
            ";"
        } else {
            ""
        };
        let async_prefix = if contains_nested_await { "async " } else { "" };
        self.edits.overwrite(
            await_start,
            argument_start,
            format!("{semi}(\n  ([__temp,__restore] = _withAsyncContext({async_prefix}() => "),
        );
        let assignment = if is_expression_statement {
            ""
        } else {
            "__temp = "
        };
        let tail = if is_expression_statement {
            String::new()
        } else {
            ",\n  __temp".to_string()
        };
        self.edits.append_left(
            await_end,
            format!(")),\n  {assignment}await __temp,\n  __restore(){tail}\n)"),
        );
    }

    fn needs_semicolon(&self, await_start: usize) -> bool {
        let is_root_scope = self.scopes.len() == 1;
        self.scopes.last().is_some_and(|scope| {
            scope.iter().enumerate().any(|(index, entry)| {
                entry.expression_start == Some(await_start) && (is_root_scope || index > 0)
            })
        })
    }

    fn push_statement_scope(&mut self, statements: &[Statement<'_>]) {
        self.scopes.push(
            statements
                .iter()
                .map(|statement| Vue3TopLevelAwaitScopeEntry {
                    expression_start: match statement {
                        Statement::ExpressionStatement(statement) => {
                            Some(statement.expression.span().start as usize)
                        }
                        _ => None,
                    },
                })
                .collect(),
        );
    }

    fn pop_statement_scope(&mut self) {
        self.scopes.pop();
    }
}

fn vue3_top_level_await_entry_statement(statement: &Statement<'_>) -> bool {
    match statement {
        Statement::VariableDeclaration(declaration) => !declaration.declare,
        Statement::BlockStatement(_)
        | Statement::BreakStatement(_)
        | Statement::ContinueStatement(_)
        | Statement::DebuggerStatement(_)
        | Statement::DoWhileStatement(_)
        | Statement::EmptyStatement(_)
        | Statement::ExpressionStatement(_)
        | Statement::ForInStatement(_)
        | Statement::ForOfStatement(_)
        | Statement::ForStatement(_)
        | Statement::IfStatement(_)
        | Statement::LabeledStatement(_)
        | Statement::ReturnStatement(_)
        | Statement::SwitchStatement(_)
        | Statement::ThrowStatement(_)
        | Statement::TryStatement(_)
        | Statement::WhileStatement(_)
        | Statement::WithStatement(_) => true,
        _ => false,
    }
}

fn contains_js_await_word(source: &str) -> bool {
    let bytes = source.as_bytes();
    let needle = b"await";
    if bytes.len() < needle.len() {
        return false;
    }
    bytes
        .windows(needle.len())
        .enumerate()
        .any(|(index, window)| {
            window == needle
                && !bytes
                    .get(index.wrapping_sub(1))
                    .is_some_and(|byte| is_js_identifier_byte(*byte))
                && !bytes
                    .get(index + needle.len())
                    .is_some_and(|byte| is_js_identifier_byte(*byte))
        })
}

fn is_js_identifier_byte(byte: u8) -> bool {
    byte == b'_' || byte == b'$' || byte.is_ascii_alphanumeric()
}

fn vue3_props_access_exp(prop: &str) -> String {
    if is_ascii_js_identifier(prop) {
        format!("__props.{prop}")
    } else {
        format!("__props[\"{}\"]", escape_js_double(prop))
    }
}

fn vue3_is_define_props_call(expression: &Expression<'_>) -> bool {
    matches!(unwrap_vue3_ts_expression(expression), Expression::CallExpression(call) if is_call_named(call, "defineProps"))
}

fn vue3_call_argument_expression<'a>(argument: &'a Argument<'a>) -> Option<&'a Expression<'a>> {
    match argument {
        Argument::SpreadElement(_) => None,
        _ => Some(argument.to_expression()),
    }
}

fn vue3_expression_references_non_literal_setup_local(
    expression: &Expression<'_>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> bool {
    let non_literal_bindings = analysis
        .local_setup_binding_types
        .iter()
        .filter_map(|(name, binding_type)| {
            (binding_type != "literal-const").then_some(name.clone())
        })
        .collect::<BTreeSet<_>>();
    vue27_expression_references_setup_local(expression, &non_literal_bindings)
}

fn collect_vue3_define_emits_call(
    source: &str,
    call: &oxc_ast::ast::CallExpression<'_>,
    binding: Option<&str>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    if analysis.has_define_emits {
        analysis.errors.push("duplicate defineEmits() call".into());
    }
    analysis.has_define_emits = true;
    if analysis.emit_binding.is_none() {
        if let Some(binding) = binding {
            analysis.emit_binding = Some(binding.to_string());
        }
    }
    if let Some(type_argument) = call
        .type_arguments
        .as_ref()
        .and_then(|arguments| arguments.params.first())
    {
        if !call.arguments.is_empty() {
            analysis
                .errors
                .push(vue27_macro_type_and_runtime_error("defineEmits"));
        }
        collect_vue3_define_emits_type(source, type_argument, analysis);
        return;
    }
    let Some(argument) = call.arguments.first() else {
        return;
    };
    let expression = argument.to_expression();
    analysis.emits_runtime = source
        .get(expression.span().start as usize..expression.span().end as usize)
        .map(ToOwned::to_owned);
}

fn collect_vue3_define_emits_type(
    source: &str,
    type_argument: &TSType<'_>,
    analysis: &mut Vue3ScriptSetupAnalysis,
) {
    let Some(emits_type) = vue3_resolve_emits_type(source, type_argument, analysis) else {
        return;
    };
    if !emits_type.events.is_empty() {
        analysis.emits_runtime = Some(format!(
            "[{}]",
            emits_type
                .events
                .iter()
                .map(|name| format!("\"{}\"", escape_js_double(name)))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
}

fn vue3_resolve_emits_type<'a>(
    source: &str,
    type_argument: &'a TSType<'a>,
    analysis: &Vue3ScriptSetupAnalysis,
) -> Option<Vue27EmitsType> {
    match type_argument {
        TSType::TSFunctionType(function) => Some(vue27_emits_type_from_function(source, function)),
        TSType::TSTypeLiteral(literal) => Some(vue27_emits_type_from_literal(source, literal)),
        TSType::TSTypeReference(reference) => {
            let name = vue27_ts_type_name_identifier(&reference.type_name)?;
            analysis.emits_type_declarations.get(name).cloned()
        }
        _ => None,
    }
}

fn vue3_runtime_prop_keys(expression: &Expression<'_>) -> Vec<String> {
    match expression {
        Expression::ObjectExpression(object) => object_expression_keys(object),
        Expression::ArrayExpression(array) => array
            .elements
            .iter()
            .filter_map(|element| match element.as_expression() {
                Some(Expression::StringLiteral(literal)) => Some(literal.value.to_string()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn vue3_setup_binding_type(
    kind: VariableDeclarationKind,
    init: Option<&Expression<'_>>,
) -> &'static str {
    if kind != VariableDeclarationKind::Const {
        return "setup-let";
    }
    if init.is_some_and(vue3_is_static_node) {
        return "literal-const";
    }
    "setup-maybe-ref"
}

fn vue3_ts_enum_binding_type(declaration: &TSEnumDeclaration<'_>) -> &'static str {
    if declaration
        .body
        .members
        .iter()
        .all(|member| member.initializer.as_ref().is_none_or(vue3_is_static_node))
    {
        "literal-const"
    } else {
        "setup-const"
    }
}

fn vue3_is_static_node(expression: &Expression<'_>) -> bool {
    match expression {
        Expression::UnaryExpression(expression) => vue3_is_static_node(&expression.argument),
        Expression::LogicalExpression(expression) => {
            vue3_is_static_node(&expression.left) && vue3_is_static_node(&expression.right)
        }
        Expression::BinaryExpression(expression) => {
            vue3_is_static_node(&expression.left) && vue3_is_static_node(&expression.right)
        }
        Expression::ConditionalExpression(expression) => {
            vue3_is_static_node(&expression.test)
                && vue3_is_static_node(&expression.consequent)
                && vue3_is_static_node(&expression.alternate)
        }
        Expression::SequenceExpression(expression) => {
            expression.expressions.iter().all(vue3_is_static_node)
        }
        Expression::TemplateLiteral(expression) => {
            expression.expressions.iter().all(vue3_is_static_node)
        }
        Expression::ParenthesizedExpression(expression) => {
            vue3_is_static_node(&expression.expression)
        }
        Expression::TSAsExpression(expression) => vue3_is_static_node(&expression.expression),
        Expression::TSSatisfiesExpression(expression) => {
            vue3_is_static_node(&expression.expression)
        }
        Expression::TSTypeAssertion(expression) => vue3_is_static_node(&expression.expression),
        Expression::TSNonNullExpression(expression) => vue3_is_static_node(&expression.expression),
        Expression::TSInstantiationExpression(expression) => {
            vue3_is_static_node(&expression.expression)
        }
        Expression::StringLiteral(_)
        | Expression::NumericLiteral(_)
        | Expression::BooleanLiteral(_)
        | Expression::NullLiteral(_)
        | Expression::BigIntLiteral(_) => true,
        _ => false,
    }
}

fn analyze_vue3_normal_script_for_setup(descriptor: &SfcDescriptor) -> Vue3NormalScriptAnalysis {
    let Some(script) = descriptor.script.as_ref() else {
        return Vue3NormalScriptAnalysis::default();
    };
    let source = script.content.as_str();
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(
        &allocator,
        source,
        script_source_type_from_attrs(&script.attrs),
    )
    .with_options(oxc_parser::ParseOptions {
        parse_regular_expression: true,
        ..oxc_parser::ParseOptions::default()
    })
    .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return Vue3NormalScriptAnalysis {
            module_content: source.to_string(),
            errors: parsed.errors.iter().map(ToString::to_string).collect(),
            ..Vue3NormalScriptAnalysis::default()
        };
    }

    let mut edits = SourceEdits::new(source);
    let mut analysis = Vue3NormalScriptAnalysis::default();
    for statement in &parsed.program.body {
        match statement {
            Statement::ExportDefaultDeclaration(declaration) => {
                analysis.has_default_export = true;
                analysis.has_default_export_name = default_export_has_name(declaration);
                rewrite_vue3_export_default("__default__", declaration, &mut edits);
            }
            Statement::ExportNamedDeclaration(declaration) => {
                if rewrite_vue3_compile_script_named_default_export(
                    source,
                    "__default__",
                    declaration,
                    &mut edits,
                ) {
                    analysis.has_default_export = true;
                }
            }
            _ => {}
        }
    }
    analysis.module_content = trim_trailing_blank_lines(&edits.apply()).to_string();
    analysis
}

fn rewrite_vue3_compile_script_named_default_export(
    input: &str,
    variable: &str,
    declaration: &ExportNamedDeclaration<'_>,
    edits: &mut SourceEdits,
) -> bool {
    let Some(specifier) = declaration
        .specifiers
        .iter()
        .find(|specifier| module_export_name(specifier.exported()) == Some("default"))
    else {
        return false;
    };

    if export_named_declaration_only_exports_default(declaration) {
        edits.remove(
            declaration.span.start as usize,
            declaration.span.end as usize,
        );
    } else {
        let end = specifier_end(
            input,
            specifier.span.end as usize,
            declaration.span.end as usize,
        );
        edits.remove(specifier.span.start as usize, end);
    }

    let local_name = module_export_name(specifier.local()).unwrap_or("default");
    if let Some(source) = declaration.source.as_ref() {
        let source_value = source.value.to_string();
        let local_source =
            &input[specifier.local().span().start as usize..specifier.local().span().end as usize];
        edits.prepend(format!(
            "import {{ {local_source} as {variable} }} from '{}'\n",
            source_value
        ));
    } else {
        edits.append(format!("\nconst {variable} = {local_name}\n"));
    }
    true
}

fn vue3_script_setup_export(
    setup_analysis: &Vue3ScriptSetupAnalysis,
    bindings: &[Vue3ScriptSetupReturnBinding],
    filename: &str,
    normal_script: &Vue3NormalScriptAnalysis,
    is_ts: bool,
    is_prod: bool,
    inline_render: Option<&Vue3InlineTemplateRender>,
) -> String {
    let runtime_options =
        vue3_script_setup_runtime_options(filename, normal_script, setup_analysis, is_prod);
    let setup_params = vue3_script_setup_params(setup_analysis);
    let setup_body = vue3_script_setup_body(setup_analysis, bindings, inline_render, is_ts);
    if is_ts {
        let options_spread = setup_analysis
            .options_runtime
            .as_ref()
            .map(|options| format!("\n  ...{options},"))
            .unwrap_or_default();
        let spread = if normal_script.has_default_export {
            "\n  ...__default__,"
        } else {
            ""
        };
        return format!(
            "export default /*@__PURE__*/_defineComponent({{{spread}{options_spread}{runtime_options}\n  {async_prefix}setup({setup_params}) {{\n{setup_body}\n}}\n\n}})",
            async_prefix = vue3_script_setup_async_prefix(setup_analysis),
        );
    }
    if normal_script.has_default_export || setup_analysis.options_runtime.is_some() {
        let default_arg = if normal_script.has_default_export {
            "__default__, "
        } else {
            ""
        };
        let options_arg = setup_analysis
            .options_runtime
            .as_ref()
            .map(|options| format!("{options}, "))
            .unwrap_or_default();
        format!(
            "export default /*@__PURE__*/Object.assign({default_arg}{options_arg}{{{runtime_options}\n  {async_prefix}setup({setup_params}) {{\n{setup_body}\n}}\n\n}})",
            async_prefix = vue3_script_setup_async_prefix(setup_analysis),
        )
    } else {
        format!(
            "export default {{{runtime_options}\n  {async_prefix}setup({setup_params}) {{\n{setup_body}\n}}\n\n}}",
            async_prefix = vue3_script_setup_async_prefix(setup_analysis),
        )
    }
}

fn vue3_script_setup_async_prefix(setup_analysis: &Vue3ScriptSetupAnalysis) -> &'static str {
    if setup_analysis.has_top_level_await {
        "async "
    } else {
        ""
    }
}

fn vue3_script_setup_runtime_options(
    filename: &str,
    normal_script: &Vue3NormalScriptAnalysis,
    setup_analysis: &Vue3ScriptSetupAnalysis,
    is_prod: bool,
) -> String {
    let mut runtime_options = String::new();
    if !normal_script.has_default_export_name {
        runtime_options.push_str(&format!(
            "\n  __name: '{}',",
            escape_js_single(&script_component_name(filename))
        ));
    }
    if let Some(props) = vue3_script_setup_props_runtime(setup_analysis, is_prod) {
        runtime_options.push_str(&format!("\n  props: {},", props.trim()));
    }
    if let Some(emits) = vue3_script_setup_emits_runtime(setup_analysis) {
        runtime_options.push_str(&format!("\n  emits: {},", emits.trim()));
    }
    runtime_options
}

fn vue3_script_setup_needs_merge_models(setup_analysis: &Vue3ScriptSetupAnalysis) -> bool {
    !setup_analysis.models.is_empty()
        && (setup_analysis.props_runtime.is_some() || setup_analysis.emits_runtime.is_some())
}

fn vue3_script_setup_props_runtime(
    setup_analysis: &Vue3ScriptSetupAnalysis,
    is_prod: bool,
) -> Option<String> {
    let props = setup_analysis.props_runtime.as_ref();
    let model_props = vue3_script_setup_model_props_runtime(&setup_analysis.models, is_prod);
    match (props, model_props) {
        (Some(props), Some(model_props)) => Some(format!(
            "/*@__PURE__*/_mergeModels({}, {})",
            props.trim(),
            model_props
        )),
        (Some(props), None) => Some(props.clone()),
        (None, Some(model_props)) => Some(model_props),
        (None, None) => None,
    }
}

fn vue3_script_setup_model_props_runtime(
    models: &[Vue3ModelDecl],
    is_prod: bool,
) -> Option<String> {
    if models.is_empty() {
        return None;
    }
    let mut entries = Vec::new();
    for model in models {
        entries.push(format!(
            "    \"{}\": {},",
            escape_js_double(&model.name),
            vue3_define_model_runtime_decl(model, is_prod)
        ));
        entries.push(format!(
            "    \"{}\": {{}},",
            escape_js_double(&vue3_model_modifiers_prop_name(&model.name))
        ));
    }
    Some(format!("{{\n{}\n  }}", entries.join("\n")))
}

fn vue3_define_model_runtime_decl(model: &Vue3ModelDecl, is_prod: bool) -> String {
    let mut runtime_types = model.runtime_types.clone();
    let has_runtime_options = model.prop_runtime.is_some();
    let mut skip_check = false;
    let mut codegen_options = String::new();

    if let Some(types) = runtime_types.as_mut() {
        let has_boolean = types.iter().any(|ty| ty == "Boolean");
        let has_function = types.iter().any(|ty| ty == "Function");
        let has_unknown = types.iter().any(|ty| ty == "Unknown");

        if has_unknown {
            if has_boolean || has_function {
                types.retain(|ty| ty != "Unknown");
                skip_check = true;
            } else {
                types.clear();
                types.push("null".to_string());
            }
        }

        if !is_prod {
            codegen_options = format!("type: {}", vue27_runtime_type_string(types));
            if skip_check {
                codegen_options.push_str(", skipCheck: true");
            }
        } else if has_boolean || (has_runtime_options && has_function) {
            codegen_options = format!("type: {}", vue27_runtime_type_string(types));
        }
    }

    match (codegen_options.is_empty(), model.prop_runtime.as_deref()) {
        (false, Some(runtime_options)) => {
            format!("{{ {codegen_options}, ...{runtime_options} }}")
        }
        (false, None) => format!("{{ {codegen_options} }}"),
        (true, Some(runtime_options)) => runtime_options.to_string(),
        (true, None) => "{}".to_string(),
    }
}

fn vue3_script_setup_emits_runtime(setup_analysis: &Vue3ScriptSetupAnalysis) -> Option<String> {
    let emits = setup_analysis.emits_runtime.as_ref();
    let model_emits = vue3_script_setup_model_emits_runtime(&setup_analysis.models);
    match (emits, model_emits) {
        (Some(emits), Some(model_emits)) => Some(format!(
            "/*@__PURE__*/_mergeModels({}, {})",
            emits.trim(),
            model_emits
        )),
        (Some(emits), None) => Some(emits.clone()),
        (None, Some(model_emits)) => Some(model_emits),
        (None, None) => None,
    }
}

fn vue3_script_setup_model_emits_runtime(models: &[Vue3ModelDecl]) -> Option<String> {
    if models.is_empty() {
        return None;
    }
    Some(format!(
        "[{}]",
        models
            .iter()
            .map(|model| format!("\"update:{}\"", escape_js_double(&model.name)))
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

fn vue3_model_modifiers_prop_name(name: &str) -> String {
    if name == "modelValue" {
        "modelModifiers".to_string()
    } else {
        format!("{name}Modifiers")
    }
}

fn vue3_script_setup_params(setup_analysis: &Vue3ScriptSetupAnalysis) -> String {
    let props = if setup_analysis.props_type_runtime {
        "__props: any"
    } else {
        "__props"
    };
    if setup_analysis.emit_binding.is_some() {
        format!("{props}, {{ expose: __expose, emit: __emit }}")
    } else {
        format!("{props}, {{ expose: __expose }}")
    }
}

fn vue3_script_setup_body(
    setup_analysis: &Vue3ScriptSetupAnalysis,
    bindings: &[Vue3ScriptSetupReturnBinding],
    inline_render: Option<&Vue3InlineTemplateRender>,
    is_ts: bool,
) -> String {
    let returned = script_setup_returned_bindings(bindings, &setup_analysis.setup_bindings);
    let mut body = String::new();
    if inline_render.is_none() && !setup_analysis.has_define_expose {
        body.push_str("  __expose();\n");
    }
    if setup_analysis.has_top_level_await {
        if is_ts {
            body.push_str("let __temp: any, __restore: any\n");
        } else {
            body.push_str("let __temp, __restore\n");
        }
    }
    if setup_analysis.setup_content.is_empty() {
        body.push('\n');
    } else {
        body.push_str(&setup_analysis.setup_content);
        if !setup_analysis.setup_content.ends_with('\n') {
            body.push('\n');
        }
    }
    if let Some(render) = inline_render {
        body.push_str("return ");
        body.push_str(&render.code);
        return body;
    }
    body.push_str(&format!(
        "const __returned__ = {returned}\nObject.defineProperty(__returned__, '__isScriptSetup', {{ enumerable: false, value: true }})\nreturn __returned__"
    ));
    body
}

fn script_setup_returned_bindings(
    bindings: &[Vue3ScriptSetupReturnBinding],
    setup_bindings: &BTreeMap<String, String>,
) -> String {
    let returned = bindings
        .iter()
        .filter(|binding| {
            !binding.name.starts_with("import:") && !binding.name.starts_with("export:")
        })
        .map(|binding| vue3_script_setup_return_binding_source(binding, setup_bindings))
        .collect::<Vec<_>>()
        .join(", ");
    if returned.is_empty() {
        "{  }".to_string()
    } else {
        format!("{{ {returned} }}")
    }
}

fn vue3_script_setup_return_binding_source(
    binding: &Vue3ScriptSetupReturnBinding,
    setup_bindings: &BTreeMap<String, String>,
) -> String {
    match &binding.kind {
        Vue3ScriptSetupReturnBindingKind::Import { source }
            if source != "vue" && !source.ends_with(".vue") =>
        {
            format!("get {0}() {{ return {0} }}", binding.name)
        }
        _ if setup_bindings
            .get(&binding.name)
            .is_some_and(|binding_type| binding_type == "setup-let") =>
        {
            let set_arg = if binding.name == "v" { "_v" } else { "v" };
            format!(
                "get {0}() {{ return {0} }}, set {0}({1}) {{ {0} = {1} }}",
                binding.name, set_arg
            )
        }
        _ => binding.name.clone(),
    }
}

fn append_vue3_module_chunk(output: &mut String, chunk: &str) {
    let chunk = trim_trailing_blank_lines(chunk);
    if chunk.is_empty() {
        return;
    }
    if !output.is_empty() && !output.ends_with('\n') {
        output.push('\n');
    }
    output.push_str(chunk);
}

fn script_component_name(filename: &str) -> String {
    let stem = std::path::Path::new(filename)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("anonymous");
    stem.to_string()
}

fn quoted_import_path(source: &str) -> Option<&str> {
    let start = source.find(['"', '\''])?;
    let quote = source[start..].chars().next()?;
    let rest = &source[start + quote.len_utf8()..];
    let end = rest.find(quote)?;
    Some(&rest[..end])
}

fn side_effect_tag_errors(source: &str) -> Vec<SfcTemplateError> {
    side_effect_tag_ranges(source)
        .into_iter()
        .filter_map(|(start, end, _)| {
            let start_pos = position_at(source, start)?;
            let end_pos = position_at(source, end)?;
            Some(SfcTemplateError {
                code: 64,
                loc: SfcSourceLocation {
                    start: start_pos,
                    end: end_pos,
                    source: source[start..end].to_string(),
                },
            })
        })
        .collect()
}

fn side_effect_tag_ranges(source: &str) -> Vec<(usize, usize, &'static str)> {
    let mut ranges = Vec::new();
    for tag in ["script", "style"] {
        let mut cursor = 0usize;
        while let Some(start_offset) = source[cursor..].find(&format!("<{tag}")) {
            let start = cursor + start_offset;
            let Some(after_open_offset) = source[start..].find('>') else {
                break;
            };
            let after_open = start + after_open_offset + 1;
            let close_tag = format!("</{tag}>");
            let Some(close_offset) = source[after_open..].find(&close_tag) else {
                break;
            };
            let end = after_open + close_offset + close_tag.len();
            ranges.push((start, end, tag));
            cursor = end;
        }
    }
    ranges.sort_by_key(|(start, _, _)| *start);
    ranges
}

fn position_at(source: &str, offset: usize) -> Option<SfcPosition> {
    if offset > source.len() || !source.is_char_boundary(offset) {
        return None;
    }
    let mut line = 1usize;
    let mut line_start = 0usize;
    let bytes = source.as_bytes();
    let mut index = 0usize;
    while index < offset {
        match bytes[index] {
            b'\r' => {
                if index + 1 < offset && bytes.get(index + 1) == Some(&b'\n') {
                    index += 1;
                }
                line += 1;
                line_start = index + 1;
            }
            b'\n' => {
                line += 1;
                line_start = index + 1;
            }
            _ => {}
        }
        index += 1;
    }
    Some(SfcPosition {
        column: source[line_start..offset].encode_utf16().count() + 1,
        line,
        offset,
    })
}

fn script_source_type(descriptor: &SfcDescriptor) -> oxc_span::SourceType {
    let lang = descriptor
        .script_setup
        .as_ref()
        .or(descriptor.script.as_ref())
        .and_then(|block| block.attrs.lang.as_deref());
    match lang {
        Some("tsx") => oxc_span::SourceType::tsx(),
        Some("ts") => oxc_span::SourceType::ts(),
        _ => oxc_span::SourceType::mjs(),
    }
}

fn script_source_type_from_attrs(attrs: &SfcBlockAttrs) -> oxc_span::SourceType {
    match attrs.lang.as_deref() {
        Some("tsx") => oxc_span::SourceType::tsx(),
        Some("ts") => oxc_span::SourceType::ts(),
        _ => oxc_span::SourceType::mjs(),
    }
}

fn script_mode(attrs: &SfcBlockAttrs) -> JsParseMode {
    if matches!(attrs.lang.as_deref(), Some("ts" | "tsx")) {
        JsParseMode::TypeScript
    } else {
        JsParseMode::ScriptModule
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compact_js_whitespace(source: &str) -> String {
        source.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    #[test]
    fn parses_blocks() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<template><div/></template><script setup lang="ts">const x = 1</script><style scoped>.a{}</style>"#,
        );
        assert!(descriptor.template.is_some());
        assert!(descriptor.script_setup.is_some());
        assert_eq!(descriptor.styles.len(), 1);
    }

    #[test]
    fn vue3_public_parse_projection_uses_official_descriptor_keys() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            concat!(
                r#"<template><div>{{ msg }}</div></template>"#,
                r#"<script setup lang="ts">const msg: string = 'hi'</script>"#,
                r#"<style scoped module>.a{ color: v-bind(color); }</style>"#,
                r#"<i18n lang="json">{"en":"hi"}</i18n>"#,
            ),
        );
        let projected = vue3_sfc_descriptor_value(
            &descriptor,
            &Vue3SfcParseProjectionOptions {
                source_map: false,
                source_root: String::new(),
                pad: Vue3SfcPad::False,
            },
        );

        assert_eq!(projected["scriptSetup"]["type"], json!("script"));
        assert_eq!(projected["scriptSetup"]["setup"], json!(true));
        assert_eq!(projected["scriptSetup"]["lang"], json!("ts"));
        assert!(projected.get("script_setup").is_none());
        assert_eq!(projected["styles"][0]["attrs"]["scoped"], json!(true));
        assert_eq!(projected["styles"][0]["module"], json!(true));
        assert_eq!(projected["customBlocks"][0]["type"], json!("i18n"));
        assert_eq!(projected["customBlocks"][0]["lang"], json!("json"));
        assert_eq!(projected["cssVars"], json!(["color"]));
        assert_eq!(
            projected["template"]["loc"]["source"],
            json!("<div>{{ msg }}</div>")
        );
        assert!(projected["template"].get("map").is_none());
    }

    #[test]
    fn vue3_public_parse_projection_maps_empty_attr_values_like_vue3() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            r#"<template src=""></template><script setup="named">x</script><style scoped="x" module="" src=""></style>"#,
        );
        let projected = vue3_sfc_descriptor_value(
            &descriptor,
            &Vue3SfcParseProjectionOptions {
                source_map: true,
                source_root: String::new(),
                pad: Vue3SfcPad::False,
            },
        );

        assert_eq!(projected["template"]["attrs"]["src"], json!(true));
        assert_eq!(projected["template"]["src"], json!(""));
        assert_eq!(projected["scriptSetup"]["attrs"]["setup"], json!("named"));
        assert_eq!(projected["scriptSetup"]["setup"], json!("named"));
        assert_eq!(projected["styles"][0]["attrs"]["module"], json!(true));
        assert_eq!(projected["styles"][0]["module"], json!(true));
        assert_eq!(projected["styles"][0]["attrs"]["scoped"], json!("x"));
        assert_eq!(projected["styles"][0]["scoped"], json!(true));
        assert!(projected["styles"][0].get("map").is_none());
    }

    #[test]
    fn vue3_parse_decodes_attrs_and_reports_duplicate_attrs_like_official_parser() {
        let mut compiler = SfcCompiler::new();
        let result = compiler.parse_vue3(
            "Attrs.vue",
            r#"<template a="1" a="&amp;" lang="p&amp;g">x</template><style module="m&amp;n" setup>.a{}</style><script setup generic="T &amp; U">y</script>"#,
        );

        let projected =
            vue3_sfc_parse_result_value(&result, &Vue3SfcParseProjectionOptions::default());
        assert_eq!(
            projected["descriptor"]["template"]["attrs"]["a"],
            json!("&")
        );
        assert_eq!(
            projected["descriptor"]["template"]["attrs"]["lang"],
            json!("p&g")
        );
        assert_eq!(projected["descriptor"]["template"]["lang"], json!("p&g"));
        assert_eq!(
            projected["descriptor"]["styles"][0]["attrs"]["module"],
            json!("m&n")
        );
        assert_eq!(projected["descriptor"]["styles"][0]["module"], json!("m&n"));
        assert!(projected["descriptor"]["styles"][0].get("setup").is_none());
        assert_eq!(
            projected["descriptor"]["scriptSetup"]["attrs"]["generic"],
            json!("T & U")
        );
        assert_eq!(
            projected["errors"][0]["message"],
            json!("Duplicate attribute.")
        );
        assert_eq!(projected["errors"][0]["loc"]["source"], json!(""));
        assert_eq!(projected["errors"][0]["loc"]["start"]["offset"], json!(16));
    }

    #[test]
    fn vue3_parse_reports_bogus_question_tags_like_official_parser() {
        let mut compiler = SfcCompiler::new();
        let result = compiler.parse_vue3(
            "Question.vue",
            r#"<?xml?><template><?x?><div/></template><docs><?keep?></docs>"#,
        );

        assert_eq!(
            result.descriptor.template.as_ref().unwrap().content,
            "<?x?><div/>"
        );
        assert_eq!(result.descriptor.custom_blocks[0].content, "<?keep?>");
        let projected =
            vue3_sfc_parse_result_value(&result, &Vue3SfcParseProjectionOptions::default());
        assert_eq!(
            projected["errors"]
                .as_array()
                .unwrap()
                .iter()
                .map(|error| error["message"].as_str().unwrap())
                .collect::<Vec<_>>(),
            vec![
                "'<?' is allowed only in XML context.",
                "'<?' is allowed only in XML context.",
            ]
        );
        assert_eq!(projected["errors"][0]["loc"]["start"]["offset"], json!(1));
        assert_eq!(projected["errors"][1]["loc"]["start"]["offset"], json!(18));
    }

    #[test]
    fn vue3_parse_reports_missing_end_tags_like_official_parser() {
        let mut compiler = SfcCompiler::new();
        let script = compiler.parse_vue3("UnclosedScript.vue", "<script>x");
        assert_eq!(script.descriptor.script.as_ref().unwrap().content, "");
        assert_eq!(script.errors[0].message, "Element is missing end tag.");
        assert_eq!(script.errors[0].loc.as_ref().unwrap().start, 0);

        let nested = compiler.parse_vue3("Nested.vue", "<template><div><span></template>");
        assert_eq!(
            nested.descriptor.template.as_ref().unwrap().content,
            "<div><span>"
        );
        assert_eq!(nested.errors.len(), 1);
        assert_eq!(nested.errors[0].loc.as_ref().unwrap().start, 15);

        let eof = compiler.parse_vue3("Eof.vue", "<template><div><span>");
        assert_eq!(eof.descriptor.template.as_ref().unwrap().content, "");
        assert_eq!(
            eof.errors
                .iter()
                .map(|error| error.loc.as_ref().unwrap().start)
                .collect::<Vec<_>>(),
            vec![15, 10, 0]
        );

        let custom = compiler.parse_vue3("Custom.vue", "<template/><docs><?x?");
        assert_eq!(custom.descriptor.custom_blocks[0].content, "");
        assert_eq!(custom.errors.len(), 1);
        assert_eq!(custom.errors[0].loc.as_ref().unwrap().start, 11);
    }

    #[test]
    fn vue3_parse_reports_malformed_descriptor_syntax_like_official_parser() {
        let mut compiler = SfcCompiler::new();

        let uppercase = compiler.parse_vue3("Upper.vue", "<SCRIPT>let a=1</SCRIPT>");
        assert_eq!(uppercase.descriptor.custom_blocks[0].type_name, "SCRIPT");
        assert_eq!(uppercase.descriptor.custom_blocks[0].content, "");
        assert_eq!(uppercase.errors[0].message, "Element is missing end tag.");
        assert_eq!(uppercase.errors[0].loc.as_ref().unwrap().start, 0);

        let raw_extra =
            compiler.parse_vue3("RawExtra.vue", r#"<script>const s = "</script>";</script>"#);
        assert_eq!(
            raw_extra.descriptor.script.as_ref().unwrap().content,
            "const s = \""
        );
        assert_eq!(raw_extra.errors[0].message, "Invalid end tag.");
        assert_eq!(raw_extra.errors[0].loc.as_ref().unwrap().start, 30);

        let cdata = compiler.parse_vue3("Cdata.vue", "<template><![CDATA[x]]></template>");
        assert_eq!(cdata.descriptor.template.as_ref().unwrap().content, "");
        assert_eq!(
            cdata.errors[0].message,
            "CDATA section is allowed only in XML context."
        );
        assert_eq!(cdata.errors[0].loc.as_ref().unwrap().start, 10);

        let invalid_end = compiler.parse_vue3("InvalidEnd.vue", "<template></div></template>");
        assert_eq!(
            invalid_end.descriptor.template.as_ref().unwrap().content,
            ""
        );
        assert_eq!(invalid_end.errors[0].message, "Invalid end tag.");
        assert_eq!(invalid_end.errors[0].loc.as_ref().unwrap().start, 10);

        let invalid_attr = compiler.parse_vue3("InvalidAttr.vue", "<template =x></template>");
        assert_eq!(
            invalid_attr.errors[0].message,
            "Attribute name cannot start with '='."
        );
        assert_eq!(invalid_attr.errors[0].loc.as_ref().unwrap().start, 10);

        let missing_value = compiler.parse_vue3("MissingValue.vue", "<template a=></template>");
        assert_eq!(
            missing_value.errors[0].message,
            "Attribute value was expected."
        );
        assert_eq!(missing_value.errors[0].loc.as_ref().unwrap().start, 12);

        let nested_duplicate = compiler.parse_vue3(
            "NestedDuplicate.vue",
            "<template><div id id></div></template>",
        );
        assert_eq!(nested_duplicate.errors[0].message, "Duplicate attribute.");
        assert_eq!(nested_duplicate.errors[0].loc.as_ref().unwrap().start, 18);
    }

    #[test]
    fn vue3_parse_preserves_boolean_src_attr_presence_like_official_parser() {
        let mut compiler = SfcCompiler::new();
        let result = compiler.parse_vue3(
            "BoolSrc.vue",
            "<template src></template><script src></script><style src></style>",
        );

        assert!(result.errors.is_empty());
        assert!(result
            .descriptor
            .template
            .as_ref()
            .unwrap()
            .attrs
            .has_src_attr());
        assert!(result
            .descriptor
            .script
            .as_ref()
            .unwrap()
            .attrs
            .has_src_attr());
        assert!(result.descriptor.styles[0].attrs.has_src_attr());

        let projected =
            vue3_sfc_parse_result_value(&result, &Vue3SfcParseProjectionOptions::default());
        assert_eq!(
            projected["descriptor"]["template"]["attrs"]["src"],
            json!(true)
        );
        assert!(projected["descriptor"]["template"].get("src").is_none());
        assert!(projected["descriptor"]["template"].get("map").is_none());
        assert_eq!(
            projected["descriptor"]["script"]["attrs"]["src"],
            json!(true)
        );
        assert!(projected["descriptor"]["script"].get("src").is_none());
        assert!(projected["descriptor"]["script"].get("map").is_none());
        assert_eq!(
            projected["descriptor"]["styles"][0]["attrs"]["src"],
            json!(true)
        );
        assert!(projected["descriptor"]["styles"][0].get("src").is_none());
        assert!(projected["descriptor"]["styles"][0].get("map").is_none());
    }

    #[test]
    fn vue3_parse_reports_duplicate_blocks_like_official_parser() {
        let mut compiler = SfcCompiler::new();
        let result = compiler.parse_vue3(
            "Dup.vue",
            "<template>a</template><template>b</template><script>one</script><script>two</script><script setup>first</script><script setup>second</script>",
        );

        assert_eq!(result.descriptor.template.as_ref().unwrap().content, "a");
        assert_eq!(result.descriptor.script.as_ref().unwrap().content, "one");
        assert_eq!(
            result.descriptor.script_setup.as_ref().unwrap().content,
            "first"
        );
        assert_eq!(
            result
                .errors
                .iter()
                .map(|error| error.message.as_str())
                .collect::<Vec<_>>(),
            vec![
                "Single file component can contain only one <template> element",
                "Single file component can contain only one <script> element",
                "Single file component can contain only one <script setup> element",
            ]
        );
        let projected =
            vue3_sfc_parse_result_value(&result, &Vue3SfcParseProjectionOptions::default());
        assert_eq!(
            projected["errors"][0]["loc"]["source"],
            json!("<template>b</template>")
        );
        assert_eq!(projected["errors"][0]["loc"]["start"]["offset"], json!(22));
    }

    #[test]
    fn vue3_parse_applies_script_src_and_empty_script_rules() {
        let mut compiler = SfcCompiler::new();
        let empty_script =
            compiler.parse_vue3("Empty.vue", "<script>  \n</script><style>x</style>");
        assert!(empty_script.descriptor.script.is_none());
        assert_eq!(
            empty_script.errors[0].message,
            "At least one <template> or <script> is required in a single file component. Empty.vue"
        );

        let setup_src = compiler.parse_vue3(
            "SetupSrc.vue",
            r#"<script setup src="x"></script><script setup>ok</script>"#,
        );
        assert!(setup_src.descriptor.script_setup.is_none());
        assert_eq!(
            setup_src
                .errors
                .iter()
                .map(|error| error.message.as_str())
                .collect::<Vec<_>>(),
            vec![
                "Single file component can contain only one <script setup> element",
                "<script setup> cannot use the \"src\" attribute because its syntax will be ambiguous outside of the component.",
            ]
        );

        let script_src_with_setup = compiler.parse_vue3(
            "SrcAndSetup.vue",
            r#"<script src="x"></script><script setup>ok</script>"#,
        );
        assert!(script_src_with_setup.descriptor.script.is_none());
        assert_eq!(
            script_src_with_setup
                .descriptor
                .script_setup
                .as_ref()
                .unwrap()
                .content,
            "ok"
        );
        assert_eq!(
            script_src_with_setup.errors[0].message,
            "<script> cannot use the \"src\" attribute when <script setup> is also present because they must be processed together."
        );

        let empty_src_with_setup = compiler.parse_vue3(
            "EmptySrcAndSetup.vue",
            r#"<script src=""></script><script setup src=""></script>"#,
        );
        assert!(empty_src_with_setup.errors.is_empty());
        assert!(empty_src_with_setup.descriptor.script.is_some());
        assert!(empty_src_with_setup.descriptor.script_setup.is_some());
    }

    #[test]
    fn vue3_parse_reports_functional_template_attr_like_official_parser() {
        let mut compiler = SfcCompiler::new();
        let result = compiler.parse_vue3(
            "Functional.vue",
            r#"<template functional="x"><div/></template><template functional>b</template>"#,
        );

        assert_eq!(
            result
                .errors
                .iter()
                .map(|error| error.message.as_str())
                .collect::<Vec<_>>(),
            vec![
                "<template functional> is no longer supported in Vue 3, since functional components no longer have significant performance difference from stateful ones. Just use a normal <template> instead.",
                "Single file component can contain only one <template> element",
            ]
        );
        let projected =
            vue3_sfc_parse_result_value(&result, &Vue3SfcParseProjectionOptions::default());
        assert_eq!(
            projected["errors"][0]["loc"]["source"],
            json!("functional=\"x\"")
        );
        assert_eq!(projected["errors"][0]["loc"]["start"]["offset"], json!(10));
        assert_eq!(
            projected["errors"][1]["loc"]["source"],
            json!("<template functional>b</template>")
        );
    }

    #[test]
    fn vue3_parse_options_pad_non_template_blocks_like_official_parser() {
        let mut compiler = SfcCompiler::new();
        let source = concat!(
            "<template>\n  div\n</template>\n",
            "<script>\nconst a = 1\n</script>\n",
            "<style>\n.a{}\n</style>\n",
            "<i18n>\n{}\n</i18n>"
        );
        let line = compiler.parse_vue3_with_options(
            "Pad.vue",
            source,
            Vue3SfcParseOptions {
                pad: Vue3SfcPad::Line,
                ..Vue3SfcParseOptions::default()
            },
        );

        assert_eq!(
            line.descriptor.template.as_ref().unwrap().content,
            "\n  div\n"
        );
        assert_eq!(
            line.descriptor.script.as_ref().unwrap().content,
            "//\n//\n//\n\nconst a = 1\n"
        );
        assert_eq!(line.descriptor.styles[0].content, "\n\n\n\n\n\n\n.a{}\n");
        assert_eq!(
            line.descriptor.custom_blocks[0].content,
            "\n\n\n\n\n\n\n\n\n\n{}\n"
        );

        let space = compiler.parse_vue3_with_options(
            "Pad.vue",
            source,
            Vue3SfcParseOptions {
                pad: Vue3SfcPad::Space,
                ..Vue3SfcParseOptions::default()
            },
        );
        assert!(space
            .descriptor
            .script
            .as_ref()
            .unwrap()
            .content
            .starts_with("          \n"));
        assert!(space.descriptor.styles[0].content.ends_with(".a{}\n"));
    }

    #[test]
    fn vue3_parse_options_ignore_empty_and_dedent_pug_template() {
        let mut compiler = SfcCompiler::new();
        let source = concat!(
            "<template lang=\"pug\">\n  div\n    span\n</template>",
            "<script> </script><style> </style><i18n> </i18n>"
        );
        let default = compiler.parse_vue3("Pug.vue", source);
        assert_eq!(
            default.descriptor.template.as_ref().unwrap().content,
            "\ndiv\n  span\n"
        );
        assert!(default.descriptor.script.is_none());
        assert!(default.descriptor.styles.is_empty());
        assert!(default.descriptor.custom_blocks.is_empty());

        let keep_empty = compiler.parse_vue3_with_options(
            "Pug.vue",
            source,
            Vue3SfcParseOptions {
                ignore_empty: false,
                ..Vue3SfcParseOptions::default()
            },
        );
        assert_eq!(keep_empty.descriptor.script.as_ref().unwrap().content, " ");
        assert_eq!(keep_empty.descriptor.styles[0].content, " ");
        assert_eq!(keep_empty.descriptor.custom_blocks[0].content, " ");
    }

    #[test]
    fn parse_descriptor_cache_hits_and_invalidates_by_source_hash() {
        let mut compiler = SfcCompiler::new();
        let first = compiler.parse("foo.vue", r#"<template><div>{{ a }}</div></template>"#);
        let second = compiler.parse("foo.vue", r#"<template><div>{{ a }}</div></template>"#);
        assert_eq!(first, second);
        assert_eq!(compiler.descriptor_cache_len(), 1);
        assert_eq!(
            compiler.cache_stats(),
            SfcCacheStats {
                descriptor_hits: 1,
                descriptor_misses: 1,
                descriptor_invalidations: 0,
            }
        );

        let changed = compiler.parse("foo.vue", r#"<template><span>{{ b }}</span></template>"#);
        assert_ne!(
            first.template.as_ref().unwrap().content,
            changed.template.as_ref().unwrap().content
        );
        assert_eq!(compiler.descriptor_cache_len(), 1);
        assert_eq!(
            compiler.cache_stats(),
            SfcCacheStats {
                descriptor_hits: 1,
                descriptor_misses: 2,
                descriptor_invalidations: 1,
            }
        );
    }

    #[test]
    fn vue27_parse_cache_preserves_error_projection() {
        let mut compiler = SfcCompiler::new();
        let source = "<template><div></template>";
        let options = Vue27ParseComponentOptions {
            output_source_range: true,
            ..Vue27ParseComponentOptions::default()
        };
        let first =
            compiler.parse_vue27_component_with_filename("bad.vue", source, options.clone());
        let second = compiler.parse_vue27_component_with_filename("bad.vue", source, options);
        assert_eq!(first.errors, second.errors);
        assert!(second.errors.iter().any(|error| error.start.is_some()));

        let masked = compiler.parse_vue27_component_with_filename(
            "bad-masked.vue",
            source,
            Vue27ParseComponentOptions::default(),
        );
        let masked_hit = compiler.parse_vue27_component_with_filename(
            "bad-masked.vue",
            source,
            Vue27ParseComponentOptions::default(),
        );
        assert_eq!(masked.errors, masked_hit.errors);
        assert!(masked_hit
            .errors
            .iter()
            .all(|error| error.start.is_none() && error.end.is_none()));
    }

    #[test]
    fn vue27_parse_component_preserves_top_level_blocks_and_attrs() {
        let mut compiler = SfcCompiler::new();
        let result = compiler.parse_vue27_component(
            r#"
<template><div><style>nested</style></div></template>
<style bool-attr val-attr="test" module></style>
<example name="simple"><my-button>Hello</my-button></example>
<div><style>ignored</style></div>
"#,
            Vue27ParseComponentOptions::default(),
        );

        let descriptor = result.descriptor;
        assert_eq!(
            descriptor.template.as_ref().unwrap().content.trim(),
            "<div><style>nested</style></div>"
        );
        assert_eq!(descriptor.styles.len(), 1);
        assert_eq!(
            descriptor.styles[0].attrs.raw.get("bool-attr"),
            Some(&SfcAttrValue::Bool(true))
        );
        assert_eq!(
            descriptor.styles[0].attrs.raw.get("val-attr"),
            Some(&SfcAttrValue::String("test".into()))
        );
        assert_eq!(descriptor.styles[0].attrs.module.as_deref(), Some(""));
        assert_eq!(descriptor.custom_blocks.len(), 2);
        assert_eq!(descriptor.custom_blocks[0].type_name, "example");
        assert_eq!(
            descriptor.custom_blocks[0].content.trim(),
            "<my-button>Hello</my-button>"
        );
        assert_eq!(descriptor.custom_blocks[1].type_name, "div");
    }

    #[test]
    fn vue27_parse_component_deindents_like_official_parser() {
        let content = r#"<template>
        <div></div>
      </template>
      <script>
        export default {}
      </script>
      <style>
        h1 { color: red }
      </style>"#;
        let mut compiler = SfcCompiler::new();
        let default = compiler.parse_vue27_component(
            content,
            Vue27ParseComponentOptions {
                pad: Vue27SfcPad::False,
                ..Vue27ParseComponentOptions::default()
            },
        );
        assert_eq!(
            default.descriptor.template.unwrap().content,
            "\n<div></div>\n"
        );
        assert_eq!(
            default.descriptor.script.unwrap().content,
            "\n        export default {}\n      "
        );
        assert_eq!(
            default.descriptor.styles[0].content,
            "\nh1 { color: red }\n"
        );

        let enabled = compiler.parse_vue27_component(
            content,
            Vue27ParseComponentOptions {
                deindent: Some(true),
                ..Vue27ParseComponentOptions::default()
            },
        );
        assert_eq!(
            enabled.descriptor.script.unwrap().content,
            "\nexport default {}\n"
        );
    }

    #[test]
    fn vue27_parse_component_pads_non_template_content() {
        let content = r#"<template>
        <div></div>
      </template>
      <script>
        export default {}
      </script>
      <style>
        h1 { color: red }
      </style>"#;
        let mut compiler = SfcCompiler::new();
        let line = compiler.parse_vue27_component(
            content,
            Vue27ParseComponentOptions {
                pad: Vue27SfcPad::Line,
                deindent: Some(true),
                ..Vue27ParseComponentOptions::default()
            },
        );
        assert_eq!(
            line.descriptor.script.unwrap().content,
            format!("{}\nexport default {{}}\n", "//\n".repeat(3))
        );
        assert_eq!(
            line.descriptor.styles[0].content,
            "\n\n\n\n\n\n\nh1 { color: red }\n"
        );

        let space = compiler.parse_vue27_component(
            content,
            Vue27ParseComponentOptions {
                pad: Vue27SfcPad::Space,
                deindent: Some(true),
                ..Vue27ParseComponentOptions::default()
            },
        );
        let script_pad = content[..space.descriptor.script.as_ref().unwrap().content_start]
            .chars()
            .map(|ch| if matches!(ch, '\n' | '\r') { ch } else { ' ' })
            .collect::<String>();
        assert_eq!(
            space.descriptor.script.unwrap().content,
            script_pad + "\nexport default {}\n"
        );
    }

    #[test]
    fn vue27_parse_component_recovers_unclosed_template_with_source_range() {
        let mut compiler = SfcCompiler::new();
        let result = compiler.parse_vue27_component(
            "<template>hi</",
            Vue27ParseComponentOptions {
                output_source_range: true,
                ..Vue27ParseComponentOptions::default()
            },
        );

        assert_eq!(result.descriptor.template.unwrap().content, "hi");
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0].start, Some(0));
        assert_eq!(result.errors[0].end, Some(10));
    }

    #[test]
    fn vue27_rewrite_default_handles_default_declarations() {
        let compiler = SfcCompiler::new();
        assert_eq!(
            compiler.rewrite_vue27_default(
                "export  default {}",
                "script",
                Vue27RewriteDefaultOptions::default()
            ),
            "const script = {}"
        );
        assert_eq!(
            compiler.rewrite_vue27_default(
                "// export default\nexport default class Foo {}",
                "script",
                Vue27RewriteDefaultOptions::default()
            ),
            "// export default\nclass Foo {}\nconst script = Foo"
        );
    }

    #[test]
    fn vue27_rewrite_default_handles_named_default_exports() {
        let compiler = SfcCompiler::new();
        assert_eq!(
            compiler.rewrite_vue27_default(
                "const a = 1 \n export { a as b, a as default, a as c}",
                "script",
                Vue27RewriteDefaultOptions::default()
            ),
            "const a = 1 \n export { a as b,  a as c}\nconst script = a"
        );
        assert_eq!(
            compiler.rewrite_vue27_default(
                "export { default, foo } from './index.js'",
                "script",
                Vue27RewriteDefaultOptions::default()
            ),
            "import { default as __VUE_DEFAULT__ } from './index.js'\nexport {  foo } from './index.js'\nconst script = __VUE_DEFAULT__"
        );
        assert_eq!(
            compiler.rewrite_vue27_default(
                "export { foo as default, bar } from './index.js'",
                "script",
                Vue27RewriteDefaultOptions::default()
            ),
            "import { foo } from './index.js'\nexport {  bar } from './index.js'\nconst script = foo"
        );
    }

    #[test]
    fn vue27_rewrite_default_handles_typescript_decorated_classes() {
        let compiler = SfcCompiler::new();
        assert_eq!(
            compiler.rewrite_vue27_default(
                "@Component({})\nexport default class HelloWorld extends Vue {\n  test = \"\";\n}",
                "script",
                Vue27RewriteDefaultOptions {
                    typescript: true,
                    decorators: true,
                },
            ),
            "@Component({})\nclass HelloWorld extends Vue {\n  test = \"\";\n}\nconst script = HelloWorld"
        );
    }

    #[test]
    fn vue3_rewrite_default_handles_official_export_shapes() {
        let compiler = SfcCompiler::new();
        assert_eq!(
            compiler
                .rewrite_vue3_default(
                    "const a = 1",
                    "script",
                    Vue3RewriteDefaultOptions::default()
                )
                .unwrap(),
            "const a = 1\nconst script = {}"
        );
        assert_eq!(
            compiler
                .rewrite_vue3_default(
                    "export default {}",
                    "script",
                    Vue3RewriteDefaultOptions::default()
                )
                .unwrap(),
            "const script = {}"
        );
        assert_eq!(
            compiler
                .rewrite_vue3_default(
                    "export default function Foo() {}",
                    "__default__",
                    Vue3RewriteDefaultOptions::default()
                )
                .unwrap(),
            "const __default__ = function Foo() {}"
        );
        assert_eq!(
            compiler
                .rewrite_vue3_default(
                    "@Component\nexport default class Foo {}",
                    "script",
                    Vue3RewriteDefaultOptions { typescript: true },
                )
                .unwrap(),
            "@Component class Foo {}\nconst script = Foo"
        );
    }

    #[test]
    fn vue3_rewrite_default_handles_named_default_exports() {
        let compiler = SfcCompiler::new();
        assert_eq!(
            compiler
                .rewrite_vue3_default(
                    "const a = 1 \n export { a as b, a as default, a as c}",
                    "script",
                    Vue3RewriteDefaultOptions::default()
                )
                .unwrap(),
            "const a = 1 \n export { a as b,  a as c}\nconst script = a"
        );
        assert_eq!(
            compiler
                .rewrite_vue3_default(
                    "export { default, foo } from './index.js'",
                    "script",
                    Vue3RewriteDefaultOptions::default()
                )
                .unwrap(),
            "import { default as __VUE_DEFAULT__ } from './index.js'\nexport {  foo } from './index.js'\nconst script = __VUE_DEFAULT__"
        );
        assert_eq!(
            compiler
                .rewrite_vue3_default(
                    "export { foo as default, bar } from './index.js'",
                    "script",
                    Vue3RewriteDefaultOptions::default()
                )
                .unwrap(),
            "import { foo as __VUE_DEFAULT__ } from './index.js'\nexport {  bar } from './index.js'\nconst script = __VUE_DEFAULT__"
        );
    }

    #[test]
    fn vue3_rewrite_default_preserves_typescript_plugin_boundary() {
        let compiler = SfcCompiler::new();
        let without_ts = compiler
            .rewrite_vue3_default(
                "export default interface Foo {}",
                "__default__",
                Vue3RewriteDefaultOptions::default(),
            )
            .unwrap_err();
        assert!(without_ts.contains("Unexpected reserved word 'interface'. (1:15)"));

        assert_eq!(
            compiler
                .rewrite_vue3_default(
                    "export default interface Foo {}",
                    "__default__",
                    Vue3RewriteDefaultOptions { typescript: true },
                )
                .unwrap(),
            "const __default__ = interface Foo {}"
        );
    }

    #[test]
    fn vue3_compile_script_merges_normal_default_export_with_setup() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            "<script>export default { name: 'X' }</script><script setup>const a = 1</script>",
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script.content.contains("const __default__ = { name: 'X' }"));
        assert!(script
            .content
            .contains("export default /*@__PURE__*/Object.assign(__default__, {"));
        assert!(!script.content.contains("__name: 'Comp'"));
        assert!(script
            .content
            .contains("const a = 1\nconst __returned__ = { a }"));
        assert!(!script.content.contains("_defineComponent"));
    }

    #[test]
    fn vue3_compile_script_merges_named_default_export_with_setup() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            "<script>const def = {}; export { def as default }</script><script setup>const a = 1</script>",
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script.content.contains("const def = {};"));
        assert!(script.content.contains("const __default__ = def"));
        assert!(!script.content.contains("export {"));
        assert!(script
            .content
            .contains("export default /*@__PURE__*/Object.assign(__default__, {"));
        assert!(script.content.contains("__name: 'Comp'"));
        assert!(script.content.contains("const __returned__ = { def, a }"));
    }

    #[test]
    fn vue3_compile_script_keeps_normal_script_without_default_in_setup_compile() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            "<script>export const n = 1</script><script setup>const a = 1</script>",
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script.content.contains("export const n = 1"));
        assert!(script.content.contains("export default {"));
        assert!(!script.content.contains("const __default__ = {}"));
        assert!(!script.content.contains("Object.assign(__default__"));
        assert!(script.content.contains("const a = 1\nconst __returned__"));
    }

    #[test]
    fn vue3_compile_script_merges_typescript_default_with_define_component() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "Comp.vue",
            "<script lang=\"ts\">export default { name: 'X' }</script><script setup lang=\"ts\">const a: number = 1</script>",
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script
            .content
            .starts_with("import { defineComponent as _defineComponent } from 'vue'\n"));
        assert!(script.content.contains("const __default__ = { name: 'X' }"));
        assert!(script
            .content
            .contains("export default /*@__PURE__*/_defineComponent({\n  ...__default__,"));
        assert!(!script.content.contains("Object.assign(__default__"));
        assert!(script
            .content
            .contains("const a: number = 1\nconst __returned__ = { a }"));
    }

    #[test]
    fn vue3_compile_script_generates_runtime_macros() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
const props = defineProps({ foo: String })
const emit = defineEmits(['save'])
defineExpose({ reset() {} })
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script.content.contains("__name: 'FooBar',"));
        assert!(script.content.contains("props: { foo: String },"));
        assert!(script.content.contains("emits: ['save'],"));
        assert!(script
            .content
            .contains("setup(__props, { expose: __expose, emit: __emit })"));
        assert!(script.content.contains("const props = __props"));
        assert!(script.content.contains("const emit = __emit"));
        assert!(script.content.contains("__expose({ reset() {} })"));
        assert!(script
            .content
            .contains("const __returned__ = { props, emit }"));
        assert!(!script.content.contains("defineProps"));
        assert!(!script.content.contains("defineEmits"));
        assert!(!script.content.contains("defineExpose"));
        assert_eq!(
            script.bindings.get("foo").map(String::as_str),
            Some("props")
        );
        assert_eq!(
            script.bindings.get("props").map(String::as_str),
            Some("setup-reactive-const")
        );
        assert_eq!(
            script.bindings.get("emit").map(String::as_str),
            Some("setup-const")
        );
    }

    #[test]
    fn vue3_compile_script_rewrites_define_slots() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">
import { defineSlots } from 'vue'
const slots = defineSlots<{
  default: { msg: string }
}>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script.content.contains(
            "import { useSlots as _useSlots, defineComponent as _defineComponent } from 'vue'"
        ));
        assert!(script.content.contains("const slots = _useSlots()"));
        assert!(script.content.contains("const __returned__ = { slots }"));
        assert!(!script.content.contains("defineSlots"));
        assert_eq!(
            script.bindings.get("slots").map(String::as_str),
            Some("setup-const")
        );
        assert!(script.bindings.get("defineSlots").is_none());

        let unbound = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">
defineSlots<{
  default: { msg: string }
}>()
</script>"#,
        );
        let script = compiler.compile_script(&unbound, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(!script.content.contains("defineSlots"));
        assert!(!script.content.contains("_useSlots"));

        let runtime = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
const slots = defineSlots()
</script>"#,
        );
        let script = compiler.compile_script(&runtime, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script
            .content
            .contains("import { useSlots as _useSlots } from 'vue'"));
        assert!(script.content.contains("const slots = _useSlots()"));
        assert!(!script.content.contains("defineSlots"));
    }

    #[test]
    fn vue3_compile_script_reports_define_slots_errors() {
        let mut compiler = SfcCompiler::new();
        let duplicate = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
defineSlots()
defineSlots()
</script>"#,
        );
        let script = compiler.compile_script(&duplicate, SfcScriptCompileOptions::default());

        assert!(script
            .errors
            .iter()
            .any(|error| error.contains("duplicate defineSlots() call")));
        assert!(!script.content.contains("defineSlots"));
        assert!(!script.content.contains("_useSlots"));

        let arguments = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
const slots = defineSlots({})
</script>"#,
        );
        let script = compiler.compile_script(&arguments, SfcScriptCompileOptions::default());

        assert!(script
            .errors
            .iter()
            .any(|error| error.contains("defineSlots() cannot accept arguments")));
        assert!(script.content.contains("const slots = _useSlots()"));
        assert!(!script.content.contains("defineSlots"));
    }

    #[test]
    fn vue3_compile_script_rewrites_top_level_await() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
const a = 1 + (await foo)
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .starts_with("import { withAsyncContext as _withAsyncContext } from 'vue'\n"));
        assert!(script.content.contains("async setup("));
        assert!(script.content.contains("let __temp, __restore\n"));
        assert!(script
            .content
            .contains("([__temp,__restore] = _withAsyncContext(() => foo))"));
        assert!(script.content.contains("__temp = await __temp"));
        assert!(script.content.contains("__restore(),\n  __temp"));
        assert!(script.content.contains("const __returned__ = { a }"));
    }

    #[test]
    fn vue3_compile_script_top_level_await_ignores_function_scopes() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
async function foo() { await bar }
const fn = async () => { await bar }
const obj = { async method() { await bar }}
const cls = class Foo { async method() { await bar } }
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(!script.content.contains("_withAsyncContext"));
        assert!(!script.content.contains("async setup("));
        assert!(!script.content.contains("let __temp"));
        assert!(script
            .content
            .contains("async function foo() { await bar }"));
        assert!(script
            .content
            .contains("const obj = { async method() { await bar }}"));
        assert!(script
            .content
            .contains("const cls = class Foo { async method() { await bar } }"));
    }

    #[test]
    fn vue3_compile_script_top_level_await_handles_nested_and_semicolon() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
foo()
await 1 + await 2
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script.content.contains("foo()\n;("));
        assert!(script.content.matches("_withAsyncContext").count() >= 2);

        let nested = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
await (await foo)
</script>"#,
        );
        let script = compiler.compile_script(&nested, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script.content.contains("_withAsyncContext(async () => ("));
        assert!(script.content.matches("_withAsyncContext").count() >= 2);
    }

    #[test]
    fn vue3_compile_script_returns_template_used_ts_import_getters() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">
import { FooBar, FooBaz, FooQux, foo } from './x'
const fooBar: FooBar = 1
</script>
<template>
  <FooBaz></FooBaz>
  <foo-qux/>
  <foo/>
  FooBar
</template>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script.content.contains(
            "const __returned__ = { fooBar, get FooBaz() { return FooBaz }, get FooQux() { return FooQux }, get foo() { return foo } }"
        ));
        assert!(!script.content.contains("fooBar, FooBar,"));
        assert_eq!(
            script.bindings.get("FooBaz").map(String::as_str),
            Some("setup-maybe-ref")
        );
        assert_eq!(
            script.bindings.get("FooQux").map(String::as_str),
            Some("setup-maybe-ref")
        );
        assert_eq!(
            script.bindings.get("foo").map(String::as_str),
            Some("setup-maybe-ref")
        );
    }

    #[test]
    fn vue3_compile_script_template_import_usage_handles_directives_and_dynamic_args() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">
import { vMyDir, FooBar, foo, bar, unused, baz, msg } from './x'
</script>
<template>
  <div v-my-dir></div>
  <FooBar #[foo.slotName] />
  <FooBar #unused />
  <div :[bar.attrName]="15"></div>
  <div unused="unused"></div>
  <div #[`item:${baz.key}`]="{ value }"></div>
  <FooBar :msg />
</template>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script.content.contains(
            "const __returned__ = { get vMyDir() { return vMyDir }, get FooBar() { return FooBar }, get foo() { return foo }, get bar() { return bar }, get baz() { return baz }, get msg() { return msg } }"
        ));
        assert!(!script.content.contains("get unused()"));
    }

    #[test]
    fn vue3_compile_script_template_import_usage_ignores_ts_annotation_identifiers() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">
import { Foo, Bar, Baz, Qux, Fred } from './x'
const a = 1
function b() {}
</script>
<template>
  {{ a as Foo }}
  {{ b<Bar>() }}
  {{ Baz }}
  <Comp v-slot="{ data }: Qux">{{ data }}</Comp>
  <div v-for="{ z = x as Qux } in list as Fred"/>
</template>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .contains("const __returned__ = { a, b, get Baz() { return Baz } }"));
        assert!(!script.content.contains("get Foo()"));
        assert!(!script.content.contains("get Bar()"));
        assert!(!script.content.contains("get Qux()"));
        assert!(!script.content.contains("get Fred()"));
    }

    #[test]
    fn vue3_compile_script_return_binding_uses_setter_for_setup_let() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
let count = 0
let v = 1
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script.content.contains(
            "const __returned__ = { get count() { return count }, set count(v) { count = v }, get v() { return v }, set v(_v) { v = _v } }"
        ));
        assert_eq!(
            script.bindings.get("count").map(String::as_str),
            Some("setup-let")
        );
        assert_eq!(
            script.bindings.get("v").map(String::as_str),
            Some("setup-let")
        );
    }

    #[test]
    fn vue3_compile_script_reports_duplicate_define_expose() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
defineExpose({ first: true })
defineExpose({ second: true })
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script
            .errors
            .iter()
            .any(|error| error.contains("duplicate defineExpose() call")));
        assert!(script.content.contains("__expose({ first: true })"));
        assert!(script.content.contains("__expose({ second: true })"));
        assert!(!script.content.contains("defineExpose"));
        assert!(!script.content.contains("__expose();"));
    }

    #[test]
    fn vue3_compile_script_unbound_define_emits_only_generates_runtime_option() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
defineEmits(['save'])
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script.content.contains("emits: ['save'],"));
        assert!(script
            .content
            .contains("setup(__props, { expose: __expose })"));
        assert!(!script.content.contains("emit: __emit"));
        assert!(!script.content.contains("defineEmits"));
        assert!(script.bindings.is_empty());
    }

    #[test]
    fn vue3_compile_script_removes_define_props_destructure() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
const { foo, bar: baz } = defineProps({ foo: String, bar: Number })
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script
            .content
            .contains("props: { foo: String, bar: Number },"));
        assert!(script.content.contains("const __returned__ = {  }"));
        assert!(!script.content.contains("const { foo, bar: baz }"));
        assert!(!script.content.contains("defineProps"));
        assert_eq!(
            script.bindings.get("foo").map(String::as_str),
            Some("props")
        );
        assert_eq!(
            script.bindings.get("bar").map(String::as_str),
            Some("props")
        );
        assert_eq!(
            script.bindings.get("baz").map(String::as_str),
            Some("props-aliased")
        );
    }

    #[test]
    fn vue3_compile_script_rewrites_define_props_destructure_references() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
const { foo, bar: baz, 'foo.bar': fooBar } = defineProps({ foo: String, bar: Number, 'foo.bar': Boolean })
const message = foo + baz
const payload = { foo, baz, fooBar }
function read(foo) {
  return foo + baz
}
for (const baz of [1]) {
  console.log(baz, foo)
}
console.log(message, payload, fooBar)
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(!script
            .content
            .contains("const { foo, bar: baz, 'foo.bar': fooBar }"));
        assert!(script
            .content
            .contains("const message = __props.foo + __props.bar"));
        assert!(script.content.contains(
            r#"const payload = { foo: __props.foo, baz: __props.bar, fooBar: __props["foo.bar"] }"#
        ));
        assert!(script
            .content
            .contains("function read(foo) {\n  return foo + __props.bar\n}"));
        assert!(script.content.contains("console.log(baz, __props.foo)"));
        assert!(script
            .content
            .contains(r#"console.log(message, payload, __props["foo.bar"])"#));
        assert_eq!(
            script.bindings.get("foo").map(String::as_str),
            Some("props")
        );
        assert_eq!(
            script.bindings.get("baz").map(String::as_str),
            Some("props-aliased")
        );
        assert_eq!(
            script.bindings.get("fooBar").map(String::as_str),
            Some("props-aliased")
        );
    }

    #[test]
    fn vue3_compile_script_generates_define_props_destructure_rest_proxy() {
        let mut compiler = SfcCompiler::new();
        let runtime = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
const { foo, bar: baz, ...rest } = defineProps(['foo', 'bar', 'baz'])
const read = foo + baz + rest.baz
</script>"#,
        );
        let script = compiler.compile_script(&runtime, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .starts_with("import { createPropsRestProxy as _createPropsRestProxy } from 'vue'\n"));
        assert!(script
            .content
            .contains(r#"const rest = _createPropsRestProxy(__props, ["foo","bar"])"#));
        assert!(script
            .content
            .contains("const read = __props.foo + __props.bar + rest.baz"));
        assert!(!script.content.contains("const { foo, bar: baz, ...rest }"));
        assert!(!script.content.contains("defineProps"));
        assert!(script
            .content
            .contains("const __returned__ = { rest, read }"));
        assert_eq!(
            script.bindings.get("foo").map(String::as_str),
            Some("props")
        );
        assert_eq!(
            script.bindings.get("bar").map(String::as_str),
            Some("props")
        );
        assert_eq!(
            script.bindings.get("baz").map(String::as_str),
            Some("props-aliased")
        );
        assert_eq!(
            script.bindings.get("rest").map(String::as_str),
            Some("setup-reactive-const")
        );

        let typed = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">
const { foo, ...rest } = defineProps<{ foo?: string, bar?: number }>()
</script>"#,
        );
        let script = compiler.compile_script(&typed, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script.content.starts_with(
            "import { createPropsRestProxy as _createPropsRestProxy, defineComponent as _defineComponent } from 'vue'\n"
        ));
        assert!(script.content.contains("setup(__props: any"));
        assert!(script
            .content
            .contains(r#"const rest = _createPropsRestProxy(__props, ["foo"])"#));
    }

    #[test]
    fn vue3_compile_script_merges_define_props_destructure_defaults() {
        let mut compiler = SfcCompiler::new();
        let runtime = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
const external = 'x'
const { foo = 1, bar = {}, func = () => {}, ext = external, 'foo:bar': fooBar = 'foo-bar' } = defineProps(['foo', 'bar', 'func', 'ext', 'foo:bar'])
</script>"#,
        );
        let script = compiler.compile_script(&runtime, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .starts_with("import { mergeDefaults as _mergeDefaults } from 'vue'\n"));
        assert!(script.content.contains(
            "props: /*@__PURE__*/_mergeDefaults(['foo', 'bar', 'func', 'ext', 'foo:bar'], {"
        ));
        assert!(script.content.contains("foo: 1"));
        assert!(script.content.contains("bar: () => ({})"));
        assert!(script.content.contains("func: () => {}, __skip_func: true"));
        assert!(script.content.contains("ext: external, __skip_ext: true"));
        assert!(script.content.contains(r#""foo:bar": 'foo-bar'"#));
        assert!(!script.content.contains("const { foo = 1"));

        let typed = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">
const { foo = 1, bar = {}, func = () => {}, label = 'x' } = defineProps<{
  foo?: number
  bar?: object
  func?: () => void
  label?: string
}>()
</script>"#,
        );
        let script = compiler.compile_script(&typed, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script
            .content
            .starts_with("import { defineComponent as _defineComponent } from 'vue'\n"));
        assert!(script
            .content
            .contains("foo: { type: Number, required: false, default: 1 }"));
        assert!(script
            .content
            .contains("bar: { type: Object, required: false, default: () => ({}) }"));
        assert!(script
            .content
            .contains("func: { type: Function, required: false, default: () => {} }"));
        assert!(script
            .content
            .contains("label: { type: String, required: false, default: 'x' }"));

        let prod = compiler.compile_script(
            &typed,
            SfcScriptCompileOptions {
                is_prod: true,
                ..SfcScriptCompileOptions::default()
            },
        );
        assert!(prod.errors.is_empty(), "{:?}", prod.errors);
        assert!(prod.content.contains("foo: { default: 1 }"));
        assert!(prod.content.contains("bar: { default: () => ({}) }"));
        assert!(prod
            .content
            .contains("func: { type: Function, default: () => {} }"));
        assert!(prod.content.contains("label: { default: 'x' }"));
    }

    #[test]
    fn vue3_compile_script_merges_default_with_runtime_macros() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script>export default { name: 'X' }</script>
<script setup>
const props = defineProps({ foo: String })
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script.content.contains("const __default__ = { name: 'X' }"));
        assert!(script
            .content
            .contains("export default /*@__PURE__*/Object.assign(__default__, {"));
        assert!(!script.content.contains("__name: 'FooBar'"));
        assert!(script.content.contains("props: { foo: String },"));
        assert!(script.content.contains("const props = __props"));
        assert_eq!(
            script.bindings.get("foo").map(String::as_str),
            Some("props")
        );
    }

    #[test]
    fn vue3_compile_script_wraps_typescript_runtime_macros() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">
const props = defineProps({ foo: String })
const emit = defineEmits(['save'])
defineExpose({ reset() {} })
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script
            .content
            .starts_with("import { defineComponent as _defineComponent } from 'vue'\n"));
        assert!(script
            .content
            .contains("export default /*@__PURE__*/_defineComponent({"));
        assert!(script.content.contains("props: { foo: String },"));
        assert!(script.content.contains("emits: ['save'],"));
        assert!(script.content.contains("const props = __props"));
        assert!(script.content.contains("const emit = __emit"));
    }

    #[test]
    fn vue3_compile_script_merges_define_options_runtime() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
import { defineOptions, ref } from 'vue'
defineOptions({ name: 'FooApp', inheritAttrs: false })
const a = ref(1)
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script.content.contains("import { ref } from 'vue'"));
        assert!(script.content.contains(
            "export default /*@__PURE__*/Object.assign({ name: 'FooApp', inheritAttrs: false }, {"
        ));
        assert!(script.content.contains("__name: 'FooBar',"));
        assert!(script.content.contains("const __returned__ = { a, ref }"));
        assert!(!script.content.contains("defineOptions"));

        let empty = compiler.parse("FooBar.vue", "<script setup>defineOptions()</script>");
        let empty_script = compiler.compile_script(&empty, SfcScriptCompileOptions::default());
        assert!(empty_script.errors.is_empty());
        assert!(empty_script.content.contains("export default {"));
        assert!(!empty_script.content.contains("Object.assign"));
        assert!(!empty_script.content.contains("defineOptions"));
    }

    #[test]
    fn vue3_compile_script_spreads_define_options_in_typescript_wrapper() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script lang="ts">export default { custom: true }</script>
<script setup lang="ts">
defineOptions({ name: 'FooApp' } as any)
const a: number = 1
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script
            .content
            .starts_with("import { defineComponent as _defineComponent } from 'vue'\n"));
        assert!(script
            .content
            .contains("const __default__ = { custom: true }"));
        assert!(script.content.contains(
            "export default /*@__PURE__*/_defineComponent({\n  ...__default__,\n  ...{ name: 'FooApp' },"
        ));
        assert!(script
            .content
            .contains("const a: number = 1\nconst __returned__ = { a }"));
        assert!(!script.content.contains("defineOptions"));
    }

    #[test]
    fn vue3_compile_script_reports_define_options_errors() {
        let mut compiler = SfcCompiler::new();
        let duplicate = compiler.parse(
            "FooBar.vue",
            "<script setup>defineOptions({}); defineOptions({})</script>",
        );
        let script = compiler.compile_script(&duplicate, SfcScriptCompileOptions::default());
        assert!(script
            .errors
            .iter()
            .any(|error| error.contains("duplicate defineOptions() call")));

        let invalid_option = compiler.parse(
            "FooBar.vue",
            "<script setup>defineOptions({ props: [] })</script>",
        );
        let script = compiler.compile_script(&invalid_option, SfcScriptCompileOptions::default());
        assert!(script
            .errors
            .iter()
            .any(|error| error.contains("cannot be used to declare props")));

        let string_key = compiler.parse(
            "FooBar.vue",
            "<script setup>defineOptions({ 'props': [] })</script>",
        );
        let script = compiler.compile_script(&string_key, SfcScriptCompileOptions::default());
        assert!(script.errors.is_empty());

        let type_argument = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">defineOptions<{ name: 'FooApp' }>()</script>"#,
        );
        let script = compiler.compile_script(&type_argument, SfcScriptCompileOptions::default());
        assert!(script
            .errors
            .iter()
            .any(|error| error.contains("cannot accept type arguments")));

        let assigned = compiler.parse(
            "FooBar.vue",
            "<script setup>const options = defineOptions({ name: 'FooApp' })</script>",
        );
        let script = compiler.compile_script(&assigned, SfcScriptCompileOptions::default());
        assert!(script
            .errors
            .iter()
            .any(|error| error.contains("has no returning value")));

        let aliased = compiler.parse(
            "FooBar.vue",
            "<script setup>import { defineOptions as d } from 'vue'\nd({ name: 'FooApp' })</script>",
        );
        let script = compiler.compile_script(&aliased, SfcScriptCompileOptions::default());
        assert!(script
            .errors
            .iter()
            .any(|error| error.contains("cannot be aliased to a different name")));
        assert!(!script.content.contains("defineOptions as d"));
    }

    #[test]
    fn vue3_compile_script_generates_define_model_runtime() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
import { defineModel, ref } from 'vue'
const modelValue = defineModel({ required: true })
const c = defineModel('count')
const title = defineModel(`title`, { default: 'x' })
const other = ref(1)
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script
            .content
            .contains("import { useModel as _useModel } from 'vue'"));
        assert!(script.content.contains("import { ref } from 'vue'"));
        assert!(script
            .content
            .contains("\"modelValue\": { required: true },"));
        assert!(script.content.contains("\"modelModifiers\": {},"));
        assert!(script.content.contains("\"count\": {},"));
        assert!(script.content.contains("\"countModifiers\": {},"));
        assert!(script.content.contains("\"title\": { default: 'x' },"));
        assert!(script.content.contains("\"titleModifiers\": {},"));
        assert!(script
            .content
            .contains("emits: [\"update:modelValue\", \"update:count\", \"update:title\"],"));
        assert!(script
            .content
            .contains(r#"const modelValue = _useModel(__props, "modelValue")"#));
        assert!(script
            .content
            .contains("const c = _useModel(__props, 'count')"));
        assert!(script
            .content
            .contains("const title = _useModel(__props, `title`)"));
        assert!(script
            .content
            .contains("const __returned__ = { modelValue, c, title, other, ref }"));
        assert!(!script.content.contains("defineModel"));
        assert_eq!(
            script.bindings.get("modelValue").map(String::as_str),
            Some("setup-ref")
        );
        assert_eq!(
            script.bindings.get("c").map(String::as_str),
            Some("setup-ref")
        );
        assert_eq!(
            script.bindings.get("count").map(String::as_str),
            Some("props")
        );
        assert_eq!(
            script.bindings.get("title").map(String::as_str),
            Some("setup-ref")
        );
        assert!(script.bindings.get("defineModel").is_none());
    }

    #[test]
    fn vue3_compile_script_merges_define_model_with_props_and_emits() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
defineProps({ foo: String })
defineEmits(['change'])
const count = defineModel({ default: 0 })
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script
            .content
            .contains("import { useModel as _useModel, mergeModels as _mergeModels } from 'vue'"));
        assert!(script
            .content
            .contains("props: /*@__PURE__*/_mergeModels({ foo: String }, {"));
        assert!(script.content.contains("\"modelValue\": { default: 0 },"));
        assert!(script.content.contains("\"modelModifiers\": {},"));
        assert!(script
            .content
            .contains("emits: /*@__PURE__*/_mergeModels(['change'], [\"update:modelValue\"]),"));
        assert!(script
            .content
            .contains(r#"const count = _useModel(__props, "modelValue")"#));
        assert!(!script.content.contains("defineModel"));
        assert_eq!(
            script.bindings.get("foo").map(String::as_str),
            Some("props")
        );
        assert_eq!(
            script.bindings.get("modelValue").map(String::as_str),
            Some("props")
        );
        assert_eq!(
            script.bindings.get("count").map(String::as_str),
            Some("setup-ref")
        );
    }

    #[test]
    fn vue3_compile_script_reports_duplicate_define_model_names() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
const a = defineModel('count')
const b = defineModel('count')
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script
            .errors
            .iter()
            .any(|error| error.contains("duplicate model name \"count\"")));
    }

    #[test]
    fn vue3_compile_script_rewrites_unbound_define_model_expression() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
defineModel('count')
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script.content.contains("\"count\": {},"));
        assert!(script.content.contains("\"countModifiers\": {},"));
        assert!(script.content.contains(r#"emits: ["update:count"],"#));
        assert!(
            script.content.contains(" _useModel(__props, 'count')")
                || script.content.contains("\n_useModel(__props, 'count')")
        );
        assert!(!script.content.contains("defineModel"));
        assert_eq!(
            script.bindings.get("count").map(String::as_str),
            Some("props")
        );
    }

    #[test]
    fn vue3_compile_script_splits_define_model_get_set_transformers() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">
const modelValue = defineModel({
  get(v) { return v - 1 },
  set: (v) => { return v + 1 },
  required: true
})
const count = defineModel('count', {
  default: 0,
  get(v) { return v - 1 },
  required: true,
  set: (v) => { return v + 1 },
})
const value = defineModel<number>('value', {
  get(v) { return v },
  required: true,
})
const only = defineModel('only', {
  "get": (v) => v - 1,
  "set": (v) => v + 1,
})
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        let compact = compact_js_whitespace(&script.content);
        assert!(compact.contains("\"modelValue\": { required: true },"));
        assert!(compact.contains("\"count\": { default: 0, required: true, },"));
        assert!(compact.contains("\"value\": { type: Number, ...{ required: true, } },"));
        assert!(compact.contains("\"only\": { },"));
        assert!(compact.contains("const modelValue = _useModel(__props, \"modelValue\", { get(v) { return v - 1 }, set: (v) => { return v + 1 }, })"));
        assert!(compact.contains("const count = _useModel(__props, 'count', { get(v) { return v - 1 }, set: (v) => { return v + 1 }, })"));
        assert!(compact.contains(
            "const value = _useModel<number>(__props, 'value', { get(v) { return v }, })"
        ));
        assert!(compact.contains("const only = _useModel(__props, 'only', { \"get\": (v) => v - 1, \"set\": (v) => v + 1, })"));
        assert!(!script.content.contains("defineModel"));
        assert_eq!(
            script.bindings.get("modelValue").map(String::as_str),
            Some("setup-ref")
        );
        assert_eq!(
            script.bindings.get("count").map(String::as_str),
            Some("setup-ref")
        );
        assert_eq!(
            script.bindings.get("value").map(String::as_str),
            Some("setup-ref")
        );
    }

    #[test]
    fn vue3_compile_script_keeps_dynamic_define_model_options_unsplit() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
const extra = { required: true }
const key = 'required'
const spread = defineModel({ get(v) { return v }, ...extra })
const computed = defineModel('computed', { get(v) { return v }, [key]: true })
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script
            .content
            .contains("\"modelValue\": { get(v) { return v }, ...extra },"));
        assert!(script
            .content
            .contains("\"computed\": { get(v) { return v }, [key]: true },"));
        assert!(script.content.contains(
            "const spread = _useModel(__props, \"modelValue\", { get(v) { return v }, ...extra })"
        ));
        assert!(script.content.contains(
            "const computed = _useModel(__props, 'computed', { get(v) { return v }, [key]: true })"
        ));
        assert!(!script.content.contains("defineModel"));
    }

    #[test]
    fn vue3_compile_script_infers_define_model_typescript_runtime_options() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">
const modelValue = defineModel<boolean | string>()
const count = defineModel<number>('count')
const disabled = defineModel<number>('disabled', { required: false })
const any = defineModel<any | boolean>('any')
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script.content.contains(
            "import { useModel as _useModel, defineComponent as _defineComponent } from 'vue'"
        ));
        assert!(script
            .content
            .contains("\"modelValue\": { type: [Boolean, String] },"));
        assert!(script.content.contains("\"modelModifiers\": {},"));
        assert!(script.content.contains("\"count\": { type: Number },"));
        assert!(script
            .content
            .contains("\"disabled\": { type: Number, ...{ required: false } },"));
        assert!(script
            .content
            .contains("\"any\": { type: Boolean, skipCheck: true },"));
        assert!(script.content.contains(
            "emits: [\"update:modelValue\", \"update:count\", \"update:disabled\", \"update:any\"],"
        ));
        assert!(script
            .content
            .contains(r#"const modelValue = _useModel<boolean | string>(__props, "modelValue")"#));
        assert!(script
            .content
            .contains("const count = _useModel<number>(__props, 'count')"));
        assert!(script
            .content
            .contains("const disabled = _useModel<number>(__props, 'disabled')"));
        assert!(script
            .content
            .contains("const any = _useModel<any | boolean>(__props, 'any')"));
        assert!(!script.content.contains("defineModel"));
        assert_eq!(
            script.bindings.get("modelValue").map(String::as_str),
            Some("setup-ref")
        );
        assert_eq!(
            script.bindings.get("count").map(String::as_str),
            Some("setup-ref")
        );
        assert_eq!(
            script.bindings.get("disabled").map(String::as_str),
            Some("setup-ref")
        );
        assert_eq!(
            script.bindings.get("any").map(String::as_str),
            Some("setup-ref")
        );
    }

    #[test]
    fn vue3_compile_script_erases_define_model_types_in_production() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">
const modelValue = defineModel<boolean>()
const fn = defineModel<() => void>('fn')
const fnWithDefault = defineModel<() => void>('fnWithDefault', { default: () => null })
const str = defineModel<string>('str')
const optional = defineModel<string>('optional', { required: false })
</script>"#,
        );
        let script = compiler.compile_script(
            &descriptor,
            SfcScriptCompileOptions {
                is_prod: true,
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(script.errors.is_empty());
        assert!(script
            .content
            .contains("\"modelValue\": { type: Boolean },"));
        assert!(script.content.contains("\"fn\": {},"));
        assert!(script
            .content
            .contains("\"fnWithDefault\": { type: Function, ...{ default: () => null } },"));
        assert!(script.content.contains("\"str\": {},"));
        assert!(script
            .content
            .contains("\"optional\": { required: false },"));
        assert!(script.content.contains(
            "emits: [\"update:modelValue\", \"update:fn\", \"update:fnWithDefault\", \"update:str\", \"update:optional\"],"
        ));
        assert!(script
            .content
            .contains(r#"const modelValue = _useModel<boolean>(__props, "modelValue")"#));
        assert!(script
            .content
            .contains("const fn = _useModel<() => void>(__props, 'fn')"));
        assert!(script
            .content
            .contains("const str = _useModel<string>(__props, 'str')"));

        let mixed = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">
const modelValue = defineModel<boolean | string | {}>()
const value = defineModel<number | (() => number)>('value', { default: () => 1 })
</script>"#,
        );
        let mixed_script = compiler.compile_script(
            &mixed,
            SfcScriptCompileOptions {
                is_prod: true,
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(mixed_script.errors.is_empty());
        assert!(mixed_script
            .content
            .contains("\"modelValue\": { type: [Boolean, String, Object] },"));
        assert!(mixed_script
            .content
            .contains("\"value\": { type: [Number, Function], ...{ default: () => 1 } },"));
    }

    #[test]
    fn vue3_compile_script_resolves_define_model_type_aliases() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script lang="ts">
type NormalMaybe = any | boolean
</script>
<script setup lang="ts">
type SetupMaybe = any | boolean
const setupAlias = defineModel<SetupMaybe>('setupAlias')
const normalAlias = defineModel<NormalMaybe>('normalAlias')
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script
            .content
            .contains("\"setupAlias\": { type: Boolean, skipCheck: true },"));
        assert!(script
            .content
            .contains("\"normalAlias\": { type: Boolean, skipCheck: true },"));
        assert!(script
            .content
            .contains("const setupAlias = _useModel<SetupMaybe>(__props, 'setupAlias')"));
        assert!(script
            .content
            .contains("const normalAlias = _useModel<NormalMaybe>(__props, 'normalAlias')"));
    }

    #[test]
    fn vue3_compile_script_infers_typescript_macro_runtime_options() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">
interface Props { foo: string; "foo-bar"?: number }
type Emits = {(e: 'save'): void; (e: 'cancel', id: number): void}
const props = defineProps<Props>()
const emit = defineEmits<Emits>()
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script.content.contains("interface Props"));
        assert!(script
            .content
            .contains("foo: { type: String, required: true }"));
        assert!(script
            .content
            .contains("\"foo-bar\": { type: Number, required: false }"));
        assert!(script.content.contains(r#"emits: ["save", "cancel"],"#));
        assert!(script
            .content
            .contains("setup(__props: any, { expose: __expose, emit: __emit })"));
        assert!(script.content.contains("const props = __props"));
        assert!(script.content.contains("const emit = __emit"));
        assert_eq!(
            script.bindings.get("foo").map(String::as_str),
            Some("props")
        );
        assert_eq!(
            script.bindings.get("foo-bar").map(String::as_str),
            Some("props")
        );
        assert_eq!(
            script.bindings.get("props").map(String::as_str),
            Some("setup-reactive-const")
        );
        assert_eq!(
            script.bindings.get("emit").map(String::as_str),
            Some("setup-const")
        );
    }

    #[test]
    fn vue3_compile_script_infers_with_defaults_runtime_props() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">
const props = withDefaults(defineProps<{
  foo?: string
  count?: number
  ok?: boolean
  list?: string[]
  fn?: () => void
}>(), {
  foo: 'hi',
  count: 1,
  ok: true,
  list: () => [],
  fn() {}
})
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script
            .content
            .contains("foo: { type: String, required: false, default: 'hi' }"));
        assert!(script
            .content
            .contains("count: { type: Number, required: false, default: 1 }"));
        assert!(script
            .content
            .contains("ok: { type: Boolean, required: false, default: true }"));
        assert!(script
            .content
            .contains("list: { type: Array, required: false, default: () => [] }"));
        assert!(script
            .content
            .contains("fn: { type: Function, required: false, default() {} }"));
        assert!(script.content.contains("const props = __props"));
        assert_eq!(
            script.bindings.get("props").map(String::as_str),
            Some("setup-const")
        );

        let prod = compiler.compile_script(
            &descriptor,
            SfcScriptCompileOptions {
                is_prod: true,
                ..SfcScriptCompileOptions::default()
            },
        );
        assert!(prod.errors.is_empty());
        assert!(prod.content.contains("foo: { default: 'hi' }"));
        assert!(prod.content.contains("count: { default: 1 }"));
        assert!(prod
            .content
            .contains("ok: { type: Boolean, default: true }"));
        assert!(prod.content.contains("list: { default: () => [] }"));
        assert!(prod
            .content
            .contains("fn: { type: Function, default() {} }"));
        assert!(!prod.content.contains("required:"));
    }

    #[test]
    fn vue3_compile_script_wraps_dynamic_with_defaults_runtime_props() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script lang="ts">const defaults = { foo: 'hi' }</script>
<script setup lang="ts">
const props = withDefaults(defineProps<{
  foo?: string
  ok?: boolean
  fn?: () => void
}>(), defaults)
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script.content.starts_with(
            "import { mergeDefaults as _mergeDefaults, defineComponent as _defineComponent } from 'vue'\n"
        ));
        assert!(script.content.contains("const defaults = { foo: 'hi' }"));
        assert!(script
            .content
            .contains("props: /*@__PURE__*/_mergeDefaults({"));
        assert!(script
            .content
            .contains("foo: { type: String, required: false }"));
        assert!(script
            .content
            .contains("ok: { type: Boolean, required: false }"));
        assert!(script
            .content
            .contains("fn: { type: Function, required: false }"));
        assert!(script.content.contains("}, defaults),"));

        let prod = compiler.compile_script(
            &descriptor,
            SfcScriptCompileOptions {
                is_prod: true,
                ..SfcScriptCompileOptions::default()
            },
        );
        assert!(prod.errors.is_empty());
        assert!(prod.content.contains("foo: {}"));
        assert!(prod.content.contains("ok: { type: Boolean }"));
        assert!(prod.content.contains("fn: { type: Function }"));
        assert!(prod.content.contains("}, defaults),"));
    }

    #[test]
    fn vue3_compile_script_removes_with_defaults_imports() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">
import { withDefaults, defineProps, ref } from 'vue'
const props = withDefaults(defineProps<{ foo?: string }>(), { foo: 'x' })
const count = ref(1)
</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script
            .content
            .contains("import { defineComponent as _defineComponent } from 'vue'"));
        assert!(script.content.contains("import { ref } from 'vue'"));
        assert!(script
            .content
            .contains("foo: { type: String, required: false, default: 'x' }"));
        assert!(script.content.contains("const props = __props"));
        assert!(!script.content.contains("withDefaults"));
        assert!(!script.content.contains("defineProps"));
        assert!(script.bindings.get("withDefaults").is_none());
        assert!(script.bindings.get("defineProps").is_none());
    }

    #[test]
    fn vue3_compile_script_reports_with_defaults_errors() {
        let mut compiler = SfcCompiler::new();
        let bad_first = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">
const props = withDefaults(foo(), { foo: 'x' })
</script>"#,
        );
        let script = compiler.compile_script(&bad_first, SfcScriptCompileOptions::default());
        assert!(
            script
                .errors
                .iter()
                .any(|error| error
                    .contains("withDefaults' first argument must be a defineProps call"))
        );
        assert!(!script.content.contains("withDefaults"));

        let runtime_props = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
const props = withDefaults(defineProps({ foo: String }), { foo: 'x' })
</script>"#,
        );
        let script = compiler.compile_script(&runtime_props, SfcScriptCompileOptions::default());
        assert!(script.errors.iter().any(|error| error
            .contains("withDefaults can only be used with type-based defineProps declaration")));
        assert!(script.content.contains("props: { foo: String },"));
        assert!(!script.content.contains("withDefaults"));
        assert!(!script.content.contains("defineProps"));

        let missing_defaults = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">
const props = withDefaults(defineProps<{ foo?: string }>())
</script>"#,
        );
        let script = compiler.compile_script(&missing_defaults, SfcScriptCompileOptions::default());
        assert!(script
            .errors
            .iter()
            .any(|error| error.contains("The 2nd argument of withDefaults is required")));
        assert!(script
            .content
            .contains("foo: { type: String, required: false }"));
        assert!(!script.content.contains("withDefaults"));
    }

    #[test]
    fn vue3_compile_script_reports_duplicate_define_props_and_emits() {
        let mut compiler = SfcCompiler::new();
        let duplicate_props = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">
defineProps<{ foo?: string }>()
const props = withDefaults(defineProps<{ bar?: number }>(), { bar: 1 })
</script>"#,
        );
        let script = compiler.compile_script(&duplicate_props, SfcScriptCompileOptions::default());

        assert!(script
            .errors
            .iter()
            .any(|error| error.contains("duplicate defineProps() call")));
        assert!(!script.content.contains("defineProps"));
        assert!(!script.content.contains("withDefaults"));

        let duplicate_emits = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
defineEmits(['save'])
const emit = defineEmits(['cancel'])
</script>"#,
        );
        let script = compiler.compile_script(&duplicate_emits, SfcScriptCompileOptions::default());

        assert!(script
            .errors
            .iter()
            .any(|error| error.contains("duplicate defineEmits() call")));
        assert!(script.content.contains("const emit = __emit"));
        assert!(!script.content.contains("defineEmits"));
    }

    #[test]
    fn vue3_compile_script_reports_define_props_destructure_errors() {
        let mut compiler = SfcCompiler::new();
        let dynamic_key = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
const key = 'foo'
const { [key]: foo } = defineProps(['foo'])
</script>"#,
        );
        let script = compiler.compile_script(&dynamic_key, SfcScriptCompileOptions::default());

        assert!(script
            .errors
            .iter()
            .any(|error| error.contains("destructure cannot use computed key")));

        let nested_pattern = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
const { foo: { bar } } = defineProps(['foo'])
</script>"#,
        );
        let script = compiler.compile_script(&nested_pattern, SfcScriptCompileOptions::default());

        assert!(script
            .errors
            .iter()
            .any(|error| error.contains("destructure does not support nested patterns")));

        let local_default = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
let x = 1
const { foo = () => x } = defineProps(['foo'])
</script>"#,
        );
        let script = compiler.compile_script(&local_default, SfcScriptCompileOptions::default());

        assert!(script
            .errors
            .iter()
            .any(|error| error.contains("cannot reference locally declared variables")));

        let literal_const_default = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
const x = 1
const { foo = x } = defineProps(['foo'])
</script>"#,
        );
        let script =
            compiler.compile_script(&literal_const_default, SfcScriptCompileOptions::default());
        assert!(script.errors.is_empty());

        let static_computed_key = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
const { ['foo']: foo } = defineProps(['foo'])
</script>"#,
        );
        let script =
            compiler.compile_script(&static_computed_key, SfcScriptCompileOptions::default());
        assert!(script.errors.is_empty());
        assert_eq!(
            script.bindings.get("foo").map(String::as_str),
            Some("props")
        );
    }

    #[test]
    fn vue3_compile_script_reports_define_props_destructure_usage_errors() {
        let mut compiler = SfcCompiler::new();
        let assignment = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
const { foo } = defineProps(['foo'])
foo = 'bar'
</script>"#,
        );
        let script = compiler.compile_script(&assignment, SfcScriptCompileOptions::default());

        assert!(script
            .errors
            .iter()
            .any(|error| error.contains("Cannot assign to destructured props")));

        let update = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
let { foo } = defineProps(['foo'])
foo++
</script>"#,
        );
        let script = compiler.compile_script(&update, SfcScriptCompileOptions::default());

        assert!(script
            .errors
            .iter()
            .any(|error| error.contains("Cannot assign to destructured props")));

        let watch_alias = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
import { watch as w } from 'vue'
const { foo } = defineProps(['foo'])
w(foo, () => {})
</script>"#,
        );
        let script = compiler.compile_script(&watch_alias, SfcScriptCompileOptions::default());

        assert!(script.errors.iter().any(|error| {
            error.contains(
                "\"foo\" is a destructured prop and should not be passed directly to watch().",
            )
        }));

        let normal_script_watch_alias = compiler.parse(
            "FooBar.vue",
            r#"<script>
import { watch as w } from 'vue'
</script>
<script setup>
const { foo } = defineProps(['foo'])
w(foo, () => {})
</script>"#,
        );
        let script = compiler.compile_script(
            &normal_script_watch_alias,
            SfcScriptCompileOptions::default(),
        );

        assert!(script.errors.iter().any(|error| {
            error.contains(
                "\"foo\" is a destructured prop and should not be passed directly to watch().",
            )
        }));

        let spread_argument = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
import { watch } from 'vue'
const { foo } = defineProps(['foo'])
watch(...[foo])
</script>"#,
        );
        let script = compiler.compile_script(&spread_argument, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);

        let to_ref_alias = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
import { toRef as r } from 'vue'
const { foo } = defineProps(['foo'])
r(foo)
</script>"#,
        );
        let script = compiler.compile_script(&to_ref_alias, SfcScriptCompileOptions::default());

        assert!(script.errors.iter().any(|error| {
            error.contains(
                "\"foo\" is a destructured prop and should not be passed directly to toRef().",
            )
        }));

        let shadowed = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
import { watch } from 'vue'
const { foo } = defineProps(['foo'])
function useLocal(foo) {
  watch(foo, () => {})
  foo++
}
const run = (foo = 1) => {
  foo++
}
</script>"#,
        );
        let script = compiler.compile_script(&shadowed, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty(), "{:?}", script.errors);
    }

    #[test]
    fn vue3_compile_script_reports_define_props_destructure_default_type_errors() {
        let mut compiler = SfcCompiler::new();
        let mismatch = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">
const { foo = 'hello' } = defineProps<{ foo?: number }>()
</script>"#,
        );
        let script = compiler.compile_script(&mismatch, SfcScriptCompileOptions::default());

        assert!(script.errors.iter().any(|error| {
            error.contains("Default value of prop \"foo\" does not match declared type.")
        }));

        let matching = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">
const { foo = 1, bar = 'ok', enabled = true, items = [], options = {}, run = () => {} } = defineProps<{
  foo?: number
  bar?: string
  enabled?: boolean
  items?: string[]
  options?: object
  run?: () => void
}>()
</script>"#,
        );
        let script = compiler.compile_script(&matching, SfcScriptCompileOptions::default());
        assert!(script.errors.is_empty(), "{:?}", script.errors);

        let nullable = compiler.parse(
            "FooBar.vue",
            r#"<script setup lang="ts">
const { foo = 'hello' } = defineProps<{ foo?: number | null }>()
</script>"#,
        );
        let script = compiler.compile_script(&nullable, SfcScriptCompileOptions::default());
        assert!(script.errors.is_empty(), "{:?}", script.errors);

        let runtime_declaration = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
const { foo = 'hello' } = defineProps({ foo: Number })
</script>"#,
        );
        let script =
            compiler.compile_script(&runtime_declaration, SfcScriptCompileOptions::default());
        assert!(script.errors.is_empty(), "{:?}", script.errors);
    }

    #[test]
    fn vue3_compile_script_hoists_setup_only_static_literals() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "contract.vue",
            r#"<template><div>{{ msg }}</div></template><script setup lang="ts">const msg = 'x'</script>"#,
        );
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.errors.is_empty());
        assert!(script
            .content
            .starts_with("import { defineComponent as _defineComponent } from 'vue'\nconst msg = 'x'\nexport default /*@__PURE__*/_defineComponent({"));
        assert!(script.content.contains("const __returned__ = { msg }"));
        assert_eq!(
            script.bindings.get("msg").map(String::as_str),
            Some("literal-const")
        );
    }

    #[test]
    fn vue3_compile_script_inlines_template_render() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script setup>
import { ref } from 'vue'
import ChildComp from './ChildComp.vue'
const count = ref(0)
const local = 1
const { title: heading } = defineProps(['title'])
</script>
<template><div>{{ count }} {{ local }} {{ heading }}</div><ChildComp /></template>"#,
        );
        let script = compiler.compile_script(
            &descriptor,
            SfcScriptCompileOptions {
                inline_template: true,
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script.content.contains("unref as _unref"));
        assert!(script
            .content
            .contains("toDisplayString as _toDisplayString"));
        assert!(script.content.contains("openBlock as _openBlock"));
        assert!(script
            .content
            .contains("createElementBlock as _createElementBlock"));
        assert!(script.content.contains("import { ref } from 'vue'"));
        assert!(script.content.contains("props: ['title'],"));
        assert!(script.content.contains("return (_ctx, _cache) => {"));
        assert!(script.content.contains("_unref(count)"));
        assert!(script.content.contains("_toDisplayString(local)"));
        assert!(script.content.contains("_toDisplayString(__props.title)"));
        assert!(script.content.contains("_createVNode(ChildComp)"));
        assert!(!script.content.contains("const __returned__"));
        assert!(!script
            .content
            .contains("Object.defineProperty(__returned__"));
        assert_eq!(
            script.bindings.get("heading").map(String::as_str),
            Some("props-aliased")
        );
    }

    #[test]
    fn vue3_compile_script_inlines_empty_template_render_when_missing_or_src() {
        let mut compiler = SfcCompiler::new();
        let no_template = compiler.parse("FooBar.vue", "<script setup>const a = 1</script>");
        let script = compiler.compile_script(
            &no_template,
            SfcScriptCompileOptions {
                inline_template: true,
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script.content.contains("return () => {}"));
        assert!(!script.content.contains("const __returned__"));

        let src_template = compiler.parse(
            "FooBar.vue",
            r#"<template src="./Foo.html"></template><script setup>const a = 1</script>"#,
        );
        let script = compiler.compile_script(
            &src_template,
            SfcScriptCompileOptions {
                inline_template: true,
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(script.errors.is_empty(), "{:?}", script.errors);
        assert!(script.content.contains("return () => {}"));
        assert!(!script.content.contains("const __returned__"));
    }

    #[test]
    fn vue27_prefix_identifiers_rewrites_render_scope_references() {
        let compiler = SfcCompiler::new();
        let source = "function render(){with(this){return _c('div',{style:{color}},[_v(_s(foo)),_l(list,function(i){return _c('p',[_v(_s(i))])})])}}";

        assert_eq!(
            compiler.prefix_vue27_identifiers(
                source,
                Vue27PrefixIdentifiersOptions::default()
            ),
            "function render(){var _vm=this,_c=_vm._self._c;return _c('div',{style:{color: _vm.color}},[_vm._v(_vm._s(_vm.foo)),_vm._l(_vm.list,function(i){return _c('p',[_vm._v(_vm._s(i))])})])}"
        );
    }

    #[test]
    fn vue27_prefix_identifiers_uses_setup_proxy_for_setup_bindings() {
        let compiler = SfcCompiler::new();
        let source = "function render(){with(this){return _c('div',{on:{click:function($event){count++}}},[_v(_s(count))])}}";
        let options = Vue27PrefixIdentifiersOptions {
            bindings: BTreeMap::from([("count".into(), "setup-ref".into())]),
            ..Vue27PrefixIdentifiersOptions::default()
        };

        assert_eq!(
            compiler.prefix_vue27_identifiers(source, options),
            "function render(){var _vm=this,_c=_vm._self._c,_setup=_vm._self._setupProxy;return _c('div',{on:{click:function($event){_setup.count++}}},[_vm._v(_vm._s(_setup.count))])}"
        );
    }

    #[test]
    fn vue27_compile_script_injects_normal_script_css_vars() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            "<script>const a = 1</script><style>div{ color: v-bind(color); }</style>",
        );
        let script = compiler.compile_vue27_script(
            &descriptor,
            SfcScriptCompileOptions {
                id: Some("xxxxxxxx".into()),
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(script.content.contains("const __default__ = {}"));
        assert!(script
            .content
            .contains("import { useCssVars as _useCssVars } from 'vue'"));
        assert!(script.content.contains("\"xxxxxxxx-color\": (_vm.color)"));
        assert!(script.content.contains("export default __default__"));
    }

    #[test]
    fn vue27_compile_script_uses_legacy_css_var_names_and_comment_rules() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            "<script>const a = 1</script><style>// color: v-bind(color)\ndiv{ font-size: v-bind('font.size'); }</style>",
        );
        let script = compiler.compile_vue27_script(
            &descriptor,
            SfcScriptCompileOptions {
                id: Some("xxxxxxxx".into()),
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(script.content.contains("\"xxxxxxxx-color\": (_vm.color)"));
        assert!(script
            .content
            .contains("\"xxxxxxxx-font_size\": (_vm.font.size)"));
    }

    #[test]
    fn vue27_compile_script_injects_setup_css_vars_with_props() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<script setup>
import { defineProps, ref } from 'vue'
const color = 'red'
const size = ref('10px')
defineProps({ foo: String })
</script><style>div{ color: v-bind(color); width: v-bind(size); border: v-bind(foo); }</style>"#,
        );
        let script = compiler.compile_vue27_script(
            &descriptor,
            SfcScriptCompileOptions {
                id: Some("xxxxxxxx".into()),
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(script.content.contains("props: { foo: String },"));
        assert!(script
            .content
            .contains("\"xxxxxxxx-color\": (_setup.color)"));
        assert!(script.content.contains("\"xxxxxxxx-size\": (_setup.size)"));
        assert!(script.content.contains("\"xxxxxxxx-foo\": (_vm.foo)"));
        assert!(script
            .content
            .contains("return { __sfc: true,color, size, ref }"));
        assert!(!script.content.contains("defineProps"));
    }

    #[test]
    fn vue27_compile_script_can_omit_script_setup_marker_for_official_tests() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse("foo.vue", "<script setup>const color = 'red'</script>");
        let script = compiler.compile_vue27_script(
            &descriptor,
            SfcScriptCompileOptions {
                emit_script_setup_marker: false,
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(script.content.contains("return { color }"));
        assert!(!script.content.contains("__sfc: true"));
        assert_eq!(
            script.bindings.get("__isScriptSetup").map(String::as_str),
            Some("true")
        );
    }

    #[test]
    fn vue27_compile_script_preserves_official_empty_test_return_spacing() {
        let mut compiler = SfcCompiler::new();
        let descriptor =
            compiler.parse("foo.vue", "<script setup>defineExpose({ foo: 1 })</script>");
        let script = compiler.compile_vue27_script(
            &descriptor,
            SfcScriptCompileOptions {
                emit_script_setup_marker: false,
                ..SfcScriptCompileOptions::default()
            },
        );

        assert!(script.content.contains("return {  }"));
        assert!(!script.content.contains("return {}"));
    }

    #[test]
    fn vue27_compile_script_reports_options_and_inject_bindings() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<script>
export default {
  inject: ['foo', 'bar'],
  props: { baz: String },
  setup() { return { qux: null } },
  data() { return { quux: null } },
  methods: { quuz() {} },
  computed: { corge() {} }
}
</script>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());

        assert_eq!(
            script.bindings.get("foo").map(String::as_str),
            Some("options")
        );
        assert_eq!(
            script.bindings.get("bar").map(String::as_str),
            Some("options")
        );
        assert_eq!(
            script.bindings.get("baz").map(String::as_str),
            Some("props")
        );
        assert_eq!(
            script.bindings.get("qux").map(String::as_str),
            Some("setup-maybe-ref")
        );
        assert_eq!(
            script.bindings.get("quux").map(String::as_str),
            Some("data")
        );
        assert_eq!(
            script.bindings.get("quuz").map(String::as_str),
            Some("options")
        );
        assert_eq!(
            script.bindings.get("corge").map(String::as_str),
            Some("options")
        );
        assert_eq!(
            script.bindings.get("__isScriptSetup").map(String::as_str),
            Some("false")
        );
    }

    #[test]
    fn vue27_compile_script_merges_normal_script_bindings_into_setup_metadata() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<script>
import { xx } from './x'
export const aa = 1
let bb = 2
function cc() {}
class dd {}
</script>
<script setup>
import { ref as r } from 'vue'
import { x } from './x'
const a = r(1)
let b = 2
</script>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());

        assert_eq!(
            script.bindings.get("xx").map(String::as_str),
            Some("setup-maybe-ref")
        );
        assert_eq!(
            script.bindings.get("aa").map(String::as_str),
            Some("setup-const")
        );
        assert_eq!(
            script.bindings.get("bb").map(String::as_str),
            Some("setup-let")
        );
        assert_eq!(
            script.bindings.get("cc").map(String::as_str),
            Some("setup-const")
        );
        assert_eq!(
            script.bindings.get("dd").map(String::as_str),
            Some("setup-const")
        );
        assert_eq!(
            script.bindings.get("x").map(String::as_str),
            Some("setup-maybe-ref")
        );
        assert_eq!(
            script.bindings.get("r").map(String::as_str),
            Some("setup-const")
        );
        assert_eq!(
            script.bindings.get("a").map(String::as_str),
            Some("setup-ref")
        );
        assert_eq!(
            script.bindings.get("b").map(String::as_str),
            Some("setup-let")
        );
        assert_eq!(
            script.bindings.get("__isScriptSetup").map(String::as_str),
            Some("true")
        );
    }

    #[test]
    fn vue27_compile_script_orders_normal_and_setup_module_chunks_like_vue27() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<script>
export const n = 1
export default{
  some:'option'
}
</script>
<script setup>
import { x } from './x'
x()
</script>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(
            script.content.find("import { x } from './x'").unwrap()
                < script.content.find("export const n = 1").unwrap()
        );
        assert!(script
            .content
            .contains("export const n = 1\nconst __default__ = {\n  some:'option'"));

        let descriptor = compiler.parse(
            "foo.vue",
            r#"<script setup>
import { x } from './x'
x()
</script>
<script>
export const n = 1
const def = {}
export { def as default }
</script>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(
            script.content.find("export const n = 1").unwrap()
                < script.content.find("import { x } from './x'").unwrap()
        );
        assert!(script.content.contains("const __default__ = def"));
    }

    #[test]
    fn vue27_compile_script_hoists_side_effect_imports_and_dedupes_setup_imports() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<script>
import { x } from './x'
</script>
<script setup>
import { x } from './x'
import { ref } from 'vue'
import 'foo/css'
x()
</script>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());

        assert_eq!(script.content.matches("import { x } from './x'").count(), 1);
        assert!(script
            .content
            .contains("import { ref } from 'vue'\nimport 'foo/css'"));
        assert!(script.content.contains("return { __sfc: true,x, ref }"));
        assert!(script.errors.is_empty());
    }

    #[test]
    fn vue27_compile_script_reports_script_setup_macro_errors() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<script>foo()</script><script setup lang="ts">bar()</script>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());
        assert!(script.errors[0].contains("same language type"));

        let descriptor = compiler.parse("foo.vue", "<script setup>export const a = 1</script>");
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());
        assert!(script.errors[0].contains("cannot contain ES module exports"));

        let descriptor = compiler.parse(
            "foo.vue",
            r#"<script setup lang="ts">defineProps<{}>({})</script>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());
        assert!(script.errors[0].contains("cannot accept both type and non-type arguments"));

        let descriptor = compiler.parse(
            "foo.vue",
            r#"<script setup>
const bar = 1
defineProps({ foo: { default: () => bar } })
</script>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());
        assert!(script
            .errors
            .iter()
            .any(|error| error.contains("cannot reference locally declared variables")));

        let descriptor = compiler.parse(
            "foo.vue",
            r#"<script>const bar = 1</script>
<script setup>
defineProps({ foo: { default: () => bar } })
</script>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());
        assert!(script.errors.is_empty());
    }

    #[test]
    fn vue27_compile_script_returns_top_level_normal_and_setup_bindings() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<script setup>
import { x } from './x'
let a = 1
const b = 2
function c() {}
class d {}
</script>
<script>
import { xx } from './x'
let aa = 1
const bb = 2
function cc() {}
class dd {}
</script>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script
            .content
            .contains("return { __sfc: true,aa, bb, cc, dd, a, b, c, d, xx, x }"));
    }

    #[test]
    fn vue27_compile_script_filters_ts_template_import_usage() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<script setup lang="ts">
import { FooBar, FooBaz, FooQux, foo } from './x'
const fooBar: FooBar = 1
</script>
<template>
  <FooBaz></FooBaz>
  <foo-qux/>
  <foo/>
  FooBar
</template>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script
            .content
            .contains("return { __sfc: true,fooBar, FooBaz, FooQux, foo }"));
        assert!(!script.content.contains("return { fooBar, FooBar,"));
    }

    #[test]
    fn vue27_compile_script_filters_template_string_import_usage() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<script setup lang="ts">
import { VAR, VAR2, VAR3 } from './x'
</script>
<template>
  {{ `${VAR}VAR2${VAR3}` }}
</template>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.content.contains("return { __sfc: true,VAR, VAR3 }"));
    }

    #[test]
    fn vue27_compile_script_filters_import_type_return_bindings() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<script setup lang="ts">
import type { Foo } from './main.ts'
import { type Bar, Baz } from './main.ts'
</script>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.content.contains("return { __sfc: true,Baz }"));
    }

    #[test]
    fn vue27_compile_script_hoists_ts_types_and_runtime_enums() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<script setup lang="ts">
export interface Foo {}
type Bar = {}
enum Baz { A = 1 }
const enum Qux { A = 2 }
</script>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());

        let setup_index = script.content.find("setup(__props)").unwrap();
        assert!(script.content.find("export interface Foo {}").unwrap() < setup_index);
        assert!(script.content.find("type Bar = {}").unwrap() < setup_index);
        assert!(script.content.find("enum Baz { A = 1 }").unwrap() < setup_index);
        assert!(script.content.find("const enum Qux { A = 2 }").unwrap() < setup_index);
        assert!(script.content.contains("return { __sfc: true,Baz, Qux }"));
        assert_eq!(
            script.bindings.get("Baz").map(String::as_str),
            Some("setup-const")
        );
        assert_eq!(
            script.bindings.get("Qux").map(String::as_str),
            Some("setup-const")
        );
        assert!(!script.bindings.contains_key("Foo"));
        assert!(!script.bindings.contains_key("Bar"));
    }

    #[test]
    fn vue27_compile_script_returns_normal_script_runtime_enums() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<script lang="ts">
export enum D { D = "D" }
const enum C { C = "C" }
enum B { B = "B" }
</script>
<script setup lang="ts">
enum Foo { A = 123 }
</script>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script
            .content
            .contains("return { __sfc: true,D, C, B, Foo }"));
        for name in ["D", "C", "B", "Foo"] {
            assert_eq!(
                script.bindings.get(name).map(String::as_str),
                Some("setup-const")
            );
        }
    }

    #[test]
    fn vue27_compile_script_infers_setup_component_name_from_filename() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            "<script setup>const a = 1</script><template>{{ a }}</template>",
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script
            .content
            .contains("export default {\n  __name: 'FooBar',"));
        assert!(script.content.contains("return { __sfc: true,a }"));
    }

    #[test]
    fn vue27_compile_script_preserves_manual_default_export_name() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script>
export default {
  name: 'Baz'
}
</script>
<script setup>const a = 1</script>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script
            .content
            .contains("const __default__ = {\n  name: 'Baz'"));
        assert!(script
            .content
            .contains("export default /*#__PURE__*/Object.assign(__default__, {"));
        assert!(!script.content.contains("__name: 'FooBar'"));
    }

    #[test]
    fn vue27_compile_script_merges_ts_default_export_with_define_component() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "FooBar.vue",
            r#"<script lang="ts">
export default {
  name: 'Baz'
}
</script>
<script setup lang="ts">const a = 1</script>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script
            .content
            .contains("import { defineComponent as _defineComponent } from 'vue'"));
        assert!(script
            .content
            .contains("const __default__ = {\n  name: 'Baz'"));
        assert!(script
            .content
            .contains("export default /*#__PURE__*/_defineComponent({\n  ...__default__,"));
    }

    #[test]
    fn vue27_compile_script_generates_runtime_macro_options() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<script setup>
const props = defineProps({ foo: String })
const emit = defineEmits(['save'])
defineExpose({ reset() {} })
</script>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.content.contains("props: { foo: String },"));
        assert!(script.content.contains("emits: ['save'],"));
        assert!(script.content.contains("setup(__props, { emit, expose })"));
        assert!(script.content.contains("const props = __props;"));
        assert!(script.content.contains("expose({ reset() {} })"));
        assert!(script
            .content
            .contains("return { __sfc: true,props, emit }"));
        assert!(!script.content.contains("defineProps"));
        assert!(!script.content.contains("defineEmits"));
        assert!(!script.content.contains("defineExpose"));
    }

    #[test]
    fn vue27_compile_script_unbound_define_emits_only_generates_runtime_option() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<script setup>
defineEmits(['save'])
</script>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.content.contains("emits: ['save'],"));
        assert!(script.content.contains("setup(__props)"));
        assert!(!script.content.contains("{ emit }"));
        assert!(!script.content.contains("defineEmits"));
    }

    #[test]
    fn vue27_compile_script_preserves_define_props_binding_pattern_alias() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<script setup>
const { foo, bar: baz } = defineProps({ foo: String, bar: Number })
</script>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script
            .content
            .contains("const { foo, bar: baz } = __props;"));
        assert!(script.content.contains("return { __sfc: true,foo, baz }"));
        assert!(!script.content.contains("defineProps"));
    }

    #[test]
    fn vue27_compile_script_removes_runtime_macros_from_multi_declaration() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<script setup>
const props = defineProps(['item']),
  a = 1,
  emit = defineEmits(['save'])
</script>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.content.contains("props: ['item'],"));
        assert!(script.content.contains("emits: ['save'],"));
        assert!(script.content.contains("const a = 1"));
        assert!(script.content.contains("const props = __props;"));
        assert!(script
            .content
            .contains("return { __sfc: true,props, a, emit }"));
        assert!(!script.content.contains("defineProps"));
        assert!(!script.content.contains("defineEmits"));
    }

    #[test]
    fn vue27_compile_script_infers_ts_define_props_from_normal_script() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<script lang="ts">
export interface Props { x?: number }
</script>
<script setup lang="ts">
defineProps<Props>()
</script>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script
            .content
            .contains("x: { type: Number, required: false }"));
        assert_eq!(script.bindings.get("x").map(String::as_str), Some("props"));
        assert!(script.errors.is_empty());
    }

    #[test]
    fn vue27_compile_script_infers_with_defaults_runtime_props() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<script setup lang="ts">
const props = withDefaults(defineProps<{
  foo?: string
  bar?: number;
  baz: boolean;
  qux?(): number
}>(), {
  foo: 'hi',
  qux() { return 1 }
})
</script>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script
            .content
            .contains("foo: { type: String, required: false, default: 'hi' }"));
        assert!(script
            .content
            .contains("qux: { type: Function, required: false, default() { return 1 } }"));
        assert!(script.content.contains(
            "const props = __props as { foo: string, bar?: number, baz: boolean, qux(): number };"
        ));
        assert!(script.errors.is_empty());
    }

    #[test]
    fn vue27_compile_script_infers_define_emits_type_and_rejects_union() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<script setup lang="ts">
const emit = defineEmits<{(e: 'foo' | 'bar'): void; (e: 'baz', id: number): void;}>()
</script>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());

        assert!(script.content.contains(r#"emits: ["foo", "bar", "baz"],"#));
        assert!(script
            .content
            .contains("emit: ({(e: 'foo' | 'bar'): void; (e: 'baz', id: number): void;})"));
        assert!(script.errors.is_empty());

        let descriptor = compiler.parse(
            "foo.vue",
            r#"<script setup lang="ts">
const emit = defineEmits<((e: 'foo') => void) | ((e: 'bar') => void)>()
</script>"#,
        );
        let script = compiler.compile_vue27_script(&descriptor, SfcScriptCompileOptions::default());

        assert_eq!(script.errors.len(), 1);
        assert!(script.errors[0].contains("type argument passed to defineEmits()"));
    }

    #[test]
    fn compile_wrappers_return_shapes() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<template><div/></template><script>export default {}</script><script setup lang="ts">const x = 1</script><style scoped src="./base.css">@import "./dep.css"; .a{ color: v-bind(color); }</style>"#,
        );
        let template = compiler.compile_template(&descriptor, SfcTemplateCompileOptions::default());
        assert!(template.code.contains("render"));
        assert!(template.ast_summary.starts_with("dom:"));
        let script = compiler.compile_script(&descriptor, SfcScriptCompileOptions::default());
        assert_eq!(script.errors.len(), 0);
        assert!(script.setup);
        assert_eq!(script.lang.as_deref(), Some("ts"));
        assert_eq!(
            script.bindings.get("x").map(String::as_str),
            Some("literal-const")
        );
        assert!(script.content.contains("_defineComponent"));
        assert!(script.content.contains("__returned__ = { x }"));
        assert_eq!(script.script_ast, vec!["JsProgramId(0)"]);
        assert_eq!(script.script_setup_ast, vec!["JsProgramId(1)"]);
        let script_json = serde_json::to_value(&script).expect("script json");
        assert!(script_json.get("scriptAst").is_some());
        assert!(script_json.get("scriptSetupAst").is_some());
        assert_eq!(
            script_json.get("type").and_then(|value| value.as_str()),
            Some("script")
        );
        let style = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        assert!(style.errors.is_empty());
        assert!(style.map.is_none());
        assert!(style.code.contains("var(--color)"));
        assert_eq!(style.dependencies, vec!["./base.css", "./dep.css"]);
        assert_eq!(style.raw_result.len(), 1);
        let style_json = serde_json::to_value(&style).expect("style json");
        assert!(style_json.get("rawResult").is_some());
    }

    #[test]
    fn compile_style_returns_css_module_exports() {
        let mut compiler = SfcCompiler::new();
        let source = r#"<style module>.red { color: red }\n:global(.blue) { color: blue }</style>"#;
        let descriptor = compiler.parse("modules.vue", source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        let modules = result.modules.expect("css modules");

        assert!(modules
            .get("red")
            .is_some_and(|value| value.contains("_red_")));
        assert!(!modules.contains_key("blue"));
        assert!(result.code.contains(".blue { color: blue }"));
    }

    #[test]
    fn compile_style_returns_css_modules_values() {
        let mut compiler = SfcCompiler::new();
        let source = r#"<style module>@value primary: red; @value query: (min-width: 1px); @media query { .button { color: primary; } }</style>"#;
        let descriptor = compiler.parse("modules.vue", source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        let modules = result.modules.expect("css modules");

        assert_eq!(modules.get("primary").map(String::as_str), Some("red"));
        assert_eq!(
            modules.get("query").map(String::as_str),
            Some("(min-width: 1px)")
        );
        assert!(modules
            .get("button")
            .is_some_and(|value| value.contains("_button_")));
        assert!(!result.code.contains("@value"));
        assert!(result.code.contains("@media (min-width: 1px)"));
        assert!(result.code.contains("color: red"));
    }

    #[test]
    fn compile_style_returns_css_modules_imported_values() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("tokens.css"),
            "@value primary: red; .remote { color: primary; }",
        )
        .expect("write dep");
        let filename = dir.path().join("modules.vue");
        let mut compiler = SfcCompiler::new();
        let source = r#"<style module>@value primary, remote as external from "./tokens.css"; .button { composes: external; color: primary; } .external { border-color: primary; }</style>"#;
        let descriptor = compiler.parse(filename.to_string_lossy().to_string(), source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        let modules = result.modules.expect("css modules");
        let external = modules.get("external").expect("external export");

        assert_eq!(modules.get("primary").map(String::as_str), Some("red"));
        assert!(external.contains("_remote_"));
        assert!(modules
            .get("button")
            .is_some_and(|value| value.contains("_button_") && value.contains(external)));
        assert!(!result.code.contains("@value"));
        assert!(!result.code.contains("_external_"));
        assert!(!result.code.contains("; }"));
        assert!(result.code.contains("color: red"));
        assert!(result.code.contains("border-color: red"));
    }

    #[test]
    fn compile_style_returns_css_modules_missing_imported_value_composes() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("tokens.css"),
            "@value primary: red; .remote { color: primary; }",
        )
        .expect("write dep");
        let filename = dir.path().join("modules.vue");
        let mut compiler = SfcCompiler::new();
        let source = r#"<style module>@value missing from "./tokens.css"; .button { composes: missing; color: missing; }</style>"#;
        let descriptor = compiler.parse(filename.to_string_lossy().to_string(), source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        let modules = result.modules.expect("css modules");
        let button = modules.get("button").expect("button export");

        assert!(result.errors.is_empty());
        assert_eq!(
            modules.get("missing").map(String::as_str),
            Some("undefined")
        );
        assert!(button.contains("_button_"));
        assert!(button.contains("undefined"));
        assert!(!button.contains("i__const_missing_0"));
        assert!(!result.code.contains("@value"));
        assert!(result.code.contains("color: i__const_missing_0"));
    }

    #[test]
    fn compile_style_forwards_css_modules_dashes_convention() {
        let mut compiler = SfcCompiler::new();
        let source = r#"<style module>.foo-bar { color: red }\n.foo_bar { color: blue }</style>"#;
        let descriptor = compiler.parse("modules.vue", source);
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                modules_options: CssModulesOptions {
                    locals_convention: "dashesOnly".into(),
                    ..CssModulesOptions::default()
                },
                ..SfcStyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules");

        assert!(modules
            .get("fooBar")
            .is_some_and(|value| value.contains("_foo-bar_")));
        assert!(!modules.contains_key("foo-bar"));
        assert!(modules
            .get("foo_bar")
            .is_some_and(|value| value.contains("_foo_bar_")));
    }

    #[test]
    fn compile_style_forwards_css_modules_hash_prefix() {
        let mut compiler = SfcCompiler::new();
        let source = r#"<style module>.button { color: red }</style>"#;
        let descriptor = compiler.parse("src/Comp.vue", source);
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                modules_options: CssModulesOptions {
                    generate_scoped_name: Some("[local]__[hash:base64:5]".into()),
                    hash_prefix: "alpha".into(),
                    ..CssModulesOptions::default()
                },
                ..SfcStyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules");

        assert_eq!(
            modules.get("button").map(String::as_str),
            Some("button__2G66Z")
        );
        assert!(result.code.contains(".button__2G66Z"));
    }

    #[test]
    fn compile_style_forwards_css_modules_global_module_paths() {
        let mut compiler = SfcCompiler::new();
        let source =
            r#"<style module>.button { color: red }:local(.forced) { color: blue }</style>"#;
        let descriptor = compiler.parse("src/theme.global.css", source);
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                modules_options: CssModulesOptions {
                    global_module_paths: vec![r"global\.css$".into()],
                    ..CssModulesOptions::default()
                },
                ..SfcStyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules");

        assert!(!modules.contains_key("button"));
        assert!(modules
            .get("forced")
            .is_some_and(|value| value.contains("_forced_")));
        assert!(result.code.contains(".button { color: red }"));
        assert!(result.code.contains("._forced_"));
    }

    #[test]
    fn compile_style_returns_css_modules_id_exports() {
        let source = r#"<style module>#panel { color: red }.button#item { color: blue }</style>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse("src/Selectors.vue", source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        let modules = result.modules.expect("css modules");

        assert_eq!(
            modules.get("panel").map(String::as_str),
            Some("_panel_7jaos_1")
        );
        assert_eq!(
            modules.get("button").map(String::as_str),
            Some("_button_7jaos_1")
        );
        assert_eq!(
            modules.get("item").map(String::as_str),
            Some("_item_7jaos_1")
        );
        assert!(result.code.contains("#_panel_7jaos_1"));
        assert!(result.code.contains("._button_7jaos_1#_item_7jaos_1"));
    }

    #[test]
    fn compile_style_leaves_css_modules_class_attribute_selectors_global() {
        let source = r#"<style module>[class="btn"] { color: red }:local([class='forced']) { color: blue }.btn { color: black }</style>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse("src/Attr.vue", source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        let modules = result.modules.expect("css modules");

        assert!(modules
            .get("btn")
            .is_some_and(|value| value.contains("_btn_")));
        assert!(!modules.contains_key("forced"));
        assert!(result.code.contains("[class=\"btn\"] { color: red }"));
        assert!(result.code.contains("[class='forced'] { color: blue }"));
        assert!(result.code.contains("._btn_"));
    }

    #[test]
    fn compile_style_returns_css_modules_keyframe_exports() {
        let source = r#"<style module>@keyframes fade { from { opacity: 0 } to { opacity: 1 } }
.button { animation-name: fade; }</style>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse("src/Anim.vue", source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        let modules = result.modules.expect("css modules");

        assert_eq!(
            modules.get("fade").map(String::as_str),
            Some("_fade_17sru_1")
        );
        assert_eq!(
            modules.get("button").map(String::as_str),
            Some("_button_17sru_2")
        );
        assert!(result.code.contains("@keyframes _fade_17sru_1"));
        assert!(result.code.contains("animation-name: _fade_17sru_1"));
    }

    #[test]
    fn compile_style_forwards_css_modules_export_globals() {
        let mut compiler = SfcCompiler::new();
        let source = r#"<style module>.local :global(.global) { color: red }</style>"#;
        let descriptor = compiler.parse("modules.vue", source);
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                modules_options: CssModulesOptions {
                    export_globals: true,
                    ..CssModulesOptions::default()
                },
                ..SfcStyleCompileOptions::default()
            },
        );
        let modules = result.modules.expect("css modules");

        assert!(modules
            .get("local")
            .is_some_and(|value| value.contains("_local_")));
        assert_eq!(modules.get("global").map(String::as_str), Some("global"));
    }

    #[test]
    fn compile_style_returns_css_modules_composes_exports() {
        let mut compiler = SfcCompiler::new();
        let source = r#"<style module>.base { color: blue }.button { composes: base global(extra); color: red }</style>"#;
        let descriptor = compiler.parse("modules.vue", source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        let modules = result.modules.expect("css modules");
        let base = modules.get("base").expect("base export");
        let button = modules.get("button").expect("button export");

        assert!(button.contains(base));
        assert!(button.contains("extra"));
        assert!(!result.code.contains("composes"));
    }

    #[test]
    fn compile_style_returns_css_modules_icss_exports() {
        let mut compiler = SfcCompiler::new();
        let source =
            r#"<style module>:export { primary: red; }.button { color: primary; }</style>"#;
        let descriptor = compiler.parse("modules.vue", source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        let modules = result.modules.expect("css modules");

        assert_eq!(modules.get("primary").map(String::as_str), Some("red"));
        assert!(modules
            .get("button")
            .is_some_and(|value| value.contains("_button_")));
        assert!(!result.code.contains(":export"));
    }

    #[test]
    fn compile_style_rewrites_css_modules_icss_import_symbols() {
        let dir = tempfile::tempdir().expect("temp dir");
        let filename = dir.path().join("component.vue");
        std::fs::write(
            dir.path().join("dep.css"),
            ".dep { color: blue; }\n:export { token: green; query: (min-width: 1px); }",
        )
        .expect("write dep");
        let source = r#"<style module>:import("./dep.css") { imported: dep; shade: token; mq: query; }.shade { color: shade; }.imported { color: shade; }@media mq { .button { color: shade; } }</style>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        let modules = result.modules.expect("css modules");

        assert!(!modules.contains_key("shade"));
        assert!(!modules.contains_key("imported"));
        assert!(modules
            .get("button")
            .is_some_and(|value| value.contains("_button_")));
        assert!(!result.code.contains(":import"));
        assert!(!result.code.contains("_shade_"));
        assert!(!result.code.contains("_imported_"));
        assert!(result.code.contains(".green"));
        assert!(result.code.contains("@media (min-width: 1px)"));
        assert!(result.code.contains("color: green"));
    }

    #[test]
    fn compile_style_preserves_empty_css_modules_for_missing_icss_imports() {
        let dir = tempfile::tempdir().expect("temp dir");
        let filename = dir.path().join("component.vue");
        std::fs::write(dir.path().join("dep.css"), ":export { token: green; }").expect("write dep");
        let source = r#"<style module>:import("./dep.css") { shade: missing; }.shade { color: red; }</style>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        let modules = result.modules.expect("empty css modules map");

        assert!(modules.is_empty());
        assert!(result.errors.is_empty());
        assert!(!result.code.contains(":import"));
        assert!(result.code.contains(".shade { color: red"));
        assert!(!result.code.contains("_shade_"));
    }

    #[test]
    fn compile_style_rewrites_css_modules_native_nested_rules() {
        let dir = tempfile::tempdir().expect("temp dir");
        let filename = dir.path().join("component.vue");
        let source = r#"<style module>.foo { color: blue; .bar { color: red; } &.active { color: green; } @media (min-width: 1px) { :global(.global) { color: black; } :local(.inner) { color: white; } } color: yellow; }</style>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        let modules = result.modules.expect("css modules");

        for key in ["foo", "bar", "active", "inner"] {
            assert!(
                modules.get(key).is_some_and(|value| value.contains('_')),
                "missing module key {key}: {modules:?}"
            );
        }
        assert!(!modules.contains_key("global"));
        assert!(result.code.contains("{ color: blue;\n"));
        assert!(result.code.contains("\n._bar_"));
        assert!(result.code.contains("\n&._active_"));
        assert!(result.code.contains("@media (min-width: 1px) {\n.global"));
        assert!(result.code.contains("\n._inner_"));
        assert!(result.code.contains("} color: yellow;"));
        assert!(!result.code.contains("\n.bar {"));
        assert!(!result.code.contains("\n&.active {"));
        assert!(!result.code.contains(":local(.inner)"));
        assert!(!result.code.contains(":global(.global)"));
    }

    #[test]
    fn compile_style_reports_css_modules_native_nested_composes() {
        let source = r#"<style module>.foo { .bar { composes: foo; color: red; } }</style>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse("modules.vue", source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());

        assert_eq!(
            result.errors,
            vec![
                "composition is not allowed in nested rule \n\n:local(.bar) { composes: foo; color: red;\n}"
            ]
        );
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].code, "VUEC_STYLE_MODULE_COMPOSE");
        assert!(result.code.is_empty());
        assert!(result.modules.is_none());
    }

    #[test]
    fn compile_style_returns_css_modules_external_composes() {
        let dir = tempfile::tempdir().expect("temp dir");
        let filename = dir.path().join("component.vue");
        let dep = dir.path().join("dep.css");
        std::fs::write(&dep, ".dep { color: blue; }\n:export { token: green; }")
            .expect("write dep");
        let source =
            r#"<style module>.button { composes: dep from "./dep.css"; color: token; }</style>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        let modules = result.modules.expect("css modules");
        let button = modules.get("button").expect("button export");

        assert!(button.contains("_button_"));
        assert!(button.contains("_dep_"));
        assert!(result.code.starts_with("._dep_"));
        assert!(!result.code.contains("composes"));
    }

    #[test]
    fn compile_style_returns_css_modules_node_modules_composes() {
        let dir = tempfile::tempdir().expect("temp dir");
        let src_dir = dir.path().join("src");
        let package_dir = dir.path().join("node_modules").join("vuec-css-fixture");
        let dist_dir = package_dir.join("dist");
        std::fs::create_dir_all(&src_dir).expect("src dir");
        std::fs::create_dir_all(&dist_dir).expect("dist dir");
        let filename = src_dir.join("component.vue");
        std::fs::write(dist_dir.join("theme.css"), ".dep { color: blue; }").expect("write dep");
        std::fs::write(
            package_dir.join("package.json"),
            r#"{"name":"vuec-css-fixture","exports":{"./theme.css":"./dist/theme.css"}}"#,
        )
        .expect("write package");
        let source = r#"<style module>.button { composes: dep from "vuec-css-fixture/theme.css"; color: red; }</style>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        let modules = result.modules.expect("css modules");
        let button = modules.get("button").expect("button export");

        assert!(button.contains("_button_"));
        assert!(button.contains("_dep_"));
        assert!(result.code.starts_with("._dep_"));
        assert!(!result.code.contains("composes"));
    }

    #[test]
    fn compile_style_returns_css_modules_composes_from_global() {
        let source =
            r#"<style module>.button { composes: reset utility from global; color: red; }</style>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse("modules.vue", source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        let modules = result.modules.expect("css modules");
        let button = modules.get("button").expect("button export");

        assert!(button.contains("_button_"));
        assert!(button.contains("reset"));
        assert!(button.contains("utility"));
        assert!(!result.code.contains("composes"));
    }

    #[test]
    fn compile_style_maps_css_modules_composes_diagnostics_to_vue_source() {
        let source = r#"<template></template>
<style module>.button { composes: missing; color: red; }</style>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse("modules.vue", source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());
        let missing_start = source.find("missing").expect("missing token");

        assert_eq!(
            result.errors,
            vec!["referenced class name \"missing\" in composes not found"]
        );
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].code, "VUEC_STYLE_MODULE_COMPOSE");
        assert_eq!(
            result.diagnostics[0].span,
            Some(vuec_source::Span::new(
                descriptor.source_file,
                missing_start,
                missing_start + "missing".len()
            ))
        );
    }

    #[test]
    fn compile_style_reports_css_modules_complex_composes_selector() {
        let source =
            r#"<style module>.button.extra { composes: base; }.base { color: red; }</style>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse("modules.vue", source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());

        assert_eq!(
            result.errors,
            vec![
                "composition is only allowed when selector is single :local class name not in \":local(.button):local(.extra)\""
            ]
        );
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].code, "VUEC_STYLE_MODULE_COMPOSE");
        assert!(!result.code.contains("composes"));
    }

    #[test]
    fn compile_style_forwards_scss_preprocess_options_and_dependencies() {
        let dir = tempfile::tempdir().expect("temp dir");
        let filename = dir.path().join("component.vue");
        let import = dir.path().join("import.scss");
        std::fs::write(&import, ".imported { color: $red; }\n").expect("write import");
        let source = r#"<style lang="scss">
@import "./import.scss";
.square { @include square(10px); }
</style>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                preprocess_options: StylePreprocessOptions {
                    additional_data: Some(
                        "$red: red;\n@mixin square($size) { width: $size; height: $size; }".into(),
                    ),
                    ..StylePreprocessOptions::default()
                },
                ..SfcStyleCompileOptions::default()
            },
        );

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(result.code.contains(".imported"));
        assert!(result.code.contains("width: 10px;"));
        let resolved_import = std::fs::canonicalize(import)
            .expect("canonical import")
            .to_string_lossy()
            .replace('\\', "/")
            .trim_start_matches("//?/")
            .to_string();
        assert_eq!(result.dependencies, vec![resolved_import]);
    }

    #[test]
    fn compile_style_forwards_less_preprocess_options_and_dependencies() {
        let dir = tempfile::tempdir().expect("temp dir");
        let filename = dir.path().join("component.vue");
        let import = dir.path().join("tokens.less");
        std::fs::write(&import, "@space: 6px;\n.imported { margin: @space; }\n")
            .expect("write import");
        let source = r#"<style lang="less">
@import "./tokens.less";
.card {
  color: @brand;
  .title {
    padding: @space;
  }
}
</style>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                preprocess_options: StylePreprocessOptions {
                    additional_data: Some("@brand: red;".into()),
                    ..StylePreprocessOptions::default()
                },
                ..SfcStyleCompileOptions::default()
            },
        );

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(result.code.contains(".imported"));
        assert!(result.code.contains("margin: 6px;"));
        assert!(result.code.contains(".card .title"));
        assert!(result.code.contains("padding: 6px;"));
        assert!(result.code.contains("color: red;"));
        let resolved_import = std::fs::canonicalize(import)
            .expect("canonical import")
            .to_string_lossy()
            .replace('\\', "/")
            .trim_start_matches("//?/")
            .to_string();
        assert_eq!(result.dependencies, vec![resolved_import]);
    }

    #[test]
    fn compile_style_forwards_stylus_preprocess_options_and_dependencies() {
        let dir = tempfile::tempdir().expect("temp dir");
        let filename = dir.path().join("component.vue");
        let import = dir.path().join("tokens.styl");
        std::fs::write(&import, "space = 6px\n.imported\n  margin space\n").expect("write import");
        let source = r#"<style lang="stylus">
@import "./tokens"
.card
  color brand
  .title
    padding space
</style>"#;
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(filename.to_string_lossy(), source);
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                preprocess_options: StylePreprocessOptions {
                    additional_data: Some("brand = red".into()),
                    ..StylePreprocessOptions::default()
                },
                ..SfcStyleCompileOptions::default()
            },
        );

        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(result.code.contains(".imported"));
        assert!(result.code.contains("margin: 6px;"));
        assert!(result.code.contains(".card .title"));
        assert!(result.code.contains("padding: 6px;"));
        assert!(result.code.contains("color: red;"));
        let resolved_import = std::fs::canonicalize(import)
            .expect("canonical import")
            .to_string_lossy()
            .replace('\\', "/")
            .trim_start_matches("//?/")
            .to_string();
        assert_eq!(result.dependencies, vec![resolved_import]);
    }

    #[test]
    fn compile_style_uses_vue3_css_var_names_by_default() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "style.vue",
            "<style>.foo { font-size: v-bind('font.size'); font-weight: v-bind(_φ); }</style>",
        );
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                id: Some("data-v-test".into()),
                ..SfcStyleCompileOptions::default()
            },
        );

        assert!(result.code.contains(r"var(--test-font\.size)"));
        assert!(result.code.contains("var(--test-_φ)"));
    }

    #[test]
    fn compile_style_rewrites_comment_separated_css_vars() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "style.vue",
            r#"<style>.foo { color: v-bind /*x*/ (color); font-size: v-bind/**/ ('font.size'); height: v-bind/**/(height); }</style>"#,
        );
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                id: Some("data-v-test".into()),
                ..SfcStyleCompileOptions::default()
            },
        );

        assert!(result.code.contains("var(--test-color)"));
        assert!(result.code.contains(r"var(--test-font\.size)"));
        assert!(result.code.contains("v-bind/**/(height)"));
    }

    #[test]
    fn compile_style_rewrites_top_level_is_where_scoped_branches() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "style.vue",
            r#"<style scoped>:is(.foo, .bar):hover { color: red; }:where(.one .child, .two > .item) { color: blue; }.host:is(.foo, .bar) { color: green; }</style>"#,
        );
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                id: Some("data-v-test".into()),
                scoped: true,
                ..SfcStyleCompileOptions::default()
            },
        );

        assert!(result
            .code
            .contains(":is(.foo[data-v-test], .bar[data-v-test]):hover"));
        assert!(result
            .code
            .contains(":where(.one .child[data-v-test], .two > .item[data-v-test])"));
        assert!(result.code.contains(".host[data-v-test]:is(.foo, .bar)"));
    }

    #[test]
    fn compile_style_rewrites_native_nested_scoped_rules() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "style.vue",
            r#"<style scoped>.foo { color: blue; .bar { color: red; } @media (min-width: 1px) { &:hover { color: green; } } }</style>"#,
        );
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                id: Some("data-v-test".into()),
                scoped: true,
                ..SfcStyleCompileOptions::default()
            },
        );

        assert!(result.code.contains(".foo {"));
        assert!(result.code.contains("&[data-v-test] { color: blue;"));
        assert!(result.code.contains(".bar[data-v-test] { color: red;"));
        assert!(result.code.contains("@media (min-width: 1px) {"));
        assert!(result.code.contains("&[data-v-test]:hover { color: green;"));
    }

    #[test]
    fn compile_style_rewrites_direct_nested_parent_selectors() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "style.vue",
            r#"<style scoped>*.foo { color: blue; .bar { color: red; } }.foo /*x*/ .bar { .child { color: orange; } }:is(.foo /*x*/ .bar, *.baz) { .child { color: purple; } }:is(:global(.g), :slotted(.s), * .item):hover { color: green; .child { color: yellow; } }</style>"#,
        );
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                id: Some("data-v-test".into()),
                scoped: true,
                ..SfcStyleCompileOptions::default()
            },
        );

        assert!(result.code.contains(".foo {"));
        assert!(!result.code.contains("*.foo {"));
        assert!(result.code.contains(":is(.g,.s,.item):hover {"));
        assert!(result.code.contains(".foo /*x*/ .bar {"));
        assert!(result.code.contains(":is(.foo  .bar,.baz) {"));
        assert!(result.code.contains(".bar[data-v-test] { color: red;"));
        assert!(result.code.contains(".child[data-v-test] { color: yellow;"));
    }

    #[test]
    fn compile_style_rewrites_first_normal_deep_container_nested_rules() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "style.vue",
            r#"<style scoped>:is(.foo, :deep(.bar), .baz) { color: blue; .child { color: red; } }.host :where(:global(.g), :slotted(.s), :deep(.d), .tail) { color: green; & .child { color: yellow; } }</style>"#,
        );
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                id: Some("data-v-test".into()),
                scoped: true,
                ..SfcStyleCompileOptions::default()
            },
        );

        assert!(result
            .code
            .contains(":is(.foo,[data-v-test] .bar, .baz[data-v-test])[data-v-test] {"));
        assert!(result.code.contains("& { color: blue;"));
        assert!(result
            .code
            .contains(".host[data-v-test] :where(.g,.s,[data-v-test] .d, .tail[data-v-test]) {"));
        assert!(result.code.contains("& .child { color: yellow;"));
    }

    #[test]
    fn compile_style_rewrites_first_normal_deep_container_suffix_nested_rules() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "style.vue",
            r#"<style scoped>:is(.foo, :deep(.bar), .baz):hover { color: blue; .child { color: red; } }.host :where(.foo, :deep(.bar), :global(.g), :slotted(.s)):hover { color: green; .child { color: yellow; } }</style>"#,
        );
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                id: Some("data-v-test".into()),
                scoped: true,
                ..SfcStyleCompileOptions::default()
            },
        );

        assert!(result.code.contains(
            ":is(.foo):hover, :is([data-v-test] .bar)[data-v-test]:hover, :is(.baz[data-v-test]):hover {"
        ));
        assert!(result.code.contains("& { color: blue;"));
        assert!(result.code.contains(
            ".host[data-v-test] :where(.foo,[data-v-test] .bar,.g,[data-v-test].s[data-v-test-s]):hover {"
        ));
        assert!(result.code.contains(".child { color: yellow;"));
    }

    #[test]
    fn compile_style_rewrites_deep_nested_scoped_rules() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "style.vue",
            r#"<style scoped>:deep(.foo, .bar) { color: blue; .child { color: red; } @media (min-width: 1px) { .inner { color: green; } } }:deep(.anchor) { color: blue; & .child { color: red; } }</style>"#,
        );
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                id: Some("data-v-test".into()),
                scoped: true,
                ..SfcStyleCompileOptions::default()
            },
        );

        assert!(result.code.contains("[data-v-test] .foo {"));
        assert!(result.code.contains("color: blue;"));
        assert!(result.code.contains(".child { color: red;"));
        assert!(result.code.contains("@media (min-width: 1px) {"));
        assert!(result.code.contains(".inner { color: green;"));
        assert!(result
            .code
            .contains("[data-v-test] .anchor { color: blue;\n& .child { color: red;"));
        assert!(!result.code.contains(".bar"));
        assert!(!result.code.contains(".child[data-v-test]"));
        assert!(!result.code.contains(".inner[data-v-test]"));
    }

    #[test]
    fn compile_style_rewrites_slotted_universal_combinators() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "style.vue",
            r#"<style scoped>:slotted(* + .foo) { color: red; }:is(:slotted(* + .bar), .baz) { color: blue; }:slotted(:is(.alpha, .beta)) { color: green; }</style>"#,
        );
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                id: Some("data-v-test".into()),
                scoped: true,
                ..SfcStyleCompileOptions::default()
            },
        );

        assert!(result.code.contains("+ .foo[data-v-test-s]"));
        assert!(result
            .code
            .contains(":is(+ .bar[data-v-test-s], .baz[data-v-test])"));
        assert!(result
            .code
            .contains(":is(.alpha[data-v-test-s], .beta[data-v-test-s])"));
    }

    #[test]
    fn compile_style_preserves_scoped_selector_list_spacing() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "style.vue",
            r#"<style scoped>.a,.b { color: red; }.a, :slotted(.b) { color: blue; }.a, :where(.b).active { color: green; }</style>"#,
        );
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                id: Some("data-v-test".into()),
                scoped: true,
                ..SfcStyleCompileOptions::default()
            },
        );

        assert!(result.code.contains(".a[data-v-test],.b[data-v-test]"));
        assert!(result.code.contains(".a[data-v-test],.b[data-v-test-s]"));
        assert!(result
            .code
            .contains(".a[data-v-test], :where(.b).active[data-v-test]"));
    }

    #[test]
    fn compile_style_rewrites_escaped_scoped_selector_tokens() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "style.vue",
            r#"<style scoped>.foo\:bar { color: red; }.foo\,bar { color: blue; }:slotted(.foo\:bar) { color: green; }.foo\:deep(.bar) { color: yellow; }</style>"#,
        );
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                id: Some("data-v-test".into()),
                scoped: true,
                ..SfcStyleCompileOptions::default()
            },
        );

        assert!(result.code.contains(r#".foo\:bar[data-v-test]"#));
        assert!(result.code.contains(r#".foo\,bar[data-v-test]"#));
        assert!(result.code.contains(r#".foo\:bar[data-v-test-s]"#));
        assert!(result.code.contains(r#".foo\:deep(.bar)[data-v-test]"#));
    }

    #[test]
    fn compile_style_rewrites_commented_scoped_selectors() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "style.vue",
            r#"<style scoped>.foo/*,*/.bar { color: red; }.foo /*x*/:hover { color: blue; }:is(.foo/*:deep(.bar)*/.baz, .qux) { color: green; }</style>"#,
        );
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                id: Some("data-v-test".into()),
                scoped: true,
                ..SfcStyleCompileOptions::default()
            },
        );

        assert!(result.code.contains(".foo/*,*/.bar[data-v-test]"));
        assert!(result.code.contains(".foo[data-v-test] :hover"));
        assert!(result
            .code
            .contains(":is(.foo/*:deep(.bar)*/.baz[data-v-test], .qux[data-v-test])"));
    }

    #[test]
    fn compile_style_rewrites_deep_container_special_branches() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "style.vue",
            r#"<style scoped>:is(:slotted(.foo), :deep(.bar), :global(.baz), .qux) { color: red; }.host:is(:deep(.foo), :global(.bar), :slotted(.baz), .qux) { @media (min-width:1px){ .child { color:red; } } }:is(:deep(.foo), :global(.bar), :slotted(.baz), .qux) { color: blue; .child { color:red; } }</style>"#,
        );
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                id: Some("data-v-test".into()),
                scoped: true,
                ..SfcStyleCompileOptions::default()
            },
        );

        assert!(result
            .code
            .contains(":is(.foo[data-v-test-s],[data-v-test] .bar,.baz, .qux[data-v-test])"));
        assert!(result
            .code
            .contains(".host[data-v-test]:is( .foo,.bar,.baz, .qux)"));
        assert!(result.code.contains(
            ":is([data-v-test] .foo,.bar,[data-v-test].baz[data-v-test-s], .qux[data-v-test])[data-v-test]"
        ));
    }

    #[test]
    fn compile_style_rewrites_deep_container_split_pseudo_suffix() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "style.vue",
            r#"<style scoped>:is(:deep(.d), .n):hover { color:red; }:where(.x :deep(.d), :slotted(.s))::before { color:red; }:has(.n,:deep(.d),.m):hover { color:red; }:where(:deep(.d), :slotted(.s))::before { color: blue; .child { color: red; } }</style>"#,
        );
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                id: Some("data-v-test".into()),
                scoped: true,
                ..SfcStyleCompileOptions::default()
            },
        );

        assert!(result
            .code
            .contains(":is([data-v-test] .d):hover, :is(.n[data-v-test]):hover"));
        assert!(result
            .code
            .contains(":where(.x[data-v-test] .d)::before, :where(.s[data-v-test-s])::before"));
        assert!(result.code.contains(
            "[data-v-test]:has(.n):hover, :has([data-v-test] .d):hover,[data-v-test]:has(.m):hover"
        ));
        assert!(result.code.contains(
            ":where([data-v-test] .d)[data-v-test]::before, :where([data-v-test].s[data-v-test-s])::before"
        ));
    }

    #[test]
    fn compile_style_rewrites_deep_passthrough_nested_at_rule_special_selectors() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "style.vue",
            r#"<style scoped>:deep(.d) { @media (min-width:1px){ :deep(.inner) { color:red; } :global(.g) { color:blue; } :slotted(.s) { color:green; } } }:is(:deep(.d), .n) { color: blue; @media (min-width:1px){ .x :deep(.inner) { color:red; } } }</style>"#,
        );
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                id: Some("data-v-test".into()),
                scoped: true,
                ..SfcStyleCompileOptions::default()
            },
        );

        assert!(result.code.contains("[data-v-test] .d {"));
        assert!(result.code.contains(" .inner { color:red;"));
        assert!(result.code.contains(".g { color:blue;"));
        assert!(result.code.contains(".s { color:green;"));
        assert!(result
            .code
            .contains(":is([data-v-test] .d, .n[data-v-test]) { color: blue;"));
        assert!(result.code.contains(".x .inner { color:red;"));
        assert!(!result.code.contains(":deep(.inner)"));
        assert!(!result.code.contains(":global(.g)"));
        assert!(!result.code.contains(":slotted(.s)"));
    }

    #[test]
    fn compile_style_emits_vue3_deprecated_deep_warnings() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "style.vue",
            r#"<style scoped>>>> .foo { color: red; } ::v-deep .bar { color: blue; }</style>"#,
        );
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                id: Some("data-v-test".into()),
                scoped: true,
                ..SfcStyleCompileOptions::default()
            },
        );

        assert_eq!(result.diagnostics.len(), 2);
        assert!(result.diagnostics.iter().all(|diagnostic| {
            diagnostic.code == "VUEC_STYLE_DEPRECATED_SCOPED_SELECTOR"
                && diagnostic.severity == Severity::Warning
        }));
        assert!(result.diagnostics[0]
            .message
            .contains("the >>> and /deep/ combinators have been deprecated"));
        assert!(result.diagnostics[1]
            .message
            .contains("::v-deep usage as a combinator has been deprecated"));
        assert!(result.errors.is_empty());
    }

    #[test]
    fn compile_style_source_map_merges_style_blocks_to_vue_source() {
        let mut compiler = SfcCompiler::new();
        let source = "<style>.a { color: red; }</style>\n<style>.b { color: blue; }</style>";
        let descriptor = compiler.parse("multi.vue", source);
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                source_map: true,
                ..SfcStyleCompileOptions::default()
            },
        );
        let map = result.map.expect("merged style source map");

        assert_eq!(map.sources, vec!["multi.vue"]);
        assert_eq!(
            map.sources_content
                .as_ref()
                .and_then(|sources| sources[0].as_ref()),
            Some(&source.to_string())
        );
        let first = map
            .original_position(vuec_source::GeneratedPosition::new(0, 0))
            .unwrap()
            .expect("first style mapping");
        assert_eq!(first.source, "multi.vue");
        assert_eq!(first.line, 0);
        assert_eq!(first.column, "<style>".len() as u32);
        let second_generated_line = result.code.lines().count().saturating_sub(1) as u32;
        let second = map
            .original_position(vuec_source::GeneratedPosition::new(
                second_generated_line,
                0,
            ))
            .unwrap()
            .expect("second style mapping");
        assert_eq!(second.source, "multi.vue");
        assert_eq!(second.line, 1);
        assert_eq!(second.column, "<style>".len() as u32);
    }

    #[test]
    fn compile_style_source_map_skips_empty_style_blocks() {
        let mut compiler = SfcCompiler::new();
        let source = "<style></style>\n<style>.b { color: blue; }</style>";
        let descriptor = compiler.parse("multi.vue", source);
        let result = compiler.compile_style(
            &descriptor,
            SfcStyleCompileOptions {
                source_map: true,
                ..SfcStyleCompileOptions::default()
            },
        );
        let map = result.map.expect("merged style source map");

        assert_eq!(result.code, ".b { color: blue;\n}");
        let first = map
            .original_position(vuec_source::GeneratedPosition::new(0, 0))
            .unwrap()
            .expect("non-empty style mapping");
        assert_eq!(first.source, "multi.vue");
        assert_eq!(first.line, 1);
        assert_eq!(first.column, "<style>".len() as u32);
    }

    #[test]
    fn compile_style_diagnostics_map_to_vue_source_offsets() {
        let mut compiler = SfcCompiler::new();
        let source = "<template><div/></template>\n<style>\n.a { color: red; }\n@import \"missing.css\";\n</style>";
        let descriptor = compiler.parse("diagnostic.vue", source);
        let result = compiler.compile_style(&descriptor, SfcStyleCompileOptions::default());

        assert_eq!(result.errors, vec!["style import could not be resolved"]);
        assert_eq!(result.diagnostics.len(), 1);
        let import_start = source.find("@import").expect("import start");
        let import_end = import_start + "@import \"missing.css\";".len();
        assert_eq!(
            result.diagnostics[0].span,
            Some(Span::new(descriptor.source_file, import_start, import_end))
        );
    }

    #[test]
    fn compile_template_uses_ssr_backend() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse("foo.vue", r#"<template><div>{{ msg }}</div></template>"#);
        let template = compiler.compile_template(
            &descriptor,
            SfcTemplateCompileOptions {
                ssr: true,
                ..SfcTemplateCompileOptions::default()
            },
        );
        assert!(template.code.contains("ssrRender"));
        assert!(template.code.contains("_ssrInterpolate(_ctx.msg)"));
    }

    #[test]
    fn compile_template_passes_asset_url_base_to_dom_backend() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<template><img src="./logo.png"><img src="~logo.png"><img srcset="@/logo.png 1x, ./logo.png 2x"></template>"#,
        );
        let template = compiler.compile_template(
            &descriptor,
            SfcTemplateCompileOptions {
                asset_url_options: AssetUrlOptions {
                    base: Some("/foo".into()),
                    ..AssetUrlOptions::default()
                },
                ..SfcTemplateCompileOptions::default()
            },
        );

        assert!(template.code.contains(r#"src: "/foo/logo.png""#));
        assert!(template.code.contains("import _imports_0 from 'logo.png'"));
        assert!(template
            .code
            .contains("import _imports_1 from '@/logo.png'"));
        assert!(template.code.contains("src: _imports_0"));
        assert!(template
            .code
            .contains(r#"srcset: _imports_1 + ' 1x, ' + "/foo/logo.png" + ' 2x'"#));
        assert!(!template.code.contains(r#"src: "~logo.png""#));
    }

    #[test]
    fn compile_template_supports_custom_asset_url_tags() {
        let mut compiler = SfcCompiler::new();
        let descriptor =
            compiler.parse("foo.vue", r#"<template><foo bar="~baz"></foo></template>"#);
        let mut tags = BTreeMap::new();
        tags.insert("foo".into(), vec!["bar".into()]);
        let template = compiler.compile_template(
            &descriptor,
            SfcTemplateCompileOptions {
                asset_url_options: AssetUrlOptions {
                    tags,
                    ..AssetUrlOptions::default()
                },
                ..SfcTemplateCompileOptions::default()
            },
        );

        assert!(template.code.contains("import _imports_0 from 'baz'"));
        assert!(template.code.contains("bar: _imports_0"));
    }

    #[test]
    fn compile_template_transforms_asset_urls_to_imports() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<template><img src="./logo.png" srcset="./logo.png 2x"><img src="@theme/logo.png"></template>"#,
        );
        let template = compiler.compile_template(&descriptor, SfcTemplateCompileOptions::default());

        assert!(template
            .code
            .contains("import _imports_0 from './logo.png'"));
        assert!(template
            .code
            .contains("import _imports_1 from '@theme/logo.png'"));
        assert!(template.code.contains("src: _imports_0"));
        assert!(template.code.contains("srcset: _imports_0 + ' 2x'"));
        assert!(!template.code.contains("_ctx._imports_"));
        assert!(!template.code.contains("PROPS"));
    }

    #[test]
    fn compile_template_uses_official_cache_handler_default() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<template><input @blur="onBlur" @[validateEvent]="onValidateEvent"></template>"#,
        );
        let template = compiler.compile_template(&descriptor, SfcTemplateCompileOptions::default());

        assert!(template.code.contains("toHandlerKey as _toHandlerKey"));
        assert!(template.code.contains("mergeProps as _mergeProps"));
        assert!(template.code.contains(
            "_cache[0] || (_cache[0] = (...args) => (_ctx.onBlur && _ctx.onBlur(...args)))"
        ));
        assert!(template.code.contains("_cache[1] || (_cache[1] = (...args) => (_ctx.onValidateEvent && _ctx.onValidateEvent(...args)))"));
        assert!(!template.code.contains("data-vuec-dom"));
    }

    #[test]
    fn compile_template_source_does_not_cache_dynamic_interpolation_subtrees() {
        let compiler = SfcCompiler::new();
        let template = compiler.compile_template_source(
            "contract.vue",
            r#"<template><div>{{ msg }}</div></template><script setup lang="ts">const msg = 'x'</script><style scoped>.a{ color: v-bind(color); }</style>"#,
            SfcTemplateCompileOptions {
                scope_id: Some("data-v-contract".into()),
                slotted: false,
                ssr: false,
                ..SfcTemplateCompileOptions::default()
            },
        );

        assert!(template.code.contains("_toDisplayString(_ctx.msg)"));
        assert!(template.code.contains("1 /* TEXT */"));
        assert!(!template.code.contains("-1 /* CACHED */"));
        assert!(!template.code.contains("[...(_cache[0]"));
        assert_eq!(template.errors.len(), 2);
        assert_eq!(template.errors[0].code, 64);
        assert_eq!(template.errors[1].code, 64);
    }

    #[test]
    fn compile_template_source_returns_dom_compile_errors() {
        let compiler = SfcCompiler::new();
        let template = compiler.compile_template_source(
            "x.vue",
            r#"<div :bar="a[" v-model="baz"/>"#,
            SfcTemplateCompileOptions::default(),
        );

        assert_eq!(template.errors.len(), 2);
        assert_eq!(template.errors[0].code, 46);
        assert_eq!(template.errors[0].loc.start.offset, 13);
        assert_eq!(template.errors[1].code, 58);
        assert_eq!(template.errors[1].loc.source, r#"v-model="baz""#);
    }

    #[test]
    fn vue27_template_preprocessor_compiles_pug_and_reports_missing_lang() {
        let compiler = SfcCompiler::new();
        let pug = compiler.preprocess_vue27_template(
            "body\n h1 Pug Examples\n div.container\n   p Cool Pug example!\n",
            Vue27TemplatePreprocessOptions {
                lang: Some("pug".into()),
                filename: Some("example.vue".into()),
            },
        );

        assert!(pug.errors.is_empty());
        assert_eq!(
            pug.source,
            r#"<body><h1>Pug Examples</h1><div class="container"><p>Cool Pug example!</p></div></body>"#
        );

        let missing = compiler.preprocess_vue27_template(
            "",
            Vue27TemplatePreprocessOptions {
                lang: Some("unknownLang".into()),
                filename: Some("example.vue".into()),
            },
        );
        assert_eq!(missing.errors.len(), 1);
        assert_eq!(missing.tips.len(), 1);
        assert!(missing.errors[0].contains("however it is not installed"));
        assert!(missing.tips[0].contains("Please install"));
    }

    #[test]
    fn compile_template_ssr_transforms_asset_urls_to_imports() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "foo.vue",
            r#"<template><img src="./logo.png" srcset="./logo.png 2x"></template>"#,
        );
        let template = compiler.compile_template(
            &descriptor,
            SfcTemplateCompileOptions {
                ssr: true,
                ..SfcTemplateCompileOptions::default()
            },
        );

        assert!(template
            .code
            .contains("import _imports_0 from './logo.png'"));
        assert!(template.code.contains("src: _imports_0"));
        assert!(template.code.contains("srcset: _imports_0 + ' 2x'"));
        assert!(template.code.contains("_ssrRenderAttrs(_mergeProps("));
        assert!(!template.code.contains("</img>"));
        assert!(!template.code.contains("_ctx._imports_"));
    }

    #[test]
    fn compile_template_source_ssr_respects_disabled_asset_url_transform() {
        let compiler = SfcCompiler::new();
        let template = compiler.compile_template_source(
            "foo.vue",
            r#"<img src="./logo.png">"#,
            SfcTemplateCompileOptions {
                ssr: true,
                transform_asset_urls: false,
                ..SfcTemplateCompileOptions::default()
            },
        );

        assert!(!template.code.contains("import _imports_0"));
        assert!(template.code.contains(r#"src: "./logo.png""#));
        assert!(template.code.contains("_ssrRenderAttrs(_mergeProps("));
        assert!(!template.code.contains("</img>"));
    }
}
