fn prepare_vue3_sfc_conformance_suite(
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

    let official_sfc_tests = official_root
        .join("packages")
        .join("compiler-sfc")
        .join("__tests__");
    let prepared_sfc_tests = prepared_root
        .join("packages")
        .join("compiler-sfc")
        .join("__tests__");
    copy_dir_recursive(&official_sfc_tests, &prepared_sfc_tests)?;
    rewrite_vue3_sfc_public_api_spec_imports(&prepared_root)?;

    let official_sfc_src = official_root
        .join("packages")
        .join("compiler-sfc")
        .join("src");
    let prepared_sfc_src = prepared_root
        .join("packages")
        .join("compiler-sfc")
        .join("src");
    copy_dir_recursive(&official_sfc_src, &prepared_sfc_src)?;
    patch_vue3_sfc_compile_template_asset_bridge(&prepared_sfc_src.join("compileTemplate.ts"))?;

    let official_dom_stringify = official_root
        .join("packages")
        .join("compiler-dom")
        .join("src")
        .join("transforms")
        .join("stringifyStatic.ts");
    let prepared_dom_transforms = prepared_root
        .join("packages")
        .join("compiler-dom")
        .join("src")
        .join("transforms");
    fs::create_dir_all(&prepared_dom_transforms)
        .with_context(|| format!("failed to create {}", prepared_dom_transforms.display()))?;
    fs::copy(
        &official_dom_stringify,
        prepared_dom_transforms.join("stringifyStatic.ts"),
    )
    .with_context(|| {
        format!(
            "failed to copy {} into {}",
            official_dom_stringify.display(),
            prepared_dom_transforms.display()
        )
    })?;

    write_vue3_core_source_shims(&prepared_root)?;
    write_vue3_sfc_conformance_shims(&prepared_root)?;
    write_prepared_test_manifest_for_suite(spec, &prepared_root)?;
    Ok(prepared_root)
}

fn patch_vue3_sfc_compile_template_asset_bridge(path: &Path) -> Result<()> {
    let source =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    if source.contains("transformAssetUrls:")
        && source.contains("normalizeOptions(transformAssetUrls)")
        && source.contains("(compilerOptions as any).transformAssetUrls")
    {
        return Ok(());
    }
    let needle = "    ...compilerOptions,\n    hmr: !isProd,";
    let replacement = "    ...compilerOptions,\n    transformAssetUrls:\n      isObject(transformAssetUrls)\n        ? normalizeOptions(transformAssetUrls)\n        : transformAssetUrls === false\n          ? false\n          : (compilerOptions as any).transformAssetUrls,\n    hmr: !isProd,";
    ensure!(
        source.replace("\r\n", "\n").contains(needle),
        "Vue 3 SFC compileTemplate asset bridge patch anchor not found in {}",
        path.display()
    );
    write_text(
        path,
        &source.replace("\r\n", "\n").replace(needle, replacement),
    )
}

const VUE3_SFC_ASSET_TRANSFORM_INTERNAL_HELPER: &str = r#"import {
  type TransformOptions,
  baseParse,
  generate,
  transform,
} from '@vue/compiler-core'
import {
  type AssetURLOptions,
  createAssetUrlTransformWithOptions,
  normalizeOptions,
  transformAssetUrl,
} from '../src/template/transformAssetUrl'
import { transformElement } from '../../compiler-core/src/transforms/transformElement'
import { transformBind } from '../../compiler-core/src/transforms/vBind'
import { stringifyStatic } from '../../compiler-dom/src/transforms/stringifyStatic'

function compileWithAssetUrls(
  template: string,
  options?: AssetURLOptions,
  transformOptions?: TransformOptions,
) {
  const ast = baseParse(template)
  const t = options
    ? createAssetUrlTransformWithOptions(normalizeOptions(options))
    : transformAssetUrl
  transform(ast, {
    nodeTransforms: [t, transformElement],
    directiveTransforms: {
      bind: transformBind,
    },
    ...transformOptions,
  })
  return generate(ast, { mode: 'module' })
}
"#;

const VUE3_SFC_SRCSET_TRANSFORM_INTERNAL_HELPER: &str = r#"import {
  type TransformOptions,
  baseParse,
  generate,
  transform,
} from '@vue/compiler-core'
import {
  createSrcsetTransformWithOptions,
  transformSrcset,
} from '../src/template/transformSrcset'
import { transformElement } from '../../compiler-core/src/transforms/transformElement'
import { transformBind } from '../../compiler-core/src/transforms/vBind'
import {
  type AssetURLOptions,
  normalizeOptions,
} from '../src/template/transformAssetUrl'
import { stringifyStatic } from '../../compiler-dom/src/transforms/stringifyStatic'

