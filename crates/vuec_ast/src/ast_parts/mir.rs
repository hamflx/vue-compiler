/// Vue 2 target-specific MIR node kinds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vue2MirKind {
    /// Vue 2 MIR root node.
    Root(Vue2MirRoot),
    /// Create-element call.
    CreateElement(Box<Vue2CreateElement>),
    /// Text call.
    Text(Vue2TextCall),
    /// Comment node.
    Comment {
        /// Comment text.
        value: String,
    },
    /// Conditional branch.
    If(Vue2IfMir),
    /// Render-list loop.
    For(Vue2ForMir),
    /// Static render function reference.
    RenderStatic(Vue2RenderStatic),
    /// `v-once` render-once wrapper used inside `v-for`.
    Once(Vue2Once),
    /// `<slot>` outlet call.
    SlotOutlet(Vue2SlotOutlet),
    /// Scoped slot function.
    ScopedSlot(Vue2ScopedSlot),
    /// Filter call.
    FilterCall {
        /// Filter name.
        name: String,
        /// Filter argument expression ids.
        args: Vec<JsExprId>,
    },
    /// Runtime directive record.
    Directive(Vue2DirectiveRuntime),
}

/// Vue 2 MIR root payload.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2MirRoot;

/// Vue 2 create-element MIR payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2CreateElement {
    /// Element tag expression.
    pub tag: MirExpr,
    /// Element data object payload.
    pub data: Option<Vue2DataObject>,
    /// Whether this call targets a component.
    pub is_component: bool,
    /// Whether the element renders a `<template>` container.
    pub is_template: bool,
    /// Whether this call should render a validation wrapper around its VNode.
    pub validation: Option<Vue2ValidationData>,
    /// Children normalization mode.
    pub normalization_type: Vue2NormalizationType,
}

/// Vue 2 data object payload.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2DataObject {
    /// Runtime directive records.
    pub directives: Vec<Vue2DirectiveRuntime>,
    /// Key binding.
    pub key: Option<MirExpr>,
    /// Ref binding.
    pub ref_name: Option<MirExpr>,
    /// Whether the ref is inside a `v-for`.
    pub ref_in_for: bool,
    /// Whether this VNode carries `pre:true`.
    pub pre: bool,
    /// Original tag for component data.
    pub tag: Option<String>,
    /// Static class expression.
    pub static_class: Option<MirExpr>,
    /// Dynamic class expression.
    pub class_binding: Option<MirExpr>,
    /// Static style expression.
    pub static_style: Option<MirExpr>,
    /// Dynamic style expression.
    pub style_binding: Option<MirExpr>,
    /// Static attrs payload.
    pub attrs: Vec<Vue2DataProp>,
    /// DOM props payload.
    pub dom_props: Vec<Vue2DataProp>,
    /// Dynamic attrs payload wrapped by `_b`.
    pub dynamic_attrs: Vec<Vue2DataProp>,
    /// Component or DOM listeners.
    pub events: BTreeMap<String, Vec<Vue2EventHandler>>,
    /// Native component listeners.
    pub native_events: BTreeMap<String, Vec<Vue2EventHandler>>,
    /// Legacy slot target expression.
    pub slot: Option<MirExpr>,
    /// Scoped slots object.
    pub scoped_slots: Vec<Vue2ScopedSlot>,
    /// Component model payload.
    pub model: Option<Vue2ComponentModelMir>,
    /// Inline-template payload.
    pub inline_template: Option<Vue2InlineTemplate>,
    /// Validation directive data.
    pub validate: Option<Vue2Validation>,
    /// Validation rules.
    pub validators: Vec<Vue2Validator>,
    /// Object `v-bind` wrapper.
    pub wrap_data: Option<Vue2BindWrap>,
    /// Object `v-on` wrapper expression.
    pub wrap_listeners: Option<MirExpr>,
}

/// Vue 2 runtime directive record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2DirectiveRuntime {
    /// Normalized directive name.
    pub name: String,
    /// Raw directive name.
    pub raw_name: String,
    /// Optional directive value expression.
    pub value: Option<MirExpr>,
    /// Optional directive argument expression.
    pub arg: Option<MirExpr>,
    /// Whether the argument is dynamic.
    pub is_dynamic_arg: bool,
    /// Directive modifiers.
    pub modifiers: BTreeMap<String, bool>,
}

