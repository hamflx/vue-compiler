'use strict';

const native = require('@vuec-rs/native');

const locStub = {
  start: { line: 1, column: 1, offset: 0 },
  end: { line: 1, column: 1, offset: 0 },
  source: '',
};

const BindingTypes = {
  DATA: 'data',
  PROPS: 'props',
  PROPS_ALIASED: 'props-aliased',
  SETUP_LET: 'setup-let',
  SETUP_CONST: 'setup-const',
  SETUP_REACTIVE_CONST: 'setup-reactive-const',
  SETUP_MAYBE_REF: 'setup-maybe-ref',
  SETUP_REF: 'setup-ref',
  OPTIONS: 'options',
  LITERAL_CONST: 'literal-const',
};

const CompilerDeprecationTypes = {
  COMPILER_IS_ON_ELEMENT: 'COMPILER_IS_ON_ELEMENT',
  COMPILER_V_BIND_SYNC: 'COMPILER_V_BIND_SYNC',
  COMPILER_V_BIND_OBJECT_ORDER: 'COMPILER_V_BIND_OBJECT_ORDER',
  COMPILER_V_ON_NATIVE: 'COMPILER_V_ON_NATIVE',
  COMPILER_V_IF_V_FOR_PRECEDENCE: 'COMPILER_V_IF_V_FOR_PRECEDENCE',
  COMPILER_NATIVE_TEMPLATE: 'COMPILER_NATIVE_TEMPLATE',
  COMPILER_INLINE_TEMPLATE: 'COMPILER_INLINE_TEMPLATE',
  COMPILER_FILTERS: 'COMPILER_FILTERS',
};

const ConstantTypes = enumObject(['NOT_CONSTANT', 'CAN_SKIP_PATCH', 'CAN_CACHE', 'CAN_STRINGIFY']);
const ElementTypes = enumObject(['ELEMENT', 'COMPONENT', 'SLOT', 'TEMPLATE']);
const Namespaces = enumObject(['HTML', 'SVG', 'MATH_ML']);
const NodeTypes = enumObject([
  'ROOT',
  'ELEMENT',
  'TEXT',
  'COMMENT',
  'SIMPLE_EXPRESSION',
  'INTERPOLATION',
  'ATTRIBUTE',
  'DIRECTIVE',
  'COMPOUND_EXPRESSION',
  'IF',
  'IF_BRANCH',
  'FOR',
  'TEXT_CALL',
  'VNODE_CALL',
  'JS_CALL_EXPRESSION',
  'JS_OBJECT_EXPRESSION',
  'JS_PROPERTY',
  'JS_ARRAY_EXPRESSION',
  'JS_FUNCTION_EXPRESSION',
  'JS_CONDITIONAL_EXPRESSION',
  'JS_CACHE_EXPRESSION',
  'JS_BLOCK_STATEMENT',
  'JS_TEMPLATE_LITERAL',
  'JS_IF_STATEMENT',
  'JS_ASSIGNMENT_EXPRESSION',
  'JS_SEQUENCE_EXPRESSION',
  'JS_RETURN_STATEMENT',
]);
const ErrorCodes = enumObject([
  'ABRUPT_CLOSING_OF_EMPTY_COMMENT',
  'CDATA_IN_HTML_CONTENT',
  'DUPLICATE_ATTRIBUTE',
  'END_TAG_WITH_ATTRIBUTES',
  'END_TAG_WITH_TRAILING_SOLIDUS',
  'EOF_BEFORE_TAG_NAME',
  'EOF_IN_CDATA',
  'EOF_IN_COMMENT',
  'EOF_IN_SCRIPT_HTML_COMMENT_LIKE_TEXT',
  'EOF_IN_TAG',
  'INCORRECTLY_CLOSED_COMMENT',
  'INCORRECTLY_OPENED_COMMENT',
  'INVALID_FIRST_CHARACTER_OF_TAG_NAME',
  'MISSING_ATTRIBUTE_VALUE',
  'MISSING_END_TAG_NAME',
  'MISSING_WHITESPACE_BETWEEN_ATTRIBUTES',
  'NESTED_COMMENT',
  'UNEXPECTED_CHARACTER_IN_ATTRIBUTE_NAME',
  'UNEXPECTED_CHARACTER_IN_UNQUOTED_ATTRIBUTE_VALUE',
  'UNEXPECTED_EQUALS_SIGN_BEFORE_ATTRIBUTE_NAME',
  'UNEXPECTED_NULL_CHARACTER',
  'UNEXPECTED_QUESTION_MARK_INSTEAD_OF_TAG_NAME',
  'UNEXPECTED_SOLIDUS_IN_TAG',
  'X_INVALID_END_TAG',
  'X_MISSING_END_TAG',
  'X_MISSING_INTERPOLATION_END',
  'X_MISSING_DIRECTIVE_NAME',
  'X_MISSING_DYNAMIC_DIRECTIVE_ARGUMENT_END',
  'X_V_IF_NO_EXPRESSION',
  'X_V_IF_SAME_KEY',
  'X_V_ELSE_NO_ADJACENT_IF',
  'X_V_FOR_NO_EXPRESSION',
  'X_V_FOR_MALFORMED_EXPRESSION',
  'X_V_FOR_TEMPLATE_KEY_PLACEMENT',
  'X_V_BIND_NO_EXPRESSION',
  'X_V_ON_NO_EXPRESSION',
  'X_V_SLOT_UNEXPECTED_DIRECTIVE_ON_SLOT_OUTLET',
  'X_V_SLOT_MIXED_SLOT_USAGE',
  'X_V_SLOT_DUPLICATE_SLOT_NAMES',
  'X_V_SLOT_EXTRANEOUS_DEFAULT_SLOT_CHILDREN',
  'X_V_SLOT_MISPLACED',
  'X_V_MODEL_NO_EXPRESSION',
  'X_V_MODEL_MALFORMED_EXPRESSION',
  'X_V_MODEL_ON_SCOPE_VARIABLE',
  'X_V_MODEL_ON_PROPS',
  'X_V_MODEL_ON_CONST',
  'X_INVALID_EXPRESSION',
  'X_KEEP_ALIVE_INVALID_CHILDREN',
  'X_PREFIX_ID_NOT_SUPPORTED',
  'X_MODULE_MODE_NOT_SUPPORTED',
  'X_CACHE_HANDLER_NOT_SUPPORTED',
  'X_SCOPE_ID_NOT_SUPPORTED',
  'X_VNODE_HOOKS',
  'X_V_BIND_INVALID_SAME_NAME_ARGUMENT',
  '__EXTEND_POINT__',
]);

