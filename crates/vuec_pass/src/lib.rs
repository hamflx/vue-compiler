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

include!("lib_parts/types_and_context.rs");
include!("lib_parts/generic_schedulers.rs");
include!("lib_parts/vue2_modules.rs");
include!("lib_parts/vue3_node_transforms.rs");
include!("lib_parts/vue3_directive_transforms.rs");
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

    include!("lib_parts/tests.rs");
}
