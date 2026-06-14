/// A transform pass over a flat list of nodes.
pub trait TransformPass<N> {
    /// Stable pass name used in diagnostics and debugging.
    fn name(&self) -> &'static str;
    /// Stable pass ordering key.
    fn order(&self) -> PassOrder {
        PassOrder::DEFAULT
    }
    /// Hook called before child or exit processing.
    fn enter(&mut self, _node: &mut N, _ctx: &mut TransformContext) {}
    /// Hook called after enter processing.
    fn exit(&mut self, _node: &mut N, _ctx: &mut TransformContext) {}
}

/// A transform pass over an arena-backed AST document.
pub trait DocumentPass<K> {
    /// Stable pass name used in diagnostics and debugging.
    fn name(&self) -> &'static str;
    /// Stable pass ordering key.
    fn order(&self) -> PassOrder {
        PassOrder::DEFAULT
    }
    /// Hook called before walking a node's children.
    fn enter(&mut self, _doc: &mut AstDocument<K>, _node: NodeId, _ctx: &mut TransformContext) {}
    /// Hook called after walking a node's children.
    fn exit(&mut self, _doc: &mut AstDocument<K>, _node: NodeId, _ctx: &mut TransformContext) {}
}

struct OrderedTransformPass<N> {
    order: PassOrder,
    insertion: usize,
    pass: Box<dyn TransformPass<N>>,
}

/// Ordered scheduler for flat node transform passes.
pub struct PassScheduler<N> {
    passes: Vec<OrderedTransformPass<N>>,
    next_insertion: usize,
}

impl<N> PassScheduler<N> {
    /// Creates an empty pass scheduler.
    pub fn new() -> Self {
        Self {
            passes: Vec::new(),
            next_insertion: 0,
        }
    }

    /// Appends a pass to the scheduler.
    pub fn push<P>(&mut self, pass: P)
    where
        P: TransformPass<N> + 'static,
    {
        let order = pass.order();
        self.passes.push(OrderedTransformPass {
            order,
            insertion: self.next_insertion,
            pass: Box::new(pass),
        });
        self.next_insertion += 1;
        sort_ordered_entries(&mut self.passes);
    }

    /// Runs all registered passes over `nodes` in insertion order.
    pub fn run(&mut self, nodes: &mut [N], ctx: &mut TransformContext) {
        for node in nodes {
            for pass in &mut self.passes {
                pass.pass.enter(node, ctx);
                pass.pass.exit(node, ctx);
            }
        }
    }

    /// Returns pass names in execution order.
    pub fn pass_names(&self) -> Vec<&'static str> {
        self.passes.iter().map(|entry| entry.pass.name()).collect()
    }
}

impl<N> Default for PassScheduler<N> {
    fn default() -> Self {
        Self::new()
    }
}

struct OrderedDocumentPass<K> {
    order: PassOrder,
    insertion: usize,
    pass: Box<dyn DocumentPass<K>>,
}

/// Ordered scheduler for document-level passes.
pub struct DocumentPassScheduler<K> {
    passes: Vec<OrderedDocumentPass<K>>,
    next_insertion: usize,
}

impl<K> DocumentPassScheduler<K> {
    /// Creates an empty document pass scheduler.
    pub fn new() -> Self {
        Self {
            passes: Vec::new(),
            next_insertion: 0,
        }
    }

    /// Appends a document pass to the scheduler.
    pub fn push<P>(&mut self, pass: P)
    where
        P: DocumentPass<K> + 'static,
    {
        let order = pass.order();
        self.passes.push(OrderedDocumentPass {
            order,
            insertion: self.next_insertion,
            pass: Box::new(pass),
        });
        self.next_insertion += 1;
        sort_ordered_entries(&mut self.passes);
    }

    /// Runs all registered document passes in pass order.
    pub fn run(&mut self, doc: &mut AstDocument<K>, ctx: &mut TransformContext) {
        for entry in &mut self.passes {
            walk_document(doc, entry.pass.as_mut(), ctx);
        }
    }

    /// Returns pass names in execution order.
    pub fn pass_names(&self) -> Vec<&'static str> {
        self.passes.iter().map(|entry| entry.pass.name()).collect()
    }
}

impl<K> Default for DocumentPassScheduler<K> {
    fn default() -> Self {
        Self::new()
    }
}

trait OrderedEntry {
    fn order(&self) -> PassOrder;
    fn insertion(&self) -> usize;
}

impl<N> OrderedEntry for OrderedTransformPass<N> {
    fn order(&self) -> PassOrder {
        self.order
    }

    fn insertion(&self) -> usize {
        self.insertion
    }
}

impl<K> OrderedEntry for OrderedDocumentPass<K> {
    fn order(&self) -> PassOrder {
        self.order
    }

    fn insertion(&self) -> usize {
        self.insertion
    }
}

fn sort_ordered_entries<T>(entries: &mut [T])
where
    T: OrderedEntry,
{
    entries.sort_by_key(|entry| (entry.order(), entry.insertion()));
}

/// Walks an [`vuec_ast::AstDocument`] in depth-first order with `pass`.
pub fn walk_document<K, P>(doc: &mut AstDocument<K>, pass: &mut P, ctx: &mut TransformContext)
where
    P: DocumentPass<K> + ?Sized,
{
    walk_document_node(doc, pass, ctx, doc.root);
}

fn walk_document_node<K, P>(
    doc: &mut AstDocument<K>,
    pass: &mut P,
    ctx: &mut TransformContext,
    node: NodeId,
) where
    P: DocumentPass<K> + ?Sized,
{
    pass.enter(doc, node, ctx);
    let children = doc
        .node(node)
        .map(|node| node.children.clone())
        .unwrap_or_default();
    for child in children {
        walk_document_node(doc, pass, ctx, child);
    }
    pass.exit(doc, node, ctx);
}