/// Vue 2 data object prop.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2DataProp {
    /// Prop or attr name.
    pub name: String,
    /// Prop or attr value expression.
    pub value: MirExpr,
    /// Whether the prop name is dynamic.
    pub dynamic: bool,
    /// Whether the value came from a static attribute and needs attr newline decoding.
    pub static_attribute: bool,
}

/// Vue 2 object `v-bind` wrapper payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2BindWrap {
    /// Bound object expression.
    pub value: MirExpr,
    /// Whether `.prop` applies.
    pub prop: bool,
    /// Whether `.sync` applies.
    pub sync: bool,
}

/// Vue 2 component model MIR payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2ComponentModelMir {
    /// Model value expression.
    pub value: MirExpr,
    /// Model callback statement.
    pub callback: JsStmtId,
    /// Original model expression string.
    pub expression: String,
}

/// Vue 2 inline-template payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2InlineTemplate {
    /// Inline template root node.
    pub body: Option<NodeId>,
}

/// Vue 2 validation wrapper payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2ValidationData {
    /// Validation directive metadata.
    pub validate: Option<Vue2Validation>,
    /// Validation rules.
    pub validators: Vec<Vue2Validator>,
}

/// Vue 2 conditional MIR payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2IfMir {
    /// Ordered branch conditions and bodies.
    pub branches: Vec<Vue2IfMirBranch>,
}

/// Vue 2 conditional branch MIR payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2IfMirBranch {
    /// Optional branch condition expression.
    pub condition: Option<JsExprId>,
    /// Branch body MIR node.
    pub body: NodeId,
}

/// Vue 2 render-list MIR payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2ForMir {
    /// Iterable source expression id.
    pub source: JsExprId,
    /// Value alias pattern id.
    pub alias: JsPatternId,
    /// First iterator alias pattern id.
    pub iterator1: Option<JsPatternId>,
    /// Second iterator alias pattern id.
    pub iterator2: Option<JsPatternId>,
    /// Loop body MIR node.
    pub body: NodeId,
}

/// Vue 2 static render call payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2RenderStatic {
    /// Static render function index.
    pub index: u32,
    /// Static root body MIR node.
    pub body: Option<NodeId>,
    /// Whether this static render function appears inside `v-for`.
    pub in_for: bool,
}

/// Vue 2 render-once wrapper payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2Once {
    /// Wrapped VNode body.
    pub body: NodeId,
    /// Stable once id allocated during lowering.
    pub once_id: u32,
    /// Optional key expression.
    pub key: Option<MirExpr>,
}

/// Vue 2 `<slot>` outlet payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2SlotOutlet {
    /// Slot name expression.
    pub name: MirExpr,
    /// Slot props passed as the third `_t` argument.
    pub props: Vec<Vue2DataProp>,
    /// Object `v-bind` passed as the fourth `_t` argument.
    pub bind: Option<MirExpr>,
}

/// Vue 2 scoped slot payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2ScopedSlot {
    /// Slot name expression.
    pub name: MirExpr,
    /// Slot parameter pattern id.
    pub params: Option<JsPatternId>,
    /// Slot body MIR node ids.
    pub body: Vec<NodeId>,
    /// Whether an empty slot scope proxies normal slots.
    pub proxy: bool,
    /// Whether this slot was generated from new `v-slot` syntax.
    pub new_syntax: bool,
    /// Whether this slot body represents a `<template>` fragment.
    pub body_is_fragment: bool,
    /// New `v-slot` syntax condition that wraps the whole slot object.
    pub condition: Option<JsExprId>,
    /// Following `v-else-if` / `v-else` slot-object branches for new `v-slot` syntax.
    #[serde(default)]
    pub branches: Vec<Vue2ScopedSlotBranch>,
    /// Legacy `slot-scope` condition that wraps the returned fragment body.
    pub legacy_condition: Option<JsExprId>,
    /// Optional scoped slot `v-for` source.
    pub for_source: Option<JsExprId>,
    /// Optional scoped slot `v-for` value alias.
    pub for_alias: Option<JsPatternId>,
    /// Optional scoped slot `v-for` first iterator alias.
    pub for_iterator1: Option<JsPatternId>,
    /// Optional scoped slot `v-for` second iterator alias.
    pub for_iterator2: Option<JsPatternId>,
    /// Whether the scoped slot collection needs force update.
    pub force_update: bool,
    /// Whether stable scoped slots need a generated branch key.
    #[serde(default)]
    pub needs_key: bool,
}

