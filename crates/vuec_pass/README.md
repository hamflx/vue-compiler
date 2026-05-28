# vuec_pass

Shared transform pass types for the vuec Rust Vue compiler.

This crate contains pass-level data structures used by Vue 2 and Vue 3 compiler crates while preserving the AST/HIR/MIR ownership boundaries.

It provides:

- stable `PassOrder` ordering for flat and document passes
- `TransformContext` helper collection, diagnostics, and scope stack state
- `DocumentPass` depth-first enter/exit walking over `vuec_ast::AstDocument`
- Vue 2 module hooks for `preTransformNode`, `transformNode`, `postTransformNode`, and `genData`
- Vue 3 node transform scheduling with post-order LIFO exit callbacks
- Vue 3 directive transform resolution that can keep, extend, or replace default behavior

The crate owns scheduling and context contracts only. Concrete Vue transform semantics belong in the dialect crates.
