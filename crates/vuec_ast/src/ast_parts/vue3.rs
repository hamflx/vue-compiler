/// Vue 3 AST node kinds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vue3AstKind {
    /// Vue 3 root node.
    Root(Vue3Root),
    /// Vue 3 element node.
    Element(Vue3Element),
    /// Plain text node.
    Text(Vue3Text),
    /// Comment node.
    Comment(Vue3Comment),
    /// Interpolation node.
    Interpolation(Vue3Interpolation),
    /// Compound expression node.
    CompoundExpression(Vue3CompoundExpression),
    /// `v-if` node.
    If(Vue3If),
    /// `v-if` branch node.
    IfBranch(Vue3IfBranch),
    /// `v-for` node.
    For(Vue3For),
    /// Text-call wrapper node.
    TextCall(Vue3TextCall),
}

impl Vue3AstKind {
    /// Creates a Vue 3 root node kind.
    pub fn root() -> Self {
        Self::Root(Vue3Root::default())
    }

    /// Creates a Vue 3 element node kind from compatibility attributes.
    pub fn element(
        tag: impl Into<String>,
        attributes: Vec<TemplateAttribute>,
        self_closing: bool,
    ) -> Self {
        Self::Element(Vue3Element {
            tag: tag.into(),
            tag_type: Vue3ElementType::Element,
            ns: HtmlNamespace::Html,
            props: attributes
                .into_iter()
                .map(Vue3Prop::compat_attribute)
                .collect(),
            self_closing,
            codegen_node: None,
            ssr_codegen_node: None,
        })
    }

    /// Creates a Vue 3 text node kind.
    pub fn text(value: impl Into<String>) -> Self {
        Self::Text(Vue3Text {
            value: value.into(),
        })
    }

    /// Creates a Vue 3 interpolation node kind.
    pub fn interpolation(expression: impl Into<String>) -> Self {
        Self::Interpolation(Vue3Interpolation {
            expression: Vue3Expression::Raw(expression.into()),
        })
    }

    /// Creates a Vue 3 comment node kind.
    pub fn comment(value: impl Into<String>) -> Self {
        Self::Comment(Vue3Comment {
            value: value.into(),
        })
    }

    /// Creates a compatibility directive node on a synthetic template element.
    pub fn directive(name: impl Into<String>, expression: Option<String>) -> Self {
        let name = name.into();
        Self::Element(Vue3Element {
            tag: "template".into(),
            tag_type: Vue3ElementType::Template,
            ns: HtmlNamespace::Html,
            props: vec![Vue3Prop::Directive(Vue3Directive {
                name: name.clone(),
                raw_name: format!("v-{name}"),
                arg: None,
                exp: expression.map(Vue3Expression::Raw),
                modifiers: Vec::new(),
                is_dynamic_arg: false,
                span: None,
                arg_span: None,
                exp_span: None,
                modifier_spans: Vec::new(),
            })],
            self_closing: true,
            codegen_node: None,
            ssr_codegen_node: None,
        })
    }
}

/// Vue 3 root node payload.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3Root {
    /// Source file id for this template, when known.
    pub source_id: Option<FileId>,
    /// Parser diagnostics collected while recovering the Vue 3 template tree.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parser_diagnostics: Vec<Vue3ParserDiagnostic>,
    /// Runtime helpers requested by transforms.
    pub helpers: BTreeSet<RuntimeHelper>,
    /// Component asset names referenced by the template.
    pub components: BTreeSet<String>,
    /// Directive asset names referenced by the template.
    pub directives: BTreeSet<String>,
    /// Import items generated from template assets.
    pub imports: Vec<Vue3ImportItem>,
    /// Hoisted node references.
    pub hoists: Vec<Vue3HoistSlot>,
    /// Temporary variable count.
    pub temps: u32,
    /// Cached expression count.
    pub cached: u32,
    /// Root codegen node reference.
    pub codegen_node: Option<Vue3CodegenRef>,
}

/// Lightweight Vue 3 parser diagnostic stored on the AST root.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3ParserDiagnostic {
    /// Stable Vue 3 compiler error code.
    pub code: u16,
    /// Public compiler error message.
    pub message: String,
    /// Optional source span for the parser recovery point.
    pub span: Option<Span>,
}

/// Vue 3 element node payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3Element {
    /// Element tag source.
    pub tag: String,
    /// Element category.
    pub tag_type: Vue3ElementType,
    /// Element namespace.
    pub ns: HtmlNamespace,
    /// Element props and directives.
    pub props: Vec<Vue3Prop>,
    /// Whether the element is self closing.
    pub self_closing: bool,
    /// DOM codegen node reference.
    pub codegen_node: Option<Vue3CodegenRef>,
    /// SSR codegen node reference.
    pub ssr_codegen_node: Option<Vue3SsrCodegenRef>,
}

impl Vue3Element {
    /// Projects props back to compatibility template attributes.
    pub fn template_attributes(&self) -> Vec<TemplateAttribute> {
        self.props
            .iter()
            .map(|prop| match prop {
                Vue3Prop::Attribute(attr) => TemplateAttribute {
                    name: attr.name.clone(),
                    value: attr.value.clone(),
                },
                Vue3Prop::Directive(directive) => TemplateAttribute {
                    name: directive.raw_name.clone(),
                    value: directive.exp.as_ref().map(Vue3Expression::source_string),
                },
            })
            .collect()
    }
}

