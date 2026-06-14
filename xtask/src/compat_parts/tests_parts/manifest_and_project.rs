    use super::*;

    fn manifest_entry<'a>(
        manifest: &'a PreparedTestManifest,
        prepared_path: &str,
    ) -> &'a PreparedTestManifestEntry {
        manifest
            .entries
            .iter()
            .find(|entry| entry.prepared_path == prepared_path)
            .unwrap_or_else(|| panic!("missing manifest entry for {prepared_path}"))
    }

    fn assert_manifest_command(entry: &PreparedTestManifestEntry, command: &str) {
        assert!(
            entry
                .related_bridge_commands
                .iter()
                .any(|existing| existing == command),
            "{} should include bridge command {command}",
            entry.prepared_path
        );
    }

    fn write_test_manifest(temp: &Path, manifest: PreparedTestManifest) -> Option<String> {
        let path = temp.join("prepared-test-manifest.json");
        write_json(&path, &manifest).unwrap();
        Some(path.display().to_string())
    }

    fn alias_runtime_fragment<'a>(
        fragments: &'a [AliasRuntimeFragmentManifestEntry],
        name: &str,
    ) -> &'a AliasRuntimeFragmentManifestEntry {
        fragments
            .iter()
            .find(|fragment| fragment.name == name)
            .unwrap_or_else(|| panic!("missing alias runtime fragment {name}"))
    }

    #[test]
    fn prepared_manifest_writes_for_all_generated_alias_suites() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-prepared-manifest-all-suites-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&temp).unwrap();

        for suite in [
            ConformanceSuite::Vue2Compiler,
            ConformanceSuite::Vue27Compiler,
            ConformanceSuite::Vue27Sfc,
            ConformanceSuite::Vue3Core,
            ConformanceSuite::Vue3Dom,
            ConformanceSuite::Vue3Sfc,
            ConformanceSuite::Vue3Ssr,
        ] {
            let spec = suite_spec(suite);
            let prepared_root = temp.join(spec.name);
            fs::create_dir_all(&prepared_root).unwrap();
            write_prepared_test_manifest_for_suite(spec, &prepared_root).unwrap();

            let manifest_path = prepared_test_manifest_path(&prepared_root);
            assert!(manifest_path.exists(), "{} manifest exists", spec.name);

            let report = prepared_test_manifest_report(&manifest_path.display().to_string())
                .unwrap_or_else(|| panic!("{} manifest report", spec.name));
            assert_eq!(report.official_test_origin, "prepared-official");
            assert!(report.entry_count > 0, "{} manifest has entries", spec.name);
        }

        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn alias_runtime_fragments_have_stable_roles_and_source_anchors() {
        let fragments = alias_runtime_fragment_manifest_entries();

        assert_eq!(fragments.len(), ALIAS_RUNTIME_FRAGMENT_SPECS.len());
        assert!(fragments
            .windows(2)
            .all(|window| window[0].order < window[1].order));

        for fragment in &fragments {
            assert!(
                ALIAS_RUNTIME_JS.contains(&fragment.source_anchor),
                "{} should have source anchor {}",
                fragment.name,
                fragment.source_anchor
            );
        }

        let roles: Vec<_> = fragments
            .iter()
            .map(|fragment| fragment.role.as_str())
            .collect();
        for role in [
            "package-api-adapter",
            "bridge-shape-adapter",
            "callback-boundary",
            "semantic-js-shim",
            "suite-helper",
        ] {
            assert!(roles.contains(&role), "missing alias runtime role {role}");
        }

        let semantic = alias_runtime_fragment(&fragments, "vue3-core-runtime");
        assert_eq!(semantic.role, "semantic-js-shim");
        assert_eq!(semantic.execution_path, "shim-backed-semantic-js");
        assert!(semantic
            .migration_note
            .as_deref()
            .is_some_and(|note| note.contains("Rust compiler-core projections")));

        let callback = alias_runtime_fragment(&fragments, "js-callback-materialization");
        assert_eq!(callback.role, "callback-boundary");
        assert_eq!(callback.execution_path, "mixed-js-callback-boundary");
    }

    #[test]
    fn prepared_manifest_reports_alias_runtime_fragment_roles() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-alias-runtime-fragment-report-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&temp).unwrap();
        write_prepared_test_manifest_for_suite(suite_spec(ConformanceSuite::Vue3Core), &temp)
            .unwrap();

        let manifest: PreparedTestManifest =
            read_json(&prepared_test_manifest_path(&temp)).expect("prepared manifest json");
        assert_eq!(
            manifest.alias_runtime_fragments.len(),
            ALIAS_RUNTIME_FRAGMENT_SPECS.len()
        );
        assert_eq!(
            alias_runtime_fragment(&manifest.alias_runtime_fragments, "node-bridge-call").role,
            "bridge-shape-adapter"
        );
        assert_eq!(
            alias_runtime_fragment(&manifest.alias_runtime_fragments, "runtime-entrypoint").role,
            "suite-helper"
        );

        let report = prepared_test_manifest_report(
            &prepared_test_manifest_path(&temp).display().to_string(),
        )
        .expect("prepared manifest report");
        assert_eq!(
            report.alias_runtime_fragments.len(),
            ALIAS_RUNTIME_FRAGMENT_SPECS.len()
        );
        assert_eq!(
            alias_runtime_fragment(&report.alias_runtime_fragments, "public-package-shapes").role,
            "package-api-adapter"
        );
        assert!(report
            .alias_runtime_fragments
            .iter()
            .any(
                |fragment| fragment.role == "semantic-js-shim" && fragment.migration_note.is_some()
            ));

        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn vue3_core_prepared_manifest_records_suite_helpers() {
        let manifest = prepared_test_manifest_for_suite(suite_spec(ConformanceSuite::Vue3Core));

        assert_eq!(manifest.schema_version, 1);
        assert_eq!(manifest.suite, "vue3-core");
        assert_eq!(manifest.official_test_origin, "prepared-official");

        let v_bind = manifest_entry(
            &manifest,
            "packages/compiler-core/__tests__/transforms/vBind.spec.ts",
        );
        assert_eq!(v_bind.rewrite_kind, "test-spec-suite-helper-reroute");
        assert_eq!(
            v_bind.helper_path.as_deref(),
            Some("packages/compiler-core/__tests__/transforms/vBind.rust-api.ts")
        );
        assert_manifest_command(v_bind, "vue3.core.transformBindSuite");
        assert_eq!(
            v_bind.expected_provenance.api_surface,
            "suite-only-bridge-command"
        );

        let transform_element = manifest_entry(
            &manifest,
            "packages/compiler-core/__tests__/transforms/transformElement.spec.ts",
        );
        assert_manifest_command(transform_element, "vue3.core.transformElementSuite");
        assert_manifest_command(transform_element, "vue3.core.transformForSuite");

        let transform_helper = manifest_entry(
            &manifest,
            "packages/compiler-core/__tests__/transform.rust-api.ts",
        );
        assert_eq!(transform_helper.rewrite_kind, "generated-suite-helper");
        assert_manifest_command(transform_helper, "vue3.core.transformSuite");

        let runner_glob =
            manifest_entry(&manifest, "packages/compiler-core/__tests__/**/*.spec.ts");
        assert_eq!(runner_glob.rewrite_kind, "runner-include-glob");
    }

    #[test]
    fn vue3_sfc_prepared_manifest_records_public_api_rewrites() {
        let manifest = prepared_test_manifest_for_suite(suite_spec(ConformanceSuite::Vue3Sfc));

        assert_eq!(manifest.suite, "vue3-sfc");
        assert_eq!(manifest.official_test_origin, "prepared-official");

        let compile_template = manifest_entry(
            &manifest,
            "packages/compiler-sfc/__tests__/compileTemplate.spec.ts",
        );
        assert_eq!(
            compile_template.helper_path.as_deref(),
            Some("packages/compiler-sfc/__tests__/utils.public-api.ts")
        );
        assert_manifest_command(compile_template, "sfc.compileTemplate");
        assert_manifest_command(compile_template, "sfc.compileScript");

        let resolve_type = manifest_entry(
            &manifest,
            "packages/compiler-sfc/__tests__/compileScript/resolveType.spec.ts",
        );
        assert_eq!(
            resolve_type.helper_path.as_deref(),
            Some("packages/compiler-sfc/__tests__/compileScript/resolveType.rust-api.ts")
        );
        assert_manifest_command(resolve_type, "sfc.resolveType");

        let template_transform_helper = manifest_entry(
            &manifest,
            "packages/compiler-sfc/__tests__/templateTransforms.public-api.ts",
        );
        assert_eq!(
            template_transform_helper.rewrite_kind,
            "generated-test-helper"
        );
        assert_manifest_command(template_transform_helper, "sfc.compileTemplate");

        let source_boundary = manifest_entry(&manifest, "packages/compiler-sfc/src/**");
        assert_eq!(
            source_boundary.rewrite_kind,
            "copied-official-source-boundary"
        );
        assert_eq!(
            source_boundary.expected_provenance.api_surface,
            "mixed-official-source-boundary"
        );
    }

    #[test]
    fn vue3_ssr_prepared_manifest_records_public_compile_rewrites() {
        let manifest = prepared_test_manifest_for_suite(suite_spec(ConformanceSuite::Vue3Ssr));

        assert_eq!(manifest.suite, "vue3-ssr");
        assert_eq!(manifest.official_test_origin, "prepared-official");

        let ssr_text = manifest_entry(&manifest, "packages/compiler-ssr/__tests__/ssrText.spec.ts");
        assert_eq!(
            ssr_text.helper_path.as_deref(),
            Some("packages/compiler-ssr/__tests__/utils.rust-ssr-text.ts")
        );
        assert_manifest_command(ssr_text, "vue3.ssr.compile");

        let ssr_v_if = manifest_entry(&manifest, "packages/compiler-ssr/__tests__/ssrVIf.spec.ts");
        assert_eq!(
            ssr_v_if.rewrite_kind,
            "test-spec-public-ssr-compile-import-rewrite"
        );
        assert_manifest_command(ssr_v_if, "vue3.ssr.compile");

        let helper = manifest_entry(
            &manifest,
            "packages/compiler-ssr/__tests__/utils.rust-ssr-text.ts",
        );
        assert_eq!(helper.rewrite_kind, "generated-public-ssr-compile-helper");
        assert_manifest_command(helper, "vue3.ssr.compile");

        let ssr_source = manifest_entry(&manifest, "packages/compiler-ssr/src/**");
        assert_eq!(ssr_source.rewrite_kind, "copied-official-source-boundary");
        assert_eq!(
            ssr_source.expected_provenance.api_surface,
            "mixed-official-source-boundary"
        );
    }

    #[test]
    fn prepared_manifest_report_derives_official_origin_from_entries() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-prepared-manifest-report-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&temp).unwrap();
        write_prepared_test_manifest_for_suite(suite_spec(ConformanceSuite::Vue3Core), &temp)
            .unwrap();

        let report = prepared_test_manifest_report(
            &prepared_test_manifest_path(&temp).display().to_string(),
        )
        .expect("prepared manifest report");
        assert_eq!(report.official_test_origin, "prepared-official");
        assert_eq!(
            report.entry_count,
            prepared_test_manifest_for_suite(suite_spec(ConformanceSuite::Vue3Core))
                .entries
                .len()
        );
        assert!(report
            .manifest_file
            .replace('\\', "/")
            .ends_with("prepared-test-manifest.json"));

        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn vue2_project_corpus_manifest_parses_toml_schema() {
        let manifest = toml::from_str::<Vue2ProjectCorpusManifest>(
            r#"
schema_version = 1
min_projects = 15
min_vue_files_per_project = 20
projects = []
"#,
        )
        .unwrap();

        assert_eq!(manifest.schema_version, 1);
        assert_eq!(manifest.min_projects, Some(15));
        assert_eq!(manifest.min_vue_files_per_project, Some(20));
        assert!(manifest.projects.is_empty());
    }

    #[test]
    fn vue2_project_dependency_detection_accepts_selected_vue2_specs() {
        assert!(vue_dependency_spec_supports_version(
            "^2.7.16",
            VersionLine::Vue27
        ));
        assert!(vue_dependency_spec_supports_version(
            "~2.7.14",
            VersionLine::Vue27
        ));
        assert!(vue_dependency_spec_supports_version(
            "npm:vue@2.7.16",
            VersionLine::Vue27
        ));
        assert!(vue_dependency_spec_supports_version(
            "workspace:vue@2.7.16",
            VersionLine::Vue27
        ));
        assert!(vue_dependency_spec_supports_version(
            "^2.6.14",
            VersionLine::Vue26
        ));
        assert!(vue_dependency_spec_supports_version(
            "^2.5.17",
            VersionLine::Vue26
        ));
        assert!(!vue_dependency_spec_supports_version(
            "^2.5.17",
            VersionLine::Vue27
        ));
        assert!(!vue_dependency_spec_supports_version(
            "2.5.21",
            VersionLine::Vue26
        ));
        assert!(!vue_dependency_spec_supports_version(
            "2.6.14",
            VersionLine::Vue27
        ));
        assert!(!vue_dependency_spec_supports_version(
            "^3.5.0",
            VersionLine::Vue27
        ));
    }

    #[test]
    fn vue2_project_dependency_detection_accepts_any_compatible_vue_section() {
        let package = serde_json::json!({
            "devDependencies": {
                "vue": "2.5.21"
            },
            "peerDependencies": {
                "vue": "^2.5.17"
            }
        });
        let specs = package_dependency_specs(&package, "vue");

        assert_eq!(specs, vec!["2.5.21", "^2.5.17"]);
        assert!(any_vue_dependency_for_version(&specs, VersionLine::Vue26));
        assert!(!any_vue_dependency_for_version(&specs, VersionLine::Vue27));
        assert_eq!(
            format_dependency_specs(&specs),
            Some("2.5.21; ^2.5.17".into())
        );
    }

    #[test]
    fn vue2_project_dependency_lookup_checks_package_sections_in_order() {
        let package = serde_json::json!({
            "dependencies": {
                "vue": "^2.7.16"
            },
            "devDependencies": {
                "vue": "2.6.14",
                "vue-template-compiler": "~2.7.16"
            },
            "peerDependencies": {
                "vue-template-compiler": "2.6.14"
            }
        });

        assert_eq!(
            package_dependency_spec(&package, "vue"),
            Some("^2.7.16".into())
        );
        assert_eq!(
            package_dependency_spec(&package, "vue-template-compiler"),
            Some("~2.7.16".into())
        );
        assert_eq!(package_dependency_spec(&package, "@vue/compiler-sfc"), None);
    }

    #[test]
    fn vue2_project_scan_applies_include_exclude_and_skips_generated_dirs() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-vue2-project-scan-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(temp.join("src/views")).unwrap();
        fs::create_dir_all(temp.join("docs")).unwrap();
        fs::create_dir_all(temp.join("node_modules/pkg")).unwrap();
        fs::write(temp.join("src/App.vue"), "<template><div/></template>").unwrap();
        fs::write(
            temp.join("src/views/Home.vue"),
            "<template><main/></template>",
        )
        .unwrap();
        fs::write(temp.join("docs/Demo.vue"), "<template><p/></template>").unwrap();
        fs::write(
            temp.join("node_modules/pkg/Bad.vue"),
            "<template><bad/></template>",
        )
        .unwrap();

        let project = Vue2ProjectSpec {
            name: "scan-fixture".into(),
            repo: "https://example.invalid/repo.git".into(),
            rev: "0123456789abcdef0123456789abcdef01234567".into(),
            package_json: None,
            submodules: None,
            include: Some(vec!["src/*.vue".into()]),
            exclude: Some(vec!["src/views/*.vue".into()]),
            min_vue_files: None,
            max_vue_files: None,
        };

        let scan = scan_project_vue_files(&temp, &project);
        let selected = scan
            .files
            .iter()
            .map(|path| relative_slash_path(&temp, path))
            .collect::<Vec<_>>();

        assert_eq!(scan.total, 3);
        assert_eq!(selected, vec!["src/App.vue"]);
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn vue2_project_file_limit_is_deterministic_and_zero_means_unlimited() {
        let files = vec![
            PathBuf::from("src/A.vue"),
            PathBuf::from("src/B.vue"),
            PathBuf::from("src/C.vue"),
        ];

        assert_eq!(limit_vue_files(files.clone(), 0), files);
        assert_eq!(
            limit_vue_files(files.clone(), 2),
            vec![PathBuf::from("src/A.vue"), PathBuf::from("src/B.vue")]
        );
        assert_eq!(limit_vue_files(files.clone(), 4), files);
    }
