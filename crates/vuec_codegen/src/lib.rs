#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Shared code emission and source-map result types.
//!
//! Compiler backends use this crate for deterministic string emission and for
//! serializable source-map artifacts passed through Rust, CLI, NAPI, and WASM
//! package boundaries.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::ops::{Deref, DerefMut};
use vuec_ast::{Mir, MirTarget, RuntimeHelper};
use vuec_source::{GeneratedPosition, Loc, SourceMap, SourceMapEntry, SourceMapTrace, Span};

/// Whitespace behavior used by code writers and emitters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WhitespaceMode {
    /// Preserve caller-provided whitespace exactly.
    Exact,
    /// Normalize indentation and trailing whitespace through the writer.
    #[default]
    Pretty,
    /// Compact output by suppressing indentation-only writes.
    Condensed,
}

/// Code generation target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CodegenTarget {
    /// Vue 2 render-function target.
    Vue2,
    /// Vue 3 DOM render-function target.
    Vue3Dom,
    /// Vue 3 SSR render-function target.
    Vue3Ssr,
    /// Vue Vapor target.
    Vapor,
}

impl From<MirTarget> for CodegenTarget {
    fn from(value: MirTarget) -> Self {
        match value {
            MirTarget::Vue2 => Self::Vue2,
            MirTarget::Vue3Dom => Self::Vue3Dom,
            MirTarget::Vue3Ssr => Self::Vue3Ssr,
            MirTarget::Vapor => Self::Vapor,
        }
    }
}

/// Shared options for exact emitters.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmitOptions {
    /// Code generation target.
    pub target: CodegenTarget,
    /// Whitespace behavior.
    pub whitespace: WhitespaceMode,
    /// Whether a source map should be produced.
    pub source_map: bool,
    /// Generated file name used in source maps.
    pub filename: Option<String>,
    /// Runtime helper import source for module output.
    pub runtime_module: String,
}

impl EmitOptions {
    /// Creates default options for a target.
    pub fn for_target(target: CodegenTarget) -> Self {
        Self {
            target,
            whitespace: WhitespaceMode::default(),
            source_map: false,
            filename: None,
            runtime_module: "vue".into(),
        }
    }

    /// Creates default options from a target-split MIR document.
    pub fn for_mir(mir: &Mir) -> Self {
        Self::for_target(mir.target().into())
    }
}

impl Default for EmitOptions {
    fn default() -> Self {
        Self::for_target(CodegenTarget::Vue3Dom)
    }
}

/// Small indentation-aware code writer used by codegen backends.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CodeWriter {
    code: String,
    indent: usize,
    at_line_start: bool,
    indent_unit: &'static str,
    whitespace: WhitespaceMode,
    generated_line: u32,
    generated_column: u32,
}

impl CodeWriter {
    /// Creates an empty writer using two spaces per indentation level.
    pub fn new() -> Self {
        Self::with_whitespace(WhitespaceMode::default())
    }

    /// Creates an exact writer that never inserts indentation automatically.
    pub fn exact() -> Self {
        Self::with_whitespace(WhitespaceMode::Exact)
    }

    /// Creates an empty writer with a whitespace mode.
    pub fn with_whitespace(whitespace: WhitespaceMode) -> Self {
        Self {
            code: String::new(),
            indent: 0,
            at_line_start: true,
            indent_unit: "  ",
            whitespace,
            generated_line: 0,
            generated_column: 0,
        }
    }

    /// Sets the indentation unit used by pretty output.
    pub fn set_indent_unit(&mut self, indent_unit: &'static str) {
        self.indent_unit = indent_unit;
    }

    /// Appends text while applying indentation at line starts.
    pub fn push_str(&mut self, text: &str) {
        for segment in text.split_inclusive('\n') {
            if self.should_indent() {
                for _ in 0..self.indent {
                    self.append_raw_segment(self.indent_unit);
                }
            }
            self.append_raw_segment(segment);
        }
    }

    /// Appends text without inserting indentation.
    pub fn push_raw(&mut self, text: &str) {
        self.append_raw_segment(text);
    }

    /// Appends one line and then writes a newline.
    pub fn push_line(&mut self, text: &str) {
        self.push_str(text);
        self.newline();
    }

    /// Writes a newline and marks the next write as line-start text.
    pub fn newline(&mut self) {
        self.append_raw_segment("\n");
    }

    /// Increases indentation for subsequent line-start writes.
    pub fn indent(&mut self) {
        self.indent += 1;
    }

    /// Decreases indentation without underflow.
    pub fn dedent(&mut self) {
        self.indent = self.indent.saturating_sub(1);
    }

    /// Returns the current generated position.
    pub fn generated_position(&self) -> GeneratedPosition {
        GeneratedPosition::new(self.generated_line, self.generated_column)
    }

    /// Returns the current output byte length.
    pub fn len(&self) -> usize {
        self.code.len()
    }