/// Vue 2 scoped slot conditional branch payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2ScopedSlotBranch {
    /// Optional branch expression. `None` represents `v-else`.
    pub condition: Option<JsExprId>,
    /// Slot object emitted for this branch.
    pub slot: Box<Vue2ScopedSlot>,
}

/// Vue 2 text-call MIR payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2TextCall {
    /// Text value expression.
    pub value: MirExpr,
}

/// Vue 2 children normalization mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vue2NormalizationType {
    /// No children normalization.
    None,
    /// Simple one-level normalization.
    Simple,
    /// Always normalize children deeply.
    Always,
}

/// Vue 3 DOM target-specific MIR node kinds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vue3DomMirKind {
    /// DOM MIR root node.
    Root(Vue3DomRoot),
    /// VNode call node.
    VNodeCall(Vue3VNodeCall),
    /// Text VNode call node.
    TextCall {
        /// Text value expression.
        value: MirExpr,
    },
    /// Interpolation node.
    Interpolation {
        /// Interpolation expression id.
        expression: JsExprId,
    },
    /// Conditional node.
    If {
        /// Optional condition expression id.
        condition: Option<JsExprId>,
    },
    /// Render-list loop node.
    For(Vue3ForMir),
    /// Render-slot call node.
    RenderSlot(Vue3RenderSlot),
    /// Directive wrapper node.
    WithDirectives,
    /// Cache expression node.
    Cache {
        /// Cache index.
        index: u32,
    },
    /// Memo expression node.
    Memo {
        /// Memo expression id.
        expression: JsExprId,
        /// Cache index.
        index: u32,
    },
    /// Hoisted expression node.
    Hoisted {
        /// Hoist index.
        index: u32,
    },
    /// Fragment node.
    Fragment,
}

/// Vue 3 DOM MIR root payload.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3DomRoot {
    /// Import items needed by generated DOM render code.
    pub imports: Vec<Vue3ImportItem>,
}

/// Vue 3 DOM `v-for` MIR payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3ForMir {
    /// Iterable source expression id.
    pub source: JsExprId,
    /// Value alias pattern id.
    pub value_alias: JsPatternId,
    /// Key alias pattern id.
    pub key_alias: Option<JsPatternId>,
    /// Index alias pattern id.
    pub index_alias: Option<JsPatternId>,
    /// Optional key expression.
    pub key: Option<MirExpr>,
    /// Optional memo metadata.
    pub memo: Option<Vue3ForMemo>,
}

/// Vue 3 DOM `v-memo` metadata for `v-for`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3ForMemo {
    /// Memo expression id.
    pub expression: JsExprId,
    /// Cache index.
    pub index: u32,
}

/// Vue 3 DOM VNode call payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3VNodeCall {
    /// VNode tag.
    pub tag: Vue3DomTag,
    /// VNode props.
    pub props: Vue3DomProps,
    /// Optional `v-show` expression.
    pub v_show: Option<JsExprId>,
    /// Runtime directives.
    pub directives: Vec<Vue3DomDirective>,
    /// Model directives.
    pub models: Vec<Vue3DomModel>,
    /// Content directive payload.
    pub content: Option<Vue3DomContent>,
    /// VNode children.
    pub children: MirChildren,
    /// Patch flags.
    pub patch_flag: Vue3PatchFlags,
    /// Dynamic prop names.
    pub dynamic_props: Vec<String>,
    /// Whether this call opens a block.
    pub is_block: bool,
    /// Whether block tracking is disabled.
    pub disable_tracking: bool,
    /// Whether this VNode targets a component.
    pub is_component: bool,
}