const errorMessages = {};
for (let i = 0; i <= 54; i += 1) {
  errorMessages[i] = '';
}

const TS_NODE_TYPES = [
  'TSAsExpression',
  'TSTypeAssertion',
  'TSNonNullExpression',
  'TSInstantiationExpression',
  'TSSatisfiesExpression',
];

const forAliasRE = /([\s\S]*?)\s+(?:in|of)\s+(\S[\s\S]*)/;
const validFirstIdentCharRE = /[A-Za-z_$\xA0-\uFFFF]/;

const helperNameMap = {};
const helperSymbols = {};
for (const [exportName, helperName] of [
  ['FRAGMENT', 'Fragment'],
  ['TELEPORT', 'Teleport'],
  ['SUSPENSE', 'Suspense'],
  ['KEEP_ALIVE', 'KeepAlive'],
  ['BASE_TRANSITION', 'BaseTransition'],
  ['OPEN_BLOCK', 'openBlock'],
  ['CREATE_BLOCK', 'createBlock'],
  ['CREATE_ELEMENT_BLOCK', 'createElementBlock'],
  ['CREATE_VNODE', 'createVNode'],
  ['CREATE_ELEMENT_VNODE', 'createElementVNode'],
  ['CREATE_COMMENT', 'createCommentVNode'],
  ['CREATE_TEXT', 'createTextVNode'],
  ['CREATE_STATIC', 'createStaticVNode'],
  ['RESOLVE_COMPONENT', 'resolveComponent'],
  ['RESOLVE_DYNAMIC_COMPONENT', 'resolveDynamicComponent'],
  ['RESOLVE_DIRECTIVE', 'resolveDirective'],
  ['RESOLVE_FILTER', 'resolveFilter'],
  ['WITH_DIRECTIVES', 'withDirectives'],
  ['RENDER_LIST', 'renderList'],
  ['RENDER_SLOT', 'renderSlot'],
  ['CREATE_SLOTS', 'createSlots'],
  ['TO_DISPLAY_STRING', 'toDisplayString'],
  ['MERGE_PROPS', 'mergeProps'],
  ['NORMALIZE_CLASS', 'normalizeClass'],
  ['NORMALIZE_STYLE', 'normalizeStyle'],
  ['NORMALIZE_PROPS', 'normalizeProps'],
  ['GUARD_REACTIVE_PROPS', 'guardReactiveProps'],
  ['TO_HANDLERS', 'toHandlers'],
  ['CAMELIZE', 'camelize'],
  ['CAPITALIZE', 'capitalize'],
  ['TO_HANDLER_KEY', 'toHandlerKey'],
  ['SET_BLOCK_TRACKING', 'setBlockTracking'],
  ['PUSH_SCOPE_ID', 'pushScopeId'],
  ['POP_SCOPE_ID', 'popScopeId'],
  ['WITH_CTX', 'withCtx'],
  ['UNREF', 'unref'],
  ['IS_REF', 'isRef'],
  ['WITH_MEMO', 'withMemo'],
  ['IS_MEMO_SAME', 'isMemoSame'],
]) {
  const symbol = Symbol(helperName);
  helperSymbols[exportName] = symbol;
  helperNameMap[symbol] = helperName;
}

