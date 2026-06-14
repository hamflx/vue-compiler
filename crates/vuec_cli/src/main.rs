//! Command line interface for the Rust Vue compiler.
//!
//! The binary exposes release-facing commands for Vue 2 template compilation,
//! Vue 3 DOM/SSR template compilation, SFC parsing and compilation, batch
//! compilation, conformance summaries, and benchmark execution.

#![forbid(unsafe_code)]

include!("main_parts/cli_types.rs");
include!("main_parts/main_dispatch.rs");
include!("main_parts/command_handlers.rs");
include!("main_parts/batch_compile.rs");
include!("main_parts/template_emit_io.rs");
include!("main_parts/diagnostics_rendering.rs");
include!("main_parts/tests.rs");
