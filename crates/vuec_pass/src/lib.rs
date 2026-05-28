//! Pass scheduling and traversal utilities for Vue compiler transforms.
//!
//! This crate intentionally stays small: it owns shared transform context and
//! generic walkers, while compiler semantics live in the dialect crates.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use vuec_ast::{
    AstDocument, NodeId, RuntimeHelper, VisitControl, Vue2Ast, Vue2Element, Vue2NodeKind, Vue3Ast,
    Vue3Directive, Vue3NodeKind, Vue3Prop,
};
use vuec_diagnostics::{Diagnostic, DiagnosticSink};

/// Stable ordering key for transform passes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PassOrder(i32);

impl PassOrder {
    /// Early pass slot used for parser-adjacent normalization.
    pub const EARLY: Self = Self(-1000);
    /// Default pass slot.
    pub const DEFAULT: Self = Self(0);
    /// Late pass slot used for cleanup or finalization.
    pub const LATE: Self = Self(1000);

    /// Creates an explicit ordering key.
    pub const fn new(value: i32) -> Self {
        Self(value)
    }

    /// Returns the numeric ordering key.
    pub const fn value(self) -> i32 {
        self.0
    }
}

impl From<i32> for PassOrder {
    fn from(value: i32) -> Self {
        Self::new(value)
    }
}

/// A transform scope category.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ScopeKind {
    /// Root transform scope.
    Root,
    /// Template expression scope.
    Template,
    /// Function-like generated scope.
    Function,
    /// `v-for` alias scope.
    VFor,
    /// Slot parameter scope.
    Slot,
    /// Dialect-specific scope category.
    Custom(String),
}

/// A single lexical or template scope frame.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopeFrame {
    /// Scope category.
    pub kind: ScopeKind,
    /// Bindings declared in this frame.
    pub bindings: BTreeSet<String>,
}

impl ScopeFrame {
    /// Creates an empty scope frame.
    pub fn new(kind: ScopeKind) -> Self {
        Self {
            kind,
            bindings: BTreeSet::new(),
        }
    }

    /// Adds a binding to this scope and returns whether it was newly inserted.
    pub fn add_binding(&mut self, binding: impl Into<String>) -> bool {
        self.bindings.insert(binding.into())
    }
}

/// Shared mutable state available to transform passes.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransformContext {
    /// Runtime helpers requested by transform passes.
    pub helpers: BTreeSet<RuntimeHelper>,
    /// Current lexical or template scope nesting depth.
    pub scope_depth: usize,
    /// Lexical and template scope frames from outermost to innermost.
    #[serde(default)]
    pub scopes: Vec<ScopeFrame>,
    #[serde(skip)]
    /// Diagnostics collected while running transforms.
    pub diagnostics: DiagnosticSink,
}

impl TransformContext {
    /// Records a diagnostic emitted by a transform pass.
    pub fn report(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    /// Adds a runtime helper requirement and returns whether it was newly added.
    pub fn add_helper(&mut self, helper: RuntimeHelper) -> bool {
        self.helpers.insert(helper)
    }

    /// Removes a runtime helper requirement and returns whether it existed.
    pub fn remove_helper(&mut self, helper: RuntimeHelper) -> bool {
        self.helpers.remove(&helper)
    }

    /// Returns whether a runtime helper has been requested.
    pub fn has_helper(&self, helper: RuntimeHelper) -> bool {
        self.helpers.contains(&helper)
    }

    /// Pushes a scope frame and updates the compatibility depth counter.
    pub fn enter_scope(&mut self, kind: ScopeKind) {
        self.scopes.push(ScopeFrame::new(kind));
        self.sync_scope_depth();
    }

    /// Pops the innermost scope frame.
    pub fn exit_scope(&mut self) -> Option<ScopeFrame> {
        let frame = self.scopes.pop();
        self.sync_scope_depth();
        frame
    }

    /// Returns the innermost scope frame.
    pub fn current_scope(&self) -> Option<&ScopeFrame> {
        self.scopes.last()
    }

    /// Returns the innermost mutable scope frame.
    pub fn current_scope_mut(&mut self) -> Option<&mut ScopeFrame> {
        self.scopes.last_mut()
    }

    /// Adds a binding to the innermost scope.
    pub fn add_scope_binding(&mut self, binding: impl Into<String>) -> bool {
        match self.current_scope_mut() {
            Some(scope) => scope.add_binding(binding),
            None => false,
        }
    }

    /// Returns whether a binding exists in any active scope.
    pub fn is_binding_in_scope(&self, binding: &str) -> bool {
        self.scopes
            .iter()
            .rev()
            .any(|scope| scope.bindings.contains(binding))
    }

    /// Returns collected diagnostics without consuming the context.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        self.diagnostics.as_slice()
    }