function enumObject(names) {
  const out = {};
  names.forEach((name, index) => {
    out[index] = name;
    out[name] = index;
  });
  return out;
}

function normalizeOptions(options) {
  return options || {};
}

function baseCompile(source) {
  return native.baseCompileVue3(String(source || ''), normalizeOptions(arguments[1]));
}

function compile(source, options) {
  return baseCompile(source, options);
}

function baseParse(content, options) {
  return native.baseParseVue3(String(content || ''), normalizeOptions(options));
}

function parse(content, options) {
  return baseParse(content, options);
}

function generate(ast) {
  return native.generateVue3Core(ast || {}, normalizeOptions(arguments[1]));
}

function generateCodeFrame(source) {
  return native.generateCodeFrameVue2(String(source || ''), Number(arguments[1]) || 0, Number(arguments[2]) || 0);
}

function advancePositionWithClone(pos, source) {
  const next = { ...(pos || {}) };
  return advancePositionWithMutation(next, source, arguments.length > 2 ? arguments[2] : undefined);
}

function advancePositionWithMutation(pos, source) {
  const target = pos || { offset: 0, line: 1, column: 1 };
  const text = String(source || '');
  const count = arguments.length > 2 && arguments[2] != null ? Number(arguments[2]) : text.length;
  const slice = Array.from(text).slice(0, count).join('');
  target.offset = Number(target.offset || 0) + count;
  for (const ch of slice) {
    if (ch === '\n') {
      target.line = Number(target.line || 1) + 1;
      target.column = 1;
    } else {
      target.column = Number(target.column || 1) + 1;
    }
  }
  if (!target.line) target.line = 1;
  if (!target.column) target.column = 1;
  return target;
}

function assert(condition, msg) {
  if (!condition) {
    throw new Error(msg || 'unexpected compiler condition');
  }
}

function createArrayExpression(elements) {
  return { type: 17, loc: locStub, elements: elements || [] };
}

function createAssignmentExpression(left, right) {
  return { type: 24, loc: locStub, left, right };
}

function createBlockStatement(body) {
  return { type: 21, loc: locStub, body: body || [] };
}

function createCacheExpression(index, value) {
  return { type: 20, loc: locStub, index, value, isVNode: false };
}

function createCallExpression(callee) {
  return { type: 14, loc: locStub, callee, arguments: Array.prototype.slice.call(arguments, 1).flat() };
}

function createCompilerError(code, loc, messages, additionalMessage) {
  const error = new SyntaxError(String((messages && messages[code]) || errorMessages[code] || code) + (additionalMessage || ''));
  error.code = code;
  error.loc = loc || locStub;
  return error;
}

function createCompoundExpression(children) {
  return { type: 8, loc: locStub, children: children || [] };
}

function createConditionalExpression(test, consequent, alternate) {
  return { type: 19, loc: locStub, test, consequent, alternate, newline: true };
}

function createForLoopParams(value) {
  const ret = [];
  if (value && value.value) ret.push(value.value);
  if (value && value.key) ret.push(value.key);
  if (value && value.index) ret.push(value.index);
  return ret;
}

