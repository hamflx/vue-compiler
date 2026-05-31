//! Canonical AST, HIR, and target-split MIR data structures.
//!
//! This crate is the structural authority for compiler documents. Internal
//! trees use [`AstDocument`] arenas with stable [`NodeId`] handles, public
//! projection is explicit, and MIR is split by output target instead of using a
//! single generic runtime-call IR.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use vuec_source::{FileId, Span};

/// Stable arena node identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct NodeId(pub u32);

/// A single arena node in an [`AstDocument`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Node<K> {
    /// Stable id equal to the node's arena index.
    pub id: NodeId,
    /// Dialect-specific node payload.
    pub kind: K,
    /// Source, generated, or missing span metadata.
    pub span: NodeSpan,
    /// Parent node id, absent only for the document root.
    pub parent: Option<NodeId>,
    /// Child node ids in source/tree order.
    pub children: Vec<NodeId>,
    /// Position inside the parent's child list.
    pub index_in_parent: u32,
}

/// Arena-backed compiler document.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AstDocument<K> {
    /// Required root node id.
    pub root: NodeId,
    /// Contiguous node arena.
    pub nodes: Vec<Node<K>>,
}

/// Estimates a node capacity hint for a Vue template using default delimiters.
pub fn template_node_capacity_hint(source: &str) -> usize {
    template_node_capacity_hint_with_interpolation(source, "{{")
}

/// Estimates a node capacity hint for a Vue template with custom interpolation.
pub fn template_node_capacity_hint_with_interpolation(
    source: &str,
    interpolation_open: &str,
) -> usize {
    let bytes = source.as_bytes();
    let open = interpolation_open.as_bytes();
    let mut nodes = 1usize;
    let mut index = 0usize;
    let mut in_tag = false;
    let mut text_has_content = false;

    while index < bytes.len() {
        if !in_tag && !open.is_empty() && bytes[index..].starts_with(open) {
            if text_has_content {
                nodes += 1;
                text_has_content = false;
            }
            nodes += 1;
            index += open.len();
            continue;
        }

        match bytes[index] {
            b'<' if !in_tag => {
                if text_has_content {
                    nodes += 1;
                    text_has_content = false;
                }
                if template_open_bracket_starts_node(bytes, index) {
                    nodes += 1;
                }
                in_tag = true;
            }
            b'>' if in_tag => {
                in_tag = false;
            }
            byte if !in_tag && !byte.is_ascii_whitespace() => {
                text_has_content = true;
            }
            _ => {}
        }
        index += 1;
    }

    if text_has_content {
        nodes += 1;
    }

    nodes + nodes / 4 + 4
}

fn template_open_bracket_starts_node(bytes: &[u8], index: usize) -> bool {
    match bytes.get(index + 1).copied() {
        Some(b'/') | None => false,
        Some(byte) => byte.is_ascii_alphabetic() || matches!(byte, b'!' | b'?'),
    }
}

impl<K> AstDocument<K> {
    /// Creates a document with a root node.
    pub fn new<S>(root_kind: K, span: S) -> Self
    where
        S: Into<NodeSpan>,
    {
        Self::with_capacity(root_kind, span, 1)
    }

    /// Creates a document with at least `node_capacity` node slots reserved.
    pub fn with_capacity<S>(root_kind: K, span: S, node_capacity: usize) -> Self
    where
        S: Into<NodeSpan>,
    {
        let mut document = Self {
            root: NodeId(0),
            nodes: Vec::with_capacity(node_capacity.max(1)),
        };
        let root = document.push(root_kind, span);
        document.root = root;
        document
    }

    /// Pushes a detached node into the arena.
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

    /// Pushes a node and attaches it as the last child of `parent`.
    pub fn push_child<S>(&mut self, parent: NodeId, kind: K, span: S) -> NodeId
    where
        S: Into<NodeSpan>,
    {
        let child = self.push(kind, span);
        self.attach_child(parent, child);
        child
    }

    /// Attaches an existing node as the last child of `parent`.
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

    /// Replaces the full child list of `parent`.
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

