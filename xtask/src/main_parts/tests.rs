#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bench_case_kind_uses_cli_targets() {
        assert_eq!(BenchCaseKind::Vue2Template.cli_target(), "vue2-template");
        assert_eq!(BenchCaseKind::Vue3Template.cli_target(), "vue3-template");
        assert_eq!(BenchCaseKind::Vue3Sfc.cli_target(), "vue3-sfc");
        assert_eq!(BenchCaseKind::Vue3Ssr.cli_target(), "vue3-ssr");
    }

    #[test]
    fn windows_executable_detection_prefers_spawnable_shims() {
        assert!(is_windows_executable(r"C:\node\npm.cmd"));
        assert!(is_windows_executable(r"C:\node\pnpm.exe"));
        assert!(!is_windows_executable(r"C:\node\npm"));
    }

    #[test]
    fn proc_status_rss_parser_prefers_high_water_mark() {
        let status = "Name:\tnode\nVmRSS:\t 100 kB\nVmHWM:\t 256 kB\n";
        assert_eq!(parse_proc_status_rss_bytes(status), Some(256 * 1024));
    }

    #[test]
    fn proc_status_rss_parser_falls_back_to_current_rss() {
        let status = "Name:\tnode\nVmRSS:\t 64 kB\n";
        assert_eq!(parse_proc_status_rss_bytes(status), Some(64 * 1024));
    }

    #[test]
    fn official_npm_versions_read_locked_compilers() {
        let lock = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace root")
            .join("compat")
            .join("official-revisions.lock");
        let versions = official_npm_versions(&lock).expect("official versions");
        assert_eq!(versions.vue2, "2.7.16");
        assert_eq!(versions.vue_template_compiler, "2.7.16");
        assert_eq!(versions.vue_compiler_dom, "3.5.34");
        assert_eq!(versions.vue_compiler_sfc, "3.5.34");
        assert_eq!(versions.vue_compiler_ssr, "3.5.34");
    }

    #[test]
    fn sha256_bytes_is_stable() {
        assert_eq!(
            sha256_bytes(b"vuec"),
            "1fc8cc70af7ec7c20b935e8970e8641a6acc9fd856788a44a68507e33c8d561d"
        );
    }

    #[test]
    fn compile_script_profile_version_parses_aliases() {
        assert_eq!(
            CompileScriptProfileVersion::parse("vue2_7").unwrap(),
            CompileScriptProfileVersion::Vue27
        );
        assert_eq!(
            CompileScriptProfileVersion::parse("vue27").unwrap(),
            CompileScriptProfileVersion::Vue27
        );
        assert_eq!(
            CompileScriptProfileVersion::parse("vue3").unwrap(),
            CompileScriptProfileVersion::Vue3
        );
        assert!(CompileScriptProfileVersion::parse("vue2_6").is_err());
    }

    #[test]
    fn compile_script_profile_ast_mode_maps_to_sfc_options() {
        assert_eq!(
            vuec_sfc::SfcScriptAstMode::from(CompileScriptProfileAstMode::None),
            vuec_sfc::SfcScriptAstMode::None
        );
        assert_eq!(
            vuec_sfc::SfcScriptAstMode::from(CompileScriptProfileAstMode::TopLevel),
            vuec_sfc::SfcScriptAstMode::TopLevel
        );
        assert_eq!(
            vuec_sfc::SfcScriptAstMode::from(CompileScriptProfileAstMode::Full),
            vuec_sfc::SfcScriptAstMode::Full
        );
        assert_eq!(
            compile_script_profile_options(vuec_sfc::SfcScriptAstMode::TopLevel).script_ast_mode,
            vuec_sfc::SfcScriptAstMode::TopLevel
        );
    }

    #[test]
    fn compile_script_profile_schema_reports_structural_counts() {
        let fixture = compile_script_profile_fixture_from_source(
            PathBuf::from("ProfileFixture.vue"),
            r#"<template>
  <section>
    <Foo :value="formatCount(count)" />
    <Bar>{{ search }}</Bar>
  </section>
</template>
<script setup lang="ts">
import { computed } from 'vue'
import Foo from './Foo.vue'
import Bar from './Bar.vue'
import { formatCount } from './format'
import type { Item } from './types'

const props = defineProps<{ count: number; item?: Item }>()
const search = computed(() => formatCount(props.count))
</script>"#
                .to_string(),
        )
        .unwrap();

        let result = compile_script_profile_fixture(
            CompileScriptProfileVersion::Vue27,
            &fixture,
            1,
            vuec_sfc::SfcScriptAstMode::None,
        )
        .unwrap();
        assert!(!result.structural_counts.ast_projection_enabled);
        assert_eq!(result.structural_counts.ast_projection_mode, "none");
        assert_eq!(
            result.structural_counts.ast_projection_loc_strategy,
            "not-run"
        );
        assert_eq!(result.structural_counts.ast_projection_statement_count, 0);
        assert_eq!(result.structural_counts.template_usage_scan_count, 1);
        assert_eq!(result.structural_counts.setup_analysis_count, 1);
        assert_eq!(
            result.structural_counts.script_compile_error_analysis_count,
            0
        );

        let report = CompileScriptProfileReport {
            status: "pass".into(),
            version_line: "vue2_7".into(),
            iterations: 1,
            build_profile: compile_script_build_profile().into(),
            script_ast_mode: "none".into(),
            environment: bench_environment(Path::new("compat/official-revisions.lock")),
            fixtures: vec![CompileScriptProfileFixtureReport::from(&fixture)],
            results: vec![result],
        };
        let value = serde_json::to_value(&report).unwrap();
        assert_eq!(value["status"], "pass");
        assert_eq!(value["versionLine"], "vue2_7");
        assert_eq!(value["buildProfile"], compile_script_build_profile());
        assert_eq!(value["scriptAstMode"], "none");
        assert_eq!(
            value["results"][0]["structuralCounts"]["astProjectionEnabled"],
            false
        );
        assert_eq!(
            value["results"][0]["structuralCounts"]["astProjectionMode"],
            "none"
        );
        assert_eq!(
            value["results"][0]["structuralCounts"]["astProjectionLocStrategy"],
            "not-run"
        );
        assert_eq!(
            value["results"][0]["structuralCounts"]["astProjectionStatementCount"],
            0
        );
        assert_eq!(
            value["results"][0]["structuralCounts"]["setupAnalysisCount"],
            1
        );
        assert_eq!(
            value["results"][0]["structuralCounts"]["scriptCompileErrorAnalysisCount"],
            0
        );
        assert_eq!(
            value["results"][0]["structuralCounts"]["templateUsageScanCount"],
            1
        );
        assert!(value["results"][0]["parse"]["medianMicros"]
            .as_u64()
            .is_some());

        let full = compile_script_profile_fixture(
            CompileScriptProfileVersion::Vue27,
            &fixture,
            1,
            vuec_sfc::SfcScriptAstMode::Full,
        )
        .unwrap();
        assert!(full.structural_counts.ast_projection_enabled);
        assert_eq!(full.structural_counts.ast_projection_mode, "full");
        assert_eq!(
            full.structural_counts.ast_projection_loc_strategy,
            "line-index"
        );
        assert!(full.structural_counts.ast_projection_statement_count > 0);
    }

    #[test]
    fn compile_script_profile_structural_counts_reports_line_index_for_ast_projection() {
        let mut compiler = vuec_sfc::SfcCompiler::new();
        let descriptor = compiler
            .parse_vue3("ProfileFixture.vue", "<script setup>const x = 1</script>")
            .descriptor;
        let script =
            compiler.compile_script(&descriptor, vuec_sfc::SfcScriptCompileOptions::default());
        assert!(script.errors.is_empty(), "{:?}", script.errors);

        let counts = compile_script_structural_counts(
            CompileScriptProfileVersion::Vue3,
            &descriptor,
            &script,
            vuec_sfc::SfcScriptAstMode::Full,
        );
        assert!(counts.ast_projection_enabled);
        assert_eq!(counts.ast_projection_mode, "full");
        assert_eq!(counts.ast_projection_loc_strategy, "line-index");
        assert_eq!(
            counts.ast_projection_statement_count,
            script.script_ast.len() + script.script_setup_ast.len()
        );
        assert!(counts.ast_projection_statement_count > 0);
    }

    #[test]
    fn compile_script_profile_comparison_confirms_ast_projection_cost() {
        let full = compile_script_profile_result_fixture("profile.vue", "full", 400, 120, 600);
        let top_level =
            compile_script_profile_result_fixture("profile.vue", "top-level", 220, 40, 360);
        let none = compile_script_profile_result_fixture("profile.vue", "none", 160, 20, 260);

        let comparison =
            compare_compile_script_profile_result("vue2_7", &full, &top_level, &none, 1.2);

        assert!(comparison.ast_projection_problem_confirmed);
        assert_eq!(comparison.full_to_none_compile_ratio, 2.5);
        assert_eq!(comparison.ast_projection_statement_count, 3);
        assert_eq!(comparison.template_usage_scan_count, 1);
        assert_eq!(comparison.setup_analysis_count, 1);

        let report = CompileScriptProfileComparisonReport {
            status: "pass".into(),
            version_line: "vue2_7".into(),
            build_profile: "debug".into(),
            iterations: 20,
            min_full_to_none_compile_ratio: 1.2,
            full_report: "full.json".into(),
            top_level_report: "top-level.json".into(),
            none_report: "none.json".into(),
            comparisons: vec![comparison],
        };
        let markdown = render_compile_script_profile_comparison_markdown(&report);
        assert!(markdown.contains("full/no-AST"));
        assert!(markdown.contains("2.500x"));
    }

    fn compile_script_profile_result_fixture(
        name: &str,
        ast_mode: &str,
        compile_micros: u128,
        serialize_micros: u128,
        total_micros: u128,
    ) -> CompileScriptProfileResult {
        CompileScriptProfileResult {
            name: name.into(),
            version_line: "vue2_7".into(),
            iterations: 20,
            parse: CompileScriptPhaseProfile {
                median_micros: 50,
                p95_micros: 60,
            },
            compile_script: CompileScriptPhaseProfile {
                median_micros: compile_micros,
                p95_micros: compile_micros + 10,
            },
            serialize: CompileScriptPhaseProfile {
                median_micros: serialize_micros,
                p95_micros: serialize_micros + 10,
            },
            total: CompileScriptPhaseProfile {
                median_micros: total_micros,
                p95_micros: total_micros + 10,
            },
            output_bytes: 1,
            errors: 0,
            warnings: 0,
            structural_counts: CompileScriptStructuralCounts {
                ast_projection_enabled: ast_mode != "none",
                ast_projection_mode: ast_mode.into(),
                ast_projection_loc_strategy: if ast_mode == "none" {
                    "not-run".into()
                } else {
                    "line-index".into()
                },
                ast_projection_statement_count: if ast_mode == "none" { 0 } else { 3 },
                template_usage_scan_count: 1,
                setup_analysis_count: 1,
                script_compile_error_analysis_count: 0,
            },
            input_sha256: "sha".into(),
        }
    }

    #[test]
    fn native_artifact_lookup_accepts_platform_subdir() {
        let root = unique_target_test_dir("native-artifact-subdir");
        let artifact = root.join("linux-x64-gnu").join("vuec_napi.node");
        fs::create_dir_all(artifact.parent().expect("artifact parent")).unwrap();
        fs::write(&artifact, b"native").unwrap();

        let found = find_native_artifact(Some(&root), "linux-x64-gnu")
            .expect("artifact lookup")
            .expect("artifact path");
        assert_eq!(found, artifact);
        assert!(find_native_artifact(Some(&root), "darwin-arm64")
            .expect("missing artifact lookup")
            .is_none());
    }

    #[test]
    fn native_artifact_lookup_accepts_flat_node_file() {
        let root = unique_target_test_dir("native-artifact-flat");
        let artifact = root.join("darwin-arm64.node");
        fs::create_dir_all(&root).unwrap();
        fs::write(&artifact, b"native").unwrap();

        let found = find_native_artifact(Some(&root), "darwin-arm64")
            .expect("artifact lookup")
            .expect("artifact path");
        assert_eq!(found, artifact);
    }

    #[test]
    fn native_artifact_lookup_accepts_downloaded_github_artifact_layout() {
        let root = unique_target_test_dir("native-artifact-github-download");
        let artifact = root
            .join("native-Linux-X64")
            .join("linux-x64-gnu")
            .join("vuec_napi.node");
        fs::create_dir_all(artifact.parent().expect("artifact parent")).unwrap();
        fs::write(&artifact, b"native").unwrap();

        let found = find_native_artifact(Some(&root), "linux-x64-gnu")
            .expect("artifact lookup")
            .expect("artifact path");
        assert_eq!(found, artifact);
    }

    #[test]
    fn ci_status_fixture_passes_when_required_jobs_succeed() {
        let root = unique_target_test_dir("ci-status-pass");
        let (runs, jobs) = write_ci_status_fixture(&root, "success", None);

        let report = verify_ci_status(
            Some("hamflx/vue-compiler"),
            Some("abc123"),
            "ci.yml",
            Some(&runs),
            Some(&jobs),
        )
        .expect("ci status report");

        assert_eq!(report.status, "pass");
        assert_eq!(report.summary.total, REQUIRED_CI_JOBS.len() + 1);
        assert_eq!(report.summary.pass, REQUIRED_CI_JOBS.len() + 1);
        assert!(report.violations.is_empty());
    }

    #[test]
    fn ci_status_fixture_fails_when_required_job_fails() {
        let root = unique_target_test_dir("ci-status-fail");
        let (runs, jobs) = write_ci_status_fixture(
            &root,
            "success",
            Some(("Compatibility (macos-latest)", "failure")),
        );

        let report = verify_ci_status(
            Some("hamflx/vue-compiler"),
            Some("abc123"),
            "ci.yml",
            Some(&runs),
            Some(&jobs),
        )
        .expect("ci status report");

        assert_eq!(report.status, "fail");
        assert!(report
            .violations
            .iter()
            .any(|violation| violation.contains("Compatibility (macos-latest)")));
    }

    #[test]
    fn ci_status_fixture_fails_when_completed_run_misses_required_job() {
        let root = unique_target_test_dir("ci-status-missing");
        let runs = root.join("runs.json");
        let jobs = root.join("jobs.json");
        fs::create_dir_all(&root).unwrap();
        fs::write(&runs, ci_runs_fixture("success")).unwrap();
        let jobs_json = json!({
            "jobs": REQUIRED_CI_JOBS
                .iter()
                .filter(|job| **job != "Release Dry Run")
                .map(|job| json!({
                    "name": job,
                    "status": "completed",
                    "conclusion": "success",
                    "html_url": format!("https://example.test/{job}")
                }))
                .collect::<Vec<_>>()
        });
        fs::write(&jobs, serde_json::to_vec_pretty(&jobs_json).unwrap()).unwrap();

        let report = verify_ci_status(
            Some("hamflx/vue-compiler"),
            Some("abc123"),
            "ci.yml",
            Some(&runs),
            Some(&jobs),
        )
        .expect("ci status report");

        assert_eq!(report.status, "fail");
        assert!(report
            .items
            .iter()
            .any(|item| item.target == "job:Release Dry Run"
                && item.status == compat::ReportStatus::Fail));
    }

    #[test]
    fn ci_status_fixture_is_pending_when_workflow_is_not_completed() {
        let root = unique_target_test_dir("ci-status-pending");
        let runs = root.join("runs.json");
        let jobs = root.join("jobs.json");
        fs::create_dir_all(&root).unwrap();
        fs::write(&runs, ci_runs_fixture_with("in_progress", None)).unwrap();
        let jobs_json = json!({
            "jobs": REQUIRED_CI_JOBS
                .iter()
                .map(|job| json!({
                    "name": job,
                    "status": "queued",
                    "conclusion": null,
                    "html_url": format!("https://example.test/{job}")
                }))
                .collect::<Vec<_>>()
        });
        fs::write(&jobs, serde_json::to_vec_pretty(&jobs_json).unwrap()).unwrap();

        let report = verify_ci_status(
            Some("hamflx/vue-compiler"),
            Some("abc123"),
            "ci.yml",
            Some(&runs),
            Some(&jobs),
        )
        .expect("ci status report");

        assert_eq!(report.status, "pending");
        assert_eq!(report.summary.pending, REQUIRED_CI_JOBS.len() + 1);
        assert!(report.violations.is_empty());
    }

    #[test]
    fn github_remote_parser_accepts_common_origin_shapes() {
        assert_eq!(
            parse_github_remote_repo("git@github.com:hamflx/vue-compiler.git").as_deref(),
            Some("hamflx/vue-compiler")
        );
        assert_eq!(
            parse_github_remote_repo("https://github.com/hamflx/vue-compiler.git").as_deref(),
            Some("hamflx/vue-compiler")
        );
        assert_eq!(
            parse_github_remote_repo("ssh://git@github.com/hamflx/vue-compiler.git").as_deref(),
            Some("hamflx/vue-compiler")
        );
        assert!(parse_github_remote_repo("https://example.com/hamflx/vue-compiler").is_none());
    }

    fn unique_target_test_dir(name: &str) -> PathBuf {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace root");
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock before unix epoch")
            .as_nanos();
        workspace
            .join("target")
            .join("xtask-tests")
            .join(format!("{name}-{}-{stamp}", std::process::id()))
    }

    fn write_ci_status_fixture(
        root: &Path,
        default_conclusion: &str,
        override_job: Option<(&str, &str)>,
    ) -> (PathBuf, PathBuf) {
        fs::create_dir_all(root).unwrap();
        let runs = root.join("runs.json");
        let jobs = root.join("jobs.json");
        fs::write(&runs, ci_runs_fixture(default_conclusion)).unwrap();
        let jobs_json = json!({
            "jobs": REQUIRED_CI_JOBS
                .iter()
                .map(|job| {
                    let conclusion = override_job
                        .filter(|(name, _)| name == job)
                        .map(|(_, conclusion)| conclusion)
                        .unwrap_or(default_conclusion);
                    json!({
                        "name": job,
                        "status": "completed",
                        "conclusion": conclusion,
                        "html_url": format!("https://example.test/{job}")
                    })
                })
                .collect::<Vec<_>>()
        });
        fs::write(&jobs, serde_json::to_vec_pretty(&jobs_json).unwrap()).unwrap();
        (runs, jobs)
    }

    fn ci_runs_fixture(conclusion: &str) -> Vec<u8> {
        ci_runs_fixture_with("completed", Some(conclusion))
    }

    fn ci_runs_fixture_with(status: &str, conclusion: Option<&str>) -> Vec<u8> {
        serde_json::to_vec_pretty(&json!({
            "workflow_runs": [{
                "id": 42,
                "head_sha": "abc123",
                "status": status,
                "conclusion": conclusion,
                "html_url": "https://example.test/run/42",
                "run_number": 7
            }]
        }))
        .unwrap()
    }
}
