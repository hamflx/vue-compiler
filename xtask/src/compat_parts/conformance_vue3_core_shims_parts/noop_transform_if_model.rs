fn rewrite_vue3_core_noop_directive_transform_public_api_spec(prepared_root: &Path) -> Result<()> {
    let transforms = prepared_root
        .join("packages")
        .join("compiler-core")
        .join("__tests__")
        .join("transforms");
    let spec = transforms.join("noopDirectiveTransform.spec.ts");
    if !spec.exists() {
        return Ok(());
    }
    rewrite_text_file_block(
        &spec,
        r#"import {
  type ElementNode,
  type VNodeCall,
  noopDirectiveTransform,
  baseParse as parse,
  transform,
} from '../../src'
import { transformElement } from '../../src/transforms/transformElement'"#,
        r#"import {
  type ElementNode,
  type VNodeCall,
} from '../../src'
import { parseWithNoopDirectiveTransform } from './noopDirectiveTransform.rust-api'"#,
        "Vue 3 core noopDirectiveTransform Rust API imports",
    )?;
    rewrite_text_file_block(
        &spec,
        r#"    const ast = parse(`<div v-noop/>`)
    transform(ast, {
      nodeTransforms: [transformElement],
      directiveTransforms: {
        noop: noopDirectiveTransform,
      },
    })
    const node = ast.children[0] as ElementNode"#,
        r#"    const node = parseWithNoopDirectiveTransform(`<div v-noop/>`) as ElementNode"#,
        "Vue 3 core noopDirectiveTransform Rust helper",
    )?;
    write_text(
        &transforms.join("noopDirectiveTransform.rust-api.ts"),
        r#"import {
  type ElementNode,
  NodeTypes,
  __vuecRuntime,
} from '../../src'

const runtime = __vuecRuntime as any

export function parseWithNoopDirectiveTransform(template: string): ElementNode {
  const result = runtime.callBridge('vue3.core.transformElementSuite', {
    source: `<div>${template}</div>`,
    options: {
      noopDirectiveTransforms: ['noop'],
    },
  })
  const root = result.root || result
  hydrateAst(root)
  const node = root.children?.[0]?.children?.[0] || null
  hydrateAst(node)
  if (node?.codegenNode?.type !== NodeTypes.VNODE_CALL) {
    throw new Error('Expected Rust transformElementSuite to return a VNodeCall')
  }
  return node
}

