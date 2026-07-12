/// Concrete syntax tree document.
pub type Cst = AstDocument<CstNodeKind>;

/// CST node kinds preserving raw source structure.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CstNodeKind {
    /// Top-level CST document node.
    Document,
    /// SFC block such as `template`, `script`, or `style`.
    SfcBlock(CstSfcBlock),
    /// Raw element node.
    Element(CstElement),
    /// Raw attribute node.
    Attribute(CstAttribute),
    /// Raw text node.
    Text {
        /// Raw text content.
        raw: String,
    },
    /// Raw comment node.
    Comment {
        /// Raw comment content.
        raw: String,
    },
    /// Raw CDATA node.
    Cdata {
        /// Raw CDATA content.
        raw: String,
    },
    /// Raw doctype node.
    Doctype {
        /// Raw doctype content.
        raw: String,
    },
    /// Raw interpolation node.
    Interpolation(CstInterpolation),
}

/// CST SFC block payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CstSfcBlock {
    /// Block type, for example `template`, `script`, or `style`.
    pub block_type: String,
    /// Raw tag text.
    pub raw_tag: String,
}

/// CST element payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CstElement {
    /// Raw tag name.
    pub raw_tag: String,
    /// Whether the element used self-closing syntax.
    pub self_closing: bool,
    /// Source span of the opening tag.
    pub open_span: Span,
    /// Source span of the closing tag, if present.
    pub close_span: Option<Span>,
}

/// CST attribute payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CstAttribute {
    /// Raw attribute name.
    pub raw_name: String,
    /// Raw attribute value, if present.
    pub raw_value: Option<String>,
    /// Quote kind used by the attribute value.
    pub quote: Option<QuoteKind>,
    /// Source span of the attribute name.
    pub name_span: Span,
    /// Source span of the attribute value.
    pub value_span: Option<Span>,
}

/// Attribute quote kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuoteKind {
    /// Double-quoted attribute value.
    Double,
    /// Single-quoted attribute value.
    Single,
    /// Unquoted attribute value.
    Unquoted,
}

/// CST interpolation payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CstInterpolation {
    /// Raw interpolation text.
    pub raw: String,
    /// Source span of the opening delimiter.
    pub open_span: Span,
    /// Source span of the inner expression.
    pub inner_span: Span,
    /// Source span of the closing delimiter.
    pub close_span: Span,
}

/// Compatibility template attribute used by parser-facing constructors.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateAttribute {
    /// Attribute or directive name.
    pub name: String,
    /// Attribute or directive value.
    pub value: Option<String>,
}

/// Vue 2 AST node kinds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vue2AstKind {
    /// Vue 2 root node.
    Root(Vue2Root),
    /// Vue 2 element node.
    Element(Box<Vue2Element>),
    /// Plain text node.
    Text(Vue2Text),
    /// Interpolated text node.
    ExpressionText(Vue2ExpressionText),
    /// Comment node.
    Comment(Vue2Comment),
}

impl Vue2AstKind {
    /// Creates a Vue 2 root node kind.
    pub fn root() -> Self {
        Self::Root(Vue2Root::default())
    }

    /// Creates a Vue 2 element node kind.
    pub fn element(tag: impl Into<String>) -> Self {
        Self::Element(Box::new(Vue2Element::new(tag)))
    }

    /// Creates a Vue 2 text node kind.
    pub fn text(value: impl Into<String>) -> Self {
        Self::Text(Vue2Text {
            value: value.into(),
            static_node: false,
        })
    }

    /// Creates a Vue 2 expression text node kind.
    pub fn expression_text(raw: impl Into<String>) -> Self {
        Self::ExpressionText(Vue2ExpressionText {
            raw: raw.into(),
            expr: None,
            filter_expr: None,
        })
    }

    /// Creates a Vue 2 comment node kind.
    pub fn comment(value: impl Into<String>) -> Self {
        Self::Comment(Vue2Comment {
            value: value.into(),
        })
    }
}

/// Vue 2 root node payload.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2Root {
    /// Source file id for this template, when known.
    pub source_id: Option<FileId>,
}