/// Vue 3 DOM VNode tag payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vue3DomTag {
    /// Native tag name.
    Native(String),
    /// Resolved component asset name.
    ComponentAsset(String),
    /// Dynamic component expression id.
    DynamicComponent(JsExprId),
    /// Runtime helper symbol used as the tag.
    RuntimeHelper(RuntimeHelper),
}

/// Vue 3 DOM prop collection.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3DomProps {
    /// Ordered prop segments preserving codegen order.
    pub segments: Vec<Vue3DomPropSegment>,
    /// Static attributes.
    pub static_attrs: Vec<Vue3DomStaticAttr>,
    /// Dynamic bindings.
    pub dynamic_bindings: Vec<Vue3DomBinding>,
    /// Event handlers.
    pub events: Vec<Vue3DomEvent>,
    /// Object `v-bind` entries.
    pub object_bindings: Vec<Vue3DomObjectBinding>,
    /// Object `v-on` entries.
    pub object_listeners: Vec<Vue3DomObjectListeners>,
    /// Prop normalization flags.
    pub normalize: Vue3DomPropsNormalize,
}

/// Ordered Vue 3 DOM prop segment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vue3DomPropSegment {
    /// Static attribute segment.
    StaticAttr(Vue3DomStaticAttr),
    /// Dynamic binding segment.
    DynamicBinding(Vue3DomBinding),
    /// Content directive segment.
    Content(Vue3DomContent),
    /// Model segment.
    Model(Vue3DomModel),
    /// Event segment.
    Event(Vue3DomEvent),
    /// Object binding segment.
    ObjectBinding(Vue3DomObjectBinding),
    /// Object listeners segment.
    ObjectListeners(Vue3DomObjectListeners),
}

/// Vue 3 DOM static attribute.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3DomStaticAttr {
    /// Attribute name.
    pub name: String,
    /// Attribute value.
    pub value: String,
}

/// Vue 3 DOM dynamic binding.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3DomBinding {
    /// Static binding name.
    pub name: String,
    /// Dynamic binding name expression id.
    pub dynamic_name: Option<JsExprId>,
    /// Bound value expression id.
    pub value: JsExprId,
    /// Whether the argument is dynamic.
    pub dynamic_arg: bool,
    /// Whether `.camel` is present.
    pub camel: bool,
    /// Whether `.prop` is present.
    pub force_prop: bool,
    /// Whether `.attr` is present.
    pub force_attr: bool,
}

/// Vue 3 DOM content directive payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vue3DomContent {
    /// `v-html` expression.
    Html {
        /// Optional raw HTML expression id.
        expression: Option<JsExprId>,
    },
    /// `v-text` expression.
    Text {
        /// Optional text expression id.
        expression: Option<JsExprId>,
    },
}

/// Vue 3 DOM model directive payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3DomModel {
    /// Model expression id.
    pub expression: JsExprId,
    /// Runtime model kind.
    pub kind: Vue3DomModelKind,
    /// Model modifiers.
    pub modifiers: Vec<String>,
}

/// Vue 3 DOM model runtime kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vue3DomModelKind {
    /// Text input model.
    Text,
    /// Radio input model.
    Radio,
    /// Checkbox input model.
    Checkbox,
    /// Select model.
    Select,
    /// Dynamic model.
    Dynamic,
}

/// Vue 3 DOM event handler payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3DomEvent {
    /// Static event name.
    pub name: String,
    /// Dynamic event name expression id.
    pub dynamic_name: Option<JsExprId>,
    /// Handler statement id.
    pub handler: JsStmtId,
    /// Whether the argument is dynamic.
    pub dynamic_arg: bool,
    /// Runtime event modifiers.
    pub runtime_modifiers: Vec<String>,
    /// Key modifiers.
    pub key_modifiers: Vec<String>,
    /// Event option modifiers.
    pub option_modifiers: Vec<String>,
    /// Rewritten click event target.
    pub click_event: Option<Vue3DomClickEvent>,
    /// Optional handler cache metadata.
    pub cache: Option<Vue3DomEventCache>,
}

