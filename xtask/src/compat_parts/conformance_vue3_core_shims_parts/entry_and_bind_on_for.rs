fn write_vue3_core_conformance_shims(prepared_root: &Path) -> Result<()> {
    write_vue3_core_source_shims(prepared_root)?;
    write_vue3_core_test_setup(prepared_root)?;
    rewrite_vue3_core_v_bind_public_api_spec(prepared_root)?;
    rewrite_vue3_core_v_model_public_api_spec(prepared_root)?;
    rewrite_vue3_core_v_on_public_api_spec(prepared_root)?;
    rewrite_vue3_core_v_for_public_api_spec(prepared_root)?;
    rewrite_vue3_core_transform_element_public_api_spec(prepared_root)?;
    rewrite_vue3_core_noop_directive_transform_public_api_spec(prepared_root)?;
    rewrite_vue3_core_transform_public_api_spec(prepared_root)?;
    rewrite_vue3_core_v_if_public_api_spec(prepared_root)?;
    rewrite_vue3_core_transform_slot_outlet_public_api_spec(prepared_root)?;
    rewrite_vue3_core_v_slot_public_api_spec(prepared_root)?;
    rewrite_vue3_core_cache_static_public_api_spec(prepared_root)?;
    rewrite_vue3_core_transform_expressions_public_api_spec(prepared_root)?;
    rewrite_vue3_core_transform_text_public_api_spec(prepared_root)?;
    rewrite_vue3_core_v_once_public_api_spec(prepared_root)?;

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
    include: ['packages/compiler-core/__tests__/**/*.spec.ts'],
  },
}
"#;
    write_text(&prepared_root.join("vitest.config.ts"), config)?;
    Ok(())
}

fn rewrite_vue3_core_v_bind_public_api_spec(prepared_root: &Path) -> Result<()> {
    let transforms = prepared_root
        .join("packages")
        .join("compiler-core")
        .join("__tests__")
        .join("transforms");
    let spec = transforms.join("vBind.spec.ts");
    if !spec.exists() {
        return Ok(());
    }
    rewrite_text_file_block(
        &spec,
        r#"import {
  type CallExpression,
  type CompilerOptions,
  type ElementNode,
  ErrorCodes,
  NodeTypes,
  type ObjectExpression,
  type VNodeCall,
  baseParse as parse,
  transform,
} from '../../src'
import { transformBind } from '../../src/transforms/vBind'
import { transformElement } from '../../src/transforms/transformElement'
import {
  CAMELIZE,
  NORMALIZE_PROPS,
  helperNameMap,
} from '../../src/runtimeHelpers'
import { transformExpression } from '../../src/transforms/transformExpression'
import { transformVBindShorthand } from '../../src/transforms/transformVBindShorthand'"#,
        r#"import {
  type CallExpression,
  type CompilerOptions,
  type ElementNode,
  ErrorCodes,
  NodeTypes,
  type ObjectExpression,
  type VNodeCall,
} from '../../src'
import {
  CAMELIZE,
  NORMALIZE_PROPS,
  helperNameMap,
} from '../../src/runtimeHelpers'
import { parseWithVBind } from './vBind.rust-api'"#,
        "Vue 3 core vBind Rust API imports",
    )?;
    rewrite_text_file_block(
        &spec,
        r#"function parseWithVBind(
  template: string,
  options: CompilerOptions = {},
): ElementNode {
  const ast = parse(template)
  transform(ast, {
    nodeTransforms: [
      transformVBindShorthand,
      ...(options.prefixIdentifiers ? [transformExpression] : []),
      transformElement,
    ],
    directiveTransforms: {
      bind: transformBind,
    },
    ...options,
  })
  return ast.children[0] as ElementNode
}
"#,
        "",
        "Vue 3 core vBind local transform helper",
    )?;
    write_text(
        &transforms.join("vBind.rust-api.ts"),
        r#"import {
  type CompilerOptions,
  NodeTypes,
  __vuecRuntime,
} from '../../src'

const runtime = __vuecRuntime as any

export function parseWithVBind(
  template: string,
  options: CompilerOptions = {},
) {
  const node = runtime.callBridge('vue3.core.transformBindSuite', {
    source: template,
    options: normalizeOptions(options),
  })
  hydrateVBindAst(node)
  emitErrors(node, options)
  return node
}

function normalizeOptions(options: CompilerOptions) {
  const normalized: Record<string, unknown> = {}
  for (const key of Object.keys(options || {}) as Array<keyof CompilerOptions>) {
    const value = options[key]
    if (typeof value !== 'function') normalized[key as string] = value
  }
  normalized.__vuecBrowser = currentBrowserFlag()
  return normalized
}