    /// Returns `true` when no code has been emitted.
    pub fn is_empty(&self) -> bool {
        self.code.is_empty()
    }

    /// Returns the current code buffer.
    pub fn as_str(&self) -> &str {
        &self.code
    }

    /// Consumes the writer and returns generated code.
    pub fn finish(self) -> String {
        self.code
    }

    fn should_indent(&self) -> bool {
        self.at_line_start && self.indent > 0 && matches!(self.whitespace, WhitespaceMode::Pretty)
    }

    fn append_raw_segment(&mut self, text: &str) {
        self.code.push_str(text);
        for ch in text.chars() {
            if ch == '\n' {
                self.generated_line += 1;
                self.generated_column = 0;
                self.at_line_start = true;
            } else {
                self.generated_column += ch.len_utf16() as u32;
                self.at_line_start = false;
            }
        }
    }
}

/// Scoped indentation guard for manual writer usage.
pub struct IndentGuard<'a> {
    writer: &'a mut CodeWriter,
}

impl<'a> IndentGuard<'a> {
    /// Creates a guard that increases indentation until dropped.
    pub fn new(writer: &'a mut CodeWriter) -> Self {
        writer.indent();
        Self { writer }
    }
}

impl Drop for IndentGuard<'_> {
    fn drop(&mut self) {
        self.writer.dedent();
    }
}

impl Deref for IndentGuard<'_> {
    type Target = CodeWriter;

    fn deref(&self) -> &Self::Target {
        self.writer
    }
}

impl DerefMut for IndentGuard<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.writer
    }
}

/// Runtime helper metadata used by codegen preambles.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeHelperSpec {
    /// Runtime helper enum value.
    pub helper: RuntimeHelper,
    /// Canonical runtime export name.
    pub name: &'static str,
    /// Local alias used inside generated code.
    pub alias: &'static str,
    /// Runtime package imported in module mode.
    pub import_source: &'static str,
    /// Whether the helper belongs to Vue 3 SSR helpers.
    pub ssr: bool,
}

/// Returns canonical runtime helper metadata.
pub fn runtime_helper_spec(helper: RuntimeHelper) -> RuntimeHelperSpec {
    let name = runtime_helper_name(helper);
    RuntimeHelperSpec {
        helper,
        name,
        alias: runtime_helper_alias(helper),
        import_source: runtime_helper_import_source(helper),
        ssr: is_vue3_ssr_helper(helper),
    }
}

