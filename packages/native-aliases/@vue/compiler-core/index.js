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
errorMessages[ErrorCodes.X_CACHE_HANDLER_NOT_SUPPORTED] = '"cacheHandlers" option is only supported when the "prefixIdentifiers" option is enabled.';
errorMessages[ErrorCodes.X_SCOPE_ID_NOT_SUPPORTED] = '"scopeId" option is only supported in module mode.';

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
  const options = normalizeOptions(arguments[1]);
  validateBaseCompileOptions(options);
  return native.baseCompileVue3(String(source || ''), vue3NativeOptions(options, source));
}

function compile(source, options) {
  return baseCompile(source, options);
}

function baseParse(content, options) {
  const source = String(content || '');
  const opts = normalizeOptions(options);
  warnIgnoredDecodeEntities(opts);
  return hydrateVue3Ast(native.baseParseVue3(source, vue3NativeOptions(opts, source)), opts);
}

function parse(content, options) {
  return baseParse(content, options);
}

function generate(ast) {
  return native.generateVue3Core(hydrateVue3Ast(ast || {}, normalizeOptions(arguments[1])), normalizeOptions(arguments[1]));
}

function generateCodeFrame(source) {
  return native.generateCodeFrameVue2(String(source || ''), Number(arguments[1]) || 0, Number(arguments[2]) || 0);
}

function callVue3CoreProjection(command, payload) {
  return native.callVue3CoreProjection(command, payload || {});
}

function helperSymbolFromProjection(name) {
  if (!name) return undefined;
  if (helperSymbols[name]) return helperSymbols[name];
  for (const symbol of Object.values(helperSymbols)) {
    if (helperNameMap[symbol] === name) return symbol;
  }
  return undefined;
}

function validateBaseCompileOptions(options) {
  const isModuleMode = options.mode === 'module';
  const prefixIdentifiers = options.prefixIdentifiers === true || isModuleMode;
  if (!prefixIdentifiers && options.cacheHandlers) {
    throw createCompilerError(ErrorCodes.X_CACHE_HANDLER_NOT_SUPPORTED);
  }
  if (options.scopeId && !isModuleMode) {
    throw createCompilerError(ErrorCodes.X_SCOPE_ID_NOT_SUPPORTED);
  }
}

function vue3NativeOptions(options, source) {
  if (!options || typeof options !== 'object') return {};
  const out = {};
  for (const key of Object.keys(options)) {
    if (typeof options[key] !== 'function') out[key] = options[key];
  }
  const tags = extractVueTemplateTags(String(source || ''));
  if (hasVuePredicateOption(options, 'isVoidTag')) {
    out.__vuecVoidTags = collectVuePredicateHits(options.isVoidTag, tags);
  }
  if (hasVuePredicateOption(options, 'isPreTag')) {
    out.__vuecPreTags = collectVuePredicateHits(options.isPreTag, tags);
  }
  if (hasVuePredicateOption(options, 'isIgnoreNewlineTag')) {
    out.__vuecIgnoreNewlineTags = collectVuePredicateHits(options.isIgnoreNewlineTag, tags);
  }
  if (typeof options.getNamespace === 'function') {
    out.__vuecNamespaces = collectVueNamespaceHits(options.getNamespace, tags);
    out.__vuecDomNamespaces = true;
  }
  if (Object.prototype.hasOwnProperty.call(options, 'ns')) {
    out.__vuecRootNamespace = options.ns;
  }
  if (hasVuePredicateOption(options, 'isNativeTag')) {
    out.__vuecNativeTags = collectVuePredicateHits(options.isNativeTag, tags);
  }
  out.__vuecCustomElements = collectVuePredicateHits(options.isCustomElement, tags);
  out.__vuecBuiltInComponents = collectVuePredicateHits(options.isBuiltInComponent, tags);
  return out;
}

function hasVuePredicateOption(options, name) {
  return Object.prototype.hasOwnProperty.call(options, name) &&
    (typeof options[name] === 'function' || Array.isArray(options[name]));
}

function extractVueTemplateTags(source) {
  const tags = [];
  const seen = new Set();
  const pattern = /<\/?\s*([A-Za-z][A-Za-z0-9._:-]*)/g;
  let match;
  while ((match = pattern.exec(source))) {
    const tag = match[1];
    if (!seen.has(tag)) {
      seen.add(tag);
      tags.push(tag);
    }
  }
  return tags;
}

function collectVuePredicateHits(predicate, values) {
  if (Array.isArray(predicate)) return predicate.map(String);
  if (typeof predicate !== 'function') return [];
  const hits = [];
  for (const value of values) {
    try {
      if (predicate(value)) hits.push(value);
    } catch (_) {}
  }
  return hits;
}

function collectVueNamespaceHits(getNamespace, values) {
  if (!getNamespace || typeof getNamespace !== 'function') return {};
  const namespaces = {};
  for (const value of values) {
    try {
      const namespace = getNamespace(value);
      if (namespace !== undefined && namespace !== null) namespaces[value] = namespace;
    } catch (_) {}
  }
  return namespaces;
}

function warnIgnoredDecodeEntities(options) {
  if (!options || typeof options.decodeEntities !== 'function') return;
  console.warn('[Vue warn]: decodeEntities option is passed but will be ignored in non-browser builds.');
}

function hydrateVue3Ast(ast, options) {
  emitVue3ParseDiagnostics(ast, options);
  hydrateVue3Node(ast);
  return ast;
}

function emitVue3ParseDiagnostics(ast, options) {
  if (!ast || !Array.isArray(ast.__vuecDiagnostics)) return;
  const onError = options && typeof options.onError === 'function'
    ? options.onError
    : error => { throw error; };
  for (const diagnostic of ast.__vuecDiagnostics) {
    const error = new SyntaxError(diagnostic.message || errorMessages[diagnostic.code] || 'Vue compiler parse error');
    error.code = diagnostic.code;
    error.loc = diagnostic.loc;
    onError(error);
  }
  delete ast.__vuecDiagnostics;
}

function hydrateVue3Node(node) {
  if (!node || typeof node !== 'object') return node;
  if (node.type === NodeTypes.ROOT) {
    node.helpers = new Set(node.helpers || []);
    node.components = node.components || [];
    node.directives = node.directives || [];
    node.hoists = node.hoists || [];
    node.imports = node.imports || [];
    node.cached = node.cached || [];
    node.temps = node.temps || 0;
    if (node.codegenNode === null) node.codegenNode = undefined;
  }
  if (node.type === NodeTypes.ELEMENT) {
    if (node.codegenNode === null) node.codegenNode = undefined;
    if (node.isSelfClosing === null) delete node.isSelfClosing;
  }
  if (node.type === NodeTypes.ATTRIBUTE) {
    if (node.value === null) node.value = undefined;
  }
  if (node.type === NodeTypes.DIRECTIVE) {
    if (node.exp === null) node.exp = undefined;
    if (node.arg === null) node.arg = undefined;
  }
  if (Array.isArray(node.children)) node.children.forEach(hydrateVue3Node);
  if (Array.isArray(node.props)) node.props.forEach(hydrateVue3Node);
  if (Array.isArray(node.modifiers)) node.modifiers.forEach(hydrateVue3Node);
  if (node.content && typeof node.content === 'object') hydrateVue3Node(node.content);
  if (node.exp && typeof node.exp === 'object') hydrateVue3Node(node.exp);
  if (node.arg && typeof node.arg === 'object') hydrateVue3Node(node.arg);
  return node;
}

function advancePositionWithClone(pos, source) {
  return callVue3CoreProjection('vue3.core.advancePositionWithClone', {
    pos: pos || {},
    source: String(source || ''),
    numberOfCharacters: arguments.length > 2 ? Number(arguments[2]) : undefined,
  });
}

function advancePositionWithMutation(pos, source) {
  const next = callVue3CoreProjection('vue3.core.advancePositionWithMutation', {
    pos: pos || {},
    source: String(source || ''),
    numberOfCharacters: arguments.length > 2 ? Number(arguments[2]) : undefined,
  });
  if (pos && typeof pos === 'object') {
    pos.offset = next.offset;
    pos.line = next.line;
    pos.column = next.column;
    return pos;
  }
  return next;
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
  return {
    type: 20,
    loc: locStub,
    index,
    value,
    needPauseTracking: !!(arguments.length > 2 && arguments[2]),
    inVOnce: !!(arguments.length > 3 && arguments[3]),
    needArraySpread: false,
  };
}

