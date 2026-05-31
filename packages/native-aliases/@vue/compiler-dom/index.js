'use strict';

const core = require('@vue/compiler-core');
const native = require('@vuec-rs/native');

const helperNameMap = core.helperNameMap;

const TRANSITION = registerHelper('Transition', 'Transition');
const TRANSITION_GROUP = registerHelper('TransitionGroup', 'TransitionGroup');
const V_MODEL_RADIO = registerHelper('vModelRadio', 'vModelRadio');
const V_MODEL_CHECKBOX = registerHelper('vModelCheckbox', 'vModelCheckbox');
const V_MODEL_TEXT = registerHelper('vModelText', 'vModelText');
const V_MODEL_SELECT = registerHelper('vModelSelect', 'vModelSelect');
const V_MODEL_DYNAMIC = registerHelper('vModelDynamic', 'vModelDynamic');
const V_ON_WITH_MODIFIERS = registerHelper('vOnModifiersGuard', 'withModifiers');
const V_ON_WITH_KEYS = registerHelper('vOnKeysGuard', 'withKeys');
const V_SHOW = registerHelper('vShow', 'vShow');

const DOMErrorCodes = enumObject(54, [
  'X_V_HTML_NO_EXPRESSION',
  'X_V_HTML_WITH_CHILDREN',
  'X_V_TEXT_NO_EXPRESSION',
  'X_V_TEXT_WITH_CHILDREN',
  'X_V_MODEL_ON_INVALID_ELEMENT',
  'X_V_MODEL_ARG_ON_ELEMENT',
  'X_V_MODEL_ON_FILE_INPUT_ELEMENT',
  'X_V_MODEL_UNNECESSARY_VALUE',
  'X_V_SHOW_NO_EXPRESSION',
  'X_TRANSITION_INVALID_CHILDREN',
  'X_IGNORED_SIDE_EFFECT_TAG',
  '__EXTEND_POINT__',
]);

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

function callVue3DomProjection(command, payload) {
  return native.callVue3DomProjection(command, payload || {});
}

const transformStyle = (node) => {
  if (!node || node.type !== core.NodeTypes.ELEMENT) return undefined;
  const projection = callVue3DomProjection('vue3.dom.transformStyle', { node });
  for (const replacement of projection && projection.replacements || []) {
    const original = node.props && node.props[replacement.index];
    if (!original || original.type !== core.NodeTypes.ATTRIBUTE) continue;
    node.props[replacement.index] = {
      type: core.NodeTypes.DIRECTIVE,
      name: 'bind',
      rawName: ':style',
      arg: core.createSimpleExpression('style', true, original.loc),
      exp: core.createSimpleExpression(
        replacement.expression || '{}',
        false,
        original.loc,
        core.ConstantTypes.CAN_STRINGIFY,
      ),
      modifiers: [],
      loc: original.loc,
    };
  }
  return undefined;
};

function domDirectiveLoc(loc, dir, node) {
  if (loc && typeof loc === 'object') return loc;
  if (loc === 'dir') return (dir && dir.loc) || core.locStub;
  if (loc === 'node') return (node && node.loc) || (dir && dir.loc) || core.locStub;
  return core.locStub;
}

function materializeDomDirectiveValue(projection, dir, node, context) {
  if (!projection || projection.kind === 'undefined') return undefined;
  if (projection.type) return projection;
  switch (projection.kind) {
    case 'node':
      if (projection.path === 'dir.exp') return dir && dir.exp;
      if (projection.path === 'dir.arg') return dir && dir.arg;
      return undefined;
    case 'simple': {
      const loc = Object.prototype.hasOwnProperty.call(projection, 'loc')
        ? domDirectiveLoc(projection.loc, dir, node)
        : core.locStub;
      const simple = core.createSimpleExpression(projection.content || '', !!projection.isStatic, loc);
      if (projection.constType !== undefined) simple.constType = projection.constType;
      return simple;
    }
    case 'displayString': {
      const helper = core.TO_DISPLAY_STRING;
      const callee = context && typeof context.helperString === 'function'
        ? context.helperString(helper)
        : `_${helperNameMap[helper] || 'toDisplayString'}`;
      return core.createCallExpression(
        callee,
        [materializeDomDirectiveValue(projection.argument, dir, node, context)],
        domDirectiveLoc(projection.loc, dir, node),
      );
    }
    default:
      throw new Error(`Unsupported Rust Vue 3 DOM directive projection: ${projection.kind}`);
  }
}

