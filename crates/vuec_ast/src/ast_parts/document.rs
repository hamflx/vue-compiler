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
    /// Parent node id, absent for the document root and detached nodes.
    pub parent: Option<NodeId>,
    /// Child node ids in source/tree order.
    pub children: Vec<NodeId>,
    /// Position inside the parent's child list, or zero when detached.
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
        if parent == child || child == self.root {
            return;
        }
        if self.node(parent).is_none() || self.node(child).is_none() {
            return;
        }
        if self.would_create_cycle(parent, child) {
            return;
        }
        if let Some(old_parent) = self.node(child).and_then(|node| node.parent) {
            self.remove_child(old_parent, child);
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

    /// Removes every reference to `child` from `parent`.
    ///
    /// The removed child becomes detached when its parent metadata points to
    /// `parent`, and the remaining sibling indexes are refreshed. Returns
    /// whether the parent contained at least one matching child reference.
    pub fn remove_child(&mut self, parent: NodeId, child: NodeId) -> bool {
        let Some(parent_node) = self.node_mut(parent) else {
            return false;
        };
        let original_len = parent_node.children.len();
        parent_node.children.retain(|id| *id != child);
        if parent_node.children.len() == original_len {
            return false;
        }

        if let Some(child_node) = self.node_mut(child) {
            if child_node.parent == Some(parent) {
                child_node.parent = None;
                child_node.index_in_parent = 0;
            }
        }
        self.refresh_child_indexes(parent);
        true
    }

    /// Replaces the full child list of `parent`.
    ///
    /// Invalid, duplicate, root, or cycle-forming child ids leave the document
    /// unchanged.
    pub fn replace_children(&mut self, parent: NodeId, children: Vec<NodeId>) {
        if self.node(parent).is_none() {
            return;
        }
        let mut unique_children = BTreeSet::new();
        if children.iter().copied().any(|child| {
            child == self.root
                || self.node(child).is_none()
                || !unique_children.insert(child)
                || self.would_create_cycle(parent, child)
        }) {
            return;
        }

        let old_children = self
            .node(parent)
            .map(|node| node.children.clone())
            .unwrap_or_default();
        for old_child in old_children {
            if !unique_children.contains(&old_child) {
                if let Some(child_node) = self.node_mut(old_child) {
                    if child_node.parent == Some(parent) {
                        child_node.parent = None;
                        child_node.index_in_parent = 0;
                    }
                }
            }
        }
        for child in &children {
            if let Some(old_parent) = self.node(*child).and_then(|node| node.parent) {
                if old_parent != parent {
                    self.remove_child(old_parent, *child);
                }
            }
        }
        if let Some(parent_node) = self.node_mut(parent) {
            parent_node.children = children.clone();
        }
        self.refresh_child_indexes(parent);
    }

    /// Sets the document root node.
    ///
    /// Returns `false` and leaves the current root unchanged when `id` does not
    /// reference an arena node.
    pub fn set_root(&mut self, id: NodeId) -> bool {
        if self.node(id).is_none() {
            return false;
        }
        if let Some(parent) = self.node(id).and_then(|node| node.parent) {
            self.remove_child(parent, id);
        }
        self.root = id;
        if let Some(root_node) = self.node_mut(id) {
            root_node.parent = None;
            root_node.index_in_parent = 0;
        }
        true
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

    /// Validates bidirectional parent/child, root, and node-id invariants.
    ///
    /// Detached nodes and detached subtrees are valid arena contents, but their
    /// internal relationships must remain consistent.
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
            } else if node.parent.is_none() && node.index_in_parent != 0 {
                return Err(AstInvariantError::InvalidDetachedMetadata {
                    node: node.id,
                    index_in_parent: node.index_in_parent,
                });
            }

            let mut children = BTreeSet::new();
            for child_id in node.children.iter().copied() {
                if !children.insert(child_id) {
                    return Err(AstInvariantError::DuplicateChild {
                        parent: node.id,
                        child: child_id,
                    });
                }
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

        for node in &self.nodes {
            let Some(parent_id) = node.parent else {
                continue;
            };
            let parent = self
                .node(parent_id)
                .ok_or(AstInvariantError::MissingParent {
                    node: node.id,
                    parent: parent_id,
                })?;
            if parent.children.get(node.index_in_parent as usize) != Some(&node.id) {
                return Err(AstInvariantError::InvalidParentMetadata {
                    node: node.id,
                    parent: parent_id,
                    index_in_parent: node.index_in_parent,
                });
            }
        }

        self.validate_parent_cycles()
    }

    fn validate_parent_cycles(&self) -> Result<(), AstInvariantError> {
        const UNVISITED: u8 = 0;
        const VISITING: u8 = 1;
        const COMPLETE: u8 = 2;

        let mut states = vec![UNVISITED; self.nodes.len()];
        for start in 0..self.nodes.len() {
            let mut current = Some(start);
            while let Some(index) = current {
                match states[index] {
                    UNVISITED => {
                        states[index] = VISITING;
                        current = self.nodes[index].parent.map(|parent| parent.0 as usize);
                    }
                    VISITING => {
                        return Err(AstInvariantError::Cycle {
                            node: self.nodes[index].id,
                        });
                    }
                    COMPLETE => break,
                    _ => unreachable!("parent visit state is internal"),
                }
            }

            let mut current = Some(start);
            while let Some(index) = current {
                if states[index] != VISITING {
                    break;
                }
                states[index] = COMPLETE;
                current = self.nodes[index].parent.map(|parent| parent.0 as usize);
            }
        }
        Ok(())
    }

    fn would_create_cycle(&self, parent: NodeId, child: NodeId) -> bool {
        let mut ancestor = Some(parent);
        for _ in 0..self.nodes.len() {
            let Some(id) = ancestor else {
                return false;
            };
            if id == child {
                return true;
            }
            ancestor = self.node(id).and_then(|node| node.parent);
        }
        ancestor.is_some()
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
