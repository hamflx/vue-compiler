/// Result returned by a Vue 3 node transform enter hook.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3NodeTransformResult {
    /// Traversal control after this enter hook.
    pub control: VisitControl,
    /// Whether this transform's exit hook should run for the current node.
    pub exit: bool,
}

impl Vue3NodeTransformResult {
    /// Continues traversal without scheduling an exit callback.
    pub const fn continue_without_exit() -> Self {
        Self {
            control: VisitControl::Continue,
            exit: false,
        }
    }

    /// Continues traversal and schedules this transform's exit callback.
    pub const fn with_exit() -> Self {
        Self {
            control: VisitControl::Continue,
            exit: true,
        }
    }

    /// Skips children while still allowing scheduled exits to run.
    pub const fn skip_children() -> Self {
        Self {
            control: VisitControl::SkipChildren,
            exit: false,
        }
    }

    /// Stops traversal immediately.
    pub const fn stop() -> Self {
        Self {
            control: VisitControl::Stop,
            exit: false,
        }
    }
}

impl Default for Vue3NodeTransformResult {
    fn default() -> Self {
        Self::continue_without_exit()
    }
}

/// Vue 3 compiler-core node transform.
pub trait Vue3NodeTransform {
    /// Stable transform name.
    fn name(&self) -> &'static str;
    /// Stable transform ordering key.
    fn order(&self) -> PassOrder {
        PassOrder::DEFAULT
    }
    /// Runs before traversing the node's children.
    fn enter(
        &mut self,
        _doc: &mut Vue3Ast,
        _node: NodeId,
        _ctx: &mut TransformContext,
    ) -> Vue3NodeTransformResult {
        Vue3NodeTransformResult::continue_without_exit()
    }
    /// Runs after children when `enter` scheduled an exit callback.
    fn exit(&mut self, _doc: &mut Vue3Ast, _node: NodeId, _ctx: &mut TransformContext) {}
}

struct OrderedVue3NodeTransform {
    order: PassOrder,
    insertion: usize,
    transform: Box<dyn Vue3NodeTransform>,
}

impl OrderedEntry for OrderedVue3NodeTransform {
    fn order(&self) -> PassOrder {
        self.order
    }

    fn insertion(&self) -> usize {
        self.insertion
    }
}

/// Ordered scheduler for Vue 3 node transforms.
pub struct Vue3NodeTransformScheduler {
    transforms: Vec<OrderedVue3NodeTransform>,
    next_insertion: usize,
}

impl Vue3NodeTransformScheduler {
    /// Creates an empty Vue 3 node transform scheduler.
    pub fn new() -> Self {
        Self {
            transforms: Vec::new(),
            next_insertion: 0,
        }
    }

    /// Appends a Vue 3 node transform.
    pub fn push<T>(&mut self, transform: T)
    where
        T: Vue3NodeTransform + 'static,
    {
        let order = transform.order();
        self.transforms.push(OrderedVue3NodeTransform {
            order,
            insertion: self.next_insertion,
            transform: Box::new(transform),
        });
        self.next_insertion += 1;
        sort_ordered_entries(&mut self.transforms);
    }

    /// Runs transforms over the document in depth-first order.
    pub fn run(&mut self, doc: &mut Vue3Ast, ctx: &mut TransformContext) -> VisitControl {
        self.walk_node(doc, ctx, doc.root)
    }

    /// Returns transform names in execution order.
    pub fn transform_names(&self) -> Vec<&'static str> {
        self.transforms
            .iter()
            .map(|entry| entry.transform.name())
            .collect()
    }

    fn walk_node(
        &mut self,
        doc: &mut Vue3Ast,
        ctx: &mut TransformContext,
        node: NodeId,
    ) -> VisitControl {
        let mut exit_indices = Vec::new();
        let mut skip_children = false;
        for index in 0..self.transforms.len() {
            let result = self.transforms[index].transform.enter(doc, node, ctx);
            if result.exit {
                exit_indices.push(index);
            }
            match result.control {
                VisitControl::Continue => {}
                VisitControl::SkipChildren => skip_children = true,
                VisitControl::Stop => return VisitControl::Stop,
            }
        }

        if !skip_children {
            let children = doc
                .node(node)
                .map(|node| node.children.clone())
                .unwrap_or_default();
            for child in children {
                if self.walk_node(doc, ctx, child) == VisitControl::Stop {
                    return VisitControl::Stop;
                }
            }
        }

        for index in exit_indices.into_iter().rev() {
            self.transforms[index].transform.exit(doc, node, ctx);
        }
        VisitControl::Continue
    }
}

impl Default for Vue3NodeTransformScheduler {
    fn default() -> Self {
        Self::new()
    }
}
