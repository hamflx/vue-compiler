#![forbid(unsafe_code)]

include!("compat_parts/types.rs");
include!("compat_parts/option_matrix_cases.rs");
include!("compat_parts/official_sync_runtime.rs");
include!("compat_parts/api_alias_generation.rs");
include!("compat_parts/runner_dependencies.rs");
include!("compat_parts/probe_scripts_runtime.rs");
include!("compat_parts/option_matrix.rs");
include!("compat_parts/conformance_types.rs");
include!("compat_parts/conformance_run_prepare.rs");
include!("compat_parts/conformance_vue27_vue2_shims.rs");
include!("compat_parts/conformance_vue3_core_shims.rs");
include!("compat_parts/conformance_vue3_dom_sfc_ssr_shims.rs");
include!("compat_parts/conformance_reports_utils.rs");
include!("compat_parts/tests.rs");
