/* vuec-runtime-fragment: callback-boundary/provenance-runtime */
const VUEC_PROVENANCE_STATE = '__vuecProvenanceState';
function vuecProvenanceState() {
  const root = globalThis;
  if (!root[VUEC_PROVENANCE_STATE]) {
    Object.defineProperty(root, VUEC_PROVENANCE_STATE, {
      value: { markers: new Set() },
      configurable: true,
      enumerable: false,
    });
  }
  return root[VUEC_PROVENANCE_STATE];
}
function recordVuecProvenance(marker) {
  if (marker === undefined || marker === null) return;
  const value = String(marker);
  if (!value) return;
  vuecProvenanceState().markers.add(value);
}
function flushVuecProvenance() {
  const state = vuecProvenanceState();
  const markers = Array.from(state.markers);
  state.markers.clear();
  return markers;
}
function peekVuecProvenance() {
  return Array.from(vuecProvenanceState().markers);
}
function markVuecRuntimeCallback(callback) {
  if (typeof callback === 'function' && !callback.__vuecRuntimeCallback) {
    Object.defineProperty(callback, '__vuecRuntimeCallback', {
      value: true,
      configurable: true,
      enumerable: false,
    });
  }
  return callback;
}
function isVuecRuntimeCallback(callback) {
  return !!(callback && callback.__vuecRuntimeCallback);
}
function recordVuecExternalCallback(marker, callback) {
  if (typeof callback === 'function' && !isVuecRuntimeCallback(callback)) {
    recordVuecProvenance(marker);
  }
}
Object.defineProperty(globalThis, '__vuecRecordProvenance', { value: recordVuecProvenance, configurable: true });
Object.defineProperty(globalThis, '__vuecFlushProvenance', { value: flushVuecProvenance, configurable: true });
Object.defineProperty(globalThis, '__vuecPeekProvenance', { value: peekVuecProvenance, configurable: true });