function currentBrowserFlag() {
  return (
    (typeof __BROWSER__ !== 'undefined' && !!__BROWSER__) ||
    (typeof globalThis !== 'undefined' && !!(globalThis as any).__BROWSER__)
  )
}

function emitErrors(node: any, options: CompilerOptions) {
  const onError = (options as any).onError
  if (typeof onError !== 'function') return
  for (const error of node.__vuecErrors || []) {
    onError({ code: error.code, loc: error.loc })
  }
}

function hydrateVBindAst(node: any): any {
  if (!node || typeof node !== 'object') return node
  if (Array.isArray(node)) {
    node.forEach(hydrateVBindAst)
    return node
  }
  if (
    node.type === NodeTypes.JS_CALL_EXPRESSION &&
    typeof node.callee === 'string'
  ) {
    node.callee = helperSymbol(node.callee) || node.callee
  }
  if (node.type === NodeTypes.VNODE_CALL && typeof node.tag === 'string') {
    node.tag = helperSymbol(node.tag) || node.tag
  }
  for (const key of [
    'children',
    'props',
    'content',
    'codegenNode',
    'arguments',
    'returns',
    'params',
    'directives',
    'source',
    'valueAlias',
    'keyAlias',
    'objectIndexAlias',
    'parseResult',
    'branches',
    'condition',
    'test',
    'consequent',
    'alternate',
    'value',
    'elements',
    'properties',
    'key',
  ]) {
    hydrateVBindAst(node[key])
  }
  return node
}

function helperSymbol(name: string) {
  return typeof name === 'string' ? runtime[name] : undefined
}
"#,
    )?;
    Ok(())
}

fn rewrite_vue3_core_v_on_public_api_spec(prepared_root: &Path) -> Result<()> {
    let transforms = prepared_root
        .join("packages")
        .join("compiler-core")
        .join("__tests__")
        .join("transforms");
    let spec = transforms.join("vOn.spec.ts");
    if !spec.exists() {
        return Ok(());
    }
    rewrite_text_file_block(
        &spec,
        r#"import {
  type CompilerOptions,
  type ElementNode,
  ErrorCodes,
  NodeTypes,
  type ObjectExpression,
  TO_HANDLER_KEY,
  type VNodeCall,
  helperNameMap,
  baseParse as parse,
  transform,
} from '../../src'
import { transformFor } from '../../src/transforms/vFor'
import { transformOn } from '../../src/transforms/vOn'
import { transformElement } from '../../src/transforms/transformElement'
import { transformExpression } from '../../src/transforms/transformExpression'"#,
        r#"import {
  type CompilerOptions,
  type ElementNode,
  ErrorCodes,
  NodeTypes,
  type ObjectExpression,
  TO_HANDLER_KEY,
  type VNodeCall,
  helperNameMap,
} from '../../src'
import { parseWithVOn } from './vOn.rust-api'"#,
        "Vue 3 core vOn Rust API imports",
    )?;
    rewrite_text_file_block(
        &spec,
        r#"function parseWithVOn(template: string, options: CompilerOptions = {}) {
  const ast = parse(template, options)
  transform(ast, {
    nodeTransforms: [transformExpression, transformElement, transformFor],
    directiveTransforms: {
      on: transformOn,
    },
    ...options,
  })
  return {
    root: ast,
    node: ast.children[0] as ElementNode,
  }
}
"#,
        "",
        "Vue 3 core vOn local transform helper",
    )?;
    write_text(
        &transforms.join("vOn.rust-api.ts"),
        r#"import {
  type CompilerOptions,
  type ElementNode,
  NodeTypes,
  __vuecRuntime,
} from '../../src'

const runtime = __vuecRuntime as any

export function parseWithVOn(
  template: string,
  options: CompilerOptions = {},
) {
  const result = runtime.callBridge('vue3.core.transformOnSuite', {
    source: template,
    options: normalizeOptions(options, template),
  })
  const root = result.root || result
  hydrateVOnAst(root)
  emitErrors(root, options)
  return {
    root,
    node: root.children[0] as ElementNode,
  }
}

function normalizeOptions(options: CompilerOptions, template: string) {
  const normalized: Record<string, unknown> = {}
  for (const key of Object.keys(options || {}) as Array<keyof CompilerOptions>) {
    const value = options[key]
    if (typeof value !== 'function') normalized[key as string] = value
  }
  const tags = extractVueTemplateTags(template)
  if (hasPredicateOption(options, 'isNativeTag')) {
    normalized.__vuecNativeTags = collectPredicateHits(
      (options as any).isNativeTag,
      tags,
    )
  }
  return normalized
}