/// Returns the canonical runtime helper export name.
pub fn runtime_helper_name(helper: RuntimeHelper) -> &'static str {
    match helper {
        RuntimeHelper::Vue2CreateElement => "createElement",
        RuntimeHelper::Vue2CreateTextVNode => "createTextVNode",
        RuntimeHelper::Vue2ToString => "toString",
        RuntimeHelper::Vue2RenderList => "renderList",
        RuntimeHelper::Vue2ResolveFilter => "resolveFilter",
        RuntimeHelper::Vue3ResolveDirective => "resolveDirective",
        RuntimeHelper::Vue3WithDirectives => "withDirectives",
        RuntimeHelper::Vue3SetBlockTracking => "setBlockTracking",
        RuntimeHelper::Vue3OpenBlock => "openBlock",
        RuntimeHelper::Vue3CreateElementVNode => "createElementVNode",
        RuntimeHelper::Vue3CreateElementBlock => "createElementBlock",
        RuntimeHelper::Vue3CreateCommentVNode => "createCommentVNode",
        RuntimeHelper::Vue3CreateTextVNode => "createTextVNode",
        RuntimeHelper::Vue3Fragment => "Fragment",
        RuntimeHelper::Vue3ToDisplayString => "toDisplayString",
        RuntimeHelper::Vue3RenderList => "renderList",
        RuntimeHelper::Vue3RenderSlot => "renderSlot",
        RuntimeHelper::Vue3NormalizeClass => "normalizeClass",
        RuntimeHelper::Vue3NormalizeProps => "normalizeProps",
        RuntimeHelper::Vue3NormalizeStyle => "normalizeStyle",
        RuntimeHelper::Vue3GuardReactiveProps => "guardReactiveProps",
        RuntimeHelper::Vue3MergeProps => "mergeProps",
        RuntimeHelper::Vue3ResolveComponent => "resolveComponent",
        RuntimeHelper::Vue3ResolveDynamicComponent => "resolveDynamicComponent",
        RuntimeHelper::Vue3BaseTransition => "BaseTransition",
        RuntimeHelper::Vue3Transition => "Transition",
        RuntimeHelper::Vue3TransitionGroup => "TransitionGroup",
        RuntimeHelper::Vue3Teleport => "Teleport",
        RuntimeHelper::Vue3Suspense => "Suspense",
        RuntimeHelper::Vue3KeepAlive => "KeepAlive",
        RuntimeHelper::Vue3WithCtx => "withCtx",
        RuntimeHelper::Vue3CreateBlock => "createBlock",
        RuntimeHelper::Vue3CreateVNode => "createVNode",
        RuntimeHelper::Vue3CreateSlots => "createSlots",
        RuntimeHelper::Vue3CreateStaticVNode => "createStaticVNode",
        RuntimeHelper::Vue3IsMemoSame => "isMemoSame",
        RuntimeHelper::Vue3WithMemo => "withMemo",
        RuntimeHelper::Vue3ToHandlers => "toHandlers",
        RuntimeHelper::Vue3Camelize => "camelize",
        RuntimeHelper::Vue3Capitalize => "capitalize",
        RuntimeHelper::Vue3ToHandlerKey => "toHandlerKey",
        RuntimeHelper::Vue3PushScopeId => "pushScopeId",
        RuntimeHelper::Vue3PopScopeId => "popScopeId",
        RuntimeHelper::Vue3Unref => "unref",
        RuntimeHelper::Vue3IsRef => "isRef",
        RuntimeHelper::Vue3VModelRadio => "vModelRadio",
        RuntimeHelper::Vue3VModelCheckbox => "vModelCheckbox",
        RuntimeHelper::Vue3VModelText => "vModelText",
        RuntimeHelper::Vue3VModelSelect => "vModelSelect",
        RuntimeHelper::Vue3VModelDynamic => "vModelDynamic",
        RuntimeHelper::Vue3WithModifiers => "withModifiers",
        RuntimeHelper::Vue3WithKeys => "withKeys",
        RuntimeHelper::Vue3VShow => "vShow",
        RuntimeHelper::Vue3SsrInterpolate => "ssrInterpolate",
        RuntimeHelper::Vue3SsrRenderVNode => "ssrRenderVNode",
        RuntimeHelper::Vue3SsrRenderComponent => "ssrRenderComponent",
        RuntimeHelper::Vue3SsrRenderSlot => "ssrRenderSlot",
        RuntimeHelper::Vue3SsrRenderSlotInner => "ssrRenderSlotInner",
        RuntimeHelper::Vue3SsrRenderClass => "ssrRenderClass",
        RuntimeHelper::Vue3SsrRenderStyle => "ssrRenderStyle",
        RuntimeHelper::Vue3SsrRenderAttrs => "ssrRenderAttrs",
        RuntimeHelper::Vue3SsrRenderAttr => "ssrRenderAttr",
        RuntimeHelper::Vue3SsrRenderDynamicAttr => "ssrRenderDynamicAttr",
        RuntimeHelper::Vue3SsrRenderList => "ssrRenderList",
        RuntimeHelper::Vue3SsrIncludeBooleanAttr => "ssrIncludeBooleanAttr",
        RuntimeHelper::Vue3SsrLooseEqual => "ssrLooseEqual",
        RuntimeHelper::Vue3SsrLooseContain => "ssrLooseContain",
        RuntimeHelper::Vue3SsrRenderDynamicModel => "ssrRenderDynamicModel",
        RuntimeHelper::Vue3SsrGetDynamicModelProps => "ssrGetDynamicModelProps",
        RuntimeHelper::Vue3SsrRenderTeleport => "ssrRenderTeleport",
        RuntimeHelper::Vue3SsrRenderSuspense => "ssrRenderSuspense",
        RuntimeHelper::Vue3SsrGetDirectiveProps => "ssrGetDirectiveProps",
    }
}

/// Returns the local helper alias used by generated code.
pub fn runtime_helper_alias(helper: RuntimeHelper) -> &'static str {
    match helper {
        RuntimeHelper::Vue2CreateElement => "_c",
        RuntimeHelper::Vue2CreateTextVNode => "_v",
        RuntimeHelper::Vue2ToString => "_s",
        RuntimeHelper::Vue2RenderList => "_l",
        RuntimeHelper::Vue2ResolveFilter => "_f",
        _ => runtime_helper_name(helper),
    }
}

/// Returns the local helper reference used by Vue 3 generated code.
pub fn vue3_helper_reference(helper: RuntimeHelper) -> String {
    format!("_{}", runtime_helper_name(helper))
}

/// Returns the runtime package for a helper.
pub fn runtime_helper_import_source(helper: RuntimeHelper) -> &'static str {
    if is_vue3_ssr_helper(helper) {
        "vue/server-renderer"
    } else {
        "vue"
    }
}