function compileWithSrcset(
  template: string,
  options?: AssetURLOptions,
  transformOptions?: TransformOptions,
) {
  const ast = baseParse(template)
  const srcsetTransform = options
    ? createSrcsetTransformWithOptions(normalizeOptions(options))
    : transformSrcset
  transform(ast, {
    hoistStatic: true,
    nodeTransforms: [srcsetTransform, transformElement],
    directiveTransforms: {
      bind: transformBind,
    },
    ...transformOptions,
  })
  return generate(ast, { mode: 'module' })
}
"#;

const VUE3_SFC_ASSET_TRANSFORM_PUBLIC_HELPER_IMPORT: &str = r#"import {
  type AssetURLOptions,
  type TransformOptions,
  compileWithAssetUrls,
  stringifyStatic,
} from './templateTransforms.public-api'
"#;

const VUE3_SFC_SRCSET_TRANSFORM_PUBLIC_HELPER_IMPORT: &str = r#"import {
  type AssetURLOptions,
  type TransformOptions,
  compileWithSrcset,
  stringifyStatic,
} from './templateTransforms.public-api'
"#;

const VUE3_SFC_TEMPLATE_TRANSFORMS_PUBLIC_API_HELPER: &str = r#"import { compileTemplate } from '@vue/compiler-sfc'

export interface AssetURLOptions {
  base?: string | null
  includeAbsolute?: boolean
  tags?: Record<string, string[]>
}

export interface TransformOptions {
  hoistStatic?: boolean
  transformHoist?: unknown
}

export function stringifyStatic(): void {}

function compilePublic(
  template: string,
  transformAssetUrls: AssetURLOptions | undefined,
  transformOptions: TransformOptions | undefined,
  defaultHoistStatic: boolean,
) {
  const compilerOptions: Record<string, unknown> = {
    hoistStatic:
      transformOptions && transformOptions.hoistStatic !== undefined
        ? transformOptions.hoistStatic
        : defaultHoistStatic,
  }
  if (transformOptions && transformOptions.transformHoist !== undefined) {
    compilerOptions.transformHoist = transformOptions.transformHoist
  }
  return compileTemplate({
    source: template,
    filename: 'template.vue',
    id: 'data-v-template-transform',
    transformAssetUrls,
    compilerOptions,
  } as any)
}

export function compileWithAssetUrls(
  template: string,
  options?: AssetURLOptions,
  transformOptions?: TransformOptions,
) {
  return compilePublic(template, options, transformOptions, false)
}

export function compileWithSrcset(
  template: string,
  options?: AssetURLOptions,
  transformOptions?: TransformOptions,
) {
  return compilePublic(
    template,
    {
      ...(options || {}),
      tags: {
        img: [],
        source: [],
      },
    },
    transformOptions,
    true,
  )
}
"#;

const VUE3_SFC_RESOLVE_TYPE_INTERNAL_IMPORTS: &str = r#"import { normalize } from 'node:path'
import type { Identifier } from '@babel/types'
import { type SFCScriptCompileOptions, parse } from '../../src'
import { ScriptCompileContext } from '../../src/script/context'
import {
  inferRuntimeType,
  invalidateTypeCache,
  recordImports,
  registerTS,
  resolveTypeElements,
} from '../../src/script/resolveType'
import { UNKNOWN_TYPE } from '../../src/script/utils'
import ts from 'typescript'

registerTS(() => ts)
"#;

const VUE3_SFC_RESOLVE_TYPE_PUBLIC_IMPORTS: &str = r#"import { normalize } from 'node:path'
import { resolve, UNKNOWN_TYPE } from './resolveType.rust-api'
"#;

const VUE3_SFC_RESOLVE_TYPE_INTERNAL_HELPER: &str = r#"function resolve(
  code: string,
  files: Record<string, string> = {},
  options?: Partial<SFCScriptCompileOptions>,
  sourceFileName: string = '/Test.vue',
  invalidateCache = true,
) {
  const { descriptor } = parse(`<script setup lang="ts">\n${code}\n</script>`, {
    filename: sourceFileName,
  })
  const ctx = new ScriptCompileContext(descriptor, {
    id: 'test',
    fs: {
      fileExists(file) {
        return !!(files[file] ?? files[normalize(file)])
      },
      readFile(file) {
        return files[file] ?? files[normalize(file)]
      },
    },
    ...options,
  })

  if (invalidateCache) {
    for (const file in files) {
      invalidateTypeCache(file)
    }
  }

  // ctx.userImports is collected when calling compileScript(), but we are
  // skipping that here, so need to manually register imports
  ctx.userImports = recordImports(ctx.scriptSetupAst!.body) as any

  let target: any
  for (const s of ctx.scriptSetupAst!.body) {
    if (
      s.type === 'ExpressionStatement' &&
      s.expression.type === 'CallExpression' &&
      (s.expression.callee as Identifier).name === 'defineProps'
    ) {
      target = s.expression.typeParameters!.params[0]
    }
  }
  const raw = resolveTypeElements(ctx, target)
  const props: Record<string, string[]> = {}
  for (const key in raw.props) {
    props[key] = inferRuntimeType(ctx, raw.props[key])
  }
  return {
    props,
    calls: raw.calls,
    deps: ctx.deps,
    raw,
  }
}
"#;