/// Vue 2 element node payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2Element {
    /// Element tag name.
    pub tag: String,
    /// Attributes in source order.
    pub attrs_list: Vec<Vue2Attribute>,
    /// Attribute name to value map.
    pub attrs_map: BTreeMap<String, String>,
    /// Raw attribute map preserving attribute payloads.
    pub raw_attrs_map: BTreeMap<String, Vue2Attribute>,
    /// Static attributes used for data object generation.
    pub attrs: Vec<Vue2Attribute>,
    /// DOM props used for data object generation.
    pub props: Vec<Vue2Attribute>,
    /// Dynamic attribute bindings.
    pub dynamic_attrs: Vec<Vue2Attribute>,
    /// Custom and built-in directive records.
    pub directives: Vec<Vue2Directive>,
    /// Component or DOM event handlers.
    pub events: BTreeMap<String, Vec<Vue2EventHandler>>,
    /// Native event handlers on component nodes.
    pub native_events: BTreeMap<String, Vec<Vue2EventHandler>>,
    /// Element namespace.
    pub ns: Option<String>,
    /// Whether the element has no generated data object.
    pub plain: bool,
    /// Whether the element is forbidden by parser rules.
    pub forbidden: bool,
    /// Whether `v-pre` applies.
    pub pre: bool,
    /// Whether `v-once` applies.
    pub once: bool,
    /// Whether the element has dynamic bindings.
    pub has_bindings: bool,
    /// `v-if` expression id.
    pub if_exp: Option<JsExprId>,
    /// `v-if` source range.
    pub if_span: Option<Span>,
    /// `v-else-if` expression id.
    pub elseif: Option<JsExprId>,
    /// `v-else-if` source range.
    pub elseif_span: Option<Span>,
    /// Whether this is a `v-else` branch.
    pub else_branch: bool,
    /// `v-else` source range.
    pub else_span: Option<Span>,
    /// Linked Vue 2 if conditions.
    pub if_conditions: Vec<Vue2IfCondition>,
    /// `v-for` source expression id.
    pub for_exp: Option<JsExprId>,
    /// `v-for` source range.
    pub for_span: Option<Span>,
    /// `v-for` value alias pattern id.
    pub alias: Option<JsPatternId>,
    /// `v-for` first iterator alias pattern id.
    pub iterator1: Option<JsPatternId>,
    /// `v-for` second iterator alias pattern id.
    pub iterator2: Option<JsPatternId>,
    /// Key binding expression id.
    pub key: Option<JsExprId>,
    /// Key binding source range.
    pub key_span: Option<Span>,
    /// Template ref name.
    pub ref_name: Option<String>,
    /// Whether the ref appears inside `v-for`.
    pub ref_in_for: bool,
    /// Legacy slot name.
    pub slot_name: Option<String>,
    /// Slot target name.
    pub slot_target: Option<String>,
    /// Whether the slot target is dynamic.
    pub slot_target_dynamic: bool,
    /// Slot scope pattern id.
    pub slot_scope: Option<JsPatternId>,
    /// Whether this element used Vue 2.6+ `v-slot` / `#` syntax.
    pub slot_new_syntax: bool,
    /// Scoped slot entries keyed by slot name.
    pub scoped_slots: BTreeMap<String, NodeId>,
    /// Component expression or tag target.
    pub component: Option<String>,
    /// Whether the element carries inline-template content.
    pub inline_template: bool,
    /// Static class attribute value.
    pub static_class: Option<String>,
    /// Dynamic class binding expression id.
    pub class_binding: Option<JsExprId>,
    /// Static style attribute value.
    pub static_style: Option<String>,
    /// Dynamic style binding expression id.
    pub style_binding: Option<JsExprId>,
    /// Component model metadata.
    pub model: Option<Vue2ComponentModel>,
    /// Data object wrapping metadata.
    pub wrap_data: Option<Vue2DataWrap>,
    /// Listener wrapping expression.
    pub wrap_listeners: Option<String>,
    /// Validation directive metadata.
    pub validate: Option<Vue2Validation>,
    /// Validation rules attached to the element.
    pub validators: Vec<Vue2Validator>,
    /// Whether the node is static.
    pub static_node: bool,
    /// Whether the node is a static root.
    pub static_root: bool,
    /// Whether a static node appears inside `v-for`.
    pub static_in_for: bool,
}

impl Vue2Element {
    /// Creates a Vue 2 element payload with default metadata.
    pub fn new(tag: impl Into<String>) -> Self {
        Self {
            tag: tag.into(),
            attrs_list: Vec::new(),
            attrs_map: BTreeMap::new(),
            raw_attrs_map: BTreeMap::new(),
            attrs: Vec::new(),
            props: Vec::new(),
            dynamic_attrs: Vec::new(),
            directives: Vec::new(),
            events: BTreeMap::new(),
            native_events: BTreeMap::new(),
            ns: None,
            plain: false,
            forbidden: false,
            pre: false,
            once: false,
            has_bindings: false,
            if_exp: None,
            if_span: None,
            elseif: None,
            elseif_span: None,
            else_branch: false,
            else_span: None,
            if_conditions: Vec::new(),
            for_exp: None,
            for_span: None,
            alias: None,
            iterator1: None,
            iterator2: None,
            key: None,
            key_span: None,
            ref_name: None,
            ref_in_for: false,
            slot_name: None,
            slot_target: None,
            slot_target_dynamic: false,
            slot_scope: None,
            slot_new_syntax: false,
            scoped_slots: BTreeMap::new(),
            component: None,
            inline_template: false,
            static_class: None,
            class_binding: None,
            static_style: None,
            style_binding: None,
            model: None,
            wrap_data: None,
            wrap_listeners: None,
            validate: None,
            validators: Vec::new(),
            static_node: false,
            static_root: false,
            static_in_for: false,
        }
    }
}

