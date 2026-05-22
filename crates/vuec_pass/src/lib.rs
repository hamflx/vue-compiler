#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use vuec_diagnostics::{Diagnostic, DiagnosticSink};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransformContext {
    pub helpers: BTreeSet<String>,
    pub scope_depth: usize,
    #[serde(skip)]
    pub diagnostics: DiagnosticSink,
}

impl TransformContext {
    pub fn report(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    pub fn add_helper(&mut self, helper: impl Into<String>) -> bool {
        self.helpers.insert(helper.into())
    }
}

pub trait TransformPass<N> {
    fn name(&self) -> &'static str;
    fn enter(&mut self, _node: &mut N, _ctx: &mut TransformContext) {}
    fn exit(&mut self, _node: &mut N, _ctx: &mut TransformContext) {}
}

pub struct PassScheduler<N> {
    passes: Vec<Box<dyn TransformPass<N>>>,
}

impl<N> PassScheduler<N> {
    pub fn new() -> Self {
        Self { passes: Vec::new() }
    }

    pub fn push<P>(&mut self, pass: P)
    where
        P: TransformPass<N> + 'static,
    {
        self.passes.push(Box::new(pass));
    }

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
            ctx.add_helper(format!("node-{node}"));
        }
    }

    #[test]
    fn scheduler_runs_passes() {
        let mut scheduler = PassScheduler::new();
        scheduler.push(CountPass::default());
        let mut nodes = vec![1, 2, 3];
        let mut ctx = TransformContext::default();
        scheduler.run(&mut nodes, &mut ctx);
        assert_eq!(ctx.helpers.len(), 3);
    }
}
