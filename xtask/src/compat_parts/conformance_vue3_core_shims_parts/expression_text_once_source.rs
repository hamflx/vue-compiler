fn rewrite_vue3_core_transform_expressions_public_api_spec(prepared_root: &Path) -> Result<()> {
    let transforms = prepared_root
        .join("packages")
        .join("compiler-core")
        .join("__tests__")
        .join("transforms");
    let spec = transforms.join("transformExpressions.spec.ts");
    if !spec.exists() {
        return Ok(());
    }
    rewrite_text_file_block(
        &spec,
        r#"import {
  BindingTypes,
  type CompilerOptions,
  ConstantTypes,
  type DirectiveNode,
  type ElementNode,
  type InterpolationNode,
  NodeTypes,
  baseCompile,
  baseParse as parse,
  transform,
} from '../../src'
import { transformIf } from '../../src/transforms/vIf'
import { transformExpression } from '../../src/transforms/transformExpression'
import { PatchFlagNames, PatchFlags } from '../../../shared/src'"#,
        r#"import {
  BindingTypes,
  type CompilerOptions,
  ConstantTypes,
  type DirectiveNode,
  type ElementNode,
  type InterpolationNode,
  NodeTypes,
  baseCompile,
} from '../../src'
import { PatchFlagNames, PatchFlags } from '../../../shared/src'
import { parseWithExpressionTransform } from './transformExpressions.rust-api'"#,
        "Vue 3 core transformExpressions Rust API imports",
    )?;
    rewrite_text_file_block(
        &spec,
        r#"function parseWithExpressionTransform(
  template: string,
  options: CompilerOptions = {},
) {
  const ast = parse(template, options)
  transform(ast, {
    prefixIdentifiers: true,
    nodeTransforms: [transformIf, transformExpression],
    ...options,
  })
  return ast.children[0]
}
"#,
        "",
        "Vue 3 core transformExpressions local transform helper",
    )?;
    write_text(
        &transforms.join("transformExpressions.rust-api.ts"),
        r#"import {
  type CompilerOptions,
  NodeTypes,
  __vuecRuntime,
} from '../../src'

const runtime = __vuecRuntime as any

export function parseWithExpressionTransform(
  template: string,
  options: CompilerOptions = {},
) {
  const node = runtime.callBridge('vue3.core.transformExpressionSuite', {
    source: template,
    options: normalizeOptions(options),
  })
  hydrateTransformExpressionsAst(node)
  emitErrors(node, options)
  return node
}

function normalizeOptions(options: CompilerOptions) {
  const normalized: Record<string, unknown> = {}
  for (const key of Object.keys(options || {}) as Array<keyof CompilerOptions>) {
    const value = options[key]
    if (typeof value !== 'function') normalized[key as string] = value
  }
  return normalized
}

function emitErrors(node: any, options: CompilerOptions) {
  const onError = (options as any).onError
  if (typeof onError !== 'function') return
  for (const error of node.__vuecErrors || []) {
    const emitted = new SyntaxError(error.message || 'Vue compiler error')
    ;(emitted as any).code = error.code
    ;(emitted as any).loc = error.loc
    onError(emitted)
  }
}

function hydrateTransformExpressionsAst(node: any): any {
  if (!node || typeof node !== 'object') return node
  if (Array.isArray(node)) {
    node.forEach(hydrateTransformExpressionsAst)
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
    hydrateTransformExpressionsAst(node[key])
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

fn rewrite_vue3_core_transform_text_public_api_spec(prepared_root: &Path) -> Result<()> {
    let transforms = prepared_root
        .join("packages")
        .join("compiler-core")
        .join("__tests__")
        .join("transforms");
    let spec = transforms.join("transformText.spec.ts");
    if !spec.exists() {
        return Ok(());
    }
    rewrite_text_file_block(
        &spec,
        r#"import {
  type CompilerOptions,
  type ElementNode,
  type ForNode,
  NodeTypes,
  generate,
  isWhitespaceText,
  baseParse as parse,
  transform,
} from '../../src'
import { transformFor } from '../../src/transforms/vFor'
import { transformText } from '../../src/transforms/transformText'
import { transformExpression } from '../../src/transforms/transformExpression'
import { transformElement } from '../../src/transforms/transformElement'
import { CREATE_TEXT } from '../../src/runtimeHelpers'"#,
        r#"import {
  type CompilerOptions,
  type ElementNode,
  type ForNode,
  NodeTypes,
  generate,
  isWhitespaceText,
} from '../../src'
import { CREATE_TEXT } from '../../src/runtimeHelpers'
import { transformWithTextOpt } from './transformText.rust-api'"#,
        "Vue 3 core transformText Rust API imports",
    )?;
    rewrite_text_file_block(
        &spec,
        r#"function transformWithTextOpt(template: string, options: CompilerOptions = {}) {
  const ast = parse(template)
  transform(ast, {
    nodeTransforms: [
      transformFor,
      ...(options.prefixIdentifiers ? [transformExpression] : []),
      transformElement,
      transformText,
    ],
    ...options,
  })
  return ast
}
"#,
        "",
        "Vue 3 core transformText local transform helper",
    )?;
    write_text(
        &transforms.join("transformText.rust-api.ts"),
        r#"import {
  type CompilerOptions,
  NodeTypes,
  __vuecRuntime,
} from '../../src'

