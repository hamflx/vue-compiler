use crate::*;

/// Template source plus location metadata.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateSource {
    /// Logical filename used for diagnostics and source maps.
    pub filename: String,
    /// Template source text.
    pub source: String,
    /// Source file id used by spans.
    pub file_id: FileId,
    /// Byte offset of `source` inside the original file.
    pub base_offset: usize,
}

/// Options shared by Vue 3 parser, transform, lowering, and codegen stages.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3CompilerOptions {
    /// Whether identifiers should be prefixed with render context bindings.
    pub prefix_identifiers: bool,
    /// Codegen mode, usually `function` or `module`.
    pub mode: String,
    /// Whether static trees should be hoisted.
    pub hoist_static: bool,
    /// Whether eligible static DOM trees should be stringified.
    pub stringify_static: bool,
    /// Whether stringify-static codegen should preserve helpers registered by
    /// the public transformHoist pipeline for official snapshot parity.
    #[serde(default)]
    pub stringify_static_preserve_helpers: bool,
    /// Whether event handlers should be cached.
    pub cache_handlers: bool,
    /// Optional scope id for scoped styles.
    pub scope_id: Option<String>,
    /// Whether slotted scope markers should be emitted.
    pub slotted: bool,
    /// Whether expressions should be parsed as TypeScript.
    pub is_ts: bool,
    /// Additional expression parser plugin names.
    pub expression_plugins: Vec<String>,
    /// Whether source maps should be generated.
    pub source_map: bool,
    /// Whether comments should be retained.
    pub comments: bool,
    /// Custom interpolation delimiters.
    pub delimiters: Option<[String; 2]>,
    /// Tags treated as void tags by the parser.
    pub void_tags: Vec<String>,
    /// Optional native tag allow-list.
    pub native_tags: Option<Vec<String>>,
    /// Tags treated as custom elements.
    pub custom_elements: Vec<String>,
    /// Tags treated as built-in components.
    pub built_in_components: Vec<String>,
    /// Per-tag namespace overrides.
    pub namespaces: BTreeMap<String, vuec_ast::HtmlNamespace>,
    /// Initial parser namespace.
    pub root_namespace: vuec_ast::HtmlNamespace,
    /// Whether DOM namespace transition rules are enabled.
    pub dom_namespaces: bool,
    /// Whitespace handling mode.
    pub whitespace: String,
    /// Tags that enable `v-pre`-like raw text preservation.
    pub pre_tags: Vec<String>,
    /// Tags whose leading newline should be ignored.
    pub ignore_newline_tags: Vec<String>,
    /// Whether parser behavior is for an SFC template block.
    pub sfc_parse_mode: bool,
    /// Plain template languages accepted by SFC parsing.
    pub sfc_plain_template_langs: Vec<String>,
    /// Binding metadata used by expression transforms.
    pub binding_metadata: BTreeMap<String, String>,
    /// Public props alias metadata used by inline template codegen.
    pub props_aliases: BTreeMap<String, String>,
    /// Whether compilation targets inline render setup output.
    pub inline: bool,
    /// Whether SSR codegen/lowering is enabled.
    pub ssr: bool,
    /// Whether module imports should be optimized.
    pub optimize_imports: bool,
    /// Original source text used for source-map generation.
    pub source_map_source: Option<String>,
    /// Base offset for source-map mappings.
    pub source_map_base_offset: usize,
    /// SSR CSS vars expression, when compiling inline SSR templates.
    pub ssr_css_vars: Option<String>,
}

impl Default for Vue3CompilerOptions {
    fn default() -> Self {
        Self {
            prefix_identifiers: false,
            mode: "function".into(),
            hoist_static: false,
            stringify_static: false,
            stringify_static_preserve_helpers: false,
            cache_handlers: false,
            scope_id: None,
            slotted: false,
            is_ts: false,
            expression_plugins: Vec::new(),
            source_map: false,
            comments: true,
            delimiters: None,
            void_tags: Vec::new(),
            native_tags: None,
            custom_elements: Vec::new(),
            built_in_components: Vec::new(),
            namespaces: BTreeMap::new(),
            root_namespace: vuec_ast::HtmlNamespace::Html,
            dom_namespaces: false,
            whitespace: "condense".into(),
            pre_tags: Vec::new(),
            ignore_newline_tags: Vec::new(),
            sfc_parse_mode: false,
            sfc_plain_template_langs: Vec::new(),
            binding_metadata: BTreeMap::new(),
            props_aliases: BTreeMap::new(),
            inline: false,
            ssr: false,
            optimize_imports: false,
            source_map_source: None,
            source_map_base_offset: 0,
            ssr_css_vars: None,
        }
    }
}

/// Generated render output and compiler metadata.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodegenResult {
    /// Generated JavaScript render code.
    pub code: String,
    /// Optional source map artifact.
    pub map: Option<SourceMapArtifact>,
    /// Deterministic AST or transform summary string.
    pub ast_summary: String,
    /// Diagnostics produced during parsing or transforms.
    pub diagnostics: Vec<Diagnostic>,
    /// Generated import/helper preamble.
    pub preamble: String,
}

/// Result of the structural Vue 3 DOM lowering contract.
///
/// This is intentionally separate from the legacy exact emitter path so AST /
/// HIR / MIR structure can be verified without changing current official
/// conformance behavior.
pub struct Vue3DomLoweringResult {
    /// Lowered shared HIR document.
    pub hir: Hir,
    /// Lowered Vue 3 DOM target MIR document.
    pub mir: Vue3DomMir,
    /// AST-to-HIR and HIR-to-MIR edge map.
    pub map: LoweringMap,
    /// JavaScript side store used by HIR/MIR expression ids.
    pub js: JsAstStore,
}

/// Generate structural Vue 3 DOM render code from target-split DOM MIR.
///
/// This is intentionally separate from the legacy exact AST emitter. It only
/// consumes `Vue3DomMir` plus the registered JS store, so it can be used to
/// verify that target codegen is moving behind the AST -> HIR -> MIR boundary.
pub fn generate_vue3_dom_mir(
    mir: &Vue3DomMir,
    js: &JsAstStore,
    options: &Vue3CompilerOptions,
) -> CodegenResult {
    Vue3DomMirCodegen::new(mir, js, options).generate()
}

/// Generate structural Vue 3 SSR render code from target-split SSR MIR.
///
/// This emitter is intentionally separate from the legacy exact AST emitter.
/// It consumes only `Vue3SsrMir` plus the registered JS store, so SSR target
/// codegen can be verified behind the AST -> HIR -> MIR boundary.
pub fn generate_vue3_ssr_mir(
    mir: &Vue3SsrMir,
    js: &JsAstStore,
    options: &Vue3CompilerOptions,
) -> CodegenResult {
    Vue3SsrMirCodegen::new(mir, js, options).generate()
}

/// Result of the structural Vue 3 SSR lowering contract.
///
/// SSR lowering has its own target MIR and must not be derived from DOM MIR.
pub struct Vue3SsrLoweringResult {
    /// Lowered shared HIR document.
    pub hir: Hir,
    /// Lowered Vue 3 SSR target MIR document.
    pub mir: Vue3SsrMir,
    /// AST-to-HIR and HIR-to-MIR edge map.
    pub map: LoweringMap,
    /// JavaScript side store used by HIR/MIR expression ids.
    pub js: JsAstStore,
}