function hydrateAst(node: any): any {
  if (!node || typeof node !== 'object') return node
  if (Array.isArray(node)) {
    node.forEach(hydrateAst)
    return node
  }
  if (node.type === NodeTypes.ROOT && Array.isArray(node.helpers)) {
    node.helpers = new Set(node.helpers.map((name: string) => helperSymbol(name) || name))
  }
  if (node.type === NodeTypes.VNODE_CALL) {
    if (typeof node.tag === 'string') node.tag = helperSymbol(node.tag) || node.tag
    for (const key of ['props', 'children', 'patchFlag', 'dynamicProps', 'directives']) {
      if (node[key] == null) node[key] = undefined
    }
  }
  for (const key of [
    'children',
    'props',
    'codegenNode',
    'arguments',
    'directives',
    'elements',
    'tag',
  ]) {
    hydrateAst(node[key])
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

fn rewrite_vue3_core_transform_public_api_spec(prepared_root: &Path) -> Result<()> {
    let tests = prepared_root
        .join("packages")
        .join("compiler-core")
        .join("__tests__");
    let spec = tests.join("transform.spec.ts");
    if !spec.exists() {
        return Ok(());
    }
    rewrite_text_file_block(
        &spec,
        r#"import { PatchFlags } from '@vue/shared'"#,
        r#"import { PatchFlags } from '@vue/shared'
import { transformWithCodegen } from './transform.rust-api'"#,
        "Vue 3 core transform Rust API imports",
    )?;
    rewrite_text_file_block(
        &spec,
        r#"  test('should inject toString helper for interpolations', () => {
    const ast = baseParse(`{{ foo }}`)
    transform(ast, {})
    expect(ast.helpers).toContain(TO_DISPLAY_STRING)
  })"#,
        r#"  test('should inject toString helper for interpolations', () => {
    const ast = transformWithCodegen(`{{ foo }}`)
    expect(ast.helpers).toContain(TO_DISPLAY_STRING)
  })"#,
        "Vue 3 core transform interpolation helper Rust path",
    )?;
    rewrite_text_file_block(
        &spec,
        r#"  test('should inject createVNode and Comment for comments', () => {
    const ast = baseParse(`<!--foo-->`)
    transform(ast, {})
    expect(ast.helpers).toContain(CREATE_COMMENT)
  })"#,
        r#"  test('should inject createVNode and Comment for comments', () => {
    const ast = transformWithCodegen(`<!--foo-->`)
    expect(ast.helpers).toContain(CREATE_COMMENT)
  })"#,
        "Vue 3 core transform comment helper Rust path",
    )?;
    rewrite_text_file_block(
        &spec,
        r#"    function transformWithCodegen(template: string) {
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
"#,
        "",
        "Vue 3 core transform local codegen helper",
    )?;
    write_text(
        &tests.join("transform.rust-api.ts"),
        r#"import { NodeTypes, __vuecRuntime } from '../src'

const runtime = __vuecRuntime as any

export function transformWithCodegen(template: string) {
  const root = runtime.callBridge('vue3.core.transformSuite', {
    source: template,
    options: {},
  })
  hydrateTransformAst(root)
  return root
}

function hydrateTransformAst(node: any): any {
  if (!node || typeof node !== 'object') return node
  if (Array.isArray(node)) {
    node.forEach(hydrateTransformAst)
    return node
  }
  if (node.type === NodeTypes.ROOT) {
    if (Array.isArray(node.helpers)) {
      node.helpers = new Set(node.helpers.map((name: string) => helperSymbol(name) || name))
    }
    if (node.codegenNode == null) node.codegenNode = undefined
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
    'tag',
    'hoists',
    'cached',
  ]) {
    hydrateTransformAst(node[key])
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

fn rewrite_vue3_core_v_if_public_api_spec(prepared_root: &Path) -> Result<()> {
    let transforms = prepared_root
        .join("packages")
        .join("compiler-core")
        .join("__tests__")
        .join("transforms");
    let spec = transforms.join("vIf.spec.ts");
    if !spec.exists() {
        return Ok(());
    }
    rewrite_text_file_block(
        &spec,
        r#"import { baseParse as parse } from '../../src/parser'
import { transform } from '../../src/transform'
import { transformIf } from '../../src/transforms/vIf'
import { transformElement } from '../../src/transforms/transformElement'
import { transformSlotOutlet } from '../../src/transforms/transformSlotOutlet'
import {
  type CommentNode,
  type ConditionalExpression,
  type ElementNode,
  ElementTypes,
  type IfBranchNode,
  type IfConditionalExpression,
  type IfNode,
  NodeTypes,
  type SimpleExpressionNode,
  type TextNode,
  type VNodeCall,
} from '../../src/ast'
import { ErrorCodes } from '../../src/errors'
import {
  type CompilerOptions,
  TO_HANDLERS,
  generate,
  transformVBindShorthand,
} from '../../src'
import {
  CREATE_COMMENT,
  FRAGMENT,
  MERGE_PROPS,
  NORMALIZE_PROPS,
  RENDER_SLOT,
} from '../../src/runtimeHelpers'
import { createObjectMatcher } from '../testUtils'"#,
        r#"import {
  type CommentNode,
  type ConditionalExpression,
  type ElementNode,
  ElementTypes,
  type IfBranchNode,
  type IfConditionalExpression,
  type IfNode,
  NodeTypes,
  type SimpleExpressionNode,
  type TextNode,
  type VNodeCall,
} from '../../src/ast'
import { ErrorCodes } from '../../src/errors'
import { type CompilerOptions, TO_HANDLERS, generate } from '../../src'
import {
  CREATE_COMMENT,
  FRAGMENT,
  MERGE_PROPS,
  NORMALIZE_PROPS,
  RENDER_SLOT,
} from '../../src/runtimeHelpers'
import { createObjectMatcher } from '../testUtils'
import { parseWithIfTransform } from './vIf.rust-api'"#,
        "Vue 3 core vIf Rust API imports",
    )?;
    rewrite_text_file_block(
        &spec,
        r#"function parseWithIfTransform(
  template: string,
  options: CompilerOptions = {},
  returnIndex: number = 0,
  childrenLen: number = 1,
) {
  const ast = parse(template, options)
  transform(ast, {
    nodeTransforms: [
      transformVBindShorthand,
      transformIf,
      transformSlotOutlet,
      transformElement,
    ],
    ...options,
  })
  if (!options.onError) {
    expect(ast.children.length).toBe(childrenLen)
    for (let i = 0; i < childrenLen; i++) {
      expect(ast.children[i].type).toBe(NodeTypes.IF)
    }
  }
  return {
    root: ast,
    node: ast.children[returnIndex] as IfNode & {
      codegenNode: IfConditionalExpression
    },
  }
}
"#,
        "",
        "Vue 3 core vIf local transform helper",
    )?;
    write_text(
        &transforms.join("vIf.rust-api.ts"),
        r#"import {
  type CompilerOptions,
  type IfConditionalExpression,
  type IfNode,
  NodeTypes,
  __vuecRuntime,
} from '../../src'

const runtime = __vuecRuntime as any

export function parseWithIfTransform(
  template: string,
  options: CompilerOptions = {},
  returnIndex: number = 0,
  childrenLen: number = 1,
) {
  const result = runtime.callBridge('vue3.core.transformIfSuite', {
    source: template,
    options: normalizeOptions(options),
  })
  const root = result.root || result
  hydrateVIfAst(root)
  emitErrors(root, options)
  if (!(options as any).onError) {
    expect(root.children.length).toBe(childrenLen)
    for (let i = 0; i < childrenLen; i++) {
      expect(root.children[i].type).toBe(NodeTypes.IF)
    }
  }
  return {
    root,
    node: root.children[returnIndex] as IfNode & {
      codegenNode: IfConditionalExpression
    },
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

function hydrateVIfAst(node: any): any {
  if (!node || typeof node !== 'object') return node
  if (Array.isArray(node)) {
    node.forEach(hydrateVIfAst)
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
  if (node.type === NodeTypes.IF_BRANCH) {
    if (node.condition == null) delete node.condition
    if (node.userKey == null) delete node.userKey
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
    hydrateVIfAst(node[key])
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

fn rewrite_vue3_core_v_model_public_api_spec(prepared_root: &Path) -> Result<()> {
    let transforms = prepared_root
        .join("packages")
        .join("compiler-core")
        .join("__tests__")
        .join("transforms");
    let spec = transforms.join("vModel.spec.ts");
    if !spec.exists() {
        return Ok(());
    }
    rewrite_text_file_block(
        &spec,
        r#"import {
  BindingTypes,
  type CompilerOptions,
  type ComponentNode,
  type ElementNode,
  type ForNode,
  NORMALIZE_PROPS,
  NodeTypes,
  type ObjectExpression,
  type PlainElementNode,
  type VNodeCall,
  generate,
  baseParse as parse,
  transform,
} from '../../src'
import { ErrorCodes } from '../../src/errors'
import { transformModel } from '../../src/transforms/vModel'
import { transformElement } from '../../src/transforms/transformElement'
import { transformExpression } from '../../src/transforms/transformExpression'
import { transformFor } from '../../src/transforms/vFor'
import { trackSlotScopes } from '../../src/transforms/vSlot'
import type { CallExpression } from '@babel/types'"#,
        r#"import {
  BindingTypes,
  type CompilerOptions,
  type ComponentNode,
  type ElementNode,
  type ForNode,
  NORMALIZE_PROPS,
  NodeTypes,
  type ObjectExpression,
  type PlainElementNode,
  type VNodeCall,
  generate,
} from '../../src'
import { ErrorCodes } from '../../src/errors'
import type { CallExpression } from '@babel/types'
import { parseWithVModel } from './vModel.rust-api'"#,
        "Vue 3 core vModel Rust API imports",
    )?;
    rewrite_text_file_block(
        &spec,
        r#"function parseWithVModel(template: string, options: CompilerOptions = {}) {
  const ast = parse(template)

  transform(ast, {
    nodeTransforms: [
      transformFor,
      transformExpression,
      transformElement,
      trackSlotScopes,
    ],
    directiveTransforms: {
      ...options.directiveTransforms,
      model: transformModel,
    },
    ...options,
  })

  return ast
}
"#,
        "",
        "Vue 3 core vModel local transform helper",
    )?;
    write_text(
        &transforms.join("vModel.rust-api.ts"),
        r#"import {
  type CompilerOptions,
  NodeTypes,
  __vuecRuntime,
} from '../../src'

const runtime = __vuecRuntime as any

export function parseWithVModel(
  template: string,
  options: CompilerOptions = {},
) {
  const root = runtime.callBridge('vue3.core.transformModelSuite', {
    source: template,
    options: normalizeOptions(options),
  })
  hydrateVModelAst(root)
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

function hydrateVModelAst(node: any): any {
  if (!node || typeof node !== 'object') return node
  if (Array.isArray(node)) {
    node.forEach(hydrateVModelAst)
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
    hydrateVModelAst(node[key])
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
