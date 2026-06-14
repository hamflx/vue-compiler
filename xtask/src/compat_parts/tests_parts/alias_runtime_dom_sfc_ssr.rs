    #[test]
    fn vue3_alias_runtime_projects_transform_hoist_to_rust_stringify_option() {
        assert!(ALIAS_RUNTIME_JS.contains(
            "normalized.__vuecStringifyStatic = typeof options.transformHoist === 'function';"
        ));
    }

    #[test]
    fn vue2_alias_runtime_emits_compile_warnings() {
        assert!(ALIAS_RUNTIME_JS.contains("function emitVue2CompileWarnings(result, options)"));
        assert!(ALIAS_RUNTIME_JS.contains("function hydrateVue2CompileResult(result)"));
        assert!(ALIAS_RUNTIME_JS.contains("'element_public_ast'"));
        assert!(ALIAS_RUNTIME_JS.contains("Object.defineProperty(out, key"));
        assert!(ALIAS_RUNTIME_JS.contains("__vuecSuppressWarnings"));
        assert!(ALIAS_RUNTIME_JS.contains("console.error(message)"));
        let target = TargetSpec {
            version_line: VersionLine::Vue26,
            package: "vue-template-compiler",
            entry: "index",
            kind: TargetKind::Vue26Template,
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
        assert!(alias_export_expression(target, "compile", Some(&detail))
            .contains("emitVue2CompileWarnings(__vuecVue2Result, __vuecPayload.options)"));
        assert!(alias_export_expression(target, "compile", Some(&detail))
            .contains("return hydrateVue2CompileResult(__vuecVue2Result)"));
    }

    #[test]
    fn vue2_generate_code_frame_alias_reads_all_arguments() {
        let target = TargetSpec {
            version_line: VersionLine::Vue27,
            package: "vue-template-compiler",
            entry: "index",
            kind: TargetKind::Vue27Template,
        };
        let detail = ApiExportDetail {
            kind: "function".into(),
            tag: "[object Function]".into(),
            name: Some("generateCodeFrame".into()),
            function_arity: Some(1),
            is_async_function: Some(false),
            is_class_like: Some(false),
            own_property_names: vec!["length".into(), "name".into(), "prototype".into()],
        };
        let expression = alias_export_expression(target, "generateCodeFrame", Some(&detail));
        assert!(expression.contains("const a1 = arguments[1];"));
        assert!(expression.contains("const a2 = arguments[2];"));
        assert!(expression.contains("callBridge(\"vue2.generateCodeFrame\""));
    }

    #[test]
    fn vue3_alias_runtime_dehydrates_public_ast_import_paths() {
        assert!(ALIAS_RUNTIME_JS.contains("key === 'imports' || key === 'path'"));
    }

    #[test]
    fn vue3_dom_conformance_coverage_is_mixed() {
        let coverage = conformance_coverage_report(
            suite_spec(ConformanceSuite::Vue3Dom),
            AliasBackend::Generated,
            None,
        );
        assert_eq!(coverage.source, ConformanceCoverageKind::Mixed);
        assert!(coverage.reason.contains("official DOM source imports"));
    }

    #[test]
    fn vue3_dom_coverage_records_public_api_in_provenance_summary() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-vue3-dom-coverage-{}",
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
                  "name": "F:/repo/prepared/vue3-dom/packages/compiler-dom/__tests__/index.spec.ts",
                  "assertionResults": [
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-dom/packages/compiler-dom/__tests__/parse.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-dom/packages/compiler-dom/__tests__/transforms/transformStyle.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-dom/packages/compiler-dom/__tests__/transforms/ignoreSideEffectTags.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-dom/packages/compiler-dom/__tests__/transforms/vHtml.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-dom/packages/compiler-dom/__tests__/transforms/vText.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-dom/packages/compiler-dom/__tests__/transforms/vShow.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-dom/packages/compiler-dom/__tests__/transforms/vOn.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-dom/packages/compiler-dom/__tests__/transforms/vModel.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-dom/packages/compiler-dom/__tests__/transforms/Transition.spec.ts",
                  "assertionResults": [
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-dom/packages/compiler-dom/__tests__/transforms/validateHtmlNesting.spec.ts",
                  "assertionResults": [
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-dom/packages/compiler-dom/__tests__/decoderHtmlBrowser.spec.ts",
                  "assertionResults": [
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-dom/packages/compiler-dom/__tests__/transforms/stringifyStatic.spec.ts",
                  "assertionResults": [
                    { "status": "failed" }
                  ]
                }
              ]
            }"#,
        )
        .unwrap();
        let manifest_file = write_test_manifest(
            &temp,
            prepared_test_manifest_for_suite(suite_spec(ConformanceSuite::Vue3Dom)),
        );
        let execution = ConformanceExecutionResult {
            status: "failed".into(),
            runner: "vitest".into(),
            prepared_root: "prepared".into(),
            prepared_manifest_file: manifest_file,
            output_file: report.display().to_string(),
            exit_code: Some(1),
            stdout: String::new(),
            stderr: String::new(),
            counts: ConformanceExecutionCounts {
                total: 24,
                pass: 23,
                fail: 1,
                skip: 0,
                pending: 0,
            },
        };

        let coverage = conformance_coverage_report(
            suite_spec(ConformanceSuite::Vue3Dom),
            AliasBackend::Generated,
            Some(&execution),
        );

        assert_eq!(coverage.source, ConformanceCoverageKind::Mixed);
        assert_eq!(coverage.rust_backed_pass, 0);
        assert_eq!(coverage.rust_backed_total, 0);
        assert_eq!(
            coverage
                .counts_by_source
                .get("mixed")
                .copied()
                .unwrap_or_default(),
            ConformanceExecutionCounts {
                total: 24,
                pass: 23,
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
                .unwrap_or_default(),
            ConformanceExecutionCounts {
                total: 1,
                pass: 1,
                fail: 0,
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
                total: 23,
                pass: 22,
                fail: 1,
                skip: 0,
                pending: 0,
            }
        );
        assert_eq!(
            coverage.files[0].source,
            ConformanceCoverageKind::RustBacked
        );
        assert_eq!(
            coverage.files[0].provenance.execution_path,
            "rust-bridge-shape-adapter"
        );
        assert_eq!(coverage.files[1].source, ConformanceCoverageKind::Mixed);
        assert!(coverage.files.iter().skip(1).all(|file| {
            file.source == ConformanceCoverageKind::Mixed
                && file.provenance.execution_path == "hybrid-js-adapter-rust-projection"
        }));
        assert!(coverage.files[0]
            .reason
            .contains("routed through vuec_node_bridge"));
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn vue3_sfc_conformance_shims_use_sfc_vitest_glob() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-vue3-sfc-shims-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        write_vue3_core_source_shims(&temp).unwrap();
        write_vue3_sfc_conformance_shims(&temp).unwrap();

        let config = fs::read_to_string(temp.join("vitest.config.ts")).unwrap();
        assert!(!config.contains("vitest/config"));
        assert!(config.contains("oxc:"));
        assert!(config.contains("target: 'es2020'"));
        assert!(config.contains("fileParallelism: false"));
        assert!(config.contains("maxWorkers: 1"));
        assert!(config.contains("include: ['packages/compiler-sfc/__tests__/**/*.spec.ts']"));
        assert!(config.contains(
            "'@vue/compiler-core': path.resolve(aliasRoot, 'node_modules/@vue/compiler-core/index.js')"
        ));
        assert!(config.contains(
            "'@vue/compiler-dom': path.resolve(aliasRoot, 'node_modules/@vue/compiler-dom/index.js')"
        ));
        assert!(config.contains(
            "'@vue/compiler-ssr': path.resolve(aliasRoot, 'node_modules/@vue/compiler-ssr/dist/compiler-ssr.cjs.js')"
        ));
        assert!(config.contains(
            "'@vue/compiler-sfc': path.resolve(aliasRoot, 'node_modules/@vue/compiler-sfc/dist/compiler-sfc.cjs.js')"
        ));
        assert!(config
            .contains("'hash-sum': path.resolve(npmRoot, 'node_modules/hash-sum/hash-sum.js')"));
        assert!(config.contains(
            "'lru-cache': path.resolve(npmRoot, 'node_modules/lru-cache/dist/esm/index.js')"
        ));
        assert!(config
            .contains("'postcss': path.resolve(npmRoot, 'node_modules/postcss/lib/postcss.mjs')"));
        assert!(config.contains(
            "'@babel/parser': path.resolve(npmRoot, 'node_modules/@babel/parser/lib/index.js')"
        ));
        let package_json = fs::read_to_string(temp.join("package.json")).unwrap();
        assert!(package_json.contains("\"type\": \"module\""));
        let transform_element = fs::read_to_string(
            temp.join("packages")
                .join("compiler-core")
                .join("src")
                .join("transforms")
                .join("transformElement.ts"),
        )
        .unwrap();
        assert!(transform_element.contains("__vuecRuntime"));
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn vue3_sfc_compile_template_patch_projects_asset_options() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-vue3-sfc-asset-patch-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&temp).unwrap();
        let path = temp.join("compileTemplate.ts");
        fs::write(
            &path,
            "compile({\r\n    mode: 'module',\r\n    ...compilerOptions,\r\n    hmr: !isProd,\r\n})\r\n",
        )
        .unwrap();

        patch_vue3_sfc_compile_template_asset_bridge(&path).unwrap();
        patch_vue3_sfc_compile_template_asset_bridge(&path).unwrap();

        let patched = fs::read_to_string(&path).unwrap();
        assert_eq!(patched.matches("transformAssetUrls:").count(), 1);
        assert!(patched.contains("normalizeOptions(transformAssetUrls)"));
        assert!(patched.contains("(compilerOptions as any).transformAssetUrls"));
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn vue3_sfc_public_api_specs_rewrite_imports_to_public_alias() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-vue3-sfc-public-api-import-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let tests = temp.join("packages").join("compiler-sfc").join("__tests__");
        fs::create_dir_all(&tests).unwrap();
        fs::write(
            tests.join("parse.spec.ts"),
            "import { parse } from '../src'\nimport { compileScript } from '../src'\n",
        )
        .unwrap();
        fs::write(
            tests.join("rewriteDefault.spec.ts"),
            "import { rewriteDefault } from '../src'\nimport { rewriteDefaultAST } from '../src/rewriteDefault'\n",
        )
        .unwrap();
        fs::write(
            tests.join("compileStyle.spec.ts"),
            "import { compileStyle, compileStyleAsync } from '../src/compileStyle'\n",
        )
        .unwrap();
        fs::write(
            tests.join("compileTemplate.spec.ts"),
            "import {\n  type SFCTemplateCompileOptions,\n  compileTemplate,\n} from '../src/compileTemplate'\nimport { type SFCTemplateBlock, parse } from '../src/parse'\nimport { compileScript } from '../src'\nimport { getPositionInCode } from './utils'\n",
        )
        .unwrap();
        fs::write(
            tests.join("cssVars.spec.ts"),
            "import { compileStyle, parse } from '../src'\nimport { assertCode, compileSFCScript, mockId } from './utils'\n",
        )
        .unwrap();
        fs::write(
            tests.join("compileScript.spec.ts"),
            "import { assertCode, compileSFCScript as compile } from './utils'\n",
        )
        .unwrap();
        let compile_script_tests = tests.join("compileScript");
        fs::create_dir_all(&compile_script_tests).unwrap();
        for compile_script_spec in [
            "defineProps.spec.ts",
            "definePropsDestructure.spec.ts",
            "defineEmits.spec.ts",
            "defineExpose.spec.ts",
            "defineModel.spec.ts",
            "defineOptions.spec.ts",
            "defineSlots.spec.ts",
            "hoistStatic.spec.ts",
            "importUsageCheck.spec.ts",
        ] {
            fs::write(
                compile_script_tests.join(compile_script_spec),
                "import { assertCode, compileSFCScript as compile } from '../utils'\n",
            )
            .unwrap();
        }
        fs::write(
            compile_script_tests.join("resolveType.spec.ts"),
            format!(
                "{}\ndescribe('resolveType', () => {{\n  test('type literal', () => {{\n    const {{ props, calls }} = resolve(`defineProps<{{ foo: number; (e: 'save'): void }}>()`)\n    expect(props).toStrictEqual({{ foo: ['Number'] }})\n    expect(calls?.length).toBe(1)\n    expect(UNKNOWN_TYPE).toBe('Unknown')\n  }})\n}})\n\n{}",
                VUE3_SFC_RESOLVE_TYPE_INTERNAL_IMPORTS,
                VUE3_SFC_RESOLVE_TYPE_INTERNAL_HELPER
            ),
        )
        .unwrap();
        fs::write(
            tests.join("templateUtils.spec.ts"),
            "import {\n  isDataUrl,\n  isExternalUrl,\n  isRelativeUrl,\n} from '../src/template/templateUtils'\n",
        )
        .unwrap();
        fs::write(
            tests.join("templateTransformAssetUrl.spec.ts"),
            format!(
                "{}\ndescribe('asset url', () => {{ compileWithAssetUrls('<img src=\"./x.png\"/>') }})\n",
                VUE3_SFC_ASSET_TRANSFORM_INTERNAL_HELPER
            ),
        )
        .unwrap();
        fs::write(
            tests.join("templateTransformSrcset.spec.ts"),
            format!(
                "{}\ndescribe('srcset', () => {{ compileWithSrcset('<img srcset=\"./x.png 2x\"/>') }})\n",
                VUE3_SFC_SRCSET_TRANSFORM_INTERNAL_HELPER
            ),
        )
        .unwrap();
        fs::write(
            tests.join("utils.ts"),
            "import { compileScript, parse } from '../src'\n",
        )
        .unwrap();

        rewrite_vue3_sfc_public_api_spec_imports(&temp).unwrap();
        rewrite_vue3_sfc_public_api_spec_imports(&temp).unwrap();

        let parse_spec = fs::read_to_string(tests.join("parse.spec.ts")).unwrap();
        assert!(parse_spec.contains("import { parse } from '@vue/compiler-sfc'"));
        assert!(parse_spec.contains("import { compileScript } from '../src'"));
        assert_eq!(parse_spec.matches("@vue/compiler-sfc").count(), 1);

        let rewrite_default_spec =
            fs::read_to_string(tests.join("rewriteDefault.spec.ts")).unwrap();
        assert!(rewrite_default_spec.contains("import { rewriteDefault } from '@vue/compiler-sfc'"));
        assert!(rewrite_default_spec
            .contains("import { rewriteDefaultAST } from '../src/rewriteDefault'"));
        assert_eq!(rewrite_default_spec.matches("@vue/compiler-sfc").count(), 1);

        let compile_style_spec = fs::read_to_string(tests.join("compileStyle.spec.ts")).unwrap();
        assert!(compile_style_spec
            .contains("import { compileStyle, compileStyleAsync } from '@vue/compiler-sfc'"));
        assert_eq!(compile_style_spec.matches("@vue/compiler-sfc").count(), 1);

        let compile_template_spec =
            fs::read_to_string(tests.join("compileTemplate.spec.ts")).unwrap();
        assert!(compile_template_spec.contains("from '@vue/compiler-sfc'"));
        assert!(compile_template_spec.contains("import { compileScript } from '@vue/compiler-sfc'"));
        assert!(compile_template_spec.contains("from './utils.public-api'"));
        assert!(!compile_template_spec.contains("../src/compileTemplate"));
        assert!(!compile_template_spec.contains("../src/parse"));
        assert_eq!(
            compile_template_spec.matches("@vue/compiler-sfc").count(),
            3
        );

        let css_vars_spec = fs::read_to_string(tests.join("cssVars.spec.ts")).unwrap();
        assert!(css_vars_spec.contains("import { compileStyle, parse } from '@vue/compiler-sfc'"));
        assert!(
            css_vars_spec.contains("from './utils.public-api'"),
            "cssVars should use a dedicated helper so shared utils.ts remains scoped to mixed files"
        );
        assert_eq!(css_vars_spec.matches("@vue/compiler-sfc").count(), 1);
        let root_compile_script_spec =
            fs::read_to_string(tests.join("compileScript.spec.ts")).unwrap();
        assert!(root_compile_script_spec.contains("from './utils.public-api'"));
        assert!(!root_compile_script_spec.contains("from './utils'"));
        let shared_utils = fs::read_to_string(tests.join("utils.ts")).unwrap();
        assert!(shared_utils.contains("from '../src'"));
        let public_utils = fs::read_to_string(tests.join("utils.public-api.ts")).unwrap();
        assert!(public_utils.contains("from '@vue/compiler-sfc'"));
        assert!(public_utils.contains("export function compileSFCScript"));
        assert!(public_utils.contains("__vuecEmitScriptSetupMarker: false"));
        assert!(public_utils.contains("babelParse(code"));
        assert!(public_utils.contains("export function getPositionInCode"));
        let template_utils_spec = fs::read_to_string(tests.join("templateUtils.spec.ts")).unwrap();
        assert!(template_utils_spec.contains("from './templateUtils.rust-api'"));
        assert!(!template_utils_spec.contains("../src/template/templateUtils"));
        let template_utils_api =
            fs::read_to_string(tests.join("templateUtils.rust-api.ts")).unwrap();
        assert!(template_utils_api.contains("from '@vue/compiler-sfc'"));
        assert!(template_utils_api.contains("sfc.templateUtils.isRelativeUrl"));
        assert!(template_utils_api.contains("sfc.templateUtils.isExternalUrl"));
        assert!(template_utils_api.contains("sfc.templateUtils.isDataUrl"));
        let asset_transform_spec =
            fs::read_to_string(tests.join("templateTransformAssetUrl.spec.ts")).unwrap();
        assert!(asset_transform_spec.contains("from './templateTransforms.public-api'"));
        assert!(asset_transform_spec.contains("compileWithAssetUrls"));
        assert!(!asset_transform_spec.contains("@vue/compiler-core"));
        assert!(!asset_transform_spec.contains("../src/template/transformAssetUrl"));
        assert!(!asset_transform_spec.contains("../../compiler-dom/src/transforms/stringifyStatic"));
        let srcset_transform_spec =
            fs::read_to_string(tests.join("templateTransformSrcset.spec.ts")).unwrap();
        assert!(srcset_transform_spec.contains("from './templateTransforms.public-api'"));
        assert!(srcset_transform_spec.contains("compileWithSrcset"));
        assert!(!srcset_transform_spec.contains("@vue/compiler-core"));
        assert!(!srcset_transform_spec.contains("../src/template/transformSrcset"));
        assert!(
            !srcset_transform_spec.contains("../../compiler-dom/src/transforms/stringifyStatic")
        );
        let template_transforms_api =
            fs::read_to_string(tests.join("templateTransforms.public-api.ts")).unwrap();
        assert!(template_transforms_api.contains("compileTemplate"));
        assert!(template_transforms_api.contains("export function compileWithAssetUrls"));
        assert!(template_transforms_api.contains("export function compileWithSrcset"));
        assert!(template_transforms_api.contains("compilerOptions"));
        assert!(template_transforms_api.contains("transformAssetUrls"));
        assert!(template_transforms_api.contains("img: []"));
        for compile_script_spec in [
            "defineProps.spec.ts",
            "definePropsDestructure.spec.ts",
            "defineEmits.spec.ts",
            "defineExpose.spec.ts",
            "defineModel.spec.ts",
            "defineOptions.spec.ts",
            "defineSlots.spec.ts",
            "hoistStatic.spec.ts",
            "importUsageCheck.spec.ts",
        ] {
            let spec = fs::read_to_string(compile_script_tests.join(compile_script_spec)).unwrap();
            assert!(spec.contains("from '../utils.public-api'"));
            assert!(!spec.contains("from '../utils'"));
        }
        let resolve_type_spec =
            fs::read_to_string(compile_script_tests.join("resolveType.spec.ts")).unwrap();
        assert!(resolve_type_spec.contains("from './resolveType.rust-api'"));
        assert!(!resolve_type_spec.contains("../../src/script/resolveType"));
        assert!(!resolve_type_spec.contains("../../src/script/context"));
        assert!(!resolve_type_spec.contains("../../src/script/utils"));
        assert!(!resolve_type_spec.contains("registerTS(() => ts)"));
        assert!(!resolve_type_spec.contains("function resolve("));
        let resolve_type_api =
            fs::read_to_string(compile_script_tests.join("resolveType.rust-api.ts")).unwrap();
        assert!(resolve_type_api.contains("from '@vue/compiler-sfc'"));
        assert!(resolve_type_api.contains("__vuecRuntime"));
        assert!(resolve_type_api.contains("sfc.resolveType"));
        assert!(resolve_type_api.contains("globalTypeFiles"));
        assert!(resolve_type_api.contains("materializeFiles"));
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn vue3_sfc_conformance_coverage_is_mixed() {
        let coverage = conformance_coverage_report(
            suite_spec(ConformanceSuite::Vue3Sfc),
            AliasBackend::Generated,
            None,
        );
        assert_eq!(coverage.source, ConformanceCoverageKind::Mixed);
        assert!(coverage.reason.contains("official SFC TypeScript source"));
        assert!(coverage.reason.contains("not standalone Rust SFC parity"));
    }

    #[test]
    fn vue3_sfc_coverage_records_public_api_and_marker_downgrade() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-vue3-sfc-coverage-{}",
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
                  "name": "F:/repo/prepared/vue3-sfc/packages/compiler-sfc/__tests__/parse.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-sfc/packages/compiler-sfc/__tests__/rewriteDefault.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-sfc/packages/compiler-sfc/__tests__/compileStyle.spec.ts",
                  "coverageProvenance": ["callback.postcssPlugin"],
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-sfc/packages/compiler-sfc/__tests__/cssVars.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-sfc/packages/compiler-sfc/__tests__/compileScript.spec.ts",
                  "assertionResults": [
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-sfc/packages/compiler-sfc/__tests__/compileTemplate.spec.ts",
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
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-sfc/packages/compiler-sfc/__tests__/templateUtils.spec.ts",
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
                  "name": "F:/repo/prepared/vue3-sfc/packages/compiler-sfc/__tests__/templateTransformAssetUrl.spec.ts",
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
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-sfc/packages/compiler-sfc/__tests__/templateTransformSrcset.spec.ts",
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
                  "name": "F:/repo/prepared/vue3-sfc/packages/compiler-sfc/__tests__/compileScript/defineEmits.spec.ts",
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
                  "name": "F:/repo/prepared/vue3-sfc/packages/compiler-sfc/__tests__/compileScript/defineExpose.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-sfc/packages/compiler-sfc/__tests__/compileScript/defineModel.spec.ts",
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
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-sfc/packages/compiler-sfc/__tests__/compileScript/defineOptions.spec.ts",
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
                  "name": "F:/repo/prepared/vue3-sfc/packages/compiler-sfc/__tests__/compileScript/defineProps.spec.ts",
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
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-sfc/packages/compiler-sfc/__tests__/compileScript/definePropsDestructure.spec.ts",
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
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-sfc/packages/compiler-sfc/__tests__/compileScript/defineSlots.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-sfc/packages/compiler-sfc/__tests__/compileScript/hoistStatic.spec.ts",
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
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-sfc/packages/compiler-sfc/__tests__/compileScript/importUsageCheck.spec.ts",
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
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue3-sfc/packages/compiler-sfc/__tests__/compileScript/resolveType.spec.ts",
                  "assertionResults": [
                    { "status": "passed" }
                  ]
                }
              ]
            }"#,
        )
        .unwrap();
        let manifest_file = write_test_manifest(
            &temp,
            prepared_test_manifest_for_suite(suite_spec(ConformanceSuite::Vue3Sfc)),
        );
        let execution = ConformanceExecutionResult {
            status: "failed".into(),
            runner: "vitest".into(),
            prepared_root: "prepared".into(),
            prepared_manifest_file: manifest_file,
            output_file: report.display().to_string(),
            exit_code: Some(1),
            stdout: String::new(),
            stderr: String::new(),
            counts: ConformanceExecutionCounts {
                total: 199,
                pass: 199,
                fail: 0,
                skip: 0,
                pending: 0,
            },
        };

        let coverage = conformance_coverage_report(
            suite_spec(ConformanceSuite::Vue3Sfc),
            AliasBackend::Generated,
            Some(&execution),
        );

        assert_eq!(coverage.source, ConformanceCoverageKind::Mixed);
        assert_eq!(coverage.rust_backed_pass, 0);
        assert_eq!(coverage.rust_backed_total, 0);
        assert_eq!(
            coverage
                .counts_by_source
                .get("mixed")
                .copied()
                .unwrap_or_default(),
            ConformanceExecutionCounts {
                total: 199,
                pass: 199,
                fail: 0,
                skip: 0,
                pending: 0,
            }
        );
        assert_eq!(
            coverage.files[0].source,
            ConformanceCoverageKind::RustBacked
        );
        assert_eq!(
            coverage.files[1].source,
            ConformanceCoverageKind::RustBacked
        );
        assert_eq!(coverage.files[2].source, ConformanceCoverageKind::Mixed);
        assert_eq!(
            coverage.files[2].provenance.execution_path,
            "mixed-js-callback-boundary"
        );
        assert!(coverage.files[2]
            .provenance
            .runtime_markers
            .iter()
            .any(|marker| marker == "callback.postcssPlugin"));
        assert!(coverage.files[2]
            .reason
            .contains("PostCSS plugin callbacks"));
        assert_eq!(
            coverage
                .summary
                .get("mixed-js-callback-boundary")
                .copied()
                .unwrap_or_default(),
            ConformanceExecutionCounts {
                total: 3,
                pass: 3,
                fail: 0,
                skip: 0,
                pending: 0,
            }
        );
        assert_eq!(
            coverage
                .summary
                .get("rust-bridge-shape-adapter")
                .copied()
                .unwrap_or_default(),
            ConformanceExecutionCounts {
                total: 186,
                pass: 186,
                fail: 0,
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
                total: 10,
                pass: 10,
                fail: 0,
                skip: 0,
                pending: 0,
            }
        );
        assert_eq!(
            coverage.files[3].source,
            ConformanceCoverageKind::RustBacked
        );
        assert!(coverage.files[3].reason.contains("Babel syntax assertion"));
        assert_eq!(
            coverage.files[4].source,
            ConformanceCoverageKind::RustBacked
        );
        assert!(coverage.files[4]
            .reason
            .contains("Rust vuec_sfc compileScript implementation"));
        assert_eq!(
            coverage.files[5].source,
            ConformanceCoverageKind::RustBacked
        );
        assert!(coverage.files[5].reason.contains("compileTemplate file"));
        assert_eq!(coverage.files[6].source, ConformanceCoverageKind::Mixed);
        assert_eq!(
            coverage.files[6].provenance.api_surface,
            "projection-command"
        );
        assert_eq!(
            coverage.files[6].provenance.execution_path,
            "hybrid-js-adapter-rust-projection"
        );
        assert!(coverage.files[6].reason.contains("projection command"));
        assert!(coverage.files[6]
            .provenance
            .bridge_commands
            .iter()
            .any(|command| command == "sfc.templateUtils.isRelativeUrl"));
        assert_eq!(
            coverage.files[7].source,
            ConformanceCoverageKind::RustBacked
        );
        assert!(coverage.files[7].reason.contains("asset/srcset transform"));
        assert_eq!(
            coverage.files[8].source,
            ConformanceCoverageKind::RustBacked
        );
        assert!(coverage.files[8].reason.contains("asset/srcset transform"));
        for file in coverage.files.iter().skip(9).take(9) {
            assert_eq!(file.source, ConformanceCoverageKind::RustBacked);
            assert!(file
                .reason
                .contains("Rust vuec_sfc compileScript implementation"));
        }
        assert_eq!(coverage.files[18].source, ConformanceCoverageKind::Mixed);
        assert_eq!(
            coverage.files[18].provenance.api_surface,
            "projection-command"
        );
        assert_eq!(
            coverage.files[18].provenance.execution_path,
            "hybrid-js-adapter-rust-projection"
        );
        assert!(coverage.files[18].reason.contains("sfc.resolveType"));
        assert_eq!(
            coverage
                .counts_by_source
                .get("rust-backed")
                .copied()
                .unwrap_or_default()
                .pass,
            0
        );
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn vue3_ssr_conformance_shims_use_ssr_vitest_glob() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-vue3-ssr-shims-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        write_vue3_core_source_shims(&temp).unwrap();
        write_vue3_ssr_conformance_shims(&temp).unwrap();

        let config = fs::read_to_string(temp.join("vitest.config.ts")).unwrap();
        assert!(!config.contains("vitest/config"));
        assert!(config.contains("include: ['packages/compiler-ssr/__tests__/**/*.spec.ts']"));
        assert!(config.contains(
            "'@vue/compiler-dom': path.resolve(root, 'packages/compiler-dom/src/index.ts')"
        ));
        assert!(config.contains(
            "'@vue/compiler-ssr': path.resolve(aliasRoot, 'node_modules/@vue/compiler-ssr/dist/compiler-ssr.cjs.js')"
        ));
        assert!(config.contains("'packages/compiler-core/src/transform': path.resolve(root, 'packages/compiler-core/src/transform.ts')"));

        let transform = fs::read_to_string(
            temp.join("packages")
                .join("compiler-core")
                .join("src")
                .join("transform.ts"),
        )
        .unwrap();
        assert!(transform.contains("export * from \"@vue/compiler-core\""));
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn vue3_ssr_rust_backed_specs_route_compile_to_public_alias() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-vue3-ssr-rust-backed-routing-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let tests = temp.join("packages").join("compiler-ssr").join("__tests__");
        fs::create_dir_all(&tests).unwrap();
        fs::write(
            tests.join("ssrText.spec.ts"),
            "import { compile } from '../src'\nimport { getCompiledString } from './utils'\n",
        )
        .unwrap();
        fs::write(
            tests.join("ssrVIf.spec.ts"),
            "import { compile } from '../src'\n",
        )
        .unwrap();
        fs::write(
            tests.join("ssrVFor.spec.ts"),
            "import { compile } from '../src'\n",
        )
        .unwrap();
        fs::write(
            tests.join("ssrScopeId.spec.ts"),
            "import { compile } from '../src'\n",
        )
        .unwrap();
        fs::write(
            tests.join("ssrFallthroughAttrs.spec.ts"),
            "import { compile } from '../src'\n",
        )
        .unwrap();
        fs::write(
            tests.join("ssrInjectCssVars.spec.ts"),
            "import { compile } from '../src'\n",
        )
        .unwrap();
        fs::write(
            tests.join("ssrVShow.spec.ts"),
            "import { compile } from '../src'\n",
        )
        .unwrap();
        fs::write(
            tests.join("ssrVModel.spec.ts"),
            "import { compile } from '../src'\n",
        )
        .unwrap();
        fs::write(
            tests.join("ssrElement.spec.ts"),
            "import { compile } from '../src'\nimport { getCompiledString } from './utils'\n",
        )
        .unwrap();
        fs::write(
            tests.join("ssrSlotOutlet.spec.ts"),
            "import { compile } from '../src'\n",
        )
        .unwrap();
        fs::write(
            tests.join("ssrPortal.spec.ts"),
            "import { compile } from '../src'\n",
        )
        .unwrap();
        fs::write(
            tests.join("ssrSuspense.spec.ts"),
            "import { compile } from '../src'\n",
        )
        .unwrap();
        fs::write(
            tests.join("ssrTransition.spec.ts"),
            "import { compile } from '../src'\n",
        )
        .unwrap();
        fs::write(
            tests.join("ssrTransitionGroup.spec.ts"),
            "import { compile } from '../src'\n",
        )
        .unwrap();
        fs::write(
            tests.join("ssrComponent.spec.ts"),
            "import { compile } from '../src'\n",
        )
        .unwrap();
        fs::write(tests.join("utils.ts"), "import { compile } from '../src'\n").unwrap();

        rewrite_vue3_ssr_rust_backed_public_compile_imports(&temp).unwrap();

        let spec = fs::read_to_string(tests.join("ssrText.spec.ts")).unwrap();
        let vif_spec = fs::read_to_string(tests.join("ssrVIf.spec.ts")).unwrap();
        let vfor_spec = fs::read_to_string(tests.join("ssrVFor.spec.ts")).unwrap();
        let scope_id_spec = fs::read_to_string(tests.join("ssrScopeId.spec.ts")).unwrap();
        let fallthrough_attrs_spec =
            fs::read_to_string(tests.join("ssrFallthroughAttrs.spec.ts")).unwrap();
        let inject_css_vars_spec =
            fs::read_to_string(tests.join("ssrInjectCssVars.spec.ts")).unwrap();
        let vshow_spec = fs::read_to_string(tests.join("ssrVShow.spec.ts")).unwrap();
        let vmodel_spec = fs::read_to_string(tests.join("ssrVModel.spec.ts")).unwrap();
        let element_spec = fs::read_to_string(tests.join("ssrElement.spec.ts")).unwrap();
        let slot_outlet_spec = fs::read_to_string(tests.join("ssrSlotOutlet.spec.ts")).unwrap();
        let portal_spec = fs::read_to_string(tests.join("ssrPortal.spec.ts")).unwrap();
        let suspense_spec = fs::read_to_string(tests.join("ssrSuspense.spec.ts")).unwrap();
        let transition_spec = fs::read_to_string(tests.join("ssrTransition.spec.ts")).unwrap();
        let transition_group_spec =
            fs::read_to_string(tests.join("ssrTransitionGroup.spec.ts")).unwrap();
        let component_spec = fs::read_to_string(tests.join("ssrComponent.spec.ts")).unwrap();
        let utils = fs::read_to_string(tests.join("utils.ts")).unwrap();
        let rust_text_utils = fs::read_to_string(tests.join("utils.rust-ssr-text.ts")).unwrap();
        assert!(spec.contains("from '@vue/compiler-ssr'"));
        assert!(spec.contains("from './utils.rust-ssr-text'"));
        assert!(vif_spec.contains("from '@vue/compiler-ssr'"));
        assert!(vfor_spec.contains("from '@vue/compiler-ssr'"));
        assert!(scope_id_spec.contains("from '@vue/compiler-ssr'"));
        assert!(fallthrough_attrs_spec.contains("from '@vue/compiler-ssr'"));
        assert!(inject_css_vars_spec.contains("from '@vue/compiler-ssr'"));
        assert!(vshow_spec.contains("from '@vue/compiler-ssr'"));
        assert!(vmodel_spec.contains("from '@vue/compiler-ssr'"));
        assert!(element_spec.contains("from '@vue/compiler-ssr'"));
        assert!(element_spec.contains("from './utils.rust-ssr-text'"));
        assert!(slot_outlet_spec.contains("from '@vue/compiler-ssr'"));
        assert!(portal_spec.contains("from '@vue/compiler-ssr'"));
        assert!(suspense_spec.contains("from '@vue/compiler-ssr'"));
        assert!(transition_spec.contains("from '@vue/compiler-ssr'"));
        assert!(transition_group_spec.contains("from '@vue/compiler-ssr'"));
        assert!(component_spec.contains("from '@vue/compiler-ssr'"));
        assert!(utils.contains("from '../src'"));
        assert!(!utils.contains("from '@vue/compiler-ssr'"));
        assert!(rust_text_utils.contains("from '@vue/compiler-ssr'"));
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn vue3_ssr_conformance_coverage_is_mixed() {
        let coverage = conformance_coverage_report(
            suite_spec(ConformanceSuite::Vue3Ssr),
            AliasBackend::Generated,
            None,
        );
        assert_eq!(coverage.source, ConformanceCoverageKind::Mixed);
        assert!(coverage
            .reason
            .contains("official SSR and DOM source imports"));
    }
