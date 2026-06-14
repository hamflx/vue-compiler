fn rewrite_vue3_sfc_public_api_spec_imports(prepared_root: &Path) -> Result<()> {
    let tests = prepared_root
        .join("packages")
        .join("compiler-sfc")
        .join("__tests__");
    let parse_spec = prepared_root
        .join("packages")
        .join("compiler-sfc")
        .join("__tests__")
        .join("parse.spec.ts");
    rewrite_text_file_import(
        &parse_spec,
        "import { parse } from '../src'",
        "import { parse } from '@vue/compiler-sfc'",
    )?;

    let rewrite_default_spec = prepared_root
        .join("packages")
        .join("compiler-sfc")
        .join("__tests__")
        .join("rewriteDefault.spec.ts");
    rewrite_text_file_import(
        &rewrite_default_spec,
        "import { rewriteDefault } from '../src'",
        "import { rewriteDefault } from '@vue/compiler-sfc'",
    )?;

    let compile_style_spec = prepared_root
        .join("packages")
        .join("compiler-sfc")
        .join("__tests__")
        .join("compileStyle.spec.ts");
    rewrite_text_file_import(
        &compile_style_spec,
        "from '../src/compileStyle'",
        "from '@vue/compiler-sfc'",
    )?;

    let compile_template_spec = tests.join("compileTemplate.spec.ts");
    rewrite_text_file_import(
        &compile_template_spec,
        "from '../src/compileTemplate'",
        "from '@vue/compiler-sfc'",
    )?;
    rewrite_text_file_import(
        &compile_template_spec,
        "from '../src/parse'",
        "from '@vue/compiler-sfc'",
    )?;
    rewrite_text_file_import(
        &compile_template_spec,
        "import { compileScript } from '../src'",
        "import { compileScript } from '@vue/compiler-sfc'",
    )?;
    rewrite_text_file_import(
        &compile_template_spec,
        "from './utils'",
        "from './utils.public-api'",
    )?;

    let css_vars_spec = tests.join("cssVars.spec.ts");
    rewrite_text_file_import(
        &css_vars_spec,
        "import { compileStyle, parse } from '../src'",
        "import { compileStyle, parse } from '@vue/compiler-sfc'",
    )?;
    rewrite_text_file_import(
        &css_vars_spec,
        "from './utils'",
        "from './utils.public-api'",
    )?;
    let compile_script_spec = tests.join("compileScript.spec.ts");
    rewrite_text_file_import(
        &compile_script_spec,
        "from './utils'",
        "from './utils.public-api'",
    )?;
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
        rewrite_text_file_import(
            &tests.join("compileScript").join(compile_script_spec),
            "from '../utils'",
            "from '../utils.public-api'",
        )?;
    }
    rewrite_vue3_sfc_resolve_type_public_api_spec(&tests)?;
    let template_utils_spec = tests.join("templateUtils.spec.ts");
    rewrite_text_file_import(
        &template_utils_spec,
        "from '../src/template/templateUtils'",
        "from './templateUtils.rust-api'",
    )?;
    if template_utils_spec.exists() {
        write_text(
            &tests.join("templateUtils.rust-api.ts"),
            r#"import { __vuecRuntime } from '@vue/compiler-sfc'

function callTemplateUtils(command: string, url: string): boolean {
  return __vuecRuntime.callBridge(command, { url }) === true
}

export function isRelativeUrl(url: string): boolean {
  return callTemplateUtils('sfc.templateUtils.isRelativeUrl', url)
}

export function isExternalUrl(url: string): boolean {
  return callTemplateUtils('sfc.templateUtils.isExternalUrl', url)
}

export function isDataUrl(url: string): boolean {
  return callTemplateUtils('sfc.templateUtils.isDataUrl', url)
}
"#,
        )?;
    }
    rewrite_vue3_sfc_template_transform_public_api_specs(&tests)?;
    if css_vars_spec.exists() || compile_template_spec.exists() || compile_script_spec.exists() {
        write_text(
            &tests.join("utils.public-api.ts"),
            r#"import {
  type SFCParseOptions,
  type SFCScriptBlock,
  type SFCScriptCompileOptions,
  compileScript,
  parse,
} from '@vue/compiler-sfc'
import { parse as babelParse } from '@babel/parser'
import { warnOnce } from '../src/warn'

export const mockId = 'xxxxxxxx'

export function compileSFCScript(
  src: string,
  options?: Partial<SFCScriptCompileOptions>,
  parseOptions?: SFCParseOptions,
): SFCScriptBlock {
  const { descriptor, errors } = parse(src, parseOptions)
  if (errors.length) {
    console.warn(errors[0])
  }
  const warnings: string[] = []
  const originalWarn = console.warn
  console.warn = (...args: unknown[]) => {
    const message = args.map(arg => String(arg)).join(' ')
    warnings.push(message)
    originalWarn(...args)
  }
  try {
    return compileScript(descriptor, {
      __vuecEmitScriptSetupMarker: false,
      ...options,
      id: mockId,
    } as any)
  } finally {
    console.warn = originalWarn
    for (const warning of warnings) {
      warnOnce(warning)
    }
  }
}

export function assertCode(code: string): void {
  try {
    babelParse(code, {
      sourceType: 'module',
      plugins: [
        'typescript',
        ['importAttributes', { deprecatedAssertSyntax: true }],
      ],
    })
  } catch (e: any) {
    console.log(code)
    throw e
  }
  expect(code).toMatchSnapshot()
}

interface Pos {
  line: number
  column: number
  name?: string
}

export function getPositionInCode(
  code: string,
  token: string,
  expectName: string | boolean = false,
): Pos {
  const generatedOffset = code.indexOf(token)
  let line = 1
  let lastNewLinePos = -1
  for (let i = 0; i < generatedOffset; i++) {
    if (code.charCodeAt(i) === 10) {
      line++
      lastNewLinePos = i
    }
  }
  const res: Pos = {
    line,
    column:
      lastNewLinePos === -1
        ? generatedOffset
        : generatedOffset - lastNewLinePos - 1,
  }
  if (expectName) {
    res.name = typeof expectName === 'string' ? expectName : token
  }
  return res
}
"#,
        )?;
    }
    Ok(())
}

