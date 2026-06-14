    #[test]
    fn api_manifest_side_selection_defaults_to_both_sides() {
        let scope = SelectionArgs::default();
        assert_eq!(
            selected_api_manifest_sides(&scope),
            vec![ApiManifestSide::Official, ApiManifestSide::Rust]
        );

        let official_only = SelectionArgs {
            official: true,
            ..SelectionArgs::default()
        };
        assert_eq!(
            selected_api_manifest_sides(&official_only),
            vec![ApiManifestSide::Official]
        );

        let rust_only = SelectionArgs {
            rust: true,
            ..SelectionArgs::default()
        };
        assert_eq!(
            selected_api_manifest_sides(&rust_only),
            vec![ApiManifestSide::Rust]
        );
    }

    #[test]
    fn generation_paths_respect_custom_out_dir() {
        let target = TargetSpec {
            version_line: VersionLine::Vue3,
            package: "@vue/compiler-sfc",
            entry: "@vue/compiler-sfc",
            kind: TargetKind::Vue3Sfc,
        };
        let out_dir = PathBuf::from("target/custom-compat");

        assert_eq!(
            target.api_manifest_path_in(&out_dir, "official"),
            PathBuf::from(
                "target/custom-compat/api/official/vue3/_vue_compiler-sfc/_vue_compiler-sfc.json"
            )
        );
        assert_eq!(
            target.option_matrix_path_in(&out_dir),
            PathBuf::from(
                "target/custom-compat/options/vue3/_vue_compiler-sfc/_vue_compiler-sfc.json"
            )
        );
        assert_eq!(
            target.output_contract_path_in(&out_dir),
            PathBuf::from(
                "target/custom-compat/output/vue3/_vue_compiler-sfc/_vue_compiler-sfc.json"
            )
        );
        assert_eq!(
            target.relative_option_matrix_path(),
            PathBuf::from("compat/options/vue3/_vue_compiler-sfc/_vue_compiler-sfc.json")
        );
    }

    #[test]
    fn target_selection_does_not_expand_filtered_misses_to_all_targets() {
        let filtered_miss = SelectionArgs {
            version_line: Some(VersionLine::Vue3),
            package: Some("@vue/compiler-sfc".into()),
            entry: Some("@vue/compiler-sfc".into()),
            ..SelectionArgs::default()
        };
        assert!(select_targets(&filtered_miss).is_empty());

        let default_scope = SelectionArgs::default();
        assert_eq!(select_targets(&default_scope).len(), all_targets().len());

        let all_scope = SelectionArgs {
            all: true,
            version_line: Some(VersionLine::Vue3),
            package: Some("@vue/compiler-sfc".into()),
            entry: Some("@vue/compiler-sfc".into()),
            ..SelectionArgs::default()
        };
        assert_eq!(select_targets(&all_scope).len(), all_targets().len());
    }

    #[test]
    fn existing_git_checkout_must_match_origin_and_be_clean() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-existing-checkout-guard-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let checkout = temp.join("checkout");
        fs::create_dir_all(&checkout).unwrap();
        run_command("git", &["init"], Some(&checkout)).unwrap();
        run_git(
            &checkout,
            &["remote", "add", "origin", "https://example.com/vue.git"],
        )
        .unwrap();

        let mismatch =
            ensure_existing_git_checkout_matches("https://github.com/vuejs/vue", &checkout)
                .unwrap_err()
                .to_string();
        assert!(mismatch.contains("refusing to reuse"));

        run_git(
            &checkout,
            &[
                "remote",
                "set-url",
                "origin",
                "https://github.com/vuejs/vue.git",
            ],
        )
        .unwrap();
        fs::write(checkout.join("official-revision.json"), "{}").unwrap();
        ensure_existing_git_checkout_matches("https://github.com/vuejs/vue", &checkout).unwrap();

        fs::write(checkout.join("local.txt"), "dirty").unwrap();

        let dirty = ensure_existing_git_checkout_matches("https://github.com/vuejs/vue", &checkout)
            .unwrap_err()
            .to_string();
        assert!(dirty.contains("local changes"));

        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn official_lock_rejects_floating_npm_versions() {
        let mut lock = OfficialRevisionsLock {
            vue2_6: BaselineLock {
                repo: "https://github.com/vuejs/vue".into(),
                rev: "612fb89547711cacb030a3893a0065b785802860".into(),
                npm: BTreeMap::from([
                    ("vue".into(), "2.6.14".into()),
                    ("vue-template-compiler".into(), "^2.6.14".into()),
                ]),
                exports: BTreeMap::new(),
            },
            vue2_7: BaselineLock {
                repo: "https://github.com/vuejs/vue".into(),
                rev: "13f4e7dc03e2caed900ac70ff8b8fe58dda45663".into(),
                npm: BTreeMap::from([
                    ("vue".into(), "2.7.16".into()),
                    ("vue-template-compiler".into(), "2.7.16".into()),
                ]),
                exports: BTreeMap::from([(
                    "vue/compiler-sfc".into(),
                    "./compiler-sfc/index.js".into(),
                )]),
            },
            vue3: BaselineLock {
                repo: "https://github.com/vuejs/core".into(),
                rev: "57545e958ae28ed17aa9e0ed321abcd8dc99f752".into(),
                npm: BTreeMap::from([
                    ("vue".into(), "3.5.34".into()),
                    ("@vue/compiler-core".into(), "3.5.34".into()),
                    ("@vue/compiler-dom".into(), "3.5.34".into()),
                    ("@vue/compiler-sfc".into(), "3.5.34".into()),
                    ("@vue/compiler-ssr".into(), "3.5.34".into()),
                ]),
                exports: BTreeMap::new(),
            },
        };

        let violations = validate_official_lock(&lock);
        assert!(violations
            .iter()
            .any(|violation| violation.contains("must be an exact npm package version")));

        lock.vue2_6
            .npm
            .insert("vue-template-compiler".into(), "latest".into());
        let violations = validate_official_lock(&lock);
        assert!(violations
            .iter()
            .any(|violation| violation.contains("must be an exact npm package version")));
    }

    #[test]
    fn official_lock_vendor_validation_rejects_tag_object_revs() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-official-lock-vendor-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let checkout = temp.join("vue2_6");
        fs::create_dir_all(checkout.join("packages/vue-template-compiler")).unwrap();
        run_command("git", &["init"], Some(&checkout)).unwrap();
        fs::write(checkout.join("package.json"), r#"{"version":"2.6.14"}"#).unwrap();
        fs::write(
            checkout
                .join("packages/vue-template-compiler")
                .join("package.json"),
            r#"{"version":"2.6.14"}"#,
        )
        .unwrap();
        run_git(&checkout, &["add", "."]).unwrap();
        run_git(
            &checkout,
            &[
                "-c",
                "user.email=a@b.c",
                "-c",
                "user.name=Vuec",
                "commit",
                "-m",
                "init",
            ],
        )
        .unwrap();
        let commit = git_output(&checkout, &["rev-parse", "HEAD"]).unwrap();
        run_git(
            &checkout,
            &[
                "-c",
                "user.email=a@b.c",
                "-c",
                "user.name=Vuec",
                "tag",
                "-a",
                "v2.6.14",
                "-m",
                "v2.6.14",
            ],
        )
        .unwrap();
        let tag_object = git_output(&checkout, &["rev-parse", "v2.6.14"]).unwrap();
        assert_ne!(tag_object, commit);

        let lock = OfficialRevisionsLock {
            vue2_6: BaselineLock {
                repo: "https://github.com/vuejs/vue".into(),
                rev: tag_object,
                npm: BTreeMap::from([
                    ("vue".into(), "2.6.14".into()),
                    ("vue-template-compiler".into(), "2.6.14".into()),
                ]),
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

        let items = validate_official_lock_vendor(&lock, &temp);
        assert!(items.iter().any(|item| {
            item.target == "vue2_6.rev-object"
                && item.status == ReportStatus::Fail
                && item.detail.contains("expected commit")
        }));
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn api_diff_detects_export_and_arity_mismatch() {
        let mut official = test_manifest(vec![("compile", 2)]);
        let mut rust = test_manifest(vec![("compile", 1)]);
        let diffs = compare_api_manifests(&official, &rust);
        assert!(
            diffs
                .iter()
                .any(|diff| diff.contains("export compile detail differs")),
            "{diffs:#?}"
        );

        rust.exports.push("extra".into());
        rust.export_details.insert(
            "extra".into(),
            ApiExportDetail {
                kind: "function".into(),
                tag: "[object Function]".into(),
                name: Some("extra".into()),
                function_arity: Some(0),
                is_async_function: Some(false),
                is_class_like: Some(false),
                own_property_names: vec!["length".into(), "name".into(), "prototype".into()],
            },
        );
        let diffs = compare_api_manifests(&official, &rust);
        assert!(
            diffs.iter().any(|diff| diff.contains("exports differ")),
            "{diffs:#?}"
        );

        official.exports = rust.exports.clone();
        official.export_details = rust.export_details.clone();
        assert!(compare_api_manifests(&official, &rust).is_empty());
    }

    #[test]
    fn vue3_dom_core_runtime_exports_forward_to_alias_runtime() {
        let function_detail = ApiExportDetail {
            kind: "function".into(),
            tag: "[object Function]".into(),
            name: Some("unwrapTSNode".into()),
            function_arity: Some(1),
            is_async_function: Some(false),
            is_class_like: Some(false),
            own_property_names: vec!["length".into(), "name".into(), "prototype".into()],
        };
        let object_detail = ApiExportDetail {
            kind: "object".into(),
            tag: "[object Object]".into(),
            name: None,
            function_arity: None,
            is_async_function: None,
            is_class_like: None,
            own_property_names: vec!["DATA".into(), "SETUP_CONST".into()],
        };
        let target = TargetSpec {
            version_line: VersionLine::Vue3,
            package: "@vue/compiler-dom",
            entry: "index",
            kind: TargetKind::Vue3Dom,
        };

        assert!(
            alias_export_expression(target, "unwrapTSNode", Some(&function_detail))
                .contains("vue3CoreRuntime[\"unwrapTSNode\"].apply")
        );
        assert_eq!(
            alias_export_expression(target, "BindingTypes", Some(&object_detail)),
            "vue3CoreRuntime[\"BindingTypes\"]"
        );
        assert_eq!(
            alias_export_expression(target, "parserOptions", Some(&object_detail)),
            "vue3DomParserOptions"
        );
        assert!(
            alias_export_expression(target, "createSimpleExpression", Some(&function_detail))
                .contains("vue3CoreRuntime[\"createSimpleExpression\"].apply")
        );
    }

    #[test]
    fn allowed_api_diff_requires_exact_target_diff_and_reason() {
        let target = TargetSpec {
            version_line: VersionLine::Vue26,
            package: "vue-template-compiler",
            entry: "index",
            kind: TargetKind::Vue26Template,
        };
        let diff = "exports differ: official=[] rust=[]";
        let allowed = AllowedApiDiffFile {
            entries: vec![AllowedApiDiffEntry {
                version_line: VersionLine::Vue26,
                package: "vue-template-compiler".into(),
                entry: "index".into(),
                diff: diff.into(),
                reason: "documented compatibility exception".into(),
            }],
        };
        assert!(is_allowed_api_diff(&allowed, target, diff));
        assert!(!is_allowed_api_diff(&allowed, target, "different diff"));
    }

    #[test]
    fn vue27_sfc_output_contract_exports_version_context() {
        let target = TargetSpec {
            version_line: VersionLine::Vue27,
            package: "vue",
            entry: "vue/compiler-sfc",
            kind: TargetKind::Vue27Sfc,
        };

        assert_eq!(output_contract_kind(target), "sfc");
        assert_eq!(api_require_request(target), "vue/compiler-sfc");
        assert!(OUTPUT_CONTRACT_PROBE_SCRIPT
            .contains("versionLine === 'vue2_7' && entry === 'vue/compiler-sfc'"));
        assert!(OUTPUT_CONTRACT_PROBE_SCRIPT.contains("api.parse({ source: fixture"));
    }

    #[test]
    fn vue27_sfc_compile_script_alias_hydrates_binding_metadata_shape() {
        let target = TargetSpec {
            version_line: VersionLine::Vue27,
            package: "vue",
            entry: "vue/compiler-sfc",
            kind: TargetKind::Vue27Sfc,
        };
        let detail = ApiExportDetail {
            kind: "function".into(),
            tag: "[object Function]".into(),
            name: Some("compileScript".into()),
            function_arity: Some(2),
            is_async_function: Some(false),
            is_class_like: Some(false),
            own_property_names: vec!["length".into(), "name".into(), "prototype".into()],
        };
        let expression = alias_export_expression(target, "compileScript", Some(&detail));

        assert!(expression.contains("hydrateVue27CompileScriptResult"));
        assert!(expression.contains("vue27CompileScriptBridgePayload"));
        assert!(ALIAS_RUNTIME_JS.contains("function hydrateVue27CompileScriptResult"));
        assert!(ALIAS_RUNTIME_JS.contains("function vue27CompileScriptBridgePayload"));
        assert!(ALIAS_RUNTIME_JS.contains("__vuecEmitScriptSetupMarker = false"));
        assert!(ALIAS_RUNTIME_JS.contains("Object.defineProperty(bindings, '__isScriptSetup'"));
    }

    #[test]
    fn vue27_sfc_compile_template_alias_applies_official_prettify_boundary() {
        let target = TargetSpec {
            version_line: VersionLine::Vue27,
            package: "vue",
            entry: "vue/compiler-sfc",
            kind: TargetKind::Vue27Sfc,
        };
        let detail = ApiExportDetail {
            kind: "function".into(),
            tag: "[object Function]".into(),
            name: Some("compileTemplate".into()),
            function_arity: Some(1),
            is_async_function: Some(false),
            is_class_like: Some(false),
            own_property_names: vec!["length".into(), "name".into(), "prototype".into()],
        };
        let expression = alias_export_expression(target, "compileTemplate", Some(&detail));

        assert!(expression.contains("sfc.vue27.compileTemplate"));
        assert!(expression.contains("prettifyVue27SfcTemplateResult"));
        assert!(expression.contains("__vuecPayload.filename"));
        assert!(ALIAS_RUNTIME_JS.contains("function prettifyVue27SfcTemplateResult"));
        assert!(ALIAS_RUNTIME_JS.contains("function vue27SfcTemplateIsProduction"));
        assert!(ALIAS_RUNTIME_JS.contains("function vue27SfcTemplatePrettifyEnabled"));
        assert!(ALIAS_RUNTIME_JS.contains("function vue27SfcCompileTemplateBridgePayload"));
        assert!(ALIAS_RUNTIME_JS.contains("maxBuffer: 64 * 1024 * 1024"));
        assert!(ALIAS_RUNTIME_JS.contains("require('prettier').format(out.code || ''"));
        assert!(ALIAS_RUNTIME_JS.contains("parser: 'babel'"));
        assert!(ALIAS_RUNTIME_JS.contains("return !!options.prettify"));
        assert!(ALIAS_RUNTIME_JS.contains("process.env.NODE_ENV === 'production'"));
        assert!(ALIAS_RUNTIME_JS.contains("The `prettify` option is on"));
        assert!(ALIAS_RUNTIME_JS.contains("Failed to prettify component"));
    }

    #[test]
    fn napi_vue27_sfc_native_alias_applies_official_prettify_boundary() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let source = fs::read_to_string(
            repo_root
                .join("packages")
                .join("native-aliases")
                .join("vue")
                .join("compiler-sfc")
                .join("index.js"),
        )
        .unwrap();

        assert!(source.contains("prettifyVue27SfcTemplateResult("));
        assert!(source.contains("native.compileVue27SfcTemplate(source, opts)"));
        assert!(source.contains("function prettifyVue27SfcTemplateResult"));
        assert!(source.contains("function vue27SfcTemplateIsProduction"));
        assert!(source.contains("function vue27SfcTemplatePrettifyEnabled"));
        assert!(source.contains("function vue27SfcCompileTemplateOptions"));
        assert!(source.contains("require('prettier').format(out.code || ''"));
        assert!(source.contains("parser: 'babel'"));
        assert!(source.contains("return !!options.prettify"));
        assert!(source.contains("process.env.NODE_ENV === 'production'"));
        assert!(source.contains("The `prettify` option is on"));
        assert!(source.contains("Failed to prettify component"));
    }

    #[test]
    fn vue27_sfc_compile_style_alias_keeps_postcss_callbacks_in_js_adapter() {
        let target = TargetSpec {
            version_line: VersionLine::Vue27,
            package: "vue",
            entry: "vue/compiler-sfc",
            kind: TargetKind::Vue27Sfc,
        };
        let detail = ApiExportDetail {
            kind: "function".into(),
            tag: "[object Function]".into(),
            name: Some("compileStyle".into()),
            function_arity: Some(1),
            is_async_function: Some(false),
            is_class_like: Some(false),
            own_property_names: vec!["length".into(), "name".into(), "prototype".into()],
        };
        let expression = alias_export_expression(target, "compileStyle", Some(&detail));

        assert!(expression.contains("vue27StyleBridgePayload"));
        assert!(expression.contains("applyVue27StylePostcssSync"));
        assert!(expression.contains("sfc.vue27.compileStyle"));
        assert!(ALIAS_RUNTIME_JS.contains("function applyVue27StylePostcssSync"));
        assert!(ALIAS_RUNTIME_JS.contains("key !== 'postcssPlugins'"));
        assert!(ALIAS_RUNTIME_JS.contains("key !== 'postcssOptions'"));
    }

    #[test]
    fn vue3_sfc_compile_style_alias_emits_rust_style_warnings() {
        let target = TargetSpec {
            version_line: VersionLine::Vue3,
            package: "@vue/compiler-sfc",
            entry: "@vue/compiler-sfc",
            kind: TargetKind::Vue3Sfc,
        };
        let detail = ApiExportDetail {
            kind: "function".into(),
            tag: "[object Function]".into(),
            name: Some("compileStyle".into()),
            function_arity: Some(1),
            is_async_function: Some(false),
            is_class_like: Some(false),
            own_property_names: vec!["length".into(), "name".into(), "prototype".into()],
        };
        let expression = alias_export_expression(target, "compileStyle", Some(&detail));

        assert!(expression.contains("emitVue3StyleWarnings"));
        assert!(expression.contains("normalizeStyleAliasResult"));
        assert!(expression.contains("vue3StyleBridgePayload"));
        assert!(ALIAS_RUNTIME_JS.contains("function emitVue3StyleWarnings"));
        assert!(ALIAS_RUNTIME_JS.contains("function normalizeStyleAliasResult"));
        assert!(ALIAS_RUNTIME_JS.contains("function vue3StyleBridgePayload"));
        assert!(ALIAS_RUNTIME_JS.contains("VUEC_STYLE_DEPRECATED_SCOPED_SELECTOR"));
    }

    #[test]
    fn vue3_sfc_parse_alias_hydrates_hmr_reload_api() {
        let target = TargetSpec {
            version_line: VersionLine::Vue3,
            package: "@vue/compiler-sfc",
            entry: "@vue/compiler-sfc",
            kind: TargetKind::Vue3Sfc,
        };
        let detail = ApiExportDetail {
            kind: "function".into(),
            tag: "[object Function]".into(),
            name: Some("parse".into()),
            function_arity: Some(2),
            is_async_function: Some(false),
            is_class_like: Some(false),
            own_property_names: vec!["length".into(), "name".into(), "prototype".into()],
        };
        let expression = alias_export_expression(target, "parse", Some(&detail));

        assert!(expression.contains("hydrateVue3SfcParseResult"));
        assert!(expression.contains("sfc.parse"));
        assert!(expression.contains("vue3SfcParseBridgePayload"));
        assert!(expression.contains("applyVue3SfcCustomCompilerParse"));
        assert!(ALIAS_RUNTIME_JS.contains("function hydrateVue3SfcParseResult"));
        assert!(ALIAS_RUNTIME_JS.contains("function vue3SfcParseBridgePayload"));
        assert!(ALIAS_RUNTIME_JS.contains("function normalizeVue3SfcParseOptionsForBridge"));
        assert!(ALIAS_RUNTIME_JS.contains("function applyVue3SfcCustomCompilerParse"));
        assert!(ALIAS_RUNTIME_JS.contains("options.templateParseOptions"));
        assert!(ALIAS_RUNTIME_JS.contains("key !== 'compiler'"));
        assert!(ALIAS_RUNTIME_JS.contains("function vue3SfcShouldForceReload"));
        assert!(ALIAS_RUNTIME_JS.contains("function vue3SfcTemplateUsedIdentifiers"));
    }

    #[test]
    fn vue3_sfc_compile_script_alias_hydrates_binding_metadata_shape() {
        let target = TargetSpec {
            version_line: VersionLine::Vue3,
            package: "@vue/compiler-sfc",
            entry: "@vue/compiler-sfc",
            kind: TargetKind::Vue3Sfc,
        };
        let detail = ApiExportDetail {
            kind: "function".into(),
            tag: "[object Function]".into(),
            name: Some("compileScript".into()),
            function_arity: Some(2),
            is_async_function: Some(false),
            is_class_like: Some(false),
            own_property_names: vec!["length".into(), "name".into(), "prototype".into()],
        };
        let expression = alias_export_expression(target, "compileScript", Some(&detail));

        assert!(expression.contains("sfc.compileScript"));
        assert!(expression.contains("hydrateVue3CompileScriptResult"));
        assert!(expression.contains("vue3CompileScriptBridgePayload"));
        assert!(ALIAS_RUNTIME_JS.contains("function hydrateVue3CompileScriptResult"));
        assert!(ALIAS_RUNTIME_JS.contains("function vue3CompileScriptBridgePayload"));
        assert!(ALIAS_RUNTIME_JS.contains("function throwVue3CompileScriptErrors"));
        assert!(ALIAS_RUNTIME_JS.contains("__vuecEmitScriptSetupMarker = false"));
        assert!(ALIAS_RUNTIME_JS
            .contains("options.__vuecCustomElement = !!options.customElement(filename)"));
        assert!(ALIAS_RUNTIME_JS.contains("delete options.customElement"));
        assert!(ALIAS_RUNTIME_JS.contains("bindings.__propsAliases = result.propsAliases"));
        assert!(ALIAS_RUNTIME_JS.contains("delete result.propsAliases"));
        assert!(ALIAS_RUNTIME_JS.contains("Array.isArray(result.warnings)"));
        assert!(ALIAS_RUNTIME_JS.contains("Object.defineProperty(bindings, '__isScriptSetup'"));
        assert!(ALIAS_RUNTIME_JS.contains("[@vue/compiler-sfc] ${message}"));
    }

    #[test]
    fn vue3_sfc_compile_template_alias_projects_public_api_boundary() {
        let target = TargetSpec {
            version_line: VersionLine::Vue3,
            package: "@vue/compiler-sfc",
            entry: "@vue/compiler-sfc",
            kind: TargetKind::Vue3Sfc,
        };
        let detail = ApiExportDetail {
            kind: "function".into(),
            tag: "[object Function]".into(),
            name: Some("compileTemplate".into()),
            function_arity: Some(1),
            is_async_function: Some(false),
            is_class_like: Some(false),
            own_property_names: vec!["length".into(), "name".into(), "prototype".into()],
        };
        let expression = alias_export_expression(target, "compileTemplate", Some(&detail));

        assert!(expression.contains("sfc.compileTemplate"));
        assert!(expression.contains("vue3SfcCompileTemplateBridgePayload"));
        assert!(expression.contains("vue3SfcCustomCompileTemplateResult"));
        assert!(expression.contains("hydrateVue3SfcCompileTemplateResult"));
        assert!(ALIAS_RUNTIME_JS.contains("function vue3SfcCompileTemplateBridgePayload"));
        assert!(ALIAS_RUNTIME_JS.contains("function vue3SfcCustomCompileTemplateResult"));
        assert!(ALIAS_RUNTIME_JS.contains("function hydrateVue3SfcCompileTemplateResult"));
        assert!(ALIAS_RUNTIME_JS.contains("vue3CoreRuntime.dehydrateForBridge(options.ast)"));
        assert!(ALIAS_RUNTIME_JS.contains("compiler.compile(source, compilerOptions)"));
        assert!(ALIAS_RUNTIME_JS.contains("new SyntaxError(message)"));
        assert!(ALIAS_RUNTIME_JS.contains("bridgeOptions.ssrCssVars = options.ssrCssVars"));
    }

    #[test]
    fn vue3_sfc_rewrite_default_alias_routes_to_rust_bridge() {
        let target = TargetSpec {
            version_line: VersionLine::Vue3,
            package: "@vue/compiler-sfc",
            entry: "@vue/compiler-sfc",
            kind: TargetKind::Vue3Sfc,
        };
        let detail = ApiExportDetail {
            kind: "function".into(),
            tag: "[object Function]".into(),
            name: Some("rewriteDefault".into()),
            function_arity: Some(3),
            is_async_function: Some(false),
            is_class_like: Some(false),
            own_property_names: vec!["length".into(), "name".into(), "prototype".into()],
        };
        let expression = alias_export_expression(target, "rewriteDefault", Some(&detail));

        assert!(expression.contains("sfc.rewriteDefault"));
        assert!(expression.contains("plugins: a2"));
        assert!(!expression.contains("sfc.vue27.rewriteDefault"));
    }

    #[test]
    fn napi_vue3_sfc_native_alias_hydrates_hmr_reload_api() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let source = fs::read_to_string(
            repo_root
                .join("packages")
                .join("native-aliases")
                .join("@vue")
                .join("compiler-sfc")
                .join("dist")
                .join("compiler-sfc.cjs.js"),
        )
        .unwrap();

        assert!(source.contains("const bridgePayload = vue3SfcParseBridgePayload(payload);"));
        assert!(source.contains("callBridge('sfc.parse', bridgePayloadForCall(bridgePayload))"));
        assert!(source.contains("maxBuffer: 64 * 1024 * 1024"));
        assert!(source
            .contains("native.parseSfcResult(payload.source, bridgePayload.bridgeOptions || {})"));
        assert!(source.contains("function hydrateVue3SfcParseResult"));
        assert!(source.contains("function vue3SfcShouldForceReload"));
        assert!(source.contains("descriptor.shouldForceReload = function shouldForceReload"));
    }

    #[test]
    fn napi_vue3_sfc_native_alias_hydrates_compile_script_bindings() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let source = fs::read_to_string(
            repo_root
                .join("packages")
                .join("native-aliases")
                .join("@vue")
                .join("compiler-sfc")
                .join("dist")
                .join("compiler-sfc.cjs.js"),
        )
        .unwrap();

        assert!(source.contains("const bridgePayload = vue3CompileScriptBridgePayload(payload);"));
        assert!(
            source.contains("callBridge('sfc.compileScript', bridgePayloadForCall(bridgePayload))")
        );
        assert!(
            source.contains("native.compileScript(descriptor || {}, bridgePayload.options || {})")
        );
        assert!(source.contains("function hydrateVue3CompileScriptResult"));
        assert!(source.contains("function vue3CompileScriptBridgePayload"));
        assert!(source.contains("function throwVue3CompileScriptErrors"));
        assert!(source.contains("bindings.__propsAliases = result.propsAliases"));
        assert!(source.contains("delete result.propsAliases"));
        assert!(source.contains("Array.isArray(result.warnings)"));
        assert!(source.contains("Object.defineProperty(bindings, '__isScriptSetup'"));
        assert!(source.contains("[@vue/compiler-sfc] ${message}"));
    }

    #[test]
    fn napi_vue3_compiler_core_native_alias_bridge_uses_large_stdout_buffer() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let source = fs::read_to_string(
            repo_root
                .join("packages")
                .join("native-aliases")
                .join("@vue")
                .join("compiler-core")
                .join("index.js"),
        )
        .unwrap();

        assert!(source.contains("cp.spawnSync(bridgeBin"));
        assert!(source.contains("maxBuffer: 64 * 1024 * 1024"));
    }

    #[test]
    fn napi_vue3_sfc_native_alias_routes_rewrite_default_to_vue3_native_api() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let source = fs::read_to_string(
            repo_root
                .join("packages")
                .join("native-aliases")
                .join("@vue")
                .join("compiler-sfc")
                .join("dist")
                .join("compiler-sfc.cjs.js"),
        )
        .unwrap();

        assert!(source.contains("function rewriteDefault(source, as, parserPlugins)"));
        assert!(source.contains(
            "return native.rewriteDefaultVue3(String(source || ''), String(as || ''), parserPlugins || []);"
        ));
        assert!(!source.contains(
            "return native.rewriteDefaultVue27(String(source || ''), String(as || ''), parserPlugins || []);"
        ));
    }

    #[test]
    fn vue3_ssr_compile_alias_hydrates_public_ast_helpers() {
        let target = TargetSpec {
            version_line: VersionLine::Vue3,
            package: "@vue/compiler-ssr",
            entry: "@vue/compiler-ssr",
            kind: TargetKind::Vue3Ssr,
        };
        let detail = ApiExportDetail {
            kind: "function".into(),
            tag: "[object Function]".into(),
            name: Some("compile".into()),
            function_arity: Some(2),
            is_async_function: Some(false),
            is_class_like: Some(false),
            own_property_names: vec!["length".into(), "name".into(), "prototype".into()],
        };
        let expression = alias_export_expression(target, "compile", Some(&detail));

        assert!(expression.contains("hydrateVue3SsrCompileResult"));
        assert!(expression.contains("hydrateVue3SsrCompileResult("));
        assert!(expression.contains("__vuecPayload.options"));
        assert!(expression.contains("__vuecPayload.source"));
        assert!(expression.contains("vue3.ssr.compile"));
        assert!(ALIAS_RUNTIME_JS.contains("function hydrateVue3SsrCompileResult"));
        assert!(ALIAS_RUNTIME_JS.contains("emitVue3CompileDiagnostics(result, options, source)"));
        assert!(ALIAS_RUNTIME_JS.contains("new Set(result.ast_helpers.map(name => Symbol(name)))"));
    }

    #[test]
    fn vue3_compile_aliases_consume_public_diagnostics() {
        let function_detail = ApiExportDetail {
            kind: "function".into(),
            tag: "[object Function]".into(),
            name: None,
            function_arity: Some(2),
            is_async_function: Some(false),
            is_class_like: Some(false),
            own_property_names: vec!["length".into(), "name".into(), "prototype".into()],
        };
        let core_base_compile = alias_export_expression(
            TargetSpec {
                version_line: VersionLine::Vue3,
                package: "@vue/compiler-core",
                entry: "@vue/compiler-core",
                kind: TargetKind::Vue3Core,
            },
            "baseCompile",
            Some(&function_detail),
        );
        let core_generate = alias_export_expression(
            TargetSpec {
                version_line: VersionLine::Vue3,
                package: "@vue/compiler-core",
                entry: "@vue/compiler-core",
                kind: TargetKind::Vue3Core,
            },
            "generate",
            Some(&function_detail),
        );
        let dom_compile = alias_export_expression(
            TargetSpec {
                version_line: VersionLine::Vue3,
                package: "@vue/compiler-dom",
                entry: "@vue/compiler-dom",
                kind: TargetKind::Vue3Dom,
            },
            "compile",
            Some(&function_detail),
        );
        let ssr_compile = alias_export_expression(
            TargetSpec {
                version_line: VersionLine::Vue3,
                package: "@vue/compiler-ssr",
                entry: "@vue/compiler-ssr",
                kind: TargetKind::Vue3Ssr,
            },
            "compile",
            Some(&function_detail),
        );

        assert!(core_base_compile.contains(
            "emitVue3CompileDiagnostics(__vuecResult, __vuecPayload.options, __vuecPayload.source)"
        ));
        assert!(core_generate
            .contains("emitVue3CompileDiagnostics(__vuecGenerateResult, __vuecPayload.options, __vuecPayload.source)"));
        assert!(dom_compile.contains(
            "emitVue3CompileDiagnostics(__vuecResult, __vuecPayload.options, __vuecPayload.source)"
        ));
        assert!(ssr_compile.contains("hydrateVue3SsrCompileResult"));
        assert!(ssr_compile.contains("__vuecPayload.options"));
        assert!(ssr_compile.contains("__vuecPayload.source"));
        assert!(ALIAS_RUNTIME_JS
            .contains("function emitVue3CompileDiagnostics(result, options, source)"));
        assert!(ALIAS_RUNTIME_JS
            .contains("const onWarn = options && typeof options.onWarn === 'function'"));
        assert!(ALIAS_RUNTIME_JS.contains("delete result.diagnostics"));
        assert!(ALIAS_RUNTIME_JS.contains("function vue3DiagnosticLoc(diagnostic, source)"));
    }

    #[test]
    fn napi_vue3_ssr_native_alias_normalizes_custom_element_predicates() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let source = fs::read_to_string(
            repo_root
                .join("packages")
                .join("native-aliases")
                .join("@vue")
                .join("compiler-ssr")
                .join("dist")
                .join("compiler-ssr.cjs.js"),
        )
        .unwrap();

        assert!(source.contains("vue3SsrNativeOptions(options, template)"));
        assert!(source.contains("Object.assign(vue3SsrNativeOptions(options, template)"));
        assert!(source.contains(
            "out.__vuecCustomElements = collectVuePredicateHits(options.isCustomElement, tags);"
        ));
        assert!(!source.contains("native.compileVue3Ssr(String(source || ''), options || {})"));
    }

    #[test]
    fn sfc_compile_style_alias_strips_public_source_map_option() {
        assert!(ALIAS_RUNTIME_JS.contains("key !== 'sourceMap' && key !== 'source_map'"));
        assert!(ALIAS_RUNTIME_JS.contains("key !== 'sourceMap'"));
    }

    #[test]
    fn option_matrix_compile_style_passes_css_source_on_both_sides() {
        assert!(OPTION_MATRIX_PROBE_SCRIPT
            .contains("api.compileStyle(optionObjectWithSource(extractStyleSource(fixture)))"));
        assert!(!OPTION_MATRIX_PROBE_SCRIPT
            .contains("side === 'official' ? extractStyleSource(fixture) : fixture"));
    }

    #[test]
    fn vue27_sfc_compile_style_async_alias_returns_postcss_promise_adapter() {
        let target = TargetSpec {
            version_line: VersionLine::Vue27,
            package: "vue",
            entry: "vue/compiler-sfc",
            kind: TargetKind::Vue27Sfc,
        };
        let detail = ApiExportDetail {
            kind: "function".into(),
            tag: "[object Function]".into(),
            name: Some("compileStyleAsync".into()),
            function_arity: Some(1),
            is_async_function: Some(false),
            is_class_like: Some(false),
            own_property_names: vec!["length".into(), "name".into(), "prototype".into()],
        };
        let expression = alias_export_expression(target, "compileStyleAsync", Some(&detail));

        assert!(expression.contains("applyVue27StylePostcssAsync"));
        assert!(expression.contains("sfc.vue27.compileStyleAsync"));
        assert!(ALIAS_RUNTIME_JS.contains("function applyVue27StylePostcssAsync"));
        assert!(
            ALIAS_RUNTIME_JS.contains("return Promise.resolve(normalizeStyleAliasResult(out));")
        );
    }

    #[test]
    fn report_value_status_uses_counts_and_nested_rows() {
        let passed = serde_json::json!({
            "counts": { "total": 1, "pass": 1, "pending": 0, "fail": 0 },
            "targets": [
                { "rows": [{ "status": "pass" }] },
                { "checks": [{ "status": "pass" }] }
            ]
        });
        assert_eq!(report_value_status(&passed), ReportStatus::Pass);

        let pending = serde_json::json!({
            "counts": { "total": 2, "pass": 1, "pending": 1, "fail": 0 },
            "targets": [{ "rows": [{ "status": "pending" }] }]
        });
        assert_eq!(report_value_status(&pending), ReportStatus::Pending);

        let failed = serde_json::json!({
            "counts": { "total": 1, "pass": 0, "pending": 0, "fail": 1 },
            "targets": [{ "checks": [{ "status": "fail" }] }]
        });
        assert_eq!(report_value_status(&failed), ReportStatus::Fail);
    }

    #[test]
    fn report_value_status_treats_discovery_only_as_pending_via_counts() {
        let discovered = serde_json::json!({
            "execution": "discovery-only",
            "counts": { "total": 3, "pass": 0, "pending": 3, "fail": 0 }
        });
        assert_eq!(report_value_status(&discovered), ReportStatus::Pending);
    }