function createFunctionExpression(params) {
  return { type: 18, loc: locStub, params: params || [], returns: undefined, body: undefined, newline: false, isSlot: false };
}

function createIfStatement(test, consequent, alternate) {
  return { type: 23, loc: locStub, test, consequent, alternate };
}

function createInterpolation(content, loc) {
  return {
    type: 5,
    content: typeof content === 'string' ? createSimpleExpression(content, false) : content,
    loc: loc || locStub,
  };
}

function createObjectExpression(properties) {
  return { type: 15, loc: locStub, properties: properties || [] };
}

function createObjectProperty(key, value) {
  return { type: 16, loc: locStub, key: typeof key === 'string' ? createSimpleExpression(key, true) : key, value };
}

function createReturnStatement(returns) {
  return { type: 26, loc: locStub, returns };
}

function createRoot(children) {
  return {
    type: 0,
    source: '',
    children: children || [],
    helpers: new Set(),
    components: [],
    directives: [],
    hoists: [],
    imports: [],
    cached: [],
    temps: 0,
    codegenNode: null,
    loc: locStub,
  };
}

function createSequenceExpression(expressions) {
  return { type: 25, loc: locStub, expressions: expressions || [] };
}

function createSimpleExpression(content) {
  const isStatic = arguments.length > 1 ? !!arguments[1] : false;
  return {
    type: 4,
    loc: arguments[2] || locStub,
    content: String(content == null ? '' : content),
    isStatic,
    constType: isStatic ? ConstantTypes.CAN_STRINGIFY : ConstantTypes.NOT_CONSTANT,
  };
}

function createStructuralDirectiveTransform(name, fn) {
  const matches = typeof name === 'string' ? dir => dir.name === name : dir => name.test(dir.name);
  return (node, context) => {
    const props = node && Array.isArray(node.props) ? node.props : [];
    const exits = [];
    for (const prop of props) {
      if (prop && prop.type === NodeTypes.DIRECTIVE && matches(prop)) {
        const onExit = fn(node, prop, context);
        if (onExit) exits.push(onExit);
      }
    }
    return exits;
  };
}

function createTemplateLiteral(elements) {
  return { type: 22, loc: locStub, elements: elements || [] };
}

function createTransformContext(root, options) {
  return {
    root,
    options: options || {},
    helpers: new Map(),
    components: new Set(),
    directives: new Set(),
    hoists: [],
    imports: [],
    temps: 0,
    cached: 0,
    identifiers: Object.create(null),
    scopes: { vFor: 0, vSlot: 0, vPre: 0, vOnce: 0 },
    helper(name) {
      this.helpers.set(name, (this.helpers.get(name) || 0) + 1);
      return name;
    },
    removeHelper(name) {
      const count = this.helpers.get(name);
      if (count > 1) this.helpers.set(name, count - 1);
      else this.helpers.delete(name);
    },
    helperString(name) {
      return `_${helperNameMap[name] || String(name).replace(/^Symbol\((.*)\)$/, '$1')}`;
    },
    replaceNode(node) {
      this.currentNode = node;
    },
    removeNode() {
      this.currentNode = null;
    },
    onError(error) {
      if (options && typeof options.onError === 'function') options.onError(error);
      else throw error;
    },
    onWarn(warning) {
      if (options && typeof options.onWarn === 'function') options.onWarn(warning);
    },
  };
}

function createVNodeCall(context, tag, props, children, patchFlag, dynamicProps, directives) {
  if (context && typeof context.helper === 'function') {
    context.helper(arguments.length > 7 && arguments[7] ? CREATE_BLOCK : CREATE_VNODE);
  }
  return {
    type: 13,
    loc: arguments[10] || locStub,
    tag,
    props,
    children,
    patchFlag,
    dynamicProps,
    directives,
    isBlock: !!(arguments.length > 7 && arguments[7]),
    disableTracking: !!(arguments.length > 8 && arguments[8]),
    isComponent: !!(arguments.length > 9 && arguments[9]),
  };
}

function extractIdentifiers(param) {
  const out = [];
  collectIdentifiers(param, out);
  return out;
}

function findDir(node, name) {
  const allowEmpty = arguments.length > 2 ? arguments[2] : false;
  return (node && node.props || []).find(prop => prop && prop.type === 7
    && (typeof name === 'string' ? prop.name === name : name.test(prop.name))
    && (allowEmpty || prop.exp));
}