    /// Sets the document root node.
    pub fn set_root(&mut self, id: NodeId) {
        self.root = id;
        if let Some(root_node) = self.node_mut(id) {
            root_node.parent = None;
            root_node.index_in_parent = 0;
        }
    }

    /// Returns a node by id.
    pub fn node(&self, id: NodeId) -> Option<&Node<K>> {
        self.nodes.get(id.0 as usize)
    }

    /// Returns a mutable node by id.
    pub fn node_mut(&mut self, id: NodeId) -> Option<&mut Node<K>> {
        self.nodes.get_mut(id.0 as usize)
    }

    /// Returns the number of nodes in the arena.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Returns the current node arena capacity.
    pub fn node_capacity(&self) -> usize {
        self.nodes.capacity()
    }

    /// Reserves additional node slots.
    pub fn reserve_nodes(&mut self, additional: usize) {
        self.nodes.reserve(additional);
    }

    /// Returns whether the arena has no nodes.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Returns the root node.
    pub fn root_node(&self) -> Option<&Node<K>> {
        self.node(self.root)
    }

    /// Returns the mutable root node.
    pub fn root_node_mut(&mut self) -> Option<&mut Node<K>> {
        self.node_mut(self.root)
    }

    /// Validates parent, child, root, and node-id invariants.
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

    /// Walks the document in deterministic depth-first pre-order.
    pub fn visit<V>(&self, visitor: &mut V) -> VisitControl
    where
        V: AstVisitor<K>,
    {
        self.visit_node(self.root, visitor)
    }

    fn visit_node<V>(&self, id: NodeId, visitor: &mut V) -> VisitControl
    where
        V: AstVisitor<K>,
    {
        let Some(node) = self.node(id) else {
            return VisitControl::Continue;
        };
        let children = node.children.clone();
        match visitor.enter_node(self, node) {
            VisitControl::Stop => return VisitControl::Stop,
            VisitControl::SkipChildren => {}
            VisitControl::Continue => {
                for child in children {
                    if self.visit_node(child, visitor) == VisitControl::Stop {
                        return VisitControl::Stop;
                    }
                }
            }
        }
        let Some(node) = self.node(id) else {
            return VisitControl::Continue;
        };
        visitor.exit_node(self, node)
    }

    /// Walks the document mutably in deterministic depth-first pre-order.
    pub fn visit_mut<V>(&mut self, visitor: &mut V) -> VisitControl
    where
        V: AstVisitorMut<K>,
    {
        self.visit_node_mut(self.root, visitor)
    }

    fn visit_node_mut<V>(&mut self, id: NodeId, visitor: &mut V) -> VisitControl
    where
        V: AstVisitorMut<K>,
    {
        let control = {
            let Some(node) = self.node_mut(id) else {
                return VisitControl::Continue;
            };
            visitor.enter_node_mut(node)
        };
        match control {
            VisitControl::Stop => return VisitControl::Stop,
            VisitControl::SkipChildren => {}
            VisitControl::Continue => {
                let children = self
                    .node(id)
                    .map(|node| node.children.clone())
                    .unwrap_or_default();
                for child in children {
                    if self.visit_node_mut(child, visitor) == VisitControl::Stop {
                        return VisitControl::Stop;
                    }
                }
            }
        }
        let Some(node) = self.node_mut(id) else {
            return VisitControl::Continue;
        };
        visitor.exit_node_mut(node)
    }

    /// Validates tree invariants and all declared node span metadata.
    pub fn validate_span_consistency(&self) -> Result<(), SpanConsistencyError>
    where
        K: SpanMetadata,
    {
        self.validate_tree().map_err(SpanConsistencyError::Tree)?;
        for node in &self.nodes {
            validate_node_span(node.id, "node", &node.span)?;
            let mut spans = Vec::new();
            node.kind.collect_extra_spans(&mut spans);
            for extra in spans {
                validate_node_span(node.id, &extra.owner, &extra.span)?;
            }
        }
        Ok(())
    }
}

/// Controls traversal after visitor callbacks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VisitControl {
    /// Continue normal traversal.
    Continue,
    /// Skip the current node's children but still run its exit callback.
    SkipChildren,
    /// Stop the traversal immediately.
    Stop,
}