/// Vue 3 DOM rewritten click event target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vue3DomClickEvent {
    /// Context-menu event.
    ContextMenu,
    /// Mouse-up event.
    MouseUp,
}

/// Vue 3 DOM event handler cache metadata.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3DomEventCache {
    /// Cache index.
    pub index: u32,
}

/// Vue 3 DOM object binding payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3DomObjectBinding {
    /// Object expression id.
    pub value: JsExprId,
}

/// Vue 3 DOM object listeners payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3DomObjectListeners {
    /// Object expression id.
    pub value: JsExprId,
    /// Whether listener key case must be preserved.
    pub preserve_case: bool,
}

/// Vue 3 DOM prop normalization flags.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3DomPropsNormalize {
    /// Whether `normalizeProps` is needed.
    pub normalize_props: bool,
    /// Whether `guardReactiveProps` is needed.
    pub guard_reactive_props: bool,
}

/// Vue 3 DOM runtime directive payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3DomDirective {
    /// Directive name.
    pub name: String,
    /// Static directive argument.
    pub argument: Option<String>,
    /// Dynamic argument expression id.
    pub dynamic_argument: Option<JsExprId>,
    /// Optional directive expression id.
    pub expression: Option<JsExprId>,
    /// Directive modifiers.
    pub modifiers: Vec<String>,
}

/// Vue 3 DOM render-slot call payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3RenderSlot {
    /// Slot name.
    pub name: Vue3DomSlotName,
    /// Slot props.
    pub props: Vue3DomProps,
    /// Fallback child node ids.
    pub fallback: Vec<NodeId>,
}

/// Vue 3 SSR slot render payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3SsrSlot {
    /// Slot name.
    pub name: Vue3DomSlotName,
    /// Slot props.
    pub props: Vue3DomProps,
    /// Fallback child node ids.
    pub fallback: Vec<NodeId>,
    /// Whether this slot must render with `ssrRenderSlotInner`.
    pub inner: bool,
}

/// Vue 3 SSR attrs render payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3SsrAttrs {
    /// Props to render as SSR attrs.
    pub props: Vue3DomProps,
    /// Custom directives whose SSR props participate in attrs rendering.
    #[serde(default)]
    pub directives: Vec<Vue3DomDirective>,
    /// Whether directive SSR props may provide textContent / innerHTML fallback.
    #[serde(default)]
    pub directive_content: bool,
    /// Static textarea fallback used when object attrs may provide `value`.
    #[serde(default)]
    pub textarea_value_fallback: Option<String>,
    /// Whether the full props payload must render through `ssrRenderAttrs`.
    #[serde(default)]
    pub force_render_attrs: bool,
    /// Optional `v-show` expression id.
    pub v_show: Option<JsExprId>,
    /// Optional SSR model metadata.
    pub v_model: Option<Vue3SsrModel>,
}

/// Vue 3 SSR model payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3SsrModel {
    /// Model expression id.
    pub expression: JsExprId,
    /// SSR model kind.
    pub kind: Vue3SsrModelKind,
}

/// Vue 3 SSR model rendering kind.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vue3SsrModelKind {
    /// Input value binding.
    InputValue,
    /// Radio input binding.
    InputRadio {
        /// Radio value expression.
        value: MirExpr,
    },
    /// Checkbox input binding.
    InputCheckbox {
        /// Checkbox value expression.
        value: MirExpr,
    },
    /// Checkbox true-value binding.
    InputCheckboxTrueValue {
        /// Checkbox true-value expression.
        true_value: MirExpr,
    },
    /// Dynamic input type binding.
    InputDynamicType {
        /// Input type expression id.
        type_expr: JsExprId,
        /// Model value expression.
        value: MirExpr,
    },
    /// Dynamic input props binding.
    InputDynamicProps,
    /// Textarea binding.
    Textarea,
    /// Select option binding.
    SelectOption {
        /// Option value expression.
        value: MirExpr,
    },
}

/// Vue 3 SSR content directive payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vue3SsrContent {
    /// Raw HTML content expression.
    Html {
        /// Raw HTML expression id.
        expression: JsExprId,
    },
    /// Text content expression.
    Text {
        /// Text expression id.
        expression: JsExprId,
    },
}

