//! Vue single-file component compiler implementation.
//!
//! This crate owns SFC descriptor parsing, Vue 2.7 `parseComponent`
//! projection, Vue 3 template/script/style compile entry points, Vue 2.7
//! SFC helper APIs, descriptor caching, and source-map/error shapes shared by
//! the CLI, NAPI, WASM, and package-alias layers.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

use oxc_ast::ast::{
    Argument, ArrowFunctionExpression, AssignmentTarget, BindingPattern, Declaration,
    ExportDefaultDeclaration, ExportDefaultDeclarationKind, ExportNamedDeclaration,
    ExportSpecifier, Expression, FormalParameter, Function, ImportDeclarationSpecifier,
    ImportOrExportKind, ModuleExportName, ObjectExpression, ObjectProperty, ObjectPropertyKind,
    PropertyKey, SimpleAssignmentTarget, Statement, TSFunctionType, TSInterfaceBody, TSLiteral,
    TSSignature, TSType, TSTypeLiteral, TSTypeName, VariableDeclaration, VariableDeclarationKind,
    WithStatement,
};
use oxc_span::GetSpan;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};
use vuec_codegen::{SourceMapArtifact, SourceMapBuilder};
use vuec_diagnostics::{Diagnostic, Severity};
use vuec_html::{HtmlAttribute, HtmlTokenKind, HtmlTokenizer};
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
    vue27_errors: Vec<Vue27SfcParseError>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum SfcParseCacheMode {
    Raw,
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
        let filename = filename.into();
        let key = SfcCacheKey::new(filename.clone(), source, SfcParseCacheMode::Raw);
        if let Some(entry) = self.descriptor_cache.get(&key) {
            self.cache_stats.descriptor_hits += 1;
            return entry.descriptor.clone();
        }
        self.invalidate_stale_descriptor_entries(&filename, &key.mode);
        self.cache_stats.descriptor_misses += 1;
        let source_file = self.sources.add_file(
            Some(std::path::PathBuf::from(&filename)),
            source.to_string(),
        );
        let descriptor = descriptor_from_blocks(
            filename,
            source,
            source_file,
            extract_sfc_blocks(source, source_file, SfcBlockContentMode::Raw).blocks,
        );
        self.descriptor_cache.insert(
            key,
            SfcDescriptorCacheEntry {
                descriptor: descriptor.clone(),
                vue27_errors: Vec::new(),
            },
        );
        descriptor
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
        _options: SfcScriptCompileOptions,
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
        let bindings = script_bindings(&summary.bindings);
        let imports = summary.imports;
        let attrs = descriptor
            .script
            .as_ref()
            .or(descriptor.script_setup.as_ref())
            .map(|block| block.attrs.clone())
            .unwrap_or_default();
        SfcScriptBlock {
            type_name: "script".into(),
            content: script_content(
                descriptor,
                &raw_content,
                &summary.bindings,
                descriptor.filename.as_str(),
            ),
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
            errors: summary.errors,
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
        let modules = (!modules.is_empty()).then_some(modules);
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
    errors: Vec<Vue27SfcParseError>,
}

#[derive(Clone, Copy)]
enum SfcBlockContentMode<'a> {
    Raw,
    Vue27 {
        options: &'a Vue27ParseComponentOptions,
    },
}

struct OpenSfcBlock {
    type_name: String,
    attrs: SfcBlockAttrs,
    start: usize,
    open_end: usize,
    self_closing: bool,
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
    let mut errors = Vec::new();
    let mut stack: Vec<(String, usize, usize)> = Vec::new();
    let mut current_block: Option<OpenSfcBlock> = None;
    let mut depth = 0usize;
    let mut malformed_tail_start = None;
    let mut tokenizer = HtmlTokenizer::new(source);

    loop {
        let token = tokenizer.next_token();
        match token.kind {
            HtmlTokenKind::StartTag {
                name,
                attributes,
                self_closing,
            } => {
                if depth == 0 {
                    current_block = Some(OpenSfcBlock {
                        type_name: name.clone(),
                        attrs: attrs_from_html(&attributes),
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
                        ));
                    }
                }
            }
            HtmlTokenKind::EndTag { name } => {
                if depth == 0 {
                    continue;
                }
                let Some(pos) = matching_open_pos(&stack, &name) else {
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
                    }
                    continue;
                };
                while stack.len() > pos + 1 {
                    if let Some((tag, start, end)) = stack.pop() {
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
                        blocks.push(finish_sfc_block(
                            source,
                            source_file,
                            mode,
                            open,
                            token.start,
                            token.end,
                        ));
                    }
                }
                depth = depth.saturating_sub(1);
            }
            HtmlTokenKind::Eof => {
                while let Some((tag, start, end)) = stack.pop() {
                    errors.push(Vue27SfcParseError {
                        msg: format!("tag <{tag}> has no matching end tag."),
                        start: Some(start),
                        end: Some(end),
                    });
                    if stack.is_empty() {
                        if let Some(open) = current_block.take() {
                            let fallback_end = malformed_tail_start.unwrap_or_else(|| {
                                malformed_tail_content_end(source, &open, token.start)
                            });
                            blocks.push(finish_sfc_block(
                                source,
                                source_file,
                                mode,
                                open,
                                fallback_end,
                                token.end,
                            ));
                        }
                    }
                }
                break;
            }
            _ => {}
        }
    }

    blocks.sort_by_key(|block| block.loc.start);
    ExtractedSfcBlocks { blocks, errors }
}

