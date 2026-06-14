#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Options controlling Vue 2 template parsing, optimization, and codegen.
pub struct Vue2CompileOptions {
    /// Enabled Vue 2 compiler module names.
    pub modules: Vec<String>,
    /// Enabled custom directive transform names.
    pub directives: Vec<String>,
    /// Whether compiler warnings should be reported.
    pub warn: bool,
    /// Whether warnings and errors should include byte ranges.
    pub output_source_range: bool,
    /// Whether comments should be preserved in the public AST and codegen.
    pub comments: bool,
    /// Custom interpolation delimiters.
    pub delimiters: Option<[String; 2]>,
    /// Whitespace handling mode.
    pub whitespace: Option<String>,
    /// Whether text whitespace should be preserved.
    pub preserve_whitespace: bool,
    /// Whether newlines are decoded in normal attributes.
    pub should_decode_newlines: bool,
    /// Whether newlines are decoded in href-like attributes.
    pub should_decode_newlines_for_href: bool,
    /// Whether static optimization should run.
    pub optimize: bool,
    /// Whether built-in must-use-prop behavior is disabled.
    pub disable_default_must_use_prop: bool,
    /// Per-tag namespace overrides.
    pub tag_namespaces: BTreeMap<String, String>,
    /// Whether default Vue 2 tag namespace rules are enabled.
    pub use_default_tag_namespaces: bool,
    /// Optional reserved-tag allow-list.
    pub reserved_tags: Option<Vec<String>>,
    /// Whether default Vue 2 reserved-tag rules are enabled.
    pub use_default_reserved_tags: bool,
    /// Script binding metadata used by SFC/template integration.
    pub bindings: BTreeMap<String, String>,
    /// Whether binding metadata came from script setup.
    pub bindings_is_script_setup: bool,
    /// Optional SFC asset URL transform configuration.
    pub sfc_asset_url_transform: Option<Vue2SfcAssetUrlTransformOptions>,
}