/// Vue 3 SSR component render payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3SsrComponent {
    /// Component tag expression.
    pub tag: MirExpr,
    /// Component props.
    pub props: Vue3DomProps,
    /// Component custom directive SSR props.
    #[serde(default)]
    pub directives: Vec<Vue3DomDirective>,
    /// Component slot payload.
    #[serde(default)]
    pub slots: Option<Vue3DomSlots>,
    /// Whether the component tag is resolved with `resolveDynamicComponent`.
    pub dynamic: bool,
}

/// Vue 3 SSR loop payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3SsrFor {
    /// Iterable source expression id.
    pub source: JsExprId,
    /// Value alias pattern id.
    pub value_alias: JsPatternId,
    /// Key alias pattern id.
    pub key_alias: Option<JsPatternId>,
    /// Index alias pattern id.
    pub index_alias: Option<JsPatternId>,
    /// Whether the loop renders an SSR fragment wrapper around its children.
    pub fragment: bool,
}

/// Vue 3 SSR teleport payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3SsrTeleport {
    /// Teleport target expression.
    pub target: MirExpr,
    /// Teleport disabled expression.
    pub disabled: MirExpr,
}

/// Vue 3 SSR suspense payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3SsrSuspense {
    /// Suspense slots.
    pub slots: Vue3DomSlots,
}

/// Vue 3 SSR target-specific MIR node kinds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vue3SsrMirKind {
    /// SSR MIR root node.
    Root(Vue3SsrRoot),
    /// Pushes a static string into the SSR buffer.
    PushString(String),
    /// Pushes an interpolated expression into the SSR buffer.
    PushInterpolated(MirExpr),
    /// Renders content directive output.
    RenderContent(Vue3SsrContent),
    /// Renders attributes.
    RenderAttrs(Vue3SsrAttrs),
    /// Renders a component.
    RenderComponent(Vue3SsrComponent),
    /// Renders an SSR Transition boundary while preserving client VNode fallback.
    Transition,
    /// Renders a slot.
    RenderSlot(Vue3SsrSlot),
    /// Conditional branch.
    If {
        /// Optional condition expression id.
        condition: Option<JsExprId>,
        /// Whether codegen emits the SSR false-branch comment marker when no alternate exists.
        comment: bool,
    },
    /// Loop node.
    For(Vue3SsrFor),
    /// Teleport node.
    Teleport(Vue3SsrTeleport),
    /// Suspense node.
    Suspense(Vue3SsrSuspense),
}

/// Vue 3 SSR MIR root payload.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3SsrRoot {
    /// Import items needed by generated SSR code.
    pub imports: Vec<Vue3ImportItem>,
}

/// Vapor target-specific MIR node kinds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VaporMirKind {
    /// Vapor MIR root node.
    Root,
    /// Static template fragment.
    Template(String),
    /// Reactive effect expression.
    Effect {
        /// Reactive effect expression id.
        expression: JsExprId,
    },
}

/// Target-independent MIR expression payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MirExpr {
    /// String literal.
    String(String),
    /// Boolean literal.
    Bool(bool),
    /// Null literal.
    Null,
    /// Registered JavaScript expression id.
    JsExpr(JsExprId),
    /// Runtime helper reference.
    Helper(RuntimeHelper),
}

/// Target-independent MIR children payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MirChildren {
    /// No children.
    None,
    /// Text children.
    Text(String),
    /// Child node ids.
    Nodes(Vec<NodeId>),
    /// Slot children.
    Slots(Vue3DomSlots),
}

/// Vue 3 DOM slot collection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3DomSlots {
    /// Static slots.
    pub slots: Vec<Vue3DomSlot>,
    /// Dynamic slot entries.
    pub dynamic_slots: Vec<Vue3DomDynamicSlot>,
    /// Slot stability flag.
    pub flag: Vue3SlotFlag,
}

/// Vue 3 DOM static slot payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3DomSlot {
    /// Slot name.
    pub name: String,
    /// Optional slot parameter pattern id.
    pub params: Option<JsPatternId>,
    /// Slot child node ids.
    pub children: Vec<NodeId>,
}