function findProp(node, name) {
  const dynamicOnly = arguments.length > 2 ? arguments[2] : false;
  const allowEmpty = arguments.length > 3 ? arguments[3] : false;
  for (const prop of node && node.props || []) {
    if (!prop) continue;
    if (prop.type === 6 && prop.name === name && !dynamicOnly) return prop;
    if (prop.type === 7 && prop.name === 'bind' && prop.arg && prop.arg.content === name && (allowEmpty || prop.exp)) {
      return prop;
    }
  }
  return undefined;
}

function getBaseTransformPreset(prefixIdentifiers) {
  return [[], {}];
}

function getConstantType(node, context) {
  if (!node) return ConstantTypes.NOT_CONSTANT;
  if (node.constType != null) return node.constType;
  if (node.type === NodeTypes.SIMPLE_EXPRESSION) {
    return node.isStatic ? ConstantTypes.CAN_STRINGIFY : ConstantTypes.NOT_CONSTANT;
  }
  return context && context.constantCache && context.constantCache.get
    ? context.constantCache.get(node) || ConstantTypes.NOT_CONSTANT
    : ConstantTypes.NOT_CONSTANT;
}

function getMemoedVNodeCall(node) {
  return node && node.type === NodeTypes.JS_CACHE_EXPRESSION ? node.value : node;
}

function getVNodeBlockHelper(ssr, isComponent) {
  return isComponent ? CREATE_BLOCK : CREATE_ELEMENT_BLOCK;
}

function getVNodeHelper(ssr, isComponent) {
  return isComponent ? CREATE_VNODE : CREATE_ELEMENT_VNODE;
}

function hasDynamicKeyVBind(node) {
  return !!(node && node.props || []).find(prop => prop && prop.type === 7 && prop.name === 'bind' && prop.arg && !prop.arg.isStatic);
}

function hasScopeRef(node, ids) {
  const known = ids || {};
  let found = false;
  walkNode(node, child => {
    if (child && child.type === NodeTypes.SIMPLE_EXPRESSION && !child.isStatic && known[child.content]) {
      found = true;
    }
  });
  return found;
}

function injectProp(node, prop, context) {
  if (!node) return node;
  if (!node.props) node.props = createObjectExpression([]);
  if (Array.isArray(node.props.properties)) node.props.properties.unshift(prop);
  return node;
}

function isAllWhitespace(source) {
  return !/[^\t\r\n\f ]/.test(String(source || ''));
}

function isCommentOrWhitespace(node) {
  return !!node && (node.type === NodeTypes.COMMENT || isWhitespaceText(node));
}

function isCoreComponent(tag) {
  const name = String(tag || '');
  if (name === 'Teleport') return TELEPORT;
  if (name === 'Suspense') return SUSPENSE;
  if (name === 'KeepAlive') return KEEP_ALIVE;
  if (name === 'BaseTransition') return BASE_TRANSITION;
  return undefined;
}

const isFnExpressionNode = (exp, context) => {
  const source = typeof exp === 'string' ? exp : exp && exp.content;
  return /^\s*(async\s*)?(\([^)]*\)|[A-Za-z_$][\w$]*)\s*=>/.test(String(source || ''))
    || /^\s*function\b/.test(String(source || ''));
};

const isFnExpressionBrowser = (exp) => {
  return isFnExpressionNode(exp, null);
};

const isFnExpression = isFnExpressionNode;

const isFunctionType = (node) => {
  const type = node && node.type;
  return typeof type === 'string' && (type.endsWith('FunctionExpression') || type === 'FunctionDeclaration' || type.endsWith('Method'));
};

function isInDestructureAssignment(parent, parentStack) {
  return Array.isArray(parentStack) && parentStack.some(node => node && node.type === 'AssignmentExpression')
    && !!parent && /(?:Property|Pattern)$/.test(String(parent.type || ''));
}

function isInNewExpression(parentStack) {
  return Array.isArray(parentStack) && parentStack.some(node => node && node.type === 'NewExpression');
}

const isMemberExpressionNode = (path, context) => {
  return isMemberExpressionSource(typeof path === 'string' ? path : path && path.content);
};

const isMemberExpressionBrowser = (path) => {
  return isMemberExpressionNode(path, null);
};