impl Default for Vue2CompileOptions {
    fn default() -> Self {
        Self {
            modules: Vec::new(),
            directives: Vec::new(),
            warn: true,
            output_source_range: false,
            comments: false,
            delimiters: None,
            whitespace: None,
            preserve_whitespace: true,
            should_decode_newlines: false,
            should_decode_newlines_for_href: false,
            optimize: true,
            disable_default_must_use_prop: false,
            tag_namespaces: BTreeMap::new(),
            use_default_tag_namespaces: true,
            reserved_tags: None,
            use_default_reserved_tags: true,
            bindings: BTreeMap::new(),
            bindings_is_script_setup: true,
            sfc_asset_url_transform: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Vue 2.7 SFC template asset URL transform options.
pub struct Vue2SfcAssetUrlTransformOptions {
    /// Optional base path to prefix transformed relative URLs.
    pub base: Option<String>,
    /// Whether absolute URLs should also be transformed.
    pub include_absolute: bool,
    /// Tag-to-attribute map that identifies URL-bearing attributes.
    pub tags: BTreeMap<String, Vec<String>>,
}

impl Default for Vue2SfcAssetUrlTransformOptions {
    fn default() -> Self {
        Self {
            base: None,
            include_absolute: false,
            tags: vue2_sfc_default_asset_url_tags(),
        }
    }
}

/// Returns the default Vue 2.7 SFC asset URL tag and attribute map.
pub fn vue2_sfc_default_asset_url_tags() -> BTreeMap<String, Vec<String>> {
    [
        ("audio", vec!["src"]),
        ("video", vec!["src", "poster"]),
        ("source", vec!["src"]),
        ("img", vec!["src"]),
        ("image", vec!["xlink:href", "href"]),
        ("use", vec!["xlink:href", "href"]),
    ]
    .into_iter()
    .map(|(tag, attrs)| {
        (
            tag.to_string(),
            attrs.into_iter().map(str::to_string).collect(),
        )
    })
    .collect()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Vue 2 compiler warning or tip.
pub struct Vue2Warning {
    /// Warning message text.
    pub msg: String,
    /// Optional start byte offset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<usize>,
    /// Optional end byte offset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<usize>,
    /// Whether this warning is a Vue 2 tip.
    pub tip: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Vue 2 compiler error.
pub struct Vue2Error {
    /// Error message text.
    pub msg: String,
    /// Optional start byte offset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<usize>,
    /// Optional end byte offset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Full Vue 2 compile result.
pub struct Vue2CompiledResult {
    /// Canonical arena-backed public AST projection.
    pub ast: Vue2Ast,
    /// Compatibility element tree used by Vue 2 codegen projections.
    pub element_ast: Option<Vue2Element>,
    /// Generated Vue 2 render function body.
    pub render: String,
    /// Generated static render function bodies.
    pub static_render_fns: Vec<String>,
    /// Compile errors in official-style public shape.
    pub errors: Vec<Vue2Error>,
    /// Compile tips and warnings in official-style public shape.
    pub tips: Vec<Vue2Warning>,
    /// Rendered diagnostic messages.
    pub diagnostics: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Vue 2 `compileToFunctions`-style result.
pub struct Vue2FunctionResult {
    /// Generated Vue 2 render function body.
    pub render: String,
    /// Generated static render function bodies.
    pub static_render_fns: Vec<String>,
    /// Public warning and tip list.
    pub warnings: Vec<Vue2Warning>,
    /// Rendered error strings.
    pub errors: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Vue 2 codegen-only result.
pub struct Vue2CodegenResult {
    /// Generated Vue 2 render function body.
    pub render: String,
    /// Generated static render function bodies.
    pub static_render_fns: Vec<String>,
}

/// Result of projecting the Vue 2 compatibility parser tree into canonical AST.
pub struct Vue2AstProjectionResult {
    /// Canonical arena-backed Vue 2 AST.
    pub ast: Vue2Ast,
    /// JavaScript side store referenced by AST expression, statement, and pattern ids.
    pub js: JsAstStore,
}

/// Result of the structural Vue 2 lowering contract.
///
/// This is separate from the current exact render emitter so the AST -> HIR ->
/// Vue2Mir boundary can be verified without changing public codegen parity.
pub struct Vue2LoweringResult {
    /// Lowered shared HIR document.
    pub hir: Hir,
    /// Lowered Vue 2 target MIR document.
    pub mir: Vue2Mir,
    /// AST-to-HIR and HIR-to-MIR edge map.
    pub map: LoweringMap,
    /// JavaScript side store used by AST/HIR/MIR ids.
    pub js: JsAstStore,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Vue 2 parsed attribute.
pub struct Vue2Attribute {
    /// Attribute name.
    pub name: String,
    /// Attribute value.
    pub value: String,
    /// Source span for the attribute.
    pub span: Option<Span>,
    /// Whether the attribute name or value is dynamic.
    pub dynamic: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Vue 2 parsed directive.
pub struct Vue2Directive {
    /// Normalized directive name.
    pub name: String,
    /// Raw source directive name.
    pub raw_name: String,
    /// Optional directive expression.
    pub value: Option<String>,
    /// Optional directive argument.
    pub arg: Option<String>,
    /// Whether the directive argument is dynamic.
    pub is_dynamic_arg: bool,
    /// Directive modifiers keyed by modifier name.
    pub modifiers: BTreeMap<String, bool>,
    /// Source span for the directive.
    pub span: Option<Span>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Vue 2 event handler metadata.
pub struct Vue2EventHandler {
    /// Handler expression.
    pub value: String,
    /// Event modifiers keyed by modifier name.
    pub modifiers: BTreeMap<String, bool>,
    /// Original modifier order.
    #[serde(default)]
    pub modifier_order: Vec<String>,
    /// Whether object-style modifier syntax was present.
    #[serde(default)]
    pub has_modifier_object: bool,
    /// Whether the event name is dynamic.
    pub dynamic: bool,
    /// Source span for the event directive.
    pub span: Option<Span>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// One Vue 2 `v-if` / `v-else-if` / `v-else` branch.
pub struct Vue2IfCondition {
    /// Optional branch expression.
    pub exp: Option<String>,
    /// Branch root element.
    pub block: Box<Vue2Element>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Node in the Vue 2 compatibility element tree.
pub enum Vue2Node {
    /// Element child node.
    Element(Box<Vue2Element>),
    /// Text, interpolation, or comment child node.
    Text(Vue2Text),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Vue 2 text, interpolation, or comment node.
pub struct Vue2Text {
    /// Raw or generated text content.
    pub text: String,
    /// Optional interpolation expression.
    pub expression: Option<String>,
    /// Whether this text node is a comment.
    pub is_comment: bool,
    /// Source span for this text node.
    pub span: Option<Span>,
    /// Static analysis marker.
    pub static_node: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Vue 2 element node used by parsing, optimization, and codegen.
pub struct Vue2Element {
    /// Element tag name.
    pub tag: String,
    /// Processed attribute list.
    pub attrs_list: Vec<Vue2Attribute>,
    /// Raw attribute list before directive/module processing.
    #[serde(default)]
    pub raw_attrs_list: Vec<Vue2Attribute>,
    /// Processed attribute map.
    pub attrs_map: BTreeMap<String, String>,
    /// Raw attribute map preserving attribute metadata.
    pub raw_attrs_map: BTreeMap<String, Vue2Attribute>,
    /// Runtime attrs emitted to `data.attrs`.
    pub attrs: Vec<Vue2Attribute>,
    /// Runtime props emitted to `data.domProps` or component props.
    pub props: Vec<Vue2Attribute>,
    /// Dynamic attributes that affect runtime patching.
    pub dynamic_attrs: Vec<Vue2Attribute>,
    /// Custom and built-in directives attached to the element.
    pub directives: Vec<Vue2Directive>,
    /// Component or DOM event listeners.
    pub events: BTreeMap<String, Vec<Vue2EventHandler>>,
    /// Native event listeners for component nodes.
    pub native_events: BTreeMap<String, Vec<Vue2EventHandler>>,
    /// Child nodes.
    pub children: Vec<Vue2Node>,
    /// Source span for the element.
    pub span: Option<Span>,
    /// Optional namespace such as SVG or MathML.
    pub ns: Option<String>,
    /// Whether the element has no data bindings or children requiring data.
    pub plain: bool,
    /// Whether the element is forbidden in the current context.
    pub forbidden: bool,
    /// Whether `v-pre` applies to this element.
    pub pre: bool,
    /// Whether `v-once` applies to this element.
    pub once: bool,
    /// Whether this element has runtime bindings.
    pub has_bindings: bool,
    /// `v-if` expression.
    pub if_exp: Option<String>,
    /// Source span for the `v-if` directive.
    #[serde(default)]
    pub if_span: Option<Span>,
    /// `v-else-if` expression.
    pub elseif: Option<String>,
    /// Source span for the `v-else-if` directive.
    #[serde(default)]
    pub elseif_span: Option<Span>,
    /// Whether this branch is `v-else`.
    pub else_branch: bool,
    /// Source span for the `v-else` directive.
    #[serde(default)]
    pub else_span: Option<Span>,
    /// Ordered conditional branches.
    pub if_conditions: Vec<Vue2IfCondition>,
    /// `v-for` source expression.
    pub for_exp: Option<String>,
    /// Source span for the `v-for` directive.
    #[serde(default)]
    pub for_span: Option<Span>,
    /// Primary `v-for` alias.
    pub alias: Option<String>,
    /// First `v-for` iterator alias.
    pub iterator1: Option<String>,
    /// Second `v-for` iterator alias.
    pub iterator2: Option<String>,
    /// Key expression.
    pub key: Option<String>,
    /// Source span for the key binding.
    #[serde(default)]
    pub key_span: Option<Span>,
    /// Ref expression.
    pub ref_name: Option<String>,
    /// Whether the ref appears inside a `v-for`.
    pub ref_in_for: bool,
    /// Legacy slot name.
    pub slot_name: Option<String>,
    /// Slot target expression.
    pub slot_target: Option<String>,
    /// Whether the slot target is dynamic.
    pub slot_target_dynamic: bool,
    /// Scoped slot expression.
    pub slot_scope: Option<String>,
    /// Whether this uses the new `v-slot` syntax.
    #[serde(default)]
    pub slot_new_syntax: bool,
    /// Scoped slots keyed by slot name.
    pub scoped_slots: BTreeMap<String, Vue2Element>,
    /// Dynamic component expression.
    pub component: Option<String>,
    /// Whether this component uses `inline-template`.
    pub inline_template: bool,
    /// Static class expression.
    pub static_class: Option<String>,
    /// Dynamic class expression.
    pub class_binding: Option<String>,
    /// Static style expression.
    pub static_style: Option<String>,
    /// Dynamic style expression.
    pub style_binding: Option<String>,
    /// Component `v-model` metadata.
    pub model: Option<Vue2ComponentModel>,
    /// Data wrapper produced by custom module transforms.
    pub wrap_data: Option<Vue2DataWrap>,
    /// Listener wrapper expression.
    pub wrap_listeners: Option<String>,
    /// Validation metadata for legacy validation directives.
    pub validate: Option<Vue2Validation>,
    /// Validation rules attached to the element.
    pub validators: Vec<Vue2Validator>,
    /// Whether this element is static.
    pub static_node: bool,
    /// Whether this element is a static root.
    pub static_root: bool,
    /// Whether this static node appears inside `v-for`.
    pub static_in_for: bool,
    #[serde(default)]
    static_processed: bool,
    #[serde(default)]
    once_processed: bool,
    #[serde(default)]
    for_processed: bool,
    #[serde(default)]
    if_processed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Vue 2 component `v-model` codegen metadata.
pub struct Vue2ComponentModel {
    /// Runtime model value expression.
    pub value: String,
    /// Runtime update callback expression.
    pub callback: String,
    /// Original model expression.
    pub expression: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Vue 2 data wrapper emitted by module transforms.
pub enum Vue2DataWrap {
    /// Wraps generated data with `_b(...)` semantics.
    Bind {
        /// Bound object expression.
        value: String,
        /// Whether `.prop` handling applies.
        prop: bool,
        /// Whether `.sync` handling applies.
        sync: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Vue 2 validation directive metadata.
pub struct Vue2Validation {
    /// Field expression being validated.
    pub field: String,
    /// Validation groups.
    pub groups: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Vue 2 validation rule metadata.
pub struct Vue2Validator {
    /// Validator name.
    pub name: String,
    /// Validator rule expression.
    pub rule: String,
}
