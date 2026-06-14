//! Vue SFC style compilation support.
//!
//! The crate owns scoped selector rewriting, CSS variable collection and
//! rewriting, lightweight preprocessor support, and source-map result shaping.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use vuec_codegen::{SourceMapArtifact, SourceMapBuilder};
use vuec_diagnostics::Diagnostic;
use vuec_source::{FileId, Span};

mod types;
pub use types::*;

mod compiler;
pub use compiler::compile_style;
pub(crate) use compiler::*;

mod preprocess;
pub(crate) use preprocess::*;

mod scoped;
pub use scoped::rewrite_scoped_selectors;
pub(crate) use scoped::*;

mod css_vars;
pub(crate) use css_vars::*;
pub use css_vars::{
    collect_css_vars, collect_css_vars_with_options, gen_css_var_name, gen_css_var_name_with_style,
    rewrite_css_vars, rewrite_css_vars_with_options, CssVarCollectOptions, CssVarRewriteOptions,
};

mod css_items;
pub(crate) use css_items::*;

mod css_modules;
pub(crate) use css_modules::*;

#[cfg(test)]
mod tests;
