/// Output produced by a Vue 3 directive transform.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3DirectiveTransformOutput {
    /// Props produced by the directive transform.
    pub props: Vec<Vue3Prop>,
    /// Runtime helpers required by the transform output.
    pub runtime_helpers: BTreeSet<RuntimeHelper>,
}

impl Vue3DirectiveTransformOutput {
    /// Creates empty directive transform output.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates output with one generated prop.
    pub fn with_prop(prop: Vue3Prop) -> Self {
        Self {
            props: vec![prop],
            runtime_helpers: BTreeSet::new(),
        }
    }

    /// Adds a runtime helper requirement.
    pub fn add_helper(&mut self, helper: RuntimeHelper) -> bool {
        self.runtime_helpers.insert(helper)
    }
}

/// Directive transform decision relative to the default transform.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vue3DirectiveTransformOutcome {
    /// Keep the default directive transform behavior.
    UseDefault,
    /// Keep the default behavior and append extra output.
    Extend(Vue3DirectiveTransformOutput),
    /// Replace default behavior with this output.
    Replace(Vue3DirectiveTransformOutput),
}

/// Resolved directive transform output after ordered overrides.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vue3DirectiveTransformResolution {
    /// Whether the caller should run the default directive transform.
    pub use_default: bool,
    /// Props produced by custom transforms.
    pub props: Vec<Vue3Prop>,
    /// Runtime helpers required by custom transform output.
    pub runtime_helpers: BTreeSet<RuntimeHelper>,
}

impl Default for Vue3DirectiveTransformResolution {
    fn default() -> Self {
        Self {
            use_default: true,
            props: Vec::new(),
            runtime_helpers: BTreeSet::new(),
        }
    }
}

impl Vue3DirectiveTransformResolution {
    fn apply(&mut self, outcome: Vue3DirectiveTransformOutcome) {
        match outcome {
            Vue3DirectiveTransformOutcome::UseDefault => {}
            Vue3DirectiveTransformOutcome::Extend(output) => {
                self.props.extend(output.props);
                self.runtime_helpers.extend(output.runtime_helpers);
            }
            Vue3DirectiveTransformOutcome::Replace(output) => {
                self.use_default = false;
                self.props = output.props;
                self.runtime_helpers = output.runtime_helpers;
            }
        }
    }
}

/// Vue 3 compiler-core directive transform.
pub trait Vue3DirectiveTransform {
    /// Stable transform name.
    fn name(&self) -> &'static str;
    /// Directive name matched against `Vue3Directive::name`.
    fn directive_name(&self) -> &'static str;
    /// Stable transform ordering key.
    fn order(&self) -> PassOrder {
        PassOrder::DEFAULT
    }
    /// Applies this directive transform.
    fn transform(
        &mut self,
        _doc: &mut Vue3Ast,
        _node: NodeId,
        _prop_index: usize,
        _directive: &Vue3Directive,
        _ctx: &mut TransformContext,
    ) -> Vue3DirectiveTransformOutcome {
        Vue3DirectiveTransformOutcome::UseDefault
    }
}

struct OrderedVue3DirectiveTransform {
    order: PassOrder,
    insertion: usize,
    directive_name: &'static str,
    transform: Box<dyn Vue3DirectiveTransform>,
}

impl OrderedEntry for OrderedVue3DirectiveTransform {
    fn order(&self) -> PassOrder {
        self.order
    }

    fn insertion(&self) -> usize {
        self.insertion
    }
}

/// Ordered registry for Vue 3 directive transforms.
pub struct Vue3DirectiveTransformRegistry {
    transforms: Vec<OrderedVue3DirectiveTransform>,
    next_insertion: usize,
}

impl Vue3DirectiveTransformRegistry {
    /// Creates an empty directive transform registry.
    pub fn new() -> Self {
        Self {
            transforms: Vec::new(),
            next_insertion: 0,
        }
    }

    /// Appends a directive transform.
    pub fn push<T>(&mut self, transform: T)
    where
        T: Vue3DirectiveTransform + 'static,
    {
        let order = transform.order();
        let directive_name = transform.directive_name();
        self.transforms.push(OrderedVue3DirectiveTransform {
            order,
            insertion: self.next_insertion,
            directive_name,
            transform: Box::new(transform),
        });
        self.next_insertion += 1;
        sort_ordered_entries(&mut self.transforms);
    }

    /// Resolves directive transform output for one Vue 3 directive prop.
    pub fn resolve(
        &mut self,
        doc: &mut Vue3Ast,
        node: NodeId,
        prop_index: usize,
        ctx: &mut TransformContext,
    ) -> Vue3DirectiveTransformResolution {
        let Some(directive) = vue3_directive_at(doc, node, prop_index) else {
            return Vue3DirectiveTransformResolution::default();
        };
        let mut resolution = Vue3DirectiveTransformResolution::default();
        for entry in &mut self.transforms {
            if entry.directive_name == directive.name {
                let outcome = entry
                    .transform
                    .transform(doc, node, prop_index, &directive, ctx);
                resolution.apply(outcome);
            }
        }
        for helper in &resolution.runtime_helpers {
            ctx.add_helper(*helper);
        }
        resolution
    }

    /// Returns transform names in execution order.
    pub fn transform_names(&self) -> Vec<&'static str> {
        self.transforms
            .iter()
            .map(|entry| entry.transform.name())
            .collect()
    }
}

impl Default for Vue3DirectiveTransformRegistry {
    fn default() -> Self {
        Self::new()
    }
}

fn vue3_directive_at(doc: &Vue3Ast, node: NodeId, prop_index: usize) -> Option<Vue3Directive> {
    let node = doc.node(node)?;
    let Vue3NodeKind::Element(element) = &node.kind else {
        return None;
    };
    let Vue3Prop::Directive(directive) = element.props.get(prop_index)? else {
        return None;
    };
    Some(directive.clone())
}
