    #[test]
    fn vue3_core_conformance_shims_use_relative_vitest_glob() {
        let temp = std::env::temp_dir().join(format!(
            "vuec-xtask-vue3-core-shims-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let transforms_tests = temp
            .join("packages")
            .join("compiler-core")
            .join("__tests__")
            .join("transforms");
        fs::create_dir_all(&transforms_tests).unwrap();
        let transform_snapshots = transforms_tests.join("__snapshots__");
        fs::create_dir_all(&transform_snapshots).unwrap();
        fs::write(
            transform_snapshots.join("transformElement.spec.ts.snap"),
            r#"// Vitest Snapshot v1, https://vitest.dev/guide/snapshot.html

exports[`compiler: v-for > codegen > basic v-for 1`] = `
"old duplicated transformElement v-for snapshot"
`;
"#,
        )
        .unwrap();
        fs::write(
            transforms_tests.join("vOnce.spec.ts"),
            r#"import {
  type CompilerOptions,
  NodeTypes,
  generate,
  getBaseTransformPreset,
  baseParse as parse,
  transform,
} from '../../src'
import { RENDER_SLOT, SET_BLOCK_TRACKING } from '../../src/runtimeHelpers'

function transformWithOnce(template: string, options: CompilerOptions = {}) {
  const ast = parse(template)
  const [nodeTransforms, directiveTransforms] = getBaseTransformPreset()
  transform(ast, {
    nodeTransforms,
    directiveTransforms,
    ...options,
  })
  return ast
}

test('placeholder', () => {
  expect(transformWithOnce('<div v-once />')).toBeTruthy()
})
"#,
        )
        .unwrap();
        fs::write(
            transforms_tests.join("vBind.spec.ts"),
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
import { transformVBindShorthand } from '../../src/transforms/transformVBindShorthand'

function parseWithVBind(
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

test('placeholder', () => {
  expect(parseWithVBind('<div :id />')).toBeTruthy()
})
"#,
        )
        .unwrap();
        fs::write(
            transforms_tests.join("vModel.spec.ts"),
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
import type { CallExpression } from '@babel/types'

function parseWithVModel(template: string, options: CompilerOptions = {}) {
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

test('placeholder', () => {
  expect(parseWithVModel('<input v-model="model" />')).toBeTruthy()
})
"#,
        )
        .unwrap();
        fs::write(
            transforms_tests.join("vOn.spec.ts"),
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
import { transformExpression } from '../../src/transforms/transformExpression'

function parseWithVOn(template: string, options: CompilerOptions = {}) {
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

        test('placeholder', () => {
  expect(parseWithVOn('<div @click="foo" />')).toBeTruthy()
})
"#,
        )
        .unwrap();
        fs::write(
            transforms_tests.join("vFor.spec.ts"),
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
import { transformVBindShorthand } from '../../src/transforms/transformVBindShorthand'

export function parseWithForTransform(
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

        test('placeholder', () => {
  expect(parseWithForTransform('<div v-for="i in list" />')).toBeTruthy()
})
"#,
        )
        .unwrap();
        fs::write(
            transforms_tests.join("transformElement.spec.ts"),
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
import { parseWithForTransform } from './vFor.spec'

function parseWithElementTransform(
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

describe('compiler: element transform', () => {
  test('should handle <KeepAlive>', () => {
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
  })

  test('directiveTransforms', () => {
    let _dir: DirectiveNode
    const { node } = parseWithElementTransform(`<div v-foo:bar="hello" />`, {
      directiveTransforms: {
        foo(dir) {
          _dir = dir
          return {
            props: [createObjectProperty(dir.arg!, dir.exp!)],
          }
        },
      },
    })
    expect(node).toBeTruthy()
  })

  test('directiveTransform with needRuntime: true', () => {
    const { root, node } = parseWithElementTransform(
      `<div v-foo:bar="hello" />`,
      {
        directiveTransforms: {
          foo() {
            return {
              props: [],
              needRuntime: true,
            }
          },
        },
      },
    )
    expect(root).toBeTruthy()
    expect(node).toBeTruthy()
  })

  test('directiveTransform with needRuntime: Symbol', () => {
    const { root, node } = parseWithElementTransform(
      `<div v-foo:bar="hello" />`,
      {
        directiveTransforms: {
          foo() {
            return {
              props: [],
              needRuntime: CREATE_VNODE,
            }
          },
        },
      },
    )
    expect(root).toBeTruthy()
    expect(node).toBeTruthy()
  })

  test('<svg> should be forced into blocks', () => {
    const ast = parse(`<div><svg/></div>`)
    transform(ast, {
      nodeTransforms: [transformElement],
    })
    expect((ast as any).children[0].children[0].codegenNode).toMatchObject({
      type: NodeTypes.VNODE_CALL,
      tag: `"svg"`,
      isBlock: true,
    })
  })

  test('<math> should be forced into blocks', () => {
    const ast = parse(`<div><math/></div>`)
    transform(ast, {
      nodeTransforms: [transformElement],
    })
    expect((ast as any).children[0].children[0].codegenNode).toMatchObject({
      type: NodeTypes.VNODE_CALL,
      tag: `"math"`,
      isBlock: true,
    })
  })

  // #938
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
  })

  test('should process node when node has been replaced', () => {
    const customNodeTransform: NodeTransform = () => {}
    expect(customNodeTransform).toBeTruthy()
  })

  test('ref_for marker on static ref', () => {
    expect(parseWithForTransform(`<div v-for="i in l" ref="x"/>`)).toBeTruthy()
  })

  test('placeholder', () => {
    expect(parseWithBind('<div :id="id" />')).toBeTruthy()
    expect(baseCompile('<div />')).toBeTruthy()
  })
})
"#,
        )
        .unwrap();
        fs::write(
            transforms_tests.join("vIf.spec.ts"),
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
import { createObjectMatcher } from '../testUtils'

function parseWithIfTransform(
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

test('placeholder', () => {
  expect(parseWithIfTransform('<div v-if="ok" />')).toBeTruthy()
})
"#,
        )
        .unwrap();
        fs::write(
            transforms_tests.join("transformExpressions.spec.ts"),
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
import { PatchFlagNames, PatchFlags } from '../../../shared/src'

function parseWithExpressionTransform(
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

test('placeholder', () => {
  expect(parseWithExpressionTransform('{{ foo }}')).toBeTruthy()
  expect(baseCompile('<div />')).toBeTruthy()
})
"#,
        )
        .unwrap();
        fs::write(
            transforms_tests.join("transformSlotOutlet.spec.ts"),
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
import { transformSlotOutlet } from '../../src/transforms/transformSlotOutlet'

function parseWithSlots(template: string, options: CompilerOptions = {}) {
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

test('placeholder', () => {
  expect(parseWithSlots('<slot />')).toBeTruthy()
})
"#,
        )
        .unwrap();
        fs::write(
            transforms_tests.join("vSlot.spec.ts"),
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
import { transformText } from '../../src/transforms/transformText'

function parseWithSlots(
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

test('placeholder', () => {
  expect(parseWithSlots('<Comp><div/></Comp>')).toBeTruthy()
})
"#,
        )
        .unwrap();
        fs::write(
            transforms_tests.join("cacheStatic.spec.ts"),
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
import { PatchFlags } from '@vue/shared'

function transformWithCache(template: string, options: CompilerOptions = {}) {
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

test('placeholder', () => {
  expect(transformWithCache('<div><span/></div>')).toBeTruthy()
})
"#,
        )
        .unwrap();
        fs::write(
            transforms_tests.join("noopDirectiveTransform.spec.ts"),
            r#"import {
  type ElementNode,
  type VNodeCall,
  noopDirectiveTransform,
  baseParse as parse,
  transform,
} from '../../src'
import { transformElement } from '../../src/transforms/transformElement'

test('placeholder', () => {
    const ast = parse(`<div v-noop/>`)
    transform(ast, {
      nodeTransforms: [transformElement],
      directiveTransforms: {
        noop: noopDirectiveTransform,
      },
    })
    const node = ast.children[0] as ElementNode
    expect((node.codegenNode as VNodeCall).props).toBeUndefined()
})
"#,
        )
        .unwrap();
        write_vue3_core_conformance_shims(&temp).unwrap();

        let parser = fs::read_to_string(
            temp.join("packages")
                .join("compiler-core")
                .join("src")
                .join("parser.ts"),
        )
        .unwrap();
        assert!(parser.contains("export * from \"@vue/compiler-core\""));

        let config = fs::read_to_string(temp.join("vitest.config.ts")).unwrap();
        assert!(!config.contains("vitest/config"));
        assert!(config.contains("include: ['packages/compiler-core/__tests__/**/*.spec.ts']"));
        let v_if = fs::read_to_string(
            temp.join("packages")
                .join("compiler-core")
                .join("src")
                .join("transforms")
                .join("vIf.ts"),
        )
        .unwrap();
        assert!(v_if.contains("__vuecRuntime"));
        assert!(v_if.contains("transformIf"));
        let v_once_spec = fs::read_to_string(transforms_tests.join("vOnce.spec.ts")).unwrap();
        assert!(v_once_spec.contains("from './vOnce.rust-api'"));
        assert!(!v_once_spec.contains("getBaseTransformPreset"));
        let v_once_api = fs::read_to_string(transforms_tests.join("vOnce.rust-api.ts")).unwrap();
        assert!(v_once_api.contains("callBridge('vue3.core.transformOnceSuite'"));
        assert!(v_once_api.contains("hydrateTransformOnceAst"));
        let v_bind_spec = fs::read_to_string(transforms_tests.join("vBind.spec.ts")).unwrap();
        assert!(v_bind_spec.contains("from './vBind.rust-api'"));
        assert!(!v_bind_spec.contains("baseParse as parse"));
        assert!(!v_bind_spec.contains("transform(ast,"));
        assert!(!v_bind_spec.contains("transformBind } from '../../src/transforms/vBind'"));
        assert!(!v_bind_spec.contains("transformVBindShorthand"));
        let v_bind_api = fs::read_to_string(transforms_tests.join("vBind.rust-api.ts")).unwrap();
        assert!(v_bind_api.contains("callBridge('vue3.core.transformBindSuite'"));
        assert!(v_bind_api.contains("emitErrors"));
        assert!(v_bind_api.contains("hydrateVBindAst"));
        assert!(v_bind_api.contains("__vuecBrowser"));
        let v_model_spec = fs::read_to_string(transforms_tests.join("vModel.spec.ts")).unwrap();
        assert!(v_model_spec.contains("from './vModel.rust-api'"));
        assert!(!v_model_spec.contains("baseParse as parse"));
        assert!(!v_model_spec.contains("transform(ast,"));
        assert!(!v_model_spec.contains("transformModel } from '../../src/transforms/vModel'"));
        assert!(!v_model_spec.contains("trackSlotScopes"));
        let v_model_api = fs::read_to_string(transforms_tests.join("vModel.rust-api.ts")).unwrap();
        assert!(v_model_api.contains("callBridge('vue3.core.transformModelSuite'"));
        assert!(v_model_api.contains("emitErrors"));
        assert!(v_model_api.contains("hydrateVModelAst"));
        let v_on_spec = fs::read_to_string(transforms_tests.join("vOn.spec.ts")).unwrap();
        assert!(v_on_spec.contains("from './vOn.rust-api'"));
        assert!(!v_on_spec.contains("baseParse as parse"));
        assert!(!v_on_spec.contains("transform(ast,"));
        assert!(!v_on_spec.contains("transformOn } from '../../src/transforms/vOn'"));
        let v_on_api = fs::read_to_string(transforms_tests.join("vOn.rust-api.ts")).unwrap();
        assert!(v_on_api.contains("callBridge('vue3.core.transformOnSuite'"));
        assert!(v_on_api.contains("emitErrors"));
        assert!(v_on_api.contains("hydrateVOnAst"));
        assert!(v_on_api.contains("__vuecNativeTags"));
        assert!(v_on_api.contains("delete node.patchFlag"));
        let v_for_spec = fs::read_to_string(transforms_tests.join("vFor.spec.ts")).unwrap();
        assert!(v_for_spec.contains("from './vFor.rust-api'"));
        assert!(v_for_spec.contains("export { parseWithForTransform }"));
        assert!(!v_for_spec.contains("baseParse as parse"));
        assert!(!v_for_spec.contains("transform(ast,"));
        assert!(!v_for_spec.contains("transformFor } from '../../src/transforms/vFor'"));
        assert!(!v_for_spec.contains("transformVBindShorthand"));
        let v_for_api = fs::read_to_string(transforms_tests.join("vFor.rust-api.ts")).unwrap();
        assert!(v_for_api.contains("callBridge('vue3.core.transformForSuite'"));
        assert!(v_for_api.contains("emitErrors"));
        assert!(v_for_api.contains("hydrateVForAst"));
        assert!(v_for_api.contains("node[key] = undefined"));
        let transform_element_spec =
            fs::read_to_string(transforms_tests.join("transformElement.spec.ts")).unwrap();
        assert!(transform_element_spec.contains("from './transformElement.rust-api'"));
        assert!(transform_element_spec.contains("from './vFor.rust-api'"));
        assert!(transform_element_spec.contains("parseWithElementTransformOriginal"));
        assert!(!transform_element_spec.contains("from './vFor.spec'"));
        assert!(!transform_snapshots
            .join("transformElement.spec.ts.snap")
            .exists());
        let transform_element_api =
            fs::read_to_string(transforms_tests.join("transformElement.rust-api.ts")).unwrap();
        assert!(transform_element_api.contains("callBridge('vue3.core.transformElementSuite'"));
        assert!(transform_element_api.contains("emitErrors"));
        assert!(transform_element_api.contains("hydrateTransformElementAst"));
        assert!(transform_element_api.contains("__vuecNativeTags"));
        let noop_spec =
            fs::read_to_string(transforms_tests.join("noopDirectiveTransform.spec.ts")).unwrap();
        assert!(noop_spec.contains("from './noopDirectiveTransform.rust-api'"));
        assert!(noop_spec.contains("parseWithNoopDirectiveTransform"));
        assert!(!noop_spec.contains("baseParse as parse"));
        assert!(!noop_spec.contains("transform(ast,"));
        assert!(!noop_spec.contains("noop: noopDirectiveTransform"));
        let noop_api =
            fs::read_to_string(transforms_tests.join("noopDirectiveTransform.rust-api.ts"))
                .unwrap();
        assert!(noop_api.contains("callBridge('vue3.core.transformElementSuite'"));
        assert!(noop_api.contains("noopDirectiveTransforms: ['noop']"));
        let v_if_spec = fs::read_to_string(transforms_tests.join("vIf.spec.ts")).unwrap();
        assert!(v_if_spec.contains("from './vIf.rust-api'"));
        assert!(!v_if_spec.contains("baseParse as parse"));
        assert!(!v_if_spec.contains("transform(ast,"));
        assert!(!v_if_spec.contains("transformIf } from '../../src/transforms/vIf'"));
        assert!(!v_if_spec.contains("transformVBindShorthand"));
        let v_if_api = fs::read_to_string(transforms_tests.join("vIf.rust-api.ts")).unwrap();
        assert!(v_if_api.contains("callBridge('vue3.core.transformIfSuite'"));
        assert!(v_if_api.contains("emitErrors"));
        assert!(v_if_api.contains("hydrateVIfAst"));
        assert!(v_if_api.contains("delete node.condition"));
        let expressions_spec =
            fs::read_to_string(transforms_tests.join("transformExpressions.spec.ts")).unwrap();
        assert!(expressions_spec.contains("from './transformExpressions.rust-api'"));
        assert!(!expressions_spec.contains("baseParse as parse"));
        assert!(!expressions_spec.contains("transform(ast,"));
        assert!(!expressions_spec
            .contains("transformExpression } from '../../src/transforms/transformExpression'"));
        assert!(!expressions_spec.contains("transformIf"));
        let expressions_api =
            fs::read_to_string(transforms_tests.join("transformExpressions.rust-api.ts")).unwrap();
        assert!(expressions_api.contains("callBridge('vue3.core.transformExpressionSuite'"));
        assert!(expressions_api.contains("new SyntaxError"));
        assert!(expressions_api.contains("hydrateTransformExpressionsAst"));
        let slot_spec =
            fs::read_to_string(transforms_tests.join("transformSlotOutlet.spec.ts")).unwrap();
        assert!(slot_spec.contains("from './transformSlotOutlet.rust-api'"));
        assert!(!slot_spec.contains("baseParse as parse"));
        assert!(!slot_spec.contains("transform(ast,"));
        assert!(!slot_spec
            .contains("transformSlotOutlet } from '../../src/transforms/transformSlotOutlet'"));
        let slot_api =
            fs::read_to_string(transforms_tests.join("transformSlotOutlet.rust-api.ts")).unwrap();
        assert!(slot_api.contains("callBridge('vue3.core.transformSlotOutletSuite'"));
        assert!(slot_api.contains("emitErrors"));
        assert!(slot_api.contains("hydrateTransformSlotOutletAst"));
        let v_slot_spec = fs::read_to_string(transforms_tests.join("vSlot.spec.ts")).unwrap();
        assert!(v_slot_spec.contains("from './vSlot.rust-api'"));
        assert!(!v_slot_spec.contains("baseParse as parse"));
        assert!(!v_slot_spec.contains("transform(ast,"));
        assert!(!v_slot_spec.contains("trackSlotScopes"));
        assert!(!v_slot_spec.contains("trackVForSlotScopes"));
        assert!(!v_slot_spec.contains("transformSlotOutlet"));
        let v_slot_api = fs::read_to_string(transforms_tests.join("vSlot.rust-api.ts")).unwrap();
        assert!(v_slot_api.contains("callBridge('vue3.core.transformSlotSuite'"));
        assert!(v_slot_api.contains("emitErrors"));
        assert!(v_slot_api.contains("hydrateVSlotAst"));
        assert!(v_slot_api.contains("node.params = undefined"));
        let cache_static_spec =
            fs::read_to_string(transforms_tests.join("cacheStatic.spec.ts")).unwrap();
        assert!(cache_static_spec.contains("from './cacheStatic.rust-api'"));
        assert!(!cache_static_spec.contains("baseParse as parse"));
        assert!(!cache_static_spec.contains("transform(ast,"));
        assert!(!cache_static_spec.contains("../../src/transforms/"));
        let cache_static_api =
            fs::read_to_string(transforms_tests.join("cacheStatic.rust-api.ts")).unwrap();
        assert!(cache_static_api.contains("callBridge('vue3.core.cacheStaticSuite'"));
        assert!(cache_static_api.contains("hydrateCacheStaticAst"));
        assert!(cache_static_api.contains("node.helpers = new Set"));
        assert!(cache_static_api.contains("node[key] = undefined"));
        let _ = fs::remove_dir_all(temp);
    }
