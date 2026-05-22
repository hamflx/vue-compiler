#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use vuec_source::Span;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct NodeId(pub u32);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Node<K> {
    pub id: NodeId,
    pub kind: K,
    pub span: Option<Span>,
    pub children: Vec<NodeId>,
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

    pub fn push(&mut self, kind: K, span: Option<Span>) -> NodeId {
        let id = NodeId(self.nodes.len() as u32);
        self.nodes.push(Node {
            id,
            kind,
            span,
            children: Vec::new(),
        });
        id
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
pub type HIR = AstDocument<HirNodeKind>;
pub type MIR = AstDocument<MirNodeKind>;

#[cfg(test)]
mod tests {
    use super::*;

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
}