const isMemberExpression = isMemberExpressionNode;

function isReferencedIdentifier(id, parent, parentStack) {
  if (!id || id.type !== 'Identifier') return false;
  if (!parent) return true;
  const relation = arguments.length > 3 ? arguments[3] : undefined;
  if (parent.type === 'MemberExpression') return relation === 'object' || !!parent.computed;
  if (parent.type === 'ObjectProperty' || parent.type === 'Property') return relation !== 'key' || !!parent.computed;
  return !/^(FunctionDeclaration|FunctionExpression|ImportSpecifier|ImportDefaultSpecifier|ImportNamespaceSpecifier)$/.test(parent.type);
}

const isSimpleIdentifier = (name) => {
  return /^[A-Za-z_$][\w$]*$/.test(String(name || ''));
};

function isSlotOutlet(node) {
  return !!node && node.type === NodeTypes.ELEMENT && node.tagType === ElementTypes.SLOT;
}

function isStaticArgOf(arg, name) {
  return !!arg && arg.type === NodeTypes.SIMPLE_EXPRESSION && arg.isStatic && arg.content === name;
}

const isStaticExp = (p) => {
  return !!p && p.type === NodeTypes.SIMPLE_EXPRESSION && !!p.isStatic;
};

const isStaticProperty = (node) => {
  return !!node && /^(ObjectProperty|ObjectMethod|Property)$/.test(String(node.type || '')) && !node.computed;
};

const isStaticPropertyKey = (node, parent) => {
  return !!parent && isStaticProperty(parent) && parent.key === node;
};

function isTemplateNode(node) {
  return !!node && node.type === NodeTypes.ELEMENT && node.tagType === ElementTypes.TEMPLATE;
}

function isText$1(node) {
  return !!node && (node.type === NodeTypes.INTERPOLATION || node.type === NodeTypes.TEXT || node.type === NodeTypes.COMPOUND_EXPRESSION);
}

function isVPre(node) {
  return !!(node && node.props || []).find(prop => prop && prop.type === 7 && prop.name === 'pre');
}

function isVSlot(prop) {
  return !!prop && prop.type === 7 && prop.name === 'slot';
}

function isWhitespaceText(node) {
  return !!node && node.type === NodeTypes.TEXT && isAllWhitespace(node.content);
}

const noopDirectiveTransform = () => {
  return { props: [] };
};

function processExpression(node, context) {
  return node;
}

function processFor(node, dir, context, processCodegen) {
  if (typeof processCodegen === 'function') return processCodegen(node);
  return undefined;
}

function processIf(node, dir, context, processCodegen) {
  if (typeof processCodegen === 'function') return processCodegen(node, dir);
  return undefined;
}

function processSlotOutlet(node, context) {
  return { slotName: '"default"', slotProps: undefined };
}

function registerRuntimeHelpers(helpers) {
  for (const key of Object.keys(helpers || {})) {
    helperNameMap[key] = helpers[key];
  }
}

function resolveComponentType(node, context) {
  return node && node.tag;
}

function stringifyExpression(exp) {
  if (typeof exp === 'string') return exp;
  if (exp && typeof exp.content === 'string') return exp.content;
  return '';
}

function toValidAssetId(name, type) {
  return `_${type}_${String(name || '').replace(/[^\w$]/g, '_')}`;
}

const trackSlotScopes = (node, context) => {
  return undefined;
};

const trackVForSlotScopes = (node, context) => {
  return undefined;
};

function transform(root, options) {
  root.helpers = root.helpers || new Set();
  root.components = root.components || [];
  root.directives = root.directives || [];
  root.hoists = root.hoists || [];
  root.imports = root.imports || [];
  root.cached = root.cached || [];
  root.temps = root.temps || 0;
  root.transformed = true;
}

const transformBind = (dir, node, context) => {
  return { props: dir && dir.arg ? [createObjectProperty(dir.arg, dir.exp || createSimpleExpression('', true))] : [] };
};

const transformElement = (node, context) => {
  return undefined;
};

const transformExpression = (node, context) => {
  return node;
};

const transformModel = (dir, node, context) => {
  return { props: dir && dir.exp ? [createObjectProperty('modelValue', dir.exp)] : [] };
};

const transformOn = (dir, node, context, augmentor) => {
  return { props: dir && dir.arg ? [createObjectProperty(dir.arg, dir.exp || createSimpleExpression('() => {}', false))] : [] };
};

