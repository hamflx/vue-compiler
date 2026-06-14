fn prepare_vue3_ssr_conformance_suite(
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

    let official_ssr_tests = official_root
        .join("packages")
        .join("compiler-ssr")
        .join("__tests__");
    let prepared_ssr_tests = prepared_root
        .join("packages")
        .join("compiler-ssr")
        .join("__tests__");
    copy_dir_recursive(&official_ssr_tests, &prepared_ssr_tests)?;

    let official_ssr_src = official_root
        .join("packages")
        .join("compiler-ssr")
        .join("src");
    let prepared_ssr_src = prepared_root
        .join("packages")
        .join("compiler-ssr")
        .join("src");
    copy_dir_recursive(&official_ssr_src, &prepared_ssr_src)?;

    write_vue3_core_source_shims(&prepared_root)?;
    let official_dom_src = official_root
        .join("packages")
        .join("compiler-dom")
        .join("src");
    let prepared_dom_src = prepared_root
        .join("packages")
        .join("compiler-dom")
        .join("src");
    copy_dir_recursive(&official_dom_src, &prepared_dom_src)?;
    rewrite_vue3_ssr_rust_backed_public_compile_imports(&prepared_root)?;
    write_vue3_ssr_conformance_shims(&prepared_root)?;
    write_prepared_test_manifest_for_suite(spec, &prepared_root)?;
    Ok(prepared_root)
}

fn rewrite_vue3_ssr_rust_backed_public_compile_imports(prepared_root: &Path) -> Result<()> {
    let tests = prepared_root
        .join("packages")
        .join("compiler-ssr")
        .join("__tests__");
    let ssr_text = tests.join("ssrText.spec.ts");
    if ssr_text.exists() {
        let original = fs::read_to_string(&ssr_text)
            .with_context(|| format!("failed to read {}", ssr_text.display()))?;
        let rewritten = original
            .replace(
                "import { compile } from '../src'",
                "import { compile } from '@vue/compiler-ssr'",
            )
            .replace(
                "import { getCompiledString } from './utils'",
                "import { getCompiledString } from './utils.rust-ssr-text'",
            );
        if rewritten != original {
            write_text(&ssr_text, &rewritten)?;
        }
    }

    rewrite_vue3_ssr_spec_compile_import(&tests.join("ssrVIf.spec.ts"))?;
    rewrite_vue3_ssr_spec_compile_import(&tests.join("ssrVFor.spec.ts"))?;
    rewrite_vue3_ssr_spec_compile_import(&tests.join("ssrScopeId.spec.ts"))?;
    rewrite_vue3_ssr_spec_compile_import(&tests.join("ssrFallthroughAttrs.spec.ts"))?;
    rewrite_vue3_ssr_spec_compile_import(&tests.join("ssrInjectCssVars.spec.ts"))?;
    rewrite_vue3_ssr_spec_compile_import(&tests.join("ssrVShow.spec.ts"))?;
    rewrite_vue3_ssr_spec_compile_import(&tests.join("ssrVModel.spec.ts"))?;
    rewrite_vue3_ssr_element_public_compile_imports(&tests.join("ssrElement.spec.ts"))?;
    rewrite_vue3_ssr_spec_compile_import(&tests.join("ssrSlotOutlet.spec.ts"))?;
    rewrite_vue3_ssr_spec_compile_import(&tests.join("ssrPortal.spec.ts"))?;
    rewrite_vue3_ssr_spec_compile_import(&tests.join("ssrSuspense.spec.ts"))?;
    rewrite_vue3_ssr_spec_compile_import(&tests.join("ssrTransition.spec.ts"))?;
    rewrite_vue3_ssr_spec_compile_import(&tests.join("ssrTransitionGroup.spec.ts"))?;
    rewrite_vue3_ssr_spec_compile_import(&tests.join("ssrComponent.spec.ts"))?;

    let utils = tests.join("utils.ts");
    if utils.exists() {
        let original = fs::read_to_string(&utils)
            .with_context(|| format!("failed to read {}", utils.display()))?;
        let rewritten = original.replace(
            "import { compile } from '../src'",
            "import { compile } from '@vue/compiler-ssr'",
        );
        write_text(&tests.join("utils.rust-ssr-text.ts"), &rewritten)?;
    }
    Ok(())
}

fn rewrite_vue3_ssr_element_public_compile_imports(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let original =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let rewritten = original
        .replace(
            "import { compile } from '../src'",
            "import { compile } from '@vue/compiler-ssr'",
        )
        .replace(
            "import { getCompiledString } from './utils'",
            "import { getCompiledString } from './utils.rust-ssr-text'",
        );
    if rewritten != original {
        write_text(path, &rewritten)?;
    }
    Ok(())
}

fn rewrite_vue3_ssr_spec_compile_import(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let original =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let rewritten = original.replace(
        "import { compile } from '../src'",
        "import { compile } from '@vue/compiler-ssr'",
    );
    if rewritten != original {
        write_text(path, &rewritten)?;
    }
    Ok(())
}

fn write_vue3_ssr_conformance_shims(prepared_root: &Path) -> Result<()> {
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
      '@vue/compiler-dom': path.resolve(root, 'packages/compiler-dom/src/index.ts'),
      '@vue/compiler-ssr': path.resolve(aliasRoot, 'node_modules/@vue/compiler-ssr/dist/compiler-ssr.cjs.js'),
      '@vue/compiler-sfc': path.resolve(aliasRoot, 'node_modules/@vue/compiler-sfc/dist/compiler-sfc.cjs.js'),
      '@vue/shared': path.resolve(npmRoot, 'node_modules/@vue/shared/index.js'),
      'packages/compiler-core/src/transform': path.resolve(root, 'packages/compiler-core/src/transform.ts'),
      'source-map-js': path.resolve(npmRoot, 'node_modules/source-map-js/source-map.js'),
    },
  },
  test: {
    globals: true,
    pool: 'forks',
    setupFiles: ['./vuec-vitest-setup.ts'],
    include: ['packages/compiler-ssr/__tests__/**/*.spec.ts'],
  },
}
"#;
    write_text(&prepared_root.join("vitest.config.ts"), config)?;
    Ok(())
}