/// HTML namespace used by Vue 3 AST/HIR nodes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HtmlNamespace {
    /// HTML namespace.
    Html,
    /// SVG namespace.
    Svg,
    /// MathML namespace.
    MathMl,
}

/// Vue 3 element category.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vue3ElementType {
    /// Native element.
    Element,
    /// Component element.
    Component,
    /// Slot outlet element.
    SlotOutlet,
    /// Template container element.
    Template,
}

/// Vue 3 element prop.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vue3Prop {
    /// Static attribute.
    Attribute(Vue3Attribute),
    /// Directive attribute.
    Directive(Vue3Directive),
}

impl Vue3Prop {
    fn compat_attribute(attribute: TemplateAttribute) -> Self {
        Self::Attribute(Vue3Attribute {
            name: attribute.name,
            value: attribute.value,
            span: None,
            name_span: None,
            value_span: None,
            quote: None,
        })
    }
}

impl From<TemplateAttribute> for Vue3Prop {
    fn from(attribute: TemplateAttribute) -> Self {
        Self::compat_attribute(attribute)
    }
}

/// Vue 3 static attribute payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3Attribute {
    /// Attribute name.
    pub name: String,
    /// Attribute value.
    pub value: Option<String>,
    /// Full attribute span.
    pub span: Option<Span>,
    /// Attribute name span.
    pub name_span: Option<Span>,
    /// Attribute value span.
    pub value_span: Option<Span>,
    /// Attribute quote kind.
    pub quote: Option<QuoteKind>,
}

/// Vue 3 directive payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3Directive {
    /// Normalized directive name.
    pub name: String,
    /// Raw directive name.
    pub raw_name: String,
    /// Directive argument expression.
    pub arg: Option<Vue3Expression>,
    /// Directive value expression.
    pub exp: Option<Vue3Expression>,
    /// Directive modifiers.
    pub modifiers: Vec<String>,
    /// Whether the directive argument is dynamic.
    pub is_dynamic_arg: bool,
    /// Full directive span.
    pub span: Option<Span>,
    /// Directive argument span.
    pub arg_span: Option<Span>,
    /// Directive expression span.
    pub exp_span: Option<Span>,
    /// Directive modifier spans.
    pub modifier_spans: Vec<NodeSpan>,
}

/// Vue 3 expression reference.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vue3Expression {
    /// Raw source expression.
    Raw(String),
    /// Registered JavaScript expression id.
    JsExpr(JsExprId),
}

impl Vue3Expression {
    /// Returns a displayable source string for compatibility projections.
    pub fn source_string(&self) -> String {
        match self {
            Self::Raw(value) => value.clone(),
            Self::JsExpr(id) => format!("#expr{}", id.0),
        }
    }
}

/// Vue 3 text node payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3Text {
    /// Text content.
    pub value: String,
}

/// Vue 3 comment node payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3Comment {
    /// Comment text.
    pub value: String,
}

/// Vue 3 interpolation node payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3Interpolation {
    /// Interpolation expression.
    pub expression: Vue3Expression,
}

/// Vue 3 compound expression payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3CompoundExpression {
    /// Ordered expression fragments.
    pub children: Vec<Vue3Expression>,
}

/// Vue 3 `v-if` node payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3If {
    /// Branch node ids.
    pub branches: Vec<NodeId>,
}

/// Vue 3 `v-if` branch payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3IfBranch {
    /// Optional branch condition.
    pub condition: Option<Vue3Expression>,
    /// Whether the branch originated from a template node.
    pub is_template_if: bool,
    /// Optional user-provided key expression.
    pub user_key: Option<Vue3Expression>,
}

/// Vue 3 `v-for` node payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3For {
    /// Iterable source expression.
    pub source: Vue3Expression,
    /// Value alias pattern id.
    pub value_alias: Option<JsPatternId>,
    /// Key alias pattern id.
    pub key_alias: Option<JsPatternId>,
    /// Object index alias pattern id.
    pub object_index_alias: Option<JsPatternId>,
}

/// Vue 3 text-call wrapper payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3TextCall {
    /// Content node id.
    pub content: NodeId,
    /// Optional generated codegen node reference.
    pub codegen_node: Option<Vue3CodegenRef>,
}

/// Vue 3 hoist slot payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3HoistSlot {
    /// Hoisted node id.
    pub node: NodeId,
}

/// Vue 3 generated import item.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3ImportItem {
    /// Local import binding name.
    pub name: String,
    /// Import source path.
    pub path: String,
}

/// Reference to a DOM codegen node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3CodegenRef {
    /// Referenced node id.
    pub node: NodeId,
}

/// Reference to an SSR codegen node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3SsrCodegenRef {
    /// Referenced node id.
    pub node: NodeId,
}

/// Compatibility alias for Vue 2 AST node kinds.
pub type Vue2NodeKind = Vue2AstKind;
/// Compatibility alias for Vue 3 AST node kinds.
pub type Vue3NodeKind = Vue3AstKind;

/// Returns the default Vue 2 root kind.
pub fn vue2_root_kind() -> Vue2NodeKind {
    Vue2AstKind::root()
}

/// Returns the default Vue 3 root kind.
pub fn vue3_root_kind() -> Vue3NodeKind {
    Vue3AstKind::root()
}