/// Immutable AST/HIR/MIR arena visitor.
pub trait AstVisitor<K> {
    /// Called before visiting a node's children.
    fn enter_node(&mut self, _document: &AstDocument<K>, _node: &Node<K>) -> VisitControl {
        VisitControl::Continue
    }

    /// Called after visiting a node's children.
    fn exit_node(&mut self, _document: &AstDocument<K>, _node: &Node<K>) -> VisitControl {
        VisitControl::Continue
    }
}

/// Mutable AST/HIR/MIR arena visitor.
pub trait AstVisitorMut<K> {
    /// Called before visiting a node's children.
    fn enter_node_mut(&mut self, _node: &mut Node<K>) -> VisitControl {
        VisitControl::Continue
    }

    /// Called after visiting a node's children.
    fn exit_node_mut(&mut self, _node: &mut Node<K>) -> VisitControl {
        VisitControl::Continue
    }
}

/// Deterministic document snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AstSnapshot<K> {
    /// Root node id.
    pub root: NodeId,
    /// Arena nodes in stable id order.
    pub nodes: Vec<AstNodeSnapshot<K>>,
}

/// Deterministic node snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AstNodeSnapshot<K> {
    /// Node id.
    pub id: NodeId,
    /// Parent id.
    pub parent: Option<NodeId>,
    /// Position inside the parent.
    pub index_in_parent: u32,
    /// Child ids in source/tree order.
    pub children: Vec<NodeId>,
    /// Source, generated, or missing span metadata.
    pub span: NodeSpan,
    /// Dialect-specific node payload.
    pub kind: K,
}

impl<K> AstDocument<K>
where
    K: Clone,
{
    /// Creates a deterministic snapshot of this document.
    pub fn snapshot(&self) -> AstSnapshot<K> {
        AstSnapshot {
            root: self.root,
            nodes: self
                .nodes
                .iter()
                .map(|node| AstNodeSnapshot {
                    id: node.id,
                    parent: node.parent,
                    index_in_parent: node.index_in_parent,
                    children: node.children.clone(),
                    span: node.span.clone(),
                    kind: node.kind.clone(),
                })
                .collect(),
        }
    }
}

impl<K> AstDocument<K>
where
    K: Clone + Serialize,
{
    /// Serializes the deterministic snapshot as compact JSON.
    pub fn snapshot_json(&self) -> serde_json::Result<String> {
        serde_json::to_string(&self.snapshot())
    }

    /// Serializes the deterministic snapshot as pretty JSON.
    pub fn snapshot_json_pretty(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(&self.snapshot())
    }
}

impl NodeSpan {
    /// Returns the source span if one is available.
    pub fn source(&self) -> Option<Span> {
        match self {
            NodeSpan::Source(span) => Some(*span),
            NodeSpan::Generated { origin, .. } => *origin,
            NodeSpan::Missing { .. } => None,
        }
    }

    /// Returns a mutable source span if one is available.
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

/// JavaScript side-store id types used by AST/HIR/MIR nodes.
pub mod ids {
    use serde::{Deserialize, Serialize};

    /// Identifier for a registered JavaScript expression.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
    pub struct JsExprId(pub u32);

    /// Identifier for a registered JavaScript statement.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
    pub struct JsStmtId(pub u32);

    /// Identifier for a registered JavaScript pattern.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
    pub struct JsPatternId(pub u32);

    /// Identifier for a registered JavaScript program.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
    pub struct JsProgramId(pub u32);
}

/// Re-exported JavaScript side-store ids.
pub use ids::{JsExprId, JsPatternId, JsProgramId, JsStmtId};

/// Reason a node span was generated rather than parsed directly from source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GeneratedReason {
    /// Span was introduced while recovering from parse errors.
    ParseRecovery,
    /// Span was introduced during AST-to-HIR or HIR-to-MIR lowering.
    Lowering,
    /// Span was introduced by code generation metadata.
    Codegen,
}