function createCallExpression(callee) {
  let args = Array.prototype.slice.call(arguments, 1);
  let loc = locStub;
  if (args.length > 1 && args[args.length - 1] && args[args.length - 1].start && args[args.length - 1].end) {
    loc = args.pop();
  }
  return { type: 14, loc, callee, arguments: args.flat() };
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
  return {
    type: 18,
    loc: arguments[4] || locStub,
    params,
    returns: arguments.length > 1 ? arguments[1] : undefined,
    body: undefined,
    newline: !!(arguments.length > 2 && arguments[2]),
    isSlot: !!(arguments.length > 3 && arguments[3]),
  };
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
    for (let index = 0; index < props.length; index++) {
      const prop = props[index];
      if (prop && prop.type === NodeTypes.DIRECTIVE && matches(prop)) {
        if (isTemplateNodeWithVSlot(node) && (prop.name === 'if' || prop.name === 'else' || prop.name === 'else-if' || prop.name === 'for')) {
          continue;
        }
        props.splice(index, 1);
        index--;
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
    cached: [],
    constantCache: new WeakMap(),
    identifiers: Object.create(null),
    scopes: { vFor: 0, vSlot: 0, vPre: 0, vOnce: 0 },
    parent: null,
    grandParent: null,
    currentNode: root,
    childIndex: 0,
    directiveTransforms: (options || {}).directiveTransforms || {},
    compatConfig: (options || {}).compatConfig,
    inSSR: !!((options || {}).inSSR || (options || {}).ssr),
    ssr: !!((options || {}).ssr),
    prefixIdentifiers: !!((options || {}).prefixIdentifiers),
    cacheHandlers: !!((options || {}).cacheHandlers),
    bindingMetadata: (options || {}).bindingMetadata || {},
    scopeId: (options || {}).scopeId,
    slotted: !!((options || {}).slotted),
    inline: !!((options || {}).inline),
    isTS: !!((options || {}).isTS),
    expressionPlugins: (options || {}).expressionPlugins || [],
    inVOnce: false,
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
      return `_${helperNameMap[this.helper(name)] || String(name).replace(/^Symbol\((.*)\)$/, '$1')}`;
    },
    replaceNode(node) {
      if (!this.parent) {
        this.currentNode = node;
      } else {
        this.parent.children[this.childIndex] = this.currentNode = node;
      }
    },
    removeNode(node) {
      if (!this.parent) {
        this.currentNode = null;
        return;
      }
      const list = this.parent.children || [];
      const removalIndex = node ? list.indexOf(node) : this.currentNode ? this.childIndex : -1;
      if (removalIndex < 0) return;
      if (!node || node === this.currentNode) {
        this.currentNode = null;
        this.onNodeRemoved();
      } else if (this.childIndex > removalIndex) {
        this.childIndex--;
        this.onNodeRemoved();
      }
      list.splice(removalIndex, 1);
    },
    onNodeRemoved() {},
    addIdentifiers(exp) {
      for (const name of expressionIdentifierNames(exp)) {
        this.identifiers[name] = (this.identifiers[name] || 0) + 1;
      }
    },
    removeIdentifiers(exp) {
      for (const name of expressionIdentifierNames(exp)) {
        if (!this.identifiers[name]) continue;
        this.identifiers[name]--;
        if (this.identifiers[name] <= 0) delete this.identifiers[name];
      }
    },
    cache(exp, isVNode, inVOnce) {
      const cached = createCacheExpression(this.cached.length, exp, !!isVNode, !!inVOnce);
      this.cached.push(cached);
      return cached;
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
    const isBlock = !!(arguments.length > 7 && arguments[7]);
    const isComponent = !!(arguments.length > 9 && arguments[9]);
    if (isBlock) {
      context.helper(OPEN_BLOCK);
      context.helper(getVNodeBlockHelper(context.inSSR, isComponent));
    } else {
      context.helper(getVNodeHelper(context.inSSR, isComponent));
    }
    if (directives) context.helper(WITH_DIRECTIVES);
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

function vue3TransformContextPayload(context) {
  context = context || {};
  return {
    prefixIdentifiers: !!context.prefixIdentifiers,
    cacheHandlers: !!context.cacheHandlers,
    inVOnce: !!context.inVOnce,
    inline: !!context.inline,
    isTS: !!context.isTS,
    identifiers: context.identifiers || {},
    bindingMetadata: context.bindingMetadata || {},
    expressionPlugins: context.expressionPlugins || [],
  };
}

function vue3TransformBindContextPayload(context) {
  context = context || {};
  return {
    inSSR: !!context.inSSR,
    browser: isBrowserBuild(),
  };
}

function vue3TransformVBindShorthandContextPayload(context) {
  return {
    browser: isBrowserBuild(),
  };
}

function vue3TransformSlotContextPayload(context) {
  context = context || {};
  return {
    prefixIdentifiers: !!context.prefixIdentifiers,
    inline: !!context.inline,
    isTS: !!context.isTS,
    ssr: !!(context.ssr || context.inSSR),
    identifiers: context.identifiers || {},
    bindingMetadata: context.bindingMetadata || {},
    expressionPlugins: context.expressionPlugins || [],
    vForDepth: context.scopes && context.scopes.vFor || 0,
    vSlotDepth: context.scopes && context.scopes.vSlot || 0,
  };
}

function isBrowserBuild() {
  return typeof __BROWSER__ !== 'undefined' && !!__BROWSER__;
}

function materializeVue3ExpressionProjection(projection, fallback, context) {
  if (!projection || projection.kind === 'unchanged') return fallback;
  if (projection.kind === 'setConstType') {
    if (fallback && typeof fallback === 'object') fallback.constType = projection.constType;
    return fallback;
  }
  if (projection.kind === 'simple') {
    const node = createSimpleExpression(
      projection.content || '',
      !!projection.isStatic,
      projection.loc || (fallback && fallback.loc) || locStub,
    );
    if (projection.constType !== undefined) node.constType = projection.constType;
    registerVue3ProjectionHelpers(projection, context);
    return node;
  }
  if (projection.kind === 'compound') {
    const children = (projection.children || []).map(child => materializeVue3ExpressionChild(child, fallback));
    const node = createCompoundExpression(children);
    node.loc = projection.loc || (fallback && fallback.loc) || locStub;
    node.identifiers = projection.identifiers || [];
    registerVue3ProjectionHelpers(projection, context);
    return node;
  }
  if (projection.kind === 'error') {
    const error = createCompilerError(projection.code || ErrorCodes.X_INVALID_EXPRESSION, projection.loc || (fallback && fallback.loc) || locStub);
    error.message = projection.message || error.message;
    if (context && typeof context.onError === 'function') context.onError(error);
    return fallback;
  }
  return fallback;
}

function materializeVue3ExpressionChild(child, fallback) {
  if (!child || typeof child !== 'object' || !child.kind) return child;
  if (child.kind === 'simple') {
    const node = createSimpleExpression(
      child.content || '',
      !!child.isStatic,
      child.loc || (fallback && fallback.loc) || locStub,
    );
    if (child.constType !== undefined) node.constType = child.constType;
    return node;
  }
  return child;
}

function materializeVue3ProjectionNode(projection, refs, context) {
  if (!projection || projection.kind === 'undefined') return undefined;
  if (typeof projection === 'string') return projection;
  if (projection.type) return projection;
  refs = refs || {};
  const dir = refs.dir;
  switch (projection.kind) {
    case 'node':
      if (projection.path === 'dir.arg') return dir && dir.arg;
      if (projection.path === 'dir.exp') return dir && dir.exp;
      if (projection.path === 'dir.arg.children') return (dir && dir.arg && dir.arg.children) || [];
      if (projection.path === 'props') {
        const prop = refs.node && refs.node.props && refs.node.props[projection.index];
        return prop && prop[projection.field || 'exp'];
      }
      return undefined;
    case 'static':
      return createSimpleExpression(projection.content || '', true, projection.loc || (dir && dir.arg && dir.arg.loc) || (dir && dir.loc) || locStub);
    case 'children': {
      const children = [];
      for (const child of projection.children || []) {
        const materialized = materializeVue3ProjectionNode(child, refs, context);
        if (Array.isArray(materialized)) children.push(...materialized);
        else children.push(materialized);
      }
      return children;
    }
    case 'helperString': {
      const helper = helperSymbolFromProjection(projection.helper);
      if (helper && context && typeof context.helperString === 'function') {
        return `${context.helperString(helper)}(`;
      }
      return `_${helperNameMap[helper] || projection.helper || ''}(`;
    }
    case 'simple': {
      registerVue3ProjectionHelpers(projection, context);
      const node = createSimpleExpression(
        projection.content || '',
        !!projection.isStatic,
        projection.loc || (dir && dir.exp && dir.exp.loc) || (dir && dir.arg && dir.arg.loc) || (dir && dir.loc) || locStub,
      );
      if (projection.constType !== undefined) node.constType = projection.constType;
      return node;
    }
    case 'compound': {
      registerVue3ProjectionHelpers(projection, context);
      const children = [];
      for (const child of projection.children || []) {
        const materialized = materializeVue3ProjectionNode(child, refs, context);
        if (Array.isArray(materialized)) children.push(...materialized);
        else children.push(materialized);
      }
      const node = createCompoundExpression(children);
      node.loc = projection.loc || (dir && dir.arg && dir.arg.loc) || (dir && dir.exp && dir.exp.loc) || (dir && dir.loc) || locStub;
      if (projection.constType !== undefined) node.constType = projection.constType;
      return node;
    }
    default:
      throw new Error(`Unsupported Rust Vue 3 projection: ${projection.kind}`);
  }
}

function registerVue3ProjectionHelpers(projection, context) {
  if (!context || typeof context.helper !== 'function') return;
  for (const helperName of projection.helpers || []) {
    const helper = helperSymbolFromProjection(helperName);
    if (helper) context.helper(helper);
  }
}

function setVue3NodeAtPath(node, path, value) {
  let target = node;
  for (let index = 0; index + 1 < path.length; index++) {
    if (!target) return;
    target = target[path[index]];
  }
  if (target) target[path[path.length - 1]] = value;
}

function getVue3NodeAtPath(node, path) {
  let target = node;
  for (const key of path || []) {
    if (!target) return undefined;
    target = target[key];
  }
  return target;
}

function vue3TransformOnceContextPayload(context) {
  context = context || {};
  return {
    inVOnce: !!context.inVOnce,
    inSSR: !!context.inSSR,
  };
}

function vue3TransformSlotOutletContextPayload(context) {
  context = context || {};
  return {
    prefixIdentifiers: !!context.prefixIdentifiers,
    inline: !!context.inline,
    isTS: !!context.isTS,
    identifiers: context.identifiers || {},
    bindingMetadata: context.bindingMetadata || {},
    expressionPlugins: context.expressionPlugins || [],
    scopeId: context.scopeId,
    slotted: !!context.slotted,
  };
}

function vue3IfSiblingPayload(siblings) {
  return (siblings || []).map(sibling => ({
    type: sibling && sibling.type,
    content: sibling && sibling.content,
    locSource: sibling && sibling.loc && sibling.loc.source,
    tagType: sibling && sibling.tagType,
    branches: sibling && sibling.branches ? sibling.branches.map(branch => ({
      hasCondition: !!branch.condition,
      userKey: branch.userKey || null,
    })) : undefined,
  }));
}

function materializeVue3IfProjection(projection, node, dir) {
  if (!projection || projection.kind === 'undefined') return undefined;
  if (projection.kind === 'simple') {
    return createSimpleExpression(
      projection.content || '',
      !!projection.isStatic,
      projection.loc || (dir && dir.exp && dir.exp.loc) || (node && node.loc) || locStub,
    );
  }
  throw new Error(`Unsupported Rust v-if projection: ${projection.kind}`);
}

function vue3IfBranchCodegenPayload(branch) {
  return {
    isTemplateIf: !!(branch && branch.isTemplateIf),
    children: (branch && branch.children || []).map(child => ({
      type: child && child.type,
      memoedCodegenType: getMemoedVNodeCall(child && child.codegenNode) && getMemoedVNodeCall(child && child.codegenNode).type,
    })),
  };
}

function getParentCondition(node) {
  while (node) {
    if (node.type === NodeTypes.JS_CONDITIONAL_EXPRESSION) {
      if (node.alternate && node.alternate.type === NodeTypes.JS_CONDITIONAL_EXPRESSION) {
        node = node.alternate;
      } else {
        return node;
      }
    } else if (node.type === NodeTypes.JS_CACHE_EXPRESSION) {
      node = node.value;
    } else {
      return node;
    }
  }
  return node;
}

function createIfCodegenNodeForBranch(branch, keyIndex, context) {
  const childCodegen = createIfBranchCodegen(branch, keyIndex, context);
  if (branch.condition) {
    return createConditionalExpression(
      branch.condition,
      childCodegen,
      createCallExpression(context.helper(CREATE_COMMENT), ['"v-if"', 'true']),
    );
  }
  return childCodegen;
}

function createIfBranchCodegen(branch, keyIndex, context) {
  const keyProperty = createObjectProperty('key', createSimpleExpression(String(keyIndex), false, locStub));
  const children = branch.children || [];
  const firstChild = children[0];
  const projection = callVue3CoreProjection('vue3.core.transformIf', {
    phase: 'branchCodegen',
    branch: vue3IfBranchCodegenPayload(branch),
    keyIndex,
  });
  if (projection.kind === 'for') {
    const vnodeCall = firstChild && firstChild.codegenNode;
    injectProp(vnodeCall, keyProperty, context);
    return vnodeCall;
  }
  if (projection.kind === 'fragment') {
    return createVNodeCall(context, context.helper(FRAGMENT), createObjectExpression([keyProperty]), children, projection.patchFlag, undefined, undefined, true, false, false, branch.loc);
  }
  const ret = firstChild && firstChild.codegenNode;
  const vnodeCall = getMemoedVNodeCall(ret);
  if (vnodeCall) {
    if (vnodeCall.type === NodeTypes.VNODE_CALL) {
      convertToBlock(vnodeCall, context);
    }
    injectProp(vnodeCall, keyProperty, context);
  }
  return ret;
}

function materializeVue3ForParseResult(parseResult, dir) {
  return {
    source: materializeVue3ForProjectionNode(parseResult && parseResult.source, dir),
    value: materializeVue3ForProjectionNode(parseResult && parseResult.value, dir),
    key: materializeVue3ForProjectionNode(parseResult && parseResult.key, dir),
    index: materializeVue3ForProjectionNode(parseResult && parseResult.index, dir),
    finalized: parseResult && parseResult.finalized !== undefined ? !!parseResult.finalized : true,
  };
}

function materializeVue3ForProjectionNode(projection, dir) {
  if (!projection || projection.kind === 'undefined') return undefined;
  if (projection.type) return projection;
  if (projection.kind === 'simple') {
    return createSimpleExpression(
      projection.content || '',
      !!projection.isStatic,
      projection.loc || (dir && dir.exp && dir.exp.loc) || (dir && dir.loc) || locStub,
    );
  }
  return undefined;
}

function vue3ForNodePayload(forNode) {
  return {
    source: forNode && forNode.source,
    children: (forNode && forNode.children || []).map(child => ({
      type: child && child.type,
      tagType: child && child.tagType,
      codegenNode: child && child.codegenNode ? {
        type: child.codegenNode.type,
        isBlock: !!child.codegenNode.isBlock,
        isComponent: !!child.codegenNode.isComponent,
      } : null,
    })),
  };
}

function createForLoopParams(parseResult) {
  const args = [parseResult && parseResult.value, parseResult && parseResult.key, parseResult && parseResult.index];
  let end = args.length;
  while (end > 0 && !args[end - 1]) end--;
  return args.slice(0, end).map((arg, index) => arg || createSimpleExpression('_'.repeat(index + 1), false));
}

function finalizeForCodegen(forNode, renderExp, context) {
  if (!renderExp || renderExp.arguments.length > 1) return;
  const children = forNode.children || [];
  let childBlock;
  if (children.length === 1 && children[0].type === NodeTypes.ELEMENT) {
    childBlock = children[0].codegenNode;
    if (childBlock && childBlock.type === NodeTypes.VNODE_CALL && !childBlock.isBlock) {
      context.removeHelper(getVNodeHelper(context.inSSR, childBlock.isComponent));
      childBlock.isBlock = true;
      context.helper(OPEN_BLOCK);
      context.helper(getVNodeBlockHelper(context.inSSR, childBlock.isComponent));
    }
  } else {
    childBlock = createVNodeCall(context, context.helper(FRAGMENT), undefined, children, 64, undefined, undefined, true, false, false, forNode.loc);
  }
  renderExp.arguments.push(createFunctionExpression(createForLoopParams(forNode.parseResult), childBlock, true));
}

function stringifyDynamicPropNames(props) {
  return `[${(props || []).map(prop => JSON.stringify(prop)).join(', ')}]`;
}

function finalizeVue3ForExitCodegen(node, forNode, renderExp, codegenProjection, context) {
  const exitProjection = callVue3CoreProjection('vue3.core.transformFor', {
    phase: 'exitCodegen',
    node,
    forNode: vue3ForNodePayload(forNode),
    isStableFragment: !!(codegenProjection && codegenProjection.isStableFragment),
    context: vue3TransformContextPayload(context),
  });
  let childBlock;
  if (exitProjection && exitProjection.kind === 'fragmentWrapper') {
    childBlock = createVNodeCall(
      context,
      context.helper(FRAGMENT),
      undefined,
      forNode.children || [],
      exitProjection.patchFlag,
      undefined,
      undefined,
      true,
      false,
      false,
      node.loc,
    );
  } else {
    childBlock = forNode.children && forNode.children[0] && forNode.children[0].codegenNode;
    if (childBlock && exitProjection && exitProjection.kind === 'singleElement' && exitProjection.childBlockIsBlock && childBlock.type === NodeTypes.VNODE_CALL) {
      convertToBlock(childBlock, context);
    }
  }
  renderExp.arguments.push(createFunctionExpression(createForLoopParams(forNode.parseResult), childBlock, true));
}

function getBaseTransformPreset(prefixIdentifiers) {
  return [[
    transformVBindShorthand,
    transformOnce,
    transformIf,
    transformFor,
    ...(prefixIdentifiers ? [trackVForSlotScopes] : []),
    transformExpression,
    transformSlotOutlet,
    transformElement,
    trackSlotScopes,
    transformText,
  ], { on: transformOn, bind: transformBind, model: transformModel }];
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

function createRootCodegen(root, context) {
  const projection = callVue3CoreProjection('vue3.core.rootCodegen', { root });
  if (!projection || projection.kind === 'none') return;
  if (projection.kind === 'child') {
    root.codegenNode = (root.children || [])[projection.index || 0];
    return;
  }
  if (projection.kind === 'childCodegen') {
    const child = (root.children || [])[projection.index || 0];
    const codegenNode = child && child.codegenNode;
    if (codegenNode && projection.asBlock) {
      convertToBlock(codegenNode, context);
    }
    root.codegenNode = codegenNode;
    return;
  }
  if (projection.kind === 'fragment') {
    root.codegenNode = createVNodeCall(
      context,
      context.helper(FRAGMENT),
      undefined,
      root.children || [],
      projection.patchFlag,
      undefined,
      undefined,
      true,
      false,
      false,
    );
  }
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

function expressionIdentifierNames(exp) {
  if (!exp) return [];
  if (typeof exp === 'string') return exp ? [exp] : [];
  if (Array.isArray(exp.identifiers)) return exp.identifiers.filter(Boolean);
  if (exp.type === NodeTypes.COMPOUND_EXPRESSION) {
    return (exp.children || []).flatMap(child => expressionIdentifierNames(child)).filter(Boolean);
  }
  if (exp.type === NodeTypes.SIMPLE_EXPRESSION && exp.content) return [exp.content];
  return [];
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
  const projection = callVue3CoreProjection('vue3.core.isInDestructureAssignment', {
    parent,
    parentStack: parentStack || [],
  });
  return !!(projection && projection.isInDestructureAssignment);
}

function isInNewExpression(parentStack) {
  return Array.isArray(parentStack) && parentStack.some(node => node && node.type === 'NewExpression');
}

const isMemberExpressionNode = (path, context) => {
  const projection = callVue3CoreProjection('vue3.core.isMemberExpression', {
    node: typeof path === 'string' ? createSimpleExpression(path) : path,
    context: context || {},
    mode: 'node',
  });
  return !!(projection && projection.isMemberExpression);
};

const isMemberExpressionBrowser = (path) => {
  const projection = callVue3CoreProjection('vue3.core.isMemberExpression', {
    node: typeof path === 'string' ? createSimpleExpression(path) : path,
    context: {},
    mode: 'browser',
  });
  return !!(projection && projection.isMemberExpression);
};

const isMemberExpression = isMemberExpressionNode;

function isReferencedIdentifier(id, parent, parentStack) {
  const relation = arguments.length > 3 ? arguments[3] : undefined;
  const projection = callVue3CoreProjection('vue3.core.isReferencedIdentifier', {
    node: id,
    parent: parent || null,
    parentStack: parentStack || [],
    relation: relation || nodeRelation(parent, id),
  });
  return !!(projection && projection.isReferencedIdentifier);
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

function isTemplateNodeWithVSlot(node) {
  return isTemplateNode(node) && (node.props || []).some(isVSlot);
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
  return !!node && (
    (node.type === NodeTypes.TEXT && isAllWhitespace(node.content)) ||
    (node.type === NodeTypes.TEXT_CALL && isWhitespaceText(node.content))
  );
}

const noopDirectiveTransform = () => {
  return { props: [] };
};

function cloneVue3SlotTemplateForProjection(node, structuralDir) {
  const props = (node.props || []).filter(prop => prop !== structuralDir);
  if (structuralDir) props.push(structuralDir);
  return {
    ...node,
    props,
    children: node.children || [],
  };
}

function processExpression(node, context) {
  return materializeVue3ExpressionProjection(
    callVue3CoreProjection('vue3.core.processExpression', {
      node,
      context: vue3TransformContextPayload(context),
      asParams: !!(arguments.length > 2 && arguments[2]),
      asRawStatements: !!(arguments.length > 3 && arguments[3]),
      localVars: context && context.identifiers,
    }),
    node,
    context,
  );
}

function processFor(node, dir, context, processCodegen) {
  const slotTemplate = node.tagType === ElementTypes.TEMPLATE && node.props && node.props.some(isVSlot)
    ? cloneVue3SlotTemplateForProjection(node, dir)
    : undefined;
  const projection = callVue3CoreProjection('vue3.core.transformFor', {
    node,
    dir,
    context: vue3TransformContextPayload(context),
  });
  if (!projection || !projection.parseResult) return undefined;
  const parsed = materializeVue3ForParseResult(projection.parseResult, dir, context);
  const aliases = projection.locals || [];
  if (context.prefixIdentifiers) aliases.forEach(alias => context.addIdentifiers(alias));
  const children = node.tagType === ElementTypes.TEMPLATE ? node.children || [] : [node];
  const forNode = {
    type: NodeTypes.FOR,
    loc: dir.loc,
    source: parsed.source,
    valueAlias: parsed.value,
    keyAlias: parsed.key,
    objectIndexAlias: parsed.index,
    parseResult: parsed,
    children,
    codegenNode: undefined,
    __vuecProjection: projection,
  };
  if (slotTemplate) forNode.__vuecSlotTemplate = slotTemplate;
  context.replaceNode(forNode);
  context.scopes.vFor++;
  const onExit = typeof processCodegen === 'function' ? processCodegen(forNode) : undefined;
  return () => {
    context.scopes.vFor--;
    if (context.prefixIdentifiers) aliases.forEach(alias => context.removeIdentifiers(alias));
    if (onExit) {
      onExit();
      return;
    }
    const renderExp = createCallExpression(context.helper(RENDER_LIST), [forNode.source]);
    forNode.codegenNode = createVNodeCall(context, context.helper(FRAGMENT), undefined, renderExp, 256, undefined, undefined, true, true, false, node.loc);
    finalizeForCodegen(forNode, renderExp, context);
  };
}

function processIf(node, dir, context, processCodegen) {
  const slotTemplate = node.tagType === ElementTypes.TEMPLATE && node.props && node.props.some(isVSlot)
    ? cloneVue3SlotTemplateForProjection(node, dir)
    : undefined;
  const siblings = context.parent && context.parent.children || [];
  const nodeIndex = siblings.indexOf(node);
  const projection = callVue3CoreProjection('vue3.core.transformIf', {
    phase: 'process',
    node,
    dir,
    parent: context.parent,
    siblings: vue3IfSiblingPayload(siblings),
    nodeIndex,
    currentUserKey: findProp(node, 'key'),
    context: vue3TransformContextPayload(context),
  });
  if (projection && projection.branch && projection.branch.condition) {
    dir.exp = materializeVue3IfProjection(projection.branch.condition, node, dir);
  }
  const branch = {
    type: NodeTypes.IF_BRANCH,
    loc: node.loc,
    condition: dir.name === 'else' ? undefined : dir.exp,
    children: projection && projection.branch && projection.branch.children === 'template' ? (node.children || []) : [node],
    userKey: findProp(node, 'key'),
    isTemplateIf: node.tagType === ElementTypes.TEMPLATE,
    __vuecSlotTemplate: slotTemplate,
  };
  const action = projection && projection.action || { kind: 'noop' };
  const finalizeBranch = (ifNode, targetBranch, isRoot) => {
    if (processCodegen) return processCodegen(ifNode, targetBranch, isRoot);
    if (context && context.ssr) return undefined;
    return () => {
      if (isRoot) {
        ifNode.codegenNode = createIfCodegenNodeForBranch(targetBranch, action.keyBase || 0, context);
      } else {
        const parentCondition = getParentCondition(ifNode.codegenNode);
        parentCondition.alternate = createIfCodegenNodeForBranch(targetBranch, (ifNode.__vuecKeyBase || 0) + ifNode.branches.length - 1, context);
      }
    };
  };
  if (dir.name !== 'if') {
    if (action.kind === 'append') {
      const comments = (action.commentIndices || []).map(index => siblings[index]).filter(Boolean);
      for (const index of [...(action.removeIndices || [])].sort((a, b) => b - a)) {
        const sibling = siblings[index];
        if (sibling) context.removeNode(sibling);
      }
      const target = siblings[action.targetIndex];
      context.removeNode();
      if (comments.length) branch.children = [...comments, ...branch.children];
      target.branches.push(branch);
      const onExit = finalizeBranch(target, branch, false);
      traverseNode(branch, context);
      if (onExit) onExit();
      context.currentNode = null;
    }
    return undefined;
  }
  const ifNode = { type: NodeTypes.IF, loc: node.loc, branches: [branch], codegenNode: undefined };
  if (branch.__vuecSlotTemplate) ifNode.__vuecSlotTemplate = branch.__vuecSlotTemplate;
  ifNode.__vuecKeyBase = action.keyBase || 0;
  context.replaceNode(ifNode);
  const onExit = finalizeBranch(ifNode, branch, true);
  return () => {
    if (onExit) onExit();
  };
}

function processSlotOutlet(node, context) {
  const projection = callVue3CoreProjection('vue3.core.transformSlotOutlet', {
    node,
    context: vue3TransformSlotOutletContextPayload(context),
  });
  const process = projection && projection.process || {};
  materializeVue3SlotOutletMutations(process, node, context);
  const nonNameProps = (process.nonNameProps || [])
    .map(index => node && node.props && node.props[index])
    .filter(Boolean);
  const slotName = materializeVue3SlotOutletName(process.slotName, node, context);
  let slotProps;
  if (nonNameProps.length) {
    const built = buildProps(node, context, nonNameProps);
    slotProps = built.props;
    emitVue3SlotOutletDirectiveError(built, context);
  }
  return { slotName, slotProps };
}

function transformSlotOutlet(node, context) {
  if (!isSlotOutlet(node)) return undefined;
  return () => {
    const projection = callVue3CoreProjection('vue3.core.transformSlotOutlet', {
      node,
      context: vue3TransformSlotOutletContextPayload(context),
    });
    if (!projection || !projection.transform) return;
    const process = projection.process || {};
    materializeVue3SlotOutletMutations(process, node, context);
    const nonNameProps = (process.nonNameProps || [])
      .map(index => node && node.props && node.props[index])
      .filter(Boolean);
    const slotName = materializeVue3SlotOutletName(process.slotName, node, context);
    let slotProps;
    if (nonNameProps.length) {
      const built = buildProps(node, context, nonNameProps);
      slotProps = built.props;
      emitVue3SlotOutletDirectiveError(built, context);
    }
    const codegen = projection.codegen || {};
    const args = [codegen.slots || (context.prefixIdentifiers ? '_ctx.$slots' : '$slots'), slotName, '{}', 'undefined', 'true'];
    let expectedLen = codegen.expectedLen == null ? 2 : codegen.expectedLen;
    if (slotProps) {
      args[2] = slotProps;
      expectedLen = Math.max(expectedLen, 3);
    }
    if (node.children && node.children.length) {
      args[3] = createFunctionExpression([], node.children, false, false, node.loc);
      expectedLen = Math.max(expectedLen, 4);
    }
    args.splice(expectedLen);
    node.codegenNode = createCallExpression(context.helper(RENDER_SLOT), args);
  };
}

function materializeVue3SlotOutletMutations(process, node, context) {
  for (const mutation of process && process.mutations || []) {
    const prop = node && node.props && node.props[mutation.index];
    if (!prop) continue;
    if (mutation.kind === 'setPropName') {
      prop.name = mutation.name || prop.name;
    } else if (mutation.kind === 'setDirectiveArgContent' && prop.arg) {
      prop.arg.content = mutation.content || '';
    } else if (mutation.kind === 'setDirectiveExp') {
      prop.exp = materializeVue3ProjectionNode(mutation.value, { node }, context);
    }
  }
}

function materializeVue3SlotOutletName(projection, node, context) {
  if (!projection) return '"default"';
  if (projection.kind === 'literal') return projection.value || '"default"';
  return materializeVue3ProjectionNode(projection, { node }, context);
}

function materializeVue3SlotErrors(projection, node, context) {
  if (!projection || !Array.isArray(projection.errors) || !context || typeof context.onError !== 'function') return;
  for (const error of projection.errors) {
    context.onError(createCompilerError(error.code, error.loc || (node && node.loc) || locStub));
  }
}

function materializeVue3SlotsProjection(projection, node, context, buildSlotFn) {
  projection = projection || {};
  if (context) context.helper(WITH_CTX);
  const properties = [];
  for (const property of projection.properties || []) {
    properties.push(createObjectProperty(
      materializeVue3SlotProjectionNode(property.key, node, context),
      materializeVue3SlotFunctionProjection(property, node, context, buildSlotFn),
    ));
  }
  const slotFlag = projection.slotFlag || 1;
  const flagText = projection.slotFlagText || (slotFlag === 2 ? 'DYNAMIC' : slotFlag === 3 ? 'FORWARDED' : 'STABLE');
  properties.push(createObjectProperty(
    '_',
    createSimpleExpression(`${slotFlag} /* ${flagText} */`, false),
  ));
  let slots = createObjectExpression(properties);
  slots.loc = node && node.loc || locStub;
  if (projection.dynamicSlots && projection.dynamicSlots.length) {
    const dynamicSlotArray = createArrayExpression(
      projection.dynamicSlots.map(slot => materializeVue3DynamicSlotProjection(slot, node, context, buildSlotFn)),
    );
    if (context) context.helper(CREATE_SLOTS);
    slots = createCallExpression(
      context ? context.helper(CREATE_SLOTS) : CREATE_SLOTS,
      [
        slots,
        dynamicSlotArray,
      ],
      node && node.loc || locStub,
    );
  }
  return slots;
}

function materializeVue3SlotFunctionProjection(property, node, context, buildSlotFn) {
  const loc = property.loc || (node && node.loc) || locStub;
  const params = materializeVue3SlotProjectionNode(property.params, node, context);
  const returns = materializeVue3SlotChildren(property, node);
  if (typeof buildSlotFn === 'function') {
    const vFor = vue3SlotFunctionVFor(property, node, context);
    const fn = buildSlotFn(params, vFor, returns, loc);
    if (property.nonScoped && context && context.compatConfig && fn) fn.isNonScopedSlot = true;
    return fn;
  }
  const fn = createFunctionExpression(params, returns, false, true, returns.length ? returns[0].loc : loc);
  if (property.nonScoped && context && context.compatConfig) fn.isNonScopedSlot = true;
  return fn;
}

function vue3SlotFunctionVFor(property, node, context) {
  for (const index of property.indices || []) {
    const child = node && node.children && node.children[index];
    const source = property.unwrapTemplate && child && child.type === NodeTypes.ELEMENT && child.tag === 'template'
      ? child
      : null;
    const dir = source && findDir(source, 'for', true);
    if (!dir) continue;
    if (!dir.forParseResult) {
      const projection = callVue3CoreProjection('vue3.core.trackVForSlotScopes', {
        node: source,
        context: vue3TransformSlotContextPayload(context),
      });
      if (projection && projection.parseResult) {
        dir.forParseResult = materializeVue3ForParseResult(projection.parseResult, dir);
      }
    }
    return dir;
  }
  return undefined;
}

function materializeVue3SlotChildren(property, node) {
  const out = [];
  for (const index of property.indices || []) {
    const child = node && node.children && node.children[index];
    if (!child) continue;
    if (child.__vuecSlotTemplate && child.__vuecSlotTemplate.children) {
      out.push(...child.__vuecSlotTemplate.children);
      continue;
    }
    if (property.unwrapTemplate && child.type === NodeTypes.ELEMENT && child.tag === 'template') {
      out.push(...(child.children || []));
    } else {
      out.push(child);
    }
  }
  return out;
}

function materializeVue3DynamicSlotProjection(projection, node, context, buildSlotFn) {
  if (!projection) return createSimpleExpression('undefined', false);
  if (projection.kind === 'conditional') {
    return createConditionalExpression(
      materializeVue3SlotProjectionNode(projection.test, node, context),
      materializeVue3DynamicSlotProjection(projection.consequent, node, context, buildSlotFn),
      materializeVue3DynamicSlotProjection(projection.alternate, node, context, buildSlotFn),
    );
  }
  if (projection.kind === 'for') {
    const params = projection.params || {};
    const slot = materializeVue3DynamicSlotProjection(projection.slot, node, context, buildSlotFn);
    const source = materializeVue3SlotProjectionNode(projection.source, node, context);
    const loopParams = createForLoopParams({
      value: materializeVue3SlotProjectionNode(params.value, node, context),
      key: materializeVue3SlotProjectionNode(params.key, node, context),
      index: materializeVue3SlotProjectionNode(params.index, node, context),
    });
    const renderListHelper = context ? context.helper(RENDER_LIST) : RENDER_LIST;
    return createCallExpression(
      renderListHelper,
      [
        source,
        createFunctionExpression(
          loopParams,
          slot,
          true,
        ),
      ],
      node && node.loc || locStub,
    );
  }
  if (projection.kind === 'dynamicSlot') {
    const properties = [
      createObjectProperty('name', materializeVue3SlotProjectionNode(projection.name, node, context)),
      createObjectProperty('fn', materializeVue3SlotFunctionProjection(projection.slot || {}, node, context, buildSlotFn)),
    ];
    if (projection.key != null) {
      properties.push(createObjectProperty('key', createSimpleExpression(String(projection.key), true)));
    }
    return createObjectExpression(properties);
  }
  return materializeVue3SlotProjectionNode(projection, node, context);
}

function materializeVue3SlotProjectionNode(projection, node, context) {
  if (!projection || projection.kind === 'undefined') return undefined;
  if (projection.type) return projection;
  registerVue3ProjectionHelpers(projection, context);
  switch (projection.kind) {
    case 'simple': {
      const simple = createSimpleExpression(
        projection.content || '',
        !!projection.isStatic,
        projection.loc || (node && node.loc) || locStub,
      );
      if (projection.constType !== undefined) simple.constType = projection.constType;
      return simple;
    }
    case 'compound': {
      const compound = createCompoundExpression(
        (projection.children || []).map(child => materializeVue3SlotProjectionNode(child, node, context)),
      );
      compound.loc = projection.loc || (node && node.loc) || locStub;
      if (projection.constType !== undefined) compound.constType = projection.constType;
      return compound;
    }
    default:
      if (typeof projection === 'string') return projection;
      throw new Error(`Unsupported Rust v-slot projection: ${projection.kind}`);
  }
}

function emitVue3SlotOutletDirectiveError(built, context) {
  const directive = built && built.directives && built.directives[0];
  if (directive && context && typeof context.onError === 'function') {
    context.onError(createCompilerError(ErrorCodes.X_V_SLOT_UNEXPECTED_DIRECTIVE_ON_SLOT_OUTLET, directive.loc));
  }
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
  const projection = callVue3CoreProjection('vue3.core.toValidAssetId', {
    name: String(name || ''),
    type: String(type || ''),
  });
  return projection && typeof projection.id === 'string' ? projection.id : `_${type}_`;
}

const trackSlotScopes = (node, context) => {
  if (!node || node.type !== NodeTypes.ELEMENT || (node.tagType !== ElementTypes.COMPONENT && node.tagType !== ElementTypes.TEMPLATE)) {
    return undefined;
  }
  const projection = callVue3CoreProjection('vue3.core.trackSlotScopes', {
    node,
    context: vue3TransformSlotContextPayload(context),
  });
  if (!projection || !projection.track) return undefined;
  const props = materializeVue3SlotProjectionNode(projection.slotProps, node, context);
  const locals = (projection.locals || []).filter(Boolean);
  if (context && context.prefixIdentifiers && props) context.addIdentifiers(props);
  if (context && context.prefixIdentifiers) locals.forEach(local => context.addIdentifiers(local));
  if (context && context.scopes) context.scopes.vSlot++;
  return () => {
    if (context && context.prefixIdentifiers && props) context.removeIdentifiers(props);
    if (context && context.prefixIdentifiers) locals.forEach(local => context.removeIdentifiers(local));
    if (context && context.scopes) context.scopes.vSlot--;
  };
};

const trackVForSlotScopes = (node, context) => {
  if (!node || node.type !== NodeTypes.ELEMENT || node.tagType !== ElementTypes.TEMPLATE || !(node.props || []).some(isVSlot)) {
    return undefined;
  }
  const projection = callVue3CoreProjection('vue3.core.trackVForSlotScopes', {
    node,
    context: vue3TransformSlotContextPayload(context),
  });
  if (!projection || !projection.track) return undefined;
  const dir = findDir(node, 'for', true);
  const parseResult = materializeVue3ForParseResult(projection.parseResult, projection.dir || dir);
  const locals = [parseResult.value, parseResult.key, parseResult.index].filter(Boolean);
  if (context && context.prefixIdentifiers) locals.forEach(local => context.addIdentifiers(local));
  if (dir) dir.forParseResult = parseResult;
  return () => {
    if (context && context.prefixIdentifiers) locals.forEach(local => context.removeIdentifiers(local));
  };
};

function vue3TransformTextContextPayload(context) {
  context = context || {};
  return {
    ssr: !!context.ssr,
    inSSR: !!context.inSSR,
    compat: !!(context.compatConfig),
    directiveTransforms: Object.keys(context.directiveTransforms || {}),
    constantCache: context.constantCache || undefined,
  };
}

function materializeVue3TextProjection(projection, node, context) {
  if (!projection || !Array.isArray(projection.operations) || !node || !Array.isArray(node.children)) return;
  for (const operation of projection.operations) {
    if (!operation || operation.kind === 'mergeText') {
      if (!operation) continue;
      const start = operation.start || 0;
      const end = operation.end || start;
      const merged = node.children.slice(start, end + 1);
      const children = [];
      for (let index = 0; index < merged.length; index++) {
        if (index > 0) children.push(' + ');
        children.push(merged[index]);
      }
      const compound = createCompoundExpression(children);
      compound.loc = merged[0] && merged[0].loc || locStub;
      node.children.splice(start, end - start + 1, compound);
    } else if (operation.kind === 'wrapTextCall') {
      const child = node.children[operation.index || 0];
      if (!child) continue;
      const callArgs = [];
      if (operation.includeContent !== false) callArgs.push(child);
      if (operation.patchFlag) callArgs.push(operation.patchFlag);
      node.children[operation.index || 0] = {
        type: NodeTypes.TEXT_CALL,
        content: child,
        loc: child.loc,
        codegenNode: createCallExpression(context.helper(CREATE_TEXT), callArgs),
      };
    }
  }
}

function transformOnce(node, context) {
  const projection = callVue3CoreProjection('vue3.core.transformOnce', {
    node,
    context: vue3TransformOnceContextPayload(context),
    seen: !!(node && node.__vuecOnceSeen),
  });
  if (!projection || projection.kind !== 'enter') return undefined;
  if (projection.markSeen) {
    Object.defineProperty(node, '__vuecOnceSeen', { value: true, configurable: true });
  }
  if (projection.enterInVOnce) context.inVOnce = true;
  const helper = helperSymbolFromProjection(projection.helper);
  if (helper) context.helper(helper);
  return () => {
    if (projection.exit && Object.prototype.hasOwnProperty.call(projection.exit, 'restoreInVOnce')) {
      context.inVOnce = !!projection.exit.restoreInVOnce;
    }
    const current = context.currentNode || node;
    if (projection.exit && projection.exit.cacheCodegen && current && current.codegenNode) {
      current.codegenNode = context.cache(
        current.codegenNode,
        projection.exit.isVNode !== false,
        projection.exit.inVOnce !== false,
      );
    }
  };
}

const transformIf = createStructuralDirectiveTransform(/^(if|else|else-if)$/, processIf);

const transformFor = createStructuralDirectiveTransform('for', (node, dir, context) => {
  return processFor(node, dir, context, forNode => {
    const renderExp = createCallExpression(context.helper(RENDER_LIST), [forNode.source]);
    const codegenProjection = callVue3CoreProjection('vue3.core.transformFor', {
      phase: 'codegen',
      node,
      forNode: vue3ForNodePayload(forNode),
      context: vue3TransformContextPayload(context),
    });
    forNode.codegenNode = createVNodeCall(
      context,
      context.helper(FRAGMENT),
      undefined,
      renderExp,
      codegenProjection && codegenProjection.fragmentFlag || 256,
      undefined,
      undefined,
      true,
      codegenProjection ? !!codegenProjection.disableTracking : true,
      false,
      node.loc,
    );
    return () => {
      finalizeVue3ForExitCodegen(node, forNode, renderExp, codegenProjection, context);
    };
  });
});

function transformText(node, context) {
  if (!node || (node.type !== NodeTypes.ROOT && node.type !== NodeTypes.ELEMENT && node.type !== NodeTypes.FOR && node.type !== NodeTypes.IF_BRANCH)) {
    return undefined;
  }
  return () => {
    materializeVue3TextProjection(
      callVue3CoreProjection('vue3.core.transformText', {
        node,
        context: vue3TransformTextContextPayload(context),
      }),
      node,
      context,
    );
  };
}

function transform(root, options) {
  options = options || {};
  if (!options.nodeTransforms || !options.directiveTransforms) {
    const [nodeTransforms, directiveTransforms] = getBaseTransformPreset(options.prefixIdentifiers);
    options = {
      ...options,
      nodeTransforms: options.nodeTransforms || nodeTransforms,
      directiveTransforms: options.directiveTransforms || directiveTransforms,
    };
  }
  const context = createTransformContext(root, options || {});
  traverseNode(root, context);
  if (!(options && options.ssr)) createRootCodegen(root, context);
  root.helpers = new Set([...context.helpers.keys()]);
  root.components = [...context.components];
  root.directives = [...context.directives];
  root.hoists = context.hoists;
  root.imports = context.imports;
  root.cached = context.cached;
  root.temps = context.temps;
  root.transformed = true;
}

function vue3ProjectionErrorLoc(error, dir) {
  if (error && error.loc === 'arg') return dir && dir.arg && dir.arg.loc || dir && dir.loc || locStub;
  return dir && dir.loc || locStub;
}

function emitVue3DirectiveProjectionErrors(projection, dir, context) {
  if (!projection || !Array.isArray(projection.errors) || !context || typeof context.onError !== 'function') return;
  for (const error of projection.errors) {
    const code = typeof error === 'number' ? error : error && error.code;
    if (code == null) continue;
    context.onError(createCompilerError(code, vue3ProjectionErrorLoc(error, dir)));
  }
}

const transformBind = (dir, node, context) => {
  context = context || {
    helper: name => name,
    helperString: name => `_${helperNameMap[name] || name}`,
    inSSR: false,
    onError: error => { throw error; },
  };
  const projection = callVue3CoreProjection('vue3.core.transformBind', {
    dir,
    context: vue3TransformBindContextPayload(context),
  });
  emitVue3DirectiveProjectionErrors(projection, dir, context);
  return {
    props: (projection && projection.props || []).map(prop => createObjectProperty(
      materializeVue3ProjectionNode(prop.key, { dir, node }, context),
      materializeVue3ProjectionNode(prop.value, { dir, node }, context),
    )),
  };
};

const transformElement = (node, context) => {
  return () => {
    node = context.currentNode || node;
    if (!node || node.type !== NodeTypes.ELEMENT) return;
    if (node.tagType !== ElementTypes.ELEMENT && node.tagType !== ElementTypes.COMPONENT) return;
    const isComponent = node.tagType === ElementTypes.COMPONENT;
    const tag = isComponent ? resolveComponentType(node, context) : `"${node.tag}"`;
    if (isComponent && typeof tag === 'string') {
      context.components.add(node.tag);
      context.helper(RESOLVE_COMPONENT);
    }
    const built = buildProps(node, context);
    let children = node.children && node.children.length ? node.children : undefined;
    let patchFlag = built.patchFlag || undefined;
    if (isComponent && children) {
      const builtSlots = buildSlots(node, context);
      children = builtSlots.slots;
      if (builtSlots.hasDynamicSlots) patchFlag = (patchFlag || 0) | 1024;
    }
    node.codegenNode = createVNodeCall(
      context,
      isComponent ? `_component_${node.tag}` : tag,
      built.props,
      children,
      patchFlag,
      built.dynamicPropNames && built.dynamicPropNames.length ? stringifyDynamicPropNames(built.dynamicPropNames) : undefined,
      built.directives && built.directives.length ? createArrayExpression(built.directives.map(dir => buildDirectiveArgs(dir, context))) : undefined,
      !!built.shouldUseBlock,
      false,
      isComponent,
      node.loc,
    );
  };
};

const transformExpression = (node, context) => {
  const projection = callVue3CoreProjection('vue3.core.transformExpression', {
    node,
    context: vue3TransformContextPayload(context),
  });
  for (const operation of projection && projection.operations || []) {
    if (!operation || operation.kind !== 'process' || !operation.path) continue;
    const current = getVue3NodeAtPath(node, operation.path);
    setVue3NodeAtPath(
      node,
      operation.path,
      materializeVue3ExpressionProjection(operation.projection, current, context),
    );
  }
  return undefined;
};

const transformModel = (dir, node, context) => {
  context = context || {
    helper: name => name,
    cache: value => value,
    onError: error => { throw error; },
  };
  const projection = callVue3CoreProjection('vue3.core.transformModel', {
    dir,
    node,
    context: vue3TransformContextPayload(context),
  });
  for (const code of projection && projection.errors || []) {
    const loc = code === ErrorCodes.X_V_MODEL_NO_EXPRESSION ? dir && dir.loc : dir && dir.exp && dir.exp.loc || dir && dir.loc || locStub;
    if (context && typeof context.onError === 'function') context.onError(createCompilerError(code, loc));
  }
  return {
    props: (projection && projection.props || []).map(prop => {
      const key = materializeVue3ProjectionNode(prop.key, { dir, node }, context);
      const value = materializeVue3ProjectionNode(prop.value, { dir, node }, context);
      const objectProp = createObjectProperty(key, value);
      objectProp.__vuecModel = {
        dynamic: !!prop.dynamic,
        cache: !!prop.cache,
        hydrate: !!prop.hydrate,
        kind: prop.kind,
      };
      if (prop.cache && context && typeof context.cache === 'function') objectProp.value = context.cache(objectProp.value);
      return objectProp;
    }),
  };
};

const transformOn = (dir, node, context, augmentor) => {
  context = context || {
    helper: name => name,
    helperString: name => `_${helperNameMap[name] || name}`,
    cache: value => value,
    onError: error => { throw error; },
  };
  const projection = callVue3CoreProjection('vue3.core.transformOn', {
    dir,
    node,
    context: vue3TransformContextPayload(context),
  });
  emitVue3DirectiveProjectionErrors(projection, dir, context);
  const onMeta = (projection && projection.props || []).map(prop => ({
    cache: !!prop.cache,
    handlerKey: !!prop.handlerKey,
    dynamicKey: !!prop.dynamicKey,
    ignoreDynamicKeyForNormalize: !!prop.ignoreDynamicKeyForNormalize,
    valueConstant: !!prop.valueConstant,
  }));
  let result = {
    props: (projection && projection.props || []).map(prop => createObjectProperty(
      materializeVue3ProjectionNode(prop.key, { dir, node }, context),
      materializeVue3ProjectionNode(prop.value, { dir, node }, context) || createSimpleExpression('() => {}', false, dir && dir.loc || locStub),
    )),
  };
  if (typeof augmentor === 'function') result = augmentor(result) || result;
  for (const [index, prop] of (result.props || []).entries()) {
    const meta = onMeta[index] || onMeta[0] || {};
    if (prop.key && meta.handlerKey) prop.key.isHandlerKey = true;
    if (meta.cache && context && typeof context.cache === 'function') prop.value = context.cache(prop.value);
    prop.__vuecOn = meta;
  }
  return result;
};

const transformVBindShorthand = (node, context) => {
  if (!node || node.type !== NodeTypes.ELEMENT) return undefined;
  const projection = callVue3CoreProjection('vue3.core.transformVBindShorthand', {
    node,
    context: vue3TransformVBindShorthandContextPayload(context),
  });
  for (const operation of projection && projection.operations || []) {
    const prop = node.props && node.props[operation.index];
    if (!prop || operation.kind !== 'setExp') continue;
    for (const error of operation.errors || []) {
      if (context && typeof context.onError === 'function') {
        context.onError(createCompilerError(error.code, vue3ProjectionErrorLoc(error, prop)));
      }
    }
    prop.exp = materializeVue3ProjectionNode(operation.exp, { dir: prop, node }, context);
  }
  return undefined;
};

function traverseNode(node, context) {
  if (!node) return;
  context.currentNode = node;
  const exitFns = [];
  for (const transform of context.options.nodeTransforms || []) {
    const onExit = transform(node, context);
    if (Array.isArray(onExit)) exitFns.push(...onExit);
    else if (onExit) exitFns.push(onExit);
    if (!context.currentNode) return;
    node = context.currentNode;
  }
  switch (node.type) {
    case NodeTypes.IF:
      for (const branch of node.branches || []) traverseNode(branch, context);
      break;
    case NodeTypes.IF_BRANCH:
    case NodeTypes.FOR:
    case NodeTypes.ELEMENT:
    case NodeTypes.ROOT:
      traverseChildren(node, context);
      break;
    case NodeTypes.INTERPOLATION:
      if (!context.ssr) context.helper(TO_DISPLAY_STRING);
      break;
    case NodeTypes.COMMENT:
      if (!context.ssr) context.helper(CREATE_COMMENT);
      break;
  }
  context.currentNode = node;
  for (let index = exitFns.length - 1; index >= 0; index--) exitFns[index]();
}

function traverseChildren(parent, context) {
  let index = 0;
  const nodeRemoved = () => { index--; };
  for (; index < (parent.children || []).length; index++) {
    const child = parent.children[index];
    if (typeof child === 'string') continue;
    context.grandParent = context.parent;
    context.parent = parent;
    context.childIndex = index;
    context.onNodeRemoved = nodeRemoved;
    traverseNode(child, context);
  }
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
  if (!dir) return createArrayExpression([]);
  const projection = callVue3CoreProjection('vue3.core.buildDirectiveArgs', { dir });
  const args = [];
  const runtime = projection && projection.runtime;
  if (!runtime || runtime.kind === 'asset') {
    context.helper(RESOLVE_DIRECTIVE);
    context.directives.add(runtime && runtime.name || dir.name);
    args.push(toValidAssetId(runtime && runtime.name || dir.name, 'directive'));
  } else {
    const helper = helperSymbolFromProjection(runtime.helper || runtime.helperName);
    args.push(helper ? context.helperString(helper) : runtime.helperName || runtime.helper);
  }
  if (projection && projection.includeExp) args.push(dir.exp);
  if (projection && projection.includeArg) {
    if (!projection.includeExp) args.push('void 0');
    args.push(dir.arg);
  }
  const modifiers = projection && projection.modifiers || [];
  if (modifiers.length) {
    if (!(projection && projection.includeArg)) {
      if (!(projection && projection.includeExp)) args.push('void 0');
      args.push('void 0');
    }
    args.push(createObjectExpression(modifiers.map(modifier =>
      createObjectProperty(modifier.name, createSimpleExpression('true', false, dir.loc || locStub)),
    )));
  }
  return createArrayExpression(args);
}

function buildProps(node, context) {
  const propList = arguments.length > 2 ? arguments[2] : undefined;
  const objectProps = [];
  const directives = [];
  const dynamicPropNames = [];
  let patchFlag = 0;
  let shouldUseBlock = false;
  let hasDynamicKey = false;
  let hasHydrationEvent = false;
  for (const prop of propList || node && node.props || []) {
    if (!prop) continue;
    if (prop.type === NodeTypes.ATTRIBUTE) {
      objectProps.push(createObjectProperty(
        createSimpleExpression(prop.name, true, prop.nameLoc || prop.loc),
        createSimpleExpression(prop.value ? prop.value.content : '', true, prop.value ? prop.value.loc : prop.loc),
      ));
      continue;
    }
    if (prop.name === 'bind' && prop.arg) {
      const transform = context && context.directiveTransforms && context.directiveTransforms.bind;
      const result = transform ? transform(prop, node, context) : transformBind(prop, node, context);
      objectProps.push(...((result && result.props) || []));
      if (prop.arg.isStatic) dynamicPropNames.push(prop.arg.content);
      if (result && result.props && result.props.some(prop => prop && prop.key && !isStaticExp(prop.key))) hasDynamicKey = true;
      continue;
    }
    if (prop.name === 'on' && prop.arg) {
      const transform = context && context.directiveTransforms && context.directiveTransforms.on;
      const result = transform ? transform(prop, node, context) : transformOn(prop, node, context);
      objectProps.push(...((result && result.props) || []));
      continue;
    }
    if (prop.name === 'model' && context && context.directiveTransforms && context.directiveTransforms.model) {
      const result = context.directiveTransforms.model(prop, node, context);
      const modelProps = (result && result.props) || [];
      objectProps.push(...modelProps);
      for (const modelProp of modelProps) {
        if (modelProp.__vuecModel && modelProp.__vuecModel.dynamic && isStaticExp(modelProp.key)) {
          dynamicPropNames.push(modelProp.key.content);
        }
        if (modelProp.__vuecModel && modelProp.__vuecModel.dynamic && !isStaticExp(modelProp.key)) {
          hasDynamicKey = true;
        }
        if (modelProp.__vuecModel && modelProp.__vuecModel.hydrate) {
          hasHydrationEvent = true;
        }
      }
      continue;
    }
    if (prop.name === 'slot') {
      if (node && node.tagType !== ElementTypes.COMPONENT && context && typeof context.onError === 'function') {
        context.onError(createCompilerError(ErrorCodes.X_V_SLOT_MISPLACED, prop.loc));
      }
      continue;
    }
    if (prop.name !== 'once' && prop.name !== 'memo') {
      directives.push(prop);
      if (node && node.children && node.children.length) shouldUseBlock = true;
    }
  }
  if (hasDynamicKey) patchFlag |= 16;
  else {
    if (dynamicPropNames.length) patchFlag |= 8;
    if (hasHydrationEvent) patchFlag |= 32;
  }
  let props = objectProps.length ? createObjectExpression(objectProps) : undefined;
  if (!context.inSSR && props && props.properties.some(prop => prop && prop.key && !prop.key.isStatic && !prop.key.isHandlerKey)) {
    props = createCallExpression(context.helper(NORMALIZE_PROPS), [props]);
  }
  return {
    props,
    directives,
    patchFlag,
    dynamicPropNames,
    shouldUseBlock,
  };
}

function buildSlots(node, context) {
  const buildSlotFn = arguments.length > 2 ? arguments[2] : undefined;
  const slotNode = vue3SlotBuildPayloadNode(node);
  const projection = callVue3CoreProjection('vue3.core.buildSlots', {
    node: slotNode,
    context: vue3TransformSlotContextPayload(context),
  });
  materializeVue3SlotErrors(projection, slotNode, context);
  const slots = materializeVue3SlotsProjection(projection, slotNode, context, buildSlotFn);
  return {
    slots,
    hasDynamicSlots: !!(projection && projection.hasDynamicSlots),
  };
}

function vue3SlotBuildPayloadNode(node) {
  if (!node || !Array.isArray(node.children)) return node;
  const children = [];
  for (const child of node.children) {
    if (child && child.__vuecSlotTemplate) {
      children.push({
        ...child.__vuecSlotTemplate,
        __vuecTransformedSlotNode: child,
      });
      if (child.type === NodeTypes.IF && Array.isArray(child.branches)) {
        for (const branch of child.branches.slice(1)) {
          if (branch && branch.__vuecSlotTemplate) {
            children.push({
              ...branch.__vuecSlotTemplate,
              __vuecTransformedSlotNode: child,
            });
          }
        }
      }
    } else {
      children.push(child);
    }
  }
  return {
    ...node,
    children,
  };
}

function convertToBlock(node, context) {
  if (!node) return node;
  if (!node.isBlock) {
    node.isBlock = true;
    if (context && typeof context.helper === 'function') {
      context.removeHelper(getVNodeHelper(context.inSSR, node.isComponent));
      context.helper(OPEN_BLOCK);
      context.helper(getVNodeBlockHelper(context.inSSR, node.isComponent));
    }
  }
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

function nodeRelation(parent, child) {
  if (!parent || !child || typeof parent !== 'object') return undefined;
  for (const key of Object.keys(parent)) {
    if (key === 'loc' || key === 'parent') continue;
    const value = parent[key];
    if (value === child) return key;
    if (Array.isArray(value) && value.includes(child)) return key;
  }
  return undefined;
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

Object.defineProperty(module.exports, '__vuecRuntime', {
  value: {
    ...module.exports,
    transformFor,
    transformIf,
    transformOnce,
    transformSlotOutlet,
    transformText,
  },
  enumerable: false,
});
