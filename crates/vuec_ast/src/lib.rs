//! Canonical AST, HIR, and target-split MIR data structures.
//!
//! This crate is the structural authority for compiler documents. Internal
//! trees use [`AstDocument`] arenas with stable [`NodeId`] handles, public
//! projection is explicit, and MIR is split by output target instead of using a
//! single generic runtime-call IR.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use vuec_source::{FileId, Span};

include!("ast_parts/document.rs");
include!("ast_parts/spans.rs");
include!("ast_parts/cst_vue2.rs");
include!("ast_parts/vue3.rs");
include!("ast_parts/hir.rs");
include!("ast_parts/mir.rs");
include!("ast_parts/tests.rs");