fn rewrite_vue3_sfc_resolve_type_public_api_spec(tests: &Path) -> Result<()> {
    let resolve_type_spec = tests.join("compileScript").join("resolveType.spec.ts");
    rewrite_text_file_block(
        &resolve_type_spec,
        VUE3_SFC_RESOLVE_TYPE_INTERNAL_IMPORTS,
        VUE3_SFC_RESOLVE_TYPE_PUBLIC_IMPORTS,
        "Vue 3 SFC resolveType public Rust helper imports",
    )?;
    rewrite_text_file_block(
        &resolve_type_spec,
        VUE3_SFC_RESOLVE_TYPE_INTERNAL_HELPER,
        "",
        "Vue 3 SFC resolveType public Rust helper body",
    )?;
    if resolve_type_spec.exists() {
        write_text(
            &tests.join("compileScript").join("resolveType.rust-api.ts"),
            VUE3_SFC_RESOLVE_TYPE_RUST_API_HELPER,
        )?;
    }
    Ok(())
}

fn rewrite_vue3_sfc_template_transform_public_api_specs(tests: &Path) -> Result<()> {
    let asset_transform_spec = tests.join("templateTransformAssetUrl.spec.ts");
    rewrite_text_file_block(
        &asset_transform_spec,
        VUE3_SFC_ASSET_TRANSFORM_INTERNAL_HELPER,
        VUE3_SFC_ASSET_TRANSFORM_PUBLIC_HELPER_IMPORT,
        "Vue 3 SFC template asset transform public helper",
    )?;

    let srcset_transform_spec = tests.join("templateTransformSrcset.spec.ts");
    rewrite_text_file_block(
        &srcset_transform_spec,
        VUE3_SFC_SRCSET_TRANSFORM_INTERNAL_HELPER,
        VUE3_SFC_SRCSET_TRANSFORM_PUBLIC_HELPER_IMPORT,
        "Vue 3 SFC template srcset transform public helper",
    )?;

    if asset_transform_spec.exists() || srcset_transform_spec.exists() {
        write_text(
            &tests.join("templateTransforms.public-api.ts"),
            VUE3_SFC_TEMPLATE_TRANSFORMS_PUBLIC_API_HELPER,
        )?;
    }
    Ok(())
}

fn rewrite_text_file_import(path: &Path, from: &str, to: &str) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let original =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let rewritten = original.replace(from, to);
    if rewritten != original {
        write_text(path, &rewritten)?;
    }
    Ok(())
}

fn rewrite_text_file_block(path: &Path, from: &str, to: &str, label: &str) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let original =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let normalized = original.replace("\r\n", "\n");
    if normalized.contains(from) {
        return write_text(path, &normalized.replace(from, to));
    }
    ensure!(
        normalized.contains(to),
        "{} anchor not found in {}",
        label,
        path.display()
    );
    Ok(())
}