/// Returns whether a helper is a Vue 3 SSR helper.
pub fn is_vue3_ssr_helper(helper: RuntimeHelper) -> bool {
    matches!(
        helper,
        RuntimeHelper::Vue3SsrInterpolate
            | RuntimeHelper::Vue3SsrRenderVNode
            | RuntimeHelper::Vue3SsrRenderComponent
            | RuntimeHelper::Vue3SsrRenderSlot
            | RuntimeHelper::Vue3SsrRenderSlotInner
            | RuntimeHelper::Vue3SsrRenderClass
            | RuntimeHelper::Vue3SsrRenderStyle
            | RuntimeHelper::Vue3SsrRenderAttrs
            | RuntimeHelper::Vue3SsrRenderAttr
            | RuntimeHelper::Vue3SsrRenderDynamicAttr
            | RuntimeHelper::Vue3SsrRenderList
            | RuntimeHelper::Vue3SsrIncludeBooleanAttr
            | RuntimeHelper::Vue3SsrLooseEqual
            | RuntimeHelper::Vue3SsrLooseContain
            | RuntimeHelper::Vue3SsrRenderDynamicModel
            | RuntimeHelper::Vue3SsrGetDynamicModelProps
            | RuntimeHelper::Vue3SsrRenderTeleport
            | RuntimeHelper::Vue3SsrRenderSuspense
            | RuntimeHelper::Vue3SsrGetDirectiveProps
    )
}

