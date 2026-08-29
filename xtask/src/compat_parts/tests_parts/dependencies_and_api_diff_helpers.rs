    #[test]
    fn npm_install_scopes_legacy_peer_resolution_to_vue3_runners() {
        assert!(!NPM_INSTALL_ARGS.contains(&"--legacy-peer-deps"));
        assert!(NPM_INSTALL_ARGS.contains(&"--include=optional"));
        assert!(NPM_INSTALL_ARGS.contains(&"--package-lock=false"));
        assert!(!runner_install_uses_legacy_peer_deps(VersionLine::Vue26));
        assert!(!runner_install_uses_legacy_peer_deps(VersionLine::Vue27));
        assert!(runner_install_uses_legacy_peer_deps(VersionLine::Vue3));
    }

    #[test]
    fn node_dependency_available_handles_scoped_packages_and_subpaths() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-node-dep-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let node_modules = temp.join("node_modules");
        fs::create_dir_all(node_modules.join("@vue").join("compiler-core")).unwrap();
        fs::write(
            node_modules
                .join("@vue")
                .join("compiler-core")
                .join("package.json"),
            "{}",
        )
        .unwrap();
        fs::create_dir_all(node_modules.join("vue").join("compiler-sfc")).unwrap();
        fs::write(
            node_modules
                .join("vue")
                .join("compiler-sfc")
                .join("index.js"),
            "",
        )
        .unwrap();

        assert!(node_dependency_available(
            &node_modules,
            "@vue/compiler-core"
        ));
        assert!(alias_package_available(&temp, "vue/compiler-sfc"));
        assert!(!node_dependency_available(&node_modules, "vitest"));
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn official_install_marker_requires_current_platform() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-platform-marker-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&temp).unwrap();
        let marker = temp.join("official-install.json");
        let specs = vec!["vue@3.5.34".to_string()];

        write_json(
            &marker,
            &serde_json::json!({
                "packages": specs,
                "platform": npm_install_platform_marker(),
            }),
        )
        .unwrap();
        assert!(official_install_marker_matches(&marker, &specs));

        write_json(
            &marker,
            &serde_json::json!({
                "packages": specs,
                "platform": "other-os-other-arch",
            }),
        )
        .unwrap();
        assert!(!official_install_marker_matches(&marker, &specs));

        write_json(
            &marker,
            &serde_json::json!({
                "packages": specs,
            }),
        )
        .unwrap();
        assert!(!official_install_marker_matches(&marker, &specs));
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn runtime_smoke_dependency_specs_use_locked_jsdom_versions() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-runtime-smoke-deps-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let root = temp.join("vue3");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("package.json"),
            r#"{
              "devDependencies": {
                "jsdom": "^29.1.1"
              }
            }"#,
        )
        .unwrap();
        fs::write(
            root.join("pnpm-lock.yaml"),
            r#"
packages:
  .
snapshots:
  jsdom@29.1.1: {}
"#,
        )
        .unwrap();

        let specs = runtime_smoke_dependency_specs(VersionLine::Vue3, &temp).unwrap();
        assert_eq!(specs, vec!["jsdom@29.1.1"]);
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn runtime_smoke_required_dependencies_cover_vue3_ssr_runtime() {
        assert_eq!(
            runtime_smoke_required_node_dependencies(VersionLine::Vue26),
            ["vue", "jsdom"]
        );
        assert_eq!(
            runtime_smoke_required_node_dependencies(VersionLine::Vue27),
            ["vue", "jsdom"]
        );
        assert_eq!(
            runtime_smoke_required_node_dependencies(VersionLine::Vue3),
            ["vue", "@vue/compiler-ssr", "@vue/server-renderer", "jsdom"]
        );
    }

    #[test]
    fn conformance_targets_include_suite_package_requests() {
        let targets = conformance_targets(&[ConformanceSuite::Vue3Dom]);
        let requests = targets
            .into_iter()
            .map(api_require_request)
            .collect::<Vec<_>>();
        assert_eq!(requests, vec!["@vue/compiler-core", "@vue/compiler-dom"]);

        let sfc_targets = conformance_targets(&[ConformanceSuite::Vue3Sfc]);
        let sfc_requests = sfc_targets
            .into_iter()
            .map(api_require_request)
            .collect::<Vec<_>>();
        assert_eq!(
            sfc_requests,
            vec![
                "@vue/compiler-core",
                "@vue/compiler-dom",
                "@vue/compiler-ssr",
                "@vue/compiler-sfc",
            ]
        );
    }

    #[test]
    fn runner_dependency_specs_use_locked_versions() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-runner-deps-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let root = temp.join("vue3");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("package.json"),
            r#"{
              "devDependencies": {
                "@babel/parser": "^7.29.3",
                "@babel/types": "^7.29.0",
                "@vue/consolidate": "1.0.0",
                "estree-walker": "^2.0.2",
                "vitest": "^4.1.5",
                "esbuild": "^0.28.0",
                "hash-sum": "^2.0.0",
                "jsdom": "^29.1.1",
                "lru-cache": "11.5.0",
                "magic-string": "^0.30.21",
                "merge-source-map": "^1.1.0",
                "minimatch": "~10.2.5",
                "postcss-modules": "^6.0.1",
                "postcss-selector-parser": "^7.1.1",
                "pug": "^3.0.4",
                "sass": "^1.99.0",
                "typescript": "~5.6.2",
                "source-map-js": "catalog:"
              }
            }"#,
        )
        .unwrap();
        fs::write(
            root.join("pnpm-lock.yaml"),
            r#"
packages:
  .
snapshots:
  '@babel/parser@7.29.3': {}
  '@babel/types@7.29.0': {}
  '@vue/consolidate@1.0.0': {}
  esbuild@0.28.0: {}
  estree-walker@2.0.2: {}
  hash-sum@2.0.0: {}
  jsdom@29.1.1: {}
  lru-cache@11.5.0: {}
  magic-string@0.30.21: {}
  merge-source-map@1.1.0: {}
  minimatch@10.2.5: {}
  postcss-modules@6.0.1(postcss@8.5.14): {}
  postcss-selector-parser@7.1.1: {}
  pug@3.0.4: {}
  sass@1.99.0: {}
  source-map-js@1.2.1: {}
  typescript@5.6.3: {}
  vitest@4.1.5(@types/node@24.12.2): {}
"#,
        )
        .unwrap();

        let specs = runner_dependency_specs(suite_spec(ConformanceSuite::Vue3Core), &temp)
            .unwrap()
            .unwrap();
        assert_eq!(
            specs,
            vec!["esbuild@0.28.0", "source-map-js@1.2.1", "vitest@4.1.5"]
        );
        let dom_specs = runner_dependency_specs(suite_spec(ConformanceSuite::Vue3Dom), &temp)
            .unwrap()
            .unwrap();
        assert_eq!(
            dom_specs,
            vec![
                "esbuild@0.28.0",
                "jsdom@29.1.1",
                "source-map-js@1.2.1",
                "vitest@4.1.5"
            ]
        );
        let sfc_specs = runner_dependency_specs(suite_spec(ConformanceSuite::Vue3Sfc), &temp)
            .unwrap()
            .unwrap();
        assert_eq!(
            sfc_specs,
            vec![
                "@babel/parser@7.29.3",
                "@babel/types@7.29.0",
                "@vue/consolidate@1.0.0",
                "esbuild@0.28.0",
                "estree-walker@2.0.2",
                "hash-sum@2.0.0",
                "lru-cache@11.5.0",
                "magic-string@0.30.21",
                "merge-source-map@1.1.0",
                "minimatch@10.2.5",
                "postcss-modules@6.0.1",
                "postcss-selector-parser@7.1.1",
                "pug@3.0.4",
                "sass@1.99.0",
                "source-map-js@1.2.1",
                "typescript@5.6.3",
                "vitest@4.1.5",
            ]
        );
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn runner_dependency_specs_fall_back_to_manifest_specs() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-runner-deps-manifest-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let root = temp.join("vue2_6");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("package.json"),
            r#"{
              "devDependencies": {
                "@babel/register": "^7.0.0",
                "jasmine": "^2.99.0"
              }
            }"#,
        )
        .unwrap();
        let fallback_root = temp.join("vue2_7");
        fs::create_dir_all(&fallback_root).unwrap();
        fs::write(
            fallback_root.join("package.json"),
            r#"{
              "devDependencies": {
                "jsdom": "^19.0.0"
              }
            }"#,
        )
        .unwrap();
        fs::write(
            fallback_root.join("pnpm-lock.yaml"),
            r#"
packages:
  .
snapshots:
  /jsdom@19.0.0: {}
"#,
        )
        .unwrap();

        let specs = runner_dependency_specs(suite_spec(ConformanceSuite::Vue2Compiler), &temp)
            .unwrap()
            .unwrap();
        assert_eq!(
            specs,
            vec!["@babel/register@^7.0.0", "jasmine@^2.99.0", "jsdom@19.0.0"]
        );
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn yarn_lock_dependency_lookup_matches_exact_package_name() {
        let lock = r#"
"@babel/register@^7.0.0":
  version "7.0.0"

eslint-plugin-jasmine@^2.8.4:
  version "2.10.1"

jasmine@^2.99.0:
  version "2.99.0"
"#;
        assert_eq!(
            locked_yarn_dependency_version(lock, "@babel/register"),
            Some("7.0.0".into())
        );
        assert_eq!(
            locked_yarn_dependency_version(lock, "jasmine"),
            Some("2.99.0".into())
        );
    }

    fn test_manifest(exports: Vec<(&str, u32)>) -> ManifestFile {
        let mut export_names = Vec::new();
        let mut export_details = BTreeMap::new();
        for (name, arity) in exports {
            export_names.push(name.to_string());
            export_details.insert(
                name.to_string(),
                ApiExportDetail {
                    kind: "function".into(),
                    tag: "[object Function]".into(),
                    name: Some(name.to_string()),
                    function_arity: Some(arity),
                    is_async_function: Some(false),
                    is_class_like: Some(false),
                    own_property_names: vec!["length".into(), "name".into(), "prototype".into()],
                },
            );
        }
        ManifestFile {
            schema_version: 1,
            version_line: VersionLine::Vue26,
            package: "vue-template-compiler".into(),
            entry: "index".into(),
            package_version: Some("2.6.14".into()),
            exports: export_names,
            export_details,
            require: ApiRequireRecord {
                request: "vue-template-compiler".into(),
                success: true,
                resolved: Some("<probe-root>/node_modules/vue-template-compiler/index.js".into()),
                error_name: None,
                error_code: None,
                error_message: None,
            },
            types: ApiTypesRecord {
                package_types: Some("types/index.d.ts".into()),
                resolved: Some(
                    "<probe-root>/node_modules/vue-template-compiler/types/index.d.ts".into(),
                ),
                exists: true,
            },
            status: "pass".into(),
            source: "official".into(),
            lock_hash: Some("lock".into()),
            official_revision: Some("612fb89547711cacb030a3893a0065b785802860".into()),
        }
    }
