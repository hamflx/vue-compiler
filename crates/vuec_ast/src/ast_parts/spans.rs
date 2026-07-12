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
    /// A detached node retains a non-zero parent index.
    InvalidDetachedMetadata {
        /// Detached node id.
        node: NodeId,
        /// Invalid index retained without a parent.
        index_in_parent: u32,
    },
    /// A node's parent reference does not point to an arena node.
    MissingParent {
        /// Node containing the parent reference.
        node: NodeId,
        /// Missing parent id.
        parent: NodeId,
    },
    /// A parent does not list a node at its declared child index.
    InvalidParentMetadata {
        /// Node with inconsistent parent metadata.
        node: NodeId,
        /// Declared parent id.
        parent: NodeId,
        /// Declared index inside the parent.
        index_in_parent: u32,
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
    /// A parent contains the same child id more than once.
    DuplicateChild {
        /// Parent containing duplicate references.
        parent: NodeId,
        /// Repeated child id.
        child: NodeId,
    },
    /// Parent relationships contain a cycle.
    Cycle {
        /// Node at which the parent walk re-entered the cycle.
        node: NodeId,
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

    /// Projects the arena tree into a nested public tree after validating the root.
    pub fn try_project_public(&self) -> Result<PublicNode<K>, AstInvariantError> {
        if self.node(self.root).is_none() {
            return Err(AstInvariantError::MissingRoot { root: self.root });
        }
        Ok(self
            .project_nested()
            .expect("validated AstDocument root should project"))
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
