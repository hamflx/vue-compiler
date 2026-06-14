//! Project automation for compatibility, release, benchmark, and verification gates.
//!
//! The binary hosts deterministic `cargo xtask ...` commands used by the
//! development plan. It orchestrates official fixture sync, API/option/output
//! contract checks, conformance reports, release documentation gates, and
//! targeted verification helpers without owning compiler semantics.

#![forbid(unsafe_code)]

mod compat;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use compat::{
    audit_option_matrix, diff_api, export_api, generate_option_matrix, generate_output_contract,
    prepare_runtime_smoke, run_conformance, run_napi_conformance, run_napi_option_matrix,
    run_napi_output_contract, run_option_matrix, run_output_contract, summarize_compat,
    sync_official_tests, verify_npm_alias, verify_official_lock, verify_vue27_project_corpus,
    verify_vue2_project_corpus, ConformanceArgs, SelectionArgs, Vue27ProjectCorpusArgs,
    Vue2ProjectCorpusArgs,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, ExitStatus, Stdio};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use std::{thread, time::Duration};
use sysinfo::{Pid, ProcessesToUpdate, System};

include!("main_parts/cli.rs");
include!("main_parts/verify_napi.rs");
include!("main_parts/verify_wasm_cli.rs");
include!("main_parts/verify_release_metadata_ci.rs");
include!("main_parts/release_dry_run.rs");
include!("main_parts/bench_profile.rs");
include!("main_parts/smoke_runners.rs");
include!("main_parts/bench_types_and_fixtures.rs");
include!("main_parts/build_packaging.rs");
include!("main_parts/tests.rs");
