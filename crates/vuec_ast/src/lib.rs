#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use vuec_source::Span;

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
    pub root: Option<NodeId>,
    pub nodes: Vec<Node<K>>,
}

impl<K> AstDocument<K> {
    pub fn new() -> Self {
        Self {
            root: None,
            nodes: Vec::new(),
        }
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
        let index_in_parent = self
            .node(parent)
            .map(|node| node.children.len() as u32)
            .unwrap_or_default();
        if let Some(parent_node) = self.node_mut(parent) {
            parent_node.children.push(child);
        }
        if let Some(child_node) = self.node_mut(child) {
            child_node.parent = Some(parent);
            child_node.index_in_parent = index_in_parent;
        }
    }

    pub fn replace_children(&mut self, parent: NodeId, children: Vec<NodeId>) {
        if let Some(parent_node) = self.node_mut(parent) {
            parent_node.children = children.clone();
        }
        for (index, child) in children.into_iter().enumerate() {
            if let Some(child_node) = self.node_mut(child) {
                child_node.parent = Some(parent);
                child_node.index_in_parent = index as u32;
            }
        }
    }

    pub fn set_root(&mut self, id: NodeId) {
        self.root = Some(id);
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
}

impl<K> Default for AstDocument<K> {
    fn default() -> Self {
        Self::new()
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

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoweringMap {
    pub ast_to_hir: Vec<(NodeId, NodeId)>,
    pub hir_to_mir: Vec<(NodeId, NodeId)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RuntimeHelper {
    Vue2CreateElement,
    Vue2CreateTextVNode,
    Vue2ToString,
    Vue2RenderList,
    Vue2ResolveFilter,
    Vue3OpenBlock,
    Vue3CreateElementVNode,
    Vue3CreateElementBlock,
    Vue3ToDisplayString,
    Vue3RenderList,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vue2NodeKind {
    Root,
    Element {
        tag: String,
    },
    Text {
        value: String,
    },
    Interpolation {
        expression: String,
    },
    Comment {
        value: String,
    },
    Directive {
        name: String,
        expression: Option<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateAttribute {
    pub name: String,
    pub value: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vue3NodeKind {
    Root,
    Element {
        tag: String,
        attributes: Vec<TemplateAttribute>,
        self_closing: bool,
    },
    Text {
        value: String,
    },
    Interpolation {
        expression: String,
    },
    Comment {
        value: String,
    },
    Directive {
        name: String,
        expression: Option<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HirNodeKind {
    Root,
    Element { tag: String },
    Text { value: String },
    Interpolation { expression: String },
    Call { callee: String },
    Helper { name: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MirNodeKind {
    Root,
    RenderChunk { name: String },
    Element { tag: String },
    Text { value: String },
    Interpolation { expression: String },
    StaticFragment { index: u32 },
}

pub type Vue2Ast = AstDocument<Vue2NodeKind>;
pub type Vue3Ast = AstDocument<Vue3NodeKind>;
pub type Hir = AstDocument<HirNodeKind>;
pub type Mir = AstDocument<MirNodeKind>;
pub type HIR = Hir;
pub type MIR = Mir;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn documents_roundtrip_through_serde() {
        let mut doc = Vue2Ast::new();
        let root = doc.push(Vue2NodeKind::Root, None);
        doc.set_root(root);
        let json = serde_json::to_string(&doc).unwrap();
        let decoded: Vue2Ast = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.root, Some(root));
        assert_eq!(decoded.len(), 1);
    }

    #[test]
    fn distinct_kind_spaces_exist() {
        let mut vue3 = Vue3Ast::new();
        let id = vue3.push(
            Vue3NodeKind::Element {
                tag: "div".into(),
                attributes: vec![TemplateAttribute {
                    name: "id".into(),
                    value: Some("app".into()),
                }],
                self_closing: false,
            },
            None,
        );
        assert!(matches!(
            vue3.node(id).unwrap().kind,
            Vue3NodeKind::Element { .. }
        ));
        let mut mir = MIR::new();
        let _ = mir.push(
            MirNodeKind::RenderChunk {
                name: "main".into(),
            },
            None,
        );
        assert_eq!(mir.len(), 1);
    }

    #[test]
    fn attach_child_records_parent_and_index() {
        let mut doc = Vue3Ast::new();
        let root = doc.push(Vue3NodeKind::Root, None);
        let child = doc.push_child(
            root,
            Vue3NodeKind::Text {
                value: "hello".into(),
            },
            None,
        );
        assert_eq!(doc.node(child).and_then(|node| node.parent), Some(root));
        assert_eq!(doc.node(child).map(|node| node.index_in_parent), Some(0));
    }

    #[test]
    fn runtime_helpers_are_orderable() {
        let mut helpers = BTreeSet::new();
        helpers.insert(RuntimeHelper::Vue3OpenBlock);
        helpers.insert(RuntimeHelper::Vue3CreateElementBlock);
        assert_eq!(helpers.len(), 2);
    }
}
