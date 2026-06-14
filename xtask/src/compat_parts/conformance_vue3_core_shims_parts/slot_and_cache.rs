fn rewrite_vue3_core_transform_slot_outlet_public_api_spec(prepared_root: &Path) -> Result<()> {
    let transforms = prepared_root
        .join("packages")
        .join("compiler-core")
        .join("__tests__")
        .join("transforms");
    let spec = transforms.join("transformSlotOutlet.spec.ts");
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
  baseParse as parse,
  transform,
} from '../../src'
import { transformElement } from '../../src/transforms/transformElement'
import { transformOn } from '../../src/transforms/vOn'
import { transformBind } from '../../src/transforms/vBind'
import { transformExpression } from '../../src/transforms/transformExpression'
import { RENDER_SLOT } from '../../src/runtimeHelpers'
import { transformSlotOutlet } from '../../src/transforms/transformSlotOutlet'"#,
        r#"import {
  type CompilerOptions,
  type ElementNode,
  ErrorCodes,
  NodeTypes,
} from '../../src'
import { RENDER_SLOT } from '../../src/runtimeHelpers'
import { parseWithSlots } from './transformSlotOutlet.rust-api'"#,
        "Vue 3 core transformSlotOutlet Rust API imports",
    )?;
    rewrite_text_file_block(
        &spec,
        r#"function parseWithSlots(template: string, options: CompilerOptions = {}) {
  const ast = parse(template)
  transform(ast, {
    nodeTransforms: [
      ...(options.prefixIdentifiers ? [transformExpression] : []),
      transformSlotOutlet,
      transformElement,
    ],
    directiveTransforms: {
      on: transformOn,
      bind: transformBind,
    },
    ...options,
  })
  return ast
}
"#,
        "",
        "Vue 3 core transformSlotOutlet local transform helper",
    )?;
    write_text(
        &transforms.join("transformSlotOutlet.rust-api.ts"),
        r#"import {
  type CompilerOptions,
  NodeTypes,
  __vuecRuntime,
} from '../../src'

const runtime = __vuecRuntime as any

