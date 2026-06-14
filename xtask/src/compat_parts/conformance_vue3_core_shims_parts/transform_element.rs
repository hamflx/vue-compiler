fn rewrite_vue3_core_transform_element_public_api_spec(prepared_root: &Path) -> Result<()> {
    let transforms = prepared_root
        .join("packages")
        .join("compiler-core")
        .join("__tests__")
        .join("transforms");
    let spec = transforms.join("transformElement.spec.ts");
    if !spec.exists() {
        return Ok(());
    }
    rewrite_text_file_block(
        &spec,
        r#"import {
  BindingTypes,
  type CompilerOptions,
  ErrorCodes,
  type NodeTransform,
  baseCompile,
  baseParse as parse,
  transform,
  transformExpression,
} from '../../src'
import {
  BASE_TRANSITION,
  CREATE_VNODE,
  GUARD_REACTIVE_PROPS,
  KEEP_ALIVE,
  MERGE_PROPS,
  NORMALIZE_CLASS,
  NORMALIZE_PROPS,
  NORMALIZE_STYLE,
  RESOLVE_COMPONENT,
  RESOLVE_DIRECTIVE,
  RESOLVE_DYNAMIC_COMPONENT,
  SUSPENSE,
  TELEPORT,
  TO_HANDLERS,
  helperNameMap,
} from '../../src/runtimeHelpers'
import {
  type DirectiveNode,
  NodeTypes,
  type RootNode,
  type VNodeCall,
  createObjectProperty,
} from '../../src/ast'
import { transformElement } from '../../src/transforms/transformElement'
import { transformStyle } from '../../../compiler-dom/src/transforms/transformStyle'
import { transformOn } from '../../src/transforms/vOn'
import { transformBind } from '../../src/transforms/vBind'
import { PatchFlags } from '@vue/shared'
import { createObjectMatcher } from '../testUtils'
import { transformText } from '../../src/transforms/transformText'
import { parseWithForTransform } from './vFor.spec'"#,
        r#"import {
  BindingTypes,
  type CompilerOptions,
  ErrorCodes,
  type NodeTransform,
  baseCompile,
  baseParse as parse,
  transform,
  transformExpression,
} from '../../src'
import {
  BASE_TRANSITION,
  CREATE_VNODE,
  GUARD_REACTIVE_PROPS,
  KEEP_ALIVE,
  MERGE_PROPS,
  NORMALIZE_CLASS,
  NORMALIZE_PROPS,
  NORMALIZE_STYLE,
  RESOLVE_COMPONENT,
  RESOLVE_DIRECTIVE,
  RESOLVE_DYNAMIC_COMPONENT,
  SUSPENSE,
  TELEPORT,
  TO_HANDLERS,
  helperNameMap,
} from '../../src/runtimeHelpers'
import {
  type DirectiveNode,
  NodeTypes,
  type RootNode,
  type VNodeCall,
  createObjectProperty,
} from '../../src/ast'
import { transformElement } from '../../src/transforms/transformElement'
import { transformStyle } from '../../../compiler-dom/src/transforms/transformStyle'
import { transformOn } from '../../src/transforms/vOn'
import { transformBind } from '../../src/transforms/vBind'
import { PatchFlags } from '@vue/shared'
import { createObjectMatcher } from '../testUtils'
import { transformText } from '../../src/transforms/transformText'
import {
  parseWithBind,
  parseWithElementTransform,
} from './transformElement.rust-api'
import { parseWithForTransform } from './vFor.rust-api'"#,
        "Vue 3 core transformElement Rust API imports",
    )?;
    rewrite_text_file_block(
        &spec,
        r#"function parseWithElementTransform(
  template: string,
  options: CompilerOptions = {},
): {
  root: RootNode
  node: VNodeCall
} {
  // wrap raw template in an extra div so that it doesn't get turned into a
  // block as root node
  const ast = parse(`<div>${template}</div>`, options)
  transform(ast, {
    nodeTransforms: [transformElement, transformText],
    ...options,
  })
  const codegenNode = (ast as any).children[0].children[0]
    .codegenNode as VNodeCall
  expect(codegenNode.type).toBe(NodeTypes.VNODE_CALL)
  return {
    root: ast,
    node: codegenNode,
  }
}