function materializeDomDirectiveErrors(projection, dir, node, context) {
  if (!projection || !Array.isArray(projection.errors) || !context || typeof context.onError !== 'function') return;
  for (const error of projection.errors) {
    context.onError(createDOMCompilerError(error.code, domDirectiveLoc(error.loc, dir, node)));
  }
}

function materializeDomModelErrors(projection, dir, node, context) {
  if (!projection || !Array.isArray(projection.errors) || !context || typeof context.onError !== 'function') return;
  for (const error of projection.errors) {
    const code = typeof error === 'number' ? error : error.code;
    const loc = domDirectiveLoc(error && error.loc, dir, node);
    context.onError(
      code >= DOMErrorCodes.X_V_HTML_NO_EXPRESSION
        ? createDOMCompilerError(code, loc)
        : core.createCompilerError(code, loc),
    );
  }
}

function materializeDomContentDirective(command, dir, node, context) {
  context = context || {
    helperString: name => `_${helperNameMap[name] || name}`,
    onError: error => { throw error; },
  };
  const projection = callVue3DomProjection(command, { dir, node });
  materializeDomDirectiveErrors(projection, dir, node, context);
  if (projection && projection.clearChildren && node && Array.isArray(node.children)) {
    node.children.length = 0;
  }
  return {
    props: (projection && projection.props || []).map(prop => {
      const key = prop.keyLoc
        ? core.createSimpleExpression(prop.key || '', true, domDirectiveLoc(prop.keyLoc, dir, node))
        : (prop.key || '');
      return core.createObjectProperty(key, materializeDomDirectiveValue(prop.value, dir, node, context));
    }),
  };
}

function domTransformContextPayload(context) {
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

function domModelTransformContextPayload(context, node) {
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
    expressionPlugins: context.expressionPlugins || [],
    isCustomElement,
  };
}

function materializeDomOnProjection(projection, dir, context) {
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
        const materialized = materializeDomOnProjection(child, dir, context);
        if (Array.isArray(materialized)) children.push(...materialized);
        else children.push(materialized);
      }
      return children;
    }
    case 'helperString': {
      const helper = helperSymbolFromProjection(projection.helper, context);
      return `${context && helper ? context.helperString(helper) : `_${helperNameMap[helper]}`}(`;
    }
    case 'static':
      return core.createSimpleExpression(projection.content || '', true, projection.loc || (dir && dir.loc) || core.locStub);
    case 'simple': {
      registerDomProjectionHelpers(projection, context);
      const node = core.createSimpleExpression(
        projection.content || '',
        !!projection.isStatic,
        projection.loc || (dir && dir.exp && dir.exp.loc) || (dir && dir.arg && dir.arg.loc) || (dir && dir.loc) || core.locStub,
      );
      if (projection.constType !== undefined) node.constType = projection.constType;
      return node;
    }
    case 'compound': {
      registerDomProjectionHelpers(projection, context);
      const children = [];
      for (const child of projection.children || []) {
        const materialized = materializeDomOnProjection(child, dir, context);
        if (Array.isArray(materialized)) children.push(...materialized);
        else children.push(materialized);
      }
      const node = core.createCompoundExpression(children);
      node.loc = projection.loc || (dir && dir.arg && dir.arg.loc) || (dir && dir.exp && dir.exp.loc) || core.locStub;
      return node;
    }
    case 'call': {
      const helper = helperSymbolFromProjection(projection.callee || projection.helper, context);
      if (helper && context && typeof context.helper === 'function') context.helper(helper);
      const args = (projection.arguments || []).map(arg => materializeDomOnProjection(arg, dir, context));
      return core.createCallExpression(helper || projection.callee || projection.helper, args, projection.loc || (dir && dir.loc) || core.locStub);
    }
    default:
      throw new Error(`Unsupported Rust Vue 3 DOM v-on projection: ${projection.kind}`);
  }
}