export function parseWithSlots(
  template: string,
  options: CompilerOptions = {},
) {
  const root = runtime.callBridge('vue3.core.transformSlotOutletSuite', {
    source: template,
    options: normalizeOptions(options),
  })
  hydrateTransformSlotOutletAst(root)
  emitErrors(root, options)
  return root
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

function hydrateTransformSlotOutletAst(node: any): any {
  if (!node || typeof node !== 'object') return node
  if (Array.isArray(node)) {
    node.forEach(hydrateTransformSlotOutletAst)
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
    hydrateTransformSlotOutletAst(node[key])
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

fn rewrite_vue3_core_v_slot_public_api_spec(prepared_root: &Path) -> Result<()> {
    let transforms = prepared_root
        .join("packages")
        .join("compiler-core")
        .join("__tests__")
        .join("transforms");
    let spec = transforms.join("vSlot.spec.ts");
    if !spec.exists() {
        return Ok(());
    }
    rewrite_text_file_block(
        &spec,
        r#"import {
  type CompilerOptions,
  type ComponentNode,
  type ElementNode,
  ErrorCodes,
  type ForNode,
  NodeTypes,
  type ObjectExpression,
  type RenderSlotCall,
  type SimpleExpressionNode,
  type SlotsExpression,
  type VNodeCall,
  generate,
  baseParse as parse,
  transform,
} from '../../src'
import { transformElement } from '../../src/transforms/transformElement'
import { transformOn } from '../../src/transforms/vOn'
import { transformBind } from '../../src/transforms/vBind'
import { transformExpression } from '../../src/transforms/transformExpression'
import { transformSlotOutlet } from '../../src/transforms/transformSlotOutlet'
import {
  trackSlotScopes,
  trackVForSlotScopes,
} from '../../src/transforms/vSlot'
import { CREATE_SLOTS, RENDER_LIST } from '../../src/runtimeHelpers'
import { createObjectMatcher } from '../testUtils'
import { PatchFlags } from '@vue/shared'
import { transformFor } from '../../src/transforms/vFor'
import { transformIf } from '../../src/transforms/vIf'
import { transformText } from '../../src/transforms/transformText'"#,
        r#"import {
  type CompilerOptions,
  type ComponentNode,
  type ElementNode,
  ErrorCodes,
  type ForNode,
  NodeTypes,
  type ObjectExpression,
  type RenderSlotCall,
  type SimpleExpressionNode,
  type VNodeCall,
  generate,
} from '../../src'
import { CREATE_SLOTS, RENDER_LIST } from '../../src/runtimeHelpers'
import { createObjectMatcher } from '../testUtils'
import { PatchFlags } from '@vue/shared'
import { parseWithSlots } from './vSlot.rust-api'"#,
        "Vue 3 core vSlot Rust API imports",
    )?;
    rewrite_text_file_block(
        &spec,
        r#"function parseWithSlots(
  template: string,
  options: CompilerOptions & { transformText?: boolean } = {},
) {
  const ast = parse(template, {
    whitespace: options.whitespace,
  })
  transform(ast, {
    nodeTransforms: [
      transformIf,
      transformFor,
      ...(options.prefixIdentifiers
        ? [trackVForSlotScopes, transformExpression]
        : []),
      transformSlotOutlet,
      transformElement,
      trackSlotScopes,
      ...(options.transformText ? [transformText] : []),
    ],
    directiveTransforms: {
      on: transformOn,
      bind: transformBind,
    },
    ...options,
  })
  return {
    root: ast,
    slots:
      ast.children[0].type === NodeTypes.ELEMENT
        ? ((ast.children[0].codegenNode as VNodeCall)
            .children as SlotsExpression)
        : null,
  }
}
"#,
        "",
        "Vue 3 core vSlot local transform helper",
    )?;
    write_text(
        &transforms.join("vSlot.rust-api.ts"),
        r#"import {
  type CompilerOptions,
  NodeTypes,
  __vuecRuntime,
} from '../../src'

const runtime = __vuecRuntime as any

export function parseWithSlots(
  template: string,
  options: CompilerOptions & { transformText?: boolean } = {},
) {
  const result = runtime.callBridge('vue3.core.transformSlotSuite', {
    source: template,
    options: normalizeOptions(options),
  })
  const root = result.root || result
  hydrateVSlotAst(root)
  hydrateVSlotAst(result.slots)
  emitErrors(root, options)
  return {
    root,
    slots: result.slots == null ? null : result.slots,
  }
}

function normalizeOptions(options: CompilerOptions & { transformText?: boolean }) {
  const normalized: Record<string, unknown> = {}
  for (const key of Object.keys(options || {}) as Array<keyof typeof options>) {
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

function hydrateVSlotAst(node: any): any {
  if (!node || typeof node !== 'object') return node
  if (Array.isArray(node)) {
    node.forEach(hydrateVSlotAst)
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
  if (node.type === NodeTypes.JS_FUNCTION_EXPRESSION) {
    if (node.params == null) node.params = undefined
  }
  if (node.type === NodeTypes.FOR) {
    for (const key of ['valueAlias', 'keyAlias', 'objectIndexAlias']) {
      if (node[key] == null) delete node[key]
    }
  }
  if (node.type === NodeTypes.IF_BRANCH) {
    if (node.condition == null) delete node.condition
    if (node.userKey == null) delete node.userKey
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
    hydrateVSlotAst(node[key])
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

fn rewrite_vue3_core_cache_static_public_api_spec(prepared_root: &Path) -> Result<()> {
    let transforms = prepared_root
        .join("packages")
        .join("compiler-core")
        .join("__tests__")
        .join("transforms");
    let spec = transforms.join("cacheStatic.spec.ts");
    if !spec.exists() {
        return Ok(());
    }
    rewrite_text_file_block(
        &spec,
        r#"import {
  type CompilerOptions,
  ConstantTypes,
  type ElementNode,
  type ForNode,
  type IfNode,
  NodeTypes,
  type VNodeCall,
  generate,
  baseParse as parse,
  transform,
} from '../../src'
import {
  FRAGMENT,
  NORMALIZE_CLASS,
  RENDER_LIST,
} from '../../src/runtimeHelpers'
import { transformElement } from '../../src/transforms/transformElement'
import { transformExpression } from '../../src/transforms/transformExpression'
import { transformIf } from '../../src/transforms/vIf'
import { transformFor } from '../../src/transforms/vFor'
import { transformBind } from '../../src/transforms/vBind'
import { transformOn } from '../../src/transforms/vOn'
import { createObjectMatcher } from '../testUtils'
import { transformText } from '../../src/transforms/transformText'
import { PatchFlags } from '@vue/shared'"#,
        r#"import {
  type CompilerOptions,
  ConstantTypes,
  type ElementNode,
  type ForNode,
  type IfNode,
  NodeTypes,
  type VNodeCall,
  generate,
} from '../../src'
import {
  FRAGMENT,
  NORMALIZE_CLASS,
  RENDER_LIST,
} from '../../src/runtimeHelpers'
import { createObjectMatcher } from '../testUtils'
import { PatchFlags } from '@vue/shared'
import { transformWithCache } from './cacheStatic.rust-api'"#,
        "Vue 3 core cacheStatic Rust API imports",
    )?;
    rewrite_text_file_block(
        &spec,
        r#"function transformWithCache(template: string, options: CompilerOptions = {}) {
  const ast = parse(template)
  transform(ast, {
    hoistStatic: true,
    nodeTransforms: [
      transformIf,
      transformFor,
      ...(options.prefixIdentifiers ? [transformExpression] : []),
      transformElement,
      transformText,
    ],
    directiveTransforms: {
      on: transformOn,
      bind: transformBind,
    },
    ...options,
  })
  expect(ast.codegenNode).toMatchObject({
    type: NodeTypes.VNODE_CALL,
    isBlock: true,
  })
  return ast
}
"#,
        "",
        "Vue 3 core cacheStatic local transform helper",
    )?;
    write_text(
        &transforms.join("cacheStatic.rust-api.ts"),
        r#"import {
  type CompilerOptions,
  NodeTypes,
  __vuecRuntime,
} from '../../src'

const runtime = __vuecRuntime as any

export function transformWithCache(
  template: string,
  options: CompilerOptions = {},
) {
  const root = runtime.callBridge('vue3.core.cacheStaticSuite', {
    source: template,
    options: normalizeOptions(options),
  })
  hydrateCacheStaticAst(root)
  emitErrors(root, options)
  expect(root.codegenNode).toMatchObject({
    type: NodeTypes.VNODE_CALL,
    isBlock: true,
  })
  return root
}

function normalizeOptions(options: CompilerOptions) {
  const normalized: Record<string, unknown> = { hoistStatic: true }
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

function hydrateCacheStaticAst(node: any): any {
  if (!node || typeof node !== 'object') return node
  if (Array.isArray(node)) {
    node.forEach(hydrateCacheStaticAst)
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
  if (node.type === NodeTypes.JS_FUNCTION_EXPRESSION) {
    if (node.params == null) node.params = undefined
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
    'hoists',
    'cached',
  ]) {
    hydrateCacheStaticAst(node[key])
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