fn remove_snapshot_entries_by_key_prefix(path: &Path, prefix: &str) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let original =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let normalized = original.replace("\r\n", "\n");
    let marker = "exports[`";
    let mut output = String::new();
    let mut cursor = 0;
    let mut removed = 0usize;
    let mut kept = 0usize;

    while let Some(relative_start) = normalized[cursor..].find(marker) {
        let start = cursor + relative_start;
        output.push_str(&normalized[cursor..start]);
        let next_start = normalized[start + marker.len()..]
            .find(marker)
            .map(|relative| start + marker.len() + relative)
            .unwrap_or(normalized.len());
        let block = &normalized[start..next_start];
        let key_start = marker.len();
        let key_end = block[key_start..]
            .find("`] = `")
            .map(|index| key_start + index);
        match key_end.map(|index| &block[key_start..index]) {
            Some(key) if key.starts_with(prefix) => {
                removed += 1;
            }
            _ => {
                kept += 1;
                output.push_str(block);
            }
        }
        cursor = next_start;
    }
    output.push_str(&normalized[cursor..]);

    if removed == 0 {
        return Ok(());
    }
    if kept == 0 {
        fs::remove_file(path).with_context(|| format!("failed to remove {}", path.display()))?;
    } else {
        write_text(path, &output)?;
    }
    Ok(())
}

fn write_vue3_sfc_conformance_shims(prepared_root: &Path) -> Result<()> {
    write_json(
        &prepared_root.join("package.json"),
        &serde_json::json!({
            "private": true,
            "type": "module",
        }),
    )?;
    write_vue3_core_test_setup(prepared_root)?;

    let config = r#"
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const root = path.dirname(fileURLToPath(import.meta.url))
const aliasRoot = process.env.VUEC_RUST_ALIAS_ROOT
const npmRoot = process.env.VUEC_OFFICIAL_NPM_ROOT

export default {
  oxc: {
    target: 'es2020',
  },
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
      '@vue/compiler-ssr': path.resolve(aliasRoot, 'node_modules/@vue/compiler-ssr/dist/compiler-ssr.cjs.js'),
      '@vue/compiler-sfc': path.resolve(aliasRoot, 'node_modules/@vue/compiler-sfc/dist/compiler-sfc.cjs.js'),
      '@vue/shared': path.resolve(npmRoot, 'node_modules/@vue/shared/index.js'),
      '@babel/parser': path.resolve(npmRoot, 'node_modules/@babel/parser/lib/index.js'),
      '@babel/types': path.resolve(npmRoot, 'node_modules/@babel/types/lib/index.js'),
      '@vue/consolidate': path.resolve(npmRoot, 'node_modules/@vue/consolidate/index.js'),
      'estree-walker': path.resolve(npmRoot, 'node_modules/estree-walker/dist/esm/estree-walker.js'),
      'hash-sum': path.resolve(npmRoot, 'node_modules/hash-sum/hash-sum.js'),
      'lru-cache': path.resolve(npmRoot, 'node_modules/lru-cache/dist/esm/index.js'),
      'magic-string': path.resolve(npmRoot, 'node_modules/magic-string/dist/magic-string.es.mjs'),
      'merge-source-map': path.resolve(npmRoot, 'node_modules/merge-source-map/index.js'),
      'minimatch': path.resolve(npmRoot, 'node_modules/minimatch/dist/esm/index.js'),
      'postcss': path.resolve(npmRoot, 'node_modules/postcss/lib/postcss.mjs'),
      'postcss-modules': path.resolve(npmRoot, 'node_modules/postcss-modules/build/index.js'),
      'postcss-selector-parser': path.resolve(npmRoot, 'node_modules/postcss-selector-parser/dist/index.js'),
      'pug': path.resolve(npmRoot, 'node_modules/pug/lib/index.js'),
      'sass': path.resolve(npmRoot, 'node_modules/sass/sass.node.mjs'),
      'source-map-js': path.resolve(npmRoot, 'node_modules/source-map-js/source-map.js'),
      'typescript': path.resolve(npmRoot, 'node_modules/typescript/lib/typescript.js'),
    },
  },
  test: {
    globals: true,
    pool: 'forks',
    fileParallelism: false,
    maxWorkers: 1,
    setupFiles: ['./vuec-vitest-setup.ts'],
    include: ['packages/compiler-sfc/__tests__/**/*.spec.ts'],
  },
}
"#;
    write_text(&prepared_root.join("vitest.config.ts"), config)?;
    Ok(())
}