function parseWithBind(template: string, options?: CompilerOptions) {
  return parseWithElementTransform(template, {
    ...options,
    directiveTransforms: {
      ...options?.directiveTransforms,
      bind: transformBind,
    },
  })
}
"#,
        r#"function parseWithElementTransformOriginal(
  template: string,
  options: CompilerOptions = {},
): {
  root: RootNode
  node: VNodeCall
} {
  // wrap raw template in an extra div so that it doesn't get turned into a
  // block as root node
  const ast = parse(`<div>${template}</div>`, options)
  transform(ast, {
    nodeTransforms: [transformElement, transformText],
    ...options,
  })
  const codegenNode = (ast as any).children[0].children[0]
    .codegenNode as VNodeCall
  expect(codegenNode.type).toBe(NodeTypes.VNODE_CALL)
  return {
    root: ast,
    node: codegenNode,
  }
}
"#,
        "Vue 3 core transformElement local transform helper",
    )?;
    rewrite_text_file_block(
        &spec,
        r#"  test('should handle <KeepAlive>', () => {
    function assert(tag: string) {
      const root = parse(`<div><${tag}><span /></${tag}></div>`)
      transform(root, {
        nodeTransforms: [transformElement, transformText],
      })
      expect(root.components.length).toBe(0)
      expect(root.helpers).toContain(KEEP_ALIVE)
      const node = (root.children[0] as any).children[0].codegenNode
      expect(node).toMatchObject({
        type: NodeTypes.VNODE_CALL,
        tag: KEEP_ALIVE,
        isBlock: true, // should be forced into a block
        props: undefined,
        // keep-alive should not compile content to slots
        children: [{ type: NodeTypes.ELEMENT, tag: 'span' }],
        // should get a dynamic slots flag to force updates
        patchFlag: PatchFlags.DYNAMIC_SLOTS,
      })
    }

    assert(`keep-alive`)
    assert(`KeepAlive`)
  })"#,
        r#"  test('should handle <KeepAlive>', () => {
    function assert(tag: string) {
      const { root, node } = parseWithElementTransform(
        `<${tag}><span /></${tag}>`,
      )
      expect(root.components.length).toBe(0)
      expect(root.helpers).toContain(KEEP_ALIVE)
      expect(node).toMatchObject({
        type: NodeTypes.VNODE_CALL,
        tag: KEEP_ALIVE,
        isBlock: true, // should be forced into a block
        props: undefined,
        // keep-alive should not compile content to slots
        children: [{ type: NodeTypes.ELEMENT, tag: 'span' }],
        // should get a dynamic slots flag to force updates
        patchFlag: PatchFlags.DYNAMIC_SLOTS,
      })
    }

    assert(`keep-alive`)
    assert(`KeepAlive`)
  })"#,
        "Vue 3 core transformElement KeepAlive Rust helper",
    )?;
    rewrite_text_file_block(
        &spec,
        r#"  test('<svg> should be forced into blocks', () => {
    const ast = parse(`<div><svg/></div>`)
    transform(ast, {
      nodeTransforms: [transformElement],
    })
    expect((ast as any).children[0].children[0].codegenNode).toMatchObject({
      type: NodeTypes.VNODE_CALL,
      tag: `"svg"`,
      isBlock: true,
    })
  })"#,
        r#"  test('<svg> should be forced into blocks', () => {
    const { node } = parseWithElementTransform(`<svg/>`)
    expect(node).toMatchObject({
      type: NodeTypes.VNODE_CALL,
      tag: `"svg"`,
      isBlock: true,
    })
  })"#,
        "Vue 3 core transformElement svg Rust helper",
    )?;
    rewrite_text_file_block(
        &spec,
        r#"  test('<math> should be forced into blocks', () => {
    const ast = parse(`<div><math/></div>`)
    transform(ast, {
      nodeTransforms: [transformElement],
    })
    expect((ast as any).children[0].children[0].codegenNode).toMatchObject({
      type: NodeTypes.VNODE_CALL,
      tag: `"math"`,
      isBlock: true,
    })
  })"#,
        r#"  test('<math> should be forced into blocks', () => {
    const { node } = parseWithElementTransform(`<math/>`)
    expect(node).toMatchObject({
      type: NodeTypes.VNODE_CALL,
      tag: `"math"`,
      isBlock: true,
    })
  })"#,
        "Vue 3 core transformElement math Rust helper",
    )?;
    rewrite_text_file_block(
        &spec,
        r#"  // #938
  test('element with dynamic keys should be forced into blocks', () => {
    const ast = parse(`<div><div :key="foo" /></div>`)
    transform(ast, {
      nodeTransforms: [transformElement],
    })
    expect((ast as any).children[0].children[0].codegenNode).toMatchObject({
      type: NodeTypes.VNODE_CALL,
      tag: `"div"`,
      isBlock: true,
    })
  })"#,
        r#"  // #938
  test('element with dynamic keys should be forced into blocks', () => {
    const { node } = parseWithElementTransform(`<div :key="foo" />`)
    expect(node).toMatchObject({
      type: NodeTypes.VNODE_CALL,
      tag: `"div"`,
      isBlock: true,
    })
  })"#,
        "Vue 3 core transformElement dynamic key Rust helper",
    )?;
    rewrite_text_file_import(
        &spec,
        "const { node } = parseWithElementTransform(`<div v-foo:bar=\"hello\" />`, {\n      directiveTransforms: {\n        foo(dir) {",
        "const { node } = parseWithElementTransformOriginal(`<div v-foo:bar=\"hello\" />`, {\n      directiveTransforms: {\n        foo(dir) {",
    )?;
    rewrite_text_file_import(
        &spec,
        "const { root, node } = parseWithElementTransform(\n      `<div v-foo:bar=\"hello\" />`,\n      {\n        directiveTransforms: {\n          foo() {\n            return {\n              props: [],\n              needRuntime: true,",
        "const { root, node } = parseWithElementTransformOriginal(\n      `<div v-foo:bar=\"hello\" />`,\n      {\n        directiveTransforms: {\n          foo() {\n            return {\n              props: [],\n              needRuntime: true,",
    )?;
    rewrite_text_file_import(
        &spec,
        "const { root, node } = parseWithElementTransform(\n      `<div v-foo:bar=\"hello\" />`,\n      {\n        directiveTransforms: {\n          foo() {\n            return {\n              props: [],\n              needRuntime: CREATE_VNODE,",
        "const { root, node } = parseWithElementTransformOriginal(\n      `<div v-foo:bar=\"hello\" />`,\n      {\n        directiveTransforms: {\n          foo() {\n            return {\n              props: [],\n              needRuntime: CREATE_VNODE,",
    )?;
    write_text(
        &transforms.join("transformElement.rust-api.ts"),
        r#"import {
  type CompilerOptions,
  NodeTypes,
  type RootNode,
  type VNodeCall,
  __vuecRuntime,
} from '../../src'