function emitErrors(root: any, options: CompilerOptions) {
  const onError = (options as any).onError
  if (typeof onError !== 'function') return
  for (const error of root.__vuecErrors || []) {
    onError({ code: error.code, loc: error.loc })
  }
}

function hydrateVOnAst(node: any): any {
  if (!node || typeof node !== 'object') return node
  if (Array.isArray(node)) {
    node.forEach(hydrateVOnAst)
    return node
  }
  if (node.type === NodeTypes.ROOT && Array.isArray(node.helpers)) {
    node.helpers = new Set(node.helpers.map((name: string) => helperSymbol(name) || name))
  }
  if (
    node.type === NodeTypes.JS_CALL_EXPRESSION &&
    typeof node.callee === 'string'
  ) {
    node.callee = helperSymbol(node.callee) || node.callee
  }
  if (node.type === NodeTypes.VNODE_CALL) {
    if (typeof node.tag === 'string') node.tag = helperSymbol(node.tag) || node.tag
    if (node.patchFlag == null) delete node.patchFlag
  }
  for (const key of [
    'children',
    'props',
    'content',
    'codegenNode',
    'arguments',
    'returns',
    'params',
    'directives',
    'source',
    'valueAlias',
    'keyAlias',
    'objectIndexAlias',
    'parseResult',
    'branches',
    'condition',
    'test',
    'consequent',
    'alternate',
    'value',
    'elements',
    'properties',
    'key',
  ]) {
    hydrateVOnAst(node[key])
  }
  return node
}

function helperSymbol(name: string) {
  return typeof name === 'string' ? runtime[name] : undefined
}

function hasPredicateOption(options: CompilerOptions, name: string) {
  return (
    Object.prototype.hasOwnProperty.call(options || {}, name) &&
    (typeof (options as any)[name] === 'function' ||
      Array.isArray((options as any)[name]))
  )
}

function extractVueTemplateTags(source: string) {
  const tags: string[] = []
  const seen = new Set<string>()
  const pattern = /<\/?\s*([A-Za-z][A-Za-z0-9._:-]*)/g
  let match: RegExpExecArray | null
  while ((match = pattern.exec(source))) {
    const tag = match[1]
    if (!seen.has(tag)) {
      seen.add(tag)
      tags.push(tag)
    }
  }
  return tags
}

function collectPredicateHits(predicate: unknown, values: string[]) {
  if (Array.isArray(predicate)) return predicate.map(String)
  if (typeof predicate !== 'function') return []
  const hits: string[] = []
  for (const value of values) {
    try {
      if (predicate(value)) hits.push(value)
    } catch (_) {}
  }
  return hits
}
"#,
    )?;
    Ok(())
}

fn rewrite_vue3_core_v_for_public_api_spec(prepared_root: &Path) -> Result<()> {
    let transforms = prepared_root
        .join("packages")
        .join("compiler-core")
        .join("__tests__")
        .join("transforms");
    let spec = transforms.join("vFor.spec.ts");
    if !spec.exists() {
        return Ok(());
    }
    rewrite_text_file_block(
        &spec,
        r#"import { baseParse as parse } from '../../src/parser'
import { transform } from '../../src/transform'
import { transformIf } from '../../src/transforms/vIf'
import { transformFor } from '../../src/transforms/vFor'
import { transformBind } from '../../src/transforms/vBind'
import { transformElement } from '../../src/transforms/transformElement'
import { transformSlotOutlet } from '../../src/transforms/transformSlotOutlet'
import { transformExpression } from '../../src/transforms/transformExpression'
import {
  ConstantTypes,
  type ElementNode,
  type ForCodegenNode,
  type ForNode,
  type InterpolationNode,
  NodeTypes,
  type RootNode,
  type SimpleExpressionNode,
} from '../../src/ast'
import { ErrorCodes } from '../../src/errors'
import { type CompilerOptions, generate } from '../../src'
import { FRAGMENT, RENDER_LIST, RENDER_SLOT } from '../../src/runtimeHelpers'
import { PatchFlags } from '@vue/shared'
import { createObjectMatcher } from '../testUtils'
import { transformVBindShorthand } from '../../src/transforms/transformVBindShorthand'"#,
        r#"import {
  ConstantTypes,
  type ElementNode,
  type ForCodegenNode,
  type ForNode,
  type InterpolationNode,
  NodeTypes,
  type RootNode,
  type SimpleExpressionNode,
} from '../../src/ast'
import { ErrorCodes } from '../../src/errors'
import { type CompilerOptions, generate } from '../../src'
import { FRAGMENT, RENDER_LIST, RENDER_SLOT } from '../../src/runtimeHelpers'
import { PatchFlags } from '@vue/shared'
import { createObjectMatcher } from '../testUtils'
import { parseWithForTransform } from './vFor.rust-api'
export { parseWithForTransform } from './vFor.rust-api'"#,
        "Vue 3 core vFor Rust API imports",
    )?;
    rewrite_text_file_block(
        &spec,
        r#"export function parseWithForTransform(
  template: string,
  options: CompilerOptions = {},
): {
  root: RootNode
  node: ForNode & { codegenNode: ForCodegenNode }
} {
  const ast = parse(template, options)
  transform(ast, {
    nodeTransforms: [
      transformVBindShorthand,
      transformIf,
      transformFor,
      ...(options.prefixIdentifiers ? [transformExpression] : []),
      transformSlotOutlet,
      transformElement,
    ],
    directiveTransforms: {
      bind: transformBind,
    },
    ...options,
  })
  return {
    root: ast,
    node: ast.children[0] as ForNode & { codegenNode: ForCodegenNode },
  }
}
"#,
        "",
        "Vue 3 core vFor local transform helper",
    )?;
    write_text(
        &transforms.join("vFor.rust-api.ts"),
        r#"import {
  type CompilerOptions,
  type ForCodegenNode,
  type ForNode,
  type RootNode,
  NodeTypes,
  __vuecRuntime,
} from '../../src'