/// Renders Vue 3 object-destructure helper aliases.
pub fn helper_aliases(helpers: impl IntoIterator<Item = RuntimeHelper>) -> String {
    helpers
        .into_iter()
        .map(|helper| {
            let name = runtime_helper_name(helper);
            format!("{name}: _{name}")
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Renders Vue 3 module import helper aliases.
pub fn import_helper_aliases(helpers: impl IntoIterator<Item = RuntimeHelper>) -> String {
    helpers
        .into_iter()
        .map(|helper| {
            let name = runtime_helper_name(helper);
            format!("{name} as _{name}")
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Groups helpers by runtime import source.
pub fn helper_import_groups(
    helpers: impl IntoIterator<Item = RuntimeHelper>,
) -> BTreeMap<&'static str, Vec<RuntimeHelper>> {
    let mut groups = BTreeMap::<&'static str, BTreeSet<RuntimeHelper>>::new();
    for helper in helpers {
        groups
            .entry(runtime_helper_import_source(helper))
            .or_default()
            .insert(helper);
    }
    groups
        .into_iter()
        .map(|(source, helpers)| (source, helpers.into_iter().collect()))
        .collect()
}

/// Renders deterministic ES module helper imports.
pub fn render_helper_imports(helpers: impl IntoIterator<Item = RuntimeHelper>) -> String {
    helper_import_groups(helpers)
        .into_iter()
        .map(|(source, helpers)| {
            format!(
                "import {{ {} }} from \"{}\"",
                import_helper_aliases(helpers),
                source
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// One source-map mapping before VLQ encoding.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceMapMapping {
    /// One-based generated line.
    pub generated_line: usize,
    /// Zero-based generated column.
    pub generated_column: usize,
    /// Optional original source span.
    pub original: Option<Span>,
    /// Optional original source file name.
    pub source_name: Option<String>,
    /// Optional original symbol name.
    pub name: Option<String>,
}

impl SourceMapMapping {
    fn generated_position(&self) -> GeneratedPosition {
        GeneratedPosition::new(
            self.generated_line.saturating_sub(1) as u32,
            self.generated_column as u32,
        )
    }
}

/// Serializable source-map artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceMapArtifact {
    /// Source-map format version.
    pub version: u8,
    /// Optional generated file name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// Original source file names.
    pub sources: Vec<String>,
    /// Source-map symbol names.
    pub names: Vec<String>,
    /// Encoded VLQ mappings string.
    pub mappings: String,
    /// Optional source contents aligned with `sources`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sources_content: Option<Vec<Option<String>>>,
}

/// Source-map parse or remap error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceMapError {
    /// Human-readable error message.
    pub message: String,
}

impl std::fmt::Display for SourceMapError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for SourceMapError {}

/// Builder for `SourceMapArtifact`.
#[derive(Clone, Debug, Default)]
pub struct SourceMapBuilder {
    file: Option<String>,
    sources: Vec<String>,
    sources_content: BTreeMap<String, String>,
    names: Vec<String>,
    mappings: Vec<SourceMapMapping>,
}

impl SourceMapBuilder {
    /// Creates an empty source-map builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the generated file name.
    pub fn file(mut self, file: impl Into<String>) -> Self {
        self.file = Some(file.into());
        self
    }

    /// Adds original source content.
    pub fn add_source_content(&mut self, source: impl Into<String>, content: impl Into<String>) {
        let source = source.into();
        if !self.sources.iter().any(|existing| existing == &source) {
            self.sources.push(source.clone());
        }
        self.sources_content.insert(source, content.into());
    }

    /// Adds a generated-to-original mapping.
    pub fn add_mapping(
        &mut self,
        generated_line: usize,
        generated_column: usize,
        original: Option<Span>,
        source_name: Option<String>,
    ) {
        if let Some(name) = source_name.as_ref() {
            if !self.sources.iter().any(|existing| existing == name) {
                self.sources.push(name.clone());
            }
        }
        self.mappings.push(SourceMapMapping {
            generated_line,
            generated_column,
            original,
            source_name,
            name: None,
        });
    }

    /// Adds a named generated-to-original mapping.
    pub fn add_named_mapping(
        &mut self,
        generated_line: usize,
        generated_column: usize,
        original: Option<Span>,
        source_name: Option<String>,
        name: Option<String>,
    ) {
        if let Some(source_name) = source_name.as_ref() {
            if !self.sources.iter().any(|existing| existing == source_name) {
                self.sources.push(source_name.clone());
            }
        }
        if let Some(name) = name.as_ref() {
            if !self.names.iter().any(|existing| existing == name) {
                self.names.push(name.clone());
            }
        }
        self.mappings.push(SourceMapMapping {
            generated_line,
            generated_column,
            original,
            source_name,
            name,
        });
    }

    /// Adds a generated-to-original mapping at the writer's current position.
    pub fn add_mapping_at_writer(
        &mut self,
        writer: &CodeWriter,
        original: Option<Span>,
        source_name: Option<String>,
    ) {
        let generated = writer.generated_position();
        self.add_mapping(
            generated.line as usize + 1,
            generated.column as usize,
            original,
            source_name,
        );
    }

    /// Adds a source-map symbol name if it is not already present.
    pub fn add_name(&mut self, name: impl Into<String>) {
        let name = name.into();
        if !self.names.iter().any(|existing| existing == &name) {
            self.names.push(name);
        }
    }

    /// Merges another builder, offsetting generated lines from the merged map.
    pub fn merge(mut self, mut other: SourceMapBuilder, line_offset: usize) -> Self {
        for mapping in other.mappings.drain(..) {
            self.mappings.push(SourceMapMapping {
                generated_line: mapping.generated_line + line_offset,
                ..mapping
            });
        }
        self.extend_sources(other.sources);
        self.extend_names(other.names);
        self.sources_content.extend(other.sources_content);
        self
    }

    /// Returns a queryable source-map trace for mappings that carry source spans.
    pub fn trace(&self) -> SourceMapTrace {
        SourceMapTrace::new(
            self.mappings
                .iter()
                .filter_map(|mapping| {
                    Some(SourceMapEntry {
                        generated: mapping.generated_position(),
                        original: mapping.original?,
                    })
                })
                .collect(),
        )
    }

    /// Builds an encoded source-map artifact.
    pub fn build(self) -> SourceMapArtifact {
        let mut encoded = oxc_sourcemap::SourceMapBuilder::default();
        if let Some(file) = self.file.as_deref() {
            encoded.set_file(file);
        }
        let source_ids = self
            .sources
            .iter()
            .map(|source| {
                let content = self
                    .sources_content
                    .get(source)
                    .map(String::as_str)
                    .unwrap_or("");
                encoded.add_source_and_content(source, content)
            })
            .collect::<Vec<_>>();
        let name_ids = self
            .names
            .iter()
            .map(|name| encoded.add_name(name))
            .collect::<Vec<_>>();
        let mut mappings = self.mappings.iter().collect::<Vec<_>>();
        mappings.sort_by_key(|mapping| (mapping.generated_line, mapping.generated_column));
        for mapping in mappings {
            let source_id = mapping
                .source_name
                .as_ref()
                .and_then(|name| self.sources.iter().position(|source| source == name))
                .and_then(|index| source_ids.get(index).copied());
            let name_id = mapping
                .name
                .as_ref()
                .and_then(|name| self.names.iter().position(|existing| existing == name))
                .and_then(|index| name_ids.get(index).copied());
            let original_loc = mapping.original.and_then(|span| {
                mapping.source_name.as_ref().and_then(|source_name| {
                    self.sources_content.get(source_name).and_then(|content| {
                        loc_for_byte_offset(content, span.start.0).map(|loc| {
                            (
                                loc.line.saturating_sub(1) as u32,
                                loc.column.saturating_sub(1) as u32,
                            )
                        })
                    })
                })
            });
            encoded.add_token(
                mapping.generated_line.saturating_sub(1) as u32,
                mapping.generated_column as u32,
                original_loc
                    .map(|(line, _)| line)
                    .or_else(|| mapping.original.map(|span| span.start.0 as u32))
                    .unwrap_or_default(),
                original_loc.map(|(_, column)| column).unwrap_or_default(),
                source_id,
                name_id,
            );
        }
        let json = encoded.into_sourcemap().to_json();
        SourceMapArtifact {
            version: 3,
            file: self.file,
            sources: self.sources,
            names: self.names,
            mappings: json.mappings,
            sources_content: json.sources_content,
        }
    }

    fn extend_sources<I>(&mut self, sources: I)
    where
        I: IntoIterator<Item = String>,
    {
        for source in sources {
            if !self.sources.iter().any(|existing| existing == &source) {
                self.sources.push(source);
            }
        }
    }

    fn extend_names<I>(&mut self, names: I)
    where
        I: IntoIterator<Item = String>,
    {
        for name in names {
            if !self.names.iter().any(|existing| existing == &name) {
                self.names.push(name);
            }
        }
    }
}

impl SourceMapArtifact {
    /// Serializes this artifact to deterministic JSON.
    pub fn to_json_string(&self) -> serde_json::Result<String> {
        serde_json::to_string(self)
    }

    /// Serializes this artifact to deterministic pretty JSON.
    pub fn to_json_string_pretty(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }

    /// Parses this artifact through the Oxc source-map decoder.
    pub fn to_oxc_source_map(&self) -> Result<oxc_sourcemap::SourceMap, SourceMapError> {
        let json = self.to_json_string().map_err(|error| SourceMapError {
            message: error.to_string(),
        })?;
        oxc_sourcemap::SourceMap::from_json_string(&json).map_err(|error| SourceMapError {
            message: error.to_string(),
        })
    }

    /// Resolves a generated position through the encoded source map.
    pub fn original_position(
        &self,
        generated: GeneratedPosition,
    ) -> Result<Option<SourceMapOriginalPosition>, SourceMapError> {
        let source_map = self.to_oxc_source_map()?;
        let lookup = source_map.generate_lookup_table();
        Ok(source_map
            .lookup_token(&lookup, generated.line, generated.column)
            .and_then(|token| {
                let source_id = token.get_source_id()?;
                Some(SourceMapOriginalPosition {
                    source: source_map.get_source(source_id)?.to_string(),
                    line: token.get_src_line(),
                    column: token.get_src_col(),
                    name: token
                        .get_name_id()
                        .and_then(|name_id| source_map.get_name(name_id).map(ToString::to_string)),
                })
            }))
    }

    /// Builds a queryable trace by matching decoded source names to registered source files.
    pub fn trace_with_sources(
        &self,
        sources: &SourceMap,
    ) -> Result<SourceMapTrace, SourceMapError> {
        let source_map = self.to_oxc_source_map()?;
        let mut entries = Vec::new();
        for token in source_map.get_tokens() {
            let Some(source_id) = token.get_source_id() else {
                continue;
            };
            let Some(source_name) = source_map.get_source(source_id) else {
                continue;
            };
            let Some(file_id) = find_source_file_id(sources, source_name.as_ref()) else {
                continue;
            };
            let Some(pos) = sources.byte_pos_at(
                file_id,
                Loc {
                    line: token.get_src_line() as usize + 1,
                    column: token.get_src_col() as usize + 1,
                },
            ) else {
                continue;
            };
            entries.push(SourceMapEntry {
                generated: GeneratedPosition::new(token.get_dst_line(), token.get_dst_col()),
                original: Span {
                    file_id,
                    start: pos,
                    end: pos,
                },
            });
        }
        Ok(SourceMapTrace::new(entries))
    }

    /// Merges SFC block maps into one generated artifact with line offsets.
    pub fn merge_sfc_blocks(
        file: Option<String>,
        blocks: impl IntoIterator<Item = SfcSourceMapBlock>,
    ) -> Result<Self, SourceMapError> {
        let mut parsed_maps = Vec::new();
        for block in blocks {
            parsed_maps.push((
                block.artifact.to_oxc_source_map()?,
                block.generated_line_offset,
            ));
        }
        let refs = parsed_maps
            .iter()
            .map(|(source_map, offset)| (source_map, *offset))
            .collect::<Vec<_>>();
        let mut merged = oxc_sourcemap::ConcatSourceMapBuilder::from_sourcemaps(&refs)
            .into_sourcemap()
            .to_json();
        if file.is_some() {
            merged.file = file;
        }
        Ok(SourceMapArtifact {
            version: 3,
            file: merged.file,
            sources: merged.sources,
            names: merged.names,
            mappings: merged.mappings,
            sources_content: merged.sources_content,
        })
    }

    /// Builds a source map directly from already-normalized segments.
    pub fn from_segments(
        file: Option<String>,
        source: String,
        source_content: String,
        names: Vec<String>,
        segments: Vec<SourceMapSegment>,
    ) -> Self {
        let mut builder = oxc_sourcemap::SourceMapBuilder::default();
        if let Some(file) = file.as_deref() {
            builder.set_file(file);
        }
        let source_id = builder.set_source_and_content(&source, &source_content);
        let name_ids = names
            .iter()
            .map(|name| builder.add_name(name))
            .collect::<Vec<_>>();
        for segment in segments {
            let name_id = segment
                .name_index
                .and_then(|index| name_ids.get(index).copied());
            builder.add_token(
                segment.generated_line,
                segment.generated_column,
                segment.original_line,
                segment.original_column,
                Some(source_id),
                name_id,
            );
        }
        let json = builder.into_sourcemap().to_json();
        SourceMapArtifact {
            version: 3,
            file,
            sources: vec![source],
            names,
            mappings: json.mappings,
            sources_content: Some(vec![Some(source_content)]),
        }
    }
}

/// Original position resolved from a source map artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceMapOriginalPosition {
    /// Original source path.
    pub source: String,
    /// Zero-based original line.
    pub line: u32,
    /// Zero-based original column.
    pub column: u32,
    /// Optional original symbol name.
    pub name: Option<String>,
}

/// One SFC sub-map to merge into a generated artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcSourceMapBlock {
    /// Source map artifact for a generated block.
    pub artifact: SourceMapArtifact,
    /// Generated line offset for this block in the concatenated output.
    pub generated_line_offset: u32,
}

/// Normalized source-map segment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceMapSegment {
    /// Zero-based generated line.
    pub generated_line: u32,
    /// Zero-based generated column.
    pub generated_column: u32,
    /// Zero-based original line.
    pub original_line: u32,
    /// Zero-based original column.
    pub original_column: u32,
    /// Optional index into the source-map names table.
    pub name_index: Option<usize>,
}

/// Code emission result with an optional source map.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmitResult {
    /// Generated JavaScript source.
    pub code: String,
    /// Optional source map for the generated code.
    pub map: Option<SourceMapArtifact>,
}

impl EmitResult {
    /// Creates an emission result without a source map.
    pub fn new(code: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            map: None,
        }
    }

    /// Creates an emission result with a source map.
    pub fn with_map(code: impl Into<String>, map: SourceMapArtifact) -> Self {
        Self {
            code: code.into(),
            map: Some(map),
        }
    }

    /// Returns a deterministic snapshot of this emission result.
    pub fn snapshot(&self) -> EmitSnapshot {
        EmitSnapshot {
            code: self.code.clone(),
            map: self.map.clone(),
        }
    }

    /// Serializes the emission snapshot as compact JSON.
    pub fn snapshot_json(&self) -> serde_json::Result<String> {
        serde_json::to_string(&self.snapshot())
    }

    /// Serializes the emission snapshot as pretty JSON.
    pub fn snapshot_json_pretty(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(&self.snapshot())
    }
}

/// Snapshot-friendly code emission result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmitSnapshot {
    /// Generated JavaScript source.
    pub code: String,
    /// Optional source map for the generated code.
    pub map: Option<SourceMapArtifact>,
}

