fn prepare_vue3_dom_conformance_suite(
    spec: ConformanceSuiteSpec,
    official_root: &Path,
    lock_hash: Option<&str>,
) -> Result<PathBuf> {
    let prepared_root = PathBuf::from("target")
        .join("conformance")
        .join(lock_hash.unwrap_or("unknown-lock"))
        .join("prepared")
        .join(spec.name);
    if prepared_root.exists() {
        fs::remove_dir_all(&prepared_root)
            .with_context(|| format!("failed to remove {}", prepared_root.display()))?;
    }
    let official_tests = official_root
        .join("packages")
        .join("compiler-dom")
        .join("__tests__");
    let prepared_tests = prepared_root
        .join("packages")
        .join("compiler-dom")
        .join("__tests__");
    copy_dir_recursive(&official_tests, &prepared_tests)?;
    let official_src = official_root
        .join("packages")
        .join("compiler-dom")
        .join("src");
    let prepared_src = prepared_root
        .join("packages")
        .join("compiler-dom")
        .join("src");
    copy_dir_recursive(&official_src, &prepared_src)?;

    let core_test_utils = official_root
        .join("packages")
        .join("compiler-core")
        .join("__tests__")
        .join("testUtils.ts");
    let prepared_core_tests = prepared_root
        .join("packages")
        .join("compiler-core")
        .join("__tests__");
    fs::create_dir_all(&prepared_core_tests)
        .with_context(|| format!("failed to create {}", prepared_core_tests.display()))?;
    fs::copy(&core_test_utils, prepared_core_tests.join("testUtils.ts")).with_context(|| {
        format!(
            "failed to copy {} into {}",
            core_test_utils.display(),
            prepared_core_tests.display()
        )
    })?;

    write_vue3_core_source_shims(&prepared_root)?;
    write_vue3_dom_conformance_shims(&prepared_root)?;
    write_prepared_test_manifest_for_suite(spec, &prepared_root)?;
    Ok(prepared_root)
}

fn write_vue3_dom_conformance_shims(prepared_root: &Path) -> Result<()> {
    rewrite_vue3_dom_public_index_spec_import(prepared_root)?;

    let dom_src = prepared_root
        .join("packages")
        .join("compiler-dom")
        .join("src");
    let transforms = dom_src.join("transforms");
    fs::create_dir_all(&transforms)
        .with_context(|| format!("failed to create {}", transforms.display()))?;

    write_text(
        &transforms.join("transformStyle.ts"),
        "export { transformStyle } from '@vue/compiler-dom'\n",
    )?;
    write_vue3_dom_stringify_static_shim(&transforms.join("stringifyStatic.ts"))?;
    write_vue3_dom_transform_shim(&transforms.join("vHtml.ts"), "transformVHtml")?;
    write_vue3_dom_transform_shim(&transforms.join("vText.ts"), "transformVText")?;
    write_vue3_dom_transform_shim(&transforms.join("vShow.ts"), "transformShow")?;
    write_vue3_dom_v_on_transform_shim(&transforms.join("vOn.ts"))?;
    write_vue3_dom_v_model_transform_shim(&transforms.join("vModel.ts"))?;
    write_vue3_dom_transition_transform_shim(&transforms.join("Transition.ts"))?;
    write_vue3_dom_ignore_side_effect_tags_shim(&transforms.join("ignoreSideEffectTags.ts"))?;
    write_vue3_dom_validate_html_nesting_shim(&transforms.join("validateHtmlNesting.ts"))?;
    write_vue3_dom_decode_html_browser_shim(&dom_src.join("decodeHtmlBrowser.ts"))?;
    write_vue3_dom_html_nesting_shim(&dom_src.join("htmlNesting.ts"))?;
    write_vue3_core_test_setup(prepared_root)?;

    let config = r#"
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const root = path.dirname(fileURLToPath(import.meta.url))
const aliasRoot = process.env.VUEC_RUST_ALIAS_ROOT
const npmRoot = process.env.VUEC_OFFICIAL_NPM_ROOT

export default {
  define: {
    __DEV__: true,
    __TEST__: true,
    __VERSION__: '"test"',
    __BROWSER__: false,
    __GLOBAL__: false,
    __ESM_BUNDLER__: true,
    __ESM_BROWSER__: false,
    __CJS__: true,
    __SSR__: true,
    __FEATURE_OPTIONS_API__: true,
    __FEATURE_SUSPENSE__: true,
    __FEATURE_PROD_DEVTOOLS__: false,
    __FEATURE_PROD_HYDRATION_MISMATCH_DETAILS__: false,
    __COMPAT__: true,
  },
  resolve: {
    alias: {
      '@vue/compiler-core': path.resolve(aliasRoot, 'node_modules/@vue/compiler-core/index.js'),
      '@vue/compiler-dom': path.resolve(aliasRoot, 'node_modules/@vue/compiler-dom/index.js'),
      '@vue/compiler-sfc': path.resolve(aliasRoot, 'node_modules/@vue/compiler-sfc/dist/compiler-sfc.cjs.js'),
      '@vue/shared': path.resolve(npmRoot, 'node_modules/@vue/shared/index.js'),
      'source-map-js': path.resolve(npmRoot, 'node_modules/source-map-js/source-map.js'),
    },
  },
  test: {
    globals: true,
    pool: 'forks',
    setupFiles: ['./vuec-vitest-setup.ts'],
    include: ['packages/compiler-dom/__tests__/**/*.spec.ts'],
  },
}
"#;
    write_text(&prepared_root.join("vitest.config.ts"), config)?;
    Ok(())
}

fn rewrite_vue3_dom_public_index_spec_import(prepared_root: &Path) -> Result<()> {
    let index_spec = prepared_root
        .join("packages")
        .join("compiler-dom")
        .join("__tests__")
        .join("index.spec.ts");
    if !index_spec.exists() {
        return Ok(());
    }
    let original = fs::read_to_string(&index_spec)
        .with_context(|| format!("failed to read {}", index_spec.display()))?;
    let rewritten = original.replace(
        "import { compile } from '../src'",
        "import { compile } from '@vue/compiler-dom'",
    );
    if rewritten != original {
        write_text(&index_spec, &rewritten)?;
    }
    Ok(())
}