const runtime = __vuecRuntime as any

export function transformWithTextOpt(
  template: string,
  options: CompilerOptions = {},
) {
  const root = runtime.callBridge('vue3.core.transformTextSuite', {
    source: template,
    options: normalizeOptions(options),
  })
  return hydrateTransformTextAst(root)
}

function normalizeOptions(options: CompilerOptions) {
  const normalized: Record<string, unknown> = {}
  for (const key of Object.keys(options || {}) as Array<keyof CompilerOptions>) {
    const value = options[key]
    if (typeof value !== 'function') normalized[key as string] = value
  }
  return normalized
}

function hydrateTransformTextAst(node: any): any {
  if (!node || typeof node !== 'object') return node
  if (Array.isArray(node)) {
    node.forEach(hydrateTransformTextAst)
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
  ]) {
    hydrateTransformTextAst(node[key])
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

fn rewrite_vue3_core_v_once_public_api_spec(prepared_root: &Path) -> Result<()> {
    let transforms = prepared_root
        .join("packages")
        .join("compiler-core")
        .join("__tests__")
        .join("transforms");
    let spec = transforms.join("vOnce.spec.ts");
    if !spec.exists() {
        return Ok(());
    }
    rewrite_text_file_block(
        &spec,
        r#"import {
  type CompilerOptions,
  NodeTypes,
  generate,
  getBaseTransformPreset,
  baseParse as parse,
  transform,
} from '../../src'
import { RENDER_SLOT, SET_BLOCK_TRACKING } from '../../src/runtimeHelpers'"#,
        r#"import {
  type CompilerOptions,
  NodeTypes,
  generate,
} from '../../src'
import { RENDER_SLOT, SET_BLOCK_TRACKING } from '../../src/runtimeHelpers'
import { transformWithOnce } from './vOnce.rust-api'"#,
        "Vue 3 core vOnce Rust API imports",
    )?;
    rewrite_text_file_block(
        &spec,
        r#"function transformWithOnce(template: string, options: CompilerOptions = {}) {
  const ast = parse(template)
  const [nodeTransforms, directiveTransforms] = getBaseTransformPreset()
  transform(ast, {
    nodeTransforms,
    directiveTransforms,
    ...options,
  })
  return ast
}
"#,
        "",
        "Vue 3 core vOnce local transform helper",
    )?;
    write_text(
        &transforms.join("vOnce.rust-api.ts"),
        r#"import {
  type CompilerOptions,
  NodeTypes,
  __vuecRuntime,
} from '../../src'

const runtime = __vuecRuntime as any

export function transformWithOnce(
  template: string,
  options: CompilerOptions = {},
) {
  const root = runtime.callBridge('vue3.core.transformOnceSuite', {
    source: template,
    options: normalizeOptions(options),
  })
  return hydrateTransformOnceAst(root)
}

function normalizeOptions(options: CompilerOptions) {
  const normalized: Record<string, unknown> = {}
  for (const key of Object.keys(options || {}) as Array<keyof CompilerOptions>) {
    const value = options[key]
    if (typeof value !== 'function') normalized[key as string] = value
  }
  return normalized
}

function hydrateTransformOnceAst(node: any): any {
  if (!node || typeof node !== 'object') return node
  if (Array.isArray(node)) {
    node.forEach(hydrateTransformOnceAst)
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
    hydrateTransformOnceAst(node[key])
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

fn write_vue3_core_source_shims(prepared_root: &Path) -> Result<()> {
    let core_src = prepared_root
        .join("packages")
        .join("compiler-core")
        .join("src");
    let transforms = core_src.join("transforms");
    fs::create_dir_all(&transforms)
        .with_context(|| format!("failed to create {}", transforms.display()))?;
    for module in [
        "index",
        "ast",
        "codegen",
        "compile",
        "errors",
        "options",
        "parser",
        "runtimeHelpers",
        "transform",
        "utils",
    ] {
        write_reexport_module(&core_src.join(format!("{module}.ts")), "@vue/compiler-core")?;
    }
    for module in [
        "transformElement",
        "transformExpression",
        "transformSlotOutlet",
        "transformText",
        "transformVBindShorthand",
        "vBind",
        "vFor",
        "vIf",
        "vMemo",
        "vModel",
        "vOn",
        "vOnce",
        "vSlot",
    ] {
        write_vue3_core_transform_shim(&transforms.join(format!("{module}.ts")), module)?;
    }

    let dom_transform = prepared_root
        .join("packages")
        .join("compiler-dom")
        .join("src")
        .join("transforms");
    fs::create_dir_all(&dom_transform)
        .with_context(|| format!("failed to create {}", dom_transform.display()))?;
    write_reexport_module(
        &dom_transform.join("transformStyle.ts"),
        "@vue/compiler-dom",
    )?;

    let shared_src = prepared_root.join("packages").join("shared").join("src");
    fs::create_dir_all(&shared_src)
        .with_context(|| format!("failed to create {}", shared_src.display()))?;
    write_reexport_module(&shared_src.join("index.ts"), "@vue/shared")?;
    Ok(())
}
