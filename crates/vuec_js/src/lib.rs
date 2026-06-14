//! JavaScript parsing and side-store support for Vue compiler ASTs.
//!
//! Vue AST/HIR/MIR nodes store JavaScript handles instead of embedding parser
//! trees directly. This crate owns the source registry, Oxc parser entry
//! points, and small summary helpers used by SFC and template compilation.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

include!("lib_parts/types_and_sources.rs");
include!("lib_parts/ast_store.rs");
include!("lib_parts/diagnostics_and_rewrite.rs");
include!("lib_parts/vue2_filters.rs");
include!("lib_parts/scanner_and_summary.rs");
include!("lib_parts/tests.rs");