const transformVBindShorthand = (dir, context) => {
  return transformBind(dir, null, context);
};

function traverseNode(node, context) {
  if (!node) return;
  for (const child of node.children || []) traverseNode(child, context);
}

function unwrapTSNode(node) {
  let current = node;
  while (current && TS_NODE_TYPES.includes(current.type)) {
    current = current.expression;
  }
  return current;
}

function walkBlockDeclarations(block, onIdent) {
  walkNode(block, node => {
    if (node && node.type === 'VariableDeclarator') collectIdentifiers(node.id, onIdent);
  });
}

function walkFunctionParams(node, onIdent) {
  for (const param of node && node.params || []) collectIdentifiers(param, onIdent);
}

function walkIdentifiers(root, onIdentifier) {
  walkNode(root, (node, parent) => {
    if (node && node.type === 'Identifier') onIdentifier(node, parent || null, []);
  });
}

function warnDeprecation(key, context, loc) {
  const warning = { key, loc: loc || locStub };
  if (context && typeof context.onWarn === 'function') context.onWarn(warning);
}

function checkCompatEnabled(key, context, loc) {
  return !!(context && context.compatConfig && context.compatConfig[key]);
}

function buildDirectiveArgs(dir, context) {
  return [dir && dir.exp, dir && dir.arg, dir && dir.modifiers].filter(Boolean);
}

function buildProps(node, context) {
  return { props: undefined, directives: [], patchFlag: 0, dynamicPropNames: [], shouldUseBlock: false };
}

function buildSlots(node, context) {
  return { slots: createObjectExpression([]), hasDynamicSlots: false };
}

function convertToBlock(node, context) {
  if (node) node.isBlock = true;
  return node;
}

function collectIdentifiers(node, out) {
  const push = typeof out === 'function' ? out : value => out.push(value);
  if (!node) return;
  if (Array.isArray(node)) {
    node.forEach(item => collectIdentifiers(item, out));
    return;
  }
  if (node.type === 'Identifier') {
    push(node);
    return;
  }
  if (node.type === 'ObjectPattern') {
    for (const prop of node.properties || []) collectIdentifiers(prop.value || prop.argument || prop, out);
    return;
  }
  if (node.type === 'ArrayPattern') {
    for (const element of node.elements || []) collectIdentifiers(element, out);
    return;
  }
  if (node.type === 'RestElement') {
    collectIdentifiers(node.argument, out);
    return;
  }
  if (node.type === 'AssignmentPattern') {
    collectIdentifiers(node.left, out);
  }
}

function walkNode(node, enter, parent) {
  if (!node || typeof node !== 'object') return;
  enter(node, parent);
  for (const key of Object.keys(node)) {
    if (key === 'loc' || key === 'parent') continue;
    const value = node[key];
    if (Array.isArray(value)) {
      for (const item of value) walkNode(item, enter, node);
    } else if (value && typeof value === 'object') {
      walkNode(value, enter, node);
    }
  }
}