    fn sync_scope_depth(&mut self) {
        self.scope_depth = self.scopes.len();
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;
    use vuec_ast::{
        QuoteKind, Vue2AstKind, Vue3AstKind, Vue3Attribute, Vue3Element, Vue3ElementType,
        Vue3Expression, Vue3Prop,
    };

    #[derive(Default)]
    struct CountPass(usize);

    impl TransformPass<usize> for CountPass {
        fn name(&self) -> &'static str {
            "count"
        }

        fn enter(&mut self, node: &mut usize, ctx: &mut TransformContext) {
            self.0 += 1;
            let _ = node;
            ctx.add_helper(RuntimeHelper::Vue3OpenBlock);
        }
    }

    struct NamedPass {
        name: &'static str,
        order: PassOrder,
        events: Rc<RefCell<Vec<&'static str>>>,
    }

    impl TransformPass<usize> for NamedPass {
        fn name(&self) -> &'static str {
            self.name
        }

        fn order(&self) -> PassOrder {
            self.order
        }

        fn enter(&mut self, _node: &mut usize, _ctx: &mut TransformContext) {
            self.events.borrow_mut().push(self.name);
        }
    }

    #[test]
    fn scheduler_runs_passes() {
        let mut scheduler = PassScheduler::new();
        scheduler.push(CountPass::default());
        let mut nodes = vec![1, 2, 3];
        let mut ctx = TransformContext::default();
        scheduler.run(&mut nodes, &mut ctx);
        assert_eq!(ctx.helpers.len(), 1);
        assert!(ctx.helpers.contains(&RuntimeHelper::Vue3OpenBlock));
    }

    #[test]
    fn scheduler_orders_passes_stably() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let mut scheduler = PassScheduler::new();
        scheduler.push(NamedPass {
            name: "late",
            order: PassOrder::LATE,
            events: events.clone(),
        });
        scheduler.push(NamedPass {
            name: "early",
            order: PassOrder::EARLY,
            events: events.clone(),
        });
        scheduler.push(NamedPass {
            name: "default-a",
            order: PassOrder::DEFAULT,
            events: events.clone(),
        });
        scheduler.push(NamedPass {
            name: "default-b",
            order: PassOrder::DEFAULT,
            events: events.clone(),
        });

        assert_eq!(
            scheduler.pass_names(),
            vec!["early", "default-a", "default-b", "late"]
        );
        let mut nodes = vec![0usize];
        scheduler.run(&mut nodes, &mut TransformContext::default());
        assert_eq!(
            events.borrow().as_slice(),
            ["early", "default-a", "default-b", "late"]
        );
    }

    #[test]
    fn transform_context_tracks_scope_stack_helpers_and_diagnostics() {
        let mut ctx = TransformContext::default();
        assert!(ctx.add_helper(RuntimeHelper::Vue3OpenBlock));
        assert!(ctx.has_helper(RuntimeHelper::Vue3OpenBlock));
        ctx.enter_scope(ScopeKind::Root);
        ctx.enter_scope(ScopeKind::VFor);
        assert_eq!(ctx.scope_depth, 2);
        assert!(ctx.add_scope_binding("item"));
        assert!(ctx.is_binding_in_scope("item"));
        assert_eq!(ctx.current_scope().unwrap().kind, ScopeKind::VFor);
        assert_eq!(ctx.exit_scope().unwrap().kind, ScopeKind::VFor);
        assert_eq!(ctx.scope_depth, 1);
        assert!(!ctx.is_binding_in_scope("item"));
    }

    #[derive(Default)]
    struct RecordDocumentPass {
        name: &'static str,
        order: PassOrder,
        events: Rc<RefCell<Vec<String>>>,
    }

    impl DocumentPass<usize> for RecordDocumentPass {
        fn name(&self) -> &'static str {
            self.name
        }

        fn order(&self) -> PassOrder {
            self.order
        }

        fn enter(
            &mut self,
            _doc: &mut AstDocument<usize>,
            node: NodeId,
            _ctx: &mut TransformContext,
        ) {
            self.events
                .borrow_mut()
                .push(format!("{}:enter:{}", self.name, node.0));
        }

        fn exit(
            &mut self,
            _doc: &mut AstDocument<usize>,
            node: NodeId,
            _ctx: &mut TransformContext,
        ) {
            self.events
                .borrow_mut()
                .push(format!("{}:exit:{}", self.name, node.0));
        }
    }

    #[test]
    fn document_walk_is_depth_first() {
        let mut doc = vuec_ast::AstDocument::new(0usize, None);
        let root = doc.root;
        let child = doc.push_child(root, 1usize, None);
        let _grandchild = doc.push_child(child, 2usize, None);
        let events = Rc::new(RefCell::new(Vec::new()));
        let mut pass = RecordDocumentPass {
            name: "document",
            order: PassOrder::DEFAULT,
            events: events.clone(),
        };
        let mut ctx = TransformContext::default();
        walk_document(&mut doc, &mut pass, &mut ctx);
        assert_eq!(
            events.borrow().as_slice(),
            [
                "document:enter:0",
                "document:enter:1",
                "document:enter:2",
                "document:exit:2",
                "document:exit:1",
                "document:exit:0"
            ]
        );
    }

    #[test]
    fn document_scheduler_orders_passes() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let mut doc = AstDocument::new(0usize, None);
        let mut scheduler = DocumentPassScheduler::new();
        scheduler.push(RecordDocumentPass {
            name: "late",
            order: PassOrder::LATE,
            events: events.clone(),
        });
        scheduler.push(RecordDocumentPass {
            name: "early",
            order: PassOrder::EARLY,
            events: events.clone(),
        });

        assert_eq!(scheduler.pass_names(), vec!["early", "late"]);
        scheduler.run(&mut doc, &mut TransformContext::default());
        assert_eq!(
            events.borrow().as_slice(),
            [
                "early:enter:0",
                "early:exit:0",
                "late:enter:0",
                "late:exit:0"
            ]
        );
    }

    struct Vue2Recorder {
        name: &'static str,
        order: PassOrder,
        events: Rc<RefCell<Vec<String>>>,
    }

    impl Vue2Module for Vue2Recorder {
        fn name(&self) -> &'static str {
            self.name
        }

        fn order(&self) -> PassOrder {
            self.order
        }

        fn pre_transform_node(
            &mut self,
            _doc: &mut Vue2Ast,
            node: NodeId,
            _ctx: &mut TransformContext,
        ) -> VisitControl {
            self.events
                .borrow_mut()
                .push(format!("{}:pre:{}", self.name, node.0));
            VisitControl::Continue
        }

        fn transform_node(
            &mut self,
            _doc: &mut Vue2Ast,
            node: NodeId,
            _ctx: &mut TransformContext,
        ) -> VisitControl {
            self.events
                .borrow_mut()
                .push(format!("{}:transform:{}", self.name, node.0));
            VisitControl::Continue
        }

        fn post_transform_node(
            &mut self,
            _doc: &mut Vue2Ast,
            node: NodeId,
            _ctx: &mut TransformContext,
        ) -> VisitControl {
            self.events
                .borrow_mut()
                .push(format!("{}:post:{}", self.name, node.0));
            VisitControl::Continue
        }

        fn gen_data(
            &mut self,
            element: &Vue2Element,
            _ctx: &mut TransformContext,
        ) -> Option<String> {
            Some(format!("{}:{}", self.name, element.tag))
        }
    }

    #[test]
    fn vue2_module_scheduler_replicates_pre_transform_post_order() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let mut doc = Vue2Ast::new(Vue2AstKind::root(), None);
        let child = doc.push_child(doc.root, Vue2AstKind::element("div"), None);
        doc.push_child(child, Vue2AstKind::text("hello"), None);
        let mut scheduler = Vue2ModuleScheduler::new();
        scheduler.push(Vue2Recorder {
            name: "module",
            order: PassOrder::DEFAULT,
            events: events.clone(),
        });

        assert_eq!(
            scheduler.run(&mut doc, &mut TransformContext::default()),
            VisitControl::Continue
        );
        assert_eq!(
            events.borrow().as_slice(),
            [
                "module:pre:0",
                "module:pre:1",
                "module:pre:2",
                "module:transform:2",
                "module:post:2",
                "module:transform:1",
                "module:post:1",
                "module:transform:0",
                "module:post:0"
            ]
        );
        assert_eq!(
            scheduler.gen_data_for_node(&doc, child, &mut TransformContext::default()),
            vec!["module:div".to_string()]
        );
    }

    struct Vue3Recorder {
        name: &'static str,
        order: PassOrder,
        events: Rc<RefCell<Vec<String>>>,
    }

    impl Vue3NodeTransform for Vue3Recorder {
        fn name(&self) -> &'static str {
            self.name
        }

        fn order(&self) -> PassOrder {
            self.order
        }

        fn enter(
            &mut self,
            _doc: &mut Vue3Ast,
            node: NodeId,
            _ctx: &mut TransformContext,
        ) -> Vue3NodeTransformResult {
            self.events
                .borrow_mut()
                .push(format!("{}:enter:{}", self.name, node.0));
            Vue3NodeTransformResult::with_exit()
        }

        fn exit(&mut self, _doc: &mut Vue3Ast, node: NodeId, _ctx: &mut TransformContext) {
            self.events
                .borrow_mut()
                .push(format!("{}:exit:{}", self.name, node.0));
        }
    }

    #[test]
    fn vue3_node_scheduler_uses_depth_first_lifo_exit_order() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let mut doc = Vue3Ast::new(Vue3AstKind::root(), None);
        let child = doc.push_child(doc.root, Vue3AstKind::text("hello"), None);
        doc.push_child(child, Vue3AstKind::comment("x"), None);
        let mut scheduler = Vue3NodeTransformScheduler::new();
        scheduler.push(Vue3Recorder {
            name: "a",
            order: PassOrder::DEFAULT,
            events: events.clone(),
        });
        scheduler.push(Vue3Recorder {
            name: "b",
            order: PassOrder::DEFAULT,
            events: events.clone(),
        });

        assert_eq!(
            scheduler.run(&mut doc, &mut TransformContext::default()),
            VisitControl::Continue
        );
        assert_eq!(
            events.borrow().as_slice(),
            [
                "a:enter:0",
                "b:enter:0",
                "a:enter:1",
                "b:enter:1",
                "a:enter:2",
                "b:enter:2",
                "b:exit:2",
                "a:exit:2",
                "b:exit:1",
                "a:exit:1",
                "b:exit:0",
                "a:exit:0"
            ]
        );
    }

    struct DirectiveTransform {
        name: &'static str,
        directive_name: &'static str,
        order: PassOrder,
        outcome: Vue3DirectiveTransformOutcome,
    }

    impl Vue3DirectiveTransform for DirectiveTransform {
        fn name(&self) -> &'static str {
            self.name
        }

        fn directive_name(&self) -> &'static str {
            self.directive_name
        }

        fn order(&self) -> PassOrder {
            self.order
        }

        fn transform(
            &mut self,
            _doc: &mut Vue3Ast,
            _node: NodeId,
            _prop_index: usize,
            _directive: &Vue3Directive,
            _ctx: &mut TransformContext,
        ) -> Vue3DirectiveTransformOutcome {
            self.outcome.clone()
        }
    }

    #[test]
    fn vue3_directive_registry_can_extend_or_replace_default_behavior() {
        let directive = Vue3Prop::Directive(Vue3Directive {
            name: "on".into(),
            raw_name: "@click".into(),
            arg: Some(Vue3Expression::Raw("click".into())),
            exp: Some(Vue3Expression::Raw("submit".into())),
            modifiers: Vec::new(),
            is_dynamic_arg: false,
            span: None,
            arg_span: None,
            exp_span: None,
            modifier_spans: Vec::new(),
        });
        let mut doc = Vue3Ast::new(Vue3AstKind::root(), None);
        let element = doc.push_child(
            doc.root,
            Vue3AstKind::Element(Vue3Element {
                tag: "button".into(),
                tag_type: Vue3ElementType::Element,
                ns: vuec_ast::HtmlNamespace::Html,
                props: vec![directive],
                self_closing: false,
                codegen_node: None,
                ssr_codegen_node: None,
            }),
            None,
        );
        let replacement = Vue3Prop::Attribute(Vue3Attribute {
            name: "data-replaced".into(),
            value: Some("yes".into()),
            span: None,
            name_span: None,
            value_span: None,
            quote: Some(QuoteKind::Double),
        });
        let extension = Vue3Prop::Attribute(Vue3Attribute {
            name: "data-extra".into(),
            value: None,
            span: None,
            name_span: None,
            value_span: None,
            quote: None,
        });
        let mut output = Vue3DirectiveTransformOutput::with_prop(replacement.clone());
        output.add_helper(RuntimeHelper::Vue3WithModifiers);
        let mut registry = Vue3DirectiveTransformRegistry::new();
        registry.push(DirectiveTransform {
            name: "replace-on",
            directive_name: "on",
            order: PassOrder::DEFAULT,
            outcome: Vue3DirectiveTransformOutcome::Replace(output),
        });
        registry.push(DirectiveTransform {
            name: "extend-on",
            directive_name: "on",
            order: PassOrder::LATE,
            outcome: Vue3DirectiveTransformOutcome::Extend(
                Vue3DirectiveTransformOutput::with_prop(extension.clone()),
            ),
        });

        let mut ctx = TransformContext::default();
        let resolution = registry.resolve(&mut doc, element, 0, &mut ctx);
        assert!(!resolution.use_default);
        assert_eq!(resolution.props, vec![replacement, extension]);
        assert!(ctx.has_helper(RuntimeHelper::Vue3WithModifiers));
    }
}