const runtime = __vuecRuntime as any

export function parseWithForTransform(
  template: string,
  options: CompilerOptions = {},
): {
  root: RootNode
  node: ForNode & { codegenNode: ForCodegenNode }
} {
  const result = runtime.callBridge('vue3.core.transformForSuite', {
    source: template,
    options: normalizeOptions(options),
  })
  const root = result.root || result
  hydrateVForAst(root)
  emitErrors(root, options)
  return {
    root,
    node: root.children[0] as ForNode & { codegenNode: ForCodegenNode },
  }
}

function normalizeOptions(options: CompilerOptions) {
  const normalized: Record<string, unknown> = {}
  for (const key of Object.keys(options || {}) as Array<keyof CompilerOptions>) {
    const value = options[key]
    if (typeof value !== 'function') normalized[key as string] = value
  }
  return normalized
}

function emitErrors(root: any, options: CompilerOptions) {
  const onError = (options as any).onError
  if (typeof onError !== 'function') return
  for (const error of root.__vuecErrors || []) {
    onError({ code: error.code, loc: error.loc })
  }
}

function hydrateVForAst(node: any): any {
  if (!node || typeof node !== 'object') return node
  if (Array.isArray(node)) {
    node.forEach(hydrateVForAst)
    return node
  }
  if (node.type === NodeTypes.ROOT && Array.isArray(node.helpers)) {
    node.helpers = new Set(node.helpers.map((name: string) => helperSymbol(name) || name))
  }
  if (
    node.type === NodeTypes.JS_CALL_EXPRESSION &&
    typeof node.callee === 'string'
  ) {
    node.callee = helperSymbol(node.callee) || node.callee
  }
  if (node.type === NodeTypes.VNODE_CALL) {
    if (typeof node.tag === 'string') node.tag = helperSymbol(node.tag) || node.tag
    for (const key of ['props', 'children', 'patchFlag', 'dynamicProps', 'directives']) {
      if (node[key] == null) node[key] = undefined
    }
  }
  if (node.type === NodeTypes.FOR) {
    for (const key of ['valueAlias', 'keyAlias', 'objectIndexAlias']) {
      if (node[key] == null) delete node[key]
    }
  }
  if (node.parseResult) {
    for (const key of ['value', 'key', 'index']) {
      if (node.parseResult[key] == null) delete node.parseResult[key]
    }
  }
  for (const key of [
    'children',
    'props',
    'content',
    'codegenNode',
    'arguments',
    'returns',
    'params',
    'directives',
    'source',
    'valueAlias',
    'keyAlias',
    'objectIndexAlias',
    'parseResult',
    'branches',
    'condition',
    'test',
    'consequent',
    'alternate',
    'value',
    'elements',
    'properties',
    'key',
  ]) {
    hydrateVForAst(node[key])
  }
  return node
}

function helperSymbol(name: string) {
  return typeof name === 'string' ? runtime[name] : undefined
}
"#,
    )?;
    Ok(())
}