/// Shared MIR-first emitter contract.
pub trait MirEmitter {
    /// Emits a target-split MIR document.
    fn emit_mir(&self, mir: &Mir, options: &EmitOptions) -> EmitResult;
}

/// Runs a MIR emitter with options derived from the target-split MIR document.
pub fn emit_mir_with<E>(emitter: &E, mir: &Mir) -> EmitResult
where
    E: MirEmitter,
{
    emitter.emit_mir(mir, &EmitOptions::for_mir(mir))
}

fn loc_for_byte_offset(source: &str, offset: usize) -> Option<Loc> {
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
                if index + 1 < source.len() && bytes[index + 1] == b'\n' {
                    index += 2;
                    line += 1;
                    line_start = index;
                } else {
                    index += 1;
                    line += 1;
                    line_start = index;
                }
            }
            b'\n' => {
                index += 1;
                line += 1;
                line_start = index;
            }
            _ => {
                index += 1;
            }
        }
    }
    Some(Loc {
        line,
        column: source[line_start..offset].encode_utf16().count() + 1,
    })
}

fn find_source_file_id(sources: &SourceMap, source_name: &str) -> Option<vuec_source::FileId> {
    let source_name = std::path::Path::new(source_name);
    let mut index = 0u32;
    loop {
        let id = vuec_source::FileId(index);
        let Some(file) = sources.file(id) else {
            return None;
        };
        if file
            .path
            .as_ref()
            .is_some_and(|path| path.as_path() == source_name)
        {
            return Some(id);
        }
        index += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vuec_ast::{AstDocument, Vue3DomMirKind, Vue3DomRoot};
    use vuec_source::FileId;

    #[test]
    fn writer_handles_indent() {
        let mut writer = CodeWriter::new();
        writer.push_line("function test() {");
        writer.indent();
        writer.push_line("return 1;");
        writer.dedent();
        writer.push_str("}");
        let code = writer.finish();
        assert!(code.contains("  return 1;"));
    }

    #[test]
    fn exact_writer_preserves_whitespace_and_tracks_generated_position() {
        let mut writer = CodeWriter::exact();
        writer.indent();
        writer.push_str("a\n");
        writer.push_str("  b");
        assert_eq!(writer.as_str(), "a\n  b");
        assert_eq!(writer.generated_position(), GeneratedPosition::new(1, 3));
        assert_eq!(writer.len(), 5);
        assert!(!writer.is_empty());
    }

    #[test]
    fn runtime_helper_mapping_is_deterministic() {
        assert_eq!(
            runtime_helper_spec(RuntimeHelper::Vue3OpenBlock).name,
            "openBlock"
        );
        assert_eq!(
            vue3_helper_reference(RuntimeHelper::Vue3CreateElementVNode),
            "_createElementVNode"
        );
        assert_eq!(
            helper_aliases([
                RuntimeHelper::Vue3OpenBlock,
                RuntimeHelper::Vue3CreateElementBlock
            ]),
            "openBlock: _openBlock, createElementBlock: _createElementBlock"
        );
        assert_eq!(
            render_helper_imports([
                RuntimeHelper::Vue3OpenBlock,
                RuntimeHelper::Vue3SsrInterpolate
            ]),
            "import { openBlock as _openBlock } from \"vue\"\nimport { ssrInterpolate as _ssrInterpolate } from \"vue/server-renderer\""
        );
        assert_eq!(runtime_helper_alias(RuntimeHelper::Vue2ToString), "_s");
    }

    #[test]
    fn source_map_builder_serializes() {
        let mut builder = SourceMapBuilder::new().file("test.js");
        builder.add_name("foo");
        builder.add_source_content("src.vue", "abc\nmsg");
        builder.add_mapping(
            1,
            0,
            Some(Span::new(FileId(0), 0, 3)),
            Some("src.vue".into()),
        );
        let map = builder.build();
        let json = serde_json::to_string(&map).unwrap();
        assert!(json.contains("\"version\":3"));
        assert!(json.contains("src.vue"));
        assert_eq!(
            map.original_position(GeneratedPosition::new(0, 0))
                .unwrap()
                .unwrap()
                .source,
            "src.vue"
        );
    }

    #[test]
    fn source_map_trace_can_resolve_registered_original_position() {
        let mut sources = SourceMap::default();
        sources.add_file(Some("src.vue".into()), "abc\nmsg");
        let map = SourceMapArtifact::from_segments(
            Some("out.js".into()),
            "src.vue".into(),
            "abc\nmsg".into(),
            Vec::new(),
            vec![SourceMapSegment {
                generated_line: 0,
                generated_column: 0,
                original_line: 1,
                original_column: 0,
                name_index: None,
            }],
        );

        let trace = map.trace_with_sources(&sources).unwrap();
        let resolved = trace
            .original_position_at(&sources, GeneratedPosition::new(0, 0))
            .unwrap();
        assert_eq!(resolved.loc.line, 2);
        assert_eq!(resolved.loc.column, 1);
    }

    #[test]
    fn emit_result_snapshot_is_stable_json() {
        let result = EmitResult::new("return 1");
        assert_eq!(result.snapshot().code, "return 1");
        assert!(result.snapshot_json().unwrap().contains("return 1"));
    }

    struct TargetEchoEmitter;

    impl MirEmitter for TargetEchoEmitter {
        fn emit_mir(&self, _mir: &Mir, options: &EmitOptions) -> EmitResult {
            EmitResult::new(format!("{:?}", options.target))
        }
    }

    #[test]
    fn mir_first_emitter_derives_target_options() {
        let mir = Mir::Vue3Dom(AstDocument::new(
            Vue3DomMirKind::Root(Vue3DomRoot::default()),
            None,
        ));
        let result = emit_mir_with(&TargetEchoEmitter, &mir);
        assert_eq!(result.code, "Vue3Dom");
    }

    #[test]
    fn sfc_source_maps_merge_with_line_offsets() {
        let first = SourceMapArtifact::from_segments(
            Some("a.js".into()),
            "a.vue".into(),
            "a".into(),
            Vec::new(),
            vec![SourceMapSegment {
                generated_line: 0,
                generated_column: 0,
                original_line: 0,
                original_column: 0,
                name_index: None,
            }],
        );
        let second = SourceMapArtifact::from_segments(
            Some("b.js".into()),
            "b.vue".into(),
            "b".into(),
            Vec::new(),
            vec![SourceMapSegment {
                generated_line: 0,
                generated_column: 0,
                original_line: 0,
                original_column: 0,
                name_index: None,
            }],
        );

        let merged = SourceMapArtifact::merge_sfc_blocks(
            Some("bundle.js".into()),
            [
                SfcSourceMapBlock {
                    artifact: first,
                    generated_line_offset: 0,
                },
                SfcSourceMapBlock {
                    artifact: second,
                    generated_line_offset: 2,
                },
            ],
        )
        .unwrap();
        assert_eq!(merged.file.as_deref(), Some("bundle.js"));
        assert!(merged.sources.contains(&"a.vue".to_string()));
        assert!(merged.sources.contains(&"b.vue".to_string()));
    }
}
