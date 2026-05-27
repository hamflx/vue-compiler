# vuec_ast

AST, HIR, and MIR arena structures for the vuec Rust Vue compiler.

This crate owns the compiler's internal arena tree model. Its structures must stay aligned with `docs/3.AST_HIR_MIR_DESIGN.md`, including public projection, lowering, and target-split MIR rules.