fn consume_plain_text_element(
    source: &str,
    source_file: FileId,
    mode: SfcBlockContentMode<'_>,
    tokenizer: &mut HtmlTokenizer<'_>,
    blocks: &mut Vec<SfcBlock>,
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
        ));
    } else {
        tokenizer.set_cursor(source.len());
        blocks.push(finish_sfc_block(
            source,
            source_file,
            mode,
            open,
            source.len(),
            source.len(),
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
) -> SfcBlock {
    let content_start = open.open_end.min(source.len());
    let raw_end = content_end.min(source.len()).max(content_start);
    let mut content = source[content_start..raw_end].to_string();
    if let SfcBlockContentMode::Vue27 { options } = mode {
        if should_vue27_deindent(&open, options) {
            content = deindent(&content);
        }
        if open.type_name != "template" && options.pad.is_enabled() {
            content = vue27_pad_content(source, &open, &options.pad) + &content;
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
    }
}

fn matching_open_pos(stack: &[(String, usize, usize)], name: &str) -> Option<usize> {
    let lower_name = name.to_ascii_lowercase();
    stack
        .iter()
        .rposition(|(tag, _, _)| tag.to_ascii_lowercase() == lower_name)
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

fn attrs_from_html(attributes: &[HtmlAttribute]) -> SfcBlockAttrs {
    let mut attrs = SfcBlockAttrs::default();
    for attribute in attributes {
        let value = attribute
            .value
            .as_ref()
            .map(|value| SfcAttrValue::String(value.clone()))
            .unwrap_or(SfcAttrValue::Bool(true));
        attrs.raw.insert(attribute.name.clone(), value.clone());
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
    matches!(name.to_ascii_lowercase().as_str(), "script" | "style")
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

fn vue27_export_named_declaration_only_exports_default(
    declaration: &ExportNamedDeclaration<'_>,
) -> bool {
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
                analysis.has_default_export_name = vue27_default_export_has_name(declaration);
                edits.overwrite(
                    declaration.span.start as usize,
                    declaration.declaration.span().start as usize,
                    "const __default__ = ",
                );
            }
            Statement::ExportNamedDeclaration(declaration) => {
                if rewrite_named_default_exports(source, "__default__", declaration, &mut edits) {
                    analysis.has_default_export = true;
                    if vue27_export_named_declaration_only_exports_default(declaration) {
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

fn vue27_default_export_has_name(declaration: &ExportDefaultDeclaration<'_>) -> bool {
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
        props_type_declarations: analysis.props_type_declarations,
        emits_type_declarations: analysis.emits_type_declarations,
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

fn vue27_template_uses_identifier(template: &str, local: &str, is_ts: bool) -> bool {
    let usage = vue27_template_usage_check_string(template, is_ts);
    identifier_usage_contains(&usage, local)
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

fn script_content(
    descriptor: &SfcDescriptor,
    raw_content: &str,
    bindings: &[String],
    filename: &str,
) -> String {
    let Some(script_setup) = descriptor.script_setup.as_ref() else {
        return raw_content.to_string();
    };
    let component_name = script_component_name(filename);
    let returned = bindings
        .iter()
        .filter(|name| !name.starts_with("import:") && !name.starts_with("export:"))
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    let returned = if returned.is_empty() {
        "{}".to_string()
    } else {
        format!("{{ {returned} }}")
    };
    format!(
        "import {{ defineComponent as _defineComponent }} from 'vue'\n{}\nexport default /*@__PURE__*/_defineComponent({{\n  __name: '{}',\n  setup(__props, {{ expose: __expose }}) {{\n  __expose();\n\nconst __returned__ = {}\nObject.defineProperty(__returned__, '__isScriptSetup', {{ enumerable: false, value: true }})\nreturn __returned__\n}}\n\n}})",
        script_setup.content, component_name, returned
    )
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
    fn compile_style_rewrites_deep_nested_scoped_rules() {
        let mut compiler = SfcCompiler::new();
        let descriptor = compiler.parse(
            "style.vue",
            r#"<style scoped>:deep(.foo, .bar) { color: blue; .child { color: red; } @media (min-width: 1px) { .inner { color: green; } } }</style>"#,
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
