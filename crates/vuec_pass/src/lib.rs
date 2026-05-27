//! Pass scheduling and traversal utilities for Vue compiler transforms.
//!
//! This crate intentionally stays small: it owns shared transform context and
//! generic walkers, while compiler semantics live in the dialect crates.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use vuec_ast::RuntimeHelper;
use vuec_diagnostics::{Diagnostic, DiagnosticSink};

/// Shared mutable state available to transform passes.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransformContext {
    /// Runtime helpers requested by transform passes.
    pub helpers: BTreeSet<RuntimeHelper>,
    /// Current lexical or template scope nesting depth.
    pub scope_depth: usize,
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
}

/// A transform pass over a flat list of nodes.
pub trait TransformPass<N> {
    /// Stable pass name used in diagnostics and debugging.
    fn name(&self) -> &'static str;
    /// Hook called before child or exit processing.
    fn enter(&mut self, _node: &mut N, _ctx: &mut TransformContext) {}
    /// Hook called after enter processing.
    fn exit(&mut self, _node: &mut N, _ctx: &mut TransformContext) {}
}

/// A transform pass over an arena-backed AST document.
pub trait DocumentPass<K> {
    /// Stable pass name used in diagnostics and debugging.
    fn name(&self) -> &'static str;
    /// Hook called before walking a node's children.
    fn enter(
        &mut self,
        _doc: &mut vuec_ast::AstDocument<K>,
        _node: vuec_ast::NodeId,
        _ctx: &mut TransformContext,
    ) {
    }
    /// Hook called after walking a node's children.
    fn exit(
        &mut self,
        _doc: &mut vuec_ast::AstDocument<K>,
        _node: vuec_ast::NodeId,
        _ctx: &mut TransformContext,
    ) {
    }
}

/// Ordered scheduler for flat node transform passes.
pub struct PassScheduler<N> {
    passes: Vec<Box<dyn TransformPass<N>>>,
}

impl<N> PassScheduler<N> {
    /// Creates an empty pass scheduler.
    pub fn new() -> Self {
        Self { passes: Vec::new() }
    }

    /// Appends a pass to the scheduler.
    pub fn push<P>(&mut self, pass: P)
    where
        P: TransformPass<N> + 'static,
    {
        self.passes.push(Box::new(pass));
    }

    /// Runs all registered passes over `nodes` in insertion order.
    pub fn run(&mut self, nodes: &mut [N], ctx: &mut TransformContext) {
        for node in nodes {
            for pass in &mut self.passes {
                pass.enter(node, ctx);
                pass.exit(node, ctx);
            }
        }
    }
}

impl<N> Default for PassScheduler<N> {
    fn default() -> Self {
        Self::new()
    }
}

/// Walks an [`vuec_ast::AstDocument`] in depth-first order with `pass`.
pub fn walk_document<K, P>(
    doc: &mut vuec_ast::AstDocument<K>,
    pass: &mut P,
    ctx: &mut TransformContext,
) where
    P: DocumentPass<K>,
{
    walk_document_node(doc, pass, ctx, doc.root);
}

fn walk_document_node<K, P>(
    doc: &mut vuec_ast::AstDocument<K>,
    pass: &mut P,
    ctx: &mut TransformContext,
    node: vuec_ast::NodeId,
) where
    P: DocumentPass<K>,
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

#[cfg(test)]
mod tests {
    use super::*;

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

    #[derive(Default)]
    struct CountDocumentPass(usize);

    impl DocumentPass<usize> for CountDocumentPass {
        fn name(&self) -> &'static str {
            "count_document"
        }

        fn enter(
            &mut self,
            _doc: &mut vuec_ast::AstDocument<usize>,
            _node: vuec_ast::NodeId,
            _ctx: &mut TransformContext,
        ) {
            self.0 += 1;
        }
    }

    #[test]
    fn document_walk_is_depth_first() {
        let mut doc = vuec_ast::AstDocument::new(0usize, None);
        let root = doc.root;
        let child = doc.push_child(root, 1usize, None);
        let _grandchild = doc.push_child(child, 2usize, None);
        let mut pass = CountDocumentPass::default();
        let mut ctx = TransformContext::default();
        walk_document(&mut doc, &mut pass, &mut ctx);
        assert_eq!(pass.0, 3);
    }
}