const runtime = __vuecRuntime as any

export function parseWithElementTransform(
  template: string,
  options: CompilerOptions = {},
): {
  root: RootNode
  node: VNodeCall
} {
  return parseWithElementTransformBridge(template, options, {})
}

export function parseWithBind(
  template: string,
  options: CompilerOptions = {},
) {
  return parseWithElementTransformBridge(template, options, {
    transformBind: true,
  })
}

function parseWithElementTransformBridge(
  template: string,
  options: CompilerOptions,
  extra: Record<string, unknown>,
) {
  const result = runtime.callBridge('vue3.core.transformElementSuite', {
    source: `<div>${template}</div>`,
    options: normalizeOptions(options, template, extra),
  })
  const root = result.root || result
  hydrateTransformElementAst(root)
  emitErrors(root, options)
  const node =
    root.children?.[0]?.children?.[0]?.codegenNode || result.node || null
  hydrateTransformElementAst(node)
  expect(node && node.type).toBe(NodeTypes.VNODE_CALL)
  return {
    root,
    node: node as VNodeCall,
  }
}

function normalizeOptions(
  options: CompilerOptions,
  template: string,
  extra: Record<string, unknown>,
) {
  const normalized: Record<string, unknown> = { ...extra }
  for (const key of Object.keys(options || {}) as Array<keyof CompilerOptions>) {
    if (key === 'directiveTransforms' || key === 'nodeTransforms') continue
    if (key === 'bindingMetadata') continue
    const value = options[key]
    if (typeof value !== 'function') normalized[key as string] = value
  }
  normalized.transformBind =
    Boolean((extra as any).transformBind) ||
    Boolean((options as any).directiveTransforms?.bind)
  normalized.transformOn = Boolean((options as any).directiveTransforms?.on)
  normalized.transformStyle = hasNamedNodeTransform(options, 'transformStyle')
  if ((options as any).bindingMetadata) {
    normalized.bindingMetadata = normalizeBindingMetadata(
      (options as any).bindingMetadata,
    )
  }
  const tags = ['div', ...extractVueTemplateTags(template)]
  if (hasPredicateOption(options, 'isNativeTag')) {
    normalized.__vuecNativeTags = collectPredicateHits(
      (options as any).isNativeTag,
      tags,
      ['div'],
    )
  }
  return normalized
}

function normalizeBindingMetadata(metadata: Record<string, unknown>) {
  const normalized: Record<string, unknown> = { ...metadata }
  if (Object.prototype.hasOwnProperty.call(metadata, '__isScriptSetup')) {
    normalized.__isScriptSetup = (metadata as any).__isScriptSetup
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

function hydrateTransformElementAst(node: any): any {
  if (!node || typeof node !== 'object') return node
  if (Array.isArray(node)) {
    node.forEach(hydrateTransformElementAst)
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
    'tag',
    'hoists',
    'cached',
  ]) {
    hydrateTransformElementAst(node[key])
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

function hasNamedNodeTransform(options: CompilerOptions, name: string) {
  const transforms = (options as any).nodeTransforms
  return (
    Array.isArray(transforms) &&
    transforms.some(
      transform =>
        typeof transform === 'function' && transform.name === name,
    )
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

function collectPredicateHits(
  predicate: unknown,
  values: string[],
  forced: string[],
) {
  const hits = new Set<string>(forced)
  if (Array.isArray(predicate)) {
    for (const value of predicate) hits.add(String(value))
    return Array.from(hits)
  }
  if (typeof predicate !== 'function') return Array.from(hits)
  for (const value of values) {
    try {
      if (predicate(value)) hits.add(value)
    } catch (_) {}
  }
  return Array.from(hits)
}
"#,
    )?;
    remove_snapshot_entries_by_key_prefix(
        &transforms
            .join("__snapshots__")
            .join("transformElement.spec.ts.snap"),
        "compiler: v-for > codegen > ",
    )?;
    Ok(())
}
