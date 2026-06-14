#[cfg(test)]
mod tests {
    include!("tests_parts/manifest_and_project.rs");
    include!("tests_parts/api_alias_option_contract.rs");
    include!("tests_parts/report_metadata_summary.rs");
    include!("tests_parts/coverage_core_reports.rs");
    include!("tests_parts/vue3_core_shims_rewrites.rs");
    include!("tests_parts/alias_runtime_dom_sfc_ssr.rs");
    include!("tests_parts/dependencies_and_api_diff_helpers.rs");
}
