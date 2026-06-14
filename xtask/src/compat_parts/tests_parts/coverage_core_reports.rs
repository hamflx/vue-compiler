    #[test]
    fn vue3_core_coverage_report_marks_mixed_and_excludes_rust_backed_counts() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-vue3-core-coverage-{}",
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
                  "name": "F:/repo/prepared/vue3-core/packages/compiler-core/__tests__/compile.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-core/packages/compiler-core/__tests__/scopeId.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-core/packages/compiler-core/__tests__/utils.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-core/packages/compiler-core/__tests__/transforms/vMemo.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-core/packages/compiler-core/__tests__/transforms/transformExpressions.spec.ts",
                  "assertionResults": [
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-core/packages/compiler-core/__tests__/transforms/transformSlotOutlet.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-core/packages/compiler-core/__tests__/transforms/transformText.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-core/packages/compiler-core/__tests__/transforms/vOnce.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-core/packages/compiler-core/__tests__/transforms/vBind.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-core/packages/compiler-core/__tests__/transforms/vModel.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-core/packages/compiler-core/__tests__/transforms/vOn.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-core/packages/compiler-core/__tests__/transforms/vFor.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-core/packages/compiler-core/__tests__/transforms/vIf.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-core/packages/compiler-core/__tests__/transforms/cacheStatic.spec.ts",
                  "assertionResults": [
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }, { "status": "passed" }, { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-core/packages/compiler-core/__tests__/transforms/transformElement.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "failed" }
                  ]
                }
              ]
            }"#,
        )
        .unwrap();
        let execution = ConformanceExecutionResult {
            status: "failed".into(),
            runner: "vitest".into(),
            prepared_root: "prepared".into(),
            prepared_manifest_file: None,
            output_file: report.display().to_string(),
            exit_code: Some(1),
            stdout: String::new(),
            stderr: String::new(),
            counts: ConformanceExecutionCounts {
                total: 287,
                pass: 286,
                fail: 1,
                skip: 0,
                pending: 0,
            },
        };

        let coverage = conformance_coverage_report(
            suite_spec(ConformanceSuite::Vue3Core),
            AliasBackend::Generated,
            Some(&execution),
        );

        assert_eq!(coverage.source, ConformanceCoverageKind::Mixed);
        assert_eq!(coverage.rust_backed_pass, 0);
        assert_eq!(coverage.rust_backed_total, 0);
        assert_eq!(
            coverage
                .counts_by_source
                .get("rust-backed")
                .copied()
                .unwrap_or_default()
                .pass,
            0
        );
        assert_eq!(
            coverage
                .counts_by_source
                .get("mixed")
                .copied()
                .unwrap_or_default()
                .total,
            287
        );
        assert!(coverage
            .files
            .iter()
            .all(|file| file.source == ConformanceCoverageKind::Mixed));
        assert_eq!(
            coverage
                .summary
                .get("hybrid-js-adapter-rust-projection")
                .copied()
                .unwrap_or_default(),
            ConformanceExecutionCounts {
                total: 287,
                pass: 286,
                fail: 1,
                skip: 0,
                pending: 0,
            }
        );
        assert_eq!(
            coverage
                .summary
                .get("rust-bridge-shape-adapter")
                .copied()
                .unwrap_or_default()
                .total,
            0
        );
        assert!(coverage.reason.contains("remaining mixed coverage"));
        assert!(coverage.reason.contains("JavaScript NodeTransform"));
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn vue3_core_coverage_splits_transform_element_assertion_groups() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-vue3-core-transform-element-coverage-{}",
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
                  "name": "F:/repo/prepared/vue3-core/packages/compiler-core/__tests__/transforms/transformElement.spec.ts",
                  "assertionResults": [
                    {
                      "fullName": "compiler: v-for transform value",
                      "status": "passed"
                    },
                    {
                      "fullName": "compiler: v-for codegen basic v-for",
                      "status": "passed"
                    },
                    {
                      "fullName": "compiler: element transform import + resolve component",
                      "status": "passed"
                    },
                    {
                      "fullName": "compiler: element transform static props",
                      "status": "passed"
                    },
                    {
                      "fullName": "compiler: element transform directiveTransforms",
                      "status": "passed"
                    },
                    {
                      "fullName": "compiler: element transform directiveTransform with needRuntime: true",
                      "status": "passed"
                    },
                    {
                      "fullName": "compiler: element transform should process node when node has been replaced",
                      "status": "failed"
                    }
                  ]
                }
              ]
            }"#,
        )
        .unwrap();
        let execution = ConformanceExecutionResult {
            status: "failed".into(),
            runner: "vitest".into(),
            prepared_root: "prepared".into(),
            prepared_manifest_file: None,
            output_file: report.display().to_string(),
            exit_code: Some(1),
            stdout: String::new(),
            stderr: String::new(),
            counts: ConformanceExecutionCounts {
                total: 7,
                pass: 6,
                fail: 1,
                skip: 0,
                pending: 0,
            },
        };

        let coverage = conformance_coverage_report(
            suite_spec(ConformanceSuite::Vue3Core),
            AliasBackend::Generated,
            Some(&execution),
        );

        assert_eq!(coverage.source, ConformanceCoverageKind::Mixed);
        assert_eq!(coverage.files.len(), 3);
        assert_eq!(coverage.rust_backed_total, 0);
        assert_eq!(coverage.rust_backed_pass, 0);
        assert_eq!(
            coverage
                .counts_by_source
                .get("mixed")
                .copied()
                .unwrap_or_default(),
            ConformanceExecutionCounts {
                total: 7,
                pass: 6,
                fail: 1,
                skip: 0,
                pending: 0,
            }
        );
        assert_eq!(
            coverage
                .summary
                .get("hybrid-js-adapter-rust-projection")
                .copied()
                .unwrap_or_default(),
            ConformanceExecutionCounts {
                total: 4,
                pass: 4,
                fail: 0,
                skip: 0,
                pending: 0,
            }
        );
        assert_eq!(
            coverage
                .summary
                .get("mixed-js-callback-boundary")
                .copied()
                .unwrap_or_default(),
            ConformanceExecutionCounts {
                total: 3,
                pass: 2,
                fail: 1,
                skip: 0,
                pending: 0,
            }
        );
        let imported_v_for = coverage
            .files
            .iter()
            .find(|file| file.scope.as_deref() == Some("imported v-for helper"))
            .expect("imported v-for coverage entry");
        assert_eq!(imported_v_for.source, ConformanceCoverageKind::Mixed);
        assert_eq!(
            imported_v_for.provenance.api_surface,
            "suite-only-bridge-command"
        );
        assert_eq!(imported_v_for.counts.total, 2);

        let element_suite = coverage
            .files
            .iter()
            .find(|file| file.scope.as_deref() == Some("element transform rust suite"))
            .expect("element transform coverage entry");
        assert_eq!(element_suite.source, ConformanceCoverageKind::Mixed);
        assert_eq!(
            element_suite.provenance.api_surface,
            "suite-only-bridge-command"
        );
        assert_eq!(element_suite.counts.total, 2);
        assert!(element_suite.reason.contains("transformElementSuite"));
        assert!(element_suite.reason.contains("prepared suite helper"));

        let callback_boundary = coverage
            .files
            .iter()
            .find(|file| file.scope.as_deref() == Some("js callback boundary"))
            .expect("callback boundary coverage entry");
        assert_eq!(callback_boundary.source, ConformanceCoverageKind::Mixed);
        assert_eq!(callback_boundary.counts.total, 3);
        assert!(callback_boundary
            .reason
            .contains("caller-provided JavaScript"));
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn vue3_core_coverage_splits_transform_assertion_groups() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-vue3-core-transform-coverage-{}",
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
                      "fullName": "compiler: transform context state",
                      "status": "passed"
                    },
                    {
                      "fullName": "compiler: transform context.replaceNode",
                      "status": "passed"
                    },
                    {
                      "fullName": "compiler: transform should inject toString helper for interpolations",
                      "status": "passed"
                    },
                    {
                      "fullName": "compiler: transform root codegenNode root v-for",
                      "status": "passed"
                    },
                    {
                      "fullName": "compiler: transform root codegenNode multiple children w/ single root + comments",
                      "status": "failed"
                    }
                  ]
                }
              ]
            }"#,
        )
        .unwrap();
        let execution = ConformanceExecutionResult {
            status: "failed".into(),
            runner: "vitest".into(),
            prepared_root: "prepared".into(),
            prepared_manifest_file: None,
            output_file: report.display().to_string(),
            exit_code: Some(1),
            stdout: String::new(),
            stderr: String::new(),
            counts: ConformanceExecutionCounts {
                total: 5,
                pass: 4,
                fail: 1,
                skip: 0,
                pending: 0,
            },
        };

        let coverage = conformance_coverage_report(
            suite_spec(ConformanceSuite::Vue3Core),
            AliasBackend::Generated,
            Some(&execution),
        );

        assert_eq!(coverage.source, ConformanceCoverageKind::Mixed);
        assert_eq!(coverage.files.len(), 2);
        assert_eq!(coverage.rust_backed_total, 0);
        assert_eq!(coverage.rust_backed_pass, 0);
        assert_eq!(
            coverage
                .summary
                .get("hybrid-js-adapter-rust-projection")
                .copied()
                .unwrap_or_default(),
            ConformanceExecutionCounts {
                total: 3,
                pass: 2,
                fail: 1,
                skip: 0,
                pending: 0,
            }
        );
        assert_eq!(
            coverage
                .summary
                .get("mixed-js-callback-boundary")
                .copied()
                .unwrap_or_default(),
            ConformanceExecutionCounts {
                total: 2,
                pass: 2,
                fail: 0,
                skip: 0,
                pending: 0,
            }
        );

        let transform_suite = coverage
            .files
            .iter()
            .find(|file| file.scope.as_deref() == Some("transform rust suite"))
            .expect("transform rust coverage entry");
        assert_eq!(transform_suite.source, ConformanceCoverageKind::Mixed);
        assert_eq!(
            transform_suite.provenance.api_surface,
            "suite-only-bridge-command"
        );
        assert_eq!(transform_suite.counts.total, 3);
        assert!(transform_suite.reason.contains("transformSuite"));
        assert!(transform_suite.reason.contains("prepared suite helper"));

        let callback_boundary = coverage
            .files
            .iter()
            .find(|file| file.scope.as_deref() == Some("js transform context boundary"))
            .expect("transform context coverage entry");
        assert_eq!(callback_boundary.source, ConformanceCoverageKind::Mixed);
        assert_eq!(callback_boundary.counts.total, 2);
        assert!(callback_boundary.reason.contains("NodeTransform callbacks"));
        let _ = fs::remove_dir_all(temp);
    }
