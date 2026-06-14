/// Vue 2 compiler module hooks.
pub trait Vue2Module {
    /// Stable module name.
    fn name(&self) -> &'static str;
    /// Stable module ordering key.
    fn order(&self) -> PassOrder {
        PassOrder::DEFAULT
    }
    /// Hook equivalent to Vue 2 `preTransformNode`.
    fn pre_transform_node(
        &mut self,
        _doc: &mut Vue2Ast,
        _node: NodeId,
        _ctx: &mut TransformContext,
    ) -> VisitControl {
        VisitControl::Continue
    }
    /// Hook equivalent to Vue 2 `transformNode`.
    fn transform_node(
        &mut self,
        _doc: &mut Vue2Ast,
        _node: NodeId,
        _ctx: &mut TransformContext,
    ) -> VisitControl {
        VisitControl::Continue
    }
    /// Hook equivalent to Vue 2 `postTransformNode`.
    fn post_transform_node(
        &mut self,
        _doc: &mut Vue2Ast,
        _node: NodeId,
        _ctx: &mut TransformContext,
    ) -> VisitControl {
        VisitControl::Continue
    }
    /// Hook equivalent to Vue 2 module `genData`.
    fn gen_data(&mut self, _element: &Vue2Element, _ctx: &mut TransformContext) -> Option<String> {
        None
    }
}

struct OrderedVue2Module {
    order: PassOrder,
    insertion: usize,
    module: Box<dyn Vue2Module>,
}

impl OrderedEntry for OrderedVue2Module {
    fn order(&self) -> PassOrder {
        self.order
    }

    fn insertion(&self) -> usize {
        self.insertion
    }
}

/// Ordered scheduler for Vue 2 compiler module hooks.
pub struct Vue2ModuleScheduler {
    modules: Vec<OrderedVue2Module>,
    next_insertion: usize,
}

impl Vue2ModuleScheduler {
    /// Creates an empty Vue 2 module scheduler.
    pub fn new() -> Self {
        Self {
            modules: Vec::new(),
            next_insertion: 0,
        }
    }

    /// Appends a Vue 2 module.
    pub fn push<M>(&mut self, module: M)
    where
        M: Vue2Module + 'static,
    {
        let order = module.order();
        self.modules.push(OrderedVue2Module {
            order,
            insertion: self.next_insertion,
            module: Box::new(module),
        });
        self.next_insertion += 1;
        sort_ordered_entries(&mut self.modules);
    }

    /// Runs `preTransformNode`, `transformNode`, and `postTransformNode`.
    pub fn run(&mut self, doc: &mut Vue2Ast, ctx: &mut TransformContext) -> VisitControl {
        self.walk_node(doc, ctx, doc.root)
    }

    /// Runs module `genData` hooks for an element and returns data fragments in order.
    pub fn gen_data_for_element(
        &mut self,
        element: &Vue2Element,
        ctx: &mut TransformContext,
    ) -> Vec<String> {
        self.modules
            .iter_mut()
            .filter_map(|entry| entry.module.gen_data(element, ctx))
            .collect()
    }

    /// Runs module `genData` hooks for a Vue 2 AST element node.
    pub fn gen_data_for_node(
        &mut self,
        doc: &Vue2Ast,
        node: NodeId,
        ctx: &mut TransformContext,
    ) -> Vec<String> {
        let Some(node) = doc.node(node) else {
            return Vec::new();
        };
        let Vue2NodeKind::Element(element) = &node.kind else {
            return Vec::new();
        };
        self.gen_data_for_element(element, ctx)
    }

    /// Returns module names in execution order.
    pub fn module_names(&self) -> Vec<&'static str> {
        self.modules
            .iter()
            .map(|entry| entry.module.name())
            .collect()
    }

    fn walk_node(
        &mut self,
        doc: &mut Vue2Ast,
        ctx: &mut TransformContext,
        node: NodeId,
    ) -> VisitControl {
        let mut skip_children = false;
        for entry in &mut self.modules {
            match entry.module.pre_transform_node(doc, node, ctx) {
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

        for entry in &mut self.modules {
            if entry.module.transform_node(doc, node, ctx) == VisitControl::Stop {
                return VisitControl::Stop;
            }
        }
        for entry in &mut self.modules {
            if entry.module.post_transform_node(doc, node, ctx) == VisitControl::Stop {
                return VisitControl::Stop;
            }
        }
        VisitControl::Continue
    }
}

impl Default for Vue2ModuleScheduler {
    fn default() -> Self {
        Self::new()
    }
}