/// Vue 2 text node payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2Text {
    /// Text content.
    pub value: String,
    /// Whether the text is static.
    pub static_node: bool,
}

/// Vue 2 interpolation text payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2ExpressionText {
    /// Raw interpolation source.
    pub raw: String,
    /// Parsed expression id.
    pub expr: Option<JsExprId>,
    /// Filter-aware expression payload.
    pub filter_expr: Option<Vue2FilterExpr>,
}

/// Vue 2 comment payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2Comment {
    /// Comment text.
    pub value: String,
}

/// Vue 2 attribute payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2Attribute {
    /// Attribute name.
    pub name: String,
    /// Attribute value.
    pub value: String,
    /// Source span for this attribute.
    pub span: Option<Span>,
    /// Whether the name or value is dynamic.
    pub dynamic: bool,
}

/// Vue 2 directive payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2Directive {
    /// Normalized directive name.
    pub name: String,
    /// Raw directive name.
    pub raw_name: String,
    /// Optional directive expression id.
    pub value: Option<JsExprId>,
    /// Optional directive argument.
    pub arg: Option<String>,
    /// Whether the argument is dynamic.
    pub is_dynamic_arg: bool,
    /// Directive modifiers.
    pub modifiers: BTreeMap<String, bool>,
}

/// Vue 2 event handler payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2EventHandler {
    /// Handler statement id.
    pub value: JsStmtId,
    /// Event modifiers.
    pub modifiers: BTreeMap<String, bool>,
    /// Original source modifier order.
    pub modifier_order: Vec<String>,
    /// Whether object-style modifier syntax was present.
    pub has_modifier_object: bool,
    /// Whether the event name is dynamic.
    pub dynamic: bool,
    /// Source span for this handler.
    pub span: Option<Span>,
}

/// Vue 2 `v-if` branch condition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2IfCondition {
    /// Optional branch condition expression id.
    pub exp: Option<JsExprId>,
    /// Branch root node id.
    pub block: NodeId,
    /// Condition source span.
    pub span: Option<Span>,
}

/// Vue 2 component `v-model` metadata.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2ComponentModel {
    /// Model value expression id.
    pub value: JsExprId,
    /// Model assignment callback statement id.
    pub callback: JsStmtId,
    /// Raw expression string.
    pub expression: String,
}

/// Vue 2 data-object wrapping metadata.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vue2DataWrap {
    /// Wrapper produced for object `v-bind`.
    Bind {
        /// Bound object expression id.
        value: JsExprId,
        /// Whether `.prop` is present.
        prop: bool,
        /// Whether `.sync` is present.
        sync: bool,
    },
}

/// Vue 2 validation directive metadata.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2Validation {
    /// Field expression being validated.
    pub field: String,
    /// Validation groups.
    pub groups: Vec<String>,
}

/// Vue 2 validation rule metadata.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2Validator {
    /// Validator name.
    pub name: String,
    /// Validator rule expression.
    pub rule: String,
}

/// Vue 2 filter expression payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2FilterExpr {
    /// Raw filter expression.
    pub raw: String,
    /// Base expression before filters are applied.
    pub base: JsExprId,
    /// Ordered filter calls.
    pub filters: Vec<Vue2FilterCall>,
}

/// Vue 2 filter call payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2FilterCall {
    /// Filter name.
    pub name: String,
    /// Filter argument expression ids.
    pub args: Vec<JsExprId>,
}

impl SpanMetadata for CstNodeKind {
    fn collect_extra_spans(&self, spans: &mut Vec<ExtraSpan>) {
        match self {
            Self::SfcBlock(_)
            | Self::Document
            | Self::Text { .. }
            | Self::Comment { .. }
            | Self::Cdata { .. }
            | Self::Doctype { .. } => {}
            Self::Element(element) => element.collect_extra_spans(spans),
            Self::Attribute(attribute) => attribute.collect_extra_spans(spans),
            Self::Interpolation(interpolation) => interpolation.collect_extra_spans(spans),
        }
    }
}

impl SpanMetadata for CstElement {
    fn collect_extra_spans(&self, spans: &mut Vec<ExtraSpan>) {
        spans.push(ExtraSpan::new("cst.open_span", self.open_span));
        if let Some(span) = self.close_span {
            spans.push(ExtraSpan::new("cst.close_span", span));
        }
    }
}