/// Vue 3 DOM dynamic slot payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vue3DomDynamicSlot {
    /// Plain dynamic slot.
    Slot(Vue3DomDynamicSlotObject),
    /// Conditional dynamic slot.
    Conditional(Vue3DomConditionalSlot),
    /// Loop-generated dynamic slot.
    For(Vue3DomForSlot),
}

/// Vue 3 DOM dynamic slot object.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3DomDynamicSlotObject {
    /// Slot name.
    pub name: Vue3DomSlotName,
    /// Optional slot parameter pattern id.
    pub params: Option<JsPatternId>,
    /// Slot child node ids.
    pub children: Vec<NodeId>,
    /// Optional branch key.
    pub key: Option<String>,
}

/// Vue 3 DOM conditional dynamic slot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3DomConditionalSlot {
    /// Optional condition expression id.
    pub condition: Option<JsExprId>,
    /// Slot object for the truthy branch.
    pub slot: Vue3DomDynamicSlotObject,
    /// Alternate dynamic slot branch.
    pub alternate: Option<Box<Vue3DomDynamicSlot>>,
}

/// Vue 3 DOM loop-generated dynamic slot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3DomForSlot {
    /// Iterable source expression id.
    pub source: JsExprId,
    /// Value alias pattern id.
    pub value_alias: JsPatternId,
    /// Key alias pattern id.
    pub key_alias: Option<JsPatternId>,
    /// Index alias pattern id.
    pub index_alias: Option<JsPatternId>,
    /// Slot object generated for each item.
    pub slot: Vue3DomDynamicSlotObject,
}

/// Vue 3 DOM slot name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vue3DomSlotName {
    /// Static slot name.
    Static(String),
    /// Dynamic slot name expression id.
    Dynamic(JsExprId),
}

/// Vue 3 slot stability flag.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vue3SlotFlag {
    /// Stable slots.
    Stable,
    /// Dynamic slots.
    Dynamic,
    /// Forwarded slots.
    Forwarded,
}

/// Vue 3 patch flag bitset.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3PatchFlags {
    /// Numeric patch flag bits.
    pub bits: i32,
}

/// Target-discriminated MIR document wrapper.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mir {
    /// Vue 2 render-function MIR.
    Vue2(Vue2Mir),
    /// Vue 3 DOM render-function MIR.
    Vue3Dom(Vue3DomMir),
    /// Vue 3 SSR render-function MIR.
    Vue3Ssr(Vue3SsrMir),
    /// Vapor MIR.
    Vapor(VaporMir),
}

impl Mir {
    /// Returns the target represented by this MIR document.
    pub fn target(&self) -> MirTarget {
        match self {
            Self::Vue2(_) => MirTarget::Vue2,
            Self::Vue3Dom(_) => MirTarget::Vue3Dom,
            Self::Vue3Ssr(_) => MirTarget::Vue3Ssr,
            Self::Vapor(_) => MirTarget::Vapor,
        }
    }
}

/// MIR target discriminator.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MirTarget {
    /// Vue 2 render-function target.
    Vue2,
    /// Vue 3 DOM render-function target.
    Vue3Dom,
    /// Vue 3 SSR render-function target.
    Vue3Ssr,
    /// Vue Vapor target.
    Vapor,
}

/// Vue 2 AST document.
pub type Vue2Ast = AstDocument<Vue2NodeKind>;
/// Vue 3 AST document.
pub type Vue3Ast = AstDocument<Vue3NodeKind>;
/// Shared HIR document.
pub type Hir = AstDocument<HirNodeKind>;
/// Vue 2 target MIR document.
pub type Vue2Mir = AstDocument<Vue2MirKind>;
/// Vue 3 DOM target MIR document.
pub type Vue3DomMir = AstDocument<Vue3DomMirKind>;
/// Vue 3 SSR target MIR document.
pub type Vue3SsrMir = AstDocument<Vue3SsrMirKind>;
/// Vapor target MIR document.
pub type VaporMir = AstDocument<VaporMirKind>;
/// Compatibility alias for [`Hir`].
pub type HIR = Hir;
/// Compatibility alias for [`Mir`].
pub type MIR = Mir;
