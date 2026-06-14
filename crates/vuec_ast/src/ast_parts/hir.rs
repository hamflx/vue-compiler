/// HIR node kinds shared before target-specific MIR lowering.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HirNodeKind {
    /// HIR root node.
    Root(HirRoot),
    /// Native element node.
    Element(HirElement),
    /// Component node.
    Component(HirComponent),
    /// Text node.
    Text(HirText),
    /// Interpolation node.
    Interpolation(HirInterpolation),
    /// Conditional node.
    If(HirIf),
    /// Loop node.
    For(HirFor),
    /// Slot outlet node.
    SlotOutlet(HirSlotOutlet),
    /// Slot declaration node.
    SlotDecl(HirSlotDecl),
    /// Fragment node.
    Fragment(HirFragment),
}

/// HIR root payload.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HirRoot;

/// HIR native element payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HirElement {
    /// Element tag.
    pub tag: HirTag,
    /// Element namespace.
    pub namespace: HtmlNamespace,
    /// Lowered props.
    pub props: HirProps,
    /// Directive uses not lowered to built-in props.
    pub directives: Vec<HirDirectiveUse>,
    /// Static analysis result.
    pub constness: HirConstness,
}

/// HIR component payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HirComponent {
    /// Component name.
    pub name: String,
    /// Lowered props.
    pub props: HirProps,
}

/// HIR text payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HirText {
    /// Text content.
    pub value: String,
}

/// HIR interpolation payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HirInterpolation {
    /// Interpolation expression.
    pub expression: HirExpr,
}

/// HIR conditional payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HirIf {
    /// Conditional branches.
    pub branches: Vec<HirIfBranch>,
}

/// HIR slot outlet payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HirSlotOutlet {
    /// Optional slot name.
    pub name: Option<String>,
    /// Slot outlet props.
    pub props: HirProps,
}

/// HIR slot declaration payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HirSlotDecl {
    /// Slot name.
    pub name: String,
    /// Optional slot parameter pattern id.
    pub params: Option<JsPatternId>,
}

/// HIR fragment payload.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HirFragment;

/// HIR tag representation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HirTag {
    /// Native tag name.
    Native(String),
    /// Dynamic tag expression id.
    Dynamic(JsExprId),
}

/// HIR prop collection.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HirProps {
    /// Ordered prop segments preserving lowering order.
    pub segments: Vec<HirPropSegment>,
    /// Static attributes.
    pub static_attrs: Vec<HirStaticAttr>,
    /// Dynamic bindings.
    pub dynamic_bindings: Vec<HirBinding>,
    /// Event handlers.
    pub events: Vec<HirEvent>,
    /// Object `v-bind` entries.
    pub object_bindings: Vec<HirObjectBinding>,
    /// Object `v-on` entries.
    pub object_listeners: Vec<HirObjectListeners>,
    /// Key expression id.
    pub key: Option<JsExprId>,
    /// Ref metadata.
    pub ref_name: Option<HirRef>,
}

/// Ordered HIR prop segment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HirPropSegment {
    /// Static attribute segment.
    StaticAttr(HirStaticAttr),
    /// Dynamic binding segment.
    DynamicBinding(HirBinding),
    /// Event segment.
    Event(HirEvent),
    /// Object binding segment.
    ObjectBinding(HirObjectBinding),
    /// Object listeners segment.
    ObjectListeners(HirObjectListeners),
}

/// HIR static attribute.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HirStaticAttr {
    /// Attribute name.
    pub name: String,
    /// Attribute value.
    pub value: String,
}

/// HIR dynamic binding.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HirBinding {
    /// Static binding name.
    pub name: String,
    /// Dynamic name expression id.
    pub dynamic_name: Option<JsExprId>,
    /// Bound value expression id.
    pub value: JsExprId,
    /// Whether the argument is dynamic.
    pub dynamic_arg: bool,
    /// Binding modifiers.
    pub modifiers: Vec<String>,
}

/// HIR event handler.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HirEvent {
    /// Static event name.
    pub name: String,
    /// Dynamic event name expression id.
    pub dynamic_name: Option<JsExprId>,
    /// Handler statement id.
    pub handler: JsStmtId,
    /// Whether the argument is dynamic.
    pub dynamic_arg: bool,
    /// Event modifiers.
    pub modifiers: Vec<String>,
}

/// HIR object binding payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HirObjectBinding {
    /// Object expression id.
    pub value: JsExprId,
}

/// HIR object listeners payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HirObjectListeners {
    /// Object expression id.
    pub value: JsExprId,
}

/// HIR ref metadata.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HirRef {
    /// Ref name.
    pub name: String,
    /// Whether the ref is inside a loop.
    pub in_for: bool,
}

/// HIR directive use that remains after built-in lowering.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HirDirectiveUse {
    /// Directive name.
    pub name: String,
    /// Static argument.
    pub argument: Option<String>,
    /// Dynamic argument expression id.
    pub dynamic_argument: Option<JsExprId>,
    /// Optional directive expression id.
    pub expression: Option<JsExprId>,
    /// Directive modifiers.
    pub modifiers: Vec<String>,
}

/// HIR constness classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HirConstness {
    /// Runtime-dynamic content.
    Dynamic,
    /// Static but not fully constant content.
    Static,
    /// Fully constant content.
    Constant,
}

/// HIR expression payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HirExpr {
    /// Registered JavaScript expression id.
    Js(JsExprId),
    /// Vue 2 filter expression.
    Vue2Filter(Vue2FilterExpr),
}

/// HIR conditional branch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HirIfBranch {
    /// Optional branch condition expression id.
    pub condition: Option<JsExprId>,
    /// Branch body node id.
    pub body: NodeId,
}

/// HIR loop payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HirFor {
    /// Iterable source expression id.
    pub source: JsExprId,
    /// Value alias pattern id.
    pub value_alias: JsPatternId,
    /// Key alias pattern id.
    pub key_alias: Option<JsPatternId>,
    /// Index alias pattern id.
    pub index_alias: Option<JsPatternId>,
    /// Loop body node id.
    pub body: NodeId,
}
