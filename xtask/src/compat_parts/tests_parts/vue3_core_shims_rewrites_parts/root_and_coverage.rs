    #[test]
    fn vue3_core_transform_spec_uses_prepared_rust_api_for_root_codegen() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-vue3-core-transform-shim-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let tests = temp
            .join("packages")
            .join("compiler-core")
            .join("__tests__");
        fs::create_dir_all(&tests).unwrap();
        fs::write(
            tests.join("transform.spec.ts"),
            r#"import { baseParse } from '../src/parser'
import { type NodeTransform, transform } from '../src/transform'
import {
  type DirectiveNode,
  type ElementNode,
  type ExpressionNode,
  NodeTypes,
  type VNodeCall,
} from '../src/ast'
import { ErrorCodes, createCompilerError } from '../src/errors'
import {
  CREATE_COMMENT,
  FRAGMENT,
  RENDER_SLOT,
  TO_DISPLAY_STRING,
} from '../src/runtimeHelpers'
import { transformIf } from '../src/transforms/vIf'
import { transformFor } from '../src/transforms/vFor'
import { transformElement } from '../src/transforms/transformElement'
import { transformSlotOutlet } from '../src/transforms/transformSlotOutlet'
import { transformText } from '../src/transforms/transformText'
import { PatchFlags } from '@vue/shared'

describe('compiler: transform', () => {
  test('context state', () => {
    const ast = baseParse(`<div>hello</div>`)
    const plugin: NodeTransform = () => {}
    transform(ast, { nodeTransforms: [plugin] })
    expect(ast).toBeTruthy()
  })

  test('should inject toString helper for interpolations', () => {
    const ast = baseParse(`{{ foo }}`)
    transform(ast, {})
    expect(ast.helpers).toContain(TO_DISPLAY_STRING)
  })

  test('should inject createVNode and Comment for comments', () => {
    const ast = baseParse(`<!--foo-->`)
    transform(ast, {})
    expect(ast.helpers).toContain(CREATE_COMMENT)
  })

  describe('root codegenNode', () => {
    function transformWithCodegen(template: string) {
      const ast = baseParse(template)
      transform(ast, {
        nodeTransforms: [
          transformIf,
          transformFor,
          transformText,
          transformSlotOutlet,
          transformElement,
        ],
      })
      return ast
    }

    test('single element', () => {
      const ast = transformWithCodegen(`<div/>`)
      expect(ast.codegenNode).toMatchObject({ type: NodeTypes.VNODE_CALL })
    })
  })
})
"#,
        )
        .unwrap();

        rewrite_vue3_core_transform_public_api_spec(&temp).unwrap();

        let spec = fs::read_to_string(tests.join("transform.spec.ts")).unwrap();
        assert!(spec.contains("from './transform.rust-api'"));
        assert!(spec.contains("const ast = transformWithCodegen(`{{ foo }}`)"));
        assert!(spec.contains("const ast = transformWithCodegen(`<!--foo-->`)"));
        assert!(!spec.contains("function transformWithCodegen(template: string)"));
        assert!(spec.contains("const plugin: NodeTransform = () => {}"));

        let api = fs::read_to_string(tests.join("transform.rust-api.ts")).unwrap();
        assert!(api.contains("callBridge('vue3.core.transformSuite'"));
        assert!(api.contains("hydrateTransformAst"));
        assert!(api.contains("node.codegenNode = undefined"));
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn vue27_sfc_coverage_marks_compile_style_mixed_postcss_boundary() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-vue27-sfc-coverage-{}",
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
                  "name": "F:/repo/prepared/vue27-sfc/packages/compiler-sfc/test/compileScript.spec.ts",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "passed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue27-sfc/packages/compiler-sfc/test/compileStyle.spec.ts",
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
                total: 4,
                pass: 3,
                fail: 1,
                skip: 0,
                pending: 0,
            },
        };

        let coverage = conformance_coverage_report(
            suite_spec(ConformanceSuite::Vue27Sfc),
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
                .unwrap_or_default()
                .total,
            4
        );
        assert_eq!(
            coverage
                .summary
                .get("hybrid-js-adapter-rust-projection")
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
        assert_eq!(
            coverage
                .summary
                .get("mixed-js-callback-boundary")
                .copied()
                .unwrap_or_default(),
            ConformanceExecutionCounts {
                total: 2,
                pass: 1,
                fail: 1,
                skip: 0,
                pending: 0,
            }
        );
        assert_eq!(coverage.files[0].source, ConformanceCoverageKind::Mixed);
        assert_eq!(
            coverage.files[0].provenance.execution_path,
            "hybrid-js-adapter-rust-projection"
        );
        assert!(!coverage.files[0].reason.contains("PostCSS"));
        assert_eq!(coverage.files[1].source, ConformanceCoverageKind::Mixed);
        assert_eq!(
            coverage.files[1].provenance.execution_path,
            "mixed-js-callback-boundary"
        );
        assert!(coverage.files[1]
            .reason
            .contains("PostCSS plugin callbacks"));
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn vue2_jasmine_coverage_report_reads_per_file_results() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-vue2-jasmine-coverage-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&temp).unwrap();
        let report = temp.join("jasmine-report.json");
        fs::write(
            &report,
            r#"{
              "counts": { "total": 3, "pass": 1, "fail": 1, "skip": 1, "pending": 0 },
              "testResults": [
                {
                  "name": "F:/repo/prepared/vue2-compiler/test/unit/modules/compiler/codegen.spec.js",
                  "assertionResults": [
                    { "status": "passed" },
                    { "status": "failed" }
                  ]
                },
                {
                  "name": "F:/repo/prepared/vue2-compiler/test/unit/modules/compiler/parser.spec.js",
                  "assertionResults": [
                    { "status": "skipped" }
                  ]
                }
              ]
            }"#,
        )
        .unwrap();
        let execution = ConformanceExecutionResult {
            status: "failed".into(),
            runner: "jasmine".into(),
            prepared_root: "prepared".into(),
            prepared_manifest_file: None,
            output_file: report.display().to_string(),
            exit_code: Some(1),
            stdout: String::new(),
            stderr: String::new(),
            counts: ConformanceExecutionCounts {
                total: 3,
                pass: 1,
                fail: 1,
                skip: 1,
                pending: 0,
            },
        };

        let coverage = conformance_coverage_report(
            suite_spec(ConformanceSuite::Vue2Compiler),
            AliasBackend::Generated,
            Some(&execution),
        );

        assert_eq!(coverage.source, ConformanceCoverageKind::RustBacked);
        assert_eq!(coverage.files.len(), 2);
        assert_eq!(coverage.rust_backed_total, 3);
        assert_eq!(coverage.rust_backed_pass, 1);
        assert!(coverage.reason.contains("prepared Jasmine suite"));
        assert!(coverage.reason.contains("not-wired pending status"));
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn napi_conformance_coverage_marks_mixed_alias_backend() {
        let coverage = conformance_coverage_report(
            suite_spec(ConformanceSuite::Vue2Compiler),
            AliasBackend::Napi,
            None,
        );
        assert_eq!(coverage.source, ConformanceCoverageKind::Mixed);
        assert_eq!(coverage.rust_backed_total, 0);
        assert!(coverage
            .reason
            .contains("NAPI-backed official package-name alias"));
        assert!(coverage.reason.contains("mixed harness coverage"));
    }

    #[test]
    fn vue2_conformance_shims_use_official_runners_and_globs() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-vue2-shims-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&temp).unwrap();

        write_vue2_compiler_source_shims(&temp, true).unwrap();
        write_vue2_jasmine_runner(&temp).unwrap();
        write_vue27_compiler_conformance_shims(&temp).unwrap();
        let compiler_config = fs::read_to_string(temp.join("vitest.config.ts")).unwrap();
        assert!(compiler_config.contains("environment: 'jsdom'"));
        assert!(compiler_config.contains("include: ['test/unit/modules/compiler/**/*.spec.ts']"));
        assert!(compiler_config.contains(
            "'vue-template-compiler': path.resolve(aliasRoot, 'node_modules/vue-template-compiler/index.js')"
        ));
        let codegen =
            fs::read_to_string(temp.join("src").join("compiler").join("codegen.ts")).unwrap();
        assert!(codegen.contains("callBridge('vue2.generate'"));
        assert!(codegen.contains("export function normalizeVue2AstForBridge"));
        assert!(codegen.contains("events[key] = []"));
        assert!(codegen.contains("function normalizeVue2PublicElementForBridge"));
        assert!(codegen.contains("static_node: Boolean(node.static ?? node.static_node)"));
        assert!(codegen.contains(
            "modifier_order: Array.isArray(handler.modifierOrder || handler.modifier_order)"
        ));
        assert!(codegen.contains("has_modifier_object: Boolean(handler.hasModifierObject"));
        let parser = fs::read_to_string(
            temp.join("src")
                .join("compiler")
                .join("parser")
                .join("index.ts"),
        )
        .unwrap();
        assert!(parser.contains("compiled.element_public_ast"));
        assert!(parser.contains("Object.defineProperty(ast, '__vuecInternal'"));
        assert!(parser.contains("hydrateVue2PublicAst(ast, null"));
        assert!(parser.contains("normalizeVue2OptionsForBridge(options, tags, true)"));
        assert!(parser.contains("__vuecTagNamespaces"));
        assert!(parser.contains("runVue2ModuleTransforms(ast, options, 'preTransformNode')"));
        let optimizer =
            fs::read_to_string(temp.join("src").join("compiler").join("optimizer.ts")).unwrap();
        assert!(optimizer.contains("callBridge('vue2.optimize'"));
        assert!(optimizer.contains("mergeVue2OptimizedAst(ast"));
        assert!(optimizer.contains("__vuecReservedTags"));
        let codeframe =
            fs::read_to_string(temp.join("src").join("compiler").join("codeframe.ts")).unwrap();
        assert!(codeframe.contains("export { generateCodeFrame }"));

        write_vue27_sfc_conformance_shims(&temp).unwrap();
        let sfc_config = fs::read_to_string(temp.join("vitest.config.ts")).unwrap();
        assert!(sfc_config.contains("environment: 'jsdom'"));
        assert!(sfc_config.contains("include: ['packages/compiler-sfc/test/**/*.spec.ts']"));
        assert!(
            sfc_config.find("'vue/compiler-sfc'").unwrap()
                < sfc_config.find("vue: path.resolve").unwrap()
        );

        let runner = fs::read_to_string(temp.join("vuec-jasmine-runner.js")).unwrap();
        assert!(runner.contains("const Jasmine = require('jasmine')"));
        assert!(runner.contains("const { JSDOM } = require('jsdom')"));
        assert!(runner.contains("global.document = dom.window.document"));
        assert!(runner.contains("function vuecInteropDefault(value)"));
        assert!(runner.contains("globalThis.__vuecInteropDefault = vuecInteropDefault"));
        assert!(runner.contains("cache: false"));
        assert!(runner.contains("t.identifier('__vuecInteropDefault')"));
        assert!(runner.contains("testResults: Array.from(testResultsByFile.values())"));
        assert!(runner.contains("compiler-options.spec.js"));
        assert!(runner.contains("__vuecFlushProvenance"));
        assert!(runner.contains("coverageProvenance"));
        let vue2_specs = suite_spec(ConformanceSuite::Vue2Compiler);
        assert!(vue2_specs.runner_dependencies.contains(&"jsdom"));
        let setup = fs::read_to_string(temp.join("vuec-vitest-setup.ts")).unwrap();
        assert!(setup.contains("import './vuec-vitest-provenance'"));
        assert!(setup.contains("warnMock"));
        assert!(setup.contains("mock.calls"));
        assert!(setup.contains("(console.error as any).mock"));
        let provenance_setup = fs::read_to_string(temp.join("vuec-vitest-provenance.ts")).unwrap();
        assert!(provenance_setup.contains("VUEC_PROVENANCE_SIDECAR"));
        assert!(provenance_setup.contains("__vuecFlushProvenance"));
        assert!(provenance_setup.contains("afterEach"));

        let specs = suite_spec(ConformanceSuite::Vue27Compiler);
        assert!(specs.runner_dependencies.contains(&"jsdom"));
        let sfc_specs = suite_spec(ConformanceSuite::Vue27Sfc);
        assert!(sfc_specs.runner_dependencies.contains(&"jsdom"));
        let _ = fs::remove_dir_all(temp);
    }