/// Reason source span metadata is missing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MissingSpanReason {
    /// Source was unavailable because of parse recovery.
    ParseRecovery,
    /// Source was unavailable at a lowering boundary.
    LoweringGap,
    /// Node was synthetic and has no source origin.
    Synthetic,
}

/// Span metadata carried by every arena node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeSpan {
    /// Node was parsed directly from this source span.
    Source(Span),
    /// Node was generated, optionally from an original source span.
    Generated {
        /// Optional source origin for generated content.
        origin: Option<Span>,
        /// Reason the span was generated.
        reason: GeneratedReason,
    },
    /// Node intentionally has no source span.
    Missing {
        /// Reason the span is missing.
        reason: MissingSpanReason,
    },
}

/// Tree invariant validation failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AstInvariantError {
    /// The root id does not reference an arena node.
    MissingRoot {
        /// Invalid root id.
        root: NodeId,
    },
    /// A node id does not match its arena index.
    MismatchedNodeId {
        /// Expected id for this arena position.
        expected: NodeId,
        /// Actual id stored in the node.
        actual: NodeId,
    },
    /// The root has parent or index metadata.
    InvalidRootMetadata {
        /// Root node with invalid metadata.
        root: NodeId,
    },
    /// A non-root node has no parent.
    DetachedNode {
        /// Detached node id.
        node: NodeId,
    },
    /// A child reference does not point to an arena node.
    MissingChild {
        /// Parent containing the child reference.
        parent: NodeId,
        /// Missing child id.
        child: NodeId,
    },
    /// Child parent/index metadata does not match its parent list position.
    InvalidChildMetadata {
        /// Parent containing the child reference.
        parent: NodeId,
        /// Child with mismatched metadata.
        child: NodeId,
        /// Expected index inside the parent child list.
        expected_index: u32,
    },
}

impl NodeSpan {
    /// Creates generated span metadata.
    pub fn generated(origin: Option<Span>, reason: GeneratedReason) -> Self {
        Self::Generated { origin, reason }
    }

    /// Creates missing span metadata.
    pub fn missing(reason: MissingSpanReason) -> Self {
        Self::Missing { reason }
    }
}

/// One additional span field owned by a node payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtraSpan {
    /// Human-readable field owner for diagnostics and snapshots.
    pub owner: String,
    /// Extra span metadata.
    pub span: NodeSpan,
}

impl ExtraSpan {
    /// Creates an extra span entry.
    pub fn new(owner: impl Into<String>, span: impl Into<NodeSpan>) -> Self {
        Self {
            owner: owner.into(),
            span: span.into(),
        }
    }
}

/// Trait implemented by node payloads that own nested source span metadata.
pub trait SpanMetadata {
    /// Appends extra spans that are not the arena node's primary span.
    fn collect_extra_spans(&self, _spans: &mut Vec<ExtraSpan>) {}
}

impl<T> SpanMetadata for Box<T>
where
    T: SpanMetadata,
{
    fn collect_extra_spans(&self, spans: &mut Vec<ExtraSpan>) {
        self.as_ref().collect_extra_spans(spans);
    }
}

/// Span consistency validation failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpanConsistencyError {
    /// Underlying tree invariants are invalid.
    Tree(AstInvariantError),
    /// A source span has `end < start`.
    InvalidSourceRange {
        /// Node owning the span.
        node: NodeId,
        /// Field that owns the span.
        owner: String,
        /// Invalid span.
        span: Span,
    },
}

fn validate_node_span(
    node: NodeId,
    owner: &str,
    span: &NodeSpan,
) -> Result<(), SpanConsistencyError> {
    match span {
        NodeSpan::Source(source) => validate_source_span(node, owner, *source),
        NodeSpan::Generated { origin, .. } => {
            if let Some(origin) = origin {
                validate_source_span(node, owner, *origin)?;
            }
            Ok(())
        }
        NodeSpan::Missing { .. } => Ok(()),
    }
}

fn validate_source_span(node: NodeId, owner: &str, span: Span) -> Result<(), SpanConsistencyError> {
    if span.end.0 < span.start.0 {
        return Err(SpanConsistencyError::InvalidSourceRange {
            node,
            owner: owner.into(),
            span,
        });
    }
    Ok(())
}