/* vuec-runtime-fragment: semantic-js-shim/vue3-core-runtime */
const vue3CoreRuntime = (() => {
  const enumObject = entries => {
    const out = {};
    for (const [key, value] of entries) {
      out[key] = value;
      out[value] = key;
    }
    return out;
  };
  const NodeTypes = enumObject([
    ['ROOT', 0], ['ELEMENT', 1], ['TEXT', 2], ['COMMENT', 3],
    ['SIMPLE_EXPRESSION', 4], ['INTERPOLATION', 5], ['ATTRIBUTE', 6],
    ['DIRECTIVE', 7], ['COMPOUND_EXPRESSION', 8], ['IF', 9],
    ['IF_BRANCH', 10], ['FOR', 11], ['TEXT_CALL', 12],
    ['VNODE_CALL', 13], ['JS_CALL_EXPRESSION', 14],
    ['JS_OBJECT_EXPRESSION', 15], ['JS_PROPERTY', 16],
    ['JS_ARRAY_EXPRESSION', 17], ['JS_FUNCTION_EXPRESSION', 18],
    ['JS_CONDITIONAL_EXPRESSION', 19], ['JS_CACHE_EXPRESSION', 20],
    ['JS_BLOCK_STATEMENT', 21], ['JS_TEMPLATE_LITERAL', 22],
    ['JS_IF_STATEMENT', 23], ['JS_ASSIGNMENT_EXPRESSION', 24],
    ['JS_SEQUENCE_EXPRESSION', 25], ['JS_RETURN_STATEMENT', 26],
  ]);
  const ElementTypes = enumObject([
    ['ELEMENT', 0], ['COMPONENT', 1], ['SLOT', 2], ['TEMPLATE', 3],
  ]);
  const ConstantTypes = enumObject([
    ['NOT_CONSTANT', 0], ['CAN_SKIP_PATCH', 1], ['CAN_CACHE', 2], ['CAN_STRINGIFY', 3],
  ]);
  const Namespaces = enumObject([
    ['HTML', 0], ['SVG', 1], ['MATH_ML', 2],
  ]);
  const ErrorCodes = enumObject([
    ['ABRUPT_CLOSING_OF_EMPTY_COMMENT', 0],
    ['CDATA_IN_HTML_CONTENT', 1],
    ['DUPLICATE_ATTRIBUTE', 2],
    ['END_TAG_WITH_ATTRIBUTES', 3],
    ['END_TAG_WITH_TRAILING_SOLIDUS', 4],
    ['EOF_BEFORE_TAG_NAME', 5],
    ['EOF_IN_CDATA', 6],
    ['EOF_IN_COMMENT', 7],
    ['EOF_IN_SCRIPT_HTML_COMMENT_LIKE_TEXT', 8],
    ['EOF_IN_TAG', 9],
    ['INCORRECTLY_CLOSED_COMMENT', 10],
    ['INCORRECTLY_OPENED_COMMENT', 11],
    ['INVALID_FIRST_CHARACTER_OF_TAG_NAME', 12],
    ['MISSING_ATTRIBUTE_VALUE', 13],
    ['MISSING_END_TAG_NAME', 14],
    ['MISSING_WHITESPACE_BETWEEN_ATTRIBUTES', 15],
    ['NESTED_COMMENT', 16],
    ['UNEXPECTED_CHARACTER_IN_ATTRIBUTE_NAME', 17],
    ['UNEXPECTED_CHARACTER_IN_UNQUOTED_ATTRIBUTE_VALUE', 18],
    ['UNEXPECTED_EQUALS_SIGN_BEFORE_ATTRIBUTE_NAME', 19],
    ['UNEXPECTED_NULL_CHARACTER', 20],
    ['UNEXPECTED_QUESTION_MARK_INSTEAD_OF_TAG_NAME', 21],
    ['UNEXPECTED_SOLIDUS_IN_TAG', 22],
    ['X_INVALID_END_TAG', 23],
    ['X_MISSING_END_TAG', 24],
    ['X_MISSING_INTERPOLATION_END', 25],
    ['X_MISSING_DIRECTIVE_NAME', 26],
    ['X_MISSING_DYNAMIC_DIRECTIVE_ARGUMENT_END', 27],
    ['X_V_IF_NO_EXPRESSION', 28],
    ['X_V_IF_SAME_KEY', 29],
    ['X_V_ELSE_NO_ADJACENT_IF', 30],
    ['X_V_FOR_NO_EXPRESSION', 31],
    ['X_V_FOR_MALFORMED_EXPRESSION', 32],
    ['X_V_FOR_TEMPLATE_KEY_PLACEMENT', 33],
    ['X_V_BIND_NO_EXPRESSION', 34],
    ['X_V_ON_NO_EXPRESSION', 35],
    ['X_V_SLOT_UNEXPECTED_DIRECTIVE_ON_SLOT_OUTLET', 36],
    ['X_V_SLOT_MIXED_SLOT_USAGE', 37],
    ['X_V_SLOT_DUPLICATE_SLOT_NAMES', 38],
    ['X_V_SLOT_EXTRANEOUS_DEFAULT_SLOT_CHILDREN', 39],
    ['X_V_SLOT_MISPLACED', 40],
    ['X_V_MODEL_NO_EXPRESSION', 41],
    ['X_V_MODEL_MALFORMED_EXPRESSION', 42],
    ['X_V_MODEL_ON_SCOPE_VARIABLE', 43],
    ['X_V_MODEL_ON_PROPS', 44],
    ['X_V_MODEL_ON_CONST', 45],
    ['X_INVALID_EXPRESSION', 46],
    ['X_KEEP_ALIVE_INVALID_CHILDREN', 47],
    ['X_PREFIX_ID_NOT_SUPPORTED', 48],
    ['X_MODULE_MODE_NOT_SUPPORTED', 49],
    ['X_CACHE_HANDLER_NOT_SUPPORTED', 50],
    ['X_SCOPE_ID_NOT_SUPPORTED', 51],
    ['X_VNODE_HOOKS', 52],
    ['X_V_BIND_INVALID_SAME_NAME_ARGUMENT', 53],
    ['__EXTEND_POINT__', 54],
  ]);
  const errorMessages = Object.fromEntries(
    Object.keys(ErrorCodes)
      .filter(key => /^\d+$/.test(key))
      .map(key => [Number(key), String(ErrorCodes[key] || '')])
  );
  errorMessages[23] = 'Invalid end tag.';
  errorMessages[24] = 'Element is missing end tag.';
  errorMessages[25] = 'Interpolation end sign was not found.';
  errorMessages[5] = 'Unexpected EOF in tag.';
  errorMessages[7] = 'Unexpected EOF in comment.';
  errorMessages[9] = 'Unexpected EOF in tag.';
  errorMessages[14] = 'End tag name was expected.';
  errorMessages[19] = "Attribute name cannot start with '='.";
  errorMessages[21] = "'<?' is allowed only in XML context.";
  errorMessages[22] = "Illegal '/' in tags.";
  errorMessages[27] = 'End bracket for dynamic directive argument was not found. Note that dynamic directive argument cannot contain spaces.';
  errorMessages[41] = 'v-model is missing expression.';
  errorMessages[42] = 'v-model value must be a valid JavaScript member expression.';
  errorMessages[43] = 'v-model cannot be used on v-for or v-slot scope variables because they are not writable.';
  errorMessages[44] = 'v-model cannot be used on a prop, because local prop bindings are not writable.\nUse a v-bind binding combined with a v-on listener that emits update:x event instead.';
  errorMessages[45] = 'v-model cannot be used on a const binding because it is not writable.';
  errorMessages[46] = 'Error parsing JavaScript expression: ';
  errorMessages[50] = '"cacheHandlers" option is only supported when the "prefixIdentifiers" option is enabled.';
  errorMessages[51] = '"scopeId" option is only supported in module mode.';

  const locStub = {
    start: { line: 1, column: 1, offset: 0 },
    end: { line: 1, column: 1, offset: 0 },
    source: '',
  };

  const helperNames = [
    ['FRAGMENT', 'Fragment'],
    ['TELEPORT', 'Teleport'],
    ['SUSPENSE', 'Suspense'],
    ['KEEP_ALIVE', 'KeepAlive'],
    ['BASE_TRANSITION', 'BaseTransition'],
    ['TRANSITION', 'Transition'],
    ['TRANSITION_GROUP', 'TransitionGroup'],
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
    ['V_MODEL_RADIO', 'vModelRadio'],
    ['V_MODEL_CHECKBOX', 'vModelCheckbox'],
    ['V_MODEL_TEXT', 'vModelText'],
    ['V_MODEL_SELECT', 'vModelSelect'],
    ['V_MODEL_DYNAMIC', 'vModelDynamic'],
    ['V_ON_WITH_MODIFIERS', 'withModifiers'],
    ['V_ON_WITH_KEYS', 'withKeys'],
    ['V_SHOW', 'vShow'],
  ];
  const runtime = {
    NodeTypes,
    ElementTypes,
    ConstantTypes,
    Namespaces,
    ErrorCodes,
    BindingTypes: {
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
    },
    CompilerDeprecationTypes: {
      COMPILER_IS_ON_ELEMENT: 'COMPILER_IS_ON_ELEMENT',
      COMPILER_V_BIND_SYNC: 'COMPILER_V_BIND_SYNC',
      COMPILER_V_BIND_OBJECT_ORDER: 'COMPILER_V_BIND_OBJECT_ORDER',
      COMPILER_V_ON_NATIVE: 'COMPILER_V_ON_NATIVE',
      COMPILER_V_IF_V_FOR_PRECEDENCE: 'COMPILER_V_IF_V_FOR_PRECEDENCE',
      COMPILER_NATIVE_TEMPLATE: 'COMPILER_NATIVE_TEMPLATE',
      COMPILER_INLINE_TEMPLATE: 'COMPILER_INLINE_TEMPLATE',
      COMPILER_FILTERS: 'COMPILER_FILTERS',
    },
    TS_NODE_TYPES: [
      'TSAsExpression',
      'TSTypeAssertion',
      'TSNonNullExpression',
      'TSInstantiationExpression',
      'TSSatisfiesExpression',
    ],
    locStub,
    errorMessages,
    helperNameMap: {},
    forAliasRE: /([\s\S]*?)\s+(?:in|of)\s+(\S[\s\S]*)/,
    validFirstIdentCharRE: /[A-Za-z_$]/,
  };
  for (const [key, name] of helperNames) {
    const symbol = Symbol(name);
    runtime[key] = symbol;
    runtime.helperNameMap[symbol] = name;
  }

  runtime.advancePositionWithClone = function advancePositionWithClone(pos, source, numberOfCharacters) {
    return callBridge('vue3.core.advancePositionWithClone', {
      pos: runtime.dehydrateForBridge(pos),
      source: String(source || ''),
      numberOfCharacters: numberOfCharacters === undefined ? undefined : numberOfCharacters,
    });
  };
  runtime.advancePositionWithMutation = function advancePositionWithMutation(pos, source, numberOfCharacters) {
    const projection = callBridge('vue3.core.advancePositionWithMutation', {
      pos: runtime.dehydrateForBridge(pos),
      source: String(source || ''),
      numberOfCharacters: numberOfCharacters === undefined ? undefined : numberOfCharacters,
    });
    pos.offset = projection.offset;
    pos.line = projection.line;
    pos.column = projection.column;
    return pos;
  };
  runtime.assert = function assert(condition, msg) {
    if (!condition) throw new Error(msg || 'unexpected compiler condition');
  };
  runtime.createRoot = function createRoot(children, source = '') {
    return hydrateVue3Ast({
      type: NodeTypes.ROOT,
      source,
      children,
      helpers: [],
      components: [],
      directives: [],
      hoists: [],
      imports: [],
      cached: [],
      temps: 0,
      codegenNode: null,
      loc: locStub,
    });
  };
  runtime.createSimpleExpression = function createSimpleExpression(content, isStatic = false, loc = locStub, constType = ConstantTypes.NOT_CONSTANT) {
    return {
      type: NodeTypes.SIMPLE_EXPRESSION,
      loc,
      content,
      isStatic,
      constType: isStatic ? ConstantTypes.CAN_STRINGIFY : constType,
    };
  };
  runtime.createInterpolation = function createInterpolation(content, loc) {
    return {
      type: NodeTypes.INTERPOLATION,
      loc,
      content: typeof content === 'string' ? runtime.createSimpleExpression(content, false, loc) : content,
    };
  };
  runtime.createCompoundExpression = function createCompoundExpression(children, loc = locStub) {
    return { type: NodeTypes.COMPOUND_EXPRESSION, loc, children };
  };
  runtime.createArrayExpression = function createArrayExpression(elements, loc = locStub) {
    return { type: NodeTypes.JS_ARRAY_EXPRESSION, loc, elements };
  };
  runtime.createObjectExpression = function createObjectExpression(properties, loc = locStub) {
    return { type: NodeTypes.JS_OBJECT_EXPRESSION, loc, properties };
  };
  runtime.createObjectProperty = function createObjectProperty(key, value) {
    return {
      type: NodeTypes.JS_PROPERTY,
      loc: locStub,
      key: typeof key === 'string' ? runtime.createSimpleExpression(key, true) : key,
      value,
    };
  };
  runtime.createCallExpression = function createCallExpression(callee, args = [], loc = locStub) {
    return { type: NodeTypes.JS_CALL_EXPRESSION, loc, callee, arguments: args };
  };
  runtime.createFunctionExpression = function createFunctionExpression(params, returns = undefined, newline = false, isSlot = false, loc = locStub) {
    return { type: NodeTypes.JS_FUNCTION_EXPRESSION, params, returns, newline, isSlot, loc };
  };
  runtime.createConditionalExpression = function createConditionalExpression(test, consequent, alternate, newline = true) {
    return { type: NodeTypes.JS_CONDITIONAL_EXPRESSION, test, consequent, alternate, newline, loc: locStub };
  };
  runtime.createCacheExpression = function createCacheExpression(index, value, needPauseTracking = false, inVOnce = false) {
    return { type: NodeTypes.JS_CACHE_EXPRESSION, index, value, needPauseTracking, inVOnce, needArraySpread: false, loc: locStub };
  };
  runtime.createBlockStatement = function createBlockStatement(body) {
    return { type: NodeTypes.JS_BLOCK_STATEMENT, body, loc: locStub };
  };
  runtime.createTemplateLiteral = function createTemplateLiteral(elements) {
    return { type: NodeTypes.JS_TEMPLATE_LITERAL, elements, loc: locStub };
  };
  runtime.createIfStatement = function createIfStatement(test, consequent, alternate) {
    return { type: NodeTypes.JS_IF_STATEMENT, test, consequent, alternate, loc: locStub };
  };
  runtime.createAssignmentExpression = function createAssignmentExpression(left, right) {
    return { type: NodeTypes.JS_ASSIGNMENT_EXPRESSION, left, right, loc: locStub };
  };
  runtime.createSequenceExpression = function createSequenceExpression(expressions) {
    return { type: NodeTypes.JS_SEQUENCE_EXPRESSION, expressions, loc: locStub };
  };
  runtime.createReturnStatement = function createReturnStatement(returns) {
    return { type: NodeTypes.JS_RETURN_STATEMENT, returns, loc: locStub };
  };
  runtime.createVNodeCall = function createVNodeCall(context, tag, props, children, patchFlag, dynamicProps, directives, isBlock = false, disableTracking = false, isComponent = false, loc = locStub) {
    if (context) {
      if (isBlock) {
        context.helper(runtime.OPEN_BLOCK);
        context.helper(runtime.getVNodeBlockHelper(context.inSSR, isComponent));
      } else {
        context.helper(runtime.getVNodeHelper(context.inSSR, isComponent));
      }
      if (directives) {
        context.helper(runtime.WITH_DIRECTIVES);
      }
    }
    return {
      type: NodeTypes.VNODE_CALL,
      tag,
      props,
      children,
      patchFlag,
      dynamicProps,
      directives,
      isBlock,
      disableTracking,
      isComponent,
      loc,
    };
  };
  runtime.getVNodeHelper = function getVNodeHelper(ssr, isComponent) {
    return ssr || isComponent ? runtime.CREATE_VNODE : runtime.CREATE_ELEMENT_VNODE;
  };
  runtime.getVNodeBlockHelper = function getVNodeBlockHelper(ssr, isComponent) {
    return ssr || isComponent ? runtime.CREATE_BLOCK : runtime.CREATE_ELEMENT_BLOCK;
  };
  runtime.convertToBlock = function convertToBlock(node, context) {
    if (!node.isBlock) {
      node.isBlock = true;
      context.removeHelper(runtime.getVNodeHelper(context.inSSR, node.isComponent));
      context.helper(runtime.OPEN_BLOCK);
      context.helper(runtime.getVNodeBlockHelper(context.inSSR, node.isComponent));
    }
  };
  runtime.createCompilerError = function createCompilerError(code, loc, messages, additionalMessage) {
    const error = new SyntaxError(String((messages || errorMessages)[code] || '') + (additionalMessage || ''));
    error.code = code;
    error.loc = loc;
    return error;
  };
  runtime.registerRuntimeHelpers = function registerRuntimeHelpers(helpers) {
    Object.getOwnPropertySymbols(helpers).forEach(symbol => {
      runtime.helperNameMap[symbol] = helpers[symbol];
      const name = Object.getOwnPropertyDescriptor(symbol, 'description') && symbol.description;
      if (name && !runtime[name]) runtime[name] = symbol;
    });
  };
  runtime.stringifyExpression = function stringifyExpression(exp) {
    return typeof exp === 'string'
      ? exp
      : exp && exp.type === NodeTypes.SIMPLE_EXPRESSION
        ? exp.content
        : exp && Array.isArray(exp.children)
          ? exp.children.map(runtime.stringifyExpression).join('')
          : exp && exp.loc
            ? exp.loc.source
            : '';
  };
  runtime.isStaticExp = function isStaticExp(p) {
    return !!(p && p.type === NodeTypes.SIMPLE_EXPRESSION && p.isStatic);
  };
  runtime.dehydrateForBridge = function dehydrateForBridge(value, seen = new WeakSet()) {
    if (value == null || typeof value !== 'object') return typeof value === 'symbol' ? projectionNameFromHelperSymbol(value) : value;
    if (typeof value === 'symbol') return projectionNameFromHelperSymbol(value);
    if (seen.has(value)) return undefined;
    seen.add(value);
    if (value instanceof Set) {
      const out = Array.from(value, item => runtime.dehydrateForBridge(item, seen));
      seen.delete(value);
      return out;
    }
    if (Array.isArray(value)) {
      const out = value.map(item => runtime.dehydrateForBridge(item, seen));
      seen.delete(value);
      return out;
    }
    const out = {};
    for (const key of Object.keys(value)) {
      if (key === 'loc' || key === 'start' || key === 'end' || key === 'offset' || key === 'line' || key === 'column' || key === 'type' || key === 'tag' || key === 'tagType' || key === 'content' || key === 'isStatic' || key === 'constType' || key === 'props' || key === 'children' || key === 'codegenNode' || key === 'patchFlag' || key === 'dynamicProps' || key === 'directives' || key === 'isBlock' || key === 'isComponent' || key === 'disableTracking' || key === 'branches' || key === 'source' || key === 'transformed' || key === 'parseResult' || key === 'valueAlias' || key === 'keyAlias' || key === 'objectIndexAlias' || key === 'returns' || key === 'body' || key === 'params' || key === 'newline' || key === 'isSlot' || key === 'isNonScopedSlot' || key === 'needPauseTracking' || key === 'inVOnce' || key === 'needArraySpread' || key === 'index' || key === 'elements' || key === 'test' || key === 'consequent' || key === 'alternate' || key === 'left' || key === 'right' || key === 'expressions' || key === 'expression' || key === 'helpers' || key === 'ssrHelpers' || key === 'components' || key === 'directives' || key === 'imports' || key === 'path' || key === 'hoists' || key === 'cached' || key === 'temps' || key === 'properties' || key === 'key' || key === 'value' || key === 'arguments' || key === 'argument' || key === 'callee' || key === 'object' || key === 'property' || key === 'name' || key === 'arg' || key === 'exp' || key === 'modifiers' || key === 'program' || key === 'declarations' || key === 'declaration' || key === 'id' || key === 'init' || key === 'update' || key === 'computed' || key === 'shorthand' || key === 'kind' || key === 'declare' || key === 'operator' || key === 'prefix' || key === 'async' || key === 'cases' || key === 'discriminant' || key === 'handler' || key === 'finalizer' || key === 'block' || key === 'param' || key === 'parameter' || key === 'specifiers' || key === 'local' || key === 'imported' || key === 'superClass' || key === 'quasi') {
        out[key] = runtime.dehydrateForBridge(value[key], seen);
      }
    }
    seen.delete(value);
    return out;
  };
  runtime.isText = function isText$1(node) {
    return !!node && (node.type === NodeTypes.INTERPOLATION || node.type === NodeTypes.TEXT);
  };
  runtime.isAllWhitespace = function isAllWhitespace(str) {
    return /^[\t\r\n\f ]*$/.test(String(str || ''));
  };
  runtime.isWhitespaceText = function isWhitespaceText(node) {
    return !!node && ((node.type === NodeTypes.TEXT && runtime.isAllWhitespace(node.content)) || (node.type === NodeTypes.TEXT_CALL && runtime.isWhitespaceText(node.content)));
  };
  runtime.isCommentOrWhitespace = function isCommentOrWhitespace(node) {
    return !!node && (node.type === NodeTypes.COMMENT || runtime.isWhitespaceText(node));
  };
  runtime.findDir = function findDir(node, name, allowEmpty = false) {
    const matches = typeof name === 'string' ? n => n === name : n => name.test(n);
    return node.props && node.props.find(p => p.type === NodeTypes.DIRECTIVE && (allowEmpty || p.exp) && matches(p.name));
  };
  runtime.findProp = function findProp(node, name, dynamicOnly = false, allowEmpty = false) {
    if (!node.props) return undefined;
    for (const p of node.props) {
      if (p.type === NodeTypes.ATTRIBUTE) {
        if (!dynamicOnly && p.name === name && (p.value || allowEmpty)) return p;
      } else if (p.name === 'bind' && (p.exp || allowEmpty) && runtime.isStaticArgOf(p.arg, name)) {
        return p;
      }
    }
    return undefined;
  };
  runtime.isStaticArgOf = function isStaticArgOf(arg, name) {
    return !!(arg && runtime.isStaticExp(arg) && arg.content === name);
  };
  runtime.hasDynamicKeyVBind = function hasDynamicKeyVBind(node) {
    return !!(node.props && node.props.some(p => p.type === NodeTypes.DIRECTIVE && p.name === 'bind' && (!p.arg || p.arg.type !== NodeTypes.SIMPLE_EXPRESSION || !p.arg.isStatic)));
  };
  runtime.isVPre = function isVPre(p) { return !!p && p.type === NodeTypes.DIRECTIVE && p.name === 'pre'; };
  runtime.isVSlot = function isVSlot(p) { return !!p && p.type === NodeTypes.DIRECTIVE && p.name === 'slot'; };
  runtime.isTemplateNode = function isTemplateNode(node) { return !!node && node.type === NodeTypes.ELEMENT && node.tagType === ElementTypes.TEMPLATE; };
  runtime.isSlotOutlet = function isSlotOutlet(node) { return !!node && node.type === NodeTypes.ELEMENT && node.tagType === ElementTypes.SLOT; };
  runtime.toValidAssetId = function toValidAssetId(name, type) {
    const projection = callBridge('vue3.core.toValidAssetId', { name: String(name), type: String(type) });
    return projection && projection.id || '';
  };
  runtime.injectProp = function injectProp(node, prop) {
    let props = node.type === NodeTypes.VNODE_CALL ? node.props : node.arguments && node.arguments[2];
    let callPath = [];
    let parentCall;
    if (props && typeof props !== 'string' && props.type === NodeTypes.JS_CALL_EXPRESSION) {
      const ret = runtime.getUnnormalizedProps(props);
      props = ret[0];
      callPath = ret[1];
      parentCall = callPath[callPath.length - 1];
    }
    let propsWithInjection;
    if (!props || typeof props === 'string') {
      propsWithInjection = runtime.createObjectExpression([prop]);
    } else if (props.type === NodeTypes.JS_CALL_EXPRESSION) {
      const first = props.arguments && props.arguments[0];
      if (first && typeof first !== 'string' && first.type === NodeTypes.JS_OBJECT_EXPRESSION) {
        runtime.prependPropOnce(first, prop);
      } else if (props.callee === runtime.TO_HANDLERS) {
        propsWithInjection = runtime.createCallExpression(runtime.MERGE_PROPS, [runtime.createObjectExpression([prop]), props]);
      } else {
        props.arguments.unshift(runtime.createObjectExpression([prop]));
      }
      if (!propsWithInjection) propsWithInjection = props;
    } else if (props.type === NodeTypes.JS_OBJECT_EXPRESSION) {
      runtime.prependPropOnce(props, prop);
      propsWithInjection = props;
    } else {
      propsWithInjection = runtime.createCallExpression(runtime.MERGE_PROPS, [runtime.createObjectExpression([prop]), props]);
      if (parentCall && parentCall.callee === runtime.GUARD_REACTIVE_PROPS) {
        parentCall = callPath[callPath.length - 2];
      }
    }
    if (node.type === NodeTypes.JS_CALL_EXPRESSION && node.callee === runtime.RENDER_SLOT && node.arguments) {
      node.arguments[2] = propsWithInjection;
    } else if (node.type === NodeTypes.VNODE_CALL) {
      if (parentCall) parentCall.arguments[0] = propsWithInjection;
      else node.props = propsWithInjection;
    } else if (node.arguments) {
      if (parentCall) parentCall.arguments[0] = propsWithInjection;
      else node.arguments[2] = propsWithInjection;
    }
  };
  runtime.getUnnormalizedProps = function getUnnormalizedProps(props, callPath = []) {
    if (props && typeof props !== 'string' && props.type === NodeTypes.JS_CALL_EXPRESSION) {
      if (props.callee === runtime.NORMALIZE_PROPS || props.callee === runtime.GUARD_REACTIVE_PROPS) {
        return runtime.getUnnormalizedProps(props.arguments[0], callPath.concat(props));
      }
    }
    return [props, callPath];
  };
  runtime.prependPropOnce = function prependPropOnce(props, prop) {
    const keyName = runtime.staticPropertyKeyName(prop);
    if (!keyName || !(props.properties || []).some(existing => runtime.staticPropertyKeyName(existing) === keyName)) {
      props.properties.unshift(prop);
    }
  };
  runtime.prependPropsExpressionProp = function prependPropsExpressionProp(props, prop, loc = locStub) {
    if (!props || typeof props === 'string') return runtime.createObjectExpression([prop], loc);
    if (props.type === NodeTypes.JS_OBJECT_EXPRESSION) {
      runtime.prependPropOnce(props, prop);
      return props;
    }
    const objectArg = runtime.createObjectExpression([prop], loc);
    if (props.type === NodeTypes.JS_CALL_EXPRESSION && props.callee === runtime.MERGE_PROPS) {
      const first = props.arguments && props.arguments[0];
      if (first && typeof first !== 'string' && first.type === NodeTypes.JS_OBJECT_EXPRESSION) {
        runtime.prependPropOnce(first, prop);
      } else {
        props.arguments.unshift(objectArg);
      }
      return props;
    }
    return runtime.createCallExpression(runtime.MERGE_PROPS, [objectArg, props], loc);
  };
  runtime.applyInlineTemplateRefProjection = function applyInlineTemplateRefProjection(props, refs, loc = locStub) {
    for (const ref of refs || []) {
      const content = ref && ref.content;
      if (!content) continue;
      props = runtime.prependPropsExpressionProp(
        props,
        runtime.createObjectProperty('ref_key', runtime.createSimpleExpression(content, true, loc)),
        loc,
      );
      for (const object of runtime.propsExpressionObjects(props)) {
        for (const prop of object.properties || []) {
          if (runtime.staticPropertyKeyName(prop) === 'ref' && prop.value && prop.value.type === NodeTypes.SIMPLE_EXPRESSION && prop.value.content === content) {
            prop.value.isStatic = false;
            prop.value.constType = ConstantTypes.NOT_CONSTANT;
          }
        }
      }
    }
    return props;
  };
  runtime.propsExpressionObjects = function propsExpressionObjects(props) {
    if (!props || typeof props === 'string') return [];
    if (props.type === NodeTypes.JS_OBJECT_EXPRESSION) return [props];
    if (props.type === NodeTypes.JS_CALL_EXPRESSION) {
      return (props.arguments || []).flatMap(arg => runtime.propsExpressionObjects(arg));
    }
    return [];
  };
  runtime.dedupeProperties = function dedupeProperties(properties) {
    const known = new Map();
    const deduped = [];
    for (const prop of properties || []) {
      const keyName = runtime.staticPropertyKeyName(prop);
      if (!keyName) {
        deduped.push(prop);
        continue;
      }
      const existing = known.get(keyName);
      if (existing) {
        if (keyName === 'class' || keyName === 'style' || /^on[A-Z]/.test(keyName)) {
          runtime.mergePropertyAsArray(existing, prop);
        }
      } else {
        known.set(keyName, prop);
        deduped.push(prop);
      }
    }
    return deduped;
  };
  runtime.mergePropertyAsArray = function mergePropertyAsArray(existing, incoming) {
    if (existing.value && existing.value.type === NodeTypes.JS_ARRAY_EXPRESSION) {
      existing.value.elements.push(incoming.value);
    } else {
      existing.value = runtime.createArrayExpression([existing.value, incoming.value], existing.loc || locStub);
    }
  };
  runtime.staticPropertyKeyName = function staticPropertyKeyName(prop) {
    const key = prop && prop.key;
    return key && key.type === NodeTypes.SIMPLE_EXPRESSION && key.isStatic ? key.content : undefined;
  };
  runtime.normalizeObjectProp = function normalizeObjectProp(props, name, helper) {
    let target = props;
    if (target && target.type === NodeTypes.JS_CALL_EXPRESSION && target.callee === runtime.NORMALIZE_PROPS) {
      target = target.arguments && target.arguments[0];
    }
    if (!target || target.type !== NodeTypes.JS_OBJECT_EXPRESSION) return;
    const prop = (target.properties || []).find(property => runtime.staticPropertyKeyName(property) === name);
    if (prop && prop.value && !runtime.isStaticExp(prop.value) && !(prop.value.type === NodeTypes.JS_CALL_EXPRESSION && prop.value.callee === helper)) {
      prop.value = runtime.createCallExpression(helper, [prop.value], prop.value.loc || prop.loc || locStub);
    }
  };
  runtime.hasScopeRef = function hasScopeRef(node, identifiers = {}) {
    const names = Object.keys(identifiers).filter(name => identifiers[name] > 0);
    if (!names.length) return false;
    const source = runtime.stringifyExpression(node);
    return names.some(name => source.includes(name));
  };
  runtime.expressionIdentifierNames = function expressionIdentifierNames(exp) {
    if (!exp) return [];
    if (typeof exp === 'string') return exp ? [exp] : [];
    if (Array.isArray(exp.identifiers)) return exp.identifiers.filter(Boolean);
    if (exp.type === NodeTypes.SIMPLE_EXPRESSION && exp.content) return [exp.content];
    return [];
  };
  runtime.getMemoedVNodeCall = function getMemoedVNodeCall(node) {
    return node && node.type === NodeTypes.JS_CALL_EXPRESSION && node.callee === runtime.WITH_MEMO ? node.arguments[1].returns : node;
  };
  runtime.isCoreComponent = function isCoreComponent(tag) {
    return tag === 'Teleport' || tag === 'teleport' ? runtime.TELEPORT
      : tag === 'Suspense' || tag === 'suspense' ? runtime.SUSPENSE
      : tag === 'KeepAlive' || tag === 'keep-alive' ? runtime.KEEP_ALIVE
      : tag === 'BaseTransition' || tag === 'base-transition' ? runtime.BASE_TRANSITION
      : undefined;
  };
  runtime.isBuiltInDirective = function isBuiltInDirective(name) {
    return new Set(['bind', 'cloak', 'else-if', 'else', 'for', 'html', 'if', 'model', 'on', 'once', 'pre', 'show', 'slot', 'text', 'memo']).has(String(name || ''));
  };
  runtime.isSimpleIdentifier = function isSimpleIdentifier(name) {
    return /^[A-Za-z_$][\w$]*$/.test(String(name || ''));
  };
  runtime.isGloballyAllowed = function isGloballyAllowed(name) {
    return new Set([
      'Infinity', 'NaN', 'undefined', 'parseInt', 'parseFloat', 'isNaN', 'isFinite',
      'decodeURI', 'decodeURIComponent', 'encodeURI', 'encodeURIComponent',
      'Math', 'Number', 'Date', 'Array', 'Object', 'Boolean', 'String', 'RegExp',
      'Map', 'Set', 'WeakMap', 'WeakSet', 'JSON', 'Intl', 'BigInt', 'console',
      'Error', 'TypeError', 'Symbol', 'Promise', 'Reflect', 'globalThis',
    ]).has(String(name || ''));
  };
  runtime.getBabelParser = function getBabelParser() {
    if (runtime._babelParser !== undefined) return runtime._babelParser;
    try {
      runtime._babelParser = require('@babel/parser');
    } catch (_error) {
      try {
        runtime._babelParser = process.env.VUEC_OFFICIAL_NPM_ROOT
          ? require(path.join(process.env.VUEC_OFFICIAL_NPM_ROOT, 'node_modules/@babel/parser'))
          : null;
      } catch (_fallbackError) {
        runtime._babelParser = null;
      }
    }
    return runtime._babelParser;
  };
  runtime.isMemberExpressionBrowser = function isMemberExpressionBrowser(path) {
    const projection = callBridge('vue3.core.isMemberExpression', {
      mode: 'browser',
      node: runtime.dehydrateForBridge(path),
      context: {},
    });
    return !!(projection && projection.isMemberExpression);
  };
  runtime.isMemberExpressionNode = function isMemberExpressionNode(path, context = {}) {
    const projection = callBridge('vue3.core.isMemberExpression', {
      mode: 'node',
      node: runtime.dehydrateForBridge(path),
      context: vue3ExpressionUtilityContextPayload(context),
    });
    return !!(projection && projection.isMemberExpression);
  };
  runtime.isMemberExpression = runtime.isMemberExpressionNode;
  runtime.isFnExpressionBrowser = function isFnExpressionBrowser(exp) {
    const content = typeof exp === 'string' ? exp : exp && exp.content;
    return /^\s*(?:async\s*)?(?:\([^)]*\)|[A-Za-z_$][\w$]*)[\s\S]*=>/.test(String(content || '')) || /^\s*(?:async\s+)?function\b/.test(String(content || ''));
  };
  runtime.isFnExpressionNode = function isFnExpressionNode(exp) { return runtime.isFnExpressionBrowser(exp); };
  runtime.isFnExpression = runtime.isFnExpressionNode;
  runtime.isFunctionType = function isFunctionType(node) {
    const projection = callBridge('vue3.core.isFunctionType', {
      node: runtime.dehydrateForBridge(node),
    });
    return !!(projection && projection.isFunctionType);
  };
  runtime.nodeAtBridgePath = function nodeAtBridgePath(root, path) {
    let node = root;
    for (const segment of path || []) {
      if (node == null) return undefined;
      node = node[segment];
    }
    return node;
  };
  runtime.bridgePathForChild = function bridgePathForChild(parent, child) {
    if (!parent || !child || typeof parent !== 'object') return undefined;
    for (const key of Object.keys(parent)) {
      const value = parent[key];
      if (value === child) return [key];
      if (Array.isArray(value)) {
        const index = value.indexOf(child);
        if (index !== -1) return [key, index];
      }
    }
    return undefined;
  };
  runtime.bridgeRelationForChild = function bridgeRelationForChild(parent, child) {
    const path = runtime.bridgePathForChild(parent, child);
    return path && typeof path[0] === 'string' ? path[0] : undefined;
  };
  runtime.isStaticProperty = function isStaticProperty(node) {
    const projection = callBridge('vue3.core.isStaticProperty', {
      node: runtime.dehydrateForBridge(node),
    });
    return !!(projection && projection.isStaticProperty);
  };
  runtime.isStaticPropertyKey = function isStaticPropertyKey(node, parent) { return !!parent && runtime.isStaticProperty(parent) && parent.key === node; };
  runtime.unwrapTSNode = function unwrapTSNode(node) {
    while (node && runtime.TS_NODE_TYPES.includes(node.type)) node = node.expression;
    return node;
  };
  runtime.isReferencedIdentifier = function isReferencedIdentifier(id, parent, parentStack = []) {
    const projection = callBridge('vue3.core.isReferencedIdentifier', {
      node: runtime.dehydrateForBridge(id),
      parent: runtime.dehydrateForBridge(parent),
      parentStack: runtime.dehydrateForBridge(parentStack),
      relation: runtime.bridgeRelationForChild(parent, id),
    });
    return !!(projection && projection.isReferencedIdentifier);
  };
  runtime.isInDestructureAssignment = function isInDestructureAssignment(parent, parentStack = []) {
    const projection = callBridge('vue3.core.isInDestructureAssignment', {
      parent: runtime.dehydrateForBridge(parent),
      parentStack: runtime.dehydrateForBridge(parentStack),
    });
    return !!(projection && projection.isInDestructureAssignment);
  };
  runtime.isInNewExpression = function isInNewExpression() { return false; };
  runtime.walkIdentifiers = function walkIdentifiers(root, onIdentifier, includeAll = false, parentStack = [], knownIds = Object.create(null)) {
    const projection = callBridge('vue3.core.walkIdentifiers', {
      root: runtime.dehydrateForBridge(root),
      includeAll: !!includeAll,
      knownIds: runtime.dehydrateForBridge(knownIds),
    });
    for (const event of (projection && projection.identifiers) || []) {
      const id = runtime.nodeAtBridgePath(root, event.path);
      const parent = runtime.nodeAtBridgePath(root, event.parentPath);
      const stack = (event.parentStackPaths || [])
        .map(path => runtime.nodeAtBridgePath(root, path))
        .filter(Boolean);
      if (id) onIdentifier(id, parent || null, stack.length ? stack : parentStack.slice(), !!event.isReferenced, !!event.isLocal);
    }
    if (projection && projection.knownIds) {
      for (const key of Object.keys(knownIds)) delete knownIds[key];
      Object.assign(knownIds, projection.knownIds);
    }
  };
  runtime.extractIdentifiers = function extractIdentifiers(param) {
    if (!param) return [];
    if (typeof param === 'string') return param.split(',').map(s => s.trim()).filter(Boolean).map(content => runtime.createSimpleExpression(content, false));
    if (param.type === NodeTypes.SIMPLE_EXPRESSION) return [param];
    const projection = callBridge('vue3.core.extractIdentifiers', {
      node: runtime.dehydrateForBridge(param),
    });
    return ((projection && projection.identifiers) || [])
      .map(item => runtime.nodeAtBridgePath(param, item.path))
      .filter(Boolean);
  };
  runtime.walkFunctionParams = function walkFunctionParams(node, onIdent) {
    for (const ident of runtime.extractIdentifiers(node && node.params)) onIdent(ident);
  };
  runtime.extractBabelIdentifiers = function extractBabelIdentifiers(node) {
    if (!node) return [];
    if (Array.isArray(node)) return node.flatMap(runtime.extractBabelIdentifiers);
    if (node.type === 'Identifier') return [node];
    if (runtime.TS_NODE_TYPES.includes(node.type)) return runtime.extractBabelIdentifiers(node.expression);
    if (node.type === 'MemberExpression') {
      let object = node;
      while (object && object.type === 'MemberExpression') object = object.object;
      return runtime.extractBabelIdentifiers(object);
    }
    if (node.type === 'ObjectPattern') return (node.properties || []).flatMap(runtime.extractBabelIdentifiers);
    if (node.type === 'ObjectProperty') {
      const out = [];
      if (node.computed && node.key) out.push(...runtime.extractBabelIdentifiers(node.key));
      if (node.value) out.push(...runtime.extractBabelIdentifiers(node.value));
      return out;
    }
    if (node.type === 'ArrayPattern') return (node.elements || []).flatMap(runtime.extractBabelIdentifiers);
    if (node.type === 'RestElement') return runtime.extractBabelIdentifiers(node.argument);
    if (node.type === 'AssignmentPattern') return runtime.extractBabelIdentifiers(node.left);
    return [];
  };
  runtime.walkBlockDeclarations = function walkBlockDeclarations() {};
  runtime.babelParse = function babelParse(source, options) {
    const parser = runtime.getBabelParser();
    if (!parser || typeof parser.parse !== 'function') throw new Error('@babel/parser is unavailable');
    return parser.parse(source, options);
  };
  runtime.createTransformContext = function createTransformContext(root, options = {}) {
    const canonicalHelpers = new Map();
    const canonicalHelper = name => {
      const helperName = runtime.helperNameMap[name];
      if (!helperName) return name;
      if (!canonicalHelpers.has(helperName)) canonicalHelpers.set(helperName, name);
      return canonicalHelpers.get(helperName);
    };
    const context = {
      filename: options.filename || '',
      selfName: options.filename ? selfNameFromFilename(options.filename) : null,
      prefixIdentifiers: !!options.prefixIdentifiers,
      hoistStatic: !!options.hoistStatic,
      hmr: !!options.hmr,
      cacheHandlers: !!options.cacheHandlers,
      nodeTransforms: options.nodeTransforms || [],
      directiveTransforms: options.directiveTransforms || {},
      transformHoist: options.transformHoist || null,
      isBuiltInComponent: options.isBuiltInComponent || (() => false),
      isCustomElement: options.isCustomElement || (() => false),
      expressionPlugins: options.expressionPlugins || [],
      scopeId: options.scopeId || null,
      slotted: options.slotted !== undefined ? options.slotted : true,
      ssr: !!options.ssr,
      inSSR: !!(options.inSSR || options.ssr),
      ssrCssVars: options.ssrCssVars || '',
      bindingMetadata: options.bindingMetadata || {},
      inline: !!options.inline,
      isTS: !!options.isTS,
      onError: options.onError || (error => { throw error; }),
      onWarn: options.onWarn || (() => {}),
      compatConfig: options.compatConfig,
      root,
      helpers: new Map(),
      components: new Set(),
      directives: new Set(),
      hoists: [],
      imports: [],
      cached: [],
      constantCache: new WeakMap(),
      temps: 0,
      identifiers: Object.create(null),
      scopes: { vFor: 0, vSlot: 0, vPre: 0, vOnce: 0 },
      parent: null,
      grandParent: null,
      currentNode: root,
      childIndex: 0,
      inVOnce: false,
      helper(name) {
        name = canonicalHelper(name);
        context.helpers.set(name, (context.helpers.get(name) || 0) + 1);
        return name;
      },
      removeHelper(name) {
        name = canonicalHelper(name);
        const count = context.helpers.get(name);
        if (count === 1) context.helpers.delete(name);
        else if (count) context.helpers.set(name, count - 1);
      },
      helperString(name) {
        return `_${runtime.helperNameMap[context.helper(name)]}`;
      },
      replaceNode(node) {
        recordVuecProvenance('js.transformContext.replaceNode');
        if (!context.currentNode) throw new Error('Node being replaced is already removed.');
        if (!context.parent) throw new Error('Cannot replace root node.');
        context.parent.children[context.childIndex] = context.currentNode = node;
      },
      removeNode(node) {
        recordVuecProvenance('js.transformContext.removeNode');
        if (!context.parent) throw new Error('Cannot remove root node.');
        const list = context.parent.children;
        const removalIndex = node ? list.indexOf(node) : context.currentNode ? context.childIndex : -1;
        if (removalIndex < 0) throw new Error('node being removed is not a child of current parent');
        if (!node || node === context.currentNode) {
          context.currentNode = null;
          context.onNodeRemoved();
        } else if (context.childIndex > removalIndex) {
          context.childIndex--;
          context.onNodeRemoved();
        }
        list.splice(removalIndex, 1);
      },
      onNodeRemoved() {},
      addIdentifiers(exp) {
        for (const name of runtime.expressionIdentifierNames(exp)) {
          context.identifiers[name] = (context.identifiers[name] || 0) + 1;
        }
      },
      removeIdentifiers(exp) {
        for (const name of runtime.expressionIdentifierNames(exp)) {
          if (!context.identifiers[name]) continue;
          context.identifiers[name]--;
          if (context.identifiers[name] <= 0) delete context.identifiers[name];
        }
      },
      hoist(exp) {
        recordVuecProvenance('js.transformContext.hoist');
        if (typeof exp === 'string') exp = runtime.createSimpleExpression(exp);
        context.hoists.push(exp);
        const identifier = runtime.createSimpleExpression(`_hoisted_${context.hoists.length}`, false, exp.loc, ConstantTypes.CAN_CACHE);
        identifier.hoisted = exp;
        return identifier;
      },
      cache(exp, isVNode = false, inVOnce = false) {
        recordVuecProvenance('js.transformContext.cache');
        const cacheExp = runtime.createCacheExpression(context.cached.length, exp, isVNode, inVOnce);
        context.cached.push(cacheExp);
        return cacheExp;
      },
      filters: new Set(),
    };
    return context;
  };
  runtime.traverseNode = function traverseNode(node, context) {
    context.currentNode = node;
    const exitFns = [];
    for (const transform of context.nodeTransforms || []) {
      recordVuecExternalCallback('callback.nodeTransform', transform);
      const onExit = transform(node, context);
      if (Array.isArray(onExit)) exitFns.push(...onExit);
      else if (onExit) exitFns.push(onExit);
      if (!context.currentNode) return;
      node = context.currentNode;
    }
    switch (node.type) {
      case NodeTypes.COMMENT:
        if (!context.ssr) context.helper(runtime.CREATE_COMMENT);
        break;
      case NodeTypes.INTERPOLATION:
        if (!context.ssr) context.helper(runtime.TO_DISPLAY_STRING);
        break;
      case NodeTypes.IF:
        for (const branch of node.branches || []) runtime.traverseNode(branch, context);
        break;
      case NodeTypes.IF_BRANCH:
      case NodeTypes.FOR:
      case NodeTypes.ELEMENT:
      case NodeTypes.ROOT:
        runtime.traverseChildren(node, context);
        break;
    }
    context.currentNode = node;
    for (let i = exitFns.length - 1; i >= 0; i--) exitFns[i]();
  };
  runtime.traverseChildren = function traverseChildren(parent, context) {
    let i = 0;
    const nodeRemoved = () => { i--; };
    for (; i < parent.children.length; i++) {
      const child = parent.children[i];
      if (typeof child === 'string') continue;
      context.grandParent = context.parent;
      context.parent = parent;
      context.childIndex = i;
      context.onNodeRemoved = nodeRemoved;
      runtime.traverseNode(child, context);
    }
  };
  runtime.transform = function transform(root, options = {}) {
    recordVuecProvenance('js.compiler.transformTraversal');
    const context = runtime.createTransformContext(root, options);
    runtime.traverseNode(root, context);
    if (options.hoistStatic) runtime.cacheStatic(root, context);
    if (!options.ssr) createRootCodegen(root, context);
    root.helpers = new Set([...context.helpers.keys()]);
    root.components = [...context.components];
    root.directives = [...context.directives];
    root.imports = context.imports;
    root.hoists = context.hoists;
    root.temps = context.temps;
    root.cached = context.cached;
    root.transformed = true;
    root.filters = [...context.filters];
  };
  runtime.baseCompile = function baseCompile(source, options = {}) {
    const onError = options.onError || (error => { throw error; });
    const isModuleMode = options.mode === 'module';
    const prefixIdentifiers = !runtime.isBrowserBuild() && (options.prefixIdentifiers === true || isModuleMode);
    if (!prefixIdentifiers && options.cacheHandlers) {
      onError(runtime.createCompilerError(ErrorCodes.X_CACHE_HANDLER_NOT_SUPPORTED));
    }
    if (options.scopeId && !isModuleMode) {
      onError(runtime.createCompilerError(ErrorCodes.X_SCOPE_ID_NOT_SUPPORTED));
    }
    const resolvedOptions = Object.assign({}, options, { prefixIdentifiers });
    const ast = typeof source === 'string'
      ? hydrateVue3Ast(callBridge('vue3.core.baseParse', bridgePayloadForCall(vue3BridgePayload(source, resolvedOptions.filename, resolvedOptions))), resolvedOptions)
      : hydrateVue3Ast(source, resolvedOptions);
    const [nodeTransforms, directiveTransforms] = runtime.getBaseTransformPreset(prefixIdentifiers);
    runtime.transform(ast, Object.assign({}, resolvedOptions, {
      nodeTransforms: [
        ...nodeTransforms,
        ...(options.nodeTransforms || []),
      ],
      directiveTransforms: Object.assign(
        {},
        directiveTransforms,
        options.directiveTransforms || {},
      ),
    }));
    return runtime.generate(ast, resolvedOptions);
  };
  runtime.generate = function generate(ast, options = {}) {
    ast = hydrateVue3Ast(ast);
    const mode = options.mode || 'function';
    const prefixIdentifiers = options.prefixIdentifiers !== undefined ? options.prefixIdentifiers : mode === 'module';
    const ssr = !!options.ssr;
    const helpers = Array.from(ast.helpers || []);
    const useWithBlock = !prefixIdentifiers && mode !== 'module';
    const runtimeModuleName = options.runtimeModuleName || 'vue';
    const runtimeGlobalName = options.runtimeGlobalName || 'Vue';
    const ssrRuntimeModuleName = options.ssrRuntimeModuleName || 'vue/server-renderer';
    const isSetupInlined = !!options.inline;
    let code = '';
    let indentLevel = 0;
    let preamble = '';
    let pure = false;
    let activeBuffer = isSetupInlined ? 'preamble' : 'code';
    const currentOutput = () => activeBuffer === 'preamble' ? preamble : code;
    const push = value => {
      if (activeBuffer === 'preamble') preamble += String(value);
      else code += String(value);
    };
    const currentIndent = () => '  '.repeat(indentLevel);
    const newline = () => { push(`\n${currentIndent()}`); };
    const indent = () => { indentLevel++; newline(); };
    const deindent = (withoutNewline = false) => {
      indentLevel = Math.max(0, indentLevel - 1);
      if (!withoutNewline) newline();
    };
    const helperAlias = (symbol, asImport = false) => {
      const name = helperName(symbol);
      return asImport ? `${name} as _${name}` : `${name}: _${name}`;
    };

    if (mode === 'module') {
      if (helpers.length) {
        if (options.optimizeImports) {
          push(`import { ${helpers.map(helperName).join(', ')} } from ${JSON.stringify(runtimeModuleName)}`);
          newline();
          newline();
          push(`// Binding optimization for webpack code-split`);
          newline();
          push(`const ${helpers.map(s => `_${helperName(s)} = ${helperName(s)}`).join(', ')}`);
          newline();
        } else {
          push(`import { ${helpers.map(s => helperAlias(s, true)).join(', ')} } from ${JSON.stringify(runtimeModuleName)}`);
          newline();
        }
      }
      if (ast.ssrHelpers && ast.ssrHelpers.length) {
        push(`import { ${ast.ssrHelpers.map(s => helperAlias(s, true)).join(', ')} } from ${JSON.stringify(ssrRuntimeModuleName)}`);
        newline();
      }
      genHoists(ast.hoists || []);
      if (!currentOutput()) push(`\n`);
      else {
        if (!currentOutput().endsWith('\n')) newline();
        if (!currentOutput().endsWith('\n\n')) newline();
      }
      if (!isSetupInlined) push(`export `);
    } else {
      const vueBinding = ssr ? `require(${JSON.stringify(runtimeModuleName)})` : runtimeGlobalName;
      if (helpers.length) {
        if (prefixIdentifiers) {
          push(`const { ${helpers.map(s => helperAlias(s)).join(', ')} } = ${vueBinding}`);
          newline();
        } else {
          push(`const _Vue = ${vueBinding}`);
          newline();
          if ((ast.hoists || []).length) {
            const staticHelpers = [runtime.CREATE_VNODE, runtime.CREATE_ELEMENT_VNODE, runtime.CREATE_COMMENT, runtime.CREATE_TEXT, runtime.CREATE_STATIC]
              .filter(symbol => helpers.includes(symbol))
              .map(s => helperAlias(s))
              .join(', ');
            if (staticHelpers) {
              push(`const { ${staticHelpers} } = _Vue`);
              newline();
            }
          }
        }
      }
      if (ast.ssrHelpers && ast.ssrHelpers.length) {
        push(`const { ${ast.ssrHelpers.map(s => helperAlias(s)).join(', ')} } = require(${JSON.stringify(ssrRuntimeModuleName)})`);
        newline();
      }
      genHoists(ast.hoists || []);
      if (!currentOutput()) push(`\n`);
      else newline();
      push(`return `);
    }
    if (isSetupInlined) {
      activeBuffer = 'code';
      indentLevel = 0;
    }

    const functionName = ssr ? 'ssrRender' : 'render';
    const args = ssr ? ['_ctx', '_push', '_parent', '_attrs'] : ['_ctx', '_cache'];
    if (options.bindingMetadata && !options.inline) args.push('$props', '$setup', '$data', '$options');
    if (isSetupInlined) {
      push(`(${args.join(', ')}) => {`);
    } else {
      push(`function ${functionName}(${args.join(', ')}) {`);
    }
    indent();

    if (useWithBlock) {
      push(`with (_ctx) {`);
      indent();
      if (helpers.length) {
        push(`const { ${helpers.map(s => helperAlias(s)).join(', ')} } = _Vue`);
        push(`\n\n${currentIndent()}`);
      }
    }

    genAssets(ast.components || [], 'component');
    if ((ast.components || []).length && ((ast.directives || []).length || ast.temps > 0)) {
      newline();
    }
    genAssets(ast.directives || [], 'directive');
    if ((ast.directives || []).length && ast.temps > 0) {
      newline();
    }
    if (ast.temps > 0) {
      push(`let ${Array.from({ length: ast.temps }, (_, i) => `_temp${i}`).join(', ')}`);
    }
    if ((ast.components || []).length || (ast.directives || []).length || ast.temps > 0) {
      push(`\n\n${currentIndent()}`);
    }

    if (!ssr) push(`return `);
    genNode(ast.codegenNode || null);

    if (useWithBlock) {
      deindent();
      push(`}`);
    }
    deindent();
    push(`}`);
    return { ast, code, preamble, map: undefined };

    function genNode(node) {
      if (node == null) {
        push('null');
        return;
      }
      if (typeof node === 'string') {
        push(node);
        return;
      }
      if (typeof node === 'symbol') {
        push(helper(node));
        return;
      }
      if (Array.isArray(node)) {
        genNodeListAsArray(node);
        return;
      }
      switch (node.type) {
        case NodeTypes.ELEMENT:
        case NodeTypes.IF:
        case NodeTypes.FOR:
          if (node.codegenNode) genNode(node.codegenNode);
          else genForExpression(node);
          break;
        case NodeTypes.TEXT:
          push(JSON.stringify(node.content));
          break;
        case NodeTypes.COMMENT:
          push(`${helper(runtime.CREATE_COMMENT)}(${JSON.stringify(node.content)})`);
          break;
        case NodeTypes.SIMPLE_EXPRESSION:
          push(node.isStatic ? JSON.stringify(node.content) : node.content);
          break;
        case NodeTypes.INTERPOLATION:
          push(`${helper(runtime.TO_DISPLAY_STRING)}(`);
          genNode(node.content);
          push(`)`);
          break;
        case NodeTypes.COMPOUND_EXPRESSION:
          for (const child of node.children || []) genNode(child);
          break;
        case NodeTypes.TEXT_CALL:
          genNode(node.codegenNode);
          break;
        case NodeTypes.VNODE_CALL:
          genVNodeCall(node);
          break;
        case NodeTypes.JS_CALL_EXPRESSION:
          genCallExpression(node);
          break;
        case NodeTypes.JS_OBJECT_EXPRESSION:
          genObjectExpression(node);
          break;
        case NodeTypes.JS_ARRAY_EXPRESSION:
          genArrayExpression(node);
          break;
        case NodeTypes.JS_FUNCTION_EXPRESSION:
          genFunctionExpression(node);
          break;
        case NodeTypes.JS_CONDITIONAL_EXPRESSION:
          genConditionalExpression(node);
          break;
        case NodeTypes.JS_CACHE_EXPRESSION:
          genCacheExpression(node);
          break;
        case NodeTypes.JS_BLOCK_STATEMENT:
          genNodeList(node.body || [], true, false);
          break;
        case NodeTypes.JS_TEMPLATE_LITERAL:
          genTemplateLiteral(node);
          break;
        case NodeTypes.JS_IF_STATEMENT:
          genIfStatement(node);
          break;
        case NodeTypes.JS_ASSIGNMENT_EXPRESSION:
          genNode(node.left);
          push(` = `);
          genNode(node.right);
          break;
        case NodeTypes.JS_SEQUENCE_EXPRESSION:
          push(`(`);
          genNodeList(node.expressions || [], false, true);
          push(`)`);
          break;
        case NodeTypes.JS_RETURN_STATEMENT:
          push(`return `);
          Array.isArray(node.returns) ? genNodeListAsArray(node.returns) : genNode(node.returns);
          break;
        default:
          push('null');
      }
    }

    function genNodeToString(node) {
      const previous = code;
      code = '';
      genNode(node);
      const out = code;
      code = previous;
      return out;
    }

    function genNodeList(nodes, multilines = false, comma = true) {
      nodes = nodes || [];
      for (let i = 0; i < nodes.length; i++) {
        genNode(nodes[i]);
        if (i < nodes.length - 1) {
          if (multilines) {
            if (comma) push(',');
            newline();
          } else if (comma) {
            push(', ');
          }
        }
      }
    }

    function genNodeListAsArray(nodes) {
      nodes = nodes || [];
      const multilines = nodes.length > 3 || nodes.some(n => Array.isArray(n) || !isTextLike(n));
      push(`[`);
      if (multilines) {
        indent();
      }
      genNodeList(nodes, multilines, true);
      if (multilines) {
        deindent();
      }
      push(`]`);
    }

    function genVNodeCall(node) {
      const call = node.isBlock ? helper(runtime.getVNodeBlockHelper(ssr, node.isComponent)) : helper(runtime.getVNodeHelper(ssr, node.isComponent));
      const args = genNullableArgs([node.tag, node.props, node.children, patchFlagText(node.patchFlag), node.dynamicProps]);
      if (node.directives) push(`${helper(runtime.WITH_DIRECTIVES)}(`);
      if (node.isBlock) push(`(${helper(runtime.OPEN_BLOCK)}(${node.disableTracking ? 'true' : ''}), `);
      push(`${call}(`);
      genNodeList(args, false, true);
      push(`)`);
      if (node.isBlock) push(`)`);
      if (node.directives) {
        push(`, `);
        genNode(node.directives);
        push(`)`);
      }
    }

    function genNullableArgs(args) {
      let i = args.length;
      while (i--) {
        if (args[i] != null) break;
      }
      return args.slice(0, i + 1).map(arg => arg || 'null');
    }

    function genCallExpression(node) {
      const callee = typeof node.callee === 'symbol' ? helper(node.callee) : String(node.callee);
      if (pure) push(`/*@__PURE__*/`);
      push(`${callee}(`);
      genNodeList(node.arguments || [], false, true);
      push(`)`);
    }

    function genObjectExpression(node) {
      const properties = node.properties || [];
      if (!properties.length) {
        push(`{}`);
        return;
      }
      const multilines = properties.length > 1 || properties.some(prop => prop.value && prop.value.type !== NodeTypes.SIMPLE_EXPRESSION);
      push(multilines ? `{` : `{ `);
      if (multilines) indent();
      for (let i = 0; i < properties.length; i++) {
        genPropertyKey(properties[i].key);
        push(`: `);
        genNode(properties[i].value);
        if (i < properties.length - 1) {
          push(`,`);
          newline();
        }
      }
      if (multilines) deindent();
      push(multilines ? `}` : ` }`);
    }

    function genPropertyKey(key) {
      if (!key) {
        push('undefined');
      } else if (key.type === NodeTypes.COMPOUND_EXPRESSION) {
        push(`[`);
        genNode(key);
        push(`]`);
      } else if (key.type === NodeTypes.SIMPLE_EXPRESSION && key.isStatic) {
        push(runtime.isSimpleIdentifier(key.content) ? key.content : JSON.stringify(key.content));
      } else if (key.type === NodeTypes.SIMPLE_EXPRESSION) {
        push(`[${key.content}]`);
      } else {
        push(`[`);
        genNode(key);
        push(`]`);
      }
    }

    function genArrayExpression(node) {
      genNodeListAsArray(node.elements || []);
    }

    function genFunctionExpression(node) {
      if (node.isSlot) push(`${helper(runtime.WITH_CTX)}(`);
      push(`(`);
      if (Array.isArray(node.params)) genNodeList(node.params, false, true);
      else if (node.params) genNode(node.params);
      push(`) => `);
      if (node.newline || node.body) {
        push(`{`);
        indent();
      }
      if (node.returns) {
        if (node.newline) push(`return `);
        Array.isArray(node.returns) ? genNodeListAsArray(node.returns) : genNode(node.returns);
      } else if (node.body) {
        genNode(node.body);
      }
      if (node.newline || node.body) {
        deindent();
        push(`}`);
      }
      if (node.isSlot) push(`)`);
    }

    function genConditionalExpression(node) {
      const nested = node.alternate && node.alternate.type === NodeTypes.JS_CONDITIONAL_EXPRESSION;
      if (ssr && node.test && node.test.type !== NodeTypes.SIMPLE_EXPRESSION) {
        push(`(`);
        genNode(node.test);
        push(`)`);
      } else if (node.test && node.test.type === NodeTypes.SIMPLE_EXPRESSION && !node.test.isStatic && !runtime.isSimpleIdentifier(node.test.content)) {
        push(`(`);
        genNode(node.test);
        push(`)`);
      } else {
        genNode(node.test);
      }
      if (node.newline === false) {
        push(` ? `);
        genNode(node.consequent);
        push(` : `);
        genNode(node.alternate);
        return;
      }
      indentLevel++;
      newline();
      push(`? `);
      indentLevel++;
      genNode(node.consequent);
      indentLevel--;
      newline();
      push(`: `);
      if (!nested) indentLevel++;
      genNode(node.alternate);
      if (!nested) indentLevel--;
      indentLevel--;
    }

    function genCacheExpression(node) {
      if (node.needArraySpread) push(`[...(`);
      push(`_cache[${node.index}] || (`);
      if (node.needPauseTracking) {
        indent();
        push(`${helper(runtime.SET_BLOCK_TRACKING)}(-1${node.inVOnce ? ', true' : ''}),`);
        newline();
        push(`(_cache[${node.index}] = `);
        genNode(node.value);
        push(`).cacheIndex = ${node.index},`);
        newline();
        push(`${helper(runtime.SET_BLOCK_TRACKING)}(1),`);
        newline();
        push(`_cache[${node.index}]`);
        deindent();
      } else {
        push(`_cache[${node.index}] = `);
        genNode(node.value);
      }
      push(`)`);
      if (node.needArraySpread) push(`)]`);
    }

    function genForExpression(node) {
      const blockHelper = helper(runtime.getVNodeBlockHelper(ssr, false));
      push(`(${helper(runtime.OPEN_BLOCK)}(true), ${blockHelper}(${helper(runtime.FRAGMENT)}, null, ${helper(runtime.RENDER_LIST)}(`);
      genNode(node.source);
      push(`, (`);
      genNodeList(runtime.createForLoopParams(node.parseResult || node), false, true);
      push(`) => {`);
      indent();
      push(`return `);
      const children = node.children || [];
      const child = children.length === 1 ? children[0] : children;
      if (Array.isArray(child)) {
        push(`(${helper(runtime.OPEN_BLOCK)}(), ${blockHelper}(${helper(runtime.FRAGMENT)}, null, `);
        genNodeListAsArray(child);
        push(`, 64 /* STABLE_FRAGMENT */))`);
      } else if (child && child.type === NodeTypes.TEXT_CALL) {
        push(`(${helper(runtime.OPEN_BLOCK)}(), ${blockHelper}(${helper(runtime.FRAGMENT)}, null, [`);
        indent();
        genNode(child);
        deindent();
        push(`], 64 /* STABLE_FRAGMENT */))`);
      } else {
        genNode(child);
      }
      deindent();
      push(`}), 256 /* UNKEYED_FRAGMENT */))`);
    }

    function genTemplateLiteral(node) {
      push('`');
      const elements = node.elements || [];
      const multiline = ssr && elements.filter(element => typeof element !== 'string').length > 1;
      for (const element of elements) {
        if (typeof element === 'string') {
          push(element.replace(/(`|\$|\\)/g, '\\$1'));
        } else {
          push('${');
          if (multiline) {
            indentLevel++;
            newline();
          }
          genNode(element);
          if (multiline) {
            indentLevel--;
            newline();
          }
          push('}');
        }
      }
      push('`');
    }

    function genIfStatement(node) {
      push(`if (`);
      genNode(node.test);
      push(`) {`);
      indent();
      genNode(node.consequent);
      deindent();
      push(`}`);
      if (node.alternate) {
        push(` else `);
        if (node.alternate.type === NodeTypes.JS_IF_STATEMENT) {
          genIfStatement(node.alternate);
        } else {
          push(`{`);
          indent();
          genNode(node.alternate);
          deindent();
          push(`}`);
        }
      }
    }

    function genAssets(assets, type) {
      if (!assets.length) return;
      const resolver = helper(type === 'component' ? runtime.RESOLVE_COMPONENT : runtime.RESOLVE_DIRECTIVE);
      for (let i = 0; i < assets.length; i++) {
        let id = assets[i];
        const maybeSelfReference = String(id).endsWith('__self');
        if (maybeSelfReference) id = id.slice(0, -6);
        push(`const ${runtime.toValidAssetId(id, type)} = ${resolver}(${JSON.stringify(id)}${maybeSelfReference ? ', true' : ''})`);
        if (i < assets.length - 1) newline();
      }
    }

    function genHoists(hoists) {
      if (!hoists.length) return;
      const previousPure = pure;
      pure = true;
      newline();
      for (let i = 0; i < hoists.length; i++) {
        const exp = hoists[i];
        if (!exp) continue;
        push(`const _hoisted_${i + 1} = `);
        genNode(exp);
        newline();
      }
      pure = previousPure;
    }

    function patchFlagText(flag) {
      if (flag == null) return flag;
      if (typeof flag === 'string' && /\/\*/.test(flag)) return flag;
      const value = Number(flag);
      if (!Number.isFinite(value) || value === 0) return flag;
      const names = {
        1: 'TEXT', 2: 'CLASS', 4: 'STYLE', 8: 'PROPS', 16: 'FULL_PROPS',
        32: 'NEED_HYDRATION', 64: 'STABLE_FRAGMENT', 128: 'KEYED_FRAGMENT',
        256: 'UNKEYED_FRAGMENT', 512: 'NEED_PATCH', 1024: 'DYNAMIC_SLOTS',
        2048: 'DEV_ROOT_FRAGMENT', [-1]: 'CACHED', [-2]: 'BAIL',
      };
      const text = value < 0 ? names[value] : Object.keys(names)
        .map(Number)
        .filter(n => n > 0 && (value & n))
        .map(n => names[n])
        .join(', ');
      return text ? `${value} /* ${text} */` : String(flag);
    }

    function isTextLike(node) {
      return typeof node === 'string'
        || (node && [NodeTypes.SIMPLE_EXPRESSION, NodeTypes.TEXT, NodeTypes.INTERPOLATION, NodeTypes.COMPOUND_EXPRESSION].includes(node.type));
    }

    function helper(symbol) {
      return `_${helperName(symbol)}`;
    }

    function helperName(symbol) {
      return runtime.helperNameMap[symbol] || String(symbol || '').replace(/^_/, '');
    }
  };
  runtime.createStructuralDirectiveTransform = function createStructuralDirectiveTransform(name, fn) {
    const matches = typeof name === 'string' ? n => n === name : n => name.test(n);
    return (node, context) => {
      if (node.type !== NodeTypes.ELEMENT) return;
      if (node.tagType === ElementTypes.TEMPLATE && (node.props || []).some(runtime.isVSlot)) return;
      const exitFns = [];
      for (let i = 0; i < node.props.length; i++) {
        const prop = node.props[i];
        if (prop.type === NodeTypes.DIRECTIVE && matches(prop.name)) {
          node.props.splice(i, 1);
          i--;
          const onExit = fn(node, prop, context);
          if (onExit) exitFns.push(onExit);
        }
      }
      return exitFns;
    };
  };
  runtime.noopDirectiveTransform = () => ({ props: [] });
  runtime.processExpression = function processExpression(node, context, asParams = false, asRawStatements = false, localVars) {
    if (!node || node.type !== NodeTypes.SIMPLE_EXPRESSION) return node;
    const projection = callBridge('vue3.core.processExpression', {
      node: runtime.dehydrateForBridge(node),
      context: vue3ProcessExpressionContextPayload(context),
      asParams: !!asParams,
      asRawStatements: !!asRawStatements,
      localVars: localVars || null,
    });
    return materializeVue3ProcessExpressionProjection(projection, node, context);
  };
  runtime.transformExpression = function transformExpression(node, context) {
    const projection = callBridge('vue3.core.transformExpression', {
      node: runtime.dehydrateForBridge(node),
      context: vue3ProcessExpressionContextPayload(context),
    });
    materializeVue3TransformExpressionProjection(projection, node, context);
  };
  runtime.isBrowserBuild = function isBrowserBuild() {
    return typeof __BROWSER__ !== 'undefined' && !!__BROWSER__;
  };
  runtime.modifierName = function modifierName(modifier) {
    return typeof modifier === 'string' ? modifier : modifier && modifier.content;
  };
  runtime.hasModifier = function hasModifier(dir, name) {
    return (dir.modifiers || []).some(modifier => runtime.modifierName(modifier) === name);
  };
  runtime.injectBindPrefix = function injectBindPrefix(arg, prefix) {
    if (arg.type === NodeTypes.SIMPLE_EXPRESSION) {
      if (arg.isStatic) {
        arg.content = prefix + arg.content;
      } else {
        arg.content = `\`${prefix}\${${arg.content}}\``;
      }
    } else {
      arg.children.unshift(`'${prefix}' + (`);
      arg.children.push(`)`);
    }
  };
  runtime.transformBind = function transformBind(dir, _node, context) {
    context = context || {
      helper: name => name,
      helperString: name => `_${runtime.helperNameMap[name] || name}`,
      inSSR: false,
      onError: error => { throw error; },
    };
    const projection = callBridge('vue3.core.transformBind', {
      dir,
      context: vue3TransformBindContextPayload(context),
    });
    materializeVue3BindErrors(projection, dir, context);
    return {
      props: (projection && projection.props || []).map(prop => {
        const key = materializeVue3OnProjection(prop.key, dir, context);
        const value = materializeVue3OnProjection(prop.value, dir, context);
        return runtime.createObjectProperty(key, value);
      }),
    };
  };
  runtime.transformOn = function transformOn(dir, node, context, augmentor) {
    context = context || { helperString: name => `_${runtime.helperNameMap[name] || name}`, helper: name => name, cache: value => value, onError: error => { throw error; } };
    const projection = callBridge('vue3.core.transformOn', {
      dir,
      node,
      context: vue3TransformOnContextPayload(context),
    });
    materializeVue3OnErrors(projection, dir, context);
    const onMeta = (projection.props || []).map(prop => ({
      cache: !!prop.cache,
      valueConstant: !!prop.valueConstant,
      handlerKey: !!prop.handlerKey,
      dynamicKey: !!prop.dynamicKey,
      ignoreDynamicKeyForNormalize: !!prop.ignoreDynamicKeyForNormalize,
    }));
    let result = {
      props: (projection.props || []).map(prop => {
        const key = materializeVue3OnProjection(prop.key, dir, context);
        const value = materializeVue3OnProjection(prop.value, dir, context) || runtime.createSimpleExpression('() => {}', false, dir.loc);
        return runtime.createObjectProperty(key, value);
      }),
    };
    if (typeof augmentor === 'function') result = augmentor(result) || result;
    for (const [index, prop] of (result.props || []).entries()) {
      const meta = onMeta[index] || onMeta[0] || {};
      if (prop.key && meta.handlerKey) prop.key.isHandlerKey = true;
      if (meta.cache && context && context.cache) prop.value = context.cache(prop.value);
      prop.__vuecOn = meta;
    }
    return result;
  };
  runtime.transformModel = function transformModel(dir) {
    const projection = callBridge('vue3.core.transformModel', {
      dir,
      node: arguments[1],
      context: vue3TransformModelContextPayload(arguments[2]),
    });
    for (const code of projection.errors || []) {
      const loc = code === ErrorCodes.X_V_MODEL_NO_EXPRESSION ? dir.loc : dir.exp && dir.exp.loc || dir.loc;
      if (arguments[2] && arguments[2].onError) arguments[2].onError(runtime.createCompilerError(code, loc));
    }
    return {
      props: (projection.props || []).map(prop => {
        const key = materializeVue3ModelProjection(prop.key, dir, arguments[2]);
        const value = materializeVue3ModelProjection(prop.value, dir, arguments[2]);
        const objectProp = runtime.createObjectProperty(key, value);
        objectProp.__vuecModel = {
          dynamic: !!prop.dynamic,
          cache: !!prop.cache,
          hydrate: !!prop.hydrate,
          kind: prop.kind,
        };
        if (prop.cache && arguments[2]) objectProp.value = arguments[2].cache(objectProp.value);
        return objectProp;
      }),
    };
  };
  runtime.transformVBindShorthand = function transformVBindShorthand(node, context) {
    if (!node || node.type !== NodeTypes.ELEMENT) return;
    const projection = callBridge('vue3.core.transformVBindShorthand', {
      node: runtime.dehydrateForBridge(node),
      context: vue3TransformVBindShorthandContextPayload(context),
    });
    materializeVue3VBindShorthandProjection(projection, node, context);
  };
  runtime.transformElement = function transformElement(node, context) {
    return () => {
      recordVuecProvenance('js.transformElement.props');
      node = context.currentNode;
      if (!node || node.type !== NodeTypes.ELEMENT) return;
      if (node.tagType !== ElementTypes.ELEMENT && node.tagType !== ElementTypes.COMPONENT) return;
      const isComponent = node.tagType === ElementTypes.COMPONENT;
      const tag = isComponent ? runtime.resolveComponentType(node, context) : `"${node.tag}"`;
      const isDynamicComponent = isComponent && tag && typeof tag === 'object' && tag.type === NodeTypes.JS_CALL_EXPRESSION && tag.callee === runtime.RESOLVE_DYNAMIC_COMPONENT;
      let patchFlag;
      let props;
      let hasDynamicKey = false;
      let hasHydrationEvent = false;
      const dynamicProps = [];
      const propSummaries = [];
      let vnodeDirectives;
      let shouldUseBlock = !!(
        isDynamicComponent
        || tag === runtime.TELEPORT
        || tag === runtime.SUSPENSE
        || (!isComponent && (node.tag === 'svg' || node.tag === 'foreignObject' || node.tag === 'math'))
      );
      if (node.props && node.props.length) {
        const objectProps = [];
        const mergeArgs = [];
        const runtimeDirectives = [];
        const pushMergeArg = arg => {
          if (objectProps.length) {
            mergeArgs.push(runtime.createObjectExpression(objectProps.splice(0), node.loc));
          }
          if (arg) mergeArgs.push(arg);
        };
        for (const prop of node.props) {
          if (prop.type === NodeTypes.ATTRIBUTE) {
            if (
              prop.name === 'is'
              && (
                node.tag === 'component'
                || node.tag === 'Component'
                || (prop.value && String(prop.value.content || '').startsWith('vue:'))
              )
            ) {
              continue;
            }
            objectProps.push(runtime.createObjectProperty(prop.name, runtime.createSimpleExpression(prop.value ? prop.value.content : '', true)));
            propSummaries.push({ kind: 'attribute', name: prop.name, value: prop.value && prop.value.content });
          } else if (prop.name === 'bind' && prop.arg) {
            if (
              runtime.isStaticArgOf(prop.arg, 'is')
              && (node.tag === 'component' || node.tag === 'Component')
            ) {
              continue;
            }
            const transform = context.directiveTransforms && context.directiveTransforms.bind;
            if (!transform) {
              if (runtime.isStaticArgOf(prop.arg, 'key')) propSummaries.push({ kind: 'directiveProp', forceBlock: true });
              continue;
            }
            recordVuecExternalCallback('callback.directiveTransform', transform);
            const result = transform(prop, node, context);
            objectProps.push(...((result && result.props) || []));
            propSummaries.push(...vue3ElementDirectivePropSummaries(prop, result, {
              forceBlock: runtime.isStaticArgOf(prop.arg, 'key'),
              propModifier: runtime.hasModifier(prop, 'prop'),
            }));
            if (result && result.props && result.props.some(p => p.key && !runtime.isStaticExp(p.key))) hasDynamicKey = true;
            else if (prop.arg.isStatic) dynamicProps.push(prop.arg.content);
          } else if (prop.name === 'on' && prop.arg) {
            const transform = context.directiveTransforms && context.directiveTransforms.on;
            recordVuecExternalCallback('callback.directiveTransform', transform);
            const result = transform ? transform(prop, node, context) : undefined;
            objectProps.push(...((result && result.props) || []));
            if (!result && node.children && node.children.length && runtime.isStaticArgOf(prop.arg, 'vue:before-update')) {
              propSummaries.push({ kind: 'directiveProp', forceBlock: true });
            }
            propSummaries.push(...vue3ElementDirectivePropSummaries(prop, result, {
              forceBlock: !!(node.children && node.children.length && runtime.isStaticArgOf(prop.arg, 'vue:before-update')),
            }));
            if (!result || !result.props || !result.props.some(p => p.value && p.value.type === NodeTypes.JS_CACHE_EXPRESSION)) {
              if (result && result.props && result.props.some(p => p.key && p.key.isHandlerKey)) dynamicProps.push(result.props[0].key.content || prop.arg.content);
            }
          } else if (prop.name === 'bind' && !prop.arg) {
            if (prop.exp) {
              pushMergeArg(prop.exp);
              hasDynamicKey = true;
              propSummaries.push({ kind: 'objectBind' });
            } else {
              context.onError(runtime.createCompilerError(ErrorCodes.X_V_BIND_NO_EXPRESSION, prop.loc));
            }
          } else if (prop.name === 'on' && !prop.arg) {
            if (prop.exp) {
              pushMergeArg(runtime.createCallExpression(context.helper(runtime.TO_HANDLERS), isComponent ? [prop.exp] : [prop.exp, 'true'], prop.loc));
              hasDynamicKey = true;
              propSummaries.push({ kind: 'objectOn' });
            } else {
              context.onError(runtime.createCompilerError(ErrorCodes.X_V_ON_NO_EXPRESSION, prop.loc));
            }
          } else if (prop.name === 'model' && context.directiveTransforms && context.directiveTransforms.model) {
            recordVuecExternalCallback('callback.directiveTransform', context.directiveTransforms.model);
            const result = context.directiveTransforms.model(prop, node, context);
            const modelProps = (result && result.props) || [];
            objectProps.push(...modelProps);
            propSummaries.push(...vue3ElementDirectivePropSummaries(prop, result));
            if (modelProps.some(p => p.key && !runtime.isStaticExp(p.key))) hasDynamicKey = true;
            for (const modelProp of modelProps) {
              if (modelProp.__vuecModel && modelProp.__vuecModel.dynamic && runtime.isStaticExp(modelProp.key)) {
                dynamicProps.push(modelProp.key.content);
              }
              if (modelProp.__vuecModel && modelProp.__vuecModel.hydrate) {
                hasHydrationEvent = true;
              }
            }
            if (result && result.needRuntime) {
              prop.__vuecNeedRuntime = result.needRuntime;
              runtimeDirectives.push(prop);
              propSummaries.push({ kind: 'runtimeDirective' });
            }
          } else if (prop.name === 'once' || prop.name === 'memo') {
            continue;
          } else if (prop.name === 'slot') {
            if (!isComponent) context.onError(runtime.createCompilerError(ErrorCodes.X_V_SLOT_MISPLACED, prop.loc));
            continue;
          } else if (context.directiveTransforms && context.directiveTransforms[prop.name]) {
            recordVuecExternalCallback('callback.directiveTransform', context.directiveTransforms[prop.name]);
            const result = context.directiveTransforms[prop.name](prop, node, context);
            objectProps.push(...((result && result.props) || []));
            propSummaries.push(...vue3ElementDirectivePropSummaries(prop, result));
            if (result && result.needRuntime) {
              prop.__vuecNeedRuntime = result.needRuntime;
              runtimeDirectives.push(prop);
              propSummaries.push({ kind: 'runtimeDirective' });
            }
          } else {
            runtimeDirectives.push(prop);
            if (!runtime.isBuiltInDirective(prop.name)) propSummaries.push({ kind: 'runtimeDirective' });
          }
        }
        if (mergeArgs.length) {
          pushMergeArg();
          props = mergeArgs.length > 1 ? runtime.createCallExpression(context.helper(runtime.MERGE_PROPS), mergeArgs, node.loc) : mergeArgs[0];
        } else if (objectProps.length) {
          props = runtime.createObjectExpression(runtime.dedupeProperties(objectProps), node.loc);
        }
        const propsProjection = callBridge('vue3.core.transformElementProps', {
          props: propSummaries,
          hasChildren: !!(node.children && node.children.length),
          isComponent,
          isDynamicComponent,
          context: vue3TransformElementContextPayload(context),
        });
        if (propsProjection && propsProjection.refForMarker) {
          props = runtime.prependPropsExpressionProp(
            props,
            runtime.createObjectProperty('ref_for', runtime.createSimpleExpression('true')),
            node.loc,
          );
        }
        if (props && propsProjection && propsProjection.inlineTemplateRefs) {
          props = runtime.applyInlineTemplateRefProjection(props, propsProjection.inlineTemplateRefs, node.loc);
        }
        if (props && propsProjection && propsProjection.normalizeClass) runtime.normalizeObjectProp(props, 'class', context.helper(runtime.NORMALIZE_CLASS));
        if (props && propsProjection && propsProjection.normalizeStyle) runtime.normalizeObjectProp(props, 'style', context.helper(runtime.NORMALIZE_STYLE));
        if (props && propsProjection && propsProjection.normalizeProps) {
          if (!(props.type === NodeTypes.JS_CALL_EXPRESSION && (props.callee === runtime.MERGE_PROPS || props.callee === runtime.TO_HANDLERS))) {
            const argument = propsProjection.guardReactiveProps
              ? runtime.createCallExpression(context.helper(runtime.GUARD_REACTIVE_PROPS), [props], node.loc)
              : props;
            props = runtime.createCallExpression(context.helper(runtime.NORMALIZE_PROPS), [argument], node.loc);
          } else if (propsProjection.guardReactiveProps && props.type !== NodeTypes.JS_CALL_EXPRESSION) {
            props = runtime.createCallExpression(context.helper(runtime.NORMALIZE_PROPS), [runtime.createCallExpression(context.helper(runtime.GUARD_REACTIVE_PROPS), [props], node.loc)], node.loc);
          }
        }
        patchFlag = propsProjection && propsProjection.patchFlag || undefined;
        dynamicProps.splice(0, dynamicProps.length, ...((propsProjection && propsProjection.dynamicPropNames) || dynamicProps));
        if (propsProjection && propsProjection.shouldUseBlock) shouldUseBlock = true;
        if (hasHydrationEvent) patchFlag = (patchFlag || 0) | 32;
        if (runtimeDirectives.length) {
          const directiveArgs = runtimeDirectives.map(d => {
            return runtime.buildDirectiveArgs(d, context);
          });
          vnodeDirectives = runtime.createArrayExpression(directiveArgs);
          if (!shouldUseBlock && (!patchFlag || patchFlag === 32)) patchFlag = (patchFlag || 0) | 512;
        }
      }
      const onlyChild = node.children && node.children.length === 1 ? node.children[0] : undefined;
      let children = onlyChild && [NodeTypes.TEXT, NodeTypes.INTERPOLATION, NodeTypes.COMPOUND_EXPRESSION].includes(onlyChild.type)
        ? onlyChild
        : node.children && node.children.length
          ? node.children
          : undefined;
      if (isComponent && node.children && node.children.length && tag !== runtime.TELEPORT && tag !== runtime.KEEP_ALIVE) {
        const builtSlots = runtime.buildSlots(node, context);
        children = builtSlots.slots;
        if (builtSlots.hasDynamicSlots) patchFlag = (patchFlag || 0) | 1024;
      } else {
        const childrenProjection = callBridge('vue3.core.transformElementChildren', {
          tag: projectionNameFromHelperSymbol(tag),
          children: node.children || [],
        });
        if (childrenProjection && childrenProjection.kind === 'slots') {
          children = materializeVue3ElementSlotsProjection(childrenProjection, node, context);
          if (childrenProjection.shouldUseBlock) shouldUseBlock = true;
        } else if (childrenProjection && childrenProjection.kind === 'children') {
          if (childrenProjection.shouldUseBlock) shouldUseBlock = true;
          if (childrenProjection.patchFlag) patchFlag = (patchFlag || 0) | childrenProjection.patchFlag;
        }
      }
      if (!patchFlag && children && (children.type === NodeTypes.INTERPOLATION || children.type === NodeTypes.COMPOUND_EXPRESSION) && runtime.getConstantType(children, context) === ConstantTypes.NOT_CONSTANT) patchFlag = 1;
      node.codegenNode = runtime.createVNodeCall(context, tag, props, children, patchFlag, dynamicProps.length ? stringifyDynamicPropNames(dynamicProps) : undefined, vnodeDirectives, shouldUseBlock, false, isComponent, node.loc);
    };
  };
  runtime.processSlotOutlet = function processSlotOutlet(node, context) {
    const projection = callBridge('vue3.core.transformSlotOutlet', {
      node: runtime.dehydrateForBridge(node),
      context: vue3TransformSlotOutletContextPayload(context),
    });
    const process = projection && projection.process || {};
    materializeVue3SlotOutletMutations(process, node, context);
    const nonNameProps = (process.nonNameProps || [])
      .map(index => node.props && node.props[index])
      .filter(Boolean);
    const slotName = materializeVue3SlotOutletName(process.slotName, node, context);
    let slotProps;
    if (nonNameProps.length) {
      const built = runtime.buildProps(node, context, nonNameProps, false, false);
      slotProps = built.props;
      if (built.directives && built.directives.length) {
        context.onError(runtime.createCompilerError(ErrorCodes.X_V_SLOT_UNEXPECTED_DIRECTIVE_ON_SLOT_OUTLET, built.directives[0].loc));
      }
    }
    return { slotName, slotProps };
  };
  runtime.transformSlotOutlet = function transformSlotOutlet(node, context) {
    if (node.type === NodeTypes.ELEMENT && node.tagType === ElementTypes.SLOT) {
      return () => {
        const projection = callBridge('vue3.core.transformSlotOutlet', {
          node: runtime.dehydrateForBridge(node),
          context: vue3TransformSlotOutletContextPayload(context),
        });
        if (!projection || !projection.transform) return;
        const process = projection.process || {};
        materializeVue3SlotOutletMutations(process, node, context);
        const nonNameProps = (process.nonNameProps || [])
          .map(index => node.props && node.props[index])
          .filter(Boolean);
        const slotName = materializeVue3SlotOutletName(process.slotName, node, context);
        let slotProps;
        if (nonNameProps.length) {
          const built = runtime.buildProps(node, context, nonNameProps, false, false);
          slotProps = built.props;
          if (built.directives && built.directives.length) {
            context.onError(runtime.createCompilerError(ErrorCodes.X_V_SLOT_UNEXPECTED_DIRECTIVE_ON_SLOT_OUTLET, built.directives[0].loc));
          }
        }
        const codegen = projection.codegen || {};
        const args = [codegen.slots || (context.prefixIdentifiers ? '_ctx.$slots' : '$slots'), slotName, '{}', 'undefined', 'true'];
        let expectedLen = codegen.expectedLen == null ? 2 : codegen.expectedLen;
        if (slotProps) {
          args[2] = slotProps;
          expectedLen = Math.max(expectedLen, 3);
        }
        if (node.children && node.children.length) {
          args[3] = runtime.createFunctionExpression([], node.children, false, false, node.loc);
          expectedLen = Math.max(expectedLen, 4);
        }
        args.splice(expectedLen);
        node.codegenNode = runtime.createCallExpression(context.helper(runtime.RENDER_SLOT), args);
      };
    }
  };
  runtime.transformText = function transformText(node, context) {
    if (![NodeTypes.ROOT, NodeTypes.ELEMENT, NodeTypes.FOR, NodeTypes.IF_BRANCH].includes(node.type)) return;
    return () => {
      const projection = callBridge('vue3.core.transformText', {
        node: runtime.dehydrateForBridge(node),
        context: vue3TransformTextContextPayload(context),
      });
      materializeVue3TransformTextProjection(projection, node, context);
    };
  };
  runtime.findUntransformedCustomDirective = function findUntransformedCustomDirective(node, context) {
    return (node.props || []).find(prop => prop.type === NodeTypes.DIRECTIVE && !(context.directiveTransforms || {})[prop.name]);
  };
  runtime.processIf = function processIf(node, dir, context, processCodegen) {
    const siblings = context.parent && context.parent.children || [];
    const nodeIndex = siblings.indexOf(node);
    const projection = callBridge('vue3.core.transformIf', {
      phase: 'process',
      node,
      dir,
      parent: context.parent,
      siblings: vue3IfSiblingPayload(siblings),
      nodeIndex,
      currentUserKey: runtime.findProp(node, 'key'),
      context: vue3TransformIfContextPayload(context),
    });
    materializeVue3IfErrors(projection, node, dir, context);
    if (projection && projection.branch && projection.branch.condition) {
      dir.exp = materializeVue3IfProjection(projection.branch.condition, node, dir, context);
    }
    const branch = {
      type: NodeTypes.IF_BRANCH,
      loc: node.loc,
      condition: dir.name === 'else' ? undefined : dir.exp,
      children: projection && projection.branch && projection.branch.children === 'template' ? (node.children || []) : [node],
      userKey: runtime.findProp(node, 'key'),
      isTemplateIf: node.tagType === ElementTypes.TEMPLATE,
    };
    const action = projection && projection.action || { kind: 'noop' };
    const finalizeBranch = (ifNode, targetBranch, isRoot) => {
      if (processCodegen) return processCodegen(ifNode, targetBranch, isRoot);
      if (context && context.ssr) return undefined;
      return () => {
        if (isRoot) {
          ifNode.codegenNode = runtime.createIfCodegenNodeForBranch(targetBranch, action.keyBase || 0, context);
        } else {
          const parentCondition = runtime.getParentCondition(ifNode.codegenNode);
          parentCondition.alternate = runtime.createIfCodegenNodeForBranch(targetBranch, (ifNode.__vuecKeyBase || 0) + ifNode.branches.length - 1, context);
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
        runtime.traverseNode(branch, context);
        if (onExit) onExit();
        context.currentNode = null;
      }
      return;
    }
    const ifNode = { type: NodeTypes.IF, loc: node.loc, branches: [branch], codegenNode: undefined };
    ifNode.__vuecKeyBase = action.keyBase || 0;
    context.replaceNode(ifNode);
    const onExit = finalizeBranch(ifNode, branch, true);
    return () => {
      if (onExit) onExit();
    };
  };
  runtime.refreshIfCodegen = function refreshIfCodegen(ifNode, context, keyBase = 0) {
    let alternate = runtime.createCallExpression(context.helper(runtime.CREATE_COMMENT), ['"v-if"', 'true']);
    for (let i = ifNode.branches.length - 1; i >= 0; i--) {
      const branch = ifNode.branches[i];
      const childCodegen = runtime.createIfBranchCodegen(branch, keyBase + i, context);
      if (branch.condition) {
        alternate = runtime.createConditionalExpression(branch.condition, childCodegen, alternate);
      } else {
        alternate = childCodegen;
      }
    }
    if (ifNode.codegenNode && ifNode.codegenNode.type === NodeTypes.JS_CACHE_EXPRESSION) {
      ifNode.codegenNode.value = alternate;
    } else {
      ifNode.codegenNode = alternate;
    }
  };
  runtime.createIfCodegenNodeForBranch = function createIfCodegenNodeForBranch(branch, keyIndex, context) {
    const childCodegen = runtime.createIfBranchCodegen(branch, keyIndex, context);
    if (branch.condition) {
      return runtime.createConditionalExpression(
        branch.condition,
        childCodegen,
        runtime.createCallExpression(context.helper(runtime.CREATE_COMMENT), ['"v-if"', 'true']),
      );
    }
    return childCodegen;
  };
  runtime.createIfBranchCodegen = function createIfBranchCodegen(branch, keyIndex, context) {
    const keyProperty = runtime.createObjectProperty('key', runtime.createSimpleExpression(String(keyIndex), false, locStub, ConstantTypes.CAN_CACHE));
    const children = branch.children || [];
    const firstChild = children[0];
    const projection = callBridge('vue3.core.transformIf', {
      phase: 'branchCodegen',
      branch: vue3IfBranchCodegenPayload(branch),
      keyIndex,
    });
    if (projection.kind === 'for') {
      const vnodeCall = firstChild.codegenNode;
      runtime.injectProp(vnodeCall, keyProperty);
      return vnodeCall;
    }
    if (projection.kind === 'fragment') {
      return runtime.createVNodeCall(context, context.helper(runtime.FRAGMENT), runtime.createObjectExpression([keyProperty]), children, projection.patchFlag, undefined, undefined, true, false, false, branch.loc);
    }
    const ret = firstChild.codegenNode;
    const vnodeCall = runtime.getMemoedVNodeCall(ret);
    if (vnodeCall) {
      if (vnodeCall.type === NodeTypes.VNODE_CALL) {
        runtime.convertToBlock(vnodeCall, context);
      }
      runtime.injectProp(vnodeCall, keyProperty);
    }
    return ret;
  };
  runtime.getParentCondition = function getParentCondition(node) {
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
  };
  runtime.transformIf = runtime.createStructuralDirectiveTransform(/^(if|else|else-if)$/, runtime.processIf);
  runtime.processFor = function processFor(node, dir, context, processCodegen) {
    const projection = callBridge('vue3.core.transformFor', {
      node,
      dir,
      context: vue3TransformForContextPayload(context),
    });
    materializeVue3ForErrors(projection, node, dir, context);
    if (!projection || !projection.parseResult) return;
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
    let renderExp;
    if (!processCodegen && !(context && context.ssr)) {
      renderExp = runtime.createCallExpression(context.helper(runtime.RENDER_LIST), [forNode.source]);
      forNode.codegenNode = runtime.createVNodeCall(context, context.helper(runtime.FRAGMENT), undefined, renderExp, 256, undefined, undefined, true, true, false, node.loc);
    }
    context.replaceNode(forNode);
    context.scopes.vFor++;
    const onExit = processCodegen ? processCodegen(forNode) : undefined;
    return () => {
      context.scopes.vFor--;
      if (context.prefixIdentifiers) aliases.forEach(alias => context.removeIdentifiers(alias));
      if (onExit) {
        onExit();
      } else if (renderExp) {
        materializeVue3ForTemplateKeyErrors(projection, node, dir, context);
        runtime.finalizeForCodegen(forNode, renderExp, context);
      }
    };
  };
  runtime.transformFor = runtime.createStructuralDirectiveTransform('for', runtime.processFor);
  runtime.transformFor = runtime.createStructuralDirectiveTransform('for', (node, dir, context) => {
    return runtime.processFor(node, dir, context, (forNode) => {
      const renderExp = runtime.createCallExpression(context.helper(runtime.RENDER_LIST), [forNode.source]);
      const codegenProjection = callBridge('vue3.core.transformFor', {
        phase: 'codegen',
        node,
        forNode: vue3ForNodePayload(forNode),
        context: vue3TransformForContextPayload(context),
      });
      const keyProperty = materializeVue3ForKeyProperty(codegenProjection && codegenProjection.keyProperty, dir, context);
      const isStableFragment = !!(codegenProjection && codegenProjection.isStableFragment);
      forNode.codegenNode = runtime.createVNodeCall(
        context,
        context.helper(runtime.FRAGMENT),
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
        materializeVue3ForTemplateKeyErrors(forNode.__vuecProjection, node, dir, context);
        const exitProjection = callBridge('vue3.core.transformFor', {
          phase: 'exitCodegen',
          node,
          forNode: vue3ForNodePayload(forNode),
          isStableFragment,
        });
        const childBlock = materializeVue3ForChildBlock(exitProjection, node, forNode, keyProperty, context);
        renderExp.arguments.push(runtime.createFunctionExpression(runtime.createForLoopParams(forNode.parseResult), childBlock, true));
      };
    });
  });
  runtime.createForLoopParams = function createForLoopParams(parseResult) {
    const args = [parseResult.value, parseResult.key, parseResult.index];
    let i = args.length;
    while (i--) {
      if (args[i]) break;
    }
    return args.slice(0, i + 1).map((arg, index) => arg || runtime.createSimpleExpression(`_`.repeat(index + 1), false));
  };
  runtime.finalizeForCodegen = function finalizeForCodegen(forNode, renderExp, context) {
    if (!renderExp || renderExp.arguments.length > 1) return;
    const children = forNode.children || [];
    let childBlock;
    if (children.length === 1 && children[0].type === NodeTypes.ELEMENT) {
      childBlock = children[0].codegenNode;
      if (childBlock && childBlock.type === NodeTypes.VNODE_CALL && !childBlock.isBlock) {
        context.removeHelper(runtime.getVNodeHelper(context.inSSR, childBlock.isComponent));
        childBlock.isBlock = true;
        context.helper(runtime.OPEN_BLOCK);
        context.helper(runtime.getVNodeBlockHelper(context.inSSR, childBlock.isComponent));
      }
    } else {
      childBlock = runtime.createVNodeCall(context, context.helper(runtime.FRAGMENT), undefined, children, 64, undefined, undefined, true, undefined, false, forNode.loc);
    }
    renderExp.arguments.push(runtime.createFunctionExpression(runtime.createForLoopParams(forNode.parseResult), childBlock, true));
  };
  runtime.trackSlotScopes = function trackSlotScopes(node, context) {
    if (!node || node.type !== NodeTypes.ELEMENT || !(node.tagType === ElementTypes.COMPONENT || node.tagType === ElementTypes.TEMPLATE)) return;
    const projection = callBridge('vue3.core.trackSlotScopes', { node, context: vue3TransformSlotContextPayload(context) });
    if (!projection || !projection.track) return;
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
  runtime.trackVForSlotScopes = function trackVForSlotScopes(node, context) {
    if (!node || node.type !== NodeTypes.ELEMENT || node.tagType !== ElementTypes.TEMPLATE || !(node.props || []).some(runtime.isVSlot)) return;
    const projection = callBridge('vue3.core.trackVForSlotScopes', { node, context: vue3TransformSlotContextPayload(context) });
    if (!projection || !projection.track) return;
    const parseResult = materializeVue3ForParseResult(projection.parseResult, projection.dir || runtime.findDir(node, 'for', true), context);
    const locals = [parseResult.value, parseResult.key, parseResult.index].filter(Boolean);
    for (const local of locals) context.addIdentifiers(local);
    const dir = runtime.findDir(node, 'for', true);
    if (dir) dir.forParseResult = parseResult;
    return () => {
      for (const local of locals) context.removeIdentifiers(local);
    };
  };
  runtime.buildProps = function buildProps(node, context, props = node && node.props || []) {
    const objectProps = [];
    const mergeArgs = [];
    const directives = [];
    let hasDynamicKey = false;
    const pushMergeArg = arg => {
      if (objectProps.length) {
        mergeArgs.push(runtime.createObjectExpression(runtime.dedupeProperties(objectProps.splice(0)), node && node.loc || locStub));
      }
      if (arg) mergeArgs.push(arg);
    };
    for (const prop of props || []) {
      if (prop.type === NodeTypes.ATTRIBUTE) {
        if (
          prop.name === 'is'
          && (
            node && (node.tag === 'component' || node.tag === 'Component')
            || prop.value && String(prop.value.content || '').startsWith('vue:')
          )
        ) {
          continue;
        }
        objectProps.push(runtime.createObjectProperty(
          runtime.createSimpleExpression(prop.name, true, prop.nameLoc || prop.loc),
          runtime.createSimpleExpression(prop.value ? prop.value.content : '', true, prop.value ? prop.value.loc : prop.loc),
        ));
        continue;
      }
      if (prop.name === 'bind' && prop.arg) {
        if (runtime.isStaticArgOf(prop.arg, 'is') && node && (node.tag === 'component' || node.tag === 'Component')) {
          continue;
        }
        const transform = context && context.directiveTransforms && context.directiveTransforms.bind;
        const result = transform ? transform(prop, node, context) : runtime.transformBind(prop, node, context);
        objectProps.push(...((result && result.props) || []));
        if (result && result.props && result.props.some(p => p.key && !runtime.isStaticExp(p.key))) hasDynamicKey = true;
      } else if (prop.name === 'bind' && !prop.arg) {
        if (prop.exp) {
          pushMergeArg(prop.exp);
          hasDynamicKey = true;
        } else if (context && context.onError) {
          context.onError(runtime.createCompilerError(ErrorCodes.X_V_BIND_NO_EXPRESSION, prop.loc));
        }
      } else if (prop.name === 'on' && prop.arg) {
        const transform = context && context.directiveTransforms && context.directiveTransforms.on;
        const result = transform ? transform(prop, node, context) : runtime.transformOn(prop, node, context);
        objectProps.push(...((result && result.props) || []));
      } else if (prop.name === 'on' && !prop.arg && context && context.inSSR) {
        continue;
      } else if (prop.name === 'model' && context && context.directiveTransforms && context.directiveTransforms.model) {
        const result = context.directiveTransforms.model(prop, node, context);
        const modelProps = (result && result.props) || [];
        objectProps.push(...modelProps);
        if (modelProps.some(p => p.key && !runtime.isStaticExp(p.key))) hasDynamicKey = true;
        for (const modelProp of modelProps) {
          if (modelProp.__vuecModel && modelProp.__vuecModel.dynamic && runtime.isStaticExp(modelProp.key)) {
            dynamicPropNames.push(modelProp.key.content);
          }
        }
      } else if (context && context.directiveTransforms && context.directiveTransforms[prop.name]) {
        const result = context.directiveTransforms[prop.name](prop, node, context);
        objectProps.push(...((result && result.props) || []));
        if (result && result.props && result.props.some(p => p.key && !runtime.isStaticExp(p.key))) hasDynamicKey = true;
      } else if (!runtime.isBuiltInDirective(prop.name)) {
        directives.push(prop);
      }
    }
    let propsExpression;
    if (mergeArgs.length) {
      pushMergeArg();
      propsExpression = mergeArgs.length > 1
        ? runtime.createCallExpression(context && context.helper ? context.helper(runtime.MERGE_PROPS) : runtime.MERGE_PROPS, mergeArgs, node && node.loc || locStub)
        : mergeArgs[0];
    } else if (objectProps.length) {
      propsExpression = runtime.createObjectExpression(runtime.dedupeProperties(objectProps), node && node.loc || locStub);
    }
    if (propsExpression && hasDynamicKey && context && !context.inSSR) {
      propsExpression = runtime.createCallExpression(context.helper(runtime.NORMALIZE_PROPS), [propsExpression], node && node.loc || locStub);
    }
    return {
      props: propsExpression,
      directives,
      patchFlag: 0,
      dynamicPropNames: [],
      shouldUseBlock: false,
    };
  };
  runtime.buildDirectiveArgs = function buildDirectiveArgs(dir, context) {
    const projection = callBridge('vue3.core.buildDirectiveArgs', {
      dir,
      needRuntime: vue3DirectiveRuntimePayload(dir && dir.__vuecNeedRuntime),
    });
    const elements = materializeVue3DirectiveArgsProjection(projection, dir, context);
    return runtime.createArrayExpression(elements);
  };
  runtime.buildSlots = function buildSlots(node, context, buildSlotFn) {
    const projection = callBridge('vue3.core.buildSlots', {
      node,
      context: vue3TransformSlotContextPayload(context),
    });
    materializeVue3SlotErrors(projection, node, context);
    const slots = materializeVue3SlotsProjection(projection, node, context, buildSlotFn);
    return {
      slots,
      hasDynamicSlots: !!(projection && projection.hasDynamicSlots),
    };
  };
  runtime.resolveComponentType = function resolveComponentType(node, context, ssr = false) {
    const projection = callBridge('vue3.core.resolveComponentType', {
      node,
      context: vue3ResolveComponentContextPayload(context),
      ssr: !!ssr,
    });
    return materializeVue3ComponentTypeProjection(projection, node, context);
  };
  runtime.getBaseTransformPreset = function getBaseTransformPreset(prefixIdentifiers = false) {
    return [[
      runtime.transformOnce,
      runtime.transformIf,
      runtime.transformMemo,
      runtime.transformFor,
      ...(prefixIdentifiers ? [runtime.trackVForSlotScopes] : []),
      runtime.transformExpression,
      runtime.transformSlotOutlet,
      runtime.transformElement,
      runtime.trackSlotScopes,
      runtime.transformText,
    ], { on: runtime.transformOn, bind: runtime.transformBind, model: runtime.transformModel }];
  };
  runtime.getConstantType = function getConstantType(node) {
    if (!node) return ConstantTypes.NOT_CONSTANT;
    const projection = callBridge('vue3.core.getConstantType', {
      node: runtime.dehydrateForBridge(node),
      context: vue3CacheStaticContextPayload(arguments[1]),
    });
    return projection && projection.constantType || ConstantTypes.NOT_CONSTANT;
  };
  runtime.cacheStatic = function cacheStatic(root, context) {
    const projection = callBridge('vue3.core.cacheStatic', {
      root: runtime.dehydrateForBridge(root),
      context: vue3CacheStaticContextPayload(context),
    });
    for (const operation of projection && projection.operations || []) {
      materializeVue3CacheStaticOperation(operation, root, context);
    }
    if (context && typeof context.transformHoist === 'function') {
      vue3ApplyTransformHoist(root, context);
    }
  };
  runtime.stringifyStatic = function stringifyStatic(children, context, parent) {
    if (!Array.isArray(children)) return;
    const projection = callBridge('vue3.core.stringifyStatic', {
      children: runtime.dehydrateForBridge(children),
      parent: runtime.dehydrateForBridge(parent),
      context: vue3StringifyStaticContextPayload(context),
    });
    materializeVue3StringifyStaticProjection(projection, children, context, runtime);
  };
  runtime.transformOnce = function transformOnce(node, context) {
    const projection = callBridge('vue3.core.transformOnce', {
      node: runtime.dehydrateForBridge(node),
      context: vue3TransformOnceContextPayload(context),
      seen: !!(node && node.__vuecOnceSeen),
    });
    if (!projection || projection.kind !== 'enter') return;
    if (projection.markSeen) {
      Object.defineProperty(node, '__vuecOnceSeen', { value: true, configurable: true });
    }
    if (projection.enterInVOnce) context.inVOnce = true;
    if (projection.helper === 'SET_BLOCK_TRACKING') context.helper(runtime.SET_BLOCK_TRACKING);
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
  };
  runtime.transformMemo = function transformMemo(node, context) {
    const projection = callBridge('vue3.core.transformMemo', {
      node: runtime.dehydrateForBridge(node),
      context: vue3TransformMemoContextPayload(context),
      seen: !!(node && node.__vuecMemoSeen),
    });
    if (!projection || projection.kind !== 'enter') return;
    if (projection.markSeen) {
      Object.defineProperty(node, '__vuecMemoSeen', { value: true, configurable: true });
    }
    return () => {
      const exit = projection.exit || {};
      if (!exit.wrapMemo) return;
      const current = context.currentNode || node;
      const codegenNode = node.codegenNode || current && current.codegenNode;
      if (!codegenNode || codegenNode.type !== NodeTypes.VNODE_CALL) return;
      if (exit.convertToBlock) runtime.convertToBlock(codegenNode, context);
      node.codegenNode = runtime.createCallExpression(context.helper(runtime.WITH_MEMO), [
        exit.exp,
        runtime.createFunctionExpression(undefined, codegenNode),
        '_cache',
        String(exit.cacheIndex || context.cached.length),
      ]);
      context.cached.push(null);
    };
  };
  runtime.checkCompatEnabled = () => false;
  runtime.warnDeprecation = () => {};
  const DOMErrorMessages = {
    54: 'v-html is missing expression.',
    55: 'v-html will override element children.',
    56: 'v-text is missing expression.',
    57: 'v-text will override element children.',
    58: 'v-model can only be used on <input>, <textarea> and <select> elements.',
    59: 'v-model argument is not supported on plain elements.',
    60: 'v-model cannot be used on file inputs since they are read-only. Use a v-on:change listener instead.',
    61: "Unnecessary value binding used alongside v-model. It will interfere with v-model's behavior.",
    62: 'v-show is missing expression.',
    63: '<Transition> expects exactly one child element or component.',
    64: 'Tags with side effect (<script> and <style>) are ignored in client component templates.',
  };
  runtime.createDOMCompilerError = function createDOMCompilerError(code, loc) {
    return runtime.createCompilerError(code, loc, DOMErrorMessages);
  };
  function vue3DomDirectiveLoc(loc, dir, node) {
    if (loc && typeof loc === 'object') return loc;
    if (loc === 'dir') return dir && dir.loc || locStub;
    if (loc === 'node') return node && node.loc || dir && dir.loc || locStub;
    return locStub;
  }
  function materializeVue3DomDirectiveValue(projection, dir, node, context) {
    if (!projection || projection.kind === 'undefined') return undefined;
    if (projection.type) return projection;
    switch (projection.kind) {
      case 'node':
        if (projection.path === 'dir.exp') return dir && dir.exp;
        if (projection.path === 'dir.arg') return dir && dir.arg;
        return undefined;
      case 'simple': {
        const loc = Object.prototype.hasOwnProperty.call(projection, 'loc')
          ? vue3DomDirectiveLoc(projection.loc, dir, node)
          : locStub;
        const simple = runtime.createSimpleExpression(projection.content || '', !!projection.isStatic, loc);
        if (projection.constType !== undefined) simple.constType = projection.constType;
        return simple;
      }
      case 'displayString': {
        const argument = materializeVue3DomDirectiveValue(projection.argument, dir, node, context);
        const helper = runtime.TO_DISPLAY_STRING;
        const callee = context && typeof context.helperString === 'function'
          ? context.helperString(helper)
          : `_${runtime.helperNameMap[helper] || 'toDisplayString'}`;
        return runtime.createCallExpression(
          callee,
          [argument],
          vue3DomDirectiveLoc(projection.loc, dir, node),
        );
      }
      default:
        throw new Error(`Unsupported Rust Vue 3 DOM directive projection: ${projection.kind}`);
    }
  }
  function materializeVue3DomDirectiveErrors(projection, dir, node, context) {
    if (!projection || !Array.isArray(projection.errors) || !context || typeof context.onError !== 'function') return;
    for (const error of projection.errors) {
      context.onError(runtime.createDOMCompilerError(error.code, vue3DomDirectiveLoc(error.loc, dir, node)));
    }
  }
  function materializeVue3DomModelErrors(projection, dir, node, context) {
    if (!projection || !Array.isArray(projection.errors) || !context || typeof context.onError !== 'function') return;
    for (const error of projection.errors) {
      const code = typeof error === 'number' ? error : error.code;
      const loc = vue3DomDirectiveLoc(error && error.loc, dir, node);
      context.onError(
        code >= 54
          ? runtime.createDOMCompilerError(code, loc)
          : runtime.createCompilerError(code, loc),
      );
    }
  }
  function materializeVue3DomContentDirective(command, dir, node, context) {
    context = context || {
      helperString: name => `_${runtime.helperNameMap[name] || name}`,
      onError: error => { throw error; },
    };
    const projection = callBridge(command, {
      dir: runtime.dehydrateForBridge(dir),
      node: runtime.dehydrateForBridge(node),
    });
    materializeVue3DomDirectiveErrors(projection, dir, node, context);
    if (projection && projection.clearChildren && node && Array.isArray(node.children)) {
      node.children.length = 0;
    }
    return {
      props: (projection && projection.props || []).map(prop => {
        const key = prop.keyLoc
          ? runtime.createSimpleExpression(prop.key || '', true, vue3DomDirectiveLoc(prop.keyLoc, dir, node))
          : (prop.key || '');
        return runtime.createObjectProperty(
          key,
          materializeVue3DomDirectiveValue(prop.value, dir, node, context),
        );
      }),
    };
  }
  runtime.transformVHtml = function transformVHtml(dir, node, context) {
    return materializeVue3DomContentDirective('vue3.dom.transformVHtml', dir, node, context);
  };
  runtime.transformVText = function transformVText(dir, node, context) {
    return materializeVue3DomContentDirective('vue3.dom.transformVText', dir, node, context);
  };
  runtime.transformShow = function transformShow(dir, node, context) {
    context = context || {
      onError: error => { throw error; },
    };
    const projection = callBridge('vue3.dom.transformShow', {
      dir: runtime.dehydrateForBridge(dir),
    });
    materializeVue3DomDirectiveErrors(projection, dir, node, context);
    return {
      props: [],
      needRuntime: projection && projection.needRuntime,
    };
  };
  runtime.transformDomModel = function transformDomModel(dir, node, context) {
    context = context || { helper: name => name, cache: value => value, onError: error => { throw error; } };
    const projection = callBridge('vue3.dom.transformModel', {
      dir,
      node,
      context: vue3TransformDomModelContextPayload(context, node),
    });
    materializeVue3DomModelErrors(projection, dir, node, context);
    const result = {
      props: (projection.props || []).map(prop => {
        const key = materializeVue3ModelProjection(prop.key, dir, context);
        const value = materializeVue3ModelProjection(prop.value, dir, context);
        const objectProp = runtime.createObjectProperty(key, value);
        objectProp.__vuecModel = {
          dynamic: !!prop.dynamic,
          cache: !!prop.cache,
          hydrate: !!prop.hydrate,
          kind: prop.kind,
        };
        if (prop.cache && context && context.cache) objectProp.value = context.cache(objectProp.value);
        return objectProp;
      }),
    };
    if (projection && projection.needRuntime) {
      const helper = helperSymbolFromProjection(projection.needRuntime, context);
      result.needRuntime = helper && context && context.helper
        ? context.helper(helper)
        : helper || projection.needRuntime;
    }
    return result;
  };
  function vue3DomNodeIsTransition(node, context) {
    if (!node || node.type !== NodeTypes.ELEMENT || node.tagType !== ElementTypes.COMPONENT) return false;
    if (!context || typeof context.isBuiltInComponent !== 'function') return false;
    let component;
    try {
      component = context.isBuiltInComponent(node.tag);
    } catch (_error) {
      component = undefined;
    }
    const transition = context.__vuecDomHelpers && context.__vuecDomHelpers.TRANSITION || vue3CoreRuntime.TRANSITION;
    return component === transition || vue3CoreRuntime.helperNameMap[component] === 'Transition';
  }
  function vue3DomTransitionContextPayload(context, node) {
    return {
      isTransition: vue3DomNodeIsTransition(node, context),
    };
  }
  function materializeVue3DomTransitionProjection(projection, node, context) {
    if (!projection || !projection.transform || !node) return;
    if (Array.isArray(projection.keepChildren) && Array.isArray(node.children)) {
      node.children = projection.keepChildren
        .map(index => node.children[index])
        .filter(Boolean);
    }
    materializeVue3DomDirectiveErrors(projection, null, node, context);
    if (projection.injectPersisted) {
      node.props = node.props || [];
      node.props.push({
        type: NodeTypes.ATTRIBUTE,
        name: 'persisted',
        nameLoc: node.loc,
        value: undefined,
        loc: node.loc,
      });
    }
  }
  runtime.ignoreSideEffectTags = function ignoreSideEffectTags(node, context) {
    const projection = callBridge('vue3.dom.ignoreSideEffectTags', { node });
    materializeVue3DomDirectiveErrors(projection, null, node, context);
    if (projection && projection.remove && context && typeof context.removeNode === 'function') {
      context.removeNode();
    }
  };
  runtime.transformDomTransition = function transformDomTransition(node, context) {
    if (!vue3DomNodeIsTransition(node, context)) return;
    return () => {
      const projection = callBridge('vue3.dom.transformTransition', {
        node,
        context: vue3DomTransitionContextPayload(context, node),
      });
      materializeVue3DomTransitionProjection(projection, node, context);
    };
  };
  runtime.isValidHTMLNesting = function isValidHTMLNesting(parent, child) {
    const projection = callBridge('vue3.dom.isValidHTMLNesting', {
      parent: String(parent || ''),
      child: String(child || ''),
    });
    return !!(projection && projection.valid);
  };
  function materializeVue3DomNestingWarnings(projection, context) {
    if (!projection || !Array.isArray(projection.warnings) || !context || typeof context.onWarn !== 'function') return;
    for (const warning of projection.warnings) {
      const error = new SyntaxError(String(warning.message || ''));
      error.loc = warning.loc || vue3CoreRuntime.locStub;
      context.onWarn(error);
    }
  }
  runtime.validateHtmlNesting = function validateHtmlNesting(node, context) {
    const projection = callBridge('vue3.dom.validateHtmlNesting', {
      node,
      parent: context && context.parent,
    });
    materializeVue3DomNestingWarnings(projection, context);
  };
  runtime.transformDomOn = function transformDomOn(dir, node, context) {
    context = context || { helperString: name => `_${runtime.helperNameMap[name] || name}`, helper: name => name, cache: value => value, onError: error => { throw error; } };
    const projection = callBridge('vue3.dom.transformOn', {
      dir,
      node,
      context: vue3TransformOnContextPayload(context),
    });
    materializeVue3OnErrors(projection, dir, context);
    const onMeta = (projection.props || []).map(prop => ({
      cache: !!prop.cache,
      valueConstant: !!prop.valueConstant,
      handlerKey: !!prop.handlerKey,
      dynamicKey: !!prop.dynamicKey,
      ignoreDynamicKeyForNormalize: !!prop.ignoreDynamicKeyForNormalize,
    }));
    const result = {
      props: (projection.props || []).map(prop => {
        const key = materializeVue3OnProjection(prop.key, dir, context);
        const value = materializeVue3OnProjection(prop.value, dir, context) || runtime.createSimpleExpression('() => {}', false, dir.loc);
        return runtime.createObjectProperty(key, value);
      }),
    };
    for (const [index, prop] of (result.props || []).entries()) {
      const meta = onMeta[index] || onMeta[0] || {};
      if (prop.key && meta.handlerKey) prop.key.isHandlerKey = true;
      if (meta.cache && context && context.cache) prop.value = context.cache(prop.value);
      prop.__vuecOn = meta;
    }
    return result;
  };
  runtime.transformStyle = function transformStyle(node) {
    if (!node || node.type !== NodeTypes.ELEMENT) return;
    const projection = callBridge('vue3.dom.transformStyle', { node });
    for (const replacement of projection && projection.replacements || []) {
      const original = node.props && node.props[replacement.index];
      if (!original || original.type !== NodeTypes.ATTRIBUTE) continue;
      node.props[replacement.index] = {
        type: NodeTypes.DIRECTIVE,
        name: 'bind',
        rawName: ':style',
        arg: runtime.createSimpleExpression('style', true, original.loc),
        exp: runtime.createSimpleExpression(replacement.expression || '{}', false, original.loc, ConstantTypes.CAN_STRINGIFY),
        modifiers: [],
        loc: original.loc,
      };
    }
  };
  runtime.decodeHtmlBrowser = function decodeHtmlBrowser(raw, asAttr = false) {
    const source = String(raw == null ? '' : raw);
    const projection = callBridge('vue3.dom.decodeHtmlBrowser', {
      raw: source,
      asAttr: !!asAttr,
    });
    return projection && typeof projection.decoded === 'string'
      ? projection.decoded
      : source;
  };
  [
    runtime.transformOnce,
    runtime.transformIf,
    runtime.transformMemo,
    runtime.transformFor,
    runtime.trackVForSlotScopes,
    runtime.transformExpression,
    runtime.transformSlotOutlet,
    runtime.transformElement,
    runtime.trackSlotScopes,
    runtime.transformText,
    runtime.transformDomTransition,
    runtime.ignoreSideEffectTags,
    runtime.validateHtmlNesting,
  ].forEach(markVuecRuntimeCallback);
  [
    runtime.transformOn,
    runtime.transformBind,
    runtime.transformModel,
    runtime.transformDomOn,
    runtime.transformDomModel,
    runtime.transformShow,
    runtime.transformVHtml,
    runtime.transformVText,
    runtime.transformStyle,
  ].forEach(markVuecRuntimeCallback);
  return runtime;
})();

function capitalize(value) {
  value = String(value || '');
  return value ? value.charAt(0).toUpperCase() + value.slice(1) : value;
}

function camelize(value) {
  return String(value || '').replace(/-(\w)/g, (_, c) => c ? c.toUpperCase() : '');
}

function toHandlerKey(value) {
  value = String(value || '');
  return value ? `on${capitalize(value)}` : '';
}

function stringifyDynamicPropNames(props) {
  return `[${(props || []).map(prop => JSON.stringify(prop)).join(', ')}]`;
}

function selfNameFromFilename(filename) {
  const match = String(filename).replace(/\?.*$/, '').match(/([^/\\]+)\.\w+$/);
  if (!match) return null;
  return match[1].replace(/(^|[-_])(\w)/g, (_, _sep, ch) => ch.toUpperCase());
}

function vue3TransformModelContextPayload(context) {
  context = context || {};
  return {
    prefixIdentifiers: !!context.prefixIdentifiers,
    cacheHandlers: !!context.cacheHandlers,
    inVOnce: !!context.inVOnce,
    inline: !!context.inline,
    isTS: !!context.isTS,
    identifiers: context.identifiers || {},
    bindingMetadata: context.bindingMetadata || {},
  };
}

function vue3TransformDomModelContextPayload(context, node) {
  context = context || {};
  let isCustomElement = false;
  if (typeof context.isCustomElement === 'function' && node) {
    try {
      isCustomElement = !!context.isCustomElement(node.tag);
    } catch (_error) {
      isCustomElement = false;
    }
  }
  return {
    prefixIdentifiers: !!context.prefixIdentifiers,
    cacheHandlers: !!context.cacheHandlers,
    inVOnce: !!context.inVOnce,
    inline: !!context.inline,
    isTS: !!context.isTS,
    identifiers: context.identifiers || {},
    bindingMetadata: context.bindingMetadata || {},
    isCustomElement,
  };
}

function vue3TransformOnContextPayload(context) {
  context = context || {};
  return {
    prefixIdentifiers: !!context.prefixIdentifiers,
    cacheHandlers: !!context.cacheHandlers,
    inVOnce: !!context.inVOnce,
    inline: !!context.inline,
    isTS: !!context.isTS,
    identifiers: context.identifiers || {},
    bindingMetadata: context.bindingMetadata || {},
  };
}

function vue3TransformBindContextPayload(context) {
  context = context || {};
  return {
    inSSR: !!context.inSSR,
    browser: vue3CoreRuntime.isBrowserBuild ? vue3CoreRuntime.isBrowserBuild() : false,
  };
}

function vue3TransformVBindShorthandContextPayload(_context) {
  return {
    browser: vue3CoreRuntime.isBrowserBuild ? vue3CoreRuntime.isBrowserBuild() : false,
  };
}

function vue3TransformOnceContextPayload(context) {
  context = context || {};
  return {
    inVOnce: !!context.inVOnce,
    inSSR: !!context.inSSR,
  };
}

function vue3TransformMemoContextPayload(context) {
  context = context || {};
  return {
    inSSR: !!context.inSSR,
    cachedLength: Array.isArray(context.cached) ? context.cached.length : 0,
  };
}

function materializeVue3OnErrors(projection, dir, context) {
  if (!projection || !Array.isArray(projection.errors) || !context || !context.onError) return;
  for (const error of projection.errors) {
    const loc = error.loc === 'arg'
      ? dir && dir.arg && dir.arg.loc || dir && dir.loc
      : dir && dir.loc || locStub;
    context.onError(vue3CoreRuntime.createCompilerError(error.code, loc));
  }
}

function materializeVue3BindErrors(projection, dir, context) {
  if (!projection || !Array.isArray(projection.errors) || !context || !context.onError) return;
  for (const error of projection.errors) {
    const code = typeof error === 'number' ? error : error.code;
    const loc = error && error.loc === 'arg'
      ? dir && dir.arg && dir.arg.loc || dir && dir.loc || locStub
      : dir && dir.loc || locStub;
    context.onError(vue3CoreRuntime.createCompilerError(code, loc));
  }
}

function materializeVue3VBindShorthandProjection(projection, node, context) {
  for (const operation of projection && projection.operations || []) {
    const prop = node && node.props && node.props[operation.index];
    if (!prop || operation.kind !== 'setExp') continue;
    for (const error of operation.errors || []) {
      if (context && context.onError) {
        const loc = error.loc === 'arg'
          ? prop.arg && prop.arg.loc || prop.loc || vue3CoreRuntime.locStub
          : prop.loc || vue3CoreRuntime.locStub;
        context.onError(vue3CoreRuntime.createCompilerError(error.code, loc));
      }
    }
    prop.exp = materializeVue3VBindShorthandExpression(operation.exp, prop);
  }
}

function materializeVue3VBindShorthandExpression(projection, prop) {
  if (!projection || projection.kind !== 'simple') return undefined;
  return vue3CoreRuntime.createSimpleExpression(
    projection.content || '',
    !!projection.isStatic,
    projection.loc || prop && prop.arg && prop.arg.loc || prop && prop.loc || vue3CoreRuntime.locStub,
    projection.constType || 0,
  );
}

function materializeVue3OnProjection(projection, dir, context) {
  if (!projection || projection.kind === 'undefined') return undefined;
  if (typeof projection === 'string') return projection;
  if (projection.type) return projection;
  switch (projection.kind) {
    case 'node':
      if (projection.path === 'dir.arg') return dir && dir.arg;
      if (projection.path === 'dir.exp') return dir && dir.exp;
      if (projection.path === 'dir.arg.children') return (dir && dir.arg && dir.arg.children) || [];
      return undefined;
    case 'children': {
      const children = [];
      for (const child of projection.children || []) {
        const materialized = materializeVue3OnProjection(child, dir, context);
        if (Array.isArray(materialized)) children.push(...materialized);
        else children.push(materialized);
      }
      return children;
    }
    case 'helperString': {
      const helper = helperSymbolFromProjection(projection.helper, context);
      return `${context && helper ? context.helperString(helper) : `_${vue3CoreRuntime.helperNameMap[helper]}`}(`;
    }
    case 'static':
      return vue3CoreRuntime.createSimpleExpression(
        projection.content || '',
        true,
        projection.loc || (dir && dir.loc) || locStub,
      );
    case 'simple':
      for (const name of projection.helpers || []) {
        const symbol = helperSymbolFromProjection(name);
        if (symbol && context) context.helper(symbol);
      }
      return vue3CoreRuntime.createSimpleExpression(
        projection.content || '',
        !!projection.isStatic,
        projection.loc || (dir && dir.exp && dir.exp.loc) || (dir && dir.loc) || locStub,
        projection.constType || 0,
      );
    case 'compound': {
      for (const name of projection.helpers || []) {
        const symbol = helperSymbolFromProjection(name);
        if (symbol && context) context.helper(symbol);
      }
      const children = [];
      for (const child of projection.children || []) {
        const materialized = materializeVue3OnProjection(child, dir, context);
        if (Array.isArray(materialized)) children.push(...materialized);
        else children.push(materialized);
      }
      return vue3CoreRuntime.createCompoundExpression(
        children,
        projection.loc || (dir && dir.arg && dir.arg.loc) || (dir && dir.exp && dir.exp.loc) || locStub,
      );
    }
    case 'call': {
      const helper = helperSymbolFromProjection(projection.callee || projection.helper, context);
      if (helper && context) context.helper(helper);
      const args = (projection.arguments || []).map(arg => materializeVue3OnProjection(arg, dir, context));
      return vue3CoreRuntime.createCallExpression(
        helper || projection.callee || projection.helper,
        args,
        projection.loc || (dir && dir.loc) || locStub,
      );
    }
    default:
      throw new Error(`Unsupported Rust v-on projection: ${projection.kind}`);
  }
}

function materializeVue3ModelProjection(projection, dir, context) {
  if (!projection || projection.kind === 'undefined') return undefined;
  if (typeof projection === 'string') return projection;
  if (projection.type) return projection;
  switch (projection.kind) {
    case 'node':
      return projection.path === 'dir.arg' ? dir.arg : dir.exp;
    case 'static':
      return vue3CoreRuntime.createSimpleExpression(projection.content || '', true);
    case 'simple':
      return vue3CoreRuntime.createSimpleExpression(
        projection.content || '',
        !!projection.isStatic,
        projection.loc || (projection.path === 'dir.arg' ? dir.arg && dir.arg.loc : dir.exp && dir.exp.loc),
        projection.constType || 0,
      );
    case 'compound': {
      if (context && Array.isArray(projection.helpers)) {
        for (const helper of projection.helpers) {
          if (helper === 'IS_REF') context.helper(vue3CoreRuntime.IS_REF);
        }
      }
      return vue3CoreRuntime.createCompoundExpression(
        (projection.children || []).map(child => materializeVue3ModelProjection(child, dir, context)),
      );
    }
    default:
      throw new Error(`Unsupported Rust v-model projection: ${projection.kind}`);
  }
}

function vue3TransformIfContextPayload(context) {
  context = context || {};
  return {
    prefixIdentifiers: !!context.prefixIdentifiers,
    inline: !!context.inline,
    isTS: !!context.isTS,
    identifiers: context.identifiers || {},
    bindingMetadata: context.bindingMetadata || {},
  };
}

function vue3TransformForContextPayload(context) {
  context = context || {};
  return {
    prefixIdentifiers: !!context.prefixIdentifiers,
    inline: !!context.inline,
    isTS: !!context.isTS,
    identifiers: context.identifiers || {},
    bindingMetadata: context.bindingMetadata || {},
  };
}

function vue3TransformElementContextPayload(context) {
  context = context || {};
  return {
    inSSR: !!context.inSSR,
    inline: !!context.inline,
    bindingMetadata: context.bindingMetadata || {},
    vForDepth: context.scopes && context.scopes.vFor || 0,
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
    vForDepth: context.scopes && context.scopes.vFor || 0,
    vSlotDepth: context.scopes && context.scopes.vSlot || 0,
  };
}

function vue3TransformSlotOutletContextPayload(context) {
  context = context || {};
  return {
    prefixIdentifiers: !!context.prefixIdentifiers,
    inline: !!context.inline,
    isTS: !!context.isTS,
    scopeId: context.scopeId || undefined,
    slotted: !!context.slotted,
    identifiers: context.identifiers || {},
    bindingMetadata: context.bindingMetadata || {},
  };
}

function vue3TransformTextContextPayload(context) {
  context = context || {};
  return {
    compat: typeof __COMPAT__ !== 'undefined' && !!__COMPAT__,
    ssr: !!context.ssr,
    inSSR: !!context.inSSR,
    directiveTransforms: Object.keys(context.directiveTransforms || {}),
    identifiers: context.identifiers || {},
    bindingMetadata: context.bindingMetadata || {},
  };
}

function vue3CacheStaticContextPayload(context) {
  context = context || {};
  return {
    prefixIdentifiers: !!context.prefixIdentifiers,
    hmr: !!context.hmr,
    inSSR: !!context.inSSR,
    identifiers: context.identifiers || {},
    bindingMetadata: context.bindingMetadata || {},
  };
}

function vue3StringifyStaticContextPayload(context) {
  context = context || {};
  return {
    scopeId: context.scopeId || undefined,
    scopes: {
      vSlot: Number(context.scopes && context.scopes.vSlot || 0),
    },
  };
}

function vue3ProcessExpressionContextPayload(context) {
  context = context || {};
  return {
    prefixIdentifiers: !!context.prefixIdentifiers,
    inline: !!context.inline,
    isTS: !!context.isTS,
    expressionPlugins: context.expressionPlugins || [],
    identifiers: context.identifiers || {},
    bindingMetadata: context.bindingMetadata || {},
  };
}

function vue3ExpressionUtilityContextPayload(context) {
  context = context || {};
  return {
    expressionPlugins: context.expressionPlugins || [],
    isTS: !!context.isTS,
    allowLexerFallback: false,
  };
}

function materializeVue3ProcessExpressionProjection(projection, node, context) {
  if (!projection || projection.kind === 'unchanged') return node;
  if (projection.kind === 'error') {
    if (context && context.onError) {
      context.onError(vue3CoreRuntime.createCompilerError(
        projection.code || vue3CoreRuntime.ErrorCodes.X_INVALID_EXPRESSION,
        projection.loc || node.loc,
        undefined,
        projection.message || 'Error parsing JavaScript expression',
      ));
    }
    return node;
  }
  if (projection.kind === 'setConstType') {
    node.constType = Number(projection.constType || 0);
    return node;
  }
  if (Array.isArray(projection.helpers) && context && context.helper) {
    for (const helper of projection.helpers) {
      if (helper === 'UNREF') context.helper(vue3CoreRuntime.UNREF);
      else if (helper === 'IS_REF') context.helper(vue3CoreRuntime.IS_REF);
    }
  }
  if (projection.kind === 'simple') {
    node.content = projection.content || '';
    node.isStatic = !!projection.isStatic;
    node.constType = Number(projection.constType || 0);
    if (projection.loc) node.loc = projection.loc;
    return node;
  }
  if (projection.kind === 'compound') {
    const compound = vue3CoreRuntime.createCompoundExpression(
      (projection.children || []).map(child => materializeVue3ProcessExpressionChild(child, context)),
      projection.loc || node.loc,
    );
    compound.identifiers = projection.identifiers || [];
    return compound;
  }
  throw new Error(`Unsupported Rust processExpression projection: ${projection.kind}`);
}

function materializeVue3ProcessExpressionChild(child, context) {
  if (typeof child === 'string') return child;
  if (!child || typeof child !== 'object') return child;
  if (child.kind === 'simple') {
    return vue3CoreRuntime.createSimpleExpression(
      child.content || '',
      !!child.isStatic,
      child.loc || vue3CoreRuntime.locStub,
      Number(child.constType || 0),
    );
  }
  if (child.kind === 'compound') {
    return materializeVue3ProcessExpressionProjection(
      child,
      vue3CoreRuntime.createSimpleExpression('', false),
      context,
    );
  }
  return child;
}

function materializeVue3TransformExpressionProjection(projection, node, context) {
  if (!projection || !Array.isArray(projection.operations)) return;
  for (const operation of projection.operations) {
    if (!operation || operation.kind !== 'process') continue;
    const holder = vue3HolderAtPath(node, operation.path);
    if (!holder || !holder.owner) continue;
    const current = holder.owner[holder.key];
    holder.owner[holder.key] = materializeVue3ProcessExpressionProjection(
      operation.projection,
      current,
      context,
    );
  }
}

function materializeVue3TransformTextProjection(projection, node, context) {
  if (!projection || !Array.isArray(projection.operations) || !node || !Array.isArray(node.children)) return;
  const children = node.children;
  for (const operation of projection.operations) {
    if (!operation || !operation.kind) continue;
    if (operation.kind === 'mergeText') {
      const start = Number(operation.start || 0);
      const end = Number(operation.end || start);
      if (start < 0 || end < start || end >= children.length) continue;
      const mergedChildren = [];
      for (let i = start; i <= end; i++) {
        if (i > start) mergedChildren.push(' + ');
        mergedChildren.push(children[i]);
      }
      children.splice(start, end - start + 1, vue3CoreRuntime.createCompoundExpression(mergedChildren, children[start] && children[start].loc || vue3CoreRuntime.locStub));
    } else if (operation.kind === 'wrapTextCall') {
      const index = Number(operation.index || 0);
      const child = children[index];
      if (!child) continue;
      const callArgs = [];
      if (operation.includeContent !== false) callArgs.push(child);
      if (operation.patchFlag) callArgs.push(operation.patchFlag);
      children[index] = {
        type: vue3CoreRuntime.NodeTypes.TEXT_CALL,
        content: child,
        loc: child.loc,
        codegenNode: vue3CoreRuntime.createCallExpression(
          context.helper(vue3CoreRuntime.CREATE_TEXT),
          callArgs,
        ),
      };
    } else {
      throw new Error(`Unsupported Rust transformText projection: ${operation.kind}`);
    }
  }
}

function materializeVue3CacheStaticOperation(operation, root, context) {
  if (!operation || !operation.kind) return;
  switch (operation.kind) {
    case 'setPatchFlag': {
      const target = vue3NodeAtPath(root, operation.path);
      if (target) target.patchFlag = operation.patchFlag;
      return;
    }
    case 'appendTextCallPatchFlag': {
      const target = vue3NodeAtPath(root, operation.path);
      if (target && target.type === vue3CoreRuntime.NodeTypes.JS_CALL_EXPRESSION && target.arguments && target.arguments.length > 0 && target.arguments.length < 2) {
        target.arguments.push(operation.patchFlag || '-1 /* CACHED */');
      }
      return;
    }
    case 'setBlock': {
      const target = vue3NodeAtPath(root, operation.path);
      if (target && target.type === vue3CoreRuntime.NodeTypes.VNODE_CALL && target.isBlock !== !!operation.isBlock) {
        vue3SetVNodeBlock(target, !!operation.isBlock, context);
      }
      return;
    }
    case 'cacheCodegen': {
      const holder = vue3HolderAtPath(root, operation.path);
      if (holder && holder.owner) holder.owner[holder.key] = context.cache(holder.owner[holder.key]);
      return;
    }
    case 'cacheChildrenArray': {
      const holder = vue3HolderAtPath(root, operation.path);
      const children = vue3NodeAtPath(root, operation.childrenPath);
      if (holder && holder.owner && Array.isArray(children)) {
        const cacheExp = context.cache(vue3CoreRuntime.createArrayExpression(children));
        cacheExp.needArraySpread = operation.needArraySpread !== false;
        holder.owner[holder.key] = cacheExp;
      }
      return;
    }
    case 'cacheSlotReturns': {
      const owner = vue3NodeAtPath(root, operation.ownerPath);
      const slot = vue3FindSlotFunction(owner && owner.codegenNode, operation.slot);
      if (slot && Array.isArray(slot.returns)) {
        const cacheExp = context.cache(vue3CoreRuntime.createArrayExpression(slot.returns));
        cacheExp.needArraySpread = operation.needArraySpread !== false;
        slot.returns = cacheExp;
      }
      return;
    }
    case 'hoistProps':
    case 'hoistDynamicProps': {
      const holder = vue3HolderAtPath(root, operation.path);
      if (holder && holder.owner && holder.owner[holder.key]) holder.owner[holder.key] = context.hoist(holder.owner[holder.key]);
      return;
    }
    default:
      throw new Error(`Unsupported Rust cacheStatic projection: ${operation.kind}`);
  }
}

function materializeVue3StringifyStaticProjection(projection, children, context, runtime = vue3CoreRuntime) {
  if (!projection || !Array.isArray(projection.operations) || !Array.isArray(children)) return;
  context = context || {};
  for (const operation of projection.operations) {
    if (!operation || !operation.kind) continue;
    const call = runtime.createCallExpression(
      context && typeof context.helper === 'function'
        ? context.helper(runtime.CREATE_STATIC)
        : runtime.CREATE_STATIC,
      [operation.html || '""', String(operation.domNodes || operation.count || 0)],
    );
    const start = Number(operation.start) || 0;
    const count = Math.max(1, Number(operation.count) || 1);
    if (operation.kind === 'stringifyParentCachedRange') {
      children.splice(start, count, call);
      continue;
    }
    if (operation.kind === 'stringifyCachedChildRange') {
      const first = children[start];
      const last = children[start + count - 1];
      const lastCache = last && last.codegenNode;
      if (first && first.codegenNode) first.codegenNode.value = call;
      if (count > 1) {
        children.splice(start + 1, count - 1);
        const cacheIndex = context.cached && context.cached.indexOf(lastCache);
        if (cacheIndex > -1) {
          for (let index = cacheIndex; index < context.cached.length; index++) {
            const cache = context.cached[index];
            if (cache) cache.index -= count - 1;
          }
          context.cached.splice(cacheIndex - count + 2, count - 1);
        }
      }
      continue;
    }
    throw new Error(`Unsupported Rust stringifyStatic projection: ${operation.kind}`);
  }
}

function vue3ApplyTransformHoist(root, context) {
  vue3ApplyTransformHoistToNode(root, context);
}

function vue3ApplyTransformHoistToNode(node, context) {
  if (!node || !Array.isArray(node.children)) return;
  if (vue3NodeHasCachedChildrenArray(node) || vue3ChildrenHaveCachedNodes(node.children)) {
    context.transformHoist(node.children, context, node);
  }
  for (const child of node.children.slice()) {
    if (child && child.type === vue3CoreRuntime.NodeTypes.ELEMENT && child.tagType === vue3CoreRuntime.ElementTypes.COMPONENT) {
      context.scopes.vSlot++;
      vue3ApplyTransformHoistToNode(child, context);
      context.scopes.vSlot--;
    } else if (child && child.type === vue3CoreRuntime.NodeTypes.IF) {
      for (const branch of child.branches || []) vue3ApplyTransformHoistToNode(branch, context);
    } else {
      vue3ApplyTransformHoistToNode(child, context);
    }
  }
}

function vue3NodeHasCachedChildrenArray(node) {
  return !!(
    node
    && node.type === vue3CoreRuntime.NodeTypes.ELEMENT
    && node.codegenNode
    && node.codegenNode.type === vue3CoreRuntime.NodeTypes.VNODE_CALL
    && node.codegenNode.children
    && node.codegenNode.children.type === vue3CoreRuntime.NodeTypes.JS_CACHE_EXPRESSION
  );
}

function vue3ChildrenHaveCachedNodes(children) {
  return (children || []).some(child => {
    return child && (
      (
        child.type === vue3CoreRuntime.NodeTypes.ELEMENT
        && child.tagType === vue3CoreRuntime.ElementTypes.ELEMENT
        && child.codegenNode
        && child.codegenNode.type === vue3CoreRuntime.NodeTypes.JS_CACHE_EXPRESSION
      )
      || (
        child.type === vue3CoreRuntime.NodeTypes.TEXT_CALL
        && child.codegenNode
        && child.codegenNode.type === vue3CoreRuntime.NodeTypes.JS_CACHE_EXPRESSION
      )
    );
  });
}

function vue3SetVNodeBlock(node, isBlock, context) {
  if (!context || !node || node.isBlock === isBlock) return;
  if (isBlock) {
    context.removeHelper(vue3CoreRuntime.getVNodeHelper(context.inSSR, node.isComponent));
    node.isBlock = true;
    context.helper(vue3CoreRuntime.OPEN_BLOCK);
    context.helper(vue3CoreRuntime.getVNodeBlockHelper(context.inSSR, node.isComponent));
  } else {
    context.removeHelper(vue3CoreRuntime.OPEN_BLOCK);
    context.removeHelper(vue3CoreRuntime.getVNodeBlockHelper(context.inSSR, node.isComponent));
    node.isBlock = false;
    context.helper(vue3CoreRuntime.getVNodeHelper(context.inSSR, node.isComponent));
  }
}

function vue3NodeAtPath(root, path) {
  let current = root;
  for (const part of path || []) {
    if (current == null) return undefined;
    current = current[vue3PathKey(part)];
  }
  return current;
}

function vue3HolderAtPath(root, path) {
  let current = root;
  const parts = path || [];
  for (let i = 0; i < parts.length - 1; i++) {
    if (current == null) return undefined;
    current = current[vue3PathKey(parts[i])];
  }
  if (current == null || !parts.length) return undefined;
  return { owner: current, key: vue3PathKey(parts[parts.length - 1]) };
}

function vue3PathKey(part) {
  return typeof part === 'number' || /^\d+$/.test(String(part)) ? Number(part) : part;
}

function vue3FindSlotFunction(codegenNode, slotProjection) {
  if (!codegenNode || codegenNode.type !== vue3CoreRuntime.NodeTypes.VNODE_CALL) return undefined;
  const children = codegenNode.children;
  if (!children || children.type !== vue3CoreRuntime.NodeTypes.JS_OBJECT_EXPRESSION) return undefined;
  const props = children.properties || [];
  return (props.find(prop => vue3SlotKeyMatches(prop.key, slotProjection)) || {}).value;
}

function vue3SlotKeyMatches(key, slotProjection) {
  if (!key || !slotProjection) return false;
  if (slotProjection.kind === 'static') {
    return key === slotProjection.name || (key.type === vue3CoreRuntime.NodeTypes.SIMPLE_EXPRESSION && key.content === slotProjection.name);
  }
  if (slotProjection.kind === 'dynamic') {
    const node = slotProjection.node;
    return key === node || (
      key.type === vue3CoreRuntime.NodeTypes.COMPOUND_EXPRESSION
      && node
      && node.type === vue3CoreRuntime.NodeTypes.COMPOUND_EXPRESSION
      && vue3ProjectionSource(key) === vue3ProjectionSource(node)
    ) || (
      key.type === vue3CoreRuntime.NodeTypes.SIMPLE_EXPRESSION
      && node
      && node.type === vue3CoreRuntime.NodeTypes.SIMPLE_EXPRESSION
      && key.content === node.content
      && key.isStatic === node.isStatic
    );
  }
  return false;
}

function vue3ProjectionSource(node) {
  if (!node) return '';
  if (typeof node === 'string') return node;
  if (node.type === vue3CoreRuntime.NodeTypes.SIMPLE_EXPRESSION) return String(node.content || '');
  if (node.type === vue3CoreRuntime.NodeTypes.COMPOUND_EXPRESSION) return (node.children || []).map(vue3ProjectionSource).join('');
  return node.loc && node.loc.source || '';
}

function vue3ElementDirectivePropSummaries(dir, result, extra = {}) {
  return ((result && result.props) || []).map(prop => {
    const key = prop && prop.key;
    const value = prop && prop.value;
    return {
      kind: 'directiveProp',
      name: key && key.type === vue3CoreRuntime.NodeTypes.SIMPLE_EXPRESSION && key.isStatic ? key.content : undefined,
      dynamicKey: !(key && key.type === vue3CoreRuntime.NodeTypes.SIMPLE_EXPRESSION && key.isStatic),
      ignoreDynamicKeyForNormalize: !!(prop && prop.__vuecOn && prop.__vuecOn.ignoreDynamicKeyForNormalize),
      valueStartsWithArray: !!(value && value.type === vue3CoreRuntime.NodeTypes.SIMPLE_EXPRESSION && String(value.content || '').trim().startsWith('[')),
      valueStatic: !!(value && value.type === vue3CoreRuntime.NodeTypes.SIMPLE_EXPRESSION && value.isStatic),
      valueType: value && value.type,
      valueConstant: vue3ElementPropValueIsConstant(value) || !!(prop && prop.__vuecOn && prop.__vuecOn.valueConstant),
      valueCached: !!(value && value.type === vue3CoreRuntime.NodeTypes.JS_CACHE_EXPRESSION),
      propModifier: !!extra.propModifier,
      forceBlock: !!extra.forceBlock,
    };
  });
}

  function vue3ElementPropValueIsConstant(value) {
    if (!value) return false;
    if (value.__vuecOn && value.__vuecOn.cache) return true;
    if (value.type === vue3CoreRuntime.NodeTypes.JS_CACHE_EXPRESSION) return true;
  if (value.type === vue3CoreRuntime.NodeTypes.SIMPLE_EXPRESSION) {
    return !!value.isStatic || Number(value.constType || 0) > 0;
  }
  if (value.type === vue3CoreRuntime.NodeTypes.COMPOUND_EXPRESSION) {
    return Number(value.constType || 0) > 0;
  }
  return false;
}

function vue3DirectiveRuntimePayload(needRuntime) {
  if (typeof needRuntime === 'symbol') {
    return { kind: 'helper', helper: projectionNameFromHelperSymbol(needRuntime), helperName: vue3CoreRuntime.helperNameMap[needRuntime] };
  }
  if (typeof needRuntime === 'string') {
    const helper = helperSymbolFromProjection(needRuntime);
    if (helper) {
      return { kind: 'helper', helper: needRuntime, helperName: vue3CoreRuntime.helperNameMap[helper] };
    }
  }
  if (needRuntime) {
    return { kind: 'asset' };
  }
  return null;
}

function projectionNameFromHelperSymbol(symbol) {
  const helperName = vue3CoreRuntime.helperNameMap[symbol];
  if (helperName) return helperName;
  const entries = Object.entries(vue3CoreRuntime).filter(([, value]) => value === symbol);
  if (entries.length) return entries[0][0];
  return symbol && symbol.description;
}

function materializeVue3DirectiveArgsProjection(projection, dir, context) {
  const elements = [];
  const runtimeProjection = projection && projection.runtime || {};
  if (runtimeProjection.kind === 'helper') {
    const helper = helperSymbolFromProjection(runtimeProjection.helper);
    const helperName = runtimeProjection.helperName || (helper && vue3CoreRuntime.helperNameMap[helper]);
    elements.push(context && helper ? context.helperString(helper) : `_${helperName || runtimeProjection.helper || ''}`);
  } else {
    if (context) {
      context.helper(vue3CoreRuntime.RESOLVE_DIRECTIVE);
      context.directives.add(runtimeProjection.name || (dir && dir.name) || '');
    }
    elements.push(vue3CoreRuntime.toValidAssetId(runtimeProjection.name || (dir && dir.name) || '', 'directive'));
  }
  if (projection && projection.includeExp && dir && dir.exp) elements.push(dir.exp);
  if (projection && projection.includeArg && dir && dir.arg) elements.push(dir.arg);
  if (projection && projection.modifiers && projection.modifiers.length) {
    if (!(projection && projection.includeArg)) {
      if (!(projection && projection.includeExp)) elements.push('void 0');
      elements.push('void 0');
    }
    elements.push(vue3CoreRuntime.createObjectExpression((projection.modifiers || []).map(modifier => {
      const name = modifier && modifier.name || '';
      return vue3CoreRuntime.createObjectProperty(
        vue3CoreRuntime.createSimpleExpression(name, true),
        vue3CoreRuntime.createSimpleExpression('true', false, dir && dir.loc || vue3CoreRuntime.locStub, vue3CoreRuntime.ConstantTypes.CAN_SKIP_PATCH),
      );
    }), dir && dir.loc || vue3CoreRuntime.locStub));
  }
  return elements;
}

function materializeVue3ElementSlotsProjection(projection, node, context) {
  const properties = [];
  for (const slot of projection.slots || []) {
    const slotChildren = [];
    for (const index of slot.indices || []) {
      const child = node.children && node.children[index];
      if (!child) continue;
      if (slot.unwrapTemplate && child.type === vue3CoreRuntime.NodeTypes.ELEMENT && child.tag === 'template') {
        slotChildren.push(...(child.children || []));
      } else {
        slotChildren.push(child);
      }
    }
    properties.push(vue3CoreRuntime.createObjectProperty(
      slot.name || 'default',
      vue3CoreRuntime.createFunctionExpression([], slotChildren, false, true, node.loc),
    ));
  }
  properties.push(vue3CoreRuntime.createObjectProperty(
    '_',
    vue3CoreRuntime.createSimpleExpression(projection.slotFlag || '1 /* STABLE */', false),
  ));
  if (context) context.helper(vue3CoreRuntime.WITH_CTX);
  return vue3CoreRuntime.createObjectExpression(properties, node.loc);
}

function materializeVue3SlotErrors(projection, node, context) {
  if (!projection || !Array.isArray(projection.errors) || !context || !context.onError) return;
  for (const error of projection.errors) {
    context.onError(vue3CoreRuntime.createCompilerError(error.code, error.loc || (node && node.loc) || vue3CoreRuntime.locStub));
  }
}

function materializeVue3SlotsProjection(projection, node, context, buildSlotFn) {
  projection = projection || {};
  if (context) context.helper(vue3CoreRuntime.WITH_CTX);
  const properties = [];
  for (const property of projection.properties || []) {
    properties.push(vue3CoreRuntime.createObjectProperty(
      materializeVue3SlotProjectionNode(property.key, node, context),
      materializeVue3SlotFunctionProjection(property, node, context, buildSlotFn),
    ));
  }
  const slotFlag = projection.slotFlag || 1;
  const flagText = projection.slotFlagText || (slotFlag === 2 ? 'DYNAMIC' : slotFlag === 3 ? 'FORWARDED' : 'STABLE');
  properties.push(vue3CoreRuntime.createObjectProperty(
    '_',
    vue3CoreRuntime.createSimpleExpression(`${slotFlag} /* ${flagText} */`, false),
  ));
  let slots = vue3CoreRuntime.createObjectExpression(properties, node && node.loc || vue3CoreRuntime.locStub);
  if (projection.dynamicSlots && projection.dynamicSlots.length) {
    const dynamicSlotArray = vue3CoreRuntime.createArrayExpression(
      projection.dynamicSlots.map(slot => materializeVue3DynamicSlotProjection(slot, node, context, buildSlotFn)),
    );
    if (context) context.helper(vue3CoreRuntime.CREATE_SLOTS);
    slots = vue3CoreRuntime.createCallExpression(
      context ? context.helper(vue3CoreRuntime.CREATE_SLOTS) : vue3CoreRuntime.CREATE_SLOTS,
      [
        slots,
        dynamicSlotArray,
      ],
      node && node.loc || vue3CoreRuntime.locStub,
    );
  }
  return slots;
}

function materializeVue3SlotFunctionProjection(property, node, context, buildSlotFn) {
  const loc = property.loc || (node && node.loc) || vue3CoreRuntime.locStub;
  const params = materializeVue3SlotProjectionNode(property.params, node, context);
  const returns = materializeVue3SlotChildren(property, node);
  if (typeof buildSlotFn === 'function') {
    const vFor = vue3SlotFunctionVFor(property, node, context);
    const fn = buildSlotFn(params, vFor, returns, loc);
    if (property.nonScoped && context && context.compatConfig && fn) fn.isNonScopedSlot = true;
    return fn;
  }
  const fn = vue3CoreRuntime.createFunctionExpression(params, returns, false, true, returns.length ? returns[0].loc : loc);
  if (property.nonScoped && context && context.compatConfig) fn.isNonScopedSlot = true;
  return fn;
}

function vue3SlotFunctionVFor(property, node, context) {
  for (const index of property.indices || []) {
    const child = node && node.children && node.children[index];
    const source = property.unwrapTemplate && child && child.type === vue3CoreRuntime.NodeTypes.ELEMENT && child.tag === 'template'
      ? child
      : null;
    const dir = source && vue3CoreRuntime.findDir(source, 'for', true);
    if (!dir) continue;
    if (!dir.forParseResult) {
      const projection = callBridge('vue3.core.trackVForSlotScopes', {
        node: source,
        context: vue3TransformSlotContextPayload(context),
      });
      if (projection && projection.parseResult) {
        dir.forParseResult = materializeVue3ForParseResult(projection.parseResult, dir, context);
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
    if (property.unwrapTemplate && child.type === vue3CoreRuntime.NodeTypes.ELEMENT && child.tag === 'template') {
      out.push(...(child.children || []));
    } else {
      out.push(child);
    }
  }
  return out;
}

function materializeVue3DynamicSlotProjection(projection, node, context, buildSlotFn) {
  if (!projection) return vue3CoreRuntime.createSimpleExpression('undefined', false);
  if (projection.kind === 'conditional') {
    return vue3CoreRuntime.createConditionalExpression(
      materializeVue3SlotProjectionNode(projection.test, node, context),
      materializeVue3DynamicSlotProjection(projection.consequent, node, context, buildSlotFn),
      materializeVue3DynamicSlotProjection(projection.alternate, node, context, buildSlotFn),
    );
  }
  if (projection.kind === 'for') {
    const params = projection.params || {};
    const slot = materializeVue3DynamicSlotProjection(projection.slot, node, context, buildSlotFn);
    const source = materializeVue3SlotProjectionNode(projection.source, node, context);
    const loopParams = vue3CoreRuntime.createForLoopParams({
      value: materializeVue3SlotProjectionNode(params.value, node, context),
      key: materializeVue3SlotProjectionNode(params.key, node, context),
      index: materializeVue3SlotProjectionNode(params.index, node, context),
    });
    const renderListHelper = context ? context.helper(vue3CoreRuntime.RENDER_LIST) : vue3CoreRuntime.RENDER_LIST;
    return vue3CoreRuntime.createCallExpression(
      renderListHelper,
      [
        source,
        vue3CoreRuntime.createFunctionExpression(
          loopParams,
          slot,
          true,
        ),
      ],
      node && node.loc || vue3CoreRuntime.locStub,
    );
  }
  if (projection.kind === 'dynamicSlot') {
    const properties = [
      vue3CoreRuntime.createObjectProperty('name', materializeVue3SlotProjectionNode(projection.name, node, context)),
      vue3CoreRuntime.createObjectProperty('fn', materializeVue3SlotFunctionProjection(projection.slot || {}, node, context, buildSlotFn)),
    ];
    if (projection.key != null) {
      properties.push(vue3CoreRuntime.createObjectProperty('key', vue3CoreRuntime.createSimpleExpression(String(projection.key), true)));
    }
    return vue3CoreRuntime.createObjectExpression(properties);
  }
  return materializeVue3SlotProjectionNode(projection, node, context);
}

function materializeVue3SlotProjectionNode(projection, node, context) {
  if (!projection || projection.kind === 'undefined') return undefined;
  if (projection.type) return projection;
  for (const name of projection.helpers || []) {
    const symbol = helperSymbolFromProjection(name);
    if (symbol && context) context.helper(symbol);
  }
  switch (projection.kind) {
    case 'simple':
      return vue3CoreRuntime.createSimpleExpression(
        projection.content || '',
        !!projection.isStatic,
        projection.loc || (node && node.loc) || vue3CoreRuntime.locStub,
        projection.constType || 0,
      );
    case 'compound':
      return vue3CoreRuntime.createCompoundExpression(
        (projection.children || []).map(child => materializeVue3SlotProjectionNode(child, node, context)),
        projection.loc || (node && node.loc) || vue3CoreRuntime.locStub,
      );
    default:
      if (typeof projection === 'string') return projection;
      throw new Error(`Unsupported Rust v-slot projection: ${projection.kind}`);
  }
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
      prop.exp = materializeVue3SlotOutletProjection(mutation.value, node, context);
    }
  }
}

function materializeVue3SlotOutletName(projection, node, context) {
  if (!projection) return '"default"';
  if (projection.kind === 'literal') return projection.value || '"default"';
  return materializeVue3SlotOutletProjection(projection, node, context);
}

function materializeVue3SlotOutletProjection(projection, node, context) {
  if (!projection || projection.kind === 'undefined') return undefined;
  if (typeof projection === 'string') return projection;
  if (projection.type) return projection;
  switch (projection.kind) {
    case 'node': {
      if (projection.path === 'props') {
        const prop = node && node.props && node.props[projection.index];
        return prop && prop[projection.field || 'exp'];
      }
      return undefined;
    }
    case 'simple':
    case 'compound':
      return materializeVue3SlotProjectionNode(projection, node, context);
    default:
      throw new Error(`Unsupported Rust slot outlet projection: ${projection.kind}`);
  }
}

function vue3IfSiblingPayload(siblings) {
  return (siblings || []).map(vue3IfNodePayload);
}

function vue3IfNodePayload(node) {
  if (!node || typeof node !== 'object') return node;
  const payload = {
    type: node.type,
    tag: node.tag,
    tagType: node.tagType,
    content: node.content,
    locSource: node.loc && node.loc.source,
  };
  if (node.type === vue3CoreRuntime.NodeTypes.TEXT_CALL) {
    payload.content = vue3IfNodePayload(node.content);
  }
  if (node.type === vue3CoreRuntime.NodeTypes.IF) {
    payload.branches = (node.branches || []).map(branch => ({
      hasCondition: branch.condition !== undefined,
      userKey: branch.userKey || null,
    }));
  }
  return payload;
}

function vue3IfBranchCodegenPayload(branch) {
  return {
    isTemplateIf: !!(branch && branch.isTemplateIf),
    children: (branch && branch.children || []).map(child => ({
      type: child && child.type,
      memoedCodegenType: vue3MemoedCodegenType(child && child.codegenNode),
    })),
  };
}

function vue3MemoedCodegenType(codegenNode) {
  const node = vue3CoreRuntime.getMemoedVNodeCall(codegenNode);
  return node && node.type;
}

function materializeVue3IfErrors(projection, node, dir, context) {
  if (!projection || !Array.isArray(projection.errors) || !context || !context.onError) return;
  for (const error of projection.errors) {
    const loc = error.loc === 'userKey'
      ? runtimeIfUserKeyLoc(node, dir)
      : error.loc === 'dir'
        ? dir.loc
        : node.loc;
    context.onError(vue3CoreRuntime.createCompilerError(error.code, loc));
  }
}

function runtimeIfUserKeyLoc(node, dir) {
  const key = vue3CoreRuntime.findProp(node, 'key');
  return key && key.loc || dir && dir.loc || node && node.loc;
}

function materializeVue3IfProjection(projection, node, dir) {
  if (!projection || projection.kind === 'undefined') return undefined;
  switch (projection.kind) {
    case 'simple':
      return vue3CoreRuntime.createSimpleExpression(
        projection.content || '',
        !!projection.isStatic,
        projection.loc || (dir && dir.exp && dir.exp.loc) || (node && node.loc),
        projection.constType || 0,
      );
    default:
      throw new Error(`Unsupported Rust v-if projection: ${projection.kind}`);
  }
}

function materializeVue3ForErrors(projection, node, dir, context) {
  if (!projection || !Array.isArray(projection.errors) || !context || !context.onError) return;
  for (const error of projection.errors) {
    context.onError(vue3CoreRuntime.createCompilerError(error.code, vue3ForErrorLoc(error, node, dir)));
  }
}

function materializeVue3ForTemplateKeyErrors(projection, node, dir, context) {
  if (!projection || !Array.isArray(projection.templateKeyErrors) || !context || !context.onError) return;
  for (const error of projection.templateKeyErrors) {
    context.onError(vue3CoreRuntime.createCompilerError(error.code, vue3ForErrorLoc(error, node, dir)));
  }
}

function vue3ForErrorLoc(error, node, dir) {
  if (!error) return dir && dir.loc || node && node.loc || locStub;
  if (error.loc && typeof error.loc === 'object') return error.loc;
  if (error.loc === 'node') return node && node.loc || dir && dir.loc || locStub;
  return dir && dir.loc || node && node.loc || locStub;
}

function materializeVue3ForParseResult(parseResult, dir, context) {
  return {
    source: materializeVue3ForProjectionNode(parseResult && parseResult.source, dir, context),
    value: materializeVue3ForProjectionNode(parseResult && parseResult.value, dir, context),
    key: materializeVue3ForProjectionNode(parseResult && parseResult.key, dir, context),
    index: materializeVue3ForProjectionNode(parseResult && parseResult.index, dir, context),
    finalized: parseResult && parseResult.finalized !== undefined ? !!parseResult.finalized : true,
  };
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

function materializeVue3ForKeyProperty(projection, dir, context) {
  if (!projection || !projection.value) return null;
  return vue3CoreRuntime.createObjectProperty(
    'key',
    materializeVue3ForProjectionNode(projection.value, dir, context),
  );
}

function materializeVue3ForChildBlock(projection, node, forNode, keyProperty, context) {
  projection = projection || {};
  const children = forNode.children || [];
  if (projection.kind === 'slotOutlet') {
    const slotOutlet = projection.path === 'templateChild'
      ? (node.children || [])[projection.index || 0]
      : node;
    const childBlock = slotOutlet && slotOutlet.codegenNode;
    if (projection.path === 'templateChild' && keyProperty && childBlock) {
      runtime.injectProp(childBlock, keyProperty, context);
    }
    return childBlock;
  }
  if (projection.kind === 'fragmentWrapper') {
    return vue3CoreRuntime.createVNodeCall(
      context,
      context.helper(vue3CoreRuntime.FRAGMENT),
      keyProperty ? vue3CoreRuntime.createObjectExpression([keyProperty]) : undefined,
      node.children,
      projection.patchFlag || 64,
      undefined,
      undefined,
      true,
      undefined,
      false,
    );
  }
  const childBlock = children[0] && children[0].codegenNode;
  if (!childBlock) return undefined;
  if (node.tagType === vue3CoreRuntime.ElementTypes.TEMPLATE && keyProperty) {
    vue3CoreRuntime.injectProp(childBlock, keyProperty, context);
  }
  const shouldBeBlock = !!projection.childBlockIsBlock;
  if (childBlock.isBlock !== shouldBeBlock) {
    if (childBlock.isBlock) {
      context.removeHelper(vue3CoreRuntime.OPEN_BLOCK);
      context.removeHelper(vue3CoreRuntime.getVNodeBlockHelper(context.inSSR, childBlock.isComponent));
    } else {
      context.removeHelper(vue3CoreRuntime.getVNodeHelper(context.inSSR, childBlock.isComponent));
    }
  }
  childBlock.isBlock = shouldBeBlock;
  if (childBlock.isBlock) {
    context.helper(vue3CoreRuntime.OPEN_BLOCK);
    context.helper(vue3CoreRuntime.getVNodeBlockHelper(context.inSSR, childBlock.isComponent));
  } else {
    context.helper(vue3CoreRuntime.getVNodeHelper(context.inSSR, childBlock.isComponent));
  }
  return childBlock;
}

function materializeVue3ForProjectionNode(projection, dir, context) {
  if (!projection || projection.kind === 'undefined') return undefined;
  if (projection.type) return projection;
  for (const name of projection.helpers || []) {
    const symbol = helperSymbolFromProjection(name);
    if (symbol && context) context.helper(symbol);
  }
  switch (projection.kind) {
    case 'simple':
      return vue3CoreRuntime.createSimpleExpression(
        projection.content || '',
        !!projection.isStatic,
        projection.loc || (dir && dir.exp && dir.exp.loc) || locStub,
        projection.constType || 0,
      );
    case 'compound':
      return vue3CoreRuntime.createCompoundExpression(
        (projection.children || []).map(child => materializeVue3ForProjectionNode(child, dir, context)),
        projection.loc || (dir && dir.exp && dir.exp.loc) || locStub,
      );
    default:
      if (typeof projection === 'string') return projection;
      throw new Error(`Unsupported Rust v-for projection: ${projection.kind}`);
  }
}

function vue3ResolveComponentContextPayload(context) {
  context = context || {};
  return {
    prefixIdentifiers: !!context.prefixIdentifiers,
    inline: !!context.inline,
    selfName: context.selfName || null,
    bindingMetadata: context.bindingMetadata || {},
    isScriptSetup: context.bindingMetadata && Object.prototype.hasOwnProperty.call(context.bindingMetadata, '__isScriptSetup')
      ? context.bindingMetadata.__isScriptSetup
      : undefined,
    compatIsOnElement: false,
    builtInComponents: vue3BuiltInComponentPayload(context),
  };
}

function vue3BuiltInComponentPayload(context) {
  const names = ['Transition', 'transition', 'TransitionGroup', 'transition-group'];
  const out = [];
  const seen = new Set();
  for (const name of names) {
    let helper;
    try {
      helper = context && context.isBuiltInComponent && context.isBuiltInComponent(name);
    } catch (_) {
      helper = undefined;
    }
    if (!helper) helper = vue3CoreRuntime.isCoreComponent(name);
    const helperName = typeof helper === 'symbol' ? vue3CoreRuntime.helperNameMap[helper] : undefined;
    if (helperName && !seen.has(name)) {
      seen.add(name);
      out.push({ tag: name, helperName });
    }
  }
  return out;
}

function materializeVue3ComponentTypeProjection(projection, node, context) {
  if (!projection) return vue3CoreRuntime.toValidAssetId(node && node.tag || '', 'component');
  const helper = helperSymbolFromProjection(projection.helper);
  switch (projection.kind) {
    case 'dynamic':
      if (helper) context.helper(helper);
      return vue3CoreRuntime.createCallExpression(
        helper || vue3CoreRuntime.RESOLVE_DYNAMIC_COMPONENT,
        [materializeVue3ComponentProjectionNode(projection.argument, node, context)],
      );
    case 'helper':
      if (projection.helperName && context && typeof context.isBuiltInComponent === 'function') {
        const contextHelper = vue3ContextBuiltInComponentSymbol(context, node, projection.helperName);
        if (contextHelper) {
          if (projection.registerHelper !== false) context.helper(contextHelper);
          return contextHelper;
        }
      }
      if (helper && projection.registerHelper !== false) context.helper(helper);
      if (helper) return helper;
      if (projection.helperName) {
        const runtimeHelper = helperSymbolFromHelperName(projection.helperName);
        if (runtimeHelper && projection.registerHelper !== false) context.helper(runtimeHelper);
        return runtimeHelper || `_${projection.helperName}`;
      }
      return projection.helper;
    case 'expression':
      for (const name of projection.helpers || []) {
        const symbol = helperSymbolFromProjection(name);
        if (symbol) context.helper(symbol);
      }
      return projection.content || '';
    case 'asset':
      if (helper) context.helper(helper);
      if (projection.component) context.components.add(projection.component);
      return projection.assetId || vue3CoreRuntime.toValidAssetId(node && node.tag || '', 'component');
    default:
      throw new Error(`Unsupported Rust component projection: ${projection.kind}`);
  }
}

function vue3ContextBuiltInComponentSymbol(context, node, helperName) {
  const tag = node && node.tag;
  const names = [tag];
  if (helperName === 'Transition') names.push('Transition', 'transition');
  else if (helperName === 'TransitionGroup') names.push('TransitionGroup', 'transition-group');
  else if (helperName === 'BaseTransition') names.push('BaseTransition', 'base-transition');
  for (const name of names) {
    if (!name) continue;
    try {
      const helper = context.isBuiltInComponent(name);
      if (typeof helper === 'symbol' && vue3CoreRuntime.helperNameMap[helper] === helperName) {
        return helper;
      }
    } catch (_) {}
  }
  return undefined;
}

function materializeVue3ComponentProjectionNode(projection, node, context) {
  if (!projection || projection.kind === 'undefined') return undefined;
  if (projection.type) return projection;
  switch (projection.kind) {
    case 'simple':
      return vue3CoreRuntime.createSimpleExpression(
        projection.content || '',
        !!projection.isStatic,
        projection.loc || (node && node.loc) || locStub,
        projection.constType || 0,
      );
    case 'expression':
      for (const name of projection.helpers || []) {
        const symbol = helperSymbolFromProjection(name);
        if (symbol && context) context.helper(symbol);
      }
      return projection.content || '';
    default:
      return projection;
  }
}

function helperSymbolFromProjection(name, context) {
  if (!name) return undefined;
  if (context && context.__vuecDomHelpers && typeof context.__vuecDomHelpers[name] === 'symbol') {
    return context.__vuecDomHelpers[name];
  }
  if (vue3CoreRuntime[name]) {
    const direct = vue3CoreRuntime[name];
    const helperName = typeof direct === 'symbol' ? vue3CoreRuntime.helperNameMap[direct] : undefined;
    return helperSymbolFromHelperName(helperName) || direct;
  }
  return helperSymbolFromHelperName(name);
}

function helperSymbolFromHelperName(name) {
  if (!name) return undefined;
  const keys = Reflect.ownKeys(vue3CoreRuntime.helperNameMap);
  for (let index = keys.length - 1; index >= 0; index--) {
    const key = keys[index];
    if (typeof key === 'symbol' && vue3CoreRuntime.helperNameMap[key] === name) return key;
  }
  return Object.values(vue3CoreRuntime).find(value => {
    return typeof value === 'symbol' && vue3CoreRuntime.helperNameMap[value] === name;
  });
}

function createRootCodegen(root, context) {
  const projection = callBridge('vue3.core.rootCodegen', { root });
  if (!projection || projection.kind === 'none') return;
  if (projection.kind === 'child') {
    root.codegenNode = (root.children || [])[projection.index || 0];
    return;
  }
  if (projection.kind === 'childCodegen') {
    const child = (root.children || [])[projection.index || 0];
    const codegenNode = child && child.codegenNode;
    if (codegenNode && projection.asBlock) {
      vue3CoreRuntime.convertToBlock(codegenNode, context);
    }
    root.codegenNode = codegenNode;
    return;
  }
  if (projection.kind === 'fragment') {
    root.codegenNode = vue3CoreRuntime.createVNodeCall(
      context,
      context.helper(vue3CoreRuntime.FRAGMENT),
      undefined,
      root.children || [],
      projection.patchFlag,
      undefined,
      undefined,
      true,
      undefined,
      false,
    );
    return;
  }
  throw new Error(`Unsupported Rust root codegen projection: ${projection.kind}`);
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
    const error = new SyntaxError(vue3CoreRuntime.errorMessages[diagnostic.code] || 'Vue compiler parse error');
    error.code = diagnostic.code;
    error.loc = diagnostic.loc;
    onError(error);
  }
  delete ast.__vuecDiagnostics;
}

function hydrateVue3Node(node) {
  if (!node || typeof node !== 'object') return node;
  if (node.type === vue3CoreRuntime.NodeTypes.ROOT) {
    node.helpers = new Set(node.helpers || []);
    node.components = node.components || [];
    node.directives = node.directives || [];
    node.hoists = node.hoists || [];
    node.imports = node.imports || [];
    node.cached = node.cached || [];
    node.temps = node.temps || 0;
    if (node.codegenNode === null) node.codegenNode = undefined;
  }
  if (node.type === vue3CoreRuntime.NodeTypes.ELEMENT) {
    if (node.codegenNode === null) node.codegenNode = undefined;
    if (node.isSelfClosing === null) delete node.isSelfClosing;
  }
  if (node.type === vue3CoreRuntime.NodeTypes.ATTRIBUTE) {
    if (node.value === null) node.value = undefined;
  }
  if (node.type === vue3CoreRuntime.NodeTypes.DIRECTIVE) {
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

/* vuec-runtime-fragment: bridge-shape-adapter/node-bridge-call */
function callBridge(command, payload) {
  recordVuecProvenance(`bridge:${command}`);
  const result = cp.spawnSync(BRIDGE_BIN, [command], {
    input: JSON.stringify(payload || {}),
    encoding: 'utf8',
    maxBuffer: 64 * 1024 * 1024
  });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    const error = new Error(result.stderr || result.stdout || `vuec bridge command failed: ${command}`);
    error.code = 'VUEC_BRIDGE_FAILED';
    throw error;
  }
  return result.stdout.trim() ? JSON.parse(result.stdout) : undefined;
}

/* vuec-runtime-fragment: suite-helper/runtime-entrypoint */
const vuecBridgeRuntime = { callBridge };

/* vuec-runtime-fragment: package-api-adapter/public-package-shapes */
function normalizeArgs(payload) {
  return payload || {};
}

function resolveStylePreprocessPayload(payload) {
  if (!payload || !payload.options || typeof payload.options !== 'object') return payload;
  const options = payload.options;
  const preprocessOptions = options.preprocessOptions;
  if (!preprocessOptions || typeof preprocessOptions !== 'object') return payload;
  if (typeof preprocessOptions.additionalData !== 'function') return payload;
  const source = payload.source == null ? '' : String(payload.source);
  const resolvedOptions = Object.assign({}, options, {
    preprocessOptions: Object.assign({}, preprocessOptions, {
      additionalData: preprocessOptions.additionalData(source, options.filename)
    })
  });
  return Object.assign({}, payload, { options: resolvedOptions });
}

function bridgePayloadForCall(payload) {
  if (!payload || !Object.prototype.hasOwnProperty.call(payload, 'bridgeOptions')) return payload || {};
  const bridgePayload = {};
  for (const key of Object.keys(payload)) {
    if (key === 'options') {
      bridgePayload.options = payload.bridgeOptions;
    } else if (key !== 'bridgeOptions') {
      bridgePayload[key] = payload[key];
    }
  }
  return bridgePayload;
}

function vue27StyleBridgePayload(payload) {
  if (!payload || !payload.options || typeof payload.options !== 'object') return payload;
  const options = payload.options;
  const bridgeOptions = {};
  for (const key of Object.keys(options)) {
    if (
      key !== 'postcssPlugins' &&
      key !== 'postcssOptions' &&
      key !== 'sourceMap' &&
      key !== 'source_map'
    ) {
      bridgeOptions[key] = options[key];
    }
  }
  return Object.assign({}, payload, { options: bridgeOptions });
}

function vue3StyleBridgePayload(payload) {
  if (!payload || !payload.options || typeof payload.options !== 'object') return payload;
  const options = payload.options;
  const bridgeOptions = {};
  for (const key of Object.keys(options)) {
    if (key !== 'sourceMap' && key !== 'source_map') {
      bridgeOptions[key] = options[key];
    }
  }
  return Object.assign({}, payload, { options: bridgeOptions });
}

function normalizeStyleAliasResult(result) {
  if (!result || typeof result !== 'object' || result.map !== null) return result;
  const out = Object.assign({}, result);
  out.map = undefined;
  return out;
}

function hydrateVue3SfcParseResult(result) {
  if (!result || typeof result !== 'object' || !result.descriptor) return result;
  const descriptor = result.descriptor;
  descriptor.shouldForceReload = function shouldForceReload(prevImports) {
    return vue3SfcShouldForceReload(prevImports, descriptor);
  };
  return result;
}

function throwVue3CompileScriptErrors(result) {
  if (!result || !Array.isArray(result.errors) || result.errors.length === 0) {
    return result;
  }
  const first = result.errors[0];
  const message = typeof first === 'string' ? first : (first && first.message) || String(first);
  throw new Error(message.startsWith('[@vue/compiler-sfc]') ? message : `[@vue/compiler-sfc] ${message}`);
}

function hydrateVue3CompileScriptResult(result) {
  result = throwVue3CompileScriptErrors(result);
  if (!result || typeof result !== 'object') return result;
  const bindings = result.bindings;
  if (bindings && typeof bindings === 'object' && result.propsAliases && typeof result.propsAliases === 'object') {
    bindings.__propsAliases = result.propsAliases;
    delete result.propsAliases;
  }
  if (Array.isArray(result.warnings)) {
    for (const warning of result.warnings) {
      if (warning == null) continue;
      const message = typeof warning === 'string' ? warning : (warning && warning.message) || String(warning);
      console.warn(message.startsWith('[@vue/compiler-sfc]') ? message : `[@vue/compiler-sfc] ${message}`);
    }
    delete result.warnings;
  }
  if (bindings && typeof bindings === 'object' && Object.prototype.hasOwnProperty.call(bindings, '__isScriptSetup')) {
    const isScriptSetup = bindings.__isScriptSetup === true || bindings.__isScriptSetup === 'true';
    delete bindings.__isScriptSetup;
    Object.defineProperty(bindings, '__isScriptSetup', {
      enumerable: false,
      configurable: true,
      value: isScriptSetup
    });
  }
  return result;
}

function vue3CompileScriptBridgePayload(payload) {
  const out = Object.assign({}, payload || {});
  const options = Object.assign({}, out.options || {});
  const filename = options.filename !== undefined ? options.filename : out.filename;
  if (typeof options.customElement === 'function') {
    try {
      options.__vuecCustomElement = !!options.customElement(filename);
    } catch (_) {
      options.__vuecCustomElement = false;
    }
    delete options.customElement;
  }
  if (typeof __TEST__ !== 'undefined' && __TEST__ === true) {
    options.__vuecEmitScriptSetupMarker = false;
  }
  out.options = options;
  return out;
}

function vue3SfcCompileTemplateBridgePayload(payload) {
  const out = Object.assign({}, payload || {});
  const options = Object.assign({}, out.options || {});
  const source = String(out.source || '');
  const bridgeOptions = vue3SfcCompileTemplateOptionsForBridge(options, source);
  if (options.ast) {
    out.ast = vue3CoreRuntime.dehydrateForBridge(options.ast);
    if (options.ast.source && !bridgeOptions.__vuecSourceMapSource) {
      bridgeOptions.__vuecSourceMapSource = options.ast.source;
      bridgeOptions.__vuecSourceMapBaseOffset = 0;
    }
  }
  out.options = options;
  out.bridgeOptions = bridgeOptions;
  return out;
}

function vue3SfcCompileTemplateOptionsForBridge(options, source) {
  const compilerOptions = options && options.compilerOptions && typeof options.compilerOptions === 'object'
    ? options.compilerOptions
    : {};
  const bridgeOptions = Object.assign({}, normalizeVue3OptionsForBridge(options, source));
  delete bridgeOptions.ast;
  delete bridgeOptions.compiler;
  delete bridgeOptions.compilerOptions;
  Object.assign(bridgeOptions, {
    mode: 'module',
    prefixIdentifiers: true,
    hoistStatic: true,
    cacheHandlers: true,
    sourceMap: true,
  });
  if (options && options.filename !== undefined) bridgeOptions.filename = options.filename;
  if (options && options.id !== undefined) bridgeOptions.id = options.id;
  if (options && options.scoped) {
    const shortId = String(options.id || '').replace(/^data-v-/, '');
    bridgeOptions.scopeId = `data-v-${shortId}`;
    bridgeOptions.scoped = true;
  } else {
    delete bridgeOptions.scopeId;
    delete bridgeOptions.scope_id;
  }
  if (options && options.slotted !== undefined) bridgeOptions.slotted = options.slotted;
  if (options && options.ssr !== undefined) bridgeOptions.ssr = options.ssr;
  if (options && options.ssrCssVars !== undefined) {
    bridgeOptions.ssrCssVars = options.ssrCssVars;
  }
  if (options && options.isProd !== undefined) bridgeOptions.isProd = options.isProd;
  if (options && options.preprocessLang !== undefined) bridgeOptions.preprocessLang = options.preprocessLang;
  if (options && options.transformAssetUrls !== undefined) bridgeOptions.transformAssetUrls = options.transformAssetUrls;
  Object.assign(bridgeOptions, normalizeVue3OptionsForBridge(compilerOptions, source));
  bridgeOptions.hmr = !(options && options.isProd);
  if (compilerOptions && compilerOptions.nodeTransforms && !Array.isArray(compilerOptions.nodeTransforms)) {
    delete bridgeOptions.nodeTransforms;
  }
  return bridgeOptions;
}

function vue3SfcCustomCompileTemplateResult(payload) {
  const options = payload && payload.options;
  const compiler = options && options.compiler;
  if (!compiler || typeof compiler.compile !== 'function') return undefined;
  const source = String(payload && payload.source || '');
  const compilerOptions = vue3SfcCompileTemplateOptionsForBridge(options, source);
  const result = compiler.compile(source, compilerOptions) || {};
  return hydrateVue3SfcCompileTemplateResult({
    code: result.code || '',
    ast: result.ast,
    preamble: result.preamble,
    map: result.map,
    source,
    errors: result.errors || [],
    tips: result.tips || [],
  });
}

function hydrateVue3SfcCompileTemplateResult(result) {
  if (!result || typeof result !== 'object') return result;
  const out = Object.assign({}, result);
  if (typeof out.ast === 'string') {
    try {
      out.ast = JSON.parse(out.ast);
    } catch (_) {}
  }
  if (out.ast && typeof out.ast === 'object' && out.ast.type === vue3CoreRuntime.NodeTypes.ROOT) {
    out.ast = hydrateVue3Ast(out.ast, {});
  }
  if (Array.isArray(out.errors)) {
    out.errors = out.errors.map(vue3SfcTemplateErrorForPublicApi);
  } else {
    out.errors = [];
  }
  if (!Array.isArray(out.tips)) out.tips = [];
  return out;
}

function vue3SfcTemplateErrorForPublicApi(error) {
  if (typeof error === 'string') return error;
  if (!error || typeof error !== 'object') return error;
  if (error instanceof Error) return error;
  const message = error.message || error.msg || String(error);
  const syntaxError = new SyntaxError(message);
  if (error.code !== undefined) syntaxError.code = error.code;
  if (error.loc !== undefined) syntaxError.loc = error.loc;
  return syntaxError;
}

function vue3SfcShouldForceReload(prevImports, descriptor) {
  const scriptSetup = descriptor && descriptor.scriptSetup;
  if (!scriptSetup || (scriptSetup.lang !== 'ts' && scriptSetup.lang !== 'tsx')) {
    return false;
  }
  for (const key in prevImports) {
    if (!prevImports[key].isUsedInTemplate && vue3SfcIsImportUsed(key, descriptor)) {
      return true;
    }
  }
  return false;
}

function vue3SfcIsImportUsed(local, descriptor) {
  return vue3SfcTemplateUsedIdentifiers(descriptor).has(local);
}

function vue3SfcTemplateUsedIdentifiers(descriptor) {
  const template = descriptor.template;
  const ids = new Set();
  const children = template.ast && Array.isArray(template.ast.children) ? template.ast.children : [];
  children.forEach(node => collectVue3SfcTemplateIds(node, ids));
  return ids;
}

function collectVue3SfcTemplateIds(node, ids) {
  if (!node || typeof node !== 'object') return;
  if (node.type === vue3CoreRuntime.NodeTypes.ELEMENT) {
    let tag = String(node.tag || '');
    if (tag.includes('.')) tag = tag.split('.')[0].trim();
    if (tag && !vue3SfcIsNativeTag(tag) && !vue3SfcIsDomBuiltInComponent(tag)) {
      ids.add(camelize(tag));
      ids.add(capitalize(camelize(tag)));
    }
    for (const prop of node.props || []) {
      if (prop && prop.type === vue3CoreRuntime.NodeTypes.DIRECTIVE) {
        if (!vue3CoreRuntime.isBuiltInDirective(prop.name)) {
          ids.add(`v${capitalize(camelize(prop.name))}`);
        }
        if (prop.arg && !prop.arg.isStatic) {
          collectVue3SfcExpressionIds(prop.arg, ids);
        }
        if (prop.name === 'for' && prop.forParseResult && prop.forParseResult.source) {
          collectVue3SfcExpressionIds(prop.forParseResult.source, ids);
        } else if (prop.exp) {
          collectVue3SfcExpressionIds(prop.exp, ids);
        } else if (prop.name === 'bind' && prop.arg && prop.arg.content) {
          ids.add(camelize(prop.arg.content));
        }
      } else if (prop && prop.type === vue3CoreRuntime.NodeTypes.ATTRIBUTE && prop.name === 'ref' && prop.value && prop.value.content) {
        ids.add(prop.value.content);
      }
    }
    for (const child of node.children || []) {
      collectVue3SfcTemplateIds(child, ids);
    }
  } else if (node.type === vue3CoreRuntime.NodeTypes.INTERPOLATION) {
    collectVue3SfcExpressionIds(node.content, ids);
  }
}

function collectVue3SfcExpressionIds(exp, ids) {
  if (!exp) return;
  if (exp.ast) {
    collectVue3SfcAstIds(exp.ast, ids, null);
  } else if (exp.ast === null) {
    collectVue3SfcStringExpressionIds(exp.content, ids);
  } else if (exp.content) {
    collectVue3SfcStringExpressionIds(exp.content, ids);
  }
}

function collectVue3SfcAstIds(node, ids, parent) {
  if (!node || typeof node !== 'object') return;
  if (Array.isArray(node)) {
    node.forEach(child => collectVue3SfcAstIds(child, ids, parent));
    return;
  }
  if (node.type === 'Identifier') {
    if (parent && parent.type === 'MemberExpression' && parent.property === node && !parent.computed) return;
    if (parent && (parent.type === 'ObjectProperty' || parent.type === 'Property') && parent.key === node && !parent.computed) return;
    ids.add(node.name);
  }
  for (const key of Object.keys(node)) {
    if (key === 'parent' || key === 'loc') continue;
    const value = node[key];
    if (value && typeof value === 'object') {
      collectVue3SfcAstIds(value, ids, node);
    }
  }
}

function collectVue3SfcStringExpressionIds(source, ids) {
  const text = String(source || '');
  const pattern = /[A-Za-z_$][\w$]*/g;
  let match;
  while ((match = pattern.exec(text))) {
    const before = text.slice(0, match.index).trimEnd();
    if (!before.endsWith('.')) ids.add(match[0]);
  }
}

function vue3SfcIsNativeTag(tag) {
  return /^(?:html|body|base|head|link|meta|style|title|address|article|aside|footer|header|hgroup|h1|h2|h3|h4|h5|h6|nav|section|div|dd|dl|dt|figcaption|figure|picture|hr|img|li|main|ol|p|pre|ul|a|b|abbr|bdi|bdo|br|cite|code|data|dfn|em|i|kbd|mark|q|rp|rt|ruby|s|samp|small|span|strong|sub|sup|time|u|var|wbr|area|audio|map|track|video|embed|object|param|source|canvas|script|noscript|del|ins|caption|col|colgroup|table|thead|tbody|td|th|tr|button|datalist|fieldset|form|input|label|legend|meter|optgroup|option|output|progress|select|textarea|details|dialog|menu|summary|template|blockquote|iframe|tfoot|svg|math)$/i.test(String(tag || ''));
}

function vue3SfcIsDomBuiltInComponent(tag) {
  return tag === 'Transition' || tag === 'transition' || tag === 'TransitionGroup' || tag === 'transition-group';
}

function hydrateVue3SsrCompileResult(result, options, source) {
  if (!result || typeof result !== 'object') return result;
  emitVue3CompileDiagnostics(result, options, source);
  if (Array.isArray(result.ast_helpers)) {
    const helpers = new Set(result.ast_helpers.map(name => Symbol(name)));
    delete result.ast_helpers;
    result.ast = Object.assign({}, result.ast || {}, { helpers });
  }
  return result;
}

function vue27StylePostcssRequired(options) {
  return !!(
    options &&
    (Array.isArray(options.postcssPlugins) || options.postcssOptions)
  );
}

function vue27StylePostcssOptions(options) {
  const postcssOptions = Object.assign({}, options && options.postcssOptions ? options.postcssOptions : {});
  const filename = options && options.filename ? options.filename : undefined;
  if (filename !== undefined) {
    if (postcssOptions.to === undefined) postcssOptions.to = filename;
    if (postcssOptions.from === undefined) postcssOptions.from = filename;
  }
  return postcssOptions;
}

/* vuec-runtime-fragment: callback-boundary/js-callback-materialization */
function applyVue27StylePostcssSync(result, options) {
  if (!vue27StylePostcssRequired(options)) return normalizeStyleAliasResult(result);
  const out = Object.assign({}, result);
  const errors = Array.isArray(out.errors) ? out.errors.slice() : [];
  let rawResult;
  try {
    const postcss = require('postcss');
    recordVuecProvenance('callback.postcssPlugin');
    rawResult = postcss((options && options.postcssPlugins) || []).process(
      out.code || '',
      vue27StylePostcssOptions(options)
    );
    out.code = rawResult.css || '';
    out.map = rawResult.map && rawResult.map.toJSON ? rawResult.map.toJSON() : out.map;
  } catch (error) {
    errors.push(error);
  }
  out.errors = errors;
  out.rawResult = rawResult;
  return normalizeStyleAliasResult(out);
}

function applyVue27StylePostcssAsync(result, options) {
  const out = Object.assign({}, result);
  const errors = Array.isArray(out.errors) ? out.errors.slice() : [];
  if (!vue27StylePostcssRequired(options)) {
    return Promise.resolve(normalizeStyleAliasResult(out));
  }
  try {
    const postcss = require('postcss');
    recordVuecProvenance('callback.postcssPlugin');
    const rawResult = postcss((options && options.postcssPlugins) || []).process(
      out.code || '',
      vue27StylePostcssOptions(options)
    );
    return Promise.resolve(rawResult)
      .then(postcssResult => {
        out.code = postcssResult.css || '';
        out.map = postcssResult.map && postcssResult.map.toJSON ? postcssResult.map.toJSON() : out.map;
        out.errors = errors;
        out.rawResult = postcssResult;
        return normalizeStyleAliasResult(out);
      })
      .catch(error => ({
        code: '',
        map: undefined,
        errors: errors.concat(error && error.message ? error.message : error),
        rawResult: undefined,
      }));
  } catch (error) {
    return Promise.resolve({
      code: '',
      map: undefined,
      errors: errors.concat(error && error.message ? error.message : error),
      rawResult: undefined,
    });
  }
}

function hydrateVue27CompileScriptResult(result) {
  if (!result || typeof result !== 'object') return result;
  const bindings = result.bindings;
  if (bindings && typeof bindings === 'object' && Object.prototype.hasOwnProperty.call(bindings, '__isScriptSetup')) {
    const isScriptSetup = bindings.__isScriptSetup === true || bindings.__isScriptSetup === 'true';
    delete bindings.__isScriptSetup;
    Object.defineProperty(bindings, '__isScriptSetup', {
      enumerable: false,
      configurable: true,
      value: isScriptSetup
    });
  }
  return result;
}

function vue27CompileScriptBridgePayload(payload) {
  const out = Object.assign({}, payload || {});
  const options = Object.assign({}, out.options || {});
  if (typeof __TEST__ !== 'undefined' && __TEST__ === true) {
    options.__vuecEmitScriptSetupMarker = false;
  }
  out.options = options;
  return out;
}

function vue27SfcCompileTemplateBridgePayload(payload) {
  const out = Object.assign({}, payload || {});
  const options = Object.assign({}, out.options || {});
  if (
    options.isProduction === undefined &&
    options.isProd === undefined &&
    options.is_prod === undefined
  ) {
    options.isProduction = process.env.NODE_ENV === 'production';
  }
  out.options = options;
  return out;
}

function prettifyVue27SfcTemplateResult(result, options, filename) {
  if (!result || typeof result !== 'object') return result;
  const out = Object.assign({}, result);
  const errors = Array.isArray(out.errors) ? out.errors : [];
  if (errors.length > 0) return out;
  if (vue27SfcTemplateIsProduction(options)) return out;
  if (!vue27SfcTemplatePrettifyEnabled(options)) return out;
  const tips = Array.isArray(out.tips) ? out.tips.slice() : [];
  try {
    out.code = require('prettier').format(out.code || '', {
      semi: false,
      parser: 'babel'
    });
  } catch (error) {
    if (error && error.code === 'MODULE_NOT_FOUND') {
      tips.push(
        'The `prettify` option is on, but the dependency `prettier` is not found.\n' +
        'Please either turn off `prettify` or manually install `prettier`.'
      );
    }
    tips.push(
      `Failed to prettify component ${filename || (options && options.filename) || 'anonymous.vue'} template source after compilation.`
    );
    out.tips = tips;
  }
  return out;
}

function vue27SfcTemplateIsProduction(options) {
  if (!options || typeof options !== 'object') return false;
  return options.isProduction === true || options.isProd === true || options.is_prod === true;
}

function vue27SfcTemplatePrettifyEnabled(options) {
  if (!options || typeof options !== 'object') return true;
  if (!Object.prototype.hasOwnProperty.call(options, 'prettify')) return true;
  return !!options.prettify;
}

function vue3SfcParseBridgePayload(payload) {
  const out = Object.assign({}, payload || {});
  const source = String(out.source || '');
  out.bridgeOptions = normalizeVue3SfcParseOptionsForBridge(out.options, source);
  return out;
}

function normalizeVue3SfcParseOptionsForBridge(options, source) {
  if (!options || typeof options !== 'object') return {};
  const normalized = {};
  for (const key of Object.keys(options)) {
    if (key !== 'compiler' && typeof options[key] !== 'function') normalized[key] = options[key];
  }
  if (options.templateParseOptions && typeof options.templateParseOptions === 'object') {
    normalized.templateParseOptions = normalizeVue3OptionsForBridge(
      options.templateParseOptions,
      source,
    );
  }
  return normalized;
}

function applyVue3SfcCustomCompilerParse(result, source, options, filename) {
  if (!result || typeof result !== 'object' || !result.descriptor) return result;
  const compiler = options && options.compiler;
  if (!compiler || typeof compiler.parse !== 'function') return result;
  const customErrors = [];
  const ast = compiler.parse(String(source || ''), Object.assign({}, options.templateParseOptions || {}, {
    parseMode: 'sfc',
    prefixIdentifiers: true,
    onError: error => customErrors.push(error),
  }));
  const out = Object.assign({}, result);
  out.errors = customErrors.concat(Array.isArray(result.errors) ? result.errors : []);
  if (ast && Array.isArray(ast.children) && ast.children.length === 0) {
    out.errors.push(new SyntaxError(
      `At least one <template> or <script> is required in a single file component. ${filename || 'anonymous.vue'}`
    ));
  }
  return out;
}

function vue3BridgePayload(source, filename, options) {
  warnIgnoredDecodeEntities(options);
  return {
    source,
    filename,
    options,
    bridgeOptions: normalizeVue3OptionsForBridge(options, source),
  };
}

function vue3CompileBridgePayload(input, filename, options) {
  if (input && typeof input === 'object' && input.type === vue3CoreRuntime.NodeTypes.ROOT && Array.isArray(input.children)) {
    const source = typeof input.source === 'string' ? input.source : '';
    const normalizedSource = vue3AstTemplateSource(input, source);
    warnIgnoredDecodeEntities(options);
    return {
      source: normalizedSource,
      filename,
      options,
      ast: vue3CoreRuntime.dehydrateForBridge(input),
      bridgeOptions: Object.assign(
        normalizeVue3OptionsForBridge(options, normalizedSource),
        { __vuecSourceMapSource: source, __vuecSourceMapBaseOffset: 0 },
      ),
    };
  }
  return vue3BridgePayload(input && input.source ? input.source : input, filename, options);
}

function vue3AstTemplateSource(ast, source) {
  const children = Array.isArray(ast && ast.children) ? ast.children : [];
  if (!children.length) return '';
  let start = Infinity;
  let end = -Infinity;
  for (const child of children) {
    const locStart = child && child.loc && child.loc.start && child.loc.start.offset;
    const locEnd = child && child.loc && child.loc.end && child.loc.end.offset;
    if (Number.isFinite(locStart) && Number.isFinite(locEnd) && locEnd >= locStart) {
      start = Math.min(start, locStart);
      end = Math.max(end, locEnd);
    }
  }
  return Number.isFinite(start) && end >= start ? String(source || '').slice(start, end) : source;
}

function warnIgnoredDecodeEntities(options) {
  if (!options || typeof options !== 'object' || typeof options.decodeEntities !== 'function') return;
  const message = '[Vue warn]: decodeEntities option is passed but will be ignored in non-browser builds.';
  if (!globalThis.__VUEC_DECODE_ENTITIES_WARNED__) {
    globalThis.__VUEC_DECODE_ENTITIES_WARNED__ = true;
  }
  console.warn(message);
}

function emitVue2CompileWarnings(result, options) {
  const suppressed = options && options.__vuecSuppressWarnings;
  if (suppressed === true) return;
  if (!result || typeof result !== 'object') return;
  const warnings = [];
  if (Array.isArray(result.errors)) warnings.push(...result.errors);
  if (Array.isArray(result.tips)) warnings.push(...result.tips);
  const suppressedMessages = Array.isArray(suppressed) ? suppressed.map(String) : [];
  for (const warning of warnings) {
    if (warning == null) continue;
    const message = typeof warning === 'string'
      ? warning
      : typeof warning.msg === 'string'
        ? warning.msg
        : null;
    if (message == null) continue;
    if (suppressedMessages.some(suppressed => message.includes(suppressed))) continue;
    if (typeof warning === 'string') {
      console.error(message);
    } else {
      console.error(message);
    }
  }
}

function hydrateVue2CompileResult(result) {
  if (!result || typeof result !== 'object') return result;
  const staticRenderFns = Array.isArray(result.staticRenderFns)
    ? result.staticRenderFns
    : Array.isArray(result.static_render_fns)
      ? result.static_render_fns
      : [];
  const out = {
    ast: result.ast_public !== undefined ? result.ast_public : result.ast,
    render: result.render || '',
    staticRenderFns,
    errors: Array.isArray(result.errors) ? result.errors : [],
    tips: Array.isArray(result.tips) ? result.tips : [],
  };
  for (const key of [
    'ast_document',
    'element_ast',
    'ast_public',
    'element_public_ast',
    'static_render_fns',
    'diagnostics',
  ]) {
    if (Object.prototype.hasOwnProperty.call(result, key)) {
      Object.defineProperty(out, key, {
        value: result[key],
        enumerable: false,
        configurable: true,
        writable: true,
      });
    }
  }
  return out;
}

function normalizeVue3OptionsForBridge(options, source) {
  if (!options || typeof options !== 'object') return {};
  const normalized = {};
  for (const key of Object.keys(options)) {
    if (typeof options[key] !== 'function') normalized[key] = options[key];
  }
  const tags = extractVueTemplateTags(String(source || ''));
  if (hasVuePredicateOption(options, 'isVoidTag')) {
    normalized.__vuecVoidTags = collectVuePredicateHits(options.isVoidTag, tags);
  }
  if (hasVuePredicateOption(options, 'isPreTag')) {
    normalized.__vuecPreTags = collectVuePredicateHits(options.isPreTag, tags);
  }
  if (hasVuePredicateOption(options, 'isIgnoreNewlineTag')) {
    normalized.__vuecIgnoreNewlineTags = collectVuePredicateHits(options.isIgnoreNewlineTag, tags);
  }
  if (typeof options.getNamespace === 'function') {
    normalized.__vuecNamespaces = collectVueNamespaceHits(options.getNamespace, tags);
    normalized.__vuecDomNamespaces = true;
  }
  if (Object.prototype.hasOwnProperty.call(options, 'ns')) {
    normalized.__vuecRootNamespace = options.ns;
  }
  if (hasVuePredicateOption(options, 'isNativeTag')) {
    normalized.__vuecNativeTags = collectVuePredicateHits(options.isNativeTag, tags);
  }
  normalized.__vuecCustomElements = collectVuePredicateHits(options.isCustomElement, tags);
  normalized.__vuecBuiltInComponents = collectVuePredicateHits(options.isBuiltInComponent, tags);
  normalized.__vuecStringifyStatic = typeof options.transformHoist === 'function';
  return normalized;
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

function usesAliasRuntimeCompile(options) {
  if (!options || typeof options !== 'object') return false;
  if (Array.isArray(options.nodeTransforms) && options.nodeTransforms.some(transform => typeof transform === 'function')) {
    return true;
  }
  if (options.directiveTransforms && typeof options.directiveTransforms === 'object') {
    return Object.values(options.directiveTransforms).some(transform => typeof transform === 'function');
  }
  return typeof options.transformHoist === 'function';
}

function emitVue3CompileDiagnostics(result, options, source) {
  if (!result || !Array.isArray(result.diagnostics)) return;
  const onError = options && typeof options.onError === 'function'
    ? options.onError
    : null;
  const onWarn = options && typeof options.onWarn === 'function'
    ? options.onWarn
    : null;
  for (const diagnostic of result.diagnostics) {
    const severity = String(diagnostic && diagnostic.severity || 'error').toLowerCase();
    if (severity === 'warning') {
      if (onWarn) onWarn(vue3DiagnosticError(diagnostic, source));
      continue;
    }
    const error = vue3DiagnosticError(diagnostic, source);
    if (onError) {
      onError(error);
    } else {
      throw error;
    }
  }
  delete result.diagnostics;
}

function vue3DiagnosticError(diagnostic, source) {
  if (typeof diagnostic === 'string') return new SyntaxError(diagnostic);
  const message = diagnostic && diagnostic.message;
  const error = new SyntaxError(message || 'Vue compiler error');
  const code = diagnostic && diagnostic.code !== undefined ? Number(diagnostic.code) : 64;
  error.code = Number.isNaN(code) ? diagnostic.code : code;
  error.loc = vue3DiagnosticLoc(diagnostic, source);
  return error;
}

function vue3DiagnosticLoc(diagnostic, source) {
  if (diagnostic && diagnostic.loc) return diagnostic.loc;
  const span = diagnostic && diagnostic.span;
  if (!span || span.start == null || span.end == null) return undefined;
  const start = Number(span.start) || 0;
  const end = Math.max(start, Number(span.end) || start);
  return vue3SourceLocValue(String(source || ''), start, end);
}

function vue3SourceLocValue(source, start, end) {
  const localStart = Math.min(start, source.length);
  const localEnd = Math.max(localStart, Math.min(end, source.length));
  return {
    start: vue3Position(source, start),
    end: vue3Position(source, end),
    source: source.slice(localStart, localEnd),
  };
}

function vue3Position(source, offset) {
  let line = 1;
  let column = 1;
  let utf16Offset = 0;
  for (let index = 0; index < source.length && index < offset;) {
    const codePoint = source.codePointAt(index);
    const size = codePoint > 0xffff ? 2 : 1;
    if (codePoint === 10) {
      line += 1;
      column = 1;
    } else {
      column += size;
    }
    utf16Offset += size;
    index += size;
  }
  if (offset > source.length) {
    const extra = offset - source.length;
    column += extra;
    utf16Offset += extra;
  }
  return { offset: utf16Offset, line, column };
}

function emitVue3StyleWarnings(result) {
  if (!result || !Array.isArray(result.diagnostics) || !result.diagnostics.length) return result;
  const diagnostics = [];
  for (const diagnostic of result.diagnostics) {
    const severity = diagnostic && diagnostic.severity;
    const code = diagnostic && diagnostic.code;
    const message = typeof diagnostic === 'string' ? diagnostic : diagnostic && diagnostic.message;
    if (severity === 'warning' && code === 'VUEC_STYLE_DEPRECATED_SCOPED_SELECTOR' && message) {
      console.warn(`[@vue/compiler-sfc] ${message}`);
    } else {
      diagnostics.push(diagnostic);
    }
  }
  if (diagnostics.length === result.diagnostics.length) return result;
  const out = Object.assign({}, result);
  if (diagnostics.length) {
    out.diagnostics = diagnostics;
  } else {
    delete out.diagnostics;
  }
  return out;
}

const vue3DomParserOptions = {
  parseMode: 'html',
  isVoidTag: tag => /^(?:area|base|br|col|embed|hr|img|input|link|meta|param|source|track|wbr)$/i.test(String(tag || '')),
  isNativeTag: tag => /^(?:html|body|base|head|link|meta|style|title|address|article|aside|footer|header|hgroup|h1|h2|h3|h4|h5|h6|nav|section|div|dd|dl|dt|figcaption|figure|picture|hr|img|li|main|ol|p|pre|ul|a|b|abbr|bdi|bdo|br|cite|code|data|dfn|em|i|kbd|mark|q|rp|rt|ruby|s|samp|small|span|strong|sub|sup|time|u|var|wbr|area|audio|map|track|video|embed|object|param|source|canvas|script|noscript|del|ins|caption|col|colgroup|table|thead|tbody|td|th|tr|button|datalist|fieldset|form|input|label|legend|meter|optgroup|option|output|progress|select|textarea|details|dialog|menu|summary|template|blockquote|iframe|tfoot|svg|math)$/i.test(String(tag || '')),
  isPreTag: tag => String(tag || '').toLowerCase() === 'pre',
  isIgnoreNewlineTag: tag => /^(?:pre|textarea)$/i.test(String(tag || '')),
  decodeEntities: vue3CoreRuntime.decodeHtmlBrowser,
  isBuiltInComponent: tag => {
    if (tag === 'Transition' || tag === 'transition') return vue3CoreRuntime.TRANSITION;
    if (tag === 'TransitionGroup' || tag === 'transition-group') return vue3CoreRuntime.TRANSITION_GROUP;
    return undefined;
  },
  getNamespace: (_tag, parent, rootNamespace) => parent && parent.ns !== undefined ? parent.ns : rootNamespace,
};

function preflightAliasCall(name, payload) {
  if (name === 'vue3.core.baseCompile') {
    const options = payload && payload.options ? payload.options : {};
    const isModuleMode = options.mode === 'module';
    const prefixIdentifiers = options.prefixIdentifiers === true || isModuleMode;
    if (!prefixIdentifiers && options.cacheHandlers) {
      throwCompilerSyntaxError(50, '"cacheHandlers" option is only supported when the "prefixIdentifiers" option is enabled.');
    }
    if (options.scopeId && !isModuleMode) {
      throwCompilerSyntaxError(51, '"scopeId" option is only supported in module mode.');
    }
  }
}

function throwCompilerSyntaxError(code, message) {
  const error = new SyntaxError(message);
  error.code = code;
  error.loc = undefined;
  throw error;
}

function extractStyleSource(source) {
  const match = String(source || '').match(/<style[^>]*>([\s\S]*?)<\/style>/i);
  return match ? match[1] : String(source || '');
}

function notImplemented(name) {
  const error = new Error(`Rust Vue compiler alias export ${name} is not implemented yet`);
  error.code = 'VUEC_NOT_IMPLEMENTED';
  throw error;
}

function namedArity(name, arity, fn) {
  const bound = fn.bind(null);
  Object.defineProperty(bound, 'name', { value: name, configurable: true });
  Object.defineProperty(bound, 'length', { value: arity, configurable: true });
  return bound;
}
