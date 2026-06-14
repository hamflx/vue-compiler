    #[test]
    fn report_value_status_fails_on_failed_conformance_smoke() {
        let value = serde_json::json!({
            "counts": { "total": 1, "pass": 0, "pending": 1, "fail": 0 },
            "smoke": [{ "status": "fail", "request": "@vue/compiler-core" }]
        });
        assert_eq!(report_value_status(&value), ReportStatus::Fail);
    }

    #[test]
    fn report_metadata_records_lock_versions_and_rust_commit() {
        let lock = OfficialRevisionsLock {
            vue2_6: BaselineLock {
                repo: "https://github.com/vuejs/vue".into(),
                rev: "612fb89547711cacb030a3893a0065b785802860".into(),
                npm: BTreeMap::new(),
                exports: BTreeMap::new(),
            },
            vue2_7: BaselineLock {
                repo: "https://github.com/vuejs/vue".into(),
                rev: "13f4e7dc03e2caed900ac70ff8b8fe58dda45663".into(),
                npm: BTreeMap::new(),
                exports: BTreeMap::new(),
            },
            vue3: BaselineLock {
                repo: "https://github.com/vuejs/core".into(),
                rev: "57545e958ae28ed17aa9e0ed321abcd8dc99f752".into(),
                npm: BTreeMap::new(),
                exports: BTreeMap::new(),
            },
        };
        let metadata =
            ReportMetadata::capture().with_lock_context(Some("lock-hash".into()), Some(&lock));

        assert_eq!(metadata.lock_hash.as_deref(), Some("lock-hash"));
        assert_eq!(
            metadata.official_commits.get("vue2_6").map(String::as_str),
            Some("612fb89547711cacb030a3893a0065b785802860")
        );
        assert_eq!(
            metadata.official_commits.get("vue2_7").map(String::as_str),
            Some("13f4e7dc03e2caed900ac70ff8b8fe58dda45663")
        );
        assert_eq!(
            metadata.official_commits.get("vue3").map(String::as_str),
            Some("57545e958ae28ed17aa9e0ed321abcd8dc99f752")
        );
        assert!(metadata
            .rust_compiler_commit
            .as_deref()
            .map(is_commit_sha)
            .unwrap_or(true));
    }

    #[test]
    fn aggregate_artifact_status_ignores_metadata_payload() {
        let value = serde_json::json!({
            "command": "run_conformance",
            "metadata": {
                "lock_hash": "lock-hash",
                "os": "linux",
                "rustc": "rustc 1.0.0",
                "node": "v22.0.0",
                "official_commits": { "vue3": "57545e958ae28ed17aa9e0ed321abcd8dc99f752" },
                "rust_compiler_commit": "0123456789012345678901234567890123456789",
                "created_unix": 1
            },
            "counts": { "total": 1, "pass": 1, "pending": 0, "fail": 0 },
            "coverage": {
                "source": "rust-backed",
                "counts_by_source": {
                    "rust-backed": { "total": 1, "pass": 1, "pending": 0, "fail": 0, "skip": 0 },
                    "mixed": { "total": 0, "pass": 0, "pending": 0, "fail": 0, "skip": 0 },
                    "shim-backed": { "total": 0, "pass": 0, "pending": 0, "fail": 0, "skip": 0 }
                }
            }
        });

        assert_eq!(report_value_status(&value), ReportStatus::Pass);
    }

    #[test]
    fn summarize_compat_reports_corpus_evidence_without_overwriting_official_gate() {
        let root = std::env::temp_dir().join(format!(
            "vuec-xtask-summary-evidence-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();

        let lock_path = root.join("compat").join("official-revisions.lock");
        fs::create_dir_all(lock_path.parent().unwrap()).unwrap();
        fs::write(
            &lock_path,
            r#"[vue2_6]
repo = "https://github.com/vuejs/vue"
rev = "0123456789012345678901234567890123456789"

[vue2_6.npm]
vue = "2.6.14"
"vue-template-compiler" = "2.6.14"

[vue2_7]
repo = "https://github.com/vuejs/vue"
rev = "abcdefabcdefabcdefabcdefabcdefabcdefabcd"

[vue2_7.npm]
vue = "2.7.16"
"vue-template-compiler" = "2.7.16"

[vue2_7.exports]
"vue/compiler-sfc" = "./compiler-sfc/index.js"

[vue3]
repo = "https://github.com/vuejs/core"
rev = "57545e958ae28ed17aa9e0ed321abcd8dc99f752"

[vue3.npm]
vue = "3.5.34"
"@vue/compiler-core" = "3.5.34"
"@vue/compiler-dom" = "3.5.34"
"@vue/compiler-ssr" = "3.5.34"
"@vue/compiler-sfc" = "3.5.34"
"#,
        )
        .unwrap();

        let lock_hash = file_sha256(&lock_path).unwrap();
        let conformance_root = root.join("target").join("conformance").join(lock_hash);
        fs::create_dir_all(&conformance_root).unwrap();
        write_json(
            &conformance_root.join("option-matrix.json"),
            &serde_json::json!({ "status": "pass" }),
        )
        .unwrap();
        write_json(
            &conformance_root.join("output-contract.json"),
            &serde_json::json!({ "status": "pass" }),
        )
        .unwrap();

        for target in all_targets() {
            for side in [ApiManifestSide::Official, ApiManifestSide::Rust] {
                let path = root.join(target.relative_api_manifest_path(side.as_str()));
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                write_json(&path, &serde_json::json!({ "status": "pass" })).unwrap();
            }
            write_json(
                &conformance_root.join(conformance_report_name(*target)),
                &serde_json::json!({
                    "status": "pass",
                    "counts": { "total": 1, "pass": 1, "pending": 0, "fail": 0 }
                }),
            )
            .unwrap();
        }

        let vue26_corpus = root
            .join("target")
            .join("external")
            .join("vue2-project-corpus")
            .join("verify_vue2_project_corpus.json");
        fs::create_dir_all(vue26_corpus.parent().unwrap()).unwrap();
        write_json(
            &vue26_corpus,
            &serde_json::json!({
                "command": "verify_vue2_project_corpus",
                "project_vue_version": "vue2_6",
                "compiler_version_line": "vue2_7",
                "counts": { "total": 15, "pass": 14, "pending": 0, "fail": 1 }
            }),
        )
        .unwrap();
        let vue27_corpus = root
            .join("target")
            .join("external")
            .join("vue27-project-corpus")
            .join("verify_vue27_project_corpus.json");
        fs::create_dir_all(vue27_corpus.parent().unwrap()).unwrap();
        write_json(
            &vue27_corpus,
            &serde_json::json!({
                "command": "verify_vue27_project_corpus",
                "project_vue_version": "vue2_7",
                "compiler_version_line": "vue2_7",
                "counts": { "total": 15, "pass": 15, "pending": 0, "fail": 0 }
            }),
        )
        .unwrap();

        let report = summarize_compat_at_root(
            true,
            &PathBuf::from("compat/official-revisions.lock"),
            &root,
        );

        assert_eq!(report.status, "pass");
        assert_eq!(report.items.len(), all_targets().len());
        assert!(report
            .items
            .iter()
            .all(|item| !item.target.starts_with("production-corpus::")));

        let official_group = report
            .evidence_groups
            .iter()
            .find(|group| group.name == "official-conformance-gate")
            .expect("official evidence group");
        assert_eq!(official_group.summary.total, all_targets().len());
        assert_eq!(official_group.summary.pass, all_targets().len());

        let corpus_group = report
            .evidence_groups
            .iter()
            .find(|group| group.name == "production-corpus-evidence")
            .expect("production corpus evidence group");
        assert_eq!(corpus_group.summary.total, 2);
        assert_eq!(corpus_group.summary.pass, 1);
        assert_eq!(corpus_group.summary.fail, 1);
        assert!(corpus_group
            .items
            .iter()
            .any(|item| item.detail.contains("not official conformance")));
        assert!(report
            .note
            .as_deref()
            .is_some_and(|note| note.contains("production corpus evidence is reported")));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn vitest_counts_treat_failed_suite_without_tests_as_failure() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-vitest-counts-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&temp).unwrap();
        let report = temp.join("vitest-report.json");
        fs::write(
            &report,
            r#"{
              "numTotalTestSuites": 0,
              "numFailedTestSuites": 1,
              "numTotalTests": 0,
              "numPassedTests": 0,
              "numFailedTests": 0,
              "numPendingTests": 0,
              "numTodoTests": 0
            }"#,
        )
        .unwrap();

        let counts = read_vitest_counts(&report).unwrap();
        assert_eq!(counts.total, 1);
        assert_eq!(counts.fail, 1);
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn vitest_provenance_sidecars_merge_assertion_markers() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-vitest-provenance-sidecar-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&temp).unwrap();
        let report = temp.join("vitest-report.json");
        fs::write(
            &report,
            r#"{
              "testResults": [
                {
                  "name": "F:/repo/prepared/vue3-core/packages/compiler-core/__tests__/transform.spec.ts",
                  "assertionResults": [
                    {
                      "ancestorTitles": ["compiler: transform"],
                      "title": "context.replaceNode",
                      "status": "passed",
                      "coverageProvenance": ["bridge:vue3.core.transformSuite"]
                    },
                    {
                      "ancestorTitles": ["compiler: transform"],
                      "title": "context.removeNode",
                      "status": "passed"
                    }
                  ]
                }
              ]
            }"#,
        )
        .unwrap();
        fs::write(
            temp.join("vuec-provenance.123.ndjson"),
            r#"{"testPath":"F:\\repo\\prepared\\vue3-core\\packages\\compiler-core\\__tests__\\transform.spec.ts","fullName":"compiler: transform > context.replaceNode","title":"compiler: transform > context.replaceNode","markers":["bridge:vue3.core.transformSuite","callback.nodeTransform","js.transformContext.replaceNode"]}
"#,
        )
        .unwrap();

        merge_vitest_provenance_sidecars(&report, &report, &temp).unwrap();

        let merged = read_json::<serde_json::Value>(&report).unwrap();
        let markers = merged["testResults"][0]["assertionResults"][0]["coverageProvenance"]
            .as_array()
            .unwrap();
        assert_eq!(
            markers
                .iter()
                .filter(|marker| marker.as_str() == Some("bridge:vue3.core.transformSuite"))
                .count(),
            1
        );
        assert!(markers
            .iter()
            .any(|marker| marker.as_str() == Some("callback.nodeTransform")));
        assert!(markers
            .iter()
            .any(|marker| marker.as_str() == Some("js.transformContext.replaceNode")));
        assert!(merged["testResults"][0]["assertionResults"][1]
            .get("coverageProvenance")
            .is_none());
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn runtime_markers_keep_bridge_evidence_and_callback_priority() {
        let base = ConformanceCoverageProvenance::new(
            "prepared-official",
            "hybrid-js-adapter-rust-projection",
            "suite-only-bridge-command",
            &["suite-helper"],
            &[],
        );

        let bridge_only = base
            .clone()
            .with_runtime_markers(vec!["bridge:vue3.core.transformElementSuite".into()]);
        assert!(bridge_only
            .bridge_commands
            .iter()
            .any(|command| command == "vue3.core.transformElementSuite"));
        assert_eq!(
            bridge_only.execution_path,
            "hybrid-js-adapter-rust-projection"
        );
        assert_eq!(bridge_only.legacy_source(), ConformanceCoverageKind::Mixed);

        let callback = base.with_runtime_markers(vec![
            "bridge:vue3.core.transformElementSuite".into(),
            "js.transformElement.props".into(),
            "callback.directiveTransform".into(),
        ]);
        assert_eq!(callback.execution_path, "mixed-js-callback-boundary");
        assert_eq!(callback.legacy_source(), ConformanceCoverageKind::Mixed);
        assert!(callback
            .adapter_roles
            .iter()
            .any(|role| role == "callback-materialization"));
        assert!(callback
            .adapter_roles
            .iter()
            .any(|role| role == "semantic-shim"));
        assert!(callback
            .runtime_markers
            .iter()
            .any(|marker| marker == "callback.directiveTransform"));
    }

    #[test]
    fn bridge_registry_drives_coverage_api_surface_without_overriding_source_boundaries() {
        assert_eq!(
            canonical_bridge_api_surface(&["sfc.compileScript".into()], "public-api"),
            "public-command"
        );
        assert_eq!(
            canonical_bridge_api_surface(
                &["vue3.core.transformElementProps".into()],
                "projection-command"
            ),
            "projection-command"
        );
        assert_eq!(
            canonical_bridge_api_surface(&["vue3.core.transformBindSuite".into()], "public-api"),
            "suite-only-bridge-command"
        );
        assert_eq!(
            canonical_bridge_api_surface(
                &["vue3.ssr.compile".into()],
                "mixed-official-source-boundary"
            ),
            "internal-helper-import"
        );

        let vue3_sfc = prepared_test_manifest_for_suite(suite_spec(ConformanceSuite::Vue3Sfc));
        let compile_template =
            ConformanceCoverageProvenance::from_prepared_expectation(manifest_entry(
                &vue3_sfc,
                "packages/compiler-sfc/__tests__/compileTemplate.spec.ts",
            ));
        assert_eq!(compile_template.api_surface, "public-command");

        let resolve_type =
            ConformanceCoverageProvenance::from_prepared_expectation(manifest_entry(
                &vue3_sfc,
                "packages/compiler-sfc/__tests__/compileScript/resolveType.spec.ts",
            ));
        assert_eq!(resolve_type.api_surface, "projection-command");

        let source_boundary = ConformanceCoverageProvenance::from_prepared_expectation(
            manifest_entry(&vue3_sfc, "packages/compiler-sfc/src/**"),
        );
        assert_eq!(source_boundary.api_surface, "internal-helper-import");

        let vue3_core = prepared_test_manifest_for_suite(suite_spec(ConformanceSuite::Vue3Core));
        let suite_helper =
            ConformanceCoverageProvenance::from_prepared_expectation(manifest_entry(
                &vue3_core,
                "packages/compiler-core/__tests__/transforms/vBind.spec.ts",
            ));
        assert_eq!(suite_helper.api_surface, "suite-only-bridge-command");
    }

    #[test]
    fn conformance_item_detail_uses_execution_counts() {
        let readiness = conformance_readiness(
            suite_spec(ConformanceSuite::Vue3Core),
            AliasBackend::Generated,
        );
        let execution = ConformanceExecutionResult {
            status: "failed".into(),
            runner: "vitest".into(),
            prepared_root: "prepared".into(),
            prepared_manifest_file: None,
            output_file: "report.json".into(),
            exit_code: Some(1),
            stdout: String::new(),
            stderr: String::new(),
            counts: ConformanceExecutionCounts {
                total: 618,
                pass: 9,
                fail: 609,
                skip: 0,
                pending: 0,
            },
        };

        assert_eq!(
            conformance_item_detail(20, &readiness, Some(&execution)),
            "9/618 official tests passed, 609 failed, 0 skipped, 0 pending"
        );
    }