const VUE3_SFC_RESOLVE_TYPE_RUST_API_HELPER: &str = r#"import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { __vuecRuntime } from '@vue/compiler-sfc'

export const UNKNOWN_TYPE = 'Unknown'

type ResolveOptions = {
  globalTypeFiles?: string[]
  [key: string]: unknown
}

type MaterializedFiles = {
  root: string
  virtualToReal: Map<string, string>
  realToVirtual: Map<string, string>
}

function normalizeReal(file: string): string {
  return path.resolve(file).replace(/\\/g, '/')
}

function normalizeVirtual(file: string): string {
  return file.replace(/\\/g, '/')
}

function virtualRelativePath(file: string): string {
  const normalized = normalizeVirtual(file)
  const drive = /^([A-Za-z]):\/(.*)$/.exec(normalized)
  if (drive) {
    return path.join(`__drive_${drive[1].toUpperCase()}`, ...drive[2].split('/').filter(Boolean))
  }
  return path.join(...normalized.replace(/^\/+/, '').split('/').filter(Boolean))
}

function materializedPath(root: string, file: string): string {
  const relative = virtualRelativePath(file)
  return relative ? path.join(root, relative) : root
}

function materializeFiles(files: Record<string, string>): MaterializedFiles {
  const root = mkdtempSync(path.join(tmpdir(), 'vuec-resolve-type-'))
  const virtualToReal = new Map<string, string>()
  const realToVirtual = new Map<string, string>()
  for (const [virtual, source] of Object.entries(files)) {
    const real = materializedPath(root, virtual)
    mkdirSync(path.dirname(real), { recursive: true })
    writeFileSync(real, source)
    virtualToReal.set(virtual, real)
    virtualToReal.set(normalizeVirtual(virtual), real)
    realToVirtual.set(normalizeReal(real), virtual)
  }
  return { root, virtualToReal, realToVirtual }
}

function mapVirtualToReal(materialized: MaterializedFiles, file: string): string {
  return materialized.virtualToReal.get(file)
    || materialized.virtualToReal.get(normalizeVirtual(file))
    || materializedPath(materialized.root, file)
}

function mapOptions(options: ResolveOptions | undefined, materialized: MaterializedFiles): ResolveOptions | undefined {
  if (!options) {
    return options
  }
  const mapped: ResolveOptions = { ...options }
  if (Array.isArray(options.globalTypeFiles)) {
    mapped.globalTypeFiles = options.globalTypeFiles.map(file => mapVirtualToReal(materialized, file))
  }
  return mapped
}

function mapDep(dep: string, materialized: MaterializedFiles): string {
  const direct = materialized.realToVirtual.get(normalizeReal(dep))
  if (direct) {
    return direct
  }
  const relative = path.relative(materialized.root, dep).replace(/\\/g, '/')
  if (!relative.startsWith('..') && !path.isAbsolute(relative)) {
    const drive = /^__drive_([A-Z])\/(.*)$/.exec(relative)
    return drive ? `${drive[1]}:/${drive[2]}` : `/${relative}`
  }
  return dep
}

function mapDeps(deps: string[] | undefined, materialized: MaterializedFiles): string[] {
  const mapped: string[] = []
  const seen = new Set<string>()
  for (const dep of deps || []) {
    const virtual = mapDep(dep, materialized)
    if (!seen.has(virtual)) {
      seen.add(virtual)
      mapped.push(virtual)
    }
  }
  return mapped
}

export function resolve(
  code: string,
  files: Record<string, string> = {},
  options?: ResolveOptions,
  sourceFileName: string = '/Test.vue',
  _invalidateCache = true,
) {
  const materialized = materializeFiles(files)
  try {
    const filename = mapVirtualToReal(materialized, sourceFileName)
    const parent = path.dirname(filename)
    if (!existsSync(parent)) {
      mkdirSync(parent, { recursive: true })
    }
    const result = __vuecRuntime.callBridge('sfc.resolveType', {
      code,
      filename,
      options: mapOptions(options, materialized),
    })
    if (Array.isArray(result.errors) && result.errors.length > 0) {
      throw new Error(String(result.errors[0]))
    }
    const calls = Array.isArray(result.calls) ? result.calls : []
    const raw = result.raw || {}
    return {
      props: result.props || {},
      calls,
      deps: mapDeps(result.deps, materialized),
      raw: {
        ...raw,
        props: raw.props || {},
        calls: Array.isArray(raw.calls) ? raw.calls : calls,
      },
    }
  } finally {
    rmSync(materialized.root, { recursive: true, force: true })
  }
}
"#;