/// Mapping edges recorded during AST to HIR and HIR to MIR lowering.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoweringMap {
    /// Recorded AST node to HIR node edges.
    pub ast_to_hir: Vec<(NodeId, NodeId)>,
    /// Recorded HIR node to MIR node edges.
    pub hir_to_mir: Vec<(NodeId, NodeId)>,
}

impl LoweringMap {
    /// Records an AST to HIR lowering edge.
    pub fn record_ast_to_hir(&mut self, ast: NodeId, hir: NodeId) {
        self.ast_to_hir.push((ast, hir));
    }

    /// Records a HIR to MIR lowering edge.
    pub fn record_hir_to_mir(&mut self, hir: NodeId, mir: NodeId) {
        self.hir_to_mir.push((hir, mir));
    }

    /// Returns HIR nodes lowered from an AST node.
    pub fn hir_for_ast(&self, ast: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        self.ast_to_hir
            .iter()
            .filter_map(move |(from, to)| (*from == ast).then_some(*to))
    }

    /// Returns MIR nodes lowered from a HIR node.
    pub fn mir_for_hir(&self, hir: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        self.hir_to_mir
            .iter()
            .filter_map(move |(from, to)| (*from == hir).then_some(*to))
    }
}

/// Produces the deterministic public projection of an internal structure.
pub trait PublicProjection {
    /// Public projection result type.
    type Output;

    /// Projects the internal structure to its public representation.
    fn project_public(&self) -> Self::Output;
}

/// Nested public tree node produced from an arena document.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicNode<K> {
    /// Projected node payload.
    pub kind: K,
    /// Projected node span.
    pub span: NodeSpan,
    /// Projected child nodes.
    pub children: Vec<PublicNode<K>>,
}