function isMemberExpressionSource(source) {
  return /^[A-Za-z_$][\w$]*(?:\s*(?:\.|\[).+)?$/.test(String(source || '').trim());
}

const {
  BASE_TRANSITION,
  CAMELIZE,
  CAPITALIZE,
  CREATE_BLOCK,
  CREATE_COMMENT,
  CREATE_ELEMENT_BLOCK,
  CREATE_ELEMENT_VNODE,
  CREATE_SLOTS,
  CREATE_STATIC,
  CREATE_TEXT,
  CREATE_VNODE,
  FRAGMENT,
  GUARD_REACTIVE_PROPS,
  IS_MEMO_SAME,
  IS_REF,
  KEEP_ALIVE,
  MERGE_PROPS,
  NORMALIZE_CLASS,
  NORMALIZE_PROPS,
  NORMALIZE_STYLE,
  OPEN_BLOCK,
  POP_SCOPE_ID,
  PUSH_SCOPE_ID,
  RENDER_LIST,
  RENDER_SLOT,
  RESOLVE_COMPONENT,
  RESOLVE_DIRECTIVE,
  RESOLVE_DYNAMIC_COMPONENT,
  RESOLVE_FILTER,
  SET_BLOCK_TRACKING,
  SUSPENSE,
  TELEPORT,
  TO_DISPLAY_STRING,
  TO_HANDLERS,
  TO_HANDLER_KEY,
  UNREF,
  WITH_CTX,
  WITH_DIRECTIVES,
  WITH_MEMO,
} = helperSymbols;

module.exports = {
  BASE_TRANSITION,
  BindingTypes,
  CAMELIZE,
  CAPITALIZE,
  CREATE_BLOCK,
  CREATE_COMMENT,
  CREATE_ELEMENT_BLOCK,
  CREATE_ELEMENT_VNODE,
  CREATE_SLOTS,
  CREATE_STATIC,
  CREATE_TEXT,
  CREATE_VNODE,
  CompilerDeprecationTypes,
  ConstantTypes,
  ElementTypes,
  ErrorCodes,
  FRAGMENT,
  GUARD_REACTIVE_PROPS,
  IS_MEMO_SAME,
  IS_REF,
  KEEP_ALIVE,
  MERGE_PROPS,
  NORMALIZE_CLASS,
  NORMALIZE_PROPS,
  NORMALIZE_STYLE,
  Namespaces,
  NodeTypes,
  OPEN_BLOCK,
  POP_SCOPE_ID,
  PUSH_SCOPE_ID,
  RENDER_LIST,
  RENDER_SLOT,
  RESOLVE_COMPONENT,
  RESOLVE_DIRECTIVE,
  RESOLVE_DYNAMIC_COMPONENT,
  RESOLVE_FILTER,
  SET_BLOCK_TRACKING,
  SUSPENSE,
  TELEPORT,
  TO_DISPLAY_STRING,
  TO_HANDLERS,
  TO_HANDLER_KEY,
  TS_NODE_TYPES,
  UNREF,
  WITH_CTX,
  WITH_DIRECTIVES,
  WITH_MEMO,
  advancePositionWithClone,
  advancePositionWithMutation,
  assert,
  baseCompile,
  baseParse,
  buildDirectiveArgs,
  buildProps,
  buildSlots,
  checkCompatEnabled,
  convertToBlock,
  createArrayExpression,
  createAssignmentExpression,
  createBlockStatement,
  createCacheExpression,
  createCallExpression,
  createCompilerError,
  createCompoundExpression,
  createConditionalExpression,
  createForLoopParams,
  createFunctionExpression,
  createIfStatement,
  createInterpolation,
  createObjectExpression,
  createObjectProperty,
  createReturnStatement,
  createRoot,
  createSequenceExpression,
  createSimpleExpression,
  createStructuralDirectiveTransform,
  createTemplateLiteral,
  createTransformContext,
  createVNodeCall,
  errorMessages,
  extractIdentifiers,
  findDir,
  findProp,
  forAliasRE,
  generate,
  generateCodeFrame,
  getBaseTransformPreset,
  getConstantType,
  getMemoedVNodeCall,
  getVNodeBlockHelper,
  getVNodeHelper,
  hasDynamicKeyVBind,
  hasScopeRef,
  helperNameMap,
  injectProp,
  isAllWhitespace,
  isCommentOrWhitespace,
  isCoreComponent,
  isFnExpression,
  isFnExpressionBrowser,
  isFnExpressionNode,
  isFunctionType,
  isInDestructureAssignment,
  isInNewExpression,
  isMemberExpression,
  isMemberExpressionBrowser,
  isMemberExpressionNode,
  isReferencedIdentifier,
  isSimpleIdentifier,
  isSlotOutlet,
  isStaticArgOf,
  isStaticExp,
  isStaticProperty,
  isStaticPropertyKey,
  isTemplateNode,
  isText: isText$1,
  isVPre,
  isVSlot,
  isWhitespaceText,
  locStub,
  noopDirectiveTransform,
  processExpression,
  processFor,
  processIf,
  processSlotOutlet,
  registerRuntimeHelpers,
  resolveComponentType,
  stringifyExpression,
  toValidAssetId,
  trackSlotScopes,
  trackVForSlotScopes,
  transform,
  transformBind,
  transformElement,
  transformExpression,
  transformModel,
  transformOn,
  transformVBindShorthand,
  traverseNode,
  unwrapTSNode,
  validFirstIdentCharRE,
  walkBlockDeclarations,
  walkFunctionParams,
  walkIdentifiers,
  warnDeprecation,
};