function registerDomProjectionHelpers(projection, context) {
  if (!context || typeof context.helper !== 'function') return;
  for (const helperName of projection.helpers || []) {
    const helper = helperSymbolFromProjection(helperName, context);
    if (helper) context.helper(helper);
  }
}

const transformOn = (dir, node, context, _augmentor) => {
  context = context || {
    helper: name => name,
    helperString: name => `_${helperNameMap[name] || name}`,
    cache: value => value,
    onError: error => { throw error; },
  };
  const projection = callVue3DomProjection('vue3.dom.transformOn', {
    dir,
    node,
    context: domTransformContextPayload(context),
  });
  materializeDomDirectiveErrors(projection, dir, node, context);
  const onMeta = (projection && projection.props || []).map(prop => ({
    cache: !!prop.cache,
    handlerKey: !!prop.handlerKey,
    dynamicKey: !!prop.dynamicKey,
    ignoreDynamicKeyForNormalize: !!prop.ignoreDynamicKeyForNormalize,
    valueConstant: !!prop.valueConstant,
  }));
  const result = {
    props: (projection && projection.props || []).map(prop => core.createObjectProperty(
      materializeDomOnProjection(prop.key, dir, context),
      materializeDomOnProjection(prop.value, dir, context) || core.createSimpleExpression('() => {}', false, dir && dir.loc || core.locStub),
    )),
  };
  for (const [index, prop] of (result.props || []).entries()) {
    const meta = onMeta[index] || onMeta[0] || {};
    if (prop.key && meta.handlerKey) prop.key.isHandlerKey = true;
    if (meta.cache && context && typeof context.cache === 'function') prop.value = context.cache(prop.value);
    prop.__vuecOn = meta;
  }
  return result;
};

const transformVHtml = (dir, node, context) => {
  return materializeDomContentDirective('vue3.dom.transformVHtml', dir, node, context);
};

const transformVText = (dir, node, context) => {
  return materializeDomContentDirective('vue3.dom.transformVText', dir, node, context);
};

