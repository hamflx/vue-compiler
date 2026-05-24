#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use vuec_source::{FileId, Span};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct NodeId(pub u32);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Node<K> {
    pub id: NodeId,
    pub kind: K,
    pub span: NodeSpan,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    pub index_in_parent: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AstDocument<K> {
    pub root: NodeId,
    pub nodes: Vec<Node<K>>,
}

impl<K> AstDocument<K> {
    pub fn new<S>(root_kind: K, span: S) -> Self
    where
        S: Into<NodeSpan>,
    {
        let mut document = Self {
            root: NodeId(0),
            nodes: Vec::new(),
        };
        let root = document.push(root_kind, span);
        document.root = root;
        document
    }

    pub fn push<S>(&mut self, kind: K, span: S) -> NodeId
    where
        S: Into<NodeSpan>,
    {
        let id = NodeId(self.nodes.len() as u32);
        self.nodes.push(Node {
            id,
            kind,
            span: span.into(),
            parent: None,
            children: Vec::new(),
            index_in_parent: 0,
        });
        id
    }

    pub fn push_child<S>(&mut self, parent: NodeId, kind: K, span: S) -> NodeId
    where
        S: Into<NodeSpan>,
    {
        let child = self.push(kind, span);
        self.attach_child(parent, child);
        child
    }

    pub fn attach_child(&mut self, parent: NodeId, child: NodeId) {
        if parent == child {
            return;
        }
        if self.node(parent).is_none() || self.node(child).is_none() {
            return;
        }
        if let Some(old_parent) = self.node(child).and_then(|node| node.parent) {
            if let Some(old_parent_node) = self.node_mut(old_parent) {
                old_parent_node.children.retain(|id| *id != child);
            }
            self.refresh_child_indexes(old_parent);
        }
        let index_in_parent = self
            .node(parent)
            .map_or(0, |node| node.children.len() as u32);
        if let Some(parent_node) = self.node_mut(parent) {
            parent_node.children.push(child);
        }
        if let Some(child_node) = self.node_mut(child) {
            child_node.parent = Some(parent);
            child_node.index_in_parent = index_in_parent;
        }
    }

    pub fn replace_children(&mut self, parent: NodeId, children: Vec<NodeId>) {
        if self.node(parent).is_none() {
            return;
        }
        let old_children = self
            .node(parent)
            .map(|node| node.children.clone())
            .unwrap_or_default();
        for old_child in old_children {
            if !children.contains(&old_child) {
                if let Some(child_node) = self.node_mut(old_child) {
                    child_node.parent = None;
                    child_node.index_in_parent = 0;
                }
            }
        }
        for child in &children {
            if let Some(old_parent) = self.node(*child).and_then(|node| node.parent) {
                if old_parent != parent {
                    if let Some(old_parent_node) = self.node_mut(old_parent) {
                        old_parent_node.children.retain(|id| id != child);
                    }
                    self.refresh_child_indexes(old_parent);
                }
            }
        }
        if let Some(parent_node) = self.node_mut(parent) {
            parent_node.children = children.clone();
        }
        self.refresh_child_indexes(parent);
    }

    pub fn set_root(&mut self, id: NodeId) {
        self.root = id;
        if let Some(root_node) = self.node_mut(id) {
            root_node.parent = None;
            root_node.index_in_parent = 0;
        }
    }

    pub fn node(&self, id: NodeId) -> Option<&Node<K>> {
        self.nodes.get(id.0 as usize)
    }

    pub fn node_mut(&mut self, id: NodeId) -> Option<&mut Node<K>> {
        self.nodes.get_mut(id.0 as usize)
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn root_node(&self) -> Option<&Node<K>> {
        self.node(self.root)
    }

    pub fn root_node_mut(&mut self) -> Option<&mut Node<K>> {
        self.node_mut(self.root)
    }

    pub fn validate_tree(&self) -> Result<(), AstInvariantError> {
        let root_index = self.root.0 as usize;
        if root_index >= self.nodes.len() {
            return Err(AstInvariantError::MissingRoot { root: self.root });
        }
        for (index, node) in self.nodes.iter().enumerate() {
            if node.id.0 as usize != index {
                return Err(AstInvariantError::MismatchedNodeId {
                    expected: NodeId(index as u32),
                    actual: node.id,
                });
            }
            if node.id == self.root {
                if node.parent.is_some() || node.index_in_parent != 0 {
                    return Err(AstInvariantError::InvalidRootMetadata { root: self.root });
                }
            } else if node.parent.is_none() {
                return Err(AstInvariantError::DetachedNode { node: node.id });
            }
            for (child_index, child_id) in node.children.iter().copied().enumerate() {
                let child = self.node(child_id).ok_or(AstInvariantError::MissingChild {
                    parent: node.id,
                    child: child_id,
                })?;
                if child.parent != Some(node.id) || child.index_in_parent != child_index as u32 {
                    return Err(AstInvariantError::InvalidChildMetadata {
                        parent: node.id,
                        child: child_id,
                        expected_index: child_index as u32,
                    });
                }
            }
        }
        Ok(())
    }

    fn refresh_child_indexes(&mut self, parent: NodeId) {
        let children = self
            .node(parent)
            .map(|node| node.children.clone())
            .unwrap_or_default();
        for (index, child) in children.into_iter().enumerate() {
            if let Some(child_node) = self.node_mut(child) {
                child_node.parent = Some(parent);
                child_node.index_in_parent = index as u32;
            }
        }
    }
}

impl NodeSpan {
    pub fn source(&self) -> Option<Span> {
        match self {
            NodeSpan::Source(span) => Some(*span),
            NodeSpan::Generated { origin, .. } => *origin,
            NodeSpan::Missing { .. } => None,
        }
    }