impl SpanMetadata for CstAttribute {
    fn collect_extra_spans(&self, spans: &mut Vec<ExtraSpan>) {
        spans.push(ExtraSpan::new("cst.name_span", self.name_span));
        if let Some(span) = self.value_span {
            spans.push(ExtraSpan::new("cst.value_span", span));
        }
    }
}

impl SpanMetadata for CstInterpolation {
    fn collect_extra_spans(&self, spans: &mut Vec<ExtraSpan>) {
        spans.push(ExtraSpan::new("cst.open_span", self.open_span));
        spans.push(ExtraSpan::new("cst.inner_span", self.inner_span));
        spans.push(ExtraSpan::new("cst.close_span", self.close_span));
    }
}

impl SpanMetadata for Vue2AstKind {
    fn collect_extra_spans(&self, spans: &mut Vec<ExtraSpan>) {
        if let Self::Element(element) = self {
            element.collect_extra_spans(spans);
        }
    }
}

impl SpanMetadata for Vue2Element {
    fn collect_extra_spans(&self, spans: &mut Vec<ExtraSpan>) {
        push_optional_span(spans, "vue2.if_span", self.if_span);
        push_optional_span(spans, "vue2.elseif_span", self.elseif_span);
        push_optional_span(spans, "vue2.else_span", self.else_span);
        push_optional_span(spans, "vue2.for_span", self.for_span);
        push_optional_span(spans, "vue2.key_span", self.key_span);
        for (index, attr) in self.attrs_list.iter().enumerate() {
            push_optional_span(spans, format!("vue2.attrs_list[{index}].span"), attr.span);
        }
        for (index, attr) in self.attrs.iter().enumerate() {
            push_optional_span(spans, format!("vue2.attrs[{index}].span"), attr.span);
        }
        for (index, attr) in self.props.iter().enumerate() {
            push_optional_span(spans, format!("vue2.props[{index}].span"), attr.span);
        }
        for (index, attr) in self.dynamic_attrs.iter().enumerate() {
            push_optional_span(
                spans,
                format!("vue2.dynamic_attrs[{index}].span"),
                attr.span,
            );
        }
        for (event, handlers) in &self.events {
            for (index, handler) in handlers.iter().enumerate() {
                push_optional_span(
                    spans,
                    format!("vue2.events[{event}][{index}].span"),
                    handler.span,
                );
            }
        }
        for (event, handlers) in &self.native_events {
            for (index, handler) in handlers.iter().enumerate() {
                push_optional_span(
                    spans,
                    format!("vue2.native_events[{event}][{index}].span"),
                    handler.span,
                );
            }
        }
        for (index, condition) in self.if_conditions.iter().enumerate() {
            push_optional_span(
                spans,
                format!("vue2.if_conditions[{index}].span"),
                condition.span,
            );
        }
    }
}

impl SpanMetadata for Vue3AstKind {
    fn collect_extra_spans(&self, spans: &mut Vec<ExtraSpan>) {
        if let Self::Element(element) = self {
            element.collect_extra_spans(spans);
        }
    }
}

impl SpanMetadata for Vue3Element {
    fn collect_extra_spans(&self, spans: &mut Vec<ExtraSpan>) {
        for (index, prop) in self.props.iter().enumerate() {
            prop.collect_extra_spans_with_prefix(spans, &format!("vue3.props[{index}]"));
        }
    }
}

impl Vue3Prop {
    fn collect_extra_spans_with_prefix(&self, spans: &mut Vec<ExtraSpan>, prefix: &str) {
        match self {
            Self::Attribute(attribute) => {
                push_optional_span(spans, format!("{prefix}.span"), attribute.span);
                push_optional_span(spans, format!("{prefix}.name_span"), attribute.name_span);
                push_optional_span(spans, format!("{prefix}.value_span"), attribute.value_span);
            }
            Self::Directive(directive) => {
                push_optional_span(spans, format!("{prefix}.span"), directive.span);
                push_optional_span(spans, format!("{prefix}.arg_span"), directive.arg_span);
                push_optional_span(spans, format!("{prefix}.exp_span"), directive.exp_span);
                for (index, span) in directive.modifier_spans.iter().enumerate() {
                    spans.push(ExtraSpan::new(
                        format!("{prefix}.modifier_spans[{index}]"),
                        span.clone(),
                    ));
                }
            }
        }
    }
}

impl SpanMetadata for HirNodeKind {}
impl SpanMetadata for Vue2MirKind {}
impl SpanMetadata for Vue3DomMirKind {}
impl SpanMetadata for Vue3SsrMirKind {}
impl SpanMetadata for VaporMirKind {}

fn push_optional_span(spans: &mut Vec<ExtraSpan>, owner: impl Into<String>, span: Option<Span>) {
    if let Some(span) = span {
        spans.push(ExtraSpan::new(owner, span));
    }
}