const transformModel = (dir, node, context) => {
  context = context || {
    helper: name => name,
    cache: value => value,
    onError: error => { throw error; },
  };
  const projection = callVue3DomProjection('vue3.dom.transformModel', {
    dir,
    node,
    context: domModelTransformContextPayload(context, node),
  });
  materializeDomModelErrors(projection, dir, node, context);
  const result = {
    props: (projection && projection.props || []).map(prop => {
      const key = materializeDomOnProjection(prop.key, dir, context);
      const value = materializeDomOnProjection(prop.value, dir, context);
      const objectProp = core.createObjectProperty(key, value);
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
  if (projection && projection.needRuntime) {
    const helper = helperSymbolFromProjection(projection.needRuntime, context);
    result.needRuntime = helper && context && typeof context.helper === 'function'
      ? context.helper(helper)
      : helper || projection.needRuntime;
  }
  return result;
};

const transformShow = (dir, node, context) => {
  context = context || {
    onError: error => { throw error; },
  };
  const projection = callVue3DomProjection('vue3.dom.transformShow', { dir });
  materializeDomDirectiveErrors(projection, dir, node, context);
  return {
    props: [],
    needRuntime: projection && projection.needRuntime === 'V_SHOW'
      ? V_SHOW
      : projection && projection.needRuntime,
  };
};

function domNodeIsTransition(node, context) {
  if (!node || node.type !== core.NodeTypes.ELEMENT || node.tagType !== core.ElementTypes.COMPONENT) return false;
  if (!context || typeof context.isBuiltInComponent !== 'function') return false;
  let component;
  try {
    component = context.isBuiltInComponent(node.tag);
  } catch (_error) {
    component = undefined;
  }
  const transition = context.__vuecDomHelpers && context.__vuecDomHelpers.TRANSITION || TRANSITION;
  return component === transition || helperNameMap[component] === 'Transition';
}

function domTransitionContextPayload(context, node) {
  return {
    isTransition: domNodeIsTransition(node, context),
  };
}

function materializeDomTransitionProjection(projection, node, context) {
  if (!projection || !projection.transform || !node) return;
  if (Array.isArray(projection.keepChildren) && Array.isArray(node.children)) {
    node.children = projection.keepChildren
      .map(index => node.children[index])
      .filter(Boolean);
  }
  materializeDomDirectiveErrors(projection, null, node, context);
  if (projection.injectPersisted) {
    node.props = node.props || [];
    node.props.push({
      type: core.NodeTypes.ATTRIBUTE,
      name: 'persisted',
      nameLoc: node.loc,
      value: undefined,
      loc: node.loc,
    });
  }
}

const transformTransition = (node, context) => {
  if (!domNodeIsTransition(node, context)) return undefined;
  return () => {
    const projection = callVue3DomProjection('vue3.dom.transformTransition', {
      node,
      context: domTransitionContextPayload(context, node),
    });
    materializeDomTransitionProjection(projection, node, context);
  };
};

const DOMNodeTransforms = [
  transformStyle,
  transformTransition,
  function validateHtmlNesting(node, context) {
    return undefined;
  },
];

const DOMDirectiveTransforms = {
  cloak: core.noopDirectiveTransform,
  html: transformVHtml,
  text: transformVText,
  model: transformModel,
  on: transformOn,
  show: transformShow,
};

const parserOptions = {
  parseMode: 'html',
  isVoidTag(tag) {
    return /^(area|base|br|col|embed|hr|img|input|link|meta|param|source|track|wbr)$/i.test(String(tag || ''));
  },
  isNativeTag(tag) {
    const name = String(tag || '');
    return isHtmlTag(name) || isSvgTag(name) || isMathMlTag(name);
  },
  isPreTag(tag) {
    return String(tag || '').toLowerCase() === 'pre';
  },
  isIgnoreNewlineTag(tag) {
    return /^(textarea|pre)$/i.test(String(tag || ''));
  },
  isBuiltInComponent(tag) {
    const name = String(tag || '');
    if (/^transition$/i.test(name)) return TRANSITION;
    if (/^transition-group$/i.test(name) || /^transitiongroup$/i.test(name)) return TRANSITION_GROUP;
    return undefined;
  },
  decodeEntities(rawText, asAttr) {
    return String(rawText || '')
      .replace(/&lt;/g, '<')
      .replace(/&gt;/g, '>')
      .replace(/&amp;/g, '&')
      .replace(/&quot;/g, '"')
      .replace(/&#39;/g, "'");
  },
  getNamespace(tag, parent, rootNamespace) {
    const name = String(tag || '');
    if (name === 'svg') return 1;
    if (name === 'math') return 2;
    if (parent && parent.ns === 1 && /^(foreignObject|desc|title)$/.test(parent.tag || '')) return 0;
    return rootNamespace == null ? 0 : rootNamespace;
  },
};

function compile(src) {
  const options = arguments.length > 1 ? arguments[1] : undefined;
  if (src && typeof src === 'object' && src.type === core.NodeTypes.ROOT && Array.isArray(src.children)) {
    const payload = vue3AstCompilePayload(src, options);
    return projectVue3DomCompileResult(
      native.compileVue3Dom(payload.source, vue3DomNativeOptions(payload.options, payload.source)),
      payload.options,
      payload.source,
    );
  }
  const source = String(src || '');
  return projectVue3DomCompileResult(
    native.compileVue3Dom(source, vue3DomNativeOptions(options, source)),
    options,
    source,
  );
}

function parse(template) {
  const source = String(template || '');
  const options = arguments.length > 1 ? arguments[1] : undefined;
  return hydrateVue3DomAst(
    native.parseVue3Dom(source, vue3DomNativeOptions(options, source)),
    options,
  );
}

function vue3DomNativeOptions(options, source) {
  options = options || {};
  const out = {};
  for (const key of Object.keys(options)) {
    if (typeof options[key] !== 'function') out[key] = options[key];
  }
  if (typeof options.transformHoist === 'function') {
    out.stringifyStatic = true;
    out.__vuecStringifyStaticPreserveHelpers = true;
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

function vue3AstCompilePayload(ast, options) {
  const source = typeof ast.source === 'string' ? ast.source : '';
  const range = vue3AstChildrenRange(ast, source);
  const template = range ? source.slice(range.start, range.end) : source;
  return {
    source: template,
    options: {
      ...(options || {}),
      __vuecTemplateBaseOffset: range ? range.start : 0,
      __vuecSourceMapSource: source,
      __vuecSourceMapBaseOffset: 0,
    },
  };
}

function vue3AstChildrenRange(ast, source) {
  const children = Array.isArray(ast && ast.children) ? ast.children : [];
  if (!children.length) return { start: 0, end: 0 };
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
  if (!Number.isFinite(start) || end < start) return null;
  return { start, end };
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

function hydrateVue3DomAst(ast, options) {
  emitVue3ParseDiagnostics(ast, options);
  hydrateVue3DomNode(ast);
  return ast;
}

function emitVue3ParseDiagnostics(ast, options) {
  if (!ast || !Array.isArray(ast.__vuecDiagnostics)) return;
  const onError = options && typeof options.onError === 'function'
    ? options.onError
    : error => { throw error; };
  for (const diagnostic of ast.__vuecDiagnostics) {
    const error = new SyntaxError(diagnostic.message || 'Vue compiler parse error');
    error.code = diagnostic.code;
    error.loc = diagnostic.loc;
    onError(error);
  }
  delete ast.__vuecDiagnostics;
}

function hydrateVue3DomNode(node) {
  if (!node || typeof node !== 'object') return node;
  if (node.type === core.NodeTypes.ROOT) {
    node.helpers = new Set(Array.from(node.helpers || [], helper => helperSymbolFromProjection(helper) || helper));
    node.components = node.components || [];
    node.directives = node.directives || [];
    node.hoists = node.hoists || [];
    node.imports = node.imports || [];
    node.cached = node.cached || [];
    node.temps = node.temps || 0;
    if (node.codegenNode === null) node.codegenNode = undefined;
  }
  if (node.type === core.NodeTypes.ELEMENT) {
    if (node.codegenNode === null) node.codegenNode = undefined;
    if (node.isSelfClosing === null) delete node.isSelfClosing;
  }
  if (node.type === core.NodeTypes.ATTRIBUTE) {
    if (node.value === null) node.value = undefined;
  }
  if (node.type === core.NodeTypes.DIRECTIVE) {
    if (node.exp === null) node.exp = undefined;
    if (node.arg === null) node.arg = undefined;
  }
  if (node.type === core.NodeTypes.JS_CALL_EXPRESSION && typeof node.callee === 'string') {
    node.callee = helperSymbolFromProjection(node.callee) || node.callee;
  }
  if (Array.isArray(node.children)) node.children.forEach(hydrateVue3DomNode);
  if (Array.isArray(node.props)) node.props.forEach(hydrateVue3DomNode);
  if (Array.isArray(node.modifiers)) node.modifiers.forEach(hydrateVue3DomNode);
  if (Array.isArray(node.arguments)) node.arguments.forEach(hydrateVue3DomNode);
  if (Array.isArray(node.elements)) node.elements.forEach(hydrateVue3DomNode);
  if (Array.isArray(node.properties)) node.properties.forEach(hydrateVue3DomNode);
  if (Array.isArray(node.params)) node.params.forEach(hydrateVue3DomNode);
  if (Array.isArray(node.returns)) node.returns.forEach(hydrateVue3DomNode);
  if (Array.isArray(node.hoists)) node.hoists.forEach(hydrateVue3DomNode);
  if (Array.isArray(node.imports)) node.imports.forEach(hydrateVue3DomNode);
  if (Array.isArray(node.cached)) node.cached.forEach(hydrateVue3DomNode);
  if (node.content && typeof node.content === 'object') hydrateVue3DomNode(node.content);
  if (node.codegenNode && typeof node.codegenNode === 'object') hydrateVue3DomNode(node.codegenNode);
  if (node.value && typeof node.value === 'object') hydrateVue3DomNode(node.value);
  if (node.key && typeof node.key === 'object') hydrateVue3DomNode(node.key);
  if (node.exp && typeof node.exp === 'object') hydrateVue3DomNode(node.exp);
  if (node.arg && typeof node.arg === 'object') hydrateVue3DomNode(node.arg);
  return node;
}

function helperSymbolFromProjection(name, context) {
  if (!name) return undefined;
  if (context && context.__vuecDomHelpers && typeof context.__vuecDomHelpers[name] === 'symbol') {
    return context.__vuecDomHelpers[name];
  }
  const direct = domProjectionHelperSymbol(name)
    || (core && core[name] && typeof core[name] === 'symbol' ? core[name] : undefined);
  const helperName = direct && helperNameMap[direct] || domProjectionHelperName(name) || name;
  return helperSymbolFromHelperName(helperName) || direct;
}

function helperSymbolFromHelperName(name) {
  if (!name) return undefined;
  const keys = Reflect.ownKeys(helperNameMap);
  for (let index = keys.length - 1; index >= 0; index -= 1) {
    const key = keys[index];
    if (typeof key === 'symbol' && helperNameMap[key] === name) return key;
  }
  for (const value of Object.values(core || {})) {
    if (typeof value === 'symbol' && helperNameMap[value] === name) return value;
  }
  return undefined;
}

function domProjectionHelperSymbol(name) {
  switch (name) {
    case 'TRANSITION': return TRANSITION;
    case 'TRANSITION_GROUP': return TRANSITION_GROUP;
    case 'V_MODEL_RADIO': return V_MODEL_RADIO;
    case 'V_MODEL_CHECKBOX': return V_MODEL_CHECKBOX;
    case 'V_MODEL_TEXT': return V_MODEL_TEXT;
    case 'V_MODEL_SELECT': return V_MODEL_SELECT;
    case 'V_MODEL_DYNAMIC': return V_MODEL_DYNAMIC;
    case 'V_ON_WITH_MODIFIERS': return V_ON_WITH_MODIFIERS;
    case 'V_ON_WITH_KEYS': return V_ON_WITH_KEYS;
    case 'V_SHOW': return V_SHOW;
    default: return undefined;
  }
}

function domProjectionHelperName(name) {
  switch (name) {
    case 'TRANSITION': return 'Transition';
    case 'TRANSITION_GROUP': return 'TransitionGroup';
    case 'V_MODEL_RADIO': return 'vModelRadio';
    case 'V_MODEL_CHECKBOX': return 'vModelCheckbox';
    case 'V_MODEL_TEXT': return 'vModelText';
    case 'V_MODEL_SELECT': return 'vModelSelect';
    case 'V_MODEL_DYNAMIC': return 'vModelDynamic';
    case 'V_ON_WITH_MODIFIERS': return 'withModifiers';
    case 'V_ON_WITH_KEYS': return 'withKeys';
    case 'V_SHOW': return 'vShow';
    default: return undefined;
  }
}

function projectVue3DomCompileResult(result, options, source) {
  emitVue3CompileDiagnostics(result, options, source);
  return result;
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
    const severity = String(diagnostic.severity || '').toLowerCase();
    if (severity === 'error') {
      const error = vue3DiagnosticError(diagnostic, source);
      if (onError) {
        onError(error);
      } else {
        throw error;
      }
    } else if (severity === 'warning' && onWarn) {
      onWarn(vue3DiagnosticError(diagnostic, source));
    }
  }
  delete result.diagnostics;
}

function vue3DiagnosticError(diagnostic, source) {
  const code = diagnostic && diagnostic.code != null
    ? Number(diagnostic.code)
    : 0;
  const loc = vue3DiagnosticLoc(diagnostic, source);
  const error = new SyntaxError(String(diagnostic && diagnostic.message || 'Vue compiler error'));
  error.code = Number.isNaN(code) ? diagnostic.code : code;
  error.loc = loc;
  return error;
}

function vue3DiagnosticLoc(diagnostic, source) {
  const span = diagnostic && diagnostic.span;
  if (!span || !span.start || !span.end) {
    return {
      start: { line: 1, column: 1, offset: 0 },
      end: { line: 1, column: 1, offset: 0 },
      source: '',
    };
  }
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

function createDOMCompilerError(code, loc) {
  return core.createCompilerError(code, loc, DOMErrorMessages);
}

function registerHelper(symbolName, runtimeName) {
  const symbol = Symbol(symbolName);
  helperNameMap[symbol] = runtimeName;
  return symbol;
}

function enumObject(start, names) {
  const out = {};
  names.forEach((name, index) => {
    const value = start + index;
    out[value] = name;
    out[name] = value;
  });
  return out;
}

function isHtmlTag(tag) {
  return HTML_TAGS.has(tag.toLowerCase());
}

function isSvgTag(tag) {
  return SVG_TAGS.has(tag);
}

function isMathMlTag(tag) {
  return MATH_ML_TAGS.has(tag);
}

const HTML_TAGS = new Set(
  'html,body,base,head,link,meta,style,title,address,article,aside,footer,header,hgroup,h1,h2,h3,h4,h5,h6,nav,section,div,dd,dl,dt,figcaption,figure,picture,hr,img,li,main,ol,p,pre,ul,a,b,abbr,bdi,bdo,br,cite,code,data,dfn,em,i,kbd,mark,q,rp,rt,ruby,s,samp,small,span,strong,sub,sup,time,u,var,wbr,area,audio,map,track,video,embed,object,param,source,canvas,script,noscript,del,ins,caption,col,colgroup,table,thead,tbody,td,th,tr,button,datalist,fieldset,form,input,label,legend,meter,optgroup,option,output,progress,select,textarea,details,dialog,menu,summary,template,blockquote,iframe,tfoot'.split(','),
);

const SVG_TAGS = new Set(
  'svg,animate,animateMotion,animateTransform,circle,clipPath,color-profile,defs,desc,discard,ellipse,feBlend,feColorMatrix,feComponentTransfer,feComposite,feConvolveMatrix,feDiffuseLighting,feDisplacementMap,feDistantLight,feDropShadow,feFlood,feFuncA,feFuncB,feFuncG,feFuncR,feGaussianBlur,feImage,feMerge,feMergeNode,feMorphology,feOffset,fePointLight,feSpecularLighting,feSpotLight,feTile,feTurbulence,filter,foreignObject,g,hatch,hatchpath,image,line,linearGradient,marker,mask,mesh,meshgradient,meshpatch,meshrow,metadata,mpath,path,pattern,polygon,polyline,radialGradient,rect,set,solidcolor,stop,switch,symbol,text,textPath,title,tspan,unknown,use,view'.split(','),
);

const MATH_ML_TAGS = new Set(
  'math,maction,maligngroup,malignmark,menclose,merror,mfenced,mfrac,mi,mlabeledtr,mlongdiv,mmultiscripts,mn,mo,mover,mpadded,mphantom,ms,mspace,msqrt,mstyle,msub,msup,msubsup,mtable,mtd,mtext,mtr,munder,munderover,none,semantics'.split(','),
);

module.exports = {
  ...core,
  DOMDirectiveTransforms,
  DOMErrorCodes,
  DOMErrorMessages,
  DOMNodeTransforms,
  TRANSITION,
  TRANSITION_GROUP,
  V_MODEL_CHECKBOX,
  V_MODEL_DYNAMIC,
  V_MODEL_RADIO,
  V_MODEL_SELECT,
  V_MODEL_TEXT,
  V_ON_WITH_KEYS,
  V_ON_WITH_MODIFIERS,
  V_SHOW,
  compile,
  createDOMCompilerError,
  parse,
  parserOptions,
  transformModel,
  transformOn,
  transformStyle,
};

Object.defineProperty(module.exports, '__vuecRuntime', {
  value: {
    ...module.exports,
    transformStyle,
    transformVHtml,
    transformVText,
    transformShow,
    transformModel,
    transformOn,
    transformTransition,
  },
  enumerable: false,
});