    pub fn source_mut(&mut self) -> Option<&mut Span> {
        match self {
            NodeSpan::Source(span) => Some(span),
            NodeSpan::Generated { origin, .. } => origin.as_mut(),
            NodeSpan::Missing { .. } => None,
        }
    }
}

impl From<Span> for NodeSpan {
    fn from(span: Span) -> Self {
        NodeSpan::Source(span)
    }
}

impl From<Option<Span>> for NodeSpan {
    fn from(span: Option<Span>) -> Self {
        match span {
            Some(span) => NodeSpan::Source(span),
            None => NodeSpan::Missing {
                reason: MissingSpanReason::Synthetic,
            },
        }
    }
}

pub mod ids {
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
    pub struct JsExprId(pub u32);

    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
    pub struct JsStmtId(pub u32);

    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
    pub struct JsPatternId(pub u32);

    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
    pub struct JsProgramId(pub u32);
}

pub use ids::{JsExprId, JsPatternId, JsProgramId, JsStmtId};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GeneratedReason {
    ParseRecovery,
    Lowering,
    Codegen,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MissingSpanReason {
    ParseRecovery,
    LoweringGap,
    Synthetic,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeSpan {
    Source(Span),
    Generated {
        origin: Option<Span>,
        reason: GeneratedReason,
    },
    Missing {
        reason: MissingSpanReason,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AstInvariantError {
    MissingRoot {
        root: NodeId,
    },
    MismatchedNodeId {
        expected: NodeId,
        actual: NodeId,
    },
    InvalidRootMetadata {
        root: NodeId,
    },
    DetachedNode {
        node: NodeId,
    },
    MissingChild {
        parent: NodeId,
        child: NodeId,
    },
    InvalidChildMetadata {
        parent: NodeId,
        child: NodeId,
        expected_index: u32,
    },
}

impl NodeSpan {
    pub fn generated(origin: Option<Span>, reason: GeneratedReason) -> Self {
        Self::Generated { origin, reason }
    }

    pub fn missing(reason: MissingSpanReason) -> Self {
        Self::Missing { reason }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoweringMap {
    pub ast_to_hir: Vec<(NodeId, NodeId)>,
    pub hir_to_mir: Vec<(NodeId, NodeId)>,
}

impl LoweringMap {
    pub fn record_ast_to_hir(&mut self, ast: NodeId, hir: NodeId) {
        self.ast_to_hir.push((ast, hir));
    }

    pub fn record_hir_to_mir(&mut self, hir: NodeId, mir: NodeId) {
        self.hir_to_mir.push((hir, mir));
    }

    pub fn hir_for_ast(&self, ast: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        self.ast_to_hir
            .iter()
            .filter_map(move |(from, to)| (*from == ast).then_some(*to))
    }

    pub fn mir_for_hir(&self, hir: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        self.hir_to_mir
            .iter()
            .filter_map(move |(from, to)| (*from == hir).then_some(*to))
    }
}

pub trait PublicProjection {
    type Output;

    fn project_public(&self) -> Self::Output;
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicNode<K> {
    pub kind: K,
    pub span: NodeSpan,
    pub children: Vec<PublicNode<K>>,
}

impl<K> AstDocument<K>
where
    K: Clone,
{
    pub fn project_nested(&self) -> Option<PublicNode<K>> {
        self.project_nested_node(self.root)
    }

    fn project_nested_node(&self, id: NodeId) -> Option<PublicNode<K>> {
        let node = self.node(id)?;
        Some(PublicNode {
            kind: node.kind.clone(),
            span: node.span.clone(),
            children: node
                .children
                .iter()
                .filter_map(|child| self.project_nested_node(*child))
                .collect(),
        })
    }
}

impl<K> PublicProjection for AstDocument<K>
where
    K: Clone,
{
    type Output = PublicNode<K>;

    fn project_public(&self) -> Self::Output {
        self.project_nested()
            .expect("AstDocument root must reference an existing node")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RuntimeHelper {
    Vue2CreateElement,
    Vue2CreateTextVNode,
    Vue2ToString,
    Vue2RenderList,
    Vue2ResolveFilter,
    Vue3ResolveDirective,
    Vue3WithDirectives,
    Vue3SetBlockTracking,
    Vue3OpenBlock,
    Vue3CreateElementVNode,
    Vue3CreateElementBlock,
    Vue3CreateCommentVNode,
    Vue3CreateTextVNode,
    Vue3Fragment,
    Vue3ToDisplayString,
    Vue3RenderList,
    Vue3RenderSlot,
    Vue3NormalizeClass,
    Vue3NormalizeProps,
    Vue3NormalizeStyle,
    Vue3GuardReactiveProps,
    Vue3MergeProps,
    Vue3ResolveComponent,
    Vue3ResolveDynamicComponent,
    Vue3BaseTransition,
    Vue3Transition,
    Vue3TransitionGroup,
    Vue3Teleport,
    Vue3Suspense,
    Vue3KeepAlive,
    Vue3WithCtx,
    Vue3CreateBlock,
    Vue3CreateVNode,
    Vue3CreateSlots,
    Vue3CreateStaticVNode,
    Vue3IsMemoSame,
    Vue3WithMemo,
    Vue3ToHandlers,
    Vue3Camelize,
    Vue3Capitalize,
    Vue3ToHandlerKey,
    Vue3PushScopeId,
    Vue3PopScopeId,
    Vue3Unref,
    Vue3IsRef,
    Vue3VModelRadio,
    Vue3VModelCheckbox,
    Vue3VModelText,
    Vue3VModelSelect,
    Vue3VModelDynamic,
    Vue3WithModifiers,
    Vue3WithKeys,
    Vue3VShow,
    Vue3SsrInterpolate,
    Vue3SsrRenderVNode,
    Vue3SsrRenderComponent,
    Vue3SsrRenderSlot,
    Vue3SsrRenderSlotInner,
    Vue3SsrRenderClass,
    Vue3SsrRenderStyle,
    Vue3SsrRenderAttrs,
    Vue3SsrRenderAttr,
    Vue3SsrRenderDynamicAttr,
    Vue3SsrRenderList,
    Vue3SsrIncludeBooleanAttr,
    Vue3SsrLooseEqual,
    Vue3SsrLooseContain,
    Vue3SsrRenderDynamicModel,
    Vue3SsrGetDynamicModelProps,
    Vue3SsrRenderTeleport,
    Vue3SsrRenderSuspense,
    Vue3SsrGetDirectiveProps,
}

pub type Cst = AstDocument<CstNodeKind>;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CstNodeKind {
    Document,
    SfcBlock(CstSfcBlock),
    Element(CstElement),
    Attribute(CstAttribute),
    Text { raw: String },
    Comment { raw: String },
    Cdata { raw: String },
    Doctype { raw: String },
    Interpolation(CstInterpolation),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CstSfcBlock {
    pub block_type: String,
    pub raw_tag: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CstElement {
    pub raw_tag: String,
    pub self_closing: bool,
    pub open_span: Span,
    pub close_span: Option<Span>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CstAttribute {
    pub raw_name: String,
    pub raw_value: Option<String>,
    pub quote: Option<QuoteKind>,
    pub name_span: Span,
    pub value_span: Option<Span>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuoteKind {
    Double,
    Single,
    Unquoted,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CstInterpolation {
    pub raw: String,
    pub open_span: Span,
    pub inner_span: Span,
    pub close_span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateAttribute {
    pub name: String,
    pub value: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vue2AstKind {
    Root(Vue2Root),
    Element(Vue2Element),
    Text(Vue2Text),
    ExpressionText(Vue2ExpressionText),
    Comment(Vue2Comment),
}

impl Vue2AstKind {
    pub fn root() -> Self {
        Self::Root(Vue2Root::default())
    }

    pub fn element(tag: impl Into<String>) -> Self {
        Self::Element(Vue2Element::new(tag))
    }

    pub fn text(value: impl Into<String>) -> Self {
        Self::Text(Vue2Text {
            value: value.into(),
            static_node: false,
        })
    }

    pub fn expression_text(raw: impl Into<String>) -> Self {
        Self::ExpressionText(Vue2ExpressionText {
            raw: raw.into(),
            expr: None,
            filter_expr: None,
        })
    }

    pub fn comment(value: impl Into<String>) -> Self {
        Self::Comment(Vue2Comment {
            value: value.into(),
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2Root {
    pub source_id: Option<FileId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2Element {
    pub tag: String,
    pub attrs_list: Vec<Vue2Attribute>,
    pub attrs_map: BTreeMap<String, String>,
    pub raw_attrs_map: BTreeMap<String, Vue2Attribute>,
    pub attrs: Vec<Vue2Attribute>,
    pub props: Vec<Vue2Attribute>,
    pub dynamic_attrs: Vec<Vue2Attribute>,
    pub directives: Vec<Vue2Directive>,
    pub events: BTreeMap<String, Vec<Vue2EventHandler>>,
    pub native_events: BTreeMap<String, Vec<Vue2EventHandler>>,
    pub ns: Option<String>,
    pub plain: bool,
    pub forbidden: bool,
    pub pre: bool,
    pub once: bool,
    pub has_bindings: bool,
    pub if_exp: Option<JsExprId>,
    pub elseif: Option<JsExprId>,
    pub else_branch: bool,
    pub if_conditions: Vec<Vue2IfCondition>,
    pub for_exp: Option<JsExprId>,
    pub alias: Option<JsPatternId>,
    pub iterator1: Option<JsPatternId>,
    pub iterator2: Option<JsPatternId>,
    pub key: Option<JsExprId>,
    pub ref_name: Option<String>,
    pub ref_in_for: bool,
    pub slot_name: Option<String>,
    pub slot_target: Option<String>,
    pub slot_target_dynamic: bool,
    pub slot_scope: Option<JsPatternId>,
    pub scoped_slots: BTreeMap<String, NodeId>,
    pub component: Option<String>,
    pub inline_template: bool,
    pub static_class: Option<String>,
    pub class_binding: Option<JsExprId>,
    pub static_style: Option<String>,
    pub style_binding: Option<JsExprId>,
    pub model: Option<Vue2ComponentModel>,
    pub wrap_data: Option<Vue2DataWrap>,
    pub wrap_listeners: Option<String>,
    pub static_node: bool,
    pub static_root: bool,
    pub static_in_for: bool,
}

impl Vue2Element {
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
            elseif: None,
            else_branch: false,
            if_conditions: Vec::new(),
            for_exp: None,
            alias: None,
            iterator1: None,
            iterator2: None,
            key: None,
            ref_name: None,
            ref_in_for: false,
            slot_name: None,
            slot_target: None,
            slot_target_dynamic: false,
            slot_scope: None,
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
            static_node: false,
            static_root: false,
            static_in_for: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2Text {
    pub value: String,
    pub static_node: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2ExpressionText {
    pub raw: String,
    pub expr: Option<JsExprId>,
    pub filter_expr: Option<Vue2FilterExpr>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2Comment {
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2Attribute {
    pub name: String,
    pub value: String,
    pub dynamic: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2Directive {
    pub name: String,
    pub raw_name: String,
    pub value: Option<JsExprId>,
    pub arg: Option<String>,
    pub is_dynamic_arg: bool,
    pub modifiers: BTreeMap<String, bool>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2EventHandler {
    pub value: JsStmtId,
    pub modifiers: BTreeMap<String, bool>,
    pub dynamic: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2IfCondition {
    pub exp: Option<JsExprId>,
    pub block: NodeId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2ComponentModel {
    pub value: JsExprId,
    pub callback: JsStmtId,
    pub expression: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vue2DataWrap {
    Bind {
        value: JsExprId,
        prop: bool,
        sync: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2FilterExpr {
    pub raw: String,
    pub base: JsExprId,
    pub filters: Vec<Vue2FilterCall>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2FilterCall {
    pub name: String,
    pub args: Vec<JsExprId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vue3AstKind {
    Root(Vue3Root),
    Element(Vue3Element),
    Text(Vue3Text),
    Comment(Vue3Comment),
    Interpolation(Vue3Interpolation),
    CompoundExpression(Vue3CompoundExpression),
    If(Vue3If),
    IfBranch(Vue3IfBranch),
    For(Vue3For),
    TextCall(Vue3TextCall),
}

impl Vue3AstKind {
    pub fn root() -> Self {
        Self::Root(Vue3Root::default())
    }

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

    pub fn text(value: impl Into<String>) -> Self {
        Self::Text(Vue3Text {
            value: value.into(),
        })
    }

    pub fn interpolation(expression: impl Into<String>) -> Self {
        Self::Interpolation(Vue3Interpolation {
            expression: Vue3Expression::Raw(expression.into()),
        })
    }

    pub fn comment(value: impl Into<String>) -> Self {
        Self::Comment(Vue3Comment {
            value: value.into(),
        })
    }

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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3Root {
    pub source_id: Option<FileId>,
    pub helpers: BTreeSet<RuntimeHelper>,
    pub components: BTreeSet<String>,
    pub directives: BTreeSet<String>,
    pub hoists: Vec<Vue3HoistSlot>,
    pub temps: u32,
    pub cached: u32,
    pub codegen_node: Option<Vue3CodegenRef>,
}

impl Default for Vue3Root {
    fn default() -> Self {
        Self {
            source_id: None,
            helpers: BTreeSet::new(),
            components: BTreeSet::new(),
            directives: BTreeSet::new(),
            hoists: Vec::new(),
            temps: 0,
            cached: 0,
            codegen_node: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3Element {
    pub tag: String,
    pub tag_type: Vue3ElementType,
    pub ns: HtmlNamespace,
    pub props: Vec<Vue3Prop>,
    pub self_closing: bool,
    pub codegen_node: Option<Vue3CodegenRef>,
    pub ssr_codegen_node: Option<Vue3SsrCodegenRef>,
}

impl Vue3Element {
    pub fn template_attributes(&self) -> Vec<TemplateAttribute> {
        self.props
            .iter()
            .filter_map(|prop| match prop {
                Vue3Prop::Attribute(attr) => Some(TemplateAttribute {
                    name: attr.name.clone(),
                    value: attr.value.clone(),
                }),
                Vue3Prop::Directive(directive) => Some(TemplateAttribute {
                    name: directive.raw_name.clone(),
                    value: directive.exp.as_ref().map(Vue3Expression::source_string),
                }),
            })
            .collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HtmlNamespace {
    Html,
    Svg,
    MathMl,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vue3ElementType {
    Element,
    Component,
    SlotOutlet,
    Template,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vue3Prop {
    Attribute(Vue3Attribute),
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3Attribute {
    pub name: String,
    pub value: Option<String>,
    pub span: Option<Span>,
    pub name_span: Option<Span>,
    pub value_span: Option<Span>,
    pub quote: Option<QuoteKind>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3Directive {
    pub name: String,
    pub raw_name: String,
    pub arg: Option<Vue3Expression>,
    pub exp: Option<Vue3Expression>,
    pub modifiers: Vec<String>,
    pub is_dynamic_arg: bool,
    pub span: Option<Span>,
    pub arg_span: Option<Span>,
    pub exp_span: Option<Span>,
    pub modifier_spans: Vec<NodeSpan>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vue3Expression {
    Raw(String),
    JsExpr(JsExprId),
}

impl Vue3Expression {
    pub fn source_string(&self) -> String {
        match self {
            Self::Raw(value) => value.clone(),
            Self::JsExpr(id) => format!("#expr{}", id.0),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3Text {
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3Comment {
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3Interpolation {
    pub expression: Vue3Expression,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3CompoundExpression {
    pub children: Vec<Vue3Expression>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3If {
    pub branches: Vec<NodeId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3IfBranch {
    pub condition: Option<Vue3Expression>,
    pub is_template_if: bool,
    pub user_key: Option<Vue3Expression>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3For {
    pub source: Vue3Expression,
    pub value_alias: Option<JsPatternId>,
    pub key_alias: Option<JsPatternId>,
    pub object_index_alias: Option<JsPatternId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3TextCall {
    pub content: NodeId,
    pub codegen_node: Option<Vue3CodegenRef>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3HoistSlot {
    pub node: NodeId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3CodegenRef {
    pub node: NodeId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3SsrCodegenRef {
    pub node: NodeId,
}

pub type Vue2NodeKind = Vue2AstKind;
pub type Vue3NodeKind = Vue3AstKind;

pub fn vue2_root_kind() -> Vue2NodeKind {
    Vue2AstKind::root()
}

pub fn vue3_root_kind() -> Vue3NodeKind {
    Vue3AstKind::root()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HirNodeKind {
    Root(HirRoot),
    Element(HirElement),
    Component(HirComponent),
    Text(HirText),
    Interpolation(HirInterpolation),
    If(HirIf),
    For(HirFor),
    SlotOutlet(HirSlotOutlet),
    SlotDecl(HirSlotDecl),
    Fragment(HirFragment),
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HirRoot;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HirElement {
    pub tag: HirTag,
    pub namespace: HtmlNamespace,
    pub props: HirProps,
    pub directives: Vec<HirDirectiveUse>,
    pub constness: HirConstness,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HirComponent {
    pub name: String,
    pub props: HirProps,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HirText {
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HirInterpolation {
    pub expression: HirExpr,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HirIf {
    pub branches: Vec<HirIfBranch>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HirSlotOutlet {
    pub name: Option<String>,
    pub props: HirProps,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HirSlotDecl {
    pub name: String,
    pub params: Option<JsPatternId>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HirFragment;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HirTag {
    Native(String),
    Dynamic(JsExprId),
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HirProps {
    pub segments: Vec<HirPropSegment>,
    pub static_attrs: Vec<HirStaticAttr>,
    pub dynamic_bindings: Vec<HirBinding>,
    pub events: Vec<HirEvent>,
    pub object_bindings: Vec<HirObjectBinding>,
    pub object_listeners: Vec<HirObjectListeners>,
    pub key: Option<JsExprId>,
    pub ref_name: Option<HirRef>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HirPropSegment {
    StaticAttr(HirStaticAttr),
    DynamicBinding(HirBinding),
    Event(HirEvent),
    ObjectBinding(HirObjectBinding),
    ObjectListeners(HirObjectListeners),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HirStaticAttr {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HirBinding {
    pub name: String,
    pub dynamic_name: Option<JsExprId>,
    pub value: JsExprId,
    pub dynamic_arg: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HirEvent {
    pub name: String,
    pub dynamic_name: Option<JsExprId>,
    pub handler: JsStmtId,
    pub dynamic_arg: bool,
    pub modifiers: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HirObjectBinding {
    pub value: JsExprId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HirObjectListeners {
    pub value: JsExprId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HirRef {
    pub name: String,
    pub in_for: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HirDirectiveUse {
    pub name: String,
    pub argument: Option<String>,
    pub dynamic_argument: Option<JsExprId>,
    pub expression: Option<JsExprId>,
    pub modifiers: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HirConstness {
    Dynamic,
    Static,
    Constant,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HirExpr {
    Js(JsExprId),
    Vue2Filter(Vue2FilterExpr),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HirIfBranch {
    pub condition: Option<JsExprId>,
    pub body: NodeId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HirFor {
    pub source: JsExprId,
    pub value_alias: JsPatternId,
    pub key_alias: Option<JsPatternId>,
    pub index_alias: Option<JsPatternId>,
    pub body: NodeId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vue2MirKind {
    Root,
    CreateElement(Vue2CreateElement),
    Text(Vue2TextCall),
    Comment {
        value: String,
    },
    If {
        condition: JsExprId,
    },
    For {
        source: JsExprId,
        alias: JsPatternId,
    },
    RenderStatic {
        index: u32,
    },
    ScopedSlot {
        name: String,
        params: Option<JsPatternId>,
    },
    FilterCall {
        name: String,
    },
    Directive {
        name: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2CreateElement {
    pub tag: MirExpr,
    pub normalization_type: Vue2NormalizationType,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue2TextCall {
    pub value: MirExpr,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vue2NormalizationType {
    None,
    Simple,
    Always,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vue3DomMirKind {
    Root,
    VNodeCall(Vue3VNodeCall),
    TextCall { value: MirExpr },
    Interpolation { expression: JsExprId },
    If { condition: Option<JsExprId> },
    For(Vue3ForMir),
    RenderSlot(Vue3RenderSlot),
    WithDirectives,
    Cache { index: u32 },
    Memo { expression: JsExprId, index: u32 },
    Hoisted { index: u32 },
    Fragment,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3ForMir {
    pub source: JsExprId,
    pub value_alias: JsPatternId,
    pub key_alias: Option<JsPatternId>,
    pub index_alias: Option<JsPatternId>,
    pub key: Option<MirExpr>,
    pub memo: Option<Vue3ForMemo>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3ForMemo {
    pub expression: JsExprId,
    pub index: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3VNodeCall {
    pub tag: Vue3DomTag,
    pub props: Vue3DomProps,
    pub directives: Vec<Vue3DomDirective>,
    pub children: MirChildren,
    pub patch_flag: Vue3PatchFlags,
    pub dynamic_props: Vec<String>,
    pub is_block: bool,
    pub disable_tracking: bool,
    pub is_component: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vue3DomTag {
    Native(String),
    ComponentAsset(String),
    DynamicComponent(JsExprId),
    RuntimeHelper(RuntimeHelper),
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3DomProps {
    pub segments: Vec<Vue3DomPropSegment>,
    pub static_attrs: Vec<Vue3DomStaticAttr>,
    pub dynamic_bindings: Vec<Vue3DomBinding>,
    pub events: Vec<Vue3DomEvent>,
    pub object_bindings: Vec<Vue3DomObjectBinding>,
    pub object_listeners: Vec<Vue3DomObjectListeners>,
    pub normalize: Vue3DomPropsNormalize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vue3DomPropSegment {
    StaticAttr(Vue3DomStaticAttr),
    DynamicBinding(Vue3DomBinding),
    Event(Vue3DomEvent),
    ObjectBinding(Vue3DomObjectBinding),
    ObjectListeners(Vue3DomObjectListeners),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3DomStaticAttr {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3DomBinding {
    pub name: String,
    pub dynamic_name: Option<JsExprId>,
    pub value: JsExprId,
    pub dynamic_arg: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3DomEvent {
    pub name: String,
    pub dynamic_name: Option<JsExprId>,
    pub handler: JsStmtId,
    pub dynamic_arg: bool,
    pub cache: Option<Vue3DomEventCache>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3DomEventCache {
    pub index: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3DomObjectBinding {
    pub value: JsExprId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3DomObjectListeners {
    pub value: JsExprId,
    pub preserve_case: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3DomPropsNormalize {
    pub normalize_props: bool,
    pub guard_reactive_props: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3DomDirective {
    pub name: String,
    pub argument: Option<String>,
    pub dynamic_argument: Option<JsExprId>,
    pub expression: Option<JsExprId>,
    pub modifiers: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3RenderSlot {
    pub name: Vue3DomSlotName,
    pub props: Vue3DomProps,
    pub fallback: Vec<NodeId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vue3SsrMirKind {
    Root,
    PushString(String),
    PushInterpolated(MirExpr),
    RenderAttrs,
    RenderComponent {
        tag: MirExpr,
    },
    RenderSlot {
        name: Option<String>,
    },
    If {
        condition: Option<JsExprId>,
    },
    For {
        source: JsExprId,
        alias: JsPatternId,
    },
    Teleport,
    Suspense,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VaporMirKind {
    Root,
    Template(String),
    Effect { expression: JsExprId },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MirExpr {
    String(String),
    JsExpr(JsExprId),
    Helper(RuntimeHelper),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MirChildren {
    None,
    Text(String),
    Nodes(Vec<NodeId>),
    Slots(Vue3DomSlots),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3DomSlots {
    pub slots: Vec<Vue3DomSlot>,
    pub dynamic_slots: Vec<Vue3DomDynamicSlot>,
    pub flag: Vue3SlotFlag,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3DomSlot {
    pub name: String,
    pub params: Option<JsPatternId>,
    pub children: Vec<NodeId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vue3DomDynamicSlot {
    Slot(Vue3DomDynamicSlotObject),
    Conditional(Vue3DomConditionalSlot),
    For(Vue3DomForSlot),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3DomDynamicSlotObject {
    pub name: Vue3DomSlotName,
    pub params: Option<JsPatternId>,
    pub children: Vec<NodeId>,
    pub key: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3DomConditionalSlot {
    pub condition: Option<JsExprId>,
    pub slot: Vue3DomDynamicSlotObject,
    pub alternate: Option<Box<Vue3DomDynamicSlot>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3DomForSlot {
    pub source: JsExprId,
    pub value_alias: JsPatternId,
    pub key_alias: Option<JsPatternId>,
    pub index_alias: Option<JsPatternId>,
    pub slot: Vue3DomDynamicSlotObject,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vue3DomSlotName {
    Static(String),
    Dynamic(JsExprId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vue3SlotFlag {
    Stable,
    Dynamic,
    Forwarded,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3PatchFlags {
    pub bits: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mir {
    Vue2(Vue2Mir),
    Vue3Dom(Vue3DomMir),
    Vue3Ssr(Vue3SsrMir),
    Vapor(VaporMir),
}

impl Mir {
    pub fn target(&self) -> MirTarget {
        match self {
            Self::Vue2(_) => MirTarget::Vue2,
            Self::Vue3Dom(_) => MirTarget::Vue3Dom,
            Self::Vue3Ssr(_) => MirTarget::Vue3Ssr,
            Self::Vapor(_) => MirTarget::Vapor,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MirTarget {
    Vue2,
    Vue3Dom,
    Vue3Ssr,
    Vapor,
}

pub type Vue2Ast = AstDocument<Vue2NodeKind>;
pub type Vue3Ast = AstDocument<Vue3NodeKind>;
pub type Hir = AstDocument<HirNodeKind>;
pub type Vue2Mir = AstDocument<Vue2MirKind>;
pub type Vue3DomMir = AstDocument<Vue3DomMirKind>;
pub type Vue3SsrMir = AstDocument<Vue3SsrMirKind>;
pub type VaporMir = AstDocument<VaporMirKind>;
pub type HIR = Hir;
pub type MIR = Mir;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn documents_roundtrip_through_serde() {
        let doc = Vue2Ast::new(Vue2NodeKind::root(), None);
        let root = doc.root;
        let json = serde_json::to_string(&doc).unwrap();
        let decoded: Vue2Ast = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.root, root);
        assert_eq!(decoded.len(), 1);
    }

    #[test]
    fn distinct_kind_spaces_exist() {
        let mut vue3 = Vue3Ast::new(Vue3NodeKind::root(), None);
        let id = vue3.push_child(
            vue3.root,
            Vue3NodeKind::element(
                "div",
                vec![TemplateAttribute {
                    name: "id".into(),
                    value: Some("app".into()),
                }],
                false,
            ),
            None,
        );
        assert!(matches!(
            vue3.node(id).unwrap().kind,
            Vue3NodeKind::Element(_)
        ));
        let mut mir = Vue3DomMir::new(Vue3DomMirKind::Root, None);
        let _ = mir.push_child(
            mir.root,
            Vue3DomMirKind::TextCall {
                value: MirExpr::String("main".into()),
            },
            None,
        );
        assert_eq!(mir.len(), 2);
        assert_eq!(Mir::Vue3Dom(mir).target(), MirTarget::Vue3Dom);
    }

    #[test]
    fn attach_child_records_parent_and_index() {
        let mut doc = Vue3Ast::new(Vue3NodeKind::root(), None);
        let root = doc.root;
        let child = doc.push_child(root, Vue3NodeKind::text("hello"), None);
        assert_eq!(doc.node(child).and_then(|node| node.parent), Some(root));
        assert_eq!(doc.node(child).map(|node| node.index_in_parent), Some(0));
        assert_eq!(doc.validate_tree(), Ok(()));
    }

    #[test]
    fn reattach_child_refreshes_old_parent_indexes() {
        let mut doc = Vue3Ast::new(Vue3NodeKind::root(), None);
        let old_parent = doc.push_child(
            doc.root,
            Vue3NodeKind::element("section", Vec::new(), false),
            None,
        );
        let first = doc.push_child(old_parent, Vue3NodeKind::text("a"), None);
        let moved = doc.push_child(old_parent, Vue3NodeKind::text("b"), None);
        let third = doc.push_child(old_parent, Vue3NodeKind::text("c"), None);

        doc.attach_child(doc.root, moved);

        assert_eq!(doc.node(first).unwrap().index_in_parent, 0);
        assert_eq!(doc.node(third).unwrap().index_in_parent, 1);
        assert_eq!(doc.node(moved).unwrap().parent, Some(doc.root));
        assert_eq!(doc.validate_tree(), Ok(()));
    }

    #[test]
    fn runtime_helpers_are_orderable() {
        let mut helpers = BTreeSet::new();
        helpers.insert(RuntimeHelper::Vue3OpenBlock);
        helpers.insert(RuntimeHelper::Vue3CreateElementBlock);
        assert_eq!(helpers.len(), 2);
    }

    #[test]
    fn public_projection_is_nested_and_deterministic() {
        let mut doc = Vue3Ast::new(Vue3NodeKind::root(), None);
        let child = doc.push_child(
            doc.root,
            Vue3NodeKind::text("hello"),
            NodeSpan::generated(None, GeneratedReason::Lowering),
        );
        let projected = doc.project_public();
        assert!(matches!(projected.kind, Vue3NodeKind::Root(_)));
        assert_eq!(projected.children.len(), 1);
        assert_eq!(doc.node(child).unwrap().index_in_parent, 0);
        let json = serde_json::to_string(&projected).unwrap();
        assert!(json.contains("Generated"));
    }

    #[test]
    fn lowering_map_records_explicit_edges() {
        let mut map = LoweringMap::default();
        map.record_ast_to_hir(NodeId(1), NodeId(10));
        map.record_hir_to_mir(NodeId(10), NodeId(20));
        assert_eq!(
            map.hir_for_ast(NodeId(1)).collect::<Vec<_>>(),
            vec![NodeId(10)]
        );
        assert_eq!(
            map.mir_for_hir(NodeId(10)).collect::<Vec<_>>(),
            vec![NodeId(20)]
        );
    }

    #[test]
    fn hir_has_no_runtime_helper_or_codegen_call_variant() {
        let expression = JsExprId(0);
        let hir = HirNodeKind::Interpolation(HirInterpolation {
            expression: HirExpr::Js(expression),
        });
        assert!(matches!(hir, HirNodeKind::Interpolation(_)));
    }
}