impl<K> AstDocument<K>
where
    K: Clone,
{
    /// Projects the arena tree into a nested public tree.
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

/// Runtime helper symbols referenced by transforms and target MIR.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RuntimeHelper {
    /// Vue 2 `_c` / create element helper.
    Vue2CreateElement,
    /// Vue 2 create text VNode helper.
    Vue2CreateTextVNode,
    /// Vue 2 stringify helper.
    Vue2ToString,
    /// Vue 2 render-list helper.
    Vue2RenderList,
    /// Vue 2 filter resolver helper.
    Vue2ResolveFilter,
    /// Vue 3 directive resolver helper.
    Vue3ResolveDirective,
    /// Vue 3 `withDirectives` helper.
    Vue3WithDirectives,
    /// Vue 3 block tracking helper.
    Vue3SetBlockTracking,
    /// Vue 3 `openBlock` helper.
    Vue3OpenBlock,
    /// Vue 3 element VNode helper.
    Vue3CreateElementVNode,
    /// Vue 3 element block helper.
    Vue3CreateElementBlock,
    /// Vue 3 comment VNode helper.
    Vue3CreateCommentVNode,
    /// Vue 3 text VNode helper.
    Vue3CreateTextVNode,
    /// Vue 3 Fragment symbol helper.
    Vue3Fragment,
    /// Vue 3 display string helper.
    Vue3ToDisplayString,
    /// Vue 3 render-list helper.
    Vue3RenderList,
    /// Vue 3 render-slot helper.
    Vue3RenderSlot,
    /// Vue 3 class normalizer helper.
    Vue3NormalizeClass,
    /// Vue 3 props normalizer helper.
    Vue3NormalizeProps,
    /// Vue 3 style normalizer helper.
    Vue3NormalizeStyle,
    /// Vue 3 reactive props guard helper.
    Vue3GuardReactiveProps,
    /// Vue 3 merge props helper.
    Vue3MergeProps,
    /// Vue 3 component resolver helper.
    Vue3ResolveComponent,
    /// Vue 3 dynamic component resolver helper.
    Vue3ResolveDynamicComponent,
    /// Vue 3 base transition helper.
    Vue3BaseTransition,
    /// Vue 3 Transition component helper.
    Vue3Transition,
    /// Vue 3 TransitionGroup component helper.
    Vue3TransitionGroup,
    /// Vue 3 Teleport component helper.
    Vue3Teleport,
    /// Vue 3 Suspense component helper.
    Vue3Suspense,
    /// Vue 3 KeepAlive component helper.
    Vue3KeepAlive,
    /// Vue 3 `withCtx` helper.
    Vue3WithCtx,
    /// Vue 3 create block helper.
    Vue3CreateBlock,
    /// Vue 3 create VNode helper.
    Vue3CreateVNode,
    /// Vue 3 dynamic slots helper.
    Vue3CreateSlots,
    /// Vue 3 static VNode helper.
    Vue3CreateStaticVNode,
    /// Vue 3 memo comparison helper.
    Vue3IsMemoSame,
    /// Vue 3 memo helper.
    Vue3WithMemo,
    /// Vue 3 object listeners helper.
    Vue3ToHandlers,
    /// Vue 3 camelize helper.
    Vue3Camelize,
    /// Vue 3 capitalize helper.
    Vue3Capitalize,
    /// Vue 3 event handler key helper.
    Vue3ToHandlerKey,
    /// Vue 3 scope id push helper.
    Vue3PushScopeId,
    /// Vue 3 scope id pop helper.
    Vue3PopScopeId,
    /// Vue 3 unref helper.
    Vue3Unref,
    /// Vue 3 ref test helper.
    Vue3IsRef,
    /// Vue 3 radio model runtime directive helper.
    Vue3VModelRadio,
    /// Vue 3 checkbox model runtime directive helper.
    Vue3VModelCheckbox,
    /// Vue 3 text model runtime directive helper.
    Vue3VModelText,
    /// Vue 3 select model runtime directive helper.
    Vue3VModelSelect,
    /// Vue 3 dynamic model runtime directive helper.
    Vue3VModelDynamic,
    /// Vue 3 event modifier helper.
    Vue3WithModifiers,
    /// Vue 3 key modifier helper.
    Vue3WithKeys,
    /// Vue 3 show runtime directive helper.
    Vue3VShow,
    /// Vue 3 SSR interpolation helper.
    Vue3SsrInterpolate,
    /// Vue 3 SSR VNode render helper.
    Vue3SsrRenderVNode,
    /// Vue 3 SSR component render helper.
    Vue3SsrRenderComponent,
    /// Vue 3 SSR slot render helper.
    Vue3SsrRenderSlot,
    /// Vue 3 SSR inner slot render helper.
    Vue3SsrRenderSlotInner,
    /// Vue 3 SSR class render helper.
    Vue3SsrRenderClass,
    /// Vue 3 SSR style render helper.
    Vue3SsrRenderStyle,
    /// Vue 3 SSR attrs render helper.
    Vue3SsrRenderAttrs,
    /// Vue 3 SSR attr render helper.
    Vue3SsrRenderAttr,
    /// Vue 3 SSR dynamic attr render helper.
    Vue3SsrRenderDynamicAttr,
    /// Vue 3 SSR render-list helper.
    Vue3SsrRenderList,
    /// Vue 3 SSR boolean attr inclusion helper.
    Vue3SsrIncludeBooleanAttr,
    /// Vue 3 SSR loose equality helper.
    Vue3SsrLooseEqual,
    /// Vue 3 SSR loose containment helper.
    Vue3SsrLooseContain,
    /// Vue 3 SSR dynamic model render helper.
    Vue3SsrRenderDynamicModel,
    /// Vue 3 SSR dynamic model props helper.
    Vue3SsrGetDynamicModelProps,
    /// Vue 3 SSR teleport render helper.
    Vue3SsrRenderTeleport,
    /// Vue 3 SSR suspense render helper.
    Vue3SsrRenderSuspense,
    /// Vue 3 SSR directive props helper.
    Vue3SsrGetDirectiveProps,
}

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
    Element(Vue2Element),
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
        Self::Element(Vue2Element::new(tag))
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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3Root {
    /// Source file id for this template, when known.
    pub source_id: Option<FileId>,
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

impl Default for Vue3Root {
    fn default() -> Self {
        Self {
            source_id: None,
            helpers: BTreeSet::new(),
            components: BTreeSet::new(),
            directives: BTreeSet::new(),
            imports: Vec::new(),
            hoists: Vec::new(),
            temps: 0,
            cached: 0,
            codegen_node: None,
        }
    }
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

/// Vue 2 target-specific MIR node kinds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vue2MirKind {
    /// Vue 2 MIR root node.
    Root(Vue2MirRoot),
    /// Create-element call.
    CreateElement(Vue2CreateElement),
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
    fn document_capacity_can_be_reserved_without_changing_tree_shape() {
        let mut doc = Vue3Ast::with_capacity(Vue3NodeKind::root(), None, 16);
        assert_eq!(doc.root, NodeId(0));
        assert_eq!(doc.len(), 1);
        assert!(doc.node_capacity() >= 16);
        doc.reserve_nodes(32);
        assert!(doc.node_capacity() >= 33);

        let child = doc.push_child(doc.root, Vue3NodeKind::text("hello"), None);
        assert_eq!(child, NodeId(1));
        assert_eq!(doc.validate_tree(), Ok(()));
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
        let mut mir = Vue3DomMir::new(Vue3DomMirKind::Root(Vue3DomRoot::default()), None);
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

    #[test]
    fn visitor_reports_stable_enter_exit_order() {
        #[derive(Default)]
        struct Recorder {
            events: Vec<String>,
        }

        impl AstVisitor<Vue3NodeKind> for Recorder {
            fn enter_node(
                &mut self,
                _document: &AstDocument<Vue3NodeKind>,
                node: &Node<Vue3NodeKind>,
            ) -> VisitControl {
                self.events.push(format!("enter:{}", node.id.0));
                VisitControl::Continue
            }

            fn exit_node(
                &mut self,
                _document: &AstDocument<Vue3NodeKind>,
                node: &Node<Vue3NodeKind>,
            ) -> VisitControl {
                self.events.push(format!("exit:{}", node.id.0));
                VisitControl::Continue
            }
        }

        let mut doc = Vue3Ast::new(Vue3NodeKind::root(), Span::new(FileId(0), 0, 10));
        let element = doc.push_child(
            doc.root,
            Vue3NodeKind::element("div", Vec::new(), false),
            Span::new(FileId(0), 0, 10),
        );
        doc.push_child(
            element,
            Vue3NodeKind::text("hello"),
            Span::new(FileId(0), 5, 10),
        );

        let mut recorder = Recorder::default();
        assert_eq!(doc.visit(&mut recorder), VisitControl::Continue);
        assert_eq!(
            recorder.events,
            vec!["enter:0", "enter:1", "enter:2", "exit:2", "exit:1", "exit:0"]
        );
    }

    #[test]
    fn mutable_visitor_can_update_payloads_without_changing_tree_shape() {
        struct UppercaseText;

        impl AstVisitorMut<Vue3NodeKind> for UppercaseText {
            fn enter_node_mut(&mut self, node: &mut Node<Vue3NodeKind>) -> VisitControl {
                if let Vue3NodeKind::Text(text) = &mut node.kind {
                    text.value.make_ascii_uppercase();
                }
                VisitControl::Continue
            }
        }

        let mut doc = Vue3Ast::new(Vue3NodeKind::root(), Span::new(FileId(0), 0, 5));
        let text = doc.push_child(
            doc.root,
            Vue3NodeKind::text("hello"),
            Span::new(FileId(0), 0, 5),
        );

        assert_eq!(doc.visit_mut(&mut UppercaseText), VisitControl::Continue);
        assert_eq!(doc.validate_tree(), Ok(()));
        assert!(matches!(
            &doc.node(text).unwrap().kind,
            Vue3NodeKind::Text(value) if value.value == "HELLO"
        ));
    }

    #[test]
    fn snapshot_json_preserves_generated_and_missing_span_reasons() {
        let mut doc = Vue3Ast::new(
            Vue3NodeKind::root(),
            NodeSpan::generated(Some(Span::new(FileId(0), 0, 1)), GeneratedReason::Lowering),
        );
        doc.push_child(
            doc.root,
            Vue3NodeKind::text("fallback"),
            NodeSpan::missing(MissingSpanReason::ParseRecovery),
        );

        let json = doc.snapshot_json().expect("snapshot json");
        assert!(json.contains("Generated"));
        assert!(json.contains("Lowering"));
        assert!(json.contains("Missing"));
        assert!(json.contains("ParseRecovery"));
    }

    #[test]
    fn span_consistency_checks_node_and_nested_spans() {
        let mut doc = Vue3Ast::new(Vue3NodeKind::root(), Span::new(FileId(0), 0, 20));
        doc.push_child(
            doc.root,
            Vue3AstKind::Element(Vue3Element {
                tag: "div".into(),
                tag_type: Vue3ElementType::Element,
                ns: HtmlNamespace::Html,
                props: vec![
                    Vue3Prop::Attribute(Vue3Attribute {
                        name: "id".into(),
                        value: Some("a".into()),
                        span: Some(Span::new(FileId(0), 5, 11)),
                        name_span: Some(Span::new(FileId(0), 5, 7)),
                        value_span: Some(Span::new(FileId(0), 9, 10)),
                        quote: Some(QuoteKind::Double),
                    }),
                    Vue3Prop::Directive(Vue3Directive {
                        name: "bind".into(),
                        raw_name: ":class.mod".into(),
                        arg: Some(Vue3Expression::Raw("class".into())),
                        exp: Some(Vue3Expression::Raw("klass".into())),
                        modifiers: vec!["mod".into()],
                        is_dynamic_arg: false,
                        span: Some(Span::new(FileId(0), 12, 30)),
                        arg_span: Some(Span::new(FileId(0), 13, 18)),
                        exp_span: Some(Span::new(FileId(0), 24, 29)),
                        modifier_spans: vec![NodeSpan::Source(Span::new(FileId(0), 19, 22))],
                    }),
                ],
                self_closing: false,
                codegen_node: None,
                ssr_codegen_node: None,
            }),
            Span::new(FileId(0), 0, 30),
        );

        assert_eq!(doc.validate_span_consistency(), Ok(()));
    }

    #[test]
    fn vue2_ast_schema_keeps_source_ranges_and_filter_structure_out_of_children() {
        let mut element = Vue2Element::new("li");
        element.if_exp = Some(JsExprId(0));
        element.if_span = Some(Span::new(FileId(0), 1, 10));
        element.for_exp = Some(JsExprId(1));
        element.for_span = Some(Span::new(FileId(0), 11, 25));
        element.alias = Some(JsPatternId(0));
        element.key = Some(JsExprId(2));
        element.key_span = Some(Span::new(FileId(0), 26, 35));
        element.attrs_list.push(Vue2Attribute {
            name: ":title".into(),
            value: "title".into(),
            span: Some(Span::new(FileId(0), 36, 50)),
            dynamic: true,
        });

        let mut doc = Vue2Ast::new(
            Vue2AstKind::Root(Vue2Root::default()),
            Span::new(FileId(0), 0, 60),
        );
        let element_id = doc.push_child(
            doc.root,
            Vue2AstKind::Element(element),
            Span::new(FileId(0), 0, 60),
        );
        let text_id = doc.push_child(
            element_id,
            Vue2AstKind::ExpressionText(Vue2ExpressionText {
                raw: "msg | cap".into(),
                expr: None,
                filter_expr: Some(Vue2FilterExpr {
                    raw: "msg | cap".into(),
                    base: JsExprId(3),
                    filters: vec![Vue2FilterCall {
                        name: "cap".into(),
                        args: Vec::new(),
                    }],
                }),
            }),
            Span::new(FileId(0), 40, 52),
        );

        assert_eq!(doc.validate_span_consistency(), Ok(()));
        assert_eq!(doc.node(element_id).unwrap().children, vec![text_id]);
        let Vue2AstKind::Element(projected) = &doc.node(element_id).unwrap().kind else {
            panic!("expected element");
        };
        assert_eq!(projected.for_span, Some(Span::new(FileId(0), 11, 25)));
        assert_eq!(
            projected.attrs_list[0].span,
            Some(Span::new(FileId(0), 36, 50))
        );
    }
}
